use std::process::Command;
use workflow_manager::discovery;

fn main() {
    println!("Testing workflow discovery...\n");

    // First test manual extraction
    println!("Testing manual metadata extraction on test_workflow:");
    let test_binary = "../target/debug/examples/test_workflow";
    let output = Command::new(test_binary)
        .arg("--workflow-metadata")
        .output();

    match output {
        Ok(out) => {
            println!("  Status: {}", out.status);
            println!("  Stdout length: {}", out.stdout.len());
            if out.stdout.len() > 0 {
                println!(
                    "  Output: {}",
                    String::from_utf8_lossy(&out.stdout[..100.min(out.stdout.len())])
                );
            }
        }
        Err(e) => println!("  Error: {}", e),
    }

    println!("\n---\n");

    // List binaries in the target folder
    println!("Binaries in target/debug/examples without extensions:");
    if let Ok(entries) = std::fs::read_dir("../target/debug/examples") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if !name.contains('.') && path.is_file() {
                    println!("  - {}", name);
                }
            }
        }
    }

    println!("\n---\n");

    let workflows = discovery::discover_workflows();

    println!("Discovered {} workflows:", workflows.len());
    for workflow in &workflows {
        println!("\n✓ {} ({})", workflow.metadata.name, workflow.metadata.id);
        println!("  Binary: {}", workflow.binary_path.display());
        println!("  Fields: {}", workflow.fields.len());
        for field in &workflow.fields {
            println!(
                "    - {} ({}): {}",
                field.name, field.label, field.description
            );
        }
    }
}
