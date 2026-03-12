use async_trait::async_trait;

use crate::{AuthError, AuthIdentity, AuthRequestParts, CallbackResult};

#[async_trait]
pub trait Authenticator: Send + Sync {
    async fn authenticate(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<AuthIdentity>, AuthError>;
}

#[async_trait]
pub trait SessionResolver: Send + Sync {
    async fn resolve_session(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<AuthIdentity>, AuthError>;
}

#[async_trait]
pub trait CallbackAdapter: Send + Sync {
    async fn handle_callback(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<CallbackResult>, AuthError>;
}

#[async_trait]
pub trait CallbackHandler: Send + Sync {
    async fn handle_callback(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<CallbackResult>, AuthError>;
}

#[derive(Debug, Clone)]
pub struct SessionBackedAuthenticator<R> {
    provider: String,
    resolver: R,
}

impl<R> SessionBackedAuthenticator<R> {
    pub fn new(provider: impl Into<String>, resolver: R) -> Self {
        Self {
            provider: provider.into(),
            resolver,
        }
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }
}

#[async_trait]
impl<R> Authenticator for SessionBackedAuthenticator<R>
where
    R: SessionResolver,
{
    async fn authenticate(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<AuthIdentity>, AuthError> {
        self.resolver
            .resolve_session(request)
            .await
            .map(|identity| {
                identity.map(|mut identity| {
                    identity.provider = self.provider.clone();
                    identity
                })
            })
    }
}

#[derive(Debug, Clone)]
pub struct CallbackSessionAuthenticator<R, A> {
    provider: String,
    session_resolver: R,
    callback_adapter: A,
}

impl<R, A> CallbackSessionAuthenticator<R, A> {
    pub fn new(provider: impl Into<String>, session_resolver: R, callback_adapter: A) -> Self {
        Self {
            provider: provider.into(),
            session_resolver,
            callback_adapter,
        }
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }
}

#[async_trait]
impl<R, A> Authenticator for CallbackSessionAuthenticator<R, A>
where
    R: SessionResolver,
    A: CallbackAdapter,
{
    async fn authenticate(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<AuthIdentity>, AuthError> {
        self.session_resolver
            .resolve_session(request)
            .await
            .map(|identity| {
                identity.map(|mut identity| {
                    identity.provider = self.provider.clone();
                    identity
                })
            })
    }
}

#[async_trait]
impl<R, A> CallbackHandler for CallbackSessionAuthenticator<R, A>
where
    R: SessionResolver,
    A: CallbackAdapter,
{
    async fn handle_callback(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<CallbackResult>, AuthError> {
        self.callback_adapter
            .handle_callback(request)
            .await
            .map(|result| {
                result.map(|mut result| {
                    result.identity.provider = self.provider.clone();
                    result
                })
            })
    }
}
