//! Tests for all task planner types
//!
//! Tests serialization/deserialization and basic functionality for:
//! - TaskOverview, TaskInfo, Dependencies, TaskDependency
//! - DetailedTask, FileSpec, FunctionGroup, CodeItem
//! - FormalVerification, TestSpec, TestStrategy, TestImplementation
//! - ExecutionPlan, Batch, BatchTask, DependenciesSummary
//! - UsageStats, TokenUsage
//! - ReviewResult

use super::common::*;
use workflow_manager::task_planner::types::*;

// ============================================================================
// TaskOverview Tests
// ============================================================================

#[test]
fn test_task_overview_creation() {
    let task = sample_task_overview();
    assert_eq!(task.task.id, 1);
    assert_eq!(task.task.name, "Sample Task");
    assert_eq!(task.task.context, "This is a sample task for testing");
    assert!(task.dependencies.requires_completion_of.is_empty());
}

#[test]
fn test_task_overview_yaml_serialization() {
    let task = sample_task_overview();
    let yaml = serde_yaml::to_string(&task).unwrap();
    assert!(yaml.contains("id:"));
    assert!(yaml.contains("name:"));
    assert!(yaml.contains("context:"));
    assert!(yaml.contains("dependencies:"));
}

#[test]
fn test_task_overview_yaml_deserialization() {
    let yaml = r#"
task:
  id: 42
  name: "Test Task"
  context: "Test context"
dependencies:
  requires_completion_of: []
"#;
    let task: TaskOverview = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(task.task.id, 42);
    assert_eq!(task.task.name, "Test Task");
}

#[test]
fn test_task_overview_yaml_roundtrip() {
    let original = sample_task_overview();
    let yaml = serde_yaml::to_string(&original).unwrap();
    let deserialized: TaskOverview = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(original.task.id, deserialized.task.id);
    assert_eq!(original.task.name, deserialized.task.name);
    assert_eq!(original.task.context, deserialized.task.context);
}

#[test]
fn test_task_overview_clone() {
    let task = sample_task_overview();
    let cloned = task.clone();
    assert_eq!(task.task.id, cloned.task.id);
    assert_eq!(task.task.name, cloned.task.name);
}

// ============================================================================
// TaskInfo Tests
// ============================================================================

#[test]
fn test_task_info_creation() {
    let info = TaskInfo {
        id: 123,
        name: "Test Task Info".to_string(),
        context: "Context info".to_string(),
    };
    assert_eq!(info.id, 123);
    assert_eq!(info.name, "Test Task Info");
}

#[test]
fn test_task_info_serialization() {
    let info = TaskInfo {
        id: 1,
        name: "Test".to_string(),
        context: "Context".to_string(),
    };
    let yaml = serde_yaml::to_string(&info).unwrap();
    assert!(yaml.contains("id: 1"));
    assert!(yaml.contains("name: Test"));
}

// ============================================================================
// Dependencies Tests
// ============================================================================

#[test]
fn test_dependencies_default() {
    let deps = Dependencies::default();
    assert!(deps.requires_completion_of.is_empty());
}

#[test]
fn test_dependencies_with_items() {
    let deps = Dependencies {
        requires_completion_of: vec![
            TaskDependency {
                task_id: 1,
                reason: "Needs task 1".to_string(),
            },
            TaskDependency {
                task_id: 2,
                reason: "Needs task 2".to_string(),
            },
        ],
    };
    assert_eq!(deps.requires_completion_of.len(), 2);
    assert_eq!(deps.requires_completion_of[0].task_id, 1);
}

#[test]
fn test_dependencies_serialization() {
    let deps = Dependencies {
        requires_completion_of: vec![TaskDependency {
            task_id: 5,
            reason: "Test dependency".to_string(),
        }],
    };
    let yaml = serde_yaml::to_string(&deps).unwrap();
    assert!(yaml.contains("task_id: 5"));
    assert!(yaml.contains("reason: Test dependency"));
}

// ============================================================================
// TaskDependency Tests
// ============================================================================

#[test]
fn test_task_dependency_creation() {
    let dep = TaskDependency {
        task_id: 42,
        reason: "Required for functionality".to_string(),
    };
    assert_eq!(dep.task_id, 42);
    assert_eq!(dep.reason, "Required for functionality");
}

#[test]
fn test_task_dependency_roundtrip() {
    let dep = TaskDependency {
        task_id: 7,
        reason: "Test".to_string(),
    };
    let yaml = serde_yaml::to_string(&dep).unwrap();
    let deserialized: TaskDependency = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(dep.task_id, deserialized.task_id);
    assert_eq!(dep.reason, deserialized.reason);
}

// ============================================================================
// DetailedTask Tests
// ============================================================================

#[test]
fn test_detailed_task_creation() {
    let task = sample_detailed_task();
    assert_eq!(task.task.id, 1);
    assert_eq!(task.files.len(), 1);
    assert_eq!(task.functions.len(), 1);
    assert!(!task.formal_verification.needed);
}

#[test]
fn test_detailed_task_serialization() {
    let task = sample_detailed_task();
    let yaml = serde_yaml::to_string(&task).unwrap();
    assert!(yaml.contains("id:"));
    assert!(yaml.contains("files:"));
    assert!(yaml.contains("functions:"));
    assert!(yaml.contains("formal_verification:"));
    assert!(yaml.contains("tests:"));
}

#[test]
fn test_detailed_task_roundtrip() {
    let original = sample_detailed_task();
    let yaml = serde_yaml::to_string(&original).unwrap();
    let deserialized: DetailedTask = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(original.task.id, deserialized.task.id);
    assert_eq!(original.files.len(), deserialized.files.len());
}

// ============================================================================
// FileSpec Tests
// ============================================================================

#[test]
fn test_file_spec_creation() {
    let file = FileSpec {
        path: "src/main.rs".to_string(),
        description: "Main entry point".to_string(),
    };
    assert_eq!(file.path, "src/main.rs");
    assert_eq!(file.description, "Main entry point");
}

#[test]
fn test_file_spec_serialization() {
    let file = FileSpec {
        path: "tests/test.rs".to_string(),
        description: "Test file".to_string(),
    };
    let yaml = serde_yaml::to_string(&file).unwrap();
    assert!(yaml.contains("path:"));
    assert!(yaml.contains("description:"));
}

// ============================================================================
// FunctionGroup Tests
// ============================================================================

#[test]
fn test_function_group_creation() {
    let group = FunctionGroup {
        file: "src/lib.rs".to_string(),
        items: vec![],
    };
    assert_eq!(group.file, "src/lib.rs");
    assert!(group.items.is_empty());
}

#[test]
fn test_function_group_with_items() {
    let group = FunctionGroup {
        file: "src/utils.rs".to_string(),
        items: vec![CodeItem {
            item_type: "function".to_string(),
            name: "helper".to_string(),
            description: "Helper function".to_string(),
            preconditions: None,
            postconditions: None,
            invariants: None,
        }],
    };
    assert_eq!(group.items.len(), 1);
    assert_eq!(group.items[0].name, "helper");
}

// ============================================================================
// CodeItem Tests
// ============================================================================

#[test]
fn test_code_item_creation() {
    let item = CodeItem {
        item_type: "function".to_string(),
        name: "calculate".to_string(),
        description: "Calculates result".to_string(),
        preconditions: Some("x > 0".to_string()),
        postconditions: Some("result >= 0".to_string()),
        invariants: Some("state unchanged".to_string()),
    };
    assert_eq!(item.item_type, "function");
    assert_eq!(item.name, "calculate");
    assert!(item.preconditions.is_some());
}

#[test]
fn test_code_item_without_conditions() {
    let item = CodeItem {
        item_type: "struct".to_string(),
        name: "Config".to_string(),
        description: "Configuration struct".to_string(),
        preconditions: None,
        postconditions: None,
        invariants: None,
    };
    assert!(item.preconditions.is_none());
    assert!(item.postconditions.is_none());
    assert!(item.invariants.is_none());
}

#[test]
fn test_code_item_serialization_skips_none() {
    let item = CodeItem {
        item_type: "function".to_string(),
        name: "test".to_string(),
        description: "Test".to_string(),
        preconditions: None,
        postconditions: None,
        invariants: None,
    };
    let yaml = serde_yaml::to_string(&item).unwrap();
    // Should not contain optional fields when None
    assert!(!yaml.contains("preconditions:"));
    assert!(!yaml.contains("postconditions:"));
    assert!(!yaml.contains("invariants:"));
}

// ============================================================================
// FormalVerification Tests
// ============================================================================

#[test]
fn test_formal_verification_not_needed() {
    let verification = FormalVerification {
        needed: false,
        level: "None".to_string(),
        explanation: "Simple function".to_string(),
        system_prompt: None,
        properties: None,
        strategy: None,
    };
    assert!(!verification.needed);
    assert_eq!(verification.level, "None");
}

#[test]
fn test_formal_verification_needed() {
    let verification = FormalVerification {
        needed: true,
        level: "Critical".to_string(),
        explanation: "Safety-critical code".to_string(),
        system_prompt: Some("Verify safety properties".to_string()),
        properties: Some(vec!["No memory leaks".to_string()]),
        strategy: Some("Model checking".to_string()),
    };
    assert!(verification.needed);
    assert!(verification.properties.is_some());
}

#[test]
fn test_formal_verification_serialization() {
    let verification = FormalVerification {
        needed: true,
        level: "Basic".to_string(),
        explanation: "Test".to_string(),
        system_prompt: None,
        properties: None,
        strategy: None,
    };
    let yaml = serde_yaml::to_string(&verification).unwrap();
    assert!(yaml.contains("needed: true"));
    assert!(yaml.contains("level: Basic"));
}

// ============================================================================
// TestSpec Tests
// ============================================================================

#[test]
fn test_test_spec_creation() {
    let spec = TestSpec {
        strategy: TestStrategy {
            approach: "Unit tests".to_string(),
            rationale: vec!["Fast feedback".to_string()],
        },
        implementation: TestImplementation {
            file: "tests/unit.rs".to_string(),
            location: "Create new".to_string(),
            code: "test code".to_string(),
        },
        coverage: vec!["Core logic".to_string()],
    };
    assert_eq!(spec.strategy.approach, "Unit tests");
    assert_eq!(spec.coverage.len(), 1);
}

#[test]
fn test_test_spec_serialization() {
    let spec = sample_detailed_task().tests;
    let yaml = serde_yaml::to_string(&spec).unwrap();
    assert!(yaml.contains("strategy:"));
    assert!(yaml.contains("implementation:"));
    assert!(yaml.contains("coverage:"));
}

// ============================================================================
// TestStrategy Tests
// ============================================================================

#[test]
fn test_test_strategy_creation() {
    let strategy = TestStrategy {
        approach: "Integration tests".to_string(),
        rationale: vec![
            "Test component interactions".to_string(),
            "Verify system behavior".to_string(),
        ],
    };
    assert_eq!(strategy.rationale.len(), 2);
}

// ============================================================================
// TestImplementation Tests
// ============================================================================

#[test]
fn test_test_implementation_creation() {
    let impl_spec = TestImplementation {
        file: "tests/integration.rs".to_string(),
        location: "Append to existing".to_string(),
        code: "#[test]\nfn test() {}".to_string(),
    };
    assert_eq!(impl_spec.file, "tests/integration.rs");
    assert!(impl_spec.code.contains("#[test]"));
}

// ============================================================================
// ExecutionPlan Tests
// ============================================================================

#[test]
fn test_execution_plan_creation() {
    let plan = sample_execution_plan();
    assert_eq!(plan.total_tasks, 3);
    assert_eq!(plan.total_batches, 2);
    assert_eq!(plan.batches.len(), 2);
}

#[test]
fn test_execution_plan_serialization() {
    let plan = sample_execution_plan();
    let yaml = serde_yaml::to_string(&plan).unwrap();
    assert!(yaml.contains("total_tasks: 3"));
    assert!(yaml.contains("total_batches: 2"));
    assert!(yaml.contains("batches:"));
    assert!(yaml.contains("dependencies_summary:"));
}

#[test]
fn test_execution_plan_roundtrip() {
    let original = sample_execution_plan();
    let yaml = serde_yaml::to_string(&original).unwrap();
    let deserialized: ExecutionPlan = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(original.total_tasks, deserialized.total_tasks);
    assert_eq!(original.batches.len(), deserialized.batches.len());
}

// ============================================================================
// Batch Tests
// ============================================================================

#[test]
fn test_batch_creation() {
    let batch = Batch {
        batch_id: 1,
        description: "First batch".to_string(),
        strategy: "parallel".to_string(),
        tasks: vec![],
        parallelization_rationale: "Independent tasks".to_string(),
    };
    assert_eq!(batch.batch_id, 1);
    assert_eq!(batch.strategy, "parallel");
}

#[test]
fn test_batch_with_tasks() {
    let batch = sample_execution_plan().batches[0].clone();
    assert_eq!(batch.tasks.len(), 2);
    assert_eq!(batch.tasks[0].task_id, 1);
}

// ============================================================================
// BatchTask Tests
// ============================================================================

#[test]
fn test_batch_task_creation() {
    let task = BatchTask {
        task_id: 42,
        task_name: "Task 42".to_string(),
        reason: "Part of first batch".to_string(),
    };
    assert_eq!(task.task_id, 42);
    assert_eq!(task.task_name, "Task 42");
}

// ============================================================================
// DependenciesSummary Tests
// ============================================================================

#[test]
fn test_dependencies_summary_creation() {
    let summary = DependenciesSummary {
        critical_path: vec![1, 3, 5],
        parallelization_potential: "high".to_string(),
        parallelization_explanation: "Many independent tasks".to_string(),
    };
    assert_eq!(summary.critical_path.len(), 3);
    assert_eq!(summary.parallelization_potential, "high");
}

#[test]
fn test_dependencies_summary_serialization() {
    let summary = sample_execution_plan().dependencies_summary;
    let yaml = serde_yaml::to_string(&summary).unwrap();
    assert!(yaml.contains("critical_path:"));
    assert!(yaml.contains("parallelization_potential:"));
}

// ============================================================================
// UsageStats Tests
// ============================================================================

#[test]
fn test_usage_stats_creation() {
    let stats = sample_usage_stats();
    assert_eq!(stats.duration_ms, 1500);
    assert_eq!(stats.num_turns, 2);
    assert_eq!(stats.usage.input_tokens, 1000);
    assert_eq!(stats.usage.output_tokens, 500);
}

#[test]
fn test_usage_stats_serialization() {
    let stats = sample_usage_stats();
    let yaml = serde_yaml::to_string(&stats).unwrap();
    assert!(yaml.contains("duration_ms:"));
    assert!(yaml.contains("num_turns:"));
    assert!(yaml.contains("usage:"));
}

#[test]
fn test_usage_stats_optional_fields() {
    let stats = UsageStats {
        duration_ms: 1000,
        duration_api_ms: None,
        num_turns: 1,
        total_cost_usd: None,
        usage: TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
        },
        session_id: None,
    };
    let yaml = serde_yaml::to_string(&stats).unwrap();
    // Optional fields should be skipped when None
    assert!(!yaml.contains("duration_api_ms:"));
    assert!(!yaml.contains("total_cost_usd:"));
    assert!(!yaml.contains("session_id:"));
}

// ============================================================================
// TokenUsage Tests
// ============================================================================

#[test]
fn test_token_usage_creation() {
    let usage = TokenUsage {
        input_tokens: 2000,
        output_tokens: 1500,
    };
    assert_eq!(usage.input_tokens, 2000);
    assert_eq!(usage.output_tokens, 1500);
}

#[test]
fn test_token_usage_serialization() {
    let usage = TokenUsage {
        input_tokens: 500,
        output_tokens: 300,
    };
    let yaml = serde_yaml::to_string(&usage).unwrap();
    assert!(yaml.contains("input_tokens: 500"));
    assert!(yaml.contains("output_tokens: 300"));
}

// ============================================================================
// ReviewResult Tests
// ============================================================================

#[test]
fn test_review_result_success() {
    let result = sample_review_result(true);
    assert!(result.success);
    assert!(result.issues.is_empty());
    assert!(result.summary.contains("passes"));
}

#[test]
fn test_review_result_failure() {
    let result = sample_review_result(false);
    assert!(!result.success);
    assert!(!result.issues.is_empty());
    assert_eq!(result.issues[0], "Missing test coverage");
}

#[test]
fn test_review_result_serialization() {
    let result = ReviewResult {
        task_id: 7,
        success: true,
        issues: vec![],
        summary: "All good".to_string(),
    };
    let yaml = serde_yaml::to_string(&result).unwrap();
    assert!(yaml.contains("task_id: 7"));
    assert!(yaml.contains("success: true"));
}

#[test]
fn test_review_result_roundtrip() {
    let original = sample_review_result(false);
    let yaml = serde_yaml::to_string(&original).unwrap();
    let deserialized: ReviewResult = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(original.task_id, deserialized.task_id);
    assert_eq!(original.success, deserialized.success);
    assert_eq!(original.issues.len(), deserialized.issues.len());
}
