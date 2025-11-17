use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use jamey_core::{
    Memory, MemoryType, PostgresMemoryStore, MemoryStore, CachedMemoryStore, 
    ConnectionPools, PoolConfig, PostgresPoolConfig, RedisPoolConfig,
};
use tokio::runtime::Runtime;
use std::time::Duration;
use uuid::Uuid;
use chrono::Utc;

fn setup_runtime() -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn create_test_memory() -> Memory {
    Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "Test memory content for benchmarking".to_string(),
        embedding: vec![0.1; 1536],
        metadata: serde_json::json!({"benchmark": true}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    }
}

fn setup_pools() -> ConnectionPools {
    let config = PoolConfig {
        postgres: PostgresPoolConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "jamey_test".to_string(),
            user: "jamey".to_string(),
            password: "test_password".to_string(),
            max_connections: 20,
            min_connections: 5,
            connect_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(300),
        },
        redis: RedisPoolConfig {
            url: "redis://localhost".to_string(),
            max_connections: 20,
            min_connections: 5,
            connect_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(300),
        },
    };

    let rt = setup_runtime();
    rt.block_on(async {
        ConnectionPools::new(config).await.unwrap()
    })
}

fn bench_memory_operations(c: &mut Criterion) {
    let rt = setup_runtime();
    let pools = setup_pools();
    let store = rt.block_on(async {
        PostgresMemoryStore::new(pools.postgres.clone(), 1536).await.unwrap()
    });

    let mut group = c.benchmark_group("memory_operations");
    
    group.bench_function("store_retrieve_delete", |b| {
        b.to_async(&rt).iter(|| async {
            let memory = create_test_memory();
            let id = store.store(black_box(memory)).await.unwrap();
            let _ = store.retrieve(black_box(id)).await.unwrap();
            store.delete(black_box(id)).await.unwrap();
        });
    });

    group.bench_function("store_only", |b| {
        b.to_async(&rt).iter(|| async {
            let memory = create_test_memory();
            let id = store.store(black_box(memory)).await.unwrap();
            // Clean up
            store.delete(id).await.unwrap();
        });
    });

    group.finish();
}

fn bench_vector_search(c: &mut Criterion) {
    let rt = setup_runtime();
    let pools = setup_pools();
    let store = rt.block_on(async {
        PostgresMemoryStore::new(pools.postgres.clone(), 1536).await.unwrap()
    });

    // Pre-populate with test data
    let memory_ids: Vec<Uuid> = rt.block_on(async {
        let mut ids = Vec::new();
        for _ in 0..100 {
            let memory = create_test_memory();
            let id = store.store(memory).await.unwrap();
            ids.push(id);
        }
        ids
    });

    let mut group = c.benchmark_group("vector_search");
    
    for limit in [1, 10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(limit), limit, |b, &limit| {
            let query_vector = vec![0.1; 1536];
            b.to_async(&rt).iter(|| async {
                let results = store.search(black_box(&query_vector), black_box(limit)).await.unwrap();
                black_box(results);
            });
        });
    }

    group.finish();

    // Cleanup
    rt.block_on(async {
        for id in memory_ids {
            let _ = store.delete(id).await;
        }
    });
}

fn bench_pagination(c: &mut Criterion) {
    let rt = setup_runtime();
    let pools = setup_pools();
    let store = rt.block_on(async {
        PostgresMemoryStore::new(pools.postgres.clone(), 1536).await.unwrap()
    });

    // Pre-populate with test data
    let memory_ids: Vec<Uuid> = rt.block_on(async {
        let mut ids = Vec::new();
        for _ in 0..1000 {
            let memory = create_test_memory();
            let id = store.store(memory).await.unwrap();
            ids.push(id);
        }
        ids
    });

    let mut group = c.benchmark_group("pagination");
    
    for page_size in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("list_paginated", page_size), 
            page_size, 
            |b, &page_size| {
                b.to_async(&rt).iter(|| async {
                    let (results, total) = store.list_paginated(
                        black_box(page_size), 
                        black_box(0)
                    ).await.unwrap();
                    black_box((results, total));
                });
            }
        );
    }

    group.finish();

    // Cleanup
    rt.block_on(async {
        for id in memory_ids {
            let _ = store.delete(id).await;
        }
    });
}

fn bench_cached_operations(c: &mut Criterion) {
    let rt = setup_runtime();
    let pools = setup_pools();
    
    let store = rt.block_on(async {
        let postgres_store = PostgresMemoryStore::new(pools.postgres.clone(), 1536).await.unwrap();
        let cache_config = jamey_core::cache::CacheConfig {
            redis_url: Some(pools.redis.clone()),
            memory_capacity: 1000,
            default_ttl_seconds: 300,
            enable_fallback: true,
        };
        CachedMemoryStore::new(postgres_store, cache_config).await.unwrap()
    });

    let mut group = c.benchmark_group("cached_operations");
    
    // Benchmark cache hit
    let test_memory = create_test_memory();
    let test_id = rt.block_on(async {
        store.store(test_memory).await.unwrap()
    });

    // Warm up cache
    rt.block_on(async {
        let _ = store.retrieve(test_id).await.unwrap();
    });

    group.bench_function("retrieve_cache_hit", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = store.retrieve(black_box(test_id)).await.unwrap();
        });
    });

    group.bench_function("retrieve_cache_miss", |b| {
        b.to_async(&rt).iter(|| async {
            let new_memory = create_test_memory();
            let id = store.store(new_memory).await.unwrap();
            let _ = store.retrieve(black_box(id)).await.unwrap();
            store.delete(id).await.unwrap();
        });
    });

    group.finish();

    // Cleanup
    rt.block_on(async {
        let _ = store.delete(test_id).await;
    });
}

fn bench_arc_clones(c: &mut Criterion) {
    use std::sync::Arc;
    
    let mut group = c.benchmark_group("arc_operations");
    
    let data = Arc::new(vec![0u8; 1024]);
    
    group.bench_function("arc_clone", |b| {
        b.iter(|| {
            let _cloned = black_box(Arc::clone(&data));
        });
    });
    
    group.bench_function("vec_clone", |b| {
        let vec_data = vec![0u8; 1024];
        b.iter(|| {
            let _cloned = black_box(vec_data.clone());
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_memory_operations,
    bench_vector_search,
    bench_pagination,
    bench_cached_operations,
    bench_arc_clones
);
criterion_main!(benches);