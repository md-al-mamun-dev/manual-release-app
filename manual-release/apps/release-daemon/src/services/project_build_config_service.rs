use uuid::Uuid;

use crate::{
    domain::project_build_config::{
        CreateProjectBuildConfig, ProjectBuildConfig, UpdateProjectBuildConfig,
    },
    error::ApiError,
    repositories::{
        project_build_config_repository::ProjectBuildConfigRepository,
        project_repository::ProjectRepository,
    },
    services::build_config_validator::BuildConfigValidator,
};

#[derive(Clone)]
pub struct ProjectBuildConfigService {
    project_repository: ProjectRepository,
    config_repository: ProjectBuildConfigRepository,
}

impl ProjectBuildConfigService {
    pub fn new(
        project_repository: ProjectRepository,
        config_repository: ProjectBuildConfigRepository,
    ) -> Self {
        Self {
            project_repository,
            config_repository,
        }
    }

    pub async fn get_config(&self, project_id: Uuid) -> Result<ProjectBuildConfig, ApiError> {
        self.project_repository
            .find_active_by_id(project_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("project not found".to_string()))?;

        self.config_repository
            .find_by_project_id(project_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("build config not found".to_string()))
    }

    pub async fn create_or_update(
        &self,
        project_id: Uuid,
        mut input: CreateProjectBuildConfig,
    ) -> Result<ProjectBuildConfig, ApiError> {
        self.project_repository
            .find_active_by_id(project_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("project not found".to_string()))?;

        // Normalize
        input.runtime_version = input.runtime_version.trim().to_string();
        input.dockerfile_path = normalize_optional(input.dockerfile_path);
        input.build_context = normalize_optional(input.build_context);
        input.health_endpoint = normalize_optional(input.health_endpoint);

        // Validate
        BuildConfigValidator::validate_compatibility(
            input.application_type,
            input.framework,
            input.package_manager,
        )?;
        BuildConfigValidator::validate_path(input.dockerfile_path.as_deref(), "dockerfile_path")?;
        BuildConfigValidator::validate_path(input.build_context.as_deref(), "build_context")?;
        BuildConfigValidator::validate_health_endpoint(input.health_endpoint.as_deref())?;
        BuildConfigValidator::validate_port(input.application_port)?;

        let config = self
            .config_repository
            .create_or_update(project_id, input)
            .await?;

        Ok(config)
    }

    pub async fn update(
        &self,
        project_id: Uuid,
        input: UpdateProjectBuildConfig,
    ) -> Result<ProjectBuildConfig, ApiError> {
        // Find existing to merge and validate
        let existing = self.get_config(project_id).await?;

        let app_type = input.application_type.unwrap_or(existing.application_type);
        let framework = input.framework.unwrap_or(existing.framework);
        let pkg_manager = input.package_manager.unwrap_or(existing.package_manager);

        let runtime = match input.runtime_version {
            Some(v) => v.trim().to_string(),
            None => existing.runtime_version,
        };

        let dockerfile = match input.dockerfile_path {
            Some(v) => normalize_optional(v),
            None => existing.dockerfile_path,
        };

        let build_ctx = match input.build_context {
            Some(v) => normalize_optional(v),
            None => existing.build_context,
        };

        let health = match input.health_endpoint {
            Some(v) => normalize_optional(v),
            None => existing.health_endpoint,
        };

        let port = match input.application_port {
            Some(v) => v,
            None => existing.application_port,
        };

        let merged = CreateProjectBuildConfig {
            application_type: app_type,
            framework,
            runtime_version: runtime,
            package_manager: pkg_manager,
            dockerfile_path: dockerfile,
            build_context: build_ctx,
            application_port: port,
            health_endpoint: health,
        };

        self.create_or_update(project_id, merged).await
    }
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
