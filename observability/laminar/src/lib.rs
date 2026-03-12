use std::collections::BTreeMap;

use async_trait::async_trait;
use mastra_observability_mastra::{ExportError, HttpRequest, ObservabilityExporter, TraceBatch};
use mastra_observability_otel_exporter::{OtelConfig, OtelExporter};
use serde_json::json;

#[derive(Clone, Debug, Default)]
pub struct LaminarConfig {
    pub api_key: String,
    pub endpoint: String,
    pub headers: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct LaminarExporter {
    otel: OtelExporter,
}

impl LaminarExporter {
    pub fn new(config: LaminarConfig) -> Self {
        let mut headers = config.headers.clone();
        headers.insert(
            "authorization".to_string(),
            format!("Bearer {}", config.api_key),
        );

        let mut resource_attributes: std::collections::BTreeMap<String, serde_json::Value> =
            Default::default();
        resource_attributes.insert("lmnr.exporter".to_string(), json!("laminar"));

        Self {
            otel: OtelExporter::new(OtelConfig {
                endpoint: config.endpoint,
                headers,
                resource_attributes,
            }),
        }
    }

    pub fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        self.otel.build_requests(batch)
    }

    pub async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError> {
        self.otel.export(batch).await
    }
}

#[async_trait]
impl ObservabilityExporter for LaminarExporter {
    async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError> {
        self.export(batch).await
    }
}
