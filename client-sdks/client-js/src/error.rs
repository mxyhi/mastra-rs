use reqwest::StatusCode;
use thiserror::Error;

use crate::types::ErrorResponse;

#[derive(Debug, Error)]
pub enum MastraClientError {
    #[error("invalid base url: {0}")]
    InvalidBaseUrl(#[from] url::ParseError),
    #[error("failed to build reqwest client: {0}")]
    Build(reqwest::Error),
    #[error("request failed: {0}")]
    Transport(reqwest::Error),
    #[error("failed to decode response body: {0}")]
    Decode(reqwest::Error),
    #[error("api returned {status}: {body}")]
    Api {
        status: StatusCode,
        body: String,
        error: Option<ErrorResponse>,
    },
}

impl MastraClientError {
    pub fn status(&self) -> Option<StatusCode> {
        match self {
            Self::Api { status, .. } => Some(*status),
            _ => None,
        }
    }
}
