//! Complete original numeric topology frames, schemas, JSON and storage keys.

use super::{DataSpaceId, LaneId, ShardId};
use iroha_crypto::{Algorithm, HashOf, KeyPair, SignatureOf};
use iroha_schema::IntoSchema;
use norito::{
    NoritoDeserialize, NoritoSchema, NoritoSerialize,
    codec::Encode,
    json::{self, JsonDeserialize, JsonSerialize, Value},
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

fn with_schema<T: NoritoSchema + NoritoSerialize + for<'a> NoritoDeserialize<'a> + IntoSchema>(
    records: &mut Vec<Value>,
    case: &str,
    value: &T,
    signer: &KeyPair,
) {
    let index = records.len();
    envelopes(records, case, value, signer);
    let schema = T::schema();
    let entry = records[index].as_object_mut().expect("frame record object");
    entry.insert(
        "schema".to_owned(),
        json::to_value(&schema).expect("encode original schema"),
    );
    entry.insert("schema_type_name".to_owned(), Value::from(T::type_name()));
    entry.insert(
        "schema_type_id".to_owned(),
        Value::from(<T as iroha_schema::TypeId>::id()),
    );
    let mut identifiers = schema
        .iter()
        .map(|(_, row)| (row.type_id.clone(), row.type_name.clone()))
        .collect::<Vec<_>>();
    identifiers.sort();
    entry.insert(
        "schema_identifiers".to_owned(),
        Value::Array(
            identifiers
                .into_iter()
                .map(|(id, name)| norito::json!({"id": id, "name": name}))
                .collect(),
        ),
    );
}

fn public<
    T: NoritoSchema
        + NoritoSerialize
        + for<'a> NoritoDeserialize<'a>
        + IntoSchema
        + JsonSerialize
        + JsonDeserialize,
>(
    records: &mut Vec<Value>,
    case: &str,
    value: &T,
    signer: &KeyPair,
) {
    let index = records.len();
    with_schema(records, case, value, signer);
    let encoded = json::to_json(value).expect("encode original JSON");
    let decoded: T = json::from_str(&encoded).expect("decode original JSON");
    assert_eq!(decoded.encode(), value.encode(), "JSON roundtrip: {case}");
    records[index]
        .as_object_mut()
        .expect("frame record object")
        .insert("json".to_owned(), Value::from(encoded));
}

fn storage_key<T: NoritoSerialize + mv::json::JsonKeyCodec>(
    records: &mut [Value],
    case: &str,
    value: &T,
) {
    let mut encoded = String::new();
    value.encode_json_key(&mut encoded);
    let literal: String = json::from_str(&encoded).expect("storage key is one JSON string");
    let decoded = T::decode_json_key(&literal).expect("decode original storage key");
    assert_eq!(
        decoded.encode(),
        value.encode(),
        "storage key roundtrip: {case}"
    );
    let mut reencoded = String::new();
    decoded.encode_json_key(&mut reencoded);
    assert_eq!(reencoded, encoded, "canonical storage key: {case}");
    let entry = records
        .iter_mut()
        .find(|record| record.get("case").and_then(Value::as_str) == Some(case))
        .expect("matching root frame")
        .as_object_mut()
        .expect("root frame object");
    entry.insert("storage_key_json".to_owned(), Value::from(encoded));
}

#[test]
fn topology_owner_matches_all_pre_extraction_envelopes_and_storage_keys() {
    let captured: Vec<Value> = json::from_str(include_str!(
        "../../../iroha_data_model/tests/fixtures/base_model_wire_identity_frames.json"
    ))
    .expect("parse immutable model fixtures");
    assert_eq!(captured.len(), 189);
    let expected: Vec<_> = captured
        .into_iter()
        .filter(|record| {
            let case = record
                .get("case")
                .and_then(Value::as_str)
                .expect("fixture case");
            case.split('/')
                .next()
                .is_some_and(|root| matches!(root, "dataspace" | "lane" | "shard"))
        })
        .collect();
    assert_eq!(
        expected.len(),
        21,
        "every captured numeric topology envelope"
    );
    let signer = KeyPair::try_from_seed(
        b"base model identity fixture A".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("deterministic fixture key");
    let mut records = Vec::new();
    public(&mut records, "dataspace", &DataSpaceId::new(7), &signer);
    public(&mut records, "lane", &LaneId::new(3), &signer);
    public(&mut records, "shard", &ShardId::new(24), &signer);
    storage_key(&mut records, "dataspace", &DataSpaceId::new(7));
    storage_key(&mut records, "lane", &LaneId::new(3));
    assert_eq!(records, expected);
}
