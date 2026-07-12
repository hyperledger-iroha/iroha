//! Native verifier for manifest-bound Kagemusha top-up finality proofs.
//!
//! The proof wire is a bounded projection of the live Sumeragi-v2
//! [`iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact`]. It retains every
//! non-roster height-context identity field and the exact live
//! [`iroha_data_model::block::consensus_v2::QuorumCertificate`], while the current roster,
//! voting powers, quorum, and aligned BLS proofs of possession come only from
//! the authenticated release artifact. Verification reconstructs the complete
//! [`iroha_data_model::block::consensus_v2::HeightContext`], recomputes its identifier, and
//! authenticates the exact
//! [`Vote::signature_preimage`]; no retired consensus vote format is accepted.

use std::{
    collections::VecDeque,
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus_v2::{HeightContextId, Vote, finality::verify_validator_power_roster_pops},
    },
    offline::{
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2,
        KagemushaRecursiveSpendArtifactManifestV3, KagemushaRecursiveSpendTopUpAnchorRefV2,
        KagemushaRecursiveSpendTopUpAnchorV2, KagemushaTopUpFinalityProofV2,
        KagemushaTopUpFinalityRosterArtifactV2, KagemushaTopUpFinalityRosterWindowV2,
    },
};
use parking_lot::Mutex;
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::sumeragi::smt::{
    KAGEMUSHA_V2_TOPUP_ANCHOR_WITNESS_KEY_TAG, KagemushaTopUpMerkleProof, KvPair,
    verify_kagemusha_topup_write_inclusion,
};

/// Number of exact authenticated roster archives whose successful full PoP
/// validation is retained by one verifier instance.
const ROSTER_VERIFICATION_CACHE_CAPACITY: usize = 16;

/// Failure returned by the native top-up finality verifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum KagemushaTopUpFinalityVerifyError {
    /// The proof or one of its bounded structural projections is malformed.
    #[error("invalid Kagemusha top-up finality structure")]
    InvalidStructure,
    /// The release manifest is malformed or not the exact authenticated bytes.
    #[error("Kagemusha top-up finality manifest digest mismatch")]
    ManifestDigestMismatch,
    /// The roster is not the exact content-addressed artifact selected by the manifest.
    #[error("Kagemusha top-up finality roster artifact mismatch")]
    ArtifactDigestMismatch,
    /// The proof does not identify the complete validated top-up anchor supplied by the caller.
    #[error("Kagemusha top-up finality anchor identity mismatch")]
    AnchorMismatch,
    /// The anchor, proof, manifest, and roster do not identify one chain.
    #[error("Kagemusha top-up finality chain mismatch")]
    ChainMismatch,
    /// The anchor and manifest do not identify one asset definition.
    #[error("Kagemusha top-up finality asset mismatch")]
    AssetMismatch,
    /// The anchor and manifest do not use one authoritative asset scale.
    #[error("Kagemusha top-up finality asset-scale mismatch")]
    ScaleMismatch,
    /// The anchor, roster, and manifest do not identify one artifact generation.
    #[error("Kagemusha top-up finality artifact-generation mismatch")]
    ArtifactGenerationMismatch,
    /// The finalized anchor height differs from the exact Commit-QC height.
    #[error("Kagemusha top-up finality height mismatch")]
    HeightMismatch,
    /// The finalized height lies outside the authenticated release window.
    #[error("Kagemusha top-up finality height is outside the release window")]
    ReleaseWindowMismatch,
    /// No unique authenticated roster window reconstructs the signed height context.
    #[error("Kagemusha top-up finality roster/context mismatch")]
    RosterContextMismatch,
    /// One of the manifest-authenticated roster proofs of possession is invalid.
    #[error("invalid Kagemusha top-up finality roster proof of possession")]
    InvalidRosterCryptography,
    /// The BLS aggregate does not authenticate the exact live Sumeragi-v2 Commit vote.
    #[error("invalid Kagemusha top-up finality aggregate signature")]
    InvalidAggregateSignature,
    /// The QC-authenticated next-epoch roster contains an invalid PoP.
    #[error("invalid Kagemusha top-up finality next-epoch proof of possession")]
    InvalidNextEpochCryptography,
    /// A raw Iroha Merkle hash does not carry the canonical marker bit.
    #[error("non-canonical Kagemusha top-up finality Merkle hash")]
    NonCanonicalHash,
    /// The exact top-up anchor is not included in the QC-authenticated execution commitment.
    #[error("invalid Kagemusha top-up anchor inclusion proof")]
    InvalidAnchorInclusion,
}

/// Typed result proving which anchor, release, context, and block were checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedKagemushaTopUpFinalityV2 {
    anchor: KagemushaRecursiveSpendTopUpAnchorRefV2,
    height: u64,
    block_hash: HashOf<BlockHeader>,
    context_id: HeightContextId,
    manifest_sha256: [u8; 32],
    roster_sha256: [u8; 32],
}

impl VerifiedKagemushaTopUpFinalityV2 {
    /// Return the exact compact anchor identity authenticated by the proof.
    #[must_use]
    pub const fn anchor(self) -> KagemushaRecursiveSpendTopUpAnchorRefV2 {
        self.anchor
    }

    /// Return the finalized block height.
    #[must_use]
    pub const fn height(self) -> u64 {
        self.height
    }

    /// Return the exact finalized block-header hash.
    #[must_use]
    pub const fn block_hash(self) -> HashOf<BlockHeader> {
        self.block_hash
    }

    /// Return the recomputed complete Sumeragi-v2 height-context identifier.
    #[must_use]
    pub const fn context_id(self) -> HeightContextId {
        self.context_id
    }

    /// Return the trusted release-manifest SHA-256.
    #[must_use]
    pub const fn manifest_sha256(self) -> [u8; 32] {
        self.manifest_sha256
    }

    /// Return the exact roster-artifact SHA-256 selected by that manifest.
    #[must_use]
    pub const fn roster_sha256(self) -> [u8; 32] {
        self.roster_sha256
    }
}

/// Stateful verifier with a bounded exact-roster PoP-validation cache.
///
/// Cache entries are inserted only after the canonical roster bytes match an
/// authenticated manifest reference. They retain the valid/invalid result of
/// one full PoP pass, so a broken immutable release cannot repeatedly consume
/// pairings. The cache stores only SHA-256 identities and booleans, never
/// peer-controlled archives.
#[derive(Debug, Default)]
pub struct KagemushaTopUpFinalityVerifier {
    roster_cache: Mutex<VecDeque<([u8; 32], bool)>>,
    roster_crypto_verifications: AtomicUsize,
}

impl KagemushaTopUpFinalityVerifier {
    /// Construct an empty verifier cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Verify one proof against a complete anchor and authenticated V3 release.
    ///
    /// `expected_manifest_sha256` must come from the caller's authenticated
    /// release envelope. The manifest then selects the exact roster digest and
    /// byte length. All digest, context, anchor, window, and Merkle checks run
    /// before any BLS pairing work.
    ///
    /// # Errors
    ///
    /// Fails closed for any malformed structure, content-address mismatch,
    /// cross-context substitution, invalid proof of possession, aggregate
    /// signature mutation, or anchor-path mutation.
    pub fn verify(
        &self,
        proof: &KagemushaTopUpFinalityProofV2,
        roster_artifact: &KagemushaTopUpFinalityRosterArtifactV2,
        expected_anchor: &KagemushaRecursiveSpendTopUpAnchorV2,
        manifest: &KagemushaRecursiveSpendArtifactManifestV3,
        expected_manifest_sha256: [u8; 32],
    ) -> Result<VerifiedKagemushaTopUpFinalityV2, KagemushaTopUpFinalityVerifyError> {
        // Phase 1: bounded structural and authenticated-content checks only.
        proof
            .validate_structure()
            .map_err(|_| KagemushaTopUpFinalityVerifyError::InvalidStructure)?;
        expected_anchor
            .validate_public_binding()
            .map_err(|_| KagemushaTopUpFinalityVerifyError::InvalidStructure)?;
        manifest
            .validate()
            .map_err(|_| KagemushaTopUpFinalityVerifyError::ManifestDigestMismatch)?;
        if expected_manifest_sha256 == [0; 32]
            || canonical_sha256(manifest)? != expected_manifest_sha256
        {
            return Err(KagemushaTopUpFinalityVerifyError::ManifestDigestMismatch);
        }

        let expected_anchor_ref = expected_anchor
            .compact_ref()
            .map_err(|_| KagemushaTopUpFinalityVerifyError::InvalidStructure)?;
        if proof.anchor != expected_anchor_ref {
            return Err(KagemushaTopUpFinalityVerifyError::AnchorMismatch);
        }

        let context = &proof.commit_qc.height_context;
        let certificate = &proof.commit_qc.certificate;
        if expected_anchor.chain_id != manifest.chain_id
            || context.chain_id != manifest.chain_id
            || roster_artifact.chain_id != manifest.chain_id
        {
            return Err(KagemushaTopUpFinalityVerifyError::ChainMismatch);
        }
        if expected_anchor.asset.definition() != &manifest.asset {
            return Err(KagemushaTopUpFinalityVerifyError::AssetMismatch);
        }
        if expected_anchor.asset_scale != manifest.asset_scale {
            return Err(KagemushaTopUpFinalityVerifyError::ScaleMismatch);
        }
        if expected_anchor.artifact_generation != manifest.generation
            || roster_artifact.artifact_generation != manifest.generation
        {
            return Err(KagemushaTopUpFinalityVerifyError::ArtifactGenerationMismatch);
        }
        if expected_anchor.finalized_height != context.height
            || context.height != certificate.round.height
        {
            return Err(KagemushaTopUpFinalityVerifyError::HeightMismatch);
        }
        if context.height < manifest.activation_height
            || context.height >= manifest.withdrawal_height
        {
            return Err(KagemushaTopUpFinalityVerifyError::ReleaseWindowMismatch);
        }

        let roster_reference = &manifest.topup_finality_roster_artifact;
        let roster_bytes = norito::to_bytes(roster_artifact)
            .map_err(|_| KagemushaTopUpFinalityVerifyError::InvalidStructure)?;
        let roster_size = u64::try_from(roster_bytes.len())
            .map_err(|_| KagemushaTopUpFinalityVerifyError::ArtifactDigestMismatch)?;
        if roster_size == 0
            || roster_size > KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2
            || roster_size != roster_reference.size_bytes
            || <[u8; 32]>::from(Sha256::digest(&roster_bytes)) != roster_reference.sha256
        {
            return Err(KagemushaTopUpFinalityVerifyError::ArtifactDigestMismatch);
        }
        roster_artifact
            .validate_structure()
            .map_err(|_| KagemushaTopUpFinalityVerifyError::InvalidStructure)?;
        let window = roster_artifact
            .window_at(context.height)
            .map_err(|_| KagemushaTopUpFinalityVerifyError::RosterContextMismatch)?;
        let complete_context = context
            .reconstruct_for_roster_window(window)
            .map_err(|_| KagemushaTopUpFinalityVerifyError::RosterContextMismatch)?;
        proof
            .commit_qc
            .validate_for_roster_window(window)
            .map_err(|_| KagemushaTopUpFinalityVerifyError::RosterContextMismatch)?;
        if complete_context.chain_id != expected_anchor.chain_id {
            return Err(KagemushaTopUpFinalityVerifyError::ChainMismatch);
        }
        verify_anchor_inclusion(proof)?;

        // Phase 2: expensive cryptography. The fully authenticated roster PoPs
        // are validated once per exact bounded cache identity.
        self.validate_roster_cryptography(roster_artifact, roster_reference.sha256)?;
        verify_commit_aggregate(proof, window)?;
        if let Some(snapshot) = &complete_context.next_epoch_snapshot {
            verify_validator_power_roster_pops(&snapshot.roster, &snapshot.validator_set_pops)
                .map_err(|_| KagemushaTopUpFinalityVerifyError::InvalidNextEpochCryptography)?;
        }

        Ok(VerifiedKagemushaTopUpFinalityV2 {
            anchor: expected_anchor_ref,
            height: context.height,
            block_hash: certificate.subject.block_hash,
            context_id: complete_context.id(),
            manifest_sha256: expected_manifest_sha256,
            roster_sha256: roster_reference.sha256,
        })
    }

    fn validate_roster_cryptography(
        &self,
        roster_artifact: &KagemushaTopUpFinalityRosterArtifactV2,
        digest: [u8; 32],
    ) -> Result<(), KagemushaTopUpFinalityVerifyError> {
        let mut cache = self.roster_cache.lock();
        if let Some(position) = cache.iter().position(|(entry, _)| *entry == digest) {
            let entry = cache
                .remove(position)
                .expect("located roster cache entry exists");
            let valid = entry.1;
            cache.push_back(entry);
            return if valid {
                Ok(())
            } else {
                Err(KagemushaTopUpFinalityVerifyError::InvalidRosterCryptography)
            };
        }

        self.roster_crypto_verifications
            .fetch_add(1, Ordering::Relaxed);
        let valid = roster_artifact.validate().is_ok();
        Self::remember_roster_result(&mut cache, digest, valid);
        if valid {
            Ok(())
        } else {
            Err(KagemushaTopUpFinalityVerifyError::InvalidRosterCryptography)
        }
    }

    fn remember_roster_result(
        cache: &mut VecDeque<([u8; 32], bool)>,
        digest: [u8; 32],
        valid: bool,
    ) {
        cache.retain(|(entry, _)| *entry != digest);
        while cache.len() >= ROSTER_VERIFICATION_CACHE_CAPACITY {
            cache.pop_front();
        }
        cache.push_back((digest, valid));
    }

    #[cfg(test)]
    fn roster_crypto_verification_count(&self) -> usize {
        self.roster_crypto_verifications.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    fn cached_roster_count(&self) -> usize {
        self.roster_cache.lock().len()
    }
}

/// Verify one proof using the process-wide bounded exact-roster cache.
///
/// See [`KagemushaTopUpFinalityVerifier::verify`] for the trust contract.
pub fn verify_kagemusha_topup_finality_v2(
    proof: &KagemushaTopUpFinalityProofV2,
    roster_artifact: &KagemushaTopUpFinalityRosterArtifactV2,
    expected_anchor: &KagemushaRecursiveSpendTopUpAnchorV2,
    manifest: &KagemushaRecursiveSpendArtifactManifestV3,
    expected_manifest_sha256: [u8; 32],
) -> Result<VerifiedKagemushaTopUpFinalityV2, KagemushaTopUpFinalityVerifyError> {
    static VERIFIER: OnceLock<KagemushaTopUpFinalityVerifier> = OnceLock::new();
    VERIFIER
        .get_or_init(KagemushaTopUpFinalityVerifier::new)
        .verify(
            proof,
            roster_artifact,
            expected_anchor,
            manifest,
            expected_manifest_sha256,
        )
}

fn canonical_sha256<T: norito::codec::Encode>(
    value: &T,
) -> Result<[u8; 32], KagemushaTopUpFinalityVerifyError> {
    let bytes =
        norito::to_bytes(value).map_err(|_| KagemushaTopUpFinalityVerifyError::InvalidStructure)?;
    Ok(Sha256::digest(bytes).into())
}

fn verify_commit_aggregate(
    proof: &KagemushaTopUpFinalityProofV2,
    window: &KagemushaTopUpFinalityRosterWindowV2,
) -> Result<(), KagemushaTopUpFinalityVerifyError> {
    let certificate = &proof.commit_qc.certificate;
    let first_signer = certificate
        .signers
        .first()
        .copied()
        .ok_or(KagemushaTopUpFinalityVerifyError::InvalidStructure)?;
    let preimage = Vote {
        round: certificate.round,
        phase: certificate.phase,
        subject: certificate.subject,
        execution_commitment: certificate.execution_commitment,
        signer: first_signer,
        signature: Vec::new(),
    }
    .signature_preimage();
    let mut public_keys = Vec::<&PublicKey>::with_capacity(certificate.signers.len());
    let mut pops = Vec::<&[u8]>::with_capacity(certificate.signers.len());
    for signer in &certificate.signers {
        let index = usize::try_from(*signer)
            .map_err(|_| KagemushaTopUpFinalityVerifyError::InvalidStructure)?;
        let validator = window
            .validator_set
            .get(index)
            .ok_or(KagemushaTopUpFinalityVerifyError::InvalidStructure)?;
        let pop = window
            .validator_set_pops
            .get(index)
            .ok_or(KagemushaTopUpFinalityVerifyError::InvalidStructure)?;
        public_keys.push(validator.validator.public_key());
        pops.push(pop.as_slice());
    }
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &preimage,
        &certificate.aggregate_signature,
        &public_keys,
        &pops,
    )
    .map_err(|_| KagemushaTopUpFinalityVerifyError::InvalidAggregateSignature)
}

fn verify_anchor_inclusion(
    proof: &KagemushaTopUpFinalityProofV2,
) -> Result<(), KagemushaTopUpFinalityVerifyError> {
    let mut key = Vec::with_capacity(33);
    key.push(KAGEMUSHA_V2_TOPUP_ANCHOR_WITNESS_KEY_TAG);
    key.extend_from_slice(&proof.anchor.topup_operation_id);
    let leaf = KvPair::new(key, proof.anchor.anchor_digest);
    let path = KagemushaTopUpMerkleProof {
        leaf_index: proof.anchor_path.leaf_index,
        leaf_count: proof.anchor_path.leaf_count,
        siblings: proof
            .anchor_path
            .siblings
            .iter()
            .copied()
            .map(canonical_hash)
            .collect::<Result<Vec<_>, _>>()?,
    };
    let commitment = proof.commit_qc.certificate.execution_commitment;
    if !verify_kagemusha_topup_write_inclusion(
        &leaf,
        &path,
        commitment.ordinary_writes_root,
        commitment.post_state_root,
    ) {
        return Err(KagemushaTopUpFinalityVerifyError::InvalidAnchorInclusion);
    }
    Ok(())
}

fn canonical_hash(bytes: [u8; Hash::LENGTH]) -> Result<Hash, KagemushaTopUpFinalityVerifyError> {
    // `Hash::prehashed` sets this marker bit. Reject first so attacker input is
    // never normalized into a different Merkle-authenticated value.
    if bytes[Hash::LENGTH - 1] & 1 == 0 {
        return Err(KagemushaTopUpFinalityVerifyError::NonCanonicalHash);
    }
    Ok(Hash::prehashed(bytes))
}

#[cfg(test)]
mod tests {
    // Adversarial coverage is kept in this module because it needs direct
    // access to the verifier's bounded-cache counters.
    use core::str::FromStr as _;

    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{
        AccountId, ChainId,
        asset::{AssetDefinitionId, AssetId},
        block::consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
            ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
            QuorumCertificate, ValidatorPower,
            finality::{FinalizedNextEpochSnapshot, V2FinalityArtifact},
        },
        offline::{
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V3,
            KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1,
            KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1,
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3,
            KAGEMUSHA_RECURSIVE_SPEND_STATE_EP_CIRCUIT_ID_V1,
            KAGEMUSHA_RECURSIVE_SPEND_STATE_PARAMETERS_FILE_NAME_V3,
            KAGEMUSHA_RECURSIVE_SPEND_STATE_PROVING_KEY_FILE_NAME_V3,
            KAGEMUSHA_RECURSIVE_SPEND_STATE_VERIFYING_KEY_FILE_NAME_V3,
            KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_EQ_CIRCUIT_ID_V1,
            KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PARAMETERS_FILE_NAME_V3,
            KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROVING_KEY_FILE_NAME_V3,
            KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_VERIFYING_KEY_FILE_NAME_V3,
            KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2, KAGEMUSHA_TOPUP_FINALITY_PROOF_VERSION_V2,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2, KagemushaPastaCycleArtifactKindV3,
            KagemushaPastaCycleArtifactV3, KagemushaPastaCycleParityV1,
            KagemushaPastaCycleProofProfileV1, KagemushaRecursiveSpendArtifactManifestV3,
            KagemushaRecursiveSpendTopUpAnchorV2, KagemushaScaledAmountV2,
            KagemushaSpendableNoteDescriptorV2, KagemushaTopUpAnchorMerkleProofV2,
            KagemushaTopUpFinalityCompactQcV2, KagemushaTopUpFinalityHeightContextV2,
            KagemushaTopUpFinalityProofV2, KagemushaTopUpFinalityRosterArtifactReferenceV2,
            KagemushaTopUpFinalityRosterArtifactV2, KagemushaTopUpFinalityRosterWindowV2,
        },
        peer::PeerId,
        proof::VerifyingKeyId,
    };

    use super::*;
    use crate::sumeragi::smt::{KvPair, build_kagemusha_topup_block_commitment};

    struct Fixture {
        proof: KagemushaTopUpFinalityProofV2,
        roster: KagemushaTopUpFinalityRosterArtifactV2,
        anchor: KagemushaRecursiveSpendTopUpAnchorV2,
        manifest: KagemushaRecursiveSpendArtifactManifestV3,
        manifest_digest: [u8; 32],
        finality_artifact: V2FinalityArtifact,
        signing_keys: Vec<KeyPair>,
    }

    fn artifact(
        kind: KagemushaPastaCycleArtifactKindV3,
        file_name: &str,
        tag: u8,
    ) -> KagemushaPastaCycleArtifactV3 {
        KagemushaPastaCycleArtifactV3 {
            kind,
            file_name: file_name.to_owned(),
            size_bytes: 128,
            sha256: [tag; 32],
            payload_size_bytes: 64,
            payload_sha256: [tag.wrapping_add(1); 32],
        }
    }

    fn profile(
        parity: KagemushaPastaCycleParityV1,
        circuit_id: &str,
        names: [&str; 3],
        tag: u8,
    ) -> KagemushaPastaCycleProofProfileV1 {
        KagemushaPastaCycleProofProfileV1 {
            parity,
            circuit_id: circuit_id.to_owned(),
            parameter_generation: "params-generation-1".to_owned(),
            ipa_k: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
            artifacts: vec![
                artifact(KagemushaPastaCycleArtifactKindV3::Parameters, names[0], tag),
                artifact(
                    KagemushaPastaCycleArtifactKindV3::ProvingKey,
                    names[1],
                    tag + 2,
                ),
                artifact(
                    KagemushaPastaCycleArtifactKindV3::VerifyingKey,
                    names[2],
                    tag + 4,
                ),
            ],
        }
    }

    fn fixture() -> Fixture {
        let keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS key")
            })
            .collect::<Vec<_>>();
        let validator_set = keys
            .iter()
            .map(|key| ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let pops = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture proof of possession")
            })
            .collect::<Vec<_>>();
        let fixed_pops = pops
            .iter()
            .map(|pop| <[u8; 96]>::try_from(pop.as_slice()).expect("96-byte PoP"))
            .collect::<Vec<_>>();
        let chain_id = ChainId::from("kagemusha-finality-chain");
        let asset = AssetDefinitionId::from_str("rose#wonderland").expect("asset id");
        let payer_key =
            KeyPair::try_from_seed(vec![0x51; 32], Algorithm::Ed25519).expect("payer key");
        let payer = AccountId::new(payer_key.public_key().clone());
        let height = 42;
        let operation_id = [0xA5; 32];
        let note = KagemushaSpendableNoteDescriptorV2 {
            chain_id: chain_id.clone(),
            asset: asset.clone(),
            note_commitment: [0x31; 32],
            spend_nullifier: [0x32; 32],
            amount: KagemushaScaledAmountV2::new(500, 2).expect("amount"),
        };
        let anchor = KagemushaRecursiveSpendTopUpAnchorV2 {
            version: 2,
            chain_id: chain_id.clone(),
            payer: payer.clone(),
            asset: AssetId::new(asset.clone(), payer),
            asset_scale: 2,
            amount: note.amount,
            initial_root: [0x11; 32],
            finalized_root: [0x12; 32],
            topup_anchor_nullifiers: vec![[0x13; 32]],
            current_note: note,
            topup_operation_id: operation_id,
            transfer_verifier_id: VerifyingKeyId::new("halo2/ipa", "transfer-v2"),
            transfer_verifier_commitment: [0x14; 32],
            artifact_generation: "release-generation-1".to_owned(),
            finalized_height: height,
            finalized_tx_hash: [0x15; 32],
            anchor_digest: [0; 32],
        }
        .finalize_digest()
        .expect("anchor");
        let mut witness_key = vec![KAGEMUSHA_V2_TOPUP_ANCHOR_WITNESS_KEY_TAG];
        witness_key.extend_from_slice(&operation_id);
        let mut other_key = vec![KAGEMUSHA_V2_TOPUP_ANCHOR_WITNESS_KEY_TAG];
        other_key.extend_from_slice(&[0xB6; 32]);
        let commitment = build_kagemusha_topup_block_commitment(&[
            KvPair::new(witness_key, anchor.anchor_digest),
            KvPair::new(other_key, [0x6B; 32]),
        ])
        .expect("bounded commitment")
        .expect("top-up commitment");
        let context = HeightContext {
            chain_id: chain_id.clone(),
            protocol_version: PROTOCOL_VERSION,
            height,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Permissioned,
            parent_commit_qc: Some(QuorumCertificate {
                round: ConsensusRound {
                    context_id: iroha_data_model::block::consensus_v2::HeightContextId(
                        HashOf::from_untyped_unchecked(Hash::new(b"parent context")),
                    ),
                    height: height - 1,
                    view: 0,
                },
                phase: GlobalPhase::Commit,
                subject: BlockSubject {
                    parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
                        b"grandparent",
                    ))),
                    block_hash: HashOf::from_untyped_unchecked(Hash::new(b"parent block")),
                    payload_hash: Hash::new(b"parent payload"),
                },
                execution_commitment: ExecutionCommitment::without_topups(
                    Hash::new(b"parent parent state"),
                    Hash::new(b"parent post state"),
                    Hash::new(b"parent ordinary writes"),
                ),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0x44; 96],
            }),
            roster: validator_set.clone(),
            quorum: DualQuorum::from_roster(&validator_set).expect("quorum"),
            nexus_amx_context_hash: Hash::new(b"nexus"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::Plain,
                chunk_size_bytes: 1024,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 4096,
                max_chunk_count: 4,
            },
            leader_seed: [0x22; 32],
        };
        context.validate().expect("context");
        let subject = BlockSubject {
            parent_block_hash: context
                .parent_commit_qc
                .as_ref()
                .map(|parent| parent.subject.block_hash),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"finalized block")),
            payload_hash: Hash::new(b"finalized payload"),
        };
        let execution_commitment = ExecutionCommitment::new(
            Hash::new(b"parent state"),
            commitment.post_state_root,
            commitment.ordinary_writes_root,
            Some(commitment.topup_anchor_root),
            2,
        )
        .expect("execution commitment");
        let mut certificate = QuorumCertificate {
            round: ConsensusRound {
                context_id: context.id(),
                height,
                view: 3,
            },
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let preimage = Vote {
            round: certificate.round,
            phase: certificate.phase,
            subject: certificate.subject,
            execution_commitment: certificate.execution_commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let signatures = keys
            .iter()
            .take(3)
            .map(|key| {
                Signature::try_new(key.private_key(), &preimage)
                    .expect("vote signature")
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
            &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate");
        let finality_artifact =
            V2FinalityArtifact::new(context.clone(), subject, certificate.clone(), pops);
        finality_artifact.verify().expect("live finality artifact");
        let projection = KagemushaTopUpFinalityHeightContextV2 {
            context_id: context.id(),
            chain_id: context.chain_id.clone(),
            protocol_version: context.protocol_version,
            height: context.height,
            epoch: context.epoch,
            epoch_end_height: context.epoch_end_height,
            next_epoch_snapshot: context.next_epoch_snapshot.clone(),
            mode: context.mode,
            parent_commit_qc: context.parent_commit_qc.clone(),
            nexus_amx_context_hash: context.nexus_amx_context_hash,
            da_layout: context.da_layout,
            leader_seed: context.leader_seed,
        };
        let proof = KagemushaTopUpFinalityProofV2 {
            version: KAGEMUSHA_TOPUP_FINALITY_PROOF_VERSION_V2,
            anchor: anchor.compact_ref().expect("anchor ref"),
            commit_qc: KagemushaTopUpFinalityCompactQcV2 {
                height_context: projection,
                certificate,
            },
            anchor_path: KagemushaTopUpAnchorMerkleProofV2 {
                leaf_index: commitment.proofs[0].leaf_index,
                leaf_count: commitment.proofs[0].leaf_count,
                siblings: commitment.proofs[0]
                    .siblings
                    .iter()
                    .copied()
                    .map(Into::into)
                    .collect(),
            },
        };
        let window = KagemushaTopUpFinalityRosterWindowV2 {
            activates_at_height: 1,
            withdraws_at_height: 100,
            consensus_mode: ConsensusMode::Permissioned,
            validator_set,
            validator_set_pops: fixed_pops,
        };
        let roster = KagemushaTopUpFinalityRosterArtifactV2 {
            version: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2,
            chain_id: chain_id.clone(),
            artifact_generation: "release-generation-1".to_owned(),
            windows: vec![window],
        };
        let roster_bytes = norito::to_bytes(&roster).expect("roster bytes");
        let roster_digest = Sha256::digest(&roster_bytes).into();
        let manifest = KagemushaRecursiveSpendArtifactManifestV3 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V3,
            bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
            mode: KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1.to_owned(),
            proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1.to_owned(),
            transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1.to_owned(),
            generation: "release-generation-1".to_owned(),
            source_commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
            chain_id,
            asset,
            asset_scale: 2,
            activation_height: 1,
            withdrawal_height: 100,
            max_proof_bytes: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3,
            profiles: vec![
                profile(
                    KagemushaPastaCycleParityV1::TransitionEq,
                    KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_EQ_CIRCUIT_ID_V1,
                    [
                        KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PARAMETERS_FILE_NAME_V3,
                        KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROVING_KEY_FILE_NAME_V3,
                        KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_VERIFYING_KEY_FILE_NAME_V3,
                    ],
                    0x20,
                ),
                profile(
                    KagemushaPastaCycleParityV1::StateEp,
                    KAGEMUSHA_RECURSIVE_SPEND_STATE_EP_CIRCUIT_ID_V1,
                    [
                        KAGEMUSHA_RECURSIVE_SPEND_STATE_PARAMETERS_FILE_NAME_V3,
                        KAGEMUSHA_RECURSIVE_SPEND_STATE_PROVING_KEY_FILE_NAME_V3,
                        KAGEMUSHA_RECURSIVE_SPEND_STATE_VERIFYING_KEY_FILE_NAME_V3,
                    ],
                    0x30,
                ),
            ],
            topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactReferenceV2 {
                file_name: KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2.to_owned(),
                size_bytes: u64::try_from(roster_bytes.len()).expect("roster size"),
                sha256: roster_digest,
                artifact_generation: "release-generation-1".to_owned(),
                circuit_id: KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2.to_owned(),
                purpose: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2.to_owned(),
                artifact_type: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2.to_owned(),
                required_bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
            },
            benchmark_evidence_sha256: [0x71; 32],
            cryptographic_review_sha256: [0x72; 32],
            release_attestation_sha256: [0x73; 32],
        };
        manifest.validate().expect("manifest");
        let manifest_digest = canonical_sha256(&manifest).expect("manifest digest");
        Fixture {
            proof,
            roster,
            anchor,
            manifest,
            manifest_digest,
            finality_artifact,
            signing_keys: keys,
        }
    }

    fn verify(
        verifier: &KagemushaTopUpFinalityVerifier,
        fixture: &Fixture,
    ) -> Result<VerifiedKagemushaTopUpFinalityV2, KagemushaTopUpFinalityVerifyError> {
        verifier.verify(
            &fixture.proof,
            &fixture.roster,
            &fixture.anchor,
            &fixture.manifest,
            fixture.manifest_digest,
        )
    }

    fn epoch_boundary_proof(
        fixture: &Fixture,
        mutate_next_epoch_pop: bool,
    ) -> KagemushaTopUpFinalityProofV2 {
        let window = &fixture.roster.windows[0];
        let mut context = fixture
            .proof
            .commit_qc
            .height_context
            .reconstruct_for_roster_window(window)
            .expect("fixture height context");
        let next_keys = (0x61_u8..=0x64)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic next-epoch BLS key")
            })
            .collect::<Vec<_>>();
        let mut next_entries = next_keys
            .iter()
            .map(|key| {
                let pop =
                    iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("next-epoch PoP");
                (
                    ValidatorPower {
                        validator: PeerId::new(key.public_key().clone()),
                        power: 1,
                    },
                    pop,
                )
            })
            .collect::<Vec<_>>();
        next_entries.sort_by(|left, right| left.0.validator.cmp(&right.0.validator));
        let next_roster = next_entries
            .iter()
            .map(|(entry, _)| entry.clone())
            .collect::<Vec<_>>();
        let mut next_pops = next_entries
            .into_iter()
            .map(|(_, pop)| pop)
            .collect::<Vec<_>>();
        if mutate_next_epoch_pop {
            next_pops[0][0] ^= 1;
        }
        context.epoch_end_height = context.height;
        context.next_epoch_snapshot = Some(FinalizedNextEpochSnapshot {
            epoch: context
                .epoch
                .checked_add(1)
                .expect("fixture epoch has a successor"),
            epoch_end_height: context
                .height
                .checked_add(100)
                .expect("fixture next epoch end height"),
            mode: context.mode,
            quorum: DualQuorum::from_roster(&next_roster).expect("next-epoch quorum"),
            roster: next_roster,
            validator_set_pops: next_pops,
            leader_seed: [0x62; 32],
        });
        context
            .validate()
            .expect("epoch-boundary context is structurally valid");

        let mut proof = fixture.proof.clone();
        proof.commit_qc.height_context = KagemushaTopUpFinalityHeightContextV2 {
            context_id: context.id(),
            chain_id: context.chain_id.clone(),
            protocol_version: context.protocol_version,
            height: context.height,
            epoch: context.epoch,
            epoch_end_height: context.epoch_end_height,
            next_epoch_snapshot: context.next_epoch_snapshot.clone(),
            mode: context.mode,
            parent_commit_qc: context.parent_commit_qc.clone(),
            nexus_amx_context_hash: context.nexus_amx_context_hash,
            da_layout: context.da_layout,
            leader_seed: context.leader_seed,
        };
        let certificate = &mut proof.commit_qc.certificate;
        certificate.round.context_id = context.id();
        let preimage = Vote {
            round: certificate.round,
            phase: certificate.phase,
            subject: certificate.subject,
            execution_commitment: certificate.execution_commitment,
            signer: certificate.signers[0],
            signature: Vec::new(),
        }
        .signature_preimage();
        let signatures = certificate
            .signers
            .iter()
            .map(|index| {
                let index = usize::try_from(*index).expect("fixture signer index fits usize");
                Signature::try_new(fixture.signing_keys[index].private_key(), &preimage)
                    .expect("epoch-boundary vote signature")
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
            &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("epoch-boundary aggregate signature");
        proof
    }

    #[test]
    fn accepts_projection_of_real_live_finality_artifact() {
        let fixture = fixture();
        let verified = verify(&KagemushaTopUpFinalityVerifier::new(), &fixture)
            .expect("valid top-up finality");
        assert_eq!(verified.anchor(), fixture.anchor.compact_ref().unwrap());
        assert_eq!(verified.height(), fixture.finality_artifact.height);
        assert_eq!(
            verified.context_id(),
            fixture.finality_artifact.context_id()
        );
        assert_eq!(verified.block_hash(), fixture.finality_artifact.block_hash);
    }

    #[test]
    fn rejects_anchor_chain_asset_scale_generation_height_and_release_substitution() {
        let verifier = KagemushaTopUpFinalityVerifier::new();
        let fixture = fixture();

        let mut other_anchor = fixture.anchor.clone();
        other_anchor.topup_operation_id[0] ^= 1;
        other_anchor = other_anchor.finalize_digest().expect("alternate anchor");
        assert_eq!(
            verifier
                .verify(
                    &fixture.proof,
                    &fixture.roster,
                    &other_anchor,
                    &fixture.manifest,
                    fixture.manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::AnchorMismatch
        );

        let mut manifest = fixture.manifest.clone();
        manifest.chain_id = ChainId::from("other-chain");
        let digest = canonical_sha256(&manifest).expect("digest");
        assert_eq!(
            verifier
                .verify(
                    &fixture.proof,
                    &fixture.roster,
                    &fixture.anchor,
                    &manifest,
                    digest
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::ChainMismatch
        );

        let mut manifest = fixture.manifest.clone();
        manifest.asset = AssetDefinitionId::from_str("other#wonderland").unwrap();
        let digest = canonical_sha256(&manifest).expect("digest");
        assert_eq!(
            verifier
                .verify(
                    &fixture.proof,
                    &fixture.roster,
                    &fixture.anchor,
                    &manifest,
                    digest
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::AssetMismatch
        );

        let mut manifest = fixture.manifest.clone();
        manifest.asset_scale += 1;
        let digest = canonical_sha256(&manifest).expect("digest");
        assert_eq!(
            verifier
                .verify(
                    &fixture.proof,
                    &fixture.roster,
                    &fixture.anchor,
                    &manifest,
                    digest
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::ScaleMismatch
        );

        let mut anchor = fixture.anchor.clone();
        anchor.artifact_generation = "other-generation".to_owned();
        anchor = anchor
            .finalize_digest()
            .expect("alternate generation anchor");
        let mut proof = fixture.proof.clone();
        proof.anchor = anchor.compact_ref().unwrap();
        assert_eq!(
            verifier
                .verify(
                    &proof,
                    &fixture.roster,
                    &anchor,
                    &fixture.manifest,
                    fixture.manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::ArtifactGenerationMismatch
        );

        let mut anchor = fixture.anchor.clone();
        anchor.finalized_height += 1;
        anchor = anchor.finalize_digest().expect("alternate height anchor");
        let mut proof = fixture.proof.clone();
        proof.anchor = anchor.compact_ref().unwrap();
        assert_eq!(
            verifier
                .verify(
                    &proof,
                    &fixture.roster,
                    &anchor,
                    &fixture.manifest,
                    fixture.manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::HeightMismatch
        );

        let mut manifest = fixture.manifest.clone();
        manifest.withdrawal_height = fixture.proof.commit_qc.height_context.height;
        let digest = canonical_sha256(&manifest).expect("digest");
        assert_eq!(
            verifier
                .verify(
                    &fixture.proof,
                    &fixture.roster,
                    &fixture.anchor,
                    &manifest,
                    digest
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::ReleaseWindowMismatch
        );
    }

    #[test]
    fn rejects_context_roster_power_signature_and_path_substitution_before_cache_fill() {
        let verifier = KagemushaTopUpFinalityVerifier::new();
        let fixture = fixture();

        let mut context = fixture.proof.clone();
        context.commit_qc.height_context.leader_seed[0] ^= 1;
        assert_eq!(
            verifier
                .verify(
                    &context,
                    &fixture.roster,
                    &fixture.anchor,
                    &fixture.manifest,
                    fixture.manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::RosterContextMismatch
        );

        let mut roster = fixture.roster.clone();
        roster.windows[0].validator_set.swap(0, 1);
        let mut manifest = fixture.manifest.clone();
        let bytes = norito::to_bytes(&roster).unwrap();
        manifest.topup_finality_roster_artifact.size_bytes = bytes.len() as u64;
        manifest.topup_finality_roster_artifact.sha256 = Sha256::digest(bytes).into();
        let digest = canonical_sha256(&manifest).unwrap();
        assert_eq!(
            verifier
                .verify(&fixture.proof, &roster, &fixture.anchor, &manifest, digest)
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::RosterContextMismatch
        );

        let mut roster = fixture.roster.clone();
        roster.windows[0].consensus_mode = ConsensusMode::Npos;
        roster.windows[0].validator_set[0].power = 2;
        let mut manifest = fixture.manifest.clone();
        let bytes = norito::to_bytes(&roster).unwrap();
        manifest.topup_finality_roster_artifact.size_bytes = bytes.len() as u64;
        manifest.topup_finality_roster_artifact.sha256 = Sha256::digest(bytes).into();
        let digest = canonical_sha256(&manifest).unwrap();
        assert_eq!(
            verifier
                .verify(&fixture.proof, &roster, &fixture.anchor, &manifest, digest)
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::RosterContextMismatch
        );
        assert_eq!(
            verifier.roster_crypto_verification_count(),
            0,
            "context/roster substitutions must fail before any PoP pairing"
        );

        let mut signature = fixture.proof.clone();
        signature.commit_qc.certificate.aggregate_signature[0] ^= 1;
        assert_eq!(
            verifier
                .verify(
                    &signature,
                    &fixture.roster,
                    &fixture.anchor,
                    &fixture.manifest,
                    fixture.manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::InvalidAggregateSignature
        );
        assert_eq!(verifier.roster_crypto_verification_count(), 1);

        let mut path = fixture.proof.clone();
        path.anchor_path.siblings[0][0] ^= 1;
        assert_eq!(
            verifier
                .verify(
                    &path,
                    &fixture.roster,
                    &fixture.anchor,
                    &fixture.manifest,
                    fixture.manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::InvalidAnchorInclusion
        );
    }

    #[test]
    fn content_addresses_and_roster_pops_fail_closed_before_commit_verification() {
        let verifier = KagemushaTopUpFinalityVerifier::new();
        let fixture = fixture();
        assert_eq!(
            verifier
                .verify(
                    &fixture.proof,
                    &fixture.roster,
                    &fixture.anchor,
                    &fixture.manifest,
                    [0xFF; 32],
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::ManifestDigestMismatch
        );
        assert_eq!(verifier.roster_crypto_verification_count(), 0);

        let mut roster = fixture.roster.clone();
        roster.windows[0].validator_set_pops[0][0] ^= 1;
        assert_eq!(
            verifier
                .verify(
                    &fixture.proof,
                    &roster,
                    &fixture.anchor,
                    &fixture.manifest,
                    fixture.manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::ArtifactDigestMismatch
        );
        assert_eq!(verifier.roster_crypto_verification_count(), 0);

        let mut manifest = fixture.manifest.clone();
        let bytes = norito::to_bytes(&roster).expect("mutated roster bytes");
        manifest.topup_finality_roster_artifact.size_bytes = bytes.len() as u64;
        manifest.topup_finality_roster_artifact.sha256 = Sha256::digest(bytes).into();
        let manifest_digest = canonical_sha256(&manifest).expect("mutated manifest digest");
        assert_eq!(
            verifier
                .verify(
                    &fixture.proof,
                    &roster,
                    &fixture.anchor,
                    &manifest,
                    manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::InvalidRosterCryptography
        );
        assert_eq!(verifier.roster_crypto_verification_count(), 1);
        assert_eq!(
            verifier
                .verify(
                    &fixture.proof,
                    &roster,
                    &fixture.anchor,
                    &manifest,
                    manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::InvalidRosterCryptography
        );
        assert_eq!(
            verifier.roster_crypto_verification_count(),
            1,
            "an immutable invalid roster digest must receive one PoP pass total"
        );
    }

    #[test]
    fn every_non_roster_context_identity_field_is_bound_by_the_signed_context_id() {
        let verifier = KagemushaTopUpFinalityVerifier::new();
        let fixture = fixture();
        let mut mutations = Vec::new();

        let mut epoch = fixture.proof.clone();
        epoch.commit_qc.height_context.epoch += 1;
        mutations.push(("epoch", epoch));

        let mut epoch_end = fixture.proof.clone();
        epoch_end.commit_qc.height_context.epoch_end_height += 1;
        mutations.push(("epoch end", epoch_end));

        let mut mode = fixture.proof.clone();
        mode.commit_qc.height_context.mode = ConsensusMode::Npos;
        mutations.push(("consensus mode", mode));

        let mut nexus = fixture.proof.clone();
        nexus.commit_qc.height_context.nexus_amx_context_hash = Hash::new(b"other nexus");
        mutations.push(("nexus", nexus));

        let mut da = fixture.proof.clone();
        da.commit_qc.height_context.da_layout.chunk_size_bytes *= 2;
        mutations.push(("data availability", da));

        let mut leader = fixture.proof.clone();
        leader.commit_qc.height_context.leader_seed[0] ^= 1;
        mutations.push(("leader seed", leader));

        let mut parent = fixture.proof.clone();
        parent
            .commit_qc
            .height_context
            .parent_commit_qc
            .as_mut()
            .expect("non-genesis fixture")
            .subject
            .payload_hash = Hash::new(b"other parent payload");
        mutations.push(("parent commit", parent));

        for (field, proof) in mutations {
            assert_eq!(
                verifier
                    .verify(
                        &proof,
                        &fixture.roster,
                        &fixture.anchor,
                        &fixture.manifest,
                        fixture.manifest_digest,
                    )
                    .unwrap_err(),
                KagemushaTopUpFinalityVerifyError::RosterContextMismatch,
                "{field} substitution must change the recomputed context id"
            );
        }
        assert_eq!(
            verifier.roster_crypto_verification_count(),
            0,
            "context substitutions must all fail before PoP verification"
        );

        let mut protocol = fixture.proof.clone();
        protocol.commit_qc.height_context.protocol_version += 1;
        assert_eq!(
            verifier
                .verify(
                    &protocol,
                    &fixture.roster,
                    &fixture.anchor,
                    &fixture.manifest,
                    fixture.manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::InvalidStructure
        );
    }

    #[test]
    fn noncanonical_merkle_hash_is_distinct_from_a_canonical_wrong_path() {
        let verifier = KagemushaTopUpFinalityVerifier::new();
        let fixture = fixture();
        let mut noncanonical = fixture.proof.clone();
        let sibling = &mut noncanonical.anchor_path.siblings[0];
        sibling[Hash::LENGTH - 1] &= !1;
        assert_eq!(
            verifier
                .verify(
                    &noncanonical,
                    &fixture.roster,
                    &fixture.anchor,
                    &fixture.manifest,
                    fixture.manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::NonCanonicalHash
        );

        let mut canonical_wrong = fixture.proof.clone();
        canonical_wrong.anchor_path.siblings[0][0] ^= 1;
        assert_eq!(
            verifier
                .verify(
                    &canonical_wrong,
                    &fixture.roster,
                    &fixture.anchor,
                    &fixture.manifest,
                    fixture.manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::InvalidAnchorInclusion
        );
    }

    #[test]
    fn release_window_is_inclusive_at_activation_and_exclusive_at_withdrawal() {
        let verifier = KagemushaTopUpFinalityVerifier::new();
        let fixture = fixture();
        let height = fixture.anchor.finalized_height;

        let mut boundary = fixture.manifest.clone();
        boundary.activation_height = height;
        boundary.withdrawal_height = height + 1;
        let digest = canonical_sha256(&boundary).expect("boundary manifest digest");
        verifier
            .verify(
                &fixture.proof,
                &fixture.roster,
                &fixture.anchor,
                &boundary,
                digest,
            )
            .expect("activation height and withdrawal minus one are accepted");

        let mut before_activation = fixture.manifest.clone();
        before_activation.activation_height = height + 1;
        let digest = canonical_sha256(&before_activation).expect("pre-activation digest");
        assert_eq!(
            verifier
                .verify(
                    &fixture.proof,
                    &fixture.roster,
                    &fixture.anchor,
                    &before_activation,
                    digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::ReleaseWindowMismatch
        );

        let mut at_withdrawal = fixture.manifest.clone();
        at_withdrawal.withdrawal_height = height;
        let digest = canonical_sha256(&at_withdrawal).expect("withdrawal digest");
        assert_eq!(
            verifier
                .verify(
                    &fixture.proof,
                    &fixture.roster,
                    &fixture.anchor,
                    &at_withdrawal,
                    digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::ReleaseWindowMismatch
        );
    }

    #[test]
    fn epoch_boundary_verifies_qc_authenticated_next_roster_pops() {
        let fixture = fixture();
        let verifier = KagemushaTopUpFinalityVerifier::new();
        let valid = epoch_boundary_proof(&fixture, false);
        verifier
            .verify(
                &valid,
                &fixture.roster,
                &fixture.anchor,
                &fixture.manifest,
                fixture.manifest_digest,
            )
            .expect("valid epoch-boundary next-roster PoPs");

        let invalid = epoch_boundary_proof(&fixture, true);
        assert_eq!(
            verifier
                .verify(
                    &invalid,
                    &fixture.roster,
                    &fixture.anchor,
                    &fixture.manifest,
                    fixture.manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::InvalidNextEpochCryptography
        );

        let mut invalid_qc_and_pop = epoch_boundary_proof(&fixture, true);
        invalid_qc_and_pop.commit_qc.certificate.aggregate_signature[0] ^= 1;
        assert_eq!(
            verifier
                .verify(
                    &invalid_qc_and_pop,
                    &fixture.roster,
                    &fixture.anchor,
                    &fixture.manifest,
                    fixture.manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::InvalidAggregateSignature,
            "the current Commit QC must authenticate the next-epoch snapshot before its PoPs are trusted"
        );
    }

    #[test]
    fn exact_roster_cache_is_bounded_and_bad_proofs_do_not_repeat_full_pop_validation() {
        let verifier = KagemushaTopUpFinalityVerifier::new();
        let fixture = fixture();
        let first = verify(&verifier, &fixture).expect("first verification");
        let repeated = verify(&verifier, &fixture).expect("same-anchor idempotent verification");
        assert_eq!(repeated, first);
        assert_eq!(verifier.roster_crypto_verification_count(), 1);

        for bit in 0_u8..8 {
            let mut bad = fixture.proof.clone();
            bad.commit_qc.certificate.aggregate_signature[0] ^= 1 << bit;
            assert_eq!(
                verifier
                    .verify(
                        &bad,
                        &fixture.roster,
                        &fixture.anchor,
                        &fixture.manifest,
                        fixture.manifest_digest,
                    )
                    .unwrap_err(),
                KagemushaTopUpFinalityVerifyError::InvalidAggregateSignature
            );
        }
        assert_eq!(verifier.roster_crypto_verification_count(), 1);
        assert_eq!(verifier.cached_roster_count(), 1);

        // Exercise exact LRU eviction without incurring unrelated pairings.
        let mut cache = VecDeque::new();
        for value in 0..ROSTER_VERIFICATION_CACHE_CAPACITY + 5 {
            KagemushaTopUpFinalityVerifier::remember_roster_result(
                &mut cache,
                [u8::try_from(value).unwrap(); 32],
                value % 2 == 0,
            );
        }
        assert_eq!(cache.len(), ROSTER_VERIFICATION_CACHE_CAPACITY);
        assert_eq!(cache.front(), Some(&([5; 32], false)));
        KagemushaTopUpFinalityVerifier::remember_roster_result(&mut cache, [5; 32], false);
        assert_eq!(cache.len(), ROSTER_VERIFICATION_CACHE_CAPACITY);
        assert_eq!(cache.back(), Some(&([5; 32], false)));
    }
}
