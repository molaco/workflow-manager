//! Main application state

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use workflow_manager_sdk::Workflow;

use crate::chat::ChatInterface;
use super::{View, WorkflowHistory, WorkflowPhase, WorkflowTab};

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
