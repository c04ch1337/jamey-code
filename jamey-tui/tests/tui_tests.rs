mod helpers;
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use jamey_runtime::{Runtime, RuntimeConfig};
use jamey_tui::{app::App, ui::UiState};
use std::sync::Arc;
use tempfile::TempDir;
use tokio;

async fn setup_test_environment() -> Result<(App, TempDir)> {
    let temp_dir = TempDir::new()?;
    let mut config = RuntimeConfig::default();
    
    config.project_name = "tui_test".to_string();
    // Test secrets - do not use in production
    config.memory.postgres_password = "test_password".to_string();
    config.llm.openrouter_api_key = "test_key".to_string();
    config.tools.backup_dir = temp_dir.path().to_path_buf();
    config.security.api_key_required = false;

    let runtime = Runtime::new(config).await?;
    let app = App::new(runtime).await?;
    
    Ok((app, temp_dir))
}

#[tokio::test]
async fn test_tui_initialization() -> Result<()> {
    let (app, _temp_dir) = setup_test_environment().await?;
    
    // Verify initial state
    assert_eq!(app.state(), UiState::Normal);
    assert!(app.chat_history().is_empty());
    assert!(!app.is_input_mode());
    
    Ok(())
}

#[tokio::test]
async fn test_tui_input_handling() -> Result<()> {
    let (mut app, _temp_dir) = setup_test_environment().await?;
    
    // Test entering input mode
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)))?;
    assert!(app.is_input_mode());
    
    // Test typing input
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE)))?;
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)))?;
    assert_eq!(app.input_buffer(), "hi");
    
    // Test exiting input mode
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)))?;
    assert!(!app.is_input_mode());
    
    Ok(())
}

#[tokio::test]
async fn test_tui_chat_interaction() -> Result<()> {
    let (mut app, _temp_dir) = setup_test_environment().await?;
    
    // Enter input mode and type message
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)))?;
    for c in "Hello, test message".chars() {
        app.handle_event(Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)))?;
    }
    
    // Send message
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)))?;
    
    // Wait for response
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    
    // Verify chat history updated
    let history = app.chat_history();
    assert!(!history.is_empty());
    assert!(history.iter().any(|msg| msg.content.contains("Hello, test message")));
    
    Ok(())
}

#[tokio::test]
async fn test_tui_navigation() -> Result<()> {
    let (mut app, _temp_dir) = setup_test_environment().await?;
    
    // Test switching to memory view
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::CONTROL)))?;
    assert_eq!(app.state(), UiState::Memory);
    
    // Test switching to system view
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)))?;
    assert_eq!(app.state(), UiState::System);
    
    // Test returning to normal view
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL)))?;
    assert_eq!(app.state(), UiState::Normal);
    
    Ok(())
}

#[tokio::test]
async fn test_tui_memory_view() -> Result<()> {
    let (mut app, _temp_dir) = setup_test_environment().await?;
    
    // Switch to memory view
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::CONTROL)))?;
    
    // Create test memory
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL)))?;
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)))?;
    for c in "Test memory content".chars() {
        app.handle_event(Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)))?;
    }
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)))?;
    
    // Wait for memory to be stored
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    
    // Verify memory appears in list
    let memories = app.memory_list();
    assert!(memories.iter().any(|m| m.content.contains("Test memory content")));
    
    Ok(())
}

#[tokio::test]
async fn test_tui_system_monitoring() -> Result<()> {
    let (mut app, _temp_dir) = setup_test_environment().await?;
    
    // Switch to system view
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)))?;
    
    // Wait for system info to update
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    
    // Verify system info is displayed
    let system_info = app.system_info();
    assert!(system_info.contains("CPU"));
    assert!(system_info.contains("Memory"));
    
    Ok(())
}

#[tokio::test]
async fn test_tui_error_handling() -> Result<()> {
    let (mut app, _temp_dir) = setup_test_environment().await?;
    
    // Test invalid key combination
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)))?;
    assert!(!app.has_error());
    
    // Test sending empty message
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)))?;
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)))?;
    assert!(app.has_error());
    
    Ok(())
}

#[tokio::test]
async fn test_tui_shutdown() -> Result<()> {
    let (mut app, _temp_dir) = setup_test_environment().await?;
    
    // Trigger shutdown
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL)))?;
    assert!(app.should_quit());
    
    // Verify cleanup
    assert!(app.chat_history().is_empty());
    assert!(!app.is_input_mode());
    
    Ok(())
}

#[tokio::test]
async fn test_tui_resize_handling() -> Result<()> {
    let (mut app, _temp_dir) = setup_test_environment().await?;
    
    // Test window resize event
    app.handle_event(Event::Resize(100, 50))?;
    
    // Verify UI adjusted
    assert_eq!(app.terminal_size(), (100, 50));
    
    Ok(())
}