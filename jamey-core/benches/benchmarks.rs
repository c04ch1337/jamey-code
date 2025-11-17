#![feature(test)]
extern crate test;

use jamey_core::{
    Memory, MemoryType, PostgresMemoryStore, CachedMemoryStore, ConnectionPools, PoolConfig,
    PostgresPoolConfig, RedisPoolConfig,
};
use test::Bencher;
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

#[bench]
fn bench_memory_store(b: &mut Bencher) {
    let rt = setup_runtime();
    let pools = setup_pools();
    let store = rt.block_on(async {
        PostgresMemoryStore::new(pools.postgres.clone(), 1536).await.unwrap()
    });
    let memory = create_test_memory();

    b.iter(|| {
        rt.block_on(async {
            let id = store.store(memory.clone()).await.unwrap();
            let _ = store.retrieve(id).await.unwrap();
            store.delete(id).await.unwrap();
        });
    });
}

#[bench]
fn bench_cached_memory_store(b: &mut Bencher) {
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

    let memory = create_test_memory();

    b.iter(|| {
        rt.block_on(async {
            let id = store.store(memory.clone()).await.unwrap();
            let _ = store.retrieve(id).await.unwrap();
            store.delete(id).await.unwrap();
        });
    });
}

#[bench]
fn bench_vector_search(b: &mut Bencher) {
    let rt = setup_runtime();
    let pools = setup_pools();
    let store = rt.block_on(async {
        PostgresMemoryStore::new(pools.postgres.clone(), 1536).await.unwrap()
    });

    // Pre-populate with test data
    rt.block_on(async {
        for _ in 0..100 {
            let memory = create_test_memory();
            store.store(memory).await.unwrap();
        }
    });

    let query_vector = vec![0.1; 1536];
    b.iter(|| {
        rt.block_on(async {
            let results = store.search(query_vector.clone(), 10).await.unwrap();
            assert!(!results.is_empty());
        });
    });
}

#[bench]
fn bench_cached_vector_search(b: &mut Bencher) {
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

    // Pre-populate with test data
    rt.block_on(async {
        for _ in 0..100 {
            let memory = create_test_memory();
            store.store(memory).await.unwrap();
        }
    });

    let query_vector = vec![0.1; 1536];
    b.iter(|| {
        rt.block_on(async {
            let results = store.search(query_vector.clone(), 10).await.unwrap();
            assert!(!results.is_empty());
        });
    });
}

#[bench]
fn bench_connection_pool_get(b: &mut Bencher) {
    let rt = setup_runtime();
    let pools = setup_pools();

    b.iter(|| {
        rt.block_on(async {
            let conn = pools.postgres.get().await.unwrap();
            let _ = conn.query_one("SELECT 1", &[]).await.unwrap();
        });
    });
}

#[bench]
fn bench_redis_operations(b: &mut Bencher) {
    let rt = setup_runtime();
    let pools = setup_pools();

    b.iter(|| {
        rt.block_on(async {
            let mut conn = pools.redis.get().await.unwrap();
            redis::cmd("SET")
                .arg("bench_key")
                .arg("bench_value")
                .query_async(&mut conn)
                .await
                .unwrap();
            let _: String = redis::cmd("GET")
                .arg("bench_key")
                .query_async(&mut conn)
                .await
                .unwrap();
        });
    });
}