//! Tab rendering functions

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use workflow_manager_sdk::WorkflowStatus;

use super::components::centered_rect;
use crate::models::*;

pub fn render_tab_bar(f: &mut Frame, area: Rect, app: &App) {
    // Calculate visible tabs for horizontal scrolling
    let max_chars = area.width as usize - 10;
    let mut current_width = 0;
    let mut first_visible = 0;

    // Find scroll window to keep active tab visible
    for (i, tab) in app.open_tabs.iter().enumerate() {
        let tab_width = tab.workflow_name.len() + 5; // " #N ● "

        if i < app.active_tab_idx {
            if current_width + tab_width > max_chars / 2 {
                first_visible = i + 1;
                current_width = 0;
            } else {
                current_width += tab_width;
            }
        }
    }

    // Build tab titles
    let mut spans = Vec::new();

    for (i, tab) in app.open_tabs.iter().skip(first_visible).enumerate() {
        let real_idx = i + first_visible;
        let is_active = real_idx == app.active_tab_idx;

        // Truncate name if too long
        let name = if tab.workflow_name.len() > 10 {
            format!("{}...", &tab.workflow_name[..7])
        } else {
            tab.workflow_name.clone()
        };

        // Status icon
        let icon = match tab.status {
            WorkflowStatus::Running => "●",
            WorkflowStatus::Completed => "✓",
            WorkflowStatus::Failed => "✗",
            WorkflowStatus::NotStarted => "○",
        };

        let title = format!("[ {} #{} {} ]", name, tab.instance_number, icon);

        let style = if is_active {
            Style::default()
                .fg(Color::White)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        spans.push(Span::styled(title, style));
        spans.push(Span::raw(" ")); // Space between tabs

        // Check if we've exceeded width
        let total_width: usize = spans.iter().map(|s| s.content.len()).sum();
        if total_width > max_chars {
            break;
        }
    }

    // Add [+ New] button
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        "[+ New]",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ));

    let tabs_line = Line::from(spans);
    let separator = Line::from("━".repeat(area.width as usize));

    let paragraph = Paragraph::new(vec![tabs_line, separator]);
    f.render_widget(paragraph, area);
}

pub fn render_empty_tabs(f: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(""),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "No workflows running",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press [Ctrl+T] or click [+ New]",
            Style::default().fg(Color::White),
        )),
        Line::from(Span::styled(
            "to start a new workflow",
            Style::default().fg(Color::White),
        )),
    ];

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

pub fn render_close_confirmation(f: &mut Frame, area: Rect) {
    let popup_area = centered_rect(50, 30, area);

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Close Running Workflow?",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "This workflow is still running.",
            Style::default().fg(Color::White),
        )),
        Line::from(Span::styled(
            "Closing will kill the process.",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "[Y]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Yes  "),
            Span::styled(
                "[N]",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" No"),
        ]),
    ];

    let paragraph = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::Black)),
    );

    f.render_widget(paragraph, popup_area);
}

pub fn render_tab_content(f: &mut Frame, area: Rect, _app: &App, tab: &WorkflowTab) {
    let title = format!(" {} #{} ", tab.workflow_name, tab.instance_number);

    let mut lines: Vec<Line> = Vec::new();

    // Display hierarchical phase/task/agent structure
    let phases_snapshot: Vec<WorkflowPhase> = if let Ok(phases) = tab.workflow_phases.lock() {
        phases.clone()
    } else {
        Vec::new()
    };

    if !phases_snapshot.is_empty() {
        for phase in &phases_snapshot {
            // Phase header
            let phase_icon = match phase.status {
                PhaseStatus::NotStarted => "○",
                PhaseStatus::Running => "▶",
                PhaseStatus::Completed => "✓",
                PhaseStatus::Failed => "✗",
            };
            let phase_color = match phase.status {
                PhaseStatus::NotStarted => Color::Gray,
                PhaseStatus::Running => Color::Yellow,
                PhaseStatus::Completed => Color::White,
                PhaseStatus::Failed => Color::Red,
            };

            let is_expanded = tab.expanded_phases.contains(&phase.id);
            let expand_icon = if is_expanded { "▼" } else { "▶" };
            let is_selected = tab.selected_phase == phase.id
                && tab.selected_task.is_none()
                && tab.selected_agent.is_none();

            let mut phase_spans = vec![
                Span::styled(format!("{} ", phase_icon), Style::default().fg(phase_color)),
                Span::styled(
                    format!("{} ", expand_icon),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("Phase {}: {}", phase.id, phase.name),
                    if is_selected {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                    } else {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    },
                ),
            ];

            // Show last task message if collapsed
            if !is_expanded && !phase.tasks.is_empty() {
                if let Some(last_task) = phase.tasks.last() {
                    if let Some(last_msg) = last_task.messages.last() {
                        let preview = if last_msg.len() > 40 {
                            format!(" - {}...", &last_msg[..40])
                        } else {
                            format!(" - {}", last_msg)
                        };
                        phase_spans
                            .push(Span::styled(preview, Style::default().fg(Color::DarkGray)));
                    }
                }
            }

            lines.push(Line::from(phase_spans));

            if is_expanded {
                // Display tasks
                for task in &phase.tasks {
                    let task_icon = match task.status {
                        TaskStatus::NotStarted => "○",
                        TaskStatus::Running => "▶",
                        TaskStatus::Completed => "✓",
                        TaskStatus::Failed => "✗",
                    };
                    let task_color = match task.status {
                        TaskStatus::NotStarted => Color::Gray,
                        TaskStatus::Running => Color::Yellow,
                        TaskStatus::Completed => Color::White,
                        TaskStatus::Failed => Color::Red,
                    };

                    let task_expanded = tab.expanded_tasks.contains(&task.id);
                    let task_expand_icon = if task_expanded { "▼" } else { "▶" };
                    let is_task_selected = tab.selected_phase == phase.id
                        && Some(&task.id) == tab.selected_task.as_ref()
                        && tab.selected_agent.is_none();

                    let mut task_spans = vec![
                        Span::raw("  "),
                        Span::styled(format!("{} ", task_icon), Style::default().fg(task_color)),
                        Span::styled(
                            format!("{} ", task_expand_icon),
                            Style::default().fg(Color::White),
                        ),
                        Span::styled(
                            &task.description,
                            if is_task_selected {
                                Style::default()
                                    .fg(Color::White)
                                    .add_modifier(Modifier::REVERSED)
                            } else {
                                Style::default().fg(Color::White)
                            },
                        ),
                    ];

                    // Show last message if collapsed
                    if !task_expanded && !task.messages.is_empty() {
                        if let Some(last_msg) = task.messages.last() {
                            let preview = if last_msg.len() > 30 {
                                format!(" - {}...", &last_msg[..30])
                            } else {
                                format!(" - {}", last_msg)
                            };
                            task_spans
                                .push(Span::styled(preview, Style::default().fg(Color::DarkGray)));
                        }
                    }

                    lines.push(Line::from(task_spans));

                    if task_expanded {
                        // Display task messages
                        for msg in &task.messages {
                            lines.push(Line::from(vec![
                                Span::raw("    "),
                                Span::styled(msg, Style::default().fg(Color::Gray)),
                            ]));
                        }

                        // Display agents
                        for agent in &task.agents {
                            let agent_icon = match agent.status {
                                AgentStatus::NotStarted => "○",
                                AgentStatus::Running => "▶",
                                AgentStatus::Completed => "✓",
                                AgentStatus::Failed => "✗",
                            };
                            let agent_color = match agent.status {
                                AgentStatus::NotStarted => Color::Gray,
                                AgentStatus::Running => Color::Yellow,
                                AgentStatus::Completed => Color::White,
                                AgentStatus::Failed => Color::Red,
                            };

                            let agent_expanded = tab.expanded_agents.contains(&agent.id);
                            let agent_expand_icon = if agent_expanded { "▼" } else { "▶" };
                            let is_agent_selected = Some(&agent.id) == tab.selected_agent.as_ref();

                            let agent_spans = vec![
                                Span::raw("    "),
                                Span::styled(
                                    format!("{} ", agent_icon),
                                    Style::default().fg(agent_color),
                                ),
                                Span::styled(
                                    format!("{} ", agent_expand_icon),
                                    Style::default().fg(Color::White),
                                ),
                                Span::styled(
                                    format!("@{}", agent.name),
                                    if is_agent_selected {
                                        Style::default()
                                            .fg(Color::Magenta)
                                            .add_modifier(Modifier::REVERSED)
                                    } else {
                                        Style::default().fg(Color::Magenta)
                                    },
                                ),
                            ];

                            // Show last line when collapsed
                            let mut agent_line_spans = agent_spans;
                            if !agent_expanded && !agent.messages.is_empty() {
                                if let Some(last_msg) = agent.messages.last() {
                                    let preview = if last_msg.len() > 50 {
                                        format!(" - {}...", &last_msg[..50])
                                    } else {
                                        format!(" - {}", last_msg)
                                    };
                                    agent_line_spans.push(Span::styled(
                                        preview,
                                        Style::default().fg(Color::DarkGray),
                                    ));
                                }
                            }
                            lines.push(Line::from(agent_line_spans));

                            if agent_expanded {
                                // Display scrollable 5-line window of agent messages
                                let window_size = 5;
                                let total_messages = agent.messages.len();

                                if total_messages > 0 {
                                    // Default to showing the LAST 5 messages (most recent)
                                    let default_offset = total_messages.saturating_sub(window_size);
                                    let scroll_offset = tab
                                        .agent_scroll_offsets
                                        .get(&agent.id)
                                        .copied()
                                        .unwrap_or(default_offset);

                                    let start = scroll_offset.min(total_messages.saturating_sub(1));
                                    let end = (start + window_size).min(total_messages);

                                    for msg in &agent.messages[start..end] {
                                        lines.push(Line::from(vec![
                                            Span::raw("      "),
                                            Span::styled(msg, Style::default().fg(Color::Gray)),
                                        ]));
                                    }

                                    // Show scroll indicator if there are more messages
                                    if total_messages > window_size {
                                        let indicator = format!(
                                            "      [Showing {}-{} of {}]",
                                            start + 1,
                                            end,
                                            total_messages
                                        );
                                        lines.push(Line::from(vec![Span::styled(
                                            indicator,
                                            Style::default()
                                                .fg(Color::White)
                                                .add_modifier(Modifier::ITALIC),
                                        )]));
                                    }
                                }
                            }
                        }
                    }
                }

                // Display output files
                if !phase.output_files.is_empty() {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled("Output files:", Style::default().fg(Color::White)),
                    ]));
                    for (path, desc) in &phase.output_files {
                        lines.push(Line::from(vec![
                            Span::raw("    "),
                            Span::styled("📄 ", Style::default().fg(Color::White)),
                            Span::styled(path, Style::default().fg(Color::Yellow)),
                            Span::raw(" - "),
                            Span::styled(desc, Style::default().fg(Color::Gray)),
                        ]));
                    }
                }
            }
        }
    } else {
        // No phases yet - show output
        if let Ok(output) = tab.workflow_output.lock() {
            for line in output.iter() {
                lines.push(Line::from(line.clone()));
            }
        }
    }

    let content = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(Style::default().fg(Color::White)),
        )
        .scroll((tab.scroll_offset as u16, 0));

    f.render_widget(content, area);
}
