//! Comprehensive unit tests for OpenRouter provider
//! Tests API errors, rate limiting, validation, and edge cases

use jamey_providers::{
    ChatRequest, ChatResponse, LlmProvider, Message, OpenRouterConfig, 
    OpenRouterError, OpenRouterProvider, Tool
};
use serde_json::json;
use url::Url;
use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

// ============================================================================
// Configuration Validation Tests
// ============================================================================

#[test]
fn test_config_default() {
    let config = OpenRouterConfig::default();
    
    assert!(!config.api_key.is_empty() || config.api_key.is_empty()); // Can be empty in default
    assert_eq!(config.api_base_url.scheme(), "https");
    assert_eq!(config.default_model, "claude-3-sonnet");
    assert!(!config.allowed_models.is_empty());
    assert_eq!(config.timeout_seconds, 30);
    assert_eq!(config.max_retries, 3);
}

#[test]
fn test_provider_creation_empty_api_key() {
    let config = OpenRouterConfig {
        api_key: "".to_string(),
        ..Default::default()
    };

    let result = OpenRouterProvider::new(config);
    assert!(result.is_err());
}

#[test]
fn test_provider_creation_empty_allowed_models() {
    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        allowed_models: vec![],
        ..Default::default()
    };

    let result = OpenRouterProvider::new(config);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_provider_creation_valid() {
    let config = OpenRouterConfig {
        api_key: "test_key_123".to_string(),
        ..Default::default()
    };

    let result = OpenRouterProvider::new(config);
    assert!(result.is_ok());
}

// ============================================================================
// Message Validation Tests
// ============================================================================

#[tokio::test]
async fn test_chat_empty_messages() {
    let mock_server = MockServer::start().await;
    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    let request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![],
        tools: None,
        tool_choice: None,
        temperature: None,
        max_tokens: None,
    };

    let result = provider.chat(request).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("At least one message"));
}

#[tokio::test]
async fn test_chat_empty_message_content() {
    let mock_server = MockServer::start().await;
    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    let request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "".to_string(),
        }],
        tools: None,
        tool_choice: None,
        temperature: None,
        max_tokens: None,
    };

    let result = provider.chat(request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_chat_whitespace_only_content() {
    let mock_server = MockServer::start().await;
    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    let request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "   \n\t  ".to_string(),
        }],
        tools: None,
        tool_choice: None,
        temperature: None,
        max_tokens: None,
    };

    let result = provider.chat(request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_chat_invalid_role() {
    let mock_server = MockServer::start().await;
    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    let invalid_roles = vec!["invalid", "admin", "moderator", ""];

    for role in invalid_roles {
        let request = ChatRequest {
            model: "claude-3-sonnet".to_string(),
            messages: vec![Message {
                role: role.to_string(),
                content: "Test message".to_string(),
            }],
            tools: None,
            tool_choice: None,
            temperature: None,
            max_tokens: None,
        };

        let result = provider.chat(request).await;
        assert!(result.is_err(), "Should reject invalid role: {}", role);
    }
}

#[tokio::test]
async fn test_chat_valid_roles() {
    let mock_server = MockServer::start().await;
    
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "test",
            "model": "claude-3-sonnet",
            "choices": [{
                "message": {"role": "assistant", "content": "Response"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        })))
        .mount(&mock_server)
        .await;

    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    let valid_roles = vec!["system", "user", "assistant", "function"];

    for role in valid_roles {
        let request = ChatRequest {
            model: "claude-3-sonnet".to_string(),
            messages: vec![Message {
                role: role.to_string(),
                content: "Test message".to_string(),
            }],
            tools: None,
            tool_choice: None,
            temperature: None,
            max_tokens: None,
        };

        let result = provider.chat(request).await;
        assert!(result.is_ok(), "Should accept valid role: {}", role);
    }
}

// ============================================================================
// Model Validation Tests
// ============================================================================

#[tokio::test]
async fn test_chat_invalid_model() {
    let mock_server = MockServer::start().await;
    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        allowed_models: vec!["claude-3-sonnet".to_string()],
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    let request = ChatRequest {
        model: "invalid-model".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Test".to_string(),
        }],
        tools: None,
        tool_choice: None,
        temperature: None,
        max_tokens: None,
    };

    let result = provider.chat(request).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid model"));
}

#[tokio::test]
async fn test_chat_empty_model_uses_default() {
    let mock_server = MockServer::start().await;
    
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "test",
            "model": "claude-3-sonnet",
            "choices": [{
                "message": {"role": "assistant", "content": "Response"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        })))
        .mount(&mock_server)
        .await;

    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        default_model: "claude-3-sonnet".to_string(),
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    let request = ChatRequest {
        model: "".to_string(), // Empty model
        messages: vec![Message {
            role: "user".to_string(),
            content: "Test".to_string(),
        }],
        tools: None,
        tool_choice: None,
        temperature: None,
        max_tokens: None,
    };

    let result = provider.chat(request).await;
    assert!(result.is_ok());
}

// ============================================================================
// Tool Validation Tests
// ============================================================================

#[tokio::test]
async fn test_chat_empty_tool_name() {
    let mock_server = MockServer::start().await;
    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    let request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Test".to_string(),
        }],
        tools: Some(vec![Tool {
            name: "".to_string(),
            description: "Test tool".to_string(),
            parameters: json!({}),
        }]),
        tool_choice: None,
        temperature: None,
        max_tokens: None,
    };

    let result = provider.chat(request).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Tool name"));
}

#[tokio::test]
async fn test_chat_empty_tool_description() {
    let mock_server = MockServer::start().await;
    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    let request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Test".to_string(),
        }],
        tools: Some(vec![Tool {
            name: "test_tool".to_string(),
            description: "".to_string(),
            parameters: json!({}),
        }]),
        tool_choice: None,
        temperature: None,
        max_tokens: None,
    };

    let result = provider.chat(request).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Tool description"));
}

// ============================================================================
// Parameter Validation Tests
// ============================================================================

#[tokio::test]
async fn test_chat_invalid_temperature() {
    let mock_server = MockServer::start().await;
    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    let invalid_temps = vec![-0.1, 2.1, 3.0, -1.0];

    for temp in invalid_temps {
        let request = ChatRequest {
            model: "claude-3-sonnet".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "Test".to_string(),
            }],
            tools: None,
            tool_choice: None,
            temperature: Some(temp),
            max_tokens: None,
        };

        let result = provider.chat(request).await;
        assert!(result.is_err(), "Should reject temperature: {}", temp);
    }
}

#[tokio::test]
async fn test_chat_zero_max_tokens() {
    let mock_server = MockServer::start().await;
    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    let request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Test".to_string(),
        }],
        tools: None,
        tool_choice: None,
        temperature: None,
        max_tokens: Some(0),
    };

    let result = provider.chat(request).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("max_tokens"));
}

// ============================================================================
// Token Limit Tests
// ============================================================================

#[tokio::test]
async fn test_chat_exceeds_token_limit() {
    let mock_server = MockServer::start().await;
    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    // Create a very long message that exceeds token limits
    let long_content = "word ".repeat(10000); // Approximately 10k tokens

    let request = ChatRequest {
        model: "gpt-3.5-turbo".to_string(), // Has 4096 token limit
        messages: vec![Message {
            role: "user".to_string(),
            content: long_content,
        }],
        tools: None,
        tool_choice: None,
        temperature: None,
        max_tokens: None,
    };

    let result = provider.chat(request).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Token"));
}

// ============================================================================
// Rate Limiting Tests
// ============================================================================

#[tokio::test]
async fn test_rate_limit_response() {
    let mock_server = MockServer::start().await;
    
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(429).set_body_json(json!({
            "error": "Rate limit exceeded"
        })))
        .mount(&mock_server)
        .await;

    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        max_retries: 1, // Limit retries for faster test
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    let request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Test".to_string(),
        }],
        tools: None,
        tool_choice: None,
        temperature: None,
        max_tokens: None,
    };

    let result = provider.chat(request).await;
    assert!(result.is_err());
}

// ============================================================================
// Error Response Tests
// ============================================================================

#[tokio::test]
async fn test_api_error_response() {
    let mock_server = MockServer::start().await;
    
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "Invalid request"
        })))
        .mount(&mock_server)
        .await;

    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    let request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Test".to_string(),
        }],
        tools: None,
        tool_choice: None,
        temperature: None,
        max_tokens: None,
    };

    let result = provider.chat(request).await;
    assert!(result.is_err());
}

// ============================================================================
// Embedding Tests
// ============================================================================

#[tokio::test]
async fn test_embedding_empty_text() {
    let mock_server = MockServer::start().await;
    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    let result = provider.get_embedding("").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_embedding_whitespace_only() {
    let mock_server = MockServer::start().await;
    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    let result = provider.get_embedding("   \n\t  ").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_embedding_exceeds_token_limit() {
    let mock_server = MockServer::start().await;
    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    // Create text that exceeds 8192 token limit
    let long_text = "word ".repeat(10000);

    let result = provider.get_embedding(&long_text).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Token"));
}

#[tokio::test]
async fn test_embedding_success() {
    let mock_server = MockServer::start().await;
    
    Mock::given(method("POST"))
        .and(path("/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{
                "object": "embedding",
                "embedding": vec![0.1; 1536],
                "index": 0
            }],
            "model": "text-embedding-ada-002",
            "usage": {"prompt_tokens": 5, "total_tokens": 5}
        })))
        .mount(&mock_server)
        .await;

    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    let result = provider.get_embedding("Test text").await;
    assert!(result.is_ok());
    
    let embedding = result.unwrap();
    assert_eq!(embedding.len(), 1536);
}

// ============================================================================
// Timeout Tests
// ============================================================================

#[tokio::test]
async fn test_request_timeout() {
    let mock_server = MockServer::start().await;
    
    // Mock server that delays response beyond timeout
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_secs(35))
                .set_body_json(json!({
                    "id": "test",
                    "model": "claude-3-sonnet",
                    "choices": [{
                        "message": {"role": "assistant", "content": "Response"},
                        "finish_reason": "stop"
                    }],
                    "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
                }))
        )
        .mount(&mock_server)
        .await;

    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        timeout_seconds: 1, // Very short timeout
        ..Default::default()
    };

    let provider = OpenRouterProvider::new(config).unwrap();

    let request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Test".to_string(),
        }],
        tools: None,
        tool_choice: None,
        temperature: None,
        max_tokens: None,
    };

    let result = provider.chat(request).await;
    assert!(result.is_err());
}

// ============================================================================
// Concurrency Tests
// ============================================================================

#[tokio::test]
async fn test_concurrent_requests() {
    let mock_server = MockServer::start().await;
    
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "test",
            "model": "claude-3-sonnet",
            "choices": [{
                "message": {"role": "assistant", "content": "Response"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        })))
        .mount(&mock_server)
        .await;

    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: Url::parse(&mock_server.uri()).unwrap(),
        ..Default::default()
    };

    let provider = std::sync::Arc::new(OpenRouterProvider::new(config).unwrap());

    let mut handles = vec![];
    
    for i in 0..10 {
        let provider_clone = provider.clone();
        let handle = tokio::spawn(async move {
            let request = ChatRequest {
                model: "claude-3-sonnet".to_string(),
                messages: vec![Message {
                    role: "user".to_string(),
                    content: format!("Test {}", i),
                }],
                tools: None,
                tool_choice: None,
                temperature: None,
                max_tokens: None,
            };
            provider_clone.chat(request).await
        });
        handles.push(handle);
    }

    let results: Vec<_> = futures::future::join_all(handles).await;
    
    // All requests should succeed
    for result in results {
        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());
    }
}