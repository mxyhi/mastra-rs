use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ExternalToolDefinition {
    pub id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub parameters: Option<Value>,
    pub input_schema: Option<Value>,
    pub output_schema: Option<Value>,
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedToolDefinition {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<Value>,
    pub output_schema: Option<Value>,
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizeToolError {
    MissingName,
}

pub fn normalize_tool(
    tool: ExternalToolDefinition,
) -> Result<NormalizedToolDefinition, NormalizeToolError> {
    let name = tool
        .name
        .or(tool.id)
        .ok_or(NormalizeToolError::MissingName)?;

    Ok(NormalizedToolDefinition {
        name,
        description: tool.description,
        input_schema: tool.input_schema.or(tool.parameters),
        output_schema: tool.output_schema,
        metadata: tool.metadata,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ExternalToolDefinition, NormalizeToolError, normalize_tool};

    #[test]
    fn normalizes_parameters_to_input_schema_and_uses_id_fallback() {
        let tool = ExternalToolDefinition {
            id: Some("weather".into()),
            parameters: Some(json!({"type":"object"})),
            ..ExternalToolDefinition::default()
        };

        let normalized = normalize_tool(tool).expect("tool should normalize");

        assert_eq!(normalized.name, "weather");
        assert_eq!(normalized.input_schema, Some(json!({"type":"object"})));
    }

    #[test]
    fn errors_when_name_and_id_are_missing() {
        let err = normalize_tool(ExternalToolDefinition::default())
            .expect_err("tool without identity should fail");

        assert_eq!(err, NormalizeToolError::MissingName);
    }
}
