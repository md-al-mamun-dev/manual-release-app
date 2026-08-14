use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReleaseStatus {
    Created,
    SourceValidated,
    CiRunning,
    CiPassed,
    ImageBuilt,
    ImageTested,
    ScanPassed,
    Published,
    StagingDeploying,
    StagingVerified,
    ProductionApproved,
    ProductionDeploying,
    ProductionVerified,
    Failed,
    RollingBack,
    RolledBack,
    RollbackFailed,
}

impl ReleaseStatus {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Created => "CREATED",
            Self::SourceValidated => "SOURCE_VALIDATED",
            Self::CiRunning => "CI_RUNNING",
            Self::CiPassed => "CI_PASSED",
            Self::ImageBuilt => "IMAGE_BUILT",
            Self::ImageTested => "IMAGE_TESTED",
            Self::ScanPassed => "SCAN_PASSED",
            Self::Published => "PUBLISHED",
            Self::StagingDeploying => "STAGING_DEPLOYING",
            Self::StagingVerified => "STAGING_VERIFIED",
            Self::ProductionApproved => "PRODUCTION_APPROVED",
            Self::ProductionDeploying => "PRODUCTION_DEPLOYING",
            Self::ProductionVerified => "PRODUCTION_VERIFIED",
            Self::Failed => "FAILED",
            Self::RollingBack => "ROLLING_BACK",
            Self::RolledBack => "ROLLED_BACK",
            Self::RollbackFailed => "ROLLBACK_FAILED",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub id: Uuid,

    pub project_id: Uuid,

    pub source_inspection_id: Option<Uuid>,

    pub version: String,

    pub git_commit: String,

    pub git_branch: Option<String>,

    pub source_dirty: bool,

    pub status: String,

    pub requested_by: Option<String>,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateReleaseRequest {
    pub version: String,
    pub inspection_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseStateTransition {
    pub id: i64,

    pub release_id: Uuid,

    pub from_status: Option<String>,

    pub to_status: String,

    pub actor: String,

    pub reason: Option<String>,

    pub metadata: serde_json::Value,

    pub created_at: DateTime<Utc>,
}
