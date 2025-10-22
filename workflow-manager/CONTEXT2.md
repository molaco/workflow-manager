# Research Agent Refactoring Progress

## Goal
Refactor `research_agent.rs` (~1300 lines) into modular library architecture for reusability, testability, and maintainability.

## Current Status: Phase 6/12 Complete

### ✅ Completed Phases

#### Phase 1: Setup
- Created `src/research/` module structure
- Added 8 placeholder module files
- Updated `src/lib.rs` to expose research module

#### Phase 2: Extract Types
- **File**: `src/research/types.rs` (30 lines)
- **Exports**: `CodebaseAnalysis`, `ResearchPrompt`, `PromptsData`, `ResearchResult`
- All types are `pub` and properly documented

#### Phase 3: Extract CLI
- **File**: `src/research/cli.rs` (130 lines)
- **Exports**: `Args` struct with clap + WorkflowDefinition derives
- All CLI fields and metadata preserved

#### Phase 4: Extract Phase 0
- **File**: `src/research/phase0_analyze.rs` (175 lines)
- **Exports**: `analyze_codebase()` - codebase analysis agent
- **Includes**: `extract_yaml()` helper function

#### Phase 5: Extract Phase 1
- **File**: `src/research/phase1_prompts.rs` (142 lines)
- **Exports**: `generate_prompts()` - research prompt generation
- **Includes**: `extract_yaml()` helper function

#### Phase 6: Extract Phase 2
- **File**: `src/research/phase2_research.rs` (205 lines)
- **Exports**:
  - `execute_research()` - parallel research execution orchestrator
  - `execute_research_prompt()` - single research agent
- **Includes**: `extract_yaml()` helper, parallel execution logic

### 📊 Metrics

| Metric | Value |
|--------|-------|
| Original binary size | ~1300 lines |
| Current binary size | ~820 lines |
| Lines extracted | ~480 lines |
| Library modules created | 6 files |
| Reduction | 37% |

## 📋 Remaining Work

### Phase 7: Extract Phase 3 (validate_yaml)
**Target**: `src/research/phase3_validate.rs`
**Functions**:
- `validate_yaml_file()` - YAML validation with Python script
- `execute_fix_yaml()` - YAML fixing agent
- `find_yaml_files()` - directory scanning helper
- Parallel validation and fixing logic

**Estimated extraction**: ~150 lines

### Phase 8: Extract Phase 4 (synthesize_documentation)
**Target**: `src/research/phase4_synthesize.rs`
**Functions**:
- `synthesize_documentation()` - documentation synthesis with file-condenser subagent
- Already has AgentDefinition for subagent

**Estimated extraction**: ~100 lines

### Phase 9: Extract Workflow Orchestration
**Target**: `src/research/workflow.rs`
**New structure**:
```rust
pub struct WorkflowConfig {
    pub objective: Option<String>,
    pub phases: Vec<u32>,
    pub batch_size: usize,
    pub dir: Option<String>,
    pub analysis_file: Option<String>,
    pub prompts_file: Option<String>,
    pub results_file: Option<String>,
    pub output: Option<String>,
    pub system_prompt: Option<String>,
    pub append: Option<String>,
}

pub async fn run_research_workflow(config: WorkflowConfig) -> anyhow::Result<()>
```

**Includes**:
- Phase selection logic
- State tracking between phases
- File path management
- Timestamp generation
- Helper functions: `load_prompt_file()`, etc.

**Estimated extraction**: ~300 lines

### Phase 10: Simplify Binary
**Target**: `src/bin/research_agent.rs` → ~15 lines

```rust
use clap::Parser;
use workflow_manager::research::{cli::Args, workflow::run_research_workflow};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if args.workflow_metadata {
        args.print_metadata();
        return Ok(());
    }

    let config = args.into();
    run_research_workflow(config).await
}
```

### Phase 11: Update Module Exports
**Target**: `src/research/mod.rs`

Enable clean re-exports:
```rust
pub use types::{CodebaseAnalysis, PromptsData, ResearchPrompt, ResearchResult};
pub use workflow::{run_research_workflow, WorkflowConfig};
```

### Phase 12: Add Tests
**Target**: `tests/research/`

Create integration tests:
- `test_phase0.rs` - codebase analysis
- `test_phase1.rs` - prompt generation
- `test_phase2.rs` - research execution
- `test_helpers.rs` - extract_yaml, etc.

## File Structure (Target)

```
workflow-manager/
├── src/
│   ├── lib.rs
│   ├── research/
│   │   ├── mod.rs                    # Module exports
│   │   ├── types.rs                  # ✅ Data structures
│   │   ├── cli.rs                    # ✅ CLI arguments
│   │   ├── phase0_analyze.rs         # ✅ Codebase analysis
│   │   ├── phase1_prompts.rs         # ✅ Prompt generation
│   │   ├── phase2_research.rs        # ✅ Research execution
│   │   ├── phase3_validate.rs        # 🔲 YAML validation
│   │   ├── phase4_synthesize.rs      # 🔲 Documentation synthesis
│   │   └── workflow.rs               # 🔲 Workflow orchestration
│   └── bin/
│       └── research_agent.rs         # 🔲 Thin CLI wrapper (~15 lines)
└── tests/
    └── research/
        └── ...                        # 🔲 Integration tests
```

## Benefits Achieved

✅ **Modularity**: Each phase is self-contained
✅ **Reusability**: Library functions callable from TUI or other code
✅ **Clarity**: Binary reduced from 1300 to 820 lines (37% reduction)
✅ **Maintainability**: Each module < 250 lines

## Next Steps

1. **Continue with Phase 7**: Extract Phase 3 validation logic
2. **Phase 8**: Extract Phase 4 synthesis
3. **Phase 9**: Extract workflow orchestration (biggest refactor)
4. **Phase 10**: Create thin binary wrapper
5. **Phase 11**: Clean up module exports
6. **Phase 12**: Add comprehensive tests

## Timeline Estimate

- Phases 7-8: ~2 hours (extract phase functions)
- Phase 9: ~3 hours (workflow orchestration is complex)
- Phases 10-12: ~2 hours (cleanup and testing)
- **Total remaining**: ~7 hours

## Notes

- Each `extract_yaml()` helper is duplicated in phase modules - consider extracting to shared utility module later
- `load_prompt_file()` and `find_yaml_files()` are helpers that will move to workflow.rs
- Binary still uses `FuturesUnordered` and `Semaphore` for Phase 3 validation - will be extracted in Phase 7
