//! Pure Core state machine for aggregate, hardware-guarded Offline Cash V1 balances.
//!
//! This module owns deterministic host state, conservation checks, durable credit staging, exact
//! replay accounting, and crash-recovery projections. It deliberately does **not** treat host
//! checks as recursive-proof authority. Every bootstrap, monetary transition, peer credit, and
//! durable journal seal crosses explicit proof and hardware-guard verifier hooks; the supplied
//! reject-all implementations make an unintegrated deployment fail closed.

mod batch;
mod candidate_lifecycle;
mod sparse_merkle;

pub use batch::{ReceiveFoldCreditPreviewV1, ReceiveFoldPreviewV1};
pub use candidate_lifecycle::{
    AcceptanceTicketNoCommitClosureOutcomeV1, AcceptanceTicketNoCommitRecoveryOutcomeV1,
    AcceptanceTicketReservationOutcomeV1, AcceptanceTicketUseOutcomeV1,
    CommittedOutgoingCandidateV1, DurableAcceptanceTicketDecisionV1, DurableOutgoingEnvelopeV1,
    OfflineCashAcceptanceIntentAuthorizationDecisionV1,
    OfflineCashAcceptanceIntentAuthorizationVerifierV1, OfflineCashAcceptanceTicketBookV1,
    OfflineCashCandidateProofVerifierV1, OfflineCashCommitWrapperPublicInputsV1,
    OfflineCashCommitWrapperVerifierV1, OfflineCashDurableCapacityV1,
    OfflineCashOutgoingCandidateJournalV1, OfflineCashOutgoingCommitCapabilityV1,
    OfflineCashOutgoingEnvelopeV1, OfflineCashOutgoingJournalStageV1,
    OfflineCashSenderOutboxCapacityV1, PersistedOutgoingCandidateV1, PreparedOutgoingCandidateV1,
    PreparedRedemptionMaterialV1, PreparedSendMaterialV1, SenderOutboxReservationOutcomeV1,
    VerifiedOfflineCashAcceptanceIntentAuthorizationV1,
};

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::{
    NetworkId,
    asset::AssetDefinitionId,
    nexus::AxtAssetIncarnationV1,
    offline::{
        OFFLINE_CASH_ASSET_SCALE_MAX_V1, OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1,
        OfflineCashAcknowledgementV1, OfflineCashAuthenticatedReleaseV1, OfflineCashMintCreditV1,
        OfflineCashPairedProofV1, OfflineCashPastaStateCommitmentV1, OfflineCashPaymentRequestV1,
        OfflineCashPaymentV1, offline_cash_asset_identity_digest_v1,
        offline_cash_liability_pool_id_v1, offline_cash_pasta_state_commitment_v1,
    },
};
use iroha_zkp_halo2::poseidon;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use self::sparse_merkle::ExactConsumedCreditIndex;
use super::offline_cash_v1_poseidon::{
    OFFLINE_CASH_STATE_DOMAIN_V1, OfflineCashPoseidonFieldV1, decode as decode_pasta, digest_limbs,
    encode as encode_pasta, from_u128 as pasta_from_u128, hash as pasta_hash,
};
use super::offline_cash_v1_recursion::{
    OfflineCashGuardContextV1, OfflineCashNormalizedGuardStatementV1,
    OfflineCashRecursionArtifactsV1, OfflineCashRecursivePublicOutputV1,
    OfflineCashRecursiveVerifierV1, OfflineCashStateRelationPublicInputsV1,
    VerifiedOfflineCashMintFinalityHelperV1, canonical_terminal_send_output_binding_v1,
    offline_cash_incoming_proof_binding_digest_v1, verify_offline_cash_recursive_proof_v1,
    verify_offline_cash_state_proof_v1,
};

// Runtime activation installs the authenticated recursive verifier and hardware provider. The
// explicit reject-all implementations remain the fail-closed default when either authority is
// absent; tests may supply narrow mocks through the same interfaces.

/// Offline Cash V1 state-machine version.
pub const OFFLINE_CASH_STATE_VERSION_V1: u16 = 1;
/// Maximum opaque proof bytes accepted by a state-machine hook.
pub const OFFLINE_CASH_PROOF_BUNDLE_MAX_BYTES_V1: usize = OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1;
/// Maximum opaque hardware GuardBundle bytes accepted by a state-machine hook.
pub const OFFLINE_CASH_GUARD_BUNDLE_MAX_BYTES_V1: usize = 65_536;
/// Exact depth of the consumed-credit sparse-Merkle tree.
pub const OFFLINE_CASH_CONSUMED_CREDIT_TREE_DEPTH_V1: usize = 256;

const STATE_COMMITMENT_DOMAIN: &[u8] = b"iroha:offline-cash:v1:state-commitment\0";
const SNAPSHOT_COMMITMENT_DOMAIN: &[u8] = b"iroha:offline-cash:v1:snapshot-commitment\0";
const BOOTSTRAP_STATEMENT_DOMAIN: &[u8] = b"iroha:offline-cash:v1:bootstrap-statement\0";
const MINT_CREDIT_DOMAIN: &[u8] = b"iroha:offline-cash:v1:mint-credit\0";
const CREDIT_ENVELOPE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:peer-credit-envelope\0";
const TRANSITION_EFFECT_DOMAIN: &[u8] = b"iroha:offline-cash:v1:transition-effect\0";
const TRANSITION_STATEMENT_DOMAIN: &[u8] = b"iroha:offline-cash:v1:transition-statement\0";
const TRANSITION_LIFECYCLE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:transition-lifecycle\0";
const TRANSPORT_STATEMENT_DOMAIN: &[u8] = b"iroha:offline-cash:v1:transport-statement\0";
const EMPTY_DURABLE_EFFECT_DOMAIN: &[u8] = b"iroha:offline-cash:v1:durable-effect:empty\0";
const TRANSITION_INTENT_DOMAIN: &[u8] = b"iroha:offline-cash:v1:transition-intent\0";
const RECOVERY_RECORD_DOMAIN: &[u8] = b"iroha:offline-cash:v1:recovery-record\0";
const DURABLE_INBOX_EFFECT_DOMAIN: &[u8] = b"iroha:offline-cash:v1:durable-inbox-effect\0";
const DURABLE_OUTBOX_EFFECT_DOMAIN: &[u8] = b"iroha:offline-cash:v1:durable-outbox-effect\0";

/// Canonical 32-byte digest used by this state machine.
pub type DigestV1 = [u8; 32];

/// Opaque state-machine capability derived from one threshold-authenticated proof release.
///
/// Production callers cannot construct this from artifact digests. They must first authenticate
/// the complete release manifest and authority threshold, then derive this capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfflineCashStateProofReleaseV1 {
    artifacts: OfflineCashRecursionArtifactsV1,
}

impl OfflineCashStateProofReleaseV1 {
    /// Derive state-machine proof authority from an authenticated V1 release.
    ///
    /// # Errors
    ///
    /// Returns an error only if canonical derivation of the release-fixed empty effect fails.
    pub fn from_authenticated_release(
        release: &OfflineCashAuthenticatedReleaseV1,
    ) -> Result<Self, OfflineCashStateErrorV1> {
        let canonical_empty_effect_digest =
            canonical_sha256_digest(EMPTY_DURABLE_EFFECT_DOMAIN, &release.release_id())?;
        Ok(Self {
            artifacts: OfflineCashRecursionArtifactsV1::from_authenticated_release(
                release,
                canonical_empty_effect_digest,
            ),
        })
    }

    /// Return the threshold-authenticated release identifier.
    #[must_use]
    pub const fn release_id(self) -> DigestV1 {
        self.artifacts.release_id
    }

    /// Return the release-fixed digest representing an empty durable transition effect.
    #[must_use]
    pub const fn canonical_empty_effect_digest(self) -> DigestV1 {
        self.artifacts.canonical_empty_effect_digest
    }

    #[cfg(test)]
    const fn from_test_artifacts(artifacts: OfflineCashRecursionArtifactsV1) -> Self {
        Self { artifacts }
    }
}

/// Stable identity of one hardware lane and asset on one network.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub struct OfflineCashLaneIdV1 {
    /// Exact typed network identity used by the public wire statement.
    pub network_id: NetworkId,
    /// Stable device-lane identity, retained across hardware epoch rotation.
    pub device_lane_id: DigestV1,
    /// Exact typed asset identity used by the public wire statement.
    pub asset: AssetDefinitionId,
    /// Authoritative decimal scale of the asset.
    pub scale: u32,
}

impl OfflineCashLaneIdV1 {
    fn validate(&self) -> Result<(), OfflineCashStateErrorV1> {
        if self.network_id.as_bytes() == &[0; 32]
            || self.device_lane_id == [0; 32]
            || self.scale > OFFLINE_CASH_ASSET_SCALE_MAX_V1
        {
            return Err(OfflineCashStateErrorV1::InvalidLane);
        }
        Ok(())
    }

    /// Return the exact normalized network identity bound by the recursive guard statement.
    #[must_use]
    pub fn normalized_network_id(&self) -> DigestV1 {
        *self.network_id.as_bytes()
    }

    /// Return the canonical normalized asset identity bound by the recursive guard statement.
    pub fn normalized_asset_id(&self) -> Result<DigestV1, OfflineCashStateErrorV1> {
        offline_cash_asset_identity_digest_v1(&self.asset)
            .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)
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
    fn validate(self) -> Result<(), OfflineCashStateErrorV1> {
        if self.generation == 0 || self.epoch_id == [0; 32] {
            return Err(OfflineCashStateErrorV1::InvalidHardwareEpoch);
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
    fn validate(self) -> Result<(), OfflineCashStateErrorV1> {
        if self.device_key_reference == [0; 32] || self.hardware_policy_id == [0; 32] {
            return Err(OfflineCashStateErrorV1::InvalidDevicePolicyBinding);
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
pub struct OfflineCashStateContextV1 {
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

impl OfflineCashStateContextV1 {
    fn validate(self) -> Result<(), OfflineCashStateErrorV1> {
        if self.protocol_version != OFFLINE_CASH_STATE_VERSION_V1
            || self.suite_id == [0; 32]
            || self.vk_digest == [0; 32]
            || self.release_id == [0; 32]
            || self.hardware_profile_id == [0; 32]
            || self.policy_epoch == 0
        {
            return Err(OfflineCashStateErrorV1::InvalidReleaseOrLiabilityPool);
        }
        self.asset_incarnation
            .validate()
            .map_err(|_| OfflineCashStateErrorV1::InvalidReleaseOrLiabilityPool)?;
        Ok(())
    }
}

/// Private aggregate balance state for one device lane and asset.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct OfflineCashStateV1 {
    /// State-machine version.
    pub version: u16,
    /// Exact offline-cash protocol version carried by the authenticated release.
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
    pub lane: OfflineCashLaneIdV1,
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
    pub consumed_credit_root: OfflineCashPastaStateCommitmentV1,
    /// Paired native Poseidon commitments to every preceding state field.
    pub state_commitment_components: OfflineCashPastaStateCommitmentV1,
    /// Compact SHA-256 name of `state_commitment_components` used by peer and settlement wires.
    pub state_commitment: DigestV1,
}

impl OfflineCashStateV1 {
    /// Return the private lifecycle context that every successor must carry unchanged unless a
    /// recursively authorized suite or hardware-profile transition explicitly replaces it.
    #[must_use]
    pub const fn context(&self) -> OfflineCashStateContextV1 {
        OfflineCashStateContextV1 {
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
        context: OfflineCashStateContextV1,
        liability_pool_id: DigestV1,
        lane: OfflineCashLaneIdV1,
        balance: u128,
        logical_sequence: u128,
        hardware_epoch: HardwareEpochV1,
        device_policy_binding: DevicePolicyBindingV1,
        state_nonce_commitment: DigestV1,
        consumed_credit_root: OfflineCashPastaStateCommitmentV1,
    ) -> Result<Self, OfflineCashStateErrorV1> {
        lane.validate()?;
        context.validate()?;
        if liability_pool_id != derive_liability_pool_id(&lane, context.asset_incarnation)? {
            return Err(OfflineCashStateErrorV1::InvalidReleaseOrLiabilityPool);
        }
        hardware_epoch.validate()?;
        device_policy_binding.validate()?;
        if state_nonce_commitment == [0; 32] {
            return Err(OfflineCashStateErrorV1::InvalidStateNonceCommitment);
        }
        let mut state = Self {
            version: OFFLINE_CASH_STATE_VERSION_V1,
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
            state_commitment_components: OfflineCashPastaStateCommitmentV1::ZERO,
            state_commitment: [0; 32],
        };
        let (components, commitment) = state.recompute_commitment()?;
        state.state_commitment_components = components;
        state.state_commitment = commitment;
        Ok(state)
    }

    /// Validate identity fields and the complete deterministic state commitment.
    pub fn validate(&self) -> Result<(), OfflineCashStateErrorV1> {
        if self.version != OFFLINE_CASH_STATE_VERSION_V1 {
            return Err(OfflineCashStateErrorV1::UnsupportedVersion(self.version));
        }
        self.lane.validate()?;
        self.asset_incarnation
            .validate()
            .map_err(|_| OfflineCashStateErrorV1::InvalidReleaseOrLiabilityPool)?;
        if self.protocol_version != OFFLINE_CASH_STATE_VERSION_V1
            || self.suite_id == [0; 32]
            || self.vk_digest == [0; 32]
            || self.release_id == [0; 32]
            || self.hardware_profile_id == [0; 32]
            || self.policy_epoch == 0
            || self.liability_pool_id
                != derive_liability_pool_id(&self.lane, self.asset_incarnation)?
        {
            return Err(OfflineCashStateErrorV1::InvalidReleaseOrLiabilityPool);
        }
        self.hardware_epoch.validate()?;
        self.device_policy_binding.validate()?;
        if self.state_nonce_commitment == [0; 32] {
            return Err(OfflineCashStateErrorV1::InvalidStateNonceCommitment);
        }
        let (components, commitment) = self.recompute_commitment()?;
        if self.state_commitment_components != components || self.state_commitment != commitment {
            return Err(OfflineCashStateErrorV1::StateCommitmentMismatch);
        }
        Ok(())
    }

    fn recompute_commitment(
        &self,
    ) -> Result<(OfflineCashPastaStateCommitmentV1, DigestV1), OfflineCashStateErrorV1> {
        let eq = self.recompute_parity_commitment::<halo2_proofs::halo2curves::pasta::Fp>(
            self.consumed_credit_root.eq,
        )?;
        let ep = self.recompute_parity_commitment::<halo2_proofs::halo2curves::pasta::Fq>(
            self.consumed_credit_root.ep,
        )?;
        let components = OfflineCashPastaStateCommitmentV1 {
            eq: encode_pasta(eq),
            ep: encode_pasta(ep),
        };
        Ok((
            components,
            offline_cash_pasta_state_commitment_v1(components),
        ))
    }

    fn recompute_parity_commitment<F>(
        &self,
        replay_root: DigestV1,
    ) -> Result<F, OfflineCashStateErrorV1>
    where
        F: OfflineCashPoseidonFieldV1,
    {
        let replay_root = decode_pasta::<F>(replay_root)
            .ok_or(OfflineCashStateErrorV1::StateCommitmentMismatch)?;
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
        Ok(pasta_hash(OFFLINE_CASH_STATE_DOMAIN_V1, &inputs))
    }
}

/// Closed set of aggregate balance transitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub enum OfflineCashTransitionKindV1 {
    /// Fold a finalized on-chain mint credit into the aggregate.
    MintFold,
    /// Split one receiver-bound peer credit from the aggregate.
    SendSplit,
    /// Fold one durably staged peer credit into the aggregate.
    ReceiveFold,
    /// Split one chain-facing redemption voucher from the aggregate.
    RedeemSplit,
    /// Recursively bridge the unchanged aggregate into a newly governed verifier suite.
    SuiteUpgrade,
    /// Move the unchanged aggregate to the exact next hardware epoch.
    Rotate,
}

/// Public recursive-proof statement derived by Core for one transition.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct TransitionProofStatementV1 {
    /// State-machine version.
    pub version: u16,
    /// Exact offline-cash protocol version.
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
    pub kind: OfflineCashTransitionKindV1,
    /// Exact monetary amount; zero only for suite or hardware-epoch rotation.
    pub amount: u128,
    /// Canonical finalized-mint statement digest, nonzero only for `MintFold`.
    pub mint_finality_semantic_digest: DigestV1,
    /// Exact paired mint-helper proof binding, nonzero only for `MintFold`.
    pub mint_finality_proof_binding_digest: DigestV1,
    /// Receiver-bound peer credit identifier, nonzero only for `SendSplit`.
    pub peer_credit_id: DigestV1,
    /// Receiver lane authorized by the peer credit, nonzero only for `SendSplit`.
    pub peer_recipient_lane_id: DigestV1,
    /// Complete released lifecycle binding used by terminal operations.
    pub lifecycle_binding_digest: DigestV1,
    /// Digest of the sealed, locally verified precommit candidate.
    pub precommit_binding_digest: DigestV1,
    /// Authenticated verifier-bridge authorization, nonzero only for `SuiteUpgrade`.
    pub suite_upgrade_authorization_digest: DigestV1,
    /// Authenticated proof release carried by the aggregate state.
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
    pub lane: OfflineCashLaneIdV1,
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
    pub fn digest(&self) -> Result<DigestV1, OfflineCashStateErrorV1> {
        transition_statement_digest(self)
    }
}

/// Exact statement that a hardware GuardBundle must authorize.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct HardwareTransitionStatementV1 {
    /// State-machine version.
    pub version: u16,
    /// Transition relation selected by the hardware guard.
    pub kind: OfflineCashTransitionKindV1,
    /// Exact monetary amount; zero only for suite or hardware-epoch rotation.
    pub amount: u128,
    /// Stable lane and asset scope.
    pub lane: OfflineCashLaneIdV1,
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
    fn validate_exact_next(&self) -> Result<(), OfflineCashStateErrorV1> {
        if self.version != OFFLINE_CASH_STATE_VERSION_V1 {
            return Err(OfflineCashStateErrorV1::UnsupportedVersion(self.version));
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
            return Err(OfflineCashStateErrorV1::HardwareCertificateMismatch);
        }
        let is_value_preserving = matches!(
            self.kind,
            OfflineCashTransitionKindV1::SuiteUpgrade | OfflineCashTransitionKindV1::Rotate
        );
        if is_value_preserving != (self.amount == 0) {
            return Err(OfflineCashStateErrorV1::HardwareCertificateMismatch);
        }
        match self.kind {
            OfflineCashTransitionKindV1::Rotate => {
                if self.successor_sequence != 0
                    || self.journal_revision_after != 0
                    || self.successor_epoch.generation
                        != self
                            .predecessor_epoch
                            .generation
                            .checked_add(1)
                            .ok_or(OfflineCashStateErrorV1::HardwareEpochOverflow)?
                    || self.successor_epoch.epoch_id == self.predecessor_epoch.epoch_id
                    || self.successor_device_policy_binding
                        == self.predecessor_device_policy_binding
                {
                    return Err(OfflineCashStateErrorV1::InvalidHardwareRotation);
                }
            }
            _ => {
                if self.successor_sequence
                    != self
                        .predecessor_sequence
                        .checked_add(1)
                        .ok_or(OfflineCashStateErrorV1::SequenceOverflow)?
                    || self.journal_revision_after
                        != self
                            .journal_revision_before
                            .checked_add(1)
                            .ok_or(OfflineCashStateErrorV1::JournalRevisionOverflow)?
                    || self.successor_epoch != self.predecessor_epoch
                    || self.successor_device_policy_binding
                        != self.predecessor_device_policy_binding
                {
                    return Err(OfflineCashStateErrorV1::HardwareCertificateMismatch);
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
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct TransitionAuthorizationV1 {
    /// Exact hardware transition certificate.
    pub hardware_certificate: HardwareTransitionCertificateV1,
    /// Complete fixed-profile paired-Pasta recursive proof.
    pub proof: OfflineCashPairedProofV1,
}

/// Deterministic transition material that a prover and hardware guard must authorize.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionPreviewV1 {
    /// Expected successor private state.
    pub successor: OfflineCashStateV1,
    /// Exact recursive-proof statement.
    pub proof_statement: TransitionProofStatementV1,
    /// Exact hardware transition statement.
    pub hardware_statement: HardwareTransitionStatementV1,
    /// Complete locally reconstructed normalized GuardBundle statement.
    pub normalized_guard_statement: OfflineCashNormalizedGuardStatementV1,
    /// Exact common transport digest required from both paired proof parities.
    pub transport_semantic_digest: DigestV1,
    /// Journal revision installed on success.
    pub journal_revision_after: u128,
}

/// Operation-specific bindings carried by the private transition proof.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TransitionAuxiliaryBindingsV1 {
    pub(crate) lifecycle_binding_digest: DigestV1,
    pub(crate) precommit_binding_digest: DigestV1,
    pub(crate) suite_upgrade_authorization_digest: DigestV1,
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
    pub predecessor_root: OfflineCashPastaStateCommitmentV1,
    /// Produced sparse-Merkle root, reconstructed with the fixed present leaf.
    pub successor_root: OfflineCashPastaStateCommitmentV1,
    /// Exact root-to-leaf sibling path selected by the credit ID bits.
    pub siblings_root_to_leaf:
        [OfflineCashPastaStateCommitmentV1; OFFLINE_CASH_CONSUMED_CREDIT_TREE_DEPTH_V1],
}

/// A credit-fold transition and its exact private replay-tree insert witness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreditFoldPreviewV1 {
    /// Deterministic public transition statements and expected successor.
    pub transition: TransitionPreviewV1,
    /// Exact nonmembership-and-insert witness consumed by the recursive circuit.
    pub replay_insert_witness: ConsumedCreditInsertWitnessV1,
}

/// Exact statement authorizing durable receipt of one peer credit.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct CreditStageStatementV1 {
    /// State-machine version.
    pub version: u16,
    /// Receiving lane and asset.
    pub recipient_lane: OfflineCashLaneIdV1,
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
    pub request: OfflineCashPaymentRequestV1,
    /// Exact public sender payment envelope.
    pub payment: OfflineCashPaymentV1,
    /// Digest used for duplicate/conflict classification.
    pub envelope_digest: DigestV1,
    /// Receiver hardware staging certificate.
    pub stage_certificate: CreditStageCertificateV1,
}

/// Exact signed acknowledgement retained for byte-identical duplicate delivery.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct DurableAcknowledgementV1 {
    /// Validated public acknowledgement.
    pub acknowledgement: OfflineCashAcknowledgementV1,
    /// Exact canonical Norito bytes returned to transport.
    pub canonical_bytes: Vec<u8>,
}

impl DurableAcknowledgementV1 {
    fn from_acknowledgement(
        acknowledgement: OfflineCashAcknowledgementV1,
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
    ) -> Result<Self, OfflineCashStateErrorV1> {
        acknowledgement
            .validate_shape_against(request, payment)
            .map_err(|_| OfflineCashStateErrorV1::InvalidAcknowledgement)?;
        let canonical_bytes = norito::encode_canonical(&acknowledgement)
            .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?;
        Ok(Self {
            acknowledgement,
            canonical_bytes,
        })
    }

    fn validate_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
    ) -> Result<(), OfflineCashStateErrorV1> {
        self.acknowledgement
            .validate_shape_against(request, payment)
            .map_err(|_| OfflineCashStateErrorV1::InvalidAcknowledgement)?;
        if self.canonical_bytes
            != norito::encode_canonical(&self.acknowledgement)
                .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?
        {
            return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
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
    pub request: OfflineCashPaymentRequestV1,
    /// Exact public payment accepted by the receiver.
    pub payment: OfflineCashPaymentV1,
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
    pub acknowledgement: OfflineCashAcknowledgementV1,
}

/// Durable staging outcome for an inbound public payment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StagePaymentOutcomeV1 {
    /// A new payment was durably staged at this journal revision.
    Staged {
        /// Durable journal revision produced by staging the payment.
        journal_revision: u128,
        /// Exact acknowledgement bytes safe to expose after staging.
        acknowledgement: DurableAcknowledgementV1,
    },
    /// Byte-identical transport retry of a still-pending payment.
    DuplicatePending {
        /// Existing durable journal revision at which the payment was staged.
        journal_revision: u128,
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
pub trait OfflineCashGuardBundleVerifierV1 {
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
pub struct RejectAllOfflineCashGuardBundleVerifierV1;

impl OfflineCashGuardBundleVerifierV1 for RejectAllOfflineCashGuardBundleVerifierV1 {
    fn verify_bootstrap(
        &self,
        _statement: &BootstrapStatementV1,
        _guard_bundle: &[u8],
    ) -> Result<(), String> {
        Err("Offline Cash V1 hardware bootstrap verifier is unavailable".to_owned())
    }

    fn verify_transition(
        &self,
        _statement: &HardwareTransitionStatementV1,
        _guard_bundle: &[u8],
    ) -> Result<(), String> {
        Err("Offline Cash V1 hardware transition verifier is unavailable".to_owned())
    }

    fn verify_credit_stage(
        &self,
        _statement: &CreditStageStatementV1,
        _guard_bundle: &[u8],
    ) -> Result<(), String> {
        Err("Offline Cash V1 hardware staging verifier is unavailable".to_owned())
    }

    fn verify_durability_anchor(
        &self,
        _statement: &DurabilityAnchorStatementV1,
        _guard_bundle: &[u8],
    ) -> Result<(), String> {
        Err("Offline Cash V1 hardware durability verifier is unavailable".to_owned())
    }
}

/// Exact device bootstrap statement.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct BootstrapStatementV1 {
    /// State-machine version.
    pub version: u16,
    /// Exact offline-cash protocol version.
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
    pub lane: OfflineCashLaneIdV1,
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
    pub fn proof_statement_digest(&self) -> Result<DigestV1, OfflineCashStateErrorV1> {
        canonical_sha256_digest(BOOTSTRAP_STATEMENT_DOMAIN, self)
    }
}

/// Complete locally derived bootstrap instance awaiting recursive and hardware authorization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootstrapPreviewV1 {
    /// Unique private zero-balance successor state.
    pub state: OfflineCashStateV1,
    /// Core bootstrap semantic statement.
    pub statement: BootstrapStatementV1,
    /// Complete normalized GuardBundle statement with canonical null predecessor and 0/0 base.
    pub normalized_guard_statement: OfflineCashNormalizedGuardStatementV1,
    /// Exact common transport digest required from both paired proof parities.
    pub transport_semantic_digest: DigestV1,
}

/// Proof and hardware authorization for a new zero-balance lane.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct BootstrapAuthorizationV1 {
    /// Complete fixed-profile paired-Pasta bootstrap proof.
    pub proof: OfflineCashPairedProofV1,
    /// Platform hardware registration GuardBundle bytes.
    pub guard_bundle: Vec<u8>,
}

/// One hardware-sealed recovery statement.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct DurabilityAnchorStatementV1 {
    /// State-machine version.
    pub version: u16,
    /// Stable lane and asset scope.
    pub lane: OfflineCashLaneIdV1,
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
pub struct OfflineCashStateSnapshotV1 {
    /// State-machine version.
    pub version: u16,
    /// Current private aggregate state.
    pub state: OfflineCashStateV1,
    /// Current durable journal revision.
    pub journal_revision: u128,
    /// Pending credits in strict credit-id order.
    pub pending_credits: Vec<StagedCreditV1>,
    /// Historical recipient key/policy bindings accepted by this stable lane, in strict order.
    ///
    /// Retaining prior bindings ensures a payment committed against an in-window request remains
    /// stageable after an offline hardware-epoch rotation.
    pub accepted_recipient_bindings: Vec<DevicePolicyBindingV1>,
    /// Durable receiver acknowledgements retained in strict credit-id order for idempotent retry.
    pub accepted_payment_receipts: Vec<AcceptedPaymentReceiptV1>,
    /// Consumed credits in strict credit-id order.
    pub consumed_credits: Vec<ConsumedCreditRecordV1>,
    /// Durable receiver ticket decisions, live reservations, and permanent tombstones.
    pub acceptance_ticket_book: OfflineCashAcceptanceTicketBookV1,
    /// Durable sender capacity reservations and terminal-envelope bindings.
    pub sender_outbox_capacity: OfflineCashSenderOutboxCapacityV1,
    /// Exact recoverable sender prepare/prove/commit/finalize journal.
    pub outgoing_candidate_journal: OfflineCashOutgoingCandidateJournalV1,
    /// Poseidon commitment to every preceding snapshot field.
    pub snapshot_commitment: DigestV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode)]
struct SnapshotCommitmentPreimageV1 {
    version: u16,
    state: OfflineCashStateV1,
    journal_revision: u128,
    pending_credits: Vec<StagedCreditV1>,
    accepted_recipient_bindings: Vec<DevicePolicyBindingV1>,
    accepted_payment_receipts: Vec<AcceptedPaymentReceiptV1>,
    consumed_credits: Vec<ConsumedCreditRecordV1>,
    acceptance_ticket_book: OfflineCashAcceptanceTicketBookV1,
    sender_outbox_capacity: OfflineCashSenderOutboxCapacityV1,
    outgoing_candidate_journal: OfflineCashOutgoingCandidateJournalV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode)]
struct ReceiverSnapshotCapacityProjectionV1 {
    pending_credits: Vec<StagedCreditV1>,
    accepted_payment_receipts: Vec<AcceptedPaymentReceiptV1>,
    consumed_credits: Vec<ConsumedCreditRecordV1>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ReceiverSnapshotCapacityUsageV1 {
    live_bytes: u64,
    retained_bytes: u64,
}

fn receiver_snapshot_capacity_usage_v1(
    pending_credits: &BTreeMap<CreditIdV1, StagedCreditV1>,
    accepted_payment_receipts: &BTreeMap<CreditIdV1, AcceptedPaymentReceiptV1>,
    consumed_credits: &ExactConsumedCreditIndex,
) -> Result<ReceiverSnapshotCapacityUsageV1, OfflineCashStateErrorV1> {
    let pending = pending_credits.values().cloned().collect::<Vec<_>>();
    let receipts = accepted_payment_receipts
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let consumed = consumed_credits.records();
    let accepted_ids = accepted_payment_receipts
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    let non_peer_consumed = consumed
        .iter()
        .filter(|record| !accepted_ids.contains(&record.credit_id))
        .copied()
        .collect::<Vec<_>>();
    let terminal_receipts = accepted_payment_receipts
        .iter()
        .filter(|(credit_id, _)| !pending_credits.contains_key(credit_id))
        .map(|(_, receipt)| receipt.clone())
        .collect::<Vec<_>>();

    let encoded_len = |projection: &ReceiverSnapshotCapacityProjectionV1| {
        norito::encode_canonical(projection)
            .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)
            .and_then(|bytes| {
                u64::try_from(bytes.len()).map_err(|_| OfflineCashStateErrorV1::ArithmeticOverflow)
            })
    };
    let baseline_bytes = encoded_len(&ReceiverSnapshotCapacityProjectionV1 {
        pending_credits: Vec::new(),
        accepted_payment_receipts: Vec::new(),
        consumed_credits: non_peer_consumed,
    })?;
    let terminal_bytes = encoded_len(&ReceiverSnapshotCapacityProjectionV1 {
        pending_credits: Vec::new(),
        accepted_payment_receipts: terminal_receipts,
        consumed_credits: consumed.clone(),
    })?
    .checked_sub(baseline_bytes)
    .ok_or(OfflineCashStateErrorV1::StateInvariant)?;
    let full_bytes = encoded_len(&ReceiverSnapshotCapacityProjectionV1 {
        pending_credits: pending,
        accepted_payment_receipts: receipts,
        consumed_credits: consumed,
    })?
    .checked_sub(baseline_bytes)
    .ok_or(OfflineCashStateErrorV1::StateInvariant)?;
    let live_bytes = full_bytes
        .checked_sub(terminal_bytes)
        .ok_or(OfflineCashStateErrorV1::StateInvariant)?;
    Ok(ReceiverSnapshotCapacityUsageV1 {
        live_bytes,
        retained_bytes: terminal_bytes,
    })
}

/// State-machine validation or authorization failure.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum OfflineCashStateErrorV1 {
    /// A value carried an unsupported wire/state version.
    #[error("unsupported Offline Cash state version {0}")]
    UnsupportedVersion(u16),
    /// A network, device-lane, or asset identity was zero.
    #[error("invalid Offline Cash lane identity")]
    InvalidLane,
    /// The authenticated release or deterministic network-and-asset liability pool was invalid.
    #[error("invalid Offline Cash proof release or liability pool")]
    InvalidReleaseOrLiabilityPool,
    /// A hardware epoch was zero or malformed.
    #[error("invalid Offline Cash hardware epoch")]
    InvalidHardwareEpoch,
    /// A device-key reference or governed hardware-policy identity was zero.
    #[error("invalid Offline Cash device and hardware-policy binding")]
    InvalidDevicePolicyBinding,
    /// A private state nonce commitment was zero or reused by an immediate successor.
    #[error("invalid Offline Cash private state nonce commitment")]
    InvalidStateNonceCommitment,
    /// A requested rotation was not the exact next distinct epoch.
    #[error("Offline Cash hardware rotation is not exact-next")]
    InvalidHardwareRotation,
    /// Hardware epoch generation overflowed `u128`.
    #[error("Offline Cash hardware epoch overflow")]
    HardwareEpochOverflow,
    /// A private state commitment did not match its fields.
    #[error("Offline Cash state commitment mismatch")]
    StateCommitmentMismatch,
    /// A private state field relation was impossible for a valid history.
    #[error("Offline Cash private state invariant failed")]
    StateInvariant,
    /// Canonical Norito encoding failed.
    #[error("Offline Cash canonical encoding failed")]
    CanonicalEncoding,
    /// A proof was empty or exceeded its fixed bound.
    #[error("Offline Cash proof bundle is invalid")]
    InvalidProofBundle,
    /// A GuardBundle was empty or exceeded its fixed bound.
    #[error("Offline Cash GuardBundle is invalid")]
    InvalidGuardBundle,
    /// The configured governed proof verifier rejected.
    #[error("Offline Cash proof verifier rejected: {0}")]
    ProofRejected(String),
    /// The configured hardware guard verifier rejected.
    #[error("Offline Cash hardware guard rejected: {0}")]
    GuardRejected(String),
    /// The supplied certificate did not equal Core's exact transition.
    #[error("Offline Cash hardware certificate mismatch")]
    HardwareCertificateMismatch,
    /// Logical transition sequence overflowed `u128`.
    #[error("Offline Cash logical sequence overflow")]
    SequenceOverflow,
    /// Durable journal revision overflowed `u128`.
    #[error("Offline Cash durable journal revision overflow")]
    JournalRevisionOverflow,
    /// Balance or consumed-credit arithmetic overflowed `u128`.
    #[error("Offline Cash checked arithmetic overflow")]
    ArithmeticOverflow,
    /// A split attempted to consume more than the current aggregate balance.
    #[error("Offline Cash balance is insufficient")]
    InsufficientBalance,
    /// A receiver request was zero, expired at issue, or otherwise malformed.
    #[error("invalid Offline Cash payment request")]
    InvalidPaymentRequest,
    /// An authenticated release did not enable the required qualified hardware profile.
    #[error("Offline Cash hardware credential is not enabled by the authenticated release")]
    InvalidHardwareProfile,
    /// An acceptance ticket was malformed, mismatched, or reused for another payment.
    #[error("invalid or conflicting Offline Cash acceptance ticket")]
    InvalidAcceptanceTicket,
    /// A sender intent was not authorized by release-pinned qualified hardware proof.
    #[error("invalid Offline Cash acceptance-intent authorization")]
    InvalidAcceptanceIntentAuthorization,
    /// Receiver inbox capacity cannot back another acceptance ticket.
    #[error("Offline Cash receiver inbox capacity is exhausted")]
    ReceiverCapacityExhausted,
    /// Durable storage cannot hold one complete receive and terminal operation.
    #[error("Offline Cash durable capacity is below the minimum complete-operation footprint")]
    InvalidDurableCapacity,
    /// Sender durable outbox capacity cannot back another terminal operation.
    #[error("Offline Cash sender outbox capacity is exhausted")]
    SenderOutboxCapacityExhausted,
    /// A transition attempted to skip or conflict with the durable outgoing-candidate lifecycle.
    #[error("invalid Offline Cash outgoing candidate lifecycle stage")]
    InvalidCandidateStage,
    /// Retried candidate material differs from the already persisted bytes.
    #[error("conflicting Offline Cash outgoing candidate retry")]
    CandidateConflict,
    /// Hardware-sealed transition or recovery material was empty or oversized.
    #[error("invalid Offline Cash sealed recovery material")]
    InvalidRecoveryMaterial,
    /// Sender commit time was outside the receiver-authorized interval.
    #[error("Offline Cash sender commit time is outside the request window")]
    SenderCommitOutsideRequestWindow,
    /// Qualified hardware supplied a zero trusted transition-commit time.
    #[error("invalid Offline Cash trusted hardware commit time")]
    InvalidTrustedCommitTime,
    /// A mint credit was malformed or targeted another lane.
    #[error("invalid Offline Cash mint credit")]
    InvalidMintCredit,
    /// A verified mint-finality capability did not authorize this exact mint statement.
    #[error("Offline Cash mint-finality verification does not match the folded credit")]
    MintFinalityMismatch,
    /// A peer credit was malformed or targeted another lane.
    #[error("invalid Offline Cash peer credit")]
    InvalidPeerCredit,
    /// A first delivery omitted its hardware staging authorization and signed acknowledgement.
    #[error("Offline Cash first payment delivery requires staging authorization")]
    MissingStageAuthorization,
    /// A receiver acknowledgement failed request, payment, receipt, sequence, or signature binding.
    #[error("invalid Offline Cash payment acknowledgement")]
    InvalidAcknowledgement,
    /// A redemption request was malformed.
    #[error("invalid Offline Cash redemption request")]
    InvalidRedemption,
    /// A credit identity already committed the same envelope.
    #[error("Offline Cash credit {0:?} was already consumed")]
    CreditAlreadyConsumed(CreditIdV1),
    /// A credit identity was reused with different canonical bytes.
    #[error("Offline Cash credit {0:?} conflicts with retained bytes")]
    CreditConflict(CreditIdV1),
    /// A consumed-credit insert witness failed its exact key, leaf, path, or root relation.
    #[error("Offline Cash consumed-credit insert witness is invalid")]
    InvalidConsumedCreditInsertWitness,
    /// No pending credit exists for a requested receive fold.
    #[error("Offline Cash credit {0:?} is not staged")]
    CreditNotStaged(CreditIdV1),
    /// Snapshot fields, ordering, roots, counts, or commitment were inconsistent.
    #[error("Offline Cash recovery snapshot failed integrity validation")]
    SnapshotIntegrity,
    /// A snapshot did not match the latest hardware-sealed anchor.
    #[error("Offline Cash recovery snapshot is stale or from another lane")]
    SnapshotRollback,
}

/// Aggregate Offline Cash V1 state machine with governed recursion and hardware verifiers.
pub struct OfflineCashStateMachineV1<R, G> {
    state: OfflineCashStateV1,
    journal_revision: u128,
    pending_credits: BTreeMap<CreditIdV1, StagedCreditV1>,
    accepted_recipient_bindings: BTreeSet<DevicePolicyBindingV1>,
    accepted_payment_receipts: BTreeMap<CreditIdV1, AcceptedPaymentReceiptV1>,
    consumed_credits: ExactConsumedCreditIndex,
    acceptance_ticket_book: OfflineCashAcceptanceTicketBookV1,
    sender_outbox_capacity: OfflineCashSenderOutboxCapacityV1,
    outgoing_candidate_journal: OfflineCashOutgoingCandidateJournalV1,
    proof_release: OfflineCashStateProofReleaseV1,
    recursive_verifier: R,
    guard_verifier: G,
}

impl<R, G> OfflineCashStateMachineV1<R, G>
where
    R: OfflineCashRecursiveVerifierV1,
    G: OfflineCashGuardBundleVerifierV1,
{
    /// Preview the unique zero-balance bootstrap state and exact authorization statement.
    pub fn preview_bootstrap(
        proof_release: OfflineCashStateProofReleaseV1,
        state_context: OfflineCashStateContextV1,
        lane: OfflineCashLaneIdV1,
        hardware_epoch: HardwareEpochV1,
        device_policy_binding: DevicePolicyBindingV1,
        state_nonce_commitment: DigestV1,
        trusted_commit_time_ms: u64,
    ) -> Result<BootstrapPreviewV1, OfflineCashStateErrorV1> {
        lane.validate()?;
        state_context.validate()?;
        if state_context.release_id != proof_release.release_id() {
            return Err(OfflineCashStateErrorV1::InvalidReleaseOrLiabilityPool);
        }
        hardware_epoch.validate()?;
        device_policy_binding.validate()?;
        if state_nonce_commitment == [0; 32] {
            return Err(OfflineCashStateErrorV1::InvalidStateNonceCommitment);
        }
        if trusted_commit_time_ms == 0 {
            return Err(OfflineCashStateErrorV1::InvalidTrustedCommitTime);
        }
        let consumed_credits = ExactConsumedCreditIndex::empty();
        let liability_pool_id = derive_liability_pool_id(&lane, state_context.asset_incarnation)?;
        let state = OfflineCashStateV1::build(
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
            version: OFFLINE_CASH_STATE_VERSION_V1,
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
            OfflineCashNormalizedGuardStatementV1::from_bootstrap_state(&statement, context)
                .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?;
        let guard_digest = normalized_guard_statement
            .canonical_digest()
            .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?;
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
        proof_release: OfflineCashStateProofReleaseV1,
        state_context: OfflineCashStateContextV1,
        lane: OfflineCashLaneIdV1,
        hardware_epoch: HardwareEpochV1,
        device_policy_binding: DevicePolicyBindingV1,
        state_nonce_commitment: DigestV1,
        trusted_commit_time_ms: u64,
        durable_capacity: OfflineCashDurableCapacityV1,
        authorization: BootstrapAuthorizationV1,
        recursive_verifier: R,
        guard_verifier: G,
    ) -> Result<Self, OfflineCashStateErrorV1> {
        durable_capacity.validate()?;
        let preview = Self::preview_bootstrap(
            proof_release,
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
        verify_offline_cash_state_proof_v1(
            &recursive_verifier,
            proof_release.artifacts,
            &public_inputs,
            &authorization.proof,
        )
        .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?;
        guard_verifier
            .verify_bootstrap(&preview.statement, &authorization.guard_bundle)
            .map_err(OfflineCashStateErrorV1::GuardRejected)?;
        Ok(Self {
            state: preview.state,
            journal_revision: 0,
            pending_credits: BTreeMap::new(),
            accepted_recipient_bindings: BTreeSet::from([device_policy_binding]),
            accepted_payment_receipts: BTreeMap::new(),
            consumed_credits: ExactConsumedCreditIndex::empty(),
            acceptance_ticket_book: OfflineCashAcceptanceTicketBookV1::new(
                durable_capacity.inbox_bytes,
            ),
            sender_outbox_capacity: OfflineCashSenderOutboxCapacityV1::new(
                durable_capacity.outbox_bytes,
            ),
            outgoing_candidate_journal: OfflineCashOutgoingCandidateJournalV1::default(),
            proof_release,
            recursive_verifier,
            guard_verifier,
        })
    }

    /// Borrow the current aggregate state.
    #[must_use]
    pub fn state(&self) -> &OfflineCashStateV1 {
        &self.state
    }

    /// Return the current durable journal revision.
    #[must_use]
    pub fn journal_revision(&self) -> u128 {
        self.journal_revision
    }

    /// Borrow the receiver ticket/capacity ledger included in every durability snapshot.
    #[must_use]
    pub const fn acceptance_ticket_book(&self) -> &OfflineCashAcceptanceTicketBookV1 {
        &self.acceptance_ticket_book
    }

    /// Borrow the sender outbox-capacity ledger included in every durability snapshot.
    #[must_use]
    pub const fn sender_outbox_capacity(&self) -> &OfflineCashSenderOutboxCapacityV1 {
        &self.sender_outbox_capacity
    }

    /// Borrow the exact recoverable outgoing journal.
    #[must_use]
    pub const fn outgoing_candidate_journal(&self) -> &OfflineCashOutgoingCandidateJournalV1 {
        &self.outgoing_candidate_journal
    }

    /// Atomically install one proof-authorized receiver ticket decision in this lane snapshot.
    pub fn reserve_acceptance_ticket(
        &mut self,
        verified_authorization: VerifiedOfflineCashAcceptanceIntentAuthorizationV1,
        ticket: iroha_data_model::offline::OfflineCashAcceptanceTicketV1,
    ) -> Result<AcceptanceTicketReservationOutcomeV1, OfflineCashStateErrorV1> {
        self.acceptance_ticket_book
            .reserve(verified_authorization, ticket)
    }

    /// Begin authenticated no-commit recovery without releasing its live capacity.
    pub fn begin_acceptance_ticket_no_commit_recovery(
        &mut self,
        verified: &super::offline_cash_v1_recursion::VerifiedOfflineCashNoCommitClosureV1,
    ) -> Result<AcceptanceTicketNoCommitRecoveryOutcomeV1, OfflineCashStateErrorV1> {
        self.acceptance_ticket_book
            .begin_authenticated_no_commit_recovery(verified)
    }

    /// Consume an authenticated no-commit capability and permanently close its ticket.
    pub fn close_acceptance_ticket_no_commit_recovery(
        &mut self,
        verified: super::offline_cash_v1_recursion::VerifiedOfflineCashNoCommitClosureV1,
    ) -> Result<AcceptanceTicketNoCommitClosureOutcomeV1, OfflineCashStateErrorV1> {
        self.acceptance_ticket_book
            .close_authenticated_no_commit_recovery(verified)
    }

    /// Atomically reserve sender bytes and persist the exact prepared operation.
    ///
    /// Hardware may lock the predecessor only after this succeeds. Failure leaves both the
    /// capacity ledger and outgoing journal unchanged.
    pub fn prepare_outgoing_candidate(
        &mut self,
        prepared: PreparedOutgoingCandidateV1,
    ) -> Result<
        (
            SenderOutboxReservationOutcomeV1,
            OfflineCashOutgoingCommitCapabilityV1,
        ),
        OfflineCashStateErrorV1,
    > {
        if prepared.private_state_link().0 != &self.state
            || prepared.proof_statement.journal_revision_before != self.journal_revision
        {
            return Err(OfflineCashStateErrorV1::InvalidCandidateStage);
        }
        prepared.validate_recovered()?;
        let mut next_outbox = self.sender_outbox_capacity.clone();
        let mut next_journal = self.outgoing_candidate_journal.clone();
        let capability = OfflineCashOutgoingCommitCapabilityV1::for_prepared(&prepared)?;
        next_journal.prepare(prepared.clone())?;
        let outcome = next_outbox.reserve(prepared.outbox_reservation, &next_journal)?;
        self.sender_outbox_capacity = next_outbox;
        self.outgoing_candidate_journal = next_journal;
        Ok((outcome, capability))
    }

    /// Reissue hardware-commit authority from an already validated prepared/candidate snapshot.
    pub fn recover_outgoing_commit_capability(
        &self,
    ) -> Result<OfflineCashOutgoingCommitCapabilityV1, OfflineCashStateErrorV1> {
        let prepared = match self.outgoing_candidate_journal.stage() {
            OfflineCashOutgoingJournalStageV1::Prepared(prepared) => prepared,
            OfflineCashOutgoingJournalStageV1::Candidate(candidate) => &candidate.prepared,
            _ => return Err(OfflineCashStateErrorV1::InvalidCandidateStage),
        };
        self.sender_outbox_capacity
            .require_reservation(prepared.outbox_reservation)?;
        OfflineCashOutgoingCommitCapabilityV1::for_prepared(prepared)
    }

    /// Verify and atomically persist the candidate proof for the current prepared operation.
    pub fn persist_outgoing_candidate(
        &mut self,
        capability: &OfflineCashOutgoingCommitCapabilityV1,
        candidate_proof: OfflineCashPairedProofV1,
    ) -> Result<PersistedOutgoingCandidateV1, OfflineCashStateErrorV1>
    where
        R: OfflineCashCandidateProofVerifierV1,
    {
        let prepared = match self.outgoing_candidate_journal.stage() {
            OfflineCashOutgoingJournalStageV1::Prepared(prepared) => prepared.clone(),
            _ => return Err(OfflineCashStateErrorV1::InvalidCandidateStage),
        };
        capability.authorizes(&prepared)?;
        let candidate = PersistedOutgoingCandidateV1::verify_and_persist(
            prepared,
            candidate_proof,
            &self.recursive_verifier,
        )?;
        let mut next_journal = self.outgoing_candidate_journal.clone();
        next_journal.persist_candidate(candidate.clone())?;
        self.outgoing_candidate_journal = next_journal;
        Ok(candidate)
    }

    /// Atomically install a hardware-committed successor and advance its journal stage.
    ///
    /// A committed predecessor can therefore never coexist with its old monetary head in a
    /// canonical snapshot.
    pub fn commit_outgoing_candidate(
        &mut self,
        capability: OfflineCashOutgoingCommitCapabilityV1,
        commit_certificate: iroha_data_model::offline::OfflineCashCommitCertificateV1,
    ) -> Result<CommittedOutgoingCandidateV1, OfflineCashStateErrorV1> {
        let candidate = match self.outgoing_candidate_journal.stage() {
            OfflineCashOutgoingJournalStageV1::Candidate(candidate) => candidate.clone(),
            _ => return Err(OfflineCashStateErrorV1::InvalidCandidateStage),
        };
        capability.authorizes(&candidate.prepared)?;
        let committed =
            CommittedOutgoingCandidateV1::from_hardware_commit(candidate, commit_certificate)?;
        let prepared = &committed.candidate.prepared;
        if prepared.private_state_link().0 != &self.state
            || prepared.proof_statement.journal_revision_before != self.journal_revision
        {
            return Err(OfflineCashStateErrorV1::InvalidCandidateStage);
        }
        self.sender_outbox_capacity
            .require_reservation(prepared.outbox_reservation)?;
        let mut next_journal = self.outgoing_candidate_journal.clone();
        next_journal.commit(committed.clone())?;
        self.state = prepared.private_state_link().1.clone();
        self.journal_revision = prepared.proof_statement.journal_revision_after;
        self.outgoing_candidate_journal = next_journal;
        Ok(committed)
    }

    /// Verify and atomically install the canonical terminal retry envelope.
    pub fn finalize_outgoing_candidate(
        &mut self,
        wrapper_proof: iroha_data_model::offline::OfflineCashCommitWrapperProofV1,
        retry_metadata: Vec<u8>,
    ) -> Result<DurableOutgoingEnvelopeV1, OfflineCashStateErrorV1>
    where
        R: OfflineCashCommitWrapperVerifierV1,
    {
        let committed = match self.outgoing_candidate_journal.stage() {
            OfflineCashOutgoingJournalStageV1::Committed(committed) => committed.clone(),
            _ => return Err(OfflineCashStateErrorV1::InvalidCandidateStage),
        };
        if committed.candidate.prepared.private_state_link().1 != &self.state
            || committed
                .candidate
                .prepared
                .proof_statement
                .journal_revision_after
                != self.journal_revision
        {
            return Err(OfflineCashStateErrorV1::InvalidCandidateStage);
        }
        let finalized = DurableOutgoingEnvelopeV1::finalize(
            committed,
            wrapper_proof,
            self.proof_release.artifacts.artifact_manifest_digest,
            retry_metadata,
            &self.recursive_verifier,
        )?;
        let mut next_outbox = self.sender_outbox_capacity.clone();
        let mut next_journal = self.outgoing_candidate_journal.clone();
        next_journal.install_finalized(finalized.clone(), &mut next_outbox)?;
        self.sender_outbox_capacity = next_outbox;
        self.outgoing_candidate_journal = next_journal;
        Ok(finalized)
    }

    /// Return byte-identical terminal bytes only after durable final installation.
    pub fn expose_outgoing_candidate(
        &self,
        reservation_id: DigestV1,
    ) -> Result<&[u8], OfflineCashStateErrorV1> {
        self.outgoing_candidate_journal.expose(reservation_id)
    }

    /// Atomically retire a retry envelope and release its sender capacity.
    pub fn release_outgoing_candidate(
        &mut self,
        reservation_id: DigestV1,
        expected_envelope_digest: DigestV1,
    ) -> Result<(), OfflineCashStateErrorV1> {
        let mut next_outbox = self.sender_outbox_capacity.clone();
        let mut next_journal = self.outgoing_candidate_journal.clone();
        next_journal.release_finalized(
            reservation_id,
            expected_envelope_digest,
            &mut next_outbox,
        )?;
        self.sender_outbox_capacity = next_outbox;
        self.outgoing_candidate_journal = next_journal;
        Ok(())
    }

    /// Return the number of durably staged credits awaiting a fold.
    #[must_use]
    pub fn pending_credit_count(&self) -> usize {
        self.pending_credits.len()
    }

    /// Select the deterministic prefix of staged credits needed to cover `amount`.
    ///
    /// Wallet orchestration calls this before a send or redemption, then drains the returned
    /// credits through repeated [`Self::receive_fold`] transitions.
    /// The selection is ordered by credit ID and has no protocol count ceiling: a larger backlog
    /// changes only local work and latency. An empty result means the current aggregate balance
    /// already covers the amount.
    pub fn pending_credits_required_for_amount(
        &self,
        amount: u128,
    ) -> Result<Vec<CreditIdV1>, OfflineCashStateErrorV1> {
        required_pending_credit_prefix(
            self.state.balance,
            amount,
            self.pending_credits
                .iter()
                .map(|(credit_id, staged)| (*credit_id, staged.payment.statement.amount)),
        )
    }

    /// Preview folding one finalized mint credit into the aggregate.
    pub fn preview_mint_fold(
        &self,
        credit: &OfflineCashMintCreditV1,
        successor_state_nonce_commitment: DigestV1,
        trusted_commit_time_ms: u64,
    ) -> Result<CreditFoldPreviewV1, OfflineCashStateErrorV1> {
        credit
            .validate_shape()
            .map_err(|_| OfflineCashStateErrorV1::InvalidMintCredit)?;
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
            return Err(OfflineCashStateErrorV1::InvalidMintCredit);
        }
        let credit_id = CreditIdV1(lifecycle.credit_id);
        let envelope_digest = canonical_sha256_digest(MINT_CREDIT_DOMAIN, credit)?;
        self.ensure_credit_id_available(credit_id, envelope_digest)?;
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
            .ok_or(OfflineCashStateErrorV1::ArithmeticOverflow)?;
        let successor = self.next_state(
            balance,
            self.state.hardware_epoch,
            self.state.device_policy_binding,
            successor_state_nonce_commitment,
            replay_insert_witness.successor_root,
        )?;
        let mint_finality_semantic_digest = mint
            .canonical_digest()
            .map_err(|_| OfflineCashStateErrorV1::InvalidMintCredit)?;
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
            OfflineCashTransitionKindV1::MintFold,
            successor.clone(),
            effect_digest,
            mint_finality_semantic_digest,
            credit.finality_proof_binding_digest,
            [0; 32],
            [0; 32],
            TransitionAuxiliaryBindingsV1::default(),
            trusted_commit_time_ms,
            |normalized_guard_statement_digest| {
                local_transition_transport_digest(
                    OfflineCashTransitionKindV1::MintFold,
                    self.state.release_id,
                    self.state.liability_pool_id,
                    effect_digest,
                    self.state.state_commitment,
                    successor.state_commitment,
                    normalized_guard_statement_digest,
                )
            },
        )?;
        Ok(CreditFoldPreviewV1 {
            transition,
            replay_insert_witness,
        })
    }

    /// Verify and atomically apply one finalized mint credit.
    pub fn mint_fold(
        &mut self,
        credit: OfflineCashMintCreditV1,
        successor_state_nonce_commitment: DigestV1,
        trusted_commit_time_ms: u64,
        mint_finality: VerifiedOfflineCashMintFinalityHelperV1,
        authorization: TransitionAuthorizationV1,
    ) -> Result<OfflineCashStateV1, OfflineCashStateErrorV1> {
        let preview = self.preview_mint_fold(
            &credit,
            successor_state_nonce_commitment,
            trusted_commit_time_ms,
        )?;
        let mint_statement_digest = credit
            .statement
            .canonical_digest()
            .map_err(|_| OfflineCashStateErrorV1::InvalidMintCredit)?;
        if mint_finality.semantic_digest() != mint_statement_digest {
            return Err(OfflineCashStateErrorV1::MintFinalityMismatch);
        }
        if mint_finality.proof_binding_digest() != credit.finality_proof_binding_digest {
            return Err(OfflineCashStateErrorV1::MintFinalityMismatch);
        }
        self.verify_transition_authorization(&preview.transition, &authorization)?;
        let credit_id = CreditIdV1(credit.statement.lifecycle.credit_id);
        let envelope_digest = canonical_sha256_digest(MINT_CREDIT_DOMAIN, &credit)?;
        self.consumed_credits.insert_with_witness(
            credit_id,
            envelope_digest,
            &preview.replay_insert_witness,
        )?;
        self.commit_preview(preview.transition);
        Ok(self.state.clone())
    }

    /// Preview the exact receiver journal statement for a new public payment.
    pub fn preview_stage_payment(
        &self,
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
        staged_at_ms: u64,
    ) -> Result<CreditStageStatementV1, OfflineCashStateErrorV1> {
        if !matches!(
            self.outgoing_candidate_journal.stage(),
            OfflineCashOutgoingJournalStageV1::Empty
        ) {
            return Err(OfflineCashStateErrorV1::InvalidCandidateStage);
        }
        self.validate_peer_payment(request, payment)?;
        let credit_id = CreditIdV1(payment.statement.lifecycle.credit_id);
        let envelope_digest = canonical_sha256_digest(CREDIT_ENVELOPE_DOMAIN, payment)?;
        self.ensure_credit_id_available(credit_id, envelope_digest)?;
        let journal_revision_after = self
            .journal_revision
            .checked_add(1)
            .ok_or(OfflineCashStateErrorV1::JournalRevisionOverflow)?;
        Ok(CreditStageStatementV1 {
            version: OFFLINE_CASH_STATE_VERSION_V1,
            recipient_lane: self.state.lane.clone(),
            receiver_state_commitment: self.state.state_commitment,
            receiver_hardware_epoch: self.state.hardware_epoch,
            receiver_device_policy_binding: self.state.device_policy_binding,
            receiver_state_nonce_commitment: self.state.state_nonce_commitment,
            credit_id,
            envelope_digest,
            staged_at_ms,
            journal_revision_before: self.journal_revision,
            journal_revision_after,
        })
    }

    /// Durably stage or idempotently classify one inbound credit.
    ///
    /// Request expiry is checked only against the sender's trusted commit time inside the credit.
    /// `staged_at_ms` may be arbitrarily later without invalidating committed value.
    pub fn stage_payment(
        &mut self,
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
        staged_at_ms: u64,
        authorization: Option<PaymentStageAuthorizationV1>,
    ) -> Result<StagePaymentOutcomeV1, OfflineCashStateErrorV1> {
        // Bound all variable-length material before canonical hashing so conflict classification
        // cannot be used as an oversized-envelope allocation path.
        self.validate_peer_payment(&request, &payment)?;
        let credit_id = CreditIdV1(payment.statement.lifecycle.credit_id);
        let envelope_digest = canonical_sha256_digest(CREDIT_ENVELOPE_DOMAIN, &payment)?;
        if let Some(existing) = self.pending_credits.get(&credit_id) {
            return if existing.envelope_digest == envelope_digest {
                let acknowledgement = self
                    .accepted_payment_receipts
                    .get(&credit_id)
                    .ok_or(OfflineCashStateErrorV1::SnapshotIntegrity)?
                    .durable_acknowledgement
                    .clone();
                Ok(StagePaymentOutcomeV1::DuplicatePending {
                    journal_revision: existing.stage_certificate.statement.journal_revision_after,
                    acknowledgement,
                })
            } else {
                Err(OfflineCashStateErrorV1::CreditConflict(credit_id))
            };
        }
        if let Some(existing) = self.consumed_credits.get(credit_id) {
            return if existing == envelope_digest {
                let acknowledgement = self
                    .accepted_payment_receipts
                    .get(&credit_id)
                    .ok_or(OfflineCashStateErrorV1::SnapshotIntegrity)?
                    .durable_acknowledgement
                    .clone();
                Ok(StagePaymentOutcomeV1::DuplicateConsumed { acknowledgement })
            } else {
                Err(OfflineCashStateErrorV1::CreditConflict(credit_id))
            };
        }
        let authorization =
            authorization.ok_or(OfflineCashStateErrorV1::MissingStageAuthorization)?;
        let expected = self.preview_stage_payment(&request, &payment, staged_at_ms)?;
        let stage_certificate = authorization.stage_certificate;
        validate_guard_bytes(&stage_certificate.guard_bundle)?;
        if stage_certificate.statement != expected {
            return Err(OfflineCashStateErrorV1::HardwareCertificateMismatch);
        }
        self.guard_verifier
            .verify_credit_stage(&expected, &stage_certificate.guard_bundle)
            .map_err(OfflineCashStateErrorV1::GuardRejected)?;
        let durable_acknowledgement = DurableAcknowledgementV1::from_acknowledgement(
            authorization.acknowledgement,
            &request,
            &payment,
        )?;
        if durable_acknowledgement
            .acknowledgement
            .inbox_receipt
            .credit_id
            != credit_id.0
        {
            return Err(OfflineCashStateErrorV1::InvalidAcknowledgement);
        }
        let journal_revision = expected.journal_revision_after;
        let receipt = AcceptedPaymentReceiptV1 {
            credit_id,
            envelope_digest,
            request: request.clone(),
            payment: payment.clone(),
            stage_certificate: stage_certificate.clone(),
            durable_acknowledgement: durable_acknowledgement.clone(),
        };
        if self.accepted_payment_receipts.contains_key(&credit_id) {
            return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
        }
        let maximum_committed_bytes = self.acceptance_ticket_book.committed_inbox_bytes();
        let mut next_ticket_book = self.acceptance_ticket_book.clone();
        next_ticket_book.consume(&request, &payment)?;
        let mut next_accepted_payment_receipts = self.accepted_payment_receipts.clone();
        let mut next_pending_credits = self.pending_credits.clone();
        if next_accepted_payment_receipts
            .insert(credit_id, receipt)
            .is_some()
        {
            return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
        }
        if next_pending_credits
            .insert(
                credit_id,
                StagedCreditV1 {
                    request,
                    payment,
                    envelope_digest,
                    stage_certificate,
                },
            )
            .is_some()
        {
            return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
        }
        let snapshot_usage = receiver_snapshot_capacity_usage_v1(
            &next_pending_credits,
            &next_accepted_payment_receipts,
            &self.consumed_credits,
        )?;
        next_ticket_book.reconcile_receiver_snapshot_usage(
            snapshot_usage.live_bytes,
            snapshot_usage.retained_bytes,
            maximum_committed_bytes,
        )?;
        self.accepted_payment_receipts = next_accepted_payment_receipts;
        self.pending_credits = next_pending_credits;
        self.acceptance_ticket_book = next_ticket_book;
        self.journal_revision = journal_revision;
        Ok(StagePaymentOutcomeV1::Staged {
            journal_revision,
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
    ) -> Result<TransitionPreviewV1, OfflineCashStateErrorV1> {
        next_epoch.validate()?;
        next_device_policy_binding.validate()?;
        if next_epoch.generation
            != self
                .state
                .hardware_epoch
                .generation
                .checked_add(1)
                .ok_or(OfflineCashStateErrorV1::HardwareEpochOverflow)?
            || next_epoch.epoch_id == self.state.hardware_epoch.epoch_id
            || next_device_policy_binding == self.state.device_policy_binding
        {
            return Err(OfflineCashStateErrorV1::InvalidHardwareRotation);
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
            OfflineCashTransitionKindV1::Rotate,
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
                    OfflineCashTransitionKindV1::Rotate,
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
    ) -> Result<OfflineCashStateV1, OfflineCashStateErrorV1> {
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
        Ok(self.state.clone())
    }

    /// Build a canonical recovery snapshot with a self-consistent commitment.
    pub fn snapshot(&self) -> Result<OfflineCashStateSnapshotV1, OfflineCashStateErrorV1> {
        let receiver_snapshot_usage = receiver_snapshot_capacity_usage_v1(
            &self.pending_credits,
            &self.accepted_payment_receipts,
            &self.consumed_credits,
        )?;
        self.acceptance_ticket_book
            .validate_recovered_with_snapshot_usage(
                receiver_snapshot_usage.live_bytes,
                receiver_snapshot_usage.retained_bytes,
            )?;
        self.sender_outbox_capacity
            .validate_recovered(&self.outgoing_candidate_journal)?;
        self.outgoing_candidate_journal.validate_recovered(
            &self.state,
            self.journal_revision,
            &self.sender_outbox_capacity,
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
                version: OFFLINE_CASH_STATE_VERSION_V1,
                state: self.state.clone(),
                journal_revision: self.journal_revision,
                pending_credits: pending_credits.clone(),
                accepted_recipient_bindings: accepted_recipient_bindings.clone(),
                accepted_payment_receipts: accepted_payment_receipts.clone(),
                consumed_credits: consumed_credits.clone(),
                acceptance_ticket_book: self.acceptance_ticket_book.clone(),
                sender_outbox_capacity: self.sender_outbox_capacity.clone(),
                outgoing_candidate_journal: self.outgoing_candidate_journal.clone(),
            },
        )?;
        Ok(OfflineCashStateSnapshotV1 {
            version: OFFLINE_CASH_STATE_VERSION_V1,
            state: self.state.clone(),
            journal_revision: self.journal_revision,
            pending_credits,
            accepted_recipient_bindings,
            accepted_payment_receipts,
            consumed_credits,
            acceptance_ticket_book: self.acceptance_ticket_book.clone(),
            sender_outbox_capacity: self.sender_outbox_capacity.clone(),
            outgoing_candidate_journal: self.outgoing_candidate_journal.clone(),
            snapshot_commitment,
        })
    }

    /// Preview the exact hardware-sealed recovery anchor for the current snapshot.
    pub fn preview_durability_anchor(
        &self,
    ) -> Result<DurabilityAnchorStatementV1, OfflineCashStateErrorV1> {
        let snapshot = self.snapshot()?;
        Ok(DurabilityAnchorStatementV1 {
            version: OFFLINE_CASH_STATE_VERSION_V1,
            lane: self.state.lane.clone(),
            state_commitment: self.state.state_commitment,
            hardware_epoch: self.state.hardware_epoch,
            device_policy_binding: self.state.device_policy_binding,
            state_nonce_commitment: self.state.state_nonce_commitment,
            logical_sequence: self.state.logical_sequence,
            journal_revision: self.journal_revision,
            snapshot_commitment: snapshot.snapshot_commitment,
        })
    }

    /// Verify and package a hardware-sealed recovery anchor.
    pub fn seal_durability_anchor(
        &self,
        guard_bundle: Vec<u8>,
    ) -> Result<DurabilityAnchorV1, OfflineCashStateErrorV1> {
        validate_guard_bytes(&guard_bundle)?;
        let statement = self.preview_durability_anchor()?;
        self.guard_verifier
            .verify_durability_anchor(&statement, &guard_bundle)
            .map_err(OfflineCashStateErrorV1::GuardRejected)?;
        Ok(DurabilityAnchorV1 {
            statement,
            guard_bundle,
        })
    }

    /// Restore a canonical snapshot only when it exactly matches the latest hardware anchor.
    pub fn restore(
        snapshot: OfflineCashStateSnapshotV1,
        anchor: &DurabilityAnchorV1,
        proof_release: OfflineCashStateProofReleaseV1,
        recursive_verifier: R,
        guard_verifier: G,
    ) -> Result<Self, OfflineCashStateErrorV1> {
        validate_guard_bytes(&anchor.guard_bundle)?;
        guard_verifier
            .verify_durability_anchor(&anchor.statement, &anchor.guard_bundle)
            .map_err(OfflineCashStateErrorV1::GuardRejected)?;
        if snapshot.version != OFFLINE_CASH_STATE_VERSION_V1
            || anchor.statement.version != OFFLINE_CASH_STATE_VERSION_V1
        {
            return Err(OfflineCashStateErrorV1::UnsupportedVersion(
                snapshot.version,
            ));
        }
        snapshot.state.validate()?;
        if snapshot.state.release_id != proof_release.release_id()
            || snapshot.state.liability_pool_id
                != derive_liability_pool_id(&snapshot.state.lane, snapshot.state.asset_incarnation)?
        {
            return Err(OfflineCashStateErrorV1::InvalidReleaseOrLiabilityPool);
        }
        let expected_snapshot_commitment = canonical_poseidon_digest(
            SNAPSHOT_COMMITMENT_DOMAIN,
            &SnapshotCommitmentPreimageV1 {
                version: snapshot.version,
                state: snapshot.state.clone(),
                journal_revision: snapshot.journal_revision,
                pending_credits: snapshot.pending_credits.clone(),
                accepted_recipient_bindings: snapshot.accepted_recipient_bindings.clone(),
                accepted_payment_receipts: snapshot.accepted_payment_receipts.clone(),
                consumed_credits: snapshot.consumed_credits.clone(),
                acceptance_ticket_book: snapshot.acceptance_ticket_book.clone(),
                sender_outbox_capacity: snapshot.sender_outbox_capacity.clone(),
                outgoing_candidate_journal: snapshot.outgoing_candidate_journal.clone(),
            },
        )?;
        if expected_snapshot_commitment != snapshot.snapshot_commitment {
            return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
        }
        let expected_anchor = DurabilityAnchorStatementV1 {
            version: OFFLINE_CASH_STATE_VERSION_V1,
            lane: snapshot.state.lane.clone(),
            state_commitment: snapshot.state.state_commitment,
            hardware_epoch: snapshot.state.hardware_epoch,
            device_policy_binding: snapshot.state.device_policy_binding,
            state_nonce_commitment: snapshot.state.state_nonce_commitment,
            logical_sequence: snapshot.state.logical_sequence,
            journal_revision: snapshot.journal_revision,
            snapshot_commitment: snapshot.snapshot_commitment,
        };
        if anchor.statement != expected_anchor {
            return Err(OfflineCashStateErrorV1::SnapshotRollback);
        }
        let consumed_credits = ExactConsumedCreditIndex::from_records(&snapshot.consumed_credits)?;
        if consumed_credits.root() != snapshot.state.consumed_credit_root {
            return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
        }
        snapshot
            .sender_outbox_capacity
            .validate_recovered(&snapshot.outgoing_candidate_journal)?;
        snapshot.outgoing_candidate_journal.validate_recovered(
            &snapshot.state,
            snapshot.journal_revision,
            &snapshot.sender_outbox_capacity,
            proof_release.artifacts,
            &recursive_verifier,
        )?;
        let mut accepted_recipient_bindings = BTreeSet::new();
        let mut previous_binding = None;
        for binding in snapshot.accepted_recipient_bindings {
            binding
                .validate()
                .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
            if previous_binding.is_some_and(|previous| previous >= binding)
                || !accepted_recipient_bindings.insert(binding)
            {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
            previous_binding = Some(binding);
        }
        if !accepted_recipient_bindings.contains(&snapshot.state.device_policy_binding) {
            return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
        }
        let mut accepted_payment_receipts = BTreeMap::new();
        let mut receipt_stage_revisions = BTreeSet::new();
        let mut previous_receipt_credit_id = None;
        for receipt in snapshot.accepted_payment_receipts {
            validate_peer_payment_against_context(
                &snapshot.state,
                proof_release,
                &recursive_verifier,
                &receipt.request,
                &receipt.payment,
            )
            .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
            receipt
                .durable_acknowledgement
                .validate_against(&receipt.request, &receipt.payment)?;
            let credit_id = CreditIdV1(receipt.payment.statement.lifecycle.credit_id);
            let statement = &receipt.stage_certificate.statement;
            if previous_receipt_credit_id.is_some_and(|previous| previous >= credit_id)
                || receipt.credit_id != credit_id
                || receipt.envelope_digest
                    != canonical_sha256_digest(CREDIT_ENVELOPE_DOMAIN, &receipt.payment)?
                || statement.version != OFFLINE_CASH_STATE_VERSION_V1
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
                        .ok_or(OfflineCashStateErrorV1::JournalRevisionOverflow)?
                || statement.receiver_hardware_epoch.generation
                    > snapshot.state.hardware_epoch.generation
                || (statement.receiver_hardware_epoch.generation
                    == snapshot.state.hardware_epoch.generation
                    && statement.receiver_hardware_epoch != snapshot.state.hardware_epoch)
                || (statement.receiver_hardware_epoch == snapshot.state.hardware_epoch
                    && statement.journal_revision_after > snapshot.journal_revision)
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
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
            validate_guard_bytes(&receipt.stage_certificate.guard_bundle)?;
            guard_verifier
                .verify_credit_stage(statement, &receipt.stage_certificate.guard_bundle)
                .map_err(OfflineCashStateErrorV1::GuardRejected)?;
            previous_receipt_credit_id = Some(credit_id);
        }
        let mut pending_credits = BTreeMap::new();
        let mut stage_revisions = BTreeSet::new();
        let mut previous_credit_id = None;
        for staged in snapshot.pending_credits {
            validate_peer_payment_against_context(
                &snapshot.state,
                proof_release,
                &recursive_verifier,
                &staged.request,
                &staged.payment,
            )
            .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
            let credit_id = CreditIdV1(staged.payment.statement.lifecycle.credit_id);
            let statement = &staged.stage_certificate.statement;
            if previous_credit_id.is_some_and(|previous| previous >= credit_id)
                || staged.envelope_digest
                    != canonical_sha256_digest(CREDIT_ENVELOPE_DOMAIN, &staged.payment)?
                || consumed_credits.get(credit_id).is_some()
                || statement.version != OFFLINE_CASH_STATE_VERSION_V1
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
                        .ok_or(OfflineCashStateErrorV1::JournalRevisionOverflow)?
                || statement.receiver_hardware_epoch.generation
                    > snapshot.state.hardware_epoch.generation
                || (statement.receiver_hardware_epoch.generation
                    == snapshot.state.hardware_epoch.generation
                    && statement.receiver_hardware_epoch != snapshot.state.hardware_epoch)
                || (statement.receiver_hardware_epoch == snapshot.state.hardware_epoch
                    && statement.journal_revision_after > snapshot.journal_revision)
                || !stage_revisions.insert((
                    statement.receiver_hardware_epoch.epoch_id,
                    statement.journal_revision_after,
                ))
            {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
            let receipt = accepted_payment_receipts
                .get(&credit_id)
                .ok_or(OfflineCashStateErrorV1::SnapshotIntegrity)?;
            if receipt.request != staged.request
                || receipt.payment != staged.payment
                || receipt.envelope_digest != staged.envelope_digest
                || receipt.stage_certificate != staged.stage_certificate
            {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
            validate_guard_bytes(&staged.stage_certificate.guard_bundle)?;
            guard_verifier
                .verify_credit_stage(statement, &staged.stage_certificate.guard_bundle)
                .map_err(OfflineCashStateErrorV1::GuardRejected)?;
            previous_credit_id = Some(credit_id);
            pending_credits.insert(credit_id, staged);
        }
        for (credit_id, receipt) in &accepted_payment_receipts {
            if !pending_credits.contains_key(credit_id)
                && consumed_credits.get(*credit_id) != Some(receipt.envelope_digest)
            {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
        }
        let receiver_snapshot_usage = receiver_snapshot_capacity_usage_v1(
            &pending_credits,
            &accepted_payment_receipts,
            &consumed_credits,
        )?;
        snapshot
            .acceptance_ticket_book
            .validate_recovered_with_snapshot_usage(
                receiver_snapshot_usage.live_bytes,
                receiver_snapshot_usage.retained_bytes,
            )?;
        Ok(Self {
            state: snapshot.state,
            journal_revision: snapshot.journal_revision,
            pending_credits,
            accepted_recipient_bindings,
            accepted_payment_receipts,
            consumed_credits,
            acceptance_ticket_book: snapshot.acceptance_ticket_book,
            sender_outbox_capacity: snapshot.sender_outbox_capacity,
            outgoing_candidate_journal: snapshot.outgoing_candidate_journal,
            proof_release,
            recursive_verifier,
            guard_verifier,
        })
    }

    fn validate_peer_payment(
        &self,
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
    ) -> Result<(), OfflineCashStateErrorV1> {
        validate_peer_payment_against_context(
            &self.state,
            self.proof_release,
            &self.recursive_verifier,
            request,
            payment,
        )
    }

    fn ensure_credit_id_available(
        &self,
        credit_id: CreditIdV1,
        envelope_digest: DigestV1,
    ) -> Result<(), OfflineCashStateErrorV1> {
        if let Some(existing) = self.pending_credits.get(&credit_id) {
            return if existing.envelope_digest == envelope_digest {
                Err(OfflineCashStateErrorV1::CreditAlreadyConsumed(credit_id))
            } else {
                Err(OfflineCashStateErrorV1::CreditConflict(credit_id))
            };
        }
        if let Some(existing) = self.consumed_credits.get(credit_id) {
            return if existing == envelope_digest {
                Err(OfflineCashStateErrorV1::CreditAlreadyConsumed(credit_id))
            } else {
                Err(OfflineCashStateErrorV1::CreditConflict(credit_id))
            };
        }
        Ok(())
    }

    fn next_state(
        &self,
        balance: u128,
        hardware_epoch: HardwareEpochV1,
        device_policy_binding: DevicePolicyBindingV1,
        state_nonce_commitment: DigestV1,
        consumed_credit_root: OfflineCashPastaStateCommitmentV1,
    ) -> Result<OfflineCashStateV1, OfflineCashStateErrorV1> {
        if state_nonce_commitment == [0; 32]
            || state_nonce_commitment == self.state.state_nonce_commitment
        {
            return Err(OfflineCashStateErrorV1::InvalidStateNonceCommitment);
        }
        let logical_sequence = if hardware_epoch == self.state.hardware_epoch {
            self.state
                .logical_sequence
                .checked_add(1)
                .ok_or(OfflineCashStateErrorV1::SequenceOverflow)?
        } else {
            0
        };
        OfflineCashStateV1::build(
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
        kind: OfflineCashTransitionKindV1,
        successor: OfflineCashStateV1,
        effect_digest: DigestV1,
        mint_finality_semantic_digest: DigestV1,
        mint_finality_proof_binding_digest: DigestV1,
        peer_credit_id: DigestV1,
        peer_recipient_lane_id: DigestV1,
        auxiliary: TransitionAuxiliaryBindingsV1,
        trusted_commit_time_ms: u64,
        transport_semantic_digest: F,
    ) -> Result<TransitionPreviewV1, OfflineCashStateErrorV1>
    where
        F: FnOnce(DigestV1) -> Result<DigestV1, OfflineCashStateErrorV1>,
    {
        if !matches!(
            self.outgoing_candidate_journal.stage(),
            OfflineCashOutgoingJournalStageV1::Empty
        ) {
            return Err(OfflineCashStateErrorV1::InvalidCandidateStage);
        }
        if trusted_commit_time_ms == 0 {
            return Err(OfflineCashStateErrorV1::InvalidTrustedCommitTime);
        }
        if (kind == OfflineCashTransitionKindV1::MintFold)
            != (mint_finality_semantic_digest != [0; 32])
            || (kind == OfflineCashTransitionKindV1::MintFold)
                != (mint_finality_proof_binding_digest != [0; 32])
        {
            return Err(OfflineCashStateErrorV1::InvalidMintCredit);
        }
        let is_peer = matches!(
            kind,
            OfflineCashTransitionKindV1::SendSplit | OfflineCashTransitionKindV1::ReceiveFold
        );
        if is_peer != (peer_credit_id != [0; 32]) || is_peer != (peer_recipient_lane_id != [0; 32])
        {
            return Err(OfflineCashStateErrorV1::InvalidPeerCredit);
        }
        let is_upgrade = kind == OfflineCashTransitionKindV1::SuiteUpgrade;
        if is_upgrade != (auxiliary.suite_upgrade_authorization_digest != [0; 32]) {
            return Err(OfflineCashStateErrorV1::StateInvariant);
        }
        let is_terminal = matches!(
            kind,
            OfflineCashTransitionKindV1::SendSplit | OfflineCashTransitionKindV1::RedeemSplit
        );
        let lifecycle_binding_digest =
            if auxiliary.lifecycle_binding_digest == [0; 32] && !is_terminal {
                canonical_sha256_digest(
                    TRANSITION_LIFECYCLE_DOMAIN,
                    &(
                        kind,
                        self.state.protocol_version,
                        self.state.suite_id,
                        self.state.vk_digest,
                        self.state.release_id,
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
            || is_terminal != (auxiliary.precommit_binding_digest != [0; 32])
        {
            return Err(OfflineCashStateErrorV1::StateInvariant);
        }
        let journal_revision_after = if kind == OfflineCashTransitionKindV1::Rotate {
            0
        } else {
            self.journal_revision
                .checked_add(1)
                .ok_or(OfflineCashStateErrorV1::JournalRevisionOverflow)?
        };
        let proof_statement = TransitionProofStatementV1 {
            version: OFFLINE_CASH_STATE_VERSION_V1,
            protocol_version: self.state.protocol_version,
            predecessor_suite_id: self.state.suite_id,
            predecessor_vk_digest: self.state.vk_digest,
            successor_suite_id: successor.suite_id,
            successor_vk_digest: successor.vk_digest,
            kind,
            amount: match kind {
                OfflineCashTransitionKindV1::MintFold
                | OfflineCashTransitionKindV1::ReceiveFold => successor
                    .balance
                    .checked_sub(self.state.balance)
                    .ok_or(OfflineCashStateErrorV1::StateInvariant)?,
                OfflineCashTransitionKindV1::SendSplit
                | OfflineCashTransitionKindV1::RedeemSplit => self
                    .state
                    .balance
                    .checked_sub(successor.balance)
                    .ok_or(OfflineCashStateErrorV1::StateInvariant)?,
                OfflineCashTransitionKindV1::SuiteUpgrade | OfflineCashTransitionKindV1::Rotate => {
                    0
                }
            },
            mint_finality_semantic_digest,
            mint_finality_proof_binding_digest,
            peer_credit_id,
            peer_recipient_lane_id,
            lifecycle_binding_digest,
            precommit_binding_digest: auxiliary.precommit_binding_digest,
            suite_upgrade_authorization_digest: auxiliary.suite_upgrade_authorization_digest,
            release_id: self.state.release_id,
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
        let guard_context = transition_guard_context(
            self.proof_release.artifacts,
            &proof_statement,
            trusted_commit_time_ms,
        )?;
        let normalized_guard_statement =
            OfflineCashNormalizedGuardStatementV1::derive_from_transition(
                &proof_statement,
                guard_context,
            )
            .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?;
        let normalized_guard_statement_digest = normalized_guard_statement
            .canonical_digest()
            .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?;
        let state_transition_digest = transition_statement_digest(&proof_statement)?;
        let hardware_statement = HardwareTransitionStatementV1 {
            version: OFFLINE_CASH_STATE_VERSION_V1,
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
            .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?;
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
    ) -> Result<(), OfflineCashStateErrorV1> {
        validate_paired_proof(&authorization.proof, preview.transport_semantic_digest)?;
        validate_guard_bytes(&authorization.hardware_certificate.guard_bundle)?;
        authorization
            .hardware_certificate
            .statement
            .validate_exact_next()?;
        if authorization.hardware_certificate.statement != preview.hardware_statement {
            return Err(OfflineCashStateErrorV1::HardwareCertificateMismatch);
        }
        preview
            .normalized_guard_statement
            .validate_hardware_binding(&preview.proof_statement, &preview.hardware_statement)
            .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?;
        let public_inputs = transition_state_public_inputs(
            self.proof_release.artifacts,
            &self.state,
            preview,
            &authorization.proof,
        )?;
        verify_offline_cash_state_proof_v1(
            &self.recursive_verifier,
            self.proof_release.artifacts,
            &public_inputs,
            &authorization.proof,
        )
        .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?;
        self.guard_verifier
            .verify_transition(
                &preview.hardware_statement,
                &authorization.hardware_certificate.guard_bundle,
            )
            .map_err(OfflineCashStateErrorV1::GuardRejected)
    }

    fn commit_preview(&mut self, preview: TransitionPreviewV1) {
        self.state = preview.successor;
        self.journal_revision = preview.journal_revision_after;
    }
}

fn required_pending_credit_prefix(
    current_balance: u128,
    amount: u128,
    pending: impl IntoIterator<Item = (CreditIdV1, u128)>,
) -> Result<Vec<CreditIdV1>, OfflineCashStateErrorV1> {
    let mut available = current_balance;
    if available >= amount {
        return Ok(Vec::new());
    }

    let mut required = Vec::new();
    for (credit_id, credit_amount) in pending {
        available = available
            .checked_add(credit_amount)
            .ok_or(OfflineCashStateErrorV1::ArithmeticOverflow)?;
        required.push(credit_id);
        if available >= amount {
            return Ok(required);
        }
    }

    Err(OfflineCashStateErrorV1::InsufficientBalance)
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
    carried_consumed_credit_root: OfflineCashPastaStateCommitmentV1,
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
    kind: OfflineCashTransitionKindV1,
    transition_effect_digest: DigestV1,
    predecessor_state_commitment: DigestV1,
    successor_state_commitment: DigestV1,
    journal_revision_after: u128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode)]
struct LocalTransitionTransportStatementV1 {
    version: u16,
    kind: OfflineCashTransitionKindV1,
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
    lane: &OfflineCashLaneIdV1,
    asset_incarnation: AxtAssetIncarnationV1,
) -> Result<DigestV1, OfflineCashStateErrorV1> {
    offline_cash_liability_pool_id_v1(&lane.network_id, &lane.asset, asset_incarnation)
        .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)
}

fn local_transition_transport_digest(
    kind: OfflineCashTransitionKindV1,
    release_id: DigestV1,
    liability_pool_id: DigestV1,
    transition_effect_digest: DigestV1,
    predecessor_state_commitment: DigestV1,
    successor_state_commitment: DigestV1,
    normalized_guard_statement_digest: DigestV1,
) -> Result<DigestV1, OfflineCashStateErrorV1> {
    canonical_sha256_digest(
        TRANSPORT_STATEMENT_DOMAIN,
        &LocalTransitionTransportStatementV1 {
            version: OFFLINE_CASH_STATE_VERSION_V1,
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
    artifacts: OfflineCashRecursionArtifactsV1,
    statement: &BootstrapStatementV1,
    trusted_commit_time_ms: u64,
) -> Result<OfflineCashGuardContextV1, OfflineCashStateErrorV1> {
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
    Ok(OfflineCashGuardContextV1 {
        release_id: artifacts.release_id,
        liability_pool_id: statement.liability_pool_id,
        lifecycle_binding_digest: canonical_sha256_digest(TRANSITION_LIFECYCLE_DOMAIN, statement)?,
        precommit_binding_digest: [0; 32],
        sender_one_time_authorization_digest: [0; 32],
        suite_upgrade_authorization_digest: [0; 32],
        transition_intent_digest,
        transition_effect_digest,
        recovery_record_digest,
        durable_inbox_effect_digest: artifacts.canonical_empty_effect_digest,
        durable_outbox_effect_digest: artifacts.canonical_empty_effect_digest,
        canonical_empty_effect_digest: artifacts.canonical_empty_effect_digest,
    })
}

fn transition_guard_context(
    artifacts: OfflineCashRecursionArtifactsV1,
    statement: &TransitionProofStatementV1,
    trusted_commit_time_ms: u64,
) -> Result<OfflineCashGuardContextV1, OfflineCashStateErrorV1> {
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
        OfflineCashTransitionKindV1::MintFold | OfflineCashTransitionKindV1::ReceiveFold => (
            canonical_sha256_digest(DURABLE_INBOX_EFFECT_DOMAIN, &durable_effect)?,
            empty,
        ),
        OfflineCashTransitionKindV1::SendSplit | OfflineCashTransitionKindV1::RedeemSplit => (
            empty,
            canonical_sha256_digest(DURABLE_OUTBOX_EFFECT_DOMAIN, &durable_effect)?,
        ),
        OfflineCashTransitionKindV1::SuiteUpgrade | OfflineCashTransitionKindV1::Rotate => {
            (empty, empty)
        }
    };
    Ok(OfflineCashGuardContextV1 {
        release_id: artifacts.release_id,
        liability_pool_id: derive_liability_pool_id(&statement.lane, statement.asset_incarnation)?,
        lifecycle_binding_digest: statement.lifecycle_binding_digest,
        precommit_binding_digest: statement.precommit_binding_digest,
        sender_one_time_authorization_digest: [0; 32],
        suite_upgrade_authorization_digest: statement.suite_upgrade_authorization_digest,
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
) -> Result<DigestV1, OfflineCashStateErrorV1> {
    canonical_sha256_digest(TRANSITION_STATEMENT_DOMAIN, statement)
}

fn transport_semantic_digest(
    normalized_guard_statement_digest: DigestV1,
) -> Result<DigestV1, OfflineCashStateErrorV1> {
    canonical_sha256_digest(
        TRANSPORT_STATEMENT_DOMAIN,
        &normalized_guard_statement_digest,
    )
}

fn bootstrap_state_public_inputs(
    artifacts: OfflineCashRecursionArtifactsV1,
    preview: &BootstrapPreviewV1,
    proof: &OfflineCashPairedProofV1,
) -> Result<OfflineCashStateRelationPublicInputsV1, OfflineCashStateErrorV1> {
    let guard = &preview.normalized_guard_statement;
    Ok(OfflineCashStateRelationPublicInputsV1 {
        operation: super::offline_cash_v1_recursion::OfflineCashOperationV1::Bootstrap,
        predecessor: None,
        successor: preview.state.clone(),
        amount: 0,
        journal_revision_before: 0,
        journal_revision_after: 0,
        transition_effect_digest: guard.transition_effect_digest,
        mint_finality_semantic_digest: [0; 32],
        mint_finality_proof_binding_digest: [0; 32],
        peer_credit_id: [0; 32],
        peer_recipient_lane_id: [0; 32],
        lifecycle_binding_digest: guard.lifecycle_binding_digest,
        precommit_binding_digest: [0; 32],
        suite_upgrade_authorization_digest: [0; 32],
        transport_semantic_digest: preview.transport_semantic_digest,
        guard_statement_digest: guard
            .canonical_digest()
            .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?,
        eq_protocol_digest: artifacts.eq_protocol_digest,
        ep_protocol_digest: artifacts.ep_protocol_digest,
        guard_eq_protocol_digest: artifacts
            .guard_bundle_protocol_digest(
                super::offline_cash_v1_recursion::OfflineCashPastaParityV1::Eq,
            )
            .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?,
        guard_ep_protocol_digest: artifacts
            .guard_bundle_protocol_digest(
                super::offline_cash_v1_recursion::OfflineCashPastaParityV1::Ep,
            )
            .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?,
        mint_eq_protocol_digest: artifacts
            .mint_finality_protocol_digest(
                super::offline_cash_v1_recursion::OfflineCashPastaParityV1::Eq,
            )
            .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?,
        mint_ep_protocol_digest: artifacts
            .mint_finality_protocol_digest(
                super::offline_cash_v1_recursion::OfflineCashPastaParityV1::Ep,
            )
            .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?,
        guard_eq_credential_audit: proof.guard_eq_credential_audit,
        guard_ep_credential_audit: proof.guard_ep_credential_audit,
        eq_deferred_audit: proof.eq_deferred_audit,
        ep_deferred_audit: proof.ep_deferred_audit,
    })
}

fn transition_state_public_inputs(
    artifacts: OfflineCashRecursionArtifactsV1,
    predecessor: &OfflineCashStateV1,
    preview: &TransitionPreviewV1,
    proof: &OfflineCashPairedProofV1,
) -> Result<OfflineCashStateRelationPublicInputsV1, OfflineCashStateErrorV1> {
    let statement = &preview.proof_statement;
    let guard_digest = preview
        .normalized_guard_statement
        .canonical_digest()
        .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?;
    Ok(OfflineCashStateRelationPublicInputsV1 {
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
        peer_recipient_lane_id: statement.peer_recipient_lane_id,
        lifecycle_binding_digest: statement.lifecycle_binding_digest,
        precommit_binding_digest: statement.precommit_binding_digest,
        suite_upgrade_authorization_digest: statement.suite_upgrade_authorization_digest,
        transport_semantic_digest: preview.transport_semantic_digest,
        guard_statement_digest: guard_digest,
        eq_protocol_digest: artifacts.eq_protocol_digest,
        ep_protocol_digest: artifacts.ep_protocol_digest,
        guard_eq_protocol_digest: artifacts
            .guard_bundle_protocol_digest(
                super::offline_cash_v1_recursion::OfflineCashPastaParityV1::Eq,
            )
            .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?,
        guard_ep_protocol_digest: artifacts
            .guard_bundle_protocol_digest(
                super::offline_cash_v1_recursion::OfflineCashPastaParityV1::Ep,
            )
            .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?,
        mint_eq_protocol_digest: artifacts
            .mint_finality_protocol_digest(
                super::offline_cash_v1_recursion::OfflineCashPastaParityV1::Eq,
            )
            .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?,
        mint_ep_protocol_digest: artifacts
            .mint_finality_protocol_digest(
                super::offline_cash_v1_recursion::OfflineCashPastaParityV1::Ep,
            )
            .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?,
        guard_eq_credential_audit: proof.guard_eq_credential_audit,
        guard_ep_credential_audit: proof.guard_ep_credential_audit,
        eq_deferred_audit: proof.eq_deferred_audit,
        ep_deferred_audit: proof.ep_deferred_audit,
    })
}

fn validate_peer_payment_against_context<R: OfflineCashRecursiveVerifierV1>(
    state: &OfflineCashStateV1,
    proof_release: OfflineCashStateProofReleaseV1,
    recursive_verifier: &R,
    request: &OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
) -> Result<(), OfflineCashStateErrorV1> {
    payment
        .validate_shape_against(request)
        .map_err(|_| OfflineCashStateErrorV1::InvalidPeerCredit)?;
    let lifecycle = &payment.statement.lifecycle;
    if lifecycle.network_id != state.lane.network_id
        || lifecycle.asset != state.lane.asset
        || lifecycle.asset_incarnation != state.asset_incarnation
        || lifecycle.scale != state.lane.scale
        || lifecycle.release_id != state.release_id
        || lifecycle.liability_pool_id != state.liability_pool_id
        || payment.artifact_manifest_digest != proof_release.artifacts.artifact_manifest_digest
        || request.hardware_credential.lane_commitment != state.lane.device_lane_id
    {
        return Err(OfflineCashStateErrorV1::InvalidPeerCredit);
    }
    let public_output = peer_payment_public_output(request, payment)?;
    let verified = verify_offline_cash_recursive_proof_v1(
        recursive_verifier,
        proof_release.artifacts,
        public_output.clone(),
        &payment.proof,
    )
    .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?;
    if verified.public_output() != public_output {
        return Err(OfflineCashStateErrorV1::ProofRejected(
            "recursive peer-credit output substitution".to_owned(),
        ));
    }
    Ok(())
}

fn peer_payment_public_output(
    request: &OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
) -> Result<OfflineCashRecursivePublicOutputV1, OfflineCashStateErrorV1> {
    let statement = &payment.statement;
    let semantic_digest = statement
        .canonical_digest()
        .map_err(|_| OfflineCashStateErrorV1::InvalidPeerCredit)?;
    OfflineCashRecursivePublicOutputV1::new(
        statement.lifecycle.clone(),
        semantic_digest,
        payment.proof.candidate_envelope_digest,
        payment.proof.commit_certificate_digest,
        statement.transition_nullifier,
        statement.request_digest,
        statement.acceptance_ticket_digest,
        statement.ciphertext_commitment,
        statement.amount,
        canonical_terminal_send_output_binding_v1(
            statement.lifecycle.credit_id,
            request.hardware_credential.lane_commitment,
            statement.request_digest,
            statement.acceptance_ticket_digest,
            statement.ciphertext_commitment,
            statement.amount,
        ),
    )
    .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))
}

fn canonical_sha256_digest<T: Encode>(
    domain: &[u8],
    value: &T,
) -> Result<DigestV1, OfflineCashStateErrorV1> {
    let encoded =
        norito::encode_canonical(value).map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?;
    let mut hasher = Sha256::new();
    hasher.update(
        u64::try_from(domain.len())
            .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?
            .to_be_bytes(),
    );
    hasher.update(domain);
    hasher.update(
        u64::try_from(encoded.len())
            .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?
            .to_be_bytes(),
    );
    hasher.update(encoded);
    Ok(hasher.finalize().into())
}

fn canonical_poseidon_digest<T: Encode>(
    domain: &[u8],
    value: &T,
) -> Result<DigestV1, OfflineCashStateErrorV1> {
    let encoded =
        norito::encode_canonical(value).map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?;
    let mut framed = Vec::with_capacity(
        domain
            .len()
            .saturating_add(encoded.len())
            .saturating_add(16),
    );
    framed.extend_from_slice(
        &u64::try_from(domain.len())
            .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?
            .to_be_bytes(),
    );
    framed.extend_from_slice(domain);
    framed.extend_from_slice(
        &u64::try_from(encoded.len())
            .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?
            .to_be_bytes(),
    );
    framed.extend_from_slice(&encoded);
    Ok(poseidon::hash_bytes(&framed))
}

fn validate_paired_proof(
    proof: &OfflineCashPairedProofV1,
    semantic_digest: DigestV1,
) -> Result<(), OfflineCashStateErrorV1> {
    proof
        .validate_shape_for_semantic_digest(semantic_digest)
        .map_err(|_| OfflineCashStateErrorV1::InvalidProofBundle)
}

fn validate_guard_bytes(bytes: &[u8]) -> Result<(), OfflineCashStateErrorV1> {
    if bytes.is_empty() || bytes.len() > OFFLINE_CASH_GUARD_BUNDLE_MAX_BYTES_V1 {
        return Err(OfflineCashStateErrorV1::InvalidGuardBundle);
    }
    Ok(())
}
