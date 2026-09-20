//! Move-owned physical WAL work for the same process-lived native reducer.

use std::sync::Arc;

use super::{LaneInstance, LaneInstanceError, LaneService, Result, bad, reducer};
use crate::sumeragi::{
    output_guard::ConsensusOutputGuard,
    v2_lane_wal::{LaneSafetyWal, LaneWalError},
    v2_lane_wire::LaneWalEnvelopeV1,
};

/// Immutable issued obligation shared by its owner ticket and physical job.
/// This is evidence for one reducer effect, not another view/lock/vote state.
pub(super) struct IssuedPersistence {
    pub(super) effect: reducer::Effect,
    pub(super) native: LaneWalEnvelopeV1,
}

/// Exact reason no physical handle transfers at this call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LanePersistenceWait {
    NoWork,
    WorkerInFlight,
    ControlCompletion,
    EffectCapacity,
}

/// Admission transfers one actual job; a wait retains the issued effect in place.
pub(crate) enum LanePersistenceLaunch {
    Job(LanePersistenceJob),
    Wait(LanePersistenceWait),
}

/// Sole physical WAL handle plus exact reducer/native input for one append.
///
/// The scheduler must reserve its bounded worker capacity before taking this
/// job. A failed channel handoff returns this same job to its caller; it must
/// not be dropped/reconstructed or acknowledged as if persistence completed.
#[must_use]
pub(crate) struct LanePersistenceJob {
    issued: Arc<IssuedPersistence>,
    wal: Option<LaneSafetyWal>,
    guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}

/// Private fsync result and physical handle. Neither result nor ticket can be
/// fabricated by a caller; every normal error returns the same physical owner.
#[must_use]
pub(crate) struct LanePersistenceCompletion {
    issued: Arc<IssuedPersistence>,
    wal: Option<LaneSafetyWal>,
    result: Option<std::result::Result<reducer::Event, LaneWalError>>,
    guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
impl std::fmt::Debug for LanePersistenceCompletion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LanePersistenceCompletion")
            .finish_non_exhaustive()
    }
}
impl Drop for LanePersistenceJob {
    fn drop(&mut self) {
        if self.armed {
            self.guard.close_admission_for_restart();
        }
    }
}
impl Drop for LanePersistenceCompletion {
    fn drop(&mut self) {
        if self.armed {
            self.guard.close_admission_for_restart();
        }
    }
}
impl LanePersistenceJob {
    /// Run on a bounded disk worker without State/MV/publication guards.
    /// Already-admitted work drains even if its instance closes concurrently.
    pub(crate) fn run(mut self) -> LanePersistenceCompletion {
        // Construction owns exactly one handle. Keep the whole armed job alive
        // through any unwinding; append_issued owns physical uncertainty rules.
        let result = self
            .wal
            .as_mut()
            .map(|wal| wal.append_issued(&self.issued.effect, &self.issued.native));
        if !matches!(result, Some(Ok(_))) {
            // The worker already knows the physical operation failed. Fence
            // productive output before this error waits in any result channel.
            self.guard.close_admission_for_restart();
        }
        let completion = LanePersistenceCompletion {
            issued: Arc::clone(&self.issued),
            wal: self.wal.take(),
            result,
            guard: Arc::clone(&self.guard),
            armed: true,
        };
        self.armed = false;
        completion
    }
}

impl Drop for LaneInstance {
    fn drop(&mut self) {
        if self.persistence.is_some() {
            // Dropping the table owner cannot orphan an admitted physical job
            // while another owner continues signing under the process guard.
            self.output_guard.close_admission_for_restart();
        }
    }
}

impl LaneInstance {
    /// Borrow the actual outstanding worker obligation, including result transit.
    /// There is no guessed busy flag or copied persistence generation.
    pub(crate) fn persistence_in_flight(&self) -> Option<&reducer::Effect> {
        self.persistence.as_ref().map(|issued| &issued.effect)
    }

    /// Move the sole physical handle and an already-issued Persist to a worker.
    /// No disk I/O or State lease occurs here. Existing admission remains valid
    /// for physical drain after authenticated instance closure.
    pub(crate) fn take_persistence_job(&mut self) -> Result<LanePersistenceLaunch> {
        self.check_open()?;
        if self.persistence.is_some() {
            return Ok(LanePersistenceLaunch::Wait(
                LanePersistenceWait::WorkerInFlight,
            ));
        }
        if self.completion.is_some() {
            return Ok(LanePersistenceLaunch::Wait(
                LanePersistenceWait::ControlCompletion,
            ));
        }
        let Some(index) = self
            .held
            .iter()
            .position(|held| matches!(held.effect, reducer::Effect::Persist { .. }))
        else {
            return Ok(LanePersistenceLaunch::Wait(LanePersistenceWait::NoWork));
        };
        if !self.reserve_step() {
            return Ok(LanePersistenceLaunch::Wait(
                LanePersistenceWait::EffectCapacity,
            ));
        }
        if self.wal.is_none() || self.held[index].native.is_none() {
            self.failed = true;
            self.output_guard.close_admission_for_restart();
            return Err(bad("issued persistence lost physical or native custody"));
        }
        // All fallible checks precede the move. These Options are private and
        // exclusively borrowed; no concurrent path can remove either owner.
        let Some(wal) = self.wal.take() else {
            return Err(bad("physical WAL changed during exclusive transfer"));
        };
        let held = self.held.remove(index);
        let Some(native) = held.native else {
            self.wal = Some(wal);
            self.failed = true;
            self.output_guard.close_admission_for_restart();
            return Err(bad("native record changed during exclusive transfer"));
        };
        let issued = Arc::new(IssuedPersistence {
            effect: held.effect,
            native,
        });
        let job = LanePersistenceJob {
            issued: Arc::clone(&issued),
            wal: Some(wal),
            guard: Arc::clone(&self.output_guard),
            armed: true,
        };
        self.persistence = Some(issued);
        Ok(LanePersistenceLaunch::Job(job))
    }

    /// Return physical custody before queuing an exact shared-reducer ack.
    /// Foreign completion returns intact to its actual owner. A matching append
    /// error permanently fences output; it never creates a retryable live WAL.
    /// This drain does not require an open/current State instance. Productive
    /// signing/output remain gated separately after the real Persisted event.
    pub(crate) fn finish_persistence_job(
        &mut self,
        mut completed: LanePersistenceCompletion,
    ) -> std::result::Result<LaneService, (LaneInstanceError, LanePersistenceCompletion)> {
        if !self
            .persistence
            .as_ref()
            .is_some_and(|issued| Arc::ptr_eq(issued, &completed.issued))
        {
            return Err((bad("foreign native persistence completion"), completed));
        }
        if self.wal.is_some()
            || self.completion.is_some()
            || completed.wal.is_none()
            || completed.result.is_none()
        {
            self.failed = true;
            self.output_guard.close_admission_for_restart();
            return Err((
                bad("matching persistence result contradicts retained physical custody"),
                completed,
            ));
        }
        self.wal = completed.wal.take();
        completed.armed = false;
        let result = completed.result.take();
        let event = match result {
            Some(Ok(event)) => event,
            Some(Err(error)) => {
                self.failed = true;
                self.output_guard.close_admission_for_restart();
                // The ticket keeps exact failed-effect evidence for diagnosis.
                return Err((bad(error), completed));
            }
            None => {
                self.failed = true;
                self.output_guard.close_admission_for_restart();
                return Err((
                    bad("matching persistence completion lost its result"),
                    completed,
                ));
            }
        };
        let matches = match (&completed.issued.effect, &event) {
            (
                reducer::Effect::Persist {
                    tag: expected,
                    entry,
                },
                reducer::Event::Persisted { tag, id },
            ) => tag == expected && *id == entry.id(),
            _ => false,
        };
        if !matches {
            self.failed = true;
            self.output_guard.close_admission_for_restart();
            return Err((
                bad("native fsync completion differs from exact issued effect"),
                completed,
            ));
        }
        self.native_records.push(completed.issued.native.clone());
        self.completion = Some(event);
        self.persistence = None;
        Ok(LaneService::PersistedAwaitingAck)
    }
}
