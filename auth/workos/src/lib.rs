use std::sync::Arc;

use async_trait::async_trait;
use jsonwebtoken::Algorithm;
use mastra_packages_auth::{
    AuthError, AuthIdentity, AuthRequestParts, Authenticator, JwksSource, OidcConfiguration,
    OidcJwtAuthenticator, SessionBackedAuthenticator, SessionResolver,
};

#[async_trait]
pub trait WorkosSessionClient: Send + Sync {
    async fn resolve_session(&self, session_token: &str)
    -> Result<Option<AuthIdentity>, AuthError>;
}

#[derive(Clone, Debug)]
pub struct MastraAuthWorkosOptions {
    api_key: Option<String>,
    client_id: Option<String>,
    issuer: Option<String>,
    jwks_uri: Option<String>,
    session_cookie_name: String,
    allow_bearer_fallback: bool,
    jwks: Option<JwksSource>,
    algorithms: Vec<Algorithm>,
    leeway_seconds: u64,
}

impl Default for MastraAuthWorkosOptions {
    fn default() -> Self {
        Self {
            api_key: std::env::var("WORKOS_API_KEY").ok(),
            client_id: std::env::var("WORKOS_CLIENT_ID").ok(),
            issuer: std::env::var("WORKOS_ISSUER").ok(),
            jwks_uri: std::env::var("WORKOS_JWKS_URI").ok(),
            session_cookie_name: "wos-session".to_string(),
            allow_bearer_fallback: true,
            jwks: None,
            algorithms: vec![Algorithm::RS256],
            leeway_seconds: 60,
        }
    }
}

impl MastraAuthWorkosOptions {
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = Some(client_id.into());
        self
    }

    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = Some(issuer.into());
        self
    }

    pub fn with_jwks_uri(mut self, jwks_uri: impl Into<String>) -> Self {
        self.jwks_uri = Some(jwks_uri.into());
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

    pub fn with_jwks(mut self, jwks: JwksSource) -> Self {
        self.jwks = Some(jwks);
        self
    }

    pub fn with_algorithms(mut self, algorithms: Vec<Algorithm>) -> Self {
        self.algorithms = algorithms;
        self
    }

    pub fn with_leeway_seconds(mut self, leeway_seconds: u64) -> Self {
        self.leeway_seconds = leeway_seconds;
        self
    }
}

#[derive(Clone)]
struct WorkosResolver<C> {
    session_cookie_name: String,
    client: Arc<C>,
}

#[async_trait]
impl<C> SessionResolver for WorkosResolver<C>
where
    C: WorkosSessionClient,
{
    async fn resolve_session(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<AuthIdentity>, AuthError> {
        match request.cookie(&self.session_cookie_name) {
            Some(token) => self.client.resolve_session(token).await,
            None => Ok(None),
        }
    }
}

#[derive(Clone)]
pub struct MastraAuthWorkos<C> {
    session_cookie_name: String,
    #[allow(dead_code)]
    api_key: Option<String>,
    #[allow(dead_code)]
    client_id: Option<String>,
    session: SessionBackedAuthenticator<WorkosResolver<C>>,
    bearer: OidcJwtAuthenticator,
    allow_bearer_fallback: bool,
}

impl<C> MastraAuthWorkos<C>
where
    C: WorkosSessionClient,
{
    pub fn new(options: MastraAuthWorkosOptions, client: C) -> Result<Self, String> {
        let client_id = options.client_id.clone();
        let jwks = options
            .jwks
            .clone()
            .or_else(|| options.jwks_uri.clone().map(JwksSource::Remote))
            .or_else(|| {
                client_id.as_ref().map(|client_id| {
                    JwksSource::Remote(format!("https://api.workos.com/sso/jwks/{client_id}"))
                })
            })
            .ok_or_else(|| "WorkOS JWKS configuration is required".to_string())?;
        let audiences = client_id.clone().into_iter().collect::<Vec<_>>();

        Ok(Self {
            session_cookie_name: options.session_cookie_name.clone(),
            api_key: options.api_key,
            client_id,
            session: SessionBackedAuthenticator::new(
                "workos",
                WorkosResolver {
                    session_cookie_name: options.session_cookie_name,
                    client: Arc::new(client),
                },
            ),
            bearer: OidcJwtAuthenticator::new(
                OidcConfiguration::new(
                    "workos",
                    options.issuer.unwrap_or_default(),
                    audiences,
                    jwks,
                )
                .with_algorithms(options.algorithms)
                .with_leeway_seconds(options.leeway_seconds),
            ),
            allow_bearer_fallback: options.allow_bearer_fallback,
        })
    }

    pub fn session_cookie_name(&self) -> &str {
        &self.session_cookie_name
    }
}

#[async_trait]
impl<C> Authenticator for MastraAuthWorkos<C>
where
    C: WorkosSessionClient,
{
    async fn authenticate(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<AuthIdentity>, AuthError> {
        if let Some(identity) = self.session.authenticate(request).await? {
            return Ok(Some(identity));
        }

        if self.allow_bearer_fallback {
            return self.bearer.authenticate(request).await;
        }

        Ok(None)
    }
}
