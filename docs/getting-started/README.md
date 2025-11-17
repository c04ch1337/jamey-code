# Getting Started with Jamey 2.0

Welcome! This guide will help you get Jamey 2.0 up and running quickly.

## Prerequisites

Before you begin, ensure you have:

- **Rust**: Version 1.70 or later ([Install Rust](https://rustup.rs/))
- **PostgreSQL**: Version 14 or later with pgvector extension
- **Redis**: Version 6 or later (optional, for caching)
- **Git**: For cloning the repository

### System Requirements

- **OS**: Windows 10/11, Linux, or macOS
- **RAM**: Minimum 4GB, recommended 8GB+
- **Disk**: 10GB free space
- **Network**: Internet connection for LLM API access

## Quick Start (5 Minutes)

### 1. Clone the Repository

```bash
git clone https://github.com/jamey-code/jamey.git
cd jamey-code
```

### 2. Set Up Database

Follow the [Database Setup Guide](database-setup.md) to configure PostgreSQL with pgvector.

Quick version:
```bash
# Windows
powershell -ExecutionPolicy Bypass -File setup-db.ps1

# Linux/macOS
./install.sh
```

### 3. Configure Environment

```bash
# Copy example environment file
cp .env.local.example .env.local

# Edit .env.local with your settings
# - Database credentials
# - OpenRouter API key
# - Other configuration
```

### 4. Build and Run

```bash
# Build all crates
cargo build --release

# Run the runtime
cargo run --package jamey-runtime

# In another terminal, use the CLI
cargo run --package jamey-cli -- chat
```

## Detailed Setup Guides

- [Database Setup](database-setup.md) - PostgreSQL and pgvector configuration
- [Installation Guide](installation.md) - Detailed installation for all platforms *(Coming Soon)*
- [Configuration Guide](../architecture/configuration.md) - Configuration options and tuning

## First Steps After Installation

### 1. Verify Installation

```bash
# Check database connection
cargo run --package jamey-cli -- status

# Test memory operations
cargo run --package jamey-cli -- memory list
```

### 2. Start a Chat Session

```bash
# Interactive chat
cargo run --package jamey-cli -- chat

# Or use the TUI
cargo run --package jamey-tui
```

### 3. Explore System Information

```bash
# View system status
cargo run --package jamey-cli -- system info

# Check running processes
cargo run --package jamey-cli -- process list
```

## Configuration Overview

### Essential Configuration

Edit `.env.local` with these required settings:

```bash
# Database
POSTGRES_HOST=localhost
POSTGRES_PORT=5432
POSTGRES_DB=jamey
POSTGRES_USER=jamey
POSTGRES_PASSWORD=your_secure_password

# LLM Provider
LLM_PROVIDER=openrouter
OPENROUTER_API_KEY=your_api_key_here
OPENROUTER_MODEL=anthropic/claude-3.5-sonnet

# Runtime
RUNTIME_HOST=0.0.0.0
RUNTIME_PORT=3000
```

### Optional Configuration

```bash
# Redis (for caching)
REDIS_URL=redis://localhost:6379

# Security
ENABLE_HTTPS=true
TLS_CERT_PATH=/path/to/cert.pem
TLS_KEY_PATH=/path/to/key.pem

# Performance
POSTGRES_MAX_CONNECTIONS=20
CACHE_MEMORY_CAPACITY=10000
```

## Architecture Overview

Jamey 2.0 consists of several crates:

- **jamey-core**: Memory, cache, and security
- **jamey-runtime**: Main orchestration engine
- **jamey-providers**: LLM provider integrations
- **jamey-tools**: System tools and utilities
- **jamey-cli**: Command-line interface
- **jamey-tui**: Terminal user interface

See the [Architecture Overview](../architecture/system-overview.md) for details.

## Common Tasks

### Managing Memory

```bash
# List memories
jamey-cli memory list

# Search memories
jamey-cli memory search "query text"

# Delete a memory
jamey-cli memory delete <memory-id>
```

### Process Management

```bash
# List processes
jamey-cli process list

# Get process info
jamey-cli process info <pid>
```

### System Operations

```bash
# View system information
jamey-cli system info

# Check health
jamey-cli system health
```

## Troubleshooting

### Database Connection Issues

If you see database connection errors:

1. Verify PostgreSQL is running
2. Check credentials in `.env.local`
3. Ensure pgvector extension is installed
4. Test connection: `psql -U jamey -d jamey`

See [Database Setup](database-setup.md) for detailed troubleshooting.

### Build Errors

If you encounter build errors:

```bash
# Clean and rebuild
cargo clean
cargo build --workspace

# Update dependencies
cargo update

# Check for conflicts
cargo tree
```

### Runtime Errors

If the runtime fails to start:

1. Check logs in `logs/jamey.log`
2. Verify all environment variables are set
3. Ensure ports are not in use
4. Check database connectivity

## Next Steps

After getting Jamey 2.0 running:

1. **Explore the Architecture**: Read the [System Overview](../architecture/system-overview.md)
2. **Learn About Security**: Review [Security Documentation](../security/README.md)
3. **Understand Testing**: Check [Testing Best Practices](../testing/best-practices.md)
4. **Monitor Performance**: See [Performance Monitoring](../operations/performance-monitoring.md)

## Getting Help

- **Documentation**: Browse the [main documentation index](../README.md)
- **Issues**: Open a GitHub issue
- **Questions**: Use GitHub Discussions
- **Security**: Email security@jamey.dev

## Related Documentation

- [Main Documentation Index](../README.md)
- [Architecture Overview](../architecture/system-overview.md)
- [Security Overview](../security/README.md)
- [Testing Guide](../testing/README.md)

---

**Last Updated**: 2025-11-17  
**Status**: ✅ Complete