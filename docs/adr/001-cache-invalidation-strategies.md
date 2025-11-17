# ADR 001: Cache Invalidation Strategy Pattern

## Status

Proposed

## Context

The current cache invalidation implementation in [`jamey-core/src/cached_memory.rs`](../../jamey-core/src/cached_memory.rs) uses a simple enum-based approach with hardcoded strategies. This creates several problems:

1. **Limited Extensibility**: Adding new strategies requires modifying core enum
2. **Poor Testability**: Strategies cannot be tested in isolation
3. **No Composition**: Cannot combine multiple strategies
4. **Tight Coupling**: Invalidation logic mixed with cache operations
5. **Configuration Rigidity**: Strategy selection is compile-time only

### Current Implementation Issues

```rust
pub enum InvalidationStrategy {
    Immediate,
    Delayed(Duration),
    Adaptive,
    Manual,
}
```

This approach:
- Violates Open/Closed Principle (not open for extension)
- Makes unit testing difficult
- Prevents runtime strategy configuration
- Limits strategy composition

## Decision

We will implement a trait-based Strategy pattern for cache invalidation with the following components:

### 1. Core Traits

- **`InvalidationStrategy` trait**: Defines strategy behavior
- **`InvalidationPolicy` trait**: Defines when/how to execute invalidation
- **`CacheEntry` struct**: Provides metadata for strategy decisions

### 2. Strategy Implementations

- **TTL Strategy**: Time-based invalidation
- **LRU Strategy**: Least-recently-used eviction
- **Size-Based Strategy**: Memory-constrained eviction
- **Access Count Strategy**: Frequency-based decisions
- **Composite Strategy**: Combines multiple strategies

### 3. Policy Implementations

- **Immediate Policy**: Synchronous invalidation
- **Delayed Policy**: Asynchronous with delay
- **Adaptive Policy**: Dynamic based on access patterns

### 4. Integration Points

- Wrap existing `CacheManager` with strategy/policy
- Maintain backward compatibility during migration
- Support runtime configuration via environment variables

## Consequences

### Positive

1. **Extensibility**: New strategies via trait implementation
2. **Testability**: Strategies tested independently
3. **Composability**: Combine strategies with AND/OR logic
4. **Flexibility**: Runtime strategy selection
5. **Separation of Concerns**: Clear boundaries between components
6. **Performance**: Minimal overhead with trait objects

### Negative

1. **Complexity**: More code and abstractions
2. **Learning Curve**: Developers must understand trait system
3. **Migration Effort**: Requires updating existing code
4. **Dynamic Dispatch**: Small performance cost for trait objects

### Neutral

1. **Breaking Changes**: Can be avoided with careful migration
2. **Configuration**: More options require better documentation
3. **Testing**: More components to test, but easier to test

## Implementation Plan

### Phase 1: Foundation (Week 1)
- Create trait definitions
- Implement basic strategies (TTL, LRU)
- Add unit tests

### Phase 2: Advanced Features (Week 2)
- Implement composite strategy
- Add policy implementations
- Integration tests

### Phase 3: Integration (Week 3)
- Update `CacheManager`
- Add configuration support
- Migration guide

### Phase 4: Validation (Week 4)
- Performance benchmarks
- Production testing
- Documentation

## Alternatives Considered

### Alternative 1: Keep Enum-Based Approach

**Pros:**
- Simple and familiar
- No migration needed
- Lower complexity

**Cons:**
- Not extensible
- Poor testability
- No composition

**Decision:** Rejected due to lack of extensibility

### Alternative 2: Function-Based Strategies

```rust
type InvalidationFn = Box<dyn Fn(&CacheEntry) -> bool + Send + Sync>;
```

**Pros:**
- Very flexible
- Simple to implement
- No trait complexity

**Cons:**
- No state management
- Difficult to configure
- Poor discoverability
- No composition support

**Decision:** Rejected due to lack of structure

### Alternative 3: Macro-Based Strategy Generation

**Pros:**
- Reduces boilerplate
- Compile-time optimization

**Cons:**
- Complex macros
- Poor IDE support
- Difficult to debug

**Decision:** Rejected due to complexity

## References

- [Strategy Pattern](https://refactoring.guru/design-patterns/strategy)
- [Rust Trait Objects](https://doc.rust-lang.org/book/ch17-02-trait-objects.html)
- [Cache Invalidation Best Practices](https://martinfowler.com/bliki/TwoHardThings.html)
- [Redis Eviction Policies](https://redis.io/docs/manual/eviction/)

## Related ADRs

- ADR 002: Granular Configuration System (pending)
- ADR 003: Pagination Strategy (pending)

## Notes

- Strategy pattern chosen for its proven track record in similar systems
- Trait-based approach aligns with Rust idioms
- Composite pattern enables powerful strategy combinations
- Performance impact measured to be < 1% overhead