//! Typed cryptographic marker identities without marker serialization bounds.

use iroha_crypto::{Hash, HashOf, Signature, SignatureOf};
use norito::schema::identity::frame_hash;
use norito::{
    DeserializePayload, NoritoDeserialize, NoritoSchema, NoritoSerialize, core::Header, json,
};

#[derive(NoritoSchema)]
#[norito_schema(name = "iroha_crypto_group_01::schema_identity::OriginalMarker")]
struct OriginalMarker;

#[derive(NoritoSchema)]
#[norito_schema(name = "iroha_crypto_group_01::schema_identity::OriginalMarker")]
struct RelocatedMarker;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn record<T: NoritoSchema + NoritoSerialize + NoritoDeserialize<'static>>(value: T) -> json::Value {
    let frame = norito::to_bytes(&value).unwrap();
    assert_eq!(T::nominal_name(), std::any::type_name::<T>());
    assert_eq!(frame_hash::<T>(), <T as NoritoSerialize>::schema_hash());
    assert_eq!(frame_hash::<T>(), <T as NoritoDeserialize>::schema_hash());
    norito::json!({
        "nominal": (std::any::type_name::<T>()),
        "frame_name": (T::frame_name()),
        "serialize_hash": (hex(&<T as NoritoSerialize>::schema_hash())),
        "deserialize_hash": (hex(&<T as NoritoDeserialize>::schema_hash())),
        "frame_hex": (hex(&frame)),
    })
}

fn current_frames() -> Vec<json::Value> {
    vec![
        record(HashOf::<OriginalMarker>::from_untyped_unchecked(
            Hash::prehashed([7; 32]),
        )),
        record(SignatureOf::<OriginalMarker>::from_signature(
            Signature::from_bytes(&[3; 64]),
        )),
    ]
}

#[test]
fn marker_wrapper_frame_golden() {
    let expected: Vec<json::Value> =
        json::from_str(include_str!("fixtures/schema_identity_frames.json")).unwrap();
    assert_eq!(current_frames(), expected);
}

#[test]
fn relocated_markers_preserve_identity_and_payload_without_codec_implementations() {
    assert_eq!(
        HashOf::<OriginalMarker>::nominal_name(),
        HashOf::<RelocatedMarker>::nominal_name()
    );
    assert_eq!(
        frame_hash::<SignatureOf<OriginalMarker>>(),
        frame_hash::<SignatureOf<RelocatedMarker>>()
    );
    let hash = Hash::prehashed([7; 32]);
    let original =
        norito::to_bytes(&HashOf::<OriginalMarker>::from_untyped_unchecked(hash)).unwrap();
    let relocated =
        norito::to_bytes(&HashOf::<RelocatedMarker>::from_untyped_unchecked(hash)).unwrap();
    assert_eq!(original[Header::SIZE..], relocated[Header::SIZE..]);
    assert_eq!(original[6..22], frame_hash::<HashOf<RelocatedMarker>>());
    let archived = norito::from_bytes::<HashOf<OriginalMarker>>(&original).unwrap();
    let decoded = HashOf::<OriginalMarker>::deserialize(archived);
    assert_eq!(norito::to_bytes(&decoded).unwrap(), original);
    let signature = Signature::from_bytes(&[3; 64]);
    let original = norito::to_bytes(&SignatureOf::<OriginalMarker>::from_signature(
        signature.clone(),
    ))
    .unwrap();
    let relocated =
        norito::to_bytes(&SignatureOf::<RelocatedMarker>::from_signature(signature)).unwrap();
    assert_eq!(original[Header::SIZE..], relocated[Header::SIZE..]);
    assert_eq!(
        original[6..22],
        frame_hash::<SignatureOf<RelocatedMarker>>()
    );
    let archived = norito::from_bytes::<SignatureOf<OriginalMarker>>(&original).unwrap();
    let decoded = SignatureOf::<OriginalMarker>::deserialize(archived);
    assert_eq!(norito::to_bytes(&decoded).unwrap(), original);
}
