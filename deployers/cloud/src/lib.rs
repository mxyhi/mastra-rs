use mastra_packages_deployer::{
    ArtifactKind, BuildArtifact, DeployPlan, Deployer, DeploymentBundle, ProviderKind, Result,
    RuntimeKind,
};
use serde_json::json;

#[derive(Debug, Clone, Default)]
pub struct CloudDeployer {
    studio: bool,
}

impl CloudDeployer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_studio(mut self, studio: bool) -> Self {
        self.studio = studio;
        self
    }
}

impl Deployer for CloudDeployer {
    fn provider(&self) -> ProviderKind {
        ProviderKind::Cloud
    }

    fn build_plan(&self, bundle: &DeploymentBundle) -> Result<DeployPlan> {
        bundle.validate()?;

        let mut artifacts = bundle.staged_artifacts("bundle");
        artifacts.push(BuildArtifact::text(
            "index.mjs",
            ArtifactKind::BundleModule,
            render_cloud_entry(bundle),
        ));
        artifacts.push(BuildArtifact::json(
            "package.json",
            ArtifactKind::Manifest,
            render_cloud_package_manifest(bundle),
        ));

        if self.studio {
            artifacts.push(BuildArtifact::text(
                "studio/index.html",
                ArtifactKind::StaticAsset,
                format!(
                    "<!doctype html>\n<html><head><meta charset=\"utf-8\" /><title>{}</title></head><body><main id=\"app\">Mastra Studio</main></body></html>\n",
                    bundle.app_name
                ),
            ));
        }

        Ok(DeployPlan::new(
            self.provider(),
            "output",
            RuntimeKind::Node,
            "index.mjs",
            artifacts,
        )
        .with_note(
            "Cloud deployer expects the bundle entrypoint to default-export a request handler.",
        ))
    }
}

fn render_cloud_entry(bundle: &DeploymentBundle) -> String {
    let import_path = format!("./bundle/{}", bundle.entrypoint.path);
    let import_line = bundle.entrypoint.import_statement(&import_path, "handler");

    format!(
        "{import_line}\n\nexport default handler;\nexport const metadata = {{ provider: 'cloud', app: '{}' }};\n",
        bundle.app_name
    )
}

fn render_cloud_package_manifest(bundle: &DeploymentBundle) -> serde_json::Value {
    let dependencies = bundle
        .dependencies
        .iter()
        .map(|(name, version)| (name.clone(), json!(version)))
        .collect::<serde_json::Map<_, _>>();

    json!({
        "name": format!("{}-cloud", bundle.app_name),
        "private": true,
        "type": "module",
        "main": "index.mjs",
        "scripts": {
            "start": "node index.mjs"
        },
        "engines": {
            "node": ">=22"
        },
        "dependencies": dependencies,
        "mastra": {
            "environment": bundle.environment
        }
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use mastra_packages_deployer::{
        ArtifactKind, BuildArtifact, BundleEntrypoint, Deployer, DeploymentBundle, RuntimeKind,
    };

    use super::CloudDeployer;

    fn temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("mastra-rs-{label}-{unique}"))
    }

    fn fixture_bundle() -> DeploymentBundle {
        DeploymentBundle::new(
            "demo-app",
            RuntimeKind::Node,
            BundleEntrypoint::default_handler("server/handler.mjs"),
            vec![
                BuildArtifact::text(
                    "server/handler.mjs",
                    ArtifactKind::BundleModule,
                    "export default async function handler(request) { return request; }\n",
                ),
                BuildArtifact::text(
                    "server/tools.mjs",
                    ArtifactKind::BundleModule,
                    "export const tools = [];\n",
                ),
            ],
        )
        .expect("bundle should be valid")
        .with_dependency("mastra-core", "0.1.0")
        .with_dependency("hono", "4.7.2")
        .with_environment("MASTRA_ENV", "production")
    }

    #[test]
    fn cloud_deployer_writes_node_runtime_layout() {
        let output_dir = temp_dir("cloud-deployer");
        let deployer = CloudDeployer::new().with_studio(true);
        let plan = deployer
            .materialize(&fixture_bundle(), &output_dir)
            .expect("cloud plan should materialize");

        assert_eq!(plan.root_dir, "output");
        assert!(output_dir.join("output/index.mjs").exists());
        assert!(output_dir.join("output/package.json").exists());
        assert!(output_dir.join("output/studio/index.html").exists());

        let manifest =
            fs::read_to_string(output_dir.join("output/package.json")).expect("cloud manifest");
        let entry = fs::read_to_string(output_dir.join("output/index.mjs")).expect("cloud entry");

        assert!(manifest.contains("\"name\": \"demo-app-cloud\""));
        assert!(manifest.contains("\"start\": \"node index.mjs\""));
        assert!(entry.contains("export default handler;"));
        assert!(entry.contains("./bundle/server/handler.mjs"));

        fs::remove_dir_all(output_dir).ok();
    }
}
