mod artifact;
mod bundle;
mod error;
mod plan;
mod traits;

pub use artifact::{ArtifactContent, ArtifactKind, BuildArtifact};
pub use bundle::{BundleEntrypoint, DeploymentBundle, EntrypointStyle, RuntimeKind};
pub use error::{DeployerError, Result};
pub use plan::{DeployPlan, ProviderKind};
pub use traits::Deployer;

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;

    use crate::{
        ArtifactKind, BuildArtifact, BundleEntrypoint, DeployPlan, DeploymentBundle, ProviderKind,
        RuntimeKind,
    };

    fn temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("mastra-rs-{label}-{unique}"))
    }

    #[test]
    fn deploy_plan_materialize_writes_text_and_json_artifacts() {
        let output_dir = temp_dir("deployer-plan");
        let plan = DeployPlan::new(
            ProviderKind::Cloud,
            "output",
            RuntimeKind::Node,
            "index.mjs",
            vec![
                BuildArtifact::text(
                    "index.mjs",
                    ArtifactKind::BundleModule,
                    "export default {};\n",
                ),
                BuildArtifact::json(
                    "package.json",
                    ArtifactKind::Manifest,
                    json!({
                        "name": "demo-app",
                        "type": "module",
                    }),
                ),
            ],
        );

        plan.write_to(&output_dir).expect("plan should materialize");

        let entry = fs::read_to_string(output_dir.join("output/index.mjs")).expect("entry file");
        let manifest =
            fs::read_to_string(output_dir.join("output/package.json")).expect("package.json");

        assert_eq!(entry, "export default {};\n");
        assert!(manifest.contains("\"name\": \"demo-app\""));
        assert!(manifest.contains("\"type\": \"module\""));

        fs::remove_dir_all(output_dir).ok();
    }

    #[test]
    fn deployment_bundle_requires_existing_entrypoint() {
        let error = DeploymentBundle::new(
            "demo-app",
            RuntimeKind::Edge,
            BundleEntrypoint::default_handler("server/handler.mjs"),
            vec![BuildArtifact::text(
                "server/not-the-entry.mjs",
                ArtifactKind::BundleModule,
                "export default async function handler() {}\n",
            )],
        )
        .expect_err("bundle should reject missing entrypoint");

        assert!(
            error
                .to_string()
                .contains("deployment bundle entrypoint `server/handler.mjs`")
        );
    }

    #[test]
    fn deploy_plan_rejects_parent_directory_escape() {
        let plan = DeployPlan::new(
            ProviderKind::Cloud,
            "output",
            RuntimeKind::Node,
            "index.mjs",
            vec![BuildArtifact::text(
                "../escape.txt",
                ArtifactKind::StaticAsset,
                "nope",
            )],
        );

        let error = plan
            .write_to(&temp_dir("deployer-invalid-path"))
            .expect_err("plan should reject invalid artifact paths");

        assert!(
            error
                .to_string()
                .contains("artifact path `../escape.txt` must stay within the deployment root")
        );
    }
}
