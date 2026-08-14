use sqlx::PgPool;

use crate::services::{
    environment_service::EnvironmentService, project_inspection_service::ProjectInspectionService,
    project_service::ProjectService, release_service::ReleaseService,
};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,

    pub project_service: ProjectService,

    pub project_inspection_service: ProjectInspectionService,

    pub environment_service: EnvironmentService,

    pub release_service: ReleaseService,
}
