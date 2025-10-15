//! Chat view rendering

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::chat;
use crate::models::App;

pub fn render_chat(f: &mut Frame, area: Rect, app: &App) {
    let chat = match &app.chat {
        Some(c) => c,
        None => {
            let error = Paragraph::new("Chat unavailable - runtime initialization failed")
                .block(Block::default().borders(Borders::ALL).title(" Error "))
                .style(Style::default().fg(Color::Red));
            f.render_widget(error, area);
            return;
        }
    };

    // Split into messages area and input area
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Messages
            Constraint::Length(3), // Input box
        ])
        .split(area);

    // Render messages
    let mut message_lines = Vec::new();
    for msg in &chat.messages {
        let role_style = match msg.role {
            chat::ChatRole::User => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            chat::ChatRole::Assistant => Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        };
        let role_text = match msg.role {
            chat::ChatRole::User => "You",
            chat::ChatRole::Assistant => "Claude",
        };

        message_lines.push(Line::from(vec![Span::styled(
            format!("{}: ", role_text),
            role_style,
        )]));
        message_lines.push(Line::from(msg.content.clone()));

        // Show tool calls if any (but not the verbose output)
        for tool_call in &msg.tool_calls {
            message_lines.push(Line::from(vec![
                Span::styled("  🔧 [Tool Used] ", Style::default().fg(Color::Yellow)),
                Span::raw(&tool_call.name),
            ]));
        }

        message_lines.push(Line::from(""));
    }

    if chat.waiting_for_response {
        message_lines.push(Line::from(Span::styled(
            "Claude is thinking...",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::ITALIC),
        )));
    }

    let messages_widget = Paragraph::new(message_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Chat with Claude ")
                .style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false })
        .scroll((chat.scroll_offset as u16, 0));

    f.render_widget(messages_widget, chunks[0]);

    // Render input box
    let input_widget = Paragraph::new(chat.input_buffer.as_str()).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Type your message (Enter to send) ")
            .style(Style::default().fg(Color::White)),
    );

    f.render_widget(input_widget, chunks[1]);
}
