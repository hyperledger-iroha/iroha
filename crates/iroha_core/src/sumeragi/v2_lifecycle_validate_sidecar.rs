//! Exact lifecycle ownership for durable Validate merge-sidecar deferral.

use super::{
    CapacityClass, CausalRoot, LifecycleContext, LifecycleCoordinator, LifecycleDigest,
    LifecycleKey, LifecyclePhase, LifecycleStage, LifecycleStageKind, LifecycleState,
    LifecycleValidateDispatchKeyV1, LifecycleWorkClass, OwnerId, PhysicalSlotId, PredecessorScope,
    ReadyEvent, ReadyValidateSuccessorV1, TerminalOutcome, WaitSource, WaitToken,
    concrete_admission::LifecycleWorkRegistryHolder,
    ledger::{LifecycleLedgerError, LifecycleLedgerStoreV1},
};
use crate::sumeragi::{
    v2_lane_work::{MergeSidecarDeferralDisposition, V2LaneWorkAdapter},
    v2_worker::PreparedDeferredLifecycleValidateCompletionV1,
};
use iroha_crypto::HashOf;
use iroha_data_model::block::{CertifiedMergeLedgerReference, consensus_v2 as wire};
use norito::codec::{Decode, Encode};
use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    path::Path,
};
use thiserror::Error;

const REGISTRATION_VERSION_V1: u8 = 1;
const MAX_REGISTRATION_BYTES: u64 = 1024 * 1024;

/// Complete immutable identity of one lifecycle-owned sidecar wait.
///
/// Construction is restricted to the sealed executed Validate dispatch or an
/// integrity-checked cold-open record joined back to its exact registry row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) struct LifecycleValidateSidecarRegistrationIdentityV1 {
    dispatch_key: LifecycleValidateDispatchKeyV1,
    lifecycle_key: LifecycleKey,
    lifecycle_stage: LifecycleStage,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    wait_token: WaitToken,
    reference: CertifiedMergeLedgerReference,
}

impl LifecycleValidateSidecarRegistrationIdentityV1 {
    /// Seal a live registration directly from one executed Validate dispatch.
    pub(super) fn from_sealed_dispatch(
        dispatch_key: LifecycleValidateDispatchKeyV1,
        lifecycle_key: LifecycleKey,
        lifecycle_stage: LifecycleStage,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        wait_token: WaitToken,
        reference: CertifiedMergeLedgerReference,
    ) -> Option<Self> {
        let identity = Self {
            dispatch_key,
            lifecycle_key,
            lifecycle_stage,
            round,
            subject,
            wait_token,
            reference,
        };
        identity.is_structurally_exact().then_some(identity)
    }

    pub(super) const fn dispatch_key(&self) -> LifecycleValidateDispatchKeyV1 {
        self.dispatch_key
    }

    pub(super) const fn lifecycle_key(&self) -> LifecycleKey {
        self.lifecycle_key
    }

    pub(super) const fn lifecycle_stage(&self) -> LifecycleStage {
        self.lifecycle_stage
    }

    pub(in crate::sumeragi) const fn round(&self) -> wire::ConsensusRound {
        self.round
    }

    pub(in crate::sumeragi) const fn subject(&self) -> wire::BlockSubject {
        self.subject
    }

    pub(super) const fn wait_token(&self) -> WaitToken {
        self.wait_token
    }

    pub(in crate::sumeragi) const fn reference(&self) -> &CertifiedMergeLedgerReference {
        &self.reference
    }

    fn is_structurally_exact(&self) -> bool {
        let key = self.dispatch_key;
        let Some(parent_hash) = self.subject.parent_block_hash else {
            return false;
        };
        key.matches_consensus_round(&self.round)
            && self.lifecycle_key.context().as_bytes() == self.round.context_id.0.as_ref()
            && self.lifecycle_key.round().height() == self.round.height
            && self.lifecycle_key.phase() == LifecyclePhase::Validate
            && self.lifecycle_stage.kind() == LifecycleStageKind::ValidateBody
            && self.lifecycle_stage.predecessor_scope() == PredecessorScope::Independent
            && key.lifecycle_ordinal() != 0
            && key.owner().first_admission_ordinal() != 0
            && key.owner().first_admission_ordinal() <= key.lifecycle_ordinal()
            && key.slot().capacity_class() == Some(LifecycleWorkClass::Validate.capacity_class())
            && matches!(self.wait_token.source(), WaitSource::External(_))
            && self.wait_token.observed_generation() != u64::MAX
            && self.reference.version == 1
            && self.reference.merge_qc.carrier_height == self.round.height
            && self.reference.merge_qc.carrier_parent_hash == parent_hash
            && self.reference.merge_qc.view <= self.round.view
    }

    fn matches_context(&self, context: LifecycleContext) -> bool {
        self.is_structurally_exact()
            && self.lifecycle_key.context() == context.id()
            && self.lifecycle_key.round().height() == context.height()
    }
}

/// Durable registration or exact-wake failure.
#[derive(Debug, Error)]
pub(in crate::sumeragi) enum LifecycleValidateSidecarRegistrationErrorV1 {
    /// Sealed dispatch or recovered fields do not match one exact Validate row.
    #[error("Validate sidecar registration identity is not exact")]
    InvalidIdentity,
    /// The lifecycle registration file failed its crash-safe storage contract.
    #[error("Validate sidecar registration persistence failed: {0}")]
    Persistence(String),
    /// Sidecar transport rejected or could not service the exact registration.
    #[error("Validate sidecar registration service failed: {0}")]
    Service(String),
}

enum LifecycleValidateSidecarCustodyV1 {
    Live(PreparedDeferredLifecycleValidateCompletionV1),
    Recovered,
}

/// One fsynced sidecar registration retaining its live move-only dispatch, or
/// the equivalent cold-open registration before the exact body is retried.
#[must_use = "a registered Validate sidecar wait must remain parked or wake its exact row"]
pub(in crate::sumeragi) struct RegisteredLifecycleValidateSidecarWaitV1 {
    identity: LifecycleValidateSidecarRegistrationIdentityV1,
    custody: LifecycleValidateSidecarCustodyV1,
}

/// Result of polling one registered lifecycle Validate sidecar dependency.
#[must_use = "sidecar progress must remain parked or update the lifecycle driver"]
pub(in crate::sumeragi) enum LifecycleValidateSidecarDriveV1 {
    /// The exact dependency is still fetching or awaiting bounded capacity.
    Waiting(RegisteredLifecycleValidateSidecarWaitV1),
    /// The exact dependency became durable and the same row is Ready.
    Woken(ReadyValidateSuccessorV1),
    /// A certified newer view cancelled this unprotected losing proposal.
    Superseded {
        /// Exact lifecycle ordinal durably terminalized by the cancellation.
        ordinal: u128,
    },
    /// The owner failed closed; dropping it arms the existing restart guard.
    RestartRequired(LifecycleValidateSidecarRegistrationErrorV1),
}

impl RegisteredLifecycleValidateSidecarWaitV1 {
    /// Fsync a live deferred dispatch before any sidecar transport ownership is
    /// acquired. Failure returns the complete guarded completion unchanged.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn register_live(
        coordinator: &LifecycleCoordinator,
        registry: &LifecycleWorkRegistryHolder,
        completion: PreparedDeferredLifecycleValidateCompletionV1,
    ) -> Result<
        Self,
        (
            LifecycleValidateSidecarRegistrationErrorV1,
            PreparedDeferredLifecycleValidateCompletionV1,
        ),
    > {
        let Some(identity) = completion.sidecar_registration_identity() else {
            return Err((
                LifecycleValidateSidecarRegistrationErrorV1::InvalidIdentity,
                completion,
            ));
        };
        if !coordinator.validate_sidecar_wait_matches(&identity, registry) {
            return Err((
                LifecycleValidateSidecarRegistrationErrorV1::InvalidIdentity,
                completion,
            ));
        }
        if let Err(error) = coordinator.persist_validate_sidecar_registration(&identity) {
            return Err((error, completion));
        }
        Ok(Self {
            identity,
            custody: LifecycleValidateSidecarCustodyV1::Live(completion),
        })
    }

    /// Reconstruct an fsynced registration and its exact Waiting generation
    /// before the live runner can select any Ready work.
    pub(in crate::sumeragi) fn recover_at_launch(
        coordinator: &mut LifecycleCoordinator,
        registry: &mut LifecycleWorkRegistryHolder,
    ) -> Result<Option<Self>, LifecycleValidateSidecarRegistrationErrorV1> {
        let Some(identity) = coordinator.load_validate_sidecar_registration()? else {
            return Ok(None);
        };
        if coordinator.cancelled_validate_sidecar_registration_matches(&identity, registry) {
            let store = coordinator.ledger_store.as_ref().ok_or_else(|| {
                LifecycleValidateSidecarRegistrationErrorV1::Persistence(
                    "cancelled lifecycle Validate sidecar has no attached LedgerV1 store"
                        .to_owned(),
                )
            })?;
            clear_registration(store, &identity)?;
            return Ok(None);
        }
        coordinator.restore_validate_sidecar_wait(&identity, registry)?;
        Ok(Some(Self {
            identity,
            custody: LifecycleValidateSidecarCustodyV1::Recovered,
        }))
    }

    /// Poll only the exact stored reference. Availability is reauthenticated
    /// against Kura by lane work before the lifecycle registration can wake.
    pub(in crate::sumeragi) fn drive(
        self,
        coordinator: &mut LifecycleCoordinator,
        registry: &mut LifecycleWorkRegistryHolder,
        lane_work: &mut V2LaneWorkAdapter,
    ) -> LifecycleValidateSidecarDriveV1 {
        if !coordinator.validate_sidecar_wait_matches(&self.identity, registry) {
            return LifecycleValidateSidecarDriveV1::RestartRequired(
                LifecycleValidateSidecarRegistrationErrorV1::InvalidIdentity,
            );
        }
        if lane_work
            .lifecycle_validate_sidecar_is_superseded(self.identity.round, self.identity.subject)
        {
            let ordinal = self.identity.dispatch_key().lifecycle_ordinal();
            if let Err(error) =
                coordinator.cancel_validate_sidecar_registration(&self.identity, registry)
            {
                return LifecycleValidateSidecarDriveV1::RestartRequired(error);
            }
            if let LifecycleValidateSidecarCustodyV1::Live(completion) = self.custody {
                let (dispatch, ack) = completion.into_sidecar_wake_parts();
                debug_assert!(dispatch.matches_dispatch_key(self.identity.dispatch_key()));
                drop(dispatch);
                ack.acknowledge_after_publication();
            }
            return LifecycleValidateSidecarDriveV1::Superseded { ordinal };
        }
        let disposition = lane_work.defer_missing_lifecycle_validate_sidecar(
            self.identity.round,
            self.identity.subject,
            self.identity.reference.clone(),
        );
        match disposition {
            Ok(
                MergeSidecarDeferralDisposition::Fetching
                | MergeSidecarDeferralDisposition::RetryLater,
            ) => LifecycleValidateSidecarDriveV1::Waiting(self),
            Ok(MergeSidecarDeferralDisposition::Available) => {
                if let Err(error) =
                    coordinator.wake_validate_sidecar_registration(&self.identity, registry)
                {
                    return LifecycleValidateSidecarDriveV1::RestartRequired(error);
                }
                let dispatch_key = self.identity.dispatch_key();
                let attestation = match coordinator
                    .attest_ready_validate_demand(registry, dispatch_key.lifecycle_ordinal())
                {
                    Ok(attestation) => attestation,
                    Err(_) => {
                        return LifecycleValidateSidecarDriveV1::RestartRequired(
                            LifecycleValidateSidecarRegistrationErrorV1::InvalidIdentity,
                        );
                    }
                };
                let Some(successor) = ReadyValidateSuccessorV1::from_sidecar_wake(
                    dispatch_key,
                    self.identity.round(),
                    self.identity.subject(),
                    attestation,
                ) else {
                    return LifecycleValidateSidecarDriveV1::RestartRequired(
                        LifecycleValidateSidecarRegistrationErrorV1::InvalidIdentity,
                    );
                };
                if let LifecycleValidateSidecarCustodyV1::Live(completion) = self.custody {
                    let (dispatch, ack) = completion.into_sidecar_wake_parts();
                    debug_assert!(dispatch.matches_dispatch_key(dispatch_key));
                    drop(dispatch);
                    ack.acknowledge_after_publication();
                }
                LifecycleValidateSidecarDriveV1::Woken(successor)
            }
            Ok(MergeSidecarDeferralDisposition::Rejected(reason)) => {
                LifecycleValidateSidecarDriveV1::RestartRequired(
                    LifecycleValidateSidecarRegistrationErrorV1::Service(reason),
                )
            }
            Err(error) => LifecycleValidateSidecarDriveV1::RestartRequired(
                LifecycleValidateSidecarRegistrationErrorV1::Service(error.to_string()),
            ),
        }
    }
}

impl LifecycleCoordinator {
    fn cancelled_validate_sidecar_registration_matches(
        &self,
        identity: &LifecycleValidateSidecarRegistrationIdentityV1,
        registry: &LifecycleWorkRegistryHolder,
    ) -> bool {
        let key = identity.dispatch_key;
        let Some(record) = self.records.get(&key.lifecycle_ordinal()) else {
            return false;
        };
        identity.matches_context(self.active_context)
            && self.fault.is_none()
            && self.active_lease.is_none()
            && record.ordinal == key.lifecycle_ordinal()
            && record.owner == key.owner()
            && record.key == identity.lifecycle_key
            && record.stage == identity.lifecycle_stage
            && record.work_class == LifecycleWorkClass::Validate
            && record.state == LifecycleState::Terminal(TerminalOutcome::Cancelled)
            && record.physical_slots.len() == 1
            && record.physical_slots.get(&key.slot()) == Some(&key.digest())
            && self.key_index.get(&record.key) == Some(&record.ordinal)
            && self.owner_index.get(&record.owner.causal_root()) == Some(&record.owner)
            && !self.ready_index.contains(&record.ordinal)
            && registry
                .registry()
                .lacks_validate_sidecar_registration(identity)
    }

    fn validate_sidecar_wait_matches(
        &self,
        identity: &LifecycleValidateSidecarRegistrationIdentityV1,
        registry: &LifecycleWorkRegistryHolder,
    ) -> bool {
        let key = identity.dispatch_key;
        let Some(record) = self.records.get(&key.lifecycle_ordinal()) else {
            return false;
        };
        identity.matches_context(self.active_context)
            && self.fault.is_none()
            && self.active_lease.is_none()
            && record.ordinal == key.lifecycle_ordinal()
            && record.owner == key.owner()
            && record.key == identity.lifecycle_key
            && record.stage == identity.lifecycle_stage
            && record.work_class == LifecycleWorkClass::Validate
            && record.state == LifecycleState::Waiting(identity.wait_token)
            && record.physical_slots.len() == 1
            && record.physical_slots.get(&key.slot()) == Some(&key.digest())
            && self.key_index.get(&record.key) == Some(&record.ordinal)
            && self.owner_index.get(&record.owner.causal_root()) == Some(&record.owner)
            && !self.ready_index.contains(&record.ordinal)
            && self.observed_generation.get(&identity.wait_token.source())
                == Some(&identity.wait_token.observed_generation())
            && self.records.iter().all(|(ordinal, candidate)| {
                *ordinal == record.ordinal
                    || !matches!(
                        candidate.state,
                        LifecycleState::Waiting(wait)
                            if wait.source() == identity.wait_token.source()
                    )
            })
            && registry
                .registry()
                .exactly_matches_validate_sidecar_registration(identity)
    }

    fn recovered_ready_matches_sidecar_registration(
        &self,
        identity: &LifecycleValidateSidecarRegistrationIdentityV1,
        registry: &LifecycleWorkRegistryHolder,
    ) -> bool {
        let key = identity.dispatch_key;
        let Some(record) = self.records.get(&key.lifecycle_ordinal()) else {
            return false;
        };
        identity.matches_context(self.active_context)
            && self.fault.is_none()
            && self.active_lease.is_none()
            && record.ordinal == key.lifecycle_ordinal()
            && record.owner == key.owner()
            && record.key == identity.lifecycle_key
            && record.stage == identity.lifecycle_stage
            && record.work_class == LifecycleWorkClass::Validate
            && record.state == LifecycleState::Ready
            && record.physical_slots.len() == 1
            && record.physical_slots.get(&key.slot()) == Some(&key.digest())
            && self.key_index.get(&record.key) == Some(&record.ordinal)
            && self.owner_index.get(&record.owner.causal_root()) == Some(&record.owner)
            && self.ready_index.contains(&record.ordinal)
            && self
                .observed_generation
                .get(&identity.wait_token.source())
                .is_none_or(|generation| *generation == identity.wait_token.observed_generation())
            && self.records.values().all(|candidate| {
                !matches!(
                    candidate.state,
                    LifecycleState::Waiting(wait)
                        if wait.source() == identity.wait_token.source()
                )
            })
            && registry
                .registry()
                .exactly_matches_validate_sidecar_registration(identity)
    }

    fn restore_validate_sidecar_wait(
        &mut self,
        identity: &LifecycleValidateSidecarRegistrationIdentityV1,
        registry: &LifecycleWorkRegistryHolder,
    ) -> Result<(), LifecycleValidateSidecarRegistrationErrorV1> {
        if !self.recovered_ready_matches_sidecar_registration(identity, registry) {
            return Err(LifecycleValidateSidecarRegistrationErrorV1::InvalidIdentity);
        }
        let ordinal = identity.dispatch_key.lifecycle_ordinal();
        self.records
            .get_mut(&ordinal)
            .expect("recovered registration retained its exact Ready row")
            .state = LifecycleState::Waiting(identity.wait_token);
        assert!(self.ready_index.remove(&ordinal));
        self.observed_generation.insert(
            identity.wait_token.source(),
            identity.wait_token.observed_generation(),
        );
        if !self.validate_sidecar_wait_matches(identity, registry) {
            self.fault = Some(super::CoordinatorFault::RecoveryRejected);
            return Err(LifecycleValidateSidecarRegistrationErrorV1::InvalidIdentity);
        }
        Ok(())
    }

    fn persist_validate_sidecar_registration(
        &self,
        identity: &LifecycleValidateSidecarRegistrationIdentityV1,
    ) -> Result<(), LifecycleValidateSidecarRegistrationErrorV1> {
        let store = self.ledger_store.as_ref().ok_or_else(|| {
            LifecycleValidateSidecarRegistrationErrorV1::Persistence(
                "live lifecycle owner has no attached LedgerV1 store".to_owned(),
            )
        })?;
        persist_registration(store, identity)
    }

    fn load_validate_sidecar_registration(
        &self,
    ) -> Result<
        Option<LifecycleValidateSidecarRegistrationIdentityV1>,
        LifecycleValidateSidecarRegistrationErrorV1,
    > {
        let store = self.ledger_store.as_ref().ok_or_else(|| {
            LifecycleValidateSidecarRegistrationErrorV1::Persistence(
                "cold lifecycle owner has no attached LedgerV1 store".to_owned(),
            )
        })?;
        load_registration(store, self)
    }

    fn wake_validate_sidecar_registration(
        &mut self,
        identity: &LifecycleValidateSidecarRegistrationIdentityV1,
        registry: &LifecycleWorkRegistryHolder,
    ) -> Result<(), LifecycleValidateSidecarRegistrationErrorV1> {
        if !self.validate_sidecar_wait_matches(identity, registry) {
            return Err(LifecycleValidateSidecarRegistrationErrorV1::InvalidIdentity);
        }
        let mut next = self.stage_durable_transaction();
        next.publish_ready(ReadyEvent::new(
            identity.dispatch_key.lifecycle_ordinal(),
            identity.dispatch_key.owner(),
            identity.wait_token,
            None,
        ));
        if !sidecar_wake_transition_is_exact(self, &next, identity) {
            return Err(LifecycleValidateSidecarRegistrationErrorV1::InvalidIdentity);
        }
        let store = self.ledger_store.as_ref().ok_or_else(|| {
            LifecycleValidateSidecarRegistrationErrorV1::Persistence(
                "live lifecycle owner has no attached LedgerV1 store".to_owned(),
            )
        })?;
        clear_registration(store, identity)?;
        *self = next;
        Ok(())
    }

    fn cancel_validate_sidecar_registration(
        &mut self,
        identity: &LifecycleValidateSidecarRegistrationIdentityV1,
        registry: &mut LifecycleWorkRegistryHolder,
    ) -> Result<(), LifecycleValidateSidecarRegistrationErrorV1> {
        if !self.validate_sidecar_wait_matches(identity, registry) {
            return Err(LifecycleValidateSidecarRegistrationErrorV1::InvalidIdentity);
        }
        let store = self.ledger_store.clone().ok_or_else(|| {
            LifecycleValidateSidecarRegistrationErrorV1::Persistence(
                "live lifecycle owner has no attached LedgerV1 store".to_owned(),
            )
        })?;
        let mut next = self.stage_durable_transaction();
        next.finish_terminal(
            identity.dispatch_key().lifecycle_ordinal(),
            TerminalOutcome::Cancelled,
        )
        .map_err(|_| LifecycleValidateSidecarRegistrationErrorV1::InvalidIdentity)?;
        self.persist_exact_staged_successor(&next)
            .map_err(map_ledger_error)?;
        let retired = registry
            .registry_mut()
            .retire_validate_sidecar_registration(identity);
        debug_assert!(
            retired,
            "preflighted Validate sidecar carrier remains exact"
        );
        *self = next;
        if !retired {
            self.fault = Some(super::CoordinatorFault::DurabilityFailure);
            return Err(LifecycleValidateSidecarRegistrationErrorV1::InvalidIdentity);
        }
        clear_registration(&store, identity)
    }
}

fn sidecar_wake_transition_is_exact(
    current: &LifecycleCoordinator,
    next: &LifecycleCoordinator,
    identity: &LifecycleValidateSidecarRegistrationIdentityV1,
) -> bool {
    let ordinal = identity.dispatch_key.lifecycle_ordinal();
    let Some(next_generation) = identity.wait_token.observed_generation().checked_add(1) else {
        return false;
    };
    let mut expected_record = current
        .records
        .get(&ordinal)
        .expect("sidecar wake preflight retained its exact row")
        .clone();
    expected_record.state = LifecycleState::Ready;
    let mut expected_ready = current.ready_index.clone();
    expected_ready.insert(ordinal);
    let mut expected_observed = current.observed_generation.clone();
    expected_observed.insert(identity.wait_token.source(), next_generation);
    next.episode_authority == current.episode_authority
        && next.active_context == current.active_context
        && next.records.len() == current.records.len()
        && next.records.get(&ordinal) == Some(&expected_record)
        && current
            .records
            .iter()
            .all(|(other, record)| *other == ordinal || next.records.get(other) == Some(record))
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

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct DurableValidateSidecarRegistrationV1 {
    version: u8,
    context: [u8; 32],
    height: u64,
    owner_causal_root: [u8; 32],
    owner_first_admission_ordinal: u128,
    ordinal: u128,
    slot_class: u16,
    slot_index: u16,
    digest: [u8; 32],
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    wait_source: [u8; 32],
    wait_generation: u64,
    reference: CertifiedMergeLedgerReference,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct DurableValidateSidecarRegistrationFrameV1 {
    registration: DurableValidateSidecarRegistrationV1,
    registration_hash: HashOf<DurableValidateSidecarRegistrationV1>,
}

impl DurableValidateSidecarRegistrationV1 {
    fn from_identity(identity: &LifecycleValidateSidecarRegistrationIdentityV1) -> Self {
        let WaitSource::External(wait_source) = identity.wait_token.source() else {
            unreachable!("validated sidecar registration retains an external wait")
        };
        let key = identity.dispatch_key;
        let slot_class = match key
            .slot()
            .capacity_class()
            .expect("validated sidecar registration retains a typed slot")
        {
            CapacityClass::Consensus => 0,
            CapacityClass::Effect => 1,
            CapacityClass::Serve => 2,
            CapacityClass::Producer => 3,
        };
        Self {
            version: REGISTRATION_VERSION_V1,
            context: *identity.lifecycle_key.context().as_bytes(),
            height: identity.round.height,
            owner_causal_root: *key.owner().causal_root().digest().as_bytes(),
            owner_first_admission_ordinal: key.owner().first_admission_ordinal(),
            ordinal: key.lifecycle_ordinal(),
            slot_class,
            slot_index: key.slot().index(),
            digest: *key.digest().as_bytes(),
            round: identity.round,
            subject: identity.subject,
            wait_source: *wait_source.as_bytes(),
            wait_generation: identity.wait_token.observed_generation(),
            reference: identity.reference.clone(),
        }
    }

    fn into_identity(
        self,
        coordinator: &LifecycleCoordinator,
    ) -> Option<LifecycleValidateSidecarRegistrationIdentityV1> {
        if self.version != REGISTRATION_VERSION_V1 {
            return None;
        }
        let context = LifecycleDigest::new(self.context);
        let owner = OwnerId::new(
            CausalRoot::new(LifecycleDigest::new(self.owner_causal_root)),
            self.owner_first_admission_ordinal,
        );
        let capacity_class = match self.slot_class {
            0 => CapacityClass::Consensus,
            1 => CapacityClass::Effect,
            2 => CapacityClass::Serve,
            3 => CapacityClass::Producer,
            _ => return None,
        };
        let slot = PhysicalSlotId::for_capacity(capacity_class, self.slot_index);
        let record = coordinator.records.get(&self.ordinal)?;
        let dispatch_key = LifecycleValidateDispatchKeyV1::from_recovered_validate_registration(
            context,
            self.height,
            owner,
            self.ordinal,
            slot,
            LifecycleDigest::new(self.digest),
        )?;
        LifecycleValidateSidecarRegistrationIdentityV1::from_sealed_dispatch(
            dispatch_key,
            record.key,
            record.stage,
            self.round,
            self.subject,
            WaitToken::new(
                WaitSource::External(LifecycleDigest::new(self.wait_source)),
                self.wait_generation,
            ),
            self.reference,
        )
    }
}

impl DurableValidateSidecarRegistrationFrameV1 {
    fn new(identity: &LifecycleValidateSidecarRegistrationIdentityV1) -> Self {
        let registration = DurableValidateSidecarRegistrationV1::from_identity(identity);
        let registration_hash = HashOf::new(&registration);
        Self {
            registration,
            registration_hash,
        }
    }

    fn into_identity(
        self,
        coordinator: &LifecycleCoordinator,
    ) -> Option<LifecycleValidateSidecarRegistrationIdentityV1> {
        (self.registration_hash == HashOf::new(&self.registration))
            .then(|| self.registration.into_identity(coordinator))?
    }
}

fn persist_registration(
    store: &LifecycleLedgerStoreV1,
    identity: &LifecycleValidateSidecarRegistrationIdentityV1,
) -> Result<(), LifecycleValidateSidecarRegistrationErrorV1> {
    if store.lifecycle_context().id() != identity.lifecycle_key.context()
        || store.lifecycle_context().height() != identity.round.height
    {
        return Err(LifecycleValidateSidecarRegistrationErrorV1::InvalidIdentity);
    }
    let path = registration_path(store)?;
    cleanup_registration_temporary(&path)?;
    if path_exists(&path)? {
        let existing = read_frame(&path)?;
        let expected = DurableValidateSidecarRegistrationFrameV1::new(identity);
        return (existing == expected).then_some(()).ok_or(
            LifecycleValidateSidecarRegistrationErrorV1::Persistence(
                "a foreign Validate sidecar registration already owns this height".to_owned(),
            ),
        );
    }
    let frame = DurableValidateSidecarRegistrationFrameV1::new(identity);
    let bytes = norito::to_bytes(&frame).map_err(|error| {
        LifecycleValidateSidecarRegistrationErrorV1::Persistence(error.to_string())
    })?;
    if bytes.is_empty() || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_REGISTRATION_BYTES {
        return Err(LifecycleValidateSidecarRegistrationErrorV1::Persistence(
            "Validate sidecar registration exceeds its byte bound".to_owned(),
        ));
    }
    let temporary = registration_temporary_path(&path);
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| persistence_io("create registration temporary", &temporary, error))?;
    file.write_all(&bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .map_err(|error| persistence_io("sync registration temporary", &temporary, error))?;
    fs::rename(&temporary, &path)
        .map_err(|error| persistence_io("publish registration", &path, error))?;
    sync_parent(&path)?;
    Ok(())
}

fn load_registration(
    store: &LifecycleLedgerStoreV1,
    coordinator: &LifecycleCoordinator,
) -> Result<
    Option<LifecycleValidateSidecarRegistrationIdentityV1>,
    LifecycleValidateSidecarRegistrationErrorV1,
> {
    let path = registration_path(store)?;
    cleanup_registration_temporary(&path)?;
    if !path_exists(&path)? {
        return Ok(None);
    }
    let frame = read_frame(&path)?;
    let identity = frame.into_identity(coordinator).ok_or(
        LifecycleValidateSidecarRegistrationErrorV1::Persistence(
            "Validate sidecar registration failed integrity or identity decoding".to_owned(),
        ),
    )?;
    if store.lifecycle_context().id() != identity.lifecycle_key.context()
        || store.lifecycle_context().height() != identity.round.height
    {
        return Err(LifecycleValidateSidecarRegistrationErrorV1::InvalidIdentity);
    }
    Ok(Some(identity))
}

fn clear_registration(
    store: &LifecycleLedgerStoreV1,
    identity: &LifecycleValidateSidecarRegistrationIdentityV1,
) -> Result<(), LifecycleValidateSidecarRegistrationErrorV1> {
    let path = registration_path(store)?;
    cleanup_registration_temporary(&path)?;
    let existing = read_frame(&path)?;
    if existing != DurableValidateSidecarRegistrationFrameV1::new(identity) {
        return Err(LifecycleValidateSidecarRegistrationErrorV1::Persistence(
            "Validate sidecar wake cannot retire a foreign registration".to_owned(),
        ));
    }
    fs::remove_file(&path).map_err(|error| persistence_io("remove registration", &path, error))?;
    sync_parent(&path)
}

fn read_frame(
    path: &Path,
) -> Result<DurableValidateSidecarRegistrationFrameV1, LifecycleValidateSidecarRegistrationErrorV1>
{
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| persistence_io("inspect registration", path, error))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_REGISTRATION_BYTES
    {
        return Err(LifecycleValidateSidecarRegistrationErrorV1::Persistence(
            "Validate sidecar registration is not one bounded regular file".to_owned(),
        ));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    File::open(path)
        .and_then(|file| {
            file.take(MAX_REGISTRATION_BYTES.saturating_add(1))
                .read_to_end(&mut bytes)
        })
        .map_err(|error| persistence_io("read registration", path, error))?;
    if bytes.is_empty() || u64::try_from(bytes.len()).unwrap_or(u64::MAX) != metadata.len() {
        return Err(LifecycleValidateSidecarRegistrationErrorV1::Persistence(
            "Validate sidecar registration changed during bounded read".to_owned(),
        ));
    }
    norito::decode_from_bytes(&bytes).map_err(|error| {
        LifecycleValidateSidecarRegistrationErrorV1::Persistence(error.to_string())
    })
}

fn registration_path(
    store: &LifecycleLedgerStoreV1,
) -> Result<std::path::PathBuf, LifecycleValidateSidecarRegistrationErrorV1> {
    store
        .validate_sidecar_registration_path()
        .map_err(map_ledger_error)
}

fn registration_temporary_path(path: &Path) -> std::path::PathBuf {
    path.with_extension("norito.tmp")
}

fn cleanup_registration_temporary(
    path: &Path,
) -> Result<(), LifecycleValidateSidecarRegistrationErrorV1> {
    let temporary = registration_temporary_path(path);
    match fs::symlink_metadata(&temporary) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(LifecycleValidateSidecarRegistrationErrorV1::Persistence(
                "Validate sidecar registration temporary is not a regular file".to_owned(),
            ))
        }
        Ok(_) => {
            fs::remove_file(&temporary).map_err(|error| {
                persistence_io("remove registration temporary", &temporary, error)
            })?;
            sync_parent(path)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(persistence_io(
            "inspect registration temporary",
            &temporary,
            error,
        )),
    }
}

fn path_exists(path: &Path) -> Result<bool, LifecycleValidateSidecarRegistrationErrorV1> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(persistence_io("inspect registration path", path, error)),
    }
}

fn sync_parent(path: &Path) -> Result<(), LifecycleValidateSidecarRegistrationErrorV1> {
    let parent = path.parent().ok_or_else(|| {
        LifecycleValidateSidecarRegistrationErrorV1::Persistence(
            "Validate sidecar registration path has no parent".to_owned(),
        )
    })?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| persistence_io("sync registration directory", parent, error))
}

fn persistence_io(
    operation: &str,
    path: &Path,
    error: std::io::Error,
) -> LifecycleValidateSidecarRegistrationErrorV1 {
    LifecycleValidateSidecarRegistrationErrorV1::Persistence(format!(
        "{operation} {}: {error}",
        path.display()
    ))
}

fn map_ledger_error(error: LifecycleLedgerError) -> LifecycleValidateSidecarRegistrationErrorV1 {
    LifecycleValidateSidecarRegistrationErrorV1::Persistence(error.to_string())
}

#[cfg(test)]
pub(super) fn persist_registration_for_test(
    coordinator: &LifecycleCoordinator,
    identity: &LifecycleValidateSidecarRegistrationIdentityV1,
) -> Result<(), LifecycleValidateSidecarRegistrationErrorV1> {
    coordinator.persist_validate_sidecar_registration(identity)
}

#[cfg(test)]
pub(super) fn load_registration_for_test(
    coordinator: &LifecycleCoordinator,
) -> Result<
    Option<LifecycleValidateSidecarRegistrationIdentityV1>,
    LifecycleValidateSidecarRegistrationErrorV1,
> {
    coordinator.load_validate_sidecar_registration()
}

#[cfg(test)]
pub(super) fn wake_registration_for_test(
    coordinator: &mut LifecycleCoordinator,
    identity: &LifecycleValidateSidecarRegistrationIdentityV1,
    registry: &LifecycleWorkRegistryHolder,
) -> Result<(), LifecycleValidateSidecarRegistrationErrorV1> {
    coordinator.wake_validate_sidecar_registration(identity, registry)
}

#[cfg(test)]
pub(super) fn cancel_registration_for_test(
    coordinator: &mut LifecycleCoordinator,
    identity: &LifecycleValidateSidecarRegistrationIdentityV1,
    registry: &mut LifecycleWorkRegistryHolder,
) -> Result<(), LifecycleValidateSidecarRegistrationErrorV1> {
    coordinator.cancel_validate_sidecar_registration(identity, registry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sumeragi::v2_lifecycle_coordinator::LifecycleRound;
    use iroha_crypto::Hash;
    use iroha_data_model::{
        block::{
            BlockHeader,
            consensus_v2::{HeightContext, HeightContextId},
        },
        merge::{MergeLedgerEntry, MergeQuorumCertificate},
        peer::PeerId,
    };
    use tempfile::TempDir;

    fn identity_fixture() -> LifecycleValidateSidecarRegistrationIdentityV1 {
        let context_hash = Hash::new(b"lifecycle Validate sidecar context");
        let context_id = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
            context_hash,
        ));
        let context = LifecycleDigest::new((*context_id.0.as_ref()).into());
        let parent = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"lifecycle Validate sidecar parent",
        ));
        let round = wire::ConsensusRound {
            context_id,
            height: 7,
            view: 3,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: Some(parent),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"lifecycle Validate sidecar block",
            )),
            payload_hash: Hash::new(b"lifecycle Validate sidecar payload"),
        };
        let reference = CertifiedMergeLedgerReference {
            version: 1,
            entry_hash: HashOf::<MergeLedgerEntry>::from_untyped_unchecked(Hash::new(
                b"lifecycle Validate sidecar entry",
            )),
            encoded_len: 512,
            epoch_id: 9,
            execution_batch_hash: None,
            entrypoint_count: None,
            entrypoint_merkle_root: None,
            result_merkle_root: None,
            base_state_height: None,
            base_state_hash: None,
            merge_qc: MergeQuorumCertificate::new(
                round.view,
                9,
                round.height,
                parent,
                crate::sumeragi::synthetic_network_id("lifecycle-validate-sidecar"),
                1,
                HashOf::new(&Vec::<PeerId>::new()),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Hash::new(b"lifecycle Validate sidecar certificate"),
            ),
        };
        let lifecycle_key = LifecycleKey::new(
            context,
            LifecycleRound::new(round.height, round.view),
            Some(LifecycleRound::new(round.height, round.view)),
            Some(LifecycleDigest::new((*subject.block_hash.as_ref()).into())),
            LifecyclePhase::Validate,
            None,
        );
        let dispatch_key = LifecycleValidateDispatchKeyV1::from_recovered_validate_registration(
            context,
            round.height,
            OwnerId::new(CausalRoot::new(LifecycleDigest::new([0x41; 32])), 4),
            9,
            PhysicalSlotId::for_capacity(LifecycleWorkClass::Validate.capacity_class(), 0),
            LifecycleDigest::new([0x52; 32]),
        )
        .expect("construct exact Validate dispatch key");
        LifecycleValidateSidecarRegistrationIdentityV1::from_sealed_dispatch(
            dispatch_key,
            lifecycle_key,
            LifecycleStage::new(
                LifecycleStageKind::ValidateBody,
                PredecessorScope::Independent,
            ),
            round,
            subject,
            WaitToken::new(WaitSource::External(LifecycleDigest::new([0x63; 32])), 5),
            reference,
        )
        .expect("construct exact sidecar registration identity")
    }

    #[test]
    fn identity_rejects_foreign_context_parent_and_generation() {
        let identity = identity_fixture();

        let mut foreign_context = identity.clone();
        foreign_context.round.context_id = HeightContextId(HashOf::from_untyped_unchecked(
            Hash::new(b"foreign lifecycle sidecar context"),
        ));
        assert!(!foreign_context.is_structurally_exact());

        let mut foreign_reference = identity.clone();
        foreign_reference.reference.merge_qc.carrier_parent_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"foreign lifecycle sidecar parent"));
        assert!(!foreign_reference.is_structurally_exact());

        let mut foreign_generation = identity;
        foreign_generation.wait_token =
            WaitToken::new(foreign_generation.wait_token.source(), u64::MAX);
        assert!(!foreign_generation.is_structurally_exact());
    }

    #[test]
    fn durable_registration_is_exactly_idempotent_and_rejects_foreign_owner() {
        let identity = identity_fixture();
        let directory = TempDir::new().expect("sidecar registration directory");
        let (store, _) = LifecycleLedgerStoreV1::open(
            directory.path(),
            LifecycleContext::new(identity.lifecycle_key.context(), identity.round.height),
        )
        .expect("open lifecycle ledger store");

        persist_registration(&store, &identity).expect("persist exact registration");
        persist_registration(&store, &identity).expect("repeat registration is idempotent");

        let mut foreign = identity.clone();
        let foreign_owner = OwnerId::new(
            CausalRoot::new(LifecycleDigest::new([0x74; 32])),
            foreign.dispatch_key.owner().first_admission_ordinal(),
        );
        foreign.dispatch_key =
            LifecycleValidateDispatchKeyV1::from_recovered_validate_registration(
                foreign.lifecycle_key.context(),
                foreign.round.height,
                foreign_owner,
                foreign.dispatch_key.lifecycle_ordinal(),
                foreign.dispatch_key.slot(),
                foreign.dispatch_key.digest(),
            )
            .expect("construct foreign-owner key");
        assert!(matches!(
            persist_registration(&store, &foreign),
            Err(LifecycleValidateSidecarRegistrationErrorV1::Persistence(_))
        ));

        clear_registration(&store, &identity).expect("retire exact registration");
        assert!(
            !store
                .validate_sidecar_registration_path()
                .expect("registration path")
                .exists()
        );
    }
}
