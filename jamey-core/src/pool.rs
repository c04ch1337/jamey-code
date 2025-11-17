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

impl PoolConfig {
    pub fn validate(&self) -> Result<()> {
        self.postgres.validate()?;
        self.redis.validate()?;
        Ok(())
    }
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

impl PostgresPoolConfig {
    pub fn validate(&self) -> Result<()> {
        // Validate host
        if self.host.is_empty() || self.host.len() > 255 {
            return Err(anyhow::anyhow!("Invalid host length"));
        }
        if !self.host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_') {
            return Err(anyhow::anyhow!("Host contains invalid characters"));
        }

        // Validate port
        if self.port == 0 {
            return Err(anyhow::anyhow!("Invalid port number"));
        }

        // Validate database name
        if self.database.is_empty() || self.database.len() > 64 {
            return Err(anyhow::anyhow!("Invalid database name length"));
        }
        if !self.database.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(anyhow::anyhow!("Database name contains invalid characters"));
        }

        // Validate user
        if self.user.is_empty() || self.user.len() > 64 {
            return Err(anyhow::anyhow!("Invalid user length"));
        }
        if !self.user.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(anyhow::anyhow!("Username contains invalid characters"));
        }

        // Validate password
        if self.password.is_empty() || self.password.len() > 256 {
            return Err(anyhow::anyhow!("Invalid password length"));
        }

        // Validate connection limits
        if self.max_connections < self.min_connections || self.max_connections > 1000 {
            return Err(anyhow::anyhow!("Invalid connection limits"));
        }
        if self.min_connections == 0 {
            return Err(anyhow::anyhow!("Min connections must be greater than 0"));
        }

        // Validate timeouts
        if self.connect_timeout.as_secs() == 0 || self.connect_timeout.as_secs() > 60 {
            return Err(anyhow::anyhow!("Invalid connect timeout (1-60 seconds)"));
        }
        if self.idle_timeout.as_secs() < 60 || self.idle_timeout.as_secs() > 3600 {
            return Err(anyhow::anyhow!("Invalid idle timeout (60-3600 seconds)"));
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct RedisPoolConfig {
    pub url: String,
    pub max_connections: usize,
    pub min_connections: usize,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
}

impl RedisPoolConfig {
    pub fn validate(&self) -> Result<()> {
        // Validate Redis URL
        if self.url.is_empty() || self.url.len() > 255 {
            return Err(anyhow::anyhow!("Invalid Redis URL length"));
        }
        if !self.url.starts_with("redis://") && !self.url.starts_with("rediss://") {
            return Err(anyhow::anyhow!("Invalid Redis URL scheme"));
        }

        // Parse URL to validate format
        let url = url::Url::parse(&self.url)
            .map_err(|e| anyhow::anyhow!("Invalid Redis URL: {}", e))?;
        
        if url.host_str().is_none() {
            return Err(anyhow::anyhow!("Redis URL must have a host"));
        }

        // Validate connection limits
        if self.max_connections < self.min_connections || self.max_connections > 1000 {
            return Err(anyhow::anyhow!("Invalid connection limits"));
        }
        if self.min_connections == 0 {
            return Err(anyhow::anyhow!("Min connections must be greater than 0"));
        }

        // Validate timeouts
        if self.connect_timeout.as_secs() == 0 || self.connect_timeout.as_secs() > 60 {
            return Err(anyhow::anyhow!("Invalid connect timeout (1-60 seconds)"));
        }
        if self.idle_timeout.as_secs() < 60 || self.idle_timeout.as_secs() > 3600 {
            return Err(anyhow::anyhow!("Invalid idle timeout (60-3600 seconds)"));
        }

        Ok(())
    }
}

impl ConnectionPools {
    pub async fn new(config: PoolConfig) -> Result<Self> {
        // Validate configuration
        config.validate()?;

        let postgres = Self::create_postgres_pool(config.postgres).await?;
        let redis = Self::create_redis_pool(config.redis).await?;

        // Verify both pools are healthy
        let health = Self {
            postgres: postgres.clone(),
            redis: redis.clone(),
        }.health_check().await?;

        if !health.postgres.is_healthy || !health.redis.is_healthy {
            return Err(anyhow::anyhow!("Failed to establish healthy connection pools"));
        }

        Ok(Self { postgres, redis })
    }

    async fn create_postgres_pool(config: PostgresPoolConfig) -> Result<PgPool> {
        let mut pg_config = PgConfig::new();
        pg_config.host = Some(config.host);
        pg_config.port = Some(config.port);
        pg_config.dbname = Some(config.database);
        pg_config.user = Some(config.user);
        pg_config.password = Some(config.password);
        log::info!("Configuring PostgreSQL connection pool with provided credentials");
        
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
            let _ = pool.get().await?;
        }

        Ok(pool)
    }

    async fn create_redis_pool(config: RedisPoolConfig) -> Result<RedisPool> {
        let mut redis_config = RedisConfig::from_url(config.url.clone());
        
        // Configure pool settings
        redis_config.pool = Some(deadpool_redis::PoolConfig::new(config.max_connections));

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
            available_connections: self.postgres.status().available as usize,
            total_connections: self.postgres.status().size,
            latency,
            is_healthy: value == 1,
        })
    }

    async fn check_redis(&self) -> Result<PoolStatus> {
        let start = std::time::Instant::now();
        let mut conn = self.redis.get().await?;
        let latency = start.elapsed();

        let cmd = redis::cmd("PING");
        let value: String = cmd.query_async(conn.as_mut()).await?;

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