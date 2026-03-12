use serde_json::{Map, Value};
use std::fmt;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceProviderKind {
    Filesystem,
    BlobStore,
    Sandbox,
}

impl WorkspaceProviderKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Filesystem => "filesystem",
            Self::BlobStore => "blob store",
            Self::Sandbox => "sandbox",
        }
    }
}

impl fmt::Display for WorkspaceProviderKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFieldKind {
    String,
    StringOrObject,
    Boolean,
    Integer,
    Number,
    Object,
    StringMap,
    Array,
    Any,
    Enum(&'static [&'static str]),
}

impl ConfigFieldKind {
    pub const fn expected(self) -> &'static str {
        match self {
            Self::String | Self::Enum(_) => "string",
            Self::StringOrObject => "string or object",
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::Number => "number",
            Self::Object | Self::StringMap => "object",
            Self::Array => "array",
            Self::Any => "any",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfigDefaultValue {
    String(&'static str),
    Boolean(bool),
    Integer(i64),
    Number(f64),
}

impl ConfigDefaultValue {
    fn into_value(self) -> Value {
        match self {
            Self::String(value) => Value::String(value.to_owned()),
            Self::Boolean(value) => Value::Bool(value),
            Self::Integer(value) => Value::Number(value.into()),
            Self::Number(value) => serde_json::Number::from_f64(value)
                .map(Value::Number)
                .unwrap_or(Value::Null),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConfigField {
    pub name: &'static str,
    pub kind: ConfigFieldKind,
    pub required: bool,
    pub description: &'static str,
    pub secret: bool,
    pub default_value: Option<ConfigDefaultValue>,
}

impl ConfigField {
    pub const fn required(
        name: &'static str,
        kind: ConfigFieldKind,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            kind,
            required: true,
            description,
            secret: false,
            default_value: None,
        }
    }

    pub const fn optional(
        name: &'static str,
        kind: ConfigFieldKind,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            kind,
            required: false,
            description,
            secret: false,
            default_value: None,
        }
    }

    pub const fn optional_secret(
        name: &'static str,
        kind: ConfigFieldKind,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            kind,
            required: false,
            description,
            secret: true,
            default_value: None,
        }
    }

    pub const fn required_secret(
        name: &'static str,
        kind: ConfigFieldKind,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            kind,
            required: true,
            description,
            secret: true,
            default_value: None,
        }
    }

    pub const fn optional_with_default(
        name: &'static str,
        kind: ConfigFieldKind,
        description: &'static str,
        default_value: ConfigDefaultValue,
    ) -> Self {
        Self {
            name,
            kind,
            required: false,
            description,
            secret: false,
            default_value: Some(default_value),
        }
    }

    pub const fn optional_enum(
        name: &'static str,
        description: &'static str,
        allowed: &'static [&'static str],
    ) -> Self {
        Self::optional(name, ConfigFieldKind::Enum(allowed), description)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WorkspaceCapabilities {
    pub filesystem: bool,
    pub blob_store: bool,
    pub sandbox: bool,
    pub mounting: bool,
    pub snapshots: bool,
    pub volumes: bool,
    pub network_policies: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorkspaceKindProfile {
    pub kind: WorkspaceProviderKind,
    pub description: &'static str,
    pub config_fields: &'static [ConfigField],
}

impl WorkspaceKindProfile {
    pub const fn new(
        kind: WorkspaceProviderKind,
        description: &'static str,
        config_fields: &'static [ConfigField],
    ) -> Self {
        Self {
            kind,
            description,
            config_fields,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorkspaceProviderProfile {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub env_vars: &'static [&'static str],
    pub kinds: &'static [WorkspaceKindProfile],
    pub capabilities: WorkspaceCapabilities,
}

impl WorkspaceProviderProfile {
    pub fn kind_profile(
        &self,
        kind: WorkspaceProviderKind,
    ) -> Option<&'static WorkspaceKindProfile> {
        self.kinds.iter().find(|profile| profile.kind == kind)
    }

    pub fn supports_kind(&self, kind: WorkspaceProviderKind) -> bool {
        self.kind_profile(kind).is_some()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkspaceProviderError {
    #[error("{provider} does not support {kind}")]
    UnsupportedKind {
        provider: &'static str,
        kind: WorkspaceProviderKind,
    },
    #[error("{provider} config must be a JSON object")]
    InvalidRootType { provider: &'static str },
    #[error("{provider} requires '{field}' for {kind}")]
    MissingRequiredField {
        provider: &'static str,
        kind: WorkspaceProviderKind,
        field: &'static str,
    },
    #[error("{provider} expects '{field}' to be {expected}")]
    InvalidFieldType {
        provider: &'static str,
        field: &'static str,
        expected: &'static str,
    },
    #[error("{provider} expects '{field}' map values to be {expected}")]
    InvalidMapValueType {
        provider: &'static str,
        field: &'static str,
        expected: &'static str,
    },
    #[error("{provider} does not accept '{value}' for '{field}' (allowed: {allowed})")]
    InvalidEnumValue {
        provider: &'static str,
        field: &'static str,
        value: String,
        allowed: String,
    },
}

pub trait WorkspaceProviderAdapter {
    fn profile(&self) -> &'static WorkspaceProviderProfile;

    fn validate_config(
        &self,
        kind: WorkspaceProviderKind,
        config: &Value,
    ) -> Result<Value, WorkspaceProviderError> {
        let profile = self.profile();
        let kind_profile =
            profile
                .kind_profile(kind)
                .ok_or(WorkspaceProviderError::UnsupportedKind {
                    provider: profile.id,
                    kind,
                })?;
        let Value::Object(config_map) = config else {
            return Err(WorkspaceProviderError::InvalidRootType {
                provider: profile.id,
            });
        };

        let mut normalized = config_map.clone();
        for field in kind_profile.config_fields {
            match normalized.get(field.name) {
                Some(value) => validate_field(profile.id, field, value)?,
                None if field.required => {
                    return Err(WorkspaceProviderError::MissingRequiredField {
                        provider: profile.id,
                        kind,
                        field: field.name,
                    });
                }
                None => {
                    if let Some(default_value) = field.default_value {
                        normalized.insert(field.name.to_owned(), default_value.into_value());
                    }
                }
            }
        }

        Ok(Value::Object(normalized))
    }
}

fn validate_field(
    provider: &'static str,
    field: &ConfigField,
    value: &Value,
) -> Result<(), WorkspaceProviderError> {
    match field.kind {
        ConfigFieldKind::String if value.is_string() => Ok(()),
        ConfigFieldKind::StringOrObject if value.is_string() || value.is_object() => Ok(()),
        ConfigFieldKind::Boolean if value.is_boolean() => Ok(()),
        ConfigFieldKind::Integer if value.as_i64().is_some() || value.as_u64().is_some() => Ok(()),
        ConfigFieldKind::Number if value.is_number() => Ok(()),
        ConfigFieldKind::Object if value.is_object() => Ok(()),
        ConfigFieldKind::Array if value.is_array() => Ok(()),
        ConfigFieldKind::Any => Ok(()),
        ConfigFieldKind::StringMap => validate_string_map(provider, field.name, value),
        ConfigFieldKind::Enum(allowed) => validate_enum(provider, field.name, allowed, value),
        _ => Err(WorkspaceProviderError::InvalidFieldType {
            provider,
            field: field.name,
            expected: field.kind.expected(),
        }),
    }
}

fn validate_enum(
    provider: &'static str,
    field: &'static str,
    allowed: &'static [&'static str],
    value: &Value,
) -> Result<(), WorkspaceProviderError> {
    let Some(value) = value.as_str() else {
        return Err(WorkspaceProviderError::InvalidFieldType {
            provider,
            field,
            expected: "string",
        });
    };

    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(WorkspaceProviderError::InvalidEnumValue {
            provider,
            field,
            value: value.to_owned(),
            allowed: allowed.join(", "),
        })
    }
}

fn validate_string_map(
    provider: &'static str,
    field: &'static str,
    value: &Value,
) -> Result<(), WorkspaceProviderError> {
    let Value::Object(entries) = value else {
        return Err(WorkspaceProviderError::InvalidFieldType {
            provider,
            field,
            expected: "object",
        });
    };

    if entries.values().all(Value::is_string) {
        Ok(())
    } else {
        Err(WorkspaceProviderError::InvalidMapValueType {
            provider,
            field,
            expected: "string",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StaticWorkspaceProvider {
    profile: &'static WorkspaceProviderProfile,
}

impl StaticWorkspaceProvider {
    pub const fn new(profile: &'static WorkspaceProviderProfile) -> Self {
        Self { profile }
    }
}

impl WorkspaceProviderAdapter for StaticWorkspaceProvider {
    fn profile(&self) -> &'static WorkspaceProviderProfile {
        self.profile
    }
}

pub fn object(entries: &[(&str, Value)]) -> Value {
    let mut object = Map::with_capacity(entries.len());
    for (key, value) in entries {
        object.insert((*key).to_owned(), value.clone());
    }
    Value::Object(object)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const FILESYSTEM_FIELDS: &[ConfigField] = &[
        ConfigField::required("bucket", ConfigFieldKind::String, "Bucket name"),
        ConfigField::optional_with_default(
            "readOnly",
            ConfigFieldKind::Boolean,
            "Mount as read-only",
            ConfigDefaultValue::Boolean(false),
        ),
    ];

    const PROFILE: WorkspaceProviderProfile = WorkspaceProviderProfile {
        id: "gcs",
        display_name: "Google Cloud Storage",
        description: "Filesystem provider",
        env_vars: &["GOOGLE_APPLICATION_CREDENTIALS"],
        kinds: &[WorkspaceKindProfile::new(
            WorkspaceProviderKind::Filesystem,
            "Filesystem mount",
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

    #[test]
    fn fills_defaults_during_validation() {
        let provider = StaticWorkspaceProvider::new(&PROFILE);

        let normalized = provider
            .validate_config(
                WorkspaceProviderKind::Filesystem,
                &json!({
                    "bucket": "docs",
                }),
            )
            .unwrap();

        assert_eq!(normalized["readOnly"], json!(false));
    }

    #[test]
    fn rejects_missing_required_fields() {
        let provider = StaticWorkspaceProvider::new(&PROFILE);

        let error = provider
            .validate_config(WorkspaceProviderKind::Filesystem, &json!({}))
            .unwrap_err();

        assert_eq!(
            error,
            WorkspaceProviderError::MissingRequiredField {
                provider: "gcs",
                kind: WorkspaceProviderKind::Filesystem,
                field: "bucket",
            }
        );
    }

    #[test]
    fn validates_enum_and_string_map_fields() {
        const SANDBOX_FIELDS: &[ConfigField] = &[
            ConfigField::optional_enum(
                "language",
                "Runtime language",
                &["typescript", "javascript", "python"],
            ),
            ConfigField::optional("env", ConfigFieldKind::StringMap, "Environment variables"),
        ];
        const SANDBOX_PROFILE: WorkspaceProviderProfile = WorkspaceProviderProfile {
            id: "daytona",
            display_name: "Daytona",
            description: "Sandbox provider",
            env_vars: &["DAYTONA_API_KEY"],
            kinds: &[WorkspaceKindProfile::new(
                WorkspaceProviderKind::Sandbox,
                "Cloud sandbox",
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
        let provider = StaticWorkspaceProvider::new(&SANDBOX_PROFILE);

        let enum_error = provider
            .validate_config(
                WorkspaceProviderKind::Sandbox,
                &json!({ "language": "ruby" }),
            )
            .unwrap_err();
        let map_error = provider
            .validate_config(
                WorkspaceProviderKind::Sandbox,
                &json!({ "language": "python", "env": { "DEBUG": true } }),
            )
            .unwrap_err();

        assert_eq!(
            enum_error,
            WorkspaceProviderError::InvalidEnumValue {
                provider: "daytona",
                field: "language",
                value: "ruby".to_owned(),
                allowed: "typescript, javascript, python".to_owned(),
            }
        );
        assert_eq!(
            map_error,
            WorkspaceProviderError::InvalidMapValueType {
                provider: "daytona",
                field: "env",
                expected: "string",
            }
        );
    }
}
