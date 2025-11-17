# Log Security and Protection Guidelines

> **Navigation**: [Documentation Home](../README.md) > [Security](README.md) > Log Security

## Overview

This document outlines the security measures implemented for logging in the Jamey project, including automatic PII filtering, log rotation, retention policies, and encryption requirements.

## Automatic Sensitive Data Filtering

### Implemented Protections

The `jamey-core::secure_logging` module provides automatic redaction of sensitive information from all log messages:

#### 1. **API Keys and Tokens**
- Pattern: `api_key`, `apikey`, `token`, `access_token`, `auth_token`
- Redaction: `***REDACTED***`
- Examples:
  - `api_key: sk_test_1234...` → `api_key=***REDACTED***`
  - `Bearer eyJhbGc...` → `Bearer ***REDACTED***`

#### 2. **Passwords and Secrets**
- Pattern: `password`, `passwd`, `pwd`, `secret`, `client_secret`
- Redaction: `***REDACTED***`
- Examples:
  - `password=mysecret123` → `password=***REDACTED***`
  - `client_secret: abc123...` → `client_secret=***REDACTED***`

#### 3. **Database Credentials**
- Pattern: Database connection strings with embedded credentials
- Redaction: Password portion replaced with `***REDACTED***`
- Example:
  - `postgres://user:pass@host/db` → `postgres://user:***REDACTED***@host/db`

#### 4. **Email Addresses**
- Pattern: Standard email format
- Redaction: `***EMAIL_REDACTED***`
- Example:
  - `user@example.com` → `***EMAIL_REDACTED***`

#### 5. **IP Addresses**
- Pattern: IPv4 addresses
- Redaction: `***IP_REDACTED***`
- Note: Can be disabled if IPs are not considered sensitive in your environment

#### 6. **JWT Tokens**
- Pattern: Standard JWT format (three base64 segments)
- Redaction: `***JWT_REDACTED***`

#### 7. **Provider-Specific Tokens**
- GitHub tokens: `gh[ps]_...` → `***GITHUB_TOKEN_REDACTED***`
- AWS keys: `AKIA...` → `***AWS_KEY_REDACTED***`

#### 8. **Credit Cards and SSNs**
- Credit card numbers → `***CC_REDACTED***`
- Social Security Numbers → `***SSN_REDACTED***`

### Field-Level Filtering

In addition to pattern-based filtering, the logging system automatically redacts any field with a sensitive name:

- `password`, `passwd`, `pwd`
- `secret`, `api_key`, `apikey`
- `token`, `access_token`, `refresh_token`, `auth_token`
- `authorization`, `client_secret`, `private_key`
- `credential`, `credentials`, `session_id`
- `cookie`, `auth`

## Log Configuration

### Default Settings

```rust
LogConfig {
    log_dir: "./logs",
    max_file_size: 10 * 1024 * 1024,  // 10MB
    max_backups: 10,
    retention_days: 30,
    compress: true,
    level: Level::INFO,
}
```

### Environment-Specific Configuration

#### Development
- Log Level: `DEBUG`
- Retention: 7 days
- Compression: Disabled for easier debugging

#### Production
- Log Level: `INFO` or `WARN`
- Retention: 30-90 days (based on compliance requirements)
- Compression: Enabled
- Encryption: Required (see below)

### Configuring Log Levels

Set the `RUST_LOG` environment variable to control log verbosity:

```bash
# Show all logs
export RUST_LOG=debug

# Show only warnings and errors
export RUST_LOG=warn

# Module-specific logging
export RUST_LOG=jamey_core=debug,jamey_runtime=info
```

## Log Rotation

### Automatic Rotation

Logs are automatically rotated daily using `tracing-appender`:

- **Rotation Schedule**: Daily at midnight (local time)
- **File Naming**: `jamey.log.YYYY-MM-DD`
- **Current Log**: `jamey.log` (symlink or latest file)

### Manual Rotation

To manually rotate logs:

```bash
# Move current log file
mv logs/jamey.log logs/jamey.log.$(date +%Y-%m-%d-%H%M%S)

# The system will automatically create a new log file
```

## Log Retention

### Automatic Cleanup

The system should implement automatic cleanup of old log files based on the `retention_days` configuration:

```rust
// Recommended implementation (to be added to a maintenance task)
async fn cleanup_old_logs(config: &LogConfig) -> Result<()> {
    let cutoff = Utc::now() - Duration::days(config.retention_days as i64);
    
    for entry in fs::read_dir(&config.log_dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        
        if let Ok(modified) = metadata.modified() {
            let modified_time: DateTime<Utc> = modified.into();
            if modified_time < cutoff {
                fs::remove_file(entry.path())?;
            }
        }
    }
    
    Ok(())
}
```

### Manual Cleanup

To manually clean up old logs:

```bash
# Remove logs older than 30 days
find logs/ -name "jamey.log.*" -mtime +30 -delete
```

## Log Encryption

### Requirements

**CRITICAL**: In production environments, logs MUST be encrypted both in transit and at rest.

### Encryption at Rest

#### Option 1: Filesystem-Level Encryption (Recommended)

Use OS-level encryption for the log directory:

**Linux (LUKS)**:
```bash
# Create encrypted volume
cryptsetup luksFormat /dev/sdX
cryptsetup open /dev/sdX jamey_logs
mkfs.ext4 /dev/mapper/jamey_logs
mount /dev/mapper/jamey_logs /var/log/jamey
```

**Windows (BitLocker)**:
```powershell
# Enable BitLocker on the drive containing logs
Enable-BitLocker -MountPoint "D:" -EncryptionMethod Aes256
```

**macOS (FileVault)**:
```bash
# Enable FileVault for the entire disk
sudo fdesetup enable
```

#### Option 2: Application-Level Encryption

For application-level encryption, implement a custom log writer:

```rust
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, NewAead};

struct EncryptedLogWriter {
    cipher: Aes256Gcm,
    inner: File,
}

impl EncryptedLogWriter {
    fn new(path: impl AsRef<Path>, key: &[u8; 32]) -> Result<Self> {
        let key = Key::from_slice(key);
        let cipher = Aes256Gcm::new(key);
        let inner = File::create(path)?;
        Ok(Self { cipher, inner })
    }
    
    fn write_encrypted(&mut self, data: &[u8]) -> Result<()> {
        let nonce = Nonce::from_slice(b"unique nonce"); // Use proper nonce generation
        let ciphertext = self.cipher.encrypt(nonce, data)?;
        self.inner.write_all(&ciphertext)?;
        Ok(())
    }
}
```

### Encryption in Transit

When shipping logs to a centralized logging system:

#### Option 1: TLS/HTTPS

```rust
// Configure log shipper with TLS
let client = reqwest::Client::builder()
    .min_tls_version(reqwest::tls::Version::TLS_1_2)
    .build()?;

// Send logs over HTTPS
client.post("https://logs.example.com/ingest")
    .json(&log_entry)
    .send()
    .await?;
```

#### Option 2: VPN/Secure Tunnel

- Use WireGuard, OpenVPN, or similar for log transmission
- Ensure all log traffic goes through encrypted tunnels

## Access Control

### File Permissions

Set restrictive permissions on log files:

```bash
# Linux/macOS
chmod 600 logs/jamey.log
chown jamey:jamey logs/

# Windows (PowerShell)
$acl = Get-Acl "logs\jamey.log"
$acl.SetAccessRuleProtection($true, $false)
$rule = New-Object System.Security.AccessControl.FileSystemAccessRule(
    "SYSTEM", "FullControl", "Allow"
)
$acl.AddAccessRule($rule)
Set-Acl "logs\jamey.log" $acl
```

### Audit Logging

Enable audit logging for log file access:

```bash
# Linux (auditd)
auditctl -w /var/log/jamey/ -p rwa -k jamey_logs

# Windows (Event Viewer)
# Configure auditing through Group Policy or Security Settings
```

## Compliance Considerations

### GDPR

- **Right to Erasure**: Implement mechanisms to remove user data from logs
- **Data Minimization**: Only log necessary information
- **Retention Limits**: Enforce maximum retention periods

### HIPAA

- **Encryption Required**: All logs containing PHI must be encrypted
- **Access Logs**: Maintain audit trails of log access
- **Retention**: Follow organizational retention policies

### PCI DSS

- **No Cardholder Data**: Never log full credit card numbers
- **Encryption**: Encrypt logs containing any payment information
- **Access Control**: Restrict log access to authorized personnel only

## Monitoring and Alerting

### Security Events to Monitor

1. **Failed Authentication Attempts**
   - Pattern: Multiple failed login attempts
   - Action: Alert security team

2. **Unusual API Usage**
   - Pattern: Excessive API calls, unusual endpoints
   - Action: Rate limit and investigate

3. **Data Access Patterns**
   - Pattern: Bulk data exports, unusual queries
   - Action: Review and audit

4. **Configuration Changes**
   - Pattern: Security settings modified
   - Action: Alert and require approval

### Recommended Tools

- **Log Aggregation**: ELK Stack, Splunk, Datadog
- **SIEM**: Wazuh, OSSEC, AlienVault
- **Alerting**: PagerDuty, Opsgenie, custom webhooks

## Testing

### Verify Redaction

Test that sensitive data is properly redacted:

```rust
#[test]
fn test_log_redaction() {
    let test_cases = vec![
        ("api_key: sk_test_123", "api_key=***REDACTED***"),
        ("password=secret123", "password=***REDACTED***"),
        ("user@example.com", "***EMAIL_REDACTED***"),
    ];
    
    for (input, expected) in test_cases {
        let output = redact_sensitive_data(input);
        assert!(output.contains(expected));
        assert!(!output.contains("sk_test_123"));
        assert!(!output.contains("secret123"));
        assert!(!output.contains("user@example.com"));
    }
}
```

### Penetration Testing

Regularly test log security:

1. Attempt to access logs without proper credentials
2. Try to inject malicious data into logs
3. Verify encryption is working correctly
4. Test log rotation and cleanup mechanisms

## Incident Response

### Log Compromise Procedure

If logs are compromised:

1. **Immediate Actions**
   - Rotate all API keys and credentials
   - Disable affected accounts
   - Isolate compromised systems

2. **Investigation**
   - Determine scope of breach
   - Identify what data was exposed
   - Review access logs

3. **Remediation**
   - Patch vulnerabilities
   - Enhance access controls
   - Update encryption keys

4. **Notification**
   - Notify affected users (if required)
   - Report to regulatory bodies (if required)
   - Document incident for future reference

## Best Practices

1. **Never Log Sensitive Data Directly**
   - Use the secure logging framework
   - Always redact before logging

2. **Use Structured Logging**
   - Leverage field-level filtering
   - Make logs machine-readable

3. **Regular Audits**
   - Review logs for accidental data exposure
   - Update redaction patterns as needed

4. **Principle of Least Privilege**
   - Limit log access to necessary personnel
   - Use role-based access control

5. **Defense in Depth**
   - Multiple layers of protection
   - Encryption + access control + monitoring

## Related Documentation

- [Security Overview](README.md) - Overall security architecture
- [TLS Configuration](tls-configuration.md) - Secure communication
- [TA-QR Overview](ta-qr/README.md) - Quantum-resistant cryptography
- [Performance Monitoring](../operations/performance-monitoring.md) - Log analysis

## References

- [OWASP Logging Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html)
- [NIST SP 800-92: Guide to Computer Security Log Management](https://csrc.nist.gov/publications/detail/sp/800-92/final)
- [CIS Controls: Log Management](https://www.cisecurity.org/controls/)

---

**Last Updated**: 2025-11-17
**Status**: ✅ Complete
**Category**: Security