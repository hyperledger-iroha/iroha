//! Public-API probes of inherited checked JSON container cleanup.

use norito::json::{self, BoundedJsonError, JsonSerialize, JsonWriteSink};
use std::collections::{BTreeMap, BTreeSet, HashSet};

const INHERITED_DEPTH: usize = 7;
const ASSERTION: &str = "checked Norito containers must restore the original inherited sink depth";

struct TrackingSink {
    output: String,
    limit: usize,
    depth: usize,
    depth_limit: Option<usize>,
}
impl TrackingSink {
    fn new(limit: usize) -> Self {
        Self {
            output: String::new(),
            limit,
            depth: INHERITED_DEPTH,
            depth_limit: None,
        }
    }
}
impl JsonWriteSink for TrackingSink {
    fn push(&mut self, value: char) -> Result<(), BoundedJsonError> {
        self.push_str(value.encode_utf8(&mut [0; 4]))
    }
    fn push_str(&mut self, value: &str) -> Result<(), BoundedJsonError> {
        if self
            .output
            .len()
            .checked_add(value.len())
            .is_none_or(|next| next > self.limit)
        {
            return Err(BoundedJsonError::BodyTooLarge);
        }
        self.output.push_str(value);
        Ok(())
    }
    fn begin_container(&mut self) -> Result<(), BoundedJsonError> {
        let next = self
            .depth
            .checked_add(1)
            .ok_or(BoundedJsonError::Unsupported)?;
        if self.depth_limit.is_some_and(|limit| next >= limit) {
            return Err(BoundedJsonError::Unsupported);
        }
        self.depth = next;
        Ok(())
    }
    fn end_container(&mut self) {
        assert!(
            self.depth > INHERITED_DEPTH,
            "writer must not release inherited levels"
        );
        self.depth -= 1;
    }
}

fn audit_all_byte_refusals(value: &impl JsonSerialize, label: &str, defects: &mut Vec<String>) {
    let ordinary = json::to_json(value).expect("ordinary original writer");
    let mut changed_depths = BTreeSet::new();
    let mut refusal_count = 0;
    for limit in 0..ordinary.len() {
        let mut sink = TrackingSink::new(limit);
        assert_eq!(
            value.json_serialize_to(&mut sink),
            Err(BoundedJsonError::BodyTooLarge)
        );
        refusal_count += 1;
        if sink.depth != INHERITED_DEPTH {
            changed_depths.insert(sink.depth);
        }
    }
    println!(
        "shape={label}; actual byte refusals={refusal_count}; changed depths={changed_depths:?}"
    );
    if !changed_depths.is_empty() {
        defects.push(format!("{label}: {changed_depths:?}"));
    }
}
fn assert_no_defects(defects: Vec<String>) {
    assert!(defects.is_empty(), "{ASSERTION}: {defects:?}");
}

#[test]
fn original_vec_refusal_preserves_inherited_depth() {
    let mut defects = Vec::new();
    audit_all_byte_refusals(&vec![vec![17_u64], vec![]], "nested Vec", &mut defects);
    assert_no_defects(defects);
}
#[test]
fn original_duration_refusal_preserves_inherited_depth() {
    let mut defects = Vec::new();
    audit_all_byte_refusals(&std::time::Duration::new(17, 42), "Duration", &mut defects);
    assert_no_defects(defects);
}
#[test]
fn original_btree_set_refusal_preserves_inherited_depth() {
    let mut defects = Vec::new();
    audit_all_byte_refusals(&BTreeSet::from([17_u64, 42]), "BTreeSet", &mut defects);
    assert_no_defects(defects);
}
#[test]
fn original_btree_map_refusal_preserves_inherited_depth() {
    let mut defects = Vec::new();
    audit_all_byte_refusals(
        &BTreeMap::from([(String::from("key"), vec![17_u64])]),
        "BTreeMap<Vec>",
        &mut defects,
    );
    assert_no_defects(defects);
}
#[test]
fn original_nested_value_refusal_preserves_inherited_depth() {
    let value: json::Value = json::from_str(r#"{"outer":[{"inner":[17]},null]}"#).unwrap();
    let mut defects = Vec::new();
    audit_all_byte_refusals(&value, "nested Value", &mut defects);
    assert_no_defects(defects);
}

#[derive(norito::derive::JsonSerialize)]
struct OriginalRecord {
    label: String,
    values: Vec<u64>,
}
#[derive(norito::derive::JsonSerialize)]
struct OriginalTuple(String, Vec<u64>);
#[derive(norito::derive::JsonSerialize)]
#[norito(tag = "kind", content = "body")]
enum OriginalTagged {
    Unit,
    Empty(),
    One(Vec<u64>),
    Pair(String, Vec<u64>),
    Named { label: String, values: Vec<u64> },
}
#[test]
fn original_derived_struct_refusal_preserves_inherited_depth() {
    let mut defects = Vec::new();
    audit_all_byte_refusals(
        &OriginalRecord {
            label: "retained".into(),
            values: vec![17],
        },
        "derived named struct",
        &mut defects,
    );
    assert_no_defects(defects);
}
#[test]
fn original_derived_tuple_refusal_preserves_inherited_depth() {
    let mut defects = Vec::new();
    audit_all_byte_refusals(
        &OriginalTuple("retained".into(), vec![17]),
        "derived tuple struct",
        &mut defects,
    );
    assert_no_defects(defects);
}
#[test]
fn original_derived_tagged_enum_refusal_preserves_inherited_depth() {
    let mut defects = Vec::new();
    for (label, value) in [
        ("unit variant", OriginalTagged::Unit),
        ("empty tuple variant", OriginalTagged::Empty()),
        ("one-field variant", OriginalTagged::One(vec![17])),
        (
            "multi-field variant",
            OriginalTagged::Pair("retained".into(), vec![17]),
        ),
        (
            "named variant",
            OriginalTagged::Named {
                label: "retained".into(),
                values: vec![17],
            },
        ),
    ] {
        audit_all_byte_refusals(&value, label, &mut defects);
    }
    assert_no_defects(defects);
}
#[test]
fn original_hash_set_refusal_preserves_inherited_depth() {
    let mut defects = Vec::new();
    audit_all_byte_refusals(&HashSet::from([17_u64, 42]), "HashSet", &mut defects);
    assert_no_defects(defects);
}
struct Validated<'a>(&'a str);
impl JsonSerialize for Validated<'_> {
    fn json_serialize(&self, out: &mut String) {
        out.push_str(self.0);
    }
    fn json_serialize_to(&self, out: &mut dyn JsonWriteSink) -> Result<(), BoundedJsonError> {
        json::write_validated_json_to(self.0, out)
    }
}
#[test]
fn original_validated_document_refusal_preserves_inherited_depth() {
    let mut defects = Vec::new();
    audit_all_byte_refusals(
        &Validated(r#"{"outer":[{"inner":[17]},null]}"#),
        "validated document",
        &mut defects,
    );
    assert_no_defects(defects);
}
struct UnsupportedLeaf;
impl JsonSerialize for UnsupportedLeaf {
    fn json_serialize(&self, _: &mut String) {
        panic!("checked write must not use ordinary fallback");
    }
}
#[test]
fn original_nested_unsupported_leaf_preserves_exact_refusal_and_depth() {
    let mut sink = TrackingSink::new(usize::MAX);
    assert_eq!(
        vec![UnsupportedLeaf].json_serialize_to(&mut sink),
        Err(BoundedJsonError::Unsupported)
    );
    assert_eq!(
        sink.depth, INHERITED_DEPTH,
        "{ASSERTION}: original leaf refusal"
    );
}
#[test]
fn original_success_and_begin_refusal_keep_bytes_and_inherited_depth() {
    let value = OriginalRecord {
        label: "retained".into(),
        values: vec![17],
    };
    let ordinary = json::to_json(&value).unwrap();
    let mut sink = TrackingSink::new(ordinary.len());
    assert_eq!(value.json_serialize_to(&mut sink), Ok(()));
    assert_eq!(sink.output, ordinary);
    assert_eq!(sink.depth, INHERITED_DEPTH);
    let mut sink = TrackingSink::new(usize::MAX);
    sink.depth_limit = Some(INHERITED_DEPTH + 1);
    assert_eq!(
        value.json_serialize_to(&mut sink),
        Err(BoundedJsonError::Unsupported)
    );
    assert!(sink.output.is_empty());
    assert_eq!(sink.depth, INHERITED_DEPTH);
}

#[test]
fn original_nested_begin_refusal_balances_prior_entered_levels() {
    let value = vec![vec![17_u64]];
    let mut sink = TrackingSink::new(usize::MAX);
    sink.depth_limit = Some(INHERITED_DEPTH + 2);
    assert_eq!(
        value.json_serialize_to(&mut sink),
        Err(BoundedJsonError::Unsupported)
    );
    assert_eq!(
        sink.depth, INHERITED_DEPTH,
        "{ASSERTION}: nested begin refusal"
    );
}
