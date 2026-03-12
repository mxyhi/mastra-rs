use mastra_packages_deployer::{
    ArtifactKind, BuildArtifact, DeployPlan, Deployer, DeploymentBundle, ProviderKind, Result,
    RuntimeKind,
};
use serde_json::json;

#[derive(Debug, Clone, Default)]
pub struct NetlifyDeployer;

impl NetlifyDeployer {
    pub fn new() -> Self {
        Self
    }
}

impl Deployer for NetlifyDeployer {
    fn provider(&self) -> ProviderKind {
        ProviderKind::Netlify
    }

    fn build_plan(&self, bundle: &DeploymentBundle) -> Result<DeployPlan> {
        bundle.validate()?;

        let mut artifacts = bundle.staged_artifacts("functions/api/bundle");
        artifacts.push(BuildArtifact::text(
            "functions/api/index.mjs",
            ArtifactKind::BundleModule,
            render_netlify_entry(bundle),
        ));
        artifacts.push(BuildArtifact::json(
            "functions/api/package.json",
            ArtifactKind::Manifest,
            render_function_manifest(bundle),
        ));
        artifacts.push(BuildArtifact::json(
            "config.json",
            ArtifactKind::Config,
            json!({
                "functions": {
                    "directory": ".netlify/v1/functions",
                    "node_bundler": "none",
                    "included_files": [".netlify/v1/functions/**"]
                },
                "redirects": [
                    {
                        "force": true,
                        "from": "/*",
                        "to": "/.netlify/functions/api/:splat",
                        "status": 200
                    }
                ]
            }),
        ));

        Ok(DeployPlan::new(
            self.provider(),
            ".netlify/v1",
            RuntimeKind::Node,
            "functions/api/index.mjs",
            artifacts,
        )
        .with_note("Netlify plan disables the platform bundler because mastra-rs pre-stages the bundle tree."))
    }
}

fn render_netlify_entry(bundle: &DeploymentBundle) -> String {
    let import_path = format!("./bundle/{}", bundle.entrypoint.path);
    let import_line = bundle.entrypoint.import_statement(&import_path, "handler");

    format!("{import_line}\n\nexport default handler;\n")
}

fn render_function_manifest(bundle: &DeploymentBundle) -> serde_json::Value {
    let dependencies = bundle
        .dependencies
        .iter()
        .map(|(name, version)| (name.clone(), json!(version)))
        .collect::<serde_json::Map<_, _>>();

    json!({
        "name": format!("{}-netlify-function", bundle.app_name),
        "private": true,
        "type": "module",
        "main": "index.mjs",
        "dependencies": dependencies
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

    use super::NetlifyDeployer;

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
            vec![BuildArtifact::text(
                "server/handler.mjs",
                ArtifactKind::BundleModule,
                "export default async function handler(request) { return request; }\n",
            )],
        )
        .expect("bundle should be valid")
        .with_dependency("@mastra/core", "0.1.0")
        .with_dependency("hono", "4.7.2")
    }

    #[test]
    fn netlify_deployer_writes_framework_manifest_and_function_files() {
        let output_dir = temp_dir("netlify-deployer");
        let deployer = NetlifyDeployer::new();
        let plan = deployer
            .materialize(&fixture_bundle(), &output_dir)
            .expect("netlify plan should materialize");

        assert_eq!(plan.root_dir, ".netlify/v1");
        assert!(output_dir.join(".netlify/v1/config.json").exists());
        assert!(
            output_dir
                .join(".netlify/v1/functions/api/index.mjs")
                .exists()
        );
        assert!(
            output_dir
                .join(".netlify/v1/functions/api/package.json")
                .exists()
        );

        let config =
            fs::read_to_string(output_dir.join(".netlify/v1/config.json")).expect("config");
        let manifest =
            fs::read_to_string(output_dir.join(".netlify/v1/functions/api/package.json"))
                .expect("function package");
        let entry = fs::read_to_string(output_dir.join(".netlify/v1/functions/api/index.mjs"))
            .expect("entry");

        assert!(config.contains("\"node_bundler\": \"none\""));
        assert!(config.contains("\"to\": \"/.netlify/functions/api/:splat\""));
        assert!(manifest.contains("\"name\": \"demo-app-netlify-function\""));
        assert!(manifest.contains("\"@mastra/core\": \"0.1.0\""));
        assert!(entry.contains("export default handler;"));

        fs::remove_dir_all(output_dir).ok();
    }
}
