/** Shared exact Norito record, vector, and primitive engine for independent game catalogs. */
import { Buffer } from "buffer";
import {
  EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1,
  GAME_ADMISSION_MAX_ACCOUNT_ENCODED_BYTES_V1,
  GAME_ADMISSION_MAX_NFT_ENCODED_BYTES_V1,
  gameValueMaximumBytesV1,
} from "./noritoGameRegistry.js";
import { GAME_RESOURCE_VALUE_NAMES_V1 } from "./noritoGameResourceEngine.js";

const TEXT_ADMISSION_ACCOUNT = "admissionAccount";
const TEXT_CLASSED_RACE_CLASS_V1 = "ClassedRaceClassV1";
const TEXT_CLASSED_RACE_TRACK_V1 = "ClassedRaceTrackV1";

const RACE_MAX_STARK_BYTES_V1 = 3_250_000;
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
/** Bind one closed schema catalog to the common exact primitive and record rules. */
export function createNoritoGameEngine(h, {
  SCHEMAS, VECTORS, validateRecord, validateScalars,
  CLASSED_RACE_CLASSES_V1, CLASSED_RACE_TRACKS_V1, TRACKS,
}) {
  function encode(type, value, context = type) {
    if (GAME_RESOURCE_VALUE_NAMES_V1.includes(type)) return h.resourceCodecs.encode(type, value);
    if (Object.hasOwn(SCHEMAS, type)) {
      const schema = SCHEMAS[type];
      exactObject(value, schema.map(([name]) => name), context);
      validateScalars(type, value);
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
      case TEXT_ADMISSION_ACCOUNT: case "admissionNft": {
        const maximum = type === TEXT_ADMISSION_ACCOUNT ? 16384 : 512;
        if (typeof value !== "string" || value.length > maximum || Buffer.byteLength(value, "utf8") > maximum) throw new TypeError(`${context} exceeds native identifier byte bound`);
        const base = type === TEXT_ADMISSION_ACCOUNT ? "account" : "nft", encoded = encode(base, value, context);
        if (encoded.length > (type === TEXT_ADMISSION_ACCOUNT ? GAME_ADMISSION_MAX_ACCOUNT_ENCODED_BYTES_V1 : GAME_ADMISSION_MAX_NFT_ENCODED_BYTES_V1)) throw new TypeError(`${context} exceeds native encoded identifier bound`);
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
      case TEXT_CLASSED_RACE_CLASS_V1: case TEXT_CLASSED_RACE_TRACK_V1: case "RaceTrackV1": case "track": {
        exactObject(value, ["kind", "value"], context);
        if (value.value !== null) throw new TypeError(`${context}.value must be null for unit tracks`);
        const catalog = type === TEXT_CLASSED_RACE_CLASS_V1 ? CLASSED_RACE_CLASSES_V1
          : type === TEXT_CLASSED_RACE_TRACK_V1 ? CLASSED_RACE_TRACKS_V1 : TRACKS;
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
    if (GAME_RESOURCE_VALUE_NAMES_V1.includes(type)) return h.resourceCodecs.decode(type, payload);
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
      case TEXT_ADMISSION_ACCOUNT: case "admissionNft": {
        const value = decode(type === TEXT_ADMISSION_ACCOUNT ? "account" : "nft", payload, context);
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
      case TEXT_CLASSED_RACE_CLASS_V1: case TEXT_CLASSED_RACE_TRACK_V1: case "RaceTrackV1": case "track": {
        const catalog = type === TEXT_CLASSED_RACE_CLASS_V1 ? CLASSED_RACE_CLASSES_V1
          : type === TEXT_CLASSED_RACE_TRACK_V1 ? CLASSED_RACE_TRACKS_V1 : TRACKS;
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