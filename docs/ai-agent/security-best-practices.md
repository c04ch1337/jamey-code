# Security Best Practices

This document provides comprehensive security guidelines for deploying and operating Jamey 2.0's AI agent capabilities. Following these practices ensures safe operation while maintaining the powerful autonomous features.

> 🔒 **Critical**: Security is paramount when granting AI agents system access. Review and implement all recommendations before production deployment.

## Table of Contents

- [Security Overview](#security-overview)
- [Defense in Depth](#defense-in-depth)
- [Configuration Security](#configuration-security)
- [Approval Workflows](#approval-workflows)
- [Audit Logging](#audit-logging)
- [Network Security](#network-security)
- [Access Control](#access-control)
- [Incident Response](#incident-response)
- [Security Checklist](#security-checklist)
- [Compliance Considerations](#compliance-considerations)

## Security Overview

### Security Principles

Jamey 2.0's security architecture is built on these core principles:

1. **Defense in Depth**: Multiple layers of security controls
2. **Least Privilege**: Minimal permissions required for operation
3. **Fail Secure**: Errors default to secure state
4. **Audit Everything**: Comprehensive logging of all operations
5. **Zero Trust**: Verify all operations, trust nothing

### Threat Model

**Primary Threats**:
- Unauthorized system access
- Code injection attacks
- Data exfiltration
- Privilege escalation
- Denial of service
- Supply chain attacks

**Attack Vectors**:
- Malicious prompts
- Compromised dependencies
- Network-based attacks
- Insider threats
- Configuration errors

## Defense in Depth

### Layer 1: Input Validation

All inputs are validated before processing:

```rust
// Path sanitization
fn sanitize_path(root: &Path, user_path: &str) -> Result<PathBuf> {
    // Reject absolute paths
    if Path::new(user_path).is_absolute() {
        bail!("Absolute paths not allowed");
    }
    
    // Block directory traversal
    if user_path.contains("..") {
        bail!("Directory traversal not allowed");
    }
    
    // Validate canonical path
    let canonical = root.join(user_path).canonicalize()?;
    if !canonical.starts_with(root.canonicalize()?) {
        bail!("Path escapes root directory");
    }
    
    Ok(canonical)
}
```

### Layer 2: Command Whitelisting

Only approved commands can execute:

```rust
const ALLOWED_COMMANDS: &[&str] = &[
    "ls", "dir", "cat", "type", "echo", "pwd",
    "git", "npm", "cargo", "python", "node",
    "grep", "find", "which", "where", "whoami",
];

const BLOCKED_FLAGS: &[&str] = &[
    "--privileged", "--cap-add", "sudo", "su",
    "rm -rf /", "format", "mkfs",
];
```

### Layer 3: SSRF Protection

Network requests are validated:

```rust
fn validate_url(url: &str) -> Result<()> {
    let parsed = url::Url::parse(url)?;
    
    // Only HTTP/HTTPS
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        bail!("Only HTTP/HTTPS allowed");
    }
    
    // Block private IPs
    if is_private_ip(&host) {
        bail!("Private IP access not allowed");
    }
    
    // Block cloud metadata
    if is_cloud_metadata(&host) {
        bail!("Cloud metadata access not allowed");
    }
    
    Ok(())
}
```

### Layer 4: Protected Resources

Critical resources cannot be modified:

```rust
const PROTECTED_PROCESSES: &[&str] = &[
    "System", "csrss.exe", "lsass.exe", "postgres",
    "init", "systemd", "redis-server",
];

fn is_protected_process(name: &str) -> bool {
    PROTECTED_PROCESSES.iter()
        .any(|p| name.to_lowercase().contains(&p.to_lowercase()))
}
```

### Layer 5: Approval Workflows

High-risk operations require confirmation:

```rust
// Self-modification requires approval
if !params.contains_key("confirmed") {
    return Err(anyhow!("Self-modification requires confirmation"));
}

// Process termination requires approval
if !params.contains_key("confirmed") {
    return Err(anyhow!("Process kill requires confirmation"));
}
```

### Layer 6: Audit Logging

All operations are logged:

```rust
tracing::info!("File read: {}", path);
tracing::warn!("Executing command: {} {:?}", cmd, args);
tracing::error!("Security violation: {}", error);
```

## Configuration Security

### Environment Variables

**Secure Configuration**:

```bash
# ✅ Use strong, unique passwords
POSTGRES_PASSWORD=$(openssl rand -base64 32)

# ✅ Use environment-specific API keys
OPENROUTER_API_KEY=sk_live_production_key_here

# ✅ Require API authentication
API_KEY_REQUIRED=true
API_KEY=$(openssl rand -base64 32)

# ✅ Disable dangerous features in production
ENABLE_REGISTRY_TOOL=false  # Windows Registry access
ENABLE_24_7=true            # Enable for production
SCHEDULER_ENABLED=true      # Enable scheduling
```

**Insecure Configuration** (❌ Don't do this):

```bash
# ❌ Weak passwords
POSTGRES_PASSWORD=password123

# ❌ Development keys in production
OPENROUTER_API_KEY=sk_test_dev_key

# ❌ No authentication
API_KEY_REQUIRED=false

# ❌ Unnecessary features enabled
ENABLE_REGISTRY_TOOL=true
```

### File Permissions

```bash
# Secure .env file
chmod 600 .env
chown jamey:jamey .env

# Secure backup directory
chmod 700 backups/
chown jamey:jamey backups/

# Secure log directory
chmod 750 logs/
chown jamey:jamey logs/
```

### Secret Management

**Use Secret Manager**:

```rust
use jamey_core::prelude::SecretManager;

let secret_manager = SecretManager::new("jamey_runtime");

// Store secrets securely
secret_manager.store_secret("postgres_password", &password)?;
secret_manager.store_secret("api_key", &api_key)?;

// Retrieve when needed
let password = secret_manager.get_secret("postgres_password")?;
```

**Never**:
- Hardcode secrets in source code
- Commit secrets to version control
- Log secrets in plain text
- Share secrets via insecure channels

## Approval Workflows

### Implementing Approval

```rust
async fn approve_operation(
    operation: &str,
    details: &str,
) -> bool {
    println!("⚠️  Approval Required");
    println!("Operation: {}", operation);
    println!("Details: {}", details);
    print!("Approve? (yes/no): ");
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    
    input.trim().to_lowercase() == "yes"
}

// Use in connector
if !params.contains_key("confirmed") {
    if approve_operation("self_modify", &file_path) {
        params.insert("confirmed".to_string(), "true".to_string());
    } else {
        return Err(anyhow!("Operation cancelled by user"));
    }
}
```

### Approval Policies

**High-Risk Operations** (Always require approval):
- Self-modification
- Process termination
- System configuration changes
- Credential access

**Medium-Risk Operations** (Consider approval):
- File writes outside designated directories
- Network requests to new domains
- Large file downloads

**Low-Risk Operations** (No approval needed):
- File reads
- Directory listings
- Web searches
- Process listings

## Audit Logging

### Structured Logging

```rust
use tracing::{info, warn, error};

// Information logging
info!(
    operation = "file_read",
    path = %file_path,
    user = %user_id,
    "File accessed"
);

// Warning logging
warn!(
    operation = "command_execute",
    command = %cmd,
    args = ?args,
    "Command executed"
);

// Error logging
error!(
    operation = "security_violation",
    violation_type = "path_traversal",
    attempted_path = %path,
    "Security violation detected"
);
```

### Log Retention

```bash
# /etc/logrotate.d/jamey
/var/log/jamey/*.log {
    daily
    rotate 90        # Keep 90 days
    compress
    delaycompress
    notifempty
    create 0640 jamey jamey
    sharedscripts
    postrotate
        systemctl reload jamey
    endscript
}
```

### Log Monitoring

```bash
# Monitor for security violations
tail -f /var/log/jamey/security.log | grep "violation"

# Alert on suspicious activity
journalctl -u jamey -f | grep -E "(WARN|ERROR)" | \
    while read line; do
        echo "$line" | mail -s "Jamey Alert" admin@example.com
    done
```

### Log Analysis

```bash
# Count operations by type
jq -r '.operation' /var/log/jamey/audit.json | sort | uniq -c

# Find failed operations
jq 'select(.success == false)' /var/log/jamey/audit.json

# Security violations
jq 'select(.level == "ERROR" and .message | contains("violation"))' \
    /var/log/jamey/audit.json
```

## Network Security

### TLS Configuration

```rust
// Enforce TLS 1.2+
let client = ClientBuilder::new()
    .min_tls_version(reqwest::tls::Version::TLS_1_2)
    .danger_accept_invalid_certs(false)
    .build()?;
```

### Firewall Rules

```bash
# Allow only necessary ports
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow 3443/tcp  # HTTPS API
sudo ufw allow 5432/tcp  # PostgreSQL (if remote)
sudo ufw enable
```

### Network Isolation

```yaml
# docker-compose.yml
version: '3.8'
services:
  jamey:
    networks:
      - internal
    ports:
      - "3443:3443"
  
  postgres:
    networks:
      - internal
    # No external ports

networks:
  internal:
    driver: bridge
    internal: true
```

## Access Control

### User Permissions

```bash
# Create dedicated user
sudo useradd -r -s /bin/false jamey

# Set ownership
sudo chown -R jamey:jamey /opt/jamey

# Restrict permissions
sudo chmod 750 /opt/jamey
sudo chmod 600 /opt/jamey/.env
```

### File System Permissions

```bash
# Application files (read-only)
chmod 755 /opt/jamey/jamey-runtime
chmod 644 /opt/jamey/config/*

# Data directories (read-write)
chmod 700 /opt/jamey/backups
chmod 700 /opt/jamey/downloads
chmod 750 /opt/jamey/logs

# Configuration (read-only, sensitive)
chmod 600 /opt/jamey/.env
```

### Database Permissions

```sql
-- Create dedicated database user
CREATE USER jamey WITH PASSWORD 'secure_password';

-- Grant minimal permissions
GRANT CONNECT ON DATABASE jamey TO jamey;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO jamey;
GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO jamey;

-- Revoke dangerous permissions
REVOKE CREATE ON SCHEMA public FROM jamey;
REVOKE ALL ON DATABASE postgres FROM jamey;
```

## Incident Response

### Detection

**Monitor for**:
- Repeated security violations
- Unusual command executions
- High-frequency API calls
- Failed authentication attempts
- Unexpected file modifications

### Response Procedures

**1. Immediate Actions**:
```bash
# Stop the service
sudo systemctl stop jamey

# Review recent logs
sudo journalctl -u jamey -n 1000 > incident_logs.txt

# Check for unauthorized changes
git status
git diff
```

**2. Investigation**:
```bash
# Analyze audit logs
jq 'select(.timestamp > "2024-01-01T00:00:00Z")' \
    /var/log/jamey/audit.json > investigation.json

# Check file modifications
find /opt/jamey -type f -mtime -1 -ls

# Review network connections
sudo netstat -tulpn | grep jamey
```

**3. Containment**:
```bash
# Isolate system
sudo ufw deny out from any to any

# Revoke credentials
# Rotate all API keys and passwords

# Restore from backup
sudo systemctl stop jamey
sudo -u postgres psql -c "DROP DATABASE jamey;"
sudo -u postgres psql -c "CREATE DATABASE jamey;"
sudo -u postgres psql jamey < /backups/jamey_clean.sql
```

**4. Recovery**:
```bash
# Update all dependencies
cargo update

# Rebuild from source
cargo clean
cargo build --release

# Verify integrity
sha256sum target/release/jamey-runtime

# Restart with monitoring
sudo systemctl start jamey
sudo journalctl -u jamey -f
```

### Post-Incident

- Document incident timeline
- Update security controls
- Review and improve detection
- Conduct lessons learned
- Update incident response plan

## Security Checklist

### Pre-Deployment

- [ ] All secrets stored securely (not in code)
- [ ] Strong, unique passwords generated
- [ ] API keys rotated from development
- [ ] File permissions configured correctly
- [ ] Firewall rules implemented
- [ ] TLS certificates valid and trusted
- [ ] Audit logging enabled
- [ ] Backup system tested
- [ ] Incident response plan documented
- [ ] Security monitoring configured

### Post-Deployment

- [ ] Monitor logs daily
- [ ] Review security violations weekly
- [ ] Rotate credentials monthly
- [ ] Update dependencies monthly
- [ ] Test backups monthly
- [ ] Review access controls quarterly
- [ ] Conduct security audit annually
- [ ] Update incident response plan annually

### Ongoing Operations

- [ ] Monitor resource usage
- [ ] Review approval requests
- [ ] Analyze audit logs
- [ ] Check for security updates
- [ ] Verify backup integrity
- [ ] Test disaster recovery
- [ ] Review and update documentation

## Compliance Considerations

### Data Protection

**GDPR Compliance**:
- Implement data minimization
- Enable right to erasure
- Maintain audit trails
- Encrypt data at rest and in transit
- Document data processing activities

**HIPAA Compliance** (if applicable):
- Implement access controls
- Enable audit logging
- Encrypt PHI
- Conduct risk assessments
- Maintain business associate agreements

### Industry Standards

**SOC 2 Type II**:
- Implement security controls
- Maintain audit trails
- Conduct regular assessments
- Document policies and procedures

**ISO 27001**:
- Implement ISMS
- Conduct risk assessments
- Maintain security documentation
- Regular internal audits

### Regulatory Requirements

Consult with legal counsel regarding:
- Data residency requirements
- Cross-border data transfers
- Industry-specific regulations
- Local privacy laws

## Security Resources

### Internal Documentation

- [AI Agent Overview](README.md)
- [Self-Improvement Guide](self-improvement.md)
- [Admin Assistant Guide](admin-assistant.md)
- [Full System Access Guide](full-system-access.md)
- [Network Access Guide](network-access.md)
- [Agent Orchestration Guide](orchestration.md)
- [24/7 Service Mode Guide](always-on.md)

### External Resources

- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [CIS Benchmarks](https://www.cisecurity.org/cis-benchmarks/)
- [NIST Cybersecurity Framework](https://www.nist.gov/cyberframework)
- [Rust Security Guidelines](https://anssi-fr.github.io/rust-guide/)

### Security Contacts

- **Security Issues**: security@jamey.dev
- **Incident Response**: incident@jamey.dev
- **General Questions**: support@jamey.dev

## Conclusion

Security is an ongoing process, not a one-time configuration. Regularly review and update security controls, monitor for threats, and stay informed about new vulnerabilities and best practices.

**Remember**:
- Defense in depth provides multiple layers of protection
- Audit everything for accountability
- Fail secure when errors occur
- Least privilege minimizes attack surface
- Regular updates patch vulnerabilities

---

**Last Updated**: 2025-11-17  
**Version**: 1.0.0  
**Status**: ✅ Complete  
**Classification**: Internal Use Only