use std::sync::Arc;

use async_trait::async_trait;
use mastra_packages_auth::{
    AuthError, AuthIdentity, AuthRequestParts, Authenticator, CallbackAdapter, CallbackHandler,
    CallbackResult, CallbackSessionAuthenticator, SessionResolver,
};

#[async_trait]
pub trait CloudTokenVerifier: Send + Sync {
    async fn verify_token(&self, token: &str) -> Result<Option<AuthIdentity>, AuthError>;
}

#[async_trait]
pub trait CloudCallbackClient: Send + Sync {
    async fn exchange_callback(
        &self,
        code: &str,
        state: &str,
    ) -> Result<Option<CallbackResult>, AuthError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MastraCloudAuthProviderOptions {
    session_cookie_name: String,
    allow_bearer_fallback: bool,
}

impl Default for MastraCloudAuthProviderOptions {
    fn default() -> Self {
        Self {
            session_cookie_name: "mastra_cloud_session".to_string(),
            allow_bearer_fallback: true,
        }
    }
}

impl MastraCloudAuthProviderOptions {
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
struct CloudSessionResolver<V> {
    session_cookie_name: String,
    allow_bearer_fallback: bool,
    verifier: Arc<V>,
}

#[async_trait]
impl<V> SessionResolver for CloudSessionResolver<V>
where
    V: CloudTokenVerifier,
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
struct CloudCallbackAdapterImpl<C> {
    client: Arc<C>,
}

#[async_trait]
impl<C> CallbackAdapter for CloudCallbackAdapterImpl<C>
where
    C: CloudCallbackClient,
{
    async fn handle_callback(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<CallbackResult>, AuthError> {
        let code = request
            .callback_code()
            .ok_or(AuthError::MissingCallbackCode)?;
        let state = request
            .callback_state()
            .ok_or(AuthError::MissingCallbackState)?;
        self.client.exchange_callback(code, state).await
    }
}

#[derive(Clone)]
pub struct MastraCloudAuthProvider<V, C> {
    session_cookie_name: String,
    inner: CallbackSessionAuthenticator<CloudSessionResolver<V>, CloudCallbackAdapterImpl<C>>,
}

impl<V, C> MastraCloudAuthProvider<V, C>
where
    V: CloudTokenVerifier,
    C: CloudCallbackClient,
{
    pub fn new(options: MastraCloudAuthProviderOptions, verifier: V, callback_client: C) -> Self {
        let session_cookie_name = options.session_cookie_name.clone();
        Self {
            session_cookie_name,
            inner: CallbackSessionAuthenticator::new(
                "cloud",
                CloudSessionResolver {
                    session_cookie_name: options.session_cookie_name,
                    allow_bearer_fallback: options.allow_bearer_fallback,
                    verifier: Arc::new(verifier),
                },
                CloudCallbackAdapterImpl {
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
impl<V, C> Authenticator for MastraCloudAuthProvider<V, C>
where
    V: CloudTokenVerifier,
    C: CloudCallbackClient,
{
    async fn authenticate(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<AuthIdentity>, AuthError> {
        self.inner.authenticate(request).await
    }
}

#[async_trait]
impl<V, C> CallbackHandler for MastraCloudAuthProvider<V, C>
where
    V: CloudTokenVerifier,
    C: CloudCallbackClient,
{
    async fn handle_callback(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<CallbackResult>, AuthError> {
        self.inner.handle_callback(request).await
    }
}
