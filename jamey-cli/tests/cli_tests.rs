mod helpers {
    use std::process::Command;
    use std::time::Duration;
    use tokio::time::sleep;

    pub async fn run_cli_command(args: &[&str]) -> std::io::Result<std::process::Output> {
        Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("jamey-cli")
            .args(args)
            .output()
    }

    pub async fn wait_for_runtime_ready() {
        sleep(Duration::from_secs(2)).await;
    }
}

use anyhow::Result;
use assert_cmd::Command;
use predicates::prelude::*;
use std::{fs, path::PathBuf, time::Duration};
use tempfile::TempDir;
use tokio;

#[tokio::test]
async fn test_cli_init_command() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("config.json");

    let mut cmd = Command::cargo_bin("jamey-cli")?;
    cmd.arg("init")
        .arg("--config")
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialization complete"));

    // Verify config file was created
    assert!(config_path.exists());
    let config_content = fs::read_to_string(&config_path)?;
    assert!(config_content.contains("project_name"));

    Ok(())
}

#[tokio::test]
async fn test_cli_start_stop_commands() -> Result<()> {
    // Start runtime
    let mut start_cmd = Command::cargo_bin("jamey-cli")?;
    start_cmd
        .arg("start")
        .assert()
        .success()
        .stdout(predicate::str::contains("Runtime started"));

    helpers::wait_for_runtime_ready().await;

    // Check status
    let mut status_cmd = Command::cargo_bin("jamey-cli")?;
    status_cmd
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("Runtime is running"));

    // Stop runtime
    let mut stop_cmd = Command::cargo_bin("jamey-cli")?;
    stop_cmd
        .arg("stop")
        .assert()
        .success()
        .stdout(predicate::str::contains("Runtime stopped"));

    Ok(())
}

#[tokio::test]
async fn test_cli_chat_command() -> Result<()> {
    // Start runtime first
    helpers::run_cli_command(&["start"]).await?;
    helpers::wait_for_runtime_ready().await;

    // Test chat command
    let mut cmd = Command::cargo_bin("jamey-cli")?;
    cmd.arg("chat")
        .arg("--message")
        .arg("Hello, this is a test message")
        .assert()
        .success()
        .stdout(predicate::str::contains("Assistant:"));

    // Stop runtime
    helpers::run_cli_command(&["stop"]).await?;

    Ok(())
}

#[tokio::test]
async fn test_cli_memory_commands() -> Result<()> {
    // Start runtime
    helpers::run_cli_command(&["start"]).await?;
    helpers::wait_for_runtime_ready().await;

    // Store memory
    let mut store_cmd = Command::cargo_bin("jamey-cli")?;
    store_cmd
        .arg("memory")
        .arg("store")
        .arg("--content")
        .arg("Test memory content")
        .assert()
        .success()
        .stdout(predicate::str::contains("Memory stored"));

    // List memories
    let mut list_cmd = Command::cargo_bin("jamey-cli")?;
    list_cmd
        .arg("memory")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("Test memory content"));

    // Search memories
    let mut search_cmd = Command::cargo_bin("jamey-cli")?;
    search_cmd
        .arg("memory")
        .arg("search")
        .arg("--query")
        .arg("test")
        .assert()
        .success();

    // Stop runtime
    helpers::run_cli_command(&["stop"]).await?;

    Ok(())
}

#[tokio::test]
async fn test_cli_process_commands() -> Result<()> {
    // Start runtime
    helpers::run_cli_command(&["start"]).await?;
    helpers::wait_for_runtime_ready().await;

    // List processes
    let mut list_cmd = Command::cargo_bin("jamey-cli")?;
    list_cmd
        .arg("process")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("PID"));

    // Get specific process info
    let mut info_cmd = Command::cargo_bin("jamey-cli")?;
    info_cmd
        .arg("process")
        .arg("info")
        .arg("--pid")
        .arg("1")
        .assert()
        .success();

    // Stop runtime
    helpers::run_cli_command(&["stop"]).await?;

    Ok(())
}

#[tokio::test]
async fn test_cli_system_commands() -> Result<()> {
    // Start runtime
    helpers::run_cli_command(&["start"]).await?;
    helpers::wait_for_runtime_ready().await;

    // Get system info
    let mut info_cmd = Command::cargo_bin("jamey-cli")?;
    info_cmd
        .arg("system")
        .arg("info")
        .assert()
        .success()
        .stdout(predicate::str::contains("CPU"))
        .stdout(predicate::str::contains("Memory"));

    // Monitor system (brief test)
    let mut monitor_cmd = Command::cargo_bin("jamey-cli")?;
    monitor_cmd
        .arg("system")
        .arg("monitor")
        .arg("--duration")
        .arg("1")
        .assert()
        .success();

    // Stop runtime
    helpers::run_cli_command(&["stop"]).await?;

    Ok(())
}

#[tokio::test]
async fn test_cli_error_handling() -> Result<()> {
    // Test invalid command
    let mut invalid_cmd = Command::cargo_bin("jamey-cli")?;
    invalid_cmd
        .arg("invalid")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error"));

    // Test missing arguments
    let mut missing_args_cmd = Command::cargo_bin("jamey-cli")?;
    missing_args_cmd
        .arg("chat")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));

    // Test commands without runtime
    let mut no_runtime_cmd = Command::cargo_bin("jamey-cli")?;
    no_runtime_cmd
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("not running"));

    Ok(())
}

#[tokio::test]
async fn test_cli_config_handling() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("test_config.json");

    // Create config
    let mut init_cmd = Command::cargo_bin("jamey-cli")?;
    init_cmd
        .arg("init")
        .arg("--config")
        .arg(&config_path)
        .assert()
        .success();

    // Start with custom config
    let mut start_cmd = Command::cargo_bin("jamey-cli")?;
    start_cmd
        .arg("start")
        .arg("--config")
        .arg(&config_path)
        .assert()
        .success();

    helpers::wait_for_runtime_ready().await;

    // Stop runtime
    helpers::run_cli_command(&["stop"]).await?;

    Ok(())
}