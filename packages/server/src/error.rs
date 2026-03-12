use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

use crate::contracts::ErrorResponse;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("{resource} '{id}' was not found")]
    NotFound { resource: &'static str, id: String },
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Internal(String),
}

impl ServerError {
    pub fn internal<E>(error: E) -> Self
    where
        E: std::fmt::Display,
    {
        Self::Internal(error.to_string())
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = ErrorResponse {
            error: self.to_string(),
        };

        (status, Json(body)).into_response()
    }
}

pub type ServerResult<T> = Result<T, ServerError>;
