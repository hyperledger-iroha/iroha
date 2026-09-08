//! Explicit wire qualification using an actual retained complete native race proof.
#![cfg(feature = "json")]
use iroha_data_model::{
    execution_proofs::{ExecutionProofEnvelopeV1, RaceProofPayloadV1},
    game::{game_message_hash_v1, game_roster_hash_v1},
};
use norito::{
    codec::{Decode, Encode},
    json,
};
// This checks the actual proof corpus and codec layout. Cryptographic verification is a
// separate native qualification gate; this test cannot establish proof soundness.

#[test]
#[ignore = "explicit native/browser qualification: set SORA_CARS_FULL_PROOF_FIXTURE to the genuine full-race envelope"]
fn genuine_full_race_payload_roundtrips_native_binary_json_and_framed_layouts() {
    let path = std::env::var_os("SORA_CARS_FULL_PROOF_FIXTURE")
        .expect("the explicit gate requires an actual retained full-race proof path");
    let bytes = std::fs::read(path).expect("read retained native full proof");
    assert!(
        bytes.len() > 1024 * 1024,
        "must exercise the large-proof corridor"
    );
    assert!(
        bytes.len() <= iroha_data_model::execution_proofs::EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1
    );
    let mut input = bytes.as_slice();
    let envelope = ExecutionProofEnvelopeV1::decode(&mut input).expect("native envelope decode");
    assert!(input.is_empty());
    assert_eq!(envelope.encode(), bytes, "canonical native envelope bytes");
    let mut input = envelope.proof_bytes.as_slice();
    let payload = RaceProofPayloadV1::decode(&mut input).expect("native execution payload decode");
    assert!(input.is_empty());
    assert_eq!(payload.encode(), envelope.proof_bytes);
    assert_eq!(payload.replay.player_count, 8);
    payload
        .admission
        .validate()
        .expect("canonical immutable admission");
    assert_eq!(payload.admission.participants.len(), 8);
    assert_eq!(
        game_roster_hash_v1(
            &envelope.statement.network_id,
            &envelope.statement.session_id,
            &payload.admission,
        ),
        envelope.statement.roster_hash
    );
    assert_eq!(payload.replay.frames.len(), 5400);
    assert_eq!(payload.outcome.terminal_tick, 5400);
    assert!(
        !payload.outcome.winner_slots.is_empty(),
        "empty lists miss packed-byte regressions"
    );
    assert_eq!(
        payload.outcome.winner_slots,
        payload.relation_inputs.result.winners
    );
    assert_eq!(
        payload.outcome.result,
        payload.relation_inputs.result.encode()
    );
    assert_eq!(
        game_message_hash_v1(
            &envelope.statement.network_id,
            "session-outcome",
            &payload.outcome
        ),
        envelope.statement.outcome_hash,
    );
    let framed = norito::to_bytes(&payload).expect("framed payload encode");
    assert_eq!(
        norito::decode_from_bytes::<RaceProofPayloadV1>(&framed).unwrap(),
        payload
    );
    let payload_json = json::to_json(&payload).expect("native payload JSON");
    assert_eq!(
        json::from_str::<RaceProofPayloadV1>(&payload_json).unwrap(),
        payload
    );
    eprintln!(
        "native full payload codec gate: envelope={}, payload={}, framed={}, JSON={}, winners={:?}",
        bytes.len(),
        envelope.proof_bytes.len(),
        framed.len(),
        payload_json.len(),
        payload.outcome.winner_slots
    );
}
