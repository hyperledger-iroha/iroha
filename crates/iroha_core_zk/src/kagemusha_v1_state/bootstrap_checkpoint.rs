//! Initial recovery material is created only by the opaque, verified bootstrap owner.

use super::*;
use iroha_fs::{OwnerDirectory, PrivateDirectory, PublishMode};
use rand_core_06::{OsRng, RngCore as _};
use std::path::Path;

const BOOTSTRAP_FORMAT: private_journal::PrivateJournalFormat =
    private_journal::PrivateJournalFormat {
        filename: "bootstrap.norito.wal",
        magic: b"IKGBI1\0\0",
        hash_domain: b"iroha:kagemusha:v1:bootstrap-initial-snapshot:frame\0",
        maximum_payload_bytes: 4 * 1024 * 1024,
    };

// One canonical first-release manifest binds the exact initialized resource policy as well
// as the complete initial snapshot. Resume must never reinterpret the same bundle under a
// different coordinator reservation budget.
#[derive(norito::Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::bootstrap_checkpoint::BootstrapJournalManifestV1"
)]
struct BootstrapJournalManifestV1 {
    coordinator_live_capacity_bytes: u64,
    snapshot: KagemushaStateSnapshotV1,
}

fn bootstrap_manifest_bytes(
    snapshot: &KagemushaStateSnapshotV1,
    coordinator_live_capacity_bytes: u64,
) -> Result<Vec<u8>, KagemushaStateErrorV1> {
    norito::encode_canonical(&BootstrapJournalManifestV1 {
        coordinator_live_capacity_bytes,
        snapshot: snapshot.clone(),
    })
    .map_err(material_error)
}

/// Verified initial state and credential awaiting creation of actual native journals.
/// This owner cannot expose a usable machine or accept host-provided journal prefixes.
pub struct KagemushaBootstrapJournalStageV1<R, G, H> {
    state: KagemushaStateV1,
    proof_release: KagemushaStateProofReleaseV1,
    initial_credential: KagemushaAcceptedCredentialFloorV1,
    enrollment: KagemushaRecoveryEnrollmentBindingV1,
    durable_capacity: KagemushaDurableCapacityV1,
    authenticated_history: KagemushaStateAuthenticatedHistoryV1<H>,
    recursive_verifier: R,
    guard_verifier: G,
    #[cfg(test)]
    pub(super) initialization_failure: Option<BootstrapJournalFailure>,
}

impl<R, G, H> KagemushaBootstrapJournalStageV1<R, G, H>
where
    R: KagemushaRecursiveVerifierV1,
    G: KagemushaGuardBundleVerifierV1,
    H: KagemushaAuthenticatedHistoryStoreV1,
{
    pub(super) fn new(
        state: KagemushaStateV1,
        proof_release: KagemushaStateProofReleaseV1,
        initial_credential: KagemushaAcceptedCredentialFloorV1,
        enrollment: KagemushaRecoveryEnrollmentBindingV1,
        durable_capacity: KagemushaDurableCapacityV1,
        authenticated_history: KagemushaStateAuthenticatedHistoryV1<H>,
        recursive_verifier: R,
        guard_verifier: G,
    ) -> Result<Self, KagemushaStateErrorV1> {
        enrollment.validate_for_state(&state)?;
        initial_credential.validate_current(&state, &proof_release)?;
        Ok(Self {
            state,
            proof_release,
            initial_credential,
            enrollment,
            durable_capacity,
            authenticated_history,
            recursive_verifier,
            guard_verifier,
            #[cfg(test)]
            initialization_failure: None,
        })
    }

    /// Atomically publish one complete, descriptor-owned journal bundle on the same filesystem.
    /// Partial private staging directories never occupy the final path. Existing bundles are never
    /// replaced or reset; initial hardware CAS still must compare the canonical zero predecessor.
    pub fn initialize_journals(
        self,
        bundle_directory: &Path,
        coordinator_live_capacity_bytes: u64,
        checkpoint_operation_id: DigestV1,
    ) -> Result<KagemushaBootstrapCheckpointV1<R, G, H>, KagemushaStateErrorV1> {
        if checkpoint_operation_id == [0; 32] {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let parent_path = bundle_directory
            .parent()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let final_name = bundle_directory
            .file_name()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if !bundle_directory.is_absolute() {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        #[cfg(unix)]
        if parent_path.canonicalize().map_err(material_error)? != parent_path {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let parent = OwnerDirectory::open(parent_path).map_err(material_error)?;
        let mut nonce = [0u8; 16];
        OsRng.try_fill_bytes(&mut nonce).map_err(material_error)?;
        let staging_directory = parent
            .create_private_child(format!(".offline-bootstrap-{}", hex::encode(nonce)))
            .map_err(material_error)?;
        let staging = staging_directory.path().to_path_buf();
        let coordinator = KagemushaCoordinatorOperationStoreV1::create_new(
            &staging.join("operations"),
            self.state.lane.clone(),
            self.state.asset_incarnation,
            coordinator_live_capacity_bytes,
        )
        .map_err(material_error)?;
        #[cfg(test)]
        if self.initialization_failure == Some(BootstrapJournalFailure::AfterFirstJournal) {
            return Err(KagemushaStateErrorV1::RecoveryMaterial(
                "injected initial journal interruption".into(),
            ));
        }
        let responses = KagemushaResponseEvidenceArchiveV1::create_new(
            &staging.join("responses"),
            &self.state.lane,
            self.state.asset_incarnation,
        )
        .map_err(material_error)?;
        let coordinator_prefix = coordinator.recovery_prefix().map_err(material_error)?;
        let response_prefix = responses.recovery_prefix().map_err(material_error)?;
        #[cfg(test)]
        let initialization_failure = self.initialization_failure;
        let (machine, candidate) =
            self.into_candidate(&coordinator, &responses, checkpoint_operation_id)?;
        let mut manifest = private_journal::PrivateJournal::create_new(
            &staging.join("bootstrap"),
            BOOTSTRAP_FORMAT,
        )
        .map_err(material_error)?;
        manifest
            .append(&bootstrap_manifest_bytes(
                candidate.snapshot(),
                coordinator_live_capacity_bytes,
            )?)
            .map_err(material_error)?;
        staging_directory.sync().map_err(material_error)?;
        parent.sync().map_err(material_error)?;
        require_same_directory(&parent, parent_path)?;
        // Paths held by each PrivateJournal change at rename. Close and reopen the exact final
        // names, comparing complete prefix identities before any hardware publication can occur.
        drop(coordinator);
        drop(responses);
        drop(manifest);
        staging_directory
            .rename_to_sibling(final_name, PublishMode::CreateNew)
            .map_err(material_error)?;
        #[cfg(test)]
        if initialization_failure == Some(BootstrapJournalFailure::AfterRename) {
            return Err(KagemushaStateErrorV1::RecoveryMaterial(
                "injected bundle publication interruption".into(),
            ));
        }
        parent.sync().map_err(material_error)?;
        require_same_directory(&parent, parent_path)?;
        let coordinator = KagemushaCoordinatorOperationStoreV1::open_existing(
            &bundle_directory.join("operations"),
            machine.state.lane.clone(),
            machine.state.asset_incarnation,
            coordinator_live_capacity_bytes,
        )
        .map_err(material_error)?;
        let responses = KagemushaResponseEvidenceArchiveV1::open_existing(
            &bundle_directory.join("responses"),
            &machine.state.lane,
            machine.state.asset_incarnation,
        )
        .map_err(material_error)?;
        if coordinator.recovery_prefix().map_err(material_error)? != coordinator_prefix
            || responses.recovery_prefix().map_err(material_error)? != response_prefix
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        let manifest = open_bootstrap_manifest(
            bundle_directory,
            candidate.snapshot(),
            coordinator_live_capacity_bytes,
        )?;
        Ok(KagemushaBootstrapCheckpointV1 {
            machine,
            candidate,
            coordinator,
            responses,
            manifest,
        })
    }

    /// Resume only a complete final initializer-only bundle for the exact verified bootstrap.
    /// Missing, changed or advanced journals fail; recovery never creates a missing child.
    pub fn resume_initialized_journals(
        self,
        bundle_directory: &Path,
        coordinator_live_capacity_bytes: u64,
        checkpoint_operation_id: DigestV1,
    ) -> Result<KagemushaBootstrapCheckpointV1<R, G, H>, KagemushaStateErrorV1> {
        if checkpoint_operation_id == [0; 32] {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let parent_path = bundle_directory
            .parent()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if !bundle_directory.is_absolute() {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        #[cfg(unix)]
        if parent_path.canonicalize().map_err(material_error)? != parent_path {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let parent = OwnerDirectory::open(parent_path).map_err(material_error)?;
        let bundle = PrivateDirectory::open(bundle_directory).map_err(material_error)?;
        bundle.sync().map_err(material_error)?;
        parent.sync().map_err(material_error)?;
        let coordinator = KagemushaCoordinatorOperationStoreV1::open_existing(
            &bundle_directory.join("operations"),
            self.state.lane.clone(),
            self.state.asset_incarnation,
            coordinator_live_capacity_bytes,
        )
        .map_err(material_error)?;
        let responses = KagemushaResponseEvidenceArchiveV1::open_existing(
            &bundle_directory.join("responses"),
            &self.state.lane,
            self.state.asset_incarnation,
        )
        .map_err(material_error)?;
        require_same_directory(&parent, parent_path)?;
        bundle.revalidate().map_err(material_error)?;
        let current_bundle = PrivateDirectory::open(bundle_directory).map_err(material_error)?;
        if bundle.path() != bundle_directory
            || bundle.identity().map_err(material_error)?
                != current_bundle.identity().map_err(material_error)?
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let (machine, candidate) =
            self.into_candidate(&coordinator, &responses, checkpoint_operation_id)?;
        let manifest = open_bootstrap_manifest(
            bundle_directory,
            candidate.snapshot(),
            coordinator_live_capacity_bytes,
        )?;
        Ok(KagemushaBootstrapCheckpointV1 {
            machine,
            candidate,
            coordinator,
            responses,
            manifest,
        })
    }

    fn into_candidate(
        self,
        coordinator: &KagemushaCoordinatorOperationStoreV1,
        responses: &KagemushaResponseEvidenceArchiveV1,
        checkpoint_operation_id: DigestV1,
    ) -> Result<
        (
            KagemushaStateMachineV1<R, G, H>,
            KagemushaRecoveryCheckpointCandidateV1,
        ),
        KagemushaStateErrorV1,
    > {
        let journals = KagemushaRecoveryJournalsV1 {
            coordinator: coordinator
                .recovery_prefix()
                .map_err(|error| KagemushaStateErrorV1::RecoveryMaterial(error.to_string()))?,
            responses: responses
                .recovery_prefix()
                .map_err(|error| KagemushaStateErrorV1::RecoveryMaterial(error.to_string()))?,
            response_history_root: empty_response_history_root(),
            // The initial CAS initializes the empty response-history register under this exact ID.
            retirement_transition_id: checkpoint_operation_id,
        };
        if journals.coordinator.sequence != 1 || journals.responses.sequence != 1 {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        let recovery_metadata = KagemushaRecoveryMetadataV1::initial(
            &self.state,
            &self.proof_release,
            self.initial_credential,
            self.enrollment,
            journals,
            checkpoint_operation_id,
        )?;
        let binding = self.state.device_policy_binding;
        let machine = KagemushaStateMachineV1 {
            recovery_metadata,
            published_checkpoint: None,
            state: self.state,
            journal_revision: 0,
            inbox_revision: 0,
            pending_credits: BTreeMap::new(),
            accepted_recipient_bindings: BTreeSet::from([binding]),
            accepted_payment_receipts: BTreeMap::new(),
            mint_inbox: KagemushaMintInboxV1::default(),
            consumed_credits: ExactConsumedCreditIndex::empty(),
            authenticated_history: self.authenticated_history,
            receiver_inbox_capacity: KagemushaReceiverInboxCapacityV1::new(
                self.durable_capacity.inbox_bytes,
            ),
            sender_outbox_capacity: KagemushaSenderOutboxCapacityV1::new(
                self.durable_capacity.outbox_bytes,
            ),
            outgoing_candidate_journal: KagemushaOutgoingCandidateJournalV1::default(),
            proof_release: self.proof_release,
            recursive_verifier: self.recursive_verifier,
            guard_verifier: self.guard_verifier,
        };
        let snapshot = machine.snapshot()?;
        let candidate = KagemushaRecoveryCheckpointCandidateV1 {
            before_snapshot_commitment: snapshot.snapshot_commitment,
            statement: snapshot
                .recovery_metadata
                .checkpoint_statement(snapshot.recovery_anchor()),
            snapshot,
        };
        Ok((machine, candidate))
    }
}

/// Initial full snapshot plus held native journals awaiting actual hardware CAS publication.
pub struct KagemushaBootstrapCheckpointV1<R, G, H> {
    machine: KagemushaStateMachineV1<R, G, H>,
    pub(super) candidate: KagemushaRecoveryCheckpointCandidateV1,
    coordinator: KagemushaCoordinatorOperationStoreV1,
    responses: KagemushaResponseEvidenceArchiveV1,
    manifest: private_journal::PrivateJournal,
}

impl<R, G, H> KagemushaBootstrapCheckpointV1<R, G, H>
where
    R: KagemushaRecursiveVerifierV1,
    G: KagemushaGuardBundleVerifierV1,
    H: KagemushaAuthenticatedHistoryStoreV1,
{
    /// Complete initial snapshot to persist before requesting hardware publication.
    #[must_use]
    pub fn snapshot(&self) -> &KagemushaStateSnapshotV1 {
        self.candidate.snapshot()
    }
    /// Exact initial CAS statement; no snapshot or decoded DTO confers authority.
    #[must_use]
    pub fn statement(&self) -> &KagemushaRecoveryCheckpointStatementV1 {
        self.candidate.statement()
    }
    /// Return the usable machine and held real journals only after CAS and fresh current selection.
    /// Failure consumes the pending owner; a lost acknowledgement requires authenticated recovery.
    pub fn finish(
        mut self,
        guard_bundle: Vec<u8>,
    ) -> Result<KagemushaBootstrappedWalletV1<R, G, H>, KagemushaStateErrorV1> {
        self.finish_attempt(guard_bundle)?;
        Ok(self.into_wallet())
    }

    /// Retain the exact exclusive initialized owner when hardware or fresh selection is lost.
    /// No previous machine or alternate initialization escapes through this retry path.
    pub fn finish_or_retain(
        mut self,
        original_guard_bundle: Vec<u8>,
    ) -> Result<KagemushaBootstrappedWalletV1<R, G, H>, (Box<Self>, KagemushaStateErrorV1)> {
        match self.finish_attempt(original_guard_bundle) {
            Ok(()) => Ok(self.into_wallet()),
            Err(error) => Err((Box::new(self), error)),
        }
    }

    fn finish_attempt(&mut self, guard_bundle: Vec<u8>) -> Result<(), KagemushaStateErrorV1> {
        self.manifest.check_owned().map_err(material_error)?;
        // Recheck the actual descriptor-owned prefixes immediately before publication verification.
        if self
            .coordinator
            .recovery_prefix()
            .map_err(|error| KagemushaStateErrorV1::RecoveryMaterial(error.to_string()))?
            != self
                .candidate
                .snapshot
                .recovery_metadata
                .journals
                .coordinator
            || self
                .responses
                .recovery_prefix()
                .map_err(|error| KagemushaStateErrorV1::RecoveryMaterial(error.to_string()))?
                != self.candidate.snapshot.recovery_metadata.journals.responses
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        self.machine
            .install_recovery_checkpoint(&self.candidate, guard_bundle)?;
        Ok(())
    }

    fn into_wallet(self) -> KagemushaBootstrappedWalletV1<R, G, H> {
        KagemushaBootstrappedWalletV1 {
            machine: self.machine,
            coordinator: self.coordinator,
            responses: self.responses,
        }
    }
}

/// Successfully checkpointed initial wallet with the exact journals used by its publication.
pub struct KagemushaBootstrappedWalletV1<R, G, H> {
    machine: KagemushaStateMachineV1<R, G, H>,
    coordinator: KagemushaCoordinatorOperationStoreV1,
    responses: KagemushaResponseEvidenceArchiveV1,
}

impl<R, G, H> KagemushaBootstrappedWalletV1<R, G, H> {
    /// Transfer all three owned components to the native coordinator without dropping journal locks.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        KagemushaStateMachineV1<R, G, H>,
        KagemushaCoordinatorOperationStoreV1,
        KagemushaResponseEvidenceArchiveV1,
    ) {
        (self.machine, self.coordinator, self.responses)
    }
}

fn open_bootstrap_manifest(
    bundle: &Path,
    expected: &KagemushaStateSnapshotV1,
    coordinator_live_capacity_bytes: u64,
) -> Result<private_journal::PrivateJournal, KagemushaStateErrorV1> {
    let mut manifest =
        private_journal::PrivateJournal::open_existing(&bundle.join("bootstrap"), BOOTSTRAP_FORMAT)
            .map_err(material_error)?;
    let expected = bootstrap_manifest_bytes(expected, coordinator_live_capacity_bytes)?;
    match manifest.replay_next().map_err(material_error)? {
        Some((0, actual)) if actual == expected => {}
        _ => return Err(KagemushaStateErrorV1::SnapshotRollback),
    }
    if manifest.replay_next().map_err(material_error)?.is_some() {
        return Err(KagemushaStateErrorV1::SnapshotRollback);
    }
    Ok(manifest)
}

fn empty_response_history_root() -> DigestV1 {
    const DOMAIN: &[u8] = b"iroha:kagemusha:device:v1:response-history\0";
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN);
    hasher.update([3]);
    let mut node: DigestV1 = hasher.finalize().into();
    for _ in 0..256 {
        let mut hasher = Sha256::new();
        hasher.update(DOMAIN);
        hasher.update([4]);
        hasher.update(node);
        hasher.update(node);
        node = hasher.finalize().into();
    }
    node
}

fn material_error(error: impl core::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::RecoveryMaterial(error.to_string())
}

fn require_same_directory(
    directory: &OwnerDirectory,
    path: &Path,
) -> Result<(), KagemushaStateErrorV1> {
    directory.revalidate().map_err(material_error)?;
    let current = OwnerDirectory::open(path).map_err(material_error)?;
    if directory.path() != path
        || directory.identity().map_err(material_error)?
            != current.identity().map_err(material_error)?
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BootstrapJournalFailure {
    AfterFirstJournal,
    AfterRename,
}

#[cfg(test)]
#[test]
fn captured_state_frame_owners() {
    crate::kagemusha_v1_state::state_frame_identity_tests::observed::<BootstrapJournalManifestV1>(
        "iroha_core::zk::kagemusha_v1_state::bootstrap_checkpoint::BootstrapJournalManifestV1",
    );
}
