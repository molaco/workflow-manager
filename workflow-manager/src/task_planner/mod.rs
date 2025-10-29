//! Task Planner Workflow
//!
//! Multi-agent task planning orchestrator that transforms high-level implementation
//! requirements (IMPL.md) into detailed, validated task specifications.
//!
//! ## Workflow Structure:
//!
//! **Phase 0: Generate Task Overview**
//! - Main orchestrator reads IMPL.md
//! - Generates high-level task breakdown (tasks_overview.yaml)
//! - Output: Strategic overview (WHAT and WHY, not HOW)
//!
//! **Phase 1: Expand Tasks**
//! - Execution planning (AI dependency analysis or simple batching)
//! - Parallel batch execution with suborchestrators
//! - Each suborchestrator coordinates 4 specialized sub-agents:
//!   - @files: Identifies files to create/modify
//!   - @functions: Specifies code items (functions, structs, traits)
//!   - @formal: Determines formal verification needs
//!   - @tests: Designs test strategy and implementation
//! - Output: Detailed task specifications (tasks.yaml)
//!
//! **Phase 2: Review Tasks**
//! - Parallel batch execution with review suborchestrators
//! - Each suborchestrator coordinates @reviewer agents
//! - Validates: completeness, consistency, correctness, testability
//! - Output: Review report (task_review_report.txt)

pub mod cli;
pub mod phase0_overview;
pub mod phase1_expand;
pub mod phase2_review;
pub mod utils;
pub mod workflow;

// Re-export main workflow function
pub use workflow::run_workflow;
