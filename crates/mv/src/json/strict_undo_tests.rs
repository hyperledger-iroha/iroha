//! Strict first-release present-undo framing rejects every retired alternative.

use super::*;

#[test]
fn explicit_undo_borrows_manual_serializers_without_a_typed_writer_or_clone() {
    struct ManualValue(u64);
    impl JsonSerialize for ManualValue {
        fn json_serialize(&self, out: &mut String) {
            self.0.json_serialize(out);
        }
    }

    let original = Some(ManualValue(7));
    let pointer = original.as_ref().unwrap() as *const ManualValue;
    let mut encoded = String::new();
    json_serialize_undo(&original, &mut encoded);
    assert_eq!(encoded, r#"{"value":7}"#);
    assert_eq!(original.as_ref().unwrap() as *const _, pointer);

    encoded.clear();
    let mut first = true;
    write_storage_undo_entry(
        &String::from("key"),
        original.as_ref(),
        &mut first,
        &mut encoded,
    );
    assert_eq!(encoded, r#""key":{"value":7}"#);
    assert!(!first);
    assert_eq!(original.as_ref().unwrap() as *const _, pointer);

    encoded.clear();
    SnapshotUndoValue {
        value: ManualValue(7),
    }
    .json_serialize(&mut encoded);
    assert_eq!(encoded, r#"{"value":7}"#);
}

#[test]
fn explicit_present_undo_requires_exactly_one_value_and_never_accepts_retired_values() {
    for undo in [
        "7",
        "true",
        "[]",
        "\"old\"",
        "{}",
        r#"{"unknown":null}"#,
        r#"{"value":null,"value":null}"#,
        r#"{"value":null,"extra":0}"#,
    ] {
        let cell = format!("{{\"revert\":{undo},\"blocks\":17}}");
        assert!(
            json::from_json::<Cell<Option<u64>>>(&cell).is_err(),
            "accepted {cell}"
        );
        let storage = format!("{{\"revert\":{{\"k\":{undo}}},\"blocks\":{{\"k\":17}}}}");
        assert!(
            json::from_json::<Storage<String, Option<u64>>>(&storage).is_err(),
            "accepted {storage}"
        );
    }
    let present: Cell<Option<u64>> =
        json::from_json(r#"{"revert":{"value":null},"blocks":17}"#).unwrap();
    let absent: Cell<Option<u64>> = json::from_json(r#"{"revert":null,"blocks":17}"#).unwrap();
    assert_eq!(present.predecessor_view().get(), &Some(None));
    assert_eq!(absent.predecessor_view().get(), &None);
    let map: Storage<String, Option<u64>> =
        json::from_json(r#"{"revert":{"present":{"value":null},"absent":null},"blocks":{}}"#)
            .unwrap();
    assert_eq!(
        map.snapshot().revert_map().get("present"),
        Some(&Some(None))
    );
    assert_eq!(map.snapshot().revert_map().get("absent"), Some(&None));
    assert_eq!(map.snapshot().revert_map().get("untouched"), None);
}

#[test]
fn payload_objects_named_value_are_not_flattened_into_the_undo_wrapper() {
    let cell = Cell::new(BTreeMap::from([(String::from("value"), None::<u64>)]));
    let mut block = cell.block();
    block.get_mut().insert(String::from("other"), Some(7));
    block.commit();
    let encoded = json::to_json(&cell).unwrap();
    assert!(encoded.starts_with(r#"{"revert":{"value":{"value":null}},"blocks":"#));
    let restored: Cell<BTreeMap<String, Option<u64>>> = json::from_json(&encoded).unwrap();
    assert_eq!(
        restored.predecessor_view().get(),
        cell.predecessor_view().get()
    );
}
