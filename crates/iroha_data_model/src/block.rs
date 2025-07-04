//! This module contains `Block` and related implementations.
//!
//! `Block`s are organised into a linear sequence over time (also known as the block chain).

#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
    format,
    string::String,
    vec::Vec,
};
use core::{fmt::Display, time::Duration};
#[cfg(feature = "std")]
use std::collections::{BTreeMap, BTreeSet};

use derive_more::Display;
use iroha_crypto::{HashOf, MerkleTree, SignatureOf};
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
    #[allow(missing_docs)]
    #[ffi_type]
    pub struct BlockHeader {
        /// Number of blocks in the chain including this block.
        #[getset(get_copy = "pub")]
        pub height: NonZeroU64,
        /// Hash of the previous block in the chain.
        #[getset(get_copy = "pub")]
        pub prev_block_hash: Option<HashOf<BlockHeader>>,
        /// Hash of merkle tree root of transactions' hashes.
        /// None if no transactions (empty block).
        #[getset(get_copy = "pub")]
        pub transactions_hash: Option<HashOf<MerkleTree<SignedTransaction>>>,
        /// Creation timestamp (unix time in milliseconds).
        #[getset(skip)]
        pub creation_time_ms: u64,
        /// Value of view change index. Used to resolve soft forks.
        #[getset(skip)]
        pub view_change_index: u32,
    }

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
    #[allow(missing_docs)]
    #[allow(clippy::redundant_pub_crate)]
    pub(crate) struct BlockPayload {
        /// Block header
        pub header: BlockHeader,
        /// array of transactions, which successfully passed validation and consensus step.
        pub transactions: Vec<SignedTransaction>,
    }

    /// Signature of a block
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
        /// Index of the peer in the topology
        pub u64,
        /// Payload
        pub SignatureOf<BlockHeader>,
    );

    /// Signed block
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
        /// Signatures of peers which approved this block.
        pub(super) signatures: BTreeSet<BlockSignature>,
        /// Block payload
        pub(super) payload: BlockPayload,
        /// Collection of rejection reasons for every transaction if exists
        ///
        /// # Warning
        ///
        /// Transaction errors are not part of the block hash or protected by the block signature.
        pub(super) errors: BTreeMap<u64, TransactionRejectionReason>,
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

    /// Calculate block hash
    #[inline]
    pub fn hash(&self) -> HashOf<BlockHeader> {
        iroha_crypto::HashOf::new(self)
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
            signatures: [signature].into_iter().collect(),
            payload: BlockPayload {
                header,
                transactions,
            },
            errors: BTreeMap::new(),
        }
        .into()
    }

    /// Setter for transaction errors
    #[cfg(feature = "transparent_api")]
    pub fn set_transaction_errors(
        &mut self,
        errors: BTreeMap<u64, TransactionRejectionReason>,
    ) -> &mut Self {
        let SignedBlock::V1(block) = self;
        block.errors = errors;
        self
    }

    /// Return error for the transaction index
    pub fn error(&self, tx: usize) -> Option<&TransactionRejectionReason> {
        let SignedBlock::V1(block) = self;
        block.errors.get(&(tx as u64))
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

    /// Block transactions
    #[inline]
    pub fn transactions(&self) -> impl ExactSizeIterator<Item = &SignedTransaction> {
        let SignedBlock::V1(block) = self;
        block.payload.transactions.iter()
    }

    /// Check if block is empty (has no transactions)
    #[inline]
    pub fn is_empty(&self) -> bool {
        let SignedBlock::V1(block) = self;
        block.payload.transactions.is_empty()
    }

    /// Collection of rejection reasons for every transaction if exists
    ///
    /// # Warning
    ///
    /// Transaction errors are not part of the block hash or protected by the block signature.
    pub fn errors(&self) -> impl ExactSizeIterator<Item = (&u64, &TransactionRejectionReason)> {
        let SignedBlock::V1(block) = self;
        block.errors.iter()
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

        block.signatures.insert(BlockSignature(
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
        if self.signatures().any(|s| signature.0 == s.0) {
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

        let transactions_hash = transactions
            .iter()
            .map(SignedTransaction::hash)
            .collect::<MerkleTree<_>>()
            .root()
            .expect("Genesis block must have transactions");
        let creation_time_ms = Self::get_genesis_block_creation_time(&transactions);
        let header = BlockHeader {
            height: nonzero!(1_u64),
            prev_block_hash: None,
            transactions_hash: Some(transactions_hash),
            creation_time_ms,
            view_change_index: 0,
        };

        let signature = BlockSignature(0, SignatureOf::new(private_key, &header));
        let payload = BlockPayload {
            header,
            transactions,
        };

        SignedBlockV1 {
            signatures: [signature].into_iter().collect(),
            payload,
            errors: BTreeMap::new(),
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
