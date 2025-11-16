use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("Database error: {0}")]
    Database(#[from] tokio_postgres::Error),
    #[error("Pool error: {0}")]
    Pool(#[from] deadpool_postgres::PoolError),
    #[error("Vector dimension mismatch: expected {expected}, got {actual}")]
    VectorDimension { expected: usize, actual: usize },
    #[error("Memory not found: {0}")]
    NotFound(Uuid),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryType {
    Conversation,
    Knowledge,
    Experience,
    Skill,
    Preference,
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryType::Conversation => write!(f, "Conversation"),
            MemoryType::Knowledge => write!(f, "Knowledge"),
            MemoryType::Experience => write!(f, "Experience"),
            MemoryType::Skill => write!(f, "Skill"),
            MemoryType::Preference => write!(f, "Preference"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: Uuid,
    pub memory_type: MemoryType,
    pub content: String,
    pub embedding: Vec<f32>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
}

#[async_trait]
pub trait MemoryStore {
    async fn store(&self, memory: Memory) -> Result<Uuid>;
    async fn retrieve(&self, id: Uuid) -> Result<Memory>;
    async fn search(&self, query_embedding: Vec<f32>, limit: usize) -> Result<Vec<Memory>>;
    async fn update(&self, id: Uuid, content: String, embedding: Vec<f32>) -> Result<()>;
    async fn delete(&self, id: Uuid) -> Result<()>;
}

pub struct PostgresMemoryStore {
    pool: Pool,
    vector_dim: usize,
}

impl PostgresMemoryStore {
    pub async fn new(pool: Pool, vector_dim: usize) -> Result<Self> {
        let client = pool.get().await?;
        
        // Ensure pgvector extension is installed first
        client
            .execute("CREATE EXTENSION IF NOT EXISTS vector", &[])
            .await?;
        
        // Create the memories table if it doesn't exist
        client
            .execute(
                "CREATE TABLE IF NOT EXISTS memories (
                    id UUID PRIMARY KEY,
                    memory_type TEXT NOT NULL,
                    content TEXT NOT NULL,
                    embedding vector(1536) NOT NULL,
                    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    last_accessed TIMESTAMPTZ NOT NULL DEFAULT NOW()
                )",
                &[],
            )
            .await?;

        // Create an index for vector similarity search
        client
            .execute(
                "CREATE INDEX IF NOT EXISTS memories_embedding_idx ON memories 
                 USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100)",
                &[],
            )
            .await?;

        Ok(Self { pool, vector_dim })
    }

    fn validate_vector_dimension(&self, embedding: &[f32]) -> Result<(), MemoryError> {
        if embedding.len() != self.vector_dim {
            return Err(MemoryError::VectorDimension {
                expected: self.vector_dim,
                actual: embedding.len(),
            });
        }
        Ok(())
    }
}

#[async_trait]
impl MemoryStore for PostgresMemoryStore {
    async fn store(&self, memory: Memory) -> Result<Uuid> {
        self.validate_vector_dimension(&memory.embedding)?;
        let client = self.pool.get().await?;

        let id = Uuid::new_v4();
        let memory_type_str = memory.memory_type.to_string();
        let metadata_json = serde_json::to_value(&memory.metadata)?;
        
        // Convert vector to string format for PostgreSQL
        let embedding_str = format!("[{}]", 
            memory.embedding.iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        
        client
            .execute(
                "INSERT INTO memories (id, memory_type, content, embedding, metadata)
                 VALUES ($1::uuid, $2, $3, $4::vector, $5::jsonb)",
                &[
                    &id,
                    &memory_type_str,
                    &memory.content,
                    &embedding_str,
                    &metadata_json,
                ],
            )
            .await?;

        Ok(id)
    }

    async fn retrieve(&self, id: Uuid) -> Result<Memory> {
        let client = self.pool.get().await?;

        let row = client
            .query_one(
                "UPDATE memories 
                 SET last_accessed = NOW()
                 WHERE id = $1
                 RETURNING id, memory_type, content, embedding, metadata, created_at, last_accessed",
                &[&id],
            )
            .await?;

        // Get embedding as string and parse it
        let embedding_str: String = row.get("embedding");
        let embedding: Vec<f32> = embedding_str
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .map(|s| s.trim().parse().unwrap_or(0.0))
            .collect();

        let memory_type_str: String = row.get("memory_type");
        let memory_type = match memory_type_str.as_str() {
            "Conversation" => MemoryType::Conversation,
            "Knowledge" => MemoryType::Knowledge,
            "Experience" => MemoryType::Experience,
            "Skill" => MemoryType::Skill,
            "Preference" => MemoryType::Preference,
            _ => MemoryType::Conversation,
        };

        Ok(Memory {
            id: row.get("id"),
            memory_type,
            content: row.get("content"),
            embedding,
            metadata: row.get("metadata"),
            created_at: row.get("created_at"),
            last_accessed: row.get("last_accessed"),
        })
    }

    async fn search(&self, query_embedding: Vec<f32>, limit: usize) -> Result<Vec<Memory>> {
        self.validate_vector_dimension(&query_embedding)?;
        let client = self.pool.get().await?;

        // Convert query embedding to string format
        let query_embedding_str = format!("[{}]", 
            query_embedding.iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        
        let rows = client
            .query(
                "SELECT id, memory_type, content, embedding, metadata, created_at, last_accessed,
                        embedding <=> $1::vector as distance
                 FROM memories
                 ORDER BY distance
                 LIMIT $2",
                &[&query_embedding_str, &(limit as i64)],
            )
            .await?;

        let mut memories = Vec::with_capacity(rows.len());
        for row in rows {
            // Get embedding as string and parse it
            let embedding_str: String = row.get("embedding");
            let embedding: Vec<f32> = embedding_str
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(|s| s.trim().parse().unwrap_or(0.0))
                .collect();
            
            let memory_type_str: String = row.get("memory_type");
            let memory_type = match memory_type_str.as_str() {
                "Conversation" => MemoryType::Conversation,
                "Knowledge" => MemoryType::Knowledge,
                "Experience" => MemoryType::Experience,
                "Skill" => MemoryType::Skill,
                "Preference" => MemoryType::Preference,
                _ => MemoryType::Conversation,
            };
            
            memories.push(Memory {
                id: row.get("id"),
                memory_type,
                content: row.get("content"),
                embedding,
                metadata: row.get("metadata"),
                created_at: row.get("created_at"),
                last_accessed: row.get("last_accessed"),
            });
        }

        Ok(memories)
    }

    async fn update(&self, id: Uuid, content: String, embedding: Vec<f32>) -> Result<()> {
        self.validate_vector_dimension(&embedding)?;
        let client = self.pool.get().await?;

        // Convert embedding to string format
        let embedding_str = format!("[{}]", 
            embedding.iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        
        let rows_affected = client
            .execute(
                "UPDATE memories 
                 SET content = $2, embedding = $3::vector, last_accessed = NOW()
                 WHERE id = $1",
                &[&id, &content, &embedding_str],
            )
            .await?;

        if rows_affected == 0 {
            return Err(MemoryError::NotFound(id).into());
        }

        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        let client = self.pool.get().await?;

        let rows_affected = client
            .execute("DELETE FROM memories WHERE id = $1", &[&id])
            .await?;

        if rows_affected == 0 {
            return Err(MemoryError::NotFound(id).into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadpool_postgres::{Config, Runtime};
    use tokio_postgres::NoTls;

    async fn create_test_pool() -> Pool {
        let mut cfg = Config::new();
        cfg.host = Some("localhost".to_string());
        cfg.dbname = Some("jamey_test".to_string());
        cfg.user = Some("jamey".to_string());
        cfg.password = Some("test_password".to_string());
        cfg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap()
    }

    #[tokio::test]
    async fn test_memory_store() {
        let pool = create_test_pool().await;
        let store = PostgresMemoryStore::new(pool, 1536).await.unwrap();

        // Test storing and retrieving a memory
        let memory = Memory {
            id: Uuid::new_v4(),
            memory_type: MemoryType::Conversation,
            content: "Test memory".to_string(),
            embedding: vec![0.0; 1536],
            metadata: serde_json::json!({"test": true}),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        };

        let id = store.store(memory.clone()).await.unwrap();
        let retrieved = store.retrieve(id).await.unwrap();

        assert_eq!(retrieved.content, memory.content);
        assert_eq!(retrieved.embedding.len(), memory.embedding.len());

        // Test searching
        let results = store.search(vec![0.0; 1536], 1).await.unwrap();
        assert!(!results.is_empty());

        // Test updating
        store
            .update(id, "Updated content".to_string(), vec![0.0; 1536])
            .await
            .unwrap();

        // Test deleting
        store.delete(id).await.unwrap();
    }
}