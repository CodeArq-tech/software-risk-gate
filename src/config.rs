use crate::domain::RiskLevel;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot read configuration {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("configuration version must be 1")]
    UnsupportedVersion,
    #[error("riskMap must contain at least one rule")]
    EmptyRiskMap,
    #[error("riskMap contains duplicate path: {0}")]
    DuplicatePath(String),
    #[error("ignorePaths contains duplicate path: {0}")]
    DuplicateIgnorePath(String),
    #[error("riskMap path cannot escape the repository: {0}")]
    EscapingPath(String),
    #[error("validation '{0}' is referenced but not defined")]
    MissingValidation(String),
    #[error("approval group '{0}' is referenced but not defined")]
    MissingApprovalGroup(String),
    #[error("validation '{0}' must have a positive timeout")]
    InvalidTimeout(String),
    #[error("validation '{0}' must define a non-empty command")]
    InvalidCommand(String),
    #[error("approval group '{0}' must define a team or at least one user")]
    InvalidApprovalGroup(String),
    #[error("riskMap shorthand must contain exactly one path/risk entry")]
    InvalidRiskMapShorthand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum NewCodePolicy {
    #[default]
    Include,
    Ignore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u8,
    #[serde(rename = "riskMap")]
    pub risk_map: Vec<RiskRule>,
    #[serde(rename = "ignorePaths", default)]
    pub ignore_paths: Vec<String>,
    #[serde(rename = "gateRules", default)]
    pub gate_rules: BTreeMap<RiskLevel, GateRule>,
    #[serde(default)]
    pub validations: BTreeMap<String, ValidationConfig>,
    #[serde(rename = "approvalGroups", default)]
    pub approval_groups: BTreeMap<String, ApprovalGroup>,
    #[serde(default)]
    pub limits: ResourceLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceLimits {
    #[serde(default = "default_max_files")]
    pub max_files: usize,
    #[serde(default = "default_max_ast_nodes")]
    pub max_ast_nodes: usize,
    #[serde(default = "default_max_comment_bytes")]
    pub max_comment_bytes: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_files: default_max_files(),
            max_ast_nodes: default_max_ast_nodes(),
            max_comment_bytes: default_max_comment_bytes(),
        }
    }
}

fn default_max_files() -> usize {
    10_000
}
fn default_max_ast_nodes() -> usize {
    1_000_000
}
fn default_max_comment_bytes() -> usize {
    60_000
}

#[derive(Debug, Clone, Serialize)]
pub struct RiskRule {
    pub path: String,
    pub risk: RiskLevel,
    pub description: Option<String>,
    #[serde(rename = "newCodePolicy", default)]
    pub new_code_policy: NewCodePolicy,
}

impl<'de> Deserialize<'de> for RiskRule {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Canonical {
            path: String,
            risk: RiskLevel,
            #[serde(default)]
            description: Option<String>,
            #[serde(rename = "newCodePolicy", default)]
            new_code_policy: NewCodePolicy,
        }

        let value = serde_yaml::Value::deserialize(deserializer)?;
        if let Ok(canonical) = Canonical::deserialize(value.clone()) {
            return Ok(Self {
                path: canonical.path,
                risk: canonical.risk,
                description: canonical.description,
                new_code_policy: canonical.new_code_policy,
            });
        }
        let mapping = value
            .as_mapping()
            .ok_or_else(|| serde::de::Error::custom("riskMap rule must be a mapping"))?;
        if mapping.len() != 1 {
            return Err(serde::de::Error::custom(
                "riskMap shorthand must contain exactly one path/risk entry",
            ));
        }
        let (path, risk) = mapping.iter().next().expect("mapping length checked");
        let path = path
            .as_str()
            .ok_or_else(|| serde::de::Error::custom("riskMap path must be a string"))?;
        let risk = RiskLevel::deserialize(risk.clone()).map_err(serde::de::Error::custom)?;
        Ok(Self {
            path: path.to_owned(),
            risk,
            description: None,
            new_code_policy: NewCodePolicy::Include,
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateRule {
    #[serde(rename = "instantMerge", default)]
    pub instant_merge: bool,
    #[serde(rename = "requireApprovalGroup", default)]
    pub require_approval_group: Option<String>,
    #[serde(default)]
    pub validations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ValidationConfig {
    Command(String),
    Detailed {
        command: String,
        #[serde(rename = "timeoutSeconds", default)]
        timeout_seconds: Option<u64>,
    },
}

impl ValidationConfig {
    pub fn command(&self) -> &str {
        match self {
            Self::Command(command) | Self::Detailed { command, .. } => command,
        }
    }

    pub fn timeout_seconds(&self) -> u64 {
        match self {
            Self::Command(_) => 1800,
            Self::Detailed {
                timeout_seconds, ..
            } => timeout_seconds.unwrap_or(1800),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalGroup {
    #[serde(default)]
    pub team: Option<String>,
    #[serde(default)]
    pub users: Vec<String>,
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref().to_path_buf();
        let contents =
            fs::read_to_string(&path).map_err(|source| ConfigError::Read { path, source })?;
        let config: Self = serde_yaml::from_str(&contents)?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.version != 1 {
            return Err(ConfigError::UnsupportedVersion);
        }
        if self.risk_map.is_empty() {
            return Err(ConfigError::EmptyRiskMap);
        }
        if self.limits.max_files == 0
            || self.limits.max_ast_nodes == 0
            || self.limits.max_comment_bytes == 0
        {
            return Err(ConfigError::InvalidTimeout(
                "resource limits must be positive".into(),
            ));
        }
        let mut paths = BTreeSet::new();
        for rule in &self.risk_map {
            let normalized = normalize_path(&rule.path)?;
            if !paths.insert(normalized) {
                return Err(ConfigError::DuplicatePath(rule.path.clone()));
            }
        }
        let mut ignored_paths = BTreeSet::new();
        for path in &self.ignore_paths {
            let normalized = normalize_path(path)?;
            if !ignored_paths.insert(normalized) {
                return Err(ConfigError::DuplicateIgnorePath(path.clone()));
            }
        }
        for gate in self.gate_rules.values() {
            for validation in &gate.validations {
                let Some(definition) = self.validations.get(validation) else {
                    return Err(ConfigError::MissingValidation(validation.clone()));
                };
                if definition.command().trim().is_empty() {
                    return Err(ConfigError::InvalidCommand(validation.clone()));
                }
                if definition.timeout_seconds() == 0 {
                    return Err(ConfigError::InvalidTimeout(validation.clone()));
                }
            }
            if let Some(group) = &gate.require_approval_group
                && !self.approval_groups.contains_key(group)
            {
                return Err(ConfigError::MissingApprovalGroup(group.clone()));
            }
        }
        for (name, group) in &self.approval_groups {
            if group.team.as_deref().is_none_or(str::is_empty) && group.users.is_empty() {
                return Err(ConfigError::InvalidApprovalGroup(name.clone()));
            }
        }
        Ok(())
    }
}

pub fn normalize_path(path: &str) -> Result<String, ConfigError> {
    let path = path.replace('\\', "/");
    let is_dir = path.ends_with('/');
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => return Err(ConfigError::EscapingPath(path)),
            part => parts.push(part),
        }
    }
    if parts.is_empty() {
        return Ok("./".to_owned());
    }
    let result = parts.join("/");
    Ok(if is_dir { format!("{result}/") } else { result })
}

pub fn path_matches_rule(path: &str, rule: &str) -> bool {
    let normalized_rule = rule.replace('\\', "/");
    if normalized_rule == "./" {
        return true;
    }
    if normalized_rule.ends_with('/') {
        path.starts_with(&normalized_rule)
    } else {
        path == normalized_rule || path.starts_with(&(normalized_rule + "/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn valid_yaml() -> &'static str {
        r#"
version: 1
riskMap:
  - path: src/payment/
    risk: danger
    description: Payments
    newCodePolicy: ignore
  - tests/: minimal
ignorePaths:
  - generated/
gateRules:
  danger:
    instantMerge: false
    validations: [e2e]
  high:
    requireApprovalGroup: architects
validations:
  e2e:
    command: ./test-e2e.sh
    timeoutSeconds: 60
approvalGroups:
  architects:
    users: [alice, bob]
"#
    }

    fn load_yaml(yaml: &str) -> Result<Config, ConfigError> {
        let mut file = NamedTempFile::new().expect("temp file");
        file.write_all(yaml.as_bytes()).expect("write yaml");
        Config::load(file.path())
    }

    #[test]
    fn loads_canonical_and_shorthand_risk_rules() {
        let config = load_yaml(valid_yaml()).expect("valid config");
        assert_eq!(config.risk_map.len(), 2);
        assert_eq!(config.risk_map[0].risk, RiskLevel::Danger);
        assert_eq!(config.risk_map[1].path, "tests/");
        assert_eq!(config.risk_map[0].new_code_policy, NewCodePolicy::Ignore);
        assert_eq!(config.ignore_paths, vec!["generated/"]);
        assert_eq!(config.validations["e2e"].timeout_seconds(), 60);
        assert_eq!(config.limits.max_files, 10_000);
    }

    #[test]
    fn rejects_missing_validation_and_approval_group() {
        let yaml = r#"
version: 1
riskMap:
  - path: src/
    risk: normal
gateRules:
  normal:
    validations: [missing]
  high:
    requireApprovalGroup: missing-group
"#;
        let error = load_yaml(yaml).expect_err("invalid references");
        assert!(matches!(error, ConfigError::MissingValidation(name) if name == "missing"));
    }

    #[test]
    fn rejects_duplicates_and_escaping_paths() {
        let duplicate = r#"
version: 1
riskMap:
  - path: src/
    risk: low
  - path: src/
    risk: high
"#;
        assert!(matches!(
            load_yaml(duplicate),
            Err(ConfigError::DuplicatePath(_))
        ));
        let duplicate_ignore = r#"
version: 1
riskMap:
  - path: src/
    risk: low
ignorePaths: [generated/, generated/]
"#;
        assert!(matches!(
            load_yaml(duplicate_ignore),
            Err(ConfigError::DuplicateIgnorePath(_))
        ));
        assert!(matches!(
            normalize_path("../secrets"),
            Err(ConfigError::EscapingPath(_))
        ));
        assert_eq!(normalize_path("./").unwrap(), "./");
    }

    #[test]
    fn rejects_invalid_version_timeout_and_unknown_fields() {
        assert!(matches!(
            load_yaml("version: 2\nriskMap:\n  - path: src/\n    risk: low\n"),
            Err(ConfigError::UnsupportedVersion)
        ));
        let zero_timeout = "version: 1\nriskMap:\n  - path: src/\n    risk: low\ngateRules:\n  low:\n    validations: [check]\nvalidations:\n  check:\n    command: 'true'\n    timeoutSeconds: 0\n";
        assert!(
            matches!(load_yaml(zero_timeout), Err(ConfigError::InvalidTimeout(name)) if name == "check")
        );
        let unknown = "version: 1\nriskMap:\n  - path: src/\n    risk: low\nextra: true\n";
        assert!(load_yaml(unknown).is_err());
        let missing_group = "version: 1\nriskMap:\n  - path: src/\n    risk: low\ngateRules:\n  low:\n    requireApprovalGroup: architects\n";
        assert!(
            matches!(load_yaml(missing_group), Err(ConfigError::MissingApprovalGroup(name)) if name == "architects")
        );
        let invalid_limits =
            "version: 1\nriskMap:\n  - path: src/\n    risk: low\nlimits:\n  maxFiles: 0\n";
        assert!(matches!(
            load_yaml(invalid_limits),
            Err(ConfigError::InvalidTimeout(_))
        ));
        for field in ["maxFiles", "maxAstNodes", "maxCommentBytes"] {
            let yaml = format!(
                "version: 1\nriskMap:\n  - path: src/\n    risk: low\nlimits:\n  {field}: 0\n"
            );
            assert!(load_yaml(&yaml).is_err());
        }
        let empty_command = "version: 1\nriskMap:\n  - path: src/\n    risk: low\ngateRules:\n  low:\n    validations: [check]\nvalidations:\n  check: '   '\n";
        assert!(
            matches!(load_yaml(empty_command), Err(ConfigError::InvalidCommand(name)) if name == "check")
        );
        let empty_group = "version: 1\nriskMap:\n  - path: src/\n    risk: low\napprovalGroups:\n  architects: {}\n";
        assert!(
            matches!(load_yaml(empty_group), Err(ConfigError::InvalidApprovalGroup(name)) if name == "architects")
        );
    }

    #[test]
    fn normalizes_windows_and_dot_segments() {
        assert_eq!(normalize_path("./src\\payment\\").unwrap(), "src/payment/");
    }
}
