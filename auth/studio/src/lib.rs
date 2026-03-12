use std::sync::Arc;

use async_trait::async_trait;
use mastra_packages_auth::{
    AuthError, AuthIdentity, AuthRequestParts, Authenticator, CallbackAdapter, CallbackHandler,
    CallbackResult, CallbackSessionAuthenticator, SessionResolver,
};

#[async_trait]
pub trait StudioTokenVerifier: Send + Sync {
    async fn verify_token(&self, token: &str) -> Result<Option<AuthIdentity>, AuthError>;
}

#[async_trait]
pub trait StudioCallbackClient: Send + Sync {
    async fn exchange_callback(
        &self,
        code: &str,
        state: Option<&str>,
    ) -> Result<Option<CallbackResult>, AuthError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MastraAuthStudioOptions {
    shared_api_url: Option<String>,
    organization_id: Option<String>,
    session_cookie_name: String,
    allow_bearer_fallback: bool,
}

impl Default for MastraAuthStudioOptions {
    fn default() -> Self {
        Self {
            shared_api_url: std::env::var("MASTRA_SHARED_API_URL")
                .ok()
                .or_else(|| std::env::var("MASTRA_STUDIO_API_URL").ok()),
            organization_id: std::env::var("MASTRA_ORGANIZATION_ID").ok(),
            session_cookie_name: "wos-session".to_string(),
            allow_bearer_fallback: true,
        }
    }
}

impl MastraAuthStudioOptions {
    pub fn with_shared_api_url(mut self, shared_api_url: impl Into<String>) -> Self {
        self.shared_api_url = Some(shared_api_url.into());
        self
    }

    pub fn with_organization_id(mut self, organization_id: impl Into<String>) -> Self {
        self.organization_id = Some(organization_id.into());
        self
    }

    pub fn with_session_cookie_name(mut self, session_cookie_name: impl Into<String>) -> Self {
        self.session_cookie_name = session_cookie_name.into();
        self
    }

    pub fn with_allow_bearer_fallback(mut self, allow_bearer_fallback: bool) -> Self {
        self.allow_bearer_fallback = allow_bearer_fallback;
        self
    }
}

#[derive(Clone)]
struct StudioSessionResolver<V> {
    session_cookie_name: String,
    allow_bearer_fallback: bool,
    verifier: Arc<V>,
}

#[async_trait]
impl<V> SessionResolver for StudioSessionResolver<V>
where
    V: StudioTokenVerifier,
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
            Some(token) => self.verifier.verify_token(&token).await,
            None => Ok(None),
        }
    }
}

#[derive(Clone)]
struct StudioCallbackAdapterImpl<C> {
    client: Arc<C>,
}

#[async_trait]
impl<C> CallbackAdapter for StudioCallbackAdapterImpl<C>
where
    C: StudioCallbackClient,
{
    async fn handle_callback(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<CallbackResult>, AuthError> {
        let code = request
            .callback_code()
            .ok_or(AuthError::MissingCallbackCode)?;
        self.client
            .exchange_callback(code, request.callback_state())
            .await
    }
}

#[derive(Clone)]
pub struct MastraAuthStudio<V, C> {
    session_cookie_name: String,
    #[allow(dead_code)]
    shared_api_url: Option<String>,
    #[allow(dead_code)]
    organization_id: Option<String>,
    inner: CallbackSessionAuthenticator<StudioSessionResolver<V>, StudioCallbackAdapterImpl<C>>,
}

impl<V, C> MastraAuthStudio<V, C>
where
    V: StudioTokenVerifier,
    C: StudioCallbackClient,
{
    pub fn new(options: MastraAuthStudioOptions, verifier: V, callback_client: C) -> Self {
        let session_cookie_name = options.session_cookie_name.clone();
        Self {
            session_cookie_name,
            shared_api_url: options.shared_api_url,
            organization_id: options.organization_id,
            inner: CallbackSessionAuthenticator::new(
                "studio",
                StudioSessionResolver {
                    session_cookie_name: options.session_cookie_name,
                    allow_bearer_fallback: options.allow_bearer_fallback,
                    verifier: Arc::new(verifier),
                },
                StudioCallbackAdapterImpl {
                    client: Arc::new(callback_client),
                },
            ),
        }
    }

    pub fn session_cookie_name(&self) -> &str {
        &self.session_cookie_name
    }
}

#[async_trait]
impl<V, C> Authenticator for MastraAuthStudio<V, C>
where
    V: StudioTokenVerifier,
    C: StudioCallbackClient,
{
    async fn authenticate(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<AuthIdentity>, AuthError> {
        self.inner.authenticate(request).await
    }
}

#[async_trait]
impl<V, C> CallbackHandler for MastraAuthStudio<V, C>
where
    V: StudioTokenVerifier,
    C: StudioCallbackClient,
{
    async fn handle_callback(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<CallbackResult>, AuthError> {
        self.inner.handle_callback(request).await
    }
}
