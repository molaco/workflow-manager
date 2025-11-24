//! Phase 2: Review and validate task specifications
//!
//! This phase:
//! - Matches overview tasks with detailed specifications
//! - Executes review suborchestrators in parallel batches
//! - Each suborchestrator coordinates @reviewer agents
//! - Validates: completeness, consistency, correctness, testability
//! - Generates final review report

use crate::task_planner::utils::{get_task_id, get_task_name};
use crate::workflow_utils::{execute_agent, execute_batch, execute_task, parse_yaml_multi, AgentConfig};
use anyhow::{Context, Result};
use claude_agent_sdk::{AgentDefinition, ClaudeAgentOptions};
use serde_yaml::Value;
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;

/// Review result structure (parsed from JSON)
struct ReviewResult {
    task_id: u32,
    success: bool,
    issues: Vec<String>,
    summary: String,
}

/// Execute review suborchestrator for a batch of tasks
async fn review_batch(
    batch: Vec<(Value, Value)>, // (overview, detailed) pairs
    impl_md: &str,
    task_template: &str,
) -> Result<Vec<ReviewResult>> {
    // Define the reviewer agent
    let reviewer_agent = AgentDefinition {
        description: "Specialist that validates individual task specifications against requirements".to_string(),
        prompt: r#"You are an implementation plan reviewer.

Your job is to validate that a detailed task specification (from tasks.yaml) matches its overview (from tasks_overview.yaml) and aligns with the IMPL.md requirements.

You will receive:
1. Implementation requirements (IMPL.md)
2. Task overview YAML (high-level strategic description)
3. Detailed task specification YAML (complete implementation spec)

Check for:
1. Completeness: All key components from overview are specified in detail
2. Consistency: Detailed spec aligns with overview purpose and scope
3. Correctness: Implementation approach makes sense for the requirements
4. Testability: Tests adequately cover the functionality
5. Dependencies: External dependencies are properly identified
6. Template adherence: Detailed spec follows the task_template structure

Report any issues found. If everything looks good, confirm that.

Format your response as:
ASSESSMENT: [APPROVED|NEEDS_REVISION]
ISSUES: [List any issues, or "None"]
SUMMARY: [Brief summary]"#.to_string(),
        tools: Some(vec!["Read".to_string()]),
        model: Some("sonnet".to_string()),
    };

    // Build task list for suborchestrator
    let task_list: Vec<String> = batch
        .iter()
        .map(|(overview, _)| {
            let task_id = get_task_id(overview).unwrap_or(0);
            let task_name = get_task_name(overview).unwrap_or("Unknown");
            format!("  - Task {}: {}", task_id, task_name)
        })
        .collect();

    // System prompt for suborchestrator
    let system_prompt = format!(
        r#"You are a review suborchestrator coordinating Phase 2: Review & Validation.

## YOUR ROLE
Coordinate the @reviewer agent to validate all {} tasks in your batch.

## AVAILABLE CONTEXT
- Implementation requirements (IMPL.md)
- Task overview structure (tasks_overview.yaml)
- Task template structure (task_template.yaml)
- Individual task details (provided when you invoke @reviewer)

## YOUR AGENT
**@reviewer** - Validates individual task specifications
- Input: Task overview + detailed spec + IMPL.md context
- Output: ASSESSMENT, ISSUES, SUMMARY

## WORKFLOW
1. For each task in your batch, invoke @reviewer agent with the task's overview and detailed spec
2. Run ALL @reviewer invocations in parallel for efficiency
3. Parse each reviewer's response to extract ASSESSMENT, ISSUES, and SUMMARY
4. Combine all results into a JSON array

## OUTPUT FORMAT
Output ONLY a valid JSON array with this exact structure:
[
  {{
    "task_id": <task_id_number>,
    "success": <true|false>,
    "issues": [<list of issue strings, or empty array>],
    "summary": "<brief summary string>"
  }},
  ...
]

IMPORTANT:
- Convert ASSESSMENT to success boolean (APPROVED=true, NEEDS_REVISION=false)
- Output ONLY the JSON array, no markdown code blocks, no extra commentary"#,
        batch.len()
    );

    let query_prompt = format!(
        r#"Coordinate review of all {} tasks in your batch.

## CONTEXT

### Implementation Requirements (IMPL.md):
```
{}
```

### Expected Task Template Structure (task_template.yaml):
```yaml
{}
```

## YOUR BATCH
Review these tasks:
{}

## INSTRUCTIONS
For EACH task above:
1. Extract the task's overview and detailed spec
2. Invoke @reviewer with both
3. Parse the reviewer's response

Run ALL @reviewer agents in PARALLEL, then combine results into JSON array."#,
        batch.len(),
        impl_md,
        task_template,
        task_list.join("\n")
    );

    let options = ClaudeAgentOptions::builder()
        .system_prompt(system_prompt)
        .allowed_tools(vec!["Read".to_string()])
        .add_agent("reviewer", reviewer_agent)
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    let config = AgentConfig::new(
        "review_batch",
        "Review Suborchestrator",
        format!("Reviewing {} tasks", batch.len()),
        query_prompt,
        options,
    );

    let response = execute_agent(config).await?;

    // Parse JSON response
    let json_str = if response.contains("```json") {
        response
            .split("```json")
            .nth(1)
            .and_then(|s| s.split("```").next())
            .unwrap_or(&response)
            .trim()
    } else if response.contains("```") {
        response
            .split("```")
            .nth(1)
            .and_then(|s| s.split("```").next())
            .unwrap_or(&response)
            .trim()
    } else {
        response.trim()
    };

    let json_value: serde_json::Value = serde_json::from_str(json_str)
        .context("Failed to parse review results JSON")?;

    let results_array = json_value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Expected JSON array"))?;

    let mut results = Vec::new();
    for result in results_array {
        let task_id = result["task_id"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Missing task_id"))?
            as u32;
        let success = result["success"]
            .as_bool()
            .ok_or_else(|| anyhow::anyhow!("Missing success"))?;
        let issues: Vec<String> = result["issues"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let summary = result["summary"]
            .as_str()
            .unwrap_or("No summary")
            .to_string();

        results.push(ReviewResult {
            task_id,
            success,
            issues,
            summary,
        });
    }

    Ok(results)
}

/// Phase 2: Review all tasks
pub async fn review_tasks(
    tasks_overview_yaml: &str,
    tasks_yaml: &str,
    impl_md: &str,
    task_template: &str,
    batch_size: usize,
) -> Result<()> {
    println!("\n{}", "=".repeat(80));
    println!("PHASE 2: Batched Review - Validate Tasks");
    println!("{}", "=".repeat(80));

    // Parse both overview and detailed tasks
    let overview_tasks: Vec<Value> = parse_yaml_multi(tasks_overview_yaml)
        .context("Failed to parse tasks_overview.yaml")?;
    let detailed_tasks: Vec<Value> =
        parse_yaml_multi(tasks_yaml).context("Failed to parse tasks.yaml")?;

    println!(
        "Matching {} overview tasks with {} detailed tasks\n",
        overview_tasks.len(),
        detailed_tasks.len()
    );

    // Build lookup map for detailed tasks
    let mut detailed_map: HashMap<u32, Value> = HashMap::new();
    for task in detailed_tasks {
        if let Some(task_id) = get_task_id(&task) {
            detailed_map.insert(task_id, task);
        }
    }

    // Match overview with detailed
    let mut task_pairs = Vec::new();
    for overview in overview_tasks {
        let task_id = get_task_id(&overview).unwrap_or(0);
        if let Some(detailed) = detailed_map.remove(&task_id) {
            task_pairs.push((overview, detailed));
        } else {
            println!(
                "Warning: No detailed task found for overview task {}",
                task_id
            );
        }
    }

    // Create batches
    let batches: Vec<Vec<(Value, Value)>> = task_pairs
        .chunks(batch_size)
        .map(|chunk| chunk.to_vec())
        .collect();

    println!(
        "Created {} batch(es) with batch_size={}\n",
        batches.len(),
        batch_size
    );

    // Execute review batches
    let impl_md_clone = impl_md.to_string();
    let task_template_clone = task_template.to_string();
    let num_batches = batches.len();

    let all_results = execute_batch(
        2, // phase number
        batches,
        num_batches, // run all batches in parallel? or sequential? Let's do sequential for now
        move |batch, ctx| {
            let impl_md = impl_md_clone.clone();
            let task_template = task_template_clone.clone();
            async move {
                // Get batch task IDs for logging
                let task_ids: Vec<u32> = batch.iter().filter_map(|(overview, _)| get_task_id(overview)).collect();
                let batch_desc = if task_ids.len() == 1 {
                    format!("Reviewing task {}", task_ids[0])
                } else {
                    format!("Reviewing {} tasks: {:?}", task_ids.len(), task_ids)
                };

                let results = execute_task(
                    format!("review_batch_{}", ctx.task_number),
                    batch_desc,
                    ctx,
                    || async move {
                        let results = review_batch(batch, &impl_md, &task_template).await?;
                        Ok((results, format!("Review batch complete")))
                    }
                ).await?;

                // Return tuple for execute_batch
                Ok((results, format!("Batch {} complete", ctx.task_number)))
            }
        },
    )
    .await?;

    // Flatten results (extract just the ReviewResult vectors, not the summary messages)
    let all_review_results: Vec<ReviewResult> = all_results
        .into_iter()
        .flat_map(|(results, _)| results)
        .collect();

    // Generate report
    generate_review_report(&all_review_results).await?;

    Ok(())
}

/// Generate final review report
async fn generate_review_report(results: &[ReviewResult]) -> Result<()> {
    println!("\n{}", "=".repeat(80));
    println!("FINAL REPORT: Main Orchestrator Summary");
    println!("{}", "=".repeat(80));

    let approved = results.iter().filter(|r| r.success).count();
    let needs_revision = results.len() - approved;

    println!("Total tasks reviewed: {}", results.len());
    println!("✓ Approved: {}", approved);
    println!("✗ Needs revision: {}\n", needs_revision);

    if needs_revision > 0 {
        println!("Tasks requiring revision:\n");
        for result in results {
            if !result.success {
                println!("  Task {}:", result.task_id);
                for issue in &result.issues {
                    println!("    - {}", issue);
                }
                println!("    Summary: {}\n", result.summary);
            }
        }
    } else {
        println!("✓ All tasks approved! Ready for implementation.\n");
    }

    // Save report to file
    let report_path = Path::new("task_review_report.txt");
    let mut report = String::new();
    report.push_str(&"=".repeat(80));
    report.push_str("\nTASK REVIEW REPORT\n");
    report.push_str(&"=".repeat(80));
    report.push_str(&format!("\n\nTotal tasks: {}\n", results.len()));
    report.push_str(&format!("Approved: {}\n", approved));
    report.push_str(&format!("Needs revision: {}\n\n", needs_revision));

    for result in results {
        report.push_str(&format!(
            "\nTask {}: {}\n",
            result.task_id,
            if result.success {
                "APPROVED"
            } else {
                "NEEDS REVISION"
            }
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

    fs::write(report_path, report).await?;
    println!("✓ Full report saved to: {}", report_path.display());

    Ok(())
}
