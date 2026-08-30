//! Native ZK-ACE authorization candidate.
//!
//! The low-level candidate proof wire is the dedicated masked execution-trace STARK in
//! `zk_ace_stark`. It proves both dense-MDS Poseidon `x^7` relations, commits an independent
//! full-space zero-knowledge mask before batching challenges, links the AIR at one quartic-extension
//! DEEP point, runs Fp4 FRI, and carries no caller-selected backend, verifier key, parameter record,
//! or legacy generic envelope. Identity and replay digests each use four fresh, explicitly
//! domain-separated capacity-1 sponges and take one canonical state-zero word from each lane, so
//! the compiled classical-ROM commitment-binding target is 128 bits.
//! Production proving, verification, and activation remain fail-closed until the
//! frozen release-evidence stages are actually captured and reviewed.
//!
//! The compiled 128-bit Fiat--Shamir certificate is work-normalized in the
//! classical random-oracle model.  The `pq_authorization` relation name does
//! not assert an additional quantum-random-oracle reduction for this STARK.
#[cfg(test)]
use super::prover_randomness::TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1;
use super::zk_ace_stark::{
    AIR_PUBLIC_TRANSCRIPT_SCHEMA_V1, COMPILED_STARK_PROFILE_DESCRIPTOR_V1, MAX_PROOF_BYTES,
    MAX_ROM_QUERY_LOG2_V1, PROVABLE_SOUNDNESS_BITS_V1, ZkAceAirRelationInputsV1, ZkAceStarkError,
    prove_zk_ace_stark_v1_with_rng, verify_zk_ace_stark_v1,
};
#[cfg(test)]
use iroha_data_model::zk::{
    ZK_ACE_POSEIDON_DIGEST_LANE_DOMAINS_V2, ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
    ZK_ACE_TRANSFER_DIGEST_SCHEMA_V1,
};
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
/// Fallible cryptographic RNG contract accepted by the native prover.
pub use rand::TryCryptoRng as ZkAceTryCryptoRngV1;
/// Fallible RNG core contract re-exported for deterministic/adversarial tests
/// without forcing transaction-builder crates to depend on `rand`.
pub use rand::TryRngCore as ZkAceTryRngCoreV1;
#[cfg(test)]
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use zeroize::Zeroize;
/// Secret witness shape for the native ZK-ACE engine.
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
    /// Derive the public identity commitment under its fixed four-lane domain profile.
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
pub const ZK_ACE_PRIVACY_TRANSCRIPT_LABEL_V1: &str = "iroha:privacy:zk-ace:transparent-stark:v2";
/// Canonical SHA-256 pins of the four reviewed release-evidence stages, in
/// `PrivacyReleaseCaseKindV1::ALL` order.
///
/// Each pin is SHA-256 over the canonical Norito encoding of the complete
/// `PrivacyReleaseStageEvidenceV1`, including its exact proof bytes, public
/// statement digest, resource facts, and immutable protocol descriptor. Zero
/// is an explicit fail-closed sentinel, never a wildcard; the four admitted
/// stage digests must also be pairwise distinct.
pub const ZK_ACE_RELEASE_STAGE_EVIDENCE_SHA256_V2: [[u8; 32]; 4] = [
    [
        0x0e, 0x43, 0x21, 0x9c, 0x21, 0xb1, 0x0d, 0x17, 0xfe, 0x7a, 0x77, 0xe3, 0x95, 0x0c, 0x32,
        0x43, 0x62, 0xa2, 0x72, 0x43, 0xa0, 0x96, 0x34, 0xa3, 0x68, 0xc6, 0xb4, 0x05, 0xca, 0x30,
        0x6b, 0x88,
    ],
    [
        0xff, 0xd4, 0x79, 0xf0, 0xa2, 0xc5, 0x24, 0xea, 0x15, 0xfe, 0x72, 0x8b, 0xc6, 0xfd, 0x8e,
        0x13, 0xf0, 0x01, 0x80, 0x9f, 0x17, 0xb0, 0x1a, 0xec, 0xdd, 0xa7, 0x64, 0x55, 0x6f, 0xb8,
        0x43, 0x9b,
    ],
    [
        0x4d, 0xd3, 0x62, 0x8a, 0x69, 0x76, 0x39, 0xb2, 0x50, 0x1d, 0x4a, 0x8d, 0x2b, 0x39, 0x58,
        0xeb, 0x20, 0x8d, 0x56, 0x04, 0x02, 0x1d, 0x32, 0x9b, 0x03, 0x00, 0xf1, 0x1d, 0x13, 0xb8,
        0xd7, 0x86,
    ],
    [
        0x49, 0x27, 0xb4, 0xa3, 0x7e, 0xd6, 0x01, 0x2f, 0xb8, 0x78, 0xfa, 0x99, 0x1d, 0x84, 0xca,
        0x4e, 0xa6, 0x8f, 0x96, 0xaf, 0x91, 0xd3, 0xef, 0xd6, 0x1a, 0x41, 0x3d, 0x32, 0xc8, 0x08,
        0x80, 0xff,
    ],
];
/// Canonical SHA-256 of the reviewed committed-network semantic receipt.
///
/// The receipt must bind the exact source profile and four-stage evidence
/// pins, a successful policy-authorized transfer, replay-nullifier
/// consumption, the terminal committed height, and a rejected replay observed
/// through authenticated state queries. Zero keeps public release incomplete;
/// it does not prevent a bounded post-stage-pin semantic canary.
pub const ZK_ACE_RELEASE_NETWORK_SEMANTIC_SHA256_V2: [u8; 32] = [0; 32];
const fn nonzero_release_digest_v2(digest: &[u8; 32]) -> bool {
    let mut index = 0;
    while index < digest.len() {
        if digest[index] != 0 {
            return true;
        }
        index += 1;
    }
    false
}
const fn same_release_digest_v2(left: &[u8; 32], right: &[u8; 32]) -> bool {
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}
const fn release_stage_pins_complete_v2(pins: &[[u8; 32]; 4]) -> bool {
    let mut index = 0;
    while index < pins.len() {
        if !nonzero_release_digest_v2(&pins[index]) {
            return false;
        }
        let mut prior = 0;
        while prior < index {
            if same_release_digest_v2(&pins[index], &pins[prior]) {
                return false;
            }
            prior += 1;
        }
        index += 1;
    }
    true
}
/// Whether all four independently reviewed native stage pins are populated
/// and pairwise distinct.
#[must_use]
pub const fn zk_ace_release_evidence_pins_complete_v2() -> bool {
    release_stage_pins_complete_v2(&ZK_ACE_RELEASE_STAGE_EVIDENCE_SHA256_V2)
}
const fn public_release_pins_complete_with_network_semantic_v2(
    network_semantic_sha256: &[u8; 32],
) -> bool {
    if !zk_ace_release_evidence_pins_complete_v2()
        || !nonzero_release_digest_v2(network_semantic_sha256)
    {
        return false;
    }
    let mut index = 0;
    while index < ZK_ACE_RELEASE_STAGE_EVIDENCE_SHA256_V2.len() {
        if same_release_digest_v2(
            network_semantic_sha256,
            &ZK_ACE_RELEASE_STAGE_EVIDENCE_SHA256_V2[index],
        ) {
            return false;
        }
        index += 1;
    }
    true
}
/// Whether both native evidence and the committed-network semantic canary are pinned.
#[must_use]
pub const fn zk_ace_public_release_pins_complete_v2() -> bool {
    public_release_pins_complete_with_network_semantic_v2(
        &ZK_ACE_RELEASE_NETWORK_SEMANTIC_SHA256_V2,
    )
}
/// Whether the explicitly non-shipping committed-network candidate may run.
///
/// The candidate corridor exists only between completion of the four native
/// evidence stages and admission of their reviewed network-semantic receipt.
/// Callers must additionally be compiled with `privacy-release-evidence`;
/// this predicate never enables an ordinary prover, verifier, wallet, or
/// governance profile.
#[must_use]
pub const fn zk_ace_nonshipping_release_candidate_available_v2() -> bool {
    zk_ace_release_evidence_pins_complete_v2() && !zk_ace_public_release_pins_complete_v2()
}
/// Whether ZK-ACE is eligible for production proving, verification, or activation.
///
/// This is derived from both the four reviewed native-stage pins and the
/// reviewed committed-network receipt pin. The isolated non-shipping
/// candidate corridor is exposed separately and never changes this value.
pub const ZK_ACE_FULL_ENGINE_AVAILABLE_V1: bool = zk_ace_public_release_pins_complete_v2();
/// Exact independently-laned public commitment construction.
pub const ZK_ACE_COMMITMENT_BINDING_PROFILE_V2: &[u8] = b"four-independent-explicit-equal-length-domain-separated-dense-mds-poseidon-x7-invocations:lane0=zk-ace.poseidon-x7.digest-lane-0.v2:lane1=zk-ace.poseidon-x7.digest-lane-1.v2:lane2=zk-ace.poseidon-x7.digest-lane-2.v2:lane3=zk-ace.poseidon-x7.digest-lane-3.v2:logical-domain-length-framed-as-part0:identity-domain=zk-ace.identity-commitment.v1:replay-domain=zk-ace.replay-nullifier.v1:transfer-domain=zk-ace.transparent-transfer.v1:authorization-domain=zk-ace.privacy-authorization.v1:air-public-domain=iroha:privacy:zk-ace:air-public-digest:v2:one-canonical-state0-u64le-output-each:classical-rom-generic-collision-target-128-bits:qrom-not-claimed";
/// Immutable release-readiness requirement; this is not release evidence.
pub const ZK_ACE_RELEASE_READINESS_BLOCKER_V2: &[u8] = b"release-readiness-requires-reviewed-sha256-pins:positive-canonical-end-to-end+public-statement-binding-mutation+proof-corruption-and-truncation+maximum-shape-resource+committed-policy-transfer+FindPrivacyZkAceReplayNullifierV1-finalized-NetworkId-height-block-hash-provenance+replay-rejection:zero-pin-means-unavailable";
/// Source and relation description frozen into the compiled profile.
pub const ZK_ACE_SOURCE_PROFILE_V1: &[u8] = b"iroha-native-rust:zk-ace:typed-statement+trusted-genesis:type-name-independent-ordered-length-framed-public-transcript:private-witness:masked-dense-mds-poseidon-x7-execution-trace:four-independent-equal-length-domain-lanes:one-state0-u64le-output-per-lane:classical-rom-binding128:qrom-not-claimed:availability-gated-by-reviewed-release-evidence-pins:fp4-deep-ali:independent-pre-batching-fri-mask:fp4-fri:producer=preflight+rand0.9-trycrypto-fixed64-reservoir-zeroize-poison-error-or-unwind+self-verify:v2";
/// Exact native proof wire description frozen into the compiled profile.
pub const ZK_ACE_PROOF_WIRE_V1: &[u8] =
    b"ZKA2:fixed-shape-big-endian:1427158:strict-exact:no-lengths:no-generic-envelope";
/// Exact low-level AIR relation schema frozen into the compiled profile.
pub const ZK_ACE_AIR_RELATION_SCHEMA_V1: &[u8] = AIR_PUBLIC_TRANSCRIPT_SCHEMA_V1;
/// Exact typed authorization projection frozen into the compiled profile.
pub const ZK_ACE_AUTHORIZATION_PROJECTION_V1: &[u8] = b"norito:zk-ace-pq-authorization-statement-v1:replay-nullifier-zero|transaction-intent-bound|trusted-genesis-bound";
/// Canonical Poseidon constant manifest consumed through `fastpq_prover`.
///
/// The manifest authenticates only round constants and the MDS matrix. The
/// construction and S-box exponent are authenticated separately by
/// [`ZK_ACE_POSEIDON_PROFILE_V1`].
pub const ZK_ACE_POSEIDON_MANIFEST_SHA256_V1: &str =
    "99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21";
/// Exact Poseidon permutation and four-lane digest profile used by the native hash and AIR.
pub const ZK_ACE_POSEIDON_PROFILE_V1: &[u8] = b"dense-mds-poseidon:goldilocks:x7:width3:rate2:capacity1:full8:partial57:v1:constants-sha256=99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21:digest-profile=four-independent-equal-length-explicit-domain-lanes-v2:one-state0-u64le-each";
/// Native and consensus proof byte ceiling.
pub const ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1: u32 = MAX_PROOF_BYTES as u32;
/// Theorem-backed classical-ROM, work-normalized soundness of the profile.
///
/// This is not a qROM security claim.
pub const ZK_ACE_PROVABLE_SOUNDNESS_BITS_V1: u16 = PROVABLE_SOUNDNESS_BITS_V1;
/// Maximum base-two random-oracle query-work exponent covered by that bound.
pub const ZK_ACE_MAX_ROM_QUERY_LOG2_V1: u8 = MAX_ROM_QUERY_LOG2_V1;
/// Frozen digest of every candidate verifier-profile field below.
///
/// This authenticates deterministic candidate material; it is not an
/// activation credential while [`ZK_ACE_FULL_ENGINE_AVAILABLE_V1`] is false.
pub const ZK_ACE_COMPILED_PROFILE_DIGEST_V1: [u8; 32] = [
    0x60, 0x04, 0x4d, 0x1b, 0xd4, 0x20, 0x6e, 0xaa, 0x8b, 0xd9, 0x5e, 0xe5, 0x52, 0xbf, 0xa0, 0xbe,
    0x15, 0x4c, 0x1f, 0x5f, 0x9e, 0x78, 0xc5, 0xe7, 0x48, 0x9f, 0xf1, 0xf1, 0x7c, 0xe6, 0x04, 0x07,
];
/// Return the frozen digest of the exact candidate verifier profile.
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
    hash_field(&mut hasher, b"iroha.privacy.zk-ace.compiled-profile.v2");
    hash_field(&mut hasher, ZK_ACE_SOURCE_PROFILE_V1);
    hash_field(&mut hasher, TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1);
    hash_field(&mut hasher, ZK_ACE_PROOF_WIRE_V1);
    hash_field(&mut hasher, ZK_ACE_AIR_RELATION_SCHEMA_V1);
    hash_field(&mut hasher, ZK_ACE_TRANSFER_DIGEST_SCHEMA_V1);
    hash_field(&mut hasher, ZK_ACE_AUTHORIZATION_PROJECTION_V1);
    hash_field(&mut hasher, ZK_ACE_POSEIDON_MANIFEST_SHA256_V1.as_bytes());
    hash_field(&mut hasher, ZK_ACE_POSEIDON_PROFILE_V1);
    hash_field(&mut hasher, ZK_ACE_COMMITMENT_BINDING_PROFILE_V2);
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
/// Reject production proving until the complete public release-pin gate is closed.
///
/// # Errors
///
/// Returns [`ZkAceNativeErrorV1::EngineUnavailable`] before validation or
/// entropy use while the public release-pin gate remains open.
pub fn prove_zk_ace_privacy_v1_with_rng<R: TryCryptoRng + ?Sized>(
    public_inputs: &ZkAcePrivacyPublicInputsV1,
    witness: &ZkAcePrivacyWitnessV1,
    randomness: &mut R,
) -> Result<Vec<u8>, ZkAceNativeErrorV1> {
    if !ZK_ACE_FULL_ENGINE_AVAILABLE_V1 {
        return Err(ZkAceNativeErrorV1::EngineUnavailable);
    }
    prove_zk_ace_privacy_candidate_v1_with_rng(public_inputs, witness, randomness)
}
fn prove_zk_ace_privacy_candidate_v1_with_rng<R: TryCryptoRng + ?Sized>(
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
/// Prove the immutable ZK-ACE candidate inside the non-shipping evidence lane.
///
/// This bypasses only the reviewed-evidence pin check. It executes the exact
/// validation, witness relation, native prover, and self-verification code used
/// by the production entry point and is not compiled into ordinary nodes.
#[cfg(feature = "privacy-release-evidence")]
pub(crate) fn prove_zk_ace_release_candidate_v2_with_rng<R: TryCryptoRng + ?Sized>(
    public_inputs: &ZkAcePrivacyPublicInputsV1,
    witness: &ZkAcePrivacyWitnessV1,
    randomness: &mut R,
) -> Result<Vec<u8>, ZkAceNativeErrorV1> {
    prove_zk_ace_privacy_candidate_v1_with_rng(public_inputs, witness, randomness)
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
/// Reject production verification until the complete public release-pin gate is closed.
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
    verify_zk_ace_privacy_candidate_v1(public_inputs, proof, caller_max_proof_bytes)
}
fn verify_zk_ace_privacy_candidate_v1(
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
/// Verify the immutable ZK-ACE candidate inside the non-shipping evidence lane.
///
/// This bypasses only the reviewed-evidence pin check and otherwise joins the
/// exact production verifier implementation.
#[cfg(feature = "privacy-release-evidence")]
pub(crate) fn verify_zk_ace_release_candidate_v2(
    public_inputs: &ZkAcePrivacyPublicInputsV1,
    proof: &[u8],
    caller_max_proof_bytes: u32,
) -> Result<(), ZkAceNativeErrorV1> {
    verify_zk_ace_privacy_candidate_v1(public_inputs, proof, caller_max_proof_bytes)
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
    /// The candidate has not completed its frozen operator release evidence.
    #[error("ZK-ACE native engine is unavailable pending operator release evidence")]
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
    use std::str::FromStr as _;

    #[test]
    fn release_stage_pin_gate_rejects_zero_and_duplicate_digests() {
        let distinct = [[1; 32], [2; 32], [3; 32], [4; 32]];
        assert!(release_stage_pins_complete_v2(&distinct));
        let mut contains_zero = distinct;
        contains_zero[2] = [0; 32];
        assert!(!release_stage_pins_complete_v2(&contains_zero));
        let mut contains_duplicate = distinct;
        contains_duplicate[3] = contains_duplicate[1];
        assert!(!release_stage_pins_complete_v2(&contains_duplicate));
        assert!(zk_ace_release_evidence_pins_complete_v2());
        assert_eq!(ZK_ACE_RELEASE_NETWORK_SEMANTIC_SHA256_V2, [0; 32]);
        assert!(!public_release_pins_complete_with_network_semantic_v2(
            &[0; 32]
        ));
        // A synthetic digest exercises the predicate only; the committed pin
        // above deliberately remains the zero sentinel until receipt review.
        assert!(public_release_pins_complete_with_network_semantic_v2(
            &[0xa5; 32]
        ));
        assert!(!public_release_pins_complete_with_network_semantic_v2(
            &ZK_ACE_RELEASE_STAGE_EVIDENCE_SHA256_V2[0]
        ));
        assert!(!zk_ace_public_release_pins_complete_v2());
        assert!(zk_ace_nonshipping_release_candidate_available_v2());
        assert!(!ZK_ACE_FULL_ENGINE_AVAILABLE_V1);
    }

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
    #[test]
    fn compiled_profile_digest_matches_every_exact_native_parameter() {
        const EXPECTED_PERMUTATION_PROFILE_ID: &str =
            "dense-mds-poseidon:goldilocks:x7:width3:rate2:full8:partial57:v1";

        assert_eq!(
            fastpq_prover::poseidon_manifest_sha256(),
            ZK_ACE_POSEIDON_MANIFEST_SHA256_V1
        );
        assert_eq!(
            fastpq_prover::poseidon_profile_id(),
            EXPECTED_PERMUTATION_PROFILE_ID
        );
        assert!(ZK_ACE_POSEIDON_PROFILE_V1.starts_with(
            b"dense-mds-poseidon:goldilocks:x7:width3:rate2:capacity1:full8:partial57:v1"
        ));
        assert!(
            core::str::from_utf8(ZK_ACE_POSEIDON_PROFILE_V1)
                .expect("profile is UTF-8")
                .contains(ZK_ACE_POSEIDON_MANIFEST_SHA256_V1)
        );
        assert_eq!(
            ZK_ACE_FULL_ENGINE_AVAILABLE_V1,
            zk_ace_public_release_pins_complete_v2()
        );
        let binding_profile = core::str::from_utf8(ZK_ACE_COMMITMENT_BINDING_PROFILE_V2)
            .expect("binding profile is UTF-8");
        assert!(binding_profile.contains("classical-rom-generic-collision-target-128-bits"));
        for lane_domain in ZK_ACE_POSEIDON_DIGEST_LANE_DOMAINS_V2 {
            let lane_domain = core::str::from_utf8(lane_domain).expect("lane domain is UTF-8");
            assert!(
                binding_profile.contains(lane_domain),
                "compiled binding profile omitted {lane_domain}"
            );
        }
        for logical_domain in [
            "zk-ace.identity-commitment.v1",
            "zk-ace.replay-nullifier.v1",
            "zk-ace.transparent-transfer.v1",
            "zk-ace.privacy-authorization.v1",
            "iroha:privacy:zk-ace:air-public-digest:v2",
        ] {
            assert!(
                binding_profile.contains(logical_domain),
                "compiled binding profile omitted {logical_domain}"
            );
        }
        assert!(
            core::str::from_utf8(ZK_ACE_RELEASE_READINESS_BLOCKER_V2)
                .expect("release blocker is UTF-8")
                .contains("zero-pin-means-unavailable")
        );
        let source_profile =
            core::str::from_utf8(ZK_ACE_SOURCE_PROFILE_V1).expect("source profile is UTF-8");
        assert!(source_profile.contains("availability-gated-by-reviewed-release-evidence-pins"));
        assert!(!source_profile.contains("activation-disabled"));
        assert!(
            !zk_ace_public_release_pins_complete_v2() || zk_ace_release_evidence_pins_complete_v2(),
            "public release evidence cannot be complete before all native stages are pinned"
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
    fn production_entrypoints_fail_before_work_while_release_gate_is_open() {
        if ZK_ACE_FULL_ENGINE_AVAILABLE_V1 {
            return;
        }
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
