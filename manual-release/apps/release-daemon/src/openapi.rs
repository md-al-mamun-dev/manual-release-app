use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::health::health,
        crate::routes::projects::list_projects,
        crate::routes::projects::get_project,
        crate::routes::projects::create_project,
        crate::routes::projects::update_project,
        crate::routes::projects::archive_project,
        crate::routes::environments::list_environments,
        crate::routes::environments::get_environment,
        crate::routes::environments::create_environment,
        crate::routes::environments::update_environment,
        crate::routes::environments::archive_environment,
        crate::routes::project_inspections::inspect_project,
        crate::routes::project_inspections::latest_inspection,
        crate::routes::releases::list_releases,
        crate::routes::releases::get_release,
        crate::routes::releases::create_release,
        crate::routes::releases::get_transitions,
    ),
    components(
        schemas(
            crate::routes::health::HealthResponse,
            crate::domain::project::Project,
            crate::domain::project::CreateProjectRequest,
            crate::domain::project::UpdateProjectRequest,
            crate::domain::environment::Environment,
            crate::domain::environment::EnvironmentType,
            crate::domain::environment::CreateEnvironmentRequest,
            crate::domain::environment::UpdateEnvironmentRequest,
            crate::domain::project_inspection::ProjectInspection,
            crate::domain::project_inspection::GitInspection,
            crate::domain::project_inspection::NodeInspection,
            crate::domain::project_inspection::ProjectInspectionReport,
            crate::domain::release::Release,
            crate::domain::release::ReleaseStatus,
            crate::domain::release::CreateReleaseRequest,
            crate::domain::release::ReleaseStateTransition,
        )
    ),
    tags(
        (name = "release-daemon", description = "Release management API")
    )
)]
pub struct ApiDoc;
