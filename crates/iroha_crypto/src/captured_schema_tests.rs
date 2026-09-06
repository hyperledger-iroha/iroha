//! Immutable compiler-captured identity checks shared by private codec owners.

use std::sync::OnceLock;

use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize, json::Value};

fn captured(nominal: &str) -> &'static Value {
    static FIXTURE: OnceLock<Vec<Value>> = OnceLock::new();
    let rows = FIXTURE.get_or_init(|| {
        let rows: Vec<Value> = norito::json::from_str(include_str!(
            "../tests/fixtures/captured_codec_schema_identities.json"
        ))
        .expect("immutable compiler capture fixture");
        let names: std::collections::BTreeSet<_> = rows
            .iter()
            .map(|row| row.get("nominal").and_then(Value::as_str).expect("nominal"))
            .collect();
        assert_eq!(rows.len(), 108);
        assert_eq!(names.len(), rows.len(), "capture names must be unique");
        rows
    });
    rows.iter()
        .find(|row| row.get("nominal").and_then(Value::as_str) == Some(nominal))
        .expect("every checked type has immutable capture evidence")
}

fn expected_hash(row: &Value, direction: &str) -> [u8; 16] {
    let hash = row
        .get(direction)
        .and_then(Value::as_str)
        .expect("captured codec direction");
    let bytes = hex::decode(hash).expect("captured hash is hexadecimal");
    bytes.try_into().expect("schema hash is exactly 16 bytes")
}

/// Check an existing serializer against its fixed nominal, root and hash.
pub(crate) fn assert_serialize<T: NoritoSchema + NoritoSerialize>(nominal: &str) {
    let row = captured(nominal);
    assert_eq!(T::nominal_name(), nominal);
    assert_eq!(
        T::frame_name(),
        row.get("root")
            .and_then(Value::as_str)
            .expect("captured root")
    );
    let hash = expected_hash(row, "serialize_hash");
    assert_eq!(norito::schema::identity::frame_hash::<T>(), hash);
    assert_eq!(<T as NoritoSerialize>::schema_hash(), hash);
}

/// Also check the independently generated decoder without constructing a value.
pub(crate) fn assert_bidirectional<T>(nominal: &str)
where
    T: NoritoSchema + NoritoSerialize + for<'a> NoritoDeserialize<'a>,
{
    assert_serialize::<T>(nominal);
    assert_eq!(
        <T as NoritoDeserialize>::schema_hash(),
        expected_hash(captured(nominal), "deserialize_hash")
    );
}
