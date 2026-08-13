//! Sealed production authentication for lifecycle planner inputs.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    CapacityClass, LifecycleCoordinator, LifecycleState, LifecycleWorkClass,
    LifecycleWorkRegistryHolder, PreparedLifecycleIngressSelector, ProductionLifecycleOwnerV1,
    schema::{AttestedReadyValidateDemand, SchedulerInputs, SchedulerReadyInputs},
    work_registry::ReadyRecoveredDecisionApplyDemand,
};
use crate::sumeragi::{
    v2_effects::{
        LifecycleModeRankSnapshot, RecoveredDecisionFetchRequestRegistrationErrorV1,
        RecoveredDecisionFetchResponseClaimErrorV1, V2EffectExecutor,
    },
    v2_runner::LifecycleRunnerRankTarget,
    v2_runtime::SerializedV2Runtime,
    v2_worker::{
        AuthenticatedLifecycleIoCapacity, LifecycleIoCapacityCaptureFailure,
        LifecycleIoCapacityWait, LifecycleIoCapacityWaitStatus, ProductionV2Services,
        RecoveredDecisionApplyCapacityCaptureErrorV1, RecoveredDecisionApplyCapacityCaptureV1,
        RecoveredDecisionFetchExactOutputCaptureV1, RecoveredLifecycleSignBroadcastOutputCaptureV1,
        RecoveredLifecycleSignCapacityCaptureErrorV1, RecoveredLifecycleSignCapacityCaptureV1,
    },
};

#[cfg(not(test))]
use crate::sumeragi::v2_runner::LifecycleCurrentRunnerTurn;
#[cfg(test)]
use crate::sumeragi::v2_runner::LifecycleRunnerRankSnapshot;

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

/// Result of one complete recovered Decision Apply claim-and-dispatch turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum ProductionRecoveredDecisionApplyDispatchV1 {
    /// The same service had no free Consensus command position; nothing was claimed.
    CapacityUnavailable,
    /// The exact Apply lease now owns one queued dedicated worker command.
    Queued {
        /// Immutable lifecycle ordinal retained by both lease and command key.
        ordinal: u128,
    },
}

/// Closed failure before or during recovered Decision Apply dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum ProductionRecoveredDecisionApplyDispatchErrorV1 {
    /// The logical owner already latched a fail-closed condition.
    CoordinatorFaulted(super::CoordinatorFault),
    /// A prior turn still owns the sole active lease.
    UnsettledLease(super::LeaseId),
    /// The live runner turn did not name this height's Completion service point.
    ForeignRunnerObservation,
    /// The launched executor, worker, or body-store instance is not this owner.
    ForeignServiceOwner,
    /// The complete Ready census is not the sole exact recovered Apply carrier.
    InvalidReadyCensus,
    /// The closed recovered Apply registry attestation failed.
    InvalidCarrier,
    /// The worker could not retain the exact queue position.
    Capacity(RecoveredDecisionApplyCapacityCaptureErrorV1),
    /// Planning did not return the sole authenticated recovered Apply lease.
    UnexpectedPlan,
    /// The claimed row could not project its exact closed worker task.
    DispatchProjection,
    /// The reserved queue key and claimed carrier projection disagreed.
    ReservedCommandMismatch,
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
    /// The borrow-bound runner cursor was not this height's Completion turn.
    ForeignRunnerObservation,
    /// The service does not own this launched height's exact body-store worker.
    ForeignServiceOwner,
    /// Coordinator records and the reverse Ready index disagree.
    InvalidReadyCensus,
    /// The selected Broadcast or its declared next-Sign link failed authentication.
    InvalidCarrier,
    /// Planning did not return the authenticated Broadcast lease.
    UnexpectedPlan,
}

/// Result of one recovered Decision Fetch request-dispatch turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum ProductionRecoveredDecisionFetchDispatchV1 {
    /// The exact-output corridor cannot own the fanout; nothing was claimed.
    CapacityUnavailable,
    /// Request/output ownership is installed and the coordinator lease remains Claimed.
    Dispatched {
        /// Immutable lifecycle ordinal shared by lease, carrier, and executor key.
        ordinal: u128,
    },
}

/// Closed failure before or during recovered Decision Fetch request dispatch.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum ProductionRecoveredDecisionFetchDispatchErrorV1 {
    /// The logical owner already latched a fail-closed condition.
    CoordinatorFaulted(super::CoordinatorFault),
    /// A prior turn still owns the sole active lease.
    UnsettledLease(super::LeaseId),
    /// The borrow-bound runner cursor was not this height's Completion turn.
    ForeignRunnerObservation,
    /// The launched executor, worker, body store, or output guard is foreign.
    ForeignServiceOwner,
    /// The complete Ready census is not the sole recovered Decision Fetch.
    InvalidReadyCensus,
    /// The closed carrier or move-only request authority failed exact validation.
    InvalidCarrier,
    /// Fixed service signing or exact-output capture failed.
    Service(String),
    /// A conflicting or full executor request owner prevented pre-claim reservation.
    Executor(RecoveredDecisionFetchRequestRegistrationErrorV1),
    /// Planning did not return the exact Fetch lease.
    UnexpectedPlan,
    /// The claimed row could not rejoin its exact closed carrier.
    DispatchProjection,
    /// The reserved executor key and claimed carrier disagreed.
    ReservedOwnerMismatch,
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
pub(crate) enum ProductionRecoveredDecisionFetchPersistenceErrorV1 {
    /// The outer current runner cursor was not this height's Ingress turn.
    ForeignRunnerObservation,
    /// The active coordinator lease/carrier/request owner was not exact.
    InvalidClaimedCarrier,
    /// The selected response or service belonged to another height owner.
    ForeignOwner,
    /// Selector consumption failed and was restored into the queue reservation.
    CommandPreparation(PreparedLifecycleIngressSelector),
    /// The dedicated queue already owns this lifecycle key.
    InFlightSelectedWork(PreparedLifecycleIngressSelector),
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
#[cfg_attr(not(test), allow(dead_code))]
#[allow(variant_size_differences, clippy::large_enum_variant)]
pub(crate) enum ProductionIngressSchedulerInputsError {
    /// The owner already latched a fail-closed condition.
    CoordinatorFaulted(super::CoordinatorFault),
    /// Another exact lease still owns coordinator execution.
    UnsettledLease(super::LeaseId),
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
    CommandPreparation(PreparedLifecycleIngressSelector),
    /// The exact selected work id is already queued, active, or completion-pending.
    InFlightSelectedWork(PreparedLifecycleIngressSelector),
    /// The consumed command no longer matched its reserved queue identity.
    InvalidReservedCommand,
    /// The sole prospective target did not produce its exact lease.
    UnexpectedPlan,
    /// Blocking the submitted lease violated coordinator invariants.
    SettlementFault(super::CoordinatorFault),
    /// The live service could not issue a capacity reservation or generation wait.
    Service {
        /// Closed reason the service rejected the capture.
        failure: LifecycleIoCapacityCaptureFailure,
        /// Complete selector with its one-shot target restored.
        prepared: PreparedLifecycleIngressSelector,
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
        let validate_attestation = match record.work_class {
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
                Some(attestation)
            }
            LifecycleWorkClass::Apply => {
                let attestation = registry
                    .attest_ready_recovered_decision_apply(coordinator, *ordinal)
                    .map_err(|_| {
                        ProductionSchedulerInputsError::InvalidRecoveredDecisionApplyCarrier {
                            ordinal: *ordinal,
                        }
                    })?;
                match attestation.demand() {
                    ReadyRecoveredDecisionApplyDemand::BoundedIo => {
                        return Err(
                            ProductionSchedulerInputsError::IoCapacityObservationRequired {
                                ordinal: *ordinal,
                            },
                        );
                    }
                }
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
                match attestation.demand() {
                    super::work_registry::ReadyRecoveredLifecycleSignDemandV1::BoundedIo => {
                        return Err(
                            ProductionSchedulerInputsError::IoCapacityObservationRequired {
                                ordinal: *ordinal,
                            },
                        );
                    }
                }
            }
            LifecycleWorkClass::Fetch => {
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
            _ => {
                return Err(ProductionSchedulerInputsError::UnsupportedReadyCarrier {
                    ordinal: *ordinal,
                    work_class: record.work_class,
                });
            }
        };
        let row = authenticated_ready_row(
            &factory,
            record,
            validate_attestation,
            None,
            None,
            None,
            AuthenticatedLiveRankDebts::DirectRegistryCompletion.components(),
        )
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

    /// Reserve, claim, and queue the sole Ready recovered Decision Apply.
    ///
    /// This fixed transaction admits no selector or raw command fields. The
    /// service locks the exact Consensus FIFO position first; only then does
    /// the coordinator claim the registry-attested Apply and project its
    /// move-only task. Queue publication consumes that borrow-bound projection
    /// and arms the carrier's in-flight key in one infallible tail.
    #[cfg(not(test))]
    pub(in crate::sumeragi) fn dispatch_recovered_decision_apply(
        &mut self,
        services: &ProductionV2Services,
        executor: &V2EffectExecutor<SerializedV2Runtime>,
        runner: LifecycleCurrentRunnerTurn<'_>,
    ) -> Result<
        ProductionRecoveredDecisionApplyDispatchV1,
        ProductionRecoveredDecisionApplyDispatchErrorV1,
    > {
        let context = self.verified.context();
        if runner.target() != LifecycleRunnerRankTarget::Completion
            || runner.height() != context.height
            || runner.context_id() != context.id()
        {
            return Err(ProductionRecoveredDecisionApplyDispatchErrorV1::ForeignRunnerObservation);
        }
        self.dispatch_recovered_decision_apply_with_runner_debt(services, executor, runner.debt())
    }

    /// Exercise the exact recovered Apply dispatch with a fixture-owned runner snapshot.
    #[cfg(test)]
    pub(in crate::sumeragi) fn dispatch_recovered_decision_apply(
        &mut self,
        services: &ProductionV2Services,
        executor: &V2EffectExecutor<SerializedV2Runtime>,
        runner: LifecycleRunnerRankSnapshot,
    ) -> Result<
        ProductionRecoveredDecisionApplyDispatchV1,
        ProductionRecoveredDecisionApplyDispatchErrorV1,
    > {
        let context = self.verified.context();
        if runner.target() != LifecycleRunnerRankTarget::Completion
            || runner.height() != context.height
            || runner.context_id() != context.id()
        {
            return Err(ProductionRecoveredDecisionApplyDispatchErrorV1::ForeignRunnerObservation);
        }
        self.dispatch_recovered_decision_apply_with_runner_debt(services, executor, runner.debt())
    }

    fn dispatch_recovered_decision_apply_with_runner_debt(
        &mut self,
        services: &ProductionV2Services,
        executor: &V2EffectExecutor<SerializedV2Runtime>,
        runner_debt: u64,
    ) -> Result<
        ProductionRecoveredDecisionApplyDispatchV1,
        ProductionRecoveredDecisionApplyDispatchErrorV1,
    > {
        if let Some(fault) = self.coordinator.fault {
            return Err(ProductionRecoveredDecisionApplyDispatchErrorV1::CoordinatorFaulted(fault));
        }
        if let Some(lease) = self.coordinator.active_lease.as_ref() {
            return Err(ProductionRecoveredDecisionApplyDispatchErrorV1::UnsettledLease(lease.id));
        }
        let Some(body_store_identity) = self.body_store_identity.as_ref() else {
            return Err(ProductionRecoveredDecisionApplyDispatchErrorV1::ForeignServiceOwner);
        };
        if self.body_store.is_some()
            || !services.matches_lifecycle_body_store(body_store_identity)
            || !services.matches_lifecycle_executor_output_guard(executor)
        {
            return Err(ProductionRecoveredDecisionApplyDispatchErrorV1::ForeignServiceOwner);
        }
        let mut ready = self.coordinator.ready_index.iter().copied();
        let Some(ordinal) = ready.next() else {
            return Err(ProductionRecoveredDecisionApplyDispatchErrorV1::InvalidReadyCensus);
        };
        if ready.next().is_some() {
            return Err(ProductionRecoveredDecisionApplyDispatchErrorV1::InvalidReadyCensus);
        }
        if self
            .coordinator
            .records
            .values()
            .filter(|record| matches!(record.state, LifecycleState::Ready))
            .map(|record| record.ordinal)
            .collect::<BTreeSet<_>>()
            != BTreeSet::from([ordinal])
        {
            return Err(ProductionRecoveredDecisionApplyDispatchErrorV1::InvalidReadyCensus);
        }
        let attestation = self
            .registry
            .attest_ready_recovered_decision_apply(&self.coordinator, ordinal)
            .map_err(|_| ProductionRecoveredDecisionApplyDispatchErrorV1::InvalidCarrier)?;
        if attestation.demand() != ReadyRecoveredDecisionApplyDemand::BoundedIo {
            return Err(ProductionRecoveredDecisionApplyDispatchErrorV1::InvalidCarrier);
        }
        let dispatch_key = attestation.dispatch_key();
        let mode = executor.lifecycle_mode_rank_snapshot();
        let context = self.verified.context();
        if mode.height() != context.height
            || mode.context_id() != context.id()
            || !dispatch_key.matches_height_context(context)
        {
            return Err(ProductionRecoveredDecisionApplyDispatchErrorV1::ForeignServiceOwner);
        }
        let capacity = services
            .capture_recovered_decision_apply_capacity(dispatch_key)
            .map_err(ProductionRecoveredDecisionApplyDispatchErrorV1::Capacity)?;
        let RecoveredDecisionApplyCapacityCaptureV1::Reserved(reservation) = capacity else {
            return Ok(ProductionRecoveredDecisionApplyDispatchV1::CapacityUnavailable);
        };
        let factory = AuthenticatedSchedulerInputsFactory::new();
        let record = self
            .coordinator
            .records
            .get(&ordinal)
            .ok_or(ProductionRecoveredDecisionApplyDispatchErrorV1::InvalidReadyCensus)?;
        let live_debts = [
            mode.debt(),
            reservation.authenticated_predecessor_debt(&factory),
            0,
            0,
            0,
            runner_debt,
        ];
        let row = authenticated_ready_row(
            &factory,
            record,
            None,
            Some(attestation),
            None,
            None,
            live_debts,
        )
        .ok_or(ProductionRecoveredDecisionApplyDispatchErrorV1::InvalidCarrier)?;
        let inputs = authenticated_scheduler_inputs(
            factory,
            BTreeMap::new(),
            BTreeMap::from([(ordinal, row)]),
        );
        let lease = match self.coordinator.plan_turn(inputs) {
            super::TurnPlan::Execute(lease)
                if lease.ordinal() == ordinal
                    && lease.work_class() == LifecycleWorkClass::Apply =>
            {
                lease
            }
            super::TurnPlan::Execute(_)
            | super::TurnPlan::Waiting(_)
            | super::TurnPlan::Idle
            | super::TurnPlan::FailClosed(_) => {
                return Err(ProductionRecoveredDecisionApplyDispatchErrorV1::UnexpectedPlan);
            }
        };
        let prepared = self
            .registry
            .prepare_recovered_decision_apply_dispatch(&self.coordinator, &lease)
            .map_err(|_| ProductionRecoveredDecisionApplyDispatchErrorV1::DispatchProjection)?;
        if !reservation.preflight(&prepared) {
            return Err(ProductionRecoveredDecisionApplyDispatchErrorV1::ReservedCommandMismatch);
        }
        reservation.commit(prepared);
        Ok(ProductionRecoveredDecisionApplyDispatchV1::Queued { ordinal })
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

    fn dispatch_recovered_lifecycle_sign_with_runner_debt(
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
                let rolled_back = lease.output_reservation().map_or_else(
                    || self.coordinator.rollback_unpublished_turn(&lease),
                    |reservation| {
                        self.coordinator
                            .rollback_unpublished_reserved_turn(&lease, reservation.class())
                    },
                );
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

    /// Refanout one durable recovered signed Broadcast at the live Completion cursor.
    #[cfg(not(test))]
    pub(in crate::sumeragi) fn refanout_recovered_lifecycle_signed_broadcast(
        &mut self,
        services: &ProductionV2Services,
        runner: LifecycleCurrentRunnerTurn<'_>,
    ) -> Result<
        ProductionRecoveredLifecycleSignedBroadcastRefanoutV1,
        ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1,
    > {
        let context = self.verified.context();
        if runner.target() != LifecycleRunnerRankTarget::Completion
            || runner.height() != context.height
            || runner.context_id() != context.id()
        {
            return Err(
                ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::ForeignRunnerObservation,
            );
        }
        self.refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(services, runner.debt())
    }

    /// Exercise durable recovered Broadcast refanout with a fixture-owned cursor.
    #[cfg(test)]
    pub(in crate::sumeragi) fn refanout_recovered_lifecycle_signed_broadcast(
        &mut self,
        services: &ProductionV2Services,
        runner: LifecycleRunnerRankSnapshot,
    ) -> Result<
        ProductionRecoveredLifecycleSignedBroadcastRefanoutV1,
        ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1,
    > {
        let context = self.verified.context();
        if runner.target() != LifecycleRunnerRankTarget::Completion
            || runner.height() != context.height
            || runner.context_id() != context.id()
        {
            return Err(
                ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::ForeignRunnerObservation,
            );
        }
        self.refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(services, runner.debt())
    }

    fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(
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
        let Some(ordinal) = exact_ready.iter().copied().find(|ordinal| {
            self.coordinator
                .records
                .get(ordinal)
                .is_some_and(|record| record.work_class == LifecycleWorkClass::Broadcast)
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
                LifecycleWorkClass::Broadcast => {
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
                LifecycleWorkClass::Store
                | LifecycleWorkClass::EnterView
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

    /// Sign, reserve, claim, and publish the sole recovered Decision Fetch request.
    ///
    /// Exact-output ownership and the executor's vacant dedicated request slot
    /// are both retained before planning. Success deliberately leaves the
    /// coordinator lease Claimed: response persistence is a later Ingress turn,
    /// and request retirement/Fetch-to-Store settlement remain restart-closed
    /// work outside this bounded tranche.
    #[cfg(not(test))]
    pub(in crate::sumeragi) fn dispatch_recovered_decision_fetch(
        &mut self,
        services: &ProductionV2Services,
        executor: &mut V2EffectExecutor<SerializedV2Runtime>,
        runner: LifecycleCurrentRunnerTurn<'_>,
    ) -> Result<
        ProductionRecoveredDecisionFetchDispatchV1,
        ProductionRecoveredDecisionFetchDispatchErrorV1,
    > {
        let context = self.verified.context();
        if runner.target() != LifecycleRunnerRankTarget::Completion
            || runner.height() != context.height
            || runner.context_id() != context.id()
        {
            return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::ForeignRunnerObservation);
        }
        self.dispatch_recovered_decision_fetch_with_runner_debt(services, executor, runner.debt())
    }

    /// Exercise recovered Decision Fetch dispatch with a fixture-owned runner snapshot.
    #[cfg(test)]
    pub(in crate::sumeragi) fn dispatch_recovered_decision_fetch(
        &mut self,
        services: &ProductionV2Services,
        executor: &mut V2EffectExecutor<SerializedV2Runtime>,
        runner: LifecycleRunnerRankSnapshot,
    ) -> Result<
        ProductionRecoveredDecisionFetchDispatchV1,
        ProductionRecoveredDecisionFetchDispatchErrorV1,
    > {
        let context = self.verified.context();
        if runner.target() != LifecycleRunnerRankTarget::Completion
            || runner.height() != context.height
            || runner.context_id() != context.id()
        {
            return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::ForeignRunnerObservation);
        }
        self.dispatch_recovered_decision_fetch_with_runner_debt(services, executor, runner.debt())
    }

    fn dispatch_recovered_decision_fetch_with_runner_debt(
        &mut self,
        services: &ProductionV2Services,
        executor: &mut V2EffectExecutor<SerializedV2Runtime>,
        runner_debt: u64,
    ) -> Result<
        ProductionRecoveredDecisionFetchDispatchV1,
        ProductionRecoveredDecisionFetchDispatchErrorV1,
    > {
        if let Some(fault) = self.coordinator.fault {
            return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::CoordinatorFaulted(fault));
        }
        if let Some(lease) = self.coordinator.active_lease.as_ref() {
            return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::UnsettledLease(lease.id));
        }
        let Some(body_store_identity) = self.body_store_identity.as_ref() else {
            return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::ForeignServiceOwner);
        };
        if self.body_store.is_some()
            || !services.matches_lifecycle_body_store(body_store_identity)
            || !services.matches_lifecycle_executor_output_guard(executor)
        {
            return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::ForeignServiceOwner);
        }
        let mut ready = self.coordinator.ready_index.iter().copied();
        let Some(ordinal) = ready.next() else {
            return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::InvalidReadyCensus);
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
            return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::InvalidReadyCensus);
        }
        let mut attestation = self
            .registry
            .registry_mut()
            .attest_ready_recovered_decision_fetch(&self.coordinator, ordinal)
            .map_err(|_| ProductionRecoveredDecisionFetchDispatchErrorV1::InvalidCarrier)?;
        if attestation.demand()
            != super::work_registry::ReadyRecoveredDecisionFetchDemandV1::ExactOutputAndExecutor
        {
            return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::InvalidCarrier);
        }
        let dispatch_key = attestation.dispatch_key();
        let mode = executor.lifecycle_mode_rank_snapshot();
        let context = self.verified.context();
        if mode.height() != context.height
            || mode.context_id() != context.id()
            || !dispatch_key.matches_height_context(context)
        {
            return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::ForeignServiceOwner);
        }
        let authority = attestation.take_request_authority();
        let owner = services
            .authenticate_recovered_decision_fetch_request(authority)
            .map_err(ProductionRecoveredDecisionFetchDispatchErrorV1::Service)?;
        if owner.dispatch_key() != dispatch_key {
            return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::InvalidCarrier);
        }
        let output = services
            .capture_recovered_decision_fetch_exact_output(&owner)
            .map_err(ProductionRecoveredDecisionFetchDispatchErrorV1::Service)?;
        let RecoveredDecisionFetchExactOutputCaptureV1::Reserved(output) = output else {
            return Ok(ProductionRecoveredDecisionFetchDispatchV1::CapacityUnavailable);
        };
        let registration =
            match executor.prepare_recovered_decision_fetch_request_registration(owner) {
                Ok(registration) => registration,
                Err(error) => {
                    output.abort_before_claim();
                    return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::Executor(
                        error,
                    ));
                }
            };
        if registration.dispatch_key() != dispatch_key {
            drop(registration);
            output.abort_before_claim();
            return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::ReservedOwnerMismatch);
        }
        let factory = AuthenticatedSchedulerInputsFactory::new();
        let Some(record) = self.coordinator.records.get(&ordinal) else {
            drop(registration);
            output.abort_before_claim();
            return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::InvalidReadyCensus);
        };
        let live_debts = [mode.debt(), output.predecessor_debt(), 0, 0, 0, runner_debt];
        let Some(row) = authenticated_ready_row(
            &factory,
            record,
            None,
            None,
            None,
            Some(attestation),
            live_debts,
        ) else {
            drop(registration);
            output.abort_before_claim();
            return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::InvalidCarrier);
        };
        let inputs = authenticated_scheduler_inputs(
            factory,
            BTreeMap::new(),
            BTreeMap::from([(ordinal, row)]),
        );
        let lease = match self.coordinator.plan_turn(inputs) {
            super::TurnPlan::Execute(lease)
                if lease.ordinal() == ordinal
                    && lease.work_class() == LifecycleWorkClass::Fetch =>
            {
                lease
            }
            super::TurnPlan::Execute(lease) => {
                assert!(
                    self.coordinator.rollback_unpublished_turn(&lease),
                    "unexpected recovered Fetch plan must restore its unpublished claim"
                );
                drop(registration);
                output.abort_before_claim();
                return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::UnexpectedPlan);
            }
            super::TurnPlan::Waiting(_)
            | super::TurnPlan::Idle
            | super::TurnPlan::FailClosed(_) => {
                drop(registration);
                output.abort_before_claim();
                return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::UnexpectedPlan);
            }
        };
        let prepared = match self
            .registry
            .registry_mut()
            .prepare_recovered_decision_fetch_dispatch(&self.coordinator, &lease, dispatch_key)
        {
            Ok(prepared) => prepared,
            Err(_) => {
                assert!(
                    self.coordinator.rollback_unpublished_turn(&lease),
                    "failed recovered Fetch projection must restore its unpublished claim"
                );
                drop(registration);
                output.abort_before_claim();
                return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::DispatchProjection);
            }
        };
        if prepared.dispatch_key() != registration.dispatch_key() {
            drop(prepared);
            assert!(
                self.coordinator.rollback_unpublished_turn(&lease),
                "mismatched recovered Fetch owner must restore its unpublished claim"
            );
            drop(registration);
            output.abort_before_claim();
            return Err(ProductionRecoveredDecisionFetchDispatchErrorV1::ReservedOwnerMismatch);
        }
        let installed = registration.commit(prepared);
        assert_eq!(installed, dispatch_key);
        output.commit();
        Ok(ProductionRecoveredDecisionFetchDispatchV1::Dispatched { ordinal })
    }

    /// Persist one selected recovered Decision Fetch response while retaining
    /// its fair-ingress occurrence, request owner, carrier, and claimed lease.
    #[cfg(not(test))]
    #[allow(clippy::result_large_err)]
    pub(crate) fn persist_recovered_decision_fetch_response(
        &mut self,
        services: &ProductionV2Services,
        executor: &mut V2EffectExecutor<SerializedV2Runtime>,
        selector: PreparedLifecycleIngressSelector,
        runner: LifecycleCurrentRunnerTurn<'_>,
    ) -> Result<
        ProductionRecoveredDecisionFetchPersistenceV1,
        ProductionRecoveredDecisionFetchPersistenceErrorV1,
    > {
        let context = selector.context();
        if runner.target() != LifecycleRunnerRankTarget::Ingress
            || runner.height() != context.height()
            || runner.context_id().0.as_ref() != context.id().as_bytes()
            || context != self.coordinator.active_context
        {
            return Err(
                ProductionRecoveredDecisionFetchPersistenceErrorV1::ForeignRunnerObservation,
            );
        }
        self.persist_recovered_decision_fetch_response_after_runner(services, executor, selector)
    }

    /// Exercise Phase A with a fixture-owned current Ingress snapshot.
    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    pub(crate) fn persist_recovered_decision_fetch_response(
        &mut self,
        services: &ProductionV2Services,
        executor: &mut V2EffectExecutor<SerializedV2Runtime>,
        selector: PreparedLifecycleIngressSelector,
        runner: LifecycleRunnerRankSnapshot,
    ) -> Result<
        ProductionRecoveredDecisionFetchPersistenceV1,
        ProductionRecoveredDecisionFetchPersistenceErrorV1,
    > {
        let context = selector.context();
        if runner.target() != LifecycleRunnerRankTarget::Ingress
            || runner.height() != context.height()
            || runner.context_id().0.as_ref() != context.id().as_bytes()
            || context != self.coordinator.active_context
        {
            return Err(
                ProductionRecoveredDecisionFetchPersistenceErrorV1::ForeignRunnerObservation,
            );
        }
        self.persist_recovered_decision_fetch_response_after_runner(services, executor, selector)
    }

    #[allow(clippy::result_large_err)]
    fn persist_recovered_decision_fetch_response_after_runner(
        &mut self,
        services: &ProductionV2Services,
        executor: &mut V2EffectExecutor<SerializedV2Runtime>,
        selector: PreparedLifecycleIngressSelector,
    ) -> Result<
        ProductionRecoveredDecisionFetchPersistenceV1,
        ProductionRecoveredDecisionFetchPersistenceErrorV1,
    > {
        let context = selector.context();
        let Some(lease) = self.coordinator.active_lease.clone() else {
            return Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::InvalidClaimedCarrier);
        };
        if self.coordinator.fault.is_some()
            || lease.work_class() != LifecycleWorkClass::Fetch
            || lease.ordinal() == 0
        {
            return Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::InvalidClaimedCarrier);
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
        drop(factory);
        if !reservation.preflight_recovered_decision_fetch_target_absent() {
            let prepared = reservation.abort_into_prepared(prepared);
            return Err(
                ProductionRecoveredDecisionFetchPersistenceErrorV1::InFlightSelectedWork(prepared),
            );
        }
        let task = match executor.prepare_recovered_decision_fetch_body_persistence(prepared) {
            Ok(task) => task,
            Err(error) => {
                let prepared = reservation.abort_into_prepared(error.into_prepared());
                return Err(
                    ProductionRecoveredDecisionFetchPersistenceErrorV1::CommandPreparation(
                        prepared,
                    ),
                );
            }
        };
        if !task
            .dispatch_key()
            .matches_height_context(self.verified.context())
            || task.dispatch_key().lifecycle_ordinal() != lease.ordinal()
            || !self
                .registry
                .registry_mut()
                .matches_claimed_dispatched_recovered_decision_fetch(
                    &self.coordinator,
                    &lease,
                    task.dispatch_key(),
                )
        {
            return Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::InvalidClaimedCarrier);
        }
        if !reservation.preflight_recovered_decision_fetch_body_persistence(&task) {
            return Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::InvalidReservedCommand);
        }
        let ordinal = lease.ordinal();
        let claim = executor
            .prepare_recovered_decision_fetch_response_claim(&task)
            .map_err(ProductionRecoveredDecisionFetchPersistenceErrorV1::Claim)?;
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
    #[cfg(not(test))]
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
    pub(crate) fn plan_ingress_turn(
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
            return Err(ProductionIngressSchedulerInputsError::CoordinatorFaulted(
                fault,
            ));
        }
        if let Some(lease) = self.coordinator.active_lease.as_ref() {
            return Err(ProductionIngressSchedulerInputsError::UnsettledLease(
                lease.id,
            ));
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
                ProductionIngressSchedulerInputsError::Service { failure, prepared }
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
            return Err(ProductionIngressSchedulerInputsError::InFlightSelectedWork(
                prepared,
            ));
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
        let row = authenticated_ready_row(&factory, record, None, None, None, None, live_debts)
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
                return Err(ProductionIngressSchedulerInputsError::CommandPreparation(
                    prepared,
                ));
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
            return Err(ProductionIngressSchedulerInputsError::SettlementFault(
                fault,
            ));
        }
        if self.coordinator.active_lease.is_some()
            || self
                .coordinator
                .records
                .get(&ordinal)
                .is_none_or(|record| record.state != LifecycleState::Waiting(post_submit_wait))
        {
            self.coordinator.fault = Some(super::CoordinatorFault::InvalidSchedulerInputs);
            return Err(ProductionIngressSchedulerInputsError::SettlementFault(
                super::CoordinatorFault::InvalidSchedulerInputs,
            ));
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
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::Arc,
    };

    use super::super::schema::SchedulerEpisode;
    use super::{
        AuthenticatedSchedulerInputsFactory,
        ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1,
        ProductionRecoveredLifecycleSignedBroadcastRefanoutV1, ProductionV2Services,
        authenticated_ready_row,
    };
    use crate::sumeragi::v2_lifecycle_coordinator::{
        CapacityClass, CausalRoot, LifecycleDigest, LifecycleKey, LifecyclePhase, LifecycleRecord,
        LifecycleRound, LifecycleStage, LifecycleStageKind, LifecycleState, LifecycleWorkClass,
        OwnerId, PhysicalSlotId, PredecessorScope, ProductionLifecycleOwnerV1,
        SchedulerEpisodeUniverse, work_registry::ReadyRecoveredLifecycleSignAttestationV1,
    };
    use iroha_crypto::{Hash, KeyPair};
    use iroha_data_model::{block::consensus_v2 as wire, peer::PeerId};

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
                payload_store,
                serve_payloads,
                body_store: Some(body_store),
                body_store_identity: None,
                kura_binding: None,
                apply_service: None,
                adapter_startup: Some(
                    crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1::fixture_for_test(),
                ),
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

    /// Remove the deliberately unrelated fixture Sign from the bounded owner.
    fn retire_unrelated_sign_for_finalization_test(&mut self, ordinal: u128) -> bool {
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
        if self
            .registry
            .registry_for_test_mut()
            .entries
            .remove(&address)
            .is_none()
        {
            return false;
        }
        self.coordinator
            .finish_terminal(ordinal, super::TerminalOutcome::Cancelled)
            .is_ok()
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
            AdmissionDecision, CapacityClass, WaitToken,
            concrete_admission::AdapterEffectAdmissionTransaction, schema::CapacityGeometry,
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
        let transaction =
            coordinator.admit_concrete_adapter_effect(&mut registry, &verified, effect, pending);
        let AdapterEffectAdmissionTransaction::Admitted(AdmissionDecision::Admitted {
            ordinal,
            ..
        }) = transaction
        else {
            panic!("the exact selected Fetch must enter the concrete registry")
        };
        let record = coordinator
            .records
            .get_mut(&ordinal)
            .expect("admitted Fetch owns its logical record");
        assert_eq!(record.key, expected_key);
        assert_eq!(record.owner.causal_root(), expected_root);
        assert_eq!(record.work_class, LifecycleWorkClass::Fetch);
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
                payload_store,
                serve_payloads,
                body_store: Some(body_store),
                body_store_identity: None,
                kura_binding: None,
                apply_service: None,
                adapter_startup: Some(
                    crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1::fixture_for_test(),
                ),
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
}
