//! Task planning workflow orchestration.
//!
//! This module implements a multi-agent task planning system that transforms
//! high-level implementation requirements into detailed, executable task specifications.
//!
//! ## Module Structure
//!
//! - `types` - Data structures for tasks, plans, and results
//! - `utils` - Utility functions for file I/O and parsing
//! - `cli` - Command-line argument definitions
//! - `execution_plan` - Batch planning and dependency analysis
//! - `step1_overview` - Generate high-level task overview
//! - `step2_expand` - Expand tasks into detailed specifications
//! - `step3_review` - Review and validate expanded tasks
//! - `workflow` - Main workflow orchestration

pub mod cli;
pub mod execution_plan;
pub mod step1_overview;
pub mod step2_expand;
pub mod step3_review;
pub mod types;
pub mod utils;
pub mod workflow;
