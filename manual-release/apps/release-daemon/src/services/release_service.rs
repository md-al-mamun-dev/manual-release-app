use uuid::Uuid;

use crate::{
    domain::{
        project_inspection::ProjectInspection,
        release::{CreateReleaseRequest, Release, ReleaseStateTransition},
    },
    error::ApiError,
    repositories::{
        project_inspection_repository::ProjectInspectionRepository,
        project_repository::ProjectRepository,
        release_repository::{CreateReleaseInput, ReleaseRepository},
    },
    services::release_validation,
};

const LOCAL_OPERATOR: &str = "LOCAL_OPERATOR";

#[derive(Clone)]
pub struct ReleaseService {
    project_repository: ProjectRepository,

    inspection_repository: ProjectInspectionRepository,

    release_repository: ReleaseRepository,
}

impl ReleaseService {
    pub fn new(
        project_repository: ProjectRepository,

        inspection_repository: ProjectInspectionRepository,

        release_repository: ReleaseRepository,
    ) -> Self {
        Self {
            project_repository,
            inspection_repository,
            release_repository,
        }
    }

    pub async fn create(
        &self,
        project_id: Uuid,
        input: CreateReleaseRequest,
    ) -> Result<Release, ApiError> {
        self.ensure_project_exists(project_id).await?;

        let version = release_validation::validate_version(&input.version)?;

        let inspection = self
            .inspection_repository
            .find_by_id(input.inspection_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("inspection not found".to_string()))?;

        self.validate_inspection(project_id, &inspection)?;

        let git_commit = inspection.git_commit.as_deref().ok_or_else(|| {
            ApiError::Unprocessable("inspection does not contain a Git commit".to_string())
        })?;

        if !release_validation::valid_git_commit(git_commit) {
            return Err(ApiError::Unprocessable(
                "inspection contains an invalid Git commit".to_string(),
            ));
        }

        let release = self
            .release_repository
            .create_with_initial_transition(CreateReleaseInput {
                id: Uuid::new_v4(),
                project_id,
                source_inspection_id: inspection.id,
                version: &version,
                git_commit,
                git_branch: inspection.git_branch.as_deref(),
                source_dirty: false,
                requested_by: None,
                actor: LOCAL_OPERATOR,
            })
            .await?;

        Ok(release)
    }

    fn validate_inspection(
        &self,
        project_id: Uuid,
        inspection: &ProjectInspection,
    ) -> Result<(), ApiError> {
        if inspection.project_id != project_id {
            return Err(ApiError::Unprocessable(
                "inspection does not belong to this project".to_string(),
            ));
        }

        if inspection.status != "SUCCEEDED" {
            return Err(ApiError::Unprocessable(
                "inspection did not succeed".to_string(),
            ));
        }

        match inspection.git_dirty {
            Some(false) => {}
            Some(true) => {
                return Err(ApiError::Unprocessable(
                    "cannot create a release from a dirty Git worktree".to_string(),
                ));
            }
            None => {
                return Err(ApiError::Unprocessable(
                    "inspection does not contain Git worktree state".to_string(),
                ));
            }
        }

        Ok(())
    }

    async fn ensure_project_exists(&self, project_id: Uuid) -> Result<(), ApiError> {
        self.project_repository
            .find_active_by_id(project_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("project not found".to_string()))?;

        Ok(())
    }

    pub async fn list(&self, project_id: Uuid) -> Result<Vec<Release>, ApiError> {
        self.ensure_project_exists(project_id).await?;

        Ok(self
            .release_repository
            .find_all_by_project(project_id)
            .await?)
    }

    pub async fn get(&self, release_id: Uuid) -> Result<Release, ApiError> {
        self.release_repository
            .find_by_id(release_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("release not found".to_string()))
    }

    pub async fn transitions(
        &self,
        release_id: Uuid,
    ) -> Result<Vec<ReleaseStateTransition>, ApiError> {
        self.get(release_id).await?;

        Ok(self.release_repository.transitions(release_id).await?)
    }
}
