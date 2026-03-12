use std::collections::BTreeMap;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BumpType {
    Patch,
    Minor,
    Major,
}

impl BumpType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Patch => "patch",
            Self::Minor => "minor",
            Self::Major => "major",
        }
    }
}

impl FromStr for BumpType {
    type Err = ParseChangesetError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "patch" => Ok(Self::Patch),
            "minor" => Ok(Self::Minor),
            "major" => Ok(Self::Major),
            other => Err(ParseChangesetError::InvalidBump(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changeset {
    pub bumps: BTreeMap<String, BumpType>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseChangesetError {
    MissingFrontMatter,
    InvalidEntry(String),
    InvalidBump(String),
}

pub fn parse_changeset(markdown: &str) -> Result<Changeset, ParseChangesetError> {
    let mut lines = markdown.lines();
    if lines.next() != Some("---") {
        return Err(ParseChangesetError::MissingFrontMatter);
    }

    let mut bumps = BTreeMap::new();
    for line in &mut lines {
        if line == "---" {
            let summary = lines.collect::<Vec<_>>().join("\n").trim().to_string();
            return Ok(Changeset { bumps, summary });
        }

        let Some((package, bump)) = line.split_once(':') else {
            return Err(ParseChangesetError::InvalidEntry(line.to_string()));
        };
        let package = package.trim().trim_matches('"').to_string();
        bumps.insert(package, BumpType::from_str(bump.trim())?);
    }

    Err(ParseChangesetError::MissingFrontMatter)
}

pub fn merge_version_bumps<I>(changesets: I) -> BTreeMap<String, BumpType>
where
    I: IntoIterator<Item = Changeset>,
{
    let mut merged = BTreeMap::new();
    for changeset in changesets {
        for (package, bump) in changeset.bumps {
            merged
                .entry(package)
                .and_modify(|current| {
                    if bump > *current {
                        *current = bump;
                    }
                })
                .or_insert(bump);
        }
    }
    merged
}

pub fn render_summary(bumps: &BTreeMap<String, BumpType>) -> String {
    bumps
        .iter()
        .map(|(package, bump)| format!("{package}: {}", bump.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        BumpType, Changeset, ParseChangesetError, merge_version_bumps, parse_changeset,
        render_summary,
    };

    #[test]
    fn parses_changeset_frontmatter_and_summary() {
        let parsed = parse_changeset(
            "---\n\"@mastra/core\": minor\n\"@mastra/evals\": patch\n---\nShip new scorer helpers.\n",
        )
        .expect("changeset should parse");

        assert_eq!(parsed.bumps["@mastra/core"], BumpType::Minor);
        assert_eq!(parsed.bumps["@mastra/evals"], BumpType::Patch);
        assert_eq!(parsed.summary, "Ship new scorer helpers.");
    }

    #[test]
    fn merge_prefers_highest_bump_for_each_package() {
        let merged = merge_version_bumps([
            Changeset {
                bumps: BTreeMap::from([("@mastra/core".into(), BumpType::Patch)]),
                summary: String::new(),
            },
            Changeset {
                bumps: BTreeMap::from([("@mastra/core".into(), BumpType::Major)]),
                summary: String::new(),
            },
        ]);

        assert_eq!(merged["@mastra/core"], BumpType::Major);
    }

    #[test]
    fn render_summary_outputs_sorted_package_lines() {
        let bumps = BTreeMap::from([
            ("@mastra/core".into(), BumpType::Minor),
            ("@mastra/evals".into(), BumpType::Patch),
        ]);

        assert_eq!(
            render_summary(&bumps),
            "@mastra/core: minor\n@mastra/evals: patch"
        );
    }

    #[test]
    fn rejects_invalid_bump_values() {
        let err = parse_changeset("---\n\"pkg\": breaking\n---\nmsg")
            .expect_err("invalid bump should fail");

        assert_eq!(err, ParseChangesetError::InvalidBump("breaking".into()));
    }
}
