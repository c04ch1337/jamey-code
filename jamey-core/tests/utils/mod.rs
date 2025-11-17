use jamey_core::Memory;
use std::time::Duration;
use tokio::time::sleep;

/// Assert that two memories have equal content and metadata
pub fn assert_memories_equal(a: &Memory, b: &Memory) {
    assert_eq!(a.id, b.id, "Memory IDs don't match");
    assert_eq!(a.memory_type, b.memory_type, "Memory types don't match");
    assert_eq!(a.content, b.content, "Memory content doesn't match");
    assert_eq!(a.embedding, b.embedding, "Memory embeddings don't match");
    assert_eq!(a.metadata, b.metadata, "Memory metadata doesn't match");
}

/// Assert that two vectors of memories contain the same elements
pub fn assert_memory_vectors_equal(a: &[Memory], b: &[Memory]) {
    assert_eq!(a.len(), b.len(), "Memory vectors have different lengths");
    for (mem_a, mem_b) in a.iter().zip(b.iter()) {
        assert_memories_equal(mem_a, mem_b);
    }
}

/// Assert that a vector contains a memory with the specified ID
pub fn assert_contains_memory_id(memories: &[Memory], id: uuid::Uuid) -> bool {
    memories.iter().any(|m| m.id == id)
}

/// Assert that embeddings have the correct dimension
pub fn assert_valid_embedding(embedding: &[f32], expected_dim: usize) {
    assert_eq!(
        embedding.len(),
        expected_dim,
        "Embedding dimension mismatch. Expected {}, got {}",
        expected_dim,
        embedding.len()
    );
}

/// Wait for async condition with timeout
pub async fn wait_for_condition<F>(mut condition: F, timeout: Duration, interval: Duration) -> bool 
where
    F: FnMut() -> bool,
{
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if condition() {
            return true;
        }
        sleep(interval).await;
    }
    false
}

/// Helper to retry an async operation with backoff
pub async fn retry_with_backoff<T, E, F, Fut>(
    operation: F,
    max_retries: u32,
    initial_delay: Duration,
) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let mut current_retry = 0;
    let mut delay = initial_delay;

    loop {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(e) => {
                if current_retry >= max_retries {
                    return Err(e);
                }
                sleep(delay).await;
                current_retry += 1;
                delay *= 2; // Exponential backoff
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::TestMemories;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_memory_assertions() {
        let memories = TestMemories::default();
        assert_memories_equal(&memories.knowledge, &memories.knowledge.clone());
        assert_valid_embedding(&memories.knowledge.embedding, 1536);
    }

    #[tokio::test]
    async fn test_wait_for_condition() {
        let counter = AtomicUsize::new(0);
        
        // Condition that becomes true after a few iterations
        let result = wait_for_condition(
            || counter.fetch_add(1, Ordering::SeqCst) >= 3,
            Duration::from_millis(100),
            Duration::from_millis(10),
        )
        .await;
        
        assert!(result);
    }

    #[tokio::test]
    async fn test_retry_with_backoff() {
        let counter = AtomicUsize::new(0);
        
        // Operation that succeeds on the third try
        let result = retry_with_backoff(
            || async {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err("Not ready")
                } else {
                    Ok(count)
                }
            },
            5,
            Duration::from_millis(10),
        )
        .await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2);
    }
}