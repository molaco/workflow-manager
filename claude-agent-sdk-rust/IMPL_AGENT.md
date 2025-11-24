# Implementation Plan: Agent Builder Methods

**Date:** 2025-10-21
**Goal:** Add `agents()` and `add_agent()` builder methods to `ClaudeAgentOptionsBuilder`
**Status:** Planning Complete - Ready for Implementation

---

## Table of Contents

1. [Overview](#overview)
2. [Current State Analysis](#current-state-analysis)
3. [Comparison with Python SDK](#comparison-with-python-sdk)
4. [Implementation Details](#implementation-details)
5. [Testing Strategy](#testing-strategy)
6. [Example Usage](#example-usage)
7. [Recommended Enhancements](#recommended-enhancements)
8. [Success Criteria](#success-criteria)

---

## Overview

### Summary

Add builder methods for the `agents` field in `ClaudeAgentOptionsBuilder` to enable ergonomic configuration of custom agent definitions. This brings the Rust SDK to feature parity with the Python SDK's agent functionality.

### Background

The `ClaudeAgentOptions` struct already has an `agents` field (`Option<HashMap<String, AgentDefinition>>`), but there are no builder methods to configure it. Users must manually construct HashMaps or use struct initialization, which is verbose and not idiomatic.

### Goals

1. ✅ Add `agents()` method to set complete HashMap
2. ✅ Add `add_agent()` method for incremental agent building
3. ✅ Maintain API compatibility with Python SDK
4. ✅ Follow Rust builder pattern conventions
5. ✅ Provide comprehensive tests and documentation

---

## Current State Analysis

### Existing Code Structure

**File:** `src/types.rs`

#### AgentDefinition (lines 639-652)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    /// Agent description
    pub description: String,
    /// Agent system prompt
    pub prompt: String,
    /// Tools available to the agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    /// Model to use for the agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}
```

#### ClaudeAgentOptions.agents field (line 704)

```rust
pub struct ClaudeAgentOptions {
    // ... other fields ...
    /// Custom agent definitions
    pub agents: Option<HashMap<String, AgentDefinition>>,
    // ... other fields ...
}
```

#### ClaudeAgentOptionsBuilder (lines 762-833)

```rust
#[derive(Debug, Default)]
pub struct ClaudeAgentOptionsBuilder {
    options: ClaudeAgentOptions,
}

impl ClaudeAgentOptionsBuilder {
    // Current methods: allowed_tools, system_prompt, mcp_servers, etc.
    // Missing: agents() and add_agent()

    pub fn build(self) -> ClaudeAgentOptions {
        self.options
    }
}
```

### Current Limitation

Users must manually build HashMap:

```rust
let mut agents = HashMap::new();
agents.insert("reviewer".to_string(), AgentDefinition {
    description: "Code reviewer".to_string(),
    prompt: "Review code thoroughly".to_string(),
    tools: Some(vec!["Read".to_string()]),
    model: Some("sonnet".to_string()),
});

let options = ClaudeAgentOptions {
    agents: Some(agents),
    ..Default::default()
};
```

---

## Comparison with Python SDK

### Python SDK Implementation

**File:** `/home/molaco/Documents/claude-agent-sdk-python/src/claude_agent_sdk/types.py`

#### AgentDefinition (lines 30-37)

```python
@dataclass
class AgentDefinition:
    """Agent definition configuration."""

    description: str
    prompt: str
    tools: list[str] | None = None
    model: Literal["sonnet", "opus", "haiku", "inherit"] | None = None
```

#### Usage Pattern (from examples/agents.py:27-38)

```python
options = ClaudeAgentOptions(
    agents={
        "code-reviewer": AgentDefinition(
            description="Reviews code for best practices",
            prompt="You are a code reviewer...",
            tools=["Read", "Grep"],
            model="sonnet",
        ),
    },
)
```

### Key Differences

| Aspect | Python SDK | Rust SDK (Current) | Rust SDK (Proposed) |
|--------|------------|-------------------|---------------------|
| Type Structure | `dict[str, AgentDefinition]` | `Option<HashMap<String, AgentDefinition>>` | Same |
| Configuration | Direct dict literal | Manual HashMap | Builder methods |
| Model Validation | `Literal["sonnet", "opus", "haiku", "inherit"]` | Open `String` | Open `String`* |
| Builder Pattern | ❌ (uses dataclass) | ❌ | ✅ |
| Incremental Add | ❌ | ❌ | ✅ (Rust advantage) |

*See [Recommended Enhancements](#recommended-enhancements) for model enum suggestion

### Feature Parity Matrix

| Feature | Python SDK | Rust SDK (Current) | Rust SDK (Planned) | Notes |
|---------|------------|-------------------|-------------------|-------|
| AgentDefinition struct | ✅ | ✅ | ✅ | Identical structure |
| agents field in Options | ✅ | ✅ | ✅ | Same type semantics |
| Dictionary/HashMap init | ✅ | ✅ | ✅ | Manual in Rust |
| Builder pattern | ❌ | ❌ | ✅ | Rust idiomatic |
| Set all agents at once | ✅ | ✅ | ✅ via `agents()` | |
| Add single agent | ❌ | ❌ | ✅ via `add_agent()` | Rust enhancement |
| Model type validation | ✅ | ❌ | ❌ | Python uses Literal |
| E2E integration tests | ✅ | ❌ | 🟡 Recommended | |
| Examples | ✅ | ❌ | 🟡 Recommended | |

---

## Implementation Details

### Method 1: `agents()` - Set Complete HashMap

**Purpose:** Replace the entire agents HashMap at once

**Location:** `src/types.rs` - ClaudeAgentOptionsBuilder impl block (after line 827)

**Signature:**

```rust
pub fn agents(mut self, agents: HashMap<String, AgentDefinition>) -> Self
```

**Implementation:**

```rust
/// Set custom agent definitions
///
/// This method replaces any previously configured agents. To add agents
/// incrementally, use `add_agent()` instead.
///
/// # Arguments
///
/// * `agents` - HashMap of agent name to AgentDefinition
///
/// # Example
///
/// ```
/// use std::collections::HashMap;
/// use claude_agent_sdk::{ClaudeAgentOptions, AgentDefinition};
///
/// let mut agents = HashMap::new();
/// agents.insert("reviewer".to_string(), AgentDefinition {
///     description: "Code reviewer".to_string(),
///     prompt: "Review code thoroughly".to_string(),
///     tools: Some(vec!["Read".to_string()]),
///     model: Some("sonnet".to_string()),
/// });
///
/// let options = ClaudeAgentOptions::builder()
///     .agents(agents)
///     .build();
/// ```
pub fn agents(mut self, agents: HashMap<String, AgentDefinition>) -> Self {
    self.options.agents = Some(agents);
    self
}
```

**Pattern:** Follows existing `hooks()` method (line 824-827)

---

### Method 2: `add_agent()` - Add Single Agent

**Purpose:** Add a single agent definition incrementally

**Location:** `src/types.rs` - ClaudeAgentOptionsBuilder impl block (after `agents()` method)

**Signature:**

```rust
pub fn add_agent(mut self, name: impl Into<String>, agent: AgentDefinition) -> Self
```

**Implementation:**

```rust
/// Add a custom agent definition
///
/// This method adds a single agent to the configuration. Multiple calls
/// can be chained to add multiple agents. If an agent with the same name
/// already exists, it will be replaced.
///
/// # Arguments
///
/// * `name` - The agent name/identifier
/// * `agent` - The agent definition
///
/// # Example
///
/// ```
/// use claude_agent_sdk::{ClaudeAgentOptions, AgentDefinition};
///
/// let options = ClaudeAgentOptions::builder()
///     .add_agent("code-reviewer", AgentDefinition {
///         description: "Reviews code for best practices".to_string(),
///         prompt: "You are a code reviewer...".to_string(),
///         tools: Some(vec!["Read".to_string(), "Grep".to_string()]),
///         model: Some("sonnet".to_string()),
///     })
///     .add_agent("tester", AgentDefinition {
///         description: "Runs tests".to_string(),
///         prompt: "You are a testing expert...".to_string(),
///         tools: Some(vec!["Bash".to_string()]),
///         model: None,
///     })
///     .build();
/// ```
pub fn add_agent(mut self, name: impl Into<String>, agent: AgentDefinition) -> Self {
    self.options
        .agents
        .get_or_insert_with(HashMap::new)
        .insert(name.into(), agent);
    self
}
```

**Pattern:** Follows `add_allowed_tool()` pattern (line 776-779), adapted for HashMap

**Why `get_or_insert_with`?**
- Lazy-initializes HashMap only when needed
- Idiomatic Rust pattern
- More efficient than if-let-some
- Avoids allocating empty HashMap unnecessarily

---

### Integration Points

#### No New Imports Required

All types are already in scope:
- `HashMap` - already used in other builder methods
- `AgentDefinition` - defined in same file (lines 639-652)
- `String` - in prelude

#### File Modification Summary

**File:** `src/types.rs`

**Changes:**
1. Add `agents()` method after line 827
2. Add `add_agent()` method after `agents()`
3. Add unit tests in test module

**Lines affected:** ~830-900 (new code only, no modifications to existing code)

---

## Testing Strategy

### Unit Tests

**Location:** `src/types.rs` - new test module or existing tests

### Test Case 1: Setting Complete Agents HashMap

```rust
#[test]
fn test_builder_agents() {
    let mut agents = HashMap::new();
    agents.insert("reviewer".to_string(), AgentDefinition {
        description: "Code reviewer".to_string(),
        prompt: "Review code".to_string(),
        tools: Some(vec!["Read".to_string()]),
        model: Some("sonnet".to_string()),
    });

    let options = ClaudeAgentOptions::builder()
        .agents(agents.clone())
        .build();

    assert_eq!(options.agents, Some(agents));
}
```

### Test Case 2: Adding Single Agent

```rust
#[test]
fn test_builder_add_agent() {
    let agent = AgentDefinition {
        description: "Code reviewer".to_string(),
        prompt: "Review code".to_string(),
        tools: Some(vec!["Read".to_string()]),
        model: Some("sonnet".to_string()),
    };

    let options = ClaudeAgentOptions::builder()
        .add_agent("reviewer", agent.clone())
        .build();

    assert!(options.agents.is_some());
    assert_eq!(options.agents.unwrap().get("reviewer"), Some(&agent));
}
```

### Test Case 3: Adding Multiple Agents Incrementally

```rust
#[test]
fn test_builder_add_multiple_agents() {
    let reviewer = AgentDefinition {
        description: "Code reviewer".to_string(),
        prompt: "Review code".to_string(),
        tools: Some(vec!["Read".to_string()]),
        model: Some("sonnet".to_string()),
    };

    let tester = AgentDefinition {
        description: "Test runner".to_string(),
        prompt: "Run tests".to_string(),
        tools: Some(vec!["Bash".to_string()]),
        model: None,
    };

    let options = ClaudeAgentOptions::builder()
        .add_agent("reviewer", reviewer.clone())
        .add_agent("tester", tester.clone())
        .build();

    let agents = options.agents.unwrap();
    assert_eq!(agents.len(), 2);
    assert_eq!(agents.get("reviewer"), Some(&reviewer));
    assert_eq!(agents.get("tester"), Some(&tester));
}
```

### Test Case 4: Mixing agents() and add_agent()

```rust
#[test]
fn test_builder_agents_mixed() {
    let mut initial_agents = HashMap::new();
    initial_agents.insert("reviewer".to_string(), AgentDefinition {
        description: "Code reviewer".to_string(),
        prompt: "Review code".to_string(),
        tools: None,
        model: None,
    });

    let tester = AgentDefinition {
        description: "Test runner".to_string(),
        prompt: "Run tests".to_string(),
        tools: None,
        model: None,
    };

    let options = ClaudeAgentOptions::builder()
        .agents(initial_agents)
        .add_agent("tester", tester)
        .build();

    let agents = options.agents.unwrap();
    assert_eq!(agents.len(), 2);
}
```

### Test Case 5: Agent Name Replacement

```rust
#[test]
fn test_builder_agent_replacement() {
    let agent_v1 = AgentDefinition {
        description: "Version 1".to_string(),
        prompt: "Old prompt".to_string(),
        tools: None,
        model: None,
    };

    let agent_v2 = AgentDefinition {
        description: "Version 2".to_string(),
        prompt: "New prompt".to_string(),
        tools: None,
        model: None,
    };

    let options = ClaudeAgentOptions::builder()
        .add_agent("test", agent_v1)
        .add_agent("test", agent_v2.clone())
        .build();

    let agents = options.agents.unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents.get("test"), Some(&agent_v2));
}
```

### Edge Cases

#### Edge Case 1: Empty agent name
- **Status:** Allowed by implementation
- **Rationale:** HashMap accepts empty strings as keys
- **Decision:** No validation needed (matches other builder methods)

#### Edge Case 2: Duplicate agent names with `add_agent()`
- **Status:** Last one wins (standard HashMap behavior)
- **Rationale:** HashMap.insert() replaces existing value
- **Decision:** Document this behavior in rustdoc

#### Edge Case 3: Calling `agents()` after `add_agent()`
- **Status:** Replaces all previously added agents
- **Rationale:** Follows builder pattern convention
- **Decision:** Document this ordering consideration

---

## Example Usage

### Example 1: Incremental Agent Building

```rust
use claude_agent_sdk::{ClaudeAgentOptions, AgentDefinition};

let options = ClaudeAgentOptions::builder()
    .add_agent("code-reviewer", AgentDefinition {
        description: "Expert code review specialist".to_string(),
        prompt: "You are a code review specialist with expertise in security, \
                 performance, and best practices.".to_string(),
        tools: Some(vec!["Read".to_string(), "Grep".to_string(), "Glob".to_string()]),
        model: Some("sonnet".to_string()),
    })
    .add_agent("test-runner", AgentDefinition {
        description: "Runs and analyzes test suites".to_string(),
        prompt: "You are a test execution specialist. Run tests and provide \
                 clear analysis of results.".to_string(),
        tools: Some(vec!["Bash".to_string(), "Read".to_string()]),
        model: None,
    })
    .max_turns(10)
    .build();
```

### Example 2: Batch Agent Configuration

```rust
use std::collections::HashMap;
use claude_agent_sdk::{ClaudeAgentOptions, AgentDefinition};

let mut agents = HashMap::new();

agents.insert("reviewer".to_string(), AgentDefinition {
    description: "Code reviewer".to_string(),
    prompt: "Review code thoroughly for bugs and issues.".to_string(),
    tools: Some(vec!["Read".to_string(), "Grep".to_string()]),
    model: Some("sonnet".to_string()),
});

agents.insert("documenter".to_string(), AgentDefinition {
    description: "Documentation writer".to_string(),
    prompt: "Write comprehensive technical documentation.".to_string(),
    tools: Some(vec!["Read".to_string(), "Write".to_string()]),
    model: Some("sonnet".to_string()),
});

let options = ClaudeAgentOptions::builder()
    .agents(agents)
    .permission_mode(PermissionMode::AcceptEdits)
    .build();
```

### Example 3: Python SDK Equivalent

**Python SDK** (`examples/agents.py:86-101`):

```python
options = ClaudeAgentOptions(
    agents={
        "analyzer": AgentDefinition(
            description="Analyzes code structure and patterns",
            prompt="You are a code analyzer...",
            tools=["Read", "Grep", "Glob"],
        ),
        "tester": AgentDefinition(
            description="Creates and runs tests",
            prompt="You are a testing expert...",
            tools=["Read", "Write", "Bash"],
            model="sonnet",
        ),
    },
)
```

**Rust SDK Equivalent**:

```rust
let options = ClaudeAgentOptions::builder()
    .add_agent("analyzer", AgentDefinition {
        description: "Analyzes code structure and patterns".to_string(),
        prompt: "You are a code analyzer...".to_string(),
        tools: Some(vec!["Read".to_string(), "Grep".to_string(), "Glob".to_string()]),
        model: None,
    })
    .add_agent("tester", AgentDefinition {
        description: "Creates and runs tests".to_string(),
        prompt: "You are a testing expert...".to_string(),
        tools: Some(vec!["Read".to_string(), "Write".to_string(), "Bash".to_string()]),
        model: Some("sonnet".to_string()),
    })
    .build();
```

---

## Recommended Enhancements

### Enhancement 1: Create `examples/agents.rs` (HIGH PRIORITY)

**Rationale:** Python SDK has `examples/agents.py` - we should match

**Location:** `examples/agents.rs`

**Content:**

```rust
//! Example of using custom agents with Claude Agent SDK
//!
//! This example demonstrates how to define and use custom agents with specific
//! tools, prompts, and models.
//!
//! Usage:
//! cargo run --example agents

use claude_agent_sdk::{query, ClaudeAgentOptions, AgentDefinition, Message, ContentBlock};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    code_reviewer_example().await?;
    multiple_agents_example().await?;
    Ok(())
}

async fn code_reviewer_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Code Reviewer Agent Example ===\n");

    let options = ClaudeAgentOptions::builder()
        .add_agent("code-reviewer", AgentDefinition {
            description: "Reviews code for best practices and potential issues".to_string(),
            prompt: "You are a code reviewer. Analyze code for bugs, performance issues, \
                     security vulnerabilities, and adherence to best practices. \
                     Provide constructive feedback.".to_string(),
            tools: Some(vec!["Read".to_string(), "Grep".to_string()]),
            model: Some("sonnet".to_string()),
        })
        .build();

    let stream = query(
        "Use the code-reviewer agent to review the code in src/types.rs",
        Some(options)
    ).await?;

    let mut stream = Box::pin(stream);

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    if let ContentBlock::Text { text } = block {
                        println!("Claude: {}", text);
                    }
                }
            }
            Message::Result { total_cost_usd, .. } => {
                if let Some(cost) = total_cost_usd {
                    println!("\nCost: ${:.4}", cost);
                }
            }
            _ => {}
        }
    }

    println!();
    Ok(())
}

async fn multiple_agents_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Multiple Agents Example ===\n");

    let options = ClaudeAgentOptions::builder()
        .add_agent("analyzer", AgentDefinition {
            description: "Analyzes code structure and patterns".to_string(),
            prompt: "You are a code analyzer. Examine code structure, patterns, and architecture.".to_string(),
            tools: Some(vec!["Read".to_string(), "Grep".to_string(), "Glob".to_string()]),
            model: None,
        })
        .add_agent("tester", AgentDefinition {
            description: "Creates and runs tests".to_string(),
            prompt: "You are a testing expert. Write comprehensive tests and ensure code quality.".to_string(),
            tools: Some(vec!["Read".to_string(), "Write".to_string(), "Bash".to_string()]),
            model: Some("sonnet".to_string()),
        })
        .build();

    let stream = query(
        "Use the analyzer agent to find all Rust files in the examples/ directory",
        Some(options)
    ).await?;

    let mut stream = Box::pin(stream);

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    if let ContentBlock::Text { text } = block {
                        println!("Claude: {}", text);
                    }
                }
            }
            Message::Result { total_cost_usd, .. } => {
                if let Some(cost) = total_cost_usd {
                    println!("\nCost: ${:.4}", cost);
                }
            }
            _ => {}
        }
    }

    println!();
    Ok(())
}
```

### Enhancement 2: Add AgentModel Enum (MEDIUM PRIORITY)

**Rationale:** Python SDK uses `Literal["sonnet", "opus", "haiku", "inherit"]` for type safety

**Location:** `src/types.rs` (before AgentDefinition)

**Implementation:**

```rust
/// AI model to use for the agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentModel {
    /// Claude Sonnet model
    Sonnet,
    /// Claude Opus model
    Opus,
    /// Claude Haiku model
    Haiku,
    /// Inherit model from parent options
    Inherit,
}

// Then update AgentDefinition:
pub struct AgentDefinition {
    pub description: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<AgentModel>,  // Changed from String
}
```

**Pros:**
- ✅ Compile-time validation of model names
- ✅ IDE autocomplete
- ✅ Clear documentation of valid models
- ✅ Matches Python SDK semantics

**Cons:**
- ⚠️ Breaking change to existing code
- ⚠️ Less flexible for future model additions

**Decision:** Consider for next major version or make it opt-in via feature flag

### Enhancement 3: Add AgentDefinitionBuilder (LOW PRIORITY)

**Rationale:** Reduce verbosity of AgentDefinition construction

**Location:** `src/types.rs`

**Implementation:**

```rust
impl AgentDefinition {
    pub fn builder() -> AgentDefinitionBuilder {
        AgentDefinitionBuilder::default()
    }
}

#[derive(Debug, Default)]
pub struct AgentDefinitionBuilder {
    description: Option<String>,
    prompt: Option<String>,
    tools: Option<Vec<String>>,
    model: Option<String>,
}

impl AgentDefinitionBuilder {
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    pub fn tools(mut self, tools: Vec<impl Into<String>>) -> Self {
        self.tools = Some(tools.into_iter().map(|t| t.into()).collect());
        self
    }

    pub fn add_tool(mut self, tool: impl Into<String>) -> Self {
        self.tools.get_or_insert_with(Vec::new).push(tool.into());
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn build(self) -> AgentDefinition {
        AgentDefinition {
            description: self.description.expect("description is required"),
            prompt: self.prompt.expect("prompt is required"),
            tools: self.tools,
            model: self.model,
        }
    }
}
```

**Usage:**

```rust
let agent = AgentDefinition::builder()
    .description("Code reviewer")
    .prompt("You are a code reviewer...")
    .add_tool("Read")
    .add_tool("Grep")
    .model("sonnet")
    .build();
```

**Decision:** Nice-to-have but not essential for initial implementation

### Enhancement 4: Integration Test (MEDIUM PRIORITY)

**Rationale:** Python SDK has E2E test in `e2e-tests/test_agents_and_settings.py`

**Location:** `tests/integration_agents.rs` or add to existing integration tests

**Implementation:**

```rust
#[tokio::test]
async fn test_agents_serialization_to_cli() {
    let options = ClaudeAgentOptions::builder()
        .add_agent("test-agent", AgentDefinition {
            description: "Test agent".to_string(),
            prompt: "You are a test".to_string(),
            tools: Some(vec!["Read".to_string()]),
            model: Some("sonnet".to_string()),
        })
        .build();

    // Verify that agents are properly serialized when passed to CLI
    // This would require mocking or integration with actual subprocess
    assert!(options.agents.is_some());
    let agents = options.agents.unwrap();
    assert_eq!(agents.len(), 1);
    assert!(agents.contains_key("test-agent"));
}
```

---

## Success Criteria

### Implementation Complete When:

1. ✅ Both `agents()` and `add_agent()` methods added to builder
2. ✅ All 5 unit tests pass
3. ✅ Documentation includes examples with doc tests
4. ✅ `cargo clippy` shows no warnings
5. ✅ `cargo test` passes all tests
6. ✅ `cargo test --doc` passes (doc tests work)
7. ✅ `cargo doc --open` renders documentation correctly
8. ✅ Manually verified with example code

### Quality Metrics:

- ✅ Follows existing code style and conventions
- ✅ Consistent with other builder methods
- ✅ Clear, helpful documentation
- ✅ Comprehensive test coverage
- ✅ Feature parity with Python SDK

### Optional (Recommended):

- 🟡 `examples/agents.rs` created and tested
- 🟡 AgentModel enum implemented
- 🟡 Integration test added

---

## Implementation Order

### Phase 1: Core Implementation (MUST)

1. **Add `agents()` method** to `ClaudeAgentOptionsBuilder`
   - Location: `src/types.rs:~827`
   - Pattern: Follow `hooks()` method
   - Time: 10 minutes

2. **Add `add_agent()` method** to `ClaudeAgentOptionsBuilder`
   - Location: After `agents()` method
   - Pattern: Follow `add_allowed_tool()` but for HashMap
   - Time: 15 minutes

3. **Add unit tests**
   - All 5 test cases from testing strategy
   - Location: `src/types.rs` test module
   - Time: 30 minutes

4. **Add rustdoc examples**
   - Inline doc tests for both methods
   - Time: 15 minutes

5. **Run test suite**
   - `cargo test`
   - `cargo test --doc`
   - `cargo clippy`
   - Time: 5 minutes

### Phase 2: Documentation & Examples (SHOULD)

6. **Create `examples/agents.rs`**
   - Two comprehensive examples
   - Time: 30 minutes

7. **Update README.md**
   - Add agents section to features
   - Time: 10 minutes

### Phase 3: Enhancements (COULD)

8. **Optional: Add AgentModel enum**
   - If type safety is desired
   - Time: 20 minutes

9. **Optional: Add integration test**
   - Verify CLI integration
   - Time: 30 minutes

**Total Estimated Time:**
- Phase 1 (core): ~75 minutes
- Phase 2 (docs): ~40 minutes
- Phase 3 (enhancements): ~50 minutes
- **Total: 2-3 hours**

---

## Code Location Summary

| Item | File | Line Range | Status |
|------|------|------------|--------|
| ClaudeAgentOptions struct | `src/types.rs` | 659-707 | ✅ Exists |
| agents field definition | `src/types.rs` | 704 | ✅ Exists |
| AgentDefinition struct | `src/types.rs` | 639-652 | ✅ Exists |
| ClaudeAgentOptionsBuilder | `src/types.rs` | 762-833 | ✅ Exists |
| Insert `agents()` method | `src/types.rs` | ~828 | ⏳ To Add |
| Insert `add_agent()` method | `src/types.rs` | ~835 | ⏳ To Add |
| Unit tests | `src/types.rs` | TBD | ⏳ To Add |
| Example file | `examples/agents.rs` | New file | 🟡 Recommended |

---

## Breaking Changes & Compatibility

### Breaking Changes: None

- ✅ Adding methods to builder is non-breaking
- ✅ Existing code continues to work
- ✅ `agents` field already exists in `ClaudeAgentOptions`

### Backward Compatibility: Full

- ✅ Existing code that sets `options.agents` directly still works
- ✅ New builder methods are purely additive
- ✅ No changes to existing public API

### Future Compatibility

- ⚠️ If we add `AgentModel` enum later, it would be a breaking change
- ✅ Solution: Feature flag or wait for next major version

---

## References

### Python SDK Files

- AgentDefinition: `/home/molaco/Documents/claude-agent-sdk-python/src/claude_agent_sdk/types.py:30-37`
- Options.agents field: `/home/molaco/Documents/claude-agent-sdk-python/src/claude_agent_sdk/types.py:541`
- Example usage: `/home/molaco/Documents/claude-agent-sdk-python/examples/agents.py`
- E2E tests: `/home/molaco/Documents/claude-agent-sdk-python/e2e-tests/test_agents_and_settings.py`

### Rust SDK Files

- AgentDefinition: `src/types.rs:639-652`
- Options.agents field: `src/types.rs:704`
- Builder implementation: `src/types.rs:762-833`

### External References

- [Rust Builder Pattern](https://rust-unofficial.github.io/patterns/patterns/creational/builder.html)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Claude Code SDK Documentation](https://docs.anthropic.com/en/docs/claude-code/sdk)

---

## Appendix: Builder Pattern Conventions

### Existing HashMap Field Patterns in Codebase

#### Pattern A: Complete Replacement (for collections)

**Example: `mcp_servers` (lines 788-791)**

```rust
pub fn mcp_servers(mut self, servers: HashMap<String, McpServerConfig>) -> Self {
    self.options.mcp_servers = McpServers::Dict(servers);
    self
}
```

**Example: `hooks` (lines 824-827)**

```rust
pub fn hooks(mut self, hooks: HashMap<HookEvent, Vec<HookMatcher>>) -> Self {
    self.options.hooks = Some(hooks);
    self
}
```

#### Pattern B: Incremental Addition (for vectors)

**Example: `allowed_tools` (lines 770-779)**

```rust
pub fn allowed_tools(mut self, tools: Vec<impl Into<ToolName>>) -> Self {
    self.options.allowed_tools = tools.into_iter().map(|t| t.into()).collect();
    self
}

pub fn add_allowed_tool(mut self, tool: impl Into<ToolName>) -> Self {
    self.options.allowed_tools.push(tool.into());
    self
}
```

### Recommended Pattern for agents

**Combine both patterns:**
- `agents()` for complete replacement (Pattern A)
- `add_agent()` for incremental addition (Pattern B adapted for HashMap)

---

## Status & Next Steps

**Status:** ✅ Planning Complete - Ready for Implementation

**Next Steps:**
1. Review and approve this implementation plan
2. Begin Phase 1 implementation
3. Run test suite
4. Create PR with Phase 1 changes
5. Consider Phase 2 and 3 enhancements

**Questions/Blockers:** None identified

---

*Document Version: 1.0*
*Last Updated: 2025-10-21*
