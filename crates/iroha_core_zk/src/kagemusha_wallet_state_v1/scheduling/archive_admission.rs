//! One fresh Archive reservation, ordered with requested exchanges in the shared scheduler.
//!
//! Only Native supplies source-verified queue/head facts. A public turn owns proof priority,
//! not dispatch permission. Unresolved dispatched work stays fenced after a dropped permit.

use super::{Arc, PaymentGuard, Scheduler};

/// Local Archive admission failure; no variant is a financial or proof verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ArchiveAdmissionErrorV1 {
    /// The exact scheduler has closed.
    #[error("Archive owner closed")]
    Closed,
    /// A capability belongs to another scheduler.
    #[error("foreign Archive owner")]
    Foreign,
    /// Another turn, proof-priority owner or unresolved Archive needs to finish.
    #[error("Archive maintenance busy")]
    Busy,
    /// A requested/open exchange or one of its accepted children blocks fresh dispatch.
    #[error("an exchange is open or requested")]
    ExchangeOpen,
    /// The source-verified current fold backlog is nonzero.
    #[error("Archive requires an empty fold backlog")]
    FoldBacklog,
    /// No fresh charging, count or age alternative admits this queued item.
    #[error("deferred Archive is not due")]
    NotDue,
    /// Nonreused turn identities are exhausted.
    #[error("Archive turn identities exhausted")]
    Exhausted,
    /// The attempt no longer owns the observed turn/phase.
    #[error("stale Archive attempt")]
    Stale,
    /// Source reconciliation contradicts this exact admitted target or selected head.
    #[error("inconsistent Archive reconciliation")]
    InvalidResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::kagemusha_wallet_state_v1) struct ArchiveHeadV1 {
    pub sequence: u128,
    pub head: [u8; 32],
    pub capsule: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::kagemusha_wallet_state_v1) struct ArchiveTargetV1 {
    pub request_id: [u8; 32],
    pub source: ArchiveHeadV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::kagemusha_wallet_state_v1) enum ArchiveCoverageV1 {
    Unfolded,
    Covered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::kagemusha_wallet_state_v1) enum ArchiveResolutionV1 {
    Unselected {
        source: ArchiveHeadV1,
    },
    SelectedPending {
        capsule: [u8; 32],
    },
    SelectedReleased {
        head: ArchiveHeadV1,
        coverage: ArchiveCoverageV1,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Attempt {
    id: u64,
    target: ArchiveTargetV1,
    selected: Option<[u8; 32]>,
}

#[derive(Default, Clone, Copy)]
enum Phase {
    #[default]
    Clear,
    Executing(Attempt),
    Reconciling(Attempt),
    AwaitingFold {
        attempt: Attempt,
        head: ArchiveHeadV1,
    },
}

#[derive(Default)]
pub(super) struct ArchiveState {
    next: u64,
    turn: Option<u64>,
    phase: Phase,
}

impl ArchiveState {
    pub(super) fn blocks_speculation(&self) -> bool {
        self.turn.is_some() || !matches!(self.phase, Phase::Clear)
    }
}

/// Exclusive preparation/proof turn, acquired before locking the wallet.
///
/// The coordinator's bounded drain consumes this value and releases its Payment priority
/// before returning. Possession alone never permits fresh Archive dispatch. Never hold this
/// turn while waiting for the Archive's covering fold or for a user interaction.
pub struct ArchiveTurnV1 {
    scheduler: Scheduler,
    id: u64,
    _priority: PaymentGuard,
}

pub(in crate::kagemusha_wallet_state_v1) struct ArchiveDispatchPermit {
    scheduler: Scheduler,
    attempt: Attempt,
}

/// Opaque observation of one unresolved attempt, never independently an admission capability.
pub(in crate::kagemusha_wallet_state_v1) struct ArchiveReconciliationV1 {
    scheduler: Scheduler,
    attempt: Attempt,
    head: Option<ArchiveHeadV1>,
}

impl Scheduler {
    /// Wait briefly for a maintenance wake hint without owning wallet or proof priority.
    ///
    /// The wait is capped at one second. A notification, spurious wake or timeout is never
    /// readiness: the caller must re-read Native's selected state before acting. The bound
    /// also covers a publication racing before the wait or occurring outside this scheduler.
    /// Never call while holding a wallet lock, Payment guard or Archive turn.
    ///
    /// # Errors
    /// The exact scheduler has closed; no later activity update can reopen it.
    pub fn wait_for_maintenance_change(
        &self,
        timeout: std::time::Duration,
    ) -> Result<(), ArchiveAdmissionErrorV1> {
        let state = self.0.lock();
        if state.closed {
            return Err(ArchiveAdmissionErrorV1::Closed);
        }
        let (state, _) = self
            .0
            .idle
            .wait_timeout(state, timeout.min(std::time::Duration::from_secs(1)))
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.closed {
            Err(ArchiveAdmissionErrorV1::Closed)
        } else {
            Ok(())
        }
    }

    /// Prepare one candidate turn outside wallet ownership, joining existing proof work.
    /// Native must still re-read current custody and reserve dispatch after obtaining the wallet.
    ///
    /// # Errors
    /// Closed owner, requested exchange, busy maintenance/priority, or exhausted identities.
    pub fn prepare_archive_turn(&self) -> Result<ArchiveTurnV1, ArchiveAdmissionErrorV1> {
        let id = {
            let mut state = self.0.lock();
            if state.closed {
                return Err(ArchiveAdmissionErrorV1::Closed);
            }
            if !state.exchanges.is_empty() {
                return Err(ArchiveAdmissionErrorV1::ExchangeOpen);
            }
            if state.payments != 0
                || state.archive.turn.is_some()
                || !matches!(state.archive.phase, Phase::Clear)
            {
                return Err(ArchiveAdmissionErrorV1::Busy);
            }
            let id = state
                .archive
                .next
                .checked_add(1)
                .ok_or(ArchiveAdmissionErrorV1::Exhausted)?;
            state.archive.next = id;
            state.archive.turn = Some(id);
            id
        };
        let turn = ArchiveTurnV1 {
            scheduler: self.clone(),
            id,
            _priority: self.payment(),
        };
        // Close may have won while an existing proof released its work. Drop joins nothing.
        let closed = self.0.lock().closed;
        if closed {
            return Err(ArchiveAdmissionErrorV1::Closed);
        }
        Ok(turn)
    }

    pub(in crate::kagemusha_wallet_state_v1) fn reserve_archive(
        &self,
        turn: &ArchiveTurnV1,
        target: ArchiveTargetV1,
        count: u64,
        age_due: bool,
        backlog: u128,
    ) -> Result<ArchiveDispatchPermit, ArchiveAdmissionErrorV1> {
        if !Arc::ptr_eq(&self.0, &turn.scheduler.0) {
            return Err(ArchiveAdmissionErrorV1::Foreign);
        }
        let mut state = self.0.lock();
        if state.closed {
            return Err(ArchiveAdmissionErrorV1::Closed);
        }
        if state.archive.turn != Some(turn.id) {
            return Err(ArchiveAdmissionErrorV1::Stale);
        }
        if !matches!(state.archive.phase, Phase::Clear) {
            return Err(ArchiveAdmissionErrorV1::Busy);
        }
        if state.payments != 1 {
            return Err(ArchiveAdmissionErrorV1::Busy);
        }
        if !state.exchanges.is_empty() {
            return Err(ArchiveAdmissionErrorV1::ExchangeOpen);
        }
        if backlog != 0 {
            return Err(ArchiveAdmissionErrorV1::FoldBacklog);
        }
        if count == 0 || !(state.charging || count >= 16 || age_due) {
            return Err(ArchiveAdmissionErrorV1::NotDue);
        }
        if target.request_id == [0; 32]
            || target.source.head == [0; 32]
            || target.source.capsule == [0; 32]
            || target.source.sequence == u128::MAX
        {
            return Err(ArchiveAdmissionErrorV1::InvalidResolution);
        }
        let attempt = Attempt {
            id: turn.id,
            target,
            selected: None,
        };
        state.archive.phase = Phase::Executing(attempt);
        drop(state);
        self.0.idle.notify_all();
        Ok(ArchiveDispatchPermit {
            scheduler: self.clone(),
            attempt,
        })
    }

    pub(in crate::kagemusha_wallet_state_v1) fn require_archive_clear(
        &self,
    ) -> Result<(), ArchiveAdmissionErrorV1> {
        let state = self.0.lock();
        if state.closed {
            return Err(ArchiveAdmissionErrorV1::Closed);
        }
        if matches!(state.archive.phase, Phase::Clear) {
            Ok(())
        } else {
            Err(ArchiveAdmissionErrorV1::Busy)
        }
    }

    pub(in crate::kagemusha_wallet_state_v1) fn settle_archive(
        &self,
        permit: ArchiveDispatchPermit,
        resolution: ArchiveResolutionV1,
    ) -> Result<(), ArchiveAdmissionErrorV1> {
        if !Arc::ptr_eq(&self.0, &permit.scheduler.0) {
            return Err(ArchiveAdmissionErrorV1::Foreign);
        }
        self.resolve_archive(permit.attempt, None, resolution, true)
    }

    pub(in crate::kagemusha_wallet_state_v1) fn reconciling_archive(
        &self,
    ) -> Option<ArchiveReconciliationV1> {
        let state = self.0.lock();
        if state.closed {
            return None;
        }
        let (attempt, head) = match state.archive.phase {
            Phase::Reconciling(attempt) => (attempt, None),
            Phase::AwaitingFold { attempt, head } => (attempt, Some(head)),
            Phase::Clear | Phase::Executing(_) => return None,
        };
        Some(ArchiveReconciliationV1 {
            scheduler: self.clone(),
            attempt,
            head,
        })
    }

    pub(in crate::kagemusha_wallet_state_v1) fn reconcile_archive(
        &self,
        observation: &ArchiveReconciliationV1,
        resolution: ArchiveResolutionV1,
    ) -> Result<(), ArchiveAdmissionErrorV1> {
        if !Arc::ptr_eq(&self.0, &observation.scheduler.0) {
            return Err(ArchiveAdmissionErrorV1::Foreign);
        }
        self.resolve_archive(observation.attempt, observation.head, resolution, false)
    }

    fn resolve_archive(
        &self,
        observed: Attempt,
        observed_head: Option<ArchiveHeadV1>,
        resolution: ArchiveResolutionV1,
        executing: bool,
    ) -> Result<(), ArchiveAdmissionErrorV1> {
        let mut state = self.0.lock();
        if state.closed {
            return Err(ArchiveAdmissionErrorV1::Closed);
        }
        let (attempt, held_head) = match state.archive.phase {
            Phase::Executing(attempt) if executing => (attempt, None),
            Phase::Reconciling(attempt) if !executing => (attempt, None),
            Phase::AwaitingFold { attempt, head } if !executing => (attempt, Some(head)),
            _ => return Err(ArchiveAdmissionErrorV1::Stale),
        };
        if attempt != observed || held_head != observed_head {
            return Err(ArchiveAdmissionErrorV1::Stale);
        }
        let phase = match resolution {
            ArchiveResolutionV1::Unselected { source } => {
                if source != attempt.target.source
                    || attempt.selected.is_some()
                    || held_head.is_some()
                {
                    return Err(ArchiveAdmissionErrorV1::InvalidResolution);
                }
                Phase::Clear
            }
            ArchiveResolutionV1::SelectedPending { capsule } => {
                if capsule == [0; 32]
                    || held_head.is_some()
                    || attempt.selected.is_some_and(|held| held != capsule)
                {
                    return Err(ArchiveAdmissionErrorV1::InvalidResolution);
                }
                Phase::Reconciling(Attempt {
                    selected: Some(capsule),
                    ..attempt
                })
            }
            ArchiveResolutionV1::SelectedReleased { head, coverage } => {
                if attempt.target.source.sequence.checked_add(1) != Some(head.sequence)
                    || head.head == [0; 32]
                    || head.capsule == [0; 32]
                    || head.head == attempt.target.source.head
                    || head.capsule == attempt.target.source.capsule
                    || attempt.selected.is_some_and(|held| held != head.capsule)
                    || held_head.is_some_and(|held| held != head)
                {
                    return Err(ArchiveAdmissionErrorV1::InvalidResolution);
                }
                match coverage {
                    ArchiveCoverageV1::Covered => Phase::Clear,
                    ArchiveCoverageV1::Unfolded => Phase::AwaitingFold {
                        attempt: Attempt {
                            selected: Some(head.capsule),
                            ..attempt
                        },
                        head,
                    },
                }
            }
        };
        state.archive.phase = phase;
        drop(state);
        self.0.idle.notify_all();
        Ok(())
    }
}

impl ArchiveTurnV1 {
    pub(in crate::kagemusha_wallet_state_v1) fn validate_for(
        &self,
        scheduler: &Scheduler,
    ) -> Result<(), ArchiveAdmissionErrorV1> {
        if !Arc::ptr_eq(&self.scheduler.0, &scheduler.0) {
            return Err(ArchiveAdmissionErrorV1::Foreign);
        }
        let state = scheduler.0.lock();
        if state.closed {
            return Err(ArchiveAdmissionErrorV1::Closed);
        }
        if state.archive.turn != Some(self.id) {
            return Err(ArchiveAdmissionErrorV1::Stale);
        }
        Ok(())
    }
}

impl ArchiveDispatchPermit {
    pub(in crate::kagemusha_wallet_state_v1) const fn target(&self) -> ArchiveTargetV1 {
        self.attempt.target
    }

    pub(in crate::kagemusha_wallet_state_v1) fn validate_for(
        &self,
        scheduler: &Scheduler,
        request_id: &[u8; 32],
        source: ArchiveHeadV1,
    ) -> Result<(), ArchiveAdmissionErrorV1> {
        if !Arc::ptr_eq(&self.scheduler.0, &scheduler.0) {
            return Err(ArchiveAdmissionErrorV1::Foreign);
        }
        let state = scheduler.0.lock();
        if state.closed {
            return Err(ArchiveAdmissionErrorV1::Closed);
        }
        if state.archive.turn != Some(self.attempt.id)
            || !matches!(state.archive.phase, Phase::Executing(held) if held == self.attempt)
        {
            return Err(ArchiveAdmissionErrorV1::Stale);
        }
        if self.attempt.target.request_id != *request_id || self.attempt.target.source != source {
            return Err(ArchiveAdmissionErrorV1::InvalidResolution);
        }
        Ok(())
    }
}

impl ArchiveReconciliationV1 {
    pub(in crate::kagemusha_wallet_state_v1) const fn target(&self) -> ArchiveTargetV1 {
        self.attempt.target
    }
    pub(in crate::kagemusha_wallet_state_v1) const fn selected_capsule(&self) -> Option<[u8; 32]> {
        self.attempt.selected
    }
    pub(in crate::kagemusha_wallet_state_v1) const fn selected_head(
        &self,
    ) -> Option<ArchiveHeadV1> {
        self.head
    }
}

impl Drop for ArchiveDispatchPermit {
    fn drop(&mut self) {
        let mut state = self.scheduler.0.lock();
        if matches!(state.archive.phase, Phase::Executing(held) if held == self.attempt) {
            state.archive.phase = Phase::Reconciling(self.attempt);
        }
        drop(state);
        self.scheduler.0.idle.notify_all();
    }
}

impl Drop for ArchiveTurnV1 {
    fn drop(&mut self) {
        let mut state = self.scheduler.0.lock();
        if state.archive.turn == Some(self.id) {
            state.archive.turn = None;
        }
        drop(state);
        self.scheduler.0.idle.notify_all();
        // _priority drops after this body, releasing proof priority outside the state lock.
    }
}

#[cfg(test)]
mod tests;
