use mastra_packages_deployer::{
    ArtifactKind, BuildArtifact, DeployPlan, Deployer, DeploymentBundle, ProviderKind, Result,
    RuntimeKind,
};
use serde_json::json;

const DEFAULT_COMPATIBILITY_DATE: &str = "2025-04-01";

#[derive(Debug, Clone, Default)]
pub struct CloudflareDeployer {
    worker_name: Option<String>,
}

impl CloudflareDeployer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_worker_name(mut self, worker_name: impl Into<String>) -> Self {
        self.worker_name = Some(worker_name.into());
        self
    }
}

impl Deployer for CloudflareDeployer {
    fn provider(&self) -> ProviderKind {
        ProviderKind::Cloudflare
    }

    fn build_plan(&self, bundle: &DeploymentBundle) -> Result<DeployPlan> {
        bundle.validate()?;

        let wrangler = render_wrangler_json(bundle, self.worker_name.as_deref());
        let mut artifacts = bundle.staged_artifacts("bundle");
        artifacts.push(BuildArtifact::text(
            "index.mjs",
            ArtifactKind::BundleModule,
            render_worker_entry(bundle),
        ));
        artifacts.push(BuildArtifact::json(
            "wrangler.json",
            ArtifactKind::Config,
            wrangler.clone(),
        ));
        artifacts.push(BuildArtifact::text(
            "wrangler.jsonc",
            ArtifactKind::Config,
            render_wrangler_jsonc(&wrangler),
        ));
        artifacts.push(BuildArtifact::text(
            "typescript-stub.mjs",
            ArtifactKind::Stub,
            typescript_stub(),
        ));
        artifacts.push(BuildArtifact::text(
            "execa-stub.mjs",
            ArtifactKind::Stub,
            execa_stub(),
        ));

        Ok(DeployPlan::new(
            self.provider(),
            "output",
            RuntimeKind::Edge,
            "index.mjs",
            artifacts,
        )
        .with_note("Cloudflare plan emits wrangler.json plus runtime stubs for packages that cannot execute on Workers."))
    }
}

fn render_worker_entry(bundle: &DeploymentBundle) -> String {
    let import_path = format!("./bundle/{}", bundle.entrypoint.path);
    let import_line = bundle.entrypoint.import_statement(&import_path, "handler");

    format!("{import_line}\n\nexport default {{\n  fetch: handler,\n}};\n")
}

fn render_wrangler_json(bundle: &DeploymentBundle, worker_name: Option<&str>) -> serde_json::Value {
    json!({
        "name": worker_name.unwrap_or(&bundle.app_name),
        "main": "./index.mjs",
        "compatibility_date": DEFAULT_COMPATIBILITY_DATE,
        "compatibility_flags": ["nodejs_compat", "nodejs_compat_populate_process_env"],
        "observability": {
            "logs": {
                "enabled": true
            }
        },
        "vars": bundle.environment,
        "alias": {
            "typescript": "./typescript-stub.mjs",
            "execa": "./execa-stub.mjs"
        }
    })
}

fn render_wrangler_jsonc(json: &serde_json::Value) -> String {
    format!(
        "/* This file was auto-generated through mastra-rs. Edit the CloudflareDeployer configuration instead. */\n{}\n",
        serde_json::to_string_pretty(json).expect("wrangler json should serialize")
    )
}

fn typescript_stub() -> &'static str {
    "export default {};\nexport const createSourceFile = () => null;\nexport const createProgram = () => null;\n"
}

fn execa_stub() -> &'static str {
    "export const execa = () => { throw new Error('execa is not available in Cloudflare Workers'); };\nexport const execaNode = execa;\n"
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

    use super::CloudflareDeployer;

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
            RuntimeKind::Edge,
            BundleEntrypoint::default_handler("server/handler.mjs"),
            vec![BuildArtifact::text(
                "server/handler.mjs",
                ArtifactKind::BundleModule,
                "export default async function handler(request) { return request; }\n",
            )],
        )
        .expect("bundle should be valid")
        .with_environment("MASTRA_ENV", "production")
    }

    #[test]
    fn cloudflare_deployer_writes_worker_layout_and_stubs() {
        let output_dir = temp_dir("cloudflare-deployer");
        let deployer = CloudflareDeployer::new().with_worker_name("edge-demo");
        let plan = deployer
            .materialize(&fixture_bundle(), &output_dir)
            .expect("cloudflare plan should materialize");

        assert_eq!(plan.root_dir, "output");
        assert!(output_dir.join("output/index.mjs").exists());
        assert!(output_dir.join("output/wrangler.json").exists());
        assert!(output_dir.join("output/wrangler.jsonc").exists());
        assert!(output_dir.join("output/typescript-stub.mjs").exists());
        assert!(output_dir.join("output/execa-stub.mjs").exists());

        let wrangler =
            fs::read_to_string(output_dir.join("output/wrangler.json")).expect("wrangler json");
        let wrangler_jsonc =
            fs::read_to_string(output_dir.join("output/wrangler.jsonc")).expect("wrangler jsonc");
        let entry = fs::read_to_string(output_dir.join("output/index.mjs")).expect("entry");

        assert!(wrangler.contains("\"name\": \"edge-demo\""));
        assert!(wrangler.contains("\"compatibility_flags\": ["));
        assert!(wrangler.contains("\"MASTRA_ENV\": \"production\""));
        assert!(wrangler_jsonc.contains("auto-generated through mastra-rs"));
        assert!(entry.contains("fetch: handler"));

        fs::remove_dir_all(output_dir).ok();
    }
}
