//! This module contains `Block` and related implementations.
//!
//! `Block`s are organized into a linear sequence over time (also known as the block chain).

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, format, string::String, vec::Vec};
use core::{fmt::Display, num::NonZeroU64, time::Duration};

use derive_more::Display;
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
    use getset::{CopyGetters, Getters};

    use super::*;

    /// Essential metadata for a block in the chain.
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
        Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Serialize, IntoSchema,
    )]
    #[display(fmt = "({header})")]
    #[allow(clippy::redundant_pub_crate)]
    pub(crate) struct BlockPayload {
        /// Essential metadata for a block in the chain.
        pub header: BlockHeader,
        /// External transactions as source of the state, forming the first half of the transaction entrypoints.
        pub transactions: Vec<SignedTransaction>,
    }

    /// Cryptographic approval from a validator for a block.
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
        IntoSchema,
    )]
    pub struct BlockSignature(
        /// Validator index in the network topology.
        pub u64,
        /// Validator signature of the block header.
        pub SignatureOf<BlockHeader>,
    );

    /// Block collecting signatures from validators.
    #[version_with_scale(version = 1, versioned_alias = "SignedBlock")]
    #[derive(
        Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Serialize, IntoSchema,
    )]
    #[display(fmt = "{}", "self.header()")]
    #[ffi_type]
    pub struct SignedBlockV1 {
        /// Signatures of validators who approved this block.
        pub(super) signatures: Vec<BlockSignature>,
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

impl BlockHeader {
    /// Checks if it's a header of a genesis block.
    #[inline]
    pub const fn is_genesis(&self) -> bool {
        self.height.get() == 1
    }

    /// Creation timestamp
    pub const fn creation_time(&self) -> Duration {
        Duration::from_millis(self.creation_time_ms)
    }

    /// Returns the consensus-level hash of the block header,
    /// excluding the `result_merkle_root` field.
    ///
    /// TODO: prevent divergent hashes caused by direct calls to `HashOf::new`,
    /// leveraging specialization once it's stabilized (<https://github.com/rust-lang/rust/issues/31844>).
    #[inline]
    pub fn hash(&self) -> HashOf<BlockHeader> {
        self.hash_without_results()
    }

    /// Computes the header hash without including `result_merkle_root`.
    #[inline]
    fn hash_without_results(&self) -> HashOf<BlockHeader> {
        /// A view of `BlockHeader` used for consensus hashing, omitting the execution results.
        #[derive(Encode)]
        struct BlockHeaderForConsensus {
            height: NonZeroU64,
            prev_block_hash: Option<HashOf<BlockHeader>>,
            merkle_root: Option<HashOf<MerkleTree<TransactionEntrypoint>>>,
            creation_time_ms: u64,
            view_change_index: u32,
        }

        impl From<&BlockHeader> for BlockHeaderForConsensus {
            fn from(value: &BlockHeader) -> Self {
                let BlockHeader {
                    height,
                    prev_block_hash,
                    merkle_root,
                    result_merkle_root: _,
                    creation_time_ms,
                    view_change_index,
                } = *value;

                Self {
                    height,
                    prev_block_hash,
                    merkle_root,
                    creation_time_ms,
                    view_change_index,
                }
            }
        }

        HashOf::from_untyped_unchecked(HashOf::new(&BlockHeaderForConsensus::from(self)).into())
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
    /// Create new block with a given signature
    ///
    /// # Warning
    ///
    /// All transactions are categorized as valid
    #[cfg(feature = "transparent_api")]
    pub fn presigned(
        signature: BlockSignature,
        header: BlockHeader,
        transactions: Vec<SignedTransaction>,
    ) -> SignedBlock {
        SignedBlockV1 {
            signatures: vec![signature],
            payload: BlockPayload {
                header,
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

        block.signatures.push(BlockSignature(
            signatory as u64,
            SignatureOf::from_hash(private_key, block.payload.header.hash()),
        ));
    }

    /// Add signature to the block
    ///
    /// # Errors
    ///
    /// if signature is invalid
    #[cfg(feature = "transparent_api")]
    pub fn add_signature(&mut self, signature: BlockSignature) -> Result<(), iroha_crypto::Error> {
        if self.signatures().any(|s| signature.0 == s.0) {
            return Err(iroha_crypto::Error::Signing(
                "Duplicate signature".to_owned(),
            ));
        }

        let SignedBlock::V1(block) = self;
        block.signatures.push(signature);

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
        signatures: Vec<BlockSignature>,
    ) -> Result<Vec<BlockSignature>, iroha_crypto::Error> {
        #[cfg(not(feature = "std"))]
        use alloc::collections::BTreeSet;
        #[cfg(feature = "std")]
        use std::collections::BTreeSet;

        if signatures.is_empty() {
            return Err(iroha_crypto::Error::Signing("Signatures empty".to_owned()));
        }

        signatures.iter().map(|signature| signature.0).try_fold(
            BTreeSet::new(),
            |mut acc, elem| {
                if !acc.insert(elem) {
                    return Err(iroha_crypto::Error::Signing(format!(
                        "{elem}: Duplicate signature"
                    )));
                }

                Ok(acc)
            },
        )?;

        let SignedBlock::V1(block) = self;
        Ok(core::mem::replace(&mut block.signatures, signatures))
    }

    /// Creates genesis block signed with genesis private key (and not signed by any peer)
    #[cfg(feature = "std")]
    pub fn genesis(
        transactions: Vec<SignedTransaction>,
        private_key: &iroha_crypto::PrivateKey,
    ) -> SignedBlock {
        use nonzero_ext::nonzero;

        let merkle_root = transactions
            .iter()
            .map(SignedTransaction::hash_as_entrypoint)
            .collect::<MerkleTree<_>>()
            .root()
            .expect("Genesis block must have transactions");
        let creation_time_ms = Self::get_genesis_block_creation_time(&transactions);
        let header = BlockHeader {
            height: nonzero!(1_u64),
            prev_block_hash: None,
            merkle_root: Some(merkle_root),
            result_merkle_root: None,
            creation_time_ms,
            view_change_index: 0,
        };

        let signature = BlockSignature(0, SignatureOf::from_hash(private_key, header.hash()));
        let payload = BlockPayload {
            header,
            transactions,
        };

        SignedBlockV1 {
            signatures: vec![signature],
            payload,
            result: BlockResult::default(),
        }
        .into()
    }

    #[cfg(feature = "std")]
    fn get_genesis_block_creation_time(transactions: &[SignedTransaction]) -> u64 {
        use std::time::SystemTime;

        let latest_txn_time = transactions
            .iter()
            .map(SignedTransaction::creation_time)
            .max()
            .expect("INTERNAL BUG: Genesis block is empty");
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        now
            // We have invariant that "transaction creation time" < "block creation time"
            // See `BlockPayloadCandidate::validate_header`
            .max(latest_txn_time + Duration::from_millis(1))
            .as_millis()
            .try_into()
            .expect("INTERNAL BUG: Unix timestamp exceedes u64::MAX")
    }
}

impl BlockSignature {
    /// Peer topology index
    pub fn index(&self) -> u64 {
        self.0
    }

    /// Signature itself
    pub fn payload(&self) -> &SignatureOf<BlockHeader> {
        &self.1
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

mod candidate {
    use parity_scale_codec::Input;

    use super::*;

    #[derive(Decode, Deserialize)]
    struct SignedBlockCandidate {
        signatures: Vec<BlockSignature>,
        payload: BlockPayload,
        result: BlockResult,
    }

    #[derive(Decode, Deserialize)]
    struct BlockPayloadCandidate {
        header: BlockHeader,
        transactions: Vec<SignedTransaction>,
    }

    impl BlockPayloadCandidate {
        fn validate(self) -> Result<BlockPayload, &'static str> {
            #[cfg(not(target_family = "wasm"))]
            {
                self.validate_header()?;
            }

            Ok(BlockPayload {
                header: self.header,
                transactions: self.transactions,
            })
        }

        #[cfg(not(target_family = "wasm"))]
        fn validate_header(&self) -> Result<(), &'static str> {
            let actual_txs_hash = self.header.merkle_root;

            let expected_txs_hash = self
                .transactions
                .iter()
                .map(SignedTransaction::hash_as_entrypoint)
                .collect::<MerkleTree<_>>()
                .root();

            if expected_txs_hash != actual_txs_hash {
                return Err("Transactions' hash incorrect");
            }

            self.transactions.iter().try_for_each(|tx| {
                if tx.creation_time() >= self.header.creation_time() {
                    return Err("Transaction creation time is ahead of block creation time");
                }

                Ok(())
            })?;

            Ok(())
        }
    }

    impl SignedBlockCandidate {
        fn validate(self) -> Result<SignedBlockV1, &'static str> {
            #[cfg(not(target_family = "wasm"))]
            {
                self.validate_signatures()?;

                if self.payload.header.height.get() == 1 {
                    self.validate_genesis()?;
                }
            }

            Ok(SignedBlockV1 {
                signatures: self.signatures,
                payload: self.payload,
                // TODO: clear secondary state; ignore any execution results from other validators
                result: self.result,
            })
        }

        #[cfg(not(target_family = "wasm"))]
        fn validate_signatures(&self) -> Result<(), &'static str> {
            #[cfg(not(feature = "std"))]
            use alloc::collections::BTreeSet;
            #[cfg(feature = "std")]
            use std::collections::BTreeSet;

            if self.signatures.is_empty() && self.payload.header.height.get() != 1 {
                return Err("Block missing signatures");
            }

            self.signatures
                .iter()
                .map(|signature| signature.0)
                .try_fold(BTreeSet::new(), |mut acc, elem| {
                    if !acc.insert(elem) {
                        return Err("Duplicate signature in block");
                    }

                    Ok(acc)
                })?;

            Ok(())
        }

        #[cfg(not(target_family = "wasm"))]
        fn validate_genesis(&self) -> Result<(), &'static str> {
            let transactions = self.payload.transactions.as_slice();

            for transaction in transactions {
                let Executable::Instructions(_) = transaction.instructions() else {
                    return Err("Genesis transaction must contain instructions");
                };
            }

            let Some(transaction_executor) = transactions.first() else {
                return Err("Genesis block must contain at least one transaction");
            };
            let Executable::Instructions(instructions_executor) =
                transaction_executor.instructions()
            else {
                return Err("Genesis transaction must contain instructions");
            };
            let [crate::isi::InstructionBox::Upgrade(_)] = instructions_executor.as_ref() else {
                return Err(
                    "First transaction must contain single `Upgrade` instruction to set executor",
                );
            };

            if transactions.len() > 5 {
                return Err(
                    "Genesis block must have 1 to 5 transactions (executor upgrade, parameters, ordinary instructions, wasm trigger registrations, initial topology)",
                );
            }

            Ok(())
        }
    }

    impl Decode for super::BlockPayload {
        fn decode<I: Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error> {
            BlockPayloadCandidate::decode(input)?
                .validate()
                .map_err(Into::into)
        }
    }

    impl<'de> Deserialize<'de> for super::BlockPayload {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::de::Error as _;

            BlockPayloadCandidate::deserialize(deserializer)?
                .validate()
                .map_err(D::Error::custom)
        }
    }

    impl Decode for SignedBlockV1 {
        fn decode<I: Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error> {
            SignedBlockCandidate::decode(input)?
                .validate()
                .map_err(Into::into)
        }
    }

    impl<'de> Deserialize<'de> for SignedBlockV1 {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::de::Error as _;

            SignedBlockCandidate::deserialize(deserializer)?
                .validate()
                .map_err(D::Error::custom)
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

        use super::*;

        /// Request sent to subscribe to blocks stream starting from the given height.
        #[derive(
            Debug, Clone, Copy, Constructor, Decode, Encode, Deserialize, Serialize, IntoSchema,
        )]
        #[repr(transparent)]
        pub struct BlockSubscriptionRequest(pub NonZeroU64);

        /// Message sent by the stream producer containing block.
        #[derive(Debug, Clone, Decode, Encode, Deserialize, Serialize, IntoSchema)]
        #[repr(transparent)]
        pub struct BlockMessage(pub SignedBlock);
    }

    impl From<BlockMessage> for SignedBlock {
        fn from(source: BlockMessage) -> Self {
            source.0
        }
    }

    /// Exports common structs and enums from this module.
    pub mod prelude {
        pub use super::{BlockMessage, BlockSubscriptionRequest};
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

#[cfg(test)]
mod tests {
    use core::num::NonZeroU64;

    use super::*;

    #[test]
    fn result_merkle_root_does_not_affect_block_hash() {
        let mut header = BlockHeader {
            height: NonZeroU64::new(123_456).unwrap(),
            prev_block_hash: Some(HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
                b"prev_block_hash",
            ))),
            merkle_root: Some(HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
                b"merkle_root",
            ))),
            result_merkle_root: None,
            creation_time_ms: 123_456_789_000,
            view_change_index: 123,
        };
        let hash0 = header.hash();
        header.result_merkle_root = Some(HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
            b"result_merkle_root",
        )));
        let hash1 = header.hash();
        assert_eq!(hash0, hash1);
    }
}
