use std::{collections::HashMap, env, path::PathBuf, time::Duration};

use tokio_util::sync::CancellationToken;

use release_daemon::executor::{process_executor::ProcessExecutor, process_result::ProcessOutcome};

#[tokio::test]
async fn test_successful_command() {
    let executor = ProcessExecutor::new(1024, Duration::from_secs(1));
    let token = CancellationToken::new();

    let result = executor
        .execute(
            "echo",
            &["hello".to_string()],
            &env::current_dir().unwrap(),
            &HashMap::new(),
            Duration::from_secs(5),
            token,
            None,
        )
        .await;

    assert_eq!(result.outcome, ProcessOutcome::Succeeded);
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.stdout.text.trim(), "hello");
}

#[tokio::test]
async fn test_nonzero_exit() {
    let executor = ProcessExecutor::new(1024, Duration::from_secs(1));
    let token = CancellationToken::new();

    let result = executor
        .execute(
            "sh",
            &["-c".to_string(), "exit 42".to_string()],
            &env::current_dir().unwrap(),
            &HashMap::new(),
            Duration::from_secs(5),
            token,
            None,
        )
        .await;

    assert_eq!(result.outcome, ProcessOutcome::NonZeroExit);
    assert_eq!(result.exit_code, Some(42));
}

#[tokio::test]
async fn test_timeout_and_process_group_kill() {
    let executor = ProcessExecutor::new(1024, Duration::from_secs(1));
    let token = CancellationToken::new();

    // Spawn a shell that spawns a sleep to test process group killing
    let result = executor
        .execute(
            "sh",
            &["-c".to_string(), "sleep 100".to_string()],
            &env::current_dir().unwrap(),
            &HashMap::new(),
            Duration::from_millis(100),
            token,
            None,
        )
        .await;

    assert_eq!(result.outcome, ProcessOutcome::TimedOut);
    // Exit code is usually None if terminated by signal
    assert!(result.exit_code.is_none());
}

#[tokio::test]
async fn test_cancellation_and_process_group_kill() {
    let executor = ProcessExecutor::new(1024, Duration::from_secs(1));
    let token = CancellationToken::new();

    let token_clone = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        token_clone.cancel();
    });

    let result = executor
        .execute(
            "sh",
            &["-c".to_string(), "sleep 100".to_string()],
            &env::current_dir().unwrap(),
            &HashMap::new(),
            Duration::from_secs(5),
            token,
            None,
        )
        .await;

    assert_eq!(result.outcome, ProcessOutcome::Cancelled);
    assert!(result.exit_code.is_none());
}

#[tokio::test]
async fn test_output_capture_and_truncation() {
    let executor = ProcessExecutor::new(10, Duration::from_secs(1)); // limit to 10 bytes
    let token = CancellationToken::new();

    let result = executor
        .execute(
            "sh",
            &["-c".to_string(), "echo '123456789012345'".to_string()],
            &env::current_dir().unwrap(),
            &HashMap::new(),
            Duration::from_secs(5),
            token,
            None,
        )
        .await;

    assert_eq!(result.outcome, ProcessOutcome::Succeeded);
    assert_eq!(result.stdout.text, "1234567890"); // EXACTLY 10 bytes
    assert!(result.stdout.truncated);
    assert!(result.stdout.bytes_read > 10);
}

#[tokio::test]
async fn test_working_directory() {
    let executor = ProcessExecutor::new(1024, Duration::from_secs(1));
    let token = CancellationToken::new();

    let cwd = env::current_dir().unwrap().join("src");

    let result = executor
        .execute(
            "pwd",
            &[],
            &cwd,
            &HashMap::new(),
            Duration::from_secs(5),
            token,
            None,
        )
        .await;

    assert_eq!(result.outcome, ProcessOutcome::Succeeded);
    let output_pwd = PathBuf::from(result.stdout.text.trim());
    assert_eq!(
        output_pwd.canonicalize().unwrap(),
        cwd.canonicalize().unwrap()
    );
}

#[tokio::test]
async fn test_stderr_capture() {
    let executor = ProcessExecutor::new(1024, Duration::from_secs(1));
    let token = CancellationToken::new();

    let result = executor
        .execute(
            "sh",
            &["-c".to_string(), "echo 'error message' >&2".to_string()],
            &env::current_dir().unwrap(),
            &HashMap::new(),
            Duration::from_secs(5),
            token,
            None,
        )
        .await;

    assert_eq!(result.outcome, ProcessOutcome::Succeeded);
    assert_eq!(result.stderr.text.trim(), "error message");
}
