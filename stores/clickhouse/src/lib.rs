#[path = "../../_test-utils/src/provider_support.rs"]
mod provider_support;

use provider_support::ensure_not_blank;
pub use provider_support::{
    ProviderBinding, ProviderBridge, ProviderCapability, ProviderConfigError, ProviderDescriptor,
    ProviderKind,
};

const CLICKHOUSE_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::MemoryStore];
const CLICKHOUSE_DESCRIPTOR: ProviderDescriptor =
    ProviderDescriptor::new("clickhouse", ProviderKind::Storage, CLICKHOUSE_CAPABILITIES);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClickhouseStoreConfig {
    pub endpoint: String,
    pub database: String,
    pub namespace: String,
    pub api_key: Option<String>,
}

impl Default for ClickhouseStoreConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:8123".to_string(),
            database: "mastra".to_string(),
            namespace: "default".to_string(),
            api_key: None,
        }
    }
}

impl ClickhouseStoreConfig {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        ensure_not_blank(&self.endpoint, "endpoint")?;
        ensure_not_blank(&self.database, "database")?;
        ensure_not_blank(&self.namespace, "namespace")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClickhouseProvider {
    config: ClickhouseStoreConfig,
}

impl ClickhouseProvider {
    pub fn new(config: ClickhouseStoreConfig) -> Result<Self, ProviderConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &ClickhouseStoreConfig {
        &self.config
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        CLICKHOUSE_DESCRIPTOR
    }

    pub fn storage_bridge(&self) -> ProviderBridge {
        let mut bridge = ProviderBridge::new(
            CLICKHOUSE_DESCRIPTOR,
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
        ClickhouseProvider, ClickhouseStoreConfig, ProviderCapability, ProviderConfigError,
        ProviderKind,
    };

    #[test]
    fn clickhouse_provider_exposes_storage_bridge() {
        let provider = ClickhouseProvider::new(ClickhouseStoreConfig {
            api_key: Some("super-secret".into()),
            ..ClickhouseStoreConfig::default()
        })
        .expect("clickhouse config should be valid");

        let descriptor = provider.descriptor();
        let bridge = provider.storage_bridge();

        assert_eq!(descriptor.id, "clickhouse");
        assert_eq!(descriptor.kind, ProviderKind::Storage);
        assert!(descriptor.supports(ProviderCapability::MemoryStore));
        assert!(bridge.supports(ProviderCapability::MemoryStore));
        assert_eq!(bridge.target, "mastra/default");
        assert_eq!(
            bridge.redacted_bindings(),
            vec![
                ("endpoint", "http://localhost:8123".to_string()),
                ("api_key", "***".to_string()),
            ]
        );
    }

    #[test]
    fn clickhouse_provider_validates_database_name() {
        let error = ClickhouseProvider::new(ClickhouseStoreConfig {
            database: " ".into(),
            ..ClickhouseStoreConfig::default()
        })
        .expect_err("blank database should be rejected");

        assert_eq!(error, ProviderConfigError::EmptyField("database"));
    }
}
