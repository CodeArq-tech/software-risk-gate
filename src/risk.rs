use crate::config::{NewCodePolicy, RiskRule, normalize_path, path_matches_rule};
use crate::domain::{AstNode, ChangedPath, ParsedFile, RiskLevel};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GraphImpact {
    pub changed_path: String,
    pub affected_path: String,
    pub path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RiskEvidence {
    pub path: String,
    pub risk: RiskLevel,
    pub description: Option<String>,
    pub matched_rule: Option<String>,
    pub modules: Vec<String>,
    pub ignored_isolated_new_code: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ImpactEvidence {
    pub changed_path: String,
    pub related_path: String,
    pub risk: RiskLevel,
    pub rule: String,
    pub description: Option<String>,
    pub path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RiskAssessment {
    pub risk: RiskLevel,
    pub evidence: Vec<RiskEvidence>,
    pub impact: Vec<ImpactEvidence>,
}

pub fn assess_risk(
    rules: &[RiskRule],
    changed: &[ChangedPath],
    files: &[ParsedFile],
) -> RiskAssessment {
    assess_risk_with_impact(rules, changed, files)
}

pub fn assess_risk_with_impact(
    rules: &[RiskRule],
    changed: &[ChangedPath],
    files: &[ParsedFile],
) -> RiskAssessment {
    assess_risk_with_impact_and_ignores(rules, &[], changed, files)
}

pub fn assess_risk_with_impact_and_ignores(
    rules: &[RiskRule],
    ignore_paths: &[String],
    changed: &[ChangedPath],
    files: &[ParsedFile],
) -> RiskAssessment {
    let changed = changed
        .iter()
        .filter(|changed_path| !is_ignored_path(&changed_path.path, ignore_paths))
        .collect::<Vec<_>>();
    let files = files
        .iter()
        .filter(|file| !is_ignored_path(&file.path, ignore_paths))
        .cloned()
        .collect::<Vec<_>>();
    let graph = dependency_graph(&files);
    let mut evidence = Vec::new();
    let mut impact = Vec::new();
    for changed_path in changed {
        let paths = changed_path
            .old_path
            .iter()
            .chain(std::iter::once(&changed_path.path));
        for path in paths {
            let normalized = normalize_path(path).unwrap_or_else(|_| path.replace('\\', "/"));
            let matching = rules
                .iter()
                .filter(|rule| matches_path(&normalized, &rule.path))
                .collect::<Vec<_>>();
            let selected = matching.iter().fold(None, |current, rule| match current {
                None => Some(rule),
                Some(selected) if rule.risk > selected.risk => Some(rule),
                Some(selected) => Some(selected),
            });
            let impacted = impact_paths(&normalized, rules, &graph);
            let isolated_new_code = changed_path.is_new
                && selected.is_some_and(|rule| rule.new_code_policy == NewCodePolicy::Ignore)
                && impacted.is_empty();
            let (mut risk, matched_rule, description) = selected
                .map(|rule| (rule.risk, Some(rule.path.clone()), rule.description.clone()))
                .unwrap_or((RiskLevel::Normal, None, None));
            if isolated_new_code {
                risk = RiskLevel::Minimal;
            }
            for (related_path, rule, path) in impacted {
                risk = risk.max(rule.risk);
                impact.push(ImpactEvidence {
                    changed_path: normalized.clone(),
                    related_path,
                    risk: rule.risk,
                    rule: rule.path.clone(),
                    description: rule.description.clone(),
                    path,
                });
            }
            let modules = files
                .iter()
                .filter(|file| file.path == normalized || file.path == *path)
                .filter_map(|file| file.root.as_ref())
                .flat_map(collect_module_names)
                .collect();
            evidence.push(RiskEvidence {
                path: normalized,
                risk,
                description,
                matched_rule,
                modules,
                ignored_isolated_new_code: isolated_new_code,
            });
        }
    }
    let risk = evidence
        .iter()
        .map(|item| item.risk)
        .max()
        .unwrap_or(RiskLevel::Normal);
    RiskAssessment {
        risk,
        evidence,
        impact,
    }
}

fn is_ignored_path(path: &str, ignore_paths: &[String]) -> bool {
    let normalized = normalize_path(path).unwrap_or_else(|_| path.replace('\\', "/"));
    ignore_paths
        .iter()
        .any(|rule| path_matches_rule(&normalized, rule))
}

pub fn graph_impacts(changed: &[ChangedPath], files: &[ParsedFile]) -> Vec<GraphImpact> {
    let graph = dependency_graph(files);
    let mut impacts = Vec::new();
    for changed_path in changed {
        let normalized = normalize_path(&changed_path.path)
            .unwrap_or_else(|_| changed_path.path.replace('\\', "/"));
        for (affected_path, path) in graph_paths(&normalized, &graph) {
            impacts.push(GraphImpact {
                changed_path: normalized.clone(),
                affected_path,
                path,
            });
        }
    }
    impacts.sort_by(|left, right| {
        (&left.changed_path, &left.affected_path, &left.path).cmp(&(
            &right.changed_path,
            &right.affected_path,
            &right.path,
        ))
    });
    impacts
}

pub fn module_names(file: &ParsedFile) -> Vec<String> {
    file.root
        .as_ref()
        .map(collect_module_names)
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DependencyEdge {
    from: String,
    to: String,
}

fn dependency_graph(files: &[ParsedFile]) -> Vec<DependencyEdge> {
    let mut edges = Vec::new();
    for source in files {
        for reference in &source.references {
            if let Some(target) = resolve_reference(&source.path, reference, files)
                && target != source.path
            {
                edges.push(DependencyEdge {
                    from: source.path.clone(),
                    to: target,
                });
            }
        }
    }
    edges.sort_by(|left, right| (&left.from, &left.to).cmp(&(&right.from, &right.to)));
    edges.dedup_by(|left, right| left.from == right.from && left.to == right.to);
    edges
}

fn resolve_reference(source: &str, reference: &str, files: &[ParsedFile]) -> Option<String> {
    let reference = reference
        .trim_matches(|character: char| character == '"' || character == '\'' || character == ';');
    if reference.is_empty() {
        return None;
    }
    let source_parent = source.rsplit_once('/').map_or("", |(parent, _)| parent);
    let normalized = reference.replace('\\', "/");
    let base = if normalized.starts_with('.') {
        let mut parent = source_parent.split('/').collect::<Vec<_>>();
        let mut relative = normalized.as_str();
        if relative == ".." {
            parent.pop();
            relative = "";
        } else {
            while let Some(stripped) = relative.strip_prefix("../") {
                parent.pop();
                relative = stripped;
            }
            while let Some(stripped) = relative.strip_prefix("./") {
                relative = stripped;
            }
            if relative.starts_with('.') {
                relative = relative.trim_start_matches('.');
            }
        }
        for part in relative.split(['/', '.']) {
            match part {
                "." | "" => {}
                ".." => {
                    parent.pop();
                }
                part => parent.push(part),
            }
        }
        parent.join("/")
    } else if normalized.contains('/') {
        normalized.clone()
    } else {
        normalized.replace('.', "/")
    };
    let mut bases = vec![base.clone()];
    let snake = snake_case_module(&base);
    if snake != base {
        bases.push(snake);
    }
    let mut candidates = Vec::new();
    for base in bases {
        candidates.extend([
            base.clone(),
            format!("{base}.py"),
            format!("{base}.js"),
            format!("{base}.ts"),
            format!("{base}.ex"),
            format!("{base}/index.js"),
            format!("{base}/index.ts"),
            format!("{base}/__init__.py"),
            format!("lib/{base}.ex"),
        ]);
    }
    files
        .iter()
        .map(|file| file.path.replace('\\', "/"))
        .find(|path| candidates.iter().any(|candidate| candidate == path))
}

fn snake_case_module(value: &str) -> String {
    let mut result = String::new();
    let mut previous = None;
    for character in value.chars() {
        if character.is_ascii_uppercase()
            && previous.is_some_and(|previous| previous != '/' && previous != '_')
        {
            result.push('_');
        }
        result.push(character.to_ascii_lowercase());
        previous = Some(character);
    }
    result
}

fn impact_paths<'a>(
    changed: &str,
    rules: &'a [RiskRule],
    graph: &[DependencyEdge],
) -> Vec<(String, &'a RiskRule, Vec<String>)> {
    let mut queue = vec![(changed.to_owned(), vec![changed.to_owned()])];
    let mut visited = std::collections::BTreeSet::from([changed.to_owned()]);
    let mut result = Vec::new();
    while let Some((current, path)) = queue.pop() {
        for neighbor in graph.iter().filter_map(|edge| {
            if edge.from == current {
                Some(edge.to.clone())
            } else if edge.to == current {
                Some(edge.from.clone())
            } else {
                None
            }
        }) {
            if !visited.insert(neighbor.clone()) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(neighbor.clone());
            if let Some(rule) = rules
                .iter()
                .filter(|rule| matches_path(&neighbor, &rule.path))
                .max_by_key(|rule| rule.risk)
            {
                result.push((neighbor.clone(), rule, next_path.clone()));
            }
            queue.push((neighbor, next_path));
        }
    }
    result
}

fn graph_paths(changed: &str, graph: &[DependencyEdge]) -> Vec<(String, Vec<String>)> {
    let mut queue = vec![(changed.to_owned(), vec![changed.to_owned()])];
    let mut visited = BTreeSet::from([changed.to_owned()]);
    let mut result = Vec::new();
    while let Some((current, path)) = queue.pop() {
        for neighbor in graph.iter().filter_map(|edge| {
            if edge.from == current {
                Some(edge.to.clone())
            } else if edge.to == current {
                Some(edge.from.clone())
            } else {
                None
            }
        }) {
            if !visited.insert(neighbor.clone()) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(neighbor.clone());
            result.push((neighbor.clone(), next_path.clone()));
            queue.push((neighbor, next_path));
        }
    }
    result
}

fn matches_path(path: &str, rule: &str) -> bool {
    path_matches_rule(path, rule)
}

fn collect_module_names(node: &AstNode) -> Vec<String> {
    let mut names = Vec::new();
    if node.name.is_some() {
        names.push(node.name.clone().unwrap_or_default());
    }
    for child in &node.children {
        names.extend(collect_module_names(child));
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RiskRule;

    fn rule(path: &str, risk: RiskLevel) -> RiskRule {
        RiskRule {
            path: path.into(),
            risk,
            description: Some(path.into()),
            new_code_policy: Default::default(),
        }
    }

    #[test]
    fn uses_highest_matching_risk_and_normal_fallback() {
        let rules = vec![
            rule("src/", RiskLevel::Low),
            rule("src/payment/", RiskLevel::Danger),
        ];
        let changed = vec![
            ChangedPath {
                path: "src/payment/card.py".into(),
                old_path: None,
                is_new: false,
            },
            ChangedPath {
                path: "new.txt".into(),
                old_path: None,
                is_new: false,
            },
        ];
        let result = assess_risk(&rules, &changed, &[]);
        assert_eq!(result.risk, RiskLevel::Danger);
        assert_eq!(result.evidence[1].risk, RiskLevel::Normal);
    }

    #[test]
    fn evaluates_old_path_on_rename() {
        let rules = vec![rule("infra/", RiskLevel::Danger)];
        let changed = vec![ChangedPath {
            path: "src/new.py".into(),
            old_path: Some("infra/old.py".into()),
            is_new: false,
        }];
        assert_eq!(assess_risk(&rules, &changed, &[]).risk, RiskLevel::Danger);
    }

    #[test]
    fn preserves_first_rule_description_when_equal_risk() {
        let rules = vec![
            RiskRule {
                path: "src/".into(),
                risk: RiskLevel::High,
                description: Some("broad".into()),
                new_code_policy: Default::default(),
            },
            RiskRule {
                path: "src/special/".into(),
                risk: RiskLevel::High,
                description: Some("specific".into()),
                new_code_policy: Default::default(),
            },
        ];
        let result = assess_risk(
            &rules,
            &[ChangedPath {
                path: "src/special/file.py".into(),
                old_path: None,
                is_new: false,
            }],
            &[],
        );
        assert_eq!(result.evidence[0].description.as_deref(), Some("broad"));

        let low_rules = vec![
            rule("tests/first/", RiskLevel::Low),
            rule("tests/second/", RiskLevel::Low),
        ];
        let low_result = assess_risk(
            &low_rules,
            &[ChangedPath {
                path: "tests/second/file.py".into(),
                old_path: None,
                is_new: false,
            }],
            &[],
        );
        assert_eq!(
            low_result.evidence[0].matched_rule.as_deref(),
            Some("tests/second/")
        );
    }

    #[test]
    fn maps_modules_when_changed_path_needs_normalization() {
        let files = vec![crate::domain::ParsedFile {
            path: "src/payment.py".into(),
            language: Some("python".into()),
            root: Some(crate::domain::AstNode {
                kind: "module".into(),
                language: "python".into(),
                path: "src/payment.py".into(),
                start_byte: 0,
                end_byte: 1,
                name: Some("Payment".into()),
                children: vec![],
            }),
            error: None,
            references: vec![],
        }];
        let result = assess_risk(
            &[rule("src/", RiskLevel::Normal)],
            &[ChangedPath {
                path: "src\\payment.py".into(),
                old_path: None,
                is_new: false,
            }],
            &files,
        );
        assert_eq!(result.evidence[0].modules, vec!["Payment"]);

        let raw_files = vec![crate::domain::ParsedFile {
            path: "src\\payment.py".into(),
            language: Some("python".into()),
            root: Some(crate::domain::AstNode {
                kind: "module".into(),
                language: "python".into(),
                path: "src\\payment.py".into(),
                start_byte: 0,
                end_byte: 1,
                name: Some("RawPayment".into()),
                children: vec![],
            }),
            error: None,
            references: vec![],
        }];
        let raw_result = assess_risk(
            &[rule("src/", RiskLevel::Normal)],
            &[ChangedPath {
                path: "src\\payment.py".into(),
                old_path: None,
                is_new: false,
            }],
            &raw_files,
        );
        assert_eq!(raw_result.evidence[0].modules, vec!["RawPayment"]);
    }

    #[test]
    fn ignores_isolated_new_code_when_configured() {
        let rules = vec![RiskRule {
            path: "src/payment/".into(),
            risk: RiskLevel::Danger,
            description: Some("payments".into()),
            new_code_policy: NewCodePolicy::Ignore,
        }];
        let files = vec![ParsedFile {
            path: "src/payment/new.py".into(),
            language: Some("python".into()),
            root: None,
            error: None,
            references: vec![],
        }];
        let result = assess_risk_with_impact(
            &rules,
            &[ChangedPath {
                path: "src/payment/new.py".into(),
                old_path: None,
                is_new: true,
            }],
            &files,
        );
        assert_eq!(result.risk, RiskLevel::Minimal);
        assert!(result.evidence[0].ignored_isolated_new_code);
        assert!(result.impact.is_empty());
    }

    #[test]
    fn excludes_configured_paths_from_risk_and_graph_analysis() {
        let files = vec![
            ParsedFile {
                path: "src/payment/core.py".into(),
                language: Some("python".into()),
                root: None,
                error: None,
                references: vec![],
            },
            ParsedFile {
                path: "tests/payment_test.py".into(),
                language: Some("python".into()),
                root: None,
                error: None,
                references: vec!["src.payment.core".into()],
            },
        ];
        let result = assess_risk_with_impact_and_ignores(
            &[rule("src/payment/", RiskLevel::Danger)],
            &["tests/".into()],
            &[ChangedPath {
                path: "tests/payment_test.py".into(),
                old_path: None,
                is_new: false,
            }],
            &files,
        );
        assert_eq!(result.risk, RiskLevel::Normal);
        assert!(result.evidence.is_empty());
        assert!(result.impact.is_empty());
    }

    #[test]
    fn includes_transitive_cascade_for_new_code_connected_to_risk_path() {
        let rules = vec![RiskRule {
            path: "src/payment/".into(),
            risk: RiskLevel::Danger,
            description: Some("payment authorization".into()),
            new_code_policy: NewCodePolicy::Ignore,
        }];
        let files = vec![
            ParsedFile {
                path: "src/payment/core.py".into(),
                language: Some("python".into()),
                root: None,
                error: None,
                references: vec![],
            },
            ParsedFile {
                path: "src/payment/new.py".into(),
                language: Some("python".into()),
                root: None,
                error: None,
                references: vec!["./core".into()],
            },
        ];
        let result = assess_risk_with_impact(
            &rules,
            &[ChangedPath {
                path: "src/payment/new.py".into(),
                old_path: None,
                is_new: true,
            }],
            &files,
        );
        assert_eq!(result.risk, RiskLevel::Danger);
        assert!(!result.evidence[0].ignored_isolated_new_code);
        assert_eq!(result.impact[0].related_path, "src/payment/core.py");
        assert_eq!(result.impact[0].path.len(), 2);
    }

    #[test]
    fn raises_risk_when_safe_changed_code_uses_critical_code() {
        let rules = vec![rule("src/payment/", RiskLevel::Danger)];
        let files = vec![
            ParsedFile {
                path: "src/payment/core.py".into(),
                language: Some("python".into()),
                root: None,
                error: None,
                references: vec![],
            },
            ParsedFile {
                path: "src/new.py".into(),
                language: Some("python".into()),
                root: None,
                error: None,
                references: vec!["./payment/core".into()],
            },
        ];
        let result = assess_risk_with_impact(
            &rules,
            &[ChangedPath {
                path: "src/new.py".into(),
                old_path: None,
                is_new: false,
            }],
            &files,
        );
        assert_eq!(result.risk, RiskLevel::Danger);
        assert_eq!(result.impact[0].rule, "src/payment/");
    }

    #[test]
    fn dependency_graph_filters_self_references_and_duplicate_edges() {
        let files = vec![
            ParsedFile {
                path: "src/a.py".into(),
                language: Some("python".into()),
                root: None,
                error: None,
                references: vec!["./a".into(), "./b".into(), "./b".into()],
            },
            ParsedFile {
                path: "src/b.py".into(),
                language: Some("python".into()),
                root: None,
                error: None,
                references: vec![],
            },
        ];
        assert_eq!(
            dependency_graph(&files),
            vec![DependencyEdge {
                from: "src/a.py".into(),
                to: "src/b.py".into()
            }]
        );
        assert!(
            !dependency_graph(&files)
                .iter()
                .any(|edge| edge.from == edge.to)
        );
    }

    #[test]
    fn graph_paths_follow_edges_in_both_directions_without_revisiting_nodes() {
        let graph = vec![
            DependencyEdge {
                from: "src/a.py".into(),
                to: "src/b.py".into(),
            },
            DependencyEdge {
                from: "src/b.py".into(),
                to: "src/c.py".into(),
            },
        ];
        let paths = graph_paths("src/b.py", &graph);
        assert!(
            paths
                .iter()
                .any(|(path, chain)| { path == "src/a.py" && chain == &["src/b.py", "src/a.py"] })
        );
        assert!(
            paths
                .iter()
                .any(|(path, chain)| { path == "src/c.py" && chain == &["src/b.py", "src/c.py"] })
        );
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn resolves_quoted_semicolon_references_and_rejects_empty_references() {
        let files = vec![ParsedFile {
            path: "src/payment/core.py".into(),
            language: Some("python".into()),
            root: None,
            error: None,
            references: vec![],
        }];
        assert_eq!(
            resolve_reference("src/main.py", "\"./payment/core;\"", &files),
            Some("src/payment/core.py".into())
        );
        assert_eq!(resolve_reference("src/main.py", "", &files), None);
    }

    #[test]
    fn detects_reverse_impact_when_critical_code_uses_changed_code() {
        let files = vec![
            ParsedFile {
                path: "src/payment/core.py".into(),
                language: Some("python".into()),
                root: None,
                error: None,
                references: vec!["../new".into()],
            },
            ParsedFile {
                path: "src/new.py".into(),
                language: Some("python".into()),
                root: None,
                error: None,
                references: vec![],
            },
        ];
        let result = assess_risk_with_impact(
            &[rule("src/payment/", RiskLevel::Danger)],
            &[ChangedPath {
                path: "src/new.py".into(),
                old_path: None,
                is_new: false,
            }],
            &files,
        );
        assert_eq!(result.risk, RiskLevel::Danger);
        assert_eq!(
            result.impact[0].path,
            vec!["src/new.py", "src/payment/core.py"]
        );
    }

    #[test]
    fn resolves_elixir_module_names_under_lib() {
        let files = vec![
            ParsedFile {
                path: "lib/my_app/payment.ex".into(),
                language: Some("elixir".into()),
                root: None,
                error: None,
                references: vec![],
            },
            ParsedFile {
                path: "lib/my_app/new.ex".into(),
                language: Some("elixir".into()),
                root: None,
                error: None,
                references: vec!["MyApp.Payment".into()],
            },
        ];
        let result = assess_risk_with_impact(
            &[rule("lib/my_app/payment.ex", RiskLevel::High)],
            &[ChangedPath {
                path: "lib/my_app/new.ex".into(),
                old_path: None,
                is_new: false,
            }],
            &files,
        );
        assert_eq!(result.risk, RiskLevel::High);
    }
}
