//! Tests for research data types

use workflow_manager::research::{
    CodebaseAnalysis, PromptsData, ResearchPrompt, ResearchResult,
};

#[test]
fn test_research_prompt_creation() {
    let prompt = ResearchPrompt {
        title: "Test Prompt".to_string(),
        query: "What is this?".to_string(),
        focus: vec!["Focus area".to_string()],
    };

    assert_eq!(prompt.title, "Test Prompt");
    assert_eq!(prompt.query, "What is this?");
    assert_eq!(prompt.focus, vec!["Focus area".to_string()]);
}

#[test]
fn test_research_prompt_empty_focus() {
    let prompt = ResearchPrompt {
        title: "Test Prompt".to_string(),
        query: "What is this?".to_string(),
        focus: vec![],
    };

    assert_eq!(prompt.title, "Test Prompt");
    assert_eq!(prompt.query, "What is this?");
    assert!(prompt.focus.is_empty());
}

#[test]
fn test_prompts_data_creation() {
    let prompts_data = PromptsData {
        objective: "Test objective".to_string(),
        prompts: vec![
            ResearchPrompt {
                title: "Prompt 1".to_string(),
                query: "Query 1".to_string(),
                focus: vec![],
            },
        ],
    };

    assert_eq!(prompts_data.objective, "Test objective");
    assert_eq!(prompts_data.prompts.len(), 1);
    assert_eq!(prompts_data.prompts[0].title, "Prompt 1");
}

#[test]
fn test_prompts_data_multiple_prompts() {
    let prompts_data = PromptsData {
        objective: "Test objective".to_string(),
        prompts: vec![
            ResearchPrompt {
                title: "Prompt 1".to_string(),
                query: "Query 1".to_string(),
                focus: vec!["Auth".to_string()],
            },
            ResearchPrompt {
                title: "Prompt 2".to_string(),
                query: "Query 2".to_string(),
                focus: vec!["API".to_string()],
            },
        ],
    };

    assert_eq!(prompts_data.prompts.len(), 2);
    assert_eq!(prompts_data.prompts[0].title, "Prompt 1");
    assert_eq!(prompts_data.prompts[1].title, "Prompt 2");
}

#[test]
fn test_research_result_creation() {
    let result = ResearchResult {
        title: "Result 1".to_string(),
        query: "Query 1".to_string(),
        response_file: "output/result1.yaml".to_string(),
        focus: vec!["Authentication".to_string()],
    };

    assert_eq!(result.title, "Result 1");
    assert_eq!(result.query, "Query 1");
    assert_eq!(result.response_file, "output/result1.yaml");
    assert_eq!(result.focus, vec!["Authentication".to_string()]);
}

// Test serialization and deserialization
#[test]
fn test_research_prompt_yaml_serialization() {
    let prompt = ResearchPrompt {
        title: "Test".to_string(),
        query: "Query".to_string(),
        focus: vec!["Focus1".to_string(), "Focus2".to_string()],
    };

    // Test YAML serialization
    let yaml = serde_yaml::to_string(&prompt).unwrap();
    assert!(yaml.contains("title"));
    assert!(yaml.contains("Test"));
    assert!(yaml.contains("query"));
    assert!(yaml.contains("Query"));
    assert!(yaml.contains("focus"));

    // Test deserialization
    let deserialized: ResearchPrompt = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(deserialized.title, prompt.title);
    assert_eq!(deserialized.query, prompt.query);
    assert_eq!(deserialized.focus, prompt.focus);
}

#[test]
fn test_prompts_data_yaml_serialization() {
    let prompts_data = PromptsData {
        objective: "Test objective".to_string(),
        prompts: vec![
            ResearchPrompt {
                title: "Prompt 1".to_string(),
                query: "Query 1".to_string(),
                focus: vec!["Auth".to_string()],
            },
        ],
    };

    // Test YAML serialization
    let yaml = serde_yaml::to_string(&prompts_data).unwrap();
    assert!(yaml.contains("objective"));
    assert!(yaml.contains("Test objective"));
    assert!(yaml.contains("prompts"));

    // Test deserialization
    let deserialized: PromptsData = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(deserialized.objective, prompts_data.objective);
    assert_eq!(deserialized.prompts.len(), prompts_data.prompts.len());
}

#[test]
fn test_research_result_yaml_serialization() {
    let result = ResearchResult {
        title: "Result 1".to_string(),
        query: "Query 1".to_string(),
        response_file: "output/result1.yaml".to_string(),
        focus: vec!["Authentication".to_string()],
    };

    // Test YAML serialization
    let yaml = serde_yaml::to_string(&result).unwrap();
    assert!(yaml.contains("title"));
    assert!(yaml.contains("Result 1"));
    assert!(yaml.contains("response_file"));

    // Test deserialization
    let deserialized: ResearchResult = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(deserialized.title, result.title);
    assert_eq!(deserialized.query, result.query);
    assert_eq!(deserialized.response_file, result.response_file);
    assert_eq!(deserialized.focus, result.focus);
}

#[test]
fn test_codebase_analysis_yaml_value() {
    // CodebaseAnalysis is a type alias for serde_yaml::Value
    // Test that we can create and serialize it
    let analysis: CodebaseAnalysis = serde_yaml::from_str(
        r#"
        files:
          total: 100
          by_extension:
            rs: 50
            toml: 5
        "#,
    )
    .unwrap();

    // Verify it's a valid YAML value
    assert!(analysis.is_mapping());

    // Test serialization
    let yaml = serde_yaml::to_string(&analysis).unwrap();
    assert!(yaml.contains("files"));
    assert!(yaml.contains("total"));
}

#[test]
fn test_research_prompt_clone() {
    let prompt = ResearchPrompt {
        title: "Test".to_string(),
        query: "Query".to_string(),
        focus: vec!["Focus".to_string()],
    };

    let cloned = prompt.clone();
    assert_eq!(cloned.title, prompt.title);
    assert_eq!(cloned.query, prompt.query);
    assert_eq!(cloned.focus, prompt.focus);
}

#[test]
fn test_research_result_clone() {
    let result = ResearchResult {
        title: "Result".to_string(),
        query: "Query".to_string(),
        response_file: "output/result.yaml".to_string(),
        focus: vec!["Focus".to_string()],
    };

    let cloned = result.clone();
    assert_eq!(cloned.title, result.title);
    assert_eq!(cloned.query, result.query);
    assert_eq!(cloned.response_file, result.response_file);
    assert_eq!(cloned.focus, result.focus);
}
