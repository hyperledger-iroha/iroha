import { Buffer } from "buffer";

/** Canonical closed Race V1 value registry shared by browser signing and native ISIs. */
export const RACE_INSTRUCTION_NAMES_V1 = Object.freeze([
  "OpenRaceV1", "JoinRaceV1", "StartRaceV1", "CommitRaceCheckpointV1",
  "ChallengeRaceV1", "CommitRaceInputsV1", "RevealRaceInputsV1",
  "AdvanceRaceDeadlineV1", "SubmitRaceProofV1", "ExpireRaceV1",
]);
export const RACE_INSTRUCTION_WIRE_IDS_V1 = Object.freeze(
  RACE_INSTRUCTION_NAMES_V1.map((name) => `iroha.instruction.v1::race::${name}`),
);
const TRACKS = Object.freeze(["neon_tokyo", "harbor", "sakura"]);
const fields = (text) => text.split(" ").map((entry) => entry.split(":"));
const SCHEMAS = Object.freeze({
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
  RacePublicInputsV1: fields("network_id:hash race_id:hash roster_hash:hash rules_hash:hash track:track transcript_root:hash dispute_root:hash result:RaceResultV1"),
  ExecutionProofEnvelopeV1: fields("version:u16 profile_id:hash statement:RacePublicInputsV1 proof_bytes:proof"),
  OpenRaceV1: fields("race_id:hash rules:RaceRulesV1 asset_definition:asset stake:quantity join_deadline_height:u64"),
  JoinRaceV1: fields("race_id:hash input_key:key car_id:u8"),
  StartRaceV1: fields("race_id:hash"),
  CommitRaceCheckpointV1: fields("race_id:hash checkpoint:SignedRaceCheckpointV1 frontier:optionalFrontier"),
  ChallengeRaceV1: fields("race_id:hash epoch:u64 slot:u8 signature:signature"),
  CommitRaceInputsV1: fields("input:RaceInputCommitmentV1"),
  RevealRaceInputsV1: fields("reveal:RaceInputRevealV1"),
  AdvanceRaceDeadlineV1: fields("race_id:hash"),
  SubmitRaceProofV1: fields("race_id:hash proof:ExecutionProofEnvelopeV1"),
  ExpireRaceV1: fields("race_id:hash"),
});
const VECTORS = Object.freeze({
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
function validateRecord(name, value) {
  if ((name === "RaceRulesV1" || name === "ExecutionProofEnvelopeV1") && Number(value.version) !== 1) throw new RangeError(`${name}.version must be 1`);
  if (name === "RaceRulesV1" && (Number(value.max_racers) < 2 || Number(value.max_racers) > 8)) throw new RangeError("RaceRulesV1.max_racers must be 2..8");
  if (name === "JoinRaceV1" && Number(value.car_id) > 5) throw new RangeError("JoinRaceV1.car_id must be 0..5");
  if (Object.hasOwn(value, "slot") && Number(value.slot) > 7) throw new RangeError(`${name}.slot must be 0..7`);
  for (const field of ["tick", "ticks", "start_tick"]) if (Object.hasOwn(value, field) && Number(value[field]) > 5400) throw new RangeError(`${name}.${field} exceeds the race tick limit`);
  if (name === "RaceInputRevealV1" && value.controls.length !== 6) throw new RangeError("Race input reveals require exactly six controls");
  if (name === "RaceReplayV1" && (Number(value.player_count) < 1 || Number(value.player_count) > 8 || value.frames.some((frame, index) => Number(frame.tick) !== index || frame.controls.length !== Number(value.player_count)))) throw new RangeError("Race replay frames must be consecutive and match the roster size");
  if (Object.hasOwn(value, "signatures") && value.signatures.some((item, index, all) => index > 0 && Number(item.slot) <= Number(all[index - 1].slot))) throw new RangeError("Race signatures must be unique and ordered by slot");
}

/** Build codecs from the canonical Norito primitives; never use a generic JSON escape hatch. */
export function createNoritoRaceCodecs(h) {
  function encode(type, value, context = type) {
    if (Object.hasOwn(SCHEMAS, type)) {
      const schema = SCHEMAS[type];
      exactObject(value, schema.map(([name]) => name), context);
      const payload = h.encodeStructValue(schema.map(([name, fieldType]) => [encode(fieldType, value[name], `${context}.${name}`)]));
      validateRecord(type, value);
      return payload;
    }
    if (Object.hasOwn(VECTORS, type)) {
      const [element, maximum] = VECTORS[type];
      if (!Array.isArray(value) || value.length > maximum || Array.from({ length: value.length }, (_, index) => index).some((index) => !Object.hasOwn(value, index))) throw new RangeError(`${context} must be a bounded dense array`);
      return h.encodeNoritoVec(value, (item, index) => encode(element, item, `${context}[${index}]`));
    }
    switch (type) {
      case "hash": return h.encodeEscrowIdValue(value, context);
      case "asset": return h.encodeAssetDefinitionIdValue(value, context);
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
      case "track": {
        exactObject(value, ["kind"], context);
        const index = TRACKS.indexOf(value.kind);
        if (index < 0) throw new TypeError(`${context} must name a compiled track`);
        return h.encodeU32Value(index, context);
      }
      case "key": {
        const key = h.parsePublicKeyLiteral(value, context);
        if (key.curve !== 1) throw new TypeError(`${context} must use Ed25519`);
        return h.encodePublicKeyValue(key, context);
      }
      case "signature": return h.encodeConstVecU8Value(bytes(value, context, 64, 64));
      case "proof": {
        if (!Array.isArray(value) || value.length === 0 || value.length > 512 * 1024 || Array.from({ length: value.length }, (_, index) => index).some((index) => !Object.hasOwn(value, index) || !Number.isInteger(value[index]) || value[index] < 0 || value[index] > 255)) throw new TypeError(`${context} must be a nonempty bounded byte array`);
        return h.encodeByteVecValue(Buffer.from(value), context);
      }
      case "optionalU32": return h.encodeOptionValue(value, (inner) => encode("u32", inner, context), context);
      case "optionalFrontier": return h.encodeOptionValue(value, (inner) => encode("RaceCommitmentSetV1", inner, context), context);
      default: throw new TypeError(`Unregistered native race type ${type}`);
    }
  }
  function decode(type, payload, context = type) {
    let value;
    if (Object.hasOwn(SCHEMAS, type)) {
      const schema = SCHEMAS[type], parts = h.decodeStructFields(payload, context, schema.map(([name]) => name));
      value = Object.fromEntries(schema.map(([name, fieldType]) => [name, decode(fieldType, parts[name], `${context}.${name}`)]));
      validateRecord(type, value);
      return value;
    }
    if (Object.hasOwn(VECTORS, type)) {
      const [element, maximum] = VECTORS[type];
      return h.decodeNoritoVec(payload, (item, index) => decode(element, item, `${context}[${index}]`), context, maximum);
    }
    switch (type) {
      case "hash": return h.decodeEscrowIdValue(payload, context);
      case "asset": return h.decodeAssetDefinitionIdValue(payload, context);
      case "quantity": return h.decodeQuantityValue(payload, context);
      case "bool": return h.decodeBoolValue(payload, context);
      case "u8": case "u16": case "u32": case "u64": return h[`decode${type.toUpperCase()}Value`](payload, context);
      case "i32": case "i64": {
        if (payload.length !== Number(type.slice(1)) / 8) throw new TypeError(`${context} has invalid signed integer width`);
        return type === "i32" ? payload.readInt32LE() : payload.readBigInt64LE().toString();
      }
      case "slot": value = h.decodeU8Value(payload, context); encode(type, value, context); return value;
      case "control": value = h.decodeU16Value(payload, context); encode(type, value, context); return value;
      case "track": value = TRACKS[h.decodeU32Value(payload, context)]; if (!value) throw new TypeError(`${context} has an unknown track tag`); return { kind: value };
      case "key": {
        const key = h.decodePublicKeyValue(payload, context);
        if (key.curve !== 1) throw new TypeError(`${context} must use Ed25519`);
        return h.publicKeyLiteralFromParts(key.curve, key.publicKey, context);
      }
      case "signature": value = h.decodeConstVecU8Value(payload, context).toString("hex").toUpperCase(); bytes(value, context, 64, 64); return value;
      case "proof": {
        const result = Array.from(h.decodeByteVecValue(payload, context, 512 * 1024));
        if (result.length === 0) throw new TypeError(`${context} must not be empty`);
        return result;
      }
      case "optionalU32": return h.decodeOptionValue(payload, (inner) => decode("u32", inner, context), context);
      case "optionalFrontier": return h.decodeOptionValue(payload, (inner) => decode("RaceCommitmentSetV1", inner, context), context);
      default: throw new TypeError(`Unregistered native race type ${type}`);
    }
  }
  return Object.freeze({ encode, decode });
}
