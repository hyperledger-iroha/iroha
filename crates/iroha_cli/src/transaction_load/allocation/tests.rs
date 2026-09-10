//! Exact public receipt controls, independent JSON expectations and bounded real files.
use super::*;

pub(in crate::transaction_load) fn expected() -> Expected {
    Expected {
        digest: "a".repeat(64),
        pair: 1,
        variant: "one_lane",
        interval_ns: NS,
        measurement_ns: 20 * NS,
        drain_ns: NS,
    }
}
pub(in crate::transaction_load) fn value(journal: usize, trace: usize) -> Value {
    norito::json!({"schema": (resource::RESPONSE_SCHEMA), "kind": "admit", "sequence": 0,
      "outcome": "complete", "admission": {"schema": ADMISSION_SCHEMA,
      "budget_sha256": ("a".repeat(64)), "pair_index": 1, "variant": "one_lane",
      "geometry": {"peers": 4, "interval_ns": NS, "measurement_ns": (20 * NS), "drain_ns": NS},
      "journal": {"label": "pair1.one_lane.journal", "max_bytes": journal},
      "trace": {"label": "pair1.one_lane.trace", "max_bytes": trace}}})
}
pub(in crate::transaction_load) fn line(value: &Value) -> Vec<u8> {
    let mut bytes = json::to_vec(value).unwrap();
    bytes.push(b'\n');
    bytes
}
pub(in crate::transaction_load) fn writers(journal: usize, trace: usize) -> Writers {
    parse(&expected(), &line(&value(journal, trace))).unwrap()
}

#[test]
fn public_receipt_requires_exact_selected_run_geometry_and_distinct_bounded_roles() {
    let admitted = writers(123, 456);
    assert_eq!(admitted.journal.max_bytes, 123);
    assert_eq!(admitted.trace.max_bytes, 456);
    assert_eq!(admitted.journal._label, "pair1.one_lane.journal");
    assert_eq!(admitted.trace._label, "pair1.one_lane.trace");
    for (section, key, changed) in [
        ("", "budget_sha256", norito::json!("b".repeat(64))),
        ("", "pair_index", norito::json!(2)),
        ("", "variant", norito::json!("four_lane")),
        ("geometry", "peers", norito::json!(3)),
        ("geometry", "peers", norito::json!(65)),
        ("geometry", "interval_ns", norito::json!(NS + 1)),
        ("geometry", "measurement_ns", norito::json!(20 * NS + 1)),
        ("geometry", "drain_ns", norito::json!(NS + 1)),
        ("journal", "max_bytes", norito::json!(0)),
        ("trace", "max_bytes", norito::json!(MAX_FILE_BYTES + 1)),
        ("journal", "max_bytes", norito::json!(true)),
        ("journal", "label", norito::json!("../secret")),
        ("trace", "label", norito::json!("pair1.one_lane.journal")),
    ] {
        let mut changed_value = value(123, 456);
        let receipt = changed_value
            .as_object_mut()
            .unwrap()
            .get_mut("admission")
            .unwrap();
        let owner = if section.is_empty() {
            receipt
        } else {
            receipt.as_object_mut().unwrap().get_mut(section).unwrap()
        };
        owner
            .as_object_mut()
            .unwrap()
            .insert(key.to_owned(), changed);
        assert!(
            parse(&expected(), &line(&changed_value)).is_err(),
            "{section}.{key}"
        );
    }
}

#[test]
fn public_receipt_rejects_old_shape_missing_fields_unknown_fields_and_duplicate_keys() {
    let original = value(123, 456);
    for field in ["schema", "kind", "sequence", "outcome", "admission"] {
        let mut malformed = original.clone();
        malformed.as_object_mut().unwrap().remove(field);
        assert!(parse(&expected(), &line(&malformed)).is_err());
    }
    for field in [
        "schema",
        "budget_sha256",
        "pair_index",
        "variant",
        "geometry",
        "journal",
        "trace",
    ] {
        let mut malformed = original.clone();
        malformed
            .as_object_mut()
            .unwrap()
            .get_mut("admission")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(parse(&expected(), &line(&malformed)).is_err());
    }
    let mut old = original.clone();
    let receipt = old.as_object_mut().unwrap().remove("admission").unwrap();
    old.as_object_mut()
        .unwrap()
        .insert("manifest".to_owned(), receipt);
    assert!(parse(&expected(), &line(&old)).is_err());
    let text = String::from_utf8(line(&original)).unwrap();
    for (old, new) in [
        ("\"sequence\":0", "\"sequence\":0,\"sequence\":0"),
        ("\"max_bytes\":123", "\"max_bytes\":123,\"max_bytes\":123"),
        ("\"peers\":4", "\"peers\":4,\"unknown\":0"),
    ] {
        assert!(text.contains(old));
        assert!(parse(&expected(), text.replace(old, new).as_bytes()).is_err());
    }
    for outcome in ["failed", "unavailable"] {
        let mut malformed = original.clone();
        malformed
            .as_object_mut()
            .unwrap()
            .insert("outcome".to_owned(), norito::json!(outcome));
        assert!(parse(&expected(), &line(&malformed)).is_err());
    }
    assert!(!digest_valid(&"A".repeat(64)));
    assert!(!digest_valid(&"a".repeat(63)));
}

#[test]
fn journal_enforces_exact_allocation_including_newline_before_writing_record() {
    let root = tempfile::tempdir().unwrap();
    let event = norito::json!({"event": "allocation_boundary", "content": "exact"});
    let bytes = line(&event);
    for cap in [bytes.len(), bytes.len() - 1] {
        let path = root
            .path()
            .canonicalize()
            .unwrap()
            .join(format!("{cap}.jsonl"));
        let journal = Journal::start(&path, 1, writers(cap, 1).journal).unwrap();
        journal.blocking_record(event.clone()).unwrap();
        let barrier = journal.flush_before_collection();
        let result = journal.finish();
        let actual = std::fs::read(&path).unwrap();
        if cap == bytes.len() {
            assert!(barrier.is_ok());
            assert!(result.is_ok());
            assert_eq!(actual, bytes);
        } else {
            assert!(barrier.is_err());
            assert!(result.is_err());
            assert!(actual.is_empty());
        }
        assert!(actual.len() <= cap);
    }
}

#[test]
fn journal_later_record_overflow_retains_only_complete_first_record() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().canonicalize().unwrap().join("journal");
    let first = norito::json!({"event": "first"});
    let second = norito::json!({"event": "second"});
    let first_bytes = line(&first);
    let second_bytes = line(&second);
    let cap = first_bytes.len() + second_bytes.len() - 1;
    let journal = Journal::start(&path, 1, writers(cap, 1).journal).unwrap();
    journal.blocking_record(first).unwrap();
    journal.flush_before_collection().unwrap();
    journal.blocking_record(second).unwrap();
    assert!(journal.finish().is_err());
    assert_eq!(std::fs::read(path).unwrap(), first_bytes);
}

#[test]
fn allocated_write_checks_overflow_and_limit_before_touching_writer() {
    for (written, cap) in [
        (usize::MAX, MAX_FILE_BYTES),
        (0, 0),
        (0, MAX_FILE_BYTES + 1),
        (2, 2),
    ] {
        let mut count = written;
        let mut bytes = Vec::new();
        assert!(bounded_write(&mut bytes, &mut count, b"x", cap).is_err());
        assert_eq!(count, written);
        assert!(bytes.is_empty());
    }
    let mut count = 0;
    let mut bytes = Vec::new();
    bounded_write(&mut bytes, &mut count, b"ok", 2).unwrap();
    assert_eq!(count, 2);
    assert_eq!(bytes, b"ok");
}

#[test]
fn actual_python_canonical_budget_receipt_decodes_against_independent_rust_sha256() {
    let public_inputs = include_bytes!("fixtures/public-run-budget.json");
    let reply = include_bytes!("fixtures/admission.jsonl");
    assert!(public_inputs.is_ascii());
    assert!(!public_inputs.ends_with(b"\n"));
    let public: Value = json::from_slice(public_inputs).unwrap();
    assert_eq!(public.get("pair_index").and_then(Value::as_u64), Some(1));
    assert_eq!(
        public.get("variant").and_then(Value::as_str),
        Some("one_lane")
    );
    let mut bound = expected();
    bound.digest = format!("{:x}", Sha256::digest(public_inputs));
    let allocated = parse(&bound, reply).unwrap();
    assert_eq!(allocated.journal.max_bytes, 4096);
    assert_eq!(allocated.trace.max_bytes, 8192);
    assert_eq!(allocated.journal._label, "pair1.one_lane.journal");
    assert_eq!(allocated.trace._label, "pair1.one_lane.trace");
    assert!(parse(&expected(), reply).is_err());
}
