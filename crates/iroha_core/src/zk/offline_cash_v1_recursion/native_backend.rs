//! Authenticated terminal verifier for the complete Offline Cash V1 recursive state relation.

use std::io::Cursor;

use halo2_base::gates::circuit::BaseCircuitParams;
use halo2_proofs::{
    SerdeFormat,
    halo2curves::{
        CurveExt as _,
        group::Curve as _,
        pasta::{Ep, EpAffine, Eq, EqAffine, Fp, Fq},
    },
    plonk::VerifyingKey,
    poly::commitment::{Params as _, ParamsProver as _},
};
use sha2::{Digest as _, Sha256};
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::ipa::{Bgh19, IpaAccumulator, IpaAs, IpaSuccinctVerifyingKey},
    system::halo2::{compile, transcript::halo2::PoseidonTranscript},
    util::arithmetic::{Domain, root_of_unity},
    verifier::{SnarkVerifier as _, plonk::PlonkSuccinctVerifier},
};

use super::{
    OFFLINE_CASH_IPA_POSEIDON_FULL_ROUNDS_V1 as PASTA_IPA_POSEIDON_FULL_ROUNDS_V1,
    OFFLINE_CASH_IPA_POSEIDON_PARTIAL_ROUNDS_V1 as PASTA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    OFFLINE_CASH_IPA_POSEIDON_RATE_V1 as PASTA_IPA_POSEIDON_RATE_V1,
    OFFLINE_CASH_IPA_POSEIDON_SECURE_MDS_V1 as PASTA_IPA_POSEIDON_SECURE_MDS_V1,
    OFFLINE_CASH_IPA_POSEIDON_WIDTH_V1 as PASTA_IPA_POSEIDON_WIDTH_V1,
    OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1, OfflineCashArtifactByteResolverV1,
    OfflineCashArtifactErrorV1, OfflineCashAuthenticatedArtifactSetV1,
    OfflineCashCommitWrapperPublicInputsV1, OfflineCashEpAccumulatorV1, OfflineCashEqAccumulatorV1,
    OfflineCashNoCommitClosureDecisionV1, OfflineCashNoCommitClosureVerifierV1,
    OfflineCashParityVerificationRequestV1, OfflineCashPastaParityV1,
    OfflineCashRecursionArtifactsV1, OfflineCashRecursiveVerifierV1,
    OfflineCashStateProofVerificationRequestV1,
    commit_wrapper::{
        COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1, OfflineCashCommitWrapperEpCircuitV1,
        OfflineCashCommitWrapperEqCircuitV1, public_instance as wrapper_public_instance,
    },
    composite::{OfflineCashRecursiveStateEpCircuitV1, OfflineCashRecursiveStateEqCircuitV1},
    decide_offline_cash_ep_accumulator_v1, decide_offline_cash_eq_accumulator_v1,
    deferred_parent::{accumulator_limb_count, native_parent_protocol_digest_v1},
    guard_bundle::{
        GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1, OfflineCashGuardBundleEpCircuitV1,
        OfflineCashGuardBundleEqCircuitV1,
    },
    mint_authority::{
        OFFLINE_CASH_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1, OfflineCashMintAuthorityCheckpointV1,
        OfflineCashMintAuthorityEpCircuitV1, OfflineCashMintAuthorityEqCircuitV1,
        OfflineCashMintAuthorityPairBindingV1, public_instance as mint_public_instance,
    },
    mint_authorization::{
        MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1, OfflineCashMintAuthorizationEpCircuitV1,
        OfflineCashMintAuthorizationEqCircuitV1, mint_authorization_public_instances_v1,
    },
    mint_helper::OfflineCashMintAuthorityStepV1,
    state_relation::PUBLIC_INSTANCE_COUNT,
    verify_offline_cash_recursive_proof_v1, verify_offline_cash_state_proof_v1,
};
use crate::zk::offline_cash_v1_poseidon::{
    OfflineCashPoseidonFieldV1, decode, digest_limbs, from_u128,
};
use crate::zk::offline_cash_v1_state::{
    OfflineCashAcceptanceIntentAuthorizationDecisionV1,
    OfflineCashAcceptanceIntentAuthorizationVerifierV1, OfflineCashCandidateProofVerifierV1,
    OfflineCashCommitWrapperPublicInputsV1 as OfflineCashTerminalPublicInputsV1,
    OfflineCashCommitWrapperVerifierV1, PreparedOutgoingCandidateV1,
};
use iroha_data_model::offline::{
    OFFLINE_CASH_HALO2_K_V1, OfflineCashAcceptanceIntentAuthorizationV1,
    OfflineCashArtifactBindingV1, OfflineCashArtifactRoleV1, OfflineCashAuthenticatedReleaseV1,
    OfflineCashMintAuthorizationV1, OfflineCashNoCommitClosureV1, OfflineCashPaymentRequestV1,
};

const RECURSIVE_PROFILE_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:paired-recursive-circuit-profile";
const RECURSIVE_PUBLIC_INSTANCE_COUNT_V1: usize = PUBLIC_INSTANCE_COUNT + accumulator_limb_count();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OfflineCashCommitWrapperAuthorityBindingV1 {
    release_id: [u8; 32],
    suite_id: [u8; 32],
    vk_set_digest: [u8; 32],
    artifact_manifest_digest: [u8; 32],
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
}

fn validate_commit_wrapper_authority_binding_v1(
    label: &str,
    actual: OfflineCashCommitWrapperAuthorityBindingV1,
    expected: OfflineCashCommitWrapperAuthorityBindingV1,
    eq_role: OfflineCashArtifactRoleV1,
    ep_role: OfflineCashArtifactRoleV1,
) -> Result<(), String> {
    if actual != expected
        || eq_role != OfflineCashArtifactRoleV1::CommitWrapperVkEq
        || ep_role != OfflineCashArtifactRoleV1::CommitWrapperVkEp
    {
        return Err(format!("Offline Cash {label} release/key-role mismatch"));
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
        return Err("Offline Cash GuardBundle compiled protocol release mismatch".to_owned());
    }
    Ok(())
}

/// Exact `halo2-base` layouts used to decode all ten authenticated recursive verifying keys.
///
/// These values are covered by [`Self::canonical_digest`], which must equal the release
/// manifest's authenticated profile digest. They are explicit because processed Halo2 keys do
/// not safely self-describe a circuit configuration.
#[derive(Clone, Debug)]
pub struct OfflineCashRecursiveVerifierProfileV1 {
    /// Eq aggregate-state recursive circuit layout.
    pub state_eq: BaseCircuitParams,
    /// Ep aggregate-state recursive circuit layout.
    pub state_ep: BaseCircuitParams,
    /// Eq GuardBundle recursive circuit layout.
    pub guard_eq: BaseCircuitParams,
    /// Ep GuardBundle recursive circuit layout.
    pub guard_ep: BaseCircuitParams,
    /// Eq terminal commit-wrapper recursive circuit layout.
    pub wrapper_eq: BaseCircuitParams,
    /// Ep terminal commit-wrapper recursive circuit layout.
    pub wrapper_ep: BaseCircuitParams,
    /// Eq mint-authorization recursive circuit layout.
    pub mint_authorization_eq: BaseCircuitParams,
    /// Ep mint-authorization recursive circuit layout.
    pub mint_authorization_ep: BaseCircuitParams,
    /// Eq stable mint-authority recursive circuit layout.
    pub mint_eq: BaseCircuitParams,
    /// Ep stable mint-authority recursive circuit layout.
    pub mint_ep: BaseCircuitParams,
    /// Actual compiled Eq finalized-mint protocol identity constrained by the state circuit.
    pub mint_eq_protocol_digest: [u8; 32],
    /// Actual compiled Ep finalized-mint protocol identity constrained by the state circuit.
    pub mint_ep_protocol_digest: [u8; 32],
    /// Release-pinned genesis mint-finality roster identifier.
    pub mint_genesis_roster_id: [u8; 32],
}

impl OfflineCashRecursiveVerifierProfileV1 {
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
            (1_u8, &self.state_eq),
            (2, &self.state_ep),
            (3, &self.guard_eq),
            (4, &self.guard_ep),
            (5, &self.wrapper_eq),
            (6, &self.wrapper_ep),
            (7, &self.mint_authorization_eq),
            (8, &self.mint_authorization_ep),
            (9, &self.mint_eq),
            (10, &self.mint_ep),
        ] {
            bytes.push(tag);
            append_base_params(&mut bytes, params)?;
        }
        bytes.push(11);
        bytes.extend_from_slice(&self.mint_eq_protocol_digest);
        bytes.push(12);
        bytes.extend_from_slice(&self.mint_ep_protocol_digest);
        bytes.push(13);
        bytes.extend_from_slice(&self.mint_genesis_roster_id);
        Ok(Sha256::digest(bytes).into())
    }

    fn validate(&self) -> Result<(), String> {
        for (label, params) in [
            ("state Eq", &self.state_eq),
            ("state Ep", &self.state_ep),
            ("GuardBundle Eq", &self.guard_eq),
            ("GuardBundle Ep", &self.guard_ep),
            ("commit wrapper Eq", &self.wrapper_eq),
            ("commit wrapper Ep", &self.wrapper_ep),
            ("mint authorization Eq", &self.mint_authorization_eq),
            ("mint authorization Ep", &self.mint_authorization_ep),
            ("mint authority Eq", &self.mint_eq),
            ("mint authority Ep", &self.mint_ep),
        ] {
            if params.k != OFFLINE_CASH_HALO2_K_V1 as usize
                || params.num_instance_columns != 1
                || params.lookup_bits != Some((OFFLINE_CASH_HALO2_K_V1 - 1) as usize)
                || params.num_advice_per_phase.is_empty()
                || params.num_lookup_advice_per_phase.is_empty()
                || params.num_advice_per_phase.iter().all(|value| *value == 0)
            {
                return Err(format!("invalid Offline Cash {label} circuit profile"));
            }
        }
        if self.mint_eq_protocol_digest == [0; 32]
            || self.mint_ep_protocol_digest == [0; 32]
            || self.mint_eq_protocol_digest == self.mint_ep_protocol_digest
            || decode::<Fp>(self.mint_eq_protocol_digest).is_none()
            || decode::<Fq>(self.mint_ep_protocol_digest).is_none()
            || self.mint_genesis_roster_id == [0; 32]
        {
            return Err("invalid Offline Cash finalized-mint protocol identities".to_owned());
        }
        Ok(())
    }
}

/// Release-authenticated accepting verifier for the paired recursive state relation.
///
/// Construction authenticates every key byte, validates the circuit profile, recompiles all
/// protocols, and checks the state protocol identities recorded by the release. Verification
/// then supplies the exact Guard protocol identities derived from the authenticated Guard keys.
pub struct OfflineCashAuthenticatedRecursiveVerifierV1 {
    artifacts: OfflineCashRecursionArtifactsV1,
    eq_parameters: halo2_proofs::poly::ipa::commitment::ParamsIPA<EqAffine>,
    ep_parameters: halo2_proofs::poly::ipa::commitment::ParamsIPA<EpAffine>,
    eq_state_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
    ep_state_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
    eq_wrapper_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
    ep_wrapper_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
    eq_mint_authorization_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
    ep_mint_authorization_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
    eq_mint_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
    ep_mint_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
    guard_eq_protocol_digest: [u8; 32],
    guard_ep_protocol_digest: [u8; 32],
    wrapper_eq_protocol_digest: [u8; 32],
    wrapper_ep_protocol_digest: [u8; 32],
    mint_authorization_eq_protocol_digest: [u8; 32],
    mint_authorization_ep_protocol_digest: [u8; 32],
    mint_eq_protocol_digest: [u8; 32],
    mint_ep_protocol_digest: [u8; 32],
    mint_genesis_roster_id: [u8; 32],
    release_id: [u8; 32],
    suite_id: [u8; 32],
    vk_set_digest: [u8; 32],
    artifact_manifest_digest: [u8; 32],
    wrapper_eq_binding: OfflineCashArtifactBindingV1,
    wrapper_ep_binding: OfflineCashArtifactBindingV1,
}

impl OfflineCashAuthenticatedRecursiveVerifierV1 {
    fn commit_wrapper_authority_binding(&self) -> OfflineCashCommitWrapperAuthorityBindingV1 {
        OfflineCashCommitWrapperAuthorityBindingV1 {
            release_id: self.release_id,
            suite_id: self.suite_id,
            vk_set_digest: self.vk_set_digest,
            artifact_manifest_digest: self.artifact_manifest_digest,
            eq_protocol_digest: self.wrapper_eq_protocol_digest,
            ep_protocol_digest: self.wrapper_ep_protocol_digest,
        }
    }

    /// Resolve and reauthenticate the exact state/Guard verifying keys for one release.
    ///
    /// # Errors
    ///
    /// Fails closed for missing or substituted bytes, profile mismatch, malformed processed
    /// keys, trailing bytes, or any compiled-protocol identity mismatch.
    pub fn load<R>(
        artifacts: &OfflineCashAuthenticatedArtifactSetV1<R>,
        profile: OfflineCashRecursiveVerifierProfileV1,
    ) -> Result<Self, OfflineCashArtifactErrorV1>
    where
        R: OfflineCashArtifactByteResolverV1,
    {
        let recursion = artifacts.recursion_artifacts();
        let profile_digest = profile
            .canonical_digest()
            .map_err(OfflineCashArtifactErrorV1::InvalidRelease)?;
        if profile_digest != recursion.profile_digest {
            return Err(OfflineCashArtifactErrorV1::InvalidRelease(
                "recursive circuit profile digest mismatch".to_owned(),
            ));
        }
        let eq_parameters = artifacts.load_eq_params()?;
        let ep_parameters = artifacts.load_ep_params()?;
        let eq_state_vk = read_eq_state_vk(
            artifacts
                .resolve(OfflineCashArtifactRoleV1::StateVkEq)?
                .as_ref(),
            profile.state_eq,
        )?;
        let ep_state_vk = read_ep_state_vk(
            artifacts
                .resolve(OfflineCashArtifactRoleV1::StateVkEp)?
                .as_ref(),
            profile.state_ep,
        )?;
        let eq_guard_vk = read_eq_guard_vk(
            artifacts
                .resolve(OfflineCashArtifactRoleV1::GuardBundleVkEq)?
                .as_ref(),
            profile.guard_eq,
        )?;
        let ep_guard_vk = read_ep_guard_vk(
            artifacts
                .resolve(OfflineCashArtifactRoleV1::GuardBundleVkEp)?
                .as_ref(),
            profile.guard_ep,
        )?;
        let eq_wrapper_vk = read_eq_wrapper_vk(
            artifacts
                .resolve(OfflineCashArtifactRoleV1::CommitWrapperVkEq)?
                .as_ref(),
            profile.wrapper_eq,
        )?;
        let ep_wrapper_vk = read_ep_wrapper_vk(
            artifacts
                .resolve(OfflineCashArtifactRoleV1::CommitWrapperVkEp)?
                .as_ref(),
            profile.wrapper_ep,
        )?;
        let eq_mint_authorization_vk = read_eq_mint_authorization_vk(
            artifacts
                .resolve(OfflineCashArtifactRoleV1::MintAuthorizationVkEq)?
                .as_ref(),
            profile.mint_authorization_eq,
        )?;
        let ep_mint_authorization_vk = read_ep_mint_authorization_vk(
            artifacts
                .resolve(OfflineCashArtifactRoleV1::MintAuthorizationVkEp)?
                .as_ref(),
            profile.mint_authorization_ep,
        )?;
        let eq_mint_vk = read_eq_mint_vk(
            artifacts
                .resolve(OfflineCashArtifactRoleV1::MintCreditVkEq)?
                .as_ref(),
            profile.mint_eq,
        )?;
        let ep_mint_vk = read_ep_mint_vk(
            artifacts
                .resolve(OfflineCashArtifactRoleV1::MintCreditVkEp)?
                .as_ref(),
            profile.mint_ep,
        )?;
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
            native_parent_protocol_digest_v1(&eq_state_protocol, OfflineCashPastaParityV1::Eq)
                .map_err(OfflineCashArtifactErrorV1::InvalidRelease)?;
        let ep_protocol_digest =
            native_parent_protocol_digest_v1(&ep_state_protocol, OfflineCashPastaParityV1::Ep)
                .map_err(OfflineCashArtifactErrorV1::InvalidRelease)?;
        if eq_protocol_digest != recursion.eq_protocol_digest
            || ep_protocol_digest != recursion.ep_protocol_digest
        {
            return Err(OfflineCashArtifactErrorV1::InvalidRelease(
                "compiled state protocol identity mismatch".to_owned(),
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
            native_parent_protocol_digest_v1(&eq_guard_protocol, OfflineCashPastaParityV1::Eq)
                .map_err(OfflineCashArtifactErrorV1::InvalidRelease)?;
        let guard_ep_protocol_digest =
            native_parent_protocol_digest_v1(&ep_guard_protocol, OfflineCashPastaParityV1::Ep)
                .map_err(OfflineCashArtifactErrorV1::InvalidRelease)?;
        let expected_guard_eq_protocol_digest = recursion
            .guard_bundle_protocol_digest(OfflineCashPastaParityV1::Eq)
            .map_err(|error| OfflineCashArtifactErrorV1::InvalidRelease(error.to_string()))?;
        let expected_guard_ep_protocol_digest = recursion
            .guard_bundle_protocol_digest(OfflineCashPastaParityV1::Ep)
            .map_err(|error| OfflineCashArtifactErrorV1::InvalidRelease(error.to_string()))?;
        validate_authenticated_guard_protocol_binding_v1(
            guard_eq_protocol_digest,
            guard_ep_protocol_digest,
            expected_guard_eq_protocol_digest,
            expected_guard_ep_protocol_digest,
        )
        .map_err(OfflineCashArtifactErrorV1::InvalidRelease)?;
        if guard_eq_protocol_digest == guard_ep_protocol_digest
            || guard_eq_protocol_digest == eq_protocol_digest
            || guard_ep_protocol_digest == ep_protocol_digest
        {
            return Err(OfflineCashArtifactErrorV1::InvalidRelease(
                "compiled recursive protocol roles alias".to_owned(),
            ));
        }
        let eq_wrapper_protocol = compile(
            &eq_parameters,
            &eq_wrapper_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let ep_wrapper_protocol = compile(
            &ep_parameters,
            &ep_wrapper_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let wrapper_eq_protocol_digest =
            native_parent_protocol_digest_v1(&eq_wrapper_protocol, OfflineCashPastaParityV1::Eq)
                .map_err(OfflineCashArtifactErrorV1::InvalidRelease)?;
        let wrapper_ep_protocol_digest =
            native_parent_protocol_digest_v1(&ep_wrapper_protocol, OfflineCashPastaParityV1::Ep)
                .map_err(OfflineCashArtifactErrorV1::InvalidRelease)?;
        if wrapper_eq_protocol_digest != recursion.commit_wrapper_eq_protocol_digest
            || wrapper_ep_protocol_digest != recursion.commit_wrapper_ep_protocol_digest
        {
            return Err(OfflineCashArtifactErrorV1::InvalidRelease(
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
            OfflineCashPastaParityV1::Eq,
        )
        .map_err(OfflineCashArtifactErrorV1::InvalidRelease)?;
        let mint_authorization_ep_protocol_digest = native_parent_protocol_digest_v1(
            &ep_mint_authorization_protocol,
            OfflineCashPastaParityV1::Ep,
        )
        .map_err(OfflineCashArtifactErrorV1::InvalidRelease)?;
        if mint_authorization_eq_protocol_digest != recursion.mint_authorization_eq_protocol_digest
            || mint_authorization_ep_protocol_digest
                != recursion.mint_authorization_ep_protocol_digest
        {
            return Err(OfflineCashArtifactErrorV1::InvalidRelease(
                "compiled mint-authorization protocol identity mismatch".to_owned(),
            ));
        }
        let eq_mint_protocol = compile(
            &eq_parameters,
            &eq_mint_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![OFFLINE_CASH_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let ep_mint_protocol = compile(
            &ep_parameters,
            &ep_mint_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![OFFLINE_CASH_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let mint_eq_protocol_digest =
            native_parent_protocol_digest_v1(&eq_mint_protocol, OfflineCashPastaParityV1::Eq)
                .map_err(OfflineCashArtifactErrorV1::InvalidRelease)?;
        let mint_ep_protocol_digest =
            native_parent_protocol_digest_v1(&ep_mint_protocol, OfflineCashPastaParityV1::Ep)
                .map_err(OfflineCashArtifactErrorV1::InvalidRelease)?;
        if mint_eq_protocol_digest != profile.mint_eq_protocol_digest
            || mint_ep_protocol_digest != profile.mint_ep_protocol_digest
        {
            return Err(OfflineCashArtifactErrorV1::InvalidRelease(
                "compiled finalized-mint protocol identity mismatch".to_owned(),
            ));
        }
        let protocol_digests = [
            eq_protocol_digest,
            ep_protocol_digest,
            guard_eq_protocol_digest,
            guard_ep_protocol_digest,
            wrapper_eq_protocol_digest,
            wrapper_ep_protocol_digest,
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
            return Err(OfflineCashArtifactErrorV1::InvalidRelease(
                "compiled recursive protocol roles alias".to_owned(),
            ));
        }
        let verifier = Self {
            artifacts: recursion,
            eq_parameters,
            ep_parameters,
            eq_state_protocol,
            ep_state_protocol,
            eq_wrapper_protocol,
            ep_wrapper_protocol,
            eq_mint_authorization_protocol,
            ep_mint_authorization_protocol,
            eq_mint_protocol,
            ep_mint_protocol,
            eq_protocol_digest,
            ep_protocol_digest,
            guard_eq_protocol_digest,
            guard_ep_protocol_digest,
            wrapper_eq_protocol_digest,
            wrapper_ep_protocol_digest,
            mint_authorization_eq_protocol_digest,
            mint_authorization_ep_protocol_digest,
            mint_eq_protocol_digest,
            mint_ep_protocol_digest,
            mint_genesis_roster_id: profile.mint_genesis_roster_id,
            release_id: recursion.release_id,
            suite_id: artifacts.suite_id(),
            vk_set_digest: artifacts.vk_set_digest(),
            artifact_manifest_digest: recursion.artifact_manifest_digest,
            wrapper_eq_binding: recursion.commit_wrapper_verifying_key_eq,
            wrapper_ep_binding: recursion.commit_wrapper_verifying_key_ep,
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

    /// Return the actual Eq terminal-wrapper protocol identity derived from its authenticated key.
    #[must_use]
    pub const fn wrapper_eq_protocol_digest(&self) -> [u8; 32] {
        self.wrapper_eq_protocol_digest
    }

    /// Return the actual Ep terminal-wrapper protocol identity derived from its authenticated key.
    #[must_use]
    pub const fn wrapper_ep_protocol_digest(&self) -> [u8; 32] {
        self.wrapper_ep_protocol_digest
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
        authorization: &OfflineCashMintAuthorizationV1,
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
            return Err("Offline Cash mint-authorization release binding mismatch".to_owned());
        }
        validate_proof_length("Eq mint authorization", &proof.eq_proof)?;
        validate_proof_length("Ep mint authorization", &proof.ep_proof)?;
        let eq_history = OfflineCashEqAccumulatorV1::try_from_bytes(&proof.eq_history)
            .map_err(|error| error.to_string())?;
        let ep_history = OfflineCashEpAccumulatorV1::try_from_bytes(&proof.ep_history)
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
        let eq_current = OfflineCashEqAccumulatorV1::from_native(&verify_eq_succinct_protocol(
            &self.eq_parameters,
            &self.eq_mint_authorization_protocol,
            &proof.eq_proof,
            &eq_instances,
        )?)
        .map_err(|error| error.to_string())?;
        let ep_current = OfflineCashEpAccumulatorV1::from_native(&verify_ep_succinct_protocol(
            &self.ep_parameters,
            &self.ep_mint_authorization_protocol,
            &proof.ep_proof,
            &ep_instances,
        )?)
        .map_err(|error| error.to_string())?;
        decide_offline_cash_eq_accumulator_v1(&self.eq_parameters, &eq_current)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_eq_accumulator_v1(&self.eq_parameters, &eq_history)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_ep_accumulator_v1(&self.ep_parameters, &ep_current)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_ep_accumulator_v1(&self.ep_parameters, &ep_history)
            .map_err(|error| error.to_string())
    }

    /// Verify and decide a release-pinned paired sender authorization before ticket issuance.
    ///
    /// This is a distinct CommitWrapper branch from terminal payment verification. Its public
    /// instances are reconstructed exclusively from the request and authorization statement;
    /// terminal lifecycle columns are never accepted as a substitute.
    pub fn verify_acceptance_intent_authorization_and_decide(
        &self,
        release: &OfflineCashAuthenticatedReleaseV1,
        request: &OfflineCashPaymentRequestV1,
        authorization: &OfflineCashAcceptanceIntentAuthorizationV1,
    ) -> Result<OfflineCashAcceptanceIntentAuthorizationDecisionV1, String> {
        self.verify_acceptance_intent_authorization_against_loaded_release_and_decide(
            Some(release),
            request,
            authorization,
        )
    }

    fn verify_acceptance_intent_authorization_against_loaded_release_and_decide(
        &self,
        release: Option<&OfflineCashAuthenticatedReleaseV1>,
        request: &OfflineCashPaymentRequestV1,
        authorization: &OfflineCashAcceptanceIntentAuthorizationV1,
    ) -> Result<OfflineCashAcceptanceIntentAuthorizationDecisionV1, String> {
        authorization
            .validate_shape_against(request)
            .map_err(|error| error.to_string())?;
        let proof = &authorization.proof;
        let supplied_release_mismatch = release.is_some_and(|release| {
            release.release_id() != self.release_id
                || release
                    .enabled_profiles()
                    .iter()
                    .any(|profile| profile.suite_id != self.suite_id)
                || release.manifest_digest() != self.artifact_manifest_digest
                || release.vk_set_digest() != self.vk_set_digest
                || release.commit_wrapper_eq_protocol_digest() != self.wrapper_eq_protocol_digest
                || release.commit_wrapper_ep_protocol_digest() != self.wrapper_ep_protocol_digest
                || release.artifact(OfflineCashArtifactRoleV1::CommitWrapperVkEq)
                    != self.wrapper_eq_binding
                || release.artifact(OfflineCashArtifactRoleV1::CommitWrapperVkEp)
                    != self.wrapper_ep_binding
        });
        if supplied_release_mismatch {
            return Err(
                "Offline Cash acceptance-intent authorization release/key-role mismatch".to_owned(),
            );
        }
        validate_commit_wrapper_authority_binding_v1(
            "acceptance-intent authorization",
            OfflineCashCommitWrapperAuthorityBindingV1 {
                release_id: authorization.statement.release_id,
                suite_id: authorization.statement.suite_id,
                vk_set_digest: authorization.statement.vk_digest,
                artifact_manifest_digest: authorization.statement.artifact_manifest_digest,
                eq_protocol_digest: proof.eq_protocol_digest,
                ep_protocol_digest: proof.ep_protocol_digest,
            },
            self.commit_wrapper_authority_binding(),
            self.wrapper_eq_binding.role,
            self.wrapper_ep_binding.role,
        )?;
        validate_proof_length("Eq acceptance-intent authorization", &proof.eq_proof)?;
        validate_proof_length("Ep acceptance-intent authorization", &proof.ep_proof)?;
        let eq_history = OfflineCashEqAccumulatorV1::try_from_bytes(&proof.eq_history)
            .map_err(|error| error.to_string())?;
        let ep_history = OfflineCashEpAccumulatorV1::try_from_bytes(&proof.ep_history)
            .map_err(|error| error.to_string())?;
        let eq_instances = acceptance_intent_authorization_public_instances::<Fp>(
            request,
            authorization,
            eq_history.as_bytes(),
            self.wrapper_eq_protocol_digest,
            self.wrapper_ep_protocol_digest,
        )?;
        let ep_instances = acceptance_intent_authorization_public_instances::<Fq>(
            request,
            authorization,
            ep_history.as_bytes(),
            self.wrapper_eq_protocol_digest,
            self.wrapper_ep_protocol_digest,
        )?;
        let eq_current = OfflineCashEqAccumulatorV1::from_native(&verify_eq_succinct_protocol(
            &self.eq_parameters,
            &self.eq_wrapper_protocol,
            &proof.eq_proof,
            &eq_instances,
        )?)
        .map_err(|error| error.to_string())?;
        let ep_current = OfflineCashEpAccumulatorV1::from_native(&verify_ep_succinct_protocol(
            &self.ep_parameters,
            &self.ep_wrapper_protocol,
            &proof.ep_proof,
            &ep_instances,
        )?)
        .map_err(|error| error.to_string())?;
        decide_offline_cash_eq_accumulator_v1(&self.eq_parameters, &eq_current)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_eq_accumulator_v1(&self.eq_parameters, &eq_history)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_ep_accumulator_v1(&self.ep_parameters, &ep_current)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_ep_accumulator_v1(&self.ep_parameters, &ep_history)
            .map_err(|error| error.to_string())?;
        let request_digest = request
            .canonical_digest()
            .map_err(|error| error.to_string())?;
        let authorization_digest = authorization
            .canonical_digest_against(request)
            .map_err(|error| error.to_string())?;
        OfflineCashAcceptanceIntentAuthorizationDecisionV1::authenticated(
            self.release_id,
            self.artifact_manifest_digest,
            self.vk_set_digest,
            request_digest,
            authorization_digest,
            self.wrapper_eq_protocol_digest,
            self.wrapper_ep_protocol_digest,
        )
    }

    /// Atomically reverify the original sender authorization and its hardware cancellation.
    ///
    /// Neither proof is authoritative in isolation. The opaque recovery capability is minted
    /// only after both CommitWrapper parities, both carried histories, the exact original
    /// authorization envelope, and the authenticated release roles have all been decided.
    pub fn verify_no_commit_closure_and_decide(
        &self,
        closure: &OfflineCashNoCommitClosureV1,
    ) -> Result<OfflineCashNoCommitClosureDecisionV1, String> {
        closure
            .validate_shape()
            .map_err(|error| error.to_string())?;
        let statement = &closure.statement;
        let proof = &closure.proof;
        validate_commit_wrapper_authority_binding_v1(
            "no-commit closure",
            OfflineCashCommitWrapperAuthorityBindingV1 {
                release_id: statement.release_id,
                suite_id: statement.suite_id,
                vk_set_digest: statement.vk_digest,
                artifact_manifest_digest: statement.artifact_manifest_digest,
                eq_protocol_digest: proof.eq_protocol_digest,
                ep_protocol_digest: proof.ep_protocol_digest,
            },
            self.commit_wrapper_authority_binding(),
            self.wrapper_eq_binding.role,
            self.wrapper_ep_binding.role,
        )?;

        self.verify_acceptance_intent_authorization_against_loaded_release_and_decide(
            None,
            &closure.request,
            &closure.intent_authorization,
        )?;

        validate_proof_length("Eq no-commit closure", &proof.eq_proof)?;
        validate_proof_length("Ep no-commit closure", &proof.ep_proof)?;
        let eq_history = OfflineCashEqAccumulatorV1::try_from_bytes(&proof.eq_history)
            .map_err(|error| error.to_string())?;
        let ep_history = OfflineCashEpAccumulatorV1::try_from_bytes(&proof.ep_history)
            .map_err(|error| error.to_string())?;
        let eq_instances = no_commit_closure_public_instances::<Fp>(
            closure,
            eq_history.as_bytes(),
            self.wrapper_eq_protocol_digest,
            self.wrapper_ep_protocol_digest,
        )?;
        let ep_instances = no_commit_closure_public_instances::<Fq>(
            closure,
            ep_history.as_bytes(),
            self.wrapper_eq_protocol_digest,
            self.wrapper_ep_protocol_digest,
        )?;
        let eq_current = OfflineCashEqAccumulatorV1::from_native(&verify_eq_succinct_protocol(
            &self.eq_parameters,
            &self.eq_wrapper_protocol,
            &proof.eq_proof,
            &eq_instances,
        )?)
        .map_err(|error| error.to_string())?;
        let ep_current = OfflineCashEpAccumulatorV1::from_native(&verify_ep_succinct_protocol(
            &self.ep_parameters,
            &self.ep_wrapper_protocol,
            &proof.ep_proof,
            &ep_instances,
        )?)
        .map_err(|error| error.to_string())?;
        decide_offline_cash_eq_accumulator_v1(&self.eq_parameters, &eq_current)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_eq_accumulator_v1(&self.eq_parameters, &eq_history)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_ep_accumulator_v1(&self.ep_parameters, &ep_current)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_ep_accumulator_v1(&self.ep_parameters, &ep_history)
            .map_err(|error| error.to_string())?;
        OfflineCashNoCommitClosureDecisionV1::authenticated(closure)
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
        checkpoint: &OfflineCashMintAuthorityCheckpointV1,
    ) -> Result<(OfflineCashEqAccumulatorV1, OfflineCashEpAccumulatorV1), String> {
        checkpoint.validate_shape()?;
        if checkpoint.proof.eq_protocol_digest != self.mint_eq_protocol_digest
            || checkpoint.proof.ep_protocol_digest != self.mint_ep_protocol_digest
            || checkpoint.genesis_roster_id != self.mint_genesis_roster_id
        {
            return Err("Offline Cash mint-authority checkpoint release mismatch".to_owned());
        }
        let eq_history = OfflineCashEqAccumulatorV1::try_from_bytes(&checkpoint.proof.eq_history)
            .map_err(|error| error.to_string())?;
        let ep_history = OfflineCashEpAccumulatorV1::try_from_bytes(&checkpoint.proof.ep_history)
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
        let eq_current = OfflineCashEqAccumulatorV1::from_native(&verify_eq_succinct_protocol(
            &self.eq_parameters,
            &self.eq_mint_protocol,
            &checkpoint.proof.eq_proof,
            &eq_instances,
        )?)
        .map_err(|error| error.to_string())?;
        let ep_current = OfflineCashEpAccumulatorV1::from_native(&verify_ep_succinct_protocol(
            &self.ep_parameters,
            &self.ep_mint_protocol,
            &checkpoint.proof.ep_proof,
            &ep_instances,
        )?)
        .map_err(|error| error.to_string())?;
        decide_offline_cash_eq_accumulator_v1(&self.eq_parameters, &eq_current)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_eq_accumulator_v1(&self.eq_parameters, &eq_history)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_ep_accumulator_v1(&self.ep_parameters, &ep_current)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_ep_accumulator_v1(&self.ep_parameters, &ep_history)
            .map_err(|error| error.to_string())?;
        Ok((eq_current, ep_current))
    }

    fn verify_request(
        &self,
        request: &OfflineCashParityVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        if request.protocol_digest
            != match request.parity {
                OfflineCashPastaParityV1::Eq => self.wrapper_eq_protocol_digest,
                OfflineCashPastaParityV1::Ep => self.wrapper_ep_protocol_digest,
            }
        {
            return Err("Offline Cash commit-wrapper protocol identity mismatch".to_owned());
        }
        validate_proof_length("commit-wrapper", request.current_proof)?;
        match request.parity {
            OfflineCashPastaParityV1::Eq => {
                let instances = wrapper_public_instances::<Fp>(
                    request,
                    self.wrapper_eq_protocol_digest,
                    self.wrapper_ep_protocol_digest,
                )?;
                let current = verify_eq_succinct_protocol(
                    &self.eq_parameters,
                    &self.eq_wrapper_protocol,
                    request.current_proof,
                    &instances,
                )?;
                let current = OfflineCashEqAccumulatorV1::from_native(&current)
                    .map_err(|error| error.to_string())?;
                decide_offline_cash_eq_accumulator_v1(&self.eq_parameters, &current)
                    .map_err(|error| error.to_string())?;
                let history =
                    OfflineCashEqAccumulatorV1::try_from_bytes(request.history_accumulator)
                        .map_err(|error| error.to_string())?;
                decide_offline_cash_eq_accumulator_v1(&self.eq_parameters, &history)
                    .map_err(|error| error.to_string())
            }
            OfflineCashPastaParityV1::Ep => {
                let instances = wrapper_public_instances::<Fq>(
                    request,
                    self.wrapper_eq_protocol_digest,
                    self.wrapper_ep_protocol_digest,
                )?;
                let current = verify_ep_succinct_protocol(
                    &self.ep_parameters,
                    &self.ep_wrapper_protocol,
                    request.current_proof,
                    &instances,
                )?;
                let current = OfflineCashEpAccumulatorV1::from_native(&current)
                    .map_err(|error| error.to_string())?;
                decide_offline_cash_ep_accumulator_v1(&self.ep_parameters, &current)
                    .map_err(|error| error.to_string())?;
                let history =
                    OfflineCashEpAccumulatorV1::try_from_bytes(request.history_accumulator)
                        .map_err(|error| error.to_string())?;
                decide_offline_cash_ep_accumulator_v1(&self.ep_parameters, &history)
                    .map_err(|error| error.to_string())
            }
        }
    }
}

impl OfflineCashAcceptanceIntentAuthorizationVerifierV1
    for OfflineCashAuthenticatedRecursiveVerifierV1
{
    fn verify_acceptance_intent_authorization(
        &self,
        release: &OfflineCashAuthenticatedReleaseV1,
        request: &OfflineCashPaymentRequestV1,
        authorization: &OfflineCashAcceptanceIntentAuthorizationV1,
    ) -> Result<OfflineCashAcceptanceIntentAuthorizationDecisionV1, String> {
        self.verify_acceptance_intent_authorization_and_decide(release, request, authorization)
    }
}

impl OfflineCashNoCommitClosureVerifierV1 for OfflineCashAuthenticatedRecursiveVerifierV1 {
    fn verify_no_commit_closure(
        &self,
        closure: &OfflineCashNoCommitClosureV1,
    ) -> Result<OfflineCashNoCommitClosureDecisionV1, String> {
        self.verify_no_commit_closure_and_decide(closure)
    }
}

impl OfflineCashCandidateProofVerifierV1 for OfflineCashAuthenticatedRecursiveVerifierV1 {
    fn verify_candidate_proof(
        &self,
        candidate: &PreparedOutgoingCandidateV1,
        proof: &iroha_data_model::offline::OfflineCashPairedProofV1,
    ) -> Result<(), String> {
        let public_inputs = candidate.candidate_public_inputs(self.artifacts, proof)?;
        verify_offline_cash_state_proof_v1(self, self.artifacts, &public_inputs, proof)
            .map_err(|error| error.to_string())
    }
}

impl OfflineCashCommitWrapperVerifierV1 for OfflineCashAuthenticatedRecursiveVerifierV1 {
    fn verify_commit_wrapper(
        &self,
        public_inputs: &OfflineCashTerminalPublicInputsV1,
        proof: &iroha_data_model::offline::OfflineCashCommitWrapperProofV1,
    ) -> Result<(), String> {
        verify_offline_cash_recursive_proof_v1(self, self.artifacts, public_inputs.clone(), proof)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

impl OfflineCashRecursiveVerifierV1 for OfflineCashAuthenticatedRecursiveVerifierV1 {
    fn verify_state_proof_and_decide(
        &self,
        request: &OfflineCashStateProofVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        validate_proof_length("Eq aggregate state", &request.proof.eq_proof)?;
        validate_proof_length("Ep aggregate state", &request.proof.ep_proof)?;
        let eq_history = OfflineCashEqAccumulatorV1::try_from_bytes(&request.proof.eq_history)
            .map_err(|error| error.to_string())?;
        let ep_history = OfflineCashEpAccumulatorV1::try_from_bytes(&request.proof.ep_history)
            .map_err(|error| error.to_string())?;
        let mut eq_instances = request.public_inputs.public_instances::<Fp>()?;
        eq_instances.extend(history_public_instances::<Fp>(eq_history.as_bytes()));
        let mut ep_instances = request.public_inputs.public_instances::<Fq>()?;
        ep_instances.extend(history_public_instances::<Fq>(ep_history.as_bytes()));
        if eq_instances.len() != RECURSIVE_PUBLIC_INSTANCE_COUNT_V1
            || ep_instances.len() != RECURSIVE_PUBLIC_INSTANCE_COUNT_V1
        {
            return Err("Offline Cash state public instance ABI mismatch".to_owned());
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
        let eq_current = OfflineCashEqAccumulatorV1::from_native(&eq_current)
            .map_err(|error| error.to_string())?;
        let ep_current = OfflineCashEpAccumulatorV1::from_native(&ep_current)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_eq_accumulator_v1(&self.eq_parameters, &eq_current)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_ep_accumulator_v1(&self.ep_parameters, &ep_current)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_eq_accumulator_v1(&self.eq_parameters, &eq_history)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_ep_accumulator_v1(&self.ep_parameters, &ep_history)
            .map_err(|error| error.to_string())
    }

    fn verify_mint_finality_helper(
        &self,
        request: &super::OfflineCashMintFinalityHelperVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        if request.eq_protocol_digest != self.mint_eq_protocol_digest
            || request.ep_protocol_digest != self.mint_ep_protocol_digest
            || request.finality_genesis_roster_id != self.mint_genesis_roster_id
        {
            return Err("Offline Cash mint-authority release binding mismatch".to_owned());
        }
        validate_proof_length("Eq mint-authority", &request.proof.eq_proof)?;
        validate_proof_length("Ep mint-authority", &request.proof.ep_proof)?;
        let eq_history = OfflineCashEqAccumulatorV1::try_from_bytes(&request.proof.eq_history)
            .map_err(|error| error.to_string())?;
        let ep_history = OfflineCashEpAccumulatorV1::try_from_bytes(&request.proof.ep_history)
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
        let eq_current = OfflineCashEqAccumulatorV1::from_native(&eq_current)
            .map_err(|error| error.to_string())?;
        let ep_current = OfflineCashEpAccumulatorV1::from_native(&ep_current)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_eq_accumulator_v1(&self.eq_parameters, &eq_current)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_ep_accumulator_v1(&self.ep_parameters, &ep_current)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_eq_accumulator_v1(&self.eq_parameters, &eq_history)
            .map_err(|error| error.to_string())?;
        decide_offline_cash_ep_accumulator_v1(&self.ep_parameters, &ep_history)
            .map_err(|error| error.to_string())
    }

    fn verify_commit_wrapper_and_decide(
        &self,
        request: &OfflineCashParityVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        self.verify_request(request)
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
            .map_err(|_| "Offline Cash circuit profile value exceeds u64".to_owned())?
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
) -> Result<VerifyingKey<EqAffine>, OfflineCashArtifactErrorV1> {
    read_eq_recursive_vk::<OfflineCashRecursiveStateEqCircuitV1>(bytes, params, "state")
}

fn read_ep_state_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, OfflineCashArtifactErrorV1> {
    read_ep_recursive_vk::<OfflineCashRecursiveStateEpCircuitV1>(bytes, params, "state")
}

fn read_eq_guard_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, OfflineCashArtifactErrorV1> {
    read_eq_recursive_vk::<OfflineCashGuardBundleEqCircuitV1>(bytes, params, "GuardBundle")
}

fn read_ep_guard_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, OfflineCashArtifactErrorV1> {
    read_ep_recursive_vk::<OfflineCashGuardBundleEpCircuitV1>(bytes, params, "GuardBundle")
}

fn read_eq_wrapper_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, OfflineCashArtifactErrorV1> {
    read_eq_recursive_vk::<OfflineCashCommitWrapperEqCircuitV1>(bytes, params, "commit wrapper")
}

fn read_ep_wrapper_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, OfflineCashArtifactErrorV1> {
    read_ep_recursive_vk::<OfflineCashCommitWrapperEpCircuitV1>(bytes, params, "commit wrapper")
}

fn read_eq_mint_authorization_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, OfflineCashArtifactErrorV1> {
    read_eq_recursive_vk::<OfflineCashMintAuthorizationEqCircuitV1>(
        bytes,
        params,
        "mint authorization",
    )
}

fn read_ep_mint_authorization_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, OfflineCashArtifactErrorV1> {
    read_ep_recursive_vk::<OfflineCashMintAuthorizationEpCircuitV1>(
        bytes,
        params,
        "mint authorization",
    )
}

fn read_eq_mint_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, OfflineCashArtifactErrorV1> {
    read_eq_recursive_vk::<OfflineCashMintAuthorityEqCircuitV1>(bytes, params, "mint authority")
}

fn read_ep_mint_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, OfflineCashArtifactErrorV1> {
    read_ep_recursive_vk::<OfflineCashMintAuthorityEpCircuitV1>(bytes, params, "mint authority")
}

fn read_eq_recursive_vk<C>(
    bytes: &[u8],
    params: BaseCircuitParams,
    label: &str,
) -> Result<VerifyingKey<EqAffine>, OfflineCashArtifactErrorV1>
where
    C: halo2_proofs::plonk::Circuit<Fp, Params = BaseCircuitParams>,
{
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read::<_, C>(&mut cursor, SerdeFormat::Processed, params).map_err(
        |error| {
            OfflineCashArtifactErrorV1::InvalidRelease(format!(
                "failed to decode Eq {label} verifying key: {error}"
            ))
        },
    )?;
    if cursor.position() != bytes.len() as u64 {
        return Err(OfflineCashArtifactErrorV1::InvalidRelease(format!(
            "Eq {label} verifying key has trailing bytes"
        )));
    }
    Ok(key)
}

fn read_ep_recursive_vk<C>(
    bytes: &[u8],
    params: BaseCircuitParams,
    label: &str,
) -> Result<VerifyingKey<EpAffine>, OfflineCashArtifactErrorV1>
where
    C: halo2_proofs::plonk::Circuit<Fq, Params = BaseCircuitParams>,
{
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read::<_, C>(&mut cursor, SerdeFormat::Processed, params).map_err(
        |error| {
            OfflineCashArtifactErrorV1::InvalidRelease(format!(
                "failed to decode Ep {label} verifying key: {error}"
            ))
        },
    )?;
    if cursor.position() != bytes.len() as u64 {
        return Err(OfflineCashArtifactErrorV1::InvalidRelease(format!(
            "Ep {label} verifying key has trailing bytes"
        )));
    }
    Ok(key)
}

fn wrapper_public_instances<F: OfflineCashPoseidonFieldV1>(
    request: &OfflineCashParityVerificationRequestV1<'_>,
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
) -> Result<Vec<F>, String> {
    let wrapper = OfflineCashCommitWrapperPublicInputsV1::from_lifecycle(
        &request.public_output.lifecycle,
        request.public_output.semantic_digest,
        request.public_output.candidate_envelope_digest,
        request.public_output.commit_certificate_digest,
        request.public_output.transition_nullifier,
        request.public_output.request_digest,
        request.public_output.acceptance_ticket_digest,
        request.public_output.ciphertext_commitment,
        request.public_output.amount,
        request.public_output.terminal_output_binding,
        request.eq_deferred_audit,
        request.ep_deferred_audit,
        eq_protocol_digest,
        ep_protocol_digest,
    )?;
    let mut public = wrapper.public_prefix::<F>()?;
    public.extend(request.history_accumulator.chunks_exact(16).map(|chunk| {
        from_u128::<F>(u128::from_le_bytes(
            chunk.try_into().expect("history chunk has sixteen bytes"),
        ))
    }));
    if public.len() != COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1
        || wrapper_public_instance::HISTORY_START != 47
    {
        return Err("Offline Cash commit-wrapper public instance ABI mismatch".to_owned());
    }
    Ok(public)
}

fn acceptance_intent_authorization_public_instances<F: OfflineCashPoseidonFieldV1>(
    request: &OfflineCashPaymentRequestV1,
    authorization: &OfflineCashAcceptanceIntentAuthorizationV1,
    history: &[u8; super::OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
) -> Result<Vec<F>, String> {
    let wrapper = OfflineCashCommitWrapperPublicInputsV1::from_acceptance_intent_authorization(
        request,
        &authorization.statement,
        authorization.proof.guard_eq_credential_audit,
        authorization.proof.guard_ep_credential_audit,
        authorization.proof.eq_deferred_audit,
        authorization.proof.ep_deferred_audit,
        eq_protocol_digest,
        ep_protocol_digest,
    )?;
    let mut public = wrapper.public_prefix::<F>()?;
    public.extend(history_public_instances::<F>(history));
    if public.len() != COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1
        || wrapper_public_instance::HISTORY_START != 47
    {
        return Err("Offline Cash acceptance-intent public instance ABI mismatch".to_owned());
    }
    Ok(public)
}

fn no_commit_closure_public_instances<F: OfflineCashPoseidonFieldV1>(
    closure: &OfflineCashNoCommitClosureV1,
    history: &[u8; super::OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
) -> Result<Vec<F>, String> {
    closure
        .validate_shape()
        .map_err(|error| error.to_string())?;
    let wrapper = OfflineCashCommitWrapperPublicInputsV1::from_no_commit_closure(
        &closure.statement,
        closure.proof.guard_eq_credential_audit,
        closure.proof.guard_ep_credential_audit,
        closure.proof.eq_deferred_audit,
        closure.proof.ep_deferred_audit,
        eq_protocol_digest,
        ep_protocol_digest,
    )?;
    let mut public = wrapper.public_prefix::<F>()?;
    public.extend(history_public_instances::<F>(history));
    if public.len() != COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1
        || wrapper_public_instance::HISTORY_START != 47
    {
        return Err("Offline Cash no-commit closure public instance ABI mismatch".to_owned());
    }
    Ok(public)
}

struct MintPublicPartsV1 {
    step: OfflineCashMintAuthorityStepV1,
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

fn history_public_instances<F: OfflineCashPoseidonFieldV1>(
    history: &[u8; super::OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
) -> impl Iterator<Item = F> + '_ {
    history.chunks_exact(16).map(|chunk| {
        from_u128::<F>(u128::from_le_bytes(
            chunk
                .try_into()
                .expect("fixed history chunk has sixteen bytes"),
        ))
    })
}

fn mint_public_instances<F: OfflineCashPoseidonFieldV1>(
    request: &super::OfflineCashMintFinalityHelperVerificationRequestV1<'_>,
    history: &[u8; super::OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<Vec<F>, String> {
    let canonical_semantic = request
        .statement
        .canonical_digest()
        .map_err(|error| error.to_string())?;
    if canonical_semantic != request.semantic_digest
        || request.proof.semantic_digest != request.semantic_digest
        || request.proof.guard_eq_credential_audit != request.finality_certificate_binding
        || request.proof.guard_ep_credential_audit != request.finality_authority_head
    {
        return Err("Offline Cash mint-authority semantic binding mismatch".to_owned());
    }
    let eq_history = OfflineCashEqAccumulatorV1::try_from_bytes(&request.proof.eq_history)
        .map_err(|error| error.to_string())?;
    let ep_history = OfflineCashEpAccumulatorV1::try_from_bytes(&request.proof.ep_history)
        .map_err(|error| error.to_string())?;
    let pair_binding = OfflineCashMintAuthorityPairBindingV1 {
        step: OfflineCashMintAuthorityStepV1::FinalizedMint,
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
        eq_history: eq_history.as_bytes(),
        ep_history: ep_history.as_bytes(),
    }
    .canonical_digest();
    if pair_binding != request.finality_proof_binding_digest {
        return Err("Offline Cash mint-authority paired binding mismatch".to_owned());
    }
    mint_public_instances_from_parts::<F>(
        &MintPublicPartsV1 {
            step: OfflineCashMintAuthorityStepV1::FinalizedMint,
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
            proof_binding_digest: pair_binding,
        },
        history,
    )
}

fn mint_public_instances_from_parts<F: OfflineCashPoseidonFieldV1>(
    parts: &MintPublicPartsV1,
    history: &[u8; super::OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
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
    if public.len() != OFFLINE_CASH_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1
        || mint_public_instance::HISTORY_START != 22
    {
        return Err("Offline Cash mint-authority public instance ABI mismatch".to_owned());
    }
    Ok(public)
}

fn validate_proof_length(parity: &str, proof: &[u8]) -> Result<(), String> {
    if proof.is_empty() || proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1 {
        return Err(format!(
            "Offline Cash {parity} proof length {} exceeds the fixed 1..={OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1} bound",
            proof.len()
        ));
    }
    Ok(())
}

pub(super) fn verify_eq_succinct_protocol(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EqAffine>,
    protocol: &snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
    proof: &[u8],
    instances: &[Fp],
) -> Result<IpaAccumulator<EqAffine, NativeLoader>, String> {
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
    .map_err(|_| "Offline Cash Eq proof parser panicked".to_owned())?
    .map_err(|error| format!("invalid Offline Cash Eq proof: {error:?}"))?;
    let accumulators = crate::panic_hook::catch_unwind_suppressed(|| {
        PlonkSuccinctVerifier::<Scheme>::verify(&svk, &protocol, &instance_columns, &parsed)
    })
    .map_err(|_| "Offline Cash Eq verifier panicked".to_owned())?
    .map_err(|error| format!("Offline Cash Eq proof rejected: {error:?}"))?;
    drop(transcript);
    if cursor.position() != proof.len() as u64 {
        return Err("Offline Cash Eq proof has trailing bytes".to_owned());
    }
    let [accumulator] = accumulators.try_into().map_err(|values: Vec<_>| {
        format!(
            "Offline Cash Eq proof returned {} accumulators",
            values.len()
        )
    })?;
    Ok(accumulator)
}

pub(super) fn verify_ep_succinct_protocol(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EpAffine>,
    protocol: &snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
    proof: &[u8],
    instances: &[Fq],
) -> Result<IpaAccumulator<EpAffine, NativeLoader>, String> {
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
    .map_err(|_| "Offline Cash Ep proof parser panicked".to_owned())?
    .map_err(|error| format!("invalid Offline Cash Ep proof: {error:?}"))?;
    let accumulators = crate::panic_hook::catch_unwind_suppressed(|| {
        PlonkSuccinctVerifier::<Scheme>::verify(&svk, &protocol, &instance_columns, &parsed)
    })
    .map_err(|_| "Offline Cash Ep verifier panicked".to_owned())?
    .map_err(|error| format!("Offline Cash Ep proof rejected: {error:?}"))?;
    drop(transcript);
    if cursor.position() != proof.len() as u64 {
        return Err("Offline Cash Ep proof has trailing bytes".to_owned());
    }
    let [accumulator] = accumulators.try_into().map_err(|values: Vec<_>| {
        format!(
            "Offline Cash Ep proof returned {} accumulators",
            values.len()
        )
    })?;
    Ok(accumulator)
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        NetworkId,
        account::AccountId,
        asset::AssetDefinitionId,
        block::BlockHeader,
        domain::DomainId,
        nexus::AxtAssetIncarnationV1,
        offline::{
            OFFLINE_CASH_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1,
            OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1, OFFLINE_CASH_WIRE_VERSION_V1,
            OfflineCashAcceptanceIntentAuthorizationStatementV1, OfflineCashAcceptanceIntentV1,
            OfflineCashAcceptanceTicketV1, OfflineCashDevicePublicKeyV1,
            OfflineCashDeviceSignatureV1, OfflineCashHardwareCredentialV1,
            OfflineCashNoCommitClosureStatementV1, OfflineCashNoCommitClosureV1,
            OfflineCashPairedProofV1, OfflineCashSingleExactRequestV1,
            offline_cash_device_key_reference_v1, offline_cash_liability_pool_id_v1,
        },
    };
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

    use super::*;

    #[cfg(feature = "zk-halo2-ipa")]
    mod real_no_commit_generation {
        include!("native_backend_no_commit_tests.rs");
    }

    fn signing_key() -> SigningKey {
        SigningKey::from_bytes((&[0x21; 32]).into()).expect("P-256 test key")
    }

    fn sign(key: &SigningKey, bytes: &[u8]) -> OfflineCashDeviceSignatureV1 {
        let signature: Signature = key.sign(bytes);
        let signature = signature.normalize_s().unwrap_or(signature);
        OfflineCashDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
            .expect("canonical test signature")
    }

    fn acceptance_authorization_fixture() -> (
        OfflineCashPaymentRequestV1,
        OfflineCashAcceptanceIntentAuthorizationV1,
    ) {
        let key = signing_key();
        let device_public_key = OfflineCashDevicePublicKeyV1::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .expect("test device key");
        let network_id =
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"native-acceptance-authorization",
            )));
        let asset = AssetDefinitionId::derive_from_components(
            DomainId::try_new("authorization", "universal").expect("test domain"),
            "cash".parse().expect("test asset name"),
        );
        let asset_incarnation =
            AxtAssetIncarnationV1::try_from_bytes([0x39; 32]).expect("asset incarnation");
        let mut credential = OfflineCashHardwareCredentialV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            credential_id: [0; 32],
            network_id,
            hardware_profile_id: [0x31; 32],
            suite_id: [0x32; 32],
            firmware_policy_digest: [0x33; 32],
            policy_epoch: 1,
            lane_commitment: [0x34; 32],
            hardware_epoch_id: [0x35; 32],
            hardware_epoch_generation: 1,
            device_public_key,
            device_key_reference: offline_cash_device_key_reference_v1(&device_public_key),
            issued_at_ms: 1,
            expires_at_ms: 100_000,
            governance_signature: sign(&key, b"shape-only governance signature"),
        }
        .seal_credential_id()
        .expect("credential id");
        credential.governance_signature = sign(&key, b"shape-only governance signature");
        let mut request = OfflineCashPaymentRequestV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            release_id: [0x41; 32],
            network_id,
            asset: asset.clone(),
            asset_incarnation,
            scale: 2,
            liability_pool_id: offline_cash_liability_pool_id_v1(
                &network_id,
                &asset,
                asset_incarnation,
            )
            .expect("liability pool"),
            recipient: AccountId::new(
                KeyPair::from_seed(vec![0x44; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone(),
            ),
            request_mode: iroha_data_model::offline::OfflineCashPaymentRequestModeV1::SingleExact(
                OfflineCashSingleExactRequestV1 { amount: 7 },
            ),
            hardware_credential: credential,
            request_id: [0x45; 32],
            issued_at_ms: 100,
            expires_at_ms: 10_000,
            signature: sign(&key, b"placeholder"),
        };
        request.signature = sign(
            &key,
            &request
                .canonical_signing_bytes()
                .expect("request signing bytes"),
        );
        let intent = OfflineCashAcceptanceIntentV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            request_digest: request.canonical_digest().expect("request digest"),
            intent_id: [0x46; 32],
            exact_amount: 7,
            sender_one_time_commitment: [0x47; 32],
        };
        let statement = OfflineCashAcceptanceIntentAuthorizationStatementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            intent,
            release_id: request.release_id,
            suite_id: request.hardware_credential.suite_id,
            vk_digest: [0x48; 32],
            artifact_manifest_digest: [0x49; 32],
        };
        let semantic_digest = statement
            .canonical_digest_against(&request)
            .expect("authorization statement digest");
        let authorization = OfflineCashAcceptanceIntentAuthorizationV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            statement,
            proof: iroha_data_model::offline::OfflineCashPairedProofV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                eq_protocol_digest: [0x51; 32],
                ep_protocol_digest: [0x52; 32],
                semantic_digest,
                guard_eq_credential_audit: [0x53; 32],
                guard_ep_credential_audit: [0x54; 32],
                eq_deferred_audit: [0x55; 32],
                ep_deferred_audit: [0x56; 32],
                eq_proof: vec![0x57],
                ep_proof: vec![0x58],
                eq_history: vec![0x59; super::super::OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
                ep_history: vec![0x5A; super::super::OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
            },
        };
        authorization
            .validate_shape_against(&request)
            .expect("authorization shape");
        (request, authorization)
    }

    fn no_commit_closure_fixture() -> OfflineCashNoCommitClosureV1 {
        let (request, intent_authorization) = acceptance_authorization_fixture();
        let key = signing_key();
        let intent = intent_authorization.statement.intent;
        let mut acceptance_ticket = OfflineCashAcceptanceTicketV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            network_id: request.network_id,
            request_id: request.request_id,
            request_digest: request.canonical_digest().expect("request digest"),
            acceptance_ticket_id: [0x61; 32],
            asset: request.asset.clone(),
            asset_incarnation: request.asset_incarnation,
            scale: request.scale,
            request_mode: request.request_mode,
            intent_digest: intent
                .canonical_digest_against(&request)
                .expect("intent digest"),
            exact_amount: intent.exact_amount,
            reserved_inbox_bytes: OFFLINE_CASH_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1,
            recipient_one_time_key: [0x62; 32],
            hardware_profile_id: request.hardware_credential.hardware_profile_id,
            policy_epoch: request.hardware_credential.policy_epoch,
            issued_at_ms: 200,
            expires_at_ms: 9_000,
            signature: sign(&key, b"placeholder"),
        };
        acceptance_ticket.signature = sign(
            &key,
            &acceptance_ticket
                .canonical_signing_bytes()
                .expect("ticket signing bytes"),
        );
        acceptance_ticket
            .validate_shape_against(&request, &intent)
            .expect("ticket shape");
        let statement = OfflineCashNoCommitClosureStatementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            release_id: intent_authorization.statement.release_id,
            suite_id: intent_authorization.statement.suite_id,
            vk_digest: intent_authorization.statement.vk_digest,
            artifact_manifest_digest: intent_authorization.statement.artifact_manifest_digest,
            sender_hardware_binding_commitment: [0x63; 32],
            request_id: request.request_id,
            request_digest: request.canonical_digest().expect("request digest"),
            acceptance_ticket_id: acceptance_ticket.acceptance_ticket_id,
            ticket_digest: acceptance_ticket
                .canonical_digest_against(&request, &intent)
                .expect("ticket digest"),
            intent_authorization_digest: intent_authorization
                .canonical_digest_against(&request)
                .expect("authorization digest"),
            intent_digest: intent
                .canonical_digest_against(&request)
                .expect("intent digest"),
            exact_amount: intent.exact_amount,
            sender_one_time_commitment: intent.sender_one_time_commitment,
            recovery_id: [0x64; 32],
            cancellation_nullifier: [0x65; 32],
            equivalent_delivery_slot_commitment: [0x66; 32],
        };
        let semantic_digest = statement.canonical_digest().expect("closure digest");
        let closure = OfflineCashNoCommitClosureV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            statement,
            request,
            intent_authorization,
            acceptance_ticket,
            proof: OfflineCashPairedProofV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                eq_protocol_digest: [0x51; 32],
                ep_protocol_digest: [0x52; 32],
                semantic_digest,
                guard_eq_credential_audit: [0x67; 32],
                guard_ep_credential_audit: [0x68; 32],
                eq_deferred_audit: [0x69; 32],
                ep_deferred_audit: [0x6A; 32],
                eq_proof: vec![0x6B],
                ep_proof: vec![0x6C],
                eq_history: vec![0x6D; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
                ep_history: vec![0x6E; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
            },
        };
        closure.validate_shape().expect("closure shape");
        closure
    }

    #[test]
    fn ordinary_proof_length_is_strictly_bounded() {
        assert!(validate_proof_length("Eq", &[]).is_err());
        assert!(validate_proof_length("Eq", &[0; 1]).is_ok());
        assert!(validate_proof_length("Ep", &[0; OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1]).is_ok());
        assert!(
            validate_proof_length("Ep", &[0; OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1 + 1]).is_err()
        );
    }

    #[test]
    fn authenticated_guard_protocol_rejects_eq_substitution() {
        let expected_eq = [0x71; 32];
        let expected_ep = [0x72; 32];
        validate_authenticated_guard_protocol_binding_v1(
            expected_eq,
            expected_ep,
            expected_eq,
            expected_ep,
        )
        .expect("exact authenticated GuardBundle protocols");
        assert!(
            validate_authenticated_guard_protocol_binding_v1(
                [0x73; 32],
                expected_ep,
                expected_eq,
                expected_ep,
            )
            .is_err()
        );
    }

    #[test]
    fn authenticated_guard_protocol_rejects_ep_substitution() {
        let expected_eq = [0x71; 32];
        let expected_ep = [0x72; 32];
        assert!(
            validate_authenticated_guard_protocol_binding_v1(
                expected_eq,
                [0x73; 32],
                expected_eq,
                expected_ep,
            )
            .is_err()
        );
    }

    #[test]
    fn acceptance_authorization_rejects_release_suite_substitution() {
        let (_request, authorization) = acceptance_authorization_fixture();
        let expected = OfflineCashCommitWrapperAuthorityBindingV1 {
            release_id: authorization.statement.release_id,
            suite_id: authorization.statement.suite_id,
            vk_set_digest: authorization.statement.vk_digest,
            artifact_manifest_digest: authorization.statement.artifact_manifest_digest,
            eq_protocol_digest: authorization.proof.eq_protocol_digest,
            ep_protocol_digest: authorization.proof.ep_protocol_digest,
        };
        validate_commit_wrapper_authority_binding_v1(
            "acceptance-intent authorization",
            expected,
            expected,
            OfflineCashArtifactRoleV1::CommitWrapperVkEq,
            OfflineCashArtifactRoleV1::CommitWrapperVkEp,
        )
        .expect("exact authenticated suite binding");
        let actual = OfflineCashCommitWrapperAuthorityBindingV1 {
            suite_id: [0xFE; 32],
            ..expected
        };
        assert!(
            validate_commit_wrapper_authority_binding_v1(
                "acceptance-intent authorization",
                actual,
                expected,
                OfflineCashArtifactRoleV1::CommitWrapperVkEq,
                OfflineCashArtifactRoleV1::CommitWrapperVkEp,
            )
            .is_err()
        );
    }

    #[test]
    fn no_commit_closure_rejects_release_suite_substitution() {
        let closure = no_commit_closure_fixture();
        let expected = OfflineCashCommitWrapperAuthorityBindingV1 {
            release_id: closure.statement.release_id,
            suite_id: closure.statement.suite_id,
            vk_set_digest: closure.statement.vk_digest,
            artifact_manifest_digest: closure.statement.artifact_manifest_digest,
            eq_protocol_digest: closure.proof.eq_protocol_digest,
            ep_protocol_digest: closure.proof.ep_protocol_digest,
        };
        validate_commit_wrapper_authority_binding_v1(
            "no-commit closure",
            expected,
            expected,
            OfflineCashArtifactRoleV1::CommitWrapperVkEq,
            OfflineCashArtifactRoleV1::CommitWrapperVkEp,
        )
        .expect("exact authenticated suite binding");
        let actual = OfflineCashCommitWrapperAuthorityBindingV1 {
            suite_id: [0xFD; 32],
            ..expected
        };
        assert!(
            validate_commit_wrapper_authority_binding_v1(
                "no-commit closure",
                actual,
                expected,
                OfflineCashArtifactRoleV1::CommitWrapperVkEq,
                OfflineCashArtifactRoleV1::CommitWrapperVkEp,
            )
            .is_err()
        );
    }

    #[test]
    fn acceptance_authorization_uses_its_exact_public_branch_for_both_parities() {
        let (request, authorization) = acceptance_authorization_fixture();
        let eq_history: [u8; super::super::OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1] =
            authorization
                .proof
                .eq_history
                .clone()
                .try_into()
                .expect("Eq history");
        let ep_history: [u8; super::super::OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1] =
            authorization
                .proof
                .ep_history
                .clone()
                .try_into()
                .expect("Ep history");
        let eq = acceptance_intent_authorization_public_instances::<Fp>(
            &request,
            &authorization,
            &eq_history,
            authorization.proof.eq_protocol_digest,
            authorization.proof.ep_protocol_digest,
        )
        .expect("Eq authorization instances");
        let ep = acceptance_intent_authorization_public_instances::<Fq>(
            &request,
            &authorization,
            &ep_history,
            authorization.proof.eq_protocol_digest,
            authorization.proof.ep_protocol_digest,
        )
        .expect("Ep authorization instances");
        assert_eq!(eq.len(), COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1);
        assert_eq!(ep.len(), COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1);

        let mut wrong_request = request.clone();
        wrong_request.request_id = [0xFF; 32];
        assert!(
            acceptance_intent_authorization_public_instances::<Fp>(
                &wrong_request,
                &authorization,
                &eq_history,
                authorization.proof.eq_protocol_digest,
                authorization.proof.ep_protocol_digest,
            )
            .is_err()
        );
        let mut wrong_release = authorization.clone();
        wrong_release.statement.release_id = [0xFE; 32];
        assert!(
            acceptance_intent_authorization_public_instances::<Fq>(
                &request,
                &wrong_release,
                &ep_history,
                authorization.proof.eq_protocol_digest,
                authorization.proof.ep_protocol_digest,
            )
            .is_err()
        );
    }

    #[test]
    fn no_commit_closure_uses_exact_public_branch_for_both_parities() {
        let closure = no_commit_closure_fixture();
        let eq_history: [u8; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1] = closure
            .proof
            .eq_history
            .clone()
            .try_into()
            .expect("Eq history");
        let ep_history: [u8; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1] = closure
            .proof
            .ep_history
            .clone()
            .try_into()
            .expect("Ep history");
        let eq = no_commit_closure_public_instances::<Fp>(
            &closure,
            &eq_history,
            closure.proof.eq_protocol_digest,
            closure.proof.ep_protocol_digest,
        )
        .expect("Eq closure instances");
        let ep = no_commit_closure_public_instances::<Fq>(
            &closure,
            &ep_history,
            closure.proof.eq_protocol_digest,
            closure.proof.ep_protocol_digest,
        )
        .expect("Ep closure instances");
        assert_eq!(eq.len(), COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1);
        assert_eq!(ep.len(), COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1);

        let mut wrong_request = closure.clone();
        wrong_request.request.request_id = [0xF1; 32];
        assert!(
            no_commit_closure_public_instances::<Fp>(
                &wrong_request,
                &eq_history,
                closure.proof.eq_protocol_digest,
                closure.proof.ep_protocol_digest,
            )
            .is_err()
        );
        let mut wrong_release = closure.clone();
        wrong_release.statement.release_id = [0xF2; 32];
        assert!(
            no_commit_closure_public_instances::<Fq>(
                &wrong_release,
                &ep_history,
                closure.proof.eq_protocol_digest,
                closure.proof.ep_protocol_digest,
            )
            .is_err()
        );
        let mut wrong_key = closure;
        wrong_key.statement.vk_digest = [0xF3; 32];
        assert!(
            no_commit_closure_public_instances::<Fp>(
                &wrong_key,
                &eq_history,
                wrong_key.proof.eq_protocol_digest,
                wrong_key.proof.ep_protocol_digest,
            )
            .is_err()
        );
    }
}
