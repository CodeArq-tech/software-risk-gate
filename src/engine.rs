use crate::ast::{AstError, parse_project_with_limits};
use crate::config::{Config, ConfigError};
use crate::diff::{DiffError, DiffProvider};
use crate::domain::{AnalysisStatus, ChangedPath, ParsedFile};
use crate::gates::{
    ApprovalChecker, GateError, GateResult, ValidationExecutor, evaluate_gate, evaluate_gate_rule,
};
use crate::report::{ReportInput, render_markdown_limited};
use crate::risk::{RiskAssessment, assess_risk_with_impact};
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
    let changed = diff.changed_paths()?;
    if changed.is_empty() {
        return Err(AnalysisError::EmptyDiff);
    }
    let parsed_files =
        parse_project_with_limits(root, config.limits.max_files, config.limits.max_ast_nodes)?;
    let assessment = assess_risk_with_impact(&config.risk_map, &changed, &parsed_files);
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
}
