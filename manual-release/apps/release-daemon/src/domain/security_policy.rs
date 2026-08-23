use serde::{Deserialize, Serialize};

use crate::domain::trivy_parser::VulnerabilitySummary;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityDecision {
    pub passed: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecurityPolicy {
    pub fail_on_critical: bool,
    pub fail_on_high: bool,
    pub fail_on_medium: bool,
    pub fail_on_low: bool,
    #[serde(default)]
    pub fail_on_unknown: bool,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            fail_on_critical: true,
            fail_on_high: true,
            fail_on_medium: false,
            fail_on_low: false,
            fail_on_unknown: true,
        }
    }
}

impl SecurityPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn evaluate(&self, summary: &VulnerabilitySummary) -> SecurityDecision {
        let mut reasons = Vec::new();
        let mut passed = true;

        if self.fail_on_critical && summary.critical > 0 {
            passed = false;
            reasons.push(format!(
                "Found {} CRITICAL vulnerabilities (policy prohibits CRITICAL)",
                summary.critical
            ));
        }

        if self.fail_on_high && summary.high > 0 {
            passed = false;
            reasons.push(format!(
                "Found {} HIGH vulnerabilities (policy prohibits HIGH)",
                summary.high
            ));
        }

        if self.fail_on_medium && summary.medium > 0 {
            passed = false;
            reasons.push(format!(
                "Found {} MEDIUM vulnerabilities (policy prohibits MEDIUM)",
                summary.medium
            ));
        }

        if self.fail_on_low && summary.low > 0 {
            passed = false;
            reasons.push(format!(
                "Found {} LOW vulnerabilities (policy prohibits LOW)",
                summary.low
            ));
        }

        if self.fail_on_unknown && summary.unknown > 0 {
            passed = false;
            reasons.push(format!(
                "Found {} UNKNOWN severity vulnerabilities (policy prohibits UNKNOWN)",
                summary.unknown
            ));
        }

        SecurityDecision { passed, reasons }
    }
}
