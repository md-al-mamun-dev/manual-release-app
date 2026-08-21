use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    domain::project_build_config::{CreateProjectBuildConfig, ProjectBuildConfig},
    error::ApiError,
};

#[derive(Clone)]
pub struct ProjectBuildConfigRepository {
    pool: PgPool,
}

impl ProjectBuildConfigRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_project_id(
        &self,
        project_id: Uuid,
    ) -> Result<Option<ProjectBuildConfig>, ApiError> {
        let config = sqlx::query_as!(
            ProjectBuildConfig,
            r#"
            SELECT 
                id, project_id, application_type as "application_type: _", framework as "framework: _",
                runtime_version, package_manager as "package_manager: _", dockerfile_path, build_context,
                application_port, health_endpoint, created_at, updated_at
            FROM project_build_configs
            WHERE project_id = $1
            "#,
            project_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!("Database error fetching build config: {}", e);
            ApiError::Internal
        })?;

        Ok(config)
    }

    pub async fn create_or_update(
        &self,
        project_id: Uuid,
        input: CreateProjectBuildConfig,
    ) -> Result<ProjectBuildConfig, ApiError> {
        let config = sqlx::query_as!(
            ProjectBuildConfig,
            r#"
            INSERT INTO project_build_configs (
                id, project_id, application_type, framework, runtime_version, package_manager,
                dockerfile_path, build_context, application_port, health_endpoint
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (project_id) DO UPDATE SET
                application_type = EXCLUDED.application_type,
                framework = EXCLUDED.framework,
                runtime_version = EXCLUDED.runtime_version,
                package_manager = EXCLUDED.package_manager,
                dockerfile_path = EXCLUDED.dockerfile_path,
                build_context = EXCLUDED.build_context,
                application_port = EXCLUDED.application_port,
                health_endpoint = EXCLUDED.health_endpoint,
                updated_at = NOW()
            RETURNING
                id, project_id, application_type as "application_type: _", framework as "framework: _",
                runtime_version, package_manager as "package_manager: _", dockerfile_path, build_context,
                application_port, health_endpoint, created_at, updated_at
            "#,
            Uuid::new_v4(),
            project_id,
            input.application_type.to_string(),
            input.framework.to_string(),
            input.runtime_version,
            input.package_manager.to_string(),
            input.dockerfile_path,
            input.build_context,
            input.application_port,
            input.health_endpoint
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!("Database error upserting build config: {}", e);
            ApiError::Internal
        })?;

        Ok(config)
    }
}
