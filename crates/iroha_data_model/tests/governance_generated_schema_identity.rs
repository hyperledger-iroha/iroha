//! Captured identities and complete frames for the generated governance hash wrappers.

use std::{collections::BTreeMap, fmt::Debug};

use iroha_data_model::governance::types::*;
use norito::{
    NoritoDeserialize, NoritoSerialize,
    json::{self, JsonSerialize, Value},
};

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut output = String::new();
    for byte in bytes {
        write!(output, "{byte:02x}").expect("String formatting");
    }
    output
}

fn frame<T>(value: &T) -> String
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de> + Debug + PartialEq,
{
    let bytes = norito::to_bytes(value).expect("encode governance hash frame");
    let decoded: T = norito::decode_from_bytes(&bytes).expect("decode governance hash frame");
    assert_eq!(&decoded, value);
    let header = norito::core::Header::read(bytes.as_slice()).expect("read frame header");
    assert_eq!(header.schema, norito::schema::identity::frame_hash::<T>());
    let mut wrong_schema = bytes.clone();
    wrong_schema[6] ^= 1;
    assert!(matches!(
        norito::decode_from_bytes::<T>(&wrong_schema),
        Err(norito::Error::SchemaMismatch)
    ));
    hex(&bytes)
}

fn record<T>(values: impl IntoIterator<Item = T>) -> Value
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + norito::NoritoSchema
        + JsonSerialize
        + Clone
        + Debug
        + PartialEq,
{
    let identity_hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(T::frame_name(), T::nominal_name());
    let cases = values
        .into_iter()
        .map(|value| {
            json::object([
                (
                    "json",
                    json::to_value(&value).expect("governance hash JSON"),
                ),
                ("frame", Value::String(frame(&value))),
                ("vector_frame", Value::String(frame(&vec![value.clone()]))),
                ("option_frame", Value::String(frame(&Some(value.clone())))),
                (
                    "map_frame",
                    Value::String(frame(&BTreeMap::from([(7_u8, value)]))),
                ),
            ])
            .expect("governance hash value")
        })
        .collect();
    json::object([
        ("nominal", Value::String(T::nominal_name())),
        ("serialize_hash", Value::String(hex(&identity_hash))),
        ("deserialize_hash", Value::String(hex(&identity_hash))),
        ("cases", Value::Array(cases)),
    ])
    .expect("governance hash type")
}

fn generated_hashes() -> Vec<Value> {
    let mut records = Vec::new();
    macro_rules! hash {
        ($ty:ty) => {
            records.push(record([
                <$ty>::new([0; 32]),
                <$ty>::new([0xff; 32]),
                <$ty>::new([0x42; 32]),
                <$ty>::new(core::array::from_fn(|index| {
                    u8::try_from(index).expect("hash byte index")
                })),
            ]));
        };
    }
    hash!(ContractCodeHash);
    hash!(ContractAbiHash);
    hash!(AgendaItemId);
    hash!(DraftId);
    hash!(ProposalContentId);
    hash!(GovernanceAttemptId);
    hash!(BodyInstanceId);
    hash!(BodyElectionAttemptId);
    hash!(AssignmentId);
    hash!(SortitionRequestId);
    hash!(BallotAttemptId);
    hash!(BeaconSessionId);
    hash!(BeaconPulseId);
    hash!(TleSessionId);
    hash!(TleKeySessionId);
    hash!(GovernanceCertificateId);
    records
}

#[test]
fn generated_governance_hashes_preserve_captured_frames() {
    let rows = generated_hashes();
    assert_eq!(rows.len(), 16);
    let captured: Value = json::from_str(include_str!(
        "fixtures/governance_generated_identity_frames.json"
    ))
    .expect("immutable governance hash capture");
    super::fixture_json::assert_json_matches(
        &captured,
        &Value::Array(rows),
        "generated governance hash identities",
    );
}
