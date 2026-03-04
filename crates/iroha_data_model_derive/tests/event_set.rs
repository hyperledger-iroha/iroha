//! Tests covering the `EventSet` derive macro behaviour.

#![allow(unexpected_cfgs)]
mod events {
    use iroha_data_model_derive::EventSet;
    /// Test event enumeration used with the `EventSet` derive.
    #[derive(EventSet)]
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

fn array(strings: &[&str]) -> Value {
    json::array(strings.iter().copied()).expect("serialize string array")
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
