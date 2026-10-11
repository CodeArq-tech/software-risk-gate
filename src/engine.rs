use crate::ast::{AstError, parse_project_with_limits};
use crate::config::{Config, ConfigError};
use crate::diff::{DiffError, DiffProvider};
use crate::domain::{AnalysisStatus, ChangedPath, ParsedFile};
use crate::gates::{
    ApprovalChecker, GateError, GateResult, ValidationExecutor, evaluate_gate, evaluate_gate_rule,
};
use crate::report::{ReportInput, render_markdown_limited};
use crate::risk::{
    GraphImpact, RiskAssessment, assess_risk_with_impact, graph_impacts, module_names,
};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Ast(#[from] AstError),
    #[error(transparent)]
    Diff(#[from] DiffError),
    #[error(transparent)]
    Gate(#[from] GateError),
    #[error("pull request has no changed files")]
    EmptyDiff,
}

#[derive(Debug)]
pub struct AnalysisResult {
    pub assessment: RiskAssessment,
    pub gate: GateResult,
    pub parsed_files: Vec<ParsedFile>,
    pub report: String,
    pub status: AnalysisStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannedValidation {
    pub name: String,
    #[serde(rename = "timeoutSeconds")]
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannedPolicy {
    #[serde(rename = "instantMerge")]
    pub instant_merge: bool,
    #[serde(
        rename = "requireApprovalGroup",
        skip_serializing_if = "Option::is_none"
    )]
    pub require_approval_group: Option<String>,
    pub validations: Vec<PlannedValidation>,
    #[serde(rename = "mergeBlockedIfUnsuccessful")]
    pub merge_blocked_if_unsuccessful: bool,
}

#[derive(Debug)]
pub struct RiskOnlyResult {
    pub changed: Vec<ChangedPath>,
    pub parsed_files: Vec<ParsedFile>,
    pub assessment: RiskAssessment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AstImpactFile {
    pub path: String,
    #[serde(rename = "oldPath", skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    #[serde(rename = "isNew")]
    pub is_new: bool,
    pub language: Option<String>,
    pub modules: Vec<String>,
    pub references: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AstImpactResult {
    #[serde(rename = "changedFiles")]
    pub changed_files: Vec<AstImpactFile>,
    pub impacts: Vec<GraphImpact>,
    #[serde(rename = "partialFiles")]
    pub partial_files: usize,
}

pub fn analyze_ast_impact<D: DiffProvider>(
    root: &std::path::Path,
    diff: &D,
) -> Result<AstImpactResult, AnalysisError> {
    let changed = diff.changed_paths()?;
    if changed.is_empty() {
        return Err(AnalysisError::EmptyDiff);
    }
    let limits = crate::config::ResourceLimits {
        max_files: 50_000,
        max_ast_nodes: 25_000_000,
        max_comment_bytes: 60_000,
    };
    let parsed_files = parse_project_with_limits(root, limits.max_files, limits.max_ast_nodes)?;
    let changed_files = changed
        .iter()
        .map(|changed_path| {
            let path = changed_path.path.replace('\\', "/");
            let parsed = parsed_files.iter().find(|file| file.path == path);
            AstImpactFile {
                path,
                old_path: changed_path.old_path.clone(),
                is_new: changed_path.is_new,
                language: parsed.and_then(|file| file.language.clone()),
                modules: parsed.map(module_names).unwrap_or_default(),
                references: parsed
                    .map(|file| file.references.clone())
                    .unwrap_or_default(),
                error: parsed.and_then(|file| file.error.clone()),
            }
        })
        .collect();
    let partial_files = parsed_files
        .iter()
        .filter(|file| file.error.is_some())
        .count();
    Ok(AstImpactResult {
        changed_files,
        impacts: graph_impacts(&changed, &parsed_files),
        partial_files,
    })
}

pub fn plan_policy(config: &Config, risk: crate::domain::RiskLevel) -> PlannedPolicy {
    let rule = config.gate_rules.get(&risk).cloned().unwrap_or_default();
    let validations = rule
        .validations
        .iter()
        .filter_map(|name| {
            config
                .validations
                .get(name)
                .map(|definition| PlannedValidation {
                    name: name.clone(),
                    timeout_seconds: definition.timeout_seconds(),
                })
        })
        .collect();
    PlannedPolicy {
        instant_merge: rule.instant_merge,
        require_approval_group: rule.require_approval_group,
        validations,
        merge_blocked_if_unsuccessful: !rule.instant_merge,
    }
}

pub fn analyze_risk_only<D: DiffProvider>(
    config: &Config,
    root: &std::path::Path,
    diff: &D,
) -> Result<RiskOnlyResult, AnalysisError> {
    let changed = diff.changed_paths()?;
    if changed.is_empty() {
        return Err(AnalysisError::EmptyDiff);
    }
    let parsed_files =
        parse_project_with_limits(root, config.limits.max_files, config.limits.max_ast_nodes)?;
    let assessment = assess_risk_with_impact(&config.risk_map, &changed, &parsed_files);
    Ok(RiskOnlyResult {
        changed,
        parsed_files,
        assessment,
    })
}

pub fn analyze<D: DiffProvider, E: ValidationExecutor>(
    config: &Config,
    root: &std::path::Path,
    diff: &D,
    executor: &E,
) -> Result<AnalysisResult, AnalysisError> {
    analyze_with_approval(config, root, diff, executor, None)
}

pub fn analyze_with_approval<D: DiffProvider, E: ValidationExecutor>(
    config: &Config,
    root: &std::path::Path,
    diff: &D,
    executor: &E,
    approval_checker: Option<&dyn ApprovalChecker>,
) -> Result<AnalysisResult, AnalysisError> {
    let RiskOnlyResult {
        parsed_files,
        assessment,
        ..
    } = analyze_risk_only(config, root, diff)?;
    let rule = config
        .gate_rules
        .get(&assessment.risk)
        .cloned()
        .unwrap_or_default();
    let gate = if approval_checker.is_some() {
        evaluate_gate_rule(config, assessment.risk, &rule, executor, approval_checker)?
    } else {
        evaluate_gate(config, assessment.risk, executor)?
    };
    let report = render_markdown_limited(
        &ReportInput {
            assessment: &assessment,
            gate: Some(&gate),
            parsed_files: &parsed_files,
            version: env!("CARGO_PKG_VERSION"),
        },
        config.limits.max_comment_bytes,
    );
    let status = if gate.status == AnalysisStatus::Failed {
        AnalysisStatus::Failed
    } else if parsed_files.iter().any(|file| file.error.is_some()) {
        AnalysisStatus::Partial
    } else {
        AnalysisStatus::Success
    };
    Ok(AnalysisResult {
        assessment,
        gate,
        parsed_files,
        report,
        status,
    })
}

#[derive(Debug, Clone)]
pub struct StaticDiffProvider(pub Vec<ChangedPath>);

impl DiffProvider for StaticDiffProvider {
    fn changed_paths(&self) -> Result<Vec<ChangedPath>, DiffError> {
        Ok(self.0.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GateRule, RiskRule, ValidationConfig};
    use crate::domain::RiskLevel;
    use crate::gates::ShellExecutor;
    use std::collections::BTreeMap;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn analyzes_changed_project_and_produces_report() {
        let root = tempdir().unwrap();
        fs::write(root.path().join("payment.py"), "class Payment: pass\n").unwrap();
        let config = Config {
            version: 1,
            risk_map: vec![RiskRule {
                path: "payment.py".into(),
                risk: RiskLevel::Danger,
                description: Some("payments".into()),
                new_code_policy: Default::default(),
            }],
            gate_rules: BTreeMap::from([(
                RiskLevel::Danger,
                GateRule {
                    validations: vec!["check".into()],
                    ..GateRule::default()
                },
            )]),
            validations: BTreeMap::from([(
                "check".into(),
                ValidationConfig::Command("true".into()),
            )]),
            approval_groups: BTreeMap::new(),
            limits: Default::default(),
        };
        let result = analyze(
            &config,
            root.path(),
            &StaticDiffProvider(vec![ChangedPath {
                path: "payment.py".into(),
                old_path: None,
                is_new: false,
            }]),
            &ShellExecutor::new(root.path()),
        )
        .unwrap();
        assert_eq!(result.assessment.risk, RiskLevel::Danger);
        assert_eq!(result.status, AnalysisStatus::Success);
        assert!(result.report.contains("payments"));
    }

    #[test]
    fn rejects_an_empty_pull_request_diff() {
        let root = tempdir().unwrap();
        let config = Config {
            version: 1,
            risk_map: vec![RiskRule {
                path: "src/".into(),
                risk: RiskLevel::Normal,
                description: None,
                new_code_policy: Default::default(),
            }],
            gate_rules: BTreeMap::new(),
            validations: BTreeMap::new(),
            approval_groups: BTreeMap::new(),
            limits: Default::default(),
        };
        let error = analyze(
            &config,
            root.path(),
            &StaticDiffProvider(vec![]),
            &ShellExecutor::new(root.path()),
        )
        .unwrap_err();
        assert!(matches!(error, AnalysisError::EmptyDiff));
    }

    #[test]
    fn plans_the_gate_without_executing_validations() {
        let config = Config {
            version: 1,
            risk_map: vec![rule_for_test("src/", RiskLevel::High)],
            gate_rules: BTreeMap::from([(
                RiskLevel::High,
                GateRule {
                    instant_merge: false,
                    require_approval_group: Some("architects".into()),
                    validations: vec!["e2e".into()],
                },
            )]),
            validations: BTreeMap::from([(
                "e2e".into(),
                ValidationConfig::Detailed {
                    command: "touch should-not-run".into(),
                    timeout_seconds: Some(42),
                },
            )]),
            approval_groups: BTreeMap::new(),
            limits: Default::default(),
        };
        let plan = plan_policy(&config, RiskLevel::High);
        assert!(!plan.instant_merge);
        assert_eq!(plan.require_approval_group.as_deref(), Some("architects"));
        assert_eq!(plan.validations[0].name, "e2e");
        assert_eq!(plan.validations[0].timeout_seconds, 42);
        assert!(plan.merge_blocked_if_unsuccessful);
    }

    fn rule_for_test(path: &str, risk: RiskLevel) -> RiskRule {
        RiskRule {
            path: path.into(),
            risk,
            description: None,
            new_code_policy: Default::default(),
        }
    }
}
