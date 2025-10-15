//! Application state and module organization
//!
//! This module contains the main App struct and re-exports all functionality
//! organized by domain.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::Child;
use std::sync::{Arc, Mutex};
use workflow_manager_sdk::{Workflow, WorkflowStatus};

use crate::chat::ChatInterface;

/// History storage: workflow_id -> field_name -> list of values
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowHistory {
    pub workflows: HashMap<String, HashMap<String, Vec<String>>>,
}

/// Status of a workflow phase
#[derive(Debug, Clone, PartialEq)]
pub enum PhaseStatus {
    NotStarted,
    Running,
    Completed,
    Failed,
}

/// Status of a workflow task
#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    NotStarted,
    Running,
    Completed,
    Failed,
}

/// Status of a workflow agent
#[derive(Debug, Clone, PartialEq)]
pub enum AgentStatus {
    NotStarted,
    Running,
    Completed,
    Failed,
}

/// A workflow agent that executes within a task
#[derive(Debug, Clone)]
pub struct WorkflowAgent {
    pub id: String, // task_id:agent_name
    pub task_id: String,
    pub name: String,
    pub description: String,
    pub status: AgentStatus,
    pub messages: Vec<String>,
    pub result: Option<String>,
}

/// A task within a workflow phase
#[derive(Debug, Clone)]
pub struct WorkflowTask {
    pub id: String,
    pub phase: usize,
    pub description: String,
    pub status: TaskStatus,
    pub agents: Vec<WorkflowAgent>,
    pub messages: Vec<String>,
    pub result: Option<String>,
}

/// A phase of workflow execution
#[derive(Debug, Clone)]
pub struct WorkflowPhase {
    pub id: usize,
    pub name: String,
    pub status: PhaseStatus,
    pub tasks: Vec<WorkflowTask>,
    pub output_files: Vec<(String, String)>, // (path, description)
}

/// Per-tab state container for tabbed interface
#[derive(Debug)]
pub struct WorkflowTab {
    // Identity
    pub id: String,                               // Unique: "research_20251014_120000"
    pub workflow_idx: usize,                      // Index in App.workflows catalog
    pub workflow_name: String,                    // "Research Agent Workflow"
    pub instance_number: usize,                   // Counter for display: #1, #2, #3
    pub start_time: Option<chrono::DateTime<chrono::Local>>,

    // Execution state
    pub status: WorkflowStatus,
    pub child_process: Option<Child>,
    pub exit_code: Option<i32>,

    // Workflow data (per tab)
    pub workflow_phases: Arc<Mutex<Vec<WorkflowPhase>>>,
    pub workflow_output: Arc<Mutex<Vec<String>>>,
    pub field_values: HashMap<String, String>,

    // UI state (per tab)
    pub scroll_offset: usize,
    pub expanded_phases: HashSet<usize>,
    pub expanded_tasks: HashSet<String>,
    pub expanded_agents: HashSet<String>,
    pub selected_phase: usize,
    pub selected_task: Option<String>,
    pub selected_agent: Option<String>,
    pub agent_scroll_offsets: HashMap<String, usize>,  // agent_id -> scroll offset

    // Session persistence
    pub saved_logs: Option<Vec<String>>,
}

/// Application view/route
#[derive(Debug, Clone, PartialEq)]
pub enum View {
    WorkflowList,
    WorkflowDetail(usize), // workflow index
    WorkflowEdit(usize),   // workflow index
    WorkflowRunning(usize), // workflow index (will be deprecated)
    Tabs,                  // Main tabbed view
    Chat,                  // Chat interface with Claude
}

/// Main application state
pub struct App {
    pub workflows: Vec<Workflow>,

    // Tab management
    pub open_tabs: Vec<WorkflowTab>,
    pub active_tab_idx: usize,
    pub workflow_counters: HashMap<String, usize>,
    pub show_close_confirmation: bool,
    pub in_new_tab_flow: bool,  // When true, we're selecting workflow for a new tab

    pub selected: usize,
    pub current_view: View,
    pub should_quit: bool,

    // Edit mode state
    pub edit_field_index: usize,
    pub edit_buffer: String,
    pub is_editing: bool,
    pub field_values: HashMap<String, String>,

    // File browser state
    pub show_file_browser: bool,
    pub file_browser_items: Vec<PathBuf>,
    pub file_browser_selected: usize,
    pub file_browser_search: String,
    pub current_dir: PathBuf,

    // Dropdown state
    pub show_dropdown: bool,
    pub dropdown_items: Vec<PathBuf>,
    pub dropdown_selected: usize,

    // History
    pub history: WorkflowHistory,
    pub history_items: Vec<String>,

    // Running workflow state
    pub workflow_output: Arc<Mutex<Vec<String>>>,
    pub workflow_running: bool,

    // Hierarchical phase tracking
    pub workflow_phases: Arc<Mutex<Vec<WorkflowPhase>>>,
    pub expanded_phases: HashSet<usize>,
    pub expanded_tasks: HashSet<String>,
    pub expanded_agents: HashSet<String>,

    // Navigation state for workflow running view
    pub selected_phase: usize,
    pub selected_task: Option<String>,
    pub selected_agent: Option<String>,
    pub workflow_scroll_offset: usize,

    // Chat interface
    pub chat: Option<ChatInterface>,
    pub runtime: Option<Arc<dyn workflow_manager_sdk::WorkflowRuntime>>,
    pub chat_initialized: bool,

    // Tokio runtime for async operations
    pub tokio_runtime: tokio::runtime::Runtime,
}

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
