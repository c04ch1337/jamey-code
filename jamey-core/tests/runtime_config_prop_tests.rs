mod fixtures;
mod helpers;
mod mocks;
mod utils;

use jamey_core::PoolConfig;
use jamey_runtime::config::RuntimeConfig;
use proptest::prelude::*;
use std::path::PathBuf;

// Strategy for generating valid project names
fn project_name_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z][a-zA-Z0-9_-]{2,63}"
}

// Strategy for generating valid API keys
fn api_key_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9]{32,64}"
}

// Strategy for generating valid file paths
fn path_strategy() -> impl Strategy<Value = PathBuf> {
    "[a-zA-Z0-9/._-]{1,255}".prop_map(PathBuf::from)
}

// Strategy for generating valid memory limits
fn memory_limit_strategy() -> impl Strategy<Value = usize> {
    (100..10000usize)
}

// Strategy for generating valid thread counts
fn thread_count_strategy() -> impl Strategy<Value = usize> {
    (1..32usize)
}

proptest! {
    #[test]
    fn test_runtime_config_validation(
        project_name in project_name_strategy(),
        api_key in api_key_strategy(),
        backup_dir in path_strategy(),
        memory_limit in memory_limit_strategy(),
        thread_count in thread_count_strategy(),
        enable_security in proptest::bool::ANY,
        debug_mode in proptest::bool::ANY,
    ) {
        let mut config = RuntimeConfig::default();
        
        // Set generated values
        config.project_name = project_name.clone();
        // Test secret - do not use in production
        config.llm.openrouter_api_key = api_key.clone();
        config.tools.backup_dir = backup_dir.clone();
        config.memory.max_memories = memory_limit;
        config.runtime.thread_count = thread_count;
        config.security.api_key_required = enable_security;
        config.runtime.debug = debug_mode;

        // Validate configuration
        prop_assert!(!config.project_name.is_empty());
        prop_assert!(config.project_name.len() <= 64);
        prop_assert!(!config.llm.openrouter_api_key.is_empty());
        prop_assert!(config.memory.max_memories > 0);
        prop_assert!(config.runtime.thread_count > 0);
        
        // Test path validation
        prop_assert!(!config.tools.backup_dir.as_os_str().is_empty());
        
        // Test default values
        prop_assert!(config.memory.postgres_port > 0);
        prop_assert!(!config.memory.postgres_host.is_empty());
        prop_assert!(!config.memory.postgres_db.is_empty());
        prop_assert!(!config.memory.postgres_user.is_empty());
    }

    #[test]
    fn test_runtime_config_combinations(
        memory_limit in memory_limit_strategy(),
        thread_count in thread_count_strategy(),
        enable_security in proptest::bool::ANY,
    ) {
        let mut config = RuntimeConfig::default();
        
        config.memory.max_memories = memory_limit;
        config.runtime.thread_count = thread_count;
        config.security.api_key_required = enable_security;

        // Test configuration combinations
        if enable_security {
            prop_assert!(!config.security.allowed_origins.is_empty());
        }

        // Verify memory limit and thread count relationship
        prop_assert!(config.memory.max_memories >= config.runtime.thread_count);
        
        // Test pool configuration derivation
        let pool_config: PoolConfig = (&config).into();
        prop_assert!(pool_config.postgres.max_connections >= thread_count as u32);
        prop_assert!(pool_config.redis.max_connections >= thread_count as u32);
    }
}

#[test]
fn test_runtime_config_edge_cases() {
    // Test empty project name
    let mut config = RuntimeConfig::default();
    config.project_name = "".to_string();
    assert!(!config.validate().is_ok());

    // Test invalid memory limit
    config.project_name = "test".to_string();
    config.memory.max_memories = 0;
    assert!(!config.validate().is_ok());

    // Test invalid thread count
    config.memory.max_memories = 1000;
    config.runtime.thread_count = 0;
    assert!(!config.validate().is_ok());

    // Test invalid backup directory
    config.runtime.thread_count = 4;
    config.tools.backup_dir = PathBuf::from("");
    assert!(!config.validate().is_ok());
}

#[test]
fn test_runtime_config_environment_override() {
    use std::env;

    // Set environment variables
    env::set_var("JAMEY_PROJECT_NAME", "env_test");
    env::set_var("JAMEY_MEMORY_LIMIT", "5000");
    env::set_var("JAMEY_THREAD_COUNT", "8");
    env::set_var("JAMEY_DEBUG", "true");

    // Create config with environment overrides
    let config = RuntimeConfig::from_env().unwrap();

    // Verify environment values were applied
    assert_eq!(config.project_name, "env_test");
    assert_eq!(config.memory.max_memories, 5000);
    assert_eq!(config.runtime.thread_count, 8);
    assert!(config.runtime.debug);

    // Clean up environment
    env::remove_var("JAMEY_PROJECT_NAME");
    env::remove_var("JAMEY_MEMORY_LIMIT");
    env::remove_var("JAMEY_THREAD_COUNT");
    env::remove_var("JAMEY_DEBUG");
}

#[test]
fn test_runtime_config_file_loading() {
    use std::fs;
    use tempfile::NamedTempFile;

    // Create temporary config file
    let config_file = NamedTempFile::new().unwrap();
    let config_content = r#"
        {
            "project_name": "file_test",
            "memory": {
                "max_memories": 3000,
                "postgres_host": "localhost",
                "postgres_port": 5432,
                "postgres_db": "test_db",
                "postgres_user": "test_user",
                "postgres_password": "test_pass"
            },
            "runtime": {
                "thread_count": 6,
                "debug": true
            }
        }
    "#;
    fs::write(&config_file, config_content).unwrap();

    // Load config from file
    let config = RuntimeConfig::from_file(config_file.path()).unwrap();

    // Verify file values were loaded
    assert_eq!(config.project_name, "file_test");
    assert_eq!(config.memory.max_memories, 3000);
    assert_eq!(config.runtime.thread_count, 6);
    assert!(config.runtime.debug);
}