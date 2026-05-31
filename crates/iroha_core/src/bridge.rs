//! Helpers for bridge finality proofs built from commit certificates.

use std::{
    collections::BTreeSet,
    num::NonZeroUsize,
    sync::{Arc, Mutex, OnceLock},
};

use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::{
    ChainId,
    block::{BlockHeader, SignedBlock},
    bridge::{
        BridgeAuthoritySet, BridgeCommitment, BridgeCommitmentJustification, BridgeFinalityBundle,
        BridgeFinalityProof,
    },
    consensus::{Qc, VALIDATOR_SET_HASH_VERSION_V1},
    isi::InstructionBox,
    peer::PeerId,
    transaction::Executable,
};
use iroha_sccp::{
    NexusBridgeFinalityProofV1, NexusConsensusPhaseV1, NexusQcRefV1, SccpHubCommitmentV1,
    SccpPayloadV1,
};
use thiserror::Error;

use crate::{
    mmr::BlockMmr,
    state::{
        State as CoreState, StateReadOnly, StateTransaction, consensus_key_pop_for_public_key,
    },
    sumeragi,
    tx::AcceptedTransaction,
};

/// Narrow read-only surface used by bridge finality proof builders.
///
/// This keeps bridge-proof construction independent from full `StateView` snapshots.
pub trait BridgeStateReadOnly {
    /// Chain identifier bound to the state snapshot.
    fn bridge_chain_id(&self) -> &ChainId;
    /// Load a committed block at `height`.
    fn bridge_block_by_height(&self, height: NonZeroUsize) -> Option<Arc<SignedBlock>>;
    /// Resolve a validator consensus-key proof-of-possession by public key.
    fn bridge_validator_pop(&self, public_key: &PublicKey) -> Option<Vec<u8>>;
}

impl<T: StateReadOnly> BridgeStateReadOnly for T {
    fn bridge_chain_id(&self) -> &ChainId {
        self.chain_id()
    }

    fn bridge_block_by_height(&self, height: NonZeroUsize) -> Option<Arc<SignedBlock>> {
        self.kura().get_block(height)
    }

    fn bridge_validator_pop(&self, public_key: &PublicKey) -> Option<Vec<u8>> {
        consensus_key_pop_for_public_key(self.world(), public_key)
    }
}

impl BridgeStateReadOnly for CoreState {
    fn bridge_chain_id(&self) -> &ChainId {
        self.chain_id_ref()
    }

    fn bridge_block_by_height(&self, height: NonZeroUsize) -> Option<Arc<SignedBlock>> {
        self.block_by_height(height)
    }

    fn bridge_validator_pop(&self, public_key: &PublicKey) -> Option<Vec<u8>> {
        let world = self.world_view();
        consensus_key_pop_for_public_key(&world, public_key)
    }
}

/// Narrow read-only surface used to validate SCCP finality proofs against local state.
pub trait SccpFinalityStateReadOnly {
    /// Chain identifier bound to the local committed chain.
    fn sccp_chain_id(&self) -> &ChainId;
    /// Load the locally committed block at `height`.
    fn sccp_block_by_height(&self, height: NonZeroUsize) -> Option<Arc<SignedBlock>>;
    /// Load the locally trusted commit QC for `height` and `block_hash`.
    fn sccp_commit_qc_for_block(&self, height: u64, block_hash: HashOf<BlockHeader>) -> Option<Qc>;
}

impl SccpFinalityStateReadOnly for CoreState {
    fn sccp_chain_id(&self) -> &ChainId {
        self.chain_id_ref()
    }

    fn sccp_block_by_height(&self, height: NonZeroUsize) -> Option<Arc<SignedBlock>> {
        self.block_by_height(height)
    }

    fn sccp_commit_qc_for_block(&self, height: u64, block_hash: HashOf<BlockHeader>) -> Option<Qc> {
        self.commit_roster_snapshot_for_block(height, block_hash)
            .map(|snapshot| snapshot.commit_qc)
    }
}

impl SccpFinalityStateReadOnly for StateTransaction<'_, '_> {
    fn sccp_chain_id(&self) -> &ChainId {
        &self.chain_id
    }

    fn sccp_block_by_height(&self, height: NonZeroUsize) -> Option<Arc<SignedBlock>> {
        self.block_by_height(height)
    }

    fn sccp_commit_qc_for_block(&self, height: u64, block_hash: HashOf<BlockHeader>) -> Option<Qc> {
        self.commit_qc_for_block(height, block_hash)
    }
}

struct MmrCache {
    mmr: BlockMmr,
    height: u64,
    /// Chain id used to detect cross-ledger reuse in-process.
    chain_id: Option<ChainId>,
    /// Cached hash for the tip at `height` to detect top-block rewrites.
    tip_hash: Option<HashOf<BlockHeader>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Decoded SCCP message plus its location in a transaction stream.
pub struct RecordedSccpMessage {
    /// Zero-based index of the transaction that emitted the SCCP message.
    pub tx_index: usize,
    /// Zero-based index of the instruction within the transaction executable.
    pub instruction_index: usize,
    /// Canonically decoded SCCP payload recorded by the instruction.
    pub payload: SccpPayloadV1,
    /// Commitment derived from the decoded SCCP payload.
    pub commitment: SccpHubCommitmentV1,
}

fn decode_recorded_sccp_message(instruction: &InstructionBox) -> Option<SccpPayloadV1> {
    let record = instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::bridge::RecordSccpMessage>()?;
    let payload = iroha_sccp::decode_canonical_sccp_payload_bytes(&record.payload_bytes)?;
    iroha_sccp::verify_sccp_payload_structure(&payload).then_some(payload)
}

fn collect_sccp_messages_from_executable(
    tx_index: usize,
    executable: &Executable,
    out: &mut Vec<RecordedSccpMessage>,
) {
    let mut push_instruction = |instruction_index: usize, instruction: &InstructionBox| {
        let Some(payload) = decode_recorded_sccp_message(instruction) else {
            return;
        };
        out.push(RecordedSccpMessage {
            tx_index,
            instruction_index,
            commitment: iroha_sccp::hub_commitment_from_sccp_payload(&payload),
            payload,
        });
    };

    match executable {
        Executable::Instructions(_) | Executable::ContractCall(_) | Executable::Ivm(_) => {}
        Executable::IvmProved(proved) => {
            for (instruction_index, instruction) in proved.overlay.iter().enumerate() {
                push_instruction(instruction_index, instruction);
            }
        }
    }
}

/// Extract all SCCP message records from accepted external transactions.
pub fn collect_sccp_messages_from_accepted_transactions(
    transactions: &[AcceptedTransaction<'_>],
) -> Vec<RecordedSccpMessage> {
    let mut messages = Vec::new();
    for (tx_index, transaction) in transactions.iter().enumerate() {
        if let Some(signed) = transaction.external() {
            collect_sccp_messages_from_executable(tx_index, signed.instructions(), &mut messages);
        }
    }
    messages
}

/// Extract all SCCP message records from the external transactions in a signed block.
pub fn collect_sccp_messages_from_signed_block(block: &SignedBlock) -> Vec<RecordedSccpMessage> {
    let mut messages = Vec::new();
    for (tx_index, transaction) in block.external_transactions().enumerate() {
        collect_sccp_messages_from_executable(tx_index, transaction.instructions(), &mut messages);
    }
    messages
}

/// Compute the SCCP commitment Merkle root for a set of recorded messages.
pub fn sccp_commitment_root_from_messages(messages: &[RecordedSccpMessage]) -> Option<[u8; 32]> {
    let commitments: Vec<_> = messages
        .iter()
        .map(|message| message.commitment.clone())
        .collect();
    iroha_sccp::commitment_merkle_root(&commitments)
}

/// Errors returned when constructing a bridge finality proof.
#[allow(variant_size_differences)]
#[derive(Debug, Error, Copy, Clone)]
pub enum BridgeFinalityError {
    /// The requested block height is zero or does not fit into the host pointer width.
    #[error("invalid block height {0}")]
    InvalidHeight(u64),
    /// The block at the requested height was not found.
    #[error("block at height {0} not found")]
    BlockNotFound(u64),
    /// No commit certificate was found for the requested height.
    #[error("commit certificate for height {0} not found")]
    QcNotFound(u64),
    /// The commit certificate references a different block hash than the stored block.
    #[error(
        "commit certificate hash {cert_hash:?} does not match block hash {block_hash:?} at height {height}"
    )]
    QcHashMismatch {
        /// Height being proven.
        height: u64,
        /// Hash recorded inside the commit certificate.
        cert_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
        /// Hash of the stored block header.
        block_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
    },
    /// Validator `PoP` missing for the validator set entry.
    #[error("validator PoP missing for index {index}")]
    MissingValidatorPop {
        /// Index into the validator set.
        index: usize,
    },
}

fn compute_block_mmr(
    state: &impl BridgeStateReadOnly,
    height: u64,
) -> Result<BlockMmr, BridgeFinalityError> {
    static BLOCK_MMR_CACHE: OnceLock<Mutex<MmrCache>> = OnceLock::new();

    if height == 0 {
        return Err(BridgeFinalityError::InvalidHeight(height));
    }

    let cache = BLOCK_MMR_CACHE.get_or_init(|| {
        Mutex::new(MmrCache {
            mmr: BlockMmr::default(),
            height: 0,
            chain_id: None,
            tip_hash: None,
        })
    });

    let mut guard = cache.lock().expect("mmr cache mutex poisoned");
    let chain_id = state.bridge_chain_id().clone();

    let mut rebuild = height < guard.height || guard.chain_id.as_ref() != Some(&chain_id);
    if !rebuild && guard.height > 0 {
        let cached_tip = guard.tip_hash;
        let current_tip = block_hash_at(state, guard.height)?;
        if cached_tip != Some(current_tip) {
            rebuild = true;
        }
    }

    if rebuild {
        // Rebuild from genesis to requested height to avoid rollback complexity.
        let mut fresh = BlockMmr::default();
        let mut tip_hash = None;
        for h in 1..=height {
            let hash = block_hash_at(state, h)?;
            fresh.push(hash);
            tip_hash = Some(hash);
        }
        guard.mmr = fresh;
        guard.height = height;
        guard.chain_id = Some(chain_id);
        guard.tip_hash = tip_hash;
    } else {
        let mut tip_hash = guard.tip_hash;
        for h in (guard.height + 1)..=height {
            let hash = block_hash_at(state, h)?;
            guard.mmr.push(hash);
            guard.height = h;
            tip_hash = Some(hash);
        }
        guard.chain_id = Some(chain_id);
        guard.tip_hash = tip_hash;
    }

    Ok(guard.mmr.clone())
}

fn block_hash_at(
    state: &impl BridgeStateReadOnly,
    height: u64,
) -> Result<iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>, BridgeFinalityError> {
    let h_usize: usize = height
        .try_into()
        .map_err(|_| BridgeFinalityError::InvalidHeight(height))?;
    let nonzero = NonZeroUsize::new(h_usize).ok_or(BridgeFinalityError::InvalidHeight(height))?;
    let block = state
        .bridge_block_by_height(nonzero)
        .ok_or(BridgeFinalityError::BlockNotFound(height))?;
    Ok(block.hash())
}

/// Build a self-contained finality proof for the block at `height`.
///
/// The proof bundles the block header, its hash, and the commit certificate
/// collected for that block. Verifiers recompute the block hash from the header
/// and validate the commit certificate signatures against the provided
/// validator set.
///
/// # Errors
///
/// Returns [`BridgeFinalityError`] when the height is invalid, the block or commit
/// certificate is missing, or their hashes do not match.
pub fn build_finality_proof(
    state: &impl BridgeStateReadOnly,
    height: u64,
) -> Result<BridgeFinalityProof, BridgeFinalityError> {
    let height_usize: usize = height
        .try_into()
        .map_err(|_| BridgeFinalityError::InvalidHeight(height))?;
    let nonzero_height =
        NonZeroUsize::new(height_usize).ok_or(BridgeFinalityError::InvalidHeight(height))?;

    let block = state
        .bridge_block_by_height(nonzero_height)
        .ok_or(BridgeFinalityError::BlockNotFound(height))?;
    let block_header = block.header();
    let block_hash = block.hash();

    let mut cert_candidates: Vec<_> = sumeragi::status::commit_qc_history()
        .into_iter()
        .filter(|entry| entry.height == height)
        .collect();
    let cert = if let Some(cert) = cert_candidates
        .iter()
        .find(|candidate| candidate.subject_block_hash == block_hash)
    {
        cert.clone()
    } else if let Some(cert) = cert_candidates.pop() {
        return Err(BridgeFinalityError::QcHashMismatch {
            height,
            cert_hash: cert.subject_block_hash,
            block_hash,
        });
    } else {
        return Err(BridgeFinalityError::QcNotFound(height));
    };

    let mut validator_set_pops = Vec::with_capacity(cert.validator_set.len());
    for (index, peer) in cert.validator_set.iter().enumerate() {
        let Some(pop) = state.bridge_validator_pop(peer.public_key()) else {
            return Err(BridgeFinalityError::MissingValidatorPop { index });
        };
        validator_set_pops.push(pop);
    }

    Ok(BridgeFinalityProof {
        height,
        chain_id: state.bridge_chain_id().clone(),
        block_header,
        block_hash,
        commit_qc: cert,
        validator_set_pops,
    })
}

/// Build a commitment + justification bundle for the block at `height`.
///
/// The bundle relies on the commit certificate aggregate signature for
/// justification; the historical signature list is left empty.
///
/// # Errors
///
/// Returns [`BridgeFinalityError`] when the underlying finality proof or block MMR
/// cannot be built for the requested height.
pub fn build_finality_bundle(
    state: &impl BridgeStateReadOnly,
    height: u64,
) -> Result<BridgeFinalityBundle, BridgeFinalityError> {
    let proof = build_finality_proof(state, height)?;
    let mmr = compute_block_mmr(state, height)?;
    let mmr_root = mmr.root();
    let authority_set = BridgeAuthoritySet {
        id: height, // simple monotonically increasing id derived from height; future revisions can carry explicit ids
        validator_set: proof.commit_qc.validator_set.clone(),
        validator_set_hash: proof.commit_qc.validator_set_hash,
        validator_set_hash_version: proof.commit_qc.validator_set_hash_version,
    };
    let commitment = BridgeCommitment {
        chain_id: proof.chain_id.clone(),
        authority_set: authority_set.clone(),
        block_height: proof.height,
        block_hash: proof.block_hash,
        mmr_root,
        mmr_leaf_index: mmr.leaves().checked_sub(1),
        mmr_peaks: Some(mmr.peaks.iter().map(|p| p.hash).collect()),
        next_authority_set: None,
    };
    let justification = BridgeCommitmentJustification {
        signatures: Vec::new(),
    };
    Ok(BridgeFinalityBundle {
        commitment,
        justification,
        block_header: proof.block_header,
        commit_qc: proof.commit_qc,
    })
}

/// Verification errors raised when checking a [`BridgeFinalityProof`].
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BridgeFinalityVerificationError {
    /// The proof carries a different chain id than expected.
    #[error("finality proof chain_id mismatch: expected {expected}, actual {actual}")]
    ChainIdMismatch {
        /// Chain id the verifier expects.
        expected: ChainId,
        /// Chain id carried inside the proof.
        actual: ChainId,
    },
    /// The caller expected a different height than the proof advertises.
    #[error("finality proof height mismatch: expected {expected}, proof {actual}")]
    HeightMismatch {
        /// Height the verifier expected.
        expected: u64,
        /// Height carried in the proof.
        actual: u64,
    },
    /// The block header height disagrees with the proof height.
    #[error("block header height {header_height} does not match proof height {proof_height}")]
    BlockHeaderHeightMismatch {
        /// Height in the proof.
        proof_height: u64,
        /// Height carried in the block header.
        header_height: u64,
    },
    /// The commit certificate height disagrees with the proof height.
    #[error("commit certificate height {cert_height} does not match proof height {proof_height}")]
    QcHeightMismatch {
        /// Height in the proof.
        proof_height: u64,
        /// Height carried in the commit certificate.
        cert_height: u64,
    },
    /// Commit certificate phase is not `Commit`.
    #[error("unexpected commit certificate phase {actual:?}")]
    UnexpectedCertificatePhase {
        /// Phase carried in the commit certificate.
        actual: sumeragi::consensus::Phase,
    },
    /// Recomputed block hash does not match the proof/certificate payloads.
    #[error(
        "block hash mismatch: header {header_hash:?}, proof {proof_hash:?}, certificate {certificate_hash:?}"
    )]
    BlockHashMismatch {
        /// Hash recomputed from the block header.
        header_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
        /// Hash carried in the proof.
        proof_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
        /// Hash advertised inside the commit certificate.
        certificate_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
    },
    /// Validator set hash version advertised by the certificate is unsupported.
    #[error("unsupported validator_set_hash_version {version}")]
    UnsupportedValidatorSetHashVersion {
        /// Unsupported version encountered.
        version: u16,
    },
    /// Recomputed validator set hash does not match the certificate payload.
    #[error("validator_set_hash mismatch: computed {computed:?}, advertised {advertised:?}")]
    ValidatorSetHashMismatch {
        /// Hash recomputed from the validator set.
        computed: HashOf<Vec<PeerId>>,
        /// Hash advertised in the commit certificate.
        advertised: HashOf<Vec<PeerId>>,
    },
    /// The verifier pinned a validator set hash that does not match the proof.
    #[error("trusted validator_set_hash mismatch: trusted {trusted:?}, certificate {advertised:?}")]
    TrustedValidatorSetHashMismatch {
        /// Trusted validator set hash supplied by the verifier.
        trusted: HashOf<Vec<PeerId>>,
        /// Hash recomputed from the proof validator set.
        advertised: HashOf<Vec<PeerId>>,
    },
    /// Validator set is empty, so no quorum can be reached.
    #[error("validator set is empty")]
    EmptyValidatorSet,
    /// Validator-set `PoP` length does not match the validator-set length.
    #[error("validator set pop length mismatch: expected {expected}, got {actual}")]
    ValidatorSetPopLengthMismatch {
        /// Expected `PoP` count.
        expected: usize,
        /// Actual `PoP` count.
        actual: usize,
    },
    /// Signer bitmap length does not match the validator set size.
    #[error("signer bitmap length mismatch: expected {expected}, got {actual}")]
    SignerBitmapLengthMismatch {
        /// Expected bitmap length in bytes.
        expected: usize,
        /// Actual bitmap length in bytes.
        actual: usize,
    },
    /// A signer index falls outside the validator set bounds.
    #[error("signer index {signer} is out of bounds for roster length {roster_len}")]
    SignerOutOfBounds {
        /// Offending signer index.
        signer: u64,
        /// Length of the validator set.
        roster_len: usize,
    },
    /// Duplicate signer index detected inside the commit certificate.
    #[error("duplicate signer index {signer} in commit certificate signatures")]
    DuplicateSigner {
        /// Signer index that appears multiple times.
        signer: u64,
    },
    /// Quorum was not met when counting unique signatures.
    #[error("insufficient signatures: collected {collected}, required {required}")]
    InsufficientSignatures {
        /// Unique signatures collected.
        collected: usize,
        /// Required quorum.
        required: usize,
    },
    /// Commit certificate carries no aggregate signature.
    #[error("commit certificate aggregate signature is missing")]
    AggregateSignatureMissing,
    /// Commit certificate aggregate signature failed verification.
    #[error("commit certificate aggregate signature is invalid")]
    AggregateSignatureInvalid,
}

/// Verification knobs for [`verify_finality_proof`].
#[derive(Debug, Clone)]
pub struct FinalityProofVerificationConfig<'a> {
    /// Chain identifier expected by the verifier.
    pub expected_chain_id: &'a ChainId,
    /// Optional expected height to bind the proof to a specific block.
    pub expected_height: Option<u64>,
    /// Optional trusted validator set hash anchor to guard against roster replays.
    pub trusted_validator_set_hash: Option<HashOf<Vec<PeerId>>>,
}

const fn min_votes_for_len(len: usize) -> usize {
    if len > 3 {
        ((len.saturating_sub(1)) / 3) * 2 + 1
    } else {
        len
    }
}

/// Verify a [`BridgeFinalityProof`] against chain/height/validator set expectations.
///
/// Callers supply the expected chain id and may optionally bind the proof to a specific
/// height and validator set hash. Verification recomputes the block hash, enforces
/// validator set hashing rules, and checks signatures for quorum and validity.
///
/// # Errors
/// Returns [`BridgeFinalityVerificationError`] when the proof fails chain/height checks,
/// validator set hashing/anchors, or signature validation.
#[allow(clippy::too_many_lines)]
pub fn verify_finality_proof(
    proof: &BridgeFinalityProof,
    config: &FinalityProofVerificationConfig<'_>,
) -> Result<(), BridgeFinalityVerificationError> {
    if proof.chain_id != *config.expected_chain_id {
        return Err(BridgeFinalityVerificationError::ChainIdMismatch {
            expected: config.expected_chain_id.clone(),
            actual: proof.chain_id.clone(),
        });
    }

    if let Some(expected_height) = config.expected_height {
        if proof.height != expected_height {
            return Err(BridgeFinalityVerificationError::HeightMismatch {
                expected: expected_height,
                actual: proof.height,
            });
        }
    }

    let header_height = proof.block_header.height().get();
    if header_height != proof.height {
        return Err(BridgeFinalityVerificationError::BlockHeaderHeightMismatch {
            proof_height: proof.height,
            header_height,
        });
    }

    let certificate = &proof.commit_qc;
    if certificate.height != proof.height {
        return Err(BridgeFinalityVerificationError::QcHeightMismatch {
            proof_height: proof.height,
            cert_height: certificate.height,
        });
    }

    if certificate.phase != sumeragi::consensus::Phase::Commit {
        return Err(
            BridgeFinalityVerificationError::UnexpectedCertificatePhase {
                actual: certificate.phase,
            },
        );
    }

    let header_hash = proof.block_header.hash();
    if header_hash != proof.block_hash || header_hash != certificate.subject_block_hash {
        return Err(BridgeFinalityVerificationError::BlockHashMismatch {
            header_hash,
            proof_hash: proof.block_hash,
            certificate_hash: certificate.subject_block_hash,
        });
    }

    if certificate.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1 {
        return Err(
            BridgeFinalityVerificationError::UnsupportedValidatorSetHashVersion {
                version: certificate.validator_set_hash_version,
            },
        );
    }

    let computed_set_hash = HashOf::new(&certificate.validator_set);
    if computed_set_hash != certificate.validator_set_hash {
        return Err(BridgeFinalityVerificationError::ValidatorSetHashMismatch {
            computed: computed_set_hash,
            advertised: certificate.validator_set_hash,
        });
    }

    if let Some(trusted) = config.trusted_validator_set_hash {
        if trusted != computed_set_hash {
            return Err(
                BridgeFinalityVerificationError::TrustedValidatorSetHashMismatch {
                    trusted,
                    advertised: computed_set_hash,
                },
            );
        }
    }

    let roster_len = certificate.validator_set.len();
    if roster_len == 0 {
        return Err(BridgeFinalityVerificationError::EmptyValidatorSet);
    }
    if proof.validator_set_pops.len() != roster_len {
        return Err(
            BridgeFinalityVerificationError::ValidatorSetPopLengthMismatch {
                expected: roster_len,
                actual: proof.validator_set_pops.len(),
            },
        );
    }
    let expected_bitmap_len = roster_len.div_ceil(8);
    if certificate.aggregate.signers_bitmap.len() != expected_bitmap_len {
        return Err(
            BridgeFinalityVerificationError::SignerBitmapLengthMismatch {
                expected: expected_bitmap_len,
                actual: certificate.aggregate.signers_bitmap.len(),
            },
        );
    }
    let required = min_votes_for_len(roster_len);
    let mut seen = BTreeSet::new();
    for (byte_idx, byte) in certificate.aggregate.signers_bitmap.iter().enumerate() {
        if *byte == 0 {
            continue;
        }
        for bit in 0..8 {
            if (byte >> bit) & 1 == 0 {
                continue;
            }
            let idx = byte_idx * 8 + bit;
            let signer = u64::try_from(idx).unwrap_or(u64::MAX);
            if idx >= roster_len {
                return Err(BridgeFinalityVerificationError::SignerOutOfBounds {
                    signer,
                    roster_len,
                });
            }
            if !seen.insert(signer) {
                return Err(BridgeFinalityVerificationError::DuplicateSigner { signer });
            }
        }
    }

    if certificate.aggregate.bls_aggregate_signature.is_empty() {
        return Err(BridgeFinalityVerificationError::AggregateSignatureMissing);
    }

    let collected = seen.len();
    if collected < required {
        return Err(BridgeFinalityVerificationError::InsufficientSignatures {
            collected,
            required,
        });
    }

    let vote = sumeragi::consensus::Vote {
        phase: certificate.phase,
        block_hash: certificate.subject_block_hash,
        parent_state_root: certificate.parent_state_root,
        post_state_root: certificate.post_state_root,
        height: certificate.height,
        view: certificate.view,
        epoch: certificate.epoch,
        chain_order_hash: certificate.chain_order_hash,
        rechain_seq: certificate.rechain_seq,
        highest_qc: None,
        signer: 0,
        bls_sig: Vec::new(),
    };
    let preimage =
        sumeragi::consensus::vote_preimage(config.expected_chain_id, &certificate.mode_tag, &vote);
    let mut public_keys: Vec<&iroha_crypto::PublicKey> = Vec::with_capacity(seen.len());
    let mut pops: Vec<&[u8]> = Vec::with_capacity(seen.len());
    for signer in &seen {
        let idx = usize::try_from(*signer).map_err(|_| {
            BridgeFinalityVerificationError::SignerOutOfBounds {
                signer: *signer,
                roster_len,
            }
        })?;
        let Some(peer) = certificate.validator_set.get(idx) else {
            return Err(BridgeFinalityVerificationError::SignerOutOfBounds {
                signer: *signer,
                roster_len,
            });
        };
        public_keys.push(peer.public_key());
        pops.push(proof.validator_set_pops[idx].as_slice());
    }
    if iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &preimage,
        &certificate.aggregate.bls_aggregate_signature,
        &public_keys,
        &pops,
    )
    .is_err()
    {
        return Err(BridgeFinalityVerificationError::AggregateSignatureInvalid);
    }

    Ok(())
}

fn sccp_block_hash_from_h256(hash: [u8; 32]) -> HashOf<BlockHeader> {
    HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(hash))
}

fn sccp_block_hash_to_h256(hash: &HashOf<BlockHeader>) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(hash.as_ref().as_ref());
    out
}

fn sccp_hash_to_h256(hash: &Hash) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(hash.as_ref());
    out
}

fn sccp_consensus_phase(phase: sumeragi::consensus::Phase) -> NexusConsensusPhaseV1 {
    match phase {
        sumeragi::consensus::Phase::Prepare => NexusConsensusPhaseV1::Prepare,
        sumeragi::consensus::Phase::Commit => NexusConsensusPhaseV1::Commit,
        sumeragi::consensus::Phase::NewView => NexusConsensusPhaseV1::NewView,
    }
}

fn sccp_qc_ref(reference: &iroha_data_model::block::consensus::QcRef) -> NexusQcRefV1 {
    NexusQcRefV1 {
        height: reference.height,
        view: reference.view,
        epoch: reference.epoch,
        subject_block_hash: sccp_block_hash_to_h256(&reference.subject_block_hash),
        phase: sccp_consensus_phase(reference.phase),
    }
}

fn sccp_qc_projection_matches_local(
    finality: &NexusBridgeFinalityProofV1,
    trusted_qc: &Qc,
) -> bool {
    let qc = &finality.commit_qc;
    qc.version == 1
        && qc.phase == NexusConsensusPhaseV1::Commit
        && trusted_qc.phase == sumeragi::consensus::Phase::Commit
        && qc.height == trusted_qc.height
        && qc.view == trusted_qc.view
        && qc.epoch == trusted_qc.epoch
        && qc.mode_tag == trusted_qc.mode_tag
        && qc.subject_block_hash == sccp_block_hash_to_h256(&trusted_qc.subject_block_hash)
        && qc.parent_state_root == sccp_hash_to_h256(&trusted_qc.parent_state_root)
        && qc.post_state_root == sccp_hash_to_h256(&trusted_qc.post_state_root)
        && qc.chain_order_hash == sccp_hash_to_h256(&trusted_qc.chain_order_hash)
        && qc.rechain_seq == trusted_qc.rechain_seq
        && qc.highest_qc == trusted_qc.highest_qc.as_ref().map(sccp_qc_ref)
        && qc.validator_set_hash_version == trusted_qc.validator_set_hash_version
        && qc.validator_public_keys
            == trusted_qc
                .validator_set
                .iter()
                .map(|peer| peer.public_key().to_string())
                .collect::<Vec<_>>()
        && qc.validator_set_pops.len() == trusted_qc.validator_set.len()
        && qc.signers_bitmap == trusted_qc.aggregate.signers_bitmap
        && qc.bls_aggregate_signature == trusted_qc.aggregate.bls_aggregate_signature
}

/// Verify an SCCP finality proof against local committed blocks and trusted commit-roster data.
///
/// This intentionally rejects proofs when the local node cannot load the committed block or
/// trusted commit QC for the referenced height.
///
/// # Errors
/// Returns a human-readable rejection reason when the SCCP proof is not anchored to local state
/// or when the trusted local QC fails full finality verification.
#[allow(clippy::too_many_lines)]
pub fn verify_sccp_finality_proof_against_local_state(
    state: &impl SccpFinalityStateReadOnly,
    finality: &NexusBridgeFinalityProofV1,
) -> Result<BridgeFinalityProof, String> {
    if !iroha_sccp::verify_nexus_bridge_finality_proof_structure(finality) {
        return Err("SCCP finality proof failed structural verification".to_owned());
    }
    if !iroha_sccp::verify_nexus_bridge_finality_proof_cryptographic(finality) {
        return Err("SCCP finality proof failed cryptographic verification".to_owned());
    }
    if finality.chain_id != state.sccp_chain_id().as_str() {
        return Err(format!(
            "SCCP finality proof chain_id mismatch: expected {}, actual {}",
            state.sccp_chain_id(),
            finality.chain_id
        ));
    }

    let height_usize: usize = finality
        .height
        .try_into()
        .map_err(|_| format!("invalid SCCP finality height {}", finality.height))?;
    let nonzero_height = NonZeroUsize::new(height_usize)
        .ok_or_else(|| format!("invalid SCCP finality height {}", finality.height))?;
    let local_block = state
        .sccp_block_by_height(nonzero_height)
        .ok_or_else(|| format!("local committed block {} not found", finality.height))?;
    let local_header = local_block.header();
    let local_hash = local_block.hash();
    let proof_hash = sccp_block_hash_from_h256(finality.block_hash);
    if proof_hash != local_hash {
        return Err(
            "SCCP finality proof block hash does not match local committed block".to_owned(),
        );
    }

    let local_header_bytes = norito::to_bytes(&local_header)
        .map_err(|err| format!("failed to encode local block header: {err}"))?;
    if local_header_bytes != finality.block_header_bytes {
        return Err(
            "SCCP finality proof block header bytes do not match local committed block".to_owned(),
        );
    }
    if local_header.sccp_commitment_root() != Some(finality.commitment_root) {
        return Err(
            "SCCP finality proof commitment root does not match local block header".to_owned(),
        );
    }

    let messages = collect_sccp_messages_from_signed_block(local_block.as_ref());
    if sccp_commitment_root_from_messages(&messages) != Some(finality.commitment_root) {
        return Err(
            "SCCP finality proof commitment root does not match local SCCP records".to_owned(),
        );
    }

    let trusted_qc = state
        .sccp_commit_qc_for_block(finality.height, local_hash)
        .ok_or_else(|| {
            format!(
                "trusted local commit QC for SCCP proof height {} is missing",
                finality.height
            )
        })?;
    if !sccp_qc_projection_matches_local(finality, &trusted_qc) {
        return Err(
            "SCCP finality QC projection does not match the trusted local commit QC".to_owned(),
        );
    }

    let proof = BridgeFinalityProof {
        height: finality.height,
        chain_id: state.sccp_chain_id().clone(),
        block_header: local_header,
        block_hash: local_hash,
        commit_qc: trusted_qc,
        validator_set_pops: finality.commit_qc.validator_set_pops.clone(),
    };
    verify_finality_proof(
        &proof,
        &FinalityProofVerificationConfig {
            expected_chain_id: state.sccp_chain_id(),
            expected_height: Some(finality.height),
            trusted_validator_set_hash: Some(proof.commit_qc.validator_set_hash),
        },
    )
    .map_err(|err| format!("trusted local finality QC failed verification: {err}"))?;
    Ok(proof)
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, num::NonZeroU64, sync::Arc};

    use iroha_crypto::{Hash, KeyPair, SignatureOf};
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        block::{BlockSignature, SignedBlock},
        isi::InstructionBox,
        nexus::DataSpaceId,
        prelude::TransactionBuilder,
        smart_contract::{CHAIN_DISCRIMINANT_MAINNET, ContractAddress},
        transaction::{
            DataTriggerSequence, Executable, ExecutionStep, IvmBytecode, IvmProved,
            SignedTransaction, TransactionEntrypoint, TransactionResultInner,
            executable::ContractInvocation,
        },
        trigger::{TimeTriggerEntrypoint, TriggerId},
    };

    use super::*;
    use crate::tx::AcceptedTransaction;

    struct EmptySccpFinalityState {
        chain_id: ChainId,
    }

    impl SccpFinalityStateReadOnly for EmptySccpFinalityState {
        fn sccp_chain_id(&self) -> &ChainId {
            &self.chain_id
        }

        fn sccp_block_by_height(&self, _height: NonZeroUsize) -> Option<Arc<SignedBlock>> {
            None
        }

        fn sccp_commit_qc_for_block(
            &self,
            _height: u64,
            _block_hash: HashOf<BlockHeader>,
        ) -> Option<Qc> {
            None
        }
    }

    fn sample_transfer_payload(nonce: u64, recipient: &[u8]) -> SccpPayloadV1 {
        SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
            version: 1,
            source_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            dest_domain: iroha_sccp::SCCP_DOMAIN_ETH,
            nonce,
            asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            asset_id_codec: 1,
            asset_id: b"xor#universal".to_vec(),
            amount: 77,
            sender_codec: 1,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: iroha_sccp::SCCP_CODEC_EVM_HEX,
            recipient: recipient.to_vec(),
            route_id_codec: 1,
            route_id: b"nexus:eth:xor".to_vec(),
        })
    }

    fn signed_transaction_with_executable(executable: Executable) -> SignedTransaction {
        let keypair = KeyPair::random();
        let chain: ChainId = "bridge-sccp-tests".parse().expect("chain id");
        let authority = AccountId::new(keypair.public_key().clone());
        TransactionBuilder::new(chain, authority)
            .with_executable(executable)
            .sign(keypair.private_key())
    }

    fn ivm_proved_with_overlay(instructions: Vec<InstructionBox>) -> Executable {
        Executable::IvmProved(IvmProved {
            bytecode: IvmBytecode::from_compiled(vec![0x01, 0x02, 0x03]),
            overlay: instructions.into(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"gas"),
        })
    }

    fn signed_block_with_transactions(
        transactions: Vec<SignedTransaction>,
        height: u64,
    ) -> SignedBlock {
        let keypair = KeyPair::random();
        let entry_hashes: Vec<_> = transactions
            .iter()
            .map(SignedTransaction::hash_as_entrypoint)
            .collect();
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let signature = BlockSignature::new(
            0,
            SignatureOf::from_hash(keypair.private_key(), header.hash()),
        );
        let mut block = SignedBlock::presigned(signature, header, transactions);
        let results =
            std::iter::repeat_with(|| TransactionResultInner::Ok(DataTriggerSequence::default()))
                .take(entry_hashes.len())
                .collect();
        block
            .set_transaction_results(Vec::new(), &entry_hashes, results)
            .expect("test block entrypoint hashes should match payload");
        block
    }

    fn signed_block_with_sccp_payloads(
        payloads: &[Vec<u8>],
        height: u64,
    ) -> (SignedBlock, Vec<SccpPayloadV1>) {
        let keypair = KeyPair::random();
        let chain: ChainId = "bridge-sccp-tests".parse().expect("chain id");
        let authority = AccountId::new(keypair.public_key().clone());
        let decoded_payloads: Vec<_> = payloads
            .iter()
            .filter_map(|payload| iroha_sccp::decode_canonical_sccp_payload_bytes(payload))
            .collect();
        let instructions: Vec<InstructionBox> = payloads
            .iter()
            .cloned()
            .map(iroha_data_model::isi::bridge::RecordSccpMessage::new)
            .map(InstructionBox::from)
            .collect();
        let tx = TransactionBuilder::new(chain, authority)
            .with_executable(ivm_proved_with_overlay(instructions))
            .sign(keypair.private_key());
        let entry_hash = tx.hash_as_entrypoint();
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let signature = BlockSignature::new(
            0,
            SignatureOf::from_hash(keypair.private_key(), header.hash()),
        );
        let mut block = SignedBlock::presigned(signature, header, vec![tx]);
        let entry_hashes = [entry_hash];
        block
            .set_transaction_results(
                Vec::new(),
                &entry_hashes,
                vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
            )
            .expect("test block entrypoint hashes should match payload");
        (block, decoded_payloads)
    }

    #[test]
    fn sccp_finality_local_state_check_rejects_unsigned_qc_before_state_lookup() {
        let chain_id: ChainId = "bridge-sccp-tests".parse().expect("chain id");
        let finality = NexusBridgeFinalityProofV1 {
            version: 1,
            chain_id: chain_id.to_string(),
            height: 7,
            block_hash: [7; 32],
            commitment_root: [8; 32],
            block_header_bytes: vec![0x42],
            commit_qc: iroha_sccp::NexusCommitQcV1 {
                version: 1,
                phase: NexusConsensusPhaseV1::Commit,
                height: 7,
                view: 0,
                epoch: 0,
                mode_tag: "iroha2-consensus::permissioned-sumeragi@v1".to_owned(),
                subject_block_hash: [7; 32],
                parent_state_root: [1; 32],
                post_state_root: [2; 32],
                chain_order_hash: [3; 32],
                rechain_seq: 0,
                highest_qc: None,
                validator_set_hash_version: 1,
                validator_public_keys: vec!["not-a-bls-key".to_owned()],
                validator_set_pops: vec![vec![1; 48]],
                signers_bitmap: vec![0b0000_0001],
                bls_aggregate_signature: vec![2; 96],
            },
        };
        assert!(iroha_sccp::verify_nexus_bridge_finality_proof_structure(
            &finality
        ));
        assert!(!iroha_sccp::verify_nexus_bridge_finality_proof_cryptographic(&finality));

        let state = EmptySccpFinalityState { chain_id };
        let err = verify_sccp_finality_proof_against_local_state(&state, &finality)
            .expect_err("unsigned SCCP finality must be rejected before local anchoring");
        assert_eq!(err, "SCCP finality proof failed cryptographic verification");
    }

    #[test]
    fn sccp_commitment_root_is_none_for_empty_messages() {
        assert_eq!(sccp_commitment_root_from_messages(&[]), None);
    }

    #[test]
    fn sccp_commitment_root_matches_direct_merkle_root() {
        let payloads = vec![
            iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
                1,
                b"0x0000000000000000000000000000000000000001",
            )),
            iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
                2,
                b"0x0000000000000000000000000000000000000002",
            )),
        ];
        let (block, _) = signed_block_with_sccp_payloads(&payloads, 1);
        let messages = collect_sccp_messages_from_signed_block(&block);
        let commitments: Vec<_> = messages
            .iter()
            .map(|message| message.commitment.clone())
            .collect();

        assert_eq!(
            sccp_commitment_root_from_messages(&messages),
            iroha_sccp::commitment_merkle_root(&commitments)
        );
    }

    #[test]
    fn collect_sccp_messages_from_empty_accepted_transactions_is_empty() {
        assert!(collect_sccp_messages_from_accepted_transactions(&[]).is_empty());
    }

    #[test]
    fn collect_sccp_messages_from_plain_instruction_executable_is_empty() {
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            12,
            b"0x0000000000000000000000000000000000000012",
        ));
        let tx = signed_transaction_with_executable(Executable::Instructions(
            vec![InstructionBox::from(
                iroha_data_model::isi::bridge::RecordSccpMessage::new(payload),
            )]
            .into(),
        ));
        let block = signed_block_with_transactions(vec![tx], 1);

        assert!(collect_sccp_messages_from_signed_block(&block).is_empty());
    }

    #[test]
    fn collect_sccp_messages_from_block_preserves_payload_order() {
        let payloads = vec![
            iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
                1,
                b"0x0000000000000000000000000000000000000001",
            )),
            iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
                2,
                b"0x0000000000000000000000000000000000000002",
            )),
        ];
        let (block, decoded_payloads) = signed_block_with_sccp_payloads(&payloads, 1);

        let messages = collect_sccp_messages_from_signed_block(&block);
        assert_eq!(
            messages
                .iter()
                .map(|message| &message.payload)
                .collect::<Vec<_>>(),
            decoded_payloads.iter().collect::<Vec<_>>()
        );

        let commitments: Vec<_> = messages
            .iter()
            .map(|message| message.commitment.clone())
            .collect();
        let root = sccp_commitment_root_from_messages(&messages).expect("commitment root");
        let proof = iroha_sccp::commitment_merkle_proof(&commitments, 1).expect("proof");
        assert_eq!(
            iroha_sccp::merkle_root_from_commitment(&messages[1].commitment, &proof),
            root
        );
    }

    #[test]
    fn collect_sccp_messages_skips_undecodable_payloads() {
        let payloads = vec![
            iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
                3,
                b"0x0000000000000000000000000000000000000003",
            )),
            vec![0xff, 0x00, 0x01],
        ];
        let (block, decoded_payloads) = signed_block_with_sccp_payloads(&payloads, 2);

        let messages = collect_sccp_messages_from_signed_block(&block);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].payload, decoded_payloads[0]);
    }

    #[test]
    fn collect_sccp_messages_skips_decodable_but_invalid_payloads() {
        let invalid = sample_transfer_payload(4, b"0x0000000000000000000000000000000000000004");
        let SccpPayloadV1::Transfer(mut invalid_transfer) = invalid else {
            panic!("sample payload should be a transfer");
        };
        invalid_transfer.amount = 0;
        let invalid_payload = SccpPayloadV1::Transfer(invalid_transfer);
        assert!(
            iroha_sccp::decode_canonical_sccp_payload_bytes(
                &iroha_sccp::canonical_sccp_payload_bytes(&invalid_payload)
            )
            .is_some()
        );
        assert!(!iroha_sccp::verify_sccp_payload_structure(&invalid_payload));
        let valid_payload =
            sample_transfer_payload(5, b"0x0000000000000000000000000000000000000005");
        let payloads = vec![
            iroha_sccp::canonical_sccp_payload_bytes(&invalid_payload),
            iroha_sccp::canonical_sccp_payload_bytes(&valid_payload),
        ];
        let (block, _) = signed_block_with_sccp_payloads(&payloads, 3);

        let messages = collect_sccp_messages_from_signed_block(&block);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].payload, valid_payload);
        assert_eq!(messages[0].instruction_index, 1);
    }

    #[test]
    fn collect_sccp_messages_from_plain_ivm_executable_is_empty() {
        let tx =
            signed_transaction_with_executable(Executable::Ivm(IvmBytecode::from_compiled(vec![
                0x01, 0x02, 0x03,
            ])));
        let block = signed_block_with_transactions(vec![tx], 3);

        assert!(collect_sccp_messages_from_signed_block(&block).is_empty());
    }

    #[test]
    fn collect_sccp_messages_from_contract_call_executable_is_empty() {
        let keypair = KeyPair::random();
        let authority = AccountId::new(keypair.public_key().clone());
        let contract_address = ContractAddress::derive(
            CHAIN_DISCRIMINANT_MAINNET,
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let tx = signed_transaction_with_executable(Executable::ContractCall(ContractInvocation {
            contract_address,
            entrypoint: "bridge".to_string(),
            payload: None,
        }));
        let block = signed_block_with_transactions(vec![tx], 4);

        assert!(collect_sccp_messages_from_signed_block(&block).is_empty());
    }

    #[test]
    fn collect_sccp_messages_preserves_instruction_indices_after_skips() {
        let payloads = vec![
            iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
                4,
                b"0x0000000000000000000000000000000000000004",
            )),
            vec![0x00, 0x01, 0xff],
            iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
                5,
                b"0x0000000000000000000000000000000000000005",
            )),
        ];
        let (block, decoded_payloads) = signed_block_with_sccp_payloads(&payloads, 3);

        let messages = collect_sccp_messages_from_signed_block(&block);
        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages
                .iter()
                .map(|message| (message.tx_index, message.instruction_index))
                .collect::<Vec<_>>(),
            vec![(0, 0), (0, 2)]
        );
        assert_eq!(messages[0].payload, decoded_payloads[0]);
        assert_eq!(messages[1].payload, decoded_payloads[1]);
    }

    #[test]
    fn collect_sccp_messages_preserves_transaction_indices_across_block() {
        let first_payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            6,
            b"0x0000000000000000000000000000000000000006",
        ));
        let second_payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            7,
            b"0x0000000000000000000000000000000000000007",
        ));
        let ignored_tx =
            signed_transaction_with_executable(Executable::Ivm(IvmBytecode::from_compiled(vec![
                0xAA,
            ])));
        let first_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                first_payload,
            )),
        ]));
        let second_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                second_payload,
            )),
        ]));
        let block = signed_block_with_transactions(vec![ignored_tx, first_tx, second_tx], 5);

        let messages = collect_sccp_messages_from_signed_block(&block);
        assert_eq!(
            messages
                .iter()
                .map(|message| (message.tx_index, message.instruction_index))
                .collect::<Vec<_>>(),
            vec![(1, 0), (2, 0)]
        );
    }

    #[test]
    fn collect_sccp_messages_from_accepted_transactions_skips_non_external_entrypoints() {
        let keypair = KeyPair::random();
        let authority = AccountId::new(keypair.public_key().clone());
        let time_entry = TimeTriggerEntrypoint {
            id: "bridge_tick".parse::<TriggerId>().expect("trigger id"),
            instructions: ExecutionStep(Vec::<InstructionBox>::new().into()),
            authority,
        };
        let internal_tx = AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(
            TransactionEntrypoint::Time(time_entry),
        ));

        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            6,
            b"0x0000000000000000000000000000000000000008",
        ));
        let external_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload.clone(),
            )),
        ]));
        let external_tx = AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(
            TransactionEntrypoint::External(external_tx),
        ));

        let messages =
            collect_sccp_messages_from_accepted_transactions(&[internal_tx, external_tx]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].tx_index, 1);
        assert_eq!(messages[0].instruction_index, 0);
        assert_eq!(
            messages[0].payload,
            iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
        );
    }

    #[test]
    fn collect_sccp_messages_from_ivm_proved_overlay() {
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            7,
            b"0x0000000000000000000000000000000000000111",
        ));
        let executable = Executable::IvmProved(IvmProved {
            bytecode: IvmBytecode::from_compiled(vec![0x01, 0x02, 0x03]),
            overlay: vec![InstructionBox::from(
                iroha_data_model::isi::bridge::RecordSccpMessage::new(payload.clone()),
            )]
            .into(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"gas"),
        });
        let tx = signed_transaction_with_executable(executable);
        let block = signed_block_with_transactions(vec![tx], 4);

        let messages = collect_sccp_messages_from_signed_block(&block);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].tx_index, 0);
        assert_eq!(messages[0].instruction_index, 0);
        assert_eq!(
            messages[0].payload,
            iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
        );
    }
}
