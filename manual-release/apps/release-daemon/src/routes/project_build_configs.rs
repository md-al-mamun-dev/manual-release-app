use actix_web::{HttpResponse, web};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    domain::project_build_config::{
        CreateProjectBuildConfig, ProjectBuildConfig, UpdateProjectBuildConfig,
    },
    error::ApiError,
};

pub fn configure(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("/projects/{project_id}/build-config")
            .route("", web::get().to(get_build_config))
            .route("", web::put().to(upsert_build_config))
            .route("", web::patch().to(update_build_config)),
    );
}

#[utoipa::path(
    get,
    path = "/api/projects/{project_id}/build-config",
    params(
        ("project_id" = Uuid, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "Get build config by project ID", body = ProjectBuildConfig),
        (status = 404, description = "Project or config not found")
    )
)]
async fn get_build_config(
    state: web::Data<AppState>,
    project_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let config = state
        .build_config_service
        .get_config(project_id.into_inner())
        .await?;

    Ok(HttpResponse::Ok().json(config))
}

#[utoipa::path(
    put,
    path = "/api/projects/{project_id}/build-config",
    params(
        ("project_id" = Uuid, Path, description = "Project ID")
    ),
    request_body = CreateProjectBuildConfig,
    responses(
        (status = 200, description = "Create or overwrite build config", body = ProjectBuildConfig)
    )
)]
async fn upsert_build_config(
    state: web::Data<AppState>,
    project_id: web::Path<Uuid>,
    body: web::Json<CreateProjectBuildConfig>,
) -> Result<HttpResponse, ApiError> {
    let config = state
        .build_config_service
        .create_or_update(project_id.into_inner(), body.into_inner())
        .await?;

    Ok(HttpResponse::Ok().json(config))
}

#[utoipa::path(
    patch,
    path = "/api/projects/{project_id}/build-config",
    params(
        ("project_id" = Uuid, Path, description = "Project ID")
    ),
    request_body = UpdateProjectBuildConfig,
    responses(
        (status = 200, description = "Update build config", body = ProjectBuildConfig)
    )
)]
async fn update_build_config(
    state: web::Data<AppState>,
    project_id: web::Path<Uuid>,
    body: web::Json<UpdateProjectBuildConfig>,
) -> Result<HttpResponse, ApiError> {
    let config = state
        .build_config_service
        .update(project_id.into_inner(), body.into_inner())
        .await?;

    Ok(HttpResponse::Ok().json(config))
}
