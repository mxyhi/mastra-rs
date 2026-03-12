#[path = "../../_test-utils/src/provider_support.rs"]
mod provider_support;

use provider_support::ensure_not_blank;
pub use provider_support::{
    ProviderBinding, ProviderBridge, ProviderCapability, ProviderConfigError, ProviderDescriptor,
    ProviderKind,
};

const ELASTICSEARCH_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::VectorStore];
const ELASTICSEARCH_DESCRIPTOR: ProviderDescriptor = ProviderDescriptor::new(
    "elasticsearch",
    ProviderKind::Vector,
    ELASTICSEARCH_CAPABILITIES,
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElasticsearchVectorConfig {
    pub endpoint: String,
    pub namespace: String,
    pub index_name: String,
    pub api_key: Option<String>,
}

impl Default for ElasticsearchVectorConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:9200".to_string(),
            namespace: "default".to_string(),
            index_name: "mastra".to_string(),
            api_key: None,
        }
    }
}

impl ElasticsearchVectorConfig {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        ensure_not_blank(&self.endpoint, "endpoint")?;
        ensure_not_blank(&self.namespace, "namespace")?;
        ensure_not_blank(&self.index_name, "index_name")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElasticsearchProvider {
    config: ElasticsearchVectorConfig,
}

impl ElasticsearchProvider {
    pub fn new(config: ElasticsearchVectorConfig) -> Result<Self, ProviderConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &ElasticsearchVectorConfig {
        &self.config
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        ELASTICSEARCH_DESCRIPTOR
    }

    pub fn vector_bridge(&self) -> ProviderBridge {
        let mut bridge = ProviderBridge::new(
            ELASTICSEARCH_DESCRIPTOR,
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
        ElasticsearchProvider, ElasticsearchVectorConfig, ProviderCapability, ProviderConfigError,
        ProviderKind,
    };

    #[test]
    fn elasticsearch_provider_exposes_vector_bridge() {
        let provider = ElasticsearchProvider::new(ElasticsearchVectorConfig::default())
            .expect("elasticsearch config should be valid");

        let descriptor = provider.descriptor();
        let bridge = provider.vector_bridge();

        assert_eq!(descriptor.id, "elasticsearch");
        assert_eq!(descriptor.kind, ProviderKind::Vector);
        assert!(bridge.supports(ProviderCapability::VectorStore));
        assert_eq!(bridge.target, "default/mastra");
    }

    #[test]
    fn elasticsearch_provider_rejects_blank_index_name() {
        let error = ElasticsearchProvider::new(ElasticsearchVectorConfig {
            index_name: String::new(),
            ..ElasticsearchVectorConfig::default()
        })
        .expect_err("blank index should be rejected");

        assert_eq!(error, ProviderConfigError::EmptyField("index_name"));
    }
}
