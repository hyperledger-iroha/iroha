import { Buffer } from "buffer";
import { GAME_RESOURCE_VALUE_NAMES_V1, encodeGameResourceValueV1, decodeGameResourceValueV1 } from "./noritoGameResourceCodecs.js";
import { CLASSED_RACE_SCHEMAS_V1, CLASSED_RACE_VECTORS_V1, CLASSED_RACE_CLASSES_V1,
  CLASSED_RACE_TRACKS_V1, validateClassedRaceScalarsV1, validateClassedRaceRecordV1 } from "./noritoClassedRaceSchemas.js";

/** Canonical generic game registry with compiled application value adapters. */
export const EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1 = 4 * 1024 * 1024;
export const GAME_ADMISSION_MAX_ACCOUNT_ENCODED_BYTES_V1 = 32768;
export const GAME_ADMISSION_MAX_NFT_ENCODED_BYTES_V1 = 1024;
export const GAME_ADMISSION_MAX_BYTES_V1 = 1024 * 1024;
const RACE_MAX_STARK_BYTES_V1 = 3_250_000;
const EXECUTION_VALUE_NAMES_V1 = new Set(["ExecutionProofEnvelopeV1", "VerifyExecutionProofV1", "SettleGameSessionV1", "RaceProofPayloadV1"]);
/** Execution-only payload corridor; unrelated native values retain the ordinary bound. */
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
const TRACKS = Object.freeze(["neon_tokyo", "harbor", "sakura"]);
const fields = (text) => text.split(" ").map((entry) => entry.split(":"));
const SCHEMAS = Object.freeze({
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
  GameManifestV1: fields("version:u16 application_id:hash profile_id:hash application_parameters:opaque min_participants:u8 max_participants:u8 batch_ticks:u16 max_ticks:u32 max_input_bytes:u16 max_participant_data_bytes:u16 access:access payout_policy:payout"),
  GameCheckpointV1: fields("session_id:hash epoch:u64 tick:u32 transcript_root:hash state_root:hash terminal:bool"),
  GameSlotSignatureV1: fields("slot:u8 signature:signature"),
  SignedGameCheckpointV1: fields("checkpoint:GameCheckpointV1 signatures:gameSignatures"),
  GameCommitmentSetBodyV1: fields("session_id:hash epoch:u64 start_tick:u32 parent_transcript_root:hash commitments:gameHashes"),
  GameCommitmentSetV1: fields("session_id:hash epoch:u64 start_tick:u32 parent_transcript_root:hash commitments:gameHashes signatures:gameSignatures"),
  GameInputCommitmentBodyV1: fields("session_id:hash epoch:u64 start_tick:u32 slot:u8 commitment:hash"),
  GameInputCommitmentV1: fields("session_id:hash epoch:u64 start_tick:u32 slot:u8 commitment:hash signature:signature"),
  GameInputRevealV1: fields("session_id:hash epoch:u64 start_tick:u32 slot:u8 payload:opaque salt:hash"),
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
  GamePayoutClaimV1: fields("slot:u8 amount:quantity remaining:quantity"),
  GameOutcomeV1: fields("terminal_tick:u32 winner_slots:gameSlots result:opaque"),
  ExecutionPublicInputsV1: fields("network_id:hash session_id:hash manifest_hash:hash roster_hash:hash transcript_root:hash dispute_root:hash outcome_hash:hash"),
  ExecutionProofEnvelopeV1: fields("version:u16 profile_id:hash statement:ExecutionPublicInputsV1 proof_bytes:proof"),
  OpenGameSessionV1: fields("session_id:hash manifest:GameManifestV1 asset_definition:asset stake:quantity join_deadline_height:u64"),
  JoinGameSessionV1: fields("session_id:hash input_key:key application_data:opaque resources:clauses invitation:optionalSignature expected_manifest_hash:hash expected_asset_definition:asset expected_stake:quantity"),
  StartGameSessionV1: fields("session_id:hash"),
  CommitGameCheckpointV1: fields("session_id:hash checkpoint:SignedGameCheckpointV1 frontier:optionalGameFrontier"),
  ChallengeGameSessionV1: fields("session_id:hash epoch:u64 slot:u8 signature:signature"),
  CommitGameInputsV1: fields("input:GameInputCommitmentV1"),
  RevealGameInputsV1: fields("reveal:GameInputRevealV1"),
  AdvanceGameDeadlineV1: fields("session_id:hash"),
  SettleGameSessionV1: fields("session_id:hash proof:ExecutionProofEnvelopeV1 outcome:GameOutcomeV1"),
  ExpireGameSessionV1: fields("session_id:hash"),
  StakeGameItemV1: fields("session_id:hash nft_id:nft expected_manifest_hash:hash"),
  ClaimGamePayoutV1: fields("session_id:hash slot:u8 destination:account amount:quantity"),
  RegisterExecutionProofProfileV1: fields("profile_id:hash"),
  VerifyExecutionProofV1: fields("proof:ExecutionProofEnvelopeV1"),
});
const VECTORS = Object.freeze({
  ...CLASSED_RACE_VECTORS_V1,
  gameClaims: ["GamePayoutClaimV1", 32],
  admissionParticipants: ["GameAdmissionParticipantV1", 32], admissionWagers: ["GameAdmissionWagerV1", 32], admissionResources: ["GameAdmissionResourceV1", 128],
  gameParticipants: ["GameParticipantV1", 32], gameItems: ["GameItemStakeV1", 32],
  gameSignatures: ["GameSlotSignatureV1", 255], gameHashes: ["hash", 255], gameInputs: ["opaque", 255],
  gameBatches: ["GameTranscriptBatchV1", 65535], gameDnfEvents: ["GameDnfEventV1", 255], gameSlots: ["u8", 255],
  signatures: ["RaceSlotSignatureV1", 8], hashes: ["hash", 8], controls: ["control", 6],
  frameControls: ["control", 8], frames: ["RaceInputFrameV1", 5400],
  standings: ["RaceStandingV1", 8], slots: ["slot", 8], cars: ["RaceCarStateV1", 8], dnfEvents: ["RaceDnfEventV1", 8],
});
function exactObject(value, names, context) {
  if (!value || typeof value !== "object" || ![Object.prototype, null].includes(Object.getPrototypeOf(value))) throw new TypeError(`${context} must be a plain object`);
  const keys = Reflect.ownKeys(value);
  if (keys.length !== names.length || names.some((key) => !keys.includes(key))) throw new TypeError(`${context} has missing or unknown fields`);
  for (const name of names) {
    const descriptor = Object.getOwnPropertyDescriptor(value, name);
    if (!descriptor?.enumerable || !("value" in descriptor)) throw new TypeError(`${context}.${name} must be an enumerable data property`);
  }
}
function integer(value, bits, signed, context) {
  if (!(typeof value === "number" && Number.isSafeInteger(value)) && !(typeof value === "string" && (signed ? /^(?:0|-?[1-9][0-9]*)$/ : /^(?:0|[1-9][0-9]*)$/).test(value)) && typeof value !== "bigint") throw new TypeError(`${context} must be a canonical integer`);
  const n = BigInt(value), limit = 1n << BigInt(signed ? bits - 1 : bits);
  if (n < (signed ? -limit : 0n) || n >= limit) throw new RangeError(`${context} exceeds ${signed ? "i" : "u"}${bits}`);
  return n;
}
function bytes(value, context, max, length = null) {
  if (typeof value !== "string" || !/^(?:[0-9A-F]{2})+$/.test(value) || value.length > max * 2 || (length !== null && value.length !== length * 2)) throw new TypeError(`${context} must be bounded canonical uppercase hexadecimal bytes`);
  return Buffer.from(value, "hex");
}
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

/** Build codecs from the canonical Norito primitives; never use a generic JSON escape hatch. */
export function createNoritoGameCodecs(h) {
  function encode(type, value, context = type) {
    if (GAME_RESOURCE_VALUE_NAMES_V1.includes(type)) return encodeGameResourceValueV1(type, value);
    if (Object.hasOwn(SCHEMAS, type)) {
      const schema = SCHEMAS[type];
      exactObject(value, schema.map(([name]) => name), context);
      validateClassedRaceScalarsV1(type, value);
      const payload = h.encodeStructValue(schema.map(([name, fieldType]) => [encode(fieldType, value[name], `${context}.${name}`)]));
      if (payload.length > gameValueMaximumBytesV1(type)) throw new RangeError(`${context} exceeds its compiled payload limit`);
      validateRecord(type, value);
      return payload;
    }
    if (Object.hasOwn(VECTORS, type)) {
      const [element, maximum] = VECTORS[type];
      if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype || value.length > maximum || Reflect.ownKeys(value).length !== value.length + 1) throw new RangeError(`${context} must be a bounded dense array`);
      for (let index = 0; index < value.length; index++) {
        const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
        if (!descriptor?.enumerable || !Object.hasOwn(descriptor, "value")) throw new TypeError(`${context} requires dense data elements`);
      }
      // Native Vec<u8> is a packed byte vector, including winner and DNF slots.
      // It has no per-element field-length prefix, unlike other Norito vectors.
      if (element === "u8" || element === "slot") return h.encodeByteVecValue(Buffer.from(value.map((item, index) => encode(element, item, `${context}[${index}]`)[0])), context);
      return h.encodeNoritoVec(value, (item, index) => encode(element, item, `${context}[${index}]`));
    }
    switch (type) {
      case "admissionKey": {
        const encoded = encode("key", value, context);
        if (decode("key", encoded, context) !== value) throw new TypeError(`${context} requires a canonical Ed25519 key`);
        return encoded;
      }
      case "admissionAccount": case "admissionNft": {
        const maximum = type === "admissionAccount" ? 16384 : 512;
        if (typeof value !== "string" || value.length > maximum || Buffer.byteLength(value, "utf8") > maximum) throw new TypeError(`${context} exceeds native identifier byte bound`);
        const base = type === "admissionAccount" ? "account" : "nft", encoded = encode(base, value, context);
        if (encoded.length > (type === "admissionAccount" ? GAME_ADMISSION_MAX_ACCOUNT_ENCODED_BYTES_V1 : GAME_ADMISSION_MAX_NFT_ENCODED_BYTES_V1)) throw new TypeError(`${context} exceeds native encoded identifier bound`);
        if (decode(base, encoded, context) !== value) throw new TypeError(`${context} requires a canonical identifier`);
        return encoded;
      }
      case "hash": return h.encodeEscrowIdValue(value, context);
      case "asset": return h.encodeAssetDefinitionIdValue(value, context);
      case "nft": return h.encodeNftIdValue(value, context);
      case "account": return h.encodeAccountIdValue(value, context);
      case "quantity": return h.encodeQuantityValue(value, context);
      case "bool": return h.encodeBoolValue(value, context);
      case "u8": case "u16": case "u32": case "u64": {
        const n = integer(value, Number(type.slice(1)), false, context);
        return h[`encode${type.toUpperCase()}Value`](n, context);
      }
      case "i32": case "i64": {
        const bits = Number(type.slice(1)), n = integer(value, bits, true, context), result = Buffer.alloc(bits / 8);
        if (bits === 32) result.writeInt32LE(Number(n)); else result.writeBigInt64LE(n);
        return result;
      }
      case "slot": if (Number(integer(value, 8, false, context)) > 7) throw new RangeError(`${context} exceeds slot 7`); return h.encodeU8Value(value, context);
      case "control": if (Number(integer(value, 16, false, context)) > 63) throw new RangeError(`${context} has undefined control bits`); return h.encodeU16Value(value, context);
      case "ClassedRaceClassV1": case "ClassedRaceTrackV1": case "RaceTrackV1": case "track": {
        exactObject(value, ["kind", "value"], context);
        if (value.value !== null) throw new TypeError(`${context}.value must be null for unit tracks`);
        const catalog = type === "ClassedRaceClassV1" ? CLASSED_RACE_CLASSES_V1
          : type === "ClassedRaceTrackV1" ? CLASSED_RACE_TRACKS_V1 : TRACKS;
        const index = catalog.indexOf(value.kind);
        if (index < 0) throw new TypeError(`${context} must name a compiled track`);
        return h.encodeU32Value(index, context);
      }
      case "key": {
        const key = h.parsePublicKeyLiteral(value, context);
        if (key.curve !== 1) throw new TypeError(`${context} must use Ed25519`);
        return h.encodePublicKeyValue(key, context);
      }
      case "signature": return h.encodeConstVecU8Value(bytes(value, context, 64, 64));
      case "admissionData": case "OpaqueBytes": case "opaque": case "proof": case "stark": {
        const maximum = type === "admissionData" ? 4096 : type === "proof" ? EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1 : type === "stark" ? RACE_MAX_STARK_BYTES_V1 : 512 * 1024;
        if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype || Reflect.ownKeys(value).length !== value.length + 1 || type === "proof" && value.length === 0 || value.length > maximum) throw new TypeError(`${context} must be a bounded byte array`);
        for (let index = 0; index < value.length; index++) {
          const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
          if (!descriptor?.enumerable || !Object.hasOwn(descriptor, "value") || !Number.isInteger(descriptor.value) || descriptor.value < 0 || descriptor.value > 255) throw new TypeError(`${context} must contain only dense canonical bytes`);
        }
        return h.encodeByteVecValue(Buffer.from(value), context);
      }
      case "access": {
        if (value?.kind === "public") { exactObject(value, ["kind", "public_key"], context); if (value.public_key !== null) throw new TypeError(`${context}.public_key must be null for public admission`); return h.encodeEnumTagValue(0); }
        exactObject(value, ["kind", "public_key"], context);
        if (value.kind !== "invite") throw new TypeError(`${context} has an unknown admission policy`);
        return h.encodeEnumTagValue(1, () => encode("key", value.public_key, context));
      }
      case "payout": {
        exactObject(value, ["kind", "value"], context);
        if (value.value !== null) throw new TypeError(`${context}.value must be null for unit payout policies`);
        const index = ["no_payout", "equal_winners_or_refund"].indexOf(value.kind);
        if (index < 0) throw new TypeError(`${context} has an unknown payout policy`);
        return h.encodeEnumTagValue(index);
      }
      case "optionalAccount": return h.encodeOptionValue(value, inner => encode("account", inner, context), context);
      case "optionalSignature": return h.encodeOptionValue(value, inner => encode("signature", inner, context), context);
      case "optionalGameFrontier": return h.encodeOptionValue(value, inner => encode("GameCommitmentSetV1", inner, context), context);
      case "optionalRaceState": return h.encodeOptionValue(value, inner => encode("RaceStateV1", inner, context), context);
      case "optionalU64": return h.encodeOptionValue(value, inner => encode("u64", inner, context), context);
      case "optionalU32": return h.encodeOptionValue(value, (inner) => encode("u32", inner, context), context);
      case "optionalFrontier": return h.encodeOptionValue(value, (inner) => encode("RaceCommitmentSetV1", inner, context), context);
      default: throw new TypeError(`Unregistered native race type ${type}`);
    }
  }
  function decode(type, payload, context = type) {
    if (GAME_RESOURCE_VALUE_NAMES_V1.includes(type)) return decodeGameResourceValueV1(type, payload);
    let value;
    if (Object.hasOwn(SCHEMAS, type)) {
      if (payload.length > gameValueMaximumBytesV1(type)) throw new RangeError(`${context} exceeds its compiled payload limit`);
      const schema = SCHEMAS[type], parts = h.decodeStructFields(payload, context, schema.map(([name]) => name));
      value = Object.fromEntries(schema.map(([name, fieldType]) => [name, decode(fieldType, parts[name], `${context}.${name}`)]));
      validateRecord(type, value);
      return value;
    }
    if (Object.hasOwn(VECTORS, type)) {
      const [element, maximum] = VECTORS[type];
      if (element === "u8" || element === "slot") return Array.from(h.decodeByteVecValue(payload, context, maximum), (item, index) => decode(element, Buffer.of(item), `${context}[${index}]`));
      return h.decodeNoritoVec(payload, (item, index) => decode(element, item, `${context}[${index}]`), context, maximum);
    }
    switch (type) {
      case "admissionKey": return decode("key", payload, context);
      case "admissionAccount": case "admissionNft": {
        const value = decode(type === "admissionAccount" ? "account" : "nft", payload, context);
        encode(type, value, context); return value;
      }
      case "hash": return h.decodeEscrowIdValue(payload, context);
      case "asset": return h.decodeAssetDefinitionIdValue(payload, context);
      case "nft": return h.decodeNftIdValue(payload, context);
      case "account": return h.decodeAccountIdValue(payload, context);
      case "quantity": return h.decodeQuantityValue(payload, context);
      case "bool": return h.decodeBoolValue(payload, context);
      case "u8": case "u16": case "u32": case "u64": return h[`decode${type.toUpperCase()}Value`](payload, context);
      case "i32": case "i64": {
        if (payload.length !== Number(type.slice(1)) / 8) throw new TypeError(`${context} has invalid signed integer width`);
        return type === "i32" ? payload.readInt32LE() : payload.readBigInt64LE().toString();
      }
      case "slot": value = h.decodeU8Value(payload, context); encode(type, value, context); return value;
      case "control": value = h.decodeU16Value(payload, context); encode(type, value, context); return value;
      case "ClassedRaceClassV1": case "ClassedRaceTrackV1": case "RaceTrackV1": case "track": {
        const catalog = type === "ClassedRaceClassV1" ? CLASSED_RACE_CLASSES_V1
          : type === "ClassedRaceTrackV1" ? CLASSED_RACE_TRACKS_V1 : TRACKS;
        value = catalog[h.decodeU32Value(payload, context)];
        if (!value) throw new TypeError(`${context} has an unknown compiled class or track tag`);
        return { kind: value, value: null };
      }
      case "key": {
        const key = h.decodePublicKeyValue(payload, context);
        if (key.curve !== 1) throw new TypeError(`${context} must use Ed25519`);
        return h.publicKeyLiteralFromParts(key.curve, key.publicKey, context);
      }
      case "signature": value = h.decodeConstVecU8Value(payload, context).toString("hex").toUpperCase(); bytes(value, context, 64, 64); return value;
      case "admissionData": case "OpaqueBytes": case "opaque": case "proof": case "stark": {
        const maximum = type === "admissionData" ? 4096 : type === "proof" ? EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1 : type === "stark" ? RACE_MAX_STARK_BYTES_V1 : 512 * 1024;
        const result = Array.from(h.decodeByteVecValue(payload, context, maximum));
        if (type === "proof" && result.length === 0) throw new TypeError(`${context} must not be empty`);
        return result;
      }
      case "access": {
        if (payload.length < 4) throw new TypeError(`${context} has no admission tag`);
        const tag = payload.readUInt32LE();
        if (tag === 0 && payload.length === 4) return { kind: "public", public_key: null };
        if (tag !== 1) throw new TypeError(`${context} has an unknown admission tag`);
        const parts = h.decodeStructFields(payload.subarray(4), context, ["public_key"]);
        return { kind: "invite", public_key: decode("key", parts.public_key, context) };
      }
      case "payout": {
        if (payload.length !== 4) throw new TypeError(`${context} has a malformed payout tag`);
        const kind = ["no_payout", "equal_winners_or_refund"][payload.readUInt32LE()];
        if (!kind) throw new TypeError(`${context} has an unknown payout tag`);
        return { kind, value: null };
      }
      case "optionalAccount": return h.decodeOptionValue(payload, inner => decode("account", inner, context), context);
      case "optionalSignature": return h.decodeOptionValue(payload, inner => decode("signature", inner, context), context);
      case "optionalGameFrontier": return h.decodeOptionValue(payload, inner => decode("GameCommitmentSetV1", inner, context), context);
      case "optionalRaceState": return h.decodeOptionValue(payload, inner => decode("RaceStateV1", inner, context), context);
      case "optionalU64": return h.decodeOptionValue(payload, inner => decode("u64", inner, context), context);
      case "optionalU32": return h.decodeOptionValue(payload, (inner) => decode("u32", inner, context), context);
      case "optionalFrontier": return h.decodeOptionValue(payload, (inner) => decode("RaceCommitmentSetV1", inner, context), context);
      default: throw new TypeError(`Unregistered native race type ${type}`);
    }
  }
  return Object.freeze({ encode, decode });
}
