# Refactoring Context - Ready for Phase 3

**Project**: Workflow Manager TUI (Rust + Ratatui)
**Goal**: Refactor main.rs (originally 4042 lines) into manageable modules
**Strategy**: Incremental 8-phase refactoring (see REFACTOR.md)
**Current Phase**: Ready to start Phase 3

---

## ✅ Completed Phases

### Phase 1: Extract Data Models ✅ (Commit: 4bc33af)
- **Created**: `src/models.rs` (187 lines)
- **Extracted**: All structs/enums
  - WorkflowHistory, PhaseStatus, TaskStatus, AgentStatus
  - WorkflowAgent, WorkflowTask, WorkflowPhase
  - WorkflowTab, View, App
- **Result**: main.rs 4042 → 3886 lines

### Phase 2: Extract UI Rendering ✅ (Commit: 0b7d730)
- **Created**: `src/ui.rs` (1387 lines)
- **Extracted**: 15 render functions
  - Main: `ui()`, `centered_rect()`
  - Views: `render_workflow_list/detail/edit/running`
  - Components: `render_header/footer/dropdown/file_browser`
  - Tabs: `render_tab_bar/empty_tabs/close_confirmation/tab_content`
  - Chat: `render_chat()`
- **Result**: main.rs 3886 → 2521 lines (-38% total)

**Bug Fixes in Phase 2:**
1. Agent message color: DarkGray → Gray (visibility)
2. Discovery: Added deps/ fallback for `cargo run`
3. Claude SDK: Restored unconditional `--verbose` flag

---

## 📂 Current File Structure

```
workflow-manager/src/
├── main.rs                   (2521 lines) ← TARGET FOR PHASE 3
├── models.rs                 (187 lines) - Data structures
├── ui.rs                     (1387 lines) - UI rendering
├── chat.rs                   (232 lines) - AI chat interface
├── runtime.rs                (279 lines) - Process-based runtime
├── mcp_tools.rs              (229 lines) - MCP workflow tools
├── discovery.rs              (212 lines) - Workflow discovery
└── bin/
    ├── research_agent.rs     - Multi-phase research workflow
    ├── demo_multiphase.rs    - Demo workflow
    ├── hooks_demo.rs         - Hooks demo
    ├── simple_echo.rs        - Simple workflow
    └── simple_query.rs       - Query workflow
```

---

## 🎯 Phase 3: Extract App Methods

### Goal
Move entire `impl App { ... }` block (~1500 lines) from main.rs to app.rs

### Files to Modify
1. **Create**: `src/app.rs` (new file)
2. **Modify**: `src/main.rs` (remove impl block)
3. **Modify**: `src/models.rs` (App struct stays here)

### What to Extract (51 methods from impl App)

**Core Methods:**
- `new()` - Constructor
- `save_session()`, `restore_session()` - Persistence

**Navigation:**
- `next()`, `previous()` - List navigation
- `next_tab()`, `previous_tab()` - Tab navigation
- `navigate_workflow_down/up()` - Running workflow nav
- `navigate_tab_down/up()` - Tab content navigation

**Tab Management:**
- `close_current_tab()`, `close_tab_confirmed()`
- `kill_current_tab()`, `rerun_current_tab()`
- `poll_all_tabs()` - Update running tabs

**Workflow Operations:**
- `view_workflow()`, `back_to_list()`
- `edit_workflow()`, `edit_current_tab()`
- `launch_workflow()`, `launch_workflow_in_tab()`
- `handle_workflow_event()` - Static method

**Edit Mode:**
- `start_editing_field()`, `save_edited_field()`, `cancel_editing()`

**File Browser:**
- `open_file_browser()`, `close_file_browser()`
- `load_file_browser_items()`
- `file_browser_next/previous/select()`
- `complete_path()`

**Dropdown:**
- `dropdown_next/previous/select()`, `close_dropdown()`
- `show_history_dropdown()`

**History:**
- `load_latest_values_from_history()`, `save_to_history()`

**Expansion/Collapse:**
- `toggle_selected_item()`, `toggle_expand_all()`
- `toggle_expand_phases/tasks/agents()`
- `toggle_tab_item()`, `toggle_tab_expand_all()`

**Scrolling:**
- `update_workflow_scroll()`
- `scroll_agent_messages_up/down()`

**Chat:**
- `open_chat()`

### Implementation Steps

1. **Create app.rs with proper imports:**
```rust
//! Application logic and state management

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use workflow_manager_sdk::{WorkflowLog, WorkflowRuntime, WorkflowStatus};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

use crate::models::*;
use crate::runtime::ProcessBasedRuntime;
use crate::chat::ChatInterface;
```

2. **Copy entire `impl App { ... }` block from main.rs to app.rs**

3. **Update main.rs:**
   - Add `mod app;` after other mod declarations
   - Remove the `impl App` block (keep only the App struct reference)
   - The App struct definition stays in models.rs

4. **Compile and fix any visibility issues**

5. **Verify line count**: main.rs should be ~1000 lines

### Expected Result
- main.rs: 2521 → ~1000 lines
- app.rs: ~1500 lines (new)
- All functionality working identically

---

## 🔧 Build & Test Commands

### Build
```bash
cargo build --release
```

### Test
```bash
# Run unit tests
cargo test

# Manual testing (requires real terminal)
cargo run --release

# Test workflow discovery
cargo test test_discover_workflows -- --nocapture
```

### Manual Test Checklist (Phase 3)
```
[ ] App launches
[ ] Workflow list displays
[ ] Navigation works (arrows)
[ ] View detail (Enter)
[ ] Edit workflow (E)
[ ] File browser (/)
[ ] History dropdown (Tab)
[ ] Launch workflow (new tab)
[ ] Tab navigation (Tab/Shift+Tab)
[ ] Close tab (C or Ctrl+W)
[ ] Running workflow displays
[ ] Expand/collapse (Space)
[ ] Chat opens (A)
[ ] Session persistence (quit/restart)
```

### Verification
```bash
# Check line counts
wc -l src/main.rs src/app.rs src/models.rs src/ui.rs

# Should see approximately:
# 1000 main.rs
# 1500 app.rs
# 187 models.rs
# 1387 ui.rs
```

---

## 📋 Important Context

### Project Structure
- **Workflow Manager**: TUI app for managing/executing workflows
- **Built-in workflows**: Compiled from `src/bin/*.rs` to target/{debug,release}/
- **Discovery**: Finds workflows in executable's directory + ~/.workflow-manager/workflows/
- **Runtime**: Process-based execution with stdout/stderr capture
- **MCP Integration**: AI chat can control workflows via MCP tools

### Key Technologies
- **UI**: Ratatui (TUI framework) + Crossterm (terminal backend)
- **Async**: Tokio runtime for async operations
- **AI**: Claude SDK for chat integration
- **IPC**: MCP (Model Context Protocol) for workflow tools

### Recent Bug Fixes (Phase 2)
1. **Agent messages visibility**: Use `Color::Gray` not `Color::DarkGray`
2. **Discovery in cargo run**: Check parent dir when in deps/
3. **Claude SDK verbose**: Always add `--verbose` with `stream-json` output

### Known Issues
- Pre-existing warnings (unused imports, dead code) - not related to refactoring
- Main.rs still has ~50 lines of standalone utility functions (will be Phase 4)

---

## 🚨 Common Pitfalls

### 1. Visibility Issues
**Problem**: Methods/fields not accessible after moving to new module
**Solution**: Make fields `pub` in models.rs, methods public in app.rs if needed by main.rs

### 2. Import Cycles
**Problem**: Circular dependencies between modules
**Solution**: Keep data models in models.rs, logic in app.rs, UI in ui.rs

### 3. Async Context
**Problem**: Some App methods use `self.tokio_runtime.block_on()`
**Solution**: Keep these calls intact, they work fine

### 4. Mutable Borrows
**Problem**: Rust borrow checker issues with &mut self
**Solution**: If compilation fails, check borrow scopes carefully

---

## 📖 Git History (Recent)

```
f97c2ce docs: Update REFACTOR.md - Phase 2 complete
0b7d730 refactor(phase2): Extract UI rendering to ui.rs + critical bug fixes
9d1466e docs: Update REFACTOR.md - Phase 1 complete
4bc33af refactor(phase1): Extract data models to models.rs
5f1510f docs: Add comprehensive refactoring plan for main.rs split
648367a Fix TUI JSON pollution and implement AI chat integration
```

---

## 🎯 Success Criteria for Phase 3

### Must Have
- ✅ Compiles without errors
- ✅ main.rs reduced to ~1000 lines
- ✅ app.rs created with ~1500 lines
- ✅ All 51 App methods moved
- ✅ No functionality changes

### Must Pass
- ✅ `cargo build --release` succeeds
- ✅ `cargo test` passes
- ✅ App launches and displays workflows
- ✅ Basic navigation works
- ✅ Workflow execution works
- ✅ Tab management works
- ✅ No visual regressions

### Commit When Done
```bash
git add workflow-manager/src/main.rs workflow-manager/src/app.rs
git commit -m "refactor(phase3): Extract App implementation to app.rs

Phase 3 of main.rs refactoring (Minimal Split - Part 3)

## Changes:
- Created src/app.rs (1500 lines)
- Extracted entire impl App block (51 methods)
- Reduced main.rs from 2521 to ~1000 lines (-60% from start)

## Build Status:
✅ Compiles successfully
✅ All tests passing
✅ All functionality verified

## Progress:
Phase 3/8 complete - App methods extracted
Next: Phase 4 - Extract utilities

Related: workflow-manager/REFACTOR.md

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"

git push
```

---

## 📞 Contact / Issues

- **Refactoring Plan**: See `REFACTOR.md` for full 8-phase plan
- **Project Root**: `/home/molaco/Documents/japanese/workflow-manager`
- **Main Branch**: `main` (already up to date)
- **Testing**: All changes should be tested before committing

---

## 🔄 Recovery Commands

If something goes wrong:

```bash
# See current changes
git status
git diff

# Undo uncommitted changes
git restore src/main.rs src/app.rs

# Rollback last commit
git reset --hard HEAD~1

# Return to known good state
git checkout 0b7d730  # Phase 2 complete
```

---

## 💡 Quick Start for Phase 3

```bash
cd /home/molaco/Documents/japanese/workflow-manager

# 1. Read this file
cat CONTEXT.md

# 2. Read the refactoring plan
cat REFACTOR.md | grep -A 50 "Phase 3"

# 3. Check current state
wc -l src/main.rs  # Should be 2521
git log --oneline | head -5

# 4. Start Phase 3
# Create src/app.rs
# Copy impl App block from main.rs
# Update imports
# Compile and test
# Commit when working
```

**Ready to start Phase 3!** 🚀
