use std::sync::Arc;

use async_trait::async_trait;
use mastra_core::RequestContext;

use crate::{
    error::Result,
    server::McpServer,
    types::{McpPrompt, McpResource, McpTool},
};

#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn list_tools(&self) -> Result<Vec<McpTool>>;
    async fn call_tool(
        &self,
        tool_name: &str,
        input: serde_json::Value,
        request_context: RequestContext,
    ) -> Result<serde_json::Value>;
    async fn list_resources(&self) -> Result<Vec<McpResource>>;
    async fn read_resource(&self, uri: &str) -> Result<McpResource>;
    async fn list_prompts(&self) -> Result<Vec<McpPrompt>>;
    async fn get_prompt(&self, name: &str) -> Result<McpPrompt>;
}

#[derive(Clone)]
pub struct LocalTransport {
    server: McpServer,
}

impl LocalTransport {
    pub fn new(server: McpServer) -> Self {
        Self { server }
    }
}

#[async_trait]
impl McpTransport for LocalTransport {
    async fn list_tools(&self) -> Result<Vec<McpTool>> {
        self.server.list_tools().await
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        input: serde_json::Value,
        request_context: RequestContext,
    ) -> Result<serde_json::Value> {
        self.server
            .call_tool(tool_name, input, request_context)
            .await
    }

    async fn list_resources(&self) -> Result<Vec<McpResource>> {
        self.server.list_resources().await
    }

    async fn read_resource(&self, uri: &str) -> Result<McpResource> {
        self.server.read_resource(uri).await
    }

    async fn list_prompts(&self) -> Result<Vec<McpPrompt>> {
        self.server.list_prompts().await
    }

    async fn get_prompt(&self, name: &str) -> Result<McpPrompt> {
        self.server.get_prompt(name).await
    }
}

#[derive(Clone)]
pub struct McpClient {
    transport: Arc<dyn McpTransport>,
}

impl McpClient {
    pub fn new<T>(transport: T) -> Self
    where
        T: McpTransport + 'static,
    {
        Self {
            transport: Arc::new(transport),
        }
    }

    pub fn local(server: McpServer) -> Self {
        Self::new(LocalTransport::new(server))
    }

    pub async fn list_tools(&self) -> Result<Vec<McpTool>> {
        self.transport.list_tools().await
    }

    pub async fn call_tool(
        &self,
        tool_name: &str,
        input: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.call_tool_with_context(tool_name, input, RequestContext::new())
            .await
    }

    pub async fn call_tool_with_context(
        &self,
        tool_name: &str,
        input: serde_json::Value,
        request_context: RequestContext,
    ) -> Result<serde_json::Value> {
        self.transport
            .call_tool(tool_name, input, request_context)
            .await
    }

    pub async fn list_resources(&self) -> Result<Vec<McpResource>> {
        self.transport.list_resources().await
    }

    pub async fn read_resource(&self, uri: &str) -> Result<McpResource> {
        self.transport.read_resource(uri).await
    }

    pub async fn list_prompts(&self) -> Result<Vec<McpPrompt>> {
        self.transport.list_prompts().await
    }

    pub async fn get_prompt(&self, name: &str) -> Result<McpPrompt> {
        self.transport.get_prompt(name).await
    }
}
