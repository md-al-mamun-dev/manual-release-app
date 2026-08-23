use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VulnerabilitySeverity {
    Critical,
    High,
    Medium,
    Low,
    Unknown,
}

impl VulnerabilitySeverity {
    pub fn from_str_relaxed(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "CRITICAL" => Self::Critical,
            "HIGH" => Self::High,
            "MEDIUM" => Self::Medium,
            "LOW" => Self::Low,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulnerabilityItem {
    #[serde(rename = "VulnerabilityID")]
    pub vulnerability_id: Option<String>,
    #[serde(rename = "PkgName")]
    pub pkg_name: Option<String>,
    #[serde(rename = "InstalledVersion")]
    pub installed_version: Option<String>,
    #[serde(rename = "FixedVersion")]
    pub fixed_version: Option<String>,
    #[serde(rename = "Severity")]
    pub severity: Option<String>,
    #[serde(rename = "Title")]
    pub title: Option<String>,
    #[serde(rename = "Description")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrivyScanResult {
    #[serde(rename = "Target")]
    pub target: Option<String>,
    #[serde(rename = "Class")]
    pub class: Option<String>,
    #[serde(rename = "Type")]
    pub result_type: Option<String>,
    #[serde(rename = "Vulnerabilities")]
    pub vulnerabilities: Option<Vec<VulnerabilityItem>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrivyReport {
    #[serde(rename = "SchemaVersion")]
    pub schema_version: Option<i32>,
    #[serde(rename = "ArtifactName")]
    pub artifact_name: Option<String>,
    #[serde(rename = "ArtifactType")]
    pub artifact_type: Option<String>,
    #[serde(rename = "Results")]
    pub results: Option<Vec<TrivyScanResult>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VulnerabilitySummary {
    pub critical: i32,
    pub high: i32,
    pub medium: i32,
    pub low: i32,
    pub unknown: i32,
}

#[derive(Debug, thiserror::Error)]
pub enum TrivyParserError {
    #[error("Failed to parse Trivy report JSON: {0}")]
    JsonParseError(#[from] serde_json::Error),
    #[error("Invalid or empty report content")]
    EmptyContent,
}

pub struct TrivyParser;

impl TrivyParser {
    pub fn parse_json_str(json_str: &str) -> Result<TrivyReport, TrivyParserError> {
        let trimmed = json_str.trim();
        if trimmed.is_empty() {
            return Err(TrivyParserError::EmptyContent);
        }
        let report: TrivyReport = serde_json::from_str(trimmed)?;
        Ok(report)
    }

    pub fn parse_json_value(val: serde_json::Value) -> Result<TrivyReport, TrivyParserError> {
        let report: TrivyReport = serde_json::from_value(val)?;
        Ok(report)
    }

    pub fn summarize(report: &TrivyReport) -> VulnerabilitySummary {
        let mut summary = VulnerabilitySummary::default();

        if let Some(results) = &report.results {
            for result in results {
                if let Some(vulns) = &result.vulnerabilities {
                    for vuln in vulns {
                        let severity = vuln
                            .severity
                            .as_deref()
                            .map(VulnerabilitySeverity::from_str_relaxed)
                            .unwrap_or(VulnerabilitySeverity::Unknown);

                        match severity {
                            VulnerabilitySeverity::Critical => summary.critical += 1,
                            VulnerabilitySeverity::High => summary.high += 1,
                            VulnerabilitySeverity::Medium => summary.medium += 1,
                            VulnerabilitySeverity::Low => summary.low += 1,
                            VulnerabilitySeverity::Unknown => summary.unknown += 1,
                        }
                    }
                }
            }
        }

        summary
    }
}
