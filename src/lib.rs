pub mod ast;
pub mod config;
pub mod diff;
pub mod domain;
pub mod engine;
pub mod gates;
pub mod github;
pub mod report;
pub mod risk;
pub mod telemetry;

pub use config::{Config, ConfigError};
pub use domain::{AnalysisStatus, RiskLevel};
pub use risk::{RiskAssessment, assess_risk};
