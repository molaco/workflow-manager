//! Interactive MCP Agent with Hooks and Custom Tools
//!
//! Rust port of test2.py with:
//! - Custom TTS tool (ElevenLabs)
//! - Bash command validation hooks
//! - Tool usage logging hooks
//! - TTS notification hooks
//! - MCP config loading from .mcp.json
//!
//! Run with: cargo run --example test2 -- --help

use clap::Parser;
use claude_agent_sdk::{
    hooks::HookMatcherBuilder,
    mcp::{SdkMcpServer, SdkMcpTool, ToolResult},
    types::{ClaudeAgentOptions, ContentBlock, HookDecision, HookEvent, HookOutput, Message},
    ClaudeSDKClient,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;

// ============================================================================
// CLI Arguments
// ============================================================================

#[derive(Parser, Debug)]
#[command(author, version, about = "MCP Config Agent with Hooks", long_about = None)]
struct Args {
    /// MCP servers to enable (space-separated). Use 'all' for all servers, 'none' for no servers
    #[arg(long)]
    servers: Option<Vec<String>>,

    /// Initial input text/query to send to Claude
    #[arg(short, long)]
    input: Option<String>,

    /// File paths to include in the context (space-separated)
    #[arg(short, long)]
    files: Option<Vec<PathBuf>>,

    /// Enable TTS notifications for Stop/SubagentStop events
    #[arg(long)]
    notify: bool,
}

// ============================================================================
// MCP Config Loading
// ============================================================================

#[derive(Debug, Deserialize)]
struct McpConfig {
    #[serde(rename = "mcpServers")]
    mcp_servers: HashMap<String, McpServerConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct McpServerConfig {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

async fn load_mcp_config(
    selected_servers: Option<Vec<String>>,
) -> anyhow::Result<(HashMap<String, McpServerConfig>, Vec<String>)> {
    // Find .mcp.json in parent directory
    let mcp_config_path = Path::new("./.mcp.json");

    let content = fs::read_to_string(mcp_config_path).await.map_err(|e| {
        anyhow::anyhow!(
            "Failed to read MCP config at {}: {}",
            mcp_config_path.display(),
            e
        )
    })?;
    let config: McpConfig = serde_json::from_str(&content)?;

    let mut mcp_servers = HashMap::new();
    let mut allowed_tools = Vec::new();

    for (server_name, server_config) in config.mcp_servers {
        // Skip if selected_servers is specified and this server is not in the list
        if let Some(ref selected) = selected_servers {
            if !selected.contains(&server_name) {
                continue;
            }
        }

        // Add wildcard pattern to allow all tools from this server
        allowed_tools.push(format!("mcp__{server_name}__*"));
        mcp_servers.insert(server_name, server_config);
    }

    Ok((mcp_servers, allowed_tools))
}

// ============================================================================
// TTS Tool (ElevenLabs)
// ============================================================================

async fn play_tts(message: &str) -> anyhow::Result<String> {
    let api_key = std::env::var("ELEVENLABS_API_KEY")
        .map_err(|_| anyhow::anyhow!("ELEVENLABS_API_KEY not found in environment"))?;

    println!("🔊 Playing TTS: {}", message);

    let client = reqwest::Client::new();

    let response = client
        .post("https://api.elevenlabs.io/v1/text-to-speech/vGQNBgLaiM3EdZtxIiuY")
        .header("xi-api-key", &api_key)
        .json(&json!({
            "text": message,
            "model_id": "eleven_flash_v2_5",
            "voice_settings": {
                "stability": 0.5,
                "similarity_boost": 0.5
            }
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await?;
        return Err(anyhow::anyhow!(
            "ElevenLabs API error {}: {}",
            status,
            error_text
        ));
    }

    // Get audio bytes
    let audio_bytes = response.bytes().await?;

    // Write to temp file
    let temp_path = std::env::temp_dir().join("tts_output.mp3");
    fs::write(&temp_path, audio_bytes).await?;

    // Play audio (platform-specific)
    #[cfg(target_os = "linux")]
    {
        tokio::process::Command::new("mpg123")
            .arg(&temp_path)
            .output()
            .await?;
    }

    #[cfg(target_os = "macos")]
    {
        tokio::process::Command::new("afplay")
            .arg(&temp_path)
            .output()
            .await?;
    }

    #[cfg(target_os = "windows")]
    {
        tokio::process::Command::new("powershell")
            .args(&[
                "-c",
                &format!(
                    "(New-Object Media.SoundPlayer '{}').PlaySync();",
                    temp_path.display()
                ),
            ])
            .output()
            .await?;
    }

    Ok(format!("TTS played: {}", message))
}

fn create_tts_tool() -> SdkMcpTool {
    SdkMcpTool::new(
        "notify_tts",
        "Send TTS notification using ElevenLabs",
        json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "Message to speak via TTS"
                }
            },
            "required": ["message"]
        }),
        |input| {
            Box::pin(async move {
                let message = input["message"].as_str().unwrap_or("Notification");

                match play_tts(message).await {
                    Ok(result) => Ok(ToolResult::text(result)),
                    Err(e) => Ok(ToolResult::error(format!("TTS Error: {}", e))),
                }
            })
        },
    )
}

// ============================================================================
// Hook Functions
// ============================================================================

fn validate_bash_command() -> claude_agent_sdk::types::HookCallback {
    claude_agent_sdk::hooks::HookManager::callback(|event_data, tool_name, _context| async move {
        if let Some(tool) = tool_name.as_deref() {
            if tool == "Bash" {
                if let Some(input) = event_data.get("tool_input") {
                    if let Some(command) = input.get("command") {
                        let cmd = command.as_str().unwrap_or("");
                        if cmd.contains("rm") {
                            eprintln!("╔════════════════════════════════════════════╗");
                            eprintln!("║  ⚠️  DANGEROUS COMMAND BLOCKED             ║");
                            eprintln!("╠════════════════════════════════════════════╣");
                            eprintln!(
                                "║  Command: {:<36} ║",
                                cmd.chars().take(36).collect::<String>()
                            );
                            eprintln!("║  Reason: rm commands are blocked for safety║");
                            eprintln!("╚════════════════════════════════════════════╝");

                            return Ok(HookOutput {
                                decision: Some(HookDecision::Block),
                                system_message: Some(
                                    "rm commands are blocked for safety".to_string(),
                                ),
                                hook_specific_output: Some(json!({
                                    "hookEventName": "PreToolUse",
                                    "permissionDecision": "deny",
                                    "permissionDecisionReason": "rm commands are blocked for safety"
                                })),
                            });
                        }
                    }
                }
            }
        }
        Ok(HookOutput::default())
    })
}

fn log_tool_use() -> claude_agent_sdk::types::HookCallback {
    claude_agent_sdk::hooks::HookManager::callback(|_event_data, tool_name, _context| async move {
        let tool = tool_name.unwrap_or_else(|| "Unknown".to_string());
        println!("┌──────────────────────────────┐");
        println!("│ 📝 Tool logged: {:<13} │", tool);
        println!("└──────────────────────────────┘");
        Ok(HookOutput::default())
    })
}

fn notify_with_tts() -> claude_agent_sdk::types::HookCallback {
    claude_agent_sdk::hooks::HookManager::callback(|_event_data, tool_name, _context| async move {
        let tool = tool_name.unwrap_or_default();
        let message = format!("Task completed: {}", tool);

        println!("┌──────────────────────────────────────────┐");
        println!("│ 🔊 TTS Notification                      │");
        println!("├──────────────────────────────────────────┤");
        println!(
            "│ Playing: {:<32} │",
            message.chars().take(32).collect::<String>()
        );
        println!("└──────────────────────────────────────────┘");

        if let Err(e) = play_tts(&message).await {
            eprintln!("┌──────────────────────────────┐");
            eprintln!("│ ❌ TTS Error                 │");
            eprintln!("├──────────────────────────────┤");
            eprintln!(
                "│ {:<28} │",
                e.to_string().chars().take(28).collect::<String>()
            );
            eprintln!("└──────────────────────────────┘");
        }

        Ok(HookOutput::default())
    })
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present
    if let Ok(path) = std::env::current_dir() {
        let env_path = path.join("../.env");
        if env_path.exists() {
            dotenv::from_path(env_path).ok();
        }
    }

    let args = Args::parse();

    // Determine which servers to load
    let selected_servers = match &args.servers {
        Some(servers) if servers.len() == 1 && servers[0].to_lowercase() == "all" => None,
        Some(servers) if servers.len() == 1 && servers[0].to_lowercase() == "none" => {
            Some(Vec::new())
        }
        Some(servers) => Some(servers.clone()),
        None => None, // Default: all servers
    };

    let (mcp_servers, mut allowed_tools) = load_mcp_config(selected_servers).await?;

    // Build hooks configuration
    let mut hooks: HashMap<HookEvent, Vec<claude_agent_sdk::types::HookMatcher>> = HashMap::new();

    // PreToolUse hooks
    hooks.insert(
        HookEvent::PreToolUse,
        vec![
            HookMatcherBuilder::new(Some("Bash"))
                .add_hook(validate_bash_command())
                .build(),
            HookMatcherBuilder::new(None::<String>)
                .add_hook(log_tool_use())
                .build(),
        ],
    );

    // PostToolUse hooks
    hooks.insert(
        HookEvent::PostToolUse,
        vec![HookMatcherBuilder::new(None::<String>)
            .add_hook(log_tool_use())
            .build()],
    );

    // Add TTS notification hooks if --notify flag is enabled
    let tts_server = if args.notify {
        allowed_tools.push("mcp__tts__notify_tts".to_string());

        let tts_hook = notify_with_tts();

        hooks.insert(
            HookEvent::Stop,
            vec![HookMatcherBuilder::new(None::<String>)
                .add_hook(tts_hook.clone())
                .build()],
        );

        hooks.insert(
            HookEvent::SubagentStop,
            vec![HookMatcherBuilder::new(None::<String>)
                .add_hook(tts_hook)
                .build()],
        );

        Some(
            SdkMcpServer::new("elevenlabs_tts")
                .version("1.0.0")
                .tool(create_tts_tool()),
        )
    } else {
        None
    };

    // Build options
    let options_builder = ClaudeAgentOptions::builder()
        .permission_mode(claude_agent_sdk::types::PermissionMode::BypassPermissions)
        .allowed_tools(allowed_tools)
        .hooks(hooks);

    // Add SDK MCP server if notify enabled
    if let Some(server) = tts_server {
        // Note: This requires add_sdk_mcp_server method
        // For now, we'll store it and handle it separately
        // TODO: Implement once SDK supports add_sdk_mcp_server
        let _ = server; // Placeholder
    }

    let options = options_builder.build();

    // Print title
    println!("╔════════════════════════════════════════╗");
    println!("║      MCP Config Agent (Rust)           ║");
    println!("╚════════════════════════════════════════╝");

    // Show which servers are enabled
    if !mcp_servers.is_empty() {
        println!("\n┌─────────────────────────────────────────┐");
        println!("│ Enabled MCP Servers:                    │");
        println!("├─────────────────────────────────────────┤");
        for server_name in mcp_servers.keys() {
            println!("│ • {:<37} │", server_name);
        }
        println!("└─────────────────────────────────────────┘");
    } else {
        println!("\n┌─────────────────────────────────────────┐");
        println!("│ No MCP servers enabled                  │");
        println!("└─────────────────────────────────────────┘");
    }

    // Create client
    let mut client = ClaudeSDKClient::new(options, None).await?;

    // Build initial input text
    let mut input_parts = Vec::new();

    // Add file contents if provided
    if let Some(files) = &args.files {
        for file_path in files {
            match fs::read_to_string(file_path).await {
                Ok(content) => {
                    input_parts.push(format!(
                        "File: {}\n```\n{}\n```\n",
                        file_path.display(),
                        content
                    ));
                }
                Err(e) => {
                    eprintln!("❌ Error reading {:?}: {}", file_path, e);
                }
            }
        }
    }

    // Add user input text
    if let Some(input) = &args.input {
        input_parts.push(input.clone());
    } else if args.files.is_none() {
        input_parts.push("What are the available mcp tools?".to_string());
    }

    let mut input_text = if input_parts.is_empty() {
        "What are the available mcp tools?".to_string()
    } else {
        input_parts.join("\n\n")
    };

    // Main conversation loop
    loop {
        client.send_message(&input_text).await?;

        let mut response_text = Vec::new();

        // Receive messages
        loop {
            match tokio::time::timeout(Duration::from_secs(60), client.next_message()).await {
                Ok(Some(Ok(msg))) => {
                    match msg {
                        Message::Assistant { message, .. } => {
                            for block in &message.content {
                                match block {
                                    ContentBlock::Text { text } => {
                                        response_text.push(text.clone());
                                    }
                                    ContentBlock::Thinking { thinking, .. } => {
                                        println!("\n┌─────────────────────────────────┐");
                                        println!("│ 💭 Thinking                     │");
                                        println!("├─────────────────────────────────┤");
                                        for line in thinking.lines().take(5) {
                                            println!(
                                                "│ {:<31} │",
                                                line.chars().take(31).collect::<String>()
                                            );
                                        }
                                        println!("└─────────────────────────────────┘");
                                    }
                                    ContentBlock::ToolUse { name, input, .. } => {
                                        println!("\n┌─────────────────────────────────┐");
                                        println!("│ 🔧 Using Tool                   │");
                                        println!("├─────────────────────────────────┤");
                                        println!("│ Tool: {:<25} │", name);
                                        let input_preview = format!("{:?}", input)
                                            .chars()
                                            .take(27)
                                            .collect::<String>();
                                        println!("│ Input: {:<24} │", input_preview);
                                        println!("└─────────────────────────────────┘");
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Message::User { message, .. } => {
                            if let Some(content) = &message.content {
                                match content {
                                    claude_agent_sdk::types::UserContent::Blocks(blocks) => {
                                        for block in blocks {
                                            if let ContentBlock::ToolResult { content, .. } = block
                                            {
                                                let preview = format!("{:?}", content)
                                                    .chars()
                                                    .take(60)
                                                    .collect::<String>();
                                                println!("\n┌────────────────────────────────────────────────────────────┐");
                                                println!("│ ✓ Tool Result                                              │");
                                                println!("├────────────────────────────────────────────────────────────┤");
                                                println!("│ {:<58} │", preview);
                                                println!("└────────────────────────────────────────────────────────────┘");
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Message::Result { .. } => {
                            // Print accumulated response text
                            if !response_text.is_empty() {
                                let full_response = response_text.join("\n");
                                println!("\n╔════════════════════════════════════════════════════════════╗");
                                println!("║ Agent Response                                             ║");
                                println!("╠════════════════════════════════════════════════════════════╣");
                                for line in full_response.lines() {
                                    println!(
                                        "║ {:<58} ║",
                                        line.chars().take(58).collect::<String>()
                                    );
                                }
                                println!("╚════════════════════════════════════════════════════════════╝");
                            }
                            break;
                        }
                        _ => {}
                    }
                }
                Ok(Some(Err(e))) => {
                    eprintln!("Error: {}", e);
                    break;
                }
                Ok(None) => break,
                Err(_) => {
                    eprintln!("Timeout waiting for response");
                    break;
                }
            }
        }

        // Prompt for next query
        println!();
        println!("────────────────────────────────────────────────────────────");
        println!("➤ Enter your query (or 'exit' to quit):");
        println!("────────────────────────────────────────────────────────────");

        let mut buffer = String::new();
        std::io::stdin().read_line(&mut buffer)?;
        input_text = buffer.trim().to_string();

        if input_text.to_lowercase() == "exit"
            || input_text.to_lowercase() == "quit"
            || input_text.to_lowercase() == "q"
        {
            println!("\nGoodbye! 👋");
            break;
        }
    }

    client.close().await?;

    Ok(())
}
