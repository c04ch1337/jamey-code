# Self-Improvement Guide

The Self-Improvement connector enables Jamey 2.0 to read and modify its own source code with automatic backup and rollback capabilities. This powerful feature allows the AI to fix bugs, add features, and improve itself autonomously.

> ⚠️ **Critical**: Self-modification is a high-risk operation. Always review changes in development before deploying to production.

## Table of Contents

- [Overview](#overview)
- [How It Works](#how-it-works)
- [Configuration](#configuration)
- [Usage Examples](#usage-examples)
- [Backup System](#backup-system)
- [Rollback Procedures](#rollback-procedures)
- [Approval Workflow](#approval-workflow)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)
- [Security Considerations](#security-considerations)

## Overview

**Connector ID**: `self_improve`  
**Capability Level**: `SelfModify`  
**Requires Approval**: ✅ Yes  
**Source**: [`jamey-tools/src/connectors/self_improve.rs`](../../jamey-tools/src/connectors/self_improve.rs)

### Key Features

- ✅ **Automatic Backups**: Every modification creates a timestamped backup
- ✅ **Source Validation**: Only allows modifications to recognized source files
- ✅ **Rollback Support**: Restore previous versions from backups
- ✅ **Approval Required**: Explicit confirmation needed for all modifications
- ✅ **Audit Trail**: All operations logged with structured tracing

### Supported File Types

The connector validates and allows modifications to:
- **Rust source**: `.rs` files
- **Configuration**: `.toml` files
- **Documentation**: `.md` files

## How It Works

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│              Self-Improvement Workflow                  │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
         ┌────────────────────────────────┐
         │  1. Read Source File           │
         │     (validate file type)       │
         └────────────────┬───────────────┘
                          │
                          ▼
         ┌────────────────────────────────┐
         │  2. Create Backup              │
         │     (timestamped .bak file)    │
         └────────────────┬───────────────┘
                          │
                          ▼
         ┌────────────────────────────────┐
         │  3. Request Approval           │
         │     (requires confirmed=true)  │
         └────────────────┬───────────────┘
                          │
                          ▼
         ┌────────────────────────────────┐
         │  4. Write New Content          │
         │     (atomic file write)        │
         └────────────────┬───────────────┘
                          │
                          ▼
         ┌────────────────────────────────┐
         │  5. Log Operation              │
         │     (audit trail)              │
         └────────────────────────────────┘
```

### Safety Mechanisms

1. **Pre-modification Backup**: Automatic backup before any change
2. **File Type Validation**: Only recognized source files can be modified
3. **Approval Requirement**: Explicit confirmation parameter required
4. **Atomic Writes**: File operations are atomic to prevent corruption
5. **Backup Retention**: Configurable number of backups kept

## Configuration

### Environment Variables

Add to your `.env` file:

```bash
# Backup directory for self-modifications
BACKUP_DIR=./backups

# Number of backups to retain per file (default: 5)
SELF_MODIFY_BACKUP_COUNT=5
```

### Runtime Configuration

```rust
use jamey_runtime::config::RuntimeConfig;

let config = RuntimeConfig::from_env()?;

// Backup configuration
println!("Backup directory: {:?}", config.tools.backup_dir);
println!("Backup count: {}", config.tools.self_modify_backup_count);
```

### Initialization

```rust
use jamey_tools::connectors::SelfImproveConnector;
use std::path::PathBuf;

// Create connector with backup configuration
let connector = SelfImproveConnector::new(
    PathBuf::from("./backups"),  // Backup directory
    5                             // Number of backups to keep
)?;
```

## Usage Examples

### Example 1: Read Source File

```rust
use std::collections::HashMap;

let mut params = HashMap::new();
params.insert("action".to_string(), "read_file".to_string());
params.insert("file_path".to_string(), "src/main.rs".to_string());

let result = orchestrator
    .execute_connector("self_improve", params)
    .await?;

println!("File content:\n{}", result.output);
```

### Example 2: Modify Source File

```rust
use std::collections::HashMap;

let new_content = r#"
// Updated main.rs with bug fix
fn main() {
    println!("Hello, fixed world!");
}
"#;

let mut params = HashMap::new();
params.insert("action".to_string(), "modify_file".to_string());
params.insert("file_path".to_string(), "src/main.rs".to_string());
params.insert("content".to_string(), new_content.to_string());
params.insert("confirmed".to_string(), "true".to_string()); // Required!

let result = orchestrator
    .execute_connector("self_improve", params)
    .await?;

if result.success {
    println!("✅ File modified successfully");
    println!("Backup: {}", result.metadata.get("backup_path").unwrap());
} else {
    eprintln!("❌ Modification failed: {:?}", result.errors);
}
```

### Example 3: List Source Files

```rust
use std::collections::HashMap;

let mut params = HashMap::new();
params.insert("action".to_string(), "list_source_files".to_string());
params.insert("pattern".to_string(), "**/*.rs".to_string());

let result = orchestrator
    .execute_connector("self_improve", params)
    .await?;

let files: Vec<String> = serde_json::from_str(&result.output)?;
println!("Found {} source files", files.len());
for file in files {
    println!("  - {}", file);
}
```

### Example 4: Update Configuration

```rust
use std::collections::HashMap;

let new_config = r#"
[package]
name = "jamey-core"
version = "2.1.0"  # Version bump
edition = "2021"
"#;

let mut params = HashMap::new();
params.insert("action".to_string(), "modify_file".to_string());
params.insert("file_path".to_string(), "jamey-core/Cargo.toml".to_string());
params.insert("content".to_string(), new_config.to_string());
params.insert("confirmed".to_string(), "true".to_string());

let result = orchestrator
    .execute_connector("self_improve", params)
    .await?;
```

## Backup System

### Backup File Format

Backups are created with the following naming convention:

```
{original_filename}.{timestamp}.bak
```

**Example**:
```
main.rs.20251117_052530.bak
```

### Backup Structure

```rust
pub struct FileBackup {
    pub original_path: PathBuf,      // Original file location
    pub backup_path: PathBuf,        // Backup file location
    pub timestamp: DateTime<Utc>,    // When backup was created
}
```

### Backup Location

All backups are stored in the configured backup directory:

```
./backups/
├── main.rs.20251117_052530.bak
├── main.rs.20251117_053045.bak
├── config.toml.20251117_054120.bak
└── README.md.20251117_055200.bak
```

### Backup Retention

The system automatically manages backup retention:

```rust
// Configure retention count
let connector = SelfImproveConnector::new(
    PathBuf::from("./backups"),
    5  // Keep last 5 backups per file
)?;
```

When the limit is exceeded, oldest backups are automatically removed.

## Rollback Procedures

### Manual Rollback

To restore a previous version:

```bash
# 1. Identify the backup file
ls -la backups/

# 2. Copy backup to original location
cp backups/main.rs.20251117_052530.bak src/main.rs

# 3. Verify restoration
cat src/main.rs
```

### Programmatic Rollback

```rust
use jamey_tools::system::SelfModifyTool;
use std::path::PathBuf;

let tool = SelfModifyTool::new("./backups")?;

// Restore from backup metadata
let backup = FileBackup {
    original_path: PathBuf::from("src/main.rs"),
    backup_path: PathBuf::from("backups/main.rs.20251117_052530.bak"),
    timestamp: Utc::now(),
};

tool.restore_backup(&backup)?;
println!("✅ File restored from backup");
```

### Emergency Rollback

If the system becomes unstable after a modification:

```bash
# 1. Stop the service
cargo run --package jamey-cli -- stop

# 2. Restore from backup
cp backups/critical_file.rs.{timestamp}.bak src/critical_file.rs

# 3. Rebuild
cargo build --release

# 4. Restart service
cargo run --package jamey-cli -- start
```

## Approval Workflow

### Why Approval is Required

Self-modification is a **high-risk operation** that can:
- Introduce bugs or security vulnerabilities
- Break system functionality
- Cause data loss or corruption
- Create infinite modification loops

### Approval Process

1. **Request Initiated**: AI proposes a modification
2. **Backup Created**: Automatic backup before any change
3. **Approval Check**: System verifies `confirmed` parameter
4. **Modification Applied**: Only if approval is present
5. **Audit Log**: Operation logged for review

### Implementing Approval

```rust
// ❌ This will fail - no approval
let mut params = HashMap::new();
params.insert("action".to_string(), "modify_file".to_string());
params.insert("file_path".to_string(), "src/main.rs".to_string());
params.insert("content".to_string(), new_content.to_string());
// Missing: confirmed parameter

// ✅ This will succeed - approval provided
params.insert("confirmed".to_string(), "true".to_string());
```

### Custom Approval Logic

You can implement custom approval workflows:

```rust
async fn approve_modification(
    file_path: &str,
    old_content: &str,
    new_content: &str,
) -> bool {
    // Show diff to user
    println!("Proposed changes to {}:", file_path);
    println!("--- OLD");
    println!("{}", old_content);
    println!("+++ NEW");
    println!("{}", new_content);
    
    // Request user confirmation
    print!("Approve modification? (yes/no): ");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    
    input.trim().to_lowercase() == "yes"
}
```

## Best Practices

### ✅ Do

1. **Test in Development First**
   ```bash
   # Always test modifications in dev environment
   cargo test
   cargo build
   cargo run
   ```

2. **Review Changes Before Approval**
   - Read the proposed modifications carefully
   - Understand the impact of changes
   - Verify the logic is correct

3. **Maintain External Backups**
   ```bash
   # Create git commits before self-modifications
   git add .
   git commit -m "Pre-modification checkpoint"
   ```

4. **Monitor After Modifications**
   ```bash
   # Watch logs for errors
   tail -f logs/jamey.log
   ```

5. **Use Version Control**
   ```bash
   # Track all changes in git
   git diff src/main.rs
   git log --oneline
   ```

6. **Validate After Changes**
   ```bash
   # Run tests after modifications
   cargo test --all
   cargo clippy
   ```

### ❌ Don't

1. **Don't Auto-Approve in Production**
   - Always require manual approval for production systems
   - Never set `confirmed=true` automatically

2. **Don't Modify Critical Files Without Review**
   - Core runtime files
   - Security-related code
   - Database schemas

3. **Don't Skip Backups**
   - Never disable the backup system
   - Always verify backups are created

4. **Don't Ignore Warnings**
   - Pay attention to file type validation warnings
   - Review all error messages

5. **Don't Modify Multiple Files Simultaneously**
   - Make one change at a time
   - Test each modification independently

## Troubleshooting

### Issue: "Self-modification requires explicit confirmation"

**Cause**: Missing `confirmed` parameter

**Solution**:
```rust
params.insert("confirmed".to_string(), "true".to_string());
```

### Issue: "File is not a recognized source file"

**Cause**: Attempting to modify unsupported file type

**Solution**: Only modify `.rs`, `.toml`, or `.md` files

### Issue: "Failed to create backup"

**Cause**: Backup directory doesn't exist or insufficient permissions

**Solution**:
```bash
# Create backup directory
mkdir -p ./backups

# Fix permissions
chmod 755 ./backups
```

### Issue: "File modified but tests fail"

**Cause**: Modification introduced bugs

**Solution**:
```bash
# Restore from backup
cp backups/file.rs.{timestamp}.bak src/file.rs

# Rebuild and test
cargo build
cargo test
```

### Issue: "Backup directory full"

**Cause**: Too many backups accumulated

**Solution**:
```bash
# Clean old backups (keep last 10)
cd backups
ls -t *.bak | tail -n +11 | xargs rm
```

## Security Considerations

### Threat Model

**Threats**:
- Malicious code injection
- Infinite modification loops
- Privilege escalation
- Data exfiltration through code changes

**Mitigations**:
- Approval workflow (prevents unauthorized changes)
- File type validation (limits attack surface)
- Backup system (enables recovery)
- Audit logging (tracks all modifications)

### Security Controls

1. **Approval Requirement**
   - All modifications require explicit confirmation
   - Prevents automated malicious changes

2. **File Type Validation**
   - Only recognized source files can be modified
   - Prevents modification of binaries or system files

3. **Backup System**
   - Automatic backups before changes
   - Enables quick rollback if compromised

4. **Audit Logging**
   ```rust
   tracing::warn!("Self-modification: {} -> {}", 
       file_path, backup_path);
   ```

5. **Path Validation**
   - Prevents modification of files outside project
   - Blocks directory traversal attempts

### Security Checklist

- [ ] Approval workflow enabled
- [ ] Backup directory configured and writable
- [ ] Audit logging enabled
- [ ] File type validation active
- [ ] External backups (git) maintained
- [ ] Monitoring alerts configured
- [ ] Rollback procedures tested
- [ ] Access controls on backup directory

## Related Documentation

- [AI Agent Overview](README.md) - Overview of all agent capabilities
- [Security Best Practices](security-best-practices.md) - Security guidelines
- [Full System Access](full-system-access.md) - File system operations
- [Admin Assistant](admin-assistant.md) - System administration

## API Reference

### Actions

#### `read_file`
Read the contents of a source file.

**Parameters**:
- `action`: `"read_file"`
- `file_path`: Path to file (relative to project root)

**Returns**: File content as string

#### `modify_file`
Modify a source file with automatic backup.

**Parameters**:
- `action`: `"modify_file"`
- `file_path`: Path to file
- `content`: New file content
- `confirmed`: `"true"` (required)

**Returns**: Backup information

#### `list_source_files`
List source files matching a pattern.

**Parameters**:
- `action`: `"list_source_files"`
- `pattern`: Glob pattern (default: `"**/*.rs"`)

**Returns**: Array of file paths

---

**Last Updated**: 2025-11-17  
**Version**: 1.0.0  
**Status**: ✅ Complete