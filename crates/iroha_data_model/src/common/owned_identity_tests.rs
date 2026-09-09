//! Complete storage-wrapper identity and frame captures before model relocation.

use std::collections::BTreeMap;

use iroha_crypto::Hash;
use iroha_primitives::numeric::Quantity;
use norito::{
    NoritoDeserialize, NoritoSchema, NoritoSerialize,
    core::{DecodeFlagsGuard, header_flags},
    json::{self, JsonDeserialize, JsonSerialize, Value},
};

use super::Owned;
use crate::{
    account::{AccountAlias, AccountAliasDomain, AccountDetails, OpaqueAccountId},
    metadata::Metadata,
    nexus::{DataSpaceId, UniversalAccountId},
};

#[path = "owned_identity_values.rs"]
mod values;

#[path = "../../tests/support/fixture_json.rs"]
mod fixture_json;

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut result = String::new();
    for byte in bytes {
        write!(result, "{byte:02x}").expect("format captured byte");
    }
    result
}

fn record<T>(value: &T) -> Value
where
    T: NoritoSchema
        + NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + JsonSerialize
        + JsonDeserialize,
{
    let serialize_hash = norito::schema::identity::frame_hash::<T>();
    let deserialize_hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(serialize_hash, deserialize_hash, "directional identity");
    assert_eq!(
        serialize_hash,
        norito::schema::identity::frame_hash::<T>(),
        "the declared projection must match the active codec"
    );
    let (payload, flags) = norito::codec::encode_with_header_flags(value);
    let frame = norito::core::frame_bare_with_header_flags::<T>(&payload, flags)
        .expect("frame captured storage value");
    let decoded: T = norito::decode_from_bytes(&frame).unwrap_or_else(|error| {
        panic!(
            "decode {} storage value with flags {flags:#04x}, payload {}: {error:?}",
            std::any::type_name::<T>(),
            hex(&payload)
        );
    });
    assert!(
        norito::decode_from_bytes::<T>(&frame[..frame.len() - 1]).is_err(),
        "a truncated storage frame must reject"
    );
    assert_eq!(
        norito::to_bytes(&decoded).expect("reencode decoded storage value"),
        frame
    );
    let json = json::to_value(value).unwrap_or_else(|error| {
        panic!(
            "capture {} storage JSON: {error}",
            std::any::type_name::<T>()
        )
    });
    assert_eq!(
        json::to_value(&decoded).expect("decoded storage JSON"),
        json
    );
    let json_decoded: T = json::from_value(json.clone()).expect("decode captured storage JSON");
    assert_eq!(
        norito::to_bytes(&json_decoded).expect("encode JSON-decoded storage value"),
        frame
    );
    norito::json!({
        "nominal": (T::nominal_name()),
        "frame_name": (T::frame_name()),
        "serialize_hash": (hex(&serialize_hash)),
        "deserialize_hash": (hex(&deserialize_hash)),
        "flags": flags,
        "payload_hex": (hex(&payload)),
        "frame_hex": (hex(&frame)),
        "json": json,
    })
}

fn frames<T>(value: &T) -> Value
where
    T: NoritoSchema
        + NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + JsonSerialize
        + JsonDeserialize
        + Clone,
{
    norito::json!({
        "root": (record(value)),
        "vector": (record(&vec![value.clone(), value.clone()])),
        "option": (record(&Some(value.clone()))),
        "map": (record(&BTreeMap::from([
            ("alpha".to_owned(), value.clone()),
            ("omega".to_owned(), value.clone()),
        ]))),
    })
}

fn reject_substituted_frame<T>(record: &Value)
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let encoded = record
        .get("frame_hex")
        .and_then(Value::as_str)
        .expect("complete container frame");
    assert_eq!(encoded.len() % 2, 0);
    let bytes: Vec<_> = (0..encoded.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(&encoded[offset..offset + 2], 16).expect("hex frame byte"))
        .collect();
    assert!(matches!(
        norito::decode_from_bytes::<T>(&bytes),
        Err(norito::core::Error::SchemaMismatch)
    ));
}

fn pair<T>(case: &str, value: T) -> Value
where
    T: NoritoSchema
        + NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + JsonSerialize
        + JsonDeserialize
        + Clone,
{
    let mut layouts = Vec::new();
    for requested in [
        0,
        header_flags::COMPACT_LEN,
        header_flags::PACKED_SEQ,
        header_flags::PACKED_SEQ | header_flags::COMPACT_LEN,
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN,
        header_flags::PACKED_STRUCT
            | header_flags::PACKED_SEQ
            | header_flags::COMPACT_LEN
            | header_flags::FIELD_BITSET,
    ] {
        let _flags = DecodeFlagsGuard::enter(requested);
        let inner = frames(&value);
        let owned = frames(&Owned::new(value.clone()));
        assert_eq!(
            inner.pointer("/root/frame_hex"),
            owned.pointer("/root/frame_hex")
        );
        assert_eq!(inner.pointer("/root/json"), owned.pointer("/root/json"));
        for container in ["vector", "option", "map"] {
            assert_eq!(
                inner.pointer(&format!("/{container}/payload_hex")),
                owned.pointer(&format!("/{container}/payload_hex")),
                "storage ownership must not alter container payloads"
            );
            assert_ne!(
                inner.pointer(&format!("/{container}/frame_hex")),
                owned.pointer(&format!("/{container}/frame_hex")),
                "generic parents retain the nominal storage wrapper"
            );
        }
        reject_substituted_frame::<Vec<T>>(owned.get("vector").unwrap());
        reject_substituted_frame::<Vec<Owned<T>>>(inner.get("vector").unwrap());
        reject_substituted_frame::<Option<T>>(owned.get("option").unwrap());
        reject_substituted_frame::<Option<Owned<T>>>(inner.get("option").unwrap());
        reject_substituted_frame::<BTreeMap<String, T>>(owned.get("map").unwrap());
        reject_substituted_frame::<BTreeMap<String, Owned<T>>>(inner.get("map").unwrap());
        layouts.push(norito::json!({"requested_flags": requested, "inner": inner, "owned": owned}));
    }
    norito::json!({"case": case, "layouts": layouts})
}

fn account_values() -> Vec<AccountDetails> {
    let mut metadata = Metadata::default();
    metadata.insert("identity_revision".parse().expect("metadata name"), 7_u64);
    metadata.insert(
        "registered_label".parse().expect("metadata name"),
        "merchant",
    );
    vec![
        AccountDetails::default(),
        AccountDetails::new(
            metadata,
            Some(AccountAlias::new(
                "merchant".parse().expect("alias label"),
                Some(AccountAliasDomain::new(
                    "payments".parse().expect("alias domain"),
                )),
                DataSpaceId::new(7),
            )),
            Some(UniversalAccountId::from_hash(Hash::new(
                b"owned-account-uaid",
            ))),
            vec![
                OpaqueAccountId::from_hash(Hash::new(b"owned-account-first-opaque-id")),
                OpaqueAccountId::from_hash(Hash::new(b"owned-account-second-opaque-id")),
            ],
        ),
    ]
}

fn render() -> Value {
    let _address = crate::account::address::ChainDiscriminantGuard::enter(42);
    let mut cases = Vec::new();
    for (index, value) in account_values().into_iter().enumerate() {
        cases.push(pair(&format!("account-{index}"), value));
    }
    for (index, value) in [
        Quantity::zero(),
        Quantity::from(7_u64),
        "123.45".parse::<Quantity>().expect("fractional quantity"),
    ]
    .into_iter()
    .enumerate()
    {
        cases.push(pair(&format!("asset-{index}"), value));
    }
    for (index, value) in values::nft_values().into_iter().enumerate() {
        cases.push(pair(&format!("nft-{index}"), value));
    }
    for (index, value) in values::rwa_values().into_iter().enumerate() {
        cases.push(pair(&format!("rwa-{index}"), value));
    }
    cases.push(pair(
        "forwarded-string",
        Box::<str>::from("storage projection"),
    ));
    assert_eq!(cases.len(), 9);
    norito::json!({
        "format": "iroha.model.owned-storage-identity",
        "version": 1_u32,
        "address_discriminant": 42_u32,
        "cases": cases,
    })
}

#[test]
fn owned_storage_identities_preserve_captured_frames() {
    use sha2::{Digest as _, Sha256};

    let source = include_str!("../../tests/fixtures/owned_storage_identity_frames.json");
    assert_eq!(
        hex(&Sha256::digest(source.as_bytes())),
        "93a70352e47926baf33b0e64b016d20fa17c1a94885095394acb526ea964b156",
        "storage identity capture digest drift"
    );
    let expected: Value = json::from_str(source).expect("immutable storage identity fixture");
    fixture_json::assert_json_matches(&render(), &expected, "owned storage identities");
}
