use mastra_observability_mastra::{Attributes, TraceBatch};
use mastra_observability_otel_exporter::build_otel_payload;
use serde_json::Value;

#[derive(Clone, Debug, Default)]
pub struct OtelBridge {
    pub resource_attributes: Attributes,
}

impl OtelBridge {
    pub fn new(resource_attributes: Attributes) -> Self {
        Self {
            resource_attributes,
        }
    }

    pub fn to_payload(&self, batch: &TraceBatch) -> Value {
        build_otel_payload(batch, &self.resource_attributes)
    }
}
