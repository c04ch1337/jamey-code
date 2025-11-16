//! Process management commands
//! 
//! Interface for system process management through Jamey

use anyhow::Result;
use colored::*;
use crate::commands::ProcessAction;
use jamey_tools::system::ProcessTool;
use tracing::{info, error};

/// Run process management action
pub async fn run_process_action(action: ProcessAction) -> Result<()> {
    match action {
        ProcessAction::List { filter, detailed } => {
            list_processes(filter, detailed).await
        }
        ProcessAction::Info { pid } => {
            show_process_info(pid).await
        }
        ProcessAction::Kill { pid, force } => {
            kill_process(pid, force).await
        }
    }
}

/// List running processes
async fn list_processes(filter: Option<String>, detailed: bool) -> Result<()> {
    println!("{}", "🔍 Running Processes:".cyan().bold());
    
    let mut tool = ProcessTool::new();
    let processes = tool.list_processes();
    
    for process in processes {
        // Apply filter if specified
        if let Some(ref filter_str) = filter {
            if !process.name.to_lowercase().contains(&filter_str.to_lowercase()) {
                continue;
            }
        }
        
        if detailed {
            println!("{} PID: {} | CPU: {:.1}% | Memory: {} KB", 
                "📋".blue(), process.pid, process.cpu_usage, process.memory_usage);
            println!("   Name: {}", process.name);
            println!("   Started: {}", process.start_time.format("%Y-%m-%d %H:%M:%S"));
            println!();
        } else {
            println!("{} {} | {} | {:.1}%", 
                "▪".blue(), process.pid, process.name, process.cpu_usage);
        }
    }
    
    Ok(())
}

/// Show detailed process information
async fn show_process_info(pid: u32) -> Result<()> {
    println!("{} Process Information for PID: {}", "📊".cyan().bold(), pid);
    
    let mut tool = ProcessTool::new();
    match tool.get_process_info(pid) {
        Ok(info) => {
            println!("{} PID: {}", "🆔".blue(), info.pid);
            println!("{} Name: {}", "📝".blue(), info.name);
            println!("{} CPU Usage: {:.2}%", "⚡".blue(), info.cpu_usage);
            println!("{} Memory Usage: {} KB", "💾".blue(), info.memory_usage);
            println!("{} Start Time: {}", "⏰".blue(), info.start_time.format("%Y-%m-%d %H:%M:%S"));
        }
        Err(e) => {
            error!("Failed to get process info: {}", e);
            println!("{} {}", "❌".red(), "Process not found or access denied.");
        }
    }
    
    Ok(())
}

/// Kill a process
async fn kill_process(pid: u32, force: bool) -> Result<()> {
    println!("{} {} process PID: {}", 
        "⚠️".yellow(), 
        if force { "Force killing" } else { "Terminating" }, 
        pid);
    
    let mut tool = ProcessTool::new();
    match tool.kill_process(pid) {
        Ok(_) => {
            println!("{} Process {} terminated successfully.", "✅".green(), pid);
        }
        Err(e) => {
            error!("Failed to kill process: {}", e);
            println!("{} {}", "❌".red(), "Failed to terminate process. Check permissions.");
        }
    }
    
    Ok(())
}