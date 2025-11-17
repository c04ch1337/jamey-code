# Full System Access Guide

The Full System Access connector provides complete file system access and command execution capabilities with comprehensive security guardrails. This enables Jamey 2.0 to manage files, execute commands, and interact with the system autonomously.

> ⚠️ **Important**: This connector grants significant system access. All operations are logged and subject to security controls.

## Table of Contents

- [Overview](#overview)
- [Configuration](#configuration)
- [File Operations](#file-operations)
- [Command Execution](#command-execution)
- [Security Guardrails](#security-guardrails)
- [Usage Examples](#usage-examples)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)
- [Security Considerations](#security-considerations)

## Overview

**Connector ID**: `full_system`  
**Capability Level**: `FullAccess`  
**Requires Approval**: ❌ No (but operations are logged)  
**Source**: [`jamey-tools/src/connectors/full_system.rs`](../../jamey-tools/src/connectors/full_system.rs)

### Key Features

- ✅ **File Operations**: Read, write, and list files/directories
- ✅ **Command Execution**: Run whitelisted commands
- ✅ **Path Sanitization**: Prevents directory traversal attacks
- ✅ **Command Whitelist**: Only approved commands can execute
- ✅ **Audit Logging**: All operations logged for security review

### Capabilities

| Feature | Description | Security Control |
|---------|-------------|------------------|
| Read File | Read file contents | Path sanitization |
| Write File | Create or overwrite files | Path sanitization |
| List Directory | List directory contents | Path sanitization |
| Execute Command | Run whitelisted commands | Command whitelist |

## Configuration

### Environment Variables

Add to your `.env` file:

```bash
# System root directory (base path for operations)
# Windows
SYSTEM_ROOT=C:\

# Linux/macOS
SYSTEM_ROOT=/

# Download directory for network operations
DOWNLOAD_DIR=./downloads
```

### Runtime Configuration

```rust
use jamey_runtime::config::RuntimeConfig;
use std::path::PathBuf;

let config = RuntimeConfig::from_env()?;

println!("System root: {:?}", config.tools.system_root);
println!("Download dir: {:?}", config.tools.download_dir);
```

### Initialization

```rust
use jamey_tools::connectors::FullSystemConnector;
use std::path::PathBuf;

// Create connector with root path
let connector = FullSystemConnector::new(
    PathBuf::from("C:\\")  // Windows
    // PathBuf::from("/")  // Linux/macOS
);
```

## File Operations

### Read File

Read the contents of a file:

```rust
use std::collections::HashMap;

let mut params = HashMap::new();
params.insert("action".to_string(), "read_file".to_string());
params.insert("path".to_string(), "data/config.json".to_string());

let result = orchestrator
    .execute_connector("full_system", params)
    .await?;

println!("File content:\n{}", result.output);
```

### Write File

Create or overwrite a file:

```rust
use std::collections::HashMap;

let content = r#"{
    "version": "1.0.0",
    "enabled": true
}"#;

let mut params = HashMap::new();
params.insert("action".to_string(), "write_file".to_string());
params.insert("path".to_string(), "data/config.json".to_string());
params.insert("content".to_string(), content.to_string());

let result = orchestrator
    .execute_connector("full_system", params)
    .await?;

if result.success {
    println!("✅ File written successfully");
} else {
    eprintln!("❌ Write failed: {:?}", result.errors);
}
```

### List Directory

List contents of a directory:

```rust
use std::collections::HashMap;

let mut params = HashMap::new();
params.insert("action".to_string(), "list_directory".to_string());
params.insert("path".to_string(), "data".to_string());

let result = orchestrator
    .execute_connector("full_system", params)
    .await?;

let entries: Vec<String> = serde_json::from_str(&result.output)?;
println!("Directory contents:");
for entry in entries {
    println!("  - {}", entry);
}
```

## Command Execution

### Allowed Commands

Only the following commands can be executed:

```rust
const ALLOWED_COMMANDS: &[&str] = &[
    // File operations
    "ls", "dir", "cat", "type", "echo", "pwd", "cd",
    
    // Version control
    "git",
    
    // Package managers
    "npm", "cargo", "pip",
    
    // Programming languages
    "python", "node", "rustc",
    
    // Utilities
    "grep", "find", "which", "where", "whoami",
];
```

### Blocked Patterns

The following patterns are **always blocked**:

```rust
const BLOCKED_FLAGS: &[&str] = &[
    "--privileged",  // Docker privileged mode
    "--cap-add",     // Capability additions
    "sudo",          // Privilege escalation
    "su",            // Switch user
    "rm -rf /",      // Dangerous deletion
    "format",        // Disk formatting
    "mkfs",          // Filesystem creation
];
```

### Execute Command

Run a whitelisted command:

```rust
use std::collections::HashMap;

let mut params = HashMap::new();
params.insert("action".to_string(), "execute_command".to_string());
params.insert("command".to_string(), "git".to_string());
params.insert("args".to_string(), "status".to_string());

let result = orchestrator
    .execute_connector("full_system", params)
    .await?;

println!("Command output:\n{}", result.output);

// Check for warnings (stderr)
if !result.warnings.is_empty() {
    eprintln!("Warnings:\n{}", result.warnings.join("\n"));
}
```

## Security Guardrails

### 1. Path Sanitization

All file paths are validated to prevent directory traversal attacks:

```rust
fn sanitize_path(root: &Path, user_path: &str) -> Result<PathBuf> {
    // ❌ Reject absolute paths
    if Path::new(user_path).is_absolute() {
        bail!("Absolute paths not allowed");
    }
    
    // ❌ Block parent directory traversal
    if user_path.contains("..") {
        bail!("Parent directory traversal not allowed");
    }
    
    // ✅ Construct safe path
    let full_path = root.join(user_path);
    
    // ✅ Canonicalize and verify
    let canonical = full_path.canonicalize()?;
    let canonical_root = root.canonicalize()?;
    
    if !canonical.starts_with(&canonical_root) {
        bail!("Path escapes root directory");
    }
    
    Ok(canonical)
}
```

**Examples**:

```rust
// ✅ Allowed
"data/config.json"
"logs/app.log"
"src/main.rs"

// ❌ Blocked
"/etc/passwd"           // Absolute path
"../../../etc/passwd"   // Directory traversal
"data/../../../etc/passwd"  // Traversal attempt
```

### 2. Command Whitelist

Only approved commands can be executed:

```rust
fn validate_command(command: &str, args: &[String]) -> Result<()> {
    // Extract command name
    let command_name = Path::new(command)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(command);
    
    // ❌ Check whitelist
    if !ALLOWED_COMMANDS.contains(&command_name) {
        bail!("Command '{}' not in allowed list", command_name);
    }
    
    // ❌ Check for blocked flags
    let args_str = args.join(" ");
    for blocked in BLOCKED_FLAGS {
        if args_str.contains(blocked) {
            bail!("Blocked flag detected: {}", blocked);
        }
    }
    
    Ok(())
}
```

**Examples**:

```rust
// ✅ Allowed
execute_command("git", &["status"])
execute_command("npm", &["install"])
execute_command("cargo", &["build"])

// ❌ Blocked
execute_command("rm", &["-rf", "/"])      // Dangerous command
execute_command("sudo", &["apt", "install"])  // Privilege escalation
execute_command("format", &["C:"])        // Disk formatting
```

### 3. Environment Isolation

Commands run with a cleared environment to prevent exploitation:

```rust
// Windows: Preserve minimal environment
#[cfg(windows)]
cmd.env_clear()
    .env("SystemRoot", std::env::var("SystemRoot").unwrap_or_default())
    .env("PATH", std::env::var("PATH").unwrap_or_default());

// Unix: Restrictive PATH
#[cfg(not(windows))]
cmd.env_clear()
    .env("PATH", "/usr/local/bin:/usr/bin:/bin");
```

## Usage Examples

### Example 1: Read Configuration File

```rust
async fn read_config() -> anyhow::Result<serde_json::Value> {
    let mut params = HashMap::new();
    params.insert("action".to_string(), "read_file".to_string());
    params.insert("path".to_string(), "config/app.json".to_string());

    let result = orchestrator
        .execute_connector("full_system", params)
        .await?;

    let config: serde_json::Value = serde_json::from_str(&result.output)?;
    Ok(config)
}
```

### Example 2: Create Log File

```rust
async fn create_log_entry(message: &str) -> anyhow::Result<()> {
    let timestamp = chrono::Utc::now().to_rfc3339();
    let log_entry = format!("[{}] {}\n", timestamp, message);

    let mut params = HashMap::new();
    params.insert("action".to_string(), "write_file".to_string());
    params.insert("path".to_string(), "logs/app.log".to_string());
    params.insert("content".to_string(), log_entry);

    let result = orchestrator
        .execute_connector("full_system", params)
        .await?;

    if result.success {
        println!("✅ Log entry created");
    }

    Ok(())
}
```

### Example 3: Run Git Commands

```rust
async fn git_status() -> anyhow::Result<String> {
    let mut params = HashMap::new();
    params.insert("action".to_string(), "execute_command".to_string());
    params.insert("command".to_string(), "git".to_string());
    params.insert("args".to_string(), "status --short".to_string());

    let result = orchestrator
        .execute_connector("full_system", params)
        .await?;

    Ok(result.output)
}

async fn git_commit(message: &str) -> anyhow::Result<()> {
    // Stage all changes
    let mut params = HashMap::new();
    params.insert("action".to_string(), "execute_command".to_string());
    params.insert("command".to_string(), "git".to_string());
    params.insert("args".to_string(), "add .".to_string());
    orchestrator.execute_connector("full_system", params).await?;

    // Commit
    let mut params = HashMap::new();
    params.insert("action".to_string(), "execute_command".to_string());
    params.insert("command".to_string(), "git".to_string());
    params.insert("args".to_string(), format!("commit -m \"{}\"", message));
    
    let result = orchestrator
        .execute_connector("full_system", params)
        .await?;

    if result.success {
        println!("✅ Changes committed");
    }

    Ok(())
}
```

### Example 4: Build Project

```rust
async fn build_project() -> anyhow::Result<()> {
    println!("🔨 Building project...");

    let mut params = HashMap::new();
    params.insert("action".to_string(), "execute_command".to_string());
    params.insert("command".to_string(), "cargo".to_string());
    params.insert("args".to_string(), "build --release".to_string());

    let result = orchestrator
        .execute_connector("full_system", params)
        .await?;

    if result.success {
        println!("✅ Build successful");
        println!("{}", result.output);
    } else {
        eprintln!("❌ Build failed");
        for error in result.errors {
            eprintln!("  {}", error);
        }
    }

    Ok(())
}
```

### Example 5: Directory Backup

```rust
async fn backup_directory(source: &str, dest: &str) -> anyhow::Result<()> {
    // List source directory
    let mut params = HashMap::new();
    params.insert("action".to_string(), "list_directory".to_string());
    params.insert("path".to_string(), source.to_string());

    let result = orchestrator
        .execute_connector("full_system", params)
        .await?;

    let files: Vec<String> = serde_json::from_str(&result.output)?;

    // Copy each file
    for file in files {
        // Read source file
        let mut params = HashMap::new();
        params.insert("action".to_string(), "read_file".to_string());
        params.insert("path".to_string(), file.clone());

        let result = orchestrator
            .execute_connector("full_system", params)
            .await?;

        let content = result.output;

        // Write to destination
        let dest_file = file.replace(source, dest);
        let mut params = HashMap::new();
        params.insert("action".to_string(), "write_file".to_string());
        params.insert("path".to_string(), dest_file);
        params.insert("content".to_string(), content);

        orchestrator
            .execute_connector("full_system", params)
            .await?;
    }

    println!("✅ Backup complete: {} -> {}", source, dest);
    Ok(())
}
```

## Best Practices

### ✅ Do

1. **Use Relative Paths**
   ```rust
   // ✅ Good
   "data/config.json"
   "logs/app.log"
   
   // ❌ Bad
   "/etc/config.json"
   "C:\\Windows\\System32\\config"
   ```

2. **Validate File Operations**
   ```rust
   // Check if file exists before reading
   match read_file("data/config.json").await {
       Ok(content) => process(content),
       Err(e) => eprintln!("File not found: {}", e),
   }
   ```

3. **Handle Command Errors**
   ```rust
   let result = execute_command("git", "status").await?;
   if !result.success {
       eprintln!("Command failed: {:?}", result.errors);
   }
   ```

4. **Log All Operations**
   ```rust
   tracing::info!("Reading file: {}", path);
   tracing::warn!("Executing command: {} {:?}", cmd, args);
   ```

5. **Use Whitelisted Commands**
   ```rust
   // ✅ Allowed
   execute_command("cargo", "build")
   execute_command("git", "status")
   
   // ❌ Not allowed
   execute_command("rm", "-rf /")
   ```

### ❌ Don't

1. **Don't Use Absolute Paths**
   - Always use relative paths from system root
   - Absolute paths are blocked by security controls

2. **Don't Attempt Directory Traversal**
   - `..` in paths is blocked
   - Use proper relative paths instead

3. **Don't Execute Dangerous Commands**
   - Stick to whitelisted commands
   - Never try to bypass command validation

4. **Don't Ignore Security Warnings**
   - Path sanitization errors indicate attack attempts
   - Command validation failures should be investigated

5. **Don't Write to System Directories**
   - Avoid modifying system files
   - Use application-specific directories

## Troubleshooting

### Issue: "Security violation: Absolute paths are not allowed"

**Cause**: Attempted to use absolute path

**Solution**: Use relative paths
```rust
// ❌ Wrong
params.insert("path".to_string(), "/etc/passwd".to_string());

// ✅ Correct
params.insert("path".to_string(), "data/config.json".to_string());
```

### Issue: "Security violation: Parent directory traversal (..) is not allowed"

**Cause**: Path contains `..`

**Solution**: Use proper relative paths
```rust
// ❌ Wrong
params.insert("path".to_string(), "../../../etc/passwd".to_string());

// ✅ Correct
params.insert("path".to_string(), "data/config.json".to_string());
```

### Issue: "Security violation: Path escapes root directory"

**Cause**: Canonicalized path is outside root

**Solution**: Ensure path stays within configured root directory

### Issue: "Security violation: Command 'xyz' is not in the allowed list"

**Cause**: Attempted to execute non-whitelisted command

**Solution**: Use only allowed commands
```rust
// ✅ Allowed commands
"git", "npm", "cargo", "python", "node", "ls", "cat", etc.
```

### Issue: "Security violation: Blocked flag or pattern detected"

**Cause**: Command contains dangerous flags

**Solution**: Remove blocked patterns
```rust
// ❌ Blocked
execute_command("rm", "-rf /")
execute_command("sudo", "apt install")

// ✅ Allowed
execute_command("git", "status")
```

### Issue: "Failed to read file"

**Cause**: File doesn't exist or insufficient permissions

**Solution**:
```rust
// Check file exists first
let mut params = HashMap::new();
params.insert("action".to_string(), "list_directory".to_string());
params.insert("path".to_string(), "data".to_string());
// Verify file is in list before reading
```

## Security Considerations

### Threat Model

**Threats**:
- Directory traversal attacks
- Arbitrary command execution
- Privilege escalation
- Data exfiltration
- System file modification

**Mitigations**:
- Path sanitization (prevents traversal)
- Command whitelist (limits execution)
- Environment isolation (prevents exploitation)
- Audit logging (tracks all operations)
- Blocked patterns (prevents dangerous operations)

### Security Controls

1. **Path Sanitization**
   - Rejects absolute paths
   - Blocks `..` traversal
   - Validates canonical paths
   - Ensures paths stay within root

2. **Command Whitelist**
   - Only approved commands
   - Blocks dangerous flags
   - Validates command structure

3. **Environment Isolation**
   - Cleared environment variables
   - Minimal PATH
   - No inherited credentials

4. **Audit Logging**
   ```rust
   tracing::info!("File read: {}", path);
   tracing::warn!("Executing command: {} {:?}", cmd, args);
   ```

5. **Atomic Operations**
   - File writes are atomic
   - Prevents partial writes
   - Ensures consistency

### Security Checklist

- [ ] Path sanitization enabled
- [ ] Command whitelist enforced
- [ ] Blocked patterns configured
- [ ] Audit logging enabled and monitored
- [ ] Root directory properly configured
- [ ] File permissions reviewed
- [ ] Command execution tested
- [ ] Security controls validated

## Related Documentation

- [AI Agent Overview](README.md) - Overview of all agent capabilities
- [Security Best Practices](security-best-practices.md) - Security guidelines
- [Self-Improvement](self-improvement.md) - Code modification capabilities
- [Admin Assistant](admin-assistant.md) - System administration

## API Reference

### Actions

#### `read_file`
Read the contents of a file.

**Parameters**:
- `action`: `"read_file"`
- `path`: Relative file path

**Returns**: File content as string

#### `write_file`
Create or overwrite a file.

**Parameters**:
- `action`: `"write_file"`
- `path`: Relative file path
- `content`: File content

**Returns**: Success message

#### `list_directory`
List directory contents.

**Parameters**:
- `action`: `"list_directory"`
- `path`: Relative directory path (default: `"."`)

**Returns**: Array of file/directory paths

#### `execute_command`
Execute a whitelisted command.

**Parameters**:
- `action`: `"execute_command"`
- `command`: Command name
- `args`: Command arguments (space-separated)

**Returns**: Command output (stdout)

---

**Last Updated**: 2025-11-17  
**Version**: 1.0.0  
**Status**: ✅ Complete