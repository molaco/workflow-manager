//! Workflow utilities for standardized batch/task/agent execution
//!
//! This module provides reusable components for workflow phases:
//! - **batch**: Parallel execution with concurrency control
//! - **task**: Task-level logging and execution
//! - **agent**: Agent execution with stream handling and sub-agent detection
//! - **yaml**: YAML extraction, parsing, and validation

pub mod agent;
pub mod batch;
pub mod task;
pub mod yaml;

// Re-export commonly used types and functions
pub use agent::{execute_agent, AgentConfig};
pub use batch::{execute_batch, TaskContext};
pub use task::execute_task;
pub use yaml::{clean_yaml, extract_yaml, parse_yaml, parse_yaml_multi, validate_yaml_syntax};
