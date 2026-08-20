use actix_web::{App, http::StatusCode, test, web};
use release_daemon::{
    app_state::AppState,
    domain::project::{CreateProjectRequest, Project, UpdateProjectRequest},
    executor::command_executor::CommandExecutor,
    inspection::project_inspector::ProjectInspector,
    repositories::{
        environment_repository::EnvironmentRepository,
        project_inspection_repository::ProjectInspectionRepository,
        project_repository::ProjectRepository, release_repository::ReleaseRepository,
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
async fn project_crud_flow(pool: PgPool) {
    let state = create_state(pool);

    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let create_request = CreateProjectRequest {
        name: "Integration Test".to_string(),

        repository_path: "/tmp/integration-test".to_string(),

        repository_url: None,

        default_branch: Some("main".to_string()),
    };

    let request = test::TestRequest::post()
        .uri("/api/projects")
        .set_json(&create_request)
        .to_request();

    let response = test::call_service(&app, request).await;

    assert_eq!(response.status(), StatusCode::CREATED);

    let created: Project = test::read_body_json(response).await;

    assert_eq!(created.name, "Integration Test");

    let request = test::TestRequest::get()
        .uri(&format!("/api/projects/{}", created.id))
        .to_request();

    let response = test::call_service(&app, request).await;

    assert_eq!(response.status(), StatusCode::OK);

    let update_request = UpdateProjectRequest {
        name: Some("Updated Integration Test".to_string()),
        repository_path: None,
        repository_url: None,
        default_branch: None,
    };

    let request = test::TestRequest::patch()
        .uri(&format!("/api/projects/{}", created.id))
        .set_json(&update_request)
        .to_request();

    let response = test::call_service(&app, request).await;

    assert_eq!(response.status(), StatusCode::OK);

    let updated: Project = test::read_body_json(response).await;

    assert_eq!(updated.name, "Updated Integration Test");

    let request = test::TestRequest::delete()
        .uri(&format!("/api/projects/{}", created.id))
        .to_request();

    let response = test::call_service(&app, request).await;

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let request = test::TestRequest::get()
        .uri(&format!("/api/projects/{}", created.id))
        .to_request();

    let response = test::call_service(&app, request).await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn project_conflict_handling(pool: PgPool) {
    let state = create_state(pool);
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let create_request = CreateProjectRequest {
        name: "Conflict Test".to_string(),
        repository_path: "/tmp/conflict".to_string(),
        repository_url: None,
        default_branch: None,
    };

    let req1 = test::TestRequest::post()
        .uri("/api/projects")
        .set_json(&create_request)
        .to_request();
    let resp1 = test::call_service(&app, req1).await;
    assert_eq!(resp1.status(), StatusCode::CREATED);

    let req2 = test::TestRequest::post()
        .uri("/api/projects")
        .set_json(&create_request)
        .to_request();
    let resp2 = test::call_service(&app, req2).await;
    assert_eq!(resp2.status(), StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "../../migrations")]
async fn project_validation_rejection(pool: PgPool) {
    let state = create_state(pool);
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let invalid_request = CreateProjectRequest {
        name: "".to_string(),
        repository_path: "relative/path".to_string(),
        repository_url: None,
        default_branch: None,
    };

    let req = test::TestRequest::post()
        .uri("/api/projects")
        .set_json(&invalid_request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../migrations")]
async fn project_soft_archive_duplicate_creation(pool: PgPool) {
    let state = create_state(pool);
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let create_request = CreateProjectRequest {
        name: "Archived Project".to_string(),
        repository_path: "/tmp/archived".to_string(),
        repository_url: None,
        default_branch: None,
    };

    // 1. Create
    let req = test::TestRequest::post()
        .uri("/api/projects")
        .set_json(&create_request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let created: Project = test::read_body_json(resp).await;

    // 2. Archive
    let req = test::TestRequest::delete()
        .uri(&format!("/api/projects/{}", created.id))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // 3. Recreate with same details
    let req = test::TestRequest::post()
        .uri("/api/projects")
        .set_json(&create_request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "Should allow creating a project with the same name if the previous one is archived"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn project_patch_unset_field(pool: PgPool) {
    let state = create_state(pool);
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let create_request = CreateProjectRequest {
        name: "Patch Test".to_string(),
        repository_path: "/tmp/patch".to_string(),
        repository_url: Some("https://example.com/repo.git".to_string()),
        default_branch: Some("main".to_string()),
    };

    let req = test::TestRequest::post()
        .uri("/api/projects")
        .set_json(&create_request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let created: Project = test::read_body_json(resp).await;

    // We send a JSON patch setting `default_branch` and `repository_url` to null.
    // In Rust, using Option<Option<T>> mapped via serde means null becomes Some(None).
    // Let's test this by sending raw JSON bytes to the endpoint.
    let patch_json = r#"{
        "defaultBranch": null,
        "repositoryUrl": null
    }"#;

    let req = test::TestRequest::patch()
        .uri(&format!("/api/projects/{}", created.id))
        .insert_header(("content-type", "application/json"))
        .set_payload(patch_json)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let updated: Project = test::read_body_json(resp).await;
    assert_eq!(
        updated.default_branch, None,
        "default_branch should be unset (None)"
    );
    assert_eq!(
        updated.repository_url, None,
        "repository_url should be unset (None)"
    );
    assert_eq!(updated.name, "Patch Test", "name should remain unchanged");
}
