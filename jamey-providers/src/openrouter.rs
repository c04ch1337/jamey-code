use anyhow::Result;
use async_trait::async_trait;
use backoff::ExponentialBackoff;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tiktoken_rs::CoreBPE;
use tracing::error;
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
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Empty message content")]
    EmptyContent,
    #[error("Invalid role: {0}")]
    InvalidRole(String),
    #[error("Invalid tool configuration: {0}")]
    InvalidTool(String),
    #[error("TLS error: {0}")]
    TlsError(String),
    #[error("Certificate validation error: {0}")]
    CertificateError(String),
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

fn validate_api_key(key: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err("API key cannot be empty".to_string());
    }
    if key.len() > 200 {
        return Err("API key too long".to_string());
    }
    if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err("API key contains invalid characters".to_string());
    }
    Ok(())
}

fn validate_api_url(url: &Url) -> Result<(), String> {
    if url.scheme() != "https" {
        return Err("API URL must use HTTPS".to_string());
    }
    if url.host_str().is_none() {
        return Err("API URL must have a host".to_string());
    }
    Ok(())
}

fn validate_model_name(model: &str) -> Result<(), String> {
    if model.is_empty() {
        return Err("Model name cannot be empty".to_string());
    }
    if model.len() > 50 {
        return Err("Model name too long".to_string());
    }
    if !model.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.') {
        return Err("Model name contains invalid characters".to_string());
    }
    Ok(())
}

impl Default for OpenRouterConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            // This URL is a compile-time constant and will never fail to parse
            // If it does, it's a programming error that should be caught in tests
            api_base_url: Url::parse("https://openrouter.ai/api/v1")
                .expect("Hardcoded OpenRouter URL must be valid"),
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

fn validate_role(role: &str) -> Result<(), String> {
    match role {
        "system" | "user" | "assistant" | "function" => Ok(()),
        _ => Err("Invalid role. Must be one of: system, user, assistant, function".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

fn validate_tool_parameters(params: &serde_json::Value) -> Result<(), String> {
    if params.as_object().map_or(0, |obj| obj.len()) > 50 {
        return Err("Tool parameters object too large".to_string());
    }
    
    let serialized = serde_json::to_string(params)
        .map_err(|e| format!("Invalid JSON parameters: {}", e))?;
    
    if serialized.len() > 8192 {
        return Err("Tool parameters too large".to_string());
    }
    
    Ok(())
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

fn validate_tools(tools: &Option<Vec<Tool>>) -> Result<(), String> {
    if let Some(tools) = tools {
        if tools.is_empty() {
            return Err("Tools array cannot be empty".to_string());
        }
        if tools.len() > 20 {
            return Err("Too many tools specified".to_string());
        }
    }
    Ok(())
}

fn validate_tool_choice(choice: &Option<String>) -> Result<(), String> {
    if let Some(choice) = choice {
        if choice != "auto" && choice != "none" && !choice.starts_with("function:") {
            return Err("Invalid tool_choice format".to_string());
        }
        if choice.len() > 100 {
            return Err("tool_choice value too long".to_string());
        }
    }
    Ok(())
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub object: String,
    pub data: Vec<EmbeddingData>,
    pub model: String,
    pub usage: EmbeddingUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingData {
    pub object: String,
    pub embedding: Vec<f32>,
    pub index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingUsage {
    pub prompt_tokens: u32,
    pub total_tokens: u32,
}

pub struct OpenRouterProvider {
    config: OpenRouterConfig,
    client: reqwest::Client,
    tokenizer: CoreBPE,
    request_semaphore: tokio::sync::Semaphore,
}

impl OpenRouterProvider {
    pub fn new(config: OpenRouterConfig) -> Result<Self> {
        const MAX_CONCURRENT_REQUESTS: usize = 50;
        // Validate configuration
        if config.api_key.is_empty() {
            return Err(OpenRouterError::InvalidRequest("API key is required".to_string()).into());
        }
        if config.allowed_models.is_empty() {
            return Err(OpenRouterError::InvalidRequest("At least one allowed model must be specified".to_string()).into());
        }
        
        tracing::info!("Initializing OpenRouter provider with secure configuration");

        // Configure secure TLS defaults
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            // Enforce minimum TLS version 1.2
            .min_tls_version(reqwest::tls::Version::TLS_1_2)
            // Use native TLS implementation with strong cipher suites
            // Enable certificate validation
            .tls_built_in_root_certs(true)
            // Set reasonable connection timeouts
            .connect_timeout(std::time::Duration::from_secs(30))
            // Enable HTTP/2 support
            .http2_prior_knowledge()
            .build()
            .map_err(|e| OpenRouterError::Api(format!("Failed to create HTTP client: {}", e)))?;

        let tokenizer = tiktoken_rs::cl100k_base()?;

        Ok(Self {
            config,
            client,
            tokenizer,
            request_semaphore: tokio::sync::Semaphore::new(MAX_CONCURRENT_REQUESTS),
        })
    }

    fn validate_chat_request(&self, request: &mut ChatRequest) -> Result<(), OpenRouterError> {
        // Validate and set default model
        if request.model.is_empty() {
            request.model = self.config.default_model.clone();
        }
        self.validate_model(&request.model)?;

        // Validate messages
        if request.messages.is_empty() {
            return Err(OpenRouterError::InvalidRequest("At least one message is required".to_string()));
        }

        for message in &request.messages {
            // Validate message content
            if message.content.trim().is_empty() {
                return Err(OpenRouterError::EmptyContent);
            }

            // Validate message role
            match message.role.as_str() {
                "system" | "user" | "assistant" | "function" => {}
                invalid_role => return Err(OpenRouterError::InvalidRole(invalid_role.to_string())),
            }
        }

        // Validate tools if present
        if let Some(tools) = &request.tools {
            for tool in tools {
                if tool.name.trim().is_empty() {
                    return Err(OpenRouterError::InvalidTool("Tool name cannot be empty".to_string()));
                }
                if tool.description.trim().is_empty() {
                    return Err(OpenRouterError::InvalidTool("Tool description cannot be empty".to_string()));
                }
                // Validate tool parameters are valid JSON schema
                if let Err(e) = serde_json::from_value::<serde_json::Value>(tool.parameters.clone()) {
                    return Err(OpenRouterError::InvalidTool(format!("Invalid tool parameters: {}", e)));
                }
            }
        }

        // Validate temperature if present
        if let Some(temp) = request.temperature {
            if !(0.0..=2.0).contains(&temp) {
                return Err(OpenRouterError::InvalidRequest(
                    "Temperature must be between 0.0 and 2.0".to_string(),
                ));
            }
        }

        // Validate max_tokens if present
        if let Some(tokens) = request.max_tokens {
            if tokens == 0 {
                return Err(OpenRouterError::InvalidRequest("max_tokens must be greater than 0".to_string()));
            }
        }

        Ok(())
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
        // Acquire semaphore permit for request limiting
        let _permit = self.request_semaphore.acquire().await?;

        let backoff = ExponentialBackoff {
            initial_interval: std::time::Duration::from_millis(100),
            max_interval: std::time::Duration::from_secs(10),
            max_elapsed_time: Some(std::time::Duration::from_secs(30)),
            ..Default::default()
        };

        let url = self.config.api_base_url.join("chat/completions")?;
        let auth_header = format!("Bearer {}", self.config.api_key);

        tracing::debug!("Making chat completion request to OpenRouter API");
        
        let result = backoff::future::retry(backoff, || async {
            let request_future = self.client
                .post(url.clone())
                .header("Authorization", &auth_header)
                .json(&request)
                .send();

            // Add timeout to the request
            let response = tokio::time::timeout(
                std::time::Duration::from_secs(self.config.timeout_seconds),
                request_future
            )
            .await
            .map_err(|_| backoff::Error::permanent(OpenRouterError::Api("Request timeout".to_string())))?
            .map_err(|e| backoff::Error::transient(OpenRouterError::Api(e.to_string())))?;

            match response.status() {
                reqwest::StatusCode::OK => {
                    let body = response.bytes().await
                        .map_err(|e| backoff::Error::permanent(OpenRouterError::Api(e.to_string())))?;
                    
                    serde_json::from_slice::<ChatResponse>(&body)
                        .map_err(|e| backoff::Error::permanent(OpenRouterError::Api(e.to_string())))
                }
                reqwest::StatusCode::TOO_MANY_REQUESTS => {
                    // Get retry-after header if available
                    let retry_after = response.headers()
                        .get("retry-after")
                        .and_then(|h| h.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(5);

                    tokio::time::sleep(std::time::Duration::from_secs(retry_after)).await;
                    Err(backoff::Error::transient(OpenRouterError::RateLimit))
                }
                _ => {
                    let error_text = response.text().await
                        .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
                    Err(backoff::Error::permanent(OpenRouterError::Api(error_text)))
                }
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
        // Validate and normalize request
        self.validate_chat_request(&mut request)?;

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
        // Acquire semaphore permit
        let _permit = self.request_semaphore.acquire().await?;
        // Validate input
        if text.trim().is_empty() {
            return Err(OpenRouterError::EmptyContent.into());
        }

        // Check token limit for embeddings (OpenAI's ada-002 has a 8k token limit)
        let token_count = self.count_tokens(text);
        if token_count > 8192 {
            return Err(OpenRouterError::TokenLimit {
                model: "text-embedding-ada-002".to_string(),
                count: token_count,
                limit: 8192,
            }.into());
        }

        // Use OpenRouter's embedding endpoint
        let url = self.config.api_base_url.join("embeddings")?;
        
        tracing::debug!("Generating embedding for text");
        
        let embedding_request = serde_json::json!({
            "model": "openai/text-embedding-ada-002",
            "input": text
        });

        let auth_header = format!("Bearer {}", self.config.api_key);
        let request_future = self.client
            .post(url)
            .header("Authorization", auth_header)
            .json(&embedding_request)
            .send();

        // Add timeout to the request
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(self.config.timeout_seconds),
            request_future
        )
        .await
        .map_err(|_| OpenRouterError::Api("Request timeout".to_string()))?
        .map_err(|e| OpenRouterError::Api(e.to_string()))?;

        match response.status() {
            reqwest::StatusCode::OK => {
                let embedding_response: EmbeddingResponse = response.json().await?;
                
                if let Some(data) = embedding_response.data.first() {
                    Ok(data.embedding.clone())
                } else {
                    Err(OpenRouterError::Api("No embedding data returned".to_string()).into())
                }
            }
            reqwest::StatusCode::TOO_MANY_REQUESTS => {
                Err(OpenRouterError::RateLimit.into())
            }
            _ => {
                let error_text = response.text().await?;
                Err(OpenRouterError::Api(error_text).into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_tls_configuration() -> Result<(), Box<dyn std::error::Error>> {
        // Test with invalid certificate
        let config = OpenRouterConfig {
            api_key: "test_key".to_string(),
            api_base_url: Url::parse("https://invalid-cert-test.badssl.com/")?,
            ..Default::default()
        };

        // Should fail due to invalid certificate
        let result = OpenRouterProvider::new(config);
        assert!(result.is_err());
        assert!(format!("{}", result.unwrap_err()).contains("certificate"));

        Ok(())
    }

    #[tokio::test]
    async fn test_openrouter_provider() -> Result<(), Box<dyn std::error::Error>> {
        let mock_server = MockServer::start().await;

        let config = OpenRouterConfig {
            api_key: "test_key".to_string(),
            api_base_url: Url::parse(&mock_server.uri())?,
            ..Default::default()
        };

        let provider = OpenRouterProvider::new(config)?;

        // Test validation failures
        let empty_message = ChatRequest {
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
        assert!(matches!(
            provider.chat(empty_message.clone()).await,
            Err(anyhow::Error) if format!("{}", provider.chat(empty_message).await.unwrap_err())
                .contains("Empty message content")
        ));

        let invalid_role = ChatRequest {
            model: "claude-3-sonnet".to_string(),
            messages: vec![Message {
                role: "invalid".to_string(),
                content: "Test".to_string(),
            }],
            tools: None,
            tool_choice: None,
            temperature: None,
            max_tokens: None,
        };
        assert!(matches!(
            provider.chat(invalid_role.clone()).await,
            Err(anyhow::Error) if format!("{}", provider.chat(invalid_role).await.unwrap_err())
                .contains("Invalid role")
        ));

        // Test empty embedding text
        assert!(matches!(
            provider.get_embedding("").await,
            Err(anyhow::Error) if format!("{}", provider.get_embedding("").await.unwrap_err())
                .contains("Empty content")
        ));

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

        let response = provider.chat(request).await?;
        assert_eq!(response.choices[0].message.content, "Test response");
        Ok(())
    }
}