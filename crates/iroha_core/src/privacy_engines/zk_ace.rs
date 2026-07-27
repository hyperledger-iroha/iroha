//! Native first-release ZK-ACE authorization engine.
//!
//! The only admitted proof wire is the dedicated masked execution-trace STARK
//! in `zk_ace_stark`. It proves both Poseidon2 relations, runs three
//! independently challenged composition/FRI lanes, and carries no caller
//! selected backend, verifier key, parameter record, or legacy generic
//! envelope.

use iroha_data_model::{
    privacy::{PrivacyConsensusLimitsV1, PrivacyStatementV1},
    zk::{ZkAcePrivacyPublicInputsV1, ZkAceWitnessV1},
};
use thiserror::Error;

#[cfg(test)]
use iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID;
#[cfg(test)]
use sha2::{Digest as _, Sha256};

#[cfg(test)]
use super::zk_ace_stark::proof_test_guard;
use super::zk_ace_stark::{
    COMPILED_STARK_PROFILE_DESCRIPTOR_V1, MAX_PROOF_BYTES, prove_zk_ace_stark_v1_with_rng,
    verify_zk_ace_stark_v1,
};

/// Transcript family frozen into the dedicated proof implementation.
pub const ZK_ACE_PRIVACY_TRANSCRIPT_LABEL_V1: &str = "iroha:privacy:zk-ace:transparent-stark:v1";
/// Source and relation description frozen into the compiled profile.
pub const ZK_ACE_SOURCE_PROFILE_V1: &[u8] = b"iroha-native-rust:zk-ace:typed-statement+trusted-genesis:private-witness:masked-poseidon2-execution-trace:three-lane-fri:v1";
/// Exact native proof wire description frozen into the compiled profile.
pub const ZK_ACE_PROOF_WIRE_V1: &[u8] =
    b"ZKA1:fixed-shape-big-endian:922214:strict-exact:no-lengths:no-generic-envelope";
/// Exact low-level AIR relation schema frozen into the compiled profile.
pub const ZK_ACE_AIR_RELATION_SCHEMA_V1: &[u8] = b"version:u16|identity_commitment:bytes32|tx_digest:bytes32|authorization_digest:bytes32|chain_id|fixed_domain|fixed_action|replay_nullifier:bytes32|policy_digest:bytes32|source|destination|asset_definition_id|amount:u128|fixed_verifier";
/// Exact typed authorization projection frozen into the compiled profile.
pub const ZK_ACE_AUTHORIZATION_PROJECTION_V1: &[u8] = b"norito:zk-ace-pq-authorization-statement-v1:replay-nullifier-zero|transaction-intent-bound|trusted-genesis-bound";
/// Canonical Poseidon2 constant manifest consumed through `fastpq_prover`.
pub const ZK_ACE_POSEIDON_MANIFEST_SHA256_V1: &str =
    "99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21";
/// Native and consensus proof byte ceiling.
pub const ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1: u32 = MAX_PROOF_BYTES as u32;
/// Frozen digest of every compiled verifier-profile field below.
pub const ZK_ACE_COMPILED_PROFILE_DIGEST_V1: [u8; 32] = [
    0xb0, 0x7a, 0x87, 0x1c, 0xe7, 0x48, 0xe4, 0xe2, 0x1d, 0x02, 0x5e, 0xe0, 0xe8, 0xd8, 0x6a, 0x69,
    0x41, 0x32, 0x2d, 0x9e, 0xd9, 0x50, 0x96, 0xf1, 0x32, 0x8a, 0x69, 0xde, 0xdc, 0xc6, 0x47, 0x86,
];

/// Return the frozen digest of the exact compiled native verifier profile.
#[must_use]
pub const fn zk_ace_compiled_profile_digest_v1() -> [u8; 32] {
    ZK_ACE_COMPILED_PROFILE_DIGEST_V1
}

/// Return the complete human-auditable compiled algebraic profile.
#[must_use]
pub const fn zk_ace_stark_profile_descriptor_v1() -> &'static [u8] {
    COMPILED_STARK_PROFILE_DESCRIPTOR_V1
}

#[cfg(test)]
fn recompute_zk_ace_compiled_profile_digest_v1() -> [u8; 32] {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"iroha.privacy.zk-ace.compiled-profile.v1");
    hash_field(&mut hasher, ZK_ACE_SOURCE_PROFILE_V1);
    hash_field(&mut hasher, ZK_ACE_PROOF_WIRE_V1);
    hash_field(&mut hasher, ZK_ACE_AIR_RELATION_SCHEMA_V1);
    hash_field(&mut hasher, ZK_ACE_AUTHORIZATION_PROJECTION_V1);
    hash_field(&mut hasher, ZK_ACE_POSEIDON_MANIFEST_SHA256_V1.as_bytes());
    hash_field(
        &mut hasher,
        ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID.as_bytes(),
    );
    hash_field(&mut hasher, ZK_ACE_PRIVACY_TRANSCRIPT_LABEL_V1.as_bytes());
    hash_field(&mut hasher, zk_ace_stark_profile_descriptor_v1());
    hash_field(
        &mut hasher,
        &ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1.to_be_bytes(),
    );
    hasher.finalize().into()
}

#[cfg(test)]
fn hash_field(hasher: &mut Sha256, field: &[u8]) {
    hasher.update(
        u64::try_from(field.len())
            .expect("fixed profile field length fits u64")
            .to_be_bytes(),
    );
    hasher.update(field);
}

/// Generate the canonical randomized proof for exact typed public inputs.
///
/// # Errors
///
/// Returns a typed validation error or fails closed if operating-system
/// randomness, canonical projection, proving, or the prover self-check fails.
pub fn prove_zk_ace_privacy_v1(
    public_inputs: &ZkAcePrivacyPublicInputsV1,
    witness: &ZkAceWitnessV1,
) -> Result<Vec<u8>, ZkAceNativeErrorV1> {
    validate_privacy_public_inputs(public_inputs)?;
    let relation_inputs = public_inputs
        .to_air_relation_inputs()
        .map_err(|_| ZkAceNativeErrorV1::PublicInputsEncoding)?;
    let mut rng = rand::rngs::OsRng;
    prove_zk_ace_stark_v1_with_rng(&relation_inputs, witness, &mut rng).map_err(|error| {
        ZkAceNativeErrorV1::Prover {
            reason: error.to_string(),
        }
    })
}

/// Verify canonical proof bytes against exact typed public inputs.
///
/// The effective byte ceiling is the smaller of the consensus caller limit
/// and the native compiled limit.
///
/// # Errors
///
/// Rejects invalid typed inputs and every empty, oversized, malformed,
/// non-canonical, transcript-inconsistent, algebraically invalid, or
/// low-degree-invalid proof.
pub fn verify_zk_ace_privacy_v1(
    public_inputs: &ZkAcePrivacyPublicInputsV1,
    proof: &[u8],
    caller_max_proof_bytes: u32,
) -> Result<(), ZkAceNativeErrorV1> {
    validate_privacy_public_inputs(public_inputs)?;
    let effective_cap = caller_max_proof_bytes.min(ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1);
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
    let relation_inputs = public_inputs
        .to_air_relation_inputs()
        .map_err(|_| ZkAceNativeErrorV1::PublicInputsEncoding)?;
    verify_zk_ace_stark_v1(&relation_inputs, proof)
        .map_err(|_| ZkAceNativeErrorV1::VerificationFailed)
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

/// Native ZK-ACE construction or verification failure.
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
    /// Canonical public-input projection failed.
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
    /// Native construction or its mandatory self-check failed.
    #[error("ZK-ACE prover failed: {reason}")]
    Prover {
        /// Fail-closed diagnostic.
        reason: String,
    },
    /// The dedicated native verifier rejected the proof.
    #[error("ZK-ACE native transparent STARK verification failed")]
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
            .expect("derive test account");
        AccountId::new(key_pair.public_key().clone())
    }

    fn asset() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("privacy", "universal").expect("test domain"),
            Name::from_str("zkace").expect("test asset"),
        )
    }

    fn public_inputs_and_witness() -> (ZkAcePrivacyPublicInputsV1, ZkAceWitnessV1) {
        let witness = ZkAceWitnessV1 {
            identity_root: [0x11; 32],
            identity_blinding: [0x22; 32],
            replay_secret: [0x33; 32],
        };
        let chain_id = ChainId::from("taira-privacy-zk-ace-test");
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
            source: account(1),
            destination: account(2),
            asset_definition_id: asset(),
            amount: 19,
            fee: 3,
            authorization_epoch: 7,
            replay_nullifier: PrivacyNullifierV1::new([0; 32]),
        };
        let mut public_inputs = ZkAcePrivacyPublicInputsV1::new(statement, [0x48; 32]);
        let authorization_digest = derive_zk_ace_privacy_authorization_digest(&public_inputs)
            .expect("typed authorization digest");
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

    fn fixture() -> &'static (ZkAcePrivacyPublicInputsV1, Vec<u8>) {
        static FIXTURE: OnceLock<(ZkAcePrivacyPublicInputsV1, Vec<u8>)> = OnceLock::new();
        let _guard = proof_test_guard();
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
            fastpq_prover::poseidon_manifest_sha256(),
            ZK_ACE_POSEIDON_MANIFEST_SHA256_V1
        );
        assert_eq!(
            recompute_zk_ace_compiled_profile_digest_v1(),
            ZK_ACE_COMPILED_PROFILE_DIGEST_V1
        );
    }

    #[test]
    fn authorization_projection_breaks_only_replay_cycle() {
        let (public_inputs, _) = public_inputs_and_witness();
        let expected = derive_zk_ace_privacy_authorization_digest(&public_inputs)
            .expect("authorization projection");
        let mut changed = public_inputs.clone();
        changed.statement.replay_nullifier.0[0] ^= 1;
        assert_eq!(
            derive_zk_ace_privacy_authorization_digest(&changed).expect("normalized nullifier"),
            expected
        );
        changed = public_inputs.clone();
        changed.statement.context.transaction_intent_digest.0[0] ^= 1;
        assert_ne!(
            derive_zk_ace_privacy_authorization_digest(&changed).expect("intent binding"),
            expected
        );
        changed = public_inputs;
        changed.genesis_hash[0] ^= 1;
        assert_ne!(
            derive_zk_ace_privacy_authorization_digest(&changed).expect("genesis binding"),
            expected
        );
    }

    #[test]
    fn typed_engine_roundtrips_and_enforces_both_caps() {
        let (public_inputs, proof) = fixture();
        verify_zk_ace_privacy_v1(public_inputs, proof, ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1)
            .expect("typed proof verifies");
        let tighter = u32::try_from(proof.len() - 1).expect("proof fits u32");
        assert!(matches!(
            verify_zk_ace_privacy_v1(public_inputs, proof, tighter),
            Err(ZkAceNativeErrorV1::ProofTooLarge { .. })
        ));
    }

    #[test]
    fn malformed_typed_inputs_fail_before_proof_decoding() {
        let (public_inputs, _) = public_inputs_and_witness();
        let mut changed = public_inputs.clone();
        changed.version = 2;
        assert!(matches!(
            verify_zk_ace_privacy_v1(&changed, &[1], ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1),
            Err(ZkAceNativeErrorV1::PublicInputVersionMismatch { actual: 2 })
        ));
        changed = public_inputs.clone();
        changed.genesis_hash = [0; 32];
        assert!(matches!(
            verify_zk_ace_privacy_v1(&changed, &[1], ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1),
            Err(ZkAceNativeErrorV1::ZeroGenesisHash)
        ));
        changed = public_inputs;
        changed.statement.policy_id = PrivacyPolicyIdV1::new([0; 32]);
        assert!(matches!(
            verify_zk_ace_privacy_v1(&changed, &[1], ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1),
            Err(ZkAceNativeErrorV1::InvalidStatement)
        ));
    }

    #[test]
    fn every_typed_public_binding_and_sampled_corruption_rejects() {
        let (public_inputs, proof) = fixture();
        let mutations: [fn(&mut ZkAcePrivacyPublicInputsV1); 20] = [
            |value| value.version ^= 1,
            |value| value.genesis_hash[0] ^= 1,
            |value| value.statement.identity_commitment.0[0] ^= 1,
            |value| value.statement.context.transaction_intent_digest.0[0] ^= 1,
            |value| value.statement.context.chain_id = ChainId::from("foreign"),
            |value| value.statement.context.parameter_id.0[0] ^= 1,
            |value| value.statement.context.parameter_digest.0[0] ^= 1,
            |value| value.statement.context.verifier_digest.0[0] ^= 1,
            |value| value.statement.context.statement_schema_digest.0[0] ^= 1,
            |value| value.statement.context.engine_manifest_digest.0[0] ^= 1,
            |value| value.statement.replay_nullifier.0[0] ^= 1,
            |value| value.statement.policy_id.0[0] ^= 1,
            |value| value.statement.policy_digest.0[0] ^= 1,
            |value| value.statement.amount += 1,
            |value| value.statement.fee += 1,
            |value| value.statement.authorization_epoch += 1,
            |value| value.statement.context.action_index = 1,
            |value| value.statement.source = account(3),
            |value| value.statement.destination = account(4),
            |value| {
                value.statement.asset_definition_id = AssetDefinitionId::new(
                    DomainId::try_new("privacy", "universal").expect("test domain"),
                    Name::from_str("other").expect("other asset"),
                );
            },
        ];
        for mutate in mutations {
            let mut changed = public_inputs.clone();
            mutate(&mut changed);
            assert!(
                verify_zk_ace_privacy_v1(&changed, proof, ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1)
                    .is_err()
            );
        }
        let samples = 96usize.min(proof.len());
        for sample in 0..samples {
            let offset = sample * proof.len() / samples;
            let mut corrupted = proof.clone();
            corrupted[offset] ^= 1 << (sample % 8);
            assert!(
                verify_zk_ace_privacy_v1(
                    public_inputs,
                    &corrupted,
                    ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1
                )
                .is_err()
            );
        }
    }
}
