//! Common utilities for research module tests

use std::path::PathBuf;
use std::env;

/// Get the project root directory for testing
pub fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Create a temporary test directory
pub fn create_temp_dir(name: &str) -> PathBuf {
    let temp_dir = env::temp_dir().join(format!("workflow_manager_test_{}", name));
    std::fs::create_dir_all(&temp_dir).unwrap();
    temp_dir
}

/// Clean up a temporary directory
pub fn cleanup_temp_dir(path: &PathBuf) {
    if path.exists() {
        let _ = std::fs::remove_dir_all(path);
    }
}
