# Testing Best Practices for Digital Twin Jamey

> **Navigation**: [Documentation Home](../README.md) > [Testing](README.md) > Best Practices

## Overview

This document outlines testing best practices, patterns, and guidelines for the Digital Twin Jamey project. Following these practices ensures high code quality, maintainability, and reliability.

## Table of Contents

1. [Test Organization](#test-organization)
2. [Test Types](#test-types)
3. [Writing Effective Tests](#writing-effective-tests)
4. [Test Fixtures and Helpers](#test-fixtures-and-helpers)
5. [Property-Based Testing](#property-based-testing)
6. [Integration Testing](#integration-testing)
7. [Mocking and Stubbing](#mocking-and-stubbing)
8. [Performance Testing](#performance-testing)
9. [CI/CD Integration](#cicd-integration)
10. [Common Patterns](#common-patterns)

## Test Organization

### Directory Structure

```
jamey-core/
├── src/
│   ├── memory.rs
│   └── secrets.rs
└── tests/
    ├── memory_unit_tests.rs      # Unit tests for memory module
    ├── secrets_unit_tests.rs     # Unit tests for secrets module
    ├── property_tests.rs         # Property-based tests
    ├── helpers/
    │   └── mod.rs                # Test helper functions
    ├── fixtures/
    │   └── mod.rs                # Test data fixtures
    └── mocks/
        └── mod.rs                # Mock implementations

tests/                            # Workspace-level integration tests
├── full_workflow_integration_tests.rs
├── database_tests.rs
└── external_service_tests.rs
```

### Naming Conventions

- **Unit test files**: `{module}_unit_tests.rs`
- **Integration test files**: `{feature}_integration_tests.rs`
- **Property test files**: `property_tests.rs` or `{module}_prop_tests.rs`
- **Test functions**: `test_{what_is_being_tested}`
- **Helper functions**: `create_test_{resource}`, `setup_{context}`

## Test Types

### 1. Unit Tests

Test individual functions and methods in isolation.

```rust
#[test]
fn test_memory_type_try_from_valid() {
    let result = MemoryType::try_from("conversation").unwrap();
    assert_eq!(result, MemoryType::Conversation);
}

#[test]
fn test_memory_type_try_from_invalid() {
    let result = MemoryType::try_from("invalid");
    assert!(result.is_err());
}
```

**Best Practices:**
- Test one thing per test
- Use descriptive test names
- Test both success and failure cases
- Test boundary conditions
- Keep tests fast and independent

### 2. Integration Tests

Test interactions between multiple components.

```rust
#[tokio::test]
async fn test_complete_chat_workflow() {
    let provider = create_mock_provider().await;
    let store = create_test_memory_store().await;
    
    // Test full workflow from user input to stored response
    let user_message = "Hello";
    let embedding = provider.get_embedding(user_message).await.unwrap();
    let memory_id = store.store(create_memory(user_message, embedding)).await.unwrap();
    
    let retrieved = store.retrieve(memory_id).await.unwrap();
    assert_eq!(retrieved.content, user_message);
}
```

**Best Practices:**
- Test realistic scenarios
- Use test databases/services
- Clean up resources after tests
- Test error recovery paths
- Verify end-to-end behavior

### 3. Property-Based Tests

Test invariants and properties that should always hold.

```rust
proptest! {
    #[test]
    fn test_serialization_roundtrip(
        memory_type in memory_type_strategy()
    ) {
        let serialized = serde_json::to_string(&memory_type).unwrap();
        let deserialized: MemoryType = serde_json::from_str(&serialized).unwrap();
        prop_assert_eq!(memory_type, deserialized);
    }
}
```

**Best Practices:**
- Define clear properties/invariants
- Use appropriate strategies for input generation
- Test mathematical properties (commutativity, associativity, etc.)
- Test serialization round-trips
- Verify idempotence where applicable

## Writing Effective Tests

### AAA Pattern (Arrange-Act-Assert)

```rust
#[test]
fn test_memory_store() {
    // Arrange
    let memory = create_test_memory();
    let store = create_test_store();
    
    // Act
    let id = store.store(memory).await.unwrap();
    
    // Assert
    let retrieved = store.retrieve(id).await.unwrap();
    assert_eq!(retrieved.content, "test content");
}
```

### Test Error Cases

```rust
#[test]
fn test_invalid_input_returns_error() {
    let result = validate_input("");
    
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ValidationError::EmptyInput));
}
```

### Test Boundary Conditions

```rust
#[test]
fn test_content_at_max_length() {
    let max_content = "x".repeat(32768);
    let result = validate_content(&max_content);
    assert!(result.is_ok());
}

#[test]
fn test_content_exceeds_max_length() {
    let too_long = "x".repeat(32769);
    let result = validate_content(&too_long);
    assert!(result.is_err());
}
```

### Test Concurrency

```rust
#[tokio::test]
async fn test_concurrent_operations() {
    let store = Arc::new(create_test_store().await);
    let mut handles = vec![];
    
    for i in 0..10 {
        let store_clone = store.clone();
        handles.push(tokio::spawn(async move {
            store_clone.store(create_memory(i)).await
        }));
    }
    
    let results = futures::future::join_all(handles).await;
    assert!(results.iter().all(|r| r.is_ok()));
}
```

## Test Fixtures and Helpers

### Creating Test Fixtures

```rust
// tests/fixtures/mod.rs
pub fn create_test_memory() -> Memory {
    Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "Test content".to_string(),
        embedding: vec![0.1; 1536],
        metadata: json!({"test": true}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    }
}

pub fn create_test_memory_with_content(content: &str) -> Memory {
    Memory {
        content: content.to_string(),
        ..create_test_memory()
    }
}
```

### Creating Test Helpers

```rust
// tests/helpers/mod.rs
pub struct TestContext {
    pub pool: Pool,
    pub store: PostgresMemoryStore,
}

impl TestContext {
    pub async fn new() -> Result<Self> {
        let pool = create_test_pool().await;
        let store = PostgresMemoryStore::new(pool.clone(), 1536).await?;
        Ok(Self { pool, store })
    }
    
    pub async fn cleanup(&self) -> Result<()> {
        // Clean up test data
        Ok(())
    }
}
```

## Property-Based Testing

### Defining Strategies

```rust
fn memory_type_strategy() -> impl Strategy<Value = MemoryType> {
    prop_oneof![
        Just(MemoryType::Conversation),
        Just(MemoryType::Knowledge),
        Just(MemoryType::Experience),
    ]
}

fn content_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 ]{1,1000}"
}
```

### Testing Invariants

```rust
proptest! {
    #[test]
    fn test_embedding_no_nan_or_inf(
        embedding in prop::collection::vec(-1.0f32..1.0f32, 128)
    ) {
        prop_assert!(embedding.iter().all(|x| !x.is_nan() && !x.is_infinite()));
    }
}
```

## Integration Testing

### Database Testing

```rust
async fn create_test_pool() -> Pool {
    let mut cfg = Config::new();
    cfg.host = Some("localhost".to_string());
    cfg.dbname = Some("jamey_test".to_string());
    cfg.user = Some("postgres".to_string());
    cfg.password = Some("test_password".to_string());
    cfg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap()
}

#[tokio::test]
async fn test_database_operations() {
    let pool = create_test_pool().await;
    let store = PostgresMemoryStore::new(pool, 1536).await.unwrap();
    
    // Test operations
    // ...
    
    // Cleanup is automatic when pool is dropped
}
```

### API Testing with Mocks

```rust
#[tokio::test]
async fn test_api_integration() {
    let mock_server = MockServer::start().await;
    
    Mock::given(method("POST"))
        .and(path("/api/endpoint"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success"
        })))
        .mount(&mock_server)
        .await;
    
    let client = create_client(&mock_server.uri());
    let result = client.call_api().await;
    
    assert!(result.is_ok());
}
```

## Mocking and Stubbing

### Creating Mock Implementations

```rust
pub struct MockMemoryStore {
    memories: Arc<Mutex<HashMap<Uuid, Memory>>>,
}

#[async_trait]
impl MemoryStore for MockMemoryStore {
    async fn store(&self, memory: Memory) -> Result<Uuid> {
        let id = memory.id;
        self.memories.lock().unwrap().insert(id, memory);
        Ok(id)
    }
    
    async fn retrieve(&self, id: Uuid) -> Result<Memory> {
        self.memories.lock().unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("Not found"))
    }
}
```

## Performance Testing

### Benchmarking

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_memory_store(c: &mut Criterion) {
    c.bench_function("memory_store", |b| {
        b.iter(|| {
            let memory = create_test_memory();
            black_box(memory)
        });
    });
}

criterion_group!(benches, benchmark_memory_store);
criterion_main!(benches);
```

### Load Testing

```rust
#[tokio::test]
async fn test_high_load() {
    let store = Arc::new(create_test_store().await);
    let mut handles = vec![];
    
    for i in 0..1000 {
        let store_clone = store.clone();
        handles.push(tokio::spawn(async move {
            store_clone.store(create_memory(i)).await
        }));
    }
    
    let start = Instant::now();
    let results = futures::future::join_all(handles).await;
    let duration = start.elapsed();
    
    assert!(results.iter().all(|r| r.is_ok()));
    assert!(duration.as_secs() < 10, "Load test took too long");
}
```

## CI/CD Integration

### Running Tests Locally

```bash
# Run all tests
cargo test --workspace

# Run specific test suite
cargo test --package jamey-core

# Run with coverage
cargo tarpaulin --workspace --out Html

# Run property tests with more cases
PROPTEST_CASES=10000 cargo test property_tests
```

### CI Pipeline Structure

1. **Fast Unit Tests** - Run first for quick feedback
2. **Integration Tests** - Require database/services
3. **Property Tests** - Extensive input generation
4. **Code Quality** - Clippy, formatting, documentation
5. **Coverage** - Enforce minimum threshold (80%)
6. **Security Audit** - Check for vulnerabilities
7. **Benchmarks** - Track performance (on main branch)

## Common Patterns

### Testing Async Code

```rust
#[tokio::test]
async fn test_async_operation() {
    let result = async_function().await;
    assert!(result.is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_concurrent_async() {
    // Test with multiple threads
}
```

### Testing with Timeouts

```rust
#[tokio::test]
async fn test_with_timeout() {
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        slow_operation()
    ).await;
    
    assert!(result.is_ok(), "Operation timed out");
}
```

### Parameterized Tests

```rust
#[test]
fn test_multiple_inputs() {
    let test_cases = vec![
        ("input1", "expected1"),
        ("input2", "expected2"),
        ("input3", "expected3"),
    ];
    
    for (input, expected) in test_cases {
        let result = process(input);
        assert_eq!(result, expected, "Failed for input: {}", input);
    }
}
```

### Testing Panics

```rust
#[test]
#[should_panic(expected = "Invalid input")]
fn test_panic_on_invalid_input() {
    process_with_panic("");
}
```

## Code Coverage Goals

- **Minimum Coverage**: 80% overall
- **Critical Modules**: 90%+ coverage
  - `memory.rs`
  - `secrets.rs`
  - `openrouter.rs`
- **New Code**: 85%+ coverage required

## Test Maintenance

### Regular Tasks

1. **Review and update tests** when requirements change
2. **Remove obsolete tests** that no longer provide value
3. **Refactor test code** to reduce duplication
4. **Update fixtures** to reflect current data models
5. **Monitor test execution time** and optimize slow tests

### Test Smells to Avoid

- ❌ Tests that depend on execution order
- ❌ Tests with hard-coded sleep/delays
- ❌ Tests that modify global state
- ❌ Tests with unclear assertions
- ❌ Tests that test implementation details
- ❌ Flaky tests that pass/fail randomly

## Related Documentation

- [Testing Strategy](strategy.md) - Overall testing approach and CI/CD
- [Testing Overview](README.md) - Testing documentation hub
- [Performance Monitoring](../operations/performance-monitoring.md) - Performance testing
- [Architecture Overview](../architecture/system-overview.md) - System architecture

## Resources

- [Rust Testing Documentation](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Proptest Book](https://altsysrq.github.io/proptest-book/)
- [Criterion.rs Guide](https://bheisler.github.io/criterion.rs/book/)
- [Tokio Testing Guide](https://tokio.rs/tokio/topics/testing)

## Conclusion

Following these testing best practices ensures:

- ✅ High code quality and reliability
- ✅ Easier refactoring and maintenance
- ✅ Better documentation through tests
- ✅ Faster development cycles
- ✅ Increased confidence in deployments

Remember: **Good tests are an investment in the future of the codebase.**

---

**Last Updated**: 2025-11-17
**Status**: ✅ Complete
**Category**: Testing