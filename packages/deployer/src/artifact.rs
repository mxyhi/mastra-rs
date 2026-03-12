use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{DeployerError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ArtifactKind {
    BundleModule,
    Config,
    Manifest,
    StaticAsset,
    Stub,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum ArtifactContent {
    Text(String),
    Json(Value),
}

impl ArtifactContent {
    pub fn render(&self) -> Result<String> {
        match self {
            Self::Text(content) => Ok(content.clone()),
            Self::Json(content) => {
                let mut rendered = serde_json::to_string_pretty(content)?;
                rendered.push('\n');
                Ok(rendered)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BuildArtifact {
    pub path: String,
    pub kind: ArtifactKind,
    pub content: ArtifactContent,
}

impl BuildArtifact {
    pub fn text(path: impl Into<String>, kind: ArtifactKind, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            kind,
            content: ArtifactContent::Text(content.into()),
        }
    }

    pub fn json(path: impl Into<String>, kind: ArtifactKind, content: Value) -> Self {
        Self {
            path: path.into(),
            kind,
            content: ArtifactContent::Json(content),
        }
    }

    pub fn rebased(&self, prefix: &str) -> Self {
        let prefix = prefix.trim_matches('/');
        let path = self.path.trim_start_matches('/');
        let rebased_path = if prefix.is_empty() {
            path.to_owned()
        } else if path.is_empty() {
            prefix.to_owned()
        } else {
            format!("{prefix}/{path}")
        };

        Self {
            path: rebased_path,
            kind: self.kind,
            content: self.content.clone(),
        }
    }

    pub fn rendered(&self) -> Result<String> {
        self.content.render()
    }

    pub fn write_to(&self, base_dir: &Path) -> Result<PathBuf> {
        validate_relative_path(&self.path)?;

        let target = base_dir.join(&self.path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&target, self.rendered()?)?;
        Ok(target)
    }
}

pub(crate) fn validate_relative_path(path: &str) -> Result<()> {
    if path.is_empty() {
        return Err(DeployerError::InvalidArtifactPath(path.to_owned()));
    }

    let parsed = Path::new(path);
    if parsed.is_absolute() {
        return Err(DeployerError::InvalidArtifactPath(path.to_owned()));
    }

    for component in parsed.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(DeployerError::InvalidArtifactPath(path.to_owned()));
            }
        }
    }

    Ok(())
}
