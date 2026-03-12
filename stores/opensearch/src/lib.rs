#[path = "../../_test-utils/src/provider_support.rs"]
mod provider_support;

use provider_support::ensure_not_blank;
pub use provider_support::{
    ProviderBinding, ProviderBridge, ProviderCapability, ProviderConfigError, ProviderDescriptor,
    ProviderKind,
};

const OPENSEARCH_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::VectorStore];
const OPENSEARCH_DESCRIPTOR: ProviderDescriptor =
    ProviderDescriptor::new("opensearch", ProviderKind::Vector, OPENSEARCH_CAPABILITIES);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSearchVectorConfig {
    pub endpoint: String,
    pub namespace: String,
    pub index_name: String,
    pub api_key: Option<String>,
}

impl Default for OpenSearchVectorConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:9200".to_string(),
            namespace: "default".to_string(),
            index_name: "mastra".to_string(),
            api_key: None,
        }
    }
}

impl OpenSearchVectorConfig {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        ensure_not_blank(&self.endpoint, "endpoint")?;
        ensure_not_blank(&self.namespace, "namespace")?;
        ensure_not_blank(&self.index_name, "index_name")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSearchProvider {
    config: OpenSearchVectorConfig,
}

impl OpenSearchProvider {
    pub fn new(config: OpenSearchVectorConfig) -> Result<Self, ProviderConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &OpenSearchVectorConfig {
        &self.config
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        OPENSEARCH_DESCRIPTOR
    }

    pub fn vector_bridge(&self) -> ProviderBridge {
        let mut bridge = ProviderBridge::new(
            OPENSEARCH_DESCRIPTOR,
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
        OpenSearchProvider, OpenSearchVectorConfig, ProviderCapability, ProviderConfigError,
        ProviderKind,
    };

    #[test]
    fn opensearch_provider_exposes_vector_bridge() {
        let provider = OpenSearchProvider::new(OpenSearchVectorConfig::default())
            .expect("opensearch config should be valid");

        let descriptor = provider.descriptor();
        let bridge = provider.vector_bridge();

        assert_eq!(descriptor.id, "opensearch");
        assert_eq!(descriptor.kind, ProviderKind::Vector);
        assert!(bridge.supports(ProviderCapability::VectorStore));
        assert_eq!(bridge.target, "default/mastra");
    }

    #[test]
    fn opensearch_provider_rejects_blank_endpoint() {
        let error = OpenSearchProvider::new(OpenSearchVectorConfig {
            endpoint: String::new(),
            ..OpenSearchVectorConfig::default()
        })
        .expect_err("blank endpoint should be rejected");

        assert_eq!(error, ProviderConfigError::EmptyField("endpoint"));
    }
}
