//! Initialization command
//! 
//! Set up Jamey configuration and environment

use anyhow::Result;
use colored::*;
use std::path::PathBuf;
use tracing::{info, error};

/// Run initialization
pub async fn run_init(dir: PathBuf, force: bool) -> Result<()> {
    println!("{} Initializing Jamey configuration...", "🚀".cyan().bold());
    
    // Expand home directory
    let config_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("jamey");
    
    println!("{} Configuration directory: {}", "📁".blue(), config_dir.display());
    
    // Create config directory
    std::fs::create_dir_all(&config_dir)?;
    
    // Create default config file
    let config_file = config_dir.join("config.toml");
    if config_file.exists() && !force {
        println!("{} Configuration already exists. Use --force to overwrite.", "⚠️".yellow());
        return Ok(());
    }
    
    // Write default configuration
    let default_config = r#"[database]
url = "postgresql://username:password@localhost:5432/jamey"

[llm]
provider = "openrouter"
model = "claude-3-sonnet"
openrouter_api_key = "your_api_key_here"

[security]
api_key_required = true
api_key = "your_api_key_here"

[api]
host = "127.0.0.1"
port = 3000
enable_cors = true
"#;
    
    std::fs::write(&config_file, default_config)?;
    
    println!("{} Configuration created at: {}", "✅".green(), config_file.display());
    println!("{} Please edit the configuration file with your settings.", "💡".yellow());
    
    Ok(())
}