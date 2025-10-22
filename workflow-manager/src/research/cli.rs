//! CLI argument parsing for research workflow

use clap::Parser;
use workflow_manager_sdk::WorkflowDefinition;

/// Research Agent CLI Arguments
#[derive(Parser, Debug, Clone, WorkflowDefinition)]
#[workflow(
    id = "research_agent",
    name = "Research Agent Workflow",
    description = "Multi-phase research workflow: Analyze codebase → Generate prompts → Execute research → Validate YAML → Synthesize docs"
)]
pub struct Args {
    /// Research objective/question
    #[arg(short, long)]
    #[field(
        label = "Research Objective",
        description = "[TEXT] What do you want to research about the codebase?",
        type = "text",
        required_for_phases = "1"
    )]
    pub input: Option<String>,

    /// Prompt writer system prompt (file path or string)
    #[arg(short = 's', long)]
    #[field(
        label = "System Prompt",
        description = "[TEXT] Path to prompt writer system prompt file",
        type = "file_path",
        required_for_phases = "1"
    )]
    pub system_prompt: Option<String>,

    /// Output style format (file path or string)
    #[arg(short = 'a', long)]
    #[field(
        label = "Output Style",
        description = "[TEXT] Path to output style format file",
        type = "file_path",
        required_for_phases = "1"
    )]
    pub append: Option<String>,

    /// Output file path for final documentation
    #[arg(short, long)]
    #[field(
        label = "Output File",
        description = "[TEXT] Path for final documentation (e.g., docs/guide.md)",
        type = "file_path"
    )]
    pub output: Option<String>,

    /// Number of research prompts to execute in parallel (default: 1 for sequential)
    #[arg(long, default_value = "1")]
    #[field(
        label = "Batch Size",
        description = "[NUMBER] Parallel execution batch size (1-10)",
        type = "number",
        min = "1",
        max = "10"
    )]
    pub batch_size: usize,

    /// Comma-separated phases to execute (0=analyze, 1=prompts, 2=research, 3=validate, 4=synthesize)
    #[arg(long, default_value = "0,1,2,3,4")]
    #[field(
        label = "Phases to Run",
        description = "[PHASES] Select which phases to execute (0-4)",
        type = "phase_selector",
        total_phases = "5"
    )]
    pub phases: String,

    /// Path to saved codebase analysis YAML (for resuming from Phase 1)
    #[arg(long)]
    #[field(
        label = "Analysis File",
        description = "[STATE FILE] Resume with existing codebase analysis",
        type = "state_file",
        pattern = "codebase_analysis_*.yaml",
        required_for_phases = "1"
    )]
    pub analysis_file: Option<String>,

    /// Path to saved prompts YAML (for resuming from Phase 2)
    #[arg(long)]
    #[field(
        label = "Prompts File",
        description = "[STATE FILE] Resume with existing research prompts",
        type = "state_file",
        pattern = "research_prompts_*.yaml",
        required_for_phases = "2"
    )]
    pub prompts_file: Option<String>,

    /// Path to saved research results YAML (for resuming from Phase 3)
    #[arg(long)]
    #[field(
        label = "Results File",
        description = "[STATE FILE] Resume with existing research results",
        type = "state_file",
        pattern = "research_results_*.yaml",
        required_for_phases = "3,4"
    )]
    pub results_file: Option<String>,

    /// Directory path to analyze for Phase 0
    #[arg(long)]
    #[field(
        label = "Directory",
        description = "[TEXT] Directory to analyze (default: current directory)",
        type = "file_path"
    )]
    pub dir: Option<String>,

    /// Directory containing YAML files to validate (for Phase 3)
    #[arg(long)]
    #[field(
        label = "Results Directory",
        description = "[TEXT] Directory containing YAML files to validate",
        type = "file_path",
        required_for_phases = "3"
    )]
    pub results_dir: Option<String>,

    // Hidden metadata flag
    #[arg(long, hide = true)]
    pub workflow_metadata: bool,
}

impl Args {
    /// Parse the comma-separated phases string into a Vec<u32>
    pub fn parse_phases(&self) -> Vec<u32> {
        self.phases
            .split(',')
            .filter_map(|p| p.trim().parse().ok())
            .collect()
    }
}

impl From<Args> for crate::research::workflow::WorkflowConfig {
    fn from(args: Args) -> Self {
        let phases = args.parse_phases();
        crate::research::workflow::WorkflowConfig {
            objective: args.input,
            phases,
            batch_size: args.batch_size,
            dir: args.dir,
            analysis_file: args.analysis_file,
            prompts_file: args.prompts_file,
            results_file: args.results_file,
            results_dir: args.results_dir,
            output: args.output,
            system_prompt: args.system_prompt,
            append: args.append,
        }
    }
}
