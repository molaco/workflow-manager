//! Unit tests for helper functions

use workflow_manager::research::phase3_validate::{find_yaml_files};
use std::fs;

use super::common::{create_temp_dir, cleanup_temp_dir};

#[tokio::test]
async fn test_find_yaml_files_basic() {
    // Create temp directory with test YAML files
    let temp_dir = create_temp_dir("yaml_test_basic");

    // Create test files
    fs::write(temp_dir.join("test1.yaml"), "key: value").unwrap();
    fs::write(temp_dir.join("test2.yml"), "key: value").unwrap();
    fs::write(temp_dir.join("test.txt"), "not yaml").unwrap();

    // Test find_yaml_files
    let yaml_files = find_yaml_files(temp_dir.to_str().unwrap()).await.unwrap();

    // Should find 2 YAML files
    assert_eq!(yaml_files.len(), 2, "Expected 2 YAML files");
    assert!(yaml_files.iter().any(|f| f.ends_with("test1.yaml")));
    assert!(yaml_files.iter().any(|f| f.ends_with("test2.yml")));
    assert!(!yaml_files.iter().any(|f| f.ends_with("test.txt")));

    // Cleanup
    cleanup_temp_dir(&temp_dir);
}

#[tokio::test]
async fn test_find_yaml_files_empty_directory() {
    // Create empty temp directory
    let temp_dir = create_temp_dir("yaml_test_empty");

    // Test find_yaml_files on empty directory
    let yaml_files = find_yaml_files(temp_dir.to_str().unwrap()).await.unwrap();

    // Should find no YAML files
    assert_eq!(yaml_files.len(), 0, "Expected no YAML files in empty directory");

    // Cleanup
    cleanup_temp_dir(&temp_dir);
}

#[tokio::test]
async fn test_find_yaml_files_no_yaml_files() {
    // Create temp directory with non-YAML files
    let temp_dir = create_temp_dir("yaml_test_no_yaml");

    // Create test files that are not YAML
    fs::write(temp_dir.join("test.txt"), "text file").unwrap();
    fs::write(temp_dir.join("test.json"), "{}").unwrap();
    fs::write(temp_dir.join("test.md"), "# Markdown").unwrap();

    // Test find_yaml_files
    let yaml_files = find_yaml_files(temp_dir.to_str().unwrap()).await.unwrap();

    // Should find no YAML files
    assert_eq!(yaml_files.len(), 0, "Expected no YAML files");

    // Cleanup
    cleanup_temp_dir(&temp_dir);
}

#[tokio::test]
async fn test_find_yaml_files_sorted() {
    // Create temp directory with multiple YAML files
    let temp_dir = create_temp_dir("yaml_test_sorted");

    // Create files in non-alphabetical order
    fs::write(temp_dir.join("charlie.yaml"), "key: value").unwrap();
    fs::write(temp_dir.join("alice.yaml"), "key: value").unwrap();
    fs::write(temp_dir.join("bob.yml"), "key: value").unwrap();

    // Test find_yaml_files
    let yaml_files = find_yaml_files(temp_dir.to_str().unwrap()).await.unwrap();

    // Should be sorted alphabetically
    assert_eq!(yaml_files.len(), 3);
    assert!(yaml_files[0].ends_with("alice.yaml"));
    assert!(yaml_files[1].ends_with("bob.yml"));
    assert!(yaml_files[2].ends_with("charlie.yaml"));

    // Cleanup
    cleanup_temp_dir(&temp_dir);
}

#[tokio::test]
async fn test_find_yaml_files_invalid_directory() {
    // Test with non-existent directory
    let result = find_yaml_files("/nonexistent/directory/path").await;

    // Should return an error
    assert!(result.is_err(), "Expected error for non-existent directory");
}
