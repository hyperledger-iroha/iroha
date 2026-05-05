//! Kura persistence and retry helpers.

use std::time::{Duration, Instant};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::{BlockHeader, SignedBlock};
use iroha_logger::prelude::*;

use super::{Actor, pending_block::PendingBlock};
use crate::{
    queue::Queue,
    state::{State, StateReadOnly},
    sumeragi::status,
};

pub(super) const KURA_STAGE_ROLLBACK_REASON_STORE: &str = "store_failure";
pub(super) const KURA_STAGE_ROLLBACK_REASON_STATE: &str = "state_commit_failure";
pub(super) const KURA_LOCK_RESET_REASON_ABORT: &str = "kura_abort";

pub(super) fn kura_and_state_aligned_for_block(
    kura_has_block: bool,
    state_height: usize,
    state_tip_hash: Option<HashOf<BlockHeader>>,
    pending_height: u64,
    pending_hash: HashOf<BlockHeader>,
) -> bool {
    if !kura_has_block {
        return false;
    }
    let Some(state_tip_hash) = state_tip_hash else {
        return false;
    };
    let state_height_u64 = u64::try_from(state_height).unwrap_or(u64::MAX);
    state_tip_hash == pending_hash && state_height_u64 >= pending_height
}

pub(super) fn reset_qcs_after_kura_abort(
    locked_qc: &mut Option<crate::sumeragi::consensus::QcHeaderRef>,
    highest_qc: &mut Option<crate::sumeragi::consensus::QcHeaderRef>,
    state: &State,
    fallback: Option<crate::sumeragi::consensus::QcHeaderRef>,
    reason: &'static str,
) {
    let fallback_qc = fallback.or_else(|| {
        state
            .view()
            .latest_block()
            .map(|block| crate::sumeragi::consensus::QcHeaderRef {
                height: block.header().height().get(),
                view: block.header().view_change_index(),
                epoch: 0,
                subject_block_hash: block.hash(),
                phase: crate::sumeragi::consensus::Phase::Commit,
            })
    });
    let (record_height, record_view, record_hash) = if let Some(qc) = fallback_qc {
        *locked_qc = Some(qc);
        *highest_qc = Some(qc);
        status::set_locked_qc(qc.height, qc.view, Some(qc.subject_block_hash));
        status::set_highest_qc(qc.height, qc.view);
        status::set_highest_qc_hash(qc.subject_block_hash);
        (qc.height, qc.view, Some(qc.subject_block_hash))
    } else {
        *locked_qc = None;
        *highest_qc = None;
        status::set_locked_qc(0, 0, None);
        status::set_highest_qc(0, 0);
        let fallback_hash = state
            .view()
            .latest_block_hash()
            .unwrap_or_else(|| HashOf::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH])));
        status::set_highest_qc_hash(fallback_hash);
        (0, 0, Some(fallback_hash))
    };
    status::record_kura_lock_reset(record_height, record_view, record_hash, reason);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KuraRetryDecision {
    Retry { attempt: u32, next_in_ms: u32 },
    Abort { attempts: u32 },
}

#[derive(Debug)]
pub(super) struct KuraStoreFailure {
    pub(super) pending: Option<PendingBlock>,
    pub(super) clean_block_hash: bool,
}

impl Actor {
    #[allow(clippy::too_many_arguments)]
    #[cfg_attr(not(feature = "telemetry"), allow(unused_variables))]
    pub(super) fn handle_kura_store_failure(
        mut pending: PendingBlock,
        committed_block: SignedBlock,
        block_hash: HashOf<BlockHeader>,
        pending_height: u64,
        pending_view: u64,
        now: Instant,
        retry_interval: Duration,
        retry_max_attempts: u32,
        queue: &Queue,
        state: &State,
        telemetry: Option<&crate::telemetry::Telemetry>,
    ) -> KuraStoreFailure {
        status::record_kura_store_failure(pending_height, pending_view, block_hash);
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = telemetry {
            telemetry.inc_kura_store_failure("retry");
        }
        let decision = pending.note_kura_failure(now, retry_interval, retry_max_attempts);
        pending.block = committed_block;
        match decision {
            KuraRetryDecision::Retry {
                attempt,
                next_in_ms,
            } => {
                status::record_kura_store_retry(attempt, next_in_ms);
                #[cfg(feature = "telemetry")]
                if let Some(telemetry) = telemetry {
                    telemetry.set_kura_store_retry(u64::from(attempt), u64::from(next_in_ms));
                }
                warn!(
                    height = pending_height,
                    view = pending_view,
                    block = %block_hash,
                    attempt,
                    backoff_ms = next_in_ms,
                    "kura persistence failed; will retry after backoff"
                );
                KuraStoreFailure {
                    pending: Some(pending),
                    clean_block_hash: false,
                }
            }
            KuraRetryDecision::Abort { attempts } => {
                status::inc_kura_store_abort();
                status::record_kura_store_retry(attempts, 0);
                #[cfg(feature = "telemetry")]
                if let Some(telemetry) = telemetry {
                    telemetry.set_kura_store_retry(u64::from(attempts), 0);
                    telemetry.inc_kura_store_failure("abort");
                }
                warn!(
                    height = pending_height,
                    view = pending_view,
                    block = %block_hash,
                    attempts,
                    "kura persistence retries exhausted; requeuing block payload"
                );
                let (_requeued, _failures, _duplicates, _) = super::requeue_block_transactions(
                    queue,
                    state,
                    pending.block.external_entrypoints_cloned().collect(),
                );
                KuraStoreFailure {
                    pending: None,
                    clean_block_hash: true,
                }
            }
        }
    }
}
