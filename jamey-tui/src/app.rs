//! Application state and logic for the TUI

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use jamey_protocol::{Message, Role};
use std::time::Instant;
use tui_textarea::TextArea;
use uuid::Uuid;

pub struct App {
    pub should_exit: bool,
    pub messages: Vec<Message>,
    pub input: TextArea<'static>,
    pub status: String,
    pub session_id: Uuid,
    pub last_update: Instant,
}

impl App {
    pub async fn new() -> Result<Self> {
        let session_id = Uuid::new_v4();
        
        Ok(Self {
            should_exit: false,
            messages: vec![
                Message::system("Welcome to Digital Twin Jamey TUI!".to_string()),
                Message::assistant("Hello! I'm Jamey, your digital twin assistant. How can I help you today?".to_string()),
            ],
            input: TextArea::default(),
            status: "Ready".to_string(),
            session_id,
            last_update: Instant::now(),
        })
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                self.should_exit = true;
            }
            KeyCode::Enter => {
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                    self.send_message();
                }
            }
            _ => {
                // Handle text input
                self.input.input(key);
            }
        }
    }

    pub async fn update(&mut self) -> Result<()> {
        // Update status based on time
        let elapsed = self.last_update.elapsed();
        if elapsed.as_secs() % 10 == 0 {
            self.status = format!("Session: {} | Uptime: {}s", 
                self.session_id.to_string()[..8].to_uppercase(),
                elapsed.as_secs());
        }
        
        Ok(())
    }

    fn send_message(&mut self) {
        let input_text = self.input.lines().join(" ").trim().to_string();
        
        if input_text.is_empty() {
            return;
        }

        // Add user message
        let user_message = Message::user(input_text.clone());
        self.messages.push(user_message);

        // Clear input
        self.input = TextArea::default();

        // Simulate response (in real implementation, this would call the runtime)
        let response = Message::assistant(format!("I received your message: {}", input_text));
        self.messages.push(response);

        self.status = "Message sent".to_string();
    }
}