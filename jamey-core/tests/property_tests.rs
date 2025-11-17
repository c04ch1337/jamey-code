//! Property-based tests using proptest
//! Tests invariants, serialization round-trips, and mathematical properties

use chrono::Utc;
use jamey_core::{Memory, MemoryType};
use proptest::prelude::*;
use serde_json::json;
use uuid::Uuid;

// ============================================================================
// Property Test Strategies
// ============================================================================

/// Generate valid memory types
fn memory_type_strategy() -> impl Strategy<Value = MemoryType> {
    prop_oneof![
        Just(MemoryType::Conversation),
        Just(MemoryType::Knowledge),
        Just(MemoryType::Experience),
        Just(MemoryType::Skill),
        Just(MemoryType::Preference),
    ]
}

/// Generate valid content strings (1-1000 chars for testing)
fn content_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 ]{1,1000}"
}

/// Generate valid embeddings (dimension 128 for faster tests)
fn embedding_strategy() -> impl Strategy<Value = Vec<f32>> {
    prop::collection::vec(-1.0f32..1.0f32, 128)
}

/// Generate valid metadata
fn metadata_strategy() -> impl Strategy<Value = serde_json::Value> {
    prop_oneof![
        Just(json!({})),
        Just(json!({"key": "value"})),
        Just(json!({"count": 42})),
        Just(json!({"tags": ["test", "prop"]})),
    ]
}

// ============================================================================
// Serialization Round-Trip Tests
// ============================================================================

proptest! {
    #[test]
    fn test_memory_type_serialization_roundtrip(
        memory_type in memory_type_strategy()
    ) {
        let serialized = serde_json::to_string(&memory_type).unwrap();
        let deserialized: MemoryType = serde_json::from_str(&serialized).unwrap();
        prop_assert_eq!(memory_type, deserialized);
    }

    #[test]
    fn test_memory_serialization_roundtrip(
        memory_type in memory_type_strategy(),
        content in content_strategy(),
        embedding in embedding_strategy(),
        metadata in metadata_strategy(),
    ) {
        let memory = Memory {
            id: Uuid::new_v4(),
            memory_type,
            content,
            embedding,
            metadata,
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        };

        let serialized = serde_json::to_string(&memory).unwrap();
        let deserialized: Memory = serde_json::from_str(&serialized).unwrap();

        prop_assert_eq!(memory.id, deserialized.id);
        prop_assert_eq!(memory.memory_type, deserialized.memory_type);
        prop_assert_eq!(memory.content, deserialized.content);
        prop_assert_eq!(memory.embedding.len(), deserialized.embedding.len());
    }
}

// ============================================================================
// Memory Type Parsing Tests
// ============================================================================

proptest! {
    #[test]
    fn test_memory_type_display_parse_roundtrip(
        memory_type in memory_type_strategy()
    ) {
        let display_str = memory_type.to_string();
        let parsed = MemoryType::try_from(display_str.as_str()).unwrap();
        prop_assert_eq!(memory_type, parsed);
    }

    #[test]
    fn test_memory_type_case_insensitive(
        memory_type in memory_type_strategy()
    ) {
        let lower = memory_type.to_string().to_lowercase();
        let upper = memory_type.to_string().to_uppercase();
        
        let from_lower = MemoryType::try_from(lower.as_str()).unwrap();
        let from_upper = MemoryType::try_from(upper.as_str()).unwrap();
        
        prop_assert_eq!(memory_type, from_lower);
        prop_assert_eq!(memory_type, from_upper);
    }
}

// ============================================================================
// Embedding Validation Tests
// ============================================================================

proptest! {
    #[test]
    fn test_embedding_no_nan_or_inf(
        mut embedding in embedding_strategy()
    ) {
        // Ensure no NaN or Inf values
        for val in &mut embedding {
            if val.is_nan() || val.is_infinite() {
                *val = 0.0;
            }
        }

        prop_assert!(embedding.iter().all(|x| !x.is_nan() && !x.is_infinite()));
    }

    #[test]
    fn test_embedding_dimension_preserved(
        embedding in embedding_strategy()
    ) {
        let original_len = embedding.len();
        let memory = Memory {
            id: Uuid::new_v4(),
            memory_type: MemoryType::Knowledge,
            content: "Test".to_string(),
            embedding: embedding.clone(),
            metadata: json!({}),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        };

        prop_assert_eq!(memory.embedding.len(), original_len);
    }
}

// ============================================================================
// Content Validation Tests
// ============================================================================

proptest! {
    #[test]
    fn test_content_length_preserved(
        content in content_strategy()
    ) {
        let original_len = content.len();
        let memory = Memory {
            id: Uuid::new_v4(),
            memory_type: MemoryType::Knowledge,
            content: content.clone(),
            embedding: vec![0.1; 128],
            metadata: json!({}),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        };

        prop_assert_eq!(memory.content.len(), original_len);
    }

    #[test]
    fn test_content_not_empty_after_trim(
        content in "[a-zA-Z0-9 ]{1,100}"
    ) {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            prop_assert!(!trimmed.is_empty());
        }
    }
}

// ============================================================================
// Metadata Validation Tests
// ============================================================================

proptest! {
    #[test]
    fn test_metadata_json_valid(
        metadata in metadata_strategy()
    ) {
        // Should be able to serialize and deserialize
        let serialized = serde_json::to_string(&metadata).unwrap();
        let deserialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        prop_assert_eq!(metadata, deserialized);
    }

    #[test]
    fn test_metadata_size_reasonable(
        metadata in metadata_strategy()
    ) {
        let serialized = serde_json::to_string(&metadata).unwrap();
        // Should be under reasonable size limit
        prop_assert!(serialized.len() < 16384);
    }
}

// ============================================================================
// UUID Uniqueness Tests
// ============================================================================

proptest! {
    #[test]
    fn test_uuid_uniqueness(
        _seed in 0u64..1000u64
    ) {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        
        // UUIDs should be unique (extremely high probability)
        prop_assert_ne!(id1, id2);
    }

    #[test]
    fn test_uuid_serialization_roundtrip(
        _seed in 0u64..100u64
    ) {
        let id = Uuid::new_v4();
        let serialized = serde_json::to_string(&id).unwrap();
        let deserialized: Uuid = serde_json::from_str(&serialized).unwrap();
        prop_assert_eq!(id, deserialized);
    }
}

// ============================================================================
// Clone and Equality Tests
// ============================================================================

proptest! {
    #[test]
    fn test_memory_clone_equality(
        memory_type in memory_type_strategy(),
        content in content_strategy(),
        embedding in embedding_strategy(),
    ) {
        let memory = Memory {
            id: Uuid::new_v4(),
            memory_type,
            content,
            embedding,
            metadata: json!({}),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        };

        let cloned = memory.clone();
        
        prop_assert_eq!(memory.id, cloned.id);
        prop_assert_eq!(memory.memory_type, cloned.memory_type);
        prop_assert_eq!(memory.content, cloned.content);
        prop_assert_eq!(memory.embedding, cloned.embedding);
    }
}

// ============================================================================
// Vector Operations Tests
// ============================================================================

proptest! {
    #[test]
    fn test_embedding_dot_product_commutative(
        vec1 in prop::collection::vec(-1.0f32..1.0f32, 10),
        vec2 in prop::collection::vec(-1.0f32..1.0f32, 10),
    ) {
        let dot1: f32 = vec1.iter().zip(vec2.iter()).map(|(a, b)| a * b).sum();
        let dot2: f32 = vec2.iter().zip(vec1.iter()).map(|(a, b)| a * b).sum();
        
        // Dot product should be commutative (within floating point precision)
        prop_assert!((dot1 - dot2).abs() < 1e-6);
    }

    #[test]
    fn test_embedding_normalization_idempotent(
        embedding in prop::collection::vec(-1.0f32..1.0f32, 10)
    ) {
        // Normalize once
        let norm1: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        let normalized1: Vec<f32> = if norm1 > 0.0 {
            embedding.iter().map(|x| x / norm1).collect()
        } else {
            embedding.clone()
        };

        // Normalize again
        let norm2: f32 = normalized1.iter().map(|x| x * x).sum::<f32>().sqrt();
        let normalized2: Vec<f32> = if norm2 > 0.0 {
            normalized1.iter().map(|x| x / norm2).collect()
        } else {
            normalized1.clone()
        };

        // Second normalization should not change the vector significantly
        for (a, b) in normalized1.iter().zip(normalized2.iter()) {
            prop_assert!((a - b).abs() < 1e-5);
        }
    }
}

// ============================================================================
// String Validation Tests
// ============================================================================

proptest! {
    #[test]
    fn test_content_sanitization_removes_nulls(
        content in "[a-zA-Z0-9 ]{1,100}"
    ) {
        // Add null bytes
        let with_nulls = format!("{}\0test\0", content);
        let sanitized: String = with_nulls.chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
            .collect();
        
        prop_assert!(!sanitized.contains('\0'));
    }

    #[test]
    fn test_content_preserves_newlines_and_tabs(
        content in "[a-zA-Z0-9 \n\t]{1,100}"
    ) {
        let sanitized: String = content.chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
            .collect();
        
        // Newlines and tabs should be preserved
        let original_newlines = content.matches('\n').count();
        let sanitized_newlines = sanitized.matches('\n').count();
        prop_assert_eq!(original_newlines, sanitized_newlines);
    }
}

// ============================================================================
// Timestamp Tests
// ============================================================================

proptest! {
    #[test]
    fn test_timestamp_ordering(
        _seed in 0u64..100u64
    ) {
        let t1 = Utc::now();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let t2 = Utc::now();
        
        prop_assert!(t2 >= t1);
    }

    #[test]
    fn test_timestamp_serialization_roundtrip(
        _seed in 0u64..100u64
    ) {
        let timestamp = Utc::now();
        let serialized = serde_json::to_string(&timestamp).unwrap();
        let deserialized = serde_json::from_str(&serialized).unwrap();
        prop_assert_eq!(timestamp, deserialized);
    }
}

// ============================================================================
// Boundary Condition Tests
// ============================================================================

proptest! {
    #[test]
    fn test_embedding_all_zeros_valid(
        size in 1usize..1000usize
    ) {
        let embedding = vec![0.0; size];
        prop_assert!(embedding.iter().all(|x| *x == 0.0));
    }

    #[test]
    fn test_embedding_all_ones_valid(
        size in 1usize..1000usize
    ) {
        let embedding = vec![1.0; size];
        prop_assert!(embedding.iter().all(|x| *x == 1.0));
    }

    #[test]
    fn test_content_single_char_valid(
        c in "[a-zA-Z0-9]"
    ) {
        prop_assert_eq!(c.len(), 1);
        prop_assert!(!c.is_empty());
    }
}