use thiserror::Error;

pub type Result<T> = std::result::Result<T, DeployerError>;

#[derive(Debug, Error)]
pub enum DeployerError {
    #[error("deployment bundle entrypoint `{0}` is missing from bundle artifacts")]
    MissingEntrypoint(String),
    #[error("artifact path `{0}` must stay within the deployment root")]
    InvalidArtifactPath(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json serialization error: {0}")]
    Json(#[from] serde_json::Error),
}
