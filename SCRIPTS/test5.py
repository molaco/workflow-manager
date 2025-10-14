#!/usr/bin/env -S sh -c 'unset PYTHONPATH && uv run --script "$0" "$@"'
# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "claude_agent_sdk",
#     "python-dotenv",
#     "elevenlabs",
#     "rpds-py>=0.20.0",
#     "pydub",
#     "audioop-lts",
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
    ResultMessage,
    HookMatcher,
    HookContext,
    tool,
    create_sdk_mcp_server,
)
from typing import Any

# Load environment variables
load_dotenv()


# Helper function to play TTS
def play_tts(message: str, volume_reduction_db: float = 10.0) -> dict[str, Any]:
    """Generate and play TTS using ElevenLabs with volume control.

    Args:
        message: Text to convert to speech
        volume_reduction_db: Volume reduction in decibels (positive = quieter, negative = louder)
    """
    try:
        import io
        from pydub import AudioSegment

        # Get API key from environment
        api_key = os.getenv("ELEVENLABS_API_KEY")
        if not api_key:
            return {
                "success": False,
                "error": "ELEVENLABS_API_KEY not found in environment",
            }

        from elevenlabs.client import ElevenLabs
        from elevenlabs.play import play

        # Initialize client
        client = ElevenLabs(api_key=api_key)

        # Generate audio (returns iterator of bytes)
        audio_iterator = client.text_to_speech.convert(
            text=message,
            voice_id="vGQNBgLaiM3EdZtxIiuY",
            model_id="eleven_flash_v2_5",
            output_format="mp3_44100_128",
        )

        # Collect all audio bytes
        audio_bytes = b"".join(audio_iterator)

        # Convert to AudioSegment for volume control
        audio_segment = AudioSegment.from_file(io.BytesIO(audio_bytes), format="mp3")

        # Apply volume reduction
        quieter_audio = audio_segment - volume_reduction_db

        # Export back to bytes for playback
        output_io = io.BytesIO()
        quieter_audio.export(output_io, format="mp3")
        output_io.seek(0)

        # Play modified audio
        play(output_io)

        return {"success": True, "message": f"TTS played: {message} (volume reduced by {volume_reduction_db}dB)"}
    except Exception as e:
        return {"success": False, "error": str(e)}


# Custom TTS Tool
@tool("notify_tts", "Send TTS notification", {"message": str})
async def notify_tts(args: dict[str, Any]) -> dict[str, Any]:
    """Generate and play TTS using ElevenLabs."""
    message = args.get("message", "Notification")
    result = play_tts(message)

    if result["success"]:
        return {"content": [{"type": "text", "text": result["message"]}]}
    else:
        return {
            "content": [{"type": "text", "text": f"TTS Error: {result['error']}"}],
            "is_error": True,
        }


# Create TTS MCP Server
tts_server = create_sdk_mcp_server(
    name="elevenlabs_tts", version="1.0.0", tools=[notify_tts]
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


async def validate_bash_command(
    input_data: dict[str, Any], tool_use_id: str | None, context: HookContext
) -> dict[str, Any]:
    """Validate and potentially block dangerous bash commands."""
    if input_data["tool_name"] == "Bash":
        command = input_data["tool_input"].get("command", "")
        if "rm" in command:
            print(f"⚠️  Dangerous command blocked: {command}")
            print(f"Reason: rm commands are blocked for safety")
            return {
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "deny",
                    "permissionDecisionReason": "rm commands are blocked for safety",
                }
            }
    return {}


async def block_tools(
    input_data: dict[str, Any], tool_use_id: str | None, context: HookContext
) -> dict[str, Any]:
    """Block specified tools."""
    tool_name = input_data.get("tool_name", "unknown")
    # print(f"⚠️  Tool blocked: {tool_name}")
    # print(f"Reason: Tool is in disallowed list")

    return {
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": f"Tool {tool_name} is disallowed",
        }
    }


def create_notify_with_tts_hook(volume_reduction_db: float):
    """Create a notify_with_tts hook with specified volume reduction."""
    async def notify_with_tts(
        input_data: dict[str, Any], tool_use_id: str | None, context: HookContext
    ) -> dict[str, Any]:
        """Trigger TTS notification for Stop/SubagentStop events."""
        tool_name = input_data.get("tool_name", "")

        # print(f"🔊 TTS Notification: Playing TTS notification for {tool_name}")

        # Trigger TTS with message
        message = f"Task completed: {tool_name}"

        # Call the helper function directly with volume control
        result = play_tts(message, volume_reduction_db)

        if not result["success"]:
            print(f"TTS Error: {result['error']}")

        return {}

    return notify_with_tts


async def main():
    parser = argparse.ArgumentParser(description="Simple MCP Config Agent")
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
        "--system-prompt",
        "-s",
        type=str,
        help="Custom system prompt to guide Claude's behavior (file path or string)",
    )
    parser.add_argument(
        "--append",
        "-a",
        type=str,
        help="Append additional instructions to system prompt (file path or string)",
    )
    parser.add_argument(
        "--disallowedTools",
        nargs="*",
        help="Tools to disallow (space-separated). Examples: Bash, Write, Read. Use 'all' to disallow all tools.",
    )
    parser.add_argument(
        "--notify",
        action="store_true",
        help="Enable TTS notifications for Stop/SubagentStop events",
    )
    parser.add_argument(
        "--tts-volume",
        type=float,
        default=10.0,
        help="TTS volume reduction in decibels (default: 10.0, positive = quieter, negative = louder)",
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

    # Define all available tools
    all_tools = [
        "Task",
        "Bash",
        "Glob",
        "Grep",
        "Read",
        "Edit",
        "Write",
        "NotebookEdit",
        "WebFetch",
        "WebSearch",
        "TodoWrite",
        "BashOutput",
        "KillShell",
        "SlashCommand",
        "ExitPlanMode",
    ]

    # Build hooks configuration
    hooks_config = {
        "PreToolUse": [
            HookMatcher(matcher="Bash", hooks=[validate_bash_command]),
        ],
    }

    # Add disallowed tools hook if specified
    if args.disallowedTools:
        if len(args.disallowedTools) == 1 and args.disallowedTools[0].lower() == "all":
            tool_list = "|".join(all_tools)
        else:
            tool_list = "|".join(args.disallowedTools)

        hooks_config["PreToolUse"].append(
            HookMatcher(matcher=tool_list, hooks=[block_tools])
        )

    # Add TTS notification hooks if --notify flag is enabled
    if args.notify:
        mcp_servers["tts"] = tts_server
        allowed_tools.append("mcp__tts__notify_tts")
        # Create hook with volume control
        notify_hook = create_notify_with_tts_hook(args.tts_volume)
        hooks_config["Stop"] = [HookMatcher(hooks=[notify_hook])]
        hooks_config["SubagentStop"] = [HookMatcher(hooks=[notify_hook])]

    # Build options dict
    options_dict = {
        "mcp_servers": mcp_servers,
        "allowed_tools": allowed_tools,
        "permission_mode": "bypassPermissions",
        "hooks": hooks_config,
    }

    # Check if system prompt is provided
    if not args.system_prompt:
        print("Error: --system-prompt is required")
        return

    # Check if append is used without system prompt
    if args.append and not args.system_prompt:
        print("Error: --append requires --system-prompt to be set")
        return

    if args.system_prompt:
        # Check for special "default" keyword
        if args.system_prompt.lower() == "default":
            if args.append:
                # Check if append is a file path
                append_path = Path(args.append)
                if append_path.exists() and append_path.is_file():
                    try:
                        with open(append_path, "r") as f:
                            append_content = f.read()
                    except Exception as e:
                        print(f"Error reading append file: {e}")
                        append_content = args.append
                else:
                    # Use as literal string
                    append_content = args.append

                options_dict["system_prompt"] = {
                    "type": "preset",
                    "preset": "claude_code",
                    "append": append_content,
                }
            else:
                options_dict["system_prompt"] = {
                    "type": "preset",
                    "preset": "claude_code",
                }
        else:
            # Check if it's a file path
            system_prompt_path = Path(args.system_prompt)
            if system_prompt_path.exists() and system_prompt_path.is_file():
                try:
                    with open(system_prompt_path, "r") as f:
                        system_prompt_content = f.read()
                except Exception as e:
                    print(f"Error reading system prompt file: {e}")
                    system_prompt_content = args.system_prompt
            else:
                # Use as literal string
                system_prompt_content = args.system_prompt

            # Check if we have append content
            if args.append:
                # Check if append is a file path
                append_path = Path(args.append)
                if append_path.exists() and append_path.is_file():
                    try:
                        with open(append_path, "r") as f:
                            append_content = f.read()
                    except Exception as e:
                        print(f"Error reading append file: {e}")
                        append_content = args.append
                else:
                    # Use as literal string
                    append_content = args.append

                # Combine system prompt + append
                options_dict["system_prompt"] = (
                    f"{system_prompt_content}\n\n{append_content}"
                )
            else:
                # Use simple string format
                options_dict["system_prompt"] = system_prompt_content

    options = ClaudeAgentOptions(**options_dict)

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
                    print(f"Warning: File not found: {file_path}")
                except Exception as e:
                    print(f"Error reading {file_path}: {e}")

        # Add user input text
        if args.input:
            input_parts.append(args.input)

        # Check if we have any input
        if not input_parts:
            print("Error: --input or --files must be provided")
            return

        input_text = "\n\n".join(input_parts)

        await client.query(input_text)

        # Extract and print final response
        response_text = []
        async for msg in client.receive_messages():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        response_text.append(block.text)
            elif isinstance(msg, ResultMessage):
                break

        # Print final response
        if response_text:
            print("\n".join(response_text))


if __name__ == "__main__":
    asyncio.run(main())
