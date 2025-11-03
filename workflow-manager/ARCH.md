# Architecture Analysis: Workflow Execution Paths

## Current State: Two Different Execution Paths

### Manual Launch Path (TUI)
**Location**: `src/app/workflow_ops.rs::launch_workflow()`

```
User clicks "Launch"
↓
App creates WorkflowTab with fresh Arcs (workflow_output, workflow_phases)
↓
App.launch_workflow() spawns process with:
  - stdin: Stdio::null() ✓
  - stdout/stderr: Stdio::piped()
↓
OS threads (std::thread::spawn) read pipes:
  - Direct writes to tab.workflow_output
  - Direct writes to tab.workflow_phases
  - No shared HashMap
  - No lock contention
  - Fast pipe draining ✓
↓
Tab immediately shows logs
```

### MCP Launch Path (Chat)
**Location**: `src/runtime.rs::ProcessBasedRuntime::execute_workflow()`

```
Claude calls execute_workflow MCP tool
↓
ProcessBasedRuntime.execute_workflow() spawns process with:
  - stdin: NOT SET ✗ (inherits from parent)
  - stdout/stderr: Stdio::piped()
↓
Async tokio tasks read pipes:
  - Lock HashMap<Uuid, ExecutionState> on EVERY line ✗
  - Both stdout and stderr compete for same lock ✗
  - Broadcast to channel
  - Write to buffer
  - Slow pipe draining ✗
↓
Returns WorkflowHandle to Claude
↓
MCP tool sends CreateTab command to App
↓
MCP tool spawns background task to stream logs
↓
Background task sends AppendTabLog commands
↓
Tab shows logs (eventually)
```

## Critical Differences

| Aspect | Manual Launch | MCP Launch |
|--------|--------------|------------|
| **stdin** | `Stdio::null()` ✓ | Not set (inherited) ✗ |
| **Threading** | OS threads | Async tokio tasks |
| **Shared state** | None | `Arc<Mutex<HashMap>>` |
| **Lock frequency** | Per output write | Per log line read ✗ |
| **Parallelism** | True parallel | Cooperative async |
| **Pipe draining** | Fast ✓ | Slow (lock contention) ✗ |
| **Code location** | `app/workflow_ops.rs` | `runtime.rs` |

## Why MCP Workflows Get Stuck

**The Perfect Storm**:

1. Workflow writes logs to stderr pipe
2. Pipe buffer fills up (4-64KB)
3. Workflow blocks waiting to write more logs
4. Async parser holds `executions` lock while processing
5. Other parser waits for same lock
6. Pipes not drained fast enough
7. **Deadlock**: Workflow waiting to write, parsers fighting over lock

**Manual launch doesn't have this issue** because OS threads drain pipes as fast as the OS can schedule them, with no lock contention between readers.

## Why Can't They Use The Same Code?

### Architectural Barriers

1. **Timing Mismatch**:
   - Manual: Tab exists BEFORE workflow starts
   - MCP: Tab created AFTER workflow already running

2. **Separation of Concerns**:
   - `ProcessBasedRuntime` in `runtime.rs` - no knowledge of TUI/App/Tabs
   - Manual launch in `app/workflow_ops.rs` - full access to App state
   - Runtime is designed to be TUI-agnostic (could be used from CLI, API, etc.)

3. **No Shared Arcs**:
   - Manual: Direct references to `tab.workflow_output` and `tab.workflow_phases`
   - MCP: Runtime creates its own state, tab created separately
   - No way to pass tab Arcs to runtime at spawn time

## Recommended Architecture: Unified Runtime Path

### The Vision

**Make manual launch use ProcessBasedRuntime too** - unify on ONE execution path.

```
┌─────────────────────────────────────────────────┐
│                     App (TUI)                    │
│                                                  │
│  Manual Launch:                                  │
│    1. User clicks "Launch"                       │
│    2. Call runtime.execute_workflow()            │
│    3. Send CreateTab command                     │
│    4. Spawn task to stream logs                  │
│    5. Send AppendTabLog commands                 │
│                                                  │
│  MCP Launch:                                     │
│    1. Claude calls execute_workflow              │
│    2. Call runtime.execute_workflow()            │
│    3. Send CreateTab command                     │
│    4. Spawn task to stream logs                  │
│    5. Send AppendTabLog commands                 │
│                                                  │
│  ▲ SAME PATH FOR BOTH!                          │
└──┼──────────────────────────────────────────────┘
   │
   │ runtime.execute_workflow()
   │ runtime.subscribe_logs()
   │ runtime.get_logs()
   │
┌──▼──────────────────────────────────────────────┐
│         ProcessBasedRuntime (Fixed)              │
│                                                  │
│  - Spawns with stdin=null ✓                     │
│  - Uses OS threads (not async) ✓                │
│  - Fast pipe draining ✓                         │
│  - Persistent log buffer ✓                      │
│  - No lock contention ✓                         │
│  - TUI-agnostic ✓                               │
└──────────────────────────────────────────────────┘
```

### Why This Is Best

#### 1. Single Source of Truth
- Only ONE place where workflows are spawned
- Only ONE place to fix bugs
- Only ONE place to add features
- Manual and MCP behave **identically**

#### 2. Maintains Separation of Concerns
- Runtime stays generic (no TUI dependencies)
- Could still be used from CLI, API server, tests
- Clean layering: Runtime → Commands → TUI

#### 3. Proper Architecture
- Runtime does what it should: manage processes
- TUI does what it should: present data
- Communication via well-defined channels

#### 4. Fixes All Current Issues
Once runtime is fixed:
- ✅ stdin properly configured
- ✅ Fast log streaming (OS threads)
- ✅ No lock contention (clone what you need, release lock)
- ✅ Historical log buffer works
- ✅ Both paths get all fixes automatically

#### 5. Future-Proof
- Want to add webhooks? Runtime already has the logs
- Want CLI tool? Use runtime directly
- Want REST API? Use runtime
- Want to add features like pause/resume? Add to runtime once

## Implementation Plan

### Phase 1: Fix Runtime Bugs (DO THIS FIRST)

**Priority: HIGH | Risk: LOW | Effort: LOW**

Fix the immediate bugs in `ProcessBasedRuntime`:

```rust
// 1. Fix stdin
cmd.stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

// 2. Fix locking - clone what you need, drop lock immediately
let (logs_tx, logs_buffer) = {
    let execs = executions.lock().unwrap();
    let state = execs.get(&exec_id)?;
    (state.logs_tx.clone(), state.logs_buffer.clone())
}; // ← DROP LOCK HERE

// Now process without holding lock
let _ = logs_tx.send(log.clone());
logs_buffer.lock().unwrap().push(log);

// 3. Fix async → use OS threads for pipe reading
std::thread::spawn(move || {
    parse_stderr_blocking(...)  // No async, just blocking I/O
});
```

**Benefits**:
- MCP workflows work correctly
- Immediate value
- Low risk (isolated changes)
- Can be done incrementally

### Phase 2: Migrate Manual Launch (LATER)

**Priority: MEDIUM | Risk: MEDIUM | Effort: MEDIUM**

After runtime works correctly, migrate manual launch to use it:

```rust
// OLD: app/workflow_ops.rs has complex spawn logic (200+ lines)
pub fn launch_workflow(&mut self, idx: usize) {
    // Complex process spawning
    // Thread management
    // Pipe reading
    // ...
}

// NEW: app/workflow_ops.rs delegates to runtime
pub fn launch_workflow(&mut self, idx: usize) {
    // Execute via runtime
    let handle = self.runtime.execute_workflow(id, params)?;

    // Create tab via command
    self.handle_command(AppCommand::CreateTab {
        workflow_id: id,
        params,
        handle_id: *handle.id(),
    });

    // Spawn log streamer (same as MCP path)
    spawn_log_streamer(
        *handle.id(),
        self.command_tx.clone(),
        self.task_registry.clone()
    );
}
```

**Benefits**:
- Single execution path
- Easier maintenance
- Consistent behavior
- Cleaner architecture

**Risks**:
- Need thorough testing
- Could break manual workflows if done wrong
- Requires coordination between changes

### Phase 3: Delete Duplicate Code

**Priority: LOW | Risk: LOW | Effort: LOW**

Once migration is complete and tested:

- Remove old manual spawn logic from `workflow_ops.rs`
- Remove thread spawn code
- Remove duplicate pipe reading logic
- Keep only runtime path

**Benefits**:
- Less code to maintain
- No drift between implementations
- Clearer architecture

## Alternative: Keep Dual Paths

**If unification is too much work**, just do **Phase 1 only**:

✅ Fix runtime bugs (stdin, locking, threads)
✅ MCP workflows work correctly
✅ Manual workflows continue as-is
❌ Still maintain two spawn implementations
❌ Changes needed in two places

This is pragmatic but not ideal long-term.

## Recommendation

**Immediate (Next Sprint)**:
- ✅ Execute Phase 1: Fix runtime bugs
  - Add `stdin(Stdio::null())`
  - Fix lock contention
  - Consider switching to OS threads

**Future (When Time Permits)**:
- Consider Phase 2: Unify on runtime path
  - High value, but higher risk/effort
  - Can be done incrementally
  - Measure twice, cut once

**Reasoning**: The runtime path is fundamentally better designed, it just has implementation bugs. Fix those bugs first and you get immediate value. The unification can wait until you have confidence the runtime works perfectly.

## Open Questions

1. **Performance**: Are OS threads better than async for pipe reading in this case?
   - Likely yes: blocking I/O, no async overhead, true parallelism
   - Could benchmark both approaches

2. **Broadcast channel capacity**: Is 100 sufficient?
   - Might need to increase if workflows are very chatty
   - Or switch to unbounded

3. **Log buffer memory**: Should we limit buffer size?
   - Currently unbounded Vec
   - Long-running workflows could accumulate huge buffers
   - Could cap at N lines or N bytes

4. **Error handling**: What if parsers crash?
   - Currently silent failures
   - Should probably log and update workflow status

## Related Files

- `workflow-manager/src/runtime.rs` - ProcessBasedRuntime implementation
- `workflow-manager/src/app/workflow_ops.rs` - Manual launch logic
- `workflow-manager/src/mcp_tools.rs` - MCP tool implementations
- `workflow-manager/src/app/command_handlers.rs` - Command processing
- `workflow-manager-sdk/src/lib.rs` - WorkflowRuntime trait definition

## References

- [NEW_PLAN.md](NEW_PLAN.md) - Original MCP-TUI integration plan
- [IMPLEMENTATION_SUMMARY.md](IMPLEMENTATION_SUMMARY.md) - Implementation details
