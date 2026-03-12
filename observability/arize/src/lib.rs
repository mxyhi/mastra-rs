use std::collections::BTreeMap;

use async_trait::async_trait;
use mastra_observability_mastra::{ExportError, HttpRequest, ObservabilityExporter, TraceBatch};
use mastra_observability_otel_exporter::{OtelConfig, OtelExporter};
use serde_json::json;

#[derive(Clone, Debug, Default)]
pub struct ArizeConfig {
    pub endpoint: String,
    pub api_key: Option<String>,
    pub project_name: Option<String>,
    pub headers: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct ArizeExporter {
    otel: OtelExporter,
}

impl ArizeExporter {
    pub fn new(config: ArizeConfig) -> Self {
        let mut headers = config.headers.clone();
        if let Some(api_key) = &config.api_key {
            headers.insert("api_key".to_string(), api_key.clone());
        }

        let mut resource_attributes: std::collections::BTreeMap<String, serde_json::Value> =
            Default::default();
        resource_attributes.insert(
            "openinference.exporter".to_string(),
            json!("arize"),
        );
        if let Some(project_name) = &config.project_name {
            resource_attributes.insert(
                "openinference.project.name".to_string(),
                json!(project_name),
            );
        }

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
impl ObservabilityExporter for ArizeExporter {
    async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError> {
        self.export(batch).await
    }
}
