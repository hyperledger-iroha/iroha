//! Stable wrappers for Merkle proofs over block entrypoints and execution results.
//!
//! These types bundle the leaf hash, canonical audit path, and the Merkle roots
//! required to verify inclusion without depending on internal structures. The
//! entrypoint root matches `BlockHeader::merkle_root` for externally submitted
//! transactions and the extended entrypoint root captured after time-trigger
//! execution for scheduled entrypoints.

use core::num::NonZeroU64;
use std::collections::BTreeMap;

use iroha_crypto::{Hash, HashOf, MerkleProof, MerkleTree};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Maximum audit path length permitted for block-level Merkle proofs.
///
/// A depth of 32 supports over four billion leaves while still boundingly
/// protecting verifiers from pathological proofs carrying huge audit paths.
const BLOCK_MERKLE_MAX_HEIGHT: usize = 32;

use crate::{
    fastpq::TransferTranscript,
    transaction::signed::{TransactionEntrypoint, TransactionResult},
};

/// Merkle inclusion proof for a transaction entrypoint referenced by
/// `BlockHeader::merkle_root`.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct BlockReceiptProof {
    /// Hash of the transaction entrypoint proven to be part of the block.
    leaf: HashOf<TransactionEntrypoint>,
    /// Canonical audit path leading to the block entrypoint Merkle root.
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

    /// Verify the proof against the supplied Merkle root.
    #[must_use]
    pub fn verify(&self, root: &HashOf<MerkleTree<TransactionEntrypoint>>) -> bool {
        self.proof
            .clone()
            .verify(&self.leaf, root, BLOCK_MERKLE_MAX_HEIGHT)
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

    /// Verify the proof against the supplied Merkle root.
    #[must_use]
    pub fn verify(&self, root: &HashOf<MerkleTree<TransactionResult>>) -> bool {
        self.proof
            .clone()
            .verify(&self.leaf, root, BLOCK_MERKLE_MAX_HEIGHT)
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
    /// Hash of the transaction entrypoint proven to exist in the block.
    pub entry_hash: HashOf<TransactionEntrypoint>,
    /// Merkle root used to verify the entrypoint proof.
    pub entry_root: HashOf<MerkleTree<TransactionEntrypoint>>,
    /// Merkle proof for the entrypoint under `BlockHeader::merkle_root`.
    pub entry_proof: BlockReceiptProof,
    /// Merkle root used to verify the execution proof when present.
    pub result_root: Option<HashOf<MerkleTree<TransactionResult>>>,
    /// Optional execution result proof under `BlockHeader::result_merkle_root`.
    pub result_proof: Option<ExecutionReceiptProof>,
    /// FASTPQ transfer transcripts grouped by transaction entrypoint hash.
    pub fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>,
}

#[cfg(test)]
mod tests {
    use std::iter::FromIterator;

    use iroha_crypto::{HashOf, KeyPair, MerkleTree};

    use super::*;
    use crate::{
        account::AccountId,
        domain::DomainId,
        transaction::{
            signed::{TransactionBuilder, TransactionResult},
            TransactionResultInner,
        },
        ChainId,
    };

    fn sample_entrypoint_hash() -> HashOf<TransactionEntrypoint> {
        let keypair = KeyPair::random();
        let chain: ChainId = "proof-chain".parse().expect("chain id");
        let _domain: DomainId = "wonderland".parse().expect("domain id");
        let authority = AccountId::new(keypair.public_key().clone());
        let tx = TransactionBuilder::new(chain, authority).sign(keypair.private_key());
        tx.hash_as_entrypoint()
    }

    #[test]
    fn block_receipt_proof_verifies_against_merkle_root() {
        let hash = sample_entrypoint_hash();
        let tree = MerkleTree::from_iter([hash]);

        let proof = tree.get_proof(0).expect("proof must exist for single leaf");
        let receipt = BlockReceiptProof::new(hash, proof);

        let root = tree.root().expect("root must exist");
        assert!(receipt.verify(&root), "proof must verify against root");
    }

    #[test]
    fn block_receipt_proof_rejects_mutated_leaf() {
        let tree = MerkleTree::from_iter([sample_entrypoint_hash()]);
        let proof = tree.get_proof(0).expect("proof must exist for single leaf");
        let forged =
            BlockReceiptProof::new(HashOf::from_untyped_unchecked(Hash::new([0xAA; 32])), proof);

        let root = tree.root().expect("root must exist");
        assert!(
            !forged.verify(&root),
            "tampered leaf hash should not verify against root"
        );
    }

    #[test]
    fn block_receipt_proof_rejects_wrong_root() {
        let first = sample_entrypoint_hash();
        let tree = MerkleTree::from_iter([first]);
        let proof = tree.get_proof(0).expect("proof must exist for first leaf");
        let receipt = BlockReceiptProof::new(first, proof);

        let wrong_root = HashOf::from_untyped_unchecked(Hash::new([0xBB; 32]));
        assert!(
            !receipt.verify(&wrong_root),
            "proof should fail when verified against a different root"
        );
    }

    #[test]
    fn execution_receipt_proof_verifies_against_result_merkle_root() {
        let sequence = TransactionResultInner::Ok(crate::trigger::DataTriggerSequence::default());
        let result_hash = TransactionResult::hash_from_inner(&sequence);
        let tree = MerkleTree::from_iter([result_hash]);
        let proof = tree.get_proof(0).expect("proof must exist for result leaf");
        let execution = ExecutionReceiptProof::new(result_hash, proof);
        let root = tree.root().expect("root must exist");
        assert!(
            execution.verify(&root),
            "execution proof must verify against root"
        );
    }

    #[test]
    fn execution_receipt_proof_rejects_wrong_root() {
        let sequence = TransactionResultInner::Ok(crate::trigger::DataTriggerSequence::default());
        let result_hash = TransactionResult::hash_from_inner(&sequence);
        let tree = MerkleTree::from_iter([result_hash]);
        let proof = tree.get_proof(0).expect("proof must exist for result leaf");
        let execution = ExecutionReceiptProof::new(result_hash, proof);

        let wrong_root = HashOf::from_untyped_unchecked(Hash::new([0xCC; 32]));

        assert!(
            !execution.verify(&wrong_root),
            "execution proof must fail against a mismatched root"
        );
    }
}
