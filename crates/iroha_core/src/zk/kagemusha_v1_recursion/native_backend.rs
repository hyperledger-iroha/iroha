//! Authenticated terminal verifier for the complete Kagemusha V1 recursive state relation.

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
    KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1 as PASTA_IPA_POSEIDON_FULL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1 as PASTA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_RATE_V1 as PASTA_IPA_POSEIDON_RATE_V1,
    KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1 as PASTA_IPA_POSEIDON_SECURE_MDS_V1,
    KAGEMUSHA_IPA_POSEIDON_WIDTH_V1 as PASTA_IPA_POSEIDON_WIDTH_V1,
    KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1, KagemushaArtifactByteResolverV1,
    KagemushaArtifactErrorV1, KagemushaAuthenticatedArtifactSetV1,
    KagemushaEpAccumulatorV1, KagemushaEqAccumulatorV1, KagemushaPastaParityV1,
    KagemushaRecursionArtifactsV1, KagemushaRecursiveVerifierV1,
    KagemushaStateProofVerificationRequestV1, KagemushaTransitionProofVerificationRequestV1,
    composite::{KagemushaRecursiveStateEpCircuitV1, KagemushaRecursiveStateEqCircuitV1},
    decide_kagemusha_ep_accumulator_v1, decide_kagemusha_eq_accumulator_v1,
    deferred_parent::{accumulator_limb_count, native_parent_protocol_digest_v1},
    guard_bundle::{
        GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1, KagemushaGuardBundleEpCircuitV1,
        KagemushaGuardBundleEqCircuitV1,
    },
    mint_authority::{
        KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1, KagemushaMintAuthorityCheckpointV1,
        KagemushaMintAuthorityEpCircuitV1, KagemushaMintAuthorityEqCircuitV1,
        KagemushaMintAuthorityPairBindingV1, public_instance as mint_public_instance,
    },
    mint_authorization::{
        MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1, KagemushaMintAuthorizationEpCircuitV1,
        KagemushaMintAuthorizationEqCircuitV1, mint_authorization_public_instances_v1,
    },
    mint_helper::KagemushaMintAuthorityStepV1,
    state_relation::PUBLIC_INSTANCE_COUNT,
    verify_kagemusha_state_proof_v1,
};
use crate::zk::kagemusha_v1_poseidon::{
    KagemushaPoseidonFieldV1, decode, digest_limbs, from_u128,
};
use crate::zk::kagemusha_v1_state::{KagemushaCandidateProofVerifierV1, PreparedOutgoingCandidateV1};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_HALO2_K_V1, KagemushaArtifactRoleV1, KagemushaMintAuthorizationV1,
    kagemusha_asset_identity_digest_v1, kagemusha_pasta_state_commitment_v1,
};

const RECURSIVE_PROFILE_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:paired-recursive-circuit-profile";
const RECURSIVE_PUBLIC_INSTANCE_COUNT_V1: usize = PUBLIC_INSTANCE_COUNT + accumulator_limb_count();

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

/// Exact `halo2-base` layouts used to decode all eight authenticated recursive verifying keys.
///
/// These values are covered by [`Self::canonical_digest`], which must equal the release
/// manifest's authenticated profile digest. They are explicit because processed Halo2 keys do
/// not safely self-describe a circuit configuration.
#[derive(Clone, Debug)]
pub struct KagemushaRecursiveVerifierProfileV1 {
    /// Eq aggregate-state recursive circuit layout.
    pub state_eq: BaseCircuitParams,
    /// Ep aggregate-state recursive circuit layout.
    pub state_ep: BaseCircuitParams,
    /// Eq GuardBundle recursive circuit layout.
    pub guard_eq: BaseCircuitParams,
    /// Ep GuardBundle recursive circuit layout.
    pub guard_ep: BaseCircuitParams,
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
            (1_u8, &self.state_eq),
            (2, &self.state_ep),
            (3, &self.guard_eq),
            (4, &self.guard_ep),
            (5, &self.mint_authorization_eq),
            (6, &self.mint_authorization_ep),
            (7, &self.mint_eq),
            (8, &self.mint_ep),
        ] {
            bytes.push(tag);
            append_base_params(&mut bytes, params)?;
        }
        bytes.push(9);
        bytes.extend_from_slice(&self.mint_eq_protocol_digest);
        bytes.push(10);
        bytes.extend_from_slice(&self.mint_ep_protocol_digest);
        bytes.push(11);
        bytes.extend_from_slice(&self.mint_genesis_roster_id);
        Ok(Sha256::digest(bytes).into())
    }

    fn validate(&self) -> Result<(), String> {
        for (label, params) in [
            ("state Eq", &self.state_eq),
            ("state Ep", &self.state_ep),
            ("GuardBundle Eq", &self.guard_eq),
            ("GuardBundle Ep", &self.guard_ep),
            ("mint authorization Eq", &self.mint_authorization_eq),
            ("mint authorization Ep", &self.mint_authorization_ep),
            ("mint authority Eq", &self.mint_eq),
            ("mint authority Ep", &self.mint_ep),
        ] {
            if params.k != KAGEMUSHA_HALO2_K_V1 as usize
                || params.num_instance_columns != 1
                || params.lookup_bits != Some((KAGEMUSHA_HALO2_K_V1 - 1) as usize)
                || params.num_advice_per_phase.is_empty()
                || params.num_lookup_advice_per_phase.is_empty()
                || params.num_advice_per_phase.iter().all(|value| *value == 0)
            {
                return Err(format!("invalid Kagemusha {label} circuit profile"));
            }
        }
        if self.mint_eq_protocol_digest == [0; 32]
            || self.mint_ep_protocol_digest == [0; 32]
            || self.mint_eq_protocol_digest == self.mint_ep_protocol_digest
            || decode::<Fp>(self.mint_eq_protocol_digest).is_none()
            || decode::<Fq>(self.mint_ep_protocol_digest).is_none()
            || self.mint_genesis_roster_id == [0; 32]
        {
            return Err("invalid Kagemusha finalized-mint protocol identities".to_owned());
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
    artifacts: KagemushaRecursionArtifactsV1,
    eq_parameters: halo2_proofs::poly::ipa::commitment::ParamsIPA<EqAffine>,
    ep_parameters: halo2_proofs::poly::ipa::commitment::ParamsIPA<EpAffine>,
    eq_state_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
    ep_state_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
    eq_mint_authorization_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
    ep_mint_authorization_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
    eq_mint_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
    ep_mint_protocol: snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
    guard_eq_protocol_digest: [u8; 32],
    guard_ep_protocol_digest: [u8; 32],
    mint_authorization_eq_protocol_digest: [u8; 32],
    mint_authorization_ep_protocol_digest: [u8; 32],
    mint_eq_protocol_digest: [u8; 32],
    mint_ep_protocol_digest: [u8; 32],
    mint_genesis_roster_id: [u8; 32],
    release_id: [u8; 32],
    artifact_manifest_digest: [u8; 32],
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
        let recursion = artifacts.recursion_artifacts();
        let profile_digest = profile
            .canonical_digest()
            .map_err(KagemushaArtifactErrorV1::InvalidRelease)?;
        if profile_digest != recursion.profile_digest {
            return Err(KagemushaArtifactErrorV1::InvalidRelease(
                "recursive circuit profile digest mismatch".to_owned(),
            ));
        }
        let eq_parameters = artifacts.load_eq_params()?;
        let ep_parameters = artifacts.load_ep_params()?;
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
        {
            return Err(KagemushaArtifactErrorV1::InvalidRelease(
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
        if mint_eq_protocol_digest != profile.mint_eq_protocol_digest
            || mint_ep_protocol_digest != profile.mint_ep_protocol_digest
        {
            return Err(KagemushaArtifactErrorV1::InvalidRelease(
                "compiled finalized-mint protocol identity mismatch".to_owned(),
            ));
        }
        let protocol_digests = [
            eq_protocol_digest,
            ep_protocol_digest,
            guard_eq_protocol_digest,
            guard_ep_protocol_digest,
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
            artifacts: recursion,
            eq_parameters,
            ep_parameters,
            eq_state_protocol,
            ep_state_protocol,
            eq_mint_authorization_protocol,
            ep_mint_authorization_protocol,
            eq_mint_protocol,
            ep_mint_protocol,
            eq_protocol_digest,
            ep_protocol_digest,
            guard_eq_protocol_digest,
            guard_ep_protocol_digest,
            mint_authorization_eq_protocol_digest,
            mint_authorization_ep_protocol_digest,
            mint_eq_protocol_digest,
            mint_ep_protocol_digest,
            mint_genesis_roster_id: profile.mint_genesis_roster_id,
            release_id: recursion.release_id,
            artifact_manifest_digest: recursion.artifact_manifest_digest,
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

    /// Verify and decide a release-pinned paired sender authorization before ticket issuance.
    ///
    /// This is a distinct CommitWrapper branch from terminal payment verification. Its public
    /// instances are reconstructed exclusively from the request and authorization statement;
    /// terminal lifecycle columns are never accepted as a substitute.
    #[cfg(any())]
    pub fn verify_acceptance_intent_authorization_and_decide(
        &self,
        release: &KagemushaAuthenticatedReleaseV1,
        request: &KagemushaPaymentRequestV1,
        authorization: &KagemushaAcceptanceIntentAuthorizationV1,
    ) -> Result<KagemushaAcceptanceIntentAuthorizationDecisionV1, String> {
        self.verify_acceptance_intent_authorization_against_loaded_release_and_decide(
            Some(release),
            request,
            authorization,
        )
    }

    #[cfg(any())]
    fn verify_acceptance_intent_authorization_against_loaded_release_and_decide(
        &self,
        release: Option<&KagemushaAuthenticatedReleaseV1>,
        request: &KagemushaPaymentRequestV1,
        authorization: &KagemushaAcceptanceIntentAuthorizationV1,
    ) -> Result<KagemushaAcceptanceIntentAuthorizationDecisionV1, String> {
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
                || release.artifact(KagemushaArtifactRoleV1::CommitWrapperVkEq)
                    != self.wrapper_eq_binding
                || release.artifact(KagemushaArtifactRoleV1::CommitWrapperVkEp)
                    != self.wrapper_ep_binding
        });
        if supplied_release_mismatch {
            return Err(
                "Kagemusha acceptance-intent authorization release/key-role mismatch".to_owned(),
            );
        }
        validate_commit_wrapper_authority_binding_v1(
            "acceptance-intent authorization",
            KagemushaCommitWrapperAuthorityBindingV1 {
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
        let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&proof.eq_history)
            .map_err(|error| error.to_string())?;
        let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&proof.ep_history)
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
        let eq_current = KagemushaEqAccumulatorV1::from_native(&verify_eq_succinct_protocol(
            &self.eq_parameters,
            &self.eq_wrapper_protocol,
            &proof.eq_proof,
            &eq_instances,
        )?)
        .map_err(|error| error.to_string())?;
        let ep_current = KagemushaEpAccumulatorV1::from_native(&verify_ep_succinct_protocol(
            &self.ep_parameters,
            &self.ep_wrapper_protocol,
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
            .map_err(|error| error.to_string())?;
        let request_digest = request
            .canonical_digest()
            .map_err(|error| error.to_string())?;
        let authorization_digest = authorization
            .canonical_digest_against(request)
            .map_err(|error| error.to_string())?;
        KagemushaAcceptanceIntentAuthorizationDecisionV1::authenticated(
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
    #[cfg(any())]
    pub fn verify_no_commit_closure_and_decide(
        &self,
        closure: &KagemushaNoCommitClosureV1,
    ) -> Result<KagemushaNoCommitClosureDecisionV1, String> {
        closure
            .validate_shape()
            .map_err(|error| error.to_string())?;
        let statement = &closure.statement;
        let proof = &closure.proof;
        validate_commit_wrapper_authority_binding_v1(
            "no-commit closure",
            KagemushaCommitWrapperAuthorityBindingV1 {
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
        let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&proof.eq_history)
            .map_err(|error| error.to_string())?;
        let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&proof.ep_history)
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
        let eq_current = KagemushaEqAccumulatorV1::from_native(&verify_eq_succinct_protocol(
            &self.eq_parameters,
            &self.eq_wrapper_protocol,
            &proof.eq_proof,
            &eq_instances,
        )?)
        .map_err(|error| error.to_string())?;
        let ep_current = KagemushaEpAccumulatorV1::from_native(&verify_ep_succinct_protocol(
            &self.ep_parameters,
            &self.ep_wrapper_protocol,
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
            .map_err(|error| error.to_string())?;
        KagemushaNoCommitClosureDecisionV1::authenticated(closure)
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
        if checkpoint.proof.eq_protocol_digest != self.mint_eq_protocol_digest
            || checkpoint.proof.ep_protocol_digest != self.mint_ep_protocol_digest
            || checkpoint.genesis_roster_id != self.mint_genesis_roster_id
        {
            return Err("Kagemusha mint-authority checkpoint release mismatch".to_owned());
        }
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

    #[cfg(any())]
    fn verify_request(
        &self,
        request: &KagemushaParityVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        if request.protocol_digest
            != match request.parity {
                KagemushaPastaParityV1::Eq => self.wrapper_eq_protocol_digest,
                KagemushaPastaParityV1::Ep => self.wrapper_ep_protocol_digest,
            }
        {
            return Err("Kagemusha commit-wrapper protocol identity mismatch".to_owned());
        }
        validate_proof_length("commit-wrapper", request.current_proof)?;
        match request.parity {
            KagemushaPastaParityV1::Eq => {
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
                let current = KagemushaEqAccumulatorV1::from_native(&current)
                    .map_err(|error| error.to_string())?;
                decide_kagemusha_eq_accumulator_v1(&self.eq_parameters, &current)
                    .map_err(|error| error.to_string())?;
                let history =
                    KagemushaEqAccumulatorV1::try_from_bytes(request.history_accumulator)
                        .map_err(|error| error.to_string())?;
                decide_kagemusha_eq_accumulator_v1(&self.eq_parameters, &history)
                    .map_err(|error| error.to_string())
            }
            KagemushaPastaParityV1::Ep => {
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
                let current = KagemushaEpAccumulatorV1::from_native(&current)
                    .map_err(|error| error.to_string())?;
                decide_kagemusha_ep_accumulator_v1(&self.ep_parameters, &current)
                    .map_err(|error| error.to_string())?;
                let history =
                    KagemushaEpAccumulatorV1::try_from_bytes(request.history_accumulator)
                        .map_err(|error| error.to_string())?;
                decide_kagemusha_ep_accumulator_v1(&self.ep_parameters, &history)
                    .map_err(|error| error.to_string())
            }
        }
    }
}

#[cfg(any())]
impl KagemushaAcceptanceIntentAuthorizationVerifierV1
    for KagemushaAuthenticatedRecursiveVerifierV1
{
    fn verify_acceptance_intent_authorization(
        &self,
        release: &KagemushaAuthenticatedReleaseV1,
        request: &KagemushaPaymentRequestV1,
        authorization: &KagemushaAcceptanceIntentAuthorizationV1,
    ) -> Result<KagemushaAcceptanceIntentAuthorizationDecisionV1, String> {
        self.verify_acceptance_intent_authorization_and_decide(release, request, authorization)
    }
}

#[cfg(any())]
impl KagemushaNoCommitClosureVerifierV1 for KagemushaAuthenticatedRecursiveVerifierV1 {
    fn verify_no_commit_closure(
        &self,
        closure: &KagemushaNoCommitClosureV1,
    ) -> Result<KagemushaNoCommitClosureDecisionV1, String> {
        self.verify_no_commit_closure_and_decide(closure)
    }
}

impl KagemushaCandidateProofVerifierV1 for KagemushaAuthenticatedRecursiveVerifierV1 {
    fn verify_candidate_proof(
        &self,
        candidate: &PreparedOutgoingCandidateV1,
        proof: &iroha_data_model::kagemusha::KagemushaPairedProofV1,
    ) -> Result<(), String> {
        let public_inputs = candidate.candidate_public_inputs(self.artifacts, proof)?;
        verify_kagemusha_state_proof_v1(self, self.artifacts, &public_inputs, proof)
            .map_err(|error| error.to_string())
    }
}

#[cfg(any())]
impl KagemushaCommitWrapperVerifierV1 for KagemushaAuthenticatedRecursiveVerifierV1 {
    fn verify_commit_wrapper(
        &self,
        public_inputs: &KagemushaTerminalPublicInputsV1,
        proof: &iroha_data_model::kagemusha::KagemushaCommitWrapperProofV1,
    ) -> Result<(), String> {
        verify_kagemusha_recursive_proof_v1(self, self.artifacts, public_inputs.clone(), proof)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

impl KagemushaRecursiveVerifierV1 for KagemushaAuthenticatedRecursiveVerifierV1 {
    fn verify_state_proof_and_decide(
        &self,
        request: &KagemushaStateProofVerificationRequestV1<'_>,
    ) -> Result<(), String> {
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
        if request.eq_protocol_digest != self.mint_eq_protocol_digest
            || request.ep_protocol_digest != self.mint_ep_protocol_digest
            || request.finality_genesis_roster_id != self.mint_genesis_roster_id
        {
            return Err("Kagemusha mint-authority release binding mismatch".to_owned());
        }
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

    fn verify_transition_proof_and_decide(
        &self,
        request: &KagemushaTransitionProofVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        if request.artifacts != self.artifacts
            || request.proof.eq_protocol_digest != self.eq_protocol_digest
            || request.proof.ep_protocol_digest != self.ep_protocol_digest
            || request.public_output.lifecycle.release_id != self.release_id
        {
            return Err("Kagemusha transition proof release binding mismatch".to_owned());
        }
        validate_proof_length("Eq aggregate transition", &request.proof.eq_proof)?;
        validate_proof_length("Ep aggregate transition", &request.proof.ep_proof)?;
        let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&request.proof.eq_history)
            .map_err(|error| error.to_string())?;
        let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&request.proof.ep_history)
            .map_err(|error| error.to_string())?;
        let mut eq_instances = transition_public_instances::<Fp>(request)?;
        eq_instances.extend(history_public_instances::<Fp>(eq_history.as_bytes()));
        let mut ep_instances = transition_public_instances::<Fq>(request)?;
        ep_instances.extend(history_public_instances::<Fq>(ep_history.as_bytes()));
        if eq_instances.len() != RECURSIVE_PUBLIC_INSTANCE_COUNT_V1
            || ep_instances.len() != RECURSIVE_PUBLIC_INSTANCE_COUNT_V1
        {
            return Err("Kagemusha transition public instance ABI mismatch".to_owned());
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
    read_eq_recursive_vk::<KagemushaRecursiveStateEqCircuitV1>(bytes, params, "state")
}

fn read_ep_state_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactErrorV1> {
    read_ep_recursive_vk::<KagemushaRecursiveStateEpCircuitV1>(bytes, params, "state")
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

#[cfg(any())]
fn read_eq_wrapper_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactErrorV1> {
    read_eq_recursive_vk::<KagemushaCommitWrapperEqCircuitV1>(bytes, params, "commit wrapper")
}

#[cfg(any())]
fn read_ep_wrapper_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactErrorV1> {
    read_ep_recursive_vk::<KagemushaCommitWrapperEpCircuitV1>(bytes, params, "commit wrapper")
}

fn read_eq_mint_authorization_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactErrorV1> {
    read_eq_recursive_vk::<KagemushaMintAuthorizationEqCircuitV1>(
        bytes,
        params,
        "mint authorization",
    )
}

fn read_ep_mint_authorization_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactErrorV1> {
    read_ep_recursive_vk::<KagemushaMintAuthorizationEpCircuitV1>(
        bytes,
        params,
        "mint authorization",
    )
}

fn read_eq_mint_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactErrorV1> {
    read_eq_recursive_vk::<KagemushaMintAuthorityEqCircuitV1>(bytes, params, "mint authority")
}

fn read_ep_mint_vk(
    bytes: &[u8],
    params: BaseCircuitParams,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactErrorV1> {
    read_ep_recursive_vk::<KagemushaMintAuthorityEpCircuitV1>(bytes, params, "mint authority")
}

fn read_eq_recursive_vk<C>(
    bytes: &[u8],
    params: BaseCircuitParams,
    label: &str,
) -> Result<VerifyingKey<EqAffine>, KagemushaArtifactErrorV1>
where
    C: halo2_proofs::plonk::Circuit<Fp, Params = BaseCircuitParams>,
{
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read::<_, C>(&mut cursor, SerdeFormat::Processed, params).map_err(
        |error| {
            KagemushaArtifactErrorV1::InvalidRelease(format!(
                "failed to decode Eq {label} verifying key: {error}"
            ))
        },
    )?;
    if cursor.position() != bytes.len() as u64 {
        return Err(KagemushaArtifactErrorV1::InvalidRelease(format!(
            "Eq {label} verifying key has trailing bytes"
        )));
    }
    Ok(key)
}

fn read_ep_recursive_vk<C>(
    bytes: &[u8],
    params: BaseCircuitParams,
    label: &str,
) -> Result<VerifyingKey<EpAffine>, KagemushaArtifactErrorV1>
where
    C: halo2_proofs::plonk::Circuit<Fq, Params = BaseCircuitParams>,
{
    let mut cursor = Cursor::new(bytes);
    let key = VerifyingKey::read::<_, C>(&mut cursor, SerdeFormat::Processed, params).map_err(
        |error| {
            KagemushaArtifactErrorV1::InvalidRelease(format!(
                "failed to decode Ep {label} verifying key: {error}"
            ))
        },
    )?;
    if cursor.position() != bytes.len() as u64 {
        return Err(KagemushaArtifactErrorV1::InvalidRelease(format!(
            "Ep {label} verifying key has trailing bytes"
        )));
    }
    Ok(key)
}

#[cfg(any())]
fn wrapper_public_instances<F: KagemushaPoseidonFieldV1>(
    request: &KagemushaParityVerificationRequestV1<'_>,
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
) -> Result<Vec<F>, String> {
    let wrapper = KagemushaCommitWrapperPublicInputsV1::from_lifecycle(
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
        return Err("Kagemusha commit-wrapper public instance ABI mismatch".to_owned());
    }
    Ok(public)
}

#[cfg(any())]
fn acceptance_intent_authorization_public_instances<F: KagemushaPoseidonFieldV1>(
    request: &KagemushaPaymentRequestV1,
    authorization: &KagemushaAcceptanceIntentAuthorizationV1,
    history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
) -> Result<Vec<F>, String> {
    let wrapper = KagemushaCommitWrapperPublicInputsV1::from_acceptance_intent_authorization(
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
        return Err("Kagemusha acceptance-intent public instance ABI mismatch".to_owned());
    }
    Ok(public)
}

#[cfg(any())]
fn no_commit_closure_public_instances<F: KagemushaPoseidonFieldV1>(
    closure: &KagemushaNoCommitClosureV1,
    history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
) -> Result<Vec<F>, String> {
    closure
        .validate_shape()
        .map_err(|error| error.to_string())?;
    let wrapper = KagemushaCommitWrapperPublicInputsV1::from_no_commit_closure(
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
        return Err("Kagemusha no-commit closure public instance ABI mismatch".to_owned());
    }
    Ok(public)
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

fn transition_public_instances<F: KagemushaPoseidonFieldV1>(
    request: &KagemushaTransitionProofVerificationRequestV1<'_>,
) -> Result<Vec<F>, String> {
    let output = request.public_output;
    let proof = request.proof;
    let artifacts = request.artifacts;
    let lifecycle = &output.lifecycle;
    let operation_tag = match output.operation() {
        super::KagemushaOperationV1::SendSplit => 2,
        super::KagemushaOperationV1::RedeemSplit => 4,
        _ => return Err("Kagemusha terminal proof selected a non-terminal operation".to_owned()),
    };
    let predecessor = decode::<F>(F::select_component(output.sender_before_commitment))
        .ok_or_else(|| "noncanonical predecessor Pasta commitment".to_owned())?;
    let successor = decode::<F>(F::select_component(output.sender_after_commitment))
        .ok_or_else(|| "noncanonical successor Pasta commitment".to_owned())?;
    let predecessor_outer = digest_limbs::<F>(kagemusha_pasta_state_commitment_v1(
        output.sender_before_commitment,
    ));
    let successor_outer = digest_limbs::<F>(kagemusha_pasta_state_commitment_v1(
        output.sender_after_commitment,
    ));
    let predecessor_eq = digest_limbs::<F>(output.sender_before_commitment.eq);
    let predecessor_ep = digest_limbs::<F>(output.sender_before_commitment.ep);
    let successor_eq = digest_limbs::<F>(output.sender_after_commitment.eq);
    let successor_ep = digest_limbs::<F>(output.sender_after_commitment.ep);
    let transport = digest_limbs::<F>(output.semantic_digest);
    let guard = digest_limbs::<F>(output.hardware_transition_commitment);
    let release = digest_limbs::<F>(lifecycle.release_id);
    let liability_pool = digest_limbs::<F>(lifecycle.liability_pool_id);
    let peer_credit = digest_limbs::<F>(if output.operation()
        == super::KagemushaOperationV1::SendSplit
    {
        lifecycle.credit_id
    } else {
        [0; 32]
    });
    let peer_recipient_lane = digest_limbs::<F>(output.recipient_lane_id);
    let eq_protocol = digest_limbs::<F>(proof.eq_protocol_digest);
    let ep_protocol = digest_limbs::<F>(proof.ep_protocol_digest);
    let guard_eq_protocol = digest_limbs::<F>(artifacts.guard_eq_protocol_digest);
    let guard_ep_protocol = digest_limbs::<F>(artifacts.guard_ep_protocol_digest);
    let mint_eq_protocol = digest_limbs::<F>(
        artifacts
            .mint_finality_protocol_digest(KagemushaPastaParityV1::Eq)
            .map_err(|error| error.to_string())?,
    );
    let mint_ep_protocol = digest_limbs::<F>(
        artifacts
            .mint_finality_protocol_digest(KagemushaPastaParityV1::Ep)
            .map_err(|error| error.to_string())?,
    );
    let guard_eq_audit = digest_limbs::<F>(proof.guard_eq_credential_audit);
    let guard_ep_audit = digest_limbs::<F>(proof.guard_ep_credential_audit);
    let eq_audit = digest_limbs::<F>(proof.eq_deferred_audit);
    let ep_audit = digest_limbs::<F>(proof.ep_deferred_audit);
    let lifecycle_digest = digest_limbs::<F>(
        lifecycle
            .canonical_digest()
            .map_err(|error| error.to_string())?,
    );
    let precommit = digest_limbs::<F>(
        output
            .precommit_binding_digest()
            .map_err(|error| error.to_string())?,
    );
    let suite = digest_limbs::<F>(lifecycle.suite_id);
    let vk = digest_limbs::<F>(lifecycle.vk_digest);
    let asset_incarnation = digest_limbs::<F>(*lifecycle.asset_incarnation.as_bytes());
    let hardware_profile = digest_limbs::<F>(lifecycle.hardware_profile_id);
    let network = digest_limbs::<F>(*lifecycle.network_id.as_bytes());
    let asset = digest_limbs::<F>(
        kagemusha_asset_identity_digest_v1(&lifecycle.asset).map_err(|error| error.to_string())?,
    );
    let zero = [F::ZERO; 2];
    let instances = vec![
        F::from(operation_tag),
        from_u128(output.amount),
        transport[0],
        transport[1],
        guard[0],
        guard[1],
        predecessor_outer[0],
        predecessor_outer[1],
        successor_outer[0],
        successor_outer[1],
        zero[0],
        zero[1],
        release[0],
        release[1],
        liability_pool[0],
        liability_pool[1],
        peer_credit[0],
        peer_credit[1],
        peer_recipient_lane[0],
        peer_recipient_lane[1],
        predecessor_eq[0],
        predecessor_eq[1],
        predecessor_ep[0],
        predecessor_ep[1],
        successor_eq[0],
        successor_eq[1],
        successor_ep[0],
        successor_ep[1],
        predecessor,
        successor,
        eq_protocol[0],
        eq_protocol[1],
        ep_protocol[0],
        ep_protocol[1],
        guard_eq_protocol[0],
        guard_eq_protocol[1],
        guard_ep_protocol[0],
        guard_ep_protocol[1],
        mint_eq_protocol[0],
        mint_eq_protocol[1],
        mint_ep_protocol[0],
        mint_ep_protocol[1],
        guard_eq_audit[0],
        guard_eq_audit[1],
        guard_ep_audit[0],
        guard_ep_audit[1],
        eq_audit[0],
        eq_audit[1],
        ep_audit[0],
        ep_audit[1],
        zero[0],
        zero[1],
        lifecycle_digest[0],
        lifecycle_digest[1],
        precommit[0],
        precommit[1],
        suite[0],
        suite[1],
        vk[0],
        vk[1],
        suite[0],
        suite[1],
        vk[0],
        vk[1],
        zero[0],
        zero[1],
        F::from(u64::from(lifecycle.protocol_version)),
        asset_incarnation[0],
        asset_incarnation[1],
        hardware_profile[0],
        hardware_profile[1],
        F::from(lifecycle.policy_epoch),
        network[0],
        network[1],
        asset[0],
        asset[1],
        F::from(u64::from(lifecycle.scale)),
    ];
    if instances.len() != PUBLIC_INSTANCE_COUNT {
        return Err("Kagemusha direct transition public instance ABI mismatch".to_owned());
    }
    Ok(instances)
}

fn mint_public_instances<F: KagemushaPoseidonFieldV1>(
    request: &super::KagemushaMintFinalityHelperVerificationRequestV1<'_>,
    history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
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
        return Err("Kagemusha mint-authority semantic binding mismatch".to_owned());
    }
    let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&request.proof.eq_history)
        .map_err(|error| error.to_string())?;
    let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&request.proof.ep_history)
        .map_err(|error| error.to_string())?;
    let pair_binding = KagemushaMintAuthorityPairBindingV1 {
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
        eq_history: eq_history.as_bytes(),
        ep_history: ep_history.as_bytes(),
    }
    .canonical_digest();
    if pair_binding != request.finality_proof_binding_digest {
        return Err("Kagemusha mint-authority paired binding mismatch".to_owned());
    }
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
            proof_binding_digest: pair_binding,
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
    .map_err(|_| "Kagemusha Eq proof parser panicked".to_owned())?
    .map_err(|error| format!("invalid Kagemusha Eq proof: {error:?}"))?;
    let accumulators = crate::panic_hook::catch_unwind_suppressed(|| {
        PlonkSuccinctVerifier::<Scheme>::verify(&svk, &protocol, &instance_columns, &parsed)
    })
    .map_err(|_| "Kagemusha Eq verifier panicked".to_owned())?
    .map_err(|error| format!("Kagemusha Eq proof rejected: {error:?}"))?;
    drop(transcript);
    if cursor.position() != proof.len() as u64 {
        return Err("Kagemusha Eq proof has trailing bytes".to_owned());
    }
    let [accumulator] = accumulators.try_into().map_err(|values: Vec<_>| {
        format!(
            "Kagemusha Eq proof returned {} accumulators",
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
    .map_err(|_| "Kagemusha Ep proof parser panicked".to_owned())?
    .map_err(|error| format!("invalid Kagemusha Ep proof: {error:?}"))?;
    let accumulators = crate::panic_hook::catch_unwind_suppressed(|| {
        PlonkSuccinctVerifier::<Scheme>::verify(&svk, &protocol, &instance_columns, &parsed)
    })
    .map_err(|_| "Kagemusha Ep verifier panicked".to_owned())?
    .map_err(|error| format!("Kagemusha Ep proof rejected: {error:?}"))?;
    drop(transcript);
    if cursor.position() != proof.len() as u64 {
        return Err("Kagemusha Ep proof has trailing bytes".to_owned());
    }
    let [accumulator] = accumulators.try_into().map_err(|values: Vec<_>| {
        format!(
            "Kagemusha Ep proof returned {} accumulators",
            values.len()
        )
    })?;
    Ok(accumulator)
}

#[cfg(any())]
mod tests {
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        NetworkId,
        account::AccountId,
        asset::AssetDefinitionId,
        block::BlockHeader,
        domain::DomainId,
        nexus::AxtAssetIncarnationV1,
        kagemusha::{
            KAGEMUSHA_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1,
            KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KAGEMUSHA_WIRE_VERSION_V1,
            KagemushaAcceptanceIntentAuthorizationStatementV1, KagemushaAcceptanceIntentV1,
            KagemushaAcceptanceTicketV1, KagemushaDevicePublicKeyV1,
            KagemushaDeviceSignatureV1, KagemushaHardwareCredentialV1,
            KagemushaNoCommitClosureStatementV1, KagemushaNoCommitClosureV1,
            KagemushaPairedProofV1, kagemusha_device_key_reference_v1,
            kagemusha_liability_pool_id_v1,
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

    fn sign(key: &SigningKey, bytes: &[u8]) -> KagemushaDeviceSignatureV1 {
        let signature: Signature = key.sign(bytes);
        let signature = signature.normalize_s().unwrap_or(signature);
        KagemushaDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
            .expect("canonical test signature")
    }

    fn acceptance_authorization_fixture() -> (
        KagemushaPaymentRequestV1,
        KagemushaAcceptanceIntentAuthorizationV1,
    ) {
        let key = signing_key();
        let device_public_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
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
        let mut credential = KagemushaHardwareCredentialV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
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
            device_key_reference: kagemusha_device_key_reference_v1(&device_public_key),
            issued_at_ms: 1,
            expires_at_ms: 100_000,
            governance_signature: sign(&key, b"shape-only governance signature"),
        }
        .seal_credential_id()
        .expect("credential id");
        credential.governance_signature = sign(&key, b"shape-only governance signature");
        let mut request = KagemushaPaymentRequestV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            release_id: [0x41; 32],
            network_id,
            asset: asset.clone(),
            asset_incarnation,
            scale: 2,
            liability_pool_id: kagemusha_liability_pool_id_v1(
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
            amount: 7,
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
        let intent = KagemushaAcceptanceIntentV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            request_digest: request.canonical_digest().expect("request digest"),
            intent_id: [0x46; 32],
            exact_amount: 7,
            sender_one_time_commitment: [0x47; 32],
        };
        let statement = KagemushaAcceptanceIntentAuthorizationStatementV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            intent,
            release_id: request.release_id,
            suite_id: request.hardware_credential.suite_id,
            vk_digest: [0x48; 32],
            artifact_manifest_digest: [0x49; 32],
        };
        let semantic_digest = statement
            .canonical_digest_against(&request)
            .expect("authorization statement digest");
        let authorization = KagemushaAcceptanceIntentAuthorizationV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            statement,
            proof: iroha_data_model::kagemusha::KagemushaPairedProofV1 {
                version: KAGEMUSHA_WIRE_VERSION_V1,
                eq_protocol_digest: [0x51; 32],
                ep_protocol_digest: [0x52; 32],
                semantic_digest,
                guard_eq_credential_audit: [0x53; 32],
                guard_ep_credential_audit: [0x54; 32],
                eq_deferred_audit: [0x55; 32],
                ep_deferred_audit: [0x56; 32],
                eq_proof: vec![0x57],
                ep_proof: vec![0x58],
                eq_history: vec![0x59; super::super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
                ep_history: vec![0x5A; super::super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
            },
        };
        authorization
            .validate_shape_against(&request)
            .expect("authorization shape");
        (request, authorization)
    }

    fn no_commit_closure_fixture() -> KagemushaNoCommitClosureV1 {
        let (request, intent_authorization) = acceptance_authorization_fixture();
        let key = signing_key();
        let intent = intent_authorization.statement.intent;
        let mut acceptance_ticket = KagemushaAcceptanceTicketV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            network_id: request.network_id,
            request_id: request.request_id,
            request_digest: request.canonical_digest().expect("request digest"),
            acceptance_ticket_id: [0x61; 32],
            asset: request.asset.clone(),
            asset_incarnation: request.asset_incarnation,
            scale: request.scale,
            intent_digest: intent
                .canonical_digest_against(&request)
                .expect("intent digest"),
            exact_amount: intent.exact_amount,
            reserved_inbox_bytes: KAGEMUSHA_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1,
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
        let statement = KagemushaNoCommitClosureStatementV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
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
        let closure = KagemushaNoCommitClosureV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            statement,
            request,
            intent_authorization,
            acceptance_ticket,
            proof: KagemushaPairedProofV1 {
                version: KAGEMUSHA_WIRE_VERSION_V1,
                eq_protocol_digest: [0x51; 32],
                ep_protocol_digest: [0x52; 32],
                semantic_digest,
                guard_eq_credential_audit: [0x67; 32],
                guard_ep_credential_audit: [0x68; 32],
                eq_deferred_audit: [0x69; 32],
                ep_deferred_audit: [0x6A; 32],
                eq_proof: vec![0x6B],
                ep_proof: vec![0x6C],
                eq_history: vec![0x6D; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
                ep_history: vec![0x6E; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
            },
        };
        closure.validate_shape().expect("closure shape");
        closure
    }

    #[test]
    fn ordinary_proof_length_is_strictly_bounded() {
        assert!(validate_proof_length("Eq", &[]).is_err());
        assert!(validate_proof_length("Eq", &[0; 1]).is_ok());
        assert!(validate_proof_length("Ep", &[0; KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1]).is_ok());
        assert!(
            validate_proof_length("Ep", &[0; KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1 + 1]).is_err()
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
        let expected = KagemushaCommitWrapperAuthorityBindingV1 {
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
            KagemushaArtifactRoleV1::CommitWrapperVkEq,
            KagemushaArtifactRoleV1::CommitWrapperVkEp,
        )
        .expect("exact authenticated suite binding");
        let actual = KagemushaCommitWrapperAuthorityBindingV1 {
            suite_id: [0xFE; 32],
            ..expected
        };
        assert!(
            validate_commit_wrapper_authority_binding_v1(
                "acceptance-intent authorization",
                actual,
                expected,
                KagemushaArtifactRoleV1::CommitWrapperVkEq,
                KagemushaArtifactRoleV1::CommitWrapperVkEp,
            )
            .is_err()
        );
    }

    #[test]
    fn no_commit_closure_rejects_release_suite_substitution() {
        let closure = no_commit_closure_fixture();
        let expected = KagemushaCommitWrapperAuthorityBindingV1 {
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
            KagemushaArtifactRoleV1::CommitWrapperVkEq,
            KagemushaArtifactRoleV1::CommitWrapperVkEp,
        )
        .expect("exact authenticated suite binding");
        let actual = KagemushaCommitWrapperAuthorityBindingV1 {
            suite_id: [0xFD; 32],
            ..expected
        };
        assert!(
            validate_commit_wrapper_authority_binding_v1(
                "no-commit closure",
                actual,
                expected,
                KagemushaArtifactRoleV1::CommitWrapperVkEq,
                KagemushaArtifactRoleV1::CommitWrapperVkEp,
            )
            .is_err()
        );
    }

    #[test]
    fn acceptance_authorization_uses_its_exact_public_branch_for_both_parities() {
        let (request, authorization) = acceptance_authorization_fixture();
        let eq_history: [u8; super::super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1] =
            authorization
                .proof
                .eq_history
                .clone()
                .try_into()
                .expect("Eq history");
        let ep_history: [u8; super::super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1] =
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
        let eq_history: [u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1] = closure
            .proof
            .eq_history
            .clone()
            .try_into()
            .expect("Eq history");
        let ep_history: [u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1] = closure
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
