//! Output formatters — JSON and SARIF serialization for CI/CD integration.

use serde::Serialize;

use crate::verify::{ChangeRisk, VerifyResult};
use houndlens_ir::invariant::ViolationSeverity;

// ─── JSON Output ────────────────────────────────────────────────

#[derive(Serialize)]
pub struct JsonOutput {
    pub version: String,
    pub status: String,
    pub risk_score: f64,
    pub confidence: f64,
    pub summary: JsonSummary,
    pub semantic_changes: Vec<JsonChange>,
    pub invariant_warnings: Vec<JsonWarning>,
    pub suggested_tests: Vec<JsonTestSuggestion>,
}

#[derive(Serialize)]
pub struct JsonSummary {
    pub total_changes: usize,
    pub breaking: usize,
    pub needs_review: usize,
    pub safe: usize,
    pub errors: usize,
    pub warnings: usize,
}

#[derive(Serialize)]
pub struct JsonChange {
    pub file: String,
    pub line: u32,
    pub kind: String,
    pub risk: String,
    pub description: String,
}

#[derive(Serialize)]
pub struct JsonWarning {
    pub file: String,
    pub line: u32,
    pub severity: String,
    pub description: String,
}

#[derive(Serialize)]
pub struct JsonTestSuggestion {
    pub description: String,
    pub priority: String,
    pub skeleton: Option<String>,
}

pub fn to_json(result: &VerifyResult) -> String {
    let breaking = result
        .semantic_changes
        .iter()
        .filter(|c| matches!(c.risk, ChangeRisk::Breaking))
        .count();
    let needs_review = result
        .semantic_changes
        .iter()
        .filter(|c| matches!(c.risk, ChangeRisk::NeedsReview))
        .count();
    let safe = result
        .semantic_changes
        .iter()
        .filter(|c| matches!(c.risk, ChangeRisk::Safe))
        .count();

    let output = JsonOutput {
        version: "0.1.0".into(),
        status: if result.has_errors() { "fail" } else { "pass" }.into(),
        risk_score: result.risk_score,
        confidence: result.confidence,
        summary: JsonSummary {
            total_changes: result.semantic_changes.len(),
            breaking,
            needs_review,
            safe,
            errors: result.error_count(),
            warnings: result.warning_count(),
        },
        semantic_changes: result
            .semantic_changes
            .iter()
            .map(|c| JsonChange {
                file: c.location.file.display().to_string(),
                line: c.location.start_line,
                kind: format!("{:?}", c.kind),
                risk: format!("{:?}", c.risk),
                description: c.description.clone(),
            })
            .collect(),
        invariant_warnings: result
            .invariant_violations
            .iter()
            .map(|v| JsonWarning {
                file: v.location.file.display().to_string(),
                line: v.location.start_line,
                severity: match v.severity {
                    ViolationSeverity::Error => "error",
                    ViolationSeverity::Warning => "warning",
                    ViolationSeverity::Info => "info",
                }
                .into(),
                description: v.description.clone(),
            })
            .collect(),
        suggested_tests: result
            .suggested_tests
            .iter()
            .map(|t| JsonTestSuggestion {
                description: t.description.clone(),
                priority: format!("{:?}", t.priority),
                skeleton: t.skeleton.clone(),
            })
            .collect(),
    };

    serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".into())
}

// ─── SARIF Output ───────────────────────────────────────────────
// SARIF = Static Analysis Results Interchange Format
// Standard for GitHub Code Scanning, Azure DevOps, etc.

#[derive(Serialize)]
pub struct SarifOutput {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub version: String,
    pub runs: Vec<SarifRun>,
}

#[derive(Serialize)]
pub struct SarifRun {
    pub tool: SarifTool,
    pub results: Vec<SarifResult>,
}

#[derive(Serialize)]
pub struct SarifTool {
    pub driver: SarifDriver,
}

#[derive(Serialize)]
pub struct SarifDriver {
    pub name: String,
    pub version: String,
    #[serde(rename = "informationUri")]
    pub information_uri: String,
    pub rules: Vec<SarifRule>,
}

#[derive(Serialize)]
pub struct SarifRule {
    pub id: String,
    pub name: String,
    #[serde(rename = "shortDescription")]
    pub short_description: SarifMessage,
    #[serde(rename = "defaultConfiguration")]
    pub default_configuration: SarifRuleConfig,
}

#[derive(Serialize)]
pub struct SarifRuleConfig {
    pub level: String,
}

#[derive(Serialize)]
pub struct SarifResult {
    #[serde(rename = "ruleId")]
    pub rule_id: String,
    pub level: String,
    pub message: SarifMessage,
    pub locations: Vec<SarifLocation>,
}

#[derive(Serialize)]
pub struct SarifMessage {
    pub text: String,
}

#[derive(Serialize)]
pub struct SarifLocation {
    #[serde(rename = "physicalLocation")]
    pub physical_location: SarifPhysicalLocation,
}

#[derive(Serialize)]
pub struct SarifPhysicalLocation {
    #[serde(rename = "artifactLocation")]
    pub artifact_location: SarifArtifactLocation,
    pub region: SarifRegion,
}

#[derive(Serialize)]
pub struct SarifArtifactLocation {
    pub uri: String,
}

#[derive(Serialize)]
pub struct SarifRegion {
    #[serde(rename = "startLine")]
    pub start_line: u32,
    #[serde(rename = "startColumn")]
    pub start_column: u32,
}

pub fn to_sarif(result: &VerifyResult) -> String {
    let mut rules = Vec::new();
    let mut results = Vec::new();

    // Add rules and results for semantic changes.
    for change in result.semantic_changes.iter() {
        let rule_id = format!("houndlens/{:?}", change.kind).to_lowercase().replace(' ', "-");
        let level = match change.risk {
            ChangeRisk::Breaking => "error",
            ChangeRisk::SecuritySensitive => "error",
            ChangeRisk::NeedsReview => "warning",
            ChangeRisk::Safe => "note",
        };

        if !rules.iter().any(|r: &SarifRule| r.id == rule_id) {
            rules.push(SarifRule {
                id: rule_id.clone(),
                name: format!("{:?}", change.kind),
                short_description: SarifMessage {
                    text: format!("Semantic change: {:?}", change.kind),
                },
                default_configuration: SarifRuleConfig {
                    level: level.into(),
                },
            });
        }

        let file_path = change.location.file.to_string_lossy().replace('\\', "/");
        results.push(SarifResult {
            rule_id,
            level: level.into(),
            message: SarifMessage {
                text: change.description.clone(),
            },
            locations: vec![SarifLocation {
                physical_location: SarifPhysicalLocation {
                    artifact_location: SarifArtifactLocation {
                        uri: file_path,
                    },
                    region: SarifRegion {
                        start_line: change.location.start_line,
                        start_column: change.location.start_col + 1,
                    },
                },
            }],
        });
    }

    // Add invariant violations.
    for violation in &result.invariant_violations {
        let level = match violation.severity {
            ViolationSeverity::Error => "error",
            ViolationSeverity::Warning => "warning",
            ViolationSeverity::Info => "note",
        };

        let file_path = violation.location.file.to_string_lossy().replace('\\', "/");
        results.push(SarifResult {
            rule_id: "houndlens/invariant-violation".into(),
            level: level.into(),
            message: SarifMessage {
                text: violation.description.clone(),
            },
            locations: vec![SarifLocation {
                physical_location: SarifPhysicalLocation {
                    artifact_location: SarifArtifactLocation {
                        uri: file_path,
                    },
                    region: SarifRegion {
                        start_line: violation.location.start_line,
                        start_column: violation.location.start_col + 1,
                    },
                },
            }],
        });
    }

    if !rules.iter().any(|r| r.id == "houndlens/invariant-violation") {
        rules.push(SarifRule {
            id: "houndlens/invariant-violation".into(),
            name: "InvariantViolation".into(),
            short_description: SarifMessage {
                text: "Code change may violate a codebase invariant".into(),
            },
            default_configuration: SarifRuleConfig {
                level: "warning".into(),
            },
        });
    }

    let sarif = SarifOutput {
        schema: "https://json.schemastore.org/sarif-2.1.0.json".into(),
        version: "2.1.0".into(),
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "houndlens".into(),
                    version: "0.1.0".into(),
                    information_uri: "https://github.com/injaehwang/houndlens".into(),
                    rules,
                },
            },
            results,
        }],
    };

    serde_json::to_string_pretty(&sarif).unwrap_or_else(|_| "{}".into())
}
