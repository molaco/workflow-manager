//! Common test utilities for task planner tests

use std::path::PathBuf;
use workflow_manager::task_planner::types::*;

/// Create a temporary directory for testing
pub fn create_temp_dir(name: &str) -> PathBuf {
    let temp_dir = std::env::temp_dir().join(format!("task_planner_test_{}", name));
    std::fs::create_dir_all(&temp_dir).unwrap();
    temp_dir
}

/// Clean up temporary directory
pub fn cleanup_temp_dir(path: &PathBuf) {
    if path.exists() {
        std::fs::remove_dir_all(path).ok();
    }
}

/// Create sample TaskOverview for testing
pub fn sample_task_overview() -> TaskOverview {
    TaskOverview {
        task: TaskInfo {
            id: 1,
            name: "Sample Task".to_string(),
            context: "This is a sample task for testing".to_string(),
        },
        dependencies: Dependencies {
            requires_completion_of: vec![],
        },
    }
}

/// Create TaskOverview with dependencies
pub fn sample_task_with_deps(id: u32, name: &str, deps: Vec<u32>) -> TaskOverview {
    TaskOverview {
        task: TaskInfo {
            id,
            name: name.to_string(),
            context: format!("Task {} context", id),
        },
        dependencies: Dependencies {
            requires_completion_of: deps
                .into_iter()
                .map(|dep_id| TaskDependency {
                    task_id: dep_id,
                    reason: format!("Depends on task {}", dep_id),
                })
                .collect(),
        },
    }
}

/// Create sample DetailedTask for testing
pub fn sample_detailed_task() -> DetailedTask {
    DetailedTask {
        task: TaskInfo {
            id: 1,
            name: "Detailed Task".to_string(),
            context: "This is a detailed task specification".to_string(),
        },
        files: vec![FileSpec {
            path: "src/example.rs".to_string(),
            description: "Example implementation file".to_string(),
        }],
        functions: vec![FunctionGroup {
            file: "src/example.rs".to_string(),
            items: vec![CodeItem {
                item_type: "function".to_string(),
                name: "example_function".to_string(),
                description: "An example function".to_string(),
                preconditions: Some("Input must be valid".to_string()),
                postconditions: Some("Returns valid result".to_string()),
                invariants: None,
            }],
        }],
        formal_verification: FormalVerification {
            needed: false,
            level: "None".to_string(),
            explanation: "Simple function doesn't require formal verification".to_string(),
            system_prompt: None,
            properties: None,
            strategy: None,
        },
        tests: TestSpec {
            strategy: TestStrategy {
                approach: "Unit tests".to_string(),
                rationale: vec!["Test core functionality".to_string()],
            },
            implementation: TestImplementation {
                file: "tests/example_test.rs".to_string(),
                location: "Create new file".to_string(),
                code: "#[test]\nfn test_example() {\n    assert!(true);\n}".to_string(),
            },
            coverage: vec!["Basic functionality".to_string()],
        },
        dependencies: Dependencies::default(),
    }
}

/// Create sample UsageStats for testing
pub fn sample_usage_stats() -> UsageStats {
    UsageStats {
        duration_ms: 1500,
        duration_api_ms: Some(1200),
        num_turns: 2,
        total_cost_usd: Some(0.05),
        usage: TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
        },
        session_id: Some("test-session-123".to_string()),
    }
}

/// Create sample ExecutionPlan for testing
pub fn sample_execution_plan() -> ExecutionPlan {
    ExecutionPlan {
        total_tasks: 3,
        total_batches: 2,
        batches: vec![
            Batch {
                batch_id: 1,
                description: "First batch".to_string(),
                strategy: "sequential".to_string(),
                tasks: vec![
                    BatchTask {
                        task_id: 1,
                        task_name: "Task 1".to_string(),
                        reason: "No dependencies".to_string(),
                    },
                    BatchTask {
                        task_id: 2,
                        task_name: "Task 2".to_string(),
                        reason: "No dependencies".to_string(),
                    },
                ],
                parallelization_rationale: "Both tasks have no dependencies".to_string(),
            },
            Batch {
                batch_id: 2,
                description: "Second batch".to_string(),
                strategy: "sequential".to_string(),
                tasks: vec![BatchTask {
                    task_id: 3,
                    task_name: "Task 3".to_string(),
                    reason: "Depends on batch 1".to_string(),
                }],
                parallelization_rationale: "Depends on previous batch".to_string(),
            },
        ],
        dependencies_summary: DependenciesSummary {
            critical_path: vec![1, 3],
            parallelization_potential: "medium".to_string(),
            parallelization_explanation: "2 out of 3 tasks can run in parallel".to_string(),
        },
    }
}

/// Create sample ReviewResult for testing
pub fn sample_review_result(success: bool) -> ReviewResult {
    ReviewResult {
        task_id: 1,
        success,
        issues: if success {
            vec![]
        } else {
            vec!["Missing test coverage".to_string()]
        },
        summary: if success {
            "Task passes all checks".to_string()
        } else {
            "Task has issues that need fixing".to_string()
        },
    }
}
