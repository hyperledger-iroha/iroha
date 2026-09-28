//! Codec assertions shared by the SCCP data-model tests.

use core::fmt::Debug;

use norito::{
    codec::{DecodeAll, Encode},
    json::{self, JsonDeserialize, JsonSerialize, Value},
};

/// Assert that `value` survives the bare Norito codec and JSON unchanged.
pub fn roundtrip<T>(value: &T)
where
    T: Encode + DecodeAll + JsonSerialize + JsonDeserialize + PartialEq + Debug,
{
    let encoded = value.encode();
    let decoded = T::decode_all(&mut encoded.as_slice()).expect("decode the bare Norito encoding");
    assert_eq!(&decoded, value, "bare Norito roundtrip");
    let text = json::to_json(value).expect("serialize JSON");
    let parsed: T = json::from_json(&text).expect("deserialize JSON");
    assert_eq!(&parsed, value, "JSON roundtrip");
}

/// Assert that the JSON decoder of `T` rejects an unknown field added to the object reached from
/// the JSON form of `value` by `path` (object keys, or decimal indices into arrays; `&[]` is the
/// top-level object), while the unmodified JSON still decodes.
pub fn assert_rejects_unknown_field<T>(value: &T, path: &[&str])
where
    T: JsonSerialize + JsonDeserialize + PartialEq + Debug,
{
    let mut root = json::to_value(value).expect("serialize JSON value");
    let mut cursor = &mut root;
    for segment in path {
        cursor = match cursor {
            Value::Object(object) => object
                .get_mut(*segment)
                .unwrap_or_else(|| panic!("path segment `{segment}` is not a field")),
            Value::Array(items) => {
                let index: usize = segment
                    .parse()
                    .unwrap_or_else(|_| panic!("path segment `{segment}` is not an array index"));
                items
                    .get_mut(index)
                    .unwrap_or_else(|| panic!("array index {index} is out of bounds"))
            }
            other => panic!("path segment `{segment}` cannot descend into {other:?}"),
        };
    }
    let Value::Object(object) = cursor else {
        panic!("the path {path:?} does not reach a JSON object");
    };
    object.insert("unexpected_field".to_owned(), Value::Bool(true));
    let hostile = json::to_json(&root).expect("serialize hostile JSON");
    assert!(
        json::from_json::<T>(&hostile).is_err(),
        "an unknown field at {path:?} must be rejected"
    );
    let honest = json::to_json(value).expect("serialize JSON");
    assert_eq!(
        &json::from_json::<T>(&honest).expect("the unmodified JSON decodes"),
        value
    );
}
