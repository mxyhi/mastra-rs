pub use mastra_workspaces_core::{
    ConfigDefaultValue, ConfigField, ConfigFieldKind, StaticWorkspaceProvider,
    WorkspaceCapabilities, WorkspaceKindProfile, WorkspaceProviderAdapter, WorkspaceProviderError,
    WorkspaceProviderKind, WorkspaceProviderProfile,
};
use serde_json::Value;

pub fn validate_ok<P: WorkspaceProviderAdapter>(
    provider: &P,
    kind: WorkspaceProviderKind,
    config: Value,
) -> Value {
    provider.validate_config(kind, &config).unwrap()
}

pub fn validate_err<P: WorkspaceProviderAdapter>(
    provider: &P,
    kind: WorkspaceProviderKind,
    config: Value,
) -> WorkspaceProviderError {
    provider.validate_config(kind, &config).unwrap_err()
}

pub fn assert_bool_field(config: &Value, field: &str, expected: bool) {
    assert_eq!(config.get(field), Some(&Value::Bool(expected)));
}

pub fn assert_string_field(config: &Value, field: &str, expected: &str) {
    assert_eq!(config.get(field), Some(&Value::String(expected.to_owned())));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const FIELDS: &[ConfigField] = &[ConfigField::optional_with_default(
        "enabled",
        ConfigFieldKind::Boolean,
        "Enabled flag",
        ConfigDefaultValue::Boolean(true),
    )];

    const PROFILE: WorkspaceProviderProfile = WorkspaceProviderProfile {
        id: "test",
        display_name: "Test Provider",
        description: "Helper profile",
        env_vars: &[],
        kinds: &[WorkspaceKindProfile::new(
            WorkspaceProviderKind::Filesystem,
            "filesystem",
            FIELDS,
        )],
        capabilities: WorkspaceCapabilities {
            filesystem: true,
            blob_store: false,
            sandbox: false,
            mounting: false,
            snapshots: false,
            volumes: false,
            network_policies: false,
        },
    };

    #[test]
    fn helper_validates_defaults() {
        let provider = StaticWorkspaceProvider::new(&PROFILE);
        let normalized = validate_ok(&provider, WorkspaceProviderKind::Filesystem, json!({}));

        assert_bool_field(&normalized, "enabled", true);
    }
}
