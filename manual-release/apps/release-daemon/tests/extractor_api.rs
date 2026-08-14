use actix_web::{App, http::StatusCode, test, web};
use release_daemon::{
    app_state::AppState,
    executor::command_executor::CommandExecutor,
    inspection::project_inspector::ProjectInspector,
    repositories::{
        project_inspection_repository::ProjectInspectionRepository,
        project_repository::ProjectRepository,
    },
    routes,
    services::{
        project_inspection_service::ProjectInspectionService, project_service::ProjectService,
    },
};
use sqlx::PgPool;

fn create_state(pool: PgPool) -> web::Data<AppState> {
    let project_repository = ProjectRepository::new(pool.clone());
    let project_service = ProjectService::new(project_repository.clone());

    let command_executor = CommandExecutor::new(1024 * 1024);
    let project_inspector = ProjectInspector::new(command_executor);

    let inspection_repository = ProjectInspectionRepository::new(pool.clone());
    let project_inspection_service =
        ProjectInspectionService::new(project_repository, inspection_repository, project_inspector);

    web::Data::new(AppState {
        pool,
        project_service,
        project_inspection_service,
    })
}

#[sqlx::test(migrations = "../../migrations")]
async fn project_extractor_error_format(pool: PgPool) {
    let state = create_state(pool);
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    // Send malformed JSON (missing required field 'name')
    let invalid_json = r#"{
        "repositoryPath": "/tmp/test"
    }"#;

    let req = test::TestRequest::post()
        .uri("/api/projects") // wait, routes are mounted at /api/projects or just /projects?
        .insert_header(("content-type", "application/json"))
        .set_payload(invalid_json)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // we expect it to match ApiError format: {"error": {"code": "VALIDATION_ERROR", "message": "..."}}
    let body_bytes = test::read_body(resp).await;
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(
        body_str.contains("\"code\":\"VALIDATION_ERROR\""),
        "Body did not contain validation error code: {}",
        body_str
    );
}
