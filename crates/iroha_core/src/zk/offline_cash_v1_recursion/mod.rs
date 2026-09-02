//! Fixed-profile paired-Pasta recursion seam for Offline Cash V1.
//!
//! This module defines the sole Offline Cash V1 aggregate proof stack. It fixes one
//! `k = 16` Eq/Ep cycle, parses the public 544-byte delayed-history accumulators without field
//! reduction, reconciles Core's aggregate-state statements with the normalized hardware guard
//! statement, and exposes a verifier interface which fails closed until the governed recursive
//! circuits are installed. Native BGH19 accumulation and terminal decisions live in
//! [`accumulation`].

mod accumulation;
mod artifacts;
mod commit_wrapper;
#[cfg(feature = "zk-halo2-ipa")]
mod composite;
#[cfg(feature = "zk-halo2-ipa")]
mod deferred_parent;
mod generation;
mod guard_bundle;
#[cfg(feature = "zk-halo2-ipa")]
mod mint_authority;
#[cfg(feature = "zk-halo2-ipa")]
mod mint_authorization;
mod mint_finality;
#[cfg(feature = "zk-halo2-ipa")]
mod mint_helper;
mod native_backend;
mod relation;
mod state_relation;

#[cfg(all(test, feature = "zk-halo2-ipa"))]
mod real_handoff_qualification_tests;
#[cfg(test)]
mod tests;

pub use accumulation::{
    OfflineCashEpAccumulatorV1, OfflineCashEpFoldOutputV1, OfflineCashEpFoldProofV1,
    OfflineCashEqAccumulatorV1, OfflineCashEqFoldOutputV1, OfflineCashEqFoldProofV1,
    decide_offline_cash_ep_accumulator_v1, decide_offline_cash_eq_accumulator_v1,
    fold_offline_cash_ep_accumulators_v1, fold_offline_cash_eq_accumulators_v1,
    initial_offline_cash_ep_accumulator_v1, initial_offline_cash_eq_accumulator_v1,
    verify_and_decide_offline_cash_ep_fold_v1, verify_and_decide_offline_cash_eq_fold_v1,
};
pub use artifacts::{
    OfflineCashArtifactByteResolverV1, OfflineCashArtifactDescriptorV1, OfflineCashArtifactErrorV1,
    OfflineCashArtifactKindV1, OfflineCashAuthenticatedArtifactSetV1, OfflineCashCircuitFamilyV1,
    OfflineCashDirectoryArtifactResolverV1, OfflineCashMemoryArtifactResolverV1,
};
pub(crate) use commit_wrapper::{
    COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1, OfflineCashCommitWrapperIntentAuthorizationPrivateV1,
    OfflineCashCommitWrapperPrivateTransitionV1, OfflineCashCommitWrapperPublicInputsV1,
    canonical_commit_certificate_digest_v1, canonical_terminal_send_output_binding_v1,
};
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) use commit_wrapper::{
    COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1, OfflineCashCommitWrapperEpCircuitV1,
    OfflineCashCommitWrapperEpWitnessV1, OfflineCashCommitWrapperEqCircuitV1,
    OfflineCashCommitWrapperEqWitnessV1, OfflineCashCommitWrapperWitnessV1,
    build_offline_cash_commit_wrapper_pair_v1, canonical_commit_wrapper_candidate_digest_v1,
    constrain_enabled_hardware_profile_membership_v1,
    public_instance as offline_cash_commit_wrapper_public_instance_v1,
};
#[cfg(feature = "zk-halo2-ipa")]
pub use generation::{
    OFFLINE_CASH_COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1,
    OfflineCashAcceptanceIntentAuthorizationGenerationPublicV1,
    OfflineCashCommitEvidenceOpeningGenerationV1, OfflineCashCommitWrapperEpGenerationWitnessV1,
    OfflineCashCommitWrapperEqGenerationWitnessV1, OfflineCashCommitWrapperGenerationPublicV1,
    OfflineCashCommitWrapperGenerationWitnessV1,
    OfflineCashCommitWrapperPrivateGenerationWitnessV1,
    OfflineCashCommitWrapperTerminalGenerationPublicV1,
    OfflineCashGeneratedCommitWrapperArtifactsV1, OfflineCashGeneratedCommitWrapperEnvelopeV1,
    OfflineCashGeneratedCommitWrapperProofV1, OfflineCashGeneratedMintAuthorityArtifactsV1,
    OfflineCashGeneratedMintAuthorityProofV1, OfflineCashGeneratedMintAuthorizationArtifactsV1,
    OfflineCashGeneratedMintAuthorizationProofV1, OfflineCashGeneratedRecursiveStateArtifactsV1,
    OfflineCashGeneratedRecursiveStateProofV1, OfflineCashLoadedEpCommitWrapperArtifactsV1,
    OfflineCashLoadedEpMintAuthorityArtifactsV1, OfflineCashLoadedEpMintAuthorizationArtifactsV1,
    OfflineCashLoadedEpRecursiveStateArtifactsV1, OfflineCashLoadedEqCommitWrapperArtifactsV1,
    OfflineCashLoadedEqMintAuthorityArtifactsV1, OfflineCashLoadedEqMintAuthorizationArtifactsV1,
    OfflineCashLoadedEqRecursiveStateArtifactsV1, OfflineCashMintAuthorityGenerationWitnessV1,
    OfflineCashMintAuthorizationGenerationWitnessV1, OfflineCashNoCommitClosureGenerationPublicV1,
    OfflineCashRecursiveIncomingEpGenerationWitnessV1,
    OfflineCashRecursiveIncomingEqGenerationWitnessV1,
    OfflineCashRecursiveStateGenerationWitnessV1, OfflineCashSuiteUpgradeGenerationWitnessV1,
    generate_offline_cash_commit_wrapper_artifacts_v1,
    generate_offline_cash_mint_authority_artifacts_v1,
    generate_offline_cash_mint_authorization_artifacts_v1,
    generate_offline_cash_recursive_state_artifacts_v1,
    load_offline_cash_ep_commit_wrapper_artifacts_v1,
    load_offline_cash_ep_mint_authority_artifacts_v1,
    load_offline_cash_ep_mint_authorization_artifacts_v1,
    load_offline_cash_ep_recursive_state_artifacts_v1,
    load_offline_cash_eq_commit_wrapper_artifacts_v1,
    load_offline_cash_eq_mint_authority_artifacts_v1,
    load_offline_cash_eq_mint_authorization_artifacts_v1,
    load_offline_cash_eq_recursive_state_artifacts_v1,
    offline_cash_commit_wrapper_enabled_profile_table_v1, prove_offline_cash_commit_wrapper_v1,
    prove_offline_cash_finalized_mint_from_checkpoint_v1,
    prove_offline_cash_mint_authority_bootstrap_v1,
    prove_offline_cash_mint_authority_rotation_from_checkpoint_v1,
    prove_offline_cash_mint_authority_v1, prove_offline_cash_mint_authorization_v1,
    prove_offline_cash_recursive_state_v1,
};
pub use generation::{
    OFFLINE_CASH_OPERATION_RELATION_SCHEMA_ID_V1, OfflineCashArtifactGenerationErrorV1,
    OfflineCashGeneratedOperationArtifactsV1,
};
pub use guard_bundle::{
    OFFLINE_CASH_HARDWARE_POLICY_TREE_DEPTH_V1, OfflineCashGuardBundleRelationWitnessV1,
    OfflineCashPlatformCredentialRelationCircuitV1, OfflineCashPlatformCredentialRelationWitnessV1,
    OfflineCashPlatformCredentialStatementV1,
};
#[cfg(feature = "zk-halo2-ipa")]
pub use mint_authority::{
    OfflineCashMintAuthorityCheckpointV1, OfflineCashMintAuthorityPairBindingV1,
};
#[cfg(feature = "zk-halo2-ipa")]
pub use mint_authorization::OfflineCashMintAuthorizationRelationWitnessV1;
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) use mint_authorization::{
    MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1, OfflineCashMintAuthorizationEpCircuitV1,
    OfflineCashMintAuthorizationEqCircuitV1, OfflineCashMintAuthorizationRecursiveWitnessV1,
    build_offline_cash_mint_authorization_pair_v1, mint_authorization_public_instances_v1,
};
pub use mint_finality::{
    OfflineCashMintFinalityErrorV1, OfflineCashMintFinalityLocalAuthorityV1,
    OfflineCashMintFinalitySignerV1, OfflineCashMintFinalityTreeV1,
    build_offline_cash_mint_finality_seal_message_v1,
    decode_offline_cash_mint_finality_seal_bundle_v1,
    decode_offline_cash_mint_finality_seal_share_v1,
    derive_offline_cash_mint_finality_validator_keys_v1, offline_cash_mint_finality_empty_root_v1,
    offline_cash_top_up_leaf_from_receipt_v1, sign_offline_cash_mint_finality_seal_v1,
    validate_offline_cash_mint_finality_epoch_v1, verify_offline_cash_mint_finality_seal_bundle_v1,
    verify_offline_cash_mint_finality_seal_share_v1, verify_offline_cash_top_up_membership_v1,
};
#[cfg(feature = "zk-halo2-ipa")]
pub use mint_helper::{OfflineCashMintAuthorityStepV1, OfflineCashMintCertificateWitnessV1};
pub use native_backend::{
    OfflineCashAuthenticatedRecursiveVerifierV1, OfflineCashRecursiveVerifierProfileV1,
};
pub use relation::{
    OfflineCashOperationRelationCircuitV1, OfflineCashOperationRelationConfigV1,
    OfflineCashOperationRelationWitnessV1,
};
pub use state_relation::{
    OfflineCashStateRelationCircuitV1, OfflineCashStateRelationPublicInputsV1,
    OfflineCashStateRelationWitnessV1, public_instance as offline_cash_state_public_instance_v1,
};

use iroha_data_model::isi::OfflineCashRedemptionRequestV1;
use iroha_data_model::nexus::AxtAssetIncarnationV1;
use iroha_data_model::offline::{
    OFFLINE_CASH_ASSET_SCALE_MAX_V1, OFFLINE_CASH_HELPER_PROVING_KEY_MAX_BYTES_V1,
    OFFLINE_CASH_RECEIVE_FOLD_BATCH_WIDTH_V1, OFFLINE_CASH_VERIFYING_KEY_MAX_BYTES_V1,
    OFFLINE_CASH_WIRE_VERSION_V1, OfflineCashArtifactBindingV1, OfflineCashArtifactRoleV1,
    OfflineCashAuthenticatedReleaseV1, OfflineCashCommitWrapperProofV1,
    OfflineCashLifecycleBindingV1, OfflineCashMintCreditStatementV1, OfflineCashMintCreditV1,
    OfflineCashNoCommitClosureStatementV1, OfflineCashNoCommitClosureV1,
    OfflineCashOperationKindV1, OfflineCashPairedProofV1, OfflineCashPastaStateCommitmentV1,
    OfflineCashQualifiedHelperCircuitV1, OfflineCashRedemptionStatementV1,
    OfflineCashTransferStatementV1, offline_cash_asset_identity_digest_v1,
    offline_cash_liability_pool_id_v1, offline_cash_pasta_state_commitment_v1,
};
pub use iroha_data_model::offline::{
    OFFLINE_CASH_CURRENT_PROOFS_MAX_BYTES_V1, OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1,
    OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1, OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1,
};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::offline_cash_v1_state::{
    BootstrapStatementV1, ConsumedCreditInsertWitnessV1, DigestV1, HardwareTransitionStatementV1,
    OfflineCashLaneIdV1, OfflineCashTransitionKindV1, TransitionProofStatementV1,
};

/// Fixed Halo2/IPA domain exponent used by both Offline Cash V1 Pasta parities.
pub const OFFLINE_CASH_RECURSION_IPA_K_V1: u32 = 16;
/// Width of the fixed IPA proof and BGH19 Poseidon transcript.
pub(crate) const OFFLINE_CASH_IPA_POSEIDON_WIDTH_V1: usize = 3;
/// Rate of the fixed IPA proof and BGH19 Poseidon transcript.
pub(crate) const OFFLINE_CASH_IPA_POSEIDON_RATE_V1: usize = 2;
/// Full rounds of the fixed IPA proof and BGH19 Poseidon transcript.
pub(crate) const OFFLINE_CASH_IPA_POSEIDON_FULL_ROUNDS_V1: usize = 8;
/// Partial rounds of the fixed IPA proof and BGH19 Poseidon transcript.
pub(crate) const OFFLINE_CASH_IPA_POSEIDON_PARTIAL_ROUNDS_V1: usize = 57;
/// Secure-MDS search selector of the fixed IPA proof and BGH19 Poseidon transcript.
pub(crate) const OFFLINE_CASH_IPA_POSEIDON_SECURE_MDS_V1: usize = 0;
/// Exact BGH19 fold transcript bytes for one `k = 16` parity.
pub const OFFLINE_CASH_IPA_FOLD_PROOF_BYTES_V1: usize =
    (OFFLINE_CASH_RECURSION_IPA_K_V1 as usize * 2 + 8) * 32;
/// Exact sparse-Merkle path depth needed by `ReceiveFoldBatch`.
pub const OFFLINE_CASH_REPLAY_PATH_DEPTH_V1: usize = 256;

const GUARD_STATEMENT_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:normalized-guard-statement\0";
const HELPER_PROTOCOL_POSEIDON_DOMAIN_V1: u64 = u64::from_le_bytes(*b"ochelp_1");

/// Exact authenticated release roles required by the paired finalized-mint helper.
pub const OFFLINE_CASH_MINT_FINALITY_ARTIFACT_ROLES_V1: [OfflineCashArtifactRoleV1; 4] = [
    OfflineCashArtifactRoleV1::MintCreditPkEq,
    OfflineCashArtifactRoleV1::MintCreditVkEq,
    OfflineCashArtifactRoleV1::MintCreditPkEp,
    OfflineCashArtifactRoleV1::MintCreditVkEp,
];

/// The two non-interchangeable roles in the fixed Pasta recursion cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub enum OfflineCashPastaParityV1 {
    /// Eq/Vesta group with canonical `Fp` accumulator challenges.
    Eq,
    /// Ep/Pallas group with canonical `Fq` accumulator challenges.
    Ep,
}

/// Closed set of fixed-shape Offline Cash V1 recursive relations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub enum OfflineCashOperationV1 {
    /// Establish a hardware-bound zero balance.
    Bootstrap,
    /// Add one finalized reserve-backed mint credit.
    MintFold,
    /// Subtract value and emit one receiver-bound credit.
    SendSplit,
    /// Consume one through sixteen staged credits and update the exact replay root.
    ReceiveFoldBatch,
    /// Subtract value and emit one terminal redemption voucher.
    RedeemSplit,
    /// Bridge the complete private monetary state to a governed successor suite.
    SuiteUpgrade,
    /// Carry the complete balance and replay root to the next hardware epoch.
    Rotate,
}

impl From<OfflineCashTransitionKindV1> for OfflineCashOperationV1 {
    fn from(value: OfflineCashTransitionKindV1) -> Self {
        match value {
            OfflineCashTransitionKindV1::MintFold => Self::MintFold,
            OfflineCashTransitionKindV1::SendSplit => Self::SendSplit,
            OfflineCashTransitionKindV1::ReceiveFoldBatch => Self::ReceiveFoldBatch,
            OfflineCashTransitionKindV1::RedeemSplit => Self::RedeemSplit,
            OfflineCashTransitionKindV1::SuiteUpgrade => Self::SuiteUpgrade,
            OfflineCashTransitionKindV1::Rotate => Self::Rotate,
        }
    }
}

impl From<OfflineCashOperationKindV1> for OfflineCashOperationV1 {
    fn from(value: OfflineCashOperationKindV1) -> Self {
        match value {
            OfflineCashOperationKindV1::Bootstrap => Self::Bootstrap,
            OfflineCashOperationKindV1::MintFold => Self::MintFold,
            OfflineCashOperationKindV1::SendSplit => Self::SendSplit,
            OfflineCashOperationKindV1::ReceiveFoldBatch => Self::ReceiveFoldBatch,
            OfflineCashOperationKindV1::RedeemSplit => Self::RedeemSplit,
            OfflineCashOperationKindV1::SuiteUpgrade => Self::SuiteUpgrade,
            OfflineCashOperationKindV1::Rotate => Self::Rotate,
        }
    }
}

impl From<OfflineCashOperationV1> for OfflineCashOperationKindV1 {
    fn from(value: OfflineCashOperationV1) -> Self {
        match value {
            OfflineCashOperationV1::Bootstrap => Self::Bootstrap,
            OfflineCashOperationV1::MintFold => Self::MintFold,
            OfflineCashOperationV1::SendSplit => Self::SendSplit,
            OfflineCashOperationV1::ReceiveFoldBatch => Self::ReceiveFoldBatch,
            OfflineCashOperationV1::RedeemSplit => Self::RedeemSplit,
            OfflineCashOperationV1::SuiteUpgrade => Self::SuiteUpgrade,
            OfflineCashOperationV1::Rotate => Self::Rotate,
        }
    }
}

/// Release-authenticated inputs which are not yet present in Core's transition statements.
///
/// These values are proof inputs, not host assertions. The governed recursive backend must
/// constrain them through the normalized `GuardBundle` helper proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct OfflineCashGuardContextV1 {
    /// Authenticated Offline Cash proof-release identifier.
    pub release_id: DigestV1,
    /// Deterministic reserve liability pool for this network and asset.
    pub liability_pool_id: DigestV1,
    /// Digest of the exact released lifecycle candidate context.
    pub lifecycle_binding_digest: DigestV1,
    /// Digest of request, ticket, capacity reservation, and prepared authorization.
    pub precommit_binding_digest: DigestV1,
    /// One-use sender authorization successor for a no-commit closure, otherwise zero.
    ///
    /// The successor is private to the Guard relation. Only its unlinkable cancellation
    /// nullifier is exposed by the final CommitWrapper proof.
    pub sender_one_time_authorization_digest: DigestV1,
    /// Governed old-suite to new-suite authorization, nonzero only for `SuiteUpgrade`.
    pub suite_upgrade_authorization_digest: DigestV1,
    /// Number of active credits in the fixed 16-slot receive relation.
    pub receive_active_count: u8,
    /// Digest of the active-prefix receive batch, nonzero only for `ReceiveFoldBatch`.
    pub receive_batch_binding_digest: DigestV1,
    /// Digest of the hardware-sealed transition intent and canonical inputs.
    pub transition_intent_digest: DigestV1,
    /// Digest of the operation-specific effect for bootstrap, or the exact Core effect for a
    /// normal aggregate transition.
    pub transition_effect_digest: DigestV1,
    /// Digest of the terminal crash-recovery record.
    pub recovery_record_digest: DigestV1,
    /// Digest of the durable inbox effect, or the release-authenticated empty digest.
    pub durable_inbox_effect_digest: DigestV1,
    /// Digest of the durable outbox effect, or the release-authenticated empty digest.
    pub durable_outbox_effect_digest: DigestV1,
    /// Exact canonical empty-effect digest fixed by the authenticated release.
    pub canonical_empty_effect_digest: DigestV1,
}

impl OfflineCashGuardContextV1 {
    fn validate(
        self,
        operation: OfflineCashOperationV1,
        amount: u128,
    ) -> Result<(), OfflineCashRecursionErrorV1> {
        if self.release_id == [0; 32]
            || self.liability_pool_id == [0; 32]
            || self.lifecycle_binding_digest == [0; 32]
            || self.transition_intent_digest == [0; 32]
            || self.transition_effect_digest == [0; 32]
            || self.recovery_record_digest == [0; 32]
            || self.canonical_empty_effect_digest == [0; 32]
        {
            return Err(OfflineCashRecursionErrorV1::InvalidGuardContext);
        }

        let is_no_commit_closure = operation == OfflineCashOperationV1::SendSplit && amount == 0;
        let is_receive_batch = operation == OfflineCashOperationV1::ReceiveFoldBatch;
        let uses_outbox = matches!(
            operation,
            OfflineCashOperationV1::SendSplit | OfflineCashOperationV1::RedeemSplit
        );
        if is_receive_batch
            != (self.receive_active_count > 0
                && self.receive_active_count <= OFFLINE_CASH_RECEIVE_FOLD_BATCH_WIDTH_V1
                && self.receive_batch_binding_digest != [0; 32])
            || (!is_receive_batch
                && (self.receive_active_count != 0 || self.receive_batch_binding_digest != [0; 32]))
            || (operation == OfflineCashOperationV1::SuiteUpgrade)
                != (self.suite_upgrade_authorization_digest != [0; 32])
            || is_no_commit_closure != (self.sender_one_time_authorization_digest != [0; 32])
            || uses_outbox != (self.precommit_binding_digest != [0; 32])
        {
            return Err(OfflineCashRecursionErrorV1::InvalidGuardContext);
        }

        let inbox_is_empty = self.durable_inbox_effect_digest == self.canonical_empty_effect_digest;
        let outbox_is_empty =
            self.durable_outbox_effect_digest == self.canonical_empty_effect_digest;
        let inbox_is_present = self.durable_inbox_effect_digest != [0; 32] && !inbox_is_empty;
        let outbox_is_present = self.durable_outbox_effect_digest != [0; 32] && !outbox_is_empty;
        let valid_effects = match operation {
            OfflineCashOperationV1::MintFold | OfflineCashOperationV1::ReceiveFoldBatch => {
                inbox_is_present && outbox_is_empty
            }
            OfflineCashOperationV1::SendSplit | OfflineCashOperationV1::RedeemSplit => {
                inbox_is_empty && outbox_is_present
            }
            OfflineCashOperationV1::Bootstrap
            | OfflineCashOperationV1::SuiteUpgrade
            | OfflineCashOperationV1::Rotate => inbox_is_empty && outbox_is_empty,
        };
        if !valid_effects {
            return Err(OfflineCashRecursionErrorV1::InvalidGuardEffects(operation));
        }
        Ok(())
    }
}

fn normalized_lane_bindings(
    lane: &OfflineCashLaneIdV1,
    asset_incarnation: AxtAssetIncarnationV1,
) -> Result<(DigestV1, DigestV1, DigestV1), OfflineCashRecursionErrorV1> {
    if lane.network_id.as_bytes() == &[0; 32]
        || lane.device_lane_id == [0; 32]
        || lane.scale > OFFLINE_CASH_ASSET_SCALE_MAX_V1
        || asset_incarnation.validate().is_err()
    {
        return Err(OfflineCashRecursionErrorV1::InvalidTransitionStatement);
    }
    let asset_id = lane
        .normalized_asset_id()
        .map_err(|error| OfflineCashRecursionErrorV1::StateStatement(error.to_string()))?;
    let liability_pool_id =
        offline_cash_liability_pool_id_v1(&lane.network_id, &lane.asset, asset_incarnation)
            .map_err(|error| OfflineCashRecursionErrorV1::StateStatement(error.to_string()))?;
    Ok((lane.normalized_network_id(), asset_id, liability_pool_id))
}

/// Fixed semantic statement recursively authenticated by an Offline Cash V1 `GuardBundle`.
///
/// It mirrors `specs/offline_cash_guard_bundle_v1.md`. State nonce fields are opaque 32-byte
/// hiding commitments; raw private nonce material never enters a public statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct OfflineCashNormalizedGuardStatementV1 {
    /// Guard statement version.
    pub version: u16,
    /// Exact offline-cash protocol version.
    pub protocol_version: u16,
    /// Proof suite consumed by this transition.
    pub predecessor_suite_id: DigestV1,
    /// Verifying-key set consumed by this transition.
    pub predecessor_vk_digest: DigestV1,
    /// Proof suite produced by this transition.
    pub successor_suite_id: DigestV1,
    /// Verifying-key set produced by this transition.
    pub successor_vk_digest: DigestV1,
    /// Selected fixed-shape operation.
    pub operation: OfflineCashOperationV1,
    /// Exact monetary amount authorized by hardware; zero only for bootstrap, rotation, suite
    /// upgrade, or the private sender-authorization cancellation branch.
    pub amount: u128,
    /// Receiver-bound credit identity, nonzero only for peer send/receive operations.
    pub peer_credit_id: DigestV1,
    /// Receiver lane carried by the peer credit, nonzero only for peer send/receive operations.
    pub peer_recipient_lane_id: DigestV1,
    /// Exact paired mint-helper proof binding, nonzero only for `MintFold`.
    pub mint_finality_proof_binding_digest: DigestV1,
    /// Authenticated proof-release identifier.
    pub release_id: DigestV1,
    /// Exact raw `NetworkId::as_bytes()` value; it is not rehashed.
    pub network_id: DigestV1,
    /// Canonical typed asset identity digest.
    ///
    /// This is returned by the data-model `offline_cash_asset_identity_digest_v1` helper: SHA-256
    /// of `"iroha:offline-cash:v1:asset-identity" || 0x00 || u64_le(encoded_len) ||
    /// canonical_norito(AssetDefinitionId)`. Core and circuits must not introduce a second asset
    /// hash convention.
    pub asset_id: DigestV1,
    /// Exact asset incarnation.
    pub asset_incarnation: AxtAssetIncarnationV1,
    /// Authoritative decimal scale of the typed asset.
    pub asset_scale: u32,
    /// Exact value returned by the data-model `offline_cash_liability_pool_id_v1` helper for the
    /// same typed network and asset; it is not independently rederived by Core.
    pub liability_pool_id: DigestV1,
    /// Qualified non-forking hardware profile.
    pub hardware_profile_id: DigestV1,
    /// Governed hardware-policy epoch.
    pub policy_epoch: u64,
    /// Stable hardware-controlled lane identity.
    pub lane_id: DigestV1,
    /// Aggregate commitment consumed by the transition.
    pub predecessor_state_commitment: DigestV1,
    /// Aggregate commitment produced by the transition.
    pub successor_state_commitment: DigestV1,
    /// Opaque predecessor state-nonce commitment.
    pub predecessor_state_nonce_commitment: DigestV1,
    /// Opaque successor state-nonce commitment.
    pub successor_state_nonce_commitment: DigestV1,
    /// Consumed logical sequence.
    pub predecessor_logical_sequence: u128,
    /// Exact-next logical sequence.
    pub successor_logical_sequence: u128,
    /// Consumed hardware-epoch generation.
    pub predecessor_hardware_epoch_generation: u128,
    /// Produced hardware-epoch generation.
    pub successor_hardware_epoch_generation: u128,
    /// Consumed hardware-epoch identity.
    pub predecessor_hardware_epoch_id: DigestV1,
    /// Produced hardware-epoch identity.
    pub successor_hardware_epoch_id: DigestV1,
    /// Consumed device-key reference.
    pub predecessor_key_reference: DigestV1,
    /// Produced device-key reference.
    pub successor_key_reference: DigestV1,
    /// Consumed hardware-policy identifier.
    pub predecessor_hardware_policy_id: DigestV1,
    /// Produced hardware-policy identifier.
    pub successor_hardware_policy_id: DigestV1,
    /// Consumed durable journal revision.
    pub journal_revision_before: u128,
    /// Exact-next durable journal revision.
    pub journal_revision_after: u128,
    /// Digest of the exact released lifecycle candidate context.
    pub lifecycle_binding_digest: DigestV1,
    /// Digest of the precommit request, ticket, and capacity reservation.
    pub precommit_binding_digest: DigestV1,
    /// Digest of the terminal hardware commit, canonical envelope, and durable recovery record.
    ///
    /// Candidate statements use the canonical all-zero value. A terminal `SendSplit` or
    /// `RedeemSplit` Guard statement sets this to the nonzero binding recomputed by the wrapper.
    pub terminal_commit_binding_digest: DigestV1,
    /// Digest of the private one-use sender authorization proven by terminal hardware.
    ///
    /// This is nonzero for a terminal `SendSplit` and for the private no-commit cancellation
    /// successor; candidates and every other operation use the canonical all-zero value.
    pub sender_one_time_authorization_digest: DigestV1,
    /// Governed suite-upgrade authorization, nonzero only for `SuiteUpgrade`.
    pub suite_upgrade_authorization_digest: DigestV1,
    /// Number of active entries in the fixed 16-slot receive relation.
    pub receive_active_count: u8,
    /// Digest of the fixed-shape receive batch, nonzero only for `ReceiveFoldBatch`.
    pub receive_batch_binding_digest: DigestV1,
    /// Digest of the hardware-sealed transition intent.
    pub transition_intent_digest: DigestV1,
    /// Digest of the operation-specific transition effect.
    pub transition_effect_digest: DigestV1,
    /// Digest of the terminal crash-recovery record.
    pub recovery_record_digest: DigestV1,
    /// Durable inbox effect or the authenticated empty-effect digest.
    pub durable_inbox_effect_digest: DigestV1,
    /// Durable outbox effect or the authenticated empty-effect digest.
    pub durable_outbox_effect_digest: DigestV1,
}

impl OfflineCashNormalizedGuardStatementV1 {
    /// Whether this is the hardware cancellation branch for a sender authorization.
    #[must_use]
    pub(crate) fn is_no_commit_closure(&self) -> bool {
        self.operation == OfflineCashOperationV1::SendSplit && self.amount == 0
    }

    /// Derive the complete normalized guard relation before constructing its hardware statement.
    ///
    /// # Errors
    ///
    /// Rejects a malformed state statement, non-exact successor, invalid rotation, or
    /// operation-specific durable effect. The resulting canonical digest is installed into the
    /// final hardware statement; no half-authorized certificate is exposed.
    pub fn derive_from_transition(
        proof: &TransitionProofStatementV1,
        context: OfflineCashGuardContextV1,
    ) -> Result<Self, OfflineCashRecursionErrorV1> {
        if proof.version != OFFLINE_CASH_WIRE_VERSION_V1 {
            return Err(OfflineCashRecursionErrorV1::UnsupportedVersion);
        }
        let operation = OfflineCashOperationV1::from(proof.kind);
        let is_no_commit_closure =
            operation == OfflineCashOperationV1::SendSplit && proof.amount == 0;
        let (network_id, asset_id, liability_pool_id) =
            normalized_lane_bindings(&proof.lane, proof.asset_incarnation)?;
        if proof.protocol_version != OFFLINE_CASH_WIRE_VERSION_V1
            || proof.predecessor_suite_id == [0; 32]
            || proof.predecessor_vk_digest == [0; 32]
            || proof.successor_suite_id == [0; 32]
            || proof.successor_vk_digest == [0; 32]
            || proof.release_id == [0; 32]
            || proof.release_id != context.release_id
            || proof.asset_incarnation.validate().is_err()
            || proof.liability_pool_id != liability_pool_id
            || context.liability_pool_id != liability_pool_id
            || proof.hardware_profile_id == [0; 32]
            || proof.policy_epoch == 0
            || proof.lifecycle_binding_digest == [0; 32]
            || proof.lifecycle_binding_digest != context.lifecycle_binding_digest
            || proof.precommit_binding_digest != context.precommit_binding_digest
            || proof.predecessor_commitment == [0; 32]
            || proof.successor_commitment == [0; 32]
            || is_no_commit_closure != (proof.predecessor_commitment == proof.successor_commitment)
            || proof.predecessor_state_nonce_commitment == [0; 32]
            || proof.successor_state_nonce_commitment == [0; 32]
            || is_no_commit_closure
                != (proof.predecessor_state_nonce_commitment
                    == proof.successor_state_nonce_commitment)
            || proof.effect_digest == [0; 32]
            || (matches!(
                operation,
                OfflineCashOperationV1::SuiteUpgrade | OfflineCashOperationV1::Rotate
            ) || is_no_commit_closure)
                != (proof.amount == 0)
            || (operation == OfflineCashOperationV1::MintFold)
                != (proof.mint_finality_semantic_digest != [0; 32])
            || (operation == OfflineCashOperationV1::MintFold)
                != (proof.mint_finality_proof_binding_digest != [0; 32])
        {
            return Err(OfflineCashRecursionErrorV1::InvalidTransitionStatement);
        }
        let is_peer = operation == OfflineCashOperationV1::SendSplit && !is_no_commit_closure;
        if is_peer != (proof.peer_credit_id != [0; 32])
            || is_peer != (proof.peer_recipient_lane_id != [0; 32])
        {
            return Err(OfflineCashRecursionErrorV1::InvalidTransitionStatement);
        }
        let uses_outbox = matches!(
            operation,
            OfflineCashOperationV1::SendSplit | OfflineCashOperationV1::RedeemSplit
        );
        if uses_outbox != (proof.precommit_binding_digest != [0; 32]) {
            return Err(OfflineCashRecursionErrorV1::InvalidTransitionStatement);
        }
        let is_receive_batch = operation == OfflineCashOperationV1::ReceiveFoldBatch;
        if is_receive_batch
            != (proof.receive_active_count > 0
                && proof.receive_active_count <= OFFLINE_CASH_RECEIVE_FOLD_BATCH_WIDTH_V1
                && proof.receive_batch_binding_digest != [0; 32])
            || (!is_receive_batch
                && (proof.receive_active_count != 0
                    || proof.receive_batch_binding_digest != [0; 32]))
            || proof.receive_active_count != context.receive_active_count
            || proof.receive_batch_binding_digest != context.receive_batch_binding_digest
            || (operation == OfflineCashOperationV1::SuiteUpgrade)
                != (proof.suite_upgrade_authorization_digest != [0; 32])
            || proof.suite_upgrade_authorization_digest
                != context.suite_upgrade_authorization_digest
            || (operation == OfflineCashOperationV1::SuiteUpgrade)
                != (proof.predecessor_suite_id != proof.successor_suite_id
                    && proof.predecessor_vk_digest != proof.successor_vk_digest)
            || (operation != OfflineCashOperationV1::SuiteUpgrade
                && (proof.predecessor_suite_id != proof.successor_suite_id
                    || proof.predecessor_vk_digest != proof.successor_vk_digest))
        {
            return Err(OfflineCashRecursionErrorV1::InvalidTransitionStatement);
        }
        let exact_successor = if operation == OfflineCashOperationV1::Rotate {
            proof.successor_sequence == 0 && proof.journal_revision_after == 0
        } else if is_no_commit_closure {
            proof.successor_sequence == proof.predecessor_sequence
                && proof.journal_revision_after
                    == proof
                        .journal_revision_before
                        .checked_add(1)
                        .ok_or(OfflineCashRecursionErrorV1::JournalOverflow)?
        } else {
            proof.successor_sequence
                == proof
                    .predecessor_sequence
                    .checked_add(1)
                    .ok_or(OfflineCashRecursionErrorV1::SequenceOverflow)?
                && proof.journal_revision_after
                    == proof
                        .journal_revision_before
                        .checked_add(1)
                        .ok_or(OfflineCashRecursionErrorV1::JournalOverflow)?
        };
        if !exact_successor {
            return Err(OfflineCashRecursionErrorV1::NonExactSuccessor);
        }
        match operation {
            OfflineCashOperationV1::Rotate => {
                if proof.successor_epoch.generation
                    != proof
                        .predecessor_epoch
                        .generation
                        .checked_add(1)
                        .ok_or(OfflineCashRecursionErrorV1::EpochOverflow)?
                    || proof.successor_epoch.epoch_id == proof.predecessor_epoch.epoch_id
                    || proof.successor_device_policy_binding.device_key_reference
                        == proof.predecessor_device_policy_binding.device_key_reference
                {
                    return Err(OfflineCashRecursionErrorV1::InvalidRotation);
                }
            }
            _ => {
                if proof.successor_epoch != proof.predecessor_epoch
                    || proof.successor_device_policy_binding
                        != proof.predecessor_device_policy_binding
                {
                    return Err(OfflineCashRecursionErrorV1::InvalidRotation);
                }
            }
        }
        context.validate(operation, proof.amount)?;
        if context.transition_effect_digest != proof.effect_digest {
            return Err(OfflineCashRecursionErrorV1::StateHardwareMismatch);
        }

        Ok(Self {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            protocol_version: proof.protocol_version,
            predecessor_suite_id: proof.predecessor_suite_id,
            predecessor_vk_digest: proof.predecessor_vk_digest,
            successor_suite_id: proof.successor_suite_id,
            successor_vk_digest: proof.successor_vk_digest,
            operation,
            amount: proof.amount,
            peer_credit_id: proof.peer_credit_id,
            peer_recipient_lane_id: proof.peer_recipient_lane_id,
            mint_finality_proof_binding_digest: proof.mint_finality_proof_binding_digest,
            release_id: proof.release_id,
            network_id,
            asset_id,
            asset_incarnation: proof.asset_incarnation,
            asset_scale: proof.lane.scale,
            liability_pool_id,
            hardware_profile_id: proof.hardware_profile_id,
            policy_epoch: proof.policy_epoch,
            lane_id: proof.lane.device_lane_id,
            predecessor_state_commitment: proof.predecessor_commitment,
            successor_state_commitment: proof.successor_commitment,
            predecessor_state_nonce_commitment: proof.predecessor_state_nonce_commitment,
            successor_state_nonce_commitment: proof.successor_state_nonce_commitment,
            predecessor_logical_sequence: proof.predecessor_sequence,
            successor_logical_sequence: proof.successor_sequence,
            predecessor_hardware_epoch_generation: proof.predecessor_epoch.generation,
            successor_hardware_epoch_generation: proof.successor_epoch.generation,
            predecessor_hardware_epoch_id: proof.predecessor_epoch.epoch_id,
            successor_hardware_epoch_id: proof.successor_epoch.epoch_id,
            predecessor_key_reference: proof.predecessor_device_policy_binding.device_key_reference,
            successor_key_reference: proof.successor_device_policy_binding.device_key_reference,
            predecessor_hardware_policy_id: proof
                .predecessor_device_policy_binding
                .hardware_policy_id,
            successor_hardware_policy_id: proof.successor_device_policy_binding.hardware_policy_id,
            journal_revision_before: proof.journal_revision_before,
            journal_revision_after: proof.journal_revision_after,
            lifecycle_binding_digest: proof.lifecycle_binding_digest,
            precommit_binding_digest: proof.precommit_binding_digest,
            terminal_commit_binding_digest: [0; 32],
            sender_one_time_authorization_digest: context.sender_one_time_authorization_digest,
            suite_upgrade_authorization_digest: proof.suite_upgrade_authorization_digest,
            receive_active_count: proof.receive_active_count,
            receive_batch_binding_digest: proof.receive_batch_binding_digest,
            transition_intent_digest: context.transition_intent_digest,
            transition_effect_digest: proof.effect_digest,
            recovery_record_digest: context.recovery_record_digest,
            durable_inbox_effect_digest: context.durable_inbox_effect_digest,
            durable_outbox_effect_digest: context.durable_outbox_effect_digest,
        })
    }

    /// Check a completed hardware statement against its Core statement and normalized digest.
    ///
    /// # Errors
    ///
    /// Rejects a structural mismatch, a substituted Core statement digest, or a substituted
    /// normalized GuardBundle statement digest. The hardware GuardBundle helper must still prove
    /// that the qualified device authorized this completed statement.
    pub fn validate_hardware_binding(
        &self,
        proof: &TransitionProofStatementV1,
        hardware: &HardwareTransitionStatementV1,
    ) -> Result<(), OfflineCashRecursionErrorV1> {
        if proof.version != OFFLINE_CASH_WIRE_VERSION_V1
            || hardware.version != OFFLINE_CASH_WIRE_VERSION_V1
        {
            return Err(OfflineCashRecursionErrorV1::UnsupportedVersion);
        }
        if hardware.kind != proof.kind
            || hardware.amount != proof.amount
            || hardware.lane != proof.lane
            || hardware.predecessor_commitment != proof.predecessor_commitment
            || hardware.successor_commitment != proof.successor_commitment
            || hardware.predecessor_sequence != proof.predecessor_sequence
            || hardware.successor_sequence != proof.successor_sequence
            || hardware.predecessor_epoch != proof.predecessor_epoch
            || hardware.successor_epoch != proof.successor_epoch
            || hardware.predecessor_device_policy_binding != proof.predecessor_device_policy_binding
            || hardware.successor_device_policy_binding != proof.successor_device_policy_binding
            || hardware.predecessor_state_nonce_commitment
                != proof.predecessor_state_nonce_commitment
            || hardware.successor_state_nonce_commitment != proof.successor_state_nonce_commitment
            || hardware.journal_revision_before != proof.journal_revision_before
            || hardware.journal_revision_after != proof.journal_revision_after
            || hardware.state_transition_digest
                != proof.digest().map_err(|error| {
                    OfflineCashRecursionErrorV1::StateStatement(error.to_string())
                })?
            || hardware.normalized_guard_statement_digest != self.canonical_digest()?
        {
            return Err(OfflineCashRecursionErrorV1::StateHardwareMismatch);
        }
        Ok(())
    }

    /// Reconcile one completed Core aggregate transition and hardware statement.
    ///
    /// # Errors
    ///
    /// Returns any normalized relation or completed hardware-binding failure. This function does
    /// not verify a platform signature; the recursive verifier must verify the complete helper
    /// proof.
    pub fn from_transition(
        proof: &TransitionProofStatementV1,
        hardware: &HardwareTransitionStatementV1,
        context: OfflineCashGuardContextV1,
    ) -> Result<Self, OfflineCashRecursionErrorV1> {
        let normalized = Self::derive_from_transition(proof, context)?;
        normalized.validate_hardware_binding(proof, hardware)?;
        Ok(normalized)
    }

    /// Normalize Core's separate bootstrap statement as the circuit's canonical base case.
    ///
    /// # Errors
    ///
    /// Rejects an invalid zero-state successor or malformed supplemental guard bindings. The null
    /// predecessor credential/state fields and initial sequence/journal values are circuit-fixed;
    /// they are not host-selectable bypasses.
    pub fn from_bootstrap_state(
        bootstrap: &BootstrapStatementV1,
        context: OfflineCashGuardContextV1,
    ) -> Result<Self, OfflineCashRecursionErrorV1> {
        let (network_id, asset_id, liability_pool_id) =
            normalized_lane_bindings(&bootstrap.lane, bootstrap.asset_incarnation)?;
        if bootstrap.version != OFFLINE_CASH_WIRE_VERSION_V1
            || bootstrap.protocol_version != OFFLINE_CASH_WIRE_VERSION_V1
            || bootstrap.suite_id == [0; 32]
            || bootstrap.vk_digest == [0; 32]
            || bootstrap.release_id == [0; 32]
            || bootstrap.release_id != context.release_id
            || bootstrap.liability_pool_id == [0; 32]
            || bootstrap.liability_pool_id != context.liability_pool_id
            || bootstrap.liability_pool_id != liability_pool_id
            || bootstrap.asset_incarnation.validate().is_err()
            || bootstrap.hardware_profile_id == [0; 32]
            || bootstrap.policy_epoch == 0
            || bootstrap.hardware_epoch.generation == 0
            || bootstrap.hardware_epoch.epoch_id == [0; 32]
            || bootstrap.device_policy_binding.device_key_reference == [0; 32]
            || bootstrap.device_policy_binding.hardware_policy_id == [0; 32]
            || bootstrap.state_nonce_commitment == [0; 32]
            || bootstrap.state_commitment == [0; 32]
        {
            return Err(OfflineCashRecursionErrorV1::InvalidTransitionStatement);
        }
        context.validate(OfflineCashOperationV1::Bootstrap, 0)?;
        Ok(Self {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            protocol_version: bootstrap.protocol_version,
            predecessor_suite_id: [0; 32],
            predecessor_vk_digest: [0; 32],
            successor_suite_id: bootstrap.suite_id,
            successor_vk_digest: bootstrap.vk_digest,
            operation: OfflineCashOperationV1::Bootstrap,
            amount: 0,
            peer_credit_id: [0; 32],
            peer_recipient_lane_id: [0; 32],
            mint_finality_proof_binding_digest: [0; 32],
            release_id: context.release_id,
            network_id,
            asset_id,
            asset_incarnation: bootstrap.asset_incarnation,
            asset_scale: bootstrap.lane.scale,
            liability_pool_id,
            hardware_profile_id: bootstrap.hardware_profile_id,
            policy_epoch: bootstrap.policy_epoch,
            lane_id: bootstrap.lane.device_lane_id,
            predecessor_state_commitment: [0; 32],
            successor_state_commitment: bootstrap.state_commitment,
            predecessor_state_nonce_commitment: [0; 32],
            successor_state_nonce_commitment: bootstrap.state_nonce_commitment,
            predecessor_logical_sequence: 0,
            successor_logical_sequence: 0,
            predecessor_hardware_epoch_generation: 0,
            successor_hardware_epoch_generation: bootstrap.hardware_epoch.generation,
            predecessor_hardware_epoch_id: [0; 32],
            successor_hardware_epoch_id: bootstrap.hardware_epoch.epoch_id,
            predecessor_key_reference: [0; 32],
            successor_key_reference: bootstrap.device_policy_binding.device_key_reference,
            predecessor_hardware_policy_id: [0; 32],
            successor_hardware_policy_id: bootstrap.device_policy_binding.hardware_policy_id,
            journal_revision_before: 0,
            journal_revision_after: 0,
            lifecycle_binding_digest: context.lifecycle_binding_digest,
            precommit_binding_digest: context.precommit_binding_digest,
            terminal_commit_binding_digest: [0; 32],
            sender_one_time_authorization_digest: [0; 32],
            suite_upgrade_authorization_digest: [0; 32],
            receive_active_count: 0,
            receive_batch_binding_digest: [0; 32],
            transition_intent_digest: context.transition_intent_digest,
            transition_effect_digest: context.transition_effect_digest,
            recovery_record_digest: context.recovery_record_digest,
            durable_inbox_effect_digest: context.durable_inbox_effect_digest,
            durable_outbox_effect_digest: context.durable_outbox_effect_digest,
        })
    }

    /// Return the canonical domain-separated fixed-layout digest constrained by both Pasta
    /// parities.
    ///
    /// # Errors
    ///
    /// Returns an error when the normalized statement shape is invalid.
    pub fn canonical_digest(&self) -> Result<DigestV1, OfflineCashRecursionErrorV1> {
        self.validate_shape()?;
        Ok(guard_bundle::normalized_guard_statement_digest_v1(self))
    }

    /// Check an exact typed payment statement against every overlapping normalized guard field.
    ///
    /// This is a prover/local-state preflight. The recursive circuit must enforce the same
    /// equalities; a successful host comparison is not monetary authority.
    ///
    /// # Errors
    ///
    /// Rejects an invalid wire statement or any release, network, asset, scale, pool, lane,
    /// epoch, key, policy, sequence, commitment, trusted-time, or guard-digest substitution.
    pub fn validate_transfer_binding(
        &self,
        statement: &OfflineCashTransferStatementV1,
    ) -> Result<(), OfflineCashRecursionErrorV1> {
        statement
            .validate()
            .map_err(|error| OfflineCashRecursionErrorV1::TransportBinding(error.to_string()))?;
        let lifecycle = &statement.lifecycle;
        let asset_id = offline_cash_asset_identity_digest_v1(&lifecycle.asset)
            .map_err(|error| OfflineCashRecursionErrorV1::TransportBinding(error.to_string()))?;
        if self.operation != OfflineCashOperationV1::SendSplit
            || statement.amount != self.amount
            || statement.version != self.version
            || lifecycle.operation_kind != OfflineCashOperationKindV1::SendSplit
            || lifecycle.credit_id != self.peer_credit_id
            || lifecycle.release_id != self.release_id
            || lifecycle.network_id.as_bytes() != &self.network_id
            || asset_id != self.asset_id
            || lifecycle.asset_incarnation != self.asset_incarnation
            || lifecycle.scale != self.asset_scale
            || lifecycle.liability_pool_id != self.liability_pool_id
            || lifecycle.hardware_profile_id != self.hardware_profile_id
            || lifecycle.policy_epoch != self.policy_epoch
            || lifecycle.suite_id != self.successor_suite_id
            || lifecycle.vk_digest != self.successor_vk_digest
            || lifecycle
                .canonical_digest()
                .map_err(|error| OfflineCashRecursionErrorV1::TransportBinding(error.to_string()))?
                != self.lifecycle_binding_digest
        {
            return Err(OfflineCashRecursionErrorV1::PublicBindingMismatch);
        }
        Ok(())
    }

    /// Check an exact typed redemption statement against every overlapping normalized guard field.
    ///
    /// This is a prover/local-state preflight. The recursive circuit must enforce the same
    /// equalities; a successful host comparison is not monetary authority.
    ///
    /// # Errors
    ///
    /// Rejects an invalid wire statement or any release, network, asset, scale, pool, lane,
    /// epoch, key, policy, sequence, commitment, trusted-time, or guard-digest substitution.
    pub fn validate_redemption_binding(
        &self,
        statement: &OfflineCashRedemptionStatementV1,
    ) -> Result<(), OfflineCashRecursionErrorV1> {
        statement
            .validate_shape()
            .map_err(|error| OfflineCashRecursionErrorV1::TransportBinding(error.to_string()))?;
        let lifecycle = &statement.lifecycle;
        let asset_id = offline_cash_asset_identity_digest_v1(&lifecycle.asset)
            .map_err(|error| OfflineCashRecursionErrorV1::TransportBinding(error.to_string()))?;
        if self.operation != OfflineCashOperationV1::RedeemSplit
            || statement.amount != self.amount
            || statement.version != self.version
            || lifecycle.operation_kind != OfflineCashOperationKindV1::RedeemSplit
            || lifecycle.release_id != self.release_id
            || lifecycle.network_id.as_bytes() != &self.network_id
            || asset_id != self.asset_id
            || lifecycle.asset_incarnation != self.asset_incarnation
            || lifecycle.scale != self.asset_scale
            || lifecycle.liability_pool_id != self.liability_pool_id
            || lifecycle.hardware_profile_id != self.hardware_profile_id
            || lifecycle.policy_epoch != self.policy_epoch
            || lifecycle.suite_id != self.successor_suite_id
            || lifecycle.vk_digest != self.successor_vk_digest
            || lifecycle
                .canonical_digest()
                .map_err(|error| OfflineCashRecursionErrorV1::TransportBinding(error.to_string()))?
                != self.lifecycle_binding_digest
        {
            return Err(OfflineCashRecursionErrorV1::PublicBindingMismatch);
        }
        Ok(())
    }

    fn validate_shape(&self) -> Result<(), OfflineCashRecursionErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.protocol_version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.successor_suite_id == [0; 32]
            || self.successor_vk_digest == [0; 32]
            || self.release_id == [0; 32]
            || self.network_id == [0; 32]
            || self.asset_id == [0; 32]
            || self.asset_incarnation.validate().is_err()
            || self.asset_scale > OFFLINE_CASH_ASSET_SCALE_MAX_V1
            || self.liability_pool_id == [0; 32]
            || self.hardware_profile_id == [0; 32]
            || self.policy_epoch == 0
            || self.lane_id == [0; 32]
            || self.successor_state_commitment == [0; 32]
            || self.successor_state_nonce_commitment == [0; 32]
            || self.successor_hardware_epoch_generation == 0
            || self.successor_hardware_epoch_id == [0; 32]
            || self.successor_key_reference == [0; 32]
            || self.successor_hardware_policy_id == [0; 32]
            || self.lifecycle_binding_digest == [0; 32]
            || self.transition_intent_digest == [0; 32]
            || self.transition_effect_digest == [0; 32]
            || self.recovery_record_digest == [0; 32]
            || self.durable_inbox_effect_digest == [0; 32]
            || self.durable_outbox_effect_digest == [0; 32]
        {
            return Err(OfflineCashRecursionErrorV1::InvalidTransitionStatement);
        }
        let is_no_commit_closure = self.is_no_commit_closure();
        if !is_no_commit_closure
            && matches!(
                self.operation,
                OfflineCashOperationV1::Bootstrap
                    | OfflineCashOperationV1::SuiteUpgrade
                    | OfflineCashOperationV1::Rotate
            ) != (self.amount == 0)
        {
            return Err(OfflineCashRecursionErrorV1::InvalidTransitionStatement);
        }
        let is_peer = self.operation == OfflineCashOperationV1::SendSplit && !is_no_commit_closure;
        if is_peer != (self.peer_credit_id != [0; 32])
            || is_peer != (self.peer_recipient_lane_id != [0; 32])
        {
            return Err(OfflineCashRecursionErrorV1::InvalidTransitionStatement);
        }
        let is_receive_batch = self.operation == OfflineCashOperationV1::ReceiveFoldBatch;
        let uses_outbox = matches!(
            self.operation,
            OfflineCashOperationV1::SendSplit | OfflineCashOperationV1::RedeemSplit
        );
        let is_terminal = self.terminal_commit_binding_digest != [0; 32];
        let has_sender_authorization = self.sender_one_time_authorization_digest != [0; 32];
        if is_receive_batch
            != (self.receive_active_count > 0
                && self.receive_active_count <= OFFLINE_CASH_RECEIVE_FOLD_BATCH_WIDTH_V1
                && self.receive_batch_binding_digest != [0; 32])
            || (!is_receive_batch
                && (self.receive_active_count != 0 || self.receive_batch_binding_digest != [0; 32]))
            || (self.operation == OfflineCashOperationV1::SuiteUpgrade)
                != (self.suite_upgrade_authorization_digest != [0; 32])
            || (self.operation == OfflineCashOperationV1::SuiteUpgrade)
                != (self.predecessor_suite_id != self.successor_suite_id
                    && self.predecessor_vk_digest != self.successor_vk_digest)
            || (self.operation != OfflineCashOperationV1::SuiteUpgrade
                && self.operation != OfflineCashOperationV1::Bootstrap
                && (self.predecessor_suite_id != self.successor_suite_id
                    || self.predecessor_vk_digest != self.successor_vk_digest))
            || uses_outbox != (self.precommit_binding_digest != [0; 32])
            || (is_terminal && !uses_outbox)
            || has_sender_authorization
                != ((is_terminal && self.operation == OfflineCashOperationV1::SendSplit)
                    || is_no_commit_closure)
            || (is_no_commit_closure && is_terminal)
        {
            return Err(OfflineCashRecursionErrorV1::InvalidTransitionStatement);
        }
        if (self.operation == OfflineCashOperationV1::MintFold)
            != (self.mint_finality_proof_binding_digest != [0; 32])
        {
            return Err(OfflineCashRecursionErrorV1::InvalidTransitionStatement);
        }
        if self.operation == OfflineCashOperationV1::Bootstrap {
            if self.predecessor_state_commitment != [0; 32]
                || self.predecessor_state_nonce_commitment != [0; 32]
                || self.predecessor_logical_sequence != 0
                || self.successor_logical_sequence != 0
                || self.predecessor_hardware_epoch_generation != 0
                || self.predecessor_hardware_epoch_id != [0; 32]
                || self.predecessor_key_reference != [0; 32]
                || self.predecessor_hardware_policy_id != [0; 32]
                || self.predecessor_suite_id != [0; 32]
                || self.predecessor_vk_digest != [0; 32]
                || self.journal_revision_before != 0
                || self.journal_revision_after != 0
            {
                return Err(OfflineCashRecursionErrorV1::InvalidTransitionStatement);
            }
            return Ok(());
        }
        let exact_successor = if self.operation == OfflineCashOperationV1::Rotate {
            self.successor_logical_sequence == 0 && self.journal_revision_after == 0
        } else if is_no_commit_closure {
            self.successor_logical_sequence == self.predecessor_logical_sequence
                && self.journal_revision_after
                    == self
                        .journal_revision_before
                        .checked_add(1)
                        .ok_or(OfflineCashRecursionErrorV1::JournalOverflow)?
        } else {
            self.successor_logical_sequence
                == self
                    .predecessor_logical_sequence
                    .checked_add(1)
                    .ok_or(OfflineCashRecursionErrorV1::SequenceOverflow)?
                && self.journal_revision_after
                    == self
                        .journal_revision_before
                        .checked_add(1)
                        .ok_or(OfflineCashRecursionErrorV1::JournalOverflow)?
        };
        if self.predecessor_state_commitment == [0; 32]
            || is_no_commit_closure
                != (self.predecessor_state_commitment == self.successor_state_commitment)
            || self.predecessor_state_nonce_commitment == [0; 32]
            || is_no_commit_closure
                != (self.predecessor_state_nonce_commitment
                    == self.successor_state_nonce_commitment)
            || self.predecessor_hardware_epoch_generation == 0
            || self.predecessor_hardware_epoch_id == [0; 32]
            || self.predecessor_key_reference == [0; 32]
            || self.predecessor_hardware_policy_id == [0; 32]
            || self.predecessor_suite_id == [0; 32]
            || self.predecessor_vk_digest == [0; 32]
            || !exact_successor
        {
            return Err(OfflineCashRecursionErrorV1::InvalidTransitionStatement);
        }
        Ok(())
    }

    fn validate_release_effects(
        &self,
        canonical_empty_effect_digest: DigestV1,
    ) -> Result<(), OfflineCashRecursionErrorV1> {
        if canonical_empty_effect_digest == [0; 32] {
            return Err(OfflineCashRecursionErrorV1::InvalidArtifacts);
        }
        let inbox_is_empty = self.durable_inbox_effect_digest == canonical_empty_effect_digest;
        let outbox_is_empty = self.durable_outbox_effect_digest == canonical_empty_effect_digest;
        let inbox_is_present = self.durable_inbox_effect_digest != [0; 32] && !inbox_is_empty;
        let outbox_is_present = self.durable_outbox_effect_digest != [0; 32] && !outbox_is_empty;
        let valid = match self.operation {
            OfflineCashOperationV1::MintFold | OfflineCashOperationV1::ReceiveFoldBatch => {
                inbox_is_present && outbox_is_empty
            }
            OfflineCashOperationV1::SendSplit | OfflineCashOperationV1::RedeemSplit => {
                inbox_is_empty && outbox_is_present
            }
            OfflineCashOperationV1::Bootstrap
            | OfflineCashOperationV1::SuiteUpgrade
            | OfflineCashOperationV1::Rotate => inbox_is_empty && outbox_is_empty,
        };
        if !valid {
            return Err(OfflineCashRecursionErrorV1::InvalidGuardEffects(
                self.operation,
            ));
        }
        Ok(())
    }
}

/// Unlinkable public outputs exposed by the final commit-wrapper parities.
///
/// Aggregate state heads, stable lane and credential identities, logical sequence, hardware
/// epoch, and journal position are intentionally absent. They remain private witnesses in the
/// prepared transition and terminal hardware-commit relations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineCashRecursivePublicOutputV1 {
    /// Complete authenticated lifecycle projection.
    pub lifecycle: OfflineCashLifecycleBindingV1,
    /// Digest of the exact public payment, redemption, or internal transition statement.
    pub semantic_digest: DigestV1,
    /// Digest of the durably persisted prepared candidate.
    pub candidate_envelope_digest: DigestV1,
    /// Digest of the hardware terminal commit certificate.
    pub commit_certificate_digest: DigestV1,
    /// Proof-derived transition or terminal nullifier.
    pub transition_nullifier: DigestV1,
    /// Exact receiver request digest for `SendSplit`, otherwise zero.
    pub request_digest: DigestV1,
    /// Exact one-use acceptance-ticket digest for `SendSplit`, otherwise zero.
    pub acceptance_ticket_digest: DigestV1,
    /// Amount-bound encrypted-credit commitment for `SendSplit`, otherwise zero.
    pub ciphertext_commitment: DigestV1,
    /// Monetary amount changed by the operation.
    pub amount: u128,
    /// Operation-specific terminal output binding. A send commits the receiver credit and lane;
    /// a redemption carries its terminal redemption commitment.
    pub terminal_output_binding: DigestV1,
}

impl OfflineCashRecursivePublicOutputV1 {
    /// Construct and validate the sole unlinkable public transition projection.
    ///
    /// # Errors
    ///
    /// Rejects malformed lifecycle data, zero authority bindings, operation-specific field
    /// substitution, or noncanonical zero padding.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        lifecycle: OfflineCashLifecycleBindingV1,
        semantic_digest: DigestV1,
        candidate_envelope_digest: DigestV1,
        commit_certificate_digest: DigestV1,
        transition_nullifier: DigestV1,
        request_digest: DigestV1,
        acceptance_ticket_digest: DigestV1,
        ciphertext_commitment: DigestV1,
        amount: u128,
        terminal_output_binding: DigestV1,
    ) -> Result<Self, OfflineCashRecursionErrorV1> {
        let output = Self {
            lifecycle,
            semantic_digest,
            candidate_envelope_digest,
            commit_certificate_digest,
            transition_nullifier,
            request_digest,
            acceptance_ticket_digest,
            ciphertext_commitment,
            amount,
            terminal_output_binding,
        };
        output.validate()?;
        Ok(output)
    }

    /// Return the selected fixed-shape operation.
    #[must_use]
    pub fn operation(&self) -> OfflineCashOperationV1 {
        self.lifecycle.operation_kind.into()
    }

    fn validate(&self) -> Result<(), OfflineCashRecursionErrorV1> {
        self.lifecycle
            .validate()
            .map_err(|error| OfflineCashRecursionErrorV1::WireProof(error.to_string()))?;
        if [
            self.semantic_digest,
            self.candidate_envelope_digest,
            self.commit_certificate_digest,
            self.transition_nullifier,
        ]
        .into_iter()
        .any(|digest| digest == [0; 32])
            || self.candidate_envelope_digest == self.commit_certificate_digest
        {
            return Err(OfflineCashRecursionErrorV1::InvalidPublicOutput);
        }
        let payment_bindings = [
            self.request_digest,
            self.acceptance_ticket_digest,
            self.ciphertext_commitment,
        ];
        match self.operation() {
            OfflineCashOperationV1::SendSplit => {
                if self.amount == 0
                    || payment_bindings.into_iter().any(|digest| digest == [0; 32])
                    || self.terminal_output_binding == [0; 32]
                {
                    return Err(OfflineCashRecursionErrorV1::InvalidPublicOutput);
                }
            }
            OfflineCashOperationV1::RedeemSplit => {
                if self.amount == 0
                    || payment_bindings.into_iter().any(|digest| digest != [0; 32])
                    || self.terminal_output_binding == [0; 32]
                {
                    return Err(OfflineCashRecursionErrorV1::InvalidPublicOutput);
                }
            }
            OfflineCashOperationV1::Bootstrap
            | OfflineCashOperationV1::MintFold
            | OfflineCashOperationV1::ReceiveFoldBatch
            | OfflineCashOperationV1::SuiteUpgrade
            | OfflineCashOperationV1::Rotate => {
                return Err(OfflineCashRecursionErrorV1::InvalidPublicOutput);
            }
        }
        Ok(())
    }
}

const INCOMING_PROOF_BINDING_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:incoming-commit-wrapper-binding";

/// Bind an accepted sender wrapper before inserting its credit in a receive batch.
///
/// The digest contains only unlinkable public wrapper outputs. Private sender predecessor and
/// successor heads never enter receiver storage, transport, or public circuit instances.
///
/// # Errors
///
/// Rejects a non-send output or common public-projection substitution. Proof bodies and delayed
/// histories are authenticated by recursive verification and the separately committed envelope.
pub fn offline_cash_incoming_proof_binding_digest_v1(
    output: &OfflineCashRecursivePublicOutputV1,
    proof: &OfflineCashCommitWrapperProofV1,
) -> Result<DigestV1, OfflineCashRecursionErrorV1> {
    output.validate()?;
    if output.operation() != OfflineCashOperationV1::SendSplit
        || proof.version != OFFLINE_CASH_WIRE_VERSION_V1
        || proof.semantic_digest != output.semantic_digest
        || proof.candidate_envelope_digest != output.candidate_envelope_digest
        || proof.commit_certificate_digest != output.commit_certificate_digest
        || proof.eq_protocol_digest == [0; 32]
        || proof.ep_protocol_digest == [0; 32]
        || proof.eq_protocol_digest == proof.ep_protocol_digest
        || proof.eq_deferred_audit == [0; 32]
        || proof.ep_deferred_audit == [0; 32]
        || proof.eq_deferred_audit == proof.ep_deferred_audit
        || proof.eq_proof.is_empty()
        || proof.ep_proof.is_empty()
        || proof.eq_proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
        || proof.ep_proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
        || proof.eq_proof.len() + proof.ep_proof.len() > OFFLINE_CASH_CURRENT_PROOFS_MAX_BYTES_V1
        || proof.eq_history.len() != OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1
        || proof.ep_history.len() != OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1
        || crate::zk::offline_cash_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fp>(
            proof.eq_deferred_audit,
        )
        .is_none()
        || crate::zk::offline_cash_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fq>(
            proof.ep_deferred_audit,
        )
        .is_none()
    {
        return Err(OfflineCashRecursionErrorV1::PublicBindingMismatch);
    }
    let lifecycle_digest = output
        .lifecycle
        .canonical_digest()
        .map_err(|error| OfflineCashRecursionErrorV1::WireProof(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(INCOMING_PROOF_BINDING_DOMAIN_V1);
    hasher.update([0]);
    for digest in [
        lifecycle_digest,
        output.semantic_digest,
        output.candidate_envelope_digest,
        output.commit_certificate_digest,
        output.transition_nullifier,
        output.request_digest,
        output.acceptance_ticket_digest,
        output.ciphertext_commitment,
        output.terminal_output_binding,
        proof.eq_protocol_digest,
        proof.ep_protocol_digest,
        proof.eq_deferred_audit,
        proof.ep_deferred_audit,
    ] {
        hasher.update(digest);
    }
    hasher.update(output.amount.to_le_bytes());
    Ok(hasher.finalize().into())
}

/// Exact four-role artifact set for the paired finalized-mint helper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfflineCashMintFinalityArtifactsV1 {
    /// Eq helper proving key binding (`MintCreditPkEq`).
    pub proving_key_eq: OfflineCashArtifactBindingV1,
    /// Eq helper verifying key binding (`MintCreditVkEq`).
    pub verifying_key_eq: OfflineCashArtifactBindingV1,
    /// Ep helper proving key binding (`MintCreditPkEp`).
    pub proving_key_ep: OfflineCashArtifactBindingV1,
    /// Ep helper verifying key binding (`MintCreditVkEp`).
    pub verifying_key_ep: OfflineCashArtifactBindingV1,
}

impl OfflineCashMintFinalityArtifactsV1 {
    /// Resolve the four non-state helper roles from an already authenticated release.
    #[must_use]
    pub fn from_authenticated_release(release: &OfflineCashAuthenticatedReleaseV1) -> Self {
        Self {
            proving_key_eq: release.artifact(OfflineCashArtifactRoleV1::MintCreditPkEq),
            verifying_key_eq: release.artifact(OfflineCashArtifactRoleV1::MintCreditVkEq),
            proving_key_ep: release.artifact(OfflineCashArtifactRoleV1::MintCreditPkEp),
            verifying_key_ep: release.artifact(OfflineCashArtifactRoleV1::MintCreditVkEp),
        }
    }

    fn validate(self) -> Result<(), OfflineCashRecursionErrorV1> {
        let bindings = [
            self.proving_key_eq,
            self.verifying_key_eq,
            self.proving_key_ep,
            self.verifying_key_ep,
        ];
        if bindings
            .iter()
            .zip(OFFLINE_CASH_MINT_FINALITY_ARTIFACT_ROLES_V1)
            .any(|(binding, role)| {
                let max = match role {
                    OfflineCashArtifactRoleV1::MintCreditPkEq
                    | OfflineCashArtifactRoleV1::MintCreditPkEp => {
                        OFFLINE_CASH_HELPER_PROVING_KEY_MAX_BYTES_V1
                    }
                    OfflineCashArtifactRoleV1::MintCreditVkEq
                    | OfflineCashArtifactRoleV1::MintCreditVkEp => {
                        OFFLINE_CASH_VERIFYING_KEY_MAX_BYTES_V1
                    }
                    _ => 0,
                };
                binding.role != role
                    || binding.sha256 == [0; 32]
                    || binding.byte_len == 0
                    || binding.byte_len > max
            })
        {
            return Err(OfflineCashRecursionErrorV1::InvalidArtifacts);
        }
        Ok(())
    }
}

/// Trusted content-addressed artifacts for the sole Offline Cash V1 proof release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfflineCashRecursionArtifactsV1 {
    /// Authenticated release identifier.
    pub release_id: DigestV1,
    /// Release-pinned circuit compilation profile.
    pub profile_digest: DigestV1,
    /// Exact Eq circuit and compiled-protocol digest.
    pub eq_protocol_digest: DigestV1,
    /// Exact Ep circuit and compiled-protocol digest.
    pub ep_protocol_digest: DigestV1,
    /// Exact Eq terminal commit-wrapper compiled-protocol digest.
    pub commit_wrapper_eq_protocol_digest: DigestV1,
    /// Exact Ep terminal commit-wrapper compiled-protocol digest.
    pub commit_wrapper_ep_protocol_digest: DigestV1,
    /// Exact authenticated Eq mint-authorization compiled-protocol digest.
    pub mint_authorization_eq_protocol_digest: DigestV1,
    /// Exact authenticated Ep mint-authorization compiled-protocol digest.
    pub mint_authorization_ep_protocol_digest: DigestV1,
    /// Exact authenticated Eq normalized `GuardBundle` compiled-protocol digest.
    pub guard_bundle_eq_protocol_digest: DigestV1,
    /// Exact authenticated Ep normalized `GuardBundle` compiled-protocol digest.
    pub guard_bundle_ep_protocol_digest: DigestV1,
    /// Exact Eq normalized `GuardBundle` verifying-key binding.
    pub guard_bundle_verifying_key_eq: OfflineCashArtifactBindingV1,
    /// Exact Ep normalized `GuardBundle` verifying-key binding.
    pub guard_bundle_verifying_key_ep: OfflineCashArtifactBindingV1,
    /// Exact Eq terminal commit-wrapper verifying-key binding.
    pub commit_wrapper_verifying_key_eq: OfflineCashArtifactBindingV1,
    /// Exact Ep terminal commit-wrapper verifying-key binding.
    pub commit_wrapper_verifying_key_ep: OfflineCashArtifactBindingV1,
    /// Four distinct finalized-mint helper artifact roles.
    pub mint_finality: OfflineCashMintFinalityArtifactsV1,
    /// Digest of the complete authenticated release manifest.
    pub artifact_manifest_digest: DigestV1,
    /// Canonical empty inbox/outbox effect digest fixed by this release.
    pub canonical_empty_effect_digest: DigestV1,
}

impl OfflineCashRecursionArtifactsV1 {
    /// Construct the recursion artifact seam from one already authenticated release.
    #[must_use]
    pub fn from_authenticated_release(
        release: &OfflineCashAuthenticatedReleaseV1,
        canonical_empty_effect_digest: DigestV1,
    ) -> Self {
        let mint_authorization = release
            .helper_protocol(OfflineCashQualifiedHelperCircuitV1::MintAuthorization)
            .expect("authenticated Offline Cash release has every helper protocol");
        let guard_bundle = release
            .helper_protocol(OfflineCashQualifiedHelperCircuitV1::GuardBundle)
            .expect("authenticated Offline Cash release has every helper protocol");
        Self {
            release_id: release.release_id(),
            profile_digest: release.profile_digest(),
            eq_protocol_digest: release.eq_protocol_digest(),
            ep_protocol_digest: release.ep_protocol_digest(),
            commit_wrapper_eq_protocol_digest: release.commit_wrapper_eq_protocol_digest(),
            commit_wrapper_ep_protocol_digest: release.commit_wrapper_ep_protocol_digest(),
            mint_authorization_eq_protocol_digest: mint_authorization.eq_protocol_digest,
            mint_authorization_ep_protocol_digest: mint_authorization.ep_protocol_digest,
            guard_bundle_eq_protocol_digest: guard_bundle.eq_protocol_digest,
            guard_bundle_ep_protocol_digest: guard_bundle.ep_protocol_digest,
            guard_bundle_verifying_key_eq: release
                .artifact(OfflineCashArtifactRoleV1::GuardBundleVkEq),
            guard_bundle_verifying_key_ep: release
                .artifact(OfflineCashArtifactRoleV1::GuardBundleVkEp),
            commit_wrapper_verifying_key_eq: release
                .artifact(OfflineCashArtifactRoleV1::CommitWrapperVkEq),
            commit_wrapper_verifying_key_ep: release
                .artifact(OfflineCashArtifactRoleV1::CommitWrapperVkEp),
            mint_finality: OfflineCashMintFinalityArtifactsV1::from_authenticated_release(release),
            artifact_manifest_digest: release.manifest_digest(),
            canonical_empty_effect_digest,
        }
    }

    fn validate(self) -> Result<(), OfflineCashRecursionErrorV1> {
        self.mint_finality.validate()?;
        let guard_bindings = [
            (
                self.guard_bundle_verifying_key_eq,
                OfflineCashArtifactRoleV1::GuardBundleVkEq,
            ),
            (
                self.guard_bundle_verifying_key_ep,
                OfflineCashArtifactRoleV1::GuardBundleVkEp,
            ),
        ];
        let wrapper_bindings = [
            (
                self.commit_wrapper_verifying_key_eq,
                OfflineCashArtifactRoleV1::CommitWrapperVkEq,
            ),
            (
                self.commit_wrapper_verifying_key_ep,
                OfflineCashArtifactRoleV1::CommitWrapperVkEp,
            ),
        ];
        if self.release_id == [0; 32]
            || self.profile_digest == [0; 32]
            || self.eq_protocol_digest == [0; 32]
            || self.ep_protocol_digest == [0; 32]
            || self.eq_protocol_digest == self.ep_protocol_digest
            || self.commit_wrapper_eq_protocol_digest == [0; 32]
            || self.commit_wrapper_ep_protocol_digest == [0; 32]
            || self.commit_wrapper_eq_protocol_digest == self.commit_wrapper_ep_protocol_digest
            || self.mint_authorization_eq_protocol_digest == [0; 32]
            || self.mint_authorization_ep_protocol_digest == [0; 32]
            || self.mint_authorization_eq_protocol_digest
                == self.mint_authorization_ep_protocol_digest
            || self.guard_bundle_eq_protocol_digest == [0; 32]
            || self.guard_bundle_ep_protocol_digest == [0; 32]
            || self.guard_bundle_eq_protocol_digest == self.guard_bundle_ep_protocol_digest
            || crate::zk::offline_cash_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fp>(
                self.eq_protocol_digest,
            )
            .is_none()
            || crate::zk::offline_cash_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fq>(
                self.ep_protocol_digest,
            )
            .is_none()
            || crate::zk::offline_cash_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fp>(
                self.commit_wrapper_eq_protocol_digest,
            )
            .is_none()
            || crate::zk::offline_cash_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fq>(
                self.commit_wrapper_ep_protocol_digest,
            )
            .is_none()
            || crate::zk::offline_cash_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fp>(
                self.mint_authorization_eq_protocol_digest,
            )
            .is_none()
            || crate::zk::offline_cash_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fq>(
                self.mint_authorization_ep_protocol_digest,
            )
            .is_none()
            || crate::zk::offline_cash_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fp>(
                self.guard_bundle_eq_protocol_digest,
            )
            .is_none()
            || crate::zk::offline_cash_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fq>(
                self.guard_bundle_ep_protocol_digest,
            )
            .is_none()
            || self.artifact_manifest_digest == [0; 32]
            || self.canonical_empty_effect_digest == [0; 32]
            || guard_bindings.iter().any(|(binding, role)| {
                binding.role != *role
                    || binding.sha256 == [0; 32]
                    || binding.byte_len == 0
                    || binding.byte_len > OFFLINE_CASH_VERIFYING_KEY_MAX_BYTES_V1
            })
            || wrapper_bindings.iter().any(|(binding, role)| {
                binding.role != *role
                    || binding.sha256 == [0; 32]
                    || binding.byte_len == 0
                    || binding.byte_len > OFFLINE_CASH_VERIFYING_KEY_MAX_BYTES_V1
            })
        {
            return Err(OfflineCashRecursionErrorV1::InvalidArtifacts);
        }
        let protocols = [
            self.eq_protocol_digest,
            self.ep_protocol_digest,
            self.commit_wrapper_eq_protocol_digest,
            self.commit_wrapper_ep_protocol_digest,
            self.mint_authorization_eq_protocol_digest,
            self.mint_authorization_ep_protocol_digest,
            self.guard_bundle_protocol_digest(OfflineCashPastaParityV1::Eq)?,
            self.guard_bundle_protocol_digest(OfflineCashPastaParityV1::Ep)?,
            self.mint_finality_protocol_digest(OfflineCashPastaParityV1::Eq)?,
            self.mint_finality_protocol_digest(OfflineCashPastaParityV1::Ep)?,
        ];
        if protocols
            .iter()
            .enumerate()
            .any(|(index, digest)| protocols[index + 1..].contains(digest))
        {
            return Err(OfflineCashRecursionErrorV1::InvalidArtifacts);
        }
        Ok(())
    }

    /// Return the exact release-pinned GuardBundle compiled-protocol identity for one parity.
    ///
    /// # Errors
    ///
    /// Returns an error only to preserve the common helper-protocol accessor shape.
    pub fn guard_bundle_protocol_digest(
        self,
        parity: OfflineCashPastaParityV1,
    ) -> Result<DigestV1, OfflineCashRecursionErrorV1> {
        Ok(match parity {
            OfflineCashPastaParityV1::Eq => self.guard_bundle_eq_protocol_digest,
            OfflineCashPastaParityV1::Ep => self.guard_bundle_ep_protocol_digest,
        })
    }

    /// Derive the exact release-pinned finalized-mint helper protocol identity for one parity.
    ///
    /// The identity commits the compilation profile and the parity-specific
    /// `MintCreditVkEq`/`MintCreditVkEp` artifact, so it cannot alias a state verifying key.
    ///
    /// # Errors
    ///
    /// Returns an error only when canonical artifact binding encoding fails.
    pub fn mint_finality_protocol_digest(
        self,
        parity: OfflineCashPastaParityV1,
    ) -> Result<DigestV1, OfflineCashRecursionErrorV1> {
        let binding = match parity {
            OfflineCashPastaParityV1::Eq => self.mint_finality.verifying_key_eq,
            OfflineCashPastaParityV1::Ep => self.mint_finality.verifying_key_ep,
        };
        helper_protocol_digest(self.profile_digest, binding, parity)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode)]
struct HelperProtocolDigestPreimageV1 {
    profile_digest: DigestV1,
    verifying_key: OfflineCashArtifactBindingV1,
}

fn helper_protocol_digest(
    profile_digest: DigestV1,
    verifying_key: OfflineCashArtifactBindingV1,
    parity: OfflineCashPastaParityV1,
) -> Result<DigestV1, OfflineCashRecursionErrorV1> {
    let encoded = norito::encode_canonical(&HelperProtocolDigestPreimageV1 {
        profile_digest,
        verifying_key,
    })
    .map_err(|error| OfflineCashRecursionErrorV1::Codec(error.to_string()))?;
    let length =
        u128::try_from(encoded.len()).map_err(|_| OfflineCashRecursionErrorV1::LengthOverflow)?;
    let chunks = core::iter::once(length)
        .chain(encoded.chunks(16).map(|chunk| {
            let mut bytes = [0_u8; 16];
            bytes[..chunk.len()].copy_from_slice(chunk);
            u128::from_le_bytes(bytes)
        }))
        .collect::<Vec<_>>();
    match parity {
        OfflineCashPastaParityV1::Eq => {
            use halo2_proofs::halo2curves::pasta::Fp;
            let elements = chunks
                .into_iter()
                .map(crate::zk::offline_cash_v1_poseidon::from_u128::<Fp>)
                .collect::<Vec<_>>();
            Ok(crate::zk::offline_cash_v1_poseidon::encode(
                crate::zk::offline_cash_v1_poseidon::hash(
                    HELPER_PROTOCOL_POSEIDON_DOMAIN_V1,
                    &elements,
                ),
            ))
        }
        OfflineCashPastaParityV1::Ep => {
            use halo2_proofs::halo2curves::pasta::Fq;
            let elements = chunks
                .into_iter()
                .map(crate::zk::offline_cash_v1_poseidon::from_u128::<Fq>)
                .collect::<Vec<_>>();
            Ok(crate::zk::offline_cash_v1_poseidon::encode(
                crate::zk::offline_cash_v1_poseidon::hash(
                    HELPER_PROTOCOL_POSEIDON_DOMAIN_V1,
                    &elements,
                ),
            ))
        }
    }
}

/// Exact one-parity final commit-wrapper request passed to the governed verifier backend.
#[derive(Clone, Copy, Debug)]
pub struct OfflineCashParityVerificationRequestV1<'a> {
    /// Non-interchangeable Eq or Ep role.
    pub parity: OfflineCashPastaParityV1,
    /// Release-pinned commit-wrapper circuit/protocol identity for this parity.
    pub protocol_digest: DigestV1,
    /// Unlinkable common public outputs expected from this parity.
    pub public_output: &'a OfflineCashRecursivePublicOutputV1,
    /// Common Eq deferred-equation audit exposed by both wrapper parities.
    pub eq_deferred_audit: DigestV1,
    /// Common Ep deferred-equation audit exposed by both wrapper parities.
    pub ep_deferred_audit: DigestV1,
    /// Current augmented commit-wrapper proof body for this parity.
    pub current_proof: &'a [u8],
    /// Strictly decoded, canonical 544-byte delayed-history accumulator.
    pub history_accumulator: &'a [u8; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
}

/// Exact paired precommit state-proof request passed to the authenticated native verifier.
#[derive(Clone, Copy, Debug)]
pub struct OfflineCashStateProofVerificationRequestV1<'a> {
    /// Verifier-reconstructed fixed 81-cell state relation projection.
    pub public_inputs: &'a OfflineCashStateRelationPublicInputsV1,
    /// Paired recursive state proof and constant-size histories.
    pub proof: &'a OfflineCashPairedProofV1,
}

/// Exact paired mint-credit/finality helper request used only by `MintFold`.
///
/// The two protocol identities are distinct from the aggregate-state Eq/Ep identities. Their
/// fixed circuits are generated from the certified reserve-receipt ordinary-write relation and
/// caller-pinned consensus-finality relation; they expose only the mint statement digest after
/// verification, so reserve provenance does not accumulate in later aggregate states.
#[derive(Clone, Copy, Debug)]
pub struct OfflineCashMintFinalityHelperVerificationRequestV1<'a> {
    /// Release-pinned Eq mint-finality helper protocol identity.
    pub eq_protocol_digest: DigestV1,
    /// Release-pinned Ep mint-finality helper protocol identity.
    pub ep_protocol_digest: DigestV1,
    /// Canonical mint statement whose digest is constrained by both helper parities.
    pub statement: &'a OfflineCashMintCreditStatementV1,
    /// Exact canonical digest of `statement`.
    pub semantic_digest: DigestV1,
    /// Complete paired mint-finality proof, including both strict history accumulators.
    pub proof: &'a OfflineCashPairedProofV1,
    /// Exact paired certificate digest constrained by both helper parities.
    pub finality_certificate_binding: DigestV1,
    /// Current recursively authenticated roster identifier.
    pub finality_authority_head: DigestV1,
    /// Release-pinned genesis roster identifier.
    pub finality_genesis_roster_id: DigestV1,
    /// Canonical binding of both complete helper public transcripts.
    pub finality_proof_binding_digest: DigestV1,
    /// Release-pinned artifact manifest carried by the mint credit.
    pub artifact_manifest_digest: DigestV1,
}

/// Authenticated native decision for one exact sender no-commit closure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfflineCashNoCommitClosureDecisionV1 {
    statement_digest: DigestV1,
    closure_digest: DigestV1,
    release_id: DigestV1,
    artifact_manifest_digest: DigestV1,
    eq_protocol_digest: DigestV1,
    ep_protocol_digest: DigestV1,
    eq_deferred_audit: DigestV1,
    ep_deferred_audit: DigestV1,
}

impl OfflineCashNoCommitClosureDecisionV1 {
    pub(crate) fn authenticated(closure: &OfflineCashNoCommitClosureV1) -> Result<Self, String> {
        Ok(Self {
            statement_digest: closure
                .statement
                .canonical_digest()
                .map_err(|error| error.to_string())?,
            closure_digest: closure
                .canonical_digest()
                .map_err(|error| error.to_string())?,
            release_id: closure.statement.release_id,
            artifact_manifest_digest: closure.statement.artifact_manifest_digest,
            eq_protocol_digest: closure.proof.eq_protocol_digest,
            ep_protocol_digest: closure.proof.ep_protocol_digest,
            eq_deferred_audit: closure.proof.eq_deferred_audit,
            ep_deferred_audit: closure.proof.ep_deferred_audit,
        })
    }
}

/// Release-authenticated verifier for sender-hardware no-commit closures.
pub trait OfflineCashNoCommitClosureVerifierV1 {
    /// Verify both CommitWrapper parities and their recursively authenticated cancellation Guard.
    fn verify_no_commit_closure(
        &self,
        closure: &OfflineCashNoCommitClosureV1,
    ) -> Result<OfflineCashNoCommitClosureDecisionV1, String>;
}

/// Opaque capability proving one exact ticket can enter and close no-commit recovery.
#[derive(Debug, PartialEq, Eq)]
#[must_use]
pub struct VerifiedOfflineCashNoCommitClosureV1 {
    statement: OfflineCashNoCommitClosureStatementV1,
    statement_digest: DigestV1,
    closure_digest: DigestV1,
}

impl VerifiedOfflineCashNoCommitClosureV1 {
    /// Authenticate the release and both recursive proof parities before minting the capability.
    pub fn verify<V: OfflineCashNoCommitClosureVerifierV1>(
        closure: OfflineCashNoCommitClosureV1,
        verifier: &V,
    ) -> Result<Self, OfflineCashRecursionErrorV1> {
        closure
            .validate_shape()
            .map_err(|error| OfflineCashRecursionErrorV1::WireProof(error.to_string()))?;
        let expected = OfflineCashNoCommitClosureDecisionV1::authenticated(&closure)
            .map_err(OfflineCashRecursionErrorV1::WireProof)?;
        let decision = verifier
            .verify_no_commit_closure(&closure)
            .map_err(OfflineCashRecursionErrorV1::NoCommitClosureProofRejected)?;
        if decision != expected {
            return Err(OfflineCashRecursionErrorV1::PublicBindingMismatch);
        }
        Ok(Self {
            statement: closure.statement,
            statement_digest: expected.statement_digest,
            closure_digest: expected.closure_digest,
        })
    }

    /// Borrow the exact statement authenticated by the paired proof.
    #[must_use]
    pub const fn statement(&self) -> &OfflineCashNoCommitClosureStatementV1 {
        &self.statement
    }

    /// Return the exact statement digest used for durable conflict detection.
    #[must_use]
    pub const fn statement_digest(&self) -> DigestV1 {
        self.statement_digest
    }

    /// Return the digest of the complete authenticated proof envelope.
    #[must_use]
    pub const fn closure_digest(&self) -> DigestV1 {
        self.closure_digest
    }

    /// Consume the capability and return its exact authenticated statement.
    #[must_use]
    pub fn into_statement(self) -> OfflineCashNoCommitClosureStatementV1 {
        self.statement
    }
}

/// Governed recursive verification backend for both Pasta parities.
///
/// Implementations must recursively verify the prepared state proof, normalized hardware guard,
/// exact outbox reservation, and terminal commit certificate before deciding the wrapper history.
/// Host-side signature or certificate checks alone never grant monetary authority.
pub trait OfflineCashRecursiveVerifierV1 {
    /// Verify both release-pinned recursive state parities and decide their carried histories.
    fn verify_state_proof_and_decide(
        &self,
        request: &OfflineCashStateProofVerificationRequestV1<'_>,
    ) -> Result<(), String>;

    /// Verify the distinct paired mint-credit/finality helper proof before constructing a
    /// `MintFold` witness.
    ///
    /// This native preflight is not monetary authority by itself. The `MintFold` state circuit
    /// must recursively verify the same Eq/Ep helper proofs and constrain their common mint
    /// statement digest.
    fn verify_mint_finality_helper(
        &self,
        request: &OfflineCashMintFinalityHelperVerificationRequestV1<'_>,
    ) -> Result<(), String>;

    /// Verify the final commit wrapper, constrain its unlinkable public outputs, and terminally
    /// decide its delayed-history accumulator.
    fn verify_commit_wrapper_and_decide(
        &self,
        request: &OfflineCashParityVerificationRequestV1<'_>,
    ) -> Result<(), String>;
}

/// Explicit fail-closed backend for deployments which have not installed an authenticated
/// Offline Cash proof release.
#[derive(Clone, Copy, Debug, Default)]
pub struct RejectAllOfflineCashRecursiveVerifierV1;

impl OfflineCashRecursiveVerifierV1 for RejectAllOfflineCashRecursiveVerifierV1 {
    fn verify_state_proof_and_decide(
        &self,
        _request: &OfflineCashStateProofVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Err("Offline Cash V1 recursive state verifier is unavailable".to_owned())
    }

    fn verify_mint_finality_helper(
        &self,
        _request: &OfflineCashMintFinalityHelperVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Err("Offline Cash V1 recursive mint-finality verifier is unavailable".to_owned())
    }

    fn verify_commit_wrapper_and_decide(
        &self,
        _request: &OfflineCashParityVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Err("Offline Cash V1 recursive commit-wrapper verifier is unavailable".to_owned())
    }
}

/// Proof of successful verification under both fixed Pasta parity roles.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct VerifiedOfflineCashRecursiveProofV1 {
    output: OfflineCashRecursivePublicOutputV1,
}

/// Opaque chain-admission capability for one exact recursively verified redemption request.
///
/// This token owns the request which was supplied to the governed paired verifier. Reserve code
/// must consume the token through [`Self::into_request`] instead of accepting a structurally
/// validated request alongside a separately supplied boolean or digest.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct VerifiedOfflineCashRedemptionProofV1 {
    request: OfflineCashRedemptionRequestV1,
    request_digest: DigestV1,
    recursive_proof: VerifiedOfflineCashRecursiveProofV1,
}

impl VerifiedOfflineCashRedemptionProofV1 {
    /// Construct an internally bound capability after a reserve test has explicitly mocked both
    /// recursive parities.
    ///
    /// This helper does not exist in production builds. It still validates the complete signed
    /// request and derives the same request/public-output bindings as the production verifier, so
    /// reserve accounting tests cannot accidentally exercise a different request identity.
    #[cfg(test)]
    pub(crate) fn for_reserve_tests_after_mock_recursive_verification(
        request: OfflineCashRedemptionRequestV1,
    ) -> Result<Self, OfflineCashRecursionErrorV1> {
        request
            .validate_shape()
            .map_err(|error| OfflineCashRecursionErrorV1::RedemptionBinding(error.to_string()))?;
        let statement = &request.voucher.statement;
        let semantic_digest = statement
            .canonical_digest()
            .map_err(|error| OfflineCashRecursionErrorV1::RedemptionBinding(error.to_string()))?;
        let output = OfflineCashRecursivePublicOutputV1::new(
            statement.lifecycle.clone(),
            semantic_digest,
            request.voucher.proof.candidate_envelope_digest,
            request.voucher.proof.commit_certificate_digest,
            statement.terminal_nullifier,
            [0; 32],
            [0; 32],
            [0; 32],
            statement.amount,
            statement.redemption_commitment,
        )?;
        let request_digest = request
            .canonical_digest()
            .map_err(|error| OfflineCashRecursionErrorV1::RedemptionBinding(error.to_string()))?;
        Ok(Self {
            request,
            request_digest,
            recursive_proof: VerifiedOfflineCashRecursiveProofV1 { output },
        })
    }

    /// Borrow the exact request whose voucher passed both recursive parities.
    #[must_use]
    pub fn request(&self) -> &OfflineCashRedemptionRequestV1 {
        &self.request
    }

    /// Return the canonical digest of the exact verified chain request.
    #[must_use]
    pub const fn request_digest(&self) -> DigestV1 {
        self.request_digest
    }

    /// Return the common recursive public output authenticated for this request.
    #[must_use]
    pub fn public_output(&self) -> OfflineCashRecursivePublicOutputV1 {
        self.recursive_proof.public_output()
    }

    /// Consume the capability and return the exact verified request for reserve admission.
    #[must_use]
    pub fn into_request(self) -> OfflineCashRedemptionRequestV1 {
        self.request
    }
}

/// Proof that the separate paired mint-finality helper accepted one exact mint statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub struct VerifiedOfflineCashMintFinalityHelperV1 {
    semantic_digest: DigestV1,
    proof_binding_digest: DigestV1,
}

impl VerifiedOfflineCashMintFinalityHelperV1 {
    /// Construct a token after a state test explicitly mocks both mint-finality helper parities.
    ///
    /// This helper is absent from production and still rejects the reserved zero statement
    /// digest, allowing state tests to exercise exact token/credit matching without duplicating
    /// authenticated release fixtures.
    #[cfg(test)]
    pub(crate) fn for_state_tests_after_mock_finality_verification(
        semantic_digest: DigestV1,
    ) -> Result<Self, OfflineCashRecursionErrorV1> {
        if semantic_digest == [0; 32] {
            return Err(OfflineCashRecursionErrorV1::MintFinalityBinding(
                "mint statement digest must be nonzero".to_owned(),
            ));
        }
        Ok(Self {
            semantic_digest,
            proof_binding_digest: [1; 32],
        })
    }

    /// Return the mint statement digest which a `MintFold` circuit must consume.
    #[must_use]
    pub const fn semantic_digest(self) -> DigestV1 {
        self.semantic_digest
    }

    /// Return the exact cross-parity helper binding consumed by the `MintFold` state proof.
    #[must_use]
    pub const fn proof_binding_digest(self) -> DigestV1 {
        self.proof_binding_digest
    }
}

/// Verify the distinct paired mint-credit/finality helper selected by the authenticated release.
///
/// The helper statement is the canonical [`OfflineCashMintCreditStatementV1`] digest. The helper
/// circuits, not the aggregate-state circuit key, prove the certified reserve receipt and pinned
/// block-finality relation which generated that mint statement.
///
/// # Errors
///
/// Rejects a malformed mint credit, statement/proof mismatch, state-VK substitution, manifest
/// substitution, noncanonical accumulator, or governed backend rejection.
pub fn verify_offline_cash_mint_finality_helper_v1<V: OfflineCashRecursiveVerifierV1>(
    verifier: &V,
    artifacts: OfflineCashRecursionArtifactsV1,
    mint_credit: &OfflineCashMintCreditV1,
) -> Result<VerifiedOfflineCashMintFinalityHelperV1, OfflineCashRecursionErrorV1> {
    artifacts.validate()?;
    mint_credit
        .validate_shape()
        .map_err(|error| OfflineCashRecursionErrorV1::MintFinalityBinding(error.to_string()))?;
    let eq_protocol_digest = mint_credit.proof.eq_protocol_digest;
    let ep_protocol_digest = mint_credit.proof.ep_protocol_digest;
    if mint_credit.statement.lifecycle.release_id != artifacts.release_id
        || mint_credit.artifact_manifest_digest != artifacts.artifact_manifest_digest
        || eq_protocol_digest == artifacts.eq_protocol_digest
        || ep_protocol_digest == artifacts.ep_protocol_digest
        || eq_protocol_digest == [0; 32]
        || ep_protocol_digest == [0; 32]
        || eq_protocol_digest == ep_protocol_digest
    {
        return Err(OfflineCashRecursionErrorV1::MintFinalityBinding(
            "release, manifest, or helper protocol identity mismatch".to_owned(),
        ));
    }
    let semantic_digest = mint_credit
        .statement
        .canonical_digest()
        .map_err(|error| OfflineCashRecursionErrorV1::MintFinalityBinding(error.to_string()))?;
    mint_credit
        .proof
        .validate_shape_for_semantic_digest(semantic_digest)
        .map_err(|error| OfflineCashRecursionErrorV1::MintFinalityBinding(error.to_string()))?;
    let eq_history = OfflineCashEqAccumulatorV1::try_from_bytes(&mint_credit.proof.eq_history)?;
    let ep_history = OfflineCashEpAccumulatorV1::try_from_bytes(&mint_credit.proof.ep_history)?;
    if mint_credit.proof.guard_eq_credential_audit != mint_credit.finality_certificate_binding
        || mint_credit.proof.guard_ep_credential_audit != mint_credit.finality_authority_head
    {
        return Err(OfflineCashRecursionErrorV1::MintFinalityBinding(
            "mint helper certificate or authority-head binding mismatch".to_owned(),
        ));
    }
    let proof_binding_digest = mint_authority::OfflineCashMintAuthorityPairBindingV1 {
        step: OfflineCashMintAuthorityStepV1::FinalizedMint,
        semantic_digest,
        amount: mint_credit.statement.amount,
        certificate_binding: mint_credit.finality_certificate_binding,
        authority_head: mint_credit.finality_authority_head,
        release_id: mint_credit.statement.lifecycle.release_id,
        genesis_roster_id: mint_credit.finality_genesis_roster_id,
        eq_protocol_digest,
        ep_protocol_digest,
        eq_deferred_audit: mint_credit.proof.eq_deferred_audit,
        ep_deferred_audit: mint_credit.proof.ep_deferred_audit,
        eq_history: eq_history.as_bytes(),
        ep_history: ep_history.as_bytes(),
    }
    .canonical_digest();
    if proof_binding_digest != mint_credit.finality_proof_binding_digest {
        return Err(OfflineCashRecursionErrorV1::MintFinalityBinding(
            "mint helper paired proof binding mismatch".to_owned(),
        ));
    }
    verifier
        .verify_mint_finality_helper(&OfflineCashMintFinalityHelperVerificationRequestV1 {
            eq_protocol_digest,
            ep_protocol_digest,
            statement: &mint_credit.statement,
            semantic_digest,
            proof: &mint_credit.proof,
            finality_certificate_binding: mint_credit.finality_certificate_binding,
            finality_authority_head: mint_credit.finality_authority_head,
            finality_genesis_roster_id: mint_credit.finality_genesis_roster_id,
            finality_proof_binding_digest: mint_credit.finality_proof_binding_digest,
            artifact_manifest_digest: mint_credit.artifact_manifest_digest,
        })
        .map_err(OfflineCashRecursionErrorV1::MintFinalityProofRejected)?;
    Ok(VerifiedOfflineCashMintFinalityHelperV1 {
        semantic_digest,
        proof_binding_digest,
    })
}

impl VerifiedOfflineCashRecursiveProofV1 {
    /// Return the common public outputs constrained by both accepted parity proofs.
    #[must_use]
    pub fn public_output(&self) -> OfflineCashRecursivePublicOutputV1 {
        self.output.clone()
    }
}

/// Verify a prepared aggregate-state proof against its exact public State+Guard projection.
///
/// # Errors
///
/// Rejects release/protocol substitution, malformed fixed histories, any public projection that
/// does not match the proof envelope, or a backend failure to verify and decide both parities.
pub fn verify_offline_cash_state_proof_v1<V: OfflineCashRecursiveVerifierV1>(
    verifier: &V,
    artifacts: OfflineCashRecursionArtifactsV1,
    public_inputs: &OfflineCashStateRelationPublicInputsV1,
    proof: &OfflineCashPairedProofV1,
) -> Result<(), OfflineCashRecursionErrorV1> {
    artifacts.validate()?;
    proof
        .validate_shape_for_semantic_digest(public_inputs.transport_semantic_digest)
        .map_err(|error| OfflineCashRecursionErrorV1::WireProof(error.to_string()))?;
    if public_inputs.successor.release_id != artifacts.release_id
        || public_inputs.eq_protocol_digest != artifacts.eq_protocol_digest
        || public_inputs.ep_protocol_digest != artifacts.ep_protocol_digest
        || public_inputs.guard_eq_protocol_digest
            != artifacts.guard_bundle_protocol_digest(OfflineCashPastaParityV1::Eq)?
        || public_inputs.guard_ep_protocol_digest
            != artifacts.guard_bundle_protocol_digest(OfflineCashPastaParityV1::Ep)?
        || public_inputs.mint_eq_protocol_digest
            != artifacts.mint_finality_protocol_digest(OfflineCashPastaParityV1::Eq)?
        || public_inputs.mint_ep_protocol_digest
            != artifacts.mint_finality_protocol_digest(OfflineCashPastaParityV1::Ep)?
        || public_inputs.eq_protocol_digest != proof.eq_protocol_digest
        || public_inputs.ep_protocol_digest != proof.ep_protocol_digest
        || public_inputs.guard_eq_credential_audit != proof.guard_eq_credential_audit
        || public_inputs.guard_ep_credential_audit != proof.guard_ep_credential_audit
        || public_inputs.eq_deferred_audit != proof.eq_deferred_audit
        || public_inputs.ep_deferred_audit != proof.ep_deferred_audit
    {
        return Err(OfflineCashRecursionErrorV1::ArtifactSubstitution);
    }
    OfflineCashEqAccumulatorV1::try_from_bytes(&proof.eq_history)?;
    OfflineCashEpAccumulatorV1::try_from_bytes(&proof.ep_history)?;
    verifier
        .verify_state_proof_and_decide(&OfflineCashStateProofVerificationRequestV1 {
            public_inputs,
            proof,
        })
        .map_err(OfflineCashRecursionErrorV1::StateProofRejected)
}

/// Verify a final V1 commit-wrapper pair against release-pinned artifacts and unlinkable outputs.
///
/// # Errors
///
/// Rejects malformed/cross-parity history, self-selected artifacts, substituted public outputs,
/// oversized proof bodies, unavailable verifier hooks, or any backend proof rejection.
pub fn verify_offline_cash_recursive_proof_v1<V: OfflineCashRecursiveVerifierV1>(
    verifier: &V,
    artifacts: OfflineCashRecursionArtifactsV1,
    output: OfflineCashRecursivePublicOutputV1,
    proof: &OfflineCashCommitWrapperProofV1,
) -> Result<VerifiedOfflineCashRecursiveProofV1, OfflineCashRecursionErrorV1> {
    artifacts.validate()?;
    output.validate()?;
    if proof.version != OFFLINE_CASH_WIRE_VERSION_V1
        || proof.semantic_digest != output.semantic_digest
        || proof.candidate_envelope_digest != output.candidate_envelope_digest
        || proof.commit_certificate_digest != output.commit_certificate_digest
        || proof.eq_protocol_digest != artifacts.commit_wrapper_eq_protocol_digest
        || proof.ep_protocol_digest != artifacts.commit_wrapper_ep_protocol_digest
        || proof.eq_deferred_audit == [0; 32]
        || proof.ep_deferred_audit == [0; 32]
        || proof.eq_deferred_audit == proof.ep_deferred_audit
        || proof.eq_proof.is_empty()
        || proof.ep_proof.is_empty()
        || proof.eq_proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
        || proof.ep_proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
        || proof.eq_proof.len() + proof.ep_proof.len() > OFFLINE_CASH_CURRENT_PROOFS_MAX_BYTES_V1
        || crate::zk::offline_cash_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fp>(
            proof.eq_protocol_digest,
        )
        .is_none()
        || crate::zk::offline_cash_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fq>(
            proof.ep_protocol_digest,
        )
        .is_none()
        || crate::zk::offline_cash_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fp>(
            proof.eq_deferred_audit,
        )
        .is_none()
        || crate::zk::offline_cash_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fq>(
            proof.ep_deferred_audit,
        )
        .is_none()
    {
        return Err(OfflineCashRecursionErrorV1::ArtifactSubstitution);
    }
    let eq_history = OfflineCashEqAccumulatorV1::try_from_bytes(&proof.eq_history)?;
    let ep_history = OfflineCashEpAccumulatorV1::try_from_bytes(&proof.ep_history)?;
    let requests = [
        OfflineCashParityVerificationRequestV1 {
            parity: OfflineCashPastaParityV1::Eq,
            protocol_digest: artifacts.commit_wrapper_eq_protocol_digest,
            public_output: &output,
            eq_deferred_audit: proof.eq_deferred_audit,
            ep_deferred_audit: proof.ep_deferred_audit,
            current_proof: &proof.eq_proof,
            history_accumulator: eq_history.as_bytes(),
        },
        OfflineCashParityVerificationRequestV1 {
            parity: OfflineCashPastaParityV1::Ep,
            protocol_digest: artifacts.commit_wrapper_ep_protocol_digest,
            public_output: &output,
            eq_deferred_audit: proof.eq_deferred_audit,
            ep_deferred_audit: proof.ep_deferred_audit,
            current_proof: &proof.ep_proof,
            history_accumulator: ep_history.as_bytes(),
        },
    ];
    for request in &requests {
        verifier
            .verify_commit_wrapper_and_decide(request)
            .map_err(
                |reason| OfflineCashRecursionErrorV1::TransitionProofRejected {
                    parity: request.parity,
                    reason,
                },
            )?;
    }

    Ok(VerifiedOfflineCashRecursiveProofV1 { output })
}

/// Recursively verify and seal one exact chain-facing redemption request.
///
/// Chain execution derives the statement digest and verifies the final wrapper against the
/// authenticated release. Private predecessor, successor, credential, lane, epoch, and journal
/// witnesses never enter the redemption transport.
///
/// The returned opaque capability owns the byte-exact request. Reserve admission should consume
/// it with [`VerifiedOfflineCashRedemptionProofV1::into_request`]; structural
/// [`OfflineCashRedemptionRequestV1::validate`] alone must never reach reserve accounting.
///
/// # Errors
///
/// Rejects an invalid request/signature, wrong authenticated release or artifact manifest,
/// malformed public binding, invalid paired proof, or any governed backend rejection.
pub fn verify_offline_cash_redemption_request_v1<V: OfflineCashRecursiveVerifierV1>(
    verifier: &V,
    artifacts: OfflineCashRecursionArtifactsV1,
    request: OfflineCashRedemptionRequestV1,
) -> Result<VerifiedOfflineCashRedemptionProofV1, OfflineCashRecursionErrorV1> {
    artifacts.validate()?;
    request
        .validate_shape()
        .map_err(|error| OfflineCashRecursionErrorV1::RedemptionBinding(error.to_string()))?;
    let statement = &request.voucher.statement;
    if statement.lifecycle.release_id != artifacts.release_id
        || request.voucher.artifact_manifest_digest != artifacts.artifact_manifest_digest
        || statement.lifecycle.vk_digest == [0; 32]
    {
        return Err(OfflineCashRecursionErrorV1::RedemptionBinding(
            "release or artifact manifest identity mismatch".to_owned(),
        ));
    }
    let semantic_digest = statement
        .canonical_digest()
        .map_err(|error| OfflineCashRecursionErrorV1::RedemptionBinding(error.to_string()))?;
    let output = OfflineCashRecursivePublicOutputV1::new(
        statement.lifecycle.clone(),
        semantic_digest,
        request.voucher.proof.candidate_envelope_digest,
        request.voucher.proof.commit_certificate_digest,
        statement.terminal_nullifier,
        [0; 32],
        [0; 32],
        [0; 32],
        statement.amount,
        statement.redemption_commitment,
    )?;
    let recursive_proof = verify_offline_cash_recursive_proof_v1(
        verifier,
        artifacts,
        output,
        &request.voucher.proof,
    )?;
    let request_digest = request
        .canonical_digest()
        .map_err(|error| OfflineCashRecursionErrorV1::RedemptionBinding(error.to_string()))?;
    Ok(VerifiedOfflineCashRedemptionProofV1 {
        request,
        request_digest,
        recursive_proof,
    })
}

/// Exact private replay-insert witness required by real `MintFold` and `ReceiveFold` circuits.
///
/// Siblings are ordered root-to-leaf: element zero is the sibling below the root and element 255
/// is the sibling of the target leaf. The circuit proves an empty predecessor leaf at `credit_id`,
/// then replaces it with the domain-separated `(credit_id, envelope_digest)` leaf and constrains
/// both roots. This witness is private and is never transported in a payment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineCashReplayInsertWitnessV1 {
    /// Unique finalized mint or staged peer-credit key whose predecessor leaf must be empty.
    pub credit_id: DigestV1,
    /// Digest of the exact staged credit envelope committed by the new leaf.
    pub envelope_digest: DigestV1,
    /// Replay root committed by the consumed aggregate state.
    pub predecessor_root: OfflineCashPastaStateCommitmentV1,
    /// Replay root committed by the produced aggregate state.
    pub successor_root: OfflineCashPastaStateCommitmentV1,
    /// Exact 256 sibling hashes, ordered root-to-leaf.
    pub siblings_root_to_leaf:
        [OfflineCashPastaStateCommitmentV1; OFFLINE_CASH_REPLAY_PATH_DEPTH_V1],
}

impl From<&ConsumedCreditInsertWitnessV1> for OfflineCashReplayInsertWitnessV1 {
    fn from(witness: &ConsumedCreditInsertWitnessV1) -> Self {
        Self {
            credit_id: witness.credit_id.0,
            envelope_digest: witness.envelope_digest,
            predecessor_root: witness.predecessor_root,
            successor_root: witness.successor_root,
            siblings_root_to_leaf: witness.siblings_root_to_leaf,
        }
    }
}

impl OfflineCashReplayInsertWitnessV1 {
    /// Validate fields which do not require evaluating the circuit's fixed sparse-Merkle hash.
    ///
    /// # Errors
    ///
    /// Rejects zero identities/roots and a no-op replay-root transition. This does not establish
    /// nonmembership; only the recursive circuit can do so.
    pub fn validate_shape(&self) -> Result<(), OfflineCashRecursionErrorV1> {
        if self.credit_id == [0; 32]
            || self.envelope_digest == [0; 32]
            || self.predecessor_root.is_zero()
            || self.successor_root.is_zero()
            || self.predecessor_root == self.successor_root
        {
            return Err(OfflineCashRecursionErrorV1::InvalidReplayWitness);
        }
        Ok(())
    }
}

/// Structural, binding, accumulation, or governed-verifier failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum OfflineCashRecursionErrorV1 {
    /// A wire or statement version was not the sole V1 value.
    #[error("unsupported Offline Cash recursion version")]
    UnsupportedVersion,
    /// A canonical field scalar was malformed.
    #[error("non-canonical {parity:?} accumulator scalar at round {round}")]
    NonCanonicalAccumulatorScalar {
        /// Parity whose scalar failed to decode.
        parity: OfflineCashPastaParityV1,
        /// Zero-based IPA round.
        round: usize,
    },
    /// A canonical compressed curve point was malformed or the identity.
    #[error("invalid {0:?} accumulator point")]
    InvalidAccumulatorPoint(OfflineCashPastaParityV1),
    /// A history accumulator had a length other than exactly 544 bytes.
    #[error("invalid {parity:?} accumulator length {actual}; expected {expected}")]
    InvalidAccumulatorLength {
        /// Parity whose accumulator was malformed.
        parity: OfflineCashPastaParityV1,
        /// Observed byte length.
        actual: usize,
        /// Required byte length.
        expected: usize,
    },
    /// A native accumulator did not contain exactly sixteen IPA challenges.
    #[error("invalid {parity:?} native accumulator round count {actual}; expected 16")]
    InvalidAccumulatorRounds {
        /// Parity whose native accumulator was malformed.
        parity: OfflineCashPastaParityV1,
        /// Observed number of challenges.
        actual: usize,
    },
    /// A fold transcript had a non-fixed byte length.
    #[error("invalid {parity:?} fold proof length {actual}; expected {expected}")]
    InvalidFoldProofLength {
        /// Parity whose fold proof was malformed.
        parity: OfflineCashPastaParityV1,
        /// Observed byte length.
        actual: usize,
        /// Required byte length.
        expected: usize,
    },
    /// IPA parameters did not use the fixed `k = 16` profile.
    #[error("invalid {parity:?} IPA parameter exponent {actual}; expected 16")]
    InvalidIpaParameters {
        /// Parity whose parameters were malformed.
        parity: OfflineCashPastaParityV1,
        /// Observed exponent.
        actual: u32,
    },
    /// Native BGH19 proof creation failed.
    #[error("failed to create {parity:?} accumulator fold: {reason}")]
    FoldCreation {
        /// Parity whose fold failed.
        parity: OfflineCashPastaParityV1,
        /// Backend reason.
        reason: String,
    },
    /// Native BGH19 proof verification or terminal decision failed.
    #[error("failed to verify or decide {parity:?} accumulator fold: {reason}")]
    FoldDecision {
        /// Parity whose fold failed.
        parity: OfflineCashPastaParityV1,
        /// Backend reason.
        reason: String,
    },
    /// A verified fold did not produce the claimed successor accumulator.
    #[error("{0:?} fold successor accumulator was substituted")]
    FoldSuccessorSubstitution(OfflineCashPastaParityV1),
    /// A native verifier panicked while rejecting malformed proof material.
    #[error("{0:?} native verifier rejected malformed proof material")]
    NativeVerifierPanic(OfflineCashPastaParityV1),
    /// Supplemental guard fields or fixed effects were malformed.
    #[error("invalid Offline Cash GuardBundle context")]
    InvalidGuardContext,
    /// Operation-specific inbox/outbox effects did not match the fixed relation.
    #[error("invalid Offline Cash GuardBundle effects for {0:?}")]
    InvalidGuardEffects(OfflineCashOperationV1),
    /// State and hardware statements disagreed on an overlapping field.
    #[error("Offline Cash state and hardware statements disagree")]
    StateHardwareMismatch,
    /// A Core state statement could not be canonically processed.
    #[error("Offline Cash Core state statement failed: {0}")]
    StateStatement(String),
    /// Required nonzero transition bindings were absent or aliased.
    #[error("invalid Offline Cash aggregate transition statement")]
    InvalidTransitionStatement,
    /// Logical sequence or journal successor was not exact-next.
    #[error("Offline Cash transition successor is not exact-next")]
    NonExactSuccessor,
    /// Logical sequence increment overflowed.
    #[error("Offline Cash logical sequence overflow")]
    SequenceOverflow,
    /// Journal revision increment overflowed.
    #[error("Offline Cash journal revision overflow")]
    JournalOverflow,
    /// Hardware epoch generation increment overflowed.
    #[error("Offline Cash hardware epoch overflow")]
    EpochOverflow,
    /// Hardware epoch/key rotation violated its fixed relation.
    #[error("invalid Offline Cash hardware rotation")]
    InvalidRotation,
    /// Canonical statement encoding failed.
    #[error("canonical Offline Cash recursion encoding failed: {0}")]
    Codec(String),
    /// A platform length could not be represented in the canonical digest frame.
    #[error("Offline Cash recursion length overflow")]
    LengthOverflow,
    /// Common recursive public outputs were zero or structurally inconsistent.
    #[error("invalid Offline Cash recursive public output")]
    InvalidPublicOutput,
    /// Release-pinned artifact identities were missing or aliased.
    #[error("invalid Offline Cash recursive artifact set")]
    InvalidArtifacts,
    /// Normalized guard and recursive public outputs did not bind identically.
    #[error("Offline Cash recursive public binding mismatch")]
    PublicBindingMismatch,
    /// A typed payment or redemption statement was invalid or did not match its guard statement.
    #[error("invalid Offline Cash typed transport binding: {0}")]
    TransportBinding(String),
    /// Canonical paired-proof validation failed.
    #[error("invalid Offline Cash paired proof: {0}")]
    WireProof(String),
    /// A mint credit did not match its statement, release, distinct helper roles, or manifest.
    #[error("invalid Offline Cash mint-finality helper binding: {0}")]
    MintFinalityBinding(String),
    /// A redemption request did not match its release, manifest, or recursive public instance.
    #[error("invalid Offline Cash redemption proof binding: {0}")]
    RedemptionBinding(String),
    /// The governed paired mint-finality helper verifier rejected.
    #[error("Offline Cash mint-finality helper proof rejected: {0}")]
    MintFinalityProofRejected(String),
    /// The governed paired aggregate-state verifier rejected before terminal commit.
    #[error("Offline Cash aggregate-state proof rejected: {0}")]
    StateProofRejected(String),
    /// The governed paired sender no-commit closure verifier rejected.
    #[error("Offline Cash no-commit closure proof rejected: {0}")]
    NoCommitClosureProofRejected(String),
    /// The proof carried a protocol identity not selected by the trusted release.
    #[error("Offline Cash recursive protocol artifact substitution")]
    ArtifactSubstitution,
    /// Recursive `GuardBundle` helper verification failed.
    #[error("{parity:?} GuardBundle helper proof rejected: {reason}")]
    GuardProofRejected {
        /// Parity whose helper verification failed.
        parity: OfflineCashPastaParityV1,
        /// Governed backend reason.
        reason: String,
    },
    /// Recursive transition/public-output verification or terminal decision failed.
    #[error("{parity:?} transition proof rejected: {reason}")]
    TransitionProofRejected {
        /// Parity whose transition verification failed.
        parity: OfflineCashPastaParityV1,
        /// Governed backend reason.
        reason: String,
    },
    /// A mint/receive replay-insert witness lacked required structural bindings.
    #[error("invalid Offline Cash replay-insert witness")]
    InvalidReplayWitness,
}

const _: () = {
    assert!(OFFLINE_CASH_RECURSION_IPA_K_V1 == 16);
    assert!(OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1 == 544);
    assert!(OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1 == 2_495);
    assert!(OFFLINE_CASH_CURRENT_PROOFS_MAX_BYTES_V1 == 4_990);
    assert!(OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1 == 6_528);
};
