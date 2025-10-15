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
mod tabs;
mod navigation;
mod file_browser;
mod history;
mod workflow_ops;

// Re-export methods from submodules
pub use tabs::*;
pub use navigation::*;
pub use file_browser::*;
pub use history::*;
pub use workflow_ops::*;

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
            chat: None,
            runtime: None,
            chat_initialized: false,
            tokio_runtime,
        };

        // Initialize runtime
        match crate::runtime::ProcessBasedRuntime::new() {
            Ok(runtime) => {
                let runtime_arc = Arc::new(runtime) as Arc<dyn workflow_manager_sdk::WorkflowRuntime>;
                app.runtime = Some(runtime_arc.clone());
                app.chat = Some(ChatInterface::new(runtime_arc));
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
        // Initialize chat if needed
        if !self.chat_initialized {
            if let Some(chat) = &mut self.chat {
                // Initialize Claude client asynchronously
                match self.tokio_runtime.block_on(chat.initialize()) {
                    Ok(_) => {
                        self.chat_initialized = true;
                    }
                    Err(e) => {
                        chat.init_error = Some(format!("Failed to initialize: {}", e));
                    }
                }
            }
        }
        self.current_view = View::Chat;
    }
}
