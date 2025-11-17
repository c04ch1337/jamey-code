use anyhow::Result;
use deadpool_postgres::{Config as PgConfig, Pool as PgPool, Runtime};
use deadpool_redis::{Config as RedisConfig, Pool as RedisPool, Runtime as RedisRuntime};
use tokio_postgres::NoTls;
use std::time::Duration;

pub struct ConnectionPools {
    pub postgres: PgPool,
    pub redis: RedisPool,
}

#[derive(Clone)]
pub struct PoolConfig {
    pub postgres: PostgresPoolConfig,
    pub redis: RedisPoolConfig,
}

#[derive(Clone)]
pub struct PostgresPoolConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
    pub max_connections: usize,
    pub min_connections: usize,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
}

#[derive(Clone)]
pub struct RedisPoolConfig {
    pub url: String,
    pub max_connections: usize,
    pub min_connections: usize,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
}

impl ConnectionPools {
    pub async fn new(config: PoolConfig) -> Result<Self> {
        let postgres = Self::create_postgres_pool(config.postgres).await?;
        let redis = Self::create_redis_pool(config.redis).await?;

        Ok(Self { postgres, redis })
    }

    async fn create_postgres_pool(config: PostgresPoolConfig) -> Result<PgPool> {
        let mut pg_config = PgConfig::new();
        pg_config.host = Some(config.host);
        pg_config.port = Some(config.port);
        pg_config.dbname = Some(config.database);
        pg_config.user = Some(config.user);
        pg_config.password = Some(config.password);

        // Configure pool settings
        pg_config.pool = Some(deadpool_postgres::PoolConfig {
            max_size: config.max_connections,
            timeouts: deadpool_postgres::Timeouts {
                wait: Some(config.connect_timeout),
                create: Some(config.connect_timeout),
                recycle: Some(config.idle_timeout),
            },
        });

        let pool = pg_config.create_pool(Some(Runtime::Tokio1), NoTls)?;

        // Prewarm the pool by creating minimum connections
        for _ in 0..config.min_connections {
            pool.get().await?;
        }

        Ok(pool)
    }

    async fn create_redis_pool(config: RedisPoolConfig) -> Result<RedisPool> {
        let mut redis_config = RedisConfig::from_url(config.url.clone());
        
        // Configure pool settings
        redis_config.pool = Some(deadpool_redis::PoolConfig {
            max_size: config.max_connections,
            timeouts: deadpool_redis::Timeouts {
                wait: Some(config.connect_timeout),
                create: Some(config.connect_timeout),
                recycle: Some(config.idle_timeout),
            },
        });

        let pool = redis_config.create_pool(Some(RedisRuntime::Tokio1))?;

        // Prewarm the pool by creating minimum connections
        for _ in 0..config.min_connections {
            pool.get().await?;
        }

        Ok(pool)
    }

    pub async fn health_check(&self) -> Result<HealthStatus> {
        let pg_status = self.check_postgres().await?;
        let redis_status = self.check_redis().await?;

        Ok(HealthStatus {
            postgres: pg_status,
            redis: redis_status,
        })
    }

    async fn check_postgres(&self) -> Result<PoolStatus> {
        let start = std::time::Instant::now();
        let client = self.postgres.get().await?;
        let latency = start.elapsed();

        let row = client.query_one("SELECT 1", &[]).await?;
        let value: i32 = row.get(0);
        
        Ok(PoolStatus {
            available_connections: self.postgres.status().available,
            total_connections: self.postgres.status().size,
            latency,
            is_healthy: value == 1,
        })
    }

    async fn check_redis(&self) -> Result<PoolStatus> {
        let start = std::time::Instant::now();
        let mut conn = self.redis.get().await?;
        let latency = start.elapsed();

        let value: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await?;

        Ok(PoolStatus {
            available_connections: self.redis.status().available,
            total_connections: self.redis.status().size,
            latency,
            is_healthy: value == "PONG",
        })
    }
}

#[derive(Debug)]
pub struct HealthStatus {
    pub postgres: PoolStatus,
    pub redis: PoolStatus,
}

#[derive(Debug)]
pub struct PoolStatus {
    pub available_connections: usize,
    pub total_connections: usize,
    pub latency: Duration,
    pub is_healthy: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_pools() -> Result<()> {
        let config = PoolConfig {
            postgres: PostgresPoolConfig {
                host: "localhost".to_string(),
                port: 5432,
                database: "test".to_string(),
                user: "test".to_string(),
                password: "test".to_string(),
                max_connections: 10,
                min_connections: 2,
                connect_timeout: Duration::from_secs(5),
                idle_timeout: Duration::from_secs(300),
            },
            redis: RedisPoolConfig {
                url: "redis://localhost".to_string(),
                max_connections: 10,
                min_connections: 2,
                connect_timeout: Duration::from_secs(5),
                idle_timeout: Duration::from_secs(300),
            },
        };

        let pools = ConnectionPools::new(config).await?;
        let health = pools.health_check().await?;

        assert!(health.postgres.is_healthy);
        assert!(health.redis.is_healthy);
        assert!(health.postgres.available_connections > 0);
        assert!(health.redis.available_connections > 0);

        Ok(())
    }
}