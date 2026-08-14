use actix_web::{App, http::StatusCode, test, web};
use release_daemon::{
    app_state::AppState,
    domain::{
        project::Project,
        release::{Release, ReleaseStateTransition, ReleaseStatus},
    },
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
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

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

macro_rules! setup_project {
    ($app:expr, $name:expr) => {{
        let create_request = json!({
            "name": $name.to_string(),
            "repositoryPath": format!("/tmp/{}", uuid::Uuid::new_v4().to_string()),
        });
        let req = test::TestRequest::post()
            .uri("/api/projects")
            .set_json(&create_request)
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let created: Project = test::read_body_json(resp).await;
        created
    }};
}

async fn create_inspection(
    pool: &PgPool,
    project_id: Uuid,
    status: &str,
    git_dirty: Option<bool>,
) -> Uuid {
    let inspection_repo = ProjectInspectionRepository::new(pool.clone());
    let inspection = inspection_repo.start(project_id).await.unwrap();

    if status == "SUCCEEDED" {
        inspection_repo
            .succeed(
                inspection.id,
                "/tmp/repo",
                "abcdef1234567890abcdef123456789012345678",
                Some("main"),
                git_dirty.unwrap_or(false),
                json!({}),
            )
            .await
            .unwrap();
    } else if status == "FAILED" {
        inspection_repo
            .fail(inspection.id, "TEST_ERROR", "Test error")
            .await
            .unwrap();
    }

    inspection.id
}

#[sqlx::test(migrations = "../../migrations")]
async fn successful_release(pool: PgPool) {
    let state = create_state(pool.clone());
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let project = setup_project!(app, "Success Test");
    let inspection_id = create_inspection(&pool, project.id, "SUCCEEDED", Some(false)).await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/projects/{}/releases", project.id))
        .set_json(&json!({
            "version": "v1.0.0",
            "inspectionId": inspection_id,
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let release: Release = test::read_body_json(resp).await;
    assert_eq!(release.project_id, project.id);
    assert_eq!(release.source_inspection_id, Some(inspection_id));
    assert_eq!(release.version, "v1.0.0");
    assert_eq!(release.git_commit, "abcdef1234567890abcdef123456789012345678");
    assert_eq!(release.git_branch.as_deref(), Some("main"));
    assert_eq!(release.source_dirty, false);
    assert_eq!(release.status, ReleaseStatus::Created.as_db_str());

    let req = test::TestRequest::get()
        .uri(&format!("/api/releases/{}/transitions", release.id))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let transitions: Vec<ReleaseStateTransition> = test::read_body_json(resp).await;
    assert_eq!(transitions.len(), 1);
    assert_eq!(transitions[0].from_status, None);
    assert_eq!(transitions[0].to_status, ReleaseStatus::Created.as_db_str());
}

#[sqlx::test(migrations = "../../migrations")]
async fn duplicate_release_version(pool: PgPool) {
    let state = create_state(pool.clone());
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let project = setup_project!(app, "Duplicate Version Test");
    let inspection_id = create_inspection(&pool, project.id, "SUCCEEDED", Some(false)).await;

    let req1 = test::TestRequest::post()
        .uri(&format!("/api/projects/{}/releases", project.id))
        .set_json(&json!({
            "version": "v1.0.0",
            "inspectionId": inspection_id,
        }))
        .to_request();
    let resp1 = test::call_service(&app, req1).await;
    assert_eq!(resp1.status(), StatusCode::CREATED);

    let req2 = test::TestRequest::post()
        .uri(&format!("/api/projects/{}/releases", project.id))
        .set_json(&json!({
            "version": "v1.0.0",
            "inspectionId": inspection_id,
        }))
        .to_request();
    let resp2 = test::call_service(&app, req2).await;
    assert_eq!(resp2.status(), StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "../../migrations")]
async fn dirty_inspection_rejected(pool: PgPool) {
    let state = create_state(pool.clone());
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let project = setup_project!(app, "Dirty Test");
    let inspection_id = create_inspection(&pool, project.id, "SUCCEEDED", Some(true)).await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/projects/{}/releases", project.id))
        .set_json(&json!({
            "version": "v1.0.0",
            "inspectionId": inspection_id,
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../../migrations")]
async fn failed_inspection_rejected(pool: PgPool) {
    let state = create_state(pool.clone());
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let project = setup_project!(app, "Failed Insp Test");
    let inspection_id = create_inspection(&pool, project.id, "FAILED", None).await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/projects/{}/releases", project.id))
        .set_json(&json!({
            "version": "v1.0.0",
            "inspectionId": inspection_id,
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../../migrations")]
async fn wrong_project_inspection_rejected(pool: PgPool) {
    let state = create_state(pool.clone());
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let project1 = setup_project!(app, "Project 1");
    let project2 = setup_project!(app, "Project 2");

    let inspection_id = create_inspection(&pool, project1.id, "SUCCEEDED", Some(false)).await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/projects/{}/releases", project2.id))
        .set_json(&json!({
            "version": "v1.0.0",
            "inspectionId": inspection_id,
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../../migrations")]
async fn unknown_inspection_rejected(pool: PgPool) {
    let state = create_state(pool.clone());
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let project = setup_project!(app, "Unknown Insp Test");

    let req = test::TestRequest::post()
        .uri(&format!("/api/projects/{}/releases", project.id))
        .set_json(&json!({
            "version": "v1.0.0",
            "inspectionId": Uuid::new_v4(),
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn archived_project_rejected(pool: PgPool) {
    let state = create_state(pool.clone());
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let project = setup_project!(app, "Archived Test");
    let inspection_id = create_inspection(&pool, project.id, "SUCCEEDED", Some(false)).await;

    let del_req = test::TestRequest::delete()
        .uri(&format!("/api/projects/{}", project.id))
        .to_request();
    let del_resp = test::call_service(&app, del_req).await;
    assert_eq!(del_resp.status(), StatusCode::NO_CONTENT);

    let req = test::TestRequest::post()
        .uri(&format!("/api/projects/{}/releases", project.id))
        .set_json(&json!({
            "version": "v1.0.0",
            "inspectionId": inspection_id,
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
