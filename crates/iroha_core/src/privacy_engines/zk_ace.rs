//! Native first-release ZK-ACE authorization engine.
//!
//! The only admitted proof wire is the dedicated masked execution-trace STARK
//! in `zk_ace_stark`. It proves both Poseidon2 relations, commits an
//! independent full-space zero-knowledge mask before batching challenges,
//! links the AIR at one quartic-extension DEEP point, runs Fp4 FRI, and
//! carries no caller-selected backend, verifier key, parameter record, or
//! legacy generic envelope.
//!
//! The compiled 128-bit Fiat--Shamir certificate is work-normalized in the
//! classical random-oracle model.  The `pq_authorization` relation name does
//! not assert an additional quantum-random-oracle reduction for this STARK.

use iroha_data_model::{
    NetworkId,
    privacy::{
        PrivacyCommitmentV1, PrivacyConsensusLimitsV1, PrivacyNullifierV1, PrivacyStatementV1,
    },
    zk::{
        ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER, ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        ZkAcePrivacyPublicInputsV1, derive_zk_ace_identity_commitment,
        derive_zk_ace_privacy_authorization_digest, derive_zk_ace_replay_nullifier,
        derive_zk_ace_transfer_digest,
    },
};
use rand::TryCryptoRng;
use thiserror::Error;
use zeroize::Zeroize;

/// Fallible cryptographic RNG contract accepted by the native prover.
pub use rand::TryCryptoRng as ZkAceTryCryptoRngV1;
/// Fallible RNG core contract re-exported for deterministic/adversarial tests
/// without forcing transaction-builder crates to depend on `rand`.
pub use rand::TryRngCore as ZkAceTryRngCoreV1;

#[cfg(test)]
use iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID;
#[cfg(test)]
use sha2::{Digest as _, Sha256};

#[cfg(test)]
use super::prover_randomness::TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1;
#[cfg(test)]
use super::zk_ace_stark::proof_test_guard;
use super::zk_ace_stark::{
    AIR_PUBLIC_TRANSCRIPT_SCHEMA_V1, COMPILED_STARK_PROFILE_DESCRIPTOR_V1, MAX_PROOF_BYTES,
    MAX_ROM_QUERY_LOG2_V1, PROVABLE_SOUNDNESS_BITS_V1, ZkAceAirRelationInputsV1, ZkAceStarkError,
    prove_zk_ace_stark_v1_with_rng, verify_zk_ace_stark_v1,
};

/// Secret witness accepted by the first-release native ZK-ACE engine.
///
/// It intentionally implements neither `Debug`, `Clone`, `Copy`, nor any
/// serialization trait. The engine owns the three secrets and overwrites them
/// when the witness leaves scope on every success and error path.
pub struct ZkAcePrivacyWitnessV1 {
    pub(super) identity_root: [u8; 32],
    pub(super) identity_blinding: [u8; 32],
    pub(super) replay_secret: [u8; 32],
}

impl ZkAcePrivacyWitnessV1 {
    /// Validate and take ownership of one native witness.
    ///
    /// # Errors
    ///
    /// Rejects any all-zero secret component before proof work or entropy use.
    pub fn try_new(
        identity_root: [u8; 32],
        identity_blinding: [u8; 32],
        replay_secret: [u8; 32],
    ) -> Result<Self, ZkAcePrivacyWitnessValidationErrorV1> {
        let witness = Self {
            identity_root,
            identity_blinding,
            replay_secret,
        };
        witness.validate()?;
        Ok(witness)
    }

    fn validate(&self) -> Result<(), ZkAcePrivacyWitnessValidationErrorV1> {
        if self.identity_root == [0; 32] {
            return Err(ZkAcePrivacyWitnessValidationErrorV1::ZeroIdentityRoot);
        }
        if self.identity_blinding == [0; 32] {
            return Err(ZkAcePrivacyWitnessValidationErrorV1::ZeroIdentityBlinding);
        }
        if self.replay_secret == [0; 32] {
            return Err(ZkAcePrivacyWitnessValidationErrorV1::ZeroReplaySecret);
        }
        Ok(())
    }

    /// Derive the public identity commitment under the only admitted domain.
    #[must_use]
    pub fn identity_commitment_v1(&self) -> PrivacyCommitmentV1 {
        PrivacyCommitmentV1::new(derive_zk_ace_identity_commitment(
            &self.identity_root,
            &self.identity_blinding,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        ))
    }

    /// Derive the typed replay nullifier without exposing the replay secret.
    #[must_use]
    pub fn replay_nullifier_v1(
        &self,
        authorization_digest: &[u8; 32],
        network_id: &NetworkId,
    ) -> PrivacyNullifierV1 {
        PrivacyNullifierV1::new(derive_zk_ace_replay_nullifier(
            &self.replay_secret,
            authorization_digest,
            network_id,
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        ))
    }
}

impl Drop for ZkAcePrivacyWitnessV1 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl Zeroize for ZkAcePrivacyWitnessV1 {
    fn zeroize(&mut self) {
        self.identity_root.zeroize();
        self.identity_blinding.zeroize();
        self.replay_secret.zeroize();
    }
}

/// Rejected native witness shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum ZkAcePrivacyWitnessValidationErrorV1 {
    /// The external identity root is the all-zero sentinel.
    #[error("ZK-ACE identity root witness must be non-zero")]
    ZeroIdentityRoot,
    /// The identity blinding is the all-zero sentinel.
    #[error("ZK-ACE identity blinding witness must be non-zero")]
    ZeroIdentityBlinding,
    /// The replay secret is the all-zero sentinel.
    #[error("ZK-ACE replay secret witness must be non-zero")]
    ZeroReplaySecret,
}

/// Transcript family frozen into the dedicated proof implementation.
pub const ZK_ACE_PRIVACY_TRANSCRIPT_LABEL_V1: &str = "iroha:privacy:zk-ace:transparent-stark:v1";
/// Source and relation description frozen into the compiled profile.
pub const ZK_ACE_SOURCE_PROFILE_V1: &[u8] = b"iroha-native-rust:zk-ace:typed-statement+trusted-genesis:type-name-independent-ordered-length-framed-public-transcript:private-witness:masked-poseidon2-execution-trace:fp4-deep-ali:independent-pre-batching-fri-mask:fp4-fri:producer=preflight+rand0.9-trycrypto-fixed64-reservoir-zeroize-poison-error-or-unwind+self-verify:v1";
/// Exact native proof wire description frozen into the compiled profile.
pub const ZK_ACE_PROOF_WIRE_V1: &[u8] =
    b"ZKA1:fixed-shape-big-endian:1341142:strict-exact:no-lengths:no-generic-envelope";
/// Exact low-level AIR relation schema frozen into the compiled profile.
pub const ZK_ACE_AIR_RELATION_SCHEMA_V1: &[u8] = AIR_PUBLIC_TRANSCRIPT_SCHEMA_V1;
/// Exact typed authorization projection frozen into the compiled profile.
pub const ZK_ACE_AUTHORIZATION_PROJECTION_V1: &[u8] = b"norito:zk-ace-pq-authorization-statement-v1:replay-nullifier-zero|transaction-intent-bound|trusted-genesis-bound";
/// Canonical Poseidon2 constant manifest consumed through `fastpq_prover`.
pub const ZK_ACE_POSEIDON_MANIFEST_SHA256_V1: &str =
    "99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21";
/// Native and consensus proof byte ceiling.
pub const ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1: u32 = MAX_PROOF_BYTES as u32;
/// Theorem-backed classical-ROM, work-normalized soundness of the profile.
///
/// This is not a qROM security claim.
pub const ZK_ACE_PROVABLE_SOUNDNESS_BITS_V1: u16 = PROVABLE_SOUNDNESS_BITS_V1;
/// Maximum base-two random-oracle query-work exponent covered by that bound.
pub const ZK_ACE_MAX_ROM_QUERY_LOG2_V1: u8 = MAX_ROM_QUERY_LOG2_V1;
/// Frozen digest of every compiled verifier-profile field below.
pub const ZK_ACE_COMPILED_PROFILE_DIGEST_V1: [u8; 32] = [
    0x88, 0xb9, 0x47, 0x02, 0x57, 0x81, 0x53, 0x2f, 0x27, 0x58, 0x52, 0x13, 0x8c, 0xfd, 0x3b, 0xd1,
    0x82, 0x6a, 0x3b, 0xb1, 0xa8, 0x0b, 0xb3, 0xbf, 0xe5, 0xcb, 0x6e, 0xaf, 0xbc, 0x3c, 0x37, 0x1c,
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
    hash_field(&mut hasher, TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1);
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

fn project_air_relation_inputs_v1(
    public_inputs: &ZkAcePrivacyPublicInputsV1,
    authorization_digest: [u8; 32],
) -> Result<ZkAceAirRelationInputsV1, ZkAceNativeErrorV1> {
    let statement = &public_inputs.statement;
    let transfer_digest = derive_zk_ace_transfer_digest(
        &statement.source,
        &statement.destination,
        &statement.asset_definition_id,
        statement.amount,
        &statement.context.network_id,
        ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
        statement.policy_digest.as_bytes(),
    )
    .map_err(|_| ZkAceNativeErrorV1::PublicInputsEncoding)?;
    Ok(ZkAceAirRelationInputsV1::transparent_transfer(
        statement.identity_commitment.into_bytes(),
        transfer_digest,
        authorization_digest,
        statement.context.network_id,
        statement.replay_nullifier.into_bytes(),
        statement.policy_digest.into_bytes(),
        statement.source.clone(),
        statement.destination.clone(),
        statement.asset_definition_id.clone(),
        statement.amount,
    ))
}

fn validate_privacy_witness_relation(
    public_inputs: &ZkAcePrivacyPublicInputsV1,
    witness: &ZkAcePrivacyWitnessV1,
    authorization_digest: &[u8; 32],
) -> Result<(), ZkAceNativeErrorV1> {
    let statement = &public_inputs.statement;
    if witness.identity_commitment_v1() != statement.identity_commitment
        || witness.replay_nullifier_v1(authorization_digest, &statement.context.network_id)
            != statement.replay_nullifier
    {
        return Err(ZkAceNativeErrorV1::WitnessRelationMismatch);
    }
    Ok(())
}

/// Generate the canonical randomized proof with injected fallible entropy.
///
/// # Errors
///
/// Returns a typed validation error or fails closed if injected
/// randomness, canonical projection, proving, or the prover self-check fails.
pub fn prove_zk_ace_privacy_v1_with_rng<R: TryCryptoRng + ?Sized>(
    public_inputs: &ZkAcePrivacyPublicInputsV1,
    witness: &ZkAcePrivacyWitnessV1,
    randomness: &mut R,
) -> Result<Vec<u8>, ZkAceNativeErrorV1> {
    validate_privacy_public_inputs(public_inputs)?;
    witness
        .validate()
        .map_err(ZkAceNativeErrorV1::InvalidWitness)?;
    let authorization_digest = derive_zk_ace_privacy_authorization_digest(public_inputs)
        .map_err(|_| ZkAceNativeErrorV1::PublicInputsEncoding)?;
    validate_privacy_witness_relation(public_inputs, witness, &authorization_digest)?;
    let relation_inputs = project_air_relation_inputs_v1(public_inputs, authorization_digest)?;
    prove_zk_ace_stark_v1_with_rng(&relation_inputs, witness, randomness).map_err(|error| {
        match error {
            ZkAceStarkError::WitnessRelation => ZkAceNativeErrorV1::WitnessRelationMismatch,
            ZkAceStarkError::RandomnessUnavailable => ZkAceNativeErrorV1::RandomnessUnavailable,
            ZkAceStarkError::RandomnessUnhealthy => ZkAceNativeErrorV1::UnhealthyRandomness,
            error => ZkAceNativeErrorV1::Prover {
                reason: error.to_string(),
            },
        }
    })
}

/// Generate the canonical randomized proof using operating-system entropy.
///
/// # Errors
///
/// Returns the same closed typed failures as
/// [`prove_zk_ace_privacy_v1_with_rng`].
pub fn prove_zk_ace_privacy_v1(
    public_inputs: &ZkAcePrivacyPublicInputsV1,
    witness: &ZkAcePrivacyWitnessV1,
) -> Result<Vec<u8>, ZkAceNativeErrorV1> {
    prove_zk_ace_privacy_v1_with_rng(public_inputs, witness, &mut rand::rngs::OsRng)
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
    let authorization_digest = derive_zk_ace_privacy_authorization_digest(public_inputs)
        .map_err(|_| ZkAceNativeErrorV1::PublicInputsEncoding)?;
    let relation_inputs = project_air_relation_inputs_v1(public_inputs, authorization_digest)?;
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
#[derive(Debug, PartialEq, Eq, Error)]
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
    /// A secret component used the all-zero sentinel.
    #[error("ZK-ACE private witness is invalid: {0}")]
    InvalidWitness(ZkAcePrivacyWitnessValidationErrorV1),
    /// A structurally valid witness does not open the public commitment or
    /// derive the public replay nullifier.
    #[error("ZK-ACE private witness does not satisfy the public commitment/nullifier relation")]
    WitnessRelationMismatch,
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
    /// The injected or operating-system cryptographic source failed.
    #[error("ZK-ACE prover randomness is unavailable")]
    RandomnessUnavailable,
    /// The cryptographic source emitted a catastrophic repeated pattern.
    #[error("ZK-ACE prover randomness failed its health check")]
    UnhealthyRandomness,
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
        NetworkId,
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
    use rand::{TryCryptoRng, TryRngCore};

    use super::*;

    #[derive(Clone, Copy)]
    enum EntropyMode {
        Constant,
        Period(usize),
        PartialFailure,
    }

    #[derive(Debug)]
    struct InjectedEntropyError;

    impl core::fmt::Display for InjectedEntropyError {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter.write_str("injected ZK-ACE entropy failure")
        }
    }

    struct AdversarialRng(EntropyMode);

    impl TryRngCore for AdversarialRng {
        type Error = InjectedEntropyError;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Err(InjectedEntropyError)
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Err(InjectedEntropyError)
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
            match self.0 {
                EntropyMode::Constant => destination.fill(0xA5),
                EntropyMode::Period(period) => {
                    for (index, byte) in destination.iter_mut().enumerate() {
                        *byte = ((index % period) as u8).wrapping_mul(43).wrapping_add(7);
                    }
                }
                EntropyMode::PartialFailure => {
                    for (index, byte) in destination.iter_mut().take(19).enumerate() {
                        *byte = index as u8;
                    }
                    return Err(InjectedEntropyError);
                }
            }
            Ok(())
        }
    }

    impl TryCryptoRng for AdversarialRng {}

    struct PanicEntropyRng;

    impl TryRngCore for PanicEntropyRng {
        type Error = InjectedEntropyError;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            panic!("deterministically invalid ZK-ACE input reached entropy")
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            panic!("deterministically invalid ZK-ACE input reached entropy")
        }

        fn try_fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), Self::Error> {
            panic!("deterministically invalid ZK-ACE input reached entropy")
        }
    }

    impl TryCryptoRng for PanicEntropyRng {}

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive test account");
        AccountId::new(key_pair.public_key().clone())
    }

    fn asset() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("privacy", "universal").expect("test domain"),
            Name::from_str("zkace").expect("test asset"),
        )
    }

    fn public_inputs_and_witness() -> (ZkAcePrivacyPublicInputsV1, ZkAcePrivacyWitnessV1) {
        let witness = ZkAcePrivacyWitnessV1 {
            identity_root: [0x11; 32],
            identity_blinding: [0x22; 32],
            replay_secret: [0x33; 32],
        };
        let network_id = NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
            iroha_data_model::block::BlockHeader,
        >::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x48; 32])
        ));
        let identity_commitment = derive_zk_ace_identity_commitment(
            &witness.identity_root,
            &witness.identity_blinding,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        let statement = ZkAcePqAuthorizationStatementV1 {
            context: PrivacyStatementContextV1 {
                network_id,
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
            public_balance_scope: iroha_data_model::asset::AssetBalanceScope::Global,
            amount: 19,
            authorization_epoch: 7,
            replay_nullifier: PrivacyNullifierV1::new([0; 32]),
        };
        let mut public_inputs = ZkAcePrivacyPublicInputsV1::new(statement, [0x48; 32]);
        let authorization_digest = derive_zk_ace_privacy_authorization_digest(&public_inputs)
            .expect("typed authorization digest");
        let replay_nullifier = derive_zk_ace_replay_nullifier(
            &witness.replay_secret,
            &authorization_digest,
            &network_id,
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
    fn producer_entropy_failures_are_typed_and_catastrophic_patterns_are_rejected() {
        let (public_inputs, witness) = public_inputs_and_witness();
        assert_eq!(
            prove_zk_ace_privacy_v1_with_rng(
                &public_inputs,
                &witness,
                &mut AdversarialRng(EntropyMode::PartialFailure),
            ),
            Err(ZkAceNativeErrorV1::RandomnessUnavailable)
        );
        for mode in [
            EntropyMode::Constant,
            EntropyMode::Period(1),
            EntropyMode::Period(2),
            EntropyMode::Period(4),
            EntropyMode::Period(8),
            EntropyMode::Period(16),
            EntropyMode::Period(32),
        ] {
            assert_eq!(
                prove_zk_ace_privacy_v1_with_rng(
                    &public_inputs,
                    &witness,
                    &mut AdversarialRng(mode),
                ),
                Err(ZkAceNativeErrorV1::UnhealthyRandomness)
            );
        }
    }

    #[test]
    fn deterministic_public_and_witness_failures_precede_entropy_preflight() {
        let (public_inputs, witness) = public_inputs_and_witness();
        let mut invalid_public = public_inputs.clone();
        invalid_public.version ^= 1;
        assert!(matches!(
            prove_zk_ace_privacy_v1_with_rng(&invalid_public, &witness, &mut PanicEntropyRng,),
            Err(ZkAceNativeErrorV1::PublicInputVersionMismatch { .. })
        ));

        let mut invalid_witness = witness;
        invalid_witness.identity_root[0] ^= 1;
        assert_eq!(
            prove_zk_ace_privacy_v1_with_rng(
                &public_inputs,
                &invalid_witness,
                &mut PanicEntropyRng,
            ),
            Err(ZkAceNativeErrorV1::WitnessRelationMismatch)
        );

        let (public_inputs, mut invalid_blinding) = public_inputs_and_witness();
        invalid_blinding.identity_blinding[0] ^= 1;
        assert_eq!(
            prove_zk_ace_privacy_v1_with_rng(
                &public_inputs,
                &invalid_blinding,
                &mut PanicEntropyRng,
            ),
            Err(ZkAceNativeErrorV1::WitnessRelationMismatch)
        );

        let (public_inputs, mut invalid_replay_secret) = public_inputs_and_witness();
        invalid_replay_secret.replay_secret[0] ^= 1;
        assert_eq!(
            prove_zk_ace_privacy_v1_with_rng(
                &public_inputs,
                &invalid_replay_secret,
                &mut PanicEntropyRng,
            ),
            Err(ZkAceNativeErrorV1::WitnessRelationMismatch)
        );
    }

    #[test]
    fn every_typed_public_binding_and_sampled_corruption_rejects() {
        let (public_inputs, proof) = fixture();
        let mutations: [fn(&mut ZkAcePrivacyPublicInputsV1); 19] = [
            |value| value.version ^= 1,
            |value| value.genesis_hash[0] ^= 1,
            |value| value.statement.identity_commitment.0[0] ^= 1,
            |value| value.statement.context.transaction_intent_digest.0[0] ^= 1,
            |value| {
                value.statement.context.network_id =
                    NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                        iroha_data_model::block::BlockHeader,
                    >::from_untyped_unchecked(
                        iroha_crypto::Hash::prehashed([0x49; 32])
                    ))
            },
            |value| value.statement.context.parameter_id.0[0] ^= 1,
            |value| value.statement.context.parameter_digest.0[0] ^= 1,
            |value| value.statement.context.verifier_digest.0[0] ^= 1,
            |value| value.statement.context.statement_schema_digest.0[0] ^= 1,
            |value| value.statement.context.engine_manifest_digest.0[0] ^= 1,
            |value| value.statement.replay_nullifier.0[0] ^= 1,
            |value| value.statement.policy_id.0[0] ^= 1,
            |value| value.statement.policy_digest.0[0] ^= 1,
            |value| value.statement.amount += 1,
            |value| value.statement.authorization_epoch += 1,
            |value| value.statement.context.action_index = 1,
            |value| value.statement.source = account(3),
            |value| value.statement.destination = account(4),
            |value| {
                value.statement.asset_definition_id = AssetDefinitionId::derive_from_components(
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
