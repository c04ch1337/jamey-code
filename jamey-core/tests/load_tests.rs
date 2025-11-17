mod fixtures;
mod helpers;
mod mocks;
mod utils;

use std::time::{Duration, Instant};
use tokio::time;
use futures::future::join_all;

use helpers::TestContext;
use utils::{assert_memories_equal, retry_with_backoff};


struct LoadTestMetrics {
    total_requests: usize,
    successful_requests: usize,
    failed_requests: usize,
    min_latency: Duration,
    max_latency: Duration,
    avg_latency: Duration,
    throughput: f64,
}

async fn run_concurrent_operations(
    store: &CachedMemoryStore,
    num_operations: usize,
    concurrency: usize,
) -> LoadTestMetrics {
    let start_time = Instant::now();
    let mut latencies = Vec::new();
    let mut successful_requests = 0;
    let mut failed_requests = 0;

    // Create batches of concurrent operations
    for batch_start in (0..num_operations).step_by(concurrency) {
        let batch_size = concurrency.min(num_operations - batch_start);
        let mut futures = Vec::with_capacity(batch_size);

        for _ in 0..batch_size {
            let store = store.clone();
            let memory = create_test_memory();
            
            futures.push(tokio::spawn(async move {
                let op_start = Instant::now();
                let result = async {
                    // Store memory
                    let id = store.store(memory.clone()).await?;
                    
                    // Retrieve memory
                    let _ = store.retrieve(id).await?;
                    
                    // Search for similar memories
                    let _ = store.search(memory.embedding.clone(), 5).await?;
                    
                    // Update memory
                    store.update(id, "Updated content".to_string(), memory.embedding).await?;
                    
                    // Delete memory
                    store.delete(id).await?;
                    
                    Ok::<_, anyhow::Error>(())
                }.await;

                (result, op_start.elapsed())
            }));
        }

        // Wait for batch completion
        let results = join_all(futures).await;
        
        for result in results {
            match result {
                Ok((Ok(_), latency)) => {
                    successful_requests += 1;
                    latencies.push(latency);
                }
                _ => failed_requests += 1,
            }
        }

        // Add small delay between batches to prevent overwhelming the system
        time::sleep(Duration::from_millis(100)).await;
    }

    // Calculate metrics
    let total_time = start_time.elapsed();
    let min_latency = latencies.iter().min().cloned().unwrap_or_default();
    let max_latency = latencies.iter().max().cloned().unwrap_or_default();
    let avg_latency = if !latencies.is_empty() {
        Duration::from_nanos(
            (latencies.iter().map(|d| d.as_nanos()).sum::<u128>() / latencies.len() as u128) as u64
        )
    } else {
        Duration::default()
    };
    let throughput = successful_requests as f64 / total_time.as_secs_f64();

    LoadTestMetrics {
        total_requests: num_operations,
        successful_requests,
        failed_requests,
        min_latency,
        max_latency,
        avg_latency,
        throughput,
    }
}

#[tokio::test]
async fn test_memory_store_load() {
    let context = TestContext::new().await.unwrap();

    // Run load tests with different concurrency levels
    let concurrency_levels = [1, 5, 10, 20, 50];
    let operations_per_test = 100;

    for &concurrency in &concurrency_levels {
        println!("\nRunning load test with concurrency level: {}", concurrency);
        
        let metrics = run_concurrent_operations(&context.store, operations_per_test, concurrency).await;
        
        println!("Load Test Results:");
        println!("Total Requests: {}", metrics.total_requests);
        println!("Successful Requests: {}", metrics.successful_requests);
        println!("Failed Requests: {}", metrics.failed_requests);
        println!("Min Latency: {:?}", metrics.min_latency);
        println!("Max Latency: {:?}", metrics.max_latency);
        println!("Avg Latency: {:?}", metrics.avg_latency);
        println!("Throughput: {:.2} ops/sec", metrics.throughput);
        
        assert!(metrics.failed_requests == 0, "Load test had failed requests");
        assert!(metrics.successful_requests > 0, "No successful requests");
    }
}

#[tokio::test]
async fn test_connection_pool_load() {
    let context = TestContext::new().await.unwrap();
    let start_time = Instant::now();
    let mut handles = Vec::new();

    // Simulate 100 concurrent database operations
    for _ in 0..100 {
        let pool = context.pools.postgres.clone();
        handles.push(tokio::spawn(async move {
            let conn = pool.get().await.unwrap();
            let result: i32 = conn.query_one("SELECT 1", &[])
                .await
                .unwrap()
                .get(0);
            assert_eq!(result, 1);
        }));
    }

    // Wait for all operations to complete
    join_all(handles).await;
    let elapsed = start_time.elapsed();
    
    println!("Connection Pool Load Test Results:");
    println!("Total Time: {:?}", elapsed);
    println!("Average Time per Operation: {:?}", elapsed / 100);
}