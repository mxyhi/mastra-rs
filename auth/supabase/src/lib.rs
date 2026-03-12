use std::sync::Arc;

use async_trait::async_trait;
use mastra_packages_auth::{
    AuthError, AuthIdentity, AuthRequestParts, Authenticator, SessionBackedAuthenticator,
    SessionResolver,
};

#[async_trait]
pub trait SupabaseAuthClient: Send + Sync {
    async fn get_user(&self, access_token: &str) -> Result<Option<AuthIdentity>, AuthError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MastraAuthSupabaseOptions {
    url: Option<String>,
    anon_key: Option<String>,
    access_token_cookie: Option<String>,
    allow_bearer_fallback: bool,
}

impl Default for MastraAuthSupabaseOptions {
    fn default() -> Self {
        Self {
            url: std::env::var("SUPABASE_URL").ok(),
            anon_key: std::env::var("SUPABASE_ANON_KEY").ok(),
            access_token_cookie: None,
            allow_bearer_fallback: true,
        }
    }
}

impl MastraAuthSupabaseOptions {
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn with_anon_key(mut self, anon_key: impl Into<String>) -> Self {
        self.anon_key = Some(anon_key.into());
        self
    }

    pub fn with_access_token_cookie(mut self, access_token_cookie: impl Into<String>) -> Self {
        self.access_token_cookie = Some(access_token_cookie.into());
        self
    }

    pub fn with_allow_bearer_fallback(mut self, allow_bearer_fallback: bool) -> Self {
        self.allow_bearer_fallback = allow_bearer_fallback;
        self
    }
}

#[derive(Clone)]
struct SupabaseSessionResolver<C> {
    access_token_cookie: Option<String>,
    allow_bearer_fallback: bool,
    client: Arc<C>,
}

#[async_trait]
impl<C> SessionResolver for SupabaseSessionResolver<C>
where
    C: SupabaseAuthClient,
{
    async fn resolve_session(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<AuthIdentity>, AuthError> {
        let token = self
            .access_token_cookie
            .as_deref()
            .and_then(|cookie_name| request.cookie(cookie_name))
            .map(ToString::to_string)
            .or_else(|| {
                self.allow_bearer_fallback
                    .then(|| request.bearer_token())
                    .transpose()
                    .ok()
                    .flatten()
                    .flatten()
                    .map(ToString::to_string)
            });

        match token {
            Some(token) => self.client.get_user(&token).await,
            None => Ok(None),
        }
    }
}

#[derive(Clone)]
pub struct MastraAuthSupabase<C> {
    access_token_cookie: Option<String>,
    #[allow(dead_code)]
    url: Option<String>,
    #[allow(dead_code)]
    anon_key: Option<String>,
    inner: SessionBackedAuthenticator<SupabaseSessionResolver<C>>,
}

impl<C> MastraAuthSupabase<C>
where
    C: SupabaseAuthClient,
{
    pub fn new(options: MastraAuthSupabaseOptions, client: C) -> Self {
        let access_token_cookie = options.access_token_cookie.clone();
        Self {
            access_token_cookie: access_token_cookie.clone(),
            url: options.url,
            anon_key: options.anon_key,
            inner: SessionBackedAuthenticator::new(
                "supabase",
                SupabaseSessionResolver {
                    access_token_cookie,
                    allow_bearer_fallback: options.allow_bearer_fallback,
                    client: Arc::new(client),
                },
            ),
        }
    }

    pub fn access_token_cookie(&self) -> Option<&str> {
        self.access_token_cookie.as_deref()
    }
}

#[async_trait]
impl<C> Authenticator for MastraAuthSupabase<C>
where
    C: SupabaseAuthClient,
{
    async fn authenticate(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<AuthIdentity>, AuthError> {
        self.inner.authenticate(request).await
    }
}
