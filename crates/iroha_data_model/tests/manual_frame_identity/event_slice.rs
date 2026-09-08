//! Complete event payloads retain their advertised layout and decode limits.

use std::time::Duration;

use iroha_data_model::events::{
    EventBox, EventFilterBox,
    data::DataEventFilter,
    stream::{EventMessage, EventSubscriptionRequest},
    time::{ExecutionTime, TimeEvent, TimeEventFilter, TimeInterval},
};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::{DecodeAll as _, Encode as _},
    core as ncore,
    json::JsonSerialize,
};

fn time_event() -> EventBox {
    EventBox::Time(TimeEvent::new(TimeInterval::new(
        Duration::from_millis(17),
        Duration::from_millis(23),
    )))
}

fn time_filter() -> EventFilterBox {
    EventFilterBox::Time(TimeEventFilter::new(ExecutionTime::PreCommit))
}

fn subscription() -> EventSubscriptionRequest {
    EventSubscriptionRequest::new(vec![EventFilterBox::Data(DataEventFilter::Any)])
}

#[test]
fn event_message_slice_decodes_its_own_complete_payload() {
    let event = time_event();
    let message = EventMessage::new(event.clone());
    let bytes = message.encode();
    let mut cursor = bytes.as_slice();
    let decoded = EventMessage::decode_all(&mut cursor).expect("public bare owner decode");
    assert!(cursor.is_empty());
    assert_eq!(EventBox::from(decoded), event);
    let (decoded, consumed) = <EventMessage as ncore::DecodeFromSlice>::decode_from_slice(&bytes)
        .expect("slice decoder reconstructs the complete EventMessage owner");
    assert_eq!(consumed, bytes.len());
    assert_eq!(decoded.encode(), bytes);
    assert_eq!(EventBox::from(decoded), event);
    let decoded = norito::codec::decode_exact_from_slice::<EventMessage>(&bytes)
        .expect("public exact slice entry point");
    assert_eq!(EventBox::from(decoded), event);
    assert!(
        norito::codec::decode_exact_from_slice::<EventMessage>(&event.encode()).is_err(),
        "a wrapped event cannot replace the complete message payload"
    );
}

fn assert_advertised_layout<T>(value: &T)
where
    T: std::fmt::Debug
        + NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'de> ncore::DecodeFromSlice<'de>
        + JsonSerialize,
{
    let expected = norito::json::to_json(value).unwrap();
    let default = ncore::default_encode_flags();
    for flags in [default, default ^ ncore::header_flags::COMPACT_LEN] {
        let frame = {
            let _flags = ncore::DecodeFlagsGuard::enter(flags);
            norito::to_bytes(value).expect("encode advertised layout")
        };
        let view = ncore::from_bytes_view(&frame).expect("authenticate owner frame");
        assert_eq!(view.flags(), flags);
        if flags != default {
            assert!(
                norito::decode_canonical::<T>(&frame).is_err(),
                "alternate advertised archives do not become canonical V1 messages"
            );
        }
        let control = view
            .decode_exact_with(ncore::decode_field_canonical::<T>)
            .expect("payload reconstruction honors the advertised layout");
        assert_eq!(norito::json::to_json(&control).unwrap(), expected);
        let actual = view
            .decode_exact_with(<T as ncore::DecodeFromSlice>::decode_from_slice)
            .expect("slice reconstruction retains the advertised layout");
        assert_eq!(norito::json::to_json(&actual).unwrap(), expected);
        let decoded = ncore::decode_from_bytes::<T>(&frame)
            .expect("framed slice entry point retains the advertised layout");
        assert_eq!(norito::json::to_json(&decoded).unwrap(), expected);
        let (decoded, used) = {
            let _flags = ncore::DecodeFlagsGuard::enter(flags);
            let result = <T as ncore::DecodeFromSlice>::decode_from_slice(view.as_bytes())
                .expect("caller-selected payload layout");
            assert_eq!(ncore::effective_decode_flags(), Some(flags));
            let mut reencoded = Vec::new();
            ncore::serialize_to_buffer(&result.0, &mut reencoded).unwrap();
            assert_eq!(reencoded, view.as_bytes(), "exact advertised payload bytes");
            result
        };
        assert_eq!(used, view.as_bytes().len());
        assert_eq!(norito::json::to_json(&decoded).unwrap(), expected);
    }
}

fn assert_bounded_slice<T>(value: &T)
where
    T: std::fmt::Debug
        + NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'de> ncore::DecodeFromSlice<'de>,
{
    let bytes = value.encode();
    assert!(bytes.len() > 1);
    let flags = ncore::default_encode_flags();
    let _flags = ncore::DecodeFlagsGuard::enter(flags);
    let field_limit = bytes.len() - 1;
    let limited = ncore::DecodeLimits::new(usize::MAX, field_limit, usize::MAX, usize::MAX, 128);
    let error = ncore::with_decode_limits(limited, || {
        <T as ncore::DecodeFromSlice>::decode_from_slice(&bytes)
    })
    .expect_err("complete owner bytes obey the caller's field limit");
    assert!(matches!(
        error,
        ncore::Error::FieldLengthExceeded { length, limit }
            if length == u64::try_from(bytes.len()).unwrap()
                && limit == u64::try_from(field_limit).unwrap()
    ));
    let no_depth = ncore::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 0);
    let error = ncore::with_decode_limits(no_depth, || {
        <T as ncore::DecodeFromSlice>::decode_from_slice(&bytes)
    })
    .expect_err("complete owner reconstruction obeys the caller's nesting limit");
    assert!(matches!(
        error,
        ncore::Error::NestingDepthExceeded { limit: 0, .. }
    ));
    assert!(<T as ncore::DecodeFromSlice>::decode_from_slice(&bytes[..bytes.len() - 1]).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(<T as ncore::DecodeFromSlice>::decode_from_slice(&trailing).is_err());
    assert_eq!(ncore::effective_decode_flags(), Some(flags));
    let (decoded, used) = <T as ncore::DecodeFromSlice>::decode_from_slice(&bytes)
        .expect("valid decode after scoped limits and malformed controls");
    assert_eq!(used, bytes.len());
    let mut reencoded = Vec::new();
    ncore::serialize_to_buffer(&decoded, &mut reencoded).unwrap();
    assert_eq!(reencoded, bytes);
}

#[test]
fn event_message_slice_honors_advertised_layout() {
    assert_advertised_layout(&EventMessage::new(time_event()));
}

#[test]
fn event_box_slice_honors_advertised_layout() {
    assert_advertised_layout(&time_event());
}

#[test]
fn event_filter_slice_honors_advertised_layout() {
    assert_advertised_layout(&time_filter());
}

#[test]
fn subscription_slice_honors_advertised_layout() {
    assert_advertised_layout(&subscription());
}

#[test]
fn event_message_slice_preserves_typed_limits_and_exact_consumption() {
    assert_bounded_slice(&EventMessage::new(time_event()));
}

#[test]
fn event_box_slice_preserves_typed_limits_and_exact_consumption() {
    assert_bounded_slice(&time_event());
}

#[test]
fn event_filter_slice_preserves_typed_limits_and_exact_consumption() {
    assert_bounded_slice(&time_filter());
}

#[test]
fn subscription_slice_preserves_typed_limits_and_exact_consumption() {
    assert_bounded_slice(&subscription());
}
