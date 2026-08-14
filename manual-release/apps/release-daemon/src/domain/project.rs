use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: Uuid,

    pub name: String,

    pub repository_path: String,

    pub repository_url: Option<String>,

    pub default_branch: Option<String>,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,

    pub archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateProjectRequest {
    pub name: String,

    pub repository_path: String,

    pub repository_url: Option<String>,

    pub default_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,

    pub repository_path: Option<String>,

    #[serde(default, deserialize_with = "deserialize_some")]
    pub repository_url: Option<Option<String>>,

    #[serde(default, deserialize_with = "deserialize_some")]
    pub default_branch: Option<Option<String>>,
}

fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}
