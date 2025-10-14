# TUI Workflow Manager - ASCII Views

Collection of ASCII mockups for the Rust + Iced workflow manager TUI.

## Main Dashboard View

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Workflow Manager v0.1.0                    [Q]uit [H]elp      Rust + Iced   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│  Tasks Overview                                            6 tasks • 2 done  │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  │
│                                                                               │
│  ● 1  Add Event Data Fields to Protocol Layer                  ✓ COMPLETED  │
│       simple • 1-2 hours • 1 file affected                                   │
│                                                                               │
│  ● 2  Update Hook Channel Type Signatures                      ✓ COMPLETED  │
│       moderate • 2-3 hours • 2 files affected                                │
│                                                                               │
│  ▶ 3  Wire Event Data Through Handler Pipeline                 🔄 IN PROGRESS│
│       simple • 1 hour • 1 file affected                                      │
│       Blocks: [4, 5]                                                         │
│                                                                               │
│  ○ 4  Implement Automatic Hook Response Communication          ⏸ BLOCKED    │
│       moderate • 2-3 hours • 1 file affected                                 │
│       Waiting on: [3]                                                        │
│                                                                               │
│  ○ 5  Update Test Suite for 4-Tuple Channel Structure          ⏸ BLOCKED    │
│       moderate • 2-3 hours • 2 files affected                                │
│       Waiting on: [1, 2, 3, 4]                                               │
│                                                                               │
│  ○ 6  End-to-End Validation with Claude Code CLI               ⏸ BLOCKED    │
│       complex • half day • 1 file affected                                   │
│       Waiting on: [5]                                                        │
│                                                                               │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓] Navigate  [Enter] Details  [D] Dependencies  [F] Filter  [/] Search    │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Task Detail View

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Task #3: Wire Event Data Through Handler Pipeline              [Esc] Back   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│  STATUS    🔄 IN PROGRESS      COMPLEXITY    Simple                          │
│  EFFORT    1 hour              RISK         Low                              │
│  FILES     1 file affected     TESTS        2 required                       │
│                                                                               │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  │
│                                                                               │
│  PURPOSE                                                                      │
│  Modify the hook handler task to extract event data from the channel and     │
│  pass it to the HookManager::invoke method. This completes the data pipeline │
│  from protocol layer to user callbacks, ensuring hooks receive actual event  │
│  payloads instead of empty JSON objects.                                     │
│                                                                               │
│  MOTIVATION                                                                   │
│  The current handler implementation destructures only hook_id and event from │
│  the channel tuple, ignoring event data, and then passes an empty json!({})  │
│  to invoke(). This is the final barrier preventing user callbacks from       │
│  accessing tool parameters, session context...                               │
│                                                                               │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  │
│                                                                               │
│  DEPENDENCIES                                                                 │
│    Requires: Task #2 (Channel must carry 4-tuple)                           │
│    Enables:  Task #4 (Response communication)                                │
│                                                                               │
│  ACCEPTANCE CRITERIA                         [2/5 complete]                  │
│    ✓ hook_handler_task destructures 4-tuple                                  │
│    ✓ invoke() called with event_data                                         │
│    ○ invoke() receives tool_name as parameter                                │
│    ○ Integration test confirms tool parameters                               │
│    ○ Integration test confirms session context                               │
│                                                                               │
├─────────────────────────────────────────────────────────────────────────────┤
│ [T]ests  [C]omponents  [I]mplementation  [N]otes  [S]tart  [M]ark Done      │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Dependency Graph View

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Task Dependency Graph                                          [Esc] Back    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│                          Critical Path: 6 tasks                               │
│                                                                               │
│                                                                               │
│         ┌─────────────┐                                                      │
│         │   Task 1    │                                                      │
│         │  Protocol   │ ✓                                                    │
│         │   Fields    │                                                      │
│         └──────┬──────┘                                                      │
│                │                                                              │
│                ▼                                                              │
│         ┌─────────────┐                                                      │
│         │   Task 2    │                                                      │
│         │  Channel    │ ✓                                                    │
│         │   Types     │                                                      │
│         └──────┬──────┘                                                      │
│                │                                                              │
│                ▼                                                              │
│         ┌─────────────┐                                                      │
│         │   Task 3    │                                                      │
│         │  Handler    │ 🔄 40% ████████░░░░░░░░░░░                          │
│         │  Pipeline   │                                                      │
│         └──────┬──────┘                                                      │
│                │                                                              │
│                ▼                                                              │
│         ┌─────────────┐                                                      │
│         │   Task 4    │                                                      │
│    ┌───▶│  Response   │ ⏸                                                    │
│    │    │   Comms     │                                                      │
│    │    └──────┬──────┘                                                      │
│    │           │                                                              │
│    │           ├─────────────────┐                                           │
│    │           ▼                 ▼                                           │
│    │    ┌─────────────┐   ┌─────────────┐                                  │
│    │    │   Task 5    │   │   Task 6    │                                  │
│    └────│    Tests    │───▶│   E2E Val   │ ⏸                                │
│         │   Update    │ ⏸ │             │                                   │
│         └─────────────┘   └─────────────┘                                   │
│                                                                               │
│  Legend:  ✓ Done  🔄 In Progress  ⏸ Blocked  ○ Pending                      │
│                                                                               │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓←→] Navigate  [Enter] View Task  [C] Critical Path  [F] Filter           │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Task Progress Stats View

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Workflow Progress                                              [Esc] Back    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│  Overall Progress                                         33% ████████░░░░░  │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  │
│                                                                               │
│  ┌─────────────────────────────────────────────────┐                        │
│  │  Tasks by Status                                │                        │
│  │                                                  │                        │
│  │  ✓ Completed     2  ████████                    │                        │
│  │  🔄 In Progress   1  ████                        │                        │
│  │  ⏸ Blocked        3  ████████████                │                        │
│  │  ○ Pending        0                              │                        │
│  │                                                  │                        │
│  └─────────────────────────────────────────────────┘                        │
│                                                                               │
│  ┌─────────────────────────────────────────────────┐                        │
│  │  Complexity Distribution                        │                        │
│  │                                                  │                        │
│  │  Simple       2 tasks  ████                     │                        │
│  │  Moderate     3 tasks  ██████                   │                        │
│  │  Complex      1 task   ██                       │                        │
│  │                                                  │                        │
│  └─────────────────────────────────────────────────┘                        │
│                                                                               │
│  Time Estimates                                                               │
│    Completed:     3-5 hours                                                  │
│    Remaining:     8-10.5 hours                                               │
│    Total:         11-15.5 hours                                              │
│                                                                               │
│  Files Affected                                                               │
│    Total unique files: 3                                                     │
│    Most modified: client/mod.rs (3 tasks)                                    │
│                                                                               │
│  Risk Assessment                                                              │
│    High Risk:     1 task  (E2E Validation)                                   │
│    Medium Risk:   2 tasks (Channel Types, Response Comms)                    │
│    Low Risk:      3 tasks                                                    │
│                                                                               │
├─────────────────────────────────────────────────────────────────────────────┤
│ [R] Refresh  [E] Export  [T] Timeline  [G] Gantt                            │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Quick Actions Panel

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Quick Actions                                                  [Esc] Back    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│  Next Available Tasks                                                         │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  │
│                                                                               │
│  ▶ Task #3: Wire Event Data Through Handler Pipeline                        │
│     Status: IN PROGRESS • Effort: 1 hour • Files: 1                         │
│     [C] Continue  [V] View Details  [P] Pause                                │
│                                                                               │
│  Recent Activity                                                              │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  │
│                                                                               │
│  14:32  Task #3 started                                                      │
│  13:15  Task #2 marked complete                                              │
│  12:40  Task #2 started                                                      │
│  11:20  Task #1 marked complete                                              │
│                                                                               │
│  Workflow Commands                                                            │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  │
│                                                                               │
│  [N] Start next task       [A] Add task          [E] Edit task               │
│  [X] Export report         [I] Import workflow   [S] Save state              │
│  [T] Run tests             [B] Build            [L] View logs                │
│                                                                               │
│  Filters                                                                      │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  │
│                                                                               │
│  ☑ Show completed    ☑ Show blocked    ☐ Show pending    ☑ Show in-progress │
│                                                                               │
│  Complexity: [All] Simple Moderate Complex                                   │
│  Risk Level: [All] Low Medium High                                           │
│                                                                               │
├─────────────────────────────────────────────────────────────────────────────┤
│ [←→] Switch View  [Tab] Next Panel  [/] Search  [?] Help                    │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Workflow Logs - Hierarchical View

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Workflow Logs - Task #3: Handler Pipeline                     [Esc] Back    │
├─────────────────────────────────────────────────────────────────────────────┤
│ Workflow: task_3_handler_pipeline  Started: 14:32:15  Duration: 00:23:41    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│ 🎯 ORCHESTRATOR [main_workflow_orchestrator]                                 │
│ ├─ 14:32:15 [INFO]  Workflow started: task_3_handler_pipeline               │
│ ├─ 14:32:16 [INFO]  Initializing task dependencies                          │
│ ├─ 14:32:17 [INFO]  Spawned suborchestrator: code_analysis_orchestrator     │
│ │  │                                                                          │
│ │  └─ 🔄 SUBORCHESTRATOR [code_analysis_orchestrator]                        │
│ │     ├─ 14:32:17 [INFO]  Analyzing client/mod.rs hook_handler_task         │
│ │     ├─ 14:32:18 [INFO]  Spawned subagent: file_analyzer_001               │
│ │     │  │                                                                    │
│ │     │  └─ 🤖 SUBAGENT [file_analyzer_001]                                  │
│ │     │     ├─ 14:32:18 [INFO]  Reading file: client/mod.rs                 │
│ │     │     ├─ 14:32:19 [INFO]  Found hook_handler_task at line 342         │
│ │     │     ├─ 14:32:20 [INFO]  Analyzing destructuring pattern             │
│ │     │     └─ 14:32:21 [DONE] ✓ Analysis complete (2.8s)                   │
│ │     │                                                                       │
│ │     ├─ 14:32:22 [INFO]  Spawned subagent: dependency_checker_002          │
│ │     │  │                                                                    │
│ │     │  └─ 🤖 SUBAGENT [dependency_checker_002]                             │
│ │     │     ├─ 14:32:22 [INFO]  Checking HookManager::invoke signature      │
│ │     │     ├─ 14:32:23 [WARN] Method signature expects 3 params, found 2   │
│ │     │     └─ 14:32:24 [DONE] ✓ Check complete (1.9s)                      │
│ │     │                                                                       │
│ │     └─ 14:32:25 [DONE] ✓ Code analysis complete (7.2s)                    │
│ │                                                                             │
│ ├─ 14:32:26 [INFO]  Spawned suborchestrator: implementation_orchestrator    │
│ │  │                                                                          │
│ │  └─ 🔄 SUBORCHESTRATOR [implementation_orchestrator]                       │
│ │     ├─ 14:32:26 [INFO]  Planning code modifications                       │
│ │     ├─ 14:32:27 [INFO]  Spawned subagent: code_editor_003                 │
│ │     │  │                                                                    │
│ │     │  └─ 🤖 SUBAGENT [code_editor_003]                                    │
│ │     │     ├─ 14:32:27 [INFO]  Editing client/mod.rs:342                   │
│ │     │     ├─ 14:32:28 [INFO]  Updated destructuring to 4-tuple            │
│ │     │     ├─ 14:32:29 [INFO]  Modified invoke() call with event_data      │
│ │     │     └─ 14:32:30 [DONE] ✓ Edits applied successfully (3.1s)          │
│ │     │                                                                       │
│ │     ├─ 14:32:31 [INFO]  Spawned subagent: syntax_validator_004            │
│ │     │  │                                                                    │
│ │     │  └─ 🤖 SUBAGENT [syntax_validator_004]                               │
│ │     │     ├─ 14:32:31 [INFO]  Running cargo check                         │
│ │     │     ├─ 14:32:45 [INFO]  Compilation successful                      │
│ │     │     └─ 14:32:46 [DONE] ✓ Validation passed (14.8s)                  │
│ │     │                                                                       │
│ │     └─ 14:32:47 [DONE] ✓ Implementation complete (20.4s)                  │
│ │                                                                             │
│ └─ 14:55:56 [DONE] ✓ Workflow completed successfully (23m 41s)              │
│                                                                               │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓] Scroll  [←→] Collapse/Expand  [L] Level  [F] Filter  [E] Export        │
│ Filter: [All Levels] INFO WARN ERROR DONE  Agents: [All] ☑Orch ☑Sub ☑Agent  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Compact Log View (Multiple Workflows)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ All Workflow Logs                                              [Esc] Back    │
├─────────────────────────────────────────────────────────────────────────────┤
│ Active: 2 workflows • Completed: 4 workflows • Failed: 0                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│ ▼ 🎯 task_3_handler_pipeline [ACTIVE] 14:32:15 → now (23m 41s)              │
│   ├─ 🔄 code_analysis_orchestrator [DONE] 7.2s • 2 subagents                │
│   ├─ 🔄 implementation_orchestrator [ACTIVE] 20.4s • 2 subagents            │
│   └─ 🔄 testing_orchestrator [PENDING]                                       │
│                                                                               │
│ ▶ 🎯 task_2_channel_types [COMPLETED] 13:40:22 → 14:15:33 (35m 11s)         │
│                                                                               │
│ ▶ 🎯 task_1_protocol_fields [COMPLETED] 11:20:10 → 12:35:18 (1h 15m 8s)     │
│                                                                               │
│ ▶ 🎯 research_workflow_tui [COMPLETED] 10:15:00 → 11:05:42 (50m 42s)        │
│                                                                               │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓] Navigate  [Enter] Expand  [D] Details  [C] Clear Completed  [/] Search │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Detailed Agent Log View (Focused on Single Agent)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Agent Logs: code_editor_003 (subagent)                        [Esc] Back    │
├─────────────────────────────────────────────────────────────────────────────┤
│ Parent: implementation_orchestrator • Task: #3 Handler Pipeline              │
│ Started: 14:32:27 • Duration: 3.1s • Status: ✓ COMPLETED                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│ 14:32:27.142 [INFO]  Agent spawned by implementation_orchestrator           │
│ 14:32:27.143 [INFO]  Received task: Edit client/mod.rs hook_handler_task    │
│ 14:32:27.145 [DEBUG] Loading file: /home/user/project/client/mod.rs         │
│ 14:32:27.198 [DEBUG] File loaded: 1247 lines, 42.3 KB                       │
│ 14:32:27.199 [INFO]  Locating hook_handler_task function                    │
│ 14:32:27.342 [DEBUG] Found function at line 342                             │
│ 14:32:27.343 [INFO]  Analyzing current destructuring pattern                │
│ 14:32:27.445 [DEBUG] Current pattern: (hook_id, event)                      │
│ 14:32:27.446 [INFO]  Planning modification to 4-tuple                       │
│ 14:32:27.550 [DEBUG] New pattern: (hook_id, event, event_data, tool_name)   │
│ 14:32:28.112 [INFO]  Applying edit at line 345                              │
│ 14:32:28.223 [DEBUG] Old: let (hook_id, event) = hook_rx.recv()             │
│ 14:32:28.224 [DEBUG] New: let (hook_id, event, event_data, tool_name) = ... │
│ 14:32:28.445 [INFO]  Edit applied successfully                              │
│ 14:32:28.446 [INFO]  Updating invoke() call at line 352                     │
│ 14:32:28.678 [DEBUG] Old: manager.invoke(hook_id, event, json!({}), None)   │
│ 14:32:28.679 [DEBUG] New: manager.invoke(hook_id, event, event_data, tool_n │
│ 14:32:29.012 [INFO]  Invoke call updated successfully                       │
│ 14:32:29.013 [INFO]  Saving file modifications                              │
│ 14:32:29.156 [DEBUG] File saved: client/mod.rs                              │
│ 14:32:29.157 [INFO]  Verifying syntax correctness                           │
│ 14:32:30.245 [DEBUG] Running: cargo check --message-format=json             │
│ 14:32:30.246 [INFO]  All edits applied successfully                         │
│ 14:32:30.247 [DONE] ✓ Agent task completed (3.105s)                         │
│                                                                               │
│                                                                               │
│                                                                               │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓] Scroll  [L] Level: ALL  [T] Timestamps: ms  [C] Copy  [S] Save         │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Split View (Parallel Agents)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Parallel Agent Logs                                            [Esc] Back    │
├──────────────────────────────────┬──────────────────────────────────────────┤
│ 🤖 file_analyzer_001             │ 🤖 dependency_checker_002                │
│ Status: ✓ DONE (2.8s)            │ Status: ✓ DONE (1.9s)                    │
├──────────────────────────────────┼──────────────────────────────────────────┤
│                                  │                                          │
│ 14:32:18 [INFO] Reading file     │ 14:32:22 [INFO] Checking method sig     │
│ 14:32:19 [INFO] Found function   │ 14:32:23 [WARN] Signature mismatch      │
│ 14:32:20 [INFO] Analyzing code   │ 14:32:23 [INFO] Expected 3 params       │
│ 14:32:21 [DONE] ✓ Complete       │ 14:32:24 [DONE] ✓ Complete              │
│                                  │                                          │
│                                  │                                          │
│                                  │                                          │
│                                  │                                          │
│                                  │                                          │
│                                  │                                          │
│                                  │                                          │
│                                  │                                          │
├──────────────────────────────────┴──────────────────────────────────────────┤
│ 🔄 PARENT: code_analysis_orchestrator [DONE] 7.2s                            │
│ ├─ Both agents completed successfully                                        │
│ └─ Results aggregated and passed to parent                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│ [Tab] Switch Pane  [←→] Navigate  [F] Focus Single  [E] Expand              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Real-time Streaming Log View

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Live Logs - task_3_handler_pipeline [STREAMING]               [Esc] Back    │
├─────────────────────────────────────────────────────────────────────────────┤
│ Auto-scroll: ON • Updates: 127 • Rate: 4.2/sec • [Space] Pause  [R] Resume  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│ 14:55:42.445 [INFO]  🔄 impl_orch   Spawning validation subagent            │
│ 14:55:42.578 [INFO]  🤖 validator_005  Agent initialized                     │
│ 14:55:42.579 [INFO]  🤖 validator_005  Running test suite                    │
│ 14:55:43.123 [DEBUG] 🤖 validator_005  Executing: cargo test --lib           │
│ 14:55:45.667 [INFO]  🤖 validator_005  Test 1/8: test_hook_deserialization   │
│ 14:55:45.890 [INFO]  🤖 validator_005  ✓ Passed                              │
│ 14:55:46.012 [INFO]  🤖 validator_005  Test 2/8: test_channel_flow           │
│ 14:55:46.234 [INFO]  🤖 validator_005  ✓ Passed                              │
│ 14:55:46.456 [WARN] 🤖 validator_005  Warning: deprecated function used      │
│ 14:55:46.789 [INFO]  🤖 validator_005  Test 3/8: test_handler_invocation     │
│ 14:55:47.123 [INFO]  🤖 validator_005  ✓ Passed                              │
│ 14:55:47.345 [INFO]  🔄 impl_orch   3/8 tests passing                        │
│ 14:55:47.567 [DEBUG] 🤖 validator_005  Test output captured                  │
│ 14:55:48.123 [INFO]  🤖 validator_005  Test 4/8: test_event_data_extraction  │
│ 14:55:48.456 [INFO]  🤖 validator_005  ✓ Passed                              │
│ 14:55:48.789 [INFO]  🤖 validator_005  Test 5/8: test_tool_name_parsing      │
│ 14:55:49.012 [INFO]  🤖 validator_005  ✓ Passed                              │
│ 14:55:49.234 [INFO]  🔄 impl_orch   5/8 tests passing                        │
│ 14:55:49.567 [INFO]  🤖 validator_005  Test 6/8: test_response_sending       │
│ 14:55:49.890 [INFO]  🤖 validator_005  ✓ Passed                              │
│ 14:55:50.123 [INFO]  🤖 validator_005  Test 7/8: test_error_handling         │
│ 14:55:50.345 [ERROR] 🤖 validator_005  ✗ Failed: assertion error line 78     │
│ 14:55:50.346 [ERROR] 🤖 validator_005  Expected Ok, got Err(ChannelClosed)   │
│ 14:55:50.567 [WARN] 🔄 impl_orch   Test failure detected, analyzing...      │
│ 14:55:50.890 [INFO]  🤖 validator_005  Test 8/8: test_integration             │
│ 14:55:51.123 [INFO]  🤖 validator_005  ✓ Passed                              │
│ 14:55:51.345 [WARN] 🤖 validator_005  Summary: 7/8 passed, 1 failed          │
│ 14:55:51.567 [INFO]  🔄 impl_orch   Reporting results to parent              │
│ 14:55:51.789 [INFO]  🎯 main_orch   Received test results                    │
│ 14:55:52.012 [WARN] 🎯 main_orch   Action required: fix failing test        │
│                                                                               │
├─────────────────────────────────────────────────────────────────────────────┤
│ [↑↓] Scroll  [Space] Pause  [C] Clear  [F] Filter  [/] Search  [S] Save     │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Key Features

### Visual Elements
- **Box drawing characters** for clean borders and separators
- **Icons** for different components (🎯 orchestrator, 🔄 suborchestrator, 🤖 subagent)
- **Status indicators** (✓ ✗ 🔄 ⏸ ○ ▶)
- **Progress bars** using block characters (████░░░░)
- **Tree structures** for hierarchical logs

### Navigation
- Keyboard-driven interface (Vim-style and arrow keys)
- Multiple view modes (list, detail, graph, logs)
- Quick filters and search functionality
- Context-aware keyboard shortcuts

### Log Views
- **Hierarchical**: Tree structure showing orchestrator → suborchestrator → subagent relationships
- **Compact**: Overview of multiple workflows with expandable details
- **Detailed**: Deep dive into individual agent execution
- **Split**: Side-by-side comparison of parallel agents
- **Streaming**: Real-time logs with auto-scroll and filtering

### Data Display
- Task metadata (status, complexity, effort, risk)
- Dependency relationships (requires/enables/blocks)
- Progress tracking with percentages
- Timestamps with duration calculations
- Log levels (INFO, WARN, ERROR, DEBUG, DONE)
