//! Workflow rendering functions (list, detail, edit, running)

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use workflow_manager_sdk::{FieldType, WorkflowSource};

use crate::models::*;

pub fn render_workflow_list(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Available Workflows ");

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let items: Vec<ListItem> = app
        .workflows
        .iter()
        .enumerate()
        .map(|(i, workflow)| {
            let is_selected = i == app.selected;
            let bullet = if is_selected { "▶" } else { " " };

            let source_label = match workflow.source {
                WorkflowSource::BuiltIn => "[Built-in]",
                WorkflowSource::UserDefined => "[User]",
            };

            let lines = vec![
                Line::from(vec![
                    Span::raw(format!(" {} ", bullet)),
                    Span::styled(
                        &workflow.info.name,
                        Style::default()
                            .fg(if is_selected {
                                Color::White
                            } else {
                                Color::Gray
                            })
                            .add_modifier(if is_selected {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            }),
                    ),
                    Span::raw(" "),
                    Span::styled(source_label, Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(vec![Span::styled(
                    format!("     {}", workflow.info.description),
                    Style::default().fg(Color::DarkGray),
                )]),
                Line::from(""),
            ];

            ListItem::new(lines)
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, inner_area);
}

pub fn render_workflow_detail(f: &mut Frame, area: Rect, app: &App, idx: usize) {
    let workflow = match app.workflows.get(idx) {
        Some(w) => w,
        None => {
            let error = Paragraph::new("Workflow not found")
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::Red));
            f.render_widget(error, area);
            return;
        }
    };

    let source_text = match workflow.source {
        WorkflowSource::BuiltIn => "Built-in workflow",
        WorkflowSource::UserDefined => "User-defined workflow",
    };

    let mut info_lines = vec![
        Line::from(vec![
            Span::styled("Name: ", Style::default().fg(Color::Gray)),
            Span::styled(
                &workflow.info.name,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("ID: ", Style::default().fg(Color::Gray)),
            Span::styled(&workflow.info.id, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Description: ", Style::default().fg(Color::Gray)),
            Span::styled(
                &workflow.info.description,
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Source: ", Style::default().fg(Color::Gray)),
            Span::styled(source_text, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Fields: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", workflow.info.fields.len()),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
    ];

    // Add arguments summary
    let configured_count = app
        .field_values
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .count();

    info_lines.push(Line::from(vec![
        Span::styled("Arguments: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{} configured", configured_count),
            Style::default().fg(if configured_count > 0 {
                Color::White
            } else {
                Color::DarkGray
            }),
        ),
    ]));
    info_lines.push(Line::from(""));

    // Show all fields with their values (or <empty>)
    for field in &workflow.info.fields {
        let value = app
            .field_values
            .get(&field.name)
            .map(|s| s.as_str())
            .unwrap_or("");

        let (display_value, value_style) = if value.is_empty() {
            ("<empty>".to_string(), Style::default().fg(Color::DarkGray))
        } else if value.len() > 60 {
            (
                format!("{}...", &value[..60]),
                Style::default().fg(Color::White),
            )
        } else {
            (value.to_string(), Style::default().fg(Color::White))
        };

        info_lines.push(Line::from(vec![
            Span::styled("  • ", Style::default().fg(Color::DarkGray)),
            Span::styled(&field.label, Style::default().fg(Color::White)),
            Span::raw(": "),
            Span::styled(display_value, value_style),
        ]));
    }
    info_lines.push(Line::from(""));

    info_lines.push(Line::from(""));
    info_lines.push(Line::from(vec![
        Span::styled(
            "[L]",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Launch workflow"),
    ]));

    let widget = Paragraph::new(info_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Workflow Info "),
    );
    f.render_widget(widget, area);
}

pub fn render_workflow_edit(f: &mut Frame, area: Rect, app: &App, idx: usize) {
    let workflow = match app.workflows.get(idx) {
        Some(w) => w,
        None => {
            let error = Paragraph::new("Workflow not found")
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::Red));
            f.render_widget(error, area);
            return;
        }
    };

    let items: Vec<ListItem> = workflow
        .info
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let is_selected = i == app.edit_field_index;
            let is_editing_this = is_selected && app.is_editing;

            // Get current value and display based on field type
            let current_value = if is_editing_this {
                &app.edit_buffer
            } else {
                app.field_values
                    .get(&field.name)
                    .map(|s| s.as_str())
                    .unwrap_or("")
            };

            let (display_text, is_empty) = match &field.field_type {
                FieldType::PhaseSelector { .. } => {
                    // Just show the value as-is (e.g., "0,1,2,3,4")
                    if current_value.is_empty() {
                        ("<empty>", true)
                    } else {
                        (current_value, false)
                    }
                }
                FieldType::StateFile { pattern, .. } => {
                    if current_value.is_empty() {
                        let msg = format!("<select file matching {}>", pattern);
                        (Box::leak(msg.into_boxed_str()) as &str, true)
                    } else {
                        (current_value, false)
                    }
                }
                _ => {
                    if current_value.is_empty() {
                        ("<empty>", true)
                    } else {
                        (current_value, false)
                    }
                }
            };

            let value_style = if is_editing_this {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if is_empty {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC)
            } else {
                Style::default().fg(Color::White)
            };

            // Determine if field is required based on selected phases
            let is_required = if let Some(required_phases) = &field.required_for_phases {
                // Get selected phases from the "phases" field value
                let selected_phases: Vec<usize> = app
                    .field_values
                    .get("phases")
                    .map(|v| v.split(',').filter_map(|s| s.trim().parse().ok()).collect())
                    .unwrap_or_default();

                // Field is required if the EARLIEST selected phase needs this field
                // (e.g., phases "1,2,3,4" only needs requirements for phase 1)
                selected_phases
                    .iter()
                    .min()
                    .map(|min_phase| required_phases.contains(min_phase))
                    .unwrap_or(false)
            } else {
                field.required
            };

            let required_marker = if is_required { "*" } else { "" };

            let lines = vec![
                Line::from(vec![Span::styled(
                    format!("{}{}: ", field.label, required_marker),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![Span::styled(
                    format!("  {}", field.description),
                    Style::default().fg(Color::DarkGray),
                )]),
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(display_text, value_style),
                    if is_editing_this {
                        Span::styled(" █", Style::default().fg(Color::White))
                    } else {
                        Span::raw("")
                    },
                ]),
                Line::from(""),
            ];

            ListItem::new(lines)
        })
        .collect();

    let title = if app.is_editing {
        format!(" Configure: {} [EDITING] ", workflow.info.name)
    } else {
        format!(" Configure: {} ", workflow.info.name)
    };

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(list, area);
}

pub fn render_workflow_running(f: &mut Frame, area: Rect, app: &App, idx: usize) {
    let workflow = match app.workflows.get(idx) {
        Some(w) => w,
        None => {
            let error =
                Paragraph::new("Workflow not found").block(Block::default().borders(Borders::ALL));
            f.render_widget(error, area);
            return;
        }
    };

    let title = format!(
        "Running: {} {}",
        workflow.info.name,
        if app.workflow_running {
            "[IN PROGRESS]"
        } else {
            "[COMPLETED]"
        }
    );

    let mut lines: Vec<Line> = Vec::new();

    // Display hierarchical phase/task/agent structure
    let phases_snapshot: Vec<WorkflowPhase> = if let Ok(phases) = app.workflow_phases.lock() {
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

            let is_expanded = app.expanded_phases.contains(&phase.id);
            let expand_icon = if is_expanded { "▼" } else { "▶" };
            let is_selected = app.selected_phase == phase.id
                && app.selected_task.is_none()
                && app.selected_agent.is_none();

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

                    let task_expanded = app.expanded_tasks.contains(&task.id);
                    let task_expand_icon = if task_expanded { "▼" } else { "▶" };
                    let is_task_selected = app.selected_phase == phase.id
                        && Some(&task.id) == app.selected_task.as_ref()
                        && app.selected_agent.is_none();

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

                            let agent_expanded = app.expanded_agents.contains(&agent.id);
                            let agent_expand_icon = if agent_expanded { "▼" } else { "▶" };
                            let is_agent_selected = Some(&agent.id) == app.selected_agent.as_ref();

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

                            lines.push(Line::from(agent_spans));

                            // Show last message in full detail if collapsed
                            if !agent_expanded && !agent.messages.is_empty() {
                                if let Some(last_msg) = agent.messages.last() {
                                    lines.push(Line::from(vec![
                                        Span::raw("      "),
                                        Span::styled(last_msg, Style::default().fg(Color::Gray)),
                                    ]));
                                }
                            }

                            if agent_expanded {
                                // Display agent messages
                                for msg in &agent.messages {
                                    lines.push(Line::from(vec![
                                        Span::raw("      "),
                                        Span::styled(msg, Style::default().fg(Color::Gray)),
                                    ]));
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
                            Span::styled(format!("📄 {}", path), Style::default().fg(Color::White)),
                            Span::raw(" - "),
                            Span::styled(desc, Style::default().fg(Color::Gray)),
                        ]));
                    }
                }
            }

            lines.push(Line::from(""));
        }
    }

    // Append regular stdout output
    if let Ok(output) = app.workflow_output.lock() {
        if !output.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "─────────────────",
                Style::default().fg(Color::DarkGray),
            )]));
            lines.push(Line::from(vec![Span::styled(
                "Workflow Output:",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(""));
            for line in output.iter() {
                lines.push(Line::from(line.clone()));
            }
        }
    }

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(Style::default().fg(Color::White)),
        )
        .scroll((app.workflow_scroll_offset as u16, 0));

    f.render_widget(paragraph, area);
}
