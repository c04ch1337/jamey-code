use jamey_core::{Memory, MemoryType};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

pub struct TestMemories {
    pub knowledge: Memory,
    pub conversation: Memory,
    pub system: Memory,
}

impl Default for TestMemories {
    fn default() -> Self {
        Self {
            knowledge: Memory {
                id: Uuid::new_v4(),
                memory_type: MemoryType::Knowledge,
                content: "Test knowledge memory".to_string(),
                embedding: vec![0.1; 1536],
                metadata: json!({
                    "source": "test",
                    "category": "knowledge",
                    "tags": ["test", "knowledge"]
                }),
                created_at: Utc::now(),
                last_accessed: Utc::now(),
            },
            conversation: Memory {
                id: Uuid::new_v4(),
                memory_type: MemoryType::Conversation,
                content: "Test conversation memory".to_string(),
                embedding: vec![0.2; 1536],
                metadata: json!({
                    "source": "test",
                    "category": "conversation",
                    "participants": ["user", "assistant"]
                }),
                created_at: Utc::now(),
                last_accessed: Utc::now(),
            },
            system: Memory {
                id: Uuid::new_v4(),
                memory_type: MemoryType::System,
                content: "Test system memory".to_string(),
                embedding: vec![0.3; 1536],
                metadata: json!({
                    "source": "test",
                    "category": "system",
                    "priority": "high"
                }),
                created_at: Utc::now(),
                last_accessed: Utc::now(),
            },
        }
    }
}

pub fn test_embeddings() -> Vec<Vec<f32>> {
    vec![
        vec![0.1; 1536], // Knowledge embedding
        vec![0.2; 1536], // Conversation embedding
        vec![0.3; 1536], // System embedding
        vec![0.5; 1536], // Random test embedding
    ]
}

pub fn test_metadata() -> serde_json::Value {
    json!({
        "test": true,
        "timestamp": Utc::now().to_rfc3339(),
        "tags": ["test", "fixture"],
        "nested": {
            "field1": "value1",
            "field2": 42,
            "field3": true
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memories_creation() {
        let memories = TestMemories::default();
        assert_eq!(memories.knowledge.memory_type, MemoryType::Knowledge);
        assert_eq!(memories.conversation.memory_type, MemoryType::Conversation);
        assert_eq!(memories.system.memory_type, MemoryType::System);
    }

    #[test]
    fn test_embeddings_dimensions() {
        let embeddings = test_embeddings();
        for embedding in embeddings {
            assert_eq!(embedding.len(), 1536);
        }
    }

    #[test]
    fn test_metadata_structure() {
        let metadata = test_metadata();
        assert!(metadata["test"].as_bool().unwrap());
        assert!(metadata["tags"].as_array().unwrap().len() == 2);
        assert!(metadata["nested"]["field2"].as_i64().unwrap() == 42);
    }
}