
# Performance Monitoring Guide

> **Navigation**: [Documentation Home](../README.md) > [Operations](README.md) > Performance Monitoring

This document provides guidelines for monitoring, profiling, and optimizing performance in the Jamey codebase.

## Table of Contents

1. [Performance Baselines](#performance-baselines)
2. [Profiling Tools](#profiling-tools)
3. [Running Benchmarks](#running-benchmarks)
4. [Performance Testing Strategy](#performance-testing-strategy)
5. [Optimization Guidelines](#optimization-guidelines)
6. [Monitoring in Production](#monitoring-in-production)

## Performance Baselines

### Memory Operations

Based on initial benchmarking with 1536-dimensional vectors:

| Operation | Baseline (avg) | Target | Notes |
|-----------|---------------|--------|-------|
| Memory Store | ~50ms | <30ms | Includes vector serialization |
| Memory Retrieve | ~20ms | <10ms | Single record lookup |
| Memory Search (10 results) | ~100ms | <50ms | Vector similarity search |
| Memory Update | ~40ms | <25ms | Content and embedding update |
| Memory Delete | ~15ms | <10ms | Single record deletion |
| List Paginated (100 records) | ~150ms | <75ms | With pagination support |

### Arc Operations

| Operation | Baseline | Notes |
|-----------|----------|-------|
| Arc::clone() | ~5ns | Atomic reference count increment |
| Vec::clone() (1KB) | ~500ns | Full data copy |
| Arc overhead | Minimal | Use for shared read-only data |

### Cache Performance

| Operation | Cache Hit | Cache Miss | Notes |
|-----------|-----------|------------|-------|
| Retrieve (Redis) | ~2ms | ~22ms | Network + DB latency |
| Retrieve (Memory) | ~100μs | ~20ms | In-process cache |
| Search (Cached) | ~5ms | ~105ms | Query result caching |

## Profiling Tools

### 1. Criterion Benchmarks

Run comprehensive benchmarks:

```bash
# Run all benchmarks
cargo bench --package jamey-core

# Run specific benchmark group
cargo bench --package jamey-core -- memory_operations

# Run with baseline comparison
cargo bench --package jamey-core --bench criterion_benchmarks -- --save-baseline main
cargo bench --package jamey-core --bench criterion_benchmarks -- --baseline main
```

Benchmark results are saved to `target/criterion/` with HTML reports.

### 2. Tracing Instrumentation

The codebase uses `tracing` for performance instrumentation:

```rust
use jamey_core::profiling::TimingGuard;

async fn my_operation() {
    let _timer = TimingGuard::new("my_operation");
    // Your code here
    // Timer automatically logs duration on drop
}
```

Enable debug logging to see timing information:

```bash
RUST_LOG=debug cargo run
```

### 3. Custom Performance Metrics

Use the `PerformanceMetrics` collector for tracking operation statistics:

```rust
use jamey_core::profiling::PerformanceMetrics;

let mut metrics = PerformanceMetrics::new();

for _ in 0..1000 {
    let start = std::time::Instant::now();
    // Your operation
    metrics.record(start.elapsed().as_millis() as u64);
}

metrics.log_summary("my_operation");
```

### 4. Built-in Rust Benchmarks

Legacy benchmarks using `#[bench]` (requires nightly):

```bash
cargo +nightly bench --package jamey-core --bench benchmarks
```

## Running Benchmarks

### Quick Performance Check

```bash
# Run criterion benchmarks (recommended)
cargo bench --package jamey-core

# View HTML reports
# Open target/criterion/report/index.html in browser
```

### Detailed Profiling

```bash
# With flamegraph generation (requires cargo-flamegraph)
cargo install flamegraph
cargo flamegraph --bench criterion_benchmarks

# With perf profiling (Linux only)
cargo bench --package jamey-core -- --profile-time=5
```

### Continuous Benchmarking

Set up baseline comparisons for regression detection:

```bash
# Save current performance as baseline
cargo bench --package jamey-core -- --save-baseline current

# After making changes, compare against baseline
cargo bench --package jamey-core -- --baseline current

# View comparison report in target/criterion/
```

## Performance Testing Strategy

### 1. Unit-Level Performance Tests

Add performance assertions to critical paths:

```rust
#[tokio::test]
async fn test_memory_store_performance() {
    let store = setup_test_store().await;
    let memory = create_test_memory();
    
    let start = std::time::Instant::now();
    let _ = store.store(memory).await.unwrap();
    let duration = start.elapsed();
    
    // Assert performance requirement
    assert!(duration.as_millis() < 50, 
        "Memory store took {}ms, expected <50ms", 
        duration.as_millis());
}
```

### 2. Load Testing

Use the existing load tests in `jamey-core/tests/load_tests.rs`:

```bash
cargo test --package jamey-core --test load_tests -- --nocapture
```

### 3. Regression Testing

Automated performance regression detection:

```bash
# Run before changes
cargo bench --package jamey-core -- --save-baseline before

# Make your changes...

# Run after changes and compare
cargo bench --package jamey-core -- --baseline before

# Check for regressions (>10% slowdown)
# Criterion will highlight significant changes
```

### 4. Production-Like Testing

Test with realistic data volumes:

```rust
// In your test
const TEST_MEMORY_COUNT: usize = 10_000;
const TEST_VECTOR_DIM: usize = 1536;

// Populate with realistic data
for i in 0..TEST_MEMORY_COUNT {
    let memory = create_realistic_memory(i);
    store.store(memory).await?;
}

// Test pagination performance
let (results, total) = store.list_paginated(100, 0).await?;
assert_eq!(total, TEST_MEMORY_COUNT as i64);
```

## Optimization Guidelines

### 1. Arc Usage Optimization

**When to use Arc:**
- Sharing read-only data across threads
- Large data structures that are expensive to clone
- Configuration objects shared across components

**When NOT to use Arc:**
- Small data structures (<100 bytes)
- Data that's only used in a single thread
- Mutable data (consider `Arc<Mutex<T>>` or `Arc<RwLock<T>>`)

**Example:**
```rust
// Good: Shared configuration
let config = Arc::new(RuntimeConfig::load());
let session_manager = SessionManager::new(Arc::clone(&config));

// Bad: Unnecessary Arc for small data
let count = Arc::new(42); // Just use i32 directly

// Good: Use references instead of cloning Arc
fn process_config(config: &RuntimeConfig) { }
process_config(&config); // Not Arc::clone(&config)
```

### 2. Memory Operation Optimization

**Use pagination for large result sets:**
```rust
// Bad: Loading all memories at once
let all_memories = session.list_memories(); // Could be huge!

// Good: Use pagination
let (page, total) = session.list_memories_paginated(100, 0);
```

**Use iterators for streaming:**
```rust
// Good: Stream processing without collecting
for memory in session.iter_memories() {
    process_memory(memory);
}
```

### 3. Database Query Optimization

**Batch operations when possible:**
```rust
// Bad: Multiple individual queries
for id in ids {
    store.retrieve(id).await?;
}

// Good: Single batch query (if supported)
let memories = store.retrieve_batch(&ids).await?;
```

**Use appropriate indexes:**
- Vector similarity: IVFFlat index (already configured)
- Timestamp queries: Index on `created_at`, `last_accessed`
- Type filtering: Index on `memory_type`

### 4. Caching Strategy

**Cache hot data:**
```rust
// Use CachedMemoryStore for frequently accessed memories
let cached_store = CachedMemoryStore::new(
    postgres_store,
    cache_config
).await?;
```

**Set appropriate TTLs:**
- Frequently changing data: 60-300 seconds
- Stable data: 3600+ seconds
- Configuration: Until restart

## Monitoring in Production

### 1. Metrics Collection

The runtime includes built-in metrics via the `metrics` crate:

```rust
use metrics::{counter, histogram, gauge};

// Track operation counts
counter!("memory.store.total").increment(1);

// Track operation duration
histogram!("memory.store.duration_ms").record(duration_ms);

// Track current state
gauge!("memory.cache.size").set(cache_size as f64);
```

### 2. Prometheus Integration

Metrics are exported via `metrics-exporter-prometheus`:

```bash
# Metrics endpoint (default)
curl http://localhost:9090/metrics
```

### 3. Tracing Integration

Configure structured logging for production:

```rust
use jamey_core::secure_logging::{init_secure_logging, LogConfig};

let log_config = LogConfig {
    level: "info",
    format: "json",
    output: "file",
    file_path: Some("/var/log/jamey/app.log"),
};

init_secure_logging(log_config)?;
```

### 4. Performance Alerts

Set up alerts for:
- Operation duration > 2x baseline
- Cache hit rate < 80%
- Database connection pool exhaustion
- Memory usage > 80% of limit

### 5. Dashboard Metrics

Key metrics to monitor:

**Throughput:**
- Requests per second
- Operations per second (by type)

**Latency:**
- P50, P95, P99 response times
- Database query times
- Cache operation times

**Resources:**
- CPU usage
- Memory usage
- Database connection pool utilization
- Cache hit/miss ratio

**Errors:**
- Error rate by operation type
- Database connection errors
- Cache failures

## Performance Regression Testing

### Automated CI/CD Integration

Add to your CI pipeline:

```yaml
# .github/workflows/performance.yml
name: Performance Tests

on: [pull_request]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Run benchmarks
        run: |
          cargo bench --package jamey-core -- --save-baseline pr
          cargo bench --package jamey-core -- --baseline main
      - name: Check for regressions
        run: |
          # Parse criterion output for regressions
          # Fail if any benchmark is >10% slower
```

### Manual Regression Checks

Before merging performance-sensitive changes:

1. Run baseline benchmarks on main branch
2. Switch to your feature branch
3. Run benchmarks and compare
4. Document any intentional performance trade-offs

## Troubleshooting Performance Issues

### Slow Memory Operations

1. Check database connection pool:
   ```rust
   let status = pools.postgres.status();
   println!("Available connections: {}", status.available);
   ```

2. Verify indexes are being used:
   ```sql
   EXPLAIN ANALYZE SELECT * FROM memories
   WHERE embedding <=> $1::vector
   ORDER BY embedding <=> $1::vector
   LIMIT 10;
   ```

## Related Documentation

- [Operations Overview](README.md) - Operational procedures
- [System Architecture](../architecture/system-overview.md) - Architecture overview
- [Cache Invalidation](../architecture/cache-invalidation.md) - Cache performance
- [Testing Best Practices](../testing/best-practices.md) - Performance testing

---

**Last Updated**: 2025-11-17
**Status**: ✅ Complete
**Category**: Operations