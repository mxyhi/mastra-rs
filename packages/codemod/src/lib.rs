use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementRule {
    pub from: String,
    pub to: String,
}

impl ReplacementRule {
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Codemod {
    pub id: String,
    pub description: String,
    pub replacements: Vec<ReplacementRule>,
}

impl Codemod {
    pub fn new(
        id: impl Into<String>,
        description: impl Into<String>,
        replacements: Vec<ReplacementRule>,
    ) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            replacements,
        }
    }

    pub fn apply(&self, source: &str) -> CodemodResult {
        let mut code = source.to_string();
        let mut applied_rules = 0;

        for rule in &self.replacements {
            if code.contains(&rule.from) {
                code = code.replace(&rule.from, &rule.to);
                applied_rules += 1;
            }
        }

        CodemodResult {
            code,
            changed: applied_rules > 0,
            applied_rules,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodemodResult {
    pub code: String,
    pub changed: bool,
    pub applied_rules: usize,
}

#[derive(Debug, Clone, Default)]
pub struct CodemodRegistry {
    codemods: BTreeMap<String, Codemod>,
}

impl CodemodRegistry {
    pub fn register(&mut self, codemod: Codemod) {
        self.codemods.insert(codemod.id.clone(), codemod);
    }

    pub fn get(&self, id: &str) -> Option<&Codemod> {
        self.codemods.get(id)
    }

    pub fn list_version(&self, version_prefix: &str) -> Vec<&Codemod> {
        self.codemods
            .iter()
            .filter(|(id, _)| id.starts_with(version_prefix))
            .map(|(_, codemod)| codemod)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{Codemod, CodemodRegistry, ReplacementRule};

    #[test]
    fn applies_replacements_in_order() {
        let codemod = Codemod::new(
            "v1/mastra-core-imports",
            "Rewrite core imports",
            vec![
                ReplacementRule::new("@mastra/core", "@mastra/core/agent"),
                ReplacementRule::new("generateVNext", "generate"),
            ],
        );

        let result = codemod.apply("import { Agent } from '@mastra/core';\nagent.generateVNext();");

        assert!(result.changed);
        assert_eq!(result.applied_rules, 2);
        assert!(result.code.contains("@mastra/core/agent"));
        assert!(result.code.contains("generate();"));
    }

    #[test]
    fn registry_lists_versioned_codemods() {
        let mut registry = CodemodRegistry::default();
        registry.register(Codemod::new("v1/a", "a", vec![]));
        registry.register(Codemod::new("v2/b", "b", vec![]));

        let listed = registry.list_version("v1/");

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "v1/a");
    }
}
