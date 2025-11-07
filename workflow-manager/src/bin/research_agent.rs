/*
┌─────────────────────────────────────────────────────────────────────────────┐
│                         RESEARCH AGENT WORKFLOW                              │
└─────────────────────────────────────────────────────────────────────────────┘

  Phase 0: ANALYZE CODEBASE
    │
    ├─> Read files (Glob, Read, Grep, Bash)
    ├─> Count files by extension
    ├─> Map directory structure
    ├─> Identify entry points & configs
    ├─> Extract dependencies & frameworks
    ├─> Detect architecture patterns
    └─> Output: codebase_analysis_<timestamp>.yaml

         ↓

  Phase 1: VALIDATE ANALYSIS
    │
    ├─> Input: codebase_analysis.yaml
    ├─> Validate YAML structure with check_yaml.py
    ├─> If invalid: Fix with Claude agent
    └─> Loop until valid

         ↓

  Phase 2: GENERATE RESEARCH PROMPTS
    │
    ├─> Input: objective + codebase_analysis.yaml
    ├─> Use custom system prompt + output style
    ├─> LLM generates research questions
    └─> Output: research_prompts_<timestamp>.yaml

         ↓

  Phase 3: EXECUTE RESEARCH (concurrent)
    │
    ├─> For each prompt in research_prompts.yaml:
    │   ├─> Query Claude with prompt (concurrent execution)
    │   ├─> Collect YAML response
    │   └─> Store result in ./RESULTS/
    └─> Output: research_results_<timestamp>.yaml

         ↓

  Phase 4: VALIDATE & FIX YAML (loop until valid)
    │
    ├─> Validate all result files with check_yaml.py
    ├─> Identify files with errors
    └─> Loop:
        ├─> Fix broken files concurrently with Claude
        ├─> Re-validate fixed files
        └─> Continue until all valid

         ↓

  Phase 5: SYNTHESIZE DOCUMENTATION
    │
    ├─> Input: objective + research_results.yaml
    ├─> LLM synthesizes all findings
    ├─> Generate comprehensive markdown
    └─> Output: research_output_<timestamp>.md (or custom path)

┌─────────────────────────────────────────────────────────────────────────────┐
│ FEATURES:                                                                    │
│ • Resume from any phase (--analysis-file, --prompts-file, --results-file)  │
│ • Concurrent execution (--batch-size N for parallel prompts & fixes)       │
│ • Phase selection (--phases 0,1,2,3,4,5)                                    │
│ • Custom prompts (--system-prompt, --append for output style)              │
│ • YAML validation & repair (Phases 1 & 4 - can run standalone or in flow)  │
└─────────────────────────────────────────────────────────────────────────────┘

EXAMPLE COMMANDS:

  # Run all phases (full workflow)
  cargo run --example new_research_agent -- \
    --input "How does the authentication system work?" \
    --system-prompt prompts/writer.md \
    --append prompts/style.md \
    --output docs/auth_guide.md

  # Phase 0 only: Analyze codebase
  cargo run --example new_research_agent -- \
    --phases 0 \
    --dir /path/to/codebase

  # Phase 1 only: Validate codebase analysis
  cargo run --example new_research_agent -- \
    --phases 1 \
    --analysis-file codebase_analysis_20250101_120000.yaml

  # Phase 2 only: Generate prompts (requires analysis file)
  cargo run --example new_research_agent -- \
    --phases 2 \
    --input "Explain the database layer" \
    --system-prompt prompts/writer.md \
    --append prompts/style.md \
    --analysis-file codebase_analysis_20250101_120000.yaml

  # Phase 3 only: Execute research (sequential)
  cargo run --example new_research_agent -- \
    --phases 3 \
    --prompts-file research_prompts_20250101_120000.yaml

  # Phase 3 only: Execute research (parallel batch of 3)
  cargo run --example new_research_agent -- \
    --phases 3 \
    --prompts-file research_prompts_20250101_120000.yaml \
    --batch-size 3

  # Phase 4 only: Validate & fix YAML files (using directory)
  cargo run --example new_research_agent -- \
    --phases 4 \
    --results-dir ./RESULTS \
    --batch-size 2

  # Phase 4 only: Validate & fix YAML files (using results file)
  cargo run --example new_research_agent -- \
    --phases 4 \
    --results-file research_results_20250101_120000.yaml \
    --batch-size 2

  # Phase 5 only: Synthesize documentation (input optional)
  cargo run --example new_research_agent -- \
    --phases 5 \
    --results-file research_results_20250101_120000.yaml \
    --output docs/api_guide.md

  # Resume from Phase 3 onwards (includes validation, input optional for phase 5)
  cargo run --example new_research_agent -- \
    --phases 3,4,5 \
    --prompts-file research_prompts_20250101_120000.yaml \
    --output docs/testing.md

*/

use clap::Parser;
use workflow_manager::research::{cli::Args, run_research_workflow, WorkflowConfig};
use workflow_manager_sdk::WorkflowDefinition;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Handle workflow metadata flag
    if args.workflow_metadata {
        args.print_metadata();
        return Ok(());
    }

    // Convert args to config and run workflow
    let config: WorkflowConfig = args.into();
    run_research_workflow(config).await
}
