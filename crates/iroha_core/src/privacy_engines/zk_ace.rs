//! Fail-closed first-release ZK-ACE authorization engine.
//!
//! The low-level proof wire is the dedicated masked execution-trace STARK in `zk_ace_stark`. Its
//! AIR proves twelve public outputs: six independently initialized, independently constanted
//! Poseidon-x7 Goldilocks lanes for the identity commitment and six separately domain-bound lanes
//! for the replay nullifier. It commits an independent full-space zero-knowledge mask before
//! batching challenges, links the AIR at one quartic-extension DEEP point, runs Fp4 FRI, and carries
//! no caller-selected backend, verifier key, parameter record, or legacy generic envelope.
//!
//! Production proving, verification, and profile activation remain fail-closed. The surrounding
//! STARK still uses SHA-256 for Merkle commitments, Fiat--Shamir challenges, FRI transcripts, and
//! query selection; its compiled 128-bit certificate is work-normalized only in the classical
//! random-oracle model. The `pq_authorization` relation name does not assert a quantum-random-oracle
//! reduction for that proof system.
#[cfg(test)]
use super::prover_randomness::TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1;
use super::zk_ace_stark::{
    AIR_PUBLIC_TRANSCRIPT_SCHEMA_V1, COMPILED_STARK_PROFILE_DESCRIPTOR_V1, MAX_PROOF_BYTES,
    MAX_ROM_QUERY_LOG2_V1, PROVABLE_SOUNDNESS_BITS_V1, ZkAceAirRelationInputsV1, ZkAceStarkError,
    prove_zk_ace_stark_v1_with_rng, verify_zk_ace_stark_v1,
};
#[cfg(test)]
use iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V1_CIRCUIT_ID;
use iroha_data_model::{
    NetworkId,
    privacy::{
        GoldilocksDigest384V1, PrivacyConsensusLimitsV1, PrivacyStatementV1,
        PrivacyZkAceIdentityCommitmentV1, PrivacyZkAceReplayNullifierV1,
    },
    zk::{
        ZK_ACE_PQ_AUTHORIZATION_V1_ACTION_TRANSFER, ZK_ACE_PQ_AUTHORIZATION_V1_DOMAIN_TAG,
        ZkAcePrivacyPublicInputsV1, derive_zk_ace_identity_commitment,
        derive_zk_ace_privacy_authorization_digest, derive_zk_ace_replay_nullifier,
        derive_zk_ace_transfer_digest,
    },
};
use rand::TryCryptoRng;
/// Fallible cryptographic RNG contract accepted by the native prover.
pub use rand::TryCryptoRng as ZkAceTryCryptoRngV1;
/// Fallible RNG core contract re-exported for deterministic/adversarial tests
/// without forcing transaction-builder crates to depend on `rand`.
pub use rand::TryRngCore as ZkAceTryRngCoreV1;
#[cfg(test)]
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use zeroize::Zeroize;
/// Secret witness shape retained for the fail-closed native ZK-ACE engine.
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
    /// Derive the six-lane public identity commitment under its fixed domain.
    #[must_use]
    pub fn identity_commitment_v1(&self) -> PrivacyZkAceIdentityCommitmentV1 {
        derive_zk_ace_identity_commitment(
            &self.identity_root,
            &self.identity_blinding,
            ZK_ACE_PQ_AUTHORIZATION_V1_DOMAIN_TAG,
        )
    }
    /// Derive the typed replay nullifier without exposing the replay secret.
    #[must_use]
    pub fn replay_nullifier_v1(
        &self,
        authorization_digest: &GoldilocksDigest384V1,
        network_id: &NetworkId,
    ) -> PrivacyZkAceReplayNullifierV1 {
        derive_zk_ace_replay_nullifier(
            &self.replay_secret,
            authorization_digest,
            network_id,
            ZK_ACE_PQ_AUTHORIZATION_V1_ACTION_TRANSFER,
            ZK_ACE_PQ_AUTHORIZATION_V1_DOMAIN_TAG,
        )
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
/// Whether ZK-ACE is eligible for production proving, verification, or activation.
///
/// The commitment relation now exposes six independent lanes per public digest,
/// but the enclosing STARK has not completed the required qROM migration,
/// exact rational security calculation, and independent review.
pub const ZK_ACE_FULL_ENGINE_AVAILABLE_V1: bool = false;
/// Release blocker that must be resolved before enabling the ZK-ACE engine.
pub const ZK_ACE_REQUIRED_QROM_QUALIFICATION_V1: &[u8] = b"replace-stark-sha256-merkle-fiat-shamir-fri-and-query-transcripts-with-goldilocks-digest384-v1:binary-fri-blowup8:query-count-divisible-by8-and-at-least64:exact-rational-qrom-bound-at-least128-bits:independent-review-required";
/// Source and relation description frozen into the compiled profile.
pub const ZK_ACE_SOURCE_PROFILE_V1: &[u8] = b"iroha-native-rust:zk-ace:typed-statement+trusted-genesis:type-name-independent-ordered-length-framed-public-transcript:private-witness:masked-poseidon-x7-execution-trace:goldilocks-digest384-v1:identity-lanes6-independent:replay-lanes6-independent:typed-role-and-phase-domains:activation-disabled-pending-qrom-stark-qualification:fp4-deep-ali:independent-pre-batching-fri-mask:fp4-fri:producer=preflight+rand0.9-trycrypto-fixed64-reservoir-zeroize-poison-error-or-unwind+self-verify:v1";
/// Exact native proof wire description frozen into the compiled profile.
pub const ZK_ACE_PROOF_WIRE_V1: &[u8] =
    b"ZKA1:fixed-shape-big-endian:1341142:strict-exact:no-lengths:no-generic-envelope";
/// Exact low-level AIR relation schema frozen into the compiled profile.
pub const ZK_ACE_AIR_RELATION_SCHEMA_V1: &[u8] = AIR_PUBLIC_TRANSCRIPT_SCHEMA_V1;
/// Exact typed authorization projection frozen into the compiled profile.
pub const ZK_ACE_AUTHORIZATION_PROJECTION_V1: &[u8] = b"norito:zk-ace-pq-authorization-statement-v1:replay-nullifier-zero|transaction-intent-bound|trusted-genesis-bound";
/// Canonical six-lane parameter-asset digest consumed through `fastpq_prover`.
///
/// The SHA3-256 value authenticates all six generated initial states and round
/// constants together with the shared MDS matrix.
pub const ZK_ACE_DIGEST384_PARAMETER_SHA3_256_V1: &str =
    "84c5055b47cc7289835e0a5f31d4563849244ffddbf51f5d67b1db95222ce3e6";
/// Exact independent-lane Poseidon profile used by the native hash and AIR.
pub const ZK_ACE_POSEIDON_PROFILE_V1: &[u8] = b"poseidon-x7-goldilocks-digest384:lanes6-independent:width3:rate2:capacity1:full8:partial57:parameter-generator=shake256-rejection-sampling-u64le-below-goldilocks-v1:parameters-sha3-256=84c5055b47cc7289835e0a5f31d4563849244ffddbf51f5d67b1db95222ce3e6";
/// Native and consensus proof byte ceiling.
pub const ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1: u32 = MAX_PROOF_BYTES as u32;
/// Theorem-backed classical-ROM, work-normalized soundness of the profile.
///
/// This is not a qROM security claim.
pub const ZK_ACE_PROVABLE_SOUNDNESS_BITS_V1: u16 = PROVABLE_SOUNDNESS_BITS_V1;
/// Maximum base-two random-oracle query-work exponent covered by that bound.
pub const ZK_ACE_MAX_ROM_QUERY_LOG2_V1: u8 = MAX_ROM_QUERY_LOG2_V1;
/// Frozen digest of every fail-closed verifier-profile field below.
///
/// This digest authenticates the profile for regression testing; it is not
/// an activation credential while [`ZK_ACE_FULL_ENGINE_AVAILABLE_V1`] is false.
pub const ZK_ACE_COMPILED_PROFILE_DIGEST_V1: [u8; 32] = [
    0x4d, 0x98, 0x1d, 0xcb, 0xe9, 0xd9, 0x26, 0xf5, 0x45, 0xa6, 0xf4, 0x86, 0x96, 0x21, 0x2a, 0xaa,
    0xa8, 0x3b, 0xb1, 0x94, 0x4f, 0xbf, 0x6e, 0xc7, 0xc8, 0x42, 0xc4, 0x64, 0xba, 0x35, 0x9a, 0x3d,
];
/// Return the frozen digest of the exact fail-closed verifier profile.
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
    hash_field(
        &mut hasher,
        ZK_ACE_DIGEST384_PARAMETER_SHA3_256_V1.as_bytes(),
    );
    hash_field(&mut hasher, ZK_ACE_POSEIDON_PROFILE_V1);
    hash_field(&mut hasher, ZK_ACE_REQUIRED_QROM_QUALIFICATION_V1);
    hash_field(
        &mut hasher,
        ZK_ACE_PQ_AUTHORIZATION_V1_CIRCUIT_ID.as_bytes(),
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
    authorization_digest: GoldilocksDigest384V1,
) -> Result<ZkAceAirRelationInputsV1, ZkAceNativeErrorV1> {
    let statement = &public_inputs.statement;
    let transfer_digest = derive_zk_ace_transfer_digest(
        &statement.source,
        &statement.destination,
        &statement.asset_definition_id,
        statement.amount,
        &statement.context.network_id,
        ZK_ACE_PQ_AUTHORIZATION_V1_ACTION_TRANSFER,
        statement.policy_digest.as_bytes(),
    )
    .map_err(|_| ZkAceNativeErrorV1::PublicInputsEncoding)?;
    Ok(ZkAceAirRelationInputsV1::transparent_transfer(
        statement.identity_commitment.into_bytes(),
        transfer_digest.to_le_bytes(),
        authorization_digest.to_le_bytes(),
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
    authorization_digest: &GoldilocksDigest384V1,
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
/// Reject production proving while the ZK-ACE engine is unqualified.
///
/// # Errors
///
/// Returns [`ZkAceNativeErrorV1::EngineUnavailable`] before validation or
/// entropy use until the enclosing STARK has completed qROM qualification.
pub fn prove_zk_ace_privacy_v1_with_rng<R: TryCryptoRng + ?Sized>(
    public_inputs: &ZkAcePrivacyPublicInputsV1,
    witness: &ZkAcePrivacyWitnessV1,
    randomness: &mut R,
) -> Result<Vec<u8>, ZkAceNativeErrorV1> {
    if !ZK_ACE_FULL_ENGINE_AVAILABLE_V1 {
        return Err(ZkAceNativeErrorV1::EngineUnavailable);
    }
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
/// Reject production proving before requesting operating-system entropy.
///
/// # Errors
///
/// Returns the same fail-closed result as
/// [`prove_zk_ace_privacy_v1_with_rng`].
pub fn prove_zk_ace_privacy_v1(
    public_inputs: &ZkAcePrivacyPublicInputsV1,
    witness: &ZkAcePrivacyWitnessV1,
) -> Result<Vec<u8>, ZkAceNativeErrorV1> {
    prove_zk_ace_privacy_v1_with_rng(public_inputs, witness, &mut rand::rngs::OsRng)
}
/// Reject production verification while the ZK-ACE engine is unqualified.
///
/// # Errors
///
/// Returns [`ZkAceNativeErrorV1::EngineUnavailable`] before parsing any proof.
pub fn verify_zk_ace_privacy_v1(
    public_inputs: &ZkAcePrivacyPublicInputsV1,
    proof: &[u8],
    caller_max_proof_bytes: u32,
) -> Result<(), ZkAceNativeErrorV1> {
    if !ZK_ACE_FULL_ENGINE_AVAILABLE_V1 {
        return Err(ZkAceNativeErrorV1::EngineUnavailable);
    }
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
    PrivacyStatementV1::ZkAcePqAuthorizationV1(public_inputs.statement.clone())
        .validate(&PrivacyConsensusLimitsV1::taira_default())
        .map_err(|_| ZkAceNativeErrorV1::InvalidStatement)
}
/// Native ZK-ACE construction or verification failure.
#[derive(Debug, PartialEq, Eq, Error)]
pub enum ZkAceNativeErrorV1 {
    /// The enclosing proof system has not completed its release qualification.
    #[error("ZK-ACE native engine is unavailable pending qROM STARK qualification")]
    EngineUnavailable,
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
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        NetworkId,
        account::AccountId,
        asset::AssetDefinitionId,
        domain::DomainId,
        name::Name,
        privacy::{
            PrivacyEngineManifestDigestV1, PrivacyParameterDigestV1, PrivacyParameterIdV1,
            PrivacyPolicyDigestV1, PrivacyPolicyIdV1, PrivacyStatementContextV1,
            PrivacyStatementSchemaDigestV1, PrivacyTransactionIntentDigestV1,
            PrivacyVerifierDigestV1, ZkAcePqAuthorizationStatementV1,
        },
        zk::{
            ZK_ACE_PQ_AUTHORIZATION_V1_ACTION_TRANSFER, ZK_ACE_PQ_AUTHORIZATION_V1_DOMAIN_TAG,
            derive_zk_ace_identity_commitment, derive_zk_ace_privacy_authorization_digest,
            derive_zk_ace_replay_nullifier,
        },
    };
    use rand::{TryCryptoRng, TryRngCore};
    use std::str::FromStr as _;
    #[derive(Debug)]
    struct InjectedEntropyError;
    impl core::fmt::Display for InjectedEntropyError {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter.write_str("injected ZK-ACE entropy failure")
        }
    }
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
            ZK_ACE_PQ_AUTHORIZATION_V1_DOMAIN_TAG,
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
            identity_commitment,
            policy_id: PrivacyPolicyIdV1::new([0x46; 32]),
            policy_digest: PrivacyPolicyDigestV1::new([0x47; 32]),
            source: account(1),
            destination: account(2),
            asset_definition_id: asset(),
            public_balance_scope: iroha_data_model::asset::AssetBalanceScope::Global,
            amount: 19,
            authorization_epoch: 7,
            replay_nullifier: Default::default(),
        };
        let mut public_inputs = ZkAcePrivacyPublicInputsV1::new(statement, [0x48; 32]);
        let authorization_digest = derive_zk_ace_privacy_authorization_digest(&public_inputs)
            .expect("typed authorization digest");
        let replay_nullifier = derive_zk_ace_replay_nullifier(
            &witness.replay_secret,
            &authorization_digest,
            &network_id,
            ZK_ACE_PQ_AUTHORIZATION_V1_ACTION_TRANSFER,
            ZK_ACE_PQ_AUTHORIZATION_V1_DOMAIN_TAG,
        );
        public_inputs.statement.replay_nullifier = replay_nullifier;
        (public_inputs, witness)
    }
    #[test]
    fn compiled_profile_digest_matches_every_exact_native_parameter() {
        assert_eq!(
            fastpq_prover::fastpq_isi_v1::GOLDILOCKS_DIGEST384_PARAMETER_SHA3_256_V1,
            [
                0x84, 0xc5, 0x05, 0x5b, 0x47, 0xcc, 0x72, 0x89, 0x83, 0x5e, 0x0a, 0x5f, 0x31, 0xd4,
                0x56, 0x38, 0x49, 0x24, 0x4f, 0xfd, 0xdb, 0xf5, 0x1f, 0x5d, 0x67, 0xb1, 0xdb, 0x95,
                0x22, 0x2c, 0xe3, 0xe6,
            ]
        );
        assert_eq!(
            ZK_ACE_DIGEST384_PARAMETER_SHA3_256_V1,
            "84c5055b47cc7289835e0a5f31d4563849244ffddbf51f5d67b1db95222ce3e6"
        );
        assert!(
            core::str::from_utf8(ZK_ACE_POSEIDON_PROFILE_V1)
                .expect("profile is UTF-8")
                .ends_with(ZK_ACE_DIGEST384_PARAMETER_SHA3_256_V1)
        );
        assert!(!ZK_ACE_FULL_ENGINE_AVAILABLE_V1);
        assert!(
            core::str::from_utf8(ZK_ACE_REQUIRED_QROM_QUALIFICATION_V1)
                .expect("qualification blocker is UTF-8")
                .contains("qrom")
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
        let mut replay_bytes = changed.statement.replay_nullifier.into_bytes();
        replay_bytes[0] ^= 1;
        changed.statement.replay_nullifier =
            PrivacyZkAceReplayNullifierV1::from_le_bytes(replay_bytes)
                .expect("low-byte mutation remains canonical");
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
    fn production_entrypoints_fail_before_validation_entropy_or_proof_work() {
        let (public_inputs, witness) = public_inputs_and_witness();
        let mut invalid_public = public_inputs.clone();
        invalid_public.version ^= 1;
        assert_eq!(
            prove_zk_ace_privacy_v1_with_rng(&invalid_public, &witness, &mut PanicEntropyRng,),
            Err(ZkAceNativeErrorV1::EngineUnavailable)
        );
        assert_eq!(
            prove_zk_ace_privacy_v1_with_rng(&public_inputs, &witness, &mut PanicEntropyRng),
            Err(ZkAceNativeErrorV1::EngineUnavailable)
        );
        assert_eq!(
            verify_zk_ace_privacy_v1(&invalid_public, &[], ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1,),
            Err(ZkAceNativeErrorV1::EngineUnavailable)
        );
        assert_eq!(
            verify_zk_ace_privacy_v1(&public_inputs, &[1], ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1,),
            Err(ZkAceNativeErrorV1::EngineUnavailable)
        );
    }
}
