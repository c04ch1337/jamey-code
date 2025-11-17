# Admin Assistant Guide

The System Admin connector provides system administration capabilities including process management, system monitoring, and Windows Registry access. This enables Jamey 2.0 to manage system resources and perform administrative tasks autonomously.

> ⚠️ **Warning**: System administration operations can affect system stability. Always use with caution and proper approval workflows.

## Table of Contents

- [Overview](#overview)
- [Configuration](#configuration)
- [Process Management](#process-management)
- [Windows Registry Access](#windows-registry-access)
- [Usage Examples](#usage-examples)
- [Safety Controls](#safety-controls)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)
- [Security Considerations](#security-considerations)

## Overview

**Connector ID**: `system_admin`  
**Capability Level**: `SystemAdmin`  
**Requires Approval**: ✅ Yes  
**Source**: [`jamey-tools/src/connectors/system_admin.rs`](../../jamey-tools/src/connectors/system_admin.rs)

### Key Features

- ✅ **Process Management**: List, monitor, and terminate processes
- ✅ **Protected Processes**: Critical system processes cannot be terminated
- ✅ **Windows Registry**: Read-only access to Windows Registry (Windows only)
- ✅ **System Monitoring**: CPU and memory usage tracking
- ✅ **Approval Required**: Destructive operations require confirmation

### Capabilities

| Feature | Description | Approval Required |
|---------|-------------|-------------------|
| List Processes | View all running processes | No |
| Get Process Info | Detailed information about a process | No |
| Kill Process | Terminate a process | Yes |
| Read Registry | Read Windows Registry values | No |

## Configuration

### Environment Variables

Add to your `.env` file:

```bash
# Enable process management tools
PROCESS_TOOL_ENABLED=true

# Maximum number of processes to list (default: 100)
PROCESS_TOOL_MAX_LIST=100

# Enable Windows Registry access (Windows only)
ENABLE_REGISTRY_TOOL=false  # Set to true if needed
```

### Runtime Configuration

```rust
use jamey_runtime::config::RuntimeConfig;

let config = RuntimeConfig::from_env()?;

// Process tool configuration
println!("Process tool enabled: {}", config.tools.process_tool_enabled);
println!("Max processes: {}", config.tools.process_tool_max_list);

// Windows-specific
#[cfg(windows)]
println!("Registry tool enabled: {}", config.tools.enable_registry_tool);
```

### Initialization

```rust
use jamey_tools::connectors::SystemAdminConnector;

// Create connector (no configuration needed)
let connector = SystemAdminConnector::new();
```

## Process Management

### List All Processes

View all running processes with CPU and memory usage:

```rust
use std::collections::HashMap;

let mut params = HashMap::new();
params.insert("action".to_string(), "list_processes".to_string());

let result = orchestrator
    .execute_connector("system_admin", params)
    .await?;

let processes: Vec<ProcessInfo> = serde_json::from_str(&result.output)?;
println!("Found {} processes", processes.len());

for process in processes {
    println!("PID: {} | Name: {} | CPU: {:.1}% | Memory: {} KB",
        process.pid,
        process.name,
        process.cpu_usage,
        process.memory_usage / 1024
    );
}
```

### Get Process Information

Retrieve detailed information about a specific process:

```rust
use std::collections::HashMap;

let mut params = HashMap::new();
params.insert("action".to_string(), "get_process_info".to_string());
params.insert("pid".to_string(), "1234".to_string());

let result = orchestrator
    .execute_connector("system_admin", params)
    .await?;

let info: ProcessInfo = serde_json::from_str(&result.output)?;
println!("Process: {}", info.name);
println!("PID: {}", info.pid);
println!("CPU Usage: {:.2}%", info.cpu_usage);
println!("Memory: {} MB", info.memory_usage / 1024 / 1024);
```

### Terminate Process

Kill a process (requires approval):

```rust
use std::collections::HashMap;

let mut params = HashMap::new();
params.insert("action".to_string(), "kill_process".to_string());
params.insert("pid".to_string(), "1234".to_string());
params.insert("confirmed".to_string(), "true".to_string()); // Required!

let result = orchestrator
    .execute_connector("system_admin", params)
    .await?;

if result.success {
    println!("✅ Process terminated successfully");
} else {
    eprintln!("❌ Failed to terminate process: {:?}", result.errors);
}
```

### Process Information Structure

```rust
pub struct ProcessInfo {
    pub pid: u32,                    // Process ID
    pub name: String,                // Process name
    pub cpu_usage: f32,              // CPU usage percentage
    pub memory_usage: u64,           // Memory usage in bytes
    pub start_time: DateTime<Utc>,  // Process start time
}
```

## Windows Registry Access

> 📝 **Note**: Registry access is only available on Windows systems.

### Read Registry Value

```rust
#[cfg(windows)]
{
    use std::collections::HashMap;

    let mut params = HashMap::new();
    params.insert("action".to_string(), "read_registry".to_string());
    params.insert("key".to_string(), 
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion".to_string());
    params.insert("value".to_string(), "ProductName".to_string());

    let result = orchestrator
        .execute_connector("system_admin", params)
        .await?;

    println!("Windows Version: {}", result.output);
}
```

### Common Registry Paths

```rust
// System information
r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"
// Values: ProductName, CurrentVersion, SystemRoot

// Network configuration
r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters"
// Values: Hostname, Domain

// User environment
r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment"
// Values: PATH, TEMP, TMP
```

### Registry Safety

- ✅ **Read-Only**: Cannot modify registry values
- ✅ **No Dangerous Keys**: Access to sensitive keys is logged
- ✅ **Error Handling**: Invalid keys return errors, not crashes

## Usage Examples

### Example 1: Monitor High CPU Processes

```rust
use std::collections::HashMap;

async fn monitor_high_cpu(threshold: f32) -> anyhow::Result<()> {
    let mut params = HashMap::new();
    params.insert("action".to_string(), "list_processes".to_string());

    let result = orchestrator
        .execute_connector("system_admin", params)
        .await?;

    let processes: Vec<ProcessInfo> = serde_json::from_str(&result.output)?;
    
    let high_cpu: Vec<_> = processes.into_iter()
        .filter(|p| p.cpu_usage > threshold)
        .collect();

    if !high_cpu.is_empty() {
        println!("⚠️ High CPU processes detected:");
        for process in high_cpu {
            println!("  {} (PID: {}) - {:.1}% CPU",
                process.name, process.pid, process.cpu_usage);
        }
    }

    Ok(())
}
```

### Example 2: Find Process by Name

```rust
async fn find_process_by_name(name: &str) -> anyhow::Result<Option<ProcessInfo>> {
    let mut params = HashMap::new();
    params.insert("action".to_string(), "list_processes".to_string());

    let result = orchestrator
        .execute_connector("system_admin", params)
        .await?;

    let processes: Vec<ProcessInfo> = serde_json::from_str(&result.output)?;
    
    Ok(processes.into_iter()
        .find(|p| p.name.to_lowercase().contains(&name.to_lowercase())))
}
```

### Example 3: Memory Usage Report

```rust
async fn memory_usage_report() -> anyhow::Result<()> {
    let mut params = HashMap::new();
    params.insert("action".to_string(), "list_processes".to_string());

    let result = orchestrator
        .execute_connector("system_admin", params)
        .await?;

    let mut processes: Vec<ProcessInfo> = serde_json::from_str(&result.output)?;
    
    // Sort by memory usage (descending)
    processes.sort_by(|a, b| b.memory_usage.cmp(&a.memory_usage));

    println!("Top 10 Memory Consumers:");
    for (i, process) in processes.iter().take(10).enumerate() {
        println!("{}. {} - {} MB",
            i + 1,
            process.name,
            process.memory_usage / 1024 / 1024
        );
    }

    Ok(())
}
```

### Example 4: Safe Process Termination

```rust
async fn safe_kill_process(pid: u32) -> anyhow::Result<()> {
    // First, get process info
    let mut params = HashMap::new();
    params.insert("action".to_string(), "get_process_info".to_string());
    params.insert("pid".to_string(), pid.to_string());

    let result = orchestrator
        .execute_connector("system_admin", params)
        .await?;

    let info: ProcessInfo = serde_json::from_str(&result.output)?;
    
    // Confirm with user
    println!("About to terminate:");
    println!("  Name: {}", info.name);
    println!("  PID: {}", info.pid);
    print!("Proceed? (yes/no): ");
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    
    if input.trim().to_lowercase() == "yes" {
        // Terminate with approval
        let mut params = HashMap::new();
        params.insert("action".to_string(), "kill_process".to_string());
        params.insert("pid".to_string(), pid.to_string());
        params.insert("confirmed".to_string(), "true".to_string());

        let result = orchestrator
            .execute_connector("system_admin", params)
            .await?;

        if result.success {
            println!("✅ Process terminated");
        }
    } else {
        println!("❌ Termination cancelled");
    }

    Ok(())
}
```

## Safety Controls

### Protected Process List

The following processes **cannot be terminated** to prevent system instability:

#### Windows Critical Processes
- `System` - Windows kernel
- `csrss.exe` - Client/Server Runtime
- `wininit.exe` - Windows initialization
- `services.exe` - Service Control Manager
- `lsass.exe` - Local Security Authority
- `winlogon.exe` - Windows Logon
- `smss.exe` - Session Manager
- `svchost.exe` - Service Host
- `explorer.exe` - Windows Explorer

#### Linux/Unix Critical Processes
- `init` - System initialization
- `systemd` - System and service manager
- `launchd` - macOS service manager
- `kernel` - Kernel threads
- `kthreadd` - Kernel thread daemon

#### Database Processes
- `postgres` - PostgreSQL
- `mysqld` - MySQL
- `mongod` - MongoDB
- `redis-server` - Redis

#### Security Processes
- `antivirus` - Antivirus software
- `defender` - Windows Defender
- `firewall` - Firewall services

### Protection Mechanism

```rust
fn is_protected_process(process_name: &str) -> bool {
    let lower_name = process_name.to_lowercase();
    
    PROTECTED_PROCESS_NAMES.iter().any(|protected| {
        lower_name.contains(&protected.to_lowercase())
    })
}
```

If you attempt to kill a protected process:

```
Error: Security violation: Cannot terminate protected process 'csrss.exe' (PID: 456).
This is a critical system process.
```

## Best Practices

### ✅ Do

1. **Check Process Info Before Termination**
   ```rust
   // Always verify what you're killing
   let info = get_process_info(pid).await?;
   println!("About to kill: {}", info.name);
   ```

2. **Monitor System Resources**
   ```rust
   // Regular monitoring prevents issues
   tokio::spawn(async {
       loop {
           monitor_high_cpu(80.0).await.ok();
           tokio::time::sleep(Duration::from_secs(60)).await;
       }
   });
   ```

3. **Use Approval Workflows**
   ```rust
   // Always require confirmation for kills
   params.insert("confirmed".to_string(), "true".to_string());
   ```

4. **Log All Operations**
   ```rust
   tracing::warn!("Terminating process: {} (PID: {})", name, pid);
   ```

5. **Handle Errors Gracefully**
   ```rust
   match kill_process(pid).await {
       Ok(_) => println!("Process terminated"),
       Err(e) => eprintln!("Failed: {}", e),
   }
   ```

### ❌ Don't

1. **Don't Kill Processes Without Verification**
   - Always check process info first
   - Verify it's not a critical process

2. **Don't Bypass Protection Mechanisms**
   - Protected process list exists for safety
   - Never modify the protection list without review

3. **Don't Auto-Approve Terminations**
   - Always require manual confirmation
   - Never set `confirmed=true` automatically

4. **Don't Ignore Error Messages**
   - "Process not found" may indicate race condition
   - "Access denied" may indicate insufficient permissions

5. **Don't Terminate Database Processes**
   - Can cause data corruption
   - Use proper shutdown procedures instead

## Troubleshooting

### Issue: "Process kill requires confirmation"

**Cause**: Missing `confirmed` parameter

**Solution**:
```rust
params.insert("confirmed".to_string(), "true".to_string());
```

### Issue: "Security violation: Cannot terminate protected process"

**Cause**: Attempted to kill a critical system process

**Solution**: Protected processes cannot be terminated. This is by design for system safety.

### Issue: "Process not found: 1234"

**Cause**: Process doesn't exist or already terminated

**Solution**:
```rust
// Check if process exists first
match get_process_info(pid).await {
    Ok(info) => kill_process(pid).await?,
    Err(_) => println!("Process no longer exists"),
}
```

### Issue: "Registry error: Failed to open registry key"

**Cause**: Invalid registry path or insufficient permissions

**Solution**:
```rust
// Use valid registry paths
let key = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
// Not: r"INVALID\PATH"
```

### Issue: High CPU usage after listing processes

**Cause**: Listing processes too frequently

**Solution**:
```rust
// Add delay between listings
tokio::time::sleep(Duration::from_secs(5)).await;
```

## Security Considerations

### Threat Model

**Threats**:
- Denial of Service (killing critical processes)
- System instability
- Data loss (terminating database processes)
- Privilege escalation attempts

**Mitigations**:
- Protected process list (prevents critical process termination)
- Approval workflow (requires confirmation)
- Read-only registry access (cannot modify system configuration)
- Audit logging (tracks all operations)

### Security Controls

1. **Protected Process List**
   - Hardcoded list of critical processes
   - Cannot be bypassed or modified at runtime
   - Prevents accidental system crashes

2. **Approval Requirement**
   - All destructive operations require `confirmed=true`
   - Prevents automated attacks
   - Enables human oversight

3. **Read-Only Registry**
   - Cannot modify registry values
   - Limits attack surface
   - Prevents system configuration changes

4. **Audit Logging**
   ```rust
   tracing::warn!("Terminating process: {} (PID: {})", name, pid);
   ```

5. **Error Handling**
   - Graceful failure on errors
   - No information leakage in error messages
   - Proper permission checks

### Security Checklist

- [ ] Approval workflow enabled for process termination
- [ ] Protected process list reviewed and up-to-date
- [ ] Audit logging enabled and monitored
- [ ] Registry access limited to read-only
- [ ] Error handling tested
- [ ] Monitoring alerts configured
- [ ] Access controls on admin connector
- [ ] Regular security audits performed

## Platform-Specific Notes

### Windows

```rust
#[cfg(windows)]
{
    // Registry access available
    let registry_enabled = config.tools.enable_registry_tool;
    
    // Protected processes include Windows-specific services
    // csrss.exe, lsass.exe, services.exe, etc.
}
```

### Linux/Unix

```rust
#[cfg(unix)]
{
    // No registry access
    // Protected processes include init, systemd, etc.
    
    // May require elevated permissions for some operations
    // Consider using sudo or capabilities
}
```

### macOS

```rust
#[cfg(target_os = "macos")]
{
    // Protected processes include launchd
    // System Integrity Protection (SIP) may block some operations
}
```

## Related Documentation

- [AI Agent Overview](README.md) - Overview of all agent capabilities
- [Security Best Practices](security-best-practices.md) - Security guidelines
- [Full System Access](full-system-access.md) - File system and command execution
- [Self-Improvement](self-improvement.md) - Code modification capabilities

## API Reference

### Actions

#### `list_processes`
List all running processes.

**Parameters**:
- `action`: `"list_processes"`

**Returns**: Array of `ProcessInfo` objects

#### `get_process_info`
Get detailed information about a specific process.

**Parameters**:
- `action`: `"get_process_info"`
- `pid`: Process ID (string)

**Returns**: `ProcessInfo` object

#### `kill_process`
Terminate a process (requires approval).

**Parameters**:
- `action`: `"kill_process"`
- `pid`: Process ID (string)
- `confirmed`: `"true"` (required)

**Returns**: Success message

#### `read_registry` (Windows only)
Read a Windows Registry value.

**Parameters**:
- `action`: `"read_registry"`
- `key`: Registry key path
- `value`: Value name

**Returns**: Registry value as string

---

**Last Updated**: 2025-11-17  
**Version**: 1.0.0  
**Status**: ✅ Complete