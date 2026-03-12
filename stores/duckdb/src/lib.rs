#[path = "../../_test-utils/src/provider_support.rs"]
mod provider_support;

use provider_support::ensure_not_blank;
pub use provider_support::{
    ProviderBinding, ProviderBridge, ProviderCapability, ProviderConfigError, ProviderDescriptor,
    ProviderKind,
};

const DUCKDB_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::VectorStore];
const DUCKDB_DESCRIPTOR: ProviderDescriptor =
    ProviderDescriptor::new("duckdb", ProviderKind::Vector, DUCKDB_CAPABILITIES);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuckDbVectorConfig {
    pub endpoint: String,
    pub namespace: String,
    pub index_name: String,
    pub api_key: Option<String>,
}

impl Default for DuckDbVectorConfig {
    fn default() -> Self {
        Self {
            endpoint: "file:///tmp/mastra.duckdb".to_string(),
            namespace: "default".to_string(),
            index_name: "mastra".to_string(),
            api_key: None,
        }
    }
}

impl DuckDbVectorConfig {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        ensure_not_blank(&self.endpoint, "endpoint")?;
        ensure_not_blank(&self.namespace, "namespace")?;
        ensure_not_blank(&self.index_name, "index_name")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuckDbProvider {
    config: DuckDbVectorConfig,
}

impl DuckDbProvider {
    pub fn new(config: DuckDbVectorConfig) -> Result<Self, ProviderConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &DuckDbVectorConfig {
        &self.config
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        DUCKDB_DESCRIPTOR
    }

    pub fn vector_bridge(&self) -> ProviderBridge {
        let mut bridge = ProviderBridge::new(
            DUCKDB_DESCRIPTOR,
            format!("{}/{}", self.config.namespace, self.config.index_name),
        )
        .with_binding(ProviderBinding::plain(
            "endpoint",
            self.config.endpoint.clone(),
        ));

        if let Some(api_key) = &self.config.api_key {
            bridge = bridge.with_binding(ProviderBinding::secret("api_key", api_key.clone()));
        }

        bridge
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DuckDbProvider, DuckDbVectorConfig, ProviderCapability, ProviderConfigError, ProviderKind,
    };

    #[test]
    fn duckdb_provider_exposes_vector_bridge() {
        let provider = DuckDbProvider::new(DuckDbVectorConfig::default())
            .expect("duckdb config should be valid");

        let descriptor = provider.descriptor();
        let bridge = provider.vector_bridge();

        assert_eq!(descriptor.id, "duckdb");
        assert_eq!(descriptor.kind, ProviderKind::Vector);
        assert!(bridge.supports(ProviderCapability::VectorStore));
        assert_eq!(bridge.target, "default/mastra");
    }

    #[test]
    fn duckdb_provider_rejects_blank_namespace() {
        let error = DuckDbProvider::new(DuckDbVectorConfig {
            namespace: " ".into(),
            ..DuckDbVectorConfig::default()
        })
        .expect_err("blank namespace should be rejected");

        assert_eq!(error, ProviderConfigError::EmptyField("namespace"));
    }
}
