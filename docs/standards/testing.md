# Testing Strategy

## Test Categories

1. **Unit Tests**: Test individual functions and types in isolation
2. **Integration Tests**: Test interactions between components
3. **Contract Tests**: Verify trait implementations meet contracts
4. **Property Tests**: Use `proptest` for property-based testing

## Test Coverage Expectations

- **Core Business Logic**: 100% coverage
- **Error Paths**: All error variants tested
- **Edge Cases**: Boundary conditions covered
- **Security-Critical Code**: Exhaustive testing (e.g., token handling, validation)

## Test File Organization

Tests should be organized in separate test files adjacent to the code they test, following this pattern:

**For a source file**: `src/module/file.rs`
**Create test file**: `src/module/file_tests.rs`

**For a module file**: `src/module/mod.rs`
**Create test file**: `src/module/mod_tests.rs`

## Test Module Declaration

Reference the external test file from the source file using:

```rust
#[cfg(test)]
#[path = "<TEST_FILE_NAME_WITH_EXTENSION>"]
mod tests;
```

## Test Organization Patterns

```rust
//! Tests for authentication module.

use super::*;

// Group related tests using module organization
mod token_tests {
    use super::*;

    #[test]
    fn test_token_creation() { }

    #[test]
    fn test_token_expiry() { }
}

mod validation_tests {
    use super::*;

    #[test]
    fn test_valid_input() { }

    #[test]
    fn test_invalid_input() { }
}
```

## Integration Testing

- Place integration tests in `tests/` directory at crate root
- Use test fixtures and mock patterns for external dependencies
- Test async code with `#[tokio::test]` or appropriate runtime
- Verify error paths and edge cases explicitly
- Use `assert_eq!`, `assert!`, and `matches!` appropriately

#### Integration Test Requirements

**DO NOT** stub or mock the following when writing integration tests:

- Real GitHub repositories in `glitchgrove` organization
- Metadata repository configuration files
- Template repository content

**ALWAYS** use:

- Real GitHub API calls to `glitchgrove` organization
- Actual `.reporoller-test` metadata repository
- Real template repositories for content testing
- GitHub CLI (`gh`) for setup and verification when appropriate

**Test Cleanup**:

- Delete created test repositories after test completion
- Use unique repository names (timestamp-based) to avoid conflicts
- Handle cleanup in test teardown even on failure

### Testing Infrastructure

**CRITICAL**: Real GitHub testing infrastructure exists and must be used for integration tests.

#### Test Organization (glitchgrove)

All integration testing infrastructure is located in the `glitchgrove` GitHub organization:

- **Environment Variable**: `TEST_ORG=glitchgrove`
- **Metadata Repositories**: See [metadata](../../tests/metadata/) for test configuration repositories.
- **Template Repositories**: 16+ template repositories with various test scenarios. See [templates](../../tests/templates/)
  for the full list.
