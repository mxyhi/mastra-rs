pub use mastra_workspaces_core::{
    ConfigDefaultValue, ConfigField, ConfigFieldKind, StaticWorkspaceProvider,
    WorkspaceCapabilities, WorkspaceKindProfile, WorkspaceProviderAdapter, WorkspaceProviderError,
    WorkspaceProviderKind, WorkspaceProviderProfile,
};

const SANDBOX_FIELDS: &[ConfigField] = &[
    ConfigField::optional("id", ConfigFieldKind::String, "Sandbox identifier"),
    ConfigField::optional_secret("apiKey", ConfigFieldKind::String, "Daytona API key"),
    ConfigField::optional("apiUrl", ConfigFieldKind::String, "Daytona API URL"),
    ConfigField::optional("target", ConfigFieldKind::String, "Runner region"),
    ConfigField::optional_with_default(
        "timeout",
        ConfigFieldKind::Integer,
        "Execution timeout in milliseconds",
        ConfigDefaultValue::Integer(300_000),
    ),
    ConfigField::optional_enum(
        "language",
        "Runtime language",
        &["typescript", "javascript", "python"],
    ),
    ConfigField::optional("snapshot", ConfigFieldKind::String, "Pre-built snapshot id"),
    ConfigField::optional("image", ConfigFieldKind::String, "Docker image"),
    ConfigField::optional("resources", ConfigFieldKind::Object, "CPU, memory and disk"),
    ConfigField::optional("env", ConfigFieldKind::StringMap, "Environment variables"),
    ConfigField::optional("labels", ConfigFieldKind::StringMap, "Custom labels"),
    ConfigField::optional("name", ConfigFieldKind::String, "Sandbox display name"),
    ConfigField::optional("user", ConfigFieldKind::String, "Sandbox user"),
    ConfigField::optional_with_default(
        "public",
        ConfigFieldKind::Boolean,
        "Expose previews publicly",
        ConfigDefaultValue::Boolean(false),
    ),
    ConfigField::optional_with_default(
        "ephemeral",
        ConfigFieldKind::Boolean,
        "Delete immediately on stop",
        ConfigDefaultValue::Boolean(false),
    ),
    ConfigField::optional_with_default(
        "autoStopInterval",
        ConfigFieldKind::Integer,
        "Auto-stop interval in minutes",
        ConfigDefaultValue::Integer(15),
    ),
    ConfigField::optional(
        "autoArchiveInterval",
        ConfigFieldKind::Integer,
        "Auto-archive interval",
    ),
    ConfigField::optional(
        "autoDeleteInterval",
        ConfigFieldKind::Integer,
        "Auto-delete interval",
    ),
    ConfigField::optional("volumes", ConfigFieldKind::Array, "Volume mounts"),
    ConfigField::optional_with_default(
        "networkBlockAll",
        ConfigFieldKind::Boolean,
        "Block all network access",
        ConfigDefaultValue::Boolean(false),
    ),
    ConfigField::optional(
        "networkAllowList",
        ConfigFieldKind::String,
        "Allowed CIDR list",
    ),
];

pub const PROFILE: WorkspaceProviderProfile = WorkspaceProviderProfile {
    id: "daytona",
    display_name: "Daytona Sandbox",
    description: "Cloud sandbox provider with snapshots, volumes and mount support.",
    env_vars: &["DAYTONA_API_KEY", "DAYTONA_API_URL", "DAYTONA_TARGET"],
    kinds: &[WorkspaceKindProfile::new(
        WorkspaceProviderKind::Sandbox,
        "Daytona cloud sandbox",
        SANDBOX_FIELDS,
    )],
    capabilities: WorkspaceCapabilities {
        filesystem: false,
        blob_store: false,
        sandbox: true,
        mounting: true,
        snapshots: true,
        volumes: true,
        network_policies: true,
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
    use mastra_workspaces_test_utils::{assert_bool_field, validate_err, validate_ok};
    use serde_json::json;

    #[test]
    fn fills_daytona_defaults() {
        let normalized = validate_ok(
            &provider(),
            WorkspaceProviderKind::Sandbox,
            json!({
                "language": "typescript"
            }),
        );

        assert_eq!(normalized["timeout"], json!(300_000));
        assert_eq!(normalized["autoStopInterval"], json!(15));
        assert_bool_field(&normalized, "public", false);
        assert_bool_field(&normalized, "networkBlockAll", false);
    }

    #[test]
    fn rejects_invalid_daytona_language() {
        let error = validate_err(
            &provider(),
            WorkspaceProviderKind::Sandbox,
            json!({
                "language": "rust"
            }),
        );

        assert_eq!(
            error,
            WorkspaceProviderError::InvalidEnumValue {
                provider: "daytona",
                field: "language",
                value: "rust".to_owned(),
                allowed: "typescript, javascript, python".to_owned(),
            }
        );
    }
}
