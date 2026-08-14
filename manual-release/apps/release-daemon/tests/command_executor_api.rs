use release_daemon::executor::command_executor::CommandExecutor;
use std::time::Duration;

#[actix_web::test]
async fn test_step22_command_timeout() {
    let executor = CommandExecutor::new(2 * 1024 * 1024);
    let res = executor
        .run(
            "sleep",
            &["5"],
            std::path::Path::new("."),
            Duration::from_millis(100),
        )
        .await;
    assert!(res.is_err());
    let err_str = res.unwrap_err().to_string();
    assert!(err_str.contains("timed out"), "Error was: {}", err_str);
}

#[actix_web::test]
async fn test_step23_output_bounding() {
    let executor = CommandExecutor::new(1024); // 1KB limit
    let res = executor
        .run(
            "sh",
            &["-c", "yes | head -n 1000000"],
            std::path::Path::new("."),
            Duration::from_secs(2),
        )
        .await
        .unwrap();
    assert!(res.stdout_truncated);
    assert!(res.stdout.len() <= 1024);
}

#[actix_web::test]
async fn test_step24_invalid_executable() {
    let executor = CommandExecutor::new(2 * 1024 * 1024);
    let res = executor
        .run(
            "guaranteed_nonexistent_executable_123",
            &[],
            std::path::Path::new("."),
            Duration::from_secs(1),
        )
        .await;
    assert!(res.is_err());
}

#[actix_web::test]
async fn test_step25_command_failure() {
    let executor = CommandExecutor::new(2 * 1024 * 1024);
    let res = executor
        .run(
            "sh",
            &["-c", "exit 42"],
            std::path::Path::new("."),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    assert_eq!(res.exit_code, Some(42));
}
