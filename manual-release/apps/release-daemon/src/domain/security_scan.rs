use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecurityScan {
    pub id: Uuid,
    pub release_id: Uuid,
    pub image_digest: String,
    pub critical_vulnerabilities: i32,
    pub high_vulnerabilities: i32,
    pub medium_vulnerabilities: i32,
    pub low_vulnerabilities: i32,
    pub passed: bool,
    pub report_json: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSecurityScanInput {
    pub id: Uuid,
    pub release_id: Uuid,
    pub image_digest: String,
    pub critical_vulnerabilities: i32,
    pub high_vulnerabilities: i32,
    pub medium_vulnerabilities: i32,
    pub low_vulnerabilities: i32,
    pub passed: bool,
    pub report_json: serde_json::Value,
    pub created_at: DateTime<Utc>,
}
