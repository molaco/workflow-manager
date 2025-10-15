//! UI rendering functions for the workflow manager TUI
//!
//! This module contains all the rendering logic for different views and components,
//! including workflow lists, edit forms, running workflows, tabs, and chat interface.

use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use std::path::PathBuf;
use workflow_manager_sdk::{FieldType, WorkflowSource, WorkflowStatus};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

use crate::models::*;
use crate::chat;

/// Main UI rendering function - orchestrates all view rendering
pub fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Header
    render_header(f, chunks[0], app);

    // Main content
    // Show tab bar if we're in Tabs view OR in new tab flow
    if matches!(app.current_view, View::Tabs) || app.in_new_tab_flow {
        // Split screen: tab bar + content
        let tab_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),     // Tab bar
                Constraint::Min(0),        // Content
            ])
            .split(chunks[1]);

        render_tab_bar(f, tab_chunks[0], app);

        // Render content based on view
        match app.current_view {
            View::Tabs => {
                if app.open_tabs.is_empty() {
                    render_empty_tabs(f, tab_chunks[1]);
                } else if let Some(tab) = app.open_tabs.get(app.active_tab_idx) {
                    render_tab_content(f, tab_chunks[1], app, tab);
                }
            }
            View::Chat => render_chat(f, tab_chunks[1], app),
            View::WorkflowList => render_workflow_list(f, tab_chunks[1], app),
            View::WorkflowDetail(idx) => render_workflow_detail(f, tab_chunks[1], app, idx),
            View::WorkflowEdit(idx) => render_workflow_edit(f, tab_chunks[1], app, idx),
            View::WorkflowRunning(idx) => render_workflow_running(f, tab_chunks[1], app, idx),
        }
    } else {
        // Traditional single-workflow view (no tabs)
        match app.current_view {
            View::WorkflowList => render_workflow_list(f, chunks[1], app),
            View::WorkflowDetail(idx) => render_workflow_detail(f, chunks[1], app, idx),
            View::WorkflowEdit(idx) => render_workflow_edit(f, chunks[1], app, idx),
            View::WorkflowRunning(idx) => render_workflow_running(f, chunks[1], app, idx),
            View::Chat => render_chat(f, chunks[1], app),
            View::Tabs => {
                // Should not happen
                let placeholder = Paragraph::new("Error: Tabs view without tab mode");
                f.render_widget(placeholder, chunks[1]);
            }
        }
    }

    // Footer
    render_footer(f, chunks[2], app);

    // Dropdown overlay
    if app.show_dropdown {
        render_dropdown(f, chunks[1], app);
    }

    // File browser overlay
    if app.show_file_browser {
        render_file_browser(f, f.area(), app);
    }

    // Close confirmation overlay
    if app.show_close_confirmation {
        render_close_confirmation(f, f.area());
    }
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let title = match app.current_view {
        View::WorkflowList => "Workflow Manager v0.2.0 - Workflows",
        View::WorkflowDetail(_) => "Workflow Manager v0.2.0 - Workflow Detail",
        View::WorkflowEdit(_) => "Workflow Manager v0.2.0 - Configure Workflow",
        View::WorkflowRunning(_) => "Workflow Manager v0.2.0 - Running Workflow",
        View::Tabs => "Workflow Manager v0.2.0 - Running Workflows",
        View::Chat => "Workflow Manager v0.2.0 - AI Chat",
    };

    let header = Paragraph::new(Line::from(vec![
        Span::styled(title, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("      "),
        Span::styled("[Q]", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("uit"),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, area);
}

fn render_workflow_list(f: &mut Frame, area: Rect, app: &App) {
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
                            .fg(if is_selected { Color::White } else { Color::Gray })
                            .add_modifier(if is_selected {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            }),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        source_label,
                        Style::default().fg(Color::DarkGray),
                    ),
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

fn render_workflow_detail(f: &mut Frame, area: Rect, app: &App, idx: usize) {
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
            Span::styled(&workflow.info.name, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("ID: ", Style::default().fg(Color::Gray)),
            Span::styled(&workflow.info.id, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Description: ", Style::default().fg(Color::Gray)),
            Span::styled(&workflow.info.description, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Source: ", Style::default().fg(Color::Gray)),
            Span::styled(source_text, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Fields: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}", workflow.info.fields.len()), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
    ];

    // Add arguments summary
    let configured_count = app.field_values.iter()
        .filter(|(_, v)| !v.is_empty())
        .count();

    info_lines.push(Line::from(vec![
        Span::styled("Arguments: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{} configured", configured_count),
            Style::default().fg(if configured_count > 0 { Color::Green } else { Color::DarkGray })
        ),
    ]));
    info_lines.push(Line::from(""));

    // Show all fields with their values (or <empty>)
    for field in &workflow.info.fields {
        let value = app.field_values.get(&field.name)
            .map(|s| s.as_str())
            .unwrap_or("");

        let (display_value, value_style) = if value.is_empty() {
            ("<empty>".to_string(), Style::default().fg(Color::DarkGray))
        } else if value.len() > 60 {
            (format!("{}...", &value[..60]), Style::default().fg(Color::White))
        } else {
            (value.to_string(), Style::default().fg(Color::White))
        };

        info_lines.push(Line::from(vec![
            Span::styled("  • ", Style::default().fg(Color::DarkGray)),
            Span::styled(&field.label, Style::default().fg(Color::Cyan)),
            Span::raw(": "),
            Span::styled(display_value, value_style),
        ]));
    }
    info_lines.push(Line::from(""));

    info_lines.push(Line::from(""));
    info_lines.push(Line::from(vec![
        Span::styled("[L]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(" Launch workflow"),
    ]));

    let widget = Paragraph::new(info_lines)
        .block(Block::default().borders(Borders::ALL).title(" Workflow Info "));
    f.render_widget(widget, area);
}

fn render_workflow_edit(f: &mut Frame, area: Rect, app: &App, idx: usize) {
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
                app.field_values.get(&field.name).map(|s| s.as_str()).unwrap_or("")
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
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else if is_selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else if is_empty {
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)
            } else {
                Style::default().fg(Color::White)
            };

            // Determine if field is required based on selected phases
            let is_required = if let Some(required_phases) = &field.required_for_phases {
                // Get selected phases from the "phases" field value
                let selected_phases: Vec<usize> = app.field_values
                    .get("phases")
                    .map(|v| v.split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect())
                    .unwrap_or_default();

                // Field is required if the EARLIEST selected phase needs this field
                // (e.g., phases "1,2,3,4" only needs requirements for phase 1)
                selected_phases.iter().min()
                    .map(|min_phase| required_phases.contains(min_phase))
                    .unwrap_or(false)
            } else {
                field.required
            };

            let required_marker = if is_required { "*" } else { "" };

            let lines = vec![
                Line::from(vec![
                    Span::styled(
                        format!("{}{}: ", field.label, required_marker),
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::styled(
                        format!("  {}", field.description),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(display_text, value_style),
                    if is_editing_this {
                        Span::styled(" █", Style::default().fg(Color::Green))
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

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(title));

    f.render_widget(list, area);
}

fn render_workflow_running(f: &mut Frame, area: Rect, app: &App, idx: usize) {
    let workflow = match app.workflows.get(idx) {
        Some(w) => w,
        None => {
            let error = Paragraph::new("Workflow not found")
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(error, area);
            return;
        }
    };

    let title = format!(
        "Running: {} {}",
        workflow.info.name,
        if app.workflow_running { "[IN PROGRESS]" } else { "[COMPLETED]" }
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
                    PhaseStatus::Completed => Color::Green,
                    PhaseStatus::Failed => Color::Red,
                };

                let is_expanded = app.expanded_phases.contains(&phase.id);
                let expand_icon = if is_expanded { "▼" } else { "▶" };
                let is_selected = app.selected_phase == phase.id && app.selected_task.is_none() && app.selected_agent.is_none();

                let mut phase_spans = vec![
                    Span::styled(format!("{} ", phase_icon), Style::default().fg(phase_color)),
                    Span::styled(format!("{} ", expand_icon), Style::default().fg(Color::Cyan)),
                    Span::styled(
                        format!("Phase {}: {}", phase.id, phase.name),
                        if is_selected {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD | Modifier::REVERSED)
                        } else {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        }
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
                            phase_spans.push(Span::styled(preview, Style::default().fg(Color::DarkGray)));
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
                            TaskStatus::Completed => Color::Green,
                            TaskStatus::Failed => Color::Red,
                        };

                        let task_expanded = app.expanded_tasks.contains(&task.id);
                        let task_expand_icon = if task_expanded { "▼" } else { "▶" };
                        let is_task_selected = app.selected_phase == phase.id &&
                                              Some(&task.id) == app.selected_task.as_ref() &&
                                              app.selected_agent.is_none();

                        let mut task_spans = vec![
                            Span::raw("  "),
                            Span::styled(format!("{} ", task_icon), Style::default().fg(task_color)),
                            Span::styled(format!("{} ", task_expand_icon), Style::default().fg(Color::Cyan)),
                            Span::styled(
                                &task.description,
                                if is_task_selected {
                                    Style::default().fg(Color::White).add_modifier(Modifier::REVERSED)
                                } else {
                                    Style::default().fg(Color::White)
                                }
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
                                task_spans.push(Span::styled(preview, Style::default().fg(Color::DarkGray)));
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
                                    AgentStatus::Completed => Color::Green,
                                    AgentStatus::Failed => Color::Red,
                                };

                                let agent_expanded = app.expanded_agents.contains(&agent.id);
                                let agent_expand_icon = if agent_expanded { "▼" } else { "▶" };
                                let is_agent_selected = Some(&agent.id) == app.selected_agent.as_ref();

                                let agent_spans = vec![
                                    Span::raw("    "),
                                    Span::styled(format!("{} ", agent_icon), Style::default().fg(agent_color)),
                                    Span::styled(format!("{} ", agent_expand_icon), Style::default().fg(Color::Cyan)),
                                    Span::styled(
                                        format!("@{}", agent.name),
                                        if is_agent_selected {
                                            Style::default().fg(Color::Magenta).add_modifier(Modifier::REVERSED)
                                        } else {
                                            Style::default().fg(Color::Magenta)
                                        }
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
                            Span::styled("Output files:", Style::default().fg(Color::Cyan)),
                        ]));
                        for (path, desc) in &phase.output_files {
                            lines.push(Line::from(vec![
                                Span::raw("    "),
                                Span::styled(format!("📄 {}", path), Style::default().fg(Color::Blue)),
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
            lines.push(Line::from(vec![
                Span::styled("─────────────────", Style::default().fg(Color::DarkGray)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Workflow Output:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]));
            lines.push(Line::from(""));
            for line in output.iter() {
                lines.push(Line::from(line.clone()));
            }
        }
    }

    let paragraph = Paragraph::new(lines)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().fg(Color::White)))
        .scroll((app.workflow_scroll_offset as u16, 0));

    f.render_widget(paragraph, area);
}

fn render_footer(f: &mut Frame, area: Rect, app: &App) {
    let footer_text = match app.current_view {
        View::WorkflowList => Line::from(vec![
            Span::styled("[↑↓]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Navigate  "),
            Span::styled("[Enter/V]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" View  "),
            Span::styled("[Q]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Quit"),
        ]),
        View::WorkflowDetail(_) => Line::from(vec![
            Span::styled("[E]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Edit  "),
            Span::styled("[Esc/B]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Back  "),
            Span::styled("[Q]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Quit"),
        ]),
        View::WorkflowEdit(_) => {
            if app.is_editing {
                Line::from(vec![
                    Span::styled("TYPE", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw(" to edit  "),
                    Span::styled("[Enter]", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" Save  "),
                    Span::styled("[Esc]", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" Cancel  "),
                    Span::styled("[Backspace]", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" Delete"),
                ])
            } else {
                Line::from(vec![
                    Span::styled("[↑↓]", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" Navigate  "),
                    Span::styled("[Enter]", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" Edit  "),
                    Span::styled("[L]", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" Launch  "),
                    Span::styled("[Esc/B]", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" Back  "),
                    Span::styled("[Q]", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" Quit"),
                ])
            }
        }
        View::WorkflowRunning(_) => Line::from(vec![
            Span::styled("[↑↓/jk]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Navigate  "),
            Span::styled("[Enter]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Expand/Collapse  "),
            Span::styled("[Space]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Toggle All  "),
            Span::styled("[Esc/B]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Back  "),
            Span::styled("[Q]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Quit"),
        ]),
        View::Tabs => Line::from(vec![
            Span::styled("[↑↓/jk]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Navigate  "),
            Span::styled("[←→/hl]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Scroll Agent  "),
            Span::styled("[Enter]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Expand  "),
            Span::styled("[Space]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Toggle All  "),
            Span::styled("[Tab]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Switch  "),
            Span::styled("[E]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Edit  "),
            Span::styled("[R]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Rerun  "),
            Span::styled("[C]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Close  "),
            Span::styled("[A]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" AI Chat  "),
            Span::styled("[Q]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Quit"),
        ]),
        View::Chat => Line::from(vec![
            Span::styled("[↑↓]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Scroll  "),
            Span::styled("[Enter]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Send  "),
            Span::styled("[Esc]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Back  "),
            Span::styled("[Ctrl+Q]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Quit"),
        ]),
    };

    let footer = Paragraph::new(footer_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, area);
}

fn render_dropdown(f: &mut Frame, area: Rect, app: &App) {
    // Check if we're showing history or file paths
    let (item_count, title) = if !app.history_items.is_empty() {
        (app.history_items.len(), " History ")
    } else if !app.dropdown_items.is_empty() {
        (app.dropdown_items.len(), " Tab Completion ")
    } else {
        return;
    };

    // Calculate dropdown position (below current field)
    let field_offset = app.edit_field_index * 4; // Each field takes ~4 lines
    let dropdown_y = area.y + field_offset as u16 + 4;
    let dropdown_height = std::cmp::min(10, item_count as u16 + 2);

    let dropdown_area = Rect {
        x: area.x + 2,
        y: std::cmp::min(dropdown_y, area.bottom().saturating_sub(dropdown_height)),
        width: area.width.saturating_sub(4),
        height: dropdown_height,
    };

    let items: Vec<ListItem> = if !app.history_items.is_empty() {
        // History dropdown
        app.history_items
            .iter()
            .enumerate()
            .map(|(i, value)| {
                let is_selected = i == app.dropdown_selected;

                let style = if is_selected {
                    Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(value, style),
                ]))
            })
            .collect()
    } else {
        // File path dropdown
        app.dropdown_items
            .iter()
            .enumerate()
            .map(|(i, path)| {
                let is_selected = i == app.dropdown_selected;
                let is_dir = path.is_dir();

                // Check if this is the parent directory (first item is always parent)
                let is_parent = i == 0;

                let name = if is_parent {
                    "../".to_string()
                } else {
                    let base_name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("?");
                    if is_dir {
                        format!("{}/", base_name)
                    } else {
                        base_name.to_string()
                    }
                };

                let style = if is_selected {
                    Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else if is_dir {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };

                ListItem::new(Line::from(Span::styled(name, style)))
            })
            .collect()
    };

    // Calculate scroll offset to keep selected item visible
    let visible_items = (dropdown_height.saturating_sub(2)) as usize; // Subtract 2 for borders
    let scroll_offset = if app.dropdown_selected >= visible_items {
        app.dropdown_selected.saturating_sub(visible_items - 1)
    } else {
        0
    };

    // Only show items in the visible window
    let visible_items: Vec<ListItem> = items
        .into_iter()
        .skip(scroll_offset)
        .take(visible_items)
        .collect();

    let list = List::new(visible_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title(title)
                .style(Style::default().bg(Color::DarkGray)),
        );

    f.render_widget(ratatui::widgets::Clear, dropdown_area);
    f.render_widget(list, dropdown_area);
}

fn render_file_browser(f: &mut Frame, area: Rect, app: &App) {
    // Create centered overlay
    let popup_area = centered_rect(80, 80, area);

    // Filter items by fuzzy search
    let matcher = SkimMatcherV2::default();
    let filtered_items: Vec<(usize, &PathBuf)> = if app.file_browser_search.is_empty() {
        app.file_browser_items.iter().enumerate().collect()
    } else {
        app.file_browser_items
            .iter()
            .enumerate()
            .filter(|(_, path)| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|name| matcher.fuzzy_match(name, &app.file_browser_search))
                    .is_some()
            })
            .collect()
    };

    let items: Vec<ListItem> = filtered_items
        .iter()
        .enumerate()
        .map(|(display_idx, (original_idx, path))| {
            let is_selected = *original_idx == app.file_browser_selected;
            let is_dir = path.is_dir();

            let icon = if is_dir { "📁" } else { "📄" };
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("..");

            let style = if is_selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else if is_dir {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(Line::from(vec![
                Span::raw(if is_selected { "▶ " } else { "  " }),
                Span::raw(format!("{} ", icon)),
                Span::styled(name, style),
            ]))
        })
        .collect();

    let title = if app.file_browser_search.is_empty() {
        format!(" File Browser: {} ", app.current_dir.display())
    } else {
        format!(" File Browser [search: {}] ", app.file_browser_search)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(Style::default().bg(Color::Black)),
        );

    f.render_widget(ratatui::widgets::Clear, popup_area);
    f.render_widget(list, popup_area);
}

// Helper to create a centered rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// Tab rendering functions
fn render_tab_bar(f: &mut Frame, area: Rect, app: &App) {
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
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    ));

    let tabs_line = Line::from(spans);
    let separator = Line::from("━".repeat(area.width as usize));

    let paragraph = Paragraph::new(vec![tabs_line, separator]);
    f.render_widget(paragraph, area);
}

fn render_empty_tabs(f: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(""),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "No workflows running",
            Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD)
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press [Ctrl+T] or click [+ New]",
            Style::default().fg(Color::Cyan)
        )),
        Line::from(Span::styled(
            "to start a new workflow",
            Style::default().fg(Color::Cyan)
        )),
    ];

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

fn render_close_confirmation(f: &mut Frame, area: Rect) {
    let popup_area = centered_rect(50, 30, area);

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Close Running Workflow?",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        )),
        Line::from(""),
        Line::from(Span::styled(
            "This workflow is still running.",
            Style::default().fg(Color::White)
        )),
        Line::from(Span::styled(
            "Closing will kill the process.",
            Style::default().fg(Color::White)
        )),
        Line::from(""),
        Line::from(Line::from(vec![
            Span::styled("[Y]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" Yes  "),
            Span::styled("[N]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(" No"),
        ])),
    ];

    let paragraph = Paragraph::new(text)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::Black)));

    f.render_widget(paragraph, popup_area);
}

fn render_tab_content(f: &mut Frame, area: Rect, _app: &App, tab: &WorkflowTab) {
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
                PhaseStatus::Completed => Color::Green,
                PhaseStatus::Failed => Color::Red,
            };

            let is_expanded = tab.expanded_phases.contains(&phase.id);
            let expand_icon = if is_expanded { "▼" } else { "▶" };
            let is_selected = tab.selected_phase == phase.id && tab.selected_task.is_none() && tab.selected_agent.is_none();

            let mut phase_spans = vec![
                Span::styled(format!("{} ", phase_icon), Style::default().fg(phase_color)),
                Span::styled(format!("{} ", expand_icon), Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("Phase {}: {}", phase.id, phase.name),
                    if is_selected {
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD | Modifier::REVERSED)
                    } else {
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                    }
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
                        phase_spans.push(Span::styled(preview, Style::default().fg(Color::DarkGray)));
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
                        TaskStatus::Completed => Color::Green,
                        TaskStatus::Failed => Color::Red,
                    };

                    let task_expanded = tab.expanded_tasks.contains(&task.id);
                    let task_expand_icon = if task_expanded { "▼" } else { "▶" };
                    let is_task_selected = tab.selected_phase == phase.id &&
                                          Some(&task.id) == tab.selected_task.as_ref() &&
                                          tab.selected_agent.is_none();

                    let mut task_spans = vec![
                        Span::raw("  "),
                        Span::styled(format!("{} ", task_icon), Style::default().fg(task_color)),
                        Span::styled(format!("{} ", task_expand_icon), Style::default().fg(Color::Cyan)),
                        Span::styled(
                            &task.description,
                            if is_task_selected {
                                Style::default().fg(Color::White).add_modifier(Modifier::REVERSED)
                            } else {
                                Style::default().fg(Color::White)
                            }
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
                            task_spans.push(Span::styled(preview, Style::default().fg(Color::DarkGray)));
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
                                AgentStatus::Completed => Color::Green,
                                AgentStatus::Failed => Color::Red,
                            };

                            let agent_expanded = tab.expanded_agents.contains(&agent.id);
                            let agent_expand_icon = if agent_expanded { "▼" } else { "▶" };
                            let is_agent_selected = Some(&agent.id) == tab.selected_agent.as_ref();

                            let agent_spans = vec![
                                Span::raw("    "),
                                Span::styled(format!("{} ", agent_icon), Style::default().fg(agent_color)),
                                Span::styled(format!("{} ", agent_expand_icon), Style::default().fg(Color::Cyan)),
                                Span::styled(
                                    format!("@{}", agent.name),
                                    if is_agent_selected {
                                        Style::default().fg(Color::Magenta).add_modifier(Modifier::REVERSED)
                                    } else {
                                        Style::default().fg(Color::Magenta)
                                    }
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
                                    agent_line_spans.push(Span::styled(preview, Style::default().fg(Color::DarkGray)));
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
                                    let scroll_offset = tab.agent_scroll_offsets.get(&agent.id).copied().unwrap_or(default_offset);

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
                                        let indicator = format!("      [Showing {}-{} of {}]",
                                            start + 1, end, total_messages);
                                        lines.push(Line::from(vec![
                                            Span::styled(indicator, Style::default().fg(Color::Cyan).add_modifier(Modifier::ITALIC)),
                                        ]));
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
                        Span::styled("Output files:", Style::default().fg(Color::Cyan)),
                    ]));
                    for (path, desc) in &phase.output_files {
                        lines.push(Line::from(vec![
                            Span::raw("    "),
                            Span::styled("📄 ", Style::default().fg(Color::Green)),
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
        .block(Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().fg(Color::White)))
        .scroll((tab.scroll_offset as u16, 0));

    f.render_widget(content, area);
}


// Chat view rendering
fn render_chat(f: &mut Frame, area: Rect, app: &App) {
    use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
    use ratatui::style::{Color, Style, Modifier};

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
            Constraint::Min(0),      // Messages
            Constraint::Length(3),   // Input box
        ])
        .split(area);

    // Render messages
    let mut message_lines = Vec::new();
    for msg in &chat.messages {
        let role_style = match msg.role {
            chat::ChatRole::User => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            chat::ChatRole::Assistant => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        };
        let role_text = match msg.role {
            chat::ChatRole::User => "You",
            chat::ChatRole::Assistant => "Claude",
        };

        message_lines.push(Line::from(vec![
            Span::styled(format!("{}: ", role_text), role_style),
        ]));
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
            Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC)
        )));
    }

    let messages_widget = Paragraph::new(message_lines)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(" Chat with Claude ")
            .style(Style::default().fg(Color::Cyan)))
        .wrap(Wrap { trim: false })
        .scroll((chat.scroll_offset as u16, 0));

    f.render_widget(messages_widget, chunks[0]);

    // Render input box
    let input_widget = Paragraph::new(chat.input_buffer.as_str())
        .block(Block::default()
            .borders(Borders::ALL)
            .title(" Type your message (Enter to send) ")
            .style(Style::default().fg(Color::White)));

    f.render_widget(input_widget, chunks[1]);
}

