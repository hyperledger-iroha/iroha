//! Native scheduled-Time, heartbeat and reconciliation carriers, independent of client RPC.

use super::*;
use iroha_core::{smartcontracts::isi::sccp::keepalive, sumeragi::schedule};
use iroha_data_model::isi::chain::Keepalive;

/// Local scheduling granularity; admission and consensus recheck exact millisecond bounds.
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// One local observation and, when eligible, a fresh signed queue insertion.
trait Host: Send + Sync + 'static {
    fn tick(&self, wall_ms: u64) -> eyre::Result<bool>;
}

/// The current validator's independent heartbeat duty.
pub(super) struct NodeKeeper {
    pub(super) host: Arc<NodeHost>,
    pub(super) peer: PeerId,
    pub(super) stagger: Duration,
}

impl Host for NodeKeeper {
    fn tick(&self, wall_ms: u64) -> eyre::Result<bool> {
        // One poll owns one nonblocking observation. A busy publication or reader
        // cannot turn this finite native duty into a blocking-worker retry loop.
        let view = self
            .host
            .state
            .try_view_once()
            .wrap_err("native progress snapshot is unavailable")?;
        let height = view.height();
        if height == 0 {
            iroha_logger::debug!(
                height,
                wall_ms,
                observation = "empty",
                "native progress poll"
            );
            return Ok(false);
        }
        let authority = AccountId::new(self.host.key.public_key().clone());
        if self
            .host
            .queue
            .has_fresh_pending_keepalive(&view, &authority, wall_ms)?
        {
            iroha_logger::debug!(height, wall_ms, pending = "own", "native progress poll");
            return Ok(false);
        }
        if self
            .host
            .queue
            .has_any_fresh_pending_keepalive(&view, wall_ms)
        {
            iroha_logger::debug!(height, wall_ms, pending = "other", "native progress poll");
            return Ok(false);
        }
        let world = view.world();
        let committed_time_ms = view
            .native_execution_tip()
            .map(|tip| tip.creation_time_ms());
        let next_height = u64::try_from(height)?
            .checked_add(1)
            .ok_or_else(|| eyre::eyre!("Keepalive height overflow"))?;
        let Some(due_ms) = keepalive::next_progress_at_ms(world, committed_time_ms, next_height)
        else {
            // TODO: Parliament's authoritative due-item iterator supplies its own due time
            // and one-second rank stagger here. Missing work is never an invented deadline.
            iroha_logger::debug!(
                height,
                wall_ms,
                ?committed_time_ms,
                observation = "no_deadline",
                "native progress poll"
            );
            return Ok(false);
        };
        if wall_ms < due_ms {
            iroha_logger::debug!(
                height,
                wall_ms,
                ?committed_time_ms,
                due_ms,
                observation = "before_deadline",
                "native progress poll"
            );
            return Ok(false);
        }
        let committee = schedule::scheduled_committee(world, next_height)?;
        let Some(rank) = committee.iter().position(|peer| peer == &self.peer) else {
            iroha_logger::debug!(
                height,
                wall_ms,
                ?committed_time_ms,
                due_ms,
                observation = "not_scheduled_validator",
                "native progress poll"
            );
            return Ok(false);
        };
        if !scheduled_progress(
            keepalive::next_heartbeat_at_ms(world),
            keepalive::next_reconciliation_at_ms(world),
            keepalive::next_scheduled_at_ms(world, committed_time_ms, next_height),
            rank,
            self.stagger,
            wall_ms,
        ) {
            iroha_logger::debug!(
                height,
                wall_ms,
                ?committed_time_ms,
                due_ms,
                rank,
                observation = "rank_stagger",
                "native progress poll"
            );
            return Ok(false);
        }
        let candidate_ms = wall_ms
            .checked_add(1)
            .ok_or_else(|| eyre::eyre!("Keepalive creation time overflow"))?;
        if !keepalive::due(world, committed_time_ms, next_height, candidate_ms)? {
            iroha_logger::debug!(
                height,
                wall_ms,
                ?committed_time_ms,
                due_ms,
                rank,
                observation = "not_due",
                "native progress poll"
            );
            return Ok(false);
        }
        drop(view);
        // Admission rechecks the current parent if the tip changed since the observation.
        submit_transaction_at(
            &self.host.state,
            &self.host.queue,
            &self.host.key,
            InstructionBox::from(Keepalive {}),
            wall_ms,
        )?;
        iroha_logger::debug!(
            height,
            wall_ms,
            ?committed_time_ms,
            due_ms,
            rank,
            observation = "submitted",
            "native progress poll"
        );
        Ok(true)
    }
}

/// Reconciliation is governance due work with a one-second stagger (§4.7.4).
/// Compare both staggered deadlines: a later reconciliation can precede a ranked heartbeat.
fn scheduled_progress(
    heartbeat_ms: Option<u64>,
    reconciliation_ms: Option<u64>,
    time_output_ms: Option<u64>,
    rank: usize,
    heartbeat_stagger: Duration,
    wall_ms: u64,
) -> bool {
    heartbeat_ms.is_some_and(|due| scheduled(due, rank, heartbeat_stagger, wall_ms))
        || reconciliation_ms
            .is_some_and(|due| scheduled(due, rank, Duration::from_secs(1), wall_ms))
        || time_output_ms.is_some_and(|due| scheduled(due, rank, Duration::from_secs(1), wall_ms))
}

/// Use the exact canonical rank without allowing overflow to move a deadline backwards.
fn scheduled(due_ms: u64, rank: usize, stagger: Duration, wall_ms: u64) -> bool {
    let Ok(rank) = u64::try_from(rank) else {
        return false;
    };
    let Ok(stagger) = u64::try_from(stagger.as_millis()) else {
        return false;
    };
    rank.checked_mul(stagger)
        .and_then(|offset| due_ms.checked_add(offset))
        .is_some_and(|deadline| wall_ms >= deadline)
}

/// Poll local committed state even when light-client setup or an endpoint is stalled.
pub(super) async fn run(host: Arc<NodeKeeper>, shutdown: ShutdownSignal) {
    run_host(host, shutdown).await;
}

async fn run_host<H: Host>(host: Arc<H>, shutdown: ShutdownSignal) {
    let mut interval = tokio::time::interval(POLL_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            () = shutdown.receive() => return,
            _ = interval.tick() => {}
        }
        let host = Arc::clone(&host);
        let poll = join_recoverable(spawn_blocking_recoverable(move || {
            host.tick(wall_clock_ms())
        }));
        let result = tokio::select! {
            () = shutdown.receive() => return,
            result = poll => result,
        };
        match result {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => iroha_logger::debug!(%error, "SCCP heartbeat keeper deferred"),
            Err(_) => iroha_logger::warn!("SCCP heartbeat keeper local poll panicked"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_waits_for_exact_deadline_and_canonical_rank_stagger() {
        let stagger = Duration::from_millis(15_000);
        assert!(!scheduled(60_000, 0, stagger, 59_999));
        assert!(scheduled(60_000, 0, stagger, 60_000));
        assert!(!scheduled(60_000, 2, stagger, 89_999));
        assert!(scheduled(60_000, 2, stagger, 90_000));
        assert!(!scheduled(u64::MAX, 1, stagger, u64::MAX));
        assert!(!scheduled(1, usize::MAX, Duration::MAX, u64::MAX));
    }

    #[test]
    fn reconciliation_uses_governance_stagger_even_after_heartbeats_stop() {
        let heartbeat_stagger = Duration::from_secs(15);
        assert!(!scheduled_progress(
            None,
            Some(110_000),
            None,
            3,
            heartbeat_stagger,
            112_999
        ));
        assert!(scheduled_progress(
            None,
            Some(110_000),
            None,
            3,
            heartbeat_stagger,
            113_000
        ));
        assert!(!scheduled_progress(
            Some(100_000),
            None,
            None,
            3,
            heartbeat_stagger,
            113_000
        ));
        assert!(scheduled_progress(
            Some(100_000),
            Some(110_000),
            None,
            3,
            heartbeat_stagger,
            113_000
        ));
        assert!(!scheduled_progress(
            None,
            None,
            None,
            0,
            heartbeat_stagger,
            u64::MAX
        ));
    }

    #[test]
    fn scheduled_time_output_wakes_without_heartbeat_and_obeys_rank() {
        let stagger = Duration::from_secs(15);
        assert!(!scheduled_progress(
            None,
            None,
            Some(100_001),
            2,
            stagger,
            102_000
        ));
        assert!(scheduled_progress(
            None,
            None,
            Some(100_001),
            2,
            stagger,
            102_001
        ));
        assert!(scheduled_progress(
            Some(900_000),
            None,
            Some(100_001),
            0,
            stagger,
            100_001
        ));
        assert!(!scheduled_progress(
            None,
            None,
            Some(u64::MAX),
            1,
            stagger,
            u64::MAX
        ));
    }

    #[tokio::test]
    async fn empty_node_has_no_heartbeat_or_synthetic_due_work() {
        let (state, queue) = super::super::tests::node();
        let key = KeyPair::random();
        let peer = PeerId::new(key.public_key().clone());
        let host = NodeKeeper {
            host: Arc::new(NodeHost {
                state,
                queue,
                key,
                l5_cursors: parking_lot::Mutex::default(),
            }),
            peer,
            stagger: Duration::from_millis(15_000),
        };
        assert!(!host.tick(1_000_000).unwrap());
    }

    #[tokio::test]
    async fn local_duty_polls_without_remote_work_and_stops_on_shutdown() {
        struct Probe(std::sync::Mutex<Option<tokio::sync::oneshot::Sender<u64>>>);
        impl Host for Probe {
            fn tick(&self, wall_ms: u64) -> eyre::Result<bool> {
                if let Some(sender) = self.0.lock().unwrap().take() {
                    let _ = sender.send(wall_ms);
                }
                Ok(false)
            }
        }
        let (sender, received) = tokio::sync::oneshot::channel();
        let shutdown = ShutdownSignal::new();
        let task = tokio::spawn(run_host(
            Arc::new(Probe(std::sync::Mutex::new(Some(sender)))),
            shutdown.clone(),
        ));
        assert!(received.await.expect("local poll must run") > 0);
        shutdown.send();
        task.await.expect("keeper stops cleanly");
    }
}
