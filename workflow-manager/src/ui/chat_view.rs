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

/// Parse message content with basic markdown formatting into styled lines
fn format_message_content(content: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip completely empty lines but preserve them as blank lines
        if trimmed.is_empty() {
            lines.push(Line::from(""));
            continue;
        }

        // Detect list items (numbered or bulleted)
        let (is_list, indent_level, list_content) = if let Some(rest) = trimmed.strip_prefix("**") {
            // Bold headers (e.g., "**Workflow Management Tools:**")
            if let Some(end_idx) = rest.find("**") {
                let bold_text = &rest[..end_idx];
                let after = &rest[end_idx + 2..];

                let mut spans = vec![
                    Span::styled(
                        format!("  {}", bold_text),
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    ),
                ];

                if !after.is_empty() {
                    spans.push(Span::styled(
                        after.to_string(),
                        Style::default().fg(Color::White),
                    ));
                }

                lines.push(Line::from(spans));
                continue;
            }
            (false, 0, trimmed)
        } else if let Some(rest) = trimmed.strip_prefix(|c: char| c.is_ascii_digit()) {
            // Numbered list (e.g., "1. Item")
            if let Some(rest) = rest.strip_prefix(". ") {
                (true, 1, rest)
            } else {
                (false, 0, trimmed)
            }
        } else if let Some(rest) = trimmed.strip_prefix("- ") {
            // Bullet list
            (true, 1, rest)
        } else if let Some(rest) = trimmed.strip_prefix("* ") {
            // Asterisk list
            (true, 1, rest)
        } else {
            (false, 0, trimmed)
        };

        // Format the content with inline markdown
        let formatted_spans = if list_content.contains("**") {
            // Parse inline bold
            let mut spans = Vec::new();
            let mut remaining = list_content;
            let indent = "  ".repeat(indent_level);

            if is_list {
                spans.push(Span::raw(format!("{}• ", indent)));
            } else {
                spans.push(Span::raw(format!("{}", indent)));
            }

            while let Some(start) = remaining.find("**") {
                // Add text before bold
                if start > 0 {
                    spans.push(Span::raw(remaining[..start].to_string()));
                }

                // Find end of bold
                if let Some(end) = remaining[start + 2..].find("**") {
                    let bold_text = &remaining[start + 2..start + 2 + end];
                    spans.push(Span::styled(
                        bold_text.to_string(),
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    ));
                    remaining = &remaining[start + 2 + end + 2..];
                } else {
                    // No closing **, just add the rest
                    spans.push(Span::raw(remaining[start..].to_string()));
                    break;
                }
            }

            // Add any remaining text
            if !remaining.is_empty() {
                spans.push(Span::raw(remaining.to_string()));
            }

            spans
        } else {
            // No inline formatting
            let indent = "  ".repeat(indent_level);
            if is_list {
                vec![Span::raw(format!("{}• {}", indent, list_content))]
            } else {
                vec![Span::raw(format!("{}{}", indent, list_content))]
            }
        };

        lines.push(Line::from(formatted_spans));
    }

    lines
}

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

    // Show loading animation if not initialized
    if !chat.initialized && chat.init_error.is_none() {
        let spinner = chat.get_spinner_char();

        message_lines.push(Line::from(""));
        message_lines.push(Line::from(""));
        message_lines.push(Line::from(""));
        message_lines.push(Line::from(vec![
            Span::styled(
                format!("    {} ", spinner),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Initializing Claude...",
                Style::default().fg(Color::Yellow),
            ),
        ]));
        message_lines.push(Line::from(""));
        message_lines.push(Line::from(Span::styled(
            "    Please wait while we set up your AI assistant",
            Style::default().fg(Color::DarkGray),
        )));
    } else if let Some(error) = &chat.init_error {
        // Show error state
        message_lines.push(Line::from(""));
        message_lines.push(Line::from(""));
        message_lines.push(Line::from(Span::styled(
            "    ✗ Initialization Failed",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
        message_lines.push(Line::from(""));
        message_lines.push(Line::from(Span::styled(
            format!("    {}", error),
            Style::default().fg(Color::Red),
        )));
    } else {
        // Normal chat mode - show messages
        for msg in &chat.messages {
            let role_style = match msg.role {
                chat::ChatRole::User => Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
                chat::ChatRole::Assistant => Style::default()
                    .fg(Color::White)
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

            // Use formatted message content with markdown parsing
            let formatted_lines = format_message_content(&msg.content);
            message_lines.extend(formatted_lines);

            // Show tool calls (simplified - details in logs pane)
            if !msg.tool_calls.is_empty() {
                message_lines.push(Line::from(""));
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
                        .fg(Color::White)
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
    }

    // Determine title and border style
    let (chat_title, chat_border_style) = if !chat.initialized && chat.init_error.is_none() {
        // Loading state - animated dots
        let indicator = chat.get_loading_indicator();
        (
            format!(" Chat with Claude [{}] ", indicator),
            Style::default().fg(Color::Yellow),
        )
    } else if chat.init_error.is_some() {
        // Error state
        (
            " Chat with Claude [✗] ".to_string(),
            Style::default().fg(Color::Red),
        )
    } else if matches!(chat.active_pane, ActivePane::ChatMessages) {
        // Ready state - active pane
        (
            " Chat with Claude [✓] ".to_string(),
            Style::default().fg(Color::White),
        )
    } else {
        // Ready state - inactive pane
        (
            " Chat with Claude [✓] ".to_string(),
            Style::default().fg(Color::DarkGray),
        )
    };

    let messages_widget = Paragraph::new(message_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(chat_title)
                .border_style(chat_border_style),
        )
        .wrap(Wrap { trim: false })
        .scroll((chat.message_scroll, 0));

    f.render_widget(messages_widget, messages_area);

    // === RENDER INPUT BOX ===
    let (input_title, input_style) = if let Some(error) = &chat.init_error {
        (
            format!(" Error: {} ", error),
            Style::default().fg(Color::Red),
        )
    } else if !chat.initialized {
        (
            " Please wait... ".to_string(),
            Style::default().fg(Color::DarkGray),
        )
    } else {
        (
            " Type your message (Enter to send, Tab to switch pane) ".to_string(),
            Style::default().fg(Color::White),
        )
    };

    let input_widget = Paragraph::new(chat.input_buffer.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(input_title)
                .style(input_style),
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
                        .fg(Color::White)
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
                    Span::styled(&tool_call.name, Style::default().fg(Color::White)),
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
