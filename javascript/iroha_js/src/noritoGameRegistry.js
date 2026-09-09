/** Shared instruction identities and exact payload ceilings for native game codecs. */
export const EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1 = 4 * 1024 * 1024;
export const GAME_ADMISSION_MAX_ACCOUNT_ENCODED_BYTES_V1 = 32768;
export const GAME_ADMISSION_MAX_NFT_ENCODED_BYTES_V1 = 1024;
export const GAME_ADMISSION_MAX_BYTES_V1 = 1024 * 1024;
const EXECUTION_VALUE_NAMES_V1 = new Set(["ExecutionProofEnvelopeV1", "VerifyExecutionProofV1", "SettleGameSessionV1", "RaceProofPayloadV1"]);
/** Preserve the exact execution and admission payload corridors for every catalog. */
export function gameValueMaximumBytesV1(name) {
  if (name === "GameAdmissionCommitmentV1") return 1024 * 1024 + 64;
  return EXECUTION_VALUE_NAMES_V1.has(name) ? EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1 : 1024 * 1024;
}
export const GAME_INSTRUCTION_NAMES_V1 = Object.freeze([
  "OpenGameSessionV1", "JoinGameSessionV1", "StartGameSessionV1", "CommitGameCheckpointV1",
  "ChallengeGameSessionV1", "CommitGameInputsV1", "RevealGameInputsV1", "AdvanceGameDeadlineV1",
  "SettleGameSessionV1", "ExpireGameSessionV1", "ClaimGamePayoutV1", "StakeGameItemV1", "RegisterExecutionProofProfileV1", "VerifyExecutionProofV1",
]);
export const GAME_INSTRUCTION_WIRE_IDS_V1 = Object.freeze(
  GAME_INSTRUCTION_NAMES_V1.map((name) => `iroha.instruction.v1::game::${name}`),
);