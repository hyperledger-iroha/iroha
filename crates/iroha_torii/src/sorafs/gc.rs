//! GC scheduler runtime wiring for Torii-managed SoraFS nodes.

use std::{sync::Arc, time::Duration};

#[cfg(feature = "app_api")]
use iroha_futures::supervisor::ShutdownSignal;
#[cfg(feature = "app_api")]
use tokio::time::{MissedTickBehavior, interval};

#[cfg(feature = "app_api")]
use crate::sorafs::unix_now_secs;

/// Runtime that periodically invokes the GC sweeper.
#[cfg(feature = "app_api")]
#[derive(Debug)]
pub struct GcSweeperRuntime {
    node: sorafs_node::NodeHandle,
    tick_interval_secs: u64,
}

#[cfg(feature = "app_api")]
impl GcSweeperRuntime {
    /// Create a new GC runtime using the supplied node handle and configuration.
    #[must_use]
    pub fn new(node: sorafs_node::NodeHandle, config: &sorafs_node::config::GcConfig) -> Self {
        Self {
            node,
            tick_interval_secs: config.interval_secs().max(1),
        }
    }

    fn run_once(&self, now_secs: u64) {
        let report = self.node.run_gc_once(now_secs);
        if report.errors > 0 {
            iroha_logger::warn!(
                errors = report.errors,
                evicted = report.evictions.len(),
                "GC sweep reported errors"
            );
        }
        if let Err(err) = self.node.run_reconciliation_once(now_secs) {
            iroha_logger::warn!(%err, "reconciliation snapshot failed");
        }
    }

    /// Spawn the GC runtime loop until the supplied shutdown signal is received.
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
