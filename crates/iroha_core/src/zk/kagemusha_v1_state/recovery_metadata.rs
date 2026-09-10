//! Credential issuance floors and complete, non-forking recovery checkpoints.
//!
//! These projections do not confer authority. Publication requires the guard owner's atomic
//! metadata CAS and a fresh hardware selection with descriptor-owned journal verification.

use super::*;
use iroha_data_model::kagemusha::KagemushaHardwareCredentialV1;

mod current_recovery_owner_sealed {
    use super::*;

    /// Only opaque Core machines implement the public recovery-owner projection.
    pub trait Sealed {}

    impl<R, G, H> Sealed for KagemushaStateMachineV1<R, G, H>
    where
        R: KagemushaRecursiveVerifierV1,
        G: KagemushaGuardBundleVerifierV1,
        H: KagemushaAuthenticatedHistoryStoreV1,
    {
    }
}

/// Sealed access to a complete current snapshot freshly matched to its hardware checkpoint.
/// Consumers need not name Core's private history-store construction or validation traits.
pub trait KagemushaCurrentRecoveryOwnerV1: current_recovery_owner_sealed::Sealed {
    /// Borrow current checkpoint material, rejecting uncheckpointed Core or committed-history
    /// mutations. The history store may retain a separately validated local Prepare/Abort suffix.
    /// The qualified guard authenticates fresh hardware selection and actual journal material.
    /// Native open must separately authenticate the account/device possession challenge.
    fn current_recovery_selection(
        &self,
    ) -> Result<KagemushaCurrentRecoverySelectionV1<'_>, KagemushaStateErrorV1>;
}

/// Borrowed immutable owner, credential floor and exact fully checkpointed Core state.
/// No decoded projection or caller-defined implementation can construct this value.
#[derive(Clone, Copy)]
pub struct KagemushaCurrentRecoverySelectionV1<'a> {
    enrollment: &'a KagemushaRecoveryEnrollmentBindingV1,
    credential_floor: &'a KagemushaAcceptedCredentialFloorV1,
    checkpoint: &'a DurabilityAnchorV1,
}

impl<'a> KagemushaCurrentRecoverySelectionV1<'a> {
    /// Original immutable retail owner authenticated by the complete selected snapshot.
    #[must_use]
    pub fn enrollment_binding(&self) -> &'a KagemushaRecoveryEnrollmentBindingV1 {
        self.enrollment
    }

    /// Original governed credential and its independently authenticated historical release.
    #[must_use]
    pub fn accepted_credential_floor(&self) -> &'a KagemushaAcceptedCredentialFloorV1 {
        self.credential_floor
    }

    /// Actual current hardware epoch, which can be newer than the retained credential floor.
    #[must_use]
    pub fn hardware_epoch(&self) -> HardwareEpochV1 {
        self.checkpoint.statement.hardware_epoch
    }

    /// Actual current hardware key and policy selected with the complete snapshot.
    #[must_use]
    pub fn device_policy_binding(&self) -> DevicePolicyBindingV1 {
        self.checkpoint.statement.device_policy_binding
    }

    /// Original terminal checkpoint certificate for the full current machine state.
    #[must_use]
    pub fn checkpoint(&self) -> &'a DurabilityAnchorV1 {
        self.checkpoint
    }
}

impl<R, G, H> KagemushaCurrentRecoveryOwnerV1 for KagemushaStateMachineV1<R, G, H>
where
    R: KagemushaRecursiveVerifierV1,
    G: KagemushaGuardBundleVerifierV1,
    H: KagemushaAuthenticatedHistoryStoreV1,
{
    fn current_recovery_selection(
        &self,
    ) -> Result<KagemushaCurrentRecoverySelectionV1<'_>, KagemushaStateErrorV1> {
        let checkpoint = self
            .published_checkpoint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotRollback)?;
        if self
            .snapshot_with_history_checkpoint(Some(checkpoint.authenticated_history_commitment))?
            .recovery_anchor()
            != checkpoint.anchor.statement
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        self.guard_verifier
            .verify_current_recovery_checkpoint(
                &checkpoint.anchor.statement,
                &self.recovery_metadata.journals,
            )
            .map_err(KagemushaStateErrorV1::GuardRejected)?;
        // Storage can fail or change while the qualified owner completes its device exchange.
        // Never return a view if the complete local snapshot stopped matching that selection.
        if self
            .snapshot_with_history_checkpoint(Some(checkpoint.authenticated_history_commitment))?
            .recovery_anchor()
            != checkpoint.anchor.statement
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        Ok(KagemushaCurrentRecoverySelectionV1 {
            enrollment: &self.recovery_metadata.enrollment,
            credential_floor: &self.recovery_metadata.accepted_credential,
            checkpoint: &checkpoint.anchor,
        })
    }
}

/// Exact fsynced prefix of one native-owned hash-chained journal.
/// Decoded values are claims until the checkpoint guard authenticates the actual stored bytes.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::recovery_metadata::KagemushaRecoveryJournalPrefixV1"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaRecoveryJournalPrefixV1 {
    /// Number of complete frames, including the journal initialization record.
    pub sequence: u64,
    /// Hash of the final complete frame.
    pub head: DigestV1,
    /// Exact complete prefix length, excluding any speculative suffix.
    pub byte_len: u64,
}

impl KagemushaRecoveryJournalPrefixV1 {
    fn validate(self) -> Result<(), KagemushaStateErrorV1> {
        if self.sequence == 0 || self.head == [0; 32] || self.byte_len == 0 {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }

    fn follows(self, previous: Self) -> bool {
        (self == previous)
            || (self.sequence > previous.sequence
                && self.byte_len > previous.byte_len
                && self.head != previous.head)
    }
}

/// Recovery material selected atomically with the complete Core snapshot.
/// This is a structural proposal, never evidence that files exist or hardware selected a root.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::recovery_metadata::KagemushaRecoveryJournalsV1"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaRecoveryJournalsV1 {
    /// Complete coordinator operation WAL prefix.
    pub coordinator: KagemushaRecoveryJournalPrefixV1,
    /// Complete exact original command/response archive prefix.
    pub responses: KagemushaRecoveryJournalPrefixV1,
    /// SHA-256 response-history SMT root using the device response-history V1 domain.
    pub response_history_root: DigestV1,
    /// Exact hardware retirement transition identity, retained even without further retirement.
    pub retirement_transition_id: DigestV1,
}

impl KagemushaRecoveryJournalsV1 {
    fn validate(&self) -> Result<(), KagemushaStateErrorV1> {
        self.coordinator.validate()?;
        self.responses.validate()?;
        if self.response_history_root == [0; 32] || self.retirement_transition_id == [0; 32] {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }

    fn validate_successor(&self, previous: &Self) -> Result<(), KagemushaStateErrorV1> {
        self.validate()?;
        if !self.coordinator.follows(previous.coordinator)
            || !self.responses.follows(previous.responses)
            || (self.response_history_root != previous.response_history_root
                && self.retirement_transition_id == previous.retirement_transition_id)
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        Ok(())
    }
}

/// Exact governed credential and authenticated catalog identity retained across restart.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::recovery_metadata::KagemushaAcceptedCredentialFloorV1"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaAcceptedCredentialFloorV1 {
    /// Original credential, including its original issuer signature and expiry.
    pub credential: KagemushaHardwareCredentialV1,
    /// Threshold-authenticated release used to admit the original credential.
    pub release_id: DigestV1,
}

impl KagemushaAcceptedCredentialFloorV1 {
    fn validate_catalog(
        &self,
        release: &KagemushaStateProofReleaseV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.release_id != release.release_id() {
            return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
        }
        let enabled = release
            .enabled_profile(self.credential.hardware_profile_id)
            .ok_or(KagemushaStateErrorV1::InvalidHardwareProfile)?;
        if self.credential.suite_id != enabled.suite_id
            || self.credential.policy_epoch != enabled.policy_epoch
        {
            return Err(KagemushaStateErrorV1::InvalidHardwareProfile);
        }
        self.credential
            .validate_against_profile(&enabled.hardware_profile)
            .map_err(|_| KagemushaStateErrorV1::InvalidHardwareProfile)
    }

    pub(super) fn validate_current(
        &self,
        state: &KagemushaStateV1,
        release: &KagemushaStateProofReleaseV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.validate_catalog(release)?;
        release.validate_state_context(state.context())?;
        let c = &self.credential;
        if c.network_id != state.lane.network_id
            || c.lane_commitment != state.lane.device_lane_id
            || u128::from(c.hardware_epoch_generation) != state.hardware_epoch.generation
            || c.hardware_epoch_id != state.hardware_epoch.epoch_id
            || c.device_key_reference != state.device_policy_binding.device_key_reference
            || c.hardware_profile_id != state.hardware_profile_id
            || c.policy_epoch != state.policy_epoch
            || c.suite_id != state.suite_id
        {
            return Err(KagemushaStateErrorV1::InvalidHardwareProfile);
        }
        Ok(())
    }

    fn advance(
        &self,
        state: &KagemushaStateV1,
        credential: KagemushaHardwareCredentialV1,
        release: &KagemushaStateProofReleaseV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let next = Self {
            credential,
            release_id: release.release_id(),
        };
        next.validate_current(state, release)?;
        let old = &self.credential;
        let new = &next.credential;
        if new.hardware_epoch_generation < old.hardware_epoch_generation
            || (new.hardware_epoch_generation == old.hardware_epoch_generation
                && (new.hardware_epoch_id != old.hardware_epoch_id
                    || new.issued_at_ms < old.issued_at_ms
                    || (new.issued_at_ms == old.issued_at_ms && new != old)))
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        // Republishing an identical credential never rewrites its original admitting catalog.
        Ok(if new == old { self.clone() } else { next })
    }
}

/// Exact hardware-selected predecessor identity. The all-zero identity is initial CAS only.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::recovery_metadata::KagemushaRecoveryCheckpointIdentityV1"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaRecoveryCheckpointIdentityV1 {
    /// Stable-wallet metadata revision.
    pub revision: u128,
    /// Complete canonical snapshot selected at this revision.
    pub snapshot_commitment: DigestV1,
}

impl KagemushaRecoveryCheckpointIdentityV1 {
    const INITIAL: Self = Self {
        revision: 0,
        snapshot_commitment: [0; 32],
    };

    fn from_anchor(anchor: &DurabilityAnchorStatementV1) -> Self {
        Self {
            revision: anchor.metadata_revision,
            snapshot_commitment: anchor.snapshot_commitment,
        }
    }
}

/// Immutable retail owner selected by the initial hardware checkpoint.
/// This decoded projection is only a selector until authenticated through the complete snapshot.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::recovery_metadata::KagemushaRecoveryEnrollmentBindingV1"
)]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaRecoveryEnrollmentBindingV1 {
    /// Stable enrollment identity derived from the exact typed owner.
    pub enrollment_id: DigestV1,
    /// Immutable account, FI, dataspace, network, asset and hardware-lane scope.
    pub owner: iroha_data_model::kagemusha::KagemushaRetailEnrollmentOwnerV1,
}

impl KagemushaRecoveryEnrollmentBindingV1 {
    pub(super) fn validate_for_state(
        &self,
        state: &KagemushaStateV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.enrollment_id
            != self
                .owner
                .enrollment_id()
                .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?
            || self.owner.lane_id != state.lane.device_lane_id
            || self.owner.runtime.network_id != state.lane.network_id
            || self.owner.runtime.asset != state.lane.asset
            || self.owner.runtime.asset_incarnation != state.asset_incarnation
            || self.owner.runtime.scale != state.lane.scale
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        Ok(())
    }
}

/// Mandatory snapshot metadata; omitted or older layouts are never accepted on recovery.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::recovery_metadata::KagemushaRecoveryMetadataV1"
)]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaRecoveryMetadataV1 {
    /// Immutable ownership established only from verified retail enrollment at bootstrap.
    pub enrollment: KagemushaRecoveryEnrollmentBindingV1,
    /// Global wallet metadata revision; epoch rotation does not reset it.
    pub revision: u128,
    /// Nonzero immutable checkpoint attempt identity, used for exact terminal replay.
    pub checkpoint_operation_id: DigestV1,
    /// Exact hardware predecessor compared by this publication.
    pub previous_checkpoint: KagemushaRecoveryCheckpointIdentityV1,
    /// Exact credential issuance floor.
    pub accepted_credential: KagemushaAcceptedCredentialFloorV1,
    /// Complete selected native journals and hardware retirement root.
    pub journals: KagemushaRecoveryJournalsV1,
}

impl KagemushaRecoveryMetadataV1 {
    pub(super) fn initial(
        state: &KagemushaStateV1,
        release: &KagemushaStateProofReleaseV1,
        credential: KagemushaHardwareCredentialV1,
        enrollment: KagemushaRecoveryEnrollmentBindingV1,
        journals: KagemushaRecoveryJournalsV1,
        checkpoint_operation_id: DigestV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        enrollment.validate_for_state(state)?;
        let accepted_credential = KagemushaAcceptedCredentialFloorV1 {
            credential,
            release_id: release.release_id(),
        };
        accepted_credential.validate_current(state, release)?;
        let value = Self {
            enrollment,
            revision: 1,
            checkpoint_operation_id,
            previous_checkpoint: KagemushaRecoveryCheckpointIdentityV1::INITIAL,
            accepted_credential,
            journals,
        };
        value.validate_shape()?;
        Ok(value)
    }

    pub(super) fn validate_shape(&self) -> Result<(), KagemushaStateErrorV1> {
        self.journals.validate()?;
        if self.checkpoint_operation_id == [0; 32]
            || self.previous_checkpoint.revision.checked_add(1) != Some(self.revision)
            || (self.previous_checkpoint.revision == 0
                && self.previous_checkpoint != KagemushaRecoveryCheckpointIdentityV1::INITIAL)
            || (self.previous_checkpoint.revision != 0
                && self.previous_checkpoint.snapshot_commitment == [0; 32])
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }

    pub(super) fn validate_restored(
        &self,
        state: &KagemushaStateV1,
        accepted_bindings: &[DevicePolicyBindingV1],
        floor_release: &KagemushaStateProofReleaseV1,
        expected_enrollment: &KagemushaRecoveryEnrollmentBindingV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.validate_shape()?;
        self.enrollment.validate_for_state(state)?;
        if &self.enrollment != expected_enrollment {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        self.accepted_credential.validate_catalog(floor_release)?;
        let c = &self.accepted_credential.credential;
        if c.network_id != state.lane.network_id
            || c.lane_commitment != state.lane.device_lane_id
            || u128::from(c.hardware_epoch_generation) > state.hardware_epoch.generation
            || !accepted_bindings
                .iter()
                .any(|binding| binding.device_key_reference == c.device_key_reference)
            || (u128::from(c.hardware_epoch_generation) == state.hardware_epoch.generation
                && (c.hardware_epoch_id != state.hardware_epoch.epoch_id
                    || c.device_key_reference != state.device_policy_binding.device_key_reference))
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        Ok(())
    }

    pub(super) fn checkpoint_statement(
        &self,
        successor: DurabilityAnchorStatementV1,
    ) -> KagemushaRecoveryCheckpointStatementV1 {
        KagemushaRecoveryCheckpointStatementV1 {
            operation_id: self.checkpoint_operation_id,
            previous: self.previous_checkpoint,
            successor,
        }
    }
}

/// Complete statement for one hardware metadata CAS and retained terminal certificate.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::recovery_metadata::KagemushaRecoveryCheckpointStatementV1"
)]
pub struct KagemushaRecoveryCheckpointStatementV1 {
    /// Nonzero exact retry identity, not authorization by itself.
    pub operation_id: DigestV1,
    /// Exact predecessor selected by hardware before the atomic transaction.
    pub previous: KagemushaRecoveryCheckpointIdentityV1,
    /// Complete successor checkpoint, including the full snapshot commitment.
    pub successor: DurabilityAnchorStatementV1,
}

/// Opaque proposal derived from one exact current machine. Preparing it changes no state.
#[derive(Clone)]
pub struct KagemushaRecoveryCheckpointCandidateV1 {
    pub(super) before_snapshot_commitment: DigestV1,
    pub(super) snapshot: KagemushaStateSnapshotV1,
    pub(super) statement: KagemushaRecoveryCheckpointStatementV1,
}

impl KagemushaRecoveryCheckpointCandidateV1 {
    /// Exact material the native owner must persist before requesting hardware CAS.
    #[must_use]
    pub fn snapshot(&self) -> &KagemushaStateSnapshotV1 {
        &self.snapshot
    }

    /// Exact locally derived statement the hardware transaction must authorize.
    #[must_use]
    pub fn statement(&self) -> &KagemushaRecoveryCheckpointStatementV1 {
        &self.statement
    }
}

/// Exclusive publication owner. Once hardware publication is attempted, the old machine cannot
/// escape on failure: recovery requires the persisted snapshot and a fresh hardware selection.
pub struct KagemushaRecoveryCheckpointPublicationV1<R, G, H> {
    machine: KagemushaStateMachineV1<R, G, H>,
    candidate: KagemushaRecoveryCheckpointCandidateV1,
}

impl<R, G, H> KagemushaRecoveryCheckpointPublicationV1<R, G, H>
where
    R: KagemushaRecursiveVerifierV1,
    G: KagemushaGuardBundleVerifierV1,
    H: KagemushaAuthenticatedHistoryStoreV1,
{
    /// Exact snapshot to persist before requesting hardware CAS.
    #[must_use]
    pub fn snapshot(&self) -> &KagemushaStateSnapshotV1 {
        self.candidate.snapshot()
    }
    /// Exact CAS statement; this projection never grants authority.
    #[must_use]
    pub fn statement(&self) -> &KagemushaRecoveryCheckpointStatementV1 {
        self.candidate.statement()
    }
    /// Consume the pending owner and return the machine only after exact certificate and current
    /// selection verification. Errors intentionally do not return an old usable machine.
    pub fn finish(
        mut self,
        guard_bundle: Vec<u8>,
    ) -> Result<KagemushaStateMachineV1<R, G, H>, KagemushaStateErrorV1> {
        self.machine
            .install_recovery_checkpoint(&self.candidate, guard_bundle)?;
        Ok(self.machine)
    }
}

impl KagemushaStateSnapshotV1 {
    pub(super) fn recovery_anchor(&self) -> DurabilityAnchorStatementV1 {
        DurabilityAnchorStatementV1 {
            metadata_revision: self.recovery_metadata.revision,
            version: self.version,
            lane: self.state.lane.clone(),
            state_commitment: self.state.state_commitment,
            hardware_epoch: self.state.hardware_epoch,
            device_policy_binding: self.state.device_policy_binding,
            state_nonce_commitment: self.state.state_nonce_commitment,
            logical_sequence: self.state.logical_sequence,
            journal_revision: self.journal_revision,
            inbox_revision: self.inbox_revision,
            snapshot_commitment: self.snapshot_commitment,
        }
    }

    fn recompute_commitment(&mut self) -> Result<(), KagemushaStateErrorV1> {
        self.snapshot_commitment = canonical_poseidon_digest(
            SNAPSHOT_COMMITMENT_DOMAIN,
            &SnapshotCommitmentPreimageV1 {
                recovery_metadata: self.recovery_metadata.clone(),
                version: self.version,
                state: self.state.clone(),
                journal_revision: self.journal_revision,
                inbox_revision: self.inbox_revision,
                pending_credits: self.pending_credits.clone(),
                accepted_recipient_bindings: self.accepted_recipient_bindings.clone(),
                accepted_payment_receipts: self.accepted_payment_receipts.clone(),
                mint_inbox: self.mint_inbox.clone(),
                consumed_credits: self.consumed_credits.clone(),
                authenticated_history_roots: self.authenticated_history_roots,
                authenticated_history_commitment: self.authenticated_history_commitment,
                receiver_inbox_capacity: self.receiver_inbox_capacity.clone(),
                sender_outbox_capacity: self.sender_outbox_capacity.clone(),
                outgoing_candidate_journal: self.outgoing_candidate_journal.clone(),
            },
        )?;
        Ok(())
    }
}

impl<R, G, H> KagemushaStateMachineV1<R, G, H> {
    /// Borrow the immutable owner authenticated by the hardware-selected complete checkpoint.
    /// Current admission/KYC is separate; historical committed recovery retains this owner.
    #[must_use]
    pub fn enrollment_binding(&self) -> &KagemushaRecoveryEnrollmentBindingV1 {
        &self.recovery_metadata.enrollment
    }

    /// Borrow the exact checkpointed credential floor; only opaque machines expose this view.
    #[must_use]
    pub fn accepted_credential_floor(&self) -> &KagemushaAcceptedCredentialFloorV1 {
        &self.recovery_metadata.accepted_credential
    }

    /// Borrow the original terminal certificate of the currently published checkpoint.
    /// Every returned machine has completed publication or authenticated restoration.
    #[must_use]
    pub fn recovery_checkpoint(&self) -> &DurabilityAnchorV1 {
        &self
            .published_checkpoint
            .as_ref()
            .expect("only a checkpointed machine escapes staging")
            .anchor
    }

    /// Borrow the complete currently published metadata, retained across monetary epoch changes.
    #[must_use]
    pub fn recovery_metadata(&self) -> &KagemushaRecoveryMetadataV1 {
        &self.recovery_metadata
    }
}

impl<R, G, H> KagemushaStateMachineV1<R, G, H>
where
    R: KagemushaRecursiveVerifierV1,
    G: KagemushaGuardBundleVerifierV1,
    H: KagemushaAuthenticatedHistoryStoreV1,
{
    /// Propose a complete successor checkpoint while retaining the exact previous credential.
    pub fn prepare_recovery_checkpoint(
        &self,
        operation_id: DigestV1,
        journals: KagemushaRecoveryJournalsV1,
    ) -> Result<KagemushaRecoveryCheckpointCandidateV1, KagemushaStateErrorV1> {
        self.prepare_checkpoint(
            operation_id,
            journals,
            self.recovery_metadata.accepted_credential.clone(),
        )
    }

    /// Propose a credential floor advance under the machine's actual authenticated release.
    /// This validates the issuer signature and exact scope but does not publish the credential.
    pub fn prepare_credential_checkpoint(
        &self,
        operation_id: DigestV1,
        journals: KagemushaRecoveryJournalsV1,
        credential: KagemushaHardwareCredentialV1,
    ) -> Result<KagemushaRecoveryCheckpointCandidateV1, KagemushaStateErrorV1> {
        let floor = self.recovery_metadata.accepted_credential.advance(
            &self.state,
            credential,
            &self.proof_release,
        )?;
        self.prepare_checkpoint(operation_id, journals, floor)
    }

    fn prepare_checkpoint(
        &self,
        operation_id: DigestV1,
        journals: KagemushaRecoveryJournalsV1,
        accepted_credential: KagemushaAcceptedCredentialFloorV1,
    ) -> Result<KagemushaRecoveryCheckpointCandidateV1, KagemushaStateErrorV1> {
        let current = self
            .published_checkpoint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotRollback)?;
        if operation_id == self.recovery_metadata.checkpoint_operation_id {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        journals.validate_successor(&self.recovery_metadata.journals)?;
        let revision = self
            .recovery_metadata
            .revision
            .checked_add(1)
            .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)?;
        let mut snapshot = self.snapshot()?;
        let before_snapshot_commitment = snapshot.snapshot_commitment;
        snapshot.recovery_metadata = KagemushaRecoveryMetadataV1 {
            enrollment: self.recovery_metadata.enrollment.clone(),
            revision,
            checkpoint_operation_id: operation_id,
            previous_checkpoint: KagemushaRecoveryCheckpointIdentityV1::from_anchor(
                &current.anchor.statement,
            ),
            accepted_credential,
            journals,
        };
        snapshot.recovery_metadata.validate_shape()?;
        snapshot.recompute_commitment()?;
        let statement = snapshot
            .recovery_metadata
            .checkpoint_statement(snapshot.recovery_anchor());
        Ok(KagemushaRecoveryCheckpointCandidateV1 {
            before_snapshot_commitment,
            snapshot,
            statement,
        })
    }

    /// Transfer this machine into an exclusive pending owner before issuing hardware CAS.
    /// Candidate staleness is checked before staging; no wallet methods are exposed while pending.
    pub fn stage_recovery_checkpoint(
        self,
        candidate: KagemushaRecoveryCheckpointCandidateV1,
    ) -> Result<KagemushaRecoveryCheckpointPublicationV1<R, G, H>, KagemushaStateErrorV1> {
        let existing = self
            .published_checkpoint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotRollback)?;
        let exact_retry = existing.anchor.statement == candidate.statement.successor;
        let current_snapshot = self.snapshot()?.snapshot_commitment;
        if (exact_retry && current_snapshot != candidate.snapshot.snapshot_commitment)
            || (!exact_retry
                && (candidate.statement.previous
                    != KagemushaRecoveryCheckpointIdentityV1::from_anchor(
                        &existing.anchor.statement,
                    )
                    || current_snapshot != candidate.before_snapshot_commitment))
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        Ok(KagemushaRecoveryCheckpointPublicationV1 {
            machine: self,
            candidate,
        })
    }

    // Only the two opaque publication owners invoke this after exclusive ownership transfer.
    pub(super) fn install_recovery_checkpoint(
        &mut self,
        candidate: &KagemushaRecoveryCheckpointCandidateV1,
        guard_bundle: Vec<u8>,
    ) -> Result<DurabilityAnchorV1, KagemushaStateErrorV1> {
        validate_guard_bytes(&guard_bundle)?;
        if let Some(existing) = &self.published_checkpoint {
            if existing.anchor.statement == candidate.statement.successor {
                if existing.anchor.guard_bundle != guard_bundle
                    || self.snapshot()?.snapshot_commitment
                        != candidate.snapshot.snapshot_commitment
                {
                    return Err(KagemushaStateErrorV1::SnapshotRollback);
                }
                self.guard_verifier
                    .verify_current_recovery_checkpoint(
                        &existing.anchor.statement,
                        &candidate.snapshot.recovery_metadata.journals,
                    )
                    .map_err(KagemushaStateErrorV1::GuardRejected)?;
                return Ok(existing.anchor.clone());
            }
        }
        let previous = self
            .published_checkpoint
            .as_ref()
            .map_or(KagemushaRecoveryCheckpointIdentityV1::INITIAL, |anchor| {
                KagemushaRecoveryCheckpointIdentityV1::from_anchor(&anchor.anchor.statement)
            });
        if candidate.statement.previous != previous
            || self.snapshot()?.snapshot_commitment != candidate.before_snapshot_commitment
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        self.guard_verifier
            .verify_recovery_checkpoint_cas(&candidate.statement, &guard_bundle)
            .map_err(KagemushaStateErrorV1::GuardRejected)?;
        self.guard_verifier
            .verify_current_recovery_checkpoint(
                &candidate.statement.successor,
                &candidate.snapshot.recovery_metadata.journals,
            )
            .map_err(KagemushaStateErrorV1::GuardRejected)?;
        let anchor = DurabilityAnchorV1 {
            statement: candidate.statement.successor.clone(),
            guard_bundle,
        };
        self.recovery_metadata = candidate.snapshot.recovery_metadata.clone();
        self.published_checkpoint = Some(PublishedRecoveryCheckpointV1 {
            anchor: anchor.clone(),
            authenticated_history_commitment: candidate.snapshot.authenticated_history_commitment,
        });
        Ok(anchor)
    }
}

#[cfg(all(test, unix))]
mod tests;

#[cfg(test)]
#[test]
fn captured_state_frame_owners() {
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<
        KagemushaRecoveryCheckpointStatementV1,
    >(
        "iroha_core::zk::kagemusha_v1_state::recovery_metadata::KagemushaRecoveryCheckpointStatementV1",
    );
}
