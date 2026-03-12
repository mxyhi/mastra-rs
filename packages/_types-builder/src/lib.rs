use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteOutput {
    pub code: String,
    pub modified_imports: usize,
    pub bundled_packages: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeImportRewriter {
    bundled_packages: BTreeSet<String>,
    known_directories: BTreeSet<String>,
}

impl TypeImportRewriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bundle_package(mut self, package: impl Into<String>) -> Self {
        self.bundled_packages.insert(package.into());
        self
    }

    pub fn known_directory(mut self, path: impl Into<String>) -> Self {
        self.known_directories.insert(path.into());
        self
    }

    pub fn rewrite(&self, source: &str) -> RewriteOutput {
        let mut modified_imports = 0;
        let mut bundled_packages = BTreeSet::new();
        let mut output = Vec::new();

        for line in source.lines() {
            let (rewritten, modified, bundled) =
                rewrite_line(line, &self.known_directories, &self.bundled_packages);
            if modified {
                modified_imports += 1;
            }
            if let Some(package) = bundled {
                bundled_packages.insert(package);
            }
            output.push(rewritten);
        }

        RewriteOutput {
            code: output.join("\n"),
            modified_imports,
            bundled_packages,
        }
    }
}

fn rewrite_line(
    line: &str,
    known_directories: &BTreeSet<String>,
    bundled_packages: &BTreeSet<String>,
) -> (String, bool, Option<String>) {
    let Some((prefix, quote, specifier, suffix)) = extract_specifier(line) else {
        return (line.to_string(), false, None);
    };

    let (next_specifier, bundled) = if specifier.starts_with("./") || specifier.starts_with("../") {
        if specifier.ends_with(".js") {
            (specifier.clone(), None)
        } else if known_directories.contains(&specifier) {
            (format!("{specifier}/index.js"), None)
        } else {
            (format!("{specifier}.js"), None)
        }
    } else if bundled_packages.contains(&specifier) {
        (bundle_import_path(&specifier), Some(specifier.clone()))
    } else {
        (specifier.clone(), None)
    };

    let modified = next_specifier != specifier;
    (
        format!("{prefix}{next_specifier}{quote}{suffix}"),
        modified,
        bundled,
    )
}

fn extract_specifier(line: &str) -> Option<(String, char, String, String)> {
    let needle = "from ";
    let start = line.find(needle)?;
    let specifier_start = start + needle.len();
    let quote = line[specifier_start..].chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let content_start = specifier_start + quote.len_utf8();
    let rest = &line[content_start..];
    let end_offset = rest.find(quote)?;
    let prefix = line[..content_start].to_string();
    let specifier = rest[..end_offset].to_string();
    let suffix = rest[end_offset + quote.len_utf8()..].to_string();
    Some((prefix, quote, specifier, suffix))
}

fn bundle_import_path(specifier: &str) -> String {
    let slug = specifier
        .trim_start_matches('@')
        .replace('/', "__")
        .replace('-', "_");
    format!("./_bundled/{slug}.d.ts")
}

#[cfg(test)]
mod tests {
    use super::TypeImportRewriter;

    #[test]
    fn appends_js_extensions_for_relative_files_and_directories() {
        let source = "import { a } from './foo';\nexport * from '../bar';";
        let output = TypeImportRewriter::new()
            .known_directory("../bar")
            .rewrite(source);

        assert_eq!(
            output.code,
            "import { a } from './foo.js';\nexport * from '../bar/index.js';"
        );
        assert_eq!(output.modified_imports, 2);
    }

    #[test]
    fn rewrites_bundled_packages_to_local_paths() {
        let source = "export type { Tool } from '@mastra/core';";
        let output = TypeImportRewriter::new()
            .bundle_package("@mastra/core")
            .rewrite(source);

        assert_eq!(
            output.code,
            "export type { Tool } from './_bundled/mastra__core.d.ts';"
        );
        assert_eq!(
            output.bundled_packages.into_iter().collect::<Vec<_>>(),
            vec!["@mastra/core".to_string()]
        );
    }
}
