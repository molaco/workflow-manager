# SDK MCP Integration Issue

## Symptom

When using SDK MCP servers (in-process MCP servers), Claude reports that tools are not available:

```
MCP method called: initialize for server: calc
MCP message processed for calc
Claude calling tool: mcp__calc__add
Claude: The add tool is not available in this environment.
```

The CLI:
- ✓ Connects to SDK MCP server (`initialize` method called)
- ✓ Routes tool invocations via control protocol
- ✗ **Never fetches tools** (`tools/list` never called)
- ✗ **Never passes tools to Claude Messages API**

## Root Cause

The Rust SDK ignores `control_response` messages from the CLI.

### Current Implementation (Broken)

**File:** `src/client/mod.rs:432-440`

```rust
// Handle control_response (init response, etc.)
if msg_type == Some("control_response") {
    // This is a response to our init request - just log and continue
    #[cfg(feature = "tracing-support")]
    tracing::debug!("Received control_response");
    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
    eprintln!("Received control_response");
    continue;  // ← PROBLEM: Discards the response!
}
```

**File:** `src/client/mod.rs:917-919`

```rust
// Wait a bit for the CLI to process initialization
// TODO: Wait for actual init response instead of sleeping
tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
```

### How Python SDK Does It (Working)

**File:** `claude-agent-sdk-python/src/claude_agent_sdk/_internal/query.py:164-176`

```python
if msg_type == "control_response":
    response = message.get("response", {})
    request_id = response.get("request_id")
    if request_id in self.pending_control_responses:
        event = self.pending_control_responses[request_id]
        if response.get("subtype") == "error":
            self.pending_control_results[request_id] = Exception(...)
        else:
            self.pending_control_results[request_id] = response
        event.set()  # Wakes up waiting initialize() call
    continue
```

**File:** `claude-agent-sdk-python/src/claude_agent_sdk/_internal/query.py:317-355`

```python
async def _send_control_request(self, request: dict[str, Any]) -> dict[str, Any]:
    """Send control request to CLI and wait for response."""
    # Generate unique request ID
    request_id = f"req_{self._request_counter}_{os.urandom(4).hex()}"

    # Create event for response
    event = anyio.Event()
    self.pending_control_responses[request_id] = event

    # Build and send request
    control_request = {
        "type": "control_request",
        "request_id": request_id,
        "request": request,
    }

    await self.transport.write(json.dumps(control_request) + "\n")

    # Wait for response with timeout
    with anyio.fail_after(60.0):
        await event.wait()

    result = self.pending_control_results.pop(request_id)
    # ... return response data
```

## Protocol Flow

### What Should Happen (Python SDK)

```
SDK                          CLI
 |                            |
 |--- Init Request ---------->|
 |    request_id: "req_123"   |
 |                            |
 |<-- Init Response -----------|
 |    request_id: "req_123"   |
 |    (contains config)       |
 |                            |
 | SDK processes response     |
 | CLI fetches MCP tools      |
 |                            |
 |--- User Message ---------->|
 |                            |
 | Claude gets tools list     |
 | Claude calls mcp__calc__add|
 |                            |
 |<-- MCP Message Request ----|
 |    tools/call: add         |
 |                            |
 |--- MCP Message Response -->|
 |    result: 42              |
```

### What Actually Happens (Rust SDK)

```
SDK                          CLI
 |                            |
 |--- Init Request ---------->|
 |    request_id: "req_123"   |
 |                            |
 | Sleep 100ms (ignore resp)  |
 |                            |
 |<-- Init Response -----------|
 |    request_id: "req_123"   | ← DISCARDED!
 |    (contains config)       |
 |                            |
 | SDK thinks init done       |
 | CLI incomplete init        | ← Never fetches tools!
 |                            |
 |--- User Message ---------->|
 |                            |
 | Claude has NO tools        | ← Tools never provided
 | Claude says tool not avail |
```

## Fix Required

1. **Store pending control requests** with request IDs
2. **Match control responses** to pending requests
3. **Wait for response** instead of sleeping
4. **Return response data** from `send_initialize()`

### Changes Needed

**`src/client/mod.rs`:**

1. Add fields to `ClaudeSDKClient`:
   ```rust
   pending_control_responses: Arc<Mutex<HashMap<String, oneshot::Sender<serde_json::Value>>>>,
   ```

2. In message handler (line 432-440), **store** control responses:
   ```rust
   if msg_type == Some("control_response") {
       let request_id = response.get("request_id").and_then(|v| v.as_str());
       if let Some(request_id) = request_id {
           if let Some(tx) = pending_responses.lock().await.remove(request_id) {
               let _ = tx.send(response);
           }
       }
       continue;
   }
   ```

3. In `send_initialize()` (line 886-922), **wait for response**:
   ```rust
   // Create oneshot channel for response
   let (tx, rx) = oneshot::channel();
   self.pending_control_responses.lock().await.insert(request_id.clone(), tx);

   // Send request
   transport.write(&message_json).await?;
   drop(transport);

   // Wait for response with timeout
   let response = tokio::time::timeout(
       Duration::from_secs(60),
       rx
   ).await??;

   // Return or process response
   Ok(())
   ```

## Verification

After fix, the output should show:

```
MCP method called: tools/list for server: calc
MCP message processed for calc
Claude calling tool: mcp__calc__add
  [SDK TOOL] add(15, 27) = 42
Claude: The result of 15 + 27 is 42.
```

## References

- Python SDK working example: `claude-agent-sdk-python/examples/mcp_calculator.py`
- Rust SDK broken example: `examples/mcp_integration_demo.rs`
- TODO comment: `src/client/mod.rs:918`
