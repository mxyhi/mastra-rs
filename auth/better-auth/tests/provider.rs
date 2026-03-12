use std::sync::Mutex;

use async_trait::async_trait;
use mastra_auth_better_auth::{
    BetterAuthSessionClient, MastraAuthBetterAuth, MastraAuthBetterAuthOptions,
};
use mastra_packages_auth::{AuthError, AuthIdentity, AuthRequestParts, Authenticator, Claims};

#[derive(Default)]
struct RecordingClient {
    seen_tokens: Mutex<Vec<String>>,
}

#[async_trait]
impl BetterAuthSessionClient for RecordingClient {
    async fn resolve_session_token(
        &self,
        session_token: &str,
    ) -> Result<Option<AuthIdentity>, AuthError> {
        self.seen_tokens
            .lock()
            .expect("lock")
            .push(session_token.to_string());

        Ok(Some(AuthIdentity {
            provider: "ignored".to_string(),
            subject: format!("session:{session_token}"),
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
async fn better_auth_prefers_session_cookie_over_bearer_token() {
    let client = RecordingClient::default();
    let provider = MastraAuthBetterAuth::new(MastraAuthBetterAuthOptions::default(), client);

    let request = AuthRequestParts::default()
        .with_cookie_header("better-auth.session_token=cookie-token")
        .with_authorization("Bearer bearer-token");
    let identity = provider
        .authenticate(&request)
        .await
        .expect("auth ok")
        .expect("identity");

    assert_eq!(provider.session_cookie_name(), "better-auth.session_token");
    assert_eq!(identity.provider, "better-auth");
    assert_eq!(identity.subject, "session:cookie-token");
}

#[tokio::test]
async fn better_auth_uses_custom_cookie_prefix_and_bearer_fallback() {
    let client = RecordingClient::default();
    let provider = MastraAuthBetterAuth::new(
        MastraAuthBetterAuthOptions::default().with_cookie_prefix("myapp"),
        client,
    );

    let identity = provider
        .authenticate(&AuthRequestParts::default().with_authorization("Bearer bearer-token"))
        .await
        .expect("auth ok")
        .expect("identity");

    assert_eq!(provider.session_cookie_name(), "myapp.session_token");
    assert_eq!(identity.session_id.as_deref(), Some("bearer-token"));
}
