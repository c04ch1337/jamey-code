//! System management commands
//! 
//! Interface for system configuration and status

use anyhow::{Context, Result};
use colored::*;
use crate::commands::{SystemAction, ConfigAction};
use crate::config::CliConfig;
use jamey_runtime::{Runtime, RuntimeConfig};
use tracing::{info, error, debug};
use std::time::Instant;
use std::fs;
use std::path::PathBuf;
use dirs::home_dir;

/// Run system management action
pub async fn run_system_action(action: SystemAction) -> Result<()> {
    match action {
        SystemAction::Info { hardware, network } => {
            show_system_info(hardware, network).await
        }
        SystemAction::Health { comprehensive } => {
            check_system_health(comprehensive).await
        }
        SystemAction::Config { action } => {
            run_config_action(action).await
        }
        SystemAction::Logs { lines, follow, level } => {
            show_logs(lines, follow, level).await
        }
    }
}

/// Show system information
async fn show_system_info(hardware: bool, network: bool) -> Result<()> {
    println!("{} System Information", "💻".cyan().bold());
    println!("{}", "═".repeat(50));
    println!();
    
    // Basic system info
    println!("{} Operating System:", "🖥️".blue().bold());
    println!("  OS: {}", std::env::consts::OS);
    println!("  Arch: {}", std::env::consts::ARCH);
    println!();
    
    // Hardware info
    if hardware {
        println!("{} Hardware Information:", "⚙️".blue().bold());
        
        // CPU info (basic)
        if let Ok(num_cpus) = std::thread::available_parallelism() {
            println!("  CPU Cores: {}", num_cpus);
        }
        
        // Memory info (if available)
        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            if let Ok(output) = Command::new("wmic")
                .args(&["computersystem", "get", "TotalPhysicalMemory", "/value"])
                .output()
            {
                let output_str = String::from_utf8_lossy(&output.stdout);
                if let Some(memory_line) = output_str.lines().find(|l| l.starts_with("TotalPhysicalMemory=")) {
                    if let Some(memory_str) = memory_line.split('=').nth(1) {
                        if let Ok(memory_bytes) = memory_str.trim().parse::<u64>() {
                            println!("  Total RAM: {}", crate::utils::format_bytes(memory_bytes));
                        }
                    }
                }
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // Try to get memory info on Unix-like systems
            if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
                for line in meminfo.lines() {
                    if line.starts_with("MemTotal:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<u64>() {
                                println!("  Total RAM: {}", crate::utils::format_bytes(kb * 1024));
                            }
                        }
                        break;
                    }
                }
            }
        }
        
        println!();
    }
    
    // Network info
    if network {
        println!("{} Network Information:", "🌐".blue().bold());
        
        // Get hostname
        if let Ok(hostname) = std::env::var("COMPUTERNAME") {
            println!("  Hostname: {}", hostname);
        } else if let Ok(hostname) = std::env::var("HOSTNAME") {
            println!("  Hostname: {}", hostname);
        }
        
        // Try to get IP addresses (basic)
        println!("  (Network interface details require additional permissions)");
        println!();
    }
    
    // Jamey-specific info
    println!("{} Jamey Configuration:", "🤖".blue().bold());
    let config = CliConfig::load().unwrap_or_default();
    println!("  Default Model: {}", config.default_model);
    println!("  Runtime URL: {}", config.runtime_url);
    println!("  API Key: {}", if config.api_key.is_some() { "✓ Configured" } else { "✗ Not configured" });
    println!();
    
    Ok(())
}

/// Check system health
async fn check_system_health(comprehensive: bool) -> Result<()> {
    println!("{} System Health Check", "🏥".cyan().bold());
    println!("{}", "═".repeat(50));
    println!();
    
    let mut all_healthy = true;
    
    // Check configuration
    println!("{} Configuration:", "⚙️".blue());
    match CliConfig::load() {
        Ok(config) => {
            println!("  {} Config loaded successfully", "✓".green());
            if config.api_key.is_none() {
                println!("  {} API key not set (use JAMEY_API_KEY or OPENROUTER_API_KEY)", "⚠".yellow());
            } else {
                println!("  {} API key configured", "✓".green());
            }
        }
        Err(e) => {
            println!("  {} Config error: {}", "✗".red(), e);
            all_healthy = false;
        }
    }
    println!();
    
    // Check runtime initialization
    println!("{} Runtime:", "🚀".blue());
    let start = Instant::now();
    match RuntimeConfig::from_env() {
        Ok(config) => {
            match Runtime::new(config).await {
                Ok(runtime) => {
                    let init_time = start.elapsed();
                    println!("  {} Runtime initialized ({}ms)", "✓".green(), init_time.as_millis());
                    
                    let state = runtime.state();
                    
                    // Check memory store
                    if comprehensive {
                        println!("  {} Checking memory store...", "⏳".yellow());
                        // Try a simple operation to verify connection
                        let vector_dim = state.config.memory.vector_dimension;
                        let test_embedding = vec![0.0; vector_dim];
                        match state.memory_store.search(test_embedding, 1).await {
                            Ok(_) => {
                                println!("  {} Memory store accessible", "✓".green());
                            }
                            Err(e) => {
                                println!("  {} Memory store error: {}", "✗".red(), e);
                                all_healthy = false;
                            }
                        }
                    }
                    
                    // Check LLM provider
                    if comprehensive {
                        println!("  {} Checking LLM provider...", "⏳".yellow());
                        // We can't easily test without making an API call, so just check config
                        if state.config.llm.openrouter_api_key.is_empty() {
                            println!("  {} LLM API key not configured", "✗".red());
                            all_healthy = false;
                        } else {
                            println!("  {} LLM provider configured", "✓".green());
                        }
                    }
                }
                Err(e) => {
                    println!("  {} Runtime initialization failed: {}", "✗".red(), e);
                    all_healthy = false;
                }
            }
        }
        Err(e) => {
            println!("  {} Config load failed: {}", "✗".red(), e);
            all_healthy = false;
        }
    }
    println!();
    
    // Overall status
    if all_healthy {
        println!("{} Overall Status: {}", "✅".green().bold(), "HEALTHY".green().bold());
    } else {
        println!("{} Overall Status: {}", "❌".red().bold(), "UNHEALTHY".red().bold());
        println!("{} Some components are not functioning correctly.", "⚠️".yellow());
    }
    
    Ok(())
}

/// Run configuration action
async fn run_config_action(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show => {
            println!("{} Current Configuration", "⚙️".cyan().bold());
            println!("{}", "═".repeat(50));
            println!();
            
            let config = CliConfig::load()?;
            println!("{} CLI Configuration:", "📋".blue().bold());
            println!("  Default Model: {}", config.default_model);
            println!("  Runtime URL: {}", config.runtime_url);
            println!("  Timeout: {} seconds", config.timeout_seconds);
            println!("  Verbose: {}", config.verbose);
            println!("  API Key: {}", if config.api_key.is_some() { "✓ Configured (from environment)" } else { "✗ Not configured" });
            println!();
            
            // Show config file path
            let config_path = home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
                .join("jamey")
                .join("cli.toml");
            println!("{} Config File: {}", "📁".blue(), config_path.display());
        }
        ConfigAction::Set { key, value } => {
            let mut config = CliConfig::load().unwrap_or_default();
            
            match key.to_lowercase().as_str() {
                "default_model" | "model" => {
                    config.default_model = value;
                    println!("{} Set default_model to: {}", "✓".green(), config.default_model);
                }
                "runtime_url" | "url" => {
                    config.runtime_url = value;
                    println!("{} Set runtime_url to: {}", "✓".green(), config.runtime_url);
                }
                "timeout" | "timeout_seconds" => {
                    config.timeout_seconds = value.parse()
                        .with_context(|| format!("Invalid timeout value: {}", value))?;
                    println!("{} Set timeout_seconds to: {}", "✓".green(), config.timeout_seconds);
                }
                "verbose" => {
                    config.verbose = value.parse()
                        .with_context(|| format!("Invalid verbose value: {}. Must be 'true' or 'false'", value))?;
                    println!("{} Set verbose to: {}", "✓".green(), config.verbose);
                }
                "api_key" => {
                    return Err(anyhow::anyhow!("API keys cannot be set via config command. Use environment variables (JAMEY_API_KEY or OPENROUTER_API_KEY) instead."));
                }
                _ => {
                    return Err(anyhow::anyhow!("Unknown config key: {}. Valid keys: default_model, runtime_url, timeout_seconds, verbose", key));
                }
            }
            
            config.save()?;
            println!("{} Configuration saved.", "✅".green());
        }
        ConfigAction::Get { key } => {
            let config = CliConfig::load()?;
            
            match key.to_lowercase().as_str() {
                "default_model" | "model" => {
                    println!("{}", config.default_model);
                }
                "runtime_url" | "url" => {
                    println!("{}", config.runtime_url);
                }
                "timeout" | "timeout_seconds" => {
                    println!("{}", config.timeout_seconds);
                }
                "verbose" => {
                    println!("{}", config.verbose);
                }
                "api_key" => {
                    if config.api_key.is_some() {
                        println!("***REDACTED*** (API keys cannot be displayed for security)");
                    } else {
                        return Err(anyhow::anyhow!("API key not configured. Use environment variables (JAMEY_API_KEY or OPENROUTER_API_KEY)"));
                    }
                }
                _ => {
                    return Err(anyhow::anyhow!("Unknown config key: {}. Valid keys: default_model, runtime_url, timeout_seconds, verbose, api_key", key));
                }
            }
        }
        ConfigAction::Reset { force } => {
            if !force {
                let confirmed = crate::utils::confirm(
                    "Are you sure you want to reset configuration to defaults? This will overwrite your current settings."
                )?;
                
                if !confirmed {
                    println!("{} Reset cancelled.", "ℹ️".blue());
                    return Ok(());
                }
            }
            
            let default_config = CliConfig::default();
            default_config.save()?;
            println!("{} Configuration reset to defaults.", "✅".green());
        }
    }
    
    Ok(())
}

/// Show system logs
async fn show_logs(lines: usize, follow: bool, level: Option<String>) -> Result<()> {
    // Find log file (common locations)
    let log_paths = vec![
        home_dir().map(|h| h.join(".local").join("share").join("jamey").join("logs").join("jamey.log")),
        Some(PathBuf::from("./logs/jamey.log")),
        Some(PathBuf::from("./jamey.log")),
    ];
    
    let log_path = log_paths.iter()
        .flatten()
        .find(|p| p.exists())
        .ok_or_else(|| anyhow::anyhow!("Log file not found. Checked: ~/.local/share/jamey/logs/jamey.log, ./logs/jamey.log, ./jamey.log"))?;
    
    println!("{} System Logs", "📋".cyan().bold());
    println!("{}", "═".repeat(50));
    println!("{} Reading from: {}", "📁".blue(), log_path.display());
    println!();
    
    if follow {
        println!("{} Following log (press Ctrl+C to stop)...", "👀".yellow());
        println!();
        
        // Simple follow implementation - read file and watch for changes
        use std::io::{BufRead, BufReader};
        use std::fs::File;
        
        let file = File::open(log_path)?;
        let reader = BufReader::new(file);
        
        // Read last N lines
        let all_lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;
        let start_idx = all_lines.len().saturating_sub(lines);
        
        for line in &all_lines[start_idx..] {
            if should_show_line(line, &level) {
                println!("{}", line);
            }
        }
        
        // Note: Full tail -f functionality would require file watching
        // For now, just show the last N lines
        println!();
        println!("{} (Full tail -f not yet implemented. Showing last {} lines)", "ℹ️".blue(), lines);
    } else {
        // Read last N lines
        let content = fs::read_to_string(log_path)
            .with_context(|| format!("Failed to read log file: {}", log_path.display()))?;
        
        let all_lines: Vec<&str> = content.lines().collect();
        let start_idx = all_lines.len().saturating_sub(lines);
        
        println!("{} Last {} lines:", "📄".blue(), lines.min(all_lines.len()));
        println!();
        
        for line in &all_lines[start_idx..] {
            if should_show_line(line, &level) {
                println!("{}", line);
            }
        }
    }
    
    Ok(())
}

/// Check if a log line should be shown based on level filter
fn should_show_line(line: &str, level_filter: &Option<String>) -> bool {
    if let Some(level) = level_filter {
        let level_upper = level.to_uppercase();
        line.to_uppercase().contains(&level_upper)
    } else {
        true
    }
}