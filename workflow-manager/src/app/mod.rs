//! Application state and module organization
//!
//! This module contains the main App struct and re-exports all functionality
//! organized by domain.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::chat::ChatInterface;

mod models;
pub use models::*;

// Declare submodules
mod file_browser;
mod history;
mod navigation;
mod tabs;
mod workflow_ops;

// Re-export methods from submodules

impl App {
    pub fn new() -> Self {
        let workflows = crate::utils::load_workflows();
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let history = crate::utils::load_history();

        // Create tokio runtime for async operations
        let tokio_runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

        let mut app = Self {
            workflows,
            // NEW: Tab management
            open_tabs: Vec::new(),
            active_tab_idx: 0,
            workflow_counters: HashMap::new(),
            show_close_confirmation: false,
            in_new_tab_flow: false,
            selected: 0,
            current_view: View::WorkflowList,
            should_quit: false,
            edit_field_index: 0,
            edit_buffer: String::new(),
            is_editing: false,
            field_values: HashMap::new(),
            show_file_browser: false,
            file_browser_items: Vec::new(),
            file_browser_selected: 0,
            file_browser_search: String::new(),
            current_dir,
            show_dropdown: false,
            dropdown_items: Vec::new(),
            dropdown_selected: 0,
            history,
            history_items: Vec::new(),
            workflow_output: Arc::new(Mutex::new(Vec::new())),
            workflow_running: false,
            workflow_phases: Arc::new(Mutex::new(Vec::new())),
            expanded_phases: HashSet::new(),
            expanded_tasks: HashSet::new(),
            expanded_agents: HashSet::new(),
            selected_phase: 0,
            selected_task: None,
            selected_agent: None,
            workflow_scroll_offset: 0,
            workflow_focused_pane: WorkflowPane::StructuredLogs,
            workflow_raw_output_scroll: 0,
            chat: None,
            runtime: None,
            tokio_runtime,
        };

        // Initialize runtime
        match crate::runtime::ProcessBasedRuntime::new() {
            Ok(runtime) => {
                let runtime_arc =
                    Arc::new(runtime) as Arc<dyn workflow_manager_sdk::WorkflowRuntime>;
                app.runtime = Some(runtime_arc.clone());

                // Wrap history for sharing with chat interface
                let history_arc = Arc::new(tokio::sync::Mutex::new(app.history.clone()));
                app.chat = Some(ChatInterface::new(
                    runtime_arc,
                    history_arc,
                    app.tokio_runtime.handle().clone(),
                ));
            }
            Err(e) => {
                eprintln!("Warning: Failed to initialize workflow runtime: {}", e);
            }
        }

        // Restore previous session
        app.restore_session();

        // Start in Tabs view (shows empty state with hint if no tabs)
        app.current_view = View::Tabs;

        app
    }

    pub fn open_chat(&mut self) {
        // Initialization happens automatically in background on startup
        self.current_view = View::Chat;
    }
}
