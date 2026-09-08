/**
 * Canonical bounded COMPACT_LEN codecs for explicit game equipment authorization.
 * Custody and entitlement are authenticated by native consensus admission.
 */
import { Buffer } from "buffer";


export const GAME_MAX_RESOURCES_PER_PARTICIPANT_V1 = 4;
export const GAME_MAX_RESOURCE_PARTICIPANTS_V1 = 32;
export const GAME_MAX_RESOURCE_RECORDS_V1 = 128;
export const GAME_RESOURCE_MAX_NFT_ID_BYTES_V1 = 512;
export const GAME_RESOURCE_MAX_ACCOUNT_ID_BYTES_V1 = 16 * 1024;
export const GAME_RESOURCE_MAX_CLAUSE_BYTES_V1 = 1024;
export const GAME_RESOURCE_MAX_SET_BYTES_V1 = 1024 * 1024;
const POLICY = "return_to_original_owner_at_terminal";
const clauseFields = [["nft_id", "nft"], ["expected_metadata_hash", "hash"],
  ["role_id", "hash"], ["policy", "GameResourceReturnPolicyV1"]];
const schemas = Object.freeze({
  GameResourceReservationClauseV1: clauseFields,
  GameResourceRequirementV1: clauseFields,
  GameResourceReservationRecordV1: [["slot", "u8"], ["nft_id", "nft"],
    ["metadata_hash", "hash"], ["role_id", "hash"], ["policy", "GameResourceReturnPolicyV1"],
    ["original_owner", "account"], ["custody", "account"],
    ["reserved_at_height", "u64"], ["released_at_height", "optionalU64"]],
  GameResourceReservationSetV1: [["version", "u16"], ["network_id", "hash"],
    ["session_id", "hash"], ["records", "records"]],
});
const vectors = Object.freeze({
  clauses: ["GameResourceReservationClauseV1", GAME_MAX_RESOURCES_PER_PARTICIPANT_V1],
  requirements: ["GameResourceRequirementV1", GAME_MAX_RESOURCES_PER_PARTICIPANT_V1],
  records: ["GameResourceReservationRecordV1", GAME_MAX_RESOURCE_RECORDS_V1],
});
export const GAME_RESOURCE_VALUE_NAMES_V1 = Object.freeze([...Object.keys(schemas), "GameResourceReturnPolicyV1", "clauses", "requirements", "records"]);
const publicNames = new Set(GAME_RESOURCE_VALUE_NAMES_V1);

function exact(value, names, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)
      || ![Object.prototype, null].includes(Object.getPrototypeOf(value))) {
    throw new TypeError(`${label} requires a plain object`);
  }
  const keys = Reflect.ownKeys(value);
  if (keys.length !== names.length || keys.some(key => !names.includes(key))) {
    throw new TypeError(`${label} requires exact fields`);
  }
  for (const key of names) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (!descriptor?.enumerable || !Object.hasOwn(descriptor, "value")) {
      throw new TypeError(`${label} requires enumerable data fields`);
    }
  }
}
function array(value, bound, label) {
  if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype
      || value.length > bound || Reflect.ownKeys(value).length !== value.length + 1) {
    throw new TypeError(`${label} exceeds its array bound or has noncanonical properties`);
  }
  for (let i = 0; i < value.length; i++) {
    const descriptor = Object.getOwnPropertyDescriptor(value, String(i));
    if (!descriptor?.enumerable || !Object.hasOwn(descriptor, "value")) {
      throw new TypeError(`${label} requires dense data elements`);
    }
  }
}
function concat(parts) {
  let size = 0;
  for (const part of parts) {
    size += part.length;
    if (size > GAME_RESOURCE_MAX_SET_BYTES_V1) throw new RangeError("resource value exceeds encoded byte bound");
  }
  return Buffer.concat(parts, size);
}
function length(value) {
  const out = [];
  do { const part = value % 128; value = Math.floor(value / 128); out.push(part | (value > 0 ? 128 : 0)); } while (value);
  return Buffer.from(out);
}
const field = payload => concat([length(payload.length), payload]);
class Reader {
  constructor(bytes) { this.bytes = bytes; this.offset = 0; }
  read(size) {
    if (!Number.isSafeInteger(size) || size < 0 || size > this.bytes.length - this.offset) {
      throw new TypeError("truncated resource value");
    }
    const out = this.bytes.subarray(this.offset, this.offset + size); this.offset += size; return out;
  }
  field() {
    // 1 MiB is the complete archive bound: four or more ULEB bytes are invalid.
    let size = 0;
    for (let i = 0; i < 3; i++) {
      const byte = this.read(1)[0]; size += (byte & 127) * 2 ** (7 * i);
      if (!(byte & 128)) {
        if (i > 0 && byte === 0) throw new TypeError("noncanonical resource field length");
        return this.read(size);
      }
    }
    throw new RangeError("resource field length exceeds byte bound");
  }
  end() { if (this.offset !== this.bytes.length) throw new TypeError("trailing resource bytes"); }
}
/** Bind resource validation to the owning canonical primitive codecs. */
export function createNoritoGameResourceEngine({ encodePrimitive, decodePrimitive }) {
  function primitive(name, value) {
    const stringLimit = name === "nft" ? GAME_RESOURCE_MAX_NFT_ID_BYTES_V1
      : name === "account" ? GAME_RESOURCE_MAX_ACCOUNT_ID_BYTES_V1 : name === "hash" ? 74 : null;
    if (stringLimit !== null && (typeof value !== "string" || value.length > stringLimit
        || Buffer.byteLength(value, "utf8") > stringLimit)) {
      throw new TypeError(`resource ${name} exceeds canonical string byte bound`);
    }
    // Bound textual integers before BigInt parsing. The existing primitive codec
    // validates sign, width and canonical decimal syntax without rounding.
    if (["u8", "u16", "u64", "optionalU64"].includes(name)
        && typeof value === "string" && value.length > 20) throw new RangeError("resource integer exceeds u64");
    const encode = encodePrimitive;
    const decode = decodePrimitive;
    const bytes = encode(name, value), canonical = decode(name, bytes);
    if (stringLimit !== null && canonical !== value) throw new TypeError(`resource ${name} must be canonical`);
    return bytes;
  }
  function roles(entries) {
    let previous;
    const nfts = new Set();
    for (const entry of entries) {
      const role = primitive("hash", entry.role_id);
      if (previous && Buffer.compare(previous, role) >= 0) throw new TypeError("resource roles must be unique and strictly ordered");
      if (nfts.has(entry.nft_id)) throw new TypeError("resource NFTs must be unique");
      previous = role; nfts.add(entry.nft_id);
    }
  }
  function setSemantics(value) {
    if (Number(value.version) !== 1) throw new TypeError("unknown resource set version");
    let previous, released;
    const nfts = new Set(), custody = new Set(), owners = new Map(), slots = new Map(), counts = new Map();
    for (const record of value.records) {
      const slot = Number(record.slot), role = primitive("hash", record.role_id);
      if (slot >= GAME_MAX_RESOURCE_PARTICIPANTS_V1) throw new RangeError("resource slot exceeds bound");
      counts.set(slot, (counts.get(slot) ?? 0) + 1);
      if (counts.get(slot) > GAME_MAX_RESOURCES_PER_PARTICIPANT_V1) throw new RangeError("too many resources per slot");
      if (previous && (previous.slot > slot || previous.slot === slot && Buffer.compare(previous.role, role) >= 0)) {
        throw new TypeError("resource records must be strictly ordered by slot and role");
      }
      previous = { slot, role };
      if (nfts.has(record.nft_id) || custody.has(record.custody)) throw new TypeError("resource NFT and custody identities must be unique");
      nfts.add(record.nft_id); custody.add(record.custody);
      if (slots.has(slot) && slots.get(slot) !== record.original_owner
          || owners.has(record.original_owner) && owners.get(record.original_owner) !== slot) {
        throw new TypeError("resource owners must uniquely match participant slots");
      }
      slots.set(slot, record.original_owner); owners.set(record.original_owner, slot);
      const reserved = BigInt(record.reserved_at_height);
      const release = record.released_at_height === null ? null : BigInt(record.released_at_height);
      if (reserved === 0n || release !== null && release < reserved) throw new TypeError("invalid resource reservation or return height");
      if (released !== undefined && release !== released) throw new TypeError("resources must return atomically at one terminal height");
      released = release;
    }
    if ([...custody].some(account => owners.has(account))) throw new TypeError("resource custody cannot be a participant owner");
  }
  function encode(name, value) {
    if (Object.hasOwn(schemas, name)) {
      const schema = schemas[name]; exact(value, schema.map(([key]) => key), name);
      const bytes = concat(schema.map(([key, type]) => field(encode(type, value[key]))));
      if (clauseFields === schema && bytes.length > GAME_RESOURCE_MAX_CLAUSE_BYTES_V1) throw new RangeError("resource clause exceeds encoded byte bound");
      if (name === "GameResourceReservationSetV1") setSemantics(value);
      return bytes;
    }
    if (Object.hasOwn(vectors, name)) {
      const [type, bound] = vectors[name]; array(value, bound, name);
      const count = Buffer.alloc(8); count.writeBigUInt64LE(BigInt(value.length));
      const bytes = concat([count, ...value.map(entry => field(encode(type, entry)))]);
      if (name === "records") setSemantics({ version: 1, records: value });
      else roles(value);
      return bytes;
    }
    if (name === "GameResourceReturnPolicyV1") {
      exact(value, ["kind", "value"], name);
      if (value.kind !== POLICY || value.value !== null) throw new TypeError("unknown resource policy or non-null unit content");
      return Buffer.alloc(4);
    }
    return primitive(name, value);
  }
  function decode(name, bytes) {
    if (Object.hasOwn(schemas, name)) {
      const reader = new Reader(bytes), value = {};
      for (const [key, type] of schemas[name]) value[key] = decode(type, reader.field());
      reader.end(); return value;
    }
    if (Object.hasOwn(vectors, name)) {
      const [type, bound] = vectors[name], reader = new Reader(bytes);
      const count = reader.read(8).readBigUInt64LE();
      if (count > BigInt(bound)) throw new RangeError("resource vector count exceeds bound");
      const value = [];
      for (let i = 0; i < Number(count); i++) value.push(decode(type, reader.field()));
      reader.end(); return value;
    }
    if (name === "GameResourceReturnPolicyV1") {
      if (bytes.length !== 4 || bytes.readUInt32LE() !== 0) throw new TypeError("unknown resource policy or enum payload");
      return { kind: POLICY, value: null };
    }
    return decodePrimitive(name, bytes);
  }

  /** Encode one canonical bounded resource value. */
  function encodeGameResourceValueV1(name, value) {
    if (!publicNames.has(name)) throw new TypeError("unknown game resource value");
    return encode(name, value);
  }
  /** Decode one bounded bare value, rejecting unknown, excess and noncanonical data. */
  function decodeGameResourceValueV1(name, input) {
    if (!publicNames.has(name)) throw new TypeError("unknown game resource value");
    if (!(input instanceof Uint8Array) || input.byteLength > GAME_RESOURCE_MAX_SET_BYTES_V1) {
      throw new TypeError("resource bytes must be a bounded Uint8Array");
    }
    const bytes = Buffer.from(input.buffer, input.byteOffset, input.byteLength);
    const value = decode(name, bytes);
    if (!encode(name, value).equals(bytes)) throw new TypeError("resource bytes are not canonical");
    return value;
  }

  return Object.freeze({
    encode: encodeGameResourceValueV1,
    decode: decodeGameResourceValueV1,
  });
}
