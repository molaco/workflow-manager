# MCP Tools Recognition - Implementation Strategy

## Problem Statement

MCP tools configured via SDK MCP servers are not being recognized by Claude in the Rust SDK, despite working correctly in the Python SDK. Both SDKs wrap the same Claude CLI with the same version, so this is a configuration/initialization issue in the Rust implementation.

## Root Cause Analysis

Based on the Python SDK research (RESEARCH.md), the issue is that **SDK MCP servers require the control protocol to be initialized with hooks configuration**. The key insight from the Python SDK is:

### Python SDK Approach (Working)

1. **Initialization Request with Hooks** - When SDK MCP servers are present, Python sends an initialization request that includes hooks configuration:
   ```python
   request = {
       "subtype": "initialize",
       "hooks": hooks_config if hooks_config else None,
   }
   ```

2. **MCP Message Routing** - CLI sends `control_request` with `subtype='mcp_message'` to SDK:
   ```python
   class SDKControlMcpMessageRequest(TypedDict):
       subtype: Literal["mcp_message"]
       server_name: str  # Name of the SDK MCP server
       message: Any      # JSONRPC payload (initialize, tools/list, tools/call)
   ```

3. **Bidirectional Communication** - SDK MCP servers REQUIRE streaming mode and bidirectional control protocol:
   - CLI → SDK: `mcp_message` requests
   - SDK → CLI: `mcp_message_response` responses

### Rust SDK Issues (Current)

1. **Missing Initialization Handshake** - `src/client/mod.rs` doesn't send initialization request with hooks
2. **No MCP Message Handling** - Control protocol doesn't route incoming `mcp_message` requests to SDK MCP server instances
3. **Streaming Mode Not Forced** - SDK MCP servers should force streaming mode (like hooks/permissions do)

## Implementation Strategy

### Phase 1: Extend Control Protocol for MCP Messages ✅ (Already Complete)

**Files to check:**
- `src/control/protocol.rs` - Already has `McpMessage` and `McpMessageResponse` variants
- Control protocol structure appears complete

**Verification:**
```rust
// src/control/protocol.rs:218-227
#[serde(rename = "mcp_message")]
McpMessage {
    id: RequestId,
    server_name: String,
    message: serde_json::Value,
}

// src/control/protocol.rs:154-161
#[serde(rename = "mcp_message_response")]
McpMessageResponse {
    id: RequestId,
    mcp_response: serde_json::Value,
}
```

✅ **Status:** Protocol message types already exist.

---

### Phase 2: Enforce Streaming Mode for SDK MCP Servers

**File:** `src/transport/subprocess.rs:143-172`

**Current Code (lines 150-162):**
```rust
let has_sdk_mcp_servers = match &self.options.mcp_servers {
    crate::types::McpServers::Dict(servers) => {
        servers.values().any(|config| matches!(config, crate::types::McpServerConfig::Sdk(_)))
    }
    _ => false,
};

let needs_control_protocol = self.options.hooks.is_some()
    || self.options.can_use_tool.is_some()
    || has_sdk_mcp_servers;

// Stream input ALWAYS needs streaming mode (used by ClaudeSDKClient.send_message)
let needs_streaming_mode = matches!(self.prompt, PromptInput::Stream) || needs_control_protocol;
```

✅ **Status:** Already implemented! SDK MCP servers force streaming mode via `needs_control_protocol`.

---

### Phase 3: Initialize Control Protocol with Hooks Configuration

**File:** `src/client/mod.rs` (implementation needed in constructor)

**Problem:** ClaudeSDKClient doesn't send initialization request when SDK MCP servers are configured.

**Required Changes:**

1. **Extract SDK MCP server instances during client creation**
2. **Convert hooks configuration to `HookMatcherConfig` format**
3. **Send initialization request with hooks configuration**
4. **Store SDK MCP server instances for later routing**

**Implementation Location:** `ClaudeSDKClient::new()` method

**Code structure needed:**
```rust
// 1. Extract SDK MCP server instances
let sdk_mcp_servers: HashMap<String, Arc<SdkMcpServer>> = /* ... */;

// 2. Build hooks configuration for CLI
let hooks_config: Option<HashMap<HookEvent, Vec<HookMatcherConfig>>> = /* ... */;

// 3. Send initialization request
let init_request = protocol.create_init_request_with_hooks(hooks_config);
let init_message = ControlMessage::Init(init_request);
// Send to CLI via transport...

// 4. Store sdk_mcp_servers for routing
```

---

### Phase 4: Route MCP Messages to SDK MCP Servers

**File:** `src/client/mod.rs` (background task implementation)

**Problem:** No handler for incoming `control_request` messages with `mcp_message` subtype.

**Required Changes:**

1. **Add MCP callback channel to ProtocolHandler**
   - Similar to hook_callback_tx and hook_callback_channel
   - Already exists: `mcp_callback_tx` and `get_mcp_callback_channel()` ✅

2. **Handle incoming MCP messages in background task**
   - Receive from `mcp_callback_rx`
   - Route to appropriate SDK MCP server by `server_name`
   - Invoke `server.handle_request(jsonrpc_request)`
   - Send response via `McpMessageResponse`

**Implementation Pattern:**
```rust
// In background task
tokio::spawn(async move {
    while let Some((request_id, server_name, message)) = mcp_callback_rx.recv().await {
        // 1. Find SDK MCP server
        let server = sdk_mcp_servers.get(&server_name)?;

        // 2. Parse JSONRPC request
        let jsonrpc_req: JsonRpcRequest = serde_json::from_value(message)?;

        // 3. Handle request
        let jsonrpc_resp = server.handle_request(jsonrpc_req).await?;

        // 4. Send response
        let response = protocol.create_mcp_message_response(
            request_id,
            serde_json::to_value(jsonrpc_resp)?
        );
        // Write to transport...
    }
});
```

---

### Phase 5: Send Initialization Request to CLI

**File:** `src/client/mod.rs`

**Implementation in `ClaudeSDKClient::new()`:**

```rust
// After transport.connect()
{
    let mut protocol_guard = protocol.lock().await;

    // Build hooks config if needed
    let hooks_config = if has_sdk_mcp_servers || has_hooks {
        Some(build_hooks_config(&options))
    } else {
        None
    };

    // Create initialization request
    let init_req = protocol_guard.create_init_request_with_hooks(hooks_config);
    let init_msg = ControlMessage::Init(init_req);

    // Serialize and send
    let serialized = protocol_guard.serialize_message(&init_msg)?;
    drop(protocol_guard); // Release lock before writing

    // Write to transport
    let mut transport_guard = transport.lock().await;
    transport_guard.write(&serialized).await?;
}

// Wait for init response and mark protocol as initialized
```

---

## Detailed Implementation Plan

### Step 1: Modify `ClaudeSDKClient::new()` Constructor

**Location:** `src/client/mod.rs` (around line 200-300)

**Changes needed:**

1. **Extract SDK MCP servers from options:**
```rust
let sdk_mcp_servers: HashMap<String, Arc<SdkMcpServer>> = match &options.mcp_servers {
    McpServers::Dict(servers) => {
        servers
            .iter()
            .filter_map(|(name, config)| {
                if let McpServerConfig::Sdk(marker) = config {
                    Some((name.clone(), marker.instance.clone()))
                } else {
                    None
                }
            })
            .collect()
    }
    _ => HashMap::new(),
};
```

2. **Build hooks configuration for CLI:**
```rust
fn build_hooks_config(options: &ClaudeAgentOptions) -> HashMap<HookEvent, Vec<HookMatcherConfig>> {
    options.hooks.as_ref().map(|hooks_map| {
        hooks_map
            .iter()
            .map(|(event, matchers)| {
                let configs: Vec<HookMatcherConfig> = matchers
                    .iter()
                    .enumerate()
                    .map(|(idx, matcher)| {
                        HookMatcherConfig {
                            matcher: matcher.matcher.clone(),
                            hook_callback_ids: vec![format!("{:?}_{}", event, idx)],
                        }
                    })
                    .collect();
                (*event, configs)
            })
            .collect()
    }).unwrap_or_default()
}
```

3. **Send initialization request:**
```rust
// After transport.connect()
let has_sdk_mcp = !sdk_mcp_servers.is_empty();
let has_hooks = options.hooks.is_some();

if has_sdk_mcp || has_hooks {
    // Build hooks config
    let hooks_config = Some(build_hooks_config(&options));

    // Create and send init request
    let init_req = protocol.lock().await.create_init_request_with_hooks(hooks_config);
    let init_msg = ControlMessage::Init(init_req);
    let serialized = protocol.lock().await.serialize_message(&init_msg)?;

    transport.lock().await.write(&serialized).await?;

    // TODO: Wait for init response and set protocol.set_initialized(true)
}
```

---

### Step 2: Add MCP Message Handler Background Task

**Location:** `src/client/mod.rs` (in constructor, spawn background task)

**Implementation:**

```rust
// Set up MCP callback channel
let (mcp_callback_tx, mut mcp_callback_rx) = mpsc::unbounded_channel();
protocol.lock().await.set_mcp_callback_channel(mcp_callback_tx);

// Spawn MCP message handler task
let sdk_mcp_servers_clone = Arc::new(sdk_mcp_servers);
let protocol_clone = protocol.clone();
let transport_clone = transport.clone();

tokio::spawn(async move {
    while let Some((request_id, server_name, message)) = mcp_callback_rx.recv().await {
        // Get server instance
        let server = match sdk_mcp_servers_clone.get(&server_name) {
            Some(s) => s.clone(),
            None => {
                eprintln!("SDK MCP server not found: {}", server_name);
                continue;
            }
        };

        // Parse JSONRPC request
        let jsonrpc_req: crate::mcp::protocol::JsonRpcRequest = match serde_json::from_value(message) {
            Ok(req) => req,
            Err(e) => {
                eprintln!("Failed to parse JSONRPC request: {}", e);
                continue;
            }
        };

        // Handle request
        let jsonrpc_resp = match server.handle_request(jsonrpc_req).await {
            Ok(resp) => resp,
            Err(e) => {
                eprintln!("MCP server error: {}", e);
                continue;
            }
        };

        // Send response
        let mcp_resp_value = match serde_json::to_value(&jsonrpc_resp) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Failed to serialize JSONRPC response: {}", e);
                continue;
            }
        };

        let response = protocol_clone.lock().await.create_mcp_message_response(
            request_id,
            mcp_resp_value
        );
        let control_msg = ControlMessage::Request(response);

        // Serialize and write
        if let Ok(serialized) = protocol_clone.lock().await.serialize_message(&control_msg) {
            let _ = transport_clone.lock().await.write(&serialized).await;
        }
    }
});
```

---

### Step 3: Handle Incoming MCP Messages in Message Reader

**Location:** `src/client/mod.rs` (message reader background task)

**Current code structure:**
The message reader already handles control messages via `parse_control_message()`. We need to ensure `ControlResponse::McpMessage` is routed to the MCP callback handler.

**Verification:**
```rust
// src/control/protocol.rs:499-505 (handle_response method)
ControlResponse::McpMessage { id, server_name, message } => {
    if let Some(ref tx) = self.mcp_callback_tx {
        tx.send((id.clone(), server_name.clone(), message.clone()))
            .map_err(|_| ClaudeError::protocol_error("MCP callback channel closed"))?;
    }
    Ok(())
}
```

✅ **Status:** Already implemented in ProtocolHandler.

---

## Summary of Required Changes

### Files to Modify:

1. **`src/client/mod.rs`** (Major changes)
   - [ ] Extract SDK MCP server instances in constructor
   - [ ] Build hooks configuration from options
   - [ ] Send initialization request to CLI
   - [ ] Spawn MCP message handler background task
   - [ ] Wait for initialization response

2. **`src/transport/subprocess.rs`** (No changes needed)
   - ✅ Already forces streaming mode for SDK MCP servers

3. **`src/control/protocol.rs`** (No changes needed)
   - ✅ Already has MCP message types
   - ✅ Already has MCP callback channel support
   - ✅ Already routes MCP messages to callback

4. **`src/mcp/server.rs`** (No changes needed)
   - ✅ Already handles JSONRPC requests correctly

---

## Testing Strategy

### Test 1: Verify SDK MCP Server Instance Extraction

```rust
#[tokio::test]
async fn test_sdk_mcp_server_extraction() {
    let server = Arc::new(SdkMcpServer::new("test"));
    let mut servers = HashMap::new();
    servers.insert(
        "test".to_string(),
        McpServerConfig::Sdk(SdkMcpServerMarker {
            name: "test".to_string(),
            instance: server.clone(),
        }),
    );

    let options = ClaudeAgentOptions::builder()
        .mcp_servers(servers)
        .build();

    // Extract SDK MCP servers
    // Verify extracted correctly
}
```

### Test 2: Verify Initialization Request Format

```rust
#[test]
fn test_init_request_serialization() {
    let protocol = ProtocolHandler::new();
    let hooks_config = Some(HashMap::new());
    let init_req = protocol.create_init_request_with_hooks(hooks_config);

    let msg = ControlMessage::Init(init_req);
    let serialized = protocol.serialize_message(&msg).unwrap();

    // Verify JSON structure matches Python SDK format
    assert!(serialized.contains("\"protocol_version\":\"1.0\""));
    assert!(serialized.contains("\"hooks\""));
}
```

### Test 3: Integration Test with Real MCP Server

```rust
#[tokio::test]
async fn test_sdk_mcp_integration() {
    let server = SdkMcpServer::new("calculator")
        .tool(SdkMcpTool::new(
            "add",
            "Add numbers",
            json!({"type": "object"}),
            |input| Box::pin(async move {
                Ok(ToolResult::text("42"))
            })
        ));

    let mut servers = HashMap::new();
    servers.insert(
        "calculator".to_string(),
        McpServerConfig::Sdk(SdkMcpServerMarker {
            name: "calculator".to_string(),
            instance: Arc::new(server),
        }),
    );

    let options = ClaudeAgentOptions::builder()
        .mcp_servers(servers)
        .build();

    let mut client = ClaudeSDKClient::new(options, None).await.unwrap();
    client.send_message("Use the add tool to calculate 2+2").await.unwrap();

    // Verify tool is called
}
```

---

## Python SDK Reference Points

### Initialization with Hooks (Python)
**File:** `src/claude_agent_sdk/_internal/query.py:107-145`
```python
async def initialize(self) -> dict[str, Any] | None:
    if not self.is_streaming_mode:
        return None

    # Build hooks configuration
    hooks_config = self._build_hooks_config() if self.hook_callbacks else None

    request = {
        "subtype": "initialize",
        "hooks": hooks_config if hooks_config else None,
    }

    response = await self._send_control_request(request)
    self._initialized = True
    self._initialization_result = response
    return response
```

### MCP Message Handling (Python)
**File:** `src/claude_agent_sdk/_internal/query.py:274-289`
```python
async def _handle_sdk_mcp_request(
    self, request_id: str, server_name: str, message: Any
) -> None:
    server = self.sdk_mcp_servers.get(server_name)
    if not server:
        return

    # Route to MCP server
    method = message.get("method")
    if method == "initialize":
        result = await server.initialize()
    elif method == "tools/list":
        result = await server.list_tools()
    elif method == "tools/call":
        result = await server.call_tool(message.get("params"))

    # Send response back to CLI
    await self._send_control_response(request_id, result)
```

---

## Implementation Priority

1. **HIGH:** Extract SDK MCP server instances in constructor
2. **HIGH:** Send initialization request with hooks config
3. **HIGH:** Spawn MCP message handler background task
4. **MEDIUM:** Add comprehensive error handling
5. **MEDIUM:** Add logging/tracing for debugging
6. **LOW:** Optimize hook config building
7. **LOW:** Add metrics/telemetry

---

## Success Criteria

✅ **SDK MCP tools appear in Claude's tool list**
✅ **Claude can successfully invoke SDK MCP tools**
✅ **JSONRPC requests routed correctly to SDK MCP servers**
✅ **Tool results returned to CLI correctly**
✅ **Integration tests pass**
✅ **Works identically to Python SDK behavior**

---

## Next Steps

1. Implement `ClaudeSDKClient::new()` modifications
2. Add MCP message handler background task
3. Test with `examples/hooks_advanced_demo.rs`
4. Verify tool list and invocation work
5. Add comprehensive tests
6. Document the changes
