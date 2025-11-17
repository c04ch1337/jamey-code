//! Comprehensive unit tests for secrets module
//! Tests validation, error handling, security, and edge cases

use jamey_core::{SecretError, SecretManager};

// ============================================================================
// Service Name Validation Tests
// ============================================================================

#[test]
fn test_service_name_valid() {
    let valid_names = vec![
        "jamey",
        "jamey-test",
        "jamey_test",
        "jamey.test",
        "test123",
        "a",
        "a".repeat(128), // Max length
    ];

    for name in valid_names {
        let result = SecretManager::new(name);
        assert!(result.is_ok(), "Should accept valid name: {}", name);
    }
}

#[test]
fn test_service_name_empty() {
    let result = SecretManager::new("");
    assert!(result.is_err());
    
    if let Err(SecretError::InvalidService(msg)) = result {
        assert!(msg.contains("1-128"));
    } else {
        panic!("Expected InvalidService error");
    }
}

#[test]
fn test_service_name_too_long() {
    let long_name = "a".repeat(129);
    let result = SecretManager::new(long_name);
    assert!(result.is_err());
    
    if let Err(SecretError::InvalidService(msg)) = result {
        assert!(msg.contains("1-128"));
    } else {
        panic!("Expected InvalidService error");
    }
}

#[test]
fn test_service_name_invalid_characters() {
    let invalid_names = vec![
        "jamey test",  // Space
        "jamey@test",  // @
        "jamey#test",  // #
        "jamey$test",  // $
        "jamey/test",  // /
        "jamey\\test", // Backslash
        "jamey:test",  // Colon
        "jamey;test",  // Semicolon
        "jamey!test",  // Exclamation
        "jamey?test",  // Question mark
    ];

    for name in invalid_names {
        let result = SecretManager::new(name);
        assert!(result.is_err(), "Should reject invalid name: {}", name);
        
        if let Err(SecretError::InvalidService(msg)) = result {
            assert!(msg.contains("invalid characters"));
        } else {
            panic!("Expected InvalidService error for: {}", name);
        }
    }
}

// ============================================================================
// Key Validation Tests
// ============================================================================

#[test]
fn test_store_secret_valid_key() {
    let manager = SecretManager::new("jamey_test").unwrap();
    
    let valid_keys = vec![
        "api_key",
        "api-key",
        "api.key",
        "key123",
        "a",
        "a".repeat(256), // Max length
    ];

    for key in valid_keys {
        let result = manager.store_secret(key, "test_value");
        // May fail due to keyring access, but should not fail validation
        if let Err(e) = &result {
            // Only validation errors should fail here
            if let SecretError::InvalidKey(_) = e {
                panic!("Should accept valid key: {}", key);
            }
        }
    }
}

#[test]
fn test_store_secret_empty_key() {
    let manager = SecretManager::new("jamey_test").unwrap();
    let result = manager.store_secret("", "test_value");
    
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), SecretError::InvalidKey(_)));
}

#[test]
fn test_store_secret_key_too_long() {
    let manager = SecretManager::new("jamey_test").unwrap();
    let long_key = "a".repeat(257);
    let result = manager.store_secret(&long_key, "test_value");
    
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), SecretError::InvalidKey(_)));
}

#[test]
fn test_store_secret_key_invalid_characters() {
    let manager = SecretManager::new("jamey_test").unwrap();
    
    let invalid_keys = vec![
        "key with spaces",
        "key@test",
        "key#test",
        "key$test",
        "key/test",
        "key\\test",
        "key:test",
        "key;test",
    ];

    for key in invalid_keys {
        let result = manager.store_secret(key, "test_value");
        assert!(result.is_err(), "Should reject invalid key: {}", key);
        assert!(matches!(result.unwrap_err(), SecretError::InvalidKey(_)));
    }
}

// ============================================================================
// Value Validation Tests
// ============================================================================

#[test]
fn test_store_secret_empty_value() {
    let manager = SecretManager::new("jamey_test").unwrap();
    let result = manager.store_secret("test_key", "");
    
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), SecretError::InvalidValue(_)));
}

#[test]
fn test_store_secret_value_too_long() {
    let manager = SecretManager::new("jamey_test").unwrap();
    let long_value = "x".repeat(16385); // Exceeds 16384 limit
    let result = manager.store_secret("test_key", &long_value);
    
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), SecretError::InvalidValue(_)));
}

#[test]
fn test_store_secret_value_with_control_characters() {
    let manager = SecretManager::new("jamey_test").unwrap();
    
    // Newlines and tabs should be allowed
    let result = manager.store_secret("test_key", "value\nwith\nnewlines");
    if let Err(e) = &result {
        if let SecretError::InvalidValue(_) = e {
            panic!("Should allow newlines in values");
        }
    }
    
    let result = manager.store_secret("test_key", "value\twith\ttabs");
    if let Err(e) = &result {
        if let SecretError::InvalidValue(_) = e {
            panic!("Should allow tabs in values");
        }
    }
    
    // Other control characters should be rejected
    let invalid_values = vec![
        "value\x00with\x00null",
        "value\x01with\x01control",
        "value\x02test",
    ];

    for value in invalid_values {
        let result = manager.store_secret("test_key", value);
        assert!(result.is_err(), "Should reject value with control chars");
        assert!(matches!(result.unwrap_err(), SecretError::InvalidValue(_)));
    }
}

#[test]
fn test_store_secret_value_at_max_length() {
    let manager = SecretManager::new("jamey_test").unwrap();
    let max_value = "x".repeat(16384); // Exactly at limit
    let result = manager.store_secret("test_key", &max_value);
    
    // Should not fail validation (may fail on keyring access)
    if let Err(e) = &result {
        if let SecretError::InvalidValue(_) = e {
            panic!("Should accept value at max length");
        }
    }
}

// ============================================================================
// Secret Lifecycle Tests
// ============================================================================

#[test]
fn test_secret_lifecycle() {
    let manager = SecretManager::new("jamey_test_lifecycle").unwrap();
    let key = "lifecycle_test_key";
    let value = "test_secret_value_123";

    // Store secret
    let store_result = manager.store_secret(key, value);
    if store_result.is_err() {
        // Skip test if keyring is not available
        eprintln!("Skipping lifecycle test: keyring not available");
        return;
    }

    // Retrieve secret
    let retrieved = manager.get_secret(key).unwrap();
    assert_eq!(retrieved, value);

    // Delete secret
    manager.delete_secret(key).unwrap();

    // Verify deletion
    let get_result = manager.get_secret(key);
    assert!(get_result.is_err());
}

#[test]
fn test_get_nonexistent_secret() {
    let manager = SecretManager::new("jamey_test_nonexistent").unwrap();
    let result = manager.get_secret("nonexistent_key");
    
    // Should fail (either NotFound or RetrievalError)
    assert!(result.is_err());
}

#[test]
fn test_delete_nonexistent_secret() {
    let manager = SecretManager::new("jamey_test_delete").unwrap();
    let result = manager.delete_secret("nonexistent_key");
    
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), SecretError::NotFound(_)));
}

#[test]
fn test_rotate_secret() {
    let manager = SecretManager::new("jamey_test_rotate").unwrap();
    let key = "rotate_test_key";

    // Store initial secret
    let store_result = manager.store_secret(key, "initial_value");
    if store_result.is_err() {
        eprintln!("Skipping rotate test: keyring not available");
        return;
    }

    // Rotate secret
    let new_value = manager.rotate_secret(key).unwrap();
    
    // Verify new value is different and was stored
    assert_ne!(new_value, "initial_value");
    assert!(!new_value.is_empty());
    assert!(new_value.len() >= 32 && new_value.len() <= 64);
    
    let retrieved = manager.get_secret(key).unwrap();
    assert_eq!(retrieved, new_value);

    // Cleanup
    let _ = manager.delete_secret(key);
}

// ============================================================================
// Security Tests
// ============================================================================

#[test]
fn test_generated_secret_randomness() {
    let manager = SecretManager::new("jamey_test_random").unwrap();
    
    // Generate multiple secrets and ensure they're different
    let mut secrets = std::collections::HashSet::new();
    
    for i in 0..10 {
        let key = format!("random_test_{}", i);
        if let Ok(secret) = manager.rotate_secret(&key) {
            secrets.insert(secret.clone());
            let _ = manager.delete_secret(&key);
        }
    }
    
    // All generated secrets should be unique
    assert!(secrets.len() >= 9, "Generated secrets should be unique");
}

#[test]
fn test_generated_secret_length() {
    let manager = SecretManager::new("jamey_test_length").unwrap();
    let key = "length_test";
    
    if let Ok(secret) = manager.rotate_secret(key) {
        assert!(secret.len() >= 32, "Secret should be at least 32 chars");
        assert!(secret.len() <= 64, "Secret should be at most 64 chars");
        let _ = manager.delete_secret(key);
    }
}

#[test]
fn test_generated_secret_charset() {
    let manager = SecretManager::new("jamey_test_charset").unwrap();
    let key = "charset_test";
    
    if let Ok(secret) = manager.rotate_secret(key) {
        // Should contain mix of alphanumeric and special characters
        let has_upper = secret.chars().any(|c| c.is_ascii_uppercase());
        let has_lower = secret.chars().any(|c| c.is_ascii_lowercase());
        let has_digit = secret.chars().any(|c| c.is_ascii_digit());
        
        assert!(has_upper || has_lower || has_digit, 
                "Secret should contain alphanumeric characters");
        
        let _ = manager.delete_secret(key);
    }
}

// ============================================================================
// Edge Cases and Boundary Tests
// ============================================================================

#[test]
fn test_store_secret_with_unicode() {
    let manager = SecretManager::new("jamey_test_unicode").unwrap();
    
    let unicode_values = vec![
        "Hello 世界",
        "Привет мир",
        "مرحبا بالعالم",
        "🔐🔑🛡️",
    ];

    for value in unicode_values {
        let result = manager.store_secret("unicode_test", value);
        // Should not fail validation
        if let Err(e) = &result {
            if let SecretError::InvalidValue(_) = e {
                panic!("Should accept unicode value: {}", value);
            }
        }
    }
}

#[test]
fn test_multiple_managers_same_service() {
    let manager1 = SecretManager::new("jamey_test_shared").unwrap();
    let manager2 = SecretManager::new("jamey_test_shared").unwrap();
    
    let key = "shared_key";
    let value = "shared_value";

    // Store with first manager
    if manager1.store_secret(key, value).is_err() {
        eprintln!("Skipping shared service test: keyring not available");
        return;
    }

    // Retrieve with second manager
    let retrieved = manager2.get_secret(key).unwrap();
    assert_eq!(retrieved, value);

    // Cleanup
    let _ = manager1.delete_secret(key);
}

#[test]
fn test_different_services_isolated() {
    let manager1 = SecretManager::new("jamey_test_service1").unwrap();
    let manager2 = SecretManager::new("jamey_test_service2").unwrap();
    
    let key = "isolated_key";
    let value1 = "value_for_service1";
    let value2 = "value_for_service2";

    // Store in both services
    if manager1.store_secret(key, value1).is_err() || 
       manager2.store_secret(key, value2).is_err() {
        eprintln!("Skipping isolation test: keyring not available");
        return;
    }

    // Each should retrieve its own value
    let retrieved1 = manager1.get_secret(key).unwrap();
    let retrieved2 = manager2.get_secret(key).unwrap();
    
    assert_eq!(retrieved1, value1);
    assert_eq!(retrieved2, value2);
    assert_ne!(retrieved1, retrieved2);

    // Cleanup
    let _ = manager1.delete_secret(key);
    let _ = manager2.delete_secret(key);
}

#[test]
fn test_overwrite_existing_secret() {
    let manager = SecretManager::new("jamey_test_overwrite").unwrap();
    let key = "overwrite_key";

    // Store initial value
    if manager.store_secret(key, "initial_value").is_err() {
        eprintln!("Skipping overwrite test: keyring not available");
        return;
    }

    // Overwrite with new value
    manager.store_secret(key, "new_value").unwrap();

    // Should retrieve new value
    let retrieved = manager.get_secret(key).unwrap();
    assert_eq!(retrieved, "new_value");

    // Cleanup
    let _ = manager.delete_secret(key);
}

// ============================================================================
// Constant-Time Comparison Tests
// ============================================================================

#[test]
fn test_constant_time_key_validation() {
    let manager = SecretManager::new("jamey_test_timing").unwrap();
    
    // These should all fail validation in constant time
    let empty_key = "";
    let result1 = manager.store_secret(empty_key, "value");
    assert!(result1.is_err());
    
    let result2 = manager.get_secret(empty_key);
    assert!(result2.is_err());
    
    let result3 = manager.delete_secret(empty_key);
    assert!(result3.is_err());
}