//! This module contains `Block` and related implementations.
//!
//! `Block`s are organized into a linear sequence over time (also known as the block chain).

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, collections::BTreeSet, format, string::String, vec::Vec};
use core::{fmt::Display, time::Duration};
#[cfg(feature = "std")]
use std::collections::BTreeSet;

use derive_more::{Constructor, Display};
use iroha_crypto::{HashOf, MerkleProof, MerkleTree, SignatureOf};
use iroha_data_model_derive::model;
use iroha_macro::FromVariant;
use iroha_schema::IntoSchema;
use iroha_version::{declare_versioned, version_with_scale};
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

pub use self::model::*;
use crate::transaction::{error::TransactionRejectionReason, prelude::*};

#[model]
mod model {
    use core::num::NonZeroU64;

    use getset::{CopyGetters, Getters};

    use super::*;

    /// Header for a newly proposed block, prior to full validation.
    ///
    /// Does not include any audit metadata. Hashing and signing should be deferred until validation completes.
    #[derive(
        Debug,
        Display,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        CopyGetters,
        Getters,
        Decode,
        Encode,
        Deserialize,
        Serialize,
        IntoSchema,
    )]
    #[display(fmt = "№{height}")]
    #[ffi_type]
    pub struct NewBlockHeader {
        /// Number of blocks in the chain including this block.
        #[getset(get_copy = "pub")]
        pub height: NonZeroU64,
        /// Hash of the previous block in the chain.
        #[getset(get_copy = "pub")]
        pub prev_block_hash: Option<HashOf<BlockHeader>>,
        /// Creation timestamp as Unix time in milliseconds.
        #[getset(skip)]
        pub creation_time_ms: u64,
        /// Value of view change index. Used to resolve soft forks.
        #[getset(skip)]
        pub view_change_index: u32,
    }

    /// Core metadata for a validated block, including audit information.
    ///
    /// The header's hash serves as the block identifier and is the target for block signatures.
    #[derive(
        Debug,
        Display,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        CopyGetters,
        Getters,
        Decode,
        Encode,
        Deserialize,
        Serialize,
        IntoSchema,
    )]
    #[display(fmt = "{} (№{height})", "self.hash()")]
    #[ffi_type]
    pub struct BlockHeader {
        /// Number of blocks in the chain including this block.
        #[getset(get_copy = "pub")]
        pub height: NonZeroU64,
        /// Hash of the previous block in the chain.
        #[getset(get_copy = "pub")]
        pub prev_block_hash: Option<HashOf<BlockHeader>>,
        /// Merkle root of this block's transactions.
        /// None if there are no transactions (empty block).
        #[getset(get_copy = "pub")]
        pub merkle_root: Option<HashOf<MerkleTree<TransactionEntrypoint>>>,
        /// Merkle root of this block's transaction results.
        /// None if there are no transactions (empty block).
        #[getset(get_copy = "pub")]
        pub result_merkle_root: Option<HashOf<MerkleTree<TransactionResult>>>,
        /// Creation timestamp as Unix time in milliseconds.
        #[getset(skip)]
        pub creation_time_ms: u64,
        /// Value of view change index. Used to resolve soft forks.
        #[getset(skip)]
        pub view_change_index: u32,
    }

    /// Core contents of a block.
    #[derive(
        Debug,
        Display,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Encode,
        Serialize,
        IntoSchema,
        Deserialize,
        Decode,
    )]
    #[display(fmt = "({header})")]
    #[allow(clippy::redundant_pub_crate)]
    pub(crate) struct BlockPayload {
        /// Essential metadata for a block in the chain.
        pub header: BlockHeader,
        /// External transactions as source of the state, forming the first half of the transaction entrypoints.
        pub transactions: Vec<SignedTransaction>,
    }

    /// The validator index and its corresponding signature on the block header.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Decode,
        Encode,
        Deserialize,
        Serialize,
        Constructor,
        IntoSchema,
    )]
    pub struct BlockSignature {
        /// Validator index in the network topology.
        pub index: u64,
        /// Validator signature on the block header.
        pub signature: SignatureOf<BlockHeader>,
    }

    /// Block collecting signatures from validators.
    #[version_with_scale(version = 1, versioned_alias = "SignedBlock")]
    #[derive(
        Debug,
        Display,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Encode,
        Serialize,
        IntoSchema,
        Decode,
        Deserialize,
    )]
    #[display(fmt = "{}", "self.header()")]
    #[ffi_type]
    pub struct SignedBlockV1 {
        /// Signatures of validators who approved this block.
        pub(super) signatures: BTreeSet<BlockSignature>,
        /// Block payload to be signed.
        pub(super) payload: BlockPayload,
        /// Secondary block state resulting from execution.
        // TODO: refactor state transitions so that only validated blocks store results.
        pub(super) result: BlockResult,
    }

    /// Secondary block state resulting from execution.
    #[derive(
        Debug,
        Display,
        Clone,
        Default,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Decode,
        Encode,
        Deserialize,
        Serialize,
        IntoSchema,
    )]
    #[display(fmt = "BlockResult")]
    pub struct BlockResult {
        /// Time-triggered entrypoints, forming the second half of the transaction entrypoints.
        pub time_triggers: Vec<TimeTriggerEntrypoint>,
        /// Merkle tree over the transaction entrypoints (external transactions followed by time triggers).
        pub merkle: MerkleTree<TransactionEntrypoint>,
        /// Merkle tree over the transaction results, with indices aligned to the entrypoint Merkle tree.
        pub result_merkle: MerkleTree<TransactionResult>,
        /// Transaction execution results, with indices aligned to the entrypoint Merkle tree.
        pub transaction_results: Vec<TransactionResult>,
    }
}

#[cfg(any(feature = "ffi_export", feature = "ffi_import"))]
declare_versioned!(SignedBlock 1..2, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, FromVariant, iroha_ffi::FfiType, IntoSchema);
#[cfg(all(not(feature = "ffi_export"), not(feature = "ffi_import")))]
declare_versioned!(SignedBlock 1..2, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, FromVariant, IntoSchema);

impl NewBlockHeader {
    /// Returns `true` if this header represents a genesis block.
    #[inline]
    pub const fn is_genesis(&self) -> bool {
        self.height.get() == 1
    }

    /// The creation timestamp of the block as a `Duration`.
    pub const fn creation_time(&self) -> Duration {
        Duration::from_millis(self.creation_time_ms)
    }
}

impl BlockHeader {
    /// Returns `true` if this header represents a genesis block.
    #[inline]
    pub const fn is_genesis(&self) -> bool {
        self.height.get() == 1
    }

    /// The creation timestamp of the block as a `Duration`.
    pub const fn creation_time(&self) -> Duration {
        Duration::from_millis(self.creation_time_ms)
    }

    /// The hash of this block header. It should also serve as the hash of the block itself.
    #[inline]
    pub fn hash(&self) -> HashOf<BlockHeader> {
        HashOf::new(self)
    }

    /// Converts back into a `NewBlockHeader`, removing all post-validation audit metadata while preserving the rest.
    pub const fn regress(self) -> NewBlockHeader {
        NewBlockHeader {
            height: self.height,
            prev_block_hash: self.prev_block_hash,
            creation_time_ms: self.creation_time_ms,
            view_change_index: self.view_change_index,
        }
    }
}

impl SignedBlockV1 {
    fn hash(&self) -> HashOf<BlockHeader> {
        self.payload.header.hash()
    }

    fn header(&self) -> BlockHeader {
        self.payload.header
    }
}

impl SignedBlock {
    /// API to construct a `SignedBlock` from a `iroha_core::block::new::NewBlock` payload.
    #[cfg(feature = "transparent_api")]
    pub fn new_unverified_unsigned(
        header: NewBlockHeader,
        transactions: Vec<SignedTransaction>,
    ) -> SignedBlock {
        let NewBlockHeader {
            height,
            prev_block_hash,
            creation_time_ms,
            view_change_index,
        } = header;

        SignedBlockV1 {
            signatures: BTreeSet::new(),
            payload: BlockPayload {
                header: BlockHeader {
                    height,
                    prev_block_hash,
                    merkle_root: None,
                    result_merkle_root: None,
                    creation_time_ms,
                    view_change_index,
                },
                transactions,
            },
            result: BlockResult::default(),
        }
        .into()
    }

    /// Set this block's transaction results.
    ///
    /// Given a pair of vectors -- transaction hashes and their corresponding results,
    /// - Record Merkle trees and the transaction results outside the block payload.
    /// - Record the Merkle root of the transaction results inside the block header, enabling client verification.
    #[cfg(feature = "transparent_api")]
    pub fn set_transaction_results(
        &mut self,
        time_triggers: Vec<TimeTriggerEntrypoint>,
        hashes: Vec<HashOf<TransactionEntrypoint>>,
        results: Vec<TransactionResultInner>,
    ) {
        let SignedBlock::V1(block) = self;

        let result_hashes = results.iter().map(TransactionResult::hash_from_inner);
        block.result.time_triggers = time_triggers;
        block.result.merkle = MerkleTree::from_iter(hashes);
        block.result.result_merkle = result_hashes.collect();
        block.result.transaction_results =
            results.into_iter().map(TransactionResult::from).collect();
        block.payload.header.merkle_root = block.result.merkle.root();
        block.payload.header.result_merkle_root = block.result.result_merkle.root();
    }

    /// Return error for the transaction index
    pub fn error(&self, tx: usize) -> Option<&TransactionRejectionReason> {
        let SignedBlock::V1(block) = self;
        block
            .result
            .transaction_results
            .get(tx)
            .and_then(|result| result.as_ref().err())
    }

    /// Block payload. Used for tests
    #[cfg(feature = "transparent_api")]
    pub fn payload(&self) -> &BlockPayload {
        let SignedBlock::V1(block) = self;
        &block.payload
    }

    /// Block header
    #[inline]
    pub fn header(&self) -> BlockHeader {
        let SignedBlock::V1(block) = self;
        block.header()
    }

    /// Mutable reference to the block header. Test-only API.
    pub fn header_mut(&mut self) -> &mut BlockHeader {
        let SignedBlock::V1(block) = self;
        &mut block.payload.header
    }

    /// Signatures of peers which approved this block.
    #[inline]
    pub fn signatures(
        &self,
    ) -> impl ExactSizeIterator<Item = &BlockSignature> + DoubleEndedIterator {
        let SignedBlock::V1(block) = self;
        block.signatures.iter()
    }

    /// Signed transactions originating from external sources.
    /// Indices align with those of the entrypoints.
    #[inline]
    pub fn external_transactions(
        &self,
    ) -> impl ExactSizeIterator<Item = &SignedTransaction> + DoubleEndedIterator {
        let SignedBlock::V1(block) = self;
        block.payload.transactions.iter()
    }

    /// Block transactions, the underlying vector
    #[inline]
    pub fn transactions_vec(&self) -> &Vec<SignedTransaction> {
        let SignedBlock::V1(block) = self;
        &block.payload.transactions
    }

    /// Check if block is empty (has no transactions)
    #[inline]
    pub fn is_empty(&self) -> bool {
        let SignedBlock::V1(block) = self;
        block.payload.transactions.is_empty()
    }

    /// Time-triggered entrypoints in execution order, following external transactions.
    /// Indices offset by the number of the external transactions align with those of the entrypoints.
    #[inline]
    pub fn time_triggers(
        &self,
    ) -> impl ExactSizeIterator<Item = &TimeTriggerEntrypoint> + DoubleEndedIterator {
        let SignedBlock::V1(block) = self;
        block.result.time_triggers.iter()
    }

    /// Hashes of each transaction entrypoint (external and time-triggered) in execution order.
    /// Indices align with those of the entrypoints.
    #[inline]
    pub fn entrypoint_hashes(
        &self,
    ) -> impl ExactSizeIterator<Item = HashOf<TransactionEntrypoint>> + DoubleEndedIterator + '_
    {
        let SignedBlock::V1(block) = self;
        block.result.merkle.leaves()
    }

    /// Merkle inclusion proofs of each transaction entrypoint (external and time-triggered) in execution order.
    /// Indices align with those of the entrypoints.
    #[inline]
    pub fn entrypoint_proofs(
        &self,
    ) -> impl ExactSizeIterator<Item = MerkleProof<TransactionEntrypoint>> + DoubleEndedIterator + '_
    {
        let SignedBlock::V1(block) = self;
        let n_leaves: u32 = block
            .result
            .merkle
            .leaves()
            .len()
            .try_into()
            .expect("bug: leaf count exceeded u32::MAX");
        (0..n_leaves).map(|i| {
            block
                .result
                .merkle
                .get_proof(i)
                .expect("bug: missing Merkle proof at valid index")
        })
    }

    /// Transaction entrypoints (external and time-triggered) in execution order.
    #[inline]
    pub fn entrypoints_cloned(
        &self,
    ) -> impl ExactSizeIterator<Item = TransactionEntrypoint> + DoubleEndedIterator + '_ {
        EntrypointIterator::new(self)
    }

    /// Hashes of each transaction result (trigger sequence or rejection reason) in execution order.
    /// Indices align with those of the entrypoints.
    #[inline]
    pub fn result_hashes(
        &self,
    ) -> impl ExactSizeIterator<Item = HashOf<TransactionResult>> + DoubleEndedIterator + '_ {
        let SignedBlock::V1(block) = self;
        block.result.result_merkle.leaves()
    }

    /// Merkle inclusion proofs of each transaction result (trigger sequence or rejection reason) in execution order.
    /// Indices align with those of the entrypoints.
    #[inline]
    pub fn result_proofs(
        &self,
    ) -> impl ExactSizeIterator<Item = MerkleProof<TransactionResult>> + DoubleEndedIterator + '_
    {
        let SignedBlock::V1(block) = self;
        let n_leaves: u32 = block
            .result
            .result_merkle
            .leaves()
            .len()
            .try_into()
            .expect("bug: leaf count exceeded u32::MAX");
        (0..n_leaves).map(|i| {
            block
                .result
                .result_merkle
                .get_proof(i)
                .expect("bug: missing Merkle proof at valid index")
        })
    }

    /// Actual transaction results (trigger sequence or rejection reason) in execution order.
    /// Indices align with those of the entrypoints.
    #[inline]
    pub fn results(
        &self,
    ) -> impl ExactSizeIterator<Item = &TransactionResult> + DoubleEndedIterator {
        let SignedBlock::V1(block) = self;
        block.result.transaction_results.iter()
    }

    /// Successful transaction indices and data trigger sequences.
    pub fn successes(&self) -> impl Iterator<Item = (u64, &DataTriggerSequence)> {
        self.results()
            .enumerate()
            .filter_map(|(i, result)| result.as_ref().ok().map(|ok| (i as u64, ok)))
    }

    /// Failed transaction indices and rejection reasons.
    pub fn errors(&self) -> impl Iterator<Item = (u64, &TransactionRejectionReason)> {
        self.results()
            .enumerate()
            .filter_map(|(i, result)| result.as_ref().err().map(|err| (i as u64, err)))
    }

    /// Calculate block hash
    #[inline]
    pub fn hash(&self) -> HashOf<BlockHeader> {
        let SignedBlock::V1(block) = self;
        block.hash()
    }

    /// Add additional signature to this block
    #[cfg(feature = "transparent_api")]
    pub fn sign(&mut self, private_key: &iroha_crypto::PrivateKey, signatory: usize) {
        let SignedBlock::V1(block) = self;

        block.signatures.insert(BlockSignature::new(
            signatory as u64,
            SignatureOf::new(private_key, &block.payload.header),
        ));
    }

    /// Add signature to the block
    ///
    /// # Errors
    ///
    /// if signature is invalid
    #[cfg(feature = "transparent_api")]
    pub fn add_signature(&mut self, signature: BlockSignature) -> Result<(), iroha_crypto::Error> {
        if self.signatures().any(|s| signature.index == s.index) {
            return Err(iroha_crypto::Error::Signing(
                "Duplicate signature".to_owned(),
            ));
        }

        let SignedBlock::V1(block) = self;
        block.signatures.insert(signature);

        Ok(())
    }

    /// Replace signatures without verification
    ///
    /// # Errors
    ///
    /// if there is a duplicate signature
    #[cfg(feature = "transparent_api")]
    pub fn replace_signatures(
        &mut self,
        signatures: BTreeSet<BlockSignature>,
    ) -> Result<BTreeSet<BlockSignature>, iroha_crypto::Error> {
        if signatures.is_empty() {
            return Err(iroha_crypto::Error::Signing("Signatures empty".to_owned()));
        }

        signatures
            .iter()
            .map(|signature| signature.index)
            .try_fold(BTreeSet::new(), |mut acc, elem| {
                if !acc.insert(elem) {
                    return Err(iroha_crypto::Error::Signing(format!(
                        "{elem}: Duplicate signature"
                    )));
                }

                Ok(acc)
            })?;

        let SignedBlock::V1(block) = self;
        Ok(core::mem::replace(&mut block.signatures, signatures))
    }

    /// Creates genesis block not signed by any peer.
    #[cfg(feature = "std")]
    pub fn genesis(transactions: Vec<SignedTransaction>, creation_time_ms: u64) -> SignedBlock {
        use nonzero_ext::nonzero;

        let merkle_root = transactions
            .iter()
            .map(SignedTransaction::hash_as_entrypoint)
            .collect::<MerkleTree<_>>()
            .root()
            .expect("Genesis block must have transactions");
        let header = BlockHeader {
            height: nonzero!(1_u64),
            prev_block_hash: None,
            merkle_root: Some(merkle_root),
            result_merkle_root: None,
            creation_time_ms,
            view_change_index: 0,
        };

        SignedBlockV1 {
            signatures: BTreeSet::new(),
            payload: BlockPayload {
                header,
                transactions,
            },
            result: BlockResult::default(),
        }
        .into()
    }
}

struct EntrypointIterator<'a> {
    block: &'a SignedBlock,
    index: usize,
    index_back: usize,
    n_external_transactions: usize,
}

impl Iterator for EntrypointIterator<'_> {
    type Item = TransactionEntrypoint;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index_back <= self.index {
            return None;
        }

        let SignedBlock::V1(block_inner) = self.block;
        let item = if self.index < self.n_external_transactions {
            block_inner.payload.transactions[self.index].clone().into()
        } else {
            block_inner.result.time_triggers[self.index - self.n_external_transactions]
                .clone()
                .into()
        };

        // Increment the front index eagerly.
        self.index += 1;
        Some(item)
    }
}

impl DoubleEndedIterator for EntrypointIterator<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.index_back <= self.index {
            return None;
        }
        // Decrement the back index lazily.
        self.index_back -= 1;

        let SignedBlock::V1(block_inner) = self.block;
        let item = if self.index_back < self.n_external_transactions {
            block_inner.payload.transactions[self.index_back]
                .clone()
                .into()
        } else {
            block_inner.result.time_triggers[self.index_back - self.n_external_transactions]
                .clone()
                .into()
        };

        Some(item)
    }
}

impl ExactSizeIterator for EntrypointIterator<'_> {
    fn len(&self) -> usize {
        self.index_back - self.index
    }
}

impl<'a> EntrypointIterator<'a> {
    fn new(block: &'a SignedBlock) -> Self {
        let SignedBlock::V1(block_inner) = block;
        let n_external_transactions = block_inner.payload.transactions.len();
        let n_entrypoints = n_external_transactions + block_inner.result.time_triggers.len();

        Self {
            block,
            index: 0,
            index_back: n_entrypoints,
            n_external_transactions,
        }
    }
}

impl Display for SignedBlock {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let SignedBlock::V1(block) = self;
        block.fmt(f)
    }
}

#[cfg(feature = "http")]
pub mod stream {
    //! Blocks for streaming API.

    use derive_more::Constructor;
    use iroha_schema::IntoSchema;
    use parity_scale_codec::{Decode, Encode};

    pub use self::model::*;
    use super::*;

    #[model]
    mod model {
        use core::num::NonZeroU64;

        use getset::Getters;

        use super::*;

        /// Request sent to subscribe to blocks stream
        #[derive(
            Debug,
            Clone,
            Copy,
            Constructor,
            Decode,
            Encode,
            Deserialize,
            Serialize,
            IntoSchema,
            Getters,
        )]
        pub struct BlockSubscriptionRequest {
            /// Height to stream blocks from
            #[getset(get = "pub")]
            pub height: NonZeroU64,
        }

        /// Block stream message
        #[derive(Debug, Clone, Decode, Encode, Deserialize, Serialize, IntoSchema)]
        pub enum BlockStreamMessage {
            /// _Sent by client:_ must be sent once the connection is established
            Subscribe(BlockSubscriptionRequest),
            /// _Sent by client:_ request to send the next block once available
            Next,
            /// _Sent by server:_ message containing a block
            Block(SignedBlock),
        }
    }

    /// Exports common structs and enums from this module.
    pub mod prelude {
        pub use super::{BlockStreamMessage, BlockSubscriptionRequest};
    }
}

pub mod error {
    //! Module containing errors that can occur during instruction evaluation

    pub use self::model::*;
    use super::*;

    #[model]
    mod model {
        use super::*;

        /// The reason for rejecting a transaction with new blocks.
        #[derive(
            Debug,
            Display,
            Clone,
            Copy,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            iroha_macro::FromVariant,
            Decode,
            Encode,
            Deserialize,
            Serialize,
            IntoSchema,
        )]
        #[display(fmt = "Block was rejected during consensus")]
        #[serde(untagged)] // Unaffected by #3330 as it's a unit variant
        #[repr(transparent)]
        #[ffi_type]
        pub enum BlockRejectionReason {
            /// Block was rejected during consensus.
            ConsensusBlockRejection,
        }
    }

    #[cfg(feature = "std")]
    impl std::error::Error for BlockRejectionReason {}
}

pub mod prelude {
    //! For glob-import
    #[cfg(feature = "http")]
    pub use super::stream::prelude::*;
    pub use super::{error::BlockRejectionReason, BlockHeader, BlockSignature, SignedBlock};
}
