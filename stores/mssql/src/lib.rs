#[path = "../../_test-utils/src/provider_support.rs"]
mod provider_support;

use provider_support::ensure_not_blank;
pub use provider_support::{
    ProviderBinding, ProviderBridge, ProviderCapability, ProviderConfigError, ProviderDescriptor,
    ProviderKind,
};

const MSSQL_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::MemoryStore];
const MSSQL_DESCRIPTOR: ProviderDescriptor =
    ProviderDescriptor::new("mssql", ProviderKind::Storage, MSSQL_CAPABILITIES);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsSqlStoreConfig {
    pub endpoint: String,
    pub database: String,
    pub namespace: String,
    pub api_key: Option<String>,
}

impl Default for MsSqlStoreConfig {
    fn default() -> Self {
        Self {
            endpoint: "sqlserver://localhost:1433".to_string(),
            database: "mastra".to_string(),
            namespace: "default".to_string(),
            api_key: None,
        }
    }
}

impl MsSqlStoreConfig {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        ensure_not_blank(&self.endpoint, "endpoint")?;
        ensure_not_blank(&self.database, "database")?;
        ensure_not_blank(&self.namespace, "namespace")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsSqlProvider {
    config: MsSqlStoreConfig,
}

impl MsSqlProvider {
    pub fn new(config: MsSqlStoreConfig) -> Result<Self, ProviderConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &MsSqlStoreConfig {
        &self.config
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        MSSQL_DESCRIPTOR
    }

    pub fn storage_bridge(&self) -> ProviderBridge {
        let mut bridge = ProviderBridge::new(
            MSSQL_DESCRIPTOR,
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
        MsSqlProvider, MsSqlStoreConfig, ProviderCapability, ProviderConfigError, ProviderKind,
    };

    #[test]
    fn mssql_provider_exposes_storage_bridge() {
        let provider =
            MsSqlProvider::new(MsSqlStoreConfig::default()).expect("mssql config should be valid");

        let descriptor = provider.descriptor();
        let bridge = provider.storage_bridge();

        assert_eq!(descriptor.id, "mssql");
        assert_eq!(descriptor.kind, ProviderKind::Storage);
        assert!(bridge.supports(ProviderCapability::MemoryStore));
        assert_eq!(bridge.target, "mastra/default");
    }

    #[test]
    fn mssql_provider_rejects_blank_endpoint() {
        let error = MsSqlProvider::new(MsSqlStoreConfig {
            endpoint: " ".into(),
            ..MsSqlStoreConfig::default()
        })
        .expect_err("blank endpoint should be rejected");

        assert_eq!(error, ProviderConfigError::EmptyField("endpoint"));
    }
}
