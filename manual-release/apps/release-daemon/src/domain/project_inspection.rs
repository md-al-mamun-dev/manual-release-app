use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GitInspection {
    pub commit: String,
    pub branch: Option<String>,
    pub dirty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodeInspection {
    pub package_manager_field: Option<String>,

    pub engines_node: Option<String>,

    pub scripts: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInspectionReport {
    pub repository_path: String,

    pub git: GitInspection,

    pub runtimes: Vec<String>,

    pub package_manager: Option<String>,

    pub lockfiles: Vec<String>,

    pub node: Option<NodeInspection>,

    pub dockerfiles: Vec<String>,

    pub compose_files: Vec<String>,

    pub github_workflows: Vec<String>,

    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInspection {
    pub id: Uuid,

    pub project_id: Uuid,

    pub status: String,

    pub canonical_repository_path: Option<String>,

    pub git_commit: Option<String>,

    pub git_branch: Option<String>,

    pub git_dirty: Option<bool>,

    pub report: Option<Value>,

    pub error_code: Option<String>,

    pub error_message: Option<String>,

    pub started_at: DateTime<Utc>,

    pub finished_at: Option<DateTime<Utc>>,

    pub created_at: DateTime<Utc>,
}
