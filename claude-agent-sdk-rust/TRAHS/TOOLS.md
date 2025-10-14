# Implementation Guide: Control Response Handling Fix

## Objective

Fix SDK MCP server integration by implementing proper control response handling. Currently, the Rust SDK ignores `control_response` messages from the Claude CLI, causing initialization to be incomplete. This prevents the CLI from fetching tools from SDK MCP servers and exposing them to Claude.

**Expected Outcome:** Claude will receive the full list of tools from SDK MCP servers and be able to invoke them successfully.

---

## Files to Create/Change

### Files to Change
1. `src/client/mod.rs` - Main client implementation
2. `examples/mcp_integration_demo.rs` - Update to verify fix

### Files to Create (Optional)
3. `tests/control_response_test.rs` - Integration test for control response handling

---

## Components to Create/Change

### Structs
- `ClaudeSDKClient` - Add `pending_control_responses` field

### Functions to Change
- `ClaudeSDKClient::new()` - Initialize pending responses map and pass to tasks
- `ClaudeSDKClient::message_reader_task()` - Add response routing logic
- `ClaudeSDKClient::send_initialize()` - Replace sleep with response waiting

### Types to Add
- Import `tokio::sync::oneshot` for one-shot channels
- `HashMap<String, oneshot::Sender<Result<serde_json::Value>>>` for pending responses

---

## Sequential Task List

1. **Add imports** - Import `oneshot` from tokio::sync
2. **Add struct field** - Add `pending_control_responses` to `ClaudeSDKClient`
3. **Initialize map** - Create the pending responses map in `new()`
4. **Pass to task** - Pass map to `message_reader_task()`
5. **Store in struct** - Store map in `ClaudeSDKClient` instance
6. **Update task signature** - Add parameter to `message_reader_task()`
7. **Implement response routing** - Handle `control_response` messages properly
8. **Update send_initialize** - Create channel, wait for response
9. **Update call site** - Handle returned response value
10. **Test with example** - Run `mcp_integration_demo` to verify
11. **Write integration test** - Create test for control response flow

---

## Detailed Specification

### Task 1: Add Imports

**File:** `src/client/mod.rs`
**Location:** Top of file (around line 10-20)
**Action:** Add imports

**Add:**
```rust
use tokio::sync::oneshot;
```

**Verify:** Check if `std::collections::HashMap` is already imported

---

### Task 2: Add Struct Field

**File:** `src/client/mod.rs`
**Location:** Line ~185 (in `ClaudeSDKClient` struct definition)
**Action:** Add new field to struct

**Before:**
```rust
pub struct ClaudeSDKClient {
    transport: Arc<Mutex<SubprocessTransport>>,
    protocol: Arc<Mutex<ProtocolHandler>>,
    message_rx: mpsc::UnboundedReceiver<Result<Message>>,
    control_tx: mpsc::UnboundedSender<ControlRequest>,
    hook_rx: Option<mpsc::UnboundedReceiver<(String, HookEvent, serde_json::Value)>>,
    permission_rx: Option<mpsc::UnboundedReceiver<(RequestId, PermissionRequest)>>,
    hook_manager: Option<Arc<Mutex<HookManager>>>,
    permission_manager: Option<Arc<Mutex<PermissionManager>>>,
    sdk_mcp_servers: HashMap<String, Arc<crate::mcp::SdkMcpServer>>,
    mcp_callback_tx: Option<mpsc::UnboundedSender<(RequestId, String, serde_json::Value)>>,
}
```

**After (add before closing brace):**
```rust
pub struct ClaudeSDKClient {
    transport: Arc<Mutex<SubprocessTransport>>,
    protocol: Arc<Mutex<ProtocolHandler>>,
    message_rx: mpsc::UnboundedReceiver<Result<Message>>,
    control_tx: mpsc::UnboundedSender<ControlRequest>,
    hook_rx: Option<mpsc::UnboundedReceiver<(String, HookEvent, serde_json::Value)>>,
    permission_rx: Option<mpsc::UnboundedReceiver<(RequestId, PermissionRequest)>>,
    hook_manager: Option<Arc<Mutex<HookManager>>>,
    permission_manager: Option<Arc<Mutex<PermissionManager>>>,
    sdk_mcp_servers: HashMap<String, Arc<crate::mcp::SdkMcpServer>>,
    mcp_callback_tx: Option<mpsc::UnboundedSender<(RequestId, String, serde_json::Value)>>,
    /// Pending control response channels (for initialize and other control requests)
    pending_control_responses: Arc<Mutex<HashMap<String, oneshot::Sender<Result<serde_json::Value>>>>>,
}
```

**Test:** Run `cargo check` to verify syntax

---

### Task 3: Initialize Map in Constructor

**File:** `src/client/mod.rs`
**Location:** Line ~265 (in `ClaudeSDKClient::new()` function)
**Action:** Create the pending responses map

**Add after extracting SDK MCP servers (around line 265):**
```rust
// Create pending control responses map for routing control responses
let pending_control_responses = Arc::new(Mutex::new(HashMap::new()));
```

**Test:** Run `cargo check`

---

### Task 4: Pass Map to Message Reader Task

**File:** `src/client/mod.rs`
**Location:** Line ~290 (where `message_reader_task` is spawned)
**Action:** Clone and pass the map to the background task

**Before:**
```rust
let transport_clone = transport.clone();
let protocol_clone = protocol.clone();
tokio::spawn(async move {
    Self::message_reader_task(
        transport_clone,
        protocol_clone,
        message_tx,
    )
    .await;
});
```

**After:**
```rust
let transport_clone = transport.clone();
let protocol_clone = protocol.clone();
let pending_responses_clone = pending_control_responses.clone();
tokio::spawn(async move {
    Self::message_reader_task(
        transport_clone,
        protocol_clone,
        message_tx,
        pending_responses_clone,
    )
    .await;
});
```

**Test:** Run `cargo check` (will fail until Task 6 is complete)

---

### Task 5: Store Map in Struct Instance

**File:** `src/client/mod.rs`
**Location:** Line ~392 (where `ClaudeSDKClient` is constructed)
**Action:** Add field to struct initialization

**Before:**
```rust
let mut client = Self {
    transport,
    protocol,
    message_rx,
    control_tx,
    hook_rx: None,
    permission_rx,
    hook_manager,
    permission_manager,
    sdk_mcp_servers,
    mcp_callback_tx,
};
```

**After:**
```rust
let mut client = Self {
    transport,
    protocol,
    message_rx,
    control_tx,
    hook_rx: None,
    permission_rx,
    hook_manager,
    permission_manager,
    sdk_mcp_servers,
    mcp_callback_tx,
    pending_control_responses,
};
```

**Test:** Run `cargo check`

---

### Task 6: Update Message Reader Task Signature

**File:** `src/client/mod.rs`
**Location:** Line ~414 (`message_reader_task` function definition)
**Action:** Add parameter to function signature

**Before:**
```rust
async fn message_reader_task(
    transport: Arc<Mutex<SubprocessTransport>>,
    protocol: Arc<Mutex<ProtocolHandler>>,
    message_tx: mpsc::UnboundedSender<Result<Message>>,
) {
```

**After:**
```rust
async fn message_reader_task(
    transport: Arc<Mutex<SubprocessTransport>>,
    protocol: Arc<Mutex<ProtocolHandler>>,
    message_tx: mpsc::UnboundedSender<Result<Message>>,
    pending_control_responses: Arc<Mutex<HashMap<String, oneshot::Sender<Result<serde_json::Value>>>>>,
) {
```

**Test:** Run `cargo check` - should now pass

---

### Task 7: Implement Control Response Routing

**File:** `src/client/mod.rs`
**Location:** Lines 432-440 (control_response handler in `message_reader_task`)
**Action:** Replace ignore logic with proper routing

**Before:**
```rust
// Handle control_response (init response, etc.)
if msg_type == Some("control_response") {
    // This is a response to our init request - just log and continue
    #[cfg(feature = "tracing-support")]
    tracing::debug!("Received control_response");
    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
    eprintln!("Received control_response");
    continue;
}
```

**After:**
```rust
// Handle control_response (init response, etc.)
if msg_type == Some("control_response") {
    #[cfg(feature = "tracing-support")]
    tracing::debug!("Received control_response");
    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
    eprintln!("Received control_response");

    // Extract request_id and route to pending request
    if let Some(response_obj) = value.get("response") {
        if let Some(request_id) = response_obj.get("request_id").and_then(|v| v.as_str()) {
            let mut pending = pending_control_responses.lock().await;
            if let Some(tx) = pending.remove(request_id) {
                // Check if it's an error response
                let result = if response_obj.get("subtype").and_then(|v| v.as_str()) == Some("error") {
                    let error_msg = response_obj.get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown error");
                    Err(ClaudeError::Protocol(error_msg.to_string()))
                } else {
                    Ok(response_obj.clone())
                };

                // Send response (ignore if receiver dropped)
                let _ = tx.send(result);

                #[cfg(feature = "tracing-support")]
                tracing::debug!(request_id = %request_id, "Control response routed to pending request");
                #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
                eprintln!("Control response routed for request: {}", request_id);
            } else {
                #[cfg(feature = "tracing-support")]
                tracing::warn!(request_id = %request_id, "Received control_response for unknown request_id");
                #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
                eprintln!("Warning: Received control_response for unknown request_id: {}", request_id);
            }
        }
    }
    continue;
}
```

**Test:** Run `cargo check`

---

### Task 8: Update send_initialize Function

**File:** `src/client/mod.rs`
**Location:** Lines 886-922 (`send_initialize` function)
**Action:** Replace sleep with proper response waiting

**Before:**
```rust
async fn send_initialize(
    &mut self,
    hooks_config: HashMap<HookEvent, Vec<HookMatcherConfig>>,
) -> Result<()> {
    // Generate unique request ID
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);
    let counter = REQUEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let request_id = format!("req_{}_{:x}", counter, nanos);

    // Build initialize request
    let init_request = serde_json::json!({
        "type": "control_request",
        "request_id": request_id,
        "request": {
            "subtype": "initialize",
            "hooks": hooks_config
        }
    });

    // Send initialization request
    let message_json = format!("{}\n", serde_json::to_string(&init_request)?);
    let mut transport = self.transport.lock().await;
    transport.write(&message_json).await?;
    drop(transport);

    // Wait a bit for the CLI to process initialization
    // TODO: Wait for actual init response instead of sleeping
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    Ok(())
}
```

**After:**
```rust
async fn send_initialize(
    &mut self,
    hooks_config: HashMap<HookEvent, Vec<HookMatcherConfig>>,
) -> Result<serde_json::Value> {
    // Generate unique request ID
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);
    let counter = REQUEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let request_id = format!("req_{}_{:x}", counter, nanos);

    // Create oneshot channel for response
    let (tx, rx) = oneshot::channel();

    // Register pending response
    {
        let mut pending = self.pending_control_responses.lock().await;
        pending.insert(request_id.clone(), tx);
    }

    // Build initialize request
    let init_request = serde_json::json!({
        "type": "control_request",
        "request_id": request_id,
        "request": {
            "subtype": "initialize",
            "hooks": hooks_config
        }
    });

    // Send initialization request
    let message_json = format!("{}\n", serde_json::to_string(&init_request)?);
    let mut transport = self.transport.lock().await;
    transport.write(&message_json).await?;
    drop(transport);

    #[cfg(feature = "tracing-support")]
    tracing::debug!(request_id = %request_id, "Sent initialize request, waiting for response");
    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
    eprintln!("Sent initialize request: {}, waiting for response...", request_id);

    // Wait for response with timeout
    let response = tokio::time::timeout(
        tokio::time::Duration::from_secs(60),
        rx
    )
    .await
    .map_err(|_| {
        // Remove from pending on timeout
        let pending_clone = self.pending_control_responses.clone();
        tokio::spawn(async move {
            pending_clone.lock().await.remove(&request_id);
        });
        ClaudeError::Protocol("Initialize request timed out after 60 seconds".to_string())
    })?
    .map_err(|_| ClaudeError::Protocol("Initialize response channel closed unexpectedly".to_string()))??;

    #[cfg(feature = "tracing-support")]
    tracing::debug!("Initialize response received successfully");
    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
    eprintln!("Initialize response received successfully");

    Ok(response)
}
```

**Test:** Run `cargo check`

---

### Task 9: Update send_initialize Call Site

**File:** `src/client/mod.rs`
**Location:** Lines 407-409 (in `ClaudeSDKClient::new()`)
**Action:** Handle the returned response value

**Before:**
```rust
if let Some(hooks_config) = _hooks_for_init {
    client.send_initialize(hooks_config).await?;
}
```

**After:**
```rust
if let Some(hooks_config) = _hooks_for_init {
    let _init_response = client.send_initialize(hooks_config).await?;
    #[cfg(feature = "tracing-support")]
    tracing::info!("Control protocol initialization complete");
    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
    eprintln!("Control protocol initialization complete");
}
```

**Test:** Run `cargo check` - should pass all checks

---

### Task 10: Test With Example

**File:** `examples/mcp_integration_demo.rs`
**Location:** Entire file (no changes needed)
**Action:** Run the example and verify output

**Command:**
```bash
cargo run --example mcp_integration_demo
```

**Expected Output (Success):**
```
=== SDK MCP Integration Demo ===

Testing: Calculate 15 + 27

Creating client...
Client created successfully
Sending message...
Sent initialize request: req_1_..., waiting for response...
Initialize response received successfully
Control protocol initialization complete
Message sent, waiting for response...
MCP method called: initialize for server: calc
MCP message processed for calc
MCP method called: tools/list for server: calc
MCP message processed for calc
Claude calling tool: mcp__calc__add
  [SDK TOOL] add(15, 27) = 42
Claude: The result of 15 + 27 is 42.
Conversation completed successfully

✓ Integration test complete!
```

**Key Indicators of Success:**
- ✓ "Initialize response received successfully" appears
- ✓ "tools/list" is called (not just "initialize")
- ✓ Tool is actually invoked: "[SDK TOOL] add(15, 27) = 42"
- ✓ Claude gives correct answer instead of saying tool is unavailable

**If Test Fails:**
- Check cargo output for compilation errors
- Review each task implementation
- Add more debug output to trace the issue
- Verify the CLI version is 2.0+

---

### Task 11: Write Integration Test

**File:** `tests/control_response_test.rs` (create new file)
**Action:** Create integration test for control response handling

**Content:**
```rust
//! Integration test for control response handling
//!
//! Verifies that the SDK properly waits for and processes control responses
//! from the CLI, specifically for the initialize protocol.

use claude_agent_sdk::mcp::{SdkMcpServer, SdkMcpTool, ToolResult};
use claude_agent_sdk::types::{ClaudeAgentOptions, McpServerConfig, McpServers, SdkMcpServerMarker, ToolName};
use claude_agent_sdk::ClaudeSDKClient;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn test_control_response_handling() {
    // Create a simple test tool
    let test_tool = SdkMcpTool::new(
        "test",
        "Test tool",
        json!({"type": "object"}),
        |_input| {
            Box::pin(async move {
                Ok(ToolResult::text("test response"))
            })
        },
    );

    // Create SDK MCP server
    let server = SdkMcpServer::new("test-server")
        .version("1.0.0")
        .tool(test_tool);

    let server_arc = Arc::new(server);

    // Configure options with SDK server
    let mut mcp_servers = HashMap::new();
    mcp_servers.insert(
        "test".to_string(),
        McpServerConfig::Sdk(SdkMcpServerMarker {
            name: "test".to_string(),
            instance: server_arc,
        }),
    );

    let options = ClaudeAgentOptions {
        mcp_servers: McpServers::Dict(mcp_servers),
        allowed_tools: vec![ToolName::new("mcp__test__test")],
        max_turns: Some(1),
        ..Default::default()
    };

    // Create client - this should trigger initialization and wait for response
    let result = ClaudeSDKClient::new(options, None).await;

    // Verify client creation succeeded (which means initialize completed)
    assert!(
        result.is_ok(),
        "Client creation should succeed with proper control response handling"
    );

    // If we get here, the initialize protocol completed successfully
    println!("✓ Control response handling test passed");
}

#[tokio::test]
async fn test_initialize_timeout() {
    // This test would require mocking the CLI to not send a response
    // For now, we just document the expected behavior:
    // - If CLI doesn't respond within 60 seconds, initialize should return an error
    // - The error should mention "timeout"

    println!("✓ Initialize timeout behavior documented");
}
```

**Run Test:**
```bash
cargo test --test control_response_test
```

**Expected Output:**
```
running 2 tests
test test_control_response_handling ... ok
test test_initialize_timeout ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

---

## Verification Checklist

After completing all tasks, verify:

- [ ] Code compiles without errors: `cargo check`
- [ ] Code compiles without warnings: `cargo clippy`
- [ ] Example runs successfully: `cargo run --example mcp_integration_demo`
- [ ] Example shows "tools/list" being called
- [ ] Claude successfully invokes the tool
- [ ] Integration test passes: `cargo test --test control_response_test`
- [ ] No "TODO: Wait for actual init response" comment remains
- [ ] Debug output shows "Initialize response received successfully"

---

## Rollback Plan

If the fix causes issues:

1. **Revert changes:**
   ```bash
   git checkout src/client/mod.rs
   git checkout examples/mcp_integration_demo.rs
   rm tests/control_response_test.rs
   ```

2. **The problem will return:** Tools won't be available to Claude

3. **Alternative approach:** Investigate if there's a different initialization protocol needed

---

## Success Metrics

The fix is successful when:

1. ✓ Initialize response is received and processed
2. ✓ CLI calls `tools/list` on SDK MCP servers
3. ✓ Claude receives tool definitions
4. ✓ Claude can successfully invoke SDK MCP tools
5. ✓ Demo output shows tool execution: `[SDK TOOL] add(15, 27) = 42`
6. ✓ Claude responds with the correct answer instead of "tool not available"

---

## Related Files

- `ISSUES.md` - Problem description and root cause analysis
- `FIX_PLAN.md` - Detailed architectural solution
- `src/control/protocol.rs` - Control protocol message definitions
- `src/mcp/server.rs` - SDK MCP server implementation
- `src/mcp/protocol.rs` - MCP JSONRPC protocol definitions
