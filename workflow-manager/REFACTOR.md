# Refactoring Plan: research_agent.rs to Library + Binary

## Overview
Refactor `src/bin/research_agent.rs` into a library-based architecture for better modularity, testability, and reusability.

## Goals
- Separate business logic from CLI orchestration
- Enable direct library usage from TUI or other Rust code
- Improve code organization and maintainability
- Enable unit testing of individual phases
- Allow parallel compilation of modules

## Target Structure

```
workflow-manager/
├── src/
│   ├── lib.rs                          # Library root
│   ├── research/
│   │   ├── mod.rs                      # Module exports
│   │   ├── types.rs                    # Data structures
│   │   ├── cli.rs                      # CLI argument parsing
│   │   ├── phase0_analyze.rs           # Codebase analysis
│   │   ├── phase1_prompts.rs           # Prompt generation
│   │   ├── phase2_research.rs          # Research execution
│   │   ├── phase3_validate.rs          # YAML validation
│   │   ├── phase4_synthesize.rs        # Documentation synthesis
│   │   └── workflow.rs                 # Workflow orchestration
│   └── bin/
│       └── research_agent.rs           # Thin CLI wrapper
└── tests/
    └── research/
        ├── test_phase0.rs
        ├── test_phase1.rs
        └── ...
```

## Detailed Module Breakdown

### 1. `src/lib.rs`
**Purpose:** Library root that exposes the research module

**Contents:**
```rust
pub mod research;
```

### 2. `src/research/mod.rs`
**Purpose:** Module organization and re-exports

**Contents:**
```rust
pub mod types;
pub mod cli;
pub mod phase0_analyze;
pub mod phase1_prompts;
pub mod phase2_research;
pub mod phase3_validate;
pub mod phase4_synthesize;
pub mod workflow;

// Re-export commonly used types
pub use types::{CodebaseAnalysis, PromptsData, ResearchResult, WorkflowConfig};
pub use workflow::run_research_workflow;
```

### 3. `src/research/types.rs`
**Purpose:** All data structures and type definitions

**Contents:**
- `CodebaseAnalysis` struct
- `PromptsData` struct
- `ResearchPrompt` struct
- `ResearchResult` struct
- `WorkflowConfig` struct (new - consolidates CLI args)
- Serde derive macros for all types

**Lines to extract:** 61-103 (current research_agent.rs)

### 4. `src/research/cli.rs`
**Purpose:** CLI argument parsing with clap

**Contents:**
- `Args` struct with clap derives
- Helper methods for Args (e.g., `parse_phases()`)
- Conversion: `impl From<Args> for WorkflowConfig`

**Lines to extract:** 105-201 (current research_agent.rs)

### 5. `src/research/phase0_analyze.rs`
**Purpose:** Codebase analysis functionality

**Public API:**
```rust
pub async fn analyze_codebase(codebase_path: &Path) -> anyhow::Result<CodebaseAnalysis>
```

**Contents:**
- `analyze_codebase()` function
- Helper functions for file analysis
- Dependencies: `tokio::fs`, `serde_yaml`

**Lines to extract:** 203-297 (current research_agent.rs)

### 6. `src/research/phase1_prompts.rs`
**Purpose:** Research prompt generation

**Public API:**
```rust
pub async fn generate_prompts(
    analysis: &CodebaseAnalysis,
    objective: &str,
) -> anyhow::Result<PromptsData>
```

**Contents:**
- `generate_prompts()` function
- Claude agent setup for prompt generation
- Dependencies: `claude_agent_sdk`, `workflow_manager_sdk`

**Lines to extract:** 299-410 (current research_agent.rs)

### 7. `src/research/phase2_research.rs`
**Purpose:** Research execution with parallel agents

**Public API:**
```rust
pub async fn execute_research(
    prompts_data: &PromptsData,
    batch_size: usize,
) -> anyhow::Result<Vec<ResearchResult>>

pub async fn execute_single_research(
    prompt: &ResearchPrompt,
    research_number: usize,
    prefix: Option<&str>,
) -> anyhow::Result<ResearchResult>
```

**Contents:**
- `execute_research()` function (orchestrates parallel execution)
- `execute_single_research()` function (single agent execution)
- Agent configuration and streaming logic
- Dependencies: `claude_agent_sdk`, `tokio`, `futures`

**Lines to extract:** 412-619 (current research_agent.rs)

### 8. `src/research/phase3_validate.rs`
**Purpose:** YAML validation and fixing

**Public API:**
```rust
pub async fn validate_yaml_file(file_path: &str) -> anyhow::Result<()>

pub fn extract_yaml(text: &str) -> String
```

**Contents:**
- `validate_yaml_file()` function
- `extract_yaml()` helper function
- Claude agent for YAML fixing
- Dependencies: `serde_yaml`, `claude_agent_sdk`

**Lines to extract:** 621-810, 916-955 (current research_agent.rs)

### 9. `src/research/phase4_synthesize.rs`
**Purpose:** Documentation synthesis with file-condenser subagent

**Public API:**
```rust
pub async fn synthesize_documentation(
    results_file: &Path,
    output_path: &Path,
) -> anyhow::Result<()>
```

**Contents:**
- `synthesize_documentation()` function
- File-condenser subagent definition
- Agent-driven strategy for large files
- Dependencies: `claude_agent_sdk`, `workflow_manager_sdk`

**Lines to extract:** 813-914 (current research_agent.rs)

### 10. `src/research/workflow.rs`
**Purpose:** High-level workflow orchestration

**Public API:**
```rust
pub struct WorkflowConfig {
    pub objective: String,
    pub phases: Vec<u32>,
    pub batch_size: usize,
    pub dir: Option<String>,
    pub analysis_file: Option<String>,
    pub prompts_file: Option<String>,
    pub results_file: Option<String>,
    pub output: Option<String>,
}

pub async fn run_research_workflow(config: WorkflowConfig) -> anyhow::Result<()>
```

**Contents:**
- `WorkflowConfig` struct (consolidates all config)
- `run_research_workflow()` function (main orchestration)
- Phase execution logic with proper file path tracking
- Directory creation and timestamp generation
- Dependencies: All phase modules

**Lines to extract:** 957-1352 (main function logic, current research_agent.rs)

### 11. `src/bin/research_agent.rs`
**Purpose:** Thin CLI wrapper

**Contents:**
```rust
use clap::Parser;
use workflow_manager::research::{cli::Args, workflow::run_research_workflow};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = args.into();
    run_research_workflow(config).await
}
```

**Size:** ~15 lines (down from 1300+)

## Implementation Steps

### Phase 1: Setup (Low Risk)
1. Create `src/lib.rs` with `pub mod research;`
2. Create `src/research/` directory
3. Create empty module files with TODOs
4. Create `src/research/mod.rs` with module declarations
5. **Verify:** `cargo build` still works

### Phase 2: Extract Types (Low Risk)
1. Create `src/research/types.rs`
2. Copy structs: `CodebaseAnalysis`, `PromptsData`, `ResearchPrompt`, `ResearchResult`
3. Add `pub` visibility to all structs and fields
4. In `research_agent.rs`, add: `use workflow_manager::research::types::*;`
5. Remove struct definitions from `research_agent.rs`
6. **Verify:** `cargo build --bin research_agent` works
7. **Verify:** `cargo run --bin research_agent -- --help` works

### Phase 3: Extract CLI (Low Risk)
1. Create `src/research/cli.rs`
2. Copy `Args` struct with clap derives
3. In `research_agent.rs`, change to: `use workflow_manager::research::cli::Args;`
4. Remove `Args` from `research_agent.rs`
5. **Verify:** CLI parsing still works
6. **Test:** `cargo run --bin research_agent -- --objective "test" --phases 0`

### Phase 4: Extract Phase 0 (Medium Risk)
1. Create `src/research/phase0_analyze.rs`
2. Copy `analyze_codebase()` function
3. Make it `pub async fn`
4. Update imports in the module
5. In `research_agent.rs`, import and use the function
6. Remove original function from `research_agent.rs`
7. **Verify:** Phase 0 still executes correctly
8. **Test:** Run with `--phases 0 --dir .`

### Phase 5: Extract Phase 1 (Medium Risk)
1. Create `src/research/phase1_prompts.rs`
2. Copy `generate_prompts()` function
3. Make it `pub async fn`
4. Update imports
5. In `research_agent.rs`, import and use
6. Remove from `research_agent.rs`
7. **Verify:** Phase 1 executes
8. **Test:** Run with `--phases 0,1 --objective "test"`

### Phase 6: Extract Phase 2 (High Risk - Complex)
1. Create `src/research/phase2_research.rs`
2. Copy both `execute_research()` and `execute_single_research()`
3. Make both `pub async fn`
4. Ensure all dependencies are imported
5. In `research_agent.rs`, import and use
6. Remove from `research_agent.rs`
7. **Verify:** Phase 2 executes with parallel agents
8. **Test:** Run full workflow through Phase 2

### Phase 7: Extract Phase 3 (Medium Risk)
1. Create `src/research/phase3_validate.rs`
2. Copy `validate_yaml_file()` and `extract_yaml()`
3. Make them `pub async fn` and `pub fn`
4. Update imports
5. In `research_agent.rs`, import and use
6. Remove from `research_agent.rs`
7. **Verify:** Phase 3 validation works
8. **Test:** Run with invalid YAML to test fixing

### Phase 8: Extract Phase 4 (Medium Risk)
1. Create `src/research/phase4_synthesize.rs`
2. Copy `synthesize_documentation()` with subagent logic
3. Make it `pub async fn`
4. Update imports (especially `AgentDefinition`)
5. In `research_agent.rs`, import and use
6. Remove from `research_agent.rs`
7. **Verify:** Phase 4 synthesis works
8. **Test:** Run full workflow 0-4

### Phase 9: Extract Workflow Orchestration (High Risk)
1. Create `src/research/workflow.rs`
2. Create `WorkflowConfig` struct with all configuration
3. Copy main() orchestration logic to `run_research_workflow()`
4. Make state tracking more explicit (file paths, etc.)
5. Add proper error handling and logging
6. **Verify:** Full workflow still executes
7. **Test:** All phase combinations work

### Phase 10: Simplify Binary (Low Risk)
1. Rewrite `src/bin/research_agent.rs` as thin wrapper
2. Import from library modules
3. Convert `Args` to `WorkflowConfig`
4. Call `run_research_workflow()`
5. **Verify:** Binary still works identically
6. **Test:** All CLI options and phase combinations

### Phase 11: Update Module Exports (Low Risk)
1. Add re-exports to `src/research/mod.rs`
2. Simplify imports throughout
3. Add module-level documentation
4. **Verify:** Public API is clean and intuitive

### Phase 12: Add Tests (Low Risk, High Value)
1. Create `tests/research/` directory
2. Add integration tests for each phase
3. Add unit tests for helpers (e.g., `extract_yaml`)
4. **Verify:** `cargo test` passes

## Testing Strategy

### During Refactoring
After each phase extraction:
1. `cargo build` - ensure compilation
2. `cargo build --bin research_agent` - ensure binary builds
3. `cargo run --bin research_agent -- --help` - verify CLI
4. Run specific phases to verify functionality
5. Check that logs/output match previous behavior

### Final Testing
1. Run full workflow: `--phases 0,1,2,3,4 --objective "..."`
2. Run individual phases with file inputs
3. Test error cases (missing files, invalid input)
4. Verify output files are identical to pre-refactor
5. Test from TUI if integrated

### Regression Prevention
1. Keep a "known good" output from current implementation
2. Compare outputs after refactoring
3. Use `diff` on generated files to verify identical behavior

## Dependencies to Watch

### Import Changes
- All phase modules need `workflow_manager_sdk` macros
- Agent modules need `claude_agent_sdk`
- Async modules need `tokio`, `futures`
- Validation needs `serde_yaml`

### Visibility Changes
- All structs need `pub` visibility
- All functions need `pub` visibility
- Struct fields may need `pub` (consider builder pattern)

### Module Paths
- Update all `use` statements in binary
- May need `use super::*` in some modules
- Watch for circular dependencies

## Potential Issues

### Issue 1: State Management
**Problem:** Current `main()` uses mutable local variables to track state between phases
**Solution:**
- Make `WorkflowConfig` track intermediate file paths
- Use `Option<PathBuf>` for file paths that may or may not exist
- Pass explicit state between phase functions

### Issue 2: Logging Macros
**Problem:** `workflow_manager_sdk` macros may not work across module boundaries
**Solution:**
- Ensure macros are properly imported in each module
- May need to re-export macros from `mod.rs`

### Issue 3: Circular Dependencies
**Problem:** Workflow depends on phases, phases may depend on types
**Solution:**
- Keep dependency flow: types ← phases ← workflow
- No phase should import workflow
- No phase should import other phases

### Issue 4: Testing with Claude API
**Problem:** Tests may trigger real API calls
**Solution:**
- Add feature flag for mock agents
- Consider environment variable to skip API tests
- Document that integration tests need API key

## Post-Refactoring Benefits

1. **TUI Integration:** Can directly call library functions
   ```rust
   use workflow_manager::research::run_research_workflow;
   let config = WorkflowConfig { ... };
   run_research_workflow(config).await?;
   ```

2. **Unit Testing:** Test individual phases in isolation
   ```rust
   #[tokio::test]
   async fn test_phase0_analyze() {
       let result = analyze_codebase(Path::new("./test_data")).await;
       assert!(result.is_ok());
   }
   ```

3. **Reusability:** Other binaries can use research functionality
   ```rust
   // New binary: src/bin/quick_research.rs
   use workflow_manager::research::phase2_research::execute_single_research;
   ```

4. **Maintainability:** Each phase is isolated and self-contained

5. **Documentation:** Can document public API with rustdoc

## Success Criteria

- [ ] All compilation succeeds without warnings
- [ ] Binary CLI works identically to before
- [ ] All phase combinations execute correctly
- [ ] Output files are identical to pre-refactor
- [ ] Code size in binary < 50 lines
- [ ] Library modules are under 300 lines each
- [ ] Public API is documented
- [ ] Integration tests exist for each phase

## Timeline Estimate

- Setup + Types + CLI: 1 hour
- Phase 0-1 extraction: 1 hour
- Phase 2 extraction: 2 hours (complex parallelism)
- Phase 3-4 extraction: 2 hours
- Workflow extraction: 2 hours
- Testing + Documentation: 2 hours
- **Total: ~10 hours**

## Rollback Plan

If issues arise:
1. Git branch for refactoring
2. Keep original `research_agent.rs` until verified
3. Can revert entire refactoring with `git reset`
4. Or cherry-pick successful extractions
