mod fixtures;
mod helpers;
mod mocks;
mod utils;

use jamey_core::{
    ConnectionPools,
    PoolConfig,
    PostgresPoolConfig,
    RedisPoolConfig,
};
use std::time::Duration;
use utils::retry_with_backoff;

#[tokio::test]
async fn test_pool_creation() {
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

    let pools = ConnectionPools::new(config).await.unwrap();
    
    // Test Postgres connection
    let conn = pools.postgres.get().await.unwrap();
    let result: i32 = conn.query_one("SELECT 1", &[]).await.unwrap().get(0);
    assert_eq!(result, 1);
    
    // Test Redis connection
    let mut conn = pools.redis.get().await.unwrap();
    redis::cmd("PING")
        .query_async::<_, String>(&mut conn)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_pool_connection_limits() {
    let config = PoolConfig {
        postgres: PostgresPoolConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "jamey_test".to_string(),
            user: "jamey".to_string(),
            password: "test_password".to_string(),
            max_connections: 5,
            min_connections: 2,
            connect_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(300),
        },
        redis: RedisPoolConfig {
            url: "redis://localhost".to_string(),
            max_connections: 5,
            min_connections: 2,
            connect_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(300),
        },
    };

    let pools = ConnectionPools::new(config).await.unwrap();
    
    // Test Postgres pool limits
    let mut pg_conns = Vec::new();
    for _ in 0..5 {
        let conn = pools.postgres.get().await.unwrap();
        pg_conns.push(conn);
    }
    
    // Next connection should timeout
    let pg_result = tokio::time::timeout(
        Duration::from_secs(1),
        pools.postgres.get()
    ).await;
    assert!(pg_result.is_err());
    
    // Test Redis pool limits
    let mut redis_conns = Vec::new();
    for _ in 0..5 {
        let conn = pools.redis.get().await.unwrap();
        redis_conns.push(conn);
    }
    
    // Next connection should timeout
    let redis_result = tokio::time::timeout(
        Duration::from_secs(1),
        pools.redis.get()
    ).await;
    assert!(redis_result.is_err());
}

#[tokio::test]
async fn test_pool_connection_timeouts() {
    let config = PoolConfig {
        postgres: PostgresPoolConfig {
            host: "invalid-host".to_string(),
            port: 5432,
            database: "jamey_test".to_string(),
            user: "jamey".to_string(),
            password: "test_password".to_string(),
            max_connections: 5,
            min_connections: 0,
            connect_timeout: Duration::from_secs(1),
            idle_timeout: Duration::from_secs(300),
        },
        redis: RedisPoolConfig {
            url: "redis://invalid-host".to_string(),
            max_connections: 5,
            min_connections: 0,
            connect_timeout: Duration::from_secs(1),
            idle_timeout: Duration::from_secs(300),
        },
    };

    // Both pools should fail to connect due to invalid hosts
    let result = ConnectionPools::new(config).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_pool_connection_recovery() {
    let context = helpers::TestContext::new().await.unwrap();
    let pools = context.pools;
    
    // Get initial connections
    let pg_conn = pools.postgres.get().await.unwrap();
    let redis_conn = pools.redis.get().await.unwrap();
    
    // Drop connections
    drop(pg_conn);
    drop(redis_conn);
    
    // Wait briefly
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Should be able to get new connections
    let pg_conn = pools.postgres.get().await.unwrap();
    let redis_conn = pools.redis.get().await.unwrap();
    
    // Verify connections work
    let pg_result: i32 = pg_conn.query_one("SELECT 1", &[]).await.unwrap().get(0);
    assert_eq!(pg_result, 1);
    
    let mut redis_conn = redis_conn;
    redis::cmd("PING")
        .query_async::<_, String>(&mut redis_conn)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_pool_idle_timeout() {
    let config = PoolConfig {
        postgres: PostgresPoolConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "jamey_test".to_string(),
            user: "jamey".to_string(),
            password: "test_password".to_string(),
            max_connections: 5,
            min_connections: 1,
            connect_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(1), // Short idle timeout
        },
        redis: RedisPoolConfig {
            url: "redis://localhost".to_string(),
            max_connections: 5,
            min_connections: 1,
            connect_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(1), // Short idle timeout
        },
    };

    let pools = ConnectionPools::new(config).await.unwrap();
    
    // Get and release connections
    let pg_conn = pools.postgres.get().await.unwrap();
    let redis_conn = pools.redis.get().await.unwrap();
    drop(pg_conn);
    drop(redis_conn);
    
    // Wait for idle timeout
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Should still be able to get new connections
    let pg_conn = pools.postgres.get().await.unwrap();
    let redis_conn = pools.redis.get().await.unwrap();
    
    // Verify connections work
    let pg_result: i32 = pg_conn.query_one("SELECT 1", &[]).await.unwrap().get(0);
    assert_eq!(pg_result, 1);
    
    let mut redis_conn = redis_conn;
    redis::cmd("PING")
        .query_async::<_, String>(&mut redis_conn)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_pool_concurrent_operations() {
    let context = helpers::TestContext::new().await.unwrap();
    let pools = context.pools;
    
    let mut handles = Vec::new();
    
    // Spawn multiple concurrent operations
    for i in 0..10 {
        let pools = pools.clone();
        handles.push(tokio::spawn(async move {
            // Get connections
            let pg_conn = pools.postgres.get().await.unwrap();
            let mut redis_conn = pools.redis.get().await.unwrap();
            
            // Perform operations
            let pg_result: i32 = pg_conn.query_one("SELECT $1", &[&i]).await.unwrap().get(0);
            assert_eq!(pg_result, i);
            
            redis::cmd("SET")
                .arg(format!("key{}", i))
                .arg(i.to_string())
                .query_async(&mut redis_conn)
                .await
                .unwrap();
            
            let redis_result: String = redis::cmd("GET")
                .arg(format!("key{}", i))
                .query_async(&mut redis_conn)
                .await
                .unwrap();
            assert_eq!(redis_result, i.to_string());
        }));
    }
    
    // Wait for all operations to complete
    for handle in handles {
        handle.await.unwrap();
    }
}