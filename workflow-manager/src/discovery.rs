use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;
use workflow_manager_sdk::{FieldSchema, FullWorkflowMetadata, WorkflowMetadata};

/// Represents a discovered workflow with its metadata and binary path
#[derive(Debug, Clone)]
pub struct DiscoveredWorkflow {
    pub metadata: WorkflowMetadata,
    pub fields: Vec<FieldSchema>,
    pub binary_path: PathBuf,
}

/// Discover all workflows by scanning for binaries and extracting metadata
pub fn discover_workflows() -> Vec<DiscoveredWorkflow> {
    let mut workflows = Vec::new();

    // Search paths for workflow binaries
    let search_paths = get_search_paths();

    for search_dir in search_paths {
        if !search_dir.exists() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(&search_dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                // Skip directories
                if !path.is_file() {
                    continue;
                }

                // Get filename
                let filename = match path.file_name().and_then(|n| n.to_str()) {
                    Some(name) => name,
                    None => continue,
                };

                // Skip the TUI binary itself
                if filename == "workflow-manager" {
                    continue;
                }

                // Skip build artifacts (files with hashes after dash, or files with extensions)
                // Process clean binary names like: test_workflow, research_agent, tasks_agent
                // Skip files like: test_workflow-abc123def, test_workflow.d
                if filename.contains('.') {
                    continue;
                }

                // Skip if it looks like a hash suffix (has dash followed by hex)
                if filename.contains('-') {
                    // Allow hyphens in the name (e.g., my-workflow) but not hash suffixes
                    if let Some(after_dash) = filename.split('-').next_back() {
                        // If after the last dash looks like a hash (long hex string), skip it
                        if after_dash.len() > 10
                            && after_dash.chars().all(|c| c.is_ascii_hexdigit())
                        {
                            continue;
                        }
                    }
                }

                // Check if executable
                if !is_executable(&path) {
                    continue;
                }

                // Try to extract workflow metadata
                if let Ok(workflow) = extract_workflow_metadata(&path) {
                    workflows.push(workflow);
                }
            }
        }
    }

    workflows
}

/// Get list of directories to search for workflow binaries
fn get_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // 1. Built-in workflows: Same directory as the workflow-manager binary
    //    When running from target/debug or target/release, workflow binaries from src/bin are here
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            paths.push(exe_dir.to_path_buf());

            // If running from target/debug/deps or target/release/deps (cargo run),
            // also check the parent directory where the actual binaries are
            if exe_dir.ends_with("deps") {
                if let Some(parent_dir) = exe_dir.parent() {
                    paths.push(parent_dir.to_path_buf());
                }
            }
        }
    }

    // 2. User workflows from ~/.workflow-manager/workflows/
    if let Ok(home) = std::env::var("HOME") {
        paths.push(PathBuf::from(home).join(".workflow-manager/workflows"));
    }

    paths
}

/// Check if a file is executable
fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }

    #[cfg(windows)]
    {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("exe"))
            .unwrap_or(false)
    }

    #[cfg(not(any(unix, windows)))]
    {
        true // Assume executable on other platforms
    }
}

/// Extract workflow metadata by running the binary with --workflow-metadata flag
fn extract_workflow_metadata(binary_path: &Path) -> Result<DiscoveredWorkflow> {
    // Execute: <binary> --workflow-metadata with timeout
    let output = Command::new(binary_path)
        .arg("--workflow-metadata")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null()) // Suppress stderr
        .output();

    let output = match output {
        Ok(out) => out,
        Err(_) => anyhow::bail!("Failed to execute binary"),
    };

    if !output.status.success() {
        anyhow::bail!("Binary did not return success status");
    }

    let json = String::from_utf8(output.stdout)
        .map_err(|_| anyhow::anyhow!("Binary output was not valid UTF-8"))?;

    // Try to parse as FullWorkflowMetadata
    let full_metadata: FullWorkflowMetadata = serde_json::from_str(&json)
        .map_err(|e| anyhow::anyhow!("Failed to parse workflow metadata JSON: {}", e))?;

    Ok(DiscoveredWorkflow {
        metadata: full_metadata.metadata,
        fields: full_metadata.fields,
        binary_path: binary_path.to_path_buf(),
    })
}

/// Build command string from field values
pub fn build_workflow_command(
    workflow: &DiscoveredWorkflow,
    field_values: &std::collections::HashMap<String, String>,
) -> String {
    let mut cmd = format!("{}", workflow.binary_path.display());

    for field in &workflow.fields {
        if let Some(value) = field_values.get(&field.name) {
            if !value.is_empty() {
                // Properly quote values with spaces
                if value.contains(' ') {
                    cmd.push_str(&format!(" {} '{}'", field.cli_arg, value));
                } else {
                    cmd.push_str(&format!(" {} {}", field.cli_arg, value));
                }
            }
        }
    }

    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_workflows() {
        // This test will discover workflows in development
        println!("Starting workflow discovery...");

        let search_paths = get_search_paths();
        println!("Search paths:");
        for path in &search_paths {
            println!("  - {}", path.display());
            if path.exists() {
                println!("    EXISTS");
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        println!("      - {}", entry.path().display());
                    }
                }
            } else {
                println!("    DOES NOT EXIST");
            }
        }

        let workflows = discover_workflows();

        println!("\nDiscovered {} workflows", workflows.len());

        // Print discovered workflows for debugging
        for workflow in &workflows {
            println!(
                "Discovered: {} ({})",
                workflow.metadata.name, workflow.metadata.id
            );
            println!("  Binary: {}", workflow.binary_path.display());
            println!("  Fields: {}", workflow.fields.len());
        }

        // Should find at least the test_workflow
        assert!(
            !workflows.is_empty(),
            "Should discover at least one workflow"
        );
    }
}
