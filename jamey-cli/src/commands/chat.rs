//! Chat command implementation
//! 
//! Interactive chat interface for conversing with Jamey

use anyhow::Result;
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
        Uuid::parse_str(&id)?
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
    
    // Create process request
    let request = ProcessMessageRequest {
        session_id,
        message: message.clone(),
        tools: None, // Let runtime decide available tools
        context: Some(ProcessContext {
            max_tokens: Some(4000),
            temperature: Some(0.7),
            include_memory: true,
            memory_limit: Some(10),
            tool_choice: None,
        }),
    };

    if verbose {
        debug!("Processing message: {:?}", request);
    }

    // For now, create a mock response since we don't have the full processing pipeline
    let response = jamey_protocol::ProcessMessageResponse {
        session_id,
        message: Message::assistant(format!("I received your message: {}", message.content)),
        tool_calls: vec![],
        tool_results: vec![],
        memory_entries_added: 0,
        processing_time_ms: 100,
        usage: jamey_protocol::TokenUsage {
            prompt_tokens: 50,
            completion_tokens: 25,
            total_tokens: 75,
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