//! Chat command implementation
//! 
//! Interactive chat interface for conversing with Jamey

use anyhow::{Context, Result};
use colored::*;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
    cursor::{MoveTo, Show, Hide},
};
use std::io::{stdout, Write};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use jamey_protocol::{Message, Role, ProcessMessageRequest, ProcessContext};
use jamey_runtime::Runtime;
use tracing::{info, debug, error};

/// Run interactive chat session
pub async fn run_chat(
    session_id: Option<String>,
    model: String,
    verbose: bool,
) -> Result<()> {
    println!("{}", "🤖 Digital Twin Jamey - Chat Mode".bright_cyan().bold());
    println!("{}", "Type 'exit' or press Ctrl+C to quit".dim());
    println!("{}", "Type 'help' for available commands".dim());
    println!();

    // Initialize runtime
    let config = load_runtime_config(&model).await?;
    let mut runtime = Runtime::new(config).await?;
    
    // Create or resume session
    let session_id = if let Some(id) = session_id {
        // Validate UUID format
        crate::utils::validate_uuid(&id)
            .with_context(|| format!("Invalid session ID format: {}", id))?
    } else {
        runtime.state().session_manager.create_session()
    };

    println!("{} Session ID: {}", "📝".blue(), session_id);
    println!();

    // Chat history
    let chat_history = Arc::new(RwLock::new(Vec::<Message>::new()));

    // Main chat loop
    loop {
        print!("{} ", "You:".green().bold());
        stdout().flush()?;

        let mut input = String::new();
        
        // Read user input
        if let Err(e) = std::io::stdin().read_line(&mut input) {
            error!("Failed to read input: {}", e);
            continue;
        }

        let input = input.trim().to_string();
        
        // Handle special commands
        match input.as_str() {
            "exit" | "quit" => {
                println!("{} Goodbye!", "👋".yellow());
                break;
            }
            "help" => {
                print_help();
                continue;
            }
            "clear" => {
                // Clear screen
                execute!(stdout(), Clear(ClearType::All), MoveTo(0, 0))?;
                continue;
            }
            "history" => {
                show_history(&chat_history).await;
                continue;
            }
            "" => continue, // Skip empty input
            _ => {}
        }

        // Add user message to history
        let user_message = Message::user(input.clone());
        chat_history.write().await.push(user_message.clone());

        if verbose {
            println!("{} Processing message...", "⏳".yellow());
        }

        // Process message through Jamey
        match process_message(&runtime, session_id, &user_message, verbose).await {
            Ok(response) => {
                // Display assistant response
                println!("{} {}", "Jamey:".blue().bold(), response.message.content);
                
                // Add to history
                chat_history.write().await.push(response.message);
                
                // Show tool results if any
                if !response.tool_results.is_empty() && verbose {
                    println!("{} Tool Results:", "🔧".cyan());
                    for result in response.tool_results {
                        if result.success {
                            println!("  ✅ {}: {}", result.name, result.output);
                        } else {
                            println!("  ❌ {}: {}", result.name, 
                                result.error.unwrap_or_else(|| "Unknown error".to_string()));
                        }
                    }
                }
                
                // Show token usage if verbose
                if verbose {
                    println!("{} Tokens: {} prompt + {} completion = {} total", 
                        "📊".magenta(),
                        response.usage.prompt_tokens,
                        response.usage.completion_tokens,
                        response.usage.total_tokens);
                }
            }
            Err(e) => {
                error!("Failed to process message: {}", e);
                println!("{} {}", "❌".red(), "Sorry, I encountered an error processing your message.");
            }
        }
        
        println!();
    }

    // Cleanup
    runtime.shutdown().await;
    Ok(())
}

/// Load runtime configuration for chat
async fn load_runtime_config(model: &str) -> Result<jamey_runtime::RuntimeConfig> {
    let mut config = jamey_runtime::RuntimeConfig::from_env()
        .map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?;
    
    // Override model setting
    config.llm.openrouter_default_model = model.to_string();
    
    Ok(config)
}

/// Process a message through the runtime
async fn process_message(
    runtime: &Runtime,
    session_id: Uuid,
    message: &Message,
    verbose: bool,
) -> Result<jamey_protocol::ProcessMessageResponse> {
    let state = runtime.state();
    let start_time = std::time::Instant::now();
    
    if verbose {
        debug!("Processing message for session {}: {}", session_id, message.content);
    }

    // Get session (should already exist from run_chat)
    let session = state.session_manager.get_session(session_id)
        .ok_or_else(|| anyhow::anyhow!("Session not found: {}. Session may have expired.", session_id))?;
    
    // Build message history for LLM
    let mut llm_messages = Vec::new();
    
    // Add system message if this is a new conversation
    if session.memory_context.is_empty() {
        llm_messages.push(jamey_providers::openrouter::Message {
            role: "system".to_string(),
            content: "You are Jamey, a helpful AI assistant. Be concise, accurate, and helpful.".to_string(),
        });
    }
    
    // Convert protocol messages to provider messages
    // For now, we'll just send the current message
    // In a full implementation, we'd include conversation history
    llm_messages.push(jamey_providers::openrouter::Message {
        role: match message.role {
            jamey_protocol::Role::User => "user".to_string(),
            jamey_protocol::Role::Assistant => "assistant".to_string(),
            jamey_protocol::Role::System => "system".to_string(),
            jamey_protocol::Role::Tool => "tool".to_string(),
        },
        content: message.content.clone(),
    });
    
    // Create chat request
    let chat_request = jamey_providers::openrouter::ChatRequest {
        model: state.config.llm.openrouter_default_model.clone(),
        messages: llm_messages,
        tools: None,
        tool_choice: None,
        temperature: Some(0.7),
        max_tokens: Some(4000),
    };
    
    // Call LLM provider
    let chat_response = state.llm_provider.chat(chat_request).await
        .with_context(|| "Failed to get response from LLM provider")?;
    
    // Extract response
    let assistant_message = chat_response.choices
        .first()
        .and_then(|c| Some(c.message.content.clone()))
        .unwrap_or_else(|| "No response from LLM".to_string());
    
    let processing_time_ms = start_time.elapsed().as_millis() as u64;
    
    // Create protocol response
    let response = jamey_protocol::ProcessMessageResponse {
        session_id,
        message: jamey_protocol::Message::assistant(assistant_message),
        tool_calls: vec![], // TODO: Extract tool calls from response
        tool_results: vec![],
        memory_entries_added: 0, // TODO: Store message in memory
        processing_time_ms,
        usage: jamey_protocol::TokenUsage {
            prompt_tokens: chat_response.usage.prompt_tokens,
            completion_tokens: chat_response.usage.completion_tokens,
            total_tokens: chat_response.usage.total_tokens,
        },
    };

    Ok(response)
}

/// Print help information
fn print_help() {
    println!("{} Available Commands:", "📖".cyan());
    println!("  {}  Exit the chat", "exit, quit".yellow());
    println!("  {}  Show this help", "help".yellow());
    println!("  {}  Clear the screen", "clear".yellow());
    println!("  {}  Show chat history", "history".yellow());
    println!("  {}  Start a new session", "new".yellow());
    println!("  {}  Save current session", "save".yellow());
    println!("  {}  Load saved session", "load <id>".yellow());
    println!();
}

/// Show chat history
async fn show_history(history: &Arc<RwLock<Vec<Message>>>) {
    let history = history.read().await;
    
    println!("{} Chat History:", "📜".cyan());
    println!("{}", "─".repeat(50));
    
    for (i, message) in history.iter().enumerate() {
        let role_color = match message.role {
            Role::User => colored::Color::Green,
            Role::Assistant => colored::Color::Blue,
            Role::System => colored::Color::Magenta,
            Role::Tool => colored::Color::Yellow,
        };
        
        println!("{} {}: {}", 
            (i + 1).to_string().dim(),
            format!("{:?}", message.role).color(role_color).bold(),
            message.content);
    }
    
    println!("{}", "─".repeat(50));
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use jamey_protocol::Role;

    #[tokio::test]
    async fn test_message_processing() {
        // This is a placeholder test - in a real implementation,
        // we would test the actual message processing logic
        let message = Message::user("Hello, Jamey!");
        assert_eq!(message.role, Role::User);
        assert_eq!(message.content, "Hello, Jamey!");
    }
}