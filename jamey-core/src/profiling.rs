//! Performance profiling utilities using tracing spans
//! 
//! This module provides helpers for instrumenting code with performance metrics
//! and timing information using the tracing framework.

use std::time::Instant;
use tracing::{info, warn, debug, span, Level};

/// Performance threshold configuration
#[derive(Debug, Clone)]
pub struct PerformanceThresholds {
    /// Warn if operation takes longer than this (milliseconds)
    pub warn_threshold_ms: u64,
    /// Log debug info if operation takes longer than this (milliseconds)
    pub debug_threshold_ms: u64,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            warn_threshold_ms: 1000,  // 1 second
            debug_threshold_ms: 100,   // 100ms
        }
    }
}

/// A guard that automatically logs timing information when dropped
pub struct TimingGuard {
    operation: String,
    start: Instant,
    thresholds: PerformanceThresholds,
    _span: tracing::Span,
}

impl TimingGuard {
    /// Create a new timing guard for an operation
    pub fn new(operation: impl Into<String>) -> Self {
        Self::with_thresholds(operation, PerformanceThresholds::default())
    }

    /// Create a new timing guard with custom thresholds
    pub fn with_thresholds(
        operation: impl Into<String>,
        thresholds: PerformanceThresholds,
    ) -> Self {
        let operation = operation.into();
        let span = span!(Level::DEBUG, "timing", operation = %operation);
        
        Self {
            operation,
            start: Instant::now(),
            thresholds,
            _span: span,
        }
    }

    /// Get the elapsed time so far
    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    /// Log a checkpoint with the current elapsed time
    pub fn checkpoint(&self, label: &str) {
        let elapsed = self.elapsed_ms();
        debug!(
            operation = %self.operation,
            checkpoint = %label,
            elapsed_ms = elapsed,
            "Performance checkpoint"
        );
    }
}

impl Drop for TimingGuard {
    fn drop(&mut self) {
        let elapsed = self.elapsed_ms();
        
        if elapsed >= self.thresholds.warn_threshold_ms {
            warn!(
                operation = %self.operation,
                elapsed_ms = elapsed,
                "Slow operation detected"
            );
        } else if elapsed >= self.thresholds.debug_threshold_ms {
            debug!(
                operation = %self.operation,
                elapsed_ms = elapsed,
                "Operation completed"
            );
        } else {
            debug!(
                operation = %self.operation,
                elapsed_ms = elapsed,
                "Operation completed (fast)"
            );
        }
    }
}

/// Macro to create a timing guard with automatic operation name
#[macro_export]
macro_rules! time_operation {
    () => {
        $crate::profiling::TimingGuard::new(format!("{}::{}", module_path!(), line!()))
    };
    ($name:expr) => {
        $crate::profiling::TimingGuard::new($name)
    };
    ($name:expr, $thresholds:expr) => {
        $crate::profiling::TimingGuard::with_thresholds($name, $thresholds)
    };
}

/// Performance metrics collector
#[derive(Debug, Default)]
pub struct PerformanceMetrics {
    pub total_operations: u64,
    pub total_duration_ms: u64,
    pub min_duration_ms: Option<u64>,
    pub max_duration_ms: Option<u64>,
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a new operation timing
    pub fn record(&mut self, duration_ms: u64) {
        self.total_operations += 1;
        self.total_duration_ms += duration_ms;
        
        self.min_duration_ms = Some(
            self.min_duration_ms
                .map(|min| min.min(duration_ms))
                .unwrap_or(duration_ms)
        );
        
        self.max_duration_ms = Some(
            self.max_duration_ms
                .map(|max| max.max(duration_ms))
                .unwrap_or(duration_ms)
        );
    }

    /// Get the average duration
    pub fn avg_duration_ms(&self) -> f64 {
        if self.total_operations == 0 {
            0.0
        } else {
            self.total_duration_ms as f64 / self.total_operations as f64
        }
    }

    /// Log the current metrics
    pub fn log_summary(&self, operation: &str) {
        info!(
            operation = %operation,
            total_ops = self.total_operations,
            avg_ms = self.avg_duration_ms(),
            min_ms = self.min_duration_ms,
            max_ms = self.max_duration_ms,
            "Performance metrics summary"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_timing_guard() {
        let guard = TimingGuard::new("test_operation");
        thread::sleep(Duration::from_millis(10));
        assert!(guard.elapsed_ms() >= 10);
    }

    #[test]
    fn test_performance_metrics() {
        let mut metrics = PerformanceMetrics::new();
        metrics.record(100);
        metrics.record(200);
        metrics.record(150);
        
        assert_eq!(metrics.total_operations, 3);
        assert_eq!(metrics.avg_duration_ms(), 150.0);
        assert_eq!(metrics.min_duration_ms, Some(100));
        assert_eq!(metrics.max_duration_ms, Some(200));
    }
}