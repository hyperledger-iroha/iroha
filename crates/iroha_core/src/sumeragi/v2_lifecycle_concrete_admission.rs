//! Atomic admission boundary between digest-only lifecycle state and concrete work.

use super::{
    AdmissionDecision, AdmissionRequest, CoordinatorFault, LifecycleCoordinator, LifecycleDigest,
    LifecyclePhase, LifecycleStageKind, LifecycleState, LifecycleWorkClass, PredecessorScope,
    TurnLease, TurnOutcome, WaitSource, WaitToken,
    body_pipeline_transition::durable_validate_payload_is_exact,
    projection::{self, AdapterEffectAdmissionError},
    schema::AttestedReadyValidateDemand,
    work_registry::{
        AuthenticatedRecoveredWalValidateLifecycleRepair, ConcreteLifecycleWork,
        ConcreteLifecycleWorkRegistry, ConcreteWorkAddress, DurableValidateCompletionAuthority,
        DurableValidateCompletionPublication, DurableValidateCompletionPublicationError,
        DurableValidateDispatch, DurableValidateExecutionError, ExecutedDurableValidateDispatch,
        OpenedRecoveredWalValidateLedger, PublishedDurableValidateCompletion,
        ReadyValidateCarrierError, RecoveredWalParentFactoryError, RegistryError,
        RegistryPublicationError, reconstruct_recovered_wal_validate_parent,
    },
};
use crate::sumeragi::{
    v2::{AdapterEffect, RecoveredWalVoteSign, VerifiedHeightContext},
    v2_body_store::V2BodyStore,
    v2_runtime::PendingRuntimeEffectBinding,
};

/// Opaque process-local holder for concrete lifecycle work.
///
/// It deliberately exposes only empty construction. Logical scheduling and
/// every registry mutation remain sealed behind [`LifecycleCoordinator`].
pub(crate) struct LifecycleWorkRegistryHolder {
    registry: ConcreteLifecycleWorkRegistry,
}

impl LifecycleWorkRegistryHolder {
    /// Construct an empty holder for a future production lifecycle service.
    pub(crate) fn empty() -> Self {
        Self {
            registry: ConcreteLifecycleWorkRegistry::default(),
        }
    }

    /// Reconstruct one storage-authenticated recovered Validate parent without a scheduler lease.
    ///
    /// The concrete registry never leaves this holder. Success returns only
    /// the opaque opened ledger and authenticated repair, while every failure
    /// retains or restores all WAL and body-marker authority internally.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(crate) fn reconstruct_recovered_wal_validate_parent<'registry, 'body>(
        &'registry mut self,
        verified: &VerifiedHeightContext,
        body_store: &'body mut V2BodyStore,
        ledger_root: &std::path::Path,
        recovered: RecoveredWalVoteSign,
    ) -> Result<
        (
            OpenedRecoveredWalValidateLedger,
            AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
        ),
        RecoveredWalParentFactoryError<'body>,
    > {
        reconstruct_recovered_wal_validate_parent(
            &mut self.registry,
            verified,
            body_store,
            ledger_root,
            recovered,
        )
    }

    /// Wrap a concrete registry for focused atomic-boundary tests.
    #[cfg(test)]
    pub(super) fn from_registry_for_test(registry: ConcreteLifecycleWorkRegistry) -> Self {
        Self { registry }
    }

    /// Borrow the concrete registry for drop- and failure-invariance checks.
    #[cfg(test)]
    pub(super) const fn registry_for_test(&self) -> &ConcreteLifecycleWorkRegistry {
        &self.registry
    }

    /// Mutably borrow the registry for focused corruption and unwind tests.
    #[cfg(test)]
    pub(super) fn registry_for_test_mut(&mut self) -> &mut ConcreteLifecycleWorkRegistry {
        &mut self.registry
    }
}

/// Closed precommit failure from the exact claimed-Validate dispatch cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DurableValidateDispatchError {
    /// The coordinator had already latched a fail-closed condition.
    CoordinatorFaulted,
    /// The supplied lease is not the coordinator's exact claimed Validate row.
    StaleLease,
    /// The closed concrete Validate carrier failed exact execution preflight.
    Registry(DurableValidateExecutionError),
    /// The sealed registry source did not produce one external wait.
    InvalidWaitSource,
    /// No later wake generation can be represented for this exact source.
    WaitGenerationExhausted,
    /// Another waiting lifecycle row already uses the supposedly unique source.
    AliasedWaitSource,
    /// Pure blocked settlement changed more than the exact lease and source row.
    InvalidStagedTransition,
}

/// Closed failure while binding a Ready Validate carrier into scheduler input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReadyValidateDemandAttestationError {
    /// The coordinator row or one of its reverse indexes is not exact and Ready.
    InvalidCoordinatorIndex,
    /// The process-local carrier no longer matches the coordinator address/digest.
    Registry(ReadyValidateCarrierError),
}

/// Closed reason an effect/pending pair could not cross the atomic boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AdapterEffectAdmissionFailure {
    /// The sealed runtime projection rejected the pair or verified context.
    Projection(AdapterEffectAdmissionError),
    /// Exact admitted-address registration rejected the pair.
    Registry(RegistryError),
    /// The lifecycle ledger could not publish after registry installation.
    Durability,
}

/// Ownership-preserving result of one concrete adapter-effect admission.
#[derive(Debug)]
#[must_use = "the caller must retain or execute the returned concrete work"]
pub(super) enum AdapterEffectAdmissionTransaction {
    /// The first logical admission, concrete installation, and optional ledger
    /// publication all committed.
    Admitted(AdmissionDecision),
    /// An exact restart carrier was installed at its existing logical address
    /// and the recovered record's Ready transition was durably published.
    Rebound(AdmissionDecision),
    /// No new concrete entry was installed. The caller retains the exact pair.
    Returned {
        /// Logical decision governing retry, terminal replay, wait, or drop.
        decision: AdmissionDecision,
        /// Exact concrete effect supplied by the caller.
        effect: AdapterEffect,
        /// Move-only binding supplied with the effect.
        pending: PendingRuntimeEffectBinding,
    },
    /// Projection, registration, or publication failed before a new logical
    /// record and concrete entry could commit together.
    Failed {
        /// Closed failure classification.
        failure: AdapterEffectAdmissionFailure,
        /// Exact concrete effect returned to the caller.
        effect: AdapterEffect,
        /// Move-only binding returned to the caller.
        pending: PendingRuntimeEffectBinding,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AdmittedWorkLocation {
    address: ConcreteWorkAddress,
    digest: LifecycleDigest,
}

impl LifecycleCoordinator {
    /// Convert one exact claimed durable Validate lease into an external wait
    /// and return its sole move-only validation dispatch.
    ///
    /// Every fallible check runs before the closed registry request is
    /// detached. Failure returns the exact caller-held lease while leaving the
    /// coordinator and registry unchanged. Success consumes that lease after
    /// a pure coordinator copy has settled it to `Waiting`; the registry row
    /// remains installed and unchanged throughout body-store execution.
    ///
    /// Executable completion is consumed by the volatile same-address
    /// carrier-plus-Ready transaction below. Merge-sidecar deferral still
    /// needs a sealed service registration and same-row wake before this path
    /// is wired.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(super) fn begin_durable_validate_dispatch(
        &mut self,
        registry: &mut LifecycleWorkRegistryHolder,
        lease: TurnLease,
        verified: &VerifiedHeightContext,
    ) -> Result<DurableValidateDispatch, (DurableValidateDispatchError, TurnLease)> {
        if self.fault.is_some() {
            return Err((DurableValidateDispatchError::CoordinatorFaulted, lease));
        }
        if !claimed_durable_validate_record_is_exact(self, &lease) {
            return Err((DurableValidateDispatchError::StaleLease, lease));
        }
        let Some((&slot, _)) = lease.physical_slots().first_key_value() else {
            return Err((
                DurableValidateDispatchError::Registry(
                    DurableValidateExecutionError::InvalidLeaseShape,
                ),
                lease,
            ));
        };
        let prepared = match registry
            .registry
            .prepare_durable_validate_execution(&lease, slot, verified)
        {
            Ok(prepared) => prepared,
            Err(error) => {
                return Err((DurableValidateDispatchError::Registry(error), lease));
            }
        };
        if !self
            .durable_records
            .get(&lease.ordinal())
            .is_some_and(|metadata| prepared.matches_durable_payload(metadata.payload))
        {
            drop(prepared);
            return Err((
                DurableValidateDispatchError::Registry(
                    DurableValidateExecutionError::InvalidValidateShape,
                ),
                lease,
            ));
        }
        let source = prepared.durable_validation_wait_source();
        if !matches!(source, WaitSource::External(_)) {
            drop(prepared);
            return Err((DurableValidateDispatchError::InvalidWaitSource, lease));
        }
        let observed_generation = self.observed_generation.get(&source).copied().unwrap_or(0);
        if observed_generation == u64::MAX {
            drop(prepared);
            return Err((DurableValidateDispatchError::WaitGenerationExhausted, lease));
        }
        if self.records.iter().any(|(ordinal, record)| {
            *ordinal != lease.ordinal()
                && matches!(
                    record.state,
                    LifecycleState::Waiting(wait) if wait.source() == source
                )
        }) {
            drop(prepared);
            return Err((DurableValidateDispatchError::AliasedWaitSource, lease));
        }

        let wait_token = WaitToken::new(source, observed_generation);
        let mut next = self.stage_durable_transaction();
        next.reduce_settle_turn(lease.clone(), TurnOutcome::Blocked(wait_token), None);
        if !staged_durable_validate_wait_is_exact(self, &next, &lease, wait_token) {
            drop(prepared);
            return Err((DurableValidateDispatchError::InvalidStagedTransition, lease));
        }
        let dispatch = match prepared.seal_waiting_dispatch(wait_token) {
            Ok(dispatch) => dispatch,
            Err(prepared) => {
                drop(prepared);
                return Err((DurableValidateDispatchError::InvalidWaitSource, lease));
            }
        };

        *self = next;
        Ok(dispatch)
    }

    /// Atomically publish one exact executable Validate result across the
    /// volatile coordinator and concrete registry.
    ///
    /// All fallible checks and the complete logical Ready projection precede
    /// registry staging. The specialized registry guard then owns rollback
    /// until the adjacent, non-panicking coordinator swap and guard commit.
    /// Merge-sidecar deferral changes neither side and retains the full
    /// executed dispatch for its future sealed service transaction.
    ///
    /// Waiting/Ready state and concrete physical carriers are intentionally
    /// excluded from LifecycleLedgerV1, so this volatile cut performs no
    /// lifecycle-ledger write. A crash before the swap restores or loses only
    /// volatile staging; a crash after it recovers the durable Validate row and
    /// revalidates the body-store marker.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(super) fn complete_durable_validate_dispatch(
        &mut self,
        registry: &mut LifecycleWorkRegistryHolder,
        dispatch: ExecutedDurableValidateDispatch,
    ) -> Result<
        DurableValidateCompletionPublication,
        (
            DurableValidateCompletionPublicationError,
            ExecutedDurableValidateDispatch,
        ),
    > {
        if self.fault.is_some() {
            return Err((
                DurableValidateCompletionPublicationError::CoordinatorFaulted,
                dispatch,
            ));
        }
        let prepared = registry
            .registry
            .prepare_executed_durable_validate_completion(dispatch)?;
        let authority = prepared.authority();
        if !waiting_durable_validate_record_is_exact(self, authority) {
            return Err(
                prepared.fail(DurableValidateCompletionPublicationError::InvalidWaitingState)
            );
        }
        if authority.is_deferred_merge_sidecar() {
            // TODO: Join the sealed missing-sidecar registration and exact
            // same-row wake before this deferred token gains a consumer.
            return Ok(DurableValidateCompletionPublication::DeferredMergeSidecar(
                prepared.defer_merge_sidecar(),
            ));
        }

        let Some(ready_event) = authority.ready_event() else {
            return Err(
                prepared.fail(DurableValidateCompletionPublicationError::InvalidStagedTransition)
            );
        };
        let mut next = self.stage_durable_transaction();
        next.publish_ready(ready_event);
        if !staged_durable_validate_ready_is_exact(self, &next, authority) {
            return Err(
                prepared.fail(DurableValidateCompletionPublicationError::InvalidStagedTransition)
            );
        }
        let staged_registry = prepared.stage_executable_carrier()?;

        core::mem::swap(self, &mut next);
        let published = staged_registry.commit();
        Ok(match published {
            PublishedDurableValidateCompletion::Validated(published) => {
                DurableValidateCompletionPublication::PublishedValidated(published)
            }
            PublishedDurableValidateCompletion::Rejected(published) => {
                DurableValidateCompletionPublication::PublishedRejected(published)
            }
        })
    }

    /// Bind one exact Ready Validate carrier into a transient scheduler seal.
    ///
    /// The registry returns only a closed carrier classification. This method
    /// joins it to the coordinator's complete row identity and physical digest
    /// without exposing a caller-mintable demand bit or capacity class.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn attest_ready_validate_demand(
        &self,
        registry: &LifecycleWorkRegistryHolder,
        ordinal: u128,
    ) -> Result<AttestedReadyValidateDemand, ReadyValidateDemandAttestationError> {
        let Some(record) = self.records.get(&ordinal) else {
            return Err(ReadyValidateDemandAttestationError::InvalidCoordinatorIndex);
        };
        let readiness_index_is_exact = match record.state {
            LifecycleState::Ready => self.ready_index.contains(&ordinal),
            LifecycleState::Waiting(wait)
                if matches!(
                    wait.source(),
                    WaitSource::External(_) | WaitSource::Recovery(_)
                ) =>
            {
                !self.ready_index.contains(&ordinal)
            }
            LifecycleState::Waiting(_)
            | LifecycleState::Claimed(_)
            | LifecycleState::Terminal(_) => false,
        };
        if self.fault.is_some()
            || self.active_lease.is_some()
            || record.ordinal != ordinal
            || record.work_class != LifecycleWorkClass::Validate
            || record.key.phase() != LifecyclePhase::Validate
            || record.stage.kind() != LifecycleStageKind::ValidateBody
            || record.stage.predecessor_scope() != PredecessorScope::Independent
            || record.physical_slots.len() != 1
            || record.episode.slot_universe.len() != 1
            || record.episode.consumed_slots != record.episode.slot_universe
            || !record.episode.frozen_predecessors.is_empty()
            || self.episode_authority.universe_for(record.key).as_ref()
                != Some(&record.episode.universe)
            || !self.episode_authority.admits_slots(
                record.work_class.capacity_class(),
                &record.episode.slot_universe,
            )
            || !readiness_index_is_exact
            || self.key_index.get(&record.key) != Some(&ordinal)
            || self.owner_index.get(&record.owner.causal_root()) != Some(&record.owner)
            || self
                .records
                .values()
                .filter(|candidate| candidate.ordinal == ordinal)
                .count()
                != 1
            || self
                .records
                .values()
                .filter(|candidate| candidate.key == record.key)
                .count()
                != 1
            || self
                .key_index
                .values()
                .filter(|candidate| **candidate == ordinal)
                .count()
                != 1
            || self
                .owner_index
                .values()
                .filter(|owner| **owner == record.owner)
                .count()
                != 1
            || !self.durable_records.get(&ordinal).is_some_and(|metadata| {
                metadata.reconstruction_source == record.owner.causal_root().digest()
                    && durable_validate_payload_is_exact(record.key, metadata.payload)
                    && metadata.continuation == super::schema::DurableContinuation::None
            })
        {
            return Err(ReadyValidateDemandAttestationError::InvalidCoordinatorIndex);
        }
        let (&slot, &digest) = record
            .physical_slots
            .first_key_value()
            .expect("one-slot Ready Validate shape checked above");
        if !record.episode.slot_universe.contains(&slot)
            || slot.capacity_class() != Some(record.work_class.capacity_class())
        {
            return Err(ReadyValidateDemandAttestationError::InvalidCoordinatorIndex);
        }
        let address = ConcreteWorkAddress::new(record.owner, ordinal, slot)
            .ok_or(ReadyValidateDemandAttestationError::InvalidCoordinatorIndex)?;
        let seal = registry
            .registry
            .classify_ready_validate_carrier(address, digest)
            .map_err(ReadyValidateDemandAttestationError::Registry)?;
        if !self
            .durable_records
            .get(&ordinal)
            .is_some_and(|metadata| seal.matches_durable_payload(record.key, metadata.payload))
        {
            return Err(ReadyValidateDemandAttestationError::InvalidCoordinatorIndex);
        }
        AttestedReadyValidateDemand::from_registry_seal(record, seal)
            .ok_or(ReadyValidateDemandAttestationError::InvalidCoordinatorIndex)
    }

    /// Atomically admit and register one exact adapter effect.
    ///
    /// The effect and pending authority are consumed. A first admission stages
    /// logical state, installs the exact coordinator-minted owner/ordinal/slot,
    /// publishes the ledger, and only then exposes both state changes. Registry
    /// installation is synchronously undone if publication fails. An exact
    /// recovered retry installs at its existing immutable address with the
    /// same ordering. Every other decision returns the pair and leaves
    /// incumbent concrete work untouched.
    // TODO: The one-cut production switch must retain or drop each returned
    // pair according to its decision; Retry executes the incumbent entry and
    // must never replace it with the returned duplicate.
    pub(super) fn admit_concrete_adapter_effect(
        &mut self,
        registry: &mut LifecycleWorkRegistryHolder,
        verified: &VerifiedHeightContext,
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
    ) -> AdapterEffectAdmissionTransaction {
        let request =
            match projection::admission_request(self.active_context, verified, &effect, &pending) {
                Ok(request) => request,
                Err(error) => {
                    return AdapterEffectAdmissionTransaction::Failed {
                        failure: AdapterEffectAdmissionFailure::Projection(error),
                        effect,
                        pending,
                    };
                }
            };
        let recovery_rebind_ordinal = match &request {
            AdmissionRequest::Candidate(candidate) => self
                .key_index
                .get(&candidate.key)
                .copied()
                .filter(|ordinal| {
                    self.records.get(ordinal).is_some_and(|record| {
                        matches!(
                            record.state,
                            LifecycleState::Waiting(super::WaitToken {
                                source: WaitSource::Recovery(_),
                                ..
                            })
                        )
                    })
                }),
            AdmissionRequest::NonCandidate(_) => None,
        };
        let work = match ConcreteLifecycleWork::from_exact(effect, pending) {
            Ok(work) => work,
            Err((error, effect, pending)) => {
                return AdapterEffectAdmissionTransaction::Failed {
                    failure: AdapterEffectAdmissionFailure::Registry(error),
                    effect,
                    pending,
                };
            }
        };

        let mut next = self.stage_durable_transaction();
        let decision = next.reduce_admit(request);
        let first_admission = matches!(decision, AdmissionDecision::Admitted { .. });
        let recovery_rebind = matches!(
            decision,
            AdmissionDecision::Retry { ordinal, .. }
                if recovery_rebind_ordinal == Some(ordinal)
                    && next.records.get(&ordinal).is_some_and(|record| {
                        record.state == LifecycleState::Ready
                    })
        );
        if first_admission || recovery_rebind {
            let location = match concrete_work_location(&next, decision, recovery_rebind_ordinal) {
                Ok(location) => location,
                Err(error) => {
                    let (effect, pending) = work.into_pair();
                    return AdapterEffectAdmissionTransaction::Failed {
                        failure: AdapterEffectAdmissionFailure::Registry(error),
                        effect,
                        pending,
                    };
                }
            };
            return match registry.registry.install_before_publication(
                location.address,
                location.digest,
                work,
                || next.persist_durable_projection(),
            ) {
                Ok(()) => {
                    *self = next;
                    if first_admission {
                        AdapterEffectAdmissionTransaction::Admitted(decision)
                    } else {
                        AdapterEffectAdmissionTransaction::Rebound(decision)
                    }
                }
                Err(RegistryPublicationError::Install(error, work)) => {
                    let (effect, pending) = work.into_pair();
                    AdapterEffectAdmissionTransaction::Failed {
                        failure: AdapterEffectAdmissionFailure::Registry(error),
                        effect,
                        pending,
                    }
                }
                Err(RegistryPublicationError::Publication(_, work)) => {
                    self.fault = Some(CoordinatorFault::DurabilityFailure);
                    let (effect, pending) = work.into_pair();
                    AdapterEffectAdmissionTransaction::Failed {
                        failure: AdapterEffectAdmissionFailure::Durability,
                        effect,
                        pending,
                    }
                }
            };
        }

        let (effect, pending) = work.into_pair();
        *self = next;
        AdapterEffectAdmissionTransaction::Returned {
            decision,
            effect,
            pending,
        }
    }
}

fn waiting_durable_validate_record_is_exact(
    coordinator: &LifecycleCoordinator,
    authority: DurableValidateCompletionAuthority,
) -> bool {
    let Some(record) = coordinator.records.get(&authority.ordinal()) else {
        return false;
    };
    let mut exact_slots = std::collections::BTreeSet::new();
    exact_slots.insert(authority.slot());
    let wait_token = authority.wait_token();
    coordinator.active_lease.is_none()
        && record.ordinal == authority.ordinal()
        && record.owner == authority.owner()
        && record.key == authority.lifecycle_key()
        && record.work_class == LifecycleWorkClass::Validate
        && record.key.phase() == LifecyclePhase::Validate
        && record.stage == authority.lifecycle_stage()
        && record.stage.kind() == LifecycleStageKind::ValidateBody
        && record.stage.predecessor_scope() == PredecessorScope::Independent
        && record.state == LifecycleState::Waiting(wait_token)
        && record.physical_slots.len() == 1
        && record.physical_slots.get(&authority.slot()) == Some(&authority.incumbent_digest())
        && record.episode.slot_universe == exact_slots
        && record.episode.consumed_slots == exact_slots
        && record.episode.frozen_predecessors.is_empty()
        && coordinator
            .episode_authority
            .universe_for(record.key)
            .as_ref()
            == Some(&record.episode.universe)
        && coordinator.episode_authority.admits_slots(
            record.work_class.capacity_class(),
            &record.episode.slot_universe,
        )
        && coordinator.key_index.get(&record.key) == Some(&record.ordinal)
        && coordinator.owner_index.get(&record.owner.causal_root()) == Some(&record.owner)
        && coordinator
            .records
            .values()
            .filter(|candidate| candidate.ordinal == record.ordinal)
            .count()
            == 1
        && coordinator
            .records
            .values()
            .filter(|candidate| candidate.key == record.key)
            .count()
            == 1
        && coordinator
            .key_index
            .values()
            .filter(|ordinal| **ordinal == record.ordinal)
            .count()
            == 1
        && coordinator
            .owner_index
            .values()
            .filter(|owner| **owner == record.owner)
            .count()
            == 1
        && !coordinator.ready_index.contains(&record.ordinal)
        && coordinator.observed_generation.get(&wait_token.source())
            == Some(&wait_token.observed_generation())
        && coordinator.records.iter().all(|(ordinal, candidate)| {
            *ordinal == record.ordinal
                || !matches!(
                    candidate.state,
                    LifecycleState::Waiting(wait) if wait.source() == wait_token.source()
                )
        })
        && coordinator
            .durable_records
            .get(&record.ordinal)
            .is_some_and(|metadata| {
                metadata.reconstruction_source == record.owner.causal_root().digest()
                    && durable_validate_payload_is_exact(record.key, metadata.payload)
                    && authority.matches_durable_payload(metadata.payload)
            })
}

#[allow(clippy::too_many_lines)]
fn staged_durable_validate_ready_is_exact(
    current: &LifecycleCoordinator,
    next: &LifecycleCoordinator,
    authority: DurableValidateCompletionAuthority,
) -> bool {
    let Some(replacement_digest) = authority.replacement_digest() else {
        return false;
    };
    let Some(next_generation) = authority.wait_token().observed_generation().checked_add(1) else {
        return false;
    };
    let mut expected_record = current
        .records
        .get(&authority.ordinal())
        .expect("waiting Validate preflight checked its exact record")
        .clone();
    expected_record.state = LifecycleState::Ready;
    expected_record
        .physical_slots
        .insert(authority.slot(), replacement_digest);
    let mut expected_ready = current.ready_index.clone();
    expected_ready.insert(authority.ordinal());
    let mut expected_observed = current.observed_generation.clone();
    expected_observed.insert(authority.wait_token().source(), next_generation);

    next.episode_authority == current.episode_authority
        && next.active_context == current.active_context
        && next.records.len() == current.records.len()
        && next.records.get(&authority.ordinal()) == Some(&expected_record)
        && current.records.iter().all(|(ordinal, record)| {
            *ordinal == authority.ordinal() || next.records.get(ordinal) == Some(record)
        })
        && next.key_index == current.key_index
        && next.owner_index == current.owner_index
        && next.ready_index == expected_ready
        && next.admission_waits == current.admission_waits
        && next.active_lease == current.active_lease
        && next.high_water == current.high_water
        && next.next_lease == current.next_lease
        && next.durable_records == current.durable_records
        && next.capacity_geometry == current.capacity_geometry
        && next.capacity_used == current.capacity_used
        && next.capacity_generation == current.capacity_generation
        && next.observed_generation == expected_observed
        && next.producer_debts == current.producer_debts
        && next.ledger_store.is_some() == current.ledger_store.is_some()
        && next.fault.is_none()
}

fn claimed_durable_validate_record_is_exact(
    coordinator: &LifecycleCoordinator,
    lease: &TurnLease,
) -> bool {
    let Some(record) = coordinator.records.get(&lease.ordinal()) else {
        return false;
    };
    let lease_slots = lease
        .physical_slots()
        .keys()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    coordinator.active_lease.as_ref() == Some(lease)
        && record.ordinal == lease.ordinal()
        && record.owner == lease.owner()
        && record.key == lease.key()
        && record.work_class == LifecycleWorkClass::Validate
        && record.work_class == lease.work_class()
        && record.key.phase() == LifecyclePhase::Validate
        && record.stage == lease.stage()
        && record.stage.kind() == LifecycleStageKind::ValidateBody
        && record.stage.predecessor_scope() == PredecessorScope::Independent
        && record.state == LifecycleState::Claimed(lease.id())
        && record.physical_slots.len() == 1
        && record.physical_slots == *lease.physical_slots()
        && record.episode.slot_universe == lease_slots
        && record.episode.consumed_slots == lease_slots
        && record.episode.frozen_predecessors.is_empty()
        && coordinator
            .episode_authority
            .universe_for(record.key)
            .as_ref()
            == Some(&record.episode.universe)
        && coordinator.episode_authority.admits_slots(
            record.work_class.capacity_class(),
            &record.episode.slot_universe,
        )
        && coordinator.key_index.get(&record.key) == Some(&record.ordinal)
        && coordinator.owner_index.get(&record.owner.causal_root()) == Some(&record.owner)
        && coordinator
            .records
            .values()
            .filter(|candidate| candidate.ordinal == record.ordinal)
            .count()
            == 1
        && coordinator
            .records
            .values()
            .filter(|candidate| candidate.key == record.key)
            .count()
            == 1
        && coordinator
            .key_index
            .values()
            .filter(|ordinal| **ordinal == record.ordinal)
            .count()
            == 1
        && coordinator
            .owner_index
            .values()
            .filter(|owner| **owner == record.owner)
            .count()
            == 1
        && !coordinator.ready_index.contains(&record.ordinal)
        && coordinator
            .durable_records
            .get(&record.ordinal)
            .is_some_and(|metadata| {
                metadata.reconstruction_source == record.owner.causal_root().digest()
                    && durable_validate_payload_is_exact(record.key, metadata.payload)
            })
}

#[allow(clippy::too_many_lines)]
fn staged_durable_validate_wait_is_exact(
    current: &LifecycleCoordinator,
    next: &LifecycleCoordinator,
    lease: &TurnLease,
    wait_token: WaitToken,
) -> bool {
    let mut expected_record = current
        .records
        .get(&lease.ordinal())
        .expect("claimed Validate preflight checked its record")
        .clone();
    expected_record.state = LifecycleState::Waiting(wait_token);
    let mut expected_observed = current.observed_generation.clone();
    expected_observed
        .entry(wait_token.source())
        .or_insert(wait_token.observed_generation());

    next.episode_authority == current.episode_authority
        && next.active_context == current.active_context
        && next.records.len() == current.records.len()
        && next.records.get(&lease.ordinal()) == Some(&expected_record)
        && current.records.iter().all(|(ordinal, record)| {
            *ordinal == lease.ordinal() || next.records.get(ordinal) == Some(record)
        })
        && next.key_index == current.key_index
        && next.owner_index == current.owner_index
        && next.ready_index == current.ready_index
        && next.admission_waits == current.admission_waits
        && next.active_lease.is_none()
        && next.high_water == current.high_water
        && next.next_lease == current.next_lease
        && next.durable_records == current.durable_records
        && next.capacity_geometry == current.capacity_geometry
        && next.capacity_used == current.capacity_used
        && next.capacity_generation == current.capacity_generation
        && next.observed_generation == expected_observed
        && next.producer_debts == current.producer_debts
        && next.ledger_store.is_some() == current.ledger_store.is_some()
        && next.fault.is_none()
}

fn concrete_work_location(
    coordinator: &LifecycleCoordinator,
    decision: AdmissionDecision,
    recovery_rebind_ordinal: Option<u128>,
) -> Result<AdmittedWorkLocation, RegistryError> {
    let (owner, ordinal) = match decision {
        AdmissionDecision::Admitted {
            owner,
            ordinal,
            producer_turn_ordinal: None,
        } => (owner, ordinal),
        AdmissionDecision::Retry { owner, ordinal, .. }
            if recovery_rebind_ordinal == Some(ordinal) =>
        {
            (owner, ordinal)
        }
        AdmissionDecision::Admitted {
            producer_turn_ordinal: Some(_),
            ..
        }
        | AdmissionDecision::Retry { .. }
        | AdmissionDecision::ReplayTerminal { .. }
        | AdmissionDecision::StutterTerminal { .. }
        | AdmissionDecision::NonCandidate
        | AdmissionDecision::WaitForCapacity(_)
        | AdmissionDecision::Rejected(_)
        | AdmissionDecision::FailClosed(_) => {
            return Err(RegistryError::InvalidAdmissionShape);
        }
    };
    let record = coordinator
        .records
        .get(&ordinal)
        .ok_or(RegistryError::InvalidAdmissionShape)?;
    if record.owner != owner || record.ordinal != ordinal || record.physical_slots.len() != 1 {
        return Err(RegistryError::InvalidAdmissionShape);
    }
    let (&slot, &digest) = record
        .physical_slots
        .first_key_value()
        .expect("one-slot admission was checked above");
    let address =
        ConcreteWorkAddress::new(owner, ordinal, slot).ok_or(RegistryError::InvalidAddress)?;
    Ok(AdmittedWorkLocation { address, digest })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{block::consensus_v2 as wire, peer::PeerId};
    use tempfile::TempDir;

    use super::super::{OwnerId, PhysicalSlotId};
    use super::*;
    use crate::sumeragi::{
        v2_core::{EventTag, Generation},
        v2_runtime::{RuntimeEffectOwnership, bind_adapter_effect_batch_ownership},
    };

    struct Fixture {
        verified: VerifiedHeightContext,
        context: wire::HeightContext,
        round: wire::ConsensusRound,
        tag: EventTag,
    }

    impl Fixture {
        fn new() -> Self {
            let mut keys = (1_u8..=4)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic concrete-admission BLS key")
                })
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let proofs = keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("concrete-admission proof of possession")
                })
                .collect::<Vec<_>>();
            let roster = keys
                .iter()
                .map(|key| wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                })
                .collect::<Vec<_>>();
            let context = wire::HeightContext {
                network_id: crate::sumeragi::synthetic_network_id(
                    "sumeragi-v2-concrete-admission-test",
                ),
                protocol_version: wire::PROTOCOL_VERSION,
                height: 7,
                epoch: 1,
                epoch_end_height: 100,
                next_epoch_snapshot: None,
                mode: wire::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                snapshot_bootstrap: None,
                quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
                roster,
                nexus_amx_context_hash: Hash::new(b"concrete admission nexus context"),
                execution_policy_hash: Hash::new(b"concrete admission execution policy"),
                da_layout: wire::DataAvailabilityLayout {
                    encoding: wire::PayloadEncoding::ReedSolomon16,
                    chunk_size_bytes: 1024,
                    data_shards: 1,
                    parity_shards: 1,
                    max_payload_size_bytes: 512 * 1024,
                    max_chunk_count: 1024,
                },
                leader_seed: [0xA7; 32],
            };
            let verified = VerifiedHeightContext::genesis(context.clone(), proofs)
                .expect("verified concrete-admission context");
            let round = wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 2,
            };
            Self {
                verified,
                context,
                round,
                tag: EventTag::new(7, 2, Generation::new(1)),
            }
        }

        fn active_context(&self) -> super::super::LifecycleContext {
            let mut digest = [0_u8; 32];
            digest.copy_from_slice(self.context.id().0.as_ref());
            super::super::LifecycleContext::new(LifecycleDigest::new(digest), self.context.height)
        }

        fn coordinator(&self, consensus_capacity: usize) -> LifecycleCoordinator {
            LifecycleCoordinator::new(
                self.active_context(),
                0,
                super::super::schema::CapacityGeometry::new([
                    (super::super::CapacityClass::Consensus, consensus_capacity),
                    (super::super::CapacityClass::Effect, 64),
                    (super::super::CapacityClass::Serve, 64),
                    (super::super::CapacityClass::Producer, 64),
                ]),
            )
        }

        fn effect(&self, marker: u8) -> AdapterEffect {
            let subject = wire::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 1])),
                payload_hash: Hash::new([marker, 2]),
            };
            let commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new([marker, 3]),
                Hash::new([marker, 4]),
                Hash::new([marker, 5]),
                1,
                Hash::new([marker, 6]),
            );
            AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(wire::Vote {
                    round: self.round,
                    proposal_round: self.round,
                    phase: wire::GlobalPhase::Prepare,
                    subject,
                    execution_commitment: commitment,
                    signer: 0,
                    signature: vec![marker],
                }),
            ))
        }

        fn pair(
            &self,
            effect: AdapterEffect,
            source_ordinal: u128,
        ) -> (AdapterEffect, PendingRuntimeEffectBinding) {
            let ownership = bind_adapter_effect_batch_ownership(
                core::slice::from_ref(&effect),
                vec![RuntimeEffectOwnership::fresh_for_test(
                    self.tag,
                    source_ordinal,
                )],
            )
            .expect("bind exact concrete-admission fixture")
            .pop()
            .expect("one concrete-admission owner");
            let pending = ownership
                .pending_adapter_effect_binding(&effect)
                .expect("mint pending concrete-admission binding");
            (effect, pending)
        }
    }

    fn consensus_slot() -> PhysicalSlotId {
        PhysicalSlotId::for_capacity(super::super::CapacityClass::Consensus, 0)
    }

    fn recovery_snapshot(coordinator: &LifecycleCoordinator) -> super::super::RecoverySnapshot {
        super::super::RecoverySnapshot {
            context: coordinator.active_context,
            high_water: coordinator.high_water,
            records: coordinator
                .records
                .values()
                .map(|record| super::super::RecoveredLifecycleRecord {
                    key: record.key,
                    owner: record.owner,
                    ordinal: record.ordinal,
                    work_class: record.work_class,
                    stage: record.stage,
                    terminal: match record.state {
                        LifecycleState::Terminal(outcome) => Some(outcome),
                        LifecycleState::Waiting(_)
                        | LifecycleState::Ready
                        | LifecycleState::Claimed(_) => None,
                    },
                    reconstruction_source: coordinator.durable_records[&record.ordinal]
                        .reconstruction_source,
                    payload: coordinator.durable_records[&record.ordinal].payload,
                    replay_authority: coordinator.durable_records[&record.ordinal]
                        .replay_authority
                        .clone(),
                    continuation: coordinator.durable_records[&record.ordinal].continuation,
                    physical_slot_universe: record.episode.slot_universe.clone(),
                })
                .collect(),
            producer_debts: coordinator.producer_debts.clone(),
        }
    }

    #[test]
    fn occupied_address_returns_pair_and_leaves_coordinator_unchanged() {
        let fixture = Fixture::new();
        let effect = fixture.effect(1);
        let (incumbent_effect, incumbent_pending) = fixture.pair(effect.clone(), 90);
        let incumbent = ConcreteLifecycleWork::from_exact(incumbent_effect, incumbent_pending)
            .expect("construct exact incumbent");
        let digest = incumbent.digest();
        let owner = OwnerId::new(incumbent.causal_root(), 1);
        let address = ConcreteWorkAddress::new(owner, 1, consensus_slot())
            .expect("valid prospective address");
        let mut registry: super::super::LifecycleWorkRegistryHolder =
            LifecycleWorkRegistryHolder::empty();
        registry
            .registry
            .install(address, digest, incumbent)
            .expect("install simulated incumbent");
        let mut coordinator = fixture.coordinator(64);
        let records_before = coordinator.records.clone();
        let owners_before = coordinator.owner_index.clone();
        let capacity_before = coordinator.capacity_used.clone();
        let (effect, pending) = fixture.pair(effect.clone(), 91);

        let outcome = coordinator.admit_concrete_adapter_effect(
            &mut registry,
            &fixture.verified,
            effect.clone(),
            pending,
        );
        let AdapterEffectAdmissionTransaction::Failed {
            failure: AdapterEffectAdmissionFailure::Registry(RegistryError::Occupied),
            effect: returned_effect,
            pending: returned_pending,
        } = outcome
        else {
            panic!("occupied exact address must return the incoming pair")
        };
        assert_eq!(coordinator.high_water(), 0);
        assert_eq!(coordinator.records, records_before);
        assert_eq!(coordinator.owner_index, owners_before);
        assert_eq!(coordinator.capacity_used, capacity_before);
        assert!(registry.registry.exactly_contains(address, &effect));
        assert_eq!(returned_effect, effect);
        assert!(returned_pending.exactly_binds_adapter_effect(&returned_effect));
    }

    #[test]
    fn capacity_wait_returns_the_same_pair_for_each_exact_retry() {
        let fixture = Fixture::new();
        let mut coordinator = fixture.coordinator(1);
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let first = fixture.effect(2);
        let (effect, pending) = fixture.pair(first, 92);
        assert!(matches!(
            coordinator.admit_concrete_adapter_effect(
                &mut registry,
                &fixture.verified,
                effect,
                pending,
            ),
            AdapterEffectAdmissionTransaction::Admitted(AdmissionDecision::Admitted {
                ordinal: 1,
                ..
            })
        ));

        let waiting_effect = fixture.effect(3);
        let (effect, pending) = fixture.pair(waiting_effect.clone(), 93);
        let outcome = coordinator.admit_concrete_adapter_effect(
            &mut registry,
            &fixture.verified,
            effect,
            pending,
        );
        let AdapterEffectAdmissionTransaction::Returned {
            decision: AdmissionDecision::WaitForCapacity(first_wait),
            effect,
            pending,
        } = outcome
        else {
            panic!("full exact capacity must return the waiting pair")
        };
        assert_eq!(effect, waiting_effect);
        assert!(pending.exactly_binds_adapter_effect(&effect));
        assert_eq!(coordinator.high_water(), 1);
        assert_eq!(coordinator.admission_waits.len(), 1);
        let outcome = coordinator.admit_concrete_adapter_effect(
            &mut registry,
            &fixture.verified,
            effect,
            pending,
        );
        let AdapterEffectAdmissionTransaction::Returned {
            decision: AdmissionDecision::WaitForCapacity(second_wait),
            effect,
            pending,
        } = outcome
        else {
            panic!("unchanged generation must return the exact retry pair")
        };
        assert_eq!(second_wait, first_wait);
        assert_eq!(effect, waiting_effect);
        assert!(pending.exactly_binds_adapter_effect(&effect));
        assert_eq!(coordinator.high_water(), 1);
        assert_eq!(coordinator.admission_waits.len(), 1);
        assert_eq!(registry.registry.len(), 1);
    }

    #[test]
    fn admitted_location_rejects_causal_owner_and_digest_mismatch() {
        let fixture = Fixture::new();
        let effect = fixture.effect(4);
        let (effect, pending) = fixture.pair(effect, 94);
        let work = ConcreteLifecycleWork::from_exact(effect, pending).expect("exact work");
        let wrong_owner = OwnerId::new(
            super::super::CausalRoot::new(LifecycleDigest::new([0xF4; 32])),
            1,
        );
        let wrong_owner_address =
            ConcreteWorkAddress::new(wrong_owner, 1, consensus_slot()).expect("valid address");
        let published = Cell::new(false);
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let error = registry
            .registry
            .install_before_publication(wrong_owner_address, work.digest(), work, || {
                published.set(true);
                Ok::<_, ()>(())
            })
            .expect_err("foreign causal owner must fail before publication");
        let RegistryPublicationError::Install(RegistryError::CausalOwnerMismatch, work) = error
        else {
            panic!("causal mismatch must return the exact work")
        };
        assert!(!published.get());
        let (effect, pending) = work.into_pair();
        assert!(pending.exactly_binds_adapter_effect(&effect));
        assert!(registry.registry.is_empty());

        let work = ConcreteLifecycleWork::from_exact(effect, pending).expect("returned exact work");
        let owner = OwnerId::new(work.causal_root(), 1);
        let address =
            ConcreteWorkAddress::new(owner, 1, consensus_slot()).expect("valid exact address");
        let error = registry
            .registry
            .install_before_publication(address, LifecycleDigest::new([0xD4; 32]), work, || {
                published.set(true);
                Ok::<_, ()>(())
            })
            .expect_err("foreign digest must fail before publication");
        let RegistryPublicationError::Install(RegistryError::DigestMismatch, work) = error else {
            panic!("digest mismatch must return the exact work")
        };
        assert!(!published.get());
        assert!(work.validate_exact());
        assert!(registry.registry.is_empty());
    }

    #[test]
    fn retry_and_terminal_decisions_never_replace_incumbent_work() {
        let fixture = Fixture::new();
        let original = fixture.effect(5);
        let mut coordinator = fixture.coordinator(64);
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let (effect, pending) = fixture.pair(original.clone(), 95);
        assert!(matches!(
            coordinator.admit_concrete_adapter_effect(
                &mut registry,
                &fixture.verified,
                effect,
                pending,
            ),
            AdapterEffectAdmissionTransaction::Admitted(AdmissionDecision::Admitted {
                ordinal: 1,
                ..
            })
        ));
        let record = coordinator.records.get(&1).expect("first admitted record");
        let address = ConcreteWorkAddress::new(record.owner, 1, consensus_slot())
            .expect("exact incumbent address");
        assert!(registry.registry.exactly_contains(address, &original));

        let (effect, pending) = fixture.pair(original.clone(), 96);
        let outcome = coordinator.admit_concrete_adapter_effect(
            &mut registry,
            &fixture.verified,
            effect,
            pending,
        );
        let AdapterEffectAdmissionTransaction::Returned {
            decision: AdmissionDecision::Retry { ordinal: 1, .. },
            effect,
            pending,
        } = outcome
        else {
            panic!("live duplicate must return a retry pair")
        };
        assert!(pending.exactly_binds_adapter_effect(&effect));
        assert!(registry.registry.exactly_contains(address, &original));
        assert_eq!(registry.registry.len(), 1);

        coordinator
            .records
            .get_mut(&1)
            .expect("first admitted record")
            .state =
            super::super::LifecycleState::Terminal(super::super::TerminalOutcome::Cancelled);
        let (effect, pending) = fixture.pair(original.clone(), 97);
        let outcome = coordinator.admit_concrete_adapter_effect(
            &mut registry,
            &fixture.verified,
            effect,
            pending,
        );
        let AdapterEffectAdmissionTransaction::Returned {
            decision: AdmissionDecision::StutterTerminal { .. },
            effect,
            pending,
        } = outcome
        else {
            panic!("terminal duplicate must return its pair")
        };
        assert!(pending.exactly_binds_adapter_effect(&effect));
        assert!(registry.registry.exactly_contains(address, &original));
        assert_eq!(registry.registry.len(), 1);
        assert_eq!(coordinator.high_water(), 1);
    }

    #[test]
    fn recovered_retry_installs_exact_work_without_allocating() {
        let fixture = Fixture::new();
        let original = fixture.effect(8);
        let mut live = fixture.coordinator(64);
        let mut live_registry = LifecycleWorkRegistryHolder::empty();
        let (effect, pending) = fixture.pair(original.clone(), 100);
        let AdapterEffectAdmissionTransaction::Admitted(
            decision @ AdmissionDecision::Admitted { ordinal, .. },
        ) = live.admit_concrete_adapter_effect(
            &mut live_registry,
            &fixture.verified,
            effect,
            pending,
        )
        else {
            panic!("fixture must admit one concrete effect")
        };
        assert_eq!(ordinal, 1);
        let snapshot = recovery_snapshot(&live);

        let mut recovered = LifecycleCoordinator::new(
            fixture.active_context(),
            live.high_water,
            super::super::schema::CapacityGeometry::new([
                (super::super::CapacityClass::Consensus, 64),
                (super::super::CapacityClass::Effect, 64),
                (super::super::CapacityClass::Serve, 64),
                (super::super::CapacityClass::Producer, 64),
            ]),
        );
        recovered.reconcile_restart(snapshot);
        assert!(matches!(
            recovered.records[&ordinal].state,
            LifecycleState::Waiting(super::super::WaitToken {
                source: WaitSource::Recovery(_),
                ..
            })
        ));
        assert!(recovered.records[&ordinal].physical_slots.is_empty());

        let mut registry = LifecycleWorkRegistryHolder::empty();
        let (effect, pending) = fixture.pair(original.clone(), 101);
        let outcome = recovered.admit_concrete_adapter_effect(
            &mut registry,
            &fixture.verified,
            effect,
            pending,
        );
        let AdapterEffectAdmissionTransaction::Rebound(
            retry @ AdmissionDecision::Retry {
                ordinal: rebound_ordinal,
                ..
            },
        ) = outcome
        else {
            panic!("exact recovery retry must atomically install concrete work")
        };
        assert_eq!(rebound_ordinal, ordinal);
        assert_eq!(recovered.high_water, 1);
        assert_eq!(recovered.records[&ordinal].state, LifecycleState::Ready);
        let location = concrete_work_location(&recovered, retry, Some(ordinal))
            .expect("rebound work has one exact location");
        assert!(
            registry
                .registry
                .exactly_contains(location.address, &original)
        );
        assert_eq!(registry.registry.len(), 1);
        assert!(matches!(decision, AdmissionDecision::Admitted { .. }));
    }

    #[test]
    fn recovered_retry_publication_failure_rolls_back_work_and_ready_transition() {
        let fixture = Fixture::new();
        let original = fixture.effect(9);
        let mut live = fixture.coordinator(64);
        let mut live_registry = LifecycleWorkRegistryHolder::empty();
        let (effect, pending) = fixture.pair(original.clone(), 102);
        assert!(matches!(
            live.admit_concrete_adapter_effect(
                &mut live_registry,
                &fixture.verified,
                effect,
                pending,
            ),
            AdapterEffectAdmissionTransaction::Admitted(AdmissionDecision::Admitted {
                ordinal: 1,
                ..
            })
        ));
        let snapshot = recovery_snapshot(&live);

        let mut recovered = LifecycleCoordinator::new(
            fixture.active_context(),
            live.high_water,
            super::super::schema::CapacityGeometry::new([
                (super::super::CapacityClass::Consensus, 64),
                (super::super::CapacityClass::Effect, 64),
                (super::super::CapacityClass::Serve, 64),
                (super::super::CapacityClass::Producer, 64),
            ]),
        );
        recovered.reconcile_restart(snapshot);
        let root = TempDir::new().expect("temporary recovered-rebind ledger");
        recovered
            .attach_empty_test_ledger(root.path())
            .expect("persist recovered coordinator before rebind");
        recovered.redirect_test_ledger_to_missing_parent(root.path());
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let (effect, pending) = fixture.pair(original.clone(), 103);

        let outcome = recovered.admit_concrete_adapter_effect(
            &mut registry,
            &fixture.verified,
            effect,
            pending,
        );
        let AdapterEffectAdmissionTransaction::Failed {
            failure: AdapterEffectAdmissionFailure::Durability,
            effect,
            pending,
        } = outcome
        else {
            panic!("failed recovered rebind publication must return exact work")
        };
        assert_eq!(effect, original);
        assert!(pending.exactly_binds_adapter_effect(&effect));
        assert!(registry.registry.is_empty());
        assert_eq!(recovered.high_water, 1);
        assert!(matches!(
            recovered.records[&1].state,
            LifecycleState::Waiting(super::super::WaitToken {
                source: WaitSource::Recovery(_),
                ..
            })
        ));
        assert!(recovered.records[&1].physical_slots.is_empty());
        assert_eq!(recovered.fault(), Some(CoordinatorFault::DurabilityFailure));
    }

    #[test]
    fn durable_publication_failure_rolls_back_registry_and_logical_admission() {
        let fixture = Fixture::new();
        let root = TempDir::new().expect("temporary concrete-admission ledger");
        let mut coordinator = fixture.coordinator(64);
        coordinator
            .attach_empty_test_ledger(root.path())
            .expect("attach empty lifecycle ledger");
        coordinator.redirect_test_ledger_to_missing_parent(root.path());
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let original = fixture.effect(6);
        let (effect, pending) = fixture.pair(original.clone(), 98);

        let outcome = coordinator.admit_concrete_adapter_effect(
            &mut registry,
            &fixture.verified,
            effect,
            pending,
        );
        let AdapterEffectAdmissionTransaction::Failed {
            failure: AdapterEffectAdmissionFailure::Durability,
            effect,
            pending,
        } = outcome
        else {
            panic!("failed durable publication must return the rolled-back pair")
        };
        assert_eq!(effect, original);
        assert!(pending.exactly_binds_adapter_effect(&effect));
        assert_eq!(coordinator.high_water(), 0);
        assert!(coordinator.records.is_empty());
        assert!(coordinator.key_index.is_empty());
        assert!(coordinator.owner_index.is_empty());
        assert!(registry.registry.is_empty());
        assert_eq!(
            coordinator.fault(),
            Some(CoordinatorFault::DurabilityFailure)
        );
    }

    #[test]
    fn projection_failure_returns_the_unmodified_pair() {
        let fixture = Fixture::new();
        let foreign_context = super::super::LifecycleContext::new(
            LifecycleDigest::new([0xFF; 32]),
            fixture.context.height,
        );
        let mut coordinator = LifecycleCoordinator::new(
            foreign_context,
            0,
            super::super::schema::CapacityGeometry::new(
                super::super::CapacityClass::ALL.map(|class| (class, 64)),
            ),
        );
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let original = fixture.effect(7);
        let (effect, pending) = fixture.pair(original.clone(), 99);

        let outcome = coordinator.admit_concrete_adapter_effect(
            &mut registry,
            &fixture.verified,
            effect,
            pending,
        );
        let AdapterEffectAdmissionTransaction::Failed {
            failure:
                AdapterEffectAdmissionFailure::Projection(AdapterEffectAdmissionError::ForeignContext),
            effect,
            pending,
        } = outcome
        else {
            panic!("foreign projection must return the exact input pair")
        };
        assert_eq!(effect, original);
        assert!(pending.exactly_binds_adapter_effect(&effect));
        assert_eq!(coordinator.high_water(), 0);
        assert!(coordinator.records.is_empty());
        assert!(registry.registry.is_empty());
    }
}
