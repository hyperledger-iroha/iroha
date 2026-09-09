/** Complete public game and application value catalog over the shared exact codec engine. */
import { GAME_INSTRUCTION_SCHEMAS_V1, GAME_INSTRUCTION_VECTORS_V1 } from './noritoGameInstructionCodecs.js';
import { createNoritoGameEngine } from "./noritoGameEngine.js";
import { CLASSED_RACE_SCHEMAS_V1, CLASSED_RACE_VECTORS_V1, CLASSED_RACE_CLASSES_V1,
  CLASSED_RACE_TRACKS_V1, validateClassedRaceScalarsV1, validateClassedRaceRecordV1 } from "./noritoClassedRaceSchemas.js";
export * from './noritoGameRegistry.js';
const fields = (text) => text.split(" ").map((entry) => entry.split(":"));
const SCHEMAS = Object.freeze({
  ...GAME_INSTRUCTION_SCHEMAS_V1,
  ...CLASSED_RACE_SCHEMAS_V1,
  RaceRulesV1: fields("version:u16 track:track max_racers:u8"),
  RaceCheckpointV1: fields("race_id:hash epoch:u64 tick:u32 transcript_root:hash state_root:hash terminal:bool"),
  RaceSlotSignatureV1: fields("slot:u8 signature:signature"),
  SignedRaceCheckpointV1: fields("checkpoint:RaceCheckpointV1 signatures:signatures"),
  RaceCommitmentSetBodyV1: fields("race_id:hash epoch:u64 start_tick:u32 parent_transcript_root:hash commitments:hashes"),
  RaceCommitmentSetV1: fields("race_id:hash epoch:u64 start_tick:u32 parent_transcript_root:hash commitments:hashes signatures:signatures"),
  RaceInputCommitmentBodyV1: fields("race_id:hash epoch:u64 start_tick:u32 slot:u8 commitment:hash"),
  RaceInputCommitmentV1: fields("race_id:hash epoch:u64 start_tick:u32 slot:u8 commitment:hash signature:signature"),
  RaceInputRevealV1: fields("race_id:hash epoch:u64 start_tick:u32 slot:u8 controls:controls salt:hash"),
  RaceChallengeBodyV1: fields("race_id:hash epoch:u64 slot:u8"),
  RaceInputFrameV1: fields("tick:u32 controls:frameControls"),
  RaceDnfEventV1: fields("tick:u32 slots:slots"),
  RaceReplayV1: fields("track:track player_count:u8 frames:frames dnf_events:dnfEvents"),
  RaceCarStateV1: fields("progress_mm:i64 lateral_mm:i32 speed_mm_per_tick:i32 lateral_velocity_mm_per_tick:i32 boost_energy:u16 finish_tick:optionalU32 dnf_tick:optionalU32"),
  RaceStateV1: fields("tick:u32 track:track cars:cars"),
  RaceStandingV1: fields("slot:u8 finish_tick:optionalU32 dnf_tick:optionalU32 progress_mm:i64"),
  RaceResultV1: fields("ticks:u32 standings:standings winners:slots"),
  RaceProverRequestV1: fields("statement:ExecutionPublicInputsV1 manifest:GameManifestV1 admission:GameAdmissionBodyV1 replay:RaceReplayV1 checkpoint_state:optionalRaceState"),
  RaceProofPayloadV1: fields("manifest:GameManifestV1 admission:GameAdmissionBodyV1 outcome:GameOutcomeV1 relation_inputs:RacePublicInputsV1 replay:RaceReplayV1 final_state:RaceStateV1 checkpoint_state:optionalRaceState stark_bytes:stark"),
  RacePublicInputsV1: fields("network_id:hash race_id:hash roster_hash:hash rules_hash:hash track:track transcript_root:hash dispute_root:hash result:RaceResultV1"),
  GameCommitmentSetBodyV1: fields("session_id:hash epoch:u64 start_tick:u32 parent_transcript_root:hash commitments:gameHashes"),
  GameInputCommitmentBodyV1: fields("session_id:hash epoch:u64 start_tick:u32 slot:u8 commitment:hash"),
  GameChallengeBodyV1: fields("session_id:hash epoch:u64 slot:u8"),
  GameInvitationBodyV1: fields("session_id:hash wallet:account input_key:key application_data:opaque"),
  GameTranscriptBatchV1: fields("start_tick:u32 inputs:gameInputs"),
  GameDnfEventV1: fields("tick:u32 slots:gameSlots"),
  GameTranscriptV1: fields("batches:gameBatches dnf_events:gameDnfEvents"),
  GameParticipantV1: fields("account:account input_key:key application_data:opaque dnf_at_tick:optionalU32"),
  GameItemStakeV1: fields("slot:u8 nft_id:nft custody:account metadata_hash:hash recipient:optionalAccount claimed:bool"),
  GameAdmissionParticipantV1: fields("account:admissionAccount input_key:admissionKey application_data:admissionData"),
  GameAdmissionWagerV1: fields("slot:u8 nft_id:admissionNft metadata_hash:hash"),
  GameAdmissionResourceV1: fields("slot:u8 nft_id:admissionNft metadata_hash:hash role_id:hash policy:GameResourceReturnPolicyV1"),
  GameAdmissionBodyV1: fields("version:u16 participants:admissionParticipants wagers:admissionWagers resources:admissionResources"),
  GameAdmissionCommitmentV1: fields("session_id:hash admission:GameAdmissionBodyV1"),
  GameSessionEventV1: fields("session_id:hash revision:u64 phase:u8 dispute_root:hash payout_claims:gameClaims item_stakes:gameItems resources:records terminal_at_height:optionalU64"),
  GamePayoutClaimV1: fields("slot:u8 amount:quantity remaining:quantity")
});
const VECTORS = Object.freeze({
  ...GAME_INSTRUCTION_VECTORS_V1,
  ...CLASSED_RACE_VECTORS_V1,
  gameClaims: ["GamePayoutClaimV1", 32],
  admissionParticipants: ["GameAdmissionParticipantV1", 32],
  admissionWagers: ["GameAdmissionWagerV1", 32],
  admissionResources: ["GameAdmissionResourceV1", 128],
  gameParticipants: ["GameParticipantV1", 32],
  gameItems: ["GameItemStakeV1", 32],
  gameInputs: ["opaque", 255],
  gameBatches: ["GameTranscriptBatchV1", 65535],
  gameDnfEvents: ["GameDnfEventV1", 255],
  signatures: ["RaceSlotSignatureV1", 8],
  hashes: ["hash", 8],
  controls: ["control", 6],
  frameControls: ["control", 8],
  frames: ["RaceInputFrameV1", 5400],
  standings: ["RaceStandingV1", 8],
  slots: ["slot", 8],
  cars: ["RaceCarStateV1", 8],
  dnfEvents: ["RaceDnfEventV1", 8]
});
const TRACKS = Object.freeze(["neon_tokyo", "harbor", "sakura"]);
function validateAdmission(value) {
  if (Number(value.version) !== 1) throw new TypeError("admission version must be one");
  const accounts = new Set(), keys = new Set(), nfts = new Set(), counts = new Map();
  for (const participant of value.participants) {
    if (accounts.has(participant.account) || keys.has(participant.input_key)) throw new TypeError("admission participants require unique wallets and input keys");
    accounts.add(participant.account); keys.add(participant.input_key);
  }
  let priorSlot = -1, priorRole;
  const item = entry => {
    if (Number(entry.slot) >= value.participants.length || nfts.has(entry.nft_id)) throw new TypeError("admission item requires an existing slot and a unique NFT");
    nfts.add(entry.nft_id);
  };
  for (const wager of value.wagers) {
    item(wager);
    if (Number(wager.slot) <= priorSlot) throw new TypeError("admission wagers must be strictly ordered by slot");
    priorSlot = Number(wager.slot);
  }
  priorSlot = -1;
  for (const resource of value.resources) {
    item(resource);
    const slot = Number(resource.slot), role = resource.role_id.slice(5, 69);
    if (slot < priorSlot || slot === priorSlot && role <= priorRole) throw new TypeError("admission resources must be strictly ordered by slot and role");
    counts.set(slot, (counts.get(slot) ?? 0) + 1);
    if (counts.get(slot) > 4) throw new TypeError("admission exceeds four resources per participant");
    priorSlot = slot; priorRole = role;
  }
}
function validateRecord(name, value) {
  validateClassedRaceRecordV1(name, value);
  if (name === "GameAdmissionBodyV1") validateAdmission(value);

  if ((name === "RaceRulesV1" || name === "GameManifestV1" || name === "ExecutionProofEnvelopeV1") && Number(value.version) !== 1) throw new RangeError(`${name}.version must be 1`);
  if (name === "RaceRulesV1" && (Number(value.max_racers) < 2 || Number(value.max_racers) > 8)) throw new RangeError("RaceRulesV1.max_racers must be 2..8");
  if (name === "GameManifestV1" && (Number(value.max_participants) === 0 || Number(value.batch_ticks) === 0 || Number(value.max_ticks) === 0 || Number(value.max_input_bytes) === 0)) throw new RangeError("Game manifest bounds must be positive");
  if (name.startsWith("Race") && Object.hasOwn(value, "slot") && Number(value.slot) > 7) throw new RangeError(`${name}.slot must be 0..7`);
  for (const field of ["tick", "ticks", "start_tick"]) if (name.startsWith("Race") && Object.hasOwn(value, field) && Number(value[field]) > 5400) throw new RangeError(`${name}.${field} exceeds the race tick limit`);
  if (name === "RaceInputRevealV1" && value.controls.length !== 6) throw new RangeError("Race input reveals require exactly six controls");
  if (name === "RaceReplayV1" && (Number(value.player_count) < 1 || Number(value.player_count) > 8 || value.frames.some((frame, index) => Number(frame.tick) !== index || frame.controls.length !== Number(value.player_count)))) throw new RangeError("Race replay frames must be consecutive and match the roster size");
  if (Object.hasOwn(value, "signatures") && value.signatures.some((item, index, all) => index > 0 && Number(item.slot) <= Number(all[index - 1].slot))) throw new RangeError("Race signatures must be unique and ordered by slot");
}
/** Build the complete public value catalog, including the compiled Race adapters. */
export function createNoritoGameCodecs(h) {
  return createNoritoGameEngine(h, {
    SCHEMAS, VECTORS, validateRecord,
    validateScalars: validateClassedRaceScalarsV1,
    CLASSED_RACE_CLASSES_V1, CLASSED_RACE_TRACKS_V1, TRACKS,
  });
}
