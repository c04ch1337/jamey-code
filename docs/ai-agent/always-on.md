# 24/7 Service Mode Guide

The 24/7 Service Mode enables Jamey 2.0 to run continuously with task scheduling, health monitoring, and graceful shutdown capabilities. This allows for autonomous operation, scheduled maintenance, and always-on assistance.

> 📝 **Note**: 24/7 mode is designed for production deployments where continuous operation is required.

## Table of Contents

- [Overview](#overview)
- [Configuration](#configuration)
- [Service Lifecycle](#service-lifecycle)
- [Task Scheduling](#task-scheduling)
- [Health Monitoring](#health-monitoring)
- [Graceful Shutdown](#graceful-shutdown)
- [Usage Examples](#usage-examples)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)
- [Deployment Considerations](#deployment-considerations)

## Overview

**Service**: `JameyService`  
**Source**: [`jamey-runtime/src/service.rs`](../../jamey-runtime/src/service.rs)

### Key Features

- ✅ **Continuous Operation**: Runs 24/7 without manual intervention
- ✅ **Task Scheduler**: Execute tasks on schedules (cron-like)
- ✅ **Health Monitoring**: Automatic health checks every 60 seconds
- ✅ **Graceful Shutdown**: Clean shutdown on Ctrl+C
- ✅ **Resource Management**: Automatic cleanup and resource monitoring

### Components

| Component | Description | Purpose |
|-----------|-------------|---------|
| Service Wrapper | Main service container | Manages lifecycle |
| Task Scheduler | Cron-like scheduler | Executes scheduled tasks |
| Health Monitor | Health check system | Monitors system status |
| Shutdown Handler | Signal handler | Graceful termination |

## Configuration

### Environment Variables

Add to your `.env` file:

```bash
# Enable 24/7 mode
ENABLE_24_7=true

# Enable task scheduler
SCHEDULER_ENABLED=true

# Optional: Scheduler configuration
SCHEDULER_CHECK_INTERVAL=60  # Seconds between schedule checks
```

### Runtime Configuration

```rust
use jamey_runtime::config::RuntimeConfig;

let config = RuntimeConfig::from_env()?;

println!("24/7 mode: {}", config.tools.enable_24_7);
println!("Scheduler: {}", config.tools.scheduler_enabled);
```

### Service Initialization

```rust
use jamey_runtime::service::JameyService;
use jamey_runtime::state::RuntimeState;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize runtime state
    let config = RuntimeConfig::from_env()?;
    let state = Arc::new(RuntimeState::new(config).await?);
    
    // Create service
    let service = JameyService::new(state);
    
    // Run in 24/7 mode
    service.run_24_7().await?;
    
    Ok(())
}
```

## Service Lifecycle

### Startup Sequence

```
┌─────────────────────────────────────────────────────────┐
│                  Service Startup                        │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
         ┌────────────────────────────────┐
         │  1. Load Configuration         │
         │     - Environment variables    │
         │     - Validate settings        │
         └────────────────┬───────────────┘
                          │
                          ▼
         ┌────────────────────────────────┐
         │  2. Initialize Runtime State   │
         │     - Database connection      │
         │     - LLM provider setup       │
         │     - Connector registration   │
         └────────────────┬───────────────┘
                          │
                          ▼
         ┌────────────────────────────────┐
         │  3. Start Task Scheduler       │
         │     - Load scheduled tasks     │
         │     - Begin schedule checking  │
         └────────────────┬───────────────┘
                          │
                          ▼
         ┌────────────────────────────────┐
         │  4. Start Health Monitoring    │
         │     - 60-second intervals      │
         │     - System health checks     │
         └────────────────┬───────────────┘
                          │
                          ▼
         ┌────────────────────────────────┐
         │  5. Listen for Shutdown Signal │
         │     - Ctrl+C handler           │
         │     - Graceful termination     │
         └────────────────────────────────┘
```

### Startup Logs

```
🚀 Starting Jamey 2.0 in 24/7 mode...
   - Full system access enabled
   - Network and web access enabled
   - Agent orchestration enabled
   - Scheduler enabled
✅ Task scheduler started
📡 Listening for shutdown signal (Ctrl+C)...
```

## Task Scheduling

### Schedule Types

```rust
pub enum Schedule {
    Once(DateTime<Utc>),           // Run once at specific time
    Interval(Duration),             // Run every X duration
    Cron(String),                   // Cron expression (future)
}
```

### Add Scheduled Task

```rust
use jamey_runtime::scheduler::{Schedule, ScheduledTask};
use std::collections::HashMap;
use std::time::Duration;
use chrono::Utc;
use uuid::Uuid;

// Schedule a task to run every hour
let mut params = HashMap::new();
params.insert("action".to_string(), "health_check".to_string());

let task_id = service.add_scheduled_task(
    "Hourly Health Check".to_string(),
    "system_admin".to_string(),
    params,
    Schedule::Interval(Duration::from_secs(3600))
).await?;

println!("Scheduled task: {}", task_id);
```

### Scheduled Task Structure

```rust
pub struct ScheduledTask {
    pub id: Uuid,                      // Unique task ID
    pub name: String,                  // Human-readable name
    pub connector_id: String,          // Connector to execute
    pub params: HashMap<String, String>, // Task parameters
    pub schedule: Schedule,            // When to run
    pub enabled: bool,                 // Enable/disable flag
    pub last_run: Option<DateTime<Utc>>, // Last execution time
    pub next_run: DateTime<Utc>,       // Next scheduled run
}
```

### Execute Connector Immediately

```rust
use std::collections::HashMap;

let mut params = HashMap::new();
params.insert("action".to_string(), "list_processes".to_string());

let result = service.execute_connector(
    "system_admin",
    params
).await?;

println!("Result: {:?}", result);
```

## Health Monitoring

### Health Check System

The service automatically performs health checks every 60 seconds:

```rust
// Health monitoring task (runs automatically)
let health_handle = {
    let state = self.state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(
            tokio::time::Duration::from_secs(60)
        );
        loop {
            interval.tick().await;
            tracing::debug!("Health check: System operational");
            // Additional health checks here
        }
    })
};
```

### Custom Health Checks

```rust
async fn custom_health_check(state: &RuntimeState) -> anyhow::Result<()> {
    // Check database connection
    state.memory.health_check().await?;
    
    // Check LLM provider
    state.llm_provider.health_check().await?;
    
    // Check disk space
    let disk_usage = check_disk_usage()?;
    if disk_usage > 90.0 {
        tracing::warn!("Disk usage high: {:.1}%", disk_usage);
    }
    
    // Check memory usage
    let memory_usage = check_memory_usage()?;
    if memory_usage > 80.0 {
        tracing::warn!("Memory usage high: {:.1}%", memory_usage);
    }
    
    Ok(())
}
```

### Service Status

```rust
let status = service.status();

println!("Service Status:");
println!("  Running: {}", status.running);
println!("  Scheduler: {}", status.scheduler_enabled);
println!("  24/7 Mode: {}", status.enable_24_7);
println!("  Connectors: {}", status.connectors_registered);
```

## Graceful Shutdown

### Shutdown Sequence

```
┌─────────────────────────────────────────────────────────┐
│              Graceful Shutdown (Ctrl+C)                 │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
         ┌────────────────────────────────┐
         │  1. Receive Shutdown Signal    │
         │     - Ctrl+C detected          │
         │     - Log shutdown initiation  │
         └────────────────┬───────────────┘
                          │
                          ▼
         ┌────────────────────────────────┐
         │  2. Stop Task Scheduler        │
         │     - Cancel pending tasks     │
         │     - Wait for current tasks   │
         └────────────────┬───────────────┘
                          │
                          ▼
         ┌────────────────────────────────┐
         │  3. Stop Health Monitoring     │
         │     - Cancel health checks     │
         │     - Log final status         │
         └────────────────┬───────────────┘
                          │
                          ▼
         ┌────────────────────────────────┐
         │  4. Shutdown Runtime           │
         │     - Close database           │
         │     - Cleanup resources        │
         └────────────────┬───────────────┘
                          │
                          ▼
         ┌────────────────────────────────┐
         │  5. Exit Cleanly               │
         │     - Log completion           │
         │     - Return success code      │
         └────────────────────────────────┘
```

### Shutdown Logs

```
🛑 Shutdown signal received, gracefully stopping...
✅ Task scheduler stopped
✅ Health monitoring stopped
✅ Jamey 2.0 shutdown complete
```

### Shutdown Handler

```rust
// Wait for shutdown signal (Ctrl+C)
match signal::ctrl_c().await {
    Ok(()) => {
        tracing::info!("🛑 Shutdown signal received, gracefully stopping...");
    }
    Err(err) => {
        tracing::error!("❌ Failed to listen for shutdown signal: {}", err);
    }
}

// Stop scheduler
if self.state.config.tools.scheduler_enabled {
    let mut scheduler = self.state.scheduler.lock().await;
    scheduler.stop();
    tracing::info!("✅ Task scheduler stopped");
}

// Cancel health monitoring
health_handle.abort();
tracing::info!("✅ Health monitoring stopped");

// Shutdown runtime
self.state.shutdown().await;
tracing::info!("✅ Jamey 2.0 shutdown complete");
```

## Usage Examples

### Example 1: Basic 24/7 Service

```rust
use jamey_runtime::service::JameyService;
use jamey_runtime::state::RuntimeState;
use jamey_runtime::config::RuntimeConfig;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load configuration
    let config = RuntimeConfig::from_env()?;
    
    // Create runtime state
    let state = Arc::new(RuntimeState::new(config).await?);
    
    // Create and run service
    let service = JameyService::new(state);
    service.run_24_7().await?;
    
    Ok(())
}
```

### Example 2: Service with Scheduled Tasks

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = RuntimeConfig::from_env()?;
    let state = Arc::new(RuntimeState::new(config).await?);
    let service = JameyService::new(state);
    
    // Schedule daily backup at 2 AM
    let mut params = HashMap::new();
    params.insert("action".to_string(), "backup".to_string());
    
    service.add_scheduled_task(
        "Daily Backup".to_string(),
        "full_system".to_string(),
        params,
        Schedule::Interval(Duration::from_secs(86400)) // 24 hours
    ).await?;
    
    // Schedule hourly health check
    let mut params = HashMap::new();
    params.insert("action".to_string(), "health_check".to_string());
    
    service.add_scheduled_task(
        "Hourly Health Check".to_string(),
        "system_admin".to_string(),
        params,
        Schedule::Interval(Duration::from_secs(3600)) // 1 hour
    ).await?;
    
    // Run service
    service.run_24_7().await?;
    
    Ok(())
}
```

### Example 3: Service with Custom Health Checks

```rust
async fn run_with_monitoring() -> anyhow::Result<()> {
    let config = RuntimeConfig::from_env()?;
    let state = Arc::new(RuntimeState::new(config).await?);
    let service = JameyService::new(state.clone());
    
    // Start custom monitoring
    let monitor_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            if let Err(e) = custom_health_check(&monitor_state).await {
                tracing::error!("Health check failed: {}", e);
            }
        }
    });
    
    // Run service
    service.run_24_7().await?;
    
    Ok(())
}
```

## Best Practices

### ✅ Do

1. **Enable Logging**
   ```rust
   // Initialize structured logging
   tracing_subscriber::fmt()
       .with_max_level(tracing::Level::INFO)
       .init();
   ```

2. **Monitor Resource Usage**
   ```rust
   // Regular resource checks
   async fn monitor_resources() {
       let mut interval = tokio::time::interval(Duration::from_secs(300));
       loop {
           interval.tick().await;
           check_disk_space().await;
           check_memory_usage().await;
           check_cpu_usage().await;
       }
   }
   ```

3. **Implement Error Recovery**
   ```rust
   // Restart on critical errors
   loop {
       match service.run_24_7().await {
           Ok(_) => break,
           Err(e) if is_recoverable(&e) => {
               tracing::error!("Recoverable error: {}", e);
               tokio::time::sleep(Duration::from_secs(10)).await;
           }
           Err(e) => return Err(e),
       }
   }
   ```

4. **Use Systemd/Service Manager**
   ```ini
   # /etc/systemd/system/jamey.service
   [Unit]
   Description=Jamey 2.0 AI Agent
   After=network.target postgresql.service
   
   [Service]
   Type=simple
   User=jamey
   WorkingDirectory=/opt/jamey
   ExecStart=/opt/jamey/target/release/jamey-runtime
   Restart=always
   RestartSec=10
   
   [Install]
   WantedBy=multi-user.target
   ```

5. **Rotate Logs**
   ```bash
   # /etc/logrotate.d/jamey
   /var/log/jamey/*.log {
       daily
       rotate 7
       compress
       delaycompress
       notifempty
       create 0640 jamey jamey
   }
   ```

### ❌ Don't

1. **Don't Run as Root**
   - Use dedicated service account
   - Apply principle of least privilege

2. **Don't Ignore Health Check Failures**
   - Monitor health check logs
   - Set up alerts for failures

3. **Don't Skip Graceful Shutdown**
   - Always handle Ctrl+C properly
   - Clean up resources on exit

4. **Don't Overload the Scheduler**
   - Limit number of scheduled tasks
   - Avoid overlapping schedules

5. **Don't Ignore Resource Limits**
   - Set memory limits
   - Monitor disk usage
   - Implement rate limiting

## Troubleshooting

### Issue: Service Won't Start

**Cause**: Configuration error or missing dependencies

**Solution**:
```bash
# Check configuration
cargo run --package jamey-runtime -- --validate-config

# Check dependencies
systemctl status postgresql
ping -c 1 api.openrouter.ai

# Check logs
journalctl -u jamey -n 50
```

### Issue: High Memory Usage

**Cause**: Memory leak or too many concurrent operations

**Solution**:
```rust
// Monitor memory
use sysinfo::{System, SystemExt};

let mut sys = System::new_all();
sys.refresh_all();
let used_memory = sys.used_memory();
let total_memory = sys.total_memory();
let usage_percent = (used_memory as f64 / total_memory as f64) * 100.0;

if usage_percent > 80.0 {
    tracing::warn!("High memory usage: {:.1}%", usage_percent);
}
```

### Issue: Scheduler Not Running Tasks

**Cause**: Scheduler disabled or task configuration error

**Solution**:
```bash
# Verify scheduler is enabled
grep SCHEDULER_ENABLED .env

# Check task configuration
# Verify connector_id is valid
# Verify schedule is correct
```

### Issue: Service Crashes on Shutdown

**Cause**: Improper cleanup or resource deadlock

**Solution**:
```rust
// Ensure proper cleanup order
// 1. Stop scheduler
// 2. Cancel background tasks
// 3. Close connections
// 4. Exit
```

## Deployment Considerations

### Production Deployment

```bash
# 1. Build release binary
cargo build --release

# 2. Create service user
sudo useradd -r -s /bin/false jamey

# 3. Install binary
sudo cp target/release/jamey-runtime /opt/jamey/
sudo chown jamey:jamey /opt/jamey/jamey-runtime

# 4. Configure environment
sudo cp .env /opt/jamey/.env
sudo chown jamey:jamey /opt/jamey/.env
sudo chmod 600 /opt/jamey/.env

# 5. Install systemd service
sudo cp jamey.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable jamey
sudo systemctl start jamey

# 6. Verify
sudo systemctl status jamey
sudo journalctl -u jamey -f
```

### Docker Deployment

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/jamey-runtime /usr/local/bin/
COPY .env /app/.env

WORKDIR /app
USER 1000:1000

CMD ["jamey-runtime"]
```

### Monitoring Setup

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'jamey'
    static_configs:
      - targets: ['localhost:9090']
```

### Backup Strategy

```bash
# Daily backup script
#!/bin/bash
DATE=$(date +%Y%m%d)
pg_dump jamey > /backups/jamey_$DATE.sql
find /backups -name "jamey_*.sql" -mtime +7 -delete
```

## Related Documentation

- [AI Agent Overview](README.md) - Overview of all agent capabilities
- [Security Best Practices](security-best-practices.md) - Security guidelines
- [Agent Orchestration](orchestration.md) - Multi-agent coordination
- [Configuration Guide](../architecture/configuration.md) - Configuration system

---

**Last Updated**: 2025-11-17  
**Version**: 1.0.0  
**Status**: ✅ Complete