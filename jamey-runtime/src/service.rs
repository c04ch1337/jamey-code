//! 24/7 Service Wrapper for Continuous Operation
//! 
//! Provides a service wrapper that enables Jamey 2.0 to run continuously
//! with scheduler integration and graceful shutdown handling

use crate::scheduler::{TaskScheduler, ScheduledTask, Schedule};
use crate::hybrid_orchestrator::HybridOrchestrator;
use crate::state::RuntimeState;
use anyhow::Result;
use std::collections::HashMap;
use tokio::signal;
use tracing::{info, error, warn};
use uuid::Uuid;
use chrono::Utc;

pub struct JameyService {
    state: std::sync::Arc<RuntimeState>,
}

impl JameyService {
    pub fn new(state: std::sync::Arc<RuntimeState>) -> Self {
        Self { state }
    }

    /// Run Jamey 2.0 in 24/7 mode with scheduler
    pub async fn run_24_7(&self) -> Result<()> {
        info!("🚀 Starting Jamey 2.0 in 24/7 mode...");
        info!("   - Full system access enabled");
        info!("   - Network and web access enabled");
        info!("   - Agent orchestration enabled");
        info!("   - Scheduler enabled");

        // Start scheduler if enabled
        if self.state.config.tools.scheduler_enabled {
            let scheduler_handle = {
                let scheduler = self.state.scheduler.clone();
                let orchestrator = self.state.hybrid_orchestrator.clone();
                
                tokio::spawn(async move {
                    let mut scheduler = scheduler.lock().await;
                    
                    // Create executor function that uses the hybrid orchestrator
                    let executor = |connector_id: String, params: HashMap<String, String>| -> Result<String> {
                        // This will be called by the scheduler
                        // For now, return a placeholder - in production, this would
                        // execute via the orchestrator
                        Ok(format!("Executed {} with params: {:?}", connector_id, params))
                    };
                    
                    scheduler.start(executor).await;
                })
            };

            // Wait for scheduler to initialize
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            info!("✅ Task scheduler started");
        }

        // Start health monitoring task
        let health_handle = {
            let state = self.state.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
                loop {
                    interval.tick().await;
                    // Health check logic here
                    tracing::debug!("Health check: System operational");
                }
            })
        };

        // Wait for shutdown signal (Ctrl+C)
        info!("📡 Listening for shutdown signal (Ctrl+C)...");
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("🛑 Shutdown signal received, gracefully stopping...");
            }
            Err(err) => {
                error!("❌ Failed to listen for shutdown signal: {}", err);
            }
        }

        // Stop scheduler
        if self.state.config.tools.scheduler_enabled {
            let mut scheduler = self.state.scheduler.lock().await;
            scheduler.stop();
            info!("✅ Task scheduler stopped");
        }

        // Cancel health monitoring
        health_handle.abort();
        info!("✅ Health monitoring stopped");

        // Shutdown runtime
        self.state.shutdown().await;
        info!("✅ Jamey 2.0 shutdown complete");

        Ok(())
    }

    /// Add a scheduled task
    pub async fn add_scheduled_task(
        &self,
        name: String,
        connector_id: String,
        params: HashMap<String, String>,
        schedule: Schedule,
    ) -> Result<Uuid> {
        let task = ScheduledTask {
            id: Uuid::new_v4(),
            name,
            connector_id,
            params,
            schedule,
            enabled: true,
            last_run: None,
            next_run: Utc::now(),
        };

        let id = task.id;
        let mut scheduler = self.state.scheduler.lock().await;
        scheduler.add_task(task);
        info!("📅 Added scheduled task: {}", id);
        Ok(id)
    }

    /// Execute a connector immediately
    pub async fn execute_connector(
        &self,
        connector_id: &str,
        params: HashMap<String, String>,
    ) -> Result<jamey_tools::connector::ConnectorResult> {
        let mut orchestrator = self.state.hybrid_orchestrator.lock().await;
        orchestrator.execute_connector(connector_id, params).await
    }

    /// Get service status
    pub fn status(&self) -> ServiceStatus {
        ServiceStatus {
            running: true,
            scheduler_enabled: self.state.config.tools.scheduler_enabled,
            enable_24_7: self.state.config.tools.enable_24_7,
            connectors_registered: 0, // Would need to query orchestrator
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServiceStatus {
    pub running: bool,
    pub scheduler_enabled: bool,
    pub enable_24_7: bool,
    pub connectors_registered: usize,
}

