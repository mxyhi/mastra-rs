pub use mastra_workspaces_core::{
    ConfigDefaultValue, ConfigField, ConfigFieldKind, StaticWorkspaceProvider,
    WorkspaceCapabilities, WorkspaceKindProfile, WorkspaceProviderAdapter, WorkspaceProviderError,
    WorkspaceProviderKind, WorkspaceProviderProfile,
};

const FILESYSTEM_FIELDS: &[ConfigField] = &[
    ConfigField::required("bucket", ConfigFieldKind::String, "S3 bucket name"),
    ConfigField::required("region", ConfigFieldKind::String, "AWS region or auto"),
    ConfigField::optional_secret("accessKeyId", ConfigFieldKind::String, "Access key id"),
    ConfigField::optional_secret(
        "secretAccessKey",
        ConfigFieldKind::String,
        "Secret access key",
    ),
    ConfigField::optional_secret("sessionToken", ConfigFieldKind::String, "Session token"),
    ConfigField::optional("endpoint", ConfigFieldKind::String, "Custom S3 endpoint"),
    ConfigField::optional_with_default(
        "forcePathStyle",
        ConfigFieldKind::Boolean,
        "Force path-style URLs",
        ConfigDefaultValue::Boolean(false),
    ),
    ConfigField::optional("prefix", ConfigFieldKind::String, "Key prefix"),
    ConfigField::optional_with_default(
        "readOnly",
        ConfigFieldKind::Boolean,
        "Mount as read-only",
        ConfigDefaultValue::Boolean(false),
    ),
];

const BLOB_STORE_FIELDS: &[ConfigField] = &[
    ConfigField::required("bucket", ConfigFieldKind::String, "S3 bucket name"),
    ConfigField::required("region", ConfigFieldKind::String, "AWS region or auto"),
    ConfigField::required_secret("accessKeyId", ConfigFieldKind::String, "Access key id"),
    ConfigField::required_secret(
        "secretAccessKey",
        ConfigFieldKind::String,
        "Secret access key",
    ),
    ConfigField::optional_secret("sessionToken", ConfigFieldKind::String, "Session token"),
    ConfigField::optional("endpoint", ConfigFieldKind::String, "Custom S3 endpoint"),
    ConfigField::optional_with_default(
        "forcePathStyle",
        ConfigFieldKind::Boolean,
        "Force path-style URLs",
        ConfigDefaultValue::Boolean(false),
    ),
    ConfigField::optional_with_default(
        "prefix",
        ConfigFieldKind::String,
        "Blob key prefix",
        ConfigDefaultValue::String("mastra_skill_blobs/"),
    ),
];

pub const PROFILE: WorkspaceProviderProfile = WorkspaceProviderProfile {
    id: "s3",
    display_name: "Amazon S3",
    description: "Filesystem and blob-store provider for S3-compatible object storage.",
    env_vars: &[
        "AWS_ACCESS_KEY_ID",
        "AWS_SECRET_ACCESS_KEY",
        "AWS_SESSION_TOKEN",
    ],
    kinds: &[
        WorkspaceKindProfile::new(
            WorkspaceProviderKind::Filesystem,
            "S3 filesystem mount",
            FILESYSTEM_FIELDS,
        ),
        WorkspaceKindProfile::new(
            WorkspaceProviderKind::BlobStore,
            "S3 blob store",
            BLOB_STORE_FIELDS,
        ),
    ],
    capabilities: WorkspaceCapabilities {
        filesystem: true,
        blob_store: true,
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
    use mastra_workspaces_test_utils::{
        assert_bool_field, assert_string_field, validate_err, validate_ok,
    };
    use serde_json::json;

    #[test]
    fn fills_filesystem_defaults() {
        let normalized = validate_ok(
            &provider(),
            WorkspaceProviderKind::Filesystem,
            json!({
                "bucket": "artifacts",
                "region": "auto"
            }),
        );

        assert_bool_field(&normalized, "forcePathStyle", false);
        assert_bool_field(&normalized, "readOnly", false);
    }

    #[test]
    fn blob_store_requires_credentials() {
        let error = validate_err(
            &provider(),
            WorkspaceProviderKind::BlobStore,
            json!({
                "bucket": "artifacts",
                "region": "us-east-1"
            }),
        );

        assert_eq!(
            error,
            WorkspaceProviderError::MissingRequiredField {
                provider: "s3",
                kind: WorkspaceProviderKind::BlobStore,
                field: "accessKeyId",
            }
        );
    }

    #[test]
    fn fills_blob_store_prefix_default() {
        let normalized = validate_ok(
            &provider(),
            WorkspaceProviderKind::BlobStore,
            json!({
                "bucket": "artifacts",
                "region": "us-east-1",
                "accessKeyId": "key",
                "secretAccessKey": "secret"
            }),
        );

        assert_string_field(&normalized, "prefix", "mastra_skill_blobs/");
    }
}
