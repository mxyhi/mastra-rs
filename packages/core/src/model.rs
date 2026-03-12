use std::{future::Future, pin::Pin, sync::Arc};

use async_trait::async_trait;
use futures::{StreamExt, stream};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    error::{MastraError, Result},
    request_context::RequestContext,
};

type ModelFuture = Pin<Box<dyn Future<Output = Result<ModelResponse>> + Send + 'static>>;
type ModelHandler = Arc<dyn Fn(ModelRequest) -> ModelFuture + Send + Sync>;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelRequest {
    pub prompt: String,
    pub instructions: String,
    pub memory: Vec<String>,
    pub tool_names: Vec<String>,
    pub tool_results: Vec<ModelToolResult>,
    pub run_id: Option<String>,
    pub thread_id: Option<String>,
    pub max_steps: Option<u32>,
    pub request_context: RequestContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    #[default]
    Stop,
    ToolCall,
    Length,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct UsageStats {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ModelToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ModelToolResult {
    pub id: String,
    pub name: String,
    pub output: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelResponse {
    pub text: String,
    pub data: Value,
    #[serde(default)]
    pub finish_reason: FinishReason,
    #[serde(default)]
    pub usage: Option<UsageStats>,
    #[serde(default)]
    pub tool_calls: Vec<ModelToolCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum ModelEvent {
    TextDelta(String),
    Done(ModelResponse),
}

#[async_trait]
pub trait LanguageModel: Send + Sync {
    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse>;

    fn stream(
        &self,
        request: ModelRequest,
    ) -> futures::stream::BoxStream<'static, Result<ModelEvent>> {
        let model = self.clone_box();
        stream::once(async move {
            let response = model.generate(request).await?;
            Ok(ModelEvent::Done(response))
        })
        .boxed()
    }

    fn clone_box(&self) -> Box<dyn LanguageModel>;
}

impl Clone for Box<dyn LanguageModel> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl ModelResponse {
    pub fn normalized_finish_reason(&self) -> FinishReason {
        if !self.tool_calls.is_empty() && self.finish_reason == FinishReason::Stop {
            return FinishReason::ToolCall;
        }

        match self
            .data
            .get("finish_reason")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "tool_call" | "tool_calls" => FinishReason::ToolCall,
            "length" => FinishReason::Length,
            _ => self.finish_reason.clone(),
        }
    }

    pub fn normalized_usage(&self) -> Option<UsageStats> {
        self.usage.clone().or_else(|| {
            let usage = self.data.get("usage")?;
            let prompt_tokens = usage.get("prompt_tokens")?.as_u64()?;
            let completion_tokens = usage.get("completion_tokens")?.as_u64()?;
            Some(UsageStats {
                prompt_tokens: prompt_tokens as u32,
                completion_tokens: completion_tokens as u32,
            })
        })
    }

    pub fn normalized_tool_calls(&self) -> Vec<ModelToolCall> {
        if !self.tool_calls.is_empty() {
            return self.tool_calls.clone();
        }

        self.data
            .get("tool_calls")
            .and_then(Value::as_array)
            .map(|calls| {
                calls
                    .iter()
                    .filter_map(|call| {
                        Some(ModelToolCall {
                            id: call.get("id")?.as_str()?.to_owned(),
                            name: call.get("name")?.as_str()?.to_owned(),
                            input: call.get("input").cloned().unwrap_or(Value::Null),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Clone)]
pub struct StaticModel {
    handler: ModelHandler,
}

impl StaticModel {
    pub fn new<F, Fut>(handler: F) -> Self
    where
        F: Fn(ModelRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ModelResponse>> + Send + 'static,
    {
        Self {
            handler: Arc::new(move |request| Box::pin(handler(request))),
        }
    }

    pub fn echo() -> Self {
        Self::new(|request| async move {
            let memory_prefix = if request.memory.is_empty() {
                String::new()
            } else {
                format!("{}\n", request.memory.join("\n"))
            };

            Ok(ModelResponse {
                text: format!("{}{}", memory_prefix, request.prompt),
                data: Value::Null,
                finish_reason: FinishReason::Stop,
                usage: None,
                tool_calls: Vec::new(),
            })
        })
    }
}

#[async_trait]
impl LanguageModel for StaticModel {
    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
        (self.handler)(request).await
    }

    fn stream(
        &self,
        request: ModelRequest,
    ) -> futures::stream::BoxStream<'static, Result<ModelEvent>> {
        let handler = Arc::clone(&self.handler);
        stream::iter([request])
            .then(move |request| {
                let handler = Arc::clone(&handler);
                async move {
                    let response = handler(request).await?;
                    Ok(ModelEvent::Done(response))
                }
            })
            .boxed()
    }

    fn clone_box(&self) -> Box<dyn LanguageModel> {
        Box::new(self.clone())
    }
}

impl Default for StaticModel {
    fn default() -> Self {
        Self::new(|_| async { Err(MastraError::model("no model handler configured")) })
    }
}
