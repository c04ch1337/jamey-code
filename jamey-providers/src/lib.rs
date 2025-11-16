//! LLM Provider implementations for Digital Twin Jamey
//! 
//! This crate provides implementations for various LLM providers,
//! starting with OpenRouter support for accessing multiple LLM models.

pub mod openrouter;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("Provider error: {0}")]
    Provider(String),
    #[error(transparent)]
    OpenRouter(#[from] openrouter::OpenRouterError),
}

/// Common traits and types used across providers
pub mod prelude {
    pub use super::openrouter::{
        ChatRequest, ChatResponse, LlmProvider, Message, OpenRouterConfig, OpenRouterProvider, Tool,
        ToolCall,
    };
    pub use super::ProviderError;
}

/// Re-export main provider implementations
pub use openrouter::{OpenRouterConfig, OpenRouterProvider};

/// Generic response type for tool calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    pub tool_name: String,
    pub success: bool,
    pub result: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Trait for handling tool execution results
#[async_trait]
pub trait ToolHandler {
    async fn handle_tool_call(&self, tool_call: &openrouter::ToolCall) -> Result<ToolResponse, ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use openrouter::{ChatRequest, Message, OpenRouterConfig};
    use url::Url;
    use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_provider_integration() {
        let mock_server = MockServer::start().await;

        let config = OpenRouterConfig {
            api_key: "test_key".to_string(),
            api_base_url: Url::parse(&mock_server.uri()).unwrap(),
            ..Default::default()
        };

        let provider = OpenRouterProvider::new(config).unwrap();

        // Mock successful chat response
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "test_response",
                "model": "claude-3-sonnet",
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "Test response"
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 5,
                    "total_tokens": 15
                }
            })))
            .mount(&mock_server)
            .await;

        // Test basic chat functionality
        let request = ChatRequest {
            model: "claude-3-sonnet".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "Test message".to_string(),
            }],
            tools: None,
            tool_choice: None,
            temperature: None,
            max_tokens: None,
        };

        let response = provider.chat(request).await.unwrap();
        assert_eq!(response.choices[0].message.content, "Test response");

        // Test embedding functionality
        let embedding = provider.get_embedding("Test text").await.unwrap();
        assert_eq!(embedding.len(), 1536); // Expected embedding dimension
    }
}