//! Exact spelling and bounded output for foundational persisted-map keys.

use crate::{
    domain::DomainId,
    name::Name,
    state_path::StatePath,
    topology::{DataSpaceId, LaneId},
};
use norito::json::{self, BoundedJsonError, JsonKeyCodec, JsonSerialize, JsonWriteSink};

struct KeyJson<'a, T>(&'a T);
impl<T: JsonKeyCodec> JsonSerialize for KeyJson<'_, T> {
    fn json_serialize(&self, output: &mut String) {
        self.0.encode_json_key(output);
    }
    fn json_serialize_to(&self, output: &mut dyn JsonWriteSink) -> Result<(), BoundedJsonError> {
        self.0.encode_json_key_to(output)
    }
}

fn parity<T: JsonKeyCodec + PartialEq + core::fmt::Debug>(key: T) {
    let value = KeyJson(&key);
    let expected = json::to_json(&value).unwrap();
    assert_eq!(
        json::to_json_bounded(&value, expected.len()),
        Ok(expected.clone())
    );
    for cap in 0..expected.len() {
        assert_eq!(
            json::to_json_bounded(&value, cap),
            Err(BoundedJsonError::BodyTooLarge)
        );
    }
    let text: String = json::from_str(&expected).unwrap();
    assert_eq!(T::decode_json_key(&text).unwrap(), key);
}

#[test]
fn bounded_json_keys_preserve_escaped_names_paths_and_domain_spelling() {
    for text in [
        "a",
        "café",
        "quote\"\\雪",
        "long-name-that-requires-owned-storage",
    ] {
        parity(text.parse::<Name>().unwrap());
        parity(text.parse::<StatePath>().unwrap());
    }
    parity(DomainId::try_new("vault", "universal").unwrap());
    parity(DomainId::try_new("café", "雪").unwrap());
}

#[test]
fn bounded_json_keys_preserve_full_topology_integer_domains() {
    for raw in [0, 1, u64::from(u32::MAX) + 1, u64::MAX] {
        parity(DataSpaceId::new(raw));
    }
    for raw in [0, 1, u32::MAX] {
        parity(LaneId::new(raw));
    }
}
