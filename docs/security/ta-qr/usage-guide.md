# TA-QR Usage Guide and Migration Manual

> **Navigation**: [Documentation Home](../../README.md) > [Security](../README.md) > [TA-QR](README.md) > Usage Guide

## Table of Contents

1. [Quick Start](#quick-start)
2. [Configuration](#configuration)
3. [Basic Usage](#basic-usage)
4. [Migration Guide](#migration-guide)
5. [Integration Examples](#integration-examples)
6. [Best Practices](#best-practices)
7. [Troubleshooting](#troubleshooting)

## Quick Start

### Installation

Add TA-QR support to your project by ensuring `jamey-core` is up to date:

```toml
[dependencies]
jamey-core = { version = "0.1.0", features = ["crypto"] }
```

### Basic Configuration

Set environment variables to enable TA-QR:

```bash
# Enable hybrid mode (recommended for migration)
export CRYPTO_MODE=hybrid

# Select algorithms (defaults shown)
export CRYPTO_KEM_ALGORITHM=kyber768
export CRYPTO_SIG_ALGORITHM=dilithium3

# Enable dual storage during migration
export CRYPTO_ENABLE_DUAL_STORAGE=true
export CRYPTO_VERIFY_CLASSICAL=true
```

### First Example

```rust
use jamey_core::crypto::{CryptoConfig, create_provider};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration from environment
    let config = CryptoConfig::from_env();
    
    // Create crypto provider
    let provider = create_provider(config)?;
    
    println!("Using {} mode", provider.name());
    println!("Quantum-resistant: {}", provider.is_quantum_resistant());
    
    // Generate a keypair
    let kex = provider.key_exchange();
    let (public_key, private_key) = kex.generate_keypair().await?;
    
    println!("Generated keypair with algorithm: {}", kex.algorithm());
    
    Ok(())
}
```

## Configuration

### Configuration Options

#### CryptoMode

| Mode | Description | Use Case |
|------|-------------|----------|
| `Classical` | Traditional cryptography only | Legacy systems, testing |
| `QuantumResistant` | PQC algorithms only | Future-proof new systems |
| `Hybrid` | Both classical and PQC | **Recommended for migration** |

#### KEM Algorithms

| Algorithm | Security Level | Key Size | Ciphertext Size | Performance |
|-----------|----------------|----------|-----------------|-------------|
| `EcdhP256` | Classical | 32 B | 32 B | Fast |
| `Kyber512` | 128-bit | 800 B | 768 B | Very Fast |
| `Kyber768` | **192-bit** | 1184 B | 1088 B | **Fast (Recommended)** |
| `Kyber1024` | 256-bit | 1568 B | 1568 B | Fast |

#### Signature Algorithms

| Algorithm | Security Level | Public Key | Signature Size | Performance |
|-----------|----------------|------------|----------------|-------------|
| `EcdsaP256` | Classical | 32 B | 64 B | Fast |
| `Dilithium2` | 128-bit | 1312 B | 2420 B | Medium |
| `Dilithium3` | **192-bit** | 1952 B | 3293 B | **Medium (Recommended)** |
| `Dilithium5` | 256-bit | 2592 B | 4595 B | Slower |

### Configuration Methods

#### 1. Environment Variables

```bash
# config/production.env
CRYPTO_MODE=hybrid
CRYPTO_KEM_ALGORITHM=kyber768
CRYPTO_SIG_ALGORITHM=dilithium3
CRYPTO_ENABLE_DUAL_STORAGE=true
CRYPTO_VERIFY_CLASSICAL=true
CRYPTO_ENABLE_METRICS=true
```

#### 2. Programmatic Configuration

```rust
use jamey_core::crypto::{CryptoConfig, CryptoMode, KemAlgorithm, SigAlgorithm};

let config = CryptoConfig {
    mode: CryptoMode::Hybrid,
    kem_algorithm: KemAlgorithm::Kyber768,
    sig_algorithm: SigAlgorithm::Dilithium3,
    enable_dual_storage: true,
    verify_classical: true,
    enable_metrics: false,
};

// Validate configuration
config.validate()?;
```

#### 3. Configuration File

```toml
# crypto.toml
[crypto]
mode = "hybrid"
kem_algorithm = "kyber768"
sig_algorithm = "dilithium3"
enable_dual_storage = true
verify_classical = true
enable_metrics = false
```

## Basic Usage

### Key Exchange

```rust
use jamey_core::crypto::{CryptoConfig, create_provider};

async fn key_exchange_example() -> Result<(), Box<dyn std::error::Error>> {
    let config = CryptoConfig::from_env();
    let provider = create_provider(config)?;
    let kex = provider.key_exchange();
    
    // Alice generates a keypair
    let (alice_pk, alice_sk) = kex.generate_keypair().await?;
    
    // Bob encapsulates a shared secret to Alice
    let (ciphertext, bob_shared_secret) = kex.encapsulate(&alice_pk).await?;
    
    // Alice decapsulates to get the same shared secret
    let alice_shared_secret = kex.decapsulate(&alice_sk, &ciphertext).await?;
    
    // Both parties now have the same shared secret
    assert_eq!(alice_shared_secret.data, bob_shared_secret.data);
    
    Ok(())
}
```

### Digital Signatures

```rust
use jamey_core::crypto::{CryptoConfig, create_provider};

async fn signature_example() -> Result<(), Box<dyn std::error::Error>> {
    let config = CryptoConfig::from_env();
    let provider = create_provider(config)?;
    let sig = provider.signature();
    
    // Generate signing keypair
    let (public_key, private_key) = sig.generate_keypair().await?;
    
    // Sign a message
    let message = b"Hello, quantum-resistant world!";
    let signature = sig.sign(&private_key, message).await?;
    
    // Verify the signature
    let is_valid = sig.verify(&public_key, message, &signature).await?;
    assert!(is_valid);
    
    // Verification fails with wrong message
    let wrong_message = b"Wrong message";
    let is_valid = sig.verify(&public_key, wrong_message, &signature).await?;
    assert!(!is_valid);
    
    Ok(())
}
```

### Symmetric Encryption

```rust
use jamey_core::crypto::{CryptoConfig, create_provider};

async fn encryption_example() -> Result<(), Box<dyn std::error::Error>> {
    let config = CryptoConfig::from_env();
    let provider = create_provider(config)?;
    let enc = provider.encryption();
    
    // Generate a random key (use key exchange in practice)
    let key = vec![0u8; enc.key_size()];
    
    // Encrypt data
    let plaintext = b"Sensitive data";
    let ciphertext = enc.encrypt(&key, plaintext).await?;
    
    // Decrypt data
    let decrypted = enc.decrypt(&key, &ciphertext).await?;
    assert_eq!(plaintext, &decrypted[..]);
    
    Ok(())
}
```

### Quantum-Resistant Secret Storage

```rust
use jamey_core::secrets_qr::QrSecretManager;
use jamey_core::crypto::CryptoConfig;

async fn secret_storage_example() -> Result<(), Box<dyn std::error::Error>> {
    let config = CryptoConfig::from_env();
    let manager = QrSecretManager::new("my-app", config)?;
    
    // Store a secret with quantum-resistant encryption
    manager.store_secret_qr("api_key", "sk_live_abc123").await?;
    
    // Retrieve the secret
    let api_key = manager.get_secret_qr("api_key").await?;
    assert_eq!(api_key, "sk_live_abc123");
    
    Ok(())
}
```

## Migration Guide

### Migration Strategy Overview

```mermaid
graph LR
    A[Classical Only] --> B[Hybrid Mode]
    B --> C[Dual Storage]
    C --> D[Verification]
    D --> E[Pure QR]
    
    style A fill:#ff9999
    style B fill:#ffff99
    style C fill:#ffff99
    style D fill:#99ff99
    style E fill:#99ff99
```

### Phase 1: Enable Hybrid Mode

**Goal**: Add quantum-resistant cryptography alongside classical

**Duration**: 1-2 weeks

**Steps**:

1. **Update Configuration**:
```bash
# Before
CRYPTO_MODE=classical

# After
CRYPTO_MODE=hybrid
CRYPTO_ENABLE_DUAL_STORAGE=true
CRYPTO_VERIFY_CLASSICAL=true
```

2. **Deploy with Monitoring**:
```rust
// Add metrics to track both modes
let config = CryptoConfig::from_env();
config.enable_metrics = true;
```

3. **Verify Functionality**:
```bash
# Run integration tests
cargo test --features crypto

# Check logs for any errors
grep "crypto" logs/jamey.log
```

### Phase 2: Migrate Existing Secrets

**Goal**: Re-encrypt existing secrets with quantum-resistant algorithms

**Duration**: 2-4 weeks

**Steps**:

1. **Create Migration Script**:
```rust
use jamey_core::secrets::{SecretManager};
use jamey_core::secrets_qr::QrSecretManager;
use jamey_core::crypto::CryptoConfig;

async fn migrate_secrets() -> Result<(), Box<dyn std::error::Error>> {
    let classical_manager = SecretManager::new("my-app")?;
    let config = CryptoConfig::from_env();
    let qr_manager = QrSecretManager::new("my-app", config)?;
    
    // List of keys to migrate
    let keys = vec!["api_key", "db_password", "jwt_secret"];
    
    for key in keys {
        println!("Migrating {}...", key);
        
        // Get classical secret
        let value = classical_manager.get_secret(key)?;
        
        // Store with QR encryption
        qr_manager.store_secret_qr(key, &value).await?;
        
        // Verify migration
        let qr_value = qr_manager.get_secret_qr(key).await?;
        assert_eq!(value, qr_value);
        
        println!("✓ Migrated {}", key);
    }
    
    Ok(())
}
```

2. **Run Migration in Stages**:
```bash
# Migrate non-critical secrets first
./migrate-secrets --keys "test_key,dev_key"

# Verify for 24 hours

# Migrate production secrets
./migrate-secrets --keys "api_key,db_password"
```

3. **Monitor and Validate**:
```bash
# Check dual storage is working
grep "dual_storage" logs/jamey.log

# Verify both classical and QR decryption work
./verify-secrets --mode both
```

### Phase 3: Transition to Pure Quantum-Resistant

**Goal**: Remove classical cryptography dependency

**Duration**: 4-8 weeks

**Steps**:

1. **Disable Classical Verification**:
```bash
# After 30 days of successful dual operation
CRYPTO_VERIFY_CLASSICAL=false
```

2. **Monitor for Issues**:
```bash
# Watch for any decryption failures
tail -f logs/jamey.log | grep "decrypt"
```

3. **Remove Classical Storage**:
```bash
# After 60 days
CRYPTO_ENABLE_DUAL_STORAGE=false
```

4. **Switch to Pure QR Mode**:
```bash
# After 90 days of successful operation
CRYPTO_MODE=quantum_resistant
```

### Migration Checklist

- [ ] **Week 1-2**: Enable hybrid mode in development
- [ ] **Week 2-3**: Deploy hybrid mode to staging
- [ ] **Week 3-4**: Enable dual storage in production
- [ ] **Week 4-6**: Migrate all secrets to QR encryption
- [ ] **Week 6-8**: Verify all operations work with QR
- [ ] **Week 8-10**: Disable classical verification
- [ ] **Week 10-12**: Remove dual storage
- [ ] **Week 12+**: Switch to pure quantum-resistant mode

### Rollback Plan

If issues occur during migration:

```bash
# Immediate rollback to classical
CRYPTO_MODE=classical
CRYPTO_ENABLE_DUAL_STORAGE=false

# Restart services
systemctl restart jamey-runtime

# Verify classical operation
./verify-secrets --mode classical
```

## Integration Examples

### Integration with Existing SecretManager

```rust
use jamey_core::secrets::SecretManager;
use jamey_core::secrets_qr::QrSecretManager;
use jamey_core::crypto::CryptoConfig;

pub struct UnifiedSecretManager {
    classical: SecretManager,
    quantum_resistant: Option<QrSecretManager>,
}

impl UnifiedSecretManager {
    pub fn new(service_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let classical = SecretManager::new(service_name)?;
        
        // Try to enable QR if configured
        let quantum_resistant = if std::env::var("CRYPTO_MODE").is_ok() {
            let config = CryptoConfig::from_env();
            Some(QrSecretManager::new(service_name, config)?)
        } else {
            None
        };
        
        Ok(Self {
            classical,
            quantum_resistant,
        })
    }
    
    pub async fn get_secret(&self, key: &str) -> Result<String, Box<dyn std::error::Error>> {
        // Try QR first if available
        if let Some(qr) = &self.quantum_resistant {
            match qr.get_secret_qr(key).await {
                Ok(value) => return Ok(value),
                Err(_) => {
                    // Fall back to classical
                    tracing::warn!("QR secret not found, falling back to classical");
                }
            }
        }
        
        // Use classical
        Ok(self.classical.get_secret(key)?)
    }
}
```

### Integration with TLS Configuration

```rust
use jamey_runtime::tls::TlsConfig;
use jamey_core::crypto::{CryptoConfig, create_provider};

pub struct QrTlsConfig {
    tls_config: TlsConfig,
    crypto_provider: Box<dyn CryptoProvider>,
}

impl QrTlsConfig {
    pub fn new(tls_config: TlsConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let crypto_config = CryptoConfig::from_env();
        let crypto_provider = create_provider(crypto_config)?;
        
        Ok(Self {
            tls_config,
            crypto_provider,
        })
    }
    
    pub async fn generate_session_key(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let kex = self.crypto_provider.key_exchange();
        let (pk, sk) = kex.generate_keypair().await?;
        let (ct, ss) = kex.encapsulate(&pk).await?;
        
        Ok(ss.data)
    }
}
```

### Integration with Database Encryption

```rust
use jamey_core::crypto::{CryptoConfig, create_provider};
use tokio_postgres::Client;

pub struct QrDatabaseClient {
    client: Client,
    crypto_provider: Box<dyn CryptoProvider>,
}

impl QrDatabaseClient {
    pub async fn store_encrypted(&self, table: &str, data: &[u8]) 
        -> Result<(), Box<dyn std::error::Error>> {
        let enc = self.crypto_provider.encryption();
        
        // Generate a key from key exchange
        let kex = self.crypto_provider.key_exchange();
        let (pk, sk) = kex.generate_keypair().await?;
        let (ct, ss) = kex.encapsulate(&pk).await?;
        
        // Encrypt data
        let encrypted = enc.encrypt(&ss.data, data).await?;
        
        // Store in database
        self.client.execute(
            &format!("INSERT INTO {} (data, key_ct) VALUES ($1, $2)", table),
            &[&encrypted, &ct.data],
        ).await?;
        
        Ok(())
    }
}
```

## Best Practices

### 1. Key Management

```rust
// ✅ DO: Generate fresh keys regularly
async fn rotate_keys(provider: &dyn CryptoProvider) -> Result<(), Box<dyn std::error::Error>> {
    let kex = provider.key_exchange();
    let (new_pk, new_sk) = kex.generate_keypair().await?;
    
    // Store new keys securely
    // Invalidate old keys
    
    Ok(())
}

// ❌ DON'T: Reuse keys indefinitely
// Keys should be rotated every 90 days
```

### 2. Error Handling

```rust
// ✅ DO: Handle crypto errors gracefully
async fn safe_decrypt(
    provider: &dyn CryptoProvider,
    key: &[u8],
    ciphertext: &[u8]
) -> Result<Vec<u8>, String> {
    let enc = provider.encryption();
    
    enc.decrypt(key, ciphertext)
        .await
        .map_err(|e| {
            tracing::error!("Decryption failed: {}", e);
            "Decryption failed".to_string()
        })
}

// ❌ DON'T: Expose crypto errors to users
// This could leak information about the system
```

### 3. Performance Optimization

```rust
// ✅ DO: Cache crypto providers
use std::sync::Arc;
use once_cell::sync::Lazy;

static CRYPTO_PROVIDER: Lazy<Arc<dyn CryptoProvider>> = Lazy::new(|| {
    let config = CryptoConfig::from_env();
    Arc::new(create_provider(config).expect("Failed to create provider"))
});

// ❌ DON'T: Create new providers for each operation
// This is expensive and unnecessary
```

### 4. Security Considerations

```rust
// ✅ DO: Zeroize sensitive data
impl Drop for SensitiveData {
    fn drop(&mut self) {
        for byte in &mut self.data {
            *byte = 0;
        }
    }
}

// ✅ DO: Use constant-time operations
use subtle::ConstantTimeEq;

fn compare_secrets(a: &[u8], b: &[u8]) -> bool {
    a.ct_eq(b).into()
}

// ❌ DON'T: Use regular comparison for secrets
// This is vulnerable to timing attacks
```

### 5. Monitoring and Logging

```rust
use tracing::{info, warn, error};

async fn monitored_operation(provider: &dyn CryptoProvider) 
    -> Result<(), Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();
    
    info!(
        mode = ?provider.mode(),
        "Starting cryptographic operation"
    );
    
    let result = perform_crypto_operation(provider).await;
    
    let duration = start.elapsed();
    
    match &result {
        Ok(_) => info!(
            duration_ms = duration.as_millis(),
            "Operation completed successfully"
        ),
        Err(e) => error!(
            error = %e,
            duration_ms = duration.as_millis(),
            "Operation failed"
        ),
    }
    
    result
}
```

## Troubleshooting

### Common Issues

#### 1. "Invalid algorithm" Error

**Problem**: Configuration specifies incompatible algorithms

**Solution**:
```bash
# Check configuration
echo $CRYPTO_MODE
echo $CRYPTO_KEM_ALGORITHM

# Fix: Ensure algorithms match mode
# For quantum_resistant mode:
export CRYPTO_KEM_ALGORITHM=kyber768
export CRYPTO_SIG_ALGORITHM=dilithium3
```

#### 2. Performance Degradation

**Problem**: Operations are slower after enabling QR

**Solution**:
```rust
// Enable caching
let config = CryptoConfig {
    mode: CryptoMode::Hybrid,
    enable_metrics: true,
    ..Default::default()
};

// Monitor performance
// Consider using Kyber512 for better performance
// if 128-bit security is sufficient
```

#### 3. Migration Failures

**Problem**: Secrets fail to decrypt after migration

**Solution**:
```bash
# Enable dual storage
export CRYPTO_ENABLE_DUAL_STORAGE=true

# Verify both versions work
./verify-secrets --mode both

# Check logs
grep "decrypt" logs/jamey.log | grep "error"
```

#### 4. Memory Issues

**Problem**: High memory usage with QR algorithms

**Solution**:
```rust
// Use smaller security levels
let config = CryptoConfig {
    kem_algorithm: KemAlgorithm::Kyber512,  // Instead of Kyber1024
    sig_algorithm: SigAlgorithm::Dilithium2, // Instead of Dilithium5
    ..Default::default()
};
```

### Debug Mode

Enable debug logging:

```bash
export RUST_LOG=jamey_core::crypto=debug
export CRYPTO_ENABLE_METRICS=true
```

### Getting Help

1. Check logs: `tail -f logs/jamey.log | grep crypto`
2. Run diagnostics: `cargo test --features crypto -- --nocapture`
3. Review configuration: `./check-crypto-config`
4. Open an issue with:
   - Configuration used
   - Error messages
   - Steps to reproduce

## Performance Benchmarks

### Expected Performance (Approximate)

| Operation | Classical | Kyber768 | Dilithium3 | Hybrid |
|-----------|-----------|----------|------------|--------|
| Keygen | 0.1ms | 0.05ms | 0.2ms | 0.25ms |
| Encap/Sign | 0.2ms | 0.1ms | 1.5ms | 1.7ms |
| Decap/Verify | 0.3ms | 0.15ms | 0.5ms | 0.8ms |

### Running Benchmarks

```bash
# Run all crypto benchmarks
cargo bench --features crypto

# Run specific benchmark
cargo bench --features crypto -- key_exchange

# Compare modes
cargo bench --features crypto -- --save-baseline classical
export CRYPTO_MODE=quantum_resistant
cargo bench --features crypto -- --baseline classical
```

## Additional Resources

- [TA-QR Architecture](./TA_QR_ARCHITECTURE.md)
- [Implementation Specification](./TA_QR_IMPLEMENTATION_SPEC.md)
- [NIST PQC Standards](https://csrc.nist.gov/projects/post-quantum-cryptography)
- [pqcrypto Documentation](https://docs.rs/pqcrypto/)

## Support

For questions or issues:
- GitHub Issues: https://github.com/jamey-code/jamey/issues
- Documentation: https://docs.jamey.dev/crypto
- Security: security@jamey.dev (for security-related issues)