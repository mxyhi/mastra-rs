use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmRequest {
    pub url: String,
    pub method: String,
    pub body: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmRecording {
    pub hash: String,
    pub request: LlmRequest,
    pub response: LlmResponse,
}

#[derive(Debug, Clone, Default)]
pub struct Recorder {
    recordings: Vec<LlmRecording>,
}

impl Recorder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, request: LlmRequest, response: LlmResponse) -> &LlmRecording {
        let hash = hash_request(&request);
        self.recordings.push(LlmRecording {
            hash,
            request,
            response,
        });
        self.recordings.last().expect("record inserted")
    }

    pub fn find(&self, request: &LlmRequest) -> Option<&LlmRecording> {
        let hash = hash_request(request);
        self.recordings
            .iter()
            .find(|recording| recording.hash == hash)
    }

    pub fn recordings(&self) -> &[LlmRecording] {
        &self.recordings
    }
}

pub fn hash_request(request: &LlmRequest) -> String {
    let mut hasher = DefaultHasher::new();
    request.url.hash(&mut hasher);
    request.method.hash(&mut hasher);
    request.body.to_string().hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaNode {
    Null,
    Bool,
    Number,
    String,
    Array(Box<SchemaNode>),
    Object(BTreeMap<String, SchemaNode>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractDifference {
    pub path: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractValidationResult {
    pub valid: bool,
    pub differences: Vec<ContractDifference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractValidationOptions {
    pub ignore_paths: BTreeSet<String>,
    pub allow_extra_fields: bool,
    pub allow_missing_fields: bool,
    pub treat_null_as_optional: bool,
}

impl Default for ContractValidationOptions {
    fn default() -> Self {
        Self {
            ignore_paths: BTreeSet::new(),
            allow_extra_fields: true,
            allow_missing_fields: false,
            treat_null_as_optional: true,
        }
    }
}

pub fn extract_schema(value: &Value) -> SchemaNode {
    match value {
        Value::Null => SchemaNode::Null,
        Value::Bool(_) => SchemaNode::Bool,
        Value::Number(_) => SchemaNode::Number,
        Value::String(_) => SchemaNode::String,
        Value::Array(values) => values
            .first()
            .map(extract_schema)
            .map(Box::new)
            .map(SchemaNode::Array)
            .unwrap_or_else(|| SchemaNode::Array(Box::new(SchemaNode::Null))),
        Value::Object(map) => SchemaNode::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), extract_schema(value)))
                .collect(),
        ),
    }
}

pub fn validate_contract(
    actual: &Value,
    expected: &Value,
    options: &ContractValidationOptions,
) -> ContractValidationResult {
    let mut differences = Vec::new();
    compare_values(actual, expected, "", options, &mut differences);
    ContractValidationResult {
        valid: differences.is_empty(),
        differences,
    }
}

fn compare_values(
    actual: &Value,
    expected: &Value,
    path: &str,
    options: &ContractValidationOptions,
    differences: &mut Vec<ContractDifference>,
) {
    if options.ignore_paths.contains(path) {
        return;
    }

    let actual_schema = extract_schema(actual);
    let expected_schema = extract_schema(expected);

    match (&actual_schema, &expected_schema) {
        (SchemaNode::Object(actual), SchemaNode::Object(expected)) => {
            for (key, expected_value) in expected {
                let next_path = join_path(path, key);
                match actual.get(key) {
                    Some(actual_value) => compare_schema_nodes(
                        actual_value,
                        expected_value,
                        &next_path,
                        options,
                        differences,
                    ),
                    None if !options.allow_missing_fields => differences.push(ContractDifference {
                        path: next_path,
                        expected: format!("{expected_value:?}"),
                        actual: "missing".into(),
                    }),
                    None => {}
                }
            }

            if !options.allow_extra_fields {
                for (key, actual_value) in actual {
                    if !expected.contains_key(key) {
                        differences.push(ContractDifference {
                            path: join_path(path, key),
                            expected: "absent".into(),
                            actual: format!("{actual_value:?}"),
                        });
                    }
                }
            }
        }
        _ => compare_schema_nodes(&actual_schema, &expected_schema, path, options, differences),
    }
}

fn compare_schema_nodes(
    actual: &SchemaNode,
    expected: &SchemaNode,
    path: &str,
    options: &ContractValidationOptions,
    differences: &mut Vec<ContractDifference>,
) {
    if actual == expected {
        return;
    }

    if options.treat_null_as_optional
        && (*actual == SchemaNode::Null || *expected == SchemaNode::Null)
    {
        return;
    }

    differences.push(ContractDifference {
        path: if path.is_empty() {
            "(root)".into()
        } else {
            path.into()
        },
        expected: format!("{expected:?}"),
        actual: format!("{actual:?}"),
    });
}

fn join_path(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::{ContractValidationOptions, LlmRequest, LlmResponse, Recorder, validate_contract};

    #[test]
    fn recorder_finds_recordings_by_hashed_request() {
        let mut recorder = Recorder::new();
        let request = LlmRequest {
            url: "https://api.example.dev/chat".into(),
            method: "POST".into(),
            body: json!({"messages":[{"role":"user","content":"hello"}]}),
        };
        recorder.record(
            request.clone(),
            LlmResponse {
                status: 200,
                headers: BTreeMap::new(),
                body: json!({"id":"1","content":"hi"}),
            },
        );

        assert!(recorder.find(&request).is_some());
    }

    #[test]
    fn contract_validation_can_ignore_dynamic_fields() {
        let result = validate_contract(
            &json!({"id":"abc","usage":{"tokens":10},"content":"hello"}),
            &json!({"id":"expected","usage":{"tokens":1},"content":"hello"}),
            &ContractValidationOptions {
                ignore_paths: ["id".into(), "usage".into()].into_iter().collect(),
                ..ContractValidationOptions::default()
            },
        );

        assert!(result.valid);
    }

    #[test]
    fn contract_validation_reports_type_changes() {
        let result = validate_contract(
            &json!({"content": ["hello"]}),
            &json!({"content": "hello"}),
            &ContractValidationOptions::default(),
        );

        assert!(!result.valid);
        assert_eq!(result.differences[0].path, "content");
    }
}
