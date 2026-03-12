#[path = "../../_test-utils/src/provider_support.rs"]
mod provider_support;

use provider_support::ensure_not_blank;
pub use provider_support::{
    ProviderBinding, ProviderBridge, ProviderCapability, ProviderConfigError, ProviderDescriptor,
    ProviderKind,
};

const TURBOPUFFER_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::VectorStore];
const TURBOPUFFER_DESCRIPTOR: ProviderDescriptor = ProviderDescriptor::new(
    "turbopuffer",
    ProviderKind::Vector,
    TURBOPUFFER_CAPABILITIES,
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurbopufferVectorConfig {
    pub endpoint: String,
    pub namespace: String,
    pub index_name: String,
    pub api_key: Option<String>,
}

impl Default for TurbopufferVectorConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://turbopuffer.local".to_string(),
            namespace: "default".to_string(),
            index_name: "mastra".to_string(),
            api_key: None,
        }
    }
}

impl TurbopufferVectorConfig {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        ensure_not_blank(&self.endpoint, "endpoint")?;
        ensure_not_blank(&self.namespace, "namespace")?;
        ensure_not_blank(&self.index_name, "index_name")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurbopufferProvider {
    config: TurbopufferVectorConfig,
}

impl TurbopufferProvider {
    pub fn new(config: TurbopufferVectorConfig) -> Result<Self, ProviderConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &TurbopufferVectorConfig {
        &self.config
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        TURBOPUFFER_DESCRIPTOR
    }

    pub fn vector_bridge(&self) -> ProviderBridge {
        let mut bridge = ProviderBridge::new(
            TURBOPUFFER_DESCRIPTOR,
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
        ProviderCapability, ProviderConfigError, ProviderKind, TurbopufferProvider,
        TurbopufferVectorConfig,
    };

    #[test]
    fn turbopuffer_provider_exposes_vector_bridge() {
        let provider = TurbopufferProvider::new(TurbopufferVectorConfig::default())
            .expect("turbopuffer config should be valid");

        let descriptor = provider.descriptor();
        let bridge = provider.vector_bridge();

        assert_eq!(descriptor.id, "turbopuffer");
        assert_eq!(descriptor.kind, ProviderKind::Vector);
        assert!(bridge.supports(ProviderCapability::VectorStore));
        assert_eq!(bridge.target, "default/mastra");
    }

    #[test]
    fn turbopuffer_provider_rejects_blank_index_name() {
        let error = TurbopufferProvider::new(TurbopufferVectorConfig {
            index_name: String::new(),
            ..TurbopufferVectorConfig::default()
        })
        .expect_err("blank index should be rejected");

        assert_eq!(error, ProviderConfigError::EmptyField("index_name"));
    }
}
