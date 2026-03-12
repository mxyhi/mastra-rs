use async_trait::async_trait;
use jsonwebtoken::Algorithm;
use mastra_packages_auth::{
    AuthError, AuthIdentity, AuthRequestParts, Authenticator, JwksSource, OidcConfiguration,
    OidcJwtAuthenticator,
};

const FIREBASE_JWKS_URI: &str =
    "https://www.googleapis.com/service_accounts/v1/jwk/securetoken@system.gserviceaccount.com";

#[derive(Clone, Debug)]
pub struct MastraAuthFirebaseOptions {
    project_id: Option<String>,
    database_id: Option<String>,
    service_account_path: Option<String>,
    jwks: Option<JwksSource>,
    algorithms: Vec<Algorithm>,
    leeway_seconds: u64,
}

impl Default for MastraAuthFirebaseOptions {
    fn default() -> Self {
        Self {
            project_id: std::env::var("FIREBASE_PROJECT_ID").ok(),
            database_id: std::env::var("FIRESTORE_DATABASE_ID")
                .ok()
                .or_else(|| std::env::var("FIREBASE_DATABASE_ID").ok()),
            service_account_path: std::env::var("FIREBASE_SERVICE_ACCOUNT").ok(),
            jwks: None,
            algorithms: vec![Algorithm::RS256],
            leeway_seconds: 60,
        }
    }
}

impl MastraAuthFirebaseOptions {
    pub fn with_project_id(mut self, project_id: impl Into<String>) -> Self {
        self.project_id = Some(project_id.into());
        self
    }

    pub fn with_database_id(mut self, database_id: impl Into<String>) -> Self {
        self.database_id = Some(database_id.into());
        self
    }

    pub fn with_service_account_path(mut self, service_account_path: impl Into<String>) -> Self {
        self.service_account_path = Some(service_account_path.into());
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
pub struct MastraAuthFirebase {
    project_id: String,
    database_id: Option<String>,
    service_account_path: Option<String>,
    inner: OidcJwtAuthenticator,
}

impl MastraAuthFirebase {
    pub fn new(options: MastraAuthFirebaseOptions) -> Result<Self, String> {
        let project_id = options
            .project_id
            .clone()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                "Firebase project ID is required via options.project_id or FIREBASE_PROJECT_ID"
                    .to_string()
            })?;
        let inner = OidcJwtAuthenticator::new(
            OidcConfiguration::new(
                "firebase",
                format!("https://securetoken.google.com/{project_id}"),
                vec![project_id.clone()],
                options
                    .jwks
                    .unwrap_or_else(|| JwksSource::Remote(FIREBASE_JWKS_URI.to_string())),
            )
            .with_algorithms(options.algorithms)
            .with_leeway_seconds(options.leeway_seconds),
        );

        Ok(Self {
            project_id,
            database_id: options.database_id,
            service_account_path: options.service_account_path,
            inner,
        })
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn database_id(&self) -> Option<&str> {
        self.database_id.as_deref()
    }

    pub fn service_account_path(&self) -> Option<&str> {
        self.service_account_path.as_deref()
    }
}

#[async_trait]
impl Authenticator for MastraAuthFirebase {
    async fn authenticate(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<AuthIdentity>, AuthError> {
        self.inner.authenticate(request).await
    }
}
