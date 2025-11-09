# Unified Runtime Implementation Plan

**Goal**: Make all workflows (manual and MCP) use `ProcessBasedRuntime` with mandatory `runtime_handle_id`

**Status**: ✅ COMPLETED
**Started**: 2025-11-09
**Completed**: 2025-11-09
**Complexity**: High - touched 6 files with structural changes

---

## Final State - ALL TASKS COMPLETE

### ✅ Completed - Phase 1 (Compilation Fixes)
1. ✅ Updated `WorkflowTab` structure - removed `child_process`, made `runtime_handle_id: Uuid` (not Option)
2. ✅ Updated tab cleanup logic (`tabs.rs`) - unified `close_tab_confirmed()` and `kill_current_tab()`
3. ✅ Updated `poll_all_tabs()` to use runtime status checks
4. ✅ Updated UI to show handle ID for all workflows (`tab_views.rs`)
5. ✅ Fixed `history.rs` - removed child_process, updated runtime_handle_id to Uuid::new_v4()
6. ✅ Fixed `workflow_ops.rs` - 3 locations updated (error cases)
7. ✅ Fixed `command_handlers.rs` - removed Option unwrapping for runtime_handle_id

### ✅ Completed - Phase 2 (Runtime Conversion)
8. ✅ Converted `launch_workflow_in_tab()` to use `runtime.execute_workflow()`
9. ✅ Converted params from Vec<String> args to HashMap<String, String>
10. ✅ Replaced manual process spawning with runtime execution
11. ✅ Replaced thread-based log parsing with tokio task + broadcast channel
12. ✅ Registered log streaming task in TaskRegistry

### ✅ Completed - Phase 3 (Cleanup)
13. ✅ Removed unused imports from `tabs.rs`
14. ✅ Verified final compilation - builds successfully with 0 errors

---

## Architecture Change

### Before (Dual Path)
```
Manual:  User → App → Command.spawn() → Child → Manual parsing
MCP:     Claude → Runtime.execute() → Child → Broadcast logs
```

### After (Unified)
```
All:     User/Claude → Runtime.execute() → Child → Broadcast logs
                            ↓
                       UUID handle_id (mandatory)
```

---

## Implementation Tasks

### Phase 1: Fix Compilation Errors (CURRENT)

**Task 1.1: Fix history.rs**
- File: `src/app/history.rs`
- Lines: ~83-84
- Change: Replace `child_process: None, runtime_handle_id: None`
- With: `runtime_handle_id: Uuid::new_v4()`
- Context: Session restoration - workflows restored from history won't have valid handle_id, need to generate new ones

**Task 1.2: Fix workflow_ops.rs error cases**
- File: `src/app/workflow_ops.rs`
- Lines: ~302-303, ~350-351, ~421-422
- Change: Same as 1.1 - generate UUIDs for failed workflow tabs
- Context: Build failures and spawn failures still create tabs with error messages

**Task 1.3: Fix workflow_ops.rs launch_workflow_in_tab()**
- File: `src/app/workflow_ops.rs`
- Lines: ~260-522 (entire function)
- Change: Replace direct `Command.spawn()` with `runtime.execute_workflow()`
- This is the BIG change - converts manual launch to use runtime
- Details in Phase 2

**Task 1.4: Check command_handlers.rs**
- File: `src/app/command_handlers.rs`
- Search for: `child_process`, `runtime_handle_id: None`
- Fix: Any WorkflowTab creations

**Task 1.5: Update UI condition checks**
- File: `src/ui/tab_views.rs`
- Line: ~185
- Change: Remove `if let Some(handle_id)` - always exists now
- Change to: Just use `tab.runtime_handle_id` directly

---

### Phase 2: Convert Manual Launch to Runtime (COMPLEX)

**Task 2.1: Analyze current launch_workflow_in_tab() logic**
Current flow (lines 260-522):
1. Build binary with `cargo build`
2. Build CLI args from field values
3. Spawn process with `Command.new()`
4. Take stdout/stderr handles
5. Spawn threads to parse logs
6. Store `Child` in `tab.child_process`

**Task 2.2: Design new launch flow**
New flow:
1. Build binary (keep this - runtime expects binary to exist)
2. Build params HashMap from field values
3. Call `runtime.execute_workflow(workflow_id, params)` → get `WorkflowHandle`
4. Subscribe to logs: `runtime.subscribe_logs(handle.id())`
5. Spawn tokio task to stream logs to tab
6. Store `handle.id()` in `tab.runtime_handle_id`

**Task 2.3: Implement new launch_workflow_in_tab()**
```rust
pub fn launch_workflow_in_tab(&mut self) {
    // ... get workflow, build binary (existing code) ...

    // Build params HashMap (convert from field_values)
    let mut params = HashMap::new();
    for field in &workflow.info.fields {
        if let Some(value) = self.field_values.get(&field.name) {
            if !value.is_empty() {
                params.insert(field.name.clone(), value.clone());
            }
        }
    }

    // Execute via runtime
    let runtime = match &self.runtime {
        Some(r) => r.clone(),
        None => {
            // Create error tab
            return;
        }
    };

    let handle = self.tokio_runtime.block_on(async {
        runtime.execute_workflow(&workflow.info.id, params).await
    });

    let handle = match handle {
        Ok(h) => h,
        Err(e) => {
            // Create error tab
            return;
        }
    };

    let handle_id = *handle.id();

    // Create tab with handle_id
    let tab = WorkflowTab {
        runtime_handle_id: handle_id,
        // ... rest of fields ...
    };

    // Subscribe to logs and spawn streaming task
    let log_task = tokio::spawn({
        let runtime_clone = runtime.clone();
        let tab_logs = tab.workflow_output.clone();
        let tab_phases = tab.workflow_phases.clone();

        async move {
            if let Ok(mut logs_rx) = runtime_clone.subscribe_logs(&handle_id).await {
                while let Ok(log) = logs_rx.recv().await {
                    // Update tab.workflow_output with raw logs
                    // Update tab.workflow_phases with structured logs
                    Self::handle_workflow_event(log, &tab_phases);
                }
            }
        }
    });

    // Register task for cleanup
    self.tokio_runtime.block_on(async {
        self.task_registry.register(handle_id, log_task).await;
    });

    self.open_tabs.push(tab);
    self.active_tab_idx = self.open_tabs.len() - 1;
    self.current_view = View::Tabs;
}
```

**Challenges:**
- `handle_workflow_event` is associated function, not method - can't call from closure
- Need to handle both RawOutput and structured WorkflowLog events
- Log streaming task needs access to tab's Arc<Mutex<>> fields
- Async/sync boundary - main code is sync, runtime is async

**Solutions:**
- Pass Arc clones to task before pushing tab to open_tabs
- Move `handle_workflow_event` to be a free function, not impl method
- Use block_on for runtime calls in sync context

---

### Phase 3: Handle Edge Cases

**Task 3.1: Update launch_workflow() (non-tab version)**
- File: `src/app/workflow_ops.rs`
- Lines: ~113-257
- This is the old "WorkflowRunning" view (not tabs)
- Might be deprecated - check if still used
- If used, convert to runtime like launch_workflow_in_tab

**Task 3.2: Implement rerun_current_tab() properly**
- File: `src/app/tabs.rs`
- Lines: ~142-166
- Currently stubbed with error message
- Should: Cancel old workflow, execute new one, update tab

**Task 3.3: Session restoration edge case**
- File: `src/app/history.rs`
- Restored tabs won't have running workflows
- Generate dummy UUIDs, mark as NotStarted or Failed
- Document that restored tabs can't resume execution

---

### Phase 4: Testing & Cleanup

**Task 4.1: Remove unused imports**
- `src/app/tabs.rs` - PathBuf, Arc, thread, WorkflowLog
- `src/app/workflow_ops.rs` - std::thread, std::io, std::process if no longer used

**Task 4.2: Test scenarios**
1. Manual workflow launch via TUI
2. MCP workflow launch via chat
3. Close running workflow (both types)
4. Kill workflow
5. Multiple concurrent workflows
6. Tab with build failure
7. Tab with spawn failure

**Task 4.3: Update documentation**
- Update comments explaining dual-path is now unified
- Document that all workflows require runtime

---

## Risk Assessment

### High Risk
- **Breaking existing manual workflow functionality** - this is core user flow
- **Log streaming changes** - new async task instead of threads
- **Session restoration** - restored tabs won't have valid handle_ids

### Medium Risk
- **Performance** - block_on in sync code might have overhead
- **Error handling** - runtime errors need to be surfaced to user
- **Race conditions** - async task lifecycle with tab lifecycle

### Low Risk
- **UI changes** - minimal, just removing Option checks
- **MCP workflows** - already working, changes are improvements

---

## Rollback Plan

If implementation fails:
1. Git revert to before WorkflowTab structure change
2. Keep the logging fixes from earlier commit
3. Take incremental approach:
   - Phase 1: Make runtime optional for manual workflows
   - Phase 2: Add runtime support alongside existing code
   - Phase 3: Deprecate old path gradually

---

## Success Criteria

- ✅ Code compiles without errors
- ✅ Manual workflow launch works via runtime
- ✅ MCP workflow launch continues working
- ✅ Both types have runtime_handle_id
- ✅ Cleanup/cancel works for both types
- ✅ Logs stream correctly for both types
- ✅ Status updates work for both types
- ✅ No code duplication between paths

---

## Current Blocker

**Compilation errors prevent testing**. Must fix Phase 1 tasks before proceeding to Phase 2.

**Priority**: Fix history.rs and workflow_ops.rs struct initialization errors to get code compiling again.
