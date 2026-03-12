#[path = "../../_test-utils/src/provider_support.rs"]
mod provider_support;

use provider_support::ensure_not_blank;
pub use provider_support::{
    ProviderBinding, ProviderBridge, ProviderCapability, ProviderConfigError, ProviderDescriptor,
    ProviderKind,
};

const S3VECTORS_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::VectorStore];
const S3VECTORS_DESCRIPTOR: ProviderDescriptor =
    ProviderDescriptor::new("s3vectors", ProviderKind::Vector, S3VECTORS_CAPABILITIES);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3VectorsConfig {
    pub endpoint: String,
    pub namespace: String,
    pub index_name: String,
    pub api_key: Option<String>,
}

impl Default for S3VectorsConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://s3vectors.local".to_string(),
            namespace: "default".to_string(),
            index_name: "mastra".to_string(),
            api_key: None,
        }
    }
}

impl S3VectorsConfig {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        ensure_not_blank(&self.endpoint, "endpoint")?;
        ensure_not_blank(&self.namespace, "namespace")?;
        ensure_not_blank(&self.index_name, "index_name")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3VectorsProvider {
    config: S3VectorsConfig,
}

impl S3VectorsProvider {
    pub fn new(config: S3VectorsConfig) -> Result<Self, ProviderConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &S3VectorsConfig {
        &self.config
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        S3VECTORS_DESCRIPTOR
    }

    pub fn vector_bridge(&self) -> ProviderBridge {
        let mut bridge = ProviderBridge::new(
            S3VECTORS_DESCRIPTOR,
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
        ProviderCapability, ProviderConfigError, ProviderKind, S3VectorsConfig, S3VectorsProvider,
    };

    #[test]
    fn s3vectors_provider_exposes_vector_bridge() {
        let provider = S3VectorsProvider::new(S3VectorsConfig::default())
            .expect("s3vectors config should be valid");

        let descriptor = provider.descriptor();
        let bridge = provider.vector_bridge();

        assert_eq!(descriptor.id, "s3vectors");
        assert_eq!(descriptor.kind, ProviderKind::Vector);
        assert!(bridge.supports(ProviderCapability::VectorStore));
        assert_eq!(bridge.target, "default/mastra");
    }

    #[test]
    fn s3vectors_provider_rejects_blank_endpoint() {
        let error = S3VectorsProvider::new(S3VectorsConfig {
            endpoint: String::new(),
            ..S3VectorsConfig::default()
        })
        .expect_err("blank endpoint should be rejected");

        assert_eq!(error, ProviderConfigError::EmptyField("endpoint"));
    }
}
