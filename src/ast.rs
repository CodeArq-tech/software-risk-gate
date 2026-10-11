use crate::domain::{AstNode, ParsedFile};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tree_sitter::{Language, Node, Parser};
use walkdir::WalkDir;

#[derive(Debug, Error)]
pub enum AstError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("parser setup failed for {0}")]
    ParserSetup(String),
    #[error("resource limit exceeded: {0}")]
    ResourceLimit(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedLanguage {
    Python,
    JavaScript,
    TypeScript,
    Elixir,
}

impl SupportedLanguage {
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("py") => Some(Self::Python),
            Some("js" | "jsx" | "mjs" | "cjs") => Some(Self::JavaScript),
            Some("ts" | "tsx") => Some(Self::TypeScript),
            Some("ex" | "exs") => Some(Self::Elixir),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Elixir => "elixir",
        }
    }

    fn language(self) -> Language {
        match self {
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Elixir => tree_sitter_elixir::LANGUAGE.into(),
        }
    }
}

pub fn parse_file(root: &Path, path: &Path) -> Result<ParsedFile, AstError> {
    let absolute = root.join(path);
    let Some(language) = SupportedLanguage::from_path(path) else {
        return Ok(ParsedFile {
            path: path.to_string_lossy().replace('\\', "/"),
            language: None,
            root: None,
            error: Some("unsupported language".to_owned()),
            references: Vec::new(),
        });
    };
    let source = fs::read_to_string(&absolute).map_err(|source| AstError::Read {
        path: absolute,
        source,
    })?;
    let mut parser = Parser::new();
    parser
        .set_language(&language.language())
        .map_err(|_| AstError::ParserSetup(language.name().to_owned()))?;
    let Some(tree) = parser.parse(&source, None) else {
        return Ok(ParsedFile {
            path: path.to_string_lossy().replace('\\', "/"),
            language: Some(language.name().to_owned()),
            root: None,
            error: Some("parser returned no tree".to_owned()),
            references: Vec::new(),
        });
    };
    let root_node = convert_node(tree.root_node(), language.name(), path, source.as_bytes());
    let error = if tree.root_node().has_error() {
        Some("syntax errors detected".to_owned())
    } else {
        None
    };
    Ok(ParsedFile {
        path: path.to_string_lossy().replace('\\', "/"),
        language: Some(language.name().to_owned()),
        root: Some(root_node),
        error,
        references: collect_references(tree.root_node(), source.as_bytes()),
    })
}

pub fn parse_project(root: &Path) -> Result<Vec<ParsedFile>, AstError> {
    parse_project_with_limits(root, usize::MAX, usize::MAX)
}

pub fn parse_project_with_limits(
    root: &Path,
    max_files: usize,
    max_ast_nodes: usize,
) -> Result<Vec<ParsedFile>, AstError> {
    let mut paths = WalkDir::new(root).into_iter().filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| !entry.path().components().any(|component| matches!(component, std::path::Component::Normal(value) if value == ".git" || value == "target" || value == "node_modules")))
        .filter_map(|entry| entry.path().strip_prefix(root).ok().map(Path::to_path_buf))
        .collect::<Vec<_>>();
    paths.sort();
    if paths.len() > max_files {
        return Err(AstError::ResourceLimit(format!(
            "file count {} exceeds {}",
            paths.len(),
            max_files
        )));
    }
    paths
        .into_iter()
        .map(|path| parse_file(root, &path))
        .collect::<Result<Vec<_>, _>>()
        .and_then(|parsed| {
            let nodes = parsed
                .iter()
                .map(|file| file.root.as_ref().map_or(0, count_nodes))
                .sum::<usize>();
            if nodes > max_ast_nodes {
                Err(AstError::ResourceLimit(format!(
                    "AST node count {} exceeds {}",
                    nodes, max_ast_nodes
                )))
            } else {
                Ok(parsed)
            }
        })
}

fn count_nodes(node: &AstNode) -> usize {
    1 + node.children.iter().map(count_nodes).sum::<usize>()
}

fn collect_references(root: Node<'_>, source: &[u8]) -> Vec<String> {
    let mut references = Vec::new();
    collect_references_from_node(root, source, &mut references);
    references.sort();
    references.dedup();
    references
}

fn collect_references_from_node(node: Node<'_>, source: &[u8], references: &mut Vec<String>) {
    let kind = node.kind();
    let is_reference_node = matches!(
        kind,
        "import_statement"
            | "import_from_statement"
            | "export_statement"
            | "call_expression"
            | "alias"
            | "import"
            | "require"
            | "use"
            | "call"
    );
    if is_reference_node && let Ok(text) = node.utf8_text(source) {
        extract_reference_tokens(kind, text, references);
    }
    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index as u32) {
            collect_references_from_node(child, source, references);
        }
    }
}

fn extract_reference_tokens(kind: &str, text: &str, references: &mut Vec<String>) {
    let mut quoted = Vec::new();
    let mut quote = None;
    let mut token = String::new();
    for character in text.chars() {
        if let Some(delimiter) = quote {
            if character == delimiter {
                if !token.is_empty() {
                    quoted.push(std::mem::take(&mut token));
                }
                quote = None;
            } else {
                token.push(character);
            }
        } else if character == '\'' || character == '"' {
            quote = Some(character);
        }
    }
    references.extend(quoted);
    if kind == "import_statement" || kind == "import_from_statement" {
        let words = text
            .split(|character: char| character.is_whitespace() || character == ',')
            .filter(|word| !word.is_empty());
        let mut capture = false;
        for word in words {
            if word == "import" || word == "from" {
                capture = true;
                continue;
            }
            if capture && word != "as" {
                references.push(word.trim_matches(';').to_owned());
                if kind == "import_from_statement" {
                    break;
                }
            }
        }
    }
    if kind == "alias" || kind == "import" || kind == "require" || kind == "use" || kind == "call" {
        let mut words = text.split_whitespace();
        let _ = words.next();
        if let Some(module) = words.next() {
            references.push(
                module
                    .trim_matches(|character: char| {
                        character == ',' || character == '(' || character == ')' || character == ':'
                    })
                    .to_owned(),
            );
        }
    }
}

fn convert_node(node: Node<'_>, language: &str, path: &Path, source: &[u8]) -> AstNode {
    let name = node.named_child(0).and_then(|child| {
        let kind = child.kind();
        if kind.contains("identifier") || kind == "alias" || kind == "module" {
            child.utf8_text(source).ok().map(str::to_owned)
        } else {
            None
        }
    });
    let children = (0..node.named_child_count())
        .filter_map(|index| node.named_child(index as u32))
        .map(|child| convert_node(child, language, path, source))
        .collect();
    AstNode {
        kind: node.kind().to_owned(),
        language: language.to_owned(),
        path: path.to_string_lossy().replace('\\', "/"),
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        name,
        children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn recognizes_the_four_initial_languages() {
        assert_eq!(
            SupportedLanguage::from_path(Path::new("main.py")),
            Some(SupportedLanguage::Python)
        );
        assert_eq!(
            SupportedLanguage::from_path(Path::new("main.js")),
            Some(SupportedLanguage::JavaScript)
        );
        assert_eq!(
            SupportedLanguage::from_path(Path::new("main.ts")),
            Some(SupportedLanguage::TypeScript)
        );
        assert_eq!(
            SupportedLanguage::from_path(Path::new("main.ex")),
            Some(SupportedLanguage::Elixir)
        );
        assert_eq!(SupportedLanguage::from_path(Path::new("README.md")), None);
        assert_eq!(SupportedLanguage::Python.name(), "python");
        assert_eq!(SupportedLanguage::JavaScript.name(), "javascript");
        assert_eq!(SupportedLanguage::TypeScript.name(), "typescript");
        assert_eq!(SupportedLanguage::Elixir.name(), "elixir");
    }

    #[test]
    fn parses_a_project_in_stable_path_order_and_reports_unknown_files() {
        let directory = tempdir().expect("temp dir");
        fs::write(directory.path().join("z.py"), "class Payment:\n    pass\n").expect("python");
        fs::write(
            directory.path().join("a.ts"),
            "export interface User { id: string }\n",
        )
        .expect("typescript");
        fs::write(directory.path().join("README.md"), "docs").expect("readme");
        fs::create_dir(directory.path().join(".git")).expect("git dir");
        fs::write(directory.path().join(".git/config"), "ignored").expect("git file");

        let parsed = parse_project(directory.path()).expect("project parses");
        assert_eq!(
            parsed
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            vec!["README.md", "a.ts", "z.py"]
        );
        assert!(parsed[0].error.as_deref() == Some("unsupported language"));
        assert_eq!(parsed[1].language.as_deref(), Some("typescript"));
        assert!(
            parsed[1]
                .root
                .as_ref()
                .is_some_and(|root| !root.children.is_empty())
        );
        assert_eq!(parsed[2].language.as_deref(), Some("python"));
    }

    #[test]
    fn reports_syntax_errors_without_dropping_the_tree() {
        let directory = tempdir().expect("temp dir");
        fs::write(directory.path().join("broken.py"), "def broken(:\n").expect("python");
        let parsed = parse_file(directory.path(), Path::new("broken.py")).expect("parse result");
        assert_eq!(parsed.language.as_deref(), Some("python"));
        assert_eq!(parsed.error.as_deref(), Some("syntax errors detected"));
        assert!(parsed.root.is_some());
    }

    #[test]
    fn parses_javascript_typescript_and_elixir_files() {
        let directory = tempdir().expect("temp dir");
        fs::write(
            directory.path().join("app.js"),
            "export function run() { return 1; }\n",
        )
        .unwrap();
        fs::write(
            directory.path().join("app.ts"),
            "export type User = { id: string };\n",
        )
        .unwrap();
        fs::write(
            directory.path().join("app.ex"),
            "defmodule App do\n  def run, do: :ok\nend\n",
        )
        .unwrap();
        for (file, language) in [
            ("app.js", "javascript"),
            ("app.ts", "typescript"),
            ("app.ex", "elixir"),
        ] {
            let parsed = parse_file(directory.path(), Path::new(file)).unwrap();
            assert_eq!(parsed.language.as_deref(), Some(language));
            assert!(
                parsed
                    .root
                    .as_ref()
                    .is_some_and(|root| !root.children.is_empty())
            );
        }
        fn has_name(node: &AstNode, expected: &str) -> bool {
            node.name.as_deref() == Some(expected)
                || node.children.iter().any(|child| has_name(child, expected))
        }
        for (file, name) in [("app.js", "run"), ("app.ts", "User"), ("app.ex", "App")] {
            let parsed = parse_file(directory.path(), Path::new(file)).unwrap();
            assert!(has_name(parsed.root.as_ref().unwrap(), name));
        }
        let parsed = parse_file(directory.path(), Path::new("app.ex")).unwrap();
        assert!(parsed.root.is_some());
    }

    #[test]
    fn extracts_dependency_references_from_supported_import_forms() {
        let directory = tempdir().unwrap();
        fs::write(
            directory.path().join("main.py"),
            "from src.payment.core import authorize\n",
        )
        .unwrap();
        fs::write(
            directory.path().join("main.js"),
            "import { authorize } from './payment/core';\n",
        )
        .unwrap();
        let python = parse_file(directory.path(), Path::new("main.py")).unwrap();
        let javascript = parse_file(directory.path(), Path::new("main.js")).unwrap();
        assert!(
            python
                .references
                .iter()
                .any(|reference| reference == "src.payment.core")
        );
        assert!(
            javascript
                .references
                .iter()
                .any(|reference| reference == "./payment/core")
        );
    }

    #[test]
    fn reference_extraction_covers_import_and_alias_forms() {
        let mut references = Vec::new();
        extract_reference_tokens(
            "import_statement",
            "import alpha.beta as alias",
            &mut references,
        );
        extract_reference_tokens(
            "import_from_statement",
            "from gamma.delta import run",
            &mut references,
        );
        assert!(references.contains(&"alpha.beta".to_owned()));
        assert!(references.contains(&"gamma.delta".to_owned()));
        assert!(!references.contains(&"as".to_owned()));

        for kind in ["alias", "import", "require", "use", "call"] {
            let mut values = Vec::new();
            extract_reference_tokens(kind, "keyword ,(:My.App):", &mut values);
            assert_eq!(values, vec!["My.App"]);
        }

        let mut unknown = Vec::new();
        extract_reference_tokens("other", "keyword My.App", &mut unknown);
        assert!(unknown.is_empty());
    }

    #[test]
    fn preserves_named_identifier_nodes() {
        let directory = tempdir().expect("temp dir");
        fs::write(
            directory.path().join("named.py"),
            "class Payment:\n    pass\n",
        )
        .unwrap();
        let parsed = parse_file(directory.path(), Path::new("named.py")).unwrap();
        fn has_name(node: &AstNode, expected: &str) -> bool {
            node.name.as_deref() == Some(expected)
                || node.children.iter().any(|child| has_name(child, expected))
        }
        assert!(has_name(parsed.root.as_ref().unwrap(), "Payment"));
    }

    #[test]
    fn reports_read_errors_for_missing_files() {
        let error = parse_file(Path::new("/definitely/missing"), Path::new("file.py")).unwrap_err();
        assert!(matches!(error, AstError::Read { .. }));
    }

    #[test]
    fn enforces_file_and_ast_node_limits() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join("one.py"), "class One: pass\n").unwrap();
        assert!(matches!(
            parse_project_with_limits(directory.path(), 0, usize::MAX),
            Err(AstError::ResourceLimit(message)) if message.contains("file count")
        ));
        assert!(matches!(
            parse_project_with_limits(directory.path(), usize::MAX, 1),
            Err(AstError::ResourceLimit(message)) if message.contains("AST node count")
        ));
    }

    #[test]
    fn accepts_exact_resource_limits() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join("one.py"), "class One: pass\n").unwrap();
        assert_eq!(
            parse_project_with_limits(directory.path(), 1, usize::MAX)
                .unwrap()
                .len(),
            1
        );
        let parsed = parse_project(directory.path()).unwrap();
        let exact_nodes = parsed
            .iter()
            .map(|file| file.root.as_ref().map_or(0, count_nodes))
            .sum();
        assert!(parse_project_with_limits(directory.path(), usize::MAX, exact_nodes).is_ok());
    }
}
