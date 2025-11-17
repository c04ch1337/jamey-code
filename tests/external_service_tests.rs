mod fixtures;
mod helpers;
mod mocks;
mod utils;

use anyhow::Result;
use jamey_providers::openrouter::{
    OpenRouterProvider,
    ChatRequest,
    Message,
    OpenRouterError,
    Role,
};
use jamey_runtime::{Runtime, RuntimeConfig};
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;
use utils::retry_with_backoff;

async fn setup_test_provider() -> OpenRouterProvider {
    OpenRouterProvider::new("test_key".to_string())
}

#[tokio::test]
async fn test_openrouter_basic_chat() -> Result<()> {
    let provider = setup_test_provider().await;
    
    let request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "Hello, this is a test message.".to_string(),
        }],
        tools: None,
        tool_choice: None,
        temperature: Some(0.0),
        max_tokens: None,
    };

    let response = provider.chat(request).await?;
    
    assert!(!response.choices.is_empty());
    assert!(!response.choices[0].message.content.is_empty());
    assert_eq!(response.choices[0].message.role, Role::Assistant);

    Ok(())
}

#[tokio::test]
async fn test_openrouter_error_handling() -> Result<()> {
    // Test with invalid API key
    let invalid_provider = OpenRouterProvider::new("invalid_key".to_string());
    
    let request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "Test message".to_string(),
        }],
        tools: None,
        tool_choice: None,
        temperature: Some(0.0),
        max_tokens: None,
    };

    let result = invalid_provider.chat(request).await;
    assert!(matches!(result, Err(OpenRouterError::AuthenticationError)));

    Ok(())
}

#[tokio::test]
async fn test_openrouter_rate_limiting() -> Result<()> {
    let provider = setup_test_provider().await;
    let mut handles = Vec::new();

    // Spawn multiple concurrent requests
    for i in 0..5 {
        let provider = provider.clone();
        handles.push(tokio::spawn(async move {
            let request = ChatRequest {
                model: "claude-3-sonnet".to_string(),
                messages: vec![Message {
                    role: Role::User,
                    content: format!("Concurrent test message {}", i),
                }],
                tools: None,
                tool_choice: None,
                temperature: Some(0.0),
                max_tokens: None,
            };

            // Use retry with backoff for rate-limited requests
            retry_with_backoff(
                || provider.chat(request.clone()),
                3,
                Duration::from_secs(1),
            ).await
        }));
    }

    // Wait for all requests to complete
    for handle in handles {
        let result = handle.await?;
        assert!(result.is_ok());
    }

    Ok(())
}

#[tokio::test]
async fn test_openrouter_conversation_context() -> Result<()> {
    let provider = setup_test_provider().await;
    
    // Multi-turn conversation
    let messages = vec![
        Message {
            role: Role::System,
            content: "You are a helpful assistant.".to_string(),
        },
        Message {
            role: Role::User,
            content: "What is your role?".to_string(),
        },
    ];

    let request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: messages.clone(),
        tools: None,
        tool_choice: None,
        temperature: Some(0.0),
        max_tokens: None,
    };

    let response = provider.chat(request).await?;
    assert!(!response.choices.is_empty());
    
    // Follow-up message
    let mut conversation = messages;
    conversation.push(response.choices[0].message.clone());
    conversation.push(Message {
        role: Role::User,
        content: "Can you elaborate on that?".to_string(),
    });

    let follow_up_request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: conversation,
        tools: None,
        tool_choice: None,
        temperature: Some(0.0),
        max_tokens: None,
    };

    let follow_up_response = provider.chat(follow_up_request).await?;
    assert!(!follow_up_response.choices.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_openrouter_tool_usage() -> Result<()> {
    let provider = setup_test_provider().await;
    
    let tools = vec![
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get the current weather",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "location": {
                            "type": "string",
                            "description": "The city and state"
                        }
                    },
                    "required": ["location"]
                }
            }
        })
    ];

    let request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "What's the weather in San Francisco?".to_string(),
        }],
        tools: Some(tools),
        tool_choice: Some("auto".to_string()),
        temperature: Some(0.0),
        max_tokens: None,
    };

    let response = provider.chat(request).await?;
    assert!(!response.choices.is_empty());
    assert!(response.choices[0].message.tool_calls.is_some());

    Ok(())
}

#[tokio::test]
async fn test_openrouter_streaming() -> Result<()> {
    let provider = setup_test_provider().await;
    
    let request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "Count from 1 to 5 slowly.".to_string(),
        }],
        tools: None,
        tool_choice: None,
        temperature: Some(0.0),
        max_tokens: None,
    };

    let mut stream = provider.chat_stream(request).await?;
    let mut count = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        assert!(!chunk.choices.is_empty());
        count += 1;
    }

    assert!(count > 1); // Should receive multiple chunks

    Ok(())
}

#[tokio::test]
async fn test_openrouter_integration_with_runtime() -> Result<()> {
    let mut config = RuntimeConfig::default();
    // Test secret - do not use in production
    config.llm.openrouter_api_key = "test_key".to_string();
    
    let runtime = Runtime::new(config).await?;
    let state = runtime.state();
    
    let request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "Integration test message.".to_string(),
        }],
        tools: None,
        tool_choice: None,
        temperature: Some(0.0),
        max_tokens: None,
    };

    let response = state.llm_provider.chat(request).await?;
    assert!(!response.choices.is_empty());

    Ok(())
}