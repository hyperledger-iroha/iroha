//! Closed native game instruction JSON admission over the canonical data model.
//!
//! This adapter checks wire shape and bounded public values. Core owns session,
//! signature, resource, proof-profile qualification and settlement authorization.

use iroha_crypto::{Algorithm, Signature};
use iroha_data_model::{
    execution_proofs::{EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1, ExecutionProofEnvelopeV1},
    game::{GameAccessV1, GameManifestV1, GameSlotSignatureV1},
    game_resources::{
        GAME_RESOURCE_MAX_SET_BYTES_V1, validate_game_nft_identity_v1, validate_resource_clauses_v1,
    },
    isi::{Instruction, InstructionBox, game::*},
};
use norito::{
    codec::Encode,
    json::{self, JsonDeserialize, JsonSerialize, Value},
};

use super::{CodecError, CodecErrorKind, CodecResult, json_u64};

// Existing native game value corridor: opaque fields are at most 512 KiB and
// fixed participant/signature vectors are at most 255 entries. These are wire
// limits; the selected relation and Core can require smaller exact shapes.
const MAX_OPAQUE_BYTES: usize = 512 * 1024;
const MAX_VECTOR_ITEMS: usize = 255;

fn invalid(message: impl Into<String>) -> CodecError {
    CodecError::new(CodecErrorKind::InvalidArgument, message)
}

fn bounded_bytes(bytes: &[u8], maximum: usize, label: &str) -> CodecResult<()> {
    if bytes.len() > maximum {
        return Err(invalid(format!("{label} exceeds its {maximum}-byte bound")));
    }
    Ok(())
}

fn signature(value: &Signature) -> CodecResult<()> {
    if value.payload().len() != 64 {
        return Err(invalid(
            "game signatures must contain exactly 64 Ed25519 bytes",
        ));
    }
    Ok(())
}

fn signatures(values: &[GameSlotSignatureV1]) -> CodecResult<()> {
    if values.len() > MAX_VECTOR_ITEMS {
        return Err(invalid("game signatures exceed the 255-item bound"));
    }
    if values.windows(2).any(|pair| pair[0].slot >= pair[1].slot) {
        return Err(invalid(
            "Race signatures must be unique and ordered by slot",
        ));
    }
    values
        .iter()
        .try_for_each(|value| signature(&value.signature))
}

fn manifest(value: &GameManifestV1) -> CodecResult<()> {
    if value.version != 1 {
        return Err(invalid("GameManifestV1.version must be 1"));
    }
    if value.max_participants == 0
        || value.batch_ticks == 0
        || value.max_ticks == 0
        || value.max_input_bytes == 0
    {
        return Err(invalid("Game manifest bounds must be positive"));
    }
    if let GameAccessV1::Invite(key) = &value.access {
        if key.algorithm() != Algorithm::Ed25519 {
            return Err(invalid("game invitation key must use Ed25519"));
        }
    }
    bounded_bytes(
        &value.application_parameters,
        MAX_OPAQUE_BYTES,
        "GameManifestV1.application_parameters",
    )
}

fn proof(value: &ExecutionProofEnvelopeV1) -> CodecResult<()> {
    if value.version != 1 {
        return Err(invalid("ExecutionProofEnvelopeV1.version must be 1"));
    }
    if value.proof_bytes.is_empty() {
        return Err(invalid(
            "ExecutionProofEnvelopeV1.proof_bytes must not be empty",
        ));
    }
    bounded_bytes(
        &value.proof_bytes,
        EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1,
        "ExecutionProofEnvelopeV1.proof_bytes",
    )?;
    bounded_bytes(
        &value.encode(),
        EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1,
        "ExecutionProofEnvelopeV1",
    )
}

fn open(value: &OpenGameSessionV1) -> CodecResult<()> {
    manifest(&value.manifest)
}

fn join(value: &JoinGameSessionV1) -> CodecResult<()> {
    if value.input_key.algorithm() != Algorithm::Ed25519 {
        return Err(invalid("JoinGameSessionV1.input_key must use Ed25519"));
    }
    bounded_bytes(
        &value.application_data,
        MAX_OPAQUE_BYTES,
        "JoinGameSessionV1.application_data",
    )?;
    validate_resource_clauses_v1(&value.resources).map_err(invalid)?;
    value.invitation.as_ref().map_or(Ok(()), signature)
}

fn checkpoint(value: &CommitGameCheckpointV1) -> CodecResult<()> {
    signatures(&value.checkpoint.signatures)?;
    if let Some(frontier) = &value.frontier {
        if frontier.commitments.len() > MAX_VECTOR_ITEMS {
            return Err(invalid(
                "GameCommitmentSetV1.commitments exceeds the 255-item bound",
            ));
        }
        signatures(&frontier.signatures)?;
    }
    Ok(())
}

fn challenge(value: &ChallengeGameSessionV1) -> CodecResult<()> {
    signature(&value.signature)
}

fn commit_inputs(value: &CommitGameInputsV1) -> CodecResult<()> {
    signature(&value.input.signature)
}

fn reveal_inputs(value: &RevealGameInputsV1) -> CodecResult<()> {
    bounded_bytes(
        &value.reveal.payload,
        MAX_OPAQUE_BYTES,
        "GameInputRevealV1.payload",
    )
}

fn settle(value: &SettleGameSessionV1) -> CodecResult<()> {
    proof(&value.proof)?;
    if value.outcome.winner_slots.len() > MAX_VECTOR_ITEMS {
        return Err(invalid(
            "GameOutcomeV1.winner_slots exceeds the 255-item bound",
        ));
    }
    if value
        .outcome
        .winner_slots
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid(
            "GameOutcomeV1.winner_slots must be unique and ordered",
        ));
    }
    bounded_bytes(
        &value.outcome.result,
        MAX_OPAQUE_BYTES,
        "GameOutcomeV1.result",
    )
}

fn stake(value: &StakeGameItemV1) -> CodecResult<()> {
    validate_game_nft_identity_v1(&value.nft_id).map_err(invalid)
}

fn verify(value: &VerifyExecutionProofV1) -> CodecResult<()> {
    proof(&value.proof)
}

fn no_additional_wire_fields<T>(_: &T) -> CodecResult<()> {
    Ok(())
}

// Only declared u64 fields cross the SDK projection; opaque payloads and hashes
// retain their model-owned values. Missing fields and null non-optionals still
// fail the complete typed record check in `parse`.
fn u64_paths(name: &str) -> &'static [&'static [&'static str]] {
    match name {
        "OpenGameSessionV1" => &[&["join_deadline_height"]],
        "CommitGameCheckpointV1" => &[
            &["checkpoint", "checkpoint", "epoch"],
            &["frontier", "epoch"],
        ],
        "ChallengeGameSessionV1" => &[&["epoch"]],
        "CommitGameInputsV1" => &[&["input", "epoch"]],
        "RevealGameInputsV1" => &[&["reveal", "epoch"]],
        _ => &[],
    }
}

fn field_at_path<'a>(value: &'a mut Value, path: &[&str]) -> Option<&'a mut Value> {
    match path.split_first() {
        Some((key, rest)) => field_at_path(value.as_object_mut()?.get_mut(*key)?, rest),
        None => Some(value),
    }
}

fn project_u64_fields(value: &mut Value, name: &str, to_native: bool) -> CodecResult<()> {
    for path in u64_paths(name) {
        if let Some(field) = field_at_path(value, path) {
            let label = format!("{name}.{}", path.join("."));
            *field = if to_native {
                Value::Number(json_u64::parse_u64(field.clone(), &label)?.into())
            } else {
                json_u64::u64_json(
                    field
                        .as_u64()
                        .ok_or_else(|| invalid(format!("{label} must be native u64")))?,
                )
            };
        }
    }
    Ok(())
}

fn value<T: JsonSerialize + Encode>(
    instruction: &T,
    name: &str,
    maximum_bytes: usize,
    validate: fn(&T) -> CodecResult<()>,
) -> CodecResult<Value> {
    validate(instruction)?;
    bounded_bytes(&instruction.encode(), maximum_bytes, name)?;
    let mut value =
        json::to_value(instruction).map_err(|error| invalid(format!("{name}: {error}")))?;
    project_u64_fields(&mut value, name, false)?;
    Ok(value)
}

fn parse<T: JsonDeserialize + JsonSerialize + Encode>(
    payload: &Value,
    name: &str,
    maximum_bytes: usize,
    validate: fn(&T) -> CodecResult<()>,
) -> CodecResult<T> {
    let mut native_payload = payload.clone();
    project_u64_fields(&mut native_payload, name, true)?;
    let instruction: T =
        json::from_value(native_payload).map_err(|error| invalid(format!("{name}: {error}")))?;
    // Typed deserialization alone can default optional fields and ignore nested
    // fields in types shared with other protocols. The first-release adapter
    // admits only the complete canonical model value, never those alternatives.
    if value(&instruction, name, maximum_bytes, validate)? != *payload {
        return Err(invalid(format!(
            "{name} has missing or unknown fields or noncanonical values"
        )));
    }
    Ok(instruction)
}

macro_rules! instruction_catalog {
    ($($ty:ident => ($validate:ident, $maximum:expr)),+ $(,)?) => {
        pub(super) fn from_json(input: &Value) -> Option<CodecResult<InstructionBox>> {
            let Value::Object(fields) = input else { return None; };
            let name = fields.keys().find(|name| matches!(name.as_str(), $(stringify!($ty))|+))?;
            if fields.len() != 1 {
                return Some(Err(invalid("game instruction envelope must contain exactly one variant")));
            }
            let payload = &fields[name];
            Some(match name.as_str() {
                $(stringify!($ty) => parse::<$ty>(payload, name, $maximum, $validate).map(Into::into),)+
                _ => Err(invalid("unknown game instruction")),
            })
        }

        pub(super) fn is_game_instruction(instruction: &InstructionBox) -> bool {
            let instruction: &dyn Instruction = &**instruction;
            $(instruction.as_any().is::<$ty>())||+
        }

        pub(super) fn to_json(instruction: &InstructionBox) -> Option<CodecResult<Value>> {
            let instruction: &dyn Instruction = &**instruction;
            $(if let Some(typed) = instruction.as_any().downcast_ref::<$ty>() {
                return Some(value(typed, stringify!($ty), $maximum, $validate).map(|payload| {
                    Value::Object([(stringify!($ty).to_owned(), payload)].into_iter().collect())
                }));
            })+
            None
        }
    };
}

instruction_catalog! {
    OpenGameSessionV1 => (open, GAME_RESOURCE_MAX_SET_BYTES_V1),
    JoinGameSessionV1 => (join, GAME_RESOURCE_MAX_SET_BYTES_V1),
    StartGameSessionV1 => (no_additional_wire_fields, GAME_RESOURCE_MAX_SET_BYTES_V1),
    CommitGameCheckpointV1 => (checkpoint, GAME_RESOURCE_MAX_SET_BYTES_V1),
    ChallengeGameSessionV1 => (challenge, GAME_RESOURCE_MAX_SET_BYTES_V1),
    CommitGameInputsV1 => (commit_inputs, GAME_RESOURCE_MAX_SET_BYTES_V1),
    RevealGameInputsV1 => (reveal_inputs, GAME_RESOURCE_MAX_SET_BYTES_V1),
    AdvanceGameDeadlineV1 => (no_additional_wire_fields, GAME_RESOURCE_MAX_SET_BYTES_V1),
    SettleGameSessionV1 => (settle, EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1),
    ExpireGameSessionV1 => (no_additional_wire_fields, GAME_RESOURCE_MAX_SET_BYTES_V1),
    ClaimGamePayoutV1 => (no_additional_wire_fields, GAME_RESOURCE_MAX_SET_BYTES_V1),
    StakeGameItemV1 => (stake, GAME_RESOURCE_MAX_SET_BYTES_V1),
    RegisterExecutionProofProfileV1 => (no_additional_wire_fields, GAME_RESOURCE_MAX_SET_BYTES_V1),
    VerifyExecutionProofV1 => (verify, EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1),
}

#[cfg(test)]
#[path = "game_instruction_tests.rs"]
mod tests;
