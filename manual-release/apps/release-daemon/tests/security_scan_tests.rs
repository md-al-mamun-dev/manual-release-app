use chrono::Utc;
use release_daemon::{
    domain::{
        project::CreateProjectRequest, security_policy::SecurityPolicy, security_scan::SecurityScan,
    },
    repositories::{
        project_inspection_repository::ProjectInspectionRepository,
        project_repository::ProjectRepository,
        release_repository::{CreateReleaseInput, ReleaseRepository},
        security_scan_repository::SecurityScanRepository,
    },
};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[test]
fn test_security_policy_defaults() {
    let policy = SecurityPolicy::default();
    assert!(policy.fail_on_critical);
    assert!(policy.fail_on_high);
    assert!(!policy.fail_on_medium);
    assert!(!policy.fail_on_low);

    let new_policy = SecurityPolicy::new();
    assert_eq!(policy, new_policy);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_security_scan_persistence_and_latest(pool: PgPool) {
    let project_repo = ProjectRepository::new(pool.clone());
    let inspection_repo = ProjectInspectionRepository::new(pool.clone());
    let release_repo = ReleaseRepository::new(pool.clone());
    let scan_repo = SecurityScanRepository::new(pool.clone());

    let project_id = Uuid::new_v4();
    let project_req = CreateProjectRequest {
        name: format!("Security Scan Project {}", project_id),
        repository_path: format!("/tmp/sec-repo-{}", project_id),
        repository_url: None,
        default_branch: Some("main".to_string()),
    };
    let project = project_repo.create(project_id, &project_req).await.unwrap();

    let inspection = inspection_repo.start(project.id).await.unwrap();
    inspection_repo
        .succeed(
            inspection.id,
            "/tmp/sec-repo",
            "0123456789abcdef0123456789abcdef01234567",
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
            git_commit: "0123456789abcdef0123456789abcdef01234567",
            git_branch: Some("main"),
            source_dirty: false,
            requested_by: None,
            actor: "TEST_OPERATOR",
        })
        .await
        .unwrap();

    assert_eq!(release.id, release_id);

    // Initial query for latest scan should be None
    let initial_latest = scan_repo
        .find_latest_by_release_id(release_id)
        .await
        .unwrap();
    assert!(initial_latest.is_none());

    // Create first scan
    let scan1_id = Uuid::new_v4();
    let scan1 = SecurityScan {
        id: scan1_id,
        release_id,
        image_digest: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
            .to_string(),
        critical_vulnerabilities: 2,
        high_vulnerabilities: 5,
        medium_vulnerabilities: 10,
        low_vulnerabilities: 20,
        passed: false,
        report_json: json!({
            "SchemaVersion": 2,
            "Results": [{"Target": "app", "Vulnerabilities": [{"VulnerabilityID": "CVE-2026-0001", "Severity": "CRITICAL"}]}]
        }),
        created_at: Utc::now() - chrono::Duration::seconds(60),
    };

    scan_repo.create(&scan1).await.unwrap();

    // Query by ID
    let fetched1 = scan_repo.find_by_id(scan1_id).await.unwrap().unwrap();
    assert_eq!(fetched1.id, scan1_id);
    assert_eq!(fetched1.release_id, release_id);
    assert_eq!(fetched1.image_digest, scan1.image_digest);
    assert_eq!(fetched1.critical_vulnerabilities, 2);
    assert_eq!(fetched1.high_vulnerabilities, 5);
    assert_eq!(fetched1.medium_vulnerabilities, 10);
    assert_eq!(fetched1.low_vulnerabilities, 20);
    assert!(!fetched1.passed);

    // Latest scan should be scan1
    let latest1 = scan_repo
        .find_latest_by_release_id(release_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(latest1.id, scan1_id);

    // Create second scan (newer)
    let scan2_id = Uuid::new_v4();
    let scan2 = SecurityScan {
        id: scan2_id,
        release_id,
        image_digest: "sha256:2222222222222222222222222222222222222222222222222222222222222222"
            .to_string(),
        critical_vulnerabilities: 0,
        high_vulnerabilities: 0,
        medium_vulnerabilities: 1,
        low_vulnerabilities: 3,
        passed: true,
        report_json: json!({
            "SchemaVersion": 2,
            "Results": []
        }),
        created_at: Utc::now(),
    };

    scan_repo.create(&scan2).await.unwrap();

    // Latest scan should now be scan2
    let latest2 = scan_repo
        .find_latest_by_release_id(release_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(latest2.id, scan2_id);
    assert!(latest2.passed);
    assert_eq!(latest2.critical_vulnerabilities, 0);
    assert_eq!(latest2.high_vulnerabilities, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_security_scan_foreign_key_cascade(pool: PgPool) {
    let project_repo = ProjectRepository::new(pool.clone());
    let inspection_repo = ProjectInspectionRepository::new(pool.clone());
    let release_repo = ReleaseRepository::new(pool.clone());
    let scan_repo = SecurityScanRepository::new(pool.clone());

    let project_id = Uuid::new_v4();
    let project_req = CreateProjectRequest {
        name: format!("Cascade Test Project {}", project_id),
        repository_path: format!("/tmp/cascade-repo-{}", project_id),
        repository_url: None,
        default_branch: Some("main".to_string()),
    };
    let project = project_repo.create(project_id, &project_req).await.unwrap();

    let inspection = inspection_repo.start(project.id).await.unwrap();
    inspection_repo
        .succeed(
            inspection.id,
            "/tmp/cascade-repo",
            "abcdefabcdefabcdefabcdefabcdefabcdefabcd",
            Some("main"),
            false,
            json!({}),
        )
        .await
        .unwrap();

    let release_id = Uuid::new_v4();
    release_repo
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

    let scan_id = Uuid::new_v4();
    let scan = SecurityScan {
        id: scan_id,
        release_id,
        image_digest: "sha256:3333333333333333333333333333333333333333333333333333333333333333"
            .to_string(),
        critical_vulnerabilities: 0,
        high_vulnerabilities: 0,
        medium_vulnerabilities: 0,
        low_vulnerabilities: 0,
        passed: true,
        report_json: json!({}),
        created_at: Utc::now(),
    };

    scan_repo.create(&scan).await.unwrap();
    assert!(scan_repo.find_by_id(scan_id).await.unwrap().is_some());

    // Deleting the release should cascade delete the security scan
    sqlx::query!("DELETE FROM releases WHERE id = $1", release_id)
        .execute(&pool)
        .await
        .unwrap();

    let fetched = scan_repo.find_by_id(scan_id).await.unwrap();
    assert!(fetched.is_none());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_security_scan_invalid_foreign_key_fails(pool: PgPool) {
    let scan_repo = SecurityScanRepository::new(pool.clone());
    let non_existent_release_id = Uuid::new_v4();

    let scan = SecurityScan {
        id: Uuid::new_v4(),
        release_id: non_existent_release_id,
        image_digest: "sha256:4444444444444444444444444444444444444444444444444444444444444444"
            .to_string(),
        critical_vulnerabilities: 0,
        high_vulnerabilities: 0,
        medium_vulnerabilities: 0,
        low_vulnerabilities: 0,
        passed: true,
        report_json: json!({}),
        created_at: Utc::now(),
    };

    let result = scan_repo.create(&scan).await;
    assert!(result.is_err());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_scanner_malformed_report_fails_safely(pool: PgPool) {
    use release_daemon::config::AppConfig;
    use release_daemon::domain::build_plan::BuildPlan;
    use release_daemon::domain::project_build_config::{
        ApplicationType, Framework, PackageManager,
    };
    use release_daemon::runner::context::RunnerExecutionContext;
    use release_daemon::services::build_plan_executor::BuildPlanExecutor;
    use std::path::PathBuf;
    use tokio_util::sync::CancellationToken;

    let project_repo = ProjectRepository::new(pool.clone());
    let inspection_repo = ProjectInspectionRepository::new(pool.clone());
    let release_repo = ReleaseRepository::new(pool.clone());

    let project_id = Uuid::new_v4();
    let project_req = CreateProjectRequest {
        name: format!("Malformed Scan Proj {}", project_id),
        repository_path: format!("/tmp/malformed-repo-{}", project_id),
        repository_url: None,
        default_branch: Some("main".to_string()),
    };
    let project = project_repo.create(project_id, &project_req).await.unwrap();

    let inspection = inspection_repo.start(project.id).await.unwrap();
    inspection_repo
        .succeed(
            inspection.id,
            "/tmp/malformed-repo",
            "abcdefabcdefabcdefabcdefabcdefabcdefabcd",
            Some("main"),
            false,
            json!({}),
        )
        .await
        .unwrap();

    let release_id = Uuid::new_v4();
    release_repo
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

    let job_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO jobs (id, project_id, release_id, job_type, status) VALUES ($1, $2, $3, 'PREPARE_RELEASE', 'RUNNING')",
        job_id,
        project.id,
        release_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Persist ReleaseImage
    let release_img = release_daemon::domain::release_image::ReleaseImage {
        id: Uuid::new_v4(),
        release_id,
        job_id,
        git_sha: "sha123".to_string(),
        image_tag: "target-img:sha123".to_string(),
        image_digest: "sha256:digest1234567890".to_string(),
        created_at: chrono::Utc::now(),
        registry: None,
        repository: None,
        remote_digest: None,
        publication_status: None,
        updated_at: None,
    };
    let img_repo =
        release_daemon::repositories::release_image_repository::ReleaseImageRepository::new(
            pool.clone(),
        );
    img_repo.create_release_image(&release_img).await.unwrap();

    let workspace = PathBuf::from(format!("/tmp/malformed-test-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&workspace).await.unwrap();

    // Write malformed report JSON into workspace
    tokio::fs::write(workspace.join("trivy-report.json"), "invalid json content")
        .await
        .unwrap();

    let config = AppConfig {
        database_url: "postgres://dummy".to_string(),
        backend_host: "localhost".to_string(),
        backend_port: 8080,
        job_workspace_root: workspace.display().to_string(),
        runner_type: "MOCK".to_string(),
        runner_timeout_seconds: 3600,
        runner_cleanup_on_success: true,
        runner_cleanup_on_failure: true,
        runner_ubuntu_image: "ubuntu:22.04".to_string(),
        runner_memory_limit: "512m".to_string(),
        runner_cpus_limit: "1.0".to_string(),
        runner_pids_limit: "100".to_string(),
        runner_network_policy: "bridge".to_string(),
        kaniko_memory_limit: "1024m".to_string(),
        kaniko_cpus_limit: "2.0".to_string(),
        kaniko_pids_limit: "200".to_string(),
        max_image_tar_size: 1073741824,
        max_trivy_report_size: 10485760,
        registry_url: "".into(),
        registry_repository: "".into(),
        registry_username: "".into(),
        registry_password: "".into(),
    };

    let mut mock_runner = release_daemon::runner::mock_runner::MockRunner::new(
        workspace.clone(),
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
    );
    mock_runner.fail_trivy_parse = true;
    let mut runner: Box<dyn release_daemon::runner::Runner> = Box::new(mock_runner);
    runner.create().await.unwrap();
    runner.prepare().await.unwrap();
    let context = RunnerExecutionContext::new(runner.as_mut(), CancellationToken::new());

    let build_plan = BuildPlan {
        source_sha: "sha123".to_string(),
        application_type: ApplicationType::Node,
        framework: Framework::Nextjs,
        runtime: "20".to_string(),
        package_manager: PackageManager::Npm,
        install_command: None,
        lint_command: None,
        typecheck_command: None,
        test_command: None,
        build_command: None,
        build_image_command: None,
        test_image_command: None,
        dockerfile: Some("Dockerfile".to_string()),
        docker_context: Some(".".to_string()),
        application_port: Some(8080),
        health_endpoint: Some("/".to_string()),
    };

    let policy = SecurityPolicy::default();
    let executor = BuildPlanExecutor::new(pool.clone(), config.clone());
    let result = executor
        .execute_image_scan(&build_plan, release_id, job_id, &policy, &context)
        .await;

    assert!(result.is_err(), "Malformed report should fail safely");
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("Failed to parse Trivy report JSON"));

    let _ = tokio::fs::remove_dir_all(&workspace).await;
}
