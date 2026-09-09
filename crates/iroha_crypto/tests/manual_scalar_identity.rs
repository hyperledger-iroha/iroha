//! Immutable pre-declaration identities and bytes of the three manual crypto scalar owners.

use iroha_crypto::{Algorithm, BfvGoldilocksDigest384V1, Hash, HashOf, KeyPair, SignatureOf};
use norito::{NoritoDeserialize, NoritoSerialize, codec::Encode, json::Value};

#[cfg(feature = "json")]
trait FixtureValue:
    Clone
    + norito::NoritoSchema
    + NoritoSerialize
    + for<'de> NoritoDeserialize<'de>
    + norito::json::JsonSerialize
    + norito::json::JsonDeserialize
{
}
#[cfg(feature = "json")]
impl<T> FixtureValue for T where
    T: Clone
        + norito::NoritoSchema
        + NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + norito::json::JsonSerialize
        + norito::json::JsonDeserialize
{
}
#[cfg(not(feature = "json"))]
trait FixtureValue:
    Clone + norito::NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de>
{
}
#[cfg(not(feature = "json"))]
impl<T> FixtureValue for T where
    T: Clone + norito::NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de>
{
}

fn record<T: FixtureValue>(case: &str, value: &T) -> Value {
    assert_eq!(T::frame_name(), T::nominal_name());
    let bare = value.encode();
    let frame = norito::encode_canonical(value).expect("encode complete scalar frame");
    let decoded: T = norito::decode_canonical(&frame).expect("decode complete scalar frame");
    assert_eq!(decoded.encode(), bare);
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), frame);
    let header = norito::core::Header::read(std::io::Cursor::new(&frame)).unwrap();
    assert_eq!(header.schema, norito::schema::identity::frame_hash::<T>());
    let mut wrong_schema = frame.clone();
    wrong_schema[6] ^= 1;
    assert!(norito::decode_canonical::<T>(&wrong_schema).is_err());
    assert!(norito::decode_canonical::<T>(&frame[..frame.len() - 1]).is_err());
    #[cfg(feature = "json")]
    let json = {
        let text = norito::json::to_json(value).expect("encode scalar JSON");
        let decoded: T = norito::json::from_json(&text).expect("decode scalar JSON");
        assert_eq!(decoded.encode(), bare);
        assert_eq!(norito::json::to_json(&decoded).unwrap(), text);
        Value::from(text)
    };
    #[cfg(not(feature = "json"))]
    let json = Value::Null;
    norito::json!({
        "case": case,
        "declared_nominal_name": (T::nominal_name()),
        "serialize_hash": (hex::encode(norito::schema::identity::frame_hash::<T>())),
        "deserialize_hash": (hex::encode(norito::schema::identity::frame_hash::<T>())),
        "header_flags": (header.flags),
        "bare_hex": (hex::encode(bare)),
        "frame_hex": (hex::encode(frame)),
        "json": json,
    })
}

fn family<T: FixtureValue>(rows: &mut Vec<Value>, label: &str, value: T, signer: &KeyPair) {
    let hash = HashOf::new(&value);
    let signature = SignatureOf::try_from_hash(signer.private_key(), hash)
        .expect("sign public deterministic fixture value");
    signature
        .verify(signer.public_key(), &value)
        .expect("verify fixture signature");
    signature
        .verify_hash(signer.public_key(), hash)
        .expect("verify exact fixture hash");
    rows.extend([
        record(&format!("{label}/root"), &value),
        record(&format!("{label}/option-none"), &None::<T>),
        record(&format!("{label}/option-some"), &Some(value.clone())),
        record(&format!("{label}/vec-empty"), &Vec::<T>::new()),
        record(&format!("{label}/vec-two"), &vec![value.clone(), value]),
        record(&format!("{label}/hash-of"), &hash),
        record(&format!("{label}/signature-of"), &signature),
    ]);
}

fn current_frames() -> Value {
    // Public test-only key material, matching the existing key identity fixture workflow.
    let signer = KeyPair::try_from_seed(
        b"Iroha crypto wire identity fixtures only".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("deterministic fixture signer");
    let mut rows = Vec::new();
    let hash = Hash::prehashed([7; Hash::LENGTH]);
    assert_eq!(hash.encode(), vec![7; Hash::LENGTH]);
    family(&mut rows, "hash", hash, &signer);
    // Exact existing digest_wire_kat_is_six_canonical_little_endian_words values.
    let digest = BfvGoldilocksDigest384V1::new([
        0x0a08_4d27_65a9_990b,
        0xd59f_602c_37b6_9e1b,
        0xde9b_b335_7209_fa18,
        0x3faf_16ba_65a6_7ba3,
        0xe68c_cc7d_9933_b79d,
        0xcad6_6b94_7931_4d52,
    ])
    .expect("canonical Goldilocks KAT words");
    assert_eq!(
        hex::encode(digest.encode()),
        "0b99a965274d080a1b9eb6372c609fd518fa097235b39bdea37ba665ba16af3f9db733997dcc8ce6524d3179946bd6ca"
    );
    family(&mut rows, "goldilocks-digest384", digest, &signer);
    #[cfg(feature = "sm")]
    {
        let digest = iroha_crypto::Sm3Digest::hash(b"abc");
        assert_eq!(
            hex::encode_upper(digest.as_bytes()),
            "66C7F0F462EEEDD9D1F2D46BDC10E4E24167C4875CF2F7A2297DA02B8F4BA8E0"
        );
        family(&mut rows, "sm3", digest, &signer);
    }
    assert_eq!(rows.len(), if cfg!(feature = "sm") { 21 } else { 14 });
    let cases: std::collections::BTreeSet<_> = rows
        .iter()
        .map(|row| row.get("case").unwrap().as_str().unwrap())
        .collect();
    assert_eq!(cases.len(), rows.len(), "every fixture case is unique");
    norito::json!({
        "schema": 1,
        "sm": (cfg!(feature = "sm")),
        "json_enabled": (cfg!(feature = "json")),
        "records": rows,
    })
}

#[test]
fn manual_scalar_identity_frames_match_pre_declaration_goldens() {
    #[cfg(all(feature = "sm", feature = "json"))]
    const FROZEN: &str = include_str!("fixtures/manual-scalars-sm-true-json-true.json");
    #[cfg(all(feature = "sm", not(feature = "json")))]
    const FROZEN: &str = include_str!("fixtures/manual-scalars-sm-true-json-false.json");
    #[cfg(all(not(feature = "sm"), feature = "json"))]
    const FROZEN: &str = include_str!("fixtures/manual-scalars-sm-false-json-true.json");
    #[cfg(all(not(feature = "sm"), not(feature = "json")))]
    const FROZEN: &str = include_str!("fixtures/manual-scalars-sm-false-json-false.json");
    let mut frozen: Value =
        norito::json::from_str(FROZEN).expect("immutable observed scalar fixtures");
    let mut current = current_frames();
    let captured_rows = frozen.get_mut("records").unwrap().as_array_mut().unwrap();
    let declared_rows = current.get_mut("records").unwrap().as_array_mut().unwrap();
    assert_eq!(declared_rows.len(), captured_rows.len());
    for (declared, captured) in declared_rows.iter_mut().zip(captured_rows.iter_mut()) {
        assert_eq!(declared.get("case"), captured.get("case"));
        // Physical moves may change Rust's current type_name, but never the captured wire identity.
        assert_eq!(
            declared.get("declared_nominal_name"),
            captured.get("actual_type_name"),
        );
        declared
            .as_object_mut()
            .unwrap()
            .remove("declared_nominal_name");
        captured.as_object_mut().unwrap().remove("actual_type_name");
    }
    assert_eq!(current, frozen);
}
