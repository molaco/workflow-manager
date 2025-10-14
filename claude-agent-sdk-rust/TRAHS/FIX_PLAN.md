# Fix Plan: Control Response Handling

## Problem Summary

The Rust SDK ignores `control_response` messages from the CLI, preventing proper initialization of SDK MCP servers. This causes Claude to not receive the list of available tools.

## Solution Architecture

### Python SDK Approach

The Python SDK's `Query` class owns:
- `pending_control_responses: dict[str, anyio.Event]` - Maps request_id → Event for signaling
- `pending_control_results: dict[str, dict | Exception]` - Maps request_id → actual response data

When sending a control request:
1. Generate unique request_id
2. Register Event in `pending_control_responses[request_id]`
3. Send request with request_id
4. Wait on Event (with timeout)
5. Retrieve result from `pending_control_results[request_id]`

When receiving a control response:
1. Extract request_id from response
2. Store response in `pending_control_results[request_id]`
3. Signal Event in `pending_control_responses[request_id]`

### Rust SDK Adaptation

Use `tokio::sync::oneshot` channels for one-time response delivery:
- `pending_control_responses: Arc<Mutex<HashMap<String, oneshot::Sender<Result<serde_json::Value>>>>>`

When sending a control request:
1. Generate unique request_id
2. Create `oneshot::channel()`
3. Store sender in `pending_control_responses[request_id]`
4. Send request with request_id
5. Wait on receiver (with timeout)

When receiving a control response:
1. Extract request_id from response
2. Remove sender from `pending_control_responses[request_id]`
3. Send response through oneshot channel
4. Channel automatically wakes waiting task

## Code Changes Required

### 1. Add Field to `ClaudeSDKClient` Structure

**File:** `src/client/mod.rs` ~line 185

**Before:**
```rust
pub struct ClaudeSDKClient {
    transport: Arc<Mutex<SubprocessTransport>>,
    protocol: Arc<Mutex<ProtocolHandler>>,
    message_rx: mpsc::UnboundedReceiver<Result<Message>>,
    control_tx: mpsc::UnboundedSender<ControlRequest>,
    // ... other fields
}
```

**After:**
```rust
pub struct ClaudeSDKClient {
    transport: Arc<Mutex<SubprocessTransport>>,
    protocol: Arc<Mutex<ProtocolHandler>>,
    message_rx: mpsc::UnboundedReceiver<Result<Message>>,
    control_tx: mpsc::UnboundedSender<ControlRequest>,
    /// Pending control response channels (for init, etc.)
    pending_control_responses: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<Result<serde_json::Value>>>>>,
    // ... other fields
}
```

### 2. Initialize Pending Responses Map

**File:** `src/client/mod.rs` ~line 220-410 in `ClaudeSDKClient::new()`

**Add after line 265:**
```rust
// Create pending control responses map
let pending_control_responses = Arc::new(Mutex::new(HashMap::new()));
```

**Pass to message_reader_task (around line 292):**
```rust
let pending_responses_clone = pending_control_responses.clone();

tokio::spawn(async move {
    Self::message_reader_task(
        transport_clone,
        protocol_clone,
        message_tx,
        pending_responses_clone,  // ← Add this parameter
    )
    .await;
});
```

**Store in struct (around line 392):**
```rust
let mut client = Self {
    transport,
    protocol,
    message_rx,
    control_tx,
    pending_control_responses,  // ← Add this field
    hook_rx: None,
    permission_rx,
    hook_manager,
    permission_manager,
    sdk_mcp_servers,
    mcp_callback_tx,
};
```

### 3. Modify `message_reader_task` Signature and Implementation

**File:** `src/client/mod.rs` ~line 414-440

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
    pending_control_responses: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<Result<serde_json::Value>>>>>,
) {
```

**Replace control_response handling (lines 432-440):**

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
            }
        }
    }
    continue;
}
```

### 4. Modify `send_initialize()` to Wait for Response

**File:** `src/client/mod.rs` ~line 886-922

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
) -> Result<serde_json::Value> {  // ← Return the response
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
    let (tx, rx) = tokio::sync::oneshot::channel();

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

    // Wait for response with timeout
    let response = tokio::time::timeout(
        tokio::time::Duration::from_secs(60),
        rx
    )
    .await
    .map_err(|_| ClaudeError::Protocol("Initialize request timed out".to_string()))?
    .map_err(|_| ClaudeError::Protocol("Initialize response channel closed".to_string()))??;

    #[cfg(feature = "tracing-support")]
    tracing::debug!("Initialize response received successfully");
    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
    eprintln!("Initialize response received successfully");

    Ok(response)
}
```

### 5. Update Call Site

**File:** `src/client/mod.rs` ~line 407-409

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
    // Response contains CLI capabilities and confirmation
    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
    eprintln!("Initialization complete");
}
```

## Required Imports

Add to the top of `src/client/mod.rs`:
```rust
use tokio::sync::oneshot;
use std::collections::HashMap;
```

## Testing

After implementing these changes:

1. Run the demo:
   ```bash
   cargo run --example mcp_integration_demo
   ```

2. Expected output should now show:
   ```
   MCP method called: initialize for server: calc
   Initialize response received successfully
   MCP method called: tools/list for server: calc
   MCP message processed for calc
   Claude calling tool: mcp__calc__add
     [SDK TOOL] add(15, 27) = 42
   Claude: The result of 15 + 27 is 42.
   ```

3. The key difference: `tools/list` should now be called, and Claude should successfully use the tool.

## Summary

The fix adds proper control response handling by:
1. Storing pending response channels in a shared map
2. Routing control responses to the correct waiting task
3. Waiting for the actual initialize response instead of sleeping
4. Properly handling timeouts and errors

This matches the Python SDK's architecture and ensures the CLI completes its initialization protocol, including fetching tools from SDK MCP servers.
