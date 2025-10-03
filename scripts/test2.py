#!/usr/bin/env -S sh -c 'unset PYTHONPATH && uv run --script "$0" "$@"'
# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "claude_agent_sdk",
#     "rich",
#     "python-dotenv",
#     "elevenlabs",
#     "rpds-py>=0.20.0",
# ]
# ///

import asyncio
import argparse
import json
import os
from pathlib import Path
from dotenv import load_dotenv
from claude_agent_sdk import (
    ClaudeSDKClient,
    ClaudeAgentOptions,
    AssistantMessage,
    TextBlock,
    ThinkingBlock,
    UserMessage,
    ToolUseBlock,
    ToolResultBlock,
    ResultMessage,
    HookMatcher,
    HookContext,
    tool,
    create_sdk_mcp_server,
)
from typing import Any
from rich.console import Console
from rich.text import Text
from rich.panel import Panel

# Load environment variables
load_dotenv()

console = Console()


# Helper function to play TTS
def play_tts(message: str) -> dict[str, Any]:
    """Generate and play TTS using ElevenLabs."""
    try:
        # Get API key from environment
        api_key = os.getenv("ELEVENLABS_API_KEY")
        if not api_key:
            return {
                "success": False,
                "error": "ELEVENLABS_API_KEY not found in environment"
            }

        from elevenlabs.client import ElevenLabs
        from elevenlabs.play import play

        # Initialize client
        client = ElevenLabs(api_key=api_key)

        # Generate and play audio
        audio = client.text_to_speech.convert(
            text=message,
            voice_id="vGQNBgLaiM3EdZtxIiuY",
            model_id="eleven_flash_v2_5",
            output_format="mp3_44100_128",
        )

        play(audio)

        return {
            "success": True,
            "message": f"TTS played: {message}"
        }
    except Exception as e:
        return {
            "success": False,
            "error": str(e)
        }


# Custom TTS Tool
@tool("notify_tts", "Send TTS notification", {"message": str})
async def notify_tts(args: dict[str, Any]) -> dict[str, Any]:
    """Generate and play TTS using ElevenLabs."""
    message = args.get("message", "Notification")
    result = play_tts(message)

    if result["success"]:
        return {
            "content": [{
                "type": "text",
                "text": result["message"]
            }]
        }
    else:
        return {
            "content": [{
                "type": "text",
                "text": f"TTS Error: {result['error']}"
            }],
            "is_error": True
        }


async def validate_bash_command(
    input_data: dict[str, Any],
    tool_use_id: str | None,
    context: HookContext
) -> dict[str, Any]:
    """Validate and potentially block dangerous bash commands."""
    if input_data['tool_name'] == 'Bash':
        command = input_data['tool_input'].get('command', '')
        if 'rm' in command:
            blocked_panel = Panel(
                f"Command: {command}\nReason: rm commands are blocked for safety",
                title="⚠️  Dangerous command blocked",
                border_style="red"
            )
            console.print(blocked_panel)
            return {
                'hookSpecificOutput': {
                    'hookEventName': 'PreToolUse',
                    'permissionDecision': 'deny',
                    'permissionDecisionReason': 'rm commands are blocked for safety'
                }
            }
    return {}


async def log_tool_use(
    input_data: dict[str, Any],
    tool_use_id: str | None,
    context: HookContext
) -> dict[str, Any]:
    """Log all tool usage for auditing."""
    tool_name = input_data.get('tool_name', 'Unknown')
    log_panel = Panel(
        f"Tool: {tool_name}",
        title="📝 Tool logged",
        border_style="dim"
    )
    console.print(log_panel)
    return {}


async def notify_with_tts(
    input_data: dict[str, Any],
    tool_use_id: str | None,
    context: HookContext
) -> dict[str, Any]:
    """Trigger TTS notification for Stop/SubagentStop events."""
    tool_name = input_data.get('tool_name', '')

    tts_panel = Panel(
        f"Playing TTS notification for: {tool_name}",
        title="🔊 TTS Notification",
        border_style="magenta"
    )
    console.print(tts_panel)

    # Trigger TTS with message
    message = f"Task completed: {tool_name}"

    # Call the helper function directly
    result = play_tts(message)

    if not result["success"]:
        error_panel = Panel(
            f"TTS Error: {result['error']}",
            title="TTS Error",
            border_style="red"
        )
        console.print(error_panel)

    return {}


# Create TTS MCP Server
tts_server = create_sdk_mcp_server(
    name="elevenlabs_tts",
    version="1.0.0",
    tools=[notify_tts]
)


def load_mcp_config(selected_servers=None):
    """Read .mcp.json and return mcp_servers dict and allowed_tools list.

    Args:
        selected_servers: List of server names to include, or None for all servers
    """
    mcp_config_path = Path(__file__).parent.parent / ".mcp.json"

    with open(mcp_config_path, "r") as f:
        config = json.load(f)

    mcp_servers = {}
    allowed_tools = []

    for server_name, server_config in config.get("mcpServers", {}).items():
        # Skip if selected_servers is specified and this server is not in the list
        if selected_servers is not None and server_name not in selected_servers:
            continue

        # Add server configuration
        mcp_servers[server_name] = {
            "command": server_config["command"],
            "args": server_config.get("args", []),
            "env": server_config.get("env", {}),
        }

        # Add wildcard pattern to allow all tools from this server
        allowed_tools.append(f"mcp__{server_name}__*")

    return mcp_servers, allowed_tools


async def main():
    parser = argparse.ArgumentParser(description="MCP Config Agent")
    parser.add_argument(
        "--servers",
        nargs="*",
        help="MCP servers to enable (space-separated). Use 'all' for all servers, 'none' for no servers, or specify server names.",
    )
    parser.add_argument(
        "--input",
        "-i",
        type=str,
        help="Initial input text/query to send to Claude",
    )
    parser.add_argument(
        "--files",
        "-f",
        nargs="*",
        help="File paths to include in the context (space-separated)",
    )
    parser.add_argument(
        "--notify",
        action="store_true",
        help="Enable TTS notifications for Stop/SubagentStop events",
    )

    args = parser.parse_args()

    # Determine which servers to load
    selected_servers = None  # Default: all servers
    if args.servers is not None:
        if len(args.servers) == 1 and args.servers[0].lower() == "all":
            selected_servers = None  # Load all
        elif len(args.servers) == 1 and args.servers[0].lower() == "none":
            selected_servers = []  # Load none
        else:
            selected_servers = args.servers  # Load specified servers

    mcp_servers, allowed_tools = load_mcp_config(selected_servers)

    # Build hooks configuration
    hooks_config = {
        'PreToolUse': [
            HookMatcher(matcher='Bash', hooks=[validate_bash_command]),
            HookMatcher(hooks=[log_tool_use])  # Applies to all tools
        ],
        'PostToolUse': [
            HookMatcher(hooks=[log_tool_use])
        ]
    }

    # Add TTS notification hooks if --notify flag is enabled
    if args.notify:
        mcp_servers["tts"] = tts_server
        allowed_tools.append("mcp__tts__notify_tts")
        hooks_config['Stop'] = [HookMatcher(hooks=[notify_with_tts])]
        hooks_config['SubagentStop'] = [HookMatcher(hooks=[notify_with_tts])]

    options = ClaudeAgentOptions(
        mcp_servers=mcp_servers,
        allowed_tools=allowed_tools,
        permission_mode="bypassPermissions",
        hooks=hooks_config
    )

    # Print title
    title_text = Text("MCP Config Agent", style="bold magenta")
    title_panel = Panel(title_text, border_style="magenta")
    console.print(title_panel)

    # Show which servers are enabled
    if mcp_servers:
        enabled_servers = ", ".join(mcp_servers.keys())
        servers_panel = Panel(
            enabled_servers, title="Enabled MCP Servers", border_style="cyan"
        )
        console.print(servers_panel)
    else:
        servers_panel = Panel(
            "No MCP servers enabled", title="Enabled MCP Servers", border_style="yellow"
        )
        console.print(servers_panel)

    async with ClaudeSDKClient(options=options) as client:
        # Build initial input text
        input_parts = []

        # Add file contents if provided
        if args.files:
            for file_path in args.files:
                try:
                    with open(file_path, "r") as f:
                        content = f.read()
                        input_parts.append(f"File: {file_path}\n```\n{content}\n```\n")
                except FileNotFoundError:
                    console.print(f"[red]Warning: File not found: {file_path}[/red]")
                except Exception as e:
                    console.print(f"[red]Error reading {file_path}: {e}[/red]")

        # Add user input text
        if args.input:
            input_parts.append(args.input)
        elif not args.files:
            # Only use default if no input and no files
            input_parts.append("What are the available mcp tools?")

        input_text = "\n\n".join(input_parts) if input_parts else "What are the available mcp tools?"

        while True:
            await client.query(input_text)

            # Extract and print response with thinking process
            response_text = []
            async for msg in client.receive_messages():
                if isinstance(msg, AssistantMessage):
                    for block in msg.content:
                        if isinstance(block, TextBlock):
                            response_text.append(block.text)
                        elif isinstance(block, ThinkingBlock):
                            thinking_panel = Panel(
                                block.thinking,
                                title="💭 Thinking",
                                border_style="blue",
                            )
                            console.print(thinking_panel)
                        elif isinstance(block, ToolUseBlock):
                            tool_name = block.name
                            tool_panel = Panel(
                                f"Tool: {tool_name}\nInput: {block.input}",
                                title="🔧 Using Tool",
                                border_style="yellow",
                            )
                            console.print(tool_panel)

                elif isinstance(msg, UserMessage):
                    for block in msg.content:
                        if isinstance(block, ToolResultBlock):
                            result_preview = (
                                str(block.content)[:200] + "..."
                                if len(str(block.content)) > 200
                                else str(block.content)
                            )
                            result_panel = Panel(
                                result_preview,
                                title=f"✓ Tool Result (id: {block.tool_use_id})",
                                border_style="cyan",
                            )
                            console.print(result_panel)

                elif isinstance(msg, ResultMessage):
                    # Print accumulated response text
                    if response_text:
                        full_response = "\n".join(response_text)
                        response_panel = Panel(
                            full_response, title="Agent Response", border_style="green"
                        )
                        console.print(response_panel)
                    break

            # Prompt for next query
            console.print()
            console.print("─" * 60)
            input_text = console.input(
                "[bold blue]➤ Enter your query (or 'exit' to quit):[/bold blue]\n"
            )
            console.print("─" * 60)
            console.print("")

            if input_text.lower() in ["exit", "quit", "q"]:
                console.print("\n[magenta]Goodbye![/magenta]")
                break


if __name__ == "__main__":
    asyncio.run(main())
