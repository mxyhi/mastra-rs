mod adapter;
mod error;
mod oidc;
mod request;
mod types;

pub use adapter::{
    Authenticator, CallbackAdapter, CallbackHandler, CallbackSessionAuthenticator,
    SessionBackedAuthenticator, SessionResolver,
};
pub use error::AuthError;
pub use oidc::{JwksSource, OidcConfiguration, OidcJwtAuthenticator};
pub use request::AuthRequestParts;
pub use types::{AuthIdentity, CallbackResult, Claims, timestamp_to_utc};

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use chrono::Utc;
    use jsonwebtoken::jwk::JwkSet;
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    use serde_json::json;

    use super::*;

    fn signing_secret() -> &'static [u8] {
        b"super-secret-for-tests"
    }

    fn inline_hs256_jwks() -> JwkSet {
        let mut jwk = jsonwebtoken::jwk::Jwk::from_encoding_key(
            &EncodingKey::from_secret(signing_secret()),
            Algorithm::HS256,
        )
        .unwrap();
        jwk.common.key_id = Some("test-key".to_string());
        JwkSet { keys: vec![jwk] }
    }

    fn bearer_request(token: &str) -> AuthRequestParts {
        AuthRequestParts::default().with_authorization(format!("Bearer {token}"))
    }

    fn test_identity(provider: &str, subject: &str) -> AuthIdentity {
        AuthIdentity {
            provider: provider.to_string(),
            subject: subject.to_string(),
            issuer: Some("https://issuer.example.com".to_string()),
            audiences: vec!["mastra".to_string()],
            expires_at: Some(Utc::now()),
            issued_at: None,
            not_before: None,
            session_id: Some("session-123".to_string()),
            claims: Claims::new(),
        }
    }

    struct StubSessionResolver {
        identity: Option<AuthIdentity>,
    }

    #[async_trait]
    impl SessionResolver for StubSessionResolver {
        async fn resolve_session(
            &self,
            _request: &AuthRequestParts,
        ) -> Result<Option<AuthIdentity>, AuthError> {
            Ok(self.identity.clone())
        }
    }

    struct StubCallbackAdapter {
        result: Option<CallbackResult>,
    }

    #[async_trait]
    impl CallbackAdapter for StubCallbackAdapter {
        async fn handle_callback(
            &self,
            _request: &AuthRequestParts,
        ) -> Result<Option<CallbackResult>, AuthError> {
            Ok(self.result.clone())
        }
    }

    #[test]
    fn extracts_bearer_token_from_request() {
        let request = bearer_request("token-123");
        assert_eq!(request.bearer_token().unwrap(), Some("token-123"));
    }

    #[test]
    fn rejects_non_bearer_authorization_scheme() {
        let request = AuthRequestParts::default().with_authorization("Basic abc");
        let error = request.bearer_token().unwrap_err();
        assert!(matches!(
            error,
            AuthError::UnsupportedAuthorizationScheme(ref scheme) if scheme == "Basic"
        ));
    }

    #[tokio::test]
    async fn oidc_authenticator_verifies_inline_jwk_token() {
        let claims = json!({
            "sub": "user_123",
            "iss": "https://issuer.example.com",
            "aud": "mastra",
            "exp": Utc::now().timestamp() + 3600,
            "sid": "session-123"
        });
        let token = jsonwebtoken::encode(
            &Header {
                alg: Algorithm::HS256,
                kid: Some("test-key".to_string()),
                ..Header::default()
            },
            &claims,
            &EncodingKey::from_secret(signing_secret()),
        )
        .unwrap();
        let authenticator = OidcJwtAuthenticator::new(
            OidcConfiguration::new(
                "test-provider",
                "https://issuer.example.com",
                vec!["mastra".to_string()],
                JwksSource::Inline(inline_hs256_jwks()),
            )
            .with_algorithms(vec![Algorithm::HS256]),
        );

        let identity = authenticator
            .authenticate(&bearer_request(&token))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(identity.provider, "test-provider");
        assert_eq!(identity.subject, "user_123");
        assert_eq!(identity.session_id.as_deref(), Some("session-123"));
        assert_eq!(identity.audiences, vec!["mastra".to_string()]);
    }

    #[tokio::test]
    async fn session_backed_authenticator_delegates_to_resolver() {
        let authenticator = SessionBackedAuthenticator::new(
            "better-auth",
            StubSessionResolver {
                identity: Some(test_identity("better-auth", "user-session")),
            },
        );

        let identity = authenticator
            .authenticate(&AuthRequestParts::default().with_cookie_header("session=abc"))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(identity.provider, "better-auth");
        assert_eq!(identity.subject, "user-session");
    }

    #[tokio::test]
    async fn callback_session_authenticator_exposes_callback_result() {
        let identity = test_identity("cloud", "user-callback");
        let callback_result = CallbackResult {
            identity: identity.clone(),
            access_token: "access-token".to_string(),
            refresh_token: Some("refresh-token".to_string()),
            id_token: None,
            set_cookie_headers: vec!["session=abc; HttpOnly".to_string()],
        };
        let authenticator = CallbackSessionAuthenticator::new(
            "cloud",
            StubSessionResolver { identity: None },
            StubCallbackAdapter {
                result: Some(callback_result.clone()),
            },
        );

        let result = authenticator
            .handle_callback(&AuthRequestParts::default().with_callback("code", "state"))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(result, callback_result);
    }
}
