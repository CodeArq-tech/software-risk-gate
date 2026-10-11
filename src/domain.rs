use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Minimal,
    Low,
    Normal,
    High,
    Danger,
}

impl RiskLevel {
    pub const ALL: [Self; 5] = [
        Self::Minimal,
        Self::Low,
        Self::Normal,
        Self::High,
        Self::Danger,
    ];
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Danger => "danger",
        };
        f.write_str(value)
    }
}

impl FromStr for RiskLevel {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "minimal" => Ok(Self::Minimal),
            "low" => Ok(Self::Low),
            "normal" => Ok(Self::Normal),
            "high" => Ok(Self::High),
            "danger" => Ok(Self::Danger),
            _ => Err(format!("unknown risk level: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AnalysisStatus {
    Success,
    Partial,
    Failed,
}

impl fmt::Display for AnalysisStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Success => "SUCCESS",
            Self::Partial => "PARTIAL",
            Self::Failed => "FAILED",
        };
        f.write_str(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_levels_are_ordered_and_parseable() {
        assert!(RiskLevel::Minimal < RiskLevel::Danger);
        assert_eq!(RiskLevel::from_str("HIGH").unwrap(), RiskLevel::High);
        for level in RiskLevel::ALL {
            assert_eq!(RiskLevel::from_str(&level.to_string()).unwrap(), level);
        }
        assert!(RiskLevel::from_str("critical").is_err());
        assert_eq!(RiskLevel::ALL.len(), 5);
        assert_eq!(RiskLevel::Danger.to_string(), "danger");
    }

    #[test]
    fn analysis_status_has_stable_public_names() {
        assert_eq!(AnalysisStatus::Success.to_string(), "SUCCESS");
        assert_eq!(AnalysisStatus::Partial.to_string(), "PARTIAL");
        assert_eq!(AnalysisStatus::Failed.to_string(), "FAILED");
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedPath {
    pub path: String,
    #[serde(default)]
    pub old_path: Option<String>,
    #[serde(default)]
    pub is_new: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstNode {
    pub kind: String,
    pub language: String,
    pub path: String,
    pub start_byte: usize,
    pub end_byte: usize,
    #[serde(default)]
    pub name: Option<String>,
    pub children: Vec<AstNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedFile {
    pub path: String,
    pub language: Option<String>,
    pub root: Option<AstNode>,
    pub error: Option<String>,
    #[serde(default)]
    pub references: Vec<String>,
}
