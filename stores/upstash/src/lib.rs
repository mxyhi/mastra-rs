#[path = "../../_test-utils/src/provider_support.rs"]
mod provider_support;

use provider_support::ensure_not_blank;
pub use provider_support::{
    ProviderBinding, ProviderBridge, ProviderCapability, ProviderConfigError, ProviderDescriptor,
    ProviderKind,
};

const UPSTASH_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::MemoryStore,
    ProviderCapability::VectorStore,
];
const UPSTASH_DESCRIPTOR: ProviderDescriptor =
    ProviderDescriptor::new("upstash", ProviderKind::Hybrid, UPSTASH_CAPABILITIES);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstashStoreConfig {
    pub endpoint: String,
    pub database: String,
    pub storage_namespace: String,
    pub vector_namespace: String,
    pub index_name: String,
    pub api_key: Option<String>,
}

impl Default for UpstashStoreConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://upstash.local".to_string(),
            database: "mastra".to_string(),
            storage_namespace: "messages".to_string(),
            vector_namespace: "default".to_string(),
            index_name: "embeddings".to_string(),
            api_key: None,
        }
    }
}

impl UpstashStoreConfig {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        ensure_not_blank(&self.endpoint, "endpoint")?;
        ensure_not_blank(&self.database, "database")?;
        ensure_not_blank(&self.storage_namespace, "storage_namespace")?;
        ensure_not_blank(&self.vector_namespace, "vector_namespace")?;
        ensure_not_blank(&self.index_name, "index_name")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstashProvider {
    config: UpstashStoreConfig,
}

impl UpstashProvider {
    pub fn new(config: UpstashStoreConfig) -> Result<Self, ProviderConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &UpstashStoreConfig {
        &self.config
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        UPSTASH_DESCRIPTOR
    }

    pub fn storage_bridge(&self) -> ProviderBridge {
        let mut bridge = ProviderBridge::new(
            UPSTASH_DESCRIPTOR,
            format!("{}/{}", self.config.database, self.config.storage_namespace),
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

    pub fn vector_bridge(&self) -> ProviderBridge {
        let mut bridge = ProviderBridge::new(
            UPSTASH_DESCRIPTOR,
            format!(
                "{}/{}",
                self.config.vector_namespace, self.config.index_name
            ),
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
        ProviderCapability, ProviderConfigError, ProviderKind, UpstashProvider, UpstashStoreConfig,
    };

    #[test]
    fn upstash_provider_splits_storage_and_vector_bridges() {
        let provider = UpstashProvider::new(UpstashStoreConfig::default())
            .expect("upstash config should be valid");

        let descriptor = provider.descriptor();
        let storage = provider.storage_bridge();
        let vector = provider.vector_bridge();

        assert_eq!(descriptor.id, "upstash");
        assert_eq!(descriptor.kind, ProviderKind::Hybrid);
        assert!(storage.supports(ProviderCapability::MemoryStore));
        assert!(vector.supports(ProviderCapability::VectorStore));
        assert_eq!(storage.target, "mastra/messages");
        assert_eq!(vector.target, "default/embeddings");
    }

    #[test]
    fn upstash_provider_rejects_blank_storage_namespace() {
        let error = UpstashProvider::new(UpstashStoreConfig {
            storage_namespace: String::new(),
            ..UpstashStoreConfig::default()
        })
        .expect_err("blank storage namespace should be rejected");

        assert_eq!(error, ProviderConfigError::EmptyField("storage_namespace"));
    }
}
