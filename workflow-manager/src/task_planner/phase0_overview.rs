//! Phase 0: Generate task overview from IMPL.md
//!
//! This phase uses a main orchestrator agent to:
//! - Analyze implementation requirements (IMPL.md)
//! - Generate high-level task breakdown (tasks_overview.yaml)
//! - Focus on strategic objectives (WHAT and WHY, not HOW)
//! - Identify task dependencies

use crate::workflow_utils::{execute_agent, extract_yaml, AgentConfig};
use anyhow::Result;
use claude_agent_sdk::ClaudeAgentOptions;

/// Generate tasks_overview.yaml from IMPL.md and template
pub async fn generate_overview(impl_md: &str, overview_template: &str) -> Result<String> {
    println!("{}", "=".repeat(80));
    println!("PHASE 0: Main Orchestrator - Generate Task Overview");
    println!("{}", "=".repeat(80));

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

Make sure to just give your response. You must not create or write any files just output the yaml and only that."#,
        impl_md, overview_template
    );

    let options = ClaudeAgentOptions::builder()
        .system_prompt(system_prompt.to_string())
        .allowed_tools(vec![
            "Read".to_string(),
            "Grep".to_string(),
            "Glob".to_string(),
        ])
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    let config = AgentConfig::new(
        "generate_overview",
        "Task Overview Generator",
        "Generating task overview from IMPL.md",
        prompt,
        options,
    );

    let response = execute_agent(config).await?;

    // Extract YAML from response
    let yaml_content = extract_yaml(&response);

    println!("\n✓ Task overview generation complete\n");
    Ok(yaml_content)
}
