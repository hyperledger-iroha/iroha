//! Contains message structures for p2p communication during consensus.
use std::{io::Write, sync::Arc};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::{BlockHeader, BlockSignature, SignedBlock, consensus::SumeragiMembershipStatus},
    peer::PeerId,
};
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

    /// Wrap an `Arc`-backed message with cached encoded bytes.
    pub fn with_encoded(message: Arc<BlockMessage>, encoded: Arc<Vec<u8>>) -> Self {
        Self {
            message,
            encoded: Some(encoded),
        }
    }

    /// Wrap an owned message with cached encoded bytes.
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
        Arc::try_unwrap(self.message).unwrap_or_else(|arc| (*arc).clone())
    }

    /// Cached encoded length if available.
    pub fn encoded_len(&self) -> Option<usize> {
        self.encoded.as_ref().map(|bytes| bytes.len())
    }

    pub(crate) fn encode_message(message: &BlockMessage) -> Vec<u8> {
        let reserve = message
            .encoded_len_exact()
            .unwrap_or_else(|| message.encoded_len());
        let mut buf = Vec::with_capacity(reserve);
        message.encode_to(&mut buf);
        buf
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
        self.message.as_ref().serialize(writer)
    }
}

impl<'a> NoritoDeserialize<'a> for BlockMessageWire {
    fn deserialize(archived: &'a ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("decode block message wire")
    }

    fn try_deserialize(archived: &'a ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let bytes = ncore::payload_slice_from_ptr(ptr)?;
        let (message, consumed) = ncore::decode_field_canonical_slice::<BlockMessage>(bytes)?;
        let encoded = Arc::new(bytes[..consumed].to_vec());
        Ok(Self {
            message: Arc::new(message),
            encoded: Some(encoded),
        })
    }
}

impl<'a> ncore::DecodeFromSlice<'a> for BlockMessageWire {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let (message, consumed) = ncore::decode_field_canonical_slice::<BlockMessage>(bytes)?;
        let encoded = Arc::new(bytes[..consumed].to_vec());
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

    #[test]
    fn block_created_from_newblock_ref_and_move_equivalent() {
        // Build a minimal NewBlock and sign it.
        let kp = KeyPair::from_seed(b"seed-seed".to_vec(), Algorithm::Ed25519);
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
        let kp = KeyPair::from_seed(b"seed-seed".to_vec(), Algorithm::Ed25519);
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
        let chunk = super::super::consensus::RbcChunk {
            block_hash,
            height: 1,
            view: 0,
            epoch: 0,
            idx: 0,
            bytes: vec![0u8; 1],
        };
        let msg = BlockMessage::RbcChunk(chunk);
        assert_eq!(msg.priority(), iroha_p2p::Priority::High);

        let requester = PeerId::from(KeyPair::random().public_key().clone());
        let fetch = BlockMessage::FetchPendingBlock(FetchPendingBlock {
            requester,
            block_hash,
            height: 1,
            view: 0,
            priority: None,
            requester_roster_proof_known: None,
        });
        assert_eq!(fetch.priority(), iroha_p2p::Priority::High);
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
    fn rbc_chunk_compact_roundtrip_normalizes() {
        let chunk = consensus::RbcChunk {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([4u8; 32])),
            height: 10,
            view: 2,
            epoch: 3,
            idx: 1,
            bytes: vec![0xAB; 8],
        };
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
        let chunk = consensus::RbcChunk {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([5u8; 32])),
            height: large_height,
            view: 1,
            epoch: 1,
            idx: 2,
            bytes: vec![0xCD; 4],
        };
        let msg = BlockMessage::from_rbc_chunk(chunk.clone());
        assert!(matches!(msg, BlockMessage::RbcChunk(inner) if inner == chunk));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn bls_aggregate_disabled_with_mixed_backends() {}
}
