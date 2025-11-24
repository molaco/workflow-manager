//! Tests for workflow configuration and orchestration
//!
//! Tests CLI argument parsing, workflow configuration, and integration

use super::common::*;
use workflow_manager::task_planner::cli::Args;
use workflow_manager::task_planner::workflow::WorkflowConfig;

// ============================================================================
// Args Validation Tests
// ============================================================================

#[test]
fn test_args_validate_step1_success() {
    let args = Args {
        step: "1".to_string(),
        impl_files: Some(vec!["impl.md".to_string()]),
        tasks_overview: None,
        tasks: None,
        stream: false,
        debug: false,
        batch_size: None,
        tasks_overview_template: Some("template.yaml".to_string()),
        task_template: None,
        workflow_metadata: false,
    };

    assert!(args.validate_step1().is_ok());
}

#[test]
fn test_args_validate_step1_missing_template() {
    let args = Args {
        step: "1".to_string(),
        impl_files: Some(vec!["impl.md".to_string()]),
        tasks_overview: None,
        tasks: None,
        stream: false,
        debug: false,
        batch_size: None,
        tasks_overview_template: None,
        task_template: None,
        workflow_metadata: false,
    };

    let result = args.validate_step1();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("tasks-overview-template"));
}

#[test]
fn test_args_validate_step2_success() {
    let args = Args {
        step: "2".to_string(),
        impl_files: None,
        tasks_overview: Some("overview.yaml".to_string()),
        tasks: None,
        stream: false,
        debug: false,
        batch_size: None,
        tasks_overview_template: None,
        task_template: Some("task_template.yaml".to_string()),
        workflow_metadata: false,
    };

    assert!(args.validate_step2_or_3().is_ok());
}

#[test]
fn test_args_validate_step2_missing_template() {
    let args = Args {
        step: "2".to_string(),
        impl_files: None,
        tasks_overview: Some("overview.yaml".to_string()),
        tasks: None,
        stream: false,
        debug: false,
        batch_size: None,
        tasks_overview_template: None,
        task_template: None,
        workflow_metadata: false,
    };

    let result = args.validate_step2_or_3();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("task-template"));
}

#[test]
fn test_args_validate_all_steps() {
    let args = Args {
        step: "all".to_string(),
        impl_files: Some(vec!["impl.md".to_string()]),
        tasks_overview: None,
        tasks: None,
        stream: false,
        debug: false,
        batch_size: Some(3),
        tasks_overview_template: Some("overview.yaml".to_string()),
        task_template: Some("task.yaml".to_string()),
        workflow_metadata: false,
    };

    assert!(args.validate().is_ok());
}

#[test]
fn test_args_validate_invalid_step() {
    let args = Args {
        step: "invalid".to_string(),
        impl_files: None,
        tasks_overview: None,
        tasks: None,
        stream: false,
        debug: false,
        batch_size: None,
        tasks_overview_template: Some("t1.yaml".to_string()),
        task_template: Some("t2.yaml".to_string()),
        workflow_metadata: false,
    };

    let result = args.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid step"));
}

// ============================================================================
// Args Utility Methods Tests
// ============================================================================

#[test]
fn test_args_get_batch_size_default() {
    let args = Args {
        step: "2".to_string(),
        impl_files: None,
        tasks_overview: None,
        tasks: None,
        stream: false,
        debug: false,
        batch_size: None,
        tasks_overview_template: None,
        task_template: Some("t.yaml".to_string()),
        workflow_metadata: false,
    };

    assert_eq!(args.get_batch_size(), Some(5));
}

#[test]
fn test_args_get_batch_size_custom() {
    let args = Args {
        step: "2".to_string(),
        impl_files: None,
        tasks_overview: None,
        tasks: None,
        stream: false,
        debug: false,
        batch_size: Some(10),
        tasks_overview_template: None,
        task_template: Some("t.yaml".to_string()),
        workflow_metadata: false,
    };

    assert_eq!(args.get_batch_size(), Some(10));
}

#[test]
fn test_args_use_ai_execution_planning_true() {
    let args = Args {
        step: "2".to_string(),
        impl_files: None,
        tasks_overview: None,
        tasks: None,
        stream: false,
        debug: false,
        batch_size: None, // No batch size means use AI
        tasks_overview_template: None,
        task_template: Some("t.yaml".to_string()),
        workflow_metadata: false,
    };

    assert!(args.use_ai_execution_planning());
}

#[test]
fn test_args_use_ai_execution_planning_false() {
    let args = Args {
        step: "2".to_string(),
        impl_files: None,
        tasks_overview: None,
        tasks: None,
        stream: false,
        debug: false,
        batch_size: Some(3), // Batch size specified means no AI
        tasks_overview_template: None,
        task_template: Some("t.yaml".to_string()),
        workflow_metadata: false,
    };

    assert!(!args.use_ai_execution_planning());
}

// ============================================================================
// WorkflowConfig Tests
// ============================================================================

#[test]
fn test_workflow_config_from_args_basic() {
    let args = Args {
        step: "all".to_string(),
        impl_files: Some(vec!["IMPL.md".to_string()]),
        tasks_overview: None,
        tasks: None,
        stream: false,
        debug: true,
        batch_size: Some(5),
        tasks_overview_template: Some("overview.yaml".to_string()),
        task_template: Some("task.yaml".to_string()),
        workflow_metadata: false,
    };

    let config: WorkflowConfig = args.into();

    assert_eq!(config.step, "all");
    assert_eq!(config.debug, true);
    assert_eq!(config.batch_size, 5);
    assert_eq!(config.use_ai_planning, false); // batch_size is set
}

#[test]
fn test_workflow_config_from_args_ai_planning() {
    let args = Args {
        step: "2".to_string(),
        impl_files: None,
        tasks_overview: None,
        tasks: None,
        stream: true,
        debug: false,
        batch_size: None, // No batch size
        tasks_overview_template: None,
        task_template: Some("task.yaml".to_string()),
        workflow_metadata: false,
    };

    let config: WorkflowConfig = args.into();

    assert_eq!(config.use_ai_planning, true); // No batch_size means AI planning
    assert_eq!(config.stream, true);
}

#[test]
fn test_workflow_config_default_paths() {
    let args = Args {
        step: "2".to_string(),
        impl_files: None,
        tasks_overview: None, // Should default to tasks_overview.yaml
        tasks: None,          // Should default to tasks.yaml
        stream: false,
        debug: false,
        batch_size: None,
        tasks_overview_template: None,
        task_template: Some("task.yaml".to_string()),
        workflow_metadata: false,
    };

    let config: WorkflowConfig = args.into();

    assert!(config.tasks_overview_path.ends_with("tasks_overview.yaml"));
    assert!(config.tasks_path.ends_with("tasks.yaml"));
    assert!(config.review_report_path.ends_with("review_report.txt"));
}

#[test]
fn test_workflow_config_custom_paths() {
    let args = Args {
        step: "3".to_string(),
        impl_files: Some(vec!["custom_impl.md".to_string()]),
        tasks_overview: Some("/custom/overview.yaml".to_string()),
        tasks: Some("/custom/tasks.yaml".to_string()),
        stream: false,
        debug: false,
        batch_size: Some(3),
        tasks_overview_template: Some("t1.yaml".to_string()),
        task_template: Some("t2.yaml".to_string()),
        workflow_metadata: false,
    };

    let config: WorkflowConfig = args.into();

    assert_eq!(
        config.tasks_overview_path.to_str().unwrap(),
        "/custom/overview.yaml"
    );
    assert_eq!(config.tasks_path.to_str().unwrap(), "/custom/tasks.yaml");
}

#[test]
fn test_workflow_config_template_paths() {
    let args = Args {
        step: "all".to_string(),
        impl_files: None,
        tasks_overview: None,
        tasks: None,
        stream: false,
        debug: false,
        batch_size: None,
        tasks_overview_template: Some("templates/overview.yaml".to_string()),
        task_template: Some("templates/task.yaml".to_string()),
        workflow_metadata: false,
    };

    let config: WorkflowConfig = args.into();

    assert_eq!(
        config.tasks_overview_template.as_ref().unwrap(),
        "templates/overview.yaml"
    );
    assert_eq!(
        config.task_template.as_ref().unwrap(),
        "templates/task.yaml"
    );
}

// ============================================================================
// Args Clone Tests
// ============================================================================

#[test]
fn test_args_clone() {
    let args = Args {
        step: "all".to_string(),
        impl_files: Some(vec!["impl.md".to_string()]),
        tasks_overview: Some("overview.yaml".to_string()),
        tasks: Some("tasks.yaml".to_string()),
        stream: true,
        debug: true,
        batch_size: Some(7),
        tasks_overview_template: Some("t1.yaml".to_string()),
        task_template: Some("t2.yaml".to_string()),
        workflow_metadata: false,
    };

    let cloned = args.clone();

    assert_eq!(args.step, cloned.step);
    assert_eq!(args.batch_size, cloned.batch_size);
    assert_eq!(args.debug, cloned.debug);
    assert_eq!(args.stream, cloned.stream);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_workflow_config_multiple_impl_files() {
    let args = Args {
        step: "1".to_string(),
        impl_files: Some(vec![
            "impl1.md".to_string(),
            "impl2.md".to_string(),
            "impl3.md".to_string(),
        ]),
        tasks_overview: None,
        tasks: None,
        stream: false,
        debug: false,
        batch_size: None,
        tasks_overview_template: Some("t.yaml".to_string()),
        task_template: None,
        workflow_metadata: false,
    };

    let config: WorkflowConfig = args.into();

    assert_eq!(config.impl_files.as_ref().unwrap().len(), 3);
}

#[test]
fn test_workflow_config_batch_size_zero() {
    let args = Args {
        step: "2".to_string(),
        impl_files: None,
        tasks_overview: None,
        tasks: None,
        stream: false,
        debug: false,
        batch_size: Some(0), // Edge case
        tasks_overview_template: None,
        task_template: Some("t.yaml".to_string()),
        workflow_metadata: false,
    };

    let config: WorkflowConfig = args.into();
    assert_eq!(config.batch_size, 0);
}

#[test]
fn test_workflow_config_stream_enabled() {
    let args = Args {
        step: "2".to_string(),
        impl_files: None,
        tasks_overview: None,
        tasks: None,
        stream: true,
        debug: false,
        batch_size: Some(5),
        tasks_overview_template: None,
        task_template: Some("t.yaml".to_string()),
        workflow_metadata: false,
    };

    let config: WorkflowConfig = args.into();
    assert!(config.stream);
}

#[test]
fn test_args_debug_flag() {
    let args = Args {
        step: "all".to_string(),
        impl_files: None,
        tasks_overview: None,
        tasks: None,
        stream: false,
        debug: true,
        batch_size: None,
        tasks_overview_template: Some("t1.yaml".to_string()),
        task_template: Some("t2.yaml".to_string()),
        workflow_metadata: false,
    };

    assert!(args.debug);
    let config: WorkflowConfig = args.into();
    assert!(config.debug);
}

#[test]
fn test_args_workflow_metadata_flag() {
    let args = Args {
        step: "all".to_string(),
        impl_files: None,
        tasks_overview: None,
        tasks: None,
        stream: false,
        debug: false,
        batch_size: None,
        tasks_overview_template: Some("t1.yaml".to_string()),
        task_template: Some("t2.yaml".to_string()),
        workflow_metadata: true,
    };

    assert!(args.workflow_metadata);
}
