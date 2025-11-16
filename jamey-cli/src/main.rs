//! Digital Twin Jamey CLI Application
//! 
//! Command-line interface for interacting with Jamey's capabilities
//! including chat, system management, and configuration.

use clap::{Parser, Subcommand};
use colored::*;
use anyhow::Result;
use std::path::PathBuf;
use tracing::{info, error, debug};
use tracing_subscriber::FmtSubscriber;

mod commands;
mod config;
mod utils;

use commands::*;

#[derive(Parser)]
#[command(name = "jamey")]
#[command(about = "Digital Twin Jamey - AI Assistant with System Capabilities")]
#[command(version = "0.1.0")]
#[command(author = "Jamey Milner")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Configuration file path
    #[arg(short, long, global = true, default_value = "~/.config/jamey/config.toml")]
    pub config: PathBuf,

    /// Enable debug logging
    #[arg(short, long, global = true)]
    pub debug: bool,

    /// Quiet mode (minimal output)
    #[arg(short, long, global = true)]
    pub quiet: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start interactive chat session
    Chat {
        /// Session ID to resume (optional)
        #[arg(short, long)]
        session: Option<String>,
        
        /// Model to use for conversation
        #[arg(short, long, default_value = "claude-3-sonnet")]
        model: String,
        
        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,
    },
    
    /// Manage system processes
    Process {
        #[command(subcommand)]
        action: ProcessAction,
    },
    
    /// Manage memory and knowledge
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },
    
    /// System configuration and status
    System {
        #[command(subcommand)]
        action: SystemAction,
    },
    
    /// Initialize Jamey configuration
    Init {
        /// Configuration directory
        #[arg(short, long, default_value = "~/.config/jamey")]
        dir: PathBuf,
        
        /// Force overwrite existing configuration
        #[arg(long)]
        force: bool,
    },
    
    /// Start Jamey runtime service
    Start {
        /// Run in background
        #[arg(short, long)]
        daemon: bool,
        
        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
    
    /// Stop Jamey runtime service
    Stop {
        /// Graceful shutdown timeout in seconds
        #[arg(short, long, default_value = "30")]
        timeout: u64,
    },
    
    /// Show system status and health
    Status {
        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,
        
        /// Output format (json, table, plain)
        #[arg(short, long, default_value = "table")]
        format: String,
    },
}

#[derive(Subcommand)]
pub enum ProcessAction {
    /// List all running processes
    List {
        /// Filter by process name
        #[arg(short, long)]
        filter: Option<String>,
        
        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,
    },
    
    /// Get information about a specific process
    Info {
        /// Process ID
        #[arg(short, long)]
        pid: u32,
    },
    
    /// Terminate a process
    Kill {
        /// Process ID
        #[arg(short, long)]
        pid: u32,
        
        /// Force kill (SIGKILL)
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Subcommand)]
pub enum MemoryAction {
    /// Search memory entries
    Search {
        /// Search query
        query: String,
        
        /// Limit number of results
        #[arg(short, long, default_value = "10")]
        limit: usize,
        
        /// Memory type filter
        #[arg(short, long)]
        type_filter: Option<String>,
    },
    
    /// List recent memories
    List {
        /// Number of recent entries to show
        #[arg(short, long, default_value = "20")]
        count: usize,
        
        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,
    },
    
    /// Delete memory entries
    Delete {
        /// Memory ID to delete
        #[arg(short, long)]
        id: String,
        
        /// Confirm deletion without prompt
        #[arg(short, long)]
        force: bool,
    },
    
    /// Export memory to file
    Export {
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,
        
        /// Export format (json, csv)
        #[arg(short, long, default_value = "json")]
        format: String,
    },
}

#[derive(Subcommand)]
pub enum SystemAction {
    /// Show system information
    Info {
        /// Include hardware details
        #[arg(short, long)]
        hardware: bool,
        
        /// Include network information
        #[arg(short, long)]
        network: bool,
    },
    
    /// Check system health
    Health {
        /// Run comprehensive health check
        #[arg(short, long)]
        comprehensive: bool,
    },
    
    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    
    /// Show logs
    Logs {
        /// Number of lines to show
        #[arg(short, long, default_value = "50")]
        lines: usize,
        
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
        
        /// Log level filter
        #[arg(short, long)]
        level: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    
    /// Set configuration value
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },
    
    /// Get configuration value
    Get {
        /// Configuration key
        key: String,
    },
    
    /// Reset configuration to defaults
    Reset {
        /// Confirm reset without prompt
        #[arg(short, long)]
        force: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.debug {
        tracing::Level::DEBUG
    } else if cli.quiet {
        tracing::Level::ERROR
    } else {
        tracing::Level::INFO
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    debug!("Starting Jamey CLI with command: {:?}", cli.command);

    // Execute command
    match run_command(cli).await {
        Ok(_) => {
            info!("Command completed successfully");
            Ok(())
        }
        Err(e) => {
            error!("Command failed: {}", e);
            eprintln!("{} {}", "Error:".red().bold(), e);
            std::process::exit(1);
        }
    }
}

async fn run_command(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Chat { session, model, verbose } => {
            chat::run_chat(session, model, verbose).await
        }
        Commands::Process { action } => {
            process::run_process_action(action).await
        }
        Commands::Memory { action } => {
            memory::run_memory_action(action).await
        }
        Commands::System { action } => {
            system::run_system_action(action).await
        }
        Commands::Init { dir, force } => {
            init::run_init(dir, force).await
        }
        Commands::Start { daemon, port } => {
            start::run_start(daemon, port).await
        }
        Commands::Stop { timeout } => {
            stop::run_stop(timeout).await
        }
        Commands::Status { detailed, format } => {
            status::run_status(detailed, format).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_parsing() {
        let cli = Cli::try_parse_from(&["jamey", "chat", "--model", "gpt-4"]).unwrap();
        match cli.command {
            Commands::Chat { model, .. } => {
                assert_eq!(model, "gpt-4");
            }
            _ => panic!("Expected chat command"),
        }
    }

    #[test]
    fn test_process_command_parsing() {
        let cli = Cli::try_parse_from(&["jamey", "process", "list", "--filter", "chrome"]).unwrap();
        match cli.command {
            Commands::Process { action } => {
                match action {
                    ProcessAction::List { filter, .. } => {
                        assert_eq!(filter, Some("chrome".to_string()));
                    }
                    _ => panic!("Expected list action"),
                }
            }
            _ => panic!("Expected process command"),
        }
    }
}