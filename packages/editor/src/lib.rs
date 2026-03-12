use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    Equals { key: String, value: String },
    Exists { key: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptStatus {
    Draft,
    Published,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptBlock {
    pub id: String,
    pub content: String,
    pub rules: Vec<Rule>,
    pub status: PromptStatus,
}

impl PromptBlock {
    pub fn new(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
            rules: Vec::new(),
            status: PromptStatus::Published,
        }
    }

    pub fn rule(mut self, rule: Rule) -> Self {
        self.rules.push(rule);
        self
    }

    pub fn status(mut self, status: PromptStatus) -> Self {
        self.status = status;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstructionBlock {
    Text(String),
    PromptBlock(PromptBlock),
    PromptBlockRef(String),
}

#[derive(Debug, Clone, Default)]
pub struct PromptStore {
    blocks: BTreeMap<String, PromptBlock>,
}

impl PromptStore {
    pub fn insert(&mut self, block: PromptBlock) {
        self.blocks.insert(block.id.clone(), block);
    }

    pub fn get(&self, id: &str) -> Option<&PromptBlock> {
        self.blocks.get(id)
    }
}

pub fn evaluate_rules(rules: &[Rule], context: &BTreeMap<String, String>) -> bool {
    rules.iter().all(|rule| match rule {
        Rule::Equals { key, value } => context.get(key) == Some(value),
        Rule::Exists { key } => context.contains_key(key),
    })
}

pub fn render_template(template: &str, context: &BTreeMap<String, String>) -> String {
    let mut rendered = template.to_string();
    for (key, value) in context {
        rendered = rendered.replace(&format!("{{{{{key}}}}}"), value);
    }
    rendered
}

pub fn resolve_instruction_blocks(
    blocks: &[InstructionBlock],
    context: &BTreeMap<String, String>,
    store: &PromptStore,
    include_drafts: bool,
) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            InstructionBlock::Text(text) => Some(render_template(text, context)),
            InstructionBlock::PromptBlock(block) => evaluate_block(block, context, include_drafts),
            InstructionBlock::PromptBlockRef(id) => store
                .get(id)
                .and_then(|block| evaluate_block(block, context, include_drafts)),
        })
        .filter(|segment| !segment.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn evaluate_block(
    block: &PromptBlock,
    context: &BTreeMap<String, String>,
    include_drafts: bool,
) -> Option<String> {
    if !include_drafts && block.status == PromptStatus::Draft {
        return None;
    }

    evaluate_rules(&block.rules, context).then(|| render_template(&block.content, context))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        InstructionBlock, PromptBlock, PromptStatus, PromptStore, Rule, resolve_instruction_blocks,
    };

    #[test]
    fn resolves_text_inline_blocks_and_refs() {
        let mut store = PromptStore::default();
        store.insert(
            PromptBlock::new("greeting", "Hello {{name}}")
                .rule(Rule::Exists { key: "name".into() }),
        );

        let context = BTreeMap::from([("name".into(), "Mastra".into())]);
        let output = resolve_instruction_blocks(
            &[
                InstructionBlock::Text("System ready.".into()),
                InstructionBlock::PromptBlockRef("greeting".into()),
            ],
            &context,
            &store,
            false,
        );

        assert_eq!(output, "System ready.\n\nHello Mastra");
    }

    #[test]
    fn skips_draft_refs_unless_requested() {
        let mut store = PromptStore::default();
        store.insert(PromptBlock::new("draft", "Preview only").status(PromptStatus::Draft));

        let context = BTreeMap::new();

        assert_eq!(
            resolve_instruction_blocks(
                &[InstructionBlock::PromptBlockRef("draft".into())],
                &context,
                &store,
                false,
            ),
            ""
        );

        assert_eq!(
            resolve_instruction_blocks(
                &[InstructionBlock::PromptBlockRef("draft".into())],
                &context,
                &store,
                true,
            ),
            "Preview only"
        );
    }
}
