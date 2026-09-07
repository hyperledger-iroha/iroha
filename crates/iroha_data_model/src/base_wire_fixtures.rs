//! Immutable pre-extraction foundational model frames and signing transcripts.

use crate::{
    ChainId,
    account::{
        AccountController, AccountId, MultisigMember, MultisigPolicy,
        address::{AccountAddress, ChainDiscriminantGuard},
    },
    asset::{AssetBalanceScope, AssetDefinitionId, AssetId},
    domain::DomainId,
    metadata::Metadata,
    name::Name,
    nexus::{DataSpaceId, LaneId, ShardId},
    peer::PeerId,
    state_path::StatePath,
};
use iroha_crypto::{Algorithm, HashOf, KeyPair, SignatureOf};
use iroha_primitives::json::Json;
use iroha_schema::IntoSchema;
use norito::{
    NoritoDeserialize, NoritoSchema, NoritoSerialize,
    codec::Encode,
    json::{self, JsonDeserialize, JsonSerialize, Value},
};
use std::collections::BTreeMap;

fn record<T: NoritoSchema + NoritoSerialize + for<'a> NoritoDeserialize<'a>>(
    case: &str,
    value: &T,
) -> Value {
    assert_eq!(
        norito::schema::identity::frame_hash::<T>(),
        <T as NoritoSerialize>::schema_hash()
    );
    assert_eq!(
        norito::schema::identity::frame_hash::<T>(),
        <T as NoritoDeserialize>::schema_hash()
    );
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
        "serialize_hash": (hex::encode(<T as NoritoSerialize>::schema_hash())),
        "deserialize_hash": (hex::encode(<T as NoritoDeserialize>::schema_hash())),
        "bare_hex": (hex::encode(value.encode())),
        "frame_hex": (hex::encode(frame)),
    })
}

pub(crate) fn envelopes<T: NoritoSchema + NoritoSerialize + for<'a> NoritoDeserialize<'a>>(
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

fn current_frames() -> Vec<Value> {
    let _context = ChainDiscriminantGuard::enter(369);
    // Public test keys, never runtime signing material.
    let signer = KeyPair::try_from_seed(
        b"base model identity fixture A".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("deterministic fixture key");
    let other = KeyPair::try_from_seed(
        b"base model identity fixture B".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("second deterministic fixture key");
    let single = AccountId::new(signer.public_key().clone());
    let member = MultisigMember::new(signer.public_key().clone(), 2).expect("weighted member");
    let policy = MultisigPolicy::new(
        2,
        vec![
            member.clone(),
            MultisigMember::new(other.public_key().clone(), 1).expect("second member"),
        ],
    )
    .expect("canonical multisignature policy");
    let multisig = AccountId::new_multisig(policy.clone());
    let name: Name = "café".parse().expect("exact NFC name");
    let state_path: StatePath = "store/001122aabb".parse().expect("canonical state path");
    let domain = DomainId::try_new("archive", "paynet").expect("qualified domain");
    let definition = AssetDefinitionId::derive_from_components(
        domain.clone(),
        "receipt".parse().expect("asset seed name"),
    );
    let global_asset = AssetId::new(definition.clone(), single.clone());
    let scoped_asset = AssetId::with_scope(
        definition.clone(),
        multisig.clone(),
        AssetBalanceScope::Dataspace(DataSpaceId::new(7)),
    );
    let mut metadata = Metadata::default();
    metadata.insert(
        name.clone(),
        Json::new(norito::json!({"count": 7, "ready": true})),
    );
    metadata.insert("alpha".parse().expect("metadata name"), Json::new("value"));
    let mut records = Vec::new();
    public(&mut records, "name/nfc", &name, &signer);
    public(&mut records, "state-path", &state_path, &signer);
    public(
        &mut records,
        "metadata/empty",
        &Metadata::default(),
        &signer,
    );
    public(&mut records, "metadata/ordered", &metadata, &signer);
    public(
        &mut records,
        "chain",
        &ChainId::from("base-fixture-1"),
        &signer,
    );
    public(&mut records, "account/single", &single, &signer);
    public(&mut records, "account/multisig", &multisig, &signer);
    public(
        &mut records,
        "controller/single",
        &AccountController::single(signer.public_key().clone()),
        &signer,
    );
    public(
        &mut records,
        "controller/multisig",
        &AccountController::multisig(policy.clone()),
        &signer,
    );
    public(&mut records, "multisig-policy", &policy, &signer);
    public(&mut records, "multisig-member", &member, &signer);
    public(
        &mut records,
        "address/single",
        &AccountAddress::from_account_id(&single).expect("single address"),
        &signer,
    );
    public(
        &mut records,
        "address/multisig",
        &AccountAddress::from_account_id(&multisig).expect("multisig address"),
        &signer,
    );
    public(&mut records, "domain", &domain, &signer);
    public(&mut records, "asset-definition", &definition, &signer);
    public(
        &mut records,
        "asset-scope/global",
        &AssetBalanceScope::Global,
        &signer,
    );
    public(
        &mut records,
        "asset-scope/dataspace",
        &AssetBalanceScope::Dataspace(DataSpaceId::new(7)),
        &signer,
    );
    public(&mut records, "asset/global", &global_asset, &signer);
    public(&mut records, "asset/dataspace", &scoped_asset, &signer);
    public(&mut records, "dataspace", &DataSpaceId::new(7), &signer);
    public(&mut records, "lane", &LaneId::new(3), &signer);
    public(&mut records, "shard", &ShardId::new(24), &signer);
    public(
        &mut records,
        "peer",
        &PeerId::new(signer.public_key().clone()),
        &signer,
    );

    storage_key(&mut records, "name/nfc", &name);
    storage_key(&mut records, "state-path", &state_path);
    storage_key(&mut records, "account/single", &single);
    storage_key(&mut records, "account/multisig", &multisig);
    storage_key(&mut records, "domain", &domain);
    storage_key(&mut records, "asset-definition", &definition);
    storage_key(&mut records, "asset/global", &global_asset);
    storage_key(&mut records, "asset/dataspace", &scoped_asset);
    storage_key(&mut records, "dataspace", &DataSpaceId::new(7));
    storage_key(&mut records, "lane", &LaneId::new(3));

    let map = BTreeMap::from([
        (name, Json::new(7)),
        (
            "alpha".parse::<Name>().expect("map key"),
            Json::new("value"),
        ),
    ]);
    public(&mut records, "name-json-map", &map, &signer);
    envelopes(
        &mut records,
        "mixed-identities",
        &(single, vec![multisig], Some(state_path)),
        &signer,
    );
    crate::id::base_wire_fixtures::append(&mut records, &signer);
    records
}

#[test]
fn base_model_frames_match_pre_extraction_goldens() {
    let expected: Vec<Value> = json::from_str(include_str!(
        "../tests/fixtures/base_model_wire_identity_frames.json"
    ))
    .expect("parse original model fixtures");
    assert_eq!(expected.len(), 189);
    let cases: std::collections::BTreeSet<_> = expected
        .iter()
        .map(|record| {
            record
                .get("case")
                .and_then(Value::as_str)
                .expect("fixture case")
        })
        .collect();
    assert_eq!(cases.len(), 189, "unique capture cases");
    assert_eq!(
        expected
            .iter()
            .filter(|record| record.get("json").is_some())
            .count(),
        24
    );
    assert_eq!(
        expected
            .iter()
            .filter(|record| record.get("schema_identifiers").is_some())
            .count(),
        24
    );
    assert_eq!(
        expected
            .iter()
            .filter(|record| record.get("storage_key_json").is_some())
            .count(),
        10
    );
    assert_eq!(current_frames(), expected);
}

#[test]
fn base_model_fixture_address_context_is_scoped() {
    let _outer_context = ChainDiscriminantGuard::enter(42);
    let frames = current_frames();
    assert_eq!(crate::account::address::chain_discriminant(), 42);
    let encoded = frames
        .iter()
        .find(|record| record.get("case").and_then(Value::as_str) == Some("account/single"))
        .and_then(|record| record.get("json"))
        .and_then(Value::as_str)
        .expect("captured account JSON");
    assert!(
        json::from_str::<AccountId>(encoded).is_err(),
        "foreign network literal must reject"
    );
}
