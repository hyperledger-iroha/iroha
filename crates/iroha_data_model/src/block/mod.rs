//! This module contains `Block` and related implementations.
//!
//! `Block`s are organized into a linear sequence over time (also known as the block chain).

use std::{
    borrow::Cow,
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
    convert::TryInto,
    fmt, format,
    string::String,
    time::Duration,
    vec::Vec,
};

#[cfg(feature = "transparent_api")]
use iroha_crypto::Hash;
use iroha_crypto::{HashOf, MerkleTree, SignatureOf};
use iroha_data_model_derive::model;
use iroha_schema::IntoSchema;
use iroha_version::{UnsupportedVersion, Version};
use norito::{
    codec::{Decode, Encode},
    core::{
        Compression, Error as NoritoFrameError, MAGIC, VERSION_MAJOR, VERSION_MINOR,
        default_encode_flags, hardware_crc64 as norito_crc64,
    },
};

use self::proofs::{BlockReceiptProof, ExecutionReceiptProof};
use crate::da::commitment::{
    DaCommitmentBundle, DaProofPolicy, DaProofPolicyBundle, DaProofScheme,
};

pub mod proofs;

fn enforce_payload_len_limit(len: usize) -> Result<(), NoritoFrameError> {
    let limit = norito::core::max_archive_len();
    if limit == u64::MAX {
        return Ok(());
    }
    let len_u64 = len as u64;
    if len_u64 > limit {
        return Err(NoritoFrameError::ArchiveLengthExceeded {
            length: len_u64,
            limit,
        });
    }
    Ok(())
}

#[cfg(feature = "transparent_api")]
#[doc = "Builder utilities for constructing blocks in transparent API mode."]
pub mod builder;
#[doc = "Consensus message types shared by Sumeragi implementations."]
pub mod consensus;
#[doc = "Block header structures and helpers."]
pub mod header;
#[doc = "Payload container types shared between block variants."]
pub mod payload;

pub use header::{BlockHeader as Header, BlockHeader, BlockSignature};
pub use payload::{BlockPayload as Payload, BlockPayload, BlockResult};

#[cfg(feature = "transparent_api")]
use crate::fastpq::TransferTranscript;
#[cfg(feature = "transparent_api")]
use crate::transaction::signed::TransactionResult;
#[cfg(feature = "transparent_api")]
use crate::transaction::signed::TransactionResultInner;
use crate::transaction::signed::{SignedTransaction, TransactionEntrypoint};
#[cfg(feature = "transparent_api")]
use crate::trigger::TimeTriggerEntrypoint;

#[model]
mod model {
    use super::*;

    /// Block collecting signatures from validators.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, IntoSchema, Decode)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct SignedBlock {
        /// Signatures of validators who approved this block.
        pub(super) signatures: BTreeSet<BlockSignature>,
        /// Block payload to be signed.
        pub(super) payload: BlockPayload,
        /// Secondary block state resulting from execution.
        ///
        /// Blocks constructed prior to validation do not carry execution results.
        pub(super) result: Option<BlockResult>,
    }
}

pub use self::model::*;

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
        SignedBlock {
            signatures: [signature].into_iter().collect(),
            payload: BlockPayload {
                header,
                transactions,
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
            },
            result: None,
        }
    }

    /// Create a block with a given signature and an explicit DA commitment bundle.
    ///
    /// The caller is responsible for ensuring `header.da_commitments_hash` matches the supplied
    /// bundle. Signatures are validated against the provided header unchanged.
    #[cfg(feature = "transparent_api")]
    pub fn presigned_with_da(
        signature: BlockSignature,
        header: BlockHeader,
        transactions: Vec<SignedTransaction>,
        da_commitments: Option<DaCommitmentBundle>,
    ) -> SignedBlock {
        SignedBlock {
            signatures: [signature].into_iter().collect(),
            payload: BlockPayload {
                header,
                transactions,
                da_commitments,
                da_proof_policies: None,
                da_pin_intents: None,
            },
            result: None,
        }
    }

    /// Create a block with a given signature and payload.
    #[cfg(feature = "transparent_api")]
    pub fn presigned_with_payload(signature: BlockSignature, payload: BlockPayload) -> SignedBlock {
        SignedBlock {
            signatures: [signature].into_iter().collect(),
            payload,
            result: None,
        }
    }

    /// Set this block's transaction results and update the Merkle roots accordingly.
    #[cfg(feature = "transparent_api")]
    pub fn set_transaction_results(
        &mut self,
        time_triggers: Vec<TimeTriggerEntrypoint>,
        hashes: &[HashOf<TransactionEntrypoint>],
        results: Vec<TransactionResultInner>,
    ) {
        self.set_transaction_results_with_transcripts(
            time_triggers,
            hashes,
            results,
            BTreeMap::new(),
            Vec::new(),
            None,
        );
    }

    /// Set this block's transaction results, including any FASTPQ transfer transcripts, and update the Merkle roots accordingly.
    #[cfg(feature = "transparent_api")]
    pub fn set_transaction_results_with_transcripts(
        &mut self,
        time_triggers: Vec<TimeTriggerEntrypoint>,
        hashes: &[HashOf<TransactionEntrypoint>],
        results: Vec<TransactionResultInner>,
        fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>,
        axt_envelopes: Vec<crate::nexus::AxtEnvelopeRecord>,
        axt_policy_snapshot: Option<crate::nexus::AxtPolicySnapshot>,
    ) {
        let result_hashes = results.iter().map(TransactionResult::hash_from_inner);

        // Merkle tree over all entrypoints (external transactions first, then time triggers).
        let merkle = MerkleTree::from_iter(hashes.to_owned());

        // Ensure the consensus merkle root covering only external transactions remains intact.
        let external_hashes = self
            .payload
            .transactions
            .iter()
            .map(SignedTransaction::hash_as_entrypoint);
        let external_merkle: MerkleTree<TransactionEntrypoint> =
            external_hashes.collect::<MerkleTree<_>>();
        let external_root = external_merkle.root();
        if self.payload.header.merkle_root.is_none() {
            // Allow tests that construct raw headers without setting merkle roots.
            self.payload.header.merkle_root = external_root;
        } else {
            debug_assert_eq!(
                self.payload.header.merkle_root, external_root,
                "consensus merkle root must equal the tree over external transactions"
            );
        }

        let result_merkle: MerkleTree<TransactionResult> = result_hashes.collect();
        let transaction_results = results.into_iter().map(TransactionResult::from).collect();
        self.payload.header.result_merkle_root = result_merkle.root();
        let axt_policy_snapshot =
            axt_policy_snapshot.map(crate::nexus::AxtPolicySnapshot::with_computed_version);
        self.result = Some(BlockResult {
            time_triggers,
            merkle,
            result_merkle,
            transaction_results,
            fastpq_transcripts,
            axt_envelopes,
            axt_policy_snapshot,
        });
    }

    /// Incrementally update a single transaction result at `index`.
    #[cfg(feature = "transparent_api")]
    pub fn update_transaction_result(
        &mut self,
        index: usize,
        result: &TransactionResultInner,
    ) -> bool {
        use crate::transaction::signed::TransactionResult;

        let Some(result_state) = self.result.as_mut() else {
            return false;
        };
        if index >= result_state.transaction_results.len() {
            return false;
        }

        result_state.transaction_results[index] = TransactionResult::from(result.clone());
        let hash = TransactionResult::hash_from_inner(result);
        result_state.result_merkle.update_typed_leaf(index, hash);
        self.payload.header.result_merkle_root = result_state.result_merkle.root();
        true
    }

    /// Produce Merkle proofs for the specified transaction entrypoint hash when available.
    #[must_use]
    pub fn proofs_for_entry_hash(
        &self,
        entry_hash: &HashOf<TransactionEntrypoint>,
    ) -> Option<crate::block::proofs::BlockProofs> {
        let result_state = self.result.as_ref()?;
        let (idx, _) = self
            .entrypoint_hashes()
            .enumerate()
            .find(|(_, hash)| hash == entry_hash)?;
        let idx_u32: u32 = idx.try_into().ok()?;

        let entry_merkle_proof = result_state.merkle.get_proof(idx_u32)?;
        let entry_proof = BlockReceiptProof::new(*entry_hash, entry_merkle_proof);

        let external_count = self.external_transactions().len();
        let entry_root = if idx < external_count {
            self.payload.header.merkle_root?
        } else {
            self.full_entry_merkle_root()?
        };

        let result_root = self.payload.header.result_merkle_root;
        let result_proof = if result_root.is_some() {
            let tx_result = result_state.transaction_results.get(idx)?;
            let result_hash = tx_result.hash();
            let proof = result_state.result_merkle.get_proof(idx_u32)?;
            Some(ExecutionReceiptProof::new(result_hash, proof))
        } else {
            None
        };

        Some(crate::block::proofs::BlockProofs {
            block_height: self.payload.header.height(),
            entry_hash: *entry_hash,
            entry_root,
            entry_proof,
            result_root,
            result_proof,
            fastpq_transcripts: self.fastpq_transcripts().clone(),
        })
    }

    /// Whether execution results are attached to this block.
    #[inline]
    #[must_use]
    pub fn has_results(&self) -> bool {
        self.result.is_some()
    }

    #[inline]
    pub(crate) fn result_ref(&self) -> &BlockResult {
        self.result
            .as_ref()
            .expect("block results are unavailable; block not validated")
    }

    /// Block header
    #[inline]
    pub fn header(&self) -> BlockHeader {
        self.payload.header
    }

    /// Signatures of peers which approved this block.
    #[inline]
    pub fn signatures(
        &self,
    ) -> impl ExactSizeIterator<Item = &BlockSignature> + DoubleEndedIterator {
        self.signatures.iter()
    }

    /// Calculate block hash
    #[inline]
    pub fn hash(&self) -> HashOf<BlockHeader> {
        self.payload.header.hash()
    }

    /// Add additional signature to this block
    #[cfg(feature = "transparent_api")]
    pub fn sign(&mut self, private_key: &iroha_crypto::PrivateKey, signatory: usize) {
        self.signatures.insert(BlockSignature::new(
            signatory as u64,
            SignatureOf::from_hash(private_key, self.payload.header.hash()),
        ));
    }

    /// Add signature to the block
    ///
    /// # Errors
    ///
    /// if signature is invalid
    #[cfg(feature = "transparent_api")]
    pub fn add_signature(&mut self, signature: BlockSignature) -> Result<(), iroha_crypto::Error> {
        if self.signatures().any(|s| signature.index() == s.index()) {
            return Err(iroha_crypto::Error::Signing(
                "Duplicate signature".to_owned(),
            ));
        }

        self.signatures.insert(signature);
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

        signatures.iter().map(BlockSignature::index).try_fold(
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

        Ok(core::mem::replace(&mut self.signatures, signatures))
    }

    /// Creates genesis block signed with the genesis private key (and not signed by any peer).
    ///
    /// `da_commitments` lets the caller embed a [`DaCommitmentBundle`] into the genesis payload once
    /// DA receipts are available during block assembly.
    pub fn genesis(
        transactions: Vec<SignedTransaction>,
        private_key: &iroha_crypto::PrivateKey,
        confidential_features: Option<crate::confidential::ConfidentialFeatureDigest>,
        da_commitments: Option<DaCommitmentBundle>,
    ) -> SignedBlock {
        Self::genesis_with_da_proof_policies(
            transactions,
            private_key,
            confidential_features,
            da_commitments,
            None,
        )
    }

    /// Creates genesis block signed with the genesis private key, overriding DA proof policies.
    ///
    /// `da_commitments` lets the caller embed a [`DaCommitmentBundle`] into the genesis payload once
    /// DA receipts are available during block assembly.
    pub fn genesis_with_da_proof_policies(
        transactions: Vec<SignedTransaction>,
        private_key: &iroha_crypto::PrivateKey,
        confidential_features: Option<crate::confidential::ConfidentialFeatureDigest>,
        da_commitments: Option<DaCommitmentBundle>,
        da_proof_policies: Option<DaProofPolicyBundle>,
    ) -> SignedBlock {
        use nonzero_ext::nonzero;

        let mut entry_merkle = MerkleTree::default();
        for tx in &transactions {
            entry_merkle.add(tx.hash_as_entrypoint());
        }
        let merkle_root = entry_merkle
            .root()
            .expect("Genesis block must have transactions");
        let result_merkle = MerkleTree::default();
        let creation_time_ms = Self::get_genesis_block_creation_time(&transactions);
        let confidential_features = confidential_features.or(Some(
            crate::confidential::DEFAULT_CONFIDENTIAL_FEATURE_DIGEST,
        ));

        let proof_policies = da_proof_policies.unwrap_or_else(|| {
            DaProofPolicyBundle::new(vec![DaProofPolicy {
                lane_id: crate::nexus::LaneId::SINGLE,
                dataspace_id: crate::nexus::DataSpaceId::GLOBAL,
                alias: "default".to_string(),
                proof_scheme: DaProofScheme::MerkleSha256,
            }])
        });
        let proof_policy_hash = HashOf::new(&proof_policies);

        let header = BlockHeader {
            height: nonzero!(1_u64),
            prev_block_hash: None,
            merkle_root: Some(merkle_root),
            result_merkle_root: result_merkle.root(),
            da_proof_policies_hash: Some(proof_policy_hash),
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            creation_time_ms,
            view_change_index: 0,
            confidential_features,
        };

        let signature = BlockSignature::new(0, SignatureOf::from_hash(private_key, header.hash()));
        let payload = BlockPayload {
            header,
            transactions,
            da_commitments: None,
            da_proof_policies: Some(proof_policies),
            da_pin_intents: None,
        };

        let result = BlockResult {
            time_triggers: Vec::new(),
            merkle: entry_merkle,
            result_merkle,
            transaction_results: Vec::new(),
            fastpq_transcripts: BTreeMap::new(),
            axt_envelopes: Vec::new(),
            axt_policy_snapshot: None,
        };
        let mut block = SignedBlock {
            signatures: [signature].into_iter().collect(),
            payload,
            result: Some(result),
        };
        block.set_da_commitments(da_commitments);
        block
    }

    /// Serialize this block into a canonical Norito wire frame (version byte + header + payload).
    ///
    /// # Errors
    /// Returns [`NoritoFrameError`] if constructing the canonical frame header fails.
    pub fn encode_wire(&self) -> Result<Vec<u8>, NoritoFrameError> {
        self.canonical_wire().map(SignedBlockWire::into_vec)
    }

    /// Obtain the canonical Norito wire helper for inspecting or framing this block.
    ///
    /// # Errors
    /// Returns [`NoritoFrameError`] if the Norito header cannot be emitted with the collected
    /// encode flags.
    pub fn canonical_wire(&self) -> Result<SignedBlockWire, NoritoFrameError> {
        let payload = encode_signed_block_payload(self);
        let version = self.version();

        let mut versioned = Vec::with_capacity(1 + payload.len());
        versioned.push(version);
        versioned.extend_from_slice(&payload);

        let mut frame = Vec::with_capacity(1 + norito::core::Header::SIZE + payload.len());
        frame.push(version);
        write_signed_block_header(&payload, &mut frame)?;
        frame.extend_from_slice(&payload);

        Ok(SignedBlockWire { frame, versioned })
    }

    fn get_genesis_block_creation_time(transactions: &[SignedTransaction]) -> u64 {
        let latest_txn_time = transactions
            .iter()
            .map(SignedTransaction::creation_time)
            .max()
            .expect("INTERNAL BUG: Genesis block is empty");
        let creation_time = latest_txn_time + Duration::from_millis(1);
        creation_time
            .as_millis()
            .try_into()
            .expect("INTERNAL BUG: Unix timestamp exceedes u64::MAX")
    }
}

impl fmt::Display for SignedBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.header())
    }
}

impl iroha_version::Version for SignedBlock {
    fn version(&self) -> u8 {
        1
    }

    fn supported_versions() -> std::ops::Range<u8> {
        1..2
    }
}

impl iroha_version::codec::EncodeVersioned for SignedBlock {
    fn encode_versioned(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(1);
        bytes.push(self.version());
        let payload = norito::codec::encode_adaptive(self);
        bytes.extend(payload);
        bytes
    }
}

impl iroha_version::codec::DecodeVersioned for SignedBlock {
    fn decode_all_versioned(input: &[u8]) -> iroha_version::error::Result<Self> {
        decode_versioned_signed_block_inner(input, input)
    }
}

#[cfg(feature = "http")]
pub mod stream {
    //! Blocks for streaming API.

    use std::{io::Write, num::NonZeroU64, sync::Arc};

    use iroha_schema::IntoSchema;
    use norito::{
        codec::{Decode, Encode},
        core::{Error as NoritoError, NoritoSerialize},
    };

    pub use self::model::*;
    use super::*;

    #[model]
    mod model {
        use std::num::NonZeroU64;

        use super::*;

        /// Request sent to subscribe to blocks stream starting from the given height.
        #[derive(Debug, Clone, Copy, Decode, Encode, IntoSchema)]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[repr(transparent)]
        pub struct BlockSubscriptionRequest(pub NonZeroU64);

        /// Message sent by the stream producer containing block.
        #[derive(Debug, Clone, Decode, Encode, IntoSchema)]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[repr(transparent)]
        pub struct BlockMessage(pub SignedBlock);
    }

    impl From<BlockMessage> for SignedBlock {
        fn from(source: BlockMessage) -> Self {
            source.0
        }
    }

    /// Message sent by the stream producer containing block by shared ownership
    /// without requiring an additional clone of the block data.
    #[derive(Debug, Clone)]
    #[repr(transparent)]
    pub struct BlockMessageSend(pub Arc<SignedBlock>);

    impl NoritoSerialize for BlockMessageSend {
        fn schema_hash() -> [u8; 16] {
            <BlockMessage as NoritoSerialize>::schema_hash()
        }

        fn serialize<W: Write>(&self, writer: W) -> Result<(), NoritoError> {
            // Serialize as a BlockMessage wrapper to keep schema and layout consistent
            let msg = BlockMessage(self.0.as_ref().clone());
            NoritoSerialize::serialize(&msg, writer)
        }
    }

    /// Exports common structs and enums from this module.
    pub mod prelude {
        pub use super::{BlockMessage, BlockMessageSend, BlockSubscriptionRequest};
    }

    impl BlockSubscriptionRequest {
        /// Create a new [`BlockSubscriptionRequest`].
        pub fn new(height: NonZeroU64) -> Self {
            Self(height)
        }
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
            Clone,
            Copy,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            iroha_macro::FromVariant,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub enum BlockRejectionReason {
            /// Block was rejected during consensus.
            ConsensusBlockRejection,
            /// Block contains transactions already committed.
            ContainsCommittedTransactions,
            /// Block violated the non-empty block policy.
            EmptyBlock,
            /// Previous block hash does not match local expectation.
            PrevBlockHashMismatch,
            /// Previous block height does not match local expectation.
            PrevBlockHeightMismatch,
            /// Merkle root of block contents is invalid.
            MerkleRootMismatch,
            /// Transaction in the block failed admission or validation.
            TransactionValidationFailed,
            /// Block signatures do not match current topology.
            TopologyMismatch,
            /// Block is missing required number of valid signatures.
            InsufficientBlockSignatures,
            /// Block contains signature from an unknown peer.
            UnknownBlockSignatory,
            /// Block signatory does not have an active consensus key for this height/role.
            InactiveConsensusKey,
            /// Block contains an invalid signature payload.
            InvalidBlockSignature,
            /// Block is missing proxy tail signature.
            ProxyTailSignatureMissing,
            /// Block is missing leader signature.
            LeaderSignatureMissing,
            /// Block signature check failed for an unspecified reason.
            OtherSignatureError,
            /// Genesis block validation failed.
            InvalidGenesis,
            /// Block creation time is earlier than previous block.
            BlockInThePast,
            /// Block creation time is in the future relative to local clock.
            BlockInTheFuture,
            /// Block contains transactions created in the future.
            TransactionInTheFuture,
            /// Confidential feature digest does not match local expectation.
            ConfidentialFeatureDigestMismatch,
            /// Proof policy hash does not match the configured lane catalog.
            DaProofPolicyMismatch,
            /// DA shard cursor was missing or regressed.
            DaShardCursorViolation,
        }
    }

    #[cfg(feature = "json")]
    impl norito::json::FastJsonWrite for BlockRejectionReason {
        fn write_json(&self, out: &mut String) {
            match self {
                BlockRejectionReason::ConsensusBlockRejection => {
                    norito::json::write_json_string("ConsensusBlockRejection", out);
                }
                BlockRejectionReason::ContainsCommittedTransactions => {
                    norito::json::write_json_string("ContainsCommittedTransactions", out);
                }
                BlockRejectionReason::EmptyBlock => {
                    norito::json::write_json_string("EmptyBlock", out);
                }
                BlockRejectionReason::PrevBlockHashMismatch => {
                    norito::json::write_json_string("PrevBlockHashMismatch", out);
                }
                BlockRejectionReason::PrevBlockHeightMismatch => {
                    norito::json::write_json_string("PrevBlockHeightMismatch", out);
                }
                BlockRejectionReason::MerkleRootMismatch => {
                    norito::json::write_json_string("MerkleRootMismatch", out);
                }
                BlockRejectionReason::TransactionValidationFailed => {
                    norito::json::write_json_string("TransactionValidationFailed", out);
                }
                BlockRejectionReason::TopologyMismatch => {
                    norito::json::write_json_string("TopologyMismatch", out);
                }
                BlockRejectionReason::InsufficientBlockSignatures => {
                    norito::json::write_json_string("InsufficientBlockSignatures", out);
                }
                BlockRejectionReason::UnknownBlockSignatory => {
                    norito::json::write_json_string("UnknownBlockSignatory", out);
                }
                BlockRejectionReason::InactiveConsensusKey => {
                    norito::json::write_json_string("InactiveConsensusKey", out);
                }
                BlockRejectionReason::InvalidBlockSignature => {
                    norito::json::write_json_string("InvalidBlockSignature", out);
                }
                BlockRejectionReason::ProxyTailSignatureMissing => {
                    norito::json::write_json_string("ProxyTailSignatureMissing", out);
                }
                BlockRejectionReason::LeaderSignatureMissing => {
                    norito::json::write_json_string("LeaderSignatureMissing", out);
                }
                BlockRejectionReason::OtherSignatureError => {
                    norito::json::write_json_string("OtherSignatureError", out);
                }
                BlockRejectionReason::InvalidGenesis => {
                    norito::json::write_json_string("InvalidGenesis", out);
                }
                BlockRejectionReason::BlockInThePast => {
                    norito::json::write_json_string("BlockInThePast", out);
                }
                BlockRejectionReason::BlockInTheFuture => {
                    norito::json::write_json_string("BlockInTheFuture", out);
                }
                BlockRejectionReason::TransactionInTheFuture => {
                    norito::json::write_json_string("TransactionInTheFuture", out);
                }
                BlockRejectionReason::ConfidentialFeatureDigestMismatch => {
                    norito::json::write_json_string("ConfidentialFeatureDigestMismatch", out);
                }
                BlockRejectionReason::DaProofPolicyMismatch => {
                    norito::json::write_json_string("DaProofPolicyMismatch", out);
                }
                BlockRejectionReason::DaShardCursorViolation => {
                    norito::json::write_json_string("DaShardCursorViolation", out);
                }
            }
        }
    }

    #[cfg(feature = "json")]
    impl norito::json::JsonDeserialize for BlockRejectionReason {
        fn json_deserialize(
            parser: &mut norito::json::Parser<'_>,
        ) -> Result<Self, norito::json::Error> {
            let value = parser.parse_string()?;
            match value.as_str() {
                "ConsensusBlockRejection" => Ok(BlockRejectionReason::ConsensusBlockRejection),
                "ContainsCommittedTransactions" => {
                    Ok(BlockRejectionReason::ContainsCommittedTransactions)
                }
                "EmptyBlock" => Ok(BlockRejectionReason::EmptyBlock),
                "PrevBlockHashMismatch" => Ok(BlockRejectionReason::PrevBlockHashMismatch),
                "PrevBlockHeightMismatch" => Ok(BlockRejectionReason::PrevBlockHeightMismatch),
                "MerkleRootMismatch" => Ok(BlockRejectionReason::MerkleRootMismatch),
                "TransactionValidationFailed" => {
                    Ok(BlockRejectionReason::TransactionValidationFailed)
                }
                "TopologyMismatch" => Ok(BlockRejectionReason::TopologyMismatch),
                "InsufficientBlockSignatures" => {
                    Ok(BlockRejectionReason::InsufficientBlockSignatures)
                }
                "UnknownBlockSignatory" => Ok(BlockRejectionReason::UnknownBlockSignatory),
                "InactiveConsensusKey" => Ok(BlockRejectionReason::InactiveConsensusKey),
                "InvalidBlockSignature" => Ok(BlockRejectionReason::InvalidBlockSignature),
                "ProxyTailSignatureMissing" => Ok(BlockRejectionReason::ProxyTailSignatureMissing),
                "LeaderSignatureMissing" => Ok(BlockRejectionReason::LeaderSignatureMissing),
                "OtherSignatureError" => Ok(BlockRejectionReason::OtherSignatureError),
                "InvalidGenesis" => Ok(BlockRejectionReason::InvalidGenesis),
                "BlockInThePast" => Ok(BlockRejectionReason::BlockInThePast),
                "BlockInTheFuture" => Ok(BlockRejectionReason::BlockInTheFuture),
                "TransactionInTheFuture" => Ok(BlockRejectionReason::TransactionInTheFuture),
                "ConfidentialFeatureDigestMismatch" => {
                    Ok(BlockRejectionReason::ConfidentialFeatureDigestMismatch)
                }
                "DaProofPolicyMismatch" => Ok(BlockRejectionReason::DaProofPolicyMismatch),
                "DaShardCursorViolation" => Ok(BlockRejectionReason::DaShardCursorViolation),
                other => Err(norito::json::Error::unknown_field(other)),
            }
        }
    }

    impl std::error::Error for BlockRejectionReason {}
}

impl fmt::Display for error::BlockRejectionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            error::BlockRejectionReason::ConsensusBlockRejection => {
                f.write_str("Block was rejected during consensus")
            }
            error::BlockRejectionReason::ContainsCommittedTransactions => {
                f.write_str("Block contains transactions already committed")
            }
            error::BlockRejectionReason::EmptyBlock => {
                f.write_str("Block contained no committed overlays")
            }
            error::BlockRejectionReason::PrevBlockHashMismatch => {
                f.write_str("Previous block hash mismatch")
            }
            error::BlockRejectionReason::PrevBlockHeightMismatch => {
                f.write_str("Previous block height mismatch")
            }
            error::BlockRejectionReason::MerkleRootMismatch => f.write_str("Merkle root mismatch"),
            error::BlockRejectionReason::TransactionValidationFailed => {
                f.write_str("Transaction validation failed during block processing")
            }
            error::BlockRejectionReason::TopologyMismatch => {
                f.write_str("Block signatures do not match network topology")
            }
            error::BlockRejectionReason::InsufficientBlockSignatures => {
                f.write_str("Block lacks sufficient valid signatures")
            }
            error::BlockRejectionReason::UnknownBlockSignatory => {
                f.write_str("Block signed by unknown peer")
            }
            error::BlockRejectionReason::InactiveConsensusKey => {
                f.write_str("Block signatory does not have an active consensus key")
            }
            error::BlockRejectionReason::InvalidBlockSignature => {
                f.write_str("Block signature is invalid")
            }
            error::BlockRejectionReason::ProxyTailSignatureMissing => {
                f.write_str("Block missing proxy tail signature")
            }
            error::BlockRejectionReason::LeaderSignatureMissing => {
                f.write_str("Block missing leader signature")
            }
            error::BlockRejectionReason::OtherSignatureError => {
                f.write_str("Block signature verification failed for unspecified reason")
            }
            error::BlockRejectionReason::InvalidGenesis => f.write_str("Invalid genesis block"),
            error::BlockRejectionReason::BlockInThePast => {
                f.write_str("Block creation time is earlier than previous block")
            }
            error::BlockRejectionReason::BlockInTheFuture => {
                f.write_str("Block creation time is in the future relative to local clock")
            }
            error::BlockRejectionReason::TransactionInTheFuture => {
                f.write_str("Block contains transactions created in the future")
            }
            error::BlockRejectionReason::ConfidentialFeatureDigestMismatch => {
                f.write_str("Confidential feature digest mismatch")
            }
            error::BlockRejectionReason::DaProofPolicyMismatch => {
                f.write_str("DA proof policy bundle hash mismatch")
            }
            error::BlockRejectionReason::DaShardCursorViolation => {
                f.write_str("DA shard cursor regression or unknown lane")
            }
        }
    }
}

/// Prefix versioned [`SignedBlock`] bytes with a Norito header using the default
/// framing flags. The returned buffer contains the original version byte
/// followed by the framed payload.
///
/// # Errors
///
/// Returns [`NoritoFrameError::LengthMismatch`] if `versioned` is empty. Propagates
/// header validation and synthesis failures from the internal
/// `validate_signed_block_header` and `write_signed_block_header` helpers.
pub fn frame_versioned_signed_block_bytes(versioned: &[u8]) -> Result<Vec<u8>, NoritoFrameError> {
    if versioned.is_empty() {
        return Err(NoritoFrameError::LengthMismatch);
    }
    let version = versioned[0];
    let payload = &versioned[1..];
    enforce_payload_len_limit(payload.len())?;

    if payload.starts_with(MAGIC.as_slice()) {
        validate_signed_block_header(payload)?;
        return Ok(versioned.to_vec());
    }

    let mut out = Vec::with_capacity(1 + norito::core::Header::SIZE + payload.len());
    out.push(version);
    write_signed_block_header(payload, &mut out)?;
    out.extend_from_slice(payload);
    Ok(out)
}

/// Canonical wire representation of a [`SignedBlock`], exposing both the framed and bare bytes.
#[derive(Clone)]
pub struct SignedBlockWire {
    frame: Vec<u8>,
    versioned: Vec<u8>,
}

impl SignedBlockWire {
    /// Serialize into an owned `(frame, versioned)` pair, transferring ownership of the buffers.
    pub fn into_parts(self) -> (Vec<u8>, Vec<u8>) {
        (self.frame, self.versioned)
    }

    /// Consume the wire representation, returning the framed bytes.
    pub fn into_vec(self) -> Vec<u8> {
        self.frame
    }

    /// Borrowing accessor for the framed bytes (version byte + Norito header + payload).
    pub fn as_framed(&self) -> &[u8] {
        &self.frame
    }

    /// Clone the framed bytes into an owned vector.
    pub fn to_vec(&self) -> Vec<u8> {
        self.frame.clone()
    }

    /// Extract the bare versioned payload (version discriminator + Norito payload).
    pub fn as_versioned(&self) -> &[u8] {
        &self.versioned
    }

    /// Return the version byte encoded in this frame.
    pub fn version(&self) -> u8 {
        self.versioned
            .first()
            .copied()
            .expect("canonical wire always contains a version byte")
    }

    /// Borrow the Norito payload bytes (without version byte or header).
    pub fn payload(&self) -> &[u8] {
        let header_size = norito::core::Header::SIZE;
        &self.frame[1 + header_size..]
    }
}

/// Ensure versioned [`SignedBlock`] bytes carry a Norito header. When the
/// payload is already framed, the returned `bytes` borrow the input slice.
/// Headerless payloads are rejected.
///
/// # Errors
///
/// Returns [`NoritoFrameError::LengthMismatch`] when `bytes` is empty. Propagates
/// header validation failures from `validate_signed_block_header` and issues
/// encountered while synthesizing the framed payload.
#[derive(Debug)]
pub struct DeframedSignedBlockBytes<'a> {
    /// Versioned bytes with a Norito header prefixing the payload.
    pub bytes: Cow<'a, [u8]>,
    /// Versioned bytes without the Norito header (version discriminator + bare payload).
    pub bare_versioned: Cow<'a, [u8]>,
}

/// Deframe versioned [`SignedBlock`] bytes, requiring a Norito header.
///
/// # Errors
///
/// Returns [`NoritoFrameError::LengthMismatch`] when `bytes` is empty and
/// propagates header validation or synthesis failures.
pub fn deframe_versioned_signed_block_bytes(
    bytes: &[u8],
) -> Result<DeframedSignedBlockBytes<'_>, NoritoFrameError> {
    if bytes.is_empty() {
        return Err(NoritoFrameError::LengthMismatch);
    }
    let version = bytes[0];
    let payload = &bytes[1..];
    if payload.starts_with(MAGIC.as_slice()) {
        validate_signed_block_header(payload)?;
        let header_size = norito::core::Header::SIZE;
        let bare_payload = &payload[header_size..];
        let mut bare_versioned = Vec::with_capacity(1 + bare_payload.len());
        bare_versioned.push(version);
        bare_versioned.extend_from_slice(bare_payload);
        enforce_payload_len_limit(bare_payload.len())?;
        Ok(DeframedSignedBlockBytes {
            bytes: Cow::Borrowed(bytes),
            bare_versioned: Cow::Owned(bare_versioned),
        })
    } else {
        Err(NoritoFrameError::InvalidMagic)
    }
}

fn write_signed_block_header(payload: &[u8], out: &mut Vec<u8>) -> Result<(), NoritoFrameError> {
    enforce_payload_len_limit(payload.len())?;
    out.extend_from_slice(MAGIC.as_slice());
    out.push(VERSION_MAJOR);
    out.push(VERSION_MINOR);
    out.extend_from_slice(&<SignedBlock as norito::NoritoSerialize>::schema_hash());
    out.push(Compression::None as u8);
    let len = u64::try_from(payload.len()).map_err(|_| NoritoFrameError::LengthMismatch)?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&norito_crc64(payload).to_le_bytes());

    #[cfg(debug_assertions)]
    if norito::debug_trace_enabled() {
        eprintln!(
            "write_signed_block_header default_flags=0x{:02x}",
            default_encode_flags()
        );
    }
    let encode_flags = default_encode_flags();
    #[cfg(debug_assertions)]
    if norito::debug_trace_enabled() {
        eprintln!("write_signed_block_header flags=0x{encode_flags:02x}");
    }
    out.push(encode_flags);
    Ok(())
}

fn validate_signed_block_header(payload: &[u8]) -> Result<(), NoritoFrameError> {
    let header_size = norito::core::Header::SIZE;
    if payload.len() < header_size {
        return Err(NoritoFrameError::LengthMismatch);
    }
    let major = payload[4];
    if major != VERSION_MAJOR {
        return Err(NoritoFrameError::UnsupportedVersion {
            found: major,
            expected: VERSION_MAJOR,
        });
    }
    let minor = payload[5];
    if minor != VERSION_MINOR {
        return Err(NoritoFrameError::UnsupportedMinorVersion {
            found: minor,
            supported: VERSION_MINOR,
        });
    }
    let mut schema_bytes = [0u8; 16];
    schema_bytes.copy_from_slice(&payload[6..22]);
    if schema_bytes != <SignedBlock as norito::NoritoSerialize>::schema_hash() {
        return Err(NoritoFrameError::SchemaMismatch);
    }
    let compression = payload[22];
    if compression != Compression::None as u8 {
        return Err(NoritoFrameError::UnsupportedCompression {
            found: compression,
            supported: &[Compression::None],
        });
    }
    let mut length_bytes = [0u8; 8];
    length_bytes.copy_from_slice(&payload[23..31]);
    let expected_len_u64 = u64::from_le_bytes(length_bytes);
    let limit = norito::core::max_archive_len();
    if expected_len_u64 > limit {
        return Err(NoritoFrameError::ArchiveLengthExceeded {
            length: expected_len_u64,
            limit,
        });
    }
    let expected_len: usize = expected_len_u64
        .try_into()
        .map_err(|_| NoritoFrameError::LengthMismatch)?;
    let mut checksum_bytes = [0u8; 8];
    checksum_bytes.copy_from_slice(&payload[31..39]);
    let checksum = u64::from_le_bytes(checksum_bytes);
    let bare_payload = &payload[header_size..];
    if bare_payload.len() != expected_len {
        return Err(NoritoFrameError::LengthMismatch);
    }
    if norito_crc64(bare_payload) != checksum {
        return Err(NoritoFrameError::ChecksumMismatch);
    }
    let flags = payload[header_size - 1];
    let expected_flags = default_encode_flags();
    if flags != expected_flags {
        return Err(NoritoFrameError::UnsupportedFeature("layout flag"));
    }
    Ok(())
}

#[cfg(test)]
fn decode_field<T>(bytes: &[u8]) -> Result<(T, &[u8]), NoritoFrameError>
where
    T: for<'de> norito::NoritoDeserialize<'de> + norito::NoritoSerialize,
{
    if bytes.len() < 8 {
        return Err(NoritoFrameError::LengthMismatch);
    }

    let mut len_prefix = [0u8; 8];
    len_prefix.copy_from_slice(&bytes[..8]);
    let field_len: usize = u64::from_le_bytes(len_prefix)
        .try_into()
        .map_err(|_| NoritoFrameError::LengthMismatch)?;

    let payload = bytes
        .get(8..8 + field_len)
        .ok_or(NoritoFrameError::LengthMismatch)?;

    let (value, used) = norito::core::decode_field_canonical::<T>(payload)?;
    if used != field_len {
        return Err(NoritoFrameError::LengthMismatch);
    }

    let rest = &bytes[8 + field_len..];
    Ok((value, rest))
}

/// Decode a versioned [`SignedBlock`] payload produced by the canonical frame
/// (version byte + Norito header + payload).
///
/// # Errors
///
/// Returns [`iroha_version::error::Error`] if the bytes are malformed, carry an
/// unsupported version, or fail Norito decoding.
pub fn decode_versioned_signed_block(
    bytes: &[u8],
) -> Result<SignedBlock, iroha_version::error::Error> {
    use iroha_version::error::Error as VersionError;
    if bytes.is_empty() {
        return Err(VersionError::NotVersioned);
    }

    let deframed = deframe_versioned_signed_block_bytes(bytes)
        .map_err(|err| VersionError::NoritoCodec(err.to_string()))?;

    decode_versioned_signed_block_inner(deframed.bare_versioned.as_ref(), deframed.bytes.as_ref())
}

/// Decode a Norito-framed [`SignedBlock`] payload, validating the header before
/// delegating to [`decode_versioned_signed_block`].
///
/// # Errors
///
/// Returns [`iroha_version::error::Error`] when the payload is too short to
/// carry a version tag, fails Norito header validation, or does not decode into
/// a valid [`SignedBlock`].
pub fn decode_framed_signed_block(
    bytes: &[u8],
) -> Result<SignedBlock, iroha_version::error::Error> {
    use iroha_version::error::Error as VersionError;

    if bytes.len() <= 1 {
        return Err(VersionError::NotVersioned);
    }

    let deframed = deframe_versioned_signed_block_bytes(bytes)
        .map_err(|err| VersionError::NoritoCodec(err.to_string()))?;

    decode_versioned_signed_block_inner(deframed.bare_versioned.as_ref(), deframed.bytes.as_ref())
}

struct RootGuard;

impl Drop for RootGuard {
    fn drop(&mut self) {
        norito::core::clear_decode_root();
    }
}

type VersionError = iroha_version::error::Error;

fn decode_signed_block_exact(bytes: &[u8]) -> Result<SignedBlock, iroha_version::error::Error> {
    norito::core::set_decode_root(bytes);
    let _root_guard = RootGuard;
    let (block, used) = <SignedBlock as norito::core::DecodeFromSlice>::decode_from_slice(bytes)
        .map_err(|err| VersionError::NoritoCodec(err.to_string()))?;
    let remaining = bytes.len().saturating_sub(used);
    if remaining != 0 {
        return Err(VersionError::ExtraBytesLeft(remaining as u64));
    }
    Ok(block)
}

fn encode_signed_block_payload(block: &SignedBlock) -> Vec<u8> {
    norito::core::reset_decode_state();
    norito::codec::encode_adaptive(block)
}

fn decode_versioned_signed_block_inner(
    bare_versioned: &[u8],
    raw_for_error: &[u8],
) -> Result<SignedBlock, iroha_version::error::Error> {
    use iroha_version::error::Error as VersionError;

    if bare_versioned.is_empty() {
        return Err(VersionError::NotVersioned);
    }

    let version = bare_versioned[0];
    let payload = &bare_versioned[1..];
    match version {
        1 => decode_signed_block_exact(payload),
        other => Err(VersionError::UnsupportedVersion(Box::new(
            UnsupportedVersion::new(
                other,
                iroha_version::RawVersioned::NoritoBytes(raw_for_error.to_vec()),
            ),
        ))),
    }
}

pub mod prelude {
    //! For glob-import
    pub use super::{BlockHeader, BlockSignature, SignedBlock, error::BlockRejectionReason};
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use iroha_crypto::{Hash, HashOf, Signature};
    use iroha_version::codec::{DecodeVersioned, EncodeVersioned};
    use norito::codec::Encode;

    use super::*;
    // Bring commonly used types referenced in transparent API tests.
    #[cfg(feature = "transparent_api")]
    use crate::ValidationFail;
    #[cfg(feature = "transparent_api")]
    use crate::transaction::signed::{TransactionEntrypoint, TransactionResultInner};
    #[cfg(feature = "transparent_api")]
    use crate::trigger::DataTriggerSequence;
    #[cfg(feature = "transparent_api")]
    use crate::trigger::TimeTriggerEntrypoint;
    use crate::{
        da::{
            commitment::{DaCommitmentBundle, DaCommitmentRecord, DaProofScheme, KzgCommitment},
            pin_intent::{DaPinIntent, DaPinIntentBundle},
            types::{BlobDigest, RetentionPolicy, StorageTicketId},
        },
        nexus::LaneId,
        query::dsl::{HasProjection, PredicateMarker, SelectorMarker},
        sorafs::pin_registry::ManifestDigest,
    };

    fn assert_predicate<T: HasProjection<PredicateMarker>>() {}
    fn assert_selector<T: HasProjection<SelectorMarker>>() {}

    fn sample_da_bundle() -> DaCommitmentBundle {
        let record = DaCommitmentRecord::new(
            LaneId::new(7),
            1,
            1,
            BlobDigest::new([0x11; 32]),
            ManifestDigest::new([0x22; 32]),
            DaProofScheme::MerkleSha256,
            Hash::prehashed([0x33; 32]),
            Some(KzgCommitment::new([0x44; 48])),
            Some(Hash::prehashed([0x55; 32])),
            RetentionPolicy::default(),
            StorageTicketId::new([0x66; 32]),
            Signature::from_bytes(&[0x77; 64]),
        );
        DaCommitmentBundle::new(vec![record])
    }

    #[test]
    fn signed_block_is_empty_without_entrypoints_or_artifacts() {
        let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
        let block = SignedBlock {
            signatures: BTreeSet::new(),
            payload: BlockPayload {
                header,
                transactions: Vec::new(),
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
            },
            result: None,
        };

        assert!(block.is_empty());
    }

    #[test]
    #[cfg(feature = "transparent_api")]
    fn presigned_with_payload_preserves_payload_and_signature() {
        let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
        let payload = BlockPayload {
            header,
            transactions: Vec::new(),
            da_commitments: None,
            da_proof_policies: None,
            da_pin_intents: None,
        };
        let key_pair =
            iroha_crypto::KeyPair::random_with_algorithm(iroha_crypto::Algorithm::BlsNormal);
        let signature = BlockSignature::new(
            0,
            iroha_crypto::SignatureOf::from_hash(key_pair.private_key(), payload.header.hash()),
        );

        let block = SignedBlock::presigned_with_payload(signature.clone(), payload.clone());

        assert_eq!(block.header(), payload.header);
        assert_eq!(block.transactions_vec(), &payload.transactions);
        assert!(block.signatures().any(|sig| sig == &signature));
    }

    #[test]
    fn signed_block_is_not_empty_with_time_triggers() {
        let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
        let authority = crate::account::AccountId::new(
            "wonderland".parse().expect("domain id"),
            "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245"
                .parse()
                .expect("public key"),
        );
        let entrypoint = crate::trigger::TimeTriggerEntrypoint {
            id: "time_trigger".parse().expect("trigger id parses"),
            instructions: crate::transaction::ExecutionStep(
                iroha_primitives::const_vec::ConstVec::new_empty(),
            ),
            authority,
        };
        let entry_hash = entrypoint.hash_as_entrypoint();
        let mut entry_merkle = iroha_crypto::MerkleTree::default();
        entry_merkle.add(entry_hash);

        let result_inner = crate::transaction::signed::TransactionResultInner::Ok(
            crate::trigger::DataTriggerSequence::default(),
        );
        let mut result_merkle = iroha_crypto::MerkleTree::default();
        result_merkle
            .add(crate::transaction::signed::TransactionResult::hash_from_inner(&result_inner));

        let result = BlockResult {
            time_triggers: vec![entrypoint],
            merkle: entry_merkle,
            result_merkle,
            transaction_results: vec![crate::transaction::signed::TransactionResult::from(
                result_inner,
            )],
            fastpq_transcripts: std::collections::BTreeMap::new(),
            axt_envelopes: Vec::new(),
            axt_policy_snapshot: None,
        };

        let block = SignedBlock {
            signatures: BTreeSet::new(),
            payload: BlockPayload {
                header,
                transactions: Vec::new(),
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
            },
            result: Some(result),
        };

        assert!(!block.is_empty());
    }

    #[test]
    fn signed_block_is_not_empty_with_da_commitments() {
        let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
        let mut block = SignedBlock {
            signatures: BTreeSet::new(),
            payload: BlockPayload {
                header,
                transactions: Vec::new(),
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
            },
            result: None,
        };

        block.set_da_commitments(Some(sample_da_bundle()));

        assert!(!block.is_empty());
    }

    #[test]
    fn signed_block_is_not_empty_with_da_pin_intents() {
        let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
        let mut block = SignedBlock {
            signatures: BTreeSet::new(),
            payload: BlockPayload {
                header,
                transactions: Vec::new(),
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
            },
            result: None,
        };

        let intent = DaPinIntent::new(
            LaneId::new(7),
            9,
            11,
            StorageTicketId::new([0xAB; 32]),
            ManifestDigest::new([0xCD; 32]),
        );
        let bundle = DaPinIntentBundle::new(vec![intent]);
        block.set_da_pin_intents(Some(bundle));

        assert!(!block.is_empty());
    }

    #[test]
    fn block_header_has_projection_impls() {
        assert_predicate::<BlockHeader>();
        assert_selector::<BlockHeader>();
    }

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
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            creation_time_ms: 123_456_789_000,
            view_change_index: 123,
            confidential_features: None,
        };
        let hash0 = header.hash();
        header.result_merkle_root = Some(HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
            b"result_merkle_root",
        )));
        let hash1 = header.hash();
        assert_eq!(hash0, hash1);
    }

    #[test]
    fn block_header_new_and_display() {
        let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
        assert_eq!(header.to_string(), format!("{} (№1)", header.hash()));
    }

    #[test]
    fn genesis_defaults_confidential_digest() {
        use iroha_crypto::KeyPair;

        use crate::{
            ChainId, account::AccountId, domain::DomainId, transaction::signed::TransactionBuilder,
        };

        let chain: ChainId = "genesis-default-conf-digest".parse().expect("chain id");
        let keypair = KeyPair::random();
        let domain: DomainId = "genesis".parse().expect("domain id");
        let authority = AccountId::new(domain, keypair.public_key().clone());
        let tx = TransactionBuilder::new(chain, authority).sign(keypair.private_key());
        let block = SignedBlock::genesis(vec![tx], keypair.private_key(), None, None);
        assert_eq!(
            block.header().confidential_features(),
            Some(crate::confidential::DEFAULT_CONFIDENTIAL_FEATURE_DIGEST)
        );
    }

    #[test]
    fn encode_versioned_prefixes_norito_payload() {
        use nonzero_ext::nonzero;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let block = SignedBlock {
            signatures: BTreeSet::new(),
            payload: BlockPayload {
                header,
                transactions: Vec::new(),
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
            },
            result: None,
        };

        let versioned = block.encode_versioned();
        assert!(!versioned.is_empty());
        assert_eq!(versioned[0], block.version());
        let framed = frame_versioned_signed_block_bytes(&versioned).expect("frame versioned block");
        let deframed = deframe_versioned_signed_block_bytes(&framed).expect("deframe framed block");
        assert_eq!(deframed.bare_versioned.as_ref(), versioned.as_slice());
        assert!(deframed.bytes.as_ref()[1..].starts_with(MAGIC.as_slice()));
    }

    #[test]
    fn deframe_rejects_payload_exceeding_max_len() {
        use nonzero_ext::nonzero;
        const LENGTH_OFFSET: usize = 1 + 4 + 1 + 1 + 16 + 1;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let block = SignedBlock {
            signatures: BTreeSet::new(),
            payload: BlockPayload {
                header,
                transactions: Vec::new(),
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
            },
            result: None,
        };

        let versioned = block.encode_versioned();
        let mut framed =
            frame_versioned_signed_block_bytes(&versioned).expect("frame versioned payload");

        let limit = norito::core::max_archive_len();
        let oversized_len = limit
            .checked_add(1)
            .expect("max archive len must be finite for tests");
        // Patch the length field inside the Norito header (after version + magic + major + minor + schema + compression).
        let length_end = LENGTH_OFFSET + core::mem::size_of::<u64>();
        framed[LENGTH_OFFSET..length_end].copy_from_slice(&oversized_len.to_le_bytes());

        let err = deframe_versioned_signed_block_bytes(&framed)
            .expect_err("payload should exceed enforced Norito length cap");
        match err {
            NoritoFrameError::ArchiveLengthExceeded {
                length,
                limit: enforced,
            } => {
                assert_eq!(length, oversized_len);
                assert_eq!(enforced, limit);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn decode_versioned_signed_block_rejects_trailing_bytes() {
        use nonzero_ext::nonzero;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let block = SignedBlock {
            signatures: BTreeSet::new(),
            payload: BlockPayload {
                header,
                transactions: Vec::new(),
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
            },
            result: None,
        };

        let mut versioned = block.encode_versioned();
        versioned.push(0_u8);

        let err = SignedBlock::decode_all_versioned(versioned.as_slice())
            .expect_err("decode must report trailing bytes");
        match err {
            iroha_version::error::Error::ExtraBytesLeft(remaining) => {
                assert_eq!(remaining, 1);
            }
            iroha_version::error::Error::NoritoCodec(reason) => {
                assert!(
                    reason.contains("length mismatch"),
                    "unexpected norito error: {reason}"
                );
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn frame_deframe_versioned_bytes_roundtrip() {
        use nonzero_ext::nonzero;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let block = SignedBlock {
            signatures: BTreeSet::new(),
            payload: BlockPayload {
                header,
                transactions: Vec::new(),
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
            },
            result: None,
        };

        let versioned = block.encode_versioned();
        let framed = frame_versioned_signed_block_bytes(&versioned).expect("frame versioned block");

        assert!(framed.len() > versioned.len());
        assert!(framed[1..].starts_with(MAGIC.as_slice()));
        let deframed = deframe_versioned_signed_block_bytes(&framed).expect("deframe framed block");
        assert_eq!(deframed.bytes.as_ref(), framed.as_slice());
        assert_eq!(deframed.bare_versioned.as_ref(), versioned.as_slice());
        let decoded_framed = decode_framed_signed_block(&framed).expect("decode framed block");
        assert_eq!(decoded_framed, block);

        let decoded_versioned =
            SignedBlock::decode_all_versioned(versioned.as_ref()).expect("decode versioned block");
        assert_eq!(decoded_versioned, block);
    }

    #[test]
    fn canonical_wire_matches_framed_payload() {
        use nonzero_ext::nonzero;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let block = SignedBlock {
            signatures: BTreeSet::new(),
            payload: BlockPayload {
                header,
                transactions: Vec::new(),
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
            },
            result: None,
        };

        let versioned = block.encode_versioned();
        let canonical = block.canonical_wire().expect("canonical wire");
        let framed = frame_versioned_signed_block_bytes(&versioned).expect("frame versioned block");

        assert_eq!(canonical.version(), block.version());
        assert_eq!(canonical.as_versioned(), versioned.as_slice());
        assert_eq!(canonical.as_framed(), framed.as_slice());
        assert_eq!(canonical.payload(), &versioned[1..]);

        let decoded = decode_framed_signed_block(canonical.as_framed())
            .expect("decode canonical framed block");
        assert_eq!(decoded, block);
    }

    #[test]
    fn canonical_wire_roundtrips_genesis_block() {
        use iroha_crypto::KeyPair;

        use crate::{
            ChainId, account::AccountId, domain::DomainId, transaction::signed::TransactionBuilder,
        };

        let keypair = KeyPair::random();
        let chain: ChainId = "genesis-canonical-wire".parse().expect("chain id");
        let domain: DomainId = "genesis".parse().expect("domain id");
        let authority = AccountId::new(domain, keypair.public_key().clone());
        let tx = TransactionBuilder::new(chain, authority)
            .with_instructions(core::iter::empty::<crate::isi::InstructionBox>())
            .sign(keypair.private_key());
        let block = SignedBlock::genesis(vec![tx], keypair.private_key(), None, None);

        let wire = block.canonical_wire().expect("canonical wire");
        assert_eq!(wire.version(), block.version());
        assert!(wire.as_framed()[1..].starts_with(MAGIC.as_slice()));

        let deframed =
            deframe_versioned_signed_block_bytes(wire.as_framed()).expect("deframe framed");
        assert_eq!(deframed.bytes.as_ref(), wire.as_framed());
        assert_eq!(deframed.bare_versioned.as_ref(), wire.as_versioned());

        let decoded =
            decode_framed_signed_block(wire.as_framed()).expect("decode canonical framed genesis");
        assert_eq!(decoded, block);
    }

    #[test]
    fn set_da_commitments_updates_header_hash() {
        use nonzero_ext::nonzero;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = SignedBlock {
            signatures: BTreeSet::new(),
            payload: BlockPayload {
                header,
                transactions: Vec::new(),
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
            },
            result: None,
        };

        assert!(block.payload.header.da_commitments_hash().is_none());

        let record = DaCommitmentRecord::new(
            LaneId::new(1),
            2,
            3,
            BlobDigest::new([0xAA; 32]),
            ManifestDigest::new([0xBB; 32]),
            DaProofScheme::MerkleSha256,
            Hash::prehashed([0xCC; 32]),
            Some(KzgCommitment::new([0xDD; 48])),
            Some(Hash::prehashed([0xEE; 32])),
            RetentionPolicy::default(),
            StorageTicketId::new([0xFF; 32]),
            Signature::from_bytes(&[0x11; 64]),
        );
        let bundle = DaCommitmentBundle::new(vec![record]);
        let expected = Some(bundle.canonical_hash());
        block.set_da_commitments(Some(bundle));
        assert_eq!(block.payload.header.da_commitments_hash(), expected);

        block.set_da_commitments(None);
        assert!(block.payload.header.da_commitments_hash().is_none());
    }

    #[test]
    fn decode_versioned_signed_block_handles_genesis_like_payload() {
        use iroha_crypto::KeyPair;

        use crate::{
            ChainId, account::AccountId, domain::DomainId, isi::InstructionBox,
            transaction::signed::TransactionBuilder,
        };

        let keypair = KeyPair::random();
        let chain: ChainId = "genesis-versioned-roundtrip"
            .parse()
            .expect("chain id must parse");
        let domain: DomainId = "genesis".parse().expect("domain id");
        let authority = AccountId::new(domain, keypair.public_key().clone());

        let tx1 = TransactionBuilder::new(chain.clone(), authority.clone())
            .with_instructions(core::iter::empty::<InstructionBox>())
            .sign(keypair.private_key());
        let tx2 = TransactionBuilder::new(chain, authority)
            .with_instructions(core::iter::empty::<InstructionBox>())
            .sign(keypair.private_key());

        let block = SignedBlock::genesis(vec![tx1, tx2], keypair.private_key(), None, None);

        let versioned = block.encode_versioned();
        let mut manual_payload = Vec::new();
        block.encode_to(&mut manual_payload);
        let mut manual_versioned = Vec::with_capacity(1 + manual_payload.len());
        manual_versioned.push(block.version());
        manual_versioned.extend_from_slice(&manual_payload);
        assert_eq!(
            manual_versioned, versioned,
            "canonical encode must be stable"
        );
        let decoded = SignedBlock::decode_all_versioned(&versioned)
            .expect("versioned genesis payload must roundtrip");
        assert_eq!(decoded, block);
    }

    #[test]
    fn decode_versioned_signed_block_accepts_framed_payload() {
        use nonzero_ext::nonzero;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let block = SignedBlock {
            signatures: BTreeSet::new(),
            payload: BlockPayload {
                header,
                transactions: Vec::new(),
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
            },
            result: None,
        };

        let versioned = block.encode_versioned();
        let framed =
            frame_versioned_signed_block_bytes(&versioned).expect("frame versioned payload");

        let decoded_from_framed = decode_versioned_signed_block(&framed)
            .expect("decode framed payload via versioned API");
        assert_eq!(decoded_from_framed, block);

        let err = decode_versioned_signed_block(&versioned)
            .expect_err("headerless payloads must be rejected");
        assert!(matches!(err, iroha_version::error::Error::NoritoCodec(_)));
    }

    #[test]
    fn framed_signed_block_uses_default_flags() {
        use nonzero_ext::nonzero;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let block = SignedBlock {
            signatures: BTreeSet::new(),
            payload: BlockPayload {
                header,
                transactions: Vec::new(),
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
            },
            result: None,
        };

        let versioned = block.encode_versioned();
        let framed = frame_versioned_signed_block_bytes(&versioned).expect("frame payload");

        let header_size = norito::core::Header::SIZE;
        let flags = framed[1 + header_size - 1];
        assert_eq!(flags, norito::core::default_encode_flags());
    }

    #[test]
    fn signed_block_da_commitments_roundtrip() {
        use nonzero_ext::nonzero;

        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = SignedBlock {
            signatures: BTreeSet::new(),
            payload: BlockPayload {
                header,
                transactions: Vec::new(),
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
            },
            result: None,
        };
        let bundle = sample_da_bundle();
        assert!(block.da_commitments().is_none());

        block.set_da_commitments(Some(bundle.clone()));
        assert_eq!(block.da_commitments().unwrap(), &bundle);
        assert!(block.header().da_commitments_hash().is_some());

        let encoded = block.encode_versioned();
        let decoded = SignedBlock::decode_all_versioned(&encoded).expect("decode versioned block");
        assert_eq!(decoded.da_commitments().unwrap(), &bundle);

        block.set_da_commitments(None);
        assert!(block.da_commitments().is_none());
        assert!(block.header().da_commitments_hash().is_none());
    }

    #[test]
    fn set_da_pin_intents_updates_header_hash() {
        use nonzero_ext::nonzero;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = SignedBlock {
            signatures: BTreeSet::new(),
            payload: BlockPayload {
                header,
                transactions: Vec::new(),
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
            },
            result: None,
        };

        assert!(block.payload.header.da_pin_intents_hash().is_none());

        let intent = DaPinIntent::new(
            LaneId::new(7),
            9,
            11,
            StorageTicketId::new([0xAB; 32]),
            ManifestDigest::new([0xCD; 32]),
        );
        let bundle = DaPinIntentBundle::new(vec![intent]);
        let expected = bundle
            .merkle_root()
            .map(HashOf::<DaPinIntentBundle>::from_untyped_unchecked)
            .expect("non-empty bundle must have root");

        block.set_da_pin_intents(Some(bundle.clone()));
        assert_eq!(block.da_pin_intents().unwrap(), &bundle);
        assert_eq!(block.header().da_pin_intents_hash(), Some(expected));

        block.set_da_pin_intents(None);
        assert!(block.da_pin_intents().is_none());
        assert!(block.header().da_pin_intents_hash().is_none());
    }

    #[test]
    fn genesis_can_embed_da_commitments() {
        use iroha_crypto::KeyPair;

        use crate::{
            ChainId, account::AccountId, domain::DomainId, isi::InstructionBox,
            transaction::signed::TransactionBuilder,
        };

        let keypair = KeyPair::random();
        let chain: ChainId = "genesis-da-commitments"
            .parse()
            .expect("chain id must parse");
        let domain: DomainId = "genesis".parse().expect("domain id");
        let authority = AccountId::new(domain, keypair.public_key().clone());
        let tx = TransactionBuilder::new(chain, authority)
            .with_instructions(core::iter::empty::<InstructionBox>())
            .sign(keypair.private_key());
        let bundle = sample_da_bundle();

        let block =
            SignedBlock::genesis(vec![tx], keypair.private_key(), None, Some(bundle.clone()));

        assert_eq!(block.da_commitments().unwrap(), &bundle);
        assert!(block.header().da_commitments_hash().is_some());
    }

    #[test]
    fn genesis_can_override_da_proof_policies() {
        use iroha_crypto::KeyPair;

        use crate::{
            ChainId,
            account::AccountId,
            da::commitment::{DaProofPolicy, DaProofPolicyBundle, DaProofScheme},
            domain::DomainId,
            isi::InstructionBox,
            nexus::{DataSpaceId, LaneId},
            transaction::signed::TransactionBuilder,
        };

        let keypair = KeyPair::random();
        let chain: ChainId = "genesis-da-proof-policies"
            .parse()
            .expect("chain id must parse");
        let domain: DomainId = "genesis".parse().expect("domain id");
        let authority = AccountId::new(domain, keypair.public_key().clone());
        let tx = TransactionBuilder::new(chain, authority)
            .with_instructions(core::iter::empty::<InstructionBox>())
            .sign(keypair.private_key());
        let bundle = DaProofPolicyBundle::new(vec![DaProofPolicy {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            alias: "custom".to_string(),
            proof_scheme: DaProofScheme::KzgBls12_381,
        }]);
        let expected_hash = HashOf::new(&bundle);

        let block = SignedBlock::genesis_with_da_proof_policies(
            vec![tx],
            keypair.private_key(),
            None,
            None,
            Some(bundle.clone()),
        );

        assert_eq!(block.da_proof_policies(), Some(&bundle));
        assert_eq!(block.header().da_proof_policies_hash(), Some(expected_hash));
    }

    #[cfg(feature = "transparent_api")]
    #[test]
    fn signed_block_has_results_only_after_assignment() {
        use iroha_crypto::KeyPair;

        use crate::transaction::signed::TransactionEntrypoint;

        let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
        let keypair = KeyPair::random();
        let mut block = SignedBlock::presigned(
            BlockSignature::new(
                0,
                SignatureOf::from_hash(keypair.private_key(), header.hash()),
            ),
            header,
            Vec::new(),
        );
        assert!(!block.has_results(), "fresh blocks must not have results");

        let entry_hashes: &[HashOf<TransactionEntrypoint>] = &[];
        block.set_transaction_results(Vec::new(), entry_hashes, Vec::new());
        assert!(
            block.has_results(),
            "setting results should mark block as executed"
        );
    }

    #[cfg(feature = "transparent_api")]
    #[test]
    fn set_transaction_results_records_fastpq_transcripts() {
        use std::{collections::BTreeMap, num::NonZeroU64};

        use iroha_crypto::{Hash, KeyPair, SignatureOf};
        use iroha_primitives::numeric::Numeric;

        use crate::{
            account::AccountId,
            asset::id::AssetDefinitionId,
            domain::DomainId,
            fastpq::{TransferDeltaTranscript, TransferTranscript},
        };

        fn fixture_account(domain: &DomainId) -> AccountId {
            let keypair = KeyPair::random();
            AccountId::new(domain.clone(), keypair.public_key().clone())
        }

        let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
        let keypair = KeyPair::random();
        let signature = BlockSignature::new(
            0,
            SignatureOf::from_hash(keypair.private_key(), header.hash()),
        );
        let mut block = SignedBlock::presigned(signature, header, Vec::new());

        let domain: DomainId = "test".parse().expect("domain id");
        let from = fixture_account(&domain);
        let to = fixture_account(&domain);
        let asset: AssetDefinitionId = "xor#test".parse().expect("asset id");

        let delta = TransferDeltaTranscript {
            from_account: from,
            to_account: to,
            asset_definition: asset,
            amount: Numeric::zero(),
            from_balance_before: Numeric::zero(),
            from_balance_after: Numeric::zero(),
            to_balance_before: Numeric::zero(),
            to_balance_after: Numeric::zero(),
            from_merkle_proof: None,
            to_merkle_proof: None,
        };
        let batch_hash = Hash::prehashed([0xAA; Hash::LENGTH]);
        let transcript = TransferTranscript {
            batch_hash,
            deltas: vec![delta],
            authority_digest: Hash::prehashed([0x11; Hash::LENGTH]),
            poseidon_preimage_digest: None,
        };

        let mut transcripts = BTreeMap::new();
        transcripts.insert(batch_hash, vec![transcript]);

        block.set_transaction_results_with_transcripts(
            Vec::new(),
            &[],
            Vec::new(),
            transcripts.clone(),
            Vec::new(),
            None,
        );

        assert_eq!(block.fastpq_transcripts(), &transcripts);
    }

    #[cfg(feature = "transparent_api")]
    #[test]
    fn block_proofs_include_fastpq_transcripts() {
        use std::{collections::BTreeMap, num::NonZeroU64};

        use iroha_crypto::{Hash, KeyPair, SignatureOf};
        use iroha_primitives::numeric::Numeric;

        use crate::{
            ChainId,
            account::AccountId,
            asset::id::AssetDefinitionId,
            domain::DomainId,
            fastpq::{TransferDeltaTranscript, TransferTranscript},
            transaction::{TransactionResultInner, signed::TransactionBuilder},
        };

        let keypair = KeyPair::random();
        let chain: ChainId = "chain".parse().expect("chain id");
        let authority_domain: DomainId = "chain".parse().expect("chain domain id");
        let authority = AccountId::new(authority_domain, KeyPair::random().public_key().clone());
        let tx =
            TransactionBuilder::new(chain.clone(), authority.clone()).sign(keypair.private_key());
        let entry_hash = tx.hash_as_entrypoint();
        let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
        let signature = BlockSignature::new(
            0,
            SignatureOf::from_hash(keypair.private_key(), header.hash()),
        );
        let mut block = SignedBlock::presigned(signature, header, vec![tx]);

        let asset: AssetDefinitionId = "xor#chain".parse().expect("asset id");
        let delta = TransferDeltaTranscript {
            from_account: authority.clone(),
            to_account: authority,
            asset_definition: asset,
            amount: Numeric::zero(),
            from_balance_before: Numeric::zero(),
            from_balance_after: Numeric::zero(),
            to_balance_before: Numeric::zero(),
            to_balance_after: Numeric::zero(),
            from_merkle_proof: None,
            to_merkle_proof: None,
        };
        let batch_hash = Hash::prehashed([0xBB; Hash::LENGTH]);
        let transcript = TransferTranscript {
            batch_hash,
            deltas: vec![delta],
            authority_digest: Hash::prehashed([0x22; Hash::LENGTH]),
            poseidon_preimage_digest: None,
        };
        let mut transcripts = BTreeMap::new();
        transcripts.insert(batch_hash, vec![transcript]);
        let binding = crate::nexus::AxtBinding::new([0x11; 32]);
        let axt_envelope = crate::nexus::AxtEnvelopeRecord {
            binding,
            lane: crate::nexus::LaneId::new(2),
            descriptor: crate::nexus::AxtDescriptor {
                dsids: vec![crate::DataSpaceId::new(9)],
                touches: Vec::new(),
            },
            touches: Vec::new(),
            proofs: Vec::new(),
            handles: Vec::new(),
            commit_height: Some(1),
        };
        let dsid = crate::DataSpaceId::new(9);
        let policy_snapshot = crate::nexus::AxtPolicySnapshot {
            version: 0,
            entries: vec![crate::nexus::AxtPolicyBinding {
                dsid,
                policy: crate::nexus::AxtPolicyEntry {
                    manifest_root: [0xAA; 32],
                    target_lane: crate::nexus::LaneId::new(2),
                    min_handle_era: 10,
                    min_sub_nonce: 5,
                    current_slot: 7,
                },
            }],
        }
        .with_computed_version();
        let expected_policy_snapshot = policy_snapshot.clone();

        block.set_transaction_results_with_transcripts(
            Vec::new(),
            &[entry_hash],
            vec![TransactionResultInner::Ok(
                crate::trigger::DataTriggerSequence::default(),
            )],
            transcripts.clone(),
            vec![axt_envelope.clone()],
            Some(policy_snapshot),
        );

        let proofs = block
            .proofs_for_entry_hash(&entry_hash)
            .expect("proofs present");
        assert_eq!(proofs.fastpq_transcripts, transcripts);

        let bytes = norito::to_bytes(&block).expect("encode block");
        let decoded: SignedBlock = norito::decode_from_bytes(&bytes).expect("decode block");
        assert_eq!(
            decoded.axt_envelopes(),
            Some(std::slice::from_ref(&axt_envelope))
        );
        assert_eq!(
            decoded.axt_policy_snapshot(),
            Some(&expected_policy_snapshot)
        );
    }

    #[cfg(feature = "transparent_api")]
    #[test]
    fn set_transaction_results_updates_merkle_roots_with_time_triggers() {
        use std::num::NonZeroU64;

        use iroha_crypto::{KeyPair, MerkleTree, SignatureOf};
        use iroha_primitives::const_vec::ConstVec;

        use crate::{
            ChainId,
            account::AccountId,
            domain::DomainId,
            transaction::{
                ExecutionStep,
                signed::{TransactionBuilder, TransactionResult, TransactionResultInner},
            },
            trigger::{DataTriggerSequence, TimeTriggerEntrypoint},
        };

        let keypair = KeyPair::random();
        let chain: ChainId = "test-chain".parse().expect("chain id");
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let authority = AccountId::new(domain, keypair.public_key().clone());

        let tx =
            TransactionBuilder::new(chain.clone(), authority.clone()).sign(keypair.private_key());
        let time_trigger = TimeTriggerEntrypoint {
            id: "housekeeping".parse().expect("trigger id"),
            instructions: ExecutionStep(ConstVec::new_empty()),
            authority: authority.clone(),
        };

        let external_hash = tx.hash_as_entrypoint();
        let entry_hashes = vec![external_hash, time_trigger.hash_as_entrypoint()];
        let expected_consensus_root = MerkleTree::from_iter([external_hash]).root();
        let expected_entry_root = entry_hashes
            .iter()
            .copied()
            .collect::<MerkleTree<_>>()
            .root();

        let results_inner = vec![
            TransactionResultInner::Ok(DataTriggerSequence::default()),
            TransactionResultInner::Ok(DataTriggerSequence::default()),
        ];
        let expected_result_root = {
            let hashes = results_inner.iter().map(TransactionResult::hash_from_inner);
            let tree: MerkleTree<TransactionResult> = hashes.collect();
            tree.root()
        };

        let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
        let signature = BlockSignature::new(
            0,
            SignatureOf::from_hash(keypair.private_key(), header.hash()),
        );
        let mut block = SignedBlock::presigned(signature, header, vec![tx]);

        block.set_transaction_results(vec![time_trigger], &entry_hashes, results_inner);

        assert_eq!(block.header().merkle_root(), expected_consensus_root);
        assert_eq!(
            block.full_entry_merkle_root(),
            expected_entry_root,
            "full entry root should cover time triggers"
        );
        assert_eq!(block.header().result_merkle_root(), expected_result_root);
    }

    #[cfg(feature = "transparent_api")]
    #[test]
    fn proofs_for_entry_hash_matches_merkle_roots() {
        use std::num::NonZeroU64;

        use iroha_crypto::{KeyPair, MerkleTree, SignatureOf};

        use crate::{
            ChainId,
            account::AccountId,
            domain::DomainId,
            transaction::signed::{TransactionBuilder, TransactionResultInner},
        };

        let keypair = KeyPair::random();
        let chain: ChainId = "proof-block".parse().expect("chain id");
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let authority = AccountId::new(domain, keypair.public_key().clone());

        let tx =
            TransactionBuilder::new(chain.clone(), authority.clone()).sign(keypair.private_key());
        let entry_hash = tx.hash_as_entrypoint();

        let entry_hashes = vec![entry_hash];
        let results_inner = vec![TransactionResultInner::Ok(
            crate::trigger::DataTriggerSequence::default(),
        )];

        let expected_entry_root = entry_hashes
            .iter()
            .copied()
            .collect::<MerkleTree<_>>()
            .root()
            .expect("entry root");
        let expected_result_root = {
            let result_hashes = results_inner.iter().map(TransactionResult::hash_from_inner);
            let tree: MerkleTree<TransactionResult> = result_hashes.collect();
            tree.root().expect("result root")
        };

        let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
        let signature = BlockSignature::new(
            0,
            SignatureOf::from_hash(keypair.private_key(), header.hash()),
        );
        let mut block = SignedBlock::presigned(signature, header, vec![tx]);
        block.set_transaction_results(Vec::new(), &entry_hashes, results_inner);

        let proofs = block
            .proofs_for_entry_hash(&entry_hash)
            .expect("proofs should exist");
        let entry_root = proofs.entry_root;
        assert_eq!(
            entry_root,
            block.header().merkle_root().expect("entry root"),
            "entry root should match consensus root for external transactions"
        );
        assert!(proofs.entry_proof.verify(&entry_root));
        assert_eq!(entry_root, expected_entry_root);

        let result_root = proofs.result_root.expect("result root");
        assert_eq!(
            result_root,
            block
                .header()
                .result_merkle_root()
                .expect("result root in header")
        );
        let result_proof = proofs.result_proof.expect("result proof");
        assert!(result_proof.verify(&result_root));
        assert_eq!(result_root, expected_result_root);
    }

    #[cfg(feature = "transparent_api")]
    #[test]
    fn proofs_for_time_trigger_use_extended_root() {
        use std::num::NonZeroU64;

        use iroha_crypto::{KeyPair, MerkleTree, SignatureOf};
        use iroha_primitives::const_vec::ConstVec;

        use crate::{
            ChainId,
            account::AccountId,
            domain::DomainId,
            transaction::{
                ExecutionStep,
                signed::{TransactionBuilder, TransactionResult, TransactionResultInner},
            },
            trigger::{DataTriggerSequence, TimeTriggerEntrypoint},
        };

        let keypair = KeyPair::random();
        let chain: ChainId = "time-proof-block".parse().expect("chain id");
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let authority = AccountId::new(domain, keypair.public_key().clone());

        let tx =
            TransactionBuilder::new(chain.clone(), authority.clone()).sign(keypair.private_key());
        let time_trigger = TimeTriggerEntrypoint {
            id: "cleanup".parse().expect("trigger id"),
            instructions: ExecutionStep(ConstVec::new_empty()),
            authority: authority.clone(),
        };

        let external_hash = tx.hash_as_entrypoint();
        let time_hash = time_trigger.hash_as_entrypoint();
        let entry_hashes = vec![external_hash, time_hash];
        let expected_full_root = entry_hashes
            .iter()
            .copied()
            .collect::<MerkleTree<_>>()
            .root()
            .expect("full root");

        let results_inner = vec![
            TransactionResultInner::Ok(DataTriggerSequence::default()),
            TransactionResultInner::Ok(DataTriggerSequence::default()),
        ];
        let expected_result_root = {
            let result_hashes = results_inner.iter().map(TransactionResult::hash_from_inner);
            let tree: MerkleTree<TransactionResult> = result_hashes.collect();
            tree.root().expect("result root")
        };

        let header = BlockHeader::new(NonZeroU64::new(3).unwrap(), None, None, None, 0, 0);
        let signature = BlockSignature::new(
            0,
            SignatureOf::from_hash(keypair.private_key(), header.hash()),
        );
        let mut block = SignedBlock::presigned(signature, header, vec![tx]);
        block.set_transaction_results(vec![time_trigger], &entry_hashes, results_inner);

        let proofs = block
            .proofs_for_entry_hash(&time_hash)
            .expect("time trigger proof exists");
        let consensus_root = block.header().merkle_root().expect("consensus root");
        let full_root = block.full_entry_merkle_root().expect("full root");
        assert_ne!(
            consensus_root, full_root,
            "time triggers extend the entrypoint root beyond consensus root"
        );
        assert_eq!(
            proofs.entry_root, full_root,
            "entry root for time trigger should match extended root"
        );
        assert!(proofs.entry_proof.verify(&proofs.entry_root));
        assert_eq!(proofs.entry_root, expected_full_root);

        let result_root = proofs.result_root.expect("result root");
        assert_eq!(result_root, expected_result_root);
        let result_proof = proofs.result_proof.expect("result proof");
        assert!(result_proof.verify(&result_root));
    }

    #[cfg(feature = "transparent_api")]
    #[test]
    fn proofs_for_entry_hash_missing_returns_none() {
        use std::num::NonZeroU64;

        use iroha_crypto::{Hash, KeyPair, SignatureOf};

        use crate::{
            ChainId,
            account::AccountId,
            domain::DomainId,
            transaction::signed::{TransactionBuilder, TransactionResultInner},
        };

        let keypair = KeyPair::random();
        let chain: ChainId = "proof-miss".parse().expect("chain id");
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let authority = AccountId::new(domain, keypair.public_key().clone());

        let tx =
            TransactionBuilder::new(chain.clone(), authority.clone()).sign(keypair.private_key());
        let entry_hash = tx.hash_as_entrypoint();

        let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
        let signature = BlockSignature::new(
            0,
            SignatureOf::from_hash(keypair.private_key(), header.hash()),
        );
        let mut block = SignedBlock::presigned(signature, header, vec![tx]);
        block.set_transaction_results(
            Vec::new(),
            &[entry_hash],
            vec![TransactionResultInner::Ok(
                crate::trigger::DataTriggerSequence::default(),
            )],
        );

        let mut missing_bytes = *entry_hash.as_ref();
        missing_bytes[0] ^= 0xFF;
        let missing = HashOf::from_untyped_unchecked(Hash::prehashed(missing_bytes));
        assert!(block.proofs_for_entry_hash(&missing).is_none());
    }

    #[test]
    fn canonical_wire_and_deframe_preserve_layout_flags() {
        use iroha_crypto::KeyPair;

        use crate::{
            ChainId, account::AccountId, block::deframe_versioned_signed_block_bytes,
            domain::DomainId, transaction::signed::TransactionBuilder,
        };

        let keypair = KeyPair::random();
        let chain: ChainId = "test-chain".parse().expect("chain id");
        let domain_id: DomainId = "genesis".parse().expect("domain id");
        let authority = AccountId::new(domain_id, keypair.public_key().clone());

        let transaction =
            TransactionBuilder::new(chain.clone(), authority.clone()).sign(keypair.private_key());
        let block = SignedBlock::genesis(vec![transaction], keypair.private_key(), None, None);

        let wire = block.canonical_wire().expect("canonical wire");
        let header_index = 1 + norito::core::Header::SIZE - 1;
        let header_flags = wire.as_framed()[header_index];

        assert_eq!(header_flags, norito::core::default_encode_flags());

        let versioned = block.encode_versioned();
        let deframed =
            deframe_versioned_signed_block_bytes(wire.as_framed()).expect("deframe framed block");
        assert_eq!(deframed.bare_versioned.as_ref(), versioned.as_slice());

        super::decode_framed_signed_block(wire.as_framed()).expect("decode canonical wire");
        let err = super::decode_framed_signed_block(&versioned)
            .expect_err("headerless payloads must be rejected");
        assert!(matches!(err, iroha_version::error::Error::NoritoCodec(_)));
    }

    #[test]
    fn framing_derives_flags_instead_of_reusing_tls_state() {
        use iroha_crypto::KeyPair;

        use crate::{
            ChainId, account::AccountId, domain::DomainId, transaction::signed::TransactionBuilder,
        };

        let keypair = KeyPair::random();
        let chain: ChainId = "stale-flags".parse().expect("chain id");
        let domain_id: DomainId = "genesis".parse().expect("domain id");
        let authority = AccountId::new(domain_id, keypair.public_key().clone());

        let transaction =
            TransactionBuilder::new(chain.clone(), authority.clone()).sign(keypair.private_key());
        let block = SignedBlock::genesis(vec![transaction], keypair.private_key(), None, None);

        let versioned = block.encode_versioned();
        let header_index = 1 + norito::core::Header::SIZE - 1;
        let expected_flags = norito::core::default_encode_flags();
        let framed =
            frame_versioned_signed_block_bytes(&versioned).expect("frame versioned payload");
        assert_eq!(
            framed[header_index], expected_flags,
            "Framing must use the canonical fixed layout flags",
        );

        let deframed =
            deframe_versioned_signed_block_bytes(&framed).expect("deframe framed payload");
        assert_eq!(deframed.bare_versioned.as_ref(), versioned.as_slice());
    }

    #[test]
    fn decode_field_respects_length_and_consumes_payload() {
        // Prepare bare Norito payload representing a String value.
        let value = String::from("field-value");
        let mut payload = Vec::new();
        value.encode_to(&mut payload);

        // Prefix the payload with its little-endian length header.
        let mut input = Vec::with_capacity(8 + payload.len());
        input.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        input.extend_from_slice(&payload);

        let (decoded, rest) = super::decode_field::<String>(&input).expect("decode field");
        assert_eq!(decoded, value);
        assert!(rest.is_empty());
    }

    #[cfg(feature = "transparent_api")]
    #[test]
    fn update_transaction_result_incremental_matches_full_rebuild() {
        use nonzero_ext::nonzero;

        // Prepare a small block with a few transactions and empty triggers.
        let txs: Vec<crate::transaction::signed::SignedTransaction> = Vec::new();
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = SignedBlock::presigned(
            BlockSignature::new(
                0,
                SignatureOf::from_hash(
                    iroha_crypto::KeyPair::random().private_key(),
                    header.hash(),
                ),
            ),
            header,
            txs,
        );

        // Seed with 4 entrypoints (2 external hashes, 2 time triggers) and 4 results
        let tx_hashes: Vec<HashOf<TransactionEntrypoint>> = (0..4)
            .map(|i| {
                let byte = u8::try_from(i).expect("transaction index fits in u8");
                HashOf::<TransactionEntrypoint>::from_untyped_unchecked(iroha_crypto::Hash::new([
                    byte,
                ]))
            })
            .collect();
        let results_inner: Vec<TransactionResultInner> = vec![
            TransactionResultInner::Ok(DataTriggerSequence::default()),
            TransactionResultInner::Err(
                crate::transaction::error::TransactionRejectionReason::Validation(
                    ValidationFail::InternalError("bad_query".into()),
                ),
            ),
            TransactionResultInner::Ok(DataTriggerSequence::default()),
            TransactionResultInner::Err(
                crate::transaction::error::TransactionRejectionReason::Validation(
                    ValidationFail::NotPermitted("no".into()),
                ),
            ),
        ];
        block.set_transaction_results(
            Vec::<TimeTriggerEntrypoint>::new(),
            &tx_hashes,
            results_inner.clone(),
        );
        let root_initial = block.header().result_merkle_root;

        // Update one result incrementally
        let idx = 1usize;
        let new_inner = TransactionResultInner::Ok(DataTriggerSequence::default());
        assert!(block.update_transaction_result(idx, &new_inner));
        let root_after_inc = block.header().result_merkle_root;

        // Rebuild a fresh block with the updated results and compare roots
        let mut rebuilt = block.clone();
        let mut new_results = results_inner;
        new_results[idx] = new_inner.clone();
        rebuilt.set_transaction_results(
            Vec::<TimeTriggerEntrypoint>::new(),
            &tx_hashes,
            new_results,
        );
        let root_rebuilt = rebuilt.header().result_merkle_root;

        assert_ne!(root_initial, root_after_inc);
        assert_eq!(root_after_inc, root_rebuilt);
    }
}
