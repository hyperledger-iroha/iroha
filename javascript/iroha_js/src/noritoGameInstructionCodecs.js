/** Closed native instruction catalog; application-specific Race values have a separate owner. */
import { createNoritoGameEngine } from "./noritoGameEngine.js";

const TEXT_SESSION_ID_HASH = "session_id:hash ";

const fields = (text) => text.split(" ").map((entry) => entry.split(":"));
/** Exact record closure shared by native instructions and public value codecs. */
export const GAME_INSTRUCTION_SCHEMAS_V1 = Object.freeze({
  GameManifestV1: fields("version:u16 application_id:hash profile_id:hash application_parameters:opaque min_participants:u8 max_participants:u8 batch_ticks:u16 max_ticks:u32 max_input_bytes:u16 max_participant_data_bytes:u16 access:access payout_policy:payout"),
  GameCheckpointV1: fields((TEXT_SESSION_ID_HASH + "epoch:u64 tick:u32 transcript_root:hash state_root:hash terminal:bool")),
  GameSlotSignatureV1: fields("slot:u8 signature:signature"),
  SignedGameCheckpointV1: fields("checkpoint:GameCheckpointV1 signatures:gameSignatures"),
  GameCommitmentSetV1: fields((TEXT_SESSION_ID_HASH + "epoch:u64 start_tick:u32 parent_transcript_root:hash commitments:gameHashes signatures:gameSignatures")),
  GameInputCommitmentV1: fields((TEXT_SESSION_ID_HASH + "epoch:u64 start_tick:u32 slot:u8 commitment:hash signature:signature")),
  GameInputRevealV1: fields((TEXT_SESSION_ID_HASH + "epoch:u64 start_tick:u32 slot:u8 payload:opaque salt:hash")),
  GameOutcomeV1: fields("terminal_tick:u32 winner_slots:gameSlots result:opaque"),
  ExecutionPublicInputsV1: fields(("network_id:hash " + TEXT_SESSION_ID_HASH + "manifest_hash:hash roster_hash:hash transcript_root:hash dispute_root:hash outcome_hash:hash")),
  ExecutionProofEnvelopeV1: fields("version:u16 profile_id:hash statement:ExecutionPublicInputsV1 proof_bytes:proof"),
  OpenGameSessionV1: fields((TEXT_SESSION_ID_HASH + "manifest:GameManifestV1 asset_definition:asset stake:quantity join_deadline_height:u64")),
  JoinGameSessionV1: fields((TEXT_SESSION_ID_HASH + "input_key:key application_data:opaque resources:clauses invitation:optionalSignature expected_manifest_hash:hash expected_asset_definition:asset expected_stake:quantity")),
  StartGameSessionV1: fields("session_id:hash"),
  CommitGameCheckpointV1: fields((TEXT_SESSION_ID_HASH + "checkpoint:SignedGameCheckpointV1 frontier:optionalGameFrontier")),
  ChallengeGameSessionV1: fields((TEXT_SESSION_ID_HASH + "epoch:u64 slot:u8 signature:signature")),
  CommitGameInputsV1: fields("input:GameInputCommitmentV1"),
  RevealGameInputsV1: fields("reveal:GameInputRevealV1"),
  AdvanceGameDeadlineV1: fields("session_id:hash"),
  SettleGameSessionV1: fields((TEXT_SESSION_ID_HASH + "proof:ExecutionProofEnvelopeV1 outcome:GameOutcomeV1")),
  ExpireGameSessionV1: fields("session_id:hash"),
  StakeGameItemV1: fields((TEXT_SESSION_ID_HASH + "nft_id:nft expected_manifest_hash:hash")),
  ClaimGamePayoutV1: fields((TEXT_SESSION_ID_HASH + "slot:u8 destination:account amount:quantity")),
  RegisterExecutionProofProfileV1: fields("profile_id:hash"),
  VerifyExecutionProofV1: fields("proof:ExecutionProofEnvelopeV1"),
});
/** Exact vector closure and existing native element ceilings. */
export const GAME_INSTRUCTION_VECTORS_V1 = Object.freeze({
  gameSignatures: ["GameSlotSignatureV1", 255],
  gameHashes: ["hash", 255],
  gameSlots: ["u8", 255],
});
function validateRecord(name, value) {
  if ((name === "GameManifestV1" || name === "ExecutionProofEnvelopeV1") && Number(value.version) !== 1) {
    throw new RangeError(`${name}.version must be 1`);
  }
  if (name === "GameManifestV1" && (Number(value.max_participants) === 0
      || Number(value.batch_ticks) === 0 || Number(value.max_ticks) === 0
      || Number(value.max_input_bytes) === 0)) {
    throw new RangeError("Game manifest bounds must be positive");
  }
  if (Object.hasOwn(value, "signatures") && value.signatures.some((item, index, all) =>
    index > 0 && Number(item.slot) <= Number(all[index - 1].slot))) {
    throw new RangeError("Race signatures must be unique and ordered by slot");
  }
}
// Instruction scalar widths and bounds are enforced by the shared primitive engine.
// Application-specific scalar checks belong only to the public value catalog.
function validateInstructionScalars() {}

/** Build only the native instruction closure from the shared canonical primitives. */
export function createNoritoGameInstructionCodecs(h) {
  return createNoritoGameEngine(h, {
    SCHEMAS: GAME_INSTRUCTION_SCHEMAS_V1,
    VECTORS: GAME_INSTRUCTION_VECTORS_V1,
    validateRecord,
    validateScalars: validateInstructionScalars,
  });
}
