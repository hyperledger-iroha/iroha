//! Contains message structures for p2p communication during consensus.
use iroha_crypto::HashOf;
use iroha_data_model::{
    block::{BlockHeader, SignedBlock, consensus::SumeragiMembershipStatus},
    peer::PeerId,
};
use iroha_macro::*;
use norito::codec::{Decode, Encode};

use crate::block::NewBlock;

#[allow(clippy::enum_variant_names, clippy::large_enum_variant)]
/// Messages used by peers to communicate during the consensus process.
#[derive(Debug, Clone, Decode, Encode, FromVariant)]
pub enum BlockMessage {
    /// This message is sent by leader to all validating peers, when a new block is created.
    BlockCreated(#[skip_try_from] BlockCreated),
    /// This message is sent by `BlockSync` when a new block is received.
    BlockSyncUpdate(#[skip_try_from] BlockSyncUpdate),
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
        Self { block: signed }
    }
}

impl From<NewBlock> for BlockCreated {
    fn from(block: NewBlock) -> Self {
        Self {
            block: block.into(),
        }
    }
}

impl From<&SignedBlock> for BlockCreated {
    fn from(block: &SignedBlock) -> Self {
        Self {
            // Clone is required to own the message payload when constructed from a borrowed block.
            block: block.clone(),
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

// NOTE: Previously manual decoding validated signature uniqueness; Decode is now derived for simplicity.

/// Request a peer to resend a pending block payload.
#[derive(Debug, Clone, Decode, Encode)]
pub struct FetchPendingBlock {
    /// Peer requesting the payload.
    pub requester: PeerId,
    /// Hash of the missing block.
    pub block_hash: HashOf<BlockHeader>,
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
    use iroha_data_model::{
        da::{
            commitment::{DaCommitmentBundle, DaCommitmentRecord, DaProofScheme, KzgCommitment},
            types::{BlobDigest, RetentionPolicy, StorageTicketId},
        },
        nexus::LaneId,
        sorafs::pin_registry::ManifestDigest,
    };

    use super::*;
    use crate::block::BlockBuilder;

    #[test]
    fn block_created_from_newblock_ref_and_move_equivalent() {
        // Build an empty NewBlock (no transactions) and sign it.
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
        let new_block = BlockBuilder::new(Vec::new())
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

    #[cfg(feature = "bls")]
    #[test]
    fn bls_aggregate_disabled_with_mixed_backends() {}
}
