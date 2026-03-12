mod client;
mod error;
mod server;
mod types;

pub use client::{LocalTransport, McpClient, McpTransport};
pub use error::{McpError, Result};
pub use server::{McpServer, McpServerConfig};
pub use types::{
    McpPrompt, McpPromptArgument, McpPromptMessage, McpPromptRole, McpResource,
    McpResourceContents, McpTool,
};
