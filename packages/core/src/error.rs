use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, MastraError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub enum MastraErrorCode {
  Validation,
  NotFound,
  ApprovalRequired,
  Model,
  Tool,
  Workflow,
  Storage,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Error)]
#[error("{message}")]
pub struct MastraError {
  pub code: MastraErrorCode,
  pub message: String,
}

impl MastraError {
  pub fn new(code: MastraErrorCode, message: impl Into<String>) -> Self {
    Self {
      code,
      message: message.into(),
    }
  }

  pub fn validation(message: impl Into<String>) -> Self {
    Self::new(MastraErrorCode::Validation, message)
  }

  pub fn not_found(message: impl Into<String>) -> Self {
    Self::new(MastraErrorCode::NotFound, message)
  }

  pub fn approval_required(message: impl Into<String>) -> Self {
    Self::new(MastraErrorCode::ApprovalRequired, message)
  }

  pub fn model(message: impl Into<String>) -> Self {
    Self::new(MastraErrorCode::Model, message)
  }

  pub fn tool(message: impl Into<String>) -> Self {
    Self::new(MastraErrorCode::Tool, message)
  }

  pub fn workflow(message: impl Into<String>) -> Self {
    Self::new(MastraErrorCode::Workflow, message)
  }

  pub fn storage(message: impl Into<String>) -> Self {
    Self::new(MastraErrorCode::Storage, message)
  }
}
