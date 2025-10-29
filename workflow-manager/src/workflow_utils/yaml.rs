//! YAML utilities for extraction, parsing, and validation

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;

/// Extract YAML content from markdown code blocks or raw text
///
/// Handles:
/// - ```yaml blocks
/// - Generic ``` blocks
/// - Raw YAML text
/// - Removes leading document separator (---)
pub fn extract_yaml(text: &str) -> String {
    let yaml = if text.contains("```yaml") {
        // Extract from ```yaml block
        let yaml_start = text.find("```yaml").unwrap() + 7;
        let yaml_end = text[yaml_start..]
            .rfind("```")
            .map(|pos| pos + yaml_start)
            .unwrap_or(text.len());
        text[yaml_start..yaml_end].trim().to_string()
    } else if text.contains("```") {
        // Extract from generic ``` block
        let yaml_start = text.find("```").unwrap() + 3;
        let yaml_end = text[yaml_start..]
            .rfind("```")
            .map(|pos| pos + yaml_start)
            .unwrap_or(text.len());
        text[yaml_start..yaml_end].trim().to_string()
    } else {
        // Assume raw YAML
        text.trim().to_string()
    };

    clean_yaml(&yaml)
}

/// Clean YAML by removing document separators and normalizing whitespace
///
/// Removes:
/// - Leading `---` document separator
/// - Trailing whitespace
pub fn clean_yaml(yaml: &str) -> String {
    yaml.trim_start_matches("---").trim().to_string()
}

/// Parse YAML string into a typed structure with better error messages
///
/// Provides helpful context when parsing fails, including:
/// - Duplicate key detection
/// - Syntax error hints
/// - Preview of problematic content
pub fn parse_yaml<T: DeserializeOwned>(yaml: &str) -> Result<T> {
    serde_yaml::from_str(yaml).context("Failed to parse YAML").map_err(|e| {
        let error_msg = format!("YAML parsing error: {}", e);

        // Provide additional context for common errors
        if error_msg.contains("duplicate") {
            eprintln!("\n❌ YAML PARSING ERROR: Duplicate keys detected");
            eprintln!("The YAML contains duplicate keys which is invalid.");
            eprintln!("\nYAML preview (first 500 chars):");
            eprintln!("{}", &yaml.chars().take(500).collect::<String>());
        } else if error_msg.contains("expected") {
            eprintln!("\n❌ YAML SYNTAX ERROR");
            eprintln!("{}", e);
            eprintln!("\nYAML preview (first 500 chars):");
            eprintln!("{}", &yaml.chars().take(500).collect::<String>());
        }

        e.into()
    })
}

/// Parse multi-document YAML (documents separated by ---)
///
/// Each document is parsed independently and returned as a vector.
/// Empty documents are skipped.
pub fn parse_yaml_multi<T: DeserializeOwned>(text: &str) -> Result<Vec<T>> {
    let documents: Vec<&str> = text.split("---").collect();
    let mut results = Vec::new();

    for (idx, doc) in documents.iter().enumerate() {
        let doc = doc.trim();
        if doc.is_empty() {
            continue;
        }

        let parsed = parse_yaml::<T>(doc)
            .with_context(|| format!("Failed to parse document {}", idx))?;
        results.push(parsed);
    }

    Ok(results)
}

/// Validate YAML syntax without parsing into a specific type
///
/// Returns Ok(()) if valid, Err with details if invalid.
pub fn validate_yaml_syntax(yaml: &str) -> Result<()> {
    serde_yaml::from_str::<serde_yaml::Value>(yaml)
        .context("YAML syntax validation failed")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestData {
        title: String,
        count: usize,
    }

    #[test]
    fn test_extract_yaml_from_markdown() {
        let text = r#"
Some text here

```yaml
title: Test
count: 42
```

More text
        "#;

        let yaml = extract_yaml(text);
        assert!(yaml.contains("title: Test"));
        assert!(yaml.contains("count: 42"));
        assert!(!yaml.contains("```"));
    }

    #[test]
    fn test_extract_yaml_generic_block() {
        let text = r#"
```
title: Test
count: 42
```
        "#;

        let yaml = extract_yaml(text);
        assert!(yaml.contains("title: Test"));
    }

    #[test]
    fn test_extract_yaml_raw() {
        let text = r#"
---
title: Test
count: 42
        "#;

        let yaml = extract_yaml(text);
        assert!(!yaml.starts_with("---"));
        assert!(yaml.contains("title: Test"));
    }

    #[test]
    fn test_parse_yaml() {
        let yaml = r#"
title: Test
count: 42
        "#;

        let data: TestData = parse_yaml(yaml).unwrap();
        assert_eq!(data.title, "Test");
        assert_eq!(data.count, 42);
    }

    #[test]
    fn test_parse_yaml_multi() {
        let yaml = r#"
---
title: First
count: 1
---
title: Second
count: 2
        "#;

        let docs: Vec<TestData> = parse_yaml_multi(yaml).unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].title, "First");
        assert_eq!(docs[1].title, "Second");
    }

    #[test]
    fn test_validate_yaml_syntax() {
        assert!(validate_yaml_syntax("title: Test").is_ok());
        assert!(validate_yaml_syntax("invalid: [unclosed").is_err());
    }
}
