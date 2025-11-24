//! Phase 5: Documentation synthesis
//!
//! Synthesizes all research findings into comprehensive, well-structured documentation.
//!
//! This phase:
//! - Reads the research results summary from Phase 3/4
//! - For each result, reads the detailed YAML file
//! - Uses a file-condenser subagent for large files (>30k chars)
//! - Combines all findings into cohesive markdown documentation
//! - Writes the final output to the specified path
//!
//! The synthesized documentation includes code examples, technical details, and actionable insights.

use anyhow::Result;
use std::path::Path;

use crate::workflow_utils::{execute_agent, AgentConfig};
use claude_agent_sdk::{AgentDefinition, ClaudeAgentOptions, SystemPrompt, SystemPromptPreset};

/// Phase 5: Synthesize documentation from research results
pub async fn synthesize_documentation(
    results_file: &Path,
    output_path: &Path,
) -> Result<()> {
    println!("\n{}", "=".repeat(80));
    println!("PHASE 4: Documentation Synthesis");
    println!("{}", "=".repeat(80));
    println!("results file path: {}", results_file.display());

    let task_id = "synthesize";
    let agent_name = "Documentation Agent";

    // Create unified synthesis prompt - agent decides strategy
    let synthesis_prompt = format!(
        r#"Read the research results overview file at {} and create comprehensive documentation.

The overview file contains a list of research findings with:
- title: The topic researched
- query: The research question
- response_file: Path to the YAML file containing the detailed findings
- focus: Key areas covered

# Your Task:

1. **Analyze the overview**: Read the overview file to understand all research findings

2. **Assess file sizes**: For each research finding, read the response_file and count the characters/lines

3. **Decide processing strategy**:
   - For files with **< 30,000 characters**: Read and process them directly
   - For files with **≥ 30,000 characters**: Use the Task tool to invoke the 'file-condenser' agent
     - Pass a message like "Condense this file: path/to/file.yaml"
     - The file-condenser will read the file and return a condensed summary (~5-10k chars)
     - Use the condensed version in your documentation

4. **Synthesize documentation**:
   - Combine all content (direct reads + condensed summaries) into cohesive, well-structured markdown
   - Include code examples, technical details, and actionable insights
   - Organize logically with proper headings and sections
   - Maintain technical accuracy while ensuring readability

5. **Write output**: Write the final documentation to {}

# Available Subagent:
- **file-condenser**: Condenses large research result files while preserving key technical details

# Strategy Tips:
- You can process multiple files with the file-condenser in parallel if needed
- Small files can be read directly for maximum detail
- Large files should be condensed to manage context efficiently
- Your goal is comprehensive yet manageable documentation"#,
        results_file.display(),
        output_path.display()
    );

    let preset = SystemPromptPreset {
        prompt_type: "preset".to_string(),
        preset: "claude_code".to_string(),
        append: Some(
            "You are a technical writer creating comprehensive documentation from research findings. You can intelligently decide when to condense large files versus read smaller files directly."
                .to_string(),
        ),
    };

    // Build options with file-condenser subagent
    let options = ClaudeAgentOptions::builder()
        .system_prompt(SystemPrompt::Preset(preset))
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .allowed_tools(vec![
            "Read".to_string(),
            "Write".to_string(),
            "Task".to_string(),
        ])
        .add_agent(
            "file-condenser",
            AgentDefinition {
                description: "Condenses a single research result file while preserving key technical details, code examples, and actionable insights".to_string(),
                prompt: "You are a technical documentation condenser. Read the provided research result YAML file and create a condensed summary that:\n\n1. Preserves all key technical details and insights\n2. Includes important code examples (condensed if very long)\n3. Maintains actionable recommendations\n4. Reduces verbosity and redundancy\n5. Target output: 5,000-10,000 characters\n\nReturn ONLY the condensed markdown content. Do not write to any files.".to_string(),
                tools: Some(vec!["Read".to_string()]),
                model: Some("sonnet".to_string()),
            },
        )
        .build();

    // Execute agent (handles all stream processing, logging, sub-agent detection, etc.)
    let config = AgentConfig::new(
        task_id,
        agent_name,
        "Synthesizing documentation with file-condenser subagent",
        synthesis_prompt,
        options,
    );

    execute_agent(config).await?;

    println!("\n{}", "=".repeat(80));
    println!("✓ Documentation synthesis complete");
    println!("{}", "=".repeat(80));

    Ok(())
}
