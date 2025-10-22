//! Research workflow automation for codebase analysis and documentation generation.
//!
//! This module provides a multi-phase research workflow system that uses Claude agents
//! to analyze codebases, generate research prompts, execute research, validate results,
//! and synthesize documentation.
//!
//! # Quick Start
//!
//! Run the complete workflow:
//!
//! ```no_run
//! use workflow_manager::research::{run_research_workflow, WorkflowConfig};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = WorkflowConfig {
//!     objective: Some("Analyze authentication system".to_string()),
//!     phases: vec![0, 1, 2, 3, 4],
//!     batch_size: 2,
//!     dir: Some(".".to_string()),
//!     analysis_file: None,
//!     prompts_file: None,
//!     results_file: None,
//!     results_dir: None,
//!     output: None,
//!     system_prompt: Some("prompts/writer.md".to_string()),
//!     append: Some("prompts/style.md".to_string()),
//! };
//!
//! run_research_workflow(config).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Workflow Phases
//!
//! The research workflow consists of 5 distinct phases:
//!
//! ## Phase 0: Codebase Analysis
//! [`phase0_analyze`] - Analyzes the codebase structure, file statistics, dependencies,
//! and architecture patterns using Claude agents with tool access.
//!
//! ## Phase 1: Prompt Generation
//! [`phase1_prompts`] - Generates targeted research prompts based on the research
//! objective and codebase analysis.
//!
//! ## Phase 2: Research Execution
//! [`phase2_research`] - Executes research prompts in parallel using multiple Claude
//! agents, with configurable concurrency control.
//!
//! ## Phase 3: YAML Validation
//! [`phase3_validate`] - Validates and automatically fixes YAML syntax errors in
//! research results using iterative agent-based repair.
//!
//! ## Phase 4: Documentation Synthesis
//! [`phase4_synthesize`] - Synthesizes all research findings into comprehensive,
//! well-structured documentation.
//!
//! # Examples
//!
//! ## Run Individual Phases
//!
//! ```no_run
//! use workflow_manager::research::phase0_analyze::analyze_codebase;
//! use std::path::Path;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let analysis = analyze_codebase(Path::new(".")).await?;
//! println!("Codebase analysis complete");
//! # Ok(())
//! # }
//! ```
//!
//! ## Resume from Saved State
//!
//! ```no_run
//! use workflow_manager::research::{run_research_workflow, WorkflowConfig};
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Resume from Phase 2 using saved prompts
//! let config = WorkflowConfig {
//!     objective: None,
//!     phases: vec![2, 3, 4],
//!     batch_size: 3,
//!     dir: None,
//!     analysis_file: None,
//!     prompts_file: Some("OUTPUT/research_prompts_20250101_120000.yaml".to_string()),
//!     results_file: None,
//!     results_dir: None,
//!     output: Some("docs/guide.md".to_string()),
//!     system_prompt: None,
//!     append: None,
//! };
//!
//! run_research_workflow(config).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Module Organization
//!
//! - [`types`] - Core data structures (CodebaseAnalysis, ResearchPrompt, etc.)
//! - [`cli`] - Command-line argument parsing
//! - [`workflow`] - Main orchestration logic
//! - [`phase0_analyze`] - Codebase analysis implementation
//! - [`phase1_prompts`] - Prompt generation implementation
//! - [`phase2_research`] - Research execution implementation
//! - [`phase3_validate`] - YAML validation and repair implementation
//! - [`phase4_synthesize`] - Documentation synthesis implementation

// Module declarations
pub mod cli;
pub mod types;
pub mod workflow;

// Phase modules
pub mod phase0_analyze;
pub mod phase1_prompts;
pub mod phase2_research;
pub mod phase3_validate;
pub mod phase4_synthesize;

// Re-export commonly used items for convenience
pub use types::{CodebaseAnalysis, PromptsData, ResearchPrompt, ResearchResult};
pub use workflow::{run_research_workflow, WorkflowConfig};
