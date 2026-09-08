//! Captured schema identities and complete frames for Nexus instruction records.

use std::{collections::BTreeMap, fmt::Debug};

use norito::{
    NoritoDeserialize, NoritoSerialize,
    json::{self, JsonDeserialize, JsonSerialize, Value},
};

use super::*;

#[path = "../../../../tests/support/fixture_json.rs"]
mod fixture_json;

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut encoded = String::new();
    for byte in bytes {
        write!(encoded, "{byte:02x}").expect("String formatting");
    }
    encoded
}

fn frame<T>(value: &T) -> String
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de> + Debug + PartialEq,
{
    let bytes = norito::to_bytes(value).expect("encode Nexus instruction frame");
    let decoded: T = norito::decode_from_bytes(&bytes).expect("decode Nexus instruction frame");
    assert_eq!(&decoded, value);
    hex(&bytes)
}

fn record<T>(values: impl IntoIterator<Item = T>) -> Value
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + norito::NoritoSchema
        + Into<InstructionBox>
        + Clone
        + Debug
        + PartialEq,
{
    let identity_hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(norito::schema::identity::frame_hash::<T>(), identity_hash);
    assert_eq!(norito::schema::identity::frame_hash::<T>(), identity_hash);
    assert_eq!(T::frame_name(), T::nominal_name());
    let cases = values
        .into_iter()
        .map(|value| {
            let instruction: InstructionBox = value.clone().into();
            let json = json::to_value(&instruction).expect("Nexus instruction carrier JSON");
            let decoded: InstructionBox =
                json::from_value(json.clone()).expect("decode Nexus instruction carrier JSON");
            assert_eq!(decoded, instruction);
            json::object([
                ("instruction_json", json),
                ("instruction_frame", Value::String(frame(&instruction))),
                ("frame", Value::String(frame(&value))),
                ("vector_frame", Value::String(frame(&vec![value.clone()]))),
                ("option_frame", Value::String(frame(&Some(value.clone())))),
                (
                    "map_frame",
                    Value::String(frame(&BTreeMap::from([(7_u8, value)]))),
                ),
            ])
            .expect("Nexus instruction case")
        })
        .collect();
    json::object([
        ("nominal", Value::String(T::nominal_name())),
        (
            "serialize_hash",
            Value::String(hex(&norito::schema::identity::frame_hash::<T>())),
        ),
        (
            "deserialize_hash",
            Value::String(hex(&norito::schema::identity::frame_hash::<T>())),
        ),
        ("cases", Value::Array(cases)),
    ])
    .expect("Nexus instruction type")
}

fn record_json<T>(values: [T; 2]) -> Value
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + norito::NoritoSchema
        + JsonSerialize
        + JsonDeserialize
        + Into<InstructionBox>
        + Clone
        + Debug
        + PartialEq,
{
    let mut row = record(values.clone());
    let cases = row
        .get_mut("cases")
        .expect("recorded cases")
        .as_array_mut()
        .expect("case array");
    for (case, value) in cases.iter_mut().zip(values) {
        let json = json::to_value(&value).expect("direct Nexus instruction JSON");
        let decoded: T = json::from_value(json.clone()).expect("decode direct instruction JSON");
        assert_eq!(decoded, value);
        case.as_object_mut()
            .expect("case object")
            .insert("json".to_owned(), json);
    }
    row
}

fn verified_instruction_records() -> [Value; 2] {
    let mut relay = sample_lane_relay_instruction();
    relay.envelope = sample_envelope(9);
    relay.proof_blob = sample_proof_blob(0x41);
    relay.proof_blob.expiry_slot = Some(110);
    let mut allocation = sample_fee_budget_instruction();
    allocation.program_revision = 2;
    allocation.verified_allocation = "0.125".parse().expect("fractional allocation");
    allocation.source_height = 9;
    allocation.expires_at_height = 109;
    allocation.proof_blob.expiry_slot = Some(110);
    [
        record([sample_fee_budget_instruction(), allocation]),
        record([sample_lane_relay_instruction(), relay]),
    ]
}

fn sponsor_instruction_records() -> [Value; 10] {
    let programs = [
        sample_fee_sponsor_program(),
        FeeSponsorProgram::new(
            FeeSponsorProgramId::new(
                sponsor_account_id(),
                "secondary".parse().expect("program name"),
            ),
            sponsor_account_id(),
        ),
    ];
    let ids = programs.clone().map(|program| program.id);
    [
        record(
            ids.clone()
                .map(|program_id| ActivateFeeSponsorProgramRevision {
                    program_id,
                    revision: 1,
                    activate_at_height: 10,
                }),
        ),
        record(
            ids.clone()
                .map(|program_id| BeginCloseFeeSponsorProgram { program_id }),
        ),
        record(
            ids.clone()
                .map(|program_id| CloseFeeSponsorProgram { program_id }),
        ),
        record(programs.map(|program| CreateFeeSponsorProgram { program })),
        record(ids.clone().map(|program_id| EnrollFeeSponsorBeneficiary {
            program_id,
            beneficiary: sponsor_account_id(),
        })),
        record(ids.clone().map(|program_id| FundFeeSponsorProgram {
            program_id,
            asset_definition_id: sample_fee_asset_id(),
            amount: Quantity::from(10_u32),
        })),
        record(
            ids.clone()
                .map(|program_id| PauseFeeSponsorProgram { program_id }),
        ),
        record(
            ids.clone()
                .map(|program_id| StageFeeSponsorProgramRevision {
                    revision: sample_fee_sponsor_revision(program_id),
                }),
        ),
        record(ids.clone().map(|program_id| UnenrollFeeSponsorBeneficiary {
            program_id,
            beneficiary: sponsor_account_id(),
        })),
        record_json(ids.map(|program_id| WithdrawFeeSponsorProgram {
            program_id,
            asset_definition_id: sample_fee_asset_id(),
            amount: "0.001".parse().expect("fractional withdrawal"),
        })),
    ]
}

#[test]
fn nexus_instructions_preserve_captured_frames() {
    let mut rows = sponsor_instruction_records().to_vec();
    rows.extend(verified_instruction_records());
    rows.sort_by(|left, right| {
        left.get("nominal")
            .unwrap()
            .as_str()
            .cmp(&right.get("nominal").unwrap().as_str())
    });
    assert_eq!(rows.len(), 12);
    assert_eq!(
        rows.iter()
            .map(|row| row.get("cases").unwrap().as_array().unwrap().len())
            .sum::<usize>(),
        24
    );
    let captured: Value = json::from_str(include_str!(
        "../../../../tests/fixtures/nexus_instruction_generated_identity_frames.json"
    ))
    .expect("immutable Nexus instruction capture");
    fixture_json::assert_json_matches(
        &captured,
        &Value::Array(rows),
        "Nexus instruction identities",
    );
}
