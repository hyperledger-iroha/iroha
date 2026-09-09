//! Current Kaigi record frames and explicit rejection of retired scalar payloads.
use super::{capture, captured, fixture_json, frame_fields, hex, unhex};
use norito::{
    NoritoDeserialize, NoritoSchema, NoritoSerialize,
    json::{self, Value},
};
use std::{collections::BTreeMap, fmt::Debug, sync::OnceLock};

fn current_capture() -> &'static [Value] {
    static CAPTURE: OnceLock<Value> = OnceLock::new();
    CAPTURE
        .get_or_init(|| {
            use sha2::{Digest as _, Sha256};
            let source =
                include_str!("../../tests/fixtures/kaigi_canonical_instruction_record_frames.json");
            assert_eq!(
                hex(&Sha256::digest(source.as_bytes())),
                "996b949a5cadcad9229fe9678ea10f20fca343e3c2335bd726d6d4c225e5bd93"
            );
            let value: Value = json::from_str(source).expect("canonical Kaigi capture");
            let rows = value.as_array().expect("Kaigi rows");
            assert_eq!(rows.len(), 5);
            let names: Vec<_> = rows
                .iter()
                .map(|row| row.get("nominal").and_then(Value::as_str).expect("nominal"))
                .collect();
            assert_eq!(
                names,
                [
                    "iroha_data_model::isi::kaigi::CreateKaigi",
                    "iroha_data_model::isi::kaigi::EndKaigi",
                    "iroha_data_model::isi::kaigi::JoinKaigi",
                    "iroha_data_model::isi::kaigi::LeaveKaigi",
                    "iroha_data_model::isi::kaigi::RecordKaigiUsage",
                ]
            );
            value
        })
        .as_array()
        .expect("canonical Kaigi rows")
}

fn reject_retired<T>(case: &Value, field: &str)
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de> + Debug,
{
    let bytes = unhex(
        case.get(field)
            .and_then(Value::as_str)
            .expect("retired frame"),
    );
    let error =
        norito::decode_from_bytes::<T>(&bytes).expect_err("retired scalar payload must not decode");
    assert!(
        matches!(error, norito::core::Error::Message(ref reason)
        if reason == "noncanonical Kaigi authorization Pasta Fp scalar"),
        "{field}: {error:?}"
    );
}

pub(super) fn check<T>(nominal: &str, retired_case: usize)
where
    T: NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de> + Clone + Debug + PartialEq,
{
    assert_eq!(T::nominal_name(), nominal);
    assert_eq!(T::frame_name(), nominal);
    let historical = captured(nominal);
    let current = current_capture()
        .iter()
        .find(|row| row.get("nominal").and_then(Value::as_str) == Some(nominal))
        .expect("current Kaigi record");
    for row in [historical, current] {
        assert_eq!(
            row.get("serialize_hash").and_then(Value::as_str),
            Some(hex(&norito::schema::identity::frame_hash::<T>()).as_str())
        );
        assert_eq!(
            row.get("deserialize_hash").and_then(Value::as_str),
            Some(hex(&norito::schema::identity::frame_hash::<T>()).as_str())
        );
    }
    let cases = historical
        .get("cases")
        .and_then(Value::as_array)
        .expect("original cases");
    assert_eq!(
        cases.len(),
        2,
        "preserve both original public and private cases"
    );
    assert!(retired_case < cases.len());
    for (index, expected) in cases.iter().enumerate() {
        if index == retired_case {
            reject_retired::<T>(expected, "frame");
            reject_retired::<Vec<T>>(expected, "vector_frame");
            reject_retired::<Option<T>>(expected, "option_frame");
            reject_retired::<BTreeMap<u8, T>>(expected, "map_frame");
        } else {
            let bytes = unhex(
                expected
                    .get("frame")
                    .and_then(Value::as_str)
                    .expect("original public frame"),
            );
            let value: T = norito::decode_from_bytes(&bytes).expect("original public record");
            fixture_json::assert_json_matches(expected, &frame_fields(&capture(value)), nominal);
        }
    }
    let bytes = unhex(
        current
            .get("frame")
            .and_then(Value::as_str)
            .expect("current private frame"),
    );
    let value: T = norito::decode_from_bytes(&bytes).expect("canonical private record");
    fixture_json::assert_json_matches(current, &capture(value), nominal);
}

use crate::{
    account::AccountId,
    domain::DomainId,
    isi::kaigi::*,
    kaigi::{
        KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier, KaigiPrivacyMode,
        KaigiRelayHop, KaigiRelayManifest, KaigiRoomPolicy, NewKaigi,
        scalar::KaigiAuthorizationScalarV1,
    },
    name::Name,
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
fn account(seed: u8) -> AccountId {
    let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap();
    AccountId::new(key.public_key().clone())
}
fn call_id() -> KaigiId {
    KaigiId::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "standup".parse::<Name>().unwrap(),
    )
}
fn scalar(byte: u8) -> KaigiAuthorizationScalarV1 {
    let mut bytes = [0; 32];
    bytes[0] = byte;
    KaigiAuthorizationScalarV1::from_le_bytes(bytes).unwrap()
}
fn commitment() -> KaigiParticipantCommitment {
    KaigiParticipantCommitment {
        commitment: scalar(0x11),
    }
}
fn nullifier() -> KaigiParticipantNullifier {
    KaigiParticipantNullifier {
        digest: scalar(0x22),
    }
}
fn current_values() -> Vec<Value> {
    let mut call = NewKaigi::with_defaults(call_id(), account(1));
    call.title = Some("Daily standup".to_owned());
    call.description = Some("Engineering sync".to_owned());
    call.max_participants = Some(16);
    call.gas_rate_per_minute = 5;
    call.scheduled_start_ms = Some(1_700_010_000);
    call.billing_account = Some(account(2));
    call.privacy_mode = KaigiPrivacyMode::ZkRosterV1;
    call.room_policy = KaigiRoomPolicy::Authenticated;
    call.relay_manifest = Some(KaigiRelayManifest {
        hops: vec![KaigiRelayHop {
            relay_id: account(3),
            hpke_public_key: vec![0xA1, 0xA2, 0xA3],
            weight: 2,
        }],
        expiry_ms: 1_700_100_000,
    });
    let root = Hash::new("kaigi-roster-root");
    vec![
        capture(CreateKaigi {
            call,
            commitment: Some(commitment()),
            nullifier: Some(nullifier()),
            roster_root: Some(root),
            proof: Some(vec![1, 2, 3]),
        }),
        capture(EndKaigi {
            call_id: call_id(),
            ended_at_ms: Some(1_700_020_000),
            commitment: Some(commitment()),
            nullifier: Some(nullifier()),
            roster_root: Some(root),
            proof: Some(vec![8, 9]),
        }),
        capture(JoinKaigi {
            call_id: call_id(),
            participant: account(5),
            commitment: Some(commitment()),
            nullifier: Some(nullifier()),
            roster_root: Some(root),
            proof: Some(vec![4, 5]),
        }),
        capture(LeaveKaigi {
            call_id: call_id(),
            participant: account(5),
            commitment: Some(commitment()),
            nullifier: Some(nullifier()),
            roster_root: Some(root),
            proof: Some(vec![6, 7]),
        }),
        capture(RecordKaigiUsage {
            call_id: call_id(),
            duration_ms: 60_000,
            billed_gas: 15,
            usage_commitment: Some(scalar(7)),
            proof: Some(vec![0x10, 0x11]),
        }),
    ]
}

#[test]
fn populated_canonical_records_match_declared_fixture() {
    let values = current_values();
    assert_eq!(values.len(), current_capture().len());
    for (expected, actual) in current_capture().iter().zip(values.iter()) {
        fixture_json::assert_json_matches(expected, actual, "populated canonical Kaigi record");
    }
}
