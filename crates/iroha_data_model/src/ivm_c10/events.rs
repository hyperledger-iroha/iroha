//! Pipeline events and event filters of C10 executions.
//!
//! A transaction status is the only changed member of the pipeline event and event filter
//! closures: [`TransactionStatusC10V1`] carries the C10 rejection family. Every enclosing type
//! below is therefore a new definition under its own nominal schema identity, and every member
//! the C10 profile did not change (block, warning and witness events, block, witness, data,
//! time, execute-trigger and trigger-completed events and filters) is the baseline's own type.
//!
//! JSON forms are the canonical Norito bytes in base64, exactly as for the baseline pipeline
//! events and filters.

use iroha_crypto::HashOf;
use iroha_model_base::topology::{DataSpaceId, LaneId};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::{num::NonZeroU64, vec::Vec};

#[cfg(feature = "transparent_api")]
mod matching;

use super::TransactionStatusC10V1;
use crate::{
    block::consensus::ExecWitnessMsg,
    events::{
        SharedDataEvent,
        data::DataEventFilter,
        execute_trigger::{ExecuteTriggerEvent, ExecuteTriggerEventFilter},
        pipeline::{BlockEvent, BlockEventFilter, PipelineWarning, WitnessEventFilter},
        time::{TimeEvent, TimeEventFilter},
        trigger_completed::{TriggerCompletedEvent, TriggerCompletedEventFilter},
    },
    transaction::SignedTransaction,
};

/// Canonical Norito bytes in base64 as the JSON form, as the baseline pipeline types use.
macro_rules! c10_json_via_norito_bytes {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl norito::json::FastJsonWrite for $ty {
                fn write_json(&self, out: &mut std::string::String) {
                    norito::json::write_canonical_base64_json(self, out);
                }
                fn write_json_to(
                    &self,
                    out: &mut dyn norito::json::JsonWriteSink,
                ) -> Result<(), norito::json::BoundedJsonError> {
                    norito::json::write_canonical_base64_json_to(self, out)
                }
            }
            impl norito::json::JsonDeserialize for $ty {
                fn json_deserialize(
                    parser: &mut norito::json::Parser<'_>,
                ) -> Result<Self, norito::json::Error> {
                    let encoded = parser.parse_string()?;
                    let bytes = base64::Engine::decode(
                        &base64::engine::general_purpose::STANDARD,
                        encoded.as_str(),
                    )
                    .map_err(|err| norito::json::Error::Message(err.to_string()))?;
                    norito::decode_canonical::<$ty>(&bytes)
                        .map_err(|err| norito::json::Error::Message(err.to_string()))
                }
            }
        )+
    };
}
pub(super) use c10_json_via_norito_bytes;

/// Pipeline report of a transaction that contains a C10 execution.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::ivm_c10::TransactionEventC10V1")]
pub struct TransactionEventC10V1 {
    /// Hash of the transaction.
    pub hash: HashOf<SignedTransaction>,
    /// Height of the block that stored the transaction, once stored.
    pub block_height: Option<NonZeroU64>,
    /// Lane the transaction was scheduled on.
    pub lane_id: LaneId,
    /// Data space the transaction targets.
    pub dataspace_id: DataSpaceId,
    /// Pipeline status, with the C10 rejection family.
    pub status: TransactionStatusC10V1,
}

/// Explicit WebSocket offer required for the certified C10 transaction event stream.
pub const C10_EVENT_SUBPROTOCOL: &str = "iroha.events.c10.norito.v1";

/// Subscribe to matching certified Network transactions executed under C10.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::ivm_c10::EventSubscriptionC10V1")]
pub struct EventSubscriptionC10V1 {
    /// At least one predicate is required. Predicates are joined by OR.
    pub filters: Vec<TransactionEventFilterC10V1>,
}

/// A certified C10 transaction event sent on its explicitly selected subprotocol.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::ivm_c10::EventMessageC10V1")]
pub struct EventMessageC10V1(pub EventBoxC10V1);

/// Pipeline event of a C10 execution root.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::ivm_c10::PipelineEventBoxC10V1")]
pub enum PipelineEventBoxC10V1 {
    /// Transaction event.
    Transaction(TransactionEventC10V1),
    /// Block event (unchanged member).
    Block(BlockEvent),
    /// Warning emitted by the pipeline (unchanged member).
    Warning(PipelineWarning),
    /// Execution witness produced after block execution (unchanged member).
    Witness(ExecWitnessMsg),
}

/// Top-level streamed event of a root that reports C10 executions.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::ivm_c10::EventBoxC10V1")]
pub enum EventBoxC10V1 {
    /// Pipeline event.
    Pipeline(PipelineEventBoxC10V1),
    /// Batch of pipeline events emitted together.
    PipelineBatch(Vec<PipelineEventBoxC10V1>),
    /// Data event (unchanged member).
    Data(SharedDataEvent),
    /// Time event (unchanged member).
    Time(TimeEvent),
    /// Trigger execution event (unchanged member).
    ExecuteTrigger(ExecuteTriggerEvent),
    /// Trigger completion event (unchanged member).
    TriggerCompleted(TriggerCompletedEvent),
}

/// Filter of [`TransactionEventC10V1`]: every present field must equal the event's.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::ivm_c10::TransactionEventFilterC10V1")]
pub struct TransactionEventFilterC10V1 {
    /// Transaction hash.
    pub hash: Option<HashOf<SignedTransaction>>,
    /// Block height, including "not stored yet" (`Some(None)`).
    pub block_height: Option<Option<NonZeroU64>>,
    /// Lane.
    pub lane_id: Option<LaneId>,
    /// Data space.
    pub dataspace_id: Option<DataSpaceId>,
    /// Exact status, with the C10 rejection family.
    pub status: Option<TransactionStatusC10V1>,
}

impl TransactionEventFilterC10V1 {
    /// Match only transactions with the given hash.
    #[must_use]
    pub fn for_hash(mut self, hash: HashOf<SignedTransaction>) -> Self {
        self.hash = Some(hash);
        self
    }

    /// Match only transactions with the given block height.
    #[must_use]
    pub fn for_block_height(mut self, block_height: Option<NonZeroU64>) -> Self {
        self.block_height = Some(block_height);
        self
    }

    /// Match only transactions scheduled on the given lane.
    #[must_use]
    pub fn for_lane_id(mut self, lane_id: LaneId) -> Self {
        self.lane_id = Some(lane_id);
        self
    }

    /// Match only transactions targeting the given data space.
    #[must_use]
    pub fn for_dataspace_id(mut self, dataspace_id: DataSpaceId) -> Self {
        self.dataspace_id = Some(dataspace_id);
        self
    }

    /// Match only transactions with the given status.
    #[must_use]
    pub fn for_status(mut self, status: TransactionStatusC10V1) -> Self {
        self.status = Some(status);
        self
    }

    /// Whether `event` satisfies every present field, exactly as the baseline
    /// `TransactionEventFilter` matches a `TransactionEvent`.
    #[must_use]
    pub fn matches(&self, event: &TransactionEventC10V1) -> bool {
        self.matches_source(
            event.hash,
            event.block_height,
            event.lane_id,
            event.dataspace_id,
        ) && self
            .status
            .as_ref()
            .is_none_or(|status| *status == event.status)
    }

    fn matches_source(
        &self,
        hash: HashOf<SignedTransaction>,
        height: Option<NonZeroU64>,
        lane: LaneId,
        data_space: DataSpaceId,
    ) -> bool {
        self.hash.is_none_or(|value| value == hash)
            && self.block_height.is_none_or(|value| value == height)
            && self.lane_id.is_none_or(|value| value == lane)
            && self.dataspace_id.is_none_or(|value| value == data_space)
    }
}

/// Pipeline event filter of a C10 execution root.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::ivm_c10::PipelineEventFilterBoxC10V1")]
pub enum PipelineEventFilterBoxC10V1 {
    /// Transaction events.
    Transaction(TransactionEventFilterC10V1),
    /// Block events (unchanged member).
    Block(BlockEventFilter),
    /// Witness events (unchanged member).
    Witness(WitnessEventFilter),
}

/// Event filter of a C10 trigger or subscription.
#[allow(variant_size_differences)]
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::ivm_c10::EventFilterBoxC10V1")]
pub enum EventFilterBoxC10V1 {
    /// Pipeline events.
    Pipeline(PipelineEventFilterBoxC10V1),
    /// Data events (unchanged member).
    Data(DataEventFilter),
    /// Time events (unchanged member).
    Time(TimeEventFilter),
    /// Trigger execution events (unchanged member).
    ExecuteTrigger(ExecuteTriggerEventFilter),
    /// Trigger completion events (unchanged member).
    TriggerCompleted(TriggerCompletedEventFilter),
}

impl From<TransactionEventFilterC10V1> for EventFilterBoxC10V1 {
    fn from(filter: TransactionEventFilterC10V1) -> Self {
        Self::Pipeline(PipelineEventFilterBoxC10V1::Transaction(filter))
    }
}

impl From<TransactionEventC10V1> for EventBoxC10V1 {
    fn from(event: TransactionEventC10V1) -> Self {
        Self::Pipeline(PipelineEventBoxC10V1::Transaction(event))
    }
}

// Reconstruct each complete payload without resetting its caller's advertised layout, resource
// limits or structured errors, as the baseline event and filter boxes do.
impl<'a> norito::core::DecodeFromSlice<'a> for EventBoxC10V1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        norito::core::decode_field_canonical::<Self>(bytes)
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for EventFilterBoxC10V1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        norito::core::decode_field_canonical::<Self>(bytes)
    }
}

c10_json_via_norito_bytes!(
    TransactionEventC10V1,
    PipelineEventBoxC10V1,
    EventBoxC10V1,
    TransactionEventFilterC10V1,
    PipelineEventFilterBoxC10V1,
    EventFilterBoxC10V1,
);
