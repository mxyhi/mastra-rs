use async_trait::async_trait;
use mastra_auth_cloud::{
    CloudCallbackClient, CloudTokenVerifier, MastraCloudAuthProvider,
    MastraCloudAuthProviderOptions,
};
use mastra_packages_auth::{
    AuthError, AuthIdentity, AuthRequestParts, Authenticator, CallbackHandler, CallbackResult,
    Claims,
};

#[derive(Default)]
struct StubVerifier;

#[async_trait]
impl CloudTokenVerifier for StubVerifier {
    async fn verify_token(&self, token: &str) -> Result<Option<AuthIdentity>, AuthError> {
        Ok(Some(AuthIdentity {
            provider: "ignored".to_string(),
            subject: format!("cloud:{token}"),
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
impl CloudCallbackClient for StubCallbackClient {
    async fn exchange_callback(
        &self,
        code: &str,
        state: &str,
    ) -> Result<Option<CallbackResult>, AuthError> {
        Ok(Some(CallbackResult {
            identity: AuthIdentity {
                provider: "ignored".to_string(),
                subject: format!("callback:{code}:{state}"),
                issuer: None,
                audiences: Vec::new(),
                expires_at: None,
                issued_at: None,
                not_before: None,
                session_id: Some("session-1".to_string()),
                claims: Claims::new(),
            },
            access_token: "access-1".to_string(),
            refresh_token: None,
            id_token: None,
            set_cookie_headers: vec!["mastra_cloud_session=access-1".to_string()],
        }))
    }
}

#[tokio::test]
async fn cloud_auth_uses_session_cookie_before_bearer_token() {
    let provider = MastraCloudAuthProvider::new(
        MastraCloudAuthProviderOptions::default(),
        StubVerifier,
        StubCallbackClient,
    );

    let request = AuthRequestParts::default()
        .with_cookie_header("mastra_cloud_session=session-token")
        .with_authorization("Bearer bearer-token");
    let identity = provider
        .authenticate(&request)
        .await
        .expect("auth ok")
        .expect("identity");

    assert_eq!(provider.session_cookie_name(), "mastra_cloud_session");
    assert_eq!(identity.provider, "cloud");
    assert_eq!(identity.subject, "cloud:session-token");
}

#[tokio::test]
async fn cloud_auth_exchanges_callback_code_and_state() {
    let provider = MastraCloudAuthProvider::new(
        MastraCloudAuthProviderOptions::default(),
        StubVerifier,
        StubCallbackClient,
    );

    let result = provider
        .handle_callback(&AuthRequestParts::default().with_callback("code-1", "state-1"))
        .await
        .expect("callback ok")
        .expect("callback result");

    assert_eq!(result.identity.provider, "cloud");
    assert_eq!(result.identity.subject, "callback:code-1:state-1");
    assert_eq!(result.access_token, "access-1");
}
