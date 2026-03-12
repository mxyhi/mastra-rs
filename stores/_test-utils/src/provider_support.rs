use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Storage,
    Vector,
    Hybrid,
    Specialized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderCapability {
    MemoryStore,
    VectorStore,
    KeyValueStore,
    DurableObjectStore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub id: &'static str,
    pub kind: ProviderKind,
    pub capabilities: &'static [ProviderCapability],
}

impl ProviderDescriptor {
    pub const fn new(
        id: &'static str,
        kind: ProviderKind,
        capabilities: &'static [ProviderCapability],
    ) -> Self {
        Self {
            id,
            kind,
            capabilities,
        }
    }

    pub fn supports(&self, capability: ProviderCapability) -> bool {
        self.capabilities.contains(&capability)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderBinding {
    pub key: &'static str,
    pub value: String,
    pub sensitive: bool,
}

impl ProviderBinding {
    pub fn plain(key: &'static str, value: impl Into<String>) -> Self {
        Self {
            key,
            value: value.into(),
            sensitive: false,
        }
    }

    pub fn secret(key: &'static str, value: impl Into<String>) -> Self {
        Self {
            key,
            value: value.into(),
            sensitive: true,
        }
    }

    pub fn redacted(&self) -> (&'static str, String) {
        if self.sensitive {
            if self.value.is_empty() {
                (self.key, String::new())
            } else {
                (self.key, "***".to_string())
            }
        } else {
            (self.key, self.value.clone())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderBridge {
    pub descriptor: ProviderDescriptor,
    pub target: String,
    pub bindings: Vec<ProviderBinding>,
}

impl ProviderBridge {
    pub fn new(descriptor: ProviderDescriptor, target: impl Into<String>) -> Self {
        Self {
            descriptor,
            target: target.into(),
            bindings: Vec::new(),
        }
    }

    pub fn with_binding(mut self, binding: ProviderBinding) -> Self {
        self.bindings.push(binding);
        self
    }

    pub fn supports(&self, capability: ProviderCapability) -> bool {
        self.descriptor.supports(capability)
    }

    pub fn redacted_bindings(&self) -> Vec<(&'static str, String)> {
        self.bindings
            .iter()
            .map(ProviderBinding::redacted)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderConfigError {
    EmptyField(&'static str),
}

impl fmt::Display for ProviderConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField(field) => {
                write!(f, "provider config field `{field}` must not be blank")
            }
        }
    }
}

impl Error for ProviderConfigError {}

pub fn ensure_not_blank(value: &str, field: &'static str) -> Result<(), ProviderConfigError> {
    if value.trim().is_empty() {
        return Err(ProviderConfigError::EmptyField(field));
    }

    Ok(())
}
