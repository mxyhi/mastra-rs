#[path = "../../_test-utils/src/provider_support.rs"]
mod provider_support;

use provider_support::ensure_not_blank;
pub use provider_support::{
    ProviderBinding, ProviderBridge, ProviderCapability, ProviderConfigError, ProviderDescriptor,
    ProviderKind,
};

const CHROMA_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::VectorStore];
const CHROMA_DESCRIPTOR: ProviderDescriptor =
    ProviderDescriptor::new("chroma", ProviderKind::Vector, CHROMA_CAPABILITIES);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChromaVectorConfig {
    pub endpoint: String,
    pub namespace: String,
    pub index_name: String,
    pub api_key: Option<String>,
}

impl Default for ChromaVectorConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:8000".to_string(),
            namespace: "default".to_string(),
            index_name: "mastra".to_string(),
            api_key: None,
        }
    }
}

impl ChromaVectorConfig {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        ensure_not_blank(&self.endpoint, "endpoint")?;
        ensure_not_blank(&self.namespace, "namespace")?;
        ensure_not_blank(&self.index_name, "index_name")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChromaProvider {
    config: ChromaVectorConfig,
}

impl ChromaProvider {
    pub fn new(config: ChromaVectorConfig) -> Result<Self, ProviderConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &ChromaVectorConfig {
        &self.config
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        CHROMA_DESCRIPTOR
    }

    pub fn vector_bridge(&self) -> ProviderBridge {
        let mut bridge = ProviderBridge::new(
            CHROMA_DESCRIPTOR,
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
        ChromaProvider, ChromaVectorConfig, ProviderCapability, ProviderConfigError, ProviderKind,
    };

    #[test]
    fn chroma_provider_exposes_vector_bridge() {
        let provider = ChromaProvider::new(ChromaVectorConfig {
            api_key: Some("secret".into()),
            ..ChromaVectorConfig::default()
        })
        .expect("chroma config should be valid");

        let descriptor = provider.descriptor();
        let bridge = provider.vector_bridge();

        assert_eq!(descriptor.id, "chroma");
        assert_eq!(descriptor.kind, ProviderKind::Vector);
        assert!(bridge.supports(ProviderCapability::VectorStore));
        assert_eq!(bridge.target, "default/mastra");
        assert_eq!(
            bridge.redacted_bindings(),
            vec![
                ("endpoint", "http://localhost:8000".to_string()),
                ("api_key", "***".to_string()),
            ]
        );
    }

    #[test]
    fn chroma_provider_rejects_blank_endpoint() {
        let error = ChromaProvider::new(ChromaVectorConfig {
            endpoint: " ".into(),
            ..ChromaVectorConfig::default()
        })
        .expect_err("blank endpoint should be rejected");

        assert_eq!(error, ProviderConfigError::EmptyField("endpoint"));
    }
}
