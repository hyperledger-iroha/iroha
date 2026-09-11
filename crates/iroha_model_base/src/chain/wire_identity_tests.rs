//! Original private `ChainId` frames remain asserted at the private codec owner.
//!
//! The immutable full capture stays at its existing repository fixture path;
//! only ownership of the 14 private-codec assertions moves to this crate.

use super::{ChainId, ChainIdText, ChainIdWire};
use iroha_crypto::{Algorithm, HashOf, KeyPair, SignatureOf};
use norito::{
    NoritoDeserialize, NoritoSchema, NoritoSerialize,
    codec::Encode,
    json::{self, Value},
};

fn record<T: NoritoSchema + NoritoSerialize + for<'a> NoritoDeserialize<'a>>(
    case: &str,
    value: &T,
) -> Value {
    let frame = norito::to_bytes(value).expect("encode original model frame");
    let decoded: T = norito::decode_from_bytes(&frame).expect("decode original model frame");
    assert_eq!(decoded.encode(), value.encode(), "bare payload: {case}");
    assert_eq!(
        norito::to_bytes(&decoded).expect("re-encode original frame"),
        frame,
        "framed payload: {case}",
    );
    assert!(
        norito::decode_from_bytes::<T>(&frame[..frame.len() - 1]).is_err(),
        "truncated frame: {case}",
    );
    let mut wrong_schema = frame.clone();
    wrong_schema[6] ^= 1;
    assert!(
        norito::decode_from_bytes::<T>(&wrong_schema).is_err(),
        "wrong schema: {case}"
    );
    norito::json!({
        "case": case,
        "nominal": (T::nominal_name()),
        "serialize_hash": (hex::encode(norito::schema::identity::frame_hash::<T>())),
        "deserialize_hash": (hex::encode(norito::schema::identity::frame_hash::<T>())),
        "bare_hex": (hex::encode(value.encode())),
        "frame_hex": (hex::encode(frame)),
    })
}

fn envelopes<T: NoritoSchema + NoritoSerialize + for<'a> NoritoDeserialize<'a>>(
    records: &mut Vec<Value>,
    case: &str,
    value: &T,
    signer: &KeyPair,
) {
    let clone_value = || {
        norito::decode_from_bytes::<T>(&norito::to_bytes(value).expect("encode fixture copy"))
            .expect("decode fixture copy")
    };
    let hash = HashOf::new(value);
    let signature = SignatureOf::try_from_hash(signer.private_key(), hash)
        .expect("sign public deterministic fixture");
    signature
        .verify(signer.public_key(), value)
        .expect("verify fixture signature");
    records.extend([
        record(case, value),
        record(&format!("{case}/option-none"), &None::<T>),
        record(&format!("{case}/option-some"), &Some(clone_value())),
        record(&format!("{case}/vec-empty"), &Vec::<T>::new()),
        record(&format!("{case}/vec-one"), &vec![clone_value()]),
        record(&format!("{case}/hash-of"), &hash),
        record(&format!("{case}/signature-of"), &signature),
    ]);
}

fn append(records: &mut Vec<Value>, signer: &KeyPair) {
    envelopes(
        records,
        "chain/private-text",
        &ChainIdText(ChainId::from("base-fixture-1")),
        signer,
    );
    envelopes(
        records,
        "chain/private-wire",
        &ChainIdWire(ChainIdText(ChainId::from("base-fixture-1"))),
        signer,
    );
}

#[test]
fn private_chain_helpers_match_all_pre_extraction_frames() {
    let captured: Vec<Value> = json::from_str(include_str!(
        "../../../iroha_data_model/tests/fixtures/base_model_wire_identity_frames.json"
    ))
    .expect("parse original model fixtures");
    assert_eq!(captured.len(), 189);
    let expected: Vec<_> = captured
        .into_iter()
        .filter(|record| {
            let case = record
                .get("case")
                .and_then(Value::as_str)
                .expect("fixture case");
            case == "chain/private-text"
                || case.starts_with("chain/private-text/")
                || case == "chain/private-wire"
                || case.starts_with("chain/private-wire/")
        })
        .collect();
    assert_eq!(expected.len(), 14, "all private helper envelope cases");
    let signer = KeyPair::try_from_seed(
        b"base model identity fixture A".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("deterministic fixture key");
    let mut records = Vec::new();
    append(&mut records, &signer);
    assert_eq!(records, expected);
}
