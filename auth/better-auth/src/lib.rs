use std::sync::Arc;

use async_trait::async_trait;
use mastra_packages_auth::{
    AuthError, AuthIdentity, AuthRequestParts, Authenticator, SessionBackedAuthenticator,
    SessionResolver,
};

#[async_trait]
pub trait BetterAuthSessionClient: Send + Sync {
    async fn resolve_session_token(
        &self,
        session_token: &str,
    ) -> Result<Option<AuthIdentity>, AuthError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MastraAuthBetterAuthOptions {
    cookie_prefix: String,
    allow_bearer_fallback: bool,
}

impl Default for MastraAuthBetterAuthOptions {
    fn default() -> Self {
        Self {
            cookie_prefix: "better-auth".to_string(),
            allow_bearer_fallback: true,
        }
    }
}

impl MastraAuthBetterAuthOptions {
    pub fn with_cookie_prefix(mut self, cookie_prefix: impl Into<String>) -> Self {
        self.cookie_prefix = cookie_prefix.into();
        self
    }

    pub fn with_allow_bearer_fallback(mut self, allow_bearer_fallback: bool) -> Self {
        self.allow_bearer_fallback = allow_bearer_fallback;
        self
    }

    fn session_cookie_name(&self) -> String {
        format!("{}.session_token", self.cookie_prefix)
    }
}

#[derive(Clone)]
struct BetterAuthSessionResolver<C> {
    session_cookie_name: String,
    allow_bearer_fallback: bool,
    client: Arc<C>,
}

#[async_trait]
impl<C> SessionResolver for BetterAuthSessionResolver<C>
where
    C: BetterAuthSessionClient,
{
    async fn resolve_session(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<AuthIdentity>, AuthError> {
        let token = request
            .cookie(&self.session_cookie_name)
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
            Some(token) => self.client.resolve_session_token(&token).await,
            None => Ok(None),
        }
    }
}

#[derive(Clone)]
pub struct MastraAuthBetterAuth<C> {
    session_cookie_name: String,
    inner: SessionBackedAuthenticator<BetterAuthSessionResolver<C>>,
}

impl<C> MastraAuthBetterAuth<C>
where
    C: BetterAuthSessionClient,
{
    pub fn new(options: MastraAuthBetterAuthOptions, client: C) -> Self {
        let session_cookie_name = options.session_cookie_name();
        let resolver = BetterAuthSessionResolver {
            session_cookie_name: session_cookie_name.clone(),
            allow_bearer_fallback: options.allow_bearer_fallback,
            client: Arc::new(client),
        };

        Self {
            session_cookie_name,
            inner: SessionBackedAuthenticator::new("better-auth", resolver),
        }
    }

    pub fn session_cookie_name(&self) -> &str {
        &self.session_cookie_name
    }
}

#[async_trait]
impl<C> Authenticator for MastraAuthBetterAuth<C>
where
    C: BetterAuthSessionClient,
{
    async fn authenticate(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<AuthIdentity>, AuthError> {
        self.inner.authenticate(request).await
    }
}
