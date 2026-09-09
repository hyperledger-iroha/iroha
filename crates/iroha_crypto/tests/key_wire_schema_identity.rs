//! Immutable pre-declaration key, algorithm and signature wire identities.

use iroha_crypto::{
    Algorithm, ExposedPrivateKey, HashOf, KeyPair, PublicKeyCompact, Signature, SignatureOf,
};
use norito::schema::identity::frame_hash;
use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize, codec::Encode, json};

fn record<T: NoritoSchema + NoritoSerialize + for<'a> NoritoDeserialize<'a>>(
    case: &str,
    value: &T,
) -> json::Value {
    assert_eq!(T::nominal_name(), std::any::type_name::<T>());
    assert_eq!(T::frame_name(), T::nominal_name());
    let frame = norito::to_bytes(value).unwrap();
    let header = norito::core::Header::read(frame.as_slice()).unwrap();
    assert_eq!(header.schema, norito::schema::identity::frame_hash::<T>());
    let decoded: T = norito::decode_from_bytes(&frame).unwrap();
    assert_eq!(norito::to_bytes(&decoded).unwrap(), frame);
    assert_eq!(decoded.encode(), value.encode());
    assert!(norito::decode_from_bytes::<T>(&frame[..frame.len() - 1]).is_err());
    let mut wrong_schema = frame.clone();
    wrong_schema[6] ^= 1;
    assert!(norito::decode_from_bytes::<T>(&wrong_schema).is_err());
    norito::json!({
        "case": case,
        "nominal": (std::any::type_name::<T>()),
        "serialize_hash": (hex::encode(norito::schema::identity::frame_hash::<T>())),
        "deserialize_hash": (hex::encode(norito::schema::identity::frame_hash::<T>())),
        "bare_hex": (hex::encode(value.encode())),
        "frame_hex": (hex::encode(frame)),
    })
}

fn record_envelopes<T: Clone + NoritoSchema + NoritoSerialize + for<'a> NoritoDeserialize<'a>>(
    records: &mut Vec<json::Value>,
    case: &str,
    value: T,
    signer: &KeyPair,
) {
    let hash = HashOf::new(&value);
    let signature = SignatureOf::try_from_hash(signer.private_key(), hash).unwrap();
    signature.verify(signer.public_key(), &value).unwrap();
    records.extend([
        record(case, &value),
        record(&format!("{case}/option-none"), &None::<T>),
        record(&format!("{case}/option-some"), &Some(value.clone())),
        record(&format!("{case}/vec-empty"), &Vec::<T>::new()),
        record(&format!("{case}/vec-one"), &vec![value]),
        record(&format!("{case}/hash-of"), &hash),
        record(&format!("{case}/signature-of"), &signature),
    ]);
}

fn current_frames() -> Vec<json::Value> {
    // Public deterministic fixture material only; never use these keys outside tests.
    let signer = KeyPair::try_from_seed(
        b"Iroha crypto wire identity fixtures only".to_vec(),
        Algorithm::Ed25519,
    )
    .unwrap();
    let mut records = Vec::new();
    record_envelopes(&mut records, "algorithm/0", Algorithm::Ed25519, &signer);
    for tag in 1..=10 {
        if let Ok(algorithm) = Algorithm::try_from(tag) {
            records.push(record(&format!("algorithm/{tag}"), &algorithm));
        }
    }
    for algorithm in [Algorithm::Ed25519, Algorithm::Secp256k1] {
        let keys =
            KeyPair::try_from_seed(b"Iroha crypto key frame fixtures only".to_vec(), algorithm)
                .unwrap();
        let compact: PublicKeyCompact =
            norito::codec::decode_adaptive(&keys.public_key().encode()).unwrap();
        let signature = Signature::try_new(keys.private_key(), b"wire identity fixture").unwrap();
        signature
            .verify(keys.public_key(), b"wire identity fixture")
            .unwrap();
        let name = algorithm.as_static_str();
        record_envelopes(&mut records, &format!("{name}/compact"), compact, &signer);
        record_envelopes(
            &mut records,
            &format!("{name}/public"),
            keys.public_key().clone(),
            &signer,
        );
        record_envelopes(
            &mut records,
            &format!("{name}/exposed-private"),
            ExposedPrivateKey(keys.private_key().clone()),
            &signer,
        );
        record_envelopes(
            &mut records,
            &format!("{name}/signature"),
            signature,
            &signer,
        );
    }
    records
}

#[test]
fn key_wire_schema_identity_frames_match_pre_declaration_goldens() {
    let mut expected: Vec<json::Value> = json::from_str(include_str!(
        "fixtures/key_wire_schema_identity_frames.json"
    ))
    .unwrap();
    assert_eq!(expected.len(), 73);
    let cases: std::collections::BTreeSet<_> = expected
        .iter()
        .map(|row| row.get("case").unwrap().as_str().unwrap())
        .collect();
    assert_eq!(cases.len(), 73, "each captured case must be unique");
    for tag in 0..=10 {
        assert!(cases.contains(format!("algorithm/{tag}").as_str()));
    }
    let names: std::collections::BTreeSet<_> = expected
        .iter()
        .map(|row| row.get("nominal").unwrap().as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 25);

    // The immutable capture contains all algorithms. Feature-reduced builds
    // verify every available value without manufacturing unavailable variants.
    expected.retain(|row| {
        let case = row.get("case").unwrap().as_str().unwrap();
        case.strip_prefix("algorithm/").is_none_or(|suffix| {
            Algorithm::try_from(suffix.split('/').next().unwrap().parse::<u8>().unwrap()).is_ok()
        })
    });
    assert_eq!(current_frames(), expected);
}

fn assert_same_payload_distinct_frames<T, U>(value: &T, representation: &U)
where
    T: NoritoSchema + NoritoSerialize + for<'a> NoritoDeserialize<'a>,
    U: NoritoSchema + NoritoSerialize + for<'a> NoritoDeserialize<'a>,
{
    assert_eq!(value.encode(), representation.encode());
    assert_ne!(frame_hash::<T>(), frame_hash::<U>());
    let value_frame = norito::to_bytes(value).unwrap();
    let representation_frame = norito::to_bytes(representation).unwrap();
    assert_eq!(
        value_frame[norito::core::Header::SIZE..],
        representation_frame[norito::core::Header::SIZE..]
    );
    assert!(norito::decode_from_bytes::<T>(&representation_frame).is_err());
    assert!(norito::decode_from_bytes::<U>(&value_frame).is_err());
}

#[test]
fn key_wire_schema_identity_retains_named_roots_for_transparent_payloads() {
    for algorithm in [Algorithm::Ed25519, Algorithm::Secp256k1] {
        let keys =
            KeyPair::try_from_seed(b"Iroha crypto key frame fixtures only".to_vec(), algorithm)
                .unwrap();
        let compact: PublicKeyCompact =
            norito::codec::decode_adaptive(&keys.public_key().encode()).unwrap();
        assert_same_payload_distinct_frames(keys.public_key(), &compact);
        let signature = Signature::try_new(keys.private_key(), b"wire identity fixture").unwrap();
        let signature_payload =
            iroha_primitives::const_vec::ConstVec::from(signature.payload().to_vec());
        assert_same_payload_distinct_frames(&signature, &signature_payload);
        let exposed = ExposedPrivateKey(keys.private_key().clone());
        assert_same_payload_distinct_frames(&exposed, &exposed.to_string());
    }
}
