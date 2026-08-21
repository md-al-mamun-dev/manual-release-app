use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, utoipa::ToSchema,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "text", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApplicationType {
    Node,
    Python,
}

impl fmt::Display for ApplicationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApplicationType::Node => write!(f, "NODE"),
            ApplicationType::Python => write!(f, "PYTHON"),
        }
    }
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, utoipa::ToSchema,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "text", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Framework {
    Nextjs,
    Nestjs,
    Express,
    Fastapi,
    Unknown,
}

impl fmt::Display for Framework {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Framework::Nextjs => write!(f, "NEXTJS"),
            Framework::Nestjs => write!(f, "NESTJS"),
            Framework::Express => write!(f, "EXPRESS"),
            Framework::Fastapi => write!(f, "FASTAPI"),
            Framework::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, utoipa::ToSchema,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "text", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Uv,
    Pip,
}

impl fmt::Display for PackageManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageManager::Npm => write!(f, "NPM"),
            PackageManager::Pnpm => write!(f, "PNPM"),
            PackageManager::Yarn => write!(f, "YARN"),
            PackageManager::Uv => write!(f, "UV"),
            PackageManager::Pip => write!(f, "PIP"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBuildConfig {
    pub id: Uuid,
    pub project_id: Uuid,
    pub application_type: ApplicationType,
    pub framework: Framework,
    pub runtime_version: String,
    pub package_manager: PackageManager,
    pub dockerfile_path: Option<String>,
    pub build_context: Option<String>,
    pub application_port: Option<i32>,
    pub health_endpoint: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateProjectBuildConfig {
    pub application_type: ApplicationType,
    pub framework: Framework,
    pub runtime_version: String,
    pub package_manager: PackageManager,
    pub dockerfile_path: Option<String>,
    pub build_context: Option<String>,
    pub application_port: Option<i32>,
    pub health_endpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateProjectBuildConfig {
    pub application_type: Option<ApplicationType>,
    pub framework: Option<Framework>,
    pub runtime_version: Option<String>,
    pub package_manager: Option<PackageManager>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub dockerfile_path: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub build_context: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub application_port: Option<Option<i32>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub health_endpoint: Option<Option<String>>,
}

fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}
