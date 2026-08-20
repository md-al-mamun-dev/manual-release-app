use release_daemon::{
    domain::release::Release, runner::context::RunnerExecutionContext, runner::mock_runner::MockRunner, runner::Runner, services::source_validation_service::SourceValidationService,
};
use sqlx::PgPool;
use std::process::Command;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

fn setup_temp_git_repo(repo_path: &str) -> String {
    std::fs::create_dir_all(repo_path).unwrap();

    Command::new("git")
        .arg("init")
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Create a file and commit
    std::fs::write(format!("{}/test.txt", repo_path), "hello").unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "initial commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let rev_parse = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    String::from_utf8_lossy(&rev_parse.stdout)
        .trim()
        .to_string()
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_source_validation_success(pool: PgPool) {
    let repo_id = Uuid::new_v4();
    let repo_path = format!("/tmp/test_repo_{}", repo_id);
    let sha = setup_temp_git_repo(&repo_path);

    // Create project
    let project_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO projects (id, name, repository_path) VALUES ($1, $2, $3)",
        project_id,
        format!("Proj {}", repo_id),
        repo_path
    )
    .execute(&pool)
    .await
    .unwrap();

    // Create release
    let release_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO releases (id, project_id, version, git_commit, status) VALUES ($1, $2, $3, $4, 'CREATED')",
        release_id,
        project_id,
        "v1.0.0",
        sha
    )
    .execute(&pool)
    .await
    .unwrap();

    let workspace_root = format!("/tmp/workspace_root_{}", Uuid::new_v4());
    let mut runner = MockRunner::new(std::path::PathBuf::from(&workspace_root));
    runner.create().await.unwrap();
    runner.prepare().await.unwrap();

    let validation_service = SourceValidationService::new(pool.clone());

    let job_id = Uuid::new_v4();
    let cancel_token = CancellationToken::new();
    let context = RunnerExecutionContext::new(&runner, cancel_token);

    let result = validation_service
        .validate_source(job_id, release_id, &context)
        .await;

    assert!(
        result.is_ok(),
        "Validation should succeed, got {:?}",
        result.err()
    );

    let workspace_path = result.unwrap();
    assert!(workspace_path.exists());
    assert!(workspace_path.starts_with(&workspace_root));
    assert!(workspace_path.join(".git").exists());

    // Verify release status
    let updated_release = sqlx::query_as!(
        Release,
        "SELECT id, project_id, source_inspection_id, version, git_commit, git_branch, source_dirty, status, requested_by, created_at, updated_at FROM releases WHERE id = $1",
        release_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(updated_release.status, "SOURCE_VALIDATED");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_source_validation_invalid_sha(pool: PgPool) {
    let repo_id = Uuid::new_v4();
    let repo_path = format!("/tmp/test_repo_{}", repo_id);
    setup_temp_git_repo(&repo_path);

    // Create project
    let project_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO projects (id, name, repository_path) VALUES ($1, $2, $3)",
        project_id,
        format!("Proj {}", repo_id),
        repo_path
    )
    .execute(&pool)
    .await
    .unwrap();

    // Create release with fake SHA
    let release_id = Uuid::new_v4();
    let fake_sha = "1234567890123456789012345678901234567890";
    sqlx::query!(
        "INSERT INTO releases (id, project_id, version, git_commit, status) VALUES ($1, $2, $3, $4, 'CREATED')",
        release_id,
        project_id,
        "v1.0.0",
        fake_sha
    )
    .execute(&pool)
    .await
    .unwrap();

    let workspace_root = format!("/tmp/workspace_root_{}", Uuid::new_v4());
    let mut runner = MockRunner::new(std::path::PathBuf::from(&workspace_root));
    runner.create().await.unwrap();
    runner.prepare().await.unwrap();

    let validation_service = SourceValidationService::new(pool.clone());

    let job_id = Uuid::new_v4();
    let cancel_token = CancellationToken::new();
    let context = RunnerExecutionContext::new(&runner, cancel_token);

    let result = validation_service
        .validate_source(job_id, release_id, &context)
        .await;

    assert!(result.is_err(), "Validation should fail for fake SHA");

    // Status should remain CREATED
    let updated_release = sqlx::query_as!(
        Release,
        "SELECT id, project_id, source_inspection_id, version, git_commit, git_branch, source_dirty, status, requested_by, created_at, updated_at FROM releases WHERE id = $1",
        release_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(updated_release.status, "CREATED");
}
