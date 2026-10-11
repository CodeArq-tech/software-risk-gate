use crate::config::{ApprovalGroup, Config, GateRule, ValidationConfig};
use crate::domain::{AnalysisStatus, RiskLevel};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    pub name: String,
    pub status: AnalysisStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateResult {
    pub risk: RiskLevel,
    pub instant_merge: bool,
    pub validations: Vec<ValidationResult>,
    pub approval: Option<ApprovalCheck>,
    pub merge_blocked: bool,
    pub status: AnalysisStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalCheck {
    pub group: String,
    pub satisfied: bool,
    pub approved_by: Vec<String>,
    pub required: Vec<String>,
}

#[derive(Debug, Error)]
pub enum GateError {
    #[error("validation '{0}' is not configured")]
    MissingValidation(String),
    #[error("approval group '{0}' could not be checked in this execution")]
    ApprovalUnavailable(String),
    #[error("cannot start validation '{name}': {source}")]
    Spawn {
        name: String,
        source: std::io::Error,
    },
}

pub trait ValidationExecutor {
    fn execute(&self, name: &str, config: &ValidationConfig)
    -> Result<ValidationResult, GateError>;
}

pub trait ApprovalChecker {
    fn check(&self, group_name: &str, group: &ApprovalGroup) -> Result<ApprovalCheck, GateError>;
}

#[derive(Debug, Clone)]
pub struct ShellExecutor {
    pub working_directory: std::path::PathBuf,
    pub output_limit: usize,
}

impl ShellExecutor {
    pub fn new(working_directory: impl Into<std::path::PathBuf>) -> Self {
        Self {
            working_directory: working_directory.into(),
            output_limit: 32 * 1024,
        }
    }
}

impl ValidationExecutor for ShellExecutor {
    fn execute(
        &self,
        name: &str,
        config: &ValidationConfig,
    ) -> Result<ValidationResult, GateError> {
        let started = Instant::now();
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(config.command())
            .current_dir(&self.working_directory)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| GateError::Spawn {
                name: name.to_owned(),
                source,
            })?;
        let timeout = Duration::from_secs(config.timeout_seconds());
        let mut timed_out = false;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if started.elapsed() >= timeout => {
                    timed_out = true;
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(_) => break,
            }
        }
        let output = child
            .wait_with_output()
            .map_err(|source| GateError::Spawn {
                name: name.to_owned(),
                source,
            })?;
        let stdout = truncate(
            String::from_utf8_lossy(&output.stdout).as_ref(),
            self.output_limit,
        );
        let stderr = truncate(
            String::from_utf8_lossy(&output.stderr).as_ref(),
            self.output_limit,
        );
        let status = if timed_out {
            AnalysisStatus::Failed
        } else if output.status.success() {
            AnalysisStatus::Success
        } else {
            AnalysisStatus::Failed
        };
        Ok(ValidationResult {
            name: name.to_owned(),
            status,
            exit_code: output.status.code(),
            stdout,
            stderr,
            duration_ms: started.elapsed().as_millis(),
        })
    }
}

pub fn evaluate_gate<E: ValidationExecutor>(
    config: &Config,
    risk: RiskLevel,
    executor: &E,
) -> Result<GateResult, GateError> {
    let rule = config.gate_rules.get(&risk).cloned().unwrap_or_default();
    evaluate_gate_rule(config, risk, &rule, executor, None)
}

pub fn evaluate_gate_rule<E: ValidationExecutor>(
    config: &Config,
    risk: RiskLevel,
    rule: &GateRule,
    executor: &E,
    approval_checker: Option<&dyn ApprovalChecker>,
) -> Result<GateResult, GateError> {
    let mut validations = Vec::with_capacity(rule.validations.len());
    for name in &rule.validations {
        let definition = config
            .validations
            .get(name)
            .ok_or_else(|| GateError::MissingValidation(name.clone()))?;
        validations.push(executor.execute(name, definition)?);
    }
    let approval = if let Some(group_name) = &rule.require_approval_group {
        let group = config
            .approval_groups
            .get(group_name)
            .ok_or_else(|| GateError::ApprovalUnavailable(group_name.clone()))?;
        let Some(checker) = approval_checker else {
            return Err(GateError::ApprovalUnavailable(group_name.clone()));
        };
        Some(checker.check(group_name, group)?)
    } else {
        None
    };
    let validations_ok = validations
        .iter()
        .all(|result| result.status == AnalysisStatus::Success);
    let approval_ok = approval.as_ref().is_none_or(|check| check.satisfied);
    let technical_success = validations_ok && approval_ok;
    let merge_blocked = !technical_success && !rule.instant_merge;
    let status = if technical_success {
        AnalysisStatus::Success
    } else {
        AnalysisStatus::Failed
    };
    Ok(GateResult {
        risk,
        instant_merge: rule.instant_merge,
        validations,
        approval,
        merge_blocked,
        status,
    })
}

fn truncate(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        value.to_owned()
    } else {
        format!("{}…", &value[..limit.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ApprovalGroup, Config, RiskRule};
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    struct FakeExecutor {
        results: BTreeMap<String, AnalysisStatus>,
    }
    impl ValidationExecutor for FakeExecutor {
        fn execute(
            &self,
            name: &str,
            _config: &ValidationConfig,
        ) -> Result<ValidationResult, GateError> {
            Ok(ValidationResult {
                name: name.to_owned(),
                status: *self.results.get(name).unwrap_or(&AnalysisStatus::Failed),
                exit_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
                duration_ms: 1,
            })
        }
    }

    fn config(rule: GateRule) -> Config {
        let mut gate_rules = BTreeMap::new();
        gate_rules.insert(RiskLevel::Danger, rule);
        Config {
            version: 1,
            risk_map: vec![RiskRule {
                path: "src/".into(),
                risk: RiskLevel::Danger,
                description: None,
                new_code_policy: Default::default(),
            }],
            gate_rules,
            validations: BTreeMap::new(),
            approval_groups: BTreeMap::<String, ApprovalGroup>::new(),
            limits: Default::default(),
        }
    }

    #[test]
    fn failed_validation_blocks_without_instant_merge() {
        let mut config = config(GateRule {
            validations: vec!["e2e".into()],
            ..GateRule::default()
        });
        config
            .validations
            .insert("e2e".into(), ValidationConfig::Command("false".into()));
        let result = evaluate_gate(
            &config,
            RiskLevel::Danger,
            &FakeExecutor {
                results: BTreeMap::new(),
            },
        )
        .unwrap();
        assert_eq!(result.status, AnalysisStatus::Failed);
        assert!(result.merge_blocked);
    }

    #[test]
    fn instant_merge_does_not_hide_failure_but_does_not_block() {
        let mut config = config(GateRule {
            instant_merge: true,
            validations: vec!["e2e".into()],
            ..GateRule::default()
        });
        config
            .validations
            .insert("e2e".into(), ValidationConfig::Command("false".into()));
        let result = evaluate_gate(
            &config,
            RiskLevel::Danger,
            &FakeExecutor {
                results: BTreeMap::new(),
            },
        )
        .unwrap();
        assert_eq!(result.status, AnalysisStatus::Failed);
        assert!(!result.merge_blocked);
    }

    #[test]
    fn shell_executor_captures_success_and_failure() {
        let directory = tempdir().unwrap();
        let executor = ShellExecutor::new(directory.path());
        let success = executor
            .execute("ok", &ValidationConfig::Command("printf ok".into()))
            .unwrap();
        assert_eq!(success.status, AnalysisStatus::Success);
        assert_eq!(success.stdout, "ok");
        let failure = executor
            .execute(
                "fail",
                &ValidationConfig::Command("printf bad >&2; exit 3".into()),
            )
            .unwrap();
        assert_eq!(failure.status, AnalysisStatus::Failed);
        assert_eq!(failure.exit_code, Some(3));
        assert_eq!(failure.stderr, "bad");
    }

    #[test]
    fn reports_missing_validation_and_timeout() {
        let config = config(GateRule {
            validations: vec!["missing".into()],
            ..GateRule::default()
        });
        assert!(
            matches!(evaluate_gate(&config, RiskLevel::Danger, &FakeExecutor { results: BTreeMap::new() }), Err(GateError::MissingValidation(name)) if name == "missing")
        );
        let directory = tempdir().unwrap();
        let executor = ShellExecutor::new(directory.path());
        let result = executor
            .execute(
                "timeout",
                &ValidationConfig::Detailed {
                    command: "sleep 1".into(),
                    timeout_seconds: Some(1),
                },
            )
            .unwrap();
        assert_eq!(result.status, AnalysisStatus::Failed);
    }

    #[test]
    fn requires_a_checker_for_approval_groups() {
        let mut config = config(GateRule {
            require_approval_group: Some("architects".into()),
            ..GateRule::default()
        });
        config.approval_groups.insert(
            "architects".into(),
            ApprovalGroup {
                team: None,
                users: vec!["alice".into()],
            },
        );
        assert!(
            matches!(evaluate_gate(&config, RiskLevel::Danger, &FakeExecutor { results: BTreeMap::new() }), Err(GateError::ApprovalUnavailable(name)) if name == "architects")
        );
    }
}
