# TTS Notification Hook Implementation Plan

## Overview
Create a hook system that uses ElevenLabs TTS to notify the user when specific tool events occur (Stop or SubagentStop).

## Components Needed

### 1. Custom MCP Tool for TTS
- **Tool Name**: `notify_tts`
- **Purpose**: Trigger ElevenLabs TTS with a message
- **Input Schema**: `{"message": str}`
- **Implementation**:
  - Use `@tool` decorator from `claude_agent_sdk`
  - Integrate existing ElevenLabs TTS code
  - Load API key from environment variables
  - Generate and play audio using `elevenlabs.text_to_speech.convert()`

### 2. Hook Function: `notify_with_tts`
- **Hook Type**: `PostToolUse` (fires after tool execution)
- **Trigger Conditions**:
  - Tool name matches "Stop" OR
  - Tool name matches "SubagentStop"
- **Behavior**:
  - Extract tool name from `input_data`
  - Generate TTS message: "Task completed: {tool_name}"
  - Call the custom `notify_tts` tool
  - Display Panel showing TTS notification was triggered

### 3. Integration Steps

#### Step 1: Add Dependencies
```python
# Add to imports
from dotenv import load_dotenv
import os
from elevenlabs.client import ElevenLabs
from elevenlabs import play
```

#### Step 2: Create Custom TTS Tool
```python
@tool("notify_tts", "Send TTS notification", {"message": str})
async def notify_tts(args: dict[str, Any]) -> dict[str, Any]:
    # Load API key
    # Initialize ElevenLabs client
    # Generate and play audio
    # Return success/error response
```

#### Step 3: Create MCP Server
```python
tts_server = create_sdk_mcp_server(
    name="elevenlabs_tts",
    version="1.0.0",
    tools=[notify_tts]
)
```

#### Step 4: Create Hook Function
```python
async def notify_with_tts(
    input_data: dict[str, Any],
    tool_use_id: str | None,
    context: HookContext
) -> dict[str, Any]:
    tool_name = input_data.get('tool_name', '')

    # Display panel
    # Trigger TTS via custom tool
    # Return hook output

    return {}
```

#### Step 5: Update ClaudeAgentOptions
```python
options = ClaudeAgentOptions(
    mcp_servers={
        **mcp_servers,  # Existing MCP servers from .mcp.json
        "tts": tts_server  # Add TTS server
    },
    allowed_tools=[
        *allowed_tools,  # Existing allowed tools
        "mcp__tts__notify_tts"  # Add TTS tool
    ],
    hooks={
        'PreToolUse': [
            HookMatcher(matcher='Bash', hooks=[validate_bash_command]),
            HookMatcher(hooks=[log_tool_use])
        ],
        'PostToolUse': [
            HookMatcher(hooks=[log_tool_use])
        ],
        'Stop': [
            HookMatcher(hooks=[notify_with_tts])
        ],
        'SubagentStop': [
            HookMatcher(hooks=[notify_with_tts])
        ]
    },
    permission_mode="bypassPermissions"
)
```

#### Step 6: Update Script Dependencies
Add to `uv` script header:
```python
# dependencies = [
#     "claude_agent_sdk",
#     "rich",
#     "python-dotenv",
#     "elevenlabs",
# ]
```

## Configuration Requirements

### Environment Variables
- `ELEVENLABS_API_KEY`: Required for TTS functionality
- Should be loaded from `.env` file in project root

### Voice Configuration
- **Voice ID**: `vGQNBgLaiM3EdZtxIiuY` (default from example)
- **Model**: `eleven_flash_v2_5` (Turbo v2.5)
- **Output Format**: `mp3_44100_128`

## Error Handling

1. **Missing API Key**: Gracefully fail with panel notification
2. **TTS Generation Error**: Catch and display error, don't block hook
3. **Network Issues**: Timeout and continue without notification

## Testing Plan

1. Test with Stop tool trigger
2. Test with SubagentStop tool trigger
3. Test with missing API key (graceful degradation)
4. Test with network failure
5. Test that other tools don't trigger TTS

## Additional Considerations

- **Performance**: TTS generation should not block main workflow
- **User Experience**: Panel should show TTS status (generating, playing, complete)
- **Customization**: Consider adding command-line flag to enable/disable TTS
- **Message Content**: Allow customization of TTS message format
