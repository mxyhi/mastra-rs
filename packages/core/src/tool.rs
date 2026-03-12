use std::{future::Future, pin::Pin, sync::Arc};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    error::{MastraError, Result},
    request_context::RequestContext,
};

type ToolFuture = Pin<Box<dyn Future<Output = Result<Value>> + Send + 'static>>;
type ToolHandler = Arc<dyn Fn(Value, ToolExecutionContext) -> ToolFuture + Send + Sync>;

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ToolExecutionContext {
    pub request_context: RequestContext,
    pub run_id: Option<String>,
    pub thread_id: Option<String>,
    pub approved: bool,
}

#[derive(Clone)]
pub struct ToolConfig {
    pub id: String,
    pub description: String,
    pub input_schema: Option<Value>,
    pub output_schema: Option<Value>,
    pub require_approval: bool,
    pub handler: ToolHandler,
}

#[derive(Clone)]
pub struct Tool {
    id: String,
    description: String,
    input_schema: Option<Value>,
    output_schema: Option<Value>,
    require_approval: bool,
    handler: ToolHandler,
}

impl Tool {
    pub fn new<F, Fut>(id: impl Into<String>, description: impl Into<String>, handler: F) -> Self
    where
        F: Fn(Value, ToolExecutionContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value>> + Send + 'static,
    {
        Self {
            id: id.into(),
            description: description.into(),
            input_schema: None,
            output_schema: None,
            require_approval: false,
            handler: Arc::new(move |input, context| Box::pin(handler(input, context))),
        }
    }

    pub fn from_config(config: ToolConfig) -> Self {
        Self {
            id: config.id,
            description: config.description,
            input_schema: config.input_schema,
            output_schema: config.output_schema,
            require_approval: config.require_approval,
            handler: config.handler,
        }
    }

    pub fn with_input_schema<T: JsonSchema>(mut self) -> Self {
        self.input_schema = serde_json::to_value(schemars::schema_for!(T)).ok();
        self
    }

    pub fn with_output_schema<T: JsonSchema>(mut self) -> Self {
        self.output_schema = serde_json::to_value(schemars::schema_for!(T)).ok();
        self
    }

    pub fn requiring_approval(mut self) -> Self {
        self.require_approval = true;
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn input_schema(&self) -> Option<&Value> {
        self.input_schema.as_ref()
    }

    pub fn output_schema(&self) -> Option<&Value> {
        self.output_schema.as_ref()
    }

    pub fn requires_approval(&self) -> bool {
        self.require_approval
    }

    pub fn schema_snapshot(&self) -> Value {
        json!({
          "input": self.input_schema,
          "output": self.output_schema,
          "requireApproval": self.require_approval,
        })
    }

    pub async fn execute(&self, input: Value, context: ToolExecutionContext) -> Result<Value> {
        if self.require_approval && !context.approved {
            return Err(MastraError::approval_required(format!(
                "tool '{}' requires approval before execution",
                self.id
            )));
        }

        validate_top_level_shape(&self.input_schema, &input, &self.id, "input")?;
        let output = (self.handler)(input, context).await?;
        validate_top_level_shape(&self.output_schema, &output, &self.id, "output")?;
        Ok(output)
    }
}

fn validate_top_level_shape(
    schema: &Option<Value>,
    value: &Value,
    tool_id: &str,
    label: &str,
) -> Result<()> {
    let Some(expected_type) = schema
        .as_ref()
        .and_then(|schema| schema.get("type"))
        .and_then(Value::as_str)
    else {
        return Ok(());
    };

    let matches = match expected_type {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => true,
    };

    if matches {
        Ok(())
    } else {
        Err(MastraError::validation(format!(
            "tool '{}' {} failed top-level schema validation: expected {}",
            tool_id, label, expected_type
        )))
    }
}
