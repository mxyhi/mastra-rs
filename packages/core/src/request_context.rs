use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const RESERVED_RESOURCE_ID: &str = "mastra__resourceId";
pub const RESERVED_THREAD_ID: &str = "mastra__threadId";

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RequestContext {
  values: IndexMap<String, Value>,
}

impl RequestContext {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn from_value_map(values: IndexMap<String, Value>) -> Self {
    Self { values }
  }

  pub fn insert(&mut self, key: impl Into<String>, value: impl Into<Value>) -> Option<Value> {
    self.values.insert(key.into(), value.into())
  }

  pub fn with_resource_id(mut self, resource_id: impl Into<String>) -> Self {
    self.insert(RESERVED_RESOURCE_ID, resource_id.into());
    self
  }

  pub fn with_thread_id(mut self, thread_id: impl Into<String>) -> Self {
    self.insert(RESERVED_THREAD_ID, thread_id.into());
    self
  }

  pub fn get(&self, key: &str) -> Option<&Value> {
    self.values.get(key)
  }

  pub fn resource_id(&self) -> Option<&str> {
    self.get(RESERVED_RESOURCE_ID).and_then(Value::as_str)
  }

  pub fn thread_id(&self) -> Option<&str> {
    self.get(RESERVED_THREAD_ID).and_then(Value::as_str)
  }

  pub fn values(&self) -> &IndexMap<String, Value> {
    &self.values
  }
}
