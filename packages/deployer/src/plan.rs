use std::{
    fs,
    path::{Path, PathBuf},
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{BuildArtifact, RuntimeKind, artifact::validate_relative_path, error::Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ProviderKind {
    Cloud,
    Vercel,
    Netlify,
    Cloudflare,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DeployPlan {
    pub provider: ProviderKind,
    pub root_dir: String,
    pub runtime: RuntimeKind,
    pub entrypoint: String,
    pub artifacts: Vec<BuildArtifact>,
    pub notes: Vec<String>,
}

impl DeployPlan {
    pub fn new(
        provider: ProviderKind,
        root_dir: impl Into<String>,
        runtime: RuntimeKind,
        entrypoint: impl Into<String>,
        artifacts: Vec<BuildArtifact>,
    ) -> Self {
        Self {
            provider,
            root_dir: root_dir.into(),
            runtime,
            entrypoint: entrypoint.into(),
            artifacts,
            notes: Vec::new(),
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn find_artifact(&self, path: &str) -> Option<&BuildArtifact> {
        self.artifacts.iter().find(|artifact| artifact.path == path)
    }

    pub fn write_to(&self, output_dir: &Path) -> Result<Vec<PathBuf>> {
        validate_relative_path(&self.root_dir)?;
        validate_relative_path(&self.entrypoint)?;

        let plan_root = output_dir.join(&self.root_dir);
        fs::create_dir_all(&plan_root)?;
        let mut written = Vec::with_capacity(self.artifacts.len());

        for artifact in &self.artifacts {
            written.push(artifact.write_to(&plan_root)?);
        }

        Ok(written)
    }
}
