//! Sealed production authentication for lifecycle planner inputs.
use super::{
    CapacityClass, LifecycleCoordinator, LifecycleState, LifecycleWorkClass,
    LifecycleWorkRegistryHolder, PreparedLifecycleIngressSelector, ProductionLifecycleOwnerV1,
    schema::{AttestedReadyValidateDemand, SchedulerInputs, SchedulerReadyInputs},
    selector::{
        PreparedCertifiedServeExactDequeueV1,
        RecoveredDecisionFetchBodyPersistencePreparationFailureV1,
    },
    work_registry::{
        ClaimedCertifiedServeDispatchErrorV1, ClaimedCertifiedServeDispatchV1,
        ClaimedProducerTurnErrorV1, ClaimedProducerTurnV1, ConcreteLifecycleWorkRegistry,
        ReadyCertifiedServeAttestationV1, ReadyLifecycleBroadcastCarrierV1,
        ReadyProducerTurnCensusAttestationErrorV1, ReadyRecoveredDecisionApplyDemand,
        ReadyRetainedDirectBroadcastAttestationV1,
    },
};
#[cfg(test)]
use crate::sumeragi::v2_runner::LifecycleRunnerRankSnapshot;
use crate::sumeragi::{
    v2::VerifiedHeightContext,
    v2_effects::{
        LifecycleModeRankSnapshot, RecoveredDecisionFetchRequestRegistrationErrorV1,
        RecoveredDecisionFetchResponseClaimErrorV1, V2EffectExecutor,
    },
    v2_runner::{LifecycleCurrentRunnerTurn, LifecycleRunnerRankTarget},
    v2_runtime::SerializedV2Runtime,
    v2_worker::{
        AuthenticatedLifecycleIoCapacity, LifecycleIoCapacityCaptureFailure,
        LifecycleIoCapacityReservation, LifecycleIoCapacityWait, LifecycleIoCapacityWaitStatus,
        ProductionV2Services, RecoveredCompletionCapacityProbeV1,
        RecoveredLifecycleSignBroadcastOutputCaptureV1,
        RecoveredLifecycleSignCapacityCaptureErrorV1, RecoveredLifecycleSignCapacityCaptureV1,
    },
};
use std::collections::{BTreeMap, BTreeSet};
/// Capability proving that raw planner rows are assembled only inside this
/// production factory.
///
/// The field is private to this module, the type is neither `Clone` nor
/// `Default`, and no API returns it. Schema constructors require this value so
/// sibling modules cannot turn scalar debt components into scheduler authority.
#[must_use = "the sealed factory capability must be consumed into SchedulerInputs"]
pub(crate) struct AuthenticatedSchedulerInputsFactory {
    _linearity: AuthenticatedSchedulerInputsFactoryLinearity,
}
struct AuthenticatedSchedulerInputsFactoryLinearity;
impl Drop for AuthenticatedSchedulerInputsFactoryLinearity {
    fn drop(&mut self) {}
}
impl AuthenticatedSchedulerInputsFactory {
    fn new() -> Self {
        Self {
            _linearity: AuthenticatedSchedulerInputsFactoryLinearity,
        }
    }
}
/// Closed origin of the six live rank components.
///
/// A direct completion is already owned at its final typed consumer. It has no
/// live I/O admission, fair-ingress occurrence, selector episode, lane/source
/// position, or outer-runner reach. The factory derives these components; no
/// caller supplies an all-zero scalar row.
enum AuthenticatedLiveRankDebts {
    DirectRegistryCompletion,
}
fn authenticated_ready_row(
    factory: &AuthenticatedSchedulerInputsFactory,
    record: &super::LifecycleRecord,
    validate_attestation: Option<AttestedReadyValidateDemand>,
    recovered_apply_attestation: Option<
        super::work_registry::ReadyRecoveredDecisionApplyAttestation,
    >,
    recovered_sign_attestation: Option<
        super::work_registry::ReadyRecoveredLifecycleSignAttestationV1,
    >,
    recovered_fetch_attestation: Option<
        super::work_registry::ReadyRecoveredDecisionFetchAttestationV1,
    >,
    live_debts: [u64; 6],
) -> Option<SchedulerReadyInputs> {
    SchedulerReadyInputs::from_authenticated(
        factory,
        record,
        validate_attestation,
        recovered_apply_attestation,
        recovered_sign_attestation,
        recovered_fetch_attestation,
        live_debts,
    )
}
fn authenticated_waiting_fetch_ready_row(
    factory: &AuthenticatedSchedulerInputsFactory,
    record: &super::LifecycleRecord,
    fetch: super::selector::LifecycleIngressSchedulerFetchSeal,
    live_debts: [u64; 6],
) -> Option<SchedulerReadyInputs> {
    SchedulerReadyInputs::from_authenticated_waiting_fetch(factory, record, fetch, live_debts)
}
fn authenticated_certified_body_pipeline_ready_row(
    factory: &AuthenticatedSchedulerInputsFactory,
    record: &super::LifecycleRecord,
    attestation: super::work_registry::ReadyCertifiedBodyPipelineAttestationV1,
    live_debts: [u64; 6],
) -> Option<SchedulerReadyInputs> {
    SchedulerReadyInputs::from_authenticated_certified_body_pipeline(
        factory,
        record,
        attestation,
        live_debts,
    )
}
fn authenticated_certified_serve_ready_row(
    factory: &AuthenticatedSchedulerInputsFactory,
    record: &super::LifecycleRecord,
    ledger: &super::ledger::LifecycleLedgerV1,
    observation: &CertifiedServeSchedulerObservationV1,
) -> Option<SchedulerReadyInputs> {
    observation
        .attestation
        .matches_ready_record(record, ledger)
        .then(|| {
            authenticated_ready_row(
                factory,
                record,
                None,
                None,
                None,
                None,
                observation.live_debts(),
            )
        })
        .flatten()
}
fn authenticated_producer_handoff_blocked_ready_row(
    factory: &AuthenticatedSchedulerInputsFactory,
    record: &super::LifecycleRecord,
    seal: super::work_registry::ProducerHandoffBlockedReadySealV1,
) -> Option<SchedulerReadyInputs> {
    SchedulerReadyInputs::from_authenticated_producer_handoff_blocked(factory, record, seal)
}
#[allow(clippy::too_many_arguments)]
fn authenticated_ready_row_with_physical_capacity(
    factory: &AuthenticatedSchedulerInputsFactory,
    record: &super::LifecycleRecord,
    validate_attestation: Option<AttestedReadyValidateDemand>,
    recovered_apply_attestation: Option<
        super::work_registry::ReadyRecoveredDecisionApplyAttestation,
    >,
    recovered_sign_attestation: Option<
        super::work_registry::ReadyRecoveredLifecycleSignAttestationV1,
    >,
    recovered_fetch_attestation: Option<
        super::work_registry::ReadyRecoveredDecisionFetchAttestationV1,
    >,
    physical_capacity_available: bool,
    live_debts: [u64; 6],
) -> Option<SchedulerReadyInputs> {
    SchedulerReadyInputs::from_authenticated_with_physical_capacity(
        factory,
        record,
        validate_attestation,
        recovered_apply_attestation,
        recovered_sign_attestation,
        recovered_fetch_attestation,
        physical_capacity_available,
        live_debts,
    )
}
fn authenticated_retained_direct_broadcast_ready_row(
    factory: &AuthenticatedSchedulerInputsFactory,
    record: &super::LifecycleRecord,
    attestation: ReadyRetainedDirectBroadcastAttestationV1,
    live_debts: [u64; 6],
) -> Option<SchedulerReadyInputs> {
    attestation.matches_ready_record(record).then_some(())?;
    authenticated_ready_row_with_physical_capacity(
        factory, record, None, None, None, None, false, live_debts,
    )
}
fn authenticated_scheduler_inputs(
    factory: AuthenticatedSchedulerInputsFactory,
    generations: BTreeMap<super::WaitSource, u64>,
    ready: BTreeMap<u128, SchedulerReadyInputs>,
) -> SchedulerInputs {
    SchedulerInputs::from_authenticated(factory, generations, ready)
}
impl AuthenticatedLiveRankDebts {
    const fn components(self) -> [u64; 6] {
        match self {
            Self::DirectRegistryCompletion => [0; 6],
        }
    }
}
/// Sealed Ready Serve carrier plus its frozen physical scheduler debts.
///
/// The attestation hides logical and physical coordinates. The scheduler
/// derives them only by matching this observation against the complete Ready
/// census from the same LedgerV1 frame.
#[must_use = "the Certified-Serve observation has not entered scheduling"]
pub(in crate::sumeragi) struct CertifiedServeSchedulerObservationV1 {
    attestation: ReadyCertifiedServeAttestationV1,
    predecessor_debt: u64,
    selector_debt: u64,
    runner_debt: u64,
}
impl CertifiedServeSchedulerObservationV1 {
    /// Derive one Serve observation only from the service-frozen capacity,
    /// exact-dequeue, and current-runner authorities.
    pub(super) fn from_live_cuts(
        attestation: ReadyCertifiedServeAttestationV1,
        capacity: &LifecycleIoCapacityReservation<'_>,
        dequeue: &PreparedCertifiedServeExactDequeueV1<'_>,
        runner: &LifecycleCurrentRunnerTurn<'_>,
    ) -> Self {
        let factory = AuthenticatedSchedulerInputsFactory::new();
        Self {
            attestation,
            predecessor_debt: capacity.authenticated_predecessor_debt(&factory),
            selector_debt: dequeue.selector_debt(),
            runner_debt: runner.debt(),
        }
    }
    /// Bind fixture-owned scalar debts without creating production authority.
    #[cfg(test)]
    fn new(
        attestation: ReadyCertifiedServeAttestationV1,
        predecessor_debt: u64,
        selector_debt: u64,
        runner_debt: u64,
    ) -> Self {
        Self {
            attestation,
            predecessor_debt,
            selector_debt,
            runner_debt,
        }
    }
    const fn live_debts(&self) -> [u64; 6] {
        [
            0,
            self.predecessor_debt,
            self.selector_debt,
            0,
            0,
            self.runner_debt,
        ]
    }
}
/// Closed failure while authenticating and claiming one full Serve Ready census.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum CertifiedServeSchedulerClaimErrorV1 {
    /// The logical owner already latched a fail-closed condition.
    CoordinatorFaulted(super::CoordinatorFault),
    /// A prior scheduler turn still owns the sole active lease.
    UnsettledLease(super::LeaseId),
    /// The supplied LedgerV1 frame is not the exact current durable authority.
    LedgerMismatch,
    /// Ready records and the reverse Ready index are not one complete Serve census.
    InvalidReadyCensus,
    /// Observations do not bijectively attest every exact Ready record.
    InvalidAttestation,
    /// Exact authenticated inputs did not yield one Serve execution lease.
    UnexpectedPlan,
    /// The claimed logical row no longer owns its sealed durable carrier.
    InvalidClaimedCarrier(ClaimedCertifiedServeDispatchErrorV1),
}
/// Authenticate the full current Ready census, deterministically claim one
/// Serve, and consume its sealed attestation into the worker dispatch carrier.
///
/// No caller supplies logical or physical lifecycle coordinates. The immutable
/// LedgerV1 frame is the durable authority and the coordinator contributes only
/// its exact volatile Ready/lease state.
pub(in crate::sumeragi) fn claim_certified_serve_turn_v1(
    coordinator: &mut LifecycleCoordinator,
    registry: &ConcreteLifecycleWorkRegistry,
    ledger: &super::ledger::LifecycleLedgerV1,
    observations: Vec<CertifiedServeSchedulerObservationV1>,
) -> Result<ClaimedCertifiedServeDispatchV1, CertifiedServeSchedulerClaimErrorV1> {
    if let Some(fault) = coordinator.fault {
        return Err(CertifiedServeSchedulerClaimErrorV1::CoordinatorFaulted(
            fault,
        ));
    }
    if let Some(lease) = coordinator.active_lease.as_ref() {
        return Err(CertifiedServeSchedulerClaimErrorV1::UnsettledLease(
            lease.id(),
        ));
    }
    if !super::ledger::LifecycleLedgerV1::from_coordinator(coordinator)
        .is_ok_and(|current| &current == ledger)
    {
        return Err(CertifiedServeSchedulerClaimErrorV1::LedgerMismatch);
    }
    let exact_ready = coordinator
        .records
        .iter()
        .filter_map(|(ordinal, record)| (record.state == LifecycleState::Ready).then_some(*ordinal))
        .collect::<BTreeSet<_>>();
    if exact_ready.is_empty()
        || exact_ready != coordinator.ready_index
        || exact_ready.len() != observations.len()
        || exact_ready.iter().any(|ordinal| {
            coordinator
                .records
                .get(ordinal)
                .is_none_or(|record| record.work_class != LifecycleWorkClass::CertifiedServe)
        })
    {
        return Err(CertifiedServeSchedulerClaimErrorV1::InvalidReadyCensus);
    }

    let factory = AuthenticatedSchedulerInputsFactory::new();
    let mut unmatched = observations.into_iter().map(Some).collect::<Vec<_>>();
    let mut matched = BTreeMap::new();
    let mut ready = BTreeMap::new();
    for ordinal in &exact_ready {
        let record = &coordinator.records[ordinal];
        let candidates = unmatched
            .iter()
            .enumerate()
            .filter_map(|(index, observation)| {
                observation.as_ref().and_then(|observation| {
                    observation
                        .attestation
                        .matches_ready_record(record, ledger)
                        .then_some(index)
                })
            })
            .collect::<Vec<_>>();
        let [index] = candidates.as_slice() else {
            return Err(CertifiedServeSchedulerClaimErrorV1::InvalidAttestation);
        };
        let observation = unmatched[*index]
            .take()
            .expect("matched Certified-Serve observation remains present");
        let row = authenticated_certified_serve_ready_row(&factory, record, ledger, &observation)
            .ok_or(CertifiedServeSchedulerClaimErrorV1::InvalidAttestation)?;
        ready.insert(*ordinal, row);
        matched.insert(*ordinal, observation);
    }
    if unmatched.iter().any(Option::is_some) {
        return Err(CertifiedServeSchedulerClaimErrorV1::InvalidAttestation);
    }
    let inputs = authenticated_scheduler_inputs(factory, BTreeMap::new(), ready);
    let lease = match coordinator.plan_turn(inputs) {
        super::TurnPlan::Execute(lease)
            if lease.work_class() == LifecycleWorkClass::CertifiedServe =>
        {
            lease
        }
        super::TurnPlan::Execute(lease) => {
            let _ = coordinator.rollback_unpublished_turn(&lease);
            return Err(CertifiedServeSchedulerClaimErrorV1::UnexpectedPlan);
        }
        super::TurnPlan::FailClosed(fault) => {
            return Err(CertifiedServeSchedulerClaimErrorV1::CoordinatorFaulted(
                fault,
            ));
        }
        super::TurnPlan::Idle | super::TurnPlan::Waiting(_) => {
            return Err(CertifiedServeSchedulerClaimErrorV1::UnexpectedPlan);
        }
    };
    let Some(observation) = matched.remove(&lease.ordinal()) else {
        let _ = coordinator.rollback_unpublished_turn(&lease);
        return Err(CertifiedServeSchedulerClaimErrorV1::UnexpectedPlan);
    };
    let rollback = lease.clone();
    registry
        .project_claimed_certified_serve_dispatch(
            coordinator,
            ledger,
            lease,
            observation.attestation,
        )
        .map_err(|error| {
            let restored = coordinator.rollback_unpublished_turn(&rollback);
            debug_assert!(
                restored,
                "failed Serve dispatch must restore its unpublished lease"
            );
            CertifiedServeSchedulerClaimErrorV1::InvalidClaimedCarrier(error)
        })
}

/// Closed failure while authenticating and claiming the oldest Ready
/// ProducerTurn from one complete lifecycle census.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum ProducerTurnSchedulerClaimErrorV1 {
    /// The logical owner already latched a fail-closed condition.
    CoordinatorFaulted(super::CoordinatorFault),
    /// A prior scheduler turn still owns the sole active lease.
    UnsettledLease(super::LeaseId),
    /// The executor mode observation belongs to another height context.
    ForeignModeObservation,
    /// The supplied LedgerV1 frame is not the exact durable authority.
    LedgerMismatch,
    /// The registry could not seal the complete Ready census.
    Attestation(ReadyProducerTurnCensusAttestationErrorV1),
    /// The sealed complete census changed before planning.
    InvalidReadyCensus,
    /// Planning did not claim the exact oldest ProducerTurn.
    UnexpectedPlan,
    /// The claimed logical row no longer owns its exact durable carrier.
    InvalidClaimedCarrier(ClaimedProducerTurnErrorV1),
}

/// Claim the oldest Ready ProducerTurn from one registry-authenticated whole
/// census. Later Ready rows receive only the opaque proof that the older
/// ProducerHandoffBarrier makes them statically ineligible; no old worker gate
/// or barrier scalar enters this API.
pub(in crate::sumeragi) fn claim_producer_turn_v1(
    coordinator: &mut LifecycleCoordinator,
    registry: &ConcreteLifecycleWorkRegistry,
    verified: &VerifiedHeightContext,
    ledger: &super::ledger::LifecycleLedgerV1,
    mode: &LifecycleModeRankSnapshot,
    runner_debt: u64,
) -> Result<Option<ClaimedProducerTurnV1>, ProducerTurnSchedulerClaimErrorV1> {
    if let Some(fault) = coordinator.fault {
        return Err(ProducerTurnSchedulerClaimErrorV1::CoordinatorFaulted(fault));
    }
    if let Some(lease) = coordinator.active_lease.as_ref() {
        return Err(ProducerTurnSchedulerClaimErrorV1::UnsettledLease(
            lease.id(),
        ));
    }
    let context = verified.context();
    if mode.context_id() != context.id() || mode.height() != context.height {
        return Err(ProducerTurnSchedulerClaimErrorV1::ForeignModeObservation);
    }
    if !super::ledger::LifecycleLedgerV1::from_coordinator(coordinator)
        .is_ok_and(|current| &current == ledger)
    {
        return Err(ProducerTurnSchedulerClaimErrorV1::LedgerMismatch);
    }
    let Some(attestation) = registry
        .attest_ready_producer_turn_census(verified, coordinator, ledger)
        .map_err(ProducerTurnSchedulerClaimErrorV1::Attestation)?
    else {
        return Ok(None);
    };
    if !attestation.matches_ready_census(coordinator, ledger) {
        return Err(ProducerTurnSchedulerClaimErrorV1::InvalidReadyCensus);
    }
    let target = attestation.target_ordinal();
    let factory = AuthenticatedSchedulerInputsFactory::new();
    let mut ready = BTreeMap::new();
    for ordinal in &coordinator.ready_index {
        let record = coordinator
            .records
            .get(ordinal)
            .ok_or(ProducerTurnSchedulerClaimErrorV1::InvalidReadyCensus)?;
        let row = if *ordinal == target {
            authenticated_ready_row(
                &factory,
                record,
                None,
                None,
                None,
                None,
                [mode.debt(), 0, 0, 0, 0, runner_debt],
            )
        } else {
            let seal = attestation
                .blocked_ready_seal(record)
                .ok_or(ProducerTurnSchedulerClaimErrorV1::InvalidReadyCensus)?;
            authenticated_producer_handoff_blocked_ready_row(&factory, record, seal)
        }
        .ok_or(ProducerTurnSchedulerClaimErrorV1::InvalidReadyCensus)?;
        if ready.insert(*ordinal, row).is_some() {
            return Err(ProducerTurnSchedulerClaimErrorV1::InvalidReadyCensus);
        }
    }
    let inputs = authenticated_scheduler_inputs(factory, BTreeMap::new(), ready);
    let lease = match coordinator.plan_turn(inputs) {
        super::TurnPlan::Execute(lease)
            if lease.ordinal() == target
                && lease.work_class() == LifecycleWorkClass::ProducerTurn =>
        {
            lease
        }
        super::TurnPlan::Execute(lease) => {
            let _ = coordinator.rollback_unpublished_turn(&lease);
            return Err(ProducerTurnSchedulerClaimErrorV1::UnexpectedPlan);
        }
        super::TurnPlan::FailClosed(fault) => {
            return Err(ProducerTurnSchedulerClaimErrorV1::CoordinatorFaulted(fault));
        }
        super::TurnPlan::Idle | super::TurnPlan::Waiting(_) => {
            return Err(ProducerTurnSchedulerClaimErrorV1::UnexpectedPlan);
        }
    };
    let rollback = lease.clone();
    registry
        .project_claimed_producer_turn(verified, coordinator, ledger, lease, attestation)
        .map(Some)
        .map_err(|error| {
            let restored = coordinator.rollback_unpublished_turn(&rollback);
            debug_assert!(
                restored,
                "failed ProducerTurn projection must restore its unpublished lease"
            );
            ProducerTurnSchedulerClaimErrorV1::InvalidClaimedCarrier(error)
        })
}
/// Failure while the production owner authenticates one complete direct-work
/// Ready census.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProductionSchedulerInputsError {
    /// The logical owner already latched a fail-closed condition.
    CoordinatorFaulted(super::CoordinatorFault),
    /// A prior plan still owns the sole live lease.
    UnsettledLease(super::LeaseId),
    /// Ready records and the coordinator's reverse Ready index disagree.
    InvalidReadyCensus,
    /// This Ready class has no direct registry carrier classifier yet.
    UnsupportedReadyCarrier {
        /// Exact logical ordinal which could not be authenticated.
        ordinal: u128,
        /// Closed work class requiring another production observation.
        work_class: LifecycleWorkClass,
    },
    /// The exact Validate registry carrier could not be bound to this row.
    InvalidValidateCarrier {
        /// Exact logical ordinal whose concrete carrier failed validation.
        ordinal: u128,
    },
    /// The exact recovered Decision Apply carrier could not be bound to this row.
    InvalidRecoveredDecisionApplyCarrier {
        /// Exact logical ordinal whose closed Apply carrier failed validation.
        ordinal: u128,
    },
    /// The exact recovered Sign carrier could not be bound to this row.
    InvalidRecoveredLifecycleSignCarrier {
        /// Exact logical ordinal whose closed Sign carrier failed validation.
        ordinal: u128,
    },
    /// The exact recovered Decision Fetch carrier could not be bound to this row.
    InvalidRecoveredDecisionFetchCarrier {
        /// Exact logical ordinal whose closed Fetch carrier failed validation.
        ordinal: u128,
    },
    /// The exact carrier requires a service-owned bounded I/O capacity cut.
    IoCapacityObservationRequired {
        /// Exact logical ordinal awaiting the missing service observation.
        ordinal: u128,
    },
}
/// Result of one complete lifecycle-owned recovered Sign claim and dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum ProductionRecoveredLifecycleSignDispatchV1 {
    /// The same worker had no free Consensus position; nothing was claimed.
    CapacityUnavailable,
    /// The exact Sign lease now owns one dedicated queued command.
    Queued {
        /// Immutable lifecycle ordinal shared by lease and command key.
        ordinal: u128,
    },
}
/// Closed failure before or during recovered Sign dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum ProductionRecoveredLifecycleSignDispatchErrorV1 {
    /// The logical owner already latched a fail-closed condition.
    CoordinatorFaulted(super::CoordinatorFault),
    /// A prior turn still owns the sole active lease.
    UnsettledLease(super::LeaseId),
    /// The borrow-bound runner cursor was not this height's Completion turn.
    ForeignRunnerObservation,
    /// The launched executor or worker is not this owner's exact service stack.
    ForeignServiceOwner,
    /// The complete Ready census is not one exact recovered Sign.
    InvalidReadyCensus,
    /// The closed Sign carrier failed exact registry attestation.
    InvalidCarrier,
    /// The worker could not retain the exact dedicated queue position.
    Capacity(RecoveredLifecycleSignCapacityCaptureErrorV1),
    /// Planning did not return the authenticated recovered Sign lease.
    UnexpectedPlan,
    /// The claimed row could not project its exact closed Sign task.
    DispatchProjection,
    /// The reserved key and claimed carrier projection disagreed.
    ReservedCommandMismatch,
}
/// Result of one restart-safe recovered signed-Broadcast refanout attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "recovered signed Broadcast refanout result must be observed"]
pub(in crate::sumeragi) enum ProductionRecoveredLifecycleSignedBroadcastRefanoutV1 {
    /// No Ready lifecycle row currently requires this typed driver.
    None,
    /// The authenticated Ready census has no supported Broadcast to refanout.
    OtherReadyWork,
    /// The exact-output corridor was full; the Broadcast remains durably Ready.
    CapacityUnavailable,
    /// The output or volatile wait transition failed and process restart is required.
    RestartRequired,
    /// The corridor owns the fanout and the live row is parked on a volatile wait.
    Refanned {
        /// Exact durable Broadcast ordinal retained as the crash-recovery source.
        ordinal: u128,
    },
}
/// Closed failure before a recovered signed Broadcast enters exact output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1 {
    /// The logical owner already latched a fail-closed condition.
    CoordinatorFaulted(super::CoordinatorFault),
    /// A prior turn still owns the sole live lease.
    UnsettledLease(super::LeaseId),
    /// The service does not own this launched height's exact body-store worker.
    ForeignServiceOwner,
    /// Coordinator records and the reverse Ready index disagree.
    InvalidReadyCensus,
    /// The selected Broadcast or its declared next-Sign link failed authentication.
    InvalidCarrier,
    /// Planning did not return the authenticated Broadcast lease.
    UnexpectedPlan,
}
/// Result of one all-row lifecycle Completion transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "the composite lifecycle Completion dispatch result must be observed"]
pub(in crate::sumeragi) enum ProductionCompletionDispatchV1 {
    /// No physically available row was claimed; every Ready carrier remains unchanged.
    CapacityUnavailable,
    /// The selected lifecycle Validate now owns one exact durable worker command.
    ValidateQueued {
        /// Exact selected lifecycle ordinal.
        ordinal: u128,
    },
    /// The selected recovered Apply now owns one dedicated worker command.
    ApplyQueued {
        /// Exact selected lifecycle ordinal.
        ordinal: u128,
    },
    /// The selected recovered Sign now owns one dedicated worker command.
    SignQueued {
        /// Exact selected lifecycle ordinal.
        ordinal: u128,
    },
    /// The selected recovered Fetch owns its executor request and exact fanout,
    /// is parked on its external wait, and has released the active lease.
    FetchDispatched {
        /// Exact selected lifecycle ordinal.
        ordinal: u128,
    },
    /// One ordinary Fetch or Store parent was durably replaced by its child.
    BodyStageAdvanced {
        /// Exact terminalized parent ordinal.
        parent_ordinal: u128,
        /// Newly Ready child class.
        child: LifecycleWorkClass,
    },
    /// One ordinary body parent is parked on the adapter reducer fence.
    ReducerFenceWait {
        /// Exact waiting parent ordinal.
        ordinal: u128,
    },
    /// One Validate completion durably advanced without creating a child.
    ValidateNoSuccessor {
        /// Exact terminalized Validate ordinal.
        ordinal: u128,
    },
}

/// Closed failure while one mixed lifecycle Completion census is authenticated.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum ProductionCompletionDispatchErrorV1 {
    /// The logical owner already latched a fail-closed condition.
    CoordinatorFaulted(super::CoordinatorFault),
    /// A prior turn still owns the sole active lease.
    UnsettledLease(super::LeaseId),
    /// The launched executor, worker, body store, or output guard is foreign.
    ForeignServiceOwner,
    /// Coordinator records and the reverse Ready index disagree.
    InvalidReadyCensus,
    /// One Ready row failed its exact closed registry attestation.
    InvalidCarrier,
    /// Service signing or the joint physical-corridor census failed.
    Service(String),
    /// A Fetch executor owner conflicted with the exact request catalogs.
    Executor(RecoveredDecisionFetchRequestRegistrationErrorV1),
    /// Planning selected no authenticated physical row or another work class.
    UnexpectedPlan,
    /// The selected claimed carrier could not project its exact task.
    DispatchProjection,
    /// The selected physical reservation and logical carrier disagreed.
    ReservedOwnerMismatch,
}
/// Nonmutating class of Ready work visible to the unified Completion driver.
///
/// `PassThrough` covers ordinary/stateful rows. `Invalid` is reserved for a
/// broken Ready-index bijection or an already faulted owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProductionCompletionReadyWorkV1 {
    /// No Ready lifecycle record exists.
    None,
    /// One complete Apply/Sign/Fetch census must use the joint physical cut.
    CompletionIo,
    /// A recovered Broadcast has a full-census refanout transaction.
    RecoveredLifecycleBroadcast,
    /// Another ordinary/stateful owner must run.
    PassThrough,
    /// Coordinator state is not safe to classify without restart.
    Invalid,
}
enum AuthenticatedRecoveredCompletionReadyV1 {
    Validate(AttestedReadyValidateDemand),
    Apply(super::work_registry::ReadyRecoveredDecisionApplyAttestation),
    Sign(super::work_registry::ReadyRecoveredLifecycleSignAttestationV1),
    Fetch(super::work_registry::ReadyRecoveredDecisionFetchAttestationV1),
    CertifiedBody(super::work_registry::ReadyCertifiedBodyPipelineAttestationV1),
    RetainedDirectBroadcast(ReadyRetainedDirectBroadcastAttestationV1),
}

fn classify_completion_ready_classes(
    classes: &[LifecycleWorkClass],
    has_retained_direct_output: bool,
    oldest_is_retained_direct_output: bool,
) -> ProductionCompletionReadyWorkV1 {
    if oldest_is_retained_direct_output {
        return ProductionCompletionReadyWorkV1::PassThrough;
    }
    if classes.is_empty() {
        return if has_retained_direct_output {
            ProductionCompletionReadyWorkV1::PassThrough
        } else {
            ProductionCompletionReadyWorkV1::None
        };
    }
    if classes.iter().any(|class| {
        matches!(
            class,
            LifecycleWorkClass::EnterView
                | LifecycleWorkClass::EquivocationReport
                | LifecycleWorkClass::InvalidBodyReport
                | LifecycleWorkClass::CertifiedServe
                | LifecycleWorkClass::ProducerTurn
        )
    }) {
        return ProductionCompletionReadyWorkV1::PassThrough;
    }
    // Broadcast refanout has the existing full-census factory and can
    // authenticate concurrent Validate, Apply, Sign, and Fetch rows. It is
    // deliberately chosen before standalone Validate passes through.
    if classes
        .iter()
        .any(|class| *class == LifecycleWorkClass::Broadcast)
    {
        return ProductionCompletionReadyWorkV1::RecoveredLifecycleBroadcast;
    }
    if classes.iter().all(|class| {
        matches!(
            class,
            LifecycleWorkClass::Validate
                | LifecycleWorkClass::Apply
                | LifecycleWorkClass::SignVote
                | LifecycleWorkClass::SignProposal
                | LifecycleWorkClass::SignTimeout
                | LifecycleWorkClass::Fetch
                | LifecycleWorkClass::Store
        )
    }) {
        return ProductionCompletionReadyWorkV1::CompletionIo;
    }
    match classes[0] {
        LifecycleWorkClass::Validate
        | LifecycleWorkClass::Apply
        | LifecycleWorkClass::SignVote
        | LifecycleWorkClass::SignProposal
        | LifecycleWorkClass::SignTimeout
        | LifecycleWorkClass::Fetch
        | LifecycleWorkClass::Store => ProductionCompletionReadyWorkV1::CompletionIo,
        LifecycleWorkClass::Broadcast
        | LifecycleWorkClass::EnterView
        | LifecycleWorkClass::EquivocationReport
        | LifecycleWorkClass::InvalidBodyReport
        | LifecycleWorkClass::CertifiedServe
        | LifecycleWorkClass::ProducerTurn => ProductionCompletionReadyWorkV1::PassThrough,
    }
}
/// Phase-A outcome for one selected recovered Decision Fetch response.
#[must_use = "capacity wait or queued durable persistence must remain owner-visible"]
pub(crate) enum ProductionRecoveredDecisionFetchPersistenceV1 {
    /// The exact worker generation is saturated; the selector remains retained.
    CapacityWait(PreparedProductionIngressCapacityWait),
    /// The response claim and dedicated persistence command are installed.
    Queued {
        /// Lifecycle ordinal whose active Fetch lease remains claimed.
        ordinal: u128,
    },
}
/// Closed failure before recovered Decision Fetch Phase-A queue publication.
#[must_use]
pub(in crate::sumeragi) enum ProductionRecoveredDecisionFetchPersistenceErrorV1 {
    /// The coordinator row/carrier/request owner was not exact.
    InvalidClaimedCarrier,
    /// The selected response or service belonged to another height owner.
    ForeignOwner,
    /// Selector consumption failed and was restored into the queue reservation.
    CommandPreparation {
        /// Typed selector/executor failure retained for retry diagnostics.
        failure: RecoveredDecisionFetchBodyPersistencePreparationFailureV1,
        /// Unchanged selector restored from the aborted capacity reservation.
        prepared: PreparedLifecycleIngressSelector,
    },
    /// The dedicated queue owns this key without the required active lease;
    /// this impossible half-cut requires restart rather than completion wait.
    InFlightSelectedWork(PreparedLifecycleIngressSelector),
    /// Direct Ready lifecycle work must drain before this external wake.
    CompetingReadyWork(PreparedLifecycleIngressSelector),
    /// The exact task changed after capacity was retained.
    InvalidReservedCommand,
    /// The executor response-family claim drifted before the commit tail.
    Claim(RecoveredDecisionFetchResponseClaimErrorV1),
    /// Capacity capture failed without consuming the selector.
    Service {
        failure: LifecycleIoCapacityCaptureFailure,
        prepared: PreparedLifecycleIngressSelector,
    },
}
/// Opaque service-generation wait retaining every ingress observation.
#[must_use = "retry only after the same service release generation advances"]
pub(crate) struct PreparedProductionIngressCapacityWait {
    mode: LifecycleModeRankSnapshot,
    wait: LifecycleIoCapacityWait,
    selector: PreparedLifecycleIngressSelector,
}
/// Consuming classification of one retained production ingress capacity wait.
#[allow(variant_size_differences)]
#[must_use = "pending capacity ownership must be reparked or retried exactly once"]
pub(crate) enum ProductionIngressCapacityRetry {
    /// No release occurred; the complete wait remains owned.
    Pending(PreparedProductionIngressCapacityWait),
    /// A real release occurred and yielded the unchanged prepared selector.
    Released(PreparedLifecycleIngressSelector),
    /// The mode or exact service generation changed incompatibly.
    RestartRequired,
}
impl PreparedProductionIngressCapacityWait {
    /// Classify the exact retained service generation without exposing it.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn capacity_status(
        &self,
        services: &ProductionV2Services,
    ) -> ProductionIngressCapacityStatus {
        let _retained_joint_observations = (&self.mode, &self.selector);
        match self.wait.status(services) {
            LifecycleIoCapacityWaitStatus::SamePending => ProductionIngressCapacityStatus::Pending,
            LifecycleIoCapacityWaitStatus::Released => ProductionIngressCapacityStatus::Released,
            LifecycleIoCapacityWaitStatus::GenerationExhausted => {
                ProductionIngressCapacityStatus::GenerationExhausted
            }
            LifecycleIoCapacityWaitStatus::ForeignOrDisconnected => {
                ProductionIngressCapacityStatus::RestartRequired
            }
        }
    }

    /// Consume this wait only after checking the exact executor and service.
    ///
    /// The retained selector has no getter. A real release is the sole branch
    /// which yields it for another Phase-A attempt; pending ownership returns
    /// whole, while terminal/foreign generations require cold recovery.
    pub(crate) fn retry(
        self,
        services: &ProductionV2Services,
        executor: &V2EffectExecutor<SerializedV2Runtime>,
    ) -> ProductionIngressCapacityRetry {
        if self.mode != executor.lifecycle_mode_rank_snapshot() {
            return ProductionIngressCapacityRetry::RestartRequired;
        }
        match self.wait.status(services) {
            LifecycleIoCapacityWaitStatus::SamePending => {
                ProductionIngressCapacityRetry::Pending(self)
            }
            LifecycleIoCapacityWaitStatus::Released => {
                let Self {
                    mode: _,
                    wait: _,
                    selector,
                } = self;
                ProductionIngressCapacityRetry::Released(selector)
            }
            LifecycleIoCapacityWaitStatus::GenerationExhausted
            | LifecycleIoCapacityWaitStatus::ForeignOrDisconnected => {
                ProductionIngressCapacityRetry::RestartRequired
            }
        }
    }
}
/// Opaque status of one service-owned capacity-generation wait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProductionIngressCapacityStatus {
    /// No release has advanced the retained generation.
    Pending,
    /// A real release advanced the retained generation; a fresh cut may retry.
    Released,
    /// The exact service generation can no longer advance and must fail closed.
    GenerationExhausted,
    /// The retained wait no longer names a live service and requires restart.
    RestartRequired,
}
/// Exact successful result after the target lease was blocked behind its
/// physically queued persistence command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct QueuedProductionIngressFetch {
    ordinal: u128,
}
impl QueuedProductionIngressFetch {
    /// Return the exact lifecycle ordinal now blocked behind its queued command.
    pub(crate) const fn ordinal(self) -> u128 {
        self.ordinal
    }
}
/// Opaque outcome of the complete production ingress plan/submit/settle cut.
#[must_use = "queued work or its service-generation wait must be retained"]
#[allow(variant_size_differences, clippy::large_enum_variant)]
pub(crate) enum ProductionIngressTurnPreparation {
    /// The target capacity generation has not advanced yet.
    CapacityWait(PreparedProductionIngressCapacityWait),
    /// One exact persistence command now precedes the blocked Fetch lease.
    Queued(QueuedProductionIngressFetch),
}
/// Failure before a complete ingress-bearing scheduler precursor is sealed.
#[must_use = "typed pre-plan rejection or fail-stop post-plan failure must be handled"]
#[allow(variant_size_differences, clippy::large_enum_variant)]
pub(crate) enum ProductionIngressSchedulerInputsError {
    /// The owner already latched a fail-closed condition.
    CoordinatorFaulted {
        /// Retained fail-closed coordinator classification.
        _fault: super::CoordinatorFault,
    },
    /// Another exact lease still owns coordinator execution.
    UnsettledLease {
        /// Retained conflicting lease identity.
        _lease: super::LeaseId,
    },
    /// The executor mode observation belongs to another height context.
    ForeignModeObservation,
    /// The executor mode observation no longer equals the live executor state.
    StaleModeObservation,
    /// The service fail-stop gate is not the executor's canonical output gate.
    ForeignOutputGuard,
    /// The runner observation is foreign or does not name the Ingress turn.
    ForeignRunnerObservation,
    /// The I/O worker does not own the exact body store recovered by this owner.
    BodyStoreNotBound,
    /// The selected family did not bind the exact waiting Fetch and registry incumbent.
    InvalidSelectedCarrier,
    /// Direct Ready work must drain before the sole ingress wake is published.
    CompetingReadyWork,
    /// The selector could not be consumed into its exact persistence command.
    CommandPreparation {
        /// Complete selector retained unchanged for rollback.
        _prepared: PreparedLifecycleIngressSelector,
    },
    /// The exact selected work id is already queued, active, or completion-pending.
    InFlightSelectedWork {
        /// Complete selector retained behind the duplicate fence.
        _prepared: PreparedLifecycleIngressSelector,
    },
    /// The consumed command no longer matched its reserved queue identity.
    InvalidReservedCommand,
    /// The sole prospective target did not produce its exact lease.
    UnexpectedPlan,
    /// Blocking the submitted lease violated coordinator invariants.
    SettlementFault {
        /// Retained fail-closed settlement classification.
        _fault: super::CoordinatorFault,
    },
    /// The live service could not issue a capacity reservation or generation wait.
    Service {
        /// Closed reason the service rejected the capture.
        _failure: LifecycleIoCapacityCaptureFailure,
        /// Complete selector with its one-shot target restored.
        _prepared: PreparedLifecycleIngressSelector,
    },
}
/// Authenticate the complete subset of Ready work already owned directly by
/// the concrete registry.
///
/// The current sound subset consists of closed Validate completion carriers and
/// the exact recovered Decision Apply carrier. I/O-bearing Validate and Apply
/// rows are classified here but rejected before planning because their capacity
/// and outer-runner reach still need a service-minted joint observation. Other
/// work classes remain closed until their concrete carrier classifier exists.
fn direct_registry_scheduler_inputs(
    coordinator: &LifecycleCoordinator,
    registry: &LifecycleWorkRegistryHolder,
) -> Result<SchedulerInputs, ProductionSchedulerInputsError> {
    if let Some(fault) = coordinator.fault {
        return Err(ProductionSchedulerInputsError::CoordinatorFaulted(fault));
    }
    if let Some(lease) = coordinator.active_lease.as_ref() {
        return Err(ProductionSchedulerInputsError::UnsettledLease(lease.id));
    }
    let exact_ready = coordinator
        .records
        .iter()
        .filter_map(|(ordinal, record)| {
            matches!(record.state, LifecycleState::Ready).then_some(*ordinal)
        })
        .collect::<BTreeSet<_>>();
    if exact_ready != coordinator.ready_index {
        return Err(ProductionSchedulerInputsError::InvalidReadyCensus);
    }
    let factory = AuthenticatedSchedulerInputsFactory::new();
    let mut ready = BTreeMap::new();
    for ordinal in &coordinator.ready_index {
        let record = coordinator
            .records
            .get(ordinal)
            .ok_or(ProductionSchedulerInputsError::InvalidReadyCensus)?;
        let row = match record.work_class {
            LifecycleWorkClass::Validate => {
                let attestation = coordinator
                    .attest_ready_validate_demand(registry, *ordinal)
                    .map_err(|_| ProductionSchedulerInputsError::InvalidValidateCarrier {
                        ordinal: *ordinal,
                    })?;
                if attestation.requires_io_dispatch() {
                    return Err(
                        ProductionSchedulerInputsError::IoCapacityObservationRequired {
                            ordinal: *ordinal,
                        },
                    );
                }
                authenticated_ready_row(
                    &factory,
                    record,
                    Some(attestation),
                    None,
                    None,
                    None,
                    AuthenticatedLiveRankDebts::DirectRegistryCompletion.components(),
                )
            }
            LifecycleWorkClass::Apply => {
                let attestation = registry
                    .attest_ready_recovered_decision_apply(coordinator, *ordinal)
                    .map_err(|_| {
                        ProductionSchedulerInputsError::InvalidRecoveredDecisionApplyCarrier {
                            ordinal: *ordinal,
                        }
                    })?;
                let ReadyRecoveredDecisionApplyDemand::BoundedIo = attestation.demand();
                return Err(
                    ProductionSchedulerInputsError::IoCapacityObservationRequired {
                        ordinal: *ordinal,
                    },
                );
            }
            LifecycleWorkClass::SignVote
            | LifecycleWorkClass::SignProposal
            | LifecycleWorkClass::SignTimeout => {
                let attestation = registry
                    .attest_ready_recovered_lifecycle_sign(coordinator, *ordinal)
                    .map_err(|_| {
                        ProductionSchedulerInputsError::InvalidRecoveredLifecycleSignCarrier {
                            ordinal: *ordinal,
                        }
                    })?;
                let super::work_registry::ReadyRecoveredLifecycleSignDemandV1::BoundedIo =
                    attestation.demand();
                return Err(
                    ProductionSchedulerInputsError::IoCapacityObservationRequired {
                        ordinal: *ordinal,
                    },
                );
            }
            LifecycleWorkClass::Fetch => {
                if let Ok(attestation) = registry
                    .registry()
                    .attest_schedulable_certified_body_pipeline(coordinator, *ordinal, None)
                {
                    authenticated_certified_body_pipeline_ready_row(
                        &factory,
                        record,
                        attestation,
                        AuthenticatedLiveRankDebts::DirectRegistryCompletion.components(),
                    )
                } else {
                    let attestation = registry
                        .attest_ready_recovered_decision_fetch(coordinator, *ordinal)
                        .map_err(|_| {
                            ProductionSchedulerInputsError::InvalidRecoveredDecisionFetchCarrier {
                                ordinal: *ordinal,
                            }
                        })?;
                    match attestation.demand() {
                        super::work_registry::ReadyRecoveredDecisionFetchDemandV1::ExactOutputAndExecutor => {
                            return Err(
                                ProductionSchedulerInputsError::IoCapacityObservationRequired {
                                    ordinal: *ordinal,
                                },
                            );
                        }
                    }
                }
            }
            LifecycleWorkClass::Store => {
                let attestation = registry
                    .registry()
                    .attest_schedulable_certified_body_pipeline(coordinator, *ordinal, None)
                    .map_err(
                        |_| ProductionSchedulerInputsError::UnsupportedReadyCarrier {
                            ordinal: *ordinal,
                            work_class: record.work_class,
                        },
                    )?;
                authenticated_certified_body_pipeline_ready_row(
                    &factory,
                    record,
                    attestation,
                    AuthenticatedLiveRankDebts::DirectRegistryCompletion.components(),
                )
            }
            _ => {
                return Err(ProductionSchedulerInputsError::UnsupportedReadyCarrier {
                    ordinal: *ordinal,
                    work_class: record.work_class,
                });
            }
        };
        let row = row
            .ok_or(ProductionSchedulerInputsError::InvalidValidateCarrier { ordinal: *ordinal })?;
        if ready.insert(*ordinal, row).is_some() {
            return Err(ProductionSchedulerInputsError::InvalidReadyCensus);
        }
    }
    Ok(authenticated_scheduler_inputs(
        factory,
        BTreeMap::new(),
        ready,
    ))
}
impl ProductionLifecycleOwnerV1 {
    /// Claim the oldest ProducerTurn at an activated runner's bounded producer
    /// point. This owner derives the durable frame and fixes runner reach debt
    /// to zero because the call site already owns that exact point.
    pub(in crate::sumeragi) fn claim_producer_turn_at_bounded_producer_point(
        &mut self,
        mode: &LifecycleModeRankSnapshot,
    ) -> Result<Option<ClaimedProducerTurnV1>, ProducerTurnSchedulerClaimErrorV1> {
        let ledger = super::ledger::LifecycleLedgerV1::from_coordinator(&self.coordinator)
            .map_err(|_| ProducerTurnSchedulerClaimErrorV1::LedgerMismatch)?;
        claim_producer_turn_v1(
            &mut self.coordinator,
            self.registry.registry(),
            &self.verified,
            &ledger,
            mode,
            0,
        )
    }

    fn classify_schedulable_completion_work(
        &self,
        schedulable: &BTreeSet<u128>,
    ) -> ProductionCompletionReadyWorkV1 {
        let mut retained_direct_outputs = BTreeSet::new();
        let mut classes = Vec::with_capacity(schedulable.len());
        for ordinal in schedulable {
            let Some(record) = self.coordinator.records.get(ordinal) else {
                return ProductionCompletionReadyWorkV1::Invalid;
            };
            if record.work_class != LifecycleWorkClass::Broadcast {
                classes.push(record.work_class);
                continue;
            }
            match self
                .registry
                .registry()
                .attest_ready_lifecycle_broadcast_carrier(&self.coordinator, *ordinal)
            {
                Ok(ReadyLifecycleBroadcastCarrierV1::RetainedDirectOutput(_)) => {
                    if !retained_direct_outputs.insert(*ordinal) {
                        return ProductionCompletionReadyWorkV1::Invalid;
                    }
                }
                Ok(ReadyLifecycleBroadcastCarrierV1::RecoveredRefanout) => {
                    classes.push(record.work_class);
                }
                Err(_) => return ProductionCompletionReadyWorkV1::Invalid,
            }
        }
        let oldest_is_retained_direct_output = schedulable
            .first()
            .is_some_and(|ordinal| retained_direct_outputs.contains(ordinal));
        classify_completion_ready_classes(
            &classes,
            !retained_direct_outputs.is_empty(),
            oldest_is_retained_direct_output,
        )
    }

    /// Classify Ready work without claiming a lease or reserving capacity.
    ///
    /// Broadcast refanout and recovered Apply/Sign/Fetch each authenticate their
    /// complete supported census. Stateful Serve/Producer and other ordinary
    /// rows pass through rather than turning legal coexistence into corruption.
    pub(in crate::sumeragi) fn classify_completion_ready_work(
        &self,
        fence: crate::sumeragi::v2::LifecycleReducerFenceObservationV1,
    ) -> ProductionCompletionReadyWorkV1 {
        if self.coordinator.fault.is_some() {
            return ProductionCompletionReadyWorkV1::Invalid;
        }
        if self.coordinator.active_lease.is_some() {
            return ProductionCompletionReadyWorkV1::PassThrough;
        }
        let exact_ready = self
            .coordinator
            .records
            .iter()
            .filter_map(|(ordinal, record)| {
                matches!(record.state, LifecycleState::Ready).then_some(*ordinal)
            })
            .collect::<BTreeSet<_>>();
        if exact_ready != self.coordinator.ready_index {
            return ProductionCompletionReadyWorkV1::Invalid;
        }
        let mut schedulable = exact_ready;
        schedulable.extend(
            self.coordinator
                .records
                .iter()
                .filter_map(|(ordinal, record)| {
                    matches!(
                        record.state,
                        LifecycleState::Waiting(wait)
                            if wait.source() == fence.source()
                                && wait.observed_generation() < fence.generation()
                    )
                    .then_some(*ordinal)
                }),
        );
        self.classify_schedulable_completion_work(&schedulable)
    }

    /// Plan one turn from the complete directly-owned production Ready census.
    ///
    /// No snapshot or raw rank row leaves the owner. Unsupported carriers fail
    /// before the coordinator mutates generations, Ready state, or leases.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn plan_direct_registry_turn(
        &mut self,
    ) -> Result<super::TurnPlan, ProductionSchedulerInputsError> {
        let inputs = direct_registry_scheduler_inputs(&self.coordinator, &self.registry)?;
        Ok(self.coordinator.plan_turn(inputs))
    }
    fn publish_ready_validate_outcome(
        &mut self,
        executor: &mut V2EffectExecutor<SerializedV2Runtime>,
        lease: super::TurnLease,
    ) -> Result<ProductionCompletionDispatchV1, ProductionCompletionDispatchErrorV1> {
        let ordinal = lease.ordinal();
        let Some((&slot, _)) = lease.physical_slots().first_key_value() else {
            return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
        };
        let execution = match self
            .registry
            .registry_mut()
            .prepare_ready_durable_validate_execution(&lease, slot, &self.verified)
        {
            Ok(execution) => execution,
            Err(error) => {
                iroha_logger::error!(
                    ?error,
                    ordinal,
                    "Ready Validate registry execution projection failed"
                );
                self.rollback_ready_validate_publication(&lease);
                return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
            }
        };
        let preview = match executor.prepare_ready_durable_validate_adapter_preview(execution) {
            Ok(preview) => preview,
            Err(error) => {
                drop(error);
                iroha_logger::error!(ordinal, "Ready Validate serialized adapter preview failed");
                self.rollback_ready_validate_publication(&lease);
                return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
            }
        };
        use crate::sumeragi::v2::ReadyDurableValidateAdapterPublicationKind as Kind;
        let publication_kind = preview.publication_kind();
        match publication_kind {
            Kind::ValidatedBusy | Kind::RejectedBusy => {
                let Some((context_id, generation)) = preview.busy_reducer_fence() else {
                    drop(preview);
                    self.rollback_ready_validate_publication(&lease);
                    return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
                };
                if context_id != self.verified.context().id() {
                    drop(preview);
                    self.rollback_ready_validate_publication(&lease);
                    return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
                }
                drop(preview);
                let source =
                    super::projection::reducer_fence_wait_source(self.coordinator.active_context);
                let wait = super::WaitToken::new(source, generation);
                if !self.coordinator.park_validate_on_reducer_fence(lease, wait) {
                    return Err(ProductionCompletionDispatchErrorV1::UnexpectedPlan);
                }
                Ok(ProductionCompletionDispatchV1::ReducerFenceWait { ordinal })
            }
            Kind::ValidatedInactive
            | Kind::ValidatedNoEffect
            | Kind::RejectedInactive
            | Kind::RejectedNoEffect => {
                let transition = match self
                    .coordinator
                    .prepare_sealed_validate_no_successor_transition(&lease, preview)
                {
                    Ok(transition) => transition,
                    Err(error) => {
                        drop(error);
                        iroha_logger::error!(
                            ordinal,
                            "Ready Validate no-successor transition preparation failed"
                        );
                        self.rollback_ready_validate_publication(&lease);
                        return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
                    }
                };
                if transition.persist_and_publish().is_err() {
                    iroha_logger::error!(
                        ordinal,
                        "Ready Validate no-successor transaction publication failed"
                    );
                    return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
                }
                Ok(ProductionCompletionDispatchV1::ValidateNoSuccessor { ordinal })
            }
            Kind::ValidatedPersist => {
                let publication = match preview.seal_live_wal_validate_sign() {
                    Ok(publication) => publication,
                    Err(error) => {
                        drop(error);
                        iroha_logger::error!(ordinal, "Ready Validate Sign WAL seal failed");
                        self.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
                        return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
                    }
                };
                let transition = match self.coordinator.prepare_sealed_validate_sign_transition(
                    &lease,
                    &self.verified,
                    publication,
                ) {
                    Ok(transition) => transition,
                    Err(error) => {
                        drop(error);
                        iroha_logger::error!(
                            ordinal,
                            "Ready Validate-to-Sign transition preparation failed"
                        );
                        self.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
                        return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
                    }
                };
                if transition.persist_and_publish().is_err() {
                    iroha_logger::error!(
                        ordinal,
                        "Ready Validate-to-Sign transaction publication failed"
                    );
                    return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
                }
                Ok(ProductionCompletionDispatchV1::BodyStageAdvanced {
                    parent_ordinal: ordinal,
                    child: LifecycleWorkClass::SignVote,
                })
            }
            Kind::RejectedReport => {
                let report = match preview.seal_invalid_body_report_replay() {
                    Ok(report) => report,
                    Err(error) => {
                        drop(error);
                        iroha_logger::error!(
                            ordinal,
                            "Ready Validate invalid-body report seal failed"
                        );
                        self.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
                        return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
                    }
                };
                let transition = match self.coordinator.prepare_sealed_validate_report_transition(
                    &lease,
                    &self.verified,
                    report,
                ) {
                    Ok(transition) => transition,
                    Err(error) => {
                        drop(error);
                        iroha_logger::error!(
                            ordinal,
                            "Ready Validate-to-report transition preparation failed"
                        );
                        self.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
                        return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
                    }
                };
                if transition.persist_and_publish().is_err() {
                    iroha_logger::error!(
                        ordinal,
                        "Ready Validate-to-report transaction publication failed"
                    );
                    return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
                }
                Ok(ProductionCompletionDispatchV1::BodyStageAdvanced {
                    parent_ordinal: ordinal,
                    child: LifecycleWorkClass::InvalidBodyReport,
                })
            }
            Kind::ValidatedApply => {
                let publication = match preview.seal_live_wal_validate_apply() {
                    Ok(publication) => publication,
                    Err(error) => {
                        drop(error);
                        iroha_logger::error!(ordinal, "Ready Validate Apply WAL seal failed");
                        self.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
                        return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
                    }
                };
                let transition = match self.coordinator.prepare_sealed_validate_apply_transition(
                    &lease,
                    &self.verified,
                    publication,
                ) {
                    Ok(transition) => transition,
                    Err(error) => {
                        drop(error);
                        iroha_logger::error!(
                            ordinal,
                            "Ready Validate-to-Apply transition preparation failed"
                        );
                        self.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
                        return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
                    }
                };
                if transition.persist_and_publish().is_err() {
                    iroha_logger::error!(
                        ordinal,
                        "Ready Validate-to-Apply transaction publication failed"
                    );
                    return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
                }
                Ok(ProductionCompletionDispatchV1::BodyStageAdvanced {
                    parent_ordinal: ordinal,
                    child: LifecycleWorkClass::Apply,
                })
            }
        }
    }
    fn rollback_ready_validate_publication(&mut self, lease: &super::TurnLease) {
        let rolled_back = if lease.output_reservation().is_some() {
            self.coordinator
                .rollback_unpublished_reserved_turn(lease, CapacityClass::Consensus)
        } else {
            self.coordinator.rollback_unpublished_turn(lease)
        };
        assert!(
            rolled_back,
            "unpublished Ready Validate claim must roll back exactly once"
        );
    }
    fn publish_certified_fetch_store(
        &mut self,
        services: &ProductionV2Services,
        executor: &mut V2EffectExecutor<SerializedV2Runtime>,
        lease: super::TurnLease,
    ) -> Result<ProductionCompletionDispatchV1, ProductionCompletionDispatchErrorV1> {
        let ordinal = lease.ordinal();
        let Some((&slot, _)) = lease.physical_slots().first_key_value() else {
            return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
        };
        let execution = match self
            .registry
            .registry_mut()
            .prepare_certified_fetch_execution(&lease, slot)
        {
            Ok(execution) => execution,
            Err(_) => {
                assert!(self.coordinator.rollback_unpublished_turn(&lease));
                return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
            }
        };
        let (tag, manifest) = execution.adapter_preview_inputs();
        let adapter = match executor.prepare_certified_fetch_store_adapter(tag, manifest) {
            Ok(crate::sumeragi::v2::CertifiedFetchStoreAdapterPreparationV1::Applied(adapter)) => {
                adapter
            }
            Ok(crate::sumeragi::v2::CertifiedFetchStoreAdapterPreparationV1::Blocked(wait)) => {
                let generation = wait.generation();
                if wait.context_id() != self.verified.context().id() {
                    drop(wait);
                    drop(execution);
                    assert!(self.coordinator.rollback_unpublished_turn(&lease));
                    return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
                }
                drop(wait);
                drop(execution);
                let source =
                    super::projection::reducer_fence_wait_source(self.coordinator.active_context);
                let wait = super::WaitToken::new(source, generation);
                self.coordinator
                    .settle_turn(lease, super::TurnOutcome::Blocked(wait));
                if self.coordinator.fault.is_some()
                    || self.coordinator.active_lease.is_some()
                    || self
                        .coordinator
                        .records
                        .get(&ordinal)
                        .is_none_or(|record| record.state != LifecycleState::Waiting(wait))
                {
                    return Err(ProductionCompletionDispatchErrorV1::UnexpectedPlan);
                }
                return Ok(ProductionCompletionDispatchV1::ReducerFenceWait { ordinal });
            }
            Ok(crate::sumeragi::v2::CertifiedFetchStoreAdapterPreparationV1::Inactive) | Err(_) => {
                drop(execution);
                assert!(self.coordinator.rollback_unpublished_turn(&lease));
                return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
            }
        };
        let successor = match execution.seal_store_successor(adapter) {
            Ok(successor) => successor,
            Err(_) => {
                assert!(self.coordinator.rollback_unpublished_turn(&lease));
                return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
            }
        };
        let transition = match self.coordinator.prepare_sealed_fetch_store_transition(
            &lease,
            &self.verified,
            successor,
        ) {
            Ok(transition) => transition,
            Err(_) => {
                assert!(self.coordinator.rollback_unpublished_turn(&lease));
                return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
            }
        };
        let output_guard = services.lifecycle_output_guard();
        let Some(operation) = output_guard.begin_fail_stop_operation() else {
            drop(transition);
            assert!(self.coordinator.rollback_unpublished_turn(&lease));
            return Err(ProductionCompletionDispatchErrorV1::Service(
                "certified Fetch publication output is closed".to_owned(),
            ));
        };
        if transition.persist_exact_successor().is_err() {
            drop(transition);
            self.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
            drop(operation);
            return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
        }
        transition.commit_after_publication();
        operation.complete();
        Ok(ProductionCompletionDispatchV1::BodyStageAdvanced {
            parent_ordinal: ordinal,
            child: LifecycleWorkClass::Store,
        })
    }
    fn publish_durable_store_validate(
        &mut self,
        services: &ProductionV2Services,
        executor: &mut V2EffectExecutor<SerializedV2Runtime>,
        lease: super::TurnLease,
    ) -> Result<ProductionCompletionDispatchV1, ProductionCompletionDispatchErrorV1> {
        let ordinal = lease.ordinal();
        let Some((&slot, _)) = lease.physical_slots().first_key_value() else {
            return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
        };
        let execution = match self
            .registry
            .registry_mut()
            .prepare_durable_store_execution(&lease, slot, &self.verified)
        {
            Ok(execution) => execution,
            Err(error) => {
                iroha_logger::error!(
                    ?error,
                    ordinal,
                    "lifecycle Store registry execution projection failed"
                );
                assert!(self.coordinator.rollback_unpublished_turn(&lease));
                return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
            }
        };
        let (tag, round, subject) = execution.adapter_preview_inputs();
        let adapter = match executor.prepare_durable_store_validate_adapter(
            tag,
            round,
            subject,
            execution.durable_body_receipt(),
        ) {
            Ok(crate::sumeragi::v2::DurableStoreValidateAdapterPreparationV1::Applied(adapter)) => {
                adapter
            }
            Ok(crate::sumeragi::v2::DurableStoreValidateAdapterPreparationV1::Blocked(wait)) => {
                let generation = wait.generation();
                if wait.context_id() != self.verified.context().id() {
                    drop(wait);
                    drop(execution);
                    assert!(self.coordinator.rollback_unpublished_turn(&lease));
                    return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
                }
                drop(wait);
                drop(execution);
                let source =
                    super::projection::reducer_fence_wait_source(self.coordinator.active_context);
                let wait = super::WaitToken::new(source, generation);
                self.coordinator
                    .settle_turn(lease, super::TurnOutcome::Blocked(wait));
                if self.coordinator.fault.is_some()
                    || self.coordinator.active_lease.is_some()
                    || self
                        .coordinator
                        .records
                        .get(&ordinal)
                        .is_none_or(|record| record.state != LifecycleState::Waiting(wait))
                {
                    return Err(ProductionCompletionDispatchErrorV1::UnexpectedPlan);
                }
                return Ok(ProductionCompletionDispatchV1::ReducerFenceWait { ordinal });
            }
            Ok(crate::sumeragi::v2::DurableStoreValidateAdapterPreparationV1::Inactive) => {
                iroha_logger::error!(
                    ordinal,
                    "lifecycle Store adapter preview classified the row inactive"
                );
                drop(execution);
                assert!(self.coordinator.rollback_unpublished_turn(&lease));
                return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
            }
            Err(error) => {
                iroha_logger::error!(?error, ordinal, "lifecycle Store adapter preview failed");
                drop(execution);
                assert!(self.coordinator.rollback_unpublished_turn(&lease));
                return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
            }
        };
        let successor = match execution.seal_validate_successor(adapter) {
            Ok(successor) => successor,
            Err(error) => {
                iroha_logger::error!(
                    ?error,
                    ordinal,
                    "lifecycle Store-to-Validate successor sealing failed"
                );
                assert!(self.coordinator.rollback_unpublished_turn(&lease));
                return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
            }
        };
        let transition = match self.coordinator.prepare_sealed_store_validate_transition(
            &lease,
            &self.verified,
            successor,
        ) {
            Ok(transition) => transition,
            Err(error) => {
                iroha_logger::error!(
                    ?error,
                    ordinal,
                    "lifecycle Store-to-Validate transition preparation failed"
                );
                assert!(self.coordinator.rollback_unpublished_turn(&lease));
                return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
            }
        };
        let output_guard = services.lifecycle_output_guard();
        let Some(operation) = output_guard.begin_fail_stop_operation() else {
            drop(transition);
            assert!(self.coordinator.rollback_unpublished_turn(&lease));
            return Err(ProductionCompletionDispatchErrorV1::Service(
                "durable Store publication output is closed".to_owned(),
            ));
        };
        if let Err(error) = transition.persist_exact_successor() {
            iroha_logger::error!(
                ?error,
                ordinal,
                "lifecycle Store-to-Validate LedgerV1 publication failed"
            );
            drop(transition);
            self.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
            drop(operation);
            return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
        }
        transition.commit_after_publication();
        operation.complete();
        Ok(ProductionCompletionDispatchV1::BodyStageAdvanced {
            parent_ordinal: ordinal,
            child: LifecycleWorkClass::Validate,
        })
    }
    /// Authenticate, rank, and dispatch one complete lifecycle Ready census.
    ///
    /// Apply, Sign, and Fetch rows are all attested before the service freezes
    /// its worker and exact-output corridors. The coordinator sees every row's
    /// physical availability in one snapshot and claims at most one. No caller
    /// can probe a wrong class or reobserve capacity after selection.
    pub(super) fn dispatch_completion_with_runner_debt(
        &mut self,
        services: &ProductionV2Services,
        executor: &mut V2EffectExecutor<SerializedV2Runtime>,
        runner_debt: u64,
    ) -> Result<ProductionCompletionDispatchV1, ProductionCompletionDispatchErrorV1> {
        if let Some(fault) = self.coordinator.fault {
            return Err(ProductionCompletionDispatchErrorV1::CoordinatorFaulted(
                fault,
            ));
        }
        if let Some(lease) = self.coordinator.active_lease.as_ref() {
            return Err(ProductionCompletionDispatchErrorV1::UnsettledLease(
                lease.id,
            ));
        }
        let Some(body_store_identity) = self.body_store_identity.as_ref() else {
            return Err(ProductionCompletionDispatchErrorV1::ForeignServiceOwner);
        };
        if self.body_store.is_some()
            || !services.matches_lifecycle_body_store(body_store_identity)
            || !services.matches_lifecycle_executor_output_guard(executor)
        {
            return Err(ProductionCompletionDispatchErrorV1::ForeignServiceOwner);
        }
        let current_ready = self.coordinator.ready_index.clone();
        if self
            .coordinator
            .records
            .iter()
            .filter_map(|(ordinal, record)| {
                matches!(record.state, LifecycleState::Ready).then_some(*ordinal)
            })
            .collect::<BTreeSet<_>>()
            != current_ready
        {
            return Err(ProductionCompletionDispatchErrorV1::InvalidReadyCensus);
        }
        let mode = executor.lifecycle_mode_rank_snapshot();
        let context = self.verified.context();
        if mode.height() != context.height || mode.context_id() != context.id() {
            return Err(ProductionCompletionDispatchErrorV1::ForeignServiceOwner);
        }
        let fence = executor.lifecycle_reducer_fence_observation();
        let mut exact_ready = current_ready;
        let reducer_fence_wakes = self
            .coordinator
            .records
            .iter()
            .filter_map(|(ordinal, record)| {
                matches!(
                    record.state,
                    LifecycleState::Waiting(wait)
                        if wait.source() == fence.source()
                            && wait.observed_generation() < fence.generation()
                )
                .then_some(*ordinal)
            })
            .collect::<BTreeSet<_>>();
        exact_ready.extend(reducer_fence_wakes.iter().copied());
        if exact_ready.is_empty() {
            return Err(ProductionCompletionDispatchErrorV1::InvalidReadyCensus);
        }
        let mut authenticated = BTreeMap::new();
        let mut classes = BTreeMap::new();
        let mut validate_io = BTreeSet::new();
        let mut probes = Vec::with_capacity(exact_ready.len());
        for ordinal in &exact_ready {
            let record = self
                .coordinator
                .records
                .get(ordinal)
                .ok_or(ProductionCompletionDispatchErrorV1::InvalidReadyCensus)?;
            let (ready, probe) = match record.work_class {
                LifecycleWorkClass::Validate => {
                    let attestation = self
                        .coordinator
                        .attest_ready_validate_demand(&self.registry, *ordinal)
                        .map_err(|_| ProductionCompletionDispatchErrorV1::InvalidCarrier)?;
                    let probe = attestation.requires_io_dispatch().then_some(
                        RecoveredCompletionCapacityProbeV1::Validate {
                            ordinal: *ordinal,
                            key: attestation.dispatch_key(),
                        },
                    );
                    if attestation.requires_io_dispatch() {
                        validate_io.insert(*ordinal);
                    }
                    (
                        AuthenticatedRecoveredCompletionReadyV1::Validate(attestation),
                        probe,
                    )
                }
                LifecycleWorkClass::Apply => {
                    let attestation = self
                        .registry
                        .attest_ready_recovered_decision_apply(&self.coordinator, *ordinal)
                        .map_err(|_| ProductionCompletionDispatchErrorV1::InvalidCarrier)?;
                    if attestation.demand() != ReadyRecoveredDecisionApplyDemand::BoundedIo
                        || !attestation.dispatch_key().matches_height_context(context)
                    {
                        return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
                    }
                    let key = attestation.dispatch_key();
                    (
                        AuthenticatedRecoveredCompletionReadyV1::Apply(attestation),
                        Some(RecoveredCompletionCapacityProbeV1::Apply {
                            ordinal: *ordinal,
                            key,
                        }),
                    )
                }
                LifecycleWorkClass::SignVote
                | LifecycleWorkClass::SignProposal
                | LifecycleWorkClass::SignTimeout => {
                    let attestation = self
                        .registry
                        .attest_ready_recovered_lifecycle_sign(&self.coordinator, *ordinal)
                        .map_err(|_| ProductionCompletionDispatchErrorV1::InvalidCarrier)?;
                    if attestation.demand()
                        != super::work_registry::ReadyRecoveredLifecycleSignDemandV1::BoundedIo
                        || !attestation.dispatch_key().matches_height_context(context)
                    {
                        return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
                    }
                    let key = attestation.dispatch_key();
                    (
                        AuthenticatedRecoveredCompletionReadyV1::Sign(attestation),
                        Some(RecoveredCompletionCapacityProbeV1::Sign {
                            ordinal: *ordinal,
                            key,
                        }),
                    )
                }
                LifecycleWorkClass::Fetch => {
                    if let Ok(attestation) = self
                        .registry
                        .registry()
                        .attest_schedulable_certified_body_pipeline(
                            &self.coordinator,
                            *ordinal,
                            Some(fence),
                        )
                    {
                        (
                            AuthenticatedRecoveredCompletionReadyV1::CertifiedBody(attestation),
                            None,
                        )
                    } else {
                        let mut attestation = self
                            .registry
                            .registry_mut()
                            .attest_ready_recovered_decision_fetch(&self.coordinator, *ordinal)
                            .map_err(|_| ProductionCompletionDispatchErrorV1::InvalidCarrier)?;
                        if attestation.demand()
                            != super::work_registry::ReadyRecoveredDecisionFetchDemandV1::ExactOutputAndExecutor
                            || !attestation.dispatch_key().matches_height_context(context)
                        {
                            return Err(
                                ProductionCompletionDispatchErrorV1::InvalidCarrier,
                            );
                        }
                        let dispatch_key = attestation.dispatch_key();
                        let owner = services
                            .authenticate_recovered_decision_fetch_request(
                                attestation.take_request_authority(),
                            )
                            .map_err(ProductionCompletionDispatchErrorV1::Service)?;
                        if owner.dispatch_key() != dispatch_key {
                            return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
                        }
                        let executor_available = executor
                            .recovered_decision_fetch_registration_available(&owner)
                            .map_err(ProductionCompletionDispatchErrorV1::Executor)?;
                        (
                            AuthenticatedRecoveredCompletionReadyV1::Fetch(attestation),
                            Some(RecoveredCompletionCapacityProbeV1::Fetch {
                                ordinal: *ordinal,
                                owner,
                                executor_available,
                            }),
                        )
                    }
                }
                LifecycleWorkClass::Store => {
                    let attestation = self
                        .registry
                        .registry()
                        .attest_schedulable_certified_body_pipeline(
                            &self.coordinator,
                            *ordinal,
                            Some(fence),
                        )
                        .map_err(|_| ProductionCompletionDispatchErrorV1::InvalidCarrier)?;
                    (
                        AuthenticatedRecoveredCompletionReadyV1::CertifiedBody(attestation),
                        None,
                    )
                }
                LifecycleWorkClass::Broadcast => {
                    let attestation = match self
                        .registry
                        .registry()
                        .attest_ready_lifecycle_broadcast_carrier(&self.coordinator, *ordinal)
                        .map_err(|_| ProductionCompletionDispatchErrorV1::InvalidCarrier)?
                    {
                        ReadyLifecycleBroadcastCarrierV1::RetainedDirectOutput(attestation) => {
                            attestation
                        }
                        ReadyLifecycleBroadcastCarrierV1::RecoveredRefanout => {
                            return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
                        }
                    };
                    (
                        AuthenticatedRecoveredCompletionReadyV1::RetainedDirectBroadcast(
                            attestation,
                        ),
                        None,
                    )
                }
                LifecycleWorkClass::EnterView
                | LifecycleWorkClass::EquivocationReport
                | LifecycleWorkClass::InvalidBodyReport
                | LifecycleWorkClass::CertifiedServe
                | LifecycleWorkClass::ProducerTurn => {
                    return Err(ProductionCompletionDispatchErrorV1::InvalidReadyCensus);
                }
            };
            if authenticated.insert(*ordinal, ready).is_some()
                || classes.insert(*ordinal, record.work_class).is_some()
            {
                return Err(ProductionCompletionDispatchErrorV1::InvalidReadyCensus);
            }
            if let Some(probe) = probe {
                probes.push(probe);
            }
        }
        let census = (!probes.is_empty())
            .then(|| services.capture_recovered_completion_capacity_census(probes))
            .transpose()
            .map_err(ProductionCompletionDispatchErrorV1::Service)?;
        let ordinary_body = authenticated
            .iter()
            .filter_map(|(ordinal, ready)| {
                matches!(
                    ready,
                    AuthenticatedRecoveredCompletionReadyV1::CertifiedBody(_)
                )
                .then_some(*ordinal)
            })
            .collect::<BTreeSet<_>>();
        let factory = AuthenticatedSchedulerInputsFactory::new();
        let mut ready_rows = BTreeMap::new();
        for (ordinal, ready) in authenticated {
            let record = self
                .coordinator
                .records
                .get(&ordinal)
                .ok_or(ProductionCompletionDispatchErrorV1::InvalidReadyCensus)?;
            let direct_physical = matches!(
                &ready,
                AuthenticatedRecoveredCompletionReadyV1::CertifiedBody(_)
            ) || matches!(
                &ready,
                AuthenticatedRecoveredCompletionReadyV1::Validate(attestation)
                    if !attestation.requires_io_dispatch()
            );
            let retained_direct_output = matches!(
                &ready,
                AuthenticatedRecoveredCompletionReadyV1::RetainedDirectBroadcast(_)
            );
            let (physical_available, predecessor_debt) = if retained_direct_output {
                (false, 0)
            } else if direct_physical {
                (true, 0)
            } else {
                census
                    .as_ref()
                    .and_then(|census| census.authenticated_capacity(ordinal, &factory))
                    .ok_or(ProductionCompletionDispatchErrorV1::InvalidCarrier)?
            };
            let live_debts = [mode.debt(), predecessor_debt, 0, 0, 0, runner_debt];
            let row = match ready {
                AuthenticatedRecoveredCompletionReadyV1::Validate(attestation) => {
                    authenticated_ready_row_with_physical_capacity(
                        &factory,
                        record,
                        Some(attestation),
                        None,
                        None,
                        None,
                        physical_available,
                        live_debts,
                    )
                }
                AuthenticatedRecoveredCompletionReadyV1::Apply(attestation) => {
                    authenticated_ready_row_with_physical_capacity(
                        &factory,
                        record,
                        None,
                        Some(attestation),
                        None,
                        None,
                        physical_available,
                        live_debts,
                    )
                }
                AuthenticatedRecoveredCompletionReadyV1::Sign(attestation) => {
                    authenticated_ready_row_with_physical_capacity(
                        &factory,
                        record,
                        None,
                        None,
                        Some(attestation),
                        None,
                        physical_available,
                        live_debts,
                    )
                }
                AuthenticatedRecoveredCompletionReadyV1::Fetch(attestation) => {
                    authenticated_ready_row_with_physical_capacity(
                        &factory,
                        record,
                        None,
                        None,
                        None,
                        Some(attestation),
                        physical_available,
                        live_debts,
                    )
                }
                AuthenticatedRecoveredCompletionReadyV1::CertifiedBody(attestation) => {
                    authenticated_certified_body_pipeline_ready_row(
                        &factory,
                        record,
                        attestation,
                        live_debts,
                    )
                }
                AuthenticatedRecoveredCompletionReadyV1::RetainedDirectBroadcast(attestation) => {
                    authenticated_retained_direct_broadcast_ready_row(
                        &factory,
                        record,
                        attestation,
                        live_debts,
                    )
                }
            }
            .ok_or(ProductionCompletionDispatchErrorV1::InvalidCarrier)?;
            if ready_rows.insert(ordinal, row).is_some() {
                return Err(ProductionCompletionDispatchErrorV1::InvalidReadyCensus);
            }
        }
        let generations = if reducer_fence_wakes.is_empty() {
            BTreeMap::new()
        } else {
            BTreeMap::from([(fence.source(), fence.generation())])
        };
        let inputs = authenticated_scheduler_inputs(factory, generations, ready_rows);
        let lease = match self.coordinator.plan_turn(inputs) {
            super::TurnPlan::Execute(lease) => lease,
            super::TurnPlan::Waiting(_) | super::TurnPlan::Idle => {
                if let Some(census) = census {
                    census.complete_without_selection();
                }
                return Ok(ProductionCompletionDispatchV1::CapacityUnavailable);
            }
            super::TurnPlan::FailClosed(_) => {
                return Err(ProductionCompletionDispatchErrorV1::UnexpectedPlan);
            }
        };
        let ordinal = lease.ordinal();
        let Some(expected_class) = classes.get(&ordinal).copied() else {
            return Err(ProductionCompletionDispatchErrorV1::UnexpectedPlan);
        };
        if lease.work_class() != expected_class {
            return Err(ProductionCompletionDispatchErrorV1::UnexpectedPlan);
        }
        match expected_class {
            LifecycleWorkClass::Validate => {
                if !validate_io.contains(&ordinal) {
                    if let Some(census) = census {
                        census.complete_without_selection();
                    }
                    return self.publish_ready_validate_outcome(executor, lease);
                }
                let census = census.ok_or(ProductionCompletionDispatchErrorV1::InvalidCarrier)?;
                let reservation = census
                    .select_validate(ordinal)
                    .map_err(|_| ProductionCompletionDispatchErrorV1::ReservedOwnerMismatch)?;
                let dispatch = self
                    .coordinator
                    .begin_durable_validate_dispatch(&mut self.registry, lease, &self.verified)
                    .map_err(|_| ProductionCompletionDispatchErrorV1::DispatchProjection)?;
                if !reservation.preflight(&dispatch) {
                    return Err(ProductionCompletionDispatchErrorV1::ReservedOwnerMismatch);
                }
                reservation.commit(dispatch);
                Ok(ProductionCompletionDispatchV1::ValidateQueued { ordinal })
            }
            LifecycleWorkClass::Apply => {
                let census = census.ok_or(ProductionCompletionDispatchErrorV1::InvalidCarrier)?;
                let reservation = census
                    .select_apply(ordinal)
                    .map_err(|_| ProductionCompletionDispatchErrorV1::ReservedOwnerMismatch)?;
                let prepared = self
                    .registry
                    .prepare_recovered_decision_apply_dispatch(&self.coordinator, &lease)
                    .map_err(|_| ProductionCompletionDispatchErrorV1::DispatchProjection)?;
                if !reservation.preflight(&prepared) {
                    return Err(ProductionCompletionDispatchErrorV1::ReservedOwnerMismatch);
                }
                reservation.commit(prepared);
                Ok(ProductionCompletionDispatchV1::ApplyQueued { ordinal })
            }
            LifecycleWorkClass::SignVote
            | LifecycleWorkClass::SignProposal
            | LifecycleWorkClass::SignTimeout => {
                let census = census.ok_or(ProductionCompletionDispatchErrorV1::InvalidCarrier)?;
                if !lease
                    .output_reservation()
                    .is_some_and(|reservation| reservation.class() == CapacityClass::Consensus)
                {
                    return Err(ProductionCompletionDispatchErrorV1::UnexpectedPlan);
                }
                let reservation = census
                    .select_sign(ordinal)
                    .map_err(|_| ProductionCompletionDispatchErrorV1::ReservedOwnerMismatch)?;
                let prepared = self
                    .registry
                    .prepare_recovered_lifecycle_sign_dispatch(&self.coordinator, &lease)
                    .map_err(|_| ProductionCompletionDispatchErrorV1::DispatchProjection)?;
                if !reservation.preflight(&prepared) {
                    return Err(ProductionCompletionDispatchErrorV1::ReservedOwnerMismatch);
                }
                reservation.commit(prepared);
                Ok(ProductionCompletionDispatchV1::SignQueued { ordinal })
            }
            LifecycleWorkClass::Fetch => {
                if ordinary_body.contains(&ordinal) {
                    if let Some(census) = census {
                        census.complete_without_selection();
                    }
                    return self.publish_certified_fetch_store(services, executor, lease);
                }
                let census = census.ok_or(ProductionCompletionDispatchErrorV1::InvalidCarrier)?;
                let (owner, output) = census
                    .select_fetch(ordinal)
                    .map_err(|_| ProductionCompletionDispatchErrorV1::ReservedOwnerMismatch)?;
                let registration = executor
                    .prepare_recovered_decision_fetch_request_registration(owner)
                    .map_err(ProductionCompletionDispatchErrorV1::Executor)?;
                let dispatch_key = registration.dispatch_key();
                let wait_source =
                    super::projection::certified_fetch_wait_source(registration.request_hash());
                let observed_generation = self
                    .coordinator
                    .observed_generation
                    .get(&wait_source)
                    .copied()
                    .unwrap_or(0);
                if observed_generation == u64::MAX
                    || self.coordinator.records.iter().any(|(candidate, record)| {
                        *candidate != ordinal
                            && matches!(
                                record.state,
                                LifecycleState::Waiting(wait) if wait.source() == wait_source
                            )
                    })
                    || lease.output_reservation().is_some()
                {
                    return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
                }
                let wait = super::WaitToken::new(wait_source, observed_generation);
                let mut expected_observed = self.coordinator.observed_generation.clone();
                expected_observed
                    .entry(wait_source)
                    .or_insert(observed_generation);
                let mut next = self.coordinator.stage_durable_transaction();
                next.reduce_settle_turn(lease.clone(), super::TurnOutcome::Blocked(wait), None);
                let staged_wait_is_exact = next.episode_authority
                    == self.coordinator.episode_authority
                    && next.active_context == self.coordinator.active_context
                    && next.fault.is_none()
                    && next.active_lease.is_none()
                    && next.records.len() == self.coordinator.records.len()
                    && next
                        .records
                        .get(&ordinal)
                        .is_some_and(|record| record.state == LifecycleState::Waiting(wait))
                    && self.coordinator.records.iter().all(|(candidate, record)| {
                        *candidate == ordinal || next.records.get(candidate) == Some(record)
                    })
                    && next.ready_index == self.coordinator.ready_index
                    && next.key_index == self.coordinator.key_index
                    && next.owner_index == self.coordinator.owner_index
                    && next.admission_waits == self.coordinator.admission_waits
                    && next.high_water == self.coordinator.high_water
                    && next.next_lease == self.coordinator.next_lease
                    && next.durable_records == self.coordinator.durable_records
                    && next.capacity_geometry == self.coordinator.capacity_geometry
                    && next.capacity_used == self.coordinator.capacity_used
                    && next.capacity_generation == self.coordinator.capacity_generation
                    && next.producer_debts == self.coordinator.producer_debts
                    && next.observed_generation == expected_observed
                    && next.ledger_store.is_some() == self.coordinator.ledger_store.is_some();
                if !staged_wait_is_exact {
                    return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
                }
                let prepared = self
                    .registry
                    .registry_mut()
                    .prepare_recovered_decision_fetch_dispatch(
                        &self.coordinator,
                        &lease,
                        dispatch_key,
                    )
                    .map_err(|_| ProductionCompletionDispatchErrorV1::DispatchProjection)?;
                if prepared.dispatch_key() != registration.dispatch_key() {
                    return Err(ProductionCompletionDispatchErrorV1::ReservedOwnerMismatch);
                }
                let installed = registration.commit(prepared, wait_source);
                if installed != dispatch_key {
                    return Err(ProductionCompletionDispatchErrorV1::ReservedOwnerMismatch);
                }
                self.coordinator = next;
                output.commit();
                Ok(ProductionCompletionDispatchV1::FetchDispatched { ordinal })
            }
            LifecycleWorkClass::Store => {
                if !ordinary_body.contains(&ordinal) {
                    return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
                }
                if let Some(census) = census {
                    census.complete_without_selection();
                }
                self.publish_durable_store_validate(services, executor, lease)
            }
            LifecycleWorkClass::Broadcast
            | LifecycleWorkClass::EnterView
            | LifecycleWorkClass::EquivocationReport
            | LifecycleWorkClass::InvalidBodyReport
            | LifecycleWorkClass::CertifiedServe
            | LifecycleWorkClass::ProducerTurn => {
                Err(ProductionCompletionDispatchErrorV1::UnexpectedPlan)
            }
        }
    }
    /// Reserve, claim, and dispatch the sole Ready lifecycle-owned recovered Sign.
    ///
    /// The current Completion cursor is borrow-bound. Worker capacity is
    /// retained before coordinator planning, whose sealed row also reserves
    /// the mandatory Consensus Broadcast successor. Every error after a
    /// volatile claim restores that row to Ready while dropping the still-armed
    /// queue reservation. The infallible final tail alone arms the sealed
    /// carrier and publishes its class-sensitive dedicated command.
    #[cfg(not(test))]
    pub(in crate::sumeragi) fn dispatch_recovered_lifecycle_sign(
        &mut self,
        services: &ProductionV2Services,
        executor: &V2EffectExecutor<SerializedV2Runtime>,
        runner: LifecycleCurrentRunnerTurn<'_>,
    ) -> Result<
        ProductionRecoveredLifecycleSignDispatchV1,
        ProductionRecoveredLifecycleSignDispatchErrorV1,
    > {
        let context = self.verified.context();
        if runner.target() != LifecycleRunnerRankTarget::Completion
            || runner.height() != context.height
            || runner.context_id() != context.id()
        {
            return Err(ProductionRecoveredLifecycleSignDispatchErrorV1::ForeignRunnerObservation);
        }
        self.dispatch_recovered_lifecycle_sign_with_runner_debt(services, executor, runner.debt())
    }
    /// Exercise recovered Sign dispatch with a fixture-owned runner snapshot.
    #[cfg(test)]
    pub(in crate::sumeragi) fn dispatch_recovered_lifecycle_sign(
        &mut self,
        services: &ProductionV2Services,
        executor: &V2EffectExecutor<SerializedV2Runtime>,
        runner: LifecycleRunnerRankSnapshot,
    ) -> Result<
        ProductionRecoveredLifecycleSignDispatchV1,
        ProductionRecoveredLifecycleSignDispatchErrorV1,
    > {
        let context = self.verified.context();
        if runner.target() != LifecycleRunnerRankTarget::Completion
            || runner.height() != context.height
            || runner.context_id() != context.id()
        {
            return Err(ProductionRecoveredLifecycleSignDispatchErrorV1::ForeignRunnerObservation);
        }
        self.dispatch_recovered_lifecycle_sign_with_runner_debt(services, executor, runner.debt())
    }
    pub(super) fn dispatch_recovered_lifecycle_sign_with_runner_debt(
        &mut self,
        services: &ProductionV2Services,
        executor: &V2EffectExecutor<SerializedV2Runtime>,
        runner_debt: u64,
    ) -> Result<
        ProductionRecoveredLifecycleSignDispatchV1,
        ProductionRecoveredLifecycleSignDispatchErrorV1,
    > {
        if let Some(fault) = self.coordinator.fault {
            return Err(ProductionRecoveredLifecycleSignDispatchErrorV1::CoordinatorFaulted(fault));
        }
        if let Some(lease) = self.coordinator.active_lease.as_ref() {
            return Err(ProductionRecoveredLifecycleSignDispatchErrorV1::UnsettledLease(lease.id));
        }
        let Some(body_store_identity) = self.body_store_identity.as_ref() else {
            return Err(ProductionRecoveredLifecycleSignDispatchErrorV1::ForeignServiceOwner);
        };
        if self.body_store.is_some()
            || !services.matches_lifecycle_body_store(body_store_identity)
            || !services.matches_lifecycle_executor_output_guard(executor)
        {
            return Err(ProductionRecoveredLifecycleSignDispatchErrorV1::ForeignServiceOwner);
        }
        let mut ready = self.coordinator.ready_index.iter().copied();
        let Some(ordinal) = ready.next() else {
            return Err(ProductionRecoveredLifecycleSignDispatchErrorV1::InvalidReadyCensus);
        };
        if ready.next().is_some()
            || self
                .coordinator
                .records
                .values()
                .filter(|record| matches!(record.state, LifecycleState::Ready))
                .map(|record| record.ordinal)
                .collect::<BTreeSet<_>>()
                != BTreeSet::from([ordinal])
        {
            return Err(ProductionRecoveredLifecycleSignDispatchErrorV1::InvalidReadyCensus);
        }
        let attestation = self
            .registry
            .attest_ready_recovered_lifecycle_sign(&self.coordinator, ordinal)
            .map_err(|_| ProductionRecoveredLifecycleSignDispatchErrorV1::InvalidCarrier)?;
        if attestation.demand()
            != super::work_registry::ReadyRecoveredLifecycleSignDemandV1::BoundedIo
        {
            return Err(ProductionRecoveredLifecycleSignDispatchErrorV1::InvalidCarrier);
        }
        let dispatch_key = attestation.dispatch_key();
        let mode = executor.lifecycle_mode_rank_snapshot();
        let context = self.verified.context();
        if mode.height() != context.height
            || mode.context_id() != context.id()
            || !dispatch_key.matches_height_context(context)
        {
            return Err(ProductionRecoveredLifecycleSignDispatchErrorV1::ForeignServiceOwner);
        }
        let capacity = services
            .capture_recovered_lifecycle_sign_capacity(dispatch_key)
            .map_err(ProductionRecoveredLifecycleSignDispatchErrorV1::Capacity)?;
        let RecoveredLifecycleSignCapacityCaptureV1::Reserved(reservation) = capacity else {
            return Ok(ProductionRecoveredLifecycleSignDispatchV1::CapacityUnavailable);
        };
        let factory = AuthenticatedSchedulerInputsFactory::new();
        let Some(record) = self.coordinator.records.get(&ordinal) else {
            reservation.cancel_uncommitted();
            return Err(ProductionRecoveredLifecycleSignDispatchErrorV1::InvalidReadyCensus);
        };
        let live_debts = [
            mode.debt(),
            reservation.authenticated_predecessor_debt(&factory),
            0,
            0,
            0,
            runner_debt,
        ];
        let Some(row) = authenticated_ready_row(
            &factory,
            record,
            None,
            None,
            Some(attestation),
            None,
            live_debts,
        ) else {
            reservation.cancel_uncommitted();
            return Err(ProductionRecoveredLifecycleSignDispatchErrorV1::InvalidCarrier);
        };
        let inputs = authenticated_scheduler_inputs(
            factory,
            BTreeMap::new(),
            BTreeMap::from([(ordinal, row)]),
        );
        let lease = match self.coordinator.plan_turn(inputs) {
            super::TurnPlan::Execute(lease)
                if lease.ordinal() == ordinal
                    && matches!(
                        lease.work_class(),
                        LifecycleWorkClass::SignVote
                            | LifecycleWorkClass::SignProposal
                            | LifecycleWorkClass::SignTimeout
                    )
                    && lease.output_reservation().is_some_and(|reservation| {
                        reservation.class() == CapacityClass::Consensus
                    }) =>
            {
                lease
            }
            super::TurnPlan::Execute(lease) => {
                let rolled_back = if let Some(output_reservation) = lease.output_reservation() {
                    self.coordinator
                        .rollback_unpublished_reserved_turn(&lease, output_reservation.class())
                } else {
                    self.coordinator.rollback_unpublished_turn(&lease)
                };
                assert!(
                    rolled_back,
                    "unexpected recovered Sign plan must restore its unpublished claim"
                );
                reservation.cancel_uncommitted();
                return Err(ProductionRecoveredLifecycleSignDispatchErrorV1::UnexpectedPlan);
            }
            super::TurnPlan::Waiting(_)
            | super::TurnPlan::Idle
            | super::TurnPlan::FailClosed(_) => {
                reservation.cancel_uncommitted();
                return Err(ProductionRecoveredLifecycleSignDispatchErrorV1::UnexpectedPlan);
            }
        };
        let prepared = match self
            .registry
            .prepare_recovered_lifecycle_sign_dispatch(&self.coordinator, &lease)
        {
            Ok(prepared) => prepared,
            Err(_) => {
                assert!(
                    self.coordinator
                        .rollback_unpublished_reserved_turn(&lease, CapacityClass::Consensus),
                    "failed recovered Sign projection must restore its unpublished claim"
                );
                reservation.cancel_uncommitted();
                return Err(ProductionRecoveredLifecycleSignDispatchErrorV1::DispatchProjection);
            }
        };
        if !reservation.preflight(&prepared) {
            drop(prepared);
            assert!(
                self.coordinator
                    .rollback_unpublished_reserved_turn(&lease, CapacityClass::Consensus),
                "mismatched recovered Sign reservation must restore its unpublished claim"
            );
            reservation.cancel_uncommitted();
            return Err(ProductionRecoveredLifecycleSignDispatchErrorV1::ReservedCommandMismatch);
        }
        reservation.commit(prepared);
        Ok(ProductionRecoveredLifecycleSignDispatchV1::Queued { ordinal })
    }
    pub(super) fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(
        &mut self,
        services: &ProductionV2Services,
        runner_debt: u64,
    ) -> Result<
        ProductionRecoveredLifecycleSignedBroadcastRefanoutV1,
        ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1,
    > {
        if let Some(fault) = self.coordinator.fault {
            return Err(
                ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::CoordinatorFaulted(
                    fault,
                ),
            );
        }
        if let Some(lease) = self.coordinator.active_lease.as_ref() {
            return Err(
                ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::UnsettledLease(
                    lease.id,
                ),
            );
        }
        let Some(body_store_identity) = self.body_store_identity.as_ref() else {
            return Err(
                ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::ForeignServiceOwner,
            );
        };
        if self.body_store.is_some() || !services.matches_lifecycle_body_store(body_store_identity)
        {
            return Err(
                ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::ForeignServiceOwner,
            );
        }
        let exact_ready = self
            .coordinator
            .records
            .iter()
            .filter_map(|(ordinal, record)| {
                matches!(record.state, LifecycleState::Ready).then_some(*ordinal)
            })
            .collect::<BTreeSet<_>>();
        if exact_ready != self.coordinator.ready_index {
            return Err(
                ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidReadyCensus,
            );
        }
        let mut broadcast_carriers = BTreeMap::new();
        for ready_ordinal in &exact_ready {
            let Some(record) = self.coordinator.records.get(ready_ordinal) else {
                return Err(
                    ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidReadyCensus,
                );
            };
            if record.work_class != LifecycleWorkClass::Broadcast {
                continue;
            }
            let carrier = self
                .registry
                .registry()
                .attest_ready_lifecycle_broadcast_carrier(&self.coordinator, *ready_ordinal)
                .map_err(|_| {
                    ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidCarrier
                })?;
            if broadcast_carriers.insert(*ready_ordinal, carrier).is_some() {
                return Err(
                    ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidReadyCensus,
                );
            }
        }
        let Some(ordinal) = broadcast_carriers.iter().find_map(|(ordinal, carrier)| {
            matches!(carrier, ReadyLifecycleBroadcastCarrierV1::RecoveredRefanout)
                .then_some(*ordinal)
        }) else {
            if exact_ready.is_empty() {
                return Ok(ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::None);
            }
            return Ok(ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::OtherReadyWork);
        };
        let paired_next_sign_ordinal = self
            .registry
            .recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal(
                &self.coordinator,
                ordinal,
            );
        if paired_next_sign_ordinal.is_none()
            && self
                .registry
                .recovered_lifecycle_signed_broadcast_declares_next_vote(&self.coordinator, ordinal)
        {
            return Err(ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidCarrier);
        }
        let mut paired_next_sign = if let Some(next_sign_ordinal) = paired_next_sign_ordinal {
            let attestation = self
                .registry
                .attest_ready_recovered_lifecycle_signed_broadcast_and_next_vote(
                    &self.coordinator,
                    ordinal,
                    next_sign_ordinal,
                )
                .map_err(|_| {
                    ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidCarrier
                })?;
            Some((next_sign_ordinal, attestation))
        } else {
            self.registry
                .attest_ready_recovered_lifecycle_signed_broadcast(&self.coordinator, ordinal)
                .map_err(|_| {
                    ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidCarrier
                })?;
            None
        };
        let factory = AuthenticatedSchedulerInputsFactory::new();
        let mut ready_rows = BTreeMap::new();
        for ready_ordinal in &exact_ready {
            let ready_record = self.coordinator.records.get(ready_ordinal).ok_or(
                ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidReadyCensus,
            )?;
            let live_debts = if *ready_ordinal == ordinal {
                [0, 0, 0, 0, 0, runner_debt]
            } else {
                [1, 0, 0, 0, 0, runner_debt]
            };
            let row = match ready_record.work_class {
                LifecycleWorkClass::Broadcast => match broadcast_carriers
                    .get(ready_ordinal)
                    .copied()
                    .ok_or(
                        ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidReadyCensus,
                    )?
                {
                    ReadyLifecycleBroadcastCarrierV1::RetainedDirectOutput(attestation) => {
                        authenticated_retained_direct_broadcast_ready_row(
                            &factory,
                            ready_record,
                            attestation,
                            live_debts,
                        )
                    }
                    ReadyLifecycleBroadcastCarrierV1::RecoveredRefanout => {
                        self.registry
                            .attest_ready_recovered_lifecycle_signed_broadcast(
                                &self.coordinator,
                                *ready_ordinal,
                            )
                            .map_err(|_| {
                                ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidCarrier
                            })?;
                        authenticated_ready_row(
                            &factory,
                            ready_record,
                            None,
                            None,
                            None,
                            None,
                            live_debts,
                        )
                    }
                },
                LifecycleWorkClass::SignVote
                | LifecycleWorkClass::SignProposal
                | LifecycleWorkClass::SignTimeout => {
                    let attestation = if paired_next_sign
                        .as_ref()
                        .is_some_and(|(next, _)| next == ready_ordinal)
                    {
                        paired_next_sign
                            .take()
                            .expect("matched pair attestation is retained")
                            .1
                    } else {
                        self.registry
                            .attest_ready_recovered_lifecycle_sign(
                                &self.coordinator,
                                *ready_ordinal,
                            )
                            .map_err(|_| {
                                ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidCarrier
                            })?
                    };
                    authenticated_ready_row(
                        &factory,
                        ready_record,
                        None,
                        None,
                        Some(attestation),
                        None,
                        live_debts,
                    )
                }
                LifecycleWorkClass::Apply => {
                    let attestation = self
                        .registry
                        .attest_ready_recovered_decision_apply(
                            &self.coordinator,
                            *ready_ordinal,
                        )
                        .map_err(|_| {
                            ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidCarrier
                        })?;
                    authenticated_ready_row(
                        &factory,
                        ready_record,
                        None,
                        Some(attestation),
                        None,
                        None,
                        live_debts,
                    )
                }
                LifecycleWorkClass::Fetch => {
                    if let Ok(attestation) = self
                        .registry
                        .registry()
                        .attest_schedulable_certified_body_pipeline(
                            &self.coordinator,
                            *ready_ordinal,
                            None,
                        )
                    {
                        authenticated_certified_body_pipeline_ready_row(
                            &factory,
                            ready_record,
                            attestation,
                            live_debts,
                        )
                    } else {
                        let attestation = self
                            .registry
                            .attest_ready_recovered_decision_fetch(
                                &self.coordinator,
                                *ready_ordinal,
                            )
                            .map_err(|_| {
                                ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidCarrier
                            })?;
                        authenticated_ready_row(
                            &factory,
                            ready_record,
                            None,
                            None,
                            None,
                            Some(attestation),
                            live_debts,
                        )
                    }
                }
                LifecycleWorkClass::Store => {
                    let attestation = self
                        .registry
                        .registry()
                        .attest_schedulable_certified_body_pipeline(
                            &self.coordinator,
                            *ready_ordinal,
                            None,
                        )
                        .map_err(|_| {
                            ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidCarrier
                        })?;
                    authenticated_certified_body_pipeline_ready_row(
                        &factory,
                        ready_record,
                        attestation,
                        live_debts,
                    )
                }
                LifecycleWorkClass::Validate => {
                    let attestation = self
                        .coordinator
                        .attest_ready_validate_demand(&self.registry, *ready_ordinal)
                        .map_err(|_| {
                            ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidCarrier
                        })?;
                    authenticated_ready_row(
                        &factory,
                        ready_record,
                        Some(attestation),
                        None,
                        None,
                        None,
                        live_debts,
                    )
                }
                LifecycleWorkClass::EnterView
                | LifecycleWorkClass::EquivocationReport
                | LifecycleWorkClass::InvalidBodyReport
                | LifecycleWorkClass::CertifiedServe
                | LifecycleWorkClass::ProducerTurn => {
                    return Ok(
                        ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::OtherReadyWork,
                    );
                }
            }
            .ok_or(
                ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidCarrier,
            )?;
            if ready_rows.insert(*ready_ordinal, row).is_some() {
                return Err(
                    ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidReadyCensus,
                );
            }
        }
        if paired_next_sign.is_some() {
            return Err(ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidCarrier);
        }
        let inputs = authenticated_scheduler_inputs(factory, BTreeMap::new(), ready_rows);
        let lease = match self.coordinator.plan_turn(inputs) {
            super::TurnPlan::Execute(lease)
                if lease.ordinal() == ordinal
                    && lease.work_class() == LifecycleWorkClass::Broadcast
                    && lease.output_reservation().is_none() =>
            {
                lease
            }
            super::TurnPlan::Execute(lease) => {
                assert!(
                    self.coordinator.rollback_unpublished_turn(&lease),
                    "unexpected recovered Broadcast plan must restore its durable owner"
                );
                return Err(
                    ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::UnexpectedPlan,
                );
            }
            super::TurnPlan::Waiting(_)
            | super::TurnPlan::Idle
            | super::TurnPlan::FailClosed(_) => {
                return Err(
                    ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::UnexpectedPlan,
                );
            }
        };
        let Some(authority) = self
            .registry
            .project_claimed_recovered_lifecycle_signed_broadcast_output(&self.coordinator, &lease)
        else {
            assert!(
                self.coordinator.rollback_unpublished_turn(&lease),
                "failed recovered Broadcast projection must restore its durable owner"
            );
            return Err(ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidCarrier);
        };
        let capture = match services
            .capture_recovered_lifecycle_signed_broadcast_refanout(authority)
        {
            Ok(capture) => capture,
            Err(_) => {
                services
                    .lifecycle_output_guard()
                    .close_admission_for_restart();
                assert!(
                    self.coordinator.rollback_unpublished_turn(&lease),
                    "failed recovered Broadcast capture must restore its durable owner"
                );
                return Ok(ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::RestartRequired);
            }
        };
        let RecoveredLifecycleSignBroadcastOutputCaptureV1::Reserved(output) = capture else {
            assert!(
                self.coordinator.rollback_unpublished_turn(&lease),
                "unavailable recovered Broadcast output must restore its durable owner"
            );
            return Ok(ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::CapacityUnavailable);
        };
        let Some((_, &wait_digest)) = lease.physical_slots().first_key_value() else {
            drop(output);
            return Ok(ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::RestartRequired);
        };
        let wait_source = super::WaitSource::Recovery(wait_digest);
        let wait_generation = self
            .coordinator
            .observed_generation
            .get(&wait_source)
            .copied()
            .unwrap_or(0);
        let wait = super::WaitToken::new(wait_source, wait_generation);
        self.coordinator
            .settle_turn(lease, super::TurnOutcome::Blocked(wait));
        if self.coordinator.fault.is_some()
            || self.coordinator.active_lease.is_some()
            || self
                .coordinator
                .records
                .get(&ordinal)
                .is_none_or(|record| record.state != LifecycleState::Waiting(wait))
        {
            drop(output);
            return Ok(ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::RestartRequired);
        }
        output.commit_after_publication();
        // TODO: Consume the still-live Broadcast only in the authenticated
        // applied-height output handoff/owner rollover transaction. Process-
        // local actor admission is not a durable terminal receipt.
        Ok(ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::Refanned { ordinal })
    }
    /// Wake and reclaim one externally parked recovered Fetch, then publish
    /// its exact response persistence while retaining the active lease for
    /// the Phase-B Fetch-to-Store transaction.
    #[allow(clippy::result_large_err)]
    pub(super) fn persist_recovered_decision_fetch_response_after_runner(
        &mut self,
        services: &ProductionV2Services,
        executor: &mut V2EffectExecutor<SerializedV2Runtime>,
        selector: PreparedLifecycleIngressSelector,
        runner: &LifecycleCurrentRunnerTurn<'_>,
    ) -> Result<
        ProductionRecoveredDecisionFetchPersistenceV1,
        ProductionRecoveredDecisionFetchPersistenceErrorV1,
    > {
        let context = selector.context();
        if self.coordinator.fault.is_some() || self.coordinator.active_lease.is_some() {
            return Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::InvalidClaimedCarrier);
        }
        if runner.target() != LifecycleRunnerRankTarget::Ingress
            || runner.height() != context.height()
            || runner.context_id().0.as_ref() != context.id().as_bytes()
        {
            return Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::ForeignOwner);
        }
        let Some(body_store_identity) = self.body_store_identity.as_ref() else {
            return Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::ForeignOwner);
        };
        if self.body_store.is_some()
            || !services.matches_lifecycle_body_store(body_store_identity)
            || !services.matches_lifecycle_executor_output_guard(executor)
        {
            return Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::ForeignOwner);
        }
        let mode = executor.lifecycle_mode_rank_snapshot();
        if mode.height() != context.height()
            || mode.context_id().0.as_ref() != context.id().as_bytes()
        {
            return Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::ForeignOwner);
        }
        let fetch = selector
            .attest_scheduler_recovered_fetch_carrier(&self.coordinator, &mut self.registry)
            .map_err(|_| {
                ProductionRecoveredDecisionFetchPersistenceErrorV1::InvalidClaimedCarrier
            })?;
        if !self.coordinator.ready_index.is_empty()
            || self
                .coordinator
                .records
                .values()
                .any(|record| matches!(record.state, LifecycleState::Ready))
        {
            return Err(
                ProductionRecoveredDecisionFetchPersistenceErrorV1::CompetingReadyWork(selector),
            );
        }
        let capacity = services
            .capture_lifecycle_capacity_rank(selector)
            .map_err(|error| {
                let failure = error.failure();
                let prepared = error.into_prepared();
                ProductionRecoveredDecisionFetchPersistenceErrorV1::Service { failure, prepared }
            })?;
        let factory = AuthenticatedSchedulerInputsFactory::new();
        let (reservation, prepared) = match capacity.into_authenticated(&factory) {
            AuthenticatedLifecycleIoCapacity::Unavailable { wait, prepared } => {
                drop(factory);
                return Ok(ProductionRecoveredDecisionFetchPersistenceV1::CapacityWait(
                    PreparedProductionIngressCapacityWait {
                        mode,
                        wait,
                        selector: prepared,
                    },
                ));
            }
            AuthenticatedLifecycleIoCapacity::Reserved {
                reservation,
                prepared,
            } => (reservation, prepared),
        };
        if !reservation.preflight_recovered_decision_fetch_target_absent() {
            let prepared = reservation.abort_into_prepared(prepared);
            return Err(
                ProductionRecoveredDecisionFetchPersistenceErrorV1::InFlightSelectedWork(prepared),
            );
        }
        let positions = prepared.selected_positions().components();
        let live_debts = [
            mode.debt(),
            reservation.authenticated_predecessor_debt(&factory),
            prepared.selector_debt(),
            positions[0],
            positions[1],
            runner.debt(),
        ];
        let task = match executor.prepare_recovered_decision_fetch_body_persistence(prepared) {
            Ok(task) => task,
            Err(error) => {
                let (failure, prepared) = error.into_parts();
                let prepared = reservation.abort_into_prepared(prepared);
                return Err(
                    ProductionRecoveredDecisionFetchPersistenceErrorV1::CommandPreparation {
                        failure,
                        prepared,
                    },
                );
            }
        };
        let ordinal = fetch.ordinal();
        let record = self
            .coordinator
            .records
            .get(&ordinal)
            .ok_or(ProductionRecoveredDecisionFetchPersistenceErrorV1::InvalidClaimedCarrier)?;
        let row = authenticated_waiting_fetch_ready_row(&factory, record, fetch, live_debts)
            .ok_or(ProductionRecoveredDecisionFetchPersistenceErrorV1::InvalidClaimedCarrier)?;
        let (source, generation) = fetch.wake_generation();
        let inputs = authenticated_scheduler_inputs(
            factory,
            BTreeMap::from([(source, generation)]),
            BTreeMap::from([(ordinal, row)]),
        );
        if !task
            .dispatch_key()
            .matches_height_context(self.verified.context())
            || task.dispatch_key().lifecycle_ordinal() != ordinal
            || super::projection::certified_fetch_wait_source(task.request_hash()) != source
            || !self
                .registry
                .registry_mut()
                .matches_waiting_dispatched_recovered_decision_fetch(
                    &self.coordinator,
                    task.dispatch_key(),
                    source,
                )
        {
            return Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::InvalidClaimedCarrier);
        }
        if !reservation.preflight_recovered_decision_fetch_body_persistence(&task) {
            return Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::InvalidReservedCommand);
        }
        let claim = executor
            .prepare_recovered_decision_fetch_response_claim(&task)
            .map_err(ProductionRecoveredDecisionFetchPersistenceErrorV1::Claim)?;
        let mut next = self.coordinator.stage_durable_transaction();
        let lease = match next.plan_turn(inputs) {
            super::TurnPlan::Execute(lease)
                if lease.ordinal() == ordinal
                    && lease.work_class() == LifecycleWorkClass::Fetch
                    && lease.output_reservation().is_none() =>
            {
                lease
            }
            super::TurnPlan::Execute(_)
            | super::TurnPlan::Idle
            | super::TurnPlan::Waiting(_)
            | super::TurnPlan::FailClosed(_) => {
                return Err(
                    ProductionRecoveredDecisionFetchPersistenceErrorV1::InvalidClaimedCarrier,
                );
            }
        };
        if !self
            .registry
            .registry_mut()
            .matches_claimed_dispatched_recovered_decision_fetch(
                &next,
                &lease,
                task.dispatch_key(),
                source,
            )
        {
            return Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::InvalidClaimedCarrier);
        }
        self.coordinator = next;
        claim.commit_with_queue(reservation, task);
        assert_eq!(self.coordinator.active_lease.as_ref(), Some(&lease));
        Ok(ProductionRecoveredDecisionFetchPersistenceV1::Queued { ordinal })
    }
    /// Plan, submit, and reblock one exact selected certified-Fetch response.
    ///
    /// The selected response is reauthenticated against the exact Waiting
    /// Fetch row and concrete registry incumbent. The service then consumes
    /// the whole selector into either a locked I/O reservation or its opaque
    /// release-generation wait. With capacity held, the owner advances the
    /// request generation, claims the sole prospective Fetch, consumes the
    /// selector into its persistence command, and settles that lease back to
    /// the same source at the advanced generation before publishing the FIFO.
    /// Phase B advances that same source once more after durable persistence.
    /// The runner supplies its borrow-bound current Ingress turn, so another
    /// cursor observation cannot be retained or minted until this complete
    /// transaction returns. Independently opened worker stores are rejected.
    #[allow(clippy::result_large_err)]
    pub(crate) fn plan_ingress_turn(
        &mut self,
        services: &ProductionV2Services,
        executor: &V2EffectExecutor<SerializedV2Runtime>,
        mode: LifecycleModeRankSnapshot,
        selector: PreparedLifecycleIngressSelector,
        runner: LifecycleCurrentRunnerTurn<'_>,
    ) -> Result<ProductionIngressTurnPreparation, ProductionIngressSchedulerInputsError> {
        let context = selector.context();
        if runner.target() != LifecycleRunnerRankTarget::Ingress
            || runner.height() != context.height()
            || runner.context_id().0.as_ref() != context.id().as_bytes()
        {
            return Err(ProductionIngressSchedulerInputsError::ForeignRunnerObservation);
        }
        self.plan_ingress_turn_with_runner_debt(services, executor, mode, selector, runner.debt())
    }
    /// Exercise the production transaction with a fixture-owned rank snapshot.
    ///
    /// Normal builds have no snapshot-taking entry point; they can call only
    /// the borrow-bound current-turn method above.
    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    pub(crate) fn plan_ingress_turn_for_test(
        &mut self,
        services: &ProductionV2Services,
        executor: &V2EffectExecutor<SerializedV2Runtime>,
        mode: LifecycleModeRankSnapshot,
        selector: PreparedLifecycleIngressSelector,
        runner: LifecycleRunnerRankSnapshot,
    ) -> Result<ProductionIngressTurnPreparation, ProductionIngressSchedulerInputsError> {
        let context = selector.context();
        if runner.target() != LifecycleRunnerRankTarget::Ingress
            || runner.height() != context.height()
            || runner.context_id().0.as_ref() != context.id().as_bytes()
        {
            return Err(ProductionIngressSchedulerInputsError::ForeignRunnerObservation);
        }
        self.plan_ingress_turn_with_runner_debt(services, executor, mode, selector, runner.debt())
    }
    #[allow(clippy::result_large_err, clippy::too_many_lines)]
    fn plan_ingress_turn_with_runner_debt(
        &mut self,
        services: &ProductionV2Services,
        executor: &V2EffectExecutor<SerializedV2Runtime>,
        mode: LifecycleModeRankSnapshot,
        selector: PreparedLifecycleIngressSelector,
        runner_debt: u64,
    ) -> Result<ProductionIngressTurnPreparation, ProductionIngressSchedulerInputsError> {
        if let Some(fault) = self.coordinator.fault {
            return Err(ProductionIngressSchedulerInputsError::CoordinatorFaulted {
                _fault: fault,
            });
        }
        if let Some(lease) = self.coordinator.active_lease.as_ref() {
            return Err(ProductionIngressSchedulerInputsError::UnsettledLease { _lease: lease.id });
        }
        let context = selector.context();
        if context != self.coordinator.active_context
            || mode.height() != context.height()
            || mode.context_id().0.as_ref() != context.id().as_bytes()
        {
            return Err(ProductionIngressSchedulerInputsError::ForeignModeObservation);
        }
        if mode != executor.lifecycle_mode_rank_snapshot() {
            return Err(ProductionIngressSchedulerInputsError::StaleModeObservation);
        }
        let Some(body_store_identity) = self.body_store_identity.as_ref() else {
            return Err(ProductionIngressSchedulerInputsError::BodyStoreNotBound);
        };
        if self.body_store.is_some() || !services.matches_lifecycle_body_store(body_store_identity)
        {
            return Err(ProductionIngressSchedulerInputsError::BodyStoreNotBound);
        }
        if !services.matches_lifecycle_executor_output_guard(executor) {
            return Err(ProductionIngressSchedulerInputsError::ForeignOutputGuard);
        }
        let fetch = selector
            .attest_scheduler_fetch_carrier(&self.coordinator, &mut self.registry)
            .map_err(|_| ProductionIngressSchedulerInputsError::InvalidSelectedCarrier)?;
        if !self.coordinator.ready_index.is_empty()
            || self
                .coordinator
                .records
                .values()
                .any(|record| matches!(record.state, LifecycleState::Ready))
        {
            return Err(ProductionIngressSchedulerInputsError::CompetingReadyWork);
        }
        let capacity = services
            .capture_lifecycle_capacity_rank(selector)
            .map_err(|error| {
                let failure = error.failure();
                let prepared = error.into_prepared();
                ProductionIngressSchedulerInputsError::Service {
                    _failure: failure,
                    _prepared: prepared,
                }
            })?;
        let factory = AuthenticatedSchedulerInputsFactory::new();
        let (reservation, prepared) = match capacity.into_authenticated(&factory) {
            AuthenticatedLifecycleIoCapacity::Unavailable { wait, prepared } => {
                drop(factory);
                return Ok(ProductionIngressTurnPreparation::CapacityWait(
                    PreparedProductionIngressCapacityWait {
                        mode,
                        wait,
                        selector: prepared,
                    },
                ));
            }
            AuthenticatedLifecycleIoCapacity::Reserved {
                reservation,
                prepared,
            } => (reservation, prepared),
        };
        if !reservation.preflight_selected_target_work_absent() {
            let prepared = reservation.abort_into_prepared(prepared);
            return Err(
                ProductionIngressSchedulerInputsError::InFlightSelectedWork {
                    _prepared: prepared,
                },
            );
        }
        let positions = prepared.selected_positions().components();
        let live_debts = [
            mode.debt(),
            reservation.authenticated_predecessor_debt(&factory),
            prepared.selector_debt(),
            positions[0],
            positions[1],
            runner_debt,
        ];
        let ordinal = fetch.ordinal();
        let record = self
            .coordinator
            .records
            .get(&ordinal)
            .ok_or(ProductionIngressSchedulerInputsError::InvalidSelectedCarrier)?;
        let row = authenticated_waiting_fetch_ready_row(&factory, record, fetch, live_debts)
            .ok_or(ProductionIngressSchedulerInputsError::InvalidSelectedCarrier)?;
        let (source, generation) = fetch.wake_generation();
        let inputs = authenticated_scheduler_inputs(
            factory,
            BTreeMap::from([(source, generation)]),
            BTreeMap::from([(ordinal, row)]),
        );
        let task = match executor.prepare_certified_fetch_body_persistence(prepared) {
            Ok(task) => task,
            Err(error) => {
                let prepared = reservation.abort_into_prepared(error.into_prepared());
                return Err(ProductionIngressSchedulerInputsError::CommandPreparation {
                    _prepared: prepared,
                });
            }
        };
        if !reservation.preflight_certified_fetch_body_persistence(&task) {
            self.coordinator.fault = Some(super::CoordinatorFault::InvalidSchedulerInputs);
            return Err(ProductionIngressSchedulerInputsError::InvalidReservedCommand);
        }
        let plan = self.coordinator.plan_turn(inputs);
        let lease = match plan {
            super::TurnPlan::Execute(lease) => lease,
            super::TurnPlan::Idle | super::TurnPlan::Waiting(_) => {
                self.coordinator.fault = Some(super::CoordinatorFault::InvalidSchedulerInputs);
                return Err(ProductionIngressSchedulerInputsError::UnexpectedPlan);
            }
            super::TurnPlan::FailClosed(fault) => {
                if self.coordinator.fault.is_none() {
                    self.coordinator.fault = Some(fault);
                }
                return Err(ProductionIngressSchedulerInputsError::UnexpectedPlan);
            }
        };
        if lease.ordinal() != ordinal || lease.work_class() != LifecycleWorkClass::Fetch {
            self.coordinator.fault = Some(super::CoordinatorFault::InvalidSchedulerInputs);
            return Err(ProductionIngressSchedulerInputsError::UnexpectedPlan);
        }
        let post_submit_wait = fetch.post_submit_wait();
        self.coordinator
            .settle_turn(lease, super::TurnOutcome::Blocked(post_submit_wait));
        if let Some(fault) = self.coordinator.fault {
            return Err(ProductionIngressSchedulerInputsError::SettlementFault { _fault: fault });
        }
        if self.coordinator.active_lease.is_some()
            || self
                .coordinator
                .records
                .get(&ordinal)
                .is_none_or(|record| record.state != LifecycleState::Waiting(post_submit_wait))
        {
            self.coordinator.fault = Some(super::CoordinatorFault::InvalidSchedulerInputs);
            return Err(ProductionIngressSchedulerInputsError::SettlementFault {
                _fault: super::CoordinatorFault::InvalidSchedulerInputs,
            });
        }
        reservation.commit_certified_fetch_body_persistence(task);
        Ok(ProductionIngressTurnPreparation::Queued(
            QueuedProductionIngressFetch { ordinal },
        ))
    }
}
#[cfg(test)]
#[allow(dead_code)] // Whole-state equality observes these fields in the fail-closed regression.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RecoveredBroadcastSchedulerStateForTest {
    records: BTreeMap<u128, super::LifecycleRecord>,
    key_index: BTreeMap<super::LifecycleKey, u128>,
    owner_index: BTreeMap<super::CausalRoot, super::OwnerId>,
    ready_index: BTreeSet<u128>,
    active_lease: Option<super::TurnLease>,
    high_water: u128,
    next_lease: Option<u128>,
    durable_records: BTreeMap<u128, super::schema::DurableRecordMetadata>,
    capacity_used: BTreeMap<CapacityClass, usize>,
    capacity_generation: BTreeMap<CapacityClass, u64>,
    observed_generation: BTreeMap<super::WaitSource, u64>,
    producer_debts: BTreeMap<u128, u128>,
    fault: Option<super::CoordinatorFault>,
    declares_pair: bool,
    paired_ordinal: Option<u128>,
}

#[cfg(test)]
mod recovered_sign_capacity_tests {
    use super::super::schema::SchedulerEpisode;
    use super::{
        AuthenticatedSchedulerInputsFactory, ProductionCompletionDispatchV1,
        ProductionCompletionReadyWorkV1,
        ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1,
        ProductionRecoveredLifecycleSignedBroadcastRefanoutV1, ProductionV2Services,
        authenticated_ready_row,
    };
    use crate::sumeragi::v2_lifecycle_coordinator::{
        CapacityClass, CausalRoot, LifecycleDigest, LifecycleKey, LifecyclePhase, LifecycleRecord,
        LifecycleRound, LifecycleStage, LifecycleStageKind, LifecycleState, LifecycleWorkClass,
        OwnerId, PhysicalSlotId, PredecessorScope, ProductionLifecycleOutputAdmissionSettlementV1,
        ProductionLifecycleOwnerV1, SchedulerEpisodeUniverse,
        work_registry::ReadyRecoveredLifecycleSignAttestationV1,
    };
    use iroha_crypto::{Hash, KeyPair};
    use iroha_data_model::{block::consensus_v2 as wire, peer::PeerId};
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::Arc,
        time::{Duration, Instant},
    };

    macro_rules! sumeragi_stack_test {
        ($name:ident, $body:block) => {
            #[test]
            fn $name() {
                let handle = crate::sumeragi::sumeragi_thread_builder(concat!(
                    "sumeragi-v2-scheduler-test-",
                    stringify!($name)
                ))
                .spawn(move || $body)
                .expect("spawn scheduler regression on the production consensus stack");
                if let Err(payload) = handle.join() {
                    std::panic::resume_unwind(payload);
                }
            }
        };
    }

    fn digest(byte: u8) -> LifecycleDigest {
        LifecycleDigest::new([byte; 32])
    }

    fn worker_context(keys: &[KeyPair]) -> wire::HeightContext {
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            network_id: crate::sumeragi::synthetic_network_id("v2-worker-test"),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster)
                .expect("scheduler fixture equal-vote quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"v2-worker-test-context"),
            execution_policy_hash: Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 8,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 32,
                max_chunk_count: 8,
            },
            leader_seed: [0x33; 32],
        };
        context.validate().expect("valid scheduler fixture context");
        context
    }

    #[allow(clippy::type_complexity)]
    fn recovered_broadcast_scheduler_fixture() -> (
        ProductionLifecycleOwnerV1,
        ProductionV2Services,
        crate::sumeragi::v2_worker::tests::LifecyclePlannerIoFixture,
        tempfile::TempDir,
        u128,
        u128,
        u128,
    ) {
        let (mut services, keys) = crate::sumeragi::v2_worker::tests::fixture();
        let context = worker_context(&keys);
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("scheduler fixture validator proof of possession")
            })
            .collect::<Vec<_>>();
        let verified = crate::sumeragi::v2::VerifiedHeightContext::genesis(context, proofs)
            .expect("verified scheduler fixture context");
        let directory = tempfile::TempDir::new().expect("temporary scheduler fixture storage");
        let (mut owner, broadcast_ordinal, paired_ordinal, unrelated_ordinal) =
            ProductionLifecycleOwnerV1::recovered_broadcast_pair_scheduler_fixture_for_test(
                verified,
                &keys[0],
                directory.path(),
            );
        let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
        let planner_io = owner.bind_body_store_to_planner_io_for_test(
            &mut services,
            Arc::clone(&output_guard),
            8,
        );
        (
            owner,
            services,
            planner_io,
            directory,
            broadcast_ordinal,
            paired_ordinal,
            unrelated_ordinal,
        )
    }
    fn recovered_completion_runtime(
        verified: crate::sumeragi::v2::VerifiedHeightContext,
        root: &std::path::Path,
    ) -> crate::sumeragi::v2_runtime::SerializedV2Runtime {
        let (adapter, startup) = crate::sumeragi::v2::SumeragiV2Adapter::open(
            root.join("completion-runtime.wal"),
            verified,
            Some(0),
            crate::sumeragi::v2_core::Generation::new(1),
            [0xC7; 32],
            crate::sumeragi::v2::AdapterFingerprints {
                node: Hash::new(b"recovered completion node"),
                build: Hash::new(b"recovered completion build"),
                config: Hash::new(b"recovered completion config"),
            },
            crate::sumeragi::v2::DeferredAdmissionOrdinalSource::new(0),
        )
        .expect("open recovered Completion runtime");
        assert!(startup.is_empty());
        crate::sumeragi::v2_runtime::SerializedV2Runtime::new(
            adapter,
            startup,
            Instant::now(),
            Duration::from_secs(10),
            crate::sumeragi::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
        )
        .expect("wrap recovered Completion adapter")
        .0
    }
    #[test]
    fn recovered_sign_ready_row_reserves_its_broadcast_capacity_before_claim() {
        let context = digest(0x31);
        let round = LifecycleRound::new(4, 2);
        let key = LifecycleKey::new(
            context,
            round,
            Some(round),
            Some(digest(0x32)),
            LifecyclePhase::Proposal,
            Some(digest(0x33)),
        );
        let ordinal = 9;
        let owner = OwnerId::new(CausalRoot::new(digest(0x34)), ordinal);
        let slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let record = LifecycleRecord {
            key,
            owner,
            ordinal,
            work_class: LifecycleWorkClass::SignProposal,
            stage: LifecycleStage::new(
                LifecycleStageKind::SignProposal,
                PredecessorScope::Independent,
            ),
            state: LifecycleState::Ready,
            physical_slots: BTreeMap::from([(slot, digest(0x35))]),
            episode: SchedulerEpisode {
                universe: SchedulerEpisodeUniverse {
                    target: key.scheduler_target(),
                    context,
                    leader: digest(0x36),
                    view: round.view(),
                    subject: key.subject(),
                    phase: key.phase(),
                    authenticated_roster_slots: BTreeSet::new(),
                    capacity_geometry: BTreeMap::new(),
                },
                slot_universe: BTreeSet::from([slot]),
                consumed_slots: BTreeSet::from([slot]),
                frozen_predecessors: BTreeSet::new(),
            },
        };
        let attestation = ReadyRecoveredLifecycleSignAttestationV1::for_test(&record)
            .expect("exact recovered Sign row mints its closed test attestation");
        let factory = AuthenticatedSchedulerInputsFactory::new();
        let row = authenticated_ready_row(
            &factory,
            &record,
            None,
            None,
            Some(attestation),
            None,
            [0; 6],
        )
        .expect("closed recovered Sign attestation authenticates its Ready row");
        assert_eq!(
            row.output_capacity_class(),
            Some(CapacityClass::Consensus),
            "every recovered signature must reserve its mandatory Broadcast slot before claim"
        );
    }

    #[test]
    fn composite_recovered_completion_dispatches_one_ranked_sign_and_preserves_the_other() {
        let (mut services, keys) = crate::sumeragi::v2_worker::tests::fixture();
        let context = worker_context(&keys);
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("composite scheduler validator proof of possession")
            })
            .collect::<Vec<_>>();
        let verified = crate::sumeragi::v2::VerifiedHeightContext::genesis(context, proofs)
            .expect("verified composite scheduler context");
        let directory = tempfile::TempDir::new().expect("temporary composite scheduler storage");
        let runtime = recovered_completion_runtime(verified.clone(), directory.path());
        let (mut owner, broadcast, paired, unrelated) =
            ProductionLifecycleOwnerV1::recovered_broadcast_pair_scheduler_fixture_for_test(
                verified,
                &keys[0],
                directory.path(),
            );
        assert!(owner.retire_ready_work_for_completion_test(broadcast));
        let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
        let (mut executor, planner_io) = owner.bind_body_store_to_recovered_completion_io_for_test(
            &mut services,
            runtime,
            Arc::clone(&output_guard),
            2,
        );

        assert_eq!(
            owner
                .dispatch_completion_with_runner_debt(&services, &mut executor, 0,)
                .expect("the joint physical census dispatches one exact Sign"),
            ProductionCompletionDispatchV1::SignQueued { ordinal: paired }
        );
        let state = owner.recovered_broadcast_scheduler_state_for_test(broadcast);
        assert!(matches!(
            state.records[&paired].state,
            LifecycleState::Claimed(_)
        ));
        assert_eq!(state.records[&unrelated].state, LifecycleState::Ready);
        assert!(state.active_lease.is_some());
        assert!(state.fault.is_none());
        assert!(!output_guard.restart_required());
        planner_io.detach(&mut services);
    }

    sumeragi_stack_test!(
        direct_broadcast_deferred_behind_completion_is_passive_and_sign_is_selected,
        {
            let (mut services, keys) = crate::sumeragi::v2_worker::tests::fixture();
            let context = worker_context(&keys);
            let proofs = keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("direct-output scheduler proof of possession")
                })
                .collect::<Vec<_>>();
            let verified = crate::sumeragi::v2::VerifiedHeightContext::genesis(context, proofs)
                .expect("verified direct-output scheduler context");
            let directory =
                tempfile::TempDir::new().expect("temporary direct-output scheduler store");
            let runtime = recovered_completion_runtime(verified.clone(), directory.path());
            let (mut owner, broadcast, paired, unrelated) =
                ProductionLifecycleOwnerV1::recovered_broadcast_pair_scheduler_fixture_for_test(
                    verified,
                    &keys[0],
                    directory.path(),
                );
            let (direct, _direct_pending) = owner
                .defer_direct_timeout_broadcast_for_test(0x71)
                .expect("defer one exact direct timeout Broadcast behind Ready completion work");
            assert!(owner.retire_ready_work_for_completion_test(broadcast));
            assert_eq!(
                owner.exact_ready_completion_classification_for_test(),
                ProductionCompletionReadyWorkV1::CompletionIo,
                "the later direct output cannot masquerade as recovered refanout"
            );
            let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
            let (mut executor, planner_io) = owner
                .bind_body_store_to_recovered_completion_io_for_test(
                    &mut services,
                    runtime,
                    Arc::clone(&output_guard),
                    2,
                );

            assert_eq!(
                owner
                    .dispatch_completion_with_runner_debt(&services, &mut executor, 0)
                    .expect("the exact completion census retains direct output passively"),
                ProductionCompletionDispatchV1::SignQueued { ordinal: paired }
            );
            let state = owner.recovered_broadcast_scheduler_state_for_test(broadcast);
            assert!(matches!(
                state.records[&paired].state,
                LifecycleState::Claimed(_)
            ));
            assert_eq!(state.records[&unrelated].state, LifecycleState::Ready);
            assert_eq!(state.records[&direct].state, LifecycleState::Ready);
            assert!(state.ready_index.contains(&direct));
            assert!(state.active_lease.is_some());
            assert!(state.fault.is_none());
            assert!(!output_guard.restart_required());
            planner_io.detach(&mut services);
        }
    );

    #[test]
    fn composite_recovered_completion_capacity_unavailable_claims_no_ready_sign() {
        let (mut services, keys) = crate::sumeragi::v2_worker::tests::fixture();
        let context = worker_context(&keys);
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("capacity scheduler validator proof of possession")
            })
            .collect::<Vec<_>>();
        let verified = crate::sumeragi::v2::VerifiedHeightContext::genesis(context, proofs)
            .expect("verified capacity scheduler context");
        let directory = tempfile::TempDir::new().expect("temporary capacity scheduler storage");
        let runtime = recovered_completion_runtime(verified.clone(), directory.path());
        let (mut owner, broadcast, paired, unrelated) =
            ProductionLifecycleOwnerV1::recovered_broadcast_pair_scheduler_fixture_for_test(
                verified,
                &keys[0],
                directory.path(),
            );
        assert!(owner.retire_ready_work_for_completion_test(broadcast));
        let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
        let (mut executor, planner_io) = owner.bind_body_store_to_recovered_completion_io_for_test(
            &mut services,
            runtime,
            Arc::clone(&output_guard),
            1,
        );
        planner_io.saturate_consensus_prefix(&services);
        let before = owner.recovered_broadcast_scheduler_state_for_test(broadcast);

        assert_eq!(
            owner
                .dispatch_completion_with_runner_debt(&services, &mut executor, 0,)
                .expect("a saturated joint census is a typed unavailable turn"),
            ProductionCompletionDispatchV1::CapacityUnavailable
        );
        assert_eq!(
            owner.recovered_broadcast_scheduler_state_for_test(broadcast),
            before,
            "physical unavailability cannot claim or reorder either Ready Sign"
        );
        assert_eq!(before.records[&paired].state, LifecycleState::Ready);
        assert_eq!(before.records[&unrelated].state, LifecycleState::Ready);
        assert!(!output_guard.restart_required());
        planner_io.release_all_predecessors();
        planner_io.detach(&mut services);
    }

    #[test]
    fn recovered_broadcast_refanout_ranks_exact_pair_before_unrelated_ready_sign() {
        let (
            mut owner,
            services,
            _planner_io,
            _directory,
            broadcast_ordinal,
            paired_ordinal,
            unrelated_ordinal,
        ) = recovered_broadcast_scheduler_fixture();
        let before = owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal);
        assert!(before.declares_pair);
        assert_eq!(before.paired_ordinal, Some(paired_ordinal));
        assert_eq!(broadcast_ordinal.checked_add(1), Some(paired_ordinal));
        assert!(before.ready_index.contains(&broadcast_ordinal));
        assert!(before.ready_index.contains(&paired_ordinal));
        assert!(before.ready_index.contains(&unrelated_ordinal));

        assert_eq!(
            owner
                .refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(&services, 0)
                .expect("the complete exact pair census refans out"),
            ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::Refanned {
                ordinal: broadcast_ordinal,
            }
        );

        let after = owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal);
        assert!(matches!(
            after.records[&broadcast_ordinal].state,
            LifecycleState::Waiting(_)
        ));
        assert_eq!(after.records[&paired_ordinal].state, LifecycleState::Ready);
        assert_eq!(
            after.records[&unrelated_ordinal].state,
            LifecycleState::Ready
        );
        assert!(after.active_lease.is_none());
        assert!(after.fault.is_none());

        let (
            mut bounded_owner,
            bounded_services,
            _bounded_planner_io,
            _bounded_directory,
            bounded_broadcast,
            _bounded_pair,
            bounded_unrelated,
        ) = recovered_broadcast_scheduler_fixture();
        assert!(bounded_owner.retire_unrelated_sign_for_finalization_test(bounded_unrelated));
        assert_eq!(
            bounded_owner
                .refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(
                    &bounded_services,
                    0,
                )
                .expect("the bounded finalization pair refans out"),
            ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::Refanned {
                ordinal: bounded_broadcast,
            }
        );
        assert!(
            bounded_owner.finalization_registry_census_is_exact_for_test(),
            "finalization accepts the exact volatile refanout wait beside its Ready next Sign"
        );
    }

    sumeragi_stack_test!(
        recovered_refanout_authenticates_coexisting_direct_broadcast_without_claiming_it,
        {
            let (mut owner, services, _planner_io, _directory, broadcast, _paired, _unrelated) =
                recovered_broadcast_scheduler_fixture();
            let (direct, _direct_pending) = owner
                .defer_direct_timeout_broadcast_for_test(0x72)
                .expect("defer one exact direct timeout Broadcast beside recovered refanout");
            assert_eq!(
                owner.exact_ready_completion_classification_for_test(),
                ProductionCompletionReadyWorkV1::RecoveredLifecycleBroadcast
            );

            assert_eq!(
                owner
                    .refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(&services, 0)
                    .expect("refanout authenticates the complete mixed Broadcast census"),
                ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::Refanned {
                    ordinal: broadcast
                }
            );
            let after = owner.recovered_broadcast_scheduler_state_for_test(broadcast);
            assert!(matches!(
                after.records[&broadcast].state,
                LifecycleState::Waiting(_)
            ));
            assert_eq!(after.records[&direct].state, LifecycleState::Ready);
            assert!(after.ready_index.contains(&direct));
            assert!(after.active_lease.is_none());
            assert!(after.fault.is_none());
        }
    );

    sumeragi_stack_test!(
        broadcast_carrier_classifier_rejects_mismatched_direct_output,
        {
            let (mut owner, services, _planner_io, _directory, broadcast, _paired, _unrelated) =
                recovered_broadcast_scheduler_fixture();
            let (direct, _direct_pending) = owner
                .defer_direct_timeout_broadcast_for_test(0x73)
                .expect("defer one exact direct timeout Broadcast before corruption");
            assert!(owner.corrupt_ready_digest_for_test(direct));
            assert_eq!(
                owner.exact_ready_completion_classification_for_test(),
                ProductionCompletionReadyWorkV1::Invalid,
                "a mismatched direct carrier must fail before route selection"
            );
            let before = owner.recovered_broadcast_scheduler_state_for_test(broadcast);
            assert_eq!(
                owner.refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(&services, 0),
                Err(ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidCarrier)
            );
            assert_eq!(
                owner.recovered_broadcast_scheduler_state_for_test(broadcast),
                before,
                "foreign direct-output coordinates fail before coordinator mutation"
            );
        }
    );

    #[test]
    fn generic_retransmit_of_recovered_broadcast_stutters_without_registry_mutation() {
        let (
            mut owner,
            services,
            _planner_io,
            _directory,
            broadcast_ordinal,
            _paired_ordinal,
            _unrelated_ordinal,
        ) = recovered_broadcast_scheduler_fixture();
        let ready_before = owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal);
        let ready_retransmit = owner
            .recovered_broadcast_runtime_retransmit_for_test(broadcast_ordinal, 11, 0xA1)
            .expect("seal the byte-identical Ready recovered-Broadcast retransmit");
        assert!(matches!(
            owner.settle_lifecycle_output_admission::<()>(
                ready_retransmit,
                |_effect, _ownership| {
                    panic!("the typed recovered Broadcast remains the sole service-I/O owner")
                },
            ),
            ProductionLifecycleOutputAdmissionSettlementV1::AlreadyCompleted
        ));
        assert_eq!(
            owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal),
            ready_before,
            "generic settlement cannot mutate or terminalize the Ready typed carrier"
        );

        assert_eq!(
            owner
                .refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(&services, 0)
                .expect("the typed owner performs its exact refanout"),
            ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::Refanned {
                ordinal: broadcast_ordinal,
            }
        );
        let waiting_before = owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal);
        assert!(matches!(
            waiting_before.records[&broadcast_ordinal].state,
            LifecycleState::Waiting(_)
        ));
        let waiting_retransmit = owner
            .recovered_broadcast_runtime_retransmit_for_test(broadcast_ordinal, 12, 0xA2)
            .expect("seal the byte-identical Waiting recovered-Broadcast retransmit");
        assert!(matches!(
            owner.settle_lifecycle_output_admission::<()>(
                waiting_retransmit,
                |_effect, _ownership| {
                    panic!("generic settlement cannot repeat typed recovered refanout")
                },
            ),
            ProductionLifecycleOutputAdmissionSettlementV1::AlreadyCompleted
        ));
        assert_eq!(
            owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal),
            waiting_before,
            "generic settlement cannot mutate the volatile typed refanout wait"
        );
    }

    #[test]
    fn recovered_broadcast_refanout_treats_adjacent_unlinked_sign_independently() {
        let (
            mut owner,
            services,
            _planner_io,
            _directory,
            broadcast_ordinal,
            paired_ordinal,
            unrelated_ordinal,
        ) = recovered_broadcast_scheduler_fixture();
        assert_eq!(broadcast_ordinal.checked_add(1), Some(paired_ordinal));
        assert!(owner.clear_recovered_broadcast_pair_link_for_test(broadcast_ordinal));
        let before = owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal);
        assert!(!before.declares_pair);
        assert_eq!(before.paired_ordinal, None);

        assert_eq!(
            owner
                .refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(&services, 0)
                .expect("an adjacent unlinked Sign uses ordinary Sign attestation"),
            ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::Refanned {
                ordinal: broadcast_ordinal,
            }
        );
        let after = owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal);
        assert_eq!(after.records[&paired_ordinal].state, LifecycleState::Ready);
        assert_eq!(
            after.records[&unrelated_ordinal].state,
            LifecycleState::Ready
        );
        assert!(after.active_lease.is_none());
        assert!(after.fault.is_none());
    }

    #[test]
    fn recovered_broadcast_refanout_rejects_corrupt_retained_link_without_mutation() {
        let (
            mut owner,
            services,
            _planner_io,
            _directory,
            broadcast_ordinal,
            paired_ordinal,
            unrelated_ordinal,
        ) = recovered_broadcast_scheduler_fixture();
        assert!(owner.corrupt_recovered_broadcast_pair_link_for_test(broadcast_ordinal));
        let before = owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal);
        assert!(before.declares_pair);
        assert_eq!(before.paired_ordinal, None);
        assert!(before.ready_index.contains(&broadcast_ordinal));
        assert!(before.ready_index.contains(&paired_ordinal));
        assert!(before.ready_index.contains(&unrelated_ordinal));

        assert_eq!(
            owner.refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(&services, 0),
            Err(ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidCarrier)
        );
        assert_eq!(
            owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal),
            before,
            "a declared but invalid retained link must fail before coordinator mutation"
        );
        assert!(owner.retire_unrelated_sign_for_finalization_test(unrelated_ordinal));
        assert!(
            !owner.finalization_registry_census_is_exact_for_test(),
            "finalization must reject the corrupted exact next-Sign link"
        );
    }
}
#[cfg(test)]
impl LifecycleCoordinator {
    /// Exercise the sealed production factory without constructing storage.
    pub(super) fn direct_registry_scheduler_inputs_for_test(
        &self,
        registry: &LifecycleWorkRegistryHolder,
    ) -> Result<SchedulerInputs, ProductionSchedulerInputsError> {
        direct_registry_scheduler_inputs(self, registry)
    }
}
#[cfg(test)]
impl ProductionLifecycleOwnerV1 {
    /// Add one closed WAL-backed Sign beside an existing recovered I/O row.
    pub(in crate::sumeragi) fn add_recovered_next_vote_completion_for_test(
        &mut self,
        marker: u8,
    ) -> u128 {
        self.registry
            .add_recovered_next_vote_scheduler_fixture_for_test(
                &mut self.coordinator,
                &self.verified,
                marker,
            )
            .expect("install one exact recovered next-Vote Sign fixture")
    }

    /// Recheck one selected and one preserved row without exposing owner parts.
    pub(in crate::sumeragi) fn recovered_completion_selection_is_exact_for_test(
        &self,
        selected: u128,
        preserved: u128,
    ) -> bool {
        self.coordinator.fault.is_none()
            && self.coordinator.active_lease.as_ref().is_some_and(|lease| {
                lease.ordinal() == selected
                    && self
                        .coordinator
                        .records
                        .get(&selected)
                        .is_some_and(|record| {
                            matches!(record.state, LifecycleState::Claimed(id) if id == lease.id())
                        })
            })
            && self
                .coordinator
                .records
                .get(&preserved)
                .is_some_and(|record| record.state == LifecycleState::Ready)
            && self.coordinator.ready_index.contains(&preserved)
            && !self.coordinator.ready_index.contains(&selected)
    }

    /// Build the opaque recovered Broadcast-pair census used by scheduler tests.
    ///
    /// The returned scalars are ordinals only; every WAL, body, signature, and
    /// concrete-work authority remains owned by the production-shaped owner.
    fn recovered_broadcast_pair_scheduler_fixture_for_test(
        verified: crate::sumeragi::v2::VerifiedHeightContext,
        local_signer: &iroha_crypto::KeyPair,
        root: &std::path::Path,
    ) -> (Self, u128, u128, u128) {
        use super::{CapacityClass, schema::CapacityGeometry};

        let context = super::projection::lifecycle_context(verified.context());
        let mut coordinator = LifecycleCoordinator::new(
            context,
            0,
            CapacityGeometry::new(CapacityClass::ALL.into_iter().map(|class| (class, 8))),
        );
        let (registry, broadcast_ordinal, paired_ordinal, unrelated_ordinal) =
            LifecycleWorkRegistryHolder::recovered_broadcast_pair_scheduler_fixture_for_test(
                &mut coordinator,
                &verified,
                local_signer,
            );
        let body_store = crate::sumeragi::v2_body_store::V2BodyStore::open(
            root.join("body"),
            verified.context().clone(),
        )
        .expect("open exact scheduler fixture body store");
        let (payload_store, recovery) =
            crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
                &root.join("serve"),
                verified.context(),
            )
            .expect("open exact scheduler fixture Serve payload store");
        let serve_payloads = recovery
            .authenticate(&verified, local_signer, &body_store)
            .expect("authenticate empty scheduler fixture Serve payload census");
        (
            Self {
                verified,
                coordinator,
                registry,
                recovered_lifecycle_outputs: None,
                payload_store,
                serve_payloads,
                body_store: Some(body_store),
                body_store_identity: None,
                kura_binding: None,
                apply_service: None,
                adapter_startup: Some(
                    crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1::fixture_for_test(),
                ),
                timeout_supersession_successor: None,
            },
            broadcast_ordinal,
            paired_ordinal,
            unrelated_ordinal,
        )
    }

    /// Clear the retained link without exposing either closed carrier.
    fn clear_recovered_broadcast_pair_link_for_test(&mut self, broadcast_ordinal: u128) -> bool {
        self.registry
            .clear_recovered_broadcast_pair_link_for_test(&self.coordinator, broadcast_ordinal)
    }

    /// Corrupt the retained link digest without exposing either closed carrier.
    fn corrupt_recovered_broadcast_pair_link_for_test(&mut self, broadcast_ordinal: u128) -> bool {
        self.registry
            .corrupt_recovered_broadcast_pair_link_for_test(&self.coordinator, broadcast_ordinal)
    }

    /// Snapshot only copyable/cloneable scheduler state and pair classification.
    fn recovered_broadcast_scheduler_state_for_test(
        &self,
        broadcast_ordinal: u128,
    ) -> RecoveredBroadcastSchedulerStateForTest {
        RecoveredBroadcastSchedulerStateForTest {
            records: self.coordinator.records.clone(),
            key_index: self.coordinator.key_index.clone(),
            owner_index: self.coordinator.owner_index.clone(),
            ready_index: self.coordinator.ready_index.clone(),
            active_lease: self.coordinator.active_lease.clone(),
            high_water: self.coordinator.high_water,
            next_lease: self.coordinator.next_lease,
            durable_records: self.coordinator.durable_records.clone(),
            capacity_used: self.coordinator.capacity_used.clone(),
            capacity_generation: self.coordinator.capacity_generation.clone(),
            observed_generation: self.coordinator.observed_generation.clone(),
            producer_debts: self.coordinator.producer_debts.clone(),
            fault: self.coordinator.fault,
            declares_pair: self
                .registry
                .recovered_lifecycle_signed_broadcast_declares_next_vote(
                    &self.coordinator,
                    broadcast_ordinal,
                ),
            paired_ordinal: self
                .registry
                .recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal(
                    &self.coordinator,
                    broadcast_ordinal,
                ),
        }
    }

    /// Seal a generic runtime retransmit without exposing the typed carrier.
    fn recovered_broadcast_runtime_retransmit_for_test(
        &self,
        broadcast_ordinal: u128,
        generation: u64,
        source_ordinal: u128,
    ) -> Option<super::work_registry::PendingLifecycleOutputAdmissionV1> {
        let record = self.coordinator.records.get(&broadcast_ordinal)?;
        let tag = crate::sumeragi::v2_core::EventTag::new(
            record.key.round().height(),
            record.key.round().view(),
            crate::sumeragi::v2_core::Generation::new(generation),
        );
        self.registry
            .registry()
            .recovered_broadcast_runtime_retransmit_for_test(
                &self.coordinator,
                broadcast_ordinal,
                tag,
                source_ordinal,
            )
    }

    /// Admit one ordinary direct timeout Broadcast behind existing Ready work
    /// and return its unchanged generic-settlement owner.
    fn defer_direct_timeout_broadcast_for_test(
        &mut self,
        marker: u8,
    ) -> Option<(
        u128,
        Box<super::work_registry::PendingLifecycleOutputAdmissionV1>,
    )> {
        use crate::sumeragi::v2_runtime::{
            RuntimeEffectOwnership, bind_adapter_effect_batch_ownership,
        };
        use iroha_data_model::block::consensus_v2 as wire;

        let context = self.verified.context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: u64::from(marker),
        };
        let effect = crate::sumeragi::v2::AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(wire::TimeoutVote {
                round,
                highest_prepare_qc: None,
                signer: 0,
                signature: vec![marker],
            }),
        ));
        let source_ordinal = self.coordinator.high_water.checked_add(0x100)?;
        let tag = crate::sumeragi::v2_core::EventTag::new(
            round.height,
            round.view,
            crate::sumeragi::v2_core::Generation::new(u64::from(marker).saturating_add(1)),
        );
        let ownership = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, source_ordinal)],
        )
        .ok()?
        .pop()?;
        let pending =
            super::work_registry::PendingLifecycleOutputAdmissionV1::seal_exact(effect, ownership)
                .ok()?;
        let expected_ordinal = self.coordinator.high_water.checked_add(1)?;
        let super::ProductionLifecycleOutputAdmissionSettlementV1::Deferred(pending) = self
            .settle_lifecycle_output_admission::<()>(pending, |_effect, _ownership| {
                panic!("a later direct output cannot overtake existing Ready work")
            })
        else {
            return None;
        };
        if self.coordinator.high_water != expected_ordinal {
            return None;
        }
        let ReadyLifecycleBroadcastCarrierV1::RetainedDirectOutput(attestation) = self
            .registry
            .registry()
            .attest_ready_lifecycle_broadcast_carrier(&self.coordinator, expected_ordinal)
            .ok()?
        else {
            return None;
        };
        attestation
            .matches_ready_record(self.coordinator.records.get(&expected_ordinal)?)
            .then(|| (expected_ordinal, Box::new(pending)))
    }

    /// Classify the exact current Ready census without constructing a reducer
    /// fence; scheduler fixtures contain no prospectively-woken rows.
    fn exact_ready_completion_classification_for_test(&self) -> ProductionCompletionReadyWorkV1 {
        let exact_ready = self
            .coordinator
            .records
            .iter()
            .filter_map(|(ordinal, record)| {
                (record.state == LifecycleState::Ready).then_some(*ordinal)
            })
            .collect::<BTreeSet<_>>();
        if exact_ready != self.coordinator.ready_index {
            return ProductionCompletionReadyWorkV1::Invalid;
        }
        self.classify_schedulable_completion_work(&exact_ready)
    }

    /// Drift only one Ready row's physical digest away from its concrete
    /// carrier while preserving the rest of the census.
    fn corrupt_ready_digest_for_test(&mut self, ordinal: u128) -> bool {
        let Some(record) = self.coordinator.records.get_mut(&ordinal) else {
            return false;
        };
        let Some((&slot, &digest)) = record.physical_slots.first_key_value() else {
            return false;
        };
        let replacement = if digest == super::LifecycleDigest::new([0xFD; 32]) {
            super::LifecycleDigest::new([0xFE; 32])
        } else {
            super::LifecycleDigest::new([0xFD; 32])
        };
        record.physical_slots.insert(slot, replacement);
        true
    }

    /// Remove the deliberately unrelated fixture Sign from the bounded owner.
    fn retire_unrelated_sign_for_finalization_test(&mut self, ordinal: u128) -> bool {
        self.retire_ready_work_for_completion_test(ordinal)
    }

    /// Remove one exact Ready carrier and terminalize only its logical row.
    fn retire_ready_work_for_completion_test(&mut self, ordinal: u128) -> bool {
        let Some(record) = self.coordinator.records.get(&ordinal) else {
            return false;
        };
        let Some((&slot, _)) = record.physical_slots.first_key_value() else {
            return false;
        };
        let Some(address) =
            super::work_registry::ConcreteWorkAddress::new(record.owner, ordinal, slot)
        else {
            return false;
        };
        if !self
            .registry
            .registry_for_test_mut()
            .remove_exact_for_test(address)
        {
            return false;
        }
        self.coordinator
            .finish_terminal(ordinal, super::TerminalOutcome::Cancelled)
            .is_ok()
    }

    /// Open one clean production executor before moving this owner's body
    /// store into the matching bounded service worker.
    pub(in crate::sumeragi) fn bind_body_store_to_recovered_completion_io_for_test(
        &mut self,
        services: &mut ProductionV2Services,
        runtime: crate::sumeragi::v2_runtime::SerializedV2Runtime,
        output_guard: std::sync::Arc<crate::sumeragi::output_guard::ConsensusOutputGuard>,
        class_capacity: usize,
    ) -> (
        crate::sumeragi::v2_effects::V2EffectExecutor<
            crate::sumeragi::v2_runtime::SerializedV2Runtime,
        >,
        crate::sumeragi::v2_worker::tests::LifecyclePlannerIoFixture,
    ) {
        let body_store = self
            .body_store
            .take()
            .expect("the startup owner transfers its body store exactly once");
        let identity = body_store.instance_identity();
        let context = self.verified.context().clone();
        let requester = context.roster[0].validator.clone();
        let (executor, body_store) =
            crate::sumeragi::v2_effects::V2EffectExecutor::open_with_body_store(
                runtime,
                body_store,
                context.clone(),
                requester,
                Some(0),
                std::sync::Arc::clone(&output_guard),
                crate::sumeragi::v2_effects::EffectQueueConfig::default(),
            )
            .expect("open the clean recovered Completion executor");
        let fixture = crate::sumeragi::v2_worker::tests::install_lifecycle_planner_io_for_test(
            services,
            context,
            output_guard,
            body_store,
            identity.clone(),
            class_capacity,
        );
        self.body_store_identity = Some(identity);
        (executor, fixture)
    }

    /// Exercise the production all-row Completion transaction without a
    /// forgeable runner snapshot.
    pub(in crate::sumeragi) fn dispatch_completion_for_test(
        &mut self,
        services: &ProductionV2Services,
        executor: &mut crate::sumeragi::v2_effects::V2EffectExecutor<
            crate::sumeragi::v2_runtime::SerializedV2Runtime,
        >,
        runner_debt: u64,
    ) -> Result<ProductionCompletionDispatchV1, ProductionCompletionDispatchErrorV1> {
        self.dispatch_completion_with_runner_debt(services, executor, runner_debt)
    }

    /// Recheck the exact finalization-only registry census without exposing it.
    fn finalization_registry_census_is_exact_for_test(&self) -> bool {
        self.registry
            .registry_for_test()
            .exactly_covers_finalization_work(&self.coordinator)
    }

    /// Build one storage-owning production owner around the exact selected
    /// Fetch carrier used by the cross-module planner transaction regression.
    pub(in crate::sumeragi) fn waiting_fetch_for_ingress_test(
        verified: crate::sumeragi::v2::VerifiedHeightContext,
        prepared: &PreparedLifecycleIngressSelector,
        effect: crate::sumeragi::v2::AdapterEffect,
        pending: crate::sumeragi::v2_runtime::PendingRuntimeEffectBinding,
        local_signer: &iroha_crypto::KeyPair,
        root: &std::path::Path,
    ) -> (Self, u128, super::WaitSource) {
        use super::{
            AdmissionDecision, AdmissionRequest, CapacityClass, WaitToken,
            schema::CapacityGeometry,
            work_registry::{ConcreteLifecycleWork, ConcreteWorkAddress},
        };
        let (context, _, _, _, expected_key, expected_root, source) = prepared
            .certified_fetch_ready_authority_for_test()
            .expect("selected Fetch must derive its exact lifecycle authority");
        assert_eq!(
            context,
            super::projection::lifecycle_context(verified.context()),
            "selected Fetch and verified owner must share one context"
        );
        let mut coordinator = LifecycleCoordinator::new(
            context,
            0,
            CapacityGeometry::new(CapacityClass::ALL.into_iter().map(|class| (class, 8))),
        );
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let candidate = super::replay_authority::exact_pending_certified_fetch_candidate_fixture(
            &verified, &effect, &pending,
        )
        .expect("the verified selected Fetch must derive exact replay authority");
        assert_eq!(candidate.key, expected_key);
        assert_eq!(candidate.causal_root, expected_root);
        assert_eq!(candidate.work_class, LifecycleWorkClass::Fetch);
        let replay_authority = candidate.replay_authority.clone();
        let work =
            ConcreteLifecycleWork::from_candidate_for_test(effect, pending, replay_authority)
                .unwrap_or_else(|(error, _, _)| {
                    panic!("the selected Fetch carrier is invalid: {error:?}")
                });
        let work_digest = work.digest();
        let AdmissionDecision::Admitted {
            owner,
            ordinal,
            producer_turn_ordinal: None,
        } = coordinator.admit(AdmissionRequest::Candidate(candidate))
        else {
            panic!("the exact selected Fetch candidate must enter the coordinator")
        };
        let slot = super::PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let address = ConcreteWorkAddress::new(owner, ordinal, slot)
            .expect("the admitted Fetch owns one exact concrete address");
        registry
            .registry_mut()
            .install(address, work_digest, work)
            .unwrap_or_else(|(error, _)| {
                panic!("the exact selected Fetch must enter the concrete registry: {error:?}")
            });
        let record = coordinator
            .records
            .get_mut(&ordinal)
            .expect("admitted Fetch owns its logical record");
        assert_eq!(record.key, expected_key);
        assert_eq!(record.owner.causal_root(), expected_root);
        assert_eq!(record.work_class, LifecycleWorkClass::Fetch);
        assert_eq!(record.physical_slots.get(&slot), Some(&work_digest));
        assert!(coordinator.ready_index.remove(&ordinal));
        record.state = LifecycleState::Waiting(WaitToken::new(source, 0));
        assert!(coordinator.observed_generation.insert(source, 0).is_none());
        let body_store = crate::sumeragi::v2_body_store::V2BodyStore::open(
            root.join("body"),
            verified.context().clone(),
        )
        .expect("open exact owner body store");
        let (payload_store, recovery) =
            crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
                &root.join("serve"),
                verified.context(),
            )
            .expect("open exact owner Serve payload store");
        let serve_payloads = recovery
            .authenticate(&verified, local_signer, &body_store)
            .expect("authenticate exact owner Serve payload census");
        (
            Self {
                verified,
                coordinator,
                registry,
                recovered_lifecycle_outputs: None,
                payload_store,
                serve_payloads,
                body_store: Some(body_store),
                body_store_identity: None,
                kura_binding: None,
                apply_service: None,
                adapter_startup: Some(
                    crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1::fixture_for_test(),
                ),
                timeout_supersession_successor: None,
            },
            ordinal,
            source,
        )
    }
    /// Move the owner's exact startup body store into the bounded test worker
    /// while retaining only its comparison seal in the running owner.
    pub(in crate::sumeragi) fn bind_body_store_to_planner_io_for_test(
        &mut self,
        services: &mut ProductionV2Services,
        output_guard: std::sync::Arc<crate::sumeragi::output_guard::ConsensusOutputGuard>,
        class_capacity: usize,
    ) -> crate::sumeragi::v2_worker::tests::LifecyclePlannerIoFixture {
        let body_store = self
            .body_store
            .take()
            .expect("the startup owner transfers its body store exactly once");
        let identity = body_store.instance_identity();
        let fixture = crate::sumeragi::v2_worker::tests::install_lifecycle_planner_io_for_test(
            services,
            self.verified.context().clone(),
            output_guard,
            body_store,
            identity.clone(),
            class_capacity,
        );
        self.body_store_identity = Some(identity);
        fixture
    }
    /// Complete one persisted ordinary certified-Fetch response through the
    /// production Phase-B coordinator transaction.
    pub(in crate::sumeragi) fn complete_certified_fetch_for_test(
        &mut self,
        executor: &mut V2EffectExecutor<SerializedV2Runtime>,
        services: &mut ProductionV2Services,
        ingress: &crate::sumeragi::FairV2Ingress,
        completion: crate::sumeragi::v2_worker::PreparedCertifiedFetchBodyPersistenceCompletion,
    ) -> Result<(), super::selector::CertifiedFetchBodyPersistenceCompletionError> {
        self.coordinator.complete_certified_fetch_body_persistence(
            &mut self.registry,
            executor,
            services,
            ingress,
            completion,
        )
    }
    /// Project the exact Fetch wait state without exposing mutable owner parts.
    pub(in crate::sumeragi) fn fetch_wait_projection_for_test(
        &self,
        ordinal: u128,
        source: super::WaitSource,
    ) -> (
        Option<LifecycleState>,
        Option<u64>,
        Option<super::CoordinatorFault>,
        bool,
    ) {
        (
            self.coordinator
                .records
                .get(&ordinal)
                .map(|record| record.state),
            self.coordinator.observed_generation.get(&source).copied(),
            self.coordinator.fault,
            self.coordinator.active_lease.is_some(),
        )
    }
    /// Rejoin the sole executor owner, exact external wait, and recovered WAL
    /// registry carrier after the production request publication cut.
    pub(in crate::sumeragi) fn recovered_fetch_dispatch_projection_for_test(
        &mut self,
        executor: &crate::sumeragi::v2_effects::V2EffectExecutor<
            crate::sumeragi::v2_runtime::SerializedV2Runtime,
        >,
        ordinal: u128,
    ) -> Option<(
        super::work_registry::RecoveredDecisionFetchDispatchKeyV1,
        iroha_crypto::HashOf<iroha_data_model::block::consensus_v2::CertifiedBodyRequest>,
        super::WaitToken,
    )> {
        let (key, request_hash) = executor.recovered_decision_fetch_owner_for_test()?;
        if key.lifecycle_ordinal() != ordinal || self.coordinator.active_lease.is_some() {
            return None;
        }
        let wait_source = super::projection::certified_fetch_wait_source(request_hash);
        let record = self.coordinator.records.get(&ordinal)?;
        let LifecycleState::Waiting(wait) = record.state else {
            return None;
        };
        (wait.source() == wait_source
            && self.coordinator.observed_generation.get(&wait_source)
                == Some(&wait.observed_generation())
            && !self.coordinator.ready_index.contains(&ordinal)
            && self
                .registry
                .registry_mut()
                .matches_waiting_dispatched_recovered_decision_fetch(
                    &self.coordinator,
                    key,
                    wait_source,
                )
            && self
                .registry
                .registry_mut()
                .exactly_covers_all_live_work(&self.verified, &self.coordinator))
        .then_some((key, request_hash, wait))
    }
    /// Corrupt or restore only the volatile recovered-Fetch wait-source join.
    pub(in crate::sumeragi) fn replace_recovered_fetch_wait_source_for_test(
        &mut self,
        ordinal: u128,
        replacement: super::WaitSource,
    ) -> Option<super::WaitSource> {
        self.registry
            .registry_mut()
            .replace_recovered_fetch_wait_source_for_test(ordinal, replacement)
    }
}

#[cfg(test)]
mod unified_completion_classifier_tests {
    use super::*;

    #[test]
    fn supported_ready_coexistence_selects_only_a_full_census_transaction() {
        assert_eq!(
            classify_completion_ready_classes(
                &[
                    LifecycleWorkClass::Validate,
                    LifecycleWorkClass::Broadcast,
                    LifecycleWorkClass::SignVote,
                    LifecycleWorkClass::Apply,
                    LifecycleWorkClass::Fetch,
                ],
                false,
                false,
            ),
            ProductionCompletionReadyWorkV1::RecoveredLifecycleBroadcast
        );
        assert_eq!(
            classify_completion_ready_classes(
                &[
                    LifecycleWorkClass::Broadcast,
                    LifecycleWorkClass::ProducerTurn,
                ],
                false,
                false,
            ),
            ProductionCompletionReadyWorkV1::PassThrough
        );
        assert_eq!(
            classify_completion_ready_classes(
                &[
                    LifecycleWorkClass::Broadcast,
                    LifecycleWorkClass::CertifiedServe,
                ],
                false,
                false,
            ),
            ProductionCompletionReadyWorkV1::PassThrough
        );
        assert_eq!(
            classify_completion_ready_classes(
                &[
                    LifecycleWorkClass::Apply,
                    LifecycleWorkClass::SignProposal,
                    LifecycleWorkClass::Fetch,
                ],
                false,
                false,
            ),
            ProductionCompletionReadyWorkV1::CompletionIo
        );
    }

    #[test]
    fn exact_single_ready_io_classes_use_the_same_composite_dispatcher() {
        assert_eq!(
            classify_completion_ready_classes(&[LifecycleWorkClass::Apply], false, false),
            ProductionCompletionReadyWorkV1::CompletionIo
        );
        assert_eq!(
            classify_completion_ready_classes(&[LifecycleWorkClass::SignTimeout], false, false),
            ProductionCompletionReadyWorkV1::CompletionIo
        );
        assert_eq!(
            classify_completion_ready_classes(&[LifecycleWorkClass::Fetch], false, false),
            ProductionCompletionReadyWorkV1::CompletionIo
        );
        assert_eq!(
            classify_completion_ready_classes(&[], false, false),
            ProductionCompletionReadyWorkV1::None
        );
    }

    #[test]
    fn later_retained_direct_broadcast_routes_validate_to_completion_io() {
        assert_eq!(
            classify_completion_ready_classes(&[LifecycleWorkClass::Validate], true, false),
            ProductionCompletionReadyWorkV1::CompletionIo
        );
        assert_eq!(
            classify_completion_ready_classes(&[LifecycleWorkClass::Validate], true, true),
            ProductionCompletionReadyWorkV1::PassThrough,
            "an oldest direct output remains owned by generic settlement"
        );
        assert_eq!(
            classify_completion_ready_classes(&[], true, true),
            ProductionCompletionReadyWorkV1::PassThrough
        );
    }
}

#[cfg(test)]
mod certified_serve_scheduler_tests {
    include!("tests/v2_lifecycle_scheduler_certified_serve_cases.rs");
}
