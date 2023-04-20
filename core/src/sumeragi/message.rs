//! Contains message structures for p2p communication during consensus.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::std_instead_of_core,
    clippy::std_instead_of_alloc,
    clippy::module_name_repetitions
)]

use iroha_crypto::{HashOf, SignaturesOf};
use iroha_data_model::{block::VersionedCommittedBlock, prelude::*};
use iroha_macro::*;
use iroha_version::prelude::*;
use parity_scale_codec::{Decode, Encode};

use super::view_change;
use crate::{
    block::{PendingBlock, Revalidate},
    tx::TransactionValidator,
    VersionedAcceptedTransaction, WorldStateView,
};

declare_versioned_with_scale!(VersionedPacket 1..2, Debug, Clone, iroha_macro::FromVariant);

impl VersionedPacket {
    /// Convert `&`[`Self`] to V1 reference
    pub const fn as_v1(&self) -> &MessagePacket {
        match self {
            Self::V1(v1) => v1,
        }
    }

    /// Convert `&mut` [`Self`] to V1 mutable reference
    pub fn as_mut_v1(&mut self) -> &mut MessagePacket {
        match self {
            Self::V1(v1) => v1,
        }
    }

    /// Perform the conversion from [`Self`] to V1
    pub fn into_v1(self) -> MessagePacket {
        match self {
            Self::V1(v1) => v1,
        }
    }
}

/// Helper structure, wrapping messages and view change proofs.
#[version_with_scale(n = 1, versioned = "VersionedPacket")]
#[derive(Debug, Clone, Decode, Encode)]
pub struct MessagePacket {
    /// Proof of view change. As part of this message handling, all
    /// peers which agree with view change should sign it.
    pub view_change_proofs: view_change::ProofChain,
    /// Actual Sumeragi message in this packet.
    pub message: Message,
}

impl MessagePacket {
    /// Construct [`Self`]
    pub fn new(view_change_proofs: view_change::ProofChain, message: impl Into<Message>) -> Self {
        Self {
            view_change_proofs,
            message: message.into(),
        }
    }
}

/// Message's variants that are used by peers to communicate in the process of consensus.
#[derive(Debug, Clone, Decode, Encode, FromVariant)]
pub enum Message {
    /// This message is sent by leader to all validating peers, when a new block is created.
    BlockCreated(BlockCreated),
    /// This message is sent by validating peers to proxy tail and observing peers when they have signed this block.
    BlockSigned(BlockSigned),
    /// This message is sent by proxy tail to validating peers and to leader, when the block is committed.
    BlockCommitted(BlockCommitted),
    /// This message is sent by `BlockSync` when new block is received
    BlockSyncUpdate(BlockSyncUpdate),
    /// View change is suggested due to some faulty peer or general fault in consensus.
    ViewChangeSuggested,
    /// This message is sent by all peers during gossiping.
    TransactionGossip(TransactionGossip),
}

/// `BlockCreated` message structure.
#[derive(Debug, Clone, Decode, Encode)]
#[non_exhaustive]
pub struct BlockCreated {
    /// The corresponding block.
    pub block: PendingBlock,
}

impl From<PendingBlock> for BlockCreated {
    fn from(block: PendingBlock) -> Self {
        Self { block }
    }
}

impl BlockCreated {
    /// Extract block from block created message.
    ///
    /// # Errors
    /// - When the block is invalid.
    pub fn validate_and_extract_block<const IS_GENESIS: bool>(
        self,
        transaction_validator: &TransactionValidator,
        wsv: WorldStateView,
    ) -> Result<PendingBlock, eyre::Report> {
        self.block
            .revalidate::<IS_GENESIS>(transaction_validator, wsv)?;
        Ok(self.block)
    }
    /// Get hash of block.
    pub fn hash(&self) -> HashOf<PendingBlock> {
        self.block.hash()
    }
}

/// `BlockSigned` message structure.
#[derive(Debug, Clone, Decode, Encode)]
#[non_exhaustive]
pub struct BlockSigned {
    /// Hash of the block being signed.
    pub hash: HashOf<PendingBlock>,
    /// Set of signatures.
    pub signatures: SignaturesOf<PendingBlock>,
}

impl From<PendingBlock> for BlockSigned {
    fn from(block: PendingBlock) -> Self {
        Self {
            hash: block.hash(),
            signatures: block.signatures,
        }
    }
}

/// `BlockCommitted` message structure.
#[derive(Debug, Clone, Decode, Encode)]
#[non_exhaustive]
pub struct BlockCommitted {
    /// Hash of the block being signed.
    pub hash: HashOf<VersionedCommittedBlock>,
    /// Set of signatures.
    pub signatures: SignaturesOf<VersionedCommittedBlock>,
}

impl From<VersionedCommittedBlock> for BlockCommitted {
    fn from(block: VersionedCommittedBlock) -> Self {
        Self {
            hash: block.hash().transmute(),
            signatures: block.as_v1().signatures.clone().transmute(),
        }
    }
}

/// `BlockSyncUpdate` message structure
#[derive(Debug, Clone, Decode, Encode)]
#[non_exhaustive]
pub struct BlockSyncUpdate {
    /// The corresponding block.
    block: VersionedCommittedBlock,
}

impl From<VersionedCommittedBlock> for BlockSyncUpdate {
    fn from(block: VersionedCommittedBlock) -> Self {
        Self { block }
    }
}

impl BlockSyncUpdate {
    /// Extract block from block sync update message.
    ///
    /// # Errors
    /// - When the block is invalid.
    pub fn validate_and_extract_block<const IS_GENESIS: bool>(
        self,
        transaction_validator: &TransactionValidator,
        wsv: WorldStateView,
    ) -> Result<VersionedCommittedBlock, eyre::Report> {
        self.block
            .revalidate::<IS_GENESIS>(transaction_validator, wsv)?;
        Ok(self.block)
    }

    /// Get hash of block.
    pub fn hash(&self) -> HashOf<VersionedCommittedBlock> {
        self.block.hash()
    }

    /// Get height of block.
    pub fn height(&self) -> u64 {
        self.block.header().height
    }
}

/// Message for gossiping batches of transactions.
#[derive(Decode, Encode, Debug, Clone)]
pub struct TransactionGossip {
    /// Batch of transactions.
    pub txs: Vec<VersionedSignedTransaction>,
}

impl TransactionGossip {
    #![allow(clippy::unused_async)]
    /// Constructor.
    pub fn new(txs: Vec<VersionedAcceptedTransaction>) -> Self {
        Self {
            // Converting into non-accepted transaction because it's not possible
            // to guarantee that the sending peer checked transaction limits
            txs: txs.into_iter().map(Into::into).collect(),
        }
    }
}
