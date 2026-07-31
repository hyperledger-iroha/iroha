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
        KagemushaRecursiveSpendArtifactManifestV4, KagemushaRecursiveSpendTopUpAnchorRefV2,
        KagemushaRecursiveSpendTopUpAnchorV4, KagemushaTopUpAnchorMerkleProofV2,
        KagemushaTopUpFinalityProofV2, KagemushaTopUpFinalityRosterArtifactV2,
        KagemushaTopUpFinalityRosterWindowV2,
    },
};
use parking_lot::Mutex;
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::sumeragi::smt::{
    KAGEMUSHA_V4_TOPUP_ANCHOR_WITNESS_KEY_TAG, KagemushaTopUpMerkleProof, KvPair,
    build_kagemusha_topup_block_commitment, verify_kagemusha_topup_write_inclusion,
};

/// Consensus execution-commitment material for a block containing one
/// finalized Kagemusha top-up anchor and no unrelated writes.
///
/// This narrow producer-side projection deliberately reuses the live
/// Sumeragi commitment builder. Release qualification and portable SDK
/// acceptance generators can therefore construct a real QC-authenticated
/// anchor without duplicating the consensus leaf tag, sparse-tree rules, or
/// balanced top-up tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KagemushaSingleTopUpExecutionCommitmentV2 {
    /// Root of the (empty) ordinary-write set.
    pub ordinary_writes_root: Hash,
    /// Post-state root combining ordinary writes and the top-up root.
    pub post_state_root: Hash,
    /// Root of the one-leaf block-local top-up tree.
    pub topup_anchor_root: Hash,
    /// Inclusion path consumed by [`KagemushaTopUpFinalityProofV2`].
    pub anchor_path: KagemushaTopUpAnchorMerkleProofV2,
}

/// Build the exact consensus commitment for one finalized top-up anchor.
///
/// # Errors
///
/// Returns an error for a zero operation id/digest or if the live consensus
/// commitment builder unexpectedly rejects or omits the supplied leaf.
pub fn build_single_kagemusha_topup_execution_commitment_v2(
    operation_id: [u8; 32],
    anchor_digest: [u8; 32],
) -> Result<KagemushaSingleTopUpExecutionCommitmentV2, &'static str> {
    if operation_id == [0; 32] || anchor_digest == [0; 32] {
        return Err("Kagemusha top-up operation id and anchor digest must be non-zero");
    }
    let mut witness_key = Vec::with_capacity(33);
    witness_key.push(KAGEMUSHA_V4_TOPUP_ANCHOR_WITNESS_KEY_TAG);
    witness_key.extend_from_slice(&operation_id);
    let commitment =
        build_kagemusha_topup_block_commitment(&[KvPair::new(witness_key, anchor_digest)])?
            .ok_or("single Kagemusha top-up commitment unexpectedly has no leaf")?;
    let proof = commitment
        .proofs
        .first()
        .ok_or("single Kagemusha top-up commitment unexpectedly has no proof")?;
    Ok(KagemushaSingleTopUpExecutionCommitmentV2 {
        ordinary_writes_root: commitment.ordinary_writes_root,
        post_state_root: commitment.post_state_root,
        topup_anchor_root: commitment.topup_anchor_root,
        anchor_path: KagemushaTopUpAnchorMerkleProofV2 {
            leaf_index: proof.leaf_index,
            leaf_count: proof.leaf_count,
            siblings: proof.siblings.iter().copied().map(Into::into).collect(),
        },
    })
}

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

    /// Verify one proof against a complete ABI-21 anchor and its exact
    /// authenticated V4 release. The V4 receipt and manifest are validated in
    /// place; this path never projects either value into an older release carrier.
    pub fn verify_v4(
        &self,
        proof: &KagemushaTopUpFinalityProofV2,
        roster_artifact: &KagemushaTopUpFinalityRosterArtifactV2,
        expected_anchor: &KagemushaRecursiveSpendTopUpAnchorV4,
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        expected_manifest_sha256: [u8; 32],
    ) -> Result<VerifiedKagemushaTopUpFinalityV2, KagemushaTopUpFinalityVerifyError> {
        self.verify_v4_with_manifest_state(
            proof,
            roster_artifact,
            expected_anchor,
            manifest,
            expected_manifest_sha256,
            true,
        )
    }

    /// Verify the same live finality proof against a clean, unsigned ABI-21
    /// candidate in an explicitly selected non-shipping evidence-lab build.
    #[cfg(feature = "kagemusha-candidate-evidence-lab")]
    pub fn verify_candidate_evidence_lab_v4(
        &self,
        proof: &KagemushaTopUpFinalityProofV2,
        roster_artifact: &KagemushaTopUpFinalityRosterArtifactV2,
        expected_anchor: &KagemushaRecursiveSpendTopUpAnchorV4,
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        expected_manifest_sha256: [u8; 32],
    ) -> Result<VerifiedKagemushaTopUpFinalityV2, KagemushaTopUpFinalityVerifyError> {
        self.verify_v4_with_manifest_state(
            proof,
            roster_artifact,
            expected_anchor,
            manifest,
            expected_manifest_sha256,
            false,
        )
    }

    fn verify_v4_with_manifest_state(
        &self,
        proof: &KagemushaTopUpFinalityProofV2,
        roster_artifact: &KagemushaTopUpFinalityRosterArtifactV2,
        expected_anchor: &KagemushaRecursiveSpendTopUpAnchorV4,
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        expected_manifest_sha256: [u8; 32],
        finalized_release: bool,
    ) -> Result<VerifiedKagemushaTopUpFinalityV2, KagemushaTopUpFinalityVerifyError> {
        proof
            .validate_structure()
            .map_err(|_| KagemushaTopUpFinalityVerifyError::InvalidStructure)?;
        expected_anchor
            .validate_public_binding()
            .map_err(|_| KagemushaTopUpFinalityVerifyError::InvalidStructure)?;
        if finalized_release {
            manifest
                .validate()
                .map_err(|_| KagemushaTopUpFinalityVerifyError::ManifestDigestMismatch)?;
        } else {
            #[cfg(feature = "kagemusha-candidate-evidence-lab")]
            manifest
                .validate_unsigned_candidate()
                .map_err(|_| KagemushaTopUpFinalityVerifyError::ManifestDigestMismatch)?;
            #[cfg(not(feature = "kagemusha-candidate-evidence-lab"))]
            return Err(KagemushaTopUpFinalityVerifyError::ManifestDigestMismatch);
        }
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
        if expected_anchor.artifact_binding.generation != manifest.generation
            || roster_artifact.artifact_generation != manifest.generation
        {
            return Err(KagemushaTopUpFinalityVerifyError::ArtifactGenerationMismatch);
        }
        if expected_anchor.artifact_binding.manifest_sha256 != expected_manifest_sha256 {
            return Err(KagemushaTopUpFinalityVerifyError::ManifestDigestMismatch);
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
        let roster_bytes = norito::encode_canonical(roster_artifact)
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

/// Verify one ABI-21 top-up proof using a process-wide bounded exact-roster
/// cache and the complete V4 anchor/manifest types.
pub fn verify_kagemusha_topup_finality_v4(
    proof: &KagemushaTopUpFinalityProofV2,
    roster_artifact: &KagemushaTopUpFinalityRosterArtifactV2,
    expected_anchor: &KagemushaRecursiveSpendTopUpAnchorV4,
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    expected_manifest_sha256: [u8; 32],
) -> Result<VerifiedKagemushaTopUpFinalityV2, KagemushaTopUpFinalityVerifyError> {
    static VERIFIER: OnceLock<KagemushaTopUpFinalityVerifier> = OnceLock::new();
    VERIFIER
        .get_or_init(KagemushaTopUpFinalityVerifier::new)
        .verify_v4(
            proof,
            roster_artifact,
            expected_anchor,
            manifest,
            expected_manifest_sha256,
        )
}

/// Verify one ABI-21 top-up proof against a clean candidate using the same
/// cryptographic verifier and bounded roster cache as production.
#[cfg(feature = "kagemusha-candidate-evidence-lab")]
pub fn verify_kagemusha_topup_finality_candidate_evidence_lab_v4(
    proof: &KagemushaTopUpFinalityProofV2,
    roster_artifact: &KagemushaTopUpFinalityRosterArtifactV2,
    expected_anchor: &KagemushaRecursiveSpendTopUpAnchorV4,
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    expected_manifest_sha256: [u8; 32],
) -> Result<VerifiedKagemushaTopUpFinalityV2, KagemushaTopUpFinalityVerifyError> {
    static VERIFIER: OnceLock<KagemushaTopUpFinalityVerifier> = OnceLock::new();
    VERIFIER
        .get_or_init(KagemushaTopUpFinalityVerifier::new)
        .verify_candidate_evidence_lab_v4(
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
    let bytes = norito::encode_canonical(value)
        .map_err(|_| KagemushaTopUpFinalityVerifyError::InvalidStructure)?;
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
        proposal_round: certificate.proposal_round,
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
    key.push(KAGEMUSHA_V4_TOPUP_ANCHOR_WITNESS_KEY_TAG);
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
        domain::DomainId,
        offline::{
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
            KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMS_IPA_FILE_NAME_V4,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMS_IPA_FILE_NAME_V4,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V4,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V4,
            KAGEMUSHA_RECURSIVE_SPEND_TOPUP_ANCHOR_VERSION_V4,
            KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_SCHEMA_V1, KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4,
            KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4,
            KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4,
            KAGEMUSHA_STEP_CIRCUIT_RELEASE_ADVICE_COLUMNS_V4,
            KAGEMUSHA_STEP_CIRCUIT_RELEASE_LOOKUP_COLUMNS_V4,
            KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2, KAGEMUSHA_TOPUP_FINALITY_PROOF_VERSION_V2,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4, KagemushaPastaCycleArtifactKindV4,
            KagemushaPastaCycleArtifactV4, KagemushaPastaCycleParityV1,
            KagemushaPastaCycleProofProfileV4, KagemushaPastaPublicLayoutV4,
            KagemushaRecursiveSpendArtifactBindingV4, KagemushaRecursiveSpendArtifactManifestV4,
            KagemushaRecursiveSpendTopUpAnchorV4, KagemushaReviewedSourceClosureV1,
            KagemushaScaledAmountV2, KagemushaSpendableNoteDescriptorV2,
            KagemushaStepCircuitParamsV4, KagemushaTopUpAnchorMerkleProofV2,
            KagemushaTopUpFinalityCompactQcV2, KagemushaTopUpFinalityHeightContextV2,
            KagemushaTopUpFinalityProofV2, KagemushaTopUpFinalityRosterArtifactReferenceV4,
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
        anchor: KagemushaRecursiveSpendTopUpAnchorV4,
        manifest: KagemushaRecursiveSpendArtifactManifestV4,
        manifest_digest: [u8; 32],
        finality_artifact: V2FinalityArtifact,
        signing_keys: Vec<KeyPair>,
    }

    fn reviewed_source_closure(
        source_commit: &str,
        source_tree_sha256: [u8; 32],
    ) -> (KagemushaReviewedSourceClosureV1, [u8; 32]) {
        let tracked_binary_diff_sha256 = Sha256::digest([0x93; 32]).into();
        let untracked_path_mode_blob_oid_manifest_sha256 = Sha256::digest([]).into();
        let mut combined = Sha256::new();
        combined.update(b"iroha-source-diff-v1\0");
        combined.update(b"tracked-binary-diff-sha256\0");
        combined.update(tracked_binary_diff_sha256);
        combined.update(b"untracked-path-blob-manifest-sha256\0");
        combined.update(untracked_path_mode_blob_oid_manifest_sha256);
        let closure = KagemushaReviewedSourceClosureV1 {
            schema: KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_SCHEMA_V1.to_owned(),
            base_commit: source_commit.to_owned(),
            source_commit: source_commit.to_owned(),
            source_repo_dirty: true,
            source_tree_sha256,
            tracked_binary_diff_sha256,
            untracked_file_count: 0,
            untracked_path_mode_blob_oid_manifest: Vec::new(),
            untracked_path_mode_blob_oid_manifest_sha256,
            ignored_cargo_lock_size_bytes: 1,
            ignored_cargo_lock_sha256: Sha256::digest([0x94]).into(),
            combined_source_fingerprint_sha256: combined.finalize().into(),
        };
        let descriptor_sha256 = closure
            .canonical_descriptor_sha256()
            .expect("finality reviewed source closure fixture");
        (closure, descriptor_sha256)
    }

    fn artifact(
        kind: KagemushaPastaCycleArtifactKindV4,
        file_name: &str,
        tag: u8,
    ) -> KagemushaPastaCycleArtifactV4 {
        KagemushaPastaCycleArtifactV4 {
            kind,
            file_name: file_name.to_owned(),
            size_bytes: 128,
            sha256: [tag; 32],
            payload_size_bytes: 64,
            payload_sha256: [tag.wrapping_add(1); 32],
        }
    }

    fn circuit_params() -> KagemushaStepCircuitParamsV4 {
        let k = KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4;
        let layout =
            KagemushaPastaPublicLayoutV4::for_ipa_round_count(k).expect("test V4 public layout");
        KagemushaStepCircuitParamsV4 {
            version: KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4,
            k,
            num_advice_per_phase: KAGEMUSHA_STEP_CIRCUIT_RELEASE_ADVICE_COLUMNS_V4.to_vec(),
            num_lookup_advice_per_phase: KAGEMUSHA_STEP_CIRCUIT_RELEASE_LOOKUP_COLUMNS_V4.to_vec(),
            num_fixed: 1,
            lookup_bits: k - 1,
            num_instance_columns: 1,
            public_input_limbs: layout.instance_column_limbs,
            minimum_unusable_rows: KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4,
            max_parent_proof_bytes: 4_096,
        }
    }

    fn profile(parity: KagemushaPastaCycleParityV1, tag: u8) -> KagemushaPastaCycleProofProfileV4 {
        let (circuit_id, names) = match parity {
            KagemushaPastaCycleParityV1::StepEq => (
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
                [
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMS_IPA_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4,
                ],
            ),
            KagemushaPastaCycleParityV1::StepEp => (
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
                [
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMS_IPA_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4,
                ],
            ),
        };
        let params = circuit_params();
        let artifacts = [
            KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            KagemushaPastaCycleArtifactKindV4::ProvingKey,
            KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
        ]
        .into_iter()
        .zip(names)
        .enumerate()
        .map(|(index, (kind, name))| {
            let index = u8::try_from(index).expect("four artifact roles fit u8");
            artifact(kind, name, tag + index * 2)
        })
        .collect();
        KagemushaPastaCycleProofProfileV4 {
            parity,
            circuit_id: circuit_id.to_owned(),
            parameter_generation: "params-generation-1".to_owned(),
            ipa_k: params.k,
            circuit_params: params.clone(),
            compiled_protocol_structure_sha256: [tag.wrapping_add(0x40); 32],
            step_proof_size_bytes: params.max_parent_proof_bytes,
            artifacts,
        }
    }

    fn fixture() -> Fixture {
        fixture_with_release_window(1, 100)
    }

    fn fixture_with_release_window(activation_height: u64, withdrawal_height: u64) -> Fixture {
        fixture_with_release_window_and_roster(activation_height, withdrawal_height, |_| {})
    }

    fn fixture_with_roster(
        mutate_roster: impl FnOnce(&mut KagemushaTopUpFinalityRosterArtifactV2),
    ) -> Fixture {
        fixture_with_release_window_and_roster(1, 100, mutate_roster)
    }

    fn fixture_with_release_window_and_roster(
        activation_height: u64,
        withdrawal_height: u64,
        mutate_roster: impl FnOnce(&mut KagemushaTopUpFinalityRosterArtifactV2),
    ) -> Fixture {
        let mut validators = (1_u8..=4)
            .map(|seed| {
                let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS key");
                let validator = ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                };
                let pop = iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture proof of possession");
                (validator, pop, key)
            })
            .collect::<Vec<_>>();
        validators.sort_unstable_by(|lhs, rhs| lhs.0.validator.cmp(&rhs.0.validator));
        let validator_set = validators
            .iter()
            .map(|(validator, _, _)| validator.clone())
            .collect::<Vec<_>>();
        let pops = validators
            .iter()
            .map(|(_, pop, _)| pop.clone())
            .collect::<Vec<_>>();
        let fixed_pops = pops
            .iter()
            .map(|pop| <[u8; 96]>::try_from(pop.as_slice()).expect("96-byte PoP"))
            .collect::<Vec<_>>();
        let keys = validators
            .into_iter()
            .map(|(_, _, key)| key)
            .collect::<Vec<_>>();
        let chain_id = ChainId::from("kagemusha-finality-chain");
        let asset = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("asset domain"),
            "rose".parse().expect("asset name"),
        );
        let payer_key =
            KeyPair::try_from_seed(vec![0x51; 32], Algorithm::Ed25519).expect("payer key");
        let payer = AccountId::new(payer_key.public_key().clone());
        let height = 42;
        let window = KagemushaTopUpFinalityRosterWindowV2 {
            activates_at_height: 1,
            withdraws_at_height: 100,
            consensus_mode: ConsensusMode::Permissioned,
            validator_set: validator_set.clone(),
            validator_set_pops: fixed_pops.clone(),
        };
        let mut roster = KagemushaTopUpFinalityRosterArtifactV2 {
            version: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2,
            chain_id: chain_id.clone(),
            artifact_generation: "release-generation-1".to_owned(),
            windows: vec![window],
        };
        mutate_roster(&mut roster);
        let roster_bytes = norito::encode_canonical(&roster).expect("roster bytes");
        let roster_digest = Sha256::digest(&roster_bytes).into();
        let source_commit = "0123456789abcdef0123456789abcdef01234567";
        let source_tree_sha256 = [0x52; 32];
        let (reviewed_source_closure, reviewed_source_closure_descriptor_sha256) =
            reviewed_source_closure(source_commit, source_tree_sha256);
        let manifest = KagemushaRecursiveSpendArtifactManifestV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
            bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
            transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
            generation: "release-generation-1".to_owned(),
            source_commit: source_commit.to_owned(),
            source_tree_sha256,
            source_repo_dirty: true,
            reviewed_source_closure,
            reviewed_source_closure_descriptor_sha256,
            chain_id: chain_id.clone(),
            asset: asset.clone(),
            asset_scale: 2,
            activation_height,
            withdrawal_height,
            max_proof_bytes: 9_000,
            profiles: vec![
                profile(KagemushaPastaCycleParityV1::StepEq, 0x20),
                profile(KagemushaPastaCycleParityV1::StepEp, 0x30),
            ],
            topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactReferenceV4 {
                file_name: KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4.to_owned(),
                size_bytes: u64::try_from(roster_bytes.len()).expect("roster size"),
                sha256: roster_digest,
                artifact_generation: "release-generation-1".to_owned(),
                circuit_id: KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2.to_owned(),
                purpose: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2.to_owned(),
                artifact_type: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2.to_owned(),
                required_bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            },
            benchmark_evidence_sha256: [0x71; 32],
            cryptographic_review_sha256: [0x72; 32],
            release_attestation_sha256: [0x73; 32],
        };
        manifest.validate().expect("manifest");
        let manifest_digest = canonical_sha256(&manifest).expect("manifest digest");
        let operation_id = [0xA5; 32];
        let note = KagemushaSpendableNoteDescriptorV2 {
            chain_id: chain_id.clone(),
            asset: asset.clone(),
            note_commitment: [0x31; 32],
            spend_nullifier: [0x32; 32],
            amount: KagemushaScaledAmountV2::new(500, 2).expect("amount"),
        };
        let anchor = KagemushaRecursiveSpendTopUpAnchorV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_TOPUP_ANCHOR_VERSION_V4,
            chain_id: chain_id.clone(),
            payer: payer.clone(),
            asset: AssetId::new(asset.clone(), payer),
            asset_scale: 2,
            amount: note.amount,
            initial_root: [0x11; 32],
            finalized_root: [0x12; 32],
            shield_leaf_index: 3,
            current_note: note,
            topup_operation_id: operation_id,
            shield_verifier_id: VerifyingKeyId::new("halo2/ipa", "topup-shield-v2"),
            shield_verifier_commitment: [0x14; 32],
            artifact_binding: KagemushaRecursiveSpendArtifactBindingV4 {
                version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
                generation: manifest.generation.clone(),
                manifest_sha256: manifest_digest,
            },
            finalized_height: height,
            finalized_tx_hash: [0x15; 32],
            anchor_digest: [0; 32],
        }
        .finalize_digest()
        .expect("anchor");
        let mut witness_key = vec![KAGEMUSHA_V4_TOPUP_ANCHOR_WITNESS_KEY_TAG];
        witness_key.extend_from_slice(&operation_id);
        let mut other_key = vec![KAGEMUSHA_V4_TOPUP_ANCHOR_WITNESS_KEY_TAG];
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
            snapshot_bootstrap: None,
            mode: ConsensusMode::Permissioned,
            parent_commit_qc: Some(QuorumCertificate {
                round: ConsensusRound {
                    context_id: iroha_data_model::block::consensus_v2::HeightContextId(
                        HashOf::from_untyped_unchecked(Hash::new(b"parent context")),
                    ),
                    height: height - 1,
                    view: 0,
                },
                proposal_round: ConsensusRound {
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
                execution_commitment: ExecutionCommitment::without_topups_or_merge_carrier(
                    Hash::new(b"parent parent state"),
                    Hash::new(b"parent post state"),
                    Hash::new(b"parent ordinary writes"),
                    1,
                    Hash::new(b"parent executed block wire"),
                ),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0x44; 96],
            }),
            roster: validator_set.clone(),
            quorum: DualQuorum::from_roster(&validator_set).expect("quorum"),
            nexus_amx_context_hash: Hash::new(b"nexus"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
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
        let execution_commitment = ExecutionCommitment::new_without_merge_carrier(
            Hash::new(b"parent state"),
            commitment.post_state_root,
            commitment.ordinary_writes_root,
            Some(commitment.topup_anchor_root),
            2,
            1,
            Hash::new(b"finalized executed block wire"),
        )
        .expect("execution commitment");
        let round = ConsensusRound {
            context_id: context.id(),
            height,
            view: 3,
        };
        let mut certificate = QuorumCertificate {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let preimage = Vote {
            round: certificate.round,
            proposal_round: certificate.proposal_round,
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
            snapshot_bootstrap: context.snapshot_bootstrap,
            nexus_amx_context_hash: context.nexus_amx_context_hash,
            execution_policy_hash: context.execution_policy_hash,
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
        verifier.verify_v4(
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
            snapshot_bootstrap: context.snapshot_bootstrap,
            nexus_amx_context_hash: context.nexus_amx_context_hash,
            execution_policy_hash: context.execution_policy_hash,
            da_layout: context.da_layout,
            leader_seed: context.leader_seed,
        };
        let certificate = &mut proof.commit_qc.certificate;
        certificate.round.context_id = context.id();
        certificate.proposal_round.context_id = context.id();
        let preimage = Vote {
            round: certificate.round,
            proposal_round: certificate.proposal_round,
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
    fn finality_manifest_and_roster_identities_ignore_ambient_norito_layout() {
        let fixture = fixture();
        let expected_manifest =
            canonical_sha256(&fixture.manifest).expect("canonical manifest digest");
        let expected_roster =
            norito::encode_canonical(&fixture.roster).expect("canonical roster frame");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_ne!(
            norito::to_bytes(&fixture.roster).expect("alternate-layout roster frame"),
            expected_roster
        );
        assert_eq!(
            norito::encode_canonical(&fixture.roster).expect("ambient-independent roster frame"),
            expected_roster
        );
        assert_eq!(
            canonical_sha256(&fixture.manifest).expect("ambient-independent manifest digest"),
            expected_manifest
        );
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
    fn aggregate_signature_authenticates_proposal_origin() {
        let fixture = fixture();
        let mut changed_origin = fixture.proof.clone();
        changed_origin.commit_qc.certificate.proposal_round.view = changed_origin
            .commit_qc
            .certificate
            .proposal_round
            .view
            .saturating_sub(1);

        assert_eq!(
            KagemushaTopUpFinalityVerifier::new()
                .verify_v4(
                    &changed_origin,
                    &fixture.roster,
                    &fixture.anchor,
                    &fixture.manifest,
                    fixture.manifest_digest,
                )
                .expect_err("a non-same-round proposal origin must fail structural validation"),
            KagemushaTopUpFinalityVerifyError::InvalidStructure
        );
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
                .verify_v4(
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
                .verify_v4(
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
        manifest.asset = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("asset domain"),
            "other".parse().expect("asset name"),
        );
        let digest = canonical_sha256(&manifest).expect("digest");
        assert_eq!(
            verifier
                .verify_v4(
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
                .verify_v4(
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
        anchor.artifact_binding.generation = "other-generation".to_owned();
        anchor = anchor
            .finalize_digest()
            .expect("alternate generation anchor");
        let mut proof = fixture.proof.clone();
        proof.anchor = anchor.compact_ref().unwrap();
        assert_eq!(
            verifier
                .verify_v4(
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
        anchor.artifact_binding.manifest_sha256[0] ^= 1;
        anchor = anchor
            .finalize_digest()
            .expect("alternate manifest-bound anchor");
        let mut proof = fixture.proof.clone();
        proof.anchor = anchor.compact_ref().unwrap();
        assert_eq!(
            verifier
                .verify_v4(
                    &proof,
                    &fixture.roster,
                    &anchor,
                    &fixture.manifest,
                    fixture.manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::ManifestDigestMismatch
        );

        let mut anchor = fixture.anchor.clone();
        anchor.finalized_height += 1;
        anchor = anchor.finalize_digest().expect("alternate height anchor");
        let mut proof = fixture.proof.clone();
        proof.anchor = anchor.compact_ref().unwrap();
        assert_eq!(
            verifier
                .verify_v4(
                    &proof,
                    &fixture.roster,
                    &anchor,
                    &fixture.manifest,
                    fixture.manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::HeightMismatch
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
                .verify_v4(
                    &context,
                    &fixture.roster,
                    &fixture.anchor,
                    &fixture.manifest,
                    fixture.manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::RosterContextMismatch
        );

        let swapped_roster = fixture_with_roster(|roster| {
            roster.windows[0].validator_set.swap(0, 1);
        });
        assert_eq!(
            verifier
                .verify_v4(
                    &swapped_roster.proof,
                    &swapped_roster.roster,
                    &swapped_roster.anchor,
                    &swapped_roster.manifest,
                    swapped_roster.manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::InvalidStructure
        );

        let changed_power = fixture_with_roster(|roster| {
            roster.windows[0].consensus_mode = ConsensusMode::Npos;
            roster.windows[0].validator_set[0].power = 2;
        });
        assert_eq!(
            verifier
                .verify_v4(
                    &changed_power.proof,
                    &changed_power.roster,
                    &changed_power.anchor,
                    &changed_power.manifest,
                    changed_power.manifest_digest,
                )
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
                .verify_v4(
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
                .verify_v4(
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
                .verify_v4(
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
                .verify_v4(
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

        let invalid_roster = fixture_with_roster(|roster| {
            roster.windows[0].validator_set_pops[0][0] ^= 1;
        });
        assert_eq!(
            verifier
                .verify_v4(
                    &invalid_roster.proof,
                    &invalid_roster.roster,
                    &invalid_roster.anchor,
                    &invalid_roster.manifest,
                    invalid_roster.manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::InvalidRosterCryptography
        );
        assert_eq!(verifier.roster_crypto_verification_count(), 1);
        assert_eq!(
            verifier
                .verify_v4(
                    &invalid_roster.proof,
                    &invalid_roster.roster,
                    &invalid_roster.anchor,
                    &invalid_roster.manifest,
                    invalid_roster.manifest_digest,
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

        let mut execution_policy = fixture.proof.clone();
        execution_policy
            .commit_qc
            .height_context
            .execution_policy_hash = Hash::new(b"other execution policy");
        mutations.push(("execution policy", execution_policy));

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
                    .verify_v4(
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
                .verify_v4(
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
                .verify_v4(
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
                .verify_v4(
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
        let boundary = fixture_with_release_window(42, 43);
        verifier
            .verify_v4(
                &boundary.proof,
                &boundary.roster,
                &boundary.anchor,
                &boundary.manifest,
                boundary.manifest_digest,
            )
            .expect("activation height and withdrawal minus one are accepted");

        let before_activation = fixture_with_release_window(43, 100);
        assert_eq!(
            verifier
                .verify_v4(
                    &before_activation.proof,
                    &before_activation.roster,
                    &before_activation.anchor,
                    &before_activation.manifest,
                    before_activation.manifest_digest,
                )
                .unwrap_err(),
            KagemushaTopUpFinalityVerifyError::ReleaseWindowMismatch
        );

        let at_withdrawal = fixture_with_release_window(1, 42);
        assert_eq!(
            verifier
                .verify_v4(
                    &at_withdrawal.proof,
                    &at_withdrawal.roster,
                    &at_withdrawal.anchor,
                    &at_withdrawal.manifest,
                    at_withdrawal.manifest_digest,
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
            .verify_v4(
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
                .verify_v4(
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
                .verify_v4(
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
                    .verify_v4(
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
