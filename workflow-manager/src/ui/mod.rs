//! UI rendering functions for the workflow manager TUI
//!
//! This module contains all the rendering logic for different views and components,
//! including workflow lists, edit forms, running workflows, tabs, and chat interface.

use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::Paragraph,
    Frame,
};

use crate::models::{App, View};

// Module declarations
mod chat_view;
mod components;
mod header_footer;
mod notifications;
mod tab_views;
mod workflow_views;

// Re-export public functions
pub use chat_view::render_chat;
pub use components::{render_dropdown, render_file_browser};
pub use header_footer::{render_footer, render_header};
pub use notifications::render_notifications;
pub use tab_views::{
    render_close_confirmation, render_empty_tabs, render_tab_bar, render_tab_content,
};
pub use workflow_views::{
    render_workflow_detail, render_workflow_edit, render_workflow_list, render_workflow_running,
};

/// Main UI rendering function - orchestrates all view rendering
pub fn ui(f: &mut Frame, app: &mut App) {
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
                Constraint::Length(3), // Tab bar
                Constraint::Min(0),    // Content
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

    // Notifications overlay (always render if there are active notifications)
    render_notifications(f, app, f.area());
}
