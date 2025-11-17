//! Protocol definitions for Digital Twin Jamey
//!
//! This crate provides the core data structures, enums, and traits
//! that define the communication protocol between Jamey components.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use validator::{Validate, ValidationError};

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("Invalid message format: {0}")]
    InvalidFormat(String),
    #[error("Unsupported message type: {0}")]
    UnsupportedType(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Validation error: {0}")]
    Validation(String),
}

/// Message roles in conversations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// A message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Message {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub role: Role,
    #[validate(length(min = 1, max = 32768))]
    pub content: String,
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,
    #[validate(custom(function = "validate_metadata"))]
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
}

fn default_metadata() -> serde_json::Value {
    serde_json::json!({})
}

fn validate_metadata(metadata: &serde_json::Value) -> Result<(), ValidationError> {
    let serialized = serde_json::to_string(metadata)
        .map_err(|_e| ValidationError::new("invalid_json"))?;
    
    if serialized.len() > 16384 {
        return Err(ValidationError::new("metadata_too_large"));
    }
    
    if let Some(obj) = metadata.as_object() {
        if obj.len() > 50 {
            return Err(ValidationError::new("too_many_fields"));
        }
        for (key, value) in obj {
            if key.len() > 64 {
                return Err(ValidationError::new("key_too_long"));
            }
            if let Some(s) = value.as_str() {
                if s.len() > 1024 {
                    return Err(ValidationError::new("value_too_long"));
                }
            }
        }
    }
    Ok(())
}

impl Message {
    pub fn new(role: Role, content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            role,
            content,
            timestamp: Utc::now(),
            metadata: serde_json::json!({}),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, content.into())
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content.into())
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, content.into())
    }

    pub fn tool(content: impl Into<String>) -> Self {
        Self::new(Role::Tool, content.into())
    }
}

/// Tool specification for function calling
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ToolSpec {
    #[validate(length(min = 1, max = 64))]
    pub name: String,
    #[validate(length(min = 1, max = 512))]
    pub description: String,
    #[validate(custom(function = "validate_tool_parameters"))]
    pub parameters: serde_json::Value,
}

fn validate_tool_parameters(params: &serde_json::Value) -> Result<(), ValidationError> {
    let serialized = serde_json::to_string(params)
        .map_err(|_e| ValidationError::new("invalid_json"))?;
    
    if serialized.len() > 8192 {
        return Err(ValidationError::new("parameters_too_large"));
    }
    
    if let Some(obj) = params.as_object() {
        if obj.len() > 50 {
            return Err(ValidationError::new("too_many_parameters"));
        }
    }
    Ok(())
}

/// Tool call from LLM
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ToolCall {
    #[validate(length(min = 1, max = 64))]
    pub id: String,
    #[validate(length(min = 1, max = 64))]
    pub name: String,
    #[validate(custom(function = "validate_tool_args"))]
    pub args: serde_json::Value,
}

fn validate_tool_args(args: &serde_json::Value) -> Result<(), ValidationError> {
    let serialized = serde_json::to_string(args)
        .map_err(|_e| ValidationError::new("invalid_json"))?;
    
    if serialized.len() > 16384 {
        return Err(ValidationError::new("arguments_too_large"));
    }
    Ok(())
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub id: String,
    pub name: String,
    pub output: String,
    pub success: bool,
    pub error: Option<String>,
    pub execution_time_ms: Option<u64>,
}

impl ToolResult {
    pub fn success(id: String, name: String, output: String) -> Self {
        Self {
            id,
            name,
            output,
            success: true,
            error: None,
            execution_time_ms: None,
        }
    }

    pub fn error(id: String, name: String, error: String) -> Self {
        Self {
            id,
            name,
            output: String::new(),
            success: false,
            error: Some(error),
            execution_time_ms: None,
        }
    }
}

/// Session state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub message_count: u32,
    pub memory_entries: u32,
    pub active_tools: Vec<String>,
    pub metadata: serde_json::Value,
}

/// Request to create a new session
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateSessionRequest {
    pub user_id: Option<String>,
    pub initial_context: Option<String>,
    #[validate(length(min = 0, max = 100))]
    pub tool_preferences: Vec<String>,
    #[validate(custom(function = "validate_metadata"))]
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
}

fn validate_optional_user_id(user_id: &String) -> Result<(), ValidationError> {
    if !user_id.is_empty() {
        let id = user_id;
        if id.is_empty() || id.len() > 64 {
            return Err(ValidationError::new("invalid_user_id_length"));
        }
        if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(ValidationError::new("invalid_user_id_format"));
        }
    }
    Ok(())
}

fn validate_optional_context(context: &String) -> Result<(), ValidationError> {
    if !context.is_empty() {
        let ctx = context;
        if ctx.len() > 32768 {
            return Err(ValidationError::new("context_too_large"));
        }
    }
    Ok(())
}

/// Response for session creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionResponse {
    pub session_id: Uuid,
    pub state: SessionState,
}

/// Request to process a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMessageRequest {
    pub session_id: Uuid,
    pub message: Message,
    pub tools: Option<Vec<ToolSpec>>,
    pub context: Option<ProcessContext>,
}

/// Additional context for message processing
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ProcessContext {
    #[validate(range(min = 1, max = 32768))]
    pub max_tokens: Option<u32>,
    #[validate(range(min = 0.0, max = 2.0))]
    pub temperature: Option<f32>,
    #[serde(default = "default_include_memory")]
    pub include_memory: bool,
    #[validate(range(min = 1, max = 1000))]
    pub memory_limit: Option<u32>,
    pub tool_choice: Option<String>,
}

fn default_include_memory() -> bool {
    true
}

fn validate_tool_choice(choice: &String) -> Result<(), ValidationError> {
    if !choice.is_empty() {
        if choice != "auto" && choice != "none" && !choice.starts_with("function:") {
            return Err(ValidationError::new("invalid_tool_choice_format"));
        }
        if choice.len() > 100 {
            return Err(ValidationError::new("tool_choice_too_long"));
        }
    }
    Ok(())
}

/// Response for message processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMessageResponse {
    pub session_id: Uuid,
    pub message: Message,
    pub tool_calls: Vec<ToolCall>,
    pub tool_results: Vec<ToolResult>,
    pub memory_entries_added: u32,
    pub processing_time_ms: u64,
    pub usage: TokenUsage,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub active_sessions: u32,
    pub memory_usage_mb: f64,
    pub components: ComponentStatus,
}

/// Status of individual components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentStatus {
    pub database: String,
    pub llm_provider: String,
    pub memory_store: String,
    pub tool_registry: String,
}

/// Trait for protocol handlers
pub trait ProtocolHandler {
    fn handle_message(&self, message: &str) -> Result<String, ProtocolError> {
        // Validate message first
        self.validate_message(message)?;
        
        // Handle message implementation
        Ok(String::new())
    }

    fn validate_message(&self, message: &str) -> Result<(), ProtocolError> {
        // Basic message validation
        if message.is_empty() {
            return Err(ProtocolError::Validation("Empty message".to_string()));
        }
        if message.len() > 65536 {
            return Err(ProtocolError::Validation("Message too large".to_string()));
        }
        
        // Try to parse as JSON
        serde_json::from_str::<serde_json::Value>(message)
            .map_err(|e| ProtocolError::InvalidFormat(format!("Invalid JSON: {}", e)))?;
        
        Ok(())
    }
}

/// Trait for session management
pub trait SessionManager {
    fn create_session(&self, request: CreateSessionRequest) -> Result<CreateSessionResponse, ProtocolError>;
    fn get_session_state(&self, session_id: Uuid) -> Result<SessionState, ProtocolError>;
    fn update_session_activity(&self, session_id: Uuid) -> Result<(), ProtocolError>;
    fn delete_session(&self, session_id: Uuid) -> Result<(), ProtocolError>;
}

/// Common re-exports
pub mod prelude {
    pub use super::{
        Message, Role, ToolSpec, ToolCall, ToolResult, SessionState,
        CreateSessionRequest, CreateSessionResponse, ProcessMessageRequest,
        ProcessMessageResponse, ProcessContext, TokenUsage, HealthCheckResponse,
        ComponentStatus, ProtocolError, ProtocolHandler, SessionManager,
    };
    pub use chrono::{DateTime, Utc};
    pub use uuid::Uuid;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = Message::user("Hello, Jamey!");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello, Jamey!");
        assert!(msg.id != Uuid::nil());
    }

    #[test]
    fn test_tool_result() {
        let success = ToolResult::success("test_id".to_string(), "test_tool".to_string(), "Success".to_string());
        assert!(success.success);
        assert!(success.error.is_none());

        let error = ToolResult::error("test_id".to_string(), "test_tool".to_string(), "Failed".to_string());
        assert!(!error.success);
        assert!(error.error.is_some());
    }

    #[test]
    fn test_session_state_serialization() {
        let state = SessionState {
            id: Uuid::new_v4(),
            created_at: Utc::now(),
            last_activity: Utc::now(),
            message_count: 5,
            memory_entries: 10,
            active_tools: vec!["process_tool".to_string()],
            metadata: serde_json::json!({"test": true}),
        };

        let serialized = serde_json::to_string(&state).unwrap();
        let deserialized: SessionState = serde_json::from_str(&serialized).unwrap();
        assert_eq!(state.id, deserialized.id);
        assert_eq!(state.message_count, deserialized.message_count);
    }
}