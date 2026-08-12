//! Sealed production authentication for lifecycle planner inputs.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    LifecycleCoordinator, LifecycleState, LifecycleWorkClass, LifecycleWorkRegistryHolder,
    PreparedLifecycleIngressSelector, ProductionLifecycleOwnerV1,
    schema::{AttestedReadyValidateDemand, SchedulerInputs, SchedulerReadyInputs},
};
use crate::sumeragi::{
    v2_effects::LifecycleModeRankSnapshot,
    v2_effects::V2EffectExecutor,
    v2_runner::LifecycleRunnerRankTarget,
    v2_runtime::SerializedV2Runtime,
    v2_worker::{
        AuthenticatedLifecycleIoCapacity, LifecycleIoCapacityCaptureFailure,
        LifecycleIoCapacityWait, LifecycleIoCapacityWaitStatus, ProductionV2Services,
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
    live_debts: [u64; 6],
) -> Option<SchedulerReadyInputs> {
    SchedulerReadyInputs::from_authenticated(factory, record, validate_attestation, live_debts)
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
    /// Execute-body Validate still requires a service-owned capacity cut.
    IoCapacityObservationRequired {
        /// Exact logical ordinal awaiting the missing service observation.
        ordinal: u128,
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
/// The current sound subset consists of closed Validate completion carriers.
/// Execute-body Validate is rejected because its I/O capacity and outer-runner
/// reach still need a service-minted joint observation. Other work classes
/// remain closed until their concrete carrier classifier exists.
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
        if record.work_class != LifecycleWorkClass::Validate {
            return Err(ProductionSchedulerInputsError::UnsupportedReadyCarrier {
                ordinal: *ordinal,
                work_class: record.work_class,
            });
        }
        let attestation = coordinator
            .attest_ready_validate_demand(registry, *ordinal)
            .map_err(|_| ProductionSchedulerInputsError::InvalidValidateCarrier {
                ordinal: *ordinal,
            })?;
        if attestation.requires_io_dispatch() {
            return Err(
                ProductionSchedulerInputsError::IoCapacityObservationRequired { ordinal: *ordinal },
            );
        }
        let row = authenticated_ready_row(
            &factory,
            record,
            Some(attestation),
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
        let row = authenticated_ready_row(&factory, record, None, live_debts)
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
    /// Build one storage-owning production owner around the exact selected
    /// Fetch carrier used by the cross-module planner transaction regression.
    pub(in crate::sumeragi) fn waiting_fetch_for_ingress_test(
        verified: crate::sumeragi::v2::VerifiedHeightContext,
        prepared: &PreparedLifecycleIngressSelector,
        effect: crate::sumeragi::v2::AdapterEffect,
        pending: crate::sumeragi::v2_runtime::PendingRuntimeEffectBinding,
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
        let (payload_store, _recovery) =
            crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
                &root.join("serve"),
                verified.context(),
            )
            .expect("open exact owner Serve payload store");
        (
            Self {
                verified,
                coordinator,
                registry,
                payload_store,
                body_store: Some(body_store),
                body_store_identity: None,
                adapter_startup:
                    crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1::fixture_for_test(),
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
