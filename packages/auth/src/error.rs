use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("authorization header used unsupported scheme `{0}`")]
    UnsupportedAuthorizationScheme(String),
    #[error("authorization header was missing bearer token")]
    MissingBearerToken,
    #[error("token header missing key id")]
    MissingKeyId,
    #[error("token header missing algorithm")]
    MissingAlgorithm,
    #[error("token algorithm `{0}` is not allowed")]
    UnsupportedAlgorithm(String),
    #[error("failed to fetch JWKS from `{uri}`: {message}")]
    JwksFetch { uri: String, message: String },
    #[error("failed to parse JWKS from `{uri}`: {message}")]
    JwksParse { uri: String, message: String },
    #[error("no matching JWK found for kid `{0}`")]
    MissingJwk(String),
    #[error("token claims must be a JSON object")]
    InvalidClaimsShape,
    #[error("token missing subject claim")]
    MissingSubject,
    #[error("callback request missing code")]
    MissingCallbackCode,
    #[error("callback request missing state")]
    MissingCallbackState,
    #[error("JWT error: {0}")]
    Jwt(String),
}
