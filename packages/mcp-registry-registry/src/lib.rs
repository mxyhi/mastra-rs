use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: BTreeSet<String>,
}

impl RegistryEntry {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            tags: BTreeSet::new(),
        }
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.insert(tag.into().to_ascii_lowercase());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerEntry {
    pub id: String,
    pub registry_id: String,
    pub title: String,
    pub description: String,
    pub tools: BTreeSet<String>,
    pub tags: BTreeSet<String>,
}

impl ServerEntry {
    pub fn new(
        id: impl Into<String>,
        registry_id: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            registry_id: registry_id.into(),
            title: title.into(),
            description: description.into(),
            tools: BTreeSet::new(),
            tags: BTreeSet::new(),
        }
    }

    pub fn tool(mut self, tool: impl Into<String>) -> Self {
        self.tools.insert(tool.into().to_ascii_lowercase());
        self
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.insert(tag.into().to_ascii_lowercase());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServerFilter {
    pub registry_id: Option<String>,
    pub required_tag: Option<String>,
    pub required_tool: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RegistryCatalog {
    registries: BTreeMap<String, RegistryEntry>,
    servers: Vec<ServerEntry>,
}

impl RegistryCatalog {
    pub fn new<I, J>(registries: I, servers: J) -> Self
    where
        I: IntoIterator<Item = RegistryEntry>,
        J: IntoIterator<Item = ServerEntry>,
    {
        Self {
            registries: registries
                .into_iter()
                .map(|entry| (entry.id.clone(), entry))
                .collect(),
            servers: servers.into_iter().collect(),
        }
    }

    pub fn list_registries(&self) -> Vec<&RegistryEntry> {
        self.registries.values().collect()
    }

    pub fn list_servers(&self, filter: &ServerFilter) -> Vec<&ServerEntry> {
        self.servers
            .iter()
            .filter(|server| match &filter.registry_id {
                Some(registry_id) => &server.registry_id == registry_id,
                None => true,
            })
            .filter(|server| match &filter.required_tag {
                Some(tag) => server.tags.contains(&tag.to_ascii_lowercase()),
                None => true,
            })
            .filter(|server| match &filter.required_tool {
                Some(tool) => server.tools.contains(&tool.to_ascii_lowercase()),
                None => true,
            })
            .filter(|server| match &filter.search {
                Some(search) => {
                    let search = search.to_ascii_lowercase();
                    server.title.to_ascii_lowercase().contains(&search)
                        || server.description.to_ascii_lowercase().contains(&search)
                }
                None => true,
            })
            .collect()
    }

    pub fn registry_counts(&self) -> BTreeMap<&str, usize> {
        let mut counts = BTreeMap::new();
        for server in &self.servers {
            *counts.entry(server.registry_id.as_str()).or_insert(0) += 1;
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::{RegistryCatalog, RegistryEntry, ServerEntry, ServerFilter};

    #[test]
    fn filters_servers_by_registry_tag_and_tool() {
        let catalog = RegistryCatalog::new(
            [RegistryEntry::new(
                "pulse",
                "Pulse",
                "Verified MCP registry",
            )],
            [
                ServerEntry::new("weather", "pulse", "Weather", "Forecast server")
                    .tool("forecast")
                    .tag("verified"),
                ServerEntry::new("search", "pulse", "Search", "Web search").tool("search"),
            ],
        );

        let filter = ServerFilter {
            registry_id: Some("pulse".into()),
            required_tag: Some("verified".into()),
            required_tool: Some("forecast".into()),
            search: None,
        };

        let results = catalog.list_servers(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "weather");
    }

    #[test]
    fn registry_counts_group_servers() {
        let catalog = RegistryCatalog::new(
            [RegistryEntry::new(
                "pulse",
                "Pulse",
                "Verified MCP registry",
            )],
            [
                ServerEntry::new("weather", "pulse", "Weather", "Forecast"),
                ServerEntry::new("search", "pulse", "Search", "Lookup"),
            ],
        );

        assert_eq!(catalog.registry_counts()["pulse"], 2);
    }
}
