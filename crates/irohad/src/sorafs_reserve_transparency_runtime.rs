//! Production supervision for finalized reserve-event transparency ingestion.

use std::{sync::Arc, time::Duration};

use eyre::{Result, bail};
use iroha_config::parameters::actual::SorafsReserveTransparencyRuntime;
use iroha_core::state::{State, StateReadOnly as _};
use iroha_data_model::{ChainId, sorafs::reserve::ReserveFinalizedCursorV1};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use sorafs_node::{
    NodeHandle,
    reputation::runtime::{
        ReputationFinalizedQueryV1, ReputationRuntimeProviderQualificationV1,
    },
    reserve_transparency_runtime::{
        ReputationReserveTransparencyQueryAdapterV1,
        ReserveTransparencyCommittedProjectionErrorV1,
        ReserveTransparencyCommittedProjectionV1, ReserveTransparencyFinalizedQueryV1,
        ReserveTransparencyScannerV1, ReserveTransparencySourceSinkV1,
    },
};

const SHUTDOWN_WAIT: Duration = Duration::from_secs(2);

#[derive(Debug)]
struct StateReserveTransparencyCommittedProjectionV1 {
    state: Arc<State>,
}

impl ReserveTransparencyCommittedProjectionV1
    for StateReserveTransparencyCommittedProjectionV1
{
    fn verify_committed_anchors(
        &self,
        chain_id: &ChainId,
        expected: &[ReserveFinalizedCursorV1],
    ) -> std::result::Result<
        ReserveFinalizedCursorV1,
        ReserveTransparencyCommittedProjectionErrorV1,
    > {
        let view = self.state.query_view();
        if &view.chain_id != chain_id {
            return Err(ReserveTransparencyCommittedProjectionErrorV1::ForkOrReorg);
        }
        let hashes = view.block_hashes();
        let head_height = u64::try_from(hashes.len())
            .map_err(|_| ReserveTransparencyCommittedProjectionErrorV1::Unavailable)?;
        let head_hash = hashes
            .last()
            .map(|hash| *hash.as_ref())
            .filter(|hash| *hash != [0; 32])
            .ok_or(ReserveTransparencyCommittedProjectionErrorV1::Unavailable)?;
        if head_height == 0
            || !committed_anchors_match(expected, |index| {
                hashes.get(index).map(|hash| *hash.as_ref())
            })
        {
            return Err(ReserveTransparencyCommittedProjectionErrorV1::ForkOrReorg);
        }
        Ok(ReserveFinalizedCursorV1 {
            height: head_height,
            block_hash: head_hash,
        })
    }
}

fn committed_anchors_match(
    expected: &[ReserveFinalizedCursorV1],
    mut hash_at: impl FnMut(usize) -> Option<[u8; 32]>,
) -> bool {
    expected.iter().all(|cursor| {
        cursor.height != 0
            && cursor.block_hash != [0; 32]
            && usize::try_from(cursor.height - 1)
                .ok()
                .and_then(&mut hash_at)
                .is_some_and(|hash| hash == cursor.block_hash)
    })
}

/// Assemble and start the bounded reserve transparency scanner.
///
/// # Errors
///
/// Fails before spawning when configuration, the exact finalized-query
/// binding, or the durable checkpoint is invalid.
pub(crate) fn start(
    config: &SorafsReserveTransparencyRuntime,
    chain_id: &ChainId,
    query_qualification: ReputationRuntimeProviderQualificationV1,
    finalized_query: Arc<dyn ReputationFinalizedQueryV1>,
    state: Arc<State>,
    node: NodeHandle,
    shutdown_signal: ShutdownSignal,
) -> Result<Child> {
    if config.poll_interval < Duration::from_millis(100)
        || config.poll_interval > Duration::from_secs(60)
        || config.retry_max_interval < config.poll_interval
        || config.retry_max_interval > Duration::from_secs(300)
    {
        bail!("finalized reserve transparency supervision intervals are invalid");
    }
    let query: Arc<dyn ReserveTransparencyFinalizedQueryV1> = Arc::new(
        ReputationReserveTransparencyQueryAdapterV1::new(finalized_query),
    );
    let projection: Arc<dyn ReserveTransparencyCommittedProjectionV1> = Arc::new(
        StateReserveTransparencyCommittedProjectionV1 { state },
    );
    let sink: Arc<dyn ReserveTransparencySourceSinkV1> = Arc::new(node);
    let mut scanner = ReserveTransparencyScannerV1::try_new(
        config,
        chain_id.clone(),
        query_qualification,
        query,
        projection,
        sink,
    )?;
    let poll_interval = config.poll_interval;
    let retry_max_interval = config.retry_max_interval;
    let task = tokio::spawn(async move {
        let mut retry_delay = Duration::ZERO;
        loop {
            let tick = tokio::task::spawn_blocking(move || {
                let result = scanner.tick();
                (scanner, result)
            });
            let joined = tokio::select! {
                joined = tick => joined,
                () = shutdown_signal.receive() => return,
            };
            let Ok((returned_scanner, result)) = joined else {
                iroha_logger::error!(
                    "finalized reserve transparency scanner task failed closed"
                );
                shutdown_signal.send();
                return;
            };
            scanner = returned_scanner;
            let delay = match result {
                Ok(outcome) => {
                    retry_delay = Duration::ZERO;
                    iroha_logger::debug!(
                        pages = outcome.pages,
                        events = outcome.events,
                        caught_up = outcome.caught_up,
                        finalized_height = outcome.finalized_anchor.height,
                        "reconciled finalized reserve events into transparency source index"
                    );
                    poll_interval
                }
                Err(error) if error.is_retryable() => {
                    retry_delay = next_retry_delay(
                        retry_delay,
                        poll_interval,
                        retry_max_interval,
                    );
                    iroha_logger::warn!(
                        %error,
                        retry_delay_ms = retry_delay.as_millis(),
                        "finalized reserve transparency scanner will retry"
                    );
                    retry_delay
                }
                Err(error) => {
                    iroha_logger::error!(
                        %error,
                        "finalized reserve transparency scanner failed closed"
                    );
                    shutdown_signal.send();
                    return;
                }
            };
            tokio::select! {
                () = tokio::time::sleep(delay) => {}
                () = shutdown_signal.receive() => return,
            }
        }
    });
    Ok(Child::new(task, OnShutdown::Wait(SHUTDOWN_WAIT)))
}

fn next_retry_delay(current: Duration, base: Duration, maximum: Duration) -> Duration {
    if current < base {
        return base.min(maximum);
    }
    current.saturating_mul(2).min(maximum)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn committed_anchor_match_is_exact_and_height_bounded() {
        let hashes = [[0x11; 32], [0x22; 32], [0x33; 32]];
        let lookups = Cell::new(0_usize);
        assert!(committed_anchors_match(
            &[
                ReserveFinalizedCursorV1 {
                    height: 1,
                    block_hash: hashes[0],
                },
                ReserveFinalizedCursorV1 {
                    height: 3,
                    block_hash: hashes[2],
                },
            ],
            |index| {
                lookups.set(lookups.get() + 1);
                hashes.get(index).copied()
            }
        ));
        assert_eq!(
            lookups.get(),
            2,
            "anchor verification must use one indexed lookup per expected cursor"
        );
        assert!(!committed_anchors_match(
            &[ReserveFinalizedCursorV1 {
                height: 2,
                block_hash: [0xFF; 32],
            }],
            |index| hashes.get(index).copied()
        ));
        assert!(!committed_anchors_match(
            &[ReserveFinalizedCursorV1 {
                height: 4,
                block_hash: [0x44; 32],
            }],
            |index| hashes.get(index).copied()
        ));
    }

    #[test]
    fn retry_delay_is_exponential_and_bounded() {
        let base = Duration::from_millis(100);
        let maximum = Duration::from_millis(350);
        assert_eq!(next_retry_delay(Duration::ZERO, base, maximum), base);
        assert_eq!(next_retry_delay(base, base, maximum), Duration::from_millis(200));
        assert_eq!(
            next_retry_delay(Duration::from_millis(200), base, maximum),
            maximum
        );
        assert_eq!(next_retry_delay(maximum, base, maximum), maximum);
    }
}
