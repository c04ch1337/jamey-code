//! Security tests for critical safety measures
//! 
//! Tests path traversal protection, command whitelisting, SSRF protection,
//! authentication, and process protection.

use jamey_tools::connectors::full_system::FullSystemConnector;
use jamey_tools::connector::{Connector, ExecutionContext};
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_path_traversal_blocked() {
    let temp_dir = TempDir::new().unwrap();
    let connector = FullSystemConnector::new(temp_dir.path().to_path_buf());
    let context = ExecutionContext::default();
    
    // Test 1: Parent directory traversal should be blocked
    let mut params = HashMap::new();
    params.insert("action".to_string(), "read_file".to_string());
    params.insert("path".to_string(), "../etc/passwd".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "Parent directory traversal should be blocked");
    
    // Test 2: Absolute paths should be blocked
    let mut params = HashMap::new();
    params.insert("action".to_string(), "read_file".to_string());
    params.insert("path".to_string(), "/etc/passwd".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "Absolute paths should be blocked");
    
    // Test 3: Multiple parent traversals should be blocked
    let mut params = HashMap::new();
    params.insert("action".to_string(), "read_file".to_string());
    params.insert("path".to_string(), "../../sensitive/data.txt".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "Multiple parent traversals should be blocked");
}

#[tokio::test]
async fn test_safe_path_allowed() {
    let temp_dir = TempDir::new().unwrap();
    let connector = FullSystemConnector::new(temp_dir.path().to_path_buf());
    let context = ExecutionContext::default();
    
    // Create a test file
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, "test content").unwrap();
    
    // Test: Safe relative path should work
    let mut params = HashMap::new();
    params.insert("action".to_string(), "read_file".to_string());
    params.insert("path".to_string(), "test.txt".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_ok(), "Safe relative path should be allowed");
    let result = result.unwrap();
    assert!(result.success, "Read should succeed");
    assert_eq!(result.output, "test content");
}

#[tokio::test]
async fn test_command_whitelist() {
    let temp_dir = TempDir::new().unwrap();
    let connector = FullSystemConnector::new(temp_dir.path().to_path_buf());
    let context = ExecutionContext::default();
    
    // Test 1: Non-whitelisted command should be blocked
    let mut params = HashMap::new();
    params.insert("action".to_string(), "execute_command".to_string());
    params.insert("command".to_string(), "curl".to_string());
    params.insert("args".to_string(), "http://example.com".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "Non-whitelisted command should be blocked");
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Command validation failed") || err_msg.contains("not in the allowed list"),
        "Error should mention command validation: {}", err_msg);
    
    // Test 2: Dangerous command should be blocked
    let mut params = HashMap::new();
    params.insert("action".to_string(), "execute_command".to_string());
    params.insert("command".to_string(), "rm".to_string());
    params.insert("args".to_string(), "-rf /".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "Dangerous command should be blocked");
    
    // Test 3: Whitelisted command validation passes (even if execution might fail due to PATH)
    // We're testing that the whitelist allows it, not that it executes successfully
    let mut params = HashMap::new();
    params.insert("action".to_string(), "execute_command".to_string());
    params.insert("command".to_string(), "git".to_string());
    params.insert("args".to_string(), "--version".to_string());
    
    let result = connector.execute(params, &context).await;
    // The command should pass validation (not be blocked by whitelist)
    // It might fail to execute if git is not in PATH, but that's different from being blocked
    if let Err(e) = &result {
        let err_msg = e.to_string();
        // Should NOT be a whitelist error
        assert!(!err_msg.contains("not in the allowed list"),
            "Git should be whitelisted, got error: {}", err_msg);
    }
}

#[tokio::test]
async fn test_dangerous_flags_blocked() {
    let temp_dir = TempDir::new().unwrap();
    let connector = FullSystemConnector::new(temp_dir.path().to_path_buf());
    let context = ExecutionContext::default();
    
    // Test: Dangerous flags should be blocked
    let mut params = HashMap::new();
    params.insert("action".to_string(), "execute_command".to_string());
    params.insert("command".to_string(), "git".to_string());
    params.insert("args".to_string(), "clone --privileged".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "Dangerous flags should be blocked");
}

#[tokio::test]
async fn test_write_file_path_validation() {
    let temp_dir = TempDir::new().unwrap();
    let connector = FullSystemConnector::new(temp_dir.path().to_path_buf());
    let context = ExecutionContext::default();
    
    // Test: Path traversal in write should be blocked
    let mut params = HashMap::new();
    params.insert("action".to_string(), "write_file".to_string());
    params.insert("path".to_string(), "../../../etc/malicious".to_string());
    params.insert("content".to_string(), "malicious content".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "Path traversal in write should be blocked");
}

#[tokio::test]
async fn test_list_directory_path_validation() {
    let temp_dir = TempDir::new().unwrap();
    let connector = FullSystemConnector::new(temp_dir.path().to_path_buf());
    let context = ExecutionContext::default();
    
    // Test: Path traversal in list should be blocked
    let mut params = HashMap::new();
    params.insert("action".to_string(), "list_directory".to_string());
    params.insert("path".to_string(), "../../".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "Path traversal in list should be blocked");
}

#[tokio::test]
async fn test_ssrf_private_ip_blocked() {
    use jamey_tools::connectors::network_web::NetworkWebConnector;
    use tempfile::TempDir;
    
    let temp_dir = TempDir::new().unwrap();
    let connector = NetworkWebConnector::new(temp_dir.path().to_path_buf(), None).unwrap();
    let context = ExecutionContext::default();
    
    // Test 1: Private IP 10.x should be blocked
    let mut params = HashMap::new();
    params.insert("action".to_string(), "fetch_url".to_string());
    params.insert("url".to_string(), "http://10.0.0.1/admin".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "Private IP 10.x should be blocked");
    
    // Test 2: Private IP 192.168.x should be blocked
    let mut params = HashMap::new();
    params.insert("action".to_string(), "fetch_url".to_string());
    params.insert("url".to_string(), "http://192.168.1.1/".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "Private IP 192.168.x should be blocked");
    
    // Test 3: Private IP 172.16-31.x should be blocked
    let mut params = HashMap::new();
    params.insert("action".to_string(), "fetch_url".to_string());
    params.insert("url".to_string(), "http://172.16.0.1/".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "Private IP 172.16-31.x should be blocked");
}

#[tokio::test]
async fn test_ssrf_localhost_blocked() {
    use jamey_tools::connectors::network_web::NetworkWebConnector;
    use tempfile::TempDir;
    
    let temp_dir = TempDir::new().unwrap();
    let connector = NetworkWebConnector::new(temp_dir.path().to_path_buf(), None).unwrap();
    let context = ExecutionContext::default();
    
    // Test 1: localhost should be blocked
    let mut params = HashMap::new();
    params.insert("action".to_string(), "fetch_url".to_string());
    params.insert("url".to_string(), "http://localhost:8080/".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "localhost should be blocked");
    
    // Test 2: 127.0.0.1 should be blocked
    let mut params = HashMap::new();
    params.insert("action".to_string(), "fetch_url".to_string());
    params.insert("url".to_string(), "http://127.0.0.1/".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "127.0.0.1 should be blocked");
}

#[tokio::test]
async fn test_ssrf_metadata_endpoints_blocked() {
    use jamey_tools::connectors::network_web::NetworkWebConnector;
    use tempfile::TempDir;
    
    let temp_dir = TempDir::new().unwrap();
    let connector = NetworkWebConnector::new(temp_dir.path().to_path_buf(), None).unwrap();
    let context = ExecutionContext::default();
    
    // Test 1: AWS metadata endpoint should be blocked
    let mut params = HashMap::new();
    params.insert("action".to_string(), "fetch_url".to_string());
    params.insert("url".to_string(), "http://169.254.169.254/latest/meta-data/".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "AWS metadata endpoint should be blocked");
    
    // Test 2: Google metadata endpoint should be blocked
    let mut params = HashMap::new();
    params.insert("action".to_string(), "fetch_url".to_string());
    params.insert("url".to_string(), "http://metadata.google.internal/".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "Google metadata endpoint should be blocked");
}

#[tokio::test]
async fn test_ssrf_invalid_schemes_blocked() {
    use jamey_tools::connectors::network_web::NetworkWebConnector;
    use tempfile::TempDir;
    
    let temp_dir = TempDir::new().unwrap();
    let connector = NetworkWebConnector::new(temp_dir.path().to_path_buf(), None).unwrap();
    let context = ExecutionContext::default();
    
    // Test 1: file:// scheme should be blocked
    let mut params = HashMap::new();
    params.insert("action".to_string(), "fetch_url".to_string());
    params.insert("url".to_string(), "file:///etc/passwd".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "file:// scheme should be blocked");
    
    // Test 2: ftp:// scheme should be blocked
    let mut params = HashMap::new();
    params.insert("action".to_string(), "fetch_url".to_string());
    params.insert("url".to_string(), "ftp://example.com/".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "ftp:// scheme should be blocked");
}

#[tokio::test]
async fn test_download_url_validation() {
    use jamey_tools::connectors::network_web::NetworkWebConnector;
    use tempfile::TempDir;
    
    let temp_dir = TempDir::new().unwrap();
    let connector = NetworkWebConnector::new(temp_dir.path().to_path_buf(), None).unwrap();
    let context = ExecutionContext::default();
    
    // Test: Private IP in download should be blocked
    let mut params = HashMap::new();
    params.insert("action".to_string(), "download".to_string());
    params.insert("url".to_string(), "http://192.168.1.1/malware.exe".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "Private IP in download should be blocked");
}

#[tokio::test]
async fn test_agent_api_key_required() {
    use jamey_tools::connectors::agent_orchestration::AgentOrchestrationConnector;
    
    let connector = AgentOrchestrationConnector::new().unwrap();
    let context = ExecutionContext::default();
    
    // Test: Missing API key should fail
    let mut params = HashMap::new();
    params.insert("action".to_string(), "register_agent".to_string());
    params.insert("agent_id".to_string(), "test-agent".to_string());
    params.insert("name".to_string(), "Test Agent".to_string());
    params.insert("url".to_string(), "https://example.com".to_string());
    // No api_key provided
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_err(), "Missing API key should fail");
}

#[tokio::test]
async fn test_agent_https_required() {
    use jamey_tools::connectors::agent_orchestration::{AgentOrchestrationConnector, AgentEndpoint};
    
    let connector = AgentOrchestrationConnector::new().unwrap();
    
    // Test: HTTP URL should be rejected
    let agent = AgentEndpoint {
        id: "test-agent".to_string(),
        name: "Test Agent".to_string(),
        url: "http://example.com".to_string(),  // HTTP not HTTPS
        api_key: "test-key".to_string(),
        capabilities: vec![],
    };
    
    let result = connector.register_agent(agent).await;
    assert!(result.is_err(), "HTTP URLs should be rejected");
}

#[tokio::test]
async fn test_agent_empty_api_key_rejected() {
    use jamey_tools::connectors::agent_orchestration::{AgentOrchestrationConnector, AgentEndpoint};
    
    let connector = AgentOrchestrationConnector::new().unwrap();
    
    // Test: Empty API key should be rejected
    let agent = AgentEndpoint {
        id: "test-agent".to_string(),
        name: "Test Agent".to_string(),
        url: "https://example.com".to_string(),
        api_key: "   ".to_string(),  // Empty/whitespace only
        capabilities: vec![],
    };
    
    let result = connector.register_agent(agent).await;
    assert!(result.is_err(), "Empty API key should be rejected");
}

#[tokio::test]
async fn test_agent_valid_registration() {
    use jamey_tools::connectors::agent_orchestration::{AgentOrchestrationConnector, AgentEndpoint};
    
    let connector = AgentOrchestrationConnector::new().unwrap();
    
    // Test: Valid agent should register successfully
    let agent = AgentEndpoint {
        id: "test-agent".to_string(),
        name: "Test Agent".to_string(),
        url: "https://example.com".to_string(),
        api_key: "valid-api-key-123".to_string(),
        capabilities: vec!["task1".to_string()],
    };
    
    let result = connector.register_agent(agent).await;
    assert!(result.is_ok(), "Valid agent should register successfully");
}

#[tokio::test]
async fn test_protected_process_cannot_be_killed() {
    use jamey_tools::connectors::system_admin::SystemAdminConnector;
    
    let connector = SystemAdminConnector::new();
    let context = ExecutionContext::default();
    
    // Note: This test assumes there's a system process running
    // In a real scenario, we'd need to find a protected process PID
    // For now, we'll test the logic by checking if the error message is correct
    
    // Test: Attempting to kill a protected process should fail
    // We'll use PID 4 which is typically the System process on Windows
    let mut params = HashMap::new();
    params.insert("action".to_string(), "kill_process".to_string());
    params.insert("pid".to_string(), "4".to_string());
    params.insert("confirmed".to_string(), "true".to_string());
    
    let result = connector.execute(params, &context).await;
    // This should either fail because it's protected, or because we don't have permission
    // Either way, it shouldn't succeed
    if let Ok(res) = result {
        assert!(!res.success || !res.errors.is_empty(), 
            "Protected process termination should not succeed");
    }
}

#[tokio::test]
async fn test_process_kill_requires_confirmation() {
    use jamey_tools::connectors::system_admin::SystemAdminConnector;
    
    let connector = SystemAdminConnector::new();
    let context = ExecutionContext::default();
    
    // Test: Kill without confirmation should fail
    let mut params = HashMap::new();
    params.insert("action".to_string(), "kill_process".to_string());
    params.insert("pid".to_string(), "12345".to_string());
    // No "confirmed" parameter
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_ok(), "Should return Ok with error in result");
    let result = result.unwrap();
    assert!(!result.success || !result.errors.is_empty(), 
        "Kill without confirmation should fail");
}

#[tokio::test]
async fn test_list_processes_works() {
    use jamey_tools::connectors::system_admin::SystemAdminConnector;
    
    let connector = SystemAdminConnector::new();
    let context = ExecutionContext::default();
    
    // Test: List processes should work
    let mut params = HashMap::new();
    params.insert("action".to_string(), "list_processes".to_string());
    
    let result = connector.execute(params, &context).await;
    assert!(result.is_ok(), "List processes should succeed");
    let result = result.unwrap();
    assert!(result.success, "List processes should be successful");
    assert!(!result.output.is_empty(), "Should return process list");
}