//! Integration tests for task planner module
//!
//! This test suite provides comprehensive coverage of the task planner module:
//! - Type serialization/deserialization
//! - Utility functions
//! - Execution plan parsing
//! - Workflow configuration
//! - Integration tests

mod task_planner {
    mod common;
    mod test_types;
    mod test_utils;
    mod test_execution_plan;
    mod test_workflow;
    mod test_integration;
}
