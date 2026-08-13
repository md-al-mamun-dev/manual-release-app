use sqlx::PgPool;

use crate::services::project_service::ProjectService;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,

    pub project_service: ProjectService,
}
