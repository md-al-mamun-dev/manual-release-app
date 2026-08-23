use release_daemon::domain::{
    security_policy::SecurityPolicy,
    trivy_parser::{TrivyParser, TrivyParserError, VulnerabilitySeverity},
};

#[test]
fn test_trivy_parser_empty_report() {
    let json = r#"{
        "SchemaVersion": 2,
        "Results": []
    }"#;

    let report = TrivyParser::parse_json_str(json).unwrap();
    let summary = TrivyParser::summarize(&report);
    assert_eq!(summary.critical, 0);
    assert_eq!(summary.high, 0);
    assert_eq!(summary.medium, 0);
    assert_eq!(summary.low, 0);
    assert_eq!(summary.unknown, 0);
}

#[test]
fn test_trivy_parser_critical_vulnerability() {
    let json = r#"{
        "SchemaVersion": 2,
        "Results": [
            {
                "Target": "node-app",
                "Vulnerabilities": [
                    {
                        "VulnerabilityID": "CVE-2026-1001",
                        "PkgName": "express",
                        "Severity": "CRITICAL"
                    }
                ]
            }
        ]
    }"#;

    let report = TrivyParser::parse_json_str(json).unwrap();
    let summary = TrivyParser::summarize(&report);
    assert_eq!(summary.critical, 1);
    assert_eq!(summary.high, 0);
    assert_eq!(summary.medium, 0);
    assert_eq!(summary.low, 0);
    assert_eq!(summary.unknown, 0);
}

#[test]
fn test_trivy_parser_high_vulnerability() {
    let json = r#"{
        "SchemaVersion": 2,
        "Results": [
            {
                "Target": "node-app",
                "Vulnerabilities": [
                    {
                        "VulnerabilityID": "CVE-2026-1002",
                        "PkgName": "lodash",
                        "Severity": "HIGH"
                    }
                ]
            }
        ]
    }"#;

    let report = TrivyParser::parse_json_str(json).unwrap();
    let summary = TrivyParser::summarize(&report);
    assert_eq!(summary.critical, 0);
    assert_eq!(summary.high, 1);
    assert_eq!(summary.medium, 0);
    assert_eq!(summary.low, 0);
    assert_eq!(summary.unknown, 0);
}

#[test]
fn test_trivy_parser_multiple_severity_levels() {
    let json = r#"{
        "SchemaVersion": 2,
        "Results": [
            {
                "Target": "app-dependencies",
                "Vulnerabilities": [
                    { "VulnerabilityID": "CVE-1", "Severity": "CRITICAL" },
                    { "VulnerabilityID": "CVE-2", "Severity": "HIGH" },
                    { "VulnerabilityID": "CVE-3", "Severity": "MEDIUM" },
                    { "VulnerabilityID": "CVE-4", "Severity": "LOW" },
                    { "VulnerabilityID": "CVE-5", "Severity": "UNKNOWN" },
                    { "VulnerabilityID": "CVE-6", "Severity": "critical" },
                    { "VulnerabilityID": "CVE-7", "Severity": "high" }
                ]
            }
        ]
    }"#;

    let report = TrivyParser::parse_json_str(json).unwrap();
    let summary = TrivyParser::summarize(&report);
    assert_eq!(summary.critical, 2);
    assert_eq!(summary.high, 2);
    assert_eq!(summary.medium, 1);
    assert_eq!(summary.low, 1);
    assert_eq!(summary.unknown, 1);
}

#[test]
fn test_trivy_parser_multiple_targets() {
    let json = r#"{
        "SchemaVersion": 2,
        "Results": [
            {
                "Target": "os-pkgs (ubuntu 22.04)",
                "Vulnerabilities": [
                    { "VulnerabilityID": "CVE-OS-1", "Severity": "HIGH" }
                ]
            },
            {
                "Target": "package-lock.json",
                "Vulnerabilities": [
                    { "VulnerabilityID": "CVE-NPM-1", "Severity": "CRITICAL" }
                ]
            }
        ]
    }"#;

    let report = TrivyParser::parse_json_str(json).unwrap();
    let summary = TrivyParser::summarize(&report);
    assert_eq!(summary.critical, 1);
    assert_eq!(summary.high, 1);
}

#[test]
fn test_trivy_parser_missing_results() {
    let json = r#"{
        "SchemaVersion": 2
    }"#;

    let report = TrivyParser::parse_json_str(json).unwrap();
    let summary = TrivyParser::summarize(&report);
    assert_eq!(summary.critical, 0);
    assert_eq!(summary.high, 0);
    assert_eq!(summary.medium, 0);
    assert_eq!(summary.low, 0);
    assert_eq!(summary.unknown, 0);
}

#[test]
fn test_trivy_parser_missing_vulnerabilities() {
    let json = r#"{
        "SchemaVersion": 2,
        "Results": [
            {
                "Target": "clean-target"
            }
        ]
    }"#;

    let report = TrivyParser::parse_json_str(json).unwrap();
    let summary = TrivyParser::summarize(&report);
    assert_eq!(summary.critical, 0);
    assert_eq!(summary.high, 0);
    assert_eq!(summary.medium, 0);
    assert_eq!(summary.low, 0);
    assert_eq!(summary.unknown, 0);
}

#[test]
fn test_trivy_parser_malformed_json() {
    let invalid_json = "{ invalid: json, ";
    let res = TrivyParser::parse_json_str(invalid_json);
    assert!(matches!(res, Err(TrivyParserError::JsonParseError(_))));

    let empty = "   ";
    let res_empty = TrivyParser::parse_json_str(empty);
    assert!(matches!(res_empty, Err(TrivyParserError::EmptyContent)));
}

#[test]
fn test_trivy_parser_unknown_severity() {
    assert_eq!(
        VulnerabilitySeverity::from_str_relaxed("CUSTOM_UNKNOWN"),
        VulnerabilitySeverity::Unknown
    );
    assert_eq!(
        VulnerabilitySeverity::from_str_relaxed(""),
        VulnerabilitySeverity::Unknown
    );
}

#[test]
fn test_policy_critical_causes_failure() {
    let policy = SecurityPolicy::default(); // fail_on_critical: true
    let report = TrivyParser::parse_json_str(
        r#"{ "Results": [{ "Vulnerabilities": [{ "Severity": "CRITICAL" }] }] }"#,
    )
    .unwrap();
    let summary = TrivyParser::summarize(&report);
    let decision = policy.evaluate(&summary);
    assert!(!decision.passed);
    assert!(decision.reasons.iter().any(|r| r.contains("CRITICAL")));
}

#[test]
fn test_policy_high_causes_failure() {
    let policy = SecurityPolicy::default(); // fail_on_high: true
    let report = TrivyParser::parse_json_str(
        r#"{ "Results": [{ "Vulnerabilities": [{ "Severity": "HIGH" }] }] }"#,
    )
    .unwrap();
    let summary = TrivyParser::summarize(&report);
    let decision = policy.evaluate(&summary);
    assert!(!decision.passed);
    assert!(decision.reasons.iter().any(|r| r.contains("HIGH")));
}

#[test]
fn test_policy_medium_allowed_by_default() {
    let policy = SecurityPolicy::default(); // fail_on_medium: false
    let report = TrivyParser::parse_json_str(
        r#"{ "Results": [{ "Vulnerabilities": [{ "Severity": "MEDIUM" }] }] }"#,
    )
    .unwrap();
    let summary = TrivyParser::summarize(&report);
    let decision = policy.evaluate(&summary);
    assert!(decision.passed);
    assert!(decision.reasons.is_empty());
}

#[test]
fn test_policy_low_allowed_by_default() {
    let policy = SecurityPolicy::default(); // fail_on_low: false
    let report = TrivyParser::parse_json_str(
        r#"{ "Results": [{ "Vulnerabilities": [{ "Severity": "LOW" }] }] }"#,
    )
    .unwrap();
    let summary = TrivyParser::summarize(&report);
    let decision = policy.evaluate(&summary);
    assert!(decision.passed);
    assert!(decision.reasons.is_empty());
}

#[test]
fn test_policy_configurable_thresholds() {
    let mut custom_policy = SecurityPolicy {
        fail_on_critical: false,
        fail_on_high: false,
        fail_on_medium: true,
        fail_on_low: true,
        fail_on_unknown: true,
    };

    let report = TrivyParser::parse_json_str(
        r#"{ "Results": [{ "Vulnerabilities": [{ "Severity": "MEDIUM" }] }] }"#,
    )
    .unwrap();
    let summary = TrivyParser::summarize(&report);
    let decision = custom_policy.evaluate(&summary);
    assert!(!decision.passed);
    assert!(decision.reasons.iter().any(|r| r.contains("MEDIUM")));

    custom_policy.fail_on_medium = false;
    let decision2 = custom_policy.evaluate(&summary);
    assert!(decision2.passed);
}

#[test]
fn test_policy_unknown_causes_failure() {
    let policy = SecurityPolicy::default(); // fail_on_unknown: true
    let report = TrivyParser::parse_json_str(
        r#"{ "Results": [{ "Vulnerabilities": [{ "Severity": "UNKNOWN" }] }] }"#,
    )
    .unwrap();
    let summary = TrivyParser::summarize(&report);
    let decision = policy.evaluate(&summary);
    assert!(!decision.passed);
    assert!(decision.reasons.iter().any(|r| r.contains("UNKNOWN")));
}
