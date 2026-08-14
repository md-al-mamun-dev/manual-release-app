use actix_web::{HttpResponse, web};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    domain::environment::{CreateEnvironmentRequest, UpdateEnvironmentRequest},
    error::ApiError,
};

async fn create_environment(
    state: web::Data<AppState>,
    project_id: web::Path<Uuid>,
    body: web::Json<CreateEnvironmentRequest>,
) -> Result<HttpResponse, ApiError> {
    let environment = state
        .environment_service
        .create(project_id.into_inner(), body.into_inner())
        .await?;

    Ok(HttpResponse::Created().json(environment))
}

async fn list_environments(
    state: web::Data<AppState>,
    project_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let environments = state
        .environment_service
        .list(project_id.into_inner())
        .await?;

    Ok(HttpResponse::Ok().json(environments))
}

async fn get_environment(
    state: web::Data<AppState>,
    environment_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let environment = state
        .environment_service
        .get(environment_id.into_inner())
        .await?;

    Ok(HttpResponse::Ok().json(environment))
}

async fn update_environment(
    state: web::Data<AppState>,
    environment_id: web::Path<Uuid>,
    body: web::Json<UpdateEnvironmentRequest>,
) -> Result<HttpResponse, ApiError> {
    let environment = state
        .environment_service
        .update(environment_id.into_inner(), body.into_inner())
        .await?;

    Ok(HttpResponse::Ok().json(environment))
}

async fn archive_environment(
    state: web::Data<AppState>,
    environment_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    state
        .environment_service
        .archive(environment_id.into_inner())
        .await?;

    Ok(HttpResponse::NoContent().finish())
}

pub fn configure(config: &mut web::ServiceConfig) {
    config
        .route(
            "/projects/{project_id}/environments",
            web::post().to(create_environment),
        )
        .route(
            "/projects/{project_id}/environments",
            web::get().to(list_environments),
        )
        .route(
            "/environments/{environment_id}",
            web::get().to(get_environment),
        )
        .route(
            "/environments/{environment_id}",
            web::put().to(update_environment),
        )
        .route(
            "/environments/{environment_id}",
            web::delete().to(archive_environment),
        );
}
