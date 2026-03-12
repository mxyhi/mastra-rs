use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Option<serde_json::Value>,
    pub output_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum McpPromptRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpPromptArgument {
    pub name: String,
    pub required: bool,
    pub description: Option<String>,
}

impl McpPromptArgument {
    pub fn new(name: impl Into<String>, required: bool) -> Self {
        Self {
            name: name.into(),
            required,
            description: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpPromptMessage {
    pub role: McpPromptRole,
    pub content: String,
}

impl McpPromptMessage {
    pub fn new(role: McpPromptRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new(McpPromptRole::User, content)
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(McpPromptRole::Assistant, content)
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new(McpPromptRole::System, content)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpPrompt {
    pub name: String,
    pub description: String,
    pub arguments: Vec<McpPromptArgument>,
    pub messages: Vec<McpPromptMessage>,
}

impl McpPrompt {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        arguments: Vec<McpPromptArgument>,
        messages: Vec<McpPromptMessage>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            arguments,
            messages,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum McpResourceContents {
    Text { text: String },
    Blob { mime_type: String, data: Vec<u8> },
}

impl McpResourceContents {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    pub fn blob(mime_type: impl Into<String>, data: Vec<u8>) -> Self {
        Self::Blob {
            mime_type: mime_type.into(),
            data,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { text } => Some(text),
            Self::Blob { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub contents: McpResourceContents,
}

impl McpResource {
    pub fn new(
        uri: impl Into<String>,
        name: impl Into<String>,
        contents: McpResourceContents,
    ) -> Self {
        Self {
            uri: uri.into(),
            name: name.into(),
            description: None,
            contents,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn as_text(&self) -> Option<&str> {
        self.contents.as_text()
    }
}
