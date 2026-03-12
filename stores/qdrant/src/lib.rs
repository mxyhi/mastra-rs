#[path = "../../_test-utils/src/provider_support.rs"]
mod provider_support;

use provider_support::ensure_not_blank;
pub use provider_support::{
    ProviderBinding, ProviderBridge, ProviderCapability, ProviderConfigError, ProviderDescriptor,
    ProviderKind,
};

const QDRANT_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::VectorStore];
const QDRANT_DESCRIPTOR: ProviderDescriptor =
    ProviderDescriptor::new("qdrant", ProviderKind::Vector, QDRANT_CAPABILITIES);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QdrantVectorConfig {
    pub endpoint: String,
    pub namespace: String,
    pub index_name: String,
    pub api_key: Option<String>,
}

impl Default for QdrantVectorConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:6334".to_string(),
            namespace: "default".to_string(),
            index_name: "mastra".to_string(),
            api_key: None,
        }
    }
}

impl QdrantVectorConfig {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        ensure_not_blank(&self.endpoint, "endpoint")?;
        ensure_not_blank(&self.namespace, "namespace")?;
        ensure_not_blank(&self.index_name, "index_name")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QdrantProvider {
    config: QdrantVectorConfig,
}

impl QdrantProvider {
    pub fn new(config: QdrantVectorConfig) -> Result<Self, ProviderConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &QdrantVectorConfig {
        &self.config
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        QDRANT_DESCRIPTOR
    }

    pub fn vector_bridge(&self) -> ProviderBridge {
        let mut bridge = ProviderBridge::new(
            QDRANT_DESCRIPTOR,
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
        ProviderCapability, ProviderConfigError, ProviderKind, QdrantProvider, QdrantVectorConfig,
    };

    #[test]
    fn qdrant_provider_exposes_vector_bridge() {
        let provider = QdrantProvider::new(QdrantVectorConfig::default())
            .expect("qdrant config should be valid");

        let descriptor = provider.descriptor();
        let bridge = provider.vector_bridge();

        assert_eq!(descriptor.id, "qdrant");
        assert_eq!(descriptor.kind, ProviderKind::Vector);
        assert!(bridge.supports(ProviderCapability::VectorStore));
        assert_eq!(bridge.target, "default/mastra");
    }

    #[test]
    fn qdrant_provider_rejects_blank_index_name() {
        let error = QdrantProvider::new(QdrantVectorConfig {
            index_name: " ".into(),
            ..QdrantVectorConfig::default()
        })
        .expect_err("blank index should be rejected");

        assert_eq!(error, ProviderConfigError::EmptyField("index_name"));
    }
}
