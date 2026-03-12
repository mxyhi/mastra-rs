use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnitKind {
    McpServer,
    Tool,
    Workflow,
    Agent,
    Integration,
    Network,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateUnit {
    pub kind: UnitKind,
    pub id: String,
    pub file: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateManifest {
    pub slug: String,
    pub description: Option<String>,
    pub units: Vec<TemplateUnit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePlan {
    pub slug: String,
    pub commit_sha: String,
    pub template_dir: String,
    pub units: Vec<TemplateUnit>,
}

pub fn plan_merge(
    manifest: &TemplateManifest,
    commit_sha: impl Into<String>,
    template_dir: impl Into<String>,
) -> MergePlan {
    let mut units = manifest.units.clone();
    units.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.id.cmp(&right.id))
    });

    MergePlan {
        slug: manifest.slug.clone(),
        commit_sha: commit_sha.into(),
        template_dir: template_dir.into(),
        units,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentBlueprint {
    pub name: String,
    pub model: String,
    pub instructions: String,
    pub tools: Vec<String>,
    pub workflows: Vec<String>,
    pub integrations: Vec<String>,
    pub mcp_servers: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AgentBuilder {
    name: String,
    model: String,
    instruction_fragments: Vec<String>,
    tools: BTreeSet<String>,
    workflows: BTreeSet<String>,
    integrations: BTreeSet<String>,
    mcp_servers: BTreeSet<String>,
}

impl AgentBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            model: "openai/gpt-4o-mini".into(),
            instruction_fragments: Vec::new(),
            tools: BTreeSet::new(),
            workflows: BTreeSet::new(),
            integrations: BTreeSet::new(),
            mcp_servers: BTreeSet::new(),
        }
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instruction_fragments.push(instructions.into());
        self
    }

    pub fn tool(mut self, tool: impl Into<String>) -> Self {
        self.tools.insert(tool.into());
        self
    }

    pub fn workflow(mut self, workflow: impl Into<String>) -> Self {
        self.workflows.insert(workflow.into());
        self
    }

    pub fn integration(mut self, integration: impl Into<String>) -> Self {
        self.integrations.insert(integration.into());
        self
    }

    pub fn mcp_server(mut self, server: impl Into<String>) -> Self {
        self.mcp_servers.insert(server.into());
        self
    }

    pub fn build(self) -> AgentBlueprint {
        AgentBlueprint {
            name: self.name,
            model: self.model,
            instructions: self.instruction_fragments.join("\n\n"),
            tools: self.tools.into_iter().collect(),
            workflows: self.workflows.into_iter().collect(),
            integrations: self.integrations.into_iter().collect(),
            mcp_servers: self.mcp_servers.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentBuilder, TemplateManifest, TemplateUnit, UnitKind, plan_merge};

    #[test]
    fn plan_merge_orders_units_by_kind_then_id() {
        let manifest = TemplateManifest {
            slug: "weather".into(),
            description: None,
            units: vec![
                TemplateUnit {
                    kind: UnitKind::Workflow,
                    id: "sync".into(),
                    file: "workflows/sync.ts".into(),
                },
                TemplateUnit {
                    kind: UnitKind::Tool,
                    id: "weather".into(),
                    file: "tools/weather.ts".into(),
                },
            ],
        };

        let plan = plan_merge(&manifest, "abc123", "/tmp/template");

        assert_eq!(plan.units[0].kind, UnitKind::Tool);
        assert_eq!(plan.units[1].kind, UnitKind::Workflow);
    }

    #[test]
    fn builder_deduplicates_units_and_joins_instructions() {
        let blueprint = AgentBuilder::new("assistant")
            .model("anthropic/claude-sonnet")
            .instructions("Use tools when needed.")
            .instructions("Return concise answers.")
            .tool("weather")
            .tool("weather")
            .workflow("triage")
            .integration("opencode")
            .mcp_server("docs")
            .build();

        assert_eq!(blueprint.model, "anthropic/claude-sonnet");
        assert_eq!(blueprint.tools, vec!["weather".to_string()]);
        assert_eq!(blueprint.workflows, vec!["triage".to_string()]);
        assert_eq!(
            blueprint.instructions,
            "Use tools when needed.\n\nReturn concise answers."
        );
    }
}
