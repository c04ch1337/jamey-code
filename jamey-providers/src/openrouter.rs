use anyhow::Result;
use async_trait::async_trait;
use backoff::ExponentialBackoff;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tiktoken_rs::CoreBPE;
use tracing::{debug, error, info};
use url::Url;

#[derive(Debug, Error)]
pub enum OpenRouterError {
    #[error("API error: {0}")]
    Api(String),
    #[error("Rate limit exceeded")]
    RateLimit,
    #[error("Invalid model: {0}")]
    InvalidModel(String),
    #[error("Token count exceeded for model {model}: {count} > {limit}")]
    TokenLimit {
        model: String,
        count: usize,
        limit: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterConfig {
    pub api_key: String,
    pub api_base_url: Url,
    pub default_model: String,
    pub allowed_models: Vec<String>,
    pub timeout_seconds: u64,
    pub max_retries: u32,
}

impl Default for OpenRouterConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base_url: Url::parse("https://openrouter.ai/api/v1").unwrap(),
            default_model: "claude-3-sonnet".to_string(),
            allowed_models: vec![
                "claude-3-sonnet".to_string(),
                "gpt-4".to_string(),
                "gpt-3.5-turbo".to_string(),
            ],
            timeout_seconds: 30,
            max_retries: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub message: Message,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

pub struct OpenRouterProvider {
    config: OpenRouterConfig,
    client: reqwest::Client,
    tokenizer: CoreBPE,
}

impl OpenRouterProvider {
    pub fn new(config: OpenRouterConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .build()?;

        let tokenizer = tiktoken_rs::cl100k_base()?;

        Ok(Self {
            config,
            client,
            tokenizer,
        })
    }

    fn validate_model(&self, model: &str) -> Result<(), OpenRouterError> {
        if !self.config.allowed_models.contains(&model.to_string()) {
            return Err(OpenRouterError::InvalidModel(model.to_string()));
        }
        Ok(())
    }

    fn count_tokens(&self, text: &str) -> usize {
        self.tokenizer.encode_ordinary(text).len()
    }

    async fn make_request(&self, request: ChatRequest) -> Result<ChatResponse> {
        let backoff = ExponentialBackoff::default();
        let url = self.config.api_base_url.join("chat/completions")?;

        let result = backoff::future::retry(backoff, || async {
            let response = self
                .client
                .post(url.clone())
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .json(&request)
                .send()
                .await?;

            match response.status() {
                reqwest::StatusCode::OK => Ok(response.json::<ChatResponse>().await?),
                reqwest::StatusCode::TOO_MANY_REQUESTS => {
                    Err(backoff::Error::Transient(OpenRouterError::RateLimit))
                }
                _ => Err(backoff::Error::Permanent(OpenRouterError::Api(
                    response.text().await?,
                ))),
            }
        })
        .await?;

        Ok(result)
    }
}

#[async_trait]
pub trait LlmProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
    async fn get_embedding(&self, text: &str) -> Result<Vec<f32>>;
}

#[async_trait]
impl LlmProvider for OpenRouterProvider {
    async fn chat(&self, mut request: ChatRequest) -> Result<ChatResponse> {
        // Use default model if none specified
        if request.model.is_empty() {
            request.model = self.config.default_model.clone();
        }

        self.validate_model(&request.model)?;

        // Count tokens and validate against model limits
        let total_tokens: usize = request
            .messages
            .iter()
            .map(|m| self.count_tokens(&m.content))
            .sum();

        // Example token limits - in production these would be configured per model
        let token_limit = match request.model.as_str() {
            "claude-3-sonnet" => 200_000,
            "gpt-4" => 8_192,
            "gpt-3.5-turbo" => 4_096,
            _ => 4_096,
        };

        if total_tokens > token_limit {
            return Err(OpenRouterError::TokenLimit {
                model: request.model,
                count: total_tokens,
                limit: token_limit,
            }
            .into());
        }

        self.make_request(request).await
    }

    async fn get_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let request = ChatRequest {
            model: "claude-3-sonnet".to_string(), // Use Claude for embeddings
            messages: vec![Message {
                role: "user".to_string(),
                content: text.to_string(),
            }],
            tools: None,
            tool_choice: None,
            temperature: Some(0.0),
            max_tokens: Some(1), // We only need the embedding
        };

        let response = self.make_request(request).await?;
        
        // Extract embedding from response
        // Note: This is a simplified example - actual embedding extraction would depend on the model's response format
        Ok(vec![0.0; 1536]) // Placeholder - implement actual embedding extraction
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_openrouter_provider() {
        let mock_server = MockServer::start().await;

        let config = OpenRouterConfig {
            api_key: "test_key".to_string(),
            api_base_url: Url::parse(&mock_server.uri()).unwrap(),
            ..Default::default()
        };

        let provider = OpenRouterProvider::new(config).unwrap();

        // Mock successful response
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
    }
}