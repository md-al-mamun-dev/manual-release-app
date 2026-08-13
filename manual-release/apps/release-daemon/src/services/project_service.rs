use std::path::Path;

use uuid::Uuid;

use crate::{
    domain::project::{CreateProjectRequest, Project, UpdateProjectRequest},
    error::ApiError,
    repositories::project_repository::ProjectRepository,
};

#[derive(Clone)]
pub struct ProjectService {
    repository: ProjectRepository,
}

impl ProjectService {
    pub fn new(repository: ProjectRepository) -> Self {
        Self { repository }
    }

    pub async fn create(&self, mut input: CreateProjectRequest) -> Result<Project, ApiError> {
        normalize_create_input(&mut input);
        validate_create_input(&input)?;

        let project = self.repository.create(Uuid::new_v4(), &input).await?;

        Ok(project)
    }

    pub async fn list(&self) -> Result<Vec<Project>, ApiError> {
        Ok(self.repository.find_all_active().await?)
    }

    pub async fn get(&self, project_id: Uuid) -> Result<Project, ApiError> {
        self.repository
            .find_active_by_id(project_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("project not found".to_string()))
    }

    pub async fn update(
        &self,
        project_id: Uuid,
        mut input: UpdateProjectRequest,
    ) -> Result<Project, ApiError> {
        normalize_update_input(&mut input);
        validate_update_input(&input)?;

        self.repository
            .update(project_id, &input)
            .await?
            .ok_or_else(|| ApiError::NotFound("project not found".to_string()))
    }

    pub async fn archive(&self, project_id: Uuid) -> Result<(), ApiError> {
        self.repository
            .archive(project_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("project not found".to_string()))?;

        Ok(())
    }
}

fn normalize_create_input(input: &mut CreateProjectRequest) {
    input.name = input.name.trim().to_string();

    input.repository_path = input.repository_path.trim().to_string();

    input.repository_url = normalize_optional(input.repository_url.take());

    input.default_branch = normalize_optional(input.default_branch.take());
}

fn normalize_update_input(input: &mut UpdateProjectRequest) {
    input.name = normalize_optional(input.name.take());

    input.repository_path = normalize_optional(input.repository_path.take());

    input.repository_url = normalize_optional(input.repository_url.take());

    input.default_branch = normalize_optional(input.default_branch.take());
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn validate_create_input(input: &CreateProjectRequest) -> Result<(), ApiError> {
    validate_name(&input.name)?;

    validate_repository_path(&input.repository_path)?;

    validate_optional_branch(input.default_branch.as_deref())?;

    Ok(())
}

fn validate_update_input(input: &UpdateProjectRequest) -> Result<(), ApiError> {
    if input.name.is_none()
        && input.repository_path.is_none()
        && input.repository_url.is_none()
        && input.default_branch.is_none()
    {
        return Err(ApiError::Validation(
            "at least one project field must be supplied".to_string(),
        ));
    }

    if let Some(name) = &input.name {
        validate_name(name)?;
    }

    if let Some(repository_path) = &input.repository_path {
        validate_repository_path(repository_path)?;
    }

    validate_optional_branch(input.default_branch.as_deref())?;

    Ok(())
}

fn validate_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty() {
        return Err(ApiError::Validation("name is required".to_string()));
    }

    if name.chars().count() > 100 {
        return Err(ApiError::Validation(
            "name must not exceed 100 characters".to_string(),
        ));
    }

    Ok(())
}

fn validate_repository_path(repository_path: &str) -> Result<(), ApiError> {
    if repository_path.is_empty() {
        return Err(ApiError::Validation(
            "repositoryPath is required".to_string(),
        ));
    }

    if !Path::new(repository_path).is_absolute() {
        return Err(ApiError::Validation(
            "repositoryPath must be an absolute path".to_string(),
        ));
    }

    Ok(())
}

fn validate_optional_branch(branch: Option<&str>) -> Result<(), ApiError> {
    if matches!(branch, Some(branch) if branch.is_empty()) {
        return Err(ApiError::Validation(
            "defaultBranch cannot be empty".to_string(),
        ));
    }

    Ok(())
}
