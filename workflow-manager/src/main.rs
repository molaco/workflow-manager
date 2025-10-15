use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread;
use workflow_manager_sdk::{Workflow, WorkflowSource, WorkflowStatus, FieldType, WorkflowLog, WorkflowRuntime};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use serde::{Deserialize, Serialize};

mod chat;
mod runtime;
mod mcp_tools;
mod discovery;
mod models;
mod ui;

use chat::ChatInterface;
use runtime::ProcessBasedRuntime;
use models::*;

impl App {
    fn new() -> Self {
        let workflows = load_workflows();
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let history = load_history();

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
        match ProcessBasedRuntime::new() {
            Ok(runtime) => {
                let runtime_arc = Arc::new(runtime) as Arc<dyn WorkflowRuntime>;
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

    // Session persistence
    fn save_session(&self) {
        #[derive(Serialize)]
        struct SavedTab {
            workflow_idx: usize,
            workflow_name: String,
            instance_number: usize,
            field_values: HashMap<String, String>,
            status: String,
            saved_logs: Vec<String>,
        }

        let saved_tabs: Vec<SavedTab> = self.open_tabs.iter()
            .map(|t| SavedTab {
                workflow_idx: t.workflow_idx,
                workflow_name: t.workflow_name.clone(),
                instance_number: t.instance_number,
                field_values: t.field_values.clone(),
                status: format!("{:?}", t.status),
                saved_logs: {
                    let mut logs = Vec::new();
                    if let Ok(output) = t.workflow_output.lock() {
                        logs = output.clone();
                    }
                    logs
                },
            })
            .collect();

        if let Some(data_dir) = directories::ProjectDirs::from("", "", "workflow-manager") {
            let session_path = data_dir.data_dir().join("session.json");
            if let Ok(json) = serde_json::to_string_pretty(&saved_tabs) {
                let _ = std::fs::write(session_path, json);
            }
        }
    }

    fn restore_session(&mut self) {
        #[derive(Deserialize)]
        struct SavedTab {
            workflow_idx: usize,
            workflow_name: String,
            instance_number: usize,
            field_values: HashMap<String, String>,
            status: String,
            saved_logs: Vec<String>,
        }

        if let Some(data_dir) = directories::ProjectDirs::from("", "", "workflow-manager") {
            let session_path = data_dir.data_dir().join("session.json");
            if let Ok(json) = std::fs::read_to_string(session_path) {
                if let Ok(saved_tabs) = serde_json::from_str::<Vec<SavedTab>>(&json) {
                    for saved in saved_tabs {
                        if saved.workflow_idx >= self.workflows.len() {
                            continue;
                        }

                        let status = match saved.status.as_str() {
                            "Completed" => WorkflowStatus::Completed,
                            "Failed" => WorkflowStatus::Failed,
                            _ => WorkflowStatus::NotStarted,
                        };

                        let tab = WorkflowTab {
                            id: format!("restored_{}", saved.instance_number),
                            workflow_idx: saved.workflow_idx,
                            workflow_name: saved.workflow_name,
                            instance_number: saved.instance_number,
                            start_time: None,
                            status,
                            child_process: None,
                            exit_code: None,
                            workflow_phases: Arc::new(Mutex::new(Vec::new())),
                            workflow_output: Arc::new(Mutex::new(saved.saved_logs)),
                            field_values: saved.field_values,
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

                        self.open_tabs.push(tab);

                        // Update counter
                        let workflow = &self.workflows[saved.workflow_idx];
                        let counter = self.workflow_counters
                            .entry(workflow.info.id.clone())
                            .or_insert(0);
                        if saved.instance_number >= *counter {
                            *counter = saved.instance_number + 1;
                        }
                    }
                }
            }
        }
    }

    fn next(&mut self) {
        match self.current_view {
            View::WorkflowList => {
                if self.selected < self.workflows.len().saturating_sub(1) {
                    self.selected += 1;
                }
            }
            View::WorkflowEdit(idx) => {
                // Navigate to next field
                if let Some(workflow) = self.workflows.get(idx) {
                    if self.edit_field_index < workflow.info.fields.len().saturating_sub(1) {
                        self.edit_field_index += 1;
                    }
                }
            }
            _ => {}
        }
    }

    fn previous(&mut self) {
        match self.current_view {
            View::WorkflowList => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            View::WorkflowEdit(_) => {
                // Navigate to previous field
                if self.edit_field_index > 0 {
                    self.edit_field_index -= 1;
                }
            }
            _ => {}
        }
    }

    // Tab navigation
    fn next_tab(&mut self) {
        if !self.open_tabs.is_empty() {
            self.active_tab_idx = (self.active_tab_idx + 1) % self.open_tabs.len();
        }
    }

    fn previous_tab(&mut self) {
        if !self.open_tabs.is_empty() {
            self.active_tab_idx = if self.active_tab_idx == 0 {
                self.open_tabs.len() - 1
            } else {
                self.active_tab_idx - 1
            };
        }
    }

    // Tab management actions
    fn close_current_tab(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }

        let tab = &self.open_tabs[self.active_tab_idx];

        // If running, show confirmation
        if tab.status == WorkflowStatus::Running {
            self.show_close_confirmation = true;
            return;
        }

        // Close tab directly if not running
        self.close_tab_confirmed();
    }

    fn close_tab_confirmed(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }

        // Kill process if running
        if let Some(tab) = self.open_tabs.get_mut(self.active_tab_idx) {
            if let Some(mut child) = tab.child_process.take() {
                let _ = child.kill();
            }
        }

        // Remove tab
        self.open_tabs.remove(self.active_tab_idx);

        // Adjust active index
        if self.open_tabs.is_empty() {
            self.active_tab_idx = 0;
        } else if self.active_tab_idx >= self.open_tabs.len() {
            self.active_tab_idx = self.open_tabs.len() - 1;
        }

        self.show_close_confirmation = false;
    }

    fn kill_current_tab(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }

        if let Some(tab) = self.open_tabs.get_mut(self.active_tab_idx) {
            if let Some(mut child) = tab.child_process.take() {
                let _ = child.kill();
                tab.status = WorkflowStatus::Failed;
                if let Ok(mut output) = tab.workflow_output.lock() {
                    output.push(String::new());
                    output.push("⚠️ Workflow killed by user".to_string());
                }
            }
        }
    }

    fn rerun_current_tab(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }

        let tab = &mut self.open_tabs[self.active_tab_idx];

        // Kill existing process if running
        if let Some(mut child) = tab.child_process.take() {
            let _ = child.kill();
        }

        // Get workflow info
        let workflow_idx = tab.workflow_idx;
        let field_values = tab.field_values.clone();

        let workflow = match self.workflows.get(workflow_idx) {
            Some(w) => w,
            None => return,
        };

        let workflow_id = &workflow.info.id;
        let binary_path = PathBuf::from("../target/debug").join(workflow_id);

        // Build command arguments from saved field values
        let mut args = Vec::new();
        for field in &workflow.info.fields {
            if let Some(value) = field_values.get(&field.name) {
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

        // Reset tab state
        tab.status = WorkflowStatus::Running;
        tab.exit_code = None;
        tab.start_time = Some(chrono::Local::now());

        // Clear output and phases
        if let Ok(mut output) = tab.workflow_output.lock() {
            output.clear();
        }
        if let Ok(mut phases) = tab.workflow_phases.lock() {
            phases.clear();
        }

        // Spawn new process
        match std::process::Command::new(&binary_path)
            .args(&args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
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
                                if let Some(json_str) = line.strip_prefix("__WF_EVENT__:") {
                                    if let Ok(event) = serde_json::from_str::<WorkflowLog>(json_str) {
                                        Self::handle_workflow_event(event, &phases);
                                    }
                                } else {
                                    if let Ok(mut output) = output.lock() {
                                        output.push(format!("ERROR: {}", line));
                                    }
                                }
                            }
                        }
                    });
                }

                tab.child_process = Some(child);
            }
            Err(e) => {
                tab.status = WorkflowStatus::Failed;
                if let Ok(mut output) = tab.workflow_output.lock() {
                    output.push(format!("❌ Failed to rerun workflow: {}", e));
                    output.push(format!("   Binary path: {}", binary_path.display()));
                    output.push(format!("   Args: {:?}", args));
                }
            }
        }
    }

    // Tab navigation methods
    fn navigate_tab_down(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }
        let tab = &mut self.open_tabs[self.active_tab_idx];

        if let Ok(phases) = tab.workflow_phases.lock() {
            if phases.is_empty() {
                return;
            }

            let mut just_exited_agent = false;

            // If agent is selected, try to move to next agent
            if let Some(ref agent_id) = tab.selected_agent.clone() {
                if let Some(ref task_id) = tab.selected_task {
                    if let Some(phase) = phases.get(tab.selected_phase) {
                        if let Some(task) = phase.tasks.iter().find(|t| &t.id == task_id) {
                            if let Some(agent_idx) = task.agents.iter().position(|a| &a.id == agent_id) {
                                if agent_idx + 1 < task.agents.len() {
                                    // Move to next agent in same task
                                    tab.selected_agent = Some(task.agents[agent_idx + 1].id.clone());
                                    return;
                                }
                            }
                        }
                    }
                }
                // No next agent, move to next task
                tab.selected_agent = None;
                just_exited_agent = true;
            }

            // If task is selected, try to move to next task or dive into agents
            if let Some(ref task_id) = tab.selected_task.clone() {
                if let Some(phase) = phases.get(tab.selected_phase) {
                    if let Some(task_idx) = phase.tasks.iter().position(|t| &t.id == task_id) {
                        let task = &phase.tasks[task_idx];

                        // If task is expanded and has agents, dive into first agent (but only if we didn't just exit an agent)
                        if !just_exited_agent && tab.expanded_tasks.contains(task_id) && !task.agents.is_empty() {
                            tab.selected_agent = Some(task.agents[0].id.clone());
                            return;
                        }

                        // Move to next task in same phase
                        if task_idx + 1 < phase.tasks.len() {
                            tab.selected_task = Some(phase.tasks[task_idx + 1].id.clone());
                            return;
                        }
                    }
                }
                // No next task in this phase, move to next phase
                tab.selected_task = None;
                if tab.selected_phase + 1 < phases.len() {
                    tab.selected_phase += 1;
                }
                return;
            }

            // Navigate phases or dive into tasks
            let phase = &phases[tab.selected_phase];

            // If current phase is expanded and has tasks, dive into first task
            if tab.expanded_phases.contains(&tab.selected_phase) && !phase.tasks.is_empty() {
                tab.selected_task = Some(phase.tasks[0].id.clone());
                return;
            }

            // Move to next phase
            if tab.selected_phase + 1 < phases.len() {
                tab.selected_phase += 1;
            }
        }
    }

    fn navigate_tab_up(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }
        let tab = &mut self.open_tabs[self.active_tab_idx];

        if let Ok(phases) = tab.workflow_phases.lock() {
            if phases.is_empty() {
                return;
            }

            // If agent is selected, try to move to previous agent
            if let Some(ref agent_id) = tab.selected_agent.clone() {
                if let Some(ref task_id) = tab.selected_task {
                    if let Some(phase) = phases.get(tab.selected_phase) {
                        if let Some(task) = phase.tasks.iter().find(|t| &t.id == task_id) {
                            if let Some(agent_idx) = task.agents.iter().position(|a| &a.id == agent_id) {
                                if agent_idx > 0 {
                                    // Move to previous agent
                                    tab.selected_agent = Some(task.agents[agent_idx - 1].id.clone());
                                    return;
                                } else {
                                    // Move back to task level
                                    tab.selected_agent = None;
                                    return;
                                }
                            }
                        }
                    }
                }
            }

            // If task is selected, try to move to previous task
            if let Some(ref task_id) = tab.selected_task.clone() {
                if let Some(phase) = phases.get(tab.selected_phase) {
                    if let Some(task_idx) = phase.tasks.iter().position(|t| &t.id == task_id) {
                        if task_idx > 0 {
                            // Move to previous task
                            let prev_task = &phase.tasks[task_idx - 1];
                            tab.selected_task = Some(prev_task.id.clone());

                            // If previous task is expanded and has agents, select last agent
                            if tab.expanded_tasks.contains(&prev_task.id) && !prev_task.agents.is_empty() {
                                tab.selected_agent = Some(prev_task.agents.last().unwrap().id.clone());
                            }
                            return;
                        } else {
                            // Move back to phase level
                            tab.selected_task = None;
                            return;
                        }
                    }
                }
            }

            // Navigate phases
            if tab.selected_phase > 0 {
                tab.selected_phase -= 1;

                // If moving to previous phase that's expanded with tasks, select last task
                if let Some(phase) = phases.get(tab.selected_phase) {
                    if tab.expanded_phases.contains(&tab.selected_phase) && !phase.tasks.is_empty() {
                        let last_task = phase.tasks.last().unwrap();
                        tab.selected_task = Some(last_task.id.clone());

                        // If last task is expanded with agents, select last agent
                        if tab.expanded_tasks.contains(&last_task.id) && !last_task.agents.is_empty() {
                            tab.selected_agent = Some(last_task.agents.last().unwrap().id.clone());
                        }
                    }
                }
            }
        }
    }

    fn toggle_tab_item(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }
        let tab = &mut self.open_tabs[self.active_tab_idx];

        // If agent is selected, toggle agent expansion
        if let Some(ref agent_id) = tab.selected_agent {
            if tab.expanded_agents.contains(agent_id) {
                tab.expanded_agents.remove(agent_id);
            } else {
                tab.expanded_agents.insert(agent_id.clone());
            }
        }
        // If task is selected, toggle task expansion
        else if let Some(ref task_id) = tab.selected_task {
            if tab.expanded_tasks.contains(task_id) {
                tab.expanded_tasks.remove(task_id);
            } else {
                tab.expanded_tasks.insert(task_id.clone());
            }
        }
        // Otherwise, toggle phase expansion
        else {
            if tab.expanded_phases.contains(&tab.selected_phase) {
                tab.expanded_phases.remove(&tab.selected_phase);
            } else {
                tab.expanded_phases.insert(tab.selected_phase);
            }
        }
    }

    fn toggle_tab_expand_all(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }
        let tab = &mut self.open_tabs[self.active_tab_idx];

        if let Ok(phases) = tab.workflow_phases.lock() {
            let all_expanded = phases.iter().all(|p| tab.expanded_phases.contains(&p.id));

            if all_expanded {
                // Collapse all
                tab.expanded_phases.clear();
                tab.expanded_tasks.clear();
                tab.expanded_agents.clear();
            } else {
                // Expand all
                for phase in phases.iter() {
                    tab.expanded_phases.insert(phase.id);
                    for task in &phase.tasks {
                        tab.expanded_tasks.insert(task.id.clone());
                        for agent in &task.agents {
                            tab.expanded_agents.insert(agent.id.clone());
                        }
                    }
                }
            }
        }
    }

    fn scroll_agent_messages_up(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }
        let tab = &mut self.open_tabs[self.active_tab_idx];

        if let Some(ref agent_id) = tab.selected_agent {
            let offset = tab.agent_scroll_offsets.entry(agent_id.clone()).or_insert(0);
            if *offset > 0 {
                *offset -= 1;
            }
        }
    }

    fn scroll_agent_messages_down(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }
        let tab = &mut self.open_tabs[self.active_tab_idx];

        if let Some(ref agent_id) = tab.selected_agent {
            // Find the agent to check message count
            if let Ok(phases) = tab.workflow_phases.lock() {
                for phase in phases.iter() {
                    for task in &phase.tasks {
                        if let Some(agent) = task.agents.iter().find(|a| &a.id == agent_id) {
                            let offset = tab.agent_scroll_offsets.entry(agent_id.clone()).or_insert(0);
                            let window_size = 5;
                            let max_offset = agent.messages.len().saturating_sub(window_size);
                            if *offset < max_offset {
                                *offset += 1;
                            }
                            return;
                        }
                    }
                }
            }
        }
    }

    // Poll all running tabs for process status (output is read by threads)
    fn poll_all_tabs(&mut self) {
        for tab in &mut self.open_tabs {
            if tab.status != WorkflowStatus::Running {
                continue;
            }

            // Check process status
            if let Some(child) = &mut tab.child_process {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        tab.status = if status.success() {
                            WorkflowStatus::Completed
                        } else {
                            WorkflowStatus::Failed
                        };
                        tab.exit_code = status.code();

                        // Add completion message
                        if let Ok(mut output) = tab.workflow_output.lock() {
                            output.push(String::new());
                            if tab.status == WorkflowStatus::Completed {
                                output.push("✅ Workflow completed successfully".to_string());

                                // Save to history on success
                                if let Some(workflow) = self.workflows.get(tab.workflow_idx) {
                                    for (field_name, value) in &tab.field_values {
                                        if !value.is_empty() {
                                            let workflow_history = self.history.workflows
                                                .entry(workflow.info.id.clone())
                                                .or_insert_with(HashMap::new);

                                            let field_history = workflow_history
                                                .entry(field_name.clone())
                                                .or_insert_with(Vec::new);

                                            if !field_history.contains(value) {
                                                field_history.insert(0, value.clone());
                                                if field_history.len() > 10 {
                                                    field_history.truncate(10);
                                                }
                                            }
                                        }
                                    }
                                    let _ = save_history(&self.history);
                                }
                            } else {
                                output.push(format!("❌ Workflow failed with exit code: {:?}", tab.exit_code));
                            }
                        }
                    }
                    Ok(None) => {
                        // Still running - threads are reading output
                    }
                    Err(_) => {
                        tab.status = WorkflowStatus::Failed;
                        if let Ok(mut output) = tab.workflow_output.lock() {
                            output.push("❌ Error checking workflow status".to_string());
                        }
                    }
                }
            }
        }
    }

    fn view_workflow(&mut self) {
        if self.selected < self.workflows.len() {
            self.load_latest_values_from_history(self.selected);
            self.current_view = View::WorkflowDetail(self.selected);
        }
    }

    fn back_to_list(&mut self) {
        self.current_view = View::WorkflowList;
        self.field_values.clear();
    }

    fn open_chat(&mut self) {
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

    fn edit_workflow(&mut self) {
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

    fn edit_current_tab(&mut self) {
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

    fn start_editing_field(&mut self) {
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


    fn save_edited_field(&mut self) {
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

    fn cancel_editing(&mut self) {
        self.is_editing = false;
        self.edit_buffer.clear();
    }

    fn launch_workflow(&mut self) {
        use std::process::{Command, Stdio};
        use std::io::{BufRead, BufReader};

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
    fn launch_workflow_in_tab(&mut self) {
        use std::process::{Command, Stdio};

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

    fn handle_workflow_event(event: WorkflowLog, phases: &Arc<Mutex<Vec<WorkflowPhase>>>) {
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

    fn open_file_browser(&mut self) {
        // Get the current field type
        if let View::WorkflowEdit(idx) = self.current_view {
            if let Some(workflow) = self.workflows.get(idx) {
                if let Some(field) = workflow.info.fields.get(self.edit_field_index) {
                    // Only open for file_path and state_file fields
                    if matches!(field.field_type, FieldType::FilePath { .. } | FieldType::StateFile { .. }) {
                        self.show_file_browser = true;
                        self.file_browser_search.clear();
                        self.load_file_browser_items();
                    }
                }
            }
        }
    }

    fn close_file_browser(&mut self) {
        self.show_file_browser = false;
        self.file_browser_items.clear();
        self.file_browser_selected = 0;
        self.file_browser_search.clear();
    }

    fn load_file_browser_items(&mut self) {
        let base_dir = if self.edit_buffer.is_empty() {
            self.current_dir.clone()
        } else {
            let path = if PathBuf::from(&self.edit_buffer).is_absolute() {
                PathBuf::from(&self.edit_buffer)
            } else {
                self.current_dir.join(&self.edit_buffer)
            };

            // If path is a directory, use it directly
            // If path is a file (or doesn't exist), use its parent
            if path.is_dir() {
                path
            } else {
                path.parent().unwrap_or(&self.current_dir).to_path_buf()
            }
        };

        let mut items = Vec::new();

        // Add parent directory
        if let Some(parent) = base_dir.parent() {
            items.push(parent.to_path_buf());
        }

        // Read directory
        if let Ok(entries) = std::fs::read_dir(&base_dir) {
            for entry in entries.flatten() {
                items.push(entry.path());
            }
        }

        // Sort: directories first, then files
        items.sort_by(|a, b| {
            match (a.is_dir(), b.is_dir()) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.file_name().cmp(&b.file_name()),
            }
        });

        self.file_browser_items = items;
        self.file_browser_selected = 0;
    }

    fn file_browser_next(&mut self) {
        if self.file_browser_selected < self.file_browser_items.len().saturating_sub(1) {
            self.file_browser_selected += 1;
        }
    }

    fn file_browser_previous(&mut self) {
        if self.file_browser_selected > 0 {
            self.file_browser_selected -= 1;
        }
    }

    fn file_browser_select(&mut self) {
        if let Some(path) = self.file_browser_items.get(self.file_browser_selected) {
            if path.is_dir() {
                // Navigate into directory
                self.current_dir = path.clone();
                self.edit_buffer = path.to_string_lossy().to_string();
                self.load_file_browser_items();
            } else {
                // Select file
                self.edit_buffer = path.to_string_lossy().to_string();
                self.close_file_browser();
            }
        }
    }

    fn complete_path(&mut self) {
        // Show dropdown with matching paths
        let partial = self.edit_buffer.clone();

        let (base_dir, prefix_str) = if partial.is_empty() {
            (self.current_dir.clone(), String::new())
        } else if partial.ends_with('/') || partial.ends_with('\\') {
            // Path ends with slash - we're inside a directory, show all contents
            let path = PathBuf::from(&partial);
            let dir = if path.is_absolute() {
                path
            } else {
                self.current_dir.join(path)
            };
            (dir, String::new())
        } else {
            let path = PathBuf::from(&partial);
            if let Some(parent) = path.parent() {
                let dir = if parent.as_os_str().is_empty() {
                    self.current_dir.clone()
                } else if path.is_absolute() {
                    parent.to_path_buf()
                } else {
                    self.current_dir.join(parent)
                };
                let prefix = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                (dir, prefix)
            } else {
                (self.current_dir.clone(), partial)
            }
        };

        let prefix = prefix_str.as_str();

        if let Ok(entries) = std::fs::read_dir(&base_dir) {
            let mut matches: Vec<PathBuf> = entries
                .flatten()
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .map(|s| s.starts_with(prefix))
                        .unwrap_or(false)
                })
                .map(|e| e.path())
                .collect();

            // Sort: directories first, then files
            matches.sort_by(|a, b| {
                match (a.is_dir(), b.is_dir()) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.file_name().cmp(&b.file_name()),
                }
            });

            // Add parent directory as first item (if it exists)
            if let Some(parent) = base_dir.parent() {
                matches.insert(0, parent.to_path_buf());
            }

            if !matches.is_empty() {
                self.dropdown_items = matches;
                self.dropdown_selected = 0;
                self.show_dropdown = true;
            }
        }
    }

    fn dropdown_next(&mut self) {
        if self.dropdown_selected < self.dropdown_items.len().saturating_sub(1) {
            self.dropdown_selected += 1;
        }
    }

    fn dropdown_previous(&mut self) {
        if self.dropdown_selected > 0 {
            self.dropdown_selected -= 1;
        }
    }

    fn dropdown_select(&mut self) {
        // Check if we're showing history or file paths
        if !self.history_items.is_empty() {
            // History dropdown
            if let Some(value) = self.history_items.get(self.dropdown_selected) {
                self.edit_buffer = value.clone();
                self.close_dropdown();
            }
        } else if let Some(path) = self.dropdown_items.get(self.dropdown_selected) {
            // File path dropdown
            let mut path_str = path.to_string_lossy().to_string();

            if path.is_dir() {
                // For directories, ensure trailing slash
                if !path_str.ends_with('/') && !path_str.ends_with('\\') {
                    path_str.push('/');
                }
                self.edit_buffer = path_str;
                self.complete_path();
            } else {
                // For files, close the dropdown
                self.edit_buffer = path_str;
                self.close_dropdown();
            }
        }
    }

    fn close_dropdown(&mut self) {
        self.show_dropdown = false;
        self.dropdown_items.clear();
        self.dropdown_selected = 0;
        self.history_items.clear();
    }

    fn show_history_dropdown(&mut self) {
        // Get current workflow and field
        if let View::WorkflowEdit(idx) = self.current_view {
            if let Some(workflow) = self.workflows.get(idx) {
                if let Some(field) = workflow.info.fields.get(self.edit_field_index) {
                    // Get history for this workflow + field
                    if let Some(workflow_history) = self.history.workflows.get(&workflow.info.id) {
                        if let Some(field_history) = workflow_history.get(&field.name) {
                            if !field_history.is_empty() {
                                self.history_items = field_history.clone();
                                self.dropdown_selected = 0;
                                self.show_dropdown = true;
                            }
                        }
                    }
                }
            }
        }
    }

    fn load_latest_values_from_history(&mut self, workflow_idx: usize) {
        // Load most recent values from history for this workflow
        if let Some(workflow) = self.workflows.get(workflow_idx) {
            let workflow_id = &workflow.info.id;

            if let Some(workflow_history) = self.history.workflows.get(workflow_id) {
                // Load the most recent value for each field
                for field in &workflow.info.fields {
                    if let Some(field_history) = workflow_history.get(&field.name) {
                        if let Some(latest_value) = field_history.first() {
                            self.field_values.insert(field.name.clone(), latest_value.clone());
                        }
                    }
                }
            }
        }
    }

    fn save_to_history(&mut self) {
        // Save current field values to history when launching workflow
        let idx = match self.current_view {
            View::WorkflowEdit(idx) | View::WorkflowDetail(idx) => idx,
            _ => return,
        };

        if let Some(workflow) = self.workflows.get(idx) {
            let workflow_id = workflow.info.id.clone();

            // Get or create workflow history
            let workflow_history = self.history.workflows
                .entry(workflow_id)
                .or_insert_with(HashMap::new);

            // Save each field value
            for (field_name, value) in &self.field_values {
                if !value.is_empty() {
                    let field_history = workflow_history
                        .entry(field_name.clone())
                        .or_insert_with(Vec::new);

                    // Add to history if not already present
                    if !field_history.contains(value) {
                        field_history.insert(0, value.clone());

                        // Keep max 10 history items
                        if field_history.len() > 10 {
                            field_history.truncate(10);
                        }
                    } else {
                        // Move to front
                        if let Some(pos) = field_history.iter().position(|v| v == value) {
                            let val = field_history.remove(pos);
                            field_history.insert(0, val);
                        }
                    }
                }
            }

            // Save history to file
            let _ = save_history(&self.history);
        }
    }

    fn navigate_workflow_down(&mut self) {
        if let Ok(phases) = self.workflow_phases.lock() {
            if phases.is_empty() {
                return;
            }

            let mut just_exited_agent = false;

            // If we're on an agent, try to move to next agent in same task
            if let Some(ref agent_id) = self.selected_agent {
                let phase = &phases[self.selected_phase];
                if let Some(task) = phase.tasks.iter().find(|t| Some(&t.id) == self.selected_task.as_ref()) {
                    let current_idx = task.agents.iter().position(|a| &a.id == agent_id);
                    if let Some(idx) = current_idx {
                        if idx + 1 < task.agents.len() {
                            self.selected_agent = Some(task.agents[idx + 1].id.clone());
                            return;
                        }
                    }
                }
                // No more agents, move to next task
                self.selected_agent = None;
                just_exited_agent = true;
            }

            // If we're on a task, try to move to next task in same phase or first agent if expanded
            if let Some(ref task_id) = self.selected_task {
                let phase = &phases[self.selected_phase];

                // Check if task is expanded and has agents (but only if we didn't just exit an agent)
                if !just_exited_agent && self.expanded_tasks.contains(task_id) {
                    if let Some(task) = phase.tasks.iter().find(|t| &t.id == task_id) {
                        if !task.agents.is_empty() {
                            self.selected_agent = Some(task.agents[0].id.clone());
                            return;
                        }
                    }
                }

                // Move to next task
                let current_idx = phase.tasks.iter().position(|t| &t.id == task_id);
                if let Some(idx) = current_idx {
                    if idx + 1 < phase.tasks.len() {
                        self.selected_task = Some(phase.tasks[idx + 1].id.clone());
                        return;
                    }
                }
                // No more tasks, move to next phase
                self.selected_task = None;
            }

            // Move to next phase or first task if expanded
            if self.expanded_phases.contains(&self.selected_phase) {
                let phase = &phases[self.selected_phase];
                if !phase.tasks.is_empty() && self.selected_task.is_none() {
                    self.selected_task = Some(phase.tasks[0].id.clone());
                    return;
                }
            }

            if self.selected_phase + 1 < phases.len() {
                self.selected_phase += 1;
                self.selected_task = None;
                self.selected_agent = None;
            }
        }
    }

    fn navigate_workflow_up(&mut self) {
        if let Ok(phases) = self.workflow_phases.lock() {
            if phases.is_empty() {
                return;
            }

            // If we're on an agent, try to move to previous agent
            if let Some(ref agent_id) = self.selected_agent {
                let phase = &phases[self.selected_phase];
                if let Some(task) = phase.tasks.iter().find(|t| Some(&t.id) == self.selected_task.as_ref()) {
                    let current_idx = task.agents.iter().position(|a| &a.id == agent_id);
                    if let Some(idx) = current_idx {
                        if idx > 0 {
                            self.selected_agent = Some(task.agents[idx - 1].id.clone());
                            return;
                        }
                    }
                }
                // At first agent, move back to task
                self.selected_agent = None;
                return;
            }

            // If we're on a task, try to move to previous task or last agent of previous task
            if let Some(ref task_id) = self.selected_task {
                let phase = &phases[self.selected_phase];
                let current_idx = phase.tasks.iter().position(|t| &t.id == task_id);
                if let Some(idx) = current_idx {
                    if idx > 0 {
                        let prev_task = &phase.tasks[idx - 1];
                        self.selected_task = Some(prev_task.id.clone());
                        // If previous task is expanded with agents, jump to last agent
                        if self.expanded_tasks.contains(&prev_task.id) && !prev_task.agents.is_empty() {
                            self.selected_agent = Some(prev_task.agents[prev_task.agents.len() - 1].id.clone());
                        }
                        return;
                    }
                }
                // At first task, move back to phase
                self.selected_task = None;
                return;
            }

            // Move to previous phase or last task if expanded
            if self.selected_phase > 0 {
                self.selected_phase -= 1;
                self.selected_task = None;
                self.selected_agent = None;

                // If new phase is expanded and has tasks, jump to last task
                if self.expanded_phases.contains(&self.selected_phase) {
                    let phase = &phases[self.selected_phase];
                    if !phase.tasks.is_empty() {
                        let last_task = &phase.tasks[phase.tasks.len() - 1];
                        self.selected_task = Some(last_task.id.clone());
                        // If last task is expanded with agents, jump to last agent
                        if self.expanded_tasks.contains(&last_task.id) && !last_task.agents.is_empty() {
                            self.selected_agent = Some(last_task.agents[last_task.agents.len() - 1].id.clone());
                        }
                    }
                }
            }
        }
    }

    fn toggle_selected_item(&mut self) {
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

    fn toggle_expand_all(&mut self) {
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

    fn update_workflow_scroll(&mut self, viewport_height: usize) {
        // Calculate which line the selected item is on and adjust scroll to keep it visible
        if let Ok(phases) = self.workflow_phases.lock() {
            let mut current_line = 0;
            let mut selected_line = 0;

            for phase in phases.iter() {
                // Check if this phase is selected
                if self.selected_phase == phase.id && self.selected_task.is_none() && self.selected_agent.is_none() {
                    selected_line = current_line;
                }
                current_line += 1; // Phase header

                if self.expanded_phases.contains(&phase.id) {
                    for task in &phase.tasks {
                        // Check if this task is selected
                        if self.selected_phase == phase.id && Some(&task.id) == self.selected_task.as_ref() && self.selected_agent.is_none() {
                            selected_line = current_line;
                        }
                        current_line += 1; // Task header

                        if self.expanded_tasks.contains(&task.id) {
                            // Count task messages
                            current_line += task.messages.len();

                            for agent in &task.agents {
                                // Check if this agent is selected
                                if Some(&agent.id) == self.selected_agent.as_ref() {
                                    selected_line = current_line;
                                }
                                current_line += 1; // Agent header

                                if self.expanded_agents.contains(&agent.id) {
                                    current_line += agent.messages.len();
                                }
                            }
                        }
                    }

                    // Count output files
                    if !phase.output_files.is_empty() {
                        current_line += 1; // "Output files:" header
                        current_line += phase.output_files.len();
                    }
                }

                current_line += 1; // Empty line after phase
            }

            // Adjust scroll offset to keep selected line visible
            // Leave some padding at top and bottom
            let padding = 2;
            let visible_lines = viewport_height.saturating_sub(2); // Account for borders

            if selected_line < self.workflow_scroll_offset + padding {
                // Selected line is above visible area, scroll up
                self.workflow_scroll_offset = selected_line.saturating_sub(padding);
            } else if selected_line >= self.workflow_scroll_offset + visible_lines.saturating_sub(padding) {
                // Selected line is below visible area, scroll down
                self.workflow_scroll_offset = selected_line.saturating_sub(visible_lines.saturating_sub(padding).saturating_sub(1));
            }
        }
    }

    fn toggle_expand_phases(&mut self) {
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

    fn toggle_expand_tasks(&mut self) {
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

    fn toggle_expand_agents(&mut self) {
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

fn history_file_path() -> PathBuf {
    use directories::ProjectDirs;

    if let Some(proj_dirs) = ProjectDirs::from("com", "workflow-manager", "workflow-manager") {
        proj_dirs.data_dir().join("history.json")
    } else {
        PathBuf::from(".workflow-manager-history.json")
    }
}

fn load_history() -> WorkflowHistory {
    let path = history_file_path();
    if let Ok(content) = std::fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        WorkflowHistory::default()
    }
}

fn save_history(history: &WorkflowHistory) -> Result<()> {
    let path = history_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(history)?;
    std::fs::write(path, content)?;
    Ok(())
}

// Load built-in and discovered workflows
fn load_workflows() -> Vec<Workflow> {
    let mut workflows = Vec::new();

    // Load built-in workflows from src/workflows/
    workflows.extend(load_builtin_workflows());

    // Load user workflows from discovery
    workflows.extend(load_discovered_workflows());

    workflows
}

fn load_builtin_workflows() -> Vec<Workflow> {
    use std::process::Command;
    use workflow_manager_sdk::{FullWorkflowMetadata, WorkflowInfo};

    let target_dir = PathBuf::from("../target/debug");
    let mut workflows = Vec::new();

    // Automatically discover all workflow binaries in target/debug
    if !target_dir.exists() {
        return workflows;
    }

    let Ok(entries) = std::fs::read_dir(&target_dir) else {
        return workflows;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip directories
        if !path.is_file() {
            continue;
        }

        // Get filename
        let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        // Skip the TUI binary itself
        if filename == "workflow-manager" {
            continue;
        }

        // Skip build artifacts (files with extensions like .d, .rlib, etc.)
        if filename.contains('.') {
            continue;
        }

        // Skip if it looks like a hash suffix (has dash followed by hex)
        if filename.contains('-') {
            if let Some(after_dash) = filename.split('-').last() {
                // If after the last dash looks like a hash (long hex string), skip it
                if after_dash.len() > 10 && after_dash.chars().all(|c| c.is_ascii_hexdigit()) {
                    continue;
                }
            }
        }

        // Check if executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = path.metadata() {
                if metadata.permissions().mode() & 0o111 == 0 {
                    continue; // Not executable
                }
            }
        }

        // Call binary with --workflow-metadata to get its metadata
        if let Ok(output) = Command::new(&path)
            .arg("--workflow-metadata")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);

                // Parse JSON metadata from output
                if let Ok(full_metadata) = serde_json::from_str::<FullWorkflowMetadata>(&stdout) {
                    workflows.push(Workflow {
                        info: WorkflowInfo {
                            id: full_metadata.metadata.id.clone(),
                            name: full_metadata.metadata.name.clone(),
                            description: full_metadata.metadata.description.clone(),
                            status: WorkflowStatus::NotStarted,
                            metadata: full_metadata.metadata,
                            fields: full_metadata.fields,
                            progress_messages: vec![],
                        },
                        source: WorkflowSource::BuiltIn,
                    });
                }
            }
        }
    }

    workflows
}

fn load_discovered_workflows() -> Vec<Workflow> {
    // TODO: Use discovery.rs to find user workflows
    vec![]
}

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
                                // Tab navigation for Tabs view
                                if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                                    if matches!(app.current_view, View::Tabs) {
                                        app.previous_tab();
                                    }
                                } else {
                                    if matches!(app.current_view, View::Tabs) {
                                        app.next_tab();
                                    }
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
