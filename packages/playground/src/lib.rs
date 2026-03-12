use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaygroundRoute {
    pub id: String,
    pub path: String,
    pub label: String,
    pub section: String,
    pub tags: BTreeSet<String>,
}

impl PlaygroundRoute {
    pub fn new(
        id: impl Into<String>,
        path: impl Into<String>,
        label: impl Into<String>,
        section: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            label: label.into(),
            section: section.into(),
            tags: BTreeSet::new(),
        }
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.insert(tag.into().to_ascii_lowercase());
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct PlaygroundManifest {
    routes: Vec<PlaygroundRoute>,
}

impl PlaygroundManifest {
    pub fn new(routes: Vec<PlaygroundRoute>) -> Self {
        Self { routes }
    }

    pub fn sidebar_sections(&self) -> BTreeMap<&str, Vec<&PlaygroundRoute>> {
        let mut sections: BTreeMap<&str, Vec<&PlaygroundRoute>> = BTreeMap::new();
        for route in &self.routes {
            sections
                .entry(route.section.as_str())
                .or_default()
                .push(route);
        }
        sections
    }

    pub fn search(&self, query: &str) -> Vec<&PlaygroundRoute> {
        let query = query.to_ascii_lowercase();
        self.routes
            .iter()
            .filter(|route| {
                route.label.to_ascii_lowercase().contains(&query)
                    || route.path.to_ascii_lowercase().contains(&query)
                    || route.tags.iter().any(|tag| tag.contains(&query))
            })
            .collect()
    }

    pub fn breadcrumbs(&self, path: &str) -> Vec<String> {
        path.trim_matches('/')
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{PlaygroundManifest, PlaygroundRoute};

    #[test]
    fn groups_routes_into_sidebar_sections() {
        let manifest = PlaygroundManifest::new(vec![
            PlaygroundRoute::new("agents", "/agents", "Agents", "build"),
            PlaygroundRoute::new("workflows", "/workflows", "Workflows", "build"),
        ]);

        let sections = manifest.sidebar_sections();

        assert_eq!(sections["build"].len(), 2);
    }

    #[test]
    fn search_matches_tags_and_labels() {
        let manifest = PlaygroundManifest::new(vec![
            PlaygroundRoute::new("observability", "/observability", "Observability", "debug")
                .tag("traces"),
        ]);

        assert_eq!(manifest.search("trace")[0].id, "observability");
        assert_eq!(
            manifest.breadcrumbs("/agents/weather"),
            vec!["agents", "weather"]
        );
    }
}
