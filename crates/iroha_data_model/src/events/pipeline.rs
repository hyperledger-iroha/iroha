//! Pipeline events.
use std::{boxed::Box, format, num::NonZeroU64, string::String, vec::Vec};

use iroha_crypto::HashOf;
use iroha_data_model_derive::model;
use iroha_macro::FromVariant;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

pub use self::model::*;
use crate::{
    block::{BlockHeader, consensus::ExecWitnessMsg},
    merge::MergeLedgerEntry,
    nexus::{DataSpaceId, LaneId},
    transaction::SignedTransaction,
};

#[model]
mod model {
    use getset::{CopyGetters, Getters};

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, FromVariant, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub enum PipelineEventBox {
        Transaction(TransactionEvent),
        Block(BlockEvent),
        /// Warning emitted by the pipeline (non-forking informational signal).
        Warning(PipelineWarning),
        /// Merge-ledger entry committed by the merge committee.
        Merge(MergeLedgerEvent),
        /// Execution witness produced after block execution.
        Witness(super::ExecWitnessMsg),
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    #[getset(get = "pub")]
    pub struct BlockEvent {
        pub header: BlockHeader,
        pub status: BlockStatus,
    }

    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        CopyGetters,
        Getters,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct TransactionEvent {
        #[getset(get = "pub")]
        pub hash: HashOf<SignedTransaction>,
        #[getset(get_copy = "pub")]
        pub block_height: Option<NonZeroU64>,
        #[getset(get_copy = "pub")]
        pub lane_id: LaneId,
        #[getset(get_copy = "pub")]
        pub dataspace_id: DataSpaceId,
        #[getset(get = "pub")]
        pub status: TransactionStatus,
    }

    /// Report of block's status in the pipeline
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub enum BlockStatus {
        /// Block created (only emitted by the leader node)
        Created,
        /// Block was approved to participate in consensus
        Approved,
        /// Block was rejected by consensus
        Rejected(crate::block::error::BlockRejectionReason),
        /// Block has passed consensus successfully
        Committed,
        /// Changes have been reflected in the WSV
        Applied,
    }

    /// Pipeline warning payload.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct PipelineWarning {
        /// Block header associated with the warning.
        pub header: BlockHeader,
        /// Machine-readable warning kind.
        pub kind: String,
        /// Human-readable details.
        pub details: String,
    }

    /// Merge-ledger entry that was appended to persistent storage.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct MergeLedgerEvent {
        /// Merge-ledger entry payload.
        pub entry: MergeLedgerEntry,
    }

    /// Report of transaction's status in the pipeline
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub enum TransactionStatus {
        /// Transaction was received and enqueued
        Queued,
        /// Transaction was dropped(not stored in a block)
        Expired,
        /// Transaction was stored in the block as valid
        Approved,
        /// Transaction was stored in the block as invalid
        Rejected(Box<crate::transaction::error::TransactionRejectionReason>),
    }

    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, FromVariant, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum PipelineEventFilterBox {
        Transaction(TransactionEventFilter),
        Block(BlockEventFilter),
        Merge(MergeLedgerEventFilter),
        Witness(WitnessEventFilter),
    }

    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Default,
        CopyGetters,
        Getters,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct BlockEventFilter {
        #[getset(get_copy = "pub")]
        pub height: Option<NonZeroU64>,
        #[getset(get = "pub")]
        pub status: Option<BlockStatus>,
    }

    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Getters, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct TransactionEventFilter {
        #[getset(get = "pub")]
        pub hash: Option<HashOf<SignedTransaction>>,
        #[getset(get_copy = "pub")]
        pub block_height: Option<Option<NonZeroU64>>,
        #[getset(get_copy = "pub")]
        pub lane_id: Option<LaneId>,
        #[getset(get_copy = "pub")]
        pub dataspace_id: Option<DataSpaceId>,
        #[getset(get = "pub")]
        pub status: Option<TransactionStatus>,
    }

    /// Filter merge-ledger events by epoch.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Getters, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct MergeLedgerEventFilter {
        #[getset(get_copy = "pub")]
        pub epoch_id: Option<u64>,
    }

    /// Filter witness events by block metadata.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Getters, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    #[getset(get = "pub")]
    pub struct WitnessEventFilter {
        /// Block hash associated with the witness event.
        pub block_hash: Option<HashOf<BlockHeader>>,
        #[getset(get_copy = "pub")]
        /// Optional block height filter.
        pub height: Option<NonZeroU64>,
        #[getset(get_copy = "pub")]
        /// Optional view filter.
        pub view: Option<u64>,
    }

    impl MergeLedgerEventFilter {
        /// Match only merge-ledger events for the given epoch identifier.
        #[must_use]
        pub fn for_epoch(mut self, epoch_id: u64) -> Self {
            self.epoch_id = Some(epoch_id);
            self
        }
    }

    impl WitnessEventFilter {
        /// Match only witnesses for the given block hash.
        #[must_use]
        pub fn for_block_hash(mut self, block_hash: HashOf<BlockHeader>) -> Self {
            self.block_hash = Some(block_hash);
            self
        }

        /// Match only witnesses at the specified block height.
        #[must_use]
        pub fn for_height(mut self, height: NonZeroU64) -> Self {
            self.height = Some(height);
            self
        }

        /// Match only witnesses emitted in the specified view.
        #[must_use]
        pub fn for_view(mut self, view: u64) -> Self {
            self.view = Some(view);
            self
        }

        /// Returns `true` when the witness event satisfies the stored filter predicates.
        pub fn matches(&self, event: &super::ExecWitnessMsg) -> bool {
            self.block_hash
                .as_ref()
                .is_none_or(|hash| hash == &event.block_hash)
                && self.height.is_none_or(|h| h.get() == event.height)
                && self.view.is_none_or(|v| v == event.view)
        }
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(
    PipelineEventBox,
    BlockEvent,
    TransactionEvent,
    BlockStatus,
    PipelineWarning,
    TransactionStatus,
    PipelineEventFilterBox,
    BlockEventFilter,
    TransactionEventFilter,
    MergeLedgerEvent,
    MergeLedgerEventFilter,
    ExecWitnessMsg,
    WitnessEventFilter
);

#[cfg(test)]
#[cfg(feature = "transparent_api")]
mod getter_tests {
    use iroha_crypto::Hash;
    use nonzero_ext::nonzero;

    use super::*;

    #[test]
    fn transaction_event_filter_block_height_getter_works() {
        let h = HashOf::from_untyped_unchecked(Hash::prehashed([0u8; Hash::LENGTH]));
        let filter = TransactionEventFilter::default()
            .for_hash(h)
            .for_status(TransactionStatus::Queued)
            .for_block_height(Some(nonzero!(42_u64)));

        // Use generated getters where available
        assert!(filter.hash().is_some());
        assert!(filter.status().is_some());
        // Access the field directly to avoid name clash with getter generation
        assert_eq!(filter.block_height, Some(Some(nonzero!(42_u64))));
    }
}

impl BlockEventFilter {
    /// Construct new instance
    #[must_use]
    pub const fn new() -> Self {
        Self {
            height: None,
            status: None,
        }
    }

    /// Match only block with the given height
    #[must_use]
    pub fn for_height(mut self, height: NonZeroU64) -> Self {
        self.height = Some(height);
        self
    }

    /// Match only block with the given status
    #[must_use]
    pub fn for_status(mut self, status: BlockStatus) -> Self {
        self.status = Some(status);
        self
    }

    #[cfg(feature = "transparent_api")]
    fn status_matches(filter: Option<&BlockStatus>, event: BlockStatus) -> bool {
        match filter {
            None => true,
            Some(BlockStatus::Rejected(reason)) => match event {
                BlockStatus::Rejected(_)
                    if *reason
                        == crate::block::error::BlockRejectionReason::ConsensusBlockRejection =>
                {
                    true
                }
                BlockStatus::Rejected(actual) => reason == &actual,
                _ => false,
            },
            Some(expected) => *expected == event,
        }
    }
}

impl TransactionEventFilter {
    /// Construct new instance
    #[must_use]
    pub const fn new() -> Self {
        Self {
            hash: None,
            block_height: None,
            lane_id: None,
            dataspace_id: None,
            status: None,
        }
    }

    /// Match only transactions with the given block height
    #[must_use]
    pub fn for_block_height(mut self, block_height: Option<NonZeroU64>) -> Self {
        self.block_height = Some(block_height);
        self
    }

    /// Match only transactions with the given hash
    #[must_use]
    pub fn for_hash(mut self, hash: HashOf<SignedTransaction>) -> Self {
        self.hash = Some(hash);
        self
    }

    /// Match only transactions with the given status
    #[must_use]
    pub fn for_status(mut self, status: TransactionStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Match only transactions scheduled on the given lane.
    #[must_use]
    pub fn for_lane_id(mut self, lane_id: LaneId) -> Self {
        self.lane_id = Some(lane_id);
        self
    }

    /// Match only transactions targeting the given dataspace.
    #[must_use]
    pub fn for_dataspace_id(mut self, dataspace_id: DataSpaceId) -> Self {
        self.dataspace_id = Some(dataspace_id);
        self
    }

    // Getter is provided by `getset(get_copy = "pub")` above: `fn block_height(&self)`.
}

#[cfg(feature = "transparent_api")]
impl TransactionEventFilter {
    fn field_matches<T: Eq>(filter: Option<&T>, event: &T) -> bool {
        filter.is_none_or(|field| field == event)
    }
}

#[cfg(feature = "transparent_api")]
impl BlockEventFilter {
    fn field_matches<T: Eq>(filter: Option<&T>, event: &T) -> bool {
        filter.is_none_or(|field| field == event)
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for PipelineEventFilterBox {
    type Event = PipelineEventBox;

    /// Check if `self` accepts the `event`.
    #[inline]
    fn matches(&self, event: &PipelineEventBox) -> bool {
        match (self, event) {
            (Self::Block(block_filter), PipelineEventBox::Block(block_event)) => [
                BlockEventFilter::field_matches(
                    block_filter.height.as_ref(),
                    &block_event.header.height,
                ),
                BlockEventFilter::status_matches(block_filter.status.as_ref(), block_event.status),
            ]
            .into_iter()
            .all(core::convert::identity),
            (
                Self::Transaction(transaction_filter),
                PipelineEventBox::Transaction(transaction_event),
            ) => [
                TransactionEventFilter::field_matches(
                    transaction_filter.hash.as_ref(),
                    &transaction_event.hash,
                ),
                TransactionEventFilter::field_matches(
                    transaction_filter.block_height.as_ref(),
                    &transaction_event.block_height,
                ),
                TransactionEventFilter::field_matches(
                    transaction_filter.lane_id.as_ref(),
                    &transaction_event.lane_id,
                ),
                TransactionEventFilter::field_matches(
                    transaction_filter.dataspace_id.as_ref(),
                    &transaction_event.dataspace_id,
                ),
                TransactionEventFilter::field_matches(
                    transaction_filter.status.as_ref(),
                    &transaction_event.status,
                ),
            ]
            .into_iter()
            .all(core::convert::identity),
            (Self::Merge(merge_filter), PipelineEventBox::Merge(merge_event)) => merge_filter
                .epoch_id
                .is_none_or(|epoch| epoch == merge_event.entry.epoch_id),
            (Self::Witness(filter), PipelineEventBox::Witness(event)) => filter.matches(event),
            _ => false,
        }
    }
}

/// Exports common structs and enums from this module.
pub mod prelude {
    pub use super::{
        BlockEvent, BlockStatus, MergeLedgerEvent, MergeLedgerEventFilter, PipelineEventBox,
        PipelineEventFilterBox, TransactionEvent, TransactionStatus, WitnessEventFilter,
    };
}

#[cfg(test)]
#[cfg(feature = "transparent_api")]
mod tests {
    use std::vec::Vec;

    use iroha_crypto::Hash;
    use nonzero_ext::nonzero;

    use super::{super::EventFilter, *};
    use crate::{
        ValidationFail, merge::MergeQuorumCertificate,
        transaction::error::TransactionRejectionReason::*,
    };

    impl BlockHeader {
        fn dummy(height: NonZeroU64) -> Self {
            let merkle_root = HashOf::from_untyped_unchecked(Hash::prehashed([1_u8; Hash::LENGTH]));
            Self {
                height,
                prev_block_hash: None,
                merkle_root: Some(merkle_root),
                result_merkle_root: None,
                da_proof_policies_hash: None,
                da_commitments_hash: None,
                da_pin_intents_hash: None,
                prev_roster_evidence_hash: None,
                creation_time_ms: 0,
                view_change_index: 0,
                confidential_features: None,
            }
        }
    }
    fn sample_pipeline_events() -> Vec<PipelineEventBox> {
        let merge_entry = MergeLedgerEntry {
            epoch_id: 42,
            lane_tips: vec![HashOf::from_untyped_unchecked(Hash::prehashed(
                [3_u8; Hash::LENGTH],
            ))],
            merge_hint_roots: vec![Hash::new(b"hint")],
            global_state_root: Hash::new(b"global"),
            merge_qc: MergeQuorumCertificate::new(
                7,
                42,
                vec![0x01],
                vec![0xAA],
                Hash::new(b"digest"),
            ),
        };

        let tx_queued: PipelineEventBox = TransactionEvent {
            hash: HashOf::from_untyped_unchecked(Hash::prehashed([0_u8; Hash::LENGTH])),
            block_height: None,
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            status: TransactionStatus::Queued,
        }
        .into();
        let tx_rejected: PipelineEventBox = TransactionEvent {
            hash: HashOf::from_untyped_unchecked(Hash::prehashed([0_u8; Hash::LENGTH])),
            block_height: Some(nonzero!(3_u64)),
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            status: TransactionStatus::Rejected(Box::new(Validation(ValidationFail::TooComplex))),
        }
        .into();
        let tx_approved: PipelineEventBox = TransactionEvent {
            hash: HashOf::from_untyped_unchecked(Hash::prehashed([2_u8; Hash::LENGTH])),
            block_height: None,
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            status: TransactionStatus::Approved,
        }
        .into();
        let block_committed: PipelineEventBox = BlockEvent {
            header: BlockHeader::dummy(nonzero!(7_u64)),
            status: BlockStatus::Committed,
        }
        .into();
        let merge_event: PipelineEventBox = MergeLedgerEvent { entry: merge_entry }.into();
        let witness_event: PipelineEventBox = ExecWitnessMsg {
            block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([4_u8; Hash::LENGTH])),
            height: 2,
            view: 1,
            epoch: 0,
            witness: crate::block::consensus::ExecWitness {
                reads: Vec::new(),
                writes: Vec::new(),
                fastpq_transcripts: Vec::new(),
                fastpq_batches: Vec::new(),
            },
        }
        .into();

        let events = vec![
            tx_queued.clone(),
            tx_rejected.clone(),
            tx_approved.clone(),
            block_committed.clone(),
            merge_event.clone(),
            witness_event.clone(),
        ];

        events
    }

    fn apply_filter(
        events: &[PipelineEventBox],
        filter: &PipelineEventFilterBox,
    ) -> Vec<PipelineEventBox> {
        events
            .iter()
            .filter(|event| filter.matches(event))
            .cloned()
            .collect()
    }

    #[test]
    fn transaction_filters_match_expected_hashes() {
        let events = sample_pipeline_events();
        let filter: PipelineEventFilterBox = TransactionEventFilter::default()
            .for_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
                [0_u8; Hash::LENGTH],
            )))
            .into();

        let matched = apply_filter(&events, &filter);
        assert_eq!(matched, vec![events[0].clone(), events[1].clone()]);
    }

    #[test]
    fn block_filters_match_committed_events() {
        let events = sample_pipeline_events();
        let filter: PipelineEventFilterBox = BlockEventFilter::default().into();

        let matched = apply_filter(&events, &filter);
        assert_eq!(matched, vec![events[3].clone()]);
    }

    #[test]
    fn transaction_filters_match_other_hash() {
        let events = sample_pipeline_events();
        let filter: PipelineEventFilterBox = TransactionEventFilter::default()
            .for_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
                [2_u8; Hash::LENGTH],
            )))
            .into();

        let matched = apply_filter(&events, &filter);
        assert_eq!(matched, vec![events[2].clone()]);
    }

    #[test]
    fn merge_filters_match_epoch() {
        let events = sample_pipeline_events();
        let filter: PipelineEventFilterBox = MergeLedgerEventFilter::default().for_epoch(42).into();

        let matched = apply_filter(&events, &filter);
        assert_eq!(matched, vec![events[4].clone()]);
    }

    #[test]
    fn witness_filters_match_metadata() {
        let events = sample_pipeline_events();
        let filter: PipelineEventFilterBox = WitnessEventFilter::default()
            .for_block_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
                [4_u8; Hash::LENGTH],
            )))
            .for_height(nonzero!(2_u64))
            .for_view(1)
            .into();

        let matched = apply_filter(&events, &filter);
        assert_eq!(matched, vec![events[5].clone()]);
    }
}
