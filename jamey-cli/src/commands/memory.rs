//! Memory management commands
//! 
//! Interface for managing Jamey's memory and knowledge base

use anyhow::{Context, Result};
use colored::*;
use crate::commands::MemoryAction;
use jamey_core::memory::{Memory, MemoryType};
use jamey_runtime::{Runtime, RuntimeConfig};
use uuid::Uuid;
use tracing::{info, error, debug};
use std::path::PathBuf;
use std::fs::File;
use std::io::Write;

/// Run memory management action
pub async fn run_memory_action(action: MemoryAction) -> Result<()> {
    match action {
        MemoryAction::Search { query, limit, type_filter } => {
            search_memory(query, limit, type_filter).await
        }
        MemoryAction::List { count, detailed } => {
            list_memory(count, detailed).await
        }
        MemoryAction::Delete { id, force } => {
            delete_memory(id, force).await
        }
        MemoryAction::Export { output, format } => {
            export_memory(output, format).await
        }
    }
}

/// Search memory entries
async fn search_memory(query: String, limit: usize, type_filter: Option<String>) -> Result<()> {
    // Validate input length to prevent DoS
    crate::utils::validate_input_length(&query, 1000, "Search query")?;
    
    // Validate limit (prevent excessive memory usage)
    if limit > 1000 {
        return Err(anyhow::anyhow!("Search limit cannot exceed 1000 (got {})", limit));
    }
    
    println!("{} Searching memory for: {}", "🔍".cyan().bold(), query);
    
    // Initialize runtime to access memory store and LLM provider
    let config = load_runtime_config().await?;
    let runtime = Runtime::new(config).await
        .context("Failed to initialize runtime for memory search")?;
    let state = runtime.state();
    
    // Generate embedding for the query
    print!("{} Generating embedding... ", "⏳".yellow());
    std::io::stdout().flush()?;
    
    let query_embedding = state.llm_provider.get_embedding(&query).await
        .with_context(|| "Failed to generate embedding for search query")?;
    
    println!("{}", "✓".green());
    
    // Search memory store
    print!("{} Searching memory store... ", "⏳".yellow());
    std::io::stdout().flush()?;
    
    let memories = state.memory_store.search(query_embedding, limit).await
        .with_context(|| "Failed to search memory store")?;
    
    println!("{}", "✓".green());
    println!();
    
    // Filter by type if specified
    let filtered_memories: Vec<&Memory> = if let Some(filter_type) = type_filter {
        let target_type = parse_memory_type(&filter_type)?;
        memories.iter()
            .filter(|m| {
                std::mem::discriminant(&m.memory_type) == std::mem::discriminant(&target_type)
            })
            .collect()
    } else {
        memories.iter().collect()
    };
    
    // Display results
    if filtered_memories.is_empty() {
        println!("{} No memories found matching your query.", "📝".blue());
    } else {
        println!("{} Found {} result(s):", "📝".blue().bold(), filtered_memories.len());
        println!();
        
        for (i, memory) in filtered_memories.iter().enumerate() {
            println!("{} Result {}:", "─".repeat(50).cyan(), (i + 1).to_string().cyan().bold());
            println!("  {} ID: {}", "🆔".blue(), memory.id);
            println!("  {} Type: {}", "📋".blue(), memory.memory_type);
            println!("  {} Content: {}", "💬".blue(), 
                if memory.content.len() > 200 {
                    format!("{}...", &memory.content[..200])
                } else {
                    memory.content.clone()
                });
            println!("  {} Created: {}", "📅".blue(), memory.created_at.format("%Y-%m-%d %H:%M:%S"));
            println!("  {} Last Accessed: {}", "🕐".blue(), memory.last_accessed.format("%Y-%m-%d %H:%M:%S"));
            println!();
        }
    }
    
    Ok(())
}

/// Parse memory type from string
fn parse_memory_type(s: &str) -> Result<MemoryType> {
    match s.to_lowercase().as_str() {
        "conversation" => Ok(MemoryType::Conversation),
        "knowledge" => Ok(MemoryType::Knowledge),
        "experience" => Ok(MemoryType::Experience),
        "skill" => Ok(MemoryType::Skill),
        "preference" => Ok(MemoryType::Preference),
        _ => Err(anyhow::anyhow!("Invalid memory type: {}. Must be one of: conversation, knowledge, experience, skill, preference", s)),
    }
}

/// Load runtime configuration for memory operations
async fn load_runtime_config() -> Result<RuntimeConfig> {
    RuntimeConfig::from_env()
        .map_err(|e| anyhow::anyhow!("Failed to load runtime config: {}", e))
}

/// List recent memory entries
async fn list_memory(count: usize, detailed: bool) -> Result<()> {
    // Validate count
    if count > 1000 {
        return Err(anyhow::anyhow!("List count cannot exceed 1000 (got {})", count));
    }
    
    println!("{} Recent Memory Entries ({}):", "📚".cyan().bold(), count);
    
    // Initialize runtime
    let config = load_runtime_config().await?;
    let runtime = Runtime::new(config).await
        .context("Failed to initialize runtime for memory listing")?;
    let state = runtime.state();
    
    // Use a generic search with a zero vector to get recent memories
    // This is a workaround - ideally we'd have a list_recent() method
    print!("{} Loading memories... ", "⏳".yellow());
    std::io::stdout().flush()?;
    
    // Create a generic embedding (all zeros) to get all memories, then sort by date
    let vector_dim = state.config.memory.vector_dimension;
    let generic_embedding = vec![0.0; vector_dim];
    
    // Get more than needed, then we'll sort and limit
    let all_memories = state.memory_store.search(generic_embedding, count * 2).await
        .with_context(|| "Failed to retrieve memories")?;
    
    // Sort by created_at (most recent first) and limit
    let mut sorted_memories = all_memories;
    sorted_memories.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    let recent_memories: Vec<Memory> = sorted_memories.into_iter().take(count).collect();
    
    println!("{}", "✓".green());
    println!();
    
    if recent_memories.is_empty() {
        println!("{} No memories found.", "📝".blue());
    } else {
        for (i, memory) in recent_memories.iter().enumerate() {
            if detailed {
                println!("{} Memory {}:", "─".repeat(50).cyan(), (i + 1).to_string().cyan().bold());
                println!("  {} ID: {}", "🆔".blue(), memory.id);
                println!("  {} Type: {}", "📋".blue(), memory.memory_type);
                println!("  {} Content: {}", "💬".blue(), memory.content);
                println!("  {} Created: {}", "📅".blue(), memory.created_at.format("%Y-%m-%d %H:%M:%S"));
                println!("  {} Last Accessed: {}", "🕐".blue(), memory.last_accessed.format("%Y-%m-%d %H:%M:%S"));
                if !memory.metadata.is_null() {
                    if let Some(obj) = memory.metadata.as_object() {
                        if !obj.is_empty() {
                            println!("  {} Metadata: {}", "📊".blue(), serde_json::to_string_pretty(&memory.metadata)?);
                        }
                    }
                }
                println!();
            } else {
                println!("  {} {} | {} | {} | {}", 
                    (i + 1).to_string().dim(),
                    memory.id.to_string()[..8].cyan(),
                    format!("{:?}", memory.memory_type).yellow(),
                    memory.created_at.format("%Y-%m-%d %H:%M").to_string().dim(),
                    if memory.content.len() > 50 {
                        format!("{}...", &memory.content[..50])
                    } else {
                        memory.content.clone()
                    }
                );
            }
        }
        
        if !detailed {
            println!();
        }
    }
    
    Ok(())
}

/// Delete memory entry
async fn delete_memory(id: String, force: bool) -> Result<()> {
    // Validate UUID format
    let memory_id = crate::utils::validate_uuid(&id)
        .with_context(|| format!("Invalid memory ID format: {}", id))?;
    
    // Initialize runtime
    let config = load_runtime_config().await?;
    let runtime = Runtime::new(config).await
        .context("Failed to initialize runtime for memory deletion")?;
    let state = runtime.state();
    
    // Check if memory exists
    match state.memory_store.retrieve(memory_id).await {
        Ok(memory) => {
            println!("{} Deleting memory entry: {}", "🗑️".red().bold(), id);
            println!("  Type: {}", memory.memory_type);
            println!("  Content: {}", 
                if memory.content.len() > 100 {
                    format!("{}...", &memory.content[..100])
                } else {
                    memory.content.clone()
                }
            );
        }
        Err(_) => {
            return Err(anyhow::anyhow!("Memory not found: {}", id));
        }
    }
    
    if !force {
        // Require confirmation for destructive operations
        let confirmed = crate::utils::confirm(
            &format!("Are you sure you want to delete memory entry {}? This action cannot be undone.", id)
        )?;
        
        if !confirmed {
            println!("{} Deletion cancelled.", "ℹ️".blue());
            return Ok(());
        }
    }
    
    // Delete the memory
    state.memory_store.delete(memory_id).await
        .with_context(|| format!("Failed to delete memory: {}", id))?;
    
    println!("{} Memory entry {} deleted successfully.", "✅".green(), id);
    
    Ok(())
}

/// Export memory to file
async fn export_memory(output: PathBuf, format: String) -> Result<()> {
    // Validate path to prevent directory traversal
    crate::utils::validate_path(&output)?;
    
    // Validate format
    if !matches!(format.as_str(), "json" | "csv") {
        return Err(anyhow::anyhow!("Invalid export format: {}. Must be 'json' or 'csv'", format));
    }
    
    println!("{} Exporting memory to: {} ({})", "📤".cyan().bold(), output.display(), format);
    
    // Initialize runtime
    let config = load_runtime_config().await?;
    let runtime = Runtime::new(config).await
        .context("Failed to initialize runtime for memory export")?;
    let state = runtime.state();
    
    // Get all memories (using generic search)
    print!("{} Loading all memories... ", "⏳".yellow());
    std::io::stdout().flush()?;
    
    let vector_dim = state.config.memory.vector_dimension;
    let generic_embedding = vec![0.0; vector_dim];
    
    // Get a large number of memories (adjust limit as needed)
    let memories = state.memory_store.search(generic_embedding, 10000).await
        .with_context(|| "Failed to retrieve memories for export")?;
    
    println!("{} Found {} memories", "✓".green(), memories.len());
    
    // Create output file
    let mut file = File::create(&output)
        .with_context(|| format!("Failed to create output file: {}", output.display()))?;
    
    // Export based on format
    match format.as_str() {
        "json" => {
            let json_data = serde_json::to_string_pretty(&memories)
                .context("Failed to serialize memories to JSON")?;
            file.write_all(json_data.as_bytes())
                .with_context(|| format!("Failed to write to file: {}", output.display()))?;
        }
        "csv" => {
            // Write CSV header
            writeln!(file, "id,memory_type,content,created_at,last_accessed,metadata")
                .with_context(|| format!("Failed to write CSV header: {}", output.display()))?;
            
            // Write CSV rows
            for memory in &memories {
                let metadata_str = serde_json::to_string(&memory.metadata)
                    .unwrap_or_else(|_| "{}".to_string());
                // Escape quotes in content and metadata
                let content_escaped = memory.content.replace('"', "\"\"");
                let metadata_escaped = metadata_str.replace('"', "\"\"");
                
                writeln!(file, "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"",
                    memory.id,
                    memory.memory_type,
                    content_escaped,
                    memory.created_at.format("%Y-%m-%d %H:%M:%S"),
                    memory.last_accessed.format("%Y-%m-%d %H:%M:%S"),
                    metadata_escaped
                ).with_context(|| format!("Failed to write CSV row: {}", output.display()))?;
            }
        }
        _ => unreachable!(), // Already validated above
    }
    
    println!("{} Successfully exported {} memories to {}", 
        "✅".green(), 
        memories.len(), 
        output.display()
    );
    
    Ok(())
}