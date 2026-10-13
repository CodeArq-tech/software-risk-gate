use crate::domain::{AnalysisStatus, ParsedFile, RiskLevel};
use crate::gates::GateResult;
use crate::risk::{ImpactEvidence, RiskAssessment};
use std::collections::{BTreeMap, BTreeSet};

pub const REPORT_MARKER: &str = "<!-- risk-gate:report -->";
const MAX_IMPACT_CHAINS: usize = 24;
const MAX_IMPACT_NODES: usize = 96;

#[derive(Debug, Clone)]
pub struct ReportInput<'a> {
    pub assessment: &'a RiskAssessment,
    pub gate: Option<&'a GateResult>,
    pub parsed_files: &'a [ParsedFile],
    pub version: &'a str,
}

pub fn render_markdown(input: &ReportInput<'_>) -> String {
    render_markdown_limited(input, usize::MAX)
}

pub fn render_markdown_limited(input: &ReportInput<'_>, max_bytes: usize) -> String {
    let parse_errors = input
        .parsed_files
        .iter()
        .filter(|file| file.error.is_some())
        .collect::<Vec<_>>();
    let technical_partial = !parse_errors.is_empty();
    let gate_status = input
        .gate
        .map(|gate| gate.status)
        .unwrap_or(AnalysisStatus::Success);
    let status = if gate_status == AnalysisStatus::Failed {
        AnalysisStatus::Failed
    } else if technical_partial {
        AnalysisStatus::Partial
    } else {
        AnalysisStatus::Success
    };
    let blocked = input.gate.is_some_and(|gate| gate.merge_blocked);

    let mut output = String::new();
    output.push_str(REPORT_MARKER);
    output.push_str("\n## Risk Gate\n\n");
    output.push_str(&format!("- **Risk:** `{}`\n", input.assessment.risk));
    output.push_str(&format!("- **Status:** `{status}`\n"));
    output.push_str(&format!("- **Merge blocked:** `{blocked}`\n"));
    output.push_str(&format!(
        "- **Version:** `{}`\n\n",
        escape_code(input.version)
    ));
    output.push_str("<details>\n<summary>Analysis details</summary>\n\n");
    output.push_str("### Why this risk\n\n");
    if input.assessment.evidence.is_empty() {
        output.push_str("No changed files were available for risk classification.\n\n");
    } else {
        for evidence in &input.assessment.evidence {
            output.push_str(&format!(
                "- `{}` → **{}**",
                escape_code(&evidence.path),
                evidence.risk
            ));
            if let Some(rule) = &evidence.matched_rule {
                output.push_str(&format!(" (rule `{}`)", escape_code(rule)));
            }
            if let Some(description) = &evidence.description {
                output.push_str(&format!(": {}", escape_text(description)));
            }
            if evidence.ignored_isolated_new_code {
                output.push_str("; isolated new code ignored by `newCodePolicy: ignore`");
            }
            if !evidence.modules.is_empty() {
                output.push_str(&format!(
                    "; modules: {}",
                    evidence
                        .modules
                        .iter()
                        .map(|module| format!("`{}`", escape_code(module)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            output.push('\n');
        }
        output.push('\n');
    }
    output.push_str("</details>\n\n");
    if let Some((highest_risk, impacts, omitted)) = summarize_impacts(input.assessment) {
        output.push_str("### Cascade impact summary\n\n");
        output.push_str(&format!(
            "Highest propagated risk: `{highest_risk}`; chains shown: `{}`.\n\n",
            impacts.len()
        ));
        if omitted > 0 {
            output.push_str(&format!(
                "`{omitted}` lower-priority or duplicate impact chain(s) were omitted to keep this comment readable.\n\n"
            ));
        }
        output.push_str("<details>\n<summary>Impact chain details</summary>\n\n");
        for impact in impacts {
            output.push_str(&format!(
                "- `{}` is connected to `{}` → **{}** (rule `{}`)",
                escape_code(&impact.changed_path),
                escape_code(&impact.related_path),
                impact.risk,
                escape_code(&impact.rule)
            ));
            if let Some(description) = &impact.description {
                output.push_str(&format!(": {}", escape_text(description)));
            }
            if impact.path.len() > 1 {
                output.push_str(&format!(
                    "; chain: {}",
                    impact
                        .path
                        .iter()
                        .map(|path| format!("`{}`", escape_code(path)))
                        .collect::<Vec<_>>()
                        .join(" → ")
                ));
            }
            output.push('\n');
        }
        output.push_str("</details>\n\n");
        output.push_str(&render_risk_legend());
        output.push_str(&render_impact_mermaid(input.assessment));
    }
    output.push_str("<details>\n<summary>Technical details</summary>\n\n");
    output.push_str("### AST analysis\n\n");
    output.push_str(&format!(
        "Parsed files: `{}`; incomplete files: `{}`.\n\n",
        input.parsed_files.len(),
        parse_errors.len()
    ));
    if !parse_errors.is_empty() {
        output.push_str("The following files require attention:\n\n");
        for file in parse_errors {
            output.push_str(&format!(
                "- `{}`: {}\n",
                escape_code(&file.path),
                escape_text(file.error.as_deref().unwrap_or("unknown error"))
            ));
        }
        output.push('\n');
    }
    if let Some(gate) = input.gate {
        if let Some(approval) = &gate.approval {
            output.push_str("### Approval\n\n");
            output.push_str(&format!("- Group: `{}`\n", escape_code(&approval.group)));
            output.push_str(&format!("- Satisfied: `{}`\n", approval.satisfied));
            output.push_str(&format!(
                "- Approved by: {}\n",
                if approval.approved_by.is_empty() {
                    "none".to_owned()
                } else {
                    approval
                        .approved_by
                        .iter()
                        .map(|user| format!("`{}`", escape_code(user)))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ));
            if !approval.required.is_empty() {
                output.push_str(&format!(
                    "- Eligible approvers: {}\n",
                    approval
                        .required
                        .iter()
                        .map(|user| format!("`{}`", escape_code(user)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            output.push('\n');
        }
        output.push_str("### Validations\n\n");
        if gate.validations.is_empty() {
            output.push_str("No validations were required.\n\n");
        }
        for validation in &gate.validations {
            output.push_str(&format!(
                "- `{}`: `{}` (exit: `{}`; {} ms)\n",
                escape_code(&validation.name),
                validation.status,
                validation
                    .exit_code
                    .map_or_else(|| "none".to_owned(), |code| code.to_string()),
                validation.duration_ms
            ));
            if !validation.stdout.is_empty() {
                output.push_str(&format!(
                    "  - stdout: `{}`\n",
                    escape_code(&one_line(&validation.stdout))
                ));
            }
            if !validation.stderr.is_empty() {
                output.push_str(&format!(
                    "  - stderr: `{}`\n",
                    escape_code(&one_line(&validation.stderr))
                ));
            }
        }
        output.push('\n');
    }
    output.push_str("</details>\n\n");
    output.push_str("_Generated by risk-gate._\n");
    truncate_utf8(output, max_bytes)
}

fn render_impact_mermaid(assessment: &RiskAssessment) -> String {
    let Some((_, impacts, _)) = summarize_impacts(assessment) else {
        return String::new();
    };
    let mut nodes = Vec::new();
    for impact in &impacts {
        for path in &impact.path {
            if nodes.len() == MAX_IMPACT_NODES {
                break;
            }
            if !nodes.iter().any(|known: &String| known == path) {
                nodes.push(path.clone());
            }
        }
        if nodes.len() == MAX_IMPACT_NODES {
            break;
        }
    }
    let ids = nodes
        .iter()
        .enumerate()
        .map(|(index, path)| (path.clone(), format!("impact_{index}")))
        .collect::<BTreeMap<_, _>>();
    let mut node_risks = BTreeMap::new();
    for impact in &impacts {
        for path in &impact.path {
            node_risks
                .entry(path.clone())
                .and_modify(|risk: &mut RiskLevel| *risk = (*risk).max(impact.risk))
                .or_insert(impact.risk);
        }
    }
    let mut output = String::from(
        "```mermaid\n%%{init: {\"flowchart\": {\"htmlLabels\": true, \"nodeSpacing\": 50, \"rankSpacing\": 90, \"curve\": \"basis\"}, \"themeVariables\": {\"fontSize\": \"16px\", \"primaryTextColor\": \"#000000\", \"secondaryTextColor\": \"#000000\", \"tertiaryTextColor\": \"#000000\", \"lineColor\": \"#374151\", \"edgeLabelBackground\": \"#ffffff\"}}}%%\nflowchart TD\n",
    );
    for path in &nodes {
        let id = &ids[path];
        output.push_str(&format!("    {id}[\"{}\"]\n", escape_mermaid_label(path)));
    }
    output.push_str(
        "    classDef risk_minimal fill:#dcfce7,stroke:#15803d,color:#000000,stroke-width:2px\n    classDef risk_low fill:#dbeafe,stroke:#1d4ed8,color:#000000,stroke-width:2px\n    classDef risk_normal fill:#fef3c7,stroke:#b45309,color:#000000,stroke-width:2px\n    classDef risk_high fill:#fed7aa,stroke:#c2410c,color:#000000,stroke-width:2px\n    classDef risk_danger fill:#fecaca,stroke:#b91c1c,color:#000000,stroke-width:3px\n",
    );
    for path in &nodes {
        if let Some(id) = ids.get(path) {
            let class = risk_class(node_risks.get(path).copied().unwrap_or(RiskLevel::Normal));
            output.push_str(&format!("    class {id} {class}\n"));
        }
    }
    let mut edges = BTreeSet::new();
    for impact in &impacts {
        for (step, pair) in impact.path.windows(2).enumerate() {
            if let (Some(from), Some(to)) = (ids.get(&pair[0]), ids.get(&pair[1]))
                && edges.insert((from.clone(), to.clone()))
            {
                output.push_str(&format!("    {from} -->|\"step {}\"| {to}\n", step + 1));
            }
        }
    }
    output.push_str("```\n\n");
    output
}

fn render_risk_legend() -> String {
    "**Risk legend:** 🟩 minimal · 🟦 low · 🟨 normal · 🟧 high · 🟥 danger\n\n".to_owned()
}

fn risk_class(risk: RiskLevel) -> &'static str {
    match risk {
        RiskLevel::Minimal => "risk_minimal",
        RiskLevel::Low => "risk_low",
        RiskLevel::Normal => "risk_normal",
        RiskLevel::High => "risk_high",
        RiskLevel::Danger => "risk_danger",
    }
}

fn summarize_impacts(
    assessment: &RiskAssessment,
) -> Option<(crate::domain::RiskLevel, Vec<&ImpactEvidence>, usize)> {
    let highest_risk = assessment.impact.iter().map(|impact| impact.risk).max()?;
    let mut candidates = assessment.impact.iter().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        (
            std::cmp::Reverse(left.risk),
            &left.path,
            &left.changed_path,
            &left.related_path,
            &left.rule,
        )
            .cmp(&(
                std::cmp::Reverse(right.risk),
                &right.path,
                &right.changed_path,
                &right.related_path,
                &right.rule,
            ))
    });
    candidates.dedup_by(|left, right| left.path == right.path);
    let omitted = candidates.len().saturating_sub(MAX_IMPACT_CHAINS);
    candidates.truncate(MAX_IMPACT_CHAINS);
    Some((highest_risk, candidates, omitted))
}

fn escape_mermaid_label(value: &str) -> String {
    value
        .split('/')
        .map(|segment| {
            let escaped = segment
                .replace('&', "&amp;")
                .replace('"', "&quot;")
                .replace(['\n', '\r'], " ");
            escaped
                .chars()
                .collect::<Vec<_>>()
                .chunks(28)
                .map(|chunk| chunk.iter().collect::<String>())
                .collect::<Vec<_>>()
                .join("<br/>")
        })
        .collect::<Vec<_>>()
        .join("<br/>")
}

fn truncate_utf8(mut value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value;
    }
    let suffix = "\n\n_Report truncated by configured resource limit._\n";
    if max_bytes <= suffix.len() {
        value.truncate(utf8_prefix_len(&value, max_bytes));
        return value;
    }
    let content_limit = max_bytes - suffix.len();
    value.truncate(utf8_prefix_len(&value, content_limit));
    value.push_str(suffix);
    value
}

fn utf8_prefix_len(value: &str, max_bytes: usize) -> usize {
    value
        .char_indices()
        .find_map(|(index, character)| (index + character.len_utf8() > max_bytes).then_some(index))
        .unwrap_or(value.len())
}

pub fn replace_marked_comment(existing: &[String], body: &str) -> Option<(i64, String)> {
    existing
        .iter()
        .enumerate()
        .find(|(_, comment)| comment.contains(REPORT_MARKER))
        .map(|(index, _)| (index as i64, body.to_owned()))
}

fn escape_code(value: &str) -> String {
    value.replace('`', "\\`").replace('\n', " ")
}
fn escape_text(value: &str) -> String {
    value.replace(['\n', '\r'], " ")
}
fn one_line(value: &str) -> String {
    value.lines().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ChangedPath, RiskLevel};
    use crate::gates::{ApprovalCheck, ValidationResult};
    use crate::risk::assess_risk;

    #[test]
    fn renders_risk_evidence_validation_and_safe_output() {
        let assessment = assess_risk(
            &[crate::config::RiskRule {
                path: "src/".into(),
                risk: RiskLevel::High,
                description: Some("danger `code`\narea".into()),
                new_code_policy: Default::default(),
            }],
            &[ChangedPath {
                path: "src/a.py".into(),
                old_path: None,
                is_new: false,
            }],
            &[],
        );
        let input = ReportInput {
            assessment: &assessment,
            gate: None,
            parsed_files: &[],
            version: "0.1.0",
        };
        let report = render_markdown(&input);
        assert!(report.starts_with(REPORT_MARKER));
        assert!(report.contains("`src/a.py`"));
        assert!(report.contains("danger `code` area"));
        assert!(!report.contains("danger `code`\narea"));
    }

    #[test]
    fn finds_existing_marked_comment_without_creating_duplicate() {
        let comments = vec!["hello".into(), format!("{REPORT_MARKER}\nold")];
        let replacement = replace_marked_comment(&comments, "new").unwrap();
        assert_eq!(replacement.0, 1);
        assert_eq!(replacement.1, "new");
    }

    #[test]
    fn renders_partial_ast_and_gate_details() {
        let assessment = RiskAssessment {
            risk: RiskLevel::Minimal,
            evidence: vec![],
            impact: vec![],
        };
        let gate = GateResult {
            risk: RiskLevel::Minimal,
            instant_merge: false,
            validations: vec![ValidationResult {
                name: "e2e".into(),
                status: AnalysisStatus::Failed,
                exit_code: Some(2),
                stdout: "out\nline".into(),
                stderr: "err".into(),
                duration_ms: 4,
            }],
            approval: Some(ApprovalCheck {
                group: "architects".into(),
                satisfied: false,
                approved_by: vec![],
                required: vec!["alice".into()],
            }),
            merge_blocked: true,
            status: AnalysisStatus::Failed,
        };
        let files = vec![ParsedFile {
            path: "unknown.bin".into(),
            language: None,
            root: None,
            error: Some("unsupported language".into()),
            references: vec![],
        }];
        let report = render_markdown(&ReportInput {
            assessment: &assessment,
            gate: Some(&gate),
            parsed_files: &files,
            version: "test",
        });
        assert!(report.contains("No changed files"));
        assert!(report.contains("The following files require attention"));
        assert!(report.contains("### Approval"));
        assert!(report.contains("Eligible approvers"));
        assert!(report.contains("### Validations"));
        assert!(report.contains("stdout: `out line`"));
        assert!(report.contains("stderr: `err`"));
        assert!(report.contains("- **Status:** `FAILED`"));

        let partial_file = ParsedFile {
            path: "unknown.py".into(),
            language: Some("python".into()),
            root: None,
            error: Some("syntax errors detected".into()),
            references: vec![],
        };
        let partial_gate = GateResult {
            risk: RiskLevel::Minimal,
            instant_merge: false,
            validations: vec![],
            approval: None,
            merge_blocked: false,
            status: AnalysisStatus::Success,
        };
        let partial_report = render_markdown(&ReportInput {
            assessment: &assessment,
            gate: Some(&partial_gate),
            parsed_files: &[partial_file],
            version: "test",
        });
        assert!(partial_report.contains("- **Status:** `PARTIAL`"));

        let module_assessment = RiskAssessment {
            risk: RiskLevel::High,
            evidence: vec![crate::risk::RiskEvidence {
                path: "src/payment.py".into(),
                risk: RiskLevel::High,
                description: None,
                matched_rule: None,
                modules: vec!["Payment".into()],
                ignored_isolated_new_code: false,
            }],
            impact: vec![],
        };
        let module_report = render_markdown(&ReportInput {
            assessment: &module_assessment,
            gate: None,
            parsed_files: &[],
            version: "test",
        });
        assert!(module_report.contains("modules: `Payment`"));

        let impact_assessment = RiskAssessment {
            risk: RiskLevel::Danger,
            evidence: vec![crate::risk::RiskEvidence {
                path: "src/new.py".into(),
                risk: RiskLevel::Danger,
                description: None,
                matched_rule: None,
                modules: vec![],
                ignored_isolated_new_code: false,
            }],
            impact: vec![crate::risk::ImpactEvidence {
                changed_path: "src/new.py".into(),
                related_path: "src/payment/core.py".into(),
                risk: RiskLevel::Danger,
                rule: "src/payment/".into(),
                description: Some("payments".into()),
                path: vec!["src/new.py".into(), "src/payment/core.py".into()],
            }],
        };
        let impact_report = render_markdown(&ReportInput {
            assessment: &impact_assessment,
            gate: None,
            parsed_files: &[],
            version: "test",
        });
        assert!(impact_report.contains("### Cascade impact summary"));
        assert!(impact_report.contains("payment/core.py"));
        assert!(impact_report.contains("chain:"));
        assert!(impact_report.contains("```mermaid"));
        assert!(impact_report.contains("flowchart TD"));
        assert!(impact_report.contains("nodeSpacing"));
        assert!(impact_report.contains("primaryTextColor"));
        assert!(impact_report.contains("classDef risk_danger"));
        assert!(impact_report.contains("Risk legend"));
        assert!(impact_report.contains("🟥 danger"));
        assert!(impact_report.contains("color:#000000"));
        assert!(!impact_report.contains("<span style="));
        assert!(!impact_report.contains("subgraph risk_legend"));
        assert!(impact_report.contains("impact_0[\"src<br/>new.py\"]"));
        assert!(impact_report.contains("step 1"));
        assert!(impact_report.contains("<details>"));

        let summarized_assessment = RiskAssessment {
            risk: RiskLevel::Danger,
            evidence: vec![],
            impact: vec![
                crate::risk::ImpactEvidence {
                    changed_path: "src/new.py".into(),
                    related_path: "tests/isolated.py".into(),
                    risk: RiskLevel::Minimal,
                    rule: "tests/".into(),
                    description: None,
                    path: vec!["src/new.py".into(), "tests/isolated.py".into()],
                },
                crate::risk::ImpactEvidence {
                    changed_path: "src/new.py".into(),
                    related_path: "src/payment/core.py".into(),
                    risk: RiskLevel::Danger,
                    rule: "src/payment/".into(),
                    description: None,
                    path: vec![
                        "src/new.py".into(),
                        "src/payment/core.py".into(),
                        "infra/database".into(),
                    ],
                },
            ],
        };
        let summarized_report = render_markdown(&ReportInput {
            assessment: &summarized_assessment,
            gate: None,
            parsed_files: &[],
            version: "test",
        });
        assert!(summarized_report.contains("Highest propagated risk: `danger`"));
        assert!(summarized_report.contains("infra/database"));
        assert!(summarized_report.contains("tests/isolated.py"));

        let isolated_impact_assessment = RiskAssessment {
            risk: RiskLevel::Normal,
            evidence: vec![],
            impact: vec![crate::risk::ImpactEvidence {
                changed_path: "src/only.py".into(),
                related_path: "src/only.py".into(),
                risk: RiskLevel::Normal,
                rule: "src/".into(),
                description: None,
                path: vec!["src/only.py".into()],
            }],
        };
        let isolated_mermaid = render_impact_mermaid(&isolated_impact_assessment);
        assert!(isolated_mermaid.contains("impact_0[\"src<br/>only.py\"]"));
        assert!(!isolated_mermaid.contains("-->"));

        let isolated_report = render_markdown(&ReportInput {
            assessment: &isolated_impact_assessment,
            gate: None,
            parsed_files: &[],
            version: "test",
        });
        assert!(!isolated_report.contains("chain:"));
    }

    #[test]
    fn truncates_large_reports_at_utf8_boundary() {
        let assessment = RiskAssessment {
            risk: RiskLevel::Normal,
            evidence: vec![],
            impact: vec![],
        };
        let report = render_markdown_limited(
            &ReportInput {
                assessment: &assessment,
                gate: None,
                parsed_files: &[],
                version: "release 🚀",
            },
            20,
        );
        assert!(report.is_char_boundary(report.len()));
        assert!(report.len() <= 20);
        let large = render_markdown_limited(
            &ReportInput {
                assessment: &assessment,
                gate: None,
                parsed_files: &[],
                version: "test",
            },
            100,
        );
        assert!(large.len() <= 100);
        assert!(large.contains("Report truncated"));
        let tiny = render_markdown_limited(
            &ReportInput {
                assessment: &assessment,
                gate: None,
                parsed_files: &[],
                version: "test",
            },
            3,
        );
        assert!(tiny.len() <= 3);
        assert_eq!(utf8_prefix_len("release 🚀", 9), 8);
        assert_eq!(utf8_prefix_len("release 🚀", 12), 12);
        assert_eq!(utf8_prefix_len("abc", 10), 3);

        let suffix = "\n\n_Report truncated by configured resource limit._\n";
        assert_eq!(
            truncate_utf8("abcdefghij".repeat(10), suffix.len() + 4),
            format!("abcd{suffix}")
        );
    }
}
