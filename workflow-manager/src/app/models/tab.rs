//! Tab state management

use std::collections::{HashMap, HashSet};
use std::process::Child;
use std::sync::{Arc, Mutex};
use workflow_manager_sdk::WorkflowStatus;

use super::workflow::WorkflowPhase;

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
