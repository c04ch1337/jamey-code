mod fixtures;
mod helpers;
mod mocks;
mod utils;

use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::Path,
    time::Instant,
};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use helpers::TestContext;

#[derive(Debug, Serialize, Deserialize)]
struct PerformanceMetrics {
    timestamp: DateTime<Utc>,
    git_commit: String,
    metrics: HashMap<String, MetricData>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MetricData {
    value: f64,
    unit: String,
    threshold: f64,
}

#[derive(Debug)]
struct RegressionTest {
    name: String,
    current_value: f64,
    historical_avg: f64,
    threshold: f64,
    unit: String,
}

impl RegressionTest {
    fn has_regressed(&self) -> bool {
        self.current_value > self.historical_avg * (1.0 + self.threshold)
    }

    fn regression_percentage(&self) -> f64 {
        ((self.current_value - self.historical_avg) / self.historical_avg) * 100.0
    }
}

const METRICS_FILE: &str = "performance_metrics.json";
const HISTORY_LIMIT: usize = 10; // Number of historical data points to keep

async fn setup_test_environment() -> (CachedMemoryStore, ConnectionPools) {
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
    let postgres_store = PostgresMemoryStore::new(pools.postgres.clone(), 1536).await.unwrap();
    
    let cache_config = jamey_core::cache::CacheConfig {
        redis_url: Some(pools.redis.clone()),
        memory_capacity: 1000,
        default_ttl_seconds: 300,
        enable_fallback: true,
    };
    
    let store = CachedMemoryStore::new(postgres_store, cache_config).await.unwrap();
    (store, pools)
}


fn get_git_commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn load_historical_metrics() -> Vec<PerformanceMetrics> {
    if Path::new(METRICS_FILE).exists() {
        let file = File::open(METRICS_FILE).unwrap();
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn save_metrics(metrics: &PerformanceMetrics) {
    let mut historical = load_historical_metrics();
    historical.push(metrics.clone());
    
    // Keep only the most recent entries
    if historical.len() > HISTORY_LIMIT {
        historical.remove(0);
    }
    
    let file = File::create(METRICS_FILE).unwrap();
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, &historical).unwrap();
}

#[tokio::test]
async fn test_performance_regression() {
    let context = TestContext::new().await.unwrap();
    let mut current_metrics = HashMap::new();
    
    // Test 1: Memory Store Operations
    {
        let memory = context.create_test_memory();
        let start = Instant::now();
        
        let id = context.store.store(memory.clone()).await.unwrap();
        let _ = context.store.retrieve(id).await.unwrap();
        let _ = context.store.search(memory.embedding.clone(), 5).await.unwrap();
        context.store.delete(id).await.unwrap();
        
        let duration = start.elapsed();
        current_metrics.insert(
            "memory_operations_ms".to_string(),
            MetricData {
                value: duration.as_millis() as f64,
                unit: "ms".to_string(),
                threshold: 0.15, // 15% threshold
            },
        );
    }
    
    // Test 2: Connection Pool Performance
    {
        let start = Instant::now();
        let mut handles = Vec::new();
        
        for _ in 0..10 {
            let pool = context.pools.postgres.clone();
            handles.push(tokio::spawn(async move {
                let conn = pool.get().await.unwrap();
                let _: i32 = conn.query_one("SELECT 1", &[]).await.unwrap().get(0);
            }));
        }
        
        futures::future::join_all(handles).await;
        let duration = start.elapsed();
        
        current_metrics.insert(
            "connection_pool_ms".to_string(),
            MetricData {
                value: duration.as_millis() as f64,
                unit: "ms".to_string(),
                threshold: 0.20, // 20% threshold
            },
        );
    }
    
    // Save current metrics
    let performance_metrics = PerformanceMetrics {
        timestamp: Utc::now(),
        git_commit: get_git_commit(),
        metrics: current_metrics.clone(),
    };
    save_metrics(&performance_metrics);
    
    // Compare with historical data
    let historical = load_historical_metrics();
    if !historical.is_empty() {
        let mut regressions = Vec::new();
        
        for (metric_name, current_data) in current_metrics {
            let historical_values: Vec<f64> = historical
                .iter()
                .filter_map(|h| h.metrics.get(&metric_name))
                .map(|m| m.value)
                .collect();
            
            if !historical_values.is_empty() {
                let avg = historical_values.iter().sum::<f64>() / historical_values.len() as f64;
                
                let test = RegressionTest {
                    name: metric_name,
                    current_value: current_data.value,
                    historical_avg: avg,
                    threshold: current_data.threshold,
                    unit: current_data.unit,
                };
                
                if test.has_regressed() {
                    regressions.push(test);
                }
            }
        }
        
        // Report regressions
        if !regressions.is_empty() {
            let mut message = String::from("\nPerformance regressions detected:\n");
            for regression in regressions {
                message.push_str(&format!(
                    "- {}: current {:.2}{} vs historical avg {:.2}{} ({:.1}% regression)\n",
                    regression.name,
                    regression.current_value,
                    regression.unit,
                    regression.historical_avg,
                    regression.unit,
                    regression.regression_percentage(),
                ));
            }
            panic!("{}", message);
        }
    }
}

#[tokio::test]
async fn test_cleanup() {
    // Clean up test data after all tests
    if Path::new(METRICS_FILE).exists() {
        fs::remove_file(METRICS_FILE).unwrap();
    }
}