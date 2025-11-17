# Jamey 2.0 Documentation

Welcome to the comprehensive documentation for Jamey 2.0 - the Digital Twin and Guardian of the Eternal Hive system.

## Quick Links

- 🚀 [Quick Start Guide](getting-started/quick-start.md)
- 🗄️ [Database Setup](getting-started/database-setup.md)
- 🏗️ [System Architecture](architecture/system-overview.md)
- 🔒 [Security Overview](security/README.md)
- 📊 [Performance Monitoring](operations/performance-monitoring.md)
- 🧪 [Testing Guide](testing/best-practices.md)

## Documentation Categories

### 🤖 AI Agent Capabilities
Comprehensive guides for Jamey 2.0's autonomous AI agent features.

- [AI Agent Overview](ai-agent/README.md) - Introduction to agent capabilities
- [Self-Improvement](ai-agent/self-improvement.md) - Code modification with automatic backups
- [Admin Assistant](ai-agent/admin-assistant.md) - System administration and process management
- [Full System Access](ai-agent/full-system-access.md) - File system and command execution
- [Network & Web Access](ai-agent/network-access.md) - Web search, downloads, and URL fetching
- [Agent Orchestration](ai-agent/orchestration.md) - Multi-agent coordination
- [24/7 Service Mode](ai-agent/always-on.md) - Continuous operation with scheduling
- [Security Best Practices](ai-agent/security-best-practices.md) - Security guidelines and controls

### 🚀 Getting Started
Essential guides for setting up and running Jamey 2.0.

- [Database Setup](getting-started/database-setup.md) - PostgreSQL and pgvector configuration
- [Quick Start Guide](getting-started/quick-start.md) - Get up and running in 5 minutes *(Coming Soon)*
- [Installation Guide](getting-started/installation.md) - Detailed installation instructions *(Coming Soon)*

### 🏗️ Architecture
Technical architecture, design decisions, and system specifications.

- [System Overview](architecture/system-overview.md) - High-level architecture with diagrams
- [Cache Invalidation](architecture/cache-invalidation.md) - Cache strategy architecture
- [Configuration System](architecture/configuration.md) - Granular configuration design
- [Pagination](architecture/pagination.md) - Pagination strategies and implementation
- [Improvements Summary](architecture/improvements-summary.md) - Architectural enhancements roadmap

### 🔒 Security
Security architecture, cryptography, and best practices.

- [Security Overview](security/README.md) - Security principles and threat model *(Coming Soon)*
- [Log Security](security/log-security.md) - Secure logging and PII protection
- [TLS Configuration](security/tls-configuration.md) - HTTPS and certificate management
- **TA-QR Cryptographic Stack**:
  - [TA-QR Overview](security/ta-qr/README.md) - Quantum-resistant crypto introduction
  - [Architecture](security/ta-qr/architecture.md) - TA-QR design and algorithms
  - [Implementation Spec](security/ta-qr/implementation-spec.md) - Technical specifications
  - [Usage Guide](security/ta-qr/usage-guide.md) - Migration and usage patterns

### 📊 Operations
Deployment, monitoring, and operational procedures.

- [Performance Monitoring](operations/performance-monitoring.md) - Metrics, profiling, and optimization
- [Deployment Guide](operations/deployment.md) - Production deployment procedures *(Coming Soon)*

### 🧪 Testing
Testing strategies, best practices, and guidelines.

- [Testing Best Practices](testing/best-practices.md) - Comprehensive testing guide
- [Testing Strategy](testing/strategy.md) - Overall testing approach and setup

### 📚 Reference
Additional reference materials and historical documents.

- [Audit Report](reference/audit-report.md) - Security and code quality audit
- [Digital Twin Notes](reference/digital-twin-notes.md) - Historical implementation notes (archived)

### 📋 Architecture Decision Records (ADRs)
Documented architectural decisions and their rationale.

- [ADR Index](adr/README.md) - All architecture decisions *(Coming Soon)*
- [ADR 001: Cache Invalidation Strategies](adr/001-cache-invalidation-strategies.md)

## Document Status Legend

- ✅ **Complete** - Fully documented and up-to-date
- 🔄 **In Progress** - Actively being updated
- 📝 **Draft** - Initial version, needs review
- 🗄️ **Archived** - Historical reference only

## Contributing to Documentation

When adding or updating documentation:

1. Place documents in the appropriate category directory
2. Update the relevant README.md index
3. Add "Last Updated" date and "Related Documents" section
4. Ensure all code examples are properly formatted with language tags
5. Verify all internal links work correctly
6. Follow the [markdown formatting standards](#markdown-standards)

## Markdown Standards

All documentation should follow these standards:

- Use ATX-style headers (`#` not underlines)
- Include language tags for code blocks (e.g., ` ```rust `)
- Use relative links for internal documentation
- Include a "Last Updated" date at the bottom
- Add "Related Documents" section where applicable
- Use consistent terminology (see glossary below)

## Glossary

- **TA-QR**: Trusted Agent - Quantum Resistant (cryptographic stack)
- **ORCH**: Orchestrator nodes in the Eternal Hive
- **Phoenix.Marie**: The Queen of the Eternal Hive (Ubuntu system)
- **Jamey 2.0**: The General and Guardian (Windows system)
- **Digital Twin**: AI representation with full system access
- **Eternal Hive**: The complete distributed AI system

## Getting Help

- **Questions**: Open an issue on GitHub
- **Security**: Email security@jamey.dev
- **Documentation Issues**: Tag with `documentation` label
- **General Discussion**: Use GitHub Discussions

## Project Links

- [Main README](../README.md) - Project overview and setup
- [Contributing Guide](../CONTRIBUTING.md) - How to contribute *(Coming Soon)*
- [Code of Conduct](../CODE_OF_CONDUCT.md) - Community guidelines *(Coming Soon)*

---

**Last Updated**: 2025-11-17  
**Version**: 2.0.0  
**Maintained by**: Jamey Code Team