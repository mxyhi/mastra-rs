pub use mastra_workspaces_core::{
    ConfigDefaultValue, ConfigField, ConfigFieldKind, StaticWorkspaceProvider,
    WorkspaceCapabilities, WorkspaceKindProfile, WorkspaceProviderAdapter, WorkspaceProviderError,
    WorkspaceProviderKind, WorkspaceProviderProfile,
};

const SANDBOX_FIELDS: &[ConfigField] = &[
    ConfigField::optional("template", ConfigFieldKind::String, "Sandbox template id"),
    ConfigField::optional_with_default(
        "timeout",
        ConfigFieldKind::Integer,
        "Execution timeout in milliseconds",
        ConfigDefaultValue::Integer(300_000),
    ),
    ConfigField::optional("env", ConfigFieldKind::StringMap, "Environment variables"),
    ConfigField::optional("metadata", ConfigFieldKind::Object, "Custom metadata"),
    ConfigField::optional("domain", ConfigFieldKind::String, "Self-hosted E2B domain"),
    ConfigField::optional("apiUrl", ConfigFieldKind::String, "Self-hosted E2B API URL"),
    ConfigField::optional_secret("apiKey", ConfigFieldKind::String, "E2B API key"),
    ConfigField::optional_secret("accessToken", ConfigFieldKind::String, "E2B access token"),
];

pub const PROFILE: WorkspaceProviderProfile = WorkspaceProviderProfile {
    id: "e2b",
    display_name: "E2B Sandbox",
    description: "Cloud sandbox provider with optional cloud filesystem mounting.",
    env_vars: &[
        "E2B_DOMAIN",
        "E2B_API_URL",
        "E2B_API_KEY",
        "E2B_ACCESS_TOKEN",
    ],
    kinds: &[WorkspaceKindProfile::new(
        WorkspaceProviderKind::Sandbox,
        "E2B cloud sandbox",
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
    use mastra_workspaces_test_utils::{validate_err, validate_ok};
    use serde_json::json;

    #[test]
    fn fills_default_e2b_timeout() {
        let normalized = validate_ok(
            &provider(),
            WorkspaceProviderKind::Sandbox,
            json!({
                "env": {
                    "DEBUG": "1"
                }
            }),
        );

        assert_eq!(normalized["timeout"], json!(300_000));
    }

    #[test]
    fn rejects_non_string_e2b_env_values() {
        let error = validate_err(
            &provider(),
            WorkspaceProviderKind::Sandbox,
            json!({
                "env": {
                    "DEBUG": true
                }
            }),
        );

        assert_eq!(
            error,
            WorkspaceProviderError::InvalidMapValueType {
                provider: "e2b",
                field: "env",
                expected: "string",
            }
        );
    }
}
