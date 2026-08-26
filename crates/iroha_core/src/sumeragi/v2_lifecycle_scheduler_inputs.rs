//! Sealed production authentication for lifecycle planner inputs.
#[cfg(test)]
use super::work_registry::ReadyLifecycleBroadcastCarrierV1;
use super::{
    CapacityClass, LifecycleCoordinator, LifecycleState, LifecycleWorkClass,
    LifecycleWorkRegistryHolder, PreparedLifecycleIngressSelector, ProductionLifecycleOwnerV1,
    concrete_admission::ProductionCertifiedFetchAdmissionSettlementV1,
    open::ReadyRecoveredLifecycleBroadcastAttestationV1,
    schema::{AttestedReadyValidateDemand, SchedulerInputs, SchedulerReadyInputs},
    selector::{
        PreparedCertifiedServeExactDequeueV1,
        RecoveredDecisionFetchBodyPersistencePreparationFailureV1,
    },
    work_registry::{
        ClaimedCertifiedServeDispatchErrorV1, ClaimedCertifiedServeDispatchV1,
        ClaimedProducerTurnErrorV1, ClaimedProducerTurnV1, ConcreteLifecycleWorkRegistry,
        ReadyCertifiedServeAttestationV1, ReadyLifecycleDecisionApplyDemandV1,
        ReadyProducerTurnCensusAttestationErrorV1, RegistryError,
        SchedulableLifecycleBroadcastCarrierV1, SchedulableRetainedDirectBroadcastAttestationV1,
    },
};
#[cfg(test)]
use crate::sumeragi::v2_runner::LifecycleRunnerRankSnapshot;
use crate::sumeragi::{
    v2::VerifiedHeightContext,
    v2_effects::{
        EffectExecutorError, LifecycleModeRankSnapshot,
        RecoveredDecisionFetchRequestRegistrationErrorV1,
        RecoveredDecisionFetchResponseClaimErrorV1, V2EffectExecutor,
    },
    v2_runner::{LifecycleCurrentRunnerTurn, LifecycleRunnerRankTarget},
    v2_runtime::SerializedV2Runtime,
    v2_worker::{
        AuthenticatedLifecycleIoCapacity, LifecycleCompletionCapacityProbeV1,
        LifecycleIoCapacityCaptureFailure, LifecycleIoCapacityReservation, LifecycleIoCapacityWait,
        LifecycleIoCapacityWaitStatus, ProductionV2Services,
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
    lifecycle_decision_apply_attestation: Option<
        super::work_registry::ReadyLifecycleDecisionApplyAttestationV1,
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
        lifecycle_decision_apply_attestation,
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
    lifecycle_decision_apply_attestation: Option<
        super::work_registry::ReadyLifecycleDecisionApplyAttestationV1,
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
        lifecycle_decision_apply_attestation,
        recovered_sign_attestation,
        recovered_fetch_attestation,
        physical_capacity_available,
        live_debts,
    )
}
fn authenticated_schedulable_retained_direct_broadcast_row(
    factory: &AuthenticatedSchedulerInputsFactory,
    record: &super::LifecycleRecord,
    attestation: SchedulableRetainedDirectBroadcastAttestationV1,
    live_debts: [u64; 6],
) -> Option<SchedulerReadyInputs> {
    attestation
        .matches_schedulable_record(record)
        .then_some(())?;
    authenticated_ready_row_with_physical_capacity(
        factory, record, None, None, None, None, false, live_debts,
    )
}
fn authenticated_ready_recovered_lifecycle_broadcast_row(
    factory: &AuthenticatedSchedulerInputsFactory,
    record: &super::LifecycleRecord,
    attestation: ReadyRecoveredLifecycleBroadcastAttestationV1,
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
    /// The exact lifecycle Decision Apply carrier could not be bound to this row.
    InvalidLifecycleDecisionApplyCarrier {
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
    CapacityUnavailable {
        /// Exact live Apply child whose executor barrier was installed before
        /// the joint capacity census, if this census contained one.
        protected_live_apply_ordinal: Option<u128>,
    },
    /// The selected lifecycle Validate now owns one exact durable worker command.
    ValidateQueued {
        /// Exact selected lifecycle ordinal.
        ordinal: u128,
    },
    /// The selected lifecycle Decision Apply now owns one dedicated worker command.
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
        /// Exact actor-global ordinal of the newly Ready child.
        child_ordinal: u128,
        /// Newly Ready child class.
        child: LifecycleWorkClass,
    },
    /// One ordinary body parent is parked on the adapter reducer fence.
    ReducerFenceWait {
        /// Exact waiting parent ordinal.
        ordinal: u128,
        /// Exact reducer-fence generation which must advance before retry.
        wait: super::WaitToken,
    },
    /// One Validate completion durably advanced without creating a child.
    ValidateNoSuccessor {
        /// Exact terminalized Validate ordinal.
        ordinal: u128,
    },
}

/// Move-only result of resolving one retained Ready Validate successor.
#[must_use = "the exact Validate successor must resolve or remain retained"]
pub(in crate::sumeragi) enum ReadyValidateSuccessorDispatchV1 {
    /// The exact successor completed its synchronous or bounded-I/O dispatch.
    Resolved(ProductionCompletionDispatchV1),
    /// Physical capacity was unavailable; the unchanged token must be reparked.
    CapacityUnavailable(super::ReadyValidateSuccessorV1),
    /// The exact row is parked on its reducer fence and retains the same token.
    ReducerFencePending {
        /// Exact move-only successor identity.
        successor: super::ReadyValidateSuccessorV1,
        /// Exact reducer fence wait installed on the row.
        wait: super::WaitToken,
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
    /// A Ready Apply could not reconcile or reserve its exact executor work.
    LiveApplyReconciliation(EffectExecutorError),
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
    /// The oldest Ready row is a direct output retained by executor settlement.
    RetainedDirectOutput,
    /// Another ordinary/stateful owner must run.
    PassThrough,
    /// Coordinator state is not safe to classify without restart.
    Invalid,
}
enum AuthenticatedLifecycleCompletionReadyV1 {
    Validate(AttestedReadyValidateDemand),
    Apply(super::work_registry::ReadyLifecycleDecisionApplyAttestationV1),
    Sign(super::work_registry::ReadyRecoveredLifecycleSignAttestationV1),
    Fetch(super::work_registry::ReadyRecoveredDecisionFetchAttestationV1),
    CertifiedBody(super::work_registry::ReadyCertifiedBodyPipelineAttestationV1),
    RetainedDirectBroadcast(SchedulableRetainedDirectBroadcastAttestationV1),
    RetainedRecoveredBroadcast(ReadyRecoveredLifecycleBroadcastAttestationV1),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SchedulableCompletionBroadcastCarrierV1 {
    RetainedDirectOutput(SchedulableRetainedDirectBroadcastAttestationV1),
    RetainedRecoveredOutput(ReadyRecoveredLifecycleBroadcastAttestationV1),
    RecoveredRefanout,
}

fn classify_completion_ready_classes(
    classes: &[LifecycleWorkClass],
    has_retained_output: bool,
    oldest_is_retained_direct_output: bool,
) -> ProductionCompletionReadyWorkV1 {
    if oldest_is_retained_direct_output {
        return ProductionCompletionReadyWorkV1::RetainedDirectOutput;
    }
    if classes.is_empty() {
        return if has_retained_output {
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
    /// The selected family could not seal its pending Fetch admission owner.
    CertifiedFetchAdmissionPreparation,
    /// The sealed pending Fetch could not enter or rejoin durable ownership.
    CertifiedFetchAdmissionSettlement,
    /// The admitted Fetch did not bind the exact waiting row and registry incumbent.
    InvalidSelectedCarrier,
    /// Logical admission capacity retained the selected occurrence unchanged.
    CertifiedFetchAdmissionDeferred {
        /// Complete selector retained for a later admission retry.
        _prepared: PreparedLifecycleIngressSelector,
    },
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
impl ProductionIngressSchedulerInputsError {
    /// Return the closed failure class without exposing retained authorities.
    pub(crate) const fn reason(&self) -> &'static str {
        match self {
            Self::CoordinatorFaulted { .. } => "coordinator faulted",
            Self::UnsettledLease { .. } => "unsettled lease",
            Self::ForeignModeObservation => "foreign mode observation",
            Self::StaleModeObservation => "stale mode observation",
            Self::ForeignOutputGuard => "foreign output guard",
            Self::ForeignRunnerObservation => "foreign runner observation",
            Self::BodyStoreNotBound => "body store not bound",
            Self::CertifiedFetchAdmissionPreparation => {
                "certified Fetch admission preparation failed"
            }
            Self::CertifiedFetchAdmissionSettlement => {
                "certified Fetch admission settlement failed"
            }
            Self::InvalidSelectedCarrier => "invalid selected carrier",
            Self::CertifiedFetchAdmissionDeferred { .. } => "certified Fetch admission deferred",
            Self::CompetingReadyWork => "competing Ready work",
            Self::CommandPreparation { .. } => "command preparation failed",
            Self::InFlightSelectedWork { .. } => "selected work already in flight",
            Self::InvalidReservedCommand => "invalid reserved command",
            Self::UnexpectedPlan => "unexpected scheduler plan",
            Self::SettlementFault { .. } => "settlement fault",
            Self::Service { .. } => "service capacity capture failed",
        }
    }
}
/// Authenticate the complete subset of Ready work already owned directly by
/// the concrete registry.
///
/// The current sound subset consists of closed Validate completion carriers and
/// the exact lifecycle Decision Apply carrier. I/O-bearing Validate and Apply
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
                    .attest_ready_lifecycle_decision_apply(coordinator, *ordinal)
                    .map_err(|_| {
                        ProductionSchedulerInputsError::InvalidLifecycleDecisionApplyCarrier {
                            ordinal: *ordinal,
                        }
                    })?;
                let ReadyLifecycleDecisionApplyDemandV1::BoundedIo = attestation.demand();
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

    fn attest_schedulable_completion_broadcast_carrier(
        &self,
        ordinal: u128,
        fence: Option<crate::sumeragi::v2::LifecycleReducerFenceObservationV1>,
    ) -> Result<SchedulableCompletionBroadcastCarrierV1, RegistryError> {
        let retained_recovered = self.attest_ready_recovered_lifecycle_broadcast(ordinal);
        let registered = self
            .registry
            .registry()
            .attest_schedulable_lifecycle_broadcast_carrier(&self.coordinator, ordinal, fence);
        match (retained_recovered, registered) {
            (Some(attestation), Err(RegistryError::Missing)) => {
                Ok(SchedulableCompletionBroadcastCarrierV1::RetainedRecoveredOutput(attestation))
            }
            (
                None,
                Ok(SchedulableLifecycleBroadcastCarrierV1::RetainedDirectOutput(attestation)),
            ) => Ok(SchedulableCompletionBroadcastCarrierV1::RetainedDirectOutput(attestation)),
            (None, Ok(SchedulableLifecycleBroadcastCarrierV1::RecoveredRefanout)) => {
                Ok(SchedulableCompletionBroadcastCarrierV1::RecoveredRefanout)
            }
            (None, Err(error)) => Err(error),
            (Some(_), Ok(_) | Err(_)) => Err(RegistryError::CorruptWork),
        }
    }

    pub(super) fn classify_schedulable_completion_work(
        &self,
        schedulable: &BTreeSet<u128>,
        fence: Option<crate::sumeragi::v2::LifecycleReducerFenceObservationV1>,
    ) -> ProductionCompletionReadyWorkV1 {
        let mut retained_outputs = BTreeSet::new();
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
            match self.attest_schedulable_completion_broadcast_carrier(*ordinal, fence) {
                Ok(SchedulableCompletionBroadcastCarrierV1::RetainedDirectOutput(_)) => {
                    if !retained_outputs.insert(*ordinal)
                        || !retained_direct_outputs.insert(*ordinal)
                    {
                        return ProductionCompletionReadyWorkV1::Invalid;
                    }
                }
                Ok(SchedulableCompletionBroadcastCarrierV1::RetainedRecoveredOutput(_)) => {
                    if !retained_outputs.insert(*ordinal) {
                        return ProductionCompletionReadyWorkV1::Invalid;
                    }
                }
                Ok(SchedulableCompletionBroadcastCarrierV1::RecoveredRefanout) => {
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
            !retained_outputs.is_empty(),
            oldest_is_retained_direct_output,
        )
    }

    /// Classify Ready work without claiming a lease or reserving capacity.
    ///
    /// Broadcast refanout and lifecycle Decision Apply/Sign/Fetch each authenticate their
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
        self.classify_schedulable_completion_work(&schedulable, Some(fence))
    }

    /// Wake only a fence-expired direct Broadcast that globally precedes post-Apply cold output.
    ///
    /// The full schedulable census is reclassified before the reducer generation
    /// advances. A prospectively Ready non-direct owner is rejected here so a
    /// higher cold Broadcast cannot overtake it while it is absent from
    /// `ready_index`.
    pub(in crate::sumeragi) fn wake_apply_terminal_direct_broadcast_if_fenced(
        &mut self,
        fence: crate::sumeragi::v2::LifecycleReducerFenceObservationV1,
    ) -> Result<bool, ProductionSchedulerInputsError> {
        if let Some(fault) = self.coordinator.fault {
            return Err(ProductionSchedulerInputsError::CoordinatorFaulted(fault));
        }
        if let Some(lease) = self.coordinator.active_lease.as_ref() {
            return Err(ProductionSchedulerInputsError::UnsettledLease(lease.id));
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
            return Err(ProductionSchedulerInputsError::InvalidReadyCensus);
        }
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
        let mut schedulable = exact_ready;
        schedulable.extend(reducer_fence_wakes.iter().copied());
        let Some(ordinal) = schedulable.first().copied() else {
            return Ok(false);
        };
        if !reducer_fence_wakes.contains(&ordinal) {
            return Ok(false);
        }
        if self.classify_schedulable_completion_work(&schedulable, Some(fence))
            != ProductionCompletionReadyWorkV1::RetainedDirectOutput
        {
            let record = self
                .coordinator
                .records
                .get(&ordinal)
                .ok_or(ProductionSchedulerInputsError::InvalidReadyCensus)?;
            return Err(ProductionSchedulerInputsError::UnsupportedReadyCarrier {
                ordinal,
                work_class: record.work_class,
            });
        }
        for wake_ordinal in reducer_fence_wakes {
            let record = self
                .coordinator
                .records
                .get(&wake_ordinal)
                .ok_or(ProductionSchedulerInputsError::InvalidReadyCensus)?;
            let Ok(SchedulableLifecycleBroadcastCarrierV1::RetainedDirectOutput(attestation)) =
                self.registry
                    .registry()
                    .attest_schedulable_lifecycle_broadcast_carrier(
                        &self.coordinator,
                        wake_ordinal,
                        Some(fence),
                    )
            else {
                return Err(ProductionSchedulerInputsError::UnsupportedReadyCarrier {
                    ordinal: wake_ordinal,
                    work_class: record.work_class,
                });
            };
            if !attestation.matches_schedulable_record(record) {
                return Err(ProductionSchedulerInputsError::UnsupportedReadyCarrier {
                    ordinal: wake_ordinal,
                    work_class: record.work_class,
                });
            }
        }
        self.coordinator
            .advance_observed_generation(fence.source(), fence.generation());
        let exact_ready_after_wake = self
            .coordinator
            .records
            .iter()
            .filter_map(|(ordinal, record)| {
                matches!(record.state, LifecycleState::Ready).then_some(*ordinal)
            })
            .collect::<BTreeSet<_>>();
        if exact_ready_after_wake != self.coordinator.ready_index
            || self.coordinator.ready_index.first().copied() != Some(ordinal)
        {
            return Err(ProductionSchedulerInputsError::InvalidReadyCensus);
        }
        Ok(true)
    }

    /// Seal the already-Ready global-minimum direct Broadcast after Apply terminal settlement.
    ///
    /// Reducer-fence wake publication is deliberately a separate outer turn;
    /// this read-only mint cannot combine generation advancement with service I/O.
    pub(in crate::sumeragi) fn prepare_apply_terminal_direct_broadcast(
        &self,
    ) -> Option<super::work_registry::PreparedApplyTerminalDirectBroadcastV1> {
        let exact_ready = self
            .coordinator
            .records
            .iter()
            .filter_map(|(ordinal, record)| {
                matches!(record.state, LifecycleState::Ready).then_some(*ordinal)
            })
            .collect::<BTreeSet<_>>();
        if exact_ready != self.coordinator.ready_index {
            return None;
        }
        let ordinal = self.coordinator.ready_index.first().copied()?;
        self.registry
            .registry()
            .prepare_apply_terminal_direct_broadcast(&self.coordinator, ordinal)
            .ok()
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
        services: &mut ProductionV2Services,
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
                #[cfg(test)]
                eprintln!("certified Fetch registry execution projection failed: {error:?}");
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
                Ok(ProductionCompletionDispatchV1::ReducerFenceWait { ordinal, wait })
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
                executor
                    .release_live_lifecycle_validate_successor(ordinal)
                    .map_err(ProductionCompletionDispatchErrorV1::LiveApplyReconciliation)?;
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
                let child_ordinal = transition.child_ordinal();
                if transition.persist_and_publish().is_err() {
                    iroha_logger::error!(
                        ordinal,
                        "Ready Validate-to-Sign transaction publication failed"
                    );
                    return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
                }
                executor
                    .release_live_lifecycle_validate_successor(ordinal)
                    .map_err(ProductionCompletionDispatchErrorV1::LiveApplyReconciliation)?;
                Ok(ProductionCompletionDispatchV1::BodyStageAdvanced {
                    parent_ordinal: ordinal,
                    child_ordinal,
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
                let child_ordinal = transition.child_ordinal();
                if transition.persist_and_publish().is_err() {
                    iroha_logger::error!(
                        ordinal,
                        "Ready Validate-to-report transaction publication failed"
                    );
                    return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
                }
                executor
                    .release_live_lifecycle_validate_successor(ordinal)
                    .map_err(ProductionCompletionDispatchErrorV1::LiveApplyReconciliation)?;
                Ok(ProductionCompletionDispatchV1::BodyStageAdvanced {
                    parent_ordinal: ordinal,
                    child_ordinal,
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
                let child_ordinal = transition.child_ordinal();
                if transition.persist_and_publish().is_err() {
                    iroha_logger::error!(
                        ordinal,
                        "Ready Validate-to-Apply transaction publication failed"
                    );
                    return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
                }
                let authority = self
                    .registry
                    .prepare_ready_live_decision_apply_reconciliation(
                        &self.coordinator,
                        child_ordinal,
                    )
                    .map_err(|_| ProductionCompletionDispatchErrorV1::InvalidCarrier)?
                    .ok_or(ProductionCompletionDispatchErrorV1::InvalidCarrier)?;
                if authority.dispatch_key().lifecycle_ordinal() != child_ordinal {
                    return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
                }
                if let Err(error) =
                    executor.reconcile_live_lifecycle_decision_apply(authority, services)
                {
                    self.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
                    return Err(
                        ProductionCompletionDispatchErrorV1::LiveApplyReconciliation(error),
                    );
                }
                Ok(ProductionCompletionDispatchV1::BodyStageAdvanced {
                    parent_ordinal: ordinal,
                    child_ordinal,
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
            Err(error) => {
                iroha_logger::error!(
                    ?error,
                    ordinal,
                    "certified Fetch registry execution projection failed"
                );
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
                return Ok(ProductionCompletionDispatchV1::ReducerFenceWait { ordinal, wait });
            }
            Ok(crate::sumeragi::v2::CertifiedFetchStoreAdapterPreparationV1::Inactive) => {
                iroha_logger::error!(ordinal, "certified Fetch adapter projection was inactive");
                drop(execution);
                assert!(self.coordinator.rollback_unpublished_turn(&lease));
                return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
            }
            Err(error) => {
                iroha_logger::error!(
                    ?error,
                    ordinal,
                    "certified Fetch serialized adapter projection failed"
                );
                drop(execution);
                assert!(self.coordinator.rollback_unpublished_turn(&lease));
                return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
            }
        };
        let successor = match execution.seal_store_successor(adapter) {
            Ok(successor) => successor,
            Err(error) => {
                iroha_logger::error!(
                    ?error,
                    ordinal,
                    "certified Fetch-to-Store successor seal failed"
                );
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
            Err(error) => {
                iroha_logger::error!(
                    ?error,
                    ordinal,
                    "certified Fetch-to-Store transition projection failed"
                );
                assert!(self.coordinator.rollback_unpublished_turn(&lease));
                return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
            }
        };
        let child_ordinal = transition.child_ordinal();
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
            child_ordinal,
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
        let retry_marker = match executor
            .prepare_published_lifecycle_validate_retry_marker(execution.durable_body_receipt())
        {
            Ok(marker) => marker,
            Err(error) => {
                iroha_logger::error!(
                    ?error,
                    ordinal,
                    "lifecycle Store-to-Validate retry marker preflight failed"
                );
                drop(execution);
                assert!(self.coordinator.rollback_unpublished_turn(&lease));
                return Err(ProductionCompletionDispatchErrorV1::DispatchProjection);
            }
        };
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
                return Ok(ProductionCompletionDispatchV1::ReducerFenceWait { ordinal, wait });
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
        let retry_marker = match retry_marker.bind_validate_successor(
            successor.validate_effect(),
            successor.pending_effect_binding(),
        ) {
            Ok(marker) => marker,
            Err(error) => {
                iroha_logger::error!(
                    %error,
                    ordinal,
                    "lifecycle Store-to-Validate retry marker sealing failed"
                );
                drop(successor);
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
        let child_ordinal = transition.child_ordinal();
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
        executor.commit_published_lifecycle_validate_retry_marker(retry_marker, child_ordinal);
        operation.complete();
        Ok(ProductionCompletionDispatchV1::BodyStageAdvanced {
            parent_ordinal: ordinal,
            child_ordinal,
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
        services: &mut ProductionV2Services,
        executor: &mut V2EffectExecutor<SerializedV2Runtime>,
        runner_debt: u64,
    ) -> Result<ProductionCompletionDispatchV1, ProductionCompletionDispatchErrorV1> {
        self.dispatch_completion_with_runner_debt_and_required_ordinal(
            services,
            executor,
            runner_debt,
            None,
        )
    }

    /// Run the same complete authenticated census while requiring its natural
    /// scheduler winner to be one exact retained Ready ordinal.
    pub(super) fn dispatch_completion_requiring_ready_ordinal(
        &mut self,
        services: &mut ProductionV2Services,
        executor: &mut V2EffectExecutor<SerializedV2Runtime>,
        runner_debt: u64,
        required_ordinal: u128,
    ) -> Result<ProductionCompletionDispatchV1, ProductionCompletionDispatchErrorV1> {
        self.dispatch_completion_with_runner_debt_and_required_ordinal(
            services,
            executor,
            runner_debt,
            Some(required_ordinal),
        )
    }

    fn dispatch_completion_with_runner_debt_and_required_ordinal(
        &mut self,
        services: &mut ProductionV2Services,
        executor: &mut V2EffectExecutor<SerializedV2Runtime>,
        runner_debt: u64,
        required_ordinal: Option<u128>,
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
        // Live Validate-to-Apply publication installs its exact executor owner
        // before exposing the Ready child. Classification remains read-only:
        // crash-open work is recovered-lineage, and a live child without its
        // already-installed owner is corrupt.
        let live_apply_ordinals = current_ready
            .iter()
            .copied()
            .filter(|ordinal| {
                self.coordinator
                    .records
                    .get(ordinal)
                    .is_some_and(|record| record.work_class == LifecycleWorkClass::Apply)
            })
            .collect::<Vec<_>>();
        let mut protected_live_apply_ordinal = None;
        for ordinal in live_apply_ordinals {
            let authority = self
                .registry
                .prepare_ready_live_decision_apply_reconciliation(&self.coordinator, ordinal)
                .map_err(|_| ProductionCompletionDispatchErrorV1::InvalidCarrier)?;
            if let Some(authority) = authority {
                let protected_ordinal = authority.dispatch_key().lifecycle_ordinal();
                if protected_ordinal != ordinal
                    || protected_live_apply_ordinal
                        .replace(protected_ordinal)
                        .is_some()
                    || !executor.exactly_owns_live_lifecycle_decision_apply(&authority)
                {
                    return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
                }
            }
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
                        LifecycleCompletionCapacityProbeV1::Validate {
                            ordinal: *ordinal,
                            key: attestation.dispatch_key(),
                        },
                    );
                    if attestation.requires_io_dispatch() {
                        validate_io.insert(*ordinal);
                    }
                    (
                        AuthenticatedLifecycleCompletionReadyV1::Validate(attestation),
                        probe,
                    )
                }
                LifecycleWorkClass::Apply => {
                    let attestation = self
                        .registry
                        .attest_ready_lifecycle_decision_apply(&self.coordinator, *ordinal)
                        .map_err(|_| ProductionCompletionDispatchErrorV1::InvalidCarrier)?;
                    if attestation.demand() != ReadyLifecycleDecisionApplyDemandV1::BoundedIo
                        || !attestation.dispatch_key().matches_height_context(context)
                    {
                        return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
                    }
                    let key = attestation.dispatch_key();
                    let executor_available = executor
                        .lifecycle_decision_apply_dispatch_available()
                        .map_err(ProductionCompletionDispatchErrorV1::LiveApplyReconciliation)?;
                    (
                        AuthenticatedLifecycleCompletionReadyV1::Apply(attestation),
                        Some(LifecycleCompletionCapacityProbeV1::Apply {
                            ordinal: *ordinal,
                            key,
                            executor_available,
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
                        AuthenticatedLifecycleCompletionReadyV1::Sign(attestation),
                        Some(LifecycleCompletionCapacityProbeV1::Sign {
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
                            AuthenticatedLifecycleCompletionReadyV1::CertifiedBody(attestation),
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
                            AuthenticatedLifecycleCompletionReadyV1::Fetch(attestation),
                            Some(LifecycleCompletionCapacityProbeV1::Fetch {
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
                        AuthenticatedLifecycleCompletionReadyV1::CertifiedBody(attestation),
                        None,
                    )
                }
                LifecycleWorkClass::Broadcast => {
                    let ready = match self
                        .attest_schedulable_completion_broadcast_carrier(*ordinal, Some(fence))
                        .map_err(|_| ProductionCompletionDispatchErrorV1::InvalidCarrier)?
                    {
                        SchedulableCompletionBroadcastCarrierV1::RetainedDirectOutput(
                            attestation,
                        ) => AuthenticatedLifecycleCompletionReadyV1::RetainedDirectBroadcast(
                            attestation,
                        ),
                        SchedulableCompletionBroadcastCarrierV1::RetainedRecoveredOutput(
                            attestation,
                        ) => AuthenticatedLifecycleCompletionReadyV1::RetainedRecoveredBroadcast(
                            attestation,
                        ),
                        SchedulableCompletionBroadcastCarrierV1::RecoveredRefanout => {
                            return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
                        }
                    };
                    (ready, None)
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
        let mut census = (!probes.is_empty())
            .then(|| services.capture_lifecycle_completion_capacity_census(probes))
            .transpose()
            .map_err(ProductionCompletionDispatchErrorV1::Service)?;
        let ordinary_body = authenticated
            .iter()
            .filter_map(|(ordinal, ready)| {
                matches!(
                    ready,
                    AuthenticatedLifecycleCompletionReadyV1::CertifiedBody(_)
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
                AuthenticatedLifecycleCompletionReadyV1::CertifiedBody(_)
            ) || matches!(
                &ready,
                AuthenticatedLifecycleCompletionReadyV1::Validate(attestation)
                    if !attestation.requires_io_dispatch()
            );
            let retained_direct_output = matches!(
                &ready,
                AuthenticatedLifecycleCompletionReadyV1::RetainedDirectBroadcast(_)
                    | AuthenticatedLifecycleCompletionReadyV1::RetainedRecoveredBroadcast(_)
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
                AuthenticatedLifecycleCompletionReadyV1::Validate(attestation) => {
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
                AuthenticatedLifecycleCompletionReadyV1::Apply(attestation) => {
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
                AuthenticatedLifecycleCompletionReadyV1::Sign(attestation) => {
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
                AuthenticatedLifecycleCompletionReadyV1::Fetch(attestation) => {
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
                AuthenticatedLifecycleCompletionReadyV1::CertifiedBody(attestation) => {
                    authenticated_certified_body_pipeline_ready_row(
                        &factory,
                        record,
                        attestation,
                        live_debts,
                    )
                }
                AuthenticatedLifecycleCompletionReadyV1::RetainedDirectBroadcast(attestation) => {
                    authenticated_schedulable_retained_direct_broadcast_row(
                        &factory,
                        record,
                        attestation,
                        live_debts,
                    )
                }
                AuthenticatedLifecycleCompletionReadyV1::RetainedRecoveredBroadcast(
                    attestation,
                ) => authenticated_ready_recovered_lifecycle_broadcast_row(
                    &factory,
                    record,
                    attestation,
                    live_debts,
                ),
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
        let plan = match required_ordinal {
            Some(ordinal) => self
                .coordinator
                .plan_turn_requiring_ordinal(inputs, ordinal),
            None => self.coordinator.plan_turn(inputs),
        };
        let lease = match plan {
            super::TurnPlan::Execute(lease) => lease,
            super::TurnPlan::Waiting(_) | super::TurnPlan::Idle => {
                if let Some(census) = census {
                    census.complete_without_selection();
                }
                return Ok(ProductionCompletionDispatchV1::CapacityUnavailable {
                    protected_live_apply_ordinal,
                });
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
                    match census.take() {
                        Some(census) => census.complete_without_selection(),
                        None => {}
                    }
                    drop(census);
                    return self.publish_ready_validate_outcome(services, executor, lease);
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
                    .prepare_lifecycle_decision_apply_dispatch(&self.coordinator, &lease)
                    .map_err(|_| ProductionCompletionDispatchErrorV1::DispatchProjection)?;
                if !reservation.preflight(&prepared) {
                    return Err(ProductionCompletionDispatchErrorV1::ReservedOwnerMismatch);
                }
                let executor_dispatch = executor
                    .prepare_lifecycle_decision_apply_executor_dispatch(&prepared)
                    .map_err(ProductionCompletionDispatchErrorV1::LiveApplyReconciliation)?;
                reservation.commit(prepared, executor_dispatch);
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

    /// Resolve one exact same-address Ready Validate successor ahead of
    /// physical completion classification.
    ///
    /// The retained publication token must still name the global Ready minimum
    /// and its complete registry coordinate. This path never observes or
    /// acknowledges the worker completion FIFO. A live Apply successor also
    /// installs its full executor retransmit owner before this method returns.
    pub(super) fn dispatch_ready_validate_successor(
        &mut self,
        services: &mut ProductionV2Services,
        executor: &mut V2EffectExecutor<SerializedV2Runtime>,
        successor: super::ReadyValidateSuccessorV1,
        runner_debt: u64,
    ) -> Result<ReadyValidateSuccessorDispatchV1, ProductionCompletionDispatchErrorV1> {
        let ordinal = successor.lifecycle_ordinal();
        let Some(record) = self.coordinator.records.get(&ordinal) else {
            return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
        };
        match successor.reducer_fence_wait() {
            Some(wait)
                if record.state == LifecycleState::Waiting(wait)
                    && !self.coordinator.ready_index.contains(&ordinal)
                    && wait.source()
                        == super::projection::reducer_fence_wait_source(
                            self.coordinator.active_context,
                        ) => {}
            None if record.state == LifecycleState::Ready
                && self.coordinator.ready_index.contains(&ordinal) => {}
            Some(_) | None => {
                return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
            }
        }
        let attestation = self
            .coordinator
            .attest_ready_validate_demand(&self.registry, ordinal)
            .map_err(|_| ProductionCompletionDispatchErrorV1::InvalidCarrier)?;
        if !successor.exactly_matches_ready_attestation(attestation) {
            return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
        }
        if let Some(wait) = successor.reducer_fence_wait() {
            let fence = executor.lifecycle_reducer_fence_observation();
            if fence.source() != wait.source() {
                return Err(ProductionCompletionDispatchErrorV1::InvalidCarrier);
            }
            if fence.generation() <= wait.observed_generation() {
                return Ok(ReadyValidateSuccessorDispatchV1::ReducerFencePending {
                    successor,
                    wait,
                });
            }
        }
        let selected = self.dispatch_completion_requiring_ready_ordinal(
            services,
            executor,
            runner_debt,
            ordinal,
        )?;
        if matches!(
            selected,
            ProductionCompletionDispatchV1::CapacityUnavailable { .. }
        ) {
            let (dispatch_key, round, subject, apply_is_authorized) = successor
                .preliminary_retransmit_identity(attestation)
                .ok_or(ProductionCompletionDispatchErrorV1::InvalidCarrier)?;
            executor
                .arm_live_lifecycle_validate_successor(
                    dispatch_key,
                    round,
                    subject,
                    apply_is_authorized,
                )
                .map_err(ProductionCompletionDispatchErrorV1::LiveApplyReconciliation)?;
            return Ok(ReadyValidateSuccessorDispatchV1::CapacityUnavailable(
                successor,
            ));
        }
        if let ProductionCompletionDispatchV1::ReducerFenceWait {
            ordinal: selected_ordinal,
            wait,
        } = &selected
        {
            if *selected_ordinal != ordinal {
                return Err(ProductionCompletionDispatchErrorV1::UnexpectedPlan);
            }
            let (dispatch_key, round, subject, apply_is_authorized) = successor
                .preliminary_retransmit_identity(attestation)
                .ok_or(ProductionCompletionDispatchErrorV1::InvalidCarrier)?;
            executor
                .arm_live_lifecycle_validate_successor(
                    dispatch_key,
                    round,
                    subject,
                    apply_is_authorized,
                )
                .map_err(ProductionCompletionDispatchErrorV1::LiveApplyReconciliation)?;
            let successor = successor
                .retain_on_reducer_fence(*wait)
                .ok_or(ProductionCompletionDispatchErrorV1::InvalidCarrier)?;
            return Ok(ReadyValidateSuccessorDispatchV1::ReducerFencePending {
                successor,
                wait: *wait,
            });
        }
        let exact = match &selected {
            ProductionCompletionDispatchV1::BodyStageAdvanced { parent_ordinal, .. } => {
                !successor.requires_io_dispatch() && *parent_ordinal == ordinal
            }
            ProductionCompletionDispatchV1::ValidateQueued {
                ordinal: selected_ordinal,
            } => successor.requires_io_dispatch() && *selected_ordinal == ordinal,
            ProductionCompletionDispatchV1::ValidateNoSuccessor {
                ordinal: selected_ordinal,
            } => *selected_ordinal == ordinal,
            _ => false,
        };
        if !exact {
            return Err(ProductionCompletionDispatchErrorV1::UnexpectedPlan);
        }
        if matches!(
            selected,
            ProductionCompletionDispatchV1::ValidateQueued { .. }
        ) {
            let (dispatch_key, round, subject, apply_is_authorized) = successor
                .preliminary_retransmit_identity(attestation)
                .ok_or(ProductionCompletionDispatchErrorV1::InvalidCarrier)?;
            executor
                .arm_live_lifecycle_validate_successor(
                    dispatch_key,
                    round,
                    subject,
                    apply_is_authorized,
                )
                .map_err(ProductionCompletionDispatchErrorV1::LiveApplyReconciliation)?;
        }
        Ok(ReadyValidateSuccessorDispatchV1::Resolved(selected))
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
                .attest_schedulable_completion_broadcast_carrier(*ready_ordinal, None)
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
            matches!(
                carrier,
                SchedulableCompletionBroadcastCarrierV1::RecoveredRefanout
            )
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
                    SchedulableCompletionBroadcastCarrierV1::RetainedDirectOutput(attestation) => {
                        authenticated_schedulable_retained_direct_broadcast_row(
                            &factory,
                            ready_record,
                            attestation,
                            live_debts,
                        )
                    }
                    SchedulableCompletionBroadcastCarrierV1::RetainedRecoveredOutput(
                        attestation,
                    ) => authenticated_ready_recovered_lifecycle_broadcast_row(
                        &factory,
                        ready_record,
                        attestation,
                        live_debts,
                    ),
                    SchedulableCompletionBroadcastCarrierV1::RecoveredRefanout => {
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
                        .attest_ready_lifecycle_decision_apply(
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
    /// The selected response first creates or rejoins its exact durable Waiting
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
        if !self.coordinator.ready_index.is_empty()
            || self
                .coordinator
                .records
                .values()
                .any(|record| matches!(record.state, LifecycleState::Ready))
        {
            return Err(ProductionIngressSchedulerInputsError::CompetingReadyWork);
        }
        let admission = selector
            .prepare_selected_certified_fetch_admission(executor, &self.verified)
            .map_err(|_| {
                ProductionIngressSchedulerInputsError::CertifiedFetchAdmissionPreparation
            })?;
        match self.settle_certified_fetch_admission(admission) {
            ProductionCertifiedFetchAdmissionSettlementV1::Admitted
            | ProductionCertifiedFetchAdmissionSettlementV1::Existing => {}
            ProductionCertifiedFetchAdmissionSettlementV1::Deferred => {
                return Err(
                    ProductionIngressSchedulerInputsError::CertifiedFetchAdmissionDeferred {
                        _prepared: selector,
                    },
                );
            }
            ProductionCertifiedFetchAdmissionSettlementV1::RestartRequired => {
                return Err(
                    ProductionIngressSchedulerInputsError::CertifiedFetchAdmissionSettlement,
                );
            }
        }
        let fetch = selector
            .attest_scheduler_fetch_carrier(&self.coordinator, &mut self.registry)
            .map_err(|_| ProductionIngressSchedulerInputsError::InvalidSelectedCarrier)?;
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
include!("tests/v2_lifecycle_scheduler_completion_cases.rs");

#[cfg(test)]
mod certified_serve_scheduler_tests {
    include!("tests/v2_lifecycle_scheduler_certified_serve_cases.rs");
}
