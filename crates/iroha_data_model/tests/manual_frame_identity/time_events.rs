//! Captured public time-event frames and checked reconstruction contracts.

use iroha_data_model::events::time::{
    ExecutionTime, Schedule, TimeEvent, TimeEventFilter, TimeInterval,
};
use norito::core as ncore;
use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::Encode as _,
    json::{JsonDeserialize, JsonSerialize, Value},
};
use std::{fmt::Debug, time::Duration};

use crate::frame_identity_test_support::record;

fn family<T>(rows: &mut Vec<Value>, name: &str, values: &[T])
where
    T: norito::NoritoSchema
        + Clone
        + Debug
        + PartialEq
        + NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + JsonSerialize
        + JsonDeserialize,
{
    assert!(!values.is_empty());
    for (index, value) in values.iter().enumerate() {
        assert!(
            !values[..index].contains(value),
            "distinct populated variants"
        );
        record(rows, &format!("{name}/root_{index}"), value);
        record(
            rows,
            &format!("{name}/option_{index}"),
            &Some(value.clone()),
        );
    }
    record(rows, &format!("{name}/option_none"), &None::<T>);
    record(rows, &format!("{name}/vec_empty"), &Vec::<T>::new());
    record(rows, &format!("{name}/vec_all"), &values.to_vec());
}

fn bounded_json<T: JsonSerialize>(value: &T) {
    let expected = norito::json::to_json(value).unwrap();
    assert_eq!(
        norito::json::to_json_bounded(value, expected.len()).unwrap(),
        expected
    );
    assert_eq!(
        norito::json::to_json_bounded(value, expected.len() - 1),
        Err(norito::json::BoundedJsonError::BodyTooLarge)
    );
}

fn schedule(start_ms: u64, period_ms: Option<u64>) -> Schedule {
    let value = Schedule::starting_at(Duration::from_millis(start_ms));
    let value = period_ms.map_or(value, |period| {
        value.with_period(Duration::from_millis(period))
    });
    assert_eq!(value.start(), Duration::from_millis(start_ms));
    assert_eq!(value.period(), period_ms.map(Duration::from_millis));
    value
}

fn event(since_ms: u64, length_ms: u64) -> TimeEvent {
    let interval = TimeInterval::new(
        Duration::from_millis(since_ms),
        Duration::from_millis(length_ms),
    );
    let value = TimeEvent::new(interval);
    assert_eq!(value.interval(), &interval);
    assert_eq!(value.interval().since(), Duration::from_millis(since_ms));
    assert_eq!(value.interval().length(), Duration::from_millis(length_ms));
    value
}

fn payload<T: ncore::SerializePayload>(value: &T) -> Vec<u8> {
    let _flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
    let mut bytes = Vec::new();
    ncore::serialize_to_buffer(value, &mut bytes).unwrap();
    bytes
}

fn append_payload(bytes: &mut Vec<u8>, field: &[u8]) {
    let _flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
    ncore::write_len_header_to_vec(bytes, u64::try_from(field.len()).unwrap());
    bytes.extend_from_slice(field);
}

fn pair_payload(first: &[u8], second: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    append_payload(&mut bytes, first);
    append_payload(&mut bytes, second);
    bytes
}

fn reject_payload<T>(bytes: &[u8])
where
    T: Debug + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let flags = ncore::default_encode_flags();
    let frame = ncore::frame_bare_with_header_flags::<T>(bytes, flags)
        .expect("frame invalid fields with the correct schema, length and checksum");
    let view = ncore::from_bytes_view(&frame).expect("authenticate malformed-field frame");
    assert_eq!(view.as_bytes(), bytes);
    view.decode_exact_with(ncore::decode_field_canonical::<T>)
        .expect_err("checked reconstruction must reject invalid fields");
    assert!(norito::decode_canonical::<T>(&frame).is_err());
    let _flags = ncore::DecodeFlagsGuard::enter(flags);
    let _payload = ncore::PayloadCtxGuard::enter_with_schema_and_flags(bytes, view.schema(), flags);
    let archived = ncore::from_bytes::<T>(&frame).expect("authenticate typed archive metadata");
    <T as ncore::DeserializePayload<'_>>::try_deserialize(archived)
        .expect_err("fallible owner decoding must reject without panicking");
}

fn field_rejections() {
    let start = payload(&1_000_u64);
    let period = payload(&Some(250_u64));
    let valid_schedule = schedule(1_000, Some(250));
    assert_eq!(
        pair_payload(&start, &period),
        valid_schedule.encode(),
        "positive control reproduces all schedule fields"
    );
    reject_payload::<Schedule>(&pair_payload(&start[..start.len() - 1], &period));
    reject_payload::<Schedule>(&pair_payload(&start, &period[..period.len() - 1]));

    let since = payload(&10_u64);
    let length = payload(&20_u64);
    let valid_event = event(10, 20);
    let interval = pair_payload(&since, &length);
    assert_eq!(
        interval,
        valid_event.interval().encode(),
        "complete nested interval control"
    );
    let mut event_payload = Vec::new();
    append_payload(&mut event_payload, &interval);
    assert_eq!(
        event_payload,
        valid_event.encode(),
        "complete event field control"
    );
    let short_since = pair_payload(&since[..since.len() - 1], &length);
    let mut invalid_event = Vec::new();
    append_payload(&mut invalid_event, &short_since);
    reject_payload::<TimeEvent>(&invalid_event);

    for (tag, execution) in [
        (0_u32, ExecutionTime::PreCommit),
        (1_u32, ExecutionTime::Schedule(valid_schedule)),
    ] {
        let mut unknown = execution.encode();
        assert_eq!(
            &unknown[..4],
            &tag.to_le_bytes(),
            "observed declared variant tag"
        );
        unknown[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        reject_payload::<ExecutionTime>(&unknown);
        let filter = TimeEventFilter::new(execution);
        let mut filter_control = Vec::new();
        append_payload(&mut filter_control, &execution.encode());
        assert_eq!(
            filter_control,
            filter.encode(),
            "complete public filter field control"
        );
        let mut invalid_filter = Vec::new();
        append_payload(&mut invalid_filter, &unknown);
        reject_payload::<TimeEventFilter>(&invalid_filter);
    }
}

fn json_rejections_and_defaults() {
    let omitted: Schedule = norito::json::from_json(r#"{"start_ms":500}"#).unwrap();
    assert_eq!(omitted, schedule(500, None));
    assert_eq!(
        norito::json::to_json(&omitted).unwrap(),
        r#"{"start_ms":500,"period_ms":null}"#
    );
    // These are structural wire records. Zero periods are representable here;
    // Core trigger registration owns the zero and signed-cadence admission checks.
    let zero: Schedule = norito::json::from_json(r#"{"start_ms":17,"period_ms":0}"#).unwrap();
    assert_eq!(zero, schedule(17, Some(0)));
    assert_ne!(zero, schedule(17, None));
    for json in [
        "null",
        "[]",
        "{}",
        r#"{"period_ms":1}"#,
        r#"{"start_ms":-1}"#,
        r#"{"start_ms":1.5}"#,
        r#"{"start_ms":"1"}"#,
        r#"{"start_ms":1,"period_ms":-1}"#,
        r#"{"start_ms":1,"period_ms":1.5}"#,
        r#"{"start_ms":1,"period_ms":"1"}"#,
        r#"{"start_ms":1,"extra":0}"#,
    ] {
        assert!(norito::json::from_json::<Schedule>(json).is_err(), "{json}");
    }
    for json in [
        "null",
        "[]",
        "{}",
        r#""Unknown""#,
        r#"{"PreCommit":null}"#,
        r#"{"Schedule":{}}"#,
        r#"{"Schedule":{"start_ms":-1}}"#,
        r#"{"Schedule":{"start_ms":0},"extra":0}"#,
    ] {
        assert!(
            norito::json::from_json::<ExecutionTime>(json).is_err(),
            "{json}"
        );
        assert!(
            norito::json::from_json::<TimeEventFilter>(json).is_err(),
            "{json}"
        );
    }
    for json in [
        "null",
        "[]",
        "{}",
        r#"{"interval":{}}"#,
        r#"{"interval":{"since_ms":-1,"length_ms":1}}"#,
        r#"{"interval":{"since_ms":0,"length_ms":1},"extra":0}"#,
    ] {
        assert!(
            norito::json::from_json::<TimeEvent>(json).is_err(),
            "{json}"
        );
    }
    let valid = ExecutionTime::Schedule(omitted);
    assert_eq!(
        norito::json::from_json::<ExecutionTime>(r#"{"Schedule":{"start_ms":500}}"#).unwrap(),
        valid
    );
    assert_eq!(
        norito::json::from_json::<TimeEventFilter>(r#"{"Schedule":{"start_ms":500}}"#).unwrap(),
        TimeEventFilter::new(valid)
    );
}

fn capture_values() -> Vec<Value> {
    field_rejections();
    json_rejections_and_defaults();
    let schedules = [
        schedule(0, None),
        schedule(17, Some(0)),
        schedule(1_000, Some(250)),
        schedule(u64::MAX, Some(u64::MAX)),
    ];
    let mut execution = vec![ExecutionTime::PreCommit];
    execution.extend(schedules.iter().copied().map(ExecutionTime::Schedule));
    let filters: Vec<_> = execution
        .iter()
        .copied()
        .map(TimeEventFilter::new)
        .collect();
    let events = [event(0, 0), event(10, 20), event(u64::MAX, u64::MAX)];
    for (inner, filter) in execution.iter().zip(&filters) {
        assert_eq!(
            norito::json::to_json(inner).unwrap(),
            norito::json::to_json(filter).unwrap()
        );
        let inner_frame = norito::encode_canonical(inner).unwrap();
        let filter_frame = norito::encode_canonical(filter).unwrap();
        assert_ne!(inner_frame, filter_frame);
        assert!(norito::decode_canonical::<ExecutionTime>(&filter_frame).is_err());
        assert!(norito::decode_canonical::<TimeEventFilter>(&inner_frame).is_err());
        bounded_json(inner);
        bounded_json(filter);
    }
    for value in &schedules {
        bounded_json(value);
    }
    for value in &events {
        bounded_json(value);
    }
    let mut rows = Vec::new();
    family(&mut rows, "schedule", &schedules);
    family(&mut rows, "execution_time", &execution);
    family(&mut rows, "time_event_filter", &filters);
    family(&mut rows, "time_event", &events);
    assert_eq!(rows.len(), 46);
    rows
}

#[test]
fn public_time_event_frames_match_capture() {
    let evidence = norito::json!({
        "format_version": 1,
        "purpose": "public time-event owners before identity declaration",
        "default_encode_flags": (ncore::default_encode_flags()),
        "rows": (capture_values()),
    });
    let captured: Value =
        norito::json::from_json(include_str!("../fixtures/time_event_identity_frames.json"))
            .expect("decode immutable pre-declaration capture");
    assert_eq!(evidence, captured);
}
