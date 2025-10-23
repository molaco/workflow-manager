# Task Planner Translation & Refactoring Plan

## Overview
Translate `SCRIPTS/test7.py` (~1,850 lines) from Python to Rust and refactor into modular library architecture, using the same approach as the successful `research_agent.rs` refactoring.

## Goals
1. **Translate Python → Rust**: Convert all functionality from Python to idiomatic Rust
2. **Replace Rich UI with logging macros**: Use workflow-manager-sdk logging instead of Rich console
3. **Modular architecture**: Break into library + thin binary (like research module)
4. **Type safety**: Leverage Rust's type system for reliability
5. **Testability**: Enable unit and integration testing
6. **Reusability**: Library API for TUI integration

## Current State Analysis

### test7.py Structure (~1,850 lines)
```
Rich Console Classes:          ~180 lines  (KeyboardReader, AgentLogger)
Utility Functions:             ~100 lines  (load_template, parse_tasks, etc.)
Step 1: Overview Generation:   ~70 lines   (step1_generate_overview)
Step 2: Task Expansion:        ~800 lines  (step2_expand_all_tasks + suborchestrator)
Step 3: Task Review:           ~350 lines  (step3_review_tasks + review_suborchestrator)
Main Workflow:                 ~210 lines  (main function + argument parsing)
Execution Planning:            ~140 lines  (generate_execution_plan, parse_execution_plan)
```

### Logging Categories in test7.py

#### 1. Section Headers (Panel.fit)
```python
console.print(Panel.fit(
    "[bold cyan]STEP 1: Main Orchestrator[/bold cyan]\n"
    "Generate tasks_overview.yaml from IMPL.md",
    border_style="cyan"
))
```
**Lines**: 378-382, 750-754, 802-806, 1513-1517, 1586-1590, 1698-1703

#### 2. Status Messages
```python
console.print("[green]✓[/green] Saved: {output_path}")          # Success
console.print("[red]✗[/red] Error: {message}")                   # Error
console.print("[yellow]⚠[/yellow] Warning: {message}")           # Warning
console.print("[blue]ℹ[/blue] Loading {filename}...")            # Info
```
**Lines**: 290, 316-317, 713, 930, 942, 997, 1028, 1473, 1485, 1548, 1597-1598, 1609, 1632, 1713, 1721, 1733, 1747, 1763, 1784, 1814

#### 3. Step/Phase Progress
```python
console.print("\n=== STEP 2: Suborchestrators - Expand Tasks ===\n")
console.print(f"Found {len(tasks)} tasks to expand\n")
console.print(f"Execution plan: {len(batches)} batch(es)")
```
**Lines**: 1023, 1031, 1048

#### 4. Batch Operations
```python
console.print(f"\n→ Executing Batch {batch_num}/{len(batches)}")
console.print(f"  Running {num_tasks} {task_label}...\n")
console.print(f"[green]✓[/green] [Batch {batch_num}] Parsed {len(results)} review results")
console.print(f"[green]✓[/green] Batch {batch_num} review complete\n")
```
**Lines**: 1071, 1077, 1473, 1575

#### 5. Task Logger (AgentLogger class methods)
```python
await task_logger.agent_start("suborchestrator", task_id=task_id)
await task_logger.info(f"Expanding: {task_name}")
await task_logger.info(f"→ Delegating to @{agent_name}...")
await task_logger.success(f"Expansion complete ({duration}ms)")
await task_logger.agent_end()
```
**Lines**: 467-468, 689, 692, 696, 704, 706, 715, 719, 735-736

#### 6. Sub-agent Delegation
```python
console.print(f"[dim]  [Batch {batch_num}] → Delegating to @reviewer agent...[/dim]")
await task_logger.info(f"→ Delegating to @{agent_name}...")
```
**Lines**: 689, 1444

#### 7. Statistics/Usage
```python
console.print(Panel(stats_text, title="[cyan]Step 1 Statistics[/cyan]", border_style="cyan"))
# stats_text contains: Duration, Turns, Cost, Tokens

console.print("\n=== Step 2 Aggregate Statistics ===")
console.print(f"Total tasks expanded: {len(all_usage_stats)}")
console.print(f"Total duration: {total_duration}ms ({total_duration/1000:.1f}s)")
console.print(f"Total turns: {total_turns}")
console.print(f"Total cost: ${total_cost:.4f}")
```
**Lines**: 430-438, 1248-1258

#### 8. Review Results
```python
console.print(f"Total tasks reviewed: {len(review_results)}")
console.print(f"[green]✓[/green] Approved: {approved_count}")
console.print(f"[red]✗[/red] Needs revision: {needs_revision_count}\n")
console.print("[yellow]Tasks requiring revision:[/yellow]\n")
console.print(f"  [red]Task {result['task_id']}:[/red]")
console.print(f"    [dim]- {issue}[/dim]")
```
**Lines**: 1596-1607

#### 9. Debug Output
```python
console.print(f"[dim]DEBUG: Parsing {len(plan_batches)} batches from execution plan[/dim]\n")
console.print(f"[dim]  Batch {batch_id}: {len(task_refs)} tasks[/dim]")
console.print(f"[dim]{block.text}[/dim]")
```
**Lines**: 911, 919, 925, 937, 1441, 1458-1459, 1474, 1476, 1486

#### 10. File Operations
```python
console.print(f"[green]✓[/green] Saved: {output_path}")
console.print(f"Streaming mode: Writing tasks directly to {tasks_path}\n")
console.print(f"✓ Tasks streamed to: {tasks_path}\n")
```
**Lines**: 290, 1066, 1270

---

## Phase 1: Add New Logging Macros to workflow-manager-sdk

### Required New Macros

Based on the analysis above, we need to add these macros to handle task planner logging patterns:

```rust
// Section headers (replaces Panel.fit)
log_phase_start!(phase_number, title, description);
// Output: "═══ STEP 1: Main Orchestrator ═══"
//         "Generate tasks_overview.yaml from IMPL.md"

log_phase_complete!(phase_number);
// Output: "✓ Step 1 complete"

// Batch operations
log_batch_start!(batch_num, total_batches, num_tasks);
// Output: "→ Executing Batch 2/5 (3 tasks)"

log_batch_complete!(batch_num);
// Output: "✓ Batch 2 complete"

// Parallel execution
log_parallel_start!(num_items, item_type);
// Output: "→ Running 3 tasks in parallel"

log_parallel_complete!(num_items, item_type);
// Output: "✓ 3 tasks completed"

// Sub-agent delegation
log_delegate_to!(parent_agent, sub_agent_name);
// Output: "  → Delegating to @files agent..."

log_delegate_complete!(sub_agent_name);
// Output: "  ✓ @files agent complete"

// Statistics (individual)
log_stats!(duration_ms, turns, cost_usd, input_tokens, output_tokens);
// Output: "Statistics: 1250ms, 3 turns, $0.0234 (tokens: 1234 in / 567 out)"

// Statistics (aggregate)
log_aggregate_stats!(item_count, total_duration_ms, total_turns, total_cost_usd);
// Output: "Total: 5 tasks, 6.2s, 15 turns, $0.1145"

// Review results
log_review_summary!(approved, needs_revision, total);
// Output: "Review: ✓ 8 approved, ✗ 2 need revision (10 total)"

log_review_issue!(task_id, issue_text);
// Output: "  ✗ Task 3: Missing test coverage for edge cases"

// Progress indicators
log_progress!(current, total, item_type);
// Output: "Progress: 3/5 tasks"

log_found!(count, item_type);
// Output: "Found 14 tasks to expand"

// Info messages
log_info!(message);
// Output: "ℹ Loading tasks_overview_template..."

// Warning messages (already exists as log_error!, but may need log_warning!)
log_warning!(message);
// Output: "⚠ Warning: Circular dependency detected"

// File operations (may already exist)
log_file_saved!(path);
// Output: "✓ Saved: ./tasks_overview.yaml"

log_streaming_start!(path);
// Output: "→ Streaming mode: Writing tasks directly to ./tasks.yaml"

log_streaming_complete!(path);
// Output: "✓ Tasks streamed to: ./tasks.yaml"

// Debug output (conditional on debug flag)
log_debug!(message);
// Output: "[DEBUG] Parsing 5 batches from execution plan"
```

### Implementation Steps for Phase 1

1. **Update workflow-manager-sdk/src/lib.rs**
   - Add new macro definitions
   - Follow existing macro patterns
   - Ensure consistent formatting with current macros

2. **Test new macros**
   - Create `tests/test_logging_macros.rs`
   - Verify output format for each macro
   - Test edge cases (0 items, large numbers, etc.)

3. **Documentation**
   - Add rustdoc comments for each macro
   - Provide usage examples
   - Update SDK README if needed

**Estimated time**: 3-4 hours

---

## Phase 2: Create Module Structure

### Target Structure

```
workflow-manager/
├── src/
│   ├── lib.rs                           # Library root
│   ├── task_planner/
│   │   ├── mod.rs                       # Module exports
│   │   ├── types.rs                     # Data structures
│   │   ├── cli.rs                       # CLI argument parsing
│   │   ├── utils.rs                     # Utility functions
│   │   ├── step1_overview.rs            # Overview generation
│   │   ├── step2_expand.rs              # Task expansion
│   │   ├── step3_review.rs              # Task review
│   │   ├── execution_plan.rs            # Execution planning
│   │   └── workflow.rs                  # Workflow orchestration
│   └── bin/
│       └── task_planner.rs              # Thin CLI wrapper (~20 lines)
└── tests/
    └── task_planner/
        ├── common.rs
        ├── test_types.rs
        ├── test_utils.rs
        ├── test_step1.rs
        ├── test_step2.rs
        └── test_step3.rs
```

### Module Breakdown

#### 1. `src/task_planner/types.rs` (~150 lines)

**Translate from Python:**
- Task structures (task overview, detailed task)
- Agent definitions
- Result structures
- Usage statistics

**Data structures:**
```rust
// From task overview
pub struct TaskOverview {
    pub task: TaskInfo,
    pub description: String,
    pub dependencies: Dependencies,
}

pub struct TaskInfo {
    pub id: u32,
    pub name: String,
    pub context: String,
}

pub struct Dependencies {
    pub requires_completion_of: Vec<TaskDependency>,
}

pub struct TaskDependency {
    pub task_id: u32,
    pub reason: String,
}

// From detailed task
pub struct DetailedTask {
    pub task: TaskInfo,
    pub files: Vec<FileSpec>,
    pub functions: Vec<FunctionGroup>,
    pub formal_verification: FormalVerification,
    pub tests: TestSpec,
}

pub struct FileSpec {
    pub path: String,
    pub description: String,
}

pub struct FunctionGroup {
    pub file: String,
    pub items: Vec<CodeItem>,
}

pub struct CodeItem {
    pub item_type: String,
    pub name: String,
    pub description: String,
    pub preconditions: Option<String>,
    pub postconditions: Option<String>,
    pub invariants: Option<String>,
}

pub struct FormalVerification {
    pub needed: bool,
    pub level: String,
    pub explanation: String,
    pub properties: Option<Vec<String>>,
    pub strategy: Option<String>,
}

pub struct TestSpec {
    pub strategy: TestStrategy,
    pub implementation: TestImplementation,
    pub coverage: Vec<String>,
}

pub struct TestStrategy {
    pub approach: String,
    pub rationale: Vec<String>,
}

pub struct TestImplementation {
    pub file: String,
    pub location: String,
    pub code: String,
}

// Execution plan
pub struct ExecutionPlan {
    pub total_tasks: usize,
    pub total_batches: usize,
    pub batches: Vec<Batch>,
    pub dependencies_summary: DependenciesSummary,
}

pub struct Batch {
    pub batch_id: usize,
    pub description: String,
    pub strategy: String,
    pub tasks: Vec<BatchTask>,
    pub parallelization_rationale: String,
}

pub struct BatchTask {
    pub task_id: u32,
    pub task_name: String,
    pub reason: String,
}

pub struct DependenciesSummary {
    pub critical_path: Vec<u32>,
    pub parallelization_potential: String,
    pub parallelization_explanation: String,
}

// Usage statistics
pub struct UsageStats {
    pub duration_ms: u64,
    pub duration_api_ms: Option<u64>,
    pub num_turns: u32,
    pub total_cost_usd: Option<f64>,
    pub usage: TokenUsage,
    pub session_id: Option<String>,
}

pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

// Review results
pub struct ReviewResult {
    pub task_id: u32,
    pub success: bool,
    pub issues: Vec<String>,
    pub summary: String,
}
```

**Estimated time**: 4-5 hours

#### 2. `src/task_planner/cli.rs` (~120 lines)

**Translate from Python argparse (lines 1641-1693):**

```rust
use clap::Parser;
use workflow_manager_sdk::WorkflowDefinition;

#[derive(Parser, WorkflowDefinition)]
#[command(name = "task-planner")]
#[command(about = "Multi-agent task planning orchestrator")]
pub struct Args {
    /// Which step to run (1=overview, 2=expand, 3=review, all=complete workflow)
    #[arg(long, value_name = "STEP", default_value = "all")]
    pub step: String,

    /// Path(s) to implementation file(s) - can specify multiple files
    #[arg(long, value_name = "PATH")]
    pub impl_files: Option<Vec<String>>,

    /// Path to tasks_overview.yaml
    #[arg(long, value_name = "PATH")]
    pub tasks_overview: Option<String>,

    /// Path to tasks.yaml
    #[arg(long, value_name = "PATH")]
    pub tasks: Option<String>,

    /// Stream tasks to file immediately (reduces memory usage)
    #[arg(long)]
    pub stream: bool,

    /// Enable debug output
    #[arg(long)]
    pub debug: bool,

    /// Use simple fixed-size batching with specified size
    #[arg(long, value_name = "SIZE")]
    pub batch_size: Option<usize>,

    /// Path to tasks_overview_template.yaml
    #[arg(long, value_name = "PATH")]
    pub tasks_overview_template: Option<String>,

    /// Path to task_template.yaml
    #[arg(long, value_name = "PATH")]
    pub task_template: Option<String>,

    /// Print workflow metadata
    #[arg(long)]
    pub workflow_metadata: bool,
}

impl Args {
    pub fn validate_step1(&self) -> anyhow::Result<()> {
        if self.tasks_overview_template.is_none() {
            anyhow::bail!("--tasks-overview-template is required for step 1");
        }
        Ok(())
    }

    pub fn validate_step2_or_3(&self) -> anyhow::Result<()> {
        if self.task_template.is_none() {
            anyhow::bail!("--task-template is required for steps 2 and 3");
        }
        Ok(())
    }
}
```

**Estimated time**: 2 hours

#### 3. `src/task_planner/utils.rs` (~200 lines)

**Translate utility functions (lines 265-364):**

```rust
use anyhow::{Context, Result};
use std::path::Path;
use serde_yaml;

use crate::task_planner::types::{TaskOverview, DetailedTask, ExecutionPlan};

/// Load a YAML template from the given path
pub fn load_template(template_path: &Path) -> Result<String> {
    std::fs::read_to_string(template_path)
        .with_context(|| format!("Failed to load template: {}", template_path.display()))
}

/// Load IMPL.md from project root or DOCS/
pub fn load_impl_md(project_root: &Path) -> Result<String> {
    let possible_paths = vec![
        project_root.join("IMPL.md"),
        project_root.join("DOCS").join("IMPL.md"),
    ];

    for path in possible_paths {
        if path.exists() {
            return std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()));
        }
    }

    anyhow::bail!("IMPL.md not found in project root or DOCS/")
}

/// Load multiple implementation files and combine them
pub fn load_impl_files(paths: &[String]) -> Result<String> {
    let mut parts = Vec::new();

    for path_str in paths {
        let path = Path::new(path_str);
        if !path.exists() {
            anyhow::bail!("Implementation file not found: {}", path.display());
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        if paths.len() > 1 {
            parts.push(format!("# Source: {}\n\n{}", path.file_name().unwrap().to_string_lossy(), content));
        } else {
            parts.push(content);
        }
    }

    Ok(parts.join("\n\n---\n\n"))
}

/// Save YAML data to file
pub fn save_yaml(data: &str, output_path: &Path) -> Result<()> {
    std::fs::write(output_path, data)
        .with_context(|| format!("Failed to write to {}", output_path.display()))?;

    log_file_saved!(output_path.display());
    Ok(())
}

/// Clean YAML response by removing markdown code blocks if present
pub fn clean_yaml_response(response: &str) -> String {
    if let Some(start) = response.find("```yaml") {
        if let Some(end) = response[start..].find("```") {
            return response[start + 7..start + end].trim().to_string();
        }
    } else if let Some(start) = response.find("```") {
        if let Some(end) = response[start + 3..].find("```") {
            return response[start + 3..start + 3 + end].trim().to_string();
        }
    }
    response.to_string()
}

/// Parse tasks_overview.yaml and extract task list
pub fn parse_tasks_overview(yaml_content: &str) -> Result<Vec<TaskOverview>> {
    let docs: Vec<serde_yaml::Value> = serde_yaml::Deserializer::from_str(yaml_content)
        .map(|doc| serde_yaml::Value::deserialize(doc).unwrap())
        .collect();

    // If single document with "task" field, wrap in vec
    if docs.len() == 1 {
        if let Some(obj) = docs[0].as_mapping() {
            if obj.contains_key(&serde_yaml::Value::String("task".to_string())) {
                let task: TaskOverview = serde_yaml::from_value(docs[0].clone())?;
                return Ok(vec![task]);
            }
        }
    }

    // Parse multiple documents
    let mut tasks = Vec::new();
    for doc in docs {
        if let Some(obj) = doc.as_mapping() {
            if obj.contains_key(&serde_yaml::Value::String("task".to_string())) {
                let task: TaskOverview = serde_yaml::from_value(doc)?;
                tasks.push(task);
            }
        }
    }

    Ok(tasks)
}

/// Parse detailed tasks from tasks.yaml
pub fn parse_detailed_tasks(yaml_content: &str) -> Result<Vec<DetailedTask>> {
    // Similar to parse_tasks_overview but for DetailedTask
    let docs: Vec<serde_yaml::Value> = serde_yaml::Deserializer::from_str(yaml_content)
        .map(|doc| serde_yaml::Value::deserialize(doc).unwrap())
        .collect();

    let mut tasks = Vec::new();
    for doc in docs {
        if let Some(obj) = doc.as_mapping() {
            if obj.contains_key(&serde_yaml::Value::String("task".to_string())) {
                let task: DetailedTask = serde_yaml::from_value(doc)?;
                tasks.push(task);
            }
        }
    }

    Ok(tasks)
}

/// Extract text and usage stats from agent response
pub async fn extract_text_and_stats(
    client: &mut claude_agent_sdk::ClaudeSDKClient,
) -> Result<(String, UsageStats)> {
    use claude_agent_sdk::{AssistantMessage, ResultMessage, TextBlock};

    let mut response_parts = Vec::new();
    let mut usage_stats = None;

    while let Some(msg) = client.receive_response().await {
        match msg {
            AssistantMessage(msg) => {
                for block in msg.content {
                    if let TextBlock(block) = block {
                        response_parts.push(block.text);
                    }
                }
            }
            ResultMessage(msg) => {
                usage_stats = Some(UsageStats {
                    duration_ms: msg.duration_ms,
                    duration_api_ms: msg.duration_api_ms,
                    num_turns: msg.num_turns,
                    total_cost_usd: msg.total_cost_usd,
                    usage: TokenUsage {
                        input_tokens: msg.usage.input_tokens,
                        output_tokens: msg.usage.output_tokens,
                    },
                    session_id: msg.session_id,
                });
            }
            _ => {}
        }
    }

    let text = response_parts.join("\n");
    let stats = usage_stats.ok_or_else(|| anyhow::anyhow!("No usage stats received"))?;

    Ok((text, stats))
}
```

**Estimated time**: 4 hours

#### 4. `src/task_planner/execution_plan.rs` (~250 lines)

**Translate execution planning functions (lines 741-1007):**

```rust
use anyhow::Result;
use crate::task_planner::types::{TaskOverview, ExecutionPlan, Batch};
use crate::task_planner::utils::clean_yaml_response;
use claude_agent_sdk::{ClaudeSDKClient, ClaudeAgentOptions};
use workflow_manager_sdk::log_info;

/// Generate simple execution plan by chunking tasks into fixed-size batches
pub fn generate_execution_plan_simple(
    tasks: &[TaskOverview],
    batch_size: usize,
) -> String {
    log_phase_start!(0, "Batch Planning", "Simple batching");
    log_info!("Using fixed batch size: {}", batch_size);

    let mut batches = Vec::new();
    for (i, chunk) in tasks.chunks(batch_size).enumerate() {
        let batch = Batch {
            batch_id: i + 1,
            description: format!("Batch {} - Tasks {} to {}",
                i + 1,
                i * batch_size + 1,
                (i + 1) * batch_size.min(tasks.len())
            ),
            strategy: "sequential".to_string(),
            tasks: chunk.iter().map(|t| BatchTask {
                task_id: t.task.id,
                task_name: t.task.name.clone(),
                reason: format!("Part of batch {}", i + 1),
            }).collect(),
            parallelization_rationale: format!(
                "Fixed batch size of {} tasks running in parallel",
                batch_size
            ),
        };
        batches.push(batch);
    }

    let plan = ExecutionPlan {
        total_tasks: tasks.len(),
        total_batches: batches.len(),
        batches,
        dependencies_summary: DependenciesSummary {
            critical_path: Vec::new(),
            parallelization_potential: if batches.len() > 1 { "high" } else { "low" }.to_string(),
            parallelization_explanation: format!(
                "Tasks split into {} fixed-size batches of up to {} tasks each",
                batches.len(), batch_size
            ),
        },
    };

    serde_yaml::to_string(&plan).unwrap()
}

/// Generate execution plan using AI agent for dependency analysis
pub async fn generate_execution_plan_ai(
    tasks_overview_yaml: &str,
) -> Result<String> {
    log_phase_start!(0, "Batch Planning", "Analyzing dependencies with AI agent");

    let system_prompt = "You are an execution planning specialist..."; // Full prompt

    let execution_plan_template = "execution_plan:\n  total_tasks: ..."; // Full template

    let prompt = format!(
        "Analyze the tasks and their dependencies, then generate an execution plan.\n\n\
         # Tasks Overview:\n```yaml\n{}\n```\n\n\
         # Execution Plan Template:\n```yaml\n{}\n```\n\n\
         Generate a complete execution_plan.yaml...",
        tasks_overview_yaml, execution_plan_template
    );

    let options = ClaudeAgentOptions {
        system_prompt: Some(system_prompt.to_string()),
        allowed_tools: vec!["Read".to_string()],
        permission_mode: "bypassPermissions".to_string(),
        ..Default::default()
    };

    let mut client = ClaudeSDKClient::new(options)?;
    client.query(&prompt).await?;

    let (response, _) = extract_text_and_stats(&mut client).await?;
    Ok(clean_yaml_response(&response))
}

/// Parse execution plan and convert to batches
pub fn parse_execution_plan(
    execution_plan_yaml: &str,
    tasks: &[TaskOverview],
    debug: bool,
) -> Result<Vec<Vec<TaskOverview>>> {
    let plan: ExecutionPlan = serde_yaml::from_str(execution_plan_yaml)?;

    // Build task lookup by ID
    let mut task_by_id = std::collections::HashMap::new();
    for task in tasks {
        task_by_id.insert(task.task.id, task.clone());
    }

    let mut batches = Vec::new();
    for batch_def in plan.batches {
        if debug {
            log_debug!("Batch {}: {} tasks", batch_def.batch_id, batch_def.tasks.len());
        }

        let mut batch_tasks = Vec::new();
        for task_ref in batch_def.tasks {
            if let Some(task) = task_by_id.get(&task_ref.task_id) {
                batch_tasks.push(task.clone());
            } else {
                log_warning!("Task {} not found in tasks_overview", task_ref.task_id);
            }
        }

        if !batch_tasks.is_empty() {
            batches.push(batch_tasks);
        }
    }

    Ok(batches)
}

/// Fallback: Build execution batches based on simple dependency analysis
pub fn build_execution_batches_fallback(
    tasks: &[TaskOverview],
) -> Vec<Vec<TaskOverview>> {
    // Build task lookup by ID
    let mut task_by_id = std::collections::HashMap::new();
    for task in tasks {
        task_by_id.insert(task.task.id, task.clone());
    }

    let mut scheduled = std::collections::HashSet::new();
    let mut batches = Vec::new();

    while scheduled.len() < tasks.len() {
        let mut current_batch = Vec::new();

        for task in tasks {
            if scheduled.contains(&task.task.id) {
                continue;
            }

            // Check if all dependencies are satisfied
            let dependencies = &task.dependencies.requires_completion_of;
            let can_run = if dependencies.is_empty() {
                true
            } else {
                dependencies.iter().all(|dep| scheduled.contains(&dep.task_id))
            };

            if can_run {
                current_batch.push(task.clone());
                scheduled.insert(task.task.id);
            }
        }

        if current_batch.is_empty() {
            // Circular dependency detected
            log_warning!("Circular dependency detected or unresolved dependencies");
            let remaining: Vec<_> = tasks.iter()
                .filter(|t| !scheduled.contains(&t.task.id))
                .cloned()
                .collect();
            if !remaining.is_empty() {
                batches.push(remaining);
            }
            break;
        }

        batches.push(current_batch);
    }

    batches
}
```

**Estimated time**: 5 hours

#### 5. `src/task_planner/step1_overview.rs` (~150 lines)

**Translate step1_generate_overview (lines 372-441):**

```rust
use anyhow::Result;
use claude_agent_sdk::{ClaudeSDKClient, ClaudeAgentOptions};
use workflow_manager_sdk::{log_phase_start, log_stats};

use crate::task_planner::types::UsageStats;
use crate::task_planner::utils::{clean_yaml_response, extract_text_and_stats};

/// Main orchestrator generates tasks_overview.yaml from IMPL.md
pub async fn step1_generate_overview(
    impl_md: &str,
    overview_template: &str,
) -> Result<(String, UsageStats)> {
    log_phase_start!(1, "Main Orchestrator", "Generate tasks_overview.yaml from IMPL.md");

    let system_prompt = r#"You are a task planning specialist focused on generating high-level task overviews.

Your goal is to analyze the implementation document and generate a tasks_overview.yaml file that breaks down the implementation into logical tasks.

Key instructions:
- Generate YAML that follows the tasks_overview_template.yaml structure exactly
- Create one task block per logical implementation objective
- Keep descriptions strategic and high-level (WHAT and WHY, not HOW)
- Assign sequential task IDs starting from 1
- Identify dependencies between tasks accurately
- Focus on business/architectural value and outcomes
- Estimate complexity and effort realistically

Output only valid YAML, no markdown code blocks or extra commentary."#;

    let prompt = format!(
        r#"Using the implementation document below, generate tasks_overview.yaml following the template structure.

# Implementation Document:
```
{}
```

# Template Structure (tasks_overview_template.yaml):
```yaml
{}
```

Generate a complete tasks_overview.yaml with all tasks identified from the implementation document. Use YAML multi-document format (separate tasks with ---) if there are multiple tasks.

Make sure to just give your response. You must not create or write any files just output the yaml and only that.
"#,
        impl_md, overview_template
    );

    let options = ClaudeAgentOptions {
        system_prompt: Some(system_prompt.to_string()),
        allowed_tools: vec!["Read".to_string(), "Grep".to_string(), "Glob".to_string()],
        permission_mode: "bypassPermissions".to_string(),
        ..Default::default()
    };

    let mut client = ClaudeSDKClient::new(options)?;
    client.query(&prompt).await?;

    let (response, usage_stats) = extract_text_and_stats(&mut client).await?;

    // Log statistics
    log_stats!(
        usage_stats.duration_ms,
        usage_stats.num_turns,
        usage_stats.total_cost_usd.unwrap_or(0.0),
        usage_stats.usage.input_tokens,
        usage_stats.usage.output_tokens
    );

    Ok((clean_yaml_response(&response), usage_stats))
}
```

**Estimated time**: 3 hours

#### 6. `src/task_planner/step2_expand.rs` (~400 lines)

**Translate step2_expand_all_tasks and suborchestrator_expand_task (lines 449-738, 1010-1271):**

This is the most complex module. Key translations:
- Remove AgentLogger (use workflow macros)
- Remove Live display (use regular logging)
- Remove keyboard input handling
- Keep parallel execution with `futures::join_all`
- Use logging macros for all output

```rust
use anyhow::Result;
use futures::future::join_all;
use std::path::Path;

use claude_agent_sdk::{
    ClaudeAgentOptions, AgentDefinition, query,
    AssistantMessage, TextBlock, ToolUseBlock, UserMessage, ToolResultBlock, ResultMessage,
};
use workflow_manager_sdk::{
    log_phase_start, log_batch_start, log_batch_complete,
    log_task_start, log_task_complete, log_agent_start, log_agent_complete,
    log_delegate_to, log_parallel_start, log_parallel_complete,
    log_aggregate_stats, log_file_saved, log_streaming_start, log_streaming_complete,
};

use crate::task_planner::types::{TaskOverview, UsageStats};
use crate::task_planner::utils::{clean_yaml_response, parse_tasks_overview};
use crate::task_planner::execution_plan::{
    generate_execution_plan_simple, generate_execution_plan_ai, parse_execution_plan,
};

/// Suborchestrator expands a single task using sub-agents
pub async fn suborchestrator_expand_task(
    task_overview: &TaskOverview,
    task_template: &str,
    debug: bool,
) -> Result<(String, UsageStats)> {
    let task_id = task_overview.task.id;
    let task_name = &task_overview.task.name;

    log_task_start!(task_id, task_name);
    log_agent_start!("suborchestrator");

    // Define specialized sub-agents
    let agents = create_sub_agents();

    let system_prompt = format!(
        r#"Your task is to expand Task {} ("{}") from a high-level overview into a complete, detailed specification.

## OBJECTIVE
Transform the task overview below into a complete task specification that matches the task_template structure by delegating to specialized agents.

IMPORTANT: You are in the PLANNING phase. DO NOT create, write, or modify any files. Your sole purpose is to OUTPUT a YAML specification that describes what should be implemented.

[... full system prompt ...]
"#,
        task_id, task_name
    );

    let query_prompt = format!(
        r#"Expand Task {} ("{}") by coordinating with your specialized agents.

IMPORTANT: Run all agents in parallel for maximum efficiency:
- Invoke @files, @functions, @formal, and @tests agents simultaneously
- Wait for all agents to complete
- Then combine their outputs into the complete task specification in YAML format."#,
        task_id, task_name
    );

    let options = ClaudeAgentOptions {
        allowed_tools: vec!["Read".to_string(), "Grep".to_string(), "Glob".to_string()],
        system_prompt: Some(system_prompt),
        agents,
        permission_mode: "bypassPermissions".to_string(),
        include_partial_messages: true,
        ..Default::default()
    };

    let mut response_parts = Vec::new();
    let mut usage_stats = None;
    let mut agents_invoked = std::collections::HashSet::new();

    let stream = query(&query_prompt, options).await?;
    for await msg in stream {
        match msg {
            AssistantMessage(msg) => {
                for block in msg.content {
                    if let TextBlock(block) = block {
                        response_parts.push(block.text.clone());

                        // Detect agent invocations
                        for agent_name in &["files", "functions", "formal", "tests"] {
                            if block.text.contains(&format!("@{}", agent_name))
                                && !agents_invoked.contains(*agent_name)
                            {
                                agents_invoked.insert(agent_name.to_string());
                                log_delegate_to!("suborchestrator", agent_name);
                            }
                        }

                        if debug {
                            log_debug!("{}", &block.text[..100.min(block.text.len())]);
                        }
                    } else if let ToolUseBlock(block) = block {
                        if block.name.starts_with("agent_") {
                            let agent_name = block.name.strip_prefix("agent_").unwrap();
                            log_agent_start!(agent_name);
                        }
                    }
                }
            }
            UserMessage(msg) => {
                for block in msg.content {
                    if let ToolResultBlock(block) = block {
                        if debug && block.content.is_some() {
                            let preview = &block.content.as_ref().unwrap()[..200.min(block.content.as_ref().unwrap().len())];
                            log_debug!("Tool result: {}...", preview);
                        }
                        if block.tool_use_id.is_some() {
                            log_agent_complete!();
                        }
                    }
                }
            }
            ResultMessage(msg) => {
                usage_stats = Some(UsageStats {
                    duration_ms: msg.duration_ms,
                    duration_api_ms: msg.duration_api_ms,
                    num_turns: msg.num_turns,
                    total_cost_usd: msg.total_cost_usd,
                    usage: TokenUsage {
                        input_tokens: msg.usage.input_tokens,
                        output_tokens: msg.usage.output_tokens,
                    },
                    session_id: msg.session_id,
                });
            }
            _ => {}
        }
    }

    let combined_output = response_parts.join("\n");
    let cleaned = clean_yaml_response(&combined_output);
    let stats = usage_stats.ok_or_else(|| anyhow::anyhow!("No usage stats"))?;

    log_task_complete!(task_id);
    log_agent_complete!();

    Ok((cleaned, stats))
}

/// Expand all tasks in batches with parallel execution
pub async fn step2_expand_all_tasks(
    tasks_overview_yaml: &str,
    task_template: &str,
    project_root: &Path,
    stream_to_file: bool,
    debug: bool,
    simple_batching: bool,
    batch_size: usize,
) -> Result<String> {
    log_phase_start!(2, "Suborchestrators", "Expand Tasks");

    let tasks = parse_tasks_overview(tasks_overview_yaml)?;
    log_found!(tasks.len(), "tasks");

    // Generate execution plan
    let execution_plan_yaml = if simple_batching {
        generate_execution_plan_simple(&tasks, batch_size)
    } else {
        generate_execution_plan_ai(tasks_overview_yaml).await?
    };

    if debug {
        log_debug!("Execution Plan:\n{}", execution_plan_yaml);
    }

    let batches = parse_execution_plan(&execution_plan_yaml, &tasks, debug)?;
    log_info!("Execution plan: {} batch(es)", batches.len());

    // Execute batches
    let mut all_expanded = Vec::new();
    let mut all_usage_stats = Vec::new();
    let tasks_path = project_root.join("tasks.yaml");

    let mut file_handle = if stream_to_file {
        log_streaming_start!(tasks_path.display());
        Some(std::fs::File::create(&tasks_path)?)
    } else {
        None
    };

    for (batch_num, batch) in batches.iter().enumerate() {
        log_batch_start!(batch_num + 1, batches.len(), batch.len());
        log_parallel_start!(batch.len(), "tasks");

        // Execute tasks in parallel
        let tasks_futures: Vec<_> = batch
            .iter()
            .map(|task| suborchestrator_expand_task(task, task_template, debug))
            .collect();

        let expanded_batch = join_all(tasks_futures).await;

        log_parallel_complete!(batch.len(), "tasks");
        log_batch_complete!(batch_num + 1);

        // Handle results
        for result in expanded_batch {
            let (expanded, usage_stats) = result?;

            if let Some(ref mut file) = file_handle {
                use std::io::Write;
                if !all_expanded.is_empty() {
                    file.write_all(b"\n---\n")?;
                }
                file.write_all(expanded.as_bytes())?;
                file.flush()?;
            } else {
                all_expanded.push(expanded);
            }

            all_usage_stats.push(usage_stats);
        }
    }

    // Aggregate stats
    let total_duration: u64 = all_usage_stats.iter().map(|s| s.duration_ms).sum();
    let total_turns: u32 = all_usage_stats.iter().map(|s| s.num_turns).sum();
    let total_cost: f64 = all_usage_stats.iter()
        .filter_map(|s| s.total_cost_usd)
        .sum();

    log_aggregate_stats!(
        all_usage_stats.len(),
        total_duration,
        total_turns,
        total_cost
    );

    if let Some(_) = file_handle {
        log_streaming_complete!(tasks_path.display());
        Ok(String::new())
    } else {
        Ok(all_expanded.join("\n---\n"))
    }
}

fn create_sub_agents() -> std::collections::HashMap<String, AgentDefinition> {
    // Create the 4 sub-agents: files, functions, formal, tests
    // (Same as Python version)
    todo!("Implement sub-agent definitions")
}
```

**Estimated time**: 8-10 hours (most complex module)

#### 7. `src/task_planner/step3_review.rs` (~300 lines)

**Translate step3_review_tasks and review_suborchestrator (lines 1278-1577):**

```rust
use anyhow::Result;
use std::collections::HashMap;

use claude_agent_sdk::{ClaudeAgentOptions, AgentDefinition, query};
use workflow_manager_sdk::{
    log_phase_start, log_batch_start, log_batch_complete,
    log_review_summary, log_review_issue, log_file_saved,
};

use crate::task_planner::types::{TaskOverview, DetailedTask, ReviewResult};
use crate::task_planner::utils::{parse_tasks_overview, parse_detailed_tasks};

/// Review suborchestrator coordinates @reviewer agents for a batch
async fn review_suborchestrator(
    batch: &[(TaskOverview, DetailedTask)],
    impl_md: &str,
    tasks_overview_yaml: &str,
    task_template: &str,
    batch_num: usize,
    debug: bool,
) -> Result<Vec<ReviewResult>> {
    log_batch_start!(batch_num, 0, batch.len()); // total_batches filled later

    // Define reviewer agent
    let reviewer_agent = AgentDefinition {
        description: "Specialist that validates individual task specifications against requirements".to_string(),
        prompt: r#"You are an implementation plan reviewer.

[... full prompt ...]
"#.to_string(),
        tools: vec!["Read".to_string()],
        model: "sonnet".to_string(),
    };

    let system_prompt = format!(
        r#"You are a review suborchestrator coordinating Step 3: Review & Validation.

## YOUR ROLE
Coordinate the @reviewer agent to validate all {} tasks in your batch.

[... full system prompt ...]
"#,
        batch.len()
    );

    let query_prompt = format!(
        r#"Coordinate review of all {} tasks in your batch.

[... full query prompt with context ...]
"#,
        batch.len()
    );

    let mut agents = HashMap::new();
    agents.insert("reviewer".to_string(), reviewer_agent);

    let options = ClaudeAgentOptions {
        allowed_tools: vec!["Read".to_string()],
        system_prompt: Some(system_prompt),
        agents,
        permission_mode: "bypassPermissions".to_string(),
        include_partial_messages: true,
        ..Default::default()
    };

    // Execute and parse results
    let stream = query(&query_prompt, options).await?;
    let mut response_parts = Vec::new();

    for await msg in stream {
        // Collect response
        if let AssistantMessage(msg) = msg {
            for block in msg.content {
                if let TextBlock(block) = block {
                    response_parts.push(block.text);
                    if block.text.contains("@reviewer") {
                        log_delegate_to!("review_suborchestrator", "reviewer");
                    }
                }
            }
        }
    }

    let combined = response_parts.join("\n");

    // Parse JSON response
    let json_str = if combined.contains("```json") {
        combined.split("```json").nth(1).unwrap().split("```").next().unwrap()
    } else {
        &combined
    };

    let results: Vec<ReviewResult> = serde_json::from_str(json_str.trim())?;

    log_batch_complete!(batch_num);

    Ok(results)
}

/// Main review coordination function
pub async fn step3_review_tasks(
    tasks_overview_yaml: &str,
    tasks_yaml: &str,
    impl_md: &str,
    task_template: &str,
    batch_size: usize,
    debug: bool,
) -> Result<Vec<ReviewResult>> {
    log_phase_start!(3, "Batched Review", "Validate expanded tasks with @reviewer agents");

    let overview_tasks = parse_tasks_overview(tasks_overview_yaml)?;
    let detailed_tasks = parse_detailed_tasks(tasks_yaml)?;

    log_info!("Matching {} overview tasks with {} detailed tasks",
        overview_tasks.len(), detailed_tasks.len());

    // Build lookup and pair tasks
    let mut detailed_map: HashMap<u32, DetailedTask> = HashMap::new();
    for task in detailed_tasks {
        detailed_map.insert(task.task.id, task);
    }

    let mut task_pairs = Vec::new();
    for overview in overview_tasks {
        if let Some(detailed) = detailed_map.remove(&overview.task.id) {
            task_pairs.push((overview, detailed));
        } else {
            log_warning!("No detailed task found for overview task {}", overview.task.id);
        }
    }

    // Create batches
    let batches: Vec<_> = task_pairs.chunks(batch_size).collect();
    log_info!("Created {} batch(es) with batch_size={}", batches.len(), batch_size);

    // Process batches
    let mut all_results = Vec::new();
    for (batch_num, batch) in batches.iter().enumerate() {
        let batch_results = review_suborchestrator(
            batch,
            impl_md,
            tasks_overview_yaml,
            task_template,
            batch_num + 1,
            debug,
        ).await?;

        all_results.extend(batch_results);
    }

    Ok(all_results)
}

/// Generate final review report
pub async fn step3_main_orchestrator_report(
    review_results: &[ReviewResult],
    report_path: &Path,
) -> Result<()> {
    log_phase_start!(0, "Final Report", "Main Orchestrator Summary");

    let approved = review_results.iter().filter(|r| r.success).count();
    let needs_revision = review_results.len() - approved;

    log_review_summary!(approved, needs_revision, review_results.len());

    if needs_revision > 0 {
        log_warning!("Tasks requiring revision:");
        for result in review_results {
            if !result.success {
                log_review_issue!(result.task_id, &result.summary);
                for issue in &result.issues {
                    log_info!("  - {}", issue);
                }
            }
        }
    } else {
        log_success!("All tasks approved! Ready for implementation.");
    }

    // Save report to file
    let mut report = String::new();
    report.push_str(&"=".repeat(80));
    report.push_str("\nTASK REVIEW REPORT\n");
    report.push_str(&"=".repeat(80));
    report.push_str(&format!("\n\nTotal tasks: {}\n", review_results.len()));
    report.push_str(&format!("Approved: {}\n", approved));
    report.push_str(&format!("Needs revision: {}\n\n", needs_revision));

    for result in review_results {
        report.push_str(&format!(
            "\nTask {}: {}\n",
            result.task_id,
            if result.success { "APPROVED" } else { "NEEDS REVISION" }
        ));
        report.push_str(&format!("Summary: {}\n", result.summary));
        if !result.issues.is_empty() {
            report.push_str("Issues:\n");
            for issue in &result.issues {
                report.push_str(&format!("  - {}\n", issue));
            }
        }
        report.push_str("\n");
    }

    std::fs::write(report_path, report)?;
    log_file_saved!(report_path.display());

    Ok(())
}
```

**Estimated time**: 6-7 hours

#### 8. `src/task_planner/workflow.rs` (~250 lines)

**Translate main workflow (lines 1640-1850):**

```rust
use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::task_planner::{
    cli::Args,
    utils::{load_template, load_impl_md, load_impl_files, save_yaml},
    step1_overview::step1_generate_overview,
    step2_expand::step2_expand_all_tasks,
    step3_review::{step3_review_tasks, step3_main_orchestrator_report},
};

/// Main workflow configuration
pub struct WorkflowConfig {
    pub step: String,
    pub impl_md: Option<String>,
    pub tasks_overview_yaml: Option<String>,
    pub tasks_yaml: Option<String>,
    pub overview_template: Option<String>,
    pub task_template: Option<String>,
    pub project_root: PathBuf,
    pub stream_to_file: bool,
    pub debug: bool,
    pub batch_size: Option<usize>,
}

impl From<Args> for WorkflowConfig {
    fn from(args: Args) -> Self {
        WorkflowConfig {
            step: args.step,
            impl_md: None, // Loaded separately
            tasks_overview_yaml: args.tasks_overview,
            tasks_yaml: args.tasks,
            overview_template: args.tasks_overview_template,
            task_template: args.task_template,
            project_root: std::env::current_dir().unwrap(),
            stream_to_file: args.stream,
            debug: args.debug,
            batch_size: args.batch_size,
        }
    }
}

/// Run the task planning workflow
pub async fn run_task_planning_workflow(mut config: WorkflowConfig) -> Result<()> {
    // Step-specific validations and loading

    // Step 1 or all: Generate overview
    if config.step == "1" || config.step == "all" {
        let overview_template = load_template(
            &PathBuf::from(config.overview_template.as_ref().unwrap())
        )?;

        let impl_md = config.impl_md.as_ref().unwrap();

        let (tasks_overview_yaml, _stats) = step1_generate_overview(
            impl_md,
            &overview_template,
        ).await?;

        let overview_path = config.project_root.join("tasks_overview.yaml");
        save_yaml(&tasks_overview_yaml, &overview_path)?;

        if config.step == "1" {
            return Ok(());
        }
    } else {
        // Load existing overview
        let overview_path = config.tasks_overview_yaml
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| config.project_root.join("tasks_overview.yaml"));

        config.tasks_overview_yaml = Some(std::fs::read_to_string(&overview_path)?);
    }

    // Step 2 or all: Expand tasks
    if config.step == "2" || config.step == "all" {
        let task_template = load_template(
            &PathBuf::from(config.task_template.as_ref().unwrap())
        )?;

        let simple_batching = config.batch_size.is_some();
        let batch_size = config.batch_size.unwrap_or(5);

        let tasks_yaml = step2_expand_all_tasks(
            config.tasks_overview_yaml.as_ref().unwrap(),
            &task_template,
            &config.project_root,
            config.stream_to_file,
            config.debug,
            simple_batching,
            batch_size,
        ).await?;

        if !tasks_yaml.is_empty() {
            let tasks_path = config.project_root.join("tasks.yaml");
            save_yaml(&tasks_yaml, &tasks_path)?;
        }

        if config.step == "2" {
            return Ok(());
        }
    } else {
        // Load existing tasks
        let tasks_path = config.tasks_yaml
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| config.project_root.join("tasks.yaml"));

        config.tasks_yaml = Some(std::fs::read_to_string(&tasks_path)?);
    }

    // Step 3 or all: Review tasks
    if config.step == "3" || config.step == "all" {
        let task_template = load_template(
            &PathBuf::from(config.task_template.as_ref().unwrap())
        )?;

        let impl_md = config.impl_md.as_ref().unwrap();
        let batch_size = config.batch_size.unwrap_or(5);

        let review_results = step3_review_tasks(
            config.tasks_overview_yaml.as_ref().unwrap(),
            config.tasks_yaml.as_ref().unwrap(),
            impl_md,
            &task_template,
            batch_size,
            config.debug,
        ).await?;

        let report_path = config.project_root.join("task_review_report.txt");
        step3_main_orchestrator_report(&review_results, &report_path).await?;
    }

    Ok(())
}
```

**Estimated time**: 4 hours

#### 9. `src/task_planner/mod.rs` (~150 lines)

**Module organization and re-exports:**

```rust
//! Task planning workflow automation using multi-agent coordination.
//!
//! This module provides a 3-step workflow for generating detailed implementation tasks:
//!
//! 1. **Step 1**: Generate high-level task overview from IMPL.md
//! 2. **Step 2**: Expand tasks into detailed specifications using suborchestrators
//! 3. **Step 3**: Review and validate task specifications
//!
//! # Quick Start
//!
//! ```no_run
//! use workflow_manager::task_planner::{run_task_planning_workflow, WorkflowConfig};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = WorkflowConfig {
//!     step: "all".to_string(),
//!     impl_md: Some("...".to_string()),
//!     // ... other config
//! };
//!
//! run_task_planning_workflow(config).await?;
//! # Ok(())
//! # }
//! ```

pub mod types;
pub mod cli;
pub mod utils;
pub mod execution_plan;
pub mod step1_overview;
pub mod step2_expand;
pub mod step3_review;
pub mod workflow;

// Re-export commonly used types
pub use types::{
    TaskOverview, DetailedTask, ExecutionPlan, ReviewResult, UsageStats,
};
pub use workflow::{run_task_planning_workflow, WorkflowConfig};
```

**Estimated time**: 2 hours

#### 10. `src/bin/task_planner.rs` (~30 lines)

**Thin CLI wrapper:**

```rust
use clap::Parser;
use workflow_manager::task_planner::{cli::Args, run_task_planning_workflow, WorkflowConfig};
use workflow_manager::task_planner::utils::{load_impl_md, load_impl_files};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Handle workflow metadata
    if args.workflow_metadata {
        args.print_metadata();
        return Ok(());
    }

    // Load IMPL.md if needed
    let mut config: WorkflowConfig = args.into();

    if config.step == "1" || config.step == "3" || config.step == "all" {
        config.impl_md = Some(if let Some(impl_files) = args.impl_files {
            load_impl_files(&impl_files)?
        } else {
            load_impl_md(&config.project_root)?
        });
    }

    // Run workflow
    run_task_planning_workflow(config).await
}
```

**Estimated time**: 1 hour

---

## Phase 3: Add Tests

### Test Structure

```
tests/task_planner/
├── common.rs              # Shared test utilities
├── test_types.rs          # Data structure tests
├── test_utils.rs          # Utility function tests
├── test_execution_plan.rs # Execution planning tests
└── test_workflow.rs       # Integration tests
```

### Test Coverage

1. **Type tests** (~50 tests)
   - TaskOverview serialization/deserialization
   - DetailedTask validation
   - ExecutionPlan parsing

2. **Utility tests** (~30 tests)
   - YAML parsing with edge cases
   - Template loading
   - Clean YAML response

3. **Execution plan tests** (~20 tests)
   - Simple batching
   - Dependency analysis (fallback)
   - Circular dependency detection

4. **Workflow tests** (~15 tests)
   - Config validation
   - File loading
   - Error handling

**Estimated time**: 6-8 hours

---

## Summary

### Total Estimated Time

| Phase | Task | Hours |
|-------|------|-------|
| 1 | Add logging macros to SDK | 3-4 |
| 2 | Module 1: types.rs | 4-5 |
| 2 | Module 2: cli.rs | 2 |
| 2 | Module 3: utils.rs | 4 |
| 2 | Module 4: execution_plan.rs | 5 |
| 2 | Module 5: step1_overview.rs | 3 |
| 2 | Module 6: step2_expand.rs | 8-10 |
| 2 | Module 7: step3_review.rs | 6-7 |
| 2 | Module 8: workflow.rs | 4 |
| 2 | Module 9: mod.rs | 2 |
| 2 | Module 10: binary | 1 |
| 3 | Tests | 6-8 |
| **Total** | | **48-58 hours** |

### Deliverables

✅ New logging macros in workflow-manager-sdk
✅ Task planner library with 9 modules
✅ Thin binary wrapper (~30 lines)
✅ Comprehensive test suite (~115 tests)
✅ Full documentation with rustdoc
✅ Clean public API for TUI integration

### Success Criteria

- [ ] All compilation succeeds without warnings
- [ ] Binary CLI works identically to Python version
- [ ] All 3 steps execute correctly
- [ ] Output files match Python version format
- [ ] Code size in binary < 50 lines
- [ ] Library modules under 400 lines each
- [ ] Public API documented
- [ ] Integration tests for each step
- [ ] 90%+ test coverage

### Benefits vs Python Version

✅ **Type safety**: Catch errors at compile time
✅ **Performance**: 10-100x faster execution
✅ **Memory efficiency**: Lower memory footprint
✅ **Maintainability**: Modular architecture
✅ **Testability**: Unit tests for all modules
✅ **Integration**: Works with iced TUI
✅ **Consistency**: Same logging as research module
✅ **No Rich dependency**: Simpler, cleaner logging

---

## Next Steps

1. **Review this plan** - Approve or suggest modifications
2. **Phase 1** - Add logging macros to workflow-manager-sdk
3. **Phase 2** - Implement modules in order (types → utils → steps → workflow → binary)
4. **Phase 3** - Add comprehensive tests
5. **Documentation** - Write usage guide and examples

**Ready to begin?**
