//! Events for streaming API.

use std::{format, ops::Deref, string::String, sync::Arc, vec::Vec};

use iroha_data_model_derive::model;
use iroha_macro::FromVariant;
use iroha_schema::{Ident, IntoSchema, MetaMap, TypeId};
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize};
use pipeline::{BlockEvent, TransactionEvent};

pub use crate::{Decode, Encode};
macro_rules! impl_json_via_norito_bytes {
    ($($ty:path),+ $(,)?) => {
        $(
            impl norito::json::FastJsonWrite for $ty {
                fn write_json(&self, out: &mut String) {
                    let bytes = norito::to_bytes(self)
                        .expect("Norito serialization must succeed");
                    let encoded = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        bytes,
                    );
                    norito::json::JsonSerialize::json_serialize(&encoded, out);
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
                    let archived = norito::from_bytes::<$ty>(&bytes)
                        .map_err(|err| norito::json::Error::Message(err.to_string()))?;
                    norito::core::NoritoDeserialize::try_deserialize(archived)
                        .map_err(|err| norito::json::Error::Message(err.to_string()))
                }
            }
        )+
    };
}

pub use self::model::*;

/// Shared, reference-counted wrapper for [`data::DataEvent`] to avoid repeated cloning.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
pub struct SharedDataEvent(Arc<data::DataEvent>);

impl SharedDataEvent {
    /// Wrap an existing [`Arc`] around a [`data::DataEvent`].
    pub fn from_arc(event: Arc<data::DataEvent>) -> Self {
        Self(event)
    }

    /// Consume the wrapper and return the inner [`Arc`].
    pub fn into_arc(self) -> Arc<data::DataEvent> {
        self.0
    }

    /// Borrow the underlying [`Arc`].
    pub fn as_arc(&self) -> &Arc<data::DataEvent> {
        &self.0
    }
}

impl Deref for SharedDataEvent {
    type Target = data::DataEvent;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl From<data::DataEvent> for SharedDataEvent {
    fn from(event: data::DataEvent) -> Self {
        Self(Arc::new(event))
    }
}

impl From<Arc<data::DataEvent>> for SharedDataEvent {
    fn from(event: Arc<data::DataEvent>) -> Self {
        Self(event)
    }
}

impl AsRef<data::DataEvent> for SharedDataEvent {
    fn as_ref(&self) -> &data::DataEvent {
        self.0.as_ref()
    }
}
/// Data-oriented event payloads and filters.
pub mod data;
/// Events emitted when triggers are executed.
pub mod execute_trigger;
/// Pipeline events produced by core processing stages.
pub mod pipeline;
/// Time-based event notifications.
pub mod time;
/// Events indicating completion of trigger execution.
pub mod trigger_completed;

#[cfg(test)]
mod tests {
    use std::{str::FromStr, sync::Arc};

    use iroha_crypto::Hash;
    use iroha_primitives::json::Json;

    use super::*;
    use crate::{
        domain::DomainId,
        events::data::prelude::{DataEvent, DomainEvent, MetadataChanged},
        name::Name,
    };

    #[test]
    fn shared_data_event_from_arc_preserves_arc_pointer() {
        let metadata = MetadataChanged {
            target: DomainId::try_new("wonderland", "universal").expect("domain id"),
            key: Name::from_str("flag").expect("metadata key"),
            value: Json::from(norito::json!("ok")),
        };
        let event = Arc::new(DataEvent::Domain(DomainEvent::MetadataInserted(metadata)));
        let shared = SharedDataEvent::from_arc(Arc::clone(&event));
        assert!(Arc::ptr_eq(shared.as_arc(), &event));
    }

    #[cfg(feature = "transparent_api")]
    #[test]
    fn pipeline_batch_filters_match_any_event() {
        use crate::events::pipeline::{
            PipelineEventBox, TransactionEvent, TransactionEventFilter, TransactionStatus,
        };
        use crate::nexus::{DataSpaceId, LaneId};
        use iroha_crypto::HashOf;

        let hash_a = HashOf::from_untyped_unchecked(Hash::prehashed([1_u8; Hash::LENGTH]));
        let hash_b = HashOf::from_untyped_unchecked(Hash::prehashed([2_u8; Hash::LENGTH]));

        let ev_a: PipelineEventBox = TransactionEvent {
            hash: hash_a,
            block_height: None,
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            status: TransactionStatus::Queued,
        }
        .into();
        let ev_b: PipelineEventBox = TransactionEvent {
            hash: hash_b,
            block_height: None,
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            status: TransactionStatus::Queued,
        }
        .into();

        let event = EventBox::PipelineBatch(vec![ev_a.clone(), ev_b.clone()]);
        let filter: EventFilterBox = TransactionEventFilter::default().for_hash(hash_a).into();
        assert!(filter.matches(&event));

        let hash_c = HashOf::from_untyped_unchecked(Hash::prehashed([3_u8; Hash::LENGTH]));
        let filter: EventFilterBox = TransactionEventFilter::default().for_hash(hash_c).into();
        assert!(!filter.matches(&event));
    }
}

#[model]
mod model {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, FromVariant, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    /// Top-level wrapper for all streamed event payloads.
    pub enum EventBox {
        /// Pipeline event.
        Pipeline(pipeline::PipelineEventBox),
        /// Batch of pipeline events emitted together.
        PipelineBatch(Vec<pipeline::PipelineEventBox>),
        /// Data event.
        Data(#[skip_from] super::SharedDataEvent),
        /// Time event.
        Time(time::TimeEvent),
        /// Trigger execution event.
        ExecuteTrigger(execute_trigger::ExecuteTriggerEvent),
        /// Trigger completion event.
        TriggerCompleted(trigger_completed::TriggerCompletedEvent),
    }

    /// Event type which could invoke trigger execution.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
    pub enum TriggeringEventType {
        /// Pipeline event.
        Pipeline,
        /// Data event.
        Data,
        /// Time event.
        Time,
        /// Trigger execution event.
        ExecuteTrigger,
    }

    /// Event filter.
    #[allow(variant_size_differences)]
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, FromVariant, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum EventFilterBox {
        /// Listen to pipeline events with filter.
        Pipeline(pipeline::PipelineEventFilterBox),
        /// Listen to data events with filter.
        // NOTE: `FromVariant` derives a `TryFrom<EventFilterBox> for DataEventFilter` impl
        // which now conflicts with a blanket `TryFrom<U> for T where U: Into<T>` in `core`.
        // Disable the generated `TryFrom` for this variant to avoid overlap.
        Data(#[skip_try_from] data::DataEventFilter),
        /// Listen to time events with filter.
        Time(time::TimeEventFilter),
        /// Listen to trigger execution event with filter.
        ExecuteTrigger(execute_trigger::ExecuteTriggerEventFilter),
        /// Listen to trigger completion event with filter.
        TriggerCompleted(trigger_completed::TriggerCompletedEventFilter),
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for TriggeringEventType {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            TriggeringEventType::Pipeline => "Pipeline",
            TriggeringEventType::Data => "Data",
            TriggeringEventType::Time => "Time",
            TriggeringEventType::ExecuteTrigger => "ExecuteTrigger",
        };
        json::write_json_string(label, out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for TriggeringEventType {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "Pipeline" => Ok(TriggeringEventType::Pipeline),
            "Data" => Ok(TriggeringEventType::Data),
            "Time" => Ok(TriggeringEventType::Time),
            "ExecuteTrigger" => Ok(TriggeringEventType::ExecuteTrigger),
            other => Err(json::Error::UnknownField {
                field: other.to_owned(),
            }),
        }
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(EventBox, EventFilterBox);

impl From<TransactionEvent> for EventBox {
    fn from(source: TransactionEvent) -> Self {
        Self::Pipeline(source.into())
    }
}

impl From<BlockEvent> for EventBox {
    fn from(source: BlockEvent) -> Self {
        Self::Pipeline(source.into())
    }
}

// Provide slice decoding via Norito codec for core consumers that expect
// `norito::core::DecodeFromSlice`. Delegate to the header-framed codec decoder.
impl<'a> norito::core::DecodeFromSlice<'a> for EventBox {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut s: &'a [u8] = bytes;
        let value = <Self as norito::codec::DecodeAll>::decode_all(&mut s)
            .map_err(|e| norito::core::Error::Message(format!("codec decode error: {e}")))?;
        let used = bytes.len() - s.len();
        Ok((value, used))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for EventFilterBox {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut s: &'a [u8] = bytes;
        let value = <Self as norito::codec::DecodeAll>::decode_all(&mut s)
            .map_err(|e| norito::core::Error::Message(format!("codec decode error: {e}")))?;
        let used = bytes.len() - s.len();
        Ok((value, used))
    }
}

impl EventBox {
    /// Returns the shared data payload if this box stores a [`data::DataEvent`].
    #[inline]
    pub const fn as_shared_data_event(&self) -> Option<&SharedDataEvent> {
        match self {
            Self::Data(event) => Some(event),
            _ => None,
        }
    }

    /// Returns the data payload if this box stores a [`data::DataEvent`].
    #[inline]
    pub fn as_data_event(&self) -> Option<&data::DataEvent> {
        self.as_shared_data_event().map(SharedDataEvent::as_ref)
    }

    /// Consumes the box and returns the shared data payload if present.
    /// # Errors
    ///
    /// Returns [`EventBox`] if the boxed event does not wrap a [`SharedDataEvent`].
    #[inline]
    #[allow(clippy::result_large_err)]
    pub fn into_shared_data_event(self) -> Result<SharedDataEvent, EventBox> {
        match self {
            Self::Data(event) => Ok(event),
            other => Err(other),
        }
    }
}

impl TryFrom<EventBox> for TransactionEvent {
    type Error = iroha_macro::error::ErrorTryFromEnum<EventBox, Self>;

    fn try_from(event: EventBox) -> Result<Self, Self::Error> {
        use iroha_macro::error::ErrorTryFromEnum;

        let EventBox::Pipeline(pipeline_event) = event else {
            return Err(ErrorTryFromEnum::default());
        };

        pipeline_event
            .try_into()
            .map_err(|_| ErrorTryFromEnum::default())
    }
}

impl TryFrom<EventBox> for BlockEvent {
    type Error = iroha_macro::error::ErrorTryFromEnum<EventBox, Self>;

    fn try_from(event: EventBox) -> Result<Self, Self::Error> {
        use iroha_macro::error::ErrorTryFromEnum;

        let EventBox::Pipeline(pipeline_event) = event else {
            return Err(ErrorTryFromEnum::default());
        };

        pipeline_event
            .try_into()
            .map_err(|_| ErrorTryFromEnum::default())
    }
}

/// Trait for filters
#[cfg(feature = "transparent_api")]
pub trait EventFilter {
    /// Type of event that can be filtered
    type Event;

    /// Check if `item` matches filter
    ///
    /// Returns `true`, if `item` matches filter and `false` if not
    fn matches(&self, event: &Self::Event) -> bool;

    /// Returns a number of times trigger should be executed for
    ///
    /// Used for time-triggers
    #[inline]
    fn count_matches(&self, event: &Self::Event) -> u32 {
        self.matches(event).into()
    }

    /// Check if filter is mintable.
    ///
    /// Returns `true` by default. Used for time-triggers
    #[inline]
    fn mintable(&self) -> bool {
        true
    }
}

#[cfg(feature = "transparent_api")]
impl EventFilter for EventFilterBox {
    type Event = EventBox;

    /// Apply filter to event.
    fn matches(&self, event: &EventBox) -> bool {
        match (event, self) {
            (EventBox::Pipeline(event), Self::Pipeline(filter)) => filter.matches(event),
            (EventBox::PipelineBatch(events), Self::Pipeline(filter)) => {
                events.iter().any(|event| filter.matches(event))
            }
            (EventBox::Data(event), Self::Data(filter)) => filter.matches(event),
            (EventBox::Time(event), Self::Time(filter)) => filter.matches(event),
            (EventBox::ExecuteTrigger(event), Self::ExecuteTrigger(filter)) => {
                filter.matches(event)
            }
            (EventBox::TriggerCompleted(event), Self::TriggerCompleted(filter)) => {
                filter.matches(event)
            }
            // Fail to compile in case when new variant to event or filter is added
            (
                EventBox::Pipeline(_)
                | EventBox::PipelineBatch(_)
                | EventBox::Data(_)
                | EventBox::Time(_)
                | EventBox::ExecuteTrigger(_)
                | EventBox::TriggerCompleted(_),
                Self::Pipeline(_)
                | Self::Data(_)
                | Self::Time(_)
                | Self::ExecuteTrigger(_)
                | Self::TriggerCompleted(_),
            ) => false,
        }
    }

    fn mintable(&self) -> bool {
        match self {
            Self::Time(filter) => filter.mintable(),
            _ => true,
        }
    }
}

mod conversions {
    use super::{
        pipeline::{BlockEventFilter, TransactionEventFilter},
        prelude::*,
    };

    macro_rules! last_tt {
        ($last:tt) => {
            $last
        };
        ($head:tt $($tail:tt)+) => {
            last_tt!($($tail)*)
        };
    }

    // chain multiple conversions into one
    macro_rules! impl_from_via_path {
        ($($initial:ty $(=> $intermediate:ty)*),+ $(,)?) => {
            $(
                impl From<$initial> for last_tt!($($intermediate)*) {
                    fn from(filter: $initial) -> Self {
                        $(
                            let filter: $intermediate = filter.into();
                        )*
                        filter
                    }
                }
            )+
        };
    }

    impl_from_via_path! {
        PeerEventFilter             => DataEventFilter => EventFilterBox,
        DomainEventFilter           => DataEventFilter => EventFilterBox,
        AccountEventFilter          => DataEventFilter => EventFilterBox,
        AssetEventFilter            => DataEventFilter => EventFilterBox,
        AssetDefinitionEventFilter  => DataEventFilter => EventFilterBox,
        NftEventFilter              => DataEventFilter => EventFilterBox,
        RwaEventFilter              => DataEventFilter => EventFilterBox,
        TriggerEventFilter          => DataEventFilter => EventFilterBox,
        RoleEventFilter             => DataEventFilter => EventFilterBox,
        ConfigurationEventFilter    => DataEventFilter => EventFilterBox,
        ExecutorEventFilter         => DataEventFilter => EventFilterBox,

        TransactionEventFilter => PipelineEventFilterBox => EventFilterBox,
        BlockEventFilter       => PipelineEventFilterBox => EventFilterBox,
    }
}

impl TypeId for SharedDataEvent {
    fn id() -> Ident {
        data::DataEvent::id()
    }
}

impl IntoSchema for SharedDataEvent {
    fn type_name() -> Ident {
        data::DataEvent::type_name()
    }

    fn update_schema_map(metamap: &mut MetaMap) {
        data::DataEvent::update_schema_map(metamap);
    }
}

#[cfg(feature = "http")]
pub mod stream {
    //! Structures related to event streaming over HTTP

    use derive_more::Constructor;
    use iroha_data_model_derive::model;
    use iroha_version::prelude::*;

    pub use self::model::*;
    use super::*;

    #[model]
    mod model {
        use super::*;

        /// Message sent by the stream producer.
        /// Event sent by the peer.
        #[derive(Debug, Clone, Decode, Encode, IntoSchema, Constructor)]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[repr(transparent)]
        pub struct EventMessage(pub EventBox);

        /// Message sent by the stream consumer.
        /// Request sent by the client to subscribe to events.
        ///
        /// Proof filters are bundled with the primary subscription message to keep
        /// the WebSocket format single-shot and deterministic for the first release.
        #[derive(Debug, Clone, Decode, Encode, IntoSchema)]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        pub struct EventSubscriptionRequest {
            /// Event filters applied to the stream.
            pub filters: Vec<EventFilterBox>,
            /// Accept only these proof backends (empty or None disables backend filtering).
            #[norito(default)]
            pub proof_backend: Option<Vec<String>>,
            /// Accept only these call hashes (empty or None disables call hash filtering).
            #[norito(default)]
            #[cfg_attr(
                feature = "json",
                norito(with = "crate::json_helpers::fixed_bytes::option_vec")
            )]
            pub proof_call_hash: Option<Vec<[u8; 32]>>,
            /// Accept only these envelope hashes (empty or None disables envelope hash filtering).
            #[norito(default)]
            #[cfg_attr(
                feature = "json",
                norito(with = "crate::json_helpers::fixed_bytes::option_vec")
            )]
            pub proof_envelope_hash: Option<Vec<[u8; 32]>>,
        }

        impl EventSubscriptionRequest {
            /// Create a subscription request without proof-specific filters.
            #[must_use]
            pub fn new(filters: Vec<EventFilterBox>) -> Self {
                Self {
                    filters,
                    proof_backend: None,
                    proof_call_hash: None,
                    proof_envelope_hash: None,
                }
            }
        }

        // Provide slice decoding via Norito codec for HTTP payloads
        impl<'a> norito::core::DecodeFromSlice<'a> for EventMessage {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let (inner, used) =
                    <EventBox as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
                Ok((EventMessage(inner), used))
            }
        }
        impl<'a> norito::core::DecodeFromSlice<'a> for EventSubscriptionRequest {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let mut s: &'a [u8] = bytes;
                let value =
                    <Self as norito::codec::DecodeAll>::decode_all(&mut s).map_err(|e| {
                        norito::core::Error::Message(format!("codec decode error: {e}"))
                    })?;
                let used = bytes.len() - s.len();
                Ok((value, used))
            }
        }
    }

    impl From<EventMessage> for EventBox {
        fn from(source: EventMessage) -> Self {
            source.0
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::events::data::prelude::DataEventFilter;

        #[test]
        fn subscription_request_new_sets_defaults() {
            let filters = vec![EventFilterBox::Data(DataEventFilter::Any)];
            let request = EventSubscriptionRequest::new(filters.clone());
            assert_eq!(request.filters, filters);
            assert!(request.proof_backend.is_none());
            assert!(request.proof_call_hash.is_none());
            assert!(request.proof_envelope_hash.is_none());
        }

        #[test]
        fn subscription_request_roundtrips_proof_filters() {
            let request = EventSubscriptionRequest {
                filters: vec![EventFilterBox::Data(DataEventFilter::Any)],
                proof_backend: Some(vec!["halo2/ipa".to_string()]),
                proof_call_hash: Some(vec![[0x11; 32]]),
                proof_envelope_hash: Some(vec![[0x22; 32]]),
            };
            let bytes = norito::to_bytes(&request).expect("encode request");
            let decoded: EventSubscriptionRequest =
                norito::decode_from_bytes(&bytes).expect("decode request");
            assert_eq!(decoded.filters, request.filters);
            assert_eq!(decoded.proof_backend, request.proof_backend);
            assert_eq!(decoded.proof_call_hash, request.proof_call_hash);
            assert_eq!(decoded.proof_envelope_hash, request.proof_envelope_hash);
        }
    }
}

/// Exports common structs and enums from this module.
pub mod prelude {
    #[cfg(feature = "transparent_api")]
    pub use super::EventFilter;
    #[cfg(feature = "http")]
    pub use super::stream::{EventMessage, EventSubscriptionRequest};
    pub use super::{
        EventBox, EventFilterBox, TriggeringEventType, data::prelude::*,
        execute_trigger::prelude::*, pipeline::prelude::*, time::prelude::*,
        trigger_completed::prelude::*,
    };
}
