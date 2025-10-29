//! Utility functions for task planner workflow

use crate::workflow_utils::{execute_agent, AgentConfig};
use anyhow::{Context, Result};
use claude_agent_sdk::ClaudeAgentOptions;
use serde_yaml::Value;
use std::collections::HashMap;

/// Extract task ID from a task YAML value
pub fn get_task_id(task: &Value) -> Option<u32> {
    task.get("task")?
        .get("id")?
        .as_u64()
        .map(|id| id as u32)
}

/// Extract task name from a task YAML value
pub fn get_task_name(task: &Value) -> Option<&str> {
    task.get("task")?.get("name")?.as_str()
}

/// Generate simple execution plan (fixed-size batches)
pub fn generate_simple_execution_plan(
    tasks: &[Value],
    batch_size: usize,
) -> Result<Vec<Vec<Value>>> {
    println!("\n{}", "=".repeat(80));
    println!("Batch Planning: Simple batching with size={}", batch_size);
    println!("{}", "=".repeat(80));

    let mut batches = Vec::new();
    for chunk in tasks.chunks(batch_size) {
        batches.push(chunk.to_vec());
    }

    println!("Created {} batch(es)", batches.len());
    Ok(batches)
}

/// Generate AI-based execution plan (dependency analysis)
pub async fn generate_ai_execution_plan(tasks_overview_yaml: &str) -> Result<String> {
    println!("\n{}", "=".repeat(80));
    println!("Batch Planning: Analyzing dependencies with AI agent");
    println!("{}", "=".repeat(80));

    let system_prompt = r#"You are an execution planning specialist focused on dependency analysis and batch optimization.

Your goal is to analyze tasks_overview.yaml and generate an optimal execution plan that maximizes parallelization while respecting dependencies.

Key instructions:
- Analyze requires_completion_of for each task
- Group tasks into batches where all tasks in a batch can run in parallel
- Tasks can only be in a batch if ALL their dependencies are in previous batches
- Maximize tasks per batch (more parallelization = faster execution)
- Batches execute sequentially, tasks within batch execute in parallel
- Identify the critical path (longest dependency chain)
- Detect any circular dependencies and warn about them

Output only valid YAML following the template structure, no markdown code blocks or extra commentary."#;

    let execution_plan_template = r#"execution_plan:
  total_tasks: [NUMBER]
  total_batches: [NUMBER]

  batches:
    - batch_id: 1
      description: "[Brief description of what this batch accomplishes]"
      strategy: "sequential"  # All batches execute sequentially
      tasks:
        - task_id: [NUMBER]
          task_name: "[TASK_NAME]"
          reason: "[Why this task is in this batch]"

      parallelization_rationale: |
        [Explain why these tasks can run in parallel]

  dependencies_summary:
    critical_path:
      - task_id: [NUMBER]
    parallelization_potential: "[low|medium|high]"
    parallelization_explanation: |
      [Explain the overall parallelization potential]"#;

    let prompt = format!(
        r#"Analyze the tasks and their dependencies, then generate an execution plan.

# Tasks Overview:
```yaml
{}
```

# Execution Plan Template:
```yaml
{}
```

Generate a complete execution_plan.yaml that:
1. Groups tasks into optimal batches for parallel execution
2. Respects all dependencies (requires_completion_of)
3. Maximizes parallelization potential
4. Includes rationale for each batch
5. Identifies critical path and parallelization potential

Output only the YAML, no markdown formatting."#,
        tasks_overview_yaml, execution_plan_template
    );

    let options = ClaudeAgentOptions::builder()
        .system_prompt(system_prompt.to_string())
        .allowed_tools(vec!["Read".to_string()])
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    let config = AgentConfig::new(
        "execution_plan",
        "Execution Planner",
        "Analyzing task dependencies",
        prompt,
        options,
    );

    let response = execute_agent(config).await?;
    Ok(response)
}

/// Parse execution plan YAML and convert to batch structure
pub fn parse_execution_plan(
    execution_plan_yaml: &str,
    tasks: &[Value],
) -> Result<Vec<Vec<Value>>> {
    let plan: Value = serde_yaml::from_str(execution_plan_yaml)
        .context("Failed to parse execution plan YAML")?;

    // Build task lookup by ID
    let mut task_map: HashMap<u32, Value> = HashMap::new();
    for task in tasks {
        if let Some(task_id) = get_task_id(task) {
            task_map.insert(task_id, task.clone());
        }
    }

    // Extract batches from plan
    let mut batches = Vec::new();
    let plan_batches = plan
        .get("execution_plan")
        .and_then(|ep| ep.get("batches"))
        .and_then(|b| b.as_sequence())
        .ok_or_else(|| anyhow::anyhow!("Missing execution_plan.batches in plan"))?;

    for batch_def in plan_batches {
        let mut batch = Vec::new();

        // Get task refs or use empty vec if none
        let empty_vec = vec![];
        let task_refs = batch_def
            .get("tasks")
            .and_then(|t| t.as_sequence())
            .unwrap_or(&empty_vec);

        for task_ref in task_refs {
            if let Some(task_id) = task_ref.get("task_id").and_then(|id| id.as_u64()) {
                if let Some(task) = task_map.remove(&(task_id as u32)) {
                    batch.push(task);
                }
            }
        }

        if !batch.is_empty() {
            batches.push(batch);
        }
    }

    Ok(batches)
}

/// Fallback: Simple dependency analysis if execution plan fails
pub fn build_execution_batches_fallback(tasks: &[Value]) -> Vec<Vec<Value>> {
    println!("Using fallback dependency analysis");

    // Build task lookup by ID
    let mut task_map: HashMap<u32, Value> = HashMap::new();
    for task in tasks {
        if let Some(task_id) = get_task_id(task) {
            task_map.insert(task_id, task.clone());
        }
    }

    // Track which tasks have been scheduled
    let mut scheduled = std::collections::HashSet::new();
    let mut batches = Vec::new();

    while scheduled.len() < tasks.len() {
        let mut current_batch = Vec::new();

        for task in tasks {
            let task_id = match get_task_id(task) {
                Some(id) => id,
                None => continue,
            };

            if scheduled.contains(&task_id) {
                continue;
            }

            // Check if all dependencies are satisfied
            let dependencies = task
                .get("task")
                .and_then(|t| t.get("dependencies"))
                .and_then(|d| d.get("requires_completion_of"))
                .and_then(|r| r.as_sequence());

            let can_run = match dependencies {
                None => true,
                Some(deps) if deps.is_empty() => true,
                Some(deps) => deps.iter().all(|dep| {
                    dep.get("task_id")
                        .and_then(|id| id.as_u64())
                        .map(|id| scheduled.contains(&(id as u32)))
                        .unwrap_or(true)
                }),
            };

            if can_run {
                current_batch.push(task.clone());
                scheduled.insert(task_id);
            }
        }

        if current_batch.is_empty() {
            // Circular dependency or error - add remaining tasks
            println!("Warning: Circular dependency detected or unresolved dependencies");
            let remaining: Vec<Value> = tasks
                .iter()
                .filter(|t| {
                    get_task_id(t)
                        .map(|id| !scheduled.contains(&id))
                        .unwrap_or(false)
                })
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
