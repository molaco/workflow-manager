# Implementation Plan: Example Workflow

## Overview
This document provides an example implementation plan for creating a new workflow in the workflow-manager system. This example demonstrates how to structure a multi-phase workflow using Rust and the claude-agent-sdk-rust package.

## Example: Code Documentation Generator Workflow

### Purpose
Create a workflow that automatically generates comprehensive documentation for Rust codebases by analyzing source code, extracting API information, and generating markdown documentation.

### Workflow Phases

#### Phase 0: Repository Scan
**Goal**: Scan the Rust project to identify modules, structs, traits, and functions.

**Inputs**:
- `project_dir`: Path to the Rust project directory

**Outputs**:
- `repository_structure.yaml`: A structured representation of the codebase
  ```yaml
  modules:
    - name: "module_name"
      path: "src/module_name/mod.rs"
      items:
        - type: "struct"
          name: "StructName"
          visibility: "pub"
          line: 42
  ```

**Implementation**:
- Use `walkdir` to traverse the source directory
- Parse `.rs` files using `syn` crate
- Extract module structure, public APIs, and doc comments
- Save results to `OUTPUT/repository_structure_{timestamp}.yaml`

#### Phase 1: Documentation Plan Generation
**Goal**: Analyze the repository structure and generate a documentation plan.

**Inputs**:
- `repository_structure.yaml` from Phase 0
- `doc_objective`: User-specified documentation focus (e.g., "API reference", "Getting Started Guide")

**Outputs**:
- `doc_plan.yaml`: List of documentation sections to generate
  ```yaml
  sections:
    - id: 1
      title: "Module Overview"
      focus: "High-level architecture"
      modules: ["core", "api", "utils"]
    - id: 2
      title: "API Reference"
      focus: "Public structs and traits"
      modules: ["api"]
  ```

**Implementation**:
- Use Claude agent with tool access to analyze repository structure
- Generate targeted documentation sections based on the objective
- Save plan to `OUTPUT/doc_plan_{timestamp}.yaml`

#### Phase 2: Documentation Generation
**Goal**: Generate documentation content for each section in parallel.

**Inputs**:
- `doc_plan.yaml` from Phase 1
- `repository_structure.yaml` from Phase 0
- `batch_size`: Number of concurrent agent processes

**Outputs**:
- `RESULTS/doc_section_{id}.md`: Individual documentation sections

**Implementation**:
- Execute documentation generation prompts concurrently
- Use semaphore-based concurrency control
- Each agent reads relevant source files and generates markdown
- Save each section to individual files

#### Phase 3: Documentation Validation
**Goal**: Validate generated documentation for completeness and accuracy.

**Inputs**:
- All `doc_section_*.md` files from Phase 2
- `repository_structure.yaml` from Phase 0

**Outputs**:
- Validated and corrected documentation sections
- `validation_report.txt`: List of issues found and fixed

**Implementation**:
- Check for broken code examples
- Verify all public APIs are documented
- Ensure code snippets compile
- Fix any issues using iterative agent-based repair

#### Phase 4: Documentation Assembly
**Goal**: Combine all sections into a final documentation file.

**Inputs**:
- All validated `doc_section_*.md` files
- `doc_plan.yaml` for ordering

**Outputs**:
- `OUTPUT/documentation_{timestamp}.md`: Final complete documentation

**Implementation**:
- Concatenate sections in the correct order
- Generate table of contents
- Add navigation links
- Format final output

### Module Structure

```
workflow-manager/src/
  doc_generator/
    mod.rs              # Module documentation and re-exports
    types.rs            # Data structures (RepositoryStructure, DocPlan, DocSection)
    utils.rs            # Utility functions (file I/O, parsing)
    cli.rs              # Command-line argument parsing
    workflow.rs         # Main orchestration logic
    phase0_scan.rs      # Repository scanning implementation
    phase1_plan.rs      # Documentation plan generation
    phase2_generate.rs  # Parallel documentation generation
    phase3_validate.rs  # Validation and correction
    phase4_assemble.rs  # Final assembly
```

### Data Types

#### RepositoryStructure
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryStructure {
    pub project_name: String,
    pub modules: Vec<Module>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub name: String,
    pub path: String,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub item_type: String,  // "struct", "trait", "function", etc.
    pub name: String,
    pub visibility: String,
    pub doc_comment: Option<String>,
    pub line: usize,
}
```

#### DocPlan
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocPlan {
    pub objective: String,
    pub sections: Vec<DocSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocSection {
    pub id: usize,
    pub title: String,
    pub focus: String,
    pub modules: Vec<String>,
    pub priority: u8,
}
```

#### WorkflowConfig
```rust
#[derive(Debug, Clone)]
pub struct WorkflowConfig {
    pub objective: Option<String>,
    pub phases: Vec<u32>,
    pub batch_size: usize,
    pub project_dir: Option<String>,
    pub repository_file: Option<String>,
    pub plan_file: Option<String>,
    pub output: Option<String>,
}
```

### CLI Interface

```bash
# Run complete workflow
cargo run --bin doc_generator -- \
  --objective "Generate API reference documentation" \
  --project-dir ./my-project \
  --phases 0,1,2,3,4 \
  --batch-size 3 \
  --output docs/api.md

# Resume from saved state
cargo run --bin doc_generator -- \
  --plan-file OUTPUT/doc_plan_20250127.yaml \
  --phases 2,3,4 \
  --batch-size 5
```

### Error Handling

- **Phase 0**: Handle parse errors gracefully, log unparseable files
- **Phase 1**: Validate plan has at least one section
- **Phase 2**: Retry failed sections up to 3 times
- **Phase 3**: Iterative validation loop until all sections pass
- **Phase 4**: Verify all sections exist before assembly

### Testing Strategy

1. **Unit Tests**:
   - Test data structure serialization/deserialization
   - Test utility functions (path handling, file I/O)
   - Test validation logic

2. **Integration Tests**:
   - Test complete workflow on sample Rust project
   - Test resumability from each phase
   - Test concurrent execution with different batch sizes

3. **Test Files**:
   ```
   workflow-manager/tests/
     doc_generator_tests.rs
     doc_generator/
       common.rs          # Shared test utilities
       test_types.rs      # Type tests
       test_workflow.rs   # Workflow orchestration tests
       test_integration.rs # End-to-end tests
       fixtures/          # Sample Rust project for testing
   ```

### Implementation Checklist

- [ ] Create module structure (`src/doc_generator/`)
- [ ] Define data types in `types.rs`
- [ ] Implement utility functions in `utils.rs`
- [ ] Implement CLI argument parsing in `cli.rs`
- [ ] Implement Phase 0: Repository scanning
- [ ] Implement Phase 1: Documentation plan generation
- [ ] Implement Phase 2: Parallel documentation generation
- [ ] Implement Phase 3: Validation and correction
- [ ] Implement Phase 4: Assembly
- [ ] Implement workflow orchestration in `workflow.rs`
- [ ] Create binary entry point (`src/bin/doc_generator.rs`)
- [ ] Write unit tests for each module
- [ ] Write integration tests
- [ ] Add documentation to `mod.rs`
- [ ] Update main `lib.rs` to export new module

### Dependencies

Add to `Cargo.toml`:
```toml
[dependencies]
syn = "2.0"           # Rust parsing
quote = "1.0"         # Code generation
walkdir = "2.4"       # Directory traversal
```

### Success Metrics

- Successfully generates documentation for a 10-module Rust project
- Handles concurrent execution with up to 10 parallel agents
- Can resume from any phase using saved state
- Validates and fixes common documentation issues automatically
- Produces well-formatted, navigable markdown output

### Future Enhancements

- Support for multiple output formats (HTML, PDF)
- Custom documentation templates
- Integration with `cargo doc`
- Automatic diagram generation from code structure
- Cross-reference validation
