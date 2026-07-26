//! Native first-release ZK-ACE authorization engine.
//!
//! The privacy runtime uses the STARK/FRI envelope directly.  It does not route
//! through the generic verifier-key registry, accept a caller-selected backend,
//! or preserve the historical `ProofAttachment` wrapper.  The exact circuit,
//! SHA-256/Goldilocks parameters, transcript label, domain tag, and proof-size
//! ceiling are compiled below and checked before the algebraic verifier runs.

use iroha_data_model::privacy::{PrivacyConsensusLimitsV1, PrivacyStatementV1};
use iroha_data_model::zk::{
    ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID, ZkAcePrivacyPublicInputsV1, ZkAceWitnessV1,
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
    b"iroha-native-rust:zk-ace:typed-statement+trusted-genesis:private-witness-air:sha256-goldilocks:v1";
/// Exact native proof wire description frozen into the compiled profile.
pub const ZK_ACE_PROOF_WIRE_V1: &[u8] =
    b"norito:stark-verify-envelope-v1:strict-exact:no-openverify-wrapper";
/// Exact low-level AIR relation schema frozen into the compiled profile.
pub const ZK_ACE_AIR_RELATION_SCHEMA_V1: &[u8] = b"version:u16|identity_commitment:bytes32|tx_digest:bytes32|authorization_digest:bytes32|chain_id|fixed_domain|fixed_action|replay_nullifier:bytes32|policy_digest:bytes32|source|destination|asset_definition_id|amount:u128|fixed_verifier";
/// Exact typed authorization projection frozen into the compiled profile.
pub const ZK_ACE_AUTHORIZATION_PROJECTION_V1: &[u8] = b"norito:zk-ace-pq-authorization-statement-v1:replay-nullifier-zero|transaction-intent-bound|trusted-genesis-bound";
/// Frozen digest of every compiled verifier-profile field below.
pub const ZK_ACE_COMPILED_PROFILE_DIGEST_V1: [u8; 32] = [
    0x59, 0x20, 0xee, 0xc3, 0x73, 0x89, 0x47, 0xc2, 0x04, 0x0a, 0xad, 0x41, 0x32, 0x43, 0x75, 0x32,
    0x45, 0xe0, 0x78, 0x84, 0x76, 0xab, 0x0c, 0x81, 0xe1, 0x30, 0xb8, 0xde, 0xfc, 0x05, 0x40, 0x1c,
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
    hash_field(&mut hasher, ZK_ACE_AIR_RELATION_SCHEMA_V1);
    hash_field(&mut hasher, ZK_ACE_AUTHORIZATION_PROJECTION_V1);
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
    public_inputs: &ZkAcePrivacyPublicInputsV1,
    witness: &ZkAceWitnessV1,
) -> Result<Vec<u8>, ZkAceNativeErrorV1> {
    validate_privacy_public_inputs(public_inputs)?;
    let relation_inputs = public_inputs
        .to_air_relation_inputs()
        .map_err(|_| ZkAceNativeErrorV1::PublicInputsEncoding)?;
    let public_digest = derive_zk_ace_air_public_digest(&relation_inputs)
        .map_err(|_| ZkAceNativeErrorV1::PublicInputsEncoding)?;
    let proof = prove_stark_fri_zk_ace_air_envelope_bytes(
        zk_ace_privacy_stark_params_v1(),
        ZK_ACE_PRIVACY_TRANSCRIPT_LABEL_V1.to_owned(),
        ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID.to_owned(),
        public_digest,
        &relation_inputs,
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
    public_inputs: &ZkAcePrivacyPublicInputsV1,
    proof: &[u8],
    caller_max_proof_bytes: u32,
) -> Result<(), ZkAceNativeErrorV1> {
    validate_privacy_public_inputs(public_inputs)?;
    let relation_inputs = public_inputs
        .to_air_relation_inputs()
        .map_err(|_| ZkAceNativeErrorV1::PublicInputsEncoding)?;
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
    if !verify_stark_fri_zk_ace_envelope_with_limits(proof, &limits, &relation_inputs) {
        return Err(ZkAceNativeErrorV1::VerificationFailed);
    }
    Ok(())
}

fn validate_privacy_public_inputs(
    public_inputs: &ZkAcePrivacyPublicInputsV1,
) -> Result<(), ZkAceNativeErrorV1> {
    if public_inputs.version != 1 {
        return Err(ZkAceNativeErrorV1::PublicInputVersionMismatch {
            actual: public_inputs.version,
        });
    }
    if public_inputs.genesis_hash == [0; 32] {
        return Err(ZkAceNativeErrorV1::ZeroGenesisHash);
    }
    PrivacyStatementV1::ZkAcePqAuthorizationV0(public_inputs.statement.clone())
        .validate(&PrivacyConsensusLimitsV1::taira_default())
        .map_err(|_| ZkAceNativeErrorV1::InvalidStatement)
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
    /// The public-input wrapper is not the exact first-release schema.
    #[error("ZK-ACE public-input version must be 1, got {actual}")]
    PublicInputVersionMismatch {
        /// Rejected version.
        actual: u16,
    },
    /// Trusted genesis binding is absent.
    #[error("ZK-ACE trusted genesis hash must be non-zero")]
    ZeroGenesisHash,
    /// The typed privacy statement failed its closed validation rules.
    #[error("ZK-ACE typed privacy statement is invalid")]
    InvalidStatement,
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
        privacy::{
            PrivacyCommitmentV1, PrivacyEngineManifestDigestV1, PrivacyNullifierV1,
            PrivacyParameterDigestV1, PrivacyParameterIdV1, PrivacyPolicyDigestV1,
            PrivacyPolicyIdV1, PrivacyStatementContextV1, PrivacyStatementSchemaDigestV1,
            PrivacyTransactionIntentDigestV1, PrivacyVerifierDigestV1,
            ZkAcePqAuthorizationStatementV1,
        },
        zk::{
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER, ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
            derive_zk_ace_identity_commitment, derive_zk_ace_privacy_authorization_digest,
            derive_zk_ace_replay_nullifier,
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

    fn public_inputs_and_witness() -> (ZkAcePrivacyPublicInputsV1, ZkAceWitnessV1) {
        let witness = ZkAceWitnessV1 {
            identity_root: [0x11; 32],
            identity_blinding: [0x22; 32],
            replay_secret: [0x33; 32],
        };
        let chain_id = ChainId::from("taira-privacy-zk-ace-test");
        let from = account(1);
        let to = account(2);
        let asset = asset();
        let identity_commitment = derive_zk_ace_identity_commitment(
            &witness.identity_root,
            &witness.identity_blinding,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        let statement = ZkAcePqAuthorizationStatementV1 {
            context: PrivacyStatementContextV1 {
                chain_id: chain_id.clone(),
                action_index: 0,
                transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0x40; 32]),
                parameter_id: PrivacyParameterIdV1::new([0x41; 32]),
                parameter_digest: PrivacyParameterDigestV1::new([0x42; 32]),
                verifier_digest: PrivacyVerifierDigestV1::new([0x43; 32]),
                statement_schema_digest: PrivacyStatementSchemaDigestV1::new([0x44; 32]),
                engine_manifest_digest: PrivacyEngineManifestDigestV1::new([0x45; 32]),
            },
            identity_commitment: PrivacyCommitmentV1::new(identity_commitment),
            policy_id: PrivacyPolicyIdV1::new([0x46; 32]),
            policy_digest: PrivacyPolicyDigestV1::new([0x47; 32]),
            source: from,
            destination: to,
            asset_definition_id: asset,
            amount: 19,
            fee: 3,
            authorization_epoch: 7,
            replay_nullifier: PrivacyNullifierV1::new([0; 32]),
        };
        let mut public_inputs = ZkAcePrivacyPublicInputsV1::new(statement, [0x48; 32]);
        let authorization_digest = derive_zk_ace_privacy_authorization_digest(&public_inputs)
            .expect("derive typed authorization digest");
        let replay_nullifier = derive_zk_ace_replay_nullifier(
            &witness.replay_secret,
            &authorization_digest,
            &chain_id,
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        public_inputs.statement.replay_nullifier = PrivacyNullifierV1::new(replay_nullifier);
        (public_inputs, witness)
    }

    fn valid_fixture() -> &'static (ZkAcePrivacyPublicInputsV1, Vec<u8>) {
        static FIXTURE: OnceLock<(ZkAcePrivacyPublicInputsV1, Vec<u8>)> = OnceLock::new();
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
            "5920eec3738947c2040aad413243753245e0788476ab0c81e130b8defc05401c"
        );
    }

    #[test]
    fn authorization_projection_breaks_only_the_replay_cycle_and_binds_intent_and_genesis() {
        let (public_inputs, _) = public_inputs_and_witness();
        let expected = derive_zk_ace_privacy_authorization_digest(&public_inputs)
            .expect("base authorization projection");

        let mut changed = public_inputs.clone();
        changed.statement.replay_nullifier.0[0] ^= 1;
        assert_eq!(
            derive_zk_ace_privacy_authorization_digest(&changed)
                .expect("nullifier-normalized projection"),
            expected
        );

        changed = public_inputs.clone();
        changed.statement.context.transaction_intent_digest.0[0] ^= 1;
        assert_ne!(
            derive_zk_ace_privacy_authorization_digest(&changed).expect("intent-bound projection"),
            expected
        );

        changed = public_inputs.clone();
        changed.genesis_hash[0] ^= 1;
        assert_ne!(
            derive_zk_ace_privacy_authorization_digest(&changed).expect("genesis-bound projection"),
            expected
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
        let envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(proof).expect("decode verified proof");
        assert_eq!(envelope.params.queries, 48);
        assert_eq!(envelope.proof.queries.len(), 48);
        assert_eq!(
            envelope
                .proof
                .air
                .as_ref()
                .expect("ZK-ACE AIR attachment")
                .openings
                .len(),
            48
        );
    }

    #[test]
    fn malformed_typed_inputs_fail_before_proof_decoding() {
        let (public_inputs, _) = public_inputs_and_witness();

        let mut changed = public_inputs.clone();
        changed.version = 2;
        assert!(matches!(
            verify_zk_ace_privacy_v1(&changed, &[1], ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES),
            Err(ZkAceNativeErrorV1::PublicInputVersionMismatch { actual: 2 })
        ));

        changed = public_inputs.clone();
        changed.genesis_hash = [0; 32];
        assert!(matches!(
            verify_zk_ace_privacy_v1(&changed, &[1], ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES),
            Err(ZkAceNativeErrorV1::ZeroGenesisHash)
        ));

        changed = public_inputs;
        changed.statement.policy_id = PrivacyPolicyIdV1::new([0; 32]);
        assert!(matches!(
            verify_zk_ace_privacy_v1(&changed, &[1], ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES),
            Err(ZkAceNativeErrorV1::InvalidStatement)
        ));
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

        let mut envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(proof).expect("decode fixture");
        envelope.proof.commits.comp_root = Some([0xA5; 32]);
        let mutated = norito::to_bytes(&envelope).expect("encode auxiliary composition mutation");
        assert!(matches!(
            verify_zk_ace_privacy_v1(public_inputs, &mutated, ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES),
            Err(ZkAceNativeErrorV1::AuxiliaryCompositionForbidden)
        ));
    }

    #[test]
    fn proof_rejects_every_public_authorization_binding_mutation() {
        let (public_inputs, proof) = valid_fixture();
        let mutations: [(&str, fn(&mut ZkAcePrivacyPublicInputsV1)); 17] = [
            ("version", |inputs| inputs.version ^= 1),
            ("genesis", |inputs| inputs.genesis_hash[0] ^= 1),
            ("identity", |inputs| {
                inputs.statement.identity_commitment.0[0] ^= 1;
            }),
            ("transaction", |inputs| {
                inputs.statement.context.transaction_intent_digest.0[0] ^= 1;
            }),
            ("chain", |inputs| {
                inputs.statement.context.chain_id = ChainId::from("foreign");
            }),
            ("parameter-id", |inputs| {
                inputs.statement.context.parameter_id.0[0] ^= 1;
            }),
            ("parameter-digest", |inputs| {
                inputs.statement.context.parameter_digest.0[0] ^= 1;
            }),
            ("verifier-digest", |inputs| {
                inputs.statement.context.verifier_digest.0[0] ^= 1;
            }),
            ("schema-digest", |inputs| {
                inputs.statement.context.statement_schema_digest.0[0] ^= 1;
            }),
            ("manifest-digest", |inputs| {
                inputs.statement.context.engine_manifest_digest.0[0] ^= 1;
            }),
            ("nullifier", |inputs| {
                inputs.statement.replay_nullifier.0[0] ^= 1;
            }),
            ("policy-id", |inputs| inputs.statement.policy_id.0[0] ^= 1),
            ("policy-digest", |inputs| {
                inputs.statement.policy_digest.0[0] ^= 1;
            }),
            ("amount", |inputs| inputs.statement.amount += 1),
            ("fee", |inputs| inputs.statement.fee += 1),
            ("authorization-epoch", |inputs| {
                inputs.statement.authorization_epoch += 1;
            }),
            ("action-index", |inputs| {
                inputs.statement.context.action_index = 1;
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
        let action_mutations: [(&str, fn(&mut ZkAcePrivacyPublicInputsV1)); 3] = [
            ("source", |inputs: &mut ZkAcePrivacyPublicInputsV1| {
                inputs.statement.source = account(3);
            }),
            ("destination", |inputs: &mut ZkAcePrivacyPublicInputsV1| {
                inputs.statement.destination = account(4);
            }),
            ("asset", |inputs: &mut ZkAcePrivacyPublicInputsV1| {
                inputs.statement.asset_definition_id = AssetDefinitionId::new(
                    DomainId::try_new("privacy", "universal").expect("test domain"),
                    Name::from_str("other").expect("test asset name"),
                );
            }),
        ];
        for (label, mutate) in action_mutations {
            let mut changed = public_inputs.clone();
            mutate(&mut changed);
            assert!(
                verify_zk_ace_privacy_v1(&changed, proof, ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES,)
                    .is_err(),
                "{label} replay must fail"
            );
        }
    }

    #[test]
    fn sampled_single_byte_corruptions_never_verify() {
        let (public_inputs, proof) = valid_fixture();
        let sample_count = 64usize.min(proof.len());
        for sample in 0..sample_count {
            let offset = sample * proof.len() / sample_count;
            let mut corrupted = proof.clone();
            corrupted[offset] ^= 1u8 << (sample % 8);
            assert!(
                verify_zk_ace_privacy_v1(
                    public_inputs,
                    &corrupted,
                    ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES
                )
                .is_err(),
                "byte corruption sample {sample} at offset {offset} must fail"
            );
        }
    }
}
