//! Native game instruction closure and adversarial JSON/binary admission tests.

use super::*;
use crate::{
    decode_instruction_archive, decode_instruction_frame, encode_instruction_archive,
    encode_instruction_frame, instruction_from_json, value_to_instruction,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};

fn object<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn fixture(source: &str, name: &str) -> Value {
    let source: Value = json::from_json(source).expect("native fixture JSON");
    source
        .get("vectors")
        .and_then(Value::as_array)
        .expect("fixture vectors")
        .iter()
        .find(|row| row.get("name").and_then(Value::as_str) == Some(name))
        .expect("native fixture row")
        .clone()
}

fn game_row(name: &str) -> Value {
    fixture(
        include_str!("../../../javascript/iroha_js/test/fixtures/game-v1-codec.json"),
        name,
    )
}

fn game(name: &str) -> Value {
    game_row(name).get("value").expect("fixture value").clone()
}

fn resource_join() -> Value {
    fixture(
        include_str!("../../../javascript/iroha_js/test/fixtures/game-resource-v1-codec.json"),
        "JoinGameSessionV1",
    )
    .get("value")
    .expect("resource join value")
    .clone()
}

fn text(value: &Value) -> String {
    json::to_json(value).expect("fixture JSON")
}

fn field(value: &Value, name: &str) -> Value {
    value.get(name).expect("fixture field").clone()
}

fn examples() -> Vec<(&'static str, Value)> {
    let checkpoint = game("SignedGameCheckpointV1");
    let session = field(&field(&checkpoint, "checkpoint"), "session_id");
    let signatures = field(&checkpoint, "signatures");
    let signature = field(&signatures.as_array().expect("signatures")[0], "signature");
    let mut frontier = game("GameCommitmentSetBodyV1");
    frontier
        .as_object_mut()
        .unwrap()
        .insert("signatures".to_owned(), signatures);
    let commitment = field(&frontier, "commitments")
        .as_array()
        .expect("commitments")[0]
        .clone();
    let proof = game("ExecutionProofEnvelopeV1");
    vec![
        ("OpenGameSessionV1", game("OpenGameSessionV1")),
        ("JoinGameSessionV1", game("JoinGameSessionV1")),
        (
            "StartGameSessionV1",
            object([("session_id", session.clone())]),
        ),
        (
            "CommitGameCheckpointV1",
            object([
                ("session_id", session.clone()),
                ("checkpoint", checkpoint),
                ("frontier", frontier),
            ]),
        ),
        (
            "ChallengeGameSessionV1",
            object([
                ("session_id", session.clone()),
                ("epoch", Value::from(2_u64)),
                ("slot", Value::from(0_u8)),
                ("signature", signature.clone()),
            ]),
        ),
        (
            "CommitGameInputsV1",
            object([(
                "input",
                object([
                    ("session_id", session.clone()),
                    ("epoch", Value::from(2_u64)),
                    ("start_tick", Value::from(0_u32)),
                    ("slot", Value::from(0_u8)),
                    ("commitment", commitment),
                    ("signature", signature),
                ]),
            )]),
        ),
        (
            "RevealGameInputsV1",
            object([("reveal", game("GameInputRevealV1"))]),
        ),
        (
            "AdvanceGameDeadlineV1",
            object([("session_id", session.clone())]),
        ),
        (
            "SettleGameSessionV1",
            object([
                ("session_id", session.clone()),
                ("proof", proof.clone()),
                ("outcome", game("GameOutcomeV1")),
            ]),
        ),
        ("ExpireGameSessionV1", object([("session_id", session)])),
        ("ClaimGamePayoutV1", game("ClaimGamePayoutV1")),
        ("StakeGameItemV1", game("StakeGameItemV1")),
        (
            "RegisterExecutionProofProfileV1",
            object([("profile_id", field(&proof, "profile_id"))]),
        ),
        ("VerifyExecutionProofV1", object([("proof", proof)])),
    ]
}

fn assert_roundtrip(value: &Value) {
    let source = text(value);
    let frame = encode_instruction_frame(&source).expect("native instruction frame");
    let archive = encode_instruction_archive(&source).expect("native instruction archive");
    for decoded in [
        decode_instruction_frame(&frame),
        decode_instruction_archive(&archive),
    ] {
        let decoded = decoded.expect("native instruction decode");
        assert_eq!(
            json::from_json::<Value>(&decoded).expect("decoded JSON"),
            *value
        );
        assert_eq!(
            encode_instruction_frame(&decoded).expect("canonical frame"),
            frame
        );
        assert_eq!(
            encode_instruction_archive(&decoded).expect("canonical archive"),
            archive
        );
    }
    let native = instruction_from_json(&source).expect("typed native instruction");
    assert_eq!(
        frame,
        norito::encode_canonical(&native).expect("native frame oracle")
    );
    assert_eq!(archive, native.encode());
    let mut trailing = archive;
    trailing.push(0);
    assert!(decode_instruction_archive(&trailing).is_err());
}

fn rejects(name: &str, payload: Value) {
    rejects_envelope(&object([(name, payload)]));
}

fn rejects_envelope(input: &Value) {
    for encode in [encode_instruction_frame, encode_instruction_archive] {
        let error = encode(&text(input)).expect_err("closed native admission must reject");
        assert_eq!(error.kind(), CodecErrorKind::InvalidArgument, "{error}");
    }
}

#[test]
fn all_fourteen_game_instructions_roundtrip_native_frames_and_archives() {
    let examples = examples();
    assert_eq!(examples.len(), 14);
    for (name, payload) in examples {
        assert_roundtrip(&object([(name, payload)]));
    }
    assert_roundtrip(&object([("JoinGameSessionV1", resource_join())]));
}

#[test]
fn game_instruction_archives_retain_independent_native_inner_fixture_bytes() {
    for name in [
        "OpenGameSessionV1",
        "JoinGameSessionV1",
        "ClaimGamePayoutV1",
        "StakeGameItemV1",
    ] {
        let row = game_row(name);
        let archive = encode_instruction_archive(&text(&object([(name, field(&row, "value"))])))
            .expect("fixture instruction archive");
        let frame = hex::decode(
            row.get("framed_hex")
                .and_then(Value::as_str)
                .expect("native frame hex"),
        )
        .expect("native fixture frame");
        assert!(
            archive.windows(frame.len()).any(|window| window == frame),
            "{name}: native inner frame must remain exact"
        );
    }
}

#[test]
fn game_instructions_reject_incomplete_authorization_and_unknown_nested_fields() {
    for (name, payload) in examples() {
        let mut extra = payload.clone();
        extra
            .as_object_mut()
            .unwrap()
            .insert("extra".to_owned(), Value::Null);
        rejects(name, extra);
        rejects_envelope(&object([(name, payload), ("extra", Value::Null)]));
    }
    for field in [
        "resources",
        "invitation",
        "expected_manifest_hash",
        "expected_asset_definition",
        "expected_stake",
    ] {
        let mut join = game("JoinGameSessionV1");
        join.as_object_mut().unwrap().remove(field);
        rejects("JoinGameSessionV1", join);
    }
    let mut open = game("OpenGameSessionV1");
    open.as_object_mut()
        .unwrap()
        .get_mut("manifest")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("extra".to_owned(), Value::Null);
    rejects("OpenGameSessionV1", open);
    let mut envelope = game("ExecutionProofEnvelopeV1");
    envelope
        .as_object_mut()
        .unwrap()
        .get_mut("statement")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("extra".to_owned(), Value::Null);
    rejects("VerifyExecutionProofV1", object([("proof", envelope)]));
    let mut join = resource_join();
    join.as_object_mut()
        .unwrap()
        .get_mut("resources")
        .unwrap()
        .as_array_mut()
        .unwrap()[0]
        .as_object_mut()
        .unwrap()
        .insert("extra".to_owned(), Value::Null);
    rejects("JoinGameSessionV1", join);
}

#[test]
fn game_instructions_reject_invalid_versions_signatures_resources_and_byte_bounds() {
    let mut open: OpenGameSessionV1 =
        json::from_value(game("OpenGameSessionV1")).expect("typed open");
    open.manifest.version = 2;
    rejects("OpenGameSessionV1", json::to_value(&open).unwrap());
    open.manifest.version = 1;
    open.manifest
        .application_parameters
        .resize(MAX_OPAQUE_BYTES + 1, 0);
    rejects("OpenGameSessionV1", json::to_value(&open).unwrap());
    let (_, value) = examples()
        .into_iter()
        .find(|(name, _)| *name == "CommitGameCheckpointV1")
        .unwrap();
    let mut checkpoint: CommitGameCheckpointV1 = json::from_value(value).unwrap();
    checkpoint.checkpoint.signatures.reverse();
    rejects(
        "CommitGameCheckpointV1",
        json::to_value(&checkpoint).unwrap(),
    );
    checkpoint.checkpoint.signatures.reverse();
    checkpoint.frontier.as_mut().unwrap().signatures[1].slot = 0;
    rejects(
        "CommitGameCheckpointV1",
        json::to_value(&checkpoint).unwrap(),
    );
    let mut join: JoinGameSessionV1 = json::from_value(resource_join()).unwrap();
    join.resources.push(join.resources[0].clone());
    rejects("JoinGameSessionV1", json::to_value(&join).unwrap());
    let mut proof: ExecutionProofEnvelopeV1 =
        json::from_value(game("ExecutionProofEnvelopeV1")).unwrap();
    proof.version = 2;
    rejects(
        "VerifyExecutionProofV1",
        object([("proof", json::to_value(&proof).unwrap())]),
    );
    proof.version = 1;
    // The envelope, including its statement and lengths, must fit its owner bound.
    proof
        .proof_bytes
        .resize(EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1, 0);
    assert!(super::proof(&proof).is_err());
    assert!(signature(&Signature::from_bytes(&[0; 63])).is_err());
    assert!(signature(&Signature::from_bytes(&[0; 65])).is_err());
}

#[test]
fn game_binary_decode_rechecks_public_invariants_and_rejects_alternate_json_envelopes() {
    let mut open: OpenGameSessionV1 = json::from_value(game("OpenGameSessionV1")).unwrap();
    open.manifest.version = 2;
    let native: InstructionBox = open.into();
    let frame = norito::encode_canonical(&native).unwrap();
    assert_eq!(
        decode_instruction_frame(&frame).unwrap_err().kind(),
        CodecErrorKind::InvalidArgument
    );
    assert_eq!(
        decode_instruction_archive(&native.encode())
            .unwrap_err()
            .kind(),
        CodecErrorKind::InvalidArgument
    );
    let canonical = object([(
        "StartGameSessionV1",
        object([(
            "session_id",
            field(&game("OpenGameSessionV1"), "session_id"),
        )]),
    )]);
    let native = instruction_from_json(&text(&canonical)).unwrap();
    let alternative = Value::String(STANDARD.encode(norito::encode_canonical(&native).unwrap()));
    assert_ne!(alternative, canonical);
    assert!(value_to_instruction(alternative).is_err());
}

#[test]
fn cancel_upload_roundtrip_preserves_the_native_batch_instruction() {
    let native = iroha_data_model::isi::smart_contract_code::CancelSmartContractCodeUpload {
        code_hash: json::from_value(field(&game("OpenGameSessionV1"), "session_id")).unwrap(),
    };
    let value = object([(
        "CancelSmartContractCodeUpload",
        object([("code_hash", json::to_value(&native.code_hash).unwrap())]),
    )]);
    assert_roundtrip(&value);
    let mut extra = object([("code_hash", json::to_value(&native.code_hash).unwrap())]);
    extra
        .as_object_mut()
        .unwrap()
        .insert("extra".to_owned(), Value::Null);
    rejects("CancelSmartContractCodeUpload", extra);
    rejects("CancelSmartContractCodeUpload", object([]));
}

fn typed_game_value(name: &str, value: Value) -> InstructionBox {
    match name {
        "OpenGameSessionV1" => json::from_value::<OpenGameSessionV1>(value).unwrap().into(),
        "CommitGameCheckpointV1" => json::from_value::<CommitGameCheckpointV1>(value)
            .unwrap()
            .into(),
        "ChallengeGameSessionV1" => json::from_value::<ChallengeGameSessionV1>(value)
            .unwrap()
            .into(),
        "CommitGameInputsV1" => json::from_value::<CommitGameInputsV1>(value)
            .unwrap()
            .into(),
        "RevealGameInputsV1" => json::from_value::<RevealGameInputsV1>(value)
            .unwrap()
            .into(),
        _ => panic!("unexpected u64 game fixture {name}"),
    }
}

#[test]
fn game_u64_projections_preserve_typed_wire_at_every_sdk_boundary() {
    const SAFE: u64 = (1_u64 << 53) - 1;
    for (name, original) in examples() {
        let paths = u64_paths(name);
        if paths.is_empty() {
            continue;
        }
        for number in [0, 1, SAFE - 1, SAFE, SAFE + 1, u64::MAX] {
            let mut native_payload = original.clone();
            let mut expected_payload = original.clone();
            for path in paths {
                *field_at_path(&mut native_payload, path).expect("native fixture u64") =
                    Value::from(number);
                *field_at_path(&mut expected_payload, path).expect("SDK fixture u64") =
                    if number <= SAFE {
                        Value::from(number)
                    } else {
                        Value::String(number.to_string())
                    };
            }
            let native = typed_game_value(name, native_payload);
            let expected = object([(name, expected_payload)]);
            let source = text(&expected);
            assert_eq!(
                encode_instruction_frame(&source).unwrap(),
                norito::encode_canonical(&native).unwrap()
            );
            assert_eq!(
                encode_instruction_archive(&source).unwrap(),
                native.encode()
            );
            assert_roundtrip(&expected);
        }
        for path in paths {
            for rejected in [
                Value::String("0".to_owned()),
                Value::String("9007199254740991".to_owned()),
                Value::String("09007199254740992".to_owned()),
                Value::String("+9007199254740992".to_owned()),
                Value::String("9007199254740992 ".to_owned()),
                Value::String("18446744073709551616".to_owned()),
                Value::from(SAFE + 1),
                Value::from(u64::MAX),
            ] {
                let mut payload = original.clone();
                *field_at_path(&mut payload, path).expect("fixture u64") = rejected;
                rejects(name, payload);
            }
        }
    }
}

#[test]
fn empty_execution_proof_is_rejected_before_encode_and_after_native_decode() {
    let mut proof: ExecutionProofEnvelopeV1 =
        json::from_value(game("ExecutionProofEnvelopeV1")).unwrap();
    proof.proof_bytes.clear();
    let verify: InstructionBox = VerifyExecutionProofV1 {
        proof: proof.clone(),
    }
    .into();
    let session_id = proof.statement.session_id;
    let outcome = json::from_value(game("GameOutcomeV1")).unwrap();
    let settle: InstructionBox = SettleGameSessionV1 {
        session_id,
        proof,
        outcome,
    }
    .into();
    for instruction in [verify, settle] {
        let error = to_json(&instruction).expect("game variant").unwrap_err();
        assert!(error.to_string().contains("proof_bytes must not be empty"));
        for decoded in [
            decode_instruction_frame(&norito::encode_canonical(&instruction).unwrap()),
            decode_instruction_archive(&instruction.encode()),
        ] {
            assert!(
                decoded
                    .unwrap_err()
                    .to_string()
                    .contains("proof_bytes must not be empty")
            );
        }
    }
    let empty = json::to_value(&VerifyExecutionProofV1 {
        proof: {
            let mut proof: ExecutionProofEnvelopeV1 =
                json::from_value(game("ExecutionProofEnvelopeV1")).unwrap();
            proof.proof_bytes.clear();
            proof
        },
    })
    .unwrap();
    rejects("VerifyExecutionProofV1", empty);
}
