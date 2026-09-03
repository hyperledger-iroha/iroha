//! Deterministic fixed-k artifact generation and proving for the recursive aggregate state.
//!
//! The emitted files use exactly the raw formats authenticated by the V1 release manifest:
//! `ParamsIPA::write` and Halo2 `SerdeFormat::Processed`. State-role keys are derived only from
//! the complete recursive circuit: the seven-operation balance relation, predecessor recursion,
//! delayed-history fold, mint-finality helper, normalized hardware GuardBundle, and reciprocal
//! Pasta equation audit are one inseparable proving authority. Device proof writers require a
//! secret recovery seed authenticated by the qualified provider. Reconstructing identical bytes
//! additionally requires the same immutable private witness, input proofs, and release artifacts;
//! a deterministic RNG alone does not implement the hardware recovery orchestration.

use std::{
    io::{self, Cursor},
    sync::Arc,
};

#[cfg(feature = "zk-halo2-ipa")]
use ff::{FromUniformBytes, WithSmallOrderMulGroup};
#[cfg(feature = "zk-halo2-ipa")]
use halo2_base::gates::circuit::BaseCircuitParams;
use halo2_proofs::{
    SerdeFormat,
    halo2curves::{
        CurveAffine,
        group::{GroupEncoding as _, prime::PrimeCurveAffine as _},
        pasta::{EpAffine, EqAffine, Fp, Fq},
    },
    plonk::{
        Circuit as _, ProvingKey, VerifyingKey, create_proof, keygen_pk, keygen_vk, verify_proof,
    },
    poly::{
        VerificationStrategy,
        commitment::{MSM as _, Params as _, ParamsProver as _},
        ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            msm::MSMIPA,
            multiopen::{ProverIPA, VerifierIPA},
            strategy::GuardIPA,
        },
    },
};
#[cfg(feature = "zk-halo2-ipa")]
use iroha_crypto::kagemusha::KagemushaRecoverySeedV1;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_HALO2_K_V1, KAGEMUSHA_PARAMS_BYTES_V1, KAGEMUSHA_STATE_PROVING_KEY_MAX_BYTES_V1,
    KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1, KagemushaArtifactBindingV1, KagemushaArtifactRoleV1,
};
#[cfg(feature = "zk-halo2-ipa")]
use iroha_data_model::kagemusha::{
    KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1, KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1,
    KAGEMUSHA_WIRE_VERSION_V1, KagemushaAuthenticatedReleaseV1, KagemushaCommitCertificateV1,
    KagemushaHardwareCredentialV1, KagemushaHardwareProfileV1, KagemushaLifecycleBindingV1,
    KagemushaOutboxReservationV1, KagemushaPairedProofV1, KagemushaPaymentOutputV1,
    KagemushaPaymentProofV1, KagemushaPaymentRequestV1, KagemushaQualifiedHelperCircuitV1,
    KagemushaQualifiedRelationV1, KagemushaRedemptionProofV1,
};
#[cfg(feature = "zk-halo2-ipa")]
use rand_core_06::OsRng;
use sha2::{Digest as _, Sha256};
#[cfg(feature = "zk-halo2-ipa")]
use snark_verifier::{
    loader::native::NativeLoader,
    system::halo2::{
        compile,
        transcript::halo2::{ChallengeScalar, PoseidonTranscript},
    },
    verifier::plonk::PlonkProtocol,
};
use thiserror::Error;

#[cfg(feature = "zk-halo2-ipa")]
#[path = "artifact_resource_preflight.rs"]
mod artifact_resource_preflight;
#[cfg(feature = "zk-halo2-ipa")]
#[path = "mint_authority_generation.rs"]
mod mint_authority_generation;
#[cfg(feature = "zk-halo2-ipa")]
use artifact_resource_preflight::keygen_vk_with_helper_resource_preflight_v1;
#[cfg(feature = "zk-halo2-ipa")]
pub use mint_authority_generation::prove_kagemusha_mint_authority_v1;

#[cfg(all(test, feature = "zk-halo2-ipa"))]
#[path = "mint_authority_generation_tests.rs"]
mod mint_authority_generation_tests;
#[cfg(feature = "zk-halo2-ipa")]
use mint_authority_generation::{
    KagemushaPreparedMintAuthorityTransportV1, read_ep_inner_mint_vk, read_eq_inner_mint_vk,
};

#[cfg(feature = "zk-halo2-ipa")]
use super::terminal_authorization::{
    KagemushaCommitWrapperEpWitnessV1, KagemushaCommitWrapperEqWitnessV1,
};
#[cfg(feature = "zk-halo2-ipa")]
use super::{
    KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1, KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_RATE_V1, KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1,
    KAGEMUSHA_IPA_POSEIDON_WIDTH_V1, KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1,
    KagemushaCommitWrapperEpCircuitV1, KagemushaCommitWrapperEqCircuitV1,
    KagemushaCommitWrapperWitnessV1, KagemushaEpAccumulatorV1, KagemushaEpFoldProofV1,
    KagemushaEqAccumulatorV1, KagemushaEqFoldProofV1, KagemushaGuardBundleRelationWitnessV1,
    KagemushaOperationV1, KagemushaStateRelationWitnessV1,
    KagemushaTerminalAuthorizationEpCircuitV1, KagemushaTerminalAuthorizationEpWitnessV1,
    KagemushaTerminalAuthorizationEqCircuitV1, KagemushaTerminalAuthorizationEqWitnessV1,
    KagemushaTerminalAuthorizationPrivateTransitionV1,
    KagemushaTerminalAuthorizationPublicInputsV1, KagemushaTerminalAuthorizationWitnessV1,
    TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1,
    TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1,
    accumulation::{
        fold_kagemusha_ep_accumulators_with_rng_v1, fold_kagemusha_eq_accumulators_with_rng_v1,
    },
    build_kagemusha_commit_wrapper_pair_v1, build_kagemusha_terminal_authorization_pair_v1,
    composite::{
        KagemushaRecursiveIncomingEpWitnessV1 as CompositeIncomingEpWitnessV1,
        KagemushaRecursiveIncomingEqWitnessV1 as CompositeIncomingEqWitnessV1,
        KagemushaRecursiveStateEpCircuitV1, KagemushaRecursiveStateEqCircuitV1,
        KagemushaRecursiveStateWitnessV1, KagemushaSuiteUpgradeBridgeWitnessV1,
        build_kagemusha_recursive_state_pair_v1,
    },
    decide_kagemusha_ep_accumulator_v1, decide_kagemusha_eq_accumulator_v1,
    deferred_parent::{
        KagemushaOrdinaryProofProfileV1, kagemusha_protocol_structure_digest_v1,
        native_parent_protocol_digest_v1, ordinary_ipa_proof_profile_v1,
    },
    fold_kagemusha_ep_accumulators_v1, fold_kagemusha_eq_accumulators_v1,
    guard_bundle::KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1,
    initial_kagemusha_ep_accumulator_v1, initial_kagemusha_eq_accumulator_v1,
    mint_authority::{
        KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1, KagemushaMintAuthorityCheckpointV1,
        KagemushaMintAuthorityEpCircuitV1, KagemushaMintAuthorityEqCircuitV1,
        KagemushaMintAuthorityPairBindingV1, KagemushaMintAuthorityPairWitnessV1,
        KagemushaMintAuthorityParityWitnessV1, build_kagemusha_mint_authority_pair_v1,
        public_instance as mint_authority_public_instance,
    },
    mint_authorization::{
        KagemushaMintAuthorizationEpCircuitV1, KagemushaMintAuthorizationEqCircuitV1,
        KagemushaMintAuthorizationRecursiveWitnessV1, KagemushaMintAuthorizationRelationWitnessV1,
        MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1, build_kagemusha_mint_authorization_pair_v1,
        mint_authorization_public_instances_v1,
    },
    mint_helper::{KagemushaMintAuthorityStepV1, KagemushaMintCertificateWitnessV1},
    mint_transport_decider::{
        KagemushaMintAuthorityTransportEpCircuitV1, KagemushaMintAuthorityTransportEqCircuitV1,
        KagemushaMintAuthorizationTransportEpCircuitV1,
        KagemushaMintAuthorizationTransportEqCircuitV1, KagemushaMintTransportDeciderWitnessV1,
        KagemushaMintTransportParityWitnessV1, build_kagemusha_mint_authority_transport_pair_v1,
        build_kagemusha_mint_authorization_transport_pair_v1,
    },
    native_backend::{verify_ep_succinct_protocol, verify_eq_succinct_protocol},
    state_relation::PUBLIC_INSTANCE_COUNT,
    transport_decider::{
        KagemushaTransportDeciderCapacityProfileV1, KagemushaTransportDeciderEpCircuitV1,
        KagemushaTransportDeciderEqCircuitV1, KagemushaTransportDeciderParityWitnessV1,
        KagemushaTransportDeciderWitnessV1, build_kagemusha_transport_decider_pair_v1,
    },
};
use super::{
    KagemushaArtifactByteResolverV1, KagemushaArtifactErrorV1, KagemushaAuthenticatedArtifactSetV1,
    KagemushaMemoryArtifactResolverV1, KagemushaPastaParityV1,
};
#[cfg(feature = "zk-halo2-ipa")]
use crate::zk::{
    kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, encode, from_u128},
    kagemusha_v1_state::{KagemushaMintFoldOpeningCapabilityV1, KagemushaStateV1},
};

#[cfg(feature = "zk-halo2-ipa")]
use super::terminal_authorization::{
    KagemushaCommitEvidenceOpeningV1, KagemushaTerminalSendPrivateV1,
};

/// Stable source-level schema identity of the executable native state relation.
pub const KAGEMUSHA_OPERATION_RELATION_SCHEMA_ID_V1: &str =
    "iroha:kagemusha:v1:aggregate-state-pasta-poseidon-256:k16";

/// Deterministic parameter, proving-key, and verifying-key bytes for one state parity.
#[derive(Clone, Debug)]
pub struct KagemushaGeneratedOperationArtifactsV1 {
    /// Non-interchangeable generated parity.
    pub parity: KagemushaPastaParityV1,
    /// Canonical transparent IPA parameter bytes.
    pub parameters: Arc<[u8]>,
    /// Processed native state proving key.
    pub proving_key: Arc<[u8]>,
    /// Processed native state verifying key.
    pub verifying_key: Arc<[u8]>,
}

impl KagemushaGeneratedOperationArtifactsV1 {
    /// Return the three exact manifest bindings selected by this parity.
    #[must_use]
    pub fn bindings(&self) -> [KagemushaArtifactBindingV1; 3] {
        let (params_role, proving_role, verifying_role) = match self.parity {
            KagemushaPastaParityV1::Eq => (
                KagemushaArtifactRoleV1::ParamsEq,
                KagemushaArtifactRoleV1::StatePkEq,
                KagemushaArtifactRoleV1::StateVkEq,
            ),
            KagemushaPastaParityV1::Ep => (
                KagemushaArtifactRoleV1::ParamsEp,
                KagemushaArtifactRoleV1::StatePkEp,
                KagemushaArtifactRoleV1::StateVkEp,
            ),
        };
        [
            binding(params_role, self.parameters.as_ref()),
            binding(proving_role, self.proving_key.as_ref()),
            binding(verifying_role, self.verifying_key.as_ref()),
        ]
    }

    /// Install all three files into an embedded content-addressed resolver.
    pub fn install_into(&self, resolver: &mut KagemushaMemoryArtifactResolverV1) {
        resolver.insert(Arc::clone(&self.parameters));
        resolver.insert(Arc::clone(&self.proving_key));
        resolver.insert(Arc::clone(&self.verifying_key));
    }

    /// Return the private recursive-carrier bindings selected by this parity.
    #[cfg(feature = "zk-halo2-ipa")]
    #[must_use]
    pub fn inner_state_bindings(&self) -> [KagemushaArtifactBindingV1; 3] {
        let (params_role, proving_role, verifying_role) = match self.parity {
            KagemushaPastaParityV1::Eq => (
                KagemushaArtifactRoleV1::ParamsEq,
                KagemushaArtifactRoleV1::InnerStatePkEq,
                KagemushaArtifactRoleV1::InnerStateVkEq,
            ),
            KagemushaPastaParityV1::Ep => (
                KagemushaArtifactRoleV1::ParamsEp,
                KagemushaArtifactRoleV1::InnerStatePkEp,
                KagemushaArtifactRoleV1::InnerStateVkEp,
            ),
        };
        [
            binding(params_role, self.parameters.as_ref()),
            binding(proving_role, self.proving_key.as_ref()),
            binding(verifying_role, self.verifying_key.as_ref()),
        ]
    }
}

/// One fixed-shape transported post-commit payment proof slot consumed by
/// `ReceiveFold`.
///
/// Every referenced protocol and proof is consumed inside the recursive circuit; host-side
/// verification is not a substitute for this witness.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Copy)]
pub struct KagemushaRecursiveIncomingEqGenerationWitnessV1<'a> {
    /// Eq post-commit payment public instances for this fixed slot.
    pub instances: &'a [Vec<Fp>],
    /// Eq message-4 payment proof or release-pinned inactive proof.
    pub proof: &'a [u8],
    /// Eq delayed history committed by the incoming authorization proof.
    pub history: &'a KagemushaEqAccumulatorV1,
    /// Eq proof completing the incoming sender history.
    pub history_fold_proof: &'a KagemushaEqFoldProofV1,
    /// Eq proof merging the complete incoming history into the receiver history.
    pub merge_fold_proof: &'a KagemushaEqFoldProofV1,
}

#[cfg(feature = "zk-halo2-ipa")]
impl<'a> KagemushaRecursiveIncomingEqGenerationWitnessV1<'a> {
    fn into_composite(self) -> CompositeIncomingEqWitnessV1<'a> {
        CompositeIncomingEqWitnessV1 {
            instances: self.instances,
            proof: self.proof,
            history: self.history,
            history_fold_proof: self.history_fold_proof,
            merge_fold_proof: self.merge_fold_proof,
        }
    }
}

/// One fixed Ep/Fq incoming post-commit payment proof slot for `ReceiveFold`.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Copy)]
pub struct KagemushaRecursiveIncomingEpGenerationWitnessV1<'a> {
    /// Ep post-commit payment public instances for this fixed slot.
    pub instances: &'a [Vec<Fq>],
    /// Ep message-4 payment proof or release-pinned inactive proof.
    pub proof: &'a [u8],
    /// Ep delayed history committed by the incoming authorization proof.
    pub history: &'a KagemushaEpAccumulatorV1,
    /// Ep proof completing the incoming sender history.
    pub history_fold_proof: &'a KagemushaEpFoldProofV1,
    /// Ep proof merging the complete incoming history into the receiver history.
    pub merge_fold_proof: &'a KagemushaEpFoldProofV1,
}

#[cfg(feature = "zk-halo2-ipa")]
impl<'a> KagemushaRecursiveIncomingEpGenerationWitnessV1<'a> {
    fn into_composite(self) -> CompositeIncomingEpWitnessV1<'a> {
        CompositeIncomingEpWitnessV1 {
            instances: self.instances,
            proof: self.proof,
            history: self.history,
            history_fold_proof: self.history_fold_proof,
            merge_fold_proof: self.merge_fold_proof,
        }
    }
}

/// Exact old-suite authorization carried only by a `SuiteUpgrade` transition.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaSuiteUpgradeGenerationWitnessV1 {
    /// Threshold-authenticated release identifier consumed by the old parent.
    pub old_release_id: [u8; 32],
    /// Threshold-authenticated release identifier installed for the successor.
    pub new_release_id: [u8; 32],
    /// Compiled Eq protocol digest of the consumed old suite.
    pub old_eq_protocol_digest: [u8; 32],
    /// Compiled Ep protocol digest of the consumed old suite.
    pub old_ep_protocol_digest: [u8; 32],
    /// Consumed old suite identifier.
    pub old_suite_id: [u8; 32],
    /// Digest of the consumed old verifying-key set.
    pub old_vk_digest: [u8; 32],
    /// Installed successor suite identifier.
    pub new_suite_id: [u8; 32],
    /// Digest of the installed successor verifying-key set.
    pub new_vk_digest: [u8; 32],
    /// Authenticated governance authorization proof digest.
    pub governance_authorization_proof_digest: [u8; 32],
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaSuiteUpgradeGenerationWitnessV1 {
    /// Canonical inactive witness required for every non-upgrade operation.
    pub const ZERO: Self = Self {
        old_release_id: [0; 32],
        new_release_id: [0; 32],
        old_eq_protocol_digest: [0; 32],
        old_ep_protocol_digest: [0; 32],
        old_suite_id: [0; 32],
        old_vk_digest: [0; 32],
        new_suite_id: [0; 32],
        new_vk_digest: [0; 32],
        governance_authorization_proof_digest: [0; 32],
    };

    const fn into_composite(self) -> KagemushaSuiteUpgradeBridgeWitnessV1 {
        KagemushaSuiteUpgradeBridgeWitnessV1 {
            old_release_id: self.old_release_id,
            new_release_id: self.new_release_id,
            old_eq_protocol_digest: self.old_eq_protocol_digest,
            old_ep_protocol_digest: self.old_ep_protocol_digest,
            old_suite_id: self.old_suite_id,
            old_vk_digest: self.old_vk_digest,
            new_suite_id: self.new_suite_id,
            new_vk_digest: self.new_vk_digest,
            governance_authorization_proof_digest: self.governance_authorization_proof_digest,
        }
    }
}

/// Complete private input needed to build both production recursive state circuits.
///
/// The sixteen incoming slots are always present. `receive_active_count` in the state witness
/// selects an active prefix; every remaining position carries the release-pinned valid padding
/// proof and history, so proof shape never depends on receipt count.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone)]
pub struct KagemushaRecursiveStateGenerationWitnessV1<'a> {
    /// Seven-operation aggregate-state transition witness.
    pub state: KagemushaStateRelationWitnessV1,
    /// Checked-preview mint opening, present exactly for `MintFold`.
    pub mint_fold_opening: Option<KagemushaMintFoldOpeningCapabilityV1<'a>>,
    /// Normalized hardware guard semantics constrained into the state proof.
    pub guard_relation: KagemushaGuardBundleRelationWitnessV1,
    /// Old-parent verifier bridge; canonical zero outside `SuiteUpgrade`.
    pub suite_upgrade_bridge: KagemushaSuiteUpgradeGenerationWitnessV1,
    /// Eq predecessor state protocol compiled from the authenticated predecessor state key.
    pub eq_parent_protocol: &'a PlonkProtocol<EqAffine>,
    /// Ep predecessor state protocol compiled from the authenticated predecessor state key.
    pub ep_parent_protocol: &'a PlonkProtocol<EpAffine>,
    /// Eq predecessor public instances.
    pub eq_parent_instances: &'a [Vec<Fp>],
    /// Ep predecessor public instances.
    pub ep_parent_instances: &'a [Vec<Fq>],
    /// Eq predecessor recursive proof.
    pub eq_parent_proof: &'a [u8],
    /// Ep predecessor recursive proof.
    pub ep_parent_proof: &'a [u8],
    /// Eq delayed-history accumulator before this transition.
    pub eq_predecessor_history: &'a KagemushaEqAccumulatorV1,
    /// Ep delayed-history accumulator before this transition.
    pub ep_predecessor_history: &'a KagemushaEpAccumulatorV1,
    /// Eq proof that folds the predecessor proof into delayed history.
    pub eq_parent_fold_proof: &'a KagemushaEqFoldProofV1,
    /// Ep proof that folds the predecessor proof into delayed history.
    pub ep_parent_fold_proof: &'a KagemushaEpFoldProofV1,
    /// Eq message-4 CommitWrapper protocol compiled from the authenticated release key.
    pub eq_incoming_protocol: &'a PlonkProtocol<EqAffine>,
    /// Ep message-4 CommitWrapper protocol compiled from the authenticated release key.
    pub ep_incoming_protocol: &'a PlonkProtocol<EpAffine>,
    /// Fixed sixteen Eq authorization proof/history slots.
    pub eq_incoming_slots: [KagemushaRecursiveIncomingEqGenerationWitnessV1<'a>; 16],
    /// Fixed sixteen Ep authorization proof/history slots.
    pub ep_incoming_slots: [KagemushaRecursiveIncomingEpGenerationWitnessV1<'a>; 16],
    /// Eq delayed-history accumulator carried by the successor.
    pub eq_successor_history: &'a KagemushaEqAccumulatorV1,
    /// Ep delayed-history accumulator carried by the successor.
    pub ep_successor_history: &'a KagemushaEpAccumulatorV1,
    /// Eq normalized GuardBundle protocol compiled from the authenticated helper key.
    pub eq_guard_protocol: &'a PlonkProtocol<EqAffine>,
    /// Ep normalized GuardBundle protocol compiled from the authenticated helper key.
    pub ep_guard_protocol: &'a PlonkProtocol<EpAffine>,
    /// Eq normalized GuardBundle proof.
    pub eq_guard_proof: &'a [u8],
    /// Ep normalized GuardBundle proof.
    pub ep_guard_proof: &'a [u8],
    /// Eq GuardBundle internal credential history committed by the Guard proof.
    pub eq_guard_history: &'a KagemushaEqAccumulatorV1,
    /// Ep GuardBundle internal credential history committed by the Guard proof.
    pub ep_guard_history: &'a KagemushaEpAccumulatorV1,
    /// Eq proof folding the Guard proof's current opening claim with its credential history.
    pub eq_guard_history_fold_proof: &'a KagemushaEqFoldProofV1,
    /// Ep proof folding the Guard proof's current opening claim with its credential history.
    pub ep_guard_history_fold_proof: &'a KagemushaEpFoldProofV1,
    /// Eq proof merging the complete Guard proof into the state history.
    pub eq_guard_merge_fold_proof: &'a KagemushaEqFoldProofV1,
    /// Ep proof merging the complete Guard proof into the state history.
    pub ep_guard_merge_fold_proof: &'a KagemushaEpFoldProofV1,
    /// Eq finalized-mint helper protocol compiled from its authenticated key.
    pub eq_mint_protocol: &'a PlonkProtocol<EqAffine>,
    /// Ep finalized-mint helper protocol compiled from its authenticated key.
    pub ep_mint_protocol: &'a PlonkProtocol<EpAffine>,
    /// Eq finalized-mint helper public instances, including its complete authority history.
    pub eq_mint_instances: &'a [Vec<Fp>],
    /// Ep finalized-mint helper public instances, including its complete authority history.
    pub ep_mint_instances: &'a [Vec<Fq>],
    /// Eq finalized-mint helper proof, selector-enabled only for `MintFold`.
    pub eq_mint_proof: &'a [u8],
    /// Ep finalized-mint helper proof, selector-enabled only for `MintFold`.
    pub ep_mint_proof: &'a [u8],
    /// Eq authority history committed by the finalized-mint helper proof.
    pub eq_mint_history: &'a KagemushaEqAccumulatorV1,
    /// Ep authority history committed by the finalized-mint helper proof.
    pub ep_mint_history: &'a KagemushaEpAccumulatorV1,
    /// Eq proof folding the mint helper's current opening claim with its authority history.
    pub eq_mint_history_fold_proof: &'a KagemushaEqFoldProofV1,
    /// Ep proof folding the mint helper's current opening claim with its authority history.
    pub ep_mint_history_fold_proof: &'a KagemushaEpFoldProofV1,
    /// Eq proof merging the complete mint helper into the state history.
    pub eq_mint_merge_fold_proof: &'a KagemushaEqFoldProofV1,
    /// Ep proof merging the complete mint helper into the state history.
    pub ep_mint_merge_fold_proof: &'a KagemushaEpFoldProofV1,
}

#[cfg(feature = "zk-halo2-ipa")]
impl<'a> KagemushaRecursiveStateGenerationWitnessV1<'a> {
    fn into_recursive<'b>(
        self,
        eq_incoming_slots: &'b [CompositeIncomingEqWitnessV1<'b>; 16],
        ep_incoming_slots: &'b [CompositeIncomingEpWitnessV1<'b>; 16],
    ) -> KagemushaRecursiveStateWitnessV1<'b>
    where
        'a: 'b,
    {
        KagemushaRecursiveStateWitnessV1 {
            state: self.state,
            mint_fold_opening: self
                .mint_fold_opening
                .map(KagemushaMintFoldOpeningCapabilityV1::opening),
            guard_relation: self.guard_relation,
            suite_upgrade_bridge: self.suite_upgrade_bridge.into_composite(),
            eq_parent_protocol: self.eq_parent_protocol,
            ep_parent_protocol: self.ep_parent_protocol,
            eq_parent_instances: self.eq_parent_instances,
            ep_parent_instances: self.ep_parent_instances,
            eq_parent_proof: self.eq_parent_proof,
            ep_parent_proof: self.ep_parent_proof,
            eq_predecessor_history: self.eq_predecessor_history,
            ep_predecessor_history: self.ep_predecessor_history,
            eq_parent_fold_proof: self.eq_parent_fold_proof,
            ep_parent_fold_proof: self.ep_parent_fold_proof,
            eq_incoming_protocol: self.eq_incoming_protocol,
            ep_incoming_protocol: self.ep_incoming_protocol,
            eq_incoming_slots,
            ep_incoming_slots,
            eq_successor_history: self.eq_successor_history,
            ep_successor_history: self.ep_successor_history,
            eq_guard_protocol: self.eq_guard_protocol,
            ep_guard_protocol: self.ep_guard_protocol,
            eq_guard_proof: self.eq_guard_proof,
            ep_guard_proof: self.ep_guard_proof,
            eq_guard_history: self.eq_guard_history,
            ep_guard_history: self.ep_guard_history,
            eq_guard_history_fold_proof: self.eq_guard_history_fold_proof,
            ep_guard_history_fold_proof: self.ep_guard_history_fold_proof,
            eq_guard_merge_fold_proof: self.eq_guard_merge_fold_proof,
            ep_guard_merge_fold_proof: self.ep_guard_merge_fold_proof,
            eq_mint_protocol: self.eq_mint_protocol,
            ep_mint_protocol: self.ep_mint_protocol,
            eq_mint_instances: self.eq_mint_instances,
            ep_mint_instances: self.ep_mint_instances,
            eq_mint_proof: self.eq_mint_proof,
            ep_mint_proof: self.ep_mint_proof,
            eq_mint_history: self.eq_mint_history,
            ep_mint_history: self.ep_mint_history,
            eq_mint_history_fold_proof: self.eq_mint_history_fold_proof,
            ep_mint_history_fold_proof: self.ep_mint_history_fold_proof,
            eq_mint_merge_fold_proof: self.eq_mint_merge_fold_proof,
            ep_mint_merge_fold_proof: self.ep_mint_merge_fold_proof,
        }
    }
}

/// Generated Eq/Ep state-role artifacts and the exact authenticated circuit identities.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug)]
pub struct KagemushaGeneratedRecursiveStateArtifactsV1 {
    /// Eq/Fp parameter and compact outer transport-decider key bytes.
    pub eq: KagemushaGeneratedOperationArtifactsV1,
    /// Ep/Fq parameter and compact outer transport-decider key bytes.
    pub ep: KagemushaGeneratedOperationArtifactsV1,
    /// Eq/Fp parameter and private recursive aggregate-state carrier key bytes.
    pub inner_eq: KagemushaGeneratedOperationArtifactsV1,
    /// Ep/Fq parameter and private recursive aggregate-state carrier key bytes.
    pub inner_ep: KagemushaGeneratedOperationArtifactsV1,
    /// Exact Eq outer-decider layout required to decode the processed key.
    pub eq_circuit_params: BaseCircuitParams,
    /// Exact Ep outer-decider layout required to decode the processed key.
    pub ep_circuit_params: BaseCircuitParams,
    /// Exact Eq inner-carrier layout required to decode the processed key.
    pub inner_eq_circuit_params: BaseCircuitParams,
    /// Exact Ep inner-carrier layout required to decode the processed key.
    pub inner_ep_circuit_params: BaseCircuitParams,
    /// Eq compiled outer transport-decider identity committed by every payment proof.
    pub eq_protocol_digest: [u8; 32],
    /// Ep compiled outer transport-decider identity committed by every payment proof.
    pub ep_protocol_digest: [u8; 32],
    /// Eq compiled private recursive-carrier identity.
    pub inner_eq_protocol_digest: [u8; 32],
    /// Ep compiled private recursive-carrier identity.
    pub inner_ep_protocol_digest: [u8; 32],
    /// Measured Eq outer-decider row and cell inventory.
    pub(super) eq_transport_capacity: KagemushaTransportDeciderCapacityProfileV1,
    /// Measured Ep outer-decider row and cell inventory.
    pub(super) ep_transport_capacity: KagemushaTransportDeciderCapacityProfileV1,
}

/// Loaded Eq production recursive-state parameters and keys.
#[cfg(feature = "zk-halo2-ipa")]
pub struct KagemushaLoadedEqRecursiveStateArtifactsV1 {
    /// Threshold-authenticated release which owns these artifacts.
    pub release_id: [u8; 32],
    /// Release-wide proof suite installed for the successor.
    pub suite_id: [u8; 32],
    /// Release-wide verifier-set digest installed for the successor.
    pub vk_digest: [u8; 32],
    /// Canonical Eq transparent IPA parameters.
    pub parameters: ParamsIPA<EqAffine>,
    /// Exact processed Eq compact transport-decider proving key.
    pub proving_key: ProvingKey<EqAffine>,
    /// Exact processed Eq compact transport-decider verifying key.
    pub verifying_key: VerifyingKey<EqAffine>,
    /// Authenticated outer-decider layout used to parse both keys.
    pub circuit_params: BaseCircuitParams,
    /// Exact processed Eq private recursive-carrier proving key.
    pub inner_proving_key: ProvingKey<EqAffine>,
    /// Exact processed Eq private recursive-carrier verifying key.
    pub inner_verifying_key: VerifyingKey<EqAffine>,
    /// Authenticated private-carrier layout used to parse both inner keys.
    pub inner_circuit_params: BaseCircuitParams,
}

/// Loaded Ep production recursive-state parameters and keys.
#[cfg(feature = "zk-halo2-ipa")]
pub struct KagemushaLoadedEpRecursiveStateArtifactsV1 {
    /// Threshold-authenticated release which owns these artifacts.
    pub release_id: [u8; 32],
    /// Release-wide proof suite installed for the successor.
    pub suite_id: [u8; 32],
    /// Release-wide verifier-set digest installed for the successor.
    pub vk_digest: [u8; 32],
    /// Canonical Ep transparent IPA parameters.
    pub parameters: ParamsIPA<EpAffine>,
    /// Exact processed Ep compact transport-decider proving key.
    pub proving_key: ProvingKey<EpAffine>,
    /// Exact processed Ep compact transport-decider verifying key.
    pub verifying_key: VerifyingKey<EpAffine>,
    /// Authenticated outer-decider layout used to parse both keys.
    pub circuit_params: BaseCircuitParams,
    /// Exact processed Ep private recursive-carrier proving key.
    pub inner_proving_key: ProvingKey<EpAffine>,
    /// Exact processed Ep private recursive-carrier verifying key.
    pub inner_verifying_key: VerifyingKey<EpAffine>,
    /// Authenticated private-carrier layout used to parse both inner keys.
    pub inner_circuit_params: BaseCircuitParams,
}

/// One complete constant-size production recursive state proof pair.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaGeneratedRecursiveStateProofV1 {
    /// Eq private-carrier public column required by the next recursive proof.
    pub eq_public_instances: Vec<Fp>,
    /// Ep private-carrier public column required by the next recursive proof.
    pub ep_public_instances: Vec<Fq>,
    /// Eq compact transport-decider public column carried by the envelope.
    pub eq_transport_public_instances: Vec<Fp>,
    /// Ep compact transport-decider public column carried by the envelope.
    pub ep_transport_public_instances: Vec<Fq>,
    /// Eq private-carrier proof required by the next local recursive transition.
    pub eq_inner_proof: Vec<u8>,
    /// Ep private-carrier proof required by the next local recursive transition.
    pub ep_inner_proof: Vec<u8>,
    /// Compact paired-Pasta outer proof carried by payments and redemptions.
    pub proof: KagemushaPairedProofV1,
    /// Eq current opening claim extracted from the generated proof for the next history fold.
    pub eq_current_accumulator: KagemushaEqAccumulatorV1,
    /// Ep current opening claim extracted from the generated proof for the next history fold.
    pub ep_current_accumulator: KagemushaEpAccumulatorV1,
    /// Eq private-carrier history retained locally for the next recursive transition.
    ///
    /// This is deliberately distinct from `proof.eq_history`, which terminally folds this
    /// history with `eq_current_accumulator` before crossing the transport boundary.
    pub eq_history: KagemushaEqAccumulatorV1,
    /// Ep private-carrier history retained locally for the next recursive transition.
    ///
    /// This is deliberately distinct from `proof.ep_history`, which terminally folds this
    /// history with `ep_current_accumulator` before crossing the transport boundary.
    pub ep_history: KagemushaEpAccumulatorV1,
}

/// Fixed release-enabled hardware-profile table width committed by every terminal-authorization key.
#[cfg(feature = "zk-halo2-ipa")]
pub const KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1: usize =
    TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1;

/// Unlinkable post-commit public values used by the internal and transported terminal proofs.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaTerminalAuthorizationTerminalGenerationPublicV1 {
    /// Exact released SendSplit or RedeemSplit lifecycle.
    pub lifecycle: KagemushaLifecycleBindingV1,
    /// Digest of the unlinkable transfer or redemption statement.
    pub semantic_digest: [u8; 32],
    /// Digest of the durably persisted candidate proof envelope.
    pub candidate_envelope_digest: [u8; 32],
    /// Digest of the exact terminal hardware certificate.
    pub commit_certificate_digest: [u8; 32],
    /// Proof-derived send or redemption nullifier.
    pub transition_nullifier: [u8; 32],
    /// Send-only request digest; zero for redemption.
    pub request_digest: [u8; 32],
    /// Send-only receiver hardware binding; zero for redemption.
    pub receiver_binding_digest: [u8; 32],
    /// Send-only ciphertext commitment; zero for redemption.
    pub ciphertext_commitment: [u8; 32],
    /// Positive terminal amount.
    pub amount: u128,
    /// Send recipient/credit binding or redemption commitment.
    pub terminal_output_binding: [u8; 32],
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaTerminalAuthorizationTerminalGenerationPublicV1 {
    fn into_internal(
        self,
        eq_deferred_audit: [u8; 32],
        ep_deferred_audit: [u8; 32],
        eq_protocol_digest: [u8; 32],
        ep_protocol_digest: [u8; 32],
    ) -> Result<KagemushaTerminalAuthorizationPublicInputsV1, String> {
        KagemushaTerminalAuthorizationPublicInputsV1::from_lifecycle(
            &self.lifecycle,
            self.semantic_digest,
            self.candidate_envelope_digest,
            self.commit_certificate_digest,
            self.transition_nullifier,
            self.request_digest,
            self.receiver_binding_digest,
            self.ciphertext_commitment,
            self.amount,
            self.terminal_output_binding,
            eq_deferred_audit,
            ep_deferred_audit,
            eq_protocol_digest,
            ep_protocol_digest,
        )
    }
}

/// Private opening of the terminal hardware commit evidence.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaCommitEvidenceOpeningGenerationV1 {
    /// Fresh hiding opening owned by committing hardware.
    pub opening: [u8; 32],
    /// Trusted commit time, or zero for monotonic-lease evidence.
    pub trusted_commit_time_ms: u64,
    /// Private monotonic lease identity, or zero for trusted-time evidence.
    pub lease_id: [u8; 32],
    /// Inclusive lease boundary, or zero for trusted-time evidence.
    pub lease_valid_from_ms: u64,
    /// Exclusive lease boundary, or zero for trusted-time evidence.
    pub lease_expires_at_ms: u64,
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaCommitEvidenceOpeningGenerationV1 {
    fn into_internal(self) -> KagemushaCommitEvidenceOpeningV1 {
        KagemushaCommitEvidenceOpeningV1 {
            opening: self.opening,
            trusted_commit_time_ms: self.trusted_commit_time_ms,
            lease_id: self.lease_id,
            lease_valid_from_ms: self.lease_valid_from_ms,
            lease_expires_at_ms: self.lease_expires_at_ms,
        }
    }
}

/// Exact request-bound SendSplit openings consumed by terminal authorization.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaTerminalSendGenerationWitnessV1 {
    /// Exact signed receiver request.
    pub request: KagemushaPaymentRequestV1,
    /// Exact final payment output, without proof bytes.
    pub output: KagemushaPaymentOutputV1,
    /// Digest of the actual encrypted credit carried by the final payment.
    pub encrypted_credit_digest: [u8; 32],
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaTerminalSendGenerationWitnessV1 {
    fn into_internal(self) -> KagemushaTerminalSendPrivateV1 {
        KagemushaTerminalSendPrivateV1 {
            request: self.request,
            output: self.output,
            encrypted_credit_digest: self.encrypted_credit_digest,
        }
    }
}

/// Private exact-next aggregate and hardware transition constrained by terminal authorization.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaTerminalAuthorizationPrivateGenerationWitnessV1 {
    /// Exact public lifecycle repeated inside the private relation.
    pub lifecycle: KagemushaLifecycleBindingV1,
    /// Private aggregate predecessor.
    pub predecessor: KagemushaStateV1,
    /// Private exact-next aggregate successor.
    pub successor: KagemushaStateV1,
    /// Exact one-use durable outbox reservation.
    pub outbox_reservation: KagemushaOutboxReservationV1,
    /// Terminal hardware commit certificate.
    pub commit_certificate: KagemushaCommitCertificateV1,
    /// Private opening of trusted-time or monotonic-lease evidence.
    pub commit_evidence_opening: KagemushaCommitEvidenceOpeningGenerationV1,
    /// Hardware-only one-use authorization for the exact private predecessor.
    pub one_use_hardware_authorization: [u8; 32],
    /// Proof-independent payment body or redemption payload bound before hardware commit.
    pub terminal_payload_digest: [u8; 32],
    /// Exact request and output openings; present only for SendSplit.
    pub send: Option<KagemushaTerminalSendGenerationWitnessV1>,
    /// Consumed rollback-resistant journal revision.
    pub journal_revision_before: u128,
    /// Exact-next rollback-resistant journal revision.
    pub journal_revision_after: u128,
    /// Consumed hardware authorization counter.
    pub authorization_counter_before: u128,
    /// Exact-next hardware authorization counter.
    pub authorization_counter_after: u128,
    /// Authenticated enabled hardware profile.
    pub hardware_profile: KagemushaHardwareProfileV1,
    /// Credential validated against `hardware_profile` inside terminal authorization.
    pub hardware_credential: KagemushaHardwareCredentialV1,
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaTerminalAuthorizationPrivateGenerationWitnessV1 {
    fn into_internal(self) -> KagemushaTerminalAuthorizationPrivateTransitionV1 {
        KagemushaTerminalAuthorizationPrivateTransitionV1 {
            lifecycle: self.lifecycle,
            predecessor: self.predecessor,
            successor: self.successor,
            outbox_reservation: self.outbox_reservation,
            commit_certificate: self.commit_certificate,
            commit_evidence_opening: self.commit_evidence_opening.into_internal(),
            one_use_hardware_authorization: self.one_use_hardware_authorization,
            terminal_payload_digest: self.terminal_payload_digest,
            send: self
                .send
                .map(KagemushaTerminalSendGenerationWitnessV1::into_internal),
            journal_revision_before: self.journal_revision_before,
            journal_revision_after: self.journal_revision_after,
            authorization_counter_before: self.authorization_counter_before,
            authorization_counter_after: self.authorization_counter_after,
            hardware_profile: self.hardware_profile,
            hardware_credential: self.hardware_credential,
        }
    }
}

/// Eq/Fp nested candidate and terminal-Guard inputs for terminal authorization.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Copy)]
pub struct KagemushaTerminalAuthorizationEqGenerationWitnessV1<'a> {
    /// Authenticated candidate-state protocol.
    pub candidate_protocol: &'a PlonkProtocol<EqAffine>,
    /// Candidate-state public instances.
    pub candidate_instances: &'a [Vec<Fp>],
    /// Candidate-state proof.
    pub candidate_proof: &'a [u8],
    /// History carried by the candidate-state proof.
    pub candidate_history: &'a KagemushaEqAccumulatorV1,
    /// Fold completing the candidate history.
    pub candidate_history_fold_proof: &'a KagemushaEqFoldProofV1,
    /// Authenticated terminal-Guard protocol.
    pub terminal_guard_protocol: &'a PlonkProtocol<EqAffine>,
    /// Terminal-Guard public instances.
    pub terminal_guard_instances: &'a [Vec<Fp>],
    /// Terminal-Guard proof.
    pub terminal_guard_proof: &'a [u8],
    /// History carried by the terminal-Guard proof.
    pub terminal_guard_history: &'a KagemushaEqAccumulatorV1,
    /// Fold completing the terminal-Guard history.
    pub terminal_guard_history_fold_proof: &'a KagemushaEqFoldProofV1,
    /// Fold merging complete candidate and terminal histories.
    pub merge_fold_proof: &'a KagemushaEqFoldProofV1,
    /// Constant-size successor history exposed by terminal authorization.
    pub successor_history: &'a KagemushaEqAccumulatorV1,
}

/// Ep/Fq nested candidate and terminal-Guard inputs for terminal authorization.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Copy)]
pub struct KagemushaTerminalAuthorizationEpGenerationWitnessV1<'a> {
    /// Authenticated candidate-state protocol.
    pub candidate_protocol: &'a PlonkProtocol<EpAffine>,
    /// Candidate-state public instances.
    pub candidate_instances: &'a [Vec<Fq>],
    /// Candidate-state proof.
    pub candidate_proof: &'a [u8],
    /// History carried by the candidate-state proof.
    pub candidate_history: &'a KagemushaEpAccumulatorV1,
    /// Fold completing the candidate history.
    pub candidate_history_fold_proof: &'a KagemushaEpFoldProofV1,
    /// Authenticated terminal-Guard protocol.
    pub terminal_guard_protocol: &'a PlonkProtocol<EpAffine>,
    /// Terminal-Guard public instances.
    pub terminal_guard_instances: &'a [Vec<Fq>],
    /// Terminal-Guard proof.
    pub terminal_guard_proof: &'a [u8],
    /// History carried by the terminal-Guard proof.
    pub terminal_guard_history: &'a KagemushaEpAccumulatorV1,
    /// Fold completing the terminal-Guard history.
    pub terminal_guard_history_fold_proof: &'a KagemushaEpFoldProofV1,
    /// Fold merging complete candidate and terminal histories.
    pub merge_fold_proof: &'a KagemushaEpFoldProofV1,
    /// Constant-size successor history exposed by terminal authorization.
    pub successor_history: &'a KagemushaEpAccumulatorV1,
}

/// Complete generation input for both mutually audited terminal-authorization parities.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone)]
pub struct KagemushaTerminalAuthorizationGenerationWitnessV1<'a> {
    /// Unlinkable terminal projection.
    pub public: KagemushaTerminalAuthorizationTerminalGenerationPublicV1,
    /// Private predecessor and terminal hardware/state transition.
    pub private_transition: KagemushaTerminalAuthorizationPrivateGenerationWitnessV1,
    /// Complete private Guard relation recursively authenticated by terminal authorization.
    pub terminal_guard_relation: KagemushaGuardBundleRelationWitnessV1,
    /// Sorted nonzero-prefix release-enabled profile IDs with canonical zero padding.
    pub enabled_hardware_profiles:
        [[u8; 32]; KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
    /// Eq/Fp nested proof inputs.
    pub eq: KagemushaTerminalAuthorizationEqGenerationWitnessV1<'a>,
    /// Ep/Fq nested proof inputs.
    pub ep: KagemushaTerminalAuthorizationEpGenerationWitnessV1<'a>,
}

/// Generated key material and protocol identities for internal post-commit terminal authorization.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug)]
pub struct KagemushaGeneratedTerminalAuthorizationArtifactsV1 {
    /// Canonical Eq transparent IPA parameters used during key generation.
    pub eq_parameters: Arc<[u8]>,
    /// Canonical Ep transparent IPA parameters used during key generation.
    pub ep_parameters: Arc<[u8]>,
    /// Processed Eq terminal-authorization proving key.
    pub eq_proving_key: Arc<[u8]>,
    /// Processed Eq terminal-authorization verifying key.
    pub eq_verifying_key: Arc<[u8]>,
    /// Processed Ep terminal-authorization proving key.
    pub ep_proving_key: Arc<[u8]>,
    /// Processed Ep terminal-authorization verifying key.
    pub ep_verifying_key: Arc<[u8]>,
    /// Exact Eq circuit layout.
    pub eq_circuit_params: BaseCircuitParams,
    /// Exact Ep circuit layout.
    pub ep_circuit_params: BaseCircuitParams,
    /// Compiled Eq terminal-authorization protocol digest.
    pub eq_protocol_digest: [u8; 32],
    /// Compiled Ep terminal-authorization protocol digest.
    pub ep_protocol_digest: [u8; 32],
    /// Exact sorted release-enabled profile constants committed by both verifying keys.
    pub enabled_hardware_profiles:
        [[u8; 32]; KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
}

/// Loaded authenticated Eq terminal-authorization parameters and keys.
#[cfg(feature = "zk-halo2-ipa")]
pub struct KagemushaLoadedEqTerminalAuthorizationArtifactsV1 {
    /// Canonical Eq transparent IPA parameters.
    pub parameters: ParamsIPA<EqAffine>,
    /// Exact processed Eq terminal-authorization proving key.
    pub proving_key: ProvingKey<EqAffine>,
    /// Exact processed Eq terminal-authorization verifying key.
    pub verifying_key: VerifyingKey<EqAffine>,
    /// Authenticated circuit layout.
    pub circuit_params: BaseCircuitParams,
    /// Compiled protocol identity for this verifying key.
    pub protocol_digest: [u8; 32],
    /// Authenticated release owning every loaded role.
    pub release_id: [u8; 32],
    /// Authenticated circuit-profile digest.
    pub profile_digest: [u8; 32],
    /// Digest of the complete authenticated artifact inventory.
    pub artifact_manifest_digest: [u8; 32],
    /// Release-wide proof suite.
    pub suite_id: [u8; 32],
    /// Digest of the release-pinned verifier set.
    pub vk_digest: [u8; 32],
    /// Exact enabled-profile constants committed by this verifying key.
    pub enabled_hardware_profiles:
        [[u8; 32]; KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
}

/// Loaded authenticated Ep terminal-authorization parameters and keys.
#[cfg(feature = "zk-halo2-ipa")]
pub struct KagemushaLoadedEpTerminalAuthorizationArtifactsV1 {
    /// Canonical Ep transparent IPA parameters.
    pub parameters: ParamsIPA<EpAffine>,
    /// Exact processed Ep terminal-authorization proving key.
    pub proving_key: ProvingKey<EpAffine>,
    /// Exact processed Ep terminal-authorization verifying key.
    pub verifying_key: VerifyingKey<EpAffine>,
    /// Authenticated circuit layout.
    pub circuit_params: BaseCircuitParams,
    /// Compiled protocol identity for this verifying key.
    pub protocol_digest: [u8; 32],
    /// Authenticated release owning every loaded role.
    pub release_id: [u8; 32],
    /// Authenticated circuit-profile digest.
    pub profile_digest: [u8; 32],
    /// Digest of the complete authenticated artifact inventory.
    pub artifact_manifest_digest: [u8; 32],
    /// Release-wide proof suite.
    pub suite_id: [u8; 32],
    /// Digest of the release-pinned verifier set.
    pub vk_digest: [u8; 32],
    /// Exact enabled-profile constants committed by this verifying key.
    pub enabled_hardware_profiles:
        [[u8; 32]; KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
}

/// Generated constant-size redemption proof and its recursive carry material.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaGeneratedRedemptionProofV1 {
    /// Exact Eq public column (81 field elements).
    pub eq_public_instances: Vec<Fp>,
    /// Exact Ep public column (81 field elements).
    pub ep_public_instances: Vec<Fq>,
    /// Final redemption proof.
    pub proof: KagemushaRedemptionProofV1,
    /// Eq current opening claim extracted for the next history fold.
    pub eq_current_accumulator: KagemushaEqAccumulatorV1,
    /// Ep current opening claim extracted for the next history fold.
    pub ep_current_accumulator: KagemushaEpAccumulatorV1,
}

/// Internal one-pair terminal authorization consumed by the transported proof.
///
/// This carrier deliberately has no Norito wire representation. Only the
/// compact CommitWrapper payment or redemption proof crosses transport.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaGeneratedTerminalAuthorizationProofV1 {
    /// Exact Eq public column (81 field elements).
    pub eq_public_instances: Vec<Fp>,
    /// Exact Ep public column (81 field elements).
    pub ep_public_instances: Vec<Fq>,
    /// Internal Eq terminal-authorization proof.
    pub eq_proof: Vec<u8>,
    /// Internal Ep terminal-authorization proof.
    pub ep_proof: Vec<u8>,
    /// Eq history carried by the internal proof.
    pub eq_history: KagemushaEqAccumulatorV1,
    /// Ep history carried by the internal proof.
    pub ep_history: KagemushaEpAccumulatorV1,
    /// Eq current opening claim extracted for the outer recursive fold.
    pub eq_current_accumulator: KagemushaEqAccumulatorV1,
    /// Ep current opening claim extracted for the outer recursive fold.
    pub ep_current_accumulator: KagemushaEpAccumulatorV1,
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaGeneratedTerminalAuthorizationArtifactsV1 {
    /// Return the four distinct terminal-authorization key bindings authenticated by a release.
    #[must_use]
    pub fn bindings(&self) -> [KagemushaArtifactBindingV1; 4] {
        [
            binding(
                KagemushaArtifactRoleV1::TerminalAuthorizationPkEq,
                &self.eq_proving_key,
            ),
            binding(
                KagemushaArtifactRoleV1::TerminalAuthorizationVkEq,
                &self.eq_verifying_key,
            ),
            binding(
                KagemushaArtifactRoleV1::TerminalAuthorizationPkEp,
                &self.ep_proving_key,
            ),
            binding(
                KagemushaArtifactRoleV1::TerminalAuthorizationVkEp,
                &self.ep_verifying_key,
            ),
        ]
    }

    /// Install parameters and all four content-addressed terminal-authorization keys.
    pub fn install_into(&self, resolver: &mut KagemushaMemoryArtifactResolverV1) {
        for bytes in [
            &self.eq_parameters,
            &self.ep_parameters,
            &self.eq_proving_key,
            &self.eq_verifying_key,
            &self.ep_proving_key,
            &self.ep_verifying_key,
        ] {
            resolver.insert(Arc::clone(bytes));
        }
    }
}

/// Eq/Fp internal `TerminalAuthorization` inputs for the transported proof.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Copy)]
pub struct KagemushaCommitWrapperEqGenerationWitnessV1<'a> {
    /// Release-pinned internal terminal-authorization protocol.
    pub terminal_authorization_protocol: &'a PlonkProtocol<EqAffine>,
    /// Exact internal public instances.
    pub terminal_authorization_instances: &'a [Vec<Fp>],
    /// Exact internal terminal-authorization proof.
    pub terminal_authorization_proof: &'a [u8],
    /// History carried by the internal proof.
    pub terminal_authorization_history: &'a KagemushaEqAccumulatorV1,
    /// Fold of the internal current claim and its carried history.
    pub terminal_authorization_history_fold_proof: &'a KagemushaEqFoldProofV1,
    /// Constant-size history exposed by the transported proof.
    pub successor_history: &'a KagemushaEqAccumulatorV1,
}

/// Ep/Fq internal `TerminalAuthorization` inputs for the transported proof.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Copy)]
pub struct KagemushaCommitWrapperEpGenerationWitnessV1<'a> {
    /// Release-pinned internal terminal-authorization protocol.
    pub terminal_authorization_protocol: &'a PlonkProtocol<EpAffine>,
    /// Exact internal public instances.
    pub terminal_authorization_instances: &'a [Vec<Fq>],
    /// Exact internal terminal-authorization proof.
    pub terminal_authorization_proof: &'a [u8],
    /// History carried by the internal proof.
    pub terminal_authorization_history: &'a KagemushaEpAccumulatorV1,
    /// Fold of the internal current claim and its carried history.
    pub terminal_authorization_history_fold_proof: &'a KagemushaEpFoldProofV1,
    /// Constant-size history exposed by the transported proof.
    pub successor_history: &'a KagemushaEpAccumulatorV1,
}

/// Complete generation input for the sole transported authorization pair.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone)]
pub struct KagemushaCommitWrapperGenerationWitnessV1<'a> {
    /// Exact post-commit projection already authenticated by the internal paired proof.
    pub public: KagemushaTerminalAuthorizationTerminalGenerationPublicV1,
    /// Exact profile table committed by the nested terminal-authorization keys.
    pub enabled_hardware_profiles:
        [[u8; 32]; KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
    /// Eq/Fp internal terminal-authorization input.
    pub eq: KagemushaCommitWrapperEqGenerationWitnessV1<'a>,
    /// Ep/Fq internal terminal-authorization input.
    pub ep: KagemushaCommitWrapperEpGenerationWitnessV1<'a>,
}

/// Generated key material for the distinct CommitWrapper relation.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug)]
pub struct KagemushaGeneratedCommitWrapperArtifactsV1 {
    /// Canonical Eq transparent IPA parameters.
    pub eq_parameters: Arc<[u8]>,
    /// Canonical Ep transparent IPA parameters.
    pub ep_parameters: Arc<[u8]>,
    /// Processed Eq authorization proving key.
    pub eq_proving_key: Arc<[u8]>,
    /// Processed Eq authorization verifying key.
    pub eq_verifying_key: Arc<[u8]>,
    /// Processed Ep authorization proving key.
    pub ep_proving_key: Arc<[u8]>,
    /// Processed Ep authorization verifying key.
    pub ep_verifying_key: Arc<[u8]>,
    /// Exact Eq circuit layout.
    pub eq_circuit_params: BaseCircuitParams,
    /// Exact Ep circuit layout.
    pub ep_circuit_params: BaseCircuitParams,
    /// Compiled Eq authorization protocol digest.
    pub eq_protocol_digest: [u8; 32],
    /// Compiled Ep authorization protocol digest.
    pub ep_protocol_digest: [u8; 32],
    /// Eq `TerminalAuthorization` protocol recursively fixed by this authorization key.
    pub terminal_authorization_eq_protocol_digest: [u8; 32],
    /// Ep `TerminalAuthorization` protocol recursively fixed by this authorization key.
    pub terminal_authorization_ep_protocol_digest: [u8; 32],
    /// Exact sorted release-enabled profile constants committed by both keys.
    pub enabled_hardware_profiles:
        [[u8; 32]; KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
}

/// Loaded authenticated Eq CommitWrapper parameters and keys.
#[cfg(feature = "zk-halo2-ipa")]
pub struct KagemushaLoadedEqCommitWrapperArtifactsV1 {
    /// Canonical Eq transparent IPA parameters.
    pub parameters: ParamsIPA<EqAffine>,
    /// Dedicated processed Eq proving key.
    pub proving_key: ProvingKey<EqAffine>,
    /// Dedicated processed Eq verifying key.
    pub verifying_key: VerifyingKey<EqAffine>,
    /// Authenticated circuit layout.
    pub circuit_params: BaseCircuitParams,
    /// Compiled protocol identity for this verifying key.
    pub protocol_digest: [u8; 32],
    /// Release-pinned Eq `TerminalAuthorization` protocol consumed recursively.
    pub terminal_authorization_protocol_digest: [u8; 32],
    /// Authenticated release owning every loaded role.
    pub release_id: [u8; 32],
    /// Authenticated circuit-profile digest.
    pub profile_digest: [u8; 32],
    /// Digest of the complete authenticated artifact inventory.
    pub artifact_manifest_digest: [u8; 32],
    /// Release-wide proof suite.
    pub suite_id: [u8; 32],
    /// Digest of the release-pinned verifier set.
    pub vk_digest: [u8; 32],
    /// Exact enabled-profile constants committed by this key.
    pub enabled_hardware_profiles:
        [[u8; 32]; KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
}

/// Loaded authenticated Ep CommitWrapper parameters and keys.
#[cfg(feature = "zk-halo2-ipa")]
pub struct KagemushaLoadedEpCommitWrapperArtifactsV1 {
    /// Canonical Ep transparent IPA parameters.
    pub parameters: ParamsIPA<EpAffine>,
    /// Dedicated processed Ep proving key.
    pub proving_key: ProvingKey<EpAffine>,
    /// Dedicated processed Ep verifying key.
    pub verifying_key: VerifyingKey<EpAffine>,
    /// Authenticated circuit layout.
    pub circuit_params: BaseCircuitParams,
    /// Compiled protocol identity for this verifying key.
    pub protocol_digest: [u8; 32],
    /// Release-pinned Ep `TerminalAuthorization` protocol consumed recursively.
    pub terminal_authorization_protocol_digest: [u8; 32],
    /// Authenticated release owning every loaded role.
    pub release_id: [u8; 32],
    /// Authenticated circuit-profile digest.
    pub profile_digest: [u8; 32],
    /// Digest of the complete authenticated artifact inventory.
    pub artifact_manifest_digest: [u8; 32],
    /// Release-wide proof suite.
    pub suite_id: [u8; 32],
    /// Digest of the release-pinned verifier set.
    pub vk_digest: [u8; 32],
    /// Exact enabled-profile constants committed by this key.
    pub enabled_hardware_profiles:
        [[u8; 32]; KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
}

/// Generated compact post-commit material before selecting its operation-specific wire family.
///
/// This carrier deliberately has no Norito representation. Its private proof material can only
/// become a payment or redemption through the operation-checked conversion methods.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaGeneratedCommitWrapperProofV1 {
    /// Exact Eq public column (81 field elements).
    pub eq_public_instances: Vec<Fp>,
    /// Exact Ep public column (81 field elements).
    pub ep_public_instances: Vec<Fq>,
    /// Eq current opening claim extracted for the next history fold.
    pub eq_current_accumulator: KagemushaEqAccumulatorV1,
    /// Ep current opening claim extracted for the next history fold.
    pub ep_current_accumulator: KagemushaEpAccumulatorV1,
    public: KagemushaTerminalAuthorizationPublicInputsV1,
    eq_proof: Vec<u8>,
    ep_proof: Vec<u8>,
    eq_history: KagemushaEqAccumulatorV1,
    ep_history: KagemushaEpAccumulatorV1,
}

/// Generated constant-size payment proof and its recursive carry material.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaGeneratedPaymentProofV1 {
    /// Exact Eq public column (81 field elements).
    pub eq_public_instances: Vec<Fp>,
    /// Exact Ep public column (81 field elements).
    pub ep_public_instances: Vec<Fq>,
    /// Final post-commit payment proof.
    pub proof: KagemushaPaymentProofV1,
    /// Eq current opening claim extracted for the next history fold.
    pub eq_current_accumulator: KagemushaEqAccumulatorV1,
    /// Ep current opening claim extracted for the next history fold.
    pub ep_current_accumulator: KagemushaEpAccumulatorV1,
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaGeneratedCommitWrapperProofV1 {
    fn validate_material(&self) -> Result<(), KagemushaArtifactGenerationErrorV1> {
        self.public
            .validate()
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
        let eq_instances = terminal_authorization_public_instances::<Fp>(
            &self.public,
            self.eq_history.as_bytes(),
        )?;
        let ep_instances = terminal_authorization_public_instances::<Fq>(
            &self.public,
            self.ep_history.as_bytes(),
        )?;
        if self.eq_public_instances != eq_instances || self.ep_public_instances != ep_instances {
            return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
                "commit-wrapper material does not match its exact terminal public projection"
                    .to_owned(),
            ));
        }
        // Both terminal wire types have this identical bounded layout. This shape check does
        // not select or authorize an operation; conversion below checks the proof-bound family.
        let shape = KagemushaPaymentProofV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            eq_protocol_digest: self.public.eq_protocol_digest,
            ep_protocol_digest: self.public.ep_protocol_digest,
            semantic_digest: self.public.semantic_digest,
            candidate_envelope_digest: self.public.candidate_envelope_digest,
            commit_certificate_digest: self.public.commit_certificate_digest,
            eq_deferred_audit: self.public.eq_deferred_audit,
            ep_deferred_audit: self.public.ep_deferred_audit,
            eq_proof: self.eq_proof.clone(),
            ep_proof: self.ep_proof.clone(),
            eq_history: self.eq_history.as_bytes().to_vec(),
            ep_history: self.ep_history.as_bytes().to_vec(),
        };
        shape
            .validate_shape_against(
                self.public.semantic_digest,
                self.public.candidate_envelope_digest,
                self.public.commit_certificate_digest,
            )
            .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
        let encoded = norito::encode_canonical(&shape)
            .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
        if encoded.len() > KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1 {
            return Err(KagemushaArtifactGenerationErrorV1::InvalidLength {
                parity: KagemushaPastaParityV1::Eq,
                kind: "paired commit-wrapper proof",
                actual: u64::try_from(encoded.len()).unwrap_or(u64::MAX),
            });
        }
        Ok(())
    }

    /// Convert the compact proof into the payment wire family.
    ///
    /// # Errors
    ///
    /// Rejects the opposite operation or any changed public column, body, candidate, certificate,
    /// history, protocol, or bounded proof field.
    pub fn into_payment(
        self,
    ) -> Result<KagemushaGeneratedPaymentProofV1, KagemushaArtifactGenerationErrorV1> {
        require_terminal_operation_v1(self.public.operation, KagemushaOperationV1::SendSplit)?;
        self.validate_material()?;
        let proof = KagemushaPaymentProofV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            eq_protocol_digest: self.public.eq_protocol_digest,
            ep_protocol_digest: self.public.ep_protocol_digest,
            semantic_digest: self.public.semantic_digest,
            candidate_envelope_digest: self.public.candidate_envelope_digest,
            commit_certificate_digest: self.public.commit_certificate_digest,
            eq_deferred_audit: self.public.eq_deferred_audit,
            ep_deferred_audit: self.public.ep_deferred_audit,
            eq_proof: self.eq_proof.clone(),
            ep_proof: self.ep_proof.clone(),
            eq_history: self.eq_history.as_bytes().to_vec(),
            ep_history: self.ep_history.as_bytes().to_vec(),
        };
        proof
            .validate_shape_against(
                self.public.semantic_digest,
                self.public.candidate_envelope_digest,
                self.public.commit_certificate_digest,
            )
            .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
        Ok(KagemushaGeneratedPaymentProofV1 {
            eq_public_instances: self.eq_public_instances,
            ep_public_instances: self.ep_public_instances,
            proof,
            eq_current_accumulator: self.eq_current_accumulator,
            ep_current_accumulator: self.ep_current_accumulator,
        })
    }

    /// Convert the compact proof into the redemption wire family.
    ///
    /// # Errors
    ///
    /// Rejects the opposite operation or any changed public column, body, candidate, certificate,
    /// history, protocol, or bounded proof field.
    pub fn into_redemption(
        self,
    ) -> Result<KagemushaGeneratedRedemptionProofV1, KagemushaArtifactGenerationErrorV1> {
        require_terminal_operation_v1(self.public.operation, KagemushaOperationV1::RedeemSplit)?;
        self.validate_material()?;
        let proof = KagemushaRedemptionProofV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            eq_protocol_digest: self.public.eq_protocol_digest,
            ep_protocol_digest: self.public.ep_protocol_digest,
            semantic_digest: self.public.semantic_digest,
            candidate_envelope_digest: self.public.candidate_envelope_digest,
            commit_certificate_digest: self.public.commit_certificate_digest,
            eq_deferred_audit: self.public.eq_deferred_audit,
            ep_deferred_audit: self.public.ep_deferred_audit,
            eq_proof: self.eq_proof.clone(),
            ep_proof: self.ep_proof.clone(),
            eq_history: self.eq_history.as_bytes().to_vec(),
            ep_history: self.ep_history.as_bytes().to_vec(),
        };
        proof
            .validate_shape_against(
                self.public.semantic_digest,
                self.public.candidate_envelope_digest,
                self.public.commit_certificate_digest,
            )
            .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
        Ok(KagemushaGeneratedRedemptionProofV1 {
            eq_public_instances: self.eq_public_instances,
            ep_public_instances: self.ep_public_instances,
            proof,
            eq_current_accumulator: self.eq_current_accumulator,
            ep_current_accumulator: self.ep_current_accumulator,
        })
    }
}

#[cfg(feature = "zk-halo2-ipa")]
fn require_terminal_operation_v1(
    actual: KagemushaOperationV1,
    expected: KagemushaOperationV1,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    if actual != expected
        || !matches!(
            actual,
            KagemushaOperationV1::SendSplit | KagemushaOperationV1::RedeemSplit
        )
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "commit-wrapper operation does not match the requested terminal wire family".to_owned(),
        ));
    }
    Ok(())
}

/// Produce the final compact post-commit payment proof under the dedicated CommitWrapper keys.
///
/// Recovery must supply the same hardware-unsealed operation seed and immutable witness.
/// The seed is never derived from public payment/certificate data or the opaque sealed blob.
///
/// # Errors
///
/// Rejects the opposite terminal operation, mismatched inner proof projection, substituted release
/// keys, invalid internal proof material, or a proof exceeding the fixed transport allocation.
#[cfg(feature = "zk-halo2-ipa")]
pub fn prove_kagemusha_payment_v1(
    eq: &KagemushaLoadedEqCommitWrapperArtifactsV1,
    ep: &KagemushaLoadedEpCommitWrapperArtifactsV1,
    witness: KagemushaCommitWrapperGenerationWitnessV1<'_>,
    recovery_seed: &KagemushaRecoverySeedV1,
) -> Result<KagemushaGeneratedPaymentProofV1, KagemushaArtifactGenerationErrorV1> {
    require_terminal_operation_v1(
        witness.public.lifecycle.operation_kind.into(),
        KagemushaOperationV1::SendSplit,
    )?;
    prove_kagemusha_commit_wrapper_v1(eq, ep, witness, recovery_seed)?.into_payment()
}

/// Produce the final compact post-commit redemption proof under the dedicated CommitWrapper keys.
///
/// Recovery must supply the same hardware-unsealed operation seed and immutable witness.
///
/// # Errors
///
/// Rejects the opposite terminal operation, mismatched inner proof projection, substituted release
/// keys, invalid internal proof material, or a proof exceeding the fixed transport allocation.
#[cfg(feature = "zk-halo2-ipa")]
pub fn prove_kagemusha_redemption_v1(
    eq: &KagemushaLoadedEqCommitWrapperArtifactsV1,
    ep: &KagemushaLoadedEpCommitWrapperArtifactsV1,
    witness: KagemushaCommitWrapperGenerationWitnessV1<'_>,
    recovery_seed: &KagemushaRecoverySeedV1,
) -> Result<KagemushaGeneratedRedemptionProofV1, KagemushaArtifactGenerationErrorV1> {
    require_terminal_operation_v1(
        witness.public.lifecycle.operation_kind.into(),
        KagemushaOperationV1::RedeemSplit,
    )?;
    prove_kagemusha_commit_wrapper_v1(eq, ep, witness, recovery_seed)?.into_redemption()
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaGeneratedCommitWrapperArtifactsV1 {
    /// Return the four non-interchangeable authorization key bindings.
    #[must_use]
    pub fn bindings(&self) -> [KagemushaArtifactBindingV1; 4] {
        [
            binding(
                KagemushaArtifactRoleV1::CommitWrapperPkEq,
                &self.eq_proving_key,
            ),
            binding(
                KagemushaArtifactRoleV1::CommitWrapperVkEq,
                &self.eq_verifying_key,
            ),
            binding(
                KagemushaArtifactRoleV1::CommitWrapperPkEp,
                &self.ep_proving_key,
            ),
            binding(
                KagemushaArtifactRoleV1::CommitWrapperVkEp,
                &self.ep_verifying_key,
            ),
        ]
    }

    /// Install parameters and all four content-addressed authorization keys.
    pub fn install_into(&self, resolver: &mut KagemushaMemoryArtifactResolverV1) {
        for bytes in [
            &self.eq_parameters,
            &self.ep_parameters,
            &self.eq_proving_key,
            &self.eq_verifying_key,
            &self.ep_proving_key,
            &self.ep_verifying_key,
        ] {
            resolver.insert(Arc::clone(bytes));
        }
    }
}

/// Lower an authenticated release's enabled profiles into the fixed terminal-authorization key table.
///
/// # Errors
///
/// Rejects an empty, unsorted, duplicate, over-wide, or cross-suite enabled-profile set.
#[cfg(feature = "zk-halo2-ipa")]
pub fn kagemusha_terminal_authorization_enabled_profile_table_v1(
    release: &KagemushaAuthenticatedReleaseV1,
) -> Result<
    [[u8; 32]; KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
    KagemushaArtifactGenerationErrorV1,
> {
    let profiles = release.enabled_profiles();
    if profiles.is_empty()
        || profiles.len() > KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "authenticated release has an invalid enabled-profile count".to_owned(),
        ));
    }
    let suite_id = profiles[0].suite_id;
    let vk_digest = profiles[0].vk_digest;
    let mut previous = None;
    let mut table = [[0; 32]; KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1];
    for (slot, profile) in table.iter_mut().zip(profiles) {
        if profile.hardware_profile_id == [0; 32]
            || previous.is_some_and(|value| value >= profile.hardware_profile_id)
            || profile.suite_id != suite_id
            || profile.vk_digest != vk_digest
        {
            return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
                "authenticated release enabled-profile table is noncanonical".to_owned(),
            ));
        }
        *slot = profile.hardware_profile_id;
        previous = Some(profile.hardware_profile_id);
    }
    if vk_digest != release.vk_set_digest() {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "authenticated release verifier-set digest is inconsistent".to_owned(),
        ));
    }
    Ok(table)
}

/// Lower an authenticated release's enabled profiles into the fixed authorization-key table.
///
/// # Errors
///
/// Returns an error if the authenticated release does not expose a nonempty, strictly sorted,
/// distinct profile prefix or if its profiles do not share one proof suite and verifier set.
#[cfg(feature = "zk-halo2-ipa")]
pub fn kagemusha_enabled_hardware_profile_table_v1(
    release: &KagemushaAuthenticatedReleaseV1,
) -> Result<
    [[u8; 32]; KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1],
    KagemushaArtifactGenerationErrorV1,
> {
    let profiles = release.enabled_profiles();
    if profiles.is_empty() || profiles.len() > KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1 {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "authenticated release has an invalid enabled-profile count".to_owned(),
        ));
    }
    let suite_id = profiles[0].suite_id;
    let vk_digest = profiles[0].vk_digest;
    let mut previous = None;
    let mut table = [[0; 32]; KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1];
    for (slot, profile) in table.iter_mut().zip(profiles) {
        if profile.hardware_profile_id == [0; 32]
            || previous.is_some_and(|value| value >= profile.hardware_profile_id)
            || profile.suite_id != suite_id
            || profile.vk_digest != vk_digest
        {
            return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
                "authenticated release enabled-profile table is noncanonical".to_owned(),
            ));
        }
        *slot = profile.hardware_profile_id;
        previous = Some(profile.hardware_profile_id);
    }
    if vk_digest != release.vk_set_digest() {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "authenticated release verifier-set digest is inconsistent".to_owned(),
        ));
    }
    Ok(table)
}

/// Complete fixed-shape witness for one release-pinned mint authorization.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone)]
pub struct KagemushaMintAuthorizationGenerationWitnessV1<'a> {
    /// Exact public/private mint-authorization relation.
    pub relation: KagemushaMintAuthorizationRelationWitnessV1,
    /// Sorted enabled-profile prefix with canonical zero padding.
    pub enabled_hardware_profiles: [[u8; 32]; KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1],
    /// Eq compiled platform-credential protocol.
    pub eq_credential_protocol: &'a PlonkProtocol<EqAffine>,
    /// Eq platform-credential proof.
    pub eq_credential_proof: &'a [u8],
    /// Eq credential history carried by the authorization proof.
    pub eq_credential_history: &'a KagemushaEqAccumulatorV1,
    /// Ep compiled platform-credential protocol.
    pub ep_credential_protocol: &'a PlonkProtocol<EpAffine>,
    /// Ep platform-credential proof.
    pub ep_credential_proof: &'a [u8],
    /// Ep credential history carried by the authorization proof.
    pub ep_credential_history: &'a KagemushaEpAccumulatorV1,
}

#[cfg(feature = "zk-halo2-ipa")]
impl<'a> KagemushaMintAuthorizationGenerationWitnessV1<'a> {
    fn into_internal(
        self,
        eq_deferred_audit: [u8; 32],
        ep_deferred_audit: [u8; 32],
    ) -> KagemushaMintAuthorizationRecursiveWitnessV1<'a> {
        KagemushaMintAuthorizationRecursiveWitnessV1 {
            relation: self.relation,
            enabled_hardware_profiles: self.enabled_hardware_profiles,
            eq_credential_protocol: self.eq_credential_protocol,
            eq_credential_proof: self.eq_credential_proof,
            eq_credential_history: self.eq_credential_history,
            ep_credential_protocol: self.ep_credential_protocol,
            ep_credential_proof: self.ep_credential_proof,
            ep_credential_history: self.ep_credential_history,
            eq_deferred_audit,
            ep_deferred_audit,
        }
    }
}

/// Generated key material for the dedicated mint-authorization circuit pair.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug)]
pub struct KagemushaGeneratedMintAuthorizationArtifactsV1 {
    /// Canonical Eq transparent IPA parameters.
    pub eq_parameters: Arc<[u8]>,
    /// Canonical Ep transparent IPA parameters.
    pub ep_parameters: Arc<[u8]>,
    /// Processed Eq mint-authorization proving key.
    pub eq_proving_key: Arc<[u8]>,
    /// Processed Eq mint-authorization verifying key.
    pub eq_verifying_key: Arc<[u8]>,
    /// Processed Ep mint-authorization proving key.
    pub ep_proving_key: Arc<[u8]>,
    /// Processed Ep mint-authorization verifying key.
    pub ep_verifying_key: Arc<[u8]>,
    /// Exact Eq circuit layout.
    pub eq_circuit_params: BaseCircuitParams,
    /// Exact Ep circuit layout.
    pub ep_circuit_params: BaseCircuitParams,
    /// Compiled Eq mint-authorization protocol digest.
    pub eq_protocol_digest: [u8; 32],
    /// Compiled Ep mint-authorization protocol digest.
    pub ep_protocol_digest: [u8; 32],
    /// Processed Eq private SHA/credential carrier proving key.
    pub inner_eq_proving_key: Arc<[u8]>,
    /// Processed Eq private SHA/credential carrier verifying key.
    pub inner_eq_verifying_key: Arc<[u8]>,
    /// Processed Ep private SHA/credential carrier proving key.
    pub inner_ep_proving_key: Arc<[u8]>,
    /// Processed Ep private SHA/credential carrier verifying key.
    pub inner_ep_verifying_key: Arc<[u8]>,
    /// Exact Eq private-carrier circuit layout.
    pub inner_eq_circuit_params: BaseCircuitParams,
    /// Exact Ep private-carrier circuit layout.
    pub inner_ep_circuit_params: BaseCircuitParams,
    /// Enabled-profile constants committed by both keys.
    pub enabled_hardware_profiles: [[u8; 32]; KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1],
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaGeneratedMintAuthorizationArtifactsV1 {
    /// Return the eight distinct inner/outer key bindings authenticated by a release.
    #[must_use]
    pub fn bindings(&self) -> [KagemushaArtifactBindingV1; 8] {
        [
            binding(
                KagemushaArtifactRoleV1::MintAuthorizationPkEq,
                &self.eq_proving_key,
            ),
            binding(
                KagemushaArtifactRoleV1::MintAuthorizationVkEq,
                &self.eq_verifying_key,
            ),
            binding(
                KagemushaArtifactRoleV1::MintAuthorizationPkEp,
                &self.ep_proving_key,
            ),
            binding(
                KagemushaArtifactRoleV1::MintAuthorizationVkEp,
                &self.ep_verifying_key,
            ),
            binding(
                KagemushaArtifactRoleV1::InnerMintAuthorizationPkEq,
                &self.inner_eq_proving_key,
            ),
            binding(
                KagemushaArtifactRoleV1::InnerMintAuthorizationVkEq,
                &self.inner_eq_verifying_key,
            ),
            binding(
                KagemushaArtifactRoleV1::InnerMintAuthorizationPkEp,
                &self.inner_ep_proving_key,
            ),
            binding(
                KagemushaArtifactRoleV1::InnerMintAuthorizationVkEp,
                &self.inner_ep_verifying_key,
            ),
        ]
    }

    /// Install the shared parameters and eight content-addressed keys.
    pub fn install_into(&self, resolver: &mut KagemushaMemoryArtifactResolverV1) {
        for bytes in [
            &self.eq_parameters,
            &self.ep_parameters,
            &self.eq_proving_key,
            &self.eq_verifying_key,
            &self.ep_proving_key,
            &self.ep_verifying_key,
            &self.inner_eq_proving_key,
            &self.inner_eq_verifying_key,
            &self.inner_ep_proving_key,
            &self.inner_ep_verifying_key,
        ] {
            resolver.insert(Arc::clone(bytes));
        }
    }
}

/// Loaded authenticated Eq mint-authorization artifacts.
#[cfg(feature = "zk-halo2-ipa")]
pub struct KagemushaLoadedEqMintAuthorizationArtifactsV1 {
    /// Canonical Eq transparent IPA parameters.
    pub parameters: ParamsIPA<EqAffine>,
    /// Processed Eq proving key.
    pub proving_key: ProvingKey<EqAffine>,
    /// Processed Eq verifying key.
    pub verifying_key: VerifyingKey<EqAffine>,
    /// Authenticated circuit layout.
    pub circuit_params: BaseCircuitParams,
    /// Compiled Eq protocol identity.
    pub protocol_digest: [u8; 32],
    /// Authenticated private SHA/credential carrier proving key.
    pub inner_proving_key: ProvingKey<EqAffine>,
    /// Authenticated private SHA/credential carrier verifying key.
    pub inner_verifying_key: VerifyingKey<EqAffine>,
    /// Authenticated private-carrier layout, distinct from the outer layout.
    pub inner_circuit_params: BaseCircuitParams,
    /// Authenticated release identity.
    pub release_id: [u8; 32],
    /// Authenticated circuit-profile identity.
    pub profile_digest: [u8; 32],
    /// Authenticated artifact-manifest identity.
    pub artifact_manifest_digest: [u8; 32],
    /// Release-wide proof suite.
    pub suite_id: [u8; 32],
    /// Complete release-pinned verifier-set digest.
    pub vk_digest: [u8; 32],
    /// Enabled-profile constants committed by this key.
    pub enabled_hardware_profiles: [[u8; 32]; KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1],
}

/// Loaded authenticated Ep mint-authorization artifacts.
#[cfg(feature = "zk-halo2-ipa")]
pub struct KagemushaLoadedEpMintAuthorizationArtifactsV1 {
    /// Canonical Ep transparent IPA parameters.
    pub parameters: ParamsIPA<EpAffine>,
    /// Processed Ep proving key.
    pub proving_key: ProvingKey<EpAffine>,
    /// Processed Ep verifying key.
    pub verifying_key: VerifyingKey<EpAffine>,
    /// Authenticated circuit layout.
    pub circuit_params: BaseCircuitParams,
    /// Compiled Ep protocol identity.
    pub protocol_digest: [u8; 32],
    /// Authenticated private SHA/credential carrier proving key.
    pub inner_proving_key: ProvingKey<EpAffine>,
    /// Authenticated private SHA/credential carrier verifying key.
    pub inner_verifying_key: VerifyingKey<EpAffine>,
    /// Authenticated private-carrier layout, distinct from the outer layout.
    pub inner_circuit_params: BaseCircuitParams,
    /// Authenticated release identity.
    pub release_id: [u8; 32],
    /// Authenticated circuit-profile identity.
    pub profile_digest: [u8; 32],
    /// Authenticated artifact-manifest identity.
    pub artifact_manifest_digest: [u8; 32],
    /// Release-wide proof suite.
    pub suite_id: [u8; 32],
    /// Complete release-pinned verifier-set digest.
    pub vk_digest: [u8; 32],
    /// Enabled-profile constants committed by this key.
    pub enabled_hardware_profiles: [[u8; 32]; KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1],
}

/// Generated constant-size mint-authorization proof and recursive openings.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaGeneratedMintAuthorizationProofV1 {
    /// Exact Eq public column (84 field elements).
    pub eq_public_instances: Vec<Fp>,
    /// Exact Ep public column (84 field elements).
    pub ep_public_instances: Vec<Fq>,
    /// Compact paired authorization proof.
    pub proof: KagemushaPairedProofV1,
    /// Eq current opening claim extracted from the proof.
    pub eq_current_accumulator: KagemushaEqAccumulatorV1,
    /// Ep current opening claim extracted from the proof.
    pub ep_current_accumulator: KagemushaEpAccumulatorV1,
}

/// Complete private input for one stable recursive mint-authority carrier step.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone)]
pub struct KagemushaMintAuthorityGenerationWitnessV1<'a> {
    /// Fixed carrier branch.
    pub step: KagemushaMintAuthorityStepV1,
    /// Authenticated Kagemusha release identifier.
    pub release_id: [u8; 32],
    /// Release-pinned initial finality-roster identifier.
    pub genesis_roster_id: [u8; 32],
    /// Actual compiled Eq carrier protocol identity.
    pub eq_protocol_digest: [u8; 32],
    /// Actual compiled Ep carrier protocol identity.
    pub ep_protocol_digest: [u8; 32],
    /// Eq deferred-equation audit exposed by this carrier proof.
    pub eq_deferred_audit: [u8; 32],
    /// Ep deferred-equation audit exposed by this carrier proof.
    pub ep_deferred_audit: [u8; 32],
    /// Fixed-shape membership, quorum, and roster witness.
    pub certificate: KagemushaMintCertificateWitnessV1,
    /// Eq predecessor carrier protocol.
    pub eq_parent_protocol: &'a PlonkProtocol<EqAffine>,
    /// Ep predecessor carrier protocol.
    pub ep_parent_protocol: &'a PlonkProtocol<EpAffine>,
    /// Eq predecessor carrier public instances.
    pub eq_parent_instances: &'a [Vec<Fp>],
    /// Ep predecessor carrier public instances.
    pub ep_parent_instances: &'a [Vec<Fq>],
    /// Eq predecessor carrier proof.
    pub eq_parent_proof: &'a [u8],
    /// Ep predecessor carrier proof.
    pub ep_parent_proof: &'a [u8],
    /// Eq predecessor carried history.
    pub eq_parent_history: &'a KagemushaEqAccumulatorV1,
    /// Ep predecessor carried history.
    pub ep_parent_history: &'a KagemushaEpAccumulatorV1,
    /// Eq proof folding the predecessor current claim into its history.
    pub eq_parent_fold_proof: &'a KagemushaEqFoldProofV1,
    /// Ep proof folding the predecessor current claim into its history.
    pub ep_parent_fold_proof: &'a KagemushaEpFoldProofV1,
    /// Eq history produced by this carrier step.
    pub eq_successor_history: &'a KagemushaEqAccumulatorV1,
    /// Ep history produced by this carrier step.
    pub ep_successor_history: &'a KagemushaEpAccumulatorV1,
}

/// Generated parameter/key bytes and authenticated protocol identities for the stable carrier.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug)]
pub struct KagemushaGeneratedMintAuthorityArtifactsV1 {
    /// Canonical Eq transparent IPA parameters.
    pub eq_parameters: Arc<[u8]>,
    /// Canonical Ep transparent IPA parameters.
    pub ep_parameters: Arc<[u8]>,
    /// Processed Eq mint-authority proving key.
    pub eq_proving_key: Arc<[u8]>,
    /// Processed Ep mint-authority proving key.
    pub ep_proving_key: Arc<[u8]>,
    /// Processed Eq mint-authority verifying key.
    pub eq_verifying_key: Arc<[u8]>,
    /// Processed Ep mint-authority verifying key.
    pub ep_verifying_key: Arc<[u8]>,
    /// Exact Eq circuit layout.
    pub eq_circuit_params: BaseCircuitParams,
    /// Exact Ep circuit layout.
    pub ep_circuit_params: BaseCircuitParams,
    /// Private Eq SHA/quorum carrier proving key, never transported as a credit proof.
    pub inner_eq_proving_key: Arc<[u8]>,
    /// Private Ep SHA/quorum carrier proving key.
    pub inner_ep_proving_key: Arc<[u8]>,
    /// Private Eq carrier verifying key pinned by the compact outer key.
    pub inner_eq_verifying_key: Arc<[u8]>,
    /// Private Ep carrier verifying key pinned by the compact outer key.
    pub inner_ep_verifying_key: Arc<[u8]>,
    /// Exact private Eq carrier layout.
    pub inner_eq_circuit_params: BaseCircuitParams,
    /// Exact private Ep carrier layout.
    pub inner_ep_circuit_params: BaseCircuitParams,
    /// Release identity used by the bootstrap witness.
    pub release_id: [u8; 32],
    /// Actual compiled Eq carrier protocol identity.
    pub eq_protocol_digest: [u8; 32],
    /// Actual compiled Ep carrier protocol identity.
    pub ep_protocol_digest: [u8; 32],
    /// Release-pinned genesis roster identifier used by the generated release.
    pub genesis_roster_id: [u8; 32],
}

/// Loaded Eq parameters and keys for production mint-authority proving.
#[cfg(feature = "zk-halo2-ipa")]
pub struct KagemushaLoadedEqMintAuthorityArtifactsV1 {
    /// Canonical Eq transparent IPA parameters.
    pub parameters: ParamsIPA<EqAffine>,
    /// Exact processed Eq mint-authority proving key.
    pub proving_key: ProvingKey<EqAffine>,
    /// Exact processed Eq mint-authority verifying key.
    pub verifying_key: VerifyingKey<EqAffine>,
    /// Authenticated circuit layout.
    pub circuit_params: BaseCircuitParams,
    /// Private SHA/quorum carrier proving key.
    pub inner_proving_key: ProvingKey<EqAffine>,
    /// Private carrier verifying key pinned by the compact outer key.
    pub inner_verifying_key: VerifyingKey<EqAffine>,
    /// Authenticated private-carrier layout.
    pub inner_circuit_params: BaseCircuitParams,
    /// Authenticated compact outer protocol identity.
    pub protocol_digest: [u8; 32],
    /// Authenticated release identity.
    pub release_id: [u8; 32],
    /// Authenticated recursive profile identity.
    pub profile_digest: [u8; 32],
    /// Authenticated artifact manifest identity.
    pub artifact_manifest_digest: [u8; 32],
    /// Release-pinned initial roster identity.
    pub genesis_roster_id: [u8; 32],
}

/// Loaded Ep parameters and keys for production mint-authority proving.
#[cfg(feature = "zk-halo2-ipa")]
pub struct KagemushaLoadedEpMintAuthorityArtifactsV1 {
    /// Canonical Ep transparent IPA parameters.
    pub parameters: ParamsIPA<EpAffine>,
    /// Exact processed Ep mint-authority proving key.
    pub proving_key: ProvingKey<EpAffine>,
    /// Exact processed Ep mint-authority verifying key.
    pub verifying_key: VerifyingKey<EpAffine>,
    /// Authenticated circuit layout.
    pub circuit_params: BaseCircuitParams,
    /// Private SHA/quorum carrier proving key.
    pub inner_proving_key: ProvingKey<EpAffine>,
    /// Private carrier verifying key pinned by the compact outer key.
    pub inner_verifying_key: VerifyingKey<EpAffine>,
    /// Authenticated private-carrier layout.
    pub inner_circuit_params: BaseCircuitParams,
    /// Authenticated compact outer protocol identity.
    pub protocol_digest: [u8; 32],
    /// Authenticated release identity.
    pub release_id: [u8; 32],
    /// Authenticated recursive profile identity.
    pub profile_digest: [u8; 32],
    /// Authenticated artifact manifest identity.
    pub artifact_manifest_digest: [u8; 32],
    /// Release-pinned initial roster identity.
    pub genesis_roster_id: [u8; 32],
}

/// One generated stable carrier proof with the exact fields required to construct a mint credit.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaGeneratedMintAuthorityProofV1 {
    /// Eq public column required by a later authority or state proof.
    pub eq_public_instances: Vec<Fp>,
    /// Ep public column required by a later authority or state proof.
    pub ep_public_instances: Vec<Fq>,
    /// Complete constant-size paired proof.
    pub proof: KagemushaPairedProofV1,
    /// Eq current opening claim extracted for later carrier/state-history folds.
    pub eq_current_accumulator: KagemushaEqAccumulatorV1,
    /// Ep current opening claim extracted for later carrier/state-history folds.
    pub ep_current_accumulator: KagemushaEpAccumulatorV1,
    /// Exact certificate binding constrained by the proof.
    pub certificate_binding: [u8; 32],
    /// Recursively authenticated current authority head.
    pub authority_head: [u8; 32],
    /// Authenticated proof-release identifier constrained by the carrier.
    pub release_id: [u8; 32],
    /// Release-pinned genesis roster identifier.
    pub genesis_roster_id: [u8; 32],
    /// Canonical binding of both complete helper public transcripts.
    pub proof_binding_digest: [u8; 32],
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaGeneratedMintAuthorityProofV1 {
    /// Convert a bootstrap or rotation result into its durable authority checkpoint.
    pub fn into_checkpoint(
        self,
        step: KagemushaMintAuthorityStepV1,
        statement: iroha_data_model::kagemusha::KagemushaMintCreditStatementV1,
    ) -> Result<KagemushaMintAuthorityCheckpointV1, KagemushaArtifactGenerationErrorV1> {
        let checkpoint = KagemushaMintAuthorityCheckpointV1 {
            step,
            statement,
            certificate_binding: self.certificate_binding,
            authority_head: self.authority_head,
            release_id: self.release_id,
            genesis_roster_id: self.genesis_roster_id,
            proof_binding_digest: self.proof_binding_digest,
            proof: self.proof,
        };
        checkpoint
            .validate_shape()
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
        Ok(checkpoint)
    }
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaGeneratedMintAuthorityArtifactsV1 {
    /// Return the ten distinct parameter/inner/outer release-manifest bindings.
    #[must_use]
    pub fn bindings(&self) -> [KagemushaArtifactBindingV1; 10] {
        [
            binding(KagemushaArtifactRoleV1::ParamsEq, &self.eq_parameters),
            binding(KagemushaArtifactRoleV1::ParamsEp, &self.ep_parameters),
            binding(
                KagemushaArtifactRoleV1::MintCreditPkEq,
                &self.eq_proving_key,
            ),
            binding(
                KagemushaArtifactRoleV1::MintCreditVkEq,
                &self.eq_verifying_key,
            ),
            binding(
                KagemushaArtifactRoleV1::MintCreditPkEp,
                &self.ep_proving_key,
            ),
            binding(
                KagemushaArtifactRoleV1::MintCreditVkEp,
                &self.ep_verifying_key,
            ),
            binding(
                KagemushaArtifactRoleV1::InnerMintCreditPkEq,
                &self.inner_eq_proving_key,
            ),
            binding(
                KagemushaArtifactRoleV1::InnerMintCreditVkEq,
                &self.inner_eq_verifying_key,
            ),
            binding(
                KagemushaArtifactRoleV1::InnerMintCreditPkEp,
                &self.inner_ep_proving_key,
            ),
            binding(
                KagemushaArtifactRoleV1::InnerMintCreditVkEp,
                &self.inner_ep_verifying_key,
            ),
        ]
    }

    /// Install all generated files into an embedded content-addressed resolver.
    pub fn install_into(&self, resolver: &mut KagemushaMemoryArtifactResolverV1) {
        for bytes in [
            &self.eq_parameters,
            &self.ep_parameters,
            &self.eq_proving_key,
            &self.eq_verifying_key,
            &self.ep_proving_key,
            &self.ep_verifying_key,
            &self.inner_eq_proving_key,
            &self.inner_eq_verifying_key,
            &self.inner_ep_proving_key,
            &self.inner_ep_verifying_key,
        ] {
            resolver.insert(Arc::clone(bytes));
        }
    }
}

/// Deterministic generation or exact key-decoding failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum KagemushaArtifactGenerationErrorV1 {
    /// Artifact resolution or authentication of the complete input stream failed.
    #[error(transparent)]
    Artifact(#[from] KagemushaArtifactErrorV1),
    /// Halo2 rejected the fixed circuit while deriving a key.
    #[error("failed to generate Kagemusha V1 {parity:?} {kind}: {reason}")]
    KeyGeneration {
        /// Pasta parity being generated.
        parity: KagemushaPastaParityV1,
        /// Human-readable key kind.
        kind: &'static str,
        /// Backend failure.
        reason: String,
    },
    /// A deterministic artifact exceeded the release manifest's fixed bound.
    #[error("generated Kagemusha V1 {parity:?} {kind} has invalid length {actual}")]
    InvalidLength {
        /// Pasta parity being generated.
        parity: KagemushaPastaParityV1,
        /// Human-readable artifact kind.
        kind: &'static str,
        /// Generated length.
        actual: u64,
    },
    /// The configured uncompressed Processed key layout cannot fit immutable helper limits.
    #[error(
        "Kagemusha V1 {parity:?} {kind} uncompressed Processed key profile (advice={advice_columns}, instance={instance_columns}, fixed={configured_fixed_columns}, selectors={selector_columns}, permutation={permutation_columns}) predicts proving-key length {predicted_proving_key_bytes}/{proving_key_maximum} and verifying-key length {predicted_verifying_key_bytes}/{verifying_key_maximum}"
    )]
    PredictedKeyResourceLimit {
        /// Pasta parity being configured.
        parity: KagemushaPastaParityV1,
        /// Human-readable circuit family.
        kind: &'static str,
        /// Advice columns configured by the circuit.
        advice_columns: u64,
        /// Instance columns configured by the circuit.
        instance_columns: u64,
        /// Fixed columns configured before selector materialization.
        configured_fixed_columns: u64,
        /// Selectors materialized one-for-one as fixed columns.
        selector_columns: u64,
        /// Columns participating in the permutation argument.
        permutation_columns: u64,
        /// Exact predicted Processed proving-key bytes.
        predicted_proving_key_bytes: u64,
        /// Immutable helper proving-key maximum.
        proving_key_maximum: u64,
        /// Exact predicted Processed verifying-key bytes.
        predicted_verifying_key_bytes: u64,
        /// Immutable verifying-key maximum.
        verifying_key_maximum: u64,
    },
    /// A compiled externally transported proof cannot fit its immutable wire slot.
    #[error(
        "Kagemusha V1 {parity:?} {kind} profile requires {actual} proof bytes (W={witness_commitments}, T={quotient_commitments}, E={evaluations}, Q={bgh19_rotation_sets}); wire maximum is {maximum}"
    )]
    TransportProofProfileTooLarge {
        /// Pasta parity being compiled.
        parity: KagemushaPastaParityV1,
        /// Externally transported circuit family.
        kind: &'static str,
        /// Witness commitments across all phases.
        witness_commitments: u64,
        /// Quotient commitments.
        quotient_commitments: u64,
        /// Transcript evaluations.
        evaluations: u64,
        /// Distinct BGH19 query rotation sets.
        bgh19_rotation_sets: u64,
        /// Exact proof bytes implied by the compiled protocol.
        actual: u64,
        /// Immutable per-parity wire maximum.
        maximum: u64,
    },
    /// A processed key was malformed or had trailing bytes.
    #[error("failed to decode Kagemusha V1 {parity:?} {kind}: {reason}")]
    KeyDecode {
        /// Pasta parity being decoded.
        parity: KagemushaPastaParityV1,
        /// Human-readable key kind.
        kind: &'static str,
        /// Exact decode failure.
        reason: String,
    },
    /// Witness construction or circuit layout failed before keying/proving.
    #[error("failed to build Kagemusha V1 proof circuit: {0}")]
    CircuitBuild(String),
    /// A witness rebuilt a different circuit layout than the authenticated proving key.
    #[error("Kagemusha V1 {0:?} circuit profile does not match the proving key")]
    CircuitProfileMismatch(KagemushaPastaParityV1),
    /// Halo2 failed to produce a recursive proof.
    #[error("failed to prove Kagemusha V1 {parity:?} circuit: {reason}")]
    ProofGeneration {
        /// Pasta parity being proved.
        parity: KagemushaPastaParityV1,
        /// Backend failure.
        reason: String,
    },
}

/// Generate private and compact mint-authority keys from an actual bootstrap witness.
///
/// The generated verifying keys do not contain a validator roster. Roster authority is carried
/// recursively from the release-pinned genesis identifier, so later epoch rotation does not
/// require a new proof release. The supplied parent protocols seed a bounded key-shape
/// convergence search; they never authorize a spend or replace the real inner bootstrap proof.
/// Final keys are checked again against their actual compact protocol identities before export.
///
/// # Errors
///
/// Returns an error for an invalid carrier witness, circuit profile, key, artifact length, or
/// protocol-role alias.
#[cfg(feature = "zk-halo2-ipa")]
pub fn generate_kagemusha_mint_authority_artifacts_v1(
    witness: KagemushaMintAuthorityGenerationWitnessV1<'_>,
) -> Result<KagemushaGeneratedMintAuthorityArtifactsV1, KagemushaArtifactGenerationErrorV1> {
    mint_authority_generation::generate(witness)
}

/// Load authenticated Eq mint-authority parameters and keys for production proving.
///
/// # Errors
///
/// Rejects an invalid layout, missing/substituted bytes, malformed key, trailing bytes, or a
/// proving key whose embedded verifier differs from the standalone authenticated key.
#[cfg(feature = "zk-halo2-ipa")]
pub fn load_kagemusha_eq_mint_authority_artifacts_v1<R>(
    artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
    profile: &super::native_backend::KagemushaRecursiveVerifierProfileV1,
) -> Result<KagemushaLoadedEqMintAuthorityArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    R: KagemushaArtifactByteResolverV1,
{
    let circuit_params = profile.mint_eq.clone();
    let inner_circuit_params = profile.inner_mint_eq.clone();
    validate_recursive_profile(KagemushaPastaParityV1::Eq, &circuit_params)?;
    validate_recursive_profile(KagemushaPastaParityV1::Eq, &inner_circuit_params)?;
    profile.validate_against_artifacts(artifacts)?;
    let release = artifacts.recursion_artifacts();
    let parameters = artifacts.load_eq_params()?;
    let vk_bytes = artifacts.resolve(KagemushaArtifactRoleV1::MintCreditVkEq)?;
    let verifying_key = read_eq_mint_vk(vk_bytes.as_ref(), circuit_params.clone())?;
    let proving_key = load_authenticated_proving_key_v1::<
        EqAffine,
        KagemushaMintAuthorityTransportEqCircuitV1,
        _,
    >(
        artifacts,
        KagemushaArtifactRoleV1::MintCreditPkEq,
        KagemushaPastaParityV1::Eq,
        circuit_params.clone(),
    )?;
    ensure_embedded_vk(KagemushaPastaParityV1::Eq, &proving_key, vk_bytes.as_ref())?;
    let inner_vk_bytes = artifacts.resolve(KagemushaArtifactRoleV1::InnerMintCreditVkEq)?;
    let inner_verifying_key =
        read_eq_inner_mint_vk(inner_vk_bytes.as_ref(), inner_circuit_params.clone())?;
    let inner_proving_key =
        load_authenticated_proving_key_v1::<EqAffine, KagemushaMintAuthorityEqCircuitV1, _>(
            artifacts,
            KagemushaArtifactRoleV1::InnerMintCreditPkEq,
            KagemushaPastaParityV1::Eq,
            inner_circuit_params.clone(),
        )?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Eq,
        &inner_proving_key,
        inner_vk_bytes.as_ref(),
    )?;
    let protocol = compile(
        &parameters,
        &verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
    );
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Eq,
        "mint-authority carrier",
        &protocol,
    )?;
    let protocol_digest = native_parent_protocol_digest_v1(&protocol, KagemushaPastaParityV1::Eq)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if protocol_digest != release.mint_finality_eq_protocol_digest
        || protocol_digest != profile.mint_eq_protocol_digest
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "authenticated Eq MintAuthority protocol differs from its compact key".to_owned(),
        ));
    }
    Ok(KagemushaLoadedEqMintAuthorityArtifactsV1 {
        parameters,
        proving_key,
        verifying_key,
        circuit_params,
        inner_proving_key,
        inner_verifying_key,
        inner_circuit_params,
        protocol_digest,
        release_id: release.release_id,
        profile_digest: release.profile_digest,
        artifact_manifest_digest: release.artifact_manifest_digest,
        genesis_roster_id: profile.mint_genesis_roster_id,
    })
}

/// Load authenticated Ep mint-authority parameters and keys for production proving.
///
/// # Errors
///
/// Rejects an invalid layout, missing/substituted bytes, malformed key, trailing bytes, or a
/// proving key whose embedded verifier differs from the standalone authenticated key.
#[cfg(feature = "zk-halo2-ipa")]
pub fn load_kagemusha_ep_mint_authority_artifacts_v1<R>(
    artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
    profile: &super::native_backend::KagemushaRecursiveVerifierProfileV1,
) -> Result<KagemushaLoadedEpMintAuthorityArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    R: KagemushaArtifactByteResolverV1,
{
    let circuit_params = profile.mint_ep.clone();
    let inner_circuit_params = profile.inner_mint_ep.clone();
    validate_recursive_profile(KagemushaPastaParityV1::Ep, &circuit_params)?;
    validate_recursive_profile(KagemushaPastaParityV1::Ep, &inner_circuit_params)?;
    profile.validate_against_artifacts(artifacts)?;
    let release = artifacts.recursion_artifacts();
    let parameters = artifacts.load_ep_params()?;
    let vk_bytes = artifacts.resolve(KagemushaArtifactRoleV1::MintCreditVkEp)?;
    let verifying_key = read_ep_mint_vk(vk_bytes.as_ref(), circuit_params.clone())?;
    let proving_key = load_authenticated_proving_key_v1::<
        EpAffine,
        KagemushaMintAuthorityTransportEpCircuitV1,
        _,
    >(
        artifacts,
        KagemushaArtifactRoleV1::MintCreditPkEp,
        KagemushaPastaParityV1::Ep,
        circuit_params.clone(),
    )?;
    ensure_embedded_vk(KagemushaPastaParityV1::Ep, &proving_key, vk_bytes.as_ref())?;
    let inner_vk_bytes = artifacts.resolve(KagemushaArtifactRoleV1::InnerMintCreditVkEp)?;
    let inner_verifying_key =
        read_ep_inner_mint_vk(inner_vk_bytes.as_ref(), inner_circuit_params.clone())?;
    let inner_proving_key =
        load_authenticated_proving_key_v1::<EpAffine, KagemushaMintAuthorityEpCircuitV1, _>(
            artifacts,
            KagemushaArtifactRoleV1::InnerMintCreditPkEp,
            KagemushaPastaParityV1::Ep,
            inner_circuit_params.clone(),
        )?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Ep,
        &inner_proving_key,
        inner_vk_bytes.as_ref(),
    )?;
    let protocol = compile(
        &parameters,
        &verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
    );
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Ep,
        "mint-authority carrier",
        &protocol,
    )?;
    let protocol_digest = native_parent_protocol_digest_v1(&protocol, KagemushaPastaParityV1::Ep)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if protocol_digest != release.mint_finality_ep_protocol_digest
        || protocol_digest != profile.mint_ep_protocol_digest
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "authenticated Ep MintAuthority protocol differs from its compact key".to_owned(),
        ));
    }
    Ok(KagemushaLoadedEpMintAuthorityArtifactsV1 {
        parameters,
        proving_key,
        verifying_key,
        circuit_params,
        inner_proving_key,
        inner_verifying_key,
        inner_circuit_params,
        protocol_digest,
        release_id: release.release_id,
        profile_digest: release.profile_digest,
        artifact_manifest_digest: release.artifact_manifest_digest,
        genesis_roster_id: profile.mint_genesis_roster_id,
    })
}

/// Prepare a compact authority circuit from a genuine, terminally decided private proof.
/// The inner pair SHA is retained while outer audits and folded history are derived afresh.
#[cfg(feature = "zk-halo2-ipa")]
fn prepare_mint_authority_transport_v1(
    eq: KagemushaMintAuthorizationInnerKeysV1<'_, EqAffine>,
    ep: KagemushaMintAuthorizationInnerKeysV1<'_, EpAffine>,
    mut witness: KagemushaMintAuthorityGenerationWitnessV1<'_>,
) -> Result<KagemushaPreparedMintAuthorityTransportV1, KagemushaArtifactGenerationErrorV1> {
    witness
        .certificate
        .validate_for_step(witness.step)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let semantic_digest = witness
        .certificate
        .statement
        .canonical_digest()
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let amount = witness.certificate.statement.amount;
    let certificate_binding = witness
        .certificate
        .certificate_binding_digest(witness.step)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let roster_id = witness
        .certificate
        .epoch_roster
        .finality_epoch_id()
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let authority_head = match witness.step {
        KagemushaMintAuthorityStepV1::Rotate => witness
            .certificate
            .seal_bundle
            .message
            .next_finality_epoch_id
            .ok_or_else(|| {
                KagemushaArtifactGenerationErrorV1::CircuitBuild(
                    "mint-authority rotation lacks its next roster identifier".to_owned(),
                )
            })?,
        KagemushaMintAuthorityStepV1::Bootstrap | KagemushaMintAuthorityStepV1::FinalizedMint => {
            roster_id
        }
    };
    let eq_protocol_digest = witness.eq_protocol_digest;
    let ep_protocol_digest = witness.ep_protocol_digest;
    let release_id = witness.release_id;
    let genesis_roster_id = witness.genesis_roster_id;
    // Deferred audit values are circuit outputs.  Derive them from a first fixed-shape build,
    // then rebuild with those exact public cells; caller-supplied guesses never gain authority.
    witness.eq_deferred_audit = [1; 32];
    witness.ep_deferred_audit = [2; 32];
    let (_, _, eq_deferred_audit, ep_deferred_audit) =
        build_mint_authority_generation_pair(eq.parameters, ep.parameters, witness.clone())?;
    witness.eq_deferred_audit = eq_deferred_audit;
    witness.ep_deferred_audit = ep_deferred_audit;
    let eq_history = witness.eq_successor_history.clone();
    let ep_history = witness.ep_successor_history.clone();
    let eq_protocol = compile(
        eq.parameters,
        eq.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_protocol = compile(
        ep.parameters,
        ep.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let proof_binding_digest = KagemushaMintAuthorityPairBindingV1 {
        step: witness.step,
        semantic_digest,
        amount,
        certificate_binding,
        authority_head,
        release_id,
        genesis_roster_id,
        eq_protocol_digest,
        ep_protocol_digest,
        eq_deferred_audit,
        ep_deferred_audit,
        eq_history: eq_history.as_bytes(),
        ep_history: ep_history.as_bytes(),
    }
    .canonical_digest();
    let eq_instances = mint_authority_public_instances::<Fp>(
        witness.step,
        semantic_digest,
        amount,
        certificate_binding,
        authority_head,
        release_id,
        genesis_roster_id,
        eq_protocol_digest,
        ep_protocol_digest,
        eq_deferred_audit,
        ep_deferred_audit,
        proof_binding_digest,
        eq_history.as_bytes(),
    )?;
    let ep_instances = mint_authority_public_instances::<Fq>(
        witness.step,
        semantic_digest,
        amount,
        certificate_binding,
        authority_head,
        release_id,
        genesis_roster_id,
        eq_protocol_digest,
        ep_protocol_digest,
        eq_deferred_audit,
        ep_deferred_audit,
        proof_binding_digest,
        ep_history.as_bytes(),
    )?;
    let step = witness.step;
    let (eq_circuit, ep_circuit, rebuilt_eq_audit, rebuilt_ep_audit) =
        build_mint_authority_generation_pair(eq.parameters, ep.parameters, witness)?;
    if rebuilt_eq_audit != eq_deferred_audit || rebuilt_ep_audit != ep_deferred_audit {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "mint-authority deferred audit changed while binding its public cells".to_owned(),
        ));
    }
    if !same_base_params(&eq_circuit.params(), eq.circuit_params) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
            KagemushaPastaParityV1::Eq,
        ));
    }
    if !same_base_params(&ep_circuit.params(), ep.circuit_params) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
            KagemushaPastaParityV1::Ep,
        ));
    }
    // Private SHA/quorum proofs have no transport slot. Their actual opening claims must
    // decide before a compact wrapper is constructed, including during bootstrap keying.
    let eq_proof = create_mint_eq_proof(eq.parameters, eq.proving_key, eq_circuit, &eq_instances)?;
    let ep_proof = create_mint_ep_proof(ep.parameters, ep.proving_key, ep_circuit, &ep_instances)?;
    let eq_current_accumulator = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(eq.parameters, &eq_protocol, &eq_proof, &eq_instances)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_current_accumulator = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(ep.parameters, &ep_protocol, &ep_proof, &ep_instances)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    decide_kagemusha_eq_accumulator_v1(eq.parameters, &eq_current_accumulator)
        .and_then(|()| decide_kagemusha_eq_accumulator_v1(eq.parameters, &eq_history))
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    decide_kagemusha_ep_accumulator_v1(ep.parameters, &ep_current_accumulator)
        .and_then(|()| decide_kagemusha_ep_accumulator_v1(ep.parameters, &ep_history))
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let eq_fold = fold_kagemusha_eq_accumulators_with_rng_v1(
        eq.parameters,
        &eq_current_accumulator,
        &eq_history,
        OsRng,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_fold = fold_kagemusha_ep_accumulators_with_rng_v1(
        ep.parameters,
        &ep_current_accumulator,
        &ep_history,
        OsRng,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let eq_native_history = eq_history
        .to_native()
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_native_history = ep_history
        .to_native()
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    // Preserve the inner pair SHA at cells20..22. Rehashing outer metadata here would prove
    // a different statement and sever the inner certificate binding.
    let outer_instances = |eq_audit, ep_audit| -> Result<_, KagemushaArtifactGenerationErrorV1> {
        Ok((
            mint_authority_public_instances::<Fp>(
                step,
                semantic_digest,
                amount,
                certificate_binding,
                authority_head,
                release_id,
                genesis_roster_id,
                eq_protocol_digest,
                ep_protocol_digest,
                eq_audit,
                ep_audit,
                proof_binding_digest,
                eq_fold.successor().as_bytes(),
            )?,
            mint_authority_public_instances::<Fq>(
                step,
                semantic_digest,
                amount,
                certificate_binding,
                authority_head,
                release_id,
                genesis_roster_id,
                eq_protocol_digest,
                ep_protocol_digest,
                eq_audit,
                ep_audit,
                proof_binding_digest,
                ep_fold.successor().as_bytes(),
            )?,
        ))
    };
    let build_outer = |eq_outer: &[Fp], ep_outer: &[Fq]| {
        build_kagemusha_mint_authority_transport_pair_v1(
            eq.parameters,
            ep.parameters,
            KagemushaMintTransportDeciderWitnessV1 {
                eq: KagemushaMintTransportParityWitnessV1 {
                    inner_protocol: &eq_protocol,
                    inner_instances: &eq_instances,
                    inner_proof: &eq_proof,
                    inner_history: &eq_native_history,
                    inner_history_fold_proof: eq_fold.proof().as_bytes(),
                    outer_instances: eq_outer,
                },
                ep: KagemushaMintTransportParityWitnessV1 {
                    inner_protocol: &ep_protocol,
                    inner_instances: &ep_instances,
                    inner_proof: &ep_proof,
                    inner_history: &ep_native_history,
                    inner_history_fold_proof: ep_fold.proof().as_bytes(),
                    outer_instances: ep_outer,
                },
            },
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)
    };
    let (eq_outer, ep_outer) = outer_instances([1; 32], [2; 32])?;
    let (_, _, eq_deferred_audit, ep_deferred_audit) = build_outer(&eq_outer, &ep_outer)?;
    let (eq_outer, ep_outer) = outer_instances(eq_deferred_audit, ep_deferred_audit)?;
    let (eq_circuit, ep_circuit, rebuilt_eq, rebuilt_ep) = build_outer(&eq_outer, &ep_outer)?;
    if (rebuilt_eq, rebuilt_ep) != (eq_deferred_audit, ep_deferred_audit) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "mint-authority transport audit changed while binding public cells".to_owned(),
        ));
    }
    Ok(KagemushaPreparedMintAuthorityTransportV1 {
        eq_circuit,
        ep_circuit,
        eq_instances: eq_outer,
        ep_instances: ep_outer,
        eq_history: eq_fold.successor().clone(),
        ep_history: ep_fold.successor().clone(),
        eq_deferred_audit,
        ep_deferred_audit,
        semantic_digest,
        certificate_binding,
        authority_head,
        release_id,
        genesis_roster_id,
        proof_binding_digest,
    })
}

/// Create the release-pinned zero-authority checkpoint from the genesis roster.
///
/// Bootstrap is generated only after the surrounding release manifest has been authenticated.
/// Its parent and fold transcripts are fixed-shape parser witnesses whose equations are disabled
/// by the public bootstrap selector. Only the inner predecessor history is a canonical
/// terminally decided empty IPA accumulator. The compact output always folds a real inner
/// bootstrap proof into that history. This avoids a self-referential
/// release/profile digest while retaining the exact same circuit and proof shape as every later
/// mint-authority step.
///
/// # Errors
///
/// Rejects a certificate not bound to the pinned genesis roster and release, invalid proving
/// artifacts, malformed fixed-shape parser material, proof generation failure, or an undecided
/// bootstrap history.
#[cfg(feature = "zk-halo2-ipa")]
pub fn prove_kagemusha_mint_authority_bootstrap_v1(
    eq: &KagemushaLoadedEqMintAuthorityArtifactsV1,
    ep: &KagemushaLoadedEpMintAuthorityArtifactsV1,
    release_id: [u8; 32],
    genesis_roster_id: [u8; 32],
    certificate: KagemushaMintCertificateWitnessV1,
) -> Result<KagemushaMintAuthorityCheckpointV1, KagemushaArtifactGenerationErrorV1> {
    certificate
        .validate_for_step(KagemushaMintAuthorityStepV1::Bootstrap)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let roster_id = certificate
        .epoch_roster
        .finality_epoch_id()
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    if release_id == [0; 32]
        || genesis_roster_id == [0; 32]
        || certificate.statement.lifecycle.release_id != release_id
        || roster_id != genesis_roster_id
        || certificate.seal_bundle.message.finality_epoch_id != genesis_roster_id
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "mint-authority bootstrap differs from its authenticated release or genesis roster"
                .to_owned(),
        ));
    }

    let eq_protocol = compile(
        &eq.parameters,
        &eq.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_protocol = compile(
        &ep.parameters,
        &ep.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let eq_protocol_digest =
        native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let ep_protocol_digest =
        native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;

    let eq_history = initial_kagemusha_eq_accumulator_v1(&eq.parameters)
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_history = initial_kagemusha_ep_accumulator_v1(&ep.parameters)
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let eq_parent_instances = bootstrap_parent_instances::<Fp>(eq_history.as_bytes());
    let ep_parent_instances = bootstrap_parent_instances::<Fq>(ep_history.as_bytes());

    let eq_point = EqAffine::generator().to_bytes();
    let ep_point = EpAffine::generator().to_bytes();
    let eq_parent_proof =
        dummy_ordinary_proof_bytes(&eq_protocol, eq_point.as_ref(), KagemushaPastaParityV1::Eq)?;
    let ep_parent_proof =
        dummy_ordinary_proof_bytes(&ep_protocol, ep_point.as_ref(), KagemushaPastaParityV1::Ep)?;
    let eq_fold_proof =
        KagemushaEqFoldProofV1::try_from_bytes(&dummy_fold_proof_bytes(eq_point.as_ref()))
            .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_fold_proof =
        KagemushaEpFoldProofV1::try_from_bytes(&dummy_fold_proof_bytes(ep_point.as_ref()))
            .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let statement = certificate.statement.clone();

    prove_kagemusha_mint_authority_v1(
        eq,
        ep,
        KagemushaMintAuthorityGenerationWitnessV1 {
            step: KagemushaMintAuthorityStepV1::Bootstrap,
            release_id,
            genesis_roster_id,
            eq_protocol_digest,
            ep_protocol_digest,
            eq_deferred_audit: [1; 32],
            ep_deferred_audit: [2; 32],
            certificate,
            eq_parent_protocol: &eq_protocol,
            ep_parent_protocol: &ep_protocol,
            eq_parent_instances: &eq_parent_instances,
            ep_parent_instances: &ep_parent_instances,
            eq_parent_proof: &eq_parent_proof,
            ep_parent_proof: &ep_parent_proof,
            eq_parent_history: &eq_history,
            ep_parent_history: &ep_history,
            eq_parent_fold_proof: &eq_fold_proof,
            ep_parent_fold_proof: &ep_fold_proof,
            eq_successor_history: &eq_history,
            ep_successor_history: &ep_history,
        },
    )?
    .into_checkpoint(KagemushaMintAuthorityStepV1::Bootstrap, statement)
}

#[cfg(feature = "zk-halo2-ipa")]
fn bootstrap_parent_instances<F: KagemushaPoseidonFieldV1>(
    history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Vec<Vec<F>> {
    let mut column = vec![F::ZERO; KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1];
    for (destination, chunk) in column[mint_authority_public_instance::HISTORY_START..]
        .iter_mut()
        .zip(history.chunks_exact(16))
    {
        *destination = from_u128::<F>(u128::from_le_bytes(
            chunk.try_into().expect("history chunk has sixteen bytes"),
        ));
    }
    vec![column]
}

#[cfg(feature = "zk-halo2-ipa")]
fn dummy_ordinary_proof_bytes<C>(
    protocol: &PlonkProtocol<C>,
    point: &[u8],
    parity: KagemushaPastaParityV1,
) -> Result<Vec<u8>, KagemushaArtifactGenerationErrorV1>
where
    C: snark_verifier::util::arithmetic::CurveAffine,
{
    if point.len() != 32 {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "dummy parent point encoding is not canonical Pasta width".to_owned(),
        ));
    }
    let profile = ordinary_ipa_proof_profile_v1(protocol)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let scalar = [0_u8; 32];
    let mut proof = Vec::with_capacity(profile.byte_len);
    for _ in 0..profile.witness_commitments {
        proof.extend_from_slice(point);
    }
    for _ in 0..profile.quotient_commitments {
        proof.extend_from_slice(point);
    }
    for _ in 0..profile.evaluations {
        proof.extend_from_slice(&scalar);
    }
    // BGH19 multi-opening proof: F, one evaluation per distinct rotation set, S, sixteen
    // left/right IPA points, c, blind, and the final basis point.
    proof.extend_from_slice(point);
    for _ in 0..profile.bgh19_rotation_sets {
        proof.extend_from_slice(&scalar);
    }
    proof.extend_from_slice(point);
    for _ in 0..(2 * super::KAGEMUSHA_RECURSION_IPA_K_V1 as usize) {
        proof.extend_from_slice(point);
    }
    proof.extend_from_slice(&scalar);
    proof.extend_from_slice(&scalar);
    proof.extend_from_slice(point);
    if proof.len() != profile.byte_len {
        return Err(KagemushaArtifactGenerationErrorV1::InvalidLength {
            parity,
            kind: "exact dummy parent proof",
            actual: u64::try_from(proof.len()).unwrap_or(u64::MAX),
        });
    }
    Ok(proof)
}

#[cfg(feature = "zk-halo2-ipa")]
fn dummy_fold_proof_bytes(point: &[u8]) -> Vec<u8> {
    let scalar = [0_u8; 32];
    let mut proof = Vec::with_capacity(super::KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1);
    proof.extend_from_slice(&scalar); // a
    proof.extend_from_slice(&scalar); // b
    proof.extend_from_slice(point); // U
    proof.extend_from_slice(&scalar); // omega
    proof.extend_from_slice(point); // C_bar
    proof.extend_from_slice(&scalar); // omega_prime
    for _ in 0..(2 * super::KAGEMUSHA_RECURSION_IPA_K_V1 as usize) {
        proof.extend_from_slice(point); // L_i, R_i
    }
    proof.extend_from_slice(point); // final U
    proof.extend_from_slice(&scalar); // c
    debug_assert_eq!(proof.len(), super::KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1);
    proof
}

/// Prove one finalized reserve top-up from a reverified durable authority checkpoint.
///
/// This is the production mint-outbox adapter: it derives both parent public columns, terminally
/// verifies the checkpoint, creates both BGH19 history folds, and invokes the fixed carrier
/// prover. No host-side finality or checkpoint predicate is accepted as proof authority.
#[cfg(feature = "zk-halo2-ipa")]
pub fn prove_kagemusha_finalized_mint_from_checkpoint_v1(
    eq: &KagemushaLoadedEqMintAuthorityArtifactsV1,
    ep: &KagemushaLoadedEpMintAuthorityArtifactsV1,
    verifier: &super::KagemushaAuthenticatedRecursiveVerifierV1,
    certificate: KagemushaMintCertificateWitnessV1,
    checkpoint: &KagemushaMintAuthorityCheckpointV1,
) -> Result<KagemushaGeneratedMintAuthorityProofV1, KagemushaArtifactGenerationErrorV1> {
    prove_kagemusha_mint_authority_from_checkpoint_v1(
        eq,
        ep,
        verifier,
        KagemushaMintAuthorityStepV1::FinalizedMint,
        certificate,
        checkpoint,
    )
}

/// Advance one reverified durable authority checkpoint to a quorum-authorized next roster.
///
/// The current roster proves the boundary seal inside both Pasta circuits. The returned
/// checkpoint carries the complete recursive proof/history and is suitable for immutable Kura
/// persistence; no native signature result grants authority to the successor roster.
#[cfg(feature = "zk-halo2-ipa")]
pub fn prove_kagemusha_mint_authority_rotation_from_checkpoint_v1(
    eq: &KagemushaLoadedEqMintAuthorityArtifactsV1,
    ep: &KagemushaLoadedEpMintAuthorityArtifactsV1,
    verifier: &super::KagemushaAuthenticatedRecursiveVerifierV1,
    certificate: KagemushaMintCertificateWitnessV1,
    checkpoint: &KagemushaMintAuthorityCheckpointV1,
) -> Result<KagemushaMintAuthorityCheckpointV1, KagemushaArtifactGenerationErrorV1> {
    let statement = certificate.statement.clone();
    prove_kagemusha_mint_authority_from_checkpoint_v1(
        eq,
        ep,
        verifier,
        KagemushaMintAuthorityStepV1::Rotate,
        certificate,
        checkpoint,
    )?
    .into_checkpoint(KagemushaMintAuthorityStepV1::Rotate, statement)
}

#[cfg(feature = "zk-halo2-ipa")]
fn prove_kagemusha_mint_authority_from_checkpoint_v1(
    eq: &KagemushaLoadedEqMintAuthorityArtifactsV1,
    ep: &KagemushaLoadedEpMintAuthorityArtifactsV1,
    verifier: &super::KagemushaAuthenticatedRecursiveVerifierV1,
    step: KagemushaMintAuthorityStepV1,
    certificate: KagemushaMintCertificateWitnessV1,
    checkpoint: &KagemushaMintAuthorityCheckpointV1,
) -> Result<KagemushaGeneratedMintAuthorityProofV1, KagemushaArtifactGenerationErrorV1> {
    if !matches!(
        step,
        KagemushaMintAuthorityStepV1::Rotate | KagemushaMintAuthorityStepV1::FinalizedMint
    ) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "a durable checkpoint may advance only by rotation or finalized mint".to_owned(),
        ));
    }
    let (eq_current, ep_current) = verifier
        .verify_mint_authority_checkpoint(checkpoint)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let roster_id = certificate
        .epoch_roster
        .finality_epoch_id()
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    if roster_id != checkpoint.authority_head
        || checkpoint.release_id != certificate.statement.lifecycle.release_id
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "finalized mint roster or release differs from its authority checkpoint".to_owned(),
        ));
    }
    let eq_parent_history = KagemushaEqAccumulatorV1::try_from_bytes(&checkpoint.proof.eq_history)
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_parent_history = KagemushaEpAccumulatorV1::try_from_bytes(&checkpoint.proof.ep_history)
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    // Online issuance persists the resulting checkpoint before serving it to a device. This
    // entropy path is deliberately separate from hardware post-commit envelope recovery.
    let eq_fold = fold_kagemusha_eq_accumulators_with_rng_v1(
        &eq.parameters,
        &eq_current,
        &eq_parent_history,
        OsRng,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_fold = fold_kagemusha_ep_accumulators_with_rng_v1(
        &ep.parameters,
        &ep_current,
        &ep_parent_history,
        OsRng,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let semantic = checkpoint
        .statement
        .canonical_digest()
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let eq_parent_instances = vec![mint_authority_public_instances::<Fp>(
        checkpoint.step,
        semantic,
        checkpoint.statement.amount,
        checkpoint.certificate_binding,
        checkpoint.authority_head,
        checkpoint.release_id,
        checkpoint.genesis_roster_id,
        checkpoint.proof.eq_protocol_digest,
        checkpoint.proof.ep_protocol_digest,
        checkpoint.proof.eq_deferred_audit,
        checkpoint.proof.ep_deferred_audit,
        checkpoint.proof_binding_digest,
        eq_parent_history.as_bytes(),
    )?];
    let ep_parent_instances = vec![mint_authority_public_instances::<Fq>(
        checkpoint.step,
        semantic,
        checkpoint.statement.amount,
        checkpoint.certificate_binding,
        checkpoint.authority_head,
        checkpoint.release_id,
        checkpoint.genesis_roster_id,
        checkpoint.proof.eq_protocol_digest,
        checkpoint.proof.ep_protocol_digest,
        checkpoint.proof.eq_deferred_audit,
        checkpoint.proof.ep_deferred_audit,
        checkpoint.proof_binding_digest,
        ep_parent_history.as_bytes(),
    )?];
    let eq_protocol = compile(
        &eq.parameters,
        &eq.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_protocol = compile(
        &ep.parameters,
        &ep.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
    );
    prove_kagemusha_mint_authority_v1(
        eq,
        ep,
        KagemushaMintAuthorityGenerationWitnessV1 {
            step,
            release_id: checkpoint.release_id,
            genesis_roster_id: checkpoint.genesis_roster_id,
            eq_protocol_digest: checkpoint.proof.eq_protocol_digest,
            ep_protocol_digest: checkpoint.proof.ep_protocol_digest,
            eq_deferred_audit: [1; 32],
            ep_deferred_audit: [2; 32],
            certificate,
            eq_parent_protocol: &eq_protocol,
            ep_parent_protocol: &ep_protocol,
            eq_parent_instances: &eq_parent_instances,
            ep_parent_instances: &ep_parent_instances,
            eq_parent_proof: &checkpoint.proof.eq_proof,
            ep_parent_proof: &checkpoint.proof.ep_proof,
            eq_parent_history: &eq_parent_history,
            ep_parent_history: &ep_parent_history,
            eq_parent_fold_proof: eq_fold.proof(),
            ep_parent_fold_proof: ep_fold.proof(),
            eq_successor_history: eq_fold.successor(),
            ep_successor_history: ep_fold.successor(),
        },
    )
}

/// Generate both complete recursive state-role key pairs from one valid fixed-shape witness.
///
/// This is the only production state-key generator. The returned protocol digests and
/// `halo2-base` layouts must be authenticated by the release manifest/profile alongside the
/// returned content-addressed artifact bindings.
///
/// # Errors
///
/// Returns an error if recursive witness construction, fixed-profile validation, key generation,
/// protocol compilation, or artifact-bound validation fails.
#[cfg(feature = "zk-halo2-ipa")]
fn build_recursive_generation_pair_v1(
    eq_parameters: &ParamsIPA<EqAffine>,
    ep_parameters: &ParamsIPA<EpAffine>,
    witness: KagemushaRecursiveStateGenerationWitnessV1<'_>,
) -> Result<
    (
        KagemushaRecursiveStateEqCircuitV1,
        KagemushaRecursiveStateEpCircuitV1,
        [u8; 32],
        [u8; 32],
    ),
    String,
> {
    let eq_incoming_slots = witness
        .eq_incoming_slots
        .map(KagemushaRecursiveIncomingEqGenerationWitnessV1::into_composite);
    let ep_incoming_slots = witness
        .ep_incoming_slots
        .map(KagemushaRecursiveIncomingEpGenerationWitnessV1::into_composite);
    build_kagemusha_recursive_state_pair_v1(
        eq_parameters,
        ep_parameters,
        witness.into_recursive(&eq_incoming_slots, &ep_incoming_slots),
    )
}

#[cfg(feature = "zk-halo2-ipa")]
struct KagemushaPrivateCarrierProofV1 {
    eq_instances: Vec<Fp>,
    ep_instances: Vec<Fq>,
    eq_proof: Vec<u8>,
    ep_proof: Vec<u8>,
    eq_history: KagemushaEqAccumulatorV1,
    ep_history: KagemushaEpAccumulatorV1,
}

#[cfg(feature = "zk-halo2-ipa")]
#[allow(clippy::too_many_arguments)]
fn prove_private_recursive_carrier_v1(
    eq_parameters: &ParamsIPA<EqAffine>,
    ep_parameters: &ParamsIPA<EpAffine>,
    eq_proving_key: &ProvingKey<EqAffine>,
    ep_proving_key: &ProvingKey<EpAffine>,
    eq_circuit_params: &BaseCircuitParams,
    ep_circuit_params: &BaseCircuitParams,
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
    mut witness: KagemushaRecursiveStateGenerationWitnessV1<'_>,
    recovery_seed: &KagemushaRecoverySeedV1,
) -> Result<KagemushaPrivateCarrierProofV1, KagemushaArtifactGenerationErrorV1> {
    witness.state.eq_protocol_digest = eq_protocol_digest;
    witness.state.ep_protocol_digest = ep_protocol_digest;
    witness.state.eq_deferred_audit = [1; 32];
    witness.state.ep_deferred_audit = [2; 32];
    let (_, _, eq_deferred_audit, ep_deferred_audit) =
        build_recursive_generation_pair_v1(eq_parameters, ep_parameters, witness.clone())
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    witness.state.eq_deferred_audit = eq_deferred_audit;
    witness.state.ep_deferred_audit = ep_deferred_audit;
    let eq_instances =
        recursive_public_instances::<Fp>(&witness.state, witness.eq_successor_history.as_bytes())?;
    let ep_instances =
        recursive_public_instances::<Fq>(&witness.state, witness.ep_successor_history.as_bytes())?;
    let eq_history = witness.eq_successor_history.clone();
    let ep_history = witness.ep_successor_history.clone();
    let (eq_circuit, ep_circuit, rebuilt_eq_audit, rebuilt_ep_audit) =
        build_recursive_generation_pair_v1(eq_parameters, ep_parameters, witness)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if rebuilt_eq_audit != eq_deferred_audit || rebuilt_ep_audit != ep_deferred_audit {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "private recursive-carrier audit changed while binding its public cells".to_owned(),
        ));
    }
    if !same_base_params(&eq_circuit.params(), eq_circuit_params) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
            KagemushaPastaParityV1::Eq,
        ));
    }
    if !same_base_params(&ep_circuit.params(), ep_circuit_params) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
            KagemushaPastaParityV1::Ep,
        ));
    }
    let eq_proof = create_eq_proof_with_key_v1(
        eq_parameters,
        eq_proving_key,
        eq_circuit,
        &eq_instances,
        KagemushaProofRecoveryPhaseV1::StateCarrier,
        recovery_seed,
    )?;
    let ep_proof = create_ep_proof_with_key_v1(
        ep_parameters,
        ep_proving_key,
        ep_circuit,
        &ep_instances,
        KagemushaProofRecoveryPhaseV1::StateCarrier,
        recovery_seed,
    )?;
    if eq_proof.is_empty() || ep_proof.is_empty() {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "private recursive carrier emitted an empty proof".to_owned(),
        ));
    }
    Ok(KagemushaPrivateCarrierProofV1 {
        eq_instances,
        ep_instances,
        eq_proof,
        ep_proof,
        eq_history,
        ep_history,
    })
}

#[cfg(feature = "zk-halo2-ipa")]
/// Generate the paired Pasta recursive-state proving and verification artifacts.
///
/// The required secret seed makes the real measurement proofs reproducible without an implicit
/// random fallback; it does not change the deterministic transparent parameter/key generation.
pub fn generate_kagemusha_recursive_state_artifacts_v1(
    witness: KagemushaRecursiveStateGenerationWitnessV1<'_>,
    recovery_seed: &KagemushaRecoverySeedV1,
) -> Result<KagemushaGeneratedRecursiveStateArtifactsV1, KagemushaArtifactGenerationErrorV1> {
    let eq_parameters = ParamsIPA::<EqAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let ep_parameters = ParamsIPA::<EpAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let (inner_eq_circuit, inner_ep_circuit, _, _) =
        build_recursive_generation_pair_v1(&eq_parameters, &ep_parameters, witness.clone())
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let inner_eq_circuit_params = inner_eq_circuit.params();
    let inner_ep_circuit_params = inner_ep_circuit.params();
    validate_recursive_profile(KagemushaPastaParityV1::Eq, &inner_eq_circuit_params)?;
    validate_recursive_profile(KagemushaPastaParityV1::Ep, &inner_ep_circuit_params)?;

    let inner_eq_vk = keygen_vk(&eq_parameters, &inner_eq_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Eq,
            kind: "private recursive-state carrier verifying key",
            reason: error.to_string(),
        }
    })?;
    let inner_eq_pk =
        keygen_pk(&eq_parameters, inner_eq_vk.clone(), &inner_eq_circuit).map_err(|error| {
            KagemushaArtifactGenerationErrorV1::KeyGeneration {
                parity: KagemushaPastaParityV1::Eq,
                kind: "private recursive-state carrier proving key",
                reason: error.to_string(),
            }
        })?;
    let inner_ep_vk = keygen_vk(&ep_parameters, &inner_ep_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Ep,
            kind: "private recursive-state carrier verifying key",
            reason: error.to_string(),
        }
    })?;
    let inner_ep_pk =
        keygen_pk(&ep_parameters, inner_ep_vk.clone(), &inner_ep_circuit).map_err(|error| {
            KagemushaArtifactGenerationErrorV1::KeyGeneration {
                parity: KagemushaPastaParityV1::Ep,
                kind: "private recursive-state carrier proving key",
                reason: error.to_string(),
            }
        })?;

    let inner_eq_protocol = compile(
        &eq_parameters,
        &inner_eq_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![recursive_public_instance_count()]),
    );
    let inner_ep_protocol = compile(
        &ep_parameters,
        &inner_ep_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![recursive_public_instance_count()]),
    );
    let inner_eq_protocol_digest =
        native_parent_protocol_digest_v1(&inner_eq_protocol, KagemushaPastaParityV1::Eq)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let inner_ep_protocol_digest =
        native_parent_protocol_digest_v1(&inner_ep_protocol, KagemushaPastaParityV1::Ep)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if inner_eq_protocol_digest == inner_ep_protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "Eq and Ep private recursive-carrier protocol identities alias".to_owned(),
        ));
    }

    // Prove one real wide carrier before keying the outer decider.  This both fixes the
    // decider's actual verifier workload and prevents a release from authenticating a compact
    // key generated against a dummy or native-preprocessed inner transcript.
    let private = prove_private_recursive_carrier_v1(
        &eq_parameters,
        &ep_parameters,
        &inner_eq_pk,
        &inner_ep_pk,
        &inner_eq_circuit_params,
        &inner_ep_circuit_params,
        inner_eq_protocol_digest,
        inner_ep_protocol_digest,
        witness.clone(),
        recovery_seed,
    )?;
    let private_eq_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(
            &eq_parameters,
            &inner_eq_protocol,
            &private.eq_proof,
            &private.eq_instances,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let private_ep_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(
            &ep_parameters,
            &inner_ep_protocol,
            &private.ep_proof,
            &private.ep_instances,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let eq_transport_fold = fold_kagemusha_eq_accumulators_v1(
        &eq_parameters,
        &private_eq_current,
        &private.eq_history,
        recovery_seed,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_transport_fold = fold_kagemusha_ep_accumulators_v1(
        &ep_parameters,
        &private_ep_current,
        &private.ep_history,
        recovery_seed,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let private_eq_history = private
        .eq_history
        .to_native()
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let private_ep_history = private
        .ep_history
        .to_native()
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;

    let mut outer_state = witness.state.clone();
    outer_state.eq_protocol_digest = inner_eq_protocol_digest;
    outer_state.ep_protocol_digest = inner_ep_protocol_digest;
    outer_state.eq_deferred_audit = [1; 32];
    outer_state.ep_deferred_audit = [2; 32];
    let mut outer_eq_instances =
        recursive_public_instances::<Fp>(&outer_state, eq_transport_fold.successor().as_bytes())?;
    let mut outer_ep_instances =
        recursive_public_instances::<Fq>(&outer_state, ep_transport_fold.successor().as_bytes())?;
    let (_, _, outer_eq_audit, outer_ep_audit) = build_kagemusha_transport_decider_pair_v1(
        &eq_parameters,
        &ep_parameters,
        KagemushaTransportDeciderWitnessV1 {
            eq: KagemushaTransportDeciderParityWitnessV1 {
                inner_protocol: &inner_eq_protocol,
                inner_instances: &private.eq_instances,
                inner_proof: &private.eq_proof,
                inner_history: &private_eq_history,
                inner_history_fold_proof: eq_transport_fold.proof().as_bytes(),
                outer_instances: &outer_eq_instances,
            },
            ep: KagemushaTransportDeciderParityWitnessV1 {
                inner_protocol: &inner_ep_protocol,
                inner_instances: &private.ep_instances,
                inner_proof: &private.ep_proof,
                inner_history: &private_ep_history,
                inner_history_fold_proof: ep_transport_fold.proof().as_bytes(),
                outer_instances: &outer_ep_instances,
            },
        },
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    outer_state.eq_deferred_audit = outer_eq_audit;
    outer_state.ep_deferred_audit = outer_ep_audit;
    outer_eq_instances =
        recursive_public_instances::<Fp>(&outer_state, eq_transport_fold.successor().as_bytes())?;
    outer_ep_instances =
        recursive_public_instances::<Fq>(&outer_state, ep_transport_fold.successor().as_bytes())?;
    let (eq_circuit, ep_circuit, rebuilt_eq_audit, rebuilt_ep_audit) =
        build_kagemusha_transport_decider_pair_v1(
            &eq_parameters,
            &ep_parameters,
            KagemushaTransportDeciderWitnessV1 {
                eq: KagemushaTransportDeciderParityWitnessV1 {
                    inner_protocol: &inner_eq_protocol,
                    inner_instances: &private.eq_instances,
                    inner_proof: &private.eq_proof,
                    inner_history: &private_eq_history,
                    inner_history_fold_proof: eq_transport_fold.proof().as_bytes(),
                    outer_instances: &outer_eq_instances,
                },
                ep: KagemushaTransportDeciderParityWitnessV1 {
                    inner_protocol: &inner_ep_protocol,
                    inner_instances: &private.ep_instances,
                    inner_proof: &private.ep_proof,
                    inner_history: &private_ep_history,
                    inner_history_fold_proof: ep_transport_fold.proof().as_bytes(),
                    outer_instances: &outer_ep_instances,
                },
            },
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if rebuilt_eq_audit != outer_eq_audit || rebuilt_ep_audit != outer_ep_audit {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "transport-decider audit changed while binding its public cells".to_owned(),
        ));
    }
    let eq_circuit_params = eq_circuit.params();
    let ep_circuit_params = ep_circuit.params();
    validate_recursive_profile(KagemushaPastaParityV1::Eq, &eq_circuit_params)?;
    validate_recursive_profile(KagemushaPastaParityV1::Ep, &ep_circuit_params)?;
    let eq_vk = keygen_vk(&eq_parameters, &eq_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Eq,
            kind: "compact transport-decider verifying key",
            reason: error.to_string(),
        }
    })?;
    let eq_pk = keygen_pk(&eq_parameters, eq_vk.clone(), &eq_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Eq,
            kind: "compact transport-decider proving key",
            reason: error.to_string(),
        }
    })?;
    let ep_vk = keygen_vk(&ep_parameters, &ep_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Ep,
            kind: "compact transport-decider verifying key",
            reason: error.to_string(),
        }
    })?;
    let ep_pk = keygen_pk(&ep_parameters, ep_vk.clone(), &ep_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Ep,
            kind: "compact transport-decider proving key",
            reason: error.to_string(),
        }
    })?;
    let eq_protocol = compile(
        &eq_parameters,
        &eq_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![recursive_public_instance_count()]),
    );
    let ep_protocol = compile(
        &ep_parameters,
        &ep_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![recursive_public_instance_count()]),
    );
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Eq,
        "compact aggregate-state transport decider",
        &eq_protocol,
    )?;
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Ep,
        "compact aggregate-state transport decider",
        &ep_protocol,
    )?;
    let eq_protocol_digest =
        native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let ep_protocol_digest =
        native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if eq_protocol_digest == ep_protocol_digest
        || eq_protocol_digest == inner_eq_protocol_digest
        || ep_protocol_digest == inner_ep_protocol_digest
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "inner and outer recursive protocol roles alias".to_owned(),
        ));
    }

    // Hard release gate: rebuild with the final outer protocol identities, generate one actual
    // proof for each parity, verify it natively, and terminally decide its IPA accumulator.
    outer_state.eq_protocol_digest = eq_protocol_digest;
    outer_state.ep_protocol_digest = ep_protocol_digest;
    outer_state.eq_deferred_audit = [1; 32];
    outer_state.ep_deferred_audit = [2; 32];
    outer_eq_instances =
        recursive_public_instances::<Fp>(&outer_state, eq_transport_fold.successor().as_bytes())?;
    outer_ep_instances =
        recursive_public_instances::<Fq>(&outer_state, ep_transport_fold.successor().as_bytes())?;
    let (_, _, measured_eq_audit, measured_ep_audit) = build_kagemusha_transport_decider_pair_v1(
        &eq_parameters,
        &ep_parameters,
        KagemushaTransportDeciderWitnessV1 {
            eq: KagemushaTransportDeciderParityWitnessV1 {
                inner_protocol: &inner_eq_protocol,
                inner_instances: &private.eq_instances,
                inner_proof: &private.eq_proof,
                inner_history: &private_eq_history,
                inner_history_fold_proof: eq_transport_fold.proof().as_bytes(),
                outer_instances: &outer_eq_instances,
            },
            ep: KagemushaTransportDeciderParityWitnessV1 {
                inner_protocol: &inner_ep_protocol,
                inner_instances: &private.ep_instances,
                inner_proof: &private.ep_proof,
                inner_history: &private_ep_history,
                inner_history_fold_proof: ep_transport_fold.proof().as_bytes(),
                outer_instances: &outer_ep_instances,
            },
        },
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    outer_state.eq_deferred_audit = measured_eq_audit;
    outer_state.ep_deferred_audit = measured_ep_audit;
    outer_eq_instances =
        recursive_public_instances::<Fp>(&outer_state, eq_transport_fold.successor().as_bytes())?;
    outer_ep_instances =
        recursive_public_instances::<Fq>(&outer_state, ep_transport_fold.successor().as_bytes())?;
    let (measured_eq_circuit, measured_ep_circuit, rebuilt_eq_audit, rebuilt_ep_audit) =
        build_kagemusha_transport_decider_pair_v1(
            &eq_parameters,
            &ep_parameters,
            KagemushaTransportDeciderWitnessV1 {
                eq: KagemushaTransportDeciderParityWitnessV1 {
                    inner_protocol: &inner_eq_protocol,
                    inner_instances: &private.eq_instances,
                    inner_proof: &private.eq_proof,
                    inner_history: &private_eq_history,
                    inner_history_fold_proof: eq_transport_fold.proof().as_bytes(),
                    outer_instances: &outer_eq_instances,
                },
                ep: KagemushaTransportDeciderParityWitnessV1 {
                    inner_protocol: &inner_ep_protocol,
                    inner_instances: &private.ep_instances,
                    inner_proof: &private.ep_proof,
                    inner_history: &private_ep_history,
                    inner_history_fold_proof: ep_transport_fold.proof().as_bytes(),
                    outer_instances: &outer_ep_instances,
                },
            },
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if rebuilt_eq_audit != measured_eq_audit
        || rebuilt_ep_audit != measured_ep_audit
        || !same_base_params(&measured_eq_circuit.params(), &eq_circuit_params)
        || !same_base_params(&measured_ep_circuit.params(), &ep_circuit_params)
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "final transport-decider witness changed its authenticated circuit profile".to_owned(),
        ));
    }
    let eq_transport_capacity = measured_eq_circuit
        .capacity_profile()
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let ep_transport_capacity = measured_ep_circuit
        .capacity_profile()
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let measured_eq_proof = create_eq_proof_with_key_v1(
        &eq_parameters,
        &eq_pk,
        measured_eq_circuit,
        &outer_eq_instances,
        KagemushaProofRecoveryPhaseV1::StateTransport,
        recovery_seed,
    )?;
    let measured_ep_proof = create_ep_proof_with_key_v1(
        &ep_parameters,
        &ep_pk,
        measured_ep_circuit,
        &outer_ep_instances,
        KagemushaProofRecoveryPhaseV1::StateTransport,
        recovery_seed,
    )?;
    validate_recursive_proof_length(KagemushaPastaParityV1::Eq, &measured_eq_proof)?;
    validate_recursive_proof_length(KagemushaPastaParityV1::Ep, &measured_ep_proof)?;
    let measured_eq_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(
            &eq_parameters,
            &eq_protocol,
            &measured_eq_proof,
            &outer_eq_instances,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let measured_ep_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(
            &ep_parameters,
            &ep_protocol,
            &measured_ep_proof,
            &outer_ep_instances,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    decide_kagemusha_eq_accumulator_v1(&eq_parameters, &measured_eq_current)
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    decide_kagemusha_ep_accumulator_v1(&ep_parameters, &measured_ep_current)
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    decide_kagemusha_eq_accumulator_v1(&eq_parameters, eq_transport_fold.successor())
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    decide_kagemusha_ep_accumulator_v1(&ep_parameters, ep_transport_fold.successor())
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;

    Ok(KagemushaGeneratedRecursiveStateArtifactsV1 {
        eq: build_generated(KagemushaPastaParityV1::Eq, &eq_parameters, &eq_pk, &eq_vk)?,
        ep: build_generated(KagemushaPastaParityV1::Ep, &ep_parameters, &ep_pk, &ep_vk)?,
        inner_eq: build_generated(
            KagemushaPastaParityV1::Eq,
            &eq_parameters,
            &inner_eq_pk,
            &inner_eq_vk,
        )?,
        inner_ep: build_generated(
            KagemushaPastaParityV1::Ep,
            &ep_parameters,
            &inner_ep_pk,
            &inner_ep_vk,
        )?,
        eq_circuit_params,
        ep_circuit_params,
        inner_eq_circuit_params,
        inner_ep_circuit_params,
        eq_protocol_digest,
        ep_protocol_digest,
        inner_eq_protocol_digest,
        inner_ep_protocol_digest,
        eq_transport_capacity,
        ep_transport_capacity,
    })
}

/// Load and cross-check the authenticated Eq recursive state-role artifacts.
///
/// # Errors
///
/// Returns an error for invalid circuit parameters, content-address failures, malformed keys,
/// trailing bytes, or a proving key whose embedded verifier differs from the standalone key.
#[cfg(feature = "zk-halo2-ipa")]
pub fn load_kagemusha_eq_recursive_state_artifacts_v1<R>(
    artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
    circuit_params: BaseCircuitParams,
    inner_circuit_params: BaseCircuitParams,
) -> Result<KagemushaLoadedEqRecursiveStateArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    R: KagemushaArtifactByteResolverV1,
{
    validate_recursive_profile(KagemushaPastaParityV1::Eq, &circuit_params)?;
    validate_recursive_profile(KagemushaPastaParityV1::Eq, &inner_circuit_params)?;
    let parameters = artifacts.load_eq_params()?;
    let verifying_bytes = artifacts.resolve(KagemushaArtifactRoleV1::StateVkEq)?;
    let verifying_key =
        read_eq_transport_decider_vk(verifying_bytes.as_ref(), circuit_params.clone())?;
    let proving_key =
        load_authenticated_proving_key_v1::<EqAffine, KagemushaTransportDeciderEqCircuitV1, _>(
            artifacts,
            KagemushaArtifactRoleV1::StatePkEq,
            KagemushaPastaParityV1::Eq,
            circuit_params.clone(),
        )?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Eq,
        &proving_key,
        verifying_bytes.as_ref(),
    )?;
    let inner_verifying_bytes = artifacts.resolve(KagemushaArtifactRoleV1::InnerStateVkEq)?;
    let inner_verifying_key =
        read_eq_recursive_vk(inner_verifying_bytes.as_ref(), inner_circuit_params.clone())?;
    let inner_proving_key =
        load_authenticated_proving_key_v1::<EqAffine, KagemushaRecursiveStateEqCircuitV1, _>(
            artifacts,
            KagemushaArtifactRoleV1::InnerStatePkEq,
            KagemushaPastaParityV1::Eq,
            inner_circuit_params.clone(),
        )?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Eq,
        &inner_proving_key,
        inner_verifying_bytes.as_ref(),
    )?;
    let protocol = compile(
        &parameters,
        &verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![recursive_public_instance_count()]),
    );
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Eq,
        "compact aggregate-state transport decider",
        &protocol,
    )?;
    let inner_protocol = compile(
        &parameters,
        &inner_verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![recursive_public_instance_count()]),
    );
    let outer_digest = native_parent_protocol_digest_v1(&protocol, KagemushaPastaParityV1::Eq)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let inner_digest =
        native_parent_protocol_digest_v1(&inner_protocol, KagemushaPastaParityV1::Eq)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let recursion = artifacts.recursion_artifacts();
    if outer_digest != recursion.eq_protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "Eq state protocol does not match the authenticated release".to_owned(),
        ));
    }
    if outer_digest == inner_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "Eq inner and outer state protocol identities alias".to_owned(),
        ));
    }
    Ok(KagemushaLoadedEqRecursiveStateArtifactsV1 {
        release_id: recursion.release_id,
        suite_id: artifacts.suite_id(),
        vk_digest: artifacts.vk_set_digest(),
        parameters,
        proving_key,
        verifying_key,
        circuit_params,
        inner_proving_key,
        inner_verifying_key,
        inner_circuit_params,
    })
}

/// Load and cross-check the authenticated Ep recursive state-role artifacts.
///
/// # Errors
///
/// Returns an error for invalid circuit parameters, content-address failures, malformed keys,
/// trailing bytes, or a proving key whose embedded verifier differs from the standalone key.
#[cfg(feature = "zk-halo2-ipa")]
pub fn load_kagemusha_ep_recursive_state_artifacts_v1<R>(
    artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
    circuit_params: BaseCircuitParams,
    inner_circuit_params: BaseCircuitParams,
) -> Result<KagemushaLoadedEpRecursiveStateArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    R: KagemushaArtifactByteResolverV1,
{
    validate_recursive_profile(KagemushaPastaParityV1::Ep, &circuit_params)?;
    validate_recursive_profile(KagemushaPastaParityV1::Ep, &inner_circuit_params)?;
    let parameters = artifacts.load_ep_params()?;
    let verifying_bytes = artifacts.resolve(KagemushaArtifactRoleV1::StateVkEp)?;
    let verifying_key =
        read_ep_transport_decider_vk(verifying_bytes.as_ref(), circuit_params.clone())?;
    let proving_key =
        load_authenticated_proving_key_v1::<EpAffine, KagemushaTransportDeciderEpCircuitV1, _>(
            artifacts,
            KagemushaArtifactRoleV1::StatePkEp,
            KagemushaPastaParityV1::Ep,
            circuit_params.clone(),
        )?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Ep,
        &proving_key,
        verifying_bytes.as_ref(),
    )?;
    let inner_verifying_bytes = artifacts.resolve(KagemushaArtifactRoleV1::InnerStateVkEp)?;
    let inner_verifying_key =
        read_ep_recursive_vk(inner_verifying_bytes.as_ref(), inner_circuit_params.clone())?;
    let inner_proving_key =
        load_authenticated_proving_key_v1::<EpAffine, KagemushaRecursiveStateEpCircuitV1, _>(
            artifacts,
            KagemushaArtifactRoleV1::InnerStatePkEp,
            KagemushaPastaParityV1::Ep,
            inner_circuit_params.clone(),
        )?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Ep,
        &inner_proving_key,
        inner_verifying_bytes.as_ref(),
    )?;
    let protocol = compile(
        &parameters,
        &verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![recursive_public_instance_count()]),
    );
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Ep,
        "compact aggregate-state transport decider",
        &protocol,
    )?;
    let inner_protocol = compile(
        &parameters,
        &inner_verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![recursive_public_instance_count()]),
    );
    let outer_digest = native_parent_protocol_digest_v1(&protocol, KagemushaPastaParityV1::Ep)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let inner_digest =
        native_parent_protocol_digest_v1(&inner_protocol, KagemushaPastaParityV1::Ep)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let recursion = artifacts.recursion_artifacts();
    if outer_digest != recursion.ep_protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "Ep state protocol does not match the authenticated release".to_owned(),
        ));
    }
    if outer_digest == inner_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "Ep inner and outer state protocol identities alias".to_owned(),
        ));
    }
    Ok(KagemushaLoadedEpRecursiveStateArtifactsV1 {
        release_id: recursion.release_id,
        suite_id: artifacts.suite_id(),
        vk_digest: artifacts.vk_set_digest(),
        parameters,
        proving_key,
        verifying_key,
        circuit_params,
        inner_proving_key,
        inner_verifying_key,
        inner_circuit_params,
    })
}

/// Produce both production recursive state proofs and their carried delayed histories.
///
/// The circuit is rebuilt from the complete private witness and its derived layout must exactly
/// match each authenticated proving key profile before Halo2 is invoked.
/// The hardware-unsealed operation seed deterministically reconstructs both private carrier and
/// transport proofs and their BGH19 folds. All input witnesses must remain byte-identical.
///
/// # Errors
///
/// Returns an error for invalid witness material, profile substitution, proof generation failure,
/// or a transcript that exceeds the fixed transport budget.
#[cfg(feature = "zk-halo2-ipa")]
pub fn prove_kagemusha_recursive_state_v1(
    eq: &KagemushaLoadedEqRecursiveStateArtifactsV1,
    ep: &KagemushaLoadedEpRecursiveStateArtifactsV1,
    witness: KagemushaRecursiveStateGenerationWitnessV1<'_>,
    recovery_seed: &KagemushaRecoverySeedV1,
) -> Result<KagemushaGeneratedRecursiveStateProofV1, KagemushaArtifactGenerationErrorV1> {
    if eq.release_id != ep.release_id
        || eq.suite_id != ep.suite_id
        || eq.vk_digest != ep.vk_digest
        || witness.state.successor.release_id != eq.release_id
        || witness.state.successor.suite_id != eq.suite_id
        || witness.state.successor.vk_digest != eq.vk_digest
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "recursive-state successor does not match the authenticated proving release".to_owned(),
        ));
    }
    let semantic_digest = witness.state.transport_semantic_digest;
    let expected_eq_protocol_digest = witness.state.eq_protocol_digest;
    let expected_ep_protocol_digest = witness.state.ep_protocol_digest;
    let guard_eq_credential_audit = witness.state.guard_eq_credential_audit;
    let guard_ep_credential_audit = witness.state.guard_ep_credential_audit;
    let mut outer_state = witness.state.clone();
    let inner_eq_protocol = compile(
        &eq.parameters,
        &eq.inner_verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![recursive_public_instance_count()]),
    );
    let inner_ep_protocol = compile(
        &ep.parameters,
        &ep.inner_verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![recursive_public_instance_count()]),
    );
    let inner_eq_protocol_digest =
        native_parent_protocol_digest_v1(&inner_eq_protocol, KagemushaPastaParityV1::Eq)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let inner_ep_protocol_digest =
        native_parent_protocol_digest_v1(&inner_ep_protocol, KagemushaPastaParityV1::Ep)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let eq_protocol = compile(
        &eq.parameters,
        &eq.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![recursive_public_instance_count()]),
    );
    let ep_protocol = compile(
        &ep.parameters,
        &ep.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![recursive_public_instance_count()]),
    );
    let actual_eq_protocol_digest =
        native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let actual_ep_protocol_digest =
        native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if actual_eq_protocol_digest != expected_eq_protocol_digest
        || actual_ep_protocol_digest != expected_ep_protocol_digest
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "recursive-state witness protocol identity does not match the loaded proving key"
                .to_owned(),
        ));
    }
    if inner_eq_protocol_digest == actual_eq_protocol_digest
        || inner_ep_protocol_digest == actual_ep_protocol_digest
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "inner and outer recursive-state protocol identities alias".to_owned(),
        ));
    }

    let private = prove_private_recursive_carrier_v1(
        &eq.parameters,
        &ep.parameters,
        &eq.inner_proving_key,
        &ep.inner_proving_key,
        &eq.inner_circuit_params,
        &ep.inner_circuit_params,
        inner_eq_protocol_digest,
        inner_ep_protocol_digest,
        witness,
        recovery_seed,
    )?;
    let eq_current_accumulator = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(
            &eq.parameters,
            &inner_eq_protocol,
            &private.eq_proof,
            &private.eq_instances,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_current_accumulator = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(
            &ep.parameters,
            &inner_ep_protocol,
            &private.ep_proof,
            &private.ep_instances,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let eq_transport_fold = fold_kagemusha_eq_accumulators_v1(
        &eq.parameters,
        &eq_current_accumulator,
        &private.eq_history,
        recovery_seed,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_transport_fold = fold_kagemusha_ep_accumulators_v1(
        &ep.parameters,
        &ep_current_accumulator,
        &private.ep_history,
        recovery_seed,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let private_eq_history = private
        .eq_history
        .to_native()
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let private_ep_history = private
        .ep_history
        .to_native()
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;

    outer_state.eq_protocol_digest = actual_eq_protocol_digest;
    outer_state.ep_protocol_digest = actual_ep_protocol_digest;
    outer_state.eq_deferred_audit = [1; 32];
    outer_state.ep_deferred_audit = [2; 32];
    let mut outer_eq_instances =
        recursive_public_instances::<Fp>(&outer_state, eq_transport_fold.successor().as_bytes())?;
    let mut outer_ep_instances =
        recursive_public_instances::<Fq>(&outer_state, ep_transport_fold.successor().as_bytes())?;
    let (_, _, eq_deferred_audit, ep_deferred_audit) = build_kagemusha_transport_decider_pair_v1(
        &eq.parameters,
        &ep.parameters,
        KagemushaTransportDeciderWitnessV1 {
            eq: KagemushaTransportDeciderParityWitnessV1 {
                inner_protocol: &inner_eq_protocol,
                inner_instances: &private.eq_instances,
                inner_proof: &private.eq_proof,
                inner_history: &private_eq_history,
                inner_history_fold_proof: eq_transport_fold.proof().as_bytes(),
                outer_instances: &outer_eq_instances,
            },
            ep: KagemushaTransportDeciderParityWitnessV1 {
                inner_protocol: &inner_ep_protocol,
                inner_instances: &private.ep_instances,
                inner_proof: &private.ep_proof,
                inner_history: &private_ep_history,
                inner_history_fold_proof: ep_transport_fold.proof().as_bytes(),
                outer_instances: &outer_ep_instances,
            },
        },
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    outer_state.eq_deferred_audit = eq_deferred_audit;
    outer_state.ep_deferred_audit = ep_deferred_audit;
    outer_eq_instances =
        recursive_public_instances::<Fp>(&outer_state, eq_transport_fold.successor().as_bytes())?;
    outer_ep_instances =
        recursive_public_instances::<Fq>(&outer_state, ep_transport_fold.successor().as_bytes())?;
    let (eq_circuit, ep_circuit, rebuilt_eq_audit, rebuilt_ep_audit) =
        build_kagemusha_transport_decider_pair_v1(
            &eq.parameters,
            &ep.parameters,
            KagemushaTransportDeciderWitnessV1 {
                eq: KagemushaTransportDeciderParityWitnessV1 {
                    inner_protocol: &inner_eq_protocol,
                    inner_instances: &private.eq_instances,
                    inner_proof: &private.eq_proof,
                    inner_history: &private_eq_history,
                    inner_history_fold_proof: eq_transport_fold.proof().as_bytes(),
                    outer_instances: &outer_eq_instances,
                },
                ep: KagemushaTransportDeciderParityWitnessV1 {
                    inner_protocol: &inner_ep_protocol,
                    inner_instances: &private.ep_instances,
                    inner_proof: &private.ep_proof,
                    inner_history: &private_ep_history,
                    inner_history_fold_proof: ep_transport_fold.proof().as_bytes(),
                    outer_instances: &outer_ep_instances,
                },
            },
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if rebuilt_eq_audit != eq_deferred_audit || rebuilt_ep_audit != ep_deferred_audit {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "transport-decider audit changed while binding its public cells".to_owned(),
        ));
    }
    if !same_base_params(&eq_circuit.params(), &eq.circuit_params) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
            KagemushaPastaParityV1::Eq,
        ));
    }
    if !same_base_params(&ep_circuit.params(), &ep.circuit_params) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
            KagemushaPastaParityV1::Ep,
        ));
    }
    let eq_proof = create_eq_proof_with_key_v1(
        &eq.parameters,
        &eq.proving_key,
        eq_circuit,
        &outer_eq_instances,
        KagemushaProofRecoveryPhaseV1::StateTransport,
        recovery_seed,
    )?;
    let ep_proof = create_ep_proof_with_key_v1(
        &ep.parameters,
        &ep.proving_key,
        ep_circuit,
        &outer_ep_instances,
        KagemushaProofRecoveryPhaseV1::StateTransport,
        recovery_seed,
    )?;
    validate_recursive_proof_length(KagemushaPastaParityV1::Eq, &eq_proof)?;
    validate_recursive_proof_length(KagemushaPastaParityV1::Ep, &ep_proof)?;
    let outer_eq_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(&eq.parameters, &eq_protocol, &eq_proof, &outer_eq_instances)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let outer_ep_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(&ep.parameters, &ep_protocol, &ep_proof, &outer_ep_instances)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    decide_kagemusha_eq_accumulator_v1(&eq.parameters, &outer_eq_current)
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    decide_kagemusha_ep_accumulator_v1(&ep.parameters, &outer_ep_current)
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    decide_kagemusha_eq_accumulator_v1(&eq.parameters, eq_transport_fold.successor())
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    decide_kagemusha_ep_accumulator_v1(&ep.parameters, ep_transport_fold.successor())
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let proof = KagemushaPairedProofV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        eq_protocol_digest: actual_eq_protocol_digest,
        ep_protocol_digest: actual_ep_protocol_digest,
        semantic_digest,
        guard_eq_credential_audit,
        guard_ep_credential_audit,
        eq_deferred_audit,
        ep_deferred_audit,
        eq_proof,
        ep_proof,
        eq_history: eq_transport_fold.successor().as_bytes().to_vec(),
        ep_history: ep_transport_fold.successor().as_bytes().to_vec(),
    };
    proof
        .validate_shape_for_semantic_digest(semantic_digest)
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    Ok(KagemushaGeneratedRecursiveStateProofV1 {
        eq_public_instances: private.eq_instances,
        ep_public_instances: private.ep_instances,
        eq_transport_public_instances: outer_eq_instances,
        ep_transport_public_instances: outer_ep_instances,
        eq_inner_proof: private.eq_proof,
        ep_inner_proof: private.ep_proof,
        proof,
        eq_current_accumulator,
        ep_current_accumulator,
        eq_history: private.eq_history,
        ep_history: private.ep_history,
    })
}

#[cfg(feature = "zk-halo2-ipa")]
#[allow(clippy::too_many_arguments)]
fn build_terminal_authorization_generation_pair_v1(
    eq_parameters: &ParamsIPA<EqAffine>,
    ep_parameters: &ParamsIPA<EpAffine>,
    witness: KagemushaTerminalAuthorizationGenerationWitnessV1<'_>,
    eq_deferred_audit: [u8; 32],
    ep_deferred_audit: [u8; 32],
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
) -> Result<
    (
        KagemushaTerminalAuthorizationEqCircuitV1,
        KagemushaTerminalAuthorizationEpCircuitV1,
        [u8; 32],
        [u8; 32],
    ),
    String,
> {
    let public = witness.public.into_internal(
        eq_deferred_audit,
        ep_deferred_audit,
        eq_protocol_digest,
        ep_protocol_digest,
    )?;
    build_kagemusha_terminal_authorization_pair_v1(
        eq_parameters,
        ep_parameters,
        KagemushaTerminalAuthorizationWitnessV1 {
            public,
            private_transition: witness.private_transition.into_internal(),
            terminal_guard_relation: witness.terminal_guard_relation,
            enabled_hardware_profiles: witness.enabled_hardware_profiles,
            eq: KagemushaTerminalAuthorizationEqWitnessV1 {
                candidate_protocol: witness.eq.candidate_protocol,
                candidate_instances: witness.eq.candidate_instances,
                candidate_proof: witness.eq.candidate_proof,
                candidate_history: witness.eq.candidate_history,
                candidate_history_fold_proof: witness.eq.candidate_history_fold_proof,
                terminal_guard_protocol: witness.eq.terminal_guard_protocol,
                terminal_guard_instances: witness.eq.terminal_guard_instances,
                terminal_guard_proof: witness.eq.terminal_guard_proof,
                terminal_guard_history: witness.eq.terminal_guard_history,
                terminal_guard_history_fold_proof: witness.eq.terminal_guard_history_fold_proof,
                merge_fold_proof: witness.eq.merge_fold_proof,
                successor_history: witness.eq.successor_history,
            },
            ep: KagemushaTerminalAuthorizationEpWitnessV1 {
                candidate_protocol: witness.ep.candidate_protocol,
                candidate_instances: witness.ep.candidate_instances,
                candidate_proof: witness.ep.candidate_proof,
                candidate_history: witness.ep.candidate_history,
                candidate_history_fold_proof: witness.ep.candidate_history_fold_proof,
                terminal_guard_protocol: witness.ep.terminal_guard_protocol,
                terminal_guard_instances: witness.ep.terminal_guard_instances,
                terminal_guard_proof: witness.ep.terminal_guard_proof,
                terminal_guard_history: witness.ep.terminal_guard_history,
                terminal_guard_history_fold_proof: witness.ep.terminal_guard_history_fold_proof,
                merge_fold_proof: witness.ep.merge_fold_proof,
                successor_history: witness.ep.successor_history,
            },
        },
    )
}

#[cfg(feature = "zk-halo2-ipa")]
#[allow(clippy::too_many_arguments)]
fn build_commit_wrapper_generation_pair_v1(
    eq_parameters: &ParamsIPA<EqAffine>,
    ep_parameters: &ParamsIPA<EpAffine>,
    witness: KagemushaCommitWrapperGenerationWitnessV1<'_>,
    eq_deferred_audit: [u8; 32],
    ep_deferred_audit: [u8; 32],
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
) -> Result<
    (
        KagemushaCommitWrapperEqCircuitV1,
        KagemushaCommitWrapperEpCircuitV1,
        [u8; 32],
        [u8; 32],
    ),
    String,
> {
    let public = witness.public.into_internal(
        eq_deferred_audit,
        ep_deferred_audit,
        eq_protocol_digest,
        ep_protocol_digest,
    )?;
    build_kagemusha_commit_wrapper_pair_v1(
        eq_parameters,
        ep_parameters,
        KagemushaCommitWrapperWitnessV1 {
            public,
            eq: KagemushaCommitWrapperEqWitnessV1 {
                terminal_authorization_protocol: witness.eq.terminal_authorization_protocol,
                terminal_authorization_instances: witness.eq.terminal_authorization_instances,
                terminal_authorization_proof: witness.eq.terminal_authorization_proof,
                terminal_authorization_history: witness.eq.terminal_authorization_history,
                terminal_authorization_history_fold_proof: witness
                    .eq
                    .terminal_authorization_history_fold_proof,
                successor_history: witness.eq.successor_history,
            },
            ep: KagemushaCommitWrapperEpWitnessV1 {
                terminal_authorization_protocol: witness.ep.terminal_authorization_protocol,
                terminal_authorization_instances: witness.ep.terminal_authorization_instances,
                terminal_authorization_proof: witness.ep.terminal_authorization_proof,
                terminal_authorization_history: witness.ep.terminal_authorization_history,
                terminal_authorization_history_fold_proof: witness
                    .ep
                    .terminal_authorization_history_fold_proof,
                successor_history: witness.ep.successor_history,
            },
        },
    )
}

/// Generate all four distinct terminal `TerminalAuthorization` keys from one fixed-shape witness.
///
/// Transparent Eq/Ep parameters are returned for content-addressed installation but are not
/// additional terminal-authorization roles; the release authenticates four dedicated PK/VK roles.
///
/// # Errors
///
/// Returns an error for an invalid witness, profile mismatch, key-generation failure,
/// noncanonical artifact size, or aliased parity protocol identity.
#[cfg(feature = "zk-halo2-ipa")]
pub fn generate_kagemusha_terminal_authorization_artifacts_v1(
    witness: KagemushaTerminalAuthorizationGenerationWitnessV1<'_>,
) -> Result<KagemushaGeneratedTerminalAuthorizationArtifactsV1, KagemushaArtifactGenerationErrorV1>
{
    let enabled_hardware_profiles = witness.enabled_hardware_profiles;
    let eq_parameters = ParamsIPA::<EqAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let ep_parameters = ParamsIPA::<EpAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let placeholder_eq_protocol = encode(Fp::from(1));
    let placeholder_ep_protocol = encode(Fq::from(2));
    let placeholder_eq_audit = encode(Fp::from(3));
    let placeholder_ep_audit = encode(Fq::from(4));
    let (eq_circuit, ep_circuit, _, _) = build_terminal_authorization_generation_pair_v1(
        &eq_parameters,
        &ep_parameters,
        witness,
        placeholder_eq_audit,
        placeholder_ep_audit,
        placeholder_eq_protocol,
        placeholder_ep_protocol,
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let eq_circuit_params = eq_circuit.params();
    let ep_circuit_params = ep_circuit.params();
    validate_terminal_authorization_profile(KagemushaPastaParityV1::Eq, &eq_circuit_params)?;
    validate_terminal_authorization_profile(KagemushaPastaParityV1::Ep, &ep_circuit_params)?;
    let eq_vk = keygen_vk(&eq_parameters, &eq_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Eq,
            kind: "terminal-authorization verifying key",
            reason: error.to_string(),
        }
    })?;
    let eq_pk = keygen_pk(&eq_parameters, eq_vk.clone(), &eq_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Eq,
            kind: "terminal-authorization proving key",
            reason: error.to_string(),
        }
    })?;
    let ep_vk = keygen_vk(&ep_parameters, &ep_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Ep,
            kind: "terminal-authorization verifying key",
            reason: error.to_string(),
        }
    })?;
    let ep_pk = keygen_pk(&ep_parameters, ep_vk.clone(), &ep_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Ep,
            kind: "terminal-authorization proving key",
            reason: error.to_string(),
        }
    })?;
    let eq_protocol = compile(
        &eq_parameters,
        &eq_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_protocol = compile(
        &ep_parameters,
        &ep_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let eq_protocol_digest =
        native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let ep_protocol_digest =
        native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if eq_protocol_digest == ep_protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "Eq and Ep terminal-authorization protocol identities alias".to_owned(),
        ));
    }
    let (eq_parameters, eq_proving_key, eq_verifying_key) = build_generated_helper_parity(
        KagemushaPastaParityV1::Eq,
        "terminal-authorization proving key",
        &eq_parameters,
        &eq_pk,
        &eq_vk,
    )?;
    let (ep_parameters, ep_proving_key, ep_verifying_key) = build_generated_helper_parity(
        KagemushaPastaParityV1::Ep,
        "terminal-authorization proving key",
        &ep_parameters,
        &ep_pk,
        &ep_vk,
    )?;
    Ok(KagemushaGeneratedTerminalAuthorizationArtifactsV1 {
        eq_parameters,
        ep_parameters,
        eq_proving_key,
        eq_verifying_key,
        ep_proving_key,
        ep_verifying_key,
        eq_circuit_params,
        ep_circuit_params,
        eq_protocol_digest,
        ep_protocol_digest,
        enabled_hardware_profiles,
    })
}

/// Generate all four dedicated CommitWrapper key roles.
///
/// The relation shares only transparent Pasta parameters and recursive candidate/Guard helper
/// logic with TerminalAuthorization. Its distinct circuit type and fixed relation-domain constraint make
/// its PKs, VKs, and compiled protocols non-interchangeable with every state or terminal-authorization role.
///
/// # Errors
///
/// Returns an error for an invalid witness, profile mismatch, key-generation failure,
/// noncanonical artifact size, or aliased parity protocol identity.
#[cfg(feature = "zk-halo2-ipa")]
pub fn generate_kagemusha_commit_wrapper_artifacts_v1(
    witness: KagemushaCommitWrapperGenerationWitnessV1<'_>,
) -> Result<KagemushaGeneratedCommitWrapperArtifactsV1, KagemushaArtifactGenerationErrorV1> {
    let enabled_hardware_profiles = witness.enabled_hardware_profiles;
    let eq_parameters = ParamsIPA::<EqAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let ep_parameters = ParamsIPA::<EpAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let terminal_authorization_eq_protocol_digest = native_parent_protocol_digest_v1(
        witness.eq.terminal_authorization_protocol,
        KagemushaPastaParityV1::Eq,
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let terminal_authorization_ep_protocol_digest = native_parent_protocol_digest_v1(
        witness.ep.terminal_authorization_protocol,
        KagemushaPastaParityV1::Ep,
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if terminal_authorization_eq_protocol_digest == terminal_authorization_ep_protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "Eq and Ep nested terminal-authorization protocols alias".to_owned(),
        ));
    }
    // The transported authorization circuit exposes its own protocol identities, which cannot be
    // known until after key generation. Those cells are public advice and do not affect the VK;
    // key generation therefore uses nonzero parity-distinct placeholders. The nested
    // TerminalAuthorization protocol is not a placeholder: it is loaded as circuit constants and
    // its exact digest is retained above for release binding.
    let layout_eq_protocol = encode(Fp::from(1));
    let layout_ep_protocol = encode(Fq::from(2));
    let layout_eq_audit = encode(Fp::from(3));
    let layout_ep_audit = encode(Fq::from(4));
    let (eq_circuit, ep_circuit, _, _) = build_commit_wrapper_generation_pair_v1(
        &eq_parameters,
        &ep_parameters,
        witness,
        layout_eq_audit,
        layout_ep_audit,
        layout_eq_protocol,
        layout_ep_protocol,
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let eq_circuit_params = eq_circuit.params();
    let ep_circuit_params = ep_circuit.params();
    validate_terminal_authorization_profile(KagemushaPastaParityV1::Eq, &eq_circuit_params)?;
    validate_terminal_authorization_profile(KagemushaPastaParityV1::Ep, &ep_circuit_params)?;
    let eq_vk = keygen_vk(&eq_parameters, &eq_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Eq,
            kind: "commit wrapper verifying key",
            reason: error.to_string(),
        }
    })?;
    let eq_pk = keygen_pk(&eq_parameters, eq_vk.clone(), &eq_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Eq,
            kind: "commit wrapper proving key",
            reason: error.to_string(),
        }
    })?;
    let ep_vk = keygen_vk(&ep_parameters, &ep_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Ep,
            kind: "commit wrapper verifying key",
            reason: error.to_string(),
        }
    })?;
    let ep_pk = keygen_pk(&ep_parameters, ep_vk.clone(), &ep_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Ep,
            kind: "commit wrapper proving key",
            reason: error.to_string(),
        }
    })?;
    let eq_protocol = compile(
        &eq_parameters,
        &eq_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_protocol = compile(
        &ep_parameters,
        &ep_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Eq,
        "commit wrapper",
        &eq_protocol,
    )?;
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Ep,
        "commit wrapper",
        &ep_protocol,
    )?;
    let eq_protocol_digest =
        native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let ep_protocol_digest =
        native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if eq_protocol_digest == ep_protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "Eq and Ep commit wrapper protocol identities alias".to_owned(),
        ));
    }
    let (eq_parameters, eq_proving_key, eq_verifying_key) = build_generated_helper_parity(
        KagemushaPastaParityV1::Eq,
        "commit wrapper proving key",
        &eq_parameters,
        &eq_pk,
        &eq_vk,
    )?;
    let (ep_parameters, ep_proving_key, ep_verifying_key) = build_generated_helper_parity(
        KagemushaPastaParityV1::Ep,
        "commit wrapper proving key",
        &ep_parameters,
        &ep_pk,
        &ep_vk,
    )?;
    Ok(KagemushaGeneratedCommitWrapperArtifactsV1 {
        eq_parameters,
        ep_parameters,
        eq_proving_key,
        eq_verifying_key,
        ep_proving_key,
        ep_verifying_key,
        eq_circuit_params,
        ep_circuit_params,
        eq_protocol_digest,
        ep_protocol_digest,
        terminal_authorization_eq_protocol_digest,
        terminal_authorization_ep_protocol_digest,
        enabled_hardware_profiles,
    })
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_terminal_authorization_release_alignment_v1(
    artifacts: super::KagemushaRecursionArtifactsV1,
    release: &KagemushaAuthenticatedReleaseV1,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    let (eq_protocol_digest, ep_protocol_digest) = release
        .qualified_relation_protocol_digests(KagemushaQualifiedRelationV1::TerminalAuthorization);
    if artifacts.release_id != release.release_id()
        || artifacts.profile_digest != release.profile_digest()
        || artifacts.artifact_manifest_digest != release.manifest_digest()
        || artifacts.terminal_authorization_eq_protocol_digest != eq_protocol_digest
        || artifacts.terminal_authorization_ep_protocol_digest != ep_protocol_digest
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "TerminalAuthorization artifacts and authenticated release do not match".to_owned(),
        ));
    }
    Ok(())
}

/// Load and cross-check the authenticated Eq terminal-authorization PK/VK roles.
///
/// # Errors
///
/// Rejects an invalid profile, content-address mismatch, malformed key, trailing bytes, or an
/// embedded proving-key verifier that differs from the separately authenticated VK.
#[cfg(feature = "zk-halo2-ipa")]
pub fn load_kagemusha_eq_terminal_authorization_artifacts_v1<R>(
    artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
    release: &KagemushaAuthenticatedReleaseV1,
    circuit_params: BaseCircuitParams,
) -> Result<KagemushaLoadedEqTerminalAuthorizationArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    R: KagemushaArtifactByteResolverV1,
{
    validate_terminal_authorization_profile(KagemushaPastaParityV1::Eq, &circuit_params)?;
    let recursion_release = artifacts.recursion_artifacts();
    validate_terminal_authorization_release_alignment_v1(recursion_release, release)?;
    let enabled_hardware_profiles =
        kagemusha_terminal_authorization_enabled_profile_table_v1(release)?;
    let suite_id = release.enabled_profiles()[0].suite_id;
    let vk_digest = release.vk_set_digest();
    let parameters = artifacts.load_eq_params()?;
    let verifying_bytes = artifacts.resolve(KagemushaArtifactRoleV1::TerminalAuthorizationVkEq)?;
    let verifying_key =
        read_eq_terminal_authorization_vk(verifying_bytes.as_ref(), circuit_params.clone())?;
    let proving_key = load_authenticated_proving_key_v1::<
        EqAffine,
        KagemushaTerminalAuthorizationEqCircuitV1,
        _,
    >(
        artifacts,
        KagemushaArtifactRoleV1::TerminalAuthorizationPkEq,
        KagemushaPastaParityV1::Eq,
        circuit_params.clone(),
    )?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Eq,
        &proving_key,
        verifying_bytes.as_ref(),
    )?;
    let protocol = compile(
        &parameters,
        &verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let protocol_digest = native_parent_protocol_digest_v1(&protocol, KagemushaPastaParityV1::Eq)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if protocol_digest != recursion_release.terminal_authorization_eq_protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "authenticated Eq terminal-authorization protocol does not match its verifying key"
                .to_owned(),
        ));
    }
    Ok(KagemushaLoadedEqTerminalAuthorizationArtifactsV1 {
        parameters,
        proving_key,
        verifying_key,
        circuit_params,
        protocol_digest,
        release_id: recursion_release.release_id,
        profile_digest: recursion_release.profile_digest,
        artifact_manifest_digest: recursion_release.artifact_manifest_digest,
        suite_id,
        vk_digest,
        enabled_hardware_profiles,
    })
}

/// Load and cross-check the authenticated Ep terminal-authorization PK/VK roles.
///
/// # Errors
///
/// Rejects an invalid profile, content-address mismatch, malformed key, trailing bytes, or an
/// embedded proving-key verifier that differs from the separately authenticated VK.
#[cfg(feature = "zk-halo2-ipa")]
pub fn load_kagemusha_ep_terminal_authorization_artifacts_v1<R>(
    artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
    release: &KagemushaAuthenticatedReleaseV1,
    circuit_params: BaseCircuitParams,
) -> Result<KagemushaLoadedEpTerminalAuthorizationArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    R: KagemushaArtifactByteResolverV1,
{
    validate_terminal_authorization_profile(KagemushaPastaParityV1::Ep, &circuit_params)?;
    let recursion_release = artifacts.recursion_artifacts();
    validate_terminal_authorization_release_alignment_v1(recursion_release, release)?;
    let enabled_hardware_profiles =
        kagemusha_terminal_authorization_enabled_profile_table_v1(release)?;
    let suite_id = release.enabled_profiles()[0].suite_id;
    let vk_digest = release.vk_set_digest();
    let parameters = artifacts.load_ep_params()?;
    let verifying_bytes = artifacts.resolve(KagemushaArtifactRoleV1::TerminalAuthorizationVkEp)?;
    let verifying_key =
        read_ep_terminal_authorization_vk(verifying_bytes.as_ref(), circuit_params.clone())?;
    let proving_key = load_authenticated_proving_key_v1::<
        EpAffine,
        KagemushaTerminalAuthorizationEpCircuitV1,
        _,
    >(
        artifacts,
        KagemushaArtifactRoleV1::TerminalAuthorizationPkEp,
        KagemushaPastaParityV1::Ep,
        circuit_params.clone(),
    )?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Ep,
        &proving_key,
        verifying_bytes.as_ref(),
    )?;
    let protocol = compile(
        &parameters,
        &verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let protocol_digest = native_parent_protocol_digest_v1(&protocol, KagemushaPastaParityV1::Ep)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if protocol_digest != recursion_release.terminal_authorization_ep_protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "authenticated Ep terminal-authorization protocol does not match its verifying key"
                .to_owned(),
        ));
    }
    Ok(KagemushaLoadedEpTerminalAuthorizationArtifactsV1 {
        parameters,
        proving_key,
        verifying_key,
        circuit_params,
        protocol_digest,
        release_id: recursion_release.release_id,
        profile_digest: recursion_release.profile_digest,
        artifact_manifest_digest: recursion_release.artifact_manifest_digest,
        suite_id,
        vk_digest,
        enabled_hardware_profiles,
    })
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_commit_wrapper_release_alignment_v1(
    artifacts: super::KagemushaRecursionArtifactsV1,
    release: &KagemushaAuthenticatedReleaseV1,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    let (eq_protocol_digest, ep_protocol_digest) =
        release.qualified_relation_protocol_digests(KagemushaQualifiedRelationV1::CommitWrapper);
    if artifacts.release_id != release.release_id()
        || artifacts.profile_digest != release.profile_digest()
        || artifacts.artifact_manifest_digest != release.manifest_digest()
        || artifacts.commit_wrapper_eq_protocol_digest != eq_protocol_digest
        || artifacts.commit_wrapper_ep_protocol_digest != ep_protocol_digest
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "CommitWrapper artifacts and authenticated release do not match".to_owned(),
        ));
    }
    Ok(())
}

/// Load and cross-check the dedicated authenticated Eq authorization PK/VK roles.
///
/// # Errors
///
/// Rejects an invalid profile, content-address mismatch, malformed or aliased key, trailing
/// bytes, or a compiled protocol differing from the qualified release relation.
#[cfg(feature = "zk-halo2-ipa")]
pub fn load_kagemusha_eq_commit_wrapper_artifacts_v1<R>(
    artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
    release: &KagemushaAuthenticatedReleaseV1,
    circuit_params: BaseCircuitParams,
) -> Result<KagemushaLoadedEqCommitWrapperArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    R: KagemushaArtifactByteResolverV1,
{
    validate_terminal_authorization_profile(KagemushaPastaParityV1::Eq, &circuit_params)?;
    let recursion_release = artifacts.recursion_artifacts();
    validate_commit_wrapper_release_alignment_v1(recursion_release, release)?;
    let enabled_hardware_profiles =
        kagemusha_terminal_authorization_enabled_profile_table_v1(release)?;
    let suite_id = release.enabled_profiles()[0].suite_id;
    let vk_digest = release.vk_set_digest();
    let parameters = artifacts.load_eq_params()?;
    let verifying_bytes = artifacts.resolve(KagemushaArtifactRoleV1::CommitWrapperVkEq)?;
    let verifying_key =
        read_eq_commit_wrapper_vk(verifying_bytes.as_ref(), circuit_params.clone())?;
    let proving_key =
        load_authenticated_proving_key_v1::<EqAffine, KagemushaCommitWrapperEqCircuitV1, _>(
            artifacts,
            KagemushaArtifactRoleV1::CommitWrapperPkEq,
            KagemushaPastaParityV1::Eq,
            circuit_params.clone(),
        )?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Eq,
        &proving_key,
        verifying_bytes.as_ref(),
    )?;
    let protocol = compile(
        &parameters,
        &verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    validate_transport_protocol_profile(KagemushaPastaParityV1::Eq, "commit wrapper", &protocol)?;
    let protocol_digest = native_parent_protocol_digest_v1(&protocol, KagemushaPastaParityV1::Eq)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if protocol_digest != recursion_release.commit_wrapper_eq_protocol_digest
        || protocol_digest == recursion_release.terminal_authorization_eq_protocol_digest
        || verifying_bytes.as_ref()
            == artifacts
                .resolve(KagemushaArtifactRoleV1::TerminalAuthorizationVkEq)?
                .as_ref()
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "authenticated Eq authorization role aliases or mismatches its dedicated protocol"
                .to_owned(),
        ));
    }
    Ok(KagemushaLoadedEqCommitWrapperArtifactsV1 {
        parameters,
        proving_key,
        verifying_key,
        circuit_params,
        protocol_digest,
        terminal_authorization_protocol_digest: recursion_release
            .terminal_authorization_eq_protocol_digest,
        release_id: recursion_release.release_id,
        profile_digest: recursion_release.profile_digest,
        artifact_manifest_digest: recursion_release.artifact_manifest_digest,
        suite_id,
        vk_digest,
        enabled_hardware_profiles,
    })
}

/// Load and cross-check the dedicated authenticated Ep authorization PK/VK roles.
///
/// # Errors
///
/// Rejects an invalid profile, content-address mismatch, malformed or aliased key, trailing
/// bytes, or a compiled protocol differing from the qualified release relation.
#[cfg(feature = "zk-halo2-ipa")]
pub fn load_kagemusha_ep_commit_wrapper_artifacts_v1<R>(
    artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
    release: &KagemushaAuthenticatedReleaseV1,
    circuit_params: BaseCircuitParams,
) -> Result<KagemushaLoadedEpCommitWrapperArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    R: KagemushaArtifactByteResolverV1,
{
    validate_terminal_authorization_profile(KagemushaPastaParityV1::Ep, &circuit_params)?;
    let recursion_release = artifacts.recursion_artifacts();
    validate_commit_wrapper_release_alignment_v1(recursion_release, release)?;
    let enabled_hardware_profiles =
        kagemusha_terminal_authorization_enabled_profile_table_v1(release)?;
    let suite_id = release.enabled_profiles()[0].suite_id;
    let vk_digest = release.vk_set_digest();
    let parameters = artifacts.load_ep_params()?;
    let verifying_bytes = artifacts.resolve(KagemushaArtifactRoleV1::CommitWrapperVkEp)?;
    let verifying_key =
        read_ep_commit_wrapper_vk(verifying_bytes.as_ref(), circuit_params.clone())?;
    let proving_key =
        load_authenticated_proving_key_v1::<EpAffine, KagemushaCommitWrapperEpCircuitV1, _>(
            artifacts,
            KagemushaArtifactRoleV1::CommitWrapperPkEp,
            KagemushaPastaParityV1::Ep,
            circuit_params.clone(),
        )?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Ep,
        &proving_key,
        verifying_bytes.as_ref(),
    )?;
    let protocol = compile(
        &parameters,
        &verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    validate_transport_protocol_profile(KagemushaPastaParityV1::Ep, "commit wrapper", &protocol)?;
    let protocol_digest = native_parent_protocol_digest_v1(&protocol, KagemushaPastaParityV1::Ep)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if protocol_digest != recursion_release.commit_wrapper_ep_protocol_digest
        || protocol_digest == recursion_release.terminal_authorization_ep_protocol_digest
        || verifying_bytes.as_ref()
            == artifacts
                .resolve(KagemushaArtifactRoleV1::TerminalAuthorizationVkEp)?
                .as_ref()
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "authenticated Ep authorization role aliases or mismatches its dedicated protocol"
                .to_owned(),
        ));
    }
    Ok(KagemushaLoadedEpCommitWrapperArtifactsV1 {
        parameters,
        proving_key,
        verifying_key,
        circuit_params,
        protocol_digest,
        terminal_authorization_protocol_digest: recursion_release
            .terminal_authorization_ep_protocol_digest,
        release_id: recursion_release.release_id,
        profile_digest: recursion_release.profile_digest,
        artifact_manifest_digest: recursion_release.artifact_manifest_digest,
        suite_id,
        vk_digest,
        enabled_hardware_profiles,
    })
}

/// Produce the internal candidate-plus-certificate proof after the hardware terminal commit.
///
/// This non-Norito carrier is recursively consumed by CommitWrapper; it is never transported.
/// Deferred audits are derived and rebound into exact 81-instance columns before proving.
/// The original operation's hardware-unsealed secret seed is mandatory after a crash; all
/// candidate, Guard, and history witnesses must be retained or deterministically reconstructed.
///
/// # Errors
///
/// Returns an error for invalid terminal material, release/profile/protocol substitution, or
/// a proof that differs from the exact release-pinned internal protocol shape.
#[cfg(feature = "zk-halo2-ipa")]
pub fn prove_kagemusha_terminal_authorization_v1(
    eq: &KagemushaLoadedEqTerminalAuthorizationArtifactsV1,
    ep: &KagemushaLoadedEpTerminalAuthorizationArtifactsV1,
    witness: KagemushaTerminalAuthorizationGenerationWitnessV1<'_>,
    recovery_seed: &KagemushaRecoverySeedV1,
) -> Result<KagemushaGeneratedTerminalAuthorizationProofV1, KagemushaArtifactGenerationErrorV1> {
    let generation_public = witness.public.clone();
    validate_terminal_authorization_profile(KagemushaPastaParityV1::Eq, &eq.circuit_params)?;
    validate_terminal_authorization_profile(KagemushaPastaParityV1::Ep, &ep.circuit_params)?;
    if eq.release_id != ep.release_id
        || eq.profile_digest != ep.profile_digest
        || eq.artifact_manifest_digest != ep.artifact_manifest_digest
        || eq.suite_id != ep.suite_id
        || eq.vk_digest != ep.vk_digest
        || eq.enabled_hardware_profiles != ep.enabled_hardware_profiles
        || witness.enabled_hardware_profiles != eq.enabled_hardware_profiles
        || generation_public.lifecycle.release_id != eq.release_id
        || generation_public.lifecycle.suite_id != eq.suite_id
        || generation_public.lifecycle.vk_digest != eq.vk_digest
        || !eq
            .enabled_hardware_profiles
            .iter()
            .take_while(|profile| **profile != [0; 32])
            .any(|profile| *profile == generation_public.lifecycle.hardware_profile_id)
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "terminal-authorization parities and witness do not belong to one authenticated release"
                .to_owned(),
        ));
    }
    if eq.protocol_digest == ep.protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "Eq and Ep loaded terminal-authorization protocol identities alias".to_owned(),
        ));
    }
    let placeholder_eq_audit = encode(Fp::from(3));
    let placeholder_ep_audit = encode(Fq::from(4));
    let (_, _, eq_deferred_audit, ep_deferred_audit) =
        build_terminal_authorization_generation_pair_v1(
            &eq.parameters,
            &ep.parameters,
            witness.clone(),
            placeholder_eq_audit,
            placeholder_ep_audit,
            eq.protocol_digest,
            ep.protocol_digest,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let public = generation_public
        .clone()
        .into_internal(
            eq_deferred_audit,
            ep_deferred_audit,
            eq.protocol_digest,
            ep.protocol_digest,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let eq_instances = terminal_authorization_public_instances::<Fp>(
        &public,
        witness.eq.successor_history.as_bytes(),
    )?;
    let ep_instances = terminal_authorization_public_instances::<Fq>(
        &public,
        witness.ep.successor_history.as_bytes(),
    )?;
    let eq_history = witness.eq.successor_history.clone();
    let ep_history = witness.ep.successor_history.clone();
    let (eq_circuit, ep_circuit, rebuilt_eq_audit, rebuilt_ep_audit) =
        build_terminal_authorization_generation_pair_v1(
            &eq.parameters,
            &ep.parameters,
            witness,
            eq_deferred_audit,
            ep_deferred_audit,
            eq.protocol_digest,
            ep.protocol_digest,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if rebuilt_eq_audit != eq_deferred_audit || rebuilt_ep_audit != ep_deferred_audit {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "terminal-authorization deferred audit changed while binding its public cells"
                .to_owned(),
        ));
    }
    if !same_base_params(&eq_circuit.params(), &eq.circuit_params) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
            KagemushaPastaParityV1::Eq,
        ));
    }
    if !same_base_params(&ep_circuit.params(), &ep.circuit_params) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
            KagemushaPastaParityV1::Ep,
        ));
    }
    let eq_proof = create_eq_proof_with_key_v1(
        &eq.parameters,
        &eq.proving_key,
        eq_circuit,
        &eq_instances,
        KagemushaProofRecoveryPhaseV1::TerminalAuthorization,
        recovery_seed,
    )?;
    let ep_proof = create_ep_proof_with_key_v1(
        &ep.parameters,
        &ep.proving_key,
        ep_circuit,
        &ep_instances,
        KagemushaProofRecoveryPhaseV1::TerminalAuthorization,
        recovery_seed,
    )?;
    let eq_protocol = compile(
        &eq.parameters,
        &eq.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_protocol = compile(
        &ep.parameters,
        &ep.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    validate_internal_recursive_proof_length(
        KagemushaPastaParityV1::Eq,
        "internal terminal authorization",
        &eq_protocol,
        &eq_proof,
    )?;
    validate_internal_recursive_proof_length(
        KagemushaPastaParityV1::Ep,
        "internal terminal authorization",
        &ep_protocol,
        &ep_proof,
    )?;
    let actual_eq_protocol =
        native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let actual_ep_protocol =
        native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if actual_eq_protocol != eq.protocol_digest || actual_ep_protocol != ep.protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "loaded terminal-authorization protocol changed before proving".to_owned(),
        ));
    }
    let eq_current_accumulator = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(&eq.parameters, &eq_protocol, &eq_proof, &eq_instances)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_current_accumulator = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(&ep.parameters, &ep_protocol, &ep_proof, &ep_instances)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    Ok(KagemushaGeneratedTerminalAuthorizationProofV1 {
        eq_public_instances: eq_instances,
        ep_public_instances: ep_instances,
        eq_proof,
        ep_proof,
        eq_history,
        ep_history,
        eq_current_accumulator,
        ep_current_accumulator,
    })
}

/// Produce the sole transported CommitWrapper pair from the genuine post-commit inner proof.
///
/// Its exact 81-cell projection preserves the body, candidate, certificate, and full lifecycle.
/// Internal TerminalAuthorization and aggregate-state keys cannot be substituted.
///
/// # Errors
///
/// Returns an error for terminal projection, release/profile/protocol substitution, invalid
/// internal proof material, proof failure, or a proof larger than the hard transport allocation.
#[cfg(feature = "zk-halo2-ipa")]
pub fn prove_kagemusha_commit_wrapper_v1(
    eq: &KagemushaLoadedEqCommitWrapperArtifactsV1,
    ep: &KagemushaLoadedEpCommitWrapperArtifactsV1,
    witness: KagemushaCommitWrapperGenerationWitnessV1<'_>,
    recovery_seed: &KagemushaRecoverySeedV1,
) -> Result<KagemushaGeneratedCommitWrapperProofV1, KagemushaArtifactGenerationErrorV1> {
    let generation_public = witness.public.clone();
    validate_terminal_authorization_profile(KagemushaPastaParityV1::Eq, &eq.circuit_params)?;
    validate_terminal_authorization_profile(KagemushaPastaParityV1::Ep, &ep.circuit_params)?;
    if eq.release_id != ep.release_id
        || eq.profile_digest != ep.profile_digest
        || eq.artifact_manifest_digest != ep.artifact_manifest_digest
        || eq.suite_id != ep.suite_id
        || eq.vk_digest != ep.vk_digest
        || eq.enabled_hardware_profiles != ep.enabled_hardware_profiles
        || witness.enabled_hardware_profiles != eq.enabled_hardware_profiles
        || generation_public.lifecycle.release_id != eq.release_id
        || generation_public.lifecycle.suite_id != eq.suite_id
        || generation_public.lifecycle.vk_digest != eq.vk_digest
        || !eq
            .enabled_hardware_profiles
            .iter()
            .take_while(|profile| **profile != [0; 32])
            .any(|profile| *profile == generation_public.lifecycle.hardware_profile_id)
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "commit-wrapper parities and terminal projection do not belong to one authenticated release"
                .to_owned(),
        ));
    }
    if eq.protocol_digest == ep.protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "Eq and Ep loaded commit wrapper protocols alias".to_owned(),
        ));
    }
    let nested_eq_protocol_digest = native_parent_protocol_digest_v1(
        witness.eq.terminal_authorization_protocol,
        KagemushaPastaParityV1::Eq,
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let nested_ep_protocol_digest = native_parent_protocol_digest_v1(
        witness.ep.terminal_authorization_protocol,
        KagemushaPastaParityV1::Ep,
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if nested_eq_protocol_digest != eq.terminal_authorization_protocol_digest
        || nested_ep_protocol_digest != ep.terminal_authorization_protocol_digest
        || nested_eq_protocol_digest == nested_ep_protocol_digest
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "nested terminal-authorization protocol does not match the authenticated release"
                .to_owned(),
        ));
    }
    validate_internal_recursive_proof_length(
        KagemushaPastaParityV1::Eq,
        "nested terminal authorization",
        witness.eq.terminal_authorization_protocol,
        witness.eq.terminal_authorization_proof,
    )?;
    validate_internal_recursive_proof_length(
        KagemushaPastaParityV1::Ep,
        "nested terminal authorization",
        witness.ep.terminal_authorization_protocol,
        witness.ep.terminal_authorization_proof,
    )?;

    // Only the second build, which binds the derived mutually audited outputs, is proved.
    // These public advice seeds affect neither the pinned inner protocol nor the outer key.
    let layout_eq_audit = encode(Fp::from(3));
    let layout_ep_audit = encode(Fq::from(4));
    let (_, _, eq_deferred_audit, ep_deferred_audit) = build_commit_wrapper_generation_pair_v1(
        &eq.parameters,
        &ep.parameters,
        witness.clone(),
        layout_eq_audit,
        layout_ep_audit,
        eq.protocol_digest,
        ep.protocol_digest,
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let public = generation_public
        .into_internal(
            eq_deferred_audit,
            ep_deferred_audit,
            eq.protocol_digest,
            ep.protocol_digest,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let eq_instances = terminal_authorization_public_instances::<Fp>(
        &public,
        witness.eq.successor_history.as_bytes(),
    )?;
    let ep_instances = terminal_authorization_public_instances::<Fq>(
        &public,
        witness.ep.successor_history.as_bytes(),
    )?;
    let eq_history = witness.eq.successor_history.clone();
    let ep_history = witness.ep.successor_history.clone();
    let (eq_circuit, ep_circuit, rebuilt_eq_audit, rebuilt_ep_audit) =
        build_commit_wrapper_generation_pair_v1(
            &eq.parameters,
            &ep.parameters,
            witness,
            eq_deferred_audit,
            ep_deferred_audit,
            eq.protocol_digest,
            ep.protocol_digest,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if rebuilt_eq_audit != eq_deferred_audit || rebuilt_ep_audit != ep_deferred_audit {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "authorization deferred audit changed while binding its public cells".to_owned(),
        ));
    }
    if !same_base_params(&eq_circuit.params(), &eq.circuit_params) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
            KagemushaPastaParityV1::Eq,
        ));
    }
    if !same_base_params(&ep_circuit.params(), &ep.circuit_params) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
            KagemushaPastaParityV1::Ep,
        ));
    }
    let eq_proof = create_eq_proof_with_key_v1(
        &eq.parameters,
        &eq.proving_key,
        eq_circuit,
        &eq_instances,
        KagemushaProofRecoveryPhaseV1::CommitWrapper,
        recovery_seed,
    )?;
    let ep_proof = create_ep_proof_with_key_v1(
        &ep.parameters,
        &ep.proving_key,
        ep_circuit,
        &ep_instances,
        KagemushaProofRecoveryPhaseV1::CommitWrapper,
        recovery_seed,
    )?;
    validate_paired_proof_length(KagemushaPastaParityV1::Eq, &eq_proof)?;
    validate_paired_proof_length(KagemushaPastaParityV1::Ep, &ep_proof)?;
    let eq_protocol = compile(
        &eq.parameters,
        &eq.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_protocol = compile(
        &ep.parameters,
        &ep.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Eq,
        "commit wrapper",
        &eq_protocol,
    )?;
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Ep,
        "commit wrapper",
        &ep_protocol,
    )?;
    let actual_eq_protocol =
        native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let actual_ep_protocol =
        native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if actual_eq_protocol != eq.protocol_digest || actual_ep_protocol != ep.protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "loaded authorization protocol changed before proving".to_owned(),
        ));
    }
    let eq_current_accumulator = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(&eq.parameters, &eq_protocol, &eq_proof, &eq_instances)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_current_accumulator = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(&ep.parameters, &ep_protocol, &ep_proof, &ep_instances)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let generated = KagemushaGeneratedCommitWrapperProofV1 {
        eq_public_instances: eq_instances,
        ep_public_instances: ep_instances,
        public,
        eq_proof,
        ep_proof,
        eq_history,
        ep_history,
        eq_current_accumulator,
        ep_current_accumulator,
    };
    generated.validate_material()?;
    Ok(generated)
}

#[cfg(feature = "zk-halo2-ipa")]
fn build_mint_authorization_generation_pair_v1(
    eq_parameters: &ParamsIPA<EqAffine>,
    ep_parameters: &ParamsIPA<EpAffine>,
    witness: KagemushaMintAuthorizationGenerationWitnessV1<'_>,
    eq_deferred_audit: [u8; 32],
    ep_deferred_audit: [u8; 32],
) -> Result<
    (
        KagemushaMintAuthorizationEqCircuitV1,
        KagemushaMintAuthorizationEpCircuitV1,
        [u8; 32],
        [u8; 32],
    ),
    KagemushaArtifactGenerationErrorV1,
> {
    let eq_svk = super::composite::eq_succinct_vk(eq_parameters);
    let ep_svk = super::composite::ep_succinct_vk(ep_parameters);
    build_kagemusha_mint_authorization_pair_v1(
        &eq_svk,
        &ep_svk,
        witness.into_internal(eq_deferred_audit, ep_deferred_audit),
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)
}

/// Generate eight inner/outer MintAuthorization keys from a real inner proof.
///
/// The operation's sealed recovery seed separates private and transported proof streams.
/// Only the compact outer protocols are admitted against the unchanged transport ceiling.
///
/// # Errors
///
/// Returns an error for an invalid relation/profile table, circuit-profile mismatch, key
/// generation failure, oversized artifact, or aliased parity protocol identity.
#[cfg(feature = "zk-halo2-ipa")]
pub fn generate_kagemusha_mint_authorization_artifacts_v1(
    witness: KagemushaMintAuthorizationGenerationWitnessV1<'_>,
    recovery_seed: &KagemushaRecoverySeedV1,
) -> Result<KagemushaGeneratedMintAuthorizationArtifactsV1, KagemushaArtifactGenerationErrorV1> {
    let enabled_hardware_profiles = witness.enabled_hardware_profiles;
    let eq_parameters = ParamsIPA::<EqAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let ep_parameters = ParamsIPA::<EpAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let (inner_eq_circuit, inner_ep_circuit, _, _) = build_mint_authorization_generation_pair_v1(
        &eq_parameters,
        &ep_parameters,
        witness.clone(),
        encode(Fp::from(3)),
        encode(Fq::from(4)),
    )?;
    let inner_eq_circuit_params = inner_eq_circuit.params();
    let inner_ep_circuit_params = inner_ep_circuit.params();
    validate_recursive_profile(KagemushaPastaParityV1::Eq, &inner_eq_circuit_params)?;
    validate_recursive_profile(KagemushaPastaParityV1::Ep, &inner_ep_circuit_params)?;
    let inner_eq_vk = keygen_vk(&eq_parameters, &inner_eq_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Eq,
            kind: "mint-authorization verifying key",
            reason: error.to_string(),
        }
    })?;
    let inner_eq_pk =
        keygen_pk(&eq_parameters, inner_eq_vk.clone(), &inner_eq_circuit).map_err(|error| {
            KagemushaArtifactGenerationErrorV1::KeyGeneration {
                parity: KagemushaPastaParityV1::Eq,
                kind: "mint-authorization proving key",
                reason: error.to_string(),
            }
        })?;
    let inner_ep_vk = keygen_vk(&ep_parameters, &inner_ep_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Ep,
            kind: "mint-authorization verifying key",
            reason: error.to_string(),
        }
    })?;
    let inner_ep_pk =
        keygen_pk(&ep_parameters, inner_ep_vk.clone(), &inner_ep_circuit).map_err(|error| {
            KagemushaArtifactGenerationErrorV1::KeyGeneration {
                parity: KagemushaPastaParityV1::Ep,
                kind: "mint-authorization proving key",
                reason: error.to_string(),
            }
        })?;

    // These four keys are private carriers. Transport limits apply to the final deciders,
    // never to the SHA-heavy inner proof; artifact/RSS qualification still applies to both.
    let (_, inner_eq_proving_key, inner_eq_verifying_key) = build_generated_helper_parity(
        KagemushaPastaParityV1::Eq,
        "inner mint-authorization proving key",
        &eq_parameters,
        &inner_eq_pk,
        &inner_eq_vk,
    )?;
    let (_, inner_ep_proving_key, inner_ep_verifying_key) = build_generated_helper_parity(
        KagemushaPastaParityV1::Ep,
        "inner mint-authorization proving key",
        &ep_parameters,
        &inner_ep_pk,
        &inner_ep_vk,
    )?;
    let prepared = prepare_mint_authorization_transport_v1(
        KagemushaMintAuthorizationInnerKeysV1 {
            parameters: &eq_parameters,
            proving_key: &inner_eq_pk,
            verifying_key: &inner_eq_vk,
            circuit_params: &inner_eq_circuit_params,
        },
        KagemushaMintAuthorizationInnerKeysV1 {
            parameters: &ep_parameters,
            proving_key: &inner_ep_pk,
            verifying_key: &inner_ep_vk,
            circuit_params: &inner_ep_circuit_params,
        },
        witness,
        recovery_seed,
    )?;
    let eq_circuit = prepared.eq_circuit;
    let ep_circuit = prepared.ep_circuit;
    let eq_circuit_params = eq_circuit.params();
    let ep_circuit_params = ep_circuit.params();
    validate_recursive_profile(KagemushaPastaParityV1::Eq, &eq_circuit_params)?;
    validate_recursive_profile(KagemushaPastaParityV1::Ep, &ep_circuit_params)?;
    let eq_vk = keygen_vk(&eq_parameters, &eq_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Eq,
            kind: "mint-authorization verifying key",
            reason: error.to_string(),
        }
    })?;
    let eq_pk = keygen_pk(&eq_parameters, eq_vk.clone(), &eq_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Eq,
            kind: "mint-authorization proving key",
            reason: error.to_string(),
        }
    })?;
    let ep_vk = keygen_vk(&ep_parameters, &ep_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Ep,
            kind: "mint-authorization verifying key",
            reason: error.to_string(),
        }
    })?;
    let ep_pk = keygen_pk(&ep_parameters, ep_vk.clone(), &ep_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Ep,
            kind: "mint-authorization proving key",
            reason: error.to_string(),
        }
    })?;
    let eq_protocol = compile(
        &eq_parameters,
        &eq_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_protocol = compile(
        &ep_parameters,
        &ep_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Eq,
        "mint authorization",
        &eq_protocol,
    )?;
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Ep,
        "mint authorization",
        &ep_protocol,
    )?;
    let eq_protocol_digest =
        native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let ep_protocol_digest =
        native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if eq_protocol_digest == ep_protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "Eq and Ep mint-authorization protocol identities alias".to_owned(),
        ));
    }
    let (eq_parameters, eq_proving_key, eq_verifying_key) = build_generated_helper_parity(
        KagemushaPastaParityV1::Eq,
        "mint-authorization proving key",
        &eq_parameters,
        &eq_pk,
        &eq_vk,
    )?;
    let (ep_parameters, ep_proving_key, ep_verifying_key) = build_generated_helper_parity(
        KagemushaPastaParityV1::Ep,
        "mint-authorization proving key",
        &ep_parameters,
        &ep_pk,
        &ep_vk,
    )?;
    Ok(KagemushaGeneratedMintAuthorizationArtifactsV1 {
        eq_parameters,
        ep_parameters,
        eq_proving_key,
        eq_verifying_key,
        ep_proving_key,
        ep_verifying_key,
        eq_circuit_params,
        ep_circuit_params,
        eq_protocol_digest,
        ep_protocol_digest,
        enabled_hardware_profiles,
        inner_eq_proving_key,
        inner_eq_verifying_key,
        inner_ep_proving_key,
        inner_ep_verifying_key,
        inner_eq_circuit_params,
        inner_ep_circuit_params,
    })
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_mint_authorization_release_alignment_v1(
    artifacts: super::KagemushaRecursionArtifactsV1,
    release: &KagemushaAuthenticatedReleaseV1,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    let helper = release
        .helper_protocol(KagemushaQualifiedHelperCircuitV1::MintAuthorization)
        .ok_or_else(|| {
            KagemushaArtifactGenerationErrorV1::CircuitBuild(
                "authenticated release omits MintAuthorization protocols".to_owned(),
            )
        })?;
    if artifacts.release_id != release.release_id()
        || artifacts.profile_digest != release.profile_digest()
        || artifacts.artifact_manifest_digest != release.manifest_digest()
        || artifacts.mint_authorization_eq_protocol_digest != helper.eq_protocol_digest
        || artifacts.mint_authorization_ep_protocol_digest != helper.ep_protocol_digest
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "MintAuthorization artifacts and authenticated release do not match".to_owned(),
        ));
    }
    Ok(())
}

/// Load and cross-check the authenticated Eq MintAuthorization PK/VK roles.
///
/// # Errors
///
/// Returns an error for release substitution, malformed keys, trailing bytes, profile mismatch,
/// or a proving key whose embedded verifier differs from the authenticated standalone key.
#[cfg(feature = "zk-halo2-ipa")]
pub fn load_kagemusha_eq_mint_authorization_artifacts_v1<R>(
    artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
    release: &KagemushaAuthenticatedReleaseV1,
    circuit_params: BaseCircuitParams,
    inner_circuit_params: BaseCircuitParams,
) -> Result<KagemushaLoadedEqMintAuthorizationArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    R: KagemushaArtifactByteResolverV1,
{
    validate_recursive_profile(KagemushaPastaParityV1::Eq, &circuit_params)?;
    validate_recursive_profile(KagemushaPastaParityV1::Eq, &inner_circuit_params)?;
    let recursion_release = artifacts.recursion_artifacts();
    validate_mint_authorization_release_alignment_v1(recursion_release, release)?;
    let helper = release
        .helper_protocol(KagemushaQualifiedHelperCircuitV1::MintAuthorization)
        .expect("release alignment established helper presence");
    let enabled_hardware_profiles = kagemusha_enabled_hardware_profile_table_v1(release)?;
    let parameters = artifacts.load_eq_params()?;
    let verifying_bytes = artifacts.resolve(KagemushaArtifactRoleV1::MintAuthorizationVkEq)?;
    let verifying_key =
        read_eq_mint_authorization_vk(verifying_bytes.as_ref(), circuit_params.clone())?;
    let proving_key = load_authenticated_proving_key_v1::<
        EqAffine,
        KagemushaMintAuthorizationTransportEqCircuitV1,
        _,
    >(
        artifacts,
        KagemushaArtifactRoleV1::MintAuthorizationPkEq,
        KagemushaPastaParityV1::Eq,
        circuit_params.clone(),
    )?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Eq,
        &proving_key,
        verifying_bytes.as_ref(),
    )?;

    let inner_verifying_bytes =
        artifacts.resolve(KagemushaArtifactRoleV1::InnerMintAuthorizationVkEq)?;
    let inner_verifying_key = read_eq_inner_mint_authorization_vk(
        inner_verifying_bytes.as_ref(),
        inner_circuit_params.clone(),
    )?;
    let inner_proving_key =
        load_authenticated_proving_key_v1::<EqAffine, KagemushaMintAuthorizationEqCircuitV1, _>(
            artifacts,
            KagemushaArtifactRoleV1::InnerMintAuthorizationPkEq,
            KagemushaPastaParityV1::Eq,
            inner_circuit_params.clone(),
        )?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Eq,
        &inner_proving_key,
        inner_verifying_bytes.as_ref(),
    )?;
    let protocol = compile(
        &parameters,
        &verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Eq,
        "mint authorization",
        &protocol,
    )?;
    let protocol_digest = native_parent_protocol_digest_v1(&protocol, KagemushaPastaParityV1::Eq)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if protocol_digest != helper.eq_protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "authenticated Eq MintAuthorization protocol does not match its verifying key"
                .to_owned(),
        ));
    }
    Ok(KagemushaLoadedEqMintAuthorizationArtifactsV1 {
        parameters,
        proving_key,
        verifying_key,
        circuit_params,
        protocol_digest,
        inner_proving_key,
        inner_verifying_key,
        inner_circuit_params,
        release_id: release.release_id(),
        profile_digest: release.profile_digest(),
        artifact_manifest_digest: release.manifest_digest(),
        suite_id: release.enabled_profiles()[0].suite_id,
        vk_digest: release.vk_set_digest(),
        enabled_hardware_profiles,
    })
}

/// Load and cross-check the authenticated Ep MintAuthorization PK/VK roles.
///
/// # Errors
///
/// Returns an error for release substitution, malformed keys, trailing bytes, profile mismatch,
/// or a proving key whose embedded verifier differs from the authenticated standalone key.
#[cfg(feature = "zk-halo2-ipa")]
pub fn load_kagemusha_ep_mint_authorization_artifacts_v1<R>(
    artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
    release: &KagemushaAuthenticatedReleaseV1,
    circuit_params: BaseCircuitParams,
    inner_circuit_params: BaseCircuitParams,
) -> Result<KagemushaLoadedEpMintAuthorizationArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    R: KagemushaArtifactByteResolverV1,
{
    validate_recursive_profile(KagemushaPastaParityV1::Ep, &circuit_params)?;
    validate_recursive_profile(KagemushaPastaParityV1::Ep, &inner_circuit_params)?;
    let recursion_release = artifacts.recursion_artifacts();
    validate_mint_authorization_release_alignment_v1(recursion_release, release)?;
    let helper = release
        .helper_protocol(KagemushaQualifiedHelperCircuitV1::MintAuthorization)
        .expect("release alignment established helper presence");
    let enabled_hardware_profiles = kagemusha_enabled_hardware_profile_table_v1(release)?;
    let parameters = artifacts.load_ep_params()?;
    let verifying_bytes = artifacts.resolve(KagemushaArtifactRoleV1::MintAuthorizationVkEp)?;
    let verifying_key =
        read_ep_mint_authorization_vk(verifying_bytes.as_ref(), circuit_params.clone())?;
    let proving_key = load_authenticated_proving_key_v1::<
        EpAffine,
        KagemushaMintAuthorizationTransportEpCircuitV1,
        _,
    >(
        artifacts,
        KagemushaArtifactRoleV1::MintAuthorizationPkEp,
        KagemushaPastaParityV1::Ep,
        circuit_params.clone(),
    )?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Ep,
        &proving_key,
        verifying_bytes.as_ref(),
    )?;

    let inner_verifying_bytes =
        artifacts.resolve(KagemushaArtifactRoleV1::InnerMintAuthorizationVkEp)?;
    let inner_verifying_key = read_ep_inner_mint_authorization_vk(
        inner_verifying_bytes.as_ref(),
        inner_circuit_params.clone(),
    )?;
    let inner_proving_key =
        load_authenticated_proving_key_v1::<EpAffine, KagemushaMintAuthorizationEpCircuitV1, _>(
            artifacts,
            KagemushaArtifactRoleV1::InnerMintAuthorizationPkEp,
            KagemushaPastaParityV1::Ep,
            inner_circuit_params.clone(),
        )?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Ep,
        &inner_proving_key,
        inner_verifying_bytes.as_ref(),
    )?;
    let protocol = compile(
        &parameters,
        &verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Ep,
        "mint authorization",
        &protocol,
    )?;
    let protocol_digest = native_parent_protocol_digest_v1(&protocol, KagemushaPastaParityV1::Ep)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if protocol_digest != helper.ep_protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "authenticated Ep MintAuthorization protocol does not match its verifying key"
                .to_owned(),
        ));
    }
    Ok(KagemushaLoadedEpMintAuthorizationArtifactsV1 {
        parameters,
        proving_key,
        verifying_key,
        circuit_params,
        protocol_digest,
        inner_proving_key,
        inner_verifying_key,
        inner_circuit_params,
        release_id: release.release_id(),
        profile_digest: release.profile_digest(),
        artifact_manifest_digest: release.manifest_digest(),
        suite_id: release.enabled_profiles()[0].suite_id,
        vk_digest: release.vk_set_digest(),
        enabled_hardware_profiles,
    })
}

/// Produce one release-pinned constant-size MintAuthorization proof pair.
///
/// # Errors
///
/// Returns an error for relation/release substitution, profile mismatch, proof failure, a changed
/// reciprocal audit, or malformed fixed-size proof/history output.
#[cfg(feature = "zk-halo2-ipa")]
pub fn prove_kagemusha_mint_authorization_v1(
    eq: &KagemushaLoadedEqMintAuthorizationArtifactsV1,
    ep: &KagemushaLoadedEpMintAuthorizationArtifactsV1,
    witness: KagemushaMintAuthorizationGenerationWitnessV1<'_>,
    recovery_seed: &KagemushaRecoverySeedV1,
) -> Result<KagemushaGeneratedMintAuthorizationProofV1, KagemushaArtifactGenerationErrorV1> {
    validate_recursive_profile(KagemushaPastaParityV1::Eq, &eq.circuit_params)?;
    validate_recursive_profile(KagemushaPastaParityV1::Ep, &ep.circuit_params)?;
    let context = &witness.relation.statement.context;
    if eq.release_id != ep.release_id
        || eq.profile_digest != ep.profile_digest
        || eq.artifact_manifest_digest != ep.artifact_manifest_digest
        || eq.suite_id != ep.suite_id
        || eq.vk_digest != ep.vk_digest
        || eq.enabled_hardware_profiles != ep.enabled_hardware_profiles
        || witness.enabled_hardware_profiles != eq.enabled_hardware_profiles
        || context.release_id != eq.release_id
        || context.suite_id != eq.suite_id
        || context.vk_digest != eq.vk_digest
        || context.artifact_manifest_digest != eq.artifact_manifest_digest
        || !eq
            .enabled_hardware_profiles
            .iter()
            .take_while(|profile| **profile != [0; 32])
            .any(|profile| *profile == context.hardware_profile_id)
        || eq.protocol_digest == ep.protocol_digest
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "MintAuthorization witness and parities do not belong to one authenticated release"
                .to_owned(),
        ));
    }

    validate_recursive_profile(KagemushaPastaParityV1::Eq, &eq.inner_circuit_params)?;
    validate_recursive_profile(KagemushaPastaParityV1::Ep, &ep.inner_circuit_params)?;
    let prepared = prepare_mint_authorization_transport_v1(
        KagemushaMintAuthorizationInnerKeysV1 {
            parameters: &eq.parameters,
            proving_key: &eq.inner_proving_key,
            verifying_key: &eq.inner_verifying_key,
            circuit_params: &eq.inner_circuit_params,
        },
        KagemushaMintAuthorizationInnerKeysV1 {
            parameters: &ep.parameters,
            proving_key: &ep.inner_proving_key,
            verifying_key: &ep.inner_verifying_key,
            circuit_params: &ep.inner_circuit_params,
        },
        witness,
        recovery_seed,
    )?;
    let KagemushaPreparedMintAuthorizationTransportV1 {
        eq_circuit,
        ep_circuit,
        eq_instances,
        ep_instances,
        eq_history,
        ep_history,
        eq_deferred_audit,
        ep_deferred_audit,
        semantic_digest,
        recipient_credential_commitment,
        hardware_authorization,
    } = prepared;
    if !same_base_params(&eq_circuit.params(), &eq.circuit_params) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
            KagemushaPastaParityV1::Eq,
        ));
    }
    if !same_base_params(&ep_circuit.params(), &ep.circuit_params) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
            KagemushaPastaParityV1::Ep,
        ));
    }
    let eq_proof = create_eq_proof_with_key_v1(
        &eq.parameters,
        &eq.proving_key,
        eq_circuit,
        &eq_instances,
        KagemushaProofRecoveryPhaseV1::MintAuthorizationTransport,
        recovery_seed,
    )?;
    let ep_proof = create_ep_proof_with_key_v1(
        &ep.parameters,
        &ep.proving_key,
        ep_circuit,
        &ep_instances,
        KagemushaProofRecoveryPhaseV1::MintAuthorizationTransport,
        recovery_seed,
    )?;
    validate_paired_proof_length(KagemushaPastaParityV1::Eq, &eq_proof)?;
    validate_paired_proof_length(KagemushaPastaParityV1::Ep, &ep_proof)?;
    let eq_protocol = compile(
        &eq.parameters,
        &eq.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_protocol = compile(
        &ep.parameters,
        &ep.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let actual_eq_protocol =
        native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let actual_ep_protocol =
        native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if actual_eq_protocol != eq.protocol_digest || actual_ep_protocol != ep.protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "loaded MintAuthorization protocol changed before proving".to_owned(),
        ));
    }
    let eq_current_accumulator = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(&eq.parameters, &eq_protocol, &eq_proof, &eq_instances)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_current_accumulator = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(&ep.parameters, &ep_protocol, &ep_proof, &ep_instances)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;

    // The exported capability carries only the compact proof, but its full monetary history
    // must already decide. Never expose a merely parsed or host-asserted opening claim.
    decide_kagemusha_eq_accumulator_v1(&eq.parameters, &eq_current_accumulator)
        .and_then(|()| decide_kagemusha_eq_accumulator_v1(&eq.parameters, &eq_history))
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    decide_kagemusha_ep_accumulator_v1(&ep.parameters, &ep_current_accumulator)
        .and_then(|()| decide_kagemusha_ep_accumulator_v1(&ep.parameters, &ep_history))
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let proof = KagemushaPairedProofV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        eq_protocol_digest: eq.protocol_digest,
        ep_protocol_digest: ep.protocol_digest,
        semantic_digest,
        guard_eq_credential_audit: recipient_credential_commitment,
        guard_ep_credential_audit: hardware_authorization,
        eq_deferred_audit,
        ep_deferred_audit,
        eq_proof,
        ep_proof,
        eq_history: eq_history.as_bytes().to_vec(),
        ep_history: ep_history.as_bytes().to_vec(),
    };
    proof
        .validate_shape_for_semantic_digest(semantic_digest)
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    Ok(KagemushaGeneratedMintAuthorizationProofV1 {
        eq_public_instances: eq_instances,
        ep_public_instances: ep_instances,
        proof,
        eq_current_accumulator,
        ep_current_accumulator,
    })
}

#[cfg(feature = "zk-halo2-ipa")]
struct KagemushaMintAuthorizationInnerKeysV1<'a, C: CurveAffine> {
    parameters: &'a ParamsIPA<C>,
    proving_key: &'a ProvingKey<C>,
    verifying_key: &'a VerifyingKey<C>,
    circuit_params: &'a BaseCircuitParams,
}

#[cfg(feature = "zk-halo2-ipa")]
struct KagemushaPreparedMintAuthorizationTransportV1 {
    eq_circuit: KagemushaMintAuthorizationTransportEqCircuitV1,
    ep_circuit: KagemushaMintAuthorizationTransportEpCircuitV1,
    eq_instances: Vec<Fp>,
    ep_instances: Vec<Fq>,
    eq_history: KagemushaEqAccumulatorV1,
    ep_history: KagemushaEpAccumulatorV1,
    eq_deferred_audit: [u8; 32],
    ep_deferred_audit: [u8; 32],
    semantic_digest: [u8; 32],
    recipient_credential_commitment: [u8; 32],
    hardware_authorization: [u8; 32],
}

/// Build the outer circuit only from a genuine, verified inner authorization proof.
#[cfg(feature = "zk-halo2-ipa")]
fn prepare_mint_authorization_transport_v1(
    eq: KagemushaMintAuthorizationInnerKeysV1<'_, EqAffine>,
    ep: KagemushaMintAuthorizationInnerKeysV1<'_, EpAffine>,
    witness: KagemushaMintAuthorizationGenerationWitnessV1<'_>,
    recovery_seed: &KagemushaRecoverySeedV1,
) -> Result<KagemushaPreparedMintAuthorizationTransportV1, KagemushaArtifactGenerationErrorV1> {
    witness
        .relation
        .validate_shape()
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let recipient_credential_commitment = witness
        .relation
        .statement
        .context
        .recipient_credential_commitment;
    let statement = witness.relation.statement.clone();
    let semantic_digest = witness
        .relation
        .statement
        .canonical_digest()
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let hardware_authorization = witness
        .relation
        .hardware_authorization_digest()
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let (_, _, eq_deferred_audit, ep_deferred_audit) = build_mint_authorization_generation_pair_v1(
        eq.parameters,
        ep.parameters,
        witness.clone(),
        encode(Fp::from(3)),
        encode(Fq::from(4)),
    )?;
    let eq_instances = mint_authorization_public_instances_v1::<Fp>(
        &witness.relation.statement,
        hardware_authorization,
        eq_deferred_audit,
        ep_deferred_audit,
        witness.eq_credential_history.as_bytes(),
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let ep_instances = mint_authorization_public_instances_v1::<Fq>(
        &witness.relation.statement,
        hardware_authorization,
        eq_deferred_audit,
        ep_deferred_audit,
        witness.ep_credential_history.as_bytes(),
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let eq_history = witness.eq_credential_history.clone();
    let ep_history = witness.ep_credential_history.clone();
    let (eq_circuit, ep_circuit, rebuilt_eq_audit, rebuilt_ep_audit) =
        build_mint_authorization_generation_pair_v1(
            eq.parameters,
            ep.parameters,
            witness,
            eq_deferred_audit,
            ep_deferred_audit,
        )?;
    if rebuilt_eq_audit != eq_deferred_audit || rebuilt_ep_audit != ep_deferred_audit {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "MintAuthorization deferred audit changed while binding public cells".to_owned(),
        ));
    }
    if !same_base_params(&eq_circuit.params(), eq.circuit_params) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
            KagemushaPastaParityV1::Eq,
        ));
    }
    if !same_base_params(&ep_circuit.params(), ep.circuit_params) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
            KagemushaPastaParityV1::Ep,
        ));
    }
    let eq_proof = create_eq_proof_with_key_v1(
        eq.parameters,
        eq.proving_key,
        eq_circuit,
        &eq_instances,
        KagemushaProofRecoveryPhaseV1::MintAuthorization,
        recovery_seed,
    )?;
    let ep_proof = create_ep_proof_with_key_v1(
        ep.parameters,
        ep.proving_key,
        ep_circuit,
        &ep_instances,
        KagemushaProofRecoveryPhaseV1::MintAuthorization,
        recovery_seed,
    )?;
    let eq_protocol = compile(
        eq.parameters,
        eq.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_protocol = compile(
        ep.parameters,
        ep.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let eq_current_accumulator = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(eq.parameters, &eq_protocol, &eq_proof, &eq_instances)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_current_accumulator = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(ep.parameters, &ep_protocol, &ep_proof, &ep_instances)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;

    // Key generation uses this same path: reject invalid private proofs/history before
    // constructing an outer witness, not only when a completed envelope is exported.
    decide_kagemusha_eq_accumulator_v1(eq.parameters, &eq_current_accumulator)
        .and_then(|()| decide_kagemusha_eq_accumulator_v1(eq.parameters, &eq_history))
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    decide_kagemusha_ep_accumulator_v1(ep.parameters, &ep_current_accumulator)
        .and_then(|()| decide_kagemusha_ep_accumulator_v1(ep.parameters, &ep_history))
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let eq_fold = fold_kagemusha_eq_accumulators_v1(
        eq.parameters,
        &eq_current_accumulator,
        &eq_history,
        recovery_seed,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_fold = fold_kagemusha_ep_accumulators_v1(
        ep.parameters,
        &ep_current_accumulator,
        &ep_history,
        recovery_seed,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let eq_native_history = eq_history
        .to_native()
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_native_history = ep_history
        .to_native()
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let outer_instances = |eq_audit, ep_audit| -> Result<_, KagemushaArtifactGenerationErrorV1> {
        Ok((
            mint_authorization_public_instances_v1::<Fp>(
                &statement,
                hardware_authorization,
                eq_audit,
                ep_audit,
                eq_fold.successor().as_bytes(),
            )
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
            mint_authorization_public_instances_v1::<Fq>(
                &statement,
                hardware_authorization,
                eq_audit,
                ep_audit,
                ep_fold.successor().as_bytes(),
            )
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
        ))
    };
    let build_outer = |eq_outer: &[Fp], ep_outer: &[Fq]| {
        build_kagemusha_mint_authorization_transport_pair_v1(
            eq.parameters,
            ep.parameters,
            KagemushaMintTransportDeciderWitnessV1 {
                eq: KagemushaMintTransportParityWitnessV1 {
                    inner_protocol: &eq_protocol,
                    inner_instances: &eq_instances,
                    inner_proof: &eq_proof,
                    inner_history: &eq_native_history,
                    inner_history_fold_proof: eq_fold.proof().as_bytes(),
                    outer_instances: eq_outer,
                },
                ep: KagemushaMintTransportParityWitnessV1 {
                    inner_protocol: &ep_protocol,
                    inner_instances: &ep_instances,
                    inner_proof: &ep_proof,
                    inner_history: &ep_native_history,
                    inner_history_fold_proof: ep_fold.proof().as_bytes(),
                    outer_instances: ep_outer,
                },
            },
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)
    };
    let (eq_outer, ep_outer) = outer_instances([1; 32], [2; 32])?;
    let (_, _, eq_deferred_audit, ep_deferred_audit) = build_outer(&eq_outer, &ep_outer)?;
    let (eq_outer, ep_outer) = outer_instances(eq_deferred_audit, ep_deferred_audit)?;
    let (eq_circuit, ep_circuit, rebuilt_eq, rebuilt_ep) = build_outer(&eq_outer, &ep_outer)?;
    if (rebuilt_eq, rebuilt_ep) != (eq_deferred_audit, ep_deferred_audit) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "mint-authorization transport audit changed while binding public cells".to_owned(),
        ));
    }
    Ok(KagemushaPreparedMintAuthorizationTransportV1 {
        eq_circuit,
        ep_circuit,
        eq_instances: eq_outer,
        ep_instances: ep_outer,
        eq_history: eq_fold.successor().clone(),
        ep_history: ep_fold.successor().clone(),
        eq_deferred_audit,
        ep_deferred_audit,
        semantic_digest,
        recipient_credential_commitment,
        hardware_authorization,
    })
}

#[cfg(feature = "zk-halo2-ipa")]
fn read_eq_recursive_vk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Eq;
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read_checked::<_, KagemushaRecursiveStateEqCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        KAGEMUSHA_HALO2_K_V1,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "recursive state verifying key", error))?;
    ensure_cursor_consumed(
        parity,
        "recursive state verifying key",
        &cursor,
        bytes.len(),
    )?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "recursive state verifying key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
fn read_eq_transport_decider_vk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Eq;
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read_checked::<_, KagemushaTransportDeciderEqCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        KAGEMUSHA_HALO2_K_V1,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "transport-decider verifying key", error))?;
    ensure_cursor_consumed(
        parity,
        "transport-decider verifying key",
        &cursor,
        bytes.len(),
    )?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "transport-decider verifying key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
fn build_mint_authority_generation_pair(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintAuthorityGenerationWitnessV1<'_>,
) -> Result<
    (
        KagemushaMintAuthorityEqCircuitV1,
        KagemushaMintAuthorityEpCircuitV1,
        [u8; 32],
        [u8; 32],
    ),
    KagemushaArtifactGenerationErrorV1,
> {
    let eq_parent_history = witness
        .eq_parent_history
        .to_native()
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_parent_history = witness
        .ep_parent_history
        .to_native()
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    build_kagemusha_mint_authority_pair_v1(
        eq_params,
        ep_params,
        KagemushaMintAuthorityPairWitnessV1 {
            step: witness.step,
            release_id: witness.release_id,
            genesis_roster_id: witness.genesis_roster_id,
            eq_protocol_digest: witness.eq_protocol_digest,
            ep_protocol_digest: witness.ep_protocol_digest,
            eq_deferred_audit: witness.eq_deferred_audit,
            ep_deferred_audit: witness.ep_deferred_audit,
            certificate: witness.certificate,
            eq: KagemushaMintAuthorityParityWitnessV1 {
                parent_protocol: witness.eq_parent_protocol,
                parent_instances: witness.eq_parent_instances,
                parent_proof: witness.eq_parent_proof,
                parent_history: &eq_parent_history,
                parent_fold_proof: witness.eq_parent_fold_proof.as_bytes(),
                successor_history: witness.eq_successor_history.as_bytes(),
            },
            ep: KagemushaMintAuthorityParityWitnessV1 {
                parent_protocol: witness.ep_parent_protocol,
                parent_instances: witness.ep_parent_instances,
                parent_proof: witness.ep_parent_proof,
                parent_history: &ep_parent_history,
                parent_fold_proof: witness.ep_parent_fold_proof.as_bytes(),
                successor_history: witness.ep_successor_history.as_bytes(),
            },
        },
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)
}

#[cfg(feature = "zk-halo2-ipa")]
#[allow(clippy::too_many_arguments)]
fn mint_authority_public_instances<F: KagemushaPoseidonFieldV1>(
    step: KagemushaMintAuthorityStepV1,
    semantic_digest: [u8; 32],
    amount: u128,
    certificate_binding: [u8; 32],
    authority_head: [u8; 32],
    release_id: [u8; 32],
    genesis_roster_id: [u8; 32],
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
    eq_deferred_audit: [u8; 32],
    ep_deferred_audit: [u8; 32],
    proof_binding_digest: [u8; 32],
    history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<Vec<F>, KagemushaArtifactGenerationErrorV1> {
    let mut public = vec![F::from(step as u64)];
    public.extend(crate::zk::kagemusha_v1_poseidon::digest_limbs::<F>(
        semantic_digest,
    ));
    public.push(from_u128::<F>(amount));
    for digest in [
        certificate_binding,
        authority_head,
        release_id,
        genesis_roster_id,
        eq_protocol_digest,
        ep_protocol_digest,
        eq_deferred_audit,
        ep_deferred_audit,
        proof_binding_digest,
    ] {
        public.extend(crate::zk::kagemusha_v1_poseidon::digest_limbs::<F>(digest));
    }
    public.extend(history.chunks_exact(16).map(|chunk| {
        from_u128::<F>(u128::from_le_bytes(
            chunk.try_into().expect("history chunk has sixteen bytes"),
        ))
    }));
    if public.len() != KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1 {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "mint-authority public instance ABI mismatch".to_owned(),
        ));
    }
    Ok(public)
}

#[cfg(feature = "zk-halo2-ipa")]
pub(super) fn read_eq_terminal_authorization_vk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Eq;
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read_checked::<_, KagemushaTerminalAuthorizationEqCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        KAGEMUSHA_HALO2_K_V1,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "terminal-authorization verifying key", error))?;
    ensure_cursor_consumed(
        parity,
        "terminal-authorization verifying key",
        &cursor,
        bytes.len(),
    )?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "terminal-authorization verifying key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
pub(super) fn read_ep_terminal_authorization_vk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Ep;
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read_checked::<_, KagemushaTerminalAuthorizationEpCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        KAGEMUSHA_HALO2_K_V1,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "terminal-authorization verifying key", error))?;
    ensure_cursor_consumed(
        parity,
        "terminal-authorization verifying key",
        &cursor,
        bytes.len(),
    )?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "terminal-authorization verifying key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
pub(super) fn read_eq_commit_wrapper_vk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Eq;
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read_checked::<_, KagemushaCommitWrapperEqCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        KAGEMUSHA_HALO2_K_V1,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "authorization verifying key", error))?;
    ensure_cursor_consumed(parity, "authorization verifying key", &cursor, bytes.len())?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "authorization verifying key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
pub(super) fn read_ep_commit_wrapper_vk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Ep;
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read_checked::<_, KagemushaCommitWrapperEpCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        KAGEMUSHA_HALO2_K_V1,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "authorization verifying key", error))?;
    ensure_cursor_consumed(parity, "authorization verifying key", &cursor, bytes.len())?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "authorization verifying key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
fn read_eq_mint_authorization_vk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Eq;
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read_checked::<_, KagemushaMintAuthorizationTransportEqCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        KAGEMUSHA_HALO2_K_V1,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "mint-authorization verifying key", error))?;
    ensure_cursor_consumed(
        parity,
        "mint-authorization verifying key",
        &cursor,
        bytes.len(),
    )?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "mint-authorization verifying key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
fn read_eq_inner_mint_authorization_vk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Eq;
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read_checked::<_, KagemushaMintAuthorizationEqCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        KAGEMUSHA_HALO2_K_V1,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "inner mint-authorization verifying key", error))?;
    ensure_cursor_consumed(
        parity,
        "inner mint-authorization verifying key",
        &cursor,
        bytes.len(),
    )?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "inner mint-authorization verifying key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
fn read_ep_mint_authorization_vk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Ep;
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read_checked::<_, KagemushaMintAuthorizationTransportEpCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        KAGEMUSHA_HALO2_K_V1,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "mint-authorization verifying key", error))?;
    ensure_cursor_consumed(
        parity,
        "mint-authorization verifying key",
        &cursor,
        bytes.len(),
    )?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "mint-authorization verifying key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
fn read_ep_inner_mint_authorization_vk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Ep;
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read_checked::<_, KagemushaMintAuthorizationEpCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        KAGEMUSHA_HALO2_K_V1,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "inner mint-authorization verifying key", error))?;
    ensure_cursor_consumed(
        parity,
        "inner mint-authorization verifying key",
        &cursor,
        bytes.len(),
    )?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "inner mint-authorization verifying key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
fn read_eq_mint_vk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Eq;
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read_checked::<_, KagemushaMintAuthorityTransportEqCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        KAGEMUSHA_HALO2_K_V1,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "mint-authority verifying key", error))?;
    ensure_cursor_consumed(parity, "mint-authority verifying key", &cursor, bytes.len())?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "mint-authority verifying key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
fn read_ep_mint_vk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Ep;
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read_checked::<_, KagemushaMintAuthorityTransportEpCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        KAGEMUSHA_HALO2_K_V1,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "mint-authority verifying key", error))?;
    ensure_cursor_consumed(parity, "mint-authority verifying key", &cursor, bytes.len())?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "mint-authority verifying key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
fn read_ep_recursive_vk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Ep;
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read_checked::<_, KagemushaRecursiveStateEpCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        KAGEMUSHA_HALO2_K_V1,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "recursive state verifying key", error))?;
    ensure_cursor_consumed(
        parity,
        "recursive state verifying key",
        &cursor,
        bytes.len(),
    )?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "recursive state verifying key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
fn read_ep_transport_decider_vk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Ep;
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read_checked::<_, KagemushaTransportDeciderEpCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        KAGEMUSHA_HALO2_K_V1,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "transport-decider verifying key", error))?;
    ensure_cursor_consumed(
        parity,
        "transport-decider verifying key",
        &cursor,
        bytes.len(),
    )?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "transport-decider verifying key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
fn recursive_public_instances<F: KagemushaPoseidonFieldV1>(
    state: &KagemushaStateRelationWitnessV1,
    successor_history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<Vec<F>, KagemushaArtifactGenerationErrorV1> {
    let mut instances = state
        .public_instances::<F>()
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    instances.extend(successor_history.chunks_exact(16).map(|chunk| {
        from_u128::<F>(u128::from_le_bytes(
            chunk
                .try_into()
                .expect("fixed history chunks are sixteen bytes"),
        ))
    }));
    if instances.len() != recursive_public_instance_count() {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "recursive state public instance ABI mismatch".to_owned(),
        ));
    }
    Ok(instances)
}

#[cfg(feature = "zk-halo2-ipa")]
fn terminal_authorization_public_instances<F: KagemushaPoseidonFieldV1>(
    public: &KagemushaTerminalAuthorizationPublicInputsV1,
    successor_history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<Vec<F>, KagemushaArtifactGenerationErrorV1> {
    let mut instances = public
        .public_prefix::<F>()
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    instances.extend(successor_history.chunks_exact(16).map(|chunk| {
        from_u128::<F>(u128::from_le_bytes(
            chunk
                .try_into()
                .expect("fixed history chunks are sixteen bytes"),
        ))
    }));
    if instances.len() != TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1 {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "terminal-authorization public instance ABI mismatch".to_owned(),
        ));
    }
    Ok(instances)
}

/// The byte stream emitted directly by Axiom Halo2's IPA prover.
///
/// This stream ends with the final coefficient and blinding scalars. It must
/// never be passed to the recursive verifier, which additionally requires the
/// transcript-derived folded SRS generator.
#[cfg(feature = "zk-halo2-ipa")]
pub(super) struct KagemushaRawHalo2IpaProofV1(Vec<u8>);

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaRawHalo2IpaProofV1 {
    pub(super) fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

/// Canonical recursive-verifier IPA proof: raw Halo2 bytes followed by the
/// final folded SRS generator encoded as one compressed Pasta point.
#[cfg(feature = "zk-halo2-ipa")]
struct KagemushaAugmentedIpaProofV1(Vec<u8>);

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaAugmentedIpaProofV1 {
    fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

/// Halo2 verification strategy which exposes the folded SRS generator.
///
/// Axiom's ordinary verifier normally consumes the round challenges by
/// expanding them against the complete SRS. The recursive verifier cannot
/// carry that SRS, so its BGH19 parser instead reads this derived point and
/// emits an accumulator whose reciprocal audit proves the same expansion.
#[cfg(feature = "zk-halo2-ipa")]
struct KagemushaFoldedGeneratorStrategyV1<'params, C: CurveAffine> {
    parameters: &'params ParamsIPA<C>,
}

#[cfg(feature = "zk-halo2-ipa")]
impl<'params, C: CurveAffine>
    VerificationStrategy<'params, IPACommitmentScheme<C>, VerifierIPA<'params, C>>
    for KagemushaFoldedGeneratorStrategyV1<'params, C>
{
    type Output = C;

    fn new(parameters: &'params ParamsIPA<C>) -> Self {
        Self { parameters }
    }

    fn process(
        self,
        verifier: impl FnOnce(
            MSMIPA<'params, C>,
        ) -> Result<GuardIPA<'params, C>, halo2_proofs::plonk::Error>,
    ) -> Result<Self::Output, halo2_proofs::plonk::Error> {
        let guard = verifier(MSMIPA::new(self.parameters))?;
        let folded_generator = guard.compute_g();
        let (derived_generator_check, _) = guard.use_g(folded_generator);
        if !derived_generator_check.check() {
            return Err(halo2_proofs::plonk::Error::ConstraintSystemFailure);
        }
        Ok(folded_generator)
    }

    fn finalize(self) -> bool {
        true
    }
}

/// Convert a raw Axiom Halo2 IPA proof into the sole proof shape accepted by
/// KAGEMUSHA's recursive verifier.
///
/// The raw proof is replayed through Halo2's native verifier solely to derive
/// the IPA round challenges from the exact public inputs and transcript. Only
/// then is the folded SRS generator appended after `c`/`f`. Monetary authority
/// does not depend on this host derivation: the recursive verifier checks the
/// opening equation and carries the generator/challenges into the reciprocal
/// terminal audit.
#[cfg(feature = "zk-halo2-ipa")]
pub(super) fn augment_halo2_ipa_proof_v1<C>(
    parameters: &ParamsIPA<C>,
    verifying_key: &VerifyingKey<C>,
    raw: KagemushaRawHalo2IpaProofV1,
    instances: &[C::ScalarExt],
) -> Result<Vec<u8>, String>
where
    C: CurveAffine,
    C::ScalarExt: FromUniformBytes<64> + WithSmallOrderMulGroup<3>,
{
    type Transcript<C, S> = PoseidonTranscript<
        C,
        NativeLoader,
        S,
        KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
        KAGEMUSHA_IPA_POSEIDON_RATE_V1,
        KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
        KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    >;

    let columns: [&[C::ScalarExt]; 1] = [instances];
    let proofs_instances: [&[&[C::ScalarExt]]; 1] = [&columns];
    let mut cursor = Cursor::new(raw.0.as_slice());
    let mut transcript =
        Transcript::<C, _>::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(&mut cursor);
    let folded_generator = verify_proof::<
        IPACommitmentScheme<C>,
        VerifierIPA<'_, C>,
        ChallengeScalar<C>,
        _,
        KagemushaFoldedGeneratorStrategyV1<'_, C>,
    >(
        parameters,
        verifying_key,
        KagemushaFoldedGeneratorStrategyV1::new(parameters),
        &proofs_instances,
        &mut transcript,
    )
    .map_err(|error| format!("raw Halo2 IPA proof cannot derive folded generator: {error}"))?;
    drop(transcript);
    let consumed = cursor.position();
    drop(cursor);
    if consumed != raw.0.len() as u64 {
        return Err("raw Halo2 IPA proof contains trailing bytes".to_owned());
    }

    let raw_len = raw.0.len();
    let mut augmented = raw.0;
    augmented.extend_from_slice(folded_generator.to_bytes().as_ref());
    if augmented.len() != raw_len + 32 {
        return Err("folded Pasta generator did not add exactly 32 bytes".to_owned());
    }
    Ok(KagemushaAugmentedIpaProofV1(augmented).into_bytes())
}

/// These labels are part of deterministic recovery, not diagnostic prose. A prepared operation
/// reuses its secret only with its immutable witness; each proof role, parity, key, and statement
/// gets a distinct stream. No public digest or opaque sealed blob substitutes for the secret.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Copy, Debug)]
enum KagemushaProofRecoveryPhaseV1 {
    StateCarrier,
    StateTransport,
    TerminalAuthorization,
    CommitWrapper,
    MintAuthorization,
    MintAuthorizationTransport,
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaProofRecoveryPhaseV1 {
    const fn label(self) -> &'static str {
        match self {
            Self::StateCarrier => "iroha:kagemusha:v1:proof-recovery:state-carrier",
            Self::StateTransport => "iroha:kagemusha:v1:proof-recovery:state-transport",
            Self::TerminalAuthorization => {
                "iroha:kagemusha:v1:proof-recovery:terminal-authorization"
            }
            Self::CommitWrapper => "iroha:kagemusha:v1:proof-recovery:commit-wrapper",
            Self::MintAuthorization => "iroha:kagemusha:v1:proof-recovery:mint-authorization",
            Self::MintAuthorizationTransport => {
                "iroha:kagemusha:v1:proof-recovery:mint-authorization-transport"
            }
        }
    }
}

#[cfg(feature = "zk-halo2-ipa")]
fn proof_recovery_context_v1<F: ff::PrimeField>(
    parity: KagemushaPastaParityV1,
    k: u32,
    verifying_key: &[u8],
    instances: &[F],
) -> [u8; 32] {
    let mut context = Sha256::new();
    context.update(b"iroha:kagemusha:v1:proof-recovery-context\0");
    context.update([match parity {
        KagemushaPastaParityV1::Eq => 0,
        KagemushaPastaParityV1::Ep => 1,
    }]);
    context.update(k.to_le_bytes());
    context.update((verifying_key.len() as u64).to_le_bytes());
    context.update(verifying_key);
    // These proof helpers accept exactly one column. Length and order are both authenticated.
    context.update(1_u64.to_le_bytes());
    context.update((instances.len() as u64).to_le_bytes());
    for instance in instances {
        context.update(instance.to_repr().as_ref());
    }
    context.finalize().into()
}

#[cfg(feature = "zk-halo2-ipa")]
fn create_eq_proof_with_key_v1<C>(
    parameters: &ParamsIPA<EqAffine>,
    proving_key: &ProvingKey<EqAffine>,
    circuit: C,
    instances: &[Fp],
    phase: KagemushaProofRecoveryPhaseV1,
    recovery_seed: &KagemushaRecoverySeedV1,
) -> Result<Vec<u8>, KagemushaArtifactGenerationErrorV1>
where
    C: halo2_proofs::plonk::Circuit<Fp>,
{
    type Transcript<S> = PoseidonTranscript<
        EqAffine,
        NativeLoader,
        S,
        KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
        KAGEMUSHA_IPA_POSEIDON_RATE_V1,
        KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
        KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    >;
    let columns: [&[Fp]; 1] = [instances];
    let proofs_instances: [&[&[Fp]]; 1] = [&columns];
    let label = phase.label();
    let context = proof_recovery_context_v1(
        KagemushaPastaParityV1::Eq,
        parameters.k(),
        &proving_key.get_vk().to_bytes(SerdeFormat::Processed),
        instances,
    );
    let rng = recovery_seed
        .rng(label.as_bytes(), &context)
        .map_err(
            |error| KagemushaArtifactGenerationErrorV1::ProofGeneration {
                parity: KagemushaPastaParityV1::Eq,
                reason: format!("{label}: {error}"),
            },
        )?;
    let mut transcript = Transcript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(Vec::<u8>::new());
    create_proof::<
        IPACommitmentScheme<EqAffine>,
        ProverIPA<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
        _,
    >(
        parameters,
        proving_key,
        &[circuit],
        &proofs_instances,
        rng,
        &mut transcript,
    )
    .map_err(
        |error| KagemushaArtifactGenerationErrorV1::ProofGeneration {
            parity: KagemushaPastaParityV1::Eq,
            reason: format!("{label}: {error}"),
        },
    )?;
    augment_halo2_ipa_proof_v1(
        parameters,
        proving_key.get_vk(),
        KagemushaRawHalo2IpaProofV1::new(transcript.finalize()),
        instances,
    )
    .map_err(
        |reason| KagemushaArtifactGenerationErrorV1::ProofGeneration {
            parity: KagemushaPastaParityV1::Eq,
            reason: format!("{label}: {reason}"),
        },
    )
}

#[cfg(feature = "zk-halo2-ipa")]
fn create_ep_proof_with_key_v1<C>(
    parameters: &ParamsIPA<EpAffine>,
    proving_key: &ProvingKey<EpAffine>,
    circuit: C,
    instances: &[Fq],
    phase: KagemushaProofRecoveryPhaseV1,
    recovery_seed: &KagemushaRecoverySeedV1,
) -> Result<Vec<u8>, KagemushaArtifactGenerationErrorV1>
where
    C: halo2_proofs::plonk::Circuit<Fq>,
{
    type Transcript<S> = PoseidonTranscript<
        EpAffine,
        NativeLoader,
        S,
        KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
        KAGEMUSHA_IPA_POSEIDON_RATE_V1,
        KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
        KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    >;
    let columns: [&[Fq]; 1] = [instances];
    let proofs_instances: [&[&[Fq]]; 1] = [&columns];
    let label = phase.label();
    let context = proof_recovery_context_v1(
        KagemushaPastaParityV1::Ep,
        parameters.k(),
        &proving_key.get_vk().to_bytes(SerdeFormat::Processed),
        instances,
    );
    let rng = recovery_seed
        .rng(label.as_bytes(), &context)
        .map_err(
            |error| KagemushaArtifactGenerationErrorV1::ProofGeneration {
                parity: KagemushaPastaParityV1::Ep,
                reason: format!("{label}: {error}"),
            },
        )?;
    let mut transcript = Transcript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(Vec::<u8>::new());
    create_proof::<
        IPACommitmentScheme<EpAffine>,
        ProverIPA<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
        _,
    >(
        parameters,
        proving_key,
        &[circuit],
        &proofs_instances,
        rng,
        &mut transcript,
    )
    .map_err(
        |error| KagemushaArtifactGenerationErrorV1::ProofGeneration {
            parity: KagemushaPastaParityV1::Ep,
            reason: format!("{label}: {error}"),
        },
    )?;
    augment_halo2_ipa_proof_v1(
        parameters,
        proving_key.get_vk(),
        KagemushaRawHalo2IpaProofV1::new(transcript.finalize()),
        instances,
    )
    .map_err(
        |reason| KagemushaArtifactGenerationErrorV1::ProofGeneration {
            parity: KagemushaPastaParityV1::Ep,
            reason: format!("{label}: {reason}"),
        },
    )
}

#[cfg(feature = "zk-halo2-ipa")]
fn create_mint_eq_proof<C: halo2_proofs::plonk::Circuit<Fp>>(
    parameters: &ParamsIPA<EqAffine>,
    proving_key: &ProvingKey<EqAffine>,
    circuit: C,
    instances: &[Fp],
) -> Result<Vec<u8>, KagemushaArtifactGenerationErrorV1> {
    type Transcript<S> = PoseidonTranscript<
        EqAffine,
        NativeLoader,
        S,
        KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
        KAGEMUSHA_IPA_POSEIDON_RATE_V1,
        KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
        KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    >;
    let columns: [&[Fp]; 1] = [instances];
    let proofs_instances: [&[&[Fp]]; 1] = [&columns];
    let mut transcript = Transcript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(Vec::<u8>::new());
    create_proof::<
        IPACommitmentScheme<EqAffine>,
        ProverIPA<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
        _,
    >(
        parameters,
        proving_key,
        &[circuit],
        &proofs_instances,
        OsRng,
        &mut transcript,
    )
    .map_err(
        |error| KagemushaArtifactGenerationErrorV1::ProofGeneration {
            parity: KagemushaPastaParityV1::Eq,
            reason: error.to_string(),
        },
    )?;
    augment_halo2_ipa_proof_v1(
        parameters,
        proving_key.get_vk(),
        KagemushaRawHalo2IpaProofV1::new(transcript.finalize()),
        instances,
    )
    .map_err(
        |reason| KagemushaArtifactGenerationErrorV1::ProofGeneration {
            parity: KagemushaPastaParityV1::Eq,
            reason,
        },
    )
}

#[cfg(feature = "zk-halo2-ipa")]
fn create_mint_ep_proof<C: halo2_proofs::plonk::Circuit<Fq>>(
    parameters: &ParamsIPA<EpAffine>,
    proving_key: &ProvingKey<EpAffine>,
    circuit: C,
    instances: &[Fq],
) -> Result<Vec<u8>, KagemushaArtifactGenerationErrorV1> {
    type Transcript<S> = PoseidonTranscript<
        EpAffine,
        NativeLoader,
        S,
        KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
        KAGEMUSHA_IPA_POSEIDON_RATE_V1,
        KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
        KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    >;
    let columns: [&[Fq]; 1] = [instances];
    let proofs_instances: [&[&[Fq]]; 1] = [&columns];
    let mut transcript = Transcript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(Vec::<u8>::new());
    create_proof::<
        IPACommitmentScheme<EpAffine>,
        ProverIPA<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
        _,
    >(
        parameters,
        proving_key,
        &[circuit],
        &proofs_instances,
        OsRng,
        &mut transcript,
    )
    .map_err(
        |error| KagemushaArtifactGenerationErrorV1::ProofGeneration {
            parity: KagemushaPastaParityV1::Ep,
            reason: error.to_string(),
        },
    )?;
    augment_halo2_ipa_proof_v1(
        parameters,
        proving_key.get_vk(),
        KagemushaRawHalo2IpaProofV1::new(transcript.finalize()),
        instances,
    )
    .map_err(
        |reason| KagemushaArtifactGenerationErrorV1::ProofGeneration {
            parity: KagemushaPastaParityV1::Ep,
            reason,
        },
    )
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_recursive_profile(
    parity: KagemushaPastaParityV1,
    params: &BaseCircuitParams,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    super::native_backend::validate_kagemusha_base_circuit_params_v1(params)
        .map_err(|_| KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(parity))
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_terminal_authorization_profile(
    parity: KagemushaPastaParityV1,
    params: &BaseCircuitParams,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    validate_recursive_profile(parity, params)
}

#[cfg(feature = "zk-halo2-ipa")]
fn same_base_params(left: &BaseCircuitParams, right: &BaseCircuitParams) -> bool {
    left.k == right.k
        && left.num_advice_per_phase == right.num_advice_per_phase
        && left.num_fixed == right.num_fixed
        && left.num_lookup_advice_per_phase == right.num_lookup_advice_per_phase
        && left.lookup_bits == right.lookup_bits
        && left.num_instance_columns == right.num_instance_columns
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_recursive_proof_length(
    parity: KagemushaPastaParityV1,
    proof: &[u8],
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    if proof.is_empty() || proof.len() > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1 {
        return Err(KagemushaArtifactGenerationErrorV1::InvalidLength {
            parity,
            kind: "recursive state proof",
            actual: u64::try_from(proof.len()).unwrap_or(u64::MAX),
        });
    }
    Ok(())
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_paired_proof_length(
    parity: KagemushaPastaParityV1,
    proof: &[u8],
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    if proof.is_empty() || proof.len() > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1 {
        return Err(KagemushaArtifactGenerationErrorV1::InvalidLength {
            parity,
            kind: "paired recursive proof",
            actual: u64::try_from(proof.len()).unwrap_or(u64::MAX),
        });
    }
    Ok(())
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_internal_recursive_proof_length<C>(
    parity: KagemushaPastaParityV1,
    kind: &'static str,
    protocol: &PlonkProtocol<C>,
    proof: &[u8],
) -> Result<(), KagemushaArtifactGenerationErrorV1>
where
    C: snark_verifier::util::arithmetic::CurveAffine,
{
    let expected = ordinary_ipa_proof_profile_v1(protocol)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?
        .byte_len;
    if proof.len() != expected {
        return Err(KagemushaArtifactGenerationErrorV1::InvalidLength {
            parity,
            kind,
            actual: u64::try_from(proof.len()).unwrap_or(u64::MAX),
        });
    }
    Ok(())
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_transport_protocol_profile<C>(
    parity: KagemushaPastaParityV1,
    kind: &'static str,
    protocol: &PlonkProtocol<C>,
) -> Result<(), KagemushaArtifactGenerationErrorV1>
where
    C: snark_verifier::util::arithmetic::CurveAffine,
{
    let profile = ordinary_ipa_proof_profile_v1(protocol)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    validate_transport_proof_profile(parity, kind, profile)
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_transport_proof_profile(
    parity: KagemushaPastaParityV1,
    kind: &'static str,
    profile: KagemushaOrdinaryProofProfileV1,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    if profile.byte_len > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1 {
        return Err(
            KagemushaArtifactGenerationErrorV1::TransportProofProfileTooLarge {
                parity,
                kind,
                witness_commitments: u64::try_from(profile.witness_commitments).unwrap_or(u64::MAX),
                quotient_commitments: u64::try_from(profile.quotient_commitments)
                    .unwrap_or(u64::MAX),
                evaluations: u64::try_from(profile.evaluations).unwrap_or(u64::MAX),
                bgh19_rotation_sets: u64::try_from(profile.bgh19_rotation_sets).unwrap_or(u64::MAX),
                actual: u64::try_from(profile.byte_len).unwrap_or(u64::MAX),
                maximum: u64::try_from(KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1)
                    .expect("fixed proof bound fits u64"),
            },
        );
    }
    Ok(())
}

#[cfg(feature = "zk-halo2-ipa")]
const fn recursive_public_instance_count() -> usize {
    PUBLIC_INSTANCE_COUNT + super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1 / 16
}

fn ensure_embedded_vk<C>(
    parity: KagemushaPastaParityV1,
    proving_key: &ProvingKey<C>,
    standalone_vk_bytes: &[u8],
) -> Result<(), KagemushaArtifactGenerationErrorV1>
where
    C: halo2_proofs::halo2curves::CurveAffine + halo2_proofs::SerdeCurveAffine,
    C::Scalar: halo2_proofs::SerdePrimeField + ff::FromUniformBytes<64>,
{
    if proving_key.get_vk().to_bytes(SerdeFormat::Processed) != standalone_vk_bytes {
        return Err(key_decode_message(
            parity,
            "proving key",
            "embedded verifying key differs from the authenticated standalone key",
        ));
    }
    Ok(())
}

/// Hash canonical preprocessing directly into the authenticated length bound. This never
/// allocates a second key-sized buffer, and every nested writer propagates its sink errors.
#[cfg(feature = "zk-halo2-ipa")]
struct CanonicalArtifactDigestWriterV1 {
    digest: Sha256,
    written: u64,
    maximum: u64,
}

#[cfg(feature = "zk-halo2-ipa")]
impl CanonicalArtifactDigestWriterV1 {
    fn new(maximum: u64) -> Self {
        Self {
            digest: Sha256::new(),
            written: 0,
            maximum,
        }
    }

    fn matches(self, binding: KagemushaArtifactBindingV1) -> bool {
        self.written == binding.byte_len
            && <[u8; 32]>::from(self.digest.finalize()) == binding.sha256
    }
}

#[cfg(feature = "zk-halo2-ipa")]
impl io::Write for CanonicalArtifactDigestWriterV1 {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = u64::try_from(bytes.len())
            .map_err(|_| io::Error::other("canonical artifact byte count overflow"))?;
        let next = self
            .written
            .checked_add(count)
            .filter(|next| *next <= self.maximum)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "canonical artifact exceeds authenticated byte length",
                )
            })?;
        self.digest.update(bytes);
        self.written = next;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Decode a shape-checked key and compare its streamed canonical encoding to the manifest.
/// The caller must separately authenticate and fully consume the input stream before exposing
/// this return value; production uses `KagemushaAuthenticatedArtifactSetV1::read_verified`.
#[cfg(feature = "zk-halo2-ipa")]
fn read_canonical_proving_key_v1<C, ConcreteCircuit>(
    mut reader: &mut dyn io::Read,
    binding: KagemushaArtifactBindingV1,
    parity: KagemushaPastaParityV1,
    expected_k: u32,
    circuit_params: ConcreteCircuit::Params,
) -> Result<ProvingKey<C>, KagemushaArtifactGenerationErrorV1>
where
    C: CurveAffine + halo2_proofs::SerdeCurveAffine,
    C::Scalar: halo2_proofs::SerdePrimeField + ff::FromUniformBytes<64>,
    ConcreteCircuit: halo2_proofs::plonk::Circuit<C::Scalar>,
{
    let key = ProvingKey::read_checked::<_, ConcreteCircuit>(
        &mut reader,
        SerdeFormat::Processed,
        expected_k,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "proving key", error))?;
    let mut canonical = CanonicalArtifactDigestWriterV1::new(binding.byte_len);
    key.write_streaming(&mut canonical, SerdeFormat::Processed)
        .map_err(|error| key_decode_error(parity, "proving key canonical encoding", error))?;
    if !canonical.matches(binding) {
        return Err(key_decode_message(
            parity,
            "proving key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
fn load_authenticated_proving_key_v1<C, ConcreteCircuit, R>(
    artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
    role: KagemushaArtifactRoleV1,
    parity: KagemushaPastaParityV1,
    circuit_params: ConcreteCircuit::Params,
) -> Result<ProvingKey<C>, KagemushaArtifactGenerationErrorV1>
where
    C: CurveAffine + halo2_proofs::SerdeCurveAffine,
    C::Scalar: halo2_proofs::SerdePrimeField + ff::FromUniformBytes<64>,
    ConcreteCircuit: halo2_proofs::plonk::Circuit<C::Scalar>,
    R: KagemushaArtifactByteResolverV1,
{
    let descriptor = super::KagemushaArtifactDescriptorV1::for_role(role);
    if descriptor.kind != super::KagemushaArtifactKindV1::ProvingKey || descriptor.parity != parity
    {
        return Err(KagemushaArtifactErrorV1::InvalidBinding(role).into());
    }
    let binding = artifacts.binding(role);
    artifacts.read_verified(role, |reader| {
        read_canonical_proving_key_v1::<C, ConcreteCircuit>(
            reader,
            binding,
            parity,
            KAGEMUSHA_HALO2_K_V1,
            circuit_params,
        )
    })
}

fn ensure_cursor_consumed<T>(
    parity: KagemushaPastaParityV1,
    kind: &'static str,
    cursor: &Cursor<T>,
    expected: usize,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    if cursor.position() != u64::try_from(expected).unwrap_or(u64::MAX) {
        return Err(key_decode_message(parity, kind, "trailing bytes"));
    }
    Ok(())
}

fn key_decode_error(
    parity: KagemushaPastaParityV1,
    kind: &'static str,
    error: impl core::fmt::Display,
) -> KagemushaArtifactGenerationErrorV1 {
    KagemushaArtifactGenerationErrorV1::KeyDecode {
        parity,
        kind,
        reason: error.to_string(),
    }
}

fn key_decode_message(
    parity: KagemushaPastaParityV1,
    kind: &'static str,
    reason: &'static str,
) -> KagemushaArtifactGenerationErrorV1 {
    KagemushaArtifactGenerationErrorV1::KeyDecode {
        parity,
        kind,
        reason: reason.to_owned(),
    }
}

fn build_generated<C>(
    parity: KagemushaPastaParityV1,
    params: &ParamsIPA<C>,
    proving_key: &ProvingKey<C>,
    verifying_key: &VerifyingKey<C>,
) -> Result<KagemushaGeneratedOperationArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    C: halo2_proofs::halo2curves::CurveAffine + halo2_proofs::SerdeCurveAffine,
    C::Scalar: halo2_proofs::SerdePrimeField + ff::FromUniformBytes<64>,
{
    let mut parameter_bytes = Vec::new();
    params.write(&mut parameter_bytes).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity,
            kind: "parameters",
            reason: error.to_string(),
        }
    })?;
    let proving_key_bytes = proving_key.to_bytes(SerdeFormat::Processed);
    let verifying_key_bytes = verifying_key.to_bytes(SerdeFormat::Processed);
    validate_length(
        parity,
        "parameters",
        parameter_bytes.len(),
        KAGEMUSHA_PARAMS_BYTES_V1,
        true,
    )?;
    validate_length(
        parity,
        "proving key",
        proving_key_bytes.len(),
        KAGEMUSHA_STATE_PROVING_KEY_MAX_BYTES_V1,
        false,
    )?;
    validate_length(
        parity,
        "verifying key",
        verifying_key_bytes.len(),
        KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1,
        false,
    )?;
    Ok(KagemushaGeneratedOperationArtifactsV1 {
        parity,
        parameters: Arc::from(parameter_bytes),
        proving_key: Arc::from(proving_key_bytes),
        verifying_key: Arc::from(verifying_key_bytes),
    })
}

#[cfg(feature = "zk-halo2-ipa")]
fn build_generated_mint_parity<C>(
    parity: KagemushaPastaParityV1,
    params: &ParamsIPA<C>,
    proving_key: &ProvingKey<C>,
    verifying_key: &VerifyingKey<C>,
) -> Result<(Arc<[u8]>, Arc<[u8]>, Arc<[u8]>), KagemushaArtifactGenerationErrorV1>
where
    C: halo2_proofs::halo2curves::CurveAffine + halo2_proofs::SerdeCurveAffine,
    C::Scalar: halo2_proofs::SerdePrimeField + ff::FromUniformBytes<64>,
{
    let mut parameter_bytes = Vec::new();
    params.write(&mut parameter_bytes).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity,
            kind: "parameters",
            reason: error.to_string(),
        }
    })?;
    let proving_key_bytes = proving_key.to_bytes(SerdeFormat::Processed);
    let verifying_key_bytes = verifying_key.to_bytes(SerdeFormat::Processed);
    validate_length(
        parity,
        "parameters",
        parameter_bytes.len(),
        KAGEMUSHA_PARAMS_BYTES_V1,
        true,
    )?;
    validate_length(
        parity,
        "mint-authority proving key",
        proving_key_bytes.len(),
        KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1,
        false,
    )?;
    validate_length(
        parity,
        "mint-authority verifying key",
        verifying_key_bytes.len(),
        KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1,
        false,
    )?;
    Ok((
        Arc::from(parameter_bytes),
        Arc::from(proving_key_bytes),
        Arc::from(verifying_key_bytes),
    ))
}

#[cfg(feature = "zk-halo2-ipa")]
fn build_generated_helper_parity<C>(
    parity: KagemushaPastaParityV1,
    label: &'static str,
    params: &ParamsIPA<C>,
    proving_key: &ProvingKey<C>,
    verifying_key: &VerifyingKey<C>,
) -> Result<(Arc<[u8]>, Arc<[u8]>, Arc<[u8]>), KagemushaArtifactGenerationErrorV1>
where
    C: halo2_proofs::halo2curves::CurveAffine + halo2_proofs::SerdeCurveAffine,
    C::Scalar: halo2_proofs::SerdePrimeField + ff::FromUniformBytes<64>,
{
    let mut parameter_bytes = Vec::new();
    params.write(&mut parameter_bytes).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity,
            kind: "parameters",
            reason: error.to_string(),
        }
    })?;
    let proving_key_bytes = proving_key.to_bytes(SerdeFormat::Processed);
    let verifying_key_bytes = verifying_key.to_bytes(SerdeFormat::Processed);
    validate_length(
        parity,
        "parameters",
        parameter_bytes.len(),
        KAGEMUSHA_PARAMS_BYTES_V1,
        true,
    )?;
    validate_length(
        parity,
        label,
        proving_key_bytes.len(),
        KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1,
        false,
    )?;
    validate_length(
        parity,
        "helper verifying key",
        verifying_key_bytes.len(),
        KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1,
        false,
    )?;
    Ok((
        Arc::from(parameter_bytes),
        Arc::from(proving_key_bytes),
        Arc::from(verifying_key_bytes),
    ))
}

fn validate_length(
    parity: KagemushaPastaParityV1,
    kind: &'static str,
    actual: usize,
    limit: u64,
    exact: bool,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    let actual = u64::try_from(actual).unwrap_or(u64::MAX);
    if actual == 0 || (exact && actual != limit) || (!exact && actual > limit) {
        return Err(KagemushaArtifactGenerationErrorV1::InvalidLength {
            parity,
            kind,
            actual,
        });
    }
    Ok(())
}

fn binding(role: KagemushaArtifactRoleV1, bytes: &[u8]) -> KagemushaArtifactBindingV1 {
    KagemushaArtifactBindingV1 {
        role,
        sha256: Sha256::digest(bytes).into(),
        byte_len: u64::try_from(bytes.len()).expect("bounded generated artifact fits u64"),
    }
}

const _: () = {
    assert!(KAGEMUSHA_HALO2_K_V1 == 16);
    assert!(KAGEMUSHA_PARAMS_BYTES_V1 == 4_194_372);
};

#[cfg(test)]
mod tests {
    #[cfg(feature = "zk-halo2-ipa")]
    use std::io::Write as _;

    use super::*;

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn canonical_artifact_digest_checks_chunks_length_digest_and_sink_bound() {
        let bytes = b"canonical artifact chunks";
        let binding = binding(KagemushaArtifactRoleV1::StatePkEq, bytes);
        let mut writer = CanonicalArtifactDigestWriterV1::new(binding.byte_len);
        for chunk in bytes.chunks(3) {
            writer.write_all(chunk).expect("bounded chunk");
        }
        writer.write_all(&[]).expect("empty chunk");
        writer.flush().expect("digest flush");
        assert_eq!(writer.written, binding.byte_len);
        assert_eq!(
            writer
                .write_all(b"extra")
                .expect_err("reject extra bytes")
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(writer.written, binding.byte_len);
        assert!(
            writer.matches(binding),
            "a rejected write cannot alter the digest"
        );

        let mut short = CanonicalArtifactDigestWriterV1::new(binding.byte_len);
        short.write_all(&bytes[..bytes.len() - 1]).expect("prefix");
        assert!(!short.matches(binding));
        let mut changed = CanonicalArtifactDigestWriterV1::new(binding.byte_len);
        changed.write_all(bytes).expect("full bytes");
        let mut different_binding = binding;
        different_binding.sha256[0] ^= 1;
        assert!(!changed.matches(different_binding));
        let mut overflow = CanonicalArtifactDigestWriterV1::new(u64::MAX);
        overflow.written = u64::MAX;
        assert!(overflow.write_all(b"x").is_err());
        assert_eq!(overflow.written, u64::MAX);
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn proof_recovery_stream_binds_phase_parity_key_and_ordered_instances() {
        use rand_core_06::RngCore as _;

        // Deliberately public test entropy; never a production recovery seed.
        let seed = KagemushaRecoverySeedV1::from_unsealed([0xA7; 32]).expect("test seed");
        let other_seed = KagemushaRecoverySeedV1::from_unsealed([0xA8; 32]).expect("test seed");
        let instances = [Fp::from(7), Fp::from(9)];
        let context = proof_recovery_context_v1(
            KagemushaPastaParityV1::Eq,
            16,
            b"test processed key",
            &instances,
        );
        let stream = |seed: &KagemushaRecoverySeedV1,
                      phase: KagemushaProofRecoveryPhaseV1,
                      context: &[u8; 32]| {
            let mut result = [0; 64];
            seed.rng(phase.label().as_bytes(), context)
                .expect("recovery stream")
                .fill_bytes(&mut result);
            result
        };
        let expected = stream(&seed, KagemushaProofRecoveryPhaseV1::StateCarrier, &context);
        assert_eq!(
            expected,
            stream(&seed, KagemushaProofRecoveryPhaseV1::StateCarrier, &context)
        );
        assert_ne!(
            expected,
            stream(
                &other_seed,
                KagemushaProofRecoveryPhaseV1::StateCarrier,
                &context
            )
        );
        let phases = [
            KagemushaProofRecoveryPhaseV1::StateCarrier,
            KagemushaProofRecoveryPhaseV1::StateTransport,
            KagemushaProofRecoveryPhaseV1::TerminalAuthorization,
            KagemushaProofRecoveryPhaseV1::CommitWrapper,
            KagemushaProofRecoveryPhaseV1::MintAuthorization,
            KagemushaProofRecoveryPhaseV1::MintAuthorizationTransport,
        ];
        for (index, phase) in phases.iter().enumerate() {
            for other in &phases[index + 1..] {
                assert_ne!(
                    stream(&seed, *phase, &context),
                    stream(&seed, *other, &context)
                );
            }
        }
        for changed in [
            proof_recovery_context_v1(
                KagemushaPastaParityV1::Ep,
                16,
                b"test processed key",
                &instances,
            ),
            proof_recovery_context_v1(
                KagemushaPastaParityV1::Eq,
                15,
                b"test processed key",
                &instances,
            ),
            proof_recovery_context_v1(
                KagemushaPastaParityV1::Eq,
                16,
                b"other processed key",
                &instances,
            ),
            proof_recovery_context_v1(
                KagemushaPastaParityV1::Eq,
                16,
                b"test processed key",
                &[instances[1], instances[0]],
            ),
            proof_recovery_context_v1(
                KagemushaPastaParityV1::Eq,
                16,
                b"test processed key",
                &instances[..1],
            ),
            proof_recovery_context_v1(
                KagemushaPastaParityV1::Eq,
                16,
                b"test processed key",
                &[instances[0], Fp::from(10)],
            ),
        ] {
            assert_ne!(context, changed);
            assert_ne!(
                expected,
                stream(&seed, KagemushaProofRecoveryPhaseV1::StateCarrier, &changed)
            );
        }
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[derive(Clone)]
    struct SmallRecoveryCircuit<F: ff::PrimeField>(halo2_proofs::circuit::Value<F>);

    #[cfg(feature = "zk-halo2-ipa")]
    impl<F: ff::PrimeField> halo2_proofs::plonk::Circuit<F> for SmallRecoveryCircuit<F> {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
        );
        type FloorPlanner = halo2_proofs::circuit::SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self(halo2_proofs::circuit::Value::unknown())
        }

        fn configure(meta: &mut halo2_proofs::plonk::ConstraintSystem<F>) -> Self::Config {
            let advice = meta.advice_column();
            let instance = meta.instance_column();
            meta.enable_equality(advice);
            meta.enable_equality(instance);
            (advice, instance)
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl halo2_proofs::circuit::Layouter<F>,
        ) -> Result<(), halo2_proofs::plonk::Error> {
            let cell = layouter.assign_region(
                || "test recovery witness",
                |mut region| Ok(region.assign_advice(config.0, 0, self.0).cell()),
            )?;
            layouter.constrain_instance(cell, config.1, 0);
            Ok(())
        }
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn real_small_proofs_recover_identically_in_both_parities() {
        // This exercises the real production proof writers on a tiny equality circuit. It is
        // not evidence that the complete K16 KAGEMUSHA circuits fit or are release-qualified.
        macro_rules! check_parity {
            ($curve:ty, $scalar:ty, $create:ident) => {{
                let params = ParamsIPA::<$curve>::new(6);
                let value = <$scalar>::from(17);
                let circuit = SmallRecoveryCircuit(halo2_proofs::circuit::Value::known(value));
                let vk = keygen_vk(&params, &circuit).expect("small recovery VK");
                let pk = keygen_pk(&params, vk, &circuit).expect("small recovery PK");
                let seed = KagemushaRecoverySeedV1::from_unsealed([0xB7; 32]).expect("test seed");
                let recovered_seed =
                    KagemushaRecoverySeedV1::from_unsealed([0xB7; 32]).expect("same test seed");
                let other_seed =
                    KagemushaRecoverySeedV1::from_unsealed([0xB8; 32]).expect("other test seed");
                let make = |seed: &KagemushaRecoverySeedV1, phase| {
                    $create(&params, &pk, circuit.clone(), &[value], phase, seed)
                        .expect("real small recovery proof")
                };
                let proof = make(&seed, KagemushaProofRecoveryPhaseV1::CommitWrapper);
                assert_eq!(
                    proof,
                    make(
                        &recovered_seed,
                        KagemushaProofRecoveryPhaseV1::CommitWrapper
                    )
                );
                assert_ne!(
                    proof,
                    make(&other_seed, KagemushaProofRecoveryPhaseV1::CommitWrapper)
                );
                assert_ne!(
                    proof,
                    make(&seed, KagemushaProofRecoveryPhaseV1::TerminalAuthorization)
                );
                // Terminally verify the raw transcript; the production writer appends exactly
                // one 32-byte folded-generator point after this complete Halo2 proof.
                type Transcript<S> = PoseidonTranscript<
                    $curve,
                    NativeLoader,
                    S,
                    KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
                    KAGEMUSHA_IPA_POSEIDON_RATE_V1,
                    KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
                    KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
                >;
                let raw = &proof[..proof.len() - 32];
                let mut cursor = Cursor::new(raw);
                let mut transcript =
                    Transcript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(&mut cursor);
                let instances = [value];
                let columns: [&[$scalar]; 1] = [&instances];
                let all_instances: [&[&[$scalar]]; 1] = [&columns];
                let strategy = halo2_proofs::poly::ipa::strategy::SingleStrategy::new(&params);
                verify_proof::<
                    IPACommitmentScheme<$curve>,
                    VerifierIPA<'_, $curve>,
                    ChallengeScalar<$curve>,
                    _,
                    _,
                >(
                    &params,
                    pk.get_vk(),
                    strategy,
                    &all_instances,
                    &mut transcript,
                )
                .expect("terminally verify recovered proof");
                drop(transcript);
                assert_eq!(cursor.position() as usize, raw.len());
            }};
        }
        check_parity!(EqAffine, Fp, create_eq_proof_with_key_v1);
        check_parity!(EpAffine, Fq, create_ep_proof_with_key_v1);
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn real_small_keys_stream_canonically_in_both_parities() {
        // K6 checks the actual checked decoder and canonical hashing path, not K16 cash
        // feasibility. Full encoded buffers are used only to provide this small test fixture.
        macro_rules! check_parity {
            ($curve:ty, $scalar:ty, $parity:ident, $role:ident, $create:ident) => {{
                let params = ParamsIPA::<$curve>::new(6);
                let value = <$scalar>::from(23);
                let circuit = SmallRecoveryCircuit(halo2_proofs::circuit::Value::known(value));
                let vk = keygen_vk(&params, &circuit).expect("small canonical VK");
                let pk = keygen_pk(&params, vk, &circuit).expect("small canonical PK");
                let bytes = pk.to_bytes(SerdeFormat::Processed);
                let binding = binding(KagemushaArtifactRoleV1::$role, &bytes);
                let mut cursor = Cursor::new(bytes.as_slice());
                let recovered = read_canonical_proving_key_v1::<
                    $curve,
                    SmallRecoveryCircuit<$scalar>,
                >(
                    &mut cursor, binding, KagemushaPastaParityV1::$parity, 6, ()
                )
                .expect("checked canonical PK");
                assert_eq!(cursor.position(), binding.byte_len);
                ensure_cursor_consumed(
                    KagemushaPastaParityV1::$parity,
                    "test proving key",
                    &cursor,
                    bytes.len(),
                )
                .expect("complete input consumed");
                assert_eq!(recovered.to_bytes(SerdeFormat::Processed), bytes);
                ensure_embedded_vk(
                    KagemushaPastaParityV1::$parity,
                    &recovered,
                    &pk.get_vk().to_bytes(SerdeFormat::Processed),
                )
                .expect("same embedded VK");
                let vk_len = pk.get_vk().to_bytes(SerdeFormat::Processed).len();
                let mut invalid_length = bytes.clone();
                invalid_length[vk_len..vk_len + 4].copy_from_slice(&u32::MAX.to_be_bytes());
                let mut invalid_field = bytes.clone();
                invalid_field[vk_len + 4..vk_len + 36].fill(0xFF);
                for malformed in [invalid_length.as_slice(), invalid_field.as_slice()]
                    .into_iter()
                    .chain([0, 9, vk_len, vk_len + 5, bytes.len() - 1].map(|cut| &bytes[..cut]))
                {
                    let result = std::panic::catch_unwind(|| {
                        read_canonical_proving_key_v1::<$curve, SmallRecoveryCircuit<$scalar>>(
                            &mut Cursor::new(malformed),
                            binding,
                            KagemushaPastaParityV1::$parity,
                            6,
                            (),
                        )
                    });
                    assert!(result.expect("malformed key must not panic").is_err());
                }
                let seed = KagemushaRecoverySeedV1::from_unsealed([0xC7; 32])
                    .expect("public test seed only");
                let make = |key| {
                    $create(
                        &params,
                        key,
                        circuit.clone(),
                        &[value],
                        KagemushaProofRecoveryPhaseV1::CommitWrapper,
                        &seed,
                    )
                    .expect("small real proof")
                };
                assert_eq!(make(&pk), make(&recovered));
                for changed in [
                    KagemushaArtifactBindingV1 {
                        byte_len: binding.byte_len - 1,
                        ..binding
                    },
                    KagemushaArtifactBindingV1 {
                        byte_len: binding.byte_len + 1,
                        ..binding
                    },
                    KagemushaArtifactBindingV1 {
                        sha256: [0x99; 32],
                        ..binding
                    },
                ] {
                    assert!(
                        read_canonical_proving_key_v1::<$curve, SmallRecoveryCircuit<$scalar>>(
                            &mut Cursor::new(bytes.as_slice()),
                            changed,
                            KagemushaPastaParityV1::$parity,
                            6,
                            (),
                        )
                        .is_err()
                    );
                }
                assert!(
                    read_canonical_proving_key_v1::<$curve, SmallRecoveryCircuit<$scalar>>(
                        &mut Cursor::new(bytes.as_slice()),
                        binding,
                        KagemushaPastaParityV1::$parity,
                        16,
                        (),
                    )
                    .is_err(),
                    "reject wrong k before domain allocation"
                );
                let mut resolver = KagemushaMemoryArtifactResolverV1::default();
                resolver.insert(Arc::<[u8]>::from(bytes.as_slice()));
                // Storage-only fixture: this does not authenticate a release or cash circuit.
                let artifacts = KagemushaAuthenticatedArtifactSetV1::for_stream_tests(
                    resolver,
                    binding,
                );
                let streamed = artifacts
                    .read_verified(binding.role, |reader| {
                        read_canonical_proving_key_v1::<$curve, SmallRecoveryCircuit<$scalar>>(
                            reader,
                            binding,
                            KagemushaPastaParityV1::$parity,
                            6,
                            (),
                        )
                    })
                    .expect("complete K6 stream authentication and canonical key decoding");
                assert_eq!(streamed.to_bytes(SerdeFormat::Processed), bytes);
                assert!(matches!(
                    load_authenticated_proving_key_v1::<
                        $curve,
                        SmallRecoveryCircuit<$scalar>,
                        _,
                    >(&artifacts, binding.role, KagemushaPastaParityV1::$parity, ()),
                    Err(KagemushaArtifactGenerationErrorV1::KeyDecode { .. }),
                ), "production key loading keeps mandatory K16");
                let wrong_parity = match KagemushaPastaParityV1::$parity {
                    KagemushaPastaParityV1::Eq => KagemushaPastaParityV1::Ep,
                    KagemushaPastaParityV1::Ep => KagemushaPastaParityV1::Eq,
                };
                for (role, parity) in [
                    (binding.role, wrong_parity),
                    (KagemushaArtifactRoleV1::ParamsEq, KagemushaPastaParityV1::Eq),
                ] {
                    assert!(matches!(
                        load_authenticated_proving_key_v1::<
                            $curve,
                            SmallRecoveryCircuit<$scalar>,
                            _,
                        >(&artifacts, role, parity, ()),
                        Err(KagemushaArtifactGenerationErrorV1::Artifact(
                            KagemushaArtifactErrorV1::InvalidBinding(_),
                        )),
                    ));
                }
            }};
        }
        check_parity!(EqAffine, Fp, Eq, StatePkEq, create_eq_proof_with_key_v1);
        check_parity!(EpAffine, Fq, Ep, StatePkEp, create_ep_proof_with_key_v1);
    }

    #[cfg(feature = "zk-halo2-ipa")]
    fn shape_only_terminal_material(send: bool) -> KagemushaGeneratedCommitWrapperProofV1 {
        use iroha_data_model::nexus::AxtAssetIncarnationV1;
        use snark_verifier::pcs::ipa::IpaAccumulator;

        // Syntactically valid accumulators and opaque max-size bytes exercise serialization
        // only; this fixture is deliberately not a proof of any monetary transition.
        let eq_history = KagemushaEqAccumulatorV1::from_native(&IpaAccumulator::new(
            vec![Fp::from(2); KAGEMUSHA_HALO2_K_V1 as usize],
            EqAffine::generator(),
        ))
        .expect("shape-only Eq history");
        let ep_history = KagemushaEpAccumulatorV1::from_native(&IpaAccumulator::new(
            vec![Fq::from(3); KAGEMUSHA_HALO2_K_V1 as usize],
            EpAffine::generator(),
        ))
        .expect("shape-only Ep history");
        let public = KagemushaTerminalAuthorizationPublicInputsV1 {
            operation: if send {
                KagemushaOperationV1::SendSplit
            } else {
                KagemushaOperationV1::RedeemSplit
            },
            protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
            suite_id: [1; 32],
            vk_digest: [2; 32],
            release_id: [3; 32],
            network_id: [4; 32],
            asset_id: [5; 32],
            asset_incarnation: AxtAssetIncarnationV1::try_from_bytes([1; 32]).expect("incarnation"),
            asset_scale: 2,
            liability_pool_id: [6; 32],
            hardware_profile_id: [7; 32],
            policy_epoch: 1,
            lifecycle_binding_digest: [8; 32],
            semantic_digest: [9; 32],
            candidate_envelope_digest: [10; 32],
            commit_certificate_digest: [11; 32],
            transition_nullifier: [12; 32],
            request_digest: if send { [13; 32] } else { [0; 32] },
            receiver_binding_digest: if send { [14; 32] } else { [0; 32] },
            ciphertext_commitment: if send { [15; 32] } else { [0; 32] },
            amount: 17,
            terminal_output_binding: [16; 32],
            eq_deferred_audit: [17; 32],
            ep_deferred_audit: [18; 32],
            eq_protocol_digest: encode(Fp::from(101)),
            ep_protocol_digest: encode(Fq::from(102)),
        };
        KagemushaGeneratedCommitWrapperProofV1 {
            eq_public_instances: terminal_authorization_public_instances::<Fp>(
                &public,
                eq_history.as_bytes(),
            )
            .expect("Eq public column"),
            ep_public_instances: terminal_authorization_public_instances::<Fq>(
                &public,
                ep_history.as_bytes(),
            )
            .expect("Ep public column"),
            public,
            eq_proof: vec![0x51; KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1],
            ep_proof: vec![0x52; KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1],
            eq_current_accumulator: eq_history.clone(),
            ep_current_accumulator: ep_history.clone(),
            eq_history,
            ep_history,
        }
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn shape_only_commit_wrapper_conversion_preserves_exact_bindings_and_hard_size() {
        assert_eq!(TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1, 81);
        assert_eq!(KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1, 2_495);
        assert_eq!(KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1, 6_528);
        let send = shape_only_terminal_material(true);
        let expected = send.public.clone();
        let payment = send.into_payment().expect("shape-only payment conversion");
        assert_eq!(payment.proof.semantic_digest, expected.semantic_digest);
        assert_eq!(
            payment.proof.candidate_envelope_digest,
            expected.candidate_envelope_digest
        );
        assert_eq!(
            payment.proof.commit_certificate_digest,
            expected.commit_certificate_digest
        );
        assert_eq!(payment.eq_public_instances.len(), 81);
        assert_eq!(payment.ep_public_instances.len(), 81);
        let payment_bytes = norito::encode_canonical(&payment.proof).expect("payment encoding");
        assert!(payment_bytes.len() <= KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1);

        let redemption = shape_only_terminal_material(false)
            .into_redemption()
            .expect("shape-only redemption conversion");
        assert_eq!(redemption.proof.semantic_digest, expected.semantic_digest);
        assert_eq!(
            redemption.proof.candidate_envelope_digest,
            expected.candidate_envelope_digest
        );
        assert_eq!(
            redemption.proof.commit_certificate_digest,
            expected.commit_certificate_digest
        );
        let redemption_bytes =
            norito::encode_canonical(&redemption.proof).expect("redemption encoding");
        assert_eq!(payment_bytes.len(), redemption_bytes.len());
        assert!(redemption_bytes.len() <= KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1);
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn shape_only_commit_wrapper_conversion_rejects_opposite_family_and_mutation() {
        assert!(
            shape_only_terminal_material(true)
                .into_redemption()
                .is_err()
        );
        assert!(shape_only_terminal_material(false).into_payment().is_err());
        for field in 0..8 {
            let mut material = shape_only_terminal_material(true);
            match field {
                0 => material.public.semantic_digest[0] ^= 1,
                1 => material.public.candidate_envelope_digest[0] ^= 1,
                2 => material.public.commit_certificate_digest[0] ^= 1,
                3 => material.public.lifecycle_binding_digest[0] ^= 1,
                4 => material.eq_public_instances.pop().map(|_| ()).unwrap(),
                5 => material.ep_public_instances[0] += Fq::from(1),
                6 => material.eq_proof.push(0),
                7 => material.ep_proof.clear(),
                _ => unreachable!(),
            }
            assert!(material.into_payment().is_err(), "mutation {field}");
        }
        for operation in [
            KagemushaOperationV1::Bootstrap,
            KagemushaOperationV1::MintFold,
            KagemushaOperationV1::ReceiveFold,
            KagemushaOperationV1::SuiteUpgrade,
            KagemushaOperationV1::Rotate,
        ] {
            assert!(require_terminal_operation_v1(operation, operation).is_err());
        }
    }

    #[test]
    fn generated_binding_is_exact_sha256_and_length() {
        let binding = binding(KagemushaArtifactRoleV1::StateVkEq, b"processed-vk");
        assert_eq!(binding.byte_len, 12);
        assert_eq!(
            binding.sha256,
            <[u8; 32]>::from(Sha256::digest(b"processed-vk"))
        );
    }

    #[test]
    fn fixed_artifact_bounds_reject_empty_and_oversized_files() {
        assert!(validate_length(KagemushaPastaParityV1::Eq, "vk", 0, 10, false).is_err());
        assert!(validate_length(KagemushaPastaParityV1::Eq, "vk", 11, 10, false).is_err());
        assert!(validate_length(KagemushaPastaParityV1::Eq, "params", 9, 10, true).is_err());
        assert!(validate_length(KagemushaPastaParityV1::Eq, "params", 10, 10, true).is_ok());
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn recursive_public_shape_and_transport_bound_are_fixed() {
        assert_eq!(PUBLIC_INSTANCE_COUNT, 85);
        assert_eq!(recursive_public_instance_count(), 119);
        assert!(
            validate_recursive_proof_length(
                KagemushaPastaParityV1::Eq,
                &[0; KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1],
            )
            .is_ok()
        );
        assert!(
            validate_recursive_proof_length(
                KagemushaPastaParityV1::Eq,
                &[0; KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1 + 1],
            )
            .is_err()
        );
        assert!(
            validate_paired_proof_length(
                KagemushaPastaParityV1::Ep,
                &[0; KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1],
            )
            .is_ok()
        );
        assert!(
            validate_paired_proof_length(
                KagemushaPastaParityV1::Ep,
                &[0; KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1 + 1],
            )
            .is_err()
        );
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn transport_profile_preflight_rejects_wide_internal_shape() {
        let wide = KagemushaOrdinaryProofProfileV1 {
            witness_commitments: 129,
            quotient_commitments: 8,
            evaluations: 449,
            bgh19_rotation_sets: 6,
            opening_items: 37,
            byte_len: 20_128,
        };
        assert!(matches!(
            validate_transport_proof_profile(
                KagemushaPastaParityV1::Eq,
                "platform credential",
                wide,
            ),
            Err(
                KagemushaArtifactGenerationErrorV1::TransportProofProfileTooLarge {
                    actual: 20_128,
                    maximum: 2_495,
                    ..
                }
            )
        ));

        let compact = KagemushaOrdinaryProofProfileV1 {
            witness_commitments: 30,
            quotient_commitments: 3,
            evaluations: 5,
            bgh19_rotation_sets: 2,
            opening_items: 37,
            byte_len: 2_464,
        };
        validate_transport_proof_profile(
            KagemushaPastaParityV1::Ep,
            "compact transport decider",
            compact,
        )
        .expect("compact transcript profile fits the configured parity slot");
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn recursive_profile_comparison_covers_every_layout_field() {
        let profile = BaseCircuitParams {
            k: KAGEMUSHA_HALO2_K_V1 as usize,
            num_advice_per_phase: vec![1],
            num_fixed: 1,
            num_lookup_advice_per_phase: vec![1],
            lookup_bits: Some((KAGEMUSHA_HALO2_K_V1 - 1) as usize),
            num_instance_columns: 1,
        };
        assert!(same_base_params(&profile, &profile));
        let mut substituted = profile.clone();
        substituted.num_fixed += 1;
        assert!(!same_base_params(&profile, &substituted));
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn mint_authority_public_shape_keeps_amount_and_pair_binding_explicit() {
        let public = mint_authority_public_instances::<Fp>(
            KagemushaMintAuthorityStepV1::FinalizedMint,
            [1; 32],
            u128::MAX,
            [2; 32],
            [3; 32],
            [4; 32],
            [5; 32],
            [6; 32],
            [7; 32],
            [8; 32],
            [9; 32],
            [10; 32],
            &[0; super::super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
        )
        .expect("fixed mint-authority instances");
        assert_eq!(
            public.len(),
            KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1
        );
        assert_eq!(public[3], from_u128::<Fp>(u128::MAX));
        assert_eq!(
            &public[20..22],
            &crate::zk::kagemusha_v1_poseidon::digest_limbs::<Fp>([10; 32])
        );
    }
}

#[cfg(all(test, feature = "zk-halo2-ipa"))]
#[path = "generation_mint_transport_tests.rs"]
mod mint_transport_tests;
