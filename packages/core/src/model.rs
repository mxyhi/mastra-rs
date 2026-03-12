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
    pub request_context: RequestContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelResponse {
    pub text: String,
    pub data: Value,
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
