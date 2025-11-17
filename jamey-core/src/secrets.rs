use keyring::Entry;
use thiserror::Error;
use subtle::ConstantTimeEq;

/// Errors that can occur during secret management operations
#[derive(Error, Debug)]
pub enum SecretError {
    #[error("Failed to store secret: {0}")]
    StoreError(#[from] keyring::Error),
    #[error("Failed to retrieve secret: {0}")]
    RetrievalError(#[from] Box<keyring::Error>),
    #[error("Secret not found for key: {0}")]
    NotFound(String),
    #[error("Invalid key format: {0}")]
    InvalidKey(String),
    #[error("Invalid secret value: {0}")]
    InvalidValue(String),
    #[error("Service name validation failed: {0}")]
    InvalidService(String),
}

const MAX_KEY_LENGTH: usize = 256;
const MAX_VALUE_LENGTH: usize = 16384;
const MAX_SERVICE_LENGTH: usize = 128;

/// Manages secure storage and retrieval of sensitive information
pub struct SecretManager {
    service_name: String,
}

impl SecretManager {
    /// Creates a new SecretManager instance with validation
    pub fn new(service_name: impl Into<String>) -> Result<Self, SecretError> {
        let service_name = service_name.into();
        Self::validate_service_name(&service_name)?;
        
        Ok(Self { service_name })
    }

    /// Validates a service name
    fn validate_service_name(name: &str) -> Result<(), SecretError> {
        if name.is_empty() || name.len() > MAX_SERVICE_LENGTH {
            return Err(SecretError::InvalidService(
                format!("Service name must be 1-{} characters", MAX_SERVICE_LENGTH)
            ));
        }

        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.') {
            return Err(SecretError::InvalidService(
                "Service name contains invalid characters".to_string()
            ));
        }

        Ok(())
    }

    /// Validates a secret key
    fn validate_key(key: &str) -> Result<(), SecretError> {
        if key.is_empty() || key.len() > MAX_KEY_LENGTH {
            return Err(SecretError::InvalidKey(
                format!("Key must be 1-{} characters", MAX_KEY_LENGTH)
            ));
        }

        if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.') {
            return Err(SecretError::InvalidKey(
                "Key contains invalid characters".to_string()
            ));
        }

        Ok(())
    }

    /// Validates a secret value
    fn validate_value(value: &str) -> Result<(), SecretError> {
        if value.is_empty() {
            return Err(SecretError::InvalidValue("Value cannot be empty".to_string()));
        }

        if value.len() > MAX_VALUE_LENGTH {
            return Err(SecretError::InvalidValue(
                format!("Value exceeds maximum length of {}", MAX_VALUE_LENGTH)
            ));
        }

        if value.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
            return Err(SecretError::InvalidValue(
                "Value contains invalid control characters".to_string()
            ));
        }

        Ok(())
    }

    /// Stores a secret in the system keyring
    /// 
    /// # Arguments
    /// * `key` - Unique identifier for the secret
    /// * `value` - The secret value to store
    pub fn store_secret(&self, key: &str, value: &str) -> Result<(), SecretError> {
        Self::validate_key(key)?;
        Self::validate_value(value)?;

        // Use constant-time comparison for key validation
        use subtle::ConstantTimeEq;
        if key.as_bytes().ct_eq("".as_bytes()).into() {
            return Err(SecretError::InvalidKey("Empty key".to_string()));
        }

        let entry = Entry::new(&self.service_name, key)?;
        
        // Securely store the secret
        entry.set_password(value)?;
        
        Ok(())
    }

    /// Retrieves a secret from the system keyring
    /// 
    /// # Arguments
    /// * `key` - Unique identifier for the secret
    pub fn get_secret(&self, key: &str) -> Result<String, SecretError> {
        Self::validate_key(key)?;

        // Use constant-time comparison for key validation
        use subtle::ConstantTimeEq;
        if key.as_bytes().ct_eq("".as_bytes()).into() {
            return Err(SecretError::InvalidKey("Empty key".to_string()));
        }

        let entry = Entry::new(&self.service_name, key)?;
        let value = entry.get_password()
            .map_err(|e| SecretError::RetrievalError(Box::new(e)))?;

        // Validate retrieved value
        Self::validate_value(&value)?;
        
        Ok(value)
    }

    /// Deletes a secret from the system keyring
    /// 
    /// # Arguments
    /// * `key` - Unique identifier for the secret to delete
    pub fn delete_secret(&self, key: &str) -> Result<(), SecretError> {
        Self::validate_key(key)?;

        // Use constant-time comparison for key validation
        use subtle::ConstantTimeEq;
        if key.as_bytes().ct_eq("".as_bytes()).into() {
            return Err(SecretError::InvalidKey("Empty key".to_string()));
        }

        let entry = Entry::new(&self.service_name, key)?;
        match entry.delete_password() {
            Ok(_) => Ok(()),
            Err(keyring::Error::NoEntry) => Err(SecretError::NotFound(key.to_string())),
            Err(e) => Err(SecretError::StoreError(e)),
        }
    }

    /// Rotates a secret by generating a new value and storing it
    pub fn rotate_secret(&self, key: &str) -> Result<String, SecretError> {
        Self::validate_key(key)?;

        // Generate new secure random value
        let new_value = generate_secure_secret();
        self.store_secret(key, &new_value)?;
        
        Ok(new_value)
    }
}

/// Generates a cryptographically secure random secret
fn generate_secure_secret() -> String {
    use rand::{thread_rng, Rng};
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()-_=+[]{}|;:,.<>?";
    
    let mut rng = thread_rng();
    let length = rng.gen_range(32..64); // Random length between 32-64 chars
    
    let secret: String = (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();
    
    secret
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_lifecycle() {
        let manager = SecretManager::new("jamey_test").unwrap();
        let test_key = "test_secret";
        let test_value = "super_secret_value";

        // Store secret
        assert!(manager.store_secret(test_key, test_value).is_ok());

        // Retrieve secret
        let retrieved = manager.get_secret(test_key).unwrap();
        assert_eq!(retrieved, test_value);

        // Delete secret
        assert!(manager.delete_secret(test_key).is_ok());

        // Verify deletion
        assert!(manager.get_secret(test_key).is_err());
    }
}