//! Fixed-profile paired-Pasta recursion seam for Kagemusha V1.
//!
//! This module defines the sole Kagemusha V1 aggregate proof stack. It fixes one
//! `k = 16` Eq/Ep cycle, parses the public 544-byte delayed-history accumulators without field
//! reduction, reconciles Core's aggregate-state statements with the normalized hardware guard
//! statement, and exposes a verifier interface which fails closed until the governed recursive
//! circuits are installed. Native BGH19 accumulation and terminal decisions live in
//! [`accumulation`].

mod accumulation;
mod artifacts;
#[cfg(feature = "zk-halo2-ipa")]
mod canonical_preimage;
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
mod mint_hash_claim_fold;
#[cfg(feature = "zk-halo2-ipa")]
mod mint_hash_shard;
#[cfg(feature = "zk-halo2-ipa")]
mod mint_helper;
#[cfg(feature = "zk-halo2-ipa")]
mod mint_transport_decider;
mod native_backend;
mod relation;
mod state_relation;
mod terminal_authorization;
mod transport_decider;

#[cfg(all(
    any(test, feature = "kagemusha-real-proof-harness"),
    feature = "zk-halo2-ipa"
))]
mod real_handoff_qualification_tests;
#[cfg(test)]
pub(crate) mod tests;

pub use accumulation::{
    KagemushaEpAccumulatorV1, KagemushaEpFoldOutputV1, KagemushaEpFoldProofV1,
    KagemushaEqAccumulatorV1, KagemushaEqFoldOutputV1, KagemushaEqFoldProofV1,
    decide_kagemusha_ep_accumulator_v1, decide_kagemusha_eq_accumulator_v1,
    fold_kagemusha_ep_accumulators_v1, fold_kagemusha_eq_accumulators_v1,
    initial_kagemusha_ep_accumulator_v1, initial_kagemusha_eq_accumulator_v1,
    verify_and_decide_kagemusha_ep_fold_v1, verify_and_decide_kagemusha_eq_fold_v1,
};
pub use artifacts::{
    KagemushaArtifactByteResolverV1, KagemushaArtifactDescriptorV1, KagemushaArtifactErrorV1,
    KagemushaArtifactKindV1, KagemushaAuthenticatedArtifactSetV1, KagemushaCircuitFamilyV1,
    KagemushaDirectoryArtifactResolverV1, KagemushaMemoryArtifactResolverV1,
};
#[cfg(all(
    any(test, feature = "kagemusha-real-proof-harness"),
    feature = "zk-halo2-ipa"
))]
pub(crate) use generation::generate_kagemusha_mint_hash_artifacts_for_guarded_test_v1;

/// Run the one real mint-authority proof qualification under the external process memory guard.
///
/// This deliberately narrow entrypoint is present only for the non-shipping dedicated harness
/// feature. It keeps the proof out of Core's monolithic unit-test executable, whose unrelated test
/// code previously dominated compile-time memory.
#[cfg(all(feature = "kagemusha-real-proof-harness", feature = "zk-halo2-ipa"))]
#[doc(hidden)]
pub fn run_guarded_real_mint_authority_proof_v1() {
    real_handoff_qualification_tests::run_guarded_real_mint_authority_proof_v1();
}
pub use generation::{
    KAGEMUSHA_OPERATION_RELATION_SCHEMA_ID_V1, KagemushaArtifactGenerationErrorV1,
    KagemushaGeneratedOperationArtifactsV1,
};
#[cfg(feature = "zk-halo2-ipa")]
pub use generation::{
    KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1,
    KagemushaCommitEvidenceOpeningGenerationV1, KagemushaCommitWrapperEpGenerationWitnessV1,
    KagemushaCommitWrapperEqGenerationWitnessV1, KagemushaCommitWrapperGenerationWitnessV1,
    KagemushaGeneratedCommitWrapperArtifactsV1, KagemushaGeneratedCommitWrapperProofV1,
    KagemushaGeneratedMintAuthorityArtifactsV1, KagemushaGeneratedMintAuthorityProofV1,
    KagemushaGeneratedMintAuthorizationArtifactsV1, KagemushaGeneratedMintAuthorizationProofV1,
    KagemushaGeneratedMintHashArtifactsV1, KagemushaGeneratedMintHashClaimV1,
    KagemushaGeneratedPaymentProofV1, KagemushaGeneratedRecursiveStateArtifactsV1,
    KagemushaGeneratedRecursiveStateProofV1, KagemushaGeneratedRedemptionProofV1,
    KagemushaGeneratedTerminalAuthorizationArtifactsV1,
    KagemushaGeneratedTerminalAuthorizationProofV1, KagemushaLoadedEpCommitWrapperArtifactsV1,
    KagemushaLoadedEpMintAuthorityArtifactsV1, KagemushaLoadedEpMintAuthorizationArtifactsV1,
    KagemushaLoadedEpMintHashArtifactsV1, KagemushaLoadedEpRecursiveStateArtifactsV1,
    KagemushaLoadedEpTerminalAuthorizationArtifactsV1, KagemushaLoadedEqCommitWrapperArtifactsV1,
    KagemushaLoadedEqMintAuthorityArtifactsV1, KagemushaLoadedEqMintAuthorizationArtifactsV1,
    KagemushaLoadedEqMintHashArtifactsV1, KagemushaLoadedEqRecursiveStateArtifactsV1,
    KagemushaLoadedEqTerminalAuthorizationArtifactsV1, KagemushaMintAuthorityGenerationWitnessV1,
    KagemushaMintAuthorizationGenerationWitnessV1, KagemushaMintHashArtifactGenerationWitnessV1,
    KagemushaMintHashClaimGenerationWitnessV1, KagemushaRecursiveIncomingEpGenerationWitnessV1,
    KagemushaRecursiveIncomingEqGenerationWitnessV1, KagemushaRecursiveStateGenerationWitnessV1,
    KagemushaTerminalAuthorizationEpGenerationWitnessV1,
    KagemushaTerminalAuthorizationEqGenerationWitnessV1,
    KagemushaTerminalAuthorizationGenerationWitnessV1,
    KagemushaTerminalAuthorizationPrivateGenerationWitnessV1,
    KagemushaTerminalAuthorizationTerminalGenerationPublicV1,
    KagemushaTerminalSendGenerationWitnessV1, generate_kagemusha_commit_wrapper_artifacts_v1,
    generate_kagemusha_mint_authority_artifacts_v1,
    generate_kagemusha_mint_authorization_artifacts_v1, generate_kagemusha_mint_hash_artifacts_v1,
    generate_kagemusha_recursive_state_artifacts_v1,
    generate_kagemusha_terminal_authorization_artifacts_v1,
    kagemusha_terminal_authorization_enabled_profile_table_v1,
    load_kagemusha_ep_commit_wrapper_artifacts_v1, load_kagemusha_ep_mint_authority_artifacts_v1,
    load_kagemusha_ep_mint_authorization_artifacts_v1, load_kagemusha_ep_mint_hash_artifacts_v1,
    load_kagemusha_ep_recursive_state_artifacts_v1,
    load_kagemusha_ep_terminal_authorization_artifacts_v1,
    load_kagemusha_eq_commit_wrapper_artifacts_v1, load_kagemusha_eq_mint_authority_artifacts_v1,
    load_kagemusha_eq_mint_authorization_artifacts_v1, load_kagemusha_eq_mint_hash_artifacts_v1,
    load_kagemusha_eq_recursive_state_artifacts_v1,
    load_kagemusha_eq_terminal_authorization_artifacts_v1, prove_kagemusha_commit_wrapper_v1,
    prove_kagemusha_finalized_mint_from_checkpoint_v1, prove_kagemusha_mint_authority_bootstrap_v1,
    prove_kagemusha_mint_authority_rotation_from_checkpoint_v1, prove_kagemusha_mint_authority_v1,
    prove_kagemusha_mint_authorization_hash_claim_v1, prove_kagemusha_mint_authorization_v1,
    prove_kagemusha_mint_hash_claim_v1, prove_kagemusha_payment_v1,
    prove_kagemusha_platform_credential_hash_claim_v1,
    prove_kagemusha_recursive_state_hash_claim_v1, prove_kagemusha_recursive_state_v1,
    prove_kagemusha_redemption_v1, prove_kagemusha_terminal_authorization_v1,
};
pub use guard_bundle::{
    KAGEMUSHA_HARDWARE_POLICY_TREE_DEPTH_V1, KagemushaGuardBundleRelationWitnessV1,
    KagemushaPlatformCredentialRelationCircuitV1, KagemushaPlatformCredentialRelationWitnessV1,
    KagemushaPlatformCredentialStatementV1,
};
#[cfg(feature = "zk-halo2-ipa")]
pub use mint_authority::KagemushaMintAuthorityCheckpointV1;
#[cfg(feature = "zk-halo2-ipa")]
pub use mint_authorization::KagemushaMintAuthorizationRelationWitnessV1;
pub use mint_finality::{
    KagemushaMintFinalityErrorV1, KagemushaMintFinalityLocalAuthorityV1,
    KagemushaMintFinalitySignerV1, KagemushaMintFinalityTreeV1,
    build_kagemusha_mint_finality_seal_message_v1, decode_kagemusha_mint_finality_seal_bundle_v1,
    decode_kagemusha_mint_finality_seal_share_v1, derive_kagemusha_mint_finality_validator_keys_v1,
    kagemusha_mint_finality_empty_root_v1, kagemusha_top_up_leaf_from_receipt_v1,
    sign_kagemusha_mint_finality_seal_v1, validate_kagemusha_mint_finality_epoch_v1,
    validate_kagemusha_mint_finality_genesis_parameter_keys_v1,
    validate_kagemusha_mint_finality_roster_keys_v1, verify_kagemusha_mint_finality_seal_bundle_v1,
    verify_kagemusha_mint_finality_seal_share_v1, verify_kagemusha_top_up_membership_v1,
};
#[cfg(feature = "zk-halo2-ipa")]
pub use mint_helper::{KagemushaMintAuthorityStepV1, KagemushaMintCertificateWitnessV1};
pub use native_backend::{
    KagemushaAuthenticatedRecursiveVerifierV1, KagemushaRecursiveVerifierProfileV1,
};
pub use relation::{
    KagemushaOperationRelationCircuitV1, KagemushaOperationRelationConfigV1,
    KagemushaOperationRelationWitnessV1,
};
pub use state_relation::{
    KagemushaReceiveFoldCreditV1, KagemushaStateRelationCircuitV1,
    KagemushaStateRelationPublicInputsV1, KagemushaStateRelationWitnessV1,
    public_instance as kagemusha_state_public_instance_v1,
};
#[cfg(all(test, feature = "zk-halo2-ipa"))]
pub(crate) use terminal_authorization::public_instance as kagemusha_terminal_authorization_public_instance_v1;
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) use terminal_authorization::{
    KagemushaCommitWrapperDeferredAuditsV1, KagemushaCommitWrapperEpCircuitV1,
    KagemushaCommitWrapperEqCircuitV1, KagemushaCommitWrapperWitnessV1,
    KagemushaTerminalAuthorizationDeferredAuditsV1, KagemushaTerminalAuthorizationEpCircuitV1,
    KagemushaTerminalAuthorizationEpWitnessV1, KagemushaTerminalAuthorizationEqCircuitV1,
    KagemushaTerminalAuthorizationEqWitnessV1, KagemushaTerminalAuthorizationWitnessV1,
    TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1, build_kagemusha_commit_wrapper_ep_v1,
    build_kagemusha_commit_wrapper_eq_v1, build_kagemusha_terminal_authorization_ep_v1,
    build_kagemusha_terminal_authorization_eq_v1,
    derive_kagemusha_commit_wrapper_deferred_audits_v1,
    derive_kagemusha_terminal_authorization_deferred_audits_v1,
};
pub(crate) use terminal_authorization::{
    KagemushaTerminalAuthorizationPrivateTransitionV1,
    KagemushaTerminalAuthorizationPublicInputsV1, TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1,
    canonical_prepared_transition_binding_digest_v1, canonical_terminal_send_output_binding_v1,
    kagemusha_candidate_envelope_digest_v1,
};

use iroha_data_model::isi::KagemushaRedemptionRequestV1;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ASSET_SCALE_MAX_V1, KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1,
    KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1, KAGEMUSHA_WIRE_VERSION_V1, KagemushaArtifactBindingV1,
    KagemushaArtifactRoleV1, KagemushaAuthenticatedReleaseV1, KagemushaLifecycleBindingV1,
    KagemushaMintCreditStatementV1, KagemushaMintCreditV1, KagemushaOperationKindV1,
    KagemushaPairedProofV1, KagemushaPastaStateCommitmentV1, KagemushaPaymentRequestV1,
    KagemushaPaymentV1, KagemushaQualifiedHelperCircuitV1, KagemushaQualifiedRelationV1,
    KagemushaRedemptionProofV1, KagemushaRedemptionStatementV1, kagemusha_asset_identity_digest_v1,
    kagemusha_ciphertext_digest_v1, kagemusha_liability_pool_id_v1,
};
pub use iroha_data_model::kagemusha::{
    KAGEMUSHA_CURRENT_PROOFS_MAX_BYTES_V1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1,
    KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1, KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1,
};
use iroha_data_model::nexus::AxtAssetIncarnationV1;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::kagemusha_v1_state::{
    BootstrapStatementV1, ConsumedCreditInsertWitnessV1, DigestV1, HardwareTransitionStatementV1,
    KagemushaLaneIdV1, KagemushaTransitionKindV1, TransitionProofStatementV1,
};

/// Fixed Halo2/IPA domain exponent used by both Kagemusha V1 Pasta parities.
pub const KAGEMUSHA_RECURSION_IPA_K_V1: u32 = 16;
/// Width of the fixed IPA proof and BGH19 Poseidon transcript.
pub(crate) const KAGEMUSHA_IPA_POSEIDON_WIDTH_V1: usize = 3;
/// Rate of the fixed IPA proof and BGH19 Poseidon transcript.
pub(crate) const KAGEMUSHA_IPA_POSEIDON_RATE_V1: usize = 2;
/// Full rounds of the fixed IPA proof and BGH19 Poseidon transcript.
pub(crate) const KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1: usize = 8;
/// Partial rounds of the fixed IPA proof and BGH19 Poseidon transcript.
pub(crate) const KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1: usize = 57;
/// Secure-MDS search selector of the fixed IPA proof and BGH19 Poseidon transcript.
pub(crate) const KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1: usize = 0;
/// Exact BGH19 fold transcript bytes for one `k = 16` parity.
pub const KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1: usize =
    (KAGEMUSHA_RECURSION_IPA_K_V1 as usize * 2 + 8) * 32;
/// Exact sparse-Merkle path depth needed by `ReceiveFold`.
pub const KAGEMUSHA_REPLAY_PATH_DEPTH_V1: usize = 256;

const GUARD_STATEMENT_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:normalized-guard-statement\0";

/// Exact authenticated release roles required by the paired finalized-mint helper.
pub const KAGEMUSHA_MINT_FINALITY_ARTIFACT_ROLES_V1: [KagemushaArtifactRoleV1; 4] = [
    KagemushaArtifactRoleV1::MintCreditPkEq,
    KagemushaArtifactRoleV1::MintCreditVkEq,
    KagemushaArtifactRoleV1::MintCreditPkEp,
    KagemushaArtifactRoleV1::MintCreditVkEp,
];

/// The two non-interchangeable roles in the fixed Pasta recursion cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub enum KagemushaPastaParityV1 {
    /// Eq/Vesta group with canonical `Fp` accumulator challenges.
    Eq,
    /// Ep/Pallas group with canonical `Fq` accumulator challenges.
    Ep,
}

/// Closed set of fixed-shape Kagemusha V1 recursive relations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub enum KagemushaOperationV1 {
    /// Establish a hardware-bound zero balance.
    Bootstrap,
    /// Add one finalized reserve-backed mint credit.
    MintFold,
    /// Subtract value and emit one receiver-bound credit.
    SendSplit,
    /// Consume exactly one staged credit and update the replay root.
    ReceiveFold,
    /// Subtract value and emit one terminal redemption voucher.
    RedeemSplit,
    /// Carry the complete balance and replay root to the next hardware epoch.
    Rotate,
}

impl From<KagemushaTransitionKindV1> for KagemushaOperationV1 {
    fn from(value: KagemushaTransitionKindV1) -> Self {
        match value {
            KagemushaTransitionKindV1::MintFold => Self::MintFold,
            KagemushaTransitionKindV1::SendSplit => Self::SendSplit,
            KagemushaTransitionKindV1::ReceiveFold => Self::ReceiveFold,
            KagemushaTransitionKindV1::RedeemSplit => Self::RedeemSplit,
            KagemushaTransitionKindV1::Rotate => Self::Rotate,
        }
    }
}

impl From<KagemushaOperationKindV1> for KagemushaOperationV1 {
    fn from(value: KagemushaOperationKindV1) -> Self {
        match value {
            KagemushaOperationKindV1::Bootstrap => Self::Bootstrap,
            KagemushaOperationKindV1::MintFold => Self::MintFold,
            KagemushaOperationKindV1::SendSplit => Self::SendSplit,
            KagemushaOperationKindV1::ReceiveFold => Self::ReceiveFold,
            KagemushaOperationKindV1::RedeemSplit => Self::RedeemSplit,
            KagemushaOperationKindV1::Rotate => Self::Rotate,
        }
    }
}

impl From<KagemushaOperationV1> for KagemushaOperationKindV1 {
    fn from(value: KagemushaOperationV1) -> Self {
        match value {
            KagemushaOperationV1::Bootstrap => Self::Bootstrap,
            KagemushaOperationV1::MintFold => Self::MintFold,
            KagemushaOperationV1::SendSplit => Self::SendSplit,
            KagemushaOperationV1::ReceiveFold => Self::ReceiveFold,
            KagemushaOperationV1::RedeemSplit => Self::RedeemSplit,
            KagemushaOperationV1::Rotate => Self::Rotate,
        }
    }
}

/// Release-authenticated inputs which are not yet present in Core's transition statements.
///
/// These values are proof inputs, not host assertions. The governed recursive backend must
/// constrain them through the normalized `GuardBundle` helper proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaGuardContextV1 {
    /// Authenticated Kagemusha proof-release identifier.
    pub release_id: DigestV1,
    /// Deterministic reserve liability pool for this network and asset.
    pub liability_pool_id: DigestV1,
    /// Digest of the exact released lifecycle candidate context.
    pub lifecycle_binding_digest: DigestV1,
    /// Digest of the exact request, receiver binding, encrypted output, and durable outbox intent.
    pub prepared_transition_binding_digest: DigestV1,
    /// Digest of the terminal hardware commit, exact candidate, and recovery record.
    ///
    /// Prepared candidates use zero. Terminal send and redemption guards use the nonzero binding
    /// recomputed by `TerminalAuthorization`.
    pub terminal_commit_binding_digest: DigestV1,
    /// Digest of the private one-use sender authorization.
    ///
    /// This is nonzero only for a committed terminal send transition.
    pub sender_one_time_authorization_digest: DigestV1,
    /// Digest of the single received credit, nonzero only for `ReceiveFold`.
    pub receive_credit_binding_digest: DigestV1,
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

impl KagemushaGuardContextV1 {
    fn validate(
        self,
        operation: KagemushaOperationV1,
        amount: u128,
    ) -> Result<(), KagemushaRecursionErrorV1> {
        if self.release_id == [0; 32]
            || self.liability_pool_id == [0; 32]
            || self.lifecycle_binding_digest == [0; 32]
            || self.transition_intent_digest == [0; 32]
            || self.transition_effect_digest == [0; 32]
            || self.recovery_record_digest == [0; 32]
            || self.canonical_empty_effect_digest == [0; 32]
        {
            return Err(KagemushaRecursionErrorV1::InvalidGuardContext);
        }

        let is_receive = operation == KagemushaOperationV1::ReceiveFold;
        let uses_outbox = matches!(
            operation,
            KagemushaOperationV1::SendSplit | KagemushaOperationV1::RedeemSplit
        );
        if is_receive != (self.receive_credit_binding_digest != [0; 32])
            || uses_outbox != (self.prepared_transition_binding_digest != [0; 32])
            || matches!(
                operation,
                KagemushaOperationV1::Bootstrap | KagemushaOperationV1::Rotate
            ) != (amount == 0)
        {
            return Err(KagemushaRecursionErrorV1::InvalidGuardContext);
        }

        let inbox_is_empty = self.durable_inbox_effect_digest == self.canonical_empty_effect_digest;
        let outbox_is_empty =
            self.durable_outbox_effect_digest == self.canonical_empty_effect_digest;
        let inbox_is_present = self.durable_inbox_effect_digest != [0; 32] && !inbox_is_empty;
        let outbox_is_present = self.durable_outbox_effect_digest != [0; 32] && !outbox_is_empty;
        let valid_effects = match operation {
            KagemushaOperationV1::MintFold | KagemushaOperationV1::ReceiveFold => {
                inbox_is_present && outbox_is_empty
            }
            KagemushaOperationV1::SendSplit | KagemushaOperationV1::RedeemSplit => {
                inbox_is_empty && outbox_is_present
            }
            KagemushaOperationV1::Bootstrap | KagemushaOperationV1::Rotate => {
                inbox_is_empty && outbox_is_empty
            }
        };
        if !valid_effects {
            return Err(KagemushaRecursionErrorV1::InvalidGuardEffects(operation));
        }
        Ok(())
    }
}

fn normalized_lane_bindings(
    lane: &KagemushaLaneIdV1,
    asset_incarnation: AxtAssetIncarnationV1,
) -> Result<(DigestV1, DigestV1, DigestV1), KagemushaRecursionErrorV1> {
    if lane.network_id.as_bytes() == &[0; 32]
        || lane.device_lane_id == [0; 32]
        || lane.scale > KAGEMUSHA_ASSET_SCALE_MAX_V1
        || asset_incarnation.validate().is_err()
    {
        return Err(KagemushaRecursionErrorV1::InvalidTransitionStatement);
    }
    let asset_id = lane
        .normalized_asset_id()
        .map_err(|error| KagemushaRecursionErrorV1::StateStatement(error.to_string()))?;
    let liability_pool_id =
        kagemusha_liability_pool_id_v1(&lane.network_id, &lane.asset, asset_incarnation)
            .map_err(|error| KagemushaRecursionErrorV1::StateStatement(error.to_string()))?;
    Ok((lane.normalized_network_id(), asset_id, liability_pool_id))
}

/// Fixed semantic statement recursively authenticated by an Kagemusha V1 `GuardBundle`.
///
/// It mirrors `specs/kagemusha_guard_bundle_v1.md`. State nonce fields are opaque 32-byte
/// hiding commitments; raw private nonce material never enters a public statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaNormalizedGuardStatementV1 {
    /// Guard statement version.
    pub version: u16,
    /// Exact kagemusha protocol version.
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
    pub operation: KagemushaOperationV1,
    /// Exact monetary amount authorized by hardware; zero for bootstrap and rotate.
    pub amount: u128,
    /// Receiver-bound credit identity, nonzero only for `SendSplit`.
    pub peer_credit_id: DigestV1,
    /// Recipient encryption-key binding carried by the peer credit, nonzero only for
    /// `SendSplit`.
    pub recipient_encryption_key_binding: DigestV1,
    /// Exact paired mint-helper proof binding, nonzero only for `MintFold`.
    pub mint_finality_proof_binding_digest: DigestV1,
    /// Authenticated proof release consumed by the predecessor state.
    ///
    /// This is zero only for `Bootstrap`; otherwise it equals `release_id`.
    pub predecessor_release_id: DigestV1,
    /// Authenticated proof release installed for the successor state.
    pub release_id: DigestV1,
    /// Exact raw `NetworkId::as_bytes()` value; it is not rehashed.
    pub network_id: DigestV1,
    /// Canonical typed asset identity digest.
    ///
    /// This is returned by the data-model `kagemusha_asset_identity_digest_v1` helper: SHA-256
    /// of `"iroha:kagemusha:v1:asset-identity" || 0x00 || u64_le(encoded_len) ||
    /// canonical_norito(AssetDefinitionId)`. Core and circuits must not introduce a second asset
    /// hash convention.
    pub asset_id: DigestV1,
    /// Exact asset incarnation.
    pub asset_incarnation: AxtAssetIncarnationV1,
    /// Authoritative decimal scale of the typed asset.
    pub asset_scale: u32,
    /// Exact value returned by the data-model `kagemusha_liability_pool_id_v1` helper for the
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
    /// Digest of the exact request, receiver binding, encrypted output, and durable outbox intent.
    pub prepared_transition_binding_digest: DigestV1,
    /// Digest binding the terminal hardware commit, exact candidate, and recovery record.
    ///
    /// This is zero for prepared state transitions and nonzero only for terminal wrapper guards.
    pub terminal_commit_binding_digest: DigestV1,
    /// Digest of the private one-use sender authorization.
    ///
    /// This is nonzero only for a committed terminal send transition.
    pub sender_one_time_authorization_digest: DigestV1,
    /// Binding of the single received credit, nonzero only for `ReceiveFold`.
    pub receive_credit_binding_digest: DigestV1,
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

impl KagemushaNormalizedGuardStatementV1 {
    /// Derive the complete normalized guard relation before constructing its hardware statement.
    ///
    /// # Errors
    ///
    /// Rejects a malformed state statement, non-exact successor, invalid rotation, or
    /// operation-specific durable effect. The resulting canonical digest is installed into the
    /// final hardware statement; no half-authorized certificate is exposed.
    pub fn derive_from_transition(
        proof: &TransitionProofStatementV1,
        context: KagemushaGuardContextV1,
    ) -> Result<Self, KagemushaRecursionErrorV1> {
        if proof.version != KAGEMUSHA_WIRE_VERSION_V1 {
            return Err(KagemushaRecursionErrorV1::UnsupportedVersion);
        }
        let operation = KagemushaOperationV1::from(proof.kind);
        let (network_id, asset_id, liability_pool_id) =
            normalized_lane_bindings(&proof.lane, proof.asset_incarnation)?;
        if proof.protocol_version != KAGEMUSHA_WIRE_VERSION_V1
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
            || proof.prepared_transition_binding_digest
                != context.prepared_transition_binding_digest
            || proof.predecessor_commitment == [0; 32]
            || proof.successor_commitment == [0; 32]
            || proof.predecessor_commitment == proof.successor_commitment
            || proof.predecessor_state_nonce_commitment == [0; 32]
            || proof.successor_state_nonce_commitment == [0; 32]
            || proof.predecessor_state_nonce_commitment == proof.successor_state_nonce_commitment
            || proof.effect_digest == [0; 32]
            || (operation == KagemushaOperationV1::Rotate) != (proof.amount == 0)
            || (operation == KagemushaOperationV1::MintFold)
                != (proof.mint_finality_semantic_digest != [0; 32])
            || (operation == KagemushaOperationV1::MintFold)
                != (proof.mint_finality_proof_binding_digest != [0; 32])
        {
            return Err(KagemushaRecursionErrorV1::InvalidTransitionStatement);
        }
        if proof.predecessor_release_id == [0; 32]
            || proof.predecessor_release_id != proof.release_id
        {
            return Err(KagemushaRecursionErrorV1::InvalidTransitionStatement);
        }
        let is_peer = operation == KagemushaOperationV1::SendSplit;
        if is_peer != (proof.peer_credit_id != [0; 32])
            || is_peer != (proof.recipient_encryption_key_binding != [0; 32])
        {
            return Err(KagemushaRecursionErrorV1::InvalidTransitionStatement);
        }
        let uses_outbox = matches!(
            operation,
            KagemushaOperationV1::SendSplit | KagemushaOperationV1::RedeemSplit
        );
        if uses_outbox != (proof.prepared_transition_binding_digest != [0; 32]) {
            return Err(KagemushaRecursionErrorV1::InvalidTransitionStatement);
        }
        let is_receive = operation == KagemushaOperationV1::ReceiveFold;
        if is_receive != (proof.receive_credit_binding_digest != [0; 32])
            || proof.receive_credit_binding_digest != context.receive_credit_binding_digest
            || proof.predecessor_suite_id != proof.successor_suite_id
            || proof.predecessor_vk_digest != proof.successor_vk_digest
        {
            return Err(KagemushaRecursionErrorV1::InvalidTransitionStatement);
        }
        let exact_successor = if operation == KagemushaOperationV1::Rotate {
            proof.successor_sequence == 0 && proof.journal_revision_after == 0
        } else {
            proof.successor_sequence
                == proof
                    .predecessor_sequence
                    .checked_add(1)
                    .ok_or(KagemushaRecursionErrorV1::SequenceOverflow)?
                && proof.journal_revision_after
                    == proof
                        .journal_revision_before
                        .checked_add(1)
                        .ok_or(KagemushaRecursionErrorV1::JournalOverflow)?
        };
        if !exact_successor {
            return Err(KagemushaRecursionErrorV1::NonExactSuccessor);
        }
        match operation {
            KagemushaOperationV1::Rotate => {
                if proof.successor_epoch.generation
                    != proof
                        .predecessor_epoch
                        .generation
                        .checked_add(1)
                        .ok_or(KagemushaRecursionErrorV1::EpochOverflow)?
                    || proof.successor_epoch.epoch_id == proof.predecessor_epoch.epoch_id
                    || proof.successor_device_policy_binding.device_key_reference
                        == proof.predecessor_device_policy_binding.device_key_reference
                {
                    return Err(KagemushaRecursionErrorV1::InvalidRotation);
                }
            }
            _ => {
                if proof.successor_epoch != proof.predecessor_epoch
                    || proof.successor_device_policy_binding
                        != proof.predecessor_device_policy_binding
                {
                    return Err(KagemushaRecursionErrorV1::InvalidRotation);
                }
            }
        }
        context.validate(operation, proof.amount)?;
        if context.transition_effect_digest != proof.effect_digest {
            return Err(KagemushaRecursionErrorV1::StateHardwareMismatch);
        }

        Ok(Self {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            protocol_version: proof.protocol_version,
            predecessor_suite_id: proof.predecessor_suite_id,
            predecessor_vk_digest: proof.predecessor_vk_digest,
            successor_suite_id: proof.successor_suite_id,
            successor_vk_digest: proof.successor_vk_digest,
            operation,
            amount: proof.amount,
            peer_credit_id: proof.peer_credit_id,
            recipient_encryption_key_binding: proof.recipient_encryption_key_binding,
            mint_finality_proof_binding_digest: proof.mint_finality_proof_binding_digest,
            predecessor_release_id: proof.predecessor_release_id,
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
            prepared_transition_binding_digest: proof.prepared_transition_binding_digest,
            terminal_commit_binding_digest: [0; 32],
            sender_one_time_authorization_digest: [0; 32],
            receive_credit_binding_digest: proof.receive_credit_binding_digest,
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
    ) -> Result<(), KagemushaRecursionErrorV1> {
        if proof.version != KAGEMUSHA_WIRE_VERSION_V1
            || hardware.version != KAGEMUSHA_WIRE_VERSION_V1
        {
            return Err(KagemushaRecursionErrorV1::UnsupportedVersion);
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
                != proof
                    .digest()
                    .map_err(|error| KagemushaRecursionErrorV1::StateStatement(error.to_string()))?
            || hardware.normalized_guard_statement_digest != self.canonical_digest()?
        {
            return Err(KagemushaRecursionErrorV1::StateHardwareMismatch);
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
        context: KagemushaGuardContextV1,
    ) -> Result<Self, KagemushaRecursionErrorV1> {
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
        context: KagemushaGuardContextV1,
    ) -> Result<Self, KagemushaRecursionErrorV1> {
        let (network_id, asset_id, liability_pool_id) =
            normalized_lane_bindings(&bootstrap.lane, bootstrap.asset_incarnation)?;
        if bootstrap.version != KAGEMUSHA_WIRE_VERSION_V1
            || bootstrap.protocol_version != KAGEMUSHA_WIRE_VERSION_V1
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
            return Err(KagemushaRecursionErrorV1::InvalidTransitionStatement);
        }
        context.validate(KagemushaOperationV1::Bootstrap, 0)?;
        Ok(Self {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            protocol_version: bootstrap.protocol_version,
            predecessor_suite_id: [0; 32],
            predecessor_vk_digest: [0; 32],
            successor_suite_id: bootstrap.suite_id,
            successor_vk_digest: bootstrap.vk_digest,
            operation: KagemushaOperationV1::Bootstrap,
            amount: 0,
            peer_credit_id: [0; 32],
            recipient_encryption_key_binding: [0; 32],
            mint_finality_proof_binding_digest: [0; 32],
            predecessor_release_id: [0; 32],
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
            prepared_transition_binding_digest: context.prepared_transition_binding_digest,
            terminal_commit_binding_digest: [0; 32],
            sender_one_time_authorization_digest: [0; 32],
            receive_credit_binding_digest: [0; 32],
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
    pub fn canonical_digest(&self) -> Result<DigestV1, KagemushaRecursionErrorV1> {
        self.validate_shape()?;
        Ok(guard_bundle::normalized_guard_statement_digest_v1(self))
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
        statement: &KagemushaRedemptionStatementV1,
    ) -> Result<(), KagemushaRecursionErrorV1> {
        statement
            .validate_shape()
            .map_err(|error| KagemushaRecursionErrorV1::TransportBinding(error.to_string()))?;
        let lifecycle = &statement.lifecycle;
        let asset_id = kagemusha_asset_identity_digest_v1(&lifecycle.asset)
            .map_err(|error| KagemushaRecursionErrorV1::TransportBinding(error.to_string()))?;
        if self.operation != KagemushaOperationV1::RedeemSplit
            || statement.amount != self.amount
            || statement.version != self.version
            || lifecycle.operation_kind != KagemushaOperationKindV1::RedeemSplit
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
                .map_err(|error| KagemushaRecursionErrorV1::TransportBinding(error.to_string()))?
                != self.lifecycle_binding_digest
        {
            return Err(KagemushaRecursionErrorV1::PublicBindingMismatch);
        }
        Ok(())
    }

    fn validate_shape(&self) -> Result<(), KagemushaRecursionErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.protocol_version != KAGEMUSHA_WIRE_VERSION_V1
            || self.successor_suite_id == [0; 32]
            || self.successor_vk_digest == [0; 32]
            || self.release_id == [0; 32]
            || self.network_id == [0; 32]
            || self.asset_id == [0; 32]
            || self.asset_incarnation.validate().is_err()
            || self.asset_scale > KAGEMUSHA_ASSET_SCALE_MAX_V1
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
            return Err(KagemushaRecursionErrorV1::InvalidTransitionStatement);
        }
        let is_bootstrap = self.operation == KagemushaOperationV1::Bootstrap;
        if is_bootstrap != (self.predecessor_release_id == [0; 32])
            || (!is_bootstrap && self.predecessor_release_id != self.release_id)
        {
            return Err(KagemushaRecursionErrorV1::InvalidTransitionStatement);
        }
        if matches!(
            self.operation,
            KagemushaOperationV1::Bootstrap | KagemushaOperationV1::Rotate
        ) != (self.amount == 0)
        {
            return Err(KagemushaRecursionErrorV1::InvalidTransitionStatement);
        }
        let is_peer = self.operation == KagemushaOperationV1::SendSplit;
        if is_peer != (self.peer_credit_id != [0; 32])
            || is_peer != (self.recipient_encryption_key_binding != [0; 32])
        {
            return Err(KagemushaRecursionErrorV1::InvalidTransitionStatement);
        }
        let is_receive = self.operation == KagemushaOperationV1::ReceiveFold;
        let uses_outbox = matches!(
            self.operation,
            KagemushaOperationV1::SendSplit | KagemushaOperationV1::RedeemSplit
        );
        let is_terminal = self.terminal_commit_binding_digest != [0; 32];
        let has_sender_authorization = self.sender_one_time_authorization_digest != [0; 32];
        if is_receive != (self.receive_credit_binding_digest != [0; 32])
            || (!is_bootstrap
                && (self.predecessor_suite_id != self.successor_suite_id
                    || self.predecessor_vk_digest != self.successor_vk_digest))
            || uses_outbox != (self.prepared_transition_binding_digest != [0; 32])
            || (is_terminal && !uses_outbox)
            || has_sender_authorization
                != (is_terminal && self.operation == KagemushaOperationV1::SendSplit)
        {
            return Err(KagemushaRecursionErrorV1::InvalidTransitionStatement);
        }
        if (self.operation == KagemushaOperationV1::MintFold)
            != (self.mint_finality_proof_binding_digest != [0; 32])
        {
            return Err(KagemushaRecursionErrorV1::InvalidTransitionStatement);
        }
        if is_bootstrap {
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
                return Err(KagemushaRecursionErrorV1::InvalidTransitionStatement);
            }
            return Ok(());
        }
        let exact_successor = if self.operation == KagemushaOperationV1::Rotate {
            self.successor_logical_sequence == 0 && self.journal_revision_after == 0
        } else {
            self.successor_logical_sequence
                == self
                    .predecessor_logical_sequence
                    .checked_add(1)
                    .ok_or(KagemushaRecursionErrorV1::SequenceOverflow)?
                && self.journal_revision_after
                    == self
                        .journal_revision_before
                        .checked_add(1)
                        .ok_or(KagemushaRecursionErrorV1::JournalOverflow)?
        };
        if self.predecessor_state_commitment == [0; 32]
            || self.predecessor_state_commitment == self.successor_state_commitment
            || self.predecessor_state_nonce_commitment == [0; 32]
            || self.predecessor_state_nonce_commitment == self.successor_state_nonce_commitment
            || self.predecessor_hardware_epoch_generation == 0
            || self.predecessor_hardware_epoch_id == [0; 32]
            || self.predecessor_key_reference == [0; 32]
            || self.predecessor_hardware_policy_id == [0; 32]
            || self.predecessor_suite_id == [0; 32]
            || self.predecessor_vk_digest == [0; 32]
            || !exact_successor
        {
            return Err(KagemushaRecursionErrorV1::InvalidTransitionStatement);
        }
        Ok(())
    }

    fn validate_release_effects(
        &self,
        canonical_empty_effect_digest: DigestV1,
    ) -> Result<(), KagemushaRecursionErrorV1> {
        if canonical_empty_effect_digest == [0; 32] {
            return Err(KagemushaRecursionErrorV1::InvalidArtifacts);
        }
        let inbox_is_empty = self.durable_inbox_effect_digest == canonical_empty_effect_digest;
        let outbox_is_empty = self.durable_outbox_effect_digest == canonical_empty_effect_digest;
        let inbox_is_present = self.durable_inbox_effect_digest != [0; 32] && !inbox_is_empty;
        let outbox_is_present = self.durable_outbox_effect_digest != [0; 32] && !outbox_is_empty;
        let valid = match self.operation {
            KagemushaOperationV1::MintFold | KagemushaOperationV1::ReceiveFold => {
                inbox_is_present && outbox_is_empty
            }
            KagemushaOperationV1::SendSplit | KagemushaOperationV1::RedeemSplit => {
                inbox_is_empty && outbox_is_present
            }
            KagemushaOperationV1::Bootstrap | KagemushaOperationV1::Rotate => {
                inbox_is_empty && outbox_is_empty
            }
        };
        if !valid {
            return Err(KagemushaRecursionErrorV1::InvalidGuardEffects(
                self.operation,
            ));
        }
        Ok(())
    }
}

/// Unlinkable public outputs exposed by the final terminal-authorization parities.
///
/// Aggregate state heads, stable lane and credential identities, logical sequence, hardware
/// epoch, and journal position are intentionally absent. They remain private witnesses in the
/// prepared transition and terminal hardware-commit relations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaRecursivePublicOutputV1 {
    /// Complete authenticated lifecycle projection.
    pub lifecycle: KagemushaLifecycleBindingV1,
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
    /// Exact receiver hardware binding for `SendSplit`, otherwise zero.
    pub receiver_binding_digest: DigestV1,
    /// Amount-bound encrypted-credit commitment for `SendSplit`, otherwise zero.
    pub ciphertext_commitment: DigestV1,
    /// Monetary amount changed by the operation.
    pub amount: u128,
    /// Operation-specific terminal output binding. A send commits the receiver credit and lane;
    /// a redemption carries its terminal redemption commitment.
    pub terminal_output_binding: DigestV1,
}

impl KagemushaRecursivePublicOutputV1 {
    /// Construct and validate the sole unlinkable public transition projection.
    ///
    /// # Errors
    ///
    /// Rejects malformed lifecycle data, zero authority bindings, operation-specific field
    /// substitution, or noncanonical zero padding.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        lifecycle: KagemushaLifecycleBindingV1,
        semantic_digest: DigestV1,
        candidate_envelope_digest: DigestV1,
        commit_certificate_digest: DigestV1,
        transition_nullifier: DigestV1,
        request_digest: DigestV1,
        receiver_binding_digest: DigestV1,
        ciphertext_commitment: DigestV1,
        amount: u128,
        terminal_output_binding: DigestV1,
    ) -> Result<Self, KagemushaRecursionErrorV1> {
        let output = Self {
            lifecycle,
            semantic_digest,
            candidate_envelope_digest,
            commit_certificate_digest,
            transition_nullifier,
            request_digest,
            receiver_binding_digest,
            ciphertext_commitment,
            amount,
            terminal_output_binding,
        };
        output.validate()?;
        Ok(output)
    }

    /// Return the selected fixed-shape operation.
    #[must_use]
    pub fn operation(&self) -> KagemushaOperationV1 {
        self.lifecycle.operation_kind.into()
    }

    fn validate(&self) -> Result<(), KagemushaRecursionErrorV1> {
        self.lifecycle
            .validate()
            .map_err(|error| KagemushaRecursionErrorV1::WireProof(error.to_string()))?;
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
            return Err(KagemushaRecursionErrorV1::InvalidPublicOutput);
        }
        let payment_bindings = [
            self.request_digest,
            self.receiver_binding_digest,
            self.ciphertext_commitment,
        ];
        match self.operation() {
            KagemushaOperationV1::SendSplit => {
                if self.amount == 0
                    || payment_bindings.into_iter().any(|digest| digest == [0; 32])
                    || self.terminal_output_binding == [0; 32]
                {
                    return Err(KagemushaRecursionErrorV1::InvalidPublicOutput);
                }
            }
            KagemushaOperationV1::RedeemSplit => {
                if self.amount == 0
                    || payment_bindings.into_iter().any(|digest| digest != [0; 32])
                    || self.terminal_output_binding == [0; 32]
                {
                    return Err(KagemushaRecursionErrorV1::InvalidPublicOutput);
                }
            }
            KagemushaOperationV1::Bootstrap
            | KagemushaOperationV1::MintFold
            | KagemushaOperationV1::ReceiveFold
            | KagemushaOperationV1::Rotate => {
                return Err(KagemushaRecursionErrorV1::InvalidPublicOutput);
            }
        }
        Ok(())
    }
}

const INCOMING_PROOF_BINDING_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:incoming-terminal-authorization-binding";

/// Hash the exact proof-independent claims shared by terminal authorization and receive staging.
///
/// The order is request, receiver, sender-state pair, output, ciphertext, candidate, certificate.
/// Candidate and certificate are already committed before this digest is constructed. Neither
/// this digest nor the terminal output binding may feed back into those prepared-transition
/// transcripts.
/// This field-only helper grants no proof authority; callers must authenticate every input.
pub(crate) fn canonical_incoming_payment_claims_binding_v1(digests: [DigestV1; 7]) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(INCOMING_PROOF_BINDING_DOMAIN_V1);
    hasher.update([0]);
    for digest in digests {
        hasher.update(digest);
    }
    hasher.finalize().into()
}

/// Bind the exact sender predecessor and successor commitments used by a terminal payment.
///
/// Keeping this transcript in one helper prevents the prepared-candidate projection, terminal
/// authorization, and receiver admission paths from silently disagreeing about the state pair.
pub(crate) fn canonical_sender_state_pair_digest_v1(
    sender_before_commitment: DigestV1,
    sender_after_commitment: DigestV1,
) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(b"iroha:kagemusha:v1:incoming-sender-state-pair");
    hasher.update([0]);
    hasher.update(sender_before_commitment);
    hasher.update(sender_after_commitment);
    hasher.finalize().into()
}

/// Bind the exact accepted payment body and post-commit proof claims before a receive fold.
///
/// Proof bytes may be randomized, but both parities must authenticate the same candidate,
/// certificate, request, sender state transition, output, and encrypted credit.
///
/// # Errors
///
/// Rejects any malformed or substituted request or payment.
pub fn kagemusha_incoming_proof_binding_digest_v1(
    request: &KagemushaPaymentRequestV1,
    payment: &KagemushaPaymentV1,
) -> Result<DigestV1, KagemushaRecursionErrorV1> {
    payment
        .validate_shape_against(request)
        .map_err(|error| KagemushaRecursionErrorV1::WireProof(error.to_string()))?;
    let request_digest = request
        .canonical_digest()
        .map_err(|error| KagemushaRecursionErrorV1::WireProof(error.to_string()))?;
    let output_digest = payment
        .output
        .canonical_digest_against(request)
        .map_err(|error| KagemushaRecursionErrorV1::WireProof(error.to_string()))?;
    let state_pair_digest = canonical_sender_state_pair_digest_v1(
        payment.output.sender_before_commitment,
        payment.output.sender_after_commitment,
    );
    Ok(canonical_incoming_payment_claims_binding_v1([
        request_digest,
        request.hardware_credential.credential_id,
        state_pair_digest,
        output_digest,
        kagemusha_ciphertext_digest_v1(&payment.encrypted_credit),
        payment.proof.candidate_envelope_digest,
        payment.proof.commit_certificate_digest,
    ]))
}

/// Exact four-role artifact set for the paired finalized-mint helper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaMintFinalityArtifactsV1 {
    /// Eq helper proving key binding (`MintCreditPkEq`).
    pub proving_key_eq: KagemushaArtifactBindingV1,
    /// Eq helper verifying key binding (`MintCreditVkEq`).
    pub verifying_key_eq: KagemushaArtifactBindingV1,
    /// Ep helper proving key binding (`MintCreditPkEp`).
    pub proving_key_ep: KagemushaArtifactBindingV1,
    /// Ep helper verifying key binding (`MintCreditVkEp`).
    pub verifying_key_ep: KagemushaArtifactBindingV1,
}

impl KagemushaMintFinalityArtifactsV1 {
    /// Resolve the four non-state helper roles from an already authenticated release.
    #[must_use]
    pub fn from_authenticated_release(release: &KagemushaAuthenticatedReleaseV1) -> Self {
        Self {
            proving_key_eq: release.artifact(KagemushaArtifactRoleV1::MintCreditPkEq),
            verifying_key_eq: release.artifact(KagemushaArtifactRoleV1::MintCreditVkEq),
            proving_key_ep: release.artifact(KagemushaArtifactRoleV1::MintCreditPkEp),
            verifying_key_ep: release.artifact(KagemushaArtifactRoleV1::MintCreditVkEp),
        }
    }

    fn validate(self) -> Result<(), KagemushaRecursionErrorV1> {
        let bindings = [
            self.proving_key_eq,
            self.verifying_key_eq,
            self.proving_key_ep,
            self.verifying_key_ep,
        ];
        if bindings
            .iter()
            .zip(KAGEMUSHA_MINT_FINALITY_ARTIFACT_ROLES_V1)
            .any(|(binding, role)| {
                let max = match role {
                    KagemushaArtifactRoleV1::MintCreditPkEq
                    | KagemushaArtifactRoleV1::MintCreditPkEp => {
                        KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1
                    }
                    KagemushaArtifactRoleV1::MintCreditVkEq
                    | KagemushaArtifactRoleV1::MintCreditVkEp => {
                        KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1
                    }
                    _ => 0,
                };
                binding.role != role
                    || binding.sha256 == [0; 32]
                    || binding.byte_len == 0
                    || binding.byte_len > max
            })
        {
            return Err(KagemushaRecursionErrorV1::InvalidArtifacts);
        }
        Ok(())
    }
}

/// Trusted content-addressed artifacts for the sole Kagemusha V1 proof release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaRecursionArtifactsV1 {
    /// Authenticated release identifier.
    pub release_id: DigestV1,
    /// Release-pinned circuit compilation profile.
    pub profile_digest: DigestV1,
    /// Exact Eq circuit and compiled-protocol digest.
    pub eq_protocol_digest: DigestV1,
    /// Exact Ep circuit and compiled-protocol digest.
    pub ep_protocol_digest: DigestV1,
    /// Exact Eq terminal `TerminalAuthorization` compiled-protocol digest.
    pub terminal_authorization_eq_protocol_digest: DigestV1,
    /// Exact Ep terminal `TerminalAuthorization` compiled-protocol digest.
    pub terminal_authorization_ep_protocol_digest: DigestV1,
    /// Exact Eq compact post-commit `CommitWrapper` compiled-protocol digest.
    pub commit_wrapper_eq_protocol_digest: DigestV1,
    /// Exact Ep compact post-commit `CommitWrapper` compiled-protocol digest.
    pub commit_wrapper_ep_protocol_digest: DigestV1,
    /// Exact authenticated Eq mint-authorization compiled-protocol digest.
    pub mint_authorization_eq_protocol_digest: DigestV1,
    /// Exact authenticated Ep mint-authorization compiled-protocol digest.
    pub mint_authorization_ep_protocol_digest: DigestV1,
    /// Exact authenticated Eq compact finalized-mint compiled-protocol digest.
    pub mint_finality_eq_protocol_digest: DigestV1,
    /// Exact authenticated Ep compact finalized-mint compiled-protocol digest.
    pub mint_finality_ep_protocol_digest: DigestV1,
    /// Exact authenticated Eq normalized `GuardBundle` compiled-protocol digest.
    pub guard_bundle_eq_protocol_digest: DigestV1,
    /// Exact authenticated Ep normalized `GuardBundle` compiled-protocol digest.
    pub guard_bundle_ep_protocol_digest: DigestV1,
    /// Exact authenticated Eq one-block mint-hash shard compiled-protocol digest.
    pub mint_hash_shard_eq_protocol_digest: DigestV1,
    /// Exact authenticated Ep one-block mint-hash shard compiled-protocol digest.
    pub mint_hash_shard_ep_protocol_digest: DigestV1,
    /// Exact authenticated Eq ordered mint-hash claim compiled-protocol digest.
    pub mint_hash_claim_eq_protocol_digest: DigestV1,
    /// Exact authenticated Ep ordered mint-hash claim compiled-protocol digest.
    pub mint_hash_claim_ep_protocol_digest: DigestV1,
    /// Exact Eq normalized `GuardBundle` verifying-key binding.
    pub guard_bundle_verifying_key_eq: KagemushaArtifactBindingV1,
    /// Exact Ep normalized `GuardBundle` verifying-key binding.
    pub guard_bundle_verifying_key_ep: KagemushaArtifactBindingV1,
    /// Exact Eq terminal `TerminalAuthorization` verifying-key binding.
    pub terminal_authorization_verifying_key_eq: KagemushaArtifactBindingV1,
    /// Exact Ep terminal `TerminalAuthorization` verifying-key binding.
    pub terminal_authorization_verifying_key_ep: KagemushaArtifactBindingV1,
    /// Exact Eq post-commit wrapper verifying-key binding.
    pub commit_wrapper_verifying_key_eq: KagemushaArtifactBindingV1,
    /// Exact Ep post-commit wrapper verifying-key binding.
    pub commit_wrapper_verifying_key_ep: KagemushaArtifactBindingV1,
    /// Four distinct finalized-mint helper artifact roles.
    pub mint_finality: KagemushaMintFinalityArtifactsV1,
    /// Digest of the complete authenticated release manifest.
    pub artifact_manifest_digest: DigestV1,
    /// Canonical empty inbox/outbox effect digest fixed by this release.
    pub canonical_empty_effect_digest: DigestV1,
}

impl KagemushaRecursionArtifactsV1 {
    /// Construct the recursion artifact seam from one already authenticated release.
    #[must_use]
    pub fn from_authenticated_release(
        release: &KagemushaAuthenticatedReleaseV1,
        canonical_empty_effect_digest: DigestV1,
    ) -> Self {
        let mint_authorization = release
            .helper_protocol(KagemushaQualifiedHelperCircuitV1::MintAuthorization)
            .expect("authenticated Kagemusha release has every helper protocol");
        let mint_finality = release
            .helper_protocol(KagemushaQualifiedHelperCircuitV1::MintCredit)
            .expect("authenticated Kagemusha release has every helper protocol");
        let guard_bundle = release
            .helper_protocol(KagemushaQualifiedHelperCircuitV1::GuardBundle)
            .expect("authenticated Kagemusha release has every helper protocol");
        let mint_hash_shard = release
            .helper_protocol(KagemushaQualifiedHelperCircuitV1::MintHashShard)
            .expect("authenticated Kagemusha release has every helper protocol");
        let mint_hash_claim = release
            .helper_protocol(KagemushaQualifiedHelperCircuitV1::MintHashClaim)
            .expect("authenticated Kagemusha release has every helper protocol");
        let (terminal_authorization_eq_protocol_digest, terminal_authorization_ep_protocol_digest) =
            release.qualified_relation_protocol_digests(
                KagemushaQualifiedRelationV1::TerminalAuthorization,
            );
        let (commit_wrapper_eq_protocol_digest, commit_wrapper_ep_protocol_digest) = release
            .qualified_relation_protocol_digests(KagemushaQualifiedRelationV1::CommitWrapper);
        Self {
            release_id: release.release_id(),
            profile_digest: release.profile_digest(),
            eq_protocol_digest: release.eq_protocol_digest(),
            ep_protocol_digest: release.ep_protocol_digest(),
            terminal_authorization_eq_protocol_digest,
            terminal_authorization_ep_protocol_digest,
            commit_wrapper_eq_protocol_digest,
            commit_wrapper_ep_protocol_digest,
            mint_authorization_eq_protocol_digest: mint_authorization.eq_protocol_digest,
            mint_authorization_ep_protocol_digest: mint_authorization.ep_protocol_digest,
            mint_finality_eq_protocol_digest: mint_finality.eq_protocol_digest,
            mint_finality_ep_protocol_digest: mint_finality.ep_protocol_digest,
            guard_bundle_eq_protocol_digest: guard_bundle.eq_protocol_digest,
            guard_bundle_ep_protocol_digest: guard_bundle.ep_protocol_digest,
            mint_hash_shard_eq_protocol_digest: mint_hash_shard.eq_protocol_digest,
            mint_hash_shard_ep_protocol_digest: mint_hash_shard.ep_protocol_digest,
            mint_hash_claim_eq_protocol_digest: mint_hash_claim.eq_protocol_digest,
            mint_hash_claim_ep_protocol_digest: mint_hash_claim.ep_protocol_digest,
            guard_bundle_verifying_key_eq: release
                .artifact(KagemushaArtifactRoleV1::GuardBundleVkEq),
            guard_bundle_verifying_key_ep: release
                .artifact(KagemushaArtifactRoleV1::GuardBundleVkEp),
            terminal_authorization_verifying_key_eq: release
                .artifact(KagemushaArtifactRoleV1::TerminalAuthorizationVkEq),
            terminal_authorization_verifying_key_ep: release
                .artifact(KagemushaArtifactRoleV1::TerminalAuthorizationVkEp),
            commit_wrapper_verifying_key_eq: release
                .artifact(KagemushaArtifactRoleV1::CommitWrapperVkEq),
            commit_wrapper_verifying_key_ep: release
                .artifact(KagemushaArtifactRoleV1::CommitWrapperVkEp),
            mint_finality: KagemushaMintFinalityArtifactsV1::from_authenticated_release(release),
            artifact_manifest_digest: release.manifest_digest(),
            canonical_empty_effect_digest,
        }
    }

    fn validate(self) -> Result<(), KagemushaRecursionErrorV1> {
        self.mint_finality.validate()?;
        let guard_bindings = [
            (
                self.guard_bundle_verifying_key_eq,
                KagemushaArtifactRoleV1::GuardBundleVkEq,
            ),
            (
                self.guard_bundle_verifying_key_ep,
                KagemushaArtifactRoleV1::GuardBundleVkEp,
            ),
        ];
        let wrapper_bindings = [
            (
                self.terminal_authorization_verifying_key_eq,
                KagemushaArtifactRoleV1::TerminalAuthorizationVkEq,
            ),
            (
                self.terminal_authorization_verifying_key_ep,
                KagemushaArtifactRoleV1::TerminalAuthorizationVkEp,
            ),
        ];
        let authorization_bindings = [
            (
                self.commit_wrapper_verifying_key_eq,
                KagemushaArtifactRoleV1::CommitWrapperVkEq,
            ),
            (
                self.commit_wrapper_verifying_key_ep,
                KagemushaArtifactRoleV1::CommitWrapperVkEp,
            ),
        ];
        let dedicated_verifier_bindings = [
            self.guard_bundle_verifying_key_eq,
            self.guard_bundle_verifying_key_ep,
            self.terminal_authorization_verifying_key_eq,
            self.terminal_authorization_verifying_key_ep,
            self.commit_wrapper_verifying_key_eq,
            self.commit_wrapper_verifying_key_ep,
        ];
        if self.release_id == [0; 32]
            || self.profile_digest == [0; 32]
            || self.eq_protocol_digest == [0; 32]
            || self.ep_protocol_digest == [0; 32]
            || self.eq_protocol_digest == self.ep_protocol_digest
            || self.terminal_authorization_eq_protocol_digest == [0; 32]
            || self.terminal_authorization_ep_protocol_digest == [0; 32]
            || self.terminal_authorization_eq_protocol_digest
                == self.terminal_authorization_ep_protocol_digest
            || self.commit_wrapper_eq_protocol_digest == [0; 32]
            || self.commit_wrapper_ep_protocol_digest == [0; 32]
            || self.commit_wrapper_eq_protocol_digest == self.commit_wrapper_ep_protocol_digest
            || self.mint_authorization_eq_protocol_digest == [0; 32]
            || self.mint_authorization_ep_protocol_digest == [0; 32]
            || self.mint_authorization_eq_protocol_digest
                == self.mint_authorization_ep_protocol_digest
            || self.mint_finality_eq_protocol_digest == [0; 32]
            || self.mint_finality_ep_protocol_digest == [0; 32]
            || self.mint_finality_eq_protocol_digest == self.mint_finality_ep_protocol_digest
            || self.guard_bundle_eq_protocol_digest == [0; 32]
            || self.guard_bundle_ep_protocol_digest == [0; 32]
            || self.guard_bundle_eq_protocol_digest == self.guard_bundle_ep_protocol_digest
            || self.mint_hash_shard_eq_protocol_digest == [0; 32]
            || self.mint_hash_shard_ep_protocol_digest == [0; 32]
            || self.mint_hash_shard_eq_protocol_digest == self.mint_hash_shard_ep_protocol_digest
            || self.mint_hash_claim_eq_protocol_digest == [0; 32]
            || self.mint_hash_claim_ep_protocol_digest == [0; 32]
            || self.mint_hash_claim_eq_protocol_digest == self.mint_hash_claim_ep_protocol_digest
            || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fp>(
                self.eq_protocol_digest,
            )
            .is_none()
            || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fq>(
                self.ep_protocol_digest,
            )
            .is_none()
            || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fp>(
                self.terminal_authorization_eq_protocol_digest,
            )
            .is_none()
            || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fq>(
                self.terminal_authorization_ep_protocol_digest,
            )
            .is_none()
            || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fp>(
                self.commit_wrapper_eq_protocol_digest,
            )
            .is_none()
            || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fq>(
                self.commit_wrapper_ep_protocol_digest,
            )
            .is_none()
            || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fp>(
                self.mint_authorization_eq_protocol_digest,
            )
            .is_none()
            || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fq>(
                self.mint_authorization_ep_protocol_digest,
            )
            .is_none()
            || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fp>(
                self.mint_finality_eq_protocol_digest,
            )
            .is_none()
            || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fq>(
                self.mint_finality_ep_protocol_digest,
            )
            .is_none()
            || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fp>(
                self.guard_bundle_eq_protocol_digest,
            )
            .is_none()
            || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fq>(
                self.guard_bundle_ep_protocol_digest,
            )
            .is_none()
            || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fp>(
                self.mint_hash_shard_eq_protocol_digest,
            )
            .is_none()
            || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fq>(
                self.mint_hash_shard_ep_protocol_digest,
            )
            .is_none()
            || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fp>(
                self.mint_hash_claim_eq_protocol_digest,
            )
            .is_none()
            || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fq>(
                self.mint_hash_claim_ep_protocol_digest,
            )
            .is_none()
            || self.artifact_manifest_digest == [0; 32]
            || self.canonical_empty_effect_digest == [0; 32]
            || guard_bindings.iter().any(|(binding, role)| {
                binding.role != *role
                    || binding.sha256 == [0; 32]
                    || binding.byte_len == 0
                    || binding.byte_len > KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1
            })
            || wrapper_bindings.iter().any(|(binding, role)| {
                binding.role != *role
                    || binding.sha256 == [0; 32]
                    || binding.byte_len == 0
                    || binding.byte_len > KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1
            })
            || authorization_bindings.iter().any(|(binding, role)| {
                binding.role != *role
                    || binding.sha256 == [0; 32]
                    || binding.byte_len == 0
                    || binding.byte_len > KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1
            })
            || dedicated_verifier_bindings
                .iter()
                .enumerate()
                .any(|(index, binding)| {
                    dedicated_verifier_bindings[index + 1..]
                        .iter()
                        .any(|other| binding.sha256 == other.sha256)
                })
        {
            return Err(KagemushaRecursionErrorV1::InvalidArtifacts);
        }
        let protocols = [
            self.eq_protocol_digest,
            self.ep_protocol_digest,
            self.terminal_authorization_eq_protocol_digest,
            self.terminal_authorization_ep_protocol_digest,
            self.commit_wrapper_eq_protocol_digest,
            self.commit_wrapper_ep_protocol_digest,
            self.mint_authorization_eq_protocol_digest,
            self.mint_authorization_ep_protocol_digest,
            self.guard_bundle_protocol_digest(KagemushaPastaParityV1::Eq)?,
            self.guard_bundle_protocol_digest(KagemushaPastaParityV1::Ep)?,
            self.mint_finality_protocol_digest(KagemushaPastaParityV1::Eq)?,
            self.mint_finality_protocol_digest(KagemushaPastaParityV1::Ep)?,
            self.mint_hash_shard_eq_protocol_digest,
            self.mint_hash_shard_ep_protocol_digest,
            self.mint_hash_claim_eq_protocol_digest,
            self.mint_hash_claim_ep_protocol_digest,
        ];
        if protocols
            .iter()
            .enumerate()
            .any(|(index, digest)| protocols[index + 1..].contains(digest))
        {
            return Err(KagemushaRecursionErrorV1::InvalidArtifacts);
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
        parity: KagemushaPastaParityV1,
    ) -> Result<DigestV1, KagemushaRecursionErrorV1> {
        Ok(match parity {
            KagemushaPastaParityV1::Eq => self.guard_bundle_eq_protocol_digest,
            KagemushaPastaParityV1::Ep => self.guard_bundle_ep_protocol_digest,
        })
    }

    /// Return the exact release-pinned finalized-mint compiled-protocol identity for one parity.
    ///
    /// The authenticated `MintCredit` helper metadata carries the actual compiled outer
    /// protocol identity. Artifact-byte authentication is separate: hashing a profile and VK
    /// binding does not reproduce the identity constrained by the recursive verifier.
    ///
    /// # Errors
    ///
    /// Returns an error only to preserve the common helper-protocol accessor shape.
    pub fn mint_finality_protocol_digest(
        self,
        parity: KagemushaPastaParityV1,
    ) -> Result<DigestV1, KagemushaRecursionErrorV1> {
        Ok(match parity {
            KagemushaPastaParityV1::Eq => self.mint_finality_eq_protocol_digest,
            KagemushaPastaParityV1::Ep => self.mint_finality_ep_protocol_digest,
        })
    }
}

/// Exact one-parity final `TerminalAuthorization` request passed to the governed verifier backend.
#[derive(Clone, Copy, Debug)]
pub struct KagemushaParityVerificationRequestV1<'a> {
    /// Non-interchangeable Eq or Ep role.
    pub parity: KagemushaPastaParityV1,
    /// Release-pinned `TerminalAuthorization` circuit/protocol identity for this parity.
    pub protocol_digest: DigestV1,
    /// Unlinkable common public outputs expected from this parity.
    pub public_output: &'a KagemushaRecursivePublicOutputV1,
    /// Common Eq deferred-equation audit exposed by both wrapper parities.
    pub eq_deferred_audit: DigestV1,
    /// Common Ep deferred-equation audit exposed by both wrapper parities.
    pub ep_deferred_audit: DigestV1,
    /// Current augmented `TerminalAuthorization` proof body for this parity.
    pub current_proof: &'a [u8],
    /// Strictly decoded, canonical 544-byte delayed-history accumulator.
    pub history_accumulator: &'a [u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
}

/// Exact paired prepared-transition state-proof request passed to the authenticated native
/// verifier.
#[derive(Clone, Copy, Debug)]
pub struct KagemushaStateProofVerificationRequestV1<'a> {
    /// Verifier-reconstructed fixed 85-cell state relation projection.
    pub public_inputs: &'a KagemushaStateRelationPublicInputsV1,
    /// Paired recursive state proof and constant-size histories.
    pub proof: &'a KagemushaPairedProofV1,
}

/// Exact paired mint-credit/finality helper request used only by `MintFold`.
///
/// The two protocol identities are distinct from the aggregate-state Eq/Ep identities. Their
/// fixed circuits are generated from the certified reserve-receipt ordinary-write relation and
/// caller-pinned consensus-finality relation; they expose only the mint statement digest after
/// verification, so reserve provenance does not accumulate in later aggregate states.
#[derive(Clone, Copy, Debug)]
pub struct KagemushaMintFinalityHelperVerificationRequestV1<'a> {
    /// Release-pinned Eq mint-finality helper protocol identity.
    pub eq_protocol_digest: DigestV1,
    /// Release-pinned Ep mint-finality helper protocol identity.
    pub ep_protocol_digest: DigestV1,
    /// Canonical mint statement whose digest is constrained by both helper parities.
    pub statement: &'a KagemushaMintCreditStatementV1,
    /// Exact canonical digest of `statement`.
    pub semantic_digest: DigestV1,
    /// Complete paired mint-finality proof, including both strict history accumulators.
    pub proof: &'a KagemushaPairedProofV1,
    /// Exact paired certificate digest constrained by both helper parities.
    pub finality_certificate_binding: DigestV1,
    /// Current recursively authenticated roster identifier.
    pub finality_authority_head: DigestV1,
    /// Release-pinned genesis roster identifier.
    pub finality_genesis_roster_id: DigestV1,
    /// Eq deferred audit, which binds the shared pair transcript and the exact Ep audit.
    pub finality_proof_binding_digest: DigestV1,
    /// Release-pinned artifact manifest carried by the mint credit.
    pub artifact_manifest_digest: DigestV1,
}

/// Governed recursive verification backend for both Pasta parities.
///
/// Implementations must recursively verify the prepared state proof, normalized hardware guard,
/// exact outbox reservation, and terminal commit certificate before deciding wrapper history.
/// Host-side signature or certificate checks alone never grant monetary authority.
pub trait KagemushaRecursiveVerifierV1 {
    /// Verify both release-pinned recursive state parities and decide their carried histories.
    fn verify_state_proof_and_decide(
        &self,
        request: &KagemushaStateProofVerificationRequestV1<'_>,
    ) -> Result<(), String>;

    /// Verify the final payment's paired post-commit proofs and decide both histories.
    ///
    /// The proof must authenticate the persisted candidate, qualified hardware commit, and exact
    /// request, sender transition, encrypted credit, and payment output.
    fn verify_payment_and_decide(
        &self,
        request: &KagemushaPaymentRequestV1,
        payment: &KagemushaPaymentV1,
    ) -> Result<(), String>;

    /// Verify the distinct paired mint-credit/finality helper proof before constructing a
    /// `MintFold` witness.
    ///
    /// This native preflight is not monetary authority by itself. The `MintFold` state circuit
    /// must recursively verify the same Eq/Ep helper proofs and constrain their common mint
    /// statement digest.
    fn verify_mint_finality_helper(
        &self,
        request: &KagemushaMintFinalityHelperVerificationRequestV1<'_>,
    ) -> Result<(), String>;

    /// Verify the final `TerminalAuthorization`, constrain its unlinkable public outputs, and terminally
    /// decide its delayed-history accumulator.
    fn verify_terminal_authorization_and_decide(
        &self,
        request: &KagemushaParityVerificationRequestV1<'_>,
    ) -> Result<(), String>;
}

/// Explicit fail-closed backend for deployments which have not installed an authenticated
/// Kagemusha proof release.
#[derive(Clone, Copy, Debug, Default)]
pub struct RejectAllKagemushaRecursiveVerifierV1;

impl KagemushaRecursiveVerifierV1 for RejectAllKagemushaRecursiveVerifierV1 {
    fn verify_state_proof_and_decide(
        &self,
        _request: &KagemushaStateProofVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Err("Kagemusha V1 recursive state verifier is unavailable".to_owned())
    }

    fn verify_payment_and_decide(
        &self,
        _request: &KagemushaPaymentRequestV1,
        _payment: &KagemushaPaymentV1,
    ) -> Result<(), String> {
        Err("Kagemusha V1 post-commit payment verifier is unavailable".to_owned())
    }

    fn verify_mint_finality_helper(
        &self,
        _request: &KagemushaMintFinalityHelperVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Err("Kagemusha V1 recursive mint-finality verifier is unavailable".to_owned())
    }

    fn verify_terminal_authorization_and_decide(
        &self,
        _request: &KagemushaParityVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Err("Kagemusha V1 recursive terminal-authorization verifier is unavailable".to_owned())
    }
}

/// Proof of successful verification under both fixed Pasta parity roles.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct VerifiedKagemushaRecursiveProofV1 {
    output: KagemushaRecursivePublicOutputV1,
}

/// Opaque chain-admission capability for one exact recursively verified redemption request.
///
/// This token owns the request which was supplied to the governed paired verifier. Reserve code
/// must consume the token through [`Self::into_request`] instead of accepting a structurally
/// validated request alongside a separately supplied boolean or digest.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct VerifiedKagemushaRedemptionProofV1 {
    request: KagemushaRedemptionRequestV1,
    request_digest: DigestV1,
    recursive_proof: VerifiedKagemushaRecursiveProofV1,
}

impl VerifiedKagemushaRedemptionProofV1 {
    /// Construct an internally bound capability after a reserve test has explicitly mocked both
    /// recursive parities.
    ///
    /// This helper does not exist in production builds. It still validates the complete signed
    /// request and derives the same request/public-output bindings as the production verifier, so
    /// reserve accounting tests cannot accidentally exercise a different request identity.
    #[cfg(test)]
    pub(crate) fn for_reserve_tests_after_mock_recursive_verification(
        request: KagemushaRedemptionRequestV1,
    ) -> Result<Self, KagemushaRecursionErrorV1> {
        request
            .validate_shape()
            .map_err(|error| KagemushaRecursionErrorV1::RedemptionBinding(error.to_string()))?;
        let statement = &request.voucher.statement;
        let semantic_digest = statement
            .canonical_digest()
            .map_err(|error| KagemushaRecursionErrorV1::RedemptionBinding(error.to_string()))?;
        let output = KagemushaRecursivePublicOutputV1::new(
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
            .map_err(|error| KagemushaRecursionErrorV1::RedemptionBinding(error.to_string()))?;
        Ok(Self {
            request,
            request_digest,
            recursive_proof: VerifiedKagemushaRecursiveProofV1 { output },
        })
    }

    /// Borrow the exact request whose voucher passed both recursive parities.
    #[must_use]
    pub fn request(&self) -> &KagemushaRedemptionRequestV1 {
        &self.request
    }

    /// Return the canonical digest of the exact verified chain request.
    #[must_use]
    pub const fn request_digest(&self) -> DigestV1 {
        self.request_digest
    }

    /// Return the common recursive public output authenticated for this request.
    #[must_use]
    pub fn public_output(&self) -> KagemushaRecursivePublicOutputV1 {
        self.recursive_proof.public_output()
    }

    /// Consume the capability and return the exact verified request for reserve admission.
    #[must_use]
    pub fn into_request(self) -> KagemushaRedemptionRequestV1 {
        self.request
    }
}

/// Proof that the separate paired mint-finality helper accepted one exact mint statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub struct VerifiedKagemushaMintFinalityHelperV1 {
    semantic_digest: DigestV1,
    proof_binding_digest: DigestV1,
}

impl VerifiedKagemushaMintFinalityHelperV1 {
    /// Construct a token after a state test explicitly mocks both mint-finality helper parities.
    ///
    /// This helper is absent from production and still rejects the reserved zero statement
    /// digest, allowing state tests to exercise exact token/credit matching without duplicating
    /// authenticated release fixtures.
    #[cfg(test)]
    pub(crate) fn for_state_tests_after_mock_finality_verification(
        semantic_digest: DigestV1,
    ) -> Result<Self, KagemushaRecursionErrorV1> {
        if semantic_digest == [0; 32] {
            return Err(KagemushaRecursionErrorV1::MintFinalityBinding(
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
/// The helper statement is the canonical [`KagemushaMintCreditStatementV1`] digest. The helper
/// circuits, not the aggregate-state circuit key, prove the certified reserve receipt and pinned
/// block-finality relation which generated that mint statement.
/// The inner pair commitment is preserved in the compact outer public column; it is not a hash
/// of the outer histories or audits. Only the governed backend's proof and terminal-history
/// verification authenticates this value.
///
/// # Errors
///
/// Rejects a malformed mint credit, statement/proof mismatch, state-VK substitution, manifest
/// substitution, noncanonical accumulator, or governed backend rejection.
pub fn verify_kagemusha_mint_finality_helper_v1<V: KagemushaRecursiveVerifierV1>(
    verifier: &V,
    artifacts: KagemushaRecursionArtifactsV1,
    mint_credit: &KagemushaMintCreditV1,
) -> Result<VerifiedKagemushaMintFinalityHelperV1, KagemushaRecursionErrorV1> {
    artifacts.validate()?;
    mint_credit
        .validate_shape()
        .map_err(|error| KagemushaRecursionErrorV1::MintFinalityBinding(error.to_string()))?;
    let eq_protocol_digest = mint_credit.proof.eq_protocol_digest;
    let ep_protocol_digest = mint_credit.proof.ep_protocol_digest;
    if mint_credit.statement.lifecycle.release_id != artifacts.release_id
        || mint_credit.artifact_manifest_digest != artifacts.artifact_manifest_digest
        || eq_protocol_digest == artifacts.eq_protocol_digest
        || ep_protocol_digest == artifacts.ep_protocol_digest
        || eq_protocol_digest != artifacts.mint_finality_eq_protocol_digest
        || ep_protocol_digest != artifacts.mint_finality_ep_protocol_digest
        || eq_protocol_digest == [0; 32]
        || ep_protocol_digest == [0; 32]
        || eq_protocol_digest == ep_protocol_digest
    {
        return Err(KagemushaRecursionErrorV1::MintFinalityBinding(
            "release, manifest, or helper protocol identity mismatch".to_owned(),
        ));
    }
    let semantic_digest = mint_credit
        .statement
        .canonical_digest()
        .map_err(|error| KagemushaRecursionErrorV1::MintFinalityBinding(error.to_string()))?;
    mint_credit
        .proof
        .validate_shape_for_semantic_digest(semantic_digest)
        .map_err(|error| KagemushaRecursionErrorV1::MintFinalityBinding(error.to_string()))?;
    KagemushaEqAccumulatorV1::try_from_bytes(&mint_credit.proof.eq_history)?;
    KagemushaEpAccumulatorV1::try_from_bytes(&mint_credit.proof.ep_history)?;
    if mint_credit.proof.guard_eq_credential_audit != mint_credit.finality_certificate_binding
        || mint_credit.proof.guard_ep_credential_audit != mint_credit.finality_authority_head
    {
        return Err(KagemushaRecursionErrorV1::MintFinalityBinding(
            "mint helper certificate or authority-head binding mismatch".to_owned(),
        ));
    }
    verifier
        .verify_mint_finality_helper(&KagemushaMintFinalityHelperVerificationRequestV1 {
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
        .map_err(KagemushaRecursionErrorV1::MintFinalityProofRejected)?;
    Ok(VerifiedKagemushaMintFinalityHelperV1 {
        semantic_digest,
        proof_binding_digest: mint_credit.finality_proof_binding_digest,
    })
}

impl VerifiedKagemushaRecursiveProofV1 {
    /// Return the common public outputs constrained by both accepted parity proofs.
    #[must_use]
    pub fn public_output(&self) -> KagemushaRecursivePublicOutputV1 {
        self.output.clone()
    }
}

/// Verify a prepared aggregate-state proof against its exact public State+Guard projection.
///
/// # Errors
///
/// Rejects release/protocol substitution, malformed fixed histories, any public projection that
/// does not match the proof envelope, or a backend failure to verify and decide both parities.
pub fn verify_kagemusha_state_proof_v1<V: KagemushaRecursiveVerifierV1>(
    verifier: &V,
    artifacts: KagemushaRecursionArtifactsV1,
    public_inputs: &KagemushaStateRelationPublicInputsV1,
    proof: &KagemushaPairedProofV1,
) -> Result<(), KagemushaRecursionErrorV1> {
    artifacts.validate()?;
    proof
        .validate_shape_for_semantic_digest(public_inputs.transport_semantic_digest)
        .map_err(|error| KagemushaRecursionErrorV1::WireProof(error.to_string()))?;
    if public_inputs.successor.release_id != artifacts.release_id
        || public_inputs.eq_protocol_digest != artifacts.eq_protocol_digest
        || public_inputs.ep_protocol_digest != artifacts.ep_protocol_digest
        || public_inputs.guard_eq_protocol_digest
            != artifacts.guard_bundle_protocol_digest(KagemushaPastaParityV1::Eq)?
        || public_inputs.guard_ep_protocol_digest
            != artifacts.guard_bundle_protocol_digest(KagemushaPastaParityV1::Ep)?
        || public_inputs.mint_eq_protocol_digest
            != artifacts.mint_finality_protocol_digest(KagemushaPastaParityV1::Eq)?
        || public_inputs.mint_ep_protocol_digest
            != artifacts.mint_finality_protocol_digest(KagemushaPastaParityV1::Ep)?
        || public_inputs.commit_wrapper_eq_protocol_digest
            != artifacts.commit_wrapper_eq_protocol_digest
        || public_inputs.commit_wrapper_ep_protocol_digest
            != artifacts.commit_wrapper_ep_protocol_digest
        || public_inputs.eq_protocol_digest != proof.eq_protocol_digest
        || public_inputs.ep_protocol_digest != proof.ep_protocol_digest
        || public_inputs.guard_eq_credential_audit != proof.guard_eq_credential_audit
        || public_inputs.guard_ep_credential_audit != proof.guard_ep_credential_audit
        || public_inputs.eq_deferred_audit != proof.eq_deferred_audit
        || public_inputs.ep_deferred_audit != proof.ep_deferred_audit
    {
        return Err(KagemushaRecursionErrorV1::ArtifactSubstitution);
    }
    KagemushaEqAccumulatorV1::try_from_bytes(&proof.eq_history)?;
    KagemushaEpAccumulatorV1::try_from_bytes(&proof.ep_history)?;
    verifier
        .verify_state_proof_and_decide(&KagemushaStateProofVerificationRequestV1 {
            public_inputs,
            proof,
        })
        .map_err(KagemushaRecursionErrorV1::StateProofRejected)
}

/// Verify a final V1 `CommitWrapper` pair against release-pinned artifacts and unlinkable outputs.
///
/// # Errors
///
/// Rejects malformed/cross-parity history, self-selected artifacts, substituted public outputs,
/// oversized proof bodies, unavailable verifier hooks, or any backend proof rejection.
pub fn verify_kagemusha_recursive_proof_v1<V: KagemushaRecursiveVerifierV1>(
    verifier: &V,
    artifacts: KagemushaRecursionArtifactsV1,
    output: KagemushaRecursivePublicOutputV1,
    proof: &KagemushaRedemptionProofV1,
) -> Result<VerifiedKagemushaRecursiveProofV1, KagemushaRecursionErrorV1> {
    artifacts.validate()?;
    output.validate()?;
    proof
        .validate_shape_against(
            output.semantic_digest,
            output.candidate_envelope_digest,
            output.commit_certificate_digest,
        )
        .map_err(|error| KagemushaRecursionErrorV1::WireProof(error.to_string()))?;
    if proof.eq_protocol_digest != artifacts.commit_wrapper_eq_protocol_digest
        || proof.ep_protocol_digest != artifacts.commit_wrapper_ep_protocol_digest
        || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fp>(
            proof.eq_protocol_digest,
        )
        .is_none()
        || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fq>(
            proof.ep_protocol_digest,
        )
        .is_none()
        || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fp>(
            proof.eq_deferred_audit,
        )
        .is_none()
        || crate::zk::kagemusha_v1_poseidon::decode::<halo2_proofs::halo2curves::pasta::Fq>(
            proof.ep_deferred_audit,
        )
        .is_none()
    {
        return Err(KagemushaRecursionErrorV1::ArtifactSubstitution);
    }
    let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&proof.eq_history)?;
    let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&proof.ep_history)?;
    let requests = [
        KagemushaParityVerificationRequestV1 {
            parity: KagemushaPastaParityV1::Eq,
            protocol_digest: artifacts.commit_wrapper_eq_protocol_digest,
            public_output: &output,
            eq_deferred_audit: proof.eq_deferred_audit,
            ep_deferred_audit: proof.ep_deferred_audit,
            current_proof: &proof.eq_proof,
            history_accumulator: eq_history.as_bytes(),
        },
        KagemushaParityVerificationRequestV1 {
            parity: KagemushaPastaParityV1::Ep,
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
            .verify_terminal_authorization_and_decide(request)
            .map_err(
                |reason| KagemushaRecursionErrorV1::TransitionProofRejected {
                    parity: request.parity,
                    reason,
                },
            )?;
    }

    Ok(VerifiedKagemushaRecursiveProofV1 { output })
}

/// Recursively verify and seal one exact chain-facing redemption request.
///
/// Chain execution derives the statement digest and verifies the final wrapper against the
/// authenticated release. Private predecessor, successor, credential, lane, epoch, and journal
/// witnesses never enter the redemption transport.
///
/// The returned opaque capability owns the byte-exact request. Reserve admission should consume
/// it with [`VerifiedKagemushaRedemptionProofV1::into_request`]; structural
/// [`KagemushaRedemptionRequestV1::validate`] alone must never reach reserve accounting.
///
/// # Errors
///
/// Rejects an invalid request/signature, wrong authenticated release or artifact manifest,
/// malformed public binding, invalid paired proof, or any governed backend rejection.
pub fn verify_kagemusha_redemption_request_v1<V: KagemushaRecursiveVerifierV1>(
    verifier: &V,
    artifacts: KagemushaRecursionArtifactsV1,
    request: KagemushaRedemptionRequestV1,
) -> Result<VerifiedKagemushaRedemptionProofV1, KagemushaRecursionErrorV1> {
    artifacts.validate()?;
    request
        .validate_shape()
        .map_err(|error| KagemushaRecursionErrorV1::RedemptionBinding(error.to_string()))?;
    let statement = &request.voucher.statement;
    if statement.lifecycle.release_id != artifacts.release_id
        || request.voucher.artifact_manifest_digest != artifacts.artifact_manifest_digest
        || statement.lifecycle.vk_digest == [0; 32]
    {
        return Err(KagemushaRecursionErrorV1::RedemptionBinding(
            "release or artifact manifest identity mismatch".to_owned(),
        ));
    }
    let semantic_digest = statement
        .canonical_digest()
        .map_err(|error| KagemushaRecursionErrorV1::RedemptionBinding(error.to_string()))?;
    let output = KagemushaRecursivePublicOutputV1::new(
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
    let recursive_proof =
        verify_kagemusha_recursive_proof_v1(verifier, artifacts, output, &request.voucher.proof)?;
    let request_digest = request
        .canonical_digest()
        .map_err(|error| KagemushaRecursionErrorV1::RedemptionBinding(error.to_string()))?;
    Ok(VerifiedKagemushaRedemptionProofV1 {
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
pub struct KagemushaReplayInsertWitnessV1 {
    /// Unique finalized mint or staged peer-credit key whose predecessor leaf must be empty.
    pub credit_id: DigestV1,
    /// Digest of the exact staged credit envelope committed by the new leaf.
    pub envelope_digest: DigestV1,
    /// Replay root committed by the consumed aggregate state.
    pub predecessor_root: KagemushaPastaStateCommitmentV1,
    /// Replay root committed by the produced aggregate state.
    pub successor_root: KagemushaPastaStateCommitmentV1,
    /// Exact 256 sibling hashes, ordered root-to-leaf.
    pub siblings_root_to_leaf: [KagemushaPastaStateCommitmentV1; KAGEMUSHA_REPLAY_PATH_DEPTH_V1],
}

impl From<&ConsumedCreditInsertWitnessV1> for KagemushaReplayInsertWitnessV1 {
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

impl KagemushaReplayInsertWitnessV1 {
    /// Validate fields which do not require evaluating the circuit's fixed sparse-Merkle hash.
    ///
    /// # Errors
    ///
    /// Rejects zero identities/roots and a no-op replay-root transition. This does not establish
    /// nonmembership; only the recursive circuit can do so.
    pub fn validate_shape(&self) -> Result<(), KagemushaRecursionErrorV1> {
        if self.credit_id == [0; 32]
            || self.envelope_digest == [0; 32]
            || self.predecessor_root.is_zero()
            || self.successor_root.is_zero()
            || self.predecessor_root == self.successor_root
        {
            return Err(KagemushaRecursionErrorV1::InvalidReplayWitness);
        }
        Ok(())
    }
}

/// Structural, binding, accumulation, or governed-verifier failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum KagemushaRecursionErrorV1 {
    /// A wire or statement version was not the sole V1 value.
    #[error("unsupported Kagemusha recursion version")]
    UnsupportedVersion,
    /// A canonical field scalar was malformed.
    #[error("non-canonical {parity:?} accumulator scalar at round {round}")]
    NonCanonicalAccumulatorScalar {
        /// Parity whose scalar failed to decode.
        parity: KagemushaPastaParityV1,
        /// Zero-based IPA round.
        round: usize,
    },
    /// A canonical compressed curve point was malformed or the identity.
    #[error("invalid {0:?} accumulator point")]
    InvalidAccumulatorPoint(KagemushaPastaParityV1),
    /// A history accumulator had a length other than exactly 544 bytes.
    #[error("invalid {parity:?} accumulator length {actual}; expected {expected}")]
    InvalidAccumulatorLength {
        /// Parity whose accumulator was malformed.
        parity: KagemushaPastaParityV1,
        /// Observed byte length.
        actual: usize,
        /// Required byte length.
        expected: usize,
    },
    /// A native accumulator did not contain exactly sixteen IPA challenges.
    #[error("invalid {parity:?} native accumulator round count {actual}; expected 16")]
    InvalidAccumulatorRounds {
        /// Parity whose native accumulator was malformed.
        parity: KagemushaPastaParityV1,
        /// Observed number of challenges.
        actual: usize,
    },
    /// A fold transcript had a non-fixed byte length.
    #[error("invalid {parity:?} fold proof length {actual}; expected {expected}")]
    InvalidFoldProofLength {
        /// Parity whose fold proof was malformed.
        parity: KagemushaPastaParityV1,
        /// Observed byte length.
        actual: usize,
        /// Required byte length.
        expected: usize,
    },
    /// IPA parameters did not use the fixed `k = 16` profile.
    #[error("invalid {parity:?} IPA parameter exponent {actual}; expected 16")]
    InvalidIpaParameters {
        /// Parity whose parameters were malformed.
        parity: KagemushaPastaParityV1,
        /// Observed exponent.
        actual: u32,
    },
    /// Native BGH19 proof creation failed.
    #[error("failed to create {parity:?} accumulator fold: {reason}")]
    FoldCreation {
        /// Parity whose fold failed.
        parity: KagemushaPastaParityV1,
        /// Backend reason.
        reason: String,
    },
    /// Native BGH19 proof verification or terminal decision failed.
    #[error("failed to verify or decide {parity:?} accumulator fold: {reason}")]
    FoldDecision {
        /// Parity whose fold failed.
        parity: KagemushaPastaParityV1,
        /// Backend reason.
        reason: String,
    },
    /// A verified fold did not produce the claimed successor accumulator.
    #[error("{0:?} fold successor accumulator was substituted")]
    FoldSuccessorSubstitution(KagemushaPastaParityV1),
    /// A native verifier panicked while rejecting malformed proof material.
    #[error("{0:?} native verifier rejected malformed proof material")]
    NativeVerifierPanic(KagemushaPastaParityV1),
    /// Supplemental guard fields or fixed effects were malformed.
    #[error("invalid Kagemusha GuardBundle context")]
    InvalidGuardContext,
    /// Operation-specific inbox/outbox effects did not match the fixed relation.
    #[error("invalid Kagemusha GuardBundle effects for {0:?}")]
    InvalidGuardEffects(KagemushaOperationV1),
    /// State and hardware statements disagreed on an overlapping field.
    #[error("Kagemusha state and hardware statements disagree")]
    StateHardwareMismatch,
    /// A Core state statement could not be canonically processed.
    #[error("Kagemusha Core state statement failed: {0}")]
    StateStatement(String),
    /// Required nonzero transition bindings were absent or aliased.
    #[error("invalid Kagemusha aggregate transition statement")]
    InvalidTransitionStatement,
    /// Logical sequence or journal successor was not exact-next.
    #[error("Kagemusha transition successor is not exact-next")]
    NonExactSuccessor,
    /// Logical sequence increment overflowed.
    #[error("Kagemusha logical sequence overflow")]
    SequenceOverflow,
    /// Journal revision increment overflowed.
    #[error("Kagemusha journal revision overflow")]
    JournalOverflow,
    /// Hardware epoch generation increment overflowed.
    #[error("Kagemusha hardware epoch overflow")]
    EpochOverflow,
    /// Hardware epoch/key rotation violated its fixed relation.
    #[error("invalid Kagemusha hardware rotation")]
    InvalidRotation,
    /// Canonical statement encoding failed.
    #[error("canonical Kagemusha recursion encoding failed: {0}")]
    Codec(String),
    /// A platform length could not be represented in the canonical digest frame.
    #[error("Kagemusha recursion length overflow")]
    LengthOverflow,
    /// Common recursive public outputs were zero or structurally inconsistent.
    #[error("invalid Kagemusha recursive public output")]
    InvalidPublicOutput,
    /// Release-pinned artifact identities were missing or aliased.
    #[error("invalid Kagemusha recursive artifact set")]
    InvalidArtifacts,
    /// Normalized guard and recursive public outputs did not bind identically.
    #[error("Kagemusha recursive public binding mismatch")]
    PublicBindingMismatch,
    /// A typed payment or redemption statement was invalid or did not match its guard statement.
    #[error("invalid Kagemusha typed transport binding: {0}")]
    TransportBinding(String),
    /// Canonical paired-proof validation failed.
    #[error("invalid Kagemusha paired proof: {0}")]
    WireProof(String),
    /// A mint credit did not match its statement, release, distinct helper roles, or manifest.
    #[error("invalid Kagemusha mint-finality helper binding: {0}")]
    MintFinalityBinding(String),
    /// A redemption request did not match its release, manifest, or recursive public instance.
    #[error("invalid Kagemusha redemption proof binding: {0}")]
    RedemptionBinding(String),
    /// The governed paired mint-finality helper verifier rejected.
    #[error("Kagemusha mint-finality helper proof rejected: {0}")]
    MintFinalityProofRejected(String),
    /// The governed paired aggregate-state verifier rejected before terminal commit.
    #[error("Kagemusha aggregate-state proof rejected: {0}")]
    StateProofRejected(String),
    /// The governed paired post-commit payment verifier rejected.
    #[error("Kagemusha post-commit payment proof rejected: {0}")]
    PaymentProofRejected(String),
    /// The proof carried a protocol identity not selected by the trusted release.
    #[error("Kagemusha recursive protocol artifact substitution")]
    ArtifactSubstitution,
    /// Recursive `GuardBundle` helper verification failed.
    #[error("{parity:?} GuardBundle helper proof rejected: {reason}")]
    GuardProofRejected {
        /// Parity whose helper verification failed.
        parity: KagemushaPastaParityV1,
        /// Governed backend reason.
        reason: String,
    },
    /// Recursive transition/public-output verification or terminal decision failed.
    #[error("{parity:?} transition proof rejected: {reason}")]
    TransitionProofRejected {
        /// Parity whose transition verification failed.
        parity: KagemushaPastaParityV1,
        /// Governed backend reason.
        reason: String,
    },
    /// A mint/receive replay-insert witness lacked required structural bindings.
    #[error("invalid Kagemusha replay-insert witness")]
    InvalidReplayWitness,
}

const _: () = {
    assert!(KAGEMUSHA_RECURSION_IPA_K_V1 == 16);
    assert!(KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1 == 544);
    assert!(KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1 == 2_495);
    assert!(KAGEMUSHA_CURRENT_PROOFS_MAX_BYTES_V1 == 4_990);
    assert!(KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1 == 6_528);
};
