use claude_agent_sdk::{
    query, ClaudeAgentOptions, ContentBlock, Message, SystemPrompt, SystemPromptPreset,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, Serialize, Deserialize)]
struct ResearchPrompt {
    title: String,
    query: String,
    focus: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PromptsData {
    objective: String,
    prompts: Vec<ResearchPrompt>,
}

#[derive(Debug)]
struct ResearchResult {
    title: String,
    query: String,
    response: String,
    #[allow(dead_code)]
    focus: Vec<String>,
}

/// Clean YAML response by removing markdown code blocks, prose, and document separators
fn clean_yaml(text: &str) -> anyhow::Result<String> {
    // Step 1: Extract from markdown code blocks if present (like tasks_agent.rs)
    let mut yaml = if text.contains("```yaml") {
        let start = text.find("```yaml").unwrap() + 7;
        let end = text[start..].find("```").unwrap() + start;
        text[start..end].trim().to_string()
    } else if text.contains("```") {
        let start = text.find("```").unwrap() + 3;
        let end = text[start..].find("```").unwrap() + start;
        text[start..end].trim().to_string()
    } else {
        text.trim().to_string()
    };

    // Step 2: Find where actual YAML starts (look for "objective:")
    // This handles prose text like "Perfect! Now I have all the data..."
    if let Some(obj_pos) = yaml.find("objective:") {
        yaml = yaml[obj_pos..].to_string();
    } else {
        // No "objective:" found - agent likely generated prose instead of YAML
        return Err(anyhow::anyhow!(
            "Could not find 'objective:' in response. The agent appears to have generated explanatory text instead of YAML.\n\nFirst 500 chars of response:\n{}",
            &text.chars().take(500).collect::<String>()
        ));
    }

    // Step 3: Strip YAML document separators (---)
    // These cause "multiple documents" errors in serde_yaml::from_str
    yaml = yaml.replace("---", "");

    // Step 4: Return cleaned YAML
    Ok(yaml.trim().to_string())
}

/// Phase 1: Generate research prompts based on objective
async fn generate_prompts(
    objective: &str,
    prompt_writer: &str,
    output_style: &str,
) -> anyhow::Result<PromptsData> {
    println!("{}", "=".repeat(80));
    println!("PHASE 1: Generating Research Prompts");
    println!("{}", "=".repeat(80));

    // Build system prompt with strong YAML-only instructions
    let system_prompt = format!(
        "{}\n\n# Output Style\n{}\n\n\
        CRITICAL INSTRUCTIONS:\n\
        - Output ONLY valid YAML\n\
        - NO markdown code blocks (no ```yaml or ```)\n\
        - NO commentary or explanations\n\
        - NO prose text before or after the YAML\n\
        - Start directly with 'objective:'\n\
        - Use proper YAML indentation",
        prompt_writer, output_style
    );

    // Create options for prompt generation
    let options = ClaudeAgentOptions::builder()
        .system_prompt(system_prompt)
        .allowed_tools(vec![
            "Read".to_string(),
            "Glob".to_string(),
            "Grep".to_string(),
        ])
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    // Send query with reinforced instructions
    let query_text = format!(
        "Generate research prompts for: {}\n\n\
        IMPORTANT: Output ONLY the YAML structure, no markdown code blocks, no extra commentary.\n\
        Start your response directly with:\n\
        objective: <the objective text>\n\
        prompts:\n  - ...",
        objective
    );

    let stream = query(&query_text, Some(options)).await?;
    let mut stream = Box::pin(stream);

    let mut response_text = String::new();

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    if let ContentBlock::Text { text } = block {
                        print!("{}", text);
                        response_text.push_str(text);
                    }
                }
            }
            Message::Result { .. } => break,
            _ => {}
        }
    }

    println!("\n");

    // Clean and parse YAML response
    let yaml_content = clean_yaml(&response_text)?;

    // Debug: Save cleaned YAML to file for inspection
    let debug_path = format!("debug_cleaned_yaml_{}.yaml", chrono::Local::now().format("%Y%m%d_%H%M%S"));
    if let Err(e) = fs::write(&debug_path, &yaml_content).await {
        eprintln!("Warning: Could not write debug YAML: {}", e);
    } else {
        println!("Debug: Cleaned YAML saved to {}", debug_path);
    }

    let prompts_data: PromptsData = serde_yaml::from_str(&yaml_content)
        .map_err(|e| anyhow::anyhow!("Failed to parse YAML: {}\n\nCleaned YAML saved to: {}\n\nError: {}", e, debug_path, yaml_content))?;

    Ok(prompts_data)
}

/// Phase 2: Execute a single research prompt
async fn execute_research_prompt(prompt: &ResearchPrompt) -> anyhow::Result<ResearchResult> {
    println!("\n{}", "-".repeat(80));
    println!("EXECUTING: {}", prompt.title);
    println!("{}", "-".repeat(80));

    // Create options with claude_code preset (gives access to tools)
    let preset = SystemPromptPreset {
        prompt_type: "preset".to_string(),
        preset: "claude_code".to_string(),
        append: Some("IMPORTANT: DO NOT create or write any files. Output all your research findings as text only.".to_string()),
    };

    let options = ClaudeAgentOptions::builder()
        .system_prompt(SystemPrompt::Preset(preset))
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    // Execute research query
    let stream = query(&prompt.query, Some(options)).await?;
    let mut stream = Box::pin(stream);

    let mut response_text = String::new();

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    if let ContentBlock::Text { text } = block {
                        print!("{}", text);
                        response_text.push_str(text);
                    }
                }
            }
            Message::Result { .. } => break,
            _ => {}
        }
    }

    println!("\n");

    Ok(ResearchResult {
        title: prompt.title.clone(),
        query: prompt.query.clone(),
        response: response_text,
        focus: prompt.focus.clone(),
    })
}

/// Phase 3: Synthesize all research into comprehensive documentation
async fn synthesize_documentation(
    objective: &str,
    research_results: &[ResearchResult],
    output_path: &Path,
) -> anyhow::Result<()> {
    println!("\n{}", "=".repeat(80));
    println!("PHASE 3: Synthesizing Documentation");
    println!("{}", "=".repeat(80));

    // Build context from all research results
    let mut research_context = format!(
        "# Research Objective\n{}\n\n# Research Findings\n\n",
        objective
    );

    for (i, result) in research_results.iter().enumerate() {
        research_context.push_str(&format!(
            "## Finding {}: {}\n\n**Query:** {}\n\n**Response:**\n{}\n\n---\n\n",
            i + 1,
            result.title,
            result.query,
            result.response
        ));
    }

    // Setup synthesis prompt
    let synthesis_prompt = format!(
        r#"Based on the research findings below, create a comprehensive documentation that:

1. Synthesizes all findings into a cohesive narrative
2. Provides clear, actionable insights
3. Includes code examples and technical details where relevant
4. Organizes information logically with proper sections
5. Serves as both user documentation and agent context

{}

Generate a well-structured markdown document and save it to {}"#,
        research_context,
        output_path.display()
    );

    // Create options for synthesis
    let options = ClaudeAgentOptions::builder()
        .system_prompt("You are a technical writer creating comprehensive documentation from research findings.")
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    // Execute synthesis
    let stream = query(&synthesis_prompt, Some(options)).await?;
    let mut stream = Box::pin(stream);

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    if let ContentBlock::Text { text } = block {
                        print!("{}", text);
                    }
                }
            }
            Message::Result { .. } => break,
            _ => {}
        }
    }

    println!("\n");

    Ok(())
}

/// Load file content or return literal string
async fn load_prompt_file(file_path: &str) -> anyhow::Result<String> {
    let path = Path::new(file_path);
    if path.exists() && path.is_file() {
        Ok(fs::read_to_string(path).await?)
    } else {
        Ok(file_path.to_string())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse arguments
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 4 {
        eprintln!(
            "Usage: {} <objective> <prompt_writer_path> <output_style_path> [output_file]",
            args[0]
        );
        std::process::exit(1);
    }

    let objective = &args[1];
    let prompt_writer_path = &args[2];
    let output_style_path = &args[3];
    let output_file = args.get(4).map(|s| PathBuf::from(s)).unwrap_or_else(|| {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        PathBuf::from(format!("research_output_{}.md", timestamp))
    });

    // Load prompts
    let prompt_writer = load_prompt_file(prompt_writer_path).await?;
    let output_style = load_prompt_file(output_style_path).await?;

    // Phase 1: Generate prompts
    let prompts_data = generate_prompts(objective, &prompt_writer, &output_style).await?;

    if prompts_data.prompts.is_empty() {
        println!("No prompts generated. Exiting.");
        return Ok(());
    }

    println!(
        "\nGenerated {} research prompts",
        prompts_data.prompts.len()
    );

    // Phase 2: Execute research prompts sequentially
    let mut research_results = Vec::new();

    for (i, prompt) in prompts_data.prompts.iter().enumerate() {
        println!(
            "\n[{}/{}] Executing research prompt...",
            i + 1,
            prompts_data.prompts.len()
        );
        let result = execute_research_prompt(prompt).await?;
        research_results.push(result);
    }

    // Phase 3: Synthesize documentation
    synthesize_documentation(objective, &research_results, &output_file).await?;

    println!("\n{}", "=".repeat(80));
    println!(
        "Research complete! Documentation saved to: {}",
        output_file.display()
    );
    println!("{}", "=".repeat(80));

    Ok(())
}
