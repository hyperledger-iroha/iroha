//! Repair scheduler runtime wiring for Torii-managed SoraFS nodes.

use std::{sync::Arc, time::Duration};

#[cfg(feature = "app_api")]
use iroha_futures::supervisor::ShutdownSignal;
#[cfg(feature = "app_api")]
use tokio::time::{MissedTickBehavior, interval};

#[cfg(feature = "app_api")]
use crate::sorafs::unix_now_secs;

/// Runtime that periodically invokes the repair watchdog and worker loops.
#[cfg(feature = "app_api")]
#[derive(Debug)]
pub struct RepairWorkerRuntime {
    node: sorafs_node::NodeHandle,
    worker_concurrency: usize,
    tick_interval_secs: u64,
}

#[cfg(feature = "app_api")]
impl RepairWorkerRuntime {
    /// Create a new repair runtime using the supplied node handle and configuration.
    #[must_use]
    pub fn new(node: sorafs_node::NodeHandle, config: &sorafs_node::config::RepairConfig) -> Self {
        Self {
            node,
            worker_concurrency: config.worker_concurrency().max(1),
            tick_interval_secs: config.heartbeat_interval_secs().max(1),
        }
    }

    fn run_once(&self, now_secs: u64) {
        if let Err(err) = self.node.run_repair_watchdog_once(now_secs) {
            iroha_logger::error!(%err, "repair watchdog tick failed");
        }

        for worker_index in 0..self.worker_concurrency {
            let worker_id = format!("sorafs-repair-worker-{worker_index}");
            let report = self.node.run_repair_worker_once(&worker_id, now_secs);
            if report.errors > 0 {
                iroha_logger::warn!(
                    errors = report.errors,
                    claimed = report.claimed,
                    "repair worker tick reported errors"
                );
            }
        }
    }

    /// Spawn the repair runtime loop until the supplied shutdown signal is received.
    pub fn spawn(self: Arc<Self>, shutdown_signal: ShutdownSignal) {
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(self.tick_interval_secs));
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = shutdown_signal.receive() => break,
                    _ = ticker.tick() => {
                        let now = unix_now_secs();
                        self.run_once(now);
                    }
                }
            }
        });
    }
}

