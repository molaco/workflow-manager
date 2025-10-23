//! Data types for the task planning workflow.
//!
//! This module defines all data structures used in the 3-step task planning process:
//!
//! 1. **Task Overview** - High-level strategic task descriptions
//! 2. **Detailed Tasks** - Complete implementation specifications
//! 3. **Execution Plan** - Batch execution with dependency analysis
//! 4. **Review Results** - Validation and review outcomes
//! 5. **Usage Statistics** - API usage tracking

use serde::{Deserialize, Serialize};

// ============================================================================
// Task Overview Types
// ============================================================================

/// High-level task overview from tasks_overview.yaml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOverview {
    /// Basic task information
    pub task: TaskInfo,

    /// Task dependencies
    #[serde(default)]
    pub dependencies: Dependencies,
}

/// Basic task information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    /// Unique task ID
    pub id: u32,

    /// Task name
    pub name: String,

    /// Task context/description
    #[serde(default)]
    pub context: String,
}

/// Task dependencies
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Dependencies {
    /// Tasks that must complete before this task
    #[serde(default)]
    pub requires_completion_of: Vec<TaskDependency>,
}

/// Single task dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDependency {
    /// ID of the required task
    pub task_id: u32,

    /// Reason for the dependency
    pub reason: String,
}

// ============================================================================
// Detailed Task Types
// ============================================================================

/// Complete detailed task specification from tasks.yaml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedTask {
    /// Basic task information
    pub task: TaskInfo,

    /// Files to create or modify
    #[serde(default)]
    pub files: Vec<FileSpec>,

    /// Functions, structs, and other code items
    #[serde(default)]
    pub functions: Vec<FunctionGroup>,

    /// Formal verification requirements
    pub formal_verification: FormalVerification,

    /// Test specifications
    pub tests: TestSpec,

    /// Task dependencies
    #[serde(default)]
    pub dependencies: Dependencies,
}

/// File specification - a file to create or modify
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSpec {
    /// Full path to the file
    pub path: String,

    /// Brief description of the file's role
    pub description: String,
}

/// Group of code items in a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionGroup {
    /// Path to the file
    pub file: String,

    /// Code items in this file
    #[serde(default)]
    pub items: Vec<CodeItem>,
}

/// Single code item (function, struct, trait, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeItem {
    /// Type of code item (function, struct, trait_impl, etc.)
    #[serde(rename = "type")]
    pub item_type: String,

    /// Name or signature of the item
    pub name: String,

    /// Description of purpose and behavior
    pub description: String,

    /// What must be true before execution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preconditions: Option<String>,

    /// What will be true after execution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postconditions: Option<String>,

    /// Properties that remain constant
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invariants: Option<String>,
}

/// Formal verification requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormalVerification {
    /// Whether formal verification is needed
    pub needed: bool,

    /// Verification level (None, Basic, Critical)
    pub level: String,

    /// Explanation of why verification is/isn't needed
    pub explanation: String,

    /// System prompt for verification (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,

    /// Formal properties to verify (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<Vec<String>>,

    /// Verification approach/strategy (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
}

/// Test specifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSpec {
    /// Test strategy and approach
    pub strategy: TestStrategy,

    /// Test implementation details
    pub implementation: TestImplementation,

    /// List of behaviors tested
    #[serde(default)]
    pub coverage: Vec<String>,
}

/// Test strategy and rationale
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestStrategy {
    /// Testing approach (unit tests, integration tests, etc.)
    pub approach: String,

    /// Rationale for the testing approach
    #[serde(default)]
    pub rationale: Vec<String>,
}

/// Test implementation details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestImplementation {
    /// Path to the test file
    pub file: String,

    /// Location (create new, append to existing, etc.)
    pub location: String,

    /// Complete test code
    pub code: String,
}

// ============================================================================
// Execution Plan Types
// ============================================================================

/// Complete execution plan for batch processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// Total number of tasks
    pub total_tasks: usize,

    /// Total number of batches
    pub total_batches: usize,

    /// List of batches
    #[serde(default)]
    pub batches: Vec<Batch>,

    /// Dependencies summary
    pub dependencies_summary: DependenciesSummary,
}

/// Single batch of tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Batch {
    /// Batch identifier
    pub batch_id: usize,

    /// Description of the batch
    pub description: String,

    /// Execution strategy (sequential, parallel, etc.)
    pub strategy: String,

    /// Tasks in this batch
    #[serde(default)]
    pub tasks: Vec<BatchTask>,

    /// Rationale for parallelization decisions
    pub parallelization_rationale: String,
}

/// Task reference in a batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchTask {
    /// Task ID
    pub task_id: u32,

    /// Task name
    pub task_name: String,

    /// Reason for including in this batch
    pub reason: String,
}

/// Dependencies summary for execution planning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependenciesSummary {
    /// Critical path (list of task IDs that must be sequential)
    #[serde(default)]
    pub critical_path: Vec<u32>,

    /// Parallelization potential (low, medium, high)
    pub parallelization_potential: String,

    /// Explanation of parallelization opportunities
    pub parallelization_explanation: String,
}

// ============================================================================
// Statistics Types
// ============================================================================

/// API usage statistics from Claude SDK
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    /// Total duration in milliseconds
    pub duration_ms: u64,

    /// API call duration in milliseconds
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_api_ms: Option<u64>,

    /// Number of conversation turns
    pub num_turns: u32,

    /// Total cost in USD
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_cost_usd: Option<f64>,

    /// Token usage details
    pub usage: TokenUsage,

    /// Session identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Token usage counts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Number of input tokens
    pub input_tokens: u32,

    /// Number of output tokens
    pub output_tokens: u32,
}

// ============================================================================
// Review Result Types
// ============================================================================

/// Single task review result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    /// Task ID being reviewed
    pub task_id: u32,

    /// Whether the review passed
    pub success: bool,

    /// List of issues found (empty if success=true)
    #[serde(default)]
    pub issues: Vec<String>,

    /// Summary of the review
    pub summary: String,
}
