use release_daemon::{
    config::AppConfig, runner::manager::RunnerManager,
    services::source_validation_service::SourceValidationService,
    worker::prepare_release_executor::PrepareReleaseExecutor,
    workspace::git_workspace::GitWorkspaceManager,
};
use sqlx::PgPool;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

async fn setup_db(pool: &PgPool) -> (Uuid, Uuid, Uuid, Uuid) {
    let project_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO projects (id, name, repository_path) VALUES ($1, $2, $3)",
        project_id,
        format!("Proj {}", project_id),
        "/tmp/dummy"
    )
    .execute(pool)
    .await
    .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO jobs (id, project_id, job_type, status) VALUES ($1, $2, 'PREPARE_RELEASE', 'RUNNING')",
        job_id,
        project_id
    )
    .execute(pool)
    .await
    .unwrap();

    let step_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO job_steps (id, job_id, step_key, step_order, status) VALUES ($1, $2, 'SOURCE_VALIDATION', 0, 'RUNNING')",
        step_id,
        job_id
    )
    .execute(pool)
    .await
    .unwrap();

    let release_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO releases (id, project_id, version, git_commit, git_branch, status, requested_by) VALUES ($1, $2, '1.0.0', '1234567890123456789012345678901234567890', 'main', 'CREATED', 'test_user')",
        release_id,
        project_id
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query!(
        "INSERT INTO project_build_configs (id, project_id, application_type, framework, runtime_version, package_manager) VALUES ($1, $2, 'NODE', 'UNKNOWN', '20', 'NPM')",
        Uuid::new_v4(),
        project_id
    )
    .execute(pool)
    .await
    .unwrap();

    (project_id, job_id, step_id, release_id)
}

fn create_executor(pool: PgPool, runner_type: &str) -> PrepareReleaseExecutor {
    let validation_service = SourceValidationService::new(pool.clone());

    let config = AppConfig {
        database_url: "postgres://dummy".to_string(),
        backend_host: "localhost".to_string(),
        backend_port: 8080,
        job_workspace_root: "/tmp/dummy".to_string(),
        runner_type: runner_type.to_string(),
        runner_timeout_seconds: 3600,
        runner_cleanup_on_success: true,
        runner_cleanup_on_failure: true,
        runner_ubuntu_image: "ubuntu:22.04".to_string(),
        runner_memory_limit: "512m".to_string(),
        runner_cpus_limit: "1.0".to_string(),
        runner_pids_limit: "100".to_string(),
        runner_network_policy: "bridge".to_string(),
    };

    let runner_manager = RunnerManager::new(config);
    let workspace_manager = GitWorkspaceManager::new(PathBuf::from("/tmp/cicd_workspaces"));

    PrepareReleaseExecutor::new(pool, validation_service, runner_manager, workspace_manager)
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_runner_create_failure_transitions_to_failed(pool: PgPool) {
    let (_, job_id, step_id, release_id) = setup_db(&pool).await;
    let executor = create_executor(pool.clone(), "MOCK_FAIL_CREATE");
    let cancel_token = CancellationToken::new();

    let result = executor
        .execute(job_id, release_id, step_id, cancel_token)
        .await;

    assert!(
        result.is_err(),
        "Executor should fail when runner create fails"
    );

    let release = sqlx::query!("SELECT status FROM releases WHERE id = $1", release_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(release.status, "FAILED", "Release status should be FAILED");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_runner_prepare_failure_transitions_to_failed_and_cleans_up(pool: PgPool) {
    let (_, job_id, step_id, release_id) = setup_db(&pool).await;
    let executor = create_executor(pool.clone(), "MOCK_FAIL_PREPARE");
    let cancel_token = CancellationToken::new();

    let result = executor
        .execute(job_id, release_id, step_id, cancel_token)
        .await;

    assert!(
        result.is_err(),
        "Executor should fail when runner prepare fails"
    );

    let release = sqlx::query!("SELECT status FROM releases WHERE id = $1", release_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(release.status, "FAILED", "Release status should be FAILED");

    let events = sqlx::query!(
        "SELECT message FROM job_events WHERE job_id = $1 ORDER BY id",
        job_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let event_messages: Vec<String> = events.into_iter().map(|e| e.message).collect();

    // Check if cleanup was executed
    assert!(
        event_messages
            .iter()
            .any(|msg| msg.contains("Runner cleaned up and destroyed")),
        "Cleanup should run"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_user_cancellation_cleans_up(pool: PgPool) {
    let (_, job_id, step_id, release_id) = setup_db(&pool).await;
    let executor = create_executor(pool.clone(), "MOCK"); // standard mock which blocks inside execute if no cancellation? Wait, our tests just cancel immediately!
    let cancel_token = CancellationToken::new();

    // Trigger cancellation
    cancel_token.cancel();

    let result = executor
        .execute(job_id, release_id, step_id, cancel_token)
        .await;

    assert!(result.is_err(), "Executor should fail when cancelled");
    assert!(
        result.unwrap_err().contains("cancelled"),
        "Error should mention cancellation"
    );

    let release = sqlx::query!("SELECT status FROM releases WHERE id = $1", release_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        release.status, "FAILED",
        "Release status should be FAILED on cancellation"
    );

    let events = sqlx::query!(
        "SELECT message FROM job_events WHERE job_id = $1 ORDER BY id",
        job_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let event_messages: Vec<String> = events.into_iter().map(|e| e.message).collect();
    assert!(
        event_messages
            .iter()
            .any(|msg| msg.contains("Runner cleaned up and destroyed")),
        "Cleanup should run on cancel"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_cleanup_failure_preserves_original_error(pool: PgPool) {
    let (_, job_id, step_id, release_id) = setup_db(&pool).await;
    let executor = create_executor(pool.clone(), "MOCK_FAIL_PREPARE_AND_CLEANUP");
    let cancel_token = CancellationToken::new();

    let result = executor
        .execute(job_id, release_id, step_id, cancel_token)
        .await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("prepare failure"),
        "Should preserve original prepare error"
    );

    let release = sqlx::query!("SELECT status FROM releases WHERE id = $1", release_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(release.status, "FAILED", "Release status should be FAILED");

    let events = sqlx::query!(
        "SELECT message FROM job_events WHERE job_id = $1 ORDER BY id",
        job_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let event_messages: Vec<String> = events.into_iter().map(|e| e.message).collect();

    assert!(
        event_messages
            .iter()
            .any(|msg| msg.contains("Simulated cleanup failure")),
        "Should log cleanup failure"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_prepare_release_executor_pipeline_success_approved(pool: PgPool) {
    let repo_id = Uuid::new_v4();
    let repo_path = format!("/tmp/test_exec_repo_{}", repo_id);
    std::fs::create_dir_all(&repo_path).unwrap();

    let _ = std::process::Command::new("git")
        .arg("init")
        .current_dir(&repo_path)
        .output();
    std::fs::write(
        format!("{}/package.json", repo_path),
        "{\"name\":\"test\",\"version\":\"1.0.0\"}",
    )
    .unwrap();
    std::fs::write(format!("{}/package-lock.json", repo_path), "{\"name\":\"test\",\"version\":\"1.0.0\",\"lockfileVersion\":3,\"packages\":{\"\":{\"name\":\"test\",\"version\":\"1.0.0\"}}}").unwrap();
    let _ = std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output();
    let _ = std::process::Command::new("git")
        .args(["commit", "-m", "initial commit"])
        .current_dir(&repo_path)
        .output();
    let rev_parse = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    let sha = String::from_utf8_lossy(&rev_parse.stdout)
        .trim()
        .to_string();

    let project_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO projects (id, name, repository_path) VALUES ($1, $2, $3)",
        project_id,
        format!("Proj {}", project_id),
        repo_path
    )
    .execute(&pool)
    .await
    .unwrap();

    let job_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO jobs (id, project_id, job_type, status) VALUES ($1, $2, 'PREPARE_RELEASE', 'RUNNING')",
        job_id,
        project_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let step_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO job_steps (id, job_id, step_key, step_order, status) VALUES ($1, $2, 'SOURCE_VALIDATION', 0, 'RUNNING')",
        step_id,
        job_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let release_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO releases (id, project_id, version, git_commit, git_branch, status, requested_by) VALUES ($1, $2, '1.0.0', $3, 'main', 'CREATED', 'test_user')",
        release_id,
        project_id,
        sha
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query!(
        "INSERT INTO project_build_configs (id, project_id, application_type, framework, runtime_version, package_manager) VALUES ($1, $2, 'NODE', 'UNKNOWN', '20', 'NPM')",
        Uuid::new_v4(),
        project_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let executor = create_executor(pool.clone(), "MOCK");
    let cancel_token = CancellationToken::new();

    let result = executor
        .execute(job_id, release_id, step_id, cancel_token)
        .await;

    assert!(
        result.is_ok(),
        "Pipeline should succeed end-to-end: {:?}",
        result.err()
    );

    let release = sqlx::query!("SELECT status FROM releases WHERE id = $1", release_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        release.status, "IMAGE_APPROVED",
        "Release status should reach IMAGE_APPROVED"
    );

    let _ = std::fs::remove_dir_all(&repo_path);
}
