//! Workflow execution data structures

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
