//! Finality-authenticated proofs over distinct network-input and typed-output trees.
//!
//! These types bundle the carrier identity, leaf hash, canonical audit path, and exact root/count
//! commitments required to verify inclusion without depending on internal structures. Block proof
//! responses use distinct complete network-input and typed-output trees. Internal invocations have
//! no synthetic input leaves. The exact executed block wire, and therefore both trees and their
//! explicit source join, is authenticated either by the certified execution result of a committed
//! Sumeragi block (its commit certificate's result preimage, read by `iroha_core`'s certified-chain
//! reader) or by the portable native verifier's `VerifiedSumeragiBlock`. `BlockHeader::merkle_root` is checked as
//! proposal metadata, but is never selected as the entry-proof anchor.
use crate::{
    block::execution_output::ExecutionOutputV1,
    block::{BlockHeader, SignedBlock},
    fastpq::TransferTranscript,
    sumeragi_finality::VerifiedSumeragiBlock,
    transaction::signed::TransactionEntrypoint,
};
use core::num::NonZeroU64;
use iroha_crypto::{Hash, HashOf, MerkleProof, MerkleTreeCommitment};
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
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    crate :: DeriveJsonSerialize,
    crate :: DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::block::proofs::BlockReceiptProof")]
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
/// Full typed output and its audit path under the sole `BlockResult` output tree.
/// Use a finality-authenticated anchor to establish the commitment's authority.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    crate :: DeriveJsonSerialize,
    crate :: DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::block::proofs::ExecutionReceiptProof")]
pub struct ExecutionReceiptProof {
    /// Exact source descriptor, result, receipts, and completions covered by the proof.
    output: ExecutionOutputV1,
    /// Canonical audit path leading to the output Merkle root.
    proof: MerkleProof<ExecutionOutputV1>,
}
impl ExecutionReceiptProof {
    /// Construct a proof from the full typed output and its audit path.
    #[must_use]
    pub const fn new(output: ExecutionOutputV1, proof: MerkleProof<ExecutionOutputV1>) -> Self {
        Self { output, proof }
    }
    /// Hash the full typed output covered by this proof.
    #[must_use]
    pub fn leaf(&self) -> HashOf<ExecutionOutputV1> {
        HashOf::new(&self.output)
    }
    /// Borrow the exact typed source/result row authenticated by this proof.
    pub const fn output(&self) -> &ExecutionOutputV1 {
        &self.output
    }
    /// Returns the underlying Merkle proof.
    #[must_use]
    pub const fn proof(&self) -> &MerkleProof<ExecutionOutputV1> {
        &self.proof
    }
    /// Verify the proof against the supplied root-and-leaf-count commitment.
    #[must_use]
    pub fn verify(&self, commitment: &MerkleTreeCommitment<ExecutionOutputV1>) -> bool {
        commitment.leaf_count().get() <= BLOCK_MERKLE_MAX_LEAF_COUNT
            && self.proof.verify(&self.leaf(), commitment)
    }
}
/// Complete network-input proof joined to a distinct full typed-output proof.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    crate :: DeriveJsonSerialize,
    crate :: DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::block::proofs::BlockProofs")]
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
    pub output_commitment: MerkleTreeCommitment<ExecutionOutputV1>,
    /// Full typed output proof; its `Network.input_index` explicitly joins the input proof.
    pub output_proof: ExecutionReceiptProof,
    /// Claimed FASTPQ transfer transcripts grouped by exact execution-call hash.
    pub fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>,
}
/// Trusted block identity, Merkle commitments, and executed transcript projection used to verify
/// [`BlockProofs`].
///
/// This capability is intentionally not serializable and its fields are private. Its public
/// constructors require either the executed-wire identity a certified-chain reader authenticated
/// for a committed block, or an independently trusted target height context and untrusted
/// Sumeragi-v2 finality; both bind the exact executed wire before recomputing the Merkle
/// commitments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedBlockProofAnchor {
    block_height: NonZeroU64,
    block_hash: HashOf<BlockHeader>,
    executed_block_wire_hash: Hash,
    entry_hash: HashOf<TransactionEntrypoint>,
    entry_index: u32,
    output_index: u32,
    output_hash: HashOf<ExecutionOutputV1>,
    entry_commitment: MerkleTreeCommitment<TransactionEntrypoint>,
    output_commitment: MerkleTreeCommitment<ExecutionOutputV1>,
    fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>,
}
/// Failure to derive a trusted proof anchor from authenticated block metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TrustedBlockProofAnchorError {
    /// Signed genesis alone does not authenticate its subsequently computed execution output.
    #[error("genesis execution needs independent node attestations or a certified successor")]
    UnauthenticatedGenesisExecution,
    /// The authenticated native decision belongs to another block header.
    #[error("verified native finality does not match the supplied block header")]
    FinalityHeaderMismatch,
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
    EntrypointNotFound,
    /// The authenticated entrypoint tree exceeds the block-proof index space.
    #[error("authenticated block entrypoint count exceeds the u32 proof index space")]
    TooManyEntrypoints,
    /// An output anchor requires a non-empty executed output tree.
    #[error("authenticated block has no execution outputs")]
    MissingResults,
    /// A submitted input has no typed Network output carrying its input index.
    #[error("authenticated input has no matching Network output")]
    MissingNetworkOutput,
    /// The requested output index is outside the authenticated output tree.
    #[error("requested output index {output_index} is absent from the authenticated block")]
    OutputNotFound {
        /// Output index requested by the proof consumer.
        output_index: u32,
    },
    /// The authenticated output tree exceeds the block-proof index space.
    #[error("authenticated block output count exceeds the u32 proof index space")]
    TooManyOutputs,
    /// Stored Merkle material disagrees with the authenticated block contents.
    #[error("authenticated block carries inconsistent Merkle material")]
    InconsistentMerkleMaterial,
}
impl TrustedBlockProofAnchor {
    /// Derive an entrypoint anchor from a native block authenticated by the portable verifier.
    ///
    /// The verified capability retains the independently selected genesis/checkpoint and complete
    /// epoch-chain authentication. The separate candidate block must reproduce that decision's
    /// exact header and executed wire; no caller-supplied context digest confers authority.
    ///
    /// # Errors
    /// Rejects a different header or executed wire, inconsistent output material, or absent entry.
    pub fn from_verified_finality(
        block: &SignedBlock,
        verified: &VerifiedSumeragiBlock,
        entry_hash: &HashOf<TransactionEntrypoint>,
    ) -> Result<Self, TrustedBlockProofAnchorError> {
        let authenticated = authenticate_verified_execution(block, verified)?;
        Self::from_authenticated_outputs(block, authenticated, entry_hash)
    }
    /// Derive a target-specific anchor for a committed block whose executed-wire identity was
    /// already authenticated by a certified-chain reader.
    ///
    /// `committed_wire_len` and `committed_wire_hash` must come from the block's certified
    /// execution result (the result preimage of its commit certificate, which the certified
    /// header chain or a verified `CommitQC` binds), never from the block itself. This binds
    /// the exact block to that identity (ignoring the node-local commit certificate the block may
    /// carry), validates the output cache in place and locates `entry_hash` in authenticated
    /// network-input order, as [`Self::from_verified_finality`] does.
    ///
    /// # Errors
    /// Returns [`TrustedBlockProofAnchorError`] when the block is not the committed wire or its
    /// Merkle material is missing or inconsistent, or the entry is absent.
    pub fn from_committed_execution(
        block: &SignedBlock,
        committed_wire_len: u64,
        committed_wire_hash: Hash,
        entry_hash: &HashOf<TransactionEntrypoint>,
    ) -> Result<Self, TrustedBlockProofAnchorError> {
        let authenticated =
            authenticate_executed_wire(block, committed_wire_len, committed_wire_hash)?;
        Self::from_authenticated_outputs(block, authenticated, entry_hash)
    }
    fn from_authenticated_outputs(
        block: &SignedBlock,
        (executed_block_wire_hash, output_commitment): (
            Hash,
            MerkleTreeCommitment<ExecutionOutputV1>,
        ),
        entry_hash: &HashOf<TransactionEntrypoint>,
    ) -> Result<Self, TrustedBlockProofAnchorError> {
        let full_entry_commitment = block
            .network_input_merkle_commitment()
            .ok_or(TrustedBlockProofAnchorError::MissingEntrypoints)?;
        if full_entry_commitment.leaf_count().get() > BLOCK_MERKLE_MAX_LEAF_COUNT {
            return Err(TrustedBlockProofAnchorError::TooManyEntrypoints);
        }
        let entry_index = block
            .network_input_hashes()
            .position(|candidate| &candidate == entry_hash)
            .ok_or(TrustedBlockProofAnchorError::EntrypointNotFound)?;
        let entry_index = u32::try_from(entry_index)
            .map_err(|_| TrustedBlockProofAnchorError::TooManyEntrypoints)?;
        let (output_index, _) = block
            .network_output_at(entry_index)
            .ok_or(TrustedBlockProofAnchorError::MissingNetworkOutput)?;
        let output_hash = HashOf::new(&block.execution_outputs()[output_index as usize]);
        Ok(Self {
            block_height: block.header().height(),
            block_hash: block.hash(),
            executed_block_wire_hash,
            entry_hash: *entry_hash,
            entry_index,
            output_index,
            output_hash,
            entry_commitment: full_entry_commitment,
            output_commitment,
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
    /// Return the anchored output-tree commitment, whose count may exceed the input count.
    #[must_use]
    pub const fn output_commitment(&self) -> MerkleTreeCommitment<ExecutionOutputV1> {
        self.output_commitment
    }
    /// Return the exact FASTPQ transcript projection authenticated by the executed block wire.
    #[must_use]
    pub fn fastpq_transcripts(&self) -> &BTreeMap<Hash, Vec<TransferTranscript>> {
        &self.fastpq_transcripts
    }
}
/// Target-specific authority for any typed output, including Pipeline and Time invocations.
///
/// This non-serializable capability has no input proof or synthetic transaction identity.
/// Its native constructor requires the verifier's authenticated block capability, then binds
/// the exact executed wire and all output cache material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustedExecutionOutputAnchor {
    block_height: NonZeroU64,
    block_hash: HashOf<BlockHeader>,
    executed_block_wire_hash: Hash,
    output_index: u32,
    output_hash: HashOf<ExecutionOutputV1>,
    output_commitment: MerkleTreeCommitment<ExecutionOutputV1>,
}

impl TrustedExecutionOutputAnchor {
    /// Bind one typed output to the native verifier's authenticated block capability.
    ///
    /// # Errors
    /// Rejects a different header, mismatched wire or output cache, or an absent output.
    pub fn from_verified_finality(
        block: &SignedBlock,
        verified: &VerifiedSumeragiBlock,
        output_index: u32,
    ) -> Result<Self, TrustedBlockProofAnchorError> {
        let authenticated = authenticate_verified_execution(block, verified)?;
        Self::from_authenticated_outputs(block, authenticated, output_index)
    }
    /// Authenticate the output at `output_index` of a committed block whose executed-wire
    /// identity a certified-chain reader already authenticated (see
    /// [`TrustedBlockProofAnchor::from_committed_execution`]).
    ///
    /// # Errors
    /// Returns an error for wire or cache mismatches, or an absent output.
    pub fn from_committed_execution(
        block: &SignedBlock,
        committed_wire_len: u64,
        committed_wire_hash: Hash,
        output_index: u32,
    ) -> Result<Self, TrustedBlockProofAnchorError> {
        let authenticated =
            authenticate_executed_wire(block, committed_wire_len, committed_wire_hash)?;
        Self::from_authenticated_outputs(block, authenticated, output_index)
    }
    fn from_authenticated_outputs(
        block: &SignedBlock,
        (executed_block_wire_hash, output_commitment): (
            Hash,
            MerkleTreeCommitment<ExecutionOutputV1>,
        ),
        output_index: u32,
    ) -> Result<Self, TrustedBlockProofAnchorError> {
        let output = block
            .execution_outputs()
            .get(output_index as usize)
            .ok_or(TrustedBlockProofAnchorError::OutputNotFound { output_index })?;
        Ok(Self {
            block_height: block.header().height(),
            block_hash: block.hash(),
            executed_block_wire_hash,
            output_index,
            output_hash: HashOf::new(output),
            output_commitment,
        })
    }

    /// Return the finalized block height.
    #[must_use]
    pub const fn block_height(&self) -> NonZeroU64 {
        self.block_height
    }
    /// Return the finalized proposal header hash.
    #[must_use]
    pub const fn block_hash(&self) -> HashOf<BlockHeader> {
        self.block_hash
    }
    /// Return the finalized exact executed-wire hash.
    #[must_use]
    pub const fn executed_block_wire_hash(&self) -> Hash {
        self.executed_block_wire_hash
    }
    /// Return the target's position in the complete typed-output sequence.
    #[must_use]
    pub const fn output_index(&self) -> u32 {
        self.output_index
    }
    /// Return the finalized output root and exact count.
    #[must_use]
    pub const fn output_commitment(&self) -> MerkleTreeCommitment<ExecutionOutputV1> {
        self.output_commitment
    }
    /// Verify a full output and audit path for this exact target.
    #[must_use]
    pub fn verify(&self, proof: &ExecutionReceiptProof) -> bool {
        proof.proof().leaf_index() == self.output_index
            && proof.leaf() == self.output_hash
            && proof.verify(&self.output_commitment)
    }
}

fn authenticate_verified_execution(
    block: &SignedBlock,
    verified: &VerifiedSumeragiBlock,
) -> Result<(Hash, MerkleTreeCommitment<ExecutionOutputV1>), TrustedBlockProofAnchorError> {
    if verified.height() <= 1 {
        return Err(TrustedBlockProofAnchorError::UnauthenticatedGenesisExecution);
    }
    if block.header() != verified.header() {
        return Err(TrustedBlockProofAnchorError::FinalityHeaderMismatch);
    }
    let commitment = verified.execution();
    authenticate_executed_wire(
        block,
        commitment.executed_block_wire_len,
        commitment.executed_block_wire_hash,
    )
}

/// Bind `block` to the executed-wire identity its certified result commits to, then return its
/// wire hash and validated output commitment.
fn authenticate_executed_wire(
    block: &SignedBlock,
    committed_len: u64,
    committed_hash: Hash,
) -> Result<(Hash, MerkleTreeCommitment<ExecutionOutputV1>), TrustedBlockProofAnchorError> {
    let (wire_len, executed_block_wire_hash) = block
        .executed_block_wire_identity()
        .map_err(|_| TrustedBlockProofAnchorError::ExecutedBlockWireEncoding)?;
    if executed_block_wire_hash != committed_hash || wire_len != committed_len {
        return Err(TrustedBlockProofAnchorError::ExecutedBlockWireMismatch);
    }
    if !block.has_results() {
        return Err(TrustedBlockProofAnchorError::MissingResults);
    }
    block
        .validate_output_merkle_cache()
        .map_err(|_| TrustedBlockProofAnchorError::InconsistentMerkleMaterial)?;
    let outputs = block
        .output_merkle_commitment()
        .ok_or(TrustedBlockProofAnchorError::MissingResults)?;
    if outputs.leaf_count().get() > BLOCK_MERKLE_MAX_LEAF_COUNT {
        return Err(TrustedBlockProofAnchorError::TooManyOutputs);
    }
    Ok((executed_block_wire_hash, outputs))
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
            || self.entry_hash != *self.entry_proof.leaf()
            || !self.entry_proof.verify(&anchor.entry_commitment)
            || self.output_commitment != anchor.output_commitment
            || self.output_proof.proof().leaf_index() != anchor.output_index
            || self.output_proof.leaf() != anchor.output_hash
            || !matches!(self.output_proof.output(), ExecutionOutputV1::Network(row) if row.input_index == anchor.entry_index)
            || !self.output_proof.verify(&anchor.output_commitment)
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
        transaction::{TransactionResultInner, signed::TransactionBuilder},
    };
    use iroha_crypto::{Hash, HashOf, KeyPair, MerkleTree};
    use iroha_model_base::domain::DomainId;
    use norito::codec::DecodeAll as _;
    use std::iter::FromIterator;
    fn sample_output(index: u32) -> ExecutionOutputV1 {
        super::super::output_test_support::network(
            index,
            TransactionResultInner::Ok(crate::trigger::DataTriggerSequence::default()),
        )
    }
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
    fn execution_receipt_proof_verifies_against_full_output_merkle_root() {
        let output = sample_output(0);
        let tree = MerkleTree::from_iter([HashOf::new(&output)]);
        let proof = tree.get_proof(0).expect("proof must exist for result leaf");
        let execution = ExecutionReceiptProof::new(output, proof);
        let commitment = tree.commitment().expect("commitment must exist");
        assert!(
            execution.verify(&commitment),
            "execution proof must verify against commitment"
        );
    }
    #[test]
    fn execution_receipt_proof_rejects_wrong_root() {
        let output = sample_output(0);
        let tree = MerkleTree::from_iter([HashOf::new(&output)]);
        let proof = tree.get_proof(0).expect("proof must exist for result leaf");
        let execution = ExecutionReceiptProof::new(output, proof);
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
        let output = sample_output(0);
        let result_tree = MerkleTree::from_iter([HashOf::new(&output)]);
        let output_commitment = result_tree.commitment().expect("result commitment");
        let output_proof =
            ExecutionReceiptProof::new(output, result_tree.get_proof(0).expect("result proof"));
        let proofs = BlockProofs {
            block_height: NonZeroU64::new(7).expect("block height must be non-zero"),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"carrier block")),
            executed_block_wire_hash: Hash::new(b"executed block wire"),
            entry_hash,
            entry_commitment,
            entry_proof,
            output_commitment,
            output_proof,
            fastpq_transcripts: BTreeMap::new(),
        };
        let encoded = proofs.encode();
        let decoded = BlockProofs::decode_all(&mut encoded.as_slice())
            .expect("canonical block proofs must decode");
        assert_eq!(decoded, proofs);
    }
    #[cfg(feature = "transparent_api")]
    fn authenticated_block_with_internal_output() -> (
        SignedBlock,
        VerifiedSumeragiBlock,
        HashOf<TransactionEntrypoint>,
        u32,
    ) {
        let native = crate::sumeragi_finality::tests::Fixture::new();
        let keypair = checked_random_keypair();
        let authority = AccountId::new(keypair.public_key().clone());
        let header = native.next_header();
        let mut builder = crate::block::builder::BlockBuilder::new(header);
        for index in 0..2 {
            let mut tx = TransactionBuilder::new(
                native.network,
                authority.clone(),
                crate::transaction::FeePaymentIntent::authority(Vec::new(), None),
            );
            tx.set_creation_time(std::time::Duration::from_millis(900 + index));
            builder.push_transaction(
                tx.try_sign(keypair.private_key())
                    .expect("fixture transaction"),
            );
        }
        let mut block = builder.build_with_signature(0, keypair.private_key());
        let external_hash = block.network_input_hashes().next().expect("network input");
        let timer = super::super::output_test_support::simple_time(&block, 0);
        super::super::output_test_support::install(
            &mut block,
            vec![sample_output(0), sample_output(1), timer],
            0,
        )
        .expect("valid full outputs");
        let verified = native.verify_block(block.clone());
        (block, verified, external_hash, 2)
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn finalized_retail_activation_authenticates_only_approved_historical_event() {
        use crate::{
            asset::{
                AssetBalancePolicy, AssetDefinition, AssetDefinitionId, RetailDailyLimitPolicyV1,
            },
            block::retail_activation_proof::{
                RetailActivationProofError, verify_finalized_retail_activation_v1,
            },
            isi::retail_daily_limit::ActivateRetailDailyLimitV1,
        };
        use iroha_model_base::topology::DataSpaceId;
        use iroha_primitives::numeric::{NumericSpec, Quantity};
        use std::collections::BTreeSet;

        let native = crate::sumeragi_finality::tests::Fixture::new();
        let owner_key = checked_random_keypair();
        let owner = AccountId::new(owner_key.public_key().clone());
        let reserve = AccountId::new(checked_random_keypair().public_key().clone());
        let domain = DomainId::try_new("retail", "bpng").expect("fixture domain");
        let definition_id = AssetDefinitionId::derive_from_components(
            domain.clone(),
            "kina".parse().expect("fixture asset name"),
        );
        let dataspace = DataSpaceId::new(7);
        let policy = RetailDailyLimitPolicyV1 {
            asset_definition_id: definition_id.clone(),
            physical_dataspace: dataspace,
            revision: 1,
            daily_cap: Quantity::from(5_u32),
            identity_issuer: owner.clone(),
            identity_issuer_public_key: owner_key.public_key().clone(),
            monetary_issuer_account: owner.clone(),
            reserve_account: reserve,
            institutional_exceptions: BTreeSet::new(),
        };
        let instruction = ActivateRetailDailyLimitV1 {
            definition: AssetDefinition::new(
                definition_id.clone(),
                "Kina".to_owned(),
                NumericSpec::fractional(2),
                AssetBalancePolicy::DataspaceRestricted,
                Some(domain.clone()),
            ),
            policy: policy.clone(),
        };
        // The native fixture signs this block with its authenticated four-validator
        // prefix; business owner and policy expectations remain independently selected.
        let header = native.next_header();
        let mut builder = crate::block::builder::BlockBuilder::new(header);
        builder.push_transaction(
            TransactionBuilder::new(
                native.network,
                owner.clone(),
                crate::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([instruction])
            .try_sign(owner_key.private_key())
            .expect("fixture signed activation"),
        );
        let mut block = builder.build_with_signature(0, owner_key.private_key());
        let entry_hash = block
            .network_input_hashes()
            .next()
            .expect("activation input");
        super::super::output_test_support::install(&mut block, vec![sample_output(0)], 0)
            .expect("fixture success output");
        let finalized = native.verify_block(block.clone());
        let verified = verify_finalized_retail_activation_v1(
            &block,
            &finalized,
            entry_hash,
            &owner,
            &policy,
            &definition_id,
            &domain,
            dataspace,
        )
        .expect("test-finalized activation with independently selected fixture coordinates");
        assert_eq!(verified.policy, policy);
        assert_eq!(
            verified.activation.activated_at_ms,
            block.header().creation_time_ms
        );
        assert_eq!(
            verified.activation.enforce_from_day_start_ms,
            (block.header().creation_time_ms / 86_400_000 + 1) * 86_400_000
        );
        assert_eq!(verified.block_hash, block.hash());
        assert_eq!(verified.entry_hash, entry_hash);

        let wrong_owner = AccountId::new(checked_random_keypair().public_key().clone());
        assert_eq!(
            verify_finalized_retail_activation_v1(
                &block,
                &finalized,
                entry_hash,
                &wrong_owner,
                &policy,
                &definition_id,
                &domain,
                dataspace,
            ),
            Err(RetailActivationProofError::WrongOwner)
        );
        let wrong_domain = DomainId::try_new("elsewhere", "bpng").unwrap();
        assert_eq!(
            verify_finalized_retail_activation_v1(
                &block,
                &finalized,
                entry_hash,
                &owner,
                &policy,
                &definition_id,
                &wrong_domain,
                dataspace,
            ),
            Err(RetailActivationProofError::WrongPolicy)
        );
        let mut wrong_policy = policy.clone();
        wrong_policy.daily_cap = Quantity::from(6_u32);
        assert_eq!(
            verify_finalized_retail_activation_v1(
                &block,
                &finalized,
                entry_hash,
                &owner,
                &wrong_policy,
                &definition_id,
                &domain,
                dataspace,
            ),
            Err(RetailActivationProofError::WrongPolicy)
        );
        // Invalid certificates are refused before a VerifiedSumeragiBlock exists;
        // a_self_consistent_candidate_cannot_create_a_native_anchor_without_valid_qc covers it.
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn a_foreign_roster_cannot_create_a_native_anchor_capability() {
        use iroha_crypto::{Algorithm, bls_normal_pop_prove};
        let native = crate::sumeragi_finality::tests::Fixture::new();
        let mut proof = native.second.clone();
        let foreign = KeyPair::from_seed(vec![99; 32], Algorithm::BlsNormal);
        proof.committee[0].public_key = foreign.public_key().clone();
        proof.committee[0].proof_of_possession =
            bls_normal_pop_prove(foreign.private_key()).unwrap();
        let mut verifier = native.verifier();
        verifier.verify(&native.first).unwrap();
        assert!(
            verifier
                .verify(&proof)
                .unwrap_err()
                .to_string()
                .contains("proof roster differs from its complete epoch context")
        );
        // Both anchor constructors require VerifiedSumeragiBlock, which a structural
        // decode or a response-supplied committee cannot create.
    }

    #[cfg(feature = "transparent_api")]
    #[test]
    fn committed_execution_anchors_bind_the_certificate_free_executed_wire() {
        let (block, _, external_hash, internal_index) = authenticated_block_with_internal_output();
        let wire = block.encode_wire().expect("fixture wire");
        let (len, hash) = (wire.len() as u64, Hash::new(&wire));
        // A node-local commit certificate does not change the committed identity.
        let stored = block.clone().with_commit_certificate(Some(
            crate::block::CommitCertificate::from_untrusted_parts(vec![1], vec![2], vec![3]),
        ));
        for candidate in [&block, &stored] {
            let anchor = TrustedBlockProofAnchor::from_committed_execution(
                candidate,
                len,
                hash,
                &external_hash,
            )
            .expect("committed anchor");
            assert_eq!(anchor.executed_block_wire_hash(), hash);
            assert_eq!(anchor.entry_index(), 0);
            assert!(
                block
                    .network_execution_proof(&external_hash)
                    .expect("proof")
                    .verify(&anchor)
            );
            let output = TrustedExecutionOutputAnchor::from_committed_execution(
                candidate,
                len,
                hash,
                internal_index,
            )
            .expect("committed output anchor");
            assert_eq!(output.output_index(), internal_index);
        }
        for (bad_len, bad_hash) in [(len + 1, hash), (len, Hash::new(b"another executed wire"))] {
            assert_eq!(
                TrustedBlockProofAnchor::from_committed_execution(
                    &block,
                    bad_len,
                    bad_hash,
                    &external_hash
                ),
                Err(TrustedBlockProofAnchorError::ExecutedBlockWireMismatch)
            );
            assert_eq!(
                TrustedExecutionOutputAnchor::from_committed_execution(
                    &block, bad_len, bad_hash, 0
                ),
                Err(TrustedBlockProofAnchorError::ExecutedBlockWireMismatch)
            );
        }
        assert!(matches!(
            TrustedBlockProofAnchor::from_committed_execution(
                &block,
                len,
                hash,
                &sample_entrypoint_hash()
            ),
            Err(TrustedBlockProofAnchorError::EntrypointNotFound)
        ));
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn trusted_anchor_accepts_real_finality_with_distinct_input_and_output_counts() {
        let (block, artifact, external_hash, _) = authenticated_block_with_internal_output();
        let proofs = block
            .network_execution_proof(&external_hash)
            .expect("external proof exists");
        let anchor =
            TrustedBlockProofAnchor::from_verified_finality(&block, &artifact, &external_hash)
                .expect("external anchor derives");
        assert_eq!(anchor.entry_hash(), external_hash);
        assert_eq!(anchor.entry_index(), 0);
        assert_eq!(anchor.entry_commitment(), proofs.entry_commitment);
        assert_eq!(
            anchor.entry_commitment(),
            block
                .network_input_merkle_commitment()
                .expect("full entry commitment")
        );
        assert_eq!(anchor.entry_commitment().leaf_count().get(), 2);
        assert_eq!(anchor.output_commitment().leaf_count().get(), 3);
        assert_eq!(
            anchor.entry_commitment().root(),
            &block.header().merkle_root().unwrap()
        );
        assert_eq!(anchor.fastpq_transcripts(), block.fastpq_transcripts());
        assert!(proofs.verify(&anchor));
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn trusted_anchor_authenticates_internal_output_without_an_input_leaf() {
        let (block, artifact, _, index) = authenticated_block_with_internal_output();
        let proof = ExecutionReceiptProof::new(
            block.execution_outputs()[index as usize].clone(),
            block.output_proof(index).unwrap(),
        );
        let anchor = TrustedExecutionOutputAnchor::from_verified_finality(&block, &artifact, index)
            .expect("internal output anchor");
        assert_eq!(anchor.output_index(), index);
        assert_eq!(anchor.block_height(), block.header().height());
        assert_eq!(anchor.block_hash(), block.hash());
        assert_eq!(
            anchor.executed_block_wire_hash(),
            block.executed_block_wire_hash().unwrap()
        );
        assert_eq!(anchor.output_commitment().leaf_count().get(), 3);
        assert!(matches!(proof.output(), ExecutionOutputV1::Time(_)));
        assert!(anchor.verify(&proof));
        let mut substituted = proof.clone();
        if let ExecutionOutputV1::Time(row) = &mut substituted.output {
            row.invocation.schedule_index += 1;
        }
        assert!(!anchor.verify(&substituted));
        let other = ExecutionReceiptProof::new(
            block.execution_outputs()[0].clone(),
            block.output_proof(0).unwrap(),
        );
        assert!(other.verify(&anchor.output_commitment()));
        assert!(!anchor.verify(&other));
        assert_eq!(
            TrustedExecutionOutputAnchor::from_verified_finality(&block, &artifact, 3),
            Err(TrustedBlockProofAnchorError::OutputNotFound { output_index: 3 })
        );
        let fake_input = HashOf::from_untyped_unchecked(proof.leaf().into());
        assert!(block.network_execution_proof(&fake_input).is_none());
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn trusted_anchor_rejects_unknown_or_substituted_target() {
        let (block, artifact, external_hash, _) = authenticated_block_with_internal_output();
        let other_input_hash = block.network_input_hashes().nth(1).unwrap();
        let external_anchor =
            TrustedBlockProofAnchor::from_verified_finality(&block, &artifact, &external_hash)
                .expect("external anchor derives");
        let other_proofs = block
            .network_execution_proof(&other_input_hash)
            .expect("second network proof exists");
        assert!(
            !other_proofs.verify(&external_anchor),
            "a valid proof for another entrypoint in the same tree must not satisfy the target anchor"
        );
        let missing_hash = HashOf::from_untyped_unchecked(Hash::new(b"missing entrypoint"));
        assert_eq!(
            TrustedBlockProofAnchor::from_verified_finality(&block, &artifact, &missing_hash,),
            Err(TrustedBlockProofAnchorError::EntrypointNotFound)
        );
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn a_self_consistent_candidate_cannot_create_a_native_anchor_without_valid_qc() {
        let native = crate::sumeragi_finality::tests::Fixture::new();
        let mut proof = native.second.clone();
        let mut block = crate::block::decode_versioned_signed_block(&proof.block_wire).unwrap();
        let certificate = block.commit_certificate().unwrap();
        let mut qc: iroha_sumeragi::message::Qc =
            norito::decode_canonical(certificate.commit_qc()).unwrap();
        qc.agg_sig.0[0] ^= 0x80;
        block.set_commit_certificate(Some(crate::block::CommitCertificate::from_untrusted_parts(
            certificate.consensus_header().to_vec(),
            norito::encode_canonical(&qc).unwrap(),
            certificate.result_preimage().to_vec(),
        )));
        proof.block_wire = block.encode_wire().unwrap();
        let mut verifier = native.verifier();
        verifier.verify(&native.first).unwrap();
        assert!(verifier.verify(&proof).is_err());
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn trusted_anchor_rejects_valid_finality_for_another_header() {
        let (block, artifact, _, _) = authenticated_block_with_internal_output();
        let (other_block, _, other_external_hash, _) = authenticated_block_with_internal_output();
        assert_eq!(
            TrustedBlockProofAnchor::from_verified_finality(
                &other_block,
                &artifact,
                &other_external_hash,
            ),
            Err(TrustedBlockProofAnchorError::FinalityHeaderMismatch)
        );
        assert_ne!(block.hash(), other_block.hash());
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn committed_wire_binding_rejects_wrong_executed_wire() {
        let (block, _, external_hash, _) = authenticated_block_with_internal_output();
        let wrong_executed_block_wire = b"different finalized executed block wire";
        let wrong_execution_commitment = (
            wrong_executed_block_wire.len() as u64,
            Hash::new(wrong_executed_block_wire),
        );
        let artifact = wrong_execution_commitment;
        assert_eq!(
            TrustedBlockProofAnchor::from_committed_execution(
                &block,
                artifact.0,
                artifact.1,
                &external_hash,
            ),
            Err(TrustedBlockProofAnchorError::ExecutedBlockWireMismatch)
        );
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn real_block_proofs_reject_wrong_count_and_commitment_substitution() {
        let (block, artifact, external_hash, _) = authenticated_block_with_internal_output();
        let proofs = block
            .network_execution_proof(&external_hash)
            .expect("external proof exists");
        let anchor =
            TrustedBlockProofAnchor::from_verified_finality(&block, &artifact, &external_hash)
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
        let other_tree: MerkleTree<TransactionEntrypoint> = [external_hash].into_iter().collect();
        let mut substituted_commitment = proofs;
        substituted_commitment.entry_commitment = other_tree.commitment().unwrap();
        assert!(
            !substituted_commitment.verify(&anchor),
            "a subset input commitment must not replace the full authenticated input tree"
        );
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn trusted_anchor_checks_network_output_join_before_target_selection() {
        let (mut block, _, external_hash, _) = authenticated_block_with_internal_output();
        let result = block.result.as_mut().unwrap();
        result.outputs.remove(1);
        result.output_merkle = result.outputs.iter().map(HashOf::new).collect();
        let wire = block.encode_wire().unwrap();
        let commitment = (wire.len() as u64, Hash::new(&wire));
        let artifact = commitment;
        assert_eq!(
            TrustedBlockProofAnchor::from_committed_execution(
                &block,
                artifact.0,
                artifact.1,
                &external_hash
            ),
            Err(TrustedBlockProofAnchorError::InconsistentMerkleMaterial)
        );
        assert_eq!(
            TrustedExecutionOutputAnchor::from_committed_execution(
                &block, artifact.0, artifact.1, 0
            ),
            Err(TrustedBlockProofAnchorError::InconsistentMerkleMaterial)
        );
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn committed_wire_binding_requires_exact_length() {
        let (block, valid, input, index) = authenticated_block_with_internal_output();
        let mut commitment = *valid.execution();
        commitment.executed_block_wire_len += 1;
        let artifact = commitment;
        assert_eq!(
            TrustedBlockProofAnchor::from_committed_execution(
                &block,
                artifact.executed_block_wire_len,
                artifact.executed_block_wire_hash,
                &input
            ),
            Err(TrustedBlockProofAnchorError::ExecutedBlockWireMismatch)
        );
        assert_eq!(
            TrustedExecutionOutputAnchor::from_committed_execution(
                &block,
                artifact.executed_block_wire_len,
                artifact.executed_block_wire_hash,
                index
            ),
            Err(TrustedBlockProofAnchorError::ExecutedBlockWireMismatch)
        );
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn committed_pipeline_output_binding_needs_no_synthetic_transaction() {
        use crate::block::execution_output::{
            PipelineEventPositionV1, PipelineExecutionOutputV1, PipelineInvocationV1,
        };
        let (mut block, _, _, index) = authenticated_block_with_internal_output();
        let result = block.result.as_mut().unwrap();
        let ExecutionOutputV1::Time(timer) = result.outputs[index as usize].clone() else {
            panic!("timer fixture")
        };
        result.outputs[index as usize] = ExecutionOutputV1::Pipeline(PipelineExecutionOutputV1 {
            invocation: PipelineInvocationV1 {
                event: PipelineEventPositionV1::BlockApproved,
                candidate_index: 0,
                trigger: timer.invocation.trigger,
            },
            result: timer.result,
            failure_root: timer.failure_root,
            completions: timer.completions,
        });
        result.output_merkle = result.outputs.iter().map(HashOf::new).collect();
        block
            .validate_output_merkle_cache()
            .expect("valid Pipeline output");
        let wire = block.encode_wire().unwrap();
        let commitment = (wire.len() as u64, Hash::new(&wire));
        let artifact = commitment;
        let anchor = TrustedExecutionOutputAnchor::from_committed_execution(
            &block, artifact.0, artifact.1, index,
        )
        .unwrap();
        let proof = ExecutionReceiptProof::new(
            block.execution_outputs()[index as usize].clone(),
            block.output_proof(index).unwrap(),
        );
        assert!(anchor.verify(&proof));
        assert!(matches!(proof.output(), ExecutionOutputV1::Pipeline(_)));
        assert_eq!(block.network_input_hashes().len(), 2);
        assert_eq!(anchor.output_commitment().leaf_count().get(), 3);
    }

    #[cfg(feature = "transparent_api")]
    #[test]
    fn both_anchor_types_reject_a_committed_stale_output_cache() {
        let (mut block, _, input, index) = authenticated_block_with_internal_output();
        let result = block.result.as_mut().unwrap();
        let ExecutionOutputV1::Time(timer) = &mut result.outputs[index as usize] else {
            panic!("timer fixture")
        };
        timer.invocation.trigger.action_hash =
            Hash::new(b"substituted action with stale output cache");
        let wire = block.encode_wire().unwrap();
        let commitment = (wire.len() as u64, Hash::new(&wire));
        let artifact = commitment;
        assert_eq!(
            TrustedBlockProofAnchor::from_committed_execution(
                &block, artifact.0, artifact.1, &input
            ),
            Err(TrustedBlockProofAnchorError::InconsistentMerkleMaterial)
        );
        assert_eq!(
            TrustedExecutionOutputAnchor::from_committed_execution(
                &block, artifact.0, artifact.1, index
            ),
            Err(TrustedBlockProofAnchorError::InconsistentMerkleMaterial)
        );
    }

    #[cfg(feature = "transparent_api")]
    #[test]
    fn internal_native_output_anchor_requires_exact_header_and_wire() {
        let (block, verified, _, index) = authenticated_block_with_internal_output();
        let (other, _, _, _) = authenticated_block_with_internal_output();
        assert!(matches!(
            TrustedExecutionOutputAnchor::from_verified_finality(&other, &verified, index),
            Err(TrustedBlockProofAnchorError::FinalityHeaderMismatch)
        ));
        let commitment = verified.execution();
        assert_eq!(
            TrustedExecutionOutputAnchor::from_committed_execution(
                &block,
                commitment.executed_block_wire_len,
                Hash::new(b"another wire"),
                index,
            ),
            Err(TrustedBlockProofAnchorError::ExecutedBlockWireMismatch)
        );
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn native_capability_rejects_substituted_execution_with_the_same_header() {
        let (mut block, verified, input, index) = authenticated_block_with_internal_output();
        let result = block.result.as_mut().unwrap();
        let ExecutionOutputV1::Time(timer) = &mut result.outputs[index as usize] else {
            panic!("time fixture")
        };
        timer.invocation.trigger.action_hash = Hash::new(b"different certified output");
        result.output_merkle = result.outputs.iter().map(HashOf::new).collect();
        block.validate_output_merkle_cache().unwrap();
        assert_eq!(block.header(), verified.header());
        assert_eq!(
            TrustedBlockProofAnchor::from_verified_finality(&block, &verified, &input),
            Err(TrustedBlockProofAnchorError::ExecutedBlockWireMismatch)
        );
        assert_eq!(
            TrustedExecutionOutputAnchor::from_verified_finality(&block, &verified, index),
            Err(TrustedBlockProofAnchorError::ExecutedBlockWireMismatch)
        );
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn signed_genesis_alone_cannot_mint_execution_anchors() {
        let fixture = crate::sumeragi_finality::tests::Fixture::new();
        let verified = fixture.verifier().verify(&fixture.first).unwrap();
        let block = verified.block();
        let input = block.network_input_hashes().next().unwrap();
        assert_eq!(
            TrustedBlockProofAnchor::from_verified_finality(block, &verified, &input),
            Err(TrustedBlockProofAnchorError::UnauthenticatedGenesisExecution)
        );
        assert_eq!(
            TrustedExecutionOutputAnchor::from_verified_finality(block, &verified, 0),
            Err(TrustedBlockProofAnchorError::UnauthenticatedGenesisExecution)
        );
    }
    fn aligned_block_proofs_fixture() -> (BlockProofs, TrustedBlockProofAnchor) {
        let entries = [sample_entrypoint_hash(), sample_entrypoint_hash()];
        let entry_tree: MerkleTree<TransactionEntrypoint> = entries.into_iter().collect();
        let outputs = [sample_output(0), sample_output(1)];
        let output_tree: MerkleTree<ExecutionOutputV1> = outputs.iter().map(HashOf::new).collect();
        let block_height = NonZeroU64::new(9).unwrap();
        let block_hash = HashOf::from_untyped_unchecked(Hash::new(b"trusted carrier block"));
        let executed_block_wire_hash = Hash::new(b"trusted executed block wire");
        let entry_commitment = entry_tree.commitment().unwrap();
        let output_commitment = output_tree.commitment().unwrap();
        let proofs = BlockProofs {
            block_height,
            block_hash,
            executed_block_wire_hash,
            entry_hash: entries[0],
            entry_commitment,
            entry_proof: BlockReceiptProof::new(entries[0], entry_tree.get_proof(0).unwrap()),
            output_commitment,
            output_proof: ExecutionReceiptProof::new(
                outputs[0].clone(),
                output_tree.get_proof(0).unwrap(),
            ),
            fastpq_transcripts: BTreeMap::new(),
        };
        let anchor = TrustedBlockProofAnchor {
            block_height,
            block_hash,
            executed_block_wire_hash,
            entry_hash: entries[0],
            entry_index: 0,
            output_index: 0,
            output_hash: HashOf::new(&outputs[0]),
            entry_commitment,
            output_commitment,
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
    fn block_proofs_require_the_anchored_network_output_index() {
        let (mut proofs, anchor) = aligned_block_proofs_fixture();
        let outputs = [sample_output(0), sample_output(1)];
        let tree: MerkleTree<ExecutionOutputV1> = outputs.iter().map(HashOf::new).collect();
        proofs.output_proof =
            ExecutionReceiptProof::new(outputs[1].clone(), tree.get_proof(1).unwrap());
        assert!(proofs.output_proof.verify(&proofs.output_commitment));
        assert!(
            !proofs.verify(&anchor),
            "another valid Network output must not satisfy the target"
        );
    }
    #[test]
    fn block_proofs_require_the_explicit_network_input_join() {
        let (mut proofs, mut anchor) = aligned_block_proofs_fixture();
        let output = sample_output(1);
        let tree: MerkleTree<ExecutionOutputV1> = [HashOf::new(&output)].into_iter().collect();
        proofs.output_commitment = tree.commitment().unwrap();
        proofs.output_proof =
            ExecutionReceiptProof::new(output.clone(), tree.get_proof(0).unwrap());
        anchor.output_commitment = proofs.output_commitment;
        anchor.output_hash = HashOf::new(&output);
        assert!(proofs.output_proof.verify(&anchor.output_commitment));
        assert!(
            !proofs.verify(&anchor),
            "row input_index must bind the independently proven input"
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

#[cfg(test)]
mod captured_proofs_schema_tests;
