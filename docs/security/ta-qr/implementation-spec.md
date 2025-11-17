# TA-QR Implementation Specification

> **Navigation**: [Documentation Home](../../README.md) > [Security](../README.md) > [TA-QR](README.md) > Implementation Spec

## Overview

This document provides detailed specifications for implementing the TA-QR cryptographic stack. These specifications should be used by Code mode to create the actual implementation.

## File Structure to Create

```
jamey-core/src/crypto/
├── mod.rs              # Module root with public API
├── traits.rs           # Core trait definitions
├── types.rs            # Common types and enums
├── error.rs            # Error types
├── config.rs           # Configuration structures
├── classical.rs        # Classical crypto provider
├── quantum_resistant.rs # PQC provider
├── hybrid.rs           # Hybrid provider
└── utils.rs            # Utility functions

jamey-core/src/
├── secrets_qr.rs       # Quantum-resistant secret manager
└── crypto.rs           # Re-export for backward compatibility
```

## 1. Core Types (`jamey-core/src/crypto/types.rs`)

```rust
use serde::{Deserialize, Serialize};

/// Cryptographic operation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CryptoMode {
    /// Classical cryptography only (RSA, ECDSA, AES)
    Classical,
    /// Quantum-resistant cryptography only (Kyber, Dilithium)
    QuantumResistant,
    /// Hybrid mode combining both
    Hybrid,
}

/// Key Encapsulation Mechanism algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KemAlgorithm {
    /// Classical ECDH P-256
    EcdhP256,
    /// ML-KEM-512 (Kyber512) - 128-bit security
    Kyber512,
    /// ML-KEM-768 (Kyber768) - 192-bit security (recommended)
    Kyber768,
    /// ML-KEM-1024 (Kyber1024) - 256-bit security
    Kyber1024,
}

/// Digital signature algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SigAlgorithm {
    /// Classical ECDSA P-256
    EcdsaP256,
    /// ML-DSA-44 (Dilithium2) - 128-bit security
    Dilithium2,
    /// ML-DSA-65 (Dilithium3) - 192-bit security (recommended)
    Dilithium3,
    /// ML-DSA-87 (Dilithium5) - 256-bit security
    Dilithium5,
}

/// Public key wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKey {
    pub algorithm: String,
    pub key_data: Vec<u8>,
}

/// Private key wrapper (sensitive data)
#[derive(Clone)]
pub struct PrivateKey {
    pub algorithm: String,
    pub key_data: Vec<u8>,
}

// Implement Drop to zeroize private key data
impl Drop for PrivateKey {
    fn drop(&mut self) {
        // Zero out the key data
        for byte in &mut self.key_data {
            *byte = 0;
        }
    }
}

/// Shared secret from key exchange
#[derive(Clone)]
pub struct SharedSecret {
    pub data: Vec<u8>,
}

impl Drop for SharedSecret {
    fn drop(&mut self) {
        for byte in &mut self.data {
            *byte = 0;
        }
    }
}

/// Ciphertext from key encapsulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ciphertext {
    pub algorithm: String,
    pub data: Vec<u8>,
}
```

## 2. Error Types (`jamey-core/src/crypto/error.rs`)

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Key generation failed: {0}")]
    KeyGenerationFailed(String),
    
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    
    #[error("Signature generation failed: {0}")]
    SignatureFailed(String),
    
    #[error("Signature verification failed: {0}")]
    VerificationFailed(String),
    
    #[error("Invalid key format: {0}")]
    InvalidKey(String),
    
    #[error("Invalid algorithm: {0}")]
    InvalidAlgorithm(String),
    
    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Internal error: {0}")]
    InternalError(String),
}

pub type Result<T> = std::result::Result<T, CryptoError>;
```

## 3. Core Traits (`jamey-core/src/crypto/traits.rs`)

```rust
use super::types::*;
use super::error::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// Main cryptographic provider trait
#[async_trait]
pub trait CryptoProvider: Send + Sync {
    /// Get the cryptographic mode
    fn mode(&self) -> CryptoMode;
    
    /// Get key exchange implementation
    fn key_exchange(&self) -> Arc<dyn KeyExchange>;
    
    /// Get signature implementation
    fn signature(&self) -> Arc<dyn Signature>;
    
    /// Get encryption implementation
    fn encryption(&self) -> Arc<dyn Encryption>;
    
    /// Check if this provider is quantum-resistant
    fn is_quantum_resistant(&self) -> bool {
        matches!(self.mode(), CryptoMode::QuantumResistant | CryptoMode::Hybrid)
    }
    
    /// Get provider name
    fn name(&self) -> &str;
}

/// Key exchange and encapsulation trait
#[async_trait]
pub trait KeyExchange: Send + Sync {
    /// Generate a new keypair
    async fn generate_keypair(&self) -> Result<(PublicKey, PrivateKey)>;
    
    /// Encapsulate a shared secret (KEM)
    async fn encapsulate(&self, public_key: &PublicKey) -> Result<(Ciphertext, SharedSecret)>;
    
    /// Decapsulate a shared secret (KEM)
    async fn decapsulate(&self, private_key: &PrivateKey, ciphertext: &Ciphertext) 
        -> Result<SharedSecret>;
    
    /// Get algorithm identifier
    fn algorithm(&self) -> &str;
    
    /// Get key size in bytes
    fn key_size(&self) -> usize;
}

/// Digital signature trait
#[async_trait]
pub trait Signature: Send + Sync {
    /// Generate a signing keypair
    async fn generate_keypair(&self) -> Result<(PublicKey, PrivateKey)>;
    
    /// Sign a message
    async fn sign(&self, private_key: &PrivateKey, message: &[u8]) -> Result<Vec<u8>>;
    
    /// Verify a signature
    async fn verify(&self, public_key: &PublicKey, message: &[u8], signature: &[u8]) 
        -> Result<bool>;
    
    /// Get algorithm identifier
    fn algorithm(&self) -> &str;
    
    /// Get signature size in bytes
    fn signature_size(&self) -> usize;
}

/// Symmetric encryption trait
#[async_trait]
pub trait Encryption: Send + Sync {
    /// Encrypt data with a key
    async fn encrypt(&self, key: &[u8], plaintext: &[u8]) -> Result<Vec<u8>>;
    
    /// Decrypt data with a key
    async fn decrypt(&self, key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>>;
    
    /// Get algorithm identifier
    fn algorithm(&self) -> &str;
    
    /// Get required key size in bytes
    fn key_size(&self) -> usize;
}
```

## 4. Configuration (`jamey-core/src/crypto/config.rs`)

```rust
use super::types::*;
use serde::{Deserialize, Serialize};

/// Cryptographic configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoConfig {
    /// Operating mode
    pub mode: CryptoMode,
    
    /// Key encapsulation mechanism algorithm
    pub kem_algorithm: KemAlgorithm,
    
    /// Signature algorithm
    pub sig_algorithm: SigAlgorithm,
    
    /// Enable dual storage during migration
    pub enable_dual_storage: bool,
    
    /// Verify classical operations in hybrid mode
    pub verify_classical: bool,
    
    /// Enable performance metrics
    pub enable_metrics: bool,
}

impl Default for CryptoConfig {
    fn default() -> Self {
        Self {
            mode: CryptoMode::Hybrid,
            kem_algorithm: KemAlgorithm::Kyber768,
            sig_algorithm: SigAlgorithm::Dilithium3,
            enable_dual_storage: true,
            verify_classical: true,
            enable_metrics: false,
        }
    }
}

impl CryptoConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            mode: std::env::var("CRYPTO_MODE")
                .ok()
                .and_then(|s| match s.to_lowercase().as_str() {
                    "classical" => Some(CryptoMode::Classical),
                    "quantum_resistant" | "qr" => Some(CryptoMode::QuantumResistant),
                    "hybrid" => Some(CryptoMode::Hybrid),
                    _ => None,
                })
                .unwrap_or(CryptoMode::Hybrid),
            
            kem_algorithm: std::env::var("CRYPTO_KEM_ALGORITHM")
                .ok()
                .and_then(|s| match s.to_lowercase().as_str() {
                    "kyber512" => Some(KemAlgorithm::Kyber512),
                    "kyber768" => Some(KemAlgorithm::Kyber768),
                    "kyber1024" => Some(KemAlgorithm::Kyber1024),
                    "ecdh" => Some(KemAlgorithm::EcdhP256),
                    _ => None,
                })
                .unwrap_or(KemAlgorithm::Kyber768),
            
            sig_algorithm: std::env::var("CRYPTO_SIG_ALGORITHM")
                .ok()
                .and_then(|s| match s.to_lowercase().as_str() {
                    "dilithium2" => Some(SigAlgorithm::Dilithium2),
                    "dilithium3" => Some(SigAlgorithm::Dilithium3),
                    "dilithium5" => Some(SigAlgorithm::Dilithium5),
                    "ecdsa" => Some(SigAlgorithm::EcdsaP256),
                    _ => None,
                })
                .unwrap_or(SigAlgorithm::Dilithium3),
            
            enable_dual_storage: std::env::var("CRYPTO_ENABLE_DUAL_STORAGE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
            
            verify_classical: std::env::var("CRYPTO_VERIFY_CLASSICAL")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
            
            enable_metrics: std::env::var("CRYPTO_ENABLE_METRICS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(false),
        }
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        match self.mode {
            CryptoMode::Classical => {
                if !matches!(self.kem_algorithm, KemAlgorithm::EcdhP256) {
                    return Err("Classical mode requires ECDH algorithm".to_string());
                }
                if !matches!(self.sig_algorithm, SigAlgorithm::EcdsaP256) {
                    return Err("Classical mode requires ECDSA algorithm".to_string());
                }
            }
            CryptoMode::QuantumResistant => {
                if matches!(self.kem_algorithm, KemAlgorithm::EcdhP256) {
                    return Err("Quantum-resistant mode requires Kyber algorithm".to_string());
                }
                if matches!(self.sig_algorithm, SigAlgorithm::EcdsaP256) {
                    return Err("Quantum-resistant mode requires Dilithium algorithm".to_string());
                }
            }
            CryptoMode::Hybrid => {
                // Hybrid mode allows any combination
            }
        }
        Ok(())
    }
}
```

## 5. Classical Provider Stub (`jamey-core/src/crypto/classical.rs`)

```rust
//! Classical cryptography provider using standard algorithms
//!
//! This provider wraps existing classical cryptographic operations
//! (ECDH, ECDSA, AES-GCM) to provide a consistent interface.

use super::traits::*;
use super::types::*;
use super::error::*;
use async_trait::async_trait;
use std::sync::Arc;

pub struct ClassicalProvider {
    key_exchange: Arc<ClassicalKeyExchange>,
    signature: Arc<ClassicalSignature>,
    encryption: Arc<ClassicalEncryption>,
}

impl ClassicalProvider {
    pub fn new() -> Self {
        Self {
            key_exchange: Arc::new(ClassicalKeyExchange),
            signature: Arc::new(ClassicalSignature),
            encryption: Arc::new(ClassicalEncryption),
        }
    }
}

#[async_trait]
impl CryptoProvider for ClassicalProvider {
    fn mode(&self) -> CryptoMode {
        CryptoMode::Classical
    }
    
    fn key_exchange(&self) -> Arc<dyn KeyExchange> {
        self.key_exchange.clone()
    }
    
    fn signature(&self) -> Arc<dyn Signature> {
        self.signature.clone()
    }
    
    fn encryption(&self) -> Arc<dyn Encryption> {
        self.encryption.clone()
    }
    
    fn name(&self) -> &str {
        "Classical"
    }
}

struct ClassicalKeyExchange;
struct ClassicalSignature;
struct ClassicalEncryption;

// Implementation details to be added in Code mode
// These will use ring or rustls for ECDH/ECDSA
// and aes-gcm for symmetric encryption
```

## 6. Quantum-Resistant Provider Stub (`jamey-core/src/crypto/quantum_resistant.rs`)

```rust
//! Quantum-resistant cryptography provider using NIST PQC algorithms
//!
//! This provider implements ML-KEM (Kyber) for key exchange and
//! ML-DSA (Dilithium) for digital signatures.

use super::traits::*;
use super::types::*;
use super::error::*;
use super::config::CryptoConfig;
use async_trait::async_trait;
use std::sync::Arc;

pub struct QuantumResistantProvider {
    config: CryptoConfig,
    key_exchange: Arc<dyn KeyExchange>,
    signature: Arc<dyn Signature>,
    encryption: Arc<dyn Encryption>,
}

impl QuantumResistantProvider {
    pub fn new(config: CryptoConfig) -> Result<Self> {
        let key_exchange: Arc<dyn KeyExchange> = match config.kem_algorithm {
            KemAlgorithm::Kyber512 => Arc::new(Kyber512KeyExchange),
            KemAlgorithm::Kyber768 => Arc::new(Kyber768KeyExchange),
            KemAlgorithm::Kyber1024 => Arc::new(Kyber1024KeyExchange),
            _ => return Err(CryptoError::InvalidAlgorithm(
                "Quantum-resistant mode requires Kyber".to_string()
            )),
        };
        
        let signature: Arc<dyn Signature> = match config.sig_algorithm {
            SigAlgorithm::Dilithium2 => Arc::new(Dilithium2Signature),
            SigAlgorithm::Dilithium3 => Arc::new(Dilithium3Signature),
            SigAlgorithm::Dilithium5 => Arc::new(Dilithium5Signature),
            _ => return Err(CryptoError::InvalidAlgorithm(
                "Quantum-resistant mode requires Dilithium".to_string()
            )),
        };
        
        // Use AES-256-GCM for symmetric encryption (quantum-safe for now)
        let encryption = Arc::new(Aes256GcmEncryption);
        
        Ok(Self {
            config,
            key_exchange,
            signature,
            encryption,
        })
    }
}

#[async_trait]
impl CryptoProvider for QuantumResistantProvider {
    fn mode(&self) -> CryptoMode {
        CryptoMode::QuantumResistant
    }
    
    fn key_exchange(&self) -> Arc<dyn KeyExchange> {
        self.key_exchange.clone()
    }
    
    fn signature(&self) -> Arc<dyn Signature> {
        self.signature.clone()
    }
    
    fn encryption(&self) -> Arc<dyn Encryption> {
        self.encryption.clone()
    }
    
    fn name(&self) -> &str {
        "QuantumResistant"
    }
}

// Kyber implementations
struct Kyber512KeyExchange;
struct Kyber768KeyExchange;
struct Kyber1024KeyExchange;

// Dilithium implementations
struct Dilithium2Signature;
struct Dilithium3Signature;
struct Dilithium5Signature;

// AES-256-GCM encryption
struct Aes256GcmEncryption;

// Implementation details to be added in Code mode
// These will use pqcrypto-kyber and pqcrypto-dilithium crates
```

## 7. Hybrid Provider Stub (`jamey-core/src/crypto/hybrid.rs`)

```rust
//! Hybrid cryptography provider combining classical and quantum-resistant algorithms
//!
//! This provider uses both classical and PQC algorithms for defense-in-depth.
//! Shared secrets are derived from both ECDH and Kyber.
//! Signatures include both ECDSA and Dilithium.

use super::traits::*;
use super::types::*;
use super::error::*;
use super::config::CryptoConfig;
use super::classical::ClassicalProvider;
use super::quantum_resistant::QuantumResistantProvider;
use async_trait::async_trait;
use std::sync::Arc;

pub struct HybridProvider {
    classical: ClassicalProvider,
    quantum_resistant: QuantumResistantProvider,
    key_exchange: Arc<HybridKeyExchange>,
    signature: Arc<HybridSignature>,
}

impl HybridProvider {
    pub fn new(config: CryptoConfig) -> Result<Self> {
        let classical = ClassicalProvider::new();
        let quantum_resistant = QuantumResistantProvider::new(config.clone())?;
        
        let key_exchange = Arc::new(HybridKeyExchange {
            classical_kex: classical.key_exchange(),
            qr_kex: quantum_resistant.key_exchange(),
        });
        
        let signature = Arc::new(HybridSignature {
            classical_sig: classical.signature(),
            qr_sig: quantum_resistant.signature(),
        });
        
        Ok(Self {
            classical,
            quantum_resistant,
            key_exchange,
            signature,
        })
    }
}

#[async_trait]
impl CryptoProvider for HybridProvider {
    fn mode(&self) -> CryptoMode {
        CryptoMode::Hybrid
    }
    
    fn key_exchange(&self) -> Arc<dyn KeyExchange> {
        self.key_exchange.clone()
    }
    
    fn signature(&self) -> Arc<dyn Signature> {
        self.signature.clone()
    }
    
    fn encryption(&self) -> Arc<dyn Encryption> {
        // Use quantum-resistant encryption
        self.quantum_resistant.encryption()
    }
    
    fn name(&self) -> &str {
        "Hybrid"
    }
}

struct HybridKeyExchange {
    classical_kex: Arc<dyn KeyExchange>,
    qr_kex: Arc<dyn KeyExchange>,
}

struct HybridSignature {
    classical_sig: Arc<dyn Signature>,
    qr_sig: Arc<dyn Signature>,
}

// Implementation details to be added in Code mode
// Hybrid key exchange: SharedSecret = KDF(ECDH_Secret || Kyber_Secret)
// Hybrid signature: (ECDSA_Sig, Dilithium_Sig) with combined verification
```

## 8. Quantum-Resistant Secret Manager (`jamey-core/src/secrets_qr.rs`)

```rust
//! Quantum-resistant secret manager wrapping the existing SecretManager
//!
//! This module provides quantum-resistant encryption for secrets stored
//! in the system keyring.

use crate::secrets::{SecretManager, SecretError};
use crate::crypto::{CryptoProvider, CryptoConfig, HybridProvider};
use std::sync::Arc;

pub struct QrSecretManager {
    inner: SecretManager,
    crypto_provider: Arc<dyn CryptoProvider>,
    enable_dual_storage: bool,
}

impl QrSecretManager {
    pub fn new(service_name: impl Into<String>, config: CryptoConfig) 
        -> Result<Self, SecretError> {
        let inner = SecretManager::new(service_name)?;
        let crypto_provider = Arc::new(HybridProvider::new(config.clone())
            .map_err(|e| SecretError::InvalidValue(e.to_string()))?);
        
        Ok(Self {
            inner,
            crypto_provider,
            enable_dual_storage: config.enable_dual_storage,
        })
    }
    
    /// Store a secret with quantum-resistant encryption
    pub async fn store_secret_qr(&self, key: &str, value: &str) 
        -> Result<(), SecretError> {
        // Implementation will:
        // 1. Generate a QR keypair
        // 2. Encrypt the value with QR encryption
        // 3. Store encrypted value in keyring
        // 4. If dual_storage enabled, also store classical version
        todo!("Implementation in Code mode")
    }
    
    /// Retrieve and decrypt a quantum-resistant secret
    pub async fn get_secret_qr(&self, key: &str) -> Result<String, SecretError> {
        // Implementation will:
        // 1. Retrieve encrypted value from keyring
        // 2. Decrypt using QR decryption
        // 3. If dual_storage enabled, verify against classical version
        todo!("Implementation in Code mode")
    }
    
    /// Migrate an existing classical secret to quantum-resistant
    pub async fn migrate_secret(&self, key: &str) -> Result<(), SecretError> {
        // Implementation will:
        // 1. Get classical secret
        // 2. Re-encrypt with QR
        // 3. Store both versions during migration
        todo!("Implementation in Code mode")
    }
}
```

## 9. Dependencies to Add to `jamey-core/Cargo.toml`

```toml
[dependencies]
# ... existing dependencies ...

# Post-Quantum Cryptography
pqcrypto-kyber = "0.8"
pqcrypto-dilithium = "0.5"
pqcrypto-traits = "0.3"

# Additional crypto primitives
aes-gcm = "0.10"
hkdf = "0.12"
sha3 = "0.10"

# For hybrid schemes
ring = "0.17"  # Classical ECDH/ECDSA
```

## 10. Module Root (`jamey-core/src/crypto/mod.rs`)

```rust
//! TA-QR (Trusted Agent - Quantum Resistant) Cryptographic Module

pub mod traits;
pub mod types;
pub mod error;
pub mod config;
pub mod classical;
pub mod quantum_resistant;
pub mod hybrid;
pub mod utils;

// Re-exports for convenience
pub use traits::{CryptoProvider, KeyExchange, Signature, Encryption};
pub use types::{CryptoMode, KemAlgorithm, SigAlgorithm, PublicKey, PrivateKey, SharedSecret, Ciphertext};
pub use error::{CryptoError, Result};
pub use config::CryptoConfig;
pub use classical::ClassicalProvider;
pub use quantum_resistant::QuantumResistantProvider;
pub use hybrid::HybridProvider;

/// Create a crypto provider based on configuration
pub fn create_provider(config: CryptoConfig) -> Result<Box<dyn CryptoProvider>> {
    config.validate()
        .map_err(|e| CryptoError::ConfigError(e))?;
    
    match config.mode {
        CryptoMode::Classical => Ok(Box::new(ClassicalProvider::new())),
        CryptoMode::QuantumResistant => Ok(Box::new(QuantumResistantProvider::new(config)?)),
        CryptoMode::Hybrid => Ok(Box::new(HybridProvider::new(config)?)),
    }
}
```

## 11. Update `jamey-core/src/lib.rs`

Add to the existing lib.rs:

```rust
// Add to existing modules
pub mod crypto;
pub mod secrets_qr;

// Re-export for convenience
pub use crypto::{CryptoProvider, CryptoConfig, CryptoMode};
pub use secrets_qr::QrSecretManager;
```

## Implementation Priority

1. **Phase 1** (Immediate):
   - Create all module files with stubs
   - Implement types, error, and config modules
   - Add dependencies to Cargo.toml
   - Ensure `cargo check` passes

2. **Phase 2** (Next):
   - Implement ClassicalProvider (wrapping existing crypto)
   - Add comprehensive tests
   - Document usage patterns

3. **Phase 3** (Following):
   - Implement QuantumResistantProvider with pqcrypto
   - Add performance benchmarks
   - Security review

4. **Phase 4** (Final):
   - Implement HybridProvider
   - Create QrSecretManager
   - Migration tools and documentation

## Testing Strategy

### Unit Tests
- Test each provider independently
- Verify algorithm correctness
- Test error handling
- Memory safety (zeroization)

### Integration Tests
- Test provider switching
- Verify backward compatibility
- Test migration scenarios
- Performance benchmarks

### Example Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_classical_provider() {
        let provider = ClassicalProvider::new();
        assert_eq!(provider.mode(), CryptoMode::Classical);
        assert!(!provider.is_quantum_resistant());
    }
    
    #[tokio::test]
    async fn test_key_exchange_roundtrip() {
        let provider = ClassicalProvider::new();
        let kex = provider.key_exchange();
        
        let (pk, sk) = kex.generate_keypair().await.unwrap();
        let (ct, ss1) = kex.encapsulate(&pk).await.unwrap();
        let ss2 = kex.decapsulate(&sk, &ct).await.unwrap();
        
        assert_eq!(ss1.data, ss2.data);
    }
}
```

## Next Steps for Code Mode

1. Create all module files with the structures defined above
2. Add dependencies to Cargo.toml
3. Implement the types, error, and config modules first
4. Create stub implementations for providers
5. Run `cargo check` to verify compilation
6. Implement ClassicalProvider using existing crypto libraries
7. Add comprehensive tests
8. Proceed with QuantumResistantProvider implementation

## Related Documentation

- [TA-QR Overview](README.md) - Introduction and quick reference
- [TA-QR Architecture](architecture.md) - Design and algorithm selection
- [Usage Guide](usage-guide.md) - Migration guide and usage patterns
- [Security Overview](../README.md) - Overall security architecture

## Notes

- All sensitive data (PrivateKey, SharedSecret) must implement Drop with zeroization
- Use `async_trait` for all trait implementations
- Follow Rust security best practices
- Add tracing/logging for all cryptographic operations
- Implement constant-time operations where applicable
- Consider side-channel attack mitigations

---

**Last Updated**: 2025-11-17
**Status**: 📝 Specification Ready for Implementation
**Category**: Security / Cryptography