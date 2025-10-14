# Claude Agent SDK Python - Architecture Research

This document provides a comprehensive analysis of the Claude Agent SDK Python implementation, covering the complete communication flow between the SDK client and Claude Code CLI subprocess.

## Table of Contents

1. [CLI Subprocess Initialization Flow](#cli-subprocess-initialization-flow)
2. [Control Protocol Handshake](#control-protocol-handshake)
3. [SDK MCP Server Registration](#sdk-mcp-server-registration)
4. [MCP Tool Communication Protocol](#mcp-tool-communication-protocol)
5. [Message Streaming and Parsing](#message-streaming-and-parsing)
6. [Tool Permission Control Flow](#tool-permission-control-flow)
7. [Streaming vs String Mode Differences](#streaming-vs-string-mode-differences)

---

## CLI Subprocess Initialization Flow

### Overview
Traces the complete initialization sequence from SDK client instantiation through Claude Code CLI subprocess startup.

### Key Components

**Client Initialization** (`src/claude_agent_sdk/_internal/client.py`)
```python
def __init__(self, options: ClaudeAgentOptions | None = None, transport: Transport | None = None):
    if options is None:
        options = ClaudeAgentOptions()
    self.options = options
    self._custom_transport = transport
    self._transport: Transport | None = None
    self._query: Any | None = None
    os.environ["CLAUDE_CODE_ENTRYPOINT"] = "sdk-py-client"
```

### Command Building Process

The `SubprocessCLITransport` handles:
- **Command-line argument construction** for MCP servers and streaming modes
- **Process spawning** with environment variables:
  - `CLAUDE_CODE_ENTRYPOINT`: Set to `"sdk-py-client"`
  - `CLAUDE_AGENT_SDK_VERSION`: SDK version information
- **Stream setup** for stdin/stdout/stderr

### Mode Selection

**String Mode:**
- Uses `--print` flag with prompt passed as CLI argument
- Closes stdin immediately after process start

**Streaming Mode:**
- Uses `--input-format stream-json` flag
- Keeps stdin open for bidirectional communication

### Version Checking

The `_check_claude_version()` method validates CLI compatibility before establishing the connection.

### Focus Areas
- `SubprocessCLITransport.connect()` method and `_build_command()` logic
- CLI argument construction for MCP servers and streaming modes
- Process spawning with environment variables and stream setup
- Version checking and validation

---

## Control Protocol Handshake

### Overview
Investigates the bidirectional control protocol handshake that occurs after CLI subprocess startup.

### Initialization Request Format

**Query.initialize()** sends an initialize control request:
```python
request = {
    "subtype": "initialize",
    "hooks": hooks_config if hooks_config else None,
}
```

### Response Structure

The CLI responds with:
- **Supported commands**: Available control protocol commands
- **Capabilities**: What the CLI server can do
- **Output styles**: Current and available output styles

### Request/Response Tracking

- Uses JSON-RPC style message format
- Request IDs for tracking pending responses
- Stored in `_initialization_result` for later access

### Focus Areas
- `Query.initialize()` control request format and hooks configuration
- Control message routing in `_read_messages()` method
- Request ID generation and pending response tracking
- Initialization result storage and retrieval (`src/claude_agent_sdk/_internal/client.py:106-107`)

---

## SDK MCP Server Registration

### Overview
Analyzes how in-process SDK MCP servers are registered and distinguished from external MCP servers.

### Configuration Structure

```python
class McpSdkServerConfig(TypedDict):
    type: Literal["sdk"]
    name: str
    instance: "McpServer"  # The actual MCP server instance
```

### Registration Process

1. **Server Creation**: `create_sdk_mcp_server()` creates MCP server instances
2. **Configuration**: Passed in `ClaudeAgentOptions.mcp_servers` with `type='sdk'`
3. **CLI Communication**: SDK strips the 'instance' field when passing to CLI via `--mcp-config`
   - Retains other configuration fields
   - Keeps server instance internally for request handling

### SDK vs External Servers

**SDK Servers (`type='sdk'`)**:
- Run in-process within the Python SDK
- Instance stored in `sdk_mcp_servers` dictionary
- Communicate via control protocol messages

**External Servers**:
- Run as separate processes
- Configured with command/args or stdio transport
- Direct CLI communication

### Focus Areas
- SDK vs external MCP server configuration format
- MCP server instance extraction in `InternalClient` and `ClaudeSDKClient`
- Command-line argument building for MCP servers
- `sdk_mcp_servers` dictionary population and storage

---

## MCP Tool Communication Protocol

### Overview
Examines the bidirectional communication mechanism for SDK MCP tool invocations.

### Message Structure

```python
# types.py:578-582
class SDKControlMcpMessageRequest(TypedDict):
    subtype: Literal["mcp_message"]
    server_name: str  # Name of the SDK MCP server
    message: Any      # JSONRPC payload (initialize, tools/list, tools/call)
```

### Communication Flow

1. **CLI → SDK**: Sends `control_request` with `subtype='mcp_message'`
2. **SDK Routing**: `Query._handle_sdk_mcp_request()` routes to appropriate server instance
3. **Method Invocation**: Manual routing to:
   - `initialize`: Server initialization
   - `tools/list`: List available tools
   - `tools/call`: Execute tool
4. **SDK → CLI**: Response flows back through `control_response` messages

### Limitations

Unlike the TypeScript SDK's transport abstraction, the Python SDK uses manual method routing for JSONRPC messages.

### Focus Areas
- Control request handling for MCP messages in `Query`
- JSONRPC message routing to MCP server handlers
- Method implementations (initialize, tools/list, tools/call)
- Response format conversion and error handling

---

## Message Streaming and Parsing

### Overview
Investigates how messages flow bidirectionally between SDK and CLI after initialization.

### Transport Write Operation

```python
async def write(self, data: str) -> None:
    """Write raw data to the transport."""
    # Safety checks
    if not self._ready or not self._stdin_stream:
        raise CLIConnectionError("ProcessTransport is not ready for writing")

    if self._process and self._process.returncode is not None:
        raise CLIConnectionError(f"Cannot write to terminated process...")

    if self._exit_error:
        raise CLIConnectionError(f"Cannot write to process that exited with error...")

    try:
        await self._stdin_stream.send(data)  # Write to TextSendStream
    except Exception as e:
        self._ready = False
        self._exit_error = CLIConnectionError(f"Failed to write to process stdin: {e}")
        raise self._exit_error from e
```

### Reading and Parsing

- **Line-by-line reading**: stdout read via `TextReceiveStream`
- **JSON buffering**: Partial JSON buffered and parsed speculatively
- **Buffer management**: `max_buffer_size` prevents memory issues
- **Message queueing**: anyio memory object streams for SDK messages

### Message Routing by Type

- `control_request`: Bidirectional requests from CLI to SDK
- `control_response`: Responses to SDK control requests
- Regular messages: SDK messages sent to `_message_receive` stream

### Focus Areas
- `Transport.write()` and `Transport.read_messages()` implementations
- JSON buffering and speculative parsing with max_buffer_size
- Message type routing and control protocol separation
- anyio memory object streams for message queueing

---

## Tool Permission Control Flow

### Overview
Analyzes the tool permission callback mechanism when `can_use_tool` is provided.

### Callback Type Definition

```python
CanUseTool = Callable[
    [str, dict[str, Any], ToolPermissionContext],
    Awaitable[PermissionResult]
]
```

### Permission Flow

1. **SDK Configuration**: Automatically sets `permission_prompt_tool_name='stdio'` when callback provided
2. **Permission Request**: CLI sends `control_request` with `subtype='can_use_tool'`
   - Contains tool name and input parameters
3. **SDK Callback**: User-provided callback returns:
   - `PermissionResultAllow`: Allow execution (optionally with updated inputs)
   - `PermissionResultDeny`: Deny execution
4. **Response**: Updated inputs and permissions flow back to CLI

### Validation

Mutually exclusive options enforced:
- Cannot use both `permission_prompt_tool_name` and `can_use_tool` callback
- SDK validates configuration at initialization

### Focus Areas
- Permission callback configuration and validation
- `SDKControlPermissionRequest` message structure
- `PermissionResult` conversion to control protocol format
- Updated input and permission suggestions handling

---

## Streaming vs String Mode Differences

### Overview
Documents the architectural differences between string mode (one-shot queries) and streaming mode (bidirectional clients).

### Mode Determination

**Location**: `src/claude_agent_sdk/_internal/transport/subprocess_cli.py:43`

```python
self._is_streaming = not isinstance(prompt, str)
```

- **String mode**: `prompt` is `str`
- **Streaming mode**: `prompt` is `AsyncIterable[dict[str, Any]]`

### CLI Invocation

#### String Mode
**Location**: `subprocess_cli.py:198-199`

```python
cmd.extend(["--print", "--", str(self._prompt)])
```

**Characteristics**:
- Prompt passed as CLI argument via `--print` flag
- No `--input-format` flag needed
- One-shot execution model

#### Streaming Mode
**Location**: `subprocess_cli.py:195-196`

```python
cmd.extend(["--input-format", "stream-json"])
```

**Characteristics**:
- Uses `--input-format stream-json` flag
- No prompt in CLI arguments
- Messages sent via stdin
- Bidirectional communication model

### Output Format

**Both modes** use the same output format (`subprocess_cli.py:88`):
```python
cmd = [self._cli_path, "--output-format", "stream-json", "--verbose"]
```

### Stdin Handling

#### String Mode
**Location**: `subprocess_cli.py:256-258`

```python
elif not self._is_streaming and self._process.stdin:
    # String mode: close stdin immediately
    await self._process.stdin.aclose()
```

**Characteristics**:
- stdin closed immediately after process start
- No further input possible
- Process reads prompt from CLI args only

#### Streaming Mode
**Location**: `subprocess_cli.py:254-255`

```python
if self._is_streaming and self._process.stdin:
    self._stdin_stream = TextSendStream(self._process.stdin)
```

**Characteristics**:
- stdin kept open via `TextSendStream`
- Used for dynamic message sending
- Enables bidirectional communication
- Closed later via `end_input()` method

### Control Protocol Availability

#### Initialization
**Location**: `src/claude_agent_sdk/_internal/query.py:107-114`

```python
async def initialize(self) -> dict[str, Any] | None:
    """Initialize control protocol if in streaming mode."""
    if not self.is_streaming_mode:
        return None
```

- **String mode**: Returns `None`, no initialization handshake
- **Streaming mode**: Sends initialize control request, receives capabilities

#### Control Request Validation
**Location**: `query.py:317-320`

```python
async def _send_control_request(self, request: dict[str, Any]) -> dict[str, Any]:
    if not self.is_streaming_mode:
        raise Exception("Control requests require streaming mode")
```

**All control protocol features disabled in string mode**:
- `interrupt()`
- `set_permission_mode()`
- `set_model()`
- `can_use_tool` callbacks
- Hook callbacks
- SDK MCP server handling

### Message Streaming

#### String Mode
- **Prompt delivery**: Via CLI `--print` argument
- **Input stream**: Closed immediately, no dynamic messages
- **Output stream**: CLI streams responses via stdout
- **Flow**: Unidirectional (CLI → SDK)

#### Streaming Mode
**Location**: `query.py:513-524`

```python
async def stream_input(self, stream: AsyncIterable[dict[str, Any]]) -> None:
    async for message in stream:
        if self._closed:
            break
        await self.transport.write(json.dumps(message) + "\n")
    await self.transport.end_input()
```

- **Prompt delivery**: Via stdin as stream-json messages
- **Input stream**: Kept open for dynamic message sending
- **Output stream**: CLI streams responses via stdout
- **Flow**: Bidirectional (SDK ↔ CLI)

### Client Integration

#### InternalClient
**Location**: `src/claude_agent_sdk/_internal/client.py:41-121`

**Mode Detection** (line 90):
```python
is_streaming = not isinstance(prompt, str)
```

**Initialization Flow** (lines 102-114):
```python
await query.start()
if is_streaming:
    await query.initialize()
    if isinstance(prompt, AsyncIterable) and query._tg:
        query._tg.start_soon(query.stream_input, prompt)
```

**String Mode Behavior**:
- No initialization call
- Prompt already in CLI args
- Just starts reading responses

**Streaming Mode Behavior**:
- Calls `initialize()` for handshake
- Spawns background task to stream input
- Enables bidirectional control protocol

#### ClaudeSDKClient
**Location**: `src/claude_agent_sdk/client.py:14-336`

**Architecture**: Always uses streaming mode

**Key Design** (line 144):
```python
self._query = Query(
    transport=self._transport,
    is_streaming_mode=True,  # ClaudeSDKClient always uses streaming mode
    ...
)
```

**Rationale**: ClaudeSDKClient is designed for interactive, bidirectional conversations. It always creates an `AsyncIterable` prompt (even if empty) to maintain an open stdin connection for dynamic message sending.

**Empty Stream Pattern** (lines 94-99):
```python
async def _empty_stream() -> AsyncIterator[dict[str, Any]]:
    # Never yields, but indicates this is an iterator and keeps connection open
    return
    yield {}  # Unreachable but makes this an async generator
```

When `connect()` is called without a prompt, creates an empty async iterator to trigger streaming mode while keeping stdin open for later `query()` calls.

**Dynamic Querying** (`query(prompt, session_id)`, lines 170-199):
- Converts string prompts to stream-json messages
- Sends via already-open stdin connection
- Requires prior `connect()` with streaming mode

### Control Protocol Features

#### Available in Streaming Only

**interrupt()** (`query.py:491-493`)
- Sends control request with `subtype: interrupt`

**set_permission_mode()** (`query.py:495-502`)
- Sends control request with `subtype: set_permission_mode`, `mode: string`

**set_model()** (`query.py:504-511`)
- Sends control request with `subtype: set_model`, `model: string`

**can_use_tool callback** (`query.py:215-256`)
- Handles request with `subtype: can_use_tool`
- Bidirectional: CLI requests permission from SDK

**hook_callbacks** (`query.py:258-272`)
- Handles request with `subtype: hook_callback`
- Bidirectional: CLI invokes SDK hook functions

**sdk_mcp_servers** (`query.py:274-289`)
- Handles request with `subtype: mcp_message`
- Bidirectional: CLI routes MCP requests to in-process Python servers

#### Unavailable in String Mode
- **Reason**: No open stdin for control messages
- **Error behavior**: Exception raised if attempted

### Message Routing

**Query._read_messages()** (`query.py:154-205`):
- `control_response`: Routes to `pending_control_results`
- `control_request`: Spawns handler task
- `control_cancel_request`: TODO (not yet implemented)
- Regular messages: Sent to `_message_receive` stream

Both modes handle message reading the same way, but string mode never receives control messages.

### Architecture Summary

#### String Mode
- **Use case**: One-shot queries with all inputs known upfront
- **Communication**: Unidirectional (CLI → SDK)
- **stdin**: Closed immediately
- **CLI args**: Includes `--print` with prompt
- **Control protocol**: Disabled
- **Initialization**: None

**Suitable for**:
- Simple questions
- Batch processing
- Fire-and-forget automation
- Stateless operations

#### Streaming Mode
- **Use case**: Interactive, bidirectional conversations
- **Communication**: Bidirectional (SDK ↔ CLI)
- **stdin**: Kept open for dynamic message sending
- **CLI args**: Includes `--input-format stream-json`
- **Control protocol**: Enabled
- **Initialization**: Handshake with capability negotiation

**Suitable for**:
- Chat interfaces
- Multi-turn conversations
- Interactive debugging
- Real-time applications
- Dynamic permission management
- SDK MCP server integration

#### ClaudeSDKClient: Always Streaming

**Design Principle**: ClaudeSDKClient is architected exclusively for streaming mode to enable its core features: dynamic message sending via `query()`, interrupts, runtime permission/model changes, and bidirectional control protocol.

**Implementation**:
- Always passes `is_streaming_mode=True` to Query
- Always uses `--input-format stream-json`
- Always keeps stdin open
- Always performs initialization handshake
- Converts string prompts to stream-json messages internally

**Contrast**:
- **query() function**: Supports both string and streaming modes
- **ClaudeSDKClient**: Streaming mode only, by design

---

## Key Takeaways

1. **Two distinct modes** with fundamentally different architectures
2. **Control protocol** is the key differentiator enabling bidirectional features
3. **ClaudeSDKClient** is purpose-built for interactive streaming scenarios
4. **SDK MCP servers** run in-process and communicate via control protocol
5. **Message routing** handles both regular SDK messages and control protocol messages
6. **Permission callbacks** enable fine-grained runtime control over tool execution
7. **Initialization handshake** establishes capabilities and configuration
