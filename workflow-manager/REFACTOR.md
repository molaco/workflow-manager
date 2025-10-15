# Workflow Manager TUI Refactoring Plan

**Goal**: Refactor `src/main.rs` (4042 lines) into manageable, maintainable modules

**Strategy**: Incremental refactoring from Approach 4 (minimal split) → Approach 1 (domain-driven modules)

**Process**: Each phase builds on the previous, with testing and review between phases

---

## Current State

```
src/main.rs: 4042 lines
├── 10 structs/enums (data models)
├── 51 App methods (business logic)
├── 13 render functions (UI)
└── 10 standalone functions (utilities)
```

---

## Phase 0: Preparation ✓

**Goal**: Set up testing infrastructure before refactoring

### Tasks:
- [x] Create this refactoring plan
- [ ] Create test suite for critical workflows
- [ ] Document current functionality
- [ ] Create backup branch

### Testing Checklist (use for all phases):
```
Manual Tests:
[ ] App launches without errors
[ ] Workflow list displays correctly
[ ] Can navigate with arrow keys
[ ] Can view workflow detail (Enter)
[ ] Can edit workflow (E key)
[ ] File browser opens and works (/)
[ ] History dropdown works (Tab)
[ ] Can launch workflow in new tab (Enter in edit view)
[ ] Tab navigation works (Tab/Shift+Tab)
[ ] Tab close works (C or Ctrl+W)
[ ] Running workflow shows phases/tasks/agents
[ ] Can expand/collapse items (Space)
[ ] Chat opens (A key) and responds
[ ] Chat can list workflows via MCP tools
[ ] Session persistence works (quit and restart)
[ ] No panics or crashes during normal use
```

### Verification:
```bash
cargo build --release
cargo test
cargo run --release  # Manual testing
```

**Commit**: "docs: Add refactoring plan for main.rs split"

---

## Phase 1: Extract Data Models (Minimal Split - Part 1)

**Goal**: Move all data structures to `models.rs`

**Estimated Time**: 1 hour
**Risk Level**: Low ⚠️

### Files Changed:
- `src/main.rs` → remove structs/enums, add `use crate::models::*`
- `src/models.rs` → NEW (create this file)

### What Moves:
```rust
// To models.rs:
- struct WorkflowHistory
- enum PhaseStatus
- enum TaskStatus
- enum AgentStatus
- struct WorkflowAgent
- struct WorkflowTask
- struct WorkflowPhase
- struct WorkflowTab
- enum View
- struct App (just the struct definition, not impl)
```

### Implementation Steps:
1. Create `src/models.rs`
2. Copy all struct/enum definitions to models.rs
3. Make all fields `pub` where needed
4. Add `mod models;` to main.rs
5. Add `use crate::models::*;` to main.rs
6. Remove original definitions from main.rs
7. Compile and fix any visibility issues

### Tests:
- [ ] `cargo build --release` succeeds
- [ ] Run full manual test checklist
- [ ] No behavioral changes

### Verification Command:
```bash
# Before
wc -l src/main.rs  # 4042 lines

# After
wc -l src/main.rs src/models.rs  # ~3600 + ~450 lines
cargo build --release && cargo test
```

**Commit**: "refactor: Extract data models to models.rs"

---

## Phase 2: Extract UI Rendering (Minimal Split - Part 2)

**Goal**: Move all render functions to `ui.rs`

**Estimated Time**: 1.5 hours
**Risk Level**: Low ⚠️

### Files Changed:
- `src/main.rs` → remove render functions, add `use crate::ui::*`
- `src/ui.rs` → NEW (create this file)

### What Moves:
```rust
// To ui.rs:
- fn ui(f: &mut Frame, app: &App)
- fn render_header(...)
- fn render_workflow_list(...)
- fn render_workflow_detail(...)
- fn render_workflow_edit(...)
- fn render_workflow_running(...)
- fn render_footer(...)
- fn render_dropdown(...)
- fn render_file_browser(...)
- fn render_tab_bar(...)
- fn render_empty_tabs(...)
- fn render_close_confirmation(...)
- fn render_tab_content(...)
- fn render_chat(...)
- fn centered_rect(...)
```

### Implementation Steps:
1. Create `src/ui.rs`
2. Add necessary imports (ratatui types, models, etc.)
3. Copy all render functions to ui.rs
4. Make functions `pub` where needed
5. Add `mod ui;` to main.rs
6. Add `use crate::ui;` to main.rs
7. Update main.rs to call `ui::ui(...)` instead of `ui(...)`
8. Remove original functions from main.rs
9. Compile and fix any import issues

### Tests:
- [ ] `cargo build --release` succeeds
- [ ] All UI renders correctly (check each view)
- [ ] No visual regressions
- [ ] Run full manual test checklist

### Verification Command:
```bash
# After
wc -l src/main.rs src/ui.rs src/models.rs  # ~2000 + ~1500 + ~450 lines
cargo build --release && cargo test
```

**Commit**: "refactor: Extract UI rendering to ui.rs"

---

## Phase 3: Extract App Methods (Minimal Split - Part 3)

**Goal**: Move App implementation to `app.rs`

**Estimated Time**: 1.5 hours
**Risk Level**: Medium ⚠️⚠️

### Files Changed:
- `src/main.rs` → keep only App struct, remove impl block
- `src/app.rs` → NEW (create this file)
- `src/models.rs` → move App struct here

### What Moves:
```rust
// To app.rs:
- impl App { ... }  (all 51 methods)
```

### Implementation Steps:
1. Move `struct App` from main.rs to models.rs
2. Create `src/app.rs`
3. Copy entire `impl App` block to app.rs
4. Add necessary imports
5. Add `mod app;` to main.rs
6. Remove impl block from main.rs
7. Compile and fix any issues

### Tests:
- [ ] `cargo build --release` succeeds
- [ ] All app functionality works (tabs, navigation, editing)
- [ ] Run full manual test checklist
- [ ] Session save/restore works

### Verification Command:
```bash
# After
wc -l src/main.rs src/app.rs src/ui.rs src/models.rs
# Should be: ~500 + ~1500 + ~1500 + ~500 lines
cargo build --release && cargo test
```

**Commit**: "refactor: Extract App implementation to app.rs"

---

## Phase 4: Extract Utilities (Minimal Split - Part 4)

**Goal**: Move standalone utility functions to appropriate modules

**Estimated Time**: 30 minutes
**Risk Level**: Low ⚠️

### Files Changed:
- `src/main.rs` → remove utility functions
- `src/utils.rs` → NEW (create this file)

### What Moves:
```rust
// To utils.rs:
- fn history_file_path() -> PathBuf
- fn load_history() -> WorkflowHistory
- fn save_history(history: &WorkflowHistory) -> Result<()>
- fn load_workflows() -> Vec<Workflow>
- fn load_builtin_workflows() -> Vec<Workflow>
- fn load_discovered_workflows() -> Vec<Workflow>
```

### Implementation Steps:
1. Create `src/utils.rs`
2. Move utility functions to utils.rs
3. Make functions `pub` where needed
4. Add `mod utils;` to main.rs
5. Update references in main.rs
6. Compile and fix issues

### Tests:
- [ ] `cargo build --release` succeeds
- [ ] Workflow discovery works
- [ ] History persistence works
- [ ] Run manual test checklist

### Verification Command:
```bash
# After - main.rs should be ~300 lines
wc -l src/main.rs
cargo build --release && cargo test
```

**Commit**: "refactor: Extract utility functions to utils.rs"

---

## Phase 5: Split UI Module by View (Incremental - Part 1)

**Goal**: Break `ui.rs` into view-specific modules

**Estimated Time**: 2 hours
**Risk Level**: Low ⚠️

### Structure:
```
src/ui/
├── mod.rs              (~100 lines - main ui() function + re-exports)
├── header_footer.rs    (~200 lines - header, footer)
├── workflow_views.rs   (~600 lines - list, detail, edit, running)
├── tab_views.rs        (~400 lines - tab bar, tab content, empty state)
├── chat_view.rs        (~100 lines - chat rendering)
└── components.rs       (~300 lines - dropdown, file browser, popups)
```

### Implementation Steps:
1. Create `src/ui/` directory
2. Create `mod.rs` with main `ui()` function
3. Create individual view files
4. Move render functions to appropriate files
5. Update imports and re-exports
6. Update `main.rs` to use `crate::ui::ui` instead of `crate::ui`
7. Delete old `ui.rs`
8. Compile and test

### Tests:
- [ ] `cargo build --release` succeeds
- [ ] All views render correctly
- [ ] Run full manual test checklist

**Commit**: "refactor: Split ui.rs into view-specific modules"

---

## Phase 6: Split App Module by Concern (Incremental - Part 2)

**Goal**: Break `app.rs` into domain-specific modules

**Estimated Time**: 3 hours
**Risk Level**: Medium ⚠️⚠️

### Structure:
```
src/app/
├── mod.rs              (~100 lines - App struct + re-exports)
├── tabs.rs             (~400 lines - tab management)
├── navigation.rs       (~300 lines - navigation methods)
├── file_browser.rs     (~300 lines - file browser logic)
├── history.rs          (~200 lines - history/session)
├── workflow_ops.rs     (~400 lines - workflow launch/edit)
└── state.rs            (~200 lines - state management helpers)
```

### Implementation Steps:
1. Create `src/app/` directory
2. Create `mod.rs` with App struct
3. Split methods by domain into separate files
4. Use traits or impl blocks for organization
5. Update imports and visibility
6. Delete old `app.rs`
7. Compile and test

### Tests:
- [ ] `cargo build --release` succeeds
- [ ] All app methods work correctly
- [ ] Tab operations work (create, close, navigate)
- [ ] File browser works
- [ ] History persistence works
- [ ] Run full manual test checklist

**Commit**: "refactor: Split app.rs into domain modules"

---

## Phase 7: Organize Models (Incremental - Part 3)

**Goal**: Split models by domain

**Estimated Time**: 1 hour
**Risk Level**: Low ⚠️

### Structure:
```
src/models/
├── mod.rs              (~50 lines - re-exports)
├── workflow.rs         (~250 lines - Phase/Task/Agent/Status enums)
├── tab.rs              (~150 lines - WorkflowTab)
├── view.rs             (~50 lines - View enum)
└── history.rs          (~100 lines - WorkflowHistory)
```

### Implementation Steps:
1. Create `src/models/` directory
2. Split models.rs by domain
3. Update imports and re-exports
4. Delete old models.rs
5. Compile and test

### Tests:
- [ ] `cargo build --release` succeeds
- [ ] All types accessible where needed
- [ ] Run manual test checklist

**Commit**: "refactor: Organize models into domain modules"

---

## Phase 8: Final Cleanup (Incremental - Part 4)

**Goal**: Polish module structure and documentation

**Estimated Time**: 1 hour
**Risk Level**: Low ⚠️

### Tasks:
- [ ] Add module-level documentation
- [ ] Ensure consistent visibility (pub vs pub(crate))
- [ ] Remove unused imports
- [ ] Run clippy and fix warnings
- [ ] Update any outdated comments
- [ ] Verify no dead code

### Implementation Steps:
1. Add doc comments to each module
2. Run `cargo clippy --all-targets`
3. Fix all warnings
4. Run `cargo fmt`
5. Review all pub visibility
6. Test everything

### Tests:
- [ ] `cargo build --release` succeeds
- [ ] `cargo clippy` has no warnings
- [ ] `cargo test` passes
- [ ] Run full manual test checklist

**Commit**: "refactor: Final cleanup and documentation"

---

## Final Structure

```
workflow-manager/src/
├── main.rs                    (~200 lines - entry point + event loop)
├── lib.rs                     (~50 lines - library exports)
│
├── app/
│   ├── mod.rs                 (~100 lines)
│   ├── tabs.rs                (~400 lines)
│   ├── navigation.rs          (~300 lines)
│   ├── file_browser.rs        (~300 lines)
│   ├── history.rs             (~200 lines)
│   ├── workflow_ops.rs        (~400 lines)
│   └── state.rs               (~200 lines)
│
├── ui/
│   ├── mod.rs                 (~100 lines)
│   ├── header_footer.rs       (~200 lines)
│   ├── workflow_views.rs      (~600 lines)
│   ├── tab_views.rs           (~400 lines)
│   ├── chat_view.rs           (~100 lines)
│   └── components.rs          (~300 lines)
│
├── models/
│   ├── mod.rs                 (~50 lines)
│   ├── workflow.rs            (~250 lines)
│   ├── tab.rs                 (~150 lines)
│   ├── view.rs                (~50 lines)
│   └── history.rs             (~100 lines)
│
├── utils.rs                   (~200 lines)
├── chat.rs                    (~232 lines - existing)
├── runtime.rs                 (~279 lines - existing)
├── mcp_tools.rs               (~229 lines - existing)
└── discovery.rs               (~212 lines - existing)

Total files: ~22
Largest file: ~600 lines
Average file: ~250 lines
```

---

## Testing Strategy

### Automated Tests (Add During Refactoring):

```rust
// In src/models/mod.rs or tests/models_test.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_tab_creation() {
        // Test WorkflowTab initialization
    }

    #[test]
    fn test_phase_status_transitions() {
        // Test status changes
    }
}

// In src/app/tabs.rs
#[cfg(test)]
mod tests {
    #[test]
    fn test_tab_navigation() {
        // Test next_tab/previous_tab
    }

    #[test]
    fn test_tab_close() {
        // Test close_current_tab
    }
}
```

### Integration Tests:

```rust
// In tests/ui_smoke_test.rs
#[test]
fn test_app_initialization() {
    let app = App::new();
    assert!(app.workflows.len() > 0);
    assert_eq!(app.current_view, View::WorkflowList);
}

#[test]
fn test_workflow_discovery() {
    let workflows = load_workflows();
    assert!(workflows.len() >= 5); // Should find built-in workflows
}
```

---

## Rollback Plan

If any phase fails or introduces bugs:

```bash
# Check current branch
git status

# Rollback last commit
git reset --hard HEAD~1

# Or rollback to specific commit
git log --oneline
git reset --hard <commit-hash>

# Test that it works
cargo build --release
cargo run --release
```

---

## Progress Tracking

| Phase | Status | Tested | Committed | Notes |
|-------|--------|--------|-----------|-------|
| 0: Preparation | ⏳ | - | - | Creating plan |
| 1: Extract Models | ⏸️ | ⬜ | ⬜ | |
| 2: Extract UI | ⏸️ | ⬜ | ⬜ | |
| 3: Extract App Methods | ⏸️ | ⬜ | ⬜ | |
| 4: Extract Utilities | ⏸️ | ⬜ | ⬜ | |
| 5: Split UI Module | ⏸️ | ⬜ | ⬜ | |
| 6: Split App Module | ⏸️ | ⬜ | ⬜ | |
| 7: Organize Models | ⏸️ | ⬜ | ⬜ | |
| 8: Final Cleanup | ⏸️ | ⬜ | ⬜ | |

Legend: ⏸️ Not started | ⏳ In progress | ✅ Complete | ❌ Failed | ⬜ Not done | ✅ Done

---

## Risk Mitigation

1. **Compile after every change** - Don't accumulate errors
2. **Commit frequently** - Easy rollback points
3. **Test between phases** - Catch issues early
4. **Use feature branches** - Protect main branch
5. **Keep backups** - Git tags before major changes

---

## Notes

- Each phase should take 30min - 3hrs
- Total estimated time: 10-14 hours
- Can be done over multiple sessions
- Pause and commit at any phase boundary
- No phase depends on completing all previous phases perfectly
