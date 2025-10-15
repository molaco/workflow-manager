//! Chat view rendering with two-pane layout

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::chat::{self, ActivePane};
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

    // STEP 1: Split horizontally into left (chat) and right (logs)
    let horizontal = Layout::horizontal([
        Constraint::Percentage(60), // Chat pane (60%)
        Constraint::Percentage(40), // Logs pane (40%)
    ]);
    let [left_area, right_area] = horizontal.areas(area);

    // STEP 2: Split left pane vertically into messages + input
    let vertical = Layout::vertical([
        Constraint::Min(0),     // Messages (fill available space)
        Constraint::Length(3),  // Input box (3 lines)
    ]);
    let [messages_area, input_area] = vertical.areas(left_area);

    // === RENDER CHAT MESSAGES ===
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

        // Show tool calls (simplified - details in logs pane)
        if !msg.tool_calls.is_empty() {
            message_lines.push(Line::from(vec![
                Span::styled(
                    format!("  🔧 {} tool(s) used (see logs →)", msg.tool_calls.len()),
                    Style::default().fg(Color::Yellow),
                ),
            ]));
        }

        message_lines.push(Line::from(""));
    }

    if chat.waiting_for_response {
        // Animated loading indicator like Claude Code
        let spinner = chat.get_spinner_char();
        let elapsed = chat.get_elapsed_seconds().unwrap_or(0);

        message_lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", spinner),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Thinking",
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                "…",
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        message_lines.push(Line::from(vec![
            Span::styled(
                "  (esc to interrupt",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!(" · {}s)", elapsed),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    // Highlight active pane border
    let chat_border_style = if matches!(chat.active_pane, ActivePane::ChatMessages) {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let messages_widget = Paragraph::new(message_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Chat with Claude ")
                .border_style(chat_border_style),
        )
        .wrap(Wrap { trim: false })
        .scroll((chat.message_scroll, 0));

    f.render_widget(messages_widget, messages_area);

    // === RENDER INPUT BOX ===
    let input_widget = Paragraph::new(chat.input_buffer.as_str()).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Type your message (Enter to send, Tab to switch pane) ")
            .style(Style::default().fg(Color::White)),
    );

    f.render_widget(input_widget, input_area);

    // === RENDER TOOL CALL LOGS ===
    let log_border_style = if matches!(chat.active_pane, ActivePane::Logs) {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // Build log lines from tool calls in message history
    let mut log_lines = Vec::new();

    for (msg_idx, msg) in chat.messages.iter().enumerate() {
        if !msg.tool_calls.is_empty() {
            // Message context header
            let context = match msg.role {
                chat::ChatRole::User => format!("After user message #{}", msg_idx + 1),
                chat::ChatRole::Assistant => format!("Claude's response #{}", msg_idx + 1),
            };
            log_lines.push(Line::from(vec![
                Span::styled(
                    format!("═══ {} ═══", context),
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            log_lines.push(Line::from(""));

            // Show each tool call with details
            for (tool_idx, tool_call) in msg.tool_calls.iter().enumerate() {
                log_lines.push(Line::from(vec![
                    Span::styled(
                        format!("🔧 Tool #{}: ", tool_idx + 1),
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(&tool_call.name, Style::default().fg(Color::Cyan)),
                ]));

                // Input (parameters)
                if !tool_call.input.is_empty() {
                    log_lines.push(Line::from(vec![
                        Span::styled("  Input: ", Style::default().fg(Color::Gray)),
                        Span::raw(&tool_call.input),
                    ]));
                }

                // Output (result)
                if !tool_call.output.is_empty() {
                    log_lines.push(Line::from(Span::styled(
                        "  Output:",
                        Style::default().fg(Color::Gray),
                    )));

                    // Split output into lines and indent
                    for line in tool_call.output.lines().take(50) {
                        log_lines.push(Line::from(format!("    {}", line)));
                    }

                    // Truncate if too long
                    if tool_call.output.lines().count() > 50 {
                        log_lines.push(Line::from(Span::styled(
                            "    ... (truncated)",
                            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                        )));
                    }
                }

                log_lines.push(Line::from(""));
            }
        }
    }

    if log_lines.is_empty() {
        log_lines.push(Line::from(Span::styled(
            "No tool calls yet...",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )));
        log_lines.push(Line::from(""));
        log_lines.push(Line::from(Span::styled(
            "Tool calls will appear here when Claude uses workflows.",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let logs_widget = Paragraph::new(log_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Tool Call Logs ")
                .border_style(log_border_style),
        )
        .wrap(Wrap { trim: false })
        .scroll((chat.log_scroll, 0));

    f.render_widget(logs_widget, right_area);
}
