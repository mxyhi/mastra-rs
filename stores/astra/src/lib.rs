#[path = "../../_test-utils/src/provider_support.rs"]
mod provider_support;

use provider_support::ensure_not_blank;
pub use provider_support::{
    ProviderBinding, ProviderBridge, ProviderCapability, ProviderConfigError, ProviderDescriptor,
    ProviderKind,
};

const ASTRA_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::VectorStore];
const ASTRA_DESCRIPTOR: ProviderDescriptor =
    ProviderDescriptor::new("astra", ProviderKind::Vector, ASTRA_CAPABILITIES);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstraVectorConfig {
    pub endpoint: String,
    pub namespace: String,
    pub index_name: String,
    pub api_key: Option<String>,
}

impl Default for AstraVectorConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://astra.local".to_string(),
            namespace: "default".to_string(),
            index_name: "mastra".to_string(),
            api_key: None,
        }
    }
}

impl AstraVectorConfig {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        ensure_not_blank(&self.endpoint, "endpoint")?;
        ensure_not_blank(&self.namespace, "namespace")?;
        ensure_not_blank(&self.index_name, "index_name")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstraProvider {
    config: AstraVectorConfig,
}

impl AstraProvider {
    pub fn new(config: AstraVectorConfig) -> Result<Self, ProviderConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &AstraVectorConfig {
        &self.config
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        ASTRA_DESCRIPTOR
    }

    pub fn vector_bridge(&self) -> ProviderBridge {
        let mut bridge = ProviderBridge::new(
            ASTRA_DESCRIPTOR,
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
        AstraProvider, AstraVectorConfig, ProviderCapability, ProviderConfigError, ProviderKind,
    };

    #[test]
    fn astra_provider_exposes_vector_bridge() {
        let provider = AstraProvider::new(AstraVectorConfig {
            api_key: Some("super-secret".into()),
            ..AstraVectorConfig::default()
        })
        .expect("astra config should be valid");

        let descriptor = provider.descriptor();
        let bridge = provider.vector_bridge();

        assert_eq!(descriptor.id, "astra");
        assert_eq!(descriptor.kind, ProviderKind::Vector);
        assert!(descriptor.supports(ProviderCapability::VectorStore));
        assert!(bridge.supports(ProviderCapability::VectorStore));
        assert_eq!(bridge.target, "default/mastra");
        assert_eq!(
            bridge.redacted_bindings(),
            vec![
                ("endpoint", "https://astra.local".to_string()),
                ("api_key", "***".to_string()),
            ]
        );
    }

    #[test]
    fn astra_provider_validates_collection_name() {
        let error = AstraProvider::new(AstraVectorConfig {
            index_name: String::new(),
            ..AstraVectorConfig::default()
        })
        .expect_err("empty vector index should be rejected");

        assert_eq!(error, ProviderConfigError::EmptyField("index_name"));
    }
}
