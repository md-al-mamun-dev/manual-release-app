use release_daemon::{
    domain::job::{append_job_event, fail_job, fail_step, succeed_step},
    repositories::release_repository::ReleaseRepository,
    runner::{context::RunnerExecutionContext, mock_runner::MockRunner, Runner},
    services::node_ci_service::{NodeCiError, NodeCiService},
};
use sqlx::PgPool;
use std::fs;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

async fn setup_job_and_step(pool: &PgPool) -> (Uuid, Uuid) {
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
        "INSERT INTO job_steps (id, job_id, step_key, step_order, status) VALUES ($1, $2, 'NODE_CI', 1, 'RUNNING')",
        step_id,
        job_id
    )
    .execute(pool)
    .await
    .unwrap();

    (job_id, step_id)
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_node_ci_success_npm(pool: PgPool) {
    let (job_id, step_id) = setup_job_and_step(&pool).await;

    let workspace_path = format!("/tmp/node_ci_workspace_{}", Uuid::new_v4());
    fs::create_dir_all(&workspace_path).unwrap();

    // Setup dummy package.json and lockfile
    let package_json = r#"{
        "name": "dummy",
        "scripts": {
            "lint": "echo LINT",
            "typecheck": "echo TYPECHECK",
            "test": "echo TEST",
            "build": "echo BUILD"
        }
    }"#;
    fs::write(format!("{}/package.json", workspace_path), package_json).unwrap();
    fs::write(format!("{}/package-lock.json", workspace_path), "{}").unwrap();

    let service = NodeCiService::new(pool.clone());
    let cancel_token = CancellationToken::new();
    let runner_workspace = PathBuf::from(&workspace_path);
    let mut runner = MockRunner::new(runner_workspace.clone());
    runner.create().await.unwrap();
    runner.prepare().await.unwrap();
    
    let context = RunnerExecutionContext::new(&runner, cancel_token.clone());

    let result = service
        .execute_ci(
            job_id,
            step_id,
            &runner_workspace,
            cancel_token,
            &context,
        )
        .await;
    assert!(
        result.is_ok(),
        "Node CI should succeed with dummy scripts, got {:?}",
        result.err()
    );

    // Verify events were persisted
    let events = sqlx::query!(
        "SELECT stream, message FROM job_events WHERE job_id = $1 ORDER BY id",
        job_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let mut found_lint = false;
    let mut found_test = false;
    for event in events {
        if event.message.contains("LINT") {
            found_lint = true;
        }
        if event.message.contains("TEST") {
            found_test = true;
        }
    }
    assert!(found_lint, "Expected LINT output in job events");
    assert!(found_test, "Expected TEST output in job events");

    fs::remove_dir_all(&workspace_path).unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_node_ci_multiple_lockfiles(pool: PgPool) {
    let (job_id, step_id) = setup_job_and_step(&pool).await;

    let workspace_path = format!("/tmp/node_ci_workspace_{}", Uuid::new_v4());
    fs::create_dir_all(&workspace_path).unwrap();

    fs::write(format!("{}/package.json", workspace_path), "{}").unwrap();
    fs::write(format!("{}/package-lock.json", workspace_path), "{}").unwrap();
    fs::write(format!("{}/yarn.lock", workspace_path), "{}").unwrap();

    let service = NodeCiService::new(pool.clone());
    let cancel_token = CancellationToken::new();
    let runner_workspace = PathBuf::from(&workspace_path);
    let mut runner = MockRunner::new(runner_workspace.clone());
    runner.create().await.unwrap();
    runner.prepare().await.unwrap();
    
    let context = RunnerExecutionContext::new(&runner, cancel_token.clone());

    let result = service
        .execute_ci(
            job_id,
            step_id,
            &runner_workspace,
            cancel_token,
            &context,
        )
        .await;

    assert!(result.is_err());
    if let Err(NodeCiError::MultipleLockfiles(_)) = result {
        // Expected
    } else {
        panic!("Expected MultipleLockfiles error, got {:?}", result);
    }

    fs::remove_dir_all(&workspace_path).unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_node_ci_script_failure(pool: PgPool) {
    let (job_id, step_id) = setup_job_and_step(&pool).await;

    let workspace_path = format!("/tmp/node_ci_workspace_{}", Uuid::new_v4());
    fs::create_dir_all(&workspace_path).unwrap();

    let package_json = r#"{
        "name": "dummy",
        "scripts": {
            "test": "exit 1"
        }
    }"#;
    fs::write(format!("{}/package.json", workspace_path), package_json).unwrap();
    fs::write(format!("{}/yarn.lock", workspace_path), "{}").unwrap();

    let service = NodeCiService::new(pool.clone());
    let cancel_token = CancellationToken::new();
    let runner_workspace = PathBuf::from(&workspace_path);
    let mut runner = MockRunner::new(runner_workspace.clone());
    runner.create().await.unwrap();
    runner.prepare().await.unwrap();
    
    let context = RunnerExecutionContext::new(&runner, cancel_token.clone());

    let result = service
        .execute_ci(
            job_id,
            step_id,
            &runner_workspace,
            cancel_token,
            &context,
        )
        .await;

    assert!(result.is_err());
    if let Err(NodeCiError::ExecutionFailed(_)) = result {
        // Expected
    } else {
        panic!("Expected ExecutionFailed error, got {:?}", result);
    }

    fs::remove_dir_all(&workspace_path).unwrap();
}
