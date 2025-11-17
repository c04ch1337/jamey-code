mod fixtures;
mod helpers;
mod mocks;
mod utils;

use jamey_core::{
    cache::CacheConfig,
    PoolConfig,
    PostgresPoolConfig,
    RedisPoolConfig,
    ConnectionPools,
};
use proptest::prelude::*;
use std::time::Duration;

// Strategy for generating valid hostnames
fn hostname_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z][a-zA-Z0-9-]{0,61}[a-zA-Z0-9]"
}

// Strategy for generating valid database names
fn database_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z][a-zA-Z0-9_]{0,61}[a-zA-Z0-9]"
}

// Strategy for generating valid usernames
fn username_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z][a-zA-Z0-9_]{0,29}"
}

// Strategy for generating valid passwords
fn password_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9!@#$%^&*()]{8,32}"
}

// Strategy for generating valid Redis URLs
fn redis_url_strategy() -> impl Strategy<Value = String> {
    hostname_strategy().prop_map(|host| format!("redis://{}", host))
}

// Strategy for generating valid port numbers
fn port_strategy() -> impl Strategy<Value = u16> {
    (1024u16..65535u16)
}

// Strategy for generating valid connection counts
fn connection_count_strategy() -> impl Strategy<Value = u32> {
    (1u32..100u32)
}

// Strategy for generating valid timeouts
fn timeout_strategy() -> impl Strategy<Value = u64> {
    (1u64..60u64)
}

proptest! {
    #[test]
    fn test_postgres_config_validation(
        host in hostname_strategy(),
        port in port_strategy(),
        database in database_strategy(),
        user in username_strategy(),
        password in password_strategy(),
        max_connections in connection_count_strategy(),
        min_connections in connection_count_strategy(),
        connect_timeout in timeout_strategy(),
        idle_timeout in timeout_strategy(),
    ) {
        let config = PostgresPoolConfig {
            host,
            port,
            database,
            user,
            password,
            max_connections,
            min_connections: min_connections.min(max_connections),
            connect_timeout: Duration::from_secs(connect_timeout),
            idle_timeout: Duration::from_secs(idle_timeout),
        };

        // Validate configuration
        prop_assert!(config.max_connections >= config.min_connections);
        prop_assert!(config.connect_timeout <= config.idle_timeout);
        prop_assert!(!config.host.is_empty());
        prop_assert!(!config.database.is_empty());
        prop_assert!(!config.user.is_empty());
        prop_assert!(!config.password.is_empty());
    }

    #[test]
    fn test_redis_config_validation(
        url in redis_url_strategy(),
        max_connections in connection_count_strategy(),
        min_connections in connection_count_strategy(),
        connect_timeout in timeout_strategy(),
        idle_timeout in timeout_strategy(),
    ) {
        let config = RedisPoolConfig {
            url,
            max_connections,
            min_connections: min_connections.min(max_connections),
            connect_timeout: Duration::from_secs(connect_timeout),
            idle_timeout: Duration::from_secs(idle_timeout),
        };

        // Validate configuration
        prop_assert!(config.max_connections >= config.min_connections);
        prop_assert!(config.connect_timeout <= config.idle_timeout);
        prop_assert!(config.url.starts_with("redis://"));
    }

    #[test]
    fn test_cache_config_validation(
        memory_capacity in 100u32..10000u32,
        ttl in 1u64..86400u64,
        enable_fallback in proptest::bool::ANY,
        redis_url in proptest::option::of(redis_url_strategy()),
    ) {
        let config = CacheConfig {
            redis_url: redis_url.map(|url| {
                let pool_config = RedisPoolConfig {
                    url,
                    max_connections: 20,
                    min_connections: 5,
                    connect_timeout: Duration::from_secs(5),
                    idle_timeout: Duration::from_secs(300),
                };
                deadpool_redis::Pool::new(pool_config)
            }),
            memory_capacity,
            default_ttl_seconds: ttl,
            enable_fallback,
        };

        // Validate configuration
        prop_assert!(config.memory_capacity > 0);
        prop_assert!(config.default_ttl_seconds > 0);
        if let Some(ref url) = config.redis_url {
            prop_assert!(url.status().is_ready());
        }
    }

    #[test]
    fn test_pool_config_combinations(
        pg_max_conn in connection_count_strategy(),
        pg_min_conn in connection_count_strategy(),
        redis_max_conn in connection_count_strategy(),
        redis_min_conn in connection_count_strategy(),
    ) {
        let config = PoolConfig {
            postgres: PostgresPoolConfig {
                host: "localhost".to_string(),
                port: 5432,
                database: "test".to_string(),
                user: "test".to_string(),
                password: "test".to_string(),
                max_connections: pg_max_conn,
                min_connections: pg_min_conn.min(pg_max_conn),
                connect_timeout: Duration::from_secs(5),
                idle_timeout: Duration::from_secs(300),
            },
            redis: RedisPoolConfig {
                url: "redis://localhost".to_string(),
                max_connections: redis_max_conn,
                min_connections: redis_min_conn.min(redis_max_conn),
                connect_timeout: Duration::from_secs(5),
                idle_timeout: Duration::from_secs(300),
            },
        };

        // Validate configuration combinations
        prop_assert!(config.postgres.max_connections >= config.postgres.min_connections);
        prop_assert!(config.redis.max_connections >= config.redis.min_connections);
    }
}

#[tokio::test]
async fn test_invalid_config_handling() {
    // Test invalid PostgreSQL configuration
    let invalid_pg_config = PostgresPoolConfig {
        host: "invalid-host".to_string(),
        port: 5432,
        database: "invalid-db".to_string(),
        user: "invalid-user".to_string(),
        password: "invalid-password".to_string(),
        max_connections: 5,
        min_connections: 10, // Invalid: min > max
        connect_timeout: Duration::from_secs(5),
        idle_timeout: Duration::from_secs(300),
    };

    // Test invalid Redis configuration
    let invalid_redis_config = RedisPoolConfig {
        url: "invalid://localhost".to_string(), // Invalid URL scheme
        max_connections: 5,
        min_connections: 10, // Invalid: min > max
        connect_timeout: Duration::from_secs(5),
        idle_timeout: Duration::from_secs(300),
    };

    let config = PoolConfig {
        postgres: invalid_pg_config,
        redis: invalid_redis_config,
    };

    // Attempt to create pools with invalid config
    let result = ConnectionPools::new(config).await;
    assert!(result.is_err());
}