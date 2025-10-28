//! Execution planning for task batching and dependency analysis.
//!
//! This module provides functions for:
//! - Generating execution plans (simple fixed-size or AI-based)
//! - Parsing execution plans into task batches
//! - Fallback dependency analysis

use anyhow::{Context, Result};
use futures::StreamExt;
use std::collections::{HashMap, HashSet};

use crate::task_planner::types::{Batch, BatchTask, DependenciesSummary, ExecutionPlan, TaskOverview};
use crate::task_planner::utils::clean_yaml_response;

/// Generate simple execution plan by chunking tasks into fixed-size batches
pub fn generate_execution_plan_simple(tasks: &[TaskOverview], batch_size: usize) -> String {
    println!("\nBatch Planning: Simple batching");
    println!("Using fixed batch size: {}", batch_size);

    let mut batches = Vec::new();

    for (batch_idx, chunk) in tasks.chunks(batch_size).enumerate() {
        let batch_id = batch_idx + 1;
        let start_task = batch_idx * batch_size + 1;
        let end_task = ((batch_idx + 1) * batch_size).min(tasks.len());

        let batch = Batch {
            batch_id,
            description: format!("Batch {} - Tasks {} to {}", batch_id, start_task, end_task),
            strategy: "sequential".to_string(),
            tasks: chunk
                .iter()
                .map(|t| BatchTask {
                    task_id: t.task.id,
                    task_name: t.task.name.clone(),
                    reason: format!("Part of batch {}", batch_id),
                })
                .collect(),
            parallelization_rationale: format!(
                "Fixed batch size of {} tasks running in parallel",
                batch_size
            ),
        };
        batches.push(batch);
    }

    let total_batches = batches.len();
    let plan = ExecutionPlan {
        total_tasks: tasks.len(),
        total_batches,
        batches,
        dependencies_summary: DependenciesSummary {
            critical_path: Vec::new(),
            parallelization_potential: if total_batches > 1 {
                "high".to_string()
            } else {
                "low".to_string()
            },
            parallelization_explanation: format!(
                "Tasks split into {} fixed-size batches of up to {} tasks each",
                total_batches,
                batch_size
            ),
        },
    };

    serde_yaml::to_string(&plan).unwrap_or_else(|e| {
        println!("⚠ Warning: Failed to serialize execution plan: {}", e);
        String::new()
    })
}

/// Generate execution plan using AI agent for dependency analysis
pub async fn generate_execution_plan_ai(tasks_overview_yaml: &str) -> Result<String> {
    println!("\nBatch Planning: Analyzing dependencies with AI agent");

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
          reason: "[Why this task is in this batch - e.g., 'No dependencies' or 'Depends on batch 1']"

      # Tasks within this batch can run in parallel because:
      parallelization_rationale: |
        [Explain why these tasks can run in parallel.
        E.g., "All tasks have no dependencies" or
        "All dependencies from previous batches are satisfied"]

  dependencies_summary:
    critical_path:
      # Longest dependency chain
      - task_id: [NUMBER]
      - task_id: [NUMBER]

    parallelization_potential: "[low|medium|high]"
    parallelization_explanation: |
      [Explain the overall parallelization potential.
      E.g., "High - 10 out of 14 tasks can run in parallel across 3 batches"]"#;

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

    let options = claude_agent_sdk::ClaudeAgentOptions {
        system_prompt: Some(claude_agent_sdk::SystemPrompt::String(system_prompt.to_string())),
        allowed_tools: vec!["Read".to_string().into()],
        permission_mode: Some(claude_agent_sdk::PermissionMode::BypassPermissions),
        ..Default::default()
    };

    let stream = claude_agent_sdk::query(&prompt, Some(options))
        .await
        .context("Failed to query Claude agent for execution plan")?;

    // Convert stream to handle anyhow::Error using map
    let stream = stream.map(|result| {
        result.with_context(|| "Stream error while generating execution plan - request may have been aborted")
    });

    let (response, _) = crate::task_planner::utils::extract_text_and_stats(stream)
        .await
        .context("Failed to extract response from execution plan agent")?;

    Ok(clean_yaml_response(&response))
}

/// Parse execution plan and convert to batches
pub fn parse_execution_plan(
    execution_plan_yaml: &str,
    tasks: &[TaskOverview],
    debug: bool,
) -> Result<Vec<Vec<TaskOverview>>> {
    let plan: serde_yaml::Value = serde_yaml::from_str(execution_plan_yaml)
        .context("Failed to parse execution plan YAML")?;

    // Extract execution_plan object
    let plan_obj = plan
        .get("execution_plan")
        .or(Some(&plan))
        .context("Missing 'execution_plan' key in YAML")?;

    // Build task lookup by ID
    let mut task_by_id: HashMap<u32, TaskOverview> = HashMap::new();
    for task in tasks {
        task_by_id.insert(task.task.id, task.clone());
    }

    let mut batches = Vec::new();

    // Extract batches array
    if let Some(batches_array) = plan_obj.get("batches") {
        if let Some(batches_seq) = batches_array.as_sequence() {
            if debug {
                println!(
                    "Parsing {} batches from execution plan",
                    batches_seq.len()
                );
            }

            for batch_value in batches_seq {
                let batch_id = batch_value
                    .get("batch_id")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                if let Some(tasks_array) = batch_value.get("tasks") {
                    if let Some(tasks_seq) = tasks_array.as_sequence() {
                        if debug {
                            println!("  Batch {}: {} tasks", batch_id, tasks_seq.len());
                        }

                        let mut batch_tasks = Vec::new();

                        for task_ref in tasks_seq {
                            let task_id = task_ref
                                .get("task_id")
                                .and_then(|v| v.as_u64())
                                .map(|v| v as u32);

                            if let Some(tid) = task_id {
                                if let Some(task) = task_by_id.get(&tid) {
                                    batch_tasks.push(task.clone());
                                } else {
                                    println!("⚠ Warning: Task {} not found in tasks_overview", tid);
                                }
                            }
                        }

                        if !batch_tasks.is_empty() {
                            batches.push(batch_tasks);
                        }
                    }
                }
            }
        }
    }

    if batches.is_empty() {
        println!("⚠ Warning: No batches found in execution plan, using fallback");
        return Ok(build_execution_batches_fallback(tasks));
    }

    Ok(batches)
}

/// Fallback: Build execution batches based on simple dependency analysis
pub fn build_execution_batches_fallback(tasks: &[TaskOverview]) -> Vec<Vec<TaskOverview>> {
    println!("Using fallback dependency analysis");

    // Build task lookup by ID
    let mut task_by_id: HashMap<u32, TaskOverview> = HashMap::new();
    for task in tasks {
        task_by_id.insert(task.task.id, task.clone());
    }

    // Track which tasks have been scheduled
    let mut scheduled: HashSet<u32> = HashSet::new();
    let mut batches = Vec::new();

    while scheduled.len() < tasks.len() {
        // Find tasks that can run now (all dependencies satisfied)
        let mut current_batch = Vec::new();

        for task in tasks {
            let task_id = task.task.id;

            if scheduled.contains(&task_id) {
                continue;
            }

            // Check if all dependencies are satisfied
            let dependencies = &task.dependencies.requires_completion_of;

            let can_run = if dependencies.is_empty() {
                true
            } else {
                dependencies
                    .iter()
                    .all(|dep| scheduled.contains(&dep.task_id))
            };

            if can_run {
                current_batch.push(task.clone());
                scheduled.insert(task_id);
            }
        }

        if current_batch.is_empty() {
            // Circular dependency detected
            println!("⚠ Warning: Circular dependency detected or unresolved dependencies");

            // Add remaining tasks to avoid infinite loop
            let remaining: Vec<TaskOverview> = tasks
                .iter()
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

    println!("Fallback analysis created {} batches", batches.len());
    batches
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_planner::types::{Dependencies, TaskDependency, TaskInfo};

    fn create_test_task(id: u32, name: &str, deps: Vec<u32>) -> TaskOverview {
        TaskOverview {
            task: TaskInfo {
                id,
                name: name.to_string(),
                context: String::new(),
            },
            dependencies: Dependencies {
                requires_completion_of: deps
                    .into_iter()
                    .map(|dep_id| TaskDependency {
                        task_id: dep_id,
                        reason: format!("Depends on task {}", dep_id),
                    })
                    .collect(),
            },
        }
    }

    #[test]
    fn test_simple_execution_plan() {
        let tasks = vec![
            create_test_task(1, "Task 1", vec![]),
            create_test_task(2, "Task 2", vec![]),
            create_test_task(3, "Task 3", vec![]),
        ];

        let plan_yaml = generate_execution_plan_simple(&tasks, 2);
        assert!(plan_yaml.contains("batch_id: 1"));
        assert!(plan_yaml.contains("batch_id: 2"));
        assert!(plan_yaml.contains("total_tasks: 3"));
    }

    #[test]
    fn test_dependency_analysis_fallback() {
        let tasks = vec![
            create_test_task(1, "Task 1", vec![]),
            create_test_task(2, "Task 2", vec![1]),
            create_test_task(3, "Task 3", vec![1]),
            create_test_task(4, "Task 4", vec![2, 3]),
        ];

        let batches = build_execution_batches_fallback(&tasks);

        // Batch 1: Task 1 (no deps)
        assert_eq!(batches[0].len(), 1);
        assert_eq!(batches[0][0].task.id, 1);

        // Batch 2: Tasks 2 and 3 (both depend on 1)
        assert_eq!(batches[1].len(), 2);

        // Batch 3: Task 4 (depends on 2 and 3)
        assert_eq!(batches[2].len(), 1);
        assert_eq!(batches[2][0].task.id, 4);
    }

    #[test]
    fn test_circular_dependency_detection() {
        let tasks = vec![
            create_test_task(1, "Task 1", vec![2]),
            create_test_task(2, "Task 2", vec![1]),
        ];

        let batches = build_execution_batches_fallback(&tasks);

        // Should handle circular dependency by grouping remaining tasks
        assert!(!batches.is_empty());
    }
}
