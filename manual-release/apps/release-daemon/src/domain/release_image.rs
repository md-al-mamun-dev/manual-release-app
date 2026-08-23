use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseImage {
    pub id: Uuid,
    pub release_id: Uuid,
    pub job_id: Uuid,
    pub git_sha: String,
    pub image_tag: String,
    pub image_digest: String,
    pub created_at: DateTime<Utc>,
}
