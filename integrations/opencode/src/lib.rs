use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MastraOmPluginConfig {
    pub model: Option<String>,
    pub storage_path: Option<PathBuf>,
    pub observation_message_tokens: usize,
    pub reflection_observation_tokens: usize,
}

impl Default for MastraOmPluginConfig {
    fn default() -> Self {
        Self {
            model: None,
            storage_path: None,
            observation_message_tokens: 20_000,
            reflection_observation_tokens: 90_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPluginConfig {
    pub model: Option<String>,
    pub storage_path: PathBuf,
    pub observation_message_tokens: usize,
    pub reflection_observation_tokens: usize,
}

impl MastraOmPluginConfig {
    pub fn resolve(&self, project_root: impl AsRef<Path>) -> ResolvedPluginConfig {
        let project_root = project_root.as_ref();
        let storage_path = self
            .storage_path
            .clone()
            .unwrap_or_else(|| PathBuf::from(".opencode/memory/observations.db"));

        let storage_path = if storage_path.is_absolute() {
            storage_path
        } else {
            project_root.join(storage_path)
        };

        ResolvedPluginConfig {
            model: self.model.clone(),
            storage_path,
            observation_message_tokens: self.observation_message_tokens,
            reflection_observation_tokens: self.reflection_observation_tokens,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenCodePart {
    Text(String),
    ToolInvocation {
        tool_name: String,
        args: String,
        result: Option<String>,
    },
    File {
        url: String,
        media_type: String,
    },
    Reasoning(String),
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenCodeMessage {
    pub id: String,
    pub role: MessageRole,
    pub created_at_ms: u64,
    pub parts: Vec<OpenCodePart>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryPart {
    Text(String),
    ToolInvocation {
        tool_name: String,
        args: String,
        result: Option<String>,
    },
    File {
        url: String,
        media_type: String,
    },
    Reasoning(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryMessage {
    pub id: String,
    pub role: MessageRole,
    pub thread_id: String,
    pub resource_id: String,
    pub created_at_ms: u64,
    pub parts: Vec<MemoryPart>,
}

pub fn convert_messages(messages: &[OpenCodeMessage], session_id: &str) -> Vec<MemoryMessage> {
    messages
        .iter()
        .filter(|message| message.role != MessageRole::System)
        .filter_map(|message| {
            let parts = message
                .parts
                .iter()
                .filter_map(|part| match part {
                    OpenCodePart::Text(text) => Some(MemoryPart::Text(text.clone())),
                    OpenCodePart::ToolInvocation {
                        tool_name,
                        args,
                        result,
                    } => Some(MemoryPart::ToolInvocation {
                        tool_name: tool_name.clone(),
                        args: args.clone(),
                        result: result.clone(),
                    }),
                    OpenCodePart::File { url, media_type } => Some(MemoryPart::File {
                        url: url.clone(),
                        media_type: media_type.clone(),
                    }),
                    OpenCodePart::Reasoning(reasoning) => {
                        Some(MemoryPart::Reasoning(reasoning.clone()))
                    }
                    OpenCodePart::Unsupported(_) => None,
                })
                .collect::<Vec<_>>();

            if parts.is_empty() {
                return None;
            }

            Some(MemoryMessage {
                id: message.id.clone(),
                role: message.role.clone(),
                thread_id: session_id.to_string(),
                resource_id: session_id.to_string(),
                created_at_ms: message.created_at_ms,
                parts,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        MastraOmPluginConfig, MemoryPart, MessageRole, OpenCodeMessage, OpenCodePart,
        convert_messages,
    };

    #[test]
    fn resolves_default_storage_under_project_root() {
        let resolved = MastraOmPluginConfig::default().resolve("/repo/project");

        assert_eq!(
            resolved.storage_path,
            PathBuf::from("/repo/project/.opencode/memory/observations.db")
        );
        assert_eq!(resolved.observation_message_tokens, 20_000);
    }

    #[test]
    fn converts_supported_parts_and_filters_system_messages() {
        let messages = vec![
            OpenCodeMessage {
                id: "1".into(),
                role: MessageRole::User,
                created_at_ms: 1,
                parts: vec![
                    OpenCodePart::Text("hello".into()),
                    OpenCodePart::ToolInvocation {
                        tool_name: "search".into(),
                        args: "{}".into(),
                        result: Some("{\"ok\":true}".into()),
                    },
                ],
            },
            OpenCodeMessage {
                id: "2".into(),
                role: MessageRole::System,
                created_at_ms: 2,
                parts: vec![OpenCodePart::Text("ignore".into())],
            },
        ];

        let converted = convert_messages(&messages, "session-1");

        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].thread_id, "session-1");
        assert_eq!(
            converted[0].parts,
            vec![
                MemoryPart::Text("hello".into()),
                MemoryPart::ToolInvocation {
                    tool_name: "search".into(),
                    args: "{}".into(),
                    result: Some("{\"ok\":true}".into()),
                },
            ]
        );
    }
}
