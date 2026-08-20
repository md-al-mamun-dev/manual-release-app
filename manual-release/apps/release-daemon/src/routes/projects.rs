use actix_web::{HttpResponse, web};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    domain::project::{CreateProjectRequest, Project, UpdateProjectRequest},
    error::ApiError,
};

pub fn configure(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("/projects")
            .route("", web::post().to(create_project))
            .route("", web::get().to(list_projects))
            .route("/{project_id}", web::get().to(get_project))
            .route("/{project_id}", web::patch().to(update_project))
            .route("/{project_id}", web::delete().to(archive_project)),
    );
}

#[utoipa::path(
    post,
    path = "/api/projects",
    request_body = CreateProjectRequest,
    responses(
        (status = 201, description = "Project created", body = Project)
    )
)]
async fn create_project(
    state: web::Data<AppState>,
    body: web::Json<CreateProjectRequest>,
) -> Result<HttpResponse, ApiError> {
    let project = state.project_service.create(body.into_inner()).await?;

    Ok(HttpResponse::Created().json(project))
}

#[utoipa::path(
    get,
    path = "/api/projects",
    responses(
        (status = 200, description = "List all projects", body = [Project])
    )
)]
async fn list_projects(state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let projects = state.project_service.list().await?;

    Ok(HttpResponse::Ok().json(projects))
}

#[utoipa::path(
    get,
    path = "/api/projects/{project_id}",
    params(
        ("project_id" = Uuid, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "Get project by ID", body = Project)
    )
)]
async fn get_project(
    state: web::Data<AppState>,
    project_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let project = state.project_service.get(project_id.into_inner()).await?;

    Ok(HttpResponse::Ok().json(project))
}

#[utoipa::path(
    patch,
    path = "/api/projects/{project_id}",
    params(
        ("project_id" = Uuid, Path, description = "Project ID")
    ),
    request_body = UpdateProjectRequest,
    responses(
        (status = 200, description = "Update project by ID", body = Project)
    )
)]
async fn update_project(
    state: web::Data<AppState>,
    project_id: web::Path<Uuid>,
    body: web::Json<UpdateProjectRequest>,
) -> Result<HttpResponse, ApiError> {
    let project = state
        .project_service
        .update(project_id.into_inner(), body.into_inner())
        .await?;

    Ok(HttpResponse::Ok().json(project))
}

#[utoipa::path(
    delete,
    path = "/api/projects/{project_id}",
    params(
        ("project_id" = Uuid, Path, description = "Project ID")
    ),
    responses(
        (status = 204, description = "Archive project by ID")
    )
)]
async fn archive_project(
    state: web::Data<AppState>,
    project_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    state
        .project_service
        .archive(project_id.into_inner())
        .await?;

    Ok(HttpResponse::NoContent().finish())
}
