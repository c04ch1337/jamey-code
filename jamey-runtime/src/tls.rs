//! TLS/HTTPS configuration and certificate management
//!
//! This module provides secure TLS configuration for production deployments,
//! including certificate loading, validation, and security headers.

use anyhow::{Context, Result};
use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, info, warn};

#[derive(Debug, Error)]
pub enum TlsError {
    #[error("Certificate file not found: {0}")]
    CertificateNotFound(String),
    #[error("Private key file not found: {0}")]
    PrivateKeyNotFound(String),
    #[error("Failed to load certificate: {0}")]
    CertificateLoadError(String),
    #[error("Failed to load private key: {0}")]
    PrivateKeyLoadError(String),
    #[error("Invalid certificate format: {0}")]
    InvalidCertificate(String),
    #[error("Invalid private key format: {0}")]
    InvalidPrivateKey(String),
    #[error("TLS configuration error: {0}")]
    ConfigError(String),
}

/// TLS configuration for HTTPS server
#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// Path to the certificate file (PEM format)
    pub cert_path: PathBuf,
    /// Path to the private key file (PEM format)
    pub key_path: PathBuf,
    /// Optional CA certificate path for client authentication
    pub ca_cert_path: Option<PathBuf>,
    /// Minimum TLS version (1.2 or 1.3)
    pub min_tls_version: TlsVersion,
    /// Enable HTTP Strict Transport Security (HSTS)
    pub enable_hsts: bool,
    /// HSTS max-age in seconds (default: 1 year)
    pub hsts_max_age: u64,
    /// Include subdomains in HSTS
    pub hsts_include_subdomains: bool,
    /// Enable HSTS preload
    pub hsts_preload: bool,
}

/// Supported TLS versions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsVersion {
    /// TLS 1.2 (minimum recommended)
    Tls12,
    /// TLS 1.3 (preferred)
    Tls13,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            cert_path: PathBuf::from("/etc/ssl/certs/jamey.crt"),
            key_path: PathBuf::from("/etc/ssl/private/jamey.key"),
            ca_cert_path: None,
            min_tls_version: TlsVersion::Tls13,
            enable_hsts: true,
            hsts_max_age: 31536000, // 1 year
            hsts_include_subdomains: true,
            hsts_preload: false,
        }
    }
}

impl TlsConfig {
    /// Create a new TLS configuration
    pub fn new(cert_path: PathBuf, key_path: PathBuf) -> Self {
        Self {
            cert_path,
            key_path,
            ..Default::default()
        }
    }

    /// Set the CA certificate path for client authentication
    pub fn with_ca_cert(mut self, ca_cert_path: PathBuf) -> Self {
        self.ca_cert_path = Some(ca_cert_path);
        self
    }

    /// Set the minimum TLS version
    pub fn with_min_tls_version(mut self, version: TlsVersion) -> Self {
        self.min_tls_version = version;
        self
    }

    /// Configure HSTS settings
    pub fn with_hsts(
        mut self,
        enable: bool,
        max_age: u64,
        include_subdomains: bool,
        preload: bool,
    ) -> Self {
        self.enable_hsts = enable;
        self.hsts_max_age = max_age;
        self.hsts_include_subdomains = include_subdomains;
        self.hsts_preload = preload;
        self
    }

    /// Validate the TLS configuration
    pub fn validate(&self) -> Result<(), TlsError> {
        if !self.cert_path.exists() {
            return Err(TlsError::CertificateNotFound(
                self.cert_path.display().to_string(),
            ));
        }

        if !self.key_path.exists() {
            return Err(TlsError::PrivateKeyNotFound(
                self.key_path.display().to_string(),
            ));
        }

        if let Some(ca_path) = &self.ca_cert_path {
            if !ca_path.exists() {
                warn!("CA certificate path specified but file not found: {}", ca_path.display());
            }
        }

        Ok(())
    }

    /// Build a rustls ServerConfig from this TLS configuration
    pub fn build_server_config(&self) -> Result<Arc<ServerConfig>> {
        info!("Building TLS server configuration");
        
        // Validate configuration first
        self.validate()
            .context("TLS configuration validation failed")?;

        // Load certificates
        let certs = load_certificates(&self.cert_path)
            .context("Failed to load certificates")?;
        
        // Load private key
        let key = load_private_key(&self.key_path)
            .context("Failed to load private key")?;

        // Build rustls config
        let mut config = ServerConfig::builder()
            .with_safe_default_cipher_suites()
            .with_safe_default_kx_groups()
            .with_protocol_versions(match self.min_tls_version {
                TlsVersion::Tls12 => &[&rustls::version::TLS12, &rustls::version::TLS13],
                TlsVersion::Tls13 => &[&rustls::version::TLS13],
            })
            .map_err(|e| TlsError::ConfigError(e.to_string()))?
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| TlsError::ConfigError(e.to_string()))?;

        // Configure ALPN protocols (HTTP/2 and HTTP/1.1)
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        info!("TLS server configuration built successfully");
        debug!("TLS version: {:?}", self.min_tls_version);
        debug!("HSTS enabled: {}", self.enable_hsts);

        Ok(Arc::new(config))
    }

    /// Get the HSTS header value
    pub fn hsts_header(&self) -> Option<String> {
        if !self.enable_hsts {
            return None;
        }

        let mut header = format!("max-age={}", self.hsts_max_age);
        
        if self.hsts_include_subdomains {
            header.push_str("; includeSubDomains");
        }
        
        if self.hsts_preload {
            header.push_str("; preload");
        }

        Some(header)
    }
}

/// Load certificates from a PEM file
fn load_certificates(path: &Path) -> Result<Vec<Certificate>, TlsError> {
    let file = File::open(path)
        .map_err(|e| TlsError::CertificateLoadError(e.to_string()))?;
    let mut reader = BufReader::new(file);
    
    let certs = certs(&mut reader)
        .map_err(|e| TlsError::InvalidCertificate(e.to_string()))?
        .into_iter()
        .map(Certificate)
        .collect();

    Ok(certs)
}

/// Load a private key from a PEM file
fn load_private_key(path: &Path) -> Result<PrivateKey, TlsError> {
    let file = File::open(path)
        .map_err(|e| TlsError::PrivateKeyLoadError(e.to_string()))?;
    let mut reader = BufReader::new(file);
    
    let keys = pkcs8_private_keys(&mut reader)
        .map_err(|e| TlsError::InvalidPrivateKey(e.to_string()))?;

    if keys.is_empty() {
        return Err(TlsError::InvalidPrivateKey(
            "No private keys found in file".to_string(),
        ));
    }

    if keys.len() > 1 {
        warn!("Multiple private keys found, using the first one");
    }

    Ok(PrivateKey(keys[0].clone()))
}

/// Security headers middleware configuration
#[derive(Debug, Clone)]
pub struct SecurityHeaders {
    /// HSTS header value
    pub hsts: Option<String>,
    /// X-Content-Type-Options
    pub content_type_options: bool,
    /// X-Frame-Options
    pub frame_options: FrameOptions,
    /// X-XSS-Protection
    pub xss_protection: bool,
    /// Content-Security-Policy
    pub csp: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameOptions {
    Deny,
    SameOrigin,
    AllowFrom(/* url */),
}

impl Default for SecurityHeaders {
    fn default() -> Self {
        Self {
            hsts: None,
            content_type_options: true,
            frame_options: FrameOptions::Deny,
            xss_protection: true,
            csp: Some("default-src 'self'".to_string()),
        }
    }
}

impl SecurityHeaders {
    /// Create security headers from TLS config
    pub fn from_tls_config(tls_config: &TlsConfig) -> Self {
        Self {
            hsts: tls_config.hsts_header(),
            ..Default::default()
        }
    }

    /// Get all headers as key-value pairs
    pub fn as_headers(&self) -> Vec<(&'static str, String)> {
        let mut headers = Vec::new();

        if let Some(hsts) = &self.hsts {
            headers.push(("Strict-Transport-Security", hsts.clone()));
        }

        if self.content_type_options {
            headers.push(("X-Content-Type-Options", "nosniff".to_string()));
        }

        match self.frame_options {
            FrameOptions::Deny => {
                headers.push(("X-Frame-Options", "DENY".to_string()));
            }
            FrameOptions::SameOrigin => {
                headers.push(("X-Frame-Options", "SAMEORIGIN".to_string()));
            }
            FrameOptions::AllowFrom(_) => {
                // Note: ALLOW-FROM is deprecated, use CSP frame-ancestors instead
                warn!("X-Frame-Options ALLOW-FROM is deprecated");
            }
        }

        if self.xss_protection {
            headers.push(("X-XSS-Protection", "1; mode=block".to_string()));
        }

        if let Some(csp) = &self.csp {
            headers.push(("Content-Security-Policy", csp.clone()));
        }

        headers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_tls_config_default() {
        let config = TlsConfig::default();
        assert_eq!(config.min_tls_version, TlsVersion::Tls13);
        assert!(config.enable_hsts);
        assert_eq!(config.hsts_max_age, 31536000);
    }

    #[test]
    fn test_hsts_header_generation() {
        let config = TlsConfig::default();
        let header = config.hsts_header().unwrap();
        assert!(header.contains("max-age=31536000"));
        assert!(header.contains("includeSubDomains"));
    }

    #[test]
    fn test_security_headers() {
        let headers = SecurityHeaders::default();
        let header_vec = headers.as_headers();
        
        assert!(header_vec.iter().any(|(k, _)| *k == "X-Content-Type-Options"));
        assert!(header_vec.iter().any(|(k, _)| *k == "X-Frame-Options"));
        assert!(header_vec.iter().any(|(k, _)| *k == "Content-Security-Policy"));
    }

    #[test]
    fn test_tls_config_validation_missing_files() {
        let config = TlsConfig::new(
            PathBuf::from("/nonexistent/cert.pem"),
            PathBuf::from("/nonexistent/key.pem"),
        );
        
        assert!(config.validate().is_err());
    }
}