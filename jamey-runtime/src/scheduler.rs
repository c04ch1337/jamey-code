//! Task Scheduler for 24/7 Operation
//! 
//! Provides scheduling capabilities for continuous operation

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use anyhow::Result;
use uuid::Uuid;
use tracing::{info, error, debug};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: Uuid,
    pub name: String,
    pub connector_id: String,
    pub params: HashMap<String, String>,
    pub schedule: Schedule,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Schedule {
    Interval { seconds: u64 },
    Cron { expression: String },
    OneTime { when: DateTime<Utc> },
    Continuous, // Run continuously
}

pub struct TaskScheduler {
    tasks: HashMap<Uuid, ScheduledTask>,
    running: bool,
}

impl TaskScheduler {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            running: false,
        }
    }

    pub fn add_task(&mut self, task: ScheduledTask) {
        let task_id = task.id;
        let task_name = task.name.clone();
        self.tasks.insert(task_id, task);
        info!("Added scheduled task: {}", task_name);
    }

    pub fn remove_task(&mut self, id: Uuid) -> Option<ScheduledTask> {
        self.tasks.remove(&id)
    }

    pub fn get_task(&self, id: Uuid) -> Option<&ScheduledTask> {
        self.tasks.get(&id)
    }

    pub fn list_tasks(&self) -> Vec<&ScheduledTask> {
        self.tasks.values().collect()
    }

    pub async fn start<F>(&mut self, executor: F)
    where
        F: Fn(String, HashMap<String, String>) -> Result<String> + Send + Sync + 'static,
    {
        self.running = true;
        info!("Task scheduler started");
        
        while self.running {
            let now = Utc::now();
            
            for (id, task) in self.tasks.iter_mut() {
                if !task.enabled {
                    continue;
                }
                
                if now >= task.next_run {
                    debug!("Executing scheduled task: {}", task.name);
                    
                    // Execute task
                    match executor(task.connector_id.clone(), task.params.clone()) {
                        Ok(output) => {
                            info!("Task {} completed: {}", task.name, output);
                        }
                        Err(e) => {
                            error!("Task {} failed: {}", task.name, e);
                        }
                    }
                    
                    task.last_run = Some(now);
                    task.next_run = self.calculate_next_run(&task.schedule, now);
                }
            }
            
            sleep(Duration::from_secs(1)).await;
        }
        
        info!("Task scheduler stopped");
    }

    fn calculate_next_run(&self, schedule: &Schedule, now: DateTime<Utc>) -> DateTime<Utc> {
        match schedule {
            Schedule::Interval { seconds } => now + chrono::Duration::seconds(*seconds as i64),
            Schedule::Cron { expression: _ } => {
                // Use cron parser library - simplified for now
                now + chrono::Duration::minutes(1)
            }
            Schedule::OneTime { when } => *when,
            Schedule::Continuous => now, // Run immediately again
        }
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn is_running(&self) -> bool {
        self.running
    }
}

impl Default for TaskScheduler {
    fn default() -> Self {
        Self::new()
    }
}

