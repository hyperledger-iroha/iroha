//! Pending-block state, local validation, and DA availability helpers.

use std::time::{Duration, Instant};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::{BlockHeader, SignedBlock};
use iroha_logger::prelude::*;

use super::kura::KuraRetryDecision;
use crate::{
    sumeragi::{consensus::Evidence, da, status},
    tx::AcceptedTransaction,
};

/// Local validation lifecycle for a pending block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ValidationStatus {
    Pending,
    Valid,
    Invalid,
}

#[derive(Debug)]
pub(super) enum ValidationGateOutcome {
    Valid,
    Deferred,
    Invalid {
        hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        reason: String,
        reason_label: &'static str,
        evidence: Option<Box<Evidence>>,
    },
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GateComputation {
    pub(super) current: Option<da::GateReason>,
    pub(super) satisfaction: Option<da::GateSatisfaction>,
    pub(super) changed: bool,
}

#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub(super) struct PendingBlock {
    pub(super) block: SignedBlock,
    pub(super) payload_hash: Hash,
    pub(super) height: u64,
    pub(super) view: u64,
    pub(super) tx_batch: Option<Vec<AcceptedTransaction<'static>>>,
    pub(super) last_gate: Option<da::GateReason>,
    pub(super) last_gate_satisfied: Option<da::GateSatisfaction>,
    pub(super) inserted_at: Instant,
    pub(super) precommit_vote_sent: bool,
    pub(super) commit_certificate_seen: bool,
    pub(super) commit_certificate_epoch: Option<u64>,
    pub(super) validation_status: ValidationStatus,
    pub(super) kura_retry_attempts: u32,
    pub(super) next_kura_retry: Option<Instant>,
    pub(super) kura_aborted: bool,
    pub(super) kura_persisted: bool,
    pub(super) aborted: bool,
    pub(super) last_quorum_reschedule: Option<Instant>,
    pub(super) last_precommit_rebroadcast: Option<Instant>,
}

impl PendingBlock {
    pub(super) fn new(block: SignedBlock, payload_hash: Hash, height: u64, view: u64) -> Self {
        Self {
            block,
            payload_hash,
            height,
            view,
            tx_batch: None,
            last_gate: None,
            last_gate_satisfied: None,
            inserted_at: Instant::now(),
            precommit_vote_sent: false,
            commit_certificate_seen: false,
            commit_certificate_epoch: None,
            validation_status: ValidationStatus::Pending,
            kura_retry_attempts: 0,
            next_kura_retry: None,
            kura_aborted: false,
            kura_persisted: false,
            aborted: false,
            last_quorum_reschedule: None,
            last_precommit_rebroadcast: None,
        }
    }

    #[allow(dead_code)]
    pub(super) fn with_transactions(
        block: SignedBlock,
        payload_hash: Hash,
        height: u64,
        view: u64,
        tx_batch: Vec<AcceptedTransaction<'static>>,
    ) -> Self {
        let mut pending = Self::new(block, payload_hash, height, view);
        pending.tx_batch = Some(tx_batch);
        pending
    }

    #[allow(dead_code)]
    pub(super) fn take_tx_batch(&mut self) -> Option<Vec<AcceptedTransaction<'static>>> {
        self.tx_batch.take()
    }

    pub(super) fn replace_block(
        &mut self,
        block: SignedBlock,
        payload_hash: Hash,
        height: u64,
        view: u64,
    ) {
        let replacing_same_subject =
            self.payload_hash == payload_hash && self.height == height && self.view == view;
        self.block = block;
        self.payload_hash = payload_hash;
        self.height = height;
        self.view = view;
        if !replacing_same_subject {
            self.inserted_at = Instant::now();
            self.precommit_vote_sent = false;
            self.commit_certificate_seen = false;
            self.commit_certificate_epoch = None;
            self.last_gate = None;
            self.last_gate_satisfied = None;
            self.reset_kura_retry();
            self.kura_persisted = false;
            self.validation_status = ValidationStatus::Pending;
            self.last_precommit_rebroadcast = None;
            self.last_quorum_reschedule = None;
            self.aborted = false;
        }
    }

    pub(super) fn reschedule_due(&self, now: Instant, backoff: Duration) -> bool {
        self.last_quorum_reschedule
            .is_none_or(|last| now.saturating_duration_since(last) >= backoff)
    }

    pub(super) fn mark_quorum_reschedule(&mut self, now: Instant) {
        self.last_quorum_reschedule = Some(now);
    }

    pub(super) fn recompute_gate(
        &mut self,
        da_enabled: bool,
        missing_local_data: bool,
    ) -> GateComputation {
        // Availability evidence is advisory; consensus only records missing local data.
        let gate = da::evaluate(da_enabled, missing_local_data);
        let previous_gate = self.last_gate;
        let satisfaction = da::gate_satisfaction(previous_gate, gate);
        if let Some(satisfied) = satisfaction {
            self.last_gate_satisfied = Some(satisfied);
        }
        let changed = previous_gate != gate;
        if previous_gate != gate {
            status::record_da_gate_transition(previous_gate, gate);
        }
        self.last_gate = gate;
        GateComputation {
            current: gate,
            satisfaction,
            changed,
        }
    }

    pub(super) fn reset_kura_retry(&mut self) {
        self.kura_retry_attempts = 0;
        self.next_kura_retry = None;
        self.kura_aborted = false;
    }

    pub(super) fn mark_kura_persisted(&mut self) {
        self.kura_persisted = true;
        self.reset_kura_retry();
    }

    pub(super) fn mark_aborted(&mut self) {
        self.aborted = true;
        self.reset_kura_retry();
        self.last_gate = None;
        self.last_gate_satisfied = None;
        self.precommit_vote_sent = false;
        self.commit_certificate_seen = false;
        self.commit_certificate_epoch = None;
        self.last_precommit_rebroadcast = None;
    }

    pub(super) fn note_kura_failure(
        &mut self,
        now: Instant,
        interval: Duration,
        max_attempts: u32,
    ) -> KuraRetryDecision {
        if max_attempts == 0 {
            self.kura_aborted = true;
            return KuraRetryDecision::Abort {
                attempts: self.kura_retry_attempts,
            };
        }

        self.kura_retry_attempts = self.kura_retry_attempts.saturating_add(1);
        let backoff_shift = self.kura_retry_attempts.saturating_sub(1).min(16);
        let multiplier = 1u32.checked_shl(backoff_shift).unwrap_or(u32::MAX).max(1);
        let backoff = interval.saturating_mul(multiplier);
        let deadline = match now.checked_add(backoff) {
            Some(deadline) => deadline,
            None => {
                warn!(
                    height = self.height,
                    view = self.view,
                    attempts = self.kura_retry_attempts,
                    backoff_ms = backoff.as_millis(),
                    "kura retry backoff overflow; aborting retries"
                );
                self.kura_aborted = true;
                self.next_kura_retry = None;
                return KuraRetryDecision::Abort {
                    attempts: self.kura_retry_attempts,
                };
            }
        };
        self.next_kura_retry = Some(deadline);

        if self.kura_retry_attempts >= max_attempts {
            self.kura_aborted = true;
            KuraRetryDecision::Abort {
                attempts: self.kura_retry_attempts,
            }
        } else {
            let next_in_ms = backoff.as_millis().try_into().unwrap_or(u32::MAX);
            KuraRetryDecision::Retry {
                attempt: self.kura_retry_attempts,
                next_in_ms,
            }
        }
    }

    pub(super) fn kura_retry_due(&self, now: Instant) -> bool {
        !self.kura_aborted && self.next_kura_retry.is_none_or(|deadline| now >= deadline)
    }

    pub(super) fn age(&self) -> Duration {
        self.inserted_at.elapsed()
    }

    pub(super) fn precommit_rebroadcast_due(&self, now: Instant, cooldown: Duration) -> bool {
        self.last_precommit_rebroadcast
            .is_none_or(|last| now.saturating_duration_since(last) >= cooldown)
    }

    pub(super) fn mark_precommit_rebroadcast(&mut self, now: Instant) {
        self.last_precommit_rebroadcast = Some(now);
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct DaGateStatus {
    pub(super) reason: Option<da::GateReason>,
    pub(super) satisfaction: Option<da::GateSatisfaction>,
    pub(super) changed: bool,
    pub(super) da_enabled: bool,
}

pub(super) fn recompute_da_gate_status(
    pending: &mut PendingBlock,
    da_enabled: bool,
    missing_local_data: bool,
) -> DaGateStatus {
    let gate = pending.recompute_gate(da_enabled, missing_local_data);
    DaGateStatus {
        reason: gate.current,
        satisfaction: gate.satisfaction,
        changed: gate.changed,
        da_enabled,
    }
}

pub(super) fn record_da_gate_telemetry(
    telemetry: Option<&crate::telemetry::Telemetry>,
    gate: &DaGateStatus,
) {
    let reason_label = gate
        .reason
        .map(status::DaGateReasonSnapshot::from)
        .map_or("none", status::DaGateReasonSnapshot::as_str);
    if gate.changed {
        if gate.reason.is_some() {
            debug!(
                reason = reason_label,
                da_enabled = gate.da_enabled,
                "DA availability still missing (advisory)"
            );
        } else {
            debug!(
                da_enabled = gate.da_enabled,
                "DA availability evidence observed"
            );
        }
    }
    if let Some(satisfied) = gate.satisfaction {
        let satisfied_label = status::DaGateSatisfactionSnapshot::from(satisfied).as_str();
        debug!(
            satisfied = satisfied_label,
            reason = reason_label,
            da_enabled = gate.da_enabled,
            "DA availability condition satisfied"
        );
    }
    if let Some(telemetry) = telemetry {
        if gate.changed {
            telemetry.set_da_gate_last_reason(gate.reason);
            if let Some(reason) = gate.reason {
                telemetry.note_da_gate_block(reason);
            }
        }
        if let Some(satisfaction) = gate.satisfaction {
            telemetry.note_da_gate_satisfaction(satisfaction);
        }
    }
}
