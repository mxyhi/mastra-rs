use async_trait::async_trait;
use jsonwebtoken::Algorithm;
use mastra_packages_auth::{
    AuthError, AuthIdentity, AuthRequestParts, Authenticator, JwksSource, OidcConfiguration,
    OidcJwtAuthenticator,
};

const AUTH0_JWKS_SUFFIX: &str = ".well-known/jwks.json";

#[derive(Clone, Debug)]
pub struct MastraAuthAuth0Options {
    domain: Option<String>,
    audience: Option<String>,
    jwks: Option<JwksSource>,
    algorithms: Vec<Algorithm>,
    leeway_seconds: u64,
}

impl Default for MastraAuthAuth0Options {
    fn default() -> Self {
        Self {
            domain: std::env::var("AUTH0_DOMAIN").ok(),
            audience: std::env::var("AUTH0_AUDIENCE").ok(),
            jwks: None,
            algorithms: vec![Algorithm::RS256],
            leeway_seconds: 60,
        }
    }
}

impl MastraAuthAuth0Options {
    pub fn with_domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    pub fn with_audience(mut self, audience: impl Into<String>) -> Self {
        self.audience = Some(audience.into());
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
pub struct MastraAuthAuth0 {
    domain: String,
    audience: String,
    inner: OidcJwtAuthenticator,
}

impl MastraAuthAuth0 {
    pub fn new(options: MastraAuthAuth0Options) -> Result<Self, String> {
        let domain = options
            .domain
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                "AUTH0 domain is required via options.domain or AUTH0_DOMAIN".to_string()
            })?;
        let audience = options
            .audience
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                "AUTH0 audience is required via options.audience or AUTH0_AUDIENCE".to_string()
            })?;
        let issuer = format!("https://{}/", domain.trim_end_matches('/'));
        let jwks_uri = format!("{issuer}{AUTH0_JWKS_SUFFIX}");
        let inner = OidcJwtAuthenticator::new(
            OidcConfiguration::new(
                "auth0",
                issuer,
                vec![audience.clone()],
                options
                    .jwks
                    .unwrap_or_else(|| JwksSource::Remote(jwks_uri.clone())),
            )
            .with_algorithms(options.algorithms)
            .with_leeway_seconds(options.leeway_seconds),
        );

        Ok(Self {
            domain,
            audience,
            inner,
        })
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub fn audience(&self) -> &str {
        &self.audience
    }

    pub fn jwks_uri(&self) -> String {
        format!("https://{}/{AUTH0_JWKS_SUFFIX}", self.domain.trim_end_matches('/'))
    }
}

#[async_trait]
impl Authenticator for MastraAuthAuth0 {
    async fn authenticate(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<AuthIdentity>, AuthError> {
        self.inner.authenticate(request).await
    }
}
