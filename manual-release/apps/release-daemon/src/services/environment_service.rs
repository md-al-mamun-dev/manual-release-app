use uuid::Uuid;

use crate::{
    domain::environment::{CreateEnvironmentRequest, Environment, UpdateEnvironmentRequest},
    error::ApiError,
    repositories::{
        environment_repository::EnvironmentRepository, project_repository::ProjectRepository,
    },
    services::environment_validation,
};

#[derive(Clone)]
pub struct EnvironmentService {
    environment_repository: EnvironmentRepository,

    project_repository: ProjectRepository,
}

impl EnvironmentService {
    pub fn new(
        environment_repository: EnvironmentRepository,
        project_repository: ProjectRepository,
    ) -> Self {
        Self {
            environment_repository,
            project_repository,
        }
    }

    pub async fn list(&self, project_id: Uuid) -> Result<Vec<Environment>, ApiError> {
        self.ensure_project_exists(project_id).await?;

        Ok(self
            .environment_repository
            .find_all_active_by_project(project_id)
            .await?)
    }

    pub async fn create(
        &self,
        project_id: Uuid,
        mut input: CreateEnvironmentRequest,
    ) -> Result<Environment, ApiError> {
        self.ensure_project_exists(project_id).await?;

        environment_validation::validate_create(&mut input)?;

        let environment = self
            .environment_repository
            .create(Uuid::new_v4(), project_id, &input)
            .await?;

        Ok(environment)
    }

    pub async fn get(&self, environment_id: Uuid) -> Result<Environment, ApiError> {
        self.environment_repository
            .find_active_by_id(environment_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("environment not found".to_string()))
    }

    pub async fn find_all_active_by_project(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<Environment>, ApiError> {
        self.ensure_project_exists(project_id).await?;

        self.environment_repository
            .find_all_active_by_project(project_id)
            .await
            .map_err(ApiError::from)
    }

    pub async fn find_active_by_id(
        &self,
        environment_id: Uuid,
    ) -> Result<Option<Environment>, ApiError> {
        self.environment_repository
            .find_active_by_id(environment_id)
            .await
            .map_err(ApiError::from)
    }

    pub async fn update(
        &self,
        environment_id: Uuid,
        mut input: UpdateEnvironmentRequest,
    ) -> Result<Environment, ApiError> {
        self.get(environment_id).await?;

        environment_validation::validate_update(&mut input)?;

        self.environment_repository
            .update(environment_id, &input)
            .await?
            .ok_or_else(|| ApiError::NotFound("environment not found".to_string()))
    }

    pub async fn archive(&self, environment_id: Uuid) -> Result<(), ApiError> {
        self.environment_repository
            .archive(environment_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("environment not found".to_string()))?;

        Ok(())
    }

    async fn ensure_project_exists(&self, project_id: Uuid) -> Result<(), ApiError> {
        if self
            .project_repository
            .exists(project_id)
            .await
            .map_err(ApiError::from)?
        {
            Ok(())
        } else {
            Err(ApiError::NotFound(format!(
                "project not found: {}",
                project_id
            )))
        }
    }
}
