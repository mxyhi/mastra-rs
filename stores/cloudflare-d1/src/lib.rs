#[path = "../../_test-utils/src/provider_support.rs"]
mod provider_support;

use provider_support::ensure_not_blank;
pub use provider_support::{
    ProviderBinding, ProviderBridge, ProviderCapability, ProviderConfigError, ProviderDescriptor,
    ProviderKind,
};

const CLOUDFLARE_D1_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::MemoryStore];
const CLOUDFLARE_D1_DESCRIPTOR: ProviderDescriptor = ProviderDescriptor::new(
    "cloudflare-d1",
    ProviderKind::Storage,
    CLOUDFLARE_D1_CAPABILITIES,
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudflareD1StoreConfig {
    pub endpoint: String,
    pub database: String,
    pub namespace: String,
    pub api_key: Option<String>,
}

impl Default for CloudflareD1StoreConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://api.cloudflare.com/client/v4/accounts/local/d1".to_string(),
            database: "mastra".to_string(),
            namespace: "default".to_string(),
            api_key: None,
        }
    }
}

impl CloudflareD1StoreConfig {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        ensure_not_blank(&self.endpoint, "endpoint")?;
        ensure_not_blank(&self.database, "database")?;
        ensure_not_blank(&self.namespace, "namespace")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudflareD1Provider {
    config: CloudflareD1StoreConfig,
}

impl CloudflareD1Provider {
    pub fn new(config: CloudflareD1StoreConfig) -> Result<Self, ProviderConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &CloudflareD1StoreConfig {
        &self.config
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        CLOUDFLARE_D1_DESCRIPTOR
    }

    pub fn storage_bridge(&self) -> ProviderBridge {
        let mut bridge = ProviderBridge::new(
            CLOUDFLARE_D1_DESCRIPTOR,
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
        CloudflareD1Provider, CloudflareD1StoreConfig, ProviderCapability, ProviderConfigError,
        ProviderKind,
    };

    #[test]
    fn cloudflare_d1_provider_exposes_storage_bridge() {
        let provider = CloudflareD1Provider::new(CloudflareD1StoreConfig::default())
            .expect("cloudflare d1 config should be valid");

        let descriptor = provider.descriptor();
        let bridge = provider.storage_bridge();

        assert_eq!(descriptor.id, "cloudflare-d1");
        assert_eq!(descriptor.kind, ProviderKind::Storage);
        assert!(bridge.supports(ProviderCapability::MemoryStore));
        assert_eq!(bridge.target, "mastra/default");
    }

    #[test]
    fn cloudflare_d1_provider_rejects_blank_database() {
        let error = CloudflareD1Provider::new(CloudflareD1StoreConfig {
            database: String::new(),
            ..CloudflareD1StoreConfig::default()
        })
        .expect_err("blank database should be rejected");

        assert_eq!(error, ProviderConfigError::EmptyField("database"));
    }
}
