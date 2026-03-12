#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiFieldType {
    String,
    Number,
    Boolean,
    Object,
    Array,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiSchemaField {
    pub name: String,
    pub field_type: UiFieldType,
    pub required: bool,
    pub children: Vec<UiSchemaField>,
}

impl UiSchemaField {
    pub fn new(name: impl Into<String>, field_type: UiFieldType, required: bool) -> Self {
        Self {
            name: name.into(),
            field_type,
            required,
            children: Vec::new(),
        }
    }

    pub fn child(mut self, child: UiSchemaField) -> Self {
        self.children.push(child);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandEntry {
    pub id: String,
    pub label: String,
    pub section: String,
    pub keywords: Vec<String>,
}

pub fn collect_variable_paths(fields: &[UiSchemaField]) -> Vec<String> {
    let mut paths = Vec::new();
    for field in fields {
        collect_paths(field, "", &mut paths);
    }
    paths
}

pub fn search_commands<'a>(commands: &'a [CommandEntry], query: &str) -> Vec<&'a CommandEntry> {
    let query = query.to_ascii_lowercase();
    let mut matches = commands
        .iter()
        .filter_map(|command| {
            let mut score = 0;
            if command.label.to_ascii_lowercase().contains(&query) {
                score += 3;
            }
            if command.section.to_ascii_lowercase().contains(&query) {
                score += 1;
            }
            if command
                .keywords
                .iter()
                .any(|keyword| keyword.to_ascii_lowercase().contains(&query))
            {
                score += 2;
            }
            (score > 0).then_some((score, command))
        })
        .collect::<Vec<_>>();

    matches.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.id.cmp(&right.1.id))
    });
    matches.into_iter().map(|(_, command)| command).collect()
}

fn collect_paths(field: &UiSchemaField, prefix: &str, paths: &mut Vec<String>) {
    let current = if prefix.is_empty() {
        field.name.clone()
    } else {
        format!("{prefix}.{}", field.name)
    };

    paths.push(current.clone());
    for child in &field.children {
        collect_paths(child, &current, paths);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CommandEntry, UiFieldType, UiSchemaField, collect_variable_paths, search_commands,
    };

    #[test]
    fn collects_nested_variable_paths() {
        let fields = vec![
            UiSchemaField::new("agent", UiFieldType::Object, true).child(UiSchemaField::new(
                "instructions",
                UiFieldType::String,
                true,
            )),
        ];

        assert_eq!(
            collect_variable_paths(&fields),
            vec!["agent".to_string(), "agent.instructions".to_string()]
        );
    }

    #[test]
    fn command_search_prefers_label_matches() {
        let commands = vec![
            CommandEntry {
                id: "open-traces".into(),
                label: "Open traces".into(),
                section: "observability".into(),
                keywords: vec!["trace".into(), "span".into()],
            },
            CommandEntry {
                id: "open-config".into(),
                label: "Settings".into(),
                section: "configuration".into(),
                keywords: vec!["config".into()],
            },
        ];

        let matches = search_commands(&commands, "trace");

        assert_eq!(matches[0].id, "open-traces");
    }
}
