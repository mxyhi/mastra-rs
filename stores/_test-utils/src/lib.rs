pub mod provider_support;

#[cfg(test)]
mod tests {
    use super::provider_support::{
        ProviderBinding, ProviderBridge, ProviderCapability, ProviderDescriptor, ProviderKind,
    };

    #[test]
    fn provider_bridge_redacts_sensitive_bindings() {
        let bridge = ProviderBridge::new(
            ProviderDescriptor::new(
                "test",
                ProviderKind::Specialized,
                &[ProviderCapability::KeyValueStore],
            ),
            "namespace",
        )
        .with_binding(ProviderBinding::plain("account_id", "account"))
        .with_binding(ProviderBinding::secret("api_token", "secret"));

        assert_eq!(
            bridge.redacted_bindings(),
            vec![
                ("account_id", "account".to_string()),
                ("api_token", "***".to_string()),
            ]
        );
    }
}
