//! Native first-release ZK-ACE authorization engine.
//!
//! The privacy runtime uses the STARK/FRI envelope directly.  It does not route
//! through the generic verifier-key registry, accept a caller-selected backend,
//! or preserve the historical `ProofAttachment` wrapper.  The exact circuit,
//! SHA-256/Goldilocks parameters, transcript label, domain tag, and proof-size
//! ceiling are compiled below and checked before the algebraic verifier runs.

use iroha_data_model::zk::{
    ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID, ZkAcePublicInputsV1, ZkAceWitnessV1,
    derive_zk_ace_air_public_digest,
};
#[cfg(test)]
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::zk_stark::{
    STARK_HASH_SHA256_V1, StarkFriParamsV1, StarkVerifierLimits, StarkVerifyEnvelopeV1,
    ZK_ACE_STARK_FRI_V1_BLOWUP_LOG2, ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES,
    ZK_ACE_STARK_FRI_V1_N_LOG2, ZK_ACE_STARK_FRI_V1_QUERIES,
    prove_stark_fri_zk_ace_air_envelope_bytes, verify_stark_fri_zk_ace_envelope_with_limits,
};

/// Fixed transcript label for the native privacy ZK-ACE verifier.
pub const ZK_ACE_PRIVACY_TRANSCRIPT_LABEL_V1: &str = "IROHA-PRIVACY-ZK-ACE-AIR-V1";
/// Fixed FRI domain tag for the native privacy ZK-ACE verifier.
pub const ZK_ACE_PRIVACY_FRI_DOMAIN_TAG_V1: &str = "iroha:privacy:zk-ace:fri:v1";
/// Source and relation description frozen into the compiled profile.
pub const ZK_ACE_SOURCE_PROFILE_V1: &[u8] =
    b"iroha-native-rust:zk-ace:private-witness-air:sha256-goldilocks:v1";
/// Exact native proof wire description frozen into the compiled profile.
pub const ZK_ACE_PROOF_WIRE_V1: &[u8] =
    b"norito:stark-verify-envelope-v1:strict-exact:no-openverify-wrapper";
/// Frozen digest of every compiled verifier-profile field below.
pub const ZK_ACE_COMPILED_PROFILE_DIGEST_V1: [u8; 32] = [
    0xfb, 0xf0, 0xcb, 0xdc, 0xe9, 0x99, 0x9e, 0x1d, 0x08, 0x52, 0xf5, 0x88, 0xc6, 0x0d, 0xab, 0x9b,
    0xb5, 0xeb, 0xcd, 0xdc, 0xed, 0xa1, 0xf1, 0x17, 0x96, 0x2a, 0xf4, 0x51, 0xca, 0x45, 0x49, 0x70,
];

/// Return the exact first-release STARK/FRI parameters.
#[must_use]
pub fn zk_ace_privacy_stark_params_v1() -> StarkFriParamsV1 {
    StarkFriParamsV1 {
        version: 1,
        n_log2: ZK_ACE_STARK_FRI_V1_N_LOG2,
        blowup_log2: ZK_ACE_STARK_FRI_V1_BLOWUP_LOG2,
        fold_arity: 2,
        queries: ZK_ACE_STARK_FRI_V1_QUERIES,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
        domain_tag: ZK_ACE_PRIVACY_FRI_DOMAIN_TAG_V1.to_owned(),
    }
}

/// Return the frozen digest of the exact compiled native verifier profile.
#[must_use]
pub const fn zk_ace_compiled_profile_digest_v1() -> [u8; 32] {
    ZK_ACE_COMPILED_PROFILE_DIGEST_V1
}

#[cfg(test)]
fn recompute_zk_ace_compiled_profile_digest_v1() -> [u8; 32] {
    let params = zk_ace_privacy_stark_params_v1();
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"iroha.privacy.zk-ace.compiled-profile.v1");
    hash_field(&mut hasher, ZK_ACE_SOURCE_PROFILE_V1);
    hash_field(&mut hasher, ZK_ACE_PROOF_WIRE_V1);
    hash_field(
        &mut hasher,
        ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID.as_bytes(),
    );
    hash_field(&mut hasher, ZK_ACE_PRIVACY_TRANSCRIPT_LABEL_V1.as_bytes());
    hash_field(&mut hasher, params.domain_tag.as_bytes());
    hash_field(&mut hasher, &params.version.to_be_bytes());
    hash_field(
        &mut hasher,
        &[
            params.n_log2,
            params.blowup_log2,
            params.fold_arity,
            params.merkle_arity,
            params.hash_fn,
        ],
    );
    hash_field(&mut hasher, &params.queries.to_be_bytes());
    hash_field(
        &mut hasher,
        &ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES.to_be_bytes(),
    );
    hasher.finalize().into()
}

#[cfg(test)]
fn hash_field(hasher: &mut Sha256, field: &[u8]) {
    hasher.update(
        u64::try_from(field.len())
            .expect("fixed ZK-ACE profile field length fits u64")
            .to_be_bytes(),
    );
    hasher.update(field);
}

/// Generate the strict native privacy proof bytes for exact public inputs.
///
/// # Errors
///
/// Returns a typed error when public-input hashing or native proof generation
/// fails, or if an internal prover result does not satisfy the compiled wire
/// and size contract.
pub fn prove_zk_ace_privacy_v1(
    public_inputs: &ZkAcePublicInputsV1,
    witness: &ZkAceWitnessV1,
) -> Result<Vec<u8>, ZkAceNativeErrorV1> {
    let public_digest = derive_zk_ace_air_public_digest(public_inputs)
        .map_err(|_| ZkAceNativeErrorV1::PublicInputsEncoding)?;
    let proof = prove_stark_fri_zk_ace_air_envelope_bytes(
        zk_ace_privacy_stark_params_v1(),
        ZK_ACE_PRIVACY_TRANSCRIPT_LABEL_V1.to_owned(),
        ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID.to_owned(),
        public_digest,
        public_inputs,
        witness,
    )
    .map_err(ZkAceNativeErrorV1::Prover)?;
    verify_zk_ace_privacy_v1(public_inputs, &proof, ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES)?;
    Ok(proof)
}

/// Verify strict native privacy proof bytes against exact public inputs.
///
/// # Errors
///
/// Rejects an empty, oversized, malformed, non-canonical, parameter-substituted,
/// transcript-substituted, or algebraically invalid proof.
pub fn verify_zk_ace_privacy_v1(
    public_inputs: &ZkAcePublicInputsV1,
    proof: &[u8],
    caller_max_proof_bytes: u32,
) -> Result<(), ZkAceNativeErrorV1> {
    let effective_cap = caller_max_proof_bytes.min(ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES);
    let proof_len = u32::try_from(proof.len()).map_err(|_| ZkAceNativeErrorV1::ProofTooLarge {
        bytes: u64::try_from(proof.len()).unwrap_or(u64::MAX),
        max: effective_cap,
    })?;
    if proof_len == 0 {
        return Err(ZkAceNativeErrorV1::EmptyProof);
    }
    if proof_len > effective_cap {
        return Err(ZkAceNativeErrorV1::ProofTooLarge {
            bytes: u64::from(proof_len),
            max: effective_cap,
        });
    }

    let envelope: StarkVerifyEnvelopeV1 =
        norito::decode_from_bytes(proof).map_err(|_| ZkAceNativeErrorV1::MalformedProof)?;
    let canonical = norito::to_bytes(&envelope).map_err(|_| ZkAceNativeErrorV1::MalformedProof)?;
    if canonical.as_slice() != proof {
        return Err(ZkAceNativeErrorV1::NonCanonicalProof);
    }
    validate_exact_envelope_profile(&envelope)?;

    let mut limits = StarkVerifierLimits::default();
    limits.max_domain_log2 = ZK_ACE_STARK_FRI_V1_N_LOG2;
    limits.max_blowup_log2 = ZK_ACE_STARK_FRI_V1_BLOWUP_LOG2;
    limits.max_fold_arity = 2;
    limits.max_queries = usize::from(ZK_ACE_STARK_FRI_V1_QUERIES);
    limits.max_merkle_depth = usize::from(ZK_ACE_STARK_FRI_V1_N_LOG2);
    limits.max_aux_terms = 0;
    limits.max_domain_tag_len = ZK_ACE_PRIVACY_FRI_DOMAIN_TAG_V1.len();
    limits.max_transcript_label_len = ZK_ACE_PRIVACY_TRANSCRIPT_LABEL_V1.len();
    limits.max_envelope_bytes = usize::try_from(effective_cap)
        .expect("u32 proof-size ceiling fits usize on supported targets");
    if !verify_stark_fri_zk_ace_envelope_with_limits(proof, &limits, public_inputs) {
        return Err(ZkAceNativeErrorV1::VerificationFailed);
    }
    Ok(())
}

fn validate_exact_envelope_profile(
    envelope: &StarkVerifyEnvelopeV1,
) -> Result<(), ZkAceNativeErrorV1> {
    let expected = zk_ace_privacy_stark_params_v1();
    let actual = &envelope.params;
    if actual.version != expected.version
        || actual.n_log2 != expected.n_log2
        || actual.blowup_log2 != expected.blowup_log2
        || actual.fold_arity != expected.fold_arity
        || actual.queries != expected.queries
        || actual.merkle_arity != expected.merkle_arity
        || actual.hash_fn != expected.hash_fn
        || actual.domain_tag != expected.domain_tag
    {
        return Err(ZkAceNativeErrorV1::ParameterMismatch);
    }
    if envelope.transcript_label != ZK_ACE_PRIVACY_TRANSCRIPT_LABEL_V1 {
        return Err(ZkAceNativeErrorV1::TranscriptMismatch);
    }
    if envelope.proof.commits.comp_root.is_some() || envelope.proof.comp_values.is_some() {
        return Err(ZkAceNativeErrorV1::AuxiliaryCompositionForbidden);
    }
    Ok(())
}

/// Native ZK-ACE proof construction or verification failure.
#[derive(Debug, Error)]
pub enum ZkAceNativeErrorV1 {
    /// Canonical public-input encoding failed.
    #[error("ZK-ACE public inputs could not be encoded canonically")]
    PublicInputsEncoding,
    /// The proof payload is empty.
    #[error("ZK-ACE proof must not be empty")]
    EmptyProof,
    /// The proof payload exceeds the effective native or caller ceiling.
    #[error("ZK-ACE proof is too large: {bytes} bytes exceeds {max}")]
    ProofTooLarge {
        /// Observed proof length.
        bytes: u64,
        /// Effective proof ceiling.
        max: u32,
    },
    /// Norito decoding failed.
    #[error("ZK-ACE proof is malformed")]
    MalformedProof,
    /// Decoding and canonical re-encoding changed the byte string.
    #[error("ZK-ACE proof is not a canonical exact Norito encoding")]
    NonCanonicalProof,
    /// A compiled STARK/FRI parameter was substituted.
    #[error("ZK-ACE proof parameters do not match the compiled profile")]
    ParameterMismatch,
    /// The transcript label was substituted.
    #[error("ZK-ACE proof transcript does not match the compiled profile")]
    TranscriptMismatch,
    /// Generic auxiliary composition fields are forbidden for this relation.
    #[error("ZK-ACE proof carries forbidden auxiliary composition data")]
    AuxiliaryCompositionForbidden,
    /// The native prover failed.
    #[error("ZK-ACE prover failed: {0}")]
    Prover(String),
    /// The algebraic verifier rejected the proof.
    #[error("ZK-ACE native STARK/FRI verification failed")]
    VerificationFailed,
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr as _, sync::OnceLock};

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        asset::AssetDefinitionId,
        domain::DomainId,
        name::Name,
        proof::VerifyingKeyId,
        zk::{
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER, ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG, derive_zk_ace_identity_commitment,
            derive_zk_ace_replay_nullifier, derive_zk_ace_transfer_digest,
        },
    };

    use super::*;

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive ZK-ACE test account");
        AccountId::new(key_pair.public_key().clone())
    }

    fn asset() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("privacy", "universal").expect("test domain"),
            Name::from_str("zkace").expect("test asset name"),
        )
    }

    fn public_inputs_and_witness() -> (ZkAcePublicInputsV1, ZkAceWitnessV1) {
        let witness = ZkAceWitnessV1 {
            identity_root: [0x11; 32],
            identity_blinding: [0x22; 32],
            replay_secret: [0x33; 32],
        };
        let chain_id = ChainId::from("taira-privacy-zk-ace-test");
        let from = account(1);
        let to = account(2);
        let asset = asset();
        let policy_hash = [0x44; 32];
        let identity_commitment = derive_zk_ace_identity_commitment(
            &witness.identity_root,
            &witness.identity_blinding,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        let tx_digest = derive_zk_ace_transfer_digest(
            &from,
            &to,
            &asset,
            19,
            &chain_id,
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
            &policy_hash,
        );
        let replay_nullifier = derive_zk_ace_replay_nullifier(
            &witness.replay_secret,
            &tx_digest,
            &chain_id,
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        (
            ZkAcePublicInputsV1::transparent_transfer(
                identity_commitment,
                tx_digest,
                chain_id,
                replay_nullifier,
                policy_hash,
                from,
                to,
                asset,
                19,
                VerifyingKeyId::new(
                    ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
                    ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
                ),
            ),
            witness,
        )
    }

    fn valid_fixture() -> &'static (ZkAcePublicInputsV1, Vec<u8>) {
        static FIXTURE: OnceLock<(ZkAcePublicInputsV1, Vec<u8>)> = OnceLock::new();
        FIXTURE.get_or_init(|| {
            let (public_inputs, witness) = public_inputs_and_witness();
            let proof =
                prove_zk_ace_privacy_v1(&public_inputs, &witness).expect("native proof fixture");
            (public_inputs, proof)
        })
    }

    #[test]
    fn compiled_profile_digest_matches_every_exact_native_parameter() {
        assert_eq!(
            recompute_zk_ace_compiled_profile_digest_v1(),
            ZK_ACE_COMPILED_PROFILE_DIGEST_V1
        );
        assert_eq!(
            hex::encode(ZK_ACE_COMPILED_PROFILE_DIGEST_V1),
            "fbf0cbdce9999e1d0852f588c60dab9bb5ebcddceda1f117962af451ca454970"
        );
    }

    #[test]
    fn native_proof_roundtrips_under_the_exact_profile_and_cap() {
        let (public_inputs, proof) = valid_fixture();
        verify_zk_ace_privacy_v1(public_inputs, proof, ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES)
            .expect("native proof verifies");
        assert!(
            proof.len()
                <= usize::try_from(ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES)
                    .expect("proof cap fits usize")
        );
    }

    #[test]
    fn strict_wire_rejects_empty_truncated_trailing_and_tighter_cap() {
        let (public_inputs, proof) = valid_fixture();
        assert!(matches!(
            verify_zk_ace_privacy_v1(public_inputs, &[], ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES),
            Err(ZkAceNativeErrorV1::EmptyProof)
        ));
        for truncated_len in [
            1,
            proof.len() / 3,
            proof.len() / 2,
            proof.len().saturating_sub(1),
        ] {
            assert!(
                verify_zk_ace_privacy_v1(
                    public_inputs,
                    &proof[..truncated_len],
                    ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES,
                )
                .is_err(),
                "truncation at {truncated_len} must fail"
            );
        }
        let mut trailing = proof.clone();
        trailing.push(0);
        assert!(
            verify_zk_ace_privacy_v1(
                public_inputs,
                &trailing,
                ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES
            )
            .is_err()
        );
        let one_byte_tighter =
            u32::try_from(proof.len() - 1).expect("fixture proof length fits u32");
        assert!(matches!(
            verify_zk_ace_privacy_v1(public_inputs, proof, one_byte_tighter),
            Err(ZkAceNativeErrorV1::ProofTooLarge { .. })
        ));
    }

    #[test]
    fn every_compiled_profile_field_substitution_is_rejected() {
        let (public_inputs, proof) = valid_fixture();
        let mutations: [(&str, fn(&mut StarkVerifyEnvelopeV1)); 9] = [
            ("version", |envelope| envelope.params.version ^= 1),
            ("domain", |envelope| envelope.params.n_log2 -= 1),
            ("blowup", |envelope| envelope.params.blowup_log2 -= 1),
            ("fold", |envelope| envelope.params.fold_arity = 4),
            ("queries", |envelope| envelope.params.queries -= 1),
            ("merkle", |envelope| envelope.params.merkle_arity = 4),
            ("hash", |envelope| envelope.params.hash_fn = 2),
            ("domain-tag", |envelope| {
                envelope.params.domain_tag.push_str(":substituted");
            }),
            ("transcript", |envelope| {
                envelope.transcript_label.push_str(":substituted");
            }),
        ];
        for (label, mutate) in mutations {
            let mut envelope: StarkVerifyEnvelopeV1 =
                norito::decode_from_bytes(proof).expect("decode fixture");
            mutate(&mut envelope);
            let mutated = norito::to_bytes(&envelope).expect("encode mutation");
            assert!(
                verify_zk_ace_privacy_v1(
                    public_inputs,
                    &mutated,
                    ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES,
                )
                .is_err(),
                "{label} substitution must fail"
            );
        }
    }

    #[test]
    fn proof_rejects_every_public_authorization_binding_mutation() {
        let (public_inputs, proof) = valid_fixture();
        let mutations: [(&str, fn(&mut ZkAcePublicInputsV1)); 10] = [
            ("version", |inputs| inputs.version ^= 1),
            ("identity", |inputs| inputs.identity_commitment[0] ^= 1),
            ("transaction", |inputs| inputs.tx_digest[0] ^= 1),
            ("chain", |inputs| inputs.chain_id = ChainId::from("foreign")),
            ("domain", |inputs| inputs.domain_tag.push('x')),
            ("action", |inputs| inputs.action_class.push('x')),
            ("nullifier", |inputs| inputs.replay_nullifier[0] ^= 1),
            ("policy", |inputs| inputs.policy_hash[0] ^= 1),
            ("amount", |inputs| inputs.amount += 1),
            ("verifier", |inputs| {
                inputs.verifier_key_id =
                    VerifyingKeyId::new(ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND, "substituted");
            }),
        ];
        for (label, mutate) in mutations {
            let mut changed = public_inputs.clone();
            mutate(&mut changed);
            assert!(
                verify_zk_ace_privacy_v1(&changed, proof, ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES,)
                    .is_err(),
                "{label} replay must fail"
            );
        }
        for (label, mutate) in [
            (
                "source",
                (|inputs: &mut ZkAcePublicInputsV1| inputs.from = account(3))
                    as fn(&mut ZkAcePublicInputsV1),
            ),
            ("destination", |inputs: &mut ZkAcePublicInputsV1| {
                inputs.to = account(4);
            }),
            ("asset", |inputs: &mut ZkAcePublicInputsV1| {
                inputs.asset = AssetDefinitionId::new(
                    DomainId::try_new("privacy", "universal").expect("test domain"),
                    Name::from_str("other").expect("test asset name"),
                );
            }),
        ] {
            let mut changed = public_inputs.clone();
            mutate(&mut changed);
            assert!(
                verify_zk_ace_privacy_v1(&changed, proof, ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES,)
                    .is_err(),
                "{label} replay must fail"
            );
        }
    }
}
