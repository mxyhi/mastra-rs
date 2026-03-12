use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    BuildArtifact,
    artifact::validate_relative_path,
    error::{DeployerError, Result},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum RuntimeKind {
    Node,
    Edge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum EntrypointStyle {
    DefaultHandler,
    NamedHandler { export_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BundleEntrypoint {
    pub path: String,
    pub style: EntrypointStyle,
}

impl BundleEntrypoint {
    pub fn default_handler(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            style: EntrypointStyle::DefaultHandler,
        }
    }

    pub fn named_handler(path: impl Into<String>, export_name: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            style: EntrypointStyle::NamedHandler {
                export_name: export_name.into(),
            },
        }
    }

    pub fn import_statement(&self, module_path: &str, binding: &str) -> String {
        match &self.style {
            EntrypointStyle::DefaultHandler => format!("import {binding} from '{module_path}';"),
            EntrypointStyle::NamedHandler { export_name } => {
                format!("import {{ {export_name} as {binding} }} from '{module_path}';")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DeploymentBundle {
    pub app_name: String,
    pub runtime: RuntimeKind,
    pub entrypoint: BundleEntrypoint,
    pub artifacts: Vec<BuildArtifact>,
    pub dependencies: BTreeMap<String, String>,
    pub environment: BTreeMap<String, String>,
}

impl DeploymentBundle {
    pub fn new(
        app_name: impl Into<String>,
        runtime: RuntimeKind,
        entrypoint: BundleEntrypoint,
        artifacts: Vec<BuildArtifact>,
    ) -> Result<Self> {
        let bundle = Self {
            app_name: app_name.into(),
            runtime,
            entrypoint,
            artifacts,
            dependencies: BTreeMap::new(),
            environment: BTreeMap::new(),
        };

        bundle.validate()?;
        Ok(bundle)
    }

    pub fn with_dependency(mut self, name: impl Into<String>, version: impl Into<String>) -> Self {
        self.dependencies.insert(name.into(), version.into());
        self
    }

    pub fn with_environment(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.insert(key.into(), value.into());
        self
    }

    pub fn find_artifact(&self, path: &str) -> Option<&BuildArtifact> {
        self.artifacts.iter().find(|artifact| artifact.path == path)
    }

    pub fn staged_artifacts(&self, prefix: &str) -> Vec<BuildArtifact> {
        self.artifacts
            .iter()
            .cloned()
            .map(|artifact| artifact.rebased(prefix))
            .collect()
    }

    pub fn validate(&self) -> Result<()> {
        validate_relative_path(&self.entrypoint.path)?;

        if self.find_artifact(&self.entrypoint.path).is_none() {
            return Err(DeployerError::MissingEntrypoint(
                self.entrypoint.path.clone(),
            ));
        }

        for artifact in &self.artifacts {
            validate_relative_path(&artifact.path)?;
        }

        Ok(())
    }
}
