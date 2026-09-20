//! Move-owned physical opening/replay, followed by fresh publication-gated adoption.
//! No file operation runs in prepare/adopt; the same recovered reducer owns safety.
//! TODO: admit these jobs/drains through the bounded process table and worker pool
//! before native activation; no production runner calls this boundary yet.

use super::{
    LaneClock, LaneCurrentGate, LaneInstance, LaneInstanceError, Result, bad, body, reducer,
};
use crate::{
    kura::Kura,
    state::{State, VerifiedLaneContext, VerifiedLaneContexts},
    sumeragi::{
        output_guard::ConsensusOutputGuard,
        v2_lane_body_store::LaneBodyStore,
        v2_lane_wal::{LaneSafetyWal, RecoveredLaneWal},
        v2_runtime::round_timeout_for_view,
    },
};
use iroha_crypto::KeyPair;
use iroha_data_model::block::consensus_v2::HeightContextId;
use iroha_model_base::peer::PeerId;
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

// Immutable routing identity only. No key secret, mutable view, vote or lock copy.
struct IssuedOpening {
    state_owner: crate::state::NativeLaneStateOwner,
    instance: HeightContextId,
    signer: u32,
    peer: PeerId,
    kura: Arc<Kura>,
}

struct OpeningResources {
    state_owner: crate::state::NativeLaneStateOwner,
    // One immutable allocation follows this accepted opening into its instance;
    // body jobs and retirement tokens borrow it through shared ownership.
    verified: Arc<VerifiedLaneContext>,
    key: KeyPair,
    now: Instant,
    base_timeout: Duration,
    retransmit_interval: Duration,
    effect_limit: usize,
    wal: Option<LaneSafetyWal>,
    body: Option<LaneBodyStore>,
}
impl OpeningResources {
    fn open_remaining(&mut self) -> Result<RecoveredLaneWal> {
        let wal = self
            .wal
            .as_ref()
            .ok_or_else(|| bad("opening lost its physical WAL"))?;
        self.body = Some(wal.open_body_store().map_err(bad)?);
        wal.recover_with_native(reducer::Generation::new(0))
            .map_err(bad)
    }
    fn into_dormant(
        mut self,
        recovered: RecoveredLaneWal,
        guard: Arc<ConsensusOutputGuard>,
    ) -> std::result::Result<Box<LaneInstance>, (LaneInstanceError, Self)> {
        let (reducer, native_records) = recovered.into_parts();
        let tag = reducer.current_tag();
        let deadlines = LaneInstance::deadline(
            self.now,
            round_timeout_for_view(self.base_timeout, tag.view()),
        )
        .and_then(|timeout| {
            LaneInstance::deadline(self.now, self.retransmit_interval)
                .map(|retransmit| (timeout, retransmit))
        });
        let (timeout, retransmit) = match deadlines {
            Ok(deadlines) => deadlines,
            Err(error) => return Err((error, self)),
        };
        if self.wal.is_none() || self.body.is_none() {
            return Err((bad("completed opening lost a physical owner"), self));
        }
        Ok(Box::new(LaneInstance {
            state_owner: self.state_owner,
            verified: self.verified,
            reducer,
            wal: self.wal.take(),
            persistence: None,
            body_store: self.body.take(),
            body: body::BodyCustody::default(),
            native_records,
            timeout_witnesses: BTreeMap::new(),
            key: self.key,
            output_guard: guard,
            clock: LaneClock {
                tag,
                timeout: Some(timeout),
                retransmit,
            },
            base_timeout: self.base_timeout,
            retransmit_interval: self.retransmit_interval,
            held: Vec::new(),
            retired: std::collections::VecDeque::new(),
            completion: None,
            effect_limit: self.effect_limit,
            failed: false,
        }))
    }
}

enum OpeningResult {
    // Resume has NOT been offered. Productive access is private until adoption.
    Dormant(Box<LaneInstance>),
    Failed {
        error: LaneInstanceError,
        resources: Option<OpeningResources>,
    },
}

/// One table-side opening obligation. The actual job/completion owns all disk work.
/// The future process table must reserve this exact instance/key once before issue.
#[must_use]
pub(crate) struct LaneOpening {
    issued: Arc<IssuedOpening>,
    guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
/// Sole opening job. A full worker queue must return and retain this same value.
/// Dropping it is an abnormal loss of an admitted instance/key obligation.
#[must_use]
pub(crate) struct LaneOpeningJob {
    #[cfg(test)]
    after_wal_open: Option<Box<dyn FnOnce() + Send>>,
    issued: Arc<IssuedOpening>,
    resources: Option<OpeningResources>,
    guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
/// Exact private replay result plus all successfully opened physical handles.
#[must_use]
pub(crate) struct LaneOpeningCompletion {
    issued: Arc<IssuedOpening>,
    result: Option<OpeningResult>,
    guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
/// Explicit physical drain on authenticated closure or terminal opening failure.
/// Run this on the disk worker; it grants no completion to the shared reducer.
#[must_use]
pub(crate) struct LaneOpeningDrain {
    issued: Arc<IssuedOpening>,
    result: Option<OpeningResult>,
    guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
/// Handle-drop completion for the exact closed/failed opening, never an Apply receipt.
pub(crate) struct LaneOpeningDrained {
    instance: HeightContextId,
    signer: u32,
}
/// Productive adoption, exact retained observation wait, or explicit physical drain.
pub(crate) enum LaneOpeningAdoption {
    Opened(Box<LaneInstance>),
    ObservationChanged {
        opening: LaneOpening,
        completion: LaneOpeningCompletion,
    },
    Closed(LaneOpeningDrain),
    Failed {
        error: LaneInstanceError,
        drain: LaneOpeningDrain,
    },
}

macro_rules! armed_drop {
    ($($name:ident),+) => { $(impl Drop for $name {
        fn drop(&mut self) {
            if self.armed { self.guard.close_admission_for_restart(); }
        }
    })+ };
}
armed_drop!(
    LaneOpening,
    LaneOpeningJob,
    LaneOpeningCompletion,
    LaneOpeningDrain
);
impl std::fmt::Debug for LaneOpeningCompletion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LaneOpeningCompletion")
            .field("instance", &self.issued.instance)
            .field("signer", &self.issued.signer)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for LaneOpening {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LaneOpening")
            .field("instance", &self.issued.instance)
            .field("signer", &self.issued.signer)
            .finish_non_exhaustive()
    }
}
impl LaneOpeningDrained {
    /// Exact immutable routing identity whose opening handles were drained.
    pub(crate) fn instance_id(&self) -> HeightContextId {
        self.instance
    }
    /// Frozen local signer index associated with those physical owners.
    pub(crate) fn signer(&self) -> u32 {
        self.signer
    }
}
impl LaneOpeningDrain {
    /// Release real opened/partial handles off the process control loop.
    pub(crate) fn run(mut self) -> LaneOpeningDrained {
        drop(self.result.take());
        self.armed = false;
        LaneOpeningDrained {
            instance: self.issued.instance,
            signer: self.issued.signer,
        }
    }
}
impl LaneOpeningJob {
    /// Borrow the original accepted context before physical opening/adoption.
    #[cfg(test)]
    pub(super) fn context_for_test(&self) -> &Arc<VerifiedLaneContext> {
        &self
            .resources
            .as_ref()
            .expect("queued opening resources")
            .verified
    }

    /// Hold the real worker after WAL open but before body open/authenticated replay.
    #[cfg(test)]
    pub(crate) fn after_wal_open_for_test(mut self, hook: impl FnOnce() + Send + 'static) -> Self {
        self.after_wal_open = Some(Box::new(hook));
        self
    }
    /// Run on a reserved bounded worker, without State or publication guards.
    /// Work already admitted drains even if global membership changes meanwhile.
    pub(crate) fn run(mut self) -> LaneOpeningCompletion {
        let result = match self.resources.take() {
            Some(mut resources) => {
                let recovered =
                    LaneSafetyWal::open(&self.issued.kura, &resources.verified, self.issued.signer)
                        .map_err(bad)
                        .and_then(|wal| {
                            resources.wal = Some(wal);
                            #[cfg(test)]
                            if let Some(after_open) = self.after_wal_open.take() {
                                after_open();
                            }
                            resources.open_remaining()
                        });
                match recovered {
                    Ok(recovered) => {
                        match resources.into_dormant(recovered, Arc::clone(&self.guard)) {
                            Ok(owner) => OpeningResult::Dormant(owner),
                            Err((error, resources)) => OpeningResult::Failed {
                                error,
                                resources: Some(resources),
                            },
                        }
                    }
                    Err(error) => OpeningResult::Failed {
                        error,
                        resources: Some(resources),
                    },
                }
            }
            None => OpeningResult::Failed {
                error: bad("opening job lost exact resources"),
                resources: None,
            },
        };
        if matches!(result, OpeningResult::Failed { .. }) {
            self.guard.close_admission_for_restart();
        }
        let completed = LaneOpeningCompletion {
            issued: Arc::clone(&self.issued),
            result: Some(result),
            guard: Arc::clone(&self.guard),
            armed: true,
        };
        self.armed = false;
        completed
    }
}
impl LaneOpening {
    /// Exact instance identifier for process-table routing, not signing authority.
    pub(crate) fn instance_id(&self) -> HeightContextId {
        self.issued.instance
    }
    /// Exact frozen signer whose physical open/replay is retained by this ticket.
    pub(crate) fn signer(&self) -> u32 {
        self.issued.signer
    }

    /// Consume matching completion only under a fresh exact State publication gate.
    /// Foreign completion returns both owners intact. Stale observations also
    /// retain both owners; closure transfers handles for explicit worker drain.
    pub(crate) fn adopt(
        mut self,
        state: &State,
        observed: &VerifiedLaneContexts,
        mut completed: LaneOpeningCompletion,
    ) -> std::result::Result<LaneOpeningAdoption, (LaneInstanceError, Self, LaneOpeningCompletion)>
    {
        if !Arc::ptr_eq(&self.issued, &completed.issued) {
            return Err((bad("foreign native opening completion"), self, completed));
        }
        if !self.issued.state_owner.matches_state(state)
            || !state.matches_kura_instance(&self.issued.kura)
        {
            return Err((
                bad("opening adoption has a foreign State storage owner"),
                self,
                completed,
            ));
        }
        let Some(result) = completed.result.as_ref() else {
            self.guard.close_admission_for_restart();
            return Err((
                bad("matching opening completion lost its result"),
                self,
                completed,
            ));
        };
        // Known physical failure needs no live membership to return/drain handles.
        if let OpeningResult::Failed { .. } = result {
            let Some(OpeningResult::Failed { error, resources }) = completed.result.take() else {
                self.guard.close_admission_for_restart();
                return Err((
                    bad("opening error changed under exclusive ownership"),
                    self,
                    completed,
                ));
            };
            let drain = self.transfer_drain(
                &mut completed,
                OpeningResult::Failed {
                    error: bad(error.to_string()),
                    resources,
                },
            );
            return Ok(LaneOpeningAdoption::Failed { error, drain });
        }
        if self.guard.restart_required() {
            let Some(result) = completed.result.take() else {
                return Err((bad("fenced opening lost its result"), self, completed));
            };
            let drain = self.transfer_drain(&mut completed, result);
            return Ok(LaneOpeningAdoption::Failed {
                error: bad("consensus output is closed"),
                drain,
            });
        }
        let _lease = state.consensus_publication_lease();
        let Some(OpeningResult::Dormant(owner)) = completed.result.as_ref() else {
            self.guard.close_admission_for_restart();
            return Err((
                bad("opening result changed under exclusive ownership"),
                self,
                completed,
            ));
        };
        let gate = LaneInstance::gate_for(&owner.verified, state, observed);
        if gate == LaneCurrentGate::ObservationChanged {
            return Ok(LaneOpeningAdoption::ObservationChanged {
                opening: self,
                completion: completed,
            });
        }
        if gate == LaneCurrentGate::InstanceClosed {
            let Some(result) = completed.result.take() else {
                self.guard.close_admission_for_restart();
                return Err((
                    bad("closed opening lost its physical result"),
                    self,
                    completed,
                ));
            };
            return Ok(LaneOpeningAdoption::Closed(
                self.transfer_drain(&mut completed, result),
            ));
        }
        if owner.verified.instance_id() != self.issued.instance
            || owner.key.public_key() != self.issued.peer.public_key()
            || owner
                .verified
                .frozen()
                .committee
                .get(self.issued.signer as usize)
                != Some(&self.issued.peer)
        {
            self.guard.close_admission_for_restart();
            return Err((
                bad("opening key differs from exact current frozen authority"),
                self,
                completed,
            ));
        }
        let guard = Arc::clone(&self.guard);
        let Some(operation) = guard.begin_fail_stop_operation() else {
            let Some(result) = completed.result.take() else {
                self.guard.close_admission_for_restart();
                return Err((
                    bad("fenced opening lost its physical result"),
                    self,
                    completed,
                ));
            };
            let drain = self.transfer_drain(&mut completed, result);
            return Ok(LaneOpeningAdoption::Failed {
                error: bad("consensus output is closed"),
                drain,
            });
        };
        let Some(OpeningResult::Dormant(mut owner)) = completed.result.take() else {
            self.guard.close_admission_for_restart();
            return Err((
                bad("matching opening changed before adoption"),
                self,
                completed,
            ));
        };
        let tag = owner.tag();
        if let Err(error) = owner.step(reducer::Event::ResumeAfterReplay { tag }, None) {
            let drain = self.transfer_drain(&mut completed, OpeningResult::Dormant(owner));
            return Ok(LaneOpeningAdoption::Failed { error, drain });
        }
        self.armed = false;
        completed.armed = false;
        operation.complete();
        Ok(LaneOpeningAdoption::Opened(owner))
    }
    fn transfer_drain(
        &mut self,
        completed: &mut LaneOpeningCompletion,
        result: OpeningResult,
    ) -> LaneOpeningDrain {
        let drain = LaneOpeningDrain {
            issued: Arc::clone(&self.issued),
            result: Some(result),
            guard: Arc::clone(&self.guard),
            armed: true,
        };
        self.armed = false;
        completed.armed = false;
        drain
    }
}

impl LaneInstance {
    /// Validate/reserve one exact opening without touching the filesystem.
    /// The caller supplies its existing actual Kura owner and retains both return
    /// values across queue pressure. One process-table instance/key reservation
    /// must precede this call; this module does not create a parallel registry.
    pub(crate) fn prepare_opening(
        state: &State,
        observed: &VerifiedLaneContexts,
        verified: &VerifiedLaneContext,
        kura: Arc<Kura>,
        key: KeyPair,
        output_guard: Arc<ConsensusOutputGuard>,
        now: Instant,
        base_timeout: Duration,
        retransmit_interval: Duration,
        effect_limit: usize,
    ) -> Result<(LaneOpening, LaneOpeningJob)> {
        if base_timeout.is_zero()
            || retransmit_interval.is_zero()
            || effect_limit < 3 * reducer::MAX_EFFECTS_PER_STEP
        {
            return Err(bad("invalid clock or complete effect reservation capacity"));
        }
        Self::preflight_clock(now, base_timeout, retransmit_interval)?;
        if !state.matches_kura_instance(&kura) {
            return Err(bad("opening has a foreign Kura owner"));
        }
        let signer = verified
            .frozen()
            .committee
            .iter()
            .position(|peer| peer.public_key() == key.public_key())
            .and_then(|index| u32::try_from(index).ok())
            .ok_or_else(|| bad("key is outside frozen committee"))?;
        let _lease = state.consensus_publication_lease();
        if Self::gate_for(verified, state, observed) != LaneCurrentGate::Current {
            return Err(bad("opening observation is no longer current"));
        }
        let guard = Arc::clone(&output_guard);
        let Some(operation) = guard.begin_fail_stop_operation() else {
            return Err(bad("consensus output is closed"));
        };
        let state_owner = state.native_lane_state_owner();
        let issued = Arc::new(IssuedOpening {
            state_owner: state_owner.clone(),
            instance: verified.instance_id(),
            signer,
            peer: PeerId::new(key.public_key().clone()),
            kura,
        });
        let ticket = LaneOpening {
            issued: Arc::clone(&issued),
            guard: Arc::clone(&output_guard),
            armed: true,
        };
        let job = LaneOpeningJob {
            #[cfg(test)]
            after_wal_open: None,
            issued,
            resources: Some(OpeningResources {
                state_owner,
                verified: Arc::new(verified.clone()),
                key,
                now,
                base_timeout,
                retransmit_interval,
                effect_limit,
                wal: None,
                body: None,
            }),
            guard: output_guard,
            armed: true,
        };
        operation.complete();
        Ok((ticket, job))
    }

    /// Explicit threaded fixture pump through the sole production opening path.
    #[cfg(test)]
    pub(crate) fn open_with_worker_for_test(
        state: &State,
        observed: &VerifiedLaneContexts,
        verified: &VerifiedLaneContext,
        key: KeyPair,
        output_guard: Arc<ConsensusOutputGuard>,
        now: Instant,
        base_timeout: Duration,
        retransmit_interval: Duration,
        effect_limit: usize,
    ) -> Result<Self> {
        let (opening, job) = Self::prepare_opening(
            state,
            observed,
            verified,
            state.kura_handle(),
            key,
            output_guard,
            now,
            base_timeout,
            retransmit_interval,
            effect_limit,
        )?;
        let completed = std::thread::scope(|scope| {
            scope
                .spawn(move || job.run())
                .join()
                .map_err(|_| bad("opening worker panicked"))
        })?;
        match opening.adopt(state, observed, completed) {
            Ok(LaneOpeningAdoption::Opened(owner)) => Ok(*owner),
            Ok(LaneOpeningAdoption::Closed(drain)) => {
                std::thread::scope(|scope| scope.spawn(move || drain.run()).join())
                    .map_err(|_| bad("opening drain panicked"))?;
                Err(bad("instance closed during fixture opening"))
            }
            Ok(LaneOpeningAdoption::Failed { error, drain }) => {
                std::thread::scope(|scope| scope.spawn(move || drain.run()).join())
                    .map_err(|_| bad("opening drain panicked"))?;
                Err(error)
            }
            Ok(LaneOpeningAdoption::ObservationChanged { .. }) => {
                Err(bad("fixture opening observation changed"))
            }
            Err((error, _, _)) => Err(error),
        }
    }
}
