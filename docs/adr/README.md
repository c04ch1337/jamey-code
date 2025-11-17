# Architecture Decision Records (ADRs)

This directory contains Architecture Decision Records (ADRs) documenting significant architectural decisions made during the development of Jamey 2.0.

## What is an ADR?

An Architecture Decision Record (ADR) captures an important architectural decision made along with its context and consequences. ADRs help teams:

- Understand why decisions were made
- Avoid revisiting settled decisions
- Onboard new team members
- Track architectural evolution

## ADR Format

Each ADR follows this structure:

1. **Title**: Short, descriptive name
2. **Status**: Proposed, Accepted, Deprecated, Superseded
3. **Context**: The issue motivating this decision
4. **Decision**: The change being proposed or made
5. **Consequences**: Positive, negative, and neutral outcomes
6. **Alternatives Considered**: Other options and why they were rejected

## Active ADRs

### Cache and Performance

- [ADR 001: Cache Invalidation Strategies](001-cache-invalidation-strategies.md) - Trait-based invalidation system

### Planned ADRs

- **ADR 002**: Granular Configuration System *(Pending)*
- **ADR 003**: Pagination Strategy Selection *(Pending)*
- **ADR 004**: TA-QR Cryptographic Stack *(Pending)*
- **ADR 005**: Multi-Model Embedding Support *(Pending)*

## ADR Index

| Number | Title | Status | Date | Category |
|--------|-------|--------|------|----------|
| [001](001-cache-invalidation-strategies.md) | Cache Invalidation Strategies | Proposed | 2025-11-17 | Performance |

## Creating a New ADR

When making a significant architectural decision:

1. **Copy the template**: Use an existing ADR as a template
2. **Assign a number**: Use the next sequential number
3. **Fill in sections**: Context, Decision, Consequences, Alternatives
4. **Get review**: Have the team review before accepting
5. **Update index**: Add to the table above
6. **Link related docs**: Reference from architecture documentation

### ADR Naming Convention

```
NNN-short-descriptive-title.md
```

Examples:
- `001-cache-invalidation-strategies.md`
- `002-granular-configuration-system.md`
- `003-pagination-strategy-selection.md`

## ADR Lifecycle

```
Proposed → Accepted → [Deprecated/Superseded]
```

- **Proposed**: Under discussion
- **Accepted**: Approved and being implemented
- **Deprecated**: No longer recommended
- **Superseded**: Replaced by a newer ADR

## Related Documentation

- [Architecture Overview](../architecture/README.md) - System architecture
- [Improvements Summary](../architecture/improvements-summary.md) - Enhancement roadmap
- [Main Documentation](../README.md) - Documentation hub

## References

- [ADR GitHub Organization](https://adr.github.io/)
- [Documenting Architecture Decisions](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions)
- [ADR Tools](https://github.com/npryce/adr-tools)

---

**Last Updated**: 2025-11-17  
**Status**: ✅ Complete  
**Total ADRs**: 1 active, 4 planned