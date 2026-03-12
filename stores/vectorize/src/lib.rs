#[path = "../../_test-utils/src/provider_support.rs"]
mod provider_support;

use provider_support::ensure_not_blank;
pub use provider_support::{
    ProviderBinding, ProviderBridge, ProviderCapability, ProviderConfigError, ProviderDescriptor,
    ProviderKind,
};

const VECTORIZE_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::VectorStore];
const VECTORIZE_DESCRIPTOR: ProviderDescriptor =
    ProviderDescriptor::new("vectorize", ProviderKind::Vector, VECTORIZE_CAPABILITIES);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorizeConfig {
    pub endpoint: String,
    pub namespace: String,
    pub index_name: String,
    pub api_key: Option<String>,
}

impl Default for VectorizeConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://api.cloudflare.com/client/v4/vectorize".to_string(),
            namespace: "default".to_string(),
            index_name: "mastra".to_string(),
            api_key: None,
        }
    }
}

impl VectorizeConfig {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        ensure_not_blank(&self.endpoint, "endpoint")?;
        ensure_not_blank(&self.namespace, "namespace")?;
        ensure_not_blank(&self.index_name, "index_name")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorizeProvider {
    config: VectorizeConfig,
}

impl VectorizeProvider {
    pub fn new(config: VectorizeConfig) -> Result<Self, ProviderConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &VectorizeConfig {
        &self.config
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        VECTORIZE_DESCRIPTOR
    }

    pub fn vector_bridge(&self) -> ProviderBridge {
        let mut bridge = ProviderBridge::new(
            VECTORIZE_DESCRIPTOR,
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
        ProviderCapability, ProviderConfigError, ProviderKind, VectorizeConfig, VectorizeProvider,
    };

    #[test]
    fn vectorize_provider_exposes_vector_bridge() {
        let provider = VectorizeProvider::new(VectorizeConfig::default())
            .expect("vectorize config should be valid");

        let descriptor = provider.descriptor();
        let bridge = provider.vector_bridge();

        assert_eq!(descriptor.id, "vectorize");
        assert_eq!(descriptor.kind, ProviderKind::Vector);
        assert!(bridge.supports(ProviderCapability::VectorStore));
        assert_eq!(bridge.target, "default/mastra");
    }

    #[test]
    fn vectorize_provider_rejects_blank_namespace() {
        let error = VectorizeProvider::new(VectorizeConfig {
            namespace: String::new(),
            ..VectorizeConfig::default()
        })
        .expect_err("blank namespace should be rejected");

        assert_eq!(error, ProviderConfigError::EmptyField("namespace"));
    }
}
