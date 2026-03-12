#[path = "../../_test-utils/src/provider_support.rs"]
mod provider_support;

use provider_support::ensure_not_blank;
pub use provider_support::{
    ProviderBinding, ProviderBridge, ProviderCapability, ProviderConfigError, ProviderDescriptor,
    ProviderKind,
};

const DYNAMODB_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::MemoryStore];
const DYNAMODB_DESCRIPTOR: ProviderDescriptor =
    ProviderDescriptor::new("dynamodb", ProviderKind::Storage, DYNAMODB_CAPABILITIES);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamoDbStoreConfig {
    pub endpoint: String,
    pub database: String,
    pub namespace: String,
    pub api_key: Option<String>,
}

impl Default for DynamoDbStoreConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:8000".to_string(),
            database: "mastra".to_string(),
            namespace: "default".to_string(),
            api_key: None,
        }
    }
}

impl DynamoDbStoreConfig {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        ensure_not_blank(&self.endpoint, "endpoint")?;
        ensure_not_blank(&self.database, "database")?;
        ensure_not_blank(&self.namespace, "namespace")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamoDbProvider {
    config: DynamoDbStoreConfig,
}

impl DynamoDbProvider {
    pub fn new(config: DynamoDbStoreConfig) -> Result<Self, ProviderConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &DynamoDbStoreConfig {
        &self.config
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        DYNAMODB_DESCRIPTOR
    }

    pub fn storage_bridge(&self) -> ProviderBridge {
        let mut bridge = ProviderBridge::new(
            DYNAMODB_DESCRIPTOR,
            format!("{}/{}", self.config.database, self.config.namespace),
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
        DynamoDbProvider, DynamoDbStoreConfig, ProviderCapability, ProviderConfigError,
        ProviderKind,
    };

    #[test]
    fn dynamodb_provider_exposes_storage_bridge() {
        let provider = DynamoDbProvider::new(DynamoDbStoreConfig::default())
            .expect("dynamodb config should be valid");

        let descriptor = provider.descriptor();
        let bridge = provider.storage_bridge();

        assert_eq!(descriptor.id, "dynamodb");
        assert_eq!(descriptor.kind, ProviderKind::Storage);
        assert!(bridge.supports(ProviderCapability::MemoryStore));
        assert_eq!(bridge.target, "mastra/default");
    }

    #[test]
    fn dynamodb_provider_rejects_blank_namespace() {
        let error = DynamoDbProvider::new(DynamoDbStoreConfig {
            namespace: String::new(),
            ..DynamoDbStoreConfig::default()
        })
        .expect_err("blank namespace should be rejected");

        assert_eq!(error, ProviderConfigError::EmptyField("namespace"));
    }
}
