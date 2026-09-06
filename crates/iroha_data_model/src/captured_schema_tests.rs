//! Immutable compiler-captured identity checks shared by private model codec owners.
//!
//! These checks preserve named identities and existing directional schema hashes.
//! They do not claim payload equivalence, generic identity closure or release qualification.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize, json::Value};
use sha2::{Digest, Sha256};

const CAPTURE_REPORT_SHA256: &str =
    "be82d3661d9e2a79fd1a60d6922f1387d3821a0ad5a65d80d294aca1251caedd";
const FIXTURE_SHA256: &str = "c3f906db5a33eb93734473891950a5c91ce1dee103448a2181345d5fd4bdbd62";

fn fixture() -> &'static BTreeMap<String, Value> {
    static FIXTURE: OnceLock<BTreeMap<String, Value>> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let source = include_str!("../tests/fixtures/captured_codec_schema_identities.json");
        assert_eq!(
            hex::encode(Sha256::digest(source.as_bytes())),
            FIXTURE_SHA256,
            "the reviewed compiler capture fixture is immutable"
        );
        let document: Value = norito::json::from_str(source).expect("immutable capture fixture");
        assert_eq!(document.get("schema").and_then(Value::as_u64), Some(1));
        assert_eq!(
            document
                .get("capture_report_sha256")
                .and_then(Value::as_str),
            Some(CAPTURE_REPORT_SHA256)
        );
        let rows = document
            .get("rows")
            .and_then(Value::as_array)
            .expect("captured rows");
        assert_eq!(rows.len(), 1_771);
        let mut names = BTreeMap::new();
        let mut direction_counts = [0, 0, 0];
        for row in rows {
            let nominal = row
                .get("nominal")
                .and_then(Value::as_str)
                .expect("captured nominal");
            let root = row
                .get("root")
                .and_then(Value::as_str)
                .expect("captured root");
            assert!(!nominal.is_empty() && !root.is_empty());
            let serialize = row.get("serialize_hash").is_some();
            let deserialize = row.get("deserialize_hash").is_some();
            match (serialize, deserialize) {
                (true, true) => direction_counts[0] += 1,
                (true, false) => direction_counts[1] += 1,
                (false, true) => direction_counts[2] += 1,
                (false, false) => panic!("a fixture row must preserve an existing codec"),
            }
            let mut expected_keys = BTreeSet::from(["nominal", "root"]);
            for (present, direction) in [
                (serialize, "serialize_hash"),
                (deserialize, "deserialize_hash"),
            ] {
                if present {
                    expected_keys.insert(direction);
                    let _ = expected_hash(row, direction);
                }
            }
            assert_eq!(
                row.as_object()
                    .expect("capture row")
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                expected_keys
            );
            assert!(
                names.insert(nominal.to_owned(), row.clone()).is_none(),
                "capture names must be unique"
            );
        }
        assert_eq!(direction_counts, [1_696, 68, 7]);
        names
    })
}

fn captured(nominal: &str) -> &'static Value {
    fixture()
        .get(nominal)
        .expect("every checked type has immutable capture evidence")
}

fn expected_hash(row: &Value, direction: &str) -> [u8; 16] {
    let hash = row
        .get(direction)
        .and_then(Value::as_str)
        .expect("captured codec direction");
    assert_eq!(hash.len(), 32, "schema hashes have exactly 16 bytes");
    assert!(
        hash.bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "schema hashes use lowercase hexadecimal"
    );
    hex::decode(hash)
        .expect("captured hash is hexadecimal")
        .try_into()
        .expect("schema hash is exactly 16 bytes")
}

fn assert_identity<T: NoritoSchema>(nominal: &str) -> &'static Value {
    let row = captured(nominal);
    assert_eq!(T::nominal_name(), nominal);
    assert_eq!(
        T::frame_name(),
        row.get("root")
            .and_then(Value::as_str)
            .expect("captured root")
    );
    row
}

/// Check an existing serializer against its fixed nominal, root and hash.
pub(crate) fn assert_serialize<T: NoritoSchema + NoritoSerialize>(nominal: &str) {
    let row = assert_identity::<T>(nominal);
    let hash = expected_hash(row, "serialize_hash");
    assert_eq!(norito::schema::identity::frame_hash::<T>(), hash);
    assert_eq!(<T as NoritoSerialize>::schema_hash(), hash);
}

/// Check an existing decoder without requiring a serializer or constructing a value.
pub(crate) fn assert_deserialize<T>(nominal: &str)
where
    T: NoritoSchema + for<'a> NoritoDeserialize<'a>,
{
    let row = assert_identity::<T>(nominal);
    let hash = expected_hash(row, "deserialize_hash");
    assert_eq!(norito::schema::identity::frame_hash::<T>(), hash);
    assert_eq!(<T as NoritoDeserialize>::schema_hash(), hash);
}

/// Check both independently generated codec directions without adding either codec.
pub(crate) fn assert_bidirectional<T>(nominal: &str)
where
    T: NoritoSchema + NoritoSerialize + for<'a> NoritoDeserialize<'a>,
{
    assert_serialize::<T>(nominal);
    assert_deserialize::<T>(nominal);
}

#[test]
fn captured_codec_fixture_is_complete() {
    assert_eq!(fixture().len(), 1_771);
}
