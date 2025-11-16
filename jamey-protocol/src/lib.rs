//! Protocol definitions for Digital Twin Jamey
//! 
//! This crate provides the core data structures, enums, and traits
//! that define the communication protocol between Jamey components.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use chrono::{DateTime, Utc};

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub role: Role,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: serde_json::Value,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Tool call from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: serde_json::Value,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub user_id: Option<String>,
    pub initial_context: Option<String>,
    pub tool_preferences: Vec<String>,
    pub metadata: serde_json::Value,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessContext {
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub include_memory: bool,
    pub memory_limit: Option<u32>,
    pub tool_choice: Option<String>,
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
    fn handle_message(&self, message: &str) -> Result<String, ProtocolError>;
    fn validate_message(&self, message: &str) -> Result<(), ProtocolError>;
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