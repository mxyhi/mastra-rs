use async_trait::async_trait;
use mastra_auth_supabase::{MastraAuthSupabase, MastraAuthSupabaseOptions, SupabaseAuthClient};
use mastra_packages_auth::{AuthError, AuthIdentity, AuthRequestParts, Authenticator, Claims};

#[derive(Default)]
struct StubSupabaseClient;

#[async_trait]
impl SupabaseAuthClient for StubSupabaseClient {
    async fn get_user(&self, access_token: &str) -> Result<Option<AuthIdentity>, AuthError> {
        Ok(Some(AuthIdentity {
            provider: "ignored".to_string(),
            subject: format!("supabase:{access_token}"),
            issuer: None,
            audiences: Vec::new(),
            expires_at: None,
            issued_at: None,
            not_before: None,
            session_id: Some(access_token.to_string()),
            claims: Claims::new(),
        }))
    }
}

#[tokio::test]
async fn supabase_uses_bearer_token_by_default() {
    let provider =
        MastraAuthSupabase::new(MastraAuthSupabaseOptions::default(), StubSupabaseClient);

    let identity = provider
        .authenticate(&AuthRequestParts::default().with_authorization("Bearer access-token"))
        .await
        .expect("auth ok")
        .expect("identity");

    assert_eq!(identity.provider, "supabase");
    assert_eq!(identity.subject, "supabase:access-token");
}

#[tokio::test]
async fn supabase_can_read_access_token_from_cookie_when_configured() {
    let provider = MastraAuthSupabase::new(
        MastraAuthSupabaseOptions::default().with_access_token_cookie("sb-access-token"),
        StubSupabaseClient,
    );

    let identity = provider
        .authenticate(
            &AuthRequestParts::default().with_cookie_header("sb-access-token=cookie-token"),
        )
        .await
        .expect("auth ok")
        .expect("identity");

    assert_eq!(provider.access_token_cookie(), Some("sb-access-token"));
    assert_eq!(identity.session_id.as_deref(), Some("cookie-token"));
}
