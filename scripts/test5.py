#!/usr/bin/env -S sh -c 'unset PYTHONPATH && uv run --script "$0" "$@"'
# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "claude_agent_sdk",
#     "python-dotenv",
# ]
# ///

import asyncio
import argparse
import json
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
)
from typing import Any

# Load environment variables
load_dotenv()


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
    input_data: dict[str, Any],
    tool_use_id: str | None,
    context: HookContext
) -> dict[str, Any]:
    """Validate and potentially block dangerous bash commands."""
    if input_data['tool_name'] == 'Bash':
        command = input_data['tool_input'].get('command', '')
        if 'rm' in command:
            print(f"⚠️  Dangerous command blocked: {command}")
            print(f"Reason: rm commands are blocked for safety")
            return {
                'hookSpecificOutput': {
                    'hookEventName': 'PreToolUse',
                    'permissionDecision': 'deny',
                    'permissionDecisionReason': 'rm commands are blocked for safety'
                }
            }
    return {}


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
        ],
    }

    # Build options dict
    options_dict = {
        "mcp_servers": mcp_servers,
        "allowed_tools": allowed_tools,
        "permission_mode": "bypassPermissions",
        "hooks": hooks_config,
    }

    # Add system prompt if provided
    if args.append and not args.system_prompt:
        print("Error: --append requires --system-prompt to be set")
        return

    if args.system_prompt:
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
            options_dict["system_prompt"] = f"{system_prompt_content}\n\n{append_content}"
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
        elif not args.files:
            # Only use default if no input and no files
            input_parts.append("What are the available mcp tools?")

        input_text = "\n\n".join(input_parts) if input_parts else "What are the available mcp tools?"

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
