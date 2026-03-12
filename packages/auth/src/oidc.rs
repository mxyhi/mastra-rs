use std::collections::BTreeSet;

use async_trait::async_trait;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use reqwest::Client;
use serde_json::Value;

use crate::{AuthError, AuthIdentity, AuthRequestParts, Authenticator, Claims, timestamp_to_utc};

#[derive(Debug, Clone)]
pub enum JwksSource {
    Remote(String),
    Inline(JwkSet),
}

#[derive(Debug, Clone)]
pub struct OidcConfiguration {
    pub provider: String,
    pub issuer: String,
    pub audiences: Vec<String>,
    pub algorithms: Vec<Algorithm>,
    pub jwks: JwksSource,
    pub required_claims: BTreeSet<String>,
    pub leeway_seconds: u64,
}

impl OidcConfiguration {
    pub fn new(
        provider: impl Into<String>,
        issuer: impl Into<String>,
        audiences: Vec<String>,
        jwks: JwksSource,
    ) -> Self {
        let issuer = issuer.into();
        let mut required_claims = BTreeSet::from(["exp".to_string(), "sub".to_string()]);
        if !issuer.is_empty() {
            required_claims.insert("iss".to_string());
        }
        if !audiences.is_empty() {
            required_claims.insert("aud".to_string());
        }
        Self {
            provider: provider.into(),
            issuer,
            audiences,
            algorithms: vec![Algorithm::RS256],
            jwks,
            required_claims,
            leeway_seconds: 60,
        }
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

#[derive(Debug, Clone)]
pub struct OidcJwtAuthenticator {
    config: OidcConfiguration,
    http_client: Client,
}

impl OidcJwtAuthenticator {
    pub fn new(config: OidcConfiguration) -> Self {
        Self {
            config,
            http_client: Client::new(),
        }
    }

    pub fn provider(&self) -> &str {
        &self.config.provider
    }

    pub fn config(&self) -> &OidcConfiguration {
        &self.config
    }

    async fn load_jwk_set(&self) -> Result<JwkSet, AuthError> {
        match &self.config.jwks {
            JwksSource::Inline(jwk_set) => Ok(jwk_set.clone()),
            JwksSource::Remote(uri) => {
                let response = self
                    .http_client
                    .get(uri)
                    .send()
                    .await
                    .map_err(|error| AuthError::JwksFetch {
                        uri: uri.clone(),
                        message: error.to_string(),
                    })?
                    .error_for_status()
                    .map_err(|error| AuthError::JwksFetch {
                        uri: uri.clone(),
                        message: error.to_string(),
                    })?;

                response
                    .json::<JwkSet>()
                    .await
                    .map_err(|error| AuthError::JwksParse {
                        uri: uri.clone(),
                        message: error.to_string(),
                    })
            }
        }
    }

    fn decode_key(
        &self,
        jwk_set: &JwkSet,
        token: &str,
    ) -> Result<(Algorithm, DecodingKey), AuthError> {
        let header = decode_header(token).map_err(|error| AuthError::Jwt(error.to_string()))?;
        if !self.config.algorithms.contains(&header.alg) {
            return Err(AuthError::UnsupportedAlgorithm(format!("{:?}", header.alg)));
        }

        let jwk = match header.kid.as_deref() {
            Some(kid) => jwk_set
                .find(kid)
                .ok_or_else(|| AuthError::MissingJwk(kid.to_string()))?,
            None if jwk_set.keys.len() == 1 => &jwk_set.keys[0],
            None => return Err(AuthError::MissingKeyId),
        };

        let decoding_key =
            DecodingKey::from_jwk(jwk).map_err(|error| AuthError::Jwt(error.to_string()))?;
        Ok((header.alg, decoding_key))
    }

    fn build_identity(&self, claims: Claims) -> Result<AuthIdentity, AuthError> {
        let subject = claims
            .get("sub")
            .and_then(Value::as_str)
            .ok_or(AuthError::MissingSubject)?
            .to_string();

        Ok(AuthIdentity {
            provider: self.config.provider.clone(),
            subject,
            issuer: claims
                .get("iss")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            audiences: claims.get("aud").map(parse_audiences).unwrap_or_default(),
            expires_at: claims
                .get("exp")
                .and_then(Value::as_i64)
                .and_then(timestamp_to_utc),
            issued_at: claims
                .get("iat")
                .and_then(Value::as_i64)
                .and_then(timestamp_to_utc),
            not_before: claims
                .get("nbf")
                .and_then(Value::as_i64)
                .and_then(timestamp_to_utc),
            session_id: claims
                .get("sid")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            claims,
        })
    }
}

#[async_trait]
impl Authenticator for OidcJwtAuthenticator {
    async fn authenticate(
        &self,
        request: &AuthRequestParts,
    ) -> Result<Option<AuthIdentity>, AuthError> {
        let Some(token) = request.bearer_token()? else {
            return Ok(None);
        };

        let jwk_set = self.load_jwk_set().await?;
        let (algorithm, decoding_key) = self.decode_key(&jwk_set, token)?;
        let mut validation = Validation::new(algorithm);
        validation.algorithms = self.config.algorithms.clone();
        validation.leeway = self.config.leeway_seconds;
        validation.validate_nbf = true;

        let required_claims = self
            .config
            .required_claims
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        validation.set_required_spec_claims(&required_claims);
        if !self.config.issuer.is_empty() {
            validation.set_issuer(&[self.config.issuer.as_str()]);
        }
        if !self.config.audiences.is_empty() {
            validation.set_audience(&self.config.audiences);
        }

        let token_data = decode::<Claims>(token, &decoding_key, &validation)
            .map_err(|error| AuthError::Jwt(error.to_string()))?;

        self.build_identity(token_data.claims).map(Some)
    }
}

fn parse_audiences(value: &Value) -> Vec<String> {
    match value {
        Value::String(audience) => vec![audience.clone()],
        Value::Array(audiences) => audiences
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect(),
        _ => Vec::new(),
    }
}
