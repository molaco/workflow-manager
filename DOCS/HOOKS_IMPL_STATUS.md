# Hooks Implementation Status

## Completed Phases

### ✅ Phase 1: CLI Mode Selection (subprocess.rs)
**Lines Modified**: 143-320

**Changes**:
- Added `needs_control_protocol` detection based on hooks/permissions config
- CLI now uses `--input-format stream-json` when hooks are configured
- CLI uses `--print` mode when no hooks/permissions
- Updated prompt handling to skip passing prompt as arg in control protocol mode

**Status**: ✅ Complete and compiles successfully

---

### ✅ Phase 2: Callback ID Generation (hooks/mod.rs)
**Lines Modified**: 6, 13-20, 25-94

**Changes**:
- Added `HashMap<String, (usize, usize)>` callback_id_map to HookManager
- Added `next_callback_id: u32` counter
- Implemented `register_with_ids()` method to generate callback IDs
- Implemented `invoke_by_id()` method to invoke hooks by callback ID
- Updated constructor to initialize new fields

**Status**: ✅ Complete and compiles successfully

---

### ✅ Phase 3: Initialization Request Types (control/protocol.rs)
**Lines Modified**: 191-214, 314-345

**Changes**:
- Added `hooks: Option<HashMap<HookEvent, Vec<HookMatcherConfig>>>` to InitRequest
- Created new `HookMatcherConfig` struct with matcher pattern and hook_callback_ids
- Added `create_init_request_with_hooks()` method
- Updated `create_init_request()` to include hooks: None

**Status**: ✅ Complete and compiles successfully

---

### ✅ Phase 4: Hook Callback Request Handling (control/protocol.rs)
**Lines Modified**: 108-154, 156-210, 280-328, 417-460, 490-513

**Changes**:
- Added `HookCallbackResponse` variant to ControlRequest enum
- Added `HookCallback` variant to ControlResponse enum with callback_id, input, tool_use_id
- Added `hook_callback_tx` channel field to ProtocolHandler
- Added `set_hook_callback_channel()` method
- Updated `get_request_id()` to handle HookCallbackResponse
- Updated `handle_response()` to route HookCallback responses to channel
- Added `create_hook_callback_response()` method

**Status**: ✅ Complete and compiles successfully

---

## Remaining Phases (NOT YET IMPLEMENTED)

### ⏳ Phase 5: Client Initialization with Hook Registration
**File**: src/client/mod.rs
**Lines to Modify**: ~212-310

**Required Changes**:
1. Replace `manager.register()` calls with `manager.register_with_ids()` in the hooks setup (lines 217-223)
2. Collect generated callback IDs and build HashMap<HookEvent, Vec<HookMatcherConfig>>
3. Set up hook_callback_tx channel before spawning tasks
4. Call protocol.set_hook_callback_channel()
5. Spawn hook_callback_handler_task (new task to add)
6. Send initialization request with hooks config if hooks are present
7. Wait for init response before marking client as ready

**Key Code Pattern**:
```rust
let mut hooks_for_init = HashMap::new();
for (event, matchers) in hooks_config {
    let mut matcher_configs = Vec::new();
    for matcher in matchers {
        let callback_ids = manager.register_with_ids(matcher.clone());
        matcher_configs.push(HookMatcherConfig {
            matcher: matcher.matcher.clone(),
            hook_callback_ids: callback_ids,
        });
    }
    hooks_for_init.insert(*event, matcher_configs);
}
```

---

### ⏳ Phase 6: Hook Callback Handler Task
**File**: src/client/mod.rs
**Lines to Add**: New async method (~50 lines)

**Required Changes**:
1. Add new `hook_callback_handler_task()` method
2. Receive messages from hook_callback_rx channel
3. Extract (request_id, callback_id, input, tool_use_id) from channel
4. Invoke hook using `manager.invoke_by_id(callback_id, ...)`
5. Create hook callback response using protocol handler
6. Send response via control_tx channel
7. Handle errors appropriately

**Key Code Pattern**:
```rust
async fn hook_callback_handler_task(
    manager: Arc<Mutex<HookManager>>,
    protocol: Arc<Mutex<ProtocolHandler>>,
    control_tx: mpsc::UnboundedSender<ControlRequest>,
    mut hook_callback_rx: mpsc::UnboundedReceiver<(RequestId, String, serde_json::Value, Option<String>)>,
) {
    while let Some((request_id, callback_id, input, tool_use_id)) = hook_callback_rx.recv().await {
        // Invoke hook and send response
    }
}
```

---

### ⏳ Phase 7: Message Reader Updates
**File**: src/client/mod.rs
**Lines to Modify**: ~320-382 (message_reader_task)

**Required Changes**:
1. Update message parsing to detect HookCallback responses
2. Route HookCallback messages to hook_callback_tx channel
3. Ensure ControlResponse::HookCallback pattern is matched
4. Extract callback_id, input, tool_use_id and forward to channel

**Key Code Pattern**:
```rust
match control_msg {
    ControlResponse::HookCallback { id, callback_id, input, tool_use_id } => {
        if let Some(tx) = protocol_guard.hook_callback_tx.clone() {
            let _ = tx.send((id, callback_id, input, tool_use_id));
        }
    }
    // ... other response types
}
```

---

### ⏳ Phase 8: Control Writer Updates
**File**: src/client/mod.rs
**Lines to Modify**: ~384-424 (control_writer_task)

**Required Changes**:
1. Ensure control_writer_task properly serializes ControlRequest messages
2. Handle HookCallbackResponse variant in serialization
3. Write serialized messages to transport stdin
4. Add error handling for write failures

**Status**: May already be correct, but needs verification

---

## Testing Requirements

After completing all phases, the following tests should pass:

1. **Hook registration generates unique callback IDs**
2. **Initialization request includes hooks configuration**
3. **CLI spawns in stream-json mode when hooks are configured**
4. **Hook callback requests are routed to the handler**
5. **Hook handler invokes correct callback and sends response**
6. **Full integration test with actual CLI interaction**

---

## Current Build Status

✅ All completed phases compile successfully with `cargo check`

## Next Steps

1. Complete Phase 5: Update client initialization
2. Add Phase 6: Hook callback handler task
3. Update Phase 7: Message reader routing
4. Verify Phase 8: Control writer works correctly
5. Run integration tests
6. Update documentation

---

## Estimated Remaining Work

- **Phase 5**: ~30 lines of code changes
- **Phase 6**: ~50 lines of new code
- **Phase 7**: ~20 lines of code changes
- **Phase 8**: ~10 lines verification/updates

**Total**: ~110 lines of code + testing

---

## References

- Implementation Plan: IMPL.md
- Python SDK Reference: Lines 107-145, 258-272 in query.py
- Current Files Modified:
  - src/transport/subprocess.rs
  - src/hooks/mod.rs
  - src/control/protocol.rs
