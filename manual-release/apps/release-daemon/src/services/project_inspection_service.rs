use uuid::Uuid;

use crate::{
    domain::project_inspection::ProjectInspection,
    error::ApiError,
    inspection::project_inspector::ProjectInspector,
    repositories::{
        project_inspection_repository::ProjectInspectionRepository,
        project_repository::ProjectRepository,
    },
};

#[derive(Clone)]
pub struct ProjectInspectionService {
    project_repository: ProjectRepository,

    inspection_repository: ProjectInspectionRepository,

    inspector: ProjectInspector,
}

impl ProjectInspectionService {
    pub fn new(
        project_repository: ProjectRepository,

        inspection_repository: ProjectInspectionRepository,

        inspector: ProjectInspector,
    ) -> Self {
        Self {
            project_repository,
            inspection_repository,
            inspector,
        }
    }

    pub async fn inspect(&self, project_id: Uuid) -> Result<ProjectInspection, ApiError> {
        let project = self
            .project_repository
            .find_active_by_id(project_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("project not found".to_string()))?;

        let inspection = self.inspection_repository.start(project_id).await?;

        match self.inspector.inspect(&project.repository_path).await {
            Ok(report) => {
                let report_json = serde_json::to_value(&report).map_err(|error| {
                    tracing::error!(
                        error = ?error,
                        "failed to serialize \
                         inspection report"
                    );

                    ApiError::Internal
                })?;

                let completed = self
                    .inspection_repository
                    .succeed(
                        inspection.id,
                        &report.repository_path,
                        &report.git.commit,
                        report.git.branch.as_deref(),
                        report.git.dirty,
                        report_json,
                    )
                    .await?;

                Ok(completed)
            }

            Err(error) => {
                tracing::warn!(
                    project_id = %project_id,
                    error = ?error,
                    "project inspection failed"
                );

                if let Err(database_error) = self
                    .inspection_repository
                    .fail(inspection.id, "INSPECTION_FAILED", &error.to_string())
                    .await
                {
                    tracing::error!(
                        error = ?database_error,
                        inspection_id =
                            %inspection.id,
                        "failed to persist \
                         inspection failure"
                    );
                }

                Err(ApiError::Unprocessable(error.to_string()))
            }
        }
    }

    pub async fn latest(&self, project_id: Uuid) -> Result<ProjectInspection, ApiError> {
        self.project_repository
            .find_active_by_id(project_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("project not found".to_string()))?;

        self.inspection_repository
            .latest(project_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("project has not been inspected".to_string()))
    }
}
