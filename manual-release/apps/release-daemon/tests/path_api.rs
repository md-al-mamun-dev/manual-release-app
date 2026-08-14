use actix_web::{App, http::StatusCode, test, web};
use release_daemon::{
    app_state::AppState,
    executor::command_executor::CommandExecutor,
    inspection::project_inspector::ProjectInspector,
    repositories::{
        environment_repository::EnvironmentRepository,
        project_inspection_repository::ProjectInspectionRepository,
        project_repository::ProjectRepository,
        release_repository::ReleaseRepository,
    },
    routes,
    services::{
        environment_service::EnvironmentService,
        project_inspection_service::ProjectInspectionService, project_service::ProjectService,
        release_service::ReleaseService,
    },
};
use sqlx::PgPool;

fn create_state(pool: PgPool) -> web::Data<AppState> {
    let project_repository = ProjectRepository::new(pool.clone());
    let project_service = ProjectService::new(project_repository.clone());

    let command_executor = CommandExecutor::new(1024 * 1024);
    let project_inspector = ProjectInspector::new(command_executor);

    let inspection_repository = ProjectInspectionRepository::new(pool.clone());
    let inspection_repository_clone = inspection_repository.clone();
    let project_inspection_service = ProjectInspectionService::new(
        project_repository.clone(),
        inspection_repository,
        project_inspector,
    );

    let environment_repository = EnvironmentRepository::new(pool.clone());
    let environment_service =
        EnvironmentService::new(environment_repository, project_repository.clone());

    let release_repository = ReleaseRepository::new(pool.clone());
    let release_service = ReleaseService::new(
        project_repository.clone(),
        inspection_repository_clone,
        release_repository,
    );

    web::Data::new(AppState {
        pool,
        project_service,
        project_inspection_service,
        environment_service,
        release_service,
    })
}

#[sqlx::test(migrations = "../../migrations")]
async fn project_invalid_uuid(pool: PgPool) {
    let state = create_state(pool);
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let req = test::TestRequest::get()
        .uri("/api/projects/not-a-uuid")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
