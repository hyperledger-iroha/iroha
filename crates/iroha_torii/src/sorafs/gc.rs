//! GC scheduler runtime wiring for Torii-managed SoraFS nodes.
#[cfg(feature = "app_api")]
use crate::sorafs::unix_now_secs;
#[cfg(feature = "app_api")]
use iroha_core::{smartcontracts::ValidSingularQuery, state::State};
#[cfg(feature = "app_api")]
use iroha_data_model::{
    query::sorafs::prelude::{FindSorafsRepairStatus, FindSorafsRepairTasks},
    sorafs::moderation_ledger::REPAIR_QUERY_MAX_ITEMS_V1,
};
#[cfg(feature = "app_api")]
use iroha_futures::supervisor::ShutdownSignal;
#[cfg(feature = "app_api")]
use sorafs_node::repair_ledger_projection::{
    RepairLedgerTaskProjectionBuilderV1, RepairLedgerTaskProjectionV1,
};
use std::{sync::Arc, time::Duration};
#[cfg(feature = "app_api")]
use tokio::time::{MissedTickBehavior, interval};
/// Runtime that periodically invokes the GC sweeper.
#[cfg(feature = "app_api")]
pub struct GcSweeperRuntime {
    node: sorafs_node::NodeHandle,
    state: Arc<State>,
    tick_interval_secs: u64,
}
#[cfg(feature = "app_api")]
impl GcSweeperRuntime {
    /// Create a new GC runtime using the supplied node handle and configuration.
    #[must_use]
    pub fn new(
        node: sorafs_node::NodeHandle,
        state: Arc<State>,
        config: &sorafs_node::config::GcConfig,
    ) -> Self {
        Self {
            node,
            state,
            tick_interval_secs: config.interval_secs().max(1),
        }
    }
    fn repair_projection(&self) -> Result<RepairLedgerTaskProjectionV1, String> {
        let view = self.state.query_view();
        let status = FindSorafsRepairStatus::new(None)
            .execute(&view)
            .map_err(|error| format!("query finalized repair status: {error}"))?;
        let finalized_cursor = status.finalized_cursor;
        let mut builder = RepairLedgerTaskProjectionBuilderV1::new(status)
            .map_err(|error| format!("initialize finalized repair projection: {error}"))?;
        let mut after_task_id = None;
        loop {
            let page = FindSorafsRepairTasks::new(
                Some(finalized_cursor),
                after_task_id,
                REPAIR_QUERY_MAX_ITEMS_V1,
            )
            .execute(&view)
            .map_err(|error| format!("query finalized repair task page: {error}"))?;
            let has_more = page.has_more;
            let next_after_task_id = page.next_after_task_id;
            builder
                .push_page(page)
                .map_err(|error| format!("validate finalized repair task page: {error}"))?;
            if !has_more {
                break;
            }
            after_task_id = next_after_task_id;
        }
        builder
            .finish()
            .map_err(|error| format!("finish finalized repair projection: {error}"))
    }
    fn run_once(&self, now_secs: u64) {
        let repair_projection = match self.repair_projection() {
            Ok(projection) => projection,
            Err(error) => {
                iroha_logger::error!(
                    %error,
                    "GC and reconciliation skipped: finalized repair projection unavailable"
                );
                return;
            }
        };
        let report = self.node.run_gc_once(now_secs, &repair_projection);
        if report.errors > 0 {
            iroha_logger::warn!(
                errors = report.errors,
                evicted = report.evictions.len(),
                "GC sweep reported errors"
            );
        }
        if let Err(err) = self
            .node
            .run_reconciliation_once(now_secs, &repair_projection)
        {
            iroha_logger::warn!(%err, "reconciliation snapshot failed");
        }
    }
    /// Spawn the supervised GC runtime loop until the supplied shutdown signal is received.
    pub(crate) fn spawn(
        self: Arc<Self>,
        shutdown_signal: ShutdownSignal,
    ) -> tokio::task::JoinHandle<crate::ToriiCriticalWorkerExit> {
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(self.tick_interval_secs));
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    biased;
                    _ = shutdown_signal.receive() => {
                        return crate::ToriiCriticalWorkerExit::StoppedByShutdown;
                    }
                    _ = ticker.tick() => {
                        if shutdown_signal.is_sent() {
                            return crate::ToriiCriticalWorkerExit::StoppedByShutdown;
                        }
                        let now = unix_now_secs();
                        let runtime = Arc::clone(&self);
                        let result = crate::panic_recovery::join_recoverable(
                            crate::panic_recovery::spawn_blocking_recoverable(move || {
                                runtime.run_once(now);
                            }),
                        )
                        .await;
                        if let Err(error) = result {
                            iroha_logger::error!(
                                ?error,
                                "SoraFS GC blocking worker failed"
                            );
                            return crate::ToriiCriticalWorkerExit::UnexpectedExit;
                        }
                    }
                }
            }
        })
    }
}
