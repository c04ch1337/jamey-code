use async_trait::async_trait;
use jamey_core::{Memory, MemoryStore};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Default)]
pub struct MockMemoryStore {
    memories: Arc<RwLock<HashMap<Uuid, Memory>>>,
}

impl MockMemoryStore {
    pub fn new() -> Self {
        Self {
            memories: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_all_memories(&self) -> Vec<Memory> {
        self.memories.read().await.values().cloned().collect()
    }
}

#[async_trait]
impl MemoryStore for MockMemoryStore {
    async fn store(&self, memory: Memory) -> anyhow::Result<Uuid> {
        let id = memory.id;
        self.memories.write().await.insert(id, memory);
        Ok(id)
    }

    async fn retrieve(&self, id: Uuid) -> anyhow::Result<Memory> {
        self.memories
            .read()
            .await
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Memory not found"))
    }

    async fn update(&self, id: Uuid, content: String, embedding: Vec<f32>) -> anyhow::Result<()> {
        let mut memories = self.memories.write().await;
        if let Some(memory) = memories.get_mut(&id) {
            memory.content = content;
            memory.embedding = embedding;
            memory.last_accessed = chrono::Utc::now();
            Ok(())
        } else {
            Err(anyhow::anyhow!("Memory not found"))
        }
    }

    async fn delete(&self, id: Uuid) -> anyhow::Result<()> {
        self.memories.write().await.remove(&id);
        Ok(())
    }

    async fn search(&self, embedding: Vec<f32>, limit: usize) -> anyhow::Result<Vec<Memory>> {
        let memories = self.memories.read().await;
        let mut results: Vec<(f32, Memory)> = memories
            .values()
            .map(|memory| {
                let similarity = cosine_similarity(&embedding, &memory.embedding);
                (similarity, memory.clone())
            })
            .collect();

        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        Ok(results.into_iter().take(limit).map(|(_, m)| m).collect())
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot_product / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::TestMemories;

    #[tokio::test]
    async fn test_mock_memory_store() {
        let store = MockMemoryStore::new();
        let memories = TestMemories::default();

        // Test store
        let id = store.store(memories.knowledge.clone()).await.unwrap();
        assert_eq!(store.get_all_memories().await.len(), 1);

        // Test retrieve
        let retrieved = store.retrieve(id).await.unwrap();
        assert_eq!(retrieved.content, memories.knowledge.content);

        // Test update
        store
            .update(id, "Updated content".to_string(), vec![0.1; 1536])
            .await
            .unwrap();
        let updated = store.retrieve(id).await.unwrap();
        assert_eq!(updated.content, "Updated content");

        // Test search
        let search_results = store.search(vec![0.1; 1536], 10).await.unwrap();
        assert!(!search_results.is_empty());

        // Test delete
        store.delete(id).await.unwrap();
        assert!(store.retrieve(id).await.is_err());
    }
}