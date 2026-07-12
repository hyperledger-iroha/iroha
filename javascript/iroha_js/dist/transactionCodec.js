import { Buffer } from "buffer";
import { blake3 } from "@noble/hashes/blake3";
import { AccountAddress } from "./address.js";
import { blake2b256 } from "./blake2b.js";
import { verifyEd25519 } from "./crypto.browser.js";
import {
  KotodamaQuantity,
  NumericV1,
  NumericV1Error,
} from "./numericV1.js";

const COMPACT_LEN_FLAG = 0x02;
const UINT16_MAX = 0xffffn;
const UINT32_MAX = 0xffff_ffffn;
const UINT64_MAX = 0xffff_ffff_ffff_ffffn;
const MAX_CHAIN_ID_BYTES = 1024;
const MAX_METADATA_JSON_BYTES = 64 * 1024;
const MAX_METADATA_ENTRIES = 64;
const MAX_METADATA_DEPTH = 32;
const MAX_METADATA_NODES = 4096;
const MAX_METADATA_KEY_BYTES = 255;
const MAX_PAYLOAD_BYTES = 1024 * 1024;
const MAX_SIGNED_TRANSACTION_BYTES = MAX_PAYLOAD_BYTES + 4096;
const MAX_NUMERIC_SCALE = 28;
// Rust BigInt permits at most 64 signed two's-complement bytes. Transfers are
// positive, so the interoperable upper bound is 2^511 - 1, not 2^512 - 1.
const MAX_NUMERIC_BITS = 511;
const MAX_NUMERIC_MANTISSA_BYTES = 64;
const MAX_NUMERIC_DECIMAL_DIGITS = 154;
const MAX_QUANTITY_LITERAL_CODE_UNITS = MAX_NUMERIC_DECIMAL_DIGITS + 1;
const ASSET_DEFINITION_ADDRESS_VERSION = 1;
const BASE58_ALPHABET =
  "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const BASE58_LOOKUP = new Map(
  Array.from(BASE58_ALPHABET, (character, index) => [character, BigInt(index)]),
);
const TRANSFER_SCHEMA_HASH = Buffer.from(
  "a4174c78d6341f8f98fc2adae8ed67b9",
  "hex",
);
const CRC64_MASK = UINT64_MAX;
const CRC64_REFLECTED_POLY = 0xc96c_5795_d787_0f42n;
const CRC64_TABLE = (() => {
  const table = new Array(256);
  for (let index = 0; index < table.length; index += 1) {
    let crc = BigInt(index);
    for (let bit = 0; bit < 8; bit += 1) {
      crc =
        (crc & 1n) !== 0n
          ? (crc >> 1n) ^ CRC64_REFLECTED_POLY
          : crc >> 1n;
    }
    table[index] = crc;
  }
  return table;
})();
const TRANSFER_INPUT_FIELDS = new Set([
  "chainId",
  "authority",
  "sourceAssetHoldingId",
  "sourceAssetId",
  "quantity",
  "destinationAccountId",
  "metadata",
  "creationTimeMs",
  "ttlMs",
  "nonce",
  "networkPrefix",
  "chainDiscriminant",
]);
const SIGNABLE_FIELDS = new Set([
  "payloadBytes",
  "payloadHashHex",
  "authority",
  "signingPublicKey",
  "signatureAlgorithm",
]);
const SIGNABLE_CONSTRAINT_FIELDS = new Set([
  "authority",
  "signingPublicKey",
]);
const SIGNATURE_FIELDS = new Set([
  "algorithm",
  "alg",
  "signature",
  "bytes",
  "payload",
]);

class BrowserTransactionCodecError extends TypeError {
  constructor(code, message) {
    super(message);
    this.name = "BrowserTransactionCodecError";
    this.code = code;
  }
}

class Reader {
  constructor(bytes, context, compactLengths = true) {
    this.bytes = bytes;
    this.context = context;
    this.compactLengths = compactLengths;
    this.offset = 0;
  }

  readU8(field) {
    return this.readBytes(1, field)[0];
  }

  readU32(field) {
    return this.readBytes(4, field).readUInt32LE(0);
  }

  readU64(field) {
    return this.readBytes(8, field).readBigUInt64LE(0);
  }

  readLength(field) {
    if (!this.compactLengths) {
      const value = this.readU64(field);
      if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
        fail("malformed_payload", `${this.context}.${field} is too large`);
      }
      return Number(value);
    }
    const start = this.offset;
    let value = 0n;
    let shift = 0n;
    for (let index = 0; index < 10; index += 1) {
      const byte = this.readU8(field);
      value |= BigInt(byte & 0x7f) << shift;
      if ((byte & 0x80) === 0) {
        if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
          fail("malformed_payload", `${this.context}.${field} is too large`);
        }
        const canonical = encodeCompactLength(value);
        const consumed = this.bytes.subarray(start, this.offset);
        if (!consumed.equals(canonical)) {
          fail(
            "malformed_payload",
            `${this.context}.${field} uses a non-canonical length prefix`,
          );
        }
        return Number(value);
      }
      shift += 7n;
    }
    fail("malformed_payload", `${this.context}.${field} length prefix is invalid`);
  }

  readField(field) {
    return this.readBytes(this.readLength(`${field}.length`), `${field}.value`);
  }

  readBytes(length, field) {
    if (!Number.isSafeInteger(length) || length < 0) {
      fail("malformed_payload", `${this.context}.${field} has an invalid length`);
    }
    const end = this.offset + length;
    if (end > this.bytes.length) {
      fail("malformed_payload", `${this.context}.${field} is truncated`);
    }
    const value = this.bytes.subarray(this.offset, end);
    this.offset = end;
    return value;
  }

  assertEof() {
    if (this.offset !== this.bytes.length) {
      fail(
        "malformed_payload",
        `${this.context} contains ${this.bytes.length - this.offset} trailing bytes`,
      );
    }
  }
}

function fail(code, message) {
  throw new BrowserTransactionCodecError(code, message);
}

function assertPlainDataObject(value, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail("invalid_input", `${context} must be a plain object`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    fail("invalid_input", `${context} must be a plain object`);
  }
  for (const key of Reflect.ownKeys(value)) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (
      typeof key !== "string" ||
      !descriptor ||
      !descriptor.enumerable ||
      !Object.prototype.hasOwnProperty.call(descriptor, "value")
    ) {
      fail("invalid_input", `${context} must contain only enumerable data fields`);
    }
  }
  return value;
}

function snapshotAllowedFields(value, allowed, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail("invalid_input", `${context} must be a plain object`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    fail("invalid_input", `${context} must be a plain object`);
  }
  const snapshot = Object.create(null);
  for (const key of Reflect.ownKeys(value)) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (
      typeof key !== "string" ||
      !descriptor ||
      !descriptor.enumerable ||
      !Object.prototype.hasOwnProperty.call(descriptor, "value")
    ) {
      fail("invalid_input", `${context} must contain only enumerable data fields`);
    }
    if (!allowed.has(key)) {
      fail("invalid_input", `${context}.${key} is not supported`);
    }
    Object.defineProperty(snapshot, key, {
      value: descriptor.value,
      enumerable: true,
      configurable: false,
      writable: false,
    });
  }
  return Object.freeze(snapshot);
}

function isWellFormedUnicode(value) {
  for (let index = 0; index < value.length; index += 1) {
    const codeUnit = value.charCodeAt(index);
    if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      if (index + 1 >= value.length) return false;
      const next = value.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) return false;
      index += 1;
    } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      return false;
    }
  }
  return true;
}

function exactString(value, context, { maxBytes, allowControls = false } = {}) {
  if (typeof value !== "string" || value.length === 0) {
    fail("invalid_input", `${context} must be a non-empty exact string`);
  }
  if (maxBytes !== undefined && value.length > maxBytes) {
    fail("bounds_exceeded", `${context} exceeds ${maxBytes} UTF-8 bytes`);
  }
  if (!isWellFormedUnicode(value)) {
    fail("invalid_input", `${context} must contain only Unicode scalar values`);
  }
  if (value.trim() !== value) {
    fail("invalid_input", `${context} must be a non-empty exact string`);
  }
  const length = Buffer.byteLength(value, "utf8");
  if (maxBytes !== undefined && length > maxBytes) {
    fail("bounds_exceeded", `${context} exceeds ${maxBytes} UTF-8 bytes`);
  }
  if (!allowControls && /[\u0000-\u001f\u007f-\u009f]/u.test(value)) {
    fail("invalid_input", `${context} must not contain control characters`);
  }
  if (value.normalize("NFC") !== value) {
    fail("invalid_input", `${context} must use NFC normalization`);
  }
  return value;
}

function normalizeNetworkPrefix(value, context) {
  const normalized = normalizeUnsigned(value, UINT16_MAX, context);
  return Number(normalized);
}

function accountInfo(value, context, expectedDiscriminant) {
  const literal = exactString(value, context, { maxBytes: 512 });
  let parsed;
  try {
    parsed = AccountAddress.parseEncoded(literal, expectedDiscriminant);
  } catch (error) {
    fail("invalid_account", `${context} is not a canonical I105 account: ${error.message}`);
  }
  const discriminant = parsed.chainDiscriminant;
  if (!Number.isInteger(discriminant)) {
    fail("invalid_account", `${context} does not advertise a chain discriminant`);
  }
  let canonical;
  try {
    canonical = parsed.address.toI105(discriminant);
  } catch (error) {
    fail("invalid_account", `${context} could not be rendered canonically: ${error.message}`);
  }
  if (canonical !== literal) {
    fail("invalid_account", `${context} must use its exact canonical I105 form`);
  }
  const controller = parsed.address._controller;
  if (
    !controller ||
    controller.tag !== 0 ||
    controller.curve !== 1 ||
    controller.publicKey?.length !== 32
  ) {
    fail(
      "unsupported_authority",
      `${context} must be a single-key Ed25519 I105 account`,
    );
  }
  return {
    literal,
    discriminant,
    publicKey: Buffer.from(controller.publicKey),
  };
}

function normalizeUnsigned(value, maximum, context) {
  let result;
  if (typeof value === "bigint") {
    result = value;
  } else if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) {
      fail("invalid_integer", `${context} must be a safe integer, bigint, or decimal string`);
    }
    result = BigInt(value);
  } else if (typeof value === "string") {
    const maximumDigits = maximum.toString(10).length;
    if (value.length > maximumDigits) {
      fail("bounds_exceeded", `${context} is outside its canonical range`);
    }
    if (!/^(?:0|[1-9]\d*)$/u.test(value)) {
      fail("invalid_integer", `${context} must be a canonical unsigned integer`);
    }
    result = BigInt(value);
  } else {
    fail("invalid_integer", `${context} must be a canonical unsigned integer`);
  }
  if (result < 0n || result > maximum) {
    fail("bounds_exceeded", `${context} is outside its canonical range`);
  }
  return result;
}

function normalizeOptionalUnsigned(value, maximum, context, { nonZero = false } = {}) {
  if (value === undefined || value === null) {
    return null;
  }
  const result = normalizeUnsigned(value, maximum, context);
  if (nonZero && result === 0n) {
    fail("invalid_integer", `${context} must be non-zero when provided`);
  }
  return result;
}

function normalizeQuantity(value) {
  let quantity;
  try {
    if (value instanceof KotodamaQuantity) {
      quantity = new KotodamaQuantity(value.mantissa, value.scale);
    } else if (typeof value === "string") {
      if (value.length > MAX_QUANTITY_LITERAL_CODE_UNITS) {
        fail("bounds_exceeded", "quantity exceeds the canonical 511-bit positive bound");
      }
      quantity = NumericV1.decodeQuantityJson(value);
    } else if (typeof value === "bigint") {
      quantity = new KotodamaQuantity(value, 0);
    } else {
      fail(
        "invalid_quantity",
        "quantity must be a KotodamaQuantity, canonical quantity string, or bigint; JavaScript numbers are rejected",
      );
    }
  } catch (error) {
    if (!(error instanceof NumericV1Error)) throw error;
    if (error.code === "mantissa_overflow" || error.code === "invalid_scale") {
      fail("bounds_exceeded", `quantity is outside the bounded Kotodama V1 domain (${error.code})`);
    }
    fail("invalid_quantity", `quantity must be canonical and non-negative (${error.code})`);
  }
  const literal = quantity.toString();
  const mantissa = quantity.mantissa;
  if (mantissa === 0n) {
    fail("invalid_quantity", "quantity must be greater than zero");
  }
  return { literal, mantissa, scale: quantity.scale };
}

function normalizeMetadata(input) {
  let value = input;
  let suppliedJson = null;
  if (value === undefined || value === null) {
    return Object.create(null);
  }
  if (typeof value === "string") {
    if (value.length > MAX_METADATA_JSON_BYTES) {
      fail("bounds_exceeded", "metadata JSON exceeds 65536 UTF-8 bytes");
    }
    if (Buffer.byteLength(value, "utf8") > MAX_METADATA_JSON_BYTES) {
      fail("bounds_exceeded", "metadata JSON exceeds 65536 UTF-8 bytes");
    }
    suppliedJson = value;
    try {
      value = JSON.parse(value);
    } catch (error) {
      fail("invalid_metadata", `metadata is not valid JSON: ${error.message}`);
    }
  }
  assertPlainDataObject(value, "metadata");
  const state = { nodes: 0, stack: new Set() };
  const normalized = normalizeMetadataValue(value, "metadata", 0, state);
  if (Object.keys(normalized).length > MAX_METADATA_ENTRIES) {
    fail("bounds_exceeded", `metadata exceeds ${MAX_METADATA_ENTRIES} top-level entries`);
  }
  const json = canonicalJsonStringify(normalized);
  if (suppliedJson !== null && suppliedJson !== json) {
    fail(
      "invalid_metadata",
      "metadata JSON strings must already use the exact canonical encoding",
    );
  }
  if (Buffer.byteLength(json, "utf8") > MAX_METADATA_JSON_BYTES) {
    fail("bounds_exceeded", "canonical metadata JSON exceeds 65536 UTF-8 bytes");
  }
  return normalized;
}

function compareUtf8Strings(left, right) {
  return Buffer.compare(Buffer.from(left, "utf8"), Buffer.from(right, "utf8"));
}

function normalizeMetadataValue(value, context, depth, state) {
  state.nodes += 1;
  if (state.nodes > MAX_METADATA_NODES) {
    fail("bounds_exceeded", `metadata exceeds ${MAX_METADATA_NODES} values`);
  }
  if (depth > MAX_METADATA_DEPTH) {
    fail("bounds_exceeded", `metadata nesting exceeds ${MAX_METADATA_DEPTH}`);
  }
  if (value === null || typeof value === "boolean") {
    return value;
  }
  if (typeof value === "string") {
    if (
      value.length > MAX_METADATA_JSON_BYTES ||
      Buffer.byteLength(value, "utf8") > MAX_METADATA_JSON_BYTES
    ) {
      fail(
        "bounds_exceeded",
        `${context} string exceeds ${MAX_METADATA_JSON_BYTES} UTF-8 bytes`,
      );
    }
    if (!isWellFormedUnicode(value)) {
      fail("invalid_metadata", `${context} must contain only Unicode scalar values`);
    }
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) {
      fail(
        "invalid_metadata",
        `${context} numbers must be safe integers; encode decimals as strings`,
      );
    }
    return value;
  }
  if (typeof value !== "object" || value instanceof Date) {
    fail("invalid_metadata", `${context} contains unsupported ${typeof value}`);
  }
  if (state.stack.has(value)) {
    fail("invalid_metadata", `${context} contains a cycle`);
  }
  state.stack.add(value);
  try {
    if (Array.isArray(value)) {
      return normalizeMetadataArray(value, context, depth, state);
    }
    assertPlainDataObject(value, context);
    const keys = Object.keys(value);
    if (keys.length > MAX_METADATA_NODES - state.nodes) {
      fail("bounds_exceeded", `metadata exceeds ${MAX_METADATA_NODES} values`);
    }
    for (let index = 0; index < keys.length; index += 1) {
      validateMetadataKey(keys[index], `${context} key ${index}`);
    }
    const output = Object.create(null);
    for (const key of keys.sort(compareUtf8Strings)) {
      const descriptor = Object.getOwnPropertyDescriptor(value, key);
      if (!descriptor || !Object.prototype.hasOwnProperty.call(descriptor, "value")) {
        fail("invalid_metadata", `${context} changed during normalization`);
      }
      output[key] = normalizeMetadataValue(
        descriptor.value,
        `${context}.${key}`,
        depth + 1,
        state,
      );
    }
    return output;
  } finally {
    state.stack.delete(value);
  }
}

function normalizeMetadataArray(value, context, depth, state) {
  if (Object.getPrototypeOf(value) !== Array.prototype) {
    fail("invalid_metadata", `${context} arrays must use Array.prototype`);
  }
  const lengthDescriptor = Object.getOwnPropertyDescriptor(value, "length");
  const length = lengthDescriptor?.value;
  if (
    !lengthDescriptor ||
    !Object.prototype.hasOwnProperty.call(lengthDescriptor, "value") ||
    !Number.isSafeInteger(length) ||
    length < 0
  ) {
    fail("invalid_metadata", `${context} has an invalid array length`);
  }
  if (length > MAX_METADATA_NODES - state.nodes) {
    fail("bounds_exceeded", `metadata exceeds ${MAX_METADATA_NODES} values`);
  }
  const ownKeys = Reflect.ownKeys(value);
  if (ownKeys.length !== length + 1) {
    fail(
      "invalid_metadata",
      `${context} arrays must be dense and contain no custom properties`,
    );
  }
  const output = new Array(length);
  for (let index = 0; index < length; index += 1) {
    const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
    if (
      !descriptor ||
      !descriptor.enumerable ||
      !Object.prototype.hasOwnProperty.call(descriptor, "value")
    ) {
      fail(
        "invalid_metadata",
        `${context} arrays must contain only dense data elements`,
      );
    }
    output[index] = normalizeMetadataValue(
      descriptor.value,
      `${context}[${index}]`,
      depth + 1,
      state,
    );
  }
  return output;
}

function isRustUnicodeWhitespace(value) {
  for (const character of value) {
    const codePoint = character.codePointAt(0);
    if (
      (codePoint >= 0x0009 && codePoint <= 0x000d) ||
      codePoint === 0x0020 ||
      codePoint === 0x0085 ||
      codePoint === 0x00a0 ||
      codePoint === 0x1680 ||
      (codePoint >= 0x2000 && codePoint <= 0x200a) ||
      codePoint === 0x2028 ||
      codePoint === 0x2029 ||
      codePoint === 0x202f ||
      codePoint === 0x205f ||
      codePoint === 0x3000
    ) {
      return true;
    }
  }
  return false;
}

function validateMetadataKey(key, context) {
  if (
    key.length === 0 ||
    key.length > MAX_METADATA_KEY_BYTES ||
    !isWellFormedUnicode(key) ||
    key.normalize("NFC") !== key ||
    isRustUnicodeWhitespace(key) ||
    /[@#$]/u.test(key) ||
    Buffer.byteLength(key, "utf8") > MAX_METADATA_KEY_BYTES
  ) {
    fail("invalid_metadata", `${context} is not a canonical metadata Name`);
  }
}

function canonicalJsonStringify(value) {
  if (Array.isArray(value)) {
    return `[${value.map(canonicalJsonStringify).join(",")}]`;
  }
  if (value && typeof value === "object") {
    return `{${Object.keys(value)
      .sort(compareUtf8Strings)
      .map((key) => `${rustPlainJsonString(key)}:${canonicalJsonStringify(value[key])}`)
      .join(",")}}`;
  }
  if (typeof value === "string") {
    return rustPlainJsonString(value);
  }
  if (value === null) return "null";
  if (typeof value === "boolean") return value ? "true" : "false";
  if (typeof value === "number" && Number.isSafeInteger(value)) {
    return Object.is(value, -0) ? "0" : String(value);
  }
  fail("invalid_metadata", "metadata contains a non-canonical JSON value");
}

function rustPlainJsonString(value) {
  let output = '"';
  for (const character of value) {
    const codePoint = character.codePointAt(0);
    switch (character) {
      case '"':
        output += '\\"';
        break;
      case "\\":
        output += "\\\\";
        break;
      case "\n":
        output += "\\n";
        break;
      case "\r":
        output += "\\r";
        break;
      case "\t":
        output += "\\t";
        break;
      default:
        if (codePoint < 0x20) {
          output += `\\u00${codePoint.toString(16).toUpperCase().padStart(2, "0")}`;
        } else {
          output += character;
        }
    }
  }
  return `${output}"`;
}

function encodeCompactLength(value) {
  let remaining = BigInt(value);
  if (remaining < 0n) {
    fail("internal_codec_error", "cannot encode a negative length");
  }
  const bytes = [];
  do {
    let byte = Number(remaining & 0x7fn);
    remaining >>= 7n;
    if (remaining !== 0n) {
      byte |= 0x80;
    }
    bytes.push(byte);
  } while (remaining !== 0n);
  return Buffer.from(bytes);
}

function u32(value) {
  const output = Buffer.allocUnsafe(4);
  output.writeUInt32LE(Number(value), 0);
  return output;
}

function u64(value) {
  const output = Buffer.allocUnsafe(8);
  output.writeBigUInt64LE(BigInt(value), 0);
  return output;
}

function field(payload) {
  return Buffer.concat([encodeCompactLength(payload.length), payload]);
}

function fieldU64(payload) {
  return Buffer.concat([u64(payload.length), payload]);
}

function stringValue(value) {
  return field(Buffer.from(value, "utf8"));
}

function chainIdArchive(value) {
  return struct([stringValue(value)]);
}

function struct(fields) {
  return Buffer.concat(fields.map(field));
}

function option(value) {
  return value === null ? Buffer.of(0) : Buffer.concat([Buffer.of(1), field(value)]);
}

function vector(values) {
  return Buffer.concat([u64(values.length), ...values.map(field)]);
}

function constVecBytes(bytes) {
  return Buffer.concat([u64(bytes.length), ...Array.from(bytes, (byte) => field(Buffer.of(byte)))]);
}

function accountArchive(info) {
  return Buffer.concat([
    u32(0),
    field(constVecBytes(Buffer.concat([Buffer.of(0), info.publicKey]))),
  ]);
}

function decodeBase58(value, context) {
  let numeric = 0n;
  for (const character of value) {
    const digit = BASE58_LOOKUP.get(character);
    if (digit === undefined) {
      fail("invalid_asset", `${context} contains a non-Base58 character`);
    }
    numeric = numeric * 58n + digit;
  }
  const reversed = [];
  while (numeric > 0n) {
    reversed.push(Number(numeric & 0xffn));
    numeric >>= 8n;
  }
  let leadingZeroes = 0;
  while (value[leadingZeroes] === "1") {
    leadingZeroes += 1;
  }
  return Buffer.concat([
    Buffer.alloc(leadingZeroes),
    Buffer.from(reversed.reverse()),
  ]);
}

function assetDefinitionArchive(literal) {
  const bytes = decodeBase58(literal, "source asset definition");
  if (bytes.length !== 21 || bytes[0] !== ASSET_DEFINITION_ADDRESS_VERSION) {
    fail("invalid_asset", "source asset definition is not a canonical v1 address");
  }
  const expectedChecksum = Buffer.from(blake3(bytes.subarray(0, 17))).subarray(0, 4);
  if (!bytes.subarray(17).equals(expectedChecksum)) {
    fail("invalid_asset", "source asset definition checksum is invalid");
  }
  return Buffer.concat(Array.from(bytes.subarray(1, 17), (byte) => field(Buffer.of(byte))));
}

function assetScopeArchive(scope) {
  if (scope === null) {
    return u32(0);
  }
  const match = /^dataspace:(0|[1-9]\d*)$/u.exec(scope);
  if (!match) {
    fail("invalid_asset", "asset scope must use canonical dataspace:<u64> syntax");
  }
  const value = normalizeUnsigned(match[1], UINT64_MAX, "asset dataspace scope");
  return Buffer.concat([u32(1), field(field(u64(value)))]);
}

function assetArchive(definition, owner, scope) {
  return struct([
    accountArchive(owner),
    assetDefinitionArchive(definition),
    assetScopeArchive(scope),
  ]);
}

function bigintToTwosBytes(value) {
  if (value === 0n) {
    return Buffer.alloc(0);
  }
  const bytes = [];
  let remaining = value;
  while (remaining > 0n) {
    bytes.push(Number(remaining & 0xffn));
    remaining >>= 8n;
  }
  if ((bytes[bytes.length - 1] & 0x80) !== 0) {
    bytes.push(0);
  }
  return Buffer.from(bytes);
}

function numericArchive(quantity) {
  const mantissaBytes = bigintToTwosBytes(quantity.mantissa);
  return struct([
    Buffer.concat([u32(mantissaBytes.length), mantissaBytes]),
    u32(quantity.scale),
  ]);
}

function metadataArchive(metadata) {
  const entries = Object.keys(metadata)
    .sort(compareUtf8Strings)
    .map((key) =>
      struct([
        stringValue(key),
        struct([stringValue(canonicalJsonStringify(metadata[key]))]),
      ]),
    );
  return vector(entries);
}

function crc64(payload) {
  let value = CRC64_MASK;
  for (const byte of payload) {
    const index = Number((value ^ BigInt(byte)) & 0xffn);
    value = CRC64_TABLE[index] ^ (value >> 8n);
  }
  return BigInt.asUintN(64, value ^ CRC64_MASK);
}

function frameTransferPayload(payload) {
  return Buffer.concat([
    Buffer.from("NRT0", "ascii"),
    Buffer.of(0, 0),
    TRANSFER_SCHEMA_HASH,
    Buffer.of(0),
    u64(payload.length),
    u64(crc64(payload)),
    Buffer.of(COMPACT_LEN_FLAG),
    payload,
  ]);
}

function transferInstructionArchive(source, quantity, destination) {
  const body = struct([
    assetArchive(source.definition, source.owner, source.scope),
    numericArchive(quantity),
    accountArchive(destination),
  ]);
  const innerPayload = Buffer.concat([u32(2), field(body)]);
  const innerFrame = frameTransferPayload(innerPayload);
  return Buffer.concat([
    field(stringValue("iroha.transfer")),
    field(fieldU64(innerFrame)),
  ]);
}

function normalizeTransferInput(input, now) {
  input = snapshotAllowedFields(input, TRANSFER_INPUT_FIELDS, "transfer input");
  const chainId = exactString(input.chainId, "chainId", {
    maxBytes: MAX_CHAIN_ID_BYTES,
  });
  const requestedDiscriminant =
    input.networkPrefix !== undefined && input.chainDiscriminant !== undefined
      ? fail(
          "invalid_input",
          "provide only one of networkPrefix or chainDiscriminant",
        )
      : input.networkPrefix ?? input.chainDiscriminant;
  const authority = accountInfo(
    input.authority,
    "authority",
    requestedDiscriminant === undefined
      ? undefined
      : normalizeNetworkPrefix(requestedDiscriminant, "networkPrefix"),
  );
  const destination = accountInfo(
    input.destinationAccountId,
    "destinationAccountId",
    authority.discriminant,
  );
  if (
    input.sourceAssetHoldingId !== undefined &&
    input.sourceAssetId !== undefined &&
    input.sourceAssetHoldingId !== input.sourceAssetId
  ) {
    fail(
      "invalid_asset",
      "sourceAssetHoldingId and sourceAssetId must not disagree",
    );
  }
  const sourceLiteral = exactString(
    input.sourceAssetHoldingId ?? input.sourceAssetId,
    "sourceAssetHoldingId",
    { maxBytes: 1024 },
  );
  const sourceParts = sourceLiteral.split("#");
  if (sourceParts.length < 2 || sourceParts.length > 3) {
    fail(
      "invalid_asset",
      "sourceAssetHoldingId must use <asset-definition>#<owner> with an optional #dataspace:<u64> suffix",
    );
  }
  const sourceOwner = accountInfo(
    sourceParts[1],
    "sourceAssetHoldingId owner",
    authority.discriminant,
  );
  if (sourceOwner.literal !== authority.literal) {
    fail(
      "authority_mismatch",
      "transparent browser transfers require the source asset owner to equal authority",
    );
  }
  const source = {
    definition: exactString(sourceParts[0], "source asset definition", {
      maxBytes: 128,
    }),
    owner: sourceOwner,
    scope: sourceParts[2] ?? null,
  };
  // Decode eagerly so checksum/version failures occur before any large archive allocation.
  assetDefinitionArchive(source.definition);
  assetScopeArchive(source.scope);
  const quantity = normalizeQuantity(input.quantity);
  const metadata = normalizeMetadata(input.metadata);
  const creationTimeMs = normalizeUnsigned(
    input.creationTimeMs ?? now(),
    UINT64_MAX,
    "creationTimeMs",
  );
  let ttlMs = normalizeOptionalUnsigned(input.ttlMs, UINT64_MAX, "ttlMs");
  if (ttlMs === 0n) {
    ttlMs = 1n;
  }
  const nonce = normalizeOptionalUnsigned(input.nonce, UINT32_MAX, "nonce", {
    nonZero: true,
  });
  return {
    chainId,
    authority,
    source,
    quantity,
    destination,
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
  };
}

function encodeTransferPayload(normalized) {
  const instruction = transferInstructionArchive(
    normalized.source,
    normalized.quantity,
    normalized.destination,
  );
  const executable = Buffer.concat([u32(0), field(vector([instruction]))]);
  const payload = struct([
    chainIdArchive(normalized.chainId),
    accountArchive(normalized.authority),
    u64(normalized.creationTimeMs),
    executable,
    option(normalized.ttlMs === null ? null : u64(normalized.ttlMs)),
    option(normalized.nonce === null ? null : u32(normalized.nonce)),
    metadataArchive(normalized.metadata),
  ]);
  if (payload.length === 0 || payload.length > MAX_PAYLOAD_BYTES) {
    fail("bounds_exceeded", `transaction payload exceeds ${MAX_PAYLOAD_BYTES} bytes`);
  }
  return payload;
}

function bytes(value, context, { hex = false, maxBytes } = {}) {
  if (Buffer.isBuffer(value)) {
    if (maxBytes !== undefined && value.length > maxBytes) {
      fail("bounds_exceeded", `${context} exceeds ${maxBytes} bytes`);
    }
    return Buffer.from(value);
  }
  if (ArrayBuffer.isView(value)) {
    if (maxBytes !== undefined && value.byteLength > maxBytes) {
      fail("bounds_exceeded", `${context} exceeds ${maxBytes} bytes`);
    }
    try {
      return Buffer.from(
        new Uint8Array(value.buffer, value.byteOffset, value.byteLength),
      );
    } catch {
      fail("invalid_bytes", `${context} references detached or invalid bytes`);
    }
  }
  if (value instanceof ArrayBuffer) {
    if (maxBytes !== undefined && value.byteLength > maxBytes) {
      fail("bounds_exceeded", `${context} exceeds ${maxBytes} bytes`);
    }
    try {
      return Buffer.from(new Uint8Array(value));
    } catch {
      fail("invalid_bytes", `${context} references detached or invalid bytes`);
    }
  }
  if (hex && typeof value === "string") {
    const maximum = maxBytes ?? MAX_SIGNED_TRANSACTION_BYTES;
    if (value.length > maximum * 2 + 2) {
      fail("bounds_exceeded", `${context} hexadecimal input is too large`);
    }
    const literal = value.startsWith("0x") ? value.slice(2) : value;
    if (/^(?:[0-9a-fA-F]{2})+$/u.test(literal)) {
      return Buffer.from(literal, "hex");
    }
  }
  fail("invalid_bytes", `${context} must be bytes${hex ? " or exact hexadecimal" : ""}`);
}

function irohaHash(value) {
  const digest = Buffer.from(blake2b256(value));
  digest[digest.length - 1] |= 1;
  return digest;
}

function exactHashHex(value, context) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)) {
    fail("invalid_hash", `${context} must be a 32-byte hexadecimal string`);
  }
  return value;
}

function validateStringArchive(payload, context, { maxBytes } = {}) {
  const reader = new Reader(payload, context);
  const value = reader.readField("utf8");
  reader.assertEof();
  if (maxBytes !== undefined && value.length > maxBytes) {
    fail("bounds_exceeded", `${context} exceeds ${maxBytes} UTF-8 bytes`);
  }
  let decoded;
  try {
    decoded = new TextDecoder("utf-8", { fatal: true }).decode(value);
  } catch {
    fail("malformed_payload", `${context} contains invalid UTF-8`);
  }
  if (!stringValue(decoded).equals(payload)) {
    fail("malformed_payload", `${context} is not canonical`);
  }
  return decoded;
}

function validateChainIdArchive(payload, context) {
  const reader = new Reader(payload, context);
  const chainId = validateStringArchive(
    reader.readField("value"),
    `${context}.value`,
    { maxBytes: MAX_CHAIN_ID_BYTES },
  );
  reader.assertEof();
  return chainId;
}

function validateConstVecBytes(payload, context, expectedLength) {
  const reader = new Reader(payload, context);
  const count = reader.readU64("count");
  if (count !== BigInt(expectedLength)) {
    fail("malformed_payload", `${context} must contain ${expectedLength} bytes`);
  }
  const output = Buffer.alloc(expectedLength);
  for (let index = 0; index < expectedLength; index += 1) {
    const item = reader.readField(`item[${index}]`);
    if (item.length !== 1) {
      fail("malformed_payload", `${context}.item[${index}] must contain one byte`);
    }
    output[index] = item[0];
  }
  reader.assertEof();
  return output;
}

function validateAccountArchive(payload, context) {
  const reader = new Reader(payload, context);
  if (reader.readU32("controllerTag") !== 0) {
    fail("unsupported_authority", `${context} must use a single-key controller`);
  }
  const controller = validateConstVecBytes(
    reader.readField("controller"),
    `${context}.controller`,
    33,
  );
  reader.assertEof();
  if (controller[0] !== 0) {
    fail("unsupported_algorithm", `${context} must use Ed25519 algorithm tag 0`);
  }
  const publicKey = controller.subarray(1);
  try {
    AccountAddress.fromAccount({ algorithm: "ed25519", publicKey });
  } catch (error) {
    fail("invalid_public_key", `${context} contains an invalid Ed25519 key: ${error.message}`);
  }
  return Buffer.from(publicKey);
}

function validateFixedByteArchive(payload, length, context) {
  const reader = new Reader(payload, context);
  const output = Buffer.alloc(length);
  for (let index = 0; index < length; index += 1) {
    const item = reader.readField(`item[${index}]`);
    if (item.length !== 1) {
      fail("malformed_payload", `${context}.item[${index}] must contain one byte`);
    }
    output[index] = item[0];
  }
  reader.assertEof();
  return output;
}

function validateScopeArchive(payload, context) {
  const reader = new Reader(payload, context);
  const tag = reader.readU32("tag");
  if (tag === 0) {
    reader.assertEof();
    return;
  }
  if (tag !== 1) {
    fail("malformed_payload", `${context} uses unsupported tag ${tag}`);
  }
  const outer = new Reader(reader.readField("dataspace"), `${context}.dataspace`);
  const inner = outer.readField("value");
  outer.assertEof();
  if (inner.length !== 8) {
    fail("malformed_payload", `${context}.dataspace must contain a u64`);
  }
  reader.assertEof();
}

function validateAssetArchive(payload, context) {
  const reader = new Reader(payload, context);
  const owner = validateAccountArchive(reader.readField("owner"), `${context}.owner`);
  validateFixedByteArchive(reader.readField("definition"), 16, `${context}.definition`);
  validateScopeArchive(reader.readField("scope"), `${context}.scope`);
  reader.assertEof();
  return owner;
}

function twosBytesToBigint(payload) {
  if (payload.length === 0) {
    return 0n;
  }
  let value = 0n;
  for (let index = payload.length - 1; index >= 0; index -= 1) {
    value = (value << 8n) | BigInt(payload[index]);
  }
  if ((payload[payload.length - 1] & 0x80) !== 0) {
    value -= 1n << BigInt(payload.length * 8);
  }
  return value;
}

function validateNumericArchive(payload, context) {
  const reader = new Reader(payload, context);
  const mantissaPayload = reader.readField("mantissa");
  const scalePayload = reader.readField("scale");
  reader.assertEof();
  const mantissaReader = new Reader(mantissaPayload, `${context}.mantissa`);
  const byteLength = mantissaReader.readU32("byteLength");
  if (byteLength > MAX_NUMERIC_MANTISSA_BYTES) {
    fail(
      "bounds_exceeded",
      `${context}.mantissa exceeds ${MAX_NUMERIC_BITS} bits`,
    );
  }
  const mantissaBytes = mantissaReader.readBytes(byteLength, "bytes");
  mantissaReader.assertEof();
  const mantissa = twosBytesToBigint(mantissaBytes);
  if (mantissa <= 0n || !bigintToTwosBytes(mantissa).equals(mantissaBytes)) {
    fail("malformed_payload", `${context}.mantissa is not canonical and positive`);
  }
  if (mantissa.toString(2).length > MAX_NUMERIC_BITS) {
    fail("bounds_exceeded", `${context}.mantissa exceeds ${MAX_NUMERIC_BITS} bits`);
  }
  if (scalePayload.length !== 4 || scalePayload.readUInt32LE(0) > MAX_NUMERIC_SCALE) {
    fail("malformed_payload", `${context}.scale is outside the supported range`);
  }
  const scale = scalePayload.readUInt32LE(0);
  if (scale > 0 && mantissa % 10n === 0n) {
    fail(
      "malformed_payload",
      `${context} has a non-canonical fractional trailing zero`,
    );
  }
}

function validateFrame(frame, context) {
  if (frame.length < 40 || frame.subarray(0, 4).toString("ascii") !== "NRT0") {
    fail("malformed_payload", `${context} is not an NRT0 frame`);
  }
  const reader = new Reader(frame, context, false);
  reader.readBytes(4, "magic");
  if (reader.readU8("major") !== 0 || reader.readU8("minor") !== 0) {
    fail("malformed_payload", `${context} uses an unsupported NRT0 version`);
  }
  if (!reader.readBytes(16, "schemaHash").equals(TRANSFER_SCHEMA_HASH)) {
    fail("malformed_payload", `${context} has the wrong transfer schema hash`);
  }
  if (reader.readU8("reserved") !== 0) {
    fail("malformed_payload", `${context} has a non-zero reserved byte`);
  }
  const length = reader.readU64("payloadLength");
  const expectedCrc = reader.readU64("crc64");
  if (reader.readU8("flags") !== COMPACT_LEN_FLAG) {
    fail("malformed_payload", `${context} must advertise compact-length layout only`);
  }
  if (length > BigInt(Number.MAX_SAFE_INTEGER)) {
    fail("malformed_payload", `${context} payload length is too large`);
  }
  const payload = reader.readBytes(Number(length), "payload");
  reader.assertEof();
  if (crc64(payload) !== expectedCrc) {
    fail("malformed_payload", `${context} CRC64 does not match`);
  }
  return payload;
}

function validateTransferExecutable(payload, context) {
  const executable = new Reader(payload, context);
  if (executable.readU32("variant") !== 0) {
    fail("unsupported_executable", `${context} must use Executable::Instructions`);
  }
  const instructions = new Reader(executable.readField("instructions"), `${context}.instructions`);
  if (instructions.readU64("count") !== 1n) {
    fail("unsupported_executable", `${context} must contain exactly one instruction`);
  }
  const instruction = new Reader(
    instructions.readField("item[0]"),
    `${context}.instructions[0]`,
  );
  const wireId = validateStringArchive(
    instruction.readField("wireId"),
    `${context}.instructions[0].wireId`,
    { maxBytes: Buffer.byteLength("iroha.transfer", "utf8") },
  );
  if (wireId !== "iroha.transfer") {
    fail("unsupported_instruction", `${context} must contain iroha.transfer`);
  }
  const frameContainer = new Reader(
    instruction.readField("frameContainer"),
    `${context}.instructions[0].frameContainer`,
    false,
  );
  const frame = frameContainer.readField("frame");
  frameContainer.assertEof();
  instruction.assertEof();
  instructions.assertEof();
  executable.assertEof();
  const transferPayload = validateFrame(frame, `${context}.instructions[0].frame`);
  const transfer = new Reader(transferPayload, `${context}.transfer`);
  if (transfer.readU32("variant") !== 2) {
    fail("unsupported_instruction", `${context} must contain Transfer::Asset`);
  }
  const body = new Reader(transfer.readField("body"), `${context}.transfer.body`);
  const sourceOwner = validateAssetArchive(
    body.readField("source"),
    `${context}.transfer.source`,
  );
  validateNumericArchive(body.readField("quantity"), `${context}.transfer.quantity`);
  validateAccountArchive(body.readField("destination"), `${context}.transfer.destination`);
  body.assertEof();
  transfer.assertEof();
  return sourceOwner;
}

function validateOption(payload, expectedLength, context, { nonZero = false } = {}) {
  const reader = new Reader(payload, context);
  const tag = reader.readU8("tag");
  if (tag === 0) {
    reader.assertEof();
    return null;
  }
  if (tag !== 1) {
    fail("malformed_payload", `${context} uses invalid option tag ${tag}`);
  }
  const value = reader.readField("value");
  reader.assertEof();
  if (value.length !== expectedLength) {
    fail("malformed_payload", `${context} has the wrong integer width`);
  }
  const numeric = expectedLength === 8 ? value.readBigUInt64LE(0) : BigInt(value.readUInt32LE(0));
  if (nonZero && numeric === 0n) {
    fail("malformed_payload", `${context} must be non-zero when present`);
  }
  return numeric;
}

function validateMetadataArchive(payload, context) {
  const reader = new Reader(payload, context);
  const count = reader.readU64("count");
  if (count > BigInt(MAX_METADATA_ENTRIES)) {
    fail("bounds_exceeded", `${context} exceeds ${MAX_METADATA_ENTRIES} entries`);
  }
  const state = { nodes: 1, stack: new Set() };
  const normalizedMetadata = Object.create(null);
  let previousKey = null;
  for (let index = 0; index < Number(count); index += 1) {
    const entry = new Reader(reader.readField(`entry[${index}]`), `${context}[${index}]`);
    const key = validateStringArchive(
      entry.readField("key"),
      `${context}[${index}].key`,
      { maxBytes: MAX_METADATA_KEY_BYTES },
    );
    validateMetadataKey(key, `${context}[${index}].key`);
    if (previousKey !== null && compareUtf8Strings(previousKey, key) >= 0) {
      fail("malformed_payload", `${context} keys must be unique and sorted`);
    }
    previousKey = key;
    const json = new Reader(entry.readField("json"), `${context}[${index}].json`);
    const jsonText = validateStringArchive(
      json.readField("value"),
      `${context}[${index}].json.value`,
      { maxBytes: MAX_METADATA_JSON_BYTES },
    );
    if (Buffer.byteLength(jsonText, "utf8") > MAX_METADATA_JSON_BYTES) {
      fail(
        "bounds_exceeded",
        `${context}[${index}] JSON exceeds ${MAX_METADATA_JSON_BYTES} UTF-8 bytes`,
      );
    }
    json.assertEof();
    entry.assertEof();
    let parsed;
    try {
      parsed = JSON.parse(jsonText);
    } catch {
      fail("malformed_payload", `${context}[${index}] contains invalid JSON`);
    }
    const normalized = normalizeMetadataValue(
      parsed,
      `${context}.${key}`,
      1,
      state,
    );
    if (canonicalJsonStringify(normalized) !== jsonText) {
      fail("malformed_payload", `${context}[${index}] JSON is not canonical`);
    }
    normalizedMetadata[key] = normalized;
  }
  reader.assertEof();
  if (
    Buffer.byteLength(canonicalJsonStringify(normalizedMetadata), "utf8") >
    MAX_METADATA_JSON_BYTES
  ) {
    fail(
      "bounds_exceeded",
      `${context} canonical JSON exceeds ${MAX_METADATA_JSON_BYTES} UTF-8 bytes`,
    );
  }
}

function validateTransactionPayload(payload, authorityLiteral) {
  if (payload.length === 0 || payload.length > MAX_PAYLOAD_BYTES) {
    fail("bounds_exceeded", `payloadBytes must contain 1..=${MAX_PAYLOAD_BYTES} bytes`);
  }
  const assertedAuthority =
    authorityLiteral === null
      ? null
      : accountInfo(authorityLiteral, "signable.authority");
  const reader = new Reader(payload, "transaction payload");
  const chainId = validateChainIdArchive(
    reader.readField("chain"),
    "transaction payload.chain",
  );
  exactString(chainId, "transaction payload.chain", { maxBytes: MAX_CHAIN_ID_BYTES });
  const authorityArchive = reader.readField("authority");
  const authorityPublicKey = validateAccountArchive(
    authorityArchive,
    "transaction payload.authority",
  );
  if (
    assertedAuthority !== null &&
    !authorityArchive.equals(accountArchive(assertedAuthority))
  ) {
    fail("authority_mismatch", "signable.authority does not match payloadBytes");
  }
  const creationTime = reader.readField("creationTimeMs");
  if (creationTime.length !== 8) {
    fail("malformed_payload", "transaction payload.creationTimeMs must be a u64");
  }
  const sourceOwner = validateTransferExecutable(
    reader.readField("executable"),
    "transaction payload.executable",
  );
  if (!sourceOwner.equals(authorityPublicKey)) {
    fail("authority_mismatch", "source asset owner does not match transaction authority");
  }
  validateOption(reader.readField("ttlMs"), 8, "transaction payload.ttlMs", {
    nonZero: true,
  });
  validateOption(reader.readField("nonce"), 4, "transaction payload.nonce", {
    nonZero: true,
  });
  validateMetadataArchive(reader.readField("metadata"), "transaction payload.metadata");
  reader.assertEof();
  return assertedAuthority ?? { publicKey: authorityPublicKey };
}

function normalizeSignature(signature) {
  if (
    Buffer.isBuffer(signature) ||
    ArrayBuffer.isView(signature) ||
    signature instanceof ArrayBuffer
  ) {
    return bytes(signature, "signature", { maxBytes: 64 });
  }
  signature = snapshotAllowedFields(signature, SIGNATURE_FIELDS, "signature");
  if (signature.algorithm !== undefined && signature.alg !== undefined) {
    fail("invalid_input", "signature must not provide both algorithm and alg");
  }
  const byteAliases = ["signature", "bytes", "payload"].filter(
    (field) => signature[field] !== undefined,
  );
  if (byteAliases.length !== 1) {
    fail(
      "invalid_input",
      "signature must provide exactly one of signature, bytes, or payload",
    );
  }
  const algorithm = signature.algorithm ?? signature.alg ?? "ed25519";
  if (algorithm !== "ed25519" && algorithm !== 0) {
    fail("unsupported_algorithm", "only Ed25519 transaction signatures are supported");
  }
  return bytes(
    signature.signature ?? signature.bytes ?? signature.payload,
    "signature.signature",
    { hex: true, maxBytes: 64 },
  );
}

function signatureArchive(signature) {
  return field(constVecBytes(signature));
}

function bareSignedTransaction(payload, signature) {
  return struct([
    signatureArchive(signature),
    payload,
    Buffer.of(0),
    Buffer.of(0),
  ]);
}

function transactionHashFromBare(bare) {
  return irohaHash(Buffer.concat([u32(0), field(bare)]));
}

/**
 * Build one canonical transparent `Transfer::Asset` transaction payload.
 *
 * The returned bare adaptive-Norito bytes are the bytes hashed and presented
 * to an external Ed25519 signer. Delegated, multisig, and non-Ed25519 transfers
 * are intentionally outside this narrow browser surface.
 *
 * @param {object} input
 * @returns {Buffer}
 */
export function buildBrowserTransferPayload(input) {
  const normalized = normalizeTransferInput(input, Date.now);
  const payload = encodeTransferPayload(normalized);
  validateTransactionPayload(payload, normalized.authority.literal);
  return payload;
}

/**
 * Return the marked Iroha Blake2b-256 prehash for a transaction payload.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} payloadBytes
 * @returns {string}
 */
export function browserTransactionPayloadHashHex(payloadBytes) {
  const payload = bytes(payloadBytes, "payloadBytes", {
    maxBytes: MAX_PAYLOAD_BYTES,
  });
  if (payload.length === 0 || payload.length > MAX_PAYLOAD_BYTES) {
    fail("bounds_exceeded", `payloadBytes must contain 1..=${MAX_PAYLOAD_BYTES} bytes`);
  }
  return irohaHash(payload).toString("hex");
}

/**
 * Snapshot and validate one exact canonical Transfer::Asset signable.
 *
 * This is intended for wallet and hardware-signer trust boundaries: the
 * returned buffers are detached copies, the payload hash is recomputed, and
 * both the asserted and optional expected authority/public key are bound to
 * the authority encoded in the payload.
 *
 * @param {object} signable
 * @param {{authority?: string | null, signingPublicKey?: ArrayBufferView | ArrayBuffer | Buffer | string | null}} constraints
 * @returns {{payloadBytes: Buffer, payloadHashHex: string, authority: string, signingPublicKey: Buffer, signatureAlgorithm: "ed25519"}}
 */
export function validateBrowserTransferSignable(signable, constraints = {}) {
  signable = snapshotAllowedFields(signable, SIGNABLE_FIELDS, "signable");
  constraints = snapshotAllowedFields(
    constraints,
    SIGNABLE_CONSTRAINT_FIELDS,
    "signable constraints",
  );
  if (
    signable.signatureAlgorithm !== undefined &&
    signable.signatureAlgorithm !== "ed25519" &&
    signable.signatureAlgorithm !== "0" &&
    signable.signatureAlgorithm !== 0
  ) {
    fail("unsupported_algorithm", "signable.signatureAlgorithm must be ed25519");
  }
  const payload = bytes(signable.payloadBytes, "signable.payloadBytes", {
    maxBytes: MAX_PAYLOAD_BYTES,
  });
  const authorityLiteral = exactString(signable.authority, "signable.authority", {
    maxBytes: 512,
  });
  const authority = validateTransactionPayload(payload, authorityLiteral);
  const payloadHashHex = irohaHash(payload).toString("hex");
  const assertedPayloadHashHex = exactHashHex(
    signable.payloadHashHex,
    "signable.payloadHashHex",
  );
  if (assertedPayloadHashHex !== payloadHashHex) {
    fail("payload_hash_mismatch", "signable.payloadHashHex does not match payloadBytes");
  }
  const signingPublicKey = bytes(
    signable.signingPublicKey,
    "signable.signingPublicKey",
    { hex: true, maxBytes: 32 },
  );
  if (signingPublicKey.length !== 32) {
    fail("invalid_public_key", "signable.signingPublicKey must be exactly 32 bytes");
  }
  if (!signingPublicKey.equals(authority.publicKey)) {
    fail(
      "authority_mismatch",
      "signable.signingPublicKey does not control signable.authority",
    );
  }
  if (constraints.authority !== undefined && constraints.authority !== null) {
    const expectedAuthority = accountInfo(
      constraints.authority,
      "signable constraints.authority",
    );
    if (expectedAuthority.literal !== authorityLiteral) {
      fail(
        "authority_mismatch",
        "signable.authority does not match the expected approved authority",
      );
    }
  }
  if (
    constraints.signingPublicKey !== undefined &&
    constraints.signingPublicKey !== null
  ) {
    const expectedPublicKey = bytes(
      constraints.signingPublicKey,
      "signable constraints.signingPublicKey",
      { hex: true, maxBytes: 32 },
    );
    if (expectedPublicKey.length !== 32 || !expectedPublicKey.equals(signingPublicKey)) {
      fail(
        "authority_mismatch",
        "signable.signingPublicKey does not match the expected approved signing key",
      );
    }
  }
  return Object.freeze({
    payloadBytes: payload,
    payloadHashHex,
    authority: authorityLiteral,
    signingPublicKey,
    signatureAlgorithm: "ed25519",
  });
}

/**
 * Verify and finalize an externally signed transparent transfer.
 *
 * @param {object} signable
 * @param {object | ArrayBufferView | ArrayBuffer | Buffer} signature
 * @param {ArrayBufferView | ArrayBuffer | Buffer | string} signingPublicKey
 * @returns {{signedTransaction: Buffer, hash: Buffer, hashHex: string}}
 */
export function finalizeBrowserSignedTransaction(
  signable,
  signature,
  signingPublicKey,
) {
  signable = snapshotAllowedFields(signable, SIGNABLE_FIELDS, "signable");
  if (
    signable.signatureAlgorithm !== undefined &&
    signable.signatureAlgorithm !== "ed25519" &&
    signable.signatureAlgorithm !== 0
  ) {
    fail("unsupported_algorithm", "signable.signatureAlgorithm must be ed25519");
  }
  const payload = bytes(signable.payloadBytes, "signable.payloadBytes", {
    maxBytes: MAX_PAYLOAD_BYTES,
  });
  const authority = validateTransactionPayload(payload, signable.authority);
  const publicKey = bytes(signingPublicKey, "signingPublicKey", {
    hex: true,
    maxBytes: 32,
  });
  if (publicKey.length !== 32) {
    fail("invalid_public_key", "signingPublicKey must be exactly 32 bytes");
  }
  if (signable.signingPublicKey != null) {
    const assertedPublicKey = bytes(
      signable.signingPublicKey,
      "signable.signingPublicKey",
      { hex: true, maxBytes: 32 },
    );
    if (assertedPublicKey.length !== 32 || !assertedPublicKey.equals(publicKey)) {
      fail(
        "authority_mismatch",
        "signable.signingPublicKey does not match signingPublicKey",
      );
    }
  }
  if (!publicKey.equals(authority.publicKey)) {
    fail("authority_mismatch", "signingPublicKey does not control signable.authority");
  }
  const payloadHash = irohaHash(payload);
  if (signable.payloadHashHex !== undefined) {
    const asserted = exactHashHex(signable.payloadHashHex, "signable.payloadHashHex");
    if (asserted !== payloadHash.toString("hex")) {
      fail("payload_hash_mismatch", "signable.payloadHashHex does not match payloadBytes");
    }
  }
  const signatureBytes = normalizeSignature(signature);
  if (signatureBytes.length !== 64) {
    fail("invalid_signature", "Ed25519 signature must be exactly 64 bytes");
  }
  let verified = false;
  try {
    verified = verifyEd25519(payloadHash, signatureBytes, publicKey);
  } catch {
    verified = false;
  }
  if (!verified) {
    fail("invalid_signature", "Ed25519 signature does not verify over the Iroha payload hash");
  }
  const bare = bareSignedTransaction(payload, signatureBytes);
  const signedTransaction = Buffer.concat([Buffer.of(1), bare]);
  const hash = transactionHashFromBare(bare);
  return {
    signedTransaction,
    hash,
    hashHex: hash.toString("hex"),
  };
}

/**
 * Compute the canonical pipeline hash of exact versioned SignedTransaction bytes.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} signedTransaction
 * @returns {string}
 */
export function browserSignedTransactionHashHex(signedTransaction) {
  const versioned = bytes(signedTransaction, "signedTransaction", {
    maxBytes: MAX_SIGNED_TRANSACTION_BYTES,
  });
  if (
    versioned.length < 2 ||
    versioned.length > MAX_SIGNED_TRANSACTION_BYTES ||
    versioned[0] !== 1
  ) {
    fail(
      "malformed_signed_transaction",
      "signedTransaction must be non-empty exact version-1 SignedTransaction bytes",
    );
  }
  const bare = versioned.subarray(1);
  const reader = new Reader(bare, "signed transaction");
  const signatureValue = new Reader(reader.readField("signature"), "signed transaction.signature");
  const rawSignature = validateConstVecBytes(
    signatureValue.readField("payload"),
    "signed transaction.signature.payload",
    64,
  );
  signatureValue.assertEof();
  const payload = reader.readField("payload");
  const attachments = reader.readField("attachments");
  const multisig = reader.readField("multisigSignatures");
  reader.assertEof();
  if (
    rawSignature.length !== 64 ||
    !attachments.equals(Buffer.of(0)) ||
    !multisig.equals(Buffer.of(0)) ||
    payload.length === 0 ||
    payload.length > MAX_PAYLOAD_BYTES
  ) {
    fail(
      "malformed_signed_transaction",
      "signedTransaction is not a supported single-signature external transaction",
    );
  }
  validateTransactionPayload(payload, null);
  return transactionHashFromBare(bare).toString("hex");
}

/** Browser-safe codec implementing the Nexus transaction-codec contract. */
export const browserTransactionCodec = Object.freeze({
  buildTransferPayload: buildBrowserTransferPayload,
  payloadHashHex: browserTransactionPayloadHashHex,
  finalizeSignedTransaction: finalizeBrowserSignedTransaction,
  validateSignable: validateBrowserTransferSignable,
});

export { BrowserTransactionCodecError };
