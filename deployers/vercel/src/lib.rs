use mastra_packages_deployer::{
    ArtifactKind, BuildArtifact, DeployPlan, Deployer, DeploymentBundle, ProviderKind, Result,
    RuntimeKind,
};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct VercelDeployer {
    studio: bool,
    max_duration_seconds: Option<u16>,
    memory_mb: Option<u16>,
    regions: Vec<String>,
}

impl Default for VercelDeployer {
    fn default() -> Self {
        Self {
            studio: false,
            max_duration_seconds: None,
            memory_mb: None,
            regions: Vec::new(),
        }
    }
}

impl VercelDeployer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_studio(mut self, studio: bool) -> Self {
        self.studio = studio;
        self
    }

    pub fn with_runtime_limits(
        mut self,
        max_duration_seconds: Option<u16>,
        memory_mb: Option<u16>,
    ) -> Self {
        self.max_duration_seconds = max_duration_seconds;
        self.memory_mb = memory_mb;
        self
    }

    pub fn with_regions(mut self, regions: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.regions = regions.into_iter().map(Into::into).collect();
        self
    }
}

impl Deployer for VercelDeployer {
    fn provider(&self) -> ProviderKind {
        ProviderKind::Vercel
    }

    fn build_plan(&self, bundle: &DeploymentBundle) -> Result<DeployPlan> {
        bundle.validate()?;

        let mut artifacts = bundle.staged_artifacts("functions/index.func/bundle");
        artifacts.push(BuildArtifact::text(
            "functions/index.func/index.mjs",
            ArtifactKind::BundleModule,
            render_vercel_entry(bundle),
        ));
        artifacts.push(BuildArtifact::json(
            "functions/index.func/.vc-config.json",
            ArtifactKind::Config,
            build_vc_config(self.max_duration_seconds, self.memory_mb, &self.regions),
        ));

        let routes = if self.studio {
            json!([
                { "src": "/api/(.*)", "dest": "/" },
                { "src": "/health", "dest": "/" },
                { "handle": "filesystem" },
                { "src": "/(.*)", "dest": "/index.html", "check": true }
            ])
        } else {
            json!([{ "src": "/(.*)", "dest": "/" }])
        };

        artifacts.push(BuildArtifact::json(
            "config.json",
            ArtifactKind::Config,
            json!({
                "version": 3,
                "routes": routes,
            }),
        ));

        if self.studio {
            artifacts.push(BuildArtifact::text(
                "static/index.html",
                ArtifactKind::StaticAsset,
                format!(
                    "<!doctype html>\n<html><head><meta charset=\"utf-8\" /><title>{}</title></head><body><div id=\"app\">Mastra Studio</div></body></html>\n",
                    bundle.app_name
                ),
            ));
        }

        Ok(DeployPlan::new(
            self.provider(),
            ".vercel/output",
            RuntimeKind::Node,
            "functions/index.func/index.mjs",
            artifacts,
        )
        .with_note("Vercel plan mirrors Build Output API v3 with a single function entrypoint."))
    }
}

fn render_vercel_entry(bundle: &DeploymentBundle) -> String {
    let import_path = format!("./bundle/{}", bundle.entrypoint.path);
    let import_line = bundle.entrypoint.import_statement(&import_path, "handler");

    format!(
        "{import_line}\n\nexport const GET = handler;\nexport const POST = handler;\nexport const PUT = handler;\nexport const DELETE = handler;\nexport const PATCH = handler;\nexport const OPTIONS = handler;\nexport const HEAD = handler;\n"
    )
}

fn build_vc_config(
    max_duration_seconds: Option<u16>,
    memory_mb: Option<u16>,
    regions: &[String],
) -> serde_json::Value {
    let mut config = json!({
        "handler": "index.mjs",
        "launcherType": "Nodejs",
        "runtime": "nodejs22.x",
        "shouldAddHelpers": true
    });

    if let Some(max_duration_seconds) = max_duration_seconds {
        config["maxDuration"] = json!(max_duration_seconds);
    }

    if let Some(memory_mb) = memory_mb {
        config["memory"] = json!(memory_mb);
    }

    if !regions.is_empty() {
        config["regions"] = json!(regions);
    }

    config
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

    use super::VercelDeployer;

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
    }

    #[test]
    fn vercel_deployer_writes_build_output_api_files() {
        let output_dir = temp_dir("vercel-deployer");
        let deployer = VercelDeployer::new()
            .with_studio(true)
            .with_runtime_limits(Some(30), Some(512))
            .with_regions(["iad1", "hkg1"]);
        let plan = deployer
            .materialize(&fixture_bundle(), &output_dir)
            .expect("vercel plan should materialize");

        assert_eq!(plan.root_dir, ".vercel/output");
        assert!(output_dir.join(".vercel/output/config.json").exists());
        assert!(
            output_dir
                .join(".vercel/output/functions/index.func/index.mjs")
                .exists()
        );
        assert!(
            output_dir
                .join(".vercel/output/functions/index.func/.vc-config.json")
                .exists()
        );

        let config =
            fs::read_to_string(output_dir.join(".vercel/output/config.json")).expect("config");
        let entry =
            fs::read_to_string(output_dir.join(".vercel/output/functions/index.func/index.mjs"))
                .expect("entry");
        let vc_config = fs::read_to_string(
            output_dir.join(".vercel/output/functions/index.func/.vc-config.json"),
        )
        .expect("vc config");

        assert!(config.contains("\"version\": 3"));
        assert!(config.contains("\"src\": \"/api/(.*)\""));
        assert!(entry.contains("export const GET = handler;"));
        assert!(entry.contains("./bundle/server/handler.mjs"));
        assert!(vc_config.contains("\"maxDuration\": 30"));
        assert!(vc_config.contains("\"memory\": 512"));
        assert!(vc_config.contains("\"regions\": ["));

        fs::remove_dir_all(output_dir).ok();
    }
}
