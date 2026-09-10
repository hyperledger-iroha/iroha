//! The persisted trigger set owns its frame; its nested DTOs remain payload-only.

use super::*;
use norito::{NoritoSchema, json::Value};

#[test]
fn captured_trigger_set_identity_and_persistence_frame() {
    let records: Vec<Value> = norito::json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/core/trigger_set_frame_identity_observations.v1.json"
    )))
    .unwrap();
    assert_eq!(records.len(), 2);
    let mut directions = std::collections::BTreeSet::new();
    for record in records {
        let text = |key| record.get(key).and_then(Value::as_str).unwrap();
        assert!(directions.insert(text("direction").to_owned()));
        assert_eq!(SetDto::nominal_name(), text("nominal"));
        assert_eq!(SetDto::frame_name(), text("root_hint"));
        let hash: Vec<u8> = text("schema_hash")
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(
            norito::schema::identity::frame_hash::<SetDto>().as_slice(),
            hash
        );
    }
    assert_eq!(
        directions,
        ["serialize".to_owned(), "deserialize".to_owned()]
            .into_iter()
            .collect()
    );
    let dto = SetDto::from(&Set::default());
    let bytes = dto.encode().expect("persist trigger set");
    let view = norito::core::from_bytes_view(&bytes).expect("header and checksum");
    assert_eq!(
        view.schema(),
        norito::schema::identity::frame_hash::<SetDto>()
    );
    let restored = SetDto::decode(&bytes).expect("restore trigger set DTO");
    assert_eq!(restored.encode().unwrap(), bytes);
    Set::try_from(restored).expect("restore trigger stores and indexes");
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(SetDto::decode(&trailing).is_err());
    let mut wrong_root = bytes.clone();
    wrong_root[6] ^= 1;
    assert!(SetDto::decode(&wrong_root).is_err());
    assert!(SetDto::decode(&bytes[..bytes.len() - 1]).is_err());
}
