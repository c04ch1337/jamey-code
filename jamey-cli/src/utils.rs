//! CLI utility functions
//! 
//! Helper functions for the CLI application

use anyhow::{Context, Result};
use colored::*;
use std::io::{self, Write};
use std::path::PathBuf;
use uuid::Uuid;

/// Print a formatted header
pub fn print_header(title: &str) {
    println!("{}", "═".repeat(50));
    println!("{}", title.cyan().bold());
    println!("{}", "═".repeat(50));
}

/// Print a formatted separator
pub fn print_separator() {
    println!("{}", "─".repeat(50));
}

/// Prompt user for confirmation
/// 
/// Returns Ok(true) if user confirms, Ok(false) if user declines,
/// or Err if there's an I/O error reading input.
pub fn confirm(message: &str) -> anyhow::Result<bool> {
    print!("{} [y/N]: ", message.yellow());
    io::stdout()
        .flush()
        .context("Failed to flush stdout")?;
    
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("Failed to read input from stdin")?;
    
    Ok(matches!(input.trim().to_lowercase().as_str(), "y" | "yes"))
}

/// Format bytes to human readable size
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

/// Format duration to human readable time
pub fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else if seconds < 86400 {
        format!("{}h {}m {}s", seconds / 3600, (seconds % 3600) / 60, seconds % 60)
    } else {
        format!("{}d {}h {}m", seconds / 86400, (seconds % 86400) / 3600, (seconds % 3600) / 60)
    }
}

/// Validate and parse a UUID string
/// 
/// Returns an error if the string is not a valid UUID format
pub fn validate_uuid(uuid_str: &str) -> Result<Uuid> {
    Uuid::parse_str(uuid_str)
        .with_context(|| format!("Invalid UUID format: {}", uuid_str))
}

/// Validate a file path to prevent directory traversal attacks
/// 
/// Returns an error if the path contains dangerous components like ".."
pub fn validate_path(path: &PathBuf) -> Result<()> {
    // Check for directory traversal attempts
    for component in path.components() {
        if let std::path::Component::ParentDir = component {
            return Err(anyhow::anyhow!(
                "Path contains '..' component - directory traversal not allowed: {}",
                path.display()
            ));
        }
    }
    
    Ok(())
}

/// Validate a PID value
/// 
/// Returns an error if the PID is invalid (e.g., 0 or too large)
pub fn validate_pid(pid: u32) -> Result<()> {
    if pid == 0 {
        return Err(anyhow::anyhow!("Invalid PID: 0 (PID 0 is reserved)"));
    }
    
    // On most systems, PIDs are limited to 2^15 - 1 (32767) or 2^22 - 1 (4194303)
    // We'll use a reasonable upper bound
    if pid > 10_000_000 {
        return Err(anyhow::anyhow!("PID value too large: {}", pid));
    }
    
    Ok(())
}

/// Validate input string length
/// 
/// Returns an error if the string exceeds the maximum length
pub fn validate_input_length(input: &str, max_length: usize, field_name: &str) -> Result<()> {
    if input.len() > max_length {
        return Err(anyhow::anyhow!(
            "{} exceeds maximum length of {} characters (got {})",
            field_name,
            max_length,
            input.len()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3661), "1h 1m 1s");
    }
}