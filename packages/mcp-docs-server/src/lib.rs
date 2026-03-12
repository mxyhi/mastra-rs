use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocPage {
    pub path: String,
    pub title: String,
    pub body: String,
    pub tags: BTreeSet<String>,
}

impl DocPage {
    pub fn new(path: impl Into<String>, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            title: title.into(),
            body: body.into(),
            tags: BTreeSet::new(),
        }
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.insert(tag.into().to_ascii_lowercase());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocMatch {
    pub path: String,
    pub score: usize,
}

#[derive(Debug, Clone, Default)]
pub struct DocsIndex {
    pages: BTreeMap<String, DocPage>,
}

impl DocsIndex {
    pub fn new<I>(pages: I) -> Self
    where
        I: IntoIterator<Item = DocPage>,
    {
        let pages = pages
            .into_iter()
            .map(|page| (page.path.clone(), page))
            .collect();
        Self { pages }
    }

    pub fn get(&self, path: &str) -> Option<&DocPage> {
        self.pages.get(path)
    }

    pub fn child_paths(&self, prefix: &str) -> Vec<&str> {
        self.pages
            .keys()
            .filter(|path| path.starts_with(prefix))
            .map(String::as_str)
            .collect()
    }

    pub fn search(&self, query: &str) -> Vec<DocMatch> {
        let tokens = tokenize(query);
        let mut matches = self
            .pages
            .values()
            .filter_map(|page| {
                let mut score = 0;
                for token in &tokens {
                    if page.path.to_ascii_lowercase().contains(token) {
                        score += 5;
                    }
                    if page.title.to_ascii_lowercase().contains(token) {
                        score += 3;
                    }
                    if page
                        .tags
                        .iter()
                        .any(|tag| tag.to_ascii_lowercase().contains(token))
                    {
                        score += 2;
                    }
                    if page.body.to_ascii_lowercase().contains(token) {
                        score += 1;
                    }
                }

                (score > 0).then(|| DocMatch {
                    path: page.path.clone(),
                    score,
                })
            })
            .collect::<Vec<_>>();

        matches.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.path.cmp(&right.path))
        });
        matches
    }
}

fn tokenize(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|token| token.to_ascii_lowercase())
        .filter(|token| !token.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{DocPage, DocsIndex};

    #[test]
    fn search_prefers_path_and_title_hits() {
        let index = DocsIndex::new([
            DocPage::new(
                "reference/workflows/run",
                "Run a workflow",
                "Detailed workflow execution reference.",
            )
            .tag("workflow"),
            DocPage::new(
                "guides/memory",
                "Observational memory workflow guide",
                "How to run memory reflection after each workflow.",
            )
            .tag("memory"),
        ]);

        let results = index.search("workflow run");

        assert_eq!(results[0].path, "reference/workflows/run");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn child_paths_return_matching_prefixes() {
        let index = DocsIndex::new([
            DocPage::new("reference/agents/create", "Create agent", "..."),
            DocPage::new("reference/agents/run", "Run agent", "..."),
            DocPage::new("guides/agents/overview", "Guide", "..."),
        ]);

        assert_eq!(
            index.child_paths("reference/agents"),
            vec!["reference/agents/create", "reference/agents/run"]
        );
    }
}
