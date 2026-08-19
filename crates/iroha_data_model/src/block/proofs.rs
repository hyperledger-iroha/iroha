//! Stable wrappers for Merkle proofs over block entrypoints and execution results.
//!
//! These types bundle the carrier identity, leaf hash, canonical audit path, and exact root/count
//! commitments required to verify inclusion without depending on internal structures. Block proof
//! responses use the full executed-entrypoint tree, including scheduled entrypoints, so entry and
//! result indices share one execution order. A fully verified Sumeragi-v2 `CommitQC` authenticates
//! the exact executed block wire and therefore that tree. `BlockHeader::merkle_root` is checked as
//! proposal metadata, but is never selected as the entry-proof anchor.
use crate::{
    block::{
        BlockHeader, SignedBlock,
        consensus_v2::{
            ExecutionCommitment,
            finality::{
                V2FinalityArtifact, V2FinalityValidationError, V2QuorumCertificateVerificationError,
            },
        },
    },
    fastpq::TransferTranscript,
    transaction::signed::{TransactionEntrypoint, TransactionResult},
};
use core::num::NonZeroU64;
use iroha_crypto::{Hash, HashOf, MerkleProof, MerkleTree, MerkleTreeCommitment};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::collections::BTreeMap;
/// Maximum leaf count representable by block receipt proof indices.
const BLOCK_MERKLE_MAX_LEAF_COUNT: u64 = 1_u64 << u32::BITS;
/// Maximum exact executed `SignedBlockWire` bytes accepted by the first-release
/// authenticated block-proof carrier and verifier.
///
/// This is a public protocol resource bound: Torii refuses to emit a larger
/// carrier and native SDK verifiers refuse to allocate or decode one.
pub const AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1: usize = 32 * 1024 * 1024;
/// Merkle inclusion proof for a transaction entrypoint under an authenticated
/// root-and-count commitment.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct BlockReceiptProof {
    /// Hash of the transaction entrypoint proven to be part of the block.
    leaf: HashOf<TransactionEntrypoint>,
    /// Canonical audit path leading to the authenticated entrypoint Merkle root.
    proof: MerkleProof<TransactionEntrypoint>,
}
impl BlockReceiptProof {
    /// Construct a new proof from a leaf hash and the corresponding audit path.
    #[must_use]
    pub const fn new(
        leaf: HashOf<TransactionEntrypoint>,
        proof: MerkleProof<TransactionEntrypoint>,
    ) -> Self {
        Self { leaf, proof }
    }
    /// Returns the leaf hash covered by this proof.
    #[must_use]
    pub const fn leaf(&self) -> &HashOf<TransactionEntrypoint> {
        &self.leaf
    }
    /// Returns the underlying Merkle proof.
    #[must_use]
    pub const fn proof(&self) -> &MerkleProof<TransactionEntrypoint> {
        &self.proof
    }
    /// Verify the proof against the supplied root-and-leaf-count commitment.
    #[must_use]
    pub fn verify(&self, commitment: &MerkleTreeCommitment<TransactionEntrypoint>) -> bool {
        commitment.leaf_count().get() <= BLOCK_MERKLE_MAX_LEAF_COUNT
            && self.proof.verify(&self.leaf, commitment)
    }
}
/// Merkle inclusion proof for a transaction execution result referenced by
/// `BlockHeader::result_merkle_root`.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ExecutionReceiptProof {
    /// Hash of the execution result proven to be part of the block.
    leaf: HashOf<TransactionResult>,
    /// Canonical audit path leading to the result Merkle root.
    proof: MerkleProof<TransactionResult>,
}
impl ExecutionReceiptProof {
    /// Construct a new proof from a result hash and its audit path.
    #[must_use]
    pub const fn new(
        leaf: HashOf<TransactionResult>,
        proof: MerkleProof<TransactionResult>,
    ) -> Self {
        Self { leaf, proof }
    }
    /// Returns the hashed execution result covered by this proof.
    #[must_use]
    pub const fn leaf(&self) -> &HashOf<TransactionResult> {
        &self.leaf
    }
    /// Returns the underlying Merkle proof.
    #[must_use]
    pub const fn proof(&self) -> &MerkleProof<TransactionResult> {
        &self.proof
    }
    /// Verify the proof against the supplied root-and-leaf-count commitment.
    #[must_use]
    pub fn verify(&self, commitment: &MerkleTreeCommitment<TransactionResult>) -> bool {
        commitment.leaf_count().get() <= BLOCK_MERKLE_MAX_LEAF_COUNT
            && self.proof.verify(&self.leaf, commitment)
    }
}
/// Combined entrypoint/result proofs for a transaction included in a block.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct BlockProofs {
    /// Height of the block containing the transaction.
    pub block_height: NonZeroU64,
    /// Consensus hash of the exact carrier block header.
    pub block_hash: HashOf<BlockHeader>,
    /// Hash of the canonical executed `SignedBlockWire` bytes.
    pub executed_block_wire_hash: Hash,
    /// Hash of the transaction entrypoint proven to exist in the block.
    pub entry_hash: HashOf<TransactionEntrypoint>,
    /// Claimed Merkle root and exact leaf count used to verify the entrypoint proof.
    pub entry_commitment: MerkleTreeCommitment<TransactionEntrypoint>,
    /// Merkle proof under the full executed-entrypoint commitment.
    pub entry_proof: BlockReceiptProof,
    /// Claimed Merkle root and exact leaf count used to verify the execution proof.
    pub result_commitment: MerkleTreeCommitment<TransactionResult>,
    /// Execution result proof under `BlockHeader::result_merkle_root`.
    pub result_proof: ExecutionReceiptProof,
    /// Claimed FASTPQ transfer transcripts grouped by transaction entrypoint hash.
    pub fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>,
}
/// Trusted block identity, Merkle commitments, and executed transcript projection used to verify
/// [`BlockProofs`].
///
/// This capability is intentionally not serializable and its fields are private. Its public
/// constructor verifies untrusted Sumeragi-v2 finality, exact header association, and executed-wire
/// binding before recomputing the Merkle commitments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedBlockProofAnchor {
    block_height: NonZeroU64,
    block_hash: HashOf<BlockHeader>,
    executed_block_wire_hash: Hash,
    entry_hash: HashOf<TransactionEntrypoint>,
    entry_index: u32,
    entry_commitment: MerkleTreeCommitment<TransactionEntrypoint>,
    result_commitment: MerkleTreeCommitment<TransactionResult>,
    fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>,
}
/// Failure to derive a trusted proof anchor from authenticated block metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TrustedBlockProofAnchorError {
    /// The supplied finality artifact failed structural, roster, proof-of-possession, or
    /// aggregate-signature verification.
    #[error("untrusted finality artifact failed cryptographic verification: {0}")]
    FinalityVerification(V2QuorumCertificateVerificationError),
    /// A cryptographically valid finality artifact does not finalize the supplied block header.
    #[error("verified finality artifact does not match the supplied block header: {0}")]
    FinalityHeaderMismatch(V2FinalityValidationError),
    /// The exact executed block wire could not be encoded canonically.
    #[error("failed to encode the authenticated executed block wire")]
    ExecutedBlockWireEncoding,
    /// The authenticated execution commitment belongs to a different block wire.
    #[error("execution commitment does not bind the supplied executed block wire")]
    ExecutedBlockWireMismatch,
    /// An entrypoint proof anchor requires a non-empty executed entrypoint tree.
    #[error("authenticated block has no executed entrypoints")]
    MissingEntrypoints,
    /// The requested entrypoint is not present in the authenticated block.
    #[error("requested entrypoint is absent from the authenticated block")]
    EntrypointNotFound {
        /// Hash requested by the proof consumer.
        entry_hash: HashOf<TransactionEntrypoint>,
    },
    /// The authenticated entrypoint tree exceeds the block-proof index space.
    #[error("authenticated block entrypoint count exceeds the u32 proof index space")]
    TooManyEntrypoints,
    /// An executed entrypoint proof anchor requires an aligned result tree.
    #[error("authenticated block has no execution results")]
    MissingResults,
    /// Entrypoint and result leaf counts are not aligned one-for-one.
    #[error("authenticated entrypoint and result counts are misaligned")]
    MisalignedLeafCounts,
    /// Stored Merkle material disagrees with the authenticated block contents.
    #[error("authenticated block carries inconsistent Merkle material")]
    InconsistentMerkleMaterial,
}
impl TrustedBlockProofAnchor {
    /// Derive a target-specific anchor from an untrusted finality artifact.
    ///
    /// This first verifies the artifact's complete frozen-roster, proof-of-possession, and
    /// `CommitQC` cryptography, then validates its exact association with `block.header()`. Only
    /// after both checks succeed does it use the `CommitQC`'s execution commitment to authenticate
    /// the exact executed block wire. It validates the retained entrypoint/result caches in place,
    /// checks their exact count alignment, locates `entry_hash` in authenticated block order, and
    /// retains the exact FASTPQ transcript map bound by that wire. Every target binds the full
    /// executed-entrypoint tree so its index is identical to the corresponding result index. The
    /// external-only header root is checked with a logarithmic-memory accumulator.
    ///
    /// # Errors
    /// Returns [`TrustedBlockProofAnchorError`] when finality verification or header association
    /// fails, the exact block wire is not the `CommitQC`-authenticated wire, or Merkle material is
    /// missing or inconsistent.
    pub fn from_untrusted_finality_artifact(
        block: &SignedBlock,
        artifact: &V2FinalityArtifact,
        entry_hash: &HashOf<TransactionEntrypoint>,
    ) -> Result<Self, TrustedBlockProofAnchorError> {
        artifact
            .verify()
            .map_err(TrustedBlockProofAnchorError::FinalityVerification)?;
        artifact
            .validate_for_header(&block.header())
            .map_err(TrustedBlockProofAnchorError::FinalityHeaderMismatch)?;
        Self::from_authenticated_execution(
            block,
            &artifact.commit_qc.execution_commitment,
            entry_hash,
        )
    }
    fn from_authenticated_execution(
        block: &SignedBlock,
        execution_commitment: &ExecutionCommitment,
        entry_hash: &HashOf<TransactionEntrypoint>,
    ) -> Result<Self, TrustedBlockProofAnchorError> {
        let executed_block_wire_hash = block
            .executed_block_wire_hash()
            .map_err(|_| TrustedBlockProofAnchorError::ExecutedBlockWireEncoding)?;
        if executed_block_wire_hash != execution_commitment.executed_block_wire_hash {
            return Err(TrustedBlockProofAnchorError::ExecutedBlockWireMismatch);
        }
        if !block.has_results() {
            return Err(TrustedBlockProofAnchorError::MissingResults);
        }
        block
            .validate_entrypoint_merkle_cache()
            .map_err(|_| TrustedBlockProofAnchorError::InconsistentMerkleMaterial)?;
        block
            .validate_result_merkle_cache()
            .map_err(|_| TrustedBlockProofAnchorError::InconsistentMerkleMaterial)?;
        let full_entry_commitment = block
            .full_entry_merkle_commitment()
            .ok_or(TrustedBlockProofAnchorError::MissingEntrypoints)?;
        let result_commitment = block
            .result_merkle_commitment()
            .ok_or(TrustedBlockProofAnchorError::MissingResults)?;
        if full_entry_commitment.leaf_count() != result_commitment.leaf_count() {
            return Err(TrustedBlockProofAnchorError::MisalignedLeafCounts);
        }
        if full_entry_commitment.leaf_count().get() > BLOCK_MERKLE_MAX_LEAF_COUNT {
            return Err(TrustedBlockProofAnchorError::TooManyEntrypoints);
        }
        let external_count = u64::try_from(block.external_entrypoint_count())
            .map_err(|_| TrustedBlockProofAnchorError::InconsistentMerkleMaterial)?;
        if external_count > full_entry_commitment.leaf_count().get() {
            return Err(TrustedBlockProofAnchorError::InconsistentMerkleMaterial);
        }
        let external_root = MerkleTree::root_from_typed_leaves(
            block
                .external_entrypoints_cloned()
                .map(|entrypoint| entrypoint.hash()),
        );
        if block.header().merkle_root() != external_root {
            return Err(TrustedBlockProofAnchorError::InconsistentMerkleMaterial);
        }
        if block.full_entry_merkle_commitment() != Some(full_entry_commitment)
            || block.result_merkle_commitment() != Some(result_commitment)
            || block.header().result_merkle_root() != Some(*result_commitment.root())
        {
            return Err(TrustedBlockProofAnchorError::InconsistentMerkleMaterial);
        }
        let entry_index_usize = block
            .entrypoint_hashes()
            .position(|candidate| &candidate == entry_hash)
            .ok_or(TrustedBlockProofAnchorError::EntrypointNotFound {
                entry_hash: *entry_hash,
            })?;
        let entry_index = u32::try_from(entry_index_usize)
            .map_err(|_| TrustedBlockProofAnchorError::TooManyEntrypoints)?;
        Ok(Self {
            block_height: block.header().height(),
            block_hash: block.hash(),
            executed_block_wire_hash,
            entry_hash: *entry_hash,
            entry_index,
            entry_commitment: full_entry_commitment,
            result_commitment,
            fastpq_transcripts: block.fastpq_transcripts().clone(),
        })
    }
    /// Return the anchored block height.
    #[must_use]
    pub const fn block_height(&self) -> NonZeroU64 {
        self.block_height
    }
    /// Return the anchored block-header hash.
    #[must_use]
    pub const fn block_hash(&self) -> HashOf<BlockHeader> {
        self.block_hash
    }
    /// Return the anchored exact executed-block wire hash.
    #[must_use]
    pub const fn executed_block_wire_hash(&self) -> Hash {
        self.executed_block_wire_hash
    }
    /// Return the anchored target entrypoint hash.
    #[must_use]
    pub const fn entry_hash(&self) -> HashOf<TransactionEntrypoint> {
        self.entry_hash
    }
    /// Return the anchored target entrypoint index in block execution order.
    #[must_use]
    pub const fn entry_index(&self) -> u32 {
        self.entry_index
    }
    /// Return the full executed-entrypoint-tree commitment.
    #[must_use]
    pub const fn entry_commitment(&self) -> MerkleTreeCommitment<TransactionEntrypoint> {
        self.entry_commitment
    }
    /// Return the anchored result-tree commitment.
    #[must_use]
    pub const fn result_commitment(&self) -> MerkleTreeCommitment<TransactionResult> {
        self.result_commitment
    }
    /// Return the exact FASTPQ transcript projection authenticated by the executed block wire.
    #[must_use]
    pub fn fastpq_transcripts(&self) -> &BTreeMap<Hash, Vec<TransferTranscript>> {
        &self.fastpq_transcripts
    }
}
impl BlockProofs {
    /// Verify all proof fields against a separately authenticated anchor.
    #[must_use]
    pub fn verify(&self, anchor: &TrustedBlockProofAnchor) -> bool {
        if self.block_height != anchor.block_height
            || self.block_hash != anchor.block_hash
            || self.executed_block_wire_hash != anchor.executed_block_wire_hash
            || self.entry_hash != anchor.entry_hash
            || self.entry_proof.proof().leaf_index() != anchor.entry_index
            || self.entry_commitment != anchor.entry_commitment
            || self.entry_commitment.leaf_count() != self.result_commitment.leaf_count()
            || self.entry_hash != *self.entry_proof.leaf()
            || !self.entry_proof.verify(&anchor.entry_commitment)
            || self.result_commitment != anchor.result_commitment
            || self.entry_proof.proof().leaf_index() != self.result_proof.proof().leaf_index()
            || !self.result_proof.verify(&anchor.result_commitment)
            || self.fastpq_transcripts != anchor.fastpq_transcripts
        {
            return false;
        }
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account::AccountId,
        domain::DomainId,
        transaction::{
            TransactionResultInner,
            signed::{TransactionBuilder, TransactionResult},
        },
    };
    #[cfg(feature = "transparent_api")]
    use crate::{
        block::{
            BlockSignature,
            consensus_v2::{
                BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
                GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding, QuorumCertificate,
                ValidatorPower, Vote,
            },
        },
        peer::PeerId,
        transaction::ExecutionStep,
        trigger::{DataTriggerSequence, TimeTriggerEntrypoint},
    };
    #[cfg(feature = "transparent_api")]
    use iroha_crypto::{Algorithm, Signature, SignatureOf};
    use iroha_crypto::{Hash, HashOf, KeyPair, MerkleTree};
    #[cfg(feature = "transparent_api")]
    use iroha_primitives::const_vec::ConstVec;
    use norito::codec::DecodeAll as _;
    use std::iter::FromIterator;
    fn sample_entrypoint_hash() -> HashOf<TransactionEntrypoint> {
        let keypair = checked_random_keypair();
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let authority = AccountId::new(keypair.public_key().clone());
        let tx = TransactionBuilder::new(
            test_network_id(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .try_sign(keypair.private_key())
        .expect("checked block proof fixture transaction signature");
        tx.hash_as_entrypoint()
    }
    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("generate checked block proof fixture keypair")
    }
    fn test_network_id() -> crate::NetworkId {
        crate::NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([0x15; Hash::LENGTH]),
        ))
    }
    #[test]
    fn block_receipt_proof_verifies_against_merkle_root() {
        let hash = sample_entrypoint_hash();
        let tree = MerkleTree::from_iter([hash]);
        let proof = tree.get_proof(0).expect("proof must exist for single leaf");
        let receipt = BlockReceiptProof::new(hash, proof);
        let commitment = tree.commitment().expect("commitment must exist");
        assert!(
            receipt.verify(&commitment),
            "proof must verify against commitment"
        );
    }
    #[test]
    fn block_receipt_proof_rejects_mutated_leaf() {
        let tree = MerkleTree::from_iter([sample_entrypoint_hash()]);
        let proof = tree.get_proof(0).expect("proof must exist for single leaf");
        let forged =
            BlockReceiptProof::new(HashOf::from_untyped_unchecked(Hash::new([0xAA; 32])), proof);
        let commitment = tree.commitment().expect("commitment must exist");
        assert!(
            !forged.verify(&commitment),
            "tampered leaf hash should not verify against commitment"
        );
    }
    #[test]
    fn block_receipt_proof_rejects_wrong_root() {
        let first = sample_entrypoint_hash();
        let tree = MerkleTree::from_iter([first]);
        let proof = tree.get_proof(0).expect("proof must exist for first leaf");
        let receipt = BlockReceiptProof::new(first, proof);
        let commitment = tree.commitment().expect("commitment must exist");
        let wrong_root = HashOf::from_untyped_unchecked(Hash::new([0xBB; 32]));
        let wrong_commitment = MerkleTreeCommitment::new(wrong_root, commitment.leaf_count());
        assert!(
            !receipt.verify(&wrong_commitment),
            "proof should fail when verified against a different commitment root"
        );
    }
    #[test]
    fn block_receipt_proof_rejects_wrong_leaf_count() {
        let hash = sample_entrypoint_hash();
        let tree = MerkleTree::from_iter([hash]);
        let proof = tree.get_proof(0).expect("proof must exist for single leaf");
        let receipt = BlockReceiptProof::new(hash, proof);
        let root = tree.root().expect("root must exist");
        let wrong_commitment = MerkleTreeCommitment::new(
            root,
            NonZeroU64::new(2).expect("leaf count must be non-zero"),
        );
        assert!(
            !receipt.verify(&wrong_commitment),
            "proof should fail when its path shape does not match the committed leaf count"
        );
    }
    #[test]
    fn block_receipt_and_generic_proof_reject_commitment_beyond_u32_index_space() {
        const LEAF_NODE_DOMAIN: &[u8] = b"iroha:merkle:leaf:v1\x00";
        const INTERNAL_NODE_DOMAIN: &[u8] = b"iroha:merkle:internal:v1\x00";
        let leaf = sample_entrypoint_hash();
        let sibling = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
            b"oversized block proof sibling",
        ));
        let audit_path = vec![Some(sibling); 33];
        let mut computed_root = HashOf::<MerkleTree<TransactionEntrypoint>>::from_untyped_unchecked(
            Hash::new_from_chunks(&[LEAF_NODE_DOMAIN, leaf.as_ref()]),
        );
        for _ in &audit_path {
            computed_root = HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
                INTERNAL_NODE_DOMAIN,
                computed_root.as_ref(),
                sibling.as_ref(),
            ]));
        }
        let commitment = MerkleTreeCommitment::new(
            computed_root,
            NonZeroU64::new(BLOCK_MERKLE_MAX_LEAF_COUNT + 1).expect("leaf count must be non-zero"),
        );
        let receipt = BlockReceiptProof::new(leaf, MerkleProof::from_audit_path(0, audit_path));
        assert!(
            !receipt.proof().verify(receipt.leaf(), &commitment),
            "the canonical proof index is u32 and cannot address a larger tree"
        );
        assert!(
            !receipt.verify(&commitment),
            "block receipt proofs must remain bounded by their u32 leaf index"
        );
    }
    #[test]
    fn execution_receipt_proof_verifies_against_result_merkle_root() {
        let sequence = TransactionResultInner::Ok(crate::trigger::DataTriggerSequence::default());
        let result_hash = TransactionResult::hash_from_inner(&sequence);
        let tree = MerkleTree::from_iter([result_hash]);
        let proof = tree.get_proof(0).expect("proof must exist for result leaf");
        let execution = ExecutionReceiptProof::new(result_hash, proof);
        let commitment = tree.commitment().expect("commitment must exist");
        assert!(
            execution.verify(&commitment),
            "execution proof must verify against commitment"
        );
    }
    #[test]
    fn execution_receipt_proof_rejects_wrong_root() {
        let sequence = TransactionResultInner::Ok(crate::trigger::DataTriggerSequence::default());
        let result_hash = TransactionResult::hash_from_inner(&sequence);
        let tree = MerkleTree::from_iter([result_hash]);
        let proof = tree.get_proof(0).expect("proof must exist for result leaf");
        let execution = ExecutionReceiptProof::new(result_hash, proof);
        let commitment = tree.commitment().expect("commitment must exist");
        let wrong_root = HashOf::from_untyped_unchecked(Hash::new([0xCC; 32]));
        let wrong_commitment = MerkleTreeCommitment::new(wrong_root, commitment.leaf_count());
        assert!(
            !execution.verify(&wrong_commitment),
            "execution proof must fail against a mismatched commitment root"
        );
    }
    #[test]
    fn block_proofs_norito_roundtrip_preserves_anchor_and_commitment() {
        let entry_hash = sample_entrypoint_hash();
        let tree = MerkleTree::from_iter([entry_hash]);
        let entry_commitment = tree.commitment().expect("commitment must exist");
        let entry_proof = BlockReceiptProof::new(
            entry_hash,
            tree.get_proof(0).expect("proof must exist for single leaf"),
        );
        let result_hash = TransactionResult::hash_from_inner(&TransactionResultInner::Ok(
            crate::trigger::DataTriggerSequence::default(),
        ));
        let result_tree = MerkleTree::from_iter([result_hash]);
        let result_commitment = result_tree.commitment().expect("result commitment");
        let result_proof = ExecutionReceiptProof::new(
            result_hash,
            result_tree.get_proof(0).expect("result proof"),
        );
        let proofs = BlockProofs {
            block_height: NonZeroU64::new(7).expect("block height must be non-zero"),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"carrier block")),
            executed_block_wire_hash: Hash::new(b"executed block wire"),
            entry_hash,
            entry_commitment,
            entry_proof,
            result_commitment,
            result_proof,
            fastpq_transcripts: BTreeMap::new(),
        };
        let encoded = proofs.encode();
        let decoded = BlockProofs::decode_all(&mut encoded.as_slice())
            .expect("canonical block proofs must decode");
        assert_eq!(decoded, proofs);
    }
    #[cfg(feature = "transparent_api")]
    fn finalized_artifact_for_block(
        block: &SignedBlock,
        execution_commitment: &ExecutionCommitment,
    ) -> V2FinalityArtifact {
        let mut key_pairs = core::iter::repeat_with(|| {
            KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
                .expect("generate checked finality fixture keypair")
        })
        .take(4)
        .collect::<Vec<_>>();
        key_pairs.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = key_pairs
            .iter()
            .map(|key| ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = HeightContext {
            network_id: test_network_id(),
            protocol_version: PROTOCOL_VERSION,
            height: block.header().height().get(),
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"trusted proof anchor finality context"),
            execution_policy_hash: Hash::new(b"trusted proof anchor execution policy"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
            },
            leader_seed: [0xA7; 32],
        };
        let subject = BlockSubject {
            parent_block_hash: block.header().prev_block_hash(),
            block_hash: block.hash(),
            payload_hash: block
                .canonical_proposal_wire_hash()
                .expect("fixture canonical proposal wire"),
        };
        let round = ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: block.header().view_change_index(),
        };
        let vote = Vote {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment: *execution_commitment,
            signer: 0,
            signature: Vec::new(),
        };
        let preimage = vote.signature_preimage();
        let shares = key_pairs[..3]
            .iter()
            .map(|key| {
                Signature::new(key.private_key(), &preimage)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let commit_qc = QuorumCertificate {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment: *execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                .expect("aggregate fixture CommitQC"),
        };
        let validator_set_pops = key_pairs
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture validator PoP")
            })
            .collect();
        let artifact = V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops);
        artifact.verify().expect("fixture finality must verify");
        artifact
            .validate_for_header(&block.header())
            .expect("fixture finality must match the block header");
        artifact
    }
    #[cfg(feature = "transparent_api")]
    fn authenticated_block_with_scheduled_entry() -> (
        SignedBlock,
        V2FinalityArtifact,
        HashOf<TransactionEntrypoint>,
        HashOf<TransactionEntrypoint>,
    ) {
        let keypair = checked_random_keypair();
        let authority = AccountId::new(keypair.public_key().clone());
        let transaction = TransactionBuilder::new_genesis(
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .try_sign(keypair.private_key())
        .expect("fixture transaction signature");
        let scheduled = TimeTriggerEntrypoint {
            id: "anchor_schedule".parse().expect("trigger id"),
            instructions: ExecutionStep(ConstVec::new_empty()),
            authority,
        };
        let external_hash = transaction.hash_as_entrypoint();
        let scheduled_hash = scheduled.hash_as_entrypoint();
        let entry_hashes = [external_hash, scheduled_hash];
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let signature = BlockSignature::new(
            0,
            SignatureOf::try_from_hash(keypair.private_key(), header.hash())
                .expect("fixture block signature"),
        );
        let mut block = SignedBlock::presigned(signature, header, vec![transaction]);
        block
            .set_transaction_results(
                vec![scheduled],
                &entry_hashes,
                vec![
                    TransactionResultInner::Ok(DataTriggerSequence::default()),
                    TransactionResultInner::Ok(DataTriggerSequence::default()),
                ],
            )
            .expect("fixture entrypoints and results align");
        let executed_block_wire = block
            .encode_wire()
            .expect("fixture executed block wire encodes");
        let executed_block_wire_len =
            u64::try_from(executed_block_wire.len()).expect("fixture wire length fits u64");
        let executed_block_wire_hash = Hash::new(&executed_block_wire);
        let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"trusted proof parent state"),
            Hash::new(b"trusted proof post state"),
            Hash::new(b"trusted proof ordinary writes"),
            executed_block_wire_len,
            executed_block_wire_hash,
        );
        execution_commitment
            .validate()
            .expect("fixture execution commitment is valid");
        let artifact = finalized_artifact_for_block(&block, &execution_commitment);
        (block, artifact, external_hash, scheduled_hash)
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn trusted_anchor_accepts_real_finality_and_uses_full_tree_for_external_target() {
        let (block, artifact, external_hash, _) = authenticated_block_with_scheduled_entry();
        let proofs = block
            .proofs_for_entry_hash(&external_hash)
            .expect("external proof exists");
        let anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
            &block,
            &artifact,
            &external_hash,
        )
        .expect("external anchor derives");
        assert_eq!(anchor.entry_hash(), external_hash);
        assert_eq!(anchor.entry_index(), 0);
        assert_eq!(anchor.entry_commitment(), proofs.entry_commitment);
        assert_eq!(
            anchor.entry_commitment(),
            block
                .full_entry_merkle_commitment()
                .expect("full entry commitment")
        );
        assert_ne!(
            anchor.entry_commitment().root(),
            &block.header().merkle_root().expect("external root"),
            "the external-only header tree must not replace the executed-entry tree"
        );
        assert_eq!(anchor.fastpq_transcripts(), block.fastpq_transcripts());
        assert!(proofs.verify(&anchor));
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn trusted_anchor_uses_full_tree_for_scheduled_target() {
        let (block, artifact, _, scheduled_hash) = authenticated_block_with_scheduled_entry();
        let proofs = block
            .proofs_for_entry_hash(&scheduled_hash)
            .expect("scheduled proof exists");
        let anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
            &block,
            &artifact,
            &scheduled_hash,
        )
        .expect("scheduled anchor derives");
        assert_eq!(anchor.entry_hash(), scheduled_hash);
        assert_eq!(anchor.entry_index(), 1);
        assert_eq!(
            anchor.entry_commitment(),
            block
                .full_entry_merkle_commitment()
                .expect("full entry commitment")
        );
        assert_eq!(anchor.entry_commitment(), proofs.entry_commitment);
        assert!(proofs.verify(&anchor));
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn trusted_anchor_rejects_unknown_or_substituted_target() {
        let (block, artifact, external_hash, scheduled_hash) =
            authenticated_block_with_scheduled_entry();
        let external_anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
            &block,
            &artifact,
            &external_hash,
        )
        .expect("external anchor derives");
        let scheduled_proofs = block
            .proofs_for_entry_hash(&scheduled_hash)
            .expect("scheduled proof exists");
        assert!(
            !scheduled_proofs.verify(&external_anchor),
            "a valid proof for another entrypoint in the same tree must not satisfy the target anchor"
        );
        let missing_hash = HashOf::from_untyped_unchecked(Hash::new(b"missing entrypoint"));
        assert_eq!(
            TrustedBlockProofAnchor::from_untrusted_finality_artifact(
                &block,
                &artifact,
                &missing_hash,
            ),
            Err(TrustedBlockProofAnchorError::EntrypointNotFound {
                entry_hash: missing_hash,
            })
        );
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn self_consistent_block_and_execution_commitment_cannot_mint_an_anchor_without_valid_qc() {
        let (block, mut artifact, external_hash, _) = authenticated_block_with_scheduled_entry();
        artifact.commit_qc.aggregate_signature[0] ^= 0x80;
        assert!(matches!(
            TrustedBlockProofAnchor::from_untrusted_finality_artifact(
                &block,
                &artifact,
                &external_hash,
            ),
            Err(TrustedBlockProofAnchorError::FinalityVerification(
                V2QuorumCertificateVerificationError::InvalidAggregateSignature
            ))
        ));
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn trusted_anchor_rejects_valid_finality_for_another_header() {
        let (block, artifact, _, _) = authenticated_block_with_scheduled_entry();
        let (other_block, _, other_external_hash, _) = authenticated_block_with_scheduled_entry();
        assert_eq!(
            TrustedBlockProofAnchor::from_untrusted_finality_artifact(
                &other_block,
                &artifact,
                &other_external_hash,
            ),
            Err(TrustedBlockProofAnchorError::FinalityHeaderMismatch(
                V2FinalityValidationError::AssociatedBlockHashMismatch,
            ))
        );
        assert_ne!(block.hash(), other_block.hash());
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn trusted_anchor_rejects_cryptographically_finalized_wrong_executed_wire() {
        let (block, _, external_hash, _) = authenticated_block_with_scheduled_entry();
        let wrong_executed_block_wire = b"different finalized executed block wire";
        let wrong_execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"wrong-wire parent state"),
            Hash::new(b"wrong-wire post state"),
            Hash::new(b"wrong-wire ordinary writes"),
            u64::try_from(wrong_executed_block_wire.len())
                .expect("wrong fixture wire length fits u64"),
            Hash::new(wrong_executed_block_wire),
        );
        let artifact = finalized_artifact_for_block(&block, &wrong_execution_commitment);
        assert_eq!(
            TrustedBlockProofAnchor::from_untrusted_finality_artifact(
                &block,
                &artifact,
                &external_hash,
            ),
            Err(TrustedBlockProofAnchorError::ExecutedBlockWireMismatch)
        );
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn real_block_proofs_reject_wrong_count_and_commitment_substitution() {
        let (block, artifact, external_hash, _) = authenticated_block_with_scheduled_entry();
        let proofs = block
            .proofs_for_entry_hash(&external_hash)
            .expect("external proof exists");
        let anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
            &block,
            &artifact,
            &external_hash,
        )
        .expect("external anchor derives");
        assert!(proofs.verify(&anchor));
        let mut wrong_count = proofs.clone();
        wrong_count.entry_commitment = MerkleTreeCommitment::new(
            *proofs.entry_commitment.root(),
            NonZeroU64::new(proofs.entry_commitment.leaf_count().get() + 1)
                .expect("wrong count remains non-zero"),
        );
        assert!(
            !wrong_count.verify(&anchor),
            "the same root must not be rebound to a different entrypoint count"
        );
        let external_tree: MerkleTree<TransactionEntrypoint> = block
            .external_entrypoints_cloned()
            .map(|entrypoint| entrypoint.hash())
            .collect();
        let mut substituted_commitment = proofs;
        substituted_commitment.entry_commitment = external_tree
            .commitment()
            .expect("external entry commitment");
        assert!(
            !substituted_commitment.verify(&anchor),
            "the external-only commitment must not replace the executed-entry commitment"
        );
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn trusted_anchor_checks_full_entry_result_count_alignment_before_target_selection() {
        let (mut block, _, external_hash, _) = authenticated_block_with_scheduled_entry();
        let result_state = block.result.as_mut().expect("fixture has results");
        result_state.transaction_results.truncate(1);
        result_state.result_merkle = result_state
            .transaction_results
            .iter()
            .map(TransactionResult::hash)
            .collect();
        block.payload.header.result_merkle_root = result_state.result_merkle.root();
        let executed_block_wire = block
            .encode_wire()
            .expect("misaligned fixture wire still encodes");
        let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"misaligned proof parent state"),
            Hash::new(b"misaligned proof post state"),
            Hash::new(b"misaligned proof ordinary writes"),
            u64::try_from(executed_block_wire.len())
                .expect("misaligned fixture wire length fits u64"),
            Hash::new(&executed_block_wire),
        );
        let artifact = finalized_artifact_for_block(&block, &execution_commitment);
        assert_eq!(
            TrustedBlockProofAnchor::from_untrusted_finality_artifact(
                &block,
                &artifact,
                &external_hash,
            ),
            Err(TrustedBlockProofAnchorError::MisalignedLeafCounts)
        );
    }
    fn aligned_block_proofs_fixture() -> (BlockProofs, TrustedBlockProofAnchor) {
        let entries = [sample_entrypoint_hash(), sample_entrypoint_hash()];
        let entry_tree: MerkleTree<TransactionEntrypoint> = entries.into_iter().collect();
        let results = [
            HashOf::<TransactionResult>::from_untyped_unchecked(Hash::new(b"result zero")),
            HashOf::<TransactionResult>::from_untyped_unchecked(Hash::new(b"result one")),
        ];
        let result_tree: MerkleTree<TransactionResult> = results.into_iter().collect();
        let block_height = NonZeroU64::new(9).expect("non-zero height");
        let block_hash = HashOf::from_untyped_unchecked(Hash::new(b"trusted carrier block"));
        let executed_block_wire_hash = Hash::new(b"trusted executed block wire");
        let entry_commitment = entry_tree.commitment().expect("entry commitment");
        let result_commitment = result_tree.commitment().expect("result commitment");
        let proofs = BlockProofs {
            block_height,
            block_hash,
            executed_block_wire_hash,
            entry_hash: entries[0],
            entry_commitment,
            entry_proof: BlockReceiptProof::new(
                entries[0],
                entry_tree.get_proof(0).expect("entry proof"),
            ),
            result_commitment,
            result_proof: ExecutionReceiptProof::new(
                results[0],
                result_tree.get_proof(0).expect("result proof"),
            ),
            fastpq_transcripts: BTreeMap::new(),
        };
        let anchor = TrustedBlockProofAnchor {
            block_height,
            block_hash,
            executed_block_wire_hash,
            entry_hash: entries[0],
            entry_index: 0,
            entry_commitment,
            result_commitment,
            fastpq_transcripts: BTreeMap::new(),
        };
        (proofs, anchor)
    }
    #[test]
    fn block_proofs_require_separately_anchored_commitments() {
        let (mut proofs, anchor) = aligned_block_proofs_fixture();
        assert!(proofs.verify(&anchor));
        let forged_entry = sample_entrypoint_hash();
        let forged_tree: MerkleTree<TransactionEntrypoint> = [forged_entry].into_iter().collect();
        proofs.entry_hash = forged_entry;
        proofs.entry_commitment = forged_tree.commitment().expect("forged commitment");
        proofs.entry_proof = BlockReceiptProof::new(
            forged_entry,
            forged_tree.get_proof(0).expect("forged proof"),
        );
        assert!(proofs.entry_proof.verify(&proofs.entry_commitment));
        assert!(
            !proofs.verify(&anchor),
            "a self-consistent response commitment must not replace the trusted anchor"
        );
    }
    #[test]
    fn block_proofs_require_entry_and_result_indices_to_match() {
        let (mut proofs, anchor) = aligned_block_proofs_fixture();
        let result_hash =
            HashOf::<TransactionResult>::from_untyped_unchecked(Hash::new(b"result one"));
        let result_tree: MerkleTree<TransactionResult> = [
            HashOf::from_untyped_unchecked(Hash::new(b"result zero")),
            result_hash,
        ]
        .into_iter()
        .collect();
        proofs.result_proof = ExecutionReceiptProof::new(
            result_hash,
            result_tree.get_proof(1).expect("result proof"),
        );
        assert!(proofs.result_proof.verify(&proofs.result_commitment));
        assert!(
            !proofs.verify(&anchor),
            "individually valid proofs for different transaction indices must be rejected"
        );
    }
    #[test]
    fn block_proofs_require_entry_and_result_counts_to_match() {
        let (mut proofs, mut anchor) = aligned_block_proofs_fixture();
        let result_hash =
            HashOf::<TransactionResult>::from_untyped_unchecked(Hash::new(b"single result"));
        let result_tree: MerkleTree<TransactionResult> = [result_hash].into_iter().collect();
        let result_commitment = result_tree.commitment().expect("result commitment");
        proofs.result_commitment = result_commitment;
        proofs.result_proof = ExecutionReceiptProof::new(
            result_hash,
            result_tree.get_proof(0).expect("result proof"),
        );
        anchor.result_commitment = result_commitment;
        assert!(proofs.result_proof.verify(&anchor.result_commitment));
        assert!(
            !proofs.verify(&anchor),
            "separately valid entry and result trees must have one aligned leaf count"
        );
    }
    #[test]
    fn block_proofs_require_authenticated_fastpq_transcripts() {
        let (mut proofs, anchor) = aligned_block_proofs_fixture();
        proofs
            .fastpq_transcripts
            .insert(Hash::new(b"forged transcript key"), Vec::new());
        assert!(
            !proofs.verify(&anchor),
            "response transcripts must match the authenticated executed block projection"
        );
    }
}
