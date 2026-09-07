//! Tests covering the `EventSet` derive macro behaviour.
#![allow(unexpected_cfgs)]
mod events {
    use iroha_data_model_derive::EventSet;
    /// Test event enumeration used with the `EventSet` derive.
    #[derive(EventSet)]
    #[event_set(schema_name = "derive_integration::event_set::events::TestEventSet")]
    pub enum TestEvent {
        Event1,
        Event2,
        NestedEvent(AnotherEvent),
    }
    /// Dummy nested event type used in the tests.
    pub struct AnotherEvent;
}
use events::{AnotherEvent, TestEvent, TestEventSet};
use norito::json::{self, Value};
#[test]
fn event_set_preserves_captured_identity() {
    assert_eq!(
        <TestEventSet as norito::NoritoSchema>::nominal_name(),
        "derive_integration::event_set::events::TestEventSet",
    );
    let captured = [
        159, 196, 218, 105, 112, 11, 120, 9, 55, 253, 142, 185, 70, 208, 133, 139,
    ];
    assert_eq!(
        norito::schema::identity::frame_hash::<TestEventSet>(),
        captured
    );
    assert_eq!(
        <TestEventSet as norito::NoritoSerialize>::schema_hash(),
        captured
    );
    assert_eq!(
        <TestEventSet as norito::NoritoDeserialize>::schema_hash(),
        captured
    );
}
fn array(strings: &[&str]) -> Value {
    json::array(strings.iter().copied()).expect("serialize string array")
}

#[test]
fn production_event_sets_preserve_captured_frames() {
    fn hex(bytes: &[u8]) -> String {
        use std::fmt::Write;
        let mut encoded = String::new();
        for byte in bytes {
            write!(encoded, "{byte:02x}").expect("writing to a String cannot fail");
        }
        encoded
    }
    fn framed<T>(value: &T) -> String
    where
        T: norito::NoritoSerialize
            + for<'de> norito::NoritoDeserialize<'de>
            + core::fmt::Debug
            + PartialEq,
    {
        let bytes = norito::to_bytes(value).expect("frame encoding");
        let decoded = norito::decode_from_bytes::<T>(&bytes).expect("frame decoding");
        assert_eq!(&decoded, value);
        hex(&bytes)
    }
    let mut rows = Vec::new();
    macro_rules! record {
        ($ty:ty) => {{
            assert_eq!(
                norito::schema::identity::frame_hash::<$ty>(),
                <$ty as norito::NoritoSerialize>::schema_hash(),
            );
            let all_json = json::to_value(&<$ty>::all()).expect("all event names");
            let first_json =
                Value::Array(vec![all_json.as_array().expect("event names")[0].clone()]);
            let first: $ty = json::from_value(first_json).expect("first event");
            let cases: Vec<Value> = [<$ty>::empty(), first, <$ty>::all()]
                .into_iter()
                .map(|value| {
                    json::object([
                        ("json", json::to_value(&value).expect("event JSON")),
                        ("frame", Value::String(framed(&value))),
                        ("vector_frame", Value::String(framed(&vec![value]))),
                        ("option_frame", Value::String(framed(&Some(value)))),
                    ])
                    .expect("case object")
                })
                .collect();
            rows.push(
                json::object([
                    (
                        "nominal",
                        Value::String(<$ty as norito::NoritoSchema>::nominal_name()),
                    ),
                    (
                        "serialize_hash",
                        Value::String(hex(&<$ty as norito::NoritoSerialize>::schema_hash())),
                    ),
                    (
                        "deserialize_hash",
                        Value::String(hex(&<$ty as norito::NoritoDeserialize>::schema_hash())),
                    ),
                    ("cases", Value::Array(cases)),
                ])
                .expect("type object"),
            );
        }};
    }
    record!(iroha_data_model::events::data::escrow::EscrowEventSet);
    record!(iroha_data_model::events::data::prelude::AccountEventSet);
    record!(iroha_data_model::events::data::prelude::AccountRecoveryEventSet);
    record!(iroha_data_model::events::data::prelude::AssetDefinitionEventSet);
    record!(iroha_data_model::events::data::prelude::AssetEventSet);
    record!(iroha_data_model::events::data::prelude::BridgeEventSet);
    record!(iroha_data_model::events::data::prelude::ConfigurationEventSet);
    record!(iroha_data_model::events::data::prelude::DomainEventSet);
    record!(iroha_data_model::events::data::prelude::ExecutorEventSet);
    record!(iroha_data_model::events::data::prelude::NftEventSet);
    record!(iroha_data_model::events::data::prelude::PeerEventSet);
    record!(iroha_data_model::events::data::prelude::RepoAccountEventSet);
    record!(iroha_data_model::events::data::prelude::RoleEventSet);
    record!(iroha_data_model::events::data::prelude::RwaEventSet);
    record!(iroha_data_model::events::data::prelude::TriggerEventSet);
    record!(iroha_data_model::events::data::governance::GovernanceEventSet);
    record!(iroha_data_model::events::data::musubi::MusubiEventSet);
    record!(iroha_data_model::events::data::oracle::OracleEventSet);
    record!(iroha_data_model::events::data::proof::ProofEventSet);
    record!(iroha_data_model::events::data::runtime_upgrade::RuntimeUpgradeEventSet);
    record!(iroha_data_model::events::data::smart_contract::SmartContractEventSet);
    record!(iroha_data_model::events::data::soradns::SoradnsDirectoryEventSet);
    record!(iroha_data_model::events::data::sorafs::SorafsGatewayEventSet);
    record!(iroha_data_model::events::data::space_directory::SpaceDirectoryEventSet);
    record!(iroha_data_model::events::data::verifying_keys::VerifyingKeyEventSet);
    let captured: Vec<Value> = json::from_str(include_str!("fixtures/event_set_frames.json"))
        .expect("immutable pre-declaration frames");
    assert_eq!(captured.len(), 25);
    assert_eq!(rows, captured);
}

#[test]
fn serialize() {
    assert_eq!(
        norito::json::to_value(&TestEventSet::Event1).unwrap(),
        array(&["Event1"])
    );
    assert_eq!(
        norito::json::to_value(&(TestEventSet::Event1 | TestEventSet::Event2)).unwrap(),
        array(&["Event1", "Event2"])
    );
    assert_eq!(
        norito::json::to_value(&(TestEventSet::Event1 | TestEventSet::AnyNestedEvent)).unwrap(),
        array(&["Event1", "AnyNestedEvent"])
    );
    assert_eq!(
        norito::json::to_value(&TestEventSet::all()).unwrap(),
        array(&["Event1", "Event2", "AnyNestedEvent"])
    );
}
#[test]
fn deserialize() {
    assert_eq!(
        norito::json::from_value::<TestEventSet>(array(&[])).unwrap(),
        TestEventSet::empty()
    );
    assert_eq!(
        norito::json::from_value::<TestEventSet>(array(&["Event1"])).unwrap(),
        TestEventSet::Event1
    );
    assert_eq!(
        norito::json::from_value::<TestEventSet>(array(&["Event1", "Event2"])).unwrap(),
        TestEventSet::Event1 | TestEventSet::Event2
    );
    assert_eq!(
        norito::json::from_value::<TestEventSet>(array(&["Event1", "AnyNestedEvent"])).unwrap(),
        TestEventSet::Event1 | TestEventSet::AnyNestedEvent
    );
    assert_eq!(
        norito::json::from_value::<TestEventSet>(array(&["Event1", "Event2", "AnyNestedEvent"]))
            .unwrap(),
        TestEventSet::all(),
    );
    assert_eq!(
        norito::json::from_value::<TestEventSet>(array(&["Event1", "Event1", "AnyNestedEvent"]))
            .unwrap(),
        TestEventSet::Event1 | TestEventSet::AnyNestedEvent,
    );
}
#[test]
fn deserialize_invalid() {
    assert_eq!(
        norito::json::from_value::<TestEventSet>(json::to_value(&32).unwrap())
            .unwrap_err()
            .to_string(),
        "invalid type: integer `32`, expected a sequence of strings"
    );
    assert_eq!(
        norito::json::from_value::<TestEventSet>(
            json::array([32]).expect("serialize integer array"),
        )
        .unwrap_err()
        .to_string(),
        "invalid type: integer `32`, expected a string"
    );
    assert_eq!(
        norito::json::from_value::<TestEventSet>(array(&["InvalidVariant"]))
            .unwrap_err()
            .to_string(),
        "unknown event variant `InvalidVariant`, expected one of `Event1`, `Event2`, `AnyNestedEvent`"
    );
    assert_eq!(
        norito::json::from_value::<TestEventSet>(array(&["Event1", "Event1", "InvalidVariant"]),)
            .unwrap_err()
            .to_string(),
        "unknown event variant `InvalidVariant`, expected one of `Event1`, `Event2`, `AnyNestedEvent`"
    );
}
#[test]
fn full_set() {
    let any_matcher = TestEventSet::all();
    assert_eq!(
        any_matcher,
        TestEventSet::Event1 | TestEventSet::Event2 | TestEventSet::AnyNestedEvent
    );
    assert_eq!(
        format!("{any_matcher:?}"),
        "TestEventSet[Event1, Event2, AnyNestedEvent]"
    );
    assert!(any_matcher.matches(&TestEvent::Event1));
    assert!(any_matcher.matches(&TestEvent::Event2));
    assert!(any_matcher.matches(&TestEvent::NestedEvent(AnotherEvent)));
}
#[test]
fn empty_set() {
    let none_matcher = TestEventSet::empty();
    assert_eq!(format!("{none_matcher:?}"), "TestEventSet[]");
    assert!(!none_matcher.matches(&TestEvent::Event1));
    assert!(!none_matcher.matches(&TestEvent::Event2));
    assert!(!none_matcher.matches(&TestEvent::NestedEvent(AnotherEvent)));
}
#[test]
fn event1_set() {
    let event1_matcher = TestEventSet::Event1;
    assert_eq!(format!("{event1_matcher:?}"), "TestEventSet[Event1]");
    assert!(event1_matcher.matches(&TestEvent::Event1));
    assert!(!event1_matcher.matches(&TestEvent::Event2));
    assert!(!event1_matcher.matches(&TestEvent::NestedEvent(AnotherEvent)));
}
#[test]
fn event1_or_2_set() {
    let event1_or_2_matcher = TestEventSet::Event1 | TestEventSet::Event2;
    assert_eq!(
        format!("{event1_or_2_matcher:?}"),
        "TestEventSet[Event1, Event2]"
    );
    assert!(event1_or_2_matcher.matches(&TestEvent::Event1));
    assert!(event1_or_2_matcher.matches(&TestEvent::Event2));
    assert!(!event1_or_2_matcher.matches(&TestEvent::NestedEvent(AnotherEvent)));
}
#[test]
fn repeated() {
    assert_eq!(
        TestEventSet::Event1 | TestEventSet::Event1,
        TestEventSet::Event1
    );
    assert_eq!(
        TestEventSet::Event1 | TestEventSet::Event2 | TestEventSet::Event1,
        TestEventSet::Event1 | TestEventSet::Event2
    );
    assert_eq!(
        TestEventSet::all() | TestEventSet::AnyNestedEvent,
        TestEventSet::all()
    );
}
