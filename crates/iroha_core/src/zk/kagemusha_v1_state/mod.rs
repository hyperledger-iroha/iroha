//! Pure Core state machine for aggregate, hardware-guarded Kagemusha V1 balances.
//!
//! This module owns deterministic host state, conservation checks, durable credit staging, exact
//! replay accounting, and crash-recovery projections. It deliberately does **not** treat host
//! checks as recursive-proof authority. Every bootstrap, monetary transition, peer credit, and
//! durable journal seal crosses explicit proof and hardware-guard verifier hooks; the supplied
//! reject-all implementations make an unintegrated deployment fail closed.

mod candidate_lifecycle;
#[cfg(unix)]
mod coordinator_operation_store;
#[cfg(unix)]
pub use coordinator_operation_store::{
    KAGEMUSHA_COORDINATOR_INTENT_MAX_BYTES_V1, KAGEMUSHA_COORDINATOR_PUBLIC_BINDING_MAX_BYTES_V1,
    KagemushaCoordinatorOperationStoreErrorV1, KagemushaCoordinatorOperationStoreV1,
    KagemushaCoordinatorSenderIntentRecoveryV1,
};
mod handoff_verification;
mod mint_fold_private_inputs;
mod mint_inbox;
mod mint_inbox_operations;
mod outgoing_operation_index;
#[cfg(unix)]
mod private_journal;
mod receive_fold;
mod receive_fold_operation;
mod redemption_release;
mod sparse_merkle;

pub use candidate_lifecycle::{
    CommittedOutgoingCandidateV1, DurableOutgoingEnvelopeV1, KagemushaDurableCapacityV1,
    KagemushaOutgoingCandidateJournalV1, KagemushaOutgoingCommitCapabilityV1,
    KagemushaOutgoingEnvelopeV1, KagemushaOutgoingJournalStageV1, KagemushaReceiverInboxCapacityV1,
    KagemushaSenderOutboxCapacityV1, PersistedOutgoingCandidateV1, PersistedOutgoingRecoveryViewV1,
    PreparedOutgoingCandidateV1, PreparedOutgoingRecoveryViewV1, SenderOutboxReservationOutcomeV1,
};
pub use handoff_verification::{
    KagemushaHandoffEvidenceSizesV1, KagemushaHandoffEvidenceV1,
    KagemushaHandoffSequenceVerificationV1, verify_kagemusha_handoff_evidence_sequence_v1,
    verify_kagemusha_handoff_evidence_v1,
};
pub use iroha_data_model::kagemusha::KagemushaOutboxReservationV1;
pub use mint_fold_private_inputs::KagemushaMintFoldOpeningCapabilityV1;
pub(crate) use mint_fold_private_inputs::{
    KagemushaMintFoldOpeningWitnessV1, KagemushaMintFoldPrivateInputsV1,
};
pub use mint_inbox::*;
pub use mint_inbox_operations::{
    KagemushaPendingCreditWatermarkV1, MintCreditStageOutcomeV1, PendingCreditFoldV1,
};
pub use outgoing_operation_index::{
    KAGEMUSHA_OUTGOING_OPERATION_PAGE_MAX_V1, KAGEMUSHA_OUTGOING_PUBLIC_INPUTS_DOMAIN_V1,
    KagemushaOutgoingOperationContextV1, KagemushaOutgoingOperationIndexErrorV1,
    KagemushaOutgoingOperationIndexResultV1, KagemushaOutgoingOperationIndexV1,
    KagemushaOutgoingOperationPageV1, KagemushaOutgoingOperationPhaseV1,
    KagemushaOutgoingOperationPrepareOutcomeV1, KagemushaOutgoingOperationRecordV1,
    KagemushaOutgoingPublicInputPreimageV1, KagemushaOutgoingPublicInputsV1,
};
pub use receive_fold::{
    KAGEMUSHA_RECEIVE_FOLD_CREDIT_BYTES_V1, KAGEMUSHA_RECEIVE_FOLD_DOMAIN_V1, ReceiveFoldCreditV1,
    ReceiveFoldErrorV1, ReceiveFoldReplayRootUpdateInputV1, ReceiveFoldV1,
};
pub use receive_fold_operation::{PeerCreditFoldInputV1, PeerCreditFoldPreviewV1};
pub use redemption_release::{
    KAGEMUSHA_REDEMPTION_TERMINAL_RECEIPT_DOMAIN_V1, KagemushaRedemptionTerminalReceiptV1,
    VerifiedKagemushaRedemptionReleaseV1,
};

#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    isi::kagemusha_v1::{KagemushaFinalityTrustAnchorV1, KagemushaOperationStatusV1},
    kagemusha::{
        KAGEMUSHA_ASSET_SCALE_MAX_V1, KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1,
        KAGEMUSHA_WIRE_VERSION_V1, KagemushaAcknowledgementV1, KagemushaAuthenticatedReleaseV1,
        KagemushaCommitCertificateV1, KagemushaCommitEvidenceV1, KagemushaCreditOpeningV1,
        KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1, KagemushaEnabledProfileV1,
        KagemushaEncryptedCreditEnvelopeV1, KagemushaLifecycleBindingV1, KagemushaMintCreditV1,
        KagemushaOperationKindV1, KagemushaPairedProofV1, KagemushaPastaStateCommitmentV1,
        KagemushaPaymentOutputV1, KagemushaPaymentRequestV1, KagemushaPaymentV1,
        KagemushaRedemptionProofV1, KagemushaRedemptionStatementV1,
        kagemusha_asset_identity_digest_v1, kagemusha_ciphertext_digest_v1,
        kagemusha_device_key_reference_v1, kagemusha_liability_pool_id_v1,
        kagemusha_pasta_state_commitment_v1, kagemusha_peer_credit_opening_commitment_v1,
    },
    nexus::AxtAssetIncarnationV1,
};
use iroha_zkp_halo2::poseidon;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use self::sparse_merkle::ExactConsumedCreditIndex;
#[cfg(unix)]
pub(crate) use self::sparse_merkle::authenticated_history::disk_history_store::{
    KagemushaDiskAuthenticatedHistoryStoreV1, KagemushaHistoryDeviceCredentialsV1,
};
pub(crate) use self::sparse_merkle::authenticated_history::{
    KagemushaAuthenticatedHistoryStoreV1, KagemushaCommittedRootReadV1,
    KagemushaHistoryAbortOutcomeV1, KagemushaHistoryCommitOutcomeV1,
    KagemushaHistoryDualInsertPreparationV1, KagemushaHistoryIdentityClassificationV1,
    KagemushaHistoryInsertPreparationV1, KagemushaHistoryNodeBodyV1, KagemushaHistoryNodeRecordV1,
    KagemushaHistoryOverlayUsageV1, KagemushaHistoryPrepareOutcomeV1,
    KagemushaHistoryProofRootBridgeErrorV1, KagemushaHistoryProofRootBridgeRequestV1,
    KagemushaHistoryRecoveryOutcomeV1, KagemushaHistoryRootCasV1,
    KagemushaHistoryRootSelectionCertificateV1, KagemushaHistoryRootSelectionSubjectV1,
    KagemushaHistoryRootSelectionV1, KagemushaHistoryRootsV1, KagemushaHistoryStoreErrorV1,
    KagemushaHistoryTreeV1, KagemushaMemoryAuthenticatedHistoryStoreV1,
    KagemushaPreparedHistoryCasV1, VerifiedKagemushaHistoryProofRootBridgeV1,
    VerifiedKagemushaHistoryRootSelectionV1, classify_history_identity_v1,
    prepare_history_identity_insert_v1, prepare_history_identity_pair_v1,
    require_history_proof_root_bridge_v1, validate_committed_history_v1,
};

use super::kagemusha_v1_poseidon::{
    KAGEMUSHA_STATE_DOMAIN_V1, KagemushaPoseidonFieldV1, decode as decode_pasta, digest_limbs,
    encode as encode_pasta, from_u128 as pasta_from_u128, hash as pasta_hash,
};
use super::kagemusha_v1_recursion::{
    KagemushaGuardContextV1, KagemushaNormalizedGuardStatementV1, KagemushaRecursionArtifactsV1,
    KagemushaRecursiveVerifierV1, KagemushaStateRelationPublicInputsV1,
    VerifiedKagemushaMintFinalityHelperV1, canonical_prepared_transition_binding_digest_v1,
    kagemusha_incoming_proof_binding_digest_v1, verify_kagemusha_state_proof_v1,
};

/// State-owned façade over one external dual-root authenticated-history store.
///
/// The façade keeps cumulative replay and terminal-decision history outside the live receiver
/// inbox. Only prepared, uncommitted CAS bytes are subject to the store's local overlay limit;
/// committed roots and exact duplicate/conflict answers remain authoritative across retries.
#[derive(Clone)]
pub(crate) struct KagemushaStateAuthenticatedHistoryV1<S> {
    store: S,
}

impl<S> KagemushaStateAuthenticatedHistoryV1<S>
where
    S: KagemushaAuthenticatedHistoryStoreV1,
{
    /// Open a store only after both complete committed trees validate.
    pub(crate) fn open(store: S) -> Result<Self, KagemushaHistoryStoreErrorV1> {
        validate_committed_history_v1(&store)?;
        Ok(Self { store })
    }

    /// Recover a store against roots sealed by the latest hardware recovery checkpoint.
    pub(crate) fn recover(
        store: S,
        expected_roots: KagemushaHistoryRootsV1,
        expected_recovery_commitment: DigestV1,
    ) -> Result<Self, KagemushaHistoryStoreErrorV1> {
        let actual = validate_committed_history_v1(&store)?;
        if actual != expected_roots {
            return Err(KagemushaHistoryStoreErrorV1::CommittedRootsMismatch {
                expected: expected_roots,
                actual,
            });
        }
        store.validate_recovery_checkpoint(expected_recovery_commitment)?;
        Ok(Self { store })
    }

    /// Return both independently selected authoritative roots.
    pub(crate) fn committed_roots(&self) -> KagemushaHistoryRootsV1 {
        self.store.committed_roots()
    }

    /// Return exact live WAL usage; this meter never includes committed history.
    pub(crate) fn overlay_usage(&self) -> KagemushaHistoryOverlayUsageV1 {
        self.store.overlay_usage()
    }

    /// Classify one consumed-credit identity without treating missing history as absence.
    pub(crate) fn classify_replay(
        &self,
        credit_id: CreditIdV1,
        envelope_digest: DigestV1,
    ) -> Result<KagemushaHistoryIdentityClassificationV1, KagemushaHistoryStoreErrorV1> {
        classify_history_identity_v1(
            &self.store,
            KagemushaHistoryTreeV1::Replay,
            credit_id.0,
            envelope_digest,
        )
    }

    /// Classify one terminal decision identity against byte-identical decision material.
    pub(crate) fn classify_terminal_decision(
        &self,
        decision_id: DigestV1,
        decision_digest: DigestV1,
    ) -> Result<KagemushaHistoryIdentityClassificationV1, KagemushaHistoryStoreErrorV1> {
        classify_history_identity_v1(
            &self.store,
            KagemushaHistoryTreeV1::TerminalDecision,
            decision_id,
            decision_digest,
        )
    }

    /// Durably prepare one consumed-credit replay insertion before hardware root selection.
    pub(crate) fn prepare_replay(
        &mut self,
        credit_id: CreditIdV1,
        envelope_digest: DigestV1,
        attempt_binding_digest: DigestV1,
    ) -> Result<KagemushaHistoryInsertPreparationV1, KagemushaHistoryStoreErrorV1> {
        prepare_history_identity_insert_v1(
            &mut self.store,
            KagemushaHistoryTreeV1::Replay,
            credit_id.0,
            envelope_digest,
            attempt_binding_digest,
        )
    }

    /// Durably prepare one terminal decision insertion before hardware root selection.
    pub(crate) fn prepare_terminal_decision(
        &mut self,
        decision_id: DigestV1,
        decision_digest: DigestV1,
        attempt_binding_digest: DigestV1,
    ) -> Result<KagemushaHistoryInsertPreparationV1, KagemushaHistoryStoreErrorV1> {
        prepare_history_identity_insert_v1(
            &mut self.store,
            KagemushaHistoryTreeV1::TerminalDecision,
            decision_id,
            decision_digest,
            attempt_binding_digest,
        )
    }

    /// Prepare one replay insertion and one terminal decision under a single atomic root CAS.
    pub(crate) fn prepare_replay_and_terminal_decision(
        &mut self,
        credit_id: CreditIdV1,
        envelope_digest: DigestV1,
        decision_id: DigestV1,
        decision_digest: DigestV1,
        attempt_binding_digest: DigestV1,
    ) -> Result<KagemushaHistoryDualInsertPreparationV1, KagemushaHistoryStoreErrorV1> {
        prepare_history_identity_pair_v1(
            &mut self.store,
            credit_id.0,
            envelope_digest,
            decision_id,
            decision_digest,
            attempt_binding_digest,
        )
    }

    /// Require this exact live attempt before requesting fresh hardware authority.
    pub(crate) fn require_prepared(
        &self,
        transaction: &KagemushaPreparedHistoryCasV1,
    ) -> Result<(), KagemushaHistoryStoreErrorV1> {
        self.store.require_prepared(transaction)
    }

    /// Commit an already prepared CAS selected by a verified hardware certificate.
    pub(crate) fn commit_prepared(
        &mut self,
        certificate: VerifiedKagemushaHistoryRootSelectionV1,
    ) -> Result<KagemushaHistoryCommitOutcomeV1, KagemushaHistoryStoreErrorV1> {
        self.store.commit_prepared(certificate)
    }

    /// Resolve a prepared CAS after restart using its verified hardware certificate.
    pub(crate) fn recover_prepared(
        &mut self,
        certificate: VerifiedKagemushaHistoryRootSelectionV1,
    ) -> Result<KagemushaHistoryRecoveryOutcomeV1, KagemushaHistoryStoreErrorV1> {
        self.store.recover_prepared(certificate)
    }

    /// Abort one uncommitted CAS without changing either authoritative committed root.
    pub(crate) fn abort_prepared(
        &mut self,
        transaction_id: DigestV1,
    ) -> Result<KagemushaHistoryAbortOutcomeV1, KagemushaHistoryStoreErrorV1> {
        self.store.abort_prepared(transaction_id)
    }

    /// Describe the exact SHA-256/Pasta relation required for a replay-changing state proof.
    pub(crate) fn proof_root_bridge_request(
        &self,
        transaction: &KagemushaPreparedHistoryCasV1,
        operation_binding_digest: DigestV1,
        pasta_predecessor_replay_root: KagemushaPastaStateCommitmentV1,
        pasta_successor_replay_root: KagemushaPastaStateCommitmentV1,
    ) -> Result<KagemushaHistoryProofRootBridgeRequestV1, KagemushaHistoryStoreErrorV1> {
        let predecessor = self.store.committed_roots();
        let successor = transaction.successor_roots_from(predecessor)?;
        KagemushaHistoryProofRootBridgeRequestV1::new(
            transaction,
            operation_binding_digest,
            predecessor,
            successor,
            pasta_predecessor_replay_root,
            pasta_successor_replay_root,
        )
    }

    /// Return the underlying store to its owner without changing durable state.
    pub(crate) fn into_store(self) -> S {
        self.store
    }
}

// Runtime activation installs the authenticated recursive verifier and hardware provider. The
// explicit reject-all implementations remain the fail-closed default when either authority is
// absent; tests may supply narrow mocks through the same interfaces.

/// Kagemusha V1 state-machine version.
pub const KAGEMUSHA_STATE_VERSION_V1: u16 = 1;
/// Maximum opaque proof bytes accepted by a state-machine hook.
pub const KAGEMUSHA_PROOF_BUNDLE_MAX_BYTES_V1: usize = KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1;
/// Maximum opaque hardware GuardBundle bytes accepted by a state-machine hook.
pub const KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1: usize = 65_536;
/// Exact depth of the consumed-credit sparse-Merkle tree.
pub const KAGEMUSHA_CONSUMED_CREDIT_TREE_DEPTH_V1: usize = 256;

const STATE_COMMITMENT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:state-commitment\0";
const SNAPSHOT_COMMITMENT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:snapshot-commitment\0";
const BOOTSTRAP_STATEMENT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:bootstrap-statement\0";
const MINT_CREDIT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:mint-credit\0";
const CREDIT_ENVELOPE_DOMAIN: &[u8] = b"iroha:kagemusha:v1:peer-credit-envelope\0";
const TRANSITION_EFFECT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:transition-effect\0";
const TRANSITION_STATEMENT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:transition-statement\0";
const TRANSITION_LIFECYCLE_DOMAIN: &[u8] = b"iroha:kagemusha:v1:transition-lifecycle\0";
const TRANSPORT_STATEMENT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:transport-statement\0";
const EMPTY_DURABLE_EFFECT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:durable-effect:empty\0";
const TRANSITION_INTENT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:transition-intent\0";
const RECOVERY_RECORD_DOMAIN: &[u8] = b"iroha:kagemusha:v1:recovery-record\0";
const DURABLE_INBOX_EFFECT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:durable-inbox-effect\0";
const DURABLE_OUTBOX_EFFECT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:durable-outbox-effect\0";

/// Canonical 32-byte digest used by this state machine.
pub type DigestV1 = [u8; 32];

/// Opaque state-machine capability derived from one threshold-authenticated proof release.
///
/// Production callers cannot construct this from artifact digests. They must first authenticate
/// the complete release manifest and authority threshold, then derive this capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaStateProofReleaseV1 {
    artifacts: KagemushaRecursionArtifactsV1,
    enabled_profiles: Arc<[KagemushaEnabledProfileV1]>,
}

impl KagemushaStateProofReleaseV1 {
    /// Derive state-machine proof authority from an authenticated V1 release.
    ///
    /// # Errors
    ///
    /// Returns an error if canonical derivation fails or the release does not select the one
    /// common proof suite and verifier set required by the installed recursive artifact package.
    pub fn from_authenticated_release(
        release: &KagemushaAuthenticatedReleaseV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let canonical_empty_effect_digest =
            canonical_sha256_digest(EMPTY_DURABLE_EFFECT_DOMAIN, &release.release_id())?;
        Self::from_release_parts(
            KagemushaRecursionArtifactsV1::from_authenticated_release(
                release,
                canonical_empty_effect_digest,
            ),
            Arc::from(release.enabled_profiles()),
        )
    }

    /// Return the threshold-authenticated release identifier.
    #[must_use]
    pub const fn release_id(&self) -> DigestV1 {
        self.artifacts.release_id
    }

    /// Return the release-fixed digest representing an empty durable transition effect.
    #[must_use]
    pub const fn canonical_empty_effect_digest(&self) -> DigestV1 {
        self.artifacts.canonical_empty_effect_digest
    }

    #[cfg(test)]
    pub(crate) fn from_test_artifacts(
        artifacts: KagemushaRecursionArtifactsV1,
        enabled_profiles: Vec<KagemushaEnabledProfileV1>,
    ) -> Result<Self, KagemushaStateErrorV1> {
        Self::from_release_parts(artifacts, enabled_profiles.into())
    }

    fn from_release_parts(
        artifacts: KagemushaRecursionArtifactsV1,
        enabled_profiles: Arc<[KagemushaEnabledProfileV1]>,
    ) -> Result<Self, KagemushaStateErrorV1> {
        // A state-proof release installs one paired recursive artifact package. Allowing an
        // enabled profile to name another suite or verifier would create apparently valid
        // receiver credentials which this runtime cannot verify.
        let first = enabled_profiles
            .first()
            .ok_or(KagemushaStateErrorV1::InvalidHardwareProfile)?;
        if enabled_profiles.iter().any(|profile| {
            profile.suite_id != first.suite_id || profile.vk_digest != first.vk_digest
        }) || !enabled_profiles
            .windows(2)
            .all(|pair| pair[0].hardware_profile_id < pair[1].hardware_profile_id)
        {
            return Err(KagemushaStateErrorV1::InvalidHardwareProfile);
        }
        Ok(Self {
            artifacts,
            enabled_profiles,
        })
    }

    fn enabled_profile(&self, hardware_profile_id: DigestV1) -> Option<&KagemushaEnabledProfileV1> {
        self.enabled_profiles
            .binary_search_by_key(&hardware_profile_id, |profile| profile.hardware_profile_id)
            .ok()
            .map(|index| &self.enabled_profiles[index])
    }

    fn validate_state_context(
        &self,
        context: KagemushaStateContextV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        if context.release_id != self.release_id() {
            return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
        }
        let enabled = self
            .enabled_profile(context.hardware_profile_id)
            .ok_or(KagemushaStateErrorV1::InvalidHardwareProfile)?;
        if enabled.suite_id != context.suite_id
            || enabled.vk_digest != context.vk_digest
            || enabled.policy_epoch != context.policy_epoch
        {
            return Err(KagemushaStateErrorV1::InvalidHardwareProfile);
        }
        Ok(())
    }

    fn validate_payment_request(
        &self,
        state: &KagemushaStateV1,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.validate_state_context(state.context())?;
        if request.release_id != self.release_id() {
            return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
        }
        let credential = &request.hardware_credential;
        let enabled = self
            .enabled_profile(credential.hardware_profile_id)
            .ok_or(KagemushaStateErrorV1::InvalidHardwareProfile)?;
        if enabled.suite_id != credential.suite_id
            || enabled.vk_digest != state.vk_digest
            || enabled.policy_epoch != credential.policy_epoch
        {
            return Err(KagemushaStateErrorV1::InvalidHardwareProfile);
        }
        request
            .validate_against_profile(&enabled.hardware_profile)
            .map_err(|_| KagemushaStateErrorV1::InvalidHardwareProfile)
    }
}

/// Stable identity of one hardware lane and asset on one network.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub struct KagemushaLaneIdV1 {
    /// Exact typed network identity used by the public wire statement.
    pub network_id: NetworkId,
    /// Stable device-lane identity, retained across hardware epoch rotation.
    pub device_lane_id: DigestV1,
    /// Exact typed asset identity used by the public wire statement.
    pub asset: AssetDefinitionId,
    /// Authoritative decimal scale of the asset.
    pub scale: u32,
}

impl KagemushaLaneIdV1 {
    fn validate(&self) -> Result<(), KagemushaStateErrorV1> {
        if self.network_id.as_bytes() == &[0; 32]
            || self.device_lane_id == [0; 32]
            || self.scale > KAGEMUSHA_ASSET_SCALE_MAX_V1
        {
            return Err(KagemushaStateErrorV1::InvalidLane);
        }
        Ok(())
    }

    /// Return the exact normalized network identity bound by the recursive guard statement.
    #[must_use]
    pub fn normalized_network_id(&self) -> DigestV1 {
        *self.network_id.as_bytes()
    }

    /// Return the canonical normalized asset identity bound by the recursive guard statement.
    pub fn normalized_asset_id(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        kagemusha_asset_identity_digest_v1(&self.asset)
            .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)
    }
}

/// One attested hardware-key/counter epoch for a stable device lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct HardwareEpochV1 {
    /// Monotonically increasing epoch generation.
    pub generation: u128,
    /// Unique digest of the attested monotonic-counter epoch.
    pub epoch_id: DigestV1,
}

impl HardwareEpochV1 {
    fn validate(self) -> Result<(), KagemushaStateErrorV1> {
        if self.generation == 0 || self.epoch_id == [0; 32] {
            return Err(KagemushaStateErrorV1::InvalidHardwareEpoch);
        }
        Ok(())
    }
}

/// Exact device key and governed hardware policy bound to one private state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub struct DevicePolicyBindingV1 {
    /// Domain-separated reference to the currently authorized hardware key.
    pub device_key_reference: DigestV1,
    /// Exact governed hardware-policy identifier for the key and backend.
    pub hardware_policy_id: DigestV1,
}

impl DevicePolicyBindingV1 {
    fn validate(self) -> Result<(), KagemushaStateErrorV1> {
        if self.device_key_reference == [0; 32] || self.hardware_policy_id == [0; 32] {
            return Err(KagemushaStateErrorV1::InvalidDevicePolicyBinding);
        }
        Ok(())
    }
}

/// Globally unique identity of one inbound mint or peer credit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub struct CreditIdV1(
    /// Raw domain-separated credit identity digest.
    pub DigestV1,
);

impl CreditIdV1 {
    /// Return true when this is the forbidden all-zero identity.
    #[must_use]
    pub fn is_zero(self) -> bool {
        self.0 == [0; 32]
    }
}

/// Private release, suite, asset-incarnation, and hardware context carried by one aggregate.
///
/// Construction of this record does not itself grant monetary authority. Bootstrap and every
/// successor transition must prove the same values against an authenticated release and a
/// qualified hardware credential.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaStateContextV1 {
    /// Exact protocol version.
    pub protocol_version: u16,
    /// Governed proof-suite identity.
    pub suite_id: DigestV1,
    /// Digest of the complete verifying-key set.
    pub vk_digest: DigestV1,
    /// Threshold-authenticated proof-release identity.
    pub release_id: DigestV1,
    /// Exact asset incarnation.
    pub asset_incarnation: AxtAssetIncarnationV1,
    /// Qualified non-forking hardware profile.
    pub hardware_profile_id: DigestV1,
    /// Governed hardware-policy epoch.
    pub policy_epoch: u64,
}

impl KagemushaStateContextV1 {
    fn validate(self) -> Result<(), KagemushaStateErrorV1> {
        if self.protocol_version != KAGEMUSHA_STATE_VERSION_V1
            || self.suite_id == [0; 32]
            || self.vk_digest == [0; 32]
            || self.release_id == [0; 32]
            || self.hardware_profile_id == [0; 32]
            || self.policy_epoch == 0
        {
            return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
        }
        self.asset_incarnation
            .validate()
            .map_err(|_| KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool)?;
        Ok(())
    }
}

/// Private aggregate balance state for one device lane and asset.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaStateV1 {
    /// State-machine version.
    pub version: u16,
    /// Exact kagemusha protocol version carried by the authenticated release.
    pub protocol_version: u16,
    /// Governed proof suite used to verify and extend this aggregate state.
    pub suite_id: DigestV1,
    /// Digest of the complete verifying-key set for `suite_id`.
    pub vk_digest: DigestV1,
    /// Threshold-authenticated recursive-proof release identifier.
    pub release_id: DigestV1,
    /// Exact asset incarnation. A reissued asset never aliases an older liability pool.
    pub asset_incarnation: AxtAssetIncarnationV1,
    /// Deterministic reserve liability pool for this network and asset.
    pub liability_pool_id: DigestV1,
    /// Qualified non-forking hardware profile controlling the current state.
    pub hardware_profile_id: DigestV1,
    /// Governed hardware-policy epoch controlling the current state.
    pub policy_epoch: u64,
    /// Stable device-lane and asset scope.
    pub lane: KagemushaLaneIdV1,
    /// Available private balance in atomic units. Zero is a valid successor.
    pub balance: u128,
    /// Exact-next logical monetary transition sequence within the current hardware epoch.
    /// Authenticated rotation resets it to zero while carrying the full balance and replay root.
    pub logical_sequence: u128,
    /// Current attested hardware epoch.
    pub hardware_epoch: HardwareEpochV1,
    /// Current hardware-key and policy binding.
    pub device_policy_binding: DevicePolicyBindingV1,
    /// Hiding commitment to fresh private nonce material unique to this state successor.
    pub state_nonce_commitment: DigestV1,
    /// Root of the exact sparse-Merkle consumed-credit dictionary.
    pub consumed_credit_root: KagemushaPastaStateCommitmentV1,
    /// Paired native Poseidon commitments to every preceding state field.
    pub state_commitment_components: KagemushaPastaStateCommitmentV1,
    /// Compact SHA-256 name of `state_commitment_components` used by peer and settlement wires.
    pub state_commitment: DigestV1,
}

impl KagemushaStateV1 {
    /// Return the private lifecycle context that every successor must carry unchanged unless a
    /// recursively authorized suite or hardware-profile transition explicitly replaces it.
    #[must_use]
    pub const fn context(&self) -> KagemushaStateContextV1 {
        KagemushaStateContextV1 {
            protocol_version: self.protocol_version,
            suite_id: self.suite_id,
            vk_digest: self.vk_digest,
            release_id: self.release_id,
            asset_incarnation: self.asset_incarnation,
            hardware_profile_id: self.hardware_profile_id,
            policy_epoch: self.policy_epoch,
        }
    }

    fn build(
        context: KagemushaStateContextV1,
        liability_pool_id: DigestV1,
        lane: KagemushaLaneIdV1,
        balance: u128,
        logical_sequence: u128,
        hardware_epoch: HardwareEpochV1,
        device_policy_binding: DevicePolicyBindingV1,
        state_nonce_commitment: DigestV1,
        consumed_credit_root: KagemushaPastaStateCommitmentV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        lane.validate()?;
        context.validate()?;
        if liability_pool_id != derive_liability_pool_id(&lane, context.asset_incarnation)? {
            return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
        }
        hardware_epoch.validate()?;
        device_policy_binding.validate()?;
        if state_nonce_commitment == [0; 32] {
            return Err(KagemushaStateErrorV1::InvalidStateNonceCommitment);
        }
        let mut state = Self {
            version: KAGEMUSHA_STATE_VERSION_V1,
            protocol_version: context.protocol_version,
            suite_id: context.suite_id,
            vk_digest: context.vk_digest,
            release_id: context.release_id,
            asset_incarnation: context.asset_incarnation,
            liability_pool_id,
            hardware_profile_id: context.hardware_profile_id,
            policy_epoch: context.policy_epoch,
            lane,
            balance,
            logical_sequence,
            hardware_epoch,
            device_policy_binding,
            state_nonce_commitment,
            consumed_credit_root,
            state_commitment_components: KagemushaPastaStateCommitmentV1::ZERO,
            state_commitment: [0; 32],
        };
        let (components, commitment) = state.recompute_commitment()?;
        state.state_commitment_components = components;
        state.state_commitment = commitment;
        Ok(state)
    }

    /// Validate identity fields and the complete deterministic state commitment.
    pub fn validate(&self) -> Result<(), KagemushaStateErrorV1> {
        if self.version != KAGEMUSHA_STATE_VERSION_V1 {
            return Err(KagemushaStateErrorV1::UnsupportedVersion(self.version));
        }
        self.lane.validate()?;
        self.asset_incarnation
            .validate()
            .map_err(|_| KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool)?;
        if self.protocol_version != KAGEMUSHA_STATE_VERSION_V1
            || self.suite_id == [0; 32]
            || self.vk_digest == [0; 32]
            || self.release_id == [0; 32]
            || self.hardware_profile_id == [0; 32]
            || self.policy_epoch == 0
            || self.liability_pool_id
                != derive_liability_pool_id(&self.lane, self.asset_incarnation)?
        {
            return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
        }
        self.hardware_epoch.validate()?;
        self.device_policy_binding.validate()?;
        if self.state_nonce_commitment == [0; 32] {
            return Err(KagemushaStateErrorV1::InvalidStateNonceCommitment);
        }
        let (components, commitment) = self.recompute_commitment()?;
        if self.state_commitment_components != components || self.state_commitment != commitment {
            return Err(KagemushaStateErrorV1::StateCommitmentMismatch);
        }
        Ok(())
    }

    fn recompute_commitment(
        &self,
    ) -> Result<(KagemushaPastaStateCommitmentV1, DigestV1), KagemushaStateErrorV1> {
        let eq = self.recompute_parity_commitment::<halo2_proofs::halo2curves::pasta::Fp>(
            self.consumed_credit_root.eq,
        )?;
        let ep = self.recompute_parity_commitment::<halo2_proofs::halo2curves::pasta::Fq>(
            self.consumed_credit_root.ep,
        )?;
        let components = KagemushaPastaStateCommitmentV1 {
            eq: encode_pasta(eq),
            ep: encode_pasta(ep),
        };
        Ok((components, kagemusha_pasta_state_commitment_v1(components)))
    }

    fn recompute_parity_commitment<F>(
        &self,
        replay_root: DigestV1,
    ) -> Result<F, KagemushaStateErrorV1>
    where
        F: KagemushaPoseidonFieldV1,
    {
        let replay_root =
            decode_pasta::<F>(replay_root).ok_or(KagemushaStateErrorV1::StateCommitmentMismatch)?;
        let asset_id = self.lane.normalized_asset_id()?;
        let mut inputs = Vec::with_capacity(34);
        inputs.push(F::from(u64::from(self.version)));
        inputs.push(F::from(u64::from(self.protocol_version)));
        inputs.extend(digest_limbs::<F>(self.suite_id));
        inputs.extend(digest_limbs::<F>(self.vk_digest));
        inputs.extend(digest_limbs::<F>(self.release_id));
        inputs.extend(digest_limbs::<F>(*self.asset_incarnation.as_bytes()));
        inputs.extend(digest_limbs::<F>(self.liability_pool_id));
        inputs.extend(digest_limbs::<F>(self.hardware_profile_id));
        inputs.push(F::from(self.policy_epoch));
        inputs.extend(digest_limbs::<F>(self.lane.normalized_network_id()));
        inputs.extend(digest_limbs::<F>(asset_id));
        inputs.push(F::from(u64::from(self.lane.scale)));
        inputs.extend(digest_limbs::<F>(self.lane.device_lane_id));
        inputs.push(pasta_from_u128(self.balance));
        inputs.push(pasta_from_u128(self.logical_sequence));
        inputs.push(pasta_from_u128(self.hardware_epoch.generation));
        inputs.extend(digest_limbs::<F>(self.hardware_epoch.epoch_id));
        inputs.extend(digest_limbs::<F>(
            self.device_policy_binding.device_key_reference,
        ));
        inputs.extend(digest_limbs::<F>(
            self.device_policy_binding.hardware_policy_id,
        ));
        inputs.extend(digest_limbs::<F>(self.state_nonce_commitment));
        inputs.push(replay_root);
        Ok(pasta_hash(KAGEMUSHA_STATE_DOMAIN_V1, &inputs))
    }
}

/// Closed set of aggregate balance transitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub enum KagemushaTransitionKindV1 {
    /// Fold a finalized on-chain mint credit into the aggregate.
    MintFold,
    /// Split one receiver-bound peer credit from the aggregate.
    SendSplit,
    /// Fold one durably staged peer credit into the aggregate.
    ReceiveFold,
    /// Split one chain-facing redemption voucher from the aggregate.
    RedeemSplit,
    /// Move the unchanged aggregate to the exact next hardware epoch.
    Rotate,
}

/// Public recursive-proof statement derived by Core for one transition.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct TransitionProofStatementV1 {
    /// State-machine version.
    pub version: u16,
    /// Exact kagemusha protocol version.
    pub protocol_version: u16,
    /// Governed proof-suite identity consumed by the private predecessor.
    pub predecessor_suite_id: DigestV1,
    /// Digest of the complete predecessor verifying-key set.
    pub predecessor_vk_digest: DigestV1,
    /// Governed proof-suite identity installed in the private successor.
    pub successor_suite_id: DigestV1,
    /// Digest of the complete successor verifying-key set.
    pub successor_vk_digest: DigestV1,
    /// Transition relation selected by the proof.
    pub kind: KagemushaTransitionKindV1,
    /// Exact monetary amount; zero only for hardware-epoch rotation.
    pub amount: u128,
    /// Canonical finalized-mint statement digest, nonzero only for `MintFold`.
    pub mint_finality_semantic_digest: DigestV1,
    /// Exact paired mint-helper proof binding, nonzero only for `MintFold`.
    pub mint_finality_proof_binding_digest: DigestV1,
    /// Receiver-bound peer credit identifier, nonzero only for `SendSplit`.
    pub peer_credit_id: DigestV1,
    /// For `SendSplit`, the exact recipient encryption key signed in `PaymentRequestV1`;
    /// zero otherwise.
    pub recipient_encryption_key_binding: DigestV1,
    /// Complete released lifecycle binding used by terminal operations.
    pub lifecycle_binding_digest: DigestV1,
    /// Digest of the sealed, locally verified prepared transition.
    pub prepared_transition_binding_digest: DigestV1,
    /// Binding of the exact received credit.
    pub receive_credit_binding_digest: DigestV1,
    /// Authenticated proof release consumed by the predecessor state.
    /// It equals `release_id` in V1.
    pub predecessor_release_id: DigestV1,
    /// Authenticated proof release installed for the successor state.
    pub release_id: DigestV1,
    /// Exact asset incarnation carried by the aggregate state.
    pub asset_incarnation: AxtAssetIncarnationV1,
    /// Canonical reserve liability pool carried by the aggregate state.
    pub liability_pool_id: DigestV1,
    /// Qualified hardware profile carried by the aggregate state.
    pub hardware_profile_id: DigestV1,
    /// Governed hardware-policy epoch carried by the aggregate state.
    pub policy_epoch: u64,
    /// Stable lane and asset scope.
    pub lane: KagemushaLaneIdV1,
    /// Consumed aggregate commitment.
    pub predecessor_commitment: DigestV1,
    /// Produced aggregate commitment.
    pub successor_commitment: DigestV1,
    /// Consumed logical sequence in the predecessor hardware epoch.
    pub predecessor_sequence: u128,
    /// Produced exact-next sequence, or zero for a hardware-epoch rotation.
    pub successor_sequence: u128,
    /// Hardware epoch consumed by the relation.
    pub predecessor_epoch: HardwareEpochV1,
    /// Hardware epoch produced by the relation.
    pub successor_epoch: HardwareEpochV1,
    /// Device key and policy consumed by the relation.
    pub predecessor_device_policy_binding: DevicePolicyBindingV1,
    /// Device key and policy produced by the relation.
    pub successor_device_policy_binding: DevicePolicyBindingV1,
    /// Hiding commitment to predecessor private nonce material.
    pub predecessor_state_nonce_commitment: DigestV1,
    /// Hiding commitment to fresh successor private nonce material.
    pub successor_state_nonce_commitment: DigestV1,
    /// Durable journal revision consumed by the relation.
    pub journal_revision_before: u128,
    /// Durable journal revision produced by the relation.
    pub journal_revision_after: u128,
    /// Domain-separated digest of the operation-specific public effect.
    pub effect_digest: DigestV1,
}

impl TransitionProofStatementV1 {
    /// Return the canonical domain-separated digest consumed by proof and guard verifiers.
    pub fn digest(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        transition_statement_digest(self)
    }
}

/// Exact statement that a hardware GuardBundle must authorize.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct HardwareTransitionStatementV1 {
    /// State-machine version.
    pub version: u16,
    /// Transition relation selected by the hardware guard.
    pub kind: KagemushaTransitionKindV1,
    /// Exact monetary amount; zero only for hardware-epoch rotation.
    pub amount: u128,
    /// Stable lane and asset scope.
    pub lane: KagemushaLaneIdV1,
    /// Consumed aggregate commitment.
    pub predecessor_commitment: DigestV1,
    /// Produced aggregate commitment.
    pub successor_commitment: DigestV1,
    /// Consumed logical sequence in the predecessor hardware epoch.
    pub predecessor_sequence: u128,
    /// Exact successor sequence, or zero for a hardware-epoch rotation.
    pub successor_sequence: u128,
    /// Hardware epoch consumed by the transition.
    pub predecessor_epoch: HardwareEpochV1,
    /// Hardware epoch produced by the transition.
    pub successor_epoch: HardwareEpochV1,
    /// Device key and policy consumed by the transition.
    pub predecessor_device_policy_binding: DevicePolicyBindingV1,
    /// Device key and policy produced by the transition.
    pub successor_device_policy_binding: DevicePolicyBindingV1,
    /// Hiding commitment to predecessor private nonce material.
    pub predecessor_state_nonce_commitment: DigestV1,
    /// Hiding commitment to fresh successor private nonce material.
    pub successor_state_nonce_commitment: DigestV1,
    /// Durable journal revision consumed by the transition.
    pub journal_revision_before: u128,
    /// Exact durable journal revision produced by the transition.
    pub journal_revision_after: u128,
    /// Digest of Core's structural state-transition statement.
    pub state_transition_digest: DigestV1,
    /// Canonical digest of the complete normalized GuardBundle statement.
    pub normalized_guard_statement_digest: DigestV1,
}

impl HardwareTransitionStatementV1 {
    fn validate_exact_next(&self) -> Result<(), KagemushaStateErrorV1> {
        if self.version != KAGEMUSHA_STATE_VERSION_V1 {
            return Err(KagemushaStateErrorV1::UnsupportedVersion(self.version));
        }
        self.lane.validate()?;
        self.predecessor_epoch.validate()?;
        self.successor_epoch.validate()?;
        self.predecessor_device_policy_binding.validate()?;
        self.successor_device_policy_binding.validate()?;
        if self.predecessor_commitment == [0; 32]
            || self.successor_commitment == [0; 32]
            || self.state_transition_digest == [0; 32]
            || self.normalized_guard_statement_digest == [0; 32]
            || self.predecessor_state_nonce_commitment == [0; 32]
            || self.successor_state_nonce_commitment == [0; 32]
            || self.predecessor_state_nonce_commitment == self.successor_state_nonce_commitment
        {
            return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
        }
        let is_value_preserving = self.kind == KagemushaTransitionKindV1::Rotate;
        if is_value_preserving != (self.amount == 0) {
            return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
        }
        match self.kind {
            KagemushaTransitionKindV1::Rotate => {
                if self.successor_sequence != 0
                    || self.journal_revision_after != 0
                    || self.successor_epoch.generation
                        != self
                            .predecessor_epoch
                            .generation
                            .checked_add(1)
                            .ok_or(KagemushaStateErrorV1::HardwareEpochOverflow)?
                    || self.successor_epoch.epoch_id == self.predecessor_epoch.epoch_id
                    || self.successor_device_policy_binding
                        == self.predecessor_device_policy_binding
                {
                    return Err(KagemushaStateErrorV1::InvalidHardwareRotation);
                }
            }
            _ => {
                if self.successor_sequence
                    != self
                        .predecessor_sequence
                        .checked_add(1)
                        .ok_or(KagemushaStateErrorV1::SequenceOverflow)?
                    || self.journal_revision_after
                        != self
                            .journal_revision_before
                            .checked_add(1)
                            .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?
                    || self.successor_epoch != self.predecessor_epoch
                    || self.successor_device_policy_binding
                        != self.predecessor_device_policy_binding
                {
                    return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
                }
            }
        }
        Ok(())
    }
}

/// Hardware transition statement plus its opaque platform GuardBundle.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct HardwareTransitionCertificateV1 {
    /// Exact statement signed or attested by the device hardware.
    pub statement: HardwareTransitionStatementV1,
    /// Canonical platform GuardBundle bytes.
    pub guard_bundle: Vec<u8>,
}

/// Proof and hardware authorization supplied to one state transition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionAuthorizationV1 {
    /// Exact hardware transition certificate.
    pub hardware_certificate: HardwareTransitionCertificateV1,
    /// Complete fixed-profile paired-Pasta recursive proof.
    pub proof: KagemushaPairedProofV1,
    /// Authenticated external-history selection, present only for a replay-changing transition.
    pub(super) authenticated_history: Option<KagemushaHistoryTransitionAuthorizationV1>,
}

impl TransitionAuthorizationV1 {
    /// Construct transition authorization without claiming an external-history root bridge.
    ///
    /// Mint/receive transitions fail closed with this constructor until the caller obtains the
    /// separate proof- and hardware-authenticated history capability.
    #[must_use]
    pub fn new(
        hardware_certificate: HardwareTransitionCertificateV1,
        proof: KagemushaPairedProofV1,
    ) -> Self {
        Self {
            hardware_certificate,
            proof,
            authenticated_history: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct KagemushaHistoryTransitionAuthorizationV1 {
    pub(super) root_selection: VerifiedKagemushaHistoryRootSelectionV1,
    pub(super) proof_root_bridge: VerifiedKagemushaHistoryProofRootBridgeV1,
}

/// Deterministic transition material that a prover and hardware guard must authorize.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionPreviewV1 {
    /// Expected successor private state.
    pub successor: KagemushaStateV1,
    /// Exact recursive-proof statement.
    pub proof_statement: TransitionProofStatementV1,
    /// Exact hardware transition statement.
    pub hardware_statement: HardwareTransitionStatementV1,
    /// Complete locally reconstructed normalized GuardBundle statement.
    pub normalized_guard_statement: KagemushaNormalizedGuardStatementV1,
    /// Exact common transport digest required from both paired proof parities.
    pub transport_semantic_digest: DigestV1,
    /// Journal revision installed on success.
    pub journal_revision_after: u128,
}

/// Caller-owned material needed to prepare one receiver-bound `SendSplit` transition.
///
/// Core derives the amount and receiver binding directly from the signed request,
/// then derives every state, lifecycle, credit, and proof-statement binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendSplitPreparationV1 {
    /// Signed receiver request authenticated against the active proof release.
    pub request: KagemushaPaymentRequestV1,
    /// Canonical recipient-only encrypted credit opening.
    pub encrypted_credit: Vec<u8>,
    /// Fresh proof-derived transition nullifier.
    pub transition_nullifier: DigestV1,
    /// Commitment to the amount-bound encrypted-credit semantics.
    pub ciphertext_commitment: DigestV1,
    /// Fresh hiding commitment for the sender successor state nonce.
    pub successor_state_nonce_commitment: DigestV1,
    /// Opaque public trusted-time or monotonic-lease commitment used by terminal hardware.
    pub commit_evidence: KagemushaCommitEvidenceV1,
    /// Private hardware reference instant bound into the recoverable transition intent.
    pub commit_authorization_reference_ms: u64,
    /// One-use physical outbox reservation for this exact send.
    pub outbox_reservation: KagemushaOutboxReservationV1,
    /// Digest of the hardware-private exact-once predecessor authorization.
    pub prepared_one_use_authorization_digest: DigestV1,
    /// Hardware-sealed private transition inputs retained for crash recovery.
    pub sealed_transition_inputs: Vec<u8>,
    /// Hardware-sealed deterministic proof and envelope recovery seeds.
    pub sealed_recovery_seeds: Vec<u8>,
}

/// Caller-owned material needed to prepare one full or partial `RedeemSplit` intent.
///
/// Core derives the private aggregate successor, terminal lifecycle, redemption identity,
/// proof statement, and normalized guard binding. Public aggregate heads never enter the
/// redemption statement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedeemSplitPreparationV1 {
    /// Positive amount to remove from the offline aggregate balance.
    pub amount: u128,
    /// Public online account credited by successful settlement.
    pub beneficiary: AccountId,
    /// Fresh proof-derived terminal nullifier.
    pub terminal_nullifier: DigestV1,
    /// Commitment to the public redemption claim and private proof output.
    pub redemption_commitment: DigestV1,
    /// Fresh hiding commitment for the sender successor state nonce.
    pub successor_state_nonce_commitment: DigestV1,
    /// Opaque public trusted-time or monotonic-lease commitment used by terminal hardware.
    pub commit_evidence: KagemushaCommitEvidenceV1,
    /// Private hardware reference instant bound into the recoverable transition intent.
    pub commit_authorization_reference_ms: u64,
    /// One-use physical outbox reservation for this exact redemption.
    pub outbox_reservation: KagemushaOutboxReservationV1,
    /// Digest of the hardware-private exact-once predecessor authorization.
    pub prepared_one_use_authorization_digest: DigestV1,
    /// Hardware-sealed private transition inputs retained for crash recovery.
    pub sealed_transition_inputs: Vec<u8>,
    /// Hardware-sealed deterministic proof and voucher recovery seeds.
    pub sealed_recovery_seeds: Vec<u8>,
}

/// Operation-specific bindings carried by the private transition proof.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TransitionAuxiliaryBindingsV1 {
    pub(crate) lifecycle_binding_digest: DigestV1,
    pub(crate) prepared_transition_binding_digest: DigestV1,
    pub(crate) receive_credit_binding_digest: DigestV1,
}

/// Exact private nonmembership-and-insert witness for one consumed credit.
///
/// The path is always 256 hashes and is ordered root-to-leaf: element zero is the sibling
/// immediately below the root for the credit ID's most-significant bit, while element 255 is the
/// sibling of the leaf. The predecessor leaf is the protocol-fixed empty leaf. The successor leaf
/// is the canonical domain-separated hash of `credit_id` and `envelope_digest`; neither leaf value
/// is supplied by an untrusted caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsumedCreditInsertWitnessV1 {
    /// Unique finalized mint or staged peer-credit identity.
    pub credit_id: CreditIdV1,
    /// Digest of the exact canonical credit envelope installed in the leaf.
    pub envelope_digest: DigestV1,
    /// Consumed sparse-Merkle root, reconstructed with the fixed empty leaf.
    pub predecessor_root: KagemushaPastaStateCommitmentV1,
    /// Produced sparse-Merkle root, reconstructed with the fixed present leaf.
    pub successor_root: KagemushaPastaStateCommitmentV1,
    /// Exact root-to-leaf sibling path selected by the credit ID bits.
    pub siblings_root_to_leaf:
        [KagemushaPastaStateCommitmentV1; KAGEMUSHA_CONSUMED_CREDIT_TREE_DEPTH_V1],
}

/// A credit-fold transition and its exact private replay-tree insert witness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreditFoldPreviewV1 {
    /// Deterministic public transition statements and expected successor.
    pub transition: TransitionPreviewV1,
    /// Exact nonmembership-and-insert witness consumed by the recursive circuit.
    pub replay_insert_witness: ConsumedCreditInsertWitnessV1,
    /// Prepared external replay-root CAS retained in the byte-bounded WAL.
    pub(crate) authenticated_history_transaction: KagemushaPreparedHistoryCasV1,
    /// Exact proof-authenticated logical operation and independent SHA/Pasta root association.
    pub(crate) proof_root_bridge_request: KagemushaHistoryProofRootBridgeRequestV1,
    /// Exact authenticated native-only mint material consumed by the recursive witness.
    mint_private_inputs: KagemushaMintFoldPrivateInputsV1,
    trusted_commit_time_ms: u64,
}

impl CreditFoldPreviewV1 {
    /// Borrow the exact authenticated native-only mint witness inputs.
    pub(crate) fn mint_private_inputs(&self) -> &KagemushaMintFoldPrivateInputsV1 {
        &self.mint_private_inputs
    }

    /// Borrow the opaque recursive-opening capability from this exact checked preview.
    #[must_use]
    pub fn mint_fold_opening(&self) -> KagemushaMintFoldOpeningCapabilityV1<'_> {
        self.mint_private_inputs.opening_capability()
    }
}

/// Exact statement authorizing durable receipt of one peer credit.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct CreditStageStatementV1 {
    /// State-machine version.
    pub version: u16,
    /// Receiving lane and asset.
    pub recipient_lane: KagemushaLaneIdV1,
    /// Current receiver state at staging time; not part of the sender credit.
    pub receiver_state_commitment: DigestV1,
    /// Current receiver hardware epoch at staging time.
    pub receiver_hardware_epoch: HardwareEpochV1,
    /// Current receiver device-key and hardware-policy binding at staging time.
    pub receiver_device_policy_binding: DevicePolicyBindingV1,
    /// Commitment to current receiver private nonce material at staging time.
    pub receiver_state_nonce_commitment: DigestV1,
    /// Staged credit identity.
    pub credit_id: CreditIdV1,
    /// Digest of the exact received envelope bytes.
    pub envelope_digest: DigestV1,
    /// Local trusted staging time. It may be later than request expiry.
    pub staged_at_ms: u64,
    /// Durable journal revision consumed by staging.
    pub journal_revision_before: u128,
    /// Exact durable journal revision produced by staging.
    pub journal_revision_after: u128,
}

/// Hardware authorization for durable, replay-safe credit staging.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct CreditStageCertificateV1 {
    /// Exact stage statement.
    pub statement: CreditStageStatementV1,
    /// Canonical platform GuardBundle bytes.
    pub guard_bundle: Vec<u8>,
}

/// One peer credit retained in the authenticated pending inbox.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct StagedCreditV1 {
    /// Exact signed recipient request which authorized the payment.
    pub request: KagemushaPaymentRequestV1,
    /// Exact public sender payment envelope.
    pub payment: KagemushaPaymentV1,
    /// Exact receiver-only plaintext recovered after authenticated decryption.
    pub credit_opening: KagemushaCreditOpeningV1,
    /// Digest used for duplicate/conflict classification.
    pub envelope_digest: DigestV1,
    /// Receiver hardware staging certificate.
    pub stage_certificate: CreditStageCertificateV1,
}

/// Exact signed acknowledgement retained for byte-identical duplicate delivery.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct DurableAcknowledgementV1 {
    /// Validated public acknowledgement.
    pub acknowledgement: KagemushaAcknowledgementV1,
    /// Exact canonical Norito bytes returned to transport.
    pub canonical_bytes: Vec<u8>,
}

impl DurableAcknowledgementV1 {
    fn from_acknowledgement(
        acknowledgement: KagemushaAcknowledgementV1,
        request: &KagemushaPaymentRequestV1,
        payment: &KagemushaPaymentV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        acknowledgement
            .validate_shape_against(request, payment)
            .map_err(|_| KagemushaStateErrorV1::InvalidAcknowledgement)?;
        let canonical_bytes = norito::encode_canonical(&acknowledgement)
            .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
        Ok(Self {
            acknowledgement,
            canonical_bytes,
        })
    }

    fn validate_against(
        &self,
        request: &KagemushaPaymentRequestV1,
        payment: &KagemushaPaymentV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.acknowledgement
            .validate_shape_against(request, payment)
            .map_err(|_| KagemushaStateErrorV1::InvalidAcknowledgement)?;
        if self.canonical_bytes
            != norito::encode_canonical(&self.acknowledgement)
                .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }
}

/// Complete durable replay record needed to reproduce one receiver acknowledgement.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct AcceptedPaymentReceiptV1 {
    /// Unique receiver-bound credit identity.
    pub credit_id: CreditIdV1,
    /// Exact public payment digest used for conflict classification.
    pub envelope_digest: DigestV1,
    /// Signed request authorizing the payment.
    pub request: KagemushaPaymentRequestV1,
    /// Exact public payment accepted by the receiver.
    pub payment: KagemushaPaymentV1,
    /// Exact receiver-only plaintext recovered after authenticated decryption.
    pub credit_opening: KagemushaCreditOpeningV1,
    /// Hardware certificate for the rollback-resistant inbox insertion.
    pub stage_certificate: CreditStageCertificateV1,
    /// Byte-identical signed acknowledgement returned for every duplicate.
    pub durable_acknowledgement: DurableAcknowledgementV1,
}

/// Hardware result supplied only when staging a previously unseen payment.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct PaymentStageAuthorizationV1 {
    /// Atomic rollback-resistant inbox transition certificate.
    pub stage_certificate: CreditStageCertificateV1,
    /// Receiver acknowledgement signed after irreversible secure staging.
    pub acknowledgement: KagemushaAcknowledgementV1,
}

/// Durable staging outcome for an inbound public payment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StagePaymentOutcomeV1 {
    /// A new payment was durably staged at this receiver-inbox revision.
    Staged {
        /// Rollback-resistant inbox revision produced by staging the payment.
        inbox_revision: u128,
        /// Exact acknowledgement bytes safe to expose after staging.
        acknowledgement: DurableAcknowledgementV1,
    },
    /// Byte-identical transport retry of a still-pending payment.
    DuplicatePending {
        /// Existing rollback-resistant inbox revision at which the payment was staged.
        inbox_revision: u128,
        /// Previously persisted acknowledgement; it is never regenerated.
        acknowledgement: DurableAcknowledgementV1,
    },
    /// Byte-identical transport retry of an already-folded payment.
    DuplicateConsumed {
        /// Previously persisted acknowledgement; it is never regenerated.
        acknowledgement: DurableAcknowledgementV1,
    },
}

/// Exact local replay record committed into the sparse-Merkle tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct ConsumedCreditRecordV1 {
    /// Unique mint or peer credit identity.
    pub credit_id: CreditIdV1,
    /// Digest of the exact mint or peer credit envelope consumed under this identity.
    pub envelope_digest: DigestV1,
}

/// Fail-closed hook for platform hardware guards and rollback anchors.
pub trait KagemushaGuardBundleVerifierV1 {
    /// Verify an exact pre-debit mint allocation and sealed recipient/key ownership.
    /// Signing an authorization without this non-forking transaction is insufficient.
    fn verify_mint_reservation(
        &self,
        _statement: &MintReservationStatementV1,
        _guard_bundle: &[u8],
    ) -> Result<(), String> {
        Err("KAGEMUSHA qualified mint reservation verifier is unavailable".to_owned())
    }

    /// Verify atomic durable mint staging under an existing sealed allocation.
    fn verify_mint_stage(
        &self,
        _statement: &MintStageStatementV1,
        _guard_bundle: &[u8],
    ) -> Result<(), String> {
        Err("KAGEMUSHA qualified mint inbox verifier is unavailable".to_owned())
    }

    /// Verify initial device registration and bootstrap authorization.
    fn verify_bootstrap(
        &self,
        statement: &BootstrapStatementV1,
        guard_bundle: &[u8],
    ) -> Result<(), String>;

    /// Verify an exact-next monetary or rotation transition.
    fn verify_transition(
        &self,
        statement: &HardwareTransitionStatementV1,
        guard_bundle: &[u8],
    ) -> Result<(), String>;

    /// Verify atomic durable staging of one exact credit envelope.
    fn verify_credit_stage(
        &self,
        statement: &CreditStageStatementV1,
        guard_bundle: &[u8],
    ) -> Result<(), String>;

    /// Verify a hardware-sealed crash-recovery anchor.
    fn verify_durability_anchor(
        &self,
        statement: &DurabilityAnchorStatementV1,
        guard_bundle: &[u8],
    ) -> Result<(), String>;
}

/// Reject-all GuardBundle verifier used until a qualified device backend is installed.
#[derive(Clone, Copy, Debug, Default)]
pub struct RejectAllKagemushaGuardBundleVerifierV1;

impl KagemushaGuardBundleVerifierV1 for RejectAllKagemushaGuardBundleVerifierV1 {
    fn verify_bootstrap(
        &self,
        _statement: &BootstrapStatementV1,
        _guard_bundle: &[u8],
    ) -> Result<(), String> {
        Err("Kagemusha V1 hardware bootstrap verifier is unavailable".to_owned())
    }

    fn verify_transition(
        &self,
        _statement: &HardwareTransitionStatementV1,
        _guard_bundle: &[u8],
    ) -> Result<(), String> {
        Err("Kagemusha V1 hardware transition verifier is unavailable".to_owned())
    }

    fn verify_credit_stage(
        &self,
        _statement: &CreditStageStatementV1,
        _guard_bundle: &[u8],
    ) -> Result<(), String> {
        Err("Kagemusha V1 hardware staging verifier is unavailable".to_owned())
    }

    fn verify_durability_anchor(
        &self,
        _statement: &DurabilityAnchorStatementV1,
        _guard_bundle: &[u8],
    ) -> Result<(), String> {
        Err("Kagemusha V1 hardware durability verifier is unavailable".to_owned())
    }
}

/// Exact device bootstrap statement.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct BootstrapStatementV1 {
    /// State-machine version.
    pub version: u16,
    /// Exact kagemusha protocol version.
    pub protocol_version: u16,
    /// Governed proof-suite identity.
    pub suite_id: DigestV1,
    /// Digest of the complete verifying-key set.
    pub vk_digest: DigestV1,
    /// Threshold-authenticated recursive-proof release.
    pub release_id: DigestV1,
    /// Exact asset incarnation.
    pub asset_incarnation: AxtAssetIncarnationV1,
    /// Deterministic reserve liability pool for the lane asset.
    pub liability_pool_id: DigestV1,
    /// Qualified non-forking hardware profile.
    pub hardware_profile_id: DigestV1,
    /// Governed hardware-policy epoch.
    pub policy_epoch: u64,
    /// Stable lane and asset scope.
    pub lane: KagemushaLaneIdV1,
    /// Initial hardware epoch.
    pub hardware_epoch: HardwareEpochV1,
    /// Initial device key and governed hardware-policy binding.
    pub device_policy_binding: DevicePolicyBindingV1,
    /// Hiding commitment to fresh private nonce material of the initial zero state.
    pub state_nonce_commitment: DigestV1,
    /// Unique zero-state commitment.
    pub state_commitment: DigestV1,
}

impl BootstrapStatementV1 {
    /// Return the exact statement digest that the bootstrap proof must authorize.
    pub fn proof_statement_digest(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        canonical_sha256_digest(BOOTSTRAP_STATEMENT_DOMAIN, self)
    }
}

/// Complete locally derived bootstrap instance awaiting recursive and hardware authorization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootstrapPreviewV1 {
    /// Unique private zero-balance successor state.
    pub state: KagemushaStateV1,
    /// Core bootstrap semantic statement.
    pub statement: BootstrapStatementV1,
    /// Complete normalized GuardBundle statement with canonical null predecessor and 0/0 base.
    pub normalized_guard_statement: KagemushaNormalizedGuardStatementV1,
    /// Exact common transport digest required from both paired proof parities.
    pub transport_semantic_digest: DigestV1,
}

/// Proof and hardware authorization for a new zero-balance lane.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct BootstrapAuthorizationV1 {
    /// Complete fixed-profile paired-Pasta bootstrap proof.
    pub proof: KagemushaPairedProofV1,
    /// Platform hardware registration GuardBundle bytes.
    pub guard_bundle: Vec<u8>,
}

/// One hardware-sealed recovery statement.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct DurabilityAnchorStatementV1 {
    /// State-machine version.
    pub version: u16,
    /// Stable lane and asset scope.
    pub lane: KagemushaLaneIdV1,
    /// Current aggregate commitment.
    pub state_commitment: DigestV1,
    /// Current attested hardware epoch.
    pub hardware_epoch: HardwareEpochV1,
    /// Current device-key and governed hardware-policy binding.
    pub device_policy_binding: DevicePolicyBindingV1,
    /// Commitment to current private nonce material.
    pub state_nonce_commitment: DigestV1,
    /// Current logical sequence.
    pub logical_sequence: u128,
    /// Current durable journal revision.
    pub journal_revision: u128,
    /// Current rollback-resistant accepted-credit inbox revision within this hardware epoch.
    pub inbox_revision: u128,
    /// Commitment to the complete canonical recovery snapshot.
    pub snapshot_commitment: DigestV1,
}

/// Hardware-sealed recovery anchor that detects an older snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct DurabilityAnchorV1 {
    /// Exact anchor statement.
    pub statement: DurabilityAnchorStatementV1,
    /// Canonical platform GuardBundle bytes.
    pub guard_bundle: Vec<u8>,
}

/// Canonical crash-recovery projection for one aggregate lane.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaStateSnapshotV1 {
    /// State-machine version.
    pub version: u16,
    /// Current private aggregate state.
    pub state: KagemushaStateV1,
    /// Current durable journal revision.
    pub journal_revision: u128,
    /// Current rollback-resistant accepted-credit inbox revision within this hardware epoch.
    pub inbox_revision: u128,
    /// Pending credits in strict credit-id order.
    pub pending_credits: Vec<StagedCreditV1>,
    /// Historical recipient key/policy bindings accepted by this stable lane, in strict order.
    ///
    /// Retaining prior bindings ensures a payment committed against an in-window request remains
    /// stageable after an offline hardware-epoch rotation.
    pub accepted_recipient_bindings: Vec<DevicePolicyBindingV1>,
    /// Durable receiver acknowledgements retained in strict credit-id order for idempotent retry.
    pub accepted_payment_receipts: Vec<AcceptedPaymentReceiptV1>,
    /// Pre-debit mint allocations and exact durable mint receipts, including old epochs.
    pub(crate) mint_inbox: KagemushaMintInboxV1,
    /// Consumed credits in strict credit-id order.
    pub consumed_credits: Vec<ConsumedCreditRecordV1>,
    /// Independently authenticated external replay and terminal-decision roots.
    pub(crate) authenticated_history_roots: KagemushaHistoryRootsV1,
    /// Exact successful history-operation order, including prepares and abort tombstones.
    pub(crate) authenticated_history_commitment: DigestV1,
    /// Physical receiver inbox capacity charged by exact canonical snapshot bytes.
    pub receiver_inbox_capacity: KagemushaReceiverInboxCapacityV1,
    /// Durable sender capacity reservations and terminal-envelope bindings.
    pub sender_outbox_capacity: KagemushaSenderOutboxCapacityV1,
    /// Exact recoverable sender prepare/prove/commit/finalize journal.
    pub outgoing_candidate_journal: KagemushaOutgoingCandidateJournalV1,
    /// Poseidon commitment to every preceding snapshot field.
    pub snapshot_commitment: DigestV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode)]
struct SnapshotCommitmentPreimageV1 {
    version: u16,
    state: KagemushaStateV1,
    journal_revision: u128,
    inbox_revision: u128,
    pending_credits: Vec<StagedCreditV1>,
    accepted_recipient_bindings: Vec<DevicePolicyBindingV1>,
    accepted_payment_receipts: Vec<AcceptedPaymentReceiptV1>,
    mint_inbox: KagemushaMintInboxV1,
    consumed_credits: Vec<ConsumedCreditRecordV1>,
    authenticated_history_roots: KagemushaHistoryRootsV1,
    authenticated_history_commitment: DigestV1,
    receiver_inbox_capacity: KagemushaReceiverInboxCapacityV1,
    sender_outbox_capacity: KagemushaSenderOutboxCapacityV1,
    outgoing_candidate_journal: KagemushaOutgoingCandidateJournalV1,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ReceiverSnapshotCapacityUsageV1 {
    total_bytes: u64,
    pending_credit_entry_bytes: u64,
    pending_receipt_entry_bytes: u64,
}

fn receiver_snapshot_capacity_usage_v1(
    pending_credits: &BTreeMap<CreditIdV1, StagedCreditV1>,
    accepted_payment_receipts: &BTreeMap<CreditIdV1, AcceptedPaymentReceiptV1>,
) -> Result<ReceiverSnapshotCapacityUsageV1, KagemushaStateErrorV1> {
    let pending_credit_entry_bytes = pending_credits.values().try_fold(
        0_u64,
        |total, staged| -> Result<u64, KagemushaStateErrorV1> {
            total
                .checked_add(receiver_sequence_entry_bytes(staged)?)
                .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)
        },
    )?;
    let pending_receipt_entry_bytes = pending_credits.keys().try_fold(
        0_u64,
        |total, credit_id| -> Result<u64, KagemushaStateErrorV1> {
            let receipt = accepted_payment_receipts
                .get(credit_id)
                .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
            total
                .checked_add(receiver_sequence_entry_bytes(receipt)?)
                .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)
        },
    )?;
    let total_bytes = candidate_lifecycle::receiver_snapshot_usage_from_entry_bytes(
        pending_credit_entry_bytes,
        pending_receipt_entry_bytes,
    )?;
    Ok(ReceiverSnapshotCapacityUsageV1 {
        total_bytes,
        pending_credit_entry_bytes,
        pending_receipt_entry_bytes,
    })
}

fn receiver_sequence_entry_bytes<T: norito::NoritoSerialize>(
    value: &T,
) -> Result<u64, KagemushaStateErrorV1> {
    let flags = norito::core::default_encode_flags();
    let _canonical_flags = norito::core::DecodeFlagsGuard::enter(flags);
    let payload_bytes = norito::core::encoded_payload_len(value)
        .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
    let framing_bytes = if norito::core::packed_seq_enabled_for_flags(flags) {
        core::mem::size_of::<u64>()
    } else {
        let mut encoded = Vec::with_capacity(10);
        norito::core::write_len_with_flags(
            &mut encoded,
            u64::try_from(payload_bytes).map_err(|_| KagemushaStateErrorV1::ArithmeticOverflow)?,
            flags,
        )
        .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
        encoded.len()
    };
    u64::try_from(
        payload_bytes
            .checked_add(framing_bytes)
            .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)?,
    )
    .map_err(|_| KagemushaStateErrorV1::ArithmeticOverflow)
}

fn map_authenticated_history_error(error: KagemushaHistoryStoreErrorV1) -> KagemushaStateErrorV1 {
    match error {
        KagemushaHistoryStoreErrorV1::AttemptNotPrepared(transaction_id) => {
            KagemushaStateErrorV1::AuthenticatedHistoryAttemptNotPrepared(transaction_id)
        }
        KagemushaHistoryStoreErrorV1::OverlayCapacityExceeded { .. } => {
            KagemushaStateErrorV1::AuthenticatedHistoryPreparedCapacityExhausted
        }
        KagemushaHistoryStoreErrorV1::StorageUnavailable
        | KagemushaHistoryStoreErrorV1::JournalCorrupt
        | KagemushaHistoryStoreErrorV1::DurabilityUncertain
        | KagemushaHistoryStoreErrorV1::StoreAlreadyOpen
        | KagemushaHistoryStoreErrorV1::RecoveryCommitmentMismatch
        | KagemushaHistoryStoreErrorV1::MissingSelectedRoot { .. }
        | KagemushaHistoryStoreErrorV1::MissingCommittedRoot { .. }
        | KagemushaHistoryStoreErrorV1::MissingHistoryNode { .. }
        | KagemushaHistoryStoreErrorV1::CorruptHistoryNode { .. }
        | KagemushaHistoryStoreErrorV1::InvalidHistoryTree { .. }
        | KagemushaHistoryStoreErrorV1::CommittedRootsMismatch { .. } => {
            KagemushaStateErrorV1::AuthenticatedHistoryUnavailable
        }
        _ => KagemushaStateErrorV1::StateInvariant,
    }
}

/// State-machine validation or authorization failure.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum KagemushaStateErrorV1 {
    /// A value carried an unsupported wire/state version.
    #[error("unsupported Kagemusha state version {0}")]
    UnsupportedVersion(u16),
    /// A network, device-lane, or asset identity was zero.
    #[error("invalid Kagemusha lane identity")]
    InvalidLane,
    /// The authenticated release or deterministic network-and-asset liability pool was invalid.
    #[error("invalid Kagemusha proof release or liability pool")]
    InvalidReleaseOrLiabilityPool,
    /// A hardware epoch was zero or malformed.
    #[error("invalid Kagemusha hardware epoch")]
    InvalidHardwareEpoch,
    /// A device-key reference or governed hardware-policy identity was zero.
    #[error("invalid Kagemusha device and hardware-policy binding")]
    InvalidDevicePolicyBinding,
    /// A private state nonce commitment was zero or reused by an immediate successor.
    #[error("invalid Kagemusha private state nonce commitment")]
    InvalidStateNonceCommitment,
    /// A requested rotation was not the exact next distinct epoch.
    #[error("Kagemusha hardware rotation is not exact-next")]
    InvalidHardwareRotation,
    /// Hardware epoch generation overflowed `u128`.
    #[error("Kagemusha hardware epoch overflow")]
    HardwareEpochOverflow,
    /// A private state commitment did not match its fields.
    #[error("Kagemusha state commitment mismatch")]
    StateCommitmentMismatch,
    /// A private state field relation was impossible for a valid history.
    #[error("Kagemusha private state invariant failed")]
    StateInvariant,
    /// Canonical Norito encoding failed.
    #[error("Kagemusha canonical encoding failed")]
    CanonicalEncoding,
    /// A proof was empty or exceeded its fixed bound.
    #[error("Kagemusha proof bundle is invalid")]
    InvalidProofBundle,
    /// A GuardBundle was empty or exceeded its fixed bound.
    #[error("Kagemusha GuardBundle is invalid")]
    InvalidGuardBundle,
    /// The configured governed proof verifier rejected.
    #[error("Kagemusha proof verifier rejected: {0}")]
    ProofRejected(String),
    /// The configured hardware guard verifier rejected.
    #[error("Kagemusha hardware guard rejected: {0}")]
    GuardRejected(String),
    /// The supplied certificate did not equal Core's exact transition.
    #[error("Kagemusha hardware certificate mismatch")]
    HardwareCertificateMismatch,
    /// Logical transition sequence overflowed `u128`.
    #[error("Kagemusha logical sequence overflow")]
    SequenceOverflow,
    /// Durable journal revision overflowed `u128`.
    #[error("Kagemusha durable journal revision overflow")]
    JournalRevisionOverflow,
    /// Balance or consumed-credit arithmetic overflowed `u128`.
    #[error("Kagemusha checked arithmetic overflow")]
    ArithmeticOverflow,
    /// A split attempted to consume more than the current aggregate balance.
    #[error("Kagemusha balance is insufficient")]
    InsufficientBalance,
    /// A receiver request was zero, expired at issue, or otherwise malformed.
    #[error("invalid Kagemusha payment request")]
    InvalidPaymentRequest,
    /// An authenticated release did not enable the required qualified hardware profile.
    #[error("Kagemusha hardware credential is not enabled by the authenticated release")]
    InvalidHardwareProfile,
    /// Receiver inbox capacity cannot durably stage another exact payment and acknowledgement.
    #[error("Kagemusha receiver inbox capacity is exhausted")]
    ReceiverCapacityExhausted,
    /// Durable storage cannot hold one complete receive and terminal operation.
    #[error("Kagemusha durable capacity is below the minimum complete-operation footprint")]
    InvalidDurableCapacity,
    /// Sender durable outbox capacity cannot back another terminal operation.
    #[error("Kagemusha sender outbox capacity is exhausted")]
    SenderOutboxCapacityExhausted,
    /// A transition attempted to skip or conflict with the durable outgoing-candidate lifecycle.
    #[error("invalid Kagemusha outgoing candidate lifecycle stage")]
    InvalidCandidateStage,
    /// Retried candidate material differs from the already persisted bytes.
    #[error("conflicting Kagemusha outgoing candidate retry")]
    CandidateConflict,
    /// Hardware-sealed transition or recovery material was empty or oversized.
    #[error("invalid Kagemusha sealed recovery material")]
    InvalidRecoveryMaterial,
    /// Sender commit time was outside the receiver-authorized interval.
    #[error("Kagemusha sender commit time is outside the request window")]
    SenderCommitOutsideRequestWindow,
    /// Qualified hardware supplied a zero trusted transition-commit time.
    #[error("invalid Kagemusha trusted hardware commit time")]
    InvalidTrustedCommitTime,
    /// A mint credit was malformed or targeted another lane.
    #[error("invalid Kagemusha mint credit")]
    InvalidMintCredit,
    /// A verified mint-finality capability did not authorize this exact mint statement.
    #[error("Kagemusha mint-finality verification does not match the folded credit")]
    MintFinalityMismatch,
    /// A peer credit was malformed or targeted another lane.
    #[error("invalid Kagemusha peer credit")]
    InvalidPeerCredit,
    /// A first delivery omitted its hardware staging authorization and signed acknowledgement.
    #[error("Kagemusha first payment delivery requires staging authorization")]
    MissingStageAuthorization,
    /// A receiver acknowledgement failed request, payment, receipt, sequence, or signature binding.
    #[error("invalid Kagemusha payment acknowledgement")]
    InvalidAcknowledgement,
    /// A redemption request was malformed.
    #[error("invalid Kagemusha redemption request")]
    InvalidRedemption,
    /// A redemption settlement status or its indexed outbox binding was not fully authenticated.
    #[error("invalid Kagemusha authenticated redemption settlement receipt")]
    InvalidRedemptionSettlementReceipt,
    /// A credit identity already committed the same envelope.
    #[error("Kagemusha credit {0:?} was already consumed")]
    CreditAlreadyConsumed(CreditIdV1),
    /// A credit identity was reused with different canonical bytes.
    #[error("Kagemusha credit {0:?} conflicts with retained bytes")]
    CreditConflict(CreditIdV1),
    /// A consumed-credit insert witness failed its exact key, leaf, path, or root relation.
    #[error("Kagemusha consumed-credit insert witness is invalid")]
    InvalidConsumedCreditInsertWitness,
    /// No pending credit exists for a requested receive fold.
    #[error("Kagemusha credit {0:?} is not staged")]
    CreditNotStaged(CreditIdV1),
    /// Snapshot fields, ordering, roots, counts, or commitment were inconsistent.
    #[error("Kagemusha recovery snapshot failed integrity validation")]
    SnapshotIntegrity,
    /// Authenticated external history was unavailable, corrupt, or inconsistent with recovery.
    #[error("Kagemusha authenticated external history failed closed")]
    AuthenticatedHistoryUnavailable,
    /// The byte-bounded external-history prepare/WAL overlay cannot accept new work.
    #[error("Kagemusha authenticated-history prepared-transition capacity is exhausted")]
    AuthenticatedHistoryPreparedCapacityExhausted,
    /// A retained attempt was aborted/committed and cannot request new hardware authority.
    #[error("KAGEMUSHA authenticated-history attempt is no longer prepared: {0:?}")]
    AuthenticatedHistoryAttemptNotPrepared(DigestV1),
    /// The proof-authenticated SHA-256/Pasta root association was missing or did not match.
    #[error("Kagemusha authenticated-history proof-root bridge is missing or mismatched")]
    AuthenticatedHistoryProofRootBridgeUnavailable,
    /// A snapshot did not match the latest hardware-sealed anchor.
    #[error("Kagemusha recovery snapshot is stale or from another lane")]
    SnapshotRollback,
}

/// Aggregate Kagemusha V1 state machine with governed recursion and hardware verifiers.
pub struct KagemushaStateMachineV1<R, G, H = KagemushaMemoryAuthenticatedHistoryStoreV1> {
    state: KagemushaStateV1,
    journal_revision: u128,
    inbox_revision: u128,
    pending_credits: BTreeMap<CreditIdV1, StagedCreditV1>,
    accepted_recipient_bindings: BTreeSet<DevicePolicyBindingV1>,
    accepted_payment_receipts: BTreeMap<CreditIdV1, AcceptedPaymentReceiptV1>,
    mint_inbox: KagemushaMintInboxV1,
    consumed_credits: ExactConsumedCreditIndex,
    authenticated_history: KagemushaStateAuthenticatedHistoryV1<H>,
    receiver_inbox_capacity: KagemushaReceiverInboxCapacityV1,
    sender_outbox_capacity: KagemushaSenderOutboxCapacityV1,
    outgoing_candidate_journal: KagemushaOutgoingCandidateJournalV1,
    proof_release: KagemushaStateProofReleaseV1,
    recursive_verifier: R,
    guard_verifier: G,
}

/// Compute the private store identity from the exact governed lane and state context.
#[cfg(unix)]
pub(crate) fn disk_history_lane_binding(
    context: KagemushaStateContextV1,
    lane: &KagemushaLaneIdV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    #[derive(Encode)]
    struct LaneBinding {
        version: u16,
        context: KagemushaStateContextV1,
        lane: KagemushaLaneIdV1,
    }
    context.validate()?;
    lane.validate()?;
    let encoded = norito::encode_canonical(&LaneBinding {
        version: KAGEMUSHA_STATE_VERSION_V1,
        context,
        lane: lane.clone(),
    })
    .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
    let mut hash = Sha256::new();
    hash.update(b"iroha:kagemusha:v1:history-disk:lane-binding\0");
    hash.update(encoded);
    Ok(hash.finalize().into())
}

#[cfg(unix)]
impl<R, G> KagemushaStateMachineV1<R, G, KagemushaDiskAuthenticatedHistoryStoreV1>
where
    R: KagemushaRecursiveVerifierV1,
    G: KagemushaGuardBundleVerifierV1,
{
    /// Restore a concrete durable state machine using the current authenticated hardware anchor.
    ///
    /// Historical device keys must be pinned by the Core owner, including the current state's
    /// exact epoch/key reference. Journal replay alone cannot grant authority: the existing
    /// restore path still verifies the guard, full snapshot commitment, both roots, and retained
    /// proof state. This function never initializes missing files or falls back to empty history.
    pub(crate) fn restore_from_disk_history(
        snapshot: KagemushaStateSnapshotV1,
        current_hardware_anchor: &DurabilityAnchorV1,
        proof_release: KagemushaStateProofReleaseV1,
        history_directory: &std::path::Path,
        history_credentials: KagemushaHistoryDeviceCredentialsV1,
        overlay_capacity_bytes: u64,
        recursive_verifier: R,
        guard_verifier: G,
    ) -> Result<Self, KagemushaStateErrorV1> {
        history_credentials
            .require_current_binding(
                snapshot.state.hardware_profile_id,
                snapshot.state.hardware_epoch.generation,
                snapshot.state.device_policy_binding.device_key_reference,
            )
            .map_err(map_authenticated_history_error)?;
        let lane_binding =
            disk_history_lane_binding(snapshot.state.context(), &snapshot.state.lane)?;
        let history_store = KagemushaDiskAuthenticatedHistoryStoreV1::open_existing(
            history_directory,
            lane_binding,
            history_credentials,
            overlay_capacity_bytes,
        )
        .map_err(map_authenticated_history_error)?;
        Self::restore(
            snapshot,
            current_hardware_anchor,
            proof_release,
            history_store,
            recursive_verifier,
            guard_verifier,
        )
    }
}

impl<R, G, H> KagemushaStateMachineV1<R, G, H>
where
    R: KagemushaRecursiveVerifierV1,
    G: KagemushaGuardBundleVerifierV1,
    H: KagemushaAuthenticatedHistoryStoreV1,
{
    /// Preview the unique zero-balance bootstrap state and exact authorization statement.
    pub fn preview_bootstrap(
        proof_release: KagemushaStateProofReleaseV1,
        state_context: KagemushaStateContextV1,
        lane: KagemushaLaneIdV1,
        hardware_epoch: HardwareEpochV1,
        device_policy_binding: DevicePolicyBindingV1,
        state_nonce_commitment: DigestV1,
        trusted_commit_time_ms: u64,
    ) -> Result<BootstrapPreviewV1, KagemushaStateErrorV1> {
        lane.validate()?;
        state_context.validate()?;
        proof_release.validate_state_context(state_context)?;
        hardware_epoch.validate()?;
        device_policy_binding.validate()?;
        if state_nonce_commitment == [0; 32] {
            return Err(KagemushaStateErrorV1::InvalidStateNonceCommitment);
        }
        if trusted_commit_time_ms == 0 {
            return Err(KagemushaStateErrorV1::InvalidTrustedCommitTime);
        }
        let consumed_credits = ExactConsumedCreditIndex::empty();
        let liability_pool_id = derive_liability_pool_id(&lane, state_context.asset_incarnation)?;
        let state = KagemushaStateV1::build(
            state_context,
            liability_pool_id,
            lane.clone(),
            0,
            0,
            hardware_epoch,
            device_policy_binding,
            state_nonce_commitment,
            consumed_credits.root(),
        )?;
        let statement = BootstrapStatementV1 {
            version: KAGEMUSHA_STATE_VERSION_V1,
            protocol_version: state_context.protocol_version,
            suite_id: state_context.suite_id,
            vk_digest: state_context.vk_digest,
            release_id: proof_release.release_id(),
            asset_incarnation: state_context.asset_incarnation,
            liability_pool_id,
            hardware_profile_id: state_context.hardware_profile_id,
            policy_epoch: state_context.policy_epoch,
            lane,
            hardware_epoch,
            device_policy_binding,
            state_nonce_commitment,
            state_commitment: state.state_commitment,
        };
        let context =
            bootstrap_guard_context(proof_release.artifacts, &statement, trusted_commit_time_ms)?;
        let normalized_guard_statement =
            KagemushaNormalizedGuardStatementV1::from_bootstrap_state(&statement, context)
                .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        let guard_digest = normalized_guard_statement
            .canonical_digest()
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        let transport_semantic_digest = transport_semantic_digest(guard_digest)?;
        Ok(BootstrapPreviewV1 {
            state,
            statement,
            normalized_guard_statement,
            transport_semantic_digest,
        })
    }

    /// Bootstrap a zero-balance lane after both proof and hardware registration verification.
    pub fn bootstrap(
        proof_release: KagemushaStateProofReleaseV1,
        state_context: KagemushaStateContextV1,
        lane: KagemushaLaneIdV1,
        hardware_epoch: HardwareEpochV1,
        device_policy_binding: DevicePolicyBindingV1,
        state_nonce_commitment: DigestV1,
        trusted_commit_time_ms: u64,
        durable_capacity: KagemushaDurableCapacityV1,
        history_store: H,
        authorization: BootstrapAuthorizationV1,
        recursive_verifier: R,
        guard_verifier: G,
    ) -> Result<Self, KagemushaStateErrorV1> {
        durable_capacity.validate()?;
        let authenticated_history = KagemushaStateAuthenticatedHistoryV1::open(history_store)
            .map_err(map_authenticated_history_error)?;
        if authenticated_history.committed_roots() != KagemushaHistoryRootsV1::empty() {
            return Err(KagemushaStateErrorV1::StateInvariant);
        }
        let preview = Self::preview_bootstrap(
            proof_release.clone(),
            state_context,
            lane,
            hardware_epoch,
            device_policy_binding,
            state_nonce_commitment,
            trusted_commit_time_ms,
        )?;
        validate_guard_bytes(&authorization.guard_bundle)?;
        let public_inputs =
            bootstrap_state_public_inputs(proof_release.artifacts, &preview, &authorization.proof)?;
        verify_kagemusha_state_proof_v1(
            &recursive_verifier,
            proof_release.artifacts,
            &public_inputs,
            &authorization.proof,
        )
        .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        guard_verifier
            .verify_bootstrap(&preview.statement, &authorization.guard_bundle)
            .map_err(KagemushaStateErrorV1::GuardRejected)?;
        Ok(Self {
            state: preview.state,
            journal_revision: 0,
            inbox_revision: 0,
            pending_credits: BTreeMap::new(),
            accepted_recipient_bindings: BTreeSet::from([device_policy_binding]),
            accepted_payment_receipts: BTreeMap::new(),
            mint_inbox: KagemushaMintInboxV1::default(),
            consumed_credits: ExactConsumedCreditIndex::empty(),
            authenticated_history,
            receiver_inbox_capacity: KagemushaReceiverInboxCapacityV1::new(
                durable_capacity.inbox_bytes,
            ),
            sender_outbox_capacity: KagemushaSenderOutboxCapacityV1::new(
                durable_capacity.outbox_bytes,
            ),
            outgoing_candidate_journal: KagemushaOutgoingCandidateJournalV1::default(),
            proof_release,
            recursive_verifier,
            guard_verifier,
        })
    }

    /// Borrow the current aggregate state.
    #[must_use]
    pub fn state(&self) -> &KagemushaStateV1 {
        &self.state
    }

    /// Return the current durable journal revision.
    #[must_use]
    pub fn journal_revision(&self) -> u128 {
        self.journal_revision
    }

    /// Return the current rollback-resistant accepted-credit inbox revision for this epoch.
    #[must_use]
    pub const fn inbox_revision(&self) -> u128 {
        self.inbox_revision
    }

    /// Borrow the physical receiver-inbox ledger included in every durability snapshot.
    #[must_use]
    pub const fn receiver_inbox_capacity(&self) -> &KagemushaReceiverInboxCapacityV1 {
        &self.receiver_inbox_capacity
    }

    /// Borrow the sender outbox-capacity ledger included in every durability snapshot.
    #[must_use]
    pub const fn sender_outbox_capacity(&self) -> &KagemushaSenderOutboxCapacityV1 {
        &self.sender_outbox_capacity
    }

    /// Borrow the exact recoverable outgoing journal.
    #[must_use]
    pub const fn outgoing_candidate_journal(&self) -> &KagemushaOutgoingCandidateJournalV1 {
        &self.outgoing_candidate_journal
    }

    /// Borrow the guarded caller-operation recovery index.
    #[must_use]
    pub const fn outgoing_operation_index(&self) -> &KagemushaOutgoingOperationIndexV1 {
        self.outgoing_candidate_journal.operation_index()
    }

    /// Classify an exact caller prepare retry before deriving any new monetary candidate.
    pub fn classify_outgoing_operation_prepare(
        &self,
        request: &KagemushaOutgoingPublicInputPreimageV1,
    ) -> KagemushaOutgoingOperationIndexResultV1<Option<&KagemushaOutgoingOperationRecordV1>> {
        self.outgoing_operation_index()
            .classify_existing_prepare(request)
    }

    /// Select an exact bounded operation page at one pinned guarded index revision.
    pub fn outgoing_operation_page(
        &self,
        pinned_revision: Option<u128>,
        after: Option<DigestV1>,
        maximum_entries: u16,
    ) -> KagemushaOutgoingOperationIndexResultV1<KagemushaOutgoingOperationPageV1> {
        self.outgoing_operation_index()
            .page(pinned_revision, after, maximum_entries)
    }

    /// Derive one complete, recoverable `SendSplit` intent without mutating monetary state.
    ///
    /// The signed request supplies the exact amount and recipient. Core authenticates its
    /// hardware profile, checks the trusted commit window and encrypted-credit framing, derives
    /// the aggregate successor and credit ID, and binds the exact normalized hardware statement.
    /// The returned candidate must still pass [`Self::prepare_indexed_outgoing_candidate`] before
    /// hardware may consume the predecessor.
    ///
    /// # Errors
    ///
    /// Returns an error for an unauthenticated or malformed request, an invalid ciphertext,
    /// insufficient folded balance, a reused state nonce, malformed recovery material, an
    /// incorrect outbox reservation, or a conflicting active outgoing intent.
    pub fn prepare_send_split(
        &self,
        preparation: SendSplitPreparationV1,
    ) -> Result<PreparedOutgoingCandidateV1, KagemushaStateErrorV1> {
        let SendSplitPreparationV1 {
            request,
            encrypted_credit,
            transition_nullifier,
            ciphertext_commitment,
            successor_state_nonce_commitment,
            commit_evidence,
            commit_authorization_reference_ms,
            outbox_reservation,
            prepared_one_use_authorization_digest,
            sealed_transition_inputs,
            sealed_recovery_seeds,
        } = preparation;

        request
            .validate_shape()
            .map_err(|_| KagemushaStateErrorV1::InvalidPaymentRequest)?;
        self.proof_release
            .validate_payment_request(&self.state, &request)?;
        candidate_lifecycle::validate_request_against_state(&request, &self.state)?;
        commit_evidence
            .validate()
            .map_err(|_| KagemushaStateErrorV1::InvalidTrustedCommitTime)?;
        if commit_authorization_reference_ms == 0
            || prepared_one_use_authorization_digest == [0; 32]
        {
            return Err(KagemushaStateErrorV1::InvalidPaymentRequest);
        }
        KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
            &encrypted_credit,
            request.recipient_encryption_key,
        )
        .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;

        let amount = request.amount;
        let successor_balance = self
            .state
            .balance
            .checked_sub(amount)
            .ok_or(KagemushaStateErrorV1::InsufficientBalance)?;
        let successor = self.next_state(
            successor_balance,
            self.state.hardware_epoch,
            self.state.device_policy_binding,
            successor_state_nonce_commitment,
            self.state.consumed_credit_root,
        )?;
        let output = KagemushaPaymentOutputV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            request_digest: request
                .canonical_digest()
                .map_err(|_| KagemushaStateErrorV1::InvalidPaymentRequest)?,
            amount,
            sender_before_commitment: self.state.state_commitment,
            sender_after_commitment: successor.state_commitment,
            transition_nullifier,
            credit_id: [0; 32],
            ciphertext_commitment,
            commit_evidence,
            committed_at_ms: commit_authorization_reference_ms,
        }
        .seal_credit_id_against(&request)
        .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;
        let mut lifecycle = terminal_lifecycle_binding_v1(
            &self.state,
            KagemushaOperationKindV1::SendSplit,
            request.request_id,
            request.hardware_credential.lane_commitment,
            kagemusha_ciphertext_digest_v1(&encrypted_credit),
        );
        lifecycle.credit_id = output.credit_id;
        let lifecycle_binding_digest = lifecycle
            .canonical_digest()
            .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;
        let request_digest = request
            .canonical_digest()
            .map_err(|_| KagemushaStateErrorV1::InvalidPaymentRequest)?;
        let reservation_commitment = outbox_reservation
            .canonical_commitment()
            .map_err(|_| KagemushaStateErrorV1::SenderOutboxCapacityExhausted)?;
        let prepared_transition_binding_digest = canonical_prepared_transition_binding_digest_v1(
            lifecycle_binding_digest,
            request_digest,
            output.sender_before_commitment,
            output.sender_after_commitment,
            amount,
            reservation_commitment,
            prepared_one_use_authorization_digest,
        );
        let successor_commitment = successor.state_commitment;
        let preview = self.transition_preview(
            KagemushaTransitionKindV1::SendSplit,
            successor,
            prepared_transition_binding_digest,
            [0; 32],
            [0; 32],
            output.credit_id,
            request.recipient_encryption_key,
            TransitionAuxiliaryBindingsV1 {
                lifecycle_binding_digest,
                prepared_transition_binding_digest,
                ..TransitionAuxiliaryBindingsV1::default()
            },
            commit_authorization_reference_ms,
            |normalized_guard_statement_digest| {
                local_transition_transport_digest(
                    KagemushaTransitionKindV1::SendSplit,
                    self.state.release_id,
                    self.state.liability_pool_id,
                    prepared_transition_binding_digest,
                    self.state.state_commitment,
                    successor_commitment,
                    normalized_guard_statement_digest,
                )
            },
        )?;
        let normalized_guard_statement_digest = preview
            .normalized_guard_statement
            .canonical_digest()
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        let state_transition_digest = preview.proof_statement.digest()?;

        PreparedOutgoingCandidateV1::send(
            self.state.clone(),
            preview.successor,
            state_transition_digest,
            candidate_lifecycle::PreparedSendMaterialV1 {
                proof_statement: preview.proof_statement,
                lifecycle,
                output,
                request,
                encrypted_credit,
                outbox_reservation,
                prepared_one_use_authorization_digest,
                sealed_transition_inputs,
                sealed_recovery_seeds,
                normalized_guard_statement_digest,
            },
        )
    }

    /// Derive one complete, recoverable full or partial `RedeemSplit` intent.
    ///
    /// Core derives the private aggregate successor, terminal lifecycle, redemption ID, proof
    /// statement, and normalized guard binding. The returned candidate must still be durably
    /// staged before qualified hardware consumes the predecessor.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero amount, insufficient folded balance, invalid terminal
    /// bindings, a reused state nonce, malformed recovery material, an incorrect outbox
    /// reservation, or a conflicting active outgoing intent.
    pub fn prepare_redeem_split(
        &self,
        preparation: RedeemSplitPreparationV1,
    ) -> Result<PreparedOutgoingCandidateV1, KagemushaStateErrorV1> {
        let RedeemSplitPreparationV1 {
            amount,
            beneficiary,
            terminal_nullifier,
            redemption_commitment,
            successor_state_nonce_commitment,
            commit_evidence,
            commit_authorization_reference_ms,
            outbox_reservation,
            prepared_one_use_authorization_digest,
            sealed_transition_inputs,
            sealed_recovery_seeds,
        } = preparation;
        if amount == 0
            || commit_evidence.validate().is_err()
            || commit_authorization_reference_ms == 0
            || prepared_one_use_authorization_digest == [0; 32]
        {
            return Err(KagemushaStateErrorV1::InvalidRedemption);
        }
        let successor_balance = self
            .state
            .balance
            .checked_sub(amount)
            .ok_or(KagemushaStateErrorV1::InsufficientBalance)?;
        let successor = self.next_state(
            successor_balance,
            self.state.hardware_epoch,
            self.state.device_policy_binding,
            successor_state_nonce_commitment,
            self.state.consumed_credit_root,
        )?;
        let statement = KagemushaRedemptionStatementV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            lifecycle: terminal_lifecycle_binding_v1(
                &self.state,
                KagemushaOperationKindV1::RedeemSplit,
                [0; 32],
                [0; 32],
                [0; 32],
            ),
            amount,
            beneficiary,
            terminal_nullifier,
            redemption_commitment,
            redemption_id: [0; 32],
            commit_evidence,
        }
        .seal_redemption_id()
        .map_err(|_| KagemushaStateErrorV1::InvalidRedemption)?;
        let lifecycle_binding_digest = statement
            .lifecycle
            .canonical_digest()
            .map_err(|_| KagemushaStateErrorV1::InvalidRedemption)?;
        let reservation_commitment = outbox_reservation
            .canonical_commitment()
            .map_err(|_| KagemushaStateErrorV1::SenderOutboxCapacityExhausted)?;
        let prepared_transition_binding_digest = canonical_prepared_transition_binding_digest_v1(
            lifecycle_binding_digest,
            [0; 32],
            [0; 32],
            [0; 32],
            amount,
            reservation_commitment,
            prepared_one_use_authorization_digest,
        );
        let successor_commitment = successor.state_commitment;
        let preview = self.transition_preview(
            KagemushaTransitionKindV1::RedeemSplit,
            successor,
            prepared_transition_binding_digest,
            [0; 32],
            [0; 32],
            [0; 32],
            [0; 32],
            TransitionAuxiliaryBindingsV1 {
                lifecycle_binding_digest,
                prepared_transition_binding_digest,
                ..TransitionAuxiliaryBindingsV1::default()
            },
            commit_authorization_reference_ms,
            |normalized_guard_statement_digest| {
                local_transition_transport_digest(
                    KagemushaTransitionKindV1::RedeemSplit,
                    self.state.release_id,
                    self.state.liability_pool_id,
                    prepared_transition_binding_digest,
                    self.state.state_commitment,
                    successor_commitment,
                    normalized_guard_statement_digest,
                )
            },
        )?;
        let normalized_guard_statement_digest = preview
            .normalized_guard_statement
            .canonical_digest()
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        let state_transition_digest = preview.proof_statement.digest()?;

        PreparedOutgoingCandidateV1::redemption(
            self.state.clone(),
            preview.successor,
            state_transition_digest,
            candidate_lifecycle::PreparedRedemptionMaterialV1 {
                proof_statement: preview.proof_statement,
                statement,
                outbox_reservation,
                prepared_one_use_authorization_digest,
                artifact_manifest_digest: self.proof_release.artifacts.artifact_manifest_digest,
                sealed_transition_inputs,
                sealed_recovery_seeds,
                normalized_guard_statement_digest,
            },
        )
    }

    /// Atomically bind a caller ID, reserve sender bytes, and prepare the exact transition.
    ///
    /// `authenticated_credential_id` must come from a qualified native session. Core binds it
    /// immutably but does not authenticate arbitrary host-supplied credential IDs. Call
    /// [`Self::classify_outgoing_operation_prepare`] before deriving a new candidate so a lost
    /// response recovers an existing operation at any phase without invoking preparation again.
    pub fn prepare_indexed_outgoing_candidate(
        &mut self,
        operation_id: DigestV1,
        authenticated_credential_id: DigestV1,
        prepared: PreparedOutgoingCandidateV1,
    ) -> Result<
        (
            KagemushaOutgoingOperationPrepareOutcomeV1,
            SenderOutboxReservationOutcomeV1,
            KagemushaOutgoingCommitCapabilityV1,
        ),
        KagemushaStateErrorV1,
    > {
        if prepared.private_state_link().0 != &self.state
            || prepared.proof_statement.journal_revision_before != self.journal_revision
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        prepared.validate_recovered()?;
        prepared.validate_recipient_against_release(&self.proof_release)?;
        let capability = KagemushaOutgoingCommitCapabilityV1::for_prepared(&prepared)?;
        let reservation = prepared.outbox_reservation;
        let mut next_outbox = self.sender_outbox_capacity.clone();
        let mut next_journal = self.outgoing_candidate_journal.clone();
        let indexed_outcome =
            next_journal.prepare_indexed(operation_id, authenticated_credential_id, prepared)?;
        let reservation_outcome = next_outbox.reserve(reservation, &next_journal)?;
        self.sender_outbox_capacity = next_outbox;
        self.outgoing_candidate_journal = next_journal;
        Ok((indexed_outcome, reservation_outcome, capability))
    }

    /// Reissue authority for one exact caller-indexed preparation after authenticated recovery.
    pub fn recover_indexed_outgoing_commit_capability(
        &self,
        operation_id: DigestV1,
    ) -> Result<KagemushaOutgoingCommitCapabilityV1, KagemushaStateErrorV1> {
        let (prepared, phase) = match self.outgoing_candidate_journal.stage() {
            KagemushaOutgoingJournalStageV1::Prepared(prepared) => {
                (prepared, KagemushaOutgoingOperationPhaseV1::Prepared)
            }
            KagemushaOutgoingJournalStageV1::Candidate(candidate) => (
                &candidate.prepared,
                KagemushaOutgoingOperationPhaseV1::CandidatePersisted,
            ),
            _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
        };
        let record = self
            .outgoing_operation_index()
            .lookup(operation_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if record.preparation_id != prepared.preparation_id || record.phase != phase {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.sender_outbox_capacity
            .require_reservation(prepared.outbox_reservation)?;
        KagemushaOutgoingCommitCapabilityV1::for_prepared(prepared)
    }

    /// Verify and persist the request-bound sender state proof before hardware consumes state.
    pub fn persist_outgoing_send_candidate(
        &mut self,
        capability: &KagemushaOutgoingCommitCapabilityV1,
        candidate_proof: KagemushaPairedProofV1,
    ) -> Result<PersistedOutgoingCandidateV1, KagemushaStateErrorV1> {
        let prepared = match self.outgoing_candidate_journal.stage() {
            KagemushaOutgoingJournalStageV1::Prepared(prepared) => prepared.clone(),
            _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
        };
        capability.authorizes(&prepared)?;
        let candidate = PersistedOutgoingCandidateV1::verify_and_persist_send(
            prepared,
            candidate_proof,
            self.proof_release.artifacts,
            &self.recursive_verifier,
        )?;
        self.persist_verified_outgoing_candidate(candidate)
    }

    /// Verify and persist a private redemption candidate proof before hardware consumes state.
    pub fn persist_outgoing_redemption_candidate(
        &mut self,
        capability: &KagemushaOutgoingCommitCapabilityV1,
        candidate_proof: KagemushaPairedProofV1,
    ) -> Result<PersistedOutgoingCandidateV1, KagemushaStateErrorV1> {
        let prepared = match self.outgoing_candidate_journal.stage() {
            KagemushaOutgoingJournalStageV1::Prepared(prepared) => prepared.clone(),
            _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
        };
        capability.authorizes(&prepared)?;
        let candidate = PersistedOutgoingCandidateV1::verify_and_persist_redemption(
            prepared,
            candidate_proof,
            self.proof_release.artifacts,
            &self.recursive_verifier,
        )?;
        self.persist_verified_outgoing_candidate(candidate)
    }

    fn persist_verified_outgoing_candidate(
        &mut self,
        candidate: PersistedOutgoingCandidateV1,
    ) -> Result<PersistedOutgoingCandidateV1, KagemushaStateErrorV1> {
        let mut next_journal = self.outgoing_candidate_journal.clone();
        next_journal.persist_candidate(candidate.clone())?;
        next_journal.validate_recovered(
            &self.state,
            self.journal_revision,
            &self.sender_outbox_capacity,
            &self.proof_release,
            self.proof_release.artifacts,
            &self.recursive_verifier,
        )?;
        self.outgoing_candidate_journal = next_journal;
        Ok(candidate)
    }

    /// Atomically install the hardware-certified successor exactly once.
    ///
    /// A committed predecessor can therefore never coexist with its old monetary head in a
    /// canonical snapshot.
    pub fn commit_outgoing_candidate(
        &mut self,
        capability: KagemushaOutgoingCommitCapabilityV1,
        commit_certificate: KagemushaCommitCertificateV1,
    ) -> Result<CommittedOutgoingCandidateV1, KagemushaStateErrorV1> {
        let candidate = match self.outgoing_candidate_journal.stage() {
            KagemushaOutgoingJournalStageV1::Candidate(candidate) => candidate.clone(),
            _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
        };
        capability.authorizes(&candidate.prepared)?;
        if candidate.prepared.private_state_link().0 != &self.state
            || candidate.prepared.proof_statement.journal_revision_before != self.journal_revision
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.sender_outbox_capacity
            .require_reservation(candidate.prepared.outbox_reservation)?;
        let committed =
            CommittedOutgoingCandidateV1::from_hardware_commit(candidate, commit_certificate)?;
        let prepared = &committed.candidate.prepared;
        if prepared.private_state_link().0 != &self.state
            || prepared.proof_statement.journal_revision_before != self.journal_revision
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let mut next_journal = self.outgoing_candidate_journal.clone();
        next_journal.commit(committed.clone())?;
        self.state = prepared.private_state_link().1.clone();
        self.journal_revision = prepared.proof_statement.journal_revision_after;
        self.outgoing_candidate_journal = next_journal;
        Ok(committed)
    }

    /// Authenticate and persist the compact terminal payment for byte-identical retry.
    pub fn finalize_outgoing_payment(
        &mut self,
        payment: KagemushaPaymentV1,
        retry_metadata: Vec<u8>,
    ) -> Result<DurableOutgoingEnvelopeV1, KagemushaStateErrorV1> {
        let committed = self.committed_candidate_for_finalization()?;
        let finalized = DurableOutgoingEnvelopeV1::finalize_payment(
            committed,
            payment,
            retry_metadata,
            self.proof_release.artifacts,
            &self.recursive_verifier,
        )?;
        self.install_finalized_outgoing_envelope(finalized)
    }

    /// Verify and persist the redemption proof and canonical voucher for byte-identical retry.
    pub fn finalize_outgoing_redemption(
        &mut self,
        proof: KagemushaRedemptionProofV1,
        retry_metadata: Vec<u8>,
    ) -> Result<DurableOutgoingEnvelopeV1, KagemushaStateErrorV1> {
        let committed = self.committed_candidate_for_finalization()?;
        let finalized = DurableOutgoingEnvelopeV1::finalize_redemption(
            committed,
            proof,
            retry_metadata,
            self.proof_release.artifacts,
            &self.recursive_verifier,
        )?;
        self.install_finalized_outgoing_envelope(finalized)
    }

    fn committed_candidate_for_finalization(
        &self,
    ) -> Result<CommittedOutgoingCandidateV1, KagemushaStateErrorV1> {
        let committed = match self.outgoing_candidate_journal.stage() {
            KagemushaOutgoingJournalStageV1::Committed(committed) => committed.clone(),
            _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
        };
        if committed.candidate.prepared.private_state_link().1 != &self.state
            || committed
                .candidate
                .prepared
                .proof_statement
                .journal_revision_after
                != self.journal_revision
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        Ok(committed)
    }

    fn install_finalized_outgoing_envelope(
        &mut self,
        finalized: DurableOutgoingEnvelopeV1,
    ) -> Result<DurableOutgoingEnvelopeV1, KagemushaStateErrorV1> {
        let mut next_outbox = self.sender_outbox_capacity.clone();
        let mut next_journal = self.outgoing_candidate_journal.clone();
        next_journal.install_finalized(finalized.clone(), &mut next_outbox)?;
        self.sender_outbox_capacity = next_outbox;
        self.outgoing_candidate_journal = next_journal;
        Ok(finalized)
    }

    /// Return byte-identical terminal bytes only after durable final installation.
    pub fn expose_outgoing_envelope(
        &self,
        reservation_id: DigestV1,
    ) -> Result<&[u8], KagemushaStateErrorV1> {
        self.outgoing_candidate_journal.expose(reservation_id)
    }

    /// Verify a receiver acknowledgement and atomically retain an indexed release tombstone.
    pub fn release_indexed_outgoing_payment(
        &mut self,
        operation_id: DigestV1,
        acknowledgement_bytes: &[u8],
    ) -> Result<(), KagemushaStateErrorV1> {
        let mut next_outbox = self.sender_outbox_capacity.clone();
        let mut next_journal = self.outgoing_candidate_journal.clone();
        next_journal.release_indexed_payment(
            operation_id,
            acknowledgement_bytes,
            &mut next_outbox,
        )?;
        self.sender_outbox_capacity = next_outbox;
        self.outgoing_candidate_journal = next_journal;
        Ok(())
    }

    /// Authenticate a finalized redemption status and bind it to one exact indexed voucher.
    ///
    /// `trust_anchor` must be pinned by the caller from an already authenticated consensus
    /// context. It is never selected from `status`. The returned capability is non-serializable
    /// and must be consumed by [`Self::release_indexed_outgoing_redemption`].
    pub fn verify_indexed_redemption_release(
        &self,
        operation_id: DigestV1,
        status: &KagemushaOperationStatusV1,
        trust_anchor: &KagemushaFinalityTrustAnchorV1,
    ) -> Result<VerifiedKagemushaRedemptionReleaseV1, KagemushaStateErrorV1> {
        let record = self
            .outgoing_candidate_journal
            .operation_index()
            .lookup(operation_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let durable = self
            .outgoing_candidate_journal
            .finalized_envelope(record.outbox_reservation_id);
        redemption_release::verify_indexed_redemption_release(
            record,
            durable,
            operation_id,
            status,
            trust_anchor,
        )
    }

    /// Consume Core's closed settlement capability and atomically retain a release tombstone.
    ///
    /// The native hardware command may bind the capability's compact public projection, but raw
    /// projection bytes or a host-computed digest can never call this authority path.
    pub fn release_indexed_outgoing_redemption(
        &mut self,
        verified: VerifiedKagemushaRedemptionReleaseV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let mut next_outbox = self.sender_outbox_capacity.clone();
        let mut next_journal = self.outgoing_candidate_journal.clone();
        next_journal.release_indexed_redemption(verified, &mut next_outbox)?;
        self.sender_outbox_capacity = next_outbox;
        self.outgoing_candidate_journal = next_journal;
        Ok(())
    }

    /// Return the number of durably staged credits awaiting a fold.
    #[must_use]
    pub fn pending_credit_count(&self) -> usize {
        self.pending_credits.len() + self.mint_inbox.pending_count()
    }

    /// Select the deterministic prefix of staged credits needed to cover `amount`.
    ///
    /// Wallet orchestration calls this before a send or redemption, then drains the returned
    /// credits through the corresponding mint or peer fold operation.
    /// The selection is ordered by credit ID and has no protocol count ceiling: a larger backlog
    /// changes only local work and latency. An empty result means the current aggregate balance
    /// already covers the amount.
    pub fn pending_credits_required_for_amount(
        &self,
        amount: u128,
    ) -> Result<Vec<CreditIdV1>, KagemushaStateErrorV1> {
        self.pending_fold_plan_required_for_amount(amount)
            .map(|plan| {
                plan.into_iter()
                    .map(PendingCreditFoldV1::credit_id)
                    .collect()
            })
    }

    /// Preview folding one finalized mint credit into the aggregate and durably prepare its
    /// external replay-root CAS.
    pub fn preview_mint_fold(
        &mut self,
        credit: &KagemushaMintCreditV1,
        successor_state_nonce_commitment: DigestV1,
        trusted_commit_time_ms: u64,
    ) -> Result<CreditFoldPreviewV1, KagemushaStateErrorV1> {
        let mint_private_inputs = self.mint_fold_private_inputs_for_credit(credit, false)?;
        let (transition, replay_insert_witness, credit_id, envelope_digest) = self
            .derive_mint_fold_transition(
                credit,
                successor_state_nonce_commitment,
                trusted_commit_time_ms,
            )?;
        match self
            .authenticated_history
            .classify_replay(credit_id, envelope_digest)
            .map_err(map_authenticated_history_error)?
        {
            KagemushaHistoryIdentityClassificationV1::Absent => {}
            KagemushaHistoryIdentityClassificationV1::ExactDuplicate => {
                return Err(KagemushaStateErrorV1::CreditAlreadyConsumed(credit_id));
            }
            KagemushaHistoryIdentityClassificationV1::Conflict { .. } => {
                return Err(KagemushaStateErrorV1::CreditConflict(credit_id));
            }
        }
        let authenticated_history_transaction = match self
            .authenticated_history
            .prepare_replay(
                credit_id,
                envelope_digest,
                transition
                    .hardware_statement
                    .normalized_guard_statement_digest,
            )
            .map_err(map_authenticated_history_error)?
        {
            KagemushaHistoryInsertPreparationV1::Prepared {
                transaction,
                outcome:
                    KagemushaHistoryPrepareOutcomeV1::Prepared
                    | KagemushaHistoryPrepareOutcomeV1::AlreadyPrepared,
            } => transaction,
            KagemushaHistoryInsertPreparationV1::Prepared {
                outcome: KagemushaHistoryPrepareOutcomeV1::AlreadyCommitted { .. },
                ..
            }
            | KagemushaHistoryInsertPreparationV1::ExactDuplicate => {
                return Err(KagemushaStateErrorV1::CreditAlreadyConsumed(credit_id));
            }
            KagemushaHistoryInsertPreparationV1::Prepared {
                outcome: KagemushaHistoryPrepareOutcomeV1::AlreadyAborted,
                ..
            } => return Err(KagemushaStateErrorV1::StateInvariant),
            KagemushaHistoryInsertPreparationV1::Conflict { .. } => {
                return Err(KagemushaStateErrorV1::CreditConflict(credit_id));
            }
        };
        let proof_root_bridge_request = self
            .authenticated_history
            .proof_root_bridge_request(
                &authenticated_history_transaction,
                transition.proof_statement.effect_digest,
                replay_insert_witness.predecessor_root,
                replay_insert_witness.successor_root,
            )
            .map_err(map_authenticated_history_error)?;
        Ok(CreditFoldPreviewV1 {
            transition,
            replay_insert_witness,
            authenticated_history_transaction,
            proof_root_bridge_request,
            mint_private_inputs,
            trusted_commit_time_ms,
        })
    }

    fn derive_mint_fold_transition(
        &self,
        credit: &KagemushaMintCreditV1,
        successor_state_nonce_commitment: DigestV1,
        trusted_commit_time_ms: u64,
    ) -> Result<
        (
            TransitionPreviewV1,
            ConsumedCreditInsertWitnessV1,
            CreditIdV1,
            DigestV1,
        ),
        KagemushaStateErrorV1,
    > {
        credit
            .validate_shape()
            .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
        let mint = &credit.statement;
        let lifecycle = &mint.lifecycle;
        if lifecycle.protocol_version != self.state.protocol_version
            || lifecycle.suite_id != self.state.suite_id
            || lifecycle.vk_digest != self.state.vk_digest
            || lifecycle.release_id != self.state.release_id
            || lifecycle.liability_pool_id != self.state.liability_pool_id
            || lifecycle.network_id != self.state.lane.network_id
            || lifecycle.asset != self.state.lane.asset
            || lifecycle.asset_incarnation != self.state.asset_incarnation
            || lifecycle.scale != self.state.lane.scale
            || lifecycle.hardware_profile_id != self.state.hardware_profile_id
            || lifecycle.policy_epoch != self.state.policy_epoch
            || credit.artifact_manifest_digest
                != self.proof_release.artifacts.artifact_manifest_digest
        {
            return Err(KagemushaStateErrorV1::InvalidMintCredit);
        }
        let credit_id = CreditIdV1(lifecycle.credit_id);
        let envelope_digest = canonical_sha256_digest(MINT_CREDIT_DOMAIN, credit)?;
        if self.mint_inbox.contains_credit_id(credit_id) {
            self.mint_inbox.validate_fold(credit)?;
            self.ensure_non_mint_credit_id_available(credit_id, envelope_digest)?;
        } else {
            self.ensure_credit_id_available(credit_id, envelope_digest)?;
        }
        let replay_insert_witness = self
            .consumed_credits
            .preview_insert_witness(credit_id, envelope_digest)?;
        // This witness is derived from the exact local tree. The recursive relation consumes it,
        // and `insert_with_witness` revalidates both complete paths before atomic host mutation;
        // repeating those Poseidon paths here would only add synchronous backlog latency.
        let balance = self
            .state
            .balance
            .checked_add(mint.amount)
            .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)?;
        let successor = self.next_state(
            balance,
            self.state.hardware_epoch,
            self.state.device_policy_binding,
            successor_state_nonce_commitment,
            replay_insert_witness.successor_root,
        )?;
        let mint_finality_semantic_digest = mint
            .canonical_digest()
            .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
        let lifecycle_binding_digest = lifecycle
            .canonical_digest()
            .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
        let effect_digest = canonical_sha256_digest(
            TRANSITION_EFFECT_DOMAIN,
            &MintFoldEffectV1 {
                credit_id,
                envelope_digest,
                amount: mint.amount,
                issuance_digest: mint.issuance_commitment,
                mint_finality_semantic_digest,
                mint_finality_proof_binding_digest: credit.finality_proof_binding_digest,
            },
        )?;
        let transition = self.transition_preview(
            KagemushaTransitionKindV1::MintFold,
            successor.clone(),
            effect_digest,
            mint_finality_semantic_digest,
            credit.finality_proof_binding_digest,
            [0; 32],
            [0; 32],
            TransitionAuxiliaryBindingsV1 {
                lifecycle_binding_digest,
                ..TransitionAuxiliaryBindingsV1::default()
            },
            trusted_commit_time_ms,
            |normalized_guard_statement_digest| {
                local_transition_transport_digest(
                    KagemushaTransitionKindV1::MintFold,
                    self.state.release_id,
                    self.state.liability_pool_id,
                    effect_digest,
                    self.state.state_commitment,
                    successor.state_commitment,
                    normalized_guard_statement_digest,
                )
            },
        )?;
        Ok((
            transition,
            replay_insert_witness,
            credit_id,
            envelope_digest,
        ))
    }

    /// Return the exact qualified-hardware message selecting a mint's external replay root.
    pub fn mint_fold_history_root_selection_signing_bytes(
        &self,
        preview: &CreditFoldPreviewV1,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.validate_mint_fold_history_preview(preview)?;
        self.authenticated_history
            .require_prepared(&preview.authenticated_history_transaction)
            .map_err(map_authenticated_history_error)?;
        KagemushaHistoryRootSelectionSubjectV1::new(
            &preview.authenticated_history_transaction,
            self.state.hardware_profile_id,
            self.state.hardware_epoch.generation,
            preview.transition.journal_revision_after,
        )
        .signing_bytes()
        .map_err(map_authenticated_history_error)
    }

    /// Authenticate and attach the hardware-selected replay root after verifying the paired mint
    /// transition proof for the same logical operation.
    pub fn authorize_mint_fold_history(
        &self,
        preview: &CreditFoldPreviewV1,
        mut authorization: TransitionAuthorizationV1,
        device_public_key: &KagemushaDevicePublicKeyV1,
        root_selection_signature: KagemushaDeviceSignatureV1,
    ) -> Result<TransitionAuthorizationV1, KagemushaStateErrorV1> {
        self.validate_mint_fold_history_preview(preview)?;
        self.authenticated_history
            .require_prepared(&preview.authenticated_history_transaction)
            .map_err(map_authenticated_history_error)?;
        if authorization.authenticated_history.is_some()
            || kagemusha_device_key_reference_v1(device_public_key)
                != self.state.device_policy_binding.device_key_reference
        {
            return Err(KagemushaStateErrorV1::InvalidDevicePolicyBinding);
        }
        self.verify_transition_authorization(&preview.transition, &authorization)?;
        let subject = KagemushaHistoryRootSelectionSubjectV1::new(
            &preview.authenticated_history_transaction,
            self.state.hardware_profile_id,
            self.state.hardware_epoch.generation,
            preview.transition.journal_revision_after,
        );
        let root_selection =
            KagemushaHistoryRootSelectionCertificateV1::new(subject, root_selection_signature)
                .verify(self.state.hardware_profile_id, device_public_key)
                .map_err(map_authenticated_history_error)?;
        let proof_root_bridge = require_history_proof_root_bridge_v1(
            preview.proof_root_bridge_request,
            preview.transition.proof_statement.effect_digest,
        )
        .map_err(|_| KagemushaStateErrorV1::AuthenticatedHistoryProofRootBridgeUnavailable)?;
        authorization.authenticated_history = Some(KagemushaHistoryTransitionAuthorizationV1 {
            root_selection,
            proof_root_bridge,
        });
        Ok(authorization)
    }

    /// Verify and atomically apply one durably prepared finalized mint credit.
    pub fn mint_fold_prepared(
        &mut self,
        credit: KagemushaMintCreditV1,
        preview: CreditFoldPreviewV1,
        mint_finality: VerifiedKagemushaMintFinalityHelperV1,
        authorization: TransitionAuthorizationV1,
    ) -> Result<KagemushaStateV1, KagemushaStateErrorV1> {
        self.install_mint_fold(credit, preview, mint_finality, authorization, false)
    }

    /// Complete an authorized mint fold after a crash at the external replay-root CAS boundary.
    pub fn recover_mint_fold_prepared(
        &mut self,
        credit: KagemushaMintCreditV1,
        preview: CreditFoldPreviewV1,
        mint_finality: VerifiedKagemushaMintFinalityHelperV1,
        authorization: TransitionAuthorizationV1,
    ) -> Result<KagemushaStateV1, KagemushaStateErrorV1> {
        self.install_mint_fold(credit, preview, mint_finality, authorization, true)
    }

    /// Release the byte-bounded WAL entry for an abandoned, uncommitted mint preview.
    pub fn abandon_mint_fold_preview(
        &mut self,
        preview: &CreditFoldPreviewV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        match self
            .authenticated_history
            .abort_prepared(preview.authenticated_history_transaction.transaction_id())
            .map_err(map_authenticated_history_error)?
        {
            KagemushaHistoryAbortOutcomeV1::Aborted
            | KagemushaHistoryAbortOutcomeV1::AlreadyAborted => Ok(()),
            KagemushaHistoryAbortOutcomeV1::AlreadyCommitted { .. } => {
                Err(KagemushaStateErrorV1::StateInvariant)
            }
        }
    }

    fn install_mint_fold(
        &mut self,
        credit: KagemushaMintCreditV1,
        preview: CreditFoldPreviewV1,
        mint_finality: VerifiedKagemushaMintFinalityHelperV1,
        authorization: TransitionAuthorizationV1,
        recovering: bool,
    ) -> Result<KagemushaStateV1, KagemushaStateErrorV1> {
        let expected_private_inputs =
            self.mint_fold_private_inputs_for_credit(&credit, recovering)?;
        if expected_private_inputs != preview.mint_private_inputs {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let (expected_transition, expected_witness, credit_id, envelope_digest) = self
            .derive_mint_fold_transition(
                &credit,
                preview.transition.successor.state_nonce_commitment,
                preview.trusted_commit_time_ms,
            )?;
        if preview
            .authenticated_history_transaction
            .attempt_binding_digest()
            != expected_transition
                .hardware_statement
                .normalized_guard_statement_digest
            || expected_transition != preview.transition
            || expected_witness != preview.replay_insert_witness
        {
            return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
        }
        let mint_statement_digest = credit
            .statement
            .canonical_digest()
            .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
        if mint_finality.semantic_digest() != mint_statement_digest {
            return Err(KagemushaStateErrorV1::MintFinalityMismatch);
        }
        if mint_finality.proof_binding_digest() != credit.finality_proof_binding_digest {
            return Err(KagemushaStateErrorV1::MintFinalityMismatch);
        }
        let mut next_consumed_credits = self.consumed_credits.clone();
        let next_mint_inbox = if self.mint_inbox.contains_credit_id(credit_id) {
            self.mint_inbox.folded_successor(&credit)?
        } else {
            self.mint_inbox.clone()
        };
        let next_capacity = self
            .receiver_inbox_capacity
            .with_mint_inbox_bytes(next_mint_inbox.capacity_charge_bytes()?)?;
        next_consumed_credits.insert_with_witness(
            credit_id,
            envelope_digest,
            &preview.replay_insert_witness,
        )?;
        self.verify_transition_authorization(&preview.transition, &authorization)?;

        let history_authorization = authorization
            .authenticated_history
            .ok_or(KagemushaStateErrorV1::AuthenticatedHistoryProofRootBridgeUnavailable)?;
        let bridge_request = preview.proof_root_bridge_request;
        let external_predecessor_roots = bridge_request.external_predecessor_roots();
        let external_successor_roots = bridge_request.external_successor_roots();
        let current_external_roots = self.authenticated_history.committed_roots();
        if history_authorization.root_selection.transaction_id()
            != preview.authenticated_history_transaction.transaction_id()
            || history_authorization.root_selection.root_selection()
                != preview.authenticated_history_transaction.root_selection()
            || history_authorization.root_selection.hardware_profile_id()
                != self.state.hardware_profile_id
            || history_authorization.root_selection.hardware_epoch()
                != self.state.hardware_epoch.generation
            || history_authorization.root_selection.monotonic_counter()
                != preview.transition.journal_revision_after
            || history_authorization.proof_root_bridge.request() != bridge_request
            || bridge_request.transaction_id()
                != preview.authenticated_history_transaction.transaction_id()
            || bridge_request.pasta_predecessor_replay_root()
                != preview.replay_insert_witness.predecessor_root
            || bridge_request.pasta_successor_replay_root()
                != preview.replay_insert_witness.successor_root
            || preview
                .authenticated_history_transaction
                .successor_roots_from(external_predecessor_roots)
                .map_err(map_authenticated_history_error)?
                != external_successor_roots
            || (current_external_roots != external_predecessor_roots
                && (!recovering || current_external_roots != external_successor_roots))
        {
            return Err(KagemushaStateErrorV1::AuthenticatedHistoryProofRootBridgeUnavailable);
        }
        let committed_roots = if recovering {
            match self
                .authenticated_history
                .recover_prepared(history_authorization.root_selection)
                .map_err(map_authenticated_history_error)?
            {
                KagemushaHistoryRecoveryOutcomeV1::Committed { committed_roots }
                | KagemushaHistoryRecoveryOutcomeV1::AlreadyCommitted { committed_roots } => {
                    committed_roots
                }
                KagemushaHistoryRecoveryOutcomeV1::Aborted => {
                    return Err(KagemushaStateErrorV1::StateInvariant);
                }
            }
        } else {
            match self
                .authenticated_history
                .commit_prepared(history_authorization.root_selection)
                .map_err(map_authenticated_history_error)?
            {
                KagemushaHistoryCommitOutcomeV1::Committed { committed_roots }
                | KagemushaHistoryCommitOutcomeV1::AlreadyCommitted { committed_roots } => {
                    committed_roots
                }
                KagemushaHistoryCommitOutcomeV1::Aborted => {
                    return Err(KagemushaStateErrorV1::StateInvariant);
                }
            }
        };
        if committed_roots != external_successor_roots {
            return Err(KagemushaStateErrorV1::AuthenticatedHistoryUnavailable);
        }
        self.consumed_credits = next_consumed_credits;
        self.mint_inbox = next_mint_inbox;
        self.receiver_inbox_capacity = next_capacity;
        self.commit_preview(preview.transition);
        Ok(self.state.clone())
    }

    fn validate_mint_fold_history_preview(
        &self,
        preview: &CreditFoldPreviewV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let expected_private_inputs =
            self.mint_fold_private_inputs_for_credit(preview.mint_private_inputs.credit(), false)?;
        let bridge_request = preview.proof_root_bridge_request;
        if preview
            .authenticated_history_transaction
            .attempt_binding_digest()
            != preview
                .transition
                .hardware_statement
                .normalized_guard_statement_digest
            || expected_private_inputs != preview.mint_private_inputs
            || preview.transition.proof_statement.kind != KagemushaTransitionKindV1::MintFold
            || preview.transition.proof_statement.predecessor_commitment
                != self.state.state_commitment
            || preview.transition.proof_statement.journal_revision_before != self.journal_revision
            || bridge_request.transaction_id()
                != preview.authenticated_history_transaction.transaction_id()
            || bridge_request.operation_binding_digest()
                != preview.transition.proof_statement.effect_digest
            || bridge_request.external_predecessor_roots()
                != self.authenticated_history.committed_roots()
            || bridge_request.pasta_predecessor_replay_root()
                != preview.replay_insert_witness.predecessor_root
            || bridge_request.pasta_successor_replay_root()
                != preview.replay_insert_witness.successor_root
        {
            return Err(KagemushaStateErrorV1::AuthenticatedHistoryProofRootBridgeUnavailable);
        }
        Ok(())
    }

    /// Preview the exact receiver journal statement for a new public payment.
    pub fn preview_stage_payment(
        &self,
        request: &KagemushaPaymentRequestV1,
        payment: &KagemushaPaymentV1,
        credit_opening: &KagemushaCreditOpeningV1,
        staged_at_ms: u64,
    ) -> Result<CreditStageStatementV1, KagemushaStateErrorV1> {
        self.validate_peer_payment(request, payment)?;
        validate_peer_credit_opening_against_payment(request, payment, credit_opening)?;
        let credit_id = CreditIdV1(payment.output.credit_id);
        let envelope_digest = canonical_sha256_digest(CREDIT_ENVELOPE_DOMAIN, payment)?;
        self.ensure_peer_credit_id_available(credit_id, envelope_digest)?;
        let journal_revision_after = self
            .inbox_revision
            .checked_add(1)
            .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?;
        Ok(CreditStageStatementV1 {
            version: KAGEMUSHA_STATE_VERSION_V1,
            recipient_lane: self.state.lane.clone(),
            receiver_state_commitment: self.state.state_commitment,
            receiver_hardware_epoch: self.state.hardware_epoch,
            receiver_device_policy_binding: self.state.device_policy_binding,
            receiver_state_nonce_commitment: self.state.state_nonce_commitment,
            credit_id,
            envelope_digest,
            staged_at_ms,
            journal_revision_before: self.inbox_revision,
            journal_revision_after,
        })
    }

    /// Durably stage or idempotently classify one inbound credit.
    ///
    /// Request expiry is checked only against the sender's trusted commit time inside the credit.
    /// `staged_at_ms` may be arbitrarily later without invalidating committed value.
    pub fn stage_payment(
        &mut self,
        request: KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
        credit_opening: KagemushaCreditOpeningV1,
        staged_at_ms: u64,
        stage_authorization: Option<PaymentStageAuthorizationV1>,
    ) -> Result<StagePaymentOutcomeV1, KagemushaStateErrorV1> {
        let credit_id = CreditIdV1(payment.output.credit_id);
        // Apply every variable-length wire bound before consulting durable replay state. This
        // keeps even a same-ID conflict from turning equality against an anchored receipt into an
        // oversized allocation/comparison path, while remaining independent of proof release.
        validate_peer_payment_wire_shape_against_lane(&self.state, &request, &payment)?;
        credit_opening
            .validate_shape()
            .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;
        if let Some(existing) = self.accepted_payment_receipts.get(&credit_id) {
            if existing.request != request
                || existing.payment != payment
                || existing.credit_opening != credit_opening
            {
                return Err(KagemushaStateErrorV1::CreditConflict(credit_id));
            }
            validate_peer_credit_opening_against_payment(&request, &payment, &credit_opening)?;
            existing
                .durable_acknowledgement
                .validate_against(&request, &payment)?;
            let envelope_digest = canonical_sha256_digest(CREDIT_ENVELOPE_DOMAIN, &payment)?;
            if existing.credit_id != credit_id || existing.envelope_digest != envelope_digest {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            if let Some(pending) = self.pending_credits.get(&credit_id) {
                if pending.request != request
                    || pending.payment != payment
                    || pending.credit_opening != credit_opening
                    || pending.envelope_digest != envelope_digest
                    || pending.stage_certificate != existing.stage_certificate
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                return Ok(StagePaymentOutcomeV1::DuplicatePending {
                    inbox_revision: existing.stage_certificate.statement.journal_revision_after,
                    acknowledgement: existing.durable_acknowledgement.clone(),
                });
            }
            if self.consumed_credits.get(credit_id) != Some(envelope_digest)
                || self
                    .authenticated_history
                    .classify_replay(credit_id, envelope_digest)
                    .map_err(map_authenticated_history_error)?
                    != KagemushaHistoryIdentityClassificationV1::ExactDuplicate
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            return Ok(StagePaymentOutcomeV1::DuplicateConsumed {
                acknowledgement: existing.durable_acknowledgement.clone(),
            });
        }

        // Bound all variable-length material before canonical hashing so conflict classification
        // cannot be used as an oversized-envelope allocation path.
        validate_peer_credit_opening_against_payment(&request, &payment, &credit_opening)?;
        self.validate_peer_payment(&request, &payment)?;
        let envelope_digest = canonical_sha256_digest(CREDIT_ENVELOPE_DOMAIN, &payment)?;
        if self.pending_credits.contains_key(&credit_id) {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        match self
            .authenticated_history
            .classify_replay(credit_id, envelope_digest)
            .map_err(map_authenticated_history_error)?
        {
            KagemushaHistoryIdentityClassificationV1::ExactDuplicate => {
                let acknowledgement = self
                    .accepted_payment_receipts
                    .get(&credit_id)
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                    .durable_acknowledgement
                    .clone();
                return Ok(StagePaymentOutcomeV1::DuplicateConsumed { acknowledgement });
            }
            KagemushaHistoryIdentityClassificationV1::Conflict { .. } => {
                return Err(KagemushaStateErrorV1::CreditConflict(credit_id));
            }
            KagemushaHistoryIdentityClassificationV1::Absent => {}
        }
        // Mint and peer identities share both replay indexes. The external classification above
        // is authoritative across recovery; this local check keeps the exact Pasta witness index
        // synchronized and rejects a corrupted local snapshot before mutation.
        if let Some(existing) = self.consumed_credits.get(credit_id) {
            return Err(if existing == envelope_digest {
                KagemushaStateErrorV1::CreditAlreadyConsumed(credit_id)
            } else {
                KagemushaStateErrorV1::CreditConflict(credit_id)
            });
        }
        let stage_authorization =
            stage_authorization.ok_or(KagemushaStateErrorV1::MissingStageAuthorization)?;
        let expected =
            self.preview_stage_payment(&request, &payment, &credit_opening, staged_at_ms)?;
        let stage_certificate = stage_authorization.stage_certificate;
        validate_guard_bytes(&stage_certificate.guard_bundle)?;
        if stage_certificate.statement != expected {
            return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
        }
        self.guard_verifier
            .verify_credit_stage(&expected, &stage_certificate.guard_bundle)
            .map_err(KagemushaStateErrorV1::GuardRejected)?;
        let durable_acknowledgement = DurableAcknowledgementV1::from_acknowledgement(
            stage_authorization.acknowledgement,
            &request,
            &payment,
        )?;
        if durable_acknowledgement
            .acknowledgement
            .inbox_receipt
            .credit_id
            != credit_id.0
        {
            return Err(KagemushaStateErrorV1::InvalidAcknowledgement);
        }
        let inbox_revision = expected.journal_revision_after;
        let receipt = AcceptedPaymentReceiptV1 {
            credit_id,
            envelope_digest,
            request: request.clone(),
            payment: payment.clone(),
            credit_opening,
            stage_certificate: stage_certificate.clone(),
            durable_acknowledgement: durable_acknowledgement.clone(),
        };
        if self.accepted_payment_receipts.contains_key(&credit_id) {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let staged_credit = StagedCreditV1 {
            request,
            payment,
            credit_opening,
            envelope_digest,
            stage_certificate,
        };
        let next_inbox_capacity = self
            .receiver_inbox_capacity
            .receiver_snapshot_staged_successor(
                receiver_sequence_entry_bytes(&staged_credit)?,
                receiver_sequence_entry_bytes(&receipt)?,
            )?;

        // Every fallible validation and capacity check is complete. These exact-key map inserts
        // cannot fail and their absence was established above, so the visible state changes as
        // one in-memory commit before the next durable snapshot can be exposed.
        let prior_receipt = self.accepted_payment_receipts.insert(credit_id, receipt);
        debug_assert!(prior_receipt.is_none());
        let prior_pending = self.pending_credits.insert(credit_id, staged_credit);
        debug_assert!(prior_pending.is_none());
        self.receiver_inbox_capacity = next_inbox_capacity;
        self.inbox_revision = inbox_revision;
        Ok(StagePaymentOutcomeV1::Staged {
            inbox_revision,
            acknowledgement: durable_acknowledgement,
        })
    }

    /// Preview exact-next hardware epoch rotation without changing balance or replay state.
    pub fn preview_rotate(
        &self,
        next_epoch: HardwareEpochV1,
        next_device_policy_binding: DevicePolicyBindingV1,
        successor_state_nonce_commitment: DigestV1,
        trusted_commit_time_ms: u64,
    ) -> Result<TransitionPreviewV1, KagemushaStateErrorV1> {
        next_epoch.validate()?;
        next_device_policy_binding.validate()?;
        if next_epoch.generation
            != self
                .state
                .hardware_epoch
                .generation
                .checked_add(1)
                .ok_or(KagemushaStateErrorV1::HardwareEpochOverflow)?
            || next_epoch.epoch_id == self.state.hardware_epoch.epoch_id
            || next_device_policy_binding == self.state.device_policy_binding
        {
            return Err(KagemushaStateErrorV1::InvalidHardwareRotation);
        }
        let successor = self.next_state(
            self.state.balance,
            next_epoch,
            next_device_policy_binding,
            successor_state_nonce_commitment,
            self.state.consumed_credit_root,
        )?;
        let effect_digest = canonical_sha256_digest(
            TRANSITION_EFFECT_DOMAIN,
            &RotateEffectV1 {
                predecessor_epoch: self.state.hardware_epoch,
                successor_epoch: next_epoch,
                predecessor_device_policy_binding: self.state.device_policy_binding,
                successor_device_policy_binding: next_device_policy_binding,
                predecessor_state_nonce_commitment: self.state.state_nonce_commitment,
                successor_state_nonce_commitment,
                carried_balance: self.state.balance,
                carried_consumed_credit_root: self.state.consumed_credit_root,
            },
        )?;
        self.transition_preview(
            KagemushaTransitionKindV1::Rotate,
            successor.clone(),
            effect_digest,
            [0; 32],
            [0; 32],
            [0; 32],
            [0; 32],
            TransitionAuxiliaryBindingsV1::default(),
            trusted_commit_time_ms,
            |normalized_guard_statement_digest| {
                local_transition_transport_digest(
                    KagemushaTransitionKindV1::Rotate,
                    self.state.release_id,
                    self.state.liability_pool_id,
                    effect_digest,
                    self.state.state_commitment,
                    successor.state_commitment,
                    normalized_guard_statement_digest,
                )
            },
        )
    }

    /// Verify and atomically rotate to the exact next hardware epoch.
    pub fn rotate(
        &mut self,
        next_epoch: HardwareEpochV1,
        next_device_policy_binding: DevicePolicyBindingV1,
        successor_state_nonce_commitment: DigestV1,
        trusted_commit_time_ms: u64,
        authorization: TransitionAuthorizationV1,
    ) -> Result<KagemushaStateV1, KagemushaStateErrorV1> {
        let preview = self.preview_rotate(
            next_epoch,
            next_device_policy_binding,
            successor_state_nonce_commitment,
            trusted_commit_time_ms,
        )?;
        self.verify_transition_authorization(&preview, &authorization)?;
        self.accepted_recipient_bindings
            .insert(next_device_policy_binding);
        self.commit_preview(preview);
        // Inbox sequence numbers are scoped by the hardware epoch just like monetary journal
        // sequence numbers. Resetting here is safe because durable receipt uniqueness includes
        // the epoch ID, and it lets a qualified rollover continue accepting credits instead of
        // stranding the lane when the previous epoch's inbox counter is saturated.
        self.inbox_revision = 0;
        Ok(self.state.clone())
    }

    /// Build a canonical recovery snapshot with a self-consistent commitment.
    pub fn snapshot(&self) -> Result<KagemushaStateSnapshotV1, KagemushaStateErrorV1> {
        self.validate_mint_inbox_snapshot()?;
        let authenticated_history_roots =
            validate_committed_history_v1(&self.authenticated_history.store)
                .map_err(map_authenticated_history_error)?;
        let authenticated_history_commitment = self
            .authenticated_history
            .store
            .recovery_commitment()
            .map_err(map_authenticated_history_error)?;
        let receiver_snapshot_usage = receiver_snapshot_capacity_usage_v1(
            &self.pending_credits,
            &self.accepted_payment_receipts,
        )?;
        self.receiver_inbox_capacity
            .validate_recovered_with_snapshot_usage(
                receiver_snapshot_usage.total_bytes,
                receiver_snapshot_usage.pending_credit_entry_bytes,
                receiver_snapshot_usage.pending_receipt_entry_bytes,
            )?;
        self.sender_outbox_capacity
            .validate_recovered(&self.outgoing_candidate_journal)?;
        self.outgoing_candidate_journal.validate_recovered(
            &self.state,
            self.journal_revision,
            &self.sender_outbox_capacity,
            &self.proof_release,
            self.proof_release.artifacts,
            &self.recursive_verifier,
        )?;
        let pending_credits = self.pending_credits.values().cloned().collect::<Vec<_>>();
        let accepted_recipient_bindings = self
            .accepted_recipient_bindings
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let accepted_payment_receipts = self
            .accepted_payment_receipts
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let consumed_credits = self.consumed_credits.records();
        let snapshot_commitment = canonical_poseidon_digest(
            SNAPSHOT_COMMITMENT_DOMAIN,
            &SnapshotCommitmentPreimageV1 {
                version: KAGEMUSHA_STATE_VERSION_V1,
                state: self.state.clone(),
                journal_revision: self.journal_revision,
                inbox_revision: self.inbox_revision,
                pending_credits: pending_credits.clone(),
                accepted_recipient_bindings: accepted_recipient_bindings.clone(),
                accepted_payment_receipts: accepted_payment_receipts.clone(),
                mint_inbox: self.mint_inbox.clone(),
                consumed_credits: consumed_credits.clone(),
                authenticated_history_roots,
                authenticated_history_commitment,
                receiver_inbox_capacity: self.receiver_inbox_capacity.clone(),
                sender_outbox_capacity: self.sender_outbox_capacity.clone(),
                outgoing_candidate_journal: self.outgoing_candidate_journal.clone(),
            },
        )?;
        Ok(KagemushaStateSnapshotV1 {
            version: KAGEMUSHA_STATE_VERSION_V1,
            state: self.state.clone(),
            journal_revision: self.journal_revision,
            inbox_revision: self.inbox_revision,
            pending_credits,
            accepted_recipient_bindings,
            accepted_payment_receipts,
            mint_inbox: self.mint_inbox.clone(),
            consumed_credits,
            authenticated_history_roots,
            authenticated_history_commitment,
            receiver_inbox_capacity: self.receiver_inbox_capacity.clone(),
            sender_outbox_capacity: self.sender_outbox_capacity.clone(),
            outgoing_candidate_journal: self.outgoing_candidate_journal.clone(),
            snapshot_commitment,
        })
    }

    /// Preview the exact hardware-sealed recovery anchor for the current snapshot.
    pub fn preview_durability_anchor(
        &self,
    ) -> Result<DurabilityAnchorStatementV1, KagemushaStateErrorV1> {
        let snapshot = self.snapshot()?;
        Ok(DurabilityAnchorStatementV1 {
            version: KAGEMUSHA_STATE_VERSION_V1,
            lane: self.state.lane.clone(),
            state_commitment: self.state.state_commitment,
            hardware_epoch: self.state.hardware_epoch,
            device_policy_binding: self.state.device_policy_binding,
            state_nonce_commitment: self.state.state_nonce_commitment,
            logical_sequence: self.state.logical_sequence,
            journal_revision: self.journal_revision,
            inbox_revision: self.inbox_revision,
            snapshot_commitment: snapshot.snapshot_commitment,
        })
    }

    /// Verify and package a hardware-sealed recovery anchor.
    pub fn seal_durability_anchor(
        &self,
        guard_bundle: Vec<u8>,
    ) -> Result<DurabilityAnchorV1, KagemushaStateErrorV1> {
        validate_guard_bytes(&guard_bundle)?;
        let statement = self.preview_durability_anchor()?;
        self.guard_verifier
            .verify_durability_anchor(&statement, &guard_bundle)
            .map_err(KagemushaStateErrorV1::GuardRejected)?;
        Ok(DurabilityAnchorV1 {
            statement,
            guard_bundle,
        })
    }

    /// Restore a canonical snapshot only when it exactly matches the latest hardware anchor.
    pub fn restore(
        snapshot: KagemushaStateSnapshotV1,
        anchor: &DurabilityAnchorV1,
        proof_release: KagemushaStateProofReleaseV1,
        history_store: H,
        recursive_verifier: R,
        guard_verifier: G,
    ) -> Result<Self, KagemushaStateErrorV1> {
        validate_guard_bytes(&anchor.guard_bundle)?;
        guard_verifier
            .verify_durability_anchor(&anchor.statement, &anchor.guard_bundle)
            .map_err(KagemushaStateErrorV1::GuardRejected)?;
        if snapshot.version != KAGEMUSHA_STATE_VERSION_V1
            || anchor.statement.version != KAGEMUSHA_STATE_VERSION_V1
        {
            return Err(KagemushaStateErrorV1::UnsupportedVersion(snapshot.version));
        }
        snapshot.state.validate()?;
        proof_release.validate_state_context(snapshot.state.context())?;
        if snapshot.state.release_id != proof_release.release_id()
            || snapshot.state.liability_pool_id
                != derive_liability_pool_id(&snapshot.state.lane, snapshot.state.asset_incarnation)?
        {
            return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
        }
        let expected_snapshot_commitment = canonical_poseidon_digest(
            SNAPSHOT_COMMITMENT_DOMAIN,
            &SnapshotCommitmentPreimageV1 {
                version: snapshot.version,
                state: snapshot.state.clone(),
                journal_revision: snapshot.journal_revision,
                inbox_revision: snapshot.inbox_revision,
                pending_credits: snapshot.pending_credits.clone(),
                accepted_recipient_bindings: snapshot.accepted_recipient_bindings.clone(),
                accepted_payment_receipts: snapshot.accepted_payment_receipts.clone(),
                mint_inbox: snapshot.mint_inbox.clone(),
                consumed_credits: snapshot.consumed_credits.clone(),
                authenticated_history_roots: snapshot.authenticated_history_roots,
                authenticated_history_commitment: snapshot.authenticated_history_commitment,
                receiver_inbox_capacity: snapshot.receiver_inbox_capacity.clone(),
                sender_outbox_capacity: snapshot.sender_outbox_capacity.clone(),
                outgoing_candidate_journal: snapshot.outgoing_candidate_journal.clone(),
            },
        )?;
        if expected_snapshot_commitment != snapshot.snapshot_commitment {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let expected_anchor = DurabilityAnchorStatementV1 {
            version: KAGEMUSHA_STATE_VERSION_V1,
            lane: snapshot.state.lane.clone(),
            state_commitment: snapshot.state.state_commitment,
            hardware_epoch: snapshot.state.hardware_epoch,
            device_policy_binding: snapshot.state.device_policy_binding,
            state_nonce_commitment: snapshot.state.state_nonce_commitment,
            logical_sequence: snapshot.state.logical_sequence,
            journal_revision: snapshot.journal_revision,
            inbox_revision: snapshot.inbox_revision,
            snapshot_commitment: snapshot.snapshot_commitment,
        };
        if anchor.statement != expected_anchor {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        let authenticated_history = KagemushaStateAuthenticatedHistoryV1::recover(
            history_store,
            snapshot.authenticated_history_roots,
            snapshot.authenticated_history_commitment,
        )
        .map_err(map_authenticated_history_error)?;
        let consumed_credits = ExactConsumedCreditIndex::from_records(&snapshot.consumed_credits)?;
        if consumed_credits.root() != snapshot.state.consumed_credit_root {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        snapshot
            .sender_outbox_capacity
            .validate_recovered(&snapshot.outgoing_candidate_journal)?;
        snapshot.outgoing_candidate_journal.validate_recovered(
            &snapshot.state,
            snapshot.journal_revision,
            &snapshot.sender_outbox_capacity,
            &proof_release,
            proof_release.artifacts,
            &recursive_verifier,
        )?;
        let mut accepted_recipient_bindings = BTreeSet::new();
        let mut previous_binding = None;
        for binding in snapshot.accepted_recipient_bindings {
            binding
                .validate()
                .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
            if previous_binding.is_some_and(|previous| previous >= binding)
                || !accepted_recipient_bindings.insert(binding)
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            previous_binding = Some(binding);
        }
        if !accepted_recipient_bindings.contains(&snapshot.state.device_policy_binding) {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let pending_receipt_credit_ids = snapshot
            .pending_credits
            .iter()
            .map(|staged| CreditIdV1(staged.payment.output.credit_id))
            .collect::<BTreeSet<_>>();
        if pending_receipt_credit_ids.len() != snapshot.pending_credits.len() {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let mut accepted_payment_receipts = BTreeMap::new();
        let mut receipt_stage_revisions = BTreeSet::new();
        let mut previous_receipt_credit_id = None;
        for receipt in snapshot.accepted_payment_receipts {
            let credit_id = CreditIdV1(receipt.payment.output.credit_id);
            validate_peer_payment_wire_shape_against_lane(
                &snapshot.state,
                &receipt.request,
                &receipt.payment,
            )
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
            let envelope_digest =
                canonical_sha256_digest(CREDIT_ENVELOPE_DOMAIN, &receipt.payment)?;
            if pending_receipt_credit_ids.contains(&credit_id) {
                validate_peer_payment_against_context(
                    &snapshot.state,
                    &proof_release,
                    &recursive_verifier,
                    &receipt.request,
                    &receipt.payment,
                )
                .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
            } else {
                // A consumed receipt remains authoritative because the verified durability
                // anchor commits its exact bytes and both replay indexes commit the same
                // envelope. It must still satisfy every bounded self-authenticating wire check,
                // stable lane/opening binding, and stage GuardBundle below. Re-running every
                // historical recursive proof would make recovery work grow with payment history;
                // pending value never takes this anchored-history fast path.
                if consumed_credits.get(credit_id) != Some(envelope_digest)
                    || authenticated_history
                        .classify_replay(credit_id, envelope_digest)
                        .map_err(map_authenticated_history_error)?
                        != KagemushaHistoryIdentityClassificationV1::ExactDuplicate
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
            }
            validate_peer_credit_opening_against_payment(
                &receipt.request,
                &receipt.payment,
                &receipt.credit_opening,
            )
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
            receipt
                .durable_acknowledgement
                .validate_against(&receipt.request, &receipt.payment)?;
            let statement = &receipt.stage_certificate.statement;
            if previous_receipt_credit_id.is_some_and(|previous| previous >= credit_id)
                || receipt.credit_id != credit_id
                || receipt.envelope_digest != envelope_digest
                || statement.version != KAGEMUSHA_STATE_VERSION_V1
                || statement.recipient_lane != snapshot.state.lane
                || statement.receiver_hardware_epoch.validate().is_err()
                || statement.receiver_device_policy_binding.validate().is_err()
                || !accepted_recipient_bindings.contains(&statement.receiver_device_policy_binding)
                || statement.receiver_state_commitment == [0; 32]
                || statement.receiver_state_nonce_commitment == [0; 32]
                || statement.credit_id != credit_id
                || statement.envelope_digest != receipt.envelope_digest
                || statement.journal_revision_after
                    != statement
                        .journal_revision_before
                        .checked_add(1)
                        .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?
                || statement.receiver_hardware_epoch.generation
                    > snapshot.state.hardware_epoch.generation
                || (statement.receiver_hardware_epoch.generation
                    == snapshot.state.hardware_epoch.generation
                    && statement.receiver_hardware_epoch != snapshot.state.hardware_epoch)
                || (statement.receiver_hardware_epoch == snapshot.state.hardware_epoch
                    && statement.journal_revision_after > snapshot.inbox_revision)
                || !receipt_stage_revisions.insert((
                    statement.receiver_hardware_epoch.epoch_id,
                    statement.journal_revision_after,
                ))
                || receipt
                    .durable_acknowledgement
                    .acknowledgement
                    .inbox_receipt
                    .credit_id
                    != credit_id.0
                || accepted_payment_receipts
                    .insert(credit_id, receipt.clone())
                    .is_some()
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            validate_guard_bytes(&receipt.stage_certificate.guard_bundle)?;
            guard_verifier
                .verify_credit_stage(statement, &receipt.stage_certificate.guard_bundle)
                .map_err(KagemushaStateErrorV1::GuardRejected)?;
            previous_receipt_credit_id = Some(credit_id);
        }
        let mut pending_credits = BTreeMap::new();
        let mut stage_revisions = BTreeSet::new();
        let mut previous_credit_id = None;
        for staged in snapshot.pending_credits {
            validate_peer_payment_against_context(
                &snapshot.state,
                &proof_release,
                &recursive_verifier,
                &staged.request,
                &staged.payment,
            )
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
            validate_peer_credit_opening_against_payment(
                &staged.request,
                &staged.payment,
                &staged.credit_opening,
            )
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
            let credit_id = CreditIdV1(staged.payment.output.credit_id);
            let statement = &staged.stage_certificate.statement;
            if previous_credit_id.is_some_and(|previous| previous >= credit_id)
                || staged.envelope_digest
                    != canonical_sha256_digest(CREDIT_ENVELOPE_DOMAIN, &staged.payment)?
                || consumed_credits.get(credit_id).is_some()
                || statement.version != KAGEMUSHA_STATE_VERSION_V1
                || statement.recipient_lane != snapshot.state.lane
                || statement.receiver_hardware_epoch.validate().is_err()
                || statement.receiver_device_policy_binding.validate().is_err()
                || !accepted_recipient_bindings.contains(&statement.receiver_device_policy_binding)
                || statement.receiver_state_commitment == [0; 32]
                || statement.receiver_state_nonce_commitment == [0; 32]
                || statement.credit_id != credit_id
                || statement.envelope_digest != staged.envelope_digest
                || statement.journal_revision_after
                    != statement
                        .journal_revision_before
                        .checked_add(1)
                        .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?
                || statement.receiver_hardware_epoch.generation
                    > snapshot.state.hardware_epoch.generation
                || (statement.receiver_hardware_epoch.generation
                    == snapshot.state.hardware_epoch.generation
                    && statement.receiver_hardware_epoch != snapshot.state.hardware_epoch)
                || (statement.receiver_hardware_epoch == snapshot.state.hardware_epoch
                    && statement.journal_revision_after > snapshot.inbox_revision)
                || !stage_revisions.insert((
                    statement.receiver_hardware_epoch.epoch_id,
                    statement.journal_revision_after,
                ))
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            let receipt = accepted_payment_receipts
                .get(&credit_id)
                .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
            if receipt.request != staged.request
                || receipt.payment != staged.payment
                || receipt.credit_opening != staged.credit_opening
                || receipt.envelope_digest != staged.envelope_digest
                || receipt.stage_certificate != staged.stage_certificate
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            if authenticated_history
                .classify_replay(credit_id, staged.envelope_digest)
                .map_err(map_authenticated_history_error)?
                != KagemushaHistoryIdentityClassificationV1::Absent
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            validate_guard_bytes(&staged.stage_certificate.guard_bundle)?;
            guard_verifier
                .verify_credit_stage(statement, &staged.stage_certificate.guard_bundle)
                .map_err(KagemushaStateErrorV1::GuardRejected)?;
            previous_credit_id = Some(credit_id);
            pending_credits.insert(credit_id, staged);
        }
        for (credit_id, receipt) in &accepted_payment_receipts {
            if !pending_credits.contains_key(credit_id) {
                if consumed_credits.get(*credit_id) != Some(receipt.envelope_digest)
                    || authenticated_history
                        .classify_replay(*credit_id, receipt.envelope_digest)
                        .map_err(map_authenticated_history_error)?
                        != KagemushaHistoryIdentityClassificationV1::ExactDuplicate
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
            }
        }
        let receiver_snapshot_usage =
            receiver_snapshot_capacity_usage_v1(&pending_credits, &accepted_payment_receipts)?;
        snapshot
            .receiver_inbox_capacity
            .validate_recovered_with_snapshot_usage(
                receiver_snapshot_usage.total_bytes,
                receiver_snapshot_usage.pending_credit_entry_bytes,
                receiver_snapshot_usage.pending_receipt_entry_bytes,
            )?;
        let recovered = Self {
            state: snapshot.state,
            journal_revision: snapshot.journal_revision,
            inbox_revision: snapshot.inbox_revision,
            pending_credits,
            accepted_recipient_bindings,
            accepted_payment_receipts,
            mint_inbox: snapshot.mint_inbox,
            consumed_credits,
            authenticated_history,
            receiver_inbox_capacity: snapshot.receiver_inbox_capacity,
            sender_outbox_capacity: snapshot.sender_outbox_capacity,
            outgoing_candidate_journal: snapshot.outgoing_candidate_journal,
            proof_release,
            recursive_verifier,
            guard_verifier,
        };
        recovered.validate_mint_inbox_snapshot()?;
        recovered.reauthenticate_pending_mint_finality()?;
        Ok(recovered)
    }

    fn validate_peer_payment(
        &self,
        request: &KagemushaPaymentRequestV1,
        payment: &KagemushaPaymentV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        validate_peer_payment_against_context(
            &self.state,
            &self.proof_release,
            &self.recursive_verifier,
            request,
            payment,
        )
    }

    fn ensure_credit_id_available(
        &self,
        credit_id: CreditIdV1,
        envelope_digest: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.mint_inbox.contains_credit_id(credit_id) {
            return Err(KagemushaStateErrorV1::CreditConflict(credit_id));
        }
        self.ensure_non_mint_credit_id_available(credit_id, envelope_digest)
    }

    fn ensure_non_mint_credit_id_available(
        &self,
        credit_id: CreditIdV1,
        envelope_digest: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        if let Some(existing) = self.pending_credits.get(&credit_id) {
            return if existing.envelope_digest == envelope_digest {
                Err(KagemushaStateErrorV1::CreditAlreadyConsumed(credit_id))
            } else {
                Err(KagemushaStateErrorV1::CreditConflict(credit_id))
            };
        }
        if let Some(existing) = self.consumed_credits.get(credit_id) {
            return if existing == envelope_digest {
                Err(KagemushaStateErrorV1::CreditAlreadyConsumed(credit_id))
            } else {
                Err(KagemushaStateErrorV1::CreditConflict(credit_id))
            };
        }
        Ok(())
    }

    fn ensure_peer_credit_id_available(
        &self,
        credit_id: CreditIdV1,
        envelope_digest: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        if let Some(existing) = self.pending_credits.get(&credit_id) {
            return Err(if existing.envelope_digest == envelope_digest {
                KagemushaStateErrorV1::CreditAlreadyConsumed(credit_id)
            } else {
                KagemushaStateErrorV1::CreditConflict(credit_id)
            });
        }
        match self
            .authenticated_history
            .classify_replay(credit_id, envelope_digest)
            .map_err(map_authenticated_history_error)?
        {
            KagemushaHistoryIdentityClassificationV1::Absent => {
                self.ensure_credit_id_available(credit_id, envelope_digest)
            }
            KagemushaHistoryIdentityClassificationV1::ExactDuplicate => {
                Err(KagemushaStateErrorV1::CreditAlreadyConsumed(credit_id))
            }
            KagemushaHistoryIdentityClassificationV1::Conflict { .. } => {
                Err(KagemushaStateErrorV1::CreditConflict(credit_id))
            }
        }
    }

    fn next_state(
        &self,
        balance: u128,
        hardware_epoch: HardwareEpochV1,
        device_policy_binding: DevicePolicyBindingV1,
        state_nonce_commitment: DigestV1,
        consumed_credit_root: KagemushaPastaStateCommitmentV1,
    ) -> Result<KagemushaStateV1, KagemushaStateErrorV1> {
        if state_nonce_commitment == [0; 32]
            || state_nonce_commitment == self.state.state_nonce_commitment
        {
            return Err(KagemushaStateErrorV1::InvalidStateNonceCommitment);
        }
        let logical_sequence = if hardware_epoch == self.state.hardware_epoch {
            self.state
                .logical_sequence
                .checked_add(1)
                .ok_or(KagemushaStateErrorV1::SequenceOverflow)?
        } else {
            0
        };
        KagemushaStateV1::build(
            self.state.context(),
            self.state.liability_pool_id,
            self.state.lane.clone(),
            balance,
            logical_sequence,
            hardware_epoch,
            device_policy_binding,
            state_nonce_commitment,
            consumed_credit_root,
        )
    }

    fn transition_preview<F>(
        &self,
        kind: KagemushaTransitionKindV1,
        successor: KagemushaStateV1,
        effect_digest: DigestV1,
        mint_finality_semantic_digest: DigestV1,
        mint_finality_proof_binding_digest: DigestV1,
        peer_credit_id: DigestV1,
        recipient_encryption_key_binding: DigestV1,
        auxiliary: TransitionAuxiliaryBindingsV1,
        trusted_commit_time_ms: u64,
        transport_semantic_digest: F,
    ) -> Result<TransitionPreviewV1, KagemushaStateErrorV1>
    where
        F: FnOnce(DigestV1) -> Result<DigestV1, KagemushaStateErrorV1>,
    {
        self.transition_preview_with_artifacts(
            self.proof_release.artifacts,
            kind,
            successor,
            effect_digest,
            mint_finality_semantic_digest,
            mint_finality_proof_binding_digest,
            peer_credit_id,
            recipient_encryption_key_binding,
            auxiliary,
            trusted_commit_time_ms,
            transport_semantic_digest,
        )
    }

    fn transition_preview_with_artifacts<F>(
        &self,
        artifacts: KagemushaRecursionArtifactsV1,
        kind: KagemushaTransitionKindV1,
        successor: KagemushaStateV1,
        effect_digest: DigestV1,
        mint_finality_semantic_digest: DigestV1,
        mint_finality_proof_binding_digest: DigestV1,
        peer_credit_id: DigestV1,
        recipient_encryption_key_binding: DigestV1,
        auxiliary: TransitionAuxiliaryBindingsV1,
        trusted_commit_time_ms: u64,
        transport_semantic_digest: F,
    ) -> Result<TransitionPreviewV1, KagemushaStateErrorV1>
    where
        F: FnOnce(DigestV1) -> Result<DigestV1, KagemushaStateErrorV1>,
    {
        if !matches!(
            self.outgoing_candidate_journal.stage(),
            KagemushaOutgoingJournalStageV1::Empty
        ) {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        if trusted_commit_time_ms == 0 {
            return Err(KagemushaStateErrorV1::InvalidTrustedCommitTime);
        }
        if artifacts.release_id != successor.release_id {
            return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
        }
        if (kind == KagemushaTransitionKindV1::MintFold)
            != (mint_finality_semantic_digest != [0; 32])
            || (kind == KagemushaTransitionKindV1::MintFold)
                != (mint_finality_proof_binding_digest != [0; 32])
        {
            return Err(KagemushaStateErrorV1::InvalidMintCredit);
        }
        let is_send = kind == KagemushaTransitionKindV1::SendSplit;
        if is_send != (peer_credit_id != [0; 32])
            || is_send != (recipient_encryption_key_binding != [0; 32])
        {
            return Err(KagemushaStateErrorV1::InvalidPeerCredit);
        }
        let is_receive = kind == KagemushaTransitionKindV1::ReceiveFold;
        if is_receive != (auxiliary.receive_credit_binding_digest != [0; 32]) {
            return Err(KagemushaStateErrorV1::InvalidPeerCredit);
        }
        let suite_changed = self.state.suite_id != successor.suite_id;
        let vk_changed = self.state.vk_digest != successor.vk_digest;
        let release_changed = self.state.release_id != successor.release_id;
        if suite_changed || vk_changed || release_changed {
            return Err(KagemushaStateErrorV1::StateInvariant);
        }
        let is_terminal = matches!(
            kind,
            KagemushaTransitionKindV1::SendSplit | KagemushaTransitionKindV1::RedeemSplit
        );
        let lifecycle_binding_digest =
            if auxiliary.lifecycle_binding_digest == [0; 32] && !is_terminal {
                canonical_sha256_digest(
                    TRANSITION_LIFECYCLE_DOMAIN,
                    &(
                        kind,
                        self.state.protocol_version,
                        successor.suite_id,
                        successor.vk_digest,
                        successor.release_id,
                        self.state.asset_incarnation,
                        self.state.liability_pool_id,
                        self.state.hardware_profile_id,
                        self.state.policy_epoch,
                        effect_digest,
                    ),
                )?
            } else {
                auxiliary.lifecycle_binding_digest
            };
        if lifecycle_binding_digest == [0; 32]
            || is_terminal != (auxiliary.prepared_transition_binding_digest != [0; 32])
        {
            return Err(KagemushaStateErrorV1::StateInvariant);
        }
        let journal_revision_after = if kind == KagemushaTransitionKindV1::Rotate {
            0
        } else {
            self.journal_revision
                .checked_add(1)
                .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?
        };
        let proof_statement = TransitionProofStatementV1 {
            version: KAGEMUSHA_STATE_VERSION_V1,
            protocol_version: self.state.protocol_version,
            predecessor_suite_id: self.state.suite_id,
            predecessor_vk_digest: self.state.vk_digest,
            successor_suite_id: successor.suite_id,
            successor_vk_digest: successor.vk_digest,
            kind,
            amount: match kind {
                KagemushaTransitionKindV1::MintFold | KagemushaTransitionKindV1::ReceiveFold => {
                    successor
                        .balance
                        .checked_sub(self.state.balance)
                        .ok_or(KagemushaStateErrorV1::StateInvariant)?
                }
                KagemushaTransitionKindV1::SendSplit | KagemushaTransitionKindV1::RedeemSplit => {
                    self.state
                        .balance
                        .checked_sub(successor.balance)
                        .ok_or(KagemushaStateErrorV1::StateInvariant)?
                }
                KagemushaTransitionKindV1::Rotate => 0,
            },
            mint_finality_semantic_digest,
            mint_finality_proof_binding_digest,
            peer_credit_id,
            recipient_encryption_key_binding,
            lifecycle_binding_digest,
            prepared_transition_binding_digest: auxiliary.prepared_transition_binding_digest,
            receive_credit_binding_digest: auxiliary.receive_credit_binding_digest,
            predecessor_release_id: self.state.release_id,
            release_id: successor.release_id,
            asset_incarnation: self.state.asset_incarnation,
            liability_pool_id: self.state.liability_pool_id,
            hardware_profile_id: self.state.hardware_profile_id,
            policy_epoch: self.state.policy_epoch,
            lane: self.state.lane.clone(),
            predecessor_commitment: self.state.state_commitment,
            successor_commitment: successor.state_commitment,
            predecessor_sequence: self.state.logical_sequence,
            successor_sequence: successor.logical_sequence,
            predecessor_epoch: self.state.hardware_epoch,
            successor_epoch: successor.hardware_epoch,
            predecessor_device_policy_binding: self.state.device_policy_binding,
            successor_device_policy_binding: successor.device_policy_binding,
            predecessor_state_nonce_commitment: self.state.state_nonce_commitment,
            successor_state_nonce_commitment: successor.state_nonce_commitment,
            journal_revision_before: self.journal_revision,
            journal_revision_after,
            effect_digest,
        };
        let guard_context =
            transition_guard_context(artifacts, &proof_statement, trusted_commit_time_ms)?;
        let normalized_guard_statement =
            KagemushaNormalizedGuardStatementV1::derive_from_transition(
                &proof_statement,
                guard_context,
            )
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        let normalized_guard_statement_digest = normalized_guard_statement
            .canonical_digest()
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        let state_transition_digest = transition_statement_digest(&proof_statement)?;
        let hardware_statement = HardwareTransitionStatementV1 {
            version: KAGEMUSHA_STATE_VERSION_V1,
            kind,
            amount: proof_statement.amount,
            lane: self.state.lane.clone(),
            predecessor_commitment: self.state.state_commitment,
            successor_commitment: successor.state_commitment,
            predecessor_sequence: self.state.logical_sequence,
            successor_sequence: successor.logical_sequence,
            predecessor_epoch: self.state.hardware_epoch,
            successor_epoch: successor.hardware_epoch,
            predecessor_device_policy_binding: self.state.device_policy_binding,
            successor_device_policy_binding: successor.device_policy_binding,
            predecessor_state_nonce_commitment: self.state.state_nonce_commitment,
            successor_state_nonce_commitment: successor.state_nonce_commitment,
            journal_revision_before: self.journal_revision,
            journal_revision_after,
            state_transition_digest,
            normalized_guard_statement_digest,
        };
        hardware_statement.validate_exact_next()?;
        normalized_guard_statement
            .validate_hardware_binding(&proof_statement, &hardware_statement)
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        let transport_semantic_digest =
            transport_semantic_digest(normalized_guard_statement_digest)?;
        Ok(TransitionPreviewV1 {
            successor,
            proof_statement,
            hardware_statement,
            normalized_guard_statement,
            transport_semantic_digest,
            journal_revision_after,
        })
    }

    fn verify_transition_authorization(
        &self,
        preview: &TransitionPreviewV1,
        authorization: &TransitionAuthorizationV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.verify_transition_authorization_against_release(
            preview,
            authorization,
            &self.proof_release,
        )
    }

    fn verify_transition_authorization_against_release(
        &self,
        preview: &TransitionPreviewV1,
        authorization: &TransitionAuthorizationV1,
        proof_release: &KagemushaStateProofReleaseV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        validate_paired_proof(&authorization.proof, preview.transport_semantic_digest)?;
        validate_guard_bytes(&authorization.hardware_certificate.guard_bundle)?;
        authorization
            .hardware_certificate
            .statement
            .validate_exact_next()?;
        if authorization.hardware_certificate.statement != preview.hardware_statement {
            return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
        }
        preview
            .normalized_guard_statement
            .validate_hardware_binding(&preview.proof_statement, &preview.hardware_statement)
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        let public_inputs = transition_state_public_inputs(
            proof_release.artifacts,
            &self.state,
            preview,
            &authorization.proof,
        )?;
        verify_kagemusha_state_proof_v1(
            &self.recursive_verifier,
            proof_release.artifacts,
            &public_inputs,
            &authorization.proof,
        )
        .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        self.guard_verifier
            .verify_transition(
                &preview.hardware_statement,
                &authorization.hardware_certificate.guard_bundle,
            )
            .map_err(KagemushaStateErrorV1::GuardRejected)
    }

    fn commit_preview(&mut self, preview: TransitionPreviewV1) {
        self.state = preview.successor;
        self.journal_revision = preview.journal_revision_after;
    }
}

fn terminal_lifecycle_binding_v1(
    state: &KagemushaStateV1,
    operation_kind: KagemushaOperationKindV1,
    request_id: DigestV1,
    receiver_lane_commitment: DigestV1,
    ciphertext_digest: DigestV1,
) -> KagemushaLifecycleBindingV1 {
    KagemushaLifecycleBindingV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        network_id: state.lane.network_id,
        protocol_version: state.protocol_version,
        suite_id: state.suite_id,
        vk_digest: state.vk_digest,
        release_id: state.release_id,
        asset: state.lane.asset.clone(),
        asset_incarnation: state.asset_incarnation,
        scale: state.lane.scale,
        liability_pool_id: state.liability_pool_id,
        hardware_profile_id: state.hardware_profile_id,
        policy_epoch: state.policy_epoch,
        operation_kind,
        request_id,
        receiver_lane_commitment,
        credit_id: [0; 32],
        ciphertext_digest,
    }
}

fn required_pending_credit_prefix(
    current_balance: u128,
    amount: u128,
    pending: impl IntoIterator<Item = (CreditIdV1, u128)>,
) -> Result<Vec<CreditIdV1>, KagemushaStateErrorV1> {
    let mut available = current_balance;
    if available >= amount {
        return Ok(Vec::new());
    }

    let mut required = Vec::new();
    for (credit_id, credit_amount) in pending {
        available = available
            .checked_add(credit_amount)
            .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)?;
        required.push(credit_id);
        if available >= amount {
            return Ok(required);
        }
    }

    Err(KagemushaStateErrorV1::InsufficientBalance)
}

#[derive(Clone, Debug, PartialEq, Eq, Encode)]
struct MintFoldEffectV1 {
    credit_id: CreditIdV1,
    envelope_digest: DigestV1,
    amount: u128,
    issuance_digest: DigestV1,
    mint_finality_semantic_digest: DigestV1,
    mint_finality_proof_binding_digest: DigestV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode)]
struct RotateEffectV1 {
    predecessor_epoch: HardwareEpochV1,
    successor_epoch: HardwareEpochV1,
    predecessor_device_policy_binding: DevicePolicyBindingV1,
    successor_device_policy_binding: DevicePolicyBindingV1,
    predecessor_state_nonce_commitment: DigestV1,
    successor_state_nonce_commitment: DigestV1,
    carried_balance: u128,
    carried_consumed_credit_root: KagemushaPastaStateCommitmentV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode)]
struct TransitionIntentPreimageV1 {
    release_id: DigestV1,
    liability_pool_id: DigestV1,
    trusted_commit_time_ms: u64,
    statement: TransitionProofStatementV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode)]
struct RecoveryRecordPreimageV1 {
    transition_intent_digest: DigestV1,
    state_transition_digest: DigestV1,
    successor_state_commitment: DigestV1,
    journal_revision_after: u128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode)]
struct DurableEffectPreimageV1 {
    kind: KagemushaTransitionKindV1,
    transition_effect_digest: DigestV1,
    predecessor_state_commitment: DigestV1,
    successor_state_commitment: DigestV1,
    journal_revision_after: u128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode)]
struct LocalTransitionTransportStatementV1 {
    version: u16,
    kind: KagemushaTransitionKindV1,
    release_id: DigestV1,
    liability_pool_id: DigestV1,
    transition_effect_digest: DigestV1,
    predecessor_state_commitment: DigestV1,
    successor_state_commitment: DigestV1,
    normalized_guard_statement_digest: DigestV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode)]
struct BootstrapIntentPreimageV1 {
    trusted_commit_time_ms: u64,
    statement: BootstrapStatementV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode)]
struct BootstrapRecoveryPreimageV1 {
    transition_intent_digest: DigestV1,
    bootstrap_statement_digest: DigestV1,
    successor_state_commitment: DigestV1,
}

fn derive_liability_pool_id(
    lane: &KagemushaLaneIdV1,
    asset_incarnation: AxtAssetIncarnationV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    kagemusha_liability_pool_id_v1(&lane.network_id, &lane.asset, asset_incarnation)
        .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)
}

fn local_transition_transport_digest(
    kind: KagemushaTransitionKindV1,
    release_id: DigestV1,
    liability_pool_id: DigestV1,
    transition_effect_digest: DigestV1,
    predecessor_state_commitment: DigestV1,
    successor_state_commitment: DigestV1,
    normalized_guard_statement_digest: DigestV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    canonical_sha256_digest(
        TRANSPORT_STATEMENT_DOMAIN,
        &LocalTransitionTransportStatementV1 {
            version: KAGEMUSHA_STATE_VERSION_V1,
            kind,
            release_id,
            liability_pool_id,
            transition_effect_digest,
            predecessor_state_commitment,
            successor_state_commitment,
            normalized_guard_statement_digest,
        },
    )
}

fn bootstrap_guard_context(
    artifacts: KagemushaRecursionArtifactsV1,
    statement: &BootstrapStatementV1,
    trusted_commit_time_ms: u64,
) -> Result<KagemushaGuardContextV1, KagemushaStateErrorV1> {
    let transition_effect_digest = canonical_sha256_digest(TRANSITION_EFFECT_DOMAIN, statement)?;
    let transition_intent_digest = canonical_sha256_digest(
        TRANSITION_INTENT_DOMAIN,
        &BootstrapIntentPreimageV1 {
            trusted_commit_time_ms,
            statement: statement.clone(),
        },
    )?;
    let recovery_record_digest = canonical_sha256_digest(
        RECOVERY_RECORD_DOMAIN,
        &BootstrapRecoveryPreimageV1 {
            transition_intent_digest,
            bootstrap_statement_digest: statement.proof_statement_digest()?,
            successor_state_commitment: statement.state_commitment,
        },
    )?;
    Ok(KagemushaGuardContextV1 {
        release_id: artifacts.release_id,
        liability_pool_id: statement.liability_pool_id,
        lifecycle_binding_digest: canonical_sha256_digest(TRANSITION_LIFECYCLE_DOMAIN, statement)?,
        prepared_transition_binding_digest: [0; 32],
        receive_credit_binding_digest: [0; 32],
        terminal_commit_binding_digest: [0; 32],
        sender_one_time_authorization_digest: [0; 32],
        transition_intent_digest,
        transition_effect_digest,
        recovery_record_digest,
        durable_inbox_effect_digest: artifacts.canonical_empty_effect_digest,
        durable_outbox_effect_digest: artifacts.canonical_empty_effect_digest,
        canonical_empty_effect_digest: artifacts.canonical_empty_effect_digest,
    })
}

fn transition_guard_context(
    artifacts: KagemushaRecursionArtifactsV1,
    statement: &TransitionProofStatementV1,
    trusted_commit_time_ms: u64,
) -> Result<KagemushaGuardContextV1, KagemushaStateErrorV1> {
    let transition_intent_digest = canonical_sha256_digest(
        TRANSITION_INTENT_DOMAIN,
        &TransitionIntentPreimageV1 {
            release_id: artifacts.release_id,
            liability_pool_id: derive_liability_pool_id(
                &statement.lane,
                statement.asset_incarnation,
            )?,
            trusted_commit_time_ms,
            statement: statement.clone(),
        },
    )?;
    let state_transition_digest = transition_statement_digest(statement)?;
    let recovery_record_digest = canonical_sha256_digest(
        RECOVERY_RECORD_DOMAIN,
        &RecoveryRecordPreimageV1 {
            transition_intent_digest,
            state_transition_digest,
            successor_state_commitment: statement.successor_commitment,
            journal_revision_after: statement.journal_revision_after,
        },
    )?;
    let durable_effect = DurableEffectPreimageV1 {
        kind: statement.kind,
        transition_effect_digest: statement.effect_digest,
        predecessor_state_commitment: statement.predecessor_commitment,
        successor_state_commitment: statement.successor_commitment,
        journal_revision_after: statement.journal_revision_after,
    };
    let empty = artifacts.canonical_empty_effect_digest;
    let (durable_inbox_effect_digest, durable_outbox_effect_digest) = match statement.kind {
        KagemushaTransitionKindV1::MintFold | KagemushaTransitionKindV1::ReceiveFold => (
            canonical_sha256_digest(DURABLE_INBOX_EFFECT_DOMAIN, &durable_effect)?,
            empty,
        ),
        KagemushaTransitionKindV1::SendSplit | KagemushaTransitionKindV1::RedeemSplit => (
            empty,
            canonical_sha256_digest(DURABLE_OUTBOX_EFFECT_DOMAIN, &durable_effect)?,
        ),
        KagemushaTransitionKindV1::Rotate => (empty, empty),
    };
    Ok(KagemushaGuardContextV1 {
        release_id: artifacts.release_id,
        liability_pool_id: derive_liability_pool_id(&statement.lane, statement.asset_incarnation)?,
        lifecycle_binding_digest: statement.lifecycle_binding_digest,
        prepared_transition_binding_digest: statement.prepared_transition_binding_digest,
        receive_credit_binding_digest: statement.receive_credit_binding_digest,
        terminal_commit_binding_digest: [0; 32],
        sender_one_time_authorization_digest: [0; 32],
        transition_intent_digest,
        transition_effect_digest: statement.effect_digest,
        recovery_record_digest,
        durable_inbox_effect_digest,
        durable_outbox_effect_digest,
        canonical_empty_effect_digest: empty,
    })
}

fn transition_statement_digest(
    statement: &TransitionProofStatementV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    canonical_sha256_digest(TRANSITION_STATEMENT_DOMAIN, statement)
}

fn transport_semantic_digest(
    normalized_guard_statement_digest: DigestV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    canonical_sha256_digest(
        TRANSPORT_STATEMENT_DOMAIN,
        &normalized_guard_statement_digest,
    )
}

fn bootstrap_state_public_inputs(
    artifacts: KagemushaRecursionArtifactsV1,
    preview: &BootstrapPreviewV1,
    proof: &KagemushaPairedProofV1,
) -> Result<KagemushaStateRelationPublicInputsV1, KagemushaStateErrorV1> {
    let guard = &preview.normalized_guard_statement;
    Ok(KagemushaStateRelationPublicInputsV1 {
        operation: super::kagemusha_v1_recursion::KagemushaOperationV1::Bootstrap,
        predecessor: None,
        successor: preview.state.clone(),
        amount: 0,
        journal_revision_before: 0,
        journal_revision_after: 0,
        transition_effect_digest: guard.transition_effect_digest,
        mint_finality_semantic_digest: [0; 32],
        mint_finality_proof_binding_digest: [0; 32],
        peer_credit_id: [0; 32],
        recipient_encryption_key_binding: [0; 32],
        receive_credit_binding_digest: [0; 32],
        lifecycle_binding_digest: guard.lifecycle_binding_digest,
        prepared_transition_binding_digest: [0; 32],
        transport_semantic_digest: preview.transport_semantic_digest,
        guard_statement_digest: guard
            .canonical_digest()
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?,
        eq_protocol_digest: artifacts.eq_protocol_digest,
        ep_protocol_digest: artifacts.ep_protocol_digest,
        guard_eq_protocol_digest: artifacts
            .guard_bundle_protocol_digest(super::kagemusha_v1_recursion::KagemushaPastaParityV1::Eq)
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?,
        guard_ep_protocol_digest: artifacts
            .guard_bundle_protocol_digest(super::kagemusha_v1_recursion::KagemushaPastaParityV1::Ep)
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?,
        mint_eq_protocol_digest: artifacts
            .mint_finality_protocol_digest(
                super::kagemusha_v1_recursion::KagemushaPastaParityV1::Eq,
            )
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?,
        mint_ep_protocol_digest: artifacts
            .mint_finality_protocol_digest(
                super::kagemusha_v1_recursion::KagemushaPastaParityV1::Ep,
            )
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?,
        mint_authorization_eq_protocol_digest: artifacts.mint_authorization_eq_protocol_digest,
        mint_authorization_ep_protocol_digest: artifacts.mint_authorization_ep_protocol_digest,
        commit_wrapper_eq_protocol_digest: artifacts.commit_wrapper_eq_protocol_digest,
        commit_wrapper_ep_protocol_digest: artifacts.commit_wrapper_ep_protocol_digest,
        guard_eq_credential_audit: proof.guard_eq_credential_audit,
        guard_ep_credential_audit: proof.guard_ep_credential_audit,
        eq_deferred_audit: proof.eq_deferred_audit,
        ep_deferred_audit: proof.ep_deferred_audit,
    })
}

fn transition_state_public_inputs(
    artifacts: KagemushaRecursionArtifactsV1,
    predecessor: &KagemushaStateV1,
    preview: &TransitionPreviewV1,
    proof: &KagemushaPairedProofV1,
) -> Result<KagemushaStateRelationPublicInputsV1, KagemushaStateErrorV1> {
    let statement = &preview.proof_statement;
    let guard_digest = preview
        .normalized_guard_statement
        .canonical_digest()
        .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
    Ok(KagemushaStateRelationPublicInputsV1 {
        operation: statement.kind.into(),
        predecessor: Some(predecessor.clone()),
        successor: preview.successor.clone(),
        amount: statement.amount,
        journal_revision_before: statement.journal_revision_before,
        journal_revision_after: statement.journal_revision_after,
        transition_effect_digest: statement.effect_digest,
        mint_finality_semantic_digest: statement.mint_finality_semantic_digest,
        mint_finality_proof_binding_digest: statement.mint_finality_proof_binding_digest,
        peer_credit_id: statement.peer_credit_id,
        recipient_encryption_key_binding: statement.recipient_encryption_key_binding,
        receive_credit_binding_digest: statement.receive_credit_binding_digest,
        lifecycle_binding_digest: statement.lifecycle_binding_digest,
        prepared_transition_binding_digest: statement.prepared_transition_binding_digest,
        transport_semantic_digest: preview.transport_semantic_digest,
        guard_statement_digest: guard_digest,
        eq_protocol_digest: artifacts.eq_protocol_digest,
        ep_protocol_digest: artifacts.ep_protocol_digest,
        guard_eq_protocol_digest: artifacts
            .guard_bundle_protocol_digest(super::kagemusha_v1_recursion::KagemushaPastaParityV1::Eq)
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?,
        guard_ep_protocol_digest: artifacts
            .guard_bundle_protocol_digest(super::kagemusha_v1_recursion::KagemushaPastaParityV1::Ep)
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?,
        mint_eq_protocol_digest: artifacts
            .mint_finality_protocol_digest(
                super::kagemusha_v1_recursion::KagemushaPastaParityV1::Eq,
            )
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?,
        mint_ep_protocol_digest: artifacts
            .mint_finality_protocol_digest(
                super::kagemusha_v1_recursion::KagemushaPastaParityV1::Ep,
            )
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?,
        mint_authorization_eq_protocol_digest: artifacts.mint_authorization_eq_protocol_digest,
        mint_authorization_ep_protocol_digest: artifacts.mint_authorization_ep_protocol_digest,
        commit_wrapper_eq_protocol_digest: artifacts.commit_wrapper_eq_protocol_digest,
        commit_wrapper_ep_protocol_digest: artifacts.commit_wrapper_ep_protocol_digest,
        guard_eq_credential_audit: proof.guard_eq_credential_audit,
        guard_ep_credential_audit: proof.guard_ep_credential_audit,
        eq_deferred_audit: proof.eq_deferred_audit,
        ep_deferred_audit: proof.ep_deferred_audit,
    })
}

fn validate_peer_payment_wire_shape_against_lane(
    state: &KagemushaStateV1,
    request: &KagemushaPaymentRequestV1,
    payment: &KagemushaPaymentV1,
) -> Result<(), KagemushaStateErrorV1> {
    request
        .validate_shape()
        .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;
    payment
        .validate_shape_against(request)
        .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;
    if request.network_id != state.lane.network_id
        || request.asset != state.lane.asset
        || request.asset_incarnation != state.asset_incarnation
        || request.scale != state.lane.scale
        || request.liability_pool_id != state.liability_pool_id
        || request.hardware_credential.lane_commitment != state.lane.device_lane_id
    {
        return Err(KagemushaStateErrorV1::InvalidPeerCredit);
    }
    Ok(())
}

fn validate_peer_credit_opening_against_payment(
    request: &KagemushaPaymentRequestV1,
    payment: &KagemushaPaymentV1,
    credit_opening: &KagemushaCreditOpeningV1,
) -> Result<(), KagemushaStateErrorV1> {
    let amount = request.amount;
    credit_opening
        .validate_shape_against(payment.output.credit_id, amount)
        .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;
    let request_digest = request
        .canonical_digest()
        .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;
    let expected_commitment = kagemusha_peer_credit_opening_commitment_v1(
        request_digest,
        request.recipient_encryption_key,
        amount,
        credit_opening.credit_commitment_opening,
        credit_opening.recipient_binding_opening,
        credit_opening.recovery_nonce,
    )
    .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;
    if expected_commitment != payment.output.ciphertext_commitment {
        return Err(KagemushaStateErrorV1::InvalidPeerCredit);
    }
    Ok(())
}

fn validate_peer_payment_against_context<R: KagemushaRecursiveVerifierV1>(
    state: &KagemushaStateV1,
    proof_release: &KagemushaStateProofReleaseV1,
    recursive_verifier: &R,
    request: &KagemushaPaymentRequestV1,
    payment: &KagemushaPaymentV1,
) -> Result<(), KagemushaStateErrorV1> {
    proof_release.validate_payment_request(state, request)?;
    candidate_lifecycle::validate_request_against_state(request, state)?;
    validate_peer_payment_wire_shape_against_lane(state, request, payment)?;
    if payment.proof.eq_protocol_digest != proof_release.artifacts.commit_wrapper_eq_protocol_digest
        || payment.proof.ep_protocol_digest
            != proof_release.artifacts.commit_wrapper_ep_protocol_digest
    {
        return Err(KagemushaStateErrorV1::InvalidPeerCredit);
    }
    recursive_verifier
        .verify_payment_and_decide(request, payment)
        .map_err(KagemushaStateErrorV1::ProofRejected)
}

fn canonical_sha256_digest<T: Encode>(
    domain: &[u8],
    value: &T,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    let encoded =
        norito::encode_canonical(value).map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
    let mut hasher = Sha256::new();
    hasher.update(
        u64::try_from(domain.len())
            .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?
            .to_be_bytes(),
    );
    hasher.update(domain);
    hasher.update(
        u64::try_from(encoded.len())
            .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?
            .to_be_bytes(),
    );
    hasher.update(encoded);
    Ok(hasher.finalize().into())
}

fn canonical_poseidon_digest<T: Encode>(
    domain: &[u8],
    value: &T,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    let encoded =
        norito::encode_canonical(value).map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
    let mut framed = Vec::with_capacity(
        domain
            .len()
            .saturating_add(encoded.len())
            .saturating_add(16),
    );
    framed.extend_from_slice(
        &u64::try_from(domain.len())
            .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?
            .to_be_bytes(),
    );
    framed.extend_from_slice(domain);
    framed.extend_from_slice(
        &u64::try_from(encoded.len())
            .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?
            .to_be_bytes(),
    );
    framed.extend_from_slice(&encoded);
    Ok(poseidon::hash_bytes(&framed))
}

fn validate_paired_proof(
    proof: &KagemushaPairedProofV1,
    semantic_digest: DigestV1,
) -> Result<(), KagemushaStateErrorV1> {
    proof
        .validate_shape_for_semantic_digest(semantic_digest)
        .map_err(|_| KagemushaStateErrorV1::InvalidProofBundle)
}

fn validate_guard_bytes(bytes: &[u8]) -> Result<(), KagemushaStateErrorV1> {
    if bytes.is_empty() || bytes.len() > KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1 {
        return Err(KagemushaStateErrorV1::InvalidGuardBundle);
    }
    Ok(())
}
