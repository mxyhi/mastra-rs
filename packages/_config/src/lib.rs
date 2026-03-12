use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigSource {
    Env,
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigEntry {
    pub key: String,
    pub env_key: String,
    pub default: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConfigSchema {
    entries: Vec<ConfigEntry>,
}

impl ConfigSchema {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entry(
        mut self,
        key: impl Into<String>,
        env_key: impl Into<String>,
        default: Option<&str>,
        required: bool,
    ) -> Self {
        self.entries.push(ConfigEntry {
            key: key.into(),
            env_key: env_key.into(),
            default: default.map(ToOwned::to_owned),
            required,
        });
        self
    }

    pub fn resolve(&self, env: &BTreeMap<String, String>) -> Result<ResolvedConfig, ConfigError> {
        let mut values = BTreeMap::new();
        let mut sources = BTreeMap::new();

        for entry in &self.entries {
            if let Some(value) = env.get(&entry.env_key) {
                values.insert(entry.key.clone(), value.clone());
                sources.insert(entry.key.clone(), ConfigSource::Env);
                continue;
            }

            if let Some(default) = &entry.default {
                values.insert(entry.key.clone(), default.clone());
                sources.insert(entry.key.clone(), ConfigSource::Default);
                continue;
            }

            if entry.required {
                return Err(ConfigError::MissingRequiredKey {
                    key: entry.key.clone(),
                    env_key: entry.env_key.clone(),
                });
            }
        }

        Ok(ResolvedConfig { values, sources })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConfig {
    values: BTreeMap<String, String>,
    sources: BTreeMap<String, ConfigSource>,
}

impl ResolvedConfig {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn source(&self, key: &str) -> Option<&ConfigSource> {
        self.sources.get(key)
    }

    pub fn values(&self) -> &BTreeMap<String, String> {
        &self.values
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    MissingRequiredKey { key: String, env_key: String },
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{ConfigError, ConfigSchema, ConfigSource};

    #[test]
    fn resolves_values_from_env_before_defaults() {
        let mut env = BTreeMap::new();
        env.insert("MASTRA_MODEL".into(), "openai/gpt-4o".into());

        let schema = ConfigSchema::new()
            .entry("model", "MASTRA_MODEL", Some("anthropic/claude"), true)
            .entry("mode", "MASTRA_MODE", Some("production"), false);

        let resolved = schema.resolve(&env).expect("config should resolve");

        assert_eq!(resolved.get("model"), Some("openai/gpt-4o"));
        assert_eq!(resolved.source("model"), Some(&ConfigSource::Env));
        assert_eq!(resolved.get("mode"), Some("production"));
        assert_eq!(resolved.source("mode"), Some(&ConfigSource::Default));
    }

    #[test]
    fn reports_missing_required_keys() {
        let schema = ConfigSchema::new().entry("api_key", "MASTRA_API_KEY", None, true);

        let err = schema
            .resolve(&BTreeMap::new())
            .expect_err("required key should be enforced");

        assert_eq!(
            err,
            ConfigError::MissingRequiredKey {
                key: "api_key".into(),
                env_key: "MASTRA_API_KEY".into(),
            }
        );
    }
}
