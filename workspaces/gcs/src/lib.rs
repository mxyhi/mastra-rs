pub use mastra_workspaces_core::{
    ConfigDefaultValue, ConfigField, ConfigFieldKind, StaticWorkspaceProvider,
    WorkspaceCapabilities, WorkspaceKindProfile, WorkspaceProviderAdapter, WorkspaceProviderError,
    WorkspaceProviderKind, WorkspaceProviderProfile,
};

const FILESYSTEM_FIELDS: &[ConfigField] = &[
    ConfigField::required("bucket", ConfigFieldKind::String, "GCS bucket name"),
    ConfigField::optional(
        "projectId",
        ConfigFieldKind::String,
        "Google Cloud project ID",
    ),
    ConfigField::optional(
        "credentials",
        ConfigFieldKind::StringOrObject,
        "Service account key JSON or key file path",
    ),
    ConfigField::optional("prefix", ConfigFieldKind::String, "Key prefix"),
    ConfigField::optional_with_default(
        "readOnly",
        ConfigFieldKind::Boolean,
        "Mount as read-only",
        ConfigDefaultValue::Boolean(false),
    ),
    ConfigField::optional("endpoint", ConfigFieldKind::String, "Custom API endpoint"),
];

pub const PROFILE: WorkspaceProviderProfile = WorkspaceProviderProfile {
    id: "gcs",
    display_name: "Google Cloud Storage",
    description: "Filesystem provider backed by a GCS bucket.",
    env_vars: &["GOOGLE_APPLICATION_CREDENTIALS", "GOOGLE_CLOUD_PROJECT"],
    kinds: &[WorkspaceKindProfile::new(
        WorkspaceProviderKind::Filesystem,
        "Google Cloud Storage filesystem",
        FILESYSTEM_FIELDS,
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
    fn fills_gcs_defaults_and_accepts_object_credentials() {
        let normalized = validate_ok(
            &provider(),
            WorkspaceProviderKind::Filesystem,
            json!({
                "bucket": "docs",
                "credentials": {
                    "type": "service_account"
                }
            }),
        );

        assert_bool_field(&normalized, "readOnly", false);
    }

    #[test]
    fn rejects_invalid_gcs_credentials_shape() {
        let error = validate_err(
            &provider(),
            WorkspaceProviderKind::Filesystem,
            json!({
                "bucket": "docs",
                "credentials": true
            }),
        );

        assert_eq!(
            error,
            WorkspaceProviderError::InvalidFieldType {
                provider: "gcs",
                field: "credentials",
                expected: "string or object",
            }
        );
    }
}
