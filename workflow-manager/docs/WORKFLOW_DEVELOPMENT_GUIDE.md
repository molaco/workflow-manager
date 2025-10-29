# Workflow Development Guide

A comprehensive guide for building multi-phase AI agent workflows in `workflow-manager`.

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [File Structure](#2-file-structure)
3. [Critical Patterns](#3-critical-patterns)
4. [Workflow Utilities API](#4-workflow-utilities-api)
5. [Code Templates](#5-code-templates)
6. [Pre-Flight Checklist](#6-pre-flight-checklist)
7. [Common Pitfalls](#7-common-pitfalls)
8. [Real-World Examples](#8-real-world-examples)
9. [Quick Start](#9-quick-start)

---

## 1. Architecture Overview

### Phase-Based Structure

Workflows are organized into sequential phases:
- **Phase 0**: Initial analysis/setup
- **Phase 1**: Core processing
- **Phase 2+**: Additional processing, validation, synthesis

Each phase:
- Can be run independently via `--phases` flag
- Saves state to YAML files for resumability
- Can load state from previous phases
- Reports progress via TUI logging macros

### Key Components

```
┌─────────────────────────────────────────────────┐
│              Your Workflow                       │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐      │
│  │ Phase 0  │─▶│ Phase 1  │─▶│ Phase 2  │      │
│  └──────────┘  └──────────┘  └──────────┘      │
│       │             │             │              │
│       ▼             ▼             ▼              │
│  ┌──────────────────────────────────────┐       │
│  │      Workflow Utilities              │       │
│  │  • execute_agent()                   │       │
│  │  • execute_batch()                   │       │
│  │  • execute_task()                    │       │
│  │  • YAML helpers                      │       │
│  └──────────────────────────────────────┘       │
│       │             │             │              │
│       ▼             ▼             ▼              │
│  ┌──────────────────────────────────────┐       │
│  │      Claude Agent SDK                │       │
│  │  • query()                           │       │
│  │  • Stream handling                   │       │
│  │  • Sub-agents                        │       │
│  └──────────────────────────────────────┘       │
│       │             │             │              │
│       ▼             ▼             ▼              │
│  ┌──────────────────────────────────────┐       │
│  │         TUI Logging System           │       │
│  │  • Hierarchical task display         │       │
│  │  • Real-time progress                │       │
│  │  • Agent message streaming           │       │
│  └──────────────────────────────────────┘       │
└─────────────────────────────────────────────────┘
```

### State Persistence

Each phase outputs YAML files:
```
OUTPUT/
  phase0_results_20251029_143022.yaml
  phase1_results_20251029_143145.yaml
  ...
```

Workflows can resume from any phase by loading these files.

---

## 2. File Structure

Standard layout for a new workflow:

```
src/
  your_workflow/
    mod.rs              # Public exports
    workflow.rs         # Main orchestration
    cli.rs              # CLI argument parsing
    phase0_*.rs         # Phase implementations
    phase1_*.rs
    phase2_*.rs
    types.rs            # Optional: custom types
    utils.rs            # Optional: helper functions
```

### Module Organization

- **`mod.rs`**: Public API surface
- **`cli.rs`**: Argument parsing, validation, defaults
- **`workflow.rs`**: Phase orchestration, state management
- **`phase*.rs`**: Individual phase logic
- **`types.rs`**: Workflow-specific data structures (if needed)
- **`utils.rs`**: Helper functions shared across phases (if needed)

---

## 3. Critical Patterns

### ⚠️ Directory Management

**THE RULE**: Set working directory ONCE at the beginning, NEVER change it mid-execution.

**Why?** The TUI logging system initializes when the workflow starts and expects a stable working directory. Changing directories mid-execution breaks logging infrastructure.

**❌ WRONG**:
```rust
// Phase 0 runs
if phases.contains(&0) {
    // ...
}

// Phase 1 - CHANGES DIRECTORY
if phases.contains(&1) {
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(&codebase_path)?;  // ❌ Breaks TUI logging!

    execute_phase1().await?;

    std::env::set_current_dir(original_dir)?;
}
```

**✅ CORRECT**:
```rust
pub async fn run_workflow(args: Args) -> Result<()> {
    // Change directory FIRST, before any logging
    std::env::set_current_dir(&args.codebase_path)?;
    println!("📁 Working directory: {}", args.codebase_path.display());

    // Create output directories (now relative to new working dir)
    fs::create_dir_all("./OUTPUT").await?;

    // Now run all phases - directory stays stable
    if phases.contains(&0) { /* ... */ }
    if phases.contains(&1) { /* ... */ }

    Ok(())
}
```

---

### ⚠️ Task ID Hierarchy (TUI Nesting)

**THE RULE**: Agent task_id MUST match parent task_id for proper TUI nesting.

**Why?** The TUI displays agents nested under their parent tasks. Mismatched IDs prevent nesting.

**❌ WRONG**:
```rust
// Parent task
execute_task(
    format!("expand_{}", task_id),  // e.g., "expand_1"
    "Expanding task",
    ctx,
    || async {
        // Agent with DIFFERENT task_id
        let config = AgentConfig::new(
            format!("expand_task_{}", task_id),  // ❌ "expand_task_1" - MISMATCH!
            "Agent",
            "Description",
            prompt,
            options,
        );
        execute_agent(config).await
    }
).await?;
```

**Result in TUI**:
```
▶ Expanding task                     ← Parent task
  (no agent messages shown - orphaned!)
```

**✅ CORRECT**:
```rust
// Parent task
execute_task(
    format!("expand_{}", task_id),  // "expand_1"
    "Expanding task",
    ctx,
    || async {
        // Agent with SAME task_id
        let config = AgentConfig::new(
            format!("expand_{}", task_id),  // ✅ "expand_1" - MATCHES!
            "Agent",
            "Description",
            prompt,
            options,
        );
        execute_agent(config).await
    }
).await?;
```

**Result in TUI**:
```
▶ ▼ Expanding task                   ← Parent task
  ▶ ▼ Agent                          ← Agent properly nested
      Agent messages here...
```

---

### ⚠️ Sub-Agent Pattern

**THE RULE**: Parent agent MUST include `"Task"` in `allowed_tools` to delegate to sub-agents.

**Why?** Sub-agents are invoked via the `Task` tool. Without it, delegation fails silently.

**❌ WRONG**:
```rust
let sub_agent = AgentDefinition {
    description: "Helper agent".to_string(),
    prompt: "You are a helper...".to_string(),
    tools: Some(vec!["Read".to_string()]),
    model: Some("sonnet".to_string()),
};

let options = ClaudeAgentOptions::builder()
    .allowed_tools(vec![
        "Read".to_string(),
        "Grep".to_string(),
        // ❌ Missing "Task" - sub-agent can't be invoked!
    ])
    .add_agent("helper", sub_agent)
    .build();
```

**✅ CORRECT**:
```rust
let sub_agent = AgentDefinition {
    description: "Helper agent".to_string(),
    prompt: "You are a helper...".to_string(),
    tools: Some(vec!["Read".to_string()]),
    model: Some("sonnet".to_string()),
};

let options = ClaudeAgentOptions::builder()
    .allowed_tools(vec![
        "Read".to_string(),
        "Grep".to_string(),
        "Task".to_string(),  // ✅ Required for sub-agent delegation!
    ])
    .add_agent("helper", sub_agent)
    .build();
```

**In system prompt**, tell the parent agent how to invoke:
```rust
let system_prompt = r#"
You can delegate to the @helper sub-agent for assistance.

To invoke: Use the Task tool with "@helper" in your prompt.
Example: "Delegate to @helper to analyze the code"
"#;
```

---

### State Persistence Pattern

Save phase outputs with timestamps, allow resuming:

```rust
// Phase 1: Generate data
if phases.contains(&1) {
    let data = generate_data().await?;

    // Save with timestamp
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let output_path = PathBuf::from(format!("./OUTPUT/phase1_data_{}.yaml", timestamp));
    let yaml = serde_yaml::to_string(&data)?;
    fs::write(&output_path, &yaml).await?;

    // Log to TUI
    log_state_file!(1, output_path.display().to_string(), "Phase 1 data");
}
// Phase 2: Load from file if not running Phase 1
else if let Some(phase1_file) = &args.phase1_file {
    let yaml = fs::read_to_string(phase1_file).await?;
    data = serde_yaml::from_str(&yaml)?;
    println!("Loaded Phase 1 data from: {}", phase1_file);
}
```

---

## 4. Workflow Utilities API

The `workflow_utils` module provides standardized helpers for common patterns.

### `execute_agent()`

Execute a single agent with automatic stream handling and logging.

```rust
use crate::workflow_utils::{execute_agent, AgentConfig};

let config = AgentConfig::new(
    "task_1",                    // task_id (for TUI nesting)
    "Research Agent",            // agent_name (shown in TUI)
    "Researching patterns",      // description
    "How does auth work?",       // prompt
    options,                     // ClaudeAgentOptions
);

let response = execute_agent(config).await?;
```

**Features**:
- Automatic `log_agent_start!()`, `log_agent_complete!()`, `log_agent_failed!()`
- Streams all messages to TUI via `log_agent_message!()`
- Detects sub-agent delegations (🤝 Delegating to @agent)
- Detects sub-agent completions (✓ Sub-agent @agent completed)
- Returns full text response

---

### `execute_batch()`

Execute multiple tasks in parallel with concurrency control.

```rust
use crate::workflow_utils::{execute_batch, TaskContext};

let tasks = vec![task1, task2, task3];
let batch_size = 2;  // Run 2 at a time

let results = execute_batch(
    1,              // phase number
    tasks,          // Vec of tasks
    batch_size,     // concurrency limit
    |task, ctx| {   // async closure
        async move {
            let result = process_task(&task).await?;
            Ok((result, "Task complete".to_string()))
        }
    }
).await?;
```

**Features**:
- Parallel execution with semaphore-based concurrency control
- Automatic task logging with `TaskContext`
- Returns `Vec<(Result, String)>` tuples
- Fail-fast on first error

---

### `execute_task()`

Wrap a single task with automatic logging.

```rust
use crate::workflow_utils::execute_task;

let result = execute_task(
    "analyze_1",                           // task_id
    "Analyzing codebase structure",        // description
    TaskContext {
        phase: 0,
        task_number: 1,
        total_tasks: 5,
    },
    || async {
        let data = analyze_codebase().await?;
        Ok((data, "Analysis complete".to_string()))
    }
).await?;
```

**Features**:
- Automatic `log_task_start!()`, `log_task_complete!()`, `log_task_failed!()`
- Supports TUI progress tracking
- Returns the result, logs the summary message

---

### YAML Helpers

```rust
use crate::workflow_utils::{extract_yaml, parse_yaml, parse_yaml_multi};

// Extract YAML from agent response (removes markdown code fences)
let yaml_content = extract_yaml(&response);

// Parse single YAML document
let data: MyStruct = parse_yaml(&yaml_content)?;

// Parse multi-document YAML (separated by ---)
let items: Vec<Value> = parse_yaml_multi(&yaml_content)?;
```

---

## 5. Code Templates

### Template: `mod.rs`

```rust
//! Your workflow module
//!
//! Brief description of what this workflow does.

pub mod workflow;
pub mod cli;
pub mod phase0_analyze;
pub mod phase1_process;
pub mod phase2_synthesize;

// Optional
pub mod types;
pub mod utils;

// Public API
pub use workflow::run_workflow;
pub use cli::Args;
```

---

### Template: `cli.rs`

```rust
//! CLI argument parsing and validation

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(name = "your-workflow")]
#[command(about = "Brief description of your workflow")]
pub struct Args {
    /// Phases to execute (0-2, default: all)
    #[arg(long, value_delimiter = ',')]
    pub phases: Option<Vec<u32>>,

    /// Directory to analyze
    #[arg(long)]
    pub dir: Option<String>,

    /// Output directory for results
    #[arg(long, default_value = "OUTPUT")]
    pub output_dir: String,

    /// Load Phase 0 results from file (skip Phase 0)
    #[arg(long)]
    pub phase0_file: Option<String>,

    /// Load Phase 1 results from file (skip Phases 0-1)
    #[arg(long)]
    pub phase1_file: Option<String>,

    /// Number of concurrent tasks
    #[arg(long, default_value = "2")]
    pub batch_size: usize,

    // Add workflow-specific args here
}

impl Args {
    /// Get phases to execute (default: all phases)
    pub fn get_phases(&self) -> Vec<u32> {
        self.phases.clone().unwrap_or_else(|| vec![0, 1, 2])
    }

    /// Validate arguments
    pub fn validate(&self) -> Result<(), String> {
        // Validate phases
        let phases = self.get_phases();
        for phase in &phases {
            if *phase > 2 {
                return Err(format!("Invalid phase: {}. Valid phases: 0-2", phase));
            }
        }

        // Validate batch_size
        if self.batch_size == 0 {
            return Err("batch_size must be > 0".to_string());
        }

        // Add custom validation here

        Ok(())
    }
}
```

---

### Template: `workflow.rs`

```rust
//! Main workflow orchestration

use crate::your_workflow::cli::Args;
use crate::your_workflow::{phase0_analyze, phase1_process, phase2_synthesize};
use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::fs;
use workflow_manager_sdk::{
    log_phase_complete, log_phase_start, log_state_file,
};

/// Main workflow entry point
pub async fn run_workflow(args: Args) -> Result<()> {
    // Validate arguments
    args.validate()
        .map_err(|e| anyhow::anyhow!("Invalid arguments: {}", e))?;

    let phases = args.get_phases();
    let output_dir = PathBuf::from(&args.output_dir);

    // Get target directory
    let target_dir = if let Some(dir_str) = &args.dir {
        PathBuf::from(dir_str)
    } else {
        std::env::current_dir()?
    };

    // CRITICAL: Change directory ONCE at the beginning
    std::env::set_current_dir(&target_dir)
        .with_context(|| format!("Failed to change to directory: {}", target_dir.display()))?;
    println!("📁 Working directory: {}", target_dir.display());

    // Create output directory (now relative to target_dir)
    fs::create_dir_all(&output_dir)
        .await
        .with_context(|| format!("Failed to create output directory: {}", output_dir.display()))?;

    println!("\n{}", "=".repeat(80));
    println!("YOUR WORKFLOW NAME");
    println!("{}", "=".repeat(80));
    println!("Phases to execute: {:?}", phases);
    println!("Output directory: {}", output_dir.display());
    println!("{}", "=".repeat(80));

    // Track state between phases
    let mut phase0_data = None;
    let mut phase1_data = None;

    // Phase 0: Analyze
    if phases.contains(&0) {
        log_phase_start!(0, "Analyze", 3);

        phase0_data = Some(phase0_analyze::analyze(&target_dir).await?);

        // Save to file
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let output_path = output_dir.join(format!("phase0_results_{}.yaml", timestamp));
        let yaml = serde_yaml::to_string(&phase0_data)?;
        fs::write(&output_path, &yaml)
            .await
            .with_context(|| format!("Failed to write Phase 0 output: {}", output_path.display()))?;

        println!("✓ Phase 0 saved to: {}", output_path.display());
        log_state_file!(0, output_path.display().to_string(), "Phase 0 results");
        log_phase_complete!(0, "Analyze");
    } else if let Some(file) = &args.phase0_file {
        // Load from file
        let yaml = fs::read_to_string(file)
            .await
            .with_context(|| format!("Failed to read Phase 0 file: {}", file))?;
        phase0_data = Some(serde_yaml::from_str(&yaml)?);
        println!("Loaded Phase 0 from: {}", file);
    }

    // Phase 1: Process
    if phases.contains(&1) {
        log_phase_start!(1, "Process", 3);

        let data = phase0_data.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Phase 0 data required. Run Phase 0 or provide --phase0-file"))?;

        phase1_data = Some(phase1_process::process(data, args.batch_size).await?);

        // Save to file
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let output_path = output_dir.join(format!("phase1_results_{}.yaml", timestamp));
        let yaml = serde_yaml::to_string(&phase1_data)?;
        fs::write(&output_path, &yaml).await?;

        println!("✓ Phase 1 saved to: {}", output_path.display());
        log_state_file!(1, output_path.display().to_string(), "Phase 1 results");
        log_phase_complete!(1, "Process");
    } else if let Some(file) = &args.phase1_file {
        let yaml = fs::read_to_string(file).await?;
        phase1_data = Some(serde_yaml::from_str(&yaml)?);
        println!("Loaded Phase 1 from: {}", file);
    }

    // Phase 2: Synthesize
    if phases.contains(&2) {
        log_phase_start!(2, "Synthesize", 3);

        let data = phase1_data.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Phase 1 data required. Run Phase 1 or provide --phase1-file"))?;

        let output_path = output_dir.join("final_output.md");
        phase2_synthesize::synthesize(data, &output_path).await?;

        println!("✓ Phase 2 saved to: {}", output_path.display());
        log_state_file!(2, output_path.display().to_string(), "Final output");
        log_phase_complete!(2, "Synthesize");
    }

    println!("\n{}", "=".repeat(80));
    println!("✓ WORKFLOW COMPLETE");
    println!("{}", "=".repeat(80));

    Ok(())
}
```

---

### Template: Simple Single-Agent Phase

```rust
//! Phase 0: Analyze codebase

use crate::workflow_utils::{execute_agent, AgentConfig};
use anyhow::Result;
use claude_agent_sdk::ClaudeAgentOptions;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub summary: String,
    pub file_count: usize,
}

pub async fn analyze(codebase_path: &Path) -> Result<AnalysisResult> {
    let system_prompt = format!(
        r#"You are a codebase analyzer. Analyze the codebase at {} and provide:
- Summary of what the codebase does
- Number of source files

Output as YAML."#,
        codebase_path.display()
    );

    let options = ClaudeAgentOptions::builder()
        .system_prompt(system_prompt)
        .allowed_tools(vec![
            "Read".to_string(),
            "Grep".to_string(),
            "Glob".to_string(),
        ])
        .build();

    let config = AgentConfig::new(
        "analyze",                          // task_id
        "Analyzer",                         // agent_name
        "Analyzing codebase",               // description
        "Analyze this codebase".to_string(), // prompt
        options,
    );

    let response = execute_agent(config).await?;

    // Parse YAML response
    let result: AnalysisResult = serde_yaml::from_str(&response)?;
    Ok(result)
}
```

---

### Template: Batch Execution Phase

```rust
//! Phase 1: Process items in parallel

use crate::workflow_utils::{execute_batch, execute_task, TaskContext};
use anyhow::Result;
use serde_yaml::Value;

pub async fn process(items: &[Value], batch_size: usize) -> Result<Vec<String>> {
    println!("\nProcessing {} items with batch_size={}\n", items.len(), batch_size);

    let results = execute_batch(
        1,              // phase number
        items.to_vec(), // tasks
        batch_size,     // concurrency
        |item, ctx| async move {
            let item_id = item.get("id")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            let result = execute_task(
                format!("process_{}", item_id),
                format!("Processing item {}", item_id),
                ctx,
                || async move {
                    // Do your processing here
                    let output = format!("Processed item {}", item_id);
                    Ok((output.clone(), format!("Item {} done", item_id)))
                }
            ).await?;

            Ok((result, format!("Item {} complete", item_id)))
        }
    ).await?;

    // Extract just the results (not the summary strings)
    let outputs = results.into_iter().map(|(output, _)| output).collect();
    Ok(outputs)
}
```

---

### Template: Sub-Agent Coordination Phase

```rust
//! Phase 2: Coordinate multiple sub-agents

use crate::workflow_utils::{execute_agent, AgentConfig};
use anyhow::Result;
use claude_agent_sdk::{AgentDefinition, ClaudeAgentOptions};

pub async fn coordinate(input_data: &str) -> Result<String> {
    // Define specialized sub-agents
    let analyzer_agent = AgentDefinition {
        description: "Analyzes code structure".to_string(),
        prompt: r#"You are a code analyzer.
Analyze the provided code and identify:
- Functions and their purposes
- Data structures
- Dependencies

Output as YAML."#.to_string(),
        tools: Some(vec![
            "Read".to_string(),
            "Grep".to_string(),
        ]),
        model: Some("sonnet".to_string()),
    };

    let reviewer_agent = AgentDefinition {
        description: "Reviews code quality".to_string(),
        prompt: r#"You are a code reviewer.
Review the code for:
- Potential bugs
- Style issues
- Best practices

Output as YAML."#.to_string(),
        tools: Some(vec!["Read".to_string()]),
        model: Some("sonnet".to_string()),
    };

    // System prompt tells orchestrator how to use sub-agents
    let system_prompt = r#"You are an orchestrator coordinating specialized sub-agents.

You have two sub-agents available:

1. **@analyzer** - Analyzes code structure
2. **@reviewer** - Reviews code quality

## WORKFLOW
1. Delegate to @analyzer to understand the code structure
2. Delegate to @reviewer to identify quality issues
3. Combine their outputs into a comprehensive report

To invoke a sub-agent, use the Task tool with their name in your prompt.
Example: "Delegate to @analyzer to analyze the code"
"#;

    let query_prompt = format!(
        r#"Coordinate with @analyzer and @reviewer to analyze this code:

{}

Combine their outputs into a final report."#,
        input_data
    );

    let options = ClaudeAgentOptions::builder()
        .system_prompt(system_prompt)
        .allowed_tools(vec![
            "Read".to_string(),
            "Task".to_string(),  // CRITICAL: Required for sub-agent delegation!
        ])
        .add_agent("analyzer", analyzer_agent)
        .add_agent("reviewer", reviewer_agent)
        .build();

    let config = AgentConfig::new(
        "coordinate",                    // task_id
        "Orchestrator",                  // agent_name
        "Coordinating sub-agents",       // description
        query_prompt,
        options,
    );

    let response = execute_agent(config).await?;
    Ok(response)
}
```

---

## 6. Pre-Flight Checklist

Before running your new workflow, verify:

### Configuration
- [ ] Set working directory at workflow start (`workflow.rs`)
- [ ] Create output directories AFTER directory change
- [ ] Output directory is configurable via CLI args

### Sub-Agents (if using)
- [ ] Add `"Task"` to parent agent's `allowed_tools`
- [ ] Define sub-agents with `AgentDefinition`
- [ ] Register via `.add_agent("name", definition)`
- [ ] System prompt explains how to invoke with `@name`

### TUI Integration
- [ ] Agent task_id matches parent task_id
- [ ] Use `log_phase_start!()` / `log_phase_complete!()`
- [ ] Use `log_task_start!()` / `log_task_complete!()`
- [ ] Use `log_state_file!()` for output files
- [ ] Agent execution via `execute_agent()` (auto-logs messages)

### State Management
- [ ] Save phase outputs as timestamped YAML files
- [ ] CLI accepts `--phase0-file`, `--phase1-file`, etc. for resuming
- [ ] Each phase checks if data is loaded or needs to run previous phase

### CLI
- [ ] Implement `Args` struct with clap
- [ ] Support `--phases` flag for selective execution
- [ ] Validate arguments in `Args::validate()`
- [ ] Set reasonable defaults

### Error Handling
- [ ] Use `anyhow::Result<T>` for all fallible functions
- [ ] Add context with `.with_context(|| "...")`
- [ ] Log failures via `log_task_failed!()`, `log_agent_failed!()`

---

## 7. Common Pitfalls

### 🚨 Missing `"Task"` in allowed_tools

**Symptom**: Sub-agents are defined but never invoked, no delegation logs in TUI.

**Cause**: Parent agent can't use Task tool to delegate.

**Fix**:
```rust
.allowed_tools(vec![
    "Read".to_string(),
    "Task".to_string(),  // ← ADD THIS
])
```

---

### 🚨 Task ID Mismatch

**Symptom**: Agent logs don't appear nested under tasks in TUI.

**Cause**: Agent's `task_id` doesn't match parent task's ID.

**Fix**:
```rust
// Parent task
execute_task("expand_1", ..., || async {
    // Agent - use SAME ID
    let config = AgentConfig::new(
        "expand_1",  // ← Must match parent!
        ...
    );
})
```

---

### 🚨 Mid-Execution Directory Change

**Symptom**: TUI logging stops working after directory change.

**Cause**: Changing working directory breaks TUI logging infrastructure.

**Fix**: Move `set_current_dir()` to the very beginning of `run_workflow()`, before any logging.

---

### 🚨 Not Using workflow_utils

**Symptom**: Reimplementing agent execution, batch processing, logging.

**Cause**: Not aware of or not using workflow_utils helpers.

**Fix**: Use `execute_agent()`, `execute_batch()`, `execute_task()` instead of rolling your own.

---

### 🚨 Forgetting to Log State Files

**Symptom**: Generated files aren't visible in TUI's state file tracker.

**Cause**: Not calling `log_state_file!()`.

**Fix**:
```rust
log_state_file!(
    phase_number,
    output_path.display().to_string(),
    "Description of this file"
);
```

---

### 🚨 Blocking on Batch Execution

**Symptom**: Tasks run sequentially instead of in parallel.

**Cause**: Not using `execute_batch()` or setting `batch_size=1`.

**Fix**:
```rust
execute_batch(
    phase,
    tasks,
    batch_size,  // ← Set > 1 for parallelism
    |task, ctx| async move { ... }
)
```

---

## 8. Real-World Examples

### Research Workflow (`src/research/`)

**Purpose**: Multi-phase codebase documentation generation.

**Phases**:
- Phase 0: Analyze codebase structure
- Phase 1: Generate research prompts
- Phase 2: Execute research in parallel (batch execution)
- Phase 3: Validate and fix YAML outputs
- Phase 4: Synthesize final documentation (uses file-condenser sub-agent)

**Key Patterns**:
- Uses `execute_batch()` for parallel research (Phase 2)
- Iterative validation loop (Phase 3)
- Single sub-agent for file condensing (Phase 4)

**Reference**: `src/research/workflow.rs`, `src/research/phase4_synthesize.rs`

---

### Task Planner Workflow (`src/task_planner/`)

**Purpose**: Expand high-level tasks into detailed implementation specifications.

**Phases**:
- Phase 0: Generate task overview
- Phase 1: Expand tasks with suborchestrators + 4 sub-agents each
- Phase 2: Review expanded tasks

**Key Patterns**:
- Each task gets a suborchestrator agent
- Each suborchestrator coordinates 4 specialized sub-agents:
  - `@files` - Identifies files to modify
  - `@functions` - Specifies functions/structs to implement
  - `@formal` - Determines formal verification needs
  - `@tests` - Designs test strategy
- AI-based dependency analysis for execution planning
- Parallel batch execution of suborchestrators

**Reference**: `src/task_planner/workflow.rs`, `src/task_planner/phase1_expand.rs`

---

### Key Differences

| Aspect | Research | Task Planner |
|--------|----------|--------------|
| **Phases** | 5 (0-4) | 3 (0-2) |
| **Sub-agents** | 1 (file-condenser) | 4 per task (@files, @functions, @formal, @tests) |
| **Parallelism** | Batch research tasks | Batch task suborchestrators |
| **Complexity** | Linear phases | Hierarchical (orchestrator → sub-agents) |
| **Output** | Final markdown doc | YAML task specifications |

---

## 9. Quick Start

### Step 1: Copy Templates

Create your workflow module structure:
```bash
mkdir -p src/my_workflow
touch src/my_workflow/{mod.rs,cli.rs,workflow.rs,phase0_init.rs}
```

### Step 2: Implement Core Files

1. Copy **`mod.rs`** template → customize exports
2. Copy **`cli.rs`** template → add your specific args
3. Copy **`workflow.rs`** template → update phase logic
4. Copy phase templates → implement your logic

### Step 3: Register in Main Binary

Edit `src/bin/workflow_manager.rs`:
```rust
mod my_workflow;

// Add to match statement:
WorkflowType::MyWorkflow => {
    let args = my_workflow::Args::parse_from(args);
    my_workflow::run_workflow(args).await?;
}
```

### Step 4: Run Through Checklist

Go through the [Pre-Flight Checklist](#6-pre-flight-checklist).

### Step 5: Test

```bash
# Build
cargo build --release

# Test each phase individually
./target/release/workflow-manager my-workflow --phases 0
./target/release/workflow-manager my-workflow --phases 1 --phase0-file OUTPUT/phase0_*.yaml
./target/release/workflow-manager my-workflow --phases 2 --phase1-file OUTPUT/phase1_*.yaml

# Test all phases together
./target/release/workflow-manager my-workflow --phases 0,1,2
```

---

## Summary

Building workflows requires:
1. **Phase-based structure** with state persistence
2. **workflow_utils** for agent/batch/task execution
3. **TUI integration** via logging macros
4. **Proper directory management** (set once, at start)
5. **Correct task ID hierarchy** for nesting
6. **Task tool** for sub-agent delegation

Follow the templates, run through the checklist, and avoid the common pitfalls. Reference the research and task_planner workflows for real-world examples.

Happy workflow building! 🚀
