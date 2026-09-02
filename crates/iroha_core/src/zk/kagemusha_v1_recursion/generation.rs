//! Deterministic fixed-k artifact generation and proving for the recursive aggregate state.
//!
//! The emitted files use exactly the raw formats authenticated by the V1 release manifest:
//! `ParamsIPA::write` and Halo2 `SerdeFormat::Processed`. State-role keys are derived only from
//! the complete recursive circuit: the six-operation balance relation, predecessor recursion,
//! delayed-history fold, mint-finality helper, normalized hardware GuardBundle, and reciprocal
//! Pasta equation audit are one inseparable proving authority.

use std::{io::Cursor, sync::Arc};

#[cfg(feature = "zk-halo2-ipa")]
use ff::Field as _;
#[cfg(feature = "zk-halo2-ipa")]
use halo2_base::gates::circuit::BaseCircuitParams;
use halo2_proofs::{
    SerdeFormat,
    halo2curves::{
        group::{GroupEncoding as _, prime::PrimeCurveAffine as _},
        pasta::{EpAffine, EqAffine, Fp, Fq},
    },
    plonk::{Circuit as _, ProvingKey, VerifyingKey, create_proof, keygen_pk, keygen_vk},
    poly::{
        commitment::{Params as _, ParamsProver as _},
        ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            multiopen::ProverIPA,
        },
    },
};
#[cfg(feature = "zk-halo2-ipa")]
use iroha_data_model::kagemusha::{
    KAGEMUSHA_COMMIT_WRAPPER_PROOF_MAX_BYTES_V1, KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1,
    KAGEMUSHA_WIRE_VERSION_V1, KagemushaAcceptanceIntentAuthorizationStatementV1,
    KagemushaAcceptanceIntentAuthorizationV1, KagemushaAcceptanceIntentV1,
    KagemushaAcceptanceTicketV1, KagemushaAuthenticatedReleaseV1,
    KagemushaCommitCertificateV1, KagemushaCommitWrapperProofV1,
    KagemushaHardwareCredentialV1, KagemushaHardwareProfileV1, KagemushaLifecycleBindingV1,
    KagemushaNoCommitClosureStatementV1, KagemushaNoCommitClosureV1,
    KagemushaOutboxReservationV1, KagemushaPairedProofV1, KagemushaPaymentRequestV1,
    KagemushaQualifiedHelperCircuitV1,
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_HALO2_K_V1, KAGEMUSHA_PARAMS_BYTES_V1,
    KAGEMUSHA_STATE_PROVING_KEY_MAX_BYTES_V1, KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1,
    KagemushaArtifactBindingV1, KagemushaArtifactRoleV1,
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
use super::commit_wrapper::{
    COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1, KagemushaCommitEvidenceOpeningV1,
    KagemushaCommitWrapperIntentAuthorizationPrivateV1,
    KagemushaCommitWrapperNoCommitClosurePrivateV1,
};
#[cfg(feature = "zk-halo2-ipa")]
use super::{
    COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1, KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1, KAGEMUSHA_IPA_POSEIDON_RATE_V1,
    KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1, KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
    KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1, KagemushaCommitWrapperEpCircuitV1,
    KagemushaCommitWrapperEpWitnessV1, KagemushaCommitWrapperEqCircuitV1,
    KagemushaCommitWrapperEqWitnessV1, KagemushaCommitWrapperPrivateTransitionV1,
    KagemushaCommitWrapperPublicInputsV1, KagemushaCommitWrapperWitnessV1,
    KagemushaEpAccumulatorV1, KagemushaEpFoldProofV1, KagemushaEqAccumulatorV1,
    KagemushaEqFoldProofV1, KagemushaGuardBundleRelationWitnessV1,
    KagemushaStateRelationWitnessV1, build_kagemusha_commit_wrapper_pair_v1,
    composite::{
        KagemushaRecursiveIncomingEpWitnessV1 as CompositeIncomingEpWitnessV1,
        KagemushaRecursiveIncomingEqWitnessV1 as CompositeIncomingEqWitnessV1,
        KagemushaRecursiveStateEpCircuitV1, KagemushaRecursiveStateEqCircuitV1,
        KagemushaRecursiveStateWitnessV1, KagemushaRotateVerifierBridgeWitnessV1,
        build_kagemusha_recursive_state_pair_v1,
    },
    deferred_parent::{
        KagemushaOrdinaryProofProfileV1, native_parent_protocol_digest_v1,
        ordinary_ipa_proof_profile_v1,
    },
    fold_kagemusha_ep_accumulators_v1, fold_kagemusha_eq_accumulators_v1,
    initial_kagemusha_ep_accumulator_v1, initial_kagemusha_eq_accumulator_v1,
    mint_authority::{
        KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1, KagemushaMintAuthorityCheckpointV1,
        KagemushaMintAuthorityEpCircuitV1, KagemushaMintAuthorityEqCircuitV1,
        KagemushaMintAuthorityPairBindingV1, KagemushaMintAuthorityPairWitnessV1,
        KagemushaMintAuthorityParityWitnessV1, build_kagemusha_mint_authority_pair_v1,
        public_instance as mint_authority_public_instance,
    },
    mint_authorization::{
        MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1, KagemushaMintAuthorizationEpCircuitV1,
        KagemushaMintAuthorizationEqCircuitV1, KagemushaMintAuthorizationRecursiveWitnessV1,
        KagemushaMintAuthorizationRelationWitnessV1,
        build_kagemusha_mint_authorization_pair_v1, mint_authorization_public_instances_v1,
    },
    mint_helper::{KagemushaMintAuthorityStepV1, KagemushaMintCertificateWitnessV1},
    native_backend::{verify_ep_succinct_protocol, verify_eq_succinct_protocol},
    state_relation::PUBLIC_INSTANCE_COUNT,
};
use super::{
    KagemushaArtifactByteResolverV1, KagemushaArtifactErrorV1,
    KagemushaAuthenticatedArtifactSetV1, KagemushaMemoryArtifactResolverV1,
    KagemushaPastaParityV1,
};
#[cfg(feature = "zk-halo2-ipa")]
use crate::zk::{
    kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, decode, encode, from_u128},
    kagemusha_v1_state::KagemushaStateV1,
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
}

/// Complete private input needed to build both production recursive state circuits.
///
/// The wrapper is public so release tooling can key and prove the exact same circuit without
/// exposing the circuit builder itself. Every referenced protocol and proof is consumed inside
/// the recursive circuit; host-side verification is not a substitute for this witness.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Copy)]
pub struct KagemushaRecursiveIncomingEqGenerationWitnessV1<'a> {
    /// Eq terminal CommitWrapper public instances for this fixed slot.
    pub instances: &'a [Vec<Fp>],
    /// Eq terminal `SendSplit` wrapper proof or release-pinned inactive wrapper-padding proof.
    pub proof: &'a [u8],
    /// Eq delayed history committed by the incoming sender proof.
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

/// The fixed Ep/Fq incoming sender proof for a `ReceiveFold` relation.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Copy)]
pub struct KagemushaRecursiveIncomingEpGenerationWitnessV1<'a> {
    /// Ep terminal CommitWrapper public instances for this fixed slot.
    pub instances: &'a [Vec<Fq>],
    /// Ep terminal `SendSplit` wrapper proof or release-pinned inactive wrapper-padding proof.
    pub proof: &'a [u8],
    /// Ep delayed history committed by the incoming sender proof.
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

/// Exact old-suite authorization carried by a verifier-changing `Rotate` transition.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaRotateVerifierBridgeGenerationWitnessV1 {
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
impl KagemushaRotateVerifierBridgeGenerationWitnessV1 {
    /// Canonical inactive witness required unless `Rotate` changes verifier suites.
    pub const ZERO: Self = Self {
        old_eq_protocol_digest: [0; 32],
        old_ep_protocol_digest: [0; 32],
        old_suite_id: [0; 32],
        old_vk_digest: [0; 32],
        new_suite_id: [0; 32],
        new_vk_digest: [0; 32],
        governance_authorization_proof_digest: [0; 32],
    };

    const fn into_composite(self) -> KagemushaRotateVerifierBridgeWitnessV1 {
        KagemushaRotateVerifierBridgeWitnessV1 {
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
/// One incoming proof slot is always present. Outside `ReceiveFold` it carries the release-pinned
/// valid padding proof and history, so proof shape never depends on aggregate history.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone)]
pub struct KagemushaRecursiveStateGenerationWitnessV1<'a> {
    /// Six-operation aggregate-state transition witness.
    pub state: KagemushaStateRelationWitnessV1,
    /// Normalized hardware guard semantics constrained into the state proof.
    pub guard_relation: KagemushaGuardBundleRelationWitnessV1,
    /// Old-parent verifier bridge; canonical zero unless `Rotate` changes verifier suites.
    pub rotate_verifier_bridge: KagemushaRotateVerifierBridgeGenerationWitnessV1,
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
    /// Eq terminal CommitWrapper protocol compiled from the authenticated release key.
    pub eq_incoming_protocol: &'a PlonkProtocol<EqAffine>,
    /// Ep terminal CommitWrapper protocol compiled from the authenticated release key.
    pub ep_incoming_protocol: &'a PlonkProtocol<EpAffine>,
    /// Eq terminal CommitWrapper proof/history consumed by `ReceiveFold`.
    pub eq_incoming: KagemushaRecursiveIncomingEqGenerationWitnessV1<'a>,
    /// Ep terminal CommitWrapper proof/history consumed by `ReceiveFold`.
    pub ep_incoming: KagemushaRecursiveIncomingEpGenerationWitnessV1<'a>,
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
        eq_incoming: CompositeIncomingEqWitnessV1<'b>,
        ep_incoming: CompositeIncomingEpWitnessV1<'b>,
    ) -> KagemushaRecursiveStateWitnessV1<'b>
    where
        'a: 'b,
    {
        KagemushaRecursiveStateWitnessV1 {
            state: self.state,
            guard_relation: self.guard_relation,
            rotate_verifier_bridge: self.rotate_verifier_bridge.into_composite(),
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
            eq_incoming,
            ep_incoming,
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
    /// Eq/Fp parameter and complete recursive-state key bytes.
    pub eq: KagemushaGeneratedOperationArtifactsV1,
    /// Ep/Fq parameter and complete recursive-state key bytes.
    pub ep: KagemushaGeneratedOperationArtifactsV1,
    /// Exact Eq `halo2-base` layout required to decode the processed key.
    pub eq_circuit_params: BaseCircuitParams,
    /// Exact Ep `halo2-base` layout required to decode the processed key.
    pub ep_circuit_params: BaseCircuitParams,
    /// Eq compiled recursive-state protocol identity committed by every state proof.
    pub eq_protocol_digest: [u8; 32],
    /// Ep compiled recursive-state protocol identity committed by every state proof.
    pub ep_protocol_digest: [u8; 32],
}

/// Loaded Eq production recursive-state parameters and keys.
#[cfg(feature = "zk-halo2-ipa")]
pub struct KagemushaLoadedEqRecursiveStateArtifactsV1 {
    /// Canonical Eq transparent IPA parameters.
    pub parameters: ParamsIPA<EqAffine>,
    /// Exact processed Eq recursive-state proving key.
    pub proving_key: ProvingKey<EqAffine>,
    /// Exact processed Eq recursive-state verifying key.
    pub verifying_key: VerifyingKey<EqAffine>,
    /// Authenticated circuit layout used to parse both keys.
    pub circuit_params: BaseCircuitParams,
}

/// Loaded Ep production recursive-state parameters and keys.
#[cfg(feature = "zk-halo2-ipa")]
pub struct KagemushaLoadedEpRecursiveStateArtifactsV1 {
    /// Canonical Ep transparent IPA parameters.
    pub parameters: ParamsIPA<EpAffine>,
    /// Exact processed Ep recursive-state proving key.
    pub proving_key: ProvingKey<EpAffine>,
    /// Exact processed Ep recursive-state verifying key.
    pub verifying_key: VerifyingKey<EpAffine>,
    /// Authenticated circuit layout used to parse both keys.
    pub circuit_params: BaseCircuitParams,
}

/// One complete constant-size production recursive state proof pair.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaGeneratedRecursiveStateProofV1 {
    /// Eq public column required by the next recursive proof.
    pub eq_public_instances: Vec<Fp>,
    /// Ep public column required by the next recursive proof.
    pub ep_public_instances: Vec<Fq>,
    /// Eq/Fp recursive proof transcript.
    pub eq_proof: Vec<u8>,
    /// Ep/Fq recursive proof transcript.
    pub ep_proof: Vec<u8>,
    /// Eq current opening claim extracted from the generated proof for the next history fold.
    pub eq_current_accumulator: KagemushaEqAccumulatorV1,
    /// Ep current opening claim extracted from the generated proof for the next history fold.
    pub ep_current_accumulator: KagemushaEpAccumulatorV1,
    /// Eq delayed-history accumulator exposed with the proof.
    pub eq_history: KagemushaEqAccumulatorV1,
    /// Ep delayed-history accumulator exposed with the proof.
    pub ep_history: KagemushaEpAccumulatorV1,
}

/// Fixed release-enabled hardware-profile table width committed by every wrapper key.
#[cfg(feature = "zk-halo2-ipa")]
pub const KAGEMUSHA_COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1: usize =
    COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1;

/// Unlinkable public terminal values used to generate a final commit-wrapper proof.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaCommitWrapperTerminalGenerationPublicV1 {
    /// Exact released payment or redemption lifecycle.
    pub lifecycle: KagemushaLifecycleBindingV1,
    /// Digest of the unlinkable transfer/redemption statement.
    pub semantic_digest: [u8; 32],
    /// Digest of the durably persisted candidate proof envelope.
    pub candidate_envelope_digest: [u8; 32],
    /// Digest of the exact terminal hardware certificate.
    pub commit_certificate_digest: [u8; 32],
    /// Proof-derived send or redemption nullifier.
    pub transition_nullifier: [u8; 32],
    /// Send-only request digest; zero for redemption.
    pub request_digest: [u8; 32],
    /// Send-only one-use ticket digest; zero for redemption.
    pub acceptance_ticket_digest: [u8; 32],
    /// Send-only ciphertext commitment; zero for redemption.
    pub ciphertext_commitment: [u8; 32],
    /// Positive public terminal amount.
    pub amount: u128,
    /// Operation-specific terminal output binding. Send commits receiver credit/lane semantics;
    /// redemption carries its redemption commitment.
    pub terminal_output_binding: [u8; 32],
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaCommitWrapperTerminalGenerationPublicV1 {
    fn into_internal(
        self,
        eq_deferred_audit: [u8; 32],
        ep_deferred_audit: [u8; 32],
        eq_protocol_digest: [u8; 32],
        ep_protocol_digest: [u8; 32],
    ) -> Result<KagemushaCommitWrapperPublicInputsV1, String> {
        KagemushaCommitWrapperPublicInputsV1::from_lifecycle(
            &self.lifecycle,
            self.semantic_digest,
            self.candidate_envelope_digest,
            self.commit_certificate_digest,
            self.transition_nullifier,
            self.request_digest,
            self.acceptance_ticket_digest,
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

/// Public and proof-envelope values for one pre-ticket sender authorization.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaAcceptanceIntentAuthorizationGenerationPublicV1 {
    /// Exact signed receiver request authorized by the hidden sender predecessor.
    pub request: KagemushaPaymentRequestV1,
    /// Compact release-bound authorization statement carried on wire.
    pub statement: KagemushaAcceptanceIntentAuthorizationStatementV1,
    /// Eq credential-equation audit exposed by the recursively verified GuardBundle.
    pub guard_eq_credential_audit: [u8; 32],
    /// Ep credential-equation audit exposed by the recursively verified GuardBundle.
    pub guard_ep_credential_audit: [u8; 32],
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaAcceptanceIntentAuthorizationGenerationPublicV1 {
    fn into_internal(
        self,
        eq_deferred_audit: [u8; 32],
        ep_deferred_audit: [u8; 32],
        eq_protocol_digest: [u8; 32],
        ep_protocol_digest: [u8; 32],
    ) -> Result<
        (
            KagemushaCommitWrapperPublicInputsV1,
            KagemushaCommitWrapperIntentAuthorizationPrivateV1,
        ),
        String,
    > {
        if self.guard_eq_credential_audit == [0; 32]
            || self.guard_ep_credential_audit == [0; 32]
            || self.guard_eq_credential_audit == self.guard_ep_credential_audit
        {
            return Err("acceptance-intent credential audits are noncanonical".to_owned());
        }
        let public = KagemushaCommitWrapperPublicInputsV1::from_acceptance_intent_authorization(
            &self.request,
            &self.statement,
            self.guard_eq_credential_audit,
            self.guard_ep_credential_audit,
            eq_deferred_audit,
            ep_deferred_audit,
            eq_protocol_digest,
            ep_protocol_digest,
        )?;
        let private = KagemushaCommitWrapperIntentAuthorizationPrivateV1 {
            request: self.request,
            statement: self.statement,
        };
        Ok((public, private))
    }
}

/// Public wire values and hardware-private uniqueness nonce for one no-commit closure.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaNoCommitClosureGenerationPublicV1 {
    /// Public unlinkable closure statement authenticated by both wrapper parities.
    pub statement: KagemushaNoCommitClosureStatementV1,
    /// Exact signed receiver request whose reserved delivery slot is released.
    pub request: KagemushaPaymentRequestV1,
    /// Exact original proof-bearing intent authorization being cancelled.
    pub intent_authorization: KagemushaAcceptanceIntentAuthorizationV1,
    /// Exact receiver-hardware acceptance ticket paired with the intent.
    pub acceptance_ticket: KagemushaAcceptanceTicketV1,
    /// Eq credential-equation audit exposed by the cancellation GuardBundle.
    pub guard_eq_credential_audit: [u8; 32],
    /// Ep credential-equation audit exposed by the cancellation GuardBundle.
    pub guard_ep_credential_audit: [u8; 32],
    /// Hardware-private fresh nonce used to derive the lane-scoped recovery identity.
    pub hardware_recovery_nonce: [u8; 32],
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaNoCommitClosureGenerationPublicV1 {
    fn into_internal(
        self,
        eq_deferred_audit: [u8; 32],
        ep_deferred_audit: [u8; 32],
        eq_protocol_digest: [u8; 32],
        ep_protocol_digest: [u8; 32],
    ) -> Result<
        (
            KagemushaCommitWrapperPublicInputsV1,
            KagemushaCommitWrapperNoCommitClosurePrivateV1,
        ),
        String,
    > {
        self.statement
            .validate_shape()
            .map_err(|error| error.to_string())?;
        self.request
            .validate_shape()
            .map_err(|error| error.to_string())?;
        self.intent_authorization
            .validate_shape_against(&self.request)
            .map_err(|error| error.to_string())?;
        self.acceptance_ticket
            .validate_shape_against(&self.request, &self.intent_authorization.statement.intent)
            .map_err(|error| error.to_string())?;
        if self.guard_eq_credential_audit == [0; 32]
            || self.guard_ep_credential_audit == [0; 32]
            || self.guard_eq_credential_audit == self.guard_ep_credential_audit
            || self.hardware_recovery_nonce == [0; 32]
        {
            return Err("no-commit closure generation inputs are noncanonical".to_owned());
        }
        let public = KagemushaCommitWrapperPublicInputsV1::from_no_commit_closure(
            &self.statement,
            self.guard_eq_credential_audit,
            self.guard_ep_credential_audit,
            eq_deferred_audit,
            ep_deferred_audit,
            eq_protocol_digest,
            ep_protocol_digest,
        )?;
        let private = KagemushaCommitWrapperNoCommitClosurePrivateV1 {
            statement: self.statement,
            intent_authorization: self.intent_authorization,
            hardware_recovery_nonce: self.hardware_recovery_nonce,
        };
        Ok((public, private))
    }
}

/// Public branch selected under the sole release-pinned CommitWrapper key pair.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KagemushaCommitWrapperGenerationPublicV1 {
    /// Prepared transition finalized by an exact-once hardware commit.
    Terminal(KagemushaCommitWrapperTerminalGenerationPublicV1),
    /// Pre-ticket proof of a qualified sender's one-use intent reservation.
    AcceptanceIntentAuthorization(KagemushaAcceptanceIntentAuthorizationGenerationPublicV1),
    /// Sender-hardware proof that an accepted slot can never terminal-commit.
    NoCommitClosure(KagemushaNoCommitClosureGenerationPublicV1),
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaCommitWrapperGenerationPublicV1 {
    fn release_id(&self) -> [u8; 32] {
        match self {
            Self::Terminal(public) => public.lifecycle.release_id,
            Self::AcceptanceIntentAuthorization(public) => public.statement.release_id,
            Self::NoCommitClosure(public) => public.statement.release_id,
        }
    }

    fn validate_against_loaded_release(
        &self,
        release_id: [u8; 32],
        suite_id: [u8; 32],
        vk_digest: [u8; 32],
        artifact_manifest_digest: [u8; 32],
        enabled_hardware_profiles: &[[u8; 32];
             KAGEMUSHA_COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
    ) -> Result<(), String> {
        if self.release_id() != release_id {
            return Err("CommitWrapper witness selects a different release".to_owned());
        }
        match self {
            Self::Terminal(public) => {
                if public.lifecycle.suite_id != suite_id
                    || public.lifecycle.vk_digest != vk_digest
                    || !enabled_hardware_profiles
                        .iter()
                        .take_while(|profile| **profile != [0; 32])
                        .any(|profile| *profile == public.lifecycle.hardware_profile_id)
                {
                    return Err(
                        "terminal witness is not admitted by the authenticated release".to_owned(),
                    );
                }
            }
            Self::AcceptanceIntentAuthorization(public) => {
                if public.statement.suite_id != suite_id
                    || public.statement.vk_digest != vk_digest
                    || public.statement.artifact_manifest_digest != artifact_manifest_digest
                    || decode::<Fp>(public.guard_eq_credential_audit).is_none()
                    || decode::<Fq>(public.guard_ep_credential_audit).is_none()
                {
                    return Err(
                        "intent authorization is not admitted by the authenticated release"
                            .to_owned(),
                    );
                }
            }
            Self::NoCommitClosure(public) => {
                if public.statement.suite_id != suite_id
                    || public.statement.vk_digest != vk_digest
                    || public.statement.artifact_manifest_digest != artifact_manifest_digest
                    || public.intent_authorization.statement.release_id != release_id
                    || public.intent_authorization.statement.suite_id != suite_id
                    || public.intent_authorization.statement.vk_digest != vk_digest
                    || public
                        .intent_authorization
                        .statement
                        .artifact_manifest_digest
                        != artifact_manifest_digest
                    || decode::<Fp>(public.guard_eq_credential_audit).is_none()
                    || decode::<Fq>(public.guard_ep_credential_audit).is_none()
                    || public.hardware_recovery_nonce == [0; 32]
                {
                    return Err(
                        "no-commit closure is not admitted by the authenticated release".to_owned(),
                    );
                }
            }
        }
        Ok(())
    }

    fn into_internal(
        self,
        eq_deferred_audit: [u8; 32],
        ep_deferred_audit: [u8; 32],
        eq_protocol_digest: [u8; 32],
        ep_protocol_digest: [u8; 32],
    ) -> Result<
        (
            KagemushaCommitWrapperPublicInputsV1,
            Option<KagemushaCommitWrapperIntentAuthorizationPrivateV1>,
            Option<KagemushaCommitWrapperNoCommitClosurePrivateV1>,
            Option<([u8; 32], [u8; 32])>,
        ),
        String,
    > {
        match self {
            Self::Terminal(public) => Ok((
                public.into_internal(
                    eq_deferred_audit,
                    ep_deferred_audit,
                    eq_protocol_digest,
                    ep_protocol_digest,
                )?,
                None,
                None,
                None,
            )),
            Self::AcceptanceIntentAuthorization(public) => {
                let audits = (
                    public.guard_eq_credential_audit,
                    public.guard_ep_credential_audit,
                );
                let (public, private) = public.into_internal(
                    eq_deferred_audit,
                    ep_deferred_audit,
                    eq_protocol_digest,
                    ep_protocol_digest,
                )?;
                Ok((public, Some(private), None, Some(audits)))
            }
            Self::NoCommitClosure(public) => {
                let audits = (
                    public.guard_eq_credential_audit,
                    public.guard_ep_credential_audit,
                );
                let (public, private) = public.into_internal(
                    eq_deferred_audit,
                    ep_deferred_audit,
                    eq_protocol_digest,
                    ep_protocol_digest,
                )?;
                Ok((public, None, Some(private), Some(audits)))
            }
        }
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

/// Private exact-next hardware transition constrained by the commit wrapper.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaCommitWrapperPrivateGenerationWitnessV1 {
    /// Exact public lifecycle repeated inside the private relation.
    pub lifecycle: KagemushaLifecycleBindingV1,
    /// Private aggregate predecessor.
    pub predecessor: KagemushaStateV1,
    /// Private exact-next aggregate successor.
    pub successor: KagemushaStateV1,
    /// Signed receiver request, present only for `SendSplit`.
    pub request: Option<KagemushaPaymentRequestV1>,
    /// Sender one-use intent, present only for `SendSplit`.
    pub acceptance_intent: Option<KagemushaAcceptanceIntentV1>,
    /// Receiver-hardware ticket, present only for `SendSplit`.
    pub acceptance_ticket: Option<KagemushaAcceptanceTicketV1>,
    /// Exact one-use durable outbox reservation.
    pub outbox_reservation: KagemushaOutboxReservationV1,
    /// Terminal hardware commit certificate.
    pub commit_certificate: KagemushaCommitCertificateV1,
    /// Private opening of the trusted-time or monotonic-lease evidence commitment.
    pub commit_evidence_opening: KagemushaCommitEvidenceOpeningGenerationV1,
    /// Hardware-only one-use authorization for the exact private predecessor.
    pub one_use_hardware_authorization: [u8; 32],
    /// Send-only private opening of the one-use predecessor commitment.
    pub sender_one_time_opening: [u8; 32],
    /// Digest of the byte-identical terminal payment or redemption envelope.
    pub terminal_envelope_digest: [u8; 32],
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
    /// Credential validated against `hardware_profile` inside the wrapper.
    pub hardware_credential: KagemushaHardwareCredentialV1,
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaCommitWrapperPrivateGenerationWitnessV1 {
    fn into_internal(self) -> KagemushaCommitWrapperPrivateTransitionV1 {
        KagemushaCommitWrapperPrivateTransitionV1 {
            lifecycle: self.lifecycle,
            predecessor: self.predecessor,
            successor: self.successor,
            request: self.request,
            acceptance_intent: self.acceptance_intent,
            acceptance_ticket: self.acceptance_ticket,
            outbox_reservation: self.outbox_reservation,
            commit_certificate: self.commit_certificate,
            commit_evidence_opening: self.commit_evidence_opening.into_internal(),
            one_use_hardware_authorization: self.one_use_hardware_authorization,
            sender_one_time_opening: self.sender_one_time_opening,
            terminal_envelope_digest: self.terminal_envelope_digest,
            journal_revision_before: self.journal_revision_before,
            journal_revision_after: self.journal_revision_after,
            authorization_counter_before: self.authorization_counter_before,
            authorization_counter_after: self.authorization_counter_after,
            hardware_profile: self.hardware_profile,
            hardware_credential: self.hardware_credential,
        }
    }
}

/// Eq/Fp nested candidate and terminal-Guard inputs for the final wrapper.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Copy)]
pub struct KagemushaCommitWrapperEqGenerationWitnessV1<'a> {
    /// Authenticated candidate-state protocol.
    pub candidate_protocol: &'a PlonkProtocol<EqAffine>,
    /// Candidate state public instances.
    pub candidate_instances: &'a [Vec<Fp>],
    /// Candidate state proof.
    pub candidate_proof: &'a [u8],
    /// History carried by the candidate state proof.
    pub candidate_history: &'a KagemushaEqAccumulatorV1,
    /// Fold completing the candidate proof history.
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
    /// Constant-size successor history exposed by the wrapper.
    pub successor_history: &'a KagemushaEqAccumulatorV1,
}

/// Ep/Fq nested candidate and terminal-Guard inputs for the final wrapper.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Copy)]
pub struct KagemushaCommitWrapperEpGenerationWitnessV1<'a> {
    /// Authenticated candidate-state protocol.
    pub candidate_protocol: &'a PlonkProtocol<EpAffine>,
    /// Candidate state public instances.
    pub candidate_instances: &'a [Vec<Fq>],
    /// Candidate state proof.
    pub candidate_proof: &'a [u8],
    /// History carried by the candidate state proof.
    pub candidate_history: &'a KagemushaEpAccumulatorV1,
    /// Fold completing the candidate proof history.
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
    /// Constant-size successor history exposed by the wrapper.
    pub successor_history: &'a KagemushaEpAccumulatorV1,
}

/// Complete generation input for both mutually audited commit-wrapper parities.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone)]
pub struct KagemushaCommitWrapperGenerationWitnessV1<'a> {
    /// Unlinkable terminal or pre-ticket authorization projection.
    pub public: KagemushaCommitWrapperGenerationPublicV1,
    /// Private predecessor reservation or terminal hardware/state transition.
    pub private_transition: KagemushaCommitWrapperPrivateGenerationWitnessV1,
    /// Complete private Guard relation recursively authenticated by the wrapper.
    pub terminal_guard_relation: KagemushaGuardBundleRelationWitnessV1,
    /// Sorted nonzero-prefix release-enabled profile IDs with canonical zero padding.
    pub enabled_hardware_profiles: [[u8; 32]; KAGEMUSHA_COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
    /// Eq/Fp nested proof inputs.
    pub eq: KagemushaCommitWrapperEqGenerationWitnessV1<'a>,
    /// Ep/Fq nested proof inputs.
    pub ep: KagemushaCommitWrapperEpGenerationWitnessV1<'a>,
}

/// Generated key material and protocol identities for the final commit wrapper.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug)]
pub struct KagemushaGeneratedCommitWrapperArtifactsV1 {
    /// Canonical Eq transparent IPA parameters used during key generation.
    pub eq_parameters: Arc<[u8]>,
    /// Canonical Ep transparent IPA parameters used during key generation.
    pub ep_parameters: Arc<[u8]>,
    /// Processed Eq commit-wrapper proving key.
    pub eq_proving_key: Arc<[u8]>,
    /// Processed Eq commit-wrapper verifying key.
    pub eq_verifying_key: Arc<[u8]>,
    /// Processed Ep commit-wrapper proving key.
    pub ep_proving_key: Arc<[u8]>,
    /// Processed Ep commit-wrapper verifying key.
    pub ep_verifying_key: Arc<[u8]>,
    /// Exact Eq circuit layout.
    pub eq_circuit_params: BaseCircuitParams,
    /// Exact Ep circuit layout.
    pub ep_circuit_params: BaseCircuitParams,
    /// Compiled Eq wrapper protocol digest.
    pub eq_protocol_digest: [u8; 32],
    /// Compiled Ep wrapper protocol digest.
    pub ep_protocol_digest: [u8; 32],
    /// Exact sorted release-enabled profile constants committed by both verifying keys.
    pub enabled_hardware_profiles: [[u8; 32]; KAGEMUSHA_COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
}

/// Loaded authenticated Eq commit-wrapper parameters and keys.
#[cfg(feature = "zk-halo2-ipa")]
pub struct KagemushaLoadedEqCommitWrapperArtifactsV1 {
    /// Canonical Eq transparent IPA parameters.
    pub parameters: ParamsIPA<EqAffine>,
    /// Exact processed Eq commit-wrapper proving key.
    pub proving_key: ProvingKey<EqAffine>,
    /// Exact processed Eq commit-wrapper verifying key.
    pub verifying_key: VerifyingKey<EqAffine>,
    /// Authenticated circuit layout.
    pub circuit_params: BaseCircuitParams,
    /// Compiled protocol identity for this verifying key.
    pub protocol_digest: [u8; 32],
    /// Authenticated release that owns every loaded role.
    pub release_id: [u8; 32],
    /// Authenticated circuit-profile digest for the release.
    pub profile_digest: [u8; 32],
    /// Digest of the complete authenticated artifact inventory.
    pub artifact_manifest_digest: [u8; 32],
    /// Release-wide proof suite admitted by every enabled profile.
    pub suite_id: [u8; 32],
    /// Digest of the complete release-pinned verifier set.
    pub vk_digest: [u8; 32],
    /// Exact sorted enabled-profile constants committed by this verifying key.
    pub enabled_hardware_profiles: [[u8; 32]; KAGEMUSHA_COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
}

/// Loaded authenticated Ep commit-wrapper parameters and keys.
#[cfg(feature = "zk-halo2-ipa")]
pub struct KagemushaLoadedEpCommitWrapperArtifactsV1 {
    /// Canonical Ep transparent IPA parameters.
    pub parameters: ParamsIPA<EpAffine>,
    /// Exact processed Ep commit-wrapper proving key.
    pub proving_key: ProvingKey<EpAffine>,
    /// Exact processed Ep commit-wrapper verifying key.
    pub verifying_key: VerifyingKey<EpAffine>,
    /// Authenticated circuit layout.
    pub circuit_params: BaseCircuitParams,
    /// Compiled protocol identity for this verifying key.
    pub protocol_digest: [u8; 32],
    /// Authenticated release that owns every loaded role.
    pub release_id: [u8; 32],
    /// Authenticated circuit-profile digest for the release.
    pub profile_digest: [u8; 32],
    /// Digest of the complete authenticated artifact inventory.
    pub artifact_manifest_digest: [u8; 32],
    /// Release-wide proof suite admitted by every enabled profile.
    pub suite_id: [u8; 32],
    /// Digest of the complete release-pinned verifier set.
    pub vk_digest: [u8; 32],
    /// Exact sorted enabled-profile constants committed by this verifying key.
    pub enabled_hardware_profiles: [[u8; 32]; KAGEMUSHA_COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
}

/// Branch-specific proof envelope produced by the sole CommitWrapper circuit family.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KagemushaGeneratedCommitWrapperEnvelopeV1 {
    /// Final payment or redemption proof carrying terminal certificate bindings.
    Terminal(KagemushaCommitWrapperProofV1),
    /// Pre-ticket paired proof of a qualified one-use sender reservation.
    AcceptanceIntentAuthorization(KagemushaPairedProofV1),
    /// Self-contained proof of hardware-authorized no-commit closure.
    NoCommitClosure(KagemushaNoCommitClosureV1),
}

/// Generated constant-size final wrapper proof and its recursive carry material.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaGeneratedCommitWrapperProofV1 {
    /// Exact Eq public column (81 field elements).
    pub eq_public_instances: Vec<Fp>,
    /// Exact Ep public column (81 field elements).
    pub ep_public_instances: Vec<Fq>,
    /// Compact branch-specific public proof envelope.
    pub proof: KagemushaGeneratedCommitWrapperEnvelopeV1,
    /// Eq current opening claim extracted from the wrapper proof.
    pub eq_current_accumulator: KagemushaEqAccumulatorV1,
    /// Ep current opening claim extracted from the wrapper proof.
    pub ep_current_accumulator: KagemushaEpAccumulatorV1,
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaGeneratedCommitWrapperArtifactsV1 {
    /// Return the four exact CommitWrapper key bindings authenticated by a release.
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

    /// Install parameters and all four content-addressed CommitWrapper keys.
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

/// Lower an authenticated release's enabled profiles into the fixed wrapper-key table.
///
/// # Errors
///
/// Returns an error if the authenticated release does not expose a nonempty, strictly sorted,
/// distinct profile prefix or if its profiles do not share one proof suite and verifier set.
#[cfg(feature = "zk-halo2-ipa")]
pub fn kagemusha_commit_wrapper_enabled_profile_table_v1(
    release: &KagemushaAuthenticatedReleaseV1,
) -> Result<
    [[u8; 32]; KAGEMUSHA_COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
    KagemushaArtifactGenerationErrorV1,
> {
    let profiles = release.enabled_profiles();
    if profiles.is_empty() || profiles.len() > KAGEMUSHA_COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "authenticated release has an invalid enabled-profile count".to_owned(),
        ));
    }
    let suite_id = profiles[0].suite_id;
    let vk_digest = profiles[0].vk_digest;
    let mut previous = None;
    let mut table = [[0; 32]; KAGEMUSHA_COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1];
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
    pub enabled_hardware_profiles: [[u8; 32]; KAGEMUSHA_COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
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
    /// Enabled-profile constants committed by both keys.
    pub enabled_hardware_profiles: [[u8; 32]; KAGEMUSHA_COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaGeneratedMintAuthorizationArtifactsV1 {
    /// Return the four dedicated key bindings authenticated by a release.
    #[must_use]
    pub fn bindings(&self) -> [KagemushaArtifactBindingV1; 4] {
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
        ]
    }

    /// Install the shared parameters and four content-addressed keys.
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
    pub enabled_hardware_profiles: [[u8; 32]; KAGEMUSHA_COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
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
    pub enabled_hardware_profiles: [[u8; 32]; KAGEMUSHA_COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
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
    /// Return the six exact release-manifest bindings for both carrier parities.
    #[must_use]
    pub fn bindings(&self) -> [KagemushaArtifactBindingV1; 6] {
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
        ] {
            resolver.insert(Arc::clone(bytes));
        }
    }
}

/// Deterministic generation or exact key-decoding failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum KagemushaArtifactGenerationErrorV1 {
    /// Authenticated artifact resolution failed before key parsing.
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

/// Generate both stable mint-authority carrier key pairs from one fixed-shape witness.
///
/// The generated verifying keys do not contain a validator roster. Roster authority is carried
/// recursively from the release-pinned genesis identifier, so later epoch rotation does not
/// require a new proof release.
///
/// # Errors
///
/// Returns an error for an invalid carrier witness, circuit profile, key, artifact length, or
/// protocol-role alias.
#[cfg(feature = "zk-halo2-ipa")]
pub fn generate_kagemusha_mint_authority_artifacts_v1(
    mut witness: KagemushaMintAuthorityGenerationWitnessV1<'_>,
) -> Result<KagemushaGeneratedMintAuthorityArtifactsV1, KagemushaArtifactGenerationErrorV1> {
    let genesis_roster_id = witness.genesis_roster_id;
    witness.eq_deferred_audit = [1; 32];
    witness.ep_deferred_audit = [2; 32];
    let eq_parameters = ParamsIPA::<EqAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let ep_parameters = ParamsIPA::<EpAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let (eq_circuit, ep_circuit, _, _) =
        build_mint_authority_generation_pair(&eq_parameters, &ep_parameters, witness)?;
    let eq_circuit_params = eq_circuit.params();
    let ep_circuit_params = ep_circuit.params();
    validate_recursive_profile(KagemushaPastaParityV1::Eq, &eq_circuit_params)?;
    validate_recursive_profile(KagemushaPastaParityV1::Ep, &ep_circuit_params)?;
    let eq_vk = keygen_vk(&eq_parameters, &eq_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Eq,
            kind: "mint-authority verifying key",
            reason: error.to_string(),
        }
    })?;
    let eq_pk = keygen_pk(&eq_parameters, eq_vk.clone(), &eq_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Eq,
            kind: "mint-authority proving key",
            reason: error.to_string(),
        }
    })?;
    let ep_vk = keygen_vk(&ep_parameters, &ep_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Ep,
            kind: "mint-authority verifying key",
            reason: error.to_string(),
        }
    })?;
    let ep_pk = keygen_pk(&ep_parameters, ep_vk.clone(), &ep_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Ep,
            kind: "mint-authority proving key",
            reason: error.to_string(),
        }
    })?;
    let eq_protocol = compile(
        &eq_parameters,
        &eq_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_protocol = compile(
        &ep_parameters,
        &ep_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
    );
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Eq,
        "mint-authority carrier",
        &eq_protocol,
    )?;
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Ep,
        "mint-authority carrier",
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
            "Eq and Ep mint-authority protocol identities alias".to_owned(),
        ));
    }
    let (eq_parameter_bytes, eq_proving_key, eq_verifying_key) =
        build_generated_mint_parity(KagemushaPastaParityV1::Eq, &eq_parameters, &eq_pk, &eq_vk)?;
    let (ep_parameter_bytes, ep_proving_key, ep_verifying_key) =
        build_generated_mint_parity(KagemushaPastaParityV1::Ep, &ep_parameters, &ep_pk, &ep_vk)?;
    Ok(KagemushaGeneratedMintAuthorityArtifactsV1 {
        eq_parameters: eq_parameter_bytes,
        ep_parameters: ep_parameter_bytes,
        eq_proving_key,
        ep_proving_key,
        eq_verifying_key,
        ep_verifying_key,
        eq_circuit_params,
        ep_circuit_params,
        eq_protocol_digest,
        ep_protocol_digest,
        genesis_roster_id,
    })
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
    circuit_params: BaseCircuitParams,
) -> Result<KagemushaLoadedEqMintAuthorityArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    R: KagemushaArtifactByteResolverV1,
{
    validate_recursive_profile(KagemushaPastaParityV1::Eq, &circuit_params)?;
    let parameters = artifacts.load_eq_params()?;
    let vk_bytes = artifacts.resolve(KagemushaArtifactRoleV1::MintCreditVkEq)?;
    let pk_bytes = artifacts.resolve(KagemushaArtifactRoleV1::MintCreditPkEq)?;
    let verifying_key = read_eq_mint_vk(vk_bytes.as_ref(), circuit_params.clone())?;
    let proving_key = read_eq_mint_pk(pk_bytes.as_ref(), circuit_params.clone())?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Eq,
        &proving_key,
        vk_bytes.as_ref(),
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
    Ok(KagemushaLoadedEqMintAuthorityArtifactsV1 {
        parameters,
        proving_key,
        verifying_key,
        circuit_params,
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
    circuit_params: BaseCircuitParams,
) -> Result<KagemushaLoadedEpMintAuthorityArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    R: KagemushaArtifactByteResolverV1,
{
    validate_recursive_profile(KagemushaPastaParityV1::Ep, &circuit_params)?;
    let parameters = artifacts.load_ep_params()?;
    let vk_bytes = artifacts.resolve(KagemushaArtifactRoleV1::MintCreditVkEp)?;
    let pk_bytes = artifacts.resolve(KagemushaArtifactRoleV1::MintCreditPkEp)?;
    let verifying_key = read_ep_mint_vk(vk_bytes.as_ref(), circuit_params.clone())?;
    let proving_key = read_ep_mint_pk(pk_bytes.as_ref(), circuit_params.clone())?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Ep,
        &proving_key,
        vk_bytes.as_ref(),
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
    Ok(KagemushaLoadedEpMintAuthorityArtifactsV1 {
        parameters,
        proving_key,
        verifying_key,
        circuit_params,
    })
}

/// Prove one bootstrap, roster-rotation, or finalized-mint authority step.
///
/// The returned paired proof is directly consumable by `KagemushaMintCreditV1` for the
/// `FinalizedMint` branch. Bootstrap and rotation results are retained by the finality authority
/// worker as the predecessor proof for later steps.
///
/// # Errors
///
/// Rejects invalid certificate/authority material, wrong authenticated key profiles, protocol
/// substitution, proof-generation failure, or a proof exceeding the transport bound.
#[cfg(feature = "zk-halo2-ipa")]
pub fn prove_kagemusha_mint_authority_v1(
    eq: &KagemushaLoadedEqMintAuthorityArtifactsV1,
    ep: &KagemushaLoadedEpMintAuthorityArtifactsV1,
    mut witness: KagemushaMintAuthorityGenerationWitnessV1<'_>,
) -> Result<KagemushaGeneratedMintAuthorityProofV1, KagemushaArtifactGenerationErrorV1> {
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
        KagemushaMintAuthorityStepV1::Bootstrap
        | KagemushaMintAuthorityStepV1::FinalizedMint => roster_id,
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
        build_mint_authority_generation_pair(&eq.parameters, &ep.parameters, witness.clone())?;
    witness.eq_deferred_audit = eq_deferred_audit;
    witness.ep_deferred_audit = ep_deferred_audit;
    let eq_history = witness.eq_successor_history.clone();
    let ep_history = witness.ep_successor_history.clone();
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
    let actual_eq_protocol =
        native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let actual_ep_protocol =
        native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if eq_protocol_digest != actual_eq_protocol || ep_protocol_digest != actual_ep_protocol {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "mint-authority witness protocol differs from authenticated proving key".to_owned(),
        ));
    }
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
    let (eq_circuit, ep_circuit, rebuilt_eq_audit, rebuilt_ep_audit) =
        build_mint_authority_generation_pair(&eq.parameters, &ep.parameters, witness)?;
    if rebuilt_eq_audit != eq_deferred_audit || rebuilt_ep_audit != ep_deferred_audit {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "mint-authority deferred audit changed while binding its public cells".to_owned(),
        ));
    }
    if !same_base_params(&eq_circuit.params(), &eq.circuit_params) {
        return Err(
            KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
                KagemushaPastaParityV1::Eq,
            ),
        );
    }
    if !same_base_params(&ep_circuit.params(), &ep.circuit_params) {
        return Err(
            KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
                KagemushaPastaParityV1::Ep,
            ),
        );
    }
    let eq_proof = create_mint_eq_proof(eq, eq_circuit, &eq_instances)?;
    let ep_proof = create_mint_ep_proof(ep, ep_circuit, &ep_instances)?;
    validate_recursive_proof_length(KagemushaPastaParityV1::Eq, &eq_proof)?;
    validate_recursive_proof_length(KagemushaPastaParityV1::Ep, &ep_proof)?;
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
    Ok(KagemushaGeneratedMintAuthorityProofV1 {
        eq_public_instances: eq_instances,
        ep_public_instances: ep_instances,
        proof: KagemushaPairedProofV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            eq_protocol_digest,
            ep_protocol_digest,
            semantic_digest,
            // Mint helper proofs use these two common slots for the exact certificate and
            // authenticated authority-head bindings. State proofs retain the Guard audit meaning.
            guard_eq_credential_audit: certificate_binding,
            guard_ep_credential_audit: authority_head,
            eq_deferred_audit,
            ep_deferred_audit,
            eq_proof,
            ep_proof,
            eq_history: eq_history.as_bytes().to_vec(),
            ep_history: ep_history.as_bytes().to_vec(),
        },
        eq_current_accumulator,
        ep_current_accumulator,
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
/// by the public bootstrap selector. The history carried out of bootstrap is instead a canonical
/// terminally decided empty IPA accumulator in each parity. This avoids a self-referential
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
    let eq_parent_proof = dummy_ordinary_proof_bytes(
        &eq_protocol,
        eq_point.as_ref(),
        KagemushaPastaParityV1::Eq,
    )?;
    let ep_parent_proof = dummy_ordinary_proof_bytes(
        &ep_protocol,
        ep_point.as_ref(),
        KagemushaPastaParityV1::Ep,
    )?;
    let eq_fold_proof =
        KagemushaEqFoldProofV1::try_from_bytes(&dummy_fold_proof_bytes(eq_point.as_ref()))
            .map_err(|error| {
                KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string())
            })?;
    let ep_fold_proof =
        KagemushaEpFoldProofV1::try_from_bytes(&dummy_fold_proof_bytes(ep_point.as_ref()))
            .map_err(|error| {
                KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string())
            })?;
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
    let eq_parent_history = KagemushaEqAccumulatorV1::try_from_bytes(
        &checkpoint.proof.eq_history,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_parent_history = KagemushaEpAccumulatorV1::try_from_bytes(
        &checkpoint.proof.ep_history,
    )
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let eq_fold =
        fold_kagemusha_eq_accumulators_v1(&eq.parameters, &eq_current, &eq_parent_history)
            .map_err(|error| {
                KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string())
            })?;
    let ep_fold =
        fold_kagemusha_ep_accumulators_v1(&ep.parameters, &ep_current, &ep_parent_history)
            .map_err(|error| {
                KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string())
            })?;
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
    let eq_incoming = witness.eq_incoming.into_composite();
    let ep_incoming = witness.ep_incoming.into_composite();
    build_kagemusha_recursive_state_pair_v1(
        eq_parameters,
        ep_parameters,
        witness.into_recursive(eq_incoming, ep_incoming),
    )
}

#[cfg(feature = "zk-halo2-ipa")]
/// Generate the paired Pasta recursive-state proving and verification artifacts.
pub fn generate_kagemusha_recursive_state_artifacts_v1(
    witness: KagemushaRecursiveStateGenerationWitnessV1<'_>,
) -> Result<KagemushaGeneratedRecursiveStateArtifactsV1, KagemushaArtifactGenerationErrorV1> {
    let eq_parameters = ParamsIPA::<EqAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let ep_parameters = ParamsIPA::<EpAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let (eq_circuit, ep_circuit, _, _) =
        build_recursive_generation_pair_v1(&eq_parameters, &ep_parameters, witness)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let eq_circuit_params = eq_circuit.params();
    let ep_circuit_params = ep_circuit.params();
    validate_recursive_profile(KagemushaPastaParityV1::Eq, &eq_circuit_params)?;
    validate_recursive_profile(KagemushaPastaParityV1::Ep, &ep_circuit_params)?;

    let eq_vk = keygen_vk(&eq_parameters, &eq_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Eq,
            kind: "recursive state verifying key",
            reason: error.to_string(),
        }
    })?;
    let eq_pk = keygen_pk(&eq_parameters, eq_vk.clone(), &eq_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Eq,
            kind: "recursive state proving key",
            reason: error.to_string(),
        }
    })?;
    let ep_vk = keygen_vk(&ep_parameters, &ep_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Ep,
            kind: "recursive state verifying key",
            reason: error.to_string(),
        }
    })?;
    let ep_pk = keygen_pk(&ep_parameters, ep_vk.clone(), &ep_circuit).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity: KagemushaPastaParityV1::Ep,
            kind: "recursive state proving key",
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
        "recursive aggregate state",
        &eq_protocol,
    )?;
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Ep,
        "recursive aggregate state",
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
            "Eq and Ep recursive state protocol identities alias".to_owned(),
        ));
    }

    Ok(KagemushaGeneratedRecursiveStateArtifactsV1 {
        eq: build_generated(KagemushaPastaParityV1::Eq, &eq_parameters, &eq_pk, &eq_vk)?,
        ep: build_generated(KagemushaPastaParityV1::Ep, &ep_parameters, &ep_pk, &ep_vk)?,
        eq_circuit_params,
        ep_circuit_params,
        eq_protocol_digest,
        ep_protocol_digest,
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
) -> Result<KagemushaLoadedEqRecursiveStateArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    R: KagemushaArtifactByteResolverV1,
{
    validate_recursive_profile(KagemushaPastaParityV1::Eq, &circuit_params)?;
    let parameters = artifacts.load_eq_params()?;
    let verifying_bytes = artifacts.resolve(KagemushaArtifactRoleV1::StateVkEq)?;
    let proving_bytes = artifacts.resolve(KagemushaArtifactRoleV1::StatePkEq)?;
    let verifying_key = read_eq_recursive_vk(verifying_bytes.as_ref(), circuit_params.clone())?;
    let proving_key = read_eq_recursive_pk(proving_bytes.as_ref(), circuit_params.clone())?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Eq,
        &proving_key,
        verifying_bytes.as_ref(),
    )?;
    let protocol = compile(
        &parameters,
        &verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![recursive_public_instance_count()]),
    );
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Eq,
        "recursive aggregate state",
        &protocol,
    )?;
    Ok(KagemushaLoadedEqRecursiveStateArtifactsV1 {
        parameters,
        proving_key,
        verifying_key,
        circuit_params,
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
) -> Result<KagemushaLoadedEpRecursiveStateArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    R: KagemushaArtifactByteResolverV1,
{
    validate_recursive_profile(KagemushaPastaParityV1::Ep, &circuit_params)?;
    let parameters = artifacts.load_ep_params()?;
    let verifying_bytes = artifacts.resolve(KagemushaArtifactRoleV1::StateVkEp)?;
    let proving_bytes = artifacts.resolve(KagemushaArtifactRoleV1::StatePkEp)?;
    let verifying_key = read_ep_recursive_vk(verifying_bytes.as_ref(), circuit_params.clone())?;
    let proving_key = read_ep_recursive_pk(proving_bytes.as_ref(), circuit_params.clone())?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Ep,
        &proving_key,
        verifying_bytes.as_ref(),
    )?;
    let protocol = compile(
        &parameters,
        &verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![recursive_public_instance_count()]),
    );
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Ep,
        "recursive aggregate state",
        &protocol,
    )?;
    Ok(KagemushaLoadedEpRecursiveStateArtifactsV1 {
        parameters,
        proving_key,
        verifying_key,
        circuit_params,
    })
}

/// Produce both production recursive state proofs and their carried delayed histories.
///
/// The circuit is rebuilt from the complete private witness and its derived layout must exactly
/// match each authenticated proving key profile before Halo2 is invoked.
///
/// # Errors
///
/// Returns an error for invalid witness material, profile substitution, proof generation failure,
/// or a transcript that exceeds the fixed transport budget.
#[cfg(feature = "zk-halo2-ipa")]
pub fn prove_kagemusha_recursive_state_v1(
    eq: &KagemushaLoadedEqRecursiveStateArtifactsV1,
    ep: &KagemushaLoadedEpRecursiveStateArtifactsV1,
    mut witness: KagemushaRecursiveStateGenerationWitnessV1<'_>,
) -> Result<KagemushaGeneratedRecursiveStateProofV1, KagemushaArtifactGenerationErrorV1> {
    // Deferred audits are circuit outputs. Build the fixed shape once with nonzero placeholders,
    // read the exact assigned digests, then rebuild and prove with those public values. This is
    // the state analogue of the mint-authority prover and prevents callers from authorizing an
    // arbitrary helper/parent equation transcript through the public audit cells.
    witness.state.eq_deferred_audit = [1; 32];
    witness.state.ep_deferred_audit = [2; 32];
    let (_, _, eq_deferred_audit, ep_deferred_audit) =
        build_recursive_generation_pair_v1(&eq.parameters, &ep.parameters, witness.clone())
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
        build_recursive_generation_pair_v1(&eq.parameters, &ep.parameters, witness)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if rebuilt_eq_audit != eq_deferred_audit || rebuilt_ep_audit != ep_deferred_audit {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "recursive-state deferred audit changed while binding its public cells".to_owned(),
        ));
    }
    if !same_base_params(&eq_circuit.params(), &eq.circuit_params) {
        return Err(
            KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
                KagemushaPastaParityV1::Eq,
            ),
        );
    }
    if !same_base_params(&ep_circuit.params(), &ep.circuit_params) {
        return Err(
            KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
                KagemushaPastaParityV1::Ep,
            ),
        );
    }
    let eq_proof = create_recursive_eq_proof(eq, eq_circuit, &eq_instances)?;
    let ep_proof = create_recursive_ep_proof(ep, ep_circuit, &ep_instances)?;
    validate_recursive_proof_length(KagemushaPastaParityV1::Eq, &eq_proof)?;
    validate_recursive_proof_length(KagemushaPastaParityV1::Ep, &ep_proof)?;
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
    Ok(KagemushaGeneratedRecursiveStateProofV1 {
        eq_public_instances: eq_instances,
        ep_public_instances: ep_instances,
        eq_proof,
        ep_proof,
        eq_current_accumulator,
        ep_current_accumulator,
        eq_history,
        ep_history,
    })
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
    let (public, intent_authorization, no_commit_closure, _) = witness.public.into_internal(
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
            private_transition: witness.private_transition.into_internal(),
            intent_authorization,
            no_commit_closure,
            terminal_guard_relation: witness.terminal_guard_relation,
            enabled_hardware_profiles: witness.enabled_hardware_profiles,
            eq: KagemushaCommitWrapperEqWitnessV1 {
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
            ep: KagemushaCommitWrapperEpWitnessV1 {
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

/// Generate all four final CommitWrapper keys from one valid fixed-shape witness.
///
/// The transparent Eq/Ep parameters are returned for content-addressed installation but are not
/// additional wrapper roles; the release authenticates the four dedicated PK/VK roles.
///
/// # Errors
///
/// Returns an error for an invalid terminal witness, an 81-instance profile mismatch, key
/// generation failure, noncanonical artifact size, or aliased parity protocol identity.
#[cfg(feature = "zk-halo2-ipa")]
pub fn generate_kagemusha_commit_wrapper_artifacts_v1(
    witness: KagemushaCommitWrapperGenerationWitnessV1<'_>,
) -> Result<KagemushaGeneratedCommitWrapperArtifactsV1, KagemushaArtifactGenerationErrorV1> {
    let enabled_hardware_profiles = witness.enabled_hardware_profiles;
    let eq_parameters = ParamsIPA::<EqAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let ep_parameters = ParamsIPA::<EpAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let placeholder_eq_protocol = encode(Fp::ONE);
    let placeholder_ep_protocol = encode(Fq::from(2));
    let placeholder_eq_audit = encode(Fp::from(3));
    let placeholder_ep_audit = encode(Fq::from(4));
    let (eq_circuit, ep_circuit, _, _) = build_commit_wrapper_generation_pair_v1(
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
    validate_commit_wrapper_profile(KagemushaPastaParityV1::Eq, &eq_circuit_params)?;
    validate_commit_wrapper_profile(KagemushaPastaParityV1::Ep, &ep_circuit_params)?;
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
            .with_num_instance(vec![COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_protocol = compile(
        &ep_parameters,
        &ep_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1]),
    );
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Eq,
        "terminal commit wrapper",
        &eq_protocol,
    )?;
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Ep,
        "terminal commit wrapper",
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
            "Eq and Ep commit-wrapper protocol identities alias".to_owned(),
        ));
    }
    let (eq_parameters, eq_proving_key, eq_verifying_key) = build_generated_helper_parity(
        KagemushaPastaParityV1::Eq,
        "commit-wrapper proving key",
        &eq_parameters,
        &eq_pk,
        &eq_vk,
    )?;
    let (ep_parameters, ep_proving_key, ep_verifying_key) = build_generated_helper_parity(
        KagemushaPastaParityV1::Ep,
        "commit-wrapper proving key",
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
        enabled_hardware_profiles,
    })
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_commit_wrapper_release_alignment_v1(
    artifacts: super::KagemushaRecursionArtifactsV1,
    release: &KagemushaAuthenticatedReleaseV1,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    if artifacts.release_id != release.release_id()
        || artifacts.profile_digest != release.profile_digest()
        || artifacts.artifact_manifest_digest != release.manifest_digest()
        || artifacts.commit_wrapper_eq_protocol_digest
            != release.commit_wrapper_eq_protocol_digest()
        || artifacts.commit_wrapper_ep_protocol_digest
            != release.commit_wrapper_ep_protocol_digest()
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "CommitWrapper artifacts and authenticated release do not match".to_owned(),
        ));
    }
    Ok(())
}

/// Load and cross-check the authenticated Eq CommitWrapper PK/VK roles.
///
/// # Errors
///
/// Returns an error for an invalid profile, content-address mismatch, malformed key, trailing
/// bytes, or a proving key whose embedded verifier differs from the authenticated VK.
#[cfg(feature = "zk-halo2-ipa")]
pub fn load_kagemusha_eq_commit_wrapper_artifacts_v1<R>(
    artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
    release: &KagemushaAuthenticatedReleaseV1,
    circuit_params: BaseCircuitParams,
) -> Result<KagemushaLoadedEqCommitWrapperArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    R: KagemushaArtifactByteResolverV1,
{
    validate_commit_wrapper_profile(KagemushaPastaParityV1::Eq, &circuit_params)?;
    let recursion_release = artifacts.recursion_artifacts();
    validate_commit_wrapper_release_alignment_v1(recursion_release, release)?;
    let enabled_hardware_profiles = kagemusha_commit_wrapper_enabled_profile_table_v1(release)?;
    let suite_id = release.enabled_profiles()[0].suite_id;
    let vk_digest = release.vk_set_digest();
    let parameters = artifacts.load_eq_params()?;
    let verifying_bytes = artifacts.resolve(KagemushaArtifactRoleV1::CommitWrapperVkEq)?;
    let proving_bytes = artifacts.resolve(KagemushaArtifactRoleV1::CommitWrapperPkEq)?;
    let verifying_key =
        read_eq_commit_wrapper_vk(verifying_bytes.as_ref(), circuit_params.clone())?;
    let proving_key = read_eq_commit_wrapper_pk(proving_bytes.as_ref(), circuit_params.clone())?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Eq,
        &proving_key,
        verifying_bytes.as_ref(),
    )?;
    let protocol = compile(
        &parameters,
        &verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1]),
    );
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Eq,
        "terminal commit wrapper",
        &protocol,
    )?;
    let protocol_digest = native_parent_protocol_digest_v1(&protocol, KagemushaPastaParityV1::Eq)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if protocol_digest != recursion_release.commit_wrapper_eq_protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "authenticated Eq commit-wrapper protocol digest does not match its verifying key"
                .to_owned(),
        ));
    }
    Ok(KagemushaLoadedEqCommitWrapperArtifactsV1 {
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

/// Load and cross-check the authenticated Ep CommitWrapper PK/VK roles.
///
/// # Errors
///
/// Returns an error for an invalid profile, content-address mismatch, malformed key, trailing
/// bytes, or a proving key whose embedded verifier differs from the authenticated VK.
#[cfg(feature = "zk-halo2-ipa")]
pub fn load_kagemusha_ep_commit_wrapper_artifacts_v1<R>(
    artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
    release: &KagemushaAuthenticatedReleaseV1,
    circuit_params: BaseCircuitParams,
) -> Result<KagemushaLoadedEpCommitWrapperArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    R: KagemushaArtifactByteResolverV1,
{
    validate_commit_wrapper_profile(KagemushaPastaParityV1::Ep, &circuit_params)?;
    let recursion_release = artifacts.recursion_artifacts();
    validate_commit_wrapper_release_alignment_v1(recursion_release, release)?;
    let enabled_hardware_profiles = kagemusha_commit_wrapper_enabled_profile_table_v1(release)?;
    let suite_id = release.enabled_profiles()[0].suite_id;
    let vk_digest = release.vk_set_digest();
    let parameters = artifacts.load_ep_params()?;
    let verifying_bytes = artifacts.resolve(KagemushaArtifactRoleV1::CommitWrapperVkEp)?;
    let proving_bytes = artifacts.resolve(KagemushaArtifactRoleV1::CommitWrapperPkEp)?;
    let verifying_key =
        read_ep_commit_wrapper_vk(verifying_bytes.as_ref(), circuit_params.clone())?;
    let proving_key = read_ep_commit_wrapper_pk(proving_bytes.as_ref(), circuit_params.clone())?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Ep,
        &proving_key,
        verifying_bytes.as_ref(),
    )?;
    let protocol = compile(
        &parameters,
        &verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1]),
    );
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Ep,
        "terminal commit wrapper",
        &protocol,
    )?;
    let protocol_digest = native_parent_protocol_digest_v1(&protocol, KagemushaPastaParityV1::Ep)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if protocol_digest != recursion_release.commit_wrapper_ep_protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "authenticated Ep commit-wrapper protocol digest does not match its verifying key"
                .to_owned(),
        ));
    }
    Ok(KagemushaLoadedEpCommitWrapperArtifactsV1 {
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

/// Produce the final constant-size paired CommitWrapper proof.
///
/// Deferred audits are derived from the mutually audited circuits, then rebound into the public
/// 81-instance columns before proving. Histories retain their fixed byte length independent of
/// transaction history.
///
/// # Errors
///
/// Returns an error for invalid terminal material, profile/protocol substitution, proof failure,
/// or an envelope exceeding the fixed wrapper transport budget.
#[cfg(feature = "zk-halo2-ipa")]
pub fn prove_kagemusha_commit_wrapper_v1(
    eq: &KagemushaLoadedEqCommitWrapperArtifactsV1,
    ep: &KagemushaLoadedEpCommitWrapperArtifactsV1,
    witness: KagemushaCommitWrapperGenerationWitnessV1<'_>,
) -> Result<KagemushaGeneratedCommitWrapperProofV1, KagemushaArtifactGenerationErrorV1> {
    let generation_public = witness.public.clone();
    validate_commit_wrapper_profile(KagemushaPastaParityV1::Eq, &eq.circuit_params)?;
    validate_commit_wrapper_profile(KagemushaPastaParityV1::Ep, &ep.circuit_params)?;
    if eq.release_id != ep.release_id
        || eq.profile_digest != ep.profile_digest
        || eq.artifact_manifest_digest != ep.artifact_manifest_digest
        || eq.suite_id != ep.suite_id
        || eq.vk_digest != ep.vk_digest
        || eq.enabled_hardware_profiles != ep.enabled_hardware_profiles
        || witness.enabled_hardware_profiles != eq.enabled_hardware_profiles
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "commit-wrapper parities and witness do not belong to one authenticated release"
                .to_owned(),
        ));
    }
    witness
        .public
        .validate_against_loaded_release(
            eq.release_id,
            eq.suite_id,
            eq.vk_digest,
            eq.artifact_manifest_digest,
            &eq.enabled_hardware_profiles,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if eq.protocol_digest == ep.protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "Eq and Ep loaded commit-wrapper protocol identities alias".to_owned(),
        ));
    }
    let placeholder_eq_audit = encode(Fp::from(3));
    let placeholder_ep_audit = encode(Fq::from(4));
    let (_, _, eq_deferred_audit, ep_deferred_audit) = build_commit_wrapper_generation_pair_v1(
        &eq.parameters,
        &ep.parameters,
        witness.clone(),
        placeholder_eq_audit,
        placeholder_ep_audit,
        eq.protocol_digest,
        ep.protocol_digest,
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let (public, _, _, credential_audits) = witness
        .public
        .clone()
        .into_internal(
            eq_deferred_audit,
            ep_deferred_audit,
            eq.protocol_digest,
            ep.protocol_digest,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let eq_instances =
        commit_wrapper_public_instances::<Fp>(&public, witness.eq.successor_history.as_bytes())?;
    let ep_instances =
        commit_wrapper_public_instances::<Fq>(&public, witness.ep.successor_history.as_bytes())?;
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
            "commit-wrapper deferred audit changed while binding its public cells".to_owned(),
        ));
    }
    if !same_base_params(&eq_circuit.params(), &eq.circuit_params) {
        return Err(
            KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
                KagemushaPastaParityV1::Eq,
            ),
        );
    }
    if !same_base_params(&ep_circuit.params(), &ep.circuit_params) {
        return Err(
            KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
                KagemushaPastaParityV1::Ep,
            ),
        );
    }
    let eq_proof = create_commit_wrapper_eq_proof(eq, eq_circuit, &eq_instances)?;
    let ep_proof = create_commit_wrapper_ep_proof(ep, ep_circuit, &ep_instances)?;
    validate_commit_wrapper_proof_length(KagemushaPastaParityV1::Eq, &eq_proof)?;
    validate_commit_wrapper_proof_length(KagemushaPastaParityV1::Ep, &ep_proof)?;
    let eq_protocol = compile(
        &eq.parameters,
        &eq.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_protocol = compile(
        &ep.parameters,
        &ep.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let actual_eq_protocol =
        native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let actual_ep_protocol =
        native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if actual_eq_protocol != eq.protocol_digest || actual_ep_protocol != ep.protocol_digest {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "loaded commit-wrapper protocol digest changed before proving".to_owned(),
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
    let (proof, encoded) = match generation_public {
        KagemushaCommitWrapperGenerationPublicV1::AcceptanceIntentAuthorization(_) => {
            let (guard_eq_credential_audit, guard_ep_credential_audit) = credential_audits
                .ok_or_else(|| {
                    KagemushaArtifactGenerationErrorV1::CircuitBuild(
                        "intent authorization credential audits are absent".to_owned(),
                    )
                })?;
            let proof = KagemushaPairedProofV1 {
                version: KAGEMUSHA_WIRE_VERSION_V1,
                eq_protocol_digest: eq.protocol_digest,
                ep_protocol_digest: ep.protocol_digest,
                semantic_digest: public.semantic_digest,
                guard_eq_credential_audit,
                guard_ep_credential_audit,
                eq_deferred_audit,
                ep_deferred_audit,
                eq_proof,
                ep_proof,
                eq_history: eq_history.as_bytes().to_vec(),
                ep_history: ep_history.as_bytes().to_vec(),
            };
            proof
                .validate_shape_for_semantic_digest(public.semantic_digest)
                .map_err(|error| {
                    KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string())
                })?;
            let encoded = norito::encode_canonical(&proof).map_err(|error| {
                KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string())
            })?;
            (
                KagemushaGeneratedCommitWrapperEnvelopeV1::AcceptanceIntentAuthorization(proof),
                encoded,
            )
        }
        KagemushaCommitWrapperGenerationPublicV1::NoCommitClosure(closure_public) => {
            let (guard_eq_credential_audit, guard_ep_credential_audit) = credential_audits
                .ok_or_else(|| {
                    KagemushaArtifactGenerationErrorV1::CircuitBuild(
                        "no-commit closure credential audits are absent".to_owned(),
                    )
                })?;
            let paired_proof = KagemushaPairedProofV1 {
                version: KAGEMUSHA_WIRE_VERSION_V1,
                eq_protocol_digest: eq.protocol_digest,
                ep_protocol_digest: ep.protocol_digest,
                semantic_digest: public.semantic_digest,
                guard_eq_credential_audit,
                guard_ep_credential_audit,
                eq_deferred_audit,
                ep_deferred_audit,
                eq_proof,
                ep_proof,
                eq_history: eq_history.as_bytes().to_vec(),
                ep_history: ep_history.as_bytes().to_vec(),
            };
            paired_proof
                .validate_shape_for_semantic_digest(public.semantic_digest)
                .map_err(|error| {
                    KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string())
                })?;
            let closure = KagemushaNoCommitClosureV1 {
                version: KAGEMUSHA_WIRE_VERSION_V1,
                statement: closure_public.statement,
                request: closure_public.request,
                intent_authorization: closure_public.intent_authorization,
                acceptance_ticket: closure_public.acceptance_ticket,
                proof: paired_proof,
            };
            closure.validate_shape().map_err(|error| {
                KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string())
            })?;
            let encoded_proof = norito::encode_canonical(&closure.proof).map_err(|error| {
                KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string())
            })?;
            (
                KagemushaGeneratedCommitWrapperEnvelopeV1::NoCommitClosure(closure),
                encoded_proof,
            )
        }
        KagemushaCommitWrapperGenerationPublicV1::Terminal(_) => {
            if credential_audits.is_some() {
                return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
                    "terminal wrapper unexpectedly exposes credential audits".to_owned(),
                ));
            }
            let proof = KagemushaCommitWrapperProofV1 {
                version: KAGEMUSHA_WIRE_VERSION_V1,
                eq_protocol_digest: eq.protocol_digest,
                ep_protocol_digest: ep.protocol_digest,
                semantic_digest: public.semantic_digest,
                candidate_envelope_digest: public.candidate_envelope_digest,
                commit_certificate_digest: public.commit_certificate_digest,
                eq_deferred_audit,
                ep_deferred_audit,
                eq_proof,
                ep_proof,
                eq_history: eq_history.as_bytes().to_vec(),
                ep_history: ep_history.as_bytes().to_vec(),
            };
            let encoded = norito::encode_canonical(&proof).map_err(|error| {
                KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string())
            })?;
            (
                KagemushaGeneratedCommitWrapperEnvelopeV1::Terminal(proof),
                encoded,
            )
        }
    };
    if encoded.len()
        > usize::try_from(KAGEMUSHA_COMMIT_WRAPPER_PROOF_MAX_BYTES_V1).unwrap_or(usize::MAX)
    {
        return Err(KagemushaArtifactGenerationErrorV1::InvalidLength {
            parity: KagemushaPastaParityV1::Eq,
            kind: "paired commit wrapper proof",
            actual: u64::try_from(encoded.len()).unwrap_or(u64::MAX),
        });
    }
    Ok(KagemushaGeneratedCommitWrapperProofV1 {
        eq_public_instances: eq_instances,
        ep_public_instances: ep_instances,
        proof,
        eq_current_accumulator,
        ep_current_accumulator,
    })
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

/// Generate the four dedicated MintAuthorization keys from one fixed-shape witness.
///
/// # Errors
///
/// Returns an error for an invalid relation/profile table, circuit-profile mismatch, key
/// generation failure, oversized artifact, or aliased parity protocol identity.
#[cfg(feature = "zk-halo2-ipa")]
pub fn generate_kagemusha_mint_authorization_artifacts_v1(
    witness: KagemushaMintAuthorizationGenerationWitnessV1<'_>,
) -> Result<KagemushaGeneratedMintAuthorizationArtifactsV1, KagemushaArtifactGenerationErrorV1>
{
    let enabled_hardware_profiles = witness.enabled_hardware_profiles;
    let eq_parameters = ParamsIPA::<EqAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let ep_parameters = ParamsIPA::<EpAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let (eq_circuit, ep_circuit, _, _) = build_mint_authorization_generation_pair_v1(
        &eq_parameters,
        &ep_parameters,
        witness,
        encode(Fp::from(3)),
        encode(Fq::from(4)),
    )?;
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
) -> Result<KagemushaLoadedEqMintAuthorizationArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    R: KagemushaArtifactByteResolverV1,
{
    validate_recursive_profile(KagemushaPastaParityV1::Eq, &circuit_params)?;
    let recursion_release = artifacts.recursion_artifacts();
    validate_mint_authorization_release_alignment_v1(recursion_release, release)?;
    let helper = release
        .helper_protocol(KagemushaQualifiedHelperCircuitV1::MintAuthorization)
        .expect("release alignment established helper presence");
    let enabled_hardware_profiles = kagemusha_commit_wrapper_enabled_profile_table_v1(release)?;
    let parameters = artifacts.load_eq_params()?;
    let verifying_bytes = artifacts.resolve(KagemushaArtifactRoleV1::MintAuthorizationVkEq)?;
    let proving_bytes = artifacts.resolve(KagemushaArtifactRoleV1::MintAuthorizationPkEq)?;
    let verifying_key =
        read_eq_mint_authorization_vk(verifying_bytes.as_ref(), circuit_params.clone())?;
    let proving_key =
        read_eq_mint_authorization_pk(proving_bytes.as_ref(), circuit_params.clone())?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Eq,
        &proving_key,
        verifying_bytes.as_ref(),
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
) -> Result<KagemushaLoadedEpMintAuthorizationArtifactsV1, KagemushaArtifactGenerationErrorV1>
where
    R: KagemushaArtifactByteResolverV1,
{
    validate_recursive_profile(KagemushaPastaParityV1::Ep, &circuit_params)?;
    let recursion_release = artifacts.recursion_artifacts();
    validate_mint_authorization_release_alignment_v1(recursion_release, release)?;
    let helper = release
        .helper_protocol(KagemushaQualifiedHelperCircuitV1::MintAuthorization)
        .expect("release alignment established helper presence");
    let enabled_hardware_profiles = kagemusha_commit_wrapper_enabled_profile_table_v1(release)?;
    let parameters = artifacts.load_ep_params()?;
    let verifying_bytes = artifacts.resolve(KagemushaArtifactRoleV1::MintAuthorizationVkEp)?;
    let proving_bytes = artifacts.resolve(KagemushaArtifactRoleV1::MintAuthorizationPkEp)?;
    let verifying_key =
        read_ep_mint_authorization_vk(verifying_bytes.as_ref(), circuit_params.clone())?;
    let proving_key =
        read_ep_mint_authorization_pk(proving_bytes.as_ref(), circuit_params.clone())?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Ep,
        &proving_key,
        verifying_bytes.as_ref(),
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
    witness
        .relation
        .validate_shape()
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let recipient_credential_commitment = context.recipient_credential_commitment;
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
        &eq.parameters,
        &ep.parameters,
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
            &eq.parameters,
            &ep.parameters,
            witness,
            eq_deferred_audit,
            ep_deferred_audit,
        )?;
    if rebuilt_eq_audit != eq_deferred_audit || rebuilt_ep_audit != ep_deferred_audit {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "MintAuthorization deferred audit changed while binding public cells".to_owned(),
        ));
    }
    if !same_base_params(&eq_circuit.params(), &eq.circuit_params) {
        return Err(
            KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
                KagemushaPastaParityV1::Eq,
            ),
        );
    }
    if !same_base_params(&ep_circuit.params(), &ep.circuit_params) {
        return Err(
            KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
                KagemushaPastaParityV1::Ep,
            ),
        );
    }
    let eq_proof = create_mint_authorization_eq_proof(eq, eq_circuit, &eq_instances)?;
    let ep_proof = create_mint_authorization_ep_proof(ep, ep_circuit, &ep_instances)?;
    validate_commit_wrapper_proof_length(KagemushaPastaParityV1::Eq, &eq_proof)?;
    validate_commit_wrapper_proof_length(KagemushaPastaParityV1::Ep, &ep_proof)?;
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
fn read_eq_recursive_vk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Eq;
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read::<_, KagemushaRecursiveStateEqCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
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
        public.extend(crate::zk::kagemusha_v1_poseidon::digest_limbs::<F>(
            digest,
        ));
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
fn commit_wrapper_public_instances<F: KagemushaPoseidonFieldV1>(
    public: &KagemushaCommitWrapperPublicInputsV1,
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
    if instances.len() != COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1 {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "commit-wrapper public instance ABI mismatch".to_owned(),
        ));
    }
    Ok(instances)
}

#[cfg(feature = "zk-halo2-ipa")]
pub(super) fn read_eq_commit_wrapper_vk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Eq;
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read::<_, KagemushaCommitWrapperEqCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "commit-wrapper verifying key", error))?;
    ensure_cursor_consumed(parity, "commit-wrapper verifying key", &cursor, bytes.len())?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "commit-wrapper verifying key",
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
    let key = VerifyingKey::read::<_, KagemushaCommitWrapperEpCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "commit-wrapper verifying key", error))?;
    ensure_cursor_consumed(parity, "commit-wrapper verifying key", &cursor, bytes.len())?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "commit-wrapper verifying key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
pub(super) fn read_eq_commit_wrapper_pk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<ProvingKey<EqAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Eq;
    let mut cursor = Cursor::new(bytes);
    let key = ProvingKey::read::<_, KagemushaCommitWrapperEqCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "commit-wrapper proving key", error))?;
    ensure_cursor_consumed(parity, "commit-wrapper proving key", &cursor, bytes.len())?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "commit-wrapper proving key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
pub(super) fn read_ep_commit_wrapper_pk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<ProvingKey<EpAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Ep;
    let mut cursor = Cursor::new(bytes);
    let key = ProvingKey::read::<_, KagemushaCommitWrapperEpCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "commit-wrapper proving key", error))?;
    ensure_cursor_consumed(parity, "commit-wrapper proving key", &cursor, bytes.len())?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "commit-wrapper proving key",
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
    let key = VerifyingKey::read::<_, KagemushaMintAuthorizationEqCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
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
fn read_ep_mint_authorization_vk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Ep;
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read::<_, KagemushaMintAuthorizationEpCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
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
fn read_eq_mint_authorization_pk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<ProvingKey<EqAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Eq;
    let mut cursor = Cursor::new(bytes);
    let key = ProvingKey::read::<_, KagemushaMintAuthorizationEqCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "mint-authorization proving key", error))?;
    ensure_cursor_consumed(
        parity,
        "mint-authorization proving key",
        &cursor,
        bytes.len(),
    )?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "mint-authorization proving key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
fn read_ep_mint_authorization_pk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<ProvingKey<EpAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Ep;
    let mut cursor = Cursor::new(bytes);
    let key = ProvingKey::read::<_, KagemushaMintAuthorizationEpCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "mint-authorization proving key", error))?;
    ensure_cursor_consumed(
        parity,
        "mint-authorization proving key",
        &cursor,
        bytes.len(),
    )?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "mint-authorization proving key",
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
    let key = VerifyingKey::read::<_, KagemushaMintAuthorityEqCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
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
    let key = VerifyingKey::read::<_, KagemushaMintAuthorityEpCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
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
fn read_eq_mint_pk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<ProvingKey<EqAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Eq;
    let mut cursor = Cursor::new(bytes);
    let key = ProvingKey::read::<_, KagemushaMintAuthorityEqCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "mint-authority proving key", error))?;
    ensure_cursor_consumed(parity, "mint-authority proving key", &cursor, bytes.len())?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "mint-authority proving key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
fn read_ep_mint_pk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<ProvingKey<EpAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Ep;
    let mut cursor = Cursor::new(bytes);
    let key = ProvingKey::read::<_, KagemushaMintAuthorityEpCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "mint-authority proving key", error))?;
    ensure_cursor_consumed(parity, "mint-authority proving key", &cursor, bytes.len())?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "mint-authority proving key",
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
    let key = VerifyingKey::read::<_, KagemushaRecursiveStateEpCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
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
fn read_eq_recursive_pk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<ProvingKey<EqAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Eq;
    let mut cursor = Cursor::new(bytes);
    let key = ProvingKey::read::<_, KagemushaRecursiveStateEqCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "recursive state proving key", error))?;
    ensure_cursor_consumed(parity, "recursive state proving key", &cursor, bytes.len())?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "recursive state proving key",
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(feature = "zk-halo2-ipa")]
fn read_ep_recursive_pk(
    bytes: &[u8],
    circuit_params: BaseCircuitParams,
) -> Result<ProvingKey<EpAffine>, KagemushaArtifactGenerationErrorV1> {
    let parity = KagemushaPastaParityV1::Ep;
    let mut cursor = Cursor::new(bytes);
    let key = ProvingKey::read::<_, KagemushaRecursiveStateEpCircuitV1>(
        &mut cursor,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, "recursive state proving key", error))?;
    ensure_cursor_consumed(parity, "recursive state proving key", &cursor, bytes.len())?;
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(key_decode_message(
            parity,
            "recursive state proving key",
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
fn create_recursive_eq_proof(
    artifacts: &KagemushaLoadedEqRecursiveStateArtifactsV1,
    circuit: KagemushaRecursiveStateEqCircuitV1,
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
    let mut transcript =
        Transcript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(Vec::<u8>::new());
    create_proof::<
        IPACommitmentScheme<EqAffine>,
        ProverIPA<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
        _,
    >(
        &artifacts.parameters,
        &artifacts.proving_key,
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
    Ok(transcript.finalize())
}

#[cfg(feature = "zk-halo2-ipa")]
fn create_recursive_ep_proof(
    artifacts: &KagemushaLoadedEpRecursiveStateArtifactsV1,
    circuit: KagemushaRecursiveStateEpCircuitV1,
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
    let mut transcript =
        Transcript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(Vec::<u8>::new());
    create_proof::<
        IPACommitmentScheme<EpAffine>,
        ProverIPA<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
        _,
    >(
        &artifacts.parameters,
        &artifacts.proving_key,
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
    Ok(transcript.finalize())
}

#[cfg(feature = "zk-halo2-ipa")]
fn create_commit_wrapper_eq_proof(
    artifacts: &KagemushaLoadedEqCommitWrapperArtifactsV1,
    circuit: KagemushaCommitWrapperEqCircuitV1,
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
    let mut transcript =
        Transcript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(Vec::<u8>::new());
    create_proof::<
        IPACommitmentScheme<EqAffine>,
        ProverIPA<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
        _,
    >(
        &artifacts.parameters,
        &artifacts.proving_key,
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
    Ok(transcript.finalize())
}

#[cfg(feature = "zk-halo2-ipa")]
fn create_commit_wrapper_ep_proof(
    artifacts: &KagemushaLoadedEpCommitWrapperArtifactsV1,
    circuit: KagemushaCommitWrapperEpCircuitV1,
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
    let mut transcript =
        Transcript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(Vec::<u8>::new());
    create_proof::<
        IPACommitmentScheme<EpAffine>,
        ProverIPA<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
        _,
    >(
        &artifacts.parameters,
        &artifacts.proving_key,
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
    Ok(transcript.finalize())
}

#[cfg(feature = "zk-halo2-ipa")]
fn create_mint_authorization_eq_proof(
    artifacts: &KagemushaLoadedEqMintAuthorizationArtifactsV1,
    circuit: KagemushaMintAuthorizationEqCircuitV1,
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
    let mut transcript =
        Transcript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(Vec::<u8>::new());
    create_proof::<
        IPACommitmentScheme<EqAffine>,
        ProverIPA<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
        _,
    >(
        &artifacts.parameters,
        &artifacts.proving_key,
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
    Ok(transcript.finalize())
}

#[cfg(feature = "zk-halo2-ipa")]
fn create_mint_authorization_ep_proof(
    artifacts: &KagemushaLoadedEpMintAuthorizationArtifactsV1,
    circuit: KagemushaMintAuthorizationEpCircuitV1,
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
    let mut transcript =
        Transcript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(Vec::<u8>::new());
    create_proof::<
        IPACommitmentScheme<EpAffine>,
        ProverIPA<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
        _,
    >(
        &artifacts.parameters,
        &artifacts.proving_key,
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
    Ok(transcript.finalize())
}

#[cfg(feature = "zk-halo2-ipa")]
fn create_mint_eq_proof(
    artifacts: &KagemushaLoadedEqMintAuthorityArtifactsV1,
    circuit: KagemushaMintAuthorityEqCircuitV1,
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
    let mut transcript =
        Transcript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(Vec::<u8>::new());
    create_proof::<
        IPACommitmentScheme<EqAffine>,
        ProverIPA<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
        _,
    >(
        &artifacts.parameters,
        &artifacts.proving_key,
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
    Ok(transcript.finalize())
}

#[cfg(feature = "zk-halo2-ipa")]
fn create_mint_ep_proof(
    artifacts: &KagemushaLoadedEpMintAuthorityArtifactsV1,
    circuit: KagemushaMintAuthorityEpCircuitV1,
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
    let mut transcript =
        Transcript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(Vec::<u8>::new());
    create_proof::<
        IPACommitmentScheme<EpAffine>,
        ProverIPA<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
        _,
    >(
        &artifacts.parameters,
        &artifacts.proving_key,
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
    Ok(transcript.finalize())
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_recursive_profile(
    parity: KagemushaPastaParityV1,
    params: &BaseCircuitParams,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    if params.k != KAGEMUSHA_HALO2_K_V1 as usize
        || params.num_instance_columns != 1
        || params.lookup_bits != Some((KAGEMUSHA_HALO2_K_V1 - 1) as usize)
        || params.num_advice_per_phase.is_empty()
        || params.num_lookup_advice_per_phase.is_empty()
        || params.num_advice_per_phase.iter().all(|count| *count == 0)
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(parity));
    }
    Ok(())
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_commit_wrapper_profile(
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
fn validate_commit_wrapper_proof_length(
    parity: KagemushaPastaParityV1,
    proof: &[u8],
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    if proof.is_empty() || proof.len() > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1 {
        return Err(KagemushaArtifactGenerationErrorV1::InvalidLength {
            parity,
            kind: "commit-wrapper proof",
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
    use super::*;

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
        assert_eq!(PUBLIC_INSTANCE_COUNT, 81);
        assert_eq!(recursive_public_instance_count(), 115);
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
        .expect("77 transcript items fit the immutable parity slot");
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
    fn commit_wrapper_shape_roles_and_transport_bounds_are_fixed() {
        assert_eq!(COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1, 81);
        assert!(core::mem::size_of::<KagemushaRecursiveIncomingEqGenerationWitnessV1<'_>>() > 0);
        assert!(core::mem::size_of::<KagemushaRecursiveIncomingEpGenerationWitnessV1<'_>>() > 0);

        let profile = BaseCircuitParams {
            k: KAGEMUSHA_HALO2_K_V1 as usize,
            num_advice_per_phase: vec![1],
            num_fixed: 1,
            num_lookup_advice_per_phase: vec![1],
            lookup_bits: Some((KAGEMUSHA_HALO2_K_V1 - 1) as usize),
            num_instance_columns: 1,
        };
        let artifacts = KagemushaGeneratedCommitWrapperArtifactsV1 {
            eq_parameters: Arc::from(&b"eq-params"[..]),
            ep_parameters: Arc::from(&b"ep-params"[..]),
            eq_proving_key: Arc::from(&b"eq-pk"[..]),
            eq_verifying_key: Arc::from(&b"eq-vk"[..]),
            ep_proving_key: Arc::from(&b"ep-pk"[..]),
            ep_verifying_key: Arc::from(&b"ep-vk"[..]),
            eq_circuit_params: profile.clone(),
            ep_circuit_params: profile,
            eq_protocol_digest: [1; 32],
            ep_protocol_digest: [2; 32],
            enabled_hardware_profiles: {
                let mut profiles = [[0; 32]; KAGEMUSHA_COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1];
                profiles[0] = [3; 32];
                profiles
            },
        };
        assert_eq!(
            artifacts.bindings().map(|binding| binding.role),
            [
                KagemushaArtifactRoleV1::CommitWrapperPkEq,
                KagemushaArtifactRoleV1::CommitWrapperVkEq,
                KagemushaArtifactRoleV1::CommitWrapperPkEp,
                KagemushaArtifactRoleV1::CommitWrapperVkEp,
            ]
        );
        assert!(
            validate_commit_wrapper_proof_length(
                KagemushaPastaParityV1::Eq,
                &[0; KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1],
            )
            .is_ok()
        );
        assert!(
            validate_commit_wrapper_proof_length(
                KagemushaPastaParityV1::Eq,
                &[0; KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1 + 1],
            )
            .is_err()
        );
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
