//! Tests for utility functions
//!
//! Tests file I/O, YAML processing, and parsing utilities

use super::common::*;
use std::io::Write;
use workflow_manager::task_planner::utils::*;

// ============================================================================
// clean_yaml_response Tests
// ============================================================================

#[test]
fn test_clean_yaml_response_with_yaml_fence() {
    let input = "Some text\n```yaml\nkey: value\n```\nMore text";
    let result = clean_yaml_response(input);
    assert_eq!(result, "key: value");
}

#[test]
fn test_clean_yaml_response_with_yaml_fence_multiline() {
    let input = r#"Here is the YAML:
```yaml
task:
  id: 1
  name: Test
```
That's it!"#;
    let cleaned = clean_yaml_response(input);
    assert_eq!(cleaned, "task:\n  id: 1\n  name: Test");
}

#[test]
fn test_clean_yaml_response_with_generic_fence() {
    let input = "```\nkey: value\n```";
    let result = clean_yaml_response(input);
    assert_eq!(result, "key: value");
}

#[test]
fn test_clean_yaml_response_without_fence() {
    let input = "key: value\nother: data";
    let result = clean_yaml_response(input);
    assert_eq!(result, "key: value\nother: data");
}

#[test]
fn test_clean_yaml_response_empty() {
    let input = "";
    let result = clean_yaml_response(input);
    assert_eq!(result, "");
}

#[test]
fn test_clean_yaml_response_only_fence() {
    let input = "```yaml\n```";
    let result = clean_yaml_response(input);
    assert_eq!(result, "");
}

// ============================================================================
// load_template Tests
// ============================================================================

#[test]
fn test_load_template_success() {
    let temp_dir = create_temp_dir("load_template");
    let template_path = temp_dir.join("template.yaml");

    std::fs::write(&template_path, "template: content").unwrap();

    let result = load_template(&template_path).unwrap();
    assert_eq!(result, "template: content");

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn test_load_template_not_found() {
    let temp_dir = create_temp_dir("load_template_missing");
    let template_path = temp_dir.join("nonexistent.yaml");

    let result = load_template(&template_path);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Failed to load template"));

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn test_load_template_multiline() {
    let temp_dir = create_temp_dir("load_template_multi");
    let template_path = temp_dir.join("template.yaml");

    let content = "line1: value1\nline2: value2\nline3: value3";
    std::fs::write(&template_path, content).unwrap();

    let result = load_template(&template_path).unwrap();
    assert_eq!(result, content);

    cleanup_temp_dir(&temp_dir);
}

// ============================================================================
// load_impl_files Tests
// ============================================================================

#[test]
fn test_load_impl_files_single() {
    let temp_dir = create_temp_dir("impl_single");
    let file_path = temp_dir.join("test.md");
    std::fs::write(&file_path, "# Test Content").unwrap();

    let result = load_impl_files(&[file_path.to_str().unwrap().to_string()]).unwrap();
    assert!(result.contains("# Test Content"));
    assert!(!result.contains("Source:")); // Single file doesn't need separator

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn test_load_impl_files_multiple() {
    let temp_dir = create_temp_dir("impl_multi");
    let file1 = temp_dir.join("file1.md");
    let file2 = temp_dir.join("file2.md");
    std::fs::write(&file1, "Content 1").unwrap();
    std::fs::write(&file2, "Content 2").unwrap();

    let result = load_impl_files(&[
        file1.to_str().unwrap().to_string(),
        file2.to_str().unwrap().to_string(),
    ])
    .unwrap();

    assert!(result.contains("Content 1"));
    assert!(result.contains("Content 2"));
    assert!(result.contains("Source: file1.md"));
    assert!(result.contains("Source: file2.md"));
    assert!(result.contains("---")); // Separator

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn test_load_impl_files_not_found() {
    let result = load_impl_files(&["/nonexistent/path/file.md".to_string()]);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_load_impl_files_empty_list() {
    let result = load_impl_files(&[]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
}

// ============================================================================
// save_yaml Tests
// ============================================================================

#[test]
fn test_save_yaml_success() {
    let temp_dir = create_temp_dir("save_yaml");
    let output_path = temp_dir.join("output.yaml");

    let data = "key: value\nother: data";
    save_yaml(data, &output_path).unwrap();

    let content = std::fs::read_to_string(&output_path).unwrap();
    assert_eq!(content, data);

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn test_save_yaml_creates_parent_dir() {
    let temp_dir = create_temp_dir("save_yaml_parent");
    let nested_path = temp_dir.join("nested").join("output.yaml");

    // Create parent directory
    std::fs::create_dir_all(nested_path.parent().unwrap()).unwrap();

    save_yaml("test: data", &nested_path).unwrap();
    assert!(nested_path.exists());

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn test_save_yaml_overwrites_existing() {
    let temp_dir = create_temp_dir("save_yaml_overwrite");
    let output_path = temp_dir.join("output.yaml");

    std::fs::write(&output_path, "old: data").unwrap();
    save_yaml("new: data", &output_path).unwrap();

    let content = std::fs::read_to_string(&output_path).unwrap();
    assert_eq!(content, "new: data");

    cleanup_temp_dir(&temp_dir);
}

// ============================================================================
// parse_tasks_overview Tests
// ============================================================================

#[test]
fn test_parse_tasks_overview_single() {
    let yaml = r#"
task:
  id: 1
  name: "Test Task"
  context: "Test context"
dependencies:
  requires_completion_of: []
"#;
    let tasks = parse_tasks_overview(yaml).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].task.id, 1);
    assert_eq!(tasks[0].task.name, "Test Task");
}

#[test]
fn test_parse_tasks_overview_multi_document() {
    let yaml = r#"
task:
  id: 1
  name: "Task 1"
  context: "First"
dependencies:
  requires_completion_of: []
---
task:
  id: 2
  name: "Task 2"
  context: "Second"
dependencies:
  requires_completion_of: []
"#;
    let tasks = parse_tasks_overview(yaml).unwrap();
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].task.id, 1);
    assert_eq!(tasks[1].task.id, 2);
}

#[test]
fn test_parse_tasks_overview_with_dependencies() {
    let yaml = r#"
task:
  id: 2
  name: "Task 2"
  context: "Depends on task 1"
dependencies:
  requires_completion_of:
    - task_id: 1
      reason: "Needs task 1 to complete first"
"#;
    let tasks = parse_tasks_overview(yaml).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].dependencies.requires_completion_of.len(), 1);
    assert_eq!(tasks[0].dependencies.requires_completion_of[0].task_id, 1);
}

#[test]
fn test_parse_tasks_overview_invalid_yaml() {
    let yaml = "invalid: yaml: content: broken";
    let result = parse_tasks_overview(yaml);
    assert!(result.is_err());
}

#[test]
fn test_parse_tasks_overview_empty() {
    let yaml = "";
    let result = parse_tasks_overview(yaml);
    assert!(result.is_err());
}

// ============================================================================
// parse_detailed_tasks Tests
// ============================================================================

#[test]
fn test_parse_detailed_tasks_single() {
    let task = sample_detailed_task();
    let yaml = serde_yaml::to_string(&task).unwrap();

    let tasks = parse_detailed_tasks(&yaml).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].task.id, 1);
}

#[test]
fn test_parse_detailed_tasks_multiple() {
    let task1 = sample_detailed_task();
    let mut task2 = sample_detailed_task();
    task2.task.id = 2;
    task2.task.name = "Task 2".to_string();

    let yaml1 = serde_yaml::to_string(&task1).unwrap();
    let yaml2 = serde_yaml::to_string(&task2).unwrap();
    let combined = format!("{}\n---\n{}", yaml1, yaml2);

    let tasks = parse_detailed_tasks(&combined).unwrap();
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].task.id, 1);
    assert_eq!(tasks[1].task.id, 2);
}

#[test]
fn test_parse_detailed_tasks_complex_structure() {
    let yaml = r#"
task:
  id: 5
  name: "Complex Task"
  context: "A complex task"
files:
  - path: "src/main.rs"
    description: "Main file"
  - path: "src/lib.rs"
    description: "Library file"
functions:
  - file: "src/main.rs"
    items:
      - type: "function"
        name: "main"
        description: "Entry point"
formal_verification:
  needed: true
  level: "Basic"
  explanation: "Verify core logic"
tests:
  strategy:
    approach: "Unit tests"
    rationale:
      - "Fast feedback"
  implementation:
    file: "tests/test.rs"
    location: "Create new"
    code: "test code"
  coverage:
    - "Core functionality"
dependencies:
  requires_completion_of: []
"#;

    let tasks = parse_detailed_tasks(yaml).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].files.len(), 2);
    assert!(tasks[0].formal_verification.needed);
}

#[test]
fn test_parse_detailed_tasks_invalid() {
    let yaml = "not: a: valid: task";
    let result = parse_detailed_tasks(yaml);
    assert!(result.is_err());
}

// ============================================================================
// Edge Cases and Error Handling
// ============================================================================

#[test]
fn test_load_impl_files_with_empty_file() {
    let temp_dir = create_temp_dir("impl_empty");
    let file_path = temp_dir.join("empty.md");
    std::fs::write(&file_path, "").unwrap();

    let result = load_impl_files(&[file_path.to_str().unwrap().to_string()]).unwrap();
    assert_eq!(result, "");

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn test_clean_yaml_response_nested_fences() {
    let input = r#"
Outer text
```yaml
outer: value
```
More text
"#;
    let result = clean_yaml_response(input);
    assert_eq!(result.trim(), "outer: value");
}

#[test]
fn test_save_yaml_empty_content() {
    let temp_dir = create_temp_dir("save_empty");
    let output_path = temp_dir.join("empty.yaml");

    save_yaml("", &output_path).unwrap();
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert_eq!(content, "");

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn test_parse_tasks_overview_default_dependencies() {
    let yaml = r#"
task:
  id: 1
  name: "Task"
  context: "Context"
"#;
    // Should work even without explicit dependencies field
    let tasks = parse_tasks_overview(yaml).unwrap();
    assert_eq!(tasks.len(), 1);
    assert!(tasks[0].dependencies.requires_completion_of.is_empty());
}
