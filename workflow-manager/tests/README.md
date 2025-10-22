# Research Module Tests

This directory contains integration and unit tests for the research workflow library.

## Test Categories

### test_helpers.rs
Unit tests for helper functions from the research module:
- `find_yaml_files()` - Finding YAML files in directories
- Tests for empty directories, non-existent directories, file sorting
- Edge cases and error handling

### test_types.rs
Tests for data structures and serialization:
- `ResearchPrompt` - Creation, cloning, serialization
- `PromptsData` - Multiple prompts, objective handling
- `ResearchResult` - Result tracking and serialization
- `CodebaseAnalysis` - YAML value type alias
- YAML serialization/deserialization round-trips

### test_workflow_config.rs
Tests for `WorkflowConfig`:
- Default configuration values
- Custom configurations with all parameters
- Phase selection and batch size settings
- Resume configurations for different phases
- Clone functionality
- Edge cases (empty phases, large batch sizes)

### test_integration.rs
Integration tests for the complete workflow:
- Module imports and public API verification
- Configuration validation
- Full workflow execution (API-dependent, marked `#[ignore]`)
- Resume from saved state scenarios
- Error handling for missing parameters

## Running Tests

### Run All Non-API Tests (Fast)
```bash
# Run all tests excluding those that require Claude API
cargo test

# Run with output visible
cargo test -- --nocapture

# Run specific test file
cargo test test_types
cargo test test_helpers
cargo test test_workflow_config
cargo test test_integration
```

### Run Specific Tests
```bash
# Run a single test by name
cargo test test_find_yaml_files_basic

# Run all tests matching a pattern
cargo test yaml

# Run tests in a specific module
cargo test research::test_types
```

### Run API-Dependent Tests (Slow, Requires API Key)
Some tests require Claude API access and are marked with `#[ignore]`. These tests:
- Require `ANTHROPIC_API_KEY` environment variable
- May incur API costs
- Can take significant time to complete
- Test actual workflow execution with Claude agents

Run them explicitly when needed:
```bash
# Run only ignored tests
cargo test -- --ignored

# Run ALL tests including ignored ones
cargo test -- --include-ignored

# Run specific ignored test
cargo test test_full_workflow_phase0_only -- --ignored

# Run with single thread (for API tests that might conflict)
cargo test -- --ignored --test-threads=1
```

## Test Organization

```
tests/
├── README.md (this file)
└── research/
    ├── mod.rs                      # Test module root
    ├── common.rs                   # Shared test utilities
    ├── test_helpers.rs             # Helper function unit tests
    ├── test_types.rs               # Data type tests
    ├── test_workflow_config.rs     # Configuration tests
    └── test_integration.rs         # Integration tests
```

## Writing New Tests

### Unit Tests
Unit tests should be:
- Fast (< 1 second each)
- Isolated (no external dependencies)
- Deterministic (same result every time)
- Use temporary directories that are cleaned up

Example:
```rust
#[test]
fn test_something() {
    let config = WorkflowConfig::default();
    assert_eq!(config.batch_size, 1);
}
```

### Async Tests
For async functionality:
```rust
#[tokio::test]
async fn test_async_function() {
    let result = some_async_function().await;
    assert!(result.is_ok());
}
```

### API-Dependent Tests
Mark tests that require Claude API with `#[ignore]`:
```rust
#[tokio::test]
#[ignore] // Requires Claude API
async fn test_with_api() {
    // Test that calls Claude agents
}
```

### Temporary Files
Always clean up temporary files:
```rust
use common::{create_temp_dir, cleanup_temp_dir};

#[tokio::test]
async fn test_with_temp_files() {
    let temp_dir = create_temp_dir("test_name");

    // Your test code here

    cleanup_temp_dir(&temp_dir);
}
```

## Test Coverage

Current test coverage includes:

**Data Types** (100%):
- ResearchPrompt creation, cloning, serialization
- PromptsData with single/multiple prompts
- ResearchResult tracking
- CodebaseAnalysis YAML handling

**WorkflowConfig** (100%):
- Default configuration
- All custom parameter combinations
- Resume scenarios for each phase
- Edge cases (empty phases, large batch sizes)
- Clone functionality

**Helper Functions** (~80%):
- find_yaml_files with various scenarios
- extract_yaml (indirectly tested)
- File validation logic

**Integration** (~60%):
- Module structure verification
- Configuration validation
- API workflow tests (basic coverage)

**Not Covered Yet**:
- Individual phase execution (requires API)
- Error handling in phase execution
- Concurrent agent execution
- YAML fixing iteration logic

## CI/CD Integration

Tests can be integrated into CI/CD pipelines:

```yaml
# .github/workflows/test.yml example
- name: Run tests
  run: cargo test

- name: Run API tests (optional)
  if: github.event_name == 'push' && github.ref == 'refs/heads/main'
  env:
    ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
  run: cargo test -- --ignored
```

## Performance

Test suite performance (without API tests):
- **Total tests**: ~35 tests
- **Execution time**: ~2-5 seconds
- **With API tests**: 10-30 minutes (depending on complexity)

## Best Practices

1. **No API calls by default** - Mark API tests with `#[ignore]`
2. **Fast tests** - Unit tests should complete quickly
3. **Clean up** - Always remove temporary files/directories
4. **Comprehensive** - Test public APIs, edge cases, and error conditions
5. **Documentation** - Tests serve as usage examples

## Debugging Tests

Run tests with debug output:
```bash
# Show println! output
cargo test -- --nocapture

# Show test names as they run
cargo test -- --nocapture --test-threads=1

# Run with debug logging
RUST_LOG=debug cargo test
```

## Troubleshooting

**Tests fail with "directory not found"**:
- Ensure you're running from the project root
- Check that temporary directory creation has permissions

**API tests timeout**:
- Increase timeout in test code
- Check API key is valid
- Verify network connectivity

**YAML validation tests fail**:
- Ensure `check_yaml.py` script is available at expected path
- Verify Python/uv is installed

## Future Improvements

Potential additions:
- Property-based testing with proptest
- Benchmark tests for performance tracking
- Mock Claude API for testing without API calls
- More comprehensive error scenario testing
- Coverage reports with tarpaulin
