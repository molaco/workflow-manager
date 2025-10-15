//! Workflow operations and execution

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::io::{BufRead, BufReader};
use workflow_manager_sdk::{WorkflowStatus, WorkflowLog};

use super::*;

impl App {
    pub fn view_workflow(&mut self) {
        if self.selected < self.workflows.len() {
            self.load_latest_values_from_history(self.selected);
            self.current_view = View::WorkflowDetail(self.selected);
        }
    }

    pub fn back_to_list(&mut self) {
        self.current_view = View::WorkflowList;
        self.field_values.clear();
    }

    pub fn edit_workflow(&mut self) {
        if self.selected < self.workflows.len() {
            self.current_view = View::WorkflowEdit(self.selected);
            self.edit_field_index = 0;
            self.is_editing = false;
            self.field_values.clear();

            // Initialize field values with defaults
            if let Some(workflow) = self.workflows.get(self.selected) {
                for field in &workflow.info.fields {
                    if let Some(default) = &field.default {
                        self.field_values.insert(field.name.clone(), default.clone());
                    }
                }
            }

            // Load latest values from history (overrides defaults)
            self.load_latest_values_from_history(self.selected);
        }
    }

    pub fn edit_current_tab(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }

        let tab = &self.open_tabs[self.active_tab_idx];
        let workflow_idx = tab.workflow_idx;

        // Switch to edit view with the tab's current field values
        self.current_view = View::WorkflowEdit(workflow_idx);
        self.edit_field_index = 0;
        self.is_editing = false;

        // Load the tab's current field values
        self.field_values = tab.field_values.clone();

        // Keep track that we're editing from a tab
        self.in_new_tab_flow = true;
    }

    pub fn start_editing_field(&mut self) {
        if let View::WorkflowEdit(idx) = self.current_view {
            if let Some(workflow) = self.workflows.get(idx) {
                if let Some(field) = workflow.info.fields.get(self.edit_field_index) {
                    // Load current value into edit buffer
                    self.edit_buffer = self.field_values
                        .get(&field.name)
                        .cloned()
                        .unwrap_or_default();
                    self.is_editing = true;
                }
            }
        }
    }

    pub fn save_edited_field(&mut self) {
        if let View::WorkflowEdit(idx) = self.current_view {
            if let Some(workflow) = self.workflows.get(idx) {
                if let Some(field) = workflow.info.fields.get(self.edit_field_index) {
                    self.field_values.insert(field.name.clone(), self.edit_buffer.clone());
                }
            }
        }
        self.is_editing = false;
        self.edit_buffer.clear();
    }

    pub fn cancel_editing(&mut self) {
        self.is_editing = false;
        self.edit_buffer.clear();
    }

    pub fn launch_workflow(&mut self) {
        // Save field values to history
        self.save_to_history();

        // Get current workflow index
        let idx = match self.current_view {
            View::WorkflowEdit(idx) | View::WorkflowDetail(idx) => idx,
            _ => return,
        };

        if let Some(workflow) = self.workflows.get(idx) {
            let workflow_id = &workflow.info.id;
            let binary_path = PathBuf::from("../target/debug").join(workflow_id);

            // Build command arguments from field values
            let mut args = Vec::new();
            for field in &workflow.info.fields {
                if let Some(value) = self.field_values.get(&field.name) {
                    if !value.is_empty() {
                        // Convert field name to CLI arg format (e.g., "message" -> "--message")
                        let arg_name = format!("--{}", field.name.replace('_', "-"));

                        // For boolean flags, check if this looks like a bool field
                        // (description contains "[BOOL]" or value is "true"/"false")
                        if field.description.contains("[BOOL]") ||
                           value.eq_ignore_ascii_case("true") ||
                           value.eq_ignore_ascii_case("false") {
                            // Only add flag if value is "true" or non-empty
                            if value.eq_ignore_ascii_case("true") {
                                args.push(arg_name);
                            }
                        } else {
                            // Regular argument with value
                            args.push(arg_name);
                            args.push(value.clone());
                        }
                    }
                }
            }

            // Clear output and phase tracking
            if let Ok(mut output) = self.workflow_output.lock() {
                output.clear();
            }
            if let Ok(mut phases) = self.workflow_phases.lock() {
                phases.clear();
            }
            self.expanded_phases.clear();
            self.expanded_tasks.clear();
            self.expanded_agents.clear();
            self.workflow_running = true;
            self.current_view = View::WorkflowRunning(idx);

            // Spawn the workflow process
            match Command::new(&binary_path)
                    .args(&args)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                {
                    Ok(mut child) => {
                        let output_clone = Arc::clone(&self.workflow_output);

                        // Spawn thread to read stdout
                        if let Some(stdout) = child.stdout.take() {
                            let output = Arc::clone(&output_clone);
                            thread::spawn(move || {
                                let reader = BufReader::new(stdout);
                                for line in reader.lines() {
                                    if let Ok(line) = line {
                                        if let Ok(mut output) = output.lock() {
                                            output.push(line);
                                        }
                                    }
                                }
                            });
                        }

                        // Spawn thread to read stderr and parse structured logs
                        if let Some(stderr) = child.stderr.take() {
                            let output = Arc::clone(&output_clone);
                            let phases = Arc::clone(&self.workflow_phases);
                            thread::spawn(move || {
                                let reader = BufReader::new(stderr);
                                for line in reader.lines() {
                                    if let Ok(line) = line {
                                        // Check for structured log events
                                        if let Some(json_str) = line.strip_prefix("__WF_EVENT__:") {
                                            // Parse WorkflowLog event
                                            if let Ok(event) = serde_json::from_str::<WorkflowLog>(json_str) {
                                                Self::handle_workflow_event(event, &phases);
                                            }
                                        } else {
                                            // Regular stderr output
                                            if let Ok(mut output) = output.lock() {
                                                output.push(format!("ERROR: {}", line));
                                            }
                                        }
                                    }
                                }
                            });
                        }

                        // Spawn thread to wait for completion
                        let output = Arc::clone(&output_clone);
                        thread::spawn(move || {
                            match child.wait() {
                                Ok(status) => {
                                    if let Ok(mut output) = output.lock() {
                                        output.push(String::new());
                                        if status.success() {
                                            output.push("✅ Workflow completed successfully".to_string());
                                        } else {
                                            output.push(format!("❌ Workflow failed with exit code: {:?}", status.code()));
                                        }
                                    }
                                }
                                Err(e) => {
                                    if let Ok(mut output) = output.lock() {
                                        output.push(format!("❌ Error waiting for workflow: {}", e));
                                    }
                                }
                            }
                        });
                    }
                    Err(e) => {
                        if let Ok(mut output) = self.workflow_output.lock() {
                            output.push(format!("❌ Failed to launch workflow: {}", e));
                            output.push(format!("   Binary path: {}", binary_path.display()));
                            output.push(format!("   Args: {:?}", args));
                        }
                        self.workflow_running = false;
                    }
                }
        }
    }

    // New: Launch workflow in a tab (for tabbed interface)
    pub fn launch_workflow_in_tab(&mut self) {
        // Get current workflow index
        let idx = match self.current_view {
            View::WorkflowEdit(idx) | View::WorkflowDetail(idx) => idx,
            _ => return,
        };

        if let Some(workflow) = self.workflows.get(idx) {
            let workflow_id = &workflow.info.id;
            let binary_path = PathBuf::from("../target/debug").join(workflow_id);

            // Get next instance number
            let instance_number = {
                let counter = self.workflow_counters
                    .entry(workflow.info.id.clone())
                    .or_insert(0);
                *counter += 1;
                *counter
            };

            // Build the workflow binary first
            let build_output = Command::new("cargo")
                .args(&["build", "--bin", workflow_id])
                .current_dir("..")
                .output();

            match build_output {
                Ok(output) if !output.status.success() => {
                    // Build failed - create tab with error
                    let tab_id = format!("{}_{}", workflow.info.id, chrono::Local::now().format("%Y%m%d_%H%M%S"));
                    let mut tab = WorkflowTab {
                        id: tab_id,
                        workflow_idx: idx,
                        workflow_name: workflow.info.name.clone(),
                        instance_number,
                        start_time: Some(chrono::Local::now()),
                        status: WorkflowStatus::Failed,
                        child_process: None,
                        exit_code: None,
                        workflow_phases: Arc::new(Mutex::new(Vec::new())),
                        workflow_output: Arc::new(Mutex::new(Vec::new())),
                        field_values: self.field_values.clone(),
                        scroll_offset: 0,
                        expanded_phases: HashSet::new(),
                        expanded_tasks: HashSet::new(),
                        expanded_agents: HashSet::new(),
                        selected_phase: 0,
                        selected_task: None,
                        selected_agent: None,
                        agent_scroll_offsets: HashMap::new(),
                        saved_logs: None,
                    };

                    if let Ok(mut output_vec) = tab.workflow_output.lock() {
                        output_vec.push(format!("❌ Build failed for workflow: {}", workflow_id));
                        output_vec.push(String::new());
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        for line in stderr.lines() {
                            output_vec.push(line.to_string());
                        }
                    }

                    self.open_tabs.push(tab);
                    self.active_tab_idx = self.open_tabs.len() - 1;
                    self.current_view = View::Tabs;
                    self.in_new_tab_flow = false;
                    return;
                }
                Err(e) => {
                    // Cargo command failed to run
                    let tab_id = format!("{}_{}", workflow.info.id, chrono::Local::now().format("%Y%m%d_%H%M%S"));
                    let mut tab = WorkflowTab {
                        id: tab_id,
                        workflow_idx: idx,
                        workflow_name: workflow.info.name.clone(),
                        instance_number,
                        start_time: Some(chrono::Local::now()),
                        status: WorkflowStatus::Failed,
                        child_process: None,
                        exit_code: None,
                        workflow_phases: Arc::new(Mutex::new(Vec::new())),
                        workflow_output: Arc::new(Mutex::new(Vec::new())),
                        field_values: self.field_values.clone(),
                        scroll_offset: 0,
                        expanded_phases: HashSet::new(),
                        expanded_tasks: HashSet::new(),
                        expanded_agents: HashSet::new(),
                        selected_phase: 0,
                        selected_task: None,
                        selected_agent: None,
                        agent_scroll_offsets: HashMap::new(),
                        saved_logs: None,
                    };

                    if let Ok(mut output_vec) = tab.workflow_output.lock() {
                        output_vec.push(format!("❌ Failed to run cargo build: {}", e));
                    }

                    self.open_tabs.push(tab);
                    self.active_tab_idx = self.open_tabs.len() - 1;
                    self.current_view = View::Tabs;
                    self.in_new_tab_flow = false;
                    return;
                }
                _ => {
                    // Build succeeded, continue
                }
            }

            // Build command arguments from field values
            let mut args = Vec::new();
            for field in &workflow.info.fields {
                if let Some(value) = self.field_values.get(&field.name) {
                    if !value.is_empty() {
                        let arg_name = format!("--{}", field.name.replace('_', "-"));

                        if field.description.contains("[BOOL]") ||
                           value.eq_ignore_ascii_case("true") ||
                           value.eq_ignore_ascii_case("false") {
                            if value.eq_ignore_ascii_case("true") {
                                args.push(arg_name);
                            }
                        } else {
                            args.push(arg_name);
                            args.push(value.clone());
                        }
                    }
                }
            }

            // Create tab
            let tab_id = format!("{}_{}", workflow.info.id, chrono::Local::now().format("%Y%m%d_%H%M%S"));

            let mut tab = WorkflowTab {
                id: tab_id,
                workflow_idx: idx,
                workflow_name: workflow.info.name.clone(),
                instance_number,
                start_time: Some(chrono::Local::now()),
                status: WorkflowStatus::Running,
                child_process: None,
                exit_code: None,
                workflow_phases: Arc::new(Mutex::new(Vec::new())),
                workflow_output: Arc::new(Mutex::new(Vec::new())),
                field_values: self.field_values.clone(),
                scroll_offset: 0,
                expanded_phases: HashSet::new(),
                expanded_tasks: HashSet::new(),
                expanded_agents: HashSet::new(),
                selected_phase: 0,
                selected_task: None,
                selected_agent: None,
                agent_scroll_offsets: HashMap::new(),
                saved_logs: None,
            };

            // Spawn process
            match Command::new(&binary_path)
                .args(&args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(mut child) => {
                    let output_clone = Arc::clone(&tab.workflow_output);
                    let phases_clone = Arc::clone(&tab.workflow_phases);

                    // Spawn thread to read stdout
                    if let Some(stdout) = child.stdout.take() {
                        let output = Arc::clone(&output_clone);
                        thread::spawn(move || {
                            use std::io::BufRead;
                            let reader = std::io::BufReader::new(stdout);
                            for line in reader.lines() {
                                if let Ok(line) = line {
                                    if let Ok(mut output) = output.lock() {
                                        output.push(line);
                                    }
                                }
                            }
                        });
                    }

                    // Spawn thread to read stderr and parse structured logs
                    if let Some(stderr) = child.stderr.take() {
                        let output = Arc::clone(&output_clone);
                        let phases = phases_clone;
                        thread::spawn(move || {
                            use std::io::BufRead;
                            let reader = std::io::BufReader::new(stderr);
                            for line in reader.lines() {
                                if let Ok(line) = line {
                                    // Check for structured log events
                                    if let Some(json_str) = line.strip_prefix("__WF_EVENT__:") {
                                        if let Ok(event) = serde_json::from_str::<WorkflowLog>(json_str) {
                                            Self::handle_workflow_event(event, &phases);
                                        }
                                    } else {
                                        // Regular stderr output
                                        if let Ok(mut output) = output.lock() {
                                            output.push(format!("ERROR: {}", line));
                                        }
                                    }
                                }
                            }
                        });
                    }

                    tab.child_process = Some(child);

                    // Add tab and switch to it
                    self.open_tabs.push(tab);
                    self.active_tab_idx = self.open_tabs.len() - 1;
                    self.current_view = View::Tabs;
                    self.in_new_tab_flow = false;  // Exit new tab flow
                }
                Err(e) => {
                    // Show error in tab
                    tab.status = WorkflowStatus::Failed;
                    if let Ok(mut output) = tab.workflow_output.lock() {
                        output.push(format!("❌ Failed to launch workflow: {}", e));
                        output.push(format!("   Binary path: {}", binary_path.display()));
                        output.push(format!("   Args: {:?}", args));
                    }
                    self.open_tabs.push(tab);
                    self.active_tab_idx = self.open_tabs.len() - 1;
                    self.current_view = View::Tabs;
                    self.in_new_tab_flow = false;  // Exit new tab flow
                }
            }
        }
    }

    pub fn handle_workflow_event(event: WorkflowLog, phases: &Arc<Mutex<Vec<WorkflowPhase>>>) {
        if let Ok(mut phases) = phases.lock() {
            match event {
                WorkflowLog::PhaseStarted { phase, name, total_phases: _ } => {
                    // Ensure we have enough phases
                    let current_len = phases.len();
                    if current_len <= phase {
                        for i in current_len..=phase {
                            phases.push(WorkflowPhase {
                                id: i,
                                name: format!("Phase {}", i),
                                status: PhaseStatus::NotStarted,
                                tasks: Vec::new(),
                                output_files: Vec::new(),
                            });
                        }
                    }
                    if let Some(p) = phases.get_mut(phase) {
                        p.name = name;
                        p.status = PhaseStatus::Running;
                    }
                }
                WorkflowLog::PhaseCompleted { phase, name: _ } => {
                    if let Some(p) = phases.get_mut(phase) {
                        p.status = PhaseStatus::Completed;
                    }
                }
                WorkflowLog::PhaseFailed { phase, name: _, error: _ } => {
                    if let Some(p) = phases.get_mut(phase) {
                        p.status = PhaseStatus::Failed;
                    }
                }
                WorkflowLog::TaskStarted { phase, task_id, description, total_tasks: _ } => {
                    // Ensure phase exists
                    let current_len = phases.len();
                    if current_len <= phase {
                        for i in current_len..=phase {
                            phases.push(WorkflowPhase {
                                id: i,
                                name: format!("Phase {}", i),
                                status: PhaseStatus::NotStarted,
                                tasks: Vec::new(),
                                output_files: Vec::new(),
                            });
                        }
                    }
                    if let Some(p) = phases.get_mut(phase) {
                        // Find or create task
                        if let Some(task) = p.tasks.iter_mut().find(|t| t.id == task_id) {
                            task.status = TaskStatus::Running;
                        } else {
                            p.tasks.push(WorkflowTask {
                                id: task_id,
                                phase,
                                description,
                                status: TaskStatus::Running,
                                agents: Vec::new(),
                                messages: Vec::new(),
                                result: None,
                            });
                        }
                    }
                }
                WorkflowLog::TaskProgress { task_id, message } => {
                    // Find task in any phase
                    for phase in phases.iter_mut() {
                        if let Some(task) = phase.tasks.iter_mut().find(|t| t.id == task_id) {
                            task.messages.push(message.clone());
                            break;
                        }
                    }
                }
                WorkflowLog::TaskCompleted { task_id, result } => {
                    for phase in phases.iter_mut() {
                        if let Some(task) = phase.tasks.iter_mut().find(|t| t.id == task_id) {
                            task.status = TaskStatus::Completed;
                            task.result = result.clone();
                            break;
                        }
                    }
                }
                WorkflowLog::TaskFailed { task_id, error } => {
                    for phase in phases.iter_mut() {
                        if let Some(task) = phase.tasks.iter_mut().find(|t| t.id == task_id) {
                            task.status = TaskStatus::Failed;
                            task.messages.push(format!("Error: {}", error));
                            break;
                        }
                    }
                }
                WorkflowLog::AgentStarted { task_id, agent_name, description } => {
                    let agent_id = format!("{}:{}", task_id, agent_name);
                    for phase in phases.iter_mut() {
                        if let Some(task) = phase.tasks.iter_mut().find(|t| t.id == task_id) {
                            if let Some(agent) = task.agents.iter_mut().find(|a| a.id == agent_id) {
                                agent.status = AgentStatus::Running;
                            } else {
                                task.agents.push(WorkflowAgent {
                                    id: agent_id,
                                    task_id: task_id.clone(),
                                    name: agent_name,
                                    description,
                                    status: AgentStatus::Running,
                                    messages: Vec::new(),
                                    result: None,
                                });
                            }
                            break;
                        }
                    }
                }
                WorkflowLog::AgentMessage { task_id, agent_name, message } => {
                    let agent_id = format!("{}:{}", task_id, agent_name);
                    for phase in phases.iter_mut() {
                        if let Some(task) = phase.tasks.iter_mut().find(|t| t.id == task_id) {
                            if let Some(agent) = task.agents.iter_mut().find(|a| a.id == agent_id) {
                                agent.messages.push(message.clone());
                                break;
                            }
                        }
                    }
                }
                WorkflowLog::AgentCompleted { task_id, agent_name, result } => {
                    let agent_id = format!("{}:{}", task_id, agent_name);
                    for phase in phases.iter_mut() {
                        if let Some(task) = phase.tasks.iter_mut().find(|t| t.id == task_id) {
                            if let Some(agent) = task.agents.iter_mut().find(|a| a.id == agent_id) {
                                agent.status = AgentStatus::Completed;
                                agent.result = result.clone();
                                break;
                            }
                        }
                    }
                }
                WorkflowLog::AgentFailed { task_id, agent_name, error } => {
                    let agent_id = format!("{}:{}", task_id, agent_name);
                    for phase in phases.iter_mut() {
                        if let Some(task) = phase.tasks.iter_mut().find(|t| t.id == task_id) {
                            if let Some(agent) = task.agents.iter_mut().find(|a| a.id == agent_id) {
                                agent.status = AgentStatus::Failed;
                                agent.messages.push(format!("Error: {}", error));
                                break;
                            }
                        }
                    }
                }
                WorkflowLog::StateFileCreated { phase, file_path, description } => {
                    let current_len = phases.len();
                    if current_len <= phase {
                        for i in current_len..=phase {
                            phases.push(WorkflowPhase {
                                id: i,
                                name: format!("Phase {}", i),
                                status: PhaseStatus::NotStarted,
                                tasks: Vec::new(),
                                output_files: Vec::new(),
                            });
                        }
                    }
                    if let Some(p) = phases.get_mut(phase) {
                        p.output_files.push((file_path, description));
                    }
                }
            }
        }
    }

    pub fn toggle_selected_item(&mut self) {
        // Toggle expansion of currently selected item
        if let Some(ref agent_id) = self.selected_agent {
            if self.expanded_agents.contains(agent_id) {
                self.expanded_agents.remove(agent_id);
            } else {
                self.expanded_agents.insert(agent_id.clone());
            }
        } else if let Some(ref task_id) = self.selected_task {
            if self.expanded_tasks.contains(task_id) {
                self.expanded_tasks.remove(task_id);
            } else {
                self.expanded_tasks.insert(task_id.clone());
            }
        } else {
            // Toggle phase
            if self.expanded_phases.contains(&self.selected_phase) {
                self.expanded_phases.remove(&self.selected_phase);
            } else {
                self.expanded_phases.insert(self.selected_phase);
            }
        }
    }

    pub fn toggle_expand_all(&mut self) {
        // Toggle between fully expanded and fully collapsed
        if let Ok(phases) = self.workflow_phases.lock() {
            if self.expanded_phases.is_empty() && self.expanded_tasks.is_empty() && self.expanded_agents.is_empty() {
                // Expand all
                for phase in phases.iter() {
                    self.expanded_phases.insert(phase.id);
                    for task in &phase.tasks {
                        self.expanded_tasks.insert(task.id.clone());
                        for agent in &task.agents {
                            self.expanded_agents.insert(agent.id.clone());
                        }
                    }
                }
            } else {
                // Collapse all
                self.expanded_phases.clear();
                self.expanded_tasks.clear();
                self.expanded_agents.clear();
            }
        }
    }

    pub fn toggle_expand_phases(&mut self) {
        // Toggle all phases
        if let Ok(phases) = self.workflow_phases.lock() {
            if self.expanded_phases.len() == phases.len() {
                // All expanded, collapse all
                self.expanded_phases.clear();
            } else {
                // Expand all phases
                for phase in phases.iter() {
                    self.expanded_phases.insert(phase.id);
                }
            }
        }
    }

    pub fn toggle_expand_tasks(&mut self) {
        // Toggle all tasks in all phases
        if let Ok(phases) = self.workflow_phases.lock() {
            let total_tasks: usize = phases.iter().map(|p| p.tasks.len()).sum();
            if self.expanded_tasks.len() == total_tasks {
                // All expanded, collapse all
                self.expanded_tasks.clear();
            } else {
                // Expand all tasks
                for phase in phases.iter() {
                    for task in &phase.tasks {
                        self.expanded_tasks.insert(task.id.clone());
                    }
                }
            }
        }
    }

    pub fn toggle_expand_agents(&mut self) {
        // Toggle all agents in all tasks
        if let Ok(phases) = self.workflow_phases.lock() {
            let mut total_agents = 0;
            for phase in phases.iter() {
                for task in &phase.tasks {
                    total_agents += task.agents.len();
                }
            }

            if self.expanded_agents.len() == total_agents {
                // All expanded, collapse all
                self.expanded_agents.clear();
            } else {
                // Expand all agents
                for phase in phases.iter() {
                    for task in &phase.tasks {
                        for agent in &task.agents {
                            self.expanded_agents.insert(agent.id.clone());
                        }
                    }
                }
            }
        }
    }
}
