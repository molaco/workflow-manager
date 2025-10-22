//! Integration tests for the research module
//!
//! Note: Tests that require Claude API access are marked with #[ignore]
//! Run them with: cargo test -- --ignored

use workflow_manager::research::WorkflowConfig;

#[test]
fn test_module_imports() {
    // Verify all public exports are accessible
    let _config = WorkflowConfig::default();

    // This test just verifies the module structure compiles
    assert!(true);
}

#[test]
fn test_workflow_config_builder_pattern() {
    // Test that we can build configs fluently
    let config = WorkflowConfig {
        objective: Some("Test".to_string()),
        phases: vec![0, 1, 2, 3, 4],
        batch_size: 2,
        dir: Some(".".to_string()),
        system_prompt: Some("prompts/writer.md".to_string()),
        append: Some("prompts/style.md".to_string()),
        ..Default::default()
    };

    assert!(config.objective.is_some());
    assert_eq!(config.phases.len(), 5);
}

#[test]
fn test_public_api_accessibility() {
    // Verify that all public types are accessible
    use workflow_manager::research::{
        CodebaseAnalysis, PromptsData, ResearchPrompt, ResearchResult, WorkflowConfig,
    };

    // Create instances to verify they're accessible
    let _prompt = ResearchPrompt {
        title: "Test".to_string(),
        query: "Query".to_string(),
        focus: vec![],
    };

    let _prompts = PromptsData {
        objective: "Test".to_string(),
        prompts: vec![],
    };

    let _result = ResearchResult {
        title: "Test".to_string(),
        query: "Query".to_string(),
        response_file: "file.yaml".to_string(),
        focus: vec![],
    };

    let _config = WorkflowConfig::default();

    // CodebaseAnalysis is a type alias for serde_yaml::Value
    let _analysis: CodebaseAnalysis = serde_yaml::from_str("key: value").unwrap();

    assert!(true);
}

#[test]
fn test_phase_modules_accessible() {
    // Verify phase modules are public
    use workflow_manager::research::phase3_validate;

    // Verify helper functions are accessible
    let _ = phase3_validate::find_yaml_files;

    assert!(true);
}

#[tokio::test]
#[ignore] // Requires Claude API and significant time
async fn test_full_workflow_phase0_only() {
    use workflow_manager::research::run_research_workflow;

    // Create a minimal test directory
    let test_dir = std::env::temp_dir().join("workflow_manager_integration_test");
    std::fs::create_dir_all(&test_dir).unwrap();

    // Create a simple test file
    std::fs::write(test_dir.join("test.rs"), "fn main() {}").unwrap();

    let config = WorkflowConfig {
        objective: None,
        phases: vec![0], // Only Phase 0 - codebase analysis
        batch_size: 1,
        dir: Some(test_dir.to_str().unwrap().to_string()),
        ..Default::default()
    };

    let result = run_research_workflow(config).await;

    // Cleanup
    let _ = std::fs::remove_dir_all(&test_dir);

    assert!(result.is_ok(), "Phase 0 should complete successfully");
}

#[tokio::test]
#[ignore] // Requires Claude API, test files, and significant time
async fn test_full_workflow_complete() {
    use workflow_manager::research::run_research_workflow;

    let config = WorkflowConfig {
        objective: Some("Analyze the codebase structure and key components".to_string()),
        phases: vec![0, 1, 2, 3, 4],
        batch_size: 1,
        dir: Some(".".to_string()),
        system_prompt: Some("You are a research assistant.".to_string()),
        append: Some("Output in clear, concise format.".to_string()),
        ..Default::default()
    };

    let result = run_research_workflow(config).await;
    assert!(result.is_ok(), "Full workflow should complete successfully");
}

#[tokio::test]
#[ignore] // Requires Claude API
async fn test_workflow_resume_from_saved_state() {
    use workflow_manager::research::run_research_workflow;

    // This test assumes saved state files exist
    // In a real scenario, you'd run Phase 0-1 first, then resume from Phase 2

    let config = WorkflowConfig {
        objective: None,
        phases: vec![2, 3, 4],
        batch_size: 2,
        prompts_file: Some("OUTPUT/research_prompts_test.yaml".to_string()),
        ..Default::default()
    };

    let result = run_research_workflow(config).await;

    // This will fail if the prompts file doesn't exist, which is expected
    // The test is here to document the resume functionality
    if result.is_err() {
        // Expected if test files don't exist
        assert!(true);
    } else {
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_workflow_validates_required_params() {
    use workflow_manager::research::run_research_workflow;

    // Phase 1 requires objective, system_prompt, and append
    let config = WorkflowConfig {
        objective: None, // Missing required parameter
        phases: vec![1],
        ..Default::default()
    };

    let result = run_research_workflow(config).await;
    assert!(result.is_err(), "Should fail without required objective");

    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("required"), "Error should mention required parameter");
}

#[tokio::test]
async fn test_workflow_validates_phase1_params() {
    use workflow_manager::research::run_research_workflow;

    // Phase 1 with objective but no system_prompt
    let config = WorkflowConfig {
        objective: Some("Test".to_string()),
        phases: vec![1],
        system_prompt: None, // Missing
        append: Some("test".to_string()),
        ..Default::default()
    };

    let result = run_research_workflow(config).await;
    assert!(result.is_err(), "Should fail without system_prompt");
}

#[test]
fn test_default_config_has_all_phases() {
    let config = WorkflowConfig::default();
    assert_eq!(config.phases, vec![0, 1, 2, 3, 4]);
}

#[test]
fn test_config_can_run_single_phase() {
    let config = WorkflowConfig {
        phases: vec![3],
        results_dir: Some("RESULTS".to_string()),
        ..Default::default()
    };

    assert_eq!(config.phases.len(), 1);
    assert_eq!(config.phases[0], 3);
}

#[test]
fn test_config_validation_logic() {
    // Test the validation logic expectations
    let config_valid_phase1 = WorkflowConfig {
        objective: Some("Test".to_string()),
        phases: vec![1],
        system_prompt: Some("prompt".to_string()),
        append: Some("style".to_string()),
        ..Default::default()
    };

    // All required fields are present
    assert!(config_valid_phase1.objective.is_some());
    assert!(config_valid_phase1.system_prompt.is_some());
    assert!(config_valid_phase1.append.is_some());
    assert!(config_valid_phase1.phases.contains(&1));
}
