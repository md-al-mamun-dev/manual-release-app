use actix_web::{HttpResponse, web};
use uuid::Uuid;

use crate::{app_state::AppState, domain::release::CreateReleaseRequest, error::ApiError};

pub fn configure(config: &mut web::ServiceConfig) {
    config
        .route(
            "/projects/{project_id}/releases",
            web::post().to(create_release),
        )
        .route(
            "/projects/{project_id}/releases",
            web::get().to(list_releases),
        )
        .route("/releases/{release_id}", web::get().to(get_release))
        .route(
            "/releases/{release_id}/transitions",
            web::get().to(get_transitions),
        );
}

async fn create_release(
    state: web::Data<AppState>,
    project_id: web::Path<Uuid>,
    body: web::Json<CreateReleaseRequest>,
) -> Result<HttpResponse, ApiError> {
    let release = state
        .release_service
        .create(project_id.into_inner(), body.into_inner())
        .await?;

    Ok(HttpResponse::Created().json(release))
}

async fn list_releases(
    state: web::Data<AppState>,
    project_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let releases = state.release_service.list(project_id.into_inner()).await?;

    Ok(HttpResponse::Ok().json(releases))
}

async fn get_release(
    state: web::Data<AppState>,
    release_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let release = state.release_service.get(release_id.into_inner()).await?;

    Ok(HttpResponse::Ok().json(release))
}

async fn get_transitions(
    state: web::Data<AppState>,
    release_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let transitions = state
        .release_service
        .transitions(release_id.into_inner())
        .await?;

    Ok(HttpResponse::Ok().json(transitions))
}
