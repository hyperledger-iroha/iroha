//! Contains message structures for p2p communication during consensus.
use std::{io::Write, sync::Arc};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::{BlockHeader, BlockSignature, SignedBlock, consensus::SumeragiMembershipStatus},
    peer::PeerId,
};
use iroha_logger::prelude::*;
use iroha_macro::*;
use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::{Decode, Encode},
    core as ncore,
};

use crate::block::NewBlock;

#[allow(clippy::enum_variant_names, clippy::large_enum_variant)]
/// Messages used by peers to communicate during the consensus process.
#[derive(Debug, Clone, Decode, Encode, FromVariant)]
pub enum BlockMessage {
    /// This message is sent by leader to all validating peers, when a new block is created.
    BlockCreated(#[skip_try_from] BlockCreated),
    /// This message is sent by `BlockSync` when a new block is received.
    BlockSyncUpdate(#[skip_try_from] BlockSyncUpdate),
    /// Exact frontier body request keyed by `(height, view, block_hash)`.
    FetchBlockBody(#[skip_try_from] FetchBlockBody),
    /// Exact frontier body response carrying the requested body.
    BlockBodyResponse(#[skip_try_from] BlockBodyResponse),
    /// Direct certified block request/response keyed by a commit QC subject.
    CertifiedBlockFetch(#[skip_try_from] CertifiedBlockFetch),
    /// Broadcast periodically or at startup to pin consensus parameters across peers.
    ///
    /// Nodes verify that their local on-chain collector parameters match advertised values.
    /// A mismatch is logged and flagged locally; consensus rules remain unchanged.
    ConsensusParams(#[skip_try_from] ConsensusParamsAdvert),
    /// VRF commit (`NPoS` randomness).
    VrfCommit(#[skip_try_from] super::consensus::VrfCommit),
    /// VRF reveal (`NPoS` randomness).
    VrfReveal(#[skip_try_from] super::consensus::VrfReveal),
    /// Execution witness with metadata for SMT recomputation.
    ExecWitness(#[skip_try_from] super::consensus::ExecWitnessMsg),
    /// RBC INIT repair request.
    RbcInitRequest(#[skip_try_from] super::consensus::RbcInitRequest),
    /// RBC chunk repair request.
    RbcChunkRequest(#[skip_try_from] super::consensus::RbcChunkRequest),
    /// RBC init (payload distribution scaffold).
    RbcInit(#[skip_try_from] super::consensus::RbcInit),
    /// RBC payload chunk.
    RbcChunk(#[skip_try_from] super::consensus::RbcChunk),
    /// RBC payload chunk with compact height/view/epoch headers.
    RbcChunkCompact(#[skip_try_from] RbcChunkCompact),
    /// RBC READY signal.
    RbcReady(#[skip_try_from] super::consensus::RbcReady),
    /// RBC DELIVER notification.
    RbcDeliver(#[skip_try_from] super::consensus::RbcDeliver),
    /// Request a pending (not-yet-committed) block payload by hash.
    FetchPendingBlock(#[skip_try_from] FetchPendingBlock),
    /// Advertisement that a peer durably retains a canonical committed block body.
    KuraReplicaAdvert(#[skip_try_from] KuraReplicaAdvert),
    /// Proposal hint: minimal header carrying `HighestQC` reference for the proposal.
    ProposalHint(#[skip_try_from] ProposalHint),
    /// Full proposal header + payload hash. Used for on-wire parent/HighestQC checks.
    Proposal(#[skip_try_from] super::consensus::Proposal),
    /// Commit vote (Prepare/Commit/NewView) carrying a BLS signature.
    QcVote(#[skip_try_from] super::consensus::QcVote),
    /// Commit certificate (Prepare/Commit/NewView) aggregating BLS signatures.
    Qc(#[skip_try_from] super::consensus::Qc),
}

impl BlockMessage {
    /// Local no-op sentinel used only when an infallible legacy decode/encode path must return a
    /// valid message after a wire-codec failure.
    pub(super) fn invalid_wire_sentinel() -> Self {
        Self::ConsensusParams(ConsensusParamsAdvert::invalid_wire_sentinel())
    }

    /// Normalize compact message variants into their full forms.
    pub fn normalize(self) -> Self {
        match self {
            Self::RbcChunkCompact(chunk) => Self::RbcChunk(chunk.into_chunk()),
            other => other,
        }
    }

    /// Build an RBC chunk message, using the compact variant when fields fit.
    pub fn from_rbc_chunk(chunk: super::consensus::RbcChunk) -> Self {
        let super::consensus::RbcChunk {
            block_hash,
            height,
            view,
            epoch,
            idx,
            bytes,
        } = chunk;
        let Ok(height_u32) = u32::try_from(height) else {
            return Self::RbcChunk(super::consensus::RbcChunk {
                block_hash,
                height,
                view,
                epoch,
                idx,
                bytes,
            });
        };
        let Ok(view_u32) = u32::try_from(view) else {
            return Self::RbcChunk(super::consensus::RbcChunk {
                block_hash,
                height,
                view,
                epoch,
                idx,
                bytes,
            });
        };
        let Ok(epoch_u32) = u32::try_from(epoch) else {
            return Self::RbcChunk(super::consensus::RbcChunk {
                block_hash,
                height,
                view,
                epoch,
                idx,
                bytes,
            });
        };
        Self::RbcChunkCompact(RbcChunkCompact {
            block_hash,
            height: height_u32,
            view: view_u32,
            epoch: epoch_u32,
            idx,
            bytes,
        })
    }

    /// Network priority for this consensus message.
    ///
    /// RBC chunks are required for deliver quorum; deprioritising them stalls consensus.
    pub fn priority(&self) -> iroha_p2p::Priority {
        iroha_p2p::Priority::High
    }
}

impl<'a> ncore::DecodeFromSlice<'a> for BlockMessage {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let mut cursor = bytes;
        let value = Decode::decode(&mut cursor)?;
        let consumed = bytes.len().saturating_sub(cursor.len());
        Ok((value, consumed))
    }
}

/// Wire wrapper for consensus payloads.
///
/// Cached bytes always store a full Norito-framed [`BlockMessage`] so the payload remains
/// self-describing even when it is forwarded through other framed envelopes.
#[derive(Debug, Clone)]
pub struct BlockMessageWire {
    message: Arc<BlockMessage>,
    encoded: Option<Arc<Vec<u8>>>,
}

impl BlockMessageWire {
    /// Wrap a consensus message without cached bytes.
    pub fn new(message: BlockMessage) -> Self {
        Self {
            message: Arc::new(message),
            encoded: None,
        }
    }

    /// Wrap an `Arc`-backed message with cached full-frame bytes.
    pub fn with_encoded(message: Arc<BlockMessage>, encoded: Arc<Vec<u8>>) -> Self {
        Self {
            message,
            encoded: Some(encoded),
        }
    }

    /// Wrap an owned message with cached full-frame bytes.
    pub fn with_encoded_owned(message: BlockMessage, encoded: Arc<Vec<u8>>) -> Self {
        Self {
            message: Arc::new(message),
            encoded: Some(encoded),
        }
    }

    /// Borrow the underlying consensus message.
    pub fn as_message(&self) -> &BlockMessage {
        self.message.as_ref()
    }

    /// Acquire a mutable reference, clearing cached encoded bytes.
    pub fn make_mut(&mut self) -> &mut BlockMessage {
        self.encoded = None;
        Arc::make_mut(&mut self.message)
    }

    /// Consume the wrapper and return the consensus message.
    pub fn into_message(self) -> BlockMessage {
        let message = self.message;
        if Arc::strong_count(&message) == 1 {
            return match Arc::into_inner(message) {
                Some(message) => message,
                None => BlockMessage::invalid_wire_sentinel(),
            };
        }
        (*message).clone()
    }

    /// Cached encoded length if available.
    pub fn encoded_len(&self) -> Option<usize> {
        self.encoded.as_ref().map(|bytes| bytes.len())
    }

    fn framed_prefix_len(bytes: &[u8]) -> Result<usize, ncore::Error> {
        const LEN_OFF: usize = 4 + 1 + 1 + 16 + 1;

        if bytes.len() < ncore::Header::SIZE {
            return Err(ncore::Error::LengthMismatch);
        }
        if bytes[..4] != ncore::MAGIC {
            return Err(ncore::Error::InvalidMagic);
        }
        if bytes.get(4) != Some(&ncore::VERSION_MAJOR) {
            return Err(ncore::Error::UnsupportedVersion {
                found: bytes[4],
                expected: ncore::VERSION_MAJOR,
            });
        }
        if bytes.get(5) != Some(&ncore::VERSION_MINOR) {
            return Err(ncore::Error::UnsupportedMinorVersion {
                found: bytes[5],
                supported: ncore::VERSION_MINOR,
            });
        }
        let schema = bytes.get(6..22).ok_or(ncore::Error::LengthMismatch)?;
        if schema != <BlockMessage as NoritoSerialize>::schema_hash().as_slice() {
            return Err(ncore::Error::SchemaMismatch);
        }
        let compression = *bytes.get(22).ok_or(ncore::Error::LengthMismatch)?;
        if compression != ncore::Compression::None as u8 {
            return Err(ncore::Error::unsupported_compression_with(
                compression,
                &[ncore::Compression::None],
            ));
        }
        let len_bytes = bytes
            .get(LEN_OFF..LEN_OFF + 8)
            .ok_or(ncore::Error::LengthMismatch)?;
        let mut length = [0u8; 8];
        length.copy_from_slice(len_bytes);
        let payload_len = usize::try_from(u64::from_le_bytes(length))
            .map_err(|_| ncore::Error::LengthMismatch)?;
        let align = core::mem::align_of::<ncore::Archived<BlockMessage>>();
        let padding = if align <= 1 {
            0
        } else {
            let rem = ncore::Header::SIZE % align;
            if rem == 0 { 0 } else { align - rem }
        };
        ncore::Header::SIZE
            .checked_add(padding)
            .and_then(|size| size.checked_add(payload_len))
            .filter(|size| *size <= bytes.len())
            .ok_or(ncore::Error::LengthMismatch)
    }

    pub(crate) fn try_encode_message(message: &BlockMessage) -> Result<Vec<u8>, ncore::Error> {
        ncore::to_bytes(message)
    }

    pub(crate) fn encode_message(message: &BlockMessage) -> Vec<u8> {
        match Self::try_encode_message(message) {
            Ok(bytes) => bytes,
            Err(error) => {
                error!(
                    %error,
                    "failed to pre-encode Sumeragi block message; substituting invalid-wire sentinel"
                );
                Self::encode_invalid_wire_sentinel()
            }
        }
    }

    fn encode_invalid_wire_sentinel() -> Vec<u8> {
        match ncore::to_bytes(&BlockMessage::invalid_wire_sentinel()) {
            Ok(bytes) => bytes,
            Err(error) => {
                error!(
                    %error,
                    "failed to encode Sumeragi invalid-wire sentinel"
                );
                Vec::new()
            }
        }
    }
}

impl AsRef<BlockMessage> for BlockMessageWire {
    fn as_ref(&self) -> &BlockMessage {
        self.message.as_ref()
    }
}

impl std::ops::Deref for BlockMessageWire {
    type Target = BlockMessage;

    fn deref(&self) -> &Self::Target {
        self.message.as_ref()
    }
}

impl From<BlockMessage> for BlockMessageWire {
    fn from(message: BlockMessage) -> Self {
        Self::new(message)
    }
}

impl NoritoSerialize for BlockMessageWire {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), ncore::Error> {
        if let Some(encoded) = self.encoded.as_ref() {
            writer.write_all(encoded)?;
            return Ok(());
        }
        let encoded = Self::try_encode_message(self.message.as_ref())?;
        writer.write_all(&encoded)?;
        Ok(())
    }
}

impl<'a> NoritoDeserialize<'a> for BlockMessageWire {
    fn deserialize(archived: &'a ncore::Archived<Self>) -> Self {
        match Self::try_deserialize(archived) {
            Ok(wire) => wire,
            Err(error) => {
                error!(
                    %error,
                    "failed to decode Sumeragi block message through infallible Norito path"
                );
                Self::new(BlockMessage::invalid_wire_sentinel())
            }
        }
    }

    fn try_deserialize(archived: &'a ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let bytes = ncore::payload_slice_from_ptr(ptr)?;
        let view = ncore::from_bytes_view(bytes)?;
        let message = view.decode::<BlockMessage>()?;
        let encoded = Arc::new(bytes.to_vec());
        Ok(Self {
            message: Arc::new(message),
            encoded: Some(encoded),
        })
    }
}

impl<'a> ncore::DecodeFromSlice<'a> for BlockMessageWire {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let consumed = Self::framed_prefix_len(bytes)?;
        let framed = bytes.get(..consumed).ok_or(ncore::Error::LengthMismatch)?;
        let message = ncore::decode_from_bytes::<BlockMessage>(framed)?;
        let encoded = Arc::new(framed.to_vec());
        Ok((
            Self {
                message: Arc::new(message),
                encoded: Some(encoded),
            },
            consumed,
        ))
    }
}

/// Compact RBC payload chunk header (u32 height/view/epoch).
#[derive(Debug, Clone, Decode, Encode)]
pub struct RbcChunkCompact {
    /// Subject block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Height (u32-compact).
    pub height: u32,
    /// View (u32-compact).
    pub view: u32,
    /// Epoch (u32-compact).
    pub epoch: u32,
    /// Chunk index (0-based).
    pub idx: u32,
    /// Chunk bytes.
    pub bytes: Vec<u8>,
}

impl RbcChunkCompact {
    /// Build a compact chunk when headers fit into u32.
    pub fn try_from_chunk(chunk: &super::consensus::RbcChunk) -> Option<Self> {
        let height = u32::try_from(chunk.height).ok()?;
        let view = u32::try_from(chunk.view).ok()?;
        let epoch = u32::try_from(chunk.epoch).ok()?;
        Some(Self {
            block_hash: chunk.block_hash,
            height,
            view,
            epoch,
            idx: chunk.idx,
            bytes: chunk.bytes.clone(),
        })
    }

    /// Convert into the full `RbcChunk` form.
    pub fn into_chunk(self) -> super::consensus::RbcChunk {
        super::consensus::RbcChunk {
            block_hash: self.block_hash,
            height: u64::from(self.height),
            view: u64::from(self.view),
            epoch: u64::from(self.epoch),
            idx: self.idx,
            bytes: self.bytes,
        }
    }
}

/// Control-flow signals exchanged between peers (pacemaker frames).
#[derive(Debug, Clone, Decode, Encode, FromVariant)]
pub enum ControlFlow {
    /// Evidence propagation for slashing/governance actions.
    Evidence(super::consensus::Evidence),
}

/// Minimal proposal header hint broadcast alongside `BlockCreated` by the leader.
/// Carries a `HighestQC` header reference for pacemaker consumers.
#[derive(Debug, Clone, Copy, Decode, Encode)]
pub struct ProposalHint {
    /// Proposed block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Proposed block height.
    pub height: u64,
    /// View for which the proposal applies.
    pub view: u64,
    /// Highest certificate reference known to the proposer.
    pub highest_qc: super::consensus::QcRef,
}

// Bridge Norito codec (Encode/Decode) to core slice-based decoding for strict-safe paths.
impl<'a> norito::core::DecodeFromSlice<'a> for ControlFlow {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut s: &'a [u8] = bytes;
        let value = <Self as norito::codec::DecodeAll>::decode_all(&mut s)
            .map_err(|e| norito::core::Error::Message(format!("codec decode error: {e}")))?;
        let used = bytes.len() - s.len();
        Ok((value, used))
    }
}

// NOTE: slice-based decode for ControlFlow is validated indirectly via
// other consensus tests; no dedicated unit test here to avoid duplication.

/// Compact advertisement of consensus parameters which must be identical across peers.
#[derive(Debug, Clone, Copy, Decode, Encode)]
pub struct ConsensusParamsAdvert {
    /// Number of collectors targeted per height (K). Stored as u16 for compactness.
    pub collectors_k: u16,
    /// Redundant send fanout (r).
    pub redundant_send_r: u8,
    /// Optional membership hash snapshot for the active `(height, view, epoch)`.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub membership: Option<SumeragiMembershipStatus>,
}

impl ConsensusParamsAdvert {
    /// Local no-op sentinel for invalid infallible wire fallback paths.
    pub(super) const fn invalid_wire_sentinel() -> Self {
        Self {
            collectors_k: 0,
            redundant_send_r: 0,
            membership: None,
        }
    }

    /// Whether this advert is the local invalid-wire sentinel.
    pub(super) fn is_invalid_wire_sentinel(&self) -> bool {
        self.collectors_k == 0 && self.redundant_send_r == 0 && self.membership.is_none()
    }
}

/// `BlockCreated` message structure.
#[derive(Debug, Clone, Decode, Encode)]
pub struct BlockCreated {
    /// The corresponding block.
    pub block: SignedBlock,
    /// Optional frontier metadata carried inline so `BlockCreated` can initialize the active slot
    /// without a separate proposal or RBC INIT side message.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub frontier: Option<BlockCreatedFrontierInfo>,
}

/// Consensus metadata bundled into `BlockCreated` for frontier progression.
#[derive(Debug, Clone, Decode, Encode)]
pub struct BlockCreatedFrontierInfo {
    /// Highest QC/lock reference known to the leader when the block was created.
    pub highest_qc: super::consensus::QcRef,
    /// Hash of the canonical block payload bytes.
    pub payload_hash: Hash,
    /// Proposer index within the validator set.
    pub proposer: super::consensus::ValidatorIndex,
    /// Epoch associated with this slot.
    pub epoch: u64,
    /// Hash of the roster snapshot used for vote validation and body transport checks.
    pub roster_hash: Hash,
    /// Total chunk count for the body transport manifest.
    pub total_chunks: u32,
    /// SHA-256 digest for each payload chunk.
    pub chunk_digests: Vec<[u8; 32]>,
    /// Merkle root for the chunk digest set.
    pub chunk_root: Hash,
    /// Leader signature over the block header.
    pub leader_signature: BlockSignature,
}

impl From<&NewBlock> for BlockCreated {
    fn from(block: &NewBlock) -> Self {
        let mut signed = SignedBlock::presigned_with_da(
            block.signature().clone(),
            block.header(),
            block
                .transactions()
                .iter()
                .map(|accepted| accepted.as_ref().clone())
                .collect(),
            block.da_commitments().cloned(),
        );
        signed.set_da_proof_policies(block.da_proof_policies().cloned());
        signed.set_da_pin_intents(block.da_pin_intents().cloned());
        signed.set_previous_roster_evidence(block.previous_roster_evidence().cloned());
        Self {
            block: signed,
            frontier: None,
        }
    }
}

impl From<NewBlock> for BlockCreated {
    fn from(block: NewBlock) -> Self {
        Self {
            block: block.into(),
            frontier: None,
        }
    }
}

impl From<&SignedBlock> for BlockCreated {
    fn from(block: &SignedBlock) -> Self {
        Self {
            // Clone is required to own the message payload when constructed from a borrowed block.
            block: block.clone(),
            frontier: None,
        }
    }
}

impl BlockCreated {
    /// Build a frontier-complete `BlockCreated`.
    pub fn with_frontier(block: SignedBlock, frontier: BlockCreatedFrontierInfo) -> Self {
        Self {
            block,
            frontier: Some(frontier),
        }
    }
}

impl BlockCreatedFrontierInfo {
    /// Build inline frontier metadata from the proposal/RBC-init information for the slot.
    pub fn from_proposal_and_rbc_init(
        proposal: &super::consensus::Proposal,
        init: &super::consensus::RbcInit,
    ) -> Self {
        Self {
            highest_qc: proposal.header.highest_qc,
            payload_hash: proposal.payload_hash,
            proposer: proposal.header.proposer,
            epoch: proposal.header.epoch,
            roster_hash: init.roster_hash,
            total_chunks: init.total_chunks,
            chunk_digests: init.chunk_digests.clone(),
            chunk_root: init.chunk_root,
            leader_signature: init.leader_signature.clone(),
        }
    }
}

/// `BlockSyncUpdate` message structure.
#[derive(Debug, Clone, Decode, Encode)]
pub struct BlockSyncUpdate {
    /// The corresponding block.
    pub block: SignedBlock,
    /// Cached commit votes for the block (used to backfill missing votes on peers).
    pub commit_votes: Vec<super::consensus::QcVote>,
    /// Optional commit certificate associated with the block height.
    pub commit_qc: Option<iroha_data_model::consensus::Qc>,
    /// Optional validator checkpoint associated with the block height.
    pub validator_checkpoint: Option<iroha_data_model::consensus::ValidatorSetCheckpoint>,
    /// Optional stake snapshot aligned to the validator set.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub stake_snapshot: Option<super::stake_snapshot::CommitStakeSnapshot>,
}

/// Direct certified block fetch message family.
#[derive(Debug, Clone, Decode, Encode, FromVariant)]
pub enum CertifiedBlockFetch {
    /// Request the exact block body certified by a known commit QC.
    Request(#[skip_try_from] CertifiedBlockFetchRequest),
    /// Response carrying the certified block and commit proof.
    Response(#[skip_try_from] CertifiedBlockFetchResponse),
    /// Commit proof companion used when the full certified response exceeds the frame cap.
    Proof(#[skip_try_from] CertifiedBlockFetchProof),
    /// Block body companion used when the full certified response exceeds the frame cap.
    Body(#[skip_try_from] CertifiedBlockFetchBody),
}

/// Request the exact certified block for a known commit QC subject.
#[derive(Debug, Clone, Decode, Encode)]
pub struct CertifiedBlockFetchRequest {
    /// Peer requesting the certified block.
    pub requester: PeerId,
    /// Height certified by the commit QC.
    pub height: u64,
    /// View certified by the commit QC.
    pub view: u64,
    /// Hash of the certified block.
    pub block_hash: HashOf<BlockHeader>,
}

/// Response carrying a block and the commit QC certifying it.
#[derive(Debug, Clone, Decode, Encode)]
pub struct CertifiedBlockFetchResponse {
    /// Height certified by the commit QC.
    pub height: u64,
    /// View certified by the commit QC.
    pub view: u64,
    /// Full block body for the certified subject.
    pub block: SignedBlock,
    /// Commit QC that certifies `block`.
    pub commit_qc: iroha_data_model::consensus::Qc,
    /// Validator checkpoint aligned to `commit_qc`.
    pub validator_checkpoint: iroha_data_model::consensus::ValidatorSetCheckpoint,
    /// Optional stake snapshot aligned to the validator set.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub stake_snapshot: Option<super::stake_snapshot::CommitStakeSnapshot>,
}

/// Proof companion for an exact certified block fetch response.
#[derive(Debug, Clone, Decode, Encode)]
pub struct CertifiedBlockFetchProof {
    /// Height certified by the commit QC.
    pub height: u64,
    /// View certified by the commit QC.
    pub view: u64,
    /// Hash of the certified block.
    pub block_hash: HashOf<BlockHeader>,
    /// Commit QC that certifies `block_hash`.
    pub commit_qc: iroha_data_model::consensus::Qc,
    /// Validator checkpoint aligned to `commit_qc`.
    pub validator_checkpoint: iroha_data_model::consensus::ValidatorSetCheckpoint,
    /// Optional stake snapshot aligned to the validator set.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub stake_snapshot: Option<super::stake_snapshot::CommitStakeSnapshot>,
}

/// Body companion for an exact certified block fetch response.
#[derive(Debug, Clone, Decode, Encode)]
pub struct CertifiedBlockFetchBody {
    /// Height of the certified block.
    pub height: u64,
    /// View of the certified block.
    pub view: u64,
    /// Full block body for the certified subject.
    pub block: SignedBlock,
}

/// Validation error for a malformed certified block response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertifiedBlockFetchValidationError {
    /// Response height does not match the block header height.
    HeightMismatch,
    /// Response view does not match the block header view.
    ViewMismatch,
    /// Commit QC subject does not match the returned block hash.
    BlockHashMismatch,
    /// Commit QC does not target the response height.
    QcHeightMismatch,
    /// Commit QC does not target the response view.
    QcViewMismatch,
    /// The response did not carry a usable commit certificate.
    Uncertified,
    /// Validator checkpoint metadata does not match the commit QC.
    CheckpointMismatch,
}

impl CertifiedBlockFetchResponse {
    /// Validate that the response self-certifies the returned block.
    pub fn validate_subject(&self) -> Result<(), CertifiedBlockFetchValidationError> {
        let header = self.block.header();
        if header.height().get() != self.height {
            return Err(CertifiedBlockFetchValidationError::HeightMismatch);
        }
        if header.view_change_index() != self.view {
            return Err(CertifiedBlockFetchValidationError::ViewMismatch);
        }
        validate_certified_fetch_proof_parts(
            self.height,
            self.view,
            self.block.hash(),
            &self.commit_qc,
            &self.validator_checkpoint,
        )
    }
}

impl CertifiedBlockFetchProof {
    /// Validate that the proof self-certifies the carried block hash.
    pub fn validate_subject(&self) -> Result<(), CertifiedBlockFetchValidationError> {
        validate_certified_fetch_proof_parts(
            self.height,
            self.view,
            self.block_hash,
            &self.commit_qc,
            &self.validator_checkpoint,
        )
    }
}

impl CertifiedBlockFetchBody {
    /// Validate that the body matches the carried height/view.
    pub fn validate_subject(&self) -> Result<(), CertifiedBlockFetchValidationError> {
        let header = self.block.header();
        if header.height().get() != self.height {
            return Err(CertifiedBlockFetchValidationError::HeightMismatch);
        }
        if header.view_change_index() != self.view {
            return Err(CertifiedBlockFetchValidationError::ViewMismatch);
        }
        Ok(())
    }
}

fn validate_certified_fetch_proof_parts(
    height: u64,
    view: u64,
    block_hash: HashOf<BlockHeader>,
    commit_qc: &iroha_data_model::consensus::Qc,
    validator_checkpoint: &iroha_data_model::consensus::ValidatorSetCheckpoint,
) -> Result<(), CertifiedBlockFetchValidationError> {
    if !matches!(commit_qc.phase, super::consensus::Phase::Commit)
        || commit_qc.aggregate.signers_bitmap.is_empty()
        || commit_qc.aggregate.bls_aggregate_signature.is_empty()
    {
        return Err(CertifiedBlockFetchValidationError::Uncertified);
    }
    if commit_qc.subject_block_hash != block_hash {
        return Err(CertifiedBlockFetchValidationError::BlockHashMismatch);
    }
    if commit_qc.height != height {
        return Err(CertifiedBlockFetchValidationError::QcHeightMismatch);
    }
    if commit_qc.view != view {
        return Err(CertifiedBlockFetchValidationError::QcViewMismatch);
    }
    if validator_checkpoint.height != height
        || validator_checkpoint.view != view
        || validator_checkpoint.block_hash != commit_qc.subject_block_hash
        || validator_checkpoint.chain_order_hash != commit_qc.chain_order_hash
        || validator_checkpoint.rechain_seq != commit_qc.rechain_seq
        || validator_checkpoint.parent_state_root != commit_qc.parent_state_root
        || validator_checkpoint.post_state_root != commit_qc.post_state_root
        || validator_checkpoint.validator_set != commit_qc.validator_set
        || validator_checkpoint.signers_bitmap != commit_qc.aggregate.signers_bitmap
        || validator_checkpoint.bls_aggregate_signature
            != commit_qc.aggregate.bls_aggregate_signature
        || validator_checkpoint.validator_set_hash != commit_qc.validator_set_hash
        || validator_checkpoint.validator_set_hash_version != commit_qc.validator_set_hash_version
    {
        return Err(CertifiedBlockFetchValidationError::CheckpointMismatch);
    }
    Ok(())
}

impl From<&SignedBlock> for BlockSyncUpdate {
    fn from(block: &SignedBlock) -> Self {
        Self {
            block: block.clone(),
            commit_votes: Vec::new(),
            commit_qc: None,
            validator_checkpoint: None,
            stake_snapshot: None,
        }
    }
}

/// Request an exact frontier block body for a known `(height, view, block_hash)` slot.
#[derive(Debug, Clone, Decode, Encode)]
pub struct FetchBlockBody {
    /// Peer requesting the body.
    pub requester: PeerId,
    /// Hash of the requested block body.
    pub block_hash: HashOf<BlockHeader>,
    /// Height hint for the requested body.
    pub height: u64,
    /// View hint for the requested body.
    pub view: u64,
}

/// Exact block-body payload carried in a `BlockBodyResponse`.
#[derive(Debug, Clone, Decode, Encode, FromVariant)]
pub enum BlockBodyData {
    /// Full authoritative body delivered as a `BlockCreated` payload.
    BlockCreated(#[skip_try_from] BlockCreated),
    /// Full authoritative body delivered as a `BlockSyncUpdate` payload with optional commit
    /// sidecars so lagging peers can recover committed frontier blocks without reproposing them.
    BlockSyncUpdate(#[skip_try_from] BlockSyncUpdate),
}

/// Exact frontier block-body response keyed by `(height, view, block_hash)`.
#[derive(Debug, Clone, Decode, Encode)]
pub struct BlockBodyResponse {
    /// Hash of the requested block body.
    pub block_hash: HashOf<BlockHeader>,
    /// Height of the requested block body.
    pub height: u64,
    /// View of the requested block body.
    pub view: u64,
    /// The returned authoritative body payload.
    pub body: BlockBodyData,
}

// NOTE: Previously manual decoding validated signature uniqueness; Decode is now derived for simplicity.

/// Request a peer to resend a pending block payload.
#[derive(Debug, Clone, Copy, Decode, Encode, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FetchPendingBlockPriority {
    /// Background fetch (default).
    Background,
    /// Consensus-critical fetch (highest QC).
    Consensus,
}

/// Request a peer to resend a pending block payload.
#[derive(Debug, Clone, Decode, Encode)]
pub struct FetchPendingBlock {
    /// Peer requesting the payload.
    pub requester: PeerId,
    /// Hash of the missing block.
    pub block_hash: HashOf<BlockHeader>,
    /// Height hint for the missing block.
    pub height: u64,
    /// View hint for the missing block.
    pub view: u64,
    /// Optional priority hint for responders.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub priority: Option<FetchPendingBlockPriority>,
    /// Optional signal that requester already has verifiable roster proof for this block round.
    ///
    /// Responders may use this to allow hintless block-sync payload recovery paths that otherwise
    /// require roster hints.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub requester_roster_proof_known: Option<bool>,
    /// Optional signal that requester already has the block payload and needs only the commit QC.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub commit_qc_only: Option<bool>,
}

/// Peer-local durable replica advertisement for canonical Kura block bodies.
#[derive(Debug, Clone, Copy, Decode, Encode)]
pub struct KuraReplicaAdvert {
    /// Height of the advertised canonical block.
    pub height: u64,
    /// Hash of the advertised canonical block.
    pub block_hash: HashOf<BlockHeader>,
    /// Canonical framed block-body length retained by the peer.
    pub payload_len: u64,
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, sync::Arc, time::Duration};

    use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
    use iroha_data_model::{
        AccountId, ChainId, Level,
        consensus::{
            PreviousRosterEvidence, VALIDATOR_SET_HASH_VERSION_V1, ValidatorSetCheckpoint,
        },
        da::{
            commitment::{DaCommitmentBundle, DaCommitmentRecord, DaProofScheme, KzgCommitment},
            types::{BlobDigest, RetentionPolicy, StorageTicketId},
        },
        isi::Log,
        nexus::LaneId,
        sorafs::pin_registry::ManifestDigest,
        transaction::TransactionBuilder,
    };
    use norito::{core as norito_core, decode_from_bytes};

    use super::*;
    use crate::{block::BlockBuilder, sumeragi::consensus, tx::AcceptedTransaction};

    fn dummy_accepted_transaction() -> AcceptedTransaction<'static> {
        let chain_id: ChainId = "00000000-0000-0000-0000-000000000000"
            .parse()
            .expect("valid chain id");
        let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let authority = AccountId::new(keypair.public_key().clone());
        let mut builder = TransactionBuilder::new(chain_id, authority);
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_instructions([Log::new(Level::INFO, "dummy".to_owned())])
            .sign(keypair.private_key());
        AcceptedTransaction::new_unchecked(Cow::Owned(tx))
    }

    fn sample_qc_vote(seed: u8) -> consensus::QcVote {
        consensus::QcVote {
            phase: consensus::Phase::Commit,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [seed; Hash::LENGTH],
            )),
            parent_state_root: Hash::prehashed([seed.wrapping_add(1); Hash::LENGTH]),
            post_state_root: Hash::prehashed([seed.wrapping_add(2); Hash::LENGTH]),
            height: u64::from(seed).saturating_add(1),
            view: u64::from(seed % 4),
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: vec![seed, seed.wrapping_add(1)],
        }
    }

    fn sample_qc(seed: u8) -> consensus::Qc {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive sample QC validator key");
        let validator = PeerId::from(key_pair.public_key().clone());
        let validator_set = vec![validator];
        consensus::Qc {
            phase: consensus::Phase::Commit,
            subject_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [seed.wrapping_add(3); Hash::LENGTH],
            )),
            parent_state_root: Hash::prehashed([seed.wrapping_add(4); Hash::LENGTH]),
            post_state_root: Hash::prehashed([seed.wrapping_add(5); Hash::LENGTH]),
            height: u64::from(seed).saturating_add(2),
            view: u64::from(seed % 3),
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: consensus::PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: consensus::QcAggregate {
                signers_bitmap: vec![1],
                bls_aggregate_signature: vec![seed.wrapping_add(6), seed.wrapping_add(7)],
            },
        }
    }

    fn sample_certified_block_fetch_response(seed: u8) -> CertifiedBlockFetchResponse {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive sample certified block key");
        let new_block = BlockBuilder::new(vec![dummy_accepted_transaction()])
            .chain(0, None)
            .sign(key_pair.private_key())
            .unpack(|_| {});
        let block = BlockCreated::from(&new_block).block;
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let mut commit_qc = sample_qc(seed.wrapping_add(1));
        commit_qc.subject_block_hash = block.hash();
        commit_qc.height = height;
        commit_qc.view = view;
        let validator_checkpoint = ValidatorSetCheckpoint::new_with_chain_order(
            commit_qc.height,
            commit_qc.view,
            commit_qc.subject_block_hash,
            commit_qc.chain_order_hash,
            commit_qc.rechain_seq,
            commit_qc.parent_state_root,
            commit_qc.post_state_root,
            commit_qc.validator_set.clone(),
            commit_qc.aggregate.signers_bitmap.clone(),
            commit_qc.aggregate.bls_aggregate_signature.clone(),
            commit_qc.validator_set_hash_version,
            None,
        );
        CertifiedBlockFetchResponse {
            height,
            view,
            block,
            commit_qc,
            validator_checkpoint,
            stake_snapshot: None,
        }
    }

    fn sample_certified_block_fetch_proof(seed: u8) -> CertifiedBlockFetchProof {
        let response = sample_certified_block_fetch_response(seed);
        CertifiedBlockFetchProof {
            height: response.height,
            view: response.view,
            block_hash: response.block.hash(),
            commit_qc: response.commit_qc,
            validator_checkpoint: response.validator_checkpoint,
            stake_snapshot: response.stake_snapshot,
        }
    }

    fn sample_certified_block_fetch_body(seed: u8) -> CertifiedBlockFetchBody {
        let response = sample_certified_block_fetch_response(seed);
        CertifiedBlockFetchBody {
            height: response.height,
            view: response.view,
            block: response.block,
        }
    }

    fn sample_exec_witness_msg(seed: u8) -> consensus::ExecWitnessMsg {
        consensus::ExecWitnessMsg {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [seed.wrapping_add(8); Hash::LENGTH],
            )),
            height: u64::from(seed).saturating_add(3),
            view: u64::from(seed % 2),
            epoch: 0,
            witness: consensus::ExecWitness::default(),
        }
    }

    fn sample_rbc_init_request(seed: u8) -> consensus::RbcInitRequest {
        consensus::RbcInitRequest {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [seed.wrapping_add(9); Hash::LENGTH],
            )),
            height: u64::from(seed).saturating_add(4),
            view: u64::from(seed % 5),
        }
    }

    fn sample_rbc_chunk_request(seed: u8) -> consensus::RbcChunkRequest {
        consensus::RbcChunkRequest {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [seed.wrapping_add(10); Hash::LENGTH],
            )),
            height: u64::from(seed).saturating_add(5),
            view: u64::from(seed % 6),
            missing_indices: vec![1, 4, 9],
        }
    }

    fn sample_rbc_chunk(
        seed: u8,
        height: u64,
        view: u64,
        epoch: u64,
        idx: u32,
        bytes: Vec<u8>,
    ) -> consensus::RbcChunk {
        consensus::RbcChunk {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [seed; Hash::LENGTH],
            )),
            height,
            view,
            epoch,
            idx,
            bytes,
        }
    }

    fn assert_compact_matches_chunk(compact: &RbcChunkCompact, chunk: &consensus::RbcChunk) {
        assert_eq!(compact.block_hash, chunk.block_hash);
        assert_eq!(u64::from(compact.height), chunk.height);
        assert_eq!(u64::from(compact.view), chunk.view);
        assert_eq!(u64::from(compact.epoch), chunk.epoch);
        assert_eq!(compact.idx, chunk.idx);
        assert_eq!(compact.bytes, chunk.bytes);
    }

    fn assert_invalid_wire_sentinel(message: &BlockMessage) {
        match message {
            BlockMessage::ConsensusParams(advert) => assert!(advert.is_invalid_wire_sentinel()),
            other => panic!("expected invalid-wire sentinel, got {other:?}"),
        }
    }

    fn roundtrip_cached_block_message_over_network_message(
        message: BlockMessage,
    ) -> crate::NetworkMessage {
        let encoded = Arc::new(BlockMessageWire::encode_message(&message));
        let wire = BlockMessageWire::with_encoded(Arc::new(message), encoded);
        let network = crate::NetworkMessage::SumeragiBlock(Box::new(wire));
        let bytes = network.encode();
        Decode::decode(&mut bytes.as_slice()).expect("decode network message")
    }

    #[test]
    fn block_created_from_newblock_ref_and_move_equivalent() {
        // Build a minimal NewBlock and sign it.
        let kp = KeyPair::try_from_seed(b"seed-seed".to_vec(), Algorithm::Ed25519)
            .expect("derive block-created fixture key");
        let da_bundle = DaCommitmentBundle::new(vec![DaCommitmentRecord::new(
            LaneId::new(1),
            2,
            3,
            BlobDigest::new([0x11; 32]),
            ManifestDigest::new([0x22; 32]),
            DaProofScheme::MerkleSha256,
            Hash::prehashed([0x33; 32]),
            Some(KzgCommitment::new([0x44; 48])),
            Some(Hash::prehashed([0x55; 32])),
            RetentionPolicy::default(),
            StorageTicketId::new([0x66; 32]),
            Signature::from_bytes(&[0x77; 64]),
        )]);
        let new_block = BlockBuilder::new(vec![dummy_accepted_transaction()])
            .chain(0, None)
            .with_da_commitments(Some(da_bundle.clone()))
            .sign(kp.private_key())
            .unpack(|_| {});

        let msg_from_ref = BlockCreated::from(&new_block);
        let msg_from_move = BlockCreated::from(new_block.clone());

        assert_eq!(msg_from_ref.block.header(), msg_from_move.block.header());
        assert_eq!(msg_from_ref.block.hash(), msg_from_move.block.hash());
        assert_eq!(msg_from_ref.block.da_commitments(), Some(&da_bundle));
        assert_eq!(msg_from_move.block.da_commitments(), Some(&da_bundle));
    }

    #[test]
    fn block_created_from_newblock_ref_preserves_previous_roster_evidence() {
        let kp = KeyPair::try_from_seed(b"seed-seed".to_vec(), Algorithm::Ed25519)
            .expect("derive previous-roster fixture key");
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x42; 32]));
        let parent_state_root = Hash::prehashed([0x12; 32]);
        let post_state_root = Hash::prehashed([0x34; 32]);
        let validator = PeerId::from(kp.public_key().clone());
        let checkpoint = ValidatorSetCheckpoint::new(
            1,
            0,
            block_hash,
            parent_state_root,
            post_state_root,
            vec![validator],
            vec![1],
            vec![2],
            VALIDATOR_SET_HASH_VERSION_V1,
            None,
        );
        let evidence = PreviousRosterEvidence {
            height: 1,
            block_hash,
            validator_checkpoint: checkpoint,
            stake_snapshot: None,
        };

        let new_block = BlockBuilder::new(vec![dummy_accepted_transaction()])
            .chain(0, None)
            .with_previous_roster_evidence(Some(evidence.clone()))
            .sign(kp.private_key())
            .unpack(|_| {});

        let msg = BlockCreated::from(&new_block);
        assert_eq!(
            msg.block.previous_roster_evidence(),
            Some(&evidence),
            "BlockCreated built from &NewBlock must preserve roster evidence payload",
        );
        assert_eq!(
            msg.block.header().prev_roster_evidence_hash(),
            Some(HashOf::new(&evidence)),
            "payload and header evidence hash must stay aligned",
        );
    }

    #[test]
    fn block_created_frontier_wire_constructors_match_formal_gate() {
        let kp = KeyPair::try_from_seed(b"frontier-wire".to_vec(), Algorithm::Ed25519)
            .expect("derive frontier-wire fixture key");
        let new_block = BlockBuilder::new(vec![dummy_accepted_transaction()])
            .chain(0, None)
            .sign(kp.private_key())
            .unpack(|_| {});

        let from_borrowed_new_block = BlockCreated::from(&new_block);
        let from_owned_new_block = BlockCreated::from(new_block.clone());

        assert!(
            from_borrowed_new_block.frontier.is_none(),
            "plain borrowed NewBlock constructors must not fabricate frontier metadata",
        );
        assert!(
            from_owned_new_block.frontier.is_none(),
            "plain owned NewBlock constructors must not fabricate frontier metadata",
        );
        assert_eq!(
            from_borrowed_new_block.block.hash(),
            from_owned_new_block.block.hash(),
            "borrowed and owned NewBlock constructors must preserve the same block",
        );

        let signed_block = from_borrowed_new_block.block.clone();
        let from_signed_block = BlockCreated::from(&signed_block);
        assert!(
            from_signed_block.frontier.is_none(),
            "plain SignedBlock constructors must not fabricate frontier metadata",
        );
        assert_eq!(
            from_signed_block.block.hash(),
            signed_block.hash(),
            "SignedBlock constructor must preserve the block payload",
        );

        let block_hash = signed_block.hash();
        let parent_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x21; Hash::LENGTH]));
        let highest_qc = consensus::QcRef {
            height: 7,
            view: 3,
            epoch: 2,
            subject_block_hash: parent_hash,
            phase: consensus::Phase::Prepare,
        };
        let proposal = consensus::Proposal {
            header: consensus::ConsensusBlockHeader {
                parent_hash,
                tx_root: Hash::prehashed([0x22; Hash::LENGTH]),
                state_root: Hash::prehashed([0x23; Hash::LENGTH]),
                proposer: 2,
                height: signed_block.header().height().get(),
                view: signed_block.header().view_change_index(),
                epoch: 9,
                highest_qc,
            },
            payload_hash: Hash::prehashed([0x24; Hash::LENGTH]),
        };
        let leader_signature = signed_block
            .signatures()
            .next()
            .cloned()
            .expect("sample block should carry a leader signature");
        let rbc_init = consensus::RbcInit {
            block_hash,
            height: proposal.header.height,
            view: proposal.header.view,
            epoch: proposal.header.epoch,
            roster: vec![PeerId::from(kp.public_key().clone())],
            roster_hash: Hash::prehashed([0x25; Hash::LENGTH]),
            total_chunks: 2,
            encoding: iroha_data_model::block::consensus::RbcEncoding::Plain,
            chunk_size_bytes: 16,
            payload_size_bytes: 31,
            data_shards: 0,
            parity_shards: 0,
            chunk_digests: vec![[0x26; 32], [0x27; 32]],
            payload_hash: Hash::prehashed([0x28; Hash::LENGTH]),
            chunk_root: Hash::prehashed([0x29; Hash::LENGTH]),
            block_header: signed_block.header(),
            leader_signature: leader_signature.clone(),
        };

        let frontier = BlockCreatedFrontierInfo::from_proposal_and_rbc_init(&proposal, &rbc_init);
        assert_eq!(frontier.highest_qc, proposal.header.highest_qc);
        assert_eq!(frontier.payload_hash, proposal.payload_hash);
        assert_eq!(frontier.proposer, proposal.header.proposer);
        assert_eq!(frontier.epoch, proposal.header.epoch);
        assert_eq!(frontier.roster_hash, rbc_init.roster_hash);
        assert_eq!(frontier.total_chunks, rbc_init.total_chunks);
        assert_eq!(frontier.chunk_digests, rbc_init.chunk_digests);
        assert_eq!(frontier.chunk_root, rbc_init.chunk_root);
        assert_eq!(frontier.leader_signature, leader_signature);

        let with_frontier = BlockCreated::with_frontier(signed_block.clone(), frontier.clone());
        let preserved_frontier = with_frontier
            .frontier
            .as_ref()
            .expect("with_frontier must preserve supplied metadata");
        assert_eq!(
            with_frontier.block.hash(),
            signed_block.hash(),
            "with_frontier must preserve the supplied block",
        );
        assert_eq!(preserved_frontier.highest_qc, frontier.highest_qc);
        assert_eq!(preserved_frontier.payload_hash, frontier.payload_hash);
        assert_eq!(preserved_frontier.proposer, frontier.proposer);
        assert_eq!(preserved_frontier.epoch, frontier.epoch);
        assert_eq!(preserved_frontier.roster_hash, frontier.roster_hash);
        assert_eq!(preserved_frontier.total_chunks, frontier.total_chunks);
        assert_eq!(preserved_frontier.chunk_digests, frontier.chunk_digests);
        assert_eq!(preserved_frontier.chunk_root, frontier.chunk_root);
        assert_eq!(
            preserved_frontier.leader_signature,
            frontier.leader_signature,
        );
    }

    #[test]
    fn rbc_repair_requests_roundtrip_over_network_wrapper() {
        let init_request = BlockMessage::RbcInitRequest(sample_rbc_init_request(7));
        let chunk_request = BlockMessage::RbcChunkRequest(sample_rbc_chunk_request(11));

        let init_roundtrip = roundtrip_cached_block_message_over_network_message(init_request);
        let chunk_roundtrip = roundtrip_cached_block_message_over_network_message(chunk_request);

        assert!(matches!(
            init_roundtrip,
            crate::NetworkMessage::SumeragiBlock(wire)
                if matches!(wire.as_message(), BlockMessage::RbcInitRequest(_))
        ));
        assert!(matches!(
            chunk_roundtrip,
            crate::NetworkMessage::SumeragiBlock(wire)
                if matches!(wire.as_message(), BlockMessage::RbcChunkRequest(_))
        ));
    }

    #[test]
    fn control_flow_evidence_roundtrip() {
        use super::super::consensus;
        // Construct minimal double-vote evidence
        let dummy_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([1u8; 32]));
        let v1 = consensus::Vote {
            phase: consensus::Phase::Prepare,
            block_hash: dummy_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 1,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let v2 = consensus::Vote {
            phase: consensus::Phase::Prepare,
            block_hash: dummy_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 1,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let ev = consensus::Evidence {
            kind: consensus::EvidenceKind::DoublePrepare,
            payload: consensus::EvidencePayload::DoubleVote { v1, v2 },
        };
        let cf = ControlFlow::Evidence(ev);
        let bytes = cf.encode();
        // Only check that encoding succeeds and yields non-empty bytes.
        assert!(!bytes.is_empty());
    }

    #[test]
    fn block_message_priority_marks_rbc_chunk_high() {
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([2u8; 32]));
        let chunk = sample_rbc_chunk(2, 1, 0, 0, 0, vec![0u8; 1]);
        let msg = BlockMessage::RbcChunk(chunk.clone());
        assert_eq!(msg.priority(), iroha_p2p::Priority::High);

        let compact = RbcChunkCompact::try_from_chunk(&chunk).expect("chunk headers fit in u32");
        assert_eq!(
            BlockMessage::RbcChunkCompact(compact).priority(),
            iroha_p2p::Priority::High
        );

        let requester = PeerId::from(KeyPair::random().public_key().clone());
        let fetch_body = BlockMessage::FetchBlockBody(FetchBlockBody {
            requester: requester.clone(),
            block_hash,
            height: 1,
            view: 0,
        });
        assert_eq!(fetch_body.priority(), iroha_p2p::Priority::High);

        let fetch = BlockMessage::FetchPendingBlock(FetchPendingBlock {
            requester,
            block_hash,
            height: 1,
            view: 0,
            priority: None,
            requester_roster_proof_known: None,
            commit_qc_only: None,
        });
        assert_eq!(fetch.priority(), iroha_p2p::Priority::High);
    }

    #[test]
    fn block_message_priority_marks_all_variants_high_match_formal_gate() {
        let response = sample_certified_block_fetch_response(0x90);
        let block = response.block.clone();
        let block_hash = block.hash();
        let requester = PeerId::from(
            KeyPair::try_from_seed(vec![0x91; 32], Algorithm::Ed25519)
                .expect("derive block-message priority requester key")
                .public_key()
                .clone(),
        );
        let leader_signature = block
            .signatures()
            .next()
            .expect("sample block has a leader signature")
            .clone();
        let highest_qc = consensus::QcRef {
            height: 3,
            view: 1,
            epoch: 0,
            subject_block_hash: block_hash,
            phase: consensus::Phase::Commit,
        };
        let proposal = consensus::Proposal {
            header: consensus::ConsensusBlockHeader {
                parent_hash: block_hash,
                tx_root: Hash::prehashed([0xA0; Hash::LENGTH]),
                state_root: Hash::prehashed([0xA1; Hash::LENGTH]),
                proposer: 0,
                height: 4,
                view: 2,
                epoch: 0,
                highest_qc,
            },
            payload_hash: Hash::prehashed([0xA2; Hash::LENGTH]),
        };
        let compact_chunk =
            RbcChunkCompact::try_from_chunk(&sample_rbc_chunk(0x92, 4, 2, 0, 1, vec![0xFA, 0xFB]))
                .expect("compact sample fits in u32");
        let roster_hash = Hash::prehashed([0x93; Hash::LENGTH]);
        let chunk_root = Hash::prehashed([0x94; Hash::LENGTH]);
        let rbc_ready = consensus::RbcReady {
            block_hash,
            height: 4,
            view: 2,
            epoch: 0,
            roster_hash,
            chunk_root,
            sender: 0,
            signature: vec![0x95],
        };
        let rbc_deliver = consensus::RbcDeliver {
            block_hash,
            height: 4,
            view: 2,
            epoch: 0,
            roster_hash,
            chunk_root,
            sender: 1,
            signature: vec![0x96],
            ready_signatures: vec![consensus::RbcReadySignature {
                sender: 0,
                signature: vec![0x97],
            }],
        };
        let rbc_init = consensus::RbcInit {
            block_hash,
            height: 4,
            view: 2,
            epoch: 0,
            roster: response.validator_checkpoint.validator_set.clone(),
            roster_hash,
            total_chunks: 1,
            encoding: iroha_data_model::block::consensus::RbcEncoding::Plain,
            chunk_size_bytes: 2,
            payload_size_bytes: 2,
            data_shards: 0,
            parity_shards: 0,
            chunk_digests: vec![[0x98; 32]],
            payload_hash: Hash::prehashed([0x99; Hash::LENGTH]),
            chunk_root,
            block_header: block.header(),
            leader_signature,
        };
        let fetch_request = CertifiedBlockFetchRequest {
            requester: requester.clone(),
            height: response.height,
            view: response.view,
            block_hash,
        };
        let fetch_proof = sample_certified_block_fetch_proof(0x9A);
        let fetch_body = sample_certified_block_fetch_body(0x9B);

        let messages = vec![
            (
                "BlockCreated",
                BlockMessage::BlockCreated(BlockCreated::from(&block)),
            ),
            (
                "BlockSyncUpdate",
                BlockMessage::BlockSyncUpdate(BlockSyncUpdate::from(&block)),
            ),
            (
                "FetchBlockBody",
                BlockMessage::FetchBlockBody(FetchBlockBody {
                    requester: requester.clone(),
                    block_hash,
                    height: 4,
                    view: 2,
                }),
            ),
            (
                "BlockBodyResponse",
                BlockMessage::BlockBodyResponse(BlockBodyResponse {
                    block_hash,
                    height: 4,
                    view: 2,
                    body: BlockBodyData::BlockCreated(BlockCreated::from(&block)),
                }),
            ),
            (
                "CertifiedBlockFetch::Request",
                BlockMessage::CertifiedBlockFetch(CertifiedBlockFetch::Request(fetch_request)),
            ),
            (
                "CertifiedBlockFetch::Response",
                BlockMessage::CertifiedBlockFetch(CertifiedBlockFetch::Response(response.clone())),
            ),
            (
                "CertifiedBlockFetch::Proof",
                BlockMessage::CertifiedBlockFetch(CertifiedBlockFetch::Proof(fetch_proof)),
            ),
            (
                "CertifiedBlockFetch::Body",
                BlockMessage::CertifiedBlockFetch(CertifiedBlockFetch::Body(fetch_body)),
            ),
            (
                "ConsensusParams",
                BlockMessage::ConsensusParams(ConsensusParamsAdvert {
                    collectors_k: 2,
                    redundant_send_r: 1,
                    membership: None,
                }),
            ),
            (
                "VrfCommit",
                BlockMessage::VrfCommit(consensus::VrfCommit {
                    epoch: 0,
                    commitment: [0x9C; 32],
                    signer: 0,
                    bls_sig: vec![0x9D],
                }),
            ),
            (
                "VrfReveal",
                BlockMessage::VrfReveal(consensus::VrfReveal {
                    epoch: 0,
                    reveal: [0x9E; 32],
                    signer: 0,
                    bls_sig: vec![0x9F],
                }),
            ),
            (
                "ExecWitness",
                BlockMessage::ExecWitness(sample_exec_witness_msg(0xA0)),
            ),
            (
                "RbcInitRequest",
                BlockMessage::RbcInitRequest(sample_rbc_init_request(0xA1)),
            ),
            (
                "RbcChunkRequest",
                BlockMessage::RbcChunkRequest(sample_rbc_chunk_request(0xA2)),
            ),
            ("RbcInit", BlockMessage::RbcInit(rbc_init)),
            (
                "RbcChunk",
                BlockMessage::RbcChunk(sample_rbc_chunk(0xA3, 4, 2, 0, 1, vec![0xA4])),
            ),
            (
                "RbcChunkCompact",
                BlockMessage::RbcChunkCompact(compact_chunk),
            ),
            ("RbcReady", BlockMessage::RbcReady(rbc_ready)),
            ("RbcDeliver", BlockMessage::RbcDeliver(rbc_deliver)),
            (
                "FetchPendingBlock",
                BlockMessage::FetchPendingBlock(FetchPendingBlock {
                    requester: requester.clone(),
                    block_hash,
                    height: 4,
                    view: 2,
                    priority: Some(FetchPendingBlockPriority::Consensus),
                    requester_roster_proof_known: Some(true),
                    commit_qc_only: Some(false),
                }),
            ),
            (
                "KuraReplicaAdvert",
                BlockMessage::KuraReplicaAdvert(KuraReplicaAdvert {
                    height: 4,
                    block_hash,
                    payload_len: 128,
                }),
            ),
            (
                "ProposalHint",
                BlockMessage::ProposalHint(ProposalHint {
                    block_hash,
                    height: 4,
                    view: 2,
                    highest_qc,
                }),
            ),
            ("Proposal", BlockMessage::Proposal(proposal)),
            ("QcVote", BlockMessage::QcVote(sample_qc_vote(0xA5))),
            ("Qc", BlockMessage::Qc(sample_qc(0xA6))),
        ];

        for (variant, message) in messages {
            assert_eq!(
                message.priority(),
                iroha_p2p::Priority::High,
                "{variant} priority changed"
            );
        }
    }

    #[test]
    fn certified_block_fetch_response_accepts_matching_block_qc_and_checkpoint() {
        let response = sample_certified_block_fetch_response(21);

        assert_eq!(response.validate_subject(), Ok(()));
    }

    #[test]
    fn certified_block_fetch_proof_accepts_matching_block_qc_and_checkpoint() {
        let proof = sample_certified_block_fetch_proof(28);

        assert_eq!(proof.validate_subject(), Ok(()));
    }

    #[test]
    fn certified_block_fetch_body_accepts_matching_height_and_view() {
        let body = sample_certified_block_fetch_body(29);

        assert_eq!(body.validate_subject(), Ok(()));
    }

    #[test]
    fn certified_block_fetch_response_rejects_mismatched_hash() {
        let mut response = sample_certified_block_fetch_response(22);
        response.commit_qc.subject_block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x44; Hash::LENGTH]));

        assert_eq!(
            response.validate_subject(),
            Err(CertifiedBlockFetchValidationError::BlockHashMismatch)
        );
    }

    #[test]
    fn certified_block_fetch_response_rejects_mismatched_height_and_view() {
        let mut height_mismatch = sample_certified_block_fetch_response(23);
        height_mismatch.height = height_mismatch.height.saturating_add(1);
        assert_eq!(
            height_mismatch.validate_subject(),
            Err(CertifiedBlockFetchValidationError::HeightMismatch)
        );

        let mut view_mismatch = sample_certified_block_fetch_response(24);
        view_mismatch.view = view_mismatch.view.saturating_add(1);
        assert_eq!(
            view_mismatch.validate_subject(),
            Err(CertifiedBlockFetchValidationError::ViewMismatch)
        );

        let mut qc_view_mismatch = sample_certified_block_fetch_response(25);
        qc_view_mismatch.commit_qc.view = qc_view_mismatch.commit_qc.view.saturating_add(1);
        assert_eq!(
            qc_view_mismatch.validate_subject(),
            Err(CertifiedBlockFetchValidationError::QcViewMismatch)
        );

        let mut qc_height_mismatch = sample_certified_block_fetch_response(38);
        qc_height_mismatch.commit_qc.height = qc_height_mismatch.commit_qc.height.saturating_add(1);
        assert_eq!(
            qc_height_mismatch.validate_subject(),
            Err(CertifiedBlockFetchValidationError::QcHeightMismatch)
        );
    }

    #[test]
    fn certified_block_fetch_response_rejects_uncertified_response() {
        let mut response = sample_certified_block_fetch_response(26);
        response.commit_qc.phase = consensus::Phase::Prepare;

        assert_eq!(
            response.validate_subject(),
            Err(CertifiedBlockFetchValidationError::Uncertified)
        );
    }

    #[test]
    fn certified_block_fetch_response_rejects_empty_certificate_parts() {
        let mut empty_bitmap = sample_certified_block_fetch_response(32);
        empty_bitmap.commit_qc.aggregate.signers_bitmap.clear();
        assert_eq!(
            empty_bitmap.validate_subject(),
            Err(CertifiedBlockFetchValidationError::Uncertified)
        );

        let mut empty_signature = sample_certified_block_fetch_response(33);
        empty_signature
            .commit_qc
            .aggregate
            .bls_aggregate_signature
            .clear();
        assert_eq!(
            empty_signature.validate_subject(),
            Err(CertifiedBlockFetchValidationError::Uncertified)
        );
    }

    #[test]
    fn certified_block_fetch_proof_rejects_checkpoint_mismatch() {
        let mut proof = sample_certified_block_fetch_proof(34);
        proof.validator_checkpoint.signers_bitmap.push(0xff);

        assert_eq!(
            proof.validate_subject(),
            Err(CertifiedBlockFetchValidationError::CheckpointMismatch)
        );
    }

    #[test]
    fn certified_block_fetch_proof_rejects_mismatched_or_uncertified_qc() {
        let mut block_hash_mismatch = sample_certified_block_fetch_proof(39);
        block_hash_mismatch.commit_qc.subject_block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x39; Hash::LENGTH]));
        assert_eq!(
            block_hash_mismatch.validate_subject(),
            Err(CertifiedBlockFetchValidationError::BlockHashMismatch)
        );

        let mut height_mismatch = sample_certified_block_fetch_proof(40);
        height_mismatch.commit_qc.height = height_mismatch.commit_qc.height.saturating_add(1);
        assert_eq!(
            height_mismatch.validate_subject(),
            Err(CertifiedBlockFetchValidationError::QcHeightMismatch)
        );

        let mut view_mismatch = sample_certified_block_fetch_proof(41);
        view_mismatch.commit_qc.view = view_mismatch.commit_qc.view.saturating_add(1);
        assert_eq!(
            view_mismatch.validate_subject(),
            Err(CertifiedBlockFetchValidationError::QcViewMismatch)
        );

        let mut uncertified = sample_certified_block_fetch_proof(42);
        uncertified.commit_qc.phase = consensus::Phase::Prepare;
        assert_eq!(
            uncertified.validate_subject(),
            Err(CertifiedBlockFetchValidationError::Uncertified)
        );
    }

    #[test]
    fn certified_block_fetch_response_rejects_checkpoint_mismatch() {
        let mut response = sample_certified_block_fetch_response(37);
        response.validator_checkpoint.validator_set_hash_version = response
            .validator_checkpoint
            .validator_set_hash_version
            .saturating_add(1);

        assert_eq!(
            response.validate_subject(),
            Err(CertifiedBlockFetchValidationError::CheckpointMismatch)
        );
    }

    #[test]
    fn certified_block_fetch_rejects_checkpoint_root_and_roster_mutations() {
        let mut chain_order = sample_certified_block_fetch_response(43);
        chain_order.validator_checkpoint.chain_order_hash = Hash::prehashed([0x43; Hash::LENGTH]);
        assert_eq!(
            chain_order.validate_subject(),
            Err(CertifiedBlockFetchValidationError::CheckpointMismatch)
        );

        let mut state_root = sample_certified_block_fetch_proof(44);
        state_root.validator_checkpoint.post_state_root = Hash::prehashed([0x44; Hash::LENGTH]);
        assert_eq!(
            state_root.validate_subject(),
            Err(CertifiedBlockFetchValidationError::CheckpointMismatch)
        );

        let mut roster = sample_certified_block_fetch_response(45);
        let extra = KeyPair::try_from_seed(vec![45; 32], Algorithm::Ed25519)
            .expect("derive mutated roster fixture key");
        roster
            .validator_checkpoint
            .validator_set
            .push(PeerId::from(extra.public_key().clone()));
        assert_eq!(
            roster.validate_subject(),
            Err(CertifiedBlockFetchValidationError::CheckpointMismatch)
        );
    }

    #[test]
    fn certified_block_fetch_rejects_checkpoint_signature_and_hash_mutations() {
        let mut parent_root = sample_certified_block_fetch_response(46);
        parent_root.validator_checkpoint.parent_state_root = Hash::prehashed([0x46; Hash::LENGTH]);
        assert_eq!(
            parent_root.validate_subject(),
            Err(CertifiedBlockFetchValidationError::CheckpointMismatch)
        );

        let mut rechain_seq = sample_certified_block_fetch_proof(47);
        rechain_seq.validator_checkpoint.rechain_seq = rechain_seq
            .validator_checkpoint
            .rechain_seq
            .saturating_add(1);
        assert_eq!(
            rechain_seq.validate_subject(),
            Err(CertifiedBlockFetchValidationError::CheckpointMismatch)
        );

        let mut aggregate_signature = sample_certified_block_fetch_response(48);
        aggregate_signature
            .validator_checkpoint
            .bls_aggregate_signature
            .push(0xff);
        assert_eq!(
            aggregate_signature.validate_subject(),
            Err(CertifiedBlockFetchValidationError::CheckpointMismatch)
        );

        let mut validator_set_hash = sample_certified_block_fetch_proof(49);
        validator_set_hash.validator_checkpoint.validator_set_hash =
            HashOf::from_untyped_unchecked(Hash::prehashed([0x49; Hash::LENGTH]));
        assert_eq!(
            validator_set_hash.validate_subject(),
            Err(CertifiedBlockFetchValidationError::CheckpointMismatch)
        );
    }

    #[test]
    fn certified_block_fetch_body_rejects_mismatched_height_and_view() {
        let mut height_mismatch = sample_certified_block_fetch_body(35);
        height_mismatch.height = height_mismatch.height.saturating_add(1);
        assert_eq!(
            height_mismatch.validate_subject(),
            Err(CertifiedBlockFetchValidationError::HeightMismatch)
        );

        let mut view_mismatch = sample_certified_block_fetch_body(36);
        view_mismatch.view = view_mismatch.view.saturating_add(1);
        assert_eq!(
            view_mismatch.validate_subject(),
            Err(CertifiedBlockFetchValidationError::ViewMismatch)
        );
    }

    #[test]
    fn certified_block_fetch_roundtrips_over_network_wrapper() {
        let requester = PeerId::from(KeyPair::random().public_key().clone());
        let response = sample_certified_block_fetch_response(27);
        let request = BlockMessage::CertifiedBlockFetch(CertifiedBlockFetch::Request(
            CertifiedBlockFetchRequest {
                requester,
                height: response.height,
                view: response.view,
                block_hash: response.block.hash(),
            },
        ));
        let response = BlockMessage::CertifiedBlockFetch(CertifiedBlockFetch::Response(response));
        let proof = BlockMessage::CertifiedBlockFetch(CertifiedBlockFetch::Proof(
            sample_certified_block_fetch_proof(30),
        ));
        let body = BlockMessage::CertifiedBlockFetch(CertifiedBlockFetch::Body(
            sample_certified_block_fetch_body(31),
        ));

        let request_roundtrip = roundtrip_cached_block_message_over_network_message(request);
        let response_roundtrip = roundtrip_cached_block_message_over_network_message(response);
        let proof_roundtrip = roundtrip_cached_block_message_over_network_message(proof);
        let body_roundtrip = roundtrip_cached_block_message_over_network_message(body);

        assert!(matches!(
            request_roundtrip,
            crate::NetworkMessage::SumeragiBlock(wire)
                if matches!(
                    wire.as_message(),
                    BlockMessage::CertifiedBlockFetch(CertifiedBlockFetch::Request(_))
                )
        ));
        assert!(matches!(
            response_roundtrip,
            crate::NetworkMessage::SumeragiBlock(wire)
                if matches!(
                    wire.as_message(),
                    BlockMessage::CertifiedBlockFetch(CertifiedBlockFetch::Response(_))
                )
        ));
        assert!(matches!(
            proof_roundtrip,
            crate::NetworkMessage::SumeragiBlock(wire)
                if matches!(
                    wire.as_message(),
                    BlockMessage::CertifiedBlockFetch(CertifiedBlockFetch::Proof(_))
                )
        ));
        assert!(matches!(
            body_roundtrip,
            crate::NetworkMessage::SumeragiBlock(wire)
                if matches!(
                    wire.as_message(),
                    BlockMessage::CertifiedBlockFetch(CertifiedBlockFetch::Body(_))
                )
        ));
    }

    #[test]
    fn block_message_wire_prefers_preencoded_payload() {
        let advert = ConsensusParamsAdvert {
            collectors_k: 1,
            redundant_send_r: 1,
            membership: None,
        };
        let msg = BlockMessage::ConsensusParams(advert);
        let encoded = BlockMessageWire::encode_message(&msg);
        let wire = BlockMessageWire::with_encoded(Arc::new(msg), Arc::new(encoded.clone()));

        assert!(encoded.starts_with(&norito_core::MAGIC));
        assert_eq!(wire.encoded_len(), Some(encoded.len()));
        let bytes = wire.encode();
        assert_eq!(bytes, encoded);
        let decoded: BlockMessageWire =
            Decode::decode(&mut bytes.as_slice()).expect("decode block message wire");
        assert!(matches!(decoded.as_ref(), BlockMessage::ConsensusParams(_)));
        assert_eq!(decoded.encoded_len(), Some(encoded.len()));
        assert_eq!(decoded.encode(), encoded);
    }

    #[test]
    fn block_message_wire_roundtrip_with_cached_payload() {
        let advert = ConsensusParamsAdvert {
            collectors_k: 2,
            redundant_send_r: 3,
            membership: None,
        };
        let msg = BlockMessage::ConsensusParams(advert);
        let encoded = BlockMessageWire::encode_message(&msg);
        let wire = BlockMessageWire::with_encoded(Arc::new(msg), Arc::new(encoded));

        let bytes = wire.encode();
        let decoded: BlockMessageWire =
            Decode::decode(&mut bytes.as_slice()).expect("decode block message wire");

        match decoded.as_ref() {
            BlockMessage::ConsensusParams(decoded_advert) => {
                assert_eq!(decoded_advert.collectors_k, 2);
                assert_eq!(decoded_advert.redundant_send_r, 3);
                assert!(decoded_advert.membership.is_none());
            }
            other => panic!("expected consensus params, got {other:?}"),
        }
        assert_eq!(decoded.encoded_len(), Some(bytes.len()));
        assert_eq!(decoded.encode(), bytes);
    }

    #[test]
    fn block_message_wire_cached_payload_is_self_describing() {
        let vote = sample_qc_vote(0x41);
        let msg = BlockMessage::QcVote(vote.clone());
        let encoded = BlockMessageWire::encode_message(&msg);

        assert!(encoded.starts_with(&norito_core::MAGIC));

        let decoded = decode_from_bytes::<BlockMessage>(&encoded).expect("decode inner message");
        match decoded {
            BlockMessage::QcVote(decoded_vote) => assert_eq!(decoded_vote, vote),
            other => panic!("expected qc vote, got {other:?}"),
        }
    }

    #[test]
    fn invalid_wire_sentinel_is_identified_and_self_describing() {
        let advert = ConsensusParamsAdvert::invalid_wire_sentinel();
        assert!(advert.is_invalid_wire_sentinel());
        let msg = BlockMessage::invalid_wire_sentinel();
        assert_invalid_wire_sentinel(&msg);

        let encoded =
            BlockMessageWire::try_encode_message(&msg).expect("encode invalid-wire sentinel");
        assert_eq!(BlockMessageWire::encode_message(&msg), encoded);

        let decoded = decode_from_bytes::<BlockMessage>(&encoded).expect("decode sentinel frame");
        assert_invalid_wire_sentinel(&decoded);
    }

    #[test]
    fn block_message_wire_into_message_clones_shared_arc() {
        let msg = Arc::new(BlockMessage::invalid_wire_sentinel());
        let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
        let wire = BlockMessageWire::with_encoded(Arc::clone(&msg), encoded);

        let message = wire.into_message();

        assert_invalid_wire_sentinel(&message);
        assert_eq!(Arc::strong_count(&msg), 1);
    }

    #[test]
    fn block_message_wire_matches_formal_gate() {
        fn consensus_params(collectors_k: u16, redundant_send_r: u8) -> BlockMessage {
            BlockMessage::ConsensusParams(ConsensusParamsAdvert {
                collectors_k,
                redundant_send_r,
                membership: None,
            })
        }

        fn assert_consensus_params(
            label: &str,
            message: &BlockMessage,
            collectors_k: u16,
            redundant_send_r: u8,
        ) {
            match message {
                BlockMessage::ConsensusParams(advert) => {
                    assert_eq!(advert.collectors_k, collectors_k, "{label} collectors_k");
                    assert_eq!(
                        advert.redundant_send_r, redundant_send_r,
                        "{label} redundant_send_r"
                    );
                }
                other => panic!("{label}: expected consensus params, got {other:?}"),
            }
        }

        fn assert_rejects_frame(label: &str, bytes: Vec<u8>) {
            assert!(
                <BlockMessageWire as norito_core::DecodeFromSlice>::decode_from_slice(&bytes)
                    .is_err(),
                "{label} frame should be rejected"
            );
        }

        const LEN_OFF: usize = 4 + 1 + 1 + 16 + 1;

        let wrapped = consensus_params(1, 2);
        let alternate = consensus_params(3, 4);
        let wrapped_encoded = BlockMessageWire::encode_message(&wrapped);
        let alternate_encoded = BlockMessageWire::encode_message(&alternate);

        assert!(wrapped_encoded.starts_with(&norito_core::MAGIC));
        assert_eq!(wrapped_encoded[4], norito_core::VERSION_MAJOR);
        assert_eq!(wrapped_encoded[5], norito_core::VERSION_MINOR);
        assert_eq!(
            &wrapped_encoded[6..22],
            <BlockMessage as NoritoSerialize>::schema_hash().as_slice()
        );
        assert_eq!(wrapped_encoded[22], norito_core::Compression::None as u8);
        assert!(LEN_OFF + 8 <= norito_core::Header::SIZE);

        let uncached = BlockMessageWire::new(wrapped.clone());
        assert_eq!(uncached.encoded_len(), None);
        assert_eq!(uncached.encode(), wrapped_encoded);
        assert_eq!(
            uncached.encoded_len(),
            None,
            "serializing an uncached wrapper must not install stale cache bytes"
        );

        let cached = BlockMessageWire::with_encoded(
            Arc::new(wrapped.clone()),
            Arc::new(alternate_encoded.clone()),
        );
        assert_eq!(cached.encoded_len(), Some(alternate_encoded.len()));
        assert_eq!(
            cached.encode(),
            alternate_encoded,
            "cached serialization must use the cached full frame"
        );
        assert_consensus_params("cached wrapper message", cached.as_message(), 1, 2);

        let cached_owned = BlockMessageWire::with_encoded_owned(
            wrapped.clone(),
            Arc::new(wrapped_encoded.clone()),
        );
        assert_eq!(cached_owned.encoded_len(), Some(wrapped_encoded.len()));

        let into_message = cached.clone().into_message();
        assert_consensus_params("into_message", &into_message, 1, 2);

        let mut mutated = cached_owned.clone();
        *mutated.make_mut() = alternate.clone();
        assert_eq!(
            mutated.encoded_len(),
            None,
            "make_mut must clear cached full-frame bytes"
        );
        assert_consensus_params("mutated wrapper", mutated.as_message(), 3, 4);
        assert_eq!(mutated.encode(), alternate_encoded);

        let mut framed_with_trailing = wrapped_encoded.clone();
        framed_with_trailing.extend_from_slice(&[0xAA; 7]);
        let (decoded, consumed) =
            <BlockMessageWire as norito_core::DecodeFromSlice>::decode_from_slice(
                &framed_with_trailing,
            )
            .expect("decode framed block message prefix");
        assert_eq!(
            consumed,
            wrapped_encoded.len(),
            "decode_from_slice must consume exactly the framed prefix"
        );
        assert!(
            consumed < framed_with_trailing.len(),
            "trailing envelope bytes must remain unconsumed"
        );
        assert_consensus_params("decode_from_slice message", decoded.as_message(), 1, 2);
        assert_eq!(decoded.encoded_len(), Some(wrapped_encoded.len()));
        assert_eq!(
            decoded.encode(),
            wrapped_encoded,
            "decoded cache must preserve exactly the consumed frame"
        );

        let decoded_via_decode: BlockMessageWire =
            Decode::decode(&mut wrapped_encoded.as_slice()).expect("decode block message wire");
        assert_consensus_params(
            "Decode::decode message",
            decoded_via_decode.as_message(),
            1,
            2,
        );
        assert_eq!(
            decoded_via_decode.encoded_len(),
            Some(wrapped_encoded.len())
        );
        assert_eq!(decoded_via_decode.encode(), wrapped_encoded);

        let decoded_payload =
            decode_from_bytes::<BlockMessage>(&wrapped_encoded).expect("decode cached payload");
        assert_consensus_params("cached payload", &decoded_payload, 1, 2);

        let mut bad_magic = wrapped_encoded.clone();
        bad_magic[0] ^= 0xFF;
        assert_rejects_frame("bad magic", bad_magic);

        let mut bad_major = wrapped_encoded.clone();
        bad_major[4] = bad_major[4].wrapping_add(1);
        assert_rejects_frame("bad major version", bad_major);

        let mut bad_minor = wrapped_encoded.clone();
        bad_minor[5] = bad_minor[5].wrapping_add(1);
        assert_rejects_frame("bad minor version", bad_minor);

        let mut bad_schema = wrapped_encoded.clone();
        bad_schema[6] ^= 0x01;
        assert_rejects_frame("bad schema hash", bad_schema);

        let mut compressed = wrapped_encoded.clone();
        compressed[22] = norito_core::Compression::Zstd as u8;
        assert_rejects_frame("compressed frame", compressed);

        let mut missing_len = wrapped_encoded.clone();
        missing_len.truncate(norito_core::Header::SIZE - 1);
        assert_rejects_frame("missing length", missing_len);

        let mut length_overflow = wrapped_encoded.clone();
        length_overflow[LEN_OFF..LEN_OFF + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_rejects_frame("length overflow", length_overflow);

        let mut payload_unavailable = wrapped_encoded.clone();
        let too_large_payload = u64::try_from(wrapped_encoded.len()).expect("test frame length");
        payload_unavailable[LEN_OFF..LEN_OFF + 8].copy_from_slice(&too_large_payload.to_le_bytes());
        assert_rejects_frame("payload unavailable", payload_unavailable);
    }

    #[test]
    fn block_message_wire_network_roundtrip_cached_qc_vote() {
        let decoded = roundtrip_cached_block_message_over_network_message(BlockMessage::QcVote(
            sample_qc_vote(0x52),
        ));
        match decoded {
            crate::NetworkMessage::SumeragiBlock(wire) => {
                assert!(matches!(
                    wire.as_ref().as_message(),
                    BlockMessage::QcVote(_)
                ));
                assert!(
                    wire.as_ref()
                        .encoded_len()
                        .is_some_and(|len| len >= norito_core::Header::SIZE)
                );
            }
            other => panic!("expected cached sumeragi block message, got {other:?}"),
        }
    }

    #[test]
    fn block_message_wire_network_roundtrip_cached_qc() {
        let decoded =
            roundtrip_cached_block_message_over_network_message(BlockMessage::Qc(sample_qc(0x63)));
        match decoded {
            crate::NetworkMessage::SumeragiBlock(wire) => {
                assert!(matches!(wire.as_ref().as_message(), BlockMessage::Qc(_)));
                assert!(
                    wire.as_ref()
                        .encoded_len()
                        .is_some_and(|len| len >= norito_core::Header::SIZE)
                );
            }
            other => panic!("expected cached sumeragi block message, got {other:?}"),
        }
    }

    #[test]
    fn block_message_wire_network_roundtrip_cached_exec_witness() {
        let decoded = roundtrip_cached_block_message_over_network_message(
            BlockMessage::ExecWitness(sample_exec_witness_msg(0x74)),
        );
        match decoded {
            crate::NetworkMessage::SumeragiBlock(wire) => {
                assert!(matches!(
                    wire.as_ref().as_message(),
                    BlockMessage::ExecWitness(_)
                ));
                assert!(
                    wire.as_ref()
                        .encoded_len()
                        .is_some_and(|len| len >= norito_core::Header::SIZE)
                );
            }
            other => panic!("expected cached sumeragi block message, got {other:?}"),
        }
    }

    #[test]
    fn rbc_chunk_compact_roundtrip_normalizes() {
        let chunk = sample_rbc_chunk(4, 10, 2, 3, 1, vec![0xAB; 8]);
        let msg = BlockMessage::from_rbc_chunk(chunk.clone());
        let compact = match msg {
            BlockMessage::RbcChunkCompact(compact) => compact,
            other => panic!("expected compact RBC chunk, got {other:?}"),
        };
        let normalized = BlockMessage::RbcChunkCompact(compact).normalize();
        match normalized {
            BlockMessage::RbcChunk(full) => assert_eq!(full, chunk),
            other => panic!("expected normalized RBC chunk, got {other:?}"),
        }
    }

    #[test]
    fn rbc_chunk_compact_falls_back_on_large_headers() {
        let large_height = u64::from(u32::MAX) + 1;
        let chunk = sample_rbc_chunk(5, large_height, 1, 1, 2, vec![0xCD; 4]);
        let msg = BlockMessage::from_rbc_chunk(chunk.clone());
        assert!(matches!(msg, BlockMessage::RbcChunk(inner) if inner == chunk));
    }

    #[test]
    fn rbc_chunk_compact_boundary_and_field_preservation_match_formal_gate() {
        let max_fit = u64::from(u32::MAX);
        let chunk = sample_rbc_chunk(
            0xB1,
            max_fit,
            max_fit - 1,
            max_fit - 2,
            u32::MAX,
            vec![0x00, 0x11, 0xFE, 0xFF],
        );

        let compact = RbcChunkCompact::try_from_chunk(&chunk).expect("u32 boundary values fit");
        assert_compact_matches_chunk(&compact, &chunk);
        assert_eq!(compact.height, u32::MAX);
        assert_eq!(compact.view, u32::MAX - 1);
        assert_eq!(compact.epoch, u32::MAX - 2);
        assert_eq!(compact.idx, u32::MAX);

        match BlockMessage::from_rbc_chunk(chunk.clone()) {
            BlockMessage::RbcChunkCompact(compact) => {
                assert_compact_matches_chunk(&compact, &chunk)
            }
            other => panic!("expected compact RBC chunk, got {other:?}"),
        }

        for overflow in [
            sample_rbc_chunk(0xB2, max_fit + 1, 1, 1, 7, vec![0xA0]),
            sample_rbc_chunk(0xB3, 1, max_fit + 1, 1, 8, vec![0xA1]),
            sample_rbc_chunk(0xB4, 1, 1, max_fit + 1, 9, vec![0xA2]),
        ] {
            assert!(RbcChunkCompact::try_from_chunk(&overflow).is_none());
            assert!(
                matches!(BlockMessage::from_rbc_chunk(overflow.clone()), BlockMessage::RbcChunk(full) if full == overflow)
            );
        }
    }

    #[test]
    fn rbc_chunk_compact_normalization_matches_formal_gate() {
        let chunk = sample_rbc_chunk(0xC1, 33, 44, 55, 66, vec![1, 3, 5, 8, 13]);
        let compact = RbcChunkCompact::try_from_chunk(&chunk).expect("chunk headers fit in u32");

        assert_eq!(compact.clone().into_chunk(), chunk);
        match BlockMessage::RbcChunkCompact(compact).normalize() {
            BlockMessage::RbcChunk(full) => assert_eq!(full, chunk),
            other => panic!("expected compact normalization to yield full chunk, got {other:?}"),
        }

        let full_chunk = sample_rbc_chunk(0xC2, u64::from(u32::MAX) + 5, 77, 88, 99, vec![0xEE]);
        match BlockMessage::RbcChunk(full_chunk.clone()).normalize() {
            BlockMessage::RbcChunk(normalized) => assert_eq!(normalized, full_chunk),
            other => panic!("expected full RBC chunk to remain unchanged, got {other:?}"),
        }

        let requester = PeerId::from(
            KeyPair::try_from_seed(vec![0xC3; 32], Algorithm::Ed25519)
                .expect("derive compact fetch requester key")
                .public_key()
                .clone(),
        );
        let fetch = FetchPendingBlock {
            requester: requester.clone(),
            block_hash: chunk.block_hash,
            height: 33,
            view: 44,
            priority: Some(FetchPendingBlockPriority::Consensus),
            requester_roster_proof_known: Some(true),
            commit_qc_only: Some(false),
        };
        match BlockMessage::FetchPendingBlock(fetch.clone()).normalize() {
            BlockMessage::FetchPendingBlock(normalized) => {
                assert_eq!(normalized.requester, fetch.requester);
                assert_eq!(normalized.block_hash, fetch.block_hash);
                assert_eq!(normalized.height, fetch.height);
                assert_eq!(normalized.view, fetch.view);
                assert_eq!(normalized.priority, fetch.priority);
                assert_eq!(
                    normalized.requester_roster_proof_known,
                    fetch.requester_roster_proof_known
                );
                assert_eq!(normalized.commit_qc_only, fetch.commit_qc_only);
            }
            other => panic!("expected fetch message to remain unchanged, got {other:?}"),
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    fn bls_aggregate_disabled_with_mixed_backends() {}
}
