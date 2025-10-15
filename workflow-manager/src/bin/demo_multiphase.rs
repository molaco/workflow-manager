use anyhow::Result;
use clap::Parser;
use std::time::Duration;
use tokio::time::sleep;
use workflow_manager_sdk::{
    log_agent_complete, log_agent_message, log_agent_start, log_phase_complete, log_phase_start,
    log_state_file, log_task_complete, log_task_progress, log_task_start, WorkflowDefinition,
};

#[derive(Parser, Debug, Clone, WorkflowDefinition)]
#[workflow(
    id = "demo_multiphase",
    name = "Demo Multi-Phase Workflow",
    description = "Demonstrates multi-phase workflow with hierarchical logging"
)]
struct Args {
    /// Number of tasks per phase
    #[arg(short = 'n', long, default_value = "3")]
    #[field(
        label = "Tasks per Phase",
        description = "[NUMBER] Number of tasks to run in each phase (1-10)",
        type = "number",
        min = "1",
        max = "10"
    )]
    tasks_per_phase: usize,

    /// Simulate slow execution
    #[arg(short = 's', long, action = clap::ArgAction::SetTrue)]
    #[field(
        label = "Slow Mode",
        description = "[BOOL] Leave empty or type 'true' to enable slow mode",
        type = "text"
    )]
    slow_mode: bool,

    // Hidden metadata flag
    #[arg(long, hide = true)]
    workflow_metadata: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Handle metadata request from TUI
    if args.workflow_metadata {
        args.print_metadata();
        return Ok(());
    }

    println!("🚀 Demo Multi-Phase Workflow Started");
    println!("========================================\n");

    // Phase 0: Initialize
    phase_0_initialize(&args).await?;

    // Phase 1: Process Data
    phase_1_process(&args).await?;

    // Phase 2: Analyze with Agents
    phase_2_analyze(&args).await?;

    println!("\n========================================");
    println!("✅ All phases completed successfully!");

    Ok(())
}

async fn phase_0_initialize(args: &Args) -> Result<()> {
    log_phase_start!(0, "Initialize", 3);
    println!("\n📋 PHASE 0: Initialize");
    println!("─────────────────────────────────────");

    for i in 1..=args.tasks_per_phase {
        let task_id = format!("init_task_{}", i);
        log_task_start!(0, &task_id, format!("Initialize component {}", i));

        println!("  • Initializing component {}...", i);
        if args.slow_mode {
            sleep(Duration::from_millis(500)).await;
        } else {
            sleep(Duration::from_millis(100)).await;
        }

        log_task_progress!(&task_id, format!("Component {} configured", i));
        if args.slow_mode {
            sleep(Duration::from_millis(300)).await;
        }

        log_task_complete!(&task_id, format!("Component {} ready", i));
        println!("    ✓ Component {} initialized", i);
    }

    // Create state file
    let state_file = "initialization_state.yaml";
    log_state_file!(0, state_file, "Initialization configuration");
    println!("  📄 Created: {}", state_file);

    log_phase_complete!(0, "Initialize");
    println!("  ✅ Phase 0 complete\n");

    Ok(())
}

async fn phase_1_process(args: &Args) -> Result<()> {
    log_phase_start!(1, "Process Data", 3);
    println!("📊 PHASE 1: Process Data");
    println!("─────────────────────────────────────");

    for i in 1..=args.tasks_per_phase {
        let task_id = format!("process_task_{}", i);
        log_task_start!(1, &task_id, format!("Process dataset {}", i));

        println!("  • Processing dataset {}...", i);

        // Simulate processing steps
        for step in 1..=3 {
            if args.slow_mode {
                sleep(Duration::from_millis(400)).await;
            } else {
                sleep(Duration::from_millis(80)).await;
            }
            log_task_progress!(&task_id, format!("Step {}/3: Processing...", step));
        }

        log_task_complete!(&task_id, format!("Dataset {} processed", i));
        println!("    ✓ Dataset {} processed", i);
    }

    // Create state file
    let state_file = "processed_data.yaml";
    log_state_file!(1, state_file, "Processed datasets");
    println!("  📄 Created: {}", state_file);

    log_phase_complete!(1, "Process Data");
    println!("  ✅ Phase 1 complete\n");

    Ok(())
}

async fn phase_2_analyze(args: &Args) -> Result<()> {
    log_phase_start!(2, "Analyze with Agents", 3);
    println!("🤖 PHASE 2: Analyze with Agents");
    println!("─────────────────────────────────────");

    for i in 1..=args.tasks_per_phase {
        let task_id = format!("analyze_task_{}", i);
        log_task_start!(2, &task_id, format!("Analyze topic {}", i));

        println!("  • Analyzing topic {}...", i);

        // Spawn multiple agents for each task
        let agents = vec!["validator", "formatter", "reviewer"];

        for agent in agents {
            log_agent_start!(&task_id, agent, format!("Running @{} agent", agent));
            println!("    → Starting @{} agent", agent);

            if args.slow_mode {
                sleep(Duration::from_millis(600)).await;
            } else {
                sleep(Duration::from_millis(150)).await;
            }

            log_agent_message!(&task_id, agent, format!("@{} analyzing...", agent));

            if args.slow_mode {
                sleep(Duration::from_millis(400)).await;
            } else {
                sleep(Duration::from_millis(100)).await;
            }

            log_agent_complete!(&task_id, agent, format!("@{} found {} items", agent, i * 2));
            println!("      ✓ @{} complete", agent);
        }

        log_task_complete!(&task_id, format!("Analysis {} complete", i));
        println!("    ✓ Topic {} analyzed", i);
    }

    // Create state file
    let state_file = "analysis_results.yaml";
    log_state_file!(2, state_file, "Analysis results from all agents");
    println!("  📄 Created: {}", state_file);

    log_phase_complete!(2, "Analyze with Agents");
    println!("  ✅ Phase 2 complete\n");

    Ok(())
}
