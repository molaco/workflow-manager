//! Phase 1: Validate codebase analysis YAML
//!
//! Validates the YAML structure from Phase 0 before generating prompts.
//!
//! This phase:
//! - Takes the codebase analysis from Phase 0
//! - Validates its YAML structure using an external Python validator
//! - If invalid, uses a Claude agent to fix the YAML
//! - Loops until the YAML is valid
//! - Returns the validated codebase analysis
//!
//! This ensures Phase 2 receives valid, parseable YAML for prompt generation.

use crate::research::phase4_validate::{execute_fix_yaml, validate_yaml_file};
use crate::research::types::CodebaseAnalysis;
use anyhow::{Context, Result};
use tokio::fs;

/// Validate codebase analysis YAML structure
///
/// Takes the codebase analysis from Phase 0, validates its YAML structure,
/// and fixes any issues using an AI agent. Returns the validated analysis.
///
/// # Arguments
/// * `analysis_file_path` - Path to the codebase analysis YAML file to validate
///
/// # Returns
/// * `Ok(CodebaseAnalysis)` - The validated codebase analysis
/// * `Err(_)` - If validation or fixing fails after multiple attempts
pub async fn validate_codebase_analysis(
    analysis_file_path: &str,
) -> Result<CodebaseAnalysis> {
    println!("\n📋 Validating codebase analysis YAML structure...");
    println!("File: {}", analysis_file_path);

    const MAX_ITERATIONS: usize = 3;
    let mut iteration = 0;

    loop {
        iteration += 1;
        if iteration > MAX_ITERATIONS {
            anyhow::bail!(
                "Failed to fix YAML after {} attempts. Manual intervention required.",
                MAX_ITERATIONS
            );
        }

        println!("\n🔍 Validation attempt {}/{}", iteration, MAX_ITERATIONS);

        // Validate the YAML file
        let (_, is_valid, error_message) = validate_yaml_file(analysis_file_path)
            .await
            .with_context(|| {
                format!(
                    "Failed to validate YAML file: {}",
                    analysis_file_path
                )
            })?;

        if is_valid {
            println!("✅ Codebase analysis YAML is valid!");
            break;
        }

        // YAML is invalid, attempt to fix it
        println!("❌ YAML validation failed");
        println!("Error: {}", error_message.lines().next().unwrap_or("Unknown error"));

        println!("\n🔧 Attempting to fix YAML with AI agent...");
        execute_fix_yaml(analysis_file_path, &error_message, None, 1)
            .await
            .with_context(|| {
                format!("Failed to fix YAML file: {}", analysis_file_path)
            })?;

        println!("✓ Fix attempt completed, re-validating...");
    }

    // Read and parse the validated YAML
    let yaml_content = fs::read_to_string(analysis_file_path)
        .await
        .with_context(|| {
            format!(
                "Failed to read validated YAML file: {}",
                analysis_file_path
            )
        })?;

    let analysis: CodebaseAnalysis = serde_yaml::from_str(&yaml_content)
        .with_context(|| {
            format!(
                "Failed to parse validated YAML file: {}",
                analysis_file_path
            )
        })?;

    println!("✅ Codebase analysis validated and loaded successfully");

    Ok(analysis)
}
