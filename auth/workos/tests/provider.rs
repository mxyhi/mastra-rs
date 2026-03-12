use async_trait::async_trait;
use chrono::Utc;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use mastra_auth_workos::{MastraAuthWorkos, MastraAuthWorkosOptions, WorkosSessionClient};
use mastra_packages_auth::{
    AuthError, AuthIdentity, AuthRequestParts, Authenticator, Claims, JwksSource,
};
use serde_json::json;

fn signing_secret() -> &'static [u8] {
    b"workos-provider-secret"
}

fn inline_hs256_jwks() -> JwkSet {
    let mut jwk = jsonwebtoken::jwk::Jwk::from_encoding_key(
        &EncodingKey::from_secret(signing_secret()),
        Algorithm::HS256,
    )
    .expect("jwk");
    jwk.common.key_id = Some("workos-key".to_string());
    JwkSet { keys: vec![jwk] }
}

#[derive(Default)]
struct StubSessionClient;

#[async_trait]
impl WorkosSessionClient for StubSessionClient {
    async fn resolve_session(
        &self,
        session_token: &str,
    ) -> Result<Option<AuthIdentity>, AuthError> {
        Ok(Some(AuthIdentity {
            provider: "ignored".to_string(),
            subject: format!("workos-session:{session_token}"),
            issuer: None,
            audiences: Vec::new(),
            expires_at: None,
            issued_at: None,
            not_before: None,
            session_id: Some(session_token.to_string()),
            claims: Claims::new(),
        }))
    }
}

#[tokio::test]
async fn workos_prefers_session_cookie_when_present() {
    let provider = MastraAuthWorkos::new(
        MastraAuthWorkosOptions::default()
            .with_jwks(JwksSource::Inline(inline_hs256_jwks()))
            .with_algorithms(vec![Algorithm::HS256]),
        StubSessionClient,
    )
    .expect("provider");

    let identity = provider
        .authenticate(&AuthRequestParts::default().with_cookie_header("wos-session=session-token"))
        .await
        .expect("auth ok")
        .expect("identity");

    assert_eq!(provider.session_cookie_name(), "wos-session");
    assert_eq!(identity.provider, "workos");
    assert_eq!(identity.subject, "workos-session:session-token");
}

#[tokio::test]
async fn workos_falls_back_to_jwt_authentication_for_bearer_tokens() {
    let claims = json!({
        "sub": "workos-user-1",
        "exp": Utc::now().timestamp() + 3600,
    });
    let token = jsonwebtoken::encode(
        &Header {
            alg: Algorithm::HS256,
            kid: Some("workos-key".to_string()),
            ..Header::default()
        },
        &claims,
        &EncodingKey::from_secret(signing_secret()),
    )
    .expect("jwt");

    let provider = MastraAuthWorkos::new(
        MastraAuthWorkosOptions::default()
            .with_jwks(JwksSource::Inline(inline_hs256_jwks()))
            .with_algorithms(vec![Algorithm::HS256])
            .with_leeway_seconds(0),
        StubSessionClient,
    )
    .expect("provider");

    let identity = provider
        .authenticate(&AuthRequestParts::default().with_authorization(format!("Bearer {token}")))
        .await
        .expect("auth ok")
        .expect("identity");

    assert_eq!(identity.provider, "workos");
    assert_eq!(identity.subject, "workos-user-1");
}
