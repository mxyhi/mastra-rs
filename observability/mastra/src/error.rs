use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("invalid exporter configuration: {0}")]
    InvalidConfiguration(String),
    #[error("failed to serialize export payload: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("http transport failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("invalid url: {0}")]
    Url(#[from] url::ParseError),
    #[error("export request to {url} failed with status {status_code}: {response_body}")]
    UnexpectedStatus {
        url: Url,
        status_code: u16,
        response_body: String,
    },
}
