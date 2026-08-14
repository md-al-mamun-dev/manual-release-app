use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EnvironmentType {
    Staging,
    Production,
}

impl EnvironmentType {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Staging => "STAGING",
            Self::Production => "PRODUCTION",
        }
    }
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    pub id: Uuid,

    pub project_id: Uuid,

    pub name: String,

    pub environment_type: String,

    pub ssh_host: String,

    pub ssh_port: i32,

    pub ssh_user: String,

    pub remote_app_directory: String,

    pub server_architecture: Option<String>,

    pub ssh_identity_secret_ref: Option<String>,

    pub registry_credential_secret_ref: Option<String>,

    pub remote_env_file_path: Option<String>,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,

    pub archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateEnvironmentRequest {
    pub name: String,

    pub environment_type: EnvironmentType,

    pub ssh_host: String,

    pub ssh_port: u16,

    pub ssh_user: String,

    pub remote_app_directory: String,

    pub server_architecture: Option<String>,

    pub ssh_identity_secret_ref: Option<String>,

    pub registry_credential_secret_ref: Option<String>,

    pub remote_env_file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateEnvironmentRequest {
    pub name: String,

    pub environment_type: EnvironmentType,

    pub ssh_host: String,

    pub ssh_port: u16,

    pub ssh_user: String,

    pub remote_app_directory: String,

    pub server_architecture: Option<String>,

    pub ssh_identity_secret_ref: Option<String>,

    pub registry_credential_secret_ref: Option<String>,

    pub remote_env_file_path: Option<String>,
}
