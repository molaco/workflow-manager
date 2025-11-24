//! Tests for WorkflowConfig

use workflow_manager::research::WorkflowConfig;

#[test]
fn test_workflow_config_default() {
    let config = WorkflowConfig::default();

    assert_eq!(config.phases, vec![0, 1, 2, 3, 4]);
    assert_eq!(config.batch_size, 1);
    assert!(config.objective.is_none());
    assert!(config.dir.is_none());
    assert!(config.analysis_file.is_none());
    assert!(config.prompts_file.is_none());
    assert!(config.results_file.is_none());
    assert!(config.results_dir.is_none());
    assert!(config.output.is_none());
    assert!(config.system_prompt.is_none());
    assert!(config.append.is_none());
}

#[test]
fn test_workflow_config_custom_phases() {
    let config = WorkflowConfig {
        phases: vec![0, 1],
        ..Default::default()
    };

    assert_eq!(config.phases, vec![0, 1]);
    assert_eq!(config.batch_size, 1);
}

#[test]
fn test_workflow_config_custom_batch_size() {
    let config = WorkflowConfig {
        batch_size: 5,
        ..Default::default()
    };

    assert_eq!(config.batch_size, 5);
}

#[test]
fn test_workflow_config_with_objective() {
    let config = WorkflowConfig {
        objective: Some("Test objective".to_string()),
        ..Default::default()
    };

    assert_eq!(config.objective, Some("Test objective".to_string()));
}

#[test]
fn test_workflow_config_with_directory() {
    let config = WorkflowConfig {
        dir: Some("./test".to_string()),
        ..Default::default()
    };

    assert_eq!(config.dir, Some("./test".to_string()));
}

#[test]
fn test_workflow_config_full_custom() {
    let config = WorkflowConfig {
        objective: Some("Test objective".to_string()),
        phases: vec![2, 3, 4],
        batch_size: 3,
        dir: Some("./test".to_string()),
        analysis_file: Some("analysis.yaml".to_string()),
        prompts_file: Some("prompts.yaml".to_string()),
        results_file: Some("results.yaml".to_string()),
        results_dir: Some("./RESULTS".to_string()),
        output: Some("output.md".to_string()),
        system_prompt: Some("prompts/writer.md".to_string()),
        append: Some("prompts/style.md".to_string()),
    };

    assert_eq!(config.objective, Some("Test objective".to_string()));
    assert_eq!(config.phases, vec![2, 3, 4]);
    assert_eq!(config.batch_size, 3);
    assert_eq!(config.dir, Some("./test".to_string()));
    assert_eq!(config.analysis_file, Some("analysis.yaml".to_string()));
    assert_eq!(config.prompts_file, Some("prompts.yaml".to_string()));
    assert_eq!(config.results_file, Some("results.yaml".to_string()));
    assert_eq!(config.results_dir, Some("./RESULTS".to_string()));
    assert_eq!(config.output, Some("output.md".to_string()));
    assert_eq!(
        config.system_prompt,
        Some("prompts/writer.md".to_string())
    );
    assert_eq!(config.append, Some("prompts/style.md".to_string()));
}

#[test]
fn test_workflow_config_clone() {
    let config = WorkflowConfig {
        objective: Some("Test objective".to_string()),
        phases: vec![0, 1, 2],
        batch_size: 2,
        dir: Some(".".to_string()),
        ..Default::default()
    };

    let cloned = config.clone();
    assert_eq!(cloned.objective, config.objective);
    assert_eq!(cloned.phases, config.phases);
    assert_eq!(cloned.batch_size, config.batch_size);
    assert_eq!(cloned.dir, config.dir);
}

#[test]
fn test_workflow_config_phase_variations() {
    // Single phase
    let config1 = WorkflowConfig {
        phases: vec![0],
        ..Default::default()
    };
    assert_eq!(config1.phases, vec![0]);

    // Partial phases
    let config2 = WorkflowConfig {
        phases: vec![1, 2, 3],
        ..Default::default()
    };
    assert_eq!(config2.phases, vec![1, 2, 3]);

    // Out of order phases (valid if user specifies)
    let config3 = WorkflowConfig {
        phases: vec![3, 1, 4],
        ..Default::default()
    };
    assert_eq!(config3.phases, vec![3, 1, 4]);

    // Empty phases
    let config4 = WorkflowConfig {
        phases: vec![],
        ..Default::default()
    };
    assert!(config4.phases.is_empty());
}

#[test]
fn test_workflow_config_batch_size_edge_cases() {
    // Minimum batch size
    let config1 = WorkflowConfig {
        batch_size: 1,
        ..Default::default()
    };
    assert_eq!(config1.batch_size, 1);

    // Large batch size
    let config2 = WorkflowConfig {
        batch_size: 100,
        ..Default::default()
    };
    assert_eq!(config2.batch_size, 100);
}

#[test]
fn test_workflow_config_resume_from_phase1() {
    // Config for resuming from Phase 1 (skip Phase 0)
    let config = WorkflowConfig {
        objective: Some("Test objective".to_string()),
        phases: vec![1, 2, 3, 4],
        analysis_file: Some("OUTPUT/codebase_analysis.yaml".to_string()),
        system_prompt: Some("prompts/writer.md".to_string()),
        append: Some("prompts/style.md".to_string()),
        ..Default::default()
    };

    assert_eq!(config.phases, vec![1, 2, 3, 4]);
    assert!(config.analysis_file.is_some());
    assert!(config.objective.is_some());
}

#[test]
fn test_workflow_config_resume_from_phase2() {
    // Config for resuming from Phase 2 (skip Phases 0-1)
    let config = WorkflowConfig {
        phases: vec![2, 3, 4],
        prompts_file: Some("OUTPUT/research_prompts.yaml".to_string()),
        batch_size: 2,
        ..Default::default()
    };

    assert_eq!(config.phases, vec![2, 3, 4]);
    assert!(config.prompts_file.is_some());
    assert!(config.objective.is_none()); // Not needed when resuming
}

#[test]
fn test_workflow_config_resume_from_phase3() {
    // Config for resuming from Phase 3 (skip Phases 0-2)
    let config = WorkflowConfig {
        phases: vec![3, 4],
        results_dir: Some("RESULTS".to_string()),
        batch_size: 3,
        ..Default::default()
    };

    assert_eq!(config.phases, vec![3, 4]);
    assert!(config.results_dir.is_some());
}

#[test]
fn test_workflow_config_only_phase4() {
    // Config for running only Phase 4 (synthesis)
    let config = WorkflowConfig {
        phases: vec![4],
        results_file: Some("RESULTS/research_results.yaml".to_string()),
        output: Some("docs/guide.md".to_string()),
        ..Default::default()
    };

    assert_eq!(config.phases, vec![4]);
    assert!(config.results_file.is_some());
    assert!(config.output.is_some());
}
