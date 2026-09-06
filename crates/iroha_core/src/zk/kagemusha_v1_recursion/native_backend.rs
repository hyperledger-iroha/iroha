//! Authenticated terminal verifier for the complete Kagemusha V1 recursive state relation.

use std::io::Cursor;

use halo2_base::gates::circuit::BaseCircuitParams;
use halo2_proofs::{
    SerdeFormat,
    arithmetic::best_multiexp,
    halo2curves::{
        CurveAffine, CurveExt as _,
        group::Curve as _,
        pasta::{Ep, EpAffine, Eq, EqAffine, Fp, Fq},
    },
    plonk::VerifyingKey,
    poly::commitment::{Blind, Params as _, ParamsProver as _},
};
use sha2::{Digest as _, Sha256};
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::{
        PolynomialCommitmentScheme,
        ipa::{Bgh19, IpaAccumulator, IpaAs, IpaSuccinctVerifyingKey},
    },
    system::halo2::{compile, transcript::halo2::PoseidonTranscript},
    util::{
        arithmetic::{Domain, root_of_unity},
        msm::Msm,
        transcript::{Transcript as _, TranscriptRead as _},
    },
    verifier::{
        SnarkVerifier as _,
        plonk::{PlonkProof, PlonkProtocol, PlonkSuccinctVerifier},
    },
};

use super::{
    KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1 as PASTA_IPA_POSEIDON_FULL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1 as PASTA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_RATE_V1 as PASTA_IPA_POSEIDON_RATE_V1,
    KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1 as PASTA_IPA_POSEIDON_SECURE_MDS_V1,
    KAGEMUSHA_IPA_POSEIDON_WIDTH_V1 as PASTA_IPA_POSEIDON_WIDTH_V1,
    KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1, KagemushaArtifactByteResolverV1, KagemushaArtifactErrorV1,
    KagemushaAuthenticatedArtifactSetV1, KagemushaEpAccumulatorV1, KagemushaEqAccumulatorV1,
    KagemushaParityVerificationRequestV1, KagemushaPastaParityV1, KagemushaRecursiveVerifierV1,
    KagemushaStateProofVerificationRequestV1, KagemushaTerminalAuthorizationPublicInputsV1,
    composite::{KagemushaRecursiveStateEpCircuitV1, KagemushaRecursiveStateEqCircuitV1},
    decide_kagemusha_ep_accumulator_v1, decide_kagemusha_eq_accumulator_v1,
    deferred_parent::{
        accumulator_limb_count, native_parent_protocol_digest_v1, ordinary_ipa_proof_profile_v1,
    },
    guard_bundle::{
        GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1, KagemushaGuardBundleEpCircuitV1,
        KagemushaGuardBundleEqCircuitV1,
    },
    mint_authority::{
        KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1, KagemushaMintAuthorityCheckpointV1,
        public_instance as mint_public_instance,
    },
    mint_authorization::{
        MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1, mint_authorization_public_instances_v1,
    },
    mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1,
    mint_hash_shard::KAGEMUSHA_MINT_HASH_SHARD_K_V1,
    mint_helper::KagemushaMintAuthorityStepV1,
    mint_transport_decider::{
        KagemushaMintAuthorityTransportEpCircuitV1, KagemushaMintAuthorityTransportEqCircuitV1,
        KagemushaMintAuthorizationTransportEpCircuitV1,
        KagemushaMintAuthorizationTransportEqCircuitV1,
    },
    state_relation::PUBLIC_INSTANCE_COUNT,
    terminal_authorization::{
        KagemushaCommitWrapperEpCircuitV1, KagemushaCommitWrapperEqCircuitV1,
        KagemushaTerminalAuthorizationEpCircuitV1, KagemushaTerminalAuthorizationEqCircuitV1,
        TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1,
        public_instance as terminal_authorization_public_instance,
    },
    transport_decider::{
        KagemushaTransportDeciderEpCircuitV1, KagemushaTransportDeciderEqCircuitV1,
    },
};
use crate::zk::kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, decode, digest_limbs, from_u128};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_HALO2_K_V1, KagemushaArtifactRoleV1, KagemushaMintAuthorizationV1,
    KagemushaPaymentRequestV1, KagemushaPaymentV1, kagemusha_asset_identity_digest_v1,
};

const RECURSIVE_PROFILE_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:paired-recursive-circuit-profile";
const RECURSIVE_PUBLIC_INSTANCE_COUNT_V1: usize = PUBLIC_INSTANCE_COUNT + accumulator_limb_count();

/// Check the fixed KAGEMUSHA layout and the actual `BaseConfig` allocation order.
///
/// This is a shape/overflow check, not a new resource ceiling. Artifact consumers must also
/// authenticate the complete profile before configuring columns selected by its widths.
pub(super) fn validate_kagemusha_base_circuit_params_v1(
    params: &BaseCircuitParams,
) -> Result<(), &'static str> {
    validate_kagemusha_base_circuit_params_at_k_v1(params, KAGEMUSHA_HALO2_K_V1)
}

/// Check the fixed KAGEMUSHA layout of a two-column private hybrid relation.
///
/// MintAuthorization uses one compact semantic column and one wide proof-carried
/// deferred-audit column. All outer and other private circuits continue to use
/// [`validate_kagemusha_base_circuit_params_v1`] and require exactly one column.
fn validate_kagemusha_hybrid_base_circuit_params_v1(
    params: &BaseCircuitParams,
) -> Result<(), &'static str> {
    validate_kagemusha_base_circuit_params_with_instance_columns_at_k_v1(
        params,
        KAGEMUSHA_HALO2_K_V1,
        2,
    )
}

/// Check the fixed hybrid layout of the private MintAuthorization relation.
pub(crate) fn validate_kagemusha_inner_mint_authorization_base_circuit_params_v1(
    params: &BaseCircuitParams,
) -> Result<(), &'static str> {
    validate_kagemusha_hybrid_base_circuit_params_v1(params)
}

/// Check the fixed three-column hybrid layout of the private MintHashClaim relation.
///
/// Its semantic column binds proof-supplied commitments for both wide carrier
/// columns. The two columns retain the Eq-vector and Ep-vector copies used by
/// the paired claim proof.
pub(crate) fn validate_kagemusha_mint_hash_claim_base_circuit_params_v1(
    params: &BaseCircuitParams,
) -> Result<(), &'static str> {
    validate_kagemusha_base_circuit_params_with_instance_columns_at_k_v1(
        params,
        KAGEMUSHA_HALO2_K_V1,
        3,
    )
}

fn validate_kagemusha_base_circuit_params_at_k_v1(
    params: &BaseCircuitParams,
    expected_k: u32,
) -> Result<(), &'static str> {
    validate_kagemusha_base_circuit_params_with_instance_columns_at_k_v1(params, expected_k, 1)
}

fn validate_kagemusha_base_circuit_params_with_instance_columns_at_k_v1(
    params: &BaseCircuitParams,
    expected_k: u32,
    expected_instance_columns: usize,
) -> Result<(), &'static str> {
    // `halo2-base` allocates columns only in FirstPhase, SecondPhase, and ThirdPhase.
    // Zero-filled tails allocate nothing and remain part of the exact authenticated profile.
    const PHASE_COUNT: usize = 3;
    if params.k != expected_k as usize
        || params.num_instance_columns != expected_instance_columns
        || params.lookup_bits != Some((expected_k - 1) as usize)
        || params.num_advice_per_phase.is_empty()
        || params.num_lookup_advice_per_phase.is_empty()
        || params.num_advice_per_phase.iter().all(|count| *count == 0)
        || params
            .num_advice_per_phase
            .iter()
            .skip(PHASE_COUNT)
            .any(|count| *count != 0)
        || params
            .num_lookup_advice_per_phase
            .iter()
            .skip(PHASE_COUNT)
            .any(|count| *count != 0)
    {
        return Err("unsupported fixed circuit layout");
    }
    let checked_sum = |counts: &[usize]| {
        counts
            .iter()
            .try_fold(0_usize, |sum, count| sum.checked_add(*count))
            .ok_or("circuit column count overflow")
    };
    let gate_columns = checked_sum(&params.num_advice_per_phase)?;
    let lookup_columns = checked_sum(&params.num_lookup_advice_per_phase)?;
    let lookup_enabled = lookup_columns != 0;
    let optimized_first_lookup = lookup_enabled
        && params.num_advice_per_phase[0] == 1
        && params.num_lookup_advice_per_phase[0] != 0;
    let allocated_lookup_columns = if optimized_first_lookup {
        lookup_columns - params.num_lookup_advice_per_phase[0]
    } else {
        lookup_columns
    };
    let fixed_columns = params
        .num_fixed
        .checked_add(usize::from(lookup_enabled))
        .ok_or("circuit fixed-column count overflow")?;
    let selectors = gate_columns
        .checked_add(usize::from(optimized_first_lookup))
        .ok_or("circuit selector count overflow")?;
    fixed_columns
        .checked_add(selectors)
        .and_then(|columns| columns.checked_add(gate_columns))
        .and_then(|columns| columns.checked_add(allocated_lookup_columns))
        .and_then(|columns| columns.checked_add(params.num_instance_columns))
        .ok_or("circuit column count overflow")?;

    // FlexGate allocates every gate column before RangeConfig allocates any lookup column.
    // A lookup in an earlier phase cannot therefore fill a hole in the gate allocation order.
    let mut phase_exists = [false; PHASE_COUNT];
    for (phase, count) in params
        .num_advice_per_phase
        .iter()
        .take(PHASE_COUNT)
        .enumerate()
    {
        if *count != 0 {
            if phase != 0 && !phase_exists[phase - 1] {
                return Err("gate advice skips a required earlier phase");
            }
            phase_exists[phase] = true;
        }
    }
    // Different vector lengths are supported: RangeConfig treats absent gate phases as zero.
    for (phase, count) in params
        .num_lookup_advice_per_phase
        .iter()
        .take(PHASE_COUNT)
        .enumerate()
    {
        if *count != 0 {
            if phase != 0 && !phase_exists[phase - 1] {
                return Err("lookup advice skips a required earlier phase");
            }
            phase_exists[phase] = true;
        }
    }
    Ok(())
}

fn validate_authenticated_guard_protocol_binding_v1(
    actual_eq: [u8; 32],
    actual_ep: [u8; 32],
    expected_eq: [u8; 32],
    expected_ep: [u8; 32],
) -> Result<(), String> {
    if actual_eq != expected_eq || actual_ep != expected_ep {
        return Err("Kagemusha GuardBundle compiled protocol release mismatch".to_owned());
    }
    Ok(())
}

/// Exact `halo2-base` layouts for authenticated outer and private recursive proof roles.
///
/// These values are covered by [`Self::canonical_digest`], which must equal the release
/// manifest's authenticated profile digest. They are explicit because processed Halo2 keys do
/// not safely self-describe a circuit configuration.
#[derive(Clone, Debug)]
pub struct KagemushaRecursiveVerifierProfileV1 {
    /// Eq private recursive aggregate-state carrier layout.
    pub inner_state_eq: BaseCircuitParams,
    /// Ep private recursive aggregate-state carrier layout.
    pub inner_state_ep: BaseCircuitParams,
    /// Eq compact aggregate-state transport-decider layout.
    pub state_eq: BaseCircuitParams,
    /// Ep compact aggregate-state transport-decider layout.
    pub state_ep: BaseCircuitParams,
    /// Eq GuardBundle recursive circuit layout.
    pub guard_eq: BaseCircuitParams,
    /// Ep GuardBundle recursive circuit layout.
    pub guard_ep: BaseCircuitParams,
    /// Eq terminal `TerminalAuthorization` recursive circuit layout.
    pub terminal_authorization_eq: BaseCircuitParams,
    /// Ep terminal `TerminalAuthorization` recursive circuit layout.
    pub terminal_authorization_ep: BaseCircuitParams,
    /// Eq post-commit `CommitWrapper` recursive circuit layout.
    pub commit_wrapper_eq: BaseCircuitParams,
    /// Ep post-commit `CommitWrapper` recursive circuit layout.
    pub commit_wrapper_ep: BaseCircuitParams,
    /// Eq compact mint-authorization transport layout.
    pub mint_authorization_eq: BaseCircuitParams,
    /// Ep compact mint-authorization transport layout.
    pub mint_authorization_ep: BaseCircuitParams,
    /// Eq compact stable mint-authority transport layout.
    pub mint_eq: BaseCircuitParams,
    /// Ep compact stable mint-authority transport layout.
    pub mint_ep: BaseCircuitParams,
    /// Eq private mint-authorization relation layout.
    pub inner_mint_authorization_eq: BaseCircuitParams,
    /// Ep private mint-authorization relation layout.
    pub inner_mint_authorization_ep: BaseCircuitParams,
    /// Eq private stable mint-authority relation layout.
    pub inner_mint_eq: BaseCircuitParams,
    /// Ep private stable mint-authority relation layout.
    pub inner_mint_ep: BaseCircuitParams,
    /// Eq one-block mint-certificate SHA-256 shard layout (`k = 12`).
    pub mint_hash_shard_eq: BaseCircuitParams,
    /// Ep one-block mint-certificate SHA-256 shard layout (`k = 12`).
    pub mint_hash_shard_ep: BaseCircuitParams,
    /// Eq ordered recursive mint-hash claim layout (`k = 16`).
    pub mint_hash_claim_eq: BaseCircuitParams,
    /// Ep ordered recursive mint-hash claim layout (`k = 16`).
    pub mint_hash_claim_ep: BaseCircuitParams,
    /// Actual compiled Eq finalized-mint protocol identity constrained by the state circuit.
    pub mint_eq_protocol_digest: [u8; 32],
    /// Actual compiled Ep finalized-mint protocol identity constrained by the state circuit.
    pub mint_ep_protocol_digest: [u8; 32],
    /// Actual compiled Eq mint-hash shard protocol identity.
    pub mint_hash_shard_eq_protocol_digest: [u8; 32],
    /// Actual compiled Ep mint-hash shard protocol identity.
    pub mint_hash_shard_ep_protocol_digest: [u8; 32],
    /// Actual compiled Eq ordered mint-hash claim protocol identity.
    pub mint_hash_claim_eq_protocol_digest: [u8; 32],
    /// Actual compiled Ep ordered mint-hash claim protocol identity.
    pub mint_hash_claim_ep_protocol_digest: [u8; 32],
    /// Release-pinned genesis mint-finality roster identifier.
    pub mint_genesis_roster_id: [u8; 32],
}

impl KagemushaRecursiveVerifierProfileV1 {
    /// Hash the exact fixed-layout profile selected by the authenticated release.
    ///
    /// No bootstrap proof or release identifier is part of this preimage. Bootstrap is proved
    /// only after release authentication, preventing a cycle in which the release identity would
    /// depend on a proof that itself exposes that release identity.
    ///
    /// # Errors
    ///
    /// Returns an error if a host `usize` cannot be represented in the canonical `u64` profile.
    pub fn canonical_digest(&self) -> Result<[u8; 32], String> {
        self.validate()?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(RECURSIVE_PROFILE_DOMAIN_V1);
        bytes.push(0);
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        for (tag, params) in [
            (1_u8, &self.inner_state_eq),
            (2, &self.inner_state_ep),
            (3, &self.state_eq),
            (4, &self.state_ep),
            (5, &self.guard_eq),
            (6, &self.guard_ep),
            (7, &self.terminal_authorization_eq),
            (8, &self.terminal_authorization_ep),
            (9, &self.commit_wrapper_eq),
            (10, &self.commit_wrapper_ep),
            (11, &self.mint_authorization_eq),
            (12, &self.mint_authorization_ep),
            (13, &self.mint_eq),
            (14, &self.mint_ep),
        ] {
            bytes.push(tag);
            append_base_params(&mut bytes, params)?;
        }
        bytes.push(15);
        bytes.extend_from_slice(&self.mint_eq_protocol_digest);
        bytes.push(16);
        bytes.extend_from_slice(&self.mint_ep_protocol_digest);
        bytes.push(17);
        bytes.extend_from_slice(&self.mint_genesis_roster_id);
        for (tag, params) in [
            (18_u8, &self.inner_mint_authorization_eq),
            (19, &self.inner_mint_authorization_ep),
            (20, &self.inner_mint_eq),
            (21, &self.inner_mint_ep),
        ] {
            bytes.push(tag);
            append_base_params(&mut bytes, params)?;
        }
        for (tag, params) in [
            (22_u8, &self.mint_hash_shard_eq),
            (23, &self.mint_hash_shard_ep),
            (24, &self.mint_hash_claim_eq),
            (25, &self.mint_hash_claim_ep),
        ] {
            bytes.push(tag);
            append_base_params(&mut bytes, params)?;
        }
        for (tag, digest) in [
            (26_u8, self.mint_hash_shard_eq_protocol_digest),
            (27, self.mint_hash_shard_ep_protocol_digest),
            (28, self.mint_hash_claim_eq_protocol_digest),
            (29, self.mint_hash_claim_ep_protocol_digest),
        ] {
            bytes.push(tag);
            bytes.extend_from_slice(&digest);
        }
        Ok(Sha256::digest(bytes).into())
    }

    /// Authenticate the complete layout before any parameter-driven artifact decoding.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported layout or a release profile-digest mismatch.
    pub(crate) fn validate_against_artifacts<R: KagemushaArtifactByteResolverV1>(
        &self,
        artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
    ) -> Result<(), KagemushaArtifactErrorV1> {
        let digest = self
            .canonical_digest()
            .map_err(KagemushaArtifactErrorV1::InvalidRelease)?;
        if digest != artifacts.recursion_artifacts().profile_digest {
            return Err(KagemushaArtifactErrorV1::InvalidRelease(
                "recursive circuit profile digest mismatch".to_owned(),
            ));
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), String> {
        for (label, params) in [
            ("inner state Eq", &self.inner_state_eq),
            ("inner state Ep", &self.inner_state_ep),
            ("transport state Eq", &self.state_eq),
            ("transport state Ep", &self.state_ep),
            ("GuardBundle Eq", &self.guard_eq),
            ("GuardBundle Ep", &self.guard_ep),
            ("terminal authorization Eq", &self.terminal_authorization_eq),
            ("terminal authorization Ep", &self.terminal_authorization_ep),
            ("commit-wrapper Eq", &self.commit_wrapper_eq),
            ("commit-wrapper Ep", &self.commit_wrapper_ep),
            ("mint authorization Eq", &self.mint_authorization_eq),
            ("mint authorization Ep", &self.mint_authorization_ep),
            ("mint authority Eq", &self.mint_eq),
            ("mint authority Ep", &self.mint_ep),
            ("inner mint authority Eq", &self.inner_mint_eq),
            ("inner mint authority Ep", &self.inner_mint_ep),
        ] {
            validate_kagemusha_base_circuit_params_v1(params)
                .map_err(|reason| format!("invalid Kagemusha {label} circuit profile: {reason}"))?;
        }
        for (label, params) in [
            (
                "inner mint authorization Eq",
                &self.inner_mint_authorization_eq,
            ),
            (
                "inner mint authorization Ep",
                &self.inner_mint_authorization_ep,
            ),
        ] {
            validate_kagemusha_hybrid_base_circuit_params_v1(params)
                .map_err(|reason| format!("invalid Kagemusha {label} circuit profile: {reason}"))?;
        }
        for (label, params) in [
            ("mint hash claim Eq", &self.mint_hash_claim_eq),
            ("mint hash claim Ep", &self.mint_hash_claim_ep),
        ] {
            validate_kagemusha_mint_hash_claim_base_circuit_params_v1(params)
                .map_err(|reason| format!("invalid Kagemusha {label} circuit profile: {reason}"))?;
        }
        for (label, params) in [
            ("mint hash shard Eq", &self.mint_hash_shard_eq),
            ("mint hash shard Ep", &self.mint_hash_shard_ep),
        ] {
            validate_kagemusha_base_circuit_params_at_k_v1(params, KAGEMUSHA_MINT_HASH_SHARD_K_V1)
                .map_err(|reason| format!("invalid Kagemusha {label} circuit profile: {reason}"))?;
        }
        let mint_protocols = [
            self.mint_eq_protocol_digest,
            self.mint_ep_protocol_digest,
            self.mint_hash_shard_eq_protocol_digest,
            self.mint_hash_shard_ep_protocol_digest,
            self.mint_hash_claim_eq_protocol_digest,
            self.mint_hash_claim_ep_protocol_digest,
        ];
        if mint_protocols.iter().any(|digest| *digest == [0; 32])
            || mint_protocols
                .iter()
                .enumerate()
                .any(|(index, digest)| mint_protocols[index + 1..].contains(digest))
            || decode::<Fp>(self.mint_eq_protocol_digest).is_none()
            || decode::<Fq>(self.mint_ep_protocol_digest).is_none()
            || self.mint_genesis_roster_id == [0; 32]
        {
            return Err("invalid Kagemusha finalized-mint protocol identities".to_owned());
        }
        if decode::<Fp>(self.mint_hash_shard_eq_protocol_digest).is_none()
            || decode::<Fq>(self.mint_hash_shard_ep_protocol_digest).is_none()
            || decode::<Fp>(self.mint_hash_claim_eq_protocol_digest).is_none()
            || decode::<Fq>(self.mint_hash_claim_ep_protocol_digest).is_none()
        {
            return Err("invalid Kagemusha mint-hash protocol identities".to_owned());
        }
        Ok(())
    }
}

/// Release-authenticated accepting verifier for the paired recursive state relation.
///
/// Construction authenticates every key byte, validates the circuit profile, recompiles all
/// protocols, and checks the state protocol identities recorded by the release. Verification
/// then supplies the exact Guard protocol identities derived from the authenticated Guard keys.
pub struct KagemushaAuthenticatedRecursiveVerifierV1 {
    eq_parameters: halo2_proofs::poly::ipa::commitment::ParamsIPA<EqAffine>,
    ep_parameters: halo2_proofs::poly::ipa::commitment::ParamsIPA<EpAffine>,
    eq_state_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
    ep_state_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
    eq_terminal_authorization_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
    ep_terminal_authorization_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
    eq_commit_wrapper_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
    ep_commit_wrapper_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
    eq_mint_authorization_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
    ep_mint_authorization_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
    eq_mint_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
    ep_mint_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
    inner_eq_protocol_digest: [u8; 32],
    inner_ep_protocol_digest: [u8; 32],
    guard_eq_protocol_digest: [u8; 32],
    guard_ep_protocol_digest: [u8; 32],
    terminal_authorization_eq_protocol_digest: [u8; 32],
    terminal_authorization_ep_protocol_digest: [u8; 32],
    commit_wrapper_eq_protocol_digest: [u8; 32],
    commit_wrapper_ep_protocol_digest: [u8; 32],
    mint_authorization_eq_protocol_digest: [u8; 32],
    mint_authorization_ep_protocol_digest: [u8; 32],
    mint_eq_protocol_digest: [u8; 32],
    mint_ep_protocol_digest: [u8; 32],
    mint_genesis_roster_id: [u8; 32],
    release_id: [u8; 32],
    suite_id: [u8; 32],
    vk_set_digest: [u8; 32],
    artifact_manifest_digest: [u8; 32],
    terminal_authorization_eq_binding: iroha_data_model::kagemusha::KagemushaArtifactBindingV1,
    terminal_authorization_ep_binding: iroha_data_model::kagemusha::KagemushaArtifactBindingV1,
    commit_wrapper_eq_binding: iroha_data_model::kagemusha::KagemushaArtifactBindingV1,
    commit_wrapper_ep_binding: iroha_data_model::kagemusha::KagemushaArtifactBindingV1,
}

impl KagemushaAuthenticatedRecursiveVerifierV1 {
    /// Resolve and reauthenticate the exact state/Guard verifying keys for one release.
    ///
    /// # Errors
    ///
    /// Fails closed for missing or substituted bytes, profile mismatch, malformed processed
    /// keys, trailing bytes, or any compiled-protocol identity mismatch.
    pub fn load<R>(
        artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
        profile: KagemushaRecursiveVerifierProfileV1,
    ) -> Result<Self, KagemushaArtifactErrorV1>
    where
        R: KagemushaArtifactByteResolverV1,
    {
        profile.validate_against_artifacts(artifacts)?;
        let recursion = artifacts.recursion_artifacts();
        let eq_parameters = artifacts.load_eq_params()?;
        let ep_parameters = artifacts.load_ep_params()?;
        let inner_eq_state_vk = read_eq_inner_state_vk(
            artifacts
                .resolve(KagemushaArtifactRoleV1::InnerStateVkEq)?
                .as_ref(),
            profile.inner_state_eq,
        )?;
        let inner_ep_state_vk = read_ep_inner_state_vk(
            artifacts
                .resolve(KagemushaArtifactRoleV1::InnerStateVkEp)?
                .as_ref(),
            profile.inner_state_ep,
        )?;
        let eq_state_vk = read_eq_state_vk(
            artifacts
                .resolve(KagemushaArtifactRoleV1::StateVkEq)?
                .as_ref(),
            profile.state_eq,
        )?;
        let ep_state_vk = read_ep_state_vk(
            artifacts
                .resolve(KagemushaArtifactRoleV1::StateVkEp)?
                .as_ref(),
            profile.state_ep,
        )?;
        let eq_guard_vk = read_eq_guard_vk(
            artifacts
                .resolve(KagemushaArtifactRoleV1::GuardBundleVkEq)?
                .as_ref(),
            profile.guard_eq,
        )?;
        let ep_guard_vk = read_ep_guard_vk(
            artifacts
                .resolve(KagemushaArtifactRoleV1::GuardBundleVkEp)?
                .as_ref(),
            profile.guard_ep,
        )?;
        let eq_terminal_authorization_vk = read_eq_terminal_authorization_vk(
            artifacts
                .resolve(KagemushaArtifactRoleV1::TerminalAuthorizationVkEq)?
                .as_ref(),
            profile.terminal_authorization_eq,
        )?;
        let ep_terminal_authorization_vk = read_ep_terminal_authorization_vk(
            artifacts
                .resolve(KagemushaArtifactRoleV1::TerminalAuthorizationVkEp)?
                .as_ref(),
            profile.terminal_authorization_ep,
        )?;
        let eq_commit_wrapper_vk = read_eq_commit_wrapper_vk(
            artifacts
                .resolve(KagemushaArtifactRoleV1::CommitWrapperVkEq)?
                .as_ref(),
            profile.commit_wrapper_eq,
        )?;
        let ep_commit_wrapper_vk = read_ep_commit_wrapper_vk(
            artifacts
                .resolve(KagemushaArtifactRoleV1::CommitWrapperVkEp)?
                .as_ref(),
            profile.commit_wrapper_ep,
        )?;
        let eq_mint_authorization_vk = read_eq_mint_authorization_vk(
            artifacts
                .resolve(KagemushaArtifactRoleV1::MintAuthorizationVkEq)?
                .as_ref(),
            profile.mint_authorization_eq,
        )?;
        let ep_mint_authorization_vk = read_ep_mint_authorization_vk(
            artifacts
                .resolve(KagemushaArtifactRoleV1::MintAuthorizationVkEp)?
                .as_ref(),
            profile.mint_authorization_ep,
        )?;
        let eq_mint_vk = read_eq_mint_vk(
            artifacts
                .resolve(KagemushaArtifactRoleV1::MintCreditVkEq)?
                .as_ref(),
            profile.mint_eq,
        )?;
        let ep_mint_vk = read_ep_mint_vk(
            artifacts
                .resolve(KagemushaArtifactRoleV1::MintCreditVkEp)?
                .as_ref(),
            profile.mint_ep,
        )?;
        let inner_eq_state_protocol = compile(
            &eq_parameters,
            &inner_eq_state_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![RECURSIVE_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let inner_ep_state_protocol = compile(
            &ep_parameters,
            &inner_ep_state_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![RECURSIVE_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let inner_eq_protocol_digest =
            native_parent_protocol_digest_v1(&inner_eq_state_protocol, KagemushaPastaParityV1::Eq)
                .map_err(KagemushaArtifactErrorV1::InvalidRelease)?;
        let inner_ep_protocol_digest =
            native_parent_protocol_digest_v1(&inner_ep_state_protocol, KagemushaPastaParityV1::Ep)
                .map_err(KagemushaArtifactErrorV1::InvalidRelease)?;
        let eq_state_protocol = compile(
            &eq_parameters,
            &eq_state_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![RECURSIVE_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let ep_state_protocol = compile(
            &ep_parameters,
            &ep_state_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![RECURSIVE_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let eq_protocol_digest =
            native_parent_protocol_digest_v1(&eq_state_protocol, KagemushaPastaParityV1::Eq)
                .map_err(KagemushaArtifactErrorV1::InvalidRelease)?;
        let ep_protocol_digest =
            native_parent_protocol_digest_v1(&ep_state_protocol, KagemushaPastaParityV1::Ep)
                .map_err(KagemushaArtifactErrorV1::InvalidRelease)?;
        if eq_protocol_digest != recursion.eq_protocol_digest
            || ep_protocol_digest != recursion.ep_protocol_digest
            || inner_eq_protocol_digest == eq_protocol_digest
            || inner_ep_protocol_digest == ep_protocol_digest
            || inner_eq_protocol_digest == inner_ep_protocol_digest
        {
            return Err(KagemushaArtifactErrorV1::InvalidRelease(
                "compiled inner/outer state protocol identity mismatch".to_owned(),
            ));
        }
        let eq_guard_protocol = compile(
            &eq_parameters,
            &eq_guard_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let ep_guard_protocol = compile(
            &ep_parameters,
            &ep_guard_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let guard_eq_protocol_digest =
            native_parent_protocol_digest_v1(&eq_guard_protocol, KagemushaPastaParityV1::Eq)
                .map_err(KagemushaArtifactErrorV1::InvalidRelease)?;
        let guard_ep_protocol_digest =
            native_parent_protocol_digest_v1(&ep_guard_protocol, KagemushaPastaParityV1::Ep)
                .map_err(KagemushaArtifactErrorV1::InvalidRelease)?;
        let expected_guard_eq_protocol_digest = recursion
            .guard_bundle_protocol_digest(KagemushaPastaParityV1::Eq)
            .map_err(|error| KagemushaArtifactErrorV1::InvalidRelease(error.to_string()))?;
        let expected_guard_ep_protocol_digest = recursion
            .guard_bundle_protocol_digest(KagemushaPastaParityV1::Ep)
            .map_err(|error| KagemushaArtifactErrorV1::InvalidRelease(error.to_string()))?;
        validate_authenticated_guard_protocol_binding_v1(
            guard_eq_protocol_digest,
            guard_ep_protocol_digest,
            expected_guard_eq_protocol_digest,
            expected_guard_ep_protocol_digest,
        )
        .map_err(KagemushaArtifactErrorV1::InvalidRelease)?;
        if guard_eq_protocol_digest == guard_ep_protocol_digest
            || guard_eq_protocol_digest == eq_protocol_digest
            || guard_ep_protocol_digest == ep_protocol_digest
        {
            return Err(KagemushaArtifactErrorV1::InvalidRelease(
                "compiled recursive protocol roles alias".to_owned(),
            ));
        }
        let eq_terminal_authorization_protocol = compile(
            &eq_parameters,
            &eq_terminal_authorization_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let ep_terminal_authorization_protocol = compile(
            &ep_parameters,
            &ep_terminal_authorization_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let terminal_authorization_eq_protocol_digest = native_parent_protocol_digest_v1(
            &eq_terminal_authorization_protocol,
            KagemushaPastaParityV1::Eq,
        )
        .map_err(KagemushaArtifactErrorV1::InvalidRelease)?;
        let terminal_authorization_ep_protocol_digest = native_parent_protocol_digest_v1(
            &ep_terminal_authorization_protocol,
            KagemushaPastaParityV1::Ep,
        )
        .map_err(KagemushaArtifactErrorV1::InvalidRelease)?;
        if terminal_authorization_eq_protocol_digest
            != recursion.terminal_authorization_eq_protocol_digest
            || terminal_authorization_ep_protocol_digest
                != recursion.terminal_authorization_ep_protocol_digest
        {
            return Err(KagemushaArtifactErrorV1::InvalidRelease(
                "compiled terminal-authorization protocol identity mismatch".to_owned(),
            ));
        }
        let eq_commit_wrapper_protocol = compile(
            &eq_parameters,
            &eq_commit_wrapper_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let ep_commit_wrapper_protocol = compile(
            &ep_parameters,
            &ep_commit_wrapper_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let commit_wrapper_eq_protocol_digest = native_parent_protocol_digest_v1(
            &eq_commit_wrapper_protocol,
            KagemushaPastaParityV1::Eq,
        )
        .map_err(KagemushaArtifactErrorV1::InvalidRelease)?;
        let commit_wrapper_ep_protocol_digest = native_parent_protocol_digest_v1(
            &ep_commit_wrapper_protocol,
            KagemushaPastaParityV1::Ep,
        )
        .map_err(KagemushaArtifactErrorV1::InvalidRelease)?;
        if commit_wrapper_eq_protocol_digest != recursion.commit_wrapper_eq_protocol_digest
            || commit_wrapper_ep_protocol_digest != recursion.commit_wrapper_ep_protocol_digest
        {
            return Err(KagemushaArtifactErrorV1::InvalidRelease(
                "compiled commit-wrapper protocol identity mismatch".to_owned(),
            ));
        }
        let eq_mint_authorization_protocol = compile(
            &eq_parameters,
            &eq_mint_authorization_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let ep_mint_authorization_protocol = compile(
            &ep_parameters,
            &ep_mint_authorization_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let mint_authorization_eq_protocol_digest = native_parent_protocol_digest_v1(
            &eq_mint_authorization_protocol,
            KagemushaPastaParityV1::Eq,
        )
        .map_err(KagemushaArtifactErrorV1::InvalidRelease)?;
        let mint_authorization_ep_protocol_digest = native_parent_protocol_digest_v1(
            &ep_mint_authorization_protocol,
            KagemushaPastaParityV1::Ep,
        )
        .map_err(KagemushaArtifactErrorV1::InvalidRelease)?;
        if mint_authorization_eq_protocol_digest != recursion.mint_authorization_eq_protocol_digest
            || mint_authorization_ep_protocol_digest
                != recursion.mint_authorization_ep_protocol_digest
        {
            return Err(KagemushaArtifactErrorV1::InvalidRelease(
                "compiled mint-authorization protocol identity mismatch".to_owned(),
            ));
        }
        let eq_mint_protocol = compile(
            &eq_parameters,
            &eq_mint_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let ep_mint_protocol = compile(
            &ep_parameters,
            &ep_mint_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let mint_eq_protocol_digest =
            native_parent_protocol_digest_v1(&eq_mint_protocol, KagemushaPastaParityV1::Eq)
                .map_err(KagemushaArtifactErrorV1::InvalidRelease)?;
        let mint_ep_protocol_digest =
            native_parent_protocol_digest_v1(&ep_mint_protocol, KagemushaPastaParityV1::Ep)
                .map_err(KagemushaArtifactErrorV1::InvalidRelease)?;
        validate_authenticated_mint_protocol_binding_v1(
            [mint_eq_protocol_digest, mint_ep_protocol_digest],
            [
                profile.mint_eq_protocol_digest,
                profile.mint_ep_protocol_digest,
            ],
            [
                recursion.mint_finality_eq_protocol_digest,
                recursion.mint_finality_ep_protocol_digest,
            ],
        )
        .map_err(KagemushaArtifactErrorV1::InvalidRelease)?;
        let protocol_digests = [
            inner_eq_protocol_digest,
            inner_ep_protocol_digest,
            eq_protocol_digest,
            ep_protocol_digest,
            guard_eq_protocol_digest,
            guard_ep_protocol_digest,
            terminal_authorization_eq_protocol_digest,
            terminal_authorization_ep_protocol_digest,
            commit_wrapper_eq_protocol_digest,
            commit_wrapper_ep_protocol_digest,
            mint_authorization_eq_protocol_digest,
            mint_authorization_ep_protocol_digest,
            mint_eq_protocol_digest,
            mint_ep_protocol_digest,
        ];
        if protocol_digests
            .iter()
            .enumerate()
            .any(|(index, digest)| protocol_digests[index + 1..].contains(digest))
        {
            return Err(KagemushaArtifactErrorV1::InvalidRelease(
                "compiled recursive protocol roles alias".to_owned(),
            ));
        }
        let verifier = Self {
            eq_parameters,
            ep_parameters,
            eq_state_protocol,
            ep_state_protocol,
            eq_terminal_authorization_protocol,
            ep_terminal_authorization_protocol,
            eq_commit_wrapper_protocol,
            ep_commit_wrapper_protocol,
            eq_mint_authorization_protocol,
            ep_mint_authorization_protocol,
            eq_mint_protocol,
            ep_mint_protocol,
            eq_protocol_digest,
            ep_protocol_digest,
            inner_eq_protocol_digest,
            inner_ep_protocol_digest,
            guard_eq_protocol_digest,
            guard_ep_protocol_digest,
            terminal_authorization_eq_protocol_digest,
            terminal_authorization_ep_protocol_digest,
            commit_wrapper_eq_protocol_digest,
            commit_wrapper_ep_protocol_digest,
            mint_authorization_eq_protocol_digest,
            mint_authorization_ep_protocol_digest,
            mint_eq_protocol_digest,
            mint_ep_protocol_digest,
            mint_genesis_roster_id: profile.mint_genesis_roster_id,
            release_id: recursion.release_id,
            suite_id: artifacts.suite_id(),
            vk_set_digest: artifacts.vk_set_digest(),
            artifact_manifest_digest: recursion.artifact_manifest_digest,
            terminal_authorization_eq_binding: recursion.terminal_authorization_verifying_key_eq,
            terminal_authorization_ep_binding: recursion.terminal_authorization_verifying_key_ep,
            commit_wrapper_eq_binding: recursion.commit_wrapper_verifying_key_eq,
            commit_wrapper_ep_binding: recursion.commit_wrapper_verifying_key_ep,
        };
        Ok(verifier)
    }

    /// Return the actual Eq state protocol identity derived from its authenticated key.
    #[must_use]
    pub const fn state_eq_protocol_digest(&self) -> [u8; 32] {
        self.eq_protocol_digest
    }

    /// Return the actual Ep state protocol identity derived from its authenticated key.
    #[must_use]
    pub const fn state_ep_protocol_digest(&self) -> [u8; 32] {
        self.ep_protocol_digest
    }

    /// Return the actual Eq private-carrier protocol identity derived from its authenticated key.
    #[must_use]
    pub const fn inner_state_eq_protocol_digest(&self) -> [u8; 32] {
        self.inner_eq_protocol_digest
    }

    /// Return the actual Ep private-carrier protocol identity derived from its authenticated key.
    #[must_use]
    pub const fn inner_state_ep_protocol_digest(&self) -> [u8; 32] {
        self.inner_ep_protocol_digest
    }

    /// Return the actual Eq GuardBundle protocol identity derived from its authenticated key.
    #[must_use]
    pub const fn guard_eq_protocol_digest(&self) -> [u8; 32] {
        self.guard_eq_protocol_digest
    }

    /// Return the actual Ep GuardBundle protocol identity derived from its authenticated key.
    #[must_use]
    pub const fn guard_ep_protocol_digest(&self) -> [u8; 32] {
        self.guard_ep_protocol_digest
    }

    /// Return the authenticated actual Eq terminal-authorization protocol identity.
    #[must_use]
    pub const fn terminal_authorization_eq_protocol_digest(&self) -> [u8; 32] {
        self.terminal_authorization_eq_protocol_digest
    }

    /// Return the authenticated actual Ep terminal-authorization protocol identity.
    #[must_use]
    pub const fn terminal_authorization_ep_protocol_digest(&self) -> [u8; 32] {
        self.terminal_authorization_ep_protocol_digest
    }

    /// Return the authenticated Eq post-commit wrapper protocol identity.
    #[must_use]
    pub const fn commit_wrapper_eq_protocol_digest(&self) -> [u8; 32] {
        self.commit_wrapper_eq_protocol_digest
    }

    /// Return the authenticated Ep post-commit wrapper protocol identity.
    #[must_use]
    pub const fn commit_wrapper_ep_protocol_digest(&self) -> [u8; 32] {
        self.commit_wrapper_ep_protocol_digest
    }

    /// Return the authenticated actual Eq mint-authorization protocol identity.
    #[must_use]
    pub const fn mint_authorization_eq_protocol_digest(&self) -> [u8; 32] {
        self.mint_authorization_eq_protocol_digest
    }

    /// Return the authenticated actual Ep mint-authorization protocol identity.
    #[must_use]
    pub const fn mint_authorization_ep_protocol_digest(&self) -> [u8; 32] {
        self.mint_authorization_ep_protocol_digest
    }

    /// Verify and decide the release-pinned paired recipient mint authorization.
    ///
    /// The hardware authorization is the Ep credential-audit projection. The Eq projection is
    /// fixed to the fresh recipient-credential commitment so neither proof can be replayed with a
    /// different top-up statement.
    pub fn verify_mint_authorization(
        &self,
        authorization: &KagemushaMintAuthorizationV1,
    ) -> Result<(), String> {
        authorization
            .validate_shape()
            .map_err(|error| error.to_string())?;
        let proof = &authorization.proof;
        if proof.eq_protocol_digest != self.mint_authorization_eq_protocol_digest
            || proof.ep_protocol_digest != self.mint_authorization_ep_protocol_digest
            || authorization.statement.context.release_id != self.release_id
            || authorization.statement.context.artifact_manifest_digest
                != self.artifact_manifest_digest
            || proof.guard_eq_credential_audit
                != authorization
                    .statement
                    .context
                    .recipient_credential_commitment
        {
            return Err("Kagemusha mint-authorization release binding mismatch".to_owned());
        }
        validate_proof_length("Eq mint authorization", &proof.eq_proof)?;
        validate_proof_length("Ep mint authorization", &proof.ep_proof)?;
        let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&proof.eq_history)
            .map_err(|error| error.to_string())?;
        let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&proof.ep_history)
            .map_err(|error| error.to_string())?;
        let eq_instances = mint_authorization_public_instances_v1::<Fp>(
            &authorization.statement,
            proof.guard_ep_credential_audit,
            proof.eq_deferred_audit,
            proof.ep_deferred_audit,
            eq_history.as_bytes(),
        )?;
        let ep_instances = mint_authorization_public_instances_v1::<Fq>(
            &authorization.statement,
            proof.guard_ep_credential_audit,
            proof.eq_deferred_audit,
            proof.ep_deferred_audit,
            ep_history.as_bytes(),
        )?;
        let eq_current = KagemushaEqAccumulatorV1::from_native(&verify_eq_succinct_protocol(
            &self.eq_parameters,
            &self.eq_mint_authorization_protocol,
            &proof.eq_proof,
            &eq_instances,
        )?)
        .map_err(|error| error.to_string())?;
        let ep_current = KagemushaEpAccumulatorV1::from_native(&verify_ep_succinct_protocol(
            &self.ep_parameters,
            &self.ep_mint_authorization_protocol,
            &proof.ep_proof,
            &ep_instances,
        )?)
        .map_err(|error| error.to_string())?;
        decide_kagemusha_eq_accumulator_v1(&self.eq_parameters, &eq_current)
            .map_err(|error| error.to_string())?;
        decide_kagemusha_eq_accumulator_v1(&self.eq_parameters, &eq_history)
            .map_err(|error| error.to_string())?;
        decide_kagemusha_ep_accumulator_v1(&self.ep_parameters, &ep_current)
            .map_err(|error| error.to_string())?;
        decide_kagemusha_ep_accumulator_v1(&self.ep_parameters, &ep_history)
            .map_err(|error| error.to_string())
    }

    /// Return the authenticated actual Eq finalized-mint protocol identity.
    #[must_use]
    pub const fn mint_eq_protocol_digest(&self) -> [u8; 32] {
        self.mint_eq_protocol_digest
    }

    /// Return the authenticated actual Ep finalized-mint protocol identity.
    #[must_use]
    pub const fn mint_ep_protocol_digest(&self) -> [u8; 32] {
        self.mint_ep_protocol_digest
    }

    /// Return the release-pinned genesis mint-finality roster identifier.
    #[must_use]
    pub const fn mint_genesis_roster_id(&self) -> [u8; 32] {
        self.mint_genesis_roster_id
    }

    /// Reverify a release bootstrap or Kura-persisted rotation authority checkpoint.
    pub fn verify_mint_authority_checkpoint(
        &self,
        checkpoint: &KagemushaMintAuthorityCheckpointV1,
    ) -> Result<(KagemushaEqAccumulatorV1, KagemushaEpAccumulatorV1), String> {
        checkpoint.validate_shape()?;
        validate_mint_checkpoint_release_v1(
            checkpoint,
            self.release_id,
            self.mint_eq_protocol_digest,
            self.mint_ep_protocol_digest,
            self.mint_genesis_roster_id,
        )?;
        let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&checkpoint.proof.eq_history)
            .map_err(|error| error.to_string())?;
        let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&checkpoint.proof.ep_history)
            .map_err(|error| error.to_string())?;
        let semantic = checkpoint
            .statement
            .canonical_digest()
            .map_err(|error| error.to_string())?;
        let common = MintPublicPartsV1 {
            step: checkpoint.step,
            semantic_digest: semantic,
            amount: checkpoint.statement.amount,
            certificate_binding: checkpoint.certificate_binding,
            authority_head: checkpoint.authority_head,
            release_id: checkpoint.release_id,
            genesis_roster_id: checkpoint.genesis_roster_id,
            eq_protocol_digest: checkpoint.proof.eq_protocol_digest,
            ep_protocol_digest: checkpoint.proof.ep_protocol_digest,
            eq_deferred_audit: checkpoint.proof.eq_deferred_audit,
            ep_deferred_audit: checkpoint.proof.ep_deferred_audit,
            proof_binding_digest: checkpoint.proof_binding_digest,
        };
        let eq_instances = mint_public_instances_from_parts::<Fp>(&common, eq_history.as_bytes())?;
        let ep_instances = mint_public_instances_from_parts::<Fq>(&common, ep_history.as_bytes())?;
        let eq_current = KagemushaEqAccumulatorV1::from_native(&verify_eq_succinct_protocol(
            &self.eq_parameters,
            &self.eq_mint_protocol,
            &checkpoint.proof.eq_proof,
            &eq_instances,
        )?)
        .map_err(|error| error.to_string())?;
        let ep_current = KagemushaEpAccumulatorV1::from_native(&verify_ep_succinct_protocol(
            &self.ep_parameters,
            &self.ep_mint_protocol,
            &checkpoint.proof.ep_proof,
            &ep_instances,
        )?)
        .map_err(|error| error.to_string())?;
        decide_kagemusha_eq_accumulator_v1(&self.eq_parameters, &eq_current)
            .map_err(|error| error.to_string())?;
        decide_kagemusha_eq_accumulator_v1(&self.eq_parameters, &eq_history)
            .map_err(|error| error.to_string())?;
        decide_kagemusha_ep_accumulator_v1(&self.ep_parameters, &ep_current)
            .map_err(|error| error.to_string())?;
        decide_kagemusha_ep_accumulator_v1(&self.ep_parameters, &ep_history)
            .map_err(|error| error.to_string())?;
        Ok((eq_current, ep_current))
    }

    /// Verify and terminally decide the paired final post-commit payment proof.
    ///
    /// The exact request, release tuple, dedicated Eq/Ep protocol identities, current
    /// proofs, and both delayed histories are all authenticated here. Shape validation alone never
    /// grants monetary authority.
    ///
    /// # Errors
    ///
    /// Returns an error for any request/statement mutation, release or role substitution,
    /// malformed proof/history, failed Halo2 verification, or failed accumulator decision.
    pub fn verify_payment_and_decide(
        &self,
        request: &KagemushaPaymentRequestV1,
        payment: &KagemushaPaymentV1,
    ) -> Result<(), String> {
        payment
            .validate_shape_against(request)
            .map_err(|error| error.to_string())?;
        let proof = &payment.proof;
        if request.release_id != self.release_id
            || request.hardware_credential.suite_id != self.suite_id
            || proof.eq_protocol_digest != self.commit_wrapper_eq_protocol_digest
            || proof.ep_protocol_digest != self.commit_wrapper_ep_protocol_digest
            || self.commit_wrapper_eq_binding.role != KagemushaArtifactRoleV1::CommitWrapperVkEq
            || self.commit_wrapper_ep_binding.role != KagemushaArtifactRoleV1::CommitWrapperVkEp
            || self.commit_wrapper_eq_binding.sha256
                == self.terminal_authorization_eq_binding.sha256
            || self.commit_wrapper_ep_binding.sha256
                == self.terminal_authorization_ep_binding.sha256
        {
            return Err("Kagemusha post-commit payment release/key-role mismatch".to_owned());
        }
        validate_proof_length("Eq post-commit payment", &proof.eq_proof)?;
        validate_proof_length("Ep post-commit payment", &proof.ep_proof)?;
        let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&proof.eq_history)
            .map_err(|error| error.to_string())?;
        let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&proof.ep_history)
            .map_err(|error| error.to_string())?;
        let public = payment_terminal_public_inputs_v1(request, payment, self.vk_set_digest)?;
        let eq_instances =
            terminal_relation_public_instances::<Fp>(&public, eq_history.as_bytes())?;
        let ep_instances =
            terminal_relation_public_instances::<Fq>(&public, ep_history.as_bytes())?;
        let eq_current = verify_eq_succinct_protocol(
            &self.eq_parameters,
            &self.eq_commit_wrapper_protocol,
            &proof.eq_proof,
            &eq_instances,
        )?;
        let ep_current = verify_ep_succinct_protocol(
            &self.ep_parameters,
            &self.ep_commit_wrapper_protocol,
            &proof.ep_proof,
            &ep_instances,
        )?;
        let eq_current = KagemushaEqAccumulatorV1::from_native(&eq_current)
            .map_err(|error| error.to_string())?;
        let ep_current = KagemushaEpAccumulatorV1::from_native(&ep_current)
            .map_err(|error| error.to_string())?;
        decide_kagemusha_eq_accumulator_v1(&self.eq_parameters, &eq_current)
            .map_err(|error| error.to_string())?;
        decide_kagemusha_ep_accumulator_v1(&self.ep_parameters, &ep_current)
            .map_err(|error| error.to_string())?;
        decide_kagemusha_eq_accumulator_v1(&self.eq_parameters, &eq_history)
            .map_err(|error| error.to_string())?;
        decide_kagemusha_ep_accumulator_v1(&self.ep_parameters, &ep_history)
            .map_err(|error| error.to_string())
    }

    fn verify_terminal_authorization_request(
        &self,
        request: &KagemushaParityVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        if request.public_output.lifecycle.release_id != self.release_id
            || request.public_output.lifecycle.suite_id != self.suite_id
            || request.public_output.lifecycle.vk_digest != self.vk_set_digest
            || self.commit_wrapper_eq_binding.role != KagemushaArtifactRoleV1::CommitWrapperVkEq
            || self.commit_wrapper_ep_binding.role != KagemushaArtifactRoleV1::CommitWrapperVkEp
            || request.protocol_digest
                != match request.parity {
                    KagemushaPastaParityV1::Eq => self.commit_wrapper_eq_protocol_digest,
                    KagemushaPastaParityV1::Ep => self.commit_wrapper_ep_protocol_digest,
                }
        {
            return Err("Kagemusha post-commit wrapper release/key-role mismatch".to_owned());
        }
        validate_proof_length("post-commit wrapper", request.current_proof)?;
        match request.parity {
            KagemushaPastaParityV1::Eq => {
                let instances = terminal_authorization_public_instances::<Fp>(
                    request,
                    self.commit_wrapper_eq_protocol_digest,
                    self.commit_wrapper_ep_protocol_digest,
                )?;
                let current = verify_eq_succinct_protocol(
                    &self.eq_parameters,
                    &self.eq_commit_wrapper_protocol,
                    request.current_proof,
                    &instances,
                )?;
                let current = KagemushaEqAccumulatorV1::from_native(&current)
                    .map_err(|error| error.to_string())?;
                decide_kagemusha_eq_accumulator_v1(&self.eq_parameters, &current)
                    .map_err(|error| error.to_string())?;
                let history = KagemushaEqAccumulatorV1::try_from_bytes(request.history_accumulator)
                    .map_err(|error| error.to_string())?;
                decide_kagemusha_eq_accumulator_v1(&self.eq_parameters, &history)
                    .map_err(|error| error.to_string())
            }
            KagemushaPastaParityV1::Ep => {
                let instances = terminal_authorization_public_instances::<Fq>(
                    request,
                    self.commit_wrapper_eq_protocol_digest,
                    self.commit_wrapper_ep_protocol_digest,
                )?;
                let current = verify_ep_succinct_protocol(
                    &self.ep_parameters,
                    &self.ep_commit_wrapper_protocol,
                    request.current_proof,
                    &instances,
                )?;
                let current = KagemushaEpAccumulatorV1::from_native(&current)
                    .map_err(|error| error.to_string())?;
                decide_kagemusha_ep_accumulator_v1(&self.ep_parameters, &current)
                    .map_err(|error| error.to_string())?;
                let history = KagemushaEpAccumulatorV1::try_from_bytes(request.history_accumulator)
                    .map_err(|error| error.to_string())?;
                decide_kagemusha_ep_accumulator_v1(&self.ep_parameters, &history)
                    .map_err(|error| error.to_string())
            }
        }
    }
}

impl KagemushaRecursiveVerifierV1 for KagemushaAuthenticatedRecursiveVerifierV1 {
    fn verify_state_proof_and_decide(
        &self,
        request: &KagemushaStateProofVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        if request.public_inputs.commit_wrapper_eq_protocol_digest
            != self.commit_wrapper_eq_protocol_digest
            || request.public_inputs.commit_wrapper_ep_protocol_digest
                != self.commit_wrapper_ep_protocol_digest
        {
            return Err("Kagemusha state commit-wrapper release binding mismatch".to_owned());
        }
        validate_proof_length("Eq aggregate state", &request.proof.eq_proof)?;
        validate_proof_length("Ep aggregate state", &request.proof.ep_proof)?;
        let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&request.proof.eq_history)
            .map_err(|error| error.to_string())?;
        let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&request.proof.ep_history)
            .map_err(|error| error.to_string())?;
        let mut eq_instances = request.public_inputs.public_instances::<Fp>()?;
        eq_instances.extend(history_public_instances::<Fp>(eq_history.as_bytes()));
        let mut ep_instances = request.public_inputs.public_instances::<Fq>()?;
        ep_instances.extend(history_public_instances::<Fq>(ep_history.as_bytes()));
        if eq_instances.len() != RECURSIVE_PUBLIC_INSTANCE_COUNT_V1
            || ep_instances.len() != RECURSIVE_PUBLIC_INSTANCE_COUNT_V1
        {
            return Err("Kagemusha state public instance ABI mismatch".to_owned());
        }
        let eq_current = verify_eq_succinct_protocol(
            &self.eq_parameters,
            &self.eq_state_protocol,
            &request.proof.eq_proof,
            &eq_instances,
        )?;
        let ep_current = verify_ep_succinct_protocol(
            &self.ep_parameters,
            &self.ep_state_protocol,
            &request.proof.ep_proof,
            &ep_instances,
        )?;
        let eq_current = KagemushaEqAccumulatorV1::from_native(&eq_current)
            .map_err(|error| error.to_string())?;
        let ep_current = KagemushaEpAccumulatorV1::from_native(&ep_current)
            .map_err(|error| error.to_string())?;
        decide_kagemusha_eq_accumulator_v1(&self.eq_parameters, &eq_current)
            .map_err(|error| error.to_string())?;
        decide_kagemusha_ep_accumulator_v1(&self.ep_parameters, &ep_current)
            .map_err(|error| error.to_string())?;
        decide_kagemusha_eq_accumulator_v1(&self.eq_parameters, &eq_history)
            .map_err(|error| error.to_string())?;
        decide_kagemusha_ep_accumulator_v1(&self.ep_parameters, &ep_history)
            .map_err(|error| error.to_string())
    }

    fn verify_mint_finality_helper(
        &self,
        request: &super::KagemushaMintFinalityHelperVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        validate_mint_finality_release_v1(
            request,
            self.release_id,
            self.mint_eq_protocol_digest,
            self.mint_ep_protocol_digest,
            self.mint_genesis_roster_id,
            self.artifact_manifest_digest,
        )?;
        validate_proof_length("Eq mint-authority", &request.proof.eq_proof)?;
        validate_proof_length("Ep mint-authority", &request.proof.ep_proof)?;
        let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&request.proof.eq_history)
            .map_err(|error| error.to_string())?;
        let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&request.proof.ep_history)
            .map_err(|error| error.to_string())?;
        let eq_instances = mint_public_instances::<Fp>(request, eq_history.as_bytes())?;
        let ep_instances = mint_public_instances::<Fq>(request, ep_history.as_bytes())?;
        let eq_current = verify_eq_succinct_protocol(
            &self.eq_parameters,
            &self.eq_mint_protocol,
            &request.proof.eq_proof,
            &eq_instances,
        )?;
        let ep_current = verify_ep_succinct_protocol(
            &self.ep_parameters,
            &self.ep_mint_protocol,
            &request.proof.ep_proof,
            &ep_instances,
        )?;
        let eq_current = KagemushaEqAccumulatorV1::from_native(&eq_current)
            .map_err(|error| error.to_string())?;
        let ep_current = KagemushaEpAccumulatorV1::from_native(&ep_current)
            .map_err(|error| error.to_string())?;
        decide_kagemusha_eq_accumulator_v1(&self.eq_parameters, &eq_current)
            .map_err(|error| error.to_string())?;
        decide_kagemusha_ep_accumulator_v1(&self.ep_parameters, &ep_current)
            .map_err(|error| error.to_string())?;
        decide_kagemusha_eq_accumulator_v1(&self.eq_parameters, &eq_history)
            .map_err(|error| error.to_string())?;
        decide_kagemusha_ep_accumulator_v1(&self.ep_parameters, &ep_history)
            .map_err(|error| error.to_string())
    }

    fn verify_payment_and_decide(
        &self,
        request: &KagemushaPaymentRequestV1,
        payment: &KagemushaPaymentV1,
    ) -> Result<(), String> {
        KagemushaAuthenticatedRecursiveVerifierV1::verify_payment_and_decide(self, request, payment)
    }

    fn verify_terminal_authorization_and_decide(
        &self,
        request: &KagemushaParityVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        self.verify_terminal_authorization_request(request)
    }
}

fn append_base_params(bytes: &mut Vec<u8>, params: &BaseCircuitParams) -> Result<(), String> {
    append_usize(bytes, params.k)?;
    append_usize_slice(bytes, &params.num_advice_per_phase)?;
    append_usize(bytes, params.num_fixed)?;
    append_usize_slice(bytes, &params.num_lookup_advice_per_phase)?;
    match params.lookup_bits {
        Some(value) => {
            bytes.push(1);
            append_usize(bytes, value)?;
        }
        None => bytes.push(0),
    }
    append_usize(bytes, params.num_instance_columns)
}

fn append_usize(bytes: &mut Vec<u8>, value: usize) -> Result<(), String> {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .map_err(|_| "Kagemusha circuit profile value exceeds u64".to_owned())?
            .to_le_bytes(),
    );
    Ok(())
}

fn append_usize_slice(bytes: &mut Vec<u8>, values: &[usize]) -> Result<(), String> {
    append_usize(bytes, values.len())?;
    for value in values {
        append_usize(bytes, *value)?;
    }
    Ok(())
}

fn read_eq_state_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactErrorV1> {
    read_eq_recursive_vk::<KagemushaTransportDeciderEqCircuitV1>(
        bytes,
        params,
        "state transport decider",
    )
}

fn read_ep_state_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactErrorV1> {
    read_ep_recursive_vk::<KagemushaTransportDeciderEpCircuitV1>(
        bytes,
        params,
        "state transport decider",
    )
}

fn read_eq_inner_state_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactErrorV1> {
    read_eq_recursive_vk::<KagemushaRecursiveStateEqCircuitV1>(
        bytes,
        params,
        "private state carrier",
    )
}

fn read_ep_inner_state_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactErrorV1> {
    read_ep_recursive_vk::<KagemushaRecursiveStateEpCircuitV1>(
        bytes,
        params,
        "private state carrier",
    )
}

fn read_eq_guard_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactErrorV1> {
    read_eq_recursive_vk::<KagemushaGuardBundleEqCircuitV1>(bytes, params, "GuardBundle")
}

fn read_ep_guard_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactErrorV1> {
    read_ep_recursive_vk::<KagemushaGuardBundleEpCircuitV1>(bytes, params, "GuardBundle")
}

fn read_eq_terminal_authorization_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactErrorV1> {
    read_eq_recursive_vk::<KagemushaTerminalAuthorizationEqCircuitV1>(
        bytes,
        params,
        "terminal authorization",
    )
}

fn read_ep_terminal_authorization_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactErrorV1> {
    read_ep_recursive_vk::<KagemushaTerminalAuthorizationEpCircuitV1>(
        bytes,
        params,
        "terminal authorization",
    )
}

fn read_eq_commit_wrapper_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactErrorV1> {
    read_eq_recursive_vk::<KagemushaCommitWrapperEqCircuitV1>(bytes, params, "commit-wrapper")
}

fn read_ep_commit_wrapper_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactErrorV1> {
    read_ep_recursive_vk::<KagemushaCommitWrapperEpCircuitV1>(bytes, params, "commit-wrapper")
}

fn read_eq_mint_authorization_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactErrorV1> {
    read_eq_recursive_vk::<KagemushaMintAuthorizationTransportEqCircuitV1>(
        bytes,
        params,
        "mint authorization",
    )
}

fn read_ep_mint_authorization_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactErrorV1> {
    read_ep_recursive_vk::<KagemushaMintAuthorizationTransportEpCircuitV1>(
        bytes,
        params,
        "mint authorization",
    )
}

fn read_eq_mint_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactErrorV1> {
    // TODO: Complete the authority generator's inner/outer bootstrap structure fixed point
    // before qualifying a mint release. The accepting reader has no raw-carrier fallback.
    read_eq_recursive_vk::<KagemushaMintAuthorityTransportEqCircuitV1>(
        bytes,
        params,
        "mint authority",
    )
}

fn read_ep_mint_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactErrorV1> {
    read_ep_recursive_vk::<KagemushaMintAuthorityTransportEpCircuitV1>(
        bytes,
        params,
        "mint authority",
    )
}

fn read_eq_recursive_vk<C>(
    bytes: &[u8],
    params: BaseCircuitParams,
    label: &str,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactErrorV1>
where
    C: halo2_proofs::plonk::Circuit<Fp, Params = BaseCircuitParams>,
{
    read_recursive_vk_checked::<EqAffine, C>(
        bytes,
        params,
        KAGEMUSHA_HALO2_K_V1,
        &format!("Eq {label}"),
    )
}

fn read_ep_recursive_vk<C>(
    bytes: &[u8],
    params: BaseCircuitParams,
    label: &str,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactErrorV1>
where
    C: halo2_proofs::plonk::Circuit<Fq, Params = BaseCircuitParams>,
{
    read_recursive_vk_checked::<EpAffine, C>(
        bytes,
        params,
        KAGEMUSHA_HALO2_K_V1,
        &format!("Ep {label}"),
    )
}

// All production callers pin K16 above. A separate private helper permits tiny real-key
// regression tests without substituting their degree into an accepting verifier.
fn read_recursive_vk_checked<C, ConcreteCircuit>(
    bytes: &[u8],
    params: ConcreteCircuit::Params,
    expected_k: u32,
    label: &str,
) -> Result<VerifyingKey<C>, KagemushaArtifactErrorV1>
where
    C: halo2_proofs::SerdeCurveAffine,
    C::Scalar: halo2_proofs::SerdePrimeField + ff::FromUniformBytes<64>,
    ConcreteCircuit: halo2_proofs::plonk::Circuit<C::Scalar>,
{
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read_checked::<_, ConcreteCircuit>(
        &mut cursor,
        SerdeFormat::Processed,
        expected_k,
        params,
    )
    .map_err(|error| {
        KagemushaArtifactErrorV1::InvalidRelease(format!(
            "failed to decode {label} verifying key: {error}"
        ))
    })?;
    if cursor.position() != bytes.len() as u64 {
        return Err(KagemushaArtifactErrorV1::InvalidRelease(format!(
            "{label} verifying key has trailing bytes"
        )));
    }
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(KagemushaArtifactErrorV1::InvalidRelease(format!(
            "{label} verifying key has noncanonical processed encoding"
        )));
    }
    Ok(key)
}

#[cfg(test)]
mod checked_loader_tests {
    use std::{
        marker::PhantomData,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use halo2_base::gates::circuit::BaseConfig;
    use halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner, Value},
        plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Selector, keygen_vk_custom},
        poly::{Rotation, ipa::commitment::ParamsIPA},
    };
    use iroha_data_model::kagemusha::KagemushaArtifactBindingV1;

    use super::*;

    fn base_params() -> BaseCircuitParams {
        BaseCircuitParams {
            k: KAGEMUSHA_HALO2_K_V1 as usize,
            num_advice_per_phase: vec![1],
            num_fixed: 1,
            num_lookup_advice_per_phase: vec![1],
            lookup_bits: Some((KAGEMUSHA_HALO2_K_V1 - 1) as usize),
            num_instance_columns: 1,
        }
    }

    fn shard_base_params() -> BaseCircuitParams {
        BaseCircuitParams {
            k: KAGEMUSHA_MINT_HASH_SHARD_K_V1 as usize,
            lookup_bits: Some((KAGEMUSHA_MINT_HASH_SHARD_K_V1 - 1) as usize),
            ..base_params()
        }
    }

    fn inner_mint_authorization_base_params() -> BaseCircuitParams {
        BaseCircuitParams {
            num_instance_columns: 2,
            ..base_params()
        }
    }

    fn mint_hash_claim_base_params() -> BaseCircuitParams {
        BaseCircuitParams {
            num_instance_columns: 3,
            ..base_params()
        }
    }

    fn profile() -> KagemushaRecursiveVerifierProfileV1 {
        KagemushaRecursiveVerifierProfileV1 {
            inner_state_eq: base_params(),
            inner_state_ep: base_params(),
            state_eq: base_params(),
            state_ep: base_params(),
            guard_eq: base_params(),
            guard_ep: base_params(),
            terminal_authorization_eq: base_params(),
            terminal_authorization_ep: base_params(),
            commit_wrapper_eq: base_params(),
            commit_wrapper_ep: base_params(),
            mint_authorization_eq: base_params(),
            mint_authorization_ep: base_params(),
            mint_eq: base_params(),
            mint_ep: base_params(),
            inner_mint_authorization_eq: inner_mint_authorization_base_params(),
            inner_mint_authorization_ep: inner_mint_authorization_base_params(),
            inner_mint_eq: base_params(),
            inner_mint_ep: base_params(),
            mint_hash_shard_eq: shard_base_params(),
            mint_hash_shard_ep: shard_base_params(),
            mint_hash_claim_eq: mint_hash_claim_base_params(),
            mint_hash_claim_ep: mint_hash_claim_base_params(),
            mint_eq_protocol_digest: crate::zk::kagemusha_v1_poseidon::encode(Fp::from(1)),
            mint_ep_protocol_digest: crate::zk::kagemusha_v1_poseidon::encode(Fq::from(2)),
            mint_hash_shard_eq_protocol_digest: crate::zk::kagemusha_v1_poseidon::encode(Fp::from(
                4,
            )),
            mint_hash_shard_ep_protocol_digest: crate::zk::kagemusha_v1_poseidon::encode(Fq::from(
                5,
            )),
            mint_hash_claim_eq_protocol_digest: crate::zk::kagemusha_v1_poseidon::encode(Fp::from(
                6,
            )),
            mint_hash_claim_ep_protocol_digest: crate::zk::kagemusha_v1_poseidon::encode(Fq::from(
                7,
            )),
            mint_genesis_roster_id: [3; 32],
        }
    }

    #[test]
    fn checked_profile_authenticates_every_private_mint_layout() {
        let baseline = profile();
        let expected = baseline.canonical_digest().expect("valid full profile");
        let mut changed_digests = std::collections::BTreeSet::new();
        for index in 0..4 {
            let mut changed = baseline.clone();
            let params = match index {
                0 => &mut changed.inner_mint_authorization_eq,
                1 => &mut changed.inner_mint_authorization_ep,
                2 => &mut changed.inner_mint_eq,
                _ => &mut changed.inner_mint_ep,
            };
            params.num_fixed += 1;
            let digest = changed.canonical_digest().expect("valid changed layout");
            assert_ne!(digest, expected);
            assert!(
                changed_digests.insert(digest),
                "private roles have unique tags"
            );
        }
        for index in 0..4 {
            let mut invalid = baseline.clone();
            let params = match index {
                0 => &mut invalid.inner_mint_authorization_eq,
                1 => &mut invalid.inner_mint_authorization_ep,
                2 => &mut invalid.inner_mint_eq,
                _ => &mut invalid.inner_mint_ep,
            };
            params.num_advice_per_phase = vec![0, 1];
            assert!(invalid.canonical_digest().is_err());
        }
    }

    #[test]
    fn checked_profile_requires_two_authorization_and_three_claim_instance_columns() {
        let mut wrong_inner = profile();
        wrong_inner.inner_mint_authorization_eq.num_instance_columns = 1;
        assert!(wrong_inner.canonical_digest().is_err());

        let mut wrong_claim = profile();
        wrong_claim.mint_hash_claim_eq.num_instance_columns = 2;
        assert!(wrong_claim.canonical_digest().is_err());

        let mut wrong_outer = profile();
        wrong_outer.mint_authorization_eq.num_instance_columns = 2;
        assert!(wrong_outer.canonical_digest().is_err());
    }

    #[test]
    fn hybrid_native_parser_counts_and_orders_proof_supplied_commitments() {
        assert_eq!(
            hybrid_proof_supplied_commitment_bytes::<EqAffine>(1, "Eq").expect("one Eq commitment"),
            32
        );
        assert_eq!(
            hybrid_proof_supplied_commitment_bytes::<EpAffine>(2, "Ep")
                .expect("two Ep commitments"),
            64
        );

        validate_hybrid_commitment_limb_indices(8, &[[2, 3]], "legacy")
            .expect("one canonical carrier");
        validate_hybrid_commitment_limb_indices(8, &[[2, 3], [4, 5]], "claim")
            .expect("two canonical carriers");
        for malformed in [
            &[][..],
            &[[3, 2]][..],
            &[[4, 5], [2, 3]][..],
            &[[2, 3], [5, 6]][..],
            &[[2, 3], [3, 4]][..],
            &[[2, 3], [7, 8]][..],
            &[[0, 1], [2, 3], [4, 5]][..],
        ] {
            assert!(
                validate_hybrid_commitment_limb_indices(8, malformed, "test").is_err(),
                "missing, extra, reversed, swapped, gapped, overlapping, or out-of-range limbs must fail"
            );
        }
    }

    #[test]
    fn checked_profile_authenticates_hash_layouts_and_protocols() {
        let baseline = profile();
        let expected = baseline.canonical_digest().expect("valid full profile");
        for index in 0..4 {
            let mut changed = baseline.clone();
            let params = match index {
                0 => &mut changed.mint_hash_shard_eq,
                1 => &mut changed.mint_hash_shard_ep,
                2 => &mut changed.mint_hash_claim_eq,
                _ => &mut changed.mint_hash_claim_ep,
            };
            params.num_fixed += 1;
            assert_ne!(
                changed.canonical_digest().expect("valid changed layout"),
                expected
            );
        }
        for index in 0..4 {
            let mut changed = baseline.clone();
            match index {
                0 => {
                    changed.mint_hash_shard_eq_protocol_digest =
                        crate::zk::kagemusha_v1_poseidon::encode(Fp::from(8));
                }
                1 => {
                    changed.mint_hash_shard_ep_protocol_digest =
                        crate::zk::kagemusha_v1_poseidon::encode(Fq::from(9));
                }
                2 => {
                    changed.mint_hash_claim_eq_protocol_digest =
                        crate::zk::kagemusha_v1_poseidon::encode(Fp::from(10));
                }
                _ => {
                    changed.mint_hash_claim_ep_protocol_digest =
                        crate::zk::kagemusha_v1_poseidon::encode(Fq::from(11));
                }
            }
            assert_ne!(
                changed
                    .canonical_digest()
                    .expect("canonical substituted digest"),
                expected
            );
        }
        let mut wrong_k = baseline.clone();
        wrong_k.mint_hash_shard_eq.k = KAGEMUSHA_HALO2_K_V1 as usize;
        wrong_k.mint_hash_shard_eq.lookup_bits = Some((KAGEMUSHA_HALO2_K_V1 - 1) as usize);
        assert!(wrong_k.canonical_digest().is_err());
        let mut aliased = baseline;
        aliased.mint_hash_claim_eq_protocol_digest = aliased.mint_hash_shard_eq_protocol_digest;
        assert!(aliased.canonical_digest().is_err());
        let mut cross_role_alias = profile();
        cross_role_alias.mint_hash_claim_eq_protocol_digest =
            cross_role_alias.mint_eq_protocol_digest;
        assert!(cross_role_alias.canonical_digest().is_err());
    }

    #[test]
    fn checked_profile_accepts_supported_unequal_phase_vectors() {
        for (advice, lookup) in [
            (vec![1], vec![0]),
            (vec![1], vec![1, 0, 0]),
            (vec![1, 1, 1], vec![1]),
            (vec![1], vec![0, 1, 1]),
            (vec![1, 0, 0], vec![0, 1, 1]),
            (vec![1, 0, 0, 0], vec![1]),
            (vec![1], vec![1, 0, 0, 0]),
            // The actual phase-zero optimization uses a selector, not this many columns.
            (vec![1], vec![usize::MAX]),
        ] {
            let params = BaseCircuitParams {
                num_advice_per_phase: advice,
                num_lookup_advice_per_phase: lookup,
                ..base_params()
            };
            validate_kagemusha_base_circuit_params_v1(&params).expect("supported allocator shape");
            crate::panic_hook::catch_unwind_suppressed(|| {
                BaseConfig::<Fp>::configure(&mut ConstraintSystem::default(), params.clone());
                BaseConfig::<Fq>::configure(&mut ConstraintSystem::default(), params.clone());
            })
            .expect("accepted shape must configure in either parity");
        }
    }

    #[test]
    fn checked_profile_rejects_phase_holes_and_column_overflow() {
        let malformed = vec![
            BaseCircuitParams {
                num_advice_per_phase: vec![0, 1],
                ..base_params()
            },
            BaseCircuitParams {
                num_advice_per_phase: vec![1, 0, 1],
                num_lookup_advice_per_phase: vec![0, 1],
                ..base_params()
            },
            BaseCircuitParams {
                num_lookup_advice_per_phase: vec![0, 0, 1],
                ..base_params()
            },
            BaseCircuitParams {
                num_advice_per_phase: vec![1, 1, 1, 1],
                ..base_params()
            },
            BaseCircuitParams {
                num_lookup_advice_per_phase: vec![1, 1, 1, 1],
                ..base_params()
            },
            BaseCircuitParams {
                num_advice_per_phase: vec![usize::MAX, 1],
                ..base_params()
            },
            BaseCircuitParams {
                num_lookup_advice_per_phase: vec![usize::MAX, 1],
                ..base_params()
            },
            BaseCircuitParams {
                num_fixed: usize::MAX,
                ..base_params()
            },
            BaseCircuitParams {
                num_advice_per_phase: vec![usize::MAX / 2],
                num_lookup_advice_per_phase: vec![0],
                ..base_params()
            },
        ];
        for params in &malformed {
            assert!(validate_kagemusha_base_circuit_params_v1(params).is_err());
            let mut invalid_profile = profile();
            invalid_profile.mint_eq = params.clone();
            assert!(invalid_profile.canonical_digest().is_err());
        }
    }

    struct CountingResolver(Arc<AtomicUsize>);

    impl KagemushaArtifactByteResolverV1 for CountingResolver {
        fn resolve_bytes(
            &self,
            binding: KagemushaArtifactBindingV1,
        ) -> Result<Arc<[u8]>, KagemushaArtifactErrorV1> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(KagemushaArtifactErrorV1::Missing(binding.role))
        }
    }

    fn unread_artifacts(
        reads: Arc<AtomicUsize>,
    ) -> KagemushaAuthenticatedArtifactSetV1<CountingResolver> {
        // This inventory only tests rejection ordering, not release authentication.
        KagemushaAuthenticatedArtifactSetV1::for_stream_tests(
            CountingResolver(reads),
            KagemushaArtifactBindingV1 {
                role: KagemushaArtifactRoleV1::StateVkEq,
                sha256: [5; 32],
                byte_len: 4,
            },
        )
    }

    #[test]
    fn checked_profile_rejects_before_any_artifact_read() {
        let reads = Arc::new(AtomicUsize::new(0));
        let artifacts = unread_artifacts(Arc::clone(&reads));
        let profile = profile();
        assert!(matches!(
            profile.validate_against_artifacts(&artifacts),
            Err(KagemushaArtifactErrorV1::InvalidRelease(reason))
                if reason.contains("profile digest mismatch")
        ));
        assert!(matches!(
            KagemushaAuthenticatedRecursiveVerifierV1::load(&artifacts, profile),
            Err(KagemushaArtifactErrorV1::InvalidRelease(reason))
                if reason.contains("profile digest mismatch")
        ));
        assert_eq!(reads.load(Ordering::SeqCst), 0);
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn checked_profile_generation_rejects_phase_holes_before_artifact_reads() {
        let reads = Arc::new(AtomicUsize::new(0));
        let artifacts = unread_artifacts(Arc::clone(&reads));
        let invalid = BaseCircuitParams {
            num_advice_per_phase: vec![0, 1],
            ..base_params()
        };
        let mut invalid_eq_profile = profile();
        invalid_eq_profile.mint_eq = invalid.clone();
        let mut invalid_ep_profile = profile();
        invalid_ep_profile.mint_ep = invalid;
        assert!(matches!(
            super::super::generation::load_kagemusha_eq_mint_authority_artifacts_v1(
                &artifacts,
                &invalid_eq_profile,
            ),
            Err(super::super::generation::KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
                KagemushaPastaParityV1::Eq,
            )),
        ));
        assert!(matches!(
            super::super::generation::load_kagemusha_ep_mint_authority_artifacts_v1(
                &artifacts,
                &invalid_ep_profile,
            ),
            Err(super::super::generation::KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
                KagemushaPastaParityV1::Ep,
            )),
        ));
        assert_eq!(reads.load(Ordering::SeqCst), 0);
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn checked_profile_generation_rejects_inner_phase_holes_before_artifact_reads() {
        let reads = Arc::new(AtomicUsize::new(0));
        let artifacts = unread_artifacts(Arc::clone(&reads));
        let invalid_inner = BaseCircuitParams {
            num_advice_per_phase: vec![0, 1],
            ..base_params()
        };
        let mut invalid_eq_profile = profile();
        invalid_eq_profile.inner_mint_eq = invalid_inner.clone();
        let mut invalid_ep_profile = profile();
        invalid_ep_profile.inner_mint_ep = invalid_inner;
        assert!(matches!(
            super::super::generation::load_kagemusha_eq_mint_authority_artifacts_v1(
                &artifacts,
                &invalid_eq_profile,
            ),
            Err(super::super::generation::KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
                KagemushaPastaParityV1::Eq,
            )),
        ));
        assert!(matches!(
            super::super::generation::load_kagemusha_ep_mint_authority_artifacts_v1(
                &artifacts,
                &invalid_ep_profile,
            ),
            Err(super::super::generation::KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
                KagemushaPastaParityV1::Ep,
            )),
        ));
        assert_eq!(reads.load(Ordering::SeqCst), 0);
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn checked_profile_generation_rejects_unauthenticated_profile_before_artifact_reads() {
        let reads = Arc::new(AtomicUsize::new(0));
        let artifacts = unread_artifacts(Arc::clone(&reads));
        let mismatching_profile = profile();
        mismatching_profile.canonical_digest().unwrap();
        assert!(matches!(
            super::super::generation::load_kagemusha_eq_mint_authority_artifacts_v1(
                &artifacts,
                &mismatching_profile,
            ),
            Err(super::super::generation::KagemushaArtifactGenerationErrorV1::Artifact(
                KagemushaArtifactErrorV1::InvalidRelease(reason),
            )) if reason.contains("profile digest mismatch"),
        ));
        assert!(matches!(
            super::super::generation::load_kagemusha_ep_mint_authority_artifacts_v1(
                &artifacts,
                &mismatching_profile,
            ),
            Err(super::super::generation::KagemushaArtifactGenerationErrorV1::Artifact(
                KagemushaArtifactErrorV1::InvalidRelease(reason),
            )) if reason.contains("profile digest mismatch"),
        ));
        assert_eq!(reads.load(Ordering::SeqCst), 0);
    }

    #[derive(Clone, Default)]
    struct SmallVkCircuit<F>(PhantomData<F>);

    impl<F: ff::PrimeField> Circuit<F> for SmallVkCircuit<F> {
        type Config = (Column<Advice>, Selector);
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self(PhantomData)
        }

        fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
            let advice = meta.advice_column();
            meta.enable_equality(advice);
            let selector = meta.selector();
            meta.create_gate("small checked-key gate", |meta| {
                let q = meta.query_selector(selector);
                let value = meta.query_advice(advice, Rotation::cur());
                vec![q * value]
            });
            (advice, selector)
        }

        fn synthesize(
            &self,
            (advice, selector): Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            layouter.assign_region(
                || "small checked-key row",
                |mut region| {
                    region.assign_advice(advice, 0, Value::known(F::ZERO));
                    selector.enable(&mut region, 0)
                },
            )
        }
    }

    #[test]
    fn checked_native_vk_k6_roundtrip_and_malformed_inputs() {
        // Actual tiny VKs exercise the decoder only; they are not K16 cash qualification.
        macro_rules! check_parity {
            ($curve:ty, $scalar:ty, $wrapper:ident, $circuit:ty) => {{
                let parameters = ParamsIPA::<$curve>::new(6);
                let circuit = SmallVkCircuit::<$scalar>::default();
                for compress_selectors in [false, true] {
                    let key = keygen_vk_custom(&parameters, &circuit, compress_selectors)
                        .expect("small native verifier key");
                    let bytes = key.to_bytes(SerdeFormat::Processed);
                    let recovered = read_recursive_vk_checked::<$curve, SmallVkCircuit<$scalar>>(
                        &bytes,
                        (),
                        6,
                        "test",
                    )
                    .expect("checked native key");
                    assert_eq!(recovered.to_bytes(SerdeFormat::Processed), bytes);
                    assert_eq!(recovered.transcript_repr(), key.transcript_repr());
                    assert!(
                        $wrapper::<$circuit>(&bytes, base_params(), "test").is_err(),
                        "production verifier retains mandatory K16"
                    );

                    let mut version = bytes.clone();
                    version[0] = 0;
                    let mut degree = bytes.clone();
                    degree[1..5].copy_from_slice(&u32::MAX.to_le_bytes());
                    let mut flag = bytes.clone();
                    flag[5] = 2;
                    let mut columns = bytes[..10].to_vec();
                    columns[6..10].copy_from_slice(&u32::MAX.to_le_bytes());
                    let mut point = bytes.clone();
                    point[10..42].fill(0xFF);
                    let mut trailing = bytes.clone();
                    trailing.push(0);
                    for malformed in [version, degree, flag, columns, point, trailing]
                        .iter()
                        .map(Vec::as_slice)
                        .chain([0, 1, 5, 9, 10, 41, bytes.len() - 1].map(|cut| &bytes[..cut]))
                    {
                        let result = crate::panic_hook::catch_unwind_suppressed(|| {
                            read_recursive_vk_checked::<$curve, SmallVkCircuit<$scalar>>(
                                malformed,
                                (),
                                6,
                                "test",
                            )
                        });
                        assert!(result.expect("malformed native VK must not panic").is_err());
                    }
                }
            }};
        }
        check_parity!(
            EqAffine,
            Fp,
            read_eq_recursive_vk,
            KagemushaTransportDeciderEqCircuitV1
        );
        check_parity!(
            EpAffine,
            Fq,
            read_ep_recursive_vk,
            KagemushaTransportDeciderEpCircuitV1
        );
    }

    #[test]
    fn checked_compact_mint_vk_readers_reject_small_keys_before_configuration() {
        // Real K6 headers must fail at all four production K16 entry points, before their
        // differing circuit configurations are consulted. This is not a mint proof test.
        let eq_params = ParamsIPA::<EqAffine>::new(6);
        let ep_params = ParamsIPA::<EpAffine>::new(6);
        let eq_bytes = keygen_vk_custom(&eq_params, &SmallVkCircuit::<Fp>::default(), false)
            .unwrap()
            .to_bytes(SerdeFormat::Processed);
        let ep_bytes = keygen_vk_custom(&ep_params, &SmallVkCircuit::<Fq>::default(), false)
            .unwrap()
            .to_bytes(SerdeFormat::Processed);
        for result in [
            read_eq_mint_authorization_vk(&eq_bytes, base_params()),
            read_eq_mint_vk(&eq_bytes, base_params()),
        ] {
            assert!(result.is_err());
        }
        for result in [
            read_ep_mint_authorization_vk(&ep_bytes, base_params()),
            read_ep_mint_vk(&ep_bytes, base_params()),
        ] {
            assert!(result.is_err());
        }
    }
}

fn validate_authenticated_mint_protocol_binding_v1(
    compiled: [[u8; 32]; 2],
    profile: [[u8; 32]; 2],
    release: [[u8; 32]; 2],
) -> Result<(), String> {
    if compiled != profile
        || compiled != release
        || compiled.contains(&[0; 32])
        || compiled[0] == compiled[1]
        || decode::<Fp>(compiled[0]).is_none()
        || decode::<Fq>(compiled[1]).is_none()
    {
        return Err("compiled finalized-mint protocol identity mismatch".to_owned());
    }
    Ok(())
}

fn validate_mint_checkpoint_release_v1(
    checkpoint: &KagemushaMintAuthorityCheckpointV1,
    release_id: [u8; 32],
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
    genesis_roster_id: [u8; 32],
) -> Result<(), String> {
    if checkpoint.release_id != release_id
        || checkpoint.statement.lifecycle.release_id != release_id
        || checkpoint.proof.eq_protocol_digest != eq_protocol_digest
        || checkpoint.proof.ep_protocol_digest != ep_protocol_digest
        || checkpoint.genesis_roster_id != genesis_roster_id
    {
        return Err("Kagemusha mint-authority checkpoint release mismatch".to_owned());
    }
    Ok(())
}

fn validate_mint_finality_release_v1(
    request: &super::KagemushaMintFinalityHelperVerificationRequestV1<'_>,
    release_id: [u8; 32],
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
    genesis_roster_id: [u8; 32],
    artifact_manifest_digest: [u8; 32],
) -> Result<(), String> {
    if request.statement.lifecycle.release_id != release_id
        || request.artifact_manifest_digest != artifact_manifest_digest
        || request.eq_protocol_digest != eq_protocol_digest
        || request.ep_protocol_digest != ep_protocol_digest
        || request.proof.eq_protocol_digest != eq_protocol_digest
        || request.proof.ep_protocol_digest != ep_protocol_digest
        || request.finality_genesis_roster_id != genesis_roster_id
    {
        return Err("Kagemusha mint-authority release binding mismatch".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod compact_mint_verifier_tests {
    use super::super::tests::{compact_mint_credit_fixture, compact_mint_request};
    use super::*;

    #[test]
    fn compact_mint_compiled_protocols_match_both_profile_and_release() {
        let canonical = [
            crate::zk::kagemusha_v1_poseidon::encode(Fp::from(31)),
            crate::zk::kagemusha_v1_poseidon::encode(Fq::from(32)),
        ];
        validate_authenticated_mint_protocol_binding_v1(canonical, canonical, canonical)
            .expect("matching canonical protocol metadata");
        for index in 0..2 {
            let mut changed = canonical;
            changed[index][0] ^= 4;
            assert!(
                validate_authenticated_mint_protocol_binding_v1(changed, canonical, canonical)
                    .is_err()
            );
            assert!(
                validate_authenticated_mint_protocol_binding_v1(canonical, changed, canonical)
                    .is_err()
            );
            assert!(
                validate_authenticated_mint_protocol_binding_v1(canonical, canonical, changed)
                    .is_err()
            );
            for invalid in [[0; 32], [0xFF; 32], canonical[1 - index]] {
                let mut changed = canonical;
                changed[index] = invalid;
                assert!(
                    validate_authenticated_mint_protocol_binding_v1(changed, changed, changed)
                        .is_err()
                );
            }
        }
    }

    #[test]
    fn compact_mint_checkpoint_preflight_rejects_every_release_component() {
        let credit = compact_mint_credit_fixture();
        let checkpoint = KagemushaMintAuthorityCheckpointV1 {
            step: KagemushaMintAuthorityStepV1::Bootstrap,
            release_id: credit.statement.lifecycle.release_id,
            statement: credit.statement,
            certificate_binding: credit.finality_certificate_binding,
            authority_head: credit.finality_authority_head,
            genesis_roster_id: credit.finality_genesis_roster_id,
            proof_binding_digest: credit.finality_proof_binding_digest,
            proof: credit.proof,
        };
        let check = |value: &KagemushaMintAuthorityCheckpointV1| {
            validate_mint_checkpoint_release_v1(
                value,
                checkpoint.release_id,
                checkpoint.proof.eq_protocol_digest,
                checkpoint.proof.ep_protocol_digest,
                checkpoint.genesis_roster_id,
            )
        };
        check(&checkpoint).expect("metadata preflight only");
        for index in 0..5 {
            let mut changed = checkpoint.clone();
            match index {
                0 => changed.release_id = [0xC1; 32],
                1 => changed.statement.lifecycle.release_id = [0xC2; 32],
                2 => changed.proof.eq_protocol_digest[0] ^= 1,
                3 => changed.proof.ep_protocol_digest[0] ^= 1,
                _ => changed.genesis_roster_id = [0xC3; 32],
            }
            assert!(
                check(&changed).is_err(),
                "checkpoint preflight case {index}"
            );
        }
    }

    #[test]
    fn compact_mint_finality_preflight_pins_manifest_release_and_both_protocol_sources() {
        let credit = compact_mint_credit_fixture();
        let check = |value: &super::super::KagemushaMintFinalityHelperVerificationRequestV1<'_>| {
            validate_mint_finality_release_v1(
                value,
                credit.statement.lifecycle.release_id,
                credit.proof.eq_protocol_digest,
                credit.proof.ep_protocol_digest,
                credit.finality_genesis_roster_id,
                credit.artifact_manifest_digest,
            )
        };
        check(&compact_mint_request(&credit)).expect("metadata preflight only");
        for index in 0..7 {
            let mut changed = credit.clone();
            match index {
                0 => changed.statement.lifecycle.release_id = [0xC1; 32],
                1 => changed.artifact_manifest_digest = [0xC2; 32],
                2 => changed.finality_genesis_roster_id = [0xC3; 32],
                3 => changed.proof.eq_protocol_digest[0] ^= 1,
                4 => changed.proof.ep_protocol_digest[0] ^= 1,
                _ => {}
            }
            let mut request = compact_mint_request(&changed);
            if index == 5 {
                request.eq_protocol_digest[0] ^= 1;
            }
            if index == 6 {
                request.ep_protocol_digest[0] ^= 1;
            }
            assert!(check(&request).is_err(), "finality preflight case {index}");
        }
    }

    #[test]
    fn compact_mint_native_projection_preserves_inner_commitment_and_outer_metadata() {
        let credit = compact_mint_credit_fixture();
        let request = compact_mint_request(&credit);
        let eq_history =
            KagemushaEqAccumulatorV1::try_from_bytes(&credit.proof.eq_history).unwrap();
        let ep_history =
            KagemushaEpAccumulatorV1::try_from_bytes(&credit.proof.ep_history).unwrap();
        let eq = mint_public_instances::<Fp>(&request, eq_history.as_bytes()).unwrap();
        let ep = mint_public_instances::<Fq>(&request, ep_history.as_bytes()).unwrap();
        assert_eq!(eq.len(), 56);
        assert_eq!(ep.len(), 56);
        for (offset, digest) in [
            (
                mint_public_instance::EQ_PROTOCOL_LO,
                credit.proof.eq_protocol_digest,
            ),
            (
                mint_public_instance::EP_PROTOCOL_LO,
                credit.proof.ep_protocol_digest,
            ),
            (
                mint_public_instance::EQ_AUDIT_LO,
                credit.proof.eq_deferred_audit,
            ),
            (
                mint_public_instance::EP_AUDIT_LO,
                credit.proof.ep_deferred_audit,
            ),
            (
                mint_public_instance::PAIR_BINDING_LO,
                credit.finality_proof_binding_digest,
            ),
        ] {
            assert_eq!(&eq[offset..offset + 2], &digest_limbs::<Fp>(digest));
            assert_eq!(&ep[offset..offset + 2], &digest_limbs::<Fq>(digest));
        }
        assert_eq!(
            &eq[22..],
            &history_public_instances::<Fp>(eq_history.as_bytes()).collect::<Vec<_>>()
        );
        assert_eq!(
            &ep[22..],
            &history_public_instances::<Fq>(ep_history.as_bytes()).collect::<Vec<_>>()
        );
        let mut changed = credit.clone();
        changed.finality_proof_binding_digest = [0xC4; 32];
        let projected =
            mint_public_instances::<Fp>(&compact_mint_request(&changed), eq_history.as_bytes())
                .unwrap();
        assert_ne!(&eq[20..22], &projected[20..22]);
        // Projection preserves the claimed inner audit; outer proof verification authenticates it.
        for index in 0..5 {
            let mut malformed = credit.clone();
            match index {
                0 => malformed.finality_proof_binding_digest = [0; 32],
                1 => malformed.proof.eq_history[0..32].fill(0xFF),
                2 => malformed.proof.ep_history[0..32].fill(0xFF),
                3 => malformed.finality_certificate_binding = [0xC5; 32],
                _ => malformed.finality_authority_head = [0xC6; 32],
            }
            assert!(
                mint_public_instances::<Fp>(
                    &compact_mint_request(&malformed),
                    eq_history.as_bytes()
                )
                .is_err()
            );
        }
    }
}

struct MintPublicPartsV1 {
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
}

fn history_public_instances<F: KagemushaPoseidonFieldV1>(
    history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> impl Iterator<Item = F> + '_ {
    history.chunks_exact(16).map(|chunk| {
        from_u128::<F>(u128::from_le_bytes(
            chunk
                .try_into()
                .expect("fixed history chunk has sixteen bytes"),
        ))
    })
}

pub(super) fn payment_terminal_public_inputs_v1(
    request: &KagemushaPaymentRequestV1,
    payment: &KagemushaPaymentV1,
    vk_digest: [u8; 32],
) -> Result<KagemushaTerminalAuthorizationPublicInputsV1, String> {
    payment
        .validate_shape_against(request)
        .map_err(|error| error.to_string())?;
    let proof = &payment.proof;
    let certificate = &payment.commit_certificate;
    let output = &payment.output;
    let request_digest = request
        .canonical_digest()
        .map_err(|error| error.to_string())?;
    let public = KagemushaTerminalAuthorizationPublicInputsV1 {
        operation: super::KagemushaOperationV1::SendSplit,
        protocol_version: request.version,
        suite_id: request.hardware_credential.suite_id,
        vk_digest,
        release_id: request.release_id,
        network_id: *request.network_id.as_bytes(),
        asset_id: kagemusha_asset_identity_digest_v1(&request.asset)
            .map_err(|error| error.to_string())?,
        asset_incarnation: request.asset_incarnation,
        asset_scale: request.scale,
        liability_pool_id: request.liability_pool_id,
        // This is the qualified SENDER profile authenticated by the final proof, not the
        // receiver's independent credential/profile carried by the request.
        hardware_profile_id: certificate.hardware_profile_id,
        policy_epoch: certificate.policy_epoch,
        lifecycle_binding_digest: certificate.lifecycle_binding_digest,
        semantic_digest: proof.semantic_digest,
        candidate_envelope_digest: proof.candidate_envelope_digest,
        commit_certificate_digest: proof.commit_certificate_digest,
        transition_nullifier: output.transition_nullifier,
        request_digest,
        receiver_binding_digest: request.hardware_credential.credential_id,
        ciphertext_commitment: output.ciphertext_commitment,
        amount: request.amount,
        terminal_output_binding: super::canonical_terminal_send_output_binding_v1(
            output.credit_id,
            request.recipient_encryption_key,
            request.hardware_credential.lane_commitment,
            iroha_data_model::kagemusha::kagemusha_prepared_transfer_digest_v1(
                request,
                output.sender_before_commitment,
                output.sender_after_commitment,
                output.transition_nullifier,
                output.ciphertext_commitment,
            )
            .map_err(|error| error.to_string())?,
            output
                .canonical_digest_against(request)
                .map_err(|error| error.to_string())?,
            super::kagemusha_incoming_proof_binding_digest_v1(request, payment)
                .map_err(|error| error.to_string())?,
        ),
        eq_deferred_audit: proof.eq_deferred_audit,
        ep_deferred_audit: proof.ep_deferred_audit,
        eq_protocol_digest: proof.eq_protocol_digest,
        ep_protocol_digest: proof.ep_protocol_digest,
    };
    public.validate()?;
    Ok(public)
}

fn terminal_authorization_public_instances<F: KagemushaPoseidonFieldV1>(
    request: &KagemushaParityVerificationRequestV1<'_>,
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
) -> Result<Vec<F>, String> {
    let terminal_authorization = KagemushaTerminalAuthorizationPublicInputsV1::from_lifecycle(
        &request.public_output.lifecycle,
        request.public_output.semantic_digest,
        request.public_output.candidate_envelope_digest,
        request.public_output.commit_certificate_digest,
        request.public_output.transition_nullifier,
        request.public_output.request_digest,
        request.public_output.receiver_binding_digest,
        request.public_output.ciphertext_commitment,
        request.public_output.amount,
        request.public_output.terminal_output_binding,
        request.eq_deferred_audit,
        request.ep_deferred_audit,
        eq_protocol_digest,
        ep_protocol_digest,
    )?;
    terminal_relation_public_instances(&terminal_authorization, request.history_accumulator)
}

fn terminal_relation_public_instances<F: KagemushaPoseidonFieldV1>(
    public_inputs: &KagemushaTerminalAuthorizationPublicInputsV1,
    history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<Vec<F>, String> {
    let mut public = public_inputs.public_prefix::<F>()?;
    public.extend(history_public_instances::<F>(history));
    if public.len() != TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1
        || terminal_authorization_public_instance::HISTORY_START != 47
    {
        return Err("Kagemusha terminal-relation public instance ABI mismatch".to_owned());
    }
    Ok(public)
}

fn mint_public_instances<F: KagemushaPoseidonFieldV1>(
    request: &super::KagemushaMintFinalityHelperVerificationRequestV1<'_>,
    history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<Vec<F>, String> {
    let canonical_semantic = request
        .statement
        .canonical_digest()
        .map_err(|error| error.to_string())?;
    request
        .proof
        .validate_shape_for_semantic_digest(canonical_semantic)
        .map_err(|error| error.to_string())?;
    if canonical_semantic != request.semantic_digest
        || request.proof.semantic_digest != request.semantic_digest
        || request.proof.eq_protocol_digest != request.eq_protocol_digest
        || request.proof.ep_protocol_digest != request.ep_protocol_digest
        || request.proof.guard_eq_credential_audit != request.finality_certificate_binding
        || request.proof.guard_ep_credential_audit != request.finality_authority_head
        || [
            request.finality_certificate_binding,
            request.finality_authority_head,
            request.finality_genesis_roster_id,
            request.finality_proof_binding_digest,
        ]
        .contains(&[0; 32])
    {
        return Err("Kagemusha mint-authority semantic binding mismatch".to_owned());
    }
    KagemushaEqAccumulatorV1::try_from_bytes(&request.proof.eq_history)
        .map_err(|error| error.to_string())?;
    KagemushaEpAccumulatorV1::try_from_bytes(&request.proof.ep_history)
        .map_err(|error| error.to_string())?;
    // Cells20..21 preserve the constrained inner Eq audit. It absorbs the inner Ep audit after
    // both inner parities bind the same semantic/protocol/history transcript. The final proof's
    // distinct outer audits authenticate this copied cell; the host does not relabel either one.
    mint_public_instances_from_parts::<F>(
        &MintPublicPartsV1 {
            step: KagemushaMintAuthorityStepV1::FinalizedMint,
            semantic_digest: request.semantic_digest,
            amount: request.statement.amount,
            certificate_binding: request.finality_certificate_binding,
            authority_head: request.finality_authority_head,
            release_id: request.statement.lifecycle.release_id,
            genesis_roster_id: request.finality_genesis_roster_id,
            eq_protocol_digest: request.eq_protocol_digest,
            ep_protocol_digest: request.ep_protocol_digest,
            eq_deferred_audit: request.proof.eq_deferred_audit,
            ep_deferred_audit: request.proof.ep_deferred_audit,
            proof_binding_digest: request.finality_proof_binding_digest,
        },
        history,
    )
}

fn mint_public_instances_from_parts<F: KagemushaPoseidonFieldV1>(
    parts: &MintPublicPartsV1,
    history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<Vec<F>, String> {
    let semantic = digest_limbs::<F>(parts.semantic_digest);
    let certificate = digest_limbs::<F>(parts.certificate_binding);
    let authority = digest_limbs::<F>(parts.authority_head);
    let release = digest_limbs::<F>(parts.release_id);
    let genesis = digest_limbs::<F>(parts.genesis_roster_id);
    let eq_protocol = digest_limbs::<F>(parts.eq_protocol_digest);
    let ep_protocol = digest_limbs::<F>(parts.ep_protocol_digest);
    let eq_audit = digest_limbs::<F>(parts.eq_deferred_audit);
    let ep_audit = digest_limbs::<F>(parts.ep_deferred_audit);
    let pair = digest_limbs::<F>(parts.proof_binding_digest);
    let mut public = vec![
        F::from(parts.step as u64),
        semantic[0],
        semantic[1],
        from_u128::<F>(parts.amount),
        certificate[0],
        certificate[1],
        authority[0],
        authority[1],
        release[0],
        release[1],
        genesis[0],
        genesis[1],
        eq_protocol[0],
        eq_protocol[1],
        ep_protocol[0],
        ep_protocol[1],
        eq_audit[0],
        eq_audit[1],
        ep_audit[0],
        ep_audit[1],
        pair[0],
        pair[1],
    ];
    public.extend(history.chunks_exact(16).map(|chunk| {
        from_u128::<F>(u128::from_le_bytes(
            chunk.try_into().expect("history chunk has sixteen bytes"),
        ))
    }));
    if public.len() != KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1
        || mint_public_instance::HISTORY_START != 22
    {
        return Err("Kagemusha mint-authority public instance ABI mismatch".to_owned());
    }
    Ok(public)
}

fn validate_proof_length(parity: &str, proof: &[u8]) -> Result<(), String> {
    if proof.is_empty() || proof.len() > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1 {
        return Err(format!(
            "Kagemusha {parity} proof length {} exceeds the fixed 1..={KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1} bound",
            proof.len()
        ));
    }
    Ok(())
}

/// Native succinct-verifier output plus the final authenticated transcript squeeze.
pub(super) struct KagemushaNativeVerifiedProofV1<C: CurveAffine> {
    /// Opening accumulator emitted by the succinct verifier.
    pub(super) accumulator: IpaAccumulator<C, NativeLoader>,
    /// Final squeeze after the protocol, instances, and every proof object were absorbed.
    pub(super) transcript_binding: C::ScalarExt,
}

pub(super) fn verify_eq_succinct_protocol(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EqAffine>,
    protocol: &snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
    proof: &[u8],
    instances: &[Fp],
) -> Result<IpaAccumulator<EqAffine, NativeLoader>, String> {
    verify_eq_succinct_protocol_with_transcript_binding(params, protocol, proof, instances)
        .map(|verified| verified.accumulator)
}

/// Verify one Eq proof and return its final transcript squeeze with its accumulator.
pub(super) fn verify_eq_succinct_protocol_with_transcript_binding(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EqAffine>,
    protocol: &snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
    proof: &[u8],
    instances: &[Fp],
) -> Result<KagemushaNativeVerifiedProofV1<EqAffine>, String> {
    type Scheme = IpaAs<EqAffine, Bgh19>;
    type Transcript<S> = PoseidonTranscript<
        EqAffine,
        NativeLoader,
        S,
        PASTA_IPA_POSEIDON_WIDTH_V1,
        PASTA_IPA_POSEIDON_RATE_V1,
        PASTA_IPA_POSEIDON_FULL_ROUNDS_V1,
        PASTA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    >;
    let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
    let svk = IpaSuccinctVerifyingKey::new(
        Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    );
    let instance_columns = vec![instances.to_vec()];
    let mut cursor = Cursor::new(proof);
    let mut transcript = Transcript::new::<PASTA_IPA_POSEIDON_SECURE_MDS_V1>(&mut cursor);
    let parsed = crate::panic_hook::catch_unwind_suppressed(|| {
        PlonkSuccinctVerifier::<Scheme>::read_proof(
            &svk,
            &protocol,
            &instance_columns,
            &mut transcript,
        )
    })
    .map_err(|_| "Kagemusha Eq proof parser panicked".to_owned())?
    .map_err(|error| format!("invalid Kagemusha Eq proof: {error:?}"))?;
    let accumulators = crate::panic_hook::catch_unwind_suppressed(|| {
        PlonkSuccinctVerifier::<Scheme>::verify(&svk, &protocol, &instance_columns, &parsed)
    })
    .map_err(|_| "Kagemusha Eq verifier panicked".to_owned())?
    .map_err(|error| format!("Kagemusha Eq proof rejected: {error:?}"))?;
    let transcript_binding = transcript.squeeze_challenge();
    drop(transcript);
    if cursor.position() != proof.len() as u64 {
        return Err("Kagemusha Eq proof has trailing bytes".to_owned());
    }
    let [accumulator] = accumulators.try_into().map_err(|values: Vec<_>| {
        format!("Kagemusha Eq proof returned {} accumulators", values.len())
    })?;
    Ok(KagemushaNativeVerifiedProofV1 {
        accumulator,
        transcript_binding,
    })
}

pub(super) fn verify_ep_succinct_protocol(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EpAffine>,
    protocol: &snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
    proof: &[u8],
    instances: &[Fq],
) -> Result<IpaAccumulator<EpAffine, NativeLoader>, String> {
    verify_ep_succinct_protocol_with_transcript_binding(params, protocol, proof, instances)
        .map(|verified| verified.accumulator)
}

/// Verify one Ep proof and return its final transcript squeeze with its accumulator.
pub(super) fn verify_ep_succinct_protocol_with_transcript_binding(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EpAffine>,
    protocol: &snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
    proof: &[u8],
    instances: &[Fq],
) -> Result<KagemushaNativeVerifiedProofV1<EpAffine>, String> {
    type Scheme = IpaAs<EpAffine, Bgh19>;
    type Transcript<S> = PoseidonTranscript<
        EpAffine,
        NativeLoader,
        S,
        PASTA_IPA_POSEIDON_WIDTH_V1,
        PASTA_IPA_POSEIDON_RATE_V1,
        PASTA_IPA_POSEIDON_FULL_ROUNDS_V1,
        PASTA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    >;
    let hash_to_curve = Ep::hash_to_curve("Halo2-Parameters");
    let svk = IpaSuccinctVerifyingKey::new(
        Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    );
    let instance_columns = vec![instances.to_vec()];
    let mut cursor = Cursor::new(proof);
    let mut transcript = Transcript::new::<PASTA_IPA_POSEIDON_SECURE_MDS_V1>(&mut cursor);
    let parsed = crate::panic_hook::catch_unwind_suppressed(|| {
        PlonkSuccinctVerifier::<Scheme>::read_proof(
            &svk,
            &protocol,
            &instance_columns,
            &mut transcript,
        )
    })
    .map_err(|_| "Kagemusha Ep proof parser panicked".to_owned())?
    .map_err(|error| format!("invalid Kagemusha Ep proof: {error:?}"))?;
    let accumulators = crate::panic_hook::catch_unwind_suppressed(|| {
        PlonkSuccinctVerifier::<Scheme>::verify(&svk, &protocol, &instance_columns, &parsed)
    })
    .map_err(|_| "Kagemusha Ep verifier panicked".to_owned())?
    .map_err(|error| format!("Kagemusha Ep proof rejected: {error:?}"))?;
    let transcript_binding = transcript.squeeze_challenge();
    drop(transcript);
    if cursor.position() != proof.len() as u64 {
        return Err("Kagemusha Ep proof has trailing bytes".to_owned());
    }
    let [accumulator] = accumulators.try_into().map_err(|values: Vec<_>| {
        format!("Kagemusha Ep proof returned {} accumulators", values.len())
    })?;
    Ok(KagemushaNativeVerifiedProofV1 {
        accumulator,
        transcript_binding,
    })
}

/// Verify one Eq proof whose second public column is carried by a proof-prefix commitment.
///
/// Unlike the recursive reader, native verification has the complete carrier and therefore
/// recomputes its exact default-blind Lagrange commitment before accepting the proof-read point.
pub(super) fn verify_eq_hybrid_succinct_protocol(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EqAffine>,
    protocol: &PlonkProtocol<EqAffine>,
    proof: &[u8],
    instances: &[Vec<Fp>],
) -> Result<IpaAccumulator<EqAffine, NativeLoader>, String> {
    let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
    let svk = IpaSuccinctVerifyingKey::new(
        Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    );
    verify_hybrid_succinct_protocol(
        params,
        &svk,
        protocol,
        proof,
        instances,
        [[
            MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1,
            MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1 + 1,
        ]],
        "Eq hybrid",
    )
    .map(|verified| verified.accumulator)
}

/// Verify one Ep proof whose second public column is carried by a proof-prefix commitment.
///
/// The proof still opens both queried instance polynomials; only the wide carrier's ICK MSM is
/// replaced by the authenticated prefix point.
pub(super) fn verify_ep_hybrid_succinct_protocol(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EpAffine>,
    protocol: &PlonkProtocol<EpAffine>,
    proof: &[u8],
    instances: &[Vec<Fq>],
) -> Result<IpaAccumulator<EpAffine, NativeLoader>, String> {
    let hash_to_curve = Ep::hash_to_curve("Halo2-Parameters");
    let svk = IpaSuccinctVerifyingKey::new(
        Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    );
    verify_hybrid_succinct_protocol(
        params,
        &svk,
        protocol,
        proof,
        instances,
        [[
            MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1 + 2,
            MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1 + 3,
        ]],
        "Ep hybrid",
    )
    .map(|verified| verified.accumulator)
}

/// Verify an Eq ordered mint-hash claim with its authenticated compact carrier.
pub(super) fn verify_eq_mint_hash_claim_hybrid_succinct_protocol(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EqAffine>,
    protocol: &PlonkProtocol<EqAffine>,
    proof: &[u8],
    instances: &[Vec<Fp>],
) -> Result<IpaAccumulator<EqAffine, NativeLoader>, String> {
    verify_eq_mint_hash_claim_hybrid_succinct_protocol_with_transcript_binding(
        params, protocol, proof, instances,
    )
    .map(|verified| verified.accumulator)
}

/// Verify an Eq ordered mint-hash claim and return its final transcript squeeze.
pub(super) fn verify_eq_mint_hash_claim_hybrid_succinct_protocol_with_transcript_binding(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EqAffine>,
    protocol: &PlonkProtocol<EqAffine>,
    proof: &[u8],
    instances: &[Vec<Fp>],
) -> Result<KagemushaNativeVerifiedProofV1<EqAffine>, String> {
    let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
    let svk = IpaSuccinctVerifyingKey::new(
        Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    );
    verify_hybrid_succinct_protocol(
        params,
        &svk,
        protocol,
        proof,
        instances,
        [
            [
                KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1,
                KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 + 1,
            ],
            [
                KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 + 2,
                KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 + 3,
            ],
        ],
        "Eq mint-hash claim hybrid",
    )
}

/// Verify an Ep ordered mint-hash claim with its authenticated compact carrier.
pub(super) fn verify_ep_mint_hash_claim_hybrid_succinct_protocol(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EpAffine>,
    protocol: &PlonkProtocol<EpAffine>,
    proof: &[u8],
    instances: &[Vec<Fq>],
) -> Result<IpaAccumulator<EpAffine, NativeLoader>, String> {
    verify_ep_mint_hash_claim_hybrid_succinct_protocol_with_transcript_binding(
        params, protocol, proof, instances,
    )
    .map(|verified| verified.accumulator)
}

/// Verify an Ep ordered mint-hash claim and return its final transcript squeeze.
pub(super) fn verify_ep_mint_hash_claim_hybrid_succinct_protocol_with_transcript_binding(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EpAffine>,
    protocol: &PlonkProtocol<EpAffine>,
    proof: &[u8],
    instances: &[Vec<Fq>],
) -> Result<KagemushaNativeVerifiedProofV1<EpAffine>, String> {
    let hash_to_curve = Ep::hash_to_curve("Halo2-Parameters");
    let svk = IpaSuccinctVerifyingKey::new(
        Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    );
    verify_hybrid_succinct_protocol(
        params,
        &svk,
        protocol,
        proof,
        instances,
        [
            [
                KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 + 4,
                KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 + 5,
            ],
            [
                KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 + 6,
                KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 + 7,
            ],
        ],
        "Ep mint-hash claim hybrid",
    )
}

fn verify_hybrid_succinct_protocol<C, const N: usize>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<C>,
    svk: &IpaSuccinctVerifyingKey<C>,
    protocol: &PlonkProtocol<C>,
    proof: &[u8],
    instances: &[Vec<C::ScalarExt>],
    carrier_commitment_limb_indices: [[usize; 2]; N],
    parity: &str,
) -> Result<KagemushaNativeVerifiedProofV1<C>, String>
where
    C: CurveAffine,
    C::ScalarExt: ff::FromUniformBytes<64> + ff::WithSmallOrderMulGroup<3>,
{
    validate_hybrid_protocol_shape(
        params,
        protocol,
        instances,
        &carrier_commitment_limb_indices,
        parity,
    )?;
    // The ordinary recursive profile already includes the appended folded-generator point;
    // hybrid framing adds one proof-supplied point per wide instance column before advice.
    let expected_proof_len = ordinary_ipa_proof_profile_v1(protocol)
        .map_err(|error| format!("Kagemusha {parity} proof profile is invalid: {error}"))?
        .byte_len
        .checked_add(hybrid_proof_supplied_commitment_bytes::<C>(N, parity)?)
        .ok_or_else(|| format!("Kagemusha {parity} proof length overflowed"))?;
    if proof.len() != expected_proof_len {
        return Err(format!(
            "Kagemusha {parity} proof has length {}, expected exactly {expected_proof_len}",
            proof.len()
        ));
    }
    let expected_carrier_commitments = instances[1..]
        .iter()
        .map(|carrier| canonical_hybrid_carrier_commitment(params, carrier, parity))
        .collect::<Result<Vec<_>, _>>()?;
    for (&carrier_commitment, &limb_indices) in expected_carrier_commitments
        .iter()
        .zip(&carrier_commitment_limb_indices)
    {
        validate_hybrid_carrier_semantic_binding(
            &instances[0],
            carrier_commitment,
            limb_indices,
            parity,
        )?;
    }
    let instance_committing_key = protocol
        .instance_committing_key
        .as_ref()
        .expect("validated hybrid protocol has a semantic ICK");

    let mut cursor = Cursor::new(proof);
    let mut transcript = PoseidonTranscript::<
        C,
        NativeLoader,
        _,
        PASTA_IPA_POSEIDON_WIDTH_V1,
        PASTA_IPA_POSEIDON_RATE_V1,
        PASTA_IPA_POSEIDON_FULL_ROUNDS_V1,
        PASTA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    >::new::<PASTA_IPA_POSEIDON_SECURE_MDS_V1>(&mut cursor);
    let parsed = crate::panic_hook::catch_unwind_suppressed(|| {
        if let Some(transcript_initial_state) = protocol.transcript_initial_state.as_ref() {
            transcript.common_scalar(transcript_initial_state)?;
        }
        let semantic_commitment = instances[0]
            .iter()
            .zip(&instance_committing_key.bases)
            .map(|(scalar, base)| Msm::<C, NativeLoader>::base(base) * scalar)
            .chain(std::iter::once(Msm::base(
                instance_committing_key
                    .constant
                    .as_ref()
                    .expect("validated hybrid ICK has its default-blind constant"),
            )))
            .sum::<Msm<'_, C, NativeLoader>>()
            .evaluate(None);
        transcript.common_ec_point(&semantic_commitment)?;
        let carrier_commitments = transcript.read_n_ec_points(N)?;
        if carrier_commitments != expected_carrier_commitments {
            return Err(snark_verifier::Error::InvalidInstances);
        }

        let mut witnesses = Vec::new();
        let mut challenges = Vec::new();
        for (&witness_count, &challenge_count) in protocol
            .num_witness
            .iter()
            .zip(protocol.num_challenge.iter())
        {
            witnesses.extend(transcript.read_n_ec_points(witness_count)?);
            challenges.extend(transcript.squeeze_n_challenges(challenge_count));
        }
        let quotients = transcript.read_n_ec_points(protocol.quotient.num_chunk())?;
        let z = transcript.squeeze_challenge();
        let evaluations = transcript.read_n_scalars(protocol.evaluations.len())?;
        let pcs = <IpaAs<C, Bgh19> as PolynomialCommitmentScheme<C, NativeLoader>>::read_proof(
            svk,
            &PlonkProof::<C, NativeLoader, IpaAs<C, Bgh19>>::empty_queries(protocol),
            &mut transcript,
        )?;
        let mut committed_instances = Vec::with_capacity(N + 1);
        committed_instances.push(semantic_commitment);
        committed_instances.extend(carrier_commitments);
        Ok::<_, snark_verifier::Error>(PlonkProof::<C, NativeLoader, IpaAs<C, Bgh19>> {
            committed_instances: Some(committed_instances),
            witnesses,
            challenges,
            quotients,
            z,
            evaluations,
            pcs,
            old_accumulators: Vec::new(),
        })
    })
    .map_err(|_| format!("Kagemusha {parity} proof parser panicked"))?
    .map_err(|error| format!("invalid Kagemusha {parity} proof: {error:?}"))?;

    // With an ICK-bearing protocol, the verifier consumes proof-read evaluations for all
    // instance polynomials. Empty carrier placeholders prevent accidental wide MSMs while
    // the authenticated commitments above continue to participate in every quotient/PCS query.
    let mut verification_instances = Vec::with_capacity(N + 1);
    verification_instances.push(instances[0].clone());
    verification_instances.extend((0..N).map(|_| Vec::new()));
    let accumulators = crate::panic_hook::catch_unwind_suppressed(|| {
        PlonkSuccinctVerifier::<IpaAs<C, Bgh19>>::verify(
            svk,
            protocol,
            &verification_instances,
            &parsed,
        )
    })
    .map_err(|_| format!("Kagemusha {parity} verifier panicked"))?
    .map_err(|error| format!("Kagemusha {parity} proof rejected: {error:?}"))?;
    let transcript_binding = transcript.squeeze_challenge();
    drop(transcript);
    if cursor.position() != proof.len() as u64 {
        return Err(format!("Kagemusha {parity} proof has trailing bytes"));
    }
    let [accumulator] = accumulators.try_into().map_err(|values: Vec<_>| {
        format!(
            "Kagemusha {parity} proof returned {} accumulators",
            values.len()
        )
    })?;
    Ok(KagemushaNativeVerifiedProofV1 {
        accumulator,
        transcript_binding,
    })
}

fn hybrid_proof_supplied_commitment_bytes<C>(
    proof_supplied_commitment_count: usize,
    parity: &str,
) -> Result<usize, String>
where
    C: CurveAffine,
{
    let encoded_identity = C::identity().to_bytes();
    let encoded_point_len = encoded_identity.as_ref().len();
    if encoded_point_len != 32 {
        return Err(format!(
            "Kagemusha {parity} hybrid commitment does not use the canonical 32-byte Pasta encoding"
        ));
    }
    proof_supplied_commitment_count
        .checked_mul(encoded_point_len)
        .ok_or_else(|| format!("Kagemusha {parity} hybrid commitment byte length overflowed"))
}

fn validate_hybrid_commitment_limb_indices(
    semantic_instance_count: usize,
    carrier_commitment_limb_indices: &[[usize; 2]],
    parity: &str,
) -> Result<(), String> {
    if !(1..=2).contains(&carrier_commitment_limb_indices.len()) {
        return Err(format!(
            "Kagemusha {parity} hybrid proof must bind one or two carrier commitments"
        ));
    }
    let mut expected_next = None;
    for indices in carrier_commitment_limb_indices {
        if indices[0].checked_add(1) != Some(indices[1])
            || indices[1] >= semantic_instance_count
            || expected_next.is_some_and(|expected| indices[0] != expected)
        {
            return Err(format!(
                "Kagemusha {parity} hybrid commitment limbs are not consecutive and canonical"
            ));
        }
        expected_next = indices[1].checked_add(1);
    }
    Ok(())
}

fn validate_hybrid_protocol_shape<C>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<C>,
    protocol: &PlonkProtocol<C>,
    instances: &[Vec<C::ScalarExt>],
    carrier_commitment_limb_indices: &[[usize; 2]],
    parity: &str,
) -> Result<(), String>
where
    C: CurveAffine,
{
    let carrier_count = carrier_commitment_limb_indices.len();
    let instance_lengths = instances.iter().map(Vec::len).collect::<Vec<_>>();
    if params.k() as usize != protocol.domain.k
        || !(1..=2).contains(&carrier_count)
        || protocol.num_instance.len() != carrier_count + 1
        || protocol.num_instance.as_slice() != instance_lengths.as_slice()
        || protocol.num_instance[0] == 0
        || protocol
            .num_instance
            .iter()
            .skip(1)
            .any(|count| *count == 0 || *count <= protocol.num_instance[0])
        || !protocol.accumulator_indices.is_empty()
    {
        return Err(format!(
            "Kagemusha {parity} proof does not have one semantic and {carrier_count} wide carrier column(s)"
        ));
    }
    validate_hybrid_commitment_limb_indices(
        protocol.num_instance[0],
        carrier_commitment_limb_indices,
        parity,
    )?;
    let instance_committing_key = protocol
        .instance_committing_key
        .as_ref()
        .ok_or_else(|| format!("Kagemusha {parity} hybrid protocol has no semantic ICK"))?;
    if instance_committing_key.bases.len() != protocol.num_instance[0]
        || instance_committing_key.constant.is_none()
        || protocol
            .num_instance
            .iter()
            .skip(1)
            .any(|count| *count > params.get_g_lagrange().len())
    {
        return Err(format!(
            "Kagemusha {parity} hybrid instance-commitment parameters have the wrong shape"
        ));
    }
    let instance_offset = protocol.preprocessed.len();
    let instance_end = instance_offset
        .checked_add(carrier_count + 1)
        .ok_or_else(|| format!("Kagemusha {parity} hybrid instance index overflowed"))?;
    for polynomial in instance_offset..instance_end {
        if !protocol
            .evaluations
            .iter()
            .any(|query| query.poly == polynomial)
            || !protocol
                .queries
                .iter()
                .any(|query| query.poly == polynomial)
        {
            return Err(format!(
                "Kagemusha {parity} hybrid instance column is absent from the quotient or IPA opening set"
            ));
        }
    }
    Ok(())
}

fn validate_hybrid_carrier_semantic_binding<C>(
    semantic: &[C::ScalarExt],
    carrier_commitment: C,
    carrier_commitment_limb_indices: [usize; 2],
    parity: &str,
) -> Result<(), String>
where
    C: CurveAffine,
{
    let encoding = carrier_commitment.to_bytes();
    let bytes = encoding.as_ref();
    if bytes.len() != 32 {
        return Err(format!(
            "Kagemusha {parity} carrier commitment does not have a Pasta encoding"
        ));
    }
    let expected: [C::ScalarExt; 2] = std::array::from_fn(|half| {
        from_u128::<C::ScalarExt>(u128::from_le_bytes(
            bytes[half * 16..(half + 1) * 16]
                .try_into()
                .expect("Pasta compressed point half has sixteen bytes"),
        ))
    });
    if carrier_commitment_limb_indices
        .into_iter()
        .zip(expected)
        .any(|(index, expected)| semantic[index] != expected)
    {
        return Err(format!(
            "Kagemusha {parity} semantic column does not bind its carrier commitment"
        ));
    }
    Ok(())
}

fn canonical_hybrid_carrier_commitment<C>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<C>,
    carrier: &[C::ScalarExt],
    parity: &str,
) -> Result<C, String>
where
    C: CurveAffine,
{
    let bases = params
        .get_g_lagrange()
        .get(..carrier.len())
        .ok_or_else(|| format!("Kagemusha {parity} carrier exceeds the IPA domain"))?;
    let default_blind = Blind::<C::ScalarExt>::default().0;
    Ok(
        (best_multiexp::<C>(carrier, bases) + params.get_blind_base().to_curve() * default_blind)
            .to_affine(),
    )
}
