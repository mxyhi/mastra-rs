pub use mastra_workspaces_core::{
    ConfigDefaultValue, ConfigField, ConfigFieldKind, StaticWorkspaceProvider,
    WorkspaceCapabilities, WorkspaceKindProfile, WorkspaceProviderAdapter, WorkspaceProviderError,
    WorkspaceProviderKind, WorkspaceProviderProfile,
};

const SANDBOX_FIELDS: &[ConfigField] = &[
    ConfigField::optional("id", ConfigFieldKind::String, "Sandbox identifier"),
    ConfigField::optional_with_default(
        "image",
        ConfigFieldKind::String,
        "Docker image",
        ConfigDefaultValue::String("blaxel/ts-app:latest"),
    ),
    ConfigField::optional_with_default(
        "memory",
        ConfigFieldKind::Integer,
        "Memory allocation in MB",
        ConfigDefaultValue::Integer(4096),
    ),
    ConfigField::optional("timeout", ConfigFieldKind::String, "Sandbox TTL"),
    ConfigField::optional("env", ConfigFieldKind::StringMap, "Environment variables"),
    ConfigField::optional("labels", ConfigFieldKind::StringMap, "Custom labels"),
    ConfigField::optional("runtimes", ConfigFieldKind::Array, "Supported runtimes"),
    ConfigField::optional("ports", ConfigFieldKind::Array, "Exposed ports"),
];

pub const PROFILE: WorkspaceProviderProfile = WorkspaceProviderProfile {
    id: "blaxel",
    display_name: "Blaxel Sandbox",
    description: "Blaxel cloud sandbox provider with mountable cloud storage.",
    env_vars: &["BL_API_KEY", "BL_CLIENT_CREDENTIALS"],
    kinds: &[WorkspaceKindProfile::new(
        WorkspaceProviderKind::Sandbox,
        "Blaxel cloud sandbox",
        SANDBOX_FIELDS,
    )],
    capabilities: WorkspaceCapabilities {
        filesystem: false,
        blob_store: false,
        sandbox: true,
        mounting: true,
        snapshots: false,
        volumes: false,
        network_policies: false,
    },
};

pub const PROVIDER: StaticWorkspaceProvider = StaticWorkspaceProvider::new(&PROFILE);

pub const fn provider() -> StaticWorkspaceProvider {
    StaticWorkspaceProvider::new(&PROFILE)
}

pub fn profile() -> &'static WorkspaceProviderProfile {
    &PROFILE
}

#[cfg(test)]
mod tests {
    use super::*;
    use mastra_workspaces_test_utils::{assert_string_field, validate_err, validate_ok};
    use serde_json::json;

    #[test]
    fn fills_blaxel_defaults() {
        let normalized = validate_ok(
            &provider(),
            WorkspaceProviderKind::Sandbox,
            json!({
                "runtimes": ["node", "python"]
            }),
        );

        assert_string_field(&normalized, "image", "blaxel/ts-app:latest");
        assert_eq!(normalized["memory"], json!(4096));
    }

    #[test]
    fn rejects_invalid_blaxel_env_values() {
        let error = validate_err(
            &provider(),
            WorkspaceProviderKind::Sandbox,
            json!({
                "env": {
                    "DEBUG": 1
                }
            }),
        );

        assert_eq!(
            error,
            WorkspaceProviderError::InvalidMapValueType {
                provider: "blaxel",
                field: "env",
                expected: "string",
            }
        );
    }
}
