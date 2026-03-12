use thiserror::Error;

pub type Result<T> = std::result::Result<T, McpError>;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum McpError {
    #[error("tool '{0}' was not found")]
    ToolNotFound(String),
    #[error("resource '{0}' was not found")]
    ResourceNotFound(String),
    #[error("prompt '{0}' was not found")]
    PromptNotFound(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("execution failed: {0}")]
    Execution(String),
}
