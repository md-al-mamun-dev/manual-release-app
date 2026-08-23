use release_daemon::{
    domain::project::CreateProjectRequest,
    repositories::{
        project_inspection_repository::ProjectInspectionRepository,
        project_repository::ProjectRepository,
        release_repository::{CreateReleaseInput, ReleaseRepository},
    },
};
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[sqlx::test(migrations = "../../migrations")]
async fn test_concurrent_release_transitions(pool: PgPool) {
    let project_repo = ProjectRepository::new(pool.clone());
    let inspection_repo = ProjectInspectionRepository::new(pool.clone());
    let release_repo = Arc::new(ReleaseRepository::new(pool.clone()));

    let project_id = Uuid::new_v4();
    let project_req = CreateProjectRequest {
        name: format!("Concurrent Test Project {}", project_id),
        repository_path: format!("/tmp/concurrent-repo-{}", project_id),
        repository_url: None,
        default_branch: Some("main".to_string()),
    };
    let project = project_repo.create(project_id, &project_req).await.unwrap();

    let inspection = inspection_repo.start(project.id).await.unwrap();
    inspection_repo
        .succeed(
            inspection.id,
            "/tmp/concurrent-repo",
            "abcdefabcdefabcdefabcdefabcdefabcdefabcd",
            Some("main"),
            false,
            json!({}),
        )
        .await
        .unwrap();

    let release_id = Uuid::new_v4();
    let release = release_repo
        .create_with_initial_transition(CreateReleaseInput {
            id: release_id,
            project_id: project.id,
            source_inspection_id: inspection.id,
            version: "v1.0.0",
            git_commit: "abcdefabcdefabcdefabcdefabcdefabcdefabcd",
            git_branch: Some("main"),
            source_dirty: false,
            requested_by: None,
            actor: "TEST_OPERATOR",
        })
        .await
        .unwrap();

    assert_eq!(release.status, "CREATED");

    // Spawn 10 concurrent tasks trying to transition CREATED -> PREPARING_RELEASE
    let mut handles = vec![];
    for i in 0..10 {
        let repo = release_repo.clone();
        handles.push(tokio::spawn(async move {
            repo.transition_status(
                release_id,
                "CREATED",
                "IMAGE_BUILDING",
                &format!("actor-{}", i),
                "Concurrency Test",
            )
            .await
        }));
    }

    let mut successes = 0;
    let mut failures = 0;

    for handle in handles {
        let res = handle.await.unwrap();
        match res {
            Ok(_) => successes += 1,
            Err(e) => {
                if matches!(e, sqlx::Error::RowNotFound) {
                    failures += 1;
                } else {
                    panic!("Unexpected error: {:?}", e);
                }
            }
        }
    }

    // Since they all expect from_status = "CREATED", only ONE should succeed because it updates the status!
    assert_eq!(successes, 1, "Exactly one transition should succeed");
    assert_eq!(failures, 9, "The rest should fail with RowNotFound");

    let updated_release = release_repo.find_by_id(release_id).await.unwrap().unwrap();
    assert_eq!(updated_release.status, "IMAGE_BUILDING");
}
