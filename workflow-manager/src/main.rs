use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io;
use std::path::PathBuf;
use workflow_manager_sdk::{Workflow, WorkflowSource, WorkflowStatus, FieldType};

mod chat;
mod runtime;
mod mcp_tools;
mod discovery;
mod models;
mod ui;
mod app;
mod utils;

use chat::ChatInterface;
use runtime::ProcessBasedRuntime;
use models::*;

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new();

    // Run main loop
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        // Poll all running tabs for output
        app.poll_all_tabs();

        terminal.draw(|f| ui::ui(f, app))?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Close confirmation dialog
                    if app.show_close_confirmation {
                        match key.code {
                            KeyCode::Char('y') | KeyCode::Char('Y') => {
                                app.close_tab_confirmed();
                            }
                            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                app.show_close_confirmation = false;
                            }
                            _ => {}
                        }
                    }
                    // Dropdown mode
                    else if app.show_dropdown {
                        match key.code {
                            KeyCode::Down | KeyCode::Tab => {
                                app.dropdown_next();
                            }
                            KeyCode::Up => {
                                app.dropdown_previous();
                            }
                            KeyCode::Enter => {
                                app.dropdown_select();
                            }
                            KeyCode::Esc => {
                                app.close_dropdown();
                            }
                            _ => {}
                        }
                    }
                    // File browser mode
                    else if app.show_file_browser {
                        match key.code {
                            KeyCode::Down | KeyCode::Char('j') => {
                                app.file_browser_next();
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                app.file_browser_previous();
                            }
                            KeyCode::Enter => {
                                app.file_browser_select();
                            }
                            KeyCode::Esc => {
                                app.close_file_browser();
                            }
                            KeyCode::Char(c) => {
                                // Fuzzy search
                                app.file_browser_search.push(c);
                            }
                            KeyCode::Backspace => {
                                app.file_browser_search.pop();
                            }
                            _ => {}
                        }
                    }
                    // Handle text input mode
                    else if app.is_editing {
                        match key.code {
                            KeyCode::Char(c) => {
                                app.edit_buffer.push(c);
                            }
                            KeyCode::Backspace => {
                                app.edit_buffer.pop();
                            }
                            KeyCode::Enter => {
                                app.save_edited_field();
                            }
                            KeyCode::Esc => {
                                app.cancel_editing();
                            }
                            KeyCode::Tab => {
                                // Tab completion - file paths or history
                                if let View::WorkflowEdit(idx) = app.current_view {
                                    if let Some(workflow) = app.workflows.get(idx) {
                                        if let Some(field) = workflow.info.fields.get(app.edit_field_index) {
                                            match field.field_type {
                                                FieldType::FilePath { .. } | FieldType::StateFile { .. } => {
                                                    app.complete_path();
                                                }
                                                FieldType::Text | FieldType::Number { .. } => {
                                                    app.show_history_dropdown();
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            }
                            KeyCode::Char('/') if app.edit_buffer.is_empty() => {
                                // Open file browser with /
                                app.open_file_browser();
                            }
                            _ => {}
                        }
                    } else if matches!(app.current_view, View::Chat) {
                        // Chat input mode
                        match key.code {
                            KeyCode::Esc => {
                                // Exit chat view
                                app.current_view = View::Tabs;
                            }
                            KeyCode::Char('q') | KeyCode::Char('Q') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                                // Ctrl+Q to quit
                                app.should_quit = true;
                            }
                            KeyCode::Char(c) => {
                                if let Some(chat) = &mut app.chat {
                                    chat.input_buffer.push(c);
                                }
                            }
                            KeyCode::Backspace => {
                                if let Some(chat) = &mut app.chat {
                                    chat.input_buffer.pop();
                                }
                            }
                            KeyCode::Enter => {
                                // Send message to Claude
                                if let Some(chat) = &mut app.chat {
                                    if !chat.input_buffer.is_empty() {
                                        let msg = chat.input_buffer.clone();
                                        chat.input_buffer.clear();

                                        // Send message via tokio runtime
                                        if let Err(e) = app.tokio_runtime.block_on(chat.send_message(msg)) {
                                            chat.messages.push(crate::chat::ChatMessage {
                                                role: crate::chat::ChatRole::Assistant,
                                                content: format!("Error: {}", e),
                                                tool_calls: Vec::new(),
                                            });
                                        }
                                    }
                                }
                            }
                            KeyCode::Up => {
                                // Scroll messages up
                                if let Some(chat) = &mut app.chat {
                                    chat.scroll_up();
                                }
                            }
                            KeyCode::Down => {
                                // Scroll messages down
                                if let Some(chat) = &mut app.chat {
                                    chat.scroll_down();
                                }
                            }
                            _ => {}
                        }
                    } else {
                        // Normal navigation mode
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('Q') => {
                                app.should_quit = true;
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if matches!(app.current_view, View::WorkflowRunning(_)) {
                                    app.navigate_workflow_down();
                                    app.update_workflow_scroll(30); // Estimate viewport height
                                } else if matches!(app.current_view, View::Tabs) {
                                    app.navigate_tab_down();
                                } else {
                                    app.next();
                                }
                            }
                            KeyCode::Up => {
                                if matches!(app.current_view, View::WorkflowRunning(_)) {
                                    app.navigate_workflow_up();
                                    app.update_workflow_scroll(30); // Estimate viewport height
                                } else if matches!(app.current_view, View::Tabs) {
                                    app.navigate_tab_up();
                                } else {
                                    app.previous();
                                }
                            }
                            KeyCode::Char('k') => {
                                if matches!(app.current_view, View::WorkflowRunning(_)) {
                                    app.navigate_workflow_up();
                                    app.update_workflow_scroll(30); // Estimate viewport height
                                } else if matches!(app.current_view, View::Tabs) {
                                    app.navigate_tab_up();
                                } else {
                                    app.previous();
                                }
                            }
                            KeyCode::Char('K') => {
                                // K: Kill workflow (in Tabs view)
                                if matches!(app.current_view, View::Tabs) {
                                    app.kill_current_tab();
                                }
                            }
                            KeyCode::Enter => {
                                match app.current_view {
                                    View::WorkflowList => app.view_workflow(),
                                    View::WorkflowEdit(_) => app.start_editing_field(),
                                    View::WorkflowRunning(_) => {
                                        app.toggle_selected_item();
                                        app.update_workflow_scroll(30); // Estimate viewport height
                                    }
                                    View::Tabs => app.toggle_tab_item(),
                                    _ => {}
                                }
                            }
                            KeyCode::Char(' ') => {
                                if matches!(app.current_view, View::WorkflowRunning(_)) {
                                    app.toggle_expand_all();
                                    app.update_workflow_scroll(30); // Estimate viewport height
                                } else if matches!(app.current_view, View::Tabs) {
                                    app.toggle_tab_expand_all();
                                }
                            }
                            KeyCode::PageUp | KeyCode::Left | KeyCode::Char('h') => {
                                if matches!(app.current_view, View::Tabs) {
                                    app.scroll_agent_messages_up();
                                }
                            }
                            KeyCode::PageDown | KeyCode::Right => {
                                if matches!(app.current_view, View::Tabs) {
                                    app.scroll_agent_messages_down();
                                }
                            }
                            KeyCode::Char('v') => {
                                if matches!(app.current_view, View::WorkflowList) {
                                    app.view_workflow();
                                }
                            }
                            KeyCode::Char('e') | KeyCode::Char('E') => {
                                if matches!(app.current_view, View::WorkflowDetail(_)) {
                                    app.edit_workflow();
                                } else if matches!(app.current_view, View::Tabs) {
                                    app.edit_current_tab();
                                }
                            }
                            KeyCode::Char('l') | KeyCode::Char('L') => {
                                match app.current_view {
                                    View::WorkflowDetail(_) | View::WorkflowEdit(_) => {
                                        app.launch_workflow_in_tab();
                                    }
                                    View::Tabs => {
                                        app.scroll_agent_messages_down();
                                    }
                                    _ => {}
                                }
                            }
                            KeyCode::Char('1') => {
                                if matches!(app.current_view, View::WorkflowRunning(_)) {
                                    app.toggle_expand_phases();
                                }
                            }
                            KeyCode::Char('2') => {
                                if matches!(app.current_view, View::WorkflowRunning(_)) {
                                    app.toggle_expand_tasks();
                                }
                            }
                            KeyCode::Char('3') => {
                                if matches!(app.current_view, View::WorkflowRunning(_)) {
                                    app.toggle_expand_agents();
                                }
                            }
                            KeyCode::Tab => {
                                // Tab navigation forward
                                if matches!(app.current_view, View::Tabs) {
                                    app.next_tab();
                                }
                            }
                            KeyCode::BackTab => {
                                // Shift+Tab navigation backward
                                if matches!(app.current_view, View::Tabs) {
                                    app.previous_tab();
                                }
                            }
                            KeyCode::Char('t') | KeyCode::Char('T') => {
                                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                                    // Ctrl+T: New tab - enter workflow selection mode
                                    app.in_new_tab_flow = true;
                                    app.current_view = View::WorkflowList;
                                    app.field_values.clear();
                                    app.selected = 0;
                                }
                            }
                            KeyCode::Char('w') | KeyCode::Char('W') => {
                                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                                    // Ctrl+W: Close tab
                                    if matches!(app.current_view, View::Tabs) {
                                        app.close_current_tab();
                                    }
                                }
                            }
                            KeyCode::Char('c') | KeyCode::Char('C') => {
                                // C: Close tab (in Tabs view)
                                if matches!(app.current_view, View::Tabs) {
                                    app.close_current_tab();
                                }
                            }
                            KeyCode::Char('r') | KeyCode::Char('R') => {
                                // R: Rerun workflow (in Tabs view)
                                if matches!(app.current_view, View::Tabs) {
                                    app.rerun_current_tab();
                                }
                            }
                            KeyCode::Char('a') | KeyCode::Char('A') => {
                                // A: Open AI chat interface
                                if matches!(app.current_view, View::Tabs) {
                                    app.open_chat();
                                }
                            }
                            KeyCode::Esc | KeyCode::Char('b') => {
                                // If in chat view, go back to Tabs
                                if matches!(app.current_view, View::Chat) {
                                    app.current_view = View::Tabs;
                                }
                                // If in new tab flow, return to Tabs view
                                else if app.in_new_tab_flow {
                                    app.in_new_tab_flow = false;
                                    app.current_view = View::Tabs;
                                    app.field_values.clear();
                                } else if !matches!(app.current_view, View::WorkflowList) {
                                    app.back_to_list();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        if app.should_quit {
            // Save session before quitting
            app.save_session();
            break;
        }
    }
    Ok(())
}
