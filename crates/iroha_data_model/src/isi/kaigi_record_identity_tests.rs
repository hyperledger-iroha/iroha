//! Current Kaigi record frames and explicit rejection of retired scalar payloads.
use super::{capture, captured, fixture_json, frame_fields, hex, unhex};
use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize, json::Value};
use std::fmt::Debug;

#[path = "kaigi_retired_scalar_rejection_tests.rs"]
mod rejected_scalars;

fn private_capture(nominal: &str) -> Value {
    let row = captured(nominal);
    let index = super::inventory::private_case_index(nominal).expect("private Kaigi case");
    let cases = row.get("cases").and_then(Value::as_array).expect("cases");
    assert_eq!(
        cases.len(),
        2,
        "preserve both original public and private cases"
    );
    assert!(index < cases.len());
    let mut capture = cases[index]
        .as_object()
        .expect("private frame fields")
        .clone();
    for field in ["nominal", "serialize_hash", "deserialize_hash"] {
        capture.insert(
            field.to_owned(),
            row.get(field).expect("identity field").clone(),
        );
    }
    Value::Object(capture)
}

pub(super) fn check<T>(nominal: &str, private_case: usize)
where
    T: NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de> + Clone + Debug + PartialEq,
{
    assert_eq!(T::nominal_name(), nominal);
    assert_eq!(T::frame_name(), nominal);
    let row = captured(nominal);
    let current = private_capture(nominal);
    for row in [row, &current] {
        assert_eq!(
            row.get("serialize_hash").and_then(Value::as_str),
            Some(hex(&norito::schema::identity::frame_hash::<T>()).as_str())
        );
        assert_eq!(
            row.get("deserialize_hash").and_then(Value::as_str),
            Some(hex(&norito::schema::identity::frame_hash::<T>()).as_str())
        );
    }
    let cases = row
        .get("cases")
        .and_then(Value::as_array)
        .expect("original cases");
    assert_eq!(
        cases.len(),
        2,
        "preserve both original public and private cases"
    );
    assert!(private_case < cases.len());
    assert_eq!(
        super::inventory::private_case_index(nominal),
        Some(private_case)
    );
    for expected in cases {
        let bytes = unhex(
            expected
                .get("frame")
                .and_then(Value::as_str)
                .expect("current frame"),
        );
        let value: T = norito::decode_from_bytes(&bytes).expect("current Kaigi record");
        fixture_json::assert_json_matches(expected, &frame_fields(&capture(value)), nominal);
    }
    let bytes = unhex(
        current
            .get("frame")
            .and_then(Value::as_str)
            .expect("current private frame"),
    );
    let value: T = norito::decode_from_bytes(&bytes).expect("canonical private record");
    fixture_json::assert_json_matches(&current, &capture(value), nominal);
    rejected_scalars::check::<T>(nominal, private_case);
}

use crate::{
    account::AccountId,
    isi::kaigi::*,
    kaigi::{
        KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier, KaigiPrivacyMode,
        KaigiRelayHop, KaigiRelayManifest, KaigiRoomPolicy, NewKaigi,
        scalar::KaigiAuthorizationScalarV1,
    },
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_model_base::domain::DomainId;
use iroha_model_base::name::Name;
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
pub(super) fn current_values() -> Vec<Value> {
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
    assert_eq!(values.len(), 5);
    for actual in &values {
        let nominal = actual
            .get("nominal")
            .and_then(Value::as_str)
            .expect("nominal");
        fixture_json::assert_json_matches(
            &private_capture(nominal),
            actual,
            "populated canonical Kaigi record",
        );
    }
}
