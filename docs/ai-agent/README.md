# AI Agent Capabilities - Overview

**Jamey 2.0** includes powerful AI agent capabilities that provide autonomous system access, self-improvement, and multi-agent orchestration. This documentation helps you understand and safely use these advanced features.

> ⚠️ **Important**: These capabilities grant significant system access. Always review the [Security Best Practices](security-best-practices.md) before enabling agent features in production.

## Quick Navigation

- [Self-Improvement](self-improvement.md) - Code modification with automatic backups
- [Admin Assistant](admin-assistant.md) - System administration and process management
- [Full System Access](full-system-access.md) - File system and command execution
- [Network & Web Access](network-access.md) - Web search, downloads, and URL fetching
- [IoT Device Connectivity](iot-devices.md) - Connect to smart home devices and sensors
- [Agent Orchestration](orchestration.md) - Multi-agent coordination
- [24/7 Service Mode](always-on.md) - Continuous operation with scheduling
- [Security Best Practices](security-best-practices.md) - Security guidelines and controls

## Overview

Jamey 2.0's AI agent system consists of seven core connectors, each providing specific capabilities with built-in security controls:

### 1. Self-Improvement Connector
**Capability Level**: `SelfModify` | **Requires Approval**: ✅ Yes

Enables the AI to read and modify its own source code with automatic backup and rollback capabilities.

**Key Features**:
- Automatic timestamped backups before modifications
- Source file validation (`.rs`, `.toml`, `.md`)
- Rollback to previous versions
- Configurable backup retention

**Use Cases**:
- Bug fixes and improvements
- Feature additions
- Configuration updates
- Documentation updates

[→ Full Documentation](self-improvement.md)

### 2. System Admin Connector
**Capability Level**: `SystemAdmin` | **Requires Approval**: ✅ Yes

Provides system administration capabilities including process management and Windows Registry access.

**Key Features**:
- Process listing and monitoring
- Process termination (with protected process list)
- Windows Registry read access (Windows only)
- System resource monitoring

**Use Cases**:
- Process management
- System monitoring
- Configuration retrieval
- Resource optimization

[→ Full Documentation](admin-assistant.md)

### 3. Full System Access Connector
**Capability Level**: `FullAccess` | **Requires Approval**: ❌ No

Complete file system access and command execution with security guardrails.

**Key Features**:
- File read/write operations
- Directory listing
- Command execution (whitelisted commands only)
- Path sanitization (prevents traversal attacks)

**Use Cases**:
- File management
- Build automation
- System configuration
- Development workflows

[→ Full Documentation](full-system-access.md)

### 4. Network & Web Access Connector
**Capability Level**: `WebAccess` | **Requires Approval**: ❌ No

Web search, file downloads, and URL fetching with SSRF protection.

**Key Features**:
- Web search via DuckDuckGo
- File downloads with size limits
- URL content fetching
- SSRF protection (blocks private IPs)

**Use Cases**:
- Information gathering
- Documentation retrieval
- Resource downloads
- API integration

[→ Full Documentation](network-access.md)

### 5. Agent Orchestration Connector
**Capability Level**: `AgentOrchestration` | **Requires Approval**: ❌ No

Coordinate tasks across multiple AI agents with secure communication.

**Key Features**:
- Agent registration and authentication
- Task delegation
- Broadcast operations
- HTTPS-only communication (TLS 1.2+)

**Use Cases**:
- Distributed task execution
- Multi-agent workflows
- Load distribution
- Specialized agent coordination

[→ Full Documentation](orchestration.md)

### 6. IoT Device Connector
**Capability Level**: `NetworkAccess` | **Requires Approval**: ✅ Yes

Secure connectivity to IoT devices via MQTT, HTTP/REST, and other protocols.

**Key Features**:
- Multi-protocol support (MQTT, HTTP, CoAP, WebSocket)
- Encrypted credential storage
- mTLS support for MQTT
- Device discovery via mDNS
- Real-time communication

**Use Cases**:
- Smart home automation
- Sensor monitoring
- IoT hub integration
- Device management

[→ Full Documentation](iot-devices.md)

### 7. 24/7 Service Mode
**Capability Level**: N/A | **Requires Approval**: ❌ No

Run Jamey 2.0 continuously with task scheduling and health monitoring.

**Key Features**:
- Task scheduler integration
- Health monitoring
- Graceful shutdown
- Resource management

**Use Cases**:
- Continuous monitoring
- Scheduled maintenance
- Automated workflows
- Always-on assistance

[→ Full Documentation](always-on.md)

## Security Architecture

All agent capabilities include multiple layers of security:

### 1. **Approval Workflows**
High-risk operations (self-modification, process termination) require explicit confirmation:

```rust
// Example: Self-modification requires confirmation
params.insert("confirmed", "true");
```

### 2. **Path Sanitization**
All file paths are validated to prevent directory traversal:

```rust
// Blocks: "../../../etc/passwd"
// Allows: "data/config.json"
```

### 3. **Command Whitelisting**
Only approved commands can be executed:

```rust
// Allowed: git, npm, cargo, python, node
// Blocked: rm -rf /, sudo, format
```

### 4. **SSRF Protection**
Network requests block private IPs and cloud metadata endpoints:

```rust
// Blocked: 127.0.0.1, 192.168.x.x, 169.254.169.254
// Allowed: Public internet URLs
```

### 5. **Protected Processes**
Critical system processes cannot be terminated:

```rust
// Protected: System, csrss.exe, postgres, redis-server
```

### 6. **TLS Enforcement**
Agent-to-agent communication requires TLS 1.2+ with certificate validation.

### 7. **Audit Logging**
All operations are logged with structured tracing for security audits.

[→ Complete Security Guide](security-best-practices.md)

## Quick Start

### 1. Configuration

Add to your `.env` file:

```bash
# Enable agent features
ENABLE_REGISTRY_TOOL=false  # Windows Registry (Windows only)
BACKUP_DIR=./backups        # Self-improvement backups
DOWNLOAD_DIR=./downloads    # Network downloads
SYSTEM_ROOT=C:\             # Windows: C:\ | Linux: /

# 24/7 Mode (optional)
ENABLE_24_7=false
SCHEDULER_ENABLED=false

# Optional API keys
GITHUB_TOKEN=your_token_here
WEB_SEARCH_API_KEY=your_key_here
```

### 2. Initialize Runtime

```rust
use jamey_runtime::config::RuntimeConfig;
use jamey_runtime::state::RuntimeState;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load configuration
    let config = RuntimeConfig::from_env()?;
    
    // Initialize runtime state
    let state = Arc::new(RuntimeState::new(config).await?);
    
    // Connectors are automatically registered
    println!("AI Agent capabilities enabled");
    
    Ok(())
}
```

### 3. Execute Connector

```rust
use std::collections::HashMap;

// Example: Read a file
let mut params = HashMap::new();
params.insert("action".to_string(), "read_file".to_string());
params.insert("path".to_string(), "README.md".to_string());

let result = state.hybrid_orchestrator
    .lock()
    .await
    .execute_connector("full_system", params)
    .await?;

println!("File content: {}", result.output);
```

## Capability Levels

Connectors are organized by capability level (from least to most privileged):

| Level | Description | Approval Required | Examples |
|-------|-------------|-------------------|----------|
| `WebAccess` | Network and web operations | No | Web search, downloads |
| `FullAccess` | File system and commands | No | File I/O, whitelisted commands |
| `SystemAdmin` | System administration | Yes | Process management, registry |
| `SelfModify` | Code modification | Yes | Source code changes |
| `AgentOrchestration` | Multi-agent coordination | No | Task delegation |

## Best Practices

### ✅ Do

- **Review logs regularly** - Monitor all agent operations
- **Use approval workflows** - Enable confirmation for dangerous operations
- **Test in development** - Validate agent behavior before production
- **Limit scope** - Only enable needed capabilities
- **Backup regularly** - Maintain system backups independent of agent backups
- **Monitor resources** - Track CPU, memory, and network usage
- **Rotate credentials** - Regularly update API keys and tokens

### ❌ Don't

- **Disable security controls** - Keep path sanitization and command whitelisting enabled
- **Run as root/admin** - Use least-privilege accounts
- **Expose to internet** - Keep agent APIs behind authentication
- **Skip testing** - Always test self-modifications in safe environments
- **Ignore warnings** - Pay attention to security warnings in logs
- **Share credentials** - Keep API keys and tokens secure

## Troubleshooting

### Common Issues

**Issue**: "Security violation: Path escapes root directory"
- **Cause**: Attempted directory traversal
- **Solution**: Use relative paths within allowed directories

**Issue**: "Command not in allowed list"
- **Cause**: Attempted to execute blocked command
- **Solution**: Use whitelisted commands or add to allowed list

**Issue**: "Process kill requires confirmation"
- **Cause**: Missing confirmation parameter
- **Solution**: Add `confirmed: true` to parameters

**Issue**: "Security violation: Cannot terminate protected process"
- **Cause**: Attempted to kill critical system process
- **Solution**: Protected processes cannot be terminated

**Issue**: "Agent returned error status: 401"
- **Cause**: Missing or invalid API key for agent orchestration
- **Solution**: Verify API key configuration

### Getting Help

- **Documentation**: Review specific connector documentation
- **Logs**: Check `tracing` output for detailed error messages
- **Security**: See [Security Best Practices](security-best-practices.md)
- **Issues**: Report bugs on GitHub with sanitized logs

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                     Jamey 2.0 Runtime                       │
├─────────────────────────────────────────────────────────────┤
│                   Hybrid Orchestrator                       │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              Connector Registry                       │  │
│  └───────────────────────────────────────────────────────┘  │
│                           │                                 │
│  ┌────────────────────────┴────────────────────────────┐   │
│  │                                                      │   │
│  ▼                  ▼                  ▼                ▼   │
│ ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │
│ │  Self    │  │  System  │  │   Full   │  │ Network  │   │
│ │ Improve  │  │  Admin   │  │  System  │  │   Web    │   │
│ └──────────┘  └──────────┘  └──────────┘  └──────────┘   │
│                                                              │
│ ┌──────────┐  ┌──────────┐  ┌──────────┐                   │
│ │   IoT   │  │  Agent   │  │   MCP    │                   │
│ │ Devices │  │  Orchest │  │ Protocol │                   │
│ └──────────┘  └──────────┘  └──────────┘                   │
│      │              │              │              │        │
│      ▼              ▼              ▼              ▼        │
│ ┌──────────────────────────────────────────────────────┐  │
│ │           Security Layer                             │  │
│ │  • Approval Workflows  • Path Sanitization           │  │
│ │  • Command Whitelist   • SSRF Protection             │  │
│ │  • Protected Processes • TLS Enforcement             │  │
│ │  • Audit Logging       • Rate Limiting               │  │
│ └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Related Documentation

- [System Architecture](../architecture/system-overview.md) - Overall system design
- [Security Overview](../security/README.md) - Security principles
- [Configuration Guide](../architecture/configuration.md) - Configuration system
- [TA-QR Cryptography](../security/ta-qr/README.md) - Quantum-resistant crypto

## Version History

- **v1.0.0** (2025-11-17) - Initial AI agent capabilities
  - Self-improvement with automatic backups
  - System administration features
  - Full system access with security controls
  - Network and web access with SSRF protection
  - IoT device connectivity with mTLS support
  - Agent orchestration with TLS enforcement
  - 24/7 service mode with scheduling

---

**Last Updated**: 2025-11-17  
**Version**: 1.0.0  
**Status**: ✅ Complete  
**Maintained by**: Jamey Code Team