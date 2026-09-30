//! Bounded native identity output from existing owner-private typed inventories.
//!
//! This module is test-only. Explicit ignored maintenance tests print public
//! identities; ordinary captured-fixture assertions never call this path.

use super::{Case, NoritoDeserialize, NoritoSchema, NoritoSerialize};
use norito::json::{self, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, io::Write};

const MAX_OWNER_ROWS: usize = 4_096;
const MAX_IDENTITY_BYTES: usize = 4_096;
const MAX_ROW_BYTES: usize = 32 * 1_024;
const MAX_OWNER_BYTES: usize = 8 * 1_024 * 1_024;

/// Current declared identity and only the codec directions supported by its owner.
pub(super) struct Identity {
    nominal: String,
    root: String,
    hash: [u8; 16],
    serialize: bool,
    deserialize: bool,
}

fn identity<T: NoritoSchema>(nominal: &str, serialize: bool, deserialize: bool) -> Identity {
    let actual = T::nominal_name();
    let root = T::frame_name();
    assert_eq!(actual, nominal, "current typed nominal identity");
    assert!(!actual.is_empty() && actual.len() <= MAX_IDENTITY_BYTES);
    assert!(!root.is_empty() && root.len() <= MAX_IDENTITY_BYTES);
    Identity {
        nominal: actual,
        root,
        hash: norito::schema::identity::frame_hash::<T>(),
        serialize,
        deserialize,
    }
}

pub(super) fn serialize<T: NoritoSchema + NoritoSerialize>(nominal: &str) -> Identity {
    identity::<T>(nominal, true, false)
}

pub(super) fn deserialize<T>(nominal: &str) -> Identity
where
    T: NoritoSchema + for<'a> NoritoDeserialize<'a>,
{
    identity::<T>(nominal, false, true)
}

pub(super) fn bidirectional<T>(nominal: &str) -> Identity
where
    T: NoritoSchema + NoritoSerialize + for<'a> NoritoDeserialize<'a>,
{
    identity::<T>(nominal, true, true)
}

impl Identity {
    fn row(self) -> Value {
        let mut fields = vec![
            ("nominal", Value::String(self.nominal)),
            ("root", Value::String(self.root)),
        ];
        if self.serialize {
            fields.push(("serialize_hash", Value::String(hex::encode(self.hash))));
        }
        if self.deserialize {
            fields.push(("deserialize_hash", Value::String(hex::encode(self.hash))));
        }
        json::object(fields).expect("native identity object")
    }
}

fn owner_document(cases: &[Case], owner: &str) -> Vec<u8> {
    assert!(!owner.is_empty() && owner.len() <= MAX_IDENTITY_BYTES);
    assert!(!cases.is_empty() && cases.len() <= MAX_OWNER_ROWS);
    let mut names = BTreeSet::new();
    let mut rows = Vec::with_capacity(cases.len());
    let mut row_bytes = 0_usize;
    for case in cases {
        assert!(
            names.insert(case.nominal),
            "duplicate typed capture identity"
        );
        let row = (case.capture)(case.nominal).row();
        let encoded = json::to_json(&row).expect("serialize native identity");
        assert!(encoded.len() <= MAX_ROW_BYTES);
        row_bytes = row_bytes.checked_add(encoded.len()).expect("capture size");
        assert!(row_bytes <= MAX_OWNER_BYTES);
        rows.push(row);
    }
    let body = json::object([
        ("schema", Value::from(1_u64)),
        ("owner", Value::String(owner.to_owned())),
        ("rows", Value::Array(rows)),
    ])
    .expect("native owner document");
    let encoded = json::to_json(&body).expect("serialize native owner");
    assert!(encoded.len() <= MAX_OWNER_BYTES);
    encoded.into_bytes()
}

/// Emit one bounded owner document with a digest of its exact UTF-8 JSON bytes.
///
/// A single stdout lock prevents parallel test output from interleaving inside
/// the record. Collection must still require every expected compiled owner.
pub fn print_owner(cases: &[Case], owner: &str) {
    let document = owner_document(cases, owner);
    let digest = hex::encode(Sha256::digest(&document));
    let mut output = std::io::stdout().lock();
    write!(output, "NATIVE_CODEC_OWNER_V1\t{digest}\t").expect("write capture prefix");
    output
        .write_all(&document)
        .expect("write native owner capture");
    writeln!(output).expect("finish native owner capture");
}

macro_rules! owner_printer {
    ($cases:expr) => {
        #[test]
        #[ignore = "explicit maintenance capture of current typed codec identities"]
        fn print_native_codec_capture_v1() {
            $crate::captured_schema_tests::native_capture::print_owner($cases, module_path!());
        }
    };
}
pub(crate) use owner_printer;

#[test]
fn current_capture_keeps_direction_bounds_and_exact_identity() {
    let nominal = <u64 as NoritoSchema>::nominal_name();
    let identity = serialize::<u64>(&nominal).row();
    assert_eq!(
        identity.get("nominal").and_then(Value::as_str),
        Some(nominal.as_str())
    );
    assert_eq!(
        identity.get("root").and_then(Value::as_str),
        Some(<u64 as NoritoSchema>::frame_name().as_str())
    );
    assert_eq!(
        identity.get("serialize_hash").and_then(Value::as_str),
        Some(hex::encode(norito::schema::identity::frame_hash::<u64>()).as_str())
    );
    assert!(identity.get("deserialize_hash").is_none());
    let decoder = deserialize::<u64>(&nominal).row();
    assert!(decoder.get("serialize_hash").is_none());
    assert!(decoder.get("deserialize_hash").is_some());
    let both = bidirectional::<u64>(&nominal).row();
    assert_eq!(both.get("serialize_hash"), both.get("deserialize_hash"));
}

#[test]
fn owner_capture_rejects_duplicates_wrong_identity_and_output_bounds() {
    fn callback(nominal: &str) -> Identity {
        identity::<u64>(nominal, true, true)
    }
    let nominal = <u64 as NoritoSchema>::nominal_name();
    assert_eq!(nominal, "u64");
    let case = || Case::bidirectional::<u64>("u64");
    let bytes = owner_document(&[case()], "native_test_owner");
    let parsed: Value = json::from_slice(&bytes).expect("capture JSON");
    assert_eq!(
        parsed.get("rows").and_then(Value::as_array).unwrap().len(),
        1
    );
    assert!(std::panic::catch_unwind(|| owner_document(&[case(), case()], "owner")).is_err());
    assert!(std::panic::catch_unwind(|| owner_document(&[], "owner")).is_err());
    assert!(
        std::panic::catch_unwind(|| owner_document(&[case()], &"x".repeat(MAX_IDENTITY_BYTES + 1)))
            .is_err()
    );
    assert!(std::panic::catch_unwind(|| callback("wrong-name")).is_err());
}
