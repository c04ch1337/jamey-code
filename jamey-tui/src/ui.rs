//! UI rendering logic for the TUI

use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Margin},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::App;

pub fn draw<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),     // Messages area
            Constraint::Length(3),  // Input area
            Constraint::Length(1),  // Status bar
        ])
        .split(f.size());

    draw_messages(f, app, chunks[0]);
    draw_input(f, app, chunks[1]);
    draw_status(f, app, chunks[2]);
}

fn draw_messages<B: Backend>(f: &mut Frame<B>, app: &mut App, area: ratatui::layout::Rect) {
    let messages: Vec<ListItem> = app
        .messages
        .iter()
        .map(|msg| {
            let (prefix, style) = match msg.role {
                jamey_protocol::Role::System => ("🔧", Style::default().fg(Color::Yellow)),
                jamey_protocol::Role::User => ("👤", Style::default().fg(Color::Green)),
                jamey_protocol::Role::Assistant => ("🤖", Style::default().fg(Color::Blue)),
                jamey_protocol::Role::Tool => ("🔧", Style::default().fg(Color::Magenta)),
            };

            let content = vec![Line::from(vec![
                Span::styled(format!("{} ", prefix), style),
                Span::styled(&msg.content, Style::default()),
            ])];

            ListItem::new(content)
        })
        .collect();

    let messages_list = List::new(messages)
        .block(Block::default().borders(Borders::ALL).title("Messages"))
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(messages_list, area);
}

fn draw_input<B: Backend>(f: &mut Frame<B>, app: &mut App, area: ratatui::layout::Rect) {
    let input_widget = Paragraph::new(app.input.lines())
        .block(Block::default().borders(Borders::ALL).title("Input (Ctrl+Enter to send)"))
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });

    f.render_widget(input_widget, area);
}

fn draw_status<B: Backend>(f: &mut Frame<B>, app: &mut App, area: ratatui::layout::Rect) {
    let status_text = vec![Line::from(vec![
        Span::styled("Status: ", Style::default().fg(Color::Gray)),
        Span::styled(&app.status, Style::default().fg(Color::Green)),
        Span::styled(" | Ctrl+C to exit", Style::default().fg(Color::Gray)),
    ])];

    let status_widget = Paragraph::new(status_text)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });

    f.render_widget(status_widget, area);
}