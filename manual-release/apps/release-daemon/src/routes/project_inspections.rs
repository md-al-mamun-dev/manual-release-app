use actix_web::{HttpResponse, web};

use uuid::Uuid;

use crate::{app_state::AppState, domain::project_inspection::ProjectInspection, error::ApiError};

pub fn configure(config: &mut web::ServiceConfig) {
    config
        .route(
            "/projects/{project_id}/inspect",
            web::post().to(inspect_project),
        )
        .route(
            "/projects/{project_id}/inspections/latest",
            web::get().to(latest_inspection),
        );
}

#[utoipa::path(
    post,
    path = "/api/projects/{project_id}/inspect",
    params(
        ("project_id" = Uuid, Path, description = "Project ID")
    ),
    responses(
        (status = 201, description = "Project inspected", body = ProjectInspection)
    )
)]
async fn inspect_project(
    state: web::Data<AppState>,
    project_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let inspection = state
        .project_inspection_service
        .inspect(project_id.into_inner())
        .await?;

    Ok(HttpResponse::Created().json(inspection))
}

#[utoipa::path(
    get,
    path = "/api/projects/{project_id}/inspections/latest",
    params(
        ("project_id" = Uuid, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "Get latest inspection", body = ProjectInspection)
    )
)]
async fn latest_inspection(
    state: web::Data<AppState>,
    project_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let inspection = state
        .project_inspection_service
        .latest(project_id.into_inner())
        .await?;

    Ok(HttpResponse::Ok().json(inspection))
}
