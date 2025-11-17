//! Secure logging module with automatic PII and sensitive data filtering
//!
//! This module provides a logging framework that automatically redacts sensitive information
//! from log messages, including API keys, passwords, tokens, email addresses, and other PII.

use regex::Regex;
use std::sync::OnceLock;
use tracing::field::{Field, Visit};
use tracing::{Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

/// Patterns for detecting sensitive data in log messages
static SENSITIVE_PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();

/// Initialize sensitive data patterns
///
/// # Panics
/// This function will panic if any of the hardcoded regex patterns are invalid.
/// Since these are compile-time constants, this should never happen in production.
fn get_sensitive_patterns() -> &'static Vec<(Regex, &'static str)> {
    SENSITIVE_PATTERNS.get_or_init(|| {
        // These regex patterns are hardcoded and should always be valid
        // If any fail to compile, it's a programming error that should be caught in tests
        vec![
            // API Keys and Tokens (various formats)
            (Regex::new(r"(?i)(api[_-]?key|apikey)[\s:=]+[a-zA-Z0-9_-]{20,}")
                .expect("API key regex pattern is invalid"), "$1=***REDACTED***"),
            (Regex::new(r"(?i)(token|access[_-]?token|auth[_-]?token)[\s:=]+[a-zA-Z0-9_.-]{20,}")
                .expect("Token regex pattern is invalid"), "$1=***REDACTED***"),
            (Regex::new(r"(?i)(bearer\s+)[a-zA-Z0-9_.-]{20,}")
                .expect("Bearer token regex pattern is invalid"), "$1***REDACTED***"),
            
            // Passwords and Secrets
            (Regex::new(r"(?i)(password|passwd|pwd|secret)[\s:=]+\S{3,}")
                .expect("Password regex pattern is invalid"), "$1=***REDACTED***"),
            (Regex::new(r"(?i)(client[_-]?secret|app[_-]?secret)[\s:=]+[a-zA-Z0-9_-]{20,}")
                .expect("Client secret regex pattern is invalid"), "$1=***REDACTED***"),
            
            // Database credentials
            (Regex::new(r"(?i)(postgres|mysql|mongodb)://([^:]+):([^@]+)@")
                .expect("Database URL regex pattern is invalid"), "$1://$2:***REDACTED***@"),
            
            // Email addresses
            (Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b")
                .expect("Email regex pattern is invalid"), "***EMAIL_REDACTED***"),
            
            // IP addresses (optional - can be disabled if IPs are not considered sensitive)
            (Regex::new(r"\b(?:[0-9]{1,3}\.){3}[0-9]{1,3}\b")
                .expect("IP address regex pattern is invalid"), "***IP_REDACTED***"),
            
            // Credit card numbers (basic pattern)
            (Regex::new(r"\b\d{4}[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4}\b")
                .expect("Credit card regex pattern is invalid"), "***CC_REDACTED***"),
            
            // Social Security Numbers (US format)
            (Regex::new(r"\b\d{3}-\d{2}-\d{4}\b")
                .expect("SSN regex pattern is invalid"), "***SSN_REDACTED***"),
            
            // JWT tokens
            (Regex::new(r"eyJ[a-zA-Z0-9_-]*\.eyJ[a-zA-Z0-9_-]*\.[a-zA-Z0-9_-]*")
                .expect("JWT regex pattern is invalid"), "***JWT_REDACTED***"),
            
            // GitHub tokens
            (Regex::new(r"(?i)(gh[ps]_[a-zA-Z0-9]{36,})")
                .expect("GitHub token regex pattern is invalid"), "***GITHUB_TOKEN_REDACTED***"),
            
            // AWS keys
            (Regex::new(r"(?i)(AKIA[0-9A-Z]{16})")
                .expect("AWS key regex pattern is invalid"), "***AWS_KEY_REDACTED***"),
        ]
    })
}

/// Redacts sensitive information from a string
pub fn redact_sensitive_data(input: &str) -> String {
    let mut result = input.to_string();
    
    for (pattern, replacement) in get_sensitive_patterns() {
        result = pattern.replace_all(&result, *replacement).to_string();
    }
    
    result
}

/// List of field names that should always be redacted
const SENSITIVE_FIELD_NAMES: &[&str] = &[
    "password",
    "passwd",
    "pwd",
    "secret",
    "api_key",
    "apikey",
    "token",
    "access_token",
    "refresh_token",
    "auth_token",
    "authorization",
    "client_secret",
    "private_key",
    "credential",
    "credentials",
    "session_id",
    "cookie",
    "auth",
];

/// Checks if a field name indicates sensitive data
fn is_sensitive_field(field_name: &str) -> bool {
    let lower = field_name.to_lowercase();
    SENSITIVE_FIELD_NAMES.iter().any(|&sensitive| {
        lower.contains(sensitive)
    })
}

/// A visitor that redacts sensitive field values
struct RedactingVisitor {
    redacted_fields: Vec<(String, String)>,
}

impl RedactingVisitor {
    fn new() -> Self {
        Self {
            redacted_fields: Vec::new(),
        }
    }
}

impl Visit for RedactingVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let field_name = field.name();
        let value_str = format!("{:?}", value);
        
        if is_sensitive_field(field_name) {
            self.redacted_fields.push((field_name.to_string(), "***REDACTED***".to_string()));
        } else {
            let redacted_value = redact_sensitive_data(&value_str);
            self.redacted_fields.push((field_name.to_string(), redacted_value));
        }
    }
    
    fn record_str(&mut self, field: &Field, value: &str) {
        let field_name = field.name();
        
        if is_sensitive_field(field_name) {
            self.redacted_fields.push((field_name.to_string(), "***REDACTED***".to_string()));
        } else {
            let redacted_value = redact_sensitive_data(value);
            self.redacted_fields.push((field_name.to_string(), redacted_value));
        }
    }
}

/// A tracing layer that redacts sensitive information from logs
pub struct RedactingLayer;

impl<S> Layer<S> for RedactingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: Context<'_, S>,
    ) {
        let metadata = event.metadata();
        
        // Create a visitor to collect and redact fields
        let mut visitor = RedactingVisitor::new();
        event.record(&mut visitor);
        
        // Log the redacted event
        let level = metadata.level();
        let target = metadata.target();
        
        let message = visitor.redacted_fields
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(", ");
        
        match *level {
            Level::ERROR => log::error!(target: target, "{}", message),
            Level::WARN => log::warn!(target: target, "{}", message),
            Level::INFO => log::info!(target: target, "{}", message),
            Level::DEBUG => log::debug!(target: target, "{}", message),
            Level::TRACE => log::trace!(target: target, "{}", message),
        }
    }
}

/// Configuration for log rotation and retention
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Directory where logs are stored
    pub log_dir: std::path::PathBuf,
    /// Maximum size of a single log file in bytes (default: 10MB)
    pub max_file_size: u64,
    /// Maximum number of archived log files to keep (default: 10)
    pub max_backups: usize,
    /// Number of days to retain logs (default: 30)
    pub retention_days: u32,
    /// Whether to compress archived logs (default: true)
    pub compress: bool,
    /// Log level filter
    pub level: Level,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            log_dir: std::path::PathBuf::from("./logs"),
            max_file_size: 10 * 1024 * 1024, // 10MB
            max_backups: 10,
            retention_days: 30,
            compress: true,
            level: Level::INFO,
        }
    }
}

impl LogConfig {
    /// Creates a new LogConfig with custom settings
    pub fn new(log_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            log_dir: log_dir.into(),
            ..Default::default()
        }
    }
    
    /// Sets the maximum file size
    pub fn with_max_file_size(mut self, size: u64) -> Self {
        self.max_file_size = size;
        self
    }
    
    /// Sets the maximum number of backups
    pub fn with_max_backups(mut self, count: usize) -> Self {
        self.max_backups = count;
        self
    }
    
    /// Sets the retention period in days
    pub fn with_retention_days(mut self, days: u32) -> Self {
        self.retention_days = days;
        self
    }
    
    /// Sets whether to compress archived logs
    pub fn with_compression(mut self, compress: bool) -> Self {
        self.compress = compress;
        self
    }
    
    /// Sets the log level
    pub fn with_level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }
}

/// Initialize secure logging with the given configuration
pub fn init_secure_logging(config: LogConfig) -> Result<(), Box<dyn std::error::Error>> {
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::fmt;
    use tracing_subscriber::EnvFilter;
    
    // Create log directory if it doesn't exist
    std::fs::create_dir_all(&config.log_dir)?;
    
    // Set up file appender with rotation
    let file_appender = tracing_appender::rolling::daily(&config.log_dir, "jamey.log");
    
    // Create the subscriber with redacting layer
    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env()
            .add_directive(config.level.into()))
        .with(RedactingLayer)
        .with(fmt::layer()
            .with_writer(file_appender)
            .with_ansi(false)
            .with_target(true)
            .with_thread_ids(true)
            .with_line_number(true));
    
    tracing::subscriber::set_global_default(subscriber)?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_api_key() {
        let input = "Using api_key: sk_test_1234567890abcdefghij";
        let output = redact_sensitive_data(input);
        assert!(output.contains("***REDACTED***"));
        assert!(!output.contains("sk_test_1234567890abcdefghij"));
    }

    #[test]
    fn test_redact_password() {
        let input = "password=mysecretpass123";
        let output = redact_sensitive_data(input);
        assert!(output.contains("***REDACTED***"));
        assert!(!output.contains("mysecretpass123"));
    }

    #[test]
    fn test_redact_email() {
        let input = "User email: user@example.com";
        let output = redact_sensitive_data(input);
        assert!(output.contains("***EMAIL_REDACTED***"));
        assert!(!output.contains("user@example.com"));
    }

    #[test]
    fn test_redact_bearer_token() {
        let input = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        let output = redact_sensitive_data(input);
        assert!(output.contains("***REDACTED***") || output.contains("***JWT_REDACTED***"));
    }

    #[test]
    fn test_redact_database_url() {
        let input = "postgres://user:password123@localhost:5432/db";
        let output = redact_sensitive_data(input);
        assert!(output.contains("***REDACTED***"));
        assert!(!output.contains("password123"));
    }

    #[test]
    fn test_is_sensitive_field() {
        assert!(is_sensitive_field("password"));
        assert!(is_sensitive_field("api_key"));
        assert!(is_sensitive_field("API_KEY"));
        assert!(is_sensitive_field("user_password"));
        assert!(is_sensitive_field("oauth_token"));
        assert!(!is_sensitive_field("username"));
        assert!(!is_sensitive_field("email_verified"));
    }

    #[test]
    fn test_non_sensitive_data() {
        let input = "Processing request for user_id: 12345";
        let output = redact_sensitive_data(input);
        assert_eq!(input, output);
    }
}