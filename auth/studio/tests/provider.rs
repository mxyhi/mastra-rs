use async_trait::async_trait;
use mastra_auth_studio::{
    MastraAuthStudio, MastraAuthStudioOptions, StudioCallbackClient, StudioTokenVerifier,
};
use mastra_packages_auth::{
    AuthError, AuthIdentity, AuthRequestParts, Authenticator, CallbackHandler, CallbackResult,
    Claims,
};

#[derive(Default)]
struct StubVerifier;

#[async_trait]
impl StudioTokenVerifier for StubVerifier {
    async fn verify_token(&self, token: &str) -> Result<Option<AuthIdentity>, AuthError> {
        Ok(Some(AuthIdentity {
            provider: "ignored".to_string(),
            subject: format!("studio:{token}"),
            issuer: None,
            audiences: Vec::new(),
            expires_at: None,
            issued_at: None,
            not_before: None,
            session_id: Some(token.to_string()),
            claims: Claims::new(),
        }))
    }
}

#[derive(Default)]
struct StubCallbackClient;

#[async_trait]
impl StudioCallbackClient for StubCallbackClient {
    async fn exchange_callback(
        &self,
        code: &str,
        state: Option<&str>,
    ) -> Result<Option<CallbackResult>, AuthError> {
        Ok(Some(CallbackResult {
            identity: AuthIdentity {
                provider: "ignored".to_string(),
                subject: format!("studio-callback:{code}:{}", state.unwrap_or("")),
                issuer: None,
                audiences: Vec::new(),
                expires_at: None,
                issued_at: None,
                not_before: None,
                session_id: Some("studio-session".to_string()),
                claims: Claims::new(),
            },
            access_token: code.to_string(),
            refresh_token: None,
            id_token: None,
            set_cookie_headers: vec!["wos-session=studio-session".to_string()],
        }))
    }
}

#[tokio::test]
async fn studio_auth_prefers_wos_session_cookie() {
    let provider = MastraAuthStudio::new(MastraAuthStudioOptions::default(), StubVerifier, StubCallbackClient);

    let request = AuthRequestParts::default()
        .with_cookie_header("wos-session=sealed-session")
        .with_authorization("Bearer api-token");
    let identity = provider
        .authenticate(&request)
        .await
        .expect("auth ok")
        .expect("identity");

    assert_eq!(provider.session_cookie_name(), "wos-session");
    assert_eq!(identity.provider, "studio");
    assert_eq!(identity.subject, "studio:sealed-session");
}

#[tokio::test]
async fn studio_auth_handles_callback_without_requiring_state_specific_logic() {
    let provider = MastraAuthStudio::new(MastraAuthStudioOptions::default(), StubVerifier, StubCallbackClient);

    let result = provider
        .handle_callback(&AuthRequestParts::default().with_callback("sealed-session", "ignored"))
        .await
        .expect("callback ok")
        .expect("callback result");

    assert_eq!(result.identity.provider, "studio");
    assert_eq!(result.access_token, "sealed-session");
}
