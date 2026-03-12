use async_trait::async_trait;
use jsonwebtoken::Algorithm;
use mastra_packages_auth::{
    AuthError, AuthIdentity, AuthRequestParts, Authenticator, JwksSource, OidcConfiguration,
    OidcJwtAuthenticator,
};

#[derive(Clone, Debug)]
pub struct MastraAuthClerkOptions {
    issuer: Option<String>,
    jwks_uri: Option<String>,
    audiences: Vec<String>,
    jwks: Option<JwksSource>,
    algorithms: Vec<Algorithm>,
    leeway_seconds: u64,
}

impl Default for MastraAuthClerkOptions {
    fn default() -> Self {
        Self {
            issuer: std::env::var("CLERK_ISSUER").ok(),
            jwks_uri: std::env::var("CLERK_JWKS_URI").ok(),
            audiences: Vec::new(),
            jwks: None,
            algorithms: vec![Algorithm::RS256],
            leeway_seconds: 60,
        }
    }
}

impl MastraAuthClerkOptions {
    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = Some(issuer.into());
        self
    }

    pub fn with_jwks_uri(mut self, jwks_uri: impl Into<String>) -> Self {
        self.jwks_uri = Some(jwks_uri.into());
        self
    }

    pub fn with_audience(mut self, audience: impl Into<String>) -> Self {
        self.audiences.push(audience.into());
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

#[derive(Clone, Debug)]
pub struct MastraAuthClerk {
    jwks_uri: String,
    inner: OidcJwtAuthenticator,
}

impl MastraAuthClerk {
    pub fn new(options: MastraAuthClerkOptions) -> Result<Self, String> {
        let jwks_uri = options
            .jwks_uri
            .clone()
            .filter(|value| !value.is_empty())
            .or_else(|| match &options.jwks {
                Some(JwksSource::Remote(uri)) => Some(uri.clone()),
                Some(JwksSource::Inline(_)) => Some("inline://jwks".to_string()),
                None => None,
            })
            .ok_or_else(|| "Clerk JWKS URI or inline JWKS is required".to_string())?;
        let inner = OidcJwtAuthenticator::new(
            OidcConfiguration::new(
                "clerk",
                options.issuer.unwrap_or_default(),
                options.audiences,
                options
                    .jwks
                    .unwrap_or_else(|| JwksSource::Remote(jwks_uri.clone())),
            )
            .with_algorithms(options.algorithms)
            .with_leeway_seconds(options.leeway_seconds),
        );

        Ok(Self { jwks_uri, inner })
    }

    pub fn jwks_uri(&self) -> &str {
        &self.jwks_uri
    }
}

#[async_trait]
impl Authenticator for MastraAuthClerk {
    async fn authenticate(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<AuthIdentity>, AuthError> {
        self.inner.authenticate(request).await
    }
}
