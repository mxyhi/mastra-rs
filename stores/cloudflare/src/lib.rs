#[path = "../../_test-utils/src/provider_support.rs"]
mod provider_support;

use provider_support::ensure_not_blank;
pub use provider_support::{
    ProviderBinding, ProviderBridge, ProviderCapability, ProviderConfigError, ProviderDescriptor,
    ProviderKind,
};

const CLOUDFLARE_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::KeyValueStore,
    ProviderCapability::DurableObjectStore,
];
const CLOUDFLARE_DESCRIPTOR: ProviderDescriptor = ProviderDescriptor::new(
    "cloudflare",
    ProviderKind::Specialized,
    CLOUDFLARE_CAPABILITIES,
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudflareStoreConfig {
    pub account_id: String,
    pub kv_namespace: String,
    pub durable_object: String,
    pub api_token: Option<String>,
}

impl Default for CloudflareStoreConfig {
    fn default() -> Self {
        Self {
            account_id: "account-id".to_string(),
            kv_namespace: "mastra-kv".to_string(),
            durable_object: "MastraCoordinator".to_string(),
            api_token: None,
        }
    }
}

impl CloudflareStoreConfig {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        ensure_not_blank(&self.account_id, "account_id")?;
        ensure_not_blank(&self.kv_namespace, "kv_namespace")?;
        ensure_not_blank(&self.durable_object, "durable_object")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudflareProvider {
    config: CloudflareStoreConfig,
}

impl CloudflareProvider {
    pub fn new(config: CloudflareStoreConfig) -> Result<Self, ProviderConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &CloudflareStoreConfig {
        &self.config
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        CLOUDFLARE_DESCRIPTOR
    }

    pub fn kv_bridge(&self) -> ProviderBridge {
        let mut bridge =
            ProviderBridge::new(CLOUDFLARE_DESCRIPTOR, self.config.kv_namespace.clone())
                .with_binding(ProviderBinding::plain(
                    "account_id",
                    self.config.account_id.clone(),
                ));

        if let Some(api_token) = &self.config.api_token {
            bridge = bridge.with_binding(ProviderBinding::secret("api_token", api_token.clone()));
        }

        bridge
    }

    pub fn durable_object_bridge(&self) -> ProviderBridge {
        let mut bridge =
            ProviderBridge::new(CLOUDFLARE_DESCRIPTOR, self.config.durable_object.clone())
                .with_binding(ProviderBinding::plain(
                    "account_id",
                    self.config.account_id.clone(),
                ));

        if let Some(api_token) = &self.config.api_token {
            bridge = bridge.with_binding(ProviderBinding::secret("api_token", api_token.clone()));
        }

        bridge
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CloudflareProvider, CloudflareStoreConfig, ProviderCapability, ProviderConfigError,
        ProviderKind,
    };

    #[test]
    fn cloudflare_provider_exposes_kv_and_durable_object_bridges() {
        let provider = CloudflareProvider::new(CloudflareStoreConfig {
            api_token: Some("super-secret".into()),
            ..CloudflareStoreConfig::default()
        })
        .expect("cloudflare config should be valid");

        let descriptor = provider.descriptor();
        let kv = provider.kv_bridge();
        let durable_object = provider.durable_object_bridge();

        assert_eq!(descriptor.id, "cloudflare");
        assert_eq!(descriptor.kind, ProviderKind::Specialized);
        assert!(descriptor.supports(ProviderCapability::KeyValueStore));
        assert!(descriptor.supports(ProviderCapability::DurableObjectStore));
        assert!(kv.supports(ProviderCapability::KeyValueStore));
        assert!(durable_object.supports(ProviderCapability::DurableObjectStore));
        assert_eq!(kv.target, "mastra-kv");
        assert_eq!(durable_object.target, "MastraCoordinator");
        assert_eq!(
            kv.redacted_bindings(),
            vec![
                ("account_id", "account-id".to_string()),
                ("api_token", "***".to_string()),
            ]
        );
    }

    #[test]
    fn cloudflare_provider_requires_a_kv_namespace() {
        let error = CloudflareProvider::new(CloudflareStoreConfig {
            kv_namespace: String::new(),
            ..CloudflareStoreConfig::default()
        })
        .expect_err("blank kv namespace should be rejected");

        assert_eq!(error, ProviderConfigError::EmptyField("kv_namespace"));
    }
}
