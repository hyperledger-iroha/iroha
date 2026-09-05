//! Independent Rust/browser compact-Norito parity for native race authorization.
#![cfg(feature = "json")]
use iroha_crypto::Hash;
use iroha_data_model::{
    NetworkId,
    execution_proofs::{ExecutionProofEnvelopeV1, RaceReplayV1},
    isi::race::{JoinRaceV1, OpenRaceV1},
    race::{RaceCheckpointV1, RaceInputRevealV1, SignedRaceCheckpointV1, race_message_hash_v1},
};
use norito::{
    codec::Encode,
    json::{self, JsonDeserialize, Value},
};

#[derive(norito::derive::JsonDeserialize)]
struct Fixtures {
    version: u16,
    network_id: NetworkId,
    vectors: Vec<Vector>,
}
#[derive(norito::derive::JsonDeserialize)]
struct Vector {
    name: String,
    value: Value,
    encoded_hex: String,
    domain: Option<String>,
    gameplay_digest_hex: Option<String>,
}
#[derive(norito::derive::JsonDeserialize)]
struct CommitmentBody {
    race_id: Hash,
    epoch: u64,
    start_tick: u32,
    parent_transcript_root: Hash,
    commitments: Vec<Hash>,
}
#[derive(norito::derive::JsonDeserialize)]
struct ChallengeBody {
    race_id: Hash,
    epoch: u64,
    slot: u8,
}

fn check_body<T: Encode>(body: &T, vector: &Vector, network: &NetworkId) {
    assert_eq!(
        hex::encode_upper(body.encode()),
        vector.encoded_hex,
        "{} bytes",
        vector.name
    );
    if let Some(domain) = &vector.domain {
        let digest = race_message_hash_v1(network, domain, body);
        assert_eq!(
            hex::encode_upper(digest.as_ref()),
            vector.gameplay_digest_hex.as_deref().unwrap(),
            "{} digest",
            vector.name
        );
        assert_ne!(
            digest,
            race_message_hash_v1(network, "different-domain", body),
            "signature domain separation"
        );
    }
}
fn check<T: Encode + JsonDeserialize>(vector: &Vector, network: &NetworkId) {
    let body: T = json::from_value(vector.value.clone()).expect(&vector.name);
    check_body(&body, vector, network);
}
#[test]
fn browser_race_v1_wire_and_gameplay_hash_vectors_match_native() {
    let fixtures: Fixtures = json::from_str(include_str!(
        "../../../javascript/iroha_js/test/fixtures/race-v1-codec.json"
    ))
    .expect("public browser fixtures");
    assert_eq!(fixtures.version, 1);
    assert_eq!(fixtures.vectors.len(), 9);
    for vector in fixtures.vectors {
        match vector.name.as_str() {
            "OpenRaceV1" => check::<OpenRaceV1>(&vector, &fixtures.network_id),
            "JoinRaceV1" => check::<JoinRaceV1>(&vector, &fixtures.network_id),
            "RaceCheckpointV1" => check::<RaceCheckpointV1>(&vector, &fixtures.network_id),
            "RaceInputRevealV1" => check::<RaceInputRevealV1>(&vector, &fixtures.network_id),
            "SignedRaceCheckpointV1" => {
                check::<SignedRaceCheckpointV1>(&vector, &fixtures.network_id)
            }
            "RaceReplayV1" => check::<RaceReplayV1>(&vector, &fixtures.network_id),
            "ExecutionProofEnvelopeV1" => {
                check::<ExecutionProofEnvelopeV1>(&vector, &fixtures.network_id)
            }
            "RaceCommitmentSetBodyV1" => {
                let b: CommitmentBody = json::from_value(vector.value.clone()).unwrap();
                check_body(
                    &(
                        b.race_id,
                        b.epoch,
                        b.start_tick,
                        b.parent_transcript_root,
                        b.commitments,
                    ),
                    &vector,
                    &fixtures.network_id,
                );
            }
            "RaceChallengeBodyV1" => {
                let b: ChallengeBody = json::from_value(vector.value.clone()).unwrap();
                check_body(&(b.race_id, b.epoch, b.slot), &vector, &fixtures.network_id);
            }
            other => panic!("unrecognized public fixture {other}"),
        }
    }
}
