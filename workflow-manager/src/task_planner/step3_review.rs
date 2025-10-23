//! Step 3: Review and validate expanded tasks
//!
//! This module implements the review coordination system that validates
//! detailed task specifications against their overviews and requirements.

use anyhow::{Context, Result};
use futures::StreamExt;
use std::collections::HashMap;
use std::path::Path;

use crate::task_planner::types::{DetailedTask, ReviewResult, TaskOverview};
use crate::task_planner::utils::{parse_detailed_tasks, parse_tasks_overview};
use workflow_manager_sdk::{
    log_batch_complete, log_batch_start, log_debug, log_delegate_to, log_file_saved, log_info,
    log_phase_start_console, log_review_issue, log_review_summary, log_stats, log_warning,
};

/// Review suborchestrator coordinates @reviewer agents for a batch
async fn review_suborchestrator(
    batch: &[(TaskOverview, DetailedTask)],
    impl_md: &str,
    tasks_overview_yaml: &str,
    task_template: &str,
    batch_num: usize,
    total_batches: usize,
    debug: bool,
) -> Result<Vec<ReviewResult>> {
    log_batch_start!(batch_num, total_batches, batch.len());

    // Define the reviewer agent
    let reviewer_agent = claude_agent_sdk::AgentDefinition {
        description: "Specialist that validates individual task specifications against requirements"
            .to_string(),
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
SUMMARY: [Brief summary]"#
            .to_string(),
        tools: Some(vec!["Read".to_string()]),
        model: Some("sonnet".to_string()),
    };

    // Build task list for suborchestrator
    let task_list: Vec<String> = batch
        .iter()
        .map(|(overview, _)| {
            format!(
                "  - Task {}: {}",
                overview.task.id, overview.task.name
            )
        })
        .collect();

    let task_summary = task_list.join("\n");

    // System prompt for suborchestrator
    let system_prompt = format!(
        r#"You are a review suborchestrator coordinating Step 3: Review & Validation.

## YOUR ROLE
Coordinate the @reviewer agent to validate all {} tasks in your batch.

## STEP 3 WORKFLOW (Review & Validation)
This is the final validation step in the multi-agent task planning workflow:
1. Each task has both an overview (tasks_overview.yaml) and detailed spec (tasks.yaml)
2. Your job is to validate that detailed specs match their overviews and align with IMPL.md
3. You coordinate @reviewer agents in parallel for efficiency
4. You collect and synthesize all review results into a JSON report

## AVAILABLE CONTEXT
You have access to:
- Implementation requirements (IMPL.md)
- Task overview structure (tasks_overview.yaml)
- Task template structure (task_template.yaml)
- Individual task details (provided when you invoke @reviewer)

## YOUR AGENT
**@reviewer** - Validates individual task specifications
- Input: Task overview + detailed spec + IMPL.md context
- Output: ASSESSMENT, ISSUES, SUMMARY

## WORKFLOW
1. For each task in your batch, invoke @reviewer agent with:
   - The task's overview YAML (from tasks_overview.yaml)
   - The task's detailed specification YAML (from tasks.yaml)
   - Reference to IMPL.md for requirements context
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

## CONTEXT FOR STEP 3 WORKFLOW

### Implementation Requirements (IMPL.md):
```
{}
```

### Tasks Overview Structure (tasks_overview.yaml):
```yaml
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
1. Extract the task's overview from tasks_overview.yaml (you have it above)
2. Extract the task's detailed spec from tasks.yaml (use Read tool if needed)
3. Invoke @reviewer with both the overview and detailed spec
4. Parse the reviewer's response

Run ALL @reviewer agents in PARALLEL, then combine results into JSON array.

IMPORTANT: Each @reviewer needs the specific task's overview and detailed YAML - delegate the task details to them, don't try to process everything yourself."#,
        batch.len(),
        impl_md,
        tasks_overview_yaml,
        task_template,
        task_summary
    );

    let mut agents = HashMap::new();
    agents.insert("reviewer".to_string(), reviewer_agent);

    let options = claude_agent_sdk::ClaudeAgentOptions {
        allowed_tools: vec!["Read".to_string().into()],
        system_prompt: Some(claude_agent_sdk::SystemPrompt::String(system_prompt)),
        agents: Some(agents),
        permission_mode: Some(claude_agent_sdk::PermissionMode::BypassPermissions),
        include_partial_messages: true,
        ..Default::default()
    };

    // Execute suborchestrator
    let stream = claude_agent_sdk::query(&query_prompt, Some(options))
        .await
        .context("Failed to query Claude agent for review")?;

    let mut response_parts = Vec::new();
    let mut usage_stats = None;

    futures::pin_mut!(stream);

    while let Some(result) = stream.next().await {
        let message = result.context("Failed to receive message from stream")?;

        match message {
            claude_agent_sdk::Message::Assistant { message, .. } => {
                for block in message.content {
                    if let claude_agent_sdk::ContentBlock::Text { text } = block {
                        response_parts.push(text.clone());

                        if text.contains("@reviewer") {
                            log_delegate_to!("review_suborchestrator", "reviewer");
                        }

                        if debug && !text.is_empty() {
                            log_debug!("  Response: {}", &text[..text.len().min(100)]);
                        }
                    }
                }
            }
            claude_agent_sdk::Message::Result {
                duration_ms,
                num_turns,
                total_cost_usd,
                ..
            } => {
                usage_stats = Some((duration_ms, num_turns, total_cost_usd));
            }
            _ => {}
        }
    }

    let combined_output = response_parts.join("\n");

    if debug {
        log_debug!("[Batch {}] Raw output: {}", batch_num, &combined_output[..combined_output.len().min(200)]);
    }

    // Parse JSON response
    let json_str = if combined_output.contains("```json") {
        combined_output
            .split("```json")
            .nth(1)
            .and_then(|s| s.split("```").next())
            .unwrap_or(&combined_output)
            .trim()
    } else if combined_output.contains("```") {
        combined_output
            .split("```")
            .nth(1)
            .and_then(|s| s.split("```").next())
            .unwrap_or(&combined_output)
            .trim()
    } else {
        combined_output.trim()
    };

    let results: Vec<ReviewResult> =
        serde_json::from_str(json_str).context("Failed to parse review results JSON")?;

    if let Some((duration_ms, num_turns, total_cost_usd)) = usage_stats {
        log_stats!(
            duration_ms,
            num_turns,
            total_cost_usd.unwrap_or(0.0),
            0,
            0
        );
    }

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
    log_phase_start_console!(3, "Batched Review", "Validate expanded tasks with @reviewer agents");

    // Parse both YAML files
    let overview_tasks = parse_tasks_overview(tasks_overview_yaml)
        .context("Failed to parse tasks_overview.yaml")?;
    let detailed_tasks =
        parse_detailed_tasks(tasks_yaml).context("Failed to parse tasks.yaml")?;

    log_info!(
        "Matching {} overview tasks with {} detailed tasks",
        overview_tasks.len(),
        detailed_tasks.len()
    );

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
            log_warning!(
                "No detailed task found for overview task {}",
                overview.task.id
            );
        }
    }

    // Create batches
    let batches: Vec<&[(TaskOverview, DetailedTask)]> = task_pairs.chunks(batch_size).collect();
    log_info!(
        "Created {} batch(es) with batch_size={}",
        batches.len(),
        batch_size
    );

    // Process batches sequentially (each batch processes tasks in parallel internally)
    let mut all_results = Vec::new();
    for (batch_idx, batch) in batches.iter().enumerate() {
        let batch_results = review_suborchestrator(
            batch,
            impl_md,
            tasks_overview_yaml,
            task_template,
            batch_idx + 1,
            batches.len(),
            debug,
        )
        .await?;

        all_results.extend(batch_results);
    }

    Ok(all_results)
}

/// Generate final review report
pub async fn step3_main_orchestrator_report(
    review_results: &[ReviewResult],
    report_path: &Path,
) -> Result<()> {
    log_phase_start_console!(0, "Final Report", "Main Orchestrator Summary");

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
        log_info!("All tasks approved! Ready for implementation.");
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

    std::fs::write(report_path, report).context("Failed to write review report")?;
    log_file_saved!(report_path.display());

    Ok(())
}
