// SPDX-License-Identifier: Apache-2.0
//
// KAGEMUSHA V1 attested-app suite (`iroha:kagemusha:v1:attested-app`): pure
// JavaScript verification, transport and issuer client.
//
// Browsers and Node.js have no attested app key, so this module never spends,
// requests or signs money. It decodes and verifies the canonical Norito objects
// produced by attested devices and the scheme issuer. It deliberately imports
// only @noble primitives and pure local helpers, never the native addon.

import { p256 } from "@noble/curves/p256";
import { blake2b } from "@noble/hashes/blake2b";
import { sha256 } from "@noble/hashes/sha2";

import { crc64Xz } from "./crc64Xz.js";
import { parseStrictLosslessIntegerJson, stringifyStrictLosslessIntegerJson } from "./strictLosslessJson.js";

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/** Typed failure raised by every decoder and strict verifier in this module. */
export class KagemushaAttestedError extends Error {
  constructor(code, message, options = undefined) {
    super(message, options);
    this.name = "KagemushaAttestedError";
    this.code = code;
  }
}

function fail(code, message) {
  throw new KagemushaAttestedError(code, message);
}

// ---------------------------------------------------------------------------
// Byte helpers
// ---------------------------------------------------------------------------

const UTF8_ENCODER = new TextEncoder();
const UTF8_DECODER = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
const HEX_DIGITS = "0123456789abcdef";

function asBytes(value, context) {
  if (value instanceof Uint8Array) return Uint8Array.from(value);
  if (value instanceof ArrayBuffer) return new Uint8Array(value.slice(0));
  if (ArrayBuffer.isView(value)) {
    return Uint8Array.from(new Uint8Array(value.buffer, value.byteOffset, value.byteLength));
  }
  return fail("InvalidArgument", `${context} must be binary data`);
}

function fixedBytes(value, length, context) {
  const bytes = asBytes(value, context);
  if (bytes.length !== length) fail("InvalidArgument", `${context} must contain exactly ${length} bytes`);
  return bytes;
}

function concatBytes(...parts) {
  let total = 0;
  for (const part of parts) total += part.length;
  const out = new Uint8Array(total);
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}

function equalBytes(left, right) {
  if (left.length !== right.length) return false;
  let difference = 0;
  for (let index = 0; index < left.length; index += 1) difference |= left[index] ^ right[index];
  return difference === 0;
}

function isZeroBytes(bytes) {
  for (let index = 0; index < bytes.length; index += 1) if (bytes[index] !== 0) return false;
  return true;
}

function toHex(bytes) {
  let out = "";
  for (let index = 0; index < bytes.length; index += 1) {
    out += HEX_DIGITS[bytes[index] >>> 4] + HEX_DIGITS[bytes[index] & 15];
  }
  return out;
}

function fromHex(text, context) {
  if (typeof text !== "string" || text.length % 2 !== 0 || !/^[0-9a-fA-F]*$/u.test(text)) {
    fail("InvalidArgument", `${context} must be an even-length hex string`);
  }
  const out = new Uint8Array(text.length / 2);
  for (let index = 0; index < out.length; index += 1) {
    out[index] = Number.parseInt(text.slice(index * 2, index * 2 + 2), 16);
  }
  return out;
}

function bigEndianUint(bytes) {
  let value = 0n;
  for (let index = 0; index < bytes.length; index += 1) value = (value << 8n) | BigInt(bytes[index]);
  return value;
}

function u16be(value) {
  return Uint8Array.of((value >>> 8) & 0xff, value & 0xff);
}

function u32be(value) {
  return Uint8Array.of((value >>> 24) & 0xff, (value >>> 16) & 0xff, (value >>> 8) & 0xff, value & 0xff);
}

function readU16be(bytes, offset) {
  return (bytes[offset] << 8) | bytes[offset + 1];
}

function readU32be(bytes, offset) {
  return ((bytes[offset] << 24) | (bytes[offset + 1] << 16) | (bytes[offset + 2] << 8) | bytes[offset + 3]) >>> 0;
}

function littleEndian(value, width, context) {
  let remaining = BigInt(value);
  if (remaining < 0n) fail("InvalidArgument", `${context} must be unsigned`);
  const out = new Uint8Array(width);
  for (let index = 0; index < width; index += 1) {
    out[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  if (remaining !== 0n) fail("InvalidArgument", `${context} exceeds ${width * 8} bits`);
  return out;
}

function readLittleEndian(bytes, offset, width) {
  let value = 0n;
  for (let index = width - 1; index >= 0; index -= 1) value = (value << 8n) | BigInt(bytes[offset + index]);
  return value;
}

// ---------------------------------------------------------------------------
// Text encodings: unpadded base64url (kga1:), RFC 9285 Base45 (IQR1)
// ---------------------------------------------------------------------------

const BASE64URL_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const BASE64URL_REVERSE = /* @__PURE__ */ (() => {
  const table = new Int16Array(128).fill(-1);
  for (let index = 0; index < 64; index += 1) table[BASE64URL_ALPHABET.charCodeAt(index)] = index;
  return table;
})();

function base64UrlEncode(bytes) {
  let out = "";
  let index = 0;
  for (; index + 2 < bytes.length; index += 3) {
    const value = (bytes[index] << 16) | (bytes[index + 1] << 8) | bytes[index + 2];
    out += BASE64URL_ALPHABET[value >>> 18] + BASE64URL_ALPHABET[(value >>> 12) & 63]
      + BASE64URL_ALPHABET[(value >>> 6) & 63] + BASE64URL_ALPHABET[value & 63];
  }
  const remaining = bytes.length - index;
  if (remaining === 1) {
    const value = bytes[index] << 16;
    out += BASE64URL_ALPHABET[value >>> 18] + BASE64URL_ALPHABET[(value >>> 12) & 63];
  } else if (remaining === 2) {
    const value = (bytes[index] << 16) | (bytes[index + 1] << 8);
    out += BASE64URL_ALPHABET[value >>> 18] + BASE64URL_ALPHABET[(value >>> 12) & 63]
      + BASE64URL_ALPHABET[(value >>> 6) & 63];
  }
  return out;
}

/** Strict unpadded base64url: rejects padding, foreign characters and non-zero trailing bits. */
function base64UrlDecode(text, context) {
  if (typeof text !== "string" || text.length % 4 === 1) {
    fail("Malformed", `${context} is not canonical unpadded base64url`);
  }
  const out = new Uint8Array(Math.floor((text.length * 3) / 4));
  let outIndex = 0;
  let buffer = 0;
  let bits = 0;
  for (let index = 0; index < text.length; index += 1) {
    const code = text.charCodeAt(index);
    const value = code < 128 ? BASE64URL_REVERSE[code] : -1;
    if (value < 0) fail("Malformed", `${context} is not canonical unpadded base64url`);
    buffer = (buffer << 6) | value;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      out[outIndex] = (buffer >>> bits) & 0xff;
      outIndex += 1;
    }
    buffer &= (1 << bits) - 1;
  }
  if (buffer !== 0) fail("Malformed", `${context} is not canonical unpadded base64url`);
  return out;
}

const BASE45_ALPHABET = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";
const BASE45_REVERSE = /* @__PURE__ */ (() => {
  const table = new Int16Array(128).fill(-1);
  for (let index = 0; index < 45; index += 1) table[BASE45_ALPHABET.charCodeAt(index)] = index;
  return table;
})();

function base45Encode(bytes) {
  let out = "";
  let index = 0;
  for (; index + 1 < bytes.length; index += 2) {
    let value = bytes[index] * 256 + bytes[index + 1];
    out += BASE45_ALPHABET[value % 45];
    value = Math.floor(value / 45);
    out += BASE45_ALPHABET[value % 45] + BASE45_ALPHABET[Math.floor(value / 45)];
  }
  if (index < bytes.length) {
    const value = bytes[index];
    out += BASE45_ALPHABET[value % 45] + BASE45_ALPHABET[Math.floor(value / 45)];
  }
  return out;
}

function base45Digit(code) {
  const value = code < 128 ? BASE45_REVERSE[code] : -1;
  if (value < 0) fail("Malformed", "IQR1 body is not canonical Base45");
  return value;
}

function base45Decode(text) {
  if (text.length === 0 || text.length % 3 === 1) fail("Malformed", "IQR1 body is not canonical Base45");
  const out = new Uint8Array(Math.floor(text.length / 3) * 2 + (text.length % 3 === 2 ? 1 : 0));
  let outIndex = 0;
  let index = 0;
  for (; index + 2 < text.length; index += 3) {
    const value = base45Digit(text.charCodeAt(index)) + base45Digit(text.charCodeAt(index + 1)) * 45
      + base45Digit(text.charCodeAt(index + 2)) * 2025;
    if (value > 0xffff) fail("Malformed", "IQR1 body is not canonical Base45");
    out[outIndex] = value >>> 8;
    out[outIndex + 1] = value & 0xff;
    outIndex += 2;
  }
  if (index < text.length) {
    const value = base45Digit(text.charCodeAt(index)) + base45Digit(text.charCodeAt(index + 1)) * 45;
    if (value > 0xff) fail("Malformed", "IQR1 body is not canonical Base45");
    out[outIndex] = value;
  }
  return out;
}

// ---------------------------------------------------------------------------
// Checksums and the bounded zlib decoder used by IPM1 encoding 1
// ---------------------------------------------------------------------------

const CRC32C_TABLE = /* @__PURE__ */ (() => {
  const table = new Uint32Array(256);
  for (let value = 0; value < 256; value += 1) {
    let crc = value;
    for (let bit = 0; bit < 8; bit += 1) crc = (crc & 1) === 0 ? crc >>> 1 : (crc >>> 1) ^ 0x82f63b78;
    table[value] = crc >>> 0;
  }
  return table;
})();

function crc32c(bytes) {
  let crc = 0xffffffff;
  for (let index = 0; index < bytes.length; index += 1) {
    crc = (crc >>> 8) ^ CRC32C_TABLE[(crc ^ bytes[index]) & 0xff];
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function adler32(bytes) {
  let first = 1;
  let second = 0;
  for (let index = 0; index < bytes.length; index += 1) {
    first = (first + bytes[index]) % 65521;
    second = (second + first) % 65521;
  }
  return ((second << 16) | first) >>> 0;
}

const INFLATE_MAX_BITS = 15;
const INFLATE_LENGTH_BASE = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const INFLATE_LENGTH_EXTRA = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const INFLATE_DISTANCE_BASE = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const INFLATE_DISTANCE_EXTRA = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
const INFLATE_CODE_LENGTH_ORDER = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

function inflateHuffman(lengths, offset, count) {
  const counts = new Uint16Array(INFLATE_MAX_BITS + 1);
  const symbols = new Uint16Array(count);
  for (let symbol = 0; symbol < count; symbol += 1) counts[lengths[offset + symbol]] += 1;
  if (counts[0] === count) return { counts, symbols, left: 0 };
  let left = 1;
  for (let length = 1; length <= INFLATE_MAX_BITS; length += 1) {
    left = (left << 1) - counts[length];
    if (left < 0) return { counts, symbols, left };
  }
  const offsets = new Uint16Array(INFLATE_MAX_BITS + 1);
  for (let length = 1; length < INFLATE_MAX_BITS; length += 1) offsets[length + 1] = offsets[length] + counts[length];
  for (let symbol = 0; symbol < count; symbol += 1) {
    const length = lengths[offset + symbol];
    if (length !== 0) {
      symbols[offsets[length]] = symbol;
      offsets[length] += 1;
    }
  }
  return { counts, symbols, left };
}

let fixedInflateTables;
function fixedTables() {
  if (fixedInflateTables === undefined) {
    const lengths = new Uint8Array(288 + 30);
    lengths.fill(8, 0, 144);
    lengths.fill(9, 144, 256);
    lengths.fill(7, 256, 280);
    lengths.fill(8, 280, 288);
    lengths.fill(5, 288, 318);
    fixedInflateTables = {
      literal: inflateHuffman(lengths, 0, 288),
      distance: inflateHuffman(lengths, 288, 30),
    };
  }
  return fixedInflateTables;
}

class InflateState {
  constructor(input, start, end, maximumOutput) {
    this.input = input;
    this.position = start;
    this.end = end;
    this.bitBuffer = 0;
    this.bitCount = 0;
    this.output = new Uint8Array(maximumOutput);
    this.outputLength = 0;
  }

  bits(need) {
    let value = this.bitBuffer;
    while (this.bitCount < need) {
      if (this.position >= this.end) fail("Malformed", "IPM1 zlib body is truncated");
      value |= this.input[this.position] << this.bitCount;
      this.position += 1;
      this.bitCount += 8;
    }
    this.bitBuffer = value >>> need;
    this.bitCount -= need;
    return value & ((1 << need) - 1);
  }

  emit(byte) {
    if (this.outputLength >= this.output.length) {
      fail("Malformed", "IPM1 zlib body expands beyond its declared canonical length");
    }
    this.output[this.outputLength] = byte;
    this.outputLength += 1;
  }

  decode(table) {
    let code = 0;
    let first = 0;
    let index = 0;
    for (let length = 1; length <= INFLATE_MAX_BITS; length += 1) {
      code |= this.bits(1);
      const count = table.counts[length];
      if (code - count < first) return table.symbols[index + (code - first)];
      index += count;
      first = (first + count) << 1;
      code <<= 1;
    }
    return fail("Malformed", "IPM1 zlib body contains an invalid Huffman code");
  }

  stored() {
    this.bitBuffer = 0;
    this.bitCount = 0;
    if (this.position + 4 > this.end) fail("Malformed", "IPM1 zlib stored block is truncated");
    const length = this.input[this.position] | (this.input[this.position + 1] << 8);
    const complement = this.input[this.position + 2] | (this.input[this.position + 3] << 8);
    this.position += 4;
    if (length !== (~complement & 0xffff)) fail("Malformed", "IPM1 zlib stored block length is invalid");
    if (this.position + length > this.end) fail("Malformed", "IPM1 zlib stored block is truncated");
    for (let index = 0; index < length; index += 1) this.emit(this.input[this.position + index]);
    this.position += length;
  }

  codes(literal, distance) {
    for (;;) {
      let symbol = this.decode(literal);
      if (symbol < 256) {
        this.emit(symbol);
      } else if (symbol === 256) {
        return;
      } else {
        symbol -= 257;
        if (symbol >= 29) fail("Malformed", "IPM1 zlib body contains an invalid length code");
        const length = INFLATE_LENGTH_BASE[symbol] + this.bits(INFLATE_LENGTH_EXTRA[symbol]);
        const distanceSymbol = this.decode(distance);
        if (distanceSymbol >= 30) fail("Malformed", "IPM1 zlib body contains an invalid distance code");
        const back = INFLATE_DISTANCE_BASE[distanceSymbol] + this.bits(INFLATE_DISTANCE_EXTRA[distanceSymbol]);
        if (back > this.outputLength) fail("Malformed", "IPM1 zlib distance precedes the output");
        for (let index = 0; index < length; index += 1) this.emit(this.output[this.outputLength - back]);
      }
    }
  }

  dynamic() {
    const literalCount = this.bits(5) + 257;
    const distanceCount = this.bits(5) + 1;
    const codeCount = this.bits(4) + 4;
    if (literalCount > 286 || distanceCount > 30) fail("Malformed", "IPM1 zlib dynamic block declares too many codes");
    const lengths = new Uint8Array(19);
    for (let index = 0; index < codeCount; index += 1) lengths[INFLATE_CODE_LENGTH_ORDER[index]] = this.bits(3);
    const lengthTable = inflateHuffman(lengths, 0, 19);
    if (lengthTable.left !== 0) fail("Malformed", "IPM1 zlib code-length code is incomplete");
    const total = literalCount + distanceCount;
    const codeLengths = new Uint8Array(total);
    let index = 0;
    while (index < total) {
      let symbol = this.decode(lengthTable);
      if (symbol < 16) {
        codeLengths[index] = symbol;
        index += 1;
        continue;
      }
      let repeated = 0;
      if (symbol === 16) {
        if (index === 0) fail("Malformed", "IPM1 zlib length repeat has no predecessor");
        repeated = codeLengths[index - 1];
        symbol = 3 + this.bits(2);
      } else if (symbol === 17) {
        symbol = 3 + this.bits(3);
      } else {
        symbol = 11 + this.bits(7);
      }
      if (index + symbol > total) fail("Malformed", "IPM1 zlib code lengths overflow their declared count");
      codeLengths.fill(repeated, index, index + symbol);
      index += symbol;
    }
    if (codeLengths[256] === 0) fail("Malformed", "IPM1 zlib block has no end-of-block code");
    const literal = inflateHuffman(codeLengths, 0, literalCount);
    if (literal.left < 0 || (literal.left > 0 && literalCount - literal.counts[0] !== 1)) {
      fail("Malformed", "IPM1 zlib literal code is invalid");
    }
    const distance = inflateHuffman(codeLengths, literalCount, distanceCount);
    if (distance.left < 0 || (distance.left > 0 && distanceCount - distance.counts[0] !== 1)) {
      fail("Malformed", "IPM1 zlib distance code is invalid");
    }
    this.codes(literal, distance);
  }
}

/** Strict RFC 1950 decoder: canonical 0x78 0x9C header, exact length, Adler-32, no trailing bytes. */
function inflateZlibExact(encoded, expectedLength) {
  if (encoded.length < 6 || encoded[0] !== 0x78 || encoded[1] !== 0x9c) {
    fail("Malformed", "IPM1 zlib header is not canonical");
  }
  const state = new InflateState(encoded, 2, encoded.length - 4, expectedLength);
  let last = 0;
  do {
    last = state.bits(1);
    const type = state.bits(2);
    if (type === 0) state.stored();
    else if (type === 1) state.codes(fixedTables().literal, fixedTables().distance);
    else if (type === 2) state.dynamic();
    else fail("Malformed", "IPM1 zlib body uses a reserved block type");
  } while (last === 0);
  if (state.position !== encoded.length - 4) fail("Malformed", "IPM1 zlib body has trailing bytes");
  if (state.outputLength !== expectedLength) fail("Malformed", "IPM1 zlib body length differs from its header");
  if (readU32be(encoded, encoded.length - 4) !== adler32(state.output)) fail("Malformed", "IPM1 zlib Adler-32 mismatch");
  return state.output;
}

// ---------------------------------------------------------------------------
// Canonical compact Norito (subset used by this suite)
// ---------------------------------------------------------------------------
//
// Mirrors `crates/norito` with the COMPACT_LEN header flag:
// - u8/u16/u32/u64 little endian, bool as 0/1;
// - struct and enum fields are each prefixed by a minimal LEB128 length, except
//   that a `[u8; N]` field is written as `len(N) || N raw bytes`;
// - enums start with a little-endian u32 discriminant;
// - `String` is `len || UTF-8`; `Vec<u8>` is `u64 length || bytes`;
// - other `Vec<T>` is `u64 count || (len || element)*`;
// - `Option<T>` is `0` or `1 || len || value`;
// - `[u8; N]` outside a field position (inside Vec/Option) uses the generic
//   array form `(len(1) || byte)*`.

const NORITO_HEADER_BYTES = 40;
const NORITO_COMPACT_LEN = 0x02;
const NORITO_MAGIC = /* @__PURE__ */ UTF8_ENCODER.encode("NRT0");
const NORITO_SCHEMA_DOMAIN = /* @__PURE__ */ UTF8_ENCODER.encode("norito:v1:type-name\0");
const MAX_U64 = (1n << 64n) - 1n;

const N_U8 = Object.freeze({ kind: "u8" });
const N_U16 = Object.freeze({ kind: "u16" });
const N_U32 = Object.freeze({ kind: "u32" });
const N_U64 = Object.freeze({ kind: "u64" });
const N_BOOL = Object.freeze({ kind: "bool" });
const nString = (maximumBytes) => Object.freeze({ kind: "string", maximumBytes });
const nArray = (length) => Object.freeze({ kind: "array", length });
const nBytes = (maximumBytes) => Object.freeze({ kind: "vec", item: N_U8, maximumItems: maximumBytes });
const nVec = (item, maximumItems) => Object.freeze({ kind: "vec", item, maximumItems });
const nOption = (item) => Object.freeze({ kind: "option", item });

function shortTypeName(schema) {
  return schema.slice(schema.lastIndexOf(":") + 1);
}

function nStruct(schema, fields, options = {}) {
  return Object.freeze({
    kind: "struct",
    name: shortTypeName(schema),
    schema,
    alignment: options.alignment ?? 8,
    fields: Object.freeze(fields.map(([key, type]) => Object.freeze([key, type]))),
  });
}

function nEnum(schema, variants, options = {}) {
  return Object.freeze({
    kind: "enum",
    name: shortTypeName(schema),
    schema,
    alignment: options.alignment ?? 8,
    variants: Object.freeze(variants.map(([key, tag, fields]) => Object.freeze({
      key,
      tag,
      fields: Object.freeze((fields ?? []).map(([fieldKey, type]) => Object.freeze([fieldKey, type]))),
    }))),
  });
}

function varint(value) {
  let remaining = BigInt(value);
  const out = [];
  do {
    let byte = Number(remaining & 0x7fn);
    remaining >>= 7n;
    if (remaining !== 0n) byte |= 0x80;
    out.push(byte);
  } while (remaining !== 0n);
  return Uint8Array.from(out);
}

class NoritoWriter {
  constructor() {
    this.parts = [];
  }

  push(bytes) {
    this.parts.push(bytes);
  }

  finish() {
    return concatBytes(...this.parts);
  }
}

function encodeNoritoValue(type, value, context) {
  const writer = new NoritoWriter();
  writeNoritoValue(writer, type, value, context);
  return writer.finish();
}

function writeNoritoField(writer, type, value, context) {
  if (type.kind === "array") {
    const raw = fixedBytes(value, type.length, context);
    writer.push(varint(type.length));
    writer.push(raw);
    return;
  }
  const payload = encodeNoritoValue(type, value, context);
  writer.push(varint(payload.length));
  writer.push(payload);
}

function writeNoritoElement(writer, type, value, context) {
  const payload = encodeNoritoValue(type, value, context);
  writer.push(varint(payload.length));
  writer.push(payload);
}

function unsignedValue(value, maximum, context) {
  let normalized;
  if (typeof value === "bigint") normalized = value;
  else if (typeof value === "number" && Number.isSafeInteger(value)) normalized = BigInt(value);
  else fail("InvalidArgument", `${context} must be an unsigned integer`);
  if (normalized < 0n || normalized > maximum) fail("InvalidArgument", `${context} is out of range`);
  return normalized;
}

function writeNoritoValue(writer, type, value, context) {
  switch (type.kind) {
    case "u8": writer.push(Uint8Array.of(Number(unsignedValue(value, 0xffn, context)))); return;
    case "u16": writer.push(littleEndian(unsignedValue(value, 0xffffn, context), 2, context)); return;
    case "u32": writer.push(littleEndian(unsignedValue(value, 0xffff_ffffn, context), 4, context)); return;
    case "u64": writer.push(littleEndian(unsignedValue(value, MAX_U64, context), 8, context)); return;
    case "bool":
      if (typeof value !== "boolean") fail("InvalidArgument", `${context} must be a boolean`);
      writer.push(Uint8Array.of(value ? 1 : 0));
      return;
    case "string": {
      if (typeof value !== "string") fail("InvalidArgument", `${context} must be a string`);
      const raw = UTF8_ENCODER.encode(value);
      if (raw.length > type.maximumBytes) fail("InvalidArgument", `${context} exceeds ${type.maximumBytes} bytes`);
      writer.push(varint(raw.length));
      writer.push(raw);
      return;
    }
    case "array": {
      const raw = fixedBytes(value, type.length, context);
      const out = new Uint8Array(raw.length * 2);
      for (let index = 0; index < raw.length; index += 1) {
        out[index * 2] = 1;
        out[index * 2 + 1] = raw[index];
      }
      writer.push(out);
      return;
    }
    case "vec": {
      if (type.item.kind === "u8") {
        const raw = asBytes(value, context);
        if (raw.length > type.maximumItems) fail("InvalidArgument", `${context} exceeds ${type.maximumItems} bytes`);
        writer.push(littleEndian(raw.length, 8, context));
        writer.push(raw);
        return;
      }
      if (!Array.isArray(value)) fail("InvalidArgument", `${context} must be an array`);
      if (value.length > type.maximumItems) fail("InvalidArgument", `${context} exceeds ${type.maximumItems} entries`);
      writer.push(littleEndian(value.length, 8, context));
      value.forEach((item, index) => writeNoritoElement(writer, type.item, item, `${context}[${index}]`));
      return;
    }
    case "option":
      if (value === null || value === undefined) {
        writer.push(Uint8Array.of(0));
      } else {
        writer.push(Uint8Array.of(1));
        writeNoritoElement(writer, type.item, value, context);
      }
      return;
    case "struct":
      if (value === null || typeof value !== "object") fail("InvalidArgument", `${context} must be an object`);
      for (const [key, fieldType] of type.fields) writeNoritoField(writer, fieldType, value[key], `${context}.${key}`);
      return;
    case "enum": {
      if (value === null || typeof value !== "object" || typeof value.type !== "string") {
        fail("InvalidArgument", `${context} must be an enum object with a string type`);
      }
      const variant = type.variants.find((candidate) => candidate.key === value.type);
      if (variant === undefined) fail("InvalidArgument", `${context} has unknown variant ${value.type}`);
      writer.push(littleEndian(variant.tag, 4, context));
      for (const [key, fieldType] of variant.fields) {
        writeNoritoField(writer, fieldType, value[key], `${context}.${key}`);
      }
      return;
    }
    default:
      fail("InvalidArgument", `${context} has an unsupported Norito type`);
  }
}

class NoritoReader {
  constructor(bytes, start, end, context) {
    this.bytes = bytes;
    this.offset = start;
    this.end = end;
    this.context = context;
  }

  remaining() {
    return this.end - this.offset;
  }

  take(length, context) {
    if (length > this.remaining()) fail("Malformed", `${context} is truncated`);
    const start = this.offset;
    this.offset += length;
    return this.bytes.subarray(start, this.offset);
  }

  varint(context) {
    let value = 0n;
    let shift = 0n;
    for (let used = 0; used < 10; used += 1) {
      if (this.offset >= this.end) fail("Malformed", `${context} length is truncated`);
      const byte = this.bytes[this.offset];
      this.offset += 1;
      if (used === 9 && (byte & 0xfe) !== 0) fail("Malformed", `${context} length exceeds u64`);
      value |= BigInt(byte & 0x7f) << shift;
      if ((byte & 0x80) === 0) {
        if (used > 0 && byte === 0) fail("Malformed", `${context} length is not minimal`);
        return value;
      }
      shift += 7n;
    }
    return fail("Malformed", `${context} length is not a valid varint`);
  }

  length(context) {
    const value = this.varint(context);
    if (value > BigInt(this.remaining())) fail("Malformed", `${context} length exceeds the available bytes`);
    return Number(value);
  }

  u64Count(context) {
    const value = readLittleEndian(this.take(8, context), 0, 8);
    if (value > BigInt(this.remaining())) fail("Malformed", `${context} count exceeds the available bytes`);
    return Number(value);
  }

  finish(context) {
    if (this.offset !== this.end) fail("Malformed", `${context} has trailing bytes`);
  }
}

function readNoritoField(reader, type, context) {
  const length = reader.length(context);
  if (type.kind === "array") {
    if (length !== type.length) fail("Malformed", `${context} must contain ${type.length} bytes`);
    return Uint8Array.from(reader.take(length, context));
  }
  const start = reader.offset;
  reader.take(length, context);
  const inner = new NoritoReader(reader.bytes, start, start + length, context);
  const value = readNoritoValue(inner, type, context);
  inner.finish(context);
  return value;
}

function readNoritoValue(reader, type, context) {
  switch (type.kind) {
    case "u8": return reader.take(1, context)[0];
    case "u16": return Number(readLittleEndian(reader.take(2, context), 0, 2));
    case "u32": return Number(readLittleEndian(reader.take(4, context), 0, 4));
    case "u64": return readLittleEndian(reader.take(8, context), 0, 8);
    case "bool": {
      const byte = reader.take(1, context)[0];
      if (byte > 1) fail("Malformed", `${context} is not a canonical boolean`);
      return byte === 1;
    }
    case "string": {
      const length = reader.length(context);
      if (length > type.maximumBytes) fail("Malformed", `${context} exceeds ${type.maximumBytes} bytes`);
      try {
        return UTF8_DECODER.decode(reader.take(length, context));
      } catch {
        return fail("Malformed", `${context} is not valid UTF-8`);
      }
    }
    case "array": {
      const raw = reader.take(type.length * 2, context);
      const out = new Uint8Array(type.length);
      for (let index = 0; index < type.length; index += 1) {
        if (raw[index * 2] !== 1) fail("Malformed", `${context} is not a canonical byte array`);
        out[index] = raw[index * 2 + 1];
      }
      return out;
    }
    case "vec": {
      const count = reader.u64Count(context);
      if (count > type.maximumItems) fail("Malformed", `${context} exceeds ${type.maximumItems} entries`);
      if (type.item.kind === "u8") return Uint8Array.from(reader.take(count, context));
      const out = [];
      for (let index = 0; index < count; index += 1) {
        out.push(readNoritoField(reader, type.item.kind === "array" ? { kind: "element", item: type.item } : type.item, `${context}[${index}]`));
      }
      return out;
    }
    case "element": {
      // Generic-position value (Vec/Option element); used for `[u8; N]` elements.
      return readNoritoValue(reader, type.item, context);
    }
    case "option": {
      const tag = reader.take(1, context)[0];
      if (tag === 0) return null;
      if (tag !== 1) fail("Malformed", `${context} has an invalid option tag`);
      return readNoritoField(reader, type.item.kind === "array" ? { kind: "element", item: type.item } : type.item, context);
    }
    case "struct": {
      const out = {};
      for (const [key, fieldType] of type.fields) out[key] = readNoritoField(reader, fieldType, `${context}.${key}`);
      return out;
    }
    case "enum": {
      const tag = Number(readLittleEndian(reader.take(4, context), 0, 4));
      const variant = type.variants.find((candidate) => candidate.tag === tag);
      if (variant === undefined) fail("Malformed", `${context} has unknown discriminant ${tag}`);
      const out = { type: variant.key };
      for (const [key, fieldType] of variant.fields) out[key] = readNoritoField(reader, fieldType, `${context}.${key}`);
      return out;
    }
    default:
      return fail("Malformed", `${context} has an unsupported Norito type`);
  }
}

const SCHEMA_HASH_CACHE = new Map();
function noritoSchemaHash(schemaName) {
  let hash = SCHEMA_HASH_CACHE.get(schemaName);
  if (hash === undefined) {
    hash = sha256(concatBytes(NORITO_SCHEMA_DOMAIN, UTF8_ENCODER.encode(schemaName))).slice(0, 16);
    SCHEMA_HASH_CACHE.set(schemaName, hash);
  }
  return hash;
}

function noritoPadding(alignment) {
  return alignment <= 1 ? 0 : (alignment - (NORITO_HEADER_BYTES % alignment)) % alignment;
}

/** Encode `value` as one canonical compact Norito frame of the named top-level type. */
function encodeNoritoFrame(type, value) {
  const payload = encodeNoritoValue(type, value, type.name);
  const header = new Uint8Array(NORITO_HEADER_BYTES);
  header.set(NORITO_MAGIC, 0);
  header.set(noritoSchemaHash(type.schema), 6);
  header.set(littleEndian(payload.length, 8, "Norito payload length"), 23);
  header.set(littleEndian(crc64Xz(payload), 8, "Norito checksum"), 31);
  header[39] = NORITO_COMPACT_LEN;
  return concatBytes(header, new Uint8Array(noritoPadding(type.alignment)), payload);
}

/** Return the top-level type whose schema hash heads `bytes`, or null. */
function noritoFrameType(bytes, types) {
  if (bytes.length < NORITO_HEADER_BYTES || !equalBytes(bytes.subarray(0, 4), NORITO_MAGIC)) return null;
  const schemaHash = bytes.subarray(6, 22);
  return types.find((type) => equalBytes(noritoSchemaHash(type.schema), schemaHash)) ?? null;
}

/**
 * Decode one canonical Norito frame. Checks magic, version, schema, compression,
 * flags, exact padding, length, CRC64 and that re-encoding reproduces the bytes.
 */
function decodeNoritoFrame(type, raw, maximumBytes) {
  const context = type.name;
  const bytes = asBytes(raw, context);
  if (bytes.length > maximumBytes) fail("Oversized", `${context} exceeds ${maximumBytes} bytes`);
  if (bytes.length < NORITO_HEADER_BYTES) fail("Malformed", `${context} is shorter than a Norito header`);
  if (!equalBytes(bytes.subarray(0, 4), NORITO_MAGIC)) fail("Malformed", `${context} is not an NRT0 frame`);
  if (bytes[4] !== 0 || bytes[5] !== 0) fail("Malformed", `${context} uses an unsupported Norito version`);
  if (!equalBytes(bytes.subarray(6, 22), noritoSchemaHash(type.schema))) fail("WrongSchema", `${context} schema hash does not match`);
  if (bytes[22] !== 0) fail("Malformed", `${context} must not be compressed`);
  if (bytes[39] !== NORITO_COMPACT_LEN) fail("Malformed", `${context} must use exactly the compact-length layout flag`);
  const padding = noritoPadding(type.alignment);
  const payloadLength = readLittleEndian(bytes, 23, 8);
  if (payloadLength !== BigInt(bytes.length - NORITO_HEADER_BYTES - padding)) fail("Malformed", `${context} payload length is inconsistent`);
  if (!isZeroBytes(bytes.subarray(NORITO_HEADER_BYTES, NORITO_HEADER_BYTES + padding))) fail("Malformed", `${context} padding is not zero`);
  const payloadStart = NORITO_HEADER_BYTES + padding;
  const payload = bytes.subarray(payloadStart);
  if (readLittleEndian(bytes, 31, 8) !== crc64Xz(payload)) fail("Malformed", `${context} checksum mismatch`);
  const reader = new NoritoReader(bytes, payloadStart, bytes.length, context);
  const value = readNoritoValue(reader, type, context);
  reader.finish(context);
  if (!equalBytes(encodeNoritoFrame(type, value), bytes)) fail("Malformed", `${context} is not canonical`);
  return value;
}

// ---------------------------------------------------------------------------
// Hashing and P-256 signatures
// ---------------------------------------------------------------------------

const P256_ORDER = 0xffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551n;
const P256_HALF_ORDER = P256_ORDER >> 1n;

/**
 * Verify a 64-byte `r || s` ECDSA P-256 signature over a 32-byte SHA-256 digest
 * with a 65-byte uncompressed SEC1 key. High-S signatures are rejected.
 */
function verifyP256Digest(publicKey, digest, signature) {
  if (!(publicKey instanceof Uint8Array) || publicKey.length !== 65 || publicKey[0] !== 0x04) return false;
  if (!(signature instanceof Uint8Array) || signature.length !== 64) return false;
  if (!(digest instanceof Uint8Array) || digest.length !== 32) return false;
  const r = bigEndianUint(signature.subarray(0, 32));
  const s = bigEndianUint(signature.subarray(32, 64));
  if (r === 0n || r >= P256_ORDER || s === 0n || s > P256_HALF_ORDER) return false;
  try {
    return p256.verify({ r, s }, digest, publicKey, { prehash: false, lowS: true });
  } catch {
    return false;
  }
}

/** True when `publicKey` is a 65-byte uncompressed SEC1 point on P-256. */
function isP256PublicKey(publicKey) {
  if (!(publicKey instanceof Uint8Array) || publicKey.length !== 65 || publicKey[0] !== 0x04) return false;
  try {
    const Point = p256.Point ?? p256.ProjectivePoint;
    Point.fromHex(publicKey).assertValidity();
    return true;
  } catch {
    return false;
  }
}

function blake2b256(bytes) {
  return blake2b(bytes, { dkLen: 32 });
}

// ---------------------------------------------------------------------------
// IPM1 peer envelope (profile KAGEMUSHA_ATTESTED_V1 = 2)
// ---------------------------------------------------------------------------

const IPM1_MAGIC = /* @__PURE__ */ UTF8_ENCODER.encode("IPM1");
const IPM1_WIRE_VERSION = 1;
const IPM1_HEADER_BYTES = 84;
const IPM1_ENCODING_NONE = 0;
const IPM1_ENCODING_ZLIB = 1;
const IPM1_CANONICAL_DOMAIN = /* @__PURE__ */ UTF8_ENCODER.encode("IROHA-PEER-PAYLOAD-V1\0");
const IPM1_WIRE_DOMAIN = /* @__PURE__ */ UTF8_ENCODER.encode("IROHA-PEER-MESSAGE-V1\0");
const IPM1_PROFILE_ATTESTED = 2;
const IPM1_SCHEMA_VERSION_ATTESTED = 1;
const QR_SHARD_BYTES = 256;
const QR_TEXT_PREFIX = "IQR1:";
const QR_TEXT_SUFFIX = ":";
const QR_MAXIMUM_TEXT_BYTES = 700;
const QR_HEADER_REPEAT_INTERVAL = 12;
const QR_FRAME_MAGIC = /* @__PURE__ */ UTF8_ENCODER.encode("IRQR");
const QR_FRAME_VERSION = 1;
const QR_PAYLOAD_OFFSET = 32;
const QR_CHECKSUM_BYTES = 4;
const QR_FRAME_COMPLETE = 0;
const QR_FRAME_HEADER = 1;
const QR_FRAME_DATA = 2;
const QR_FRAME_PARITY = 3;
const TEXT_PREFIX = "kga1:";

function shardCount(byteCount) {
  return Math.ceil(byteCount / QR_SHARD_BYTES);
}

function ipm1CanonicalHash(kindTag, canonical) {
  return blake2b256(concatBytes(
    IPM1_CANONICAL_DOMAIN,
    u16be(IPM1_PROFILE_ATTESTED),
    Uint8Array.of(kindTag),
    u16be(IPM1_SCHEMA_VERSION_ATTESTED),
    canonical,
  ));
}

/** Uncompressed IPM1 encoding of one canonical attested peer value. */
function encodeIpm1(kindTag, canonical) {
  const prefix = concatBytes(
    IPM1_MAGIC,
    Uint8Array.of(IPM1_WIRE_VERSION, IPM1_ENCODING_NONE),
    u16be(IPM1_PROFILE_ATTESTED),
    Uint8Array.of(kindTag, 0),
    u16be(IPM1_SCHEMA_VERSION_ATTESTED),
    u32be(canonical.length),
    u32be(canonical.length),
    ipm1CanonicalHash(kindTag, canonical),
  );
  const wireHash = blake2b256(concatBytes(IPM1_WIRE_DOMAIN, prefix, canonical));
  return concatBytes(prefix, wireHash, canonical);
}

function inspectIpm1Header(header) {
  if (header.length !== IPM1_HEADER_BYTES) fail("Malformed", "IPM1 header must be 84 bytes");
  if (!equalBytes(header.subarray(0, 4), IPM1_MAGIC)) fail("Malformed", "IPM1 magic mismatch");
  if (header[4] !== IPM1_WIRE_VERSION) fail("Malformed", `unsupported IPM1 wire version ${header[4]}`);
  const encoding = header[5];
  if (encoding !== IPM1_ENCODING_NONE && encoding !== IPM1_ENCODING_ZLIB) fail("Malformed", `unsupported IPM1 encoding ${encoding}`);
  const profile = readU16be(header, 6);
  if (profile !== IPM1_PROFILE_ATTESTED) fail("WrongProfile", `IPM1 profile ${profile} is not KAGEMUSHA_ATTESTED_V1 (2)`);
  const kind = messageKindByTag(header[8]);
  if (header[9] !== 0) fail("Malformed", "IPM1 flags must be zero");
  const schemaVersion = readU16be(header, 10);
  if (schemaVersion !== IPM1_SCHEMA_VERSION_ATTESTED) fail("Malformed", `IPM1 profile 2 requires schema version 1, received ${schemaVersion}`);
  const canonicalLength = readU32be(header, 12);
  const encodedLength = readU32be(header, 16);
  if (canonicalLength === 0 || encodedLength === 0) fail("Malformed", "IPM1 payload must not be empty");
  if (canonicalLength > kind.maximumBytes) fail("Oversized", `IPM1 ${kind.key} exceeds ${kind.maximumBytes} bytes`);
  if (encodedLength > maximumPeerBytes()) fail("Oversized", "IPM1 encoded body exceeds the profile bound");
  if (encoding === IPM1_ENCODING_NONE && canonicalLength !== encodedLength) fail("Malformed", "IPM1 declared lengths differ");
  if (encoding === IPM1_ENCODING_ZLIB && !(canonicalLength - encodedLength >= 32
    && shardCount(encodedLength) < shardCount(canonicalLength))) {
    fail("Malformed", "IPM1 zlib body does not satisfy the peer compression policy");
  }
  return {
    encoding,
    kind,
    canonicalLength,
    encodedLength,
    canonicalHash: header.slice(20, 52),
    wireHash: header.slice(52, 84),
    streamId: header.slice(52, 68),
    dataShardCount: shardCount(encodedLength),
    bytes: Uint8Array.from(header),
  };
}

/** Decode and fully verify an IPM1 message; returns the header and canonical payload. */
function decodeIpm1(bytes) {
  if (bytes.length < IPM1_HEADER_BYTES) fail("Malformed", "IPM1 message is shorter than its header");
  const header = inspectIpm1Header(bytes.subarray(0, IPM1_HEADER_BYTES));
  if (bytes.length !== IPM1_HEADER_BYTES + header.encodedLength) fail("Malformed", "IPM1 body length mismatch");
  const body = bytes.subarray(IPM1_HEADER_BYTES);
  const wireHash = blake2b256(concatBytes(IPM1_WIRE_DOMAIN, bytes.subarray(0, 52), body));
  if (!equalBytes(wireHash, header.wireHash)) fail("Malformed", "IPM1 wire hash mismatch");
  const canonical = header.encoding === IPM1_ENCODING_ZLIB
    ? inflateZlibExact(body, header.canonicalLength)
    : Uint8Array.from(body);
  if (!equalBytes(ipm1CanonicalHash(header.kind.tag, canonical), header.canonicalHash)) {
    fail("Malformed", "IPM1 canonical hash mismatch");
  }
  return { header, canonical };
}

// ---------------------------------------------------------------------------
// IQR1 / IRQR QR frames
// ---------------------------------------------------------------------------

function encodeQrFrame(frameKind, kindTag, streamId, index, total, payload) {
  const prefix = concatBytes(
    QR_FRAME_MAGIC,
    Uint8Array.of(QR_FRAME_VERSION, frameKind),
    u16be(IPM1_PROFILE_ATTESTED),
    Uint8Array.of(kindTag, 0),
    streamId,
    u16be(index),
    u16be(total),
    u16be(payload.length),
    payload,
  );
  return `${QR_TEXT_PREFIX}${base45Encode(concatBytes(prefix, u32be(crc32c(prefix))))}${QR_TEXT_SUFFIX}`;
}

function decodeQrFrame(text) {
  if (typeof text !== "string") fail("InvalidArgument", "QR frame must be a string");
  if (UTF8_ENCODER.encode(text).length > QR_MAXIMUM_TEXT_BYTES) fail("Oversized", `IQR1 frame text exceeds ${QR_MAXIMUM_TEXT_BYTES} bytes`);
  if (!text.startsWith(QR_TEXT_PREFIX) || !text.endsWith(QR_TEXT_SUFFIX) || text.length <= QR_TEXT_PREFIX.length + QR_TEXT_SUFFIX.length) {
    fail("Malformed", "malformed IQR1 frame text");
  }
  const bytes = base45Decode(text.slice(QR_TEXT_PREFIX.length, text.length - QR_TEXT_SUFFIX.length));
  if (base45Encode(bytes) !== text.slice(QR_TEXT_PREFIX.length, text.length - QR_TEXT_SUFFIX.length)) {
    fail("Malformed", "IQR1 body is not canonical Base45");
  }
  if (bytes.length < QR_PAYLOAD_OFFSET + QR_CHECKSUM_BYTES) fail("Malformed", "IRQR frame is truncated");
  if (!equalBytes(bytes.subarray(0, 4), QR_FRAME_MAGIC)) fail("Malformed", "IRQR frame magic mismatch");
  if (bytes[4] !== QR_FRAME_VERSION) fail("Malformed", `unsupported IRQR version ${bytes[4]}`);
  const frameKind = bytes[5];
  if (frameKind > QR_FRAME_PARITY) fail("Malformed", `invalid IRQR frame kind ${frameKind}`);
  const profile = readU16be(bytes, 6);
  if (profile !== IPM1_PROFILE_ATTESTED) fail("WrongProfile", `IRQR profile ${profile} is not KAGEMUSHA_ATTESTED_V1 (2)`);
  const kind = messageKindByTag(bytes[8]);
  if (bytes[9] !== 0) fail("Malformed", "IRQR flags must be zero");
  const payloadLength = readU16be(bytes, 30);
  const payloadEnd = QR_PAYLOAD_OFFSET + payloadLength;
  if (payloadEnd + QR_CHECKSUM_BYTES !== bytes.length) fail("Malformed", "IRQR frame length mismatch");
  if (readU32be(bytes, payloadEnd) !== crc32c(bytes.subarray(0, payloadEnd))) fail("Malformed", "IRQR CRC32C mismatch");
  const index = readU16be(bytes, 26);
  const total = readU16be(bytes, 28);
  const payload = bytes.slice(QR_PAYLOAD_OFFSET, payloadEnd);
  const maximumShards = shardCount(maximumPeerBytes());
  if (total === 0 || total > maximumShards) fail("Malformed", "IRQR shard total is out of range");
  const shapeOk = (frameKind === QR_FRAME_COMPLETE && index === 0 && total === 1
      && payloadLength > IPM1_HEADER_BYTES && payloadLength <= IPM1_HEADER_BYTES + maximumPeerBytes())
    || (frameKind === QR_FRAME_HEADER && index === 0 && payloadLength === IPM1_HEADER_BYTES)
    || (frameKind === QR_FRAME_DATA && index < total && payloadLength === QR_SHARD_BYTES)
    || (frameKind === QR_FRAME_PARITY && index < Math.ceil(total / 2) && payloadLength === QR_SHARD_BYTES);
  if (!shapeOk) fail("Malformed", "IRQR frame fields are inconsistent");
  return {
    frameKind,
    kind,
    streamId: bytes.slice(10, 26),
    index,
    total,
    payload,
    encoded: bytes,
  };
}

/** IQR1 texts for one IPM1 message: one complete frame when it fits, else animated shards. */
function qrFrameTexts(kindTag, ipm1) {
  const header = ipm1.subarray(0, IPM1_HEADER_BYTES);
  const body = ipm1.subarray(IPM1_HEADER_BYTES);
  const streamId = header.slice(52, 68);
  const complete = encodeQrFrame(QR_FRAME_COMPLETE, kindTag, streamId, 0, 1, ipm1);
  if (UTF8_ENCODER.encode(complete).length <= QR_MAXIMUM_TEXT_BYTES) return [complete];
  const dataCount = shardCount(body.length);
  const shards = [];
  for (let index = 0; index < dataCount; index += 1) {
    const shard = new Uint8Array(QR_SHARD_BYTES);
    shard.set(body.subarray(index * QR_SHARD_BYTES, Math.min(body.length, (index + 1) * QR_SHARD_BYTES)));
    shards.push(shard);
  }
  const headerText = encodeQrFrame(QR_FRAME_HEADER, kindTag, streamId, 0, dataCount, header);
  const texts = [headerText];
  let nonHeader = 0;
  const append = (text) => {
    texts.push(text);
    nonHeader += 1;
    if (nonHeader % QR_HEADER_REPEAT_INTERVAL === 0) texts.push(headerText);
  };
  for (let pair = 0; pair < Math.ceil(dataCount / 2); pair += 1) {
    const first = pair * 2;
    append(encodeQrFrame(QR_FRAME_DATA, kindTag, streamId, first, dataCount, shards[first]));
    const parity = Uint8Array.from(shards[first]);
    if (first + 1 < dataCount) {
      append(encodeQrFrame(QR_FRAME_DATA, kindTag, streamId, first + 1, dataCount, shards[first + 1]));
      for (let index = 0; index < QR_SHARD_BYTES; index += 1) parity[index] ^= shards[first + 1][index];
    }
    append(encodeQrFrame(QR_FRAME_PARITY, kindTag, streamId, pair, dataCount, parity));
  }
  return texts;
}

/**
 * Bounded reassembler for animated IQR1 frames. Frames may arrive in any order
 * and repeat; a conflicting duplicate quarantines its stream. One missing data
 * shard per pair is recovered from parity.
 */
export class KagemushaAttestedQrAssembler {
  #streams = new Map();
  #quarantined = new Set();
  #maximumStreams;

  constructor(options = {}) {
    const maximumStreams = options.maximumStreams ?? 3;
    if (!Number.isInteger(maximumStreams) || maximumStreams < 1 || maximumStreams > 3) {
      fail("InvalidArgument", "maximumStreams must be an integer from 1 to 3");
    }
    this.#maximumStreams = maximumStreams;
  }

  reset() {
    this.#streams.clear();
    this.#quarantined.clear();
  }

  /** Ingest one frame text. Returns `{status, streamId, receivedDataShards, totalDataShards, message?}`. */
  push(text) {
    const frame = decodeQrFrame(text);
    const streamKey = toHex(frame.streamId);
    if (this.#quarantined.has(streamKey)) fail("Quarantined", "IQR1 stream is quarantined");
    try {
      return this.#ingest(frame, streamKey);
    } catch (error) {
      this.#streams.delete(streamKey);
      this.#quarantined.add(streamKey);
      throw error;
    }
  }

  #ingest(frame, streamKey) {
    if (frame.frameKind === QR_FRAME_COMPLETE) {
      const message = messageFromIpm1(frame.payload);
      if (!equalBytes(message.streamId, frame.streamId) || message.kind !== frame.kind.key) {
        fail("Malformed", "IQR1 complete frame does not match its IPM1 message");
      }
      this.#streams.delete(streamKey);
      return { status: "completed", streamId: frame.streamId, receivedDataShards: 1, totalDataShards: 1, message };
    }
    let stream = this.#streams.get(streamKey);
    if (stream === undefined) {
      if (this.#streams.size >= this.#maximumStreams) fail("TooManyStreams", "too many active IQR1 streams");
      stream = { kindTag: frame.kind.tag, total: frame.total, frames: new Map(), header: null, data: new Map(), parity: new Map() };
      this.#streams.set(streamKey, stream);
    }
    if (stream.kindTag !== frame.kind.tag || stream.total !== frame.total) fail("Conflict", "IQR1 frame conflicts with its stream");
    const frameKey = `${frame.frameKind}:${frame.index}`;
    const previous = stream.frames.get(frameKey);
    if (previous !== undefined) {
      if (!equalBytes(previous, frame.encoded)) fail("Conflict", "IQR1 stream supplied a conflicting duplicate");
      return this.#progress("duplicate", frame.streamId, stream);
    }
    if (stream.header === null && frame.frameKind !== QR_FRAME_HEADER && stream.frames.size >= 12) {
      fail("Oversized", "IQR1 stream buffered too many frames before its header");
    }
    stream.frames.set(frameKey, frame.encoded);
    if (frame.frameKind === QR_FRAME_HEADER) {
      const header = inspectIpm1Header(frame.payload);
      if (!equalBytes(header.streamId, frame.streamId) || header.dataShardCount !== frame.total || header.kind.tag !== frame.kind.tag) {
        fail("Malformed", "IQR1 header frame does not match its stream");
      }
      stream.header = header;
    } else if (frame.frameKind === QR_FRAME_DATA) {
      const recovered = stream.data.get(frame.index);
      if (recovered !== undefined && !equalBytes(recovered, frame.payload)) fail("Conflict", "IQR1 data shard conflicts with parity recovery");
      stream.data.set(frame.index, frame.payload);
    } else {
      stream.parity.set(frame.index, frame.payload);
    }
    if (stream.header !== null) {
      this.#recover(stream);
      if (stream.data.size === stream.total) {
        const message = this.#finish(stream, frame.streamId);
        this.#streams.delete(streamKey);
        return { status: "completed", streamId: frame.streamId, receivedDataShards: stream.total, totalDataShards: stream.total, message };
      }
    }
    return this.#progress("accepted", frame.streamId, stream);
  }

  #recover(stream) {
    for (const [pair, parity] of stream.parity) {
      const first = pair * 2;
      const indices = first + 1 < stream.total ? [first, first + 1] : [first];
      const missing = indices.filter((index) => !stream.data.has(index));
      if (missing.length !== 1) continue;
      const recovered = Uint8Array.from(parity);
      for (const index of indices) {
        if (index === missing[0]) continue;
        const present = stream.data.get(index);
        for (let byte = 0; byte < QR_SHARD_BYTES; byte += 1) recovered[byte] ^= present[byte];
      }
      stream.data.set(missing[0], recovered);
    }
  }

  #finish(stream, streamId) {
    const padded = new Uint8Array(stream.total * QR_SHARD_BYTES);
    for (let index = 0; index < stream.total; index += 1) padded.set(stream.data.get(index), index * QR_SHARD_BYTES);
    if (!isZeroBytes(padded.subarray(stream.header.encodedLength))) fail("Malformed", "IQR1 shard padding is not zero");
    const message = messageFromIpm1(concatBytes(stream.header.bytes, padded.subarray(0, stream.header.encodedLength)));
    if (!equalBytes(message.streamId, streamId)) fail("Malformed", "IQR1 stream does not match its IPM1 message");
    return message;
  }

  #progress(status, streamId, stream) {
    return {
      status,
      streamId,
      receivedDataShards: stream.data.size,
      totalDataShards: stream.header === null ? 0 : stream.total,
    };
  }
}

// ---------------------------------------------------------------------------
// Suite constants
// ---------------------------------------------------------------------------

const SUITE_ID = "iroha:kagemusha:v1:attested-app";
const DOMAIN_PREFIX = `${SUITE_ID}:`;
const MODEL = "iroha_data_model::kagemusha::kagemusha_attested_v1::";
const WIRE_VERSION = 1;
const MAXIMUM_AMOUNT = 1_000_000_000_000_000n;
const ZERO_32 = new Uint8Array(32);

const TRANSITION_KINDS = Object.freeze({
  bootstrap: 0,
  mintFold: 1,
  sendSplit: 2,
  receiveFold: 3,
  redeemSplit: 4,
  rotate: 5,
});
const TRANSITION_KIND_NAMES = Object.freeze(Object.keys(TRANSITION_KINDS));

const PLATFORMS = Object.freeze({
  androidStrongBox: 1,
  androidTee: 2,
  appleSecureEnclave: 3,
});

const REVOCATION_REASONS = Object.freeze({
  fraud: 0,
  lost: 1,
  integrity: 2,
  superseded: 3,
  closed: 4,
});

/** Receiver refusal reasons, in the order the receive checks run. */
const REFUSAL_REASONS = Object.freeze([
  "Malformed",
  "InvalidCert",
  "InvalidSignature",
  "WrongReceiver",
  "Duplicate",
  "Revoked",
  "Fork",
  "Expired",
  "PayerLimit",
]);

const DOMAIN_CACHE = new Map();
function domain(purpose) {
  let value = DOMAIN_CACHE.get(purpose);
  if (value === undefined) {
    value = UTF8_ENCODER.encode(`${DOMAIN_PREFIX}${purpose}\0`);
    DOMAIN_CACHE.set(purpose, value);
  }
  return value;
}

function domainHash(purpose, ...parts) {
  return sha256(concatBytes(domain(purpose), ...parts));
}

// ---------------------------------------------------------------------------
// Canonical model (§1.4 field order). Every top-level type carries an explicit
// `iroha_data_model::kagemusha::kagemusha_attested_v1::<Type>` schema name.
// ---------------------------------------------------------------------------

const MAX_STRING_BYTES = 1024;
const MAX_ISSUER_KEYS = 16;
const MAX_TIERS = 16;
const MAX_SIGNERS = 8;
const MAX_ROOTS = 32;
const MAX_DELTA_ENTRIES = 8;
const MAX_CRL_ENTRIES = 1_000_000;
const MAX_SEGMENT_TRANSITIONS = 100_000;

const T_ISSUER_KEY = nStruct(`${MODEL}KagemushaAttestedIssuerKeyV1`, [
  ["index", N_U8],
  ["publicKey", nArray(65)],
  ["notBeforeMs", N_U64],
  ["notAfterMs", N_U64],
]);

const T_PLAY_INTEGRITY_POLICY = nStruct(`${MODEL}KagemushaAttestedPlayIntegrityPolicyV1`, [
  ["minDeviceIntegrity", N_U8],
  ["requireRecognized", N_BOOL],
  ["requireLicensed", N_BOOL],
  ["maxEvidenceAgeMs", N_U64],
]);

const T_ANDROID_POLICY = nStruct(`${MODEL}KagemushaAttestedAndroidPolicyV1`, [
  ["package", nString(MAX_STRING_BYTES)],
  ["signerSha256", nVec(nArray(32), MAX_SIGNERS)],
  ["minVersionCode", N_U64],
  ["allowStrongBox", N_BOOL],
  ["allowTee", N_BOOL],
  ["osPatchFloor", N_U32],
  ["playIntegrity", nOption(T_PLAY_INTEGRITY_POLICY)],
]);

const T_APPLE_POLICY = nStruct(`${MODEL}KagemushaAttestedApplePolicyV1`, [
  ["appId", nString(MAX_STRING_BYTES)],
  ["production", N_BOOL],
]);

const T_VENDOR_ROOT = nStruct(`${MODEL}KagemushaAttestedVendorRootV1`, [
  ["vendor", nString(64)],
  ["sha256", nArray(32)],
]);

const T_TIER = nStruct(`${MODEL}KagemushaAttestedTierV1`, [
  ["tier", N_U8],
  ["maxBalance", N_U64],
  ["maxPayment", N_U64],
  ["maxUnsyncedOut", N_U64],
  ["leaseMs", N_U64],
]);

const T_ACCOUNT_LIMITS = nStruct(`${MODEL}KagemushaAttestedAccountLimitsV1`, [
  ["devices", N_U8],
  ["dailyLoad", N_U64],
  ["dailyUnload", N_U64],
]);

const DESCRIPTOR_FIELDS = [
  ["version", N_U16],
  ["schemeId", nArray(32)],
  ["descriptorEpoch", N_U64],
  ["chainId", nString(MAX_STRING_BYTES)],
  ["assetDefinitionId", nString(MAX_STRING_BYTES)],
  ["assetScale", N_U8],
  ["reserveAccountId", nString(MAX_STRING_BYTES)],
  ["issuerUrl", nString(MAX_STRING_BYTES)],
  ["issuerKeys", nVec(T_ISSUER_KEY, MAX_ISSUER_KEYS)],
  ["attestationRoots", nVec(T_VENDOR_ROOT, MAX_ROOTS)],
  ["android", T_ANDROID_POLICY],
  ["apple", T_APPLE_POLICY],
  ["tiers", nVec(T_TIER, MAX_TIERS)],
  ["accountLimits", T_ACCOUNT_LIMITS],
  ["receiverGraceMs", N_U64],
  ["headroomQuantum", N_U64],
  ["allowTestDevices", N_BOOL],
];

const CERT_FIELDS = [
  ["version", N_U16],
  ["schemeId", nArray(32)],
  ["deviceId", nArray(32)],
  ["devicePublicKey", nArray(65)],
  ["platform", N_U8],
  ["tier", N_U8],
  ["certSerial", N_U32],
  ["notBeforeMs", N_U64],
  ["notAfterMs", N_U64],
  ["maxBalance", N_U64],
  ["maxPayment", N_U64],
  ["maxUnsyncedOut", N_U64],
  ["issuerKeyIndex", N_U8],
];

const T_TRANSITION = nStruct(`${MODEL}KagemushaAttestedTransitionV1`, [
  ["version", N_U16],
  ["deviceId", nArray(32)],
  ["seq", N_U64],
  ["prevDigest", nArray(32)],
  ["kind", N_U8],
  ["amount", N_U64],
  ["balanceAfter", N_U64],
  ["subject", nArray(32)],
  ["counterparty", nArray(32)],
  ["ackedSeq", N_U64],
  ["unsyncedOutAfter", N_U64],
  ["crlEpochHeld", N_U64],
  ["deviceTimeMs", N_U64],
]);

const T_REVOCATION_ENTRY = nStruct(`${MODEL}KagemushaAttestedRevocationEntryV1`, [
  ["deviceId", nArray(32)],
  ["reason", N_U8],
  ["epoch", N_U64],
]);

const DELTA_FIELDS = [
  ["schemeId", nArray(32)],
  ["fromEpoch", N_U64],
  ["toEpoch", N_U64],
  ["entries", nVec(T_REVOCATION_ENTRY, MAX_DELTA_ENTRIES)],
  ["keyIndex", N_U8],
];

const CRL_FIELDS = [
  ["schemeId", nArray(32)],
  ["epoch", N_U64],
  ["issuedAtMs", N_U64],
  ["entries", nVec(T_REVOCATION_ENTRY, MAX_CRL_ENTRIES)],
  ["keyIndex", N_U8],
];

/** Build an unsigned body type and its signed counterpart (`body fields || signature [64]`). */
function signedPair(name, fields, signatureKey) {
  return {
    body: nStruct(`${MODEL}${name}BodyV1`, fields),
    signed: nStruct(`${MODEL}${name}V1`, [...fields, [signatureKey, nArray(64)]]),
    signatureKey,
  };
}

const P_DESCRIPTOR = signedPair("KagemushaAttestedSchemeDescriptor", DESCRIPTOR_FIELDS, "rootSignature");
const P_CERT = signedPair("KagemushaAttestedDeviceCert", CERT_FIELDS, "issuerSignature");
const P_DELTA = signedPair("KagemushaAttestedRevocationDelta", DELTA_FIELDS, "signature");
const P_CRL = signedPair("KagemushaAttestedRevocationList", CRL_FIELDS, "signature");

const T_CERT = P_CERT.signed;
const T_DELTA = P_DELTA.signed;

const P_REQUEST = signedPair("KagemushaAttestedPaymentRequest", [
  ["version", N_U16],
  ["receiverCert", T_CERT],
  ["requestNonce", nArray(16)],
  ["amount", N_U64],
  ["headroom", N_U64],
  ["reusable", N_BOOL],
  ["maxUses", N_U16],
  ["createdAtMs", N_U64],
  ["crlEpochHeld", N_U64],
  ["crlDelta", nOption(T_DELTA)],
], "signature");

const T_PAYMENT = nStruct(`${MODEL}KagemushaAttestedPaymentV1`, [
  ["version", N_U16],
  ["payerCert", T_CERT],
  ["transition", T_TRANSITION],
  ["signature", nArray(64)],
  ["crlDelta", nOption(T_DELTA)],
]);

const P_ACK = signedPair("KagemushaAttestedAcknowledgement", [
  ["version", N_U16],
  ["paymentId", nArray(32)],
  ["receiverDeviceId", nArray(32)],
  ["receiveTransitionDigest", nArray(32)],
], "signature");

const P_VOUCHER = signedPair("KagemushaAttestedMintVoucher", [
  ["schemeId", nArray(32)],
  ["voucherId", nArray(32)],
  ["deviceId", nArray(32)],
  ["loadId", nArray(16)],
  ["amount", N_U64],
  ["txHash", nArray(32)],
  ["issuedAtMs", N_U64],
  ["issuerKeyIndex", N_U8],
], "signature");

const P_DELIVERY = signedPair("KagemushaAttestedDelivery", [
  ["schemeId", nArray(32)],
  ["paymentId", nArray(32)],
  ["receiverDeviceId", nArray(32)],
  ["keyIndex", N_U8],
], "signature");

const T_SIGNED_TRANSITION = nStruct(`${MODEL}KagemushaAttestedSignedTransitionV1`, [
  ["transition", T_TRANSITION],
  ["signature", nArray(64)],
]);

const FORK_FIELDS = [
  ["cert", T_CERT],
  ["a", T_TRANSITION],
  ["signatureA", nArray(64)],
  ["b", T_TRANSITION],
  ["signatureB", nArray(64)],
];
const T_FORK_EVIDENCE = nEnum(`${MODEL}KagemushaAttestedForkEvidenceV1`, [
  ["SameSeq", 0, FORK_FIELDS],
  ["Inconsistent", 1, FORK_FIELDS],
]);

// ---------------------------------------------------------------------------
// Derived identifiers. IDs and digests cover unsigned bodies only, so ECDSA
// malleability can never change an identifier.
// ---------------------------------------------------------------------------

function noritoStringBytes(value, context) {
  if (typeof value !== "string") fail("InvalidArgument", `${context} must be a string`);
  const raw = UTF8_ENCODER.encode(value);
  return concatBytes(varint(raw.length), raw);
}

function schemeIdFor({ chainId, assetDefinitionId, reserveAccountId, rootPublicKey }) {
  return domainHash(
    "scheme-id",
    noritoStringBytes(chainId, "chainId"),
    noritoStringBytes(assetDefinitionId, "assetDefinitionId"),
    noritoStringBytes(reserveAccountId, "reserveAccountId"),
    fixedBytes(rootPublicKey, 65, "rootPublicKey"),
  );
}

function deviceIdFor(schemeId, devicePublicKey) {
  return domainHash("device-id", fixedBytes(schemeId, 32, "schemeId"), fixedBytes(devicePublicKey, 65, "devicePublicKey"));
}

function transitionDigestOf(transition) {
  return domainHash("transition", encodeNoritoFrame(T_TRANSITION, transition));
}

function signedDigest(purpose, pair, value) {
  return domainHash(purpose, encodeNoritoFrame(pair.body, value));
}

const certificateDigestOf = (cert) => signedDigest("cert", P_CERT, cert);
const descriptorDigestOf = (descriptor) => signedDigest("descriptor", P_DESCRIPTOR, descriptor);
const requestDigestOf = (request) => signedDigest("request", P_REQUEST, request);
const acknowledgementDigestOf = (ack) => signedDigest("ack", P_ACK, ack);
const voucherDigestOf = (voucher) => signedDigest("voucher", P_VOUCHER, voucher);
const revocationListDigestOf = (crl) => signedDigest("crl", P_CRL, crl);
const revocationDeltaDigestOf = (delta) => signedDigest("crl-delta", P_DELTA, delta);
const deliveryDigestOf = (delivery) => signedDigest("delivery", P_DELIVERY, delivery);

function voucherIdFor(txHash, loadId) {
  return domainHash("voucher", fixedBytes(txHash, 32, "txHash"), fixedBytes(loadId, 16, "loadId"));
}

function redemptionIdFor(deviceId, seq, accountDigest, amount) {
  return domainHash(
    "redemption-id",
    fixedBytes(deviceId, 32, "deviceId"),
    littleEndian(unsignedValue(seq, MAX_U64, "seq"), 8, "seq"),
    fixedBytes(accountDigest, 32, "accountDigest"),
    littleEndian(unsignedValue(amount, MAX_U64, "amount"), 8, "amount"),
  );
}

function loadBindingFor(schemeId, deviceId, loadId, amount) {
  return domainHash(
    "load-binding",
    fixedBytes(schemeId, 32, "schemeId"),
    fixedBytes(deviceId, 32, "deviceId"),
    fixedBytes(loadId, 16, "loadId"),
    littleEndian(unsignedValue(amount, MAX_U64, "amount"), 8, "amount"),
  );
}

// ---------------------------------------------------------------------------
// Peer messages: kinds, `kga1:` text, IPM1 and IQR1
// ---------------------------------------------------------------------------

const MAXIMUM_REQUEST_BYTES = 2048;
const MAXIMUM_PAYMENT_BYTES = 2048;
const MAXIMUM_ACKNOWLEDGEMENT_BYTES = 512;

const MESSAGE_KINDS = Object.freeze([
  Object.freeze({ key: "request", tag: 1, type: P_REQUEST.signed, maximumBytes: MAXIMUM_REQUEST_BYTES }),
  Object.freeze({ key: "payment", tag: 2, type: T_PAYMENT, maximumBytes: MAXIMUM_PAYMENT_BYTES }),
  Object.freeze({ key: "acknowledgement", tag: 3, type: P_ACK.signed, maximumBytes: MAXIMUM_ACKNOWLEDGEMENT_BYTES }),
]);

function messageKindByTag(tag) {
  const kind = MESSAGE_KINDS.find((candidate) => candidate.tag === tag);
  if (kind === undefined) fail("Malformed", `unknown KAGEMUSHA attested peer kind ${tag}`);
  return kind;
}

function messageKindByKey(key) {
  const kind = MESSAGE_KINDS.find((candidate) => candidate.key === key);
  if (kind === undefined) fail("InvalidArgument", `unknown KAGEMUSHA attested peer kind ${String(key)}`);
  return kind;
}

function maximumPeerBytes() {
  return Math.max(MAXIMUM_REQUEST_BYTES, MAXIMUM_PAYMENT_BYTES, MAXIMUM_ACKNOWLEDGEMENT_BYTES);
}

function maximumTextBytes(rawBytes) {
  return TEXT_PREFIX.length + Math.ceil((rawBytes * 4) / 3);
}

/** One decoded, canonical peer message (Request, Payment or Acknowledgement). */
export class KagemushaAttestedMessage {
  #kind;
  #canonical;
  #value;

  constructor(kind, canonical, value) {
    this.#kind = kind;
    this.#canonical = canonical;
    this.#value = value;
    Object.freeze(this);
  }

  /** `"request" | "payment" | "acknowledgement"`. */
  get kind() { return this.#kind.key; }
  /** IPM1 kind tag (1, 2 or 3). */
  get kindTag() { return this.#kind.tag; }
  /** Exact canonical Norito bytes (a copy). */
  get canonical() { return Uint8Array.from(this.#canonical); }
  /** Decoded value (a deep copy). */
  get value() { return structuredClone(this.#value); }
  /** First 16 bytes of the IPM1 wire hash of the uncompressed encoding. */
  get streamId() { return this.ipm1().slice(52, 68); }

  /** `kga1:` plus unpadded base64url of the canonical bytes. */
  text() { return `${TEXT_PREFIX}${base64UrlEncode(this.#canonical)}`; }
  /** Uncompressed IPM1 envelope (profile 2, schema version 1). */
  ipm1() { return encodeIpm1(this.#kind.tag, this.#canonical); }
  /** IQR1 frame texts: a single complete frame when it fits 700 bytes, else animated shards. */
  qrFrames() { return qrFrameTexts(this.#kind.tag, this.ipm1()); }
}

function messageFromCanonical(canonical, expectedKind = undefined) {
  const kind = expectedKind ?? MESSAGE_KINDS.find((candidate) => noritoFrameType(canonical, [candidate.type]) !== null);
  if (kind === undefined) fail("WrongSchema", "bytes are not a KAGEMUSHA attested peer message");
  const value = decodeNoritoFrame(kind.type, canonical, kind.maximumBytes);
  return new KagemushaAttestedMessage(kind, Uint8Array.from(canonical), value);
}

function messageFromIpm1(bytes) {
  const { header, canonical } = decodeIpm1(bytes);
  return messageFromCanonical(canonical, header.kind);
}

function messageFromText(text) {
  if (!text.startsWith(TEXT_PREFIX)) fail("Malformed", `text must start with ${TEXT_PREFIX}`);
  if (text.length > maximumTextBytes(maximumPeerBytes())) fail("Oversized", "kga1 text exceeds the peer bound");
  const canonical = base64UrlDecode(text.slice(TEXT_PREFIX.length), "kga1 text");
  return messageFromCanonical(canonical);
}

function messageFromQrFrames(frames) {
  if (frames.length === 0) fail("InvalidArgument", "at least one QR frame is required");
  const assembler = new KagemushaAttestedQrAssembler({ maximumStreams: 1 });
  let completed = null;
  for (const frame of frames) {
    const result = assembler.push(frame);
    if (result.status === "completed") {
      if (completed !== null && !equalBytes(completed.canonical, result.message.canonical)) {
        fail("Conflict", "QR frames complete more than one message");
      }
      completed = result.message;
    }
  }
  if (completed === null) fail("Incomplete", "QR frames do not yet complete a message");
  return completed;
}

/**
 * Decode one attested peer message from `kga1:` text, one `IQR1:` frame, an
 * array of `IQR1:` frames, IPM1 bytes or canonical Norito bytes.
 */
export function decodeKagemushaAttestedMessage(input) {
  if (input instanceof KagemushaAttestedMessage) return input;
  if (typeof input === "string") {
    if (input.startsWith(QR_TEXT_PREFIX)) return messageFromQrFrames([input]);
    return messageFromText(input);
  }
  if (Array.isArray(input)) {
    if (!input.every((frame) => typeof frame === "string")) fail("InvalidArgument", "QR frames must be strings");
    return messageFromQrFrames(input);
  }
  const bytes = asBytes(input, "peer message");
  if (bytes.length >= 4 && equalBytes(bytes.subarray(0, 4), IPM1_MAGIC)) return messageFromIpm1(bytes);
  return messageFromCanonical(bytes);
}

/** Wrap canonical bytes (or a decoded value of `kind`) as a peer message for text/QR/IPM1 rendering. */
export function encodeKagemushaAttestedMessage(kind, value) {
  const messageKind = messageKindByKey(kind);
  const canonical = value instanceof Uint8Array || value instanceof ArrayBuffer || ArrayBuffer.isView(value)
    ? asBytes(value, kind)
    : encodeNoritoFrame(messageKind.type, value);
  return messageFromCanonical(canonical, messageKind);
}

// ---------------------------------------------------------------------------
// Scheme verifier
// ---------------------------------------------------------------------------

function deviceKey(deviceId) {
  return toHex(deviceId);
}

function refusal(reason, extra = {}) {
  return Object.freeze({ ok: false, reason, ...extra });
}

function decodeSigned(pair, input, maximumBytes) {
  if (input !== null && typeof input === "object" && !(input instanceof Uint8Array)
    && !(input instanceof ArrayBuffer) && !ArrayBuffer.isView(input)) {
    // Re-encode decoded objects so every check runs on canonical values.
    return decodeNoritoFrame(pair.signed ?? pair, encodeNoritoFrame(pair.signed ?? pair, input), maximumBytes);
  }
  return decodeNoritoFrame(pair.signed ?? pair, input, maximumBytes);
}

/** Normalize a CRL-like input into a Map of revoked device-id hex → reason. */
function revokedSet(crl) {
  const revoked = new Map();
  if (crl === null || crl === undefined) return revoked;
  const entries = Array.isArray(crl?.entries) ? crl.entries : crl;
  if (entries instanceof Map) {
    for (const [key, reason] of entries) revoked.set(typeof key === "string" ? key.toLowerCase() : deviceKey(key), reason);
    return revoked;
  }
  if (entries instanceof Set || Array.isArray(entries)) {
    for (const entry of entries) {
      if (typeof entry === "string") revoked.set(entry.toLowerCase(), null);
      else if (entry instanceof Uint8Array) revoked.set(deviceKey(entry), null);
      else if (entry?.deviceId instanceof Uint8Array) revoked.set(deviceKey(entry.deviceId), entry.reason ?? null);
      else fail("InvalidArgument", "CRL entries must be device ids or revocation entries");
    }
    return revoked;
  }
  return fail("InvalidArgument", "crl must be a revocation list, an array, a Set or a Map");
}

/** Verified scheme descriptor pinned to an owner root key and scheme id. */
export class KagemushaAttestedScheme {
  #descriptor;
  #descriptorBytes;
  #descriptorDigest;
  #rootPublicKey;
  #issuerKeys;
  #tiers;

  constructor(token, descriptor, descriptorBytes, rootPublicKey) {
    if (token !== SCHEME_TOKEN) fail("InvalidArgument", "use KagemushaAttestedScheme.fromDescriptor");
    this.#descriptor = descriptor;
    this.#descriptorBytes = descriptorBytes;
    this.#descriptorDigest = descriptorDigestOf(descriptor);
    this.#rootPublicKey = rootPublicKey;
    this.#issuerKeys = new Map(descriptor.issuerKeys.map((key) => [key.index, key]));
    this.#tiers = new Map(descriptor.tiers.map((tier) => [tier.tier, tier]));
    Object.freeze(this);
  }

  /**
   * Verify a root-signed descriptor against the pinned root key and scheme id.
   * Throws unless the descriptor is authentic. A descriptor that admits test
   * devices is refused unless `testing: true` (debug builds only).
   */
  static fromDescriptor(descriptorBytes, options = {}) {
    const { rootPublicKey, schemeId, testing = false } = options;
    const root = fixedBytes(rootPublicKey, 65, "rootPublicKey");
    const pinnedSchemeId = fixedBytes(schemeId, 32, "schemeId");
    if (!isP256PublicKey(root)) fail("InvalidArgument", "rootPublicKey is not a P-256 point");
    const bytes = asBytes(descriptorBytes, "descriptor");
    const descriptor = decodeNoritoFrame(P_DESCRIPTOR.signed, bytes, 64 * 1024);
    if (descriptor.version !== WIRE_VERSION) fail("Unsupported", "descriptor version must be 1");
    if (!verifyP256Digest(root, descriptorDigestOf(descriptor), descriptor.rootSignature)) {
      fail("InvalidSignature", "descriptor root signature is invalid");
    }
    const derived = schemeIdFor({ ...descriptor, rootPublicKey: root });
    if (!equalBytes(derived, descriptor.schemeId)) fail("WrongScheme", "descriptor scheme id does not match its contents");
    if (!equalBytes(derived, pinnedSchemeId)) fail("WrongScheme", "descriptor scheme id does not match the pinned scheme");
    if (descriptor.allowTestDevices && testing !== true) {
      fail("TestDescriptorRefused", "descriptor admits test devices; pass testing: true only in debug builds");
    }
    const indices = new Set();
    for (const key of descriptor.issuerKeys) {
      if (indices.has(key.index)) fail("Malformed", "descriptor repeats an issuer key index");
      indices.add(key.index);
      if (!isP256PublicKey(key.publicKey)) fail("Malformed", "descriptor issuer key is not a P-256 point");
      if (key.notBeforeMs > key.notAfterMs) fail("Malformed", "descriptor issuer key validity is inverted");
    }
    if (descriptor.issuerKeys.length === 0) fail("Malformed", "descriptor has no issuer key");
    const tiers = new Set();
    for (const tier of descriptor.tiers) {
      if (tiers.has(tier.tier)) fail("Malformed", "descriptor repeats a tier");
      tiers.add(tier.tier);
      if (tier.maxPayment > tier.maxBalance || tier.maxBalance > MAXIMUM_AMOUNT || tier.maxUnsyncedOut > MAXIMUM_AMOUNT) {
        fail("Malformed", "descriptor tier limits are inconsistent");
      }
    }
    if (descriptor.headroomQuantum === 0n) fail("Malformed", "descriptor headroom quantum must be positive");
    return new KagemushaAttestedScheme(SCHEME_TOKEN, descriptor, bytes, root);
  }

  get schemeId() { return Uint8Array.from(this.#descriptor.schemeId); }
  get rootPublicKey() { return Uint8Array.from(this.#rootPublicKey); }
  get descriptor() { return structuredClone(this.#descriptor); }
  get descriptorBytes() { return Uint8Array.from(this.#descriptorBytes); }
  get descriptorDigest() { return Uint8Array.from(this.#descriptorDigest); }
  get descriptorEpoch() { return this.#descriptor.descriptorEpoch; }

  #issuerKeyFor(index, atMs) {
    const key = this.#issuerKeys.get(index);
    if (key === undefined) return null;
    if (atMs !== undefined && (atMs < key.notBeforeMs || atMs > key.notAfterMs)) return null;
    return key;
  }

  /** Returns null when the certificate is authentic under this scheme, else a reason string. */
  #certificateProblem(cert) {
    if (cert.version !== WIRE_VERSION) return "version";
    if (!equalBytes(cert.schemeId, this.#descriptor.schemeId)) return "scheme";
    if (!Object.values(PLATFORMS).includes(cert.platform)) return "platform";
    if (cert.notBeforeMs > cert.notAfterMs) return "validity";
    if (!isP256PublicKey(cert.devicePublicKey)) return "device key";
    if (!equalBytes(deviceIdFor(cert.schemeId, cert.devicePublicKey), cert.deviceId)) return "device id";
    const tier = this.#tiers.get(cert.tier);
    if (tier === undefined) return "tier";
    if (cert.maxBalance > tier.maxBalance || cert.maxPayment > tier.maxPayment || cert.maxUnsyncedOut > tier.maxUnsyncedOut) {
      return "limits";
    }
    const key = this.#issuerKeyFor(cert.issuerKeyIndex, cert.notBeforeMs);
    if (key === null) return "issuer key";
    if (!verifyP256Digest(key.publicKey, certificateDigestOf(cert), cert.issuerSignature)) return "signature";
    return null;
  }

  /** Verify an issuer-certified device certificate (bytes or decoded object). */
  verifyCertificate(input) {
    let cert;
    try {
      cert = decodeSigned(P_CERT, input, 4096);
    } catch {
      return refusal("Malformed");
    }
    const problem = this.#certificateProblem(cert);
    if (problem !== null) return refusal("InvalidCert", { detail: problem });
    return Object.freeze({
      ok: true,
      deviceId: Uint8Array.from(cert.deviceId),
      certificateDigest: certificateDigestOf(cert),
      certificate: cert,
    });
  }

  /** Verify a receiver's signed payment request. */
  verifyRequest(input, options = {}) {
    let request;
    try {
      const message = decodeKagemushaAttestedMessage(input);
      if (message.kind !== "request") return refusal("Malformed");
      request = message.value;
    } catch {
      return refusal("Malformed");
    }
    if (request.version !== WIRE_VERSION || request.amount > MAXIMUM_AMOUNT) return refusal("Malformed");
    if (request.maxUses === 0 || (!request.reusable && request.maxUses !== 1)) return refusal("Malformed");
    if (request.headroom % this.#descriptor.headroomQuantum !== 0n) return refusal("Malformed");
    if (this.#certificateProblem(request.receiverCert) !== null) return refusal("InvalidCert");
    const digest = requestDigestOf(request);
    if (!verifyP256Digest(request.receiverCert.devicePublicKey, digest, request.signature)) return refusal("InvalidSignature");
    const revoked = revokedSet(options.crl);
    if (revoked.has(deviceKey(request.receiverCert.deviceId))) return refusal("Revoked");
    return Object.freeze({
      ok: true,
      requestDigest: digest,
      receiverDeviceId: Uint8Array.from(request.receiverCert.deviceId),
      amount: request.amount,
      headroom: request.headroom,
      reusable: request.reusable,
      request,
    });
  }

  /**
   * Run the receiver checks (§1.6) on a Payment. Returns
   * `{ok: true, paymentId, amount, payerDeviceId, ...}` or `{ok: false, reason}`.
   * Receiver caps, request state and request age are never refusal reasons.
   */
  verifyPayment(input, options = {}) {
    const receiverDeviceId = fixedBytes(options.receiverDeviceId, 32, "receiverDeviceId");
    const nowMs = unsignedValue(options.nowMs ?? Date.now(), MAX_U64, "nowMs");
    const effectiveNowMs = options.highWaterMs !== undefined && BigInt(options.highWaterMs) > nowMs
      ? BigInt(options.highWaterMs)
      : nowMs;
    let payment;
    try {
      const message = decodeKagemushaAttestedMessage(input);
      if (message.kind !== "payment") return refusal("Malformed");
      payment = message.value;
    } catch {
      return refusal("Malformed");
    }
    const transition = payment.transition;
    const cert = payment.payerCert;
    if (payment.version !== WIRE_VERSION || transition.version !== WIRE_VERSION) return refusal("Malformed");
    if (!equalBytes(cert.schemeId, this.#descriptor.schemeId)) return refusal("Malformed");
    if (transition.amount === 0n || transition.amount > MAXIMUM_AMOUNT) return refusal("Malformed");
    if (this.#certificateProblem(cert) !== null) return refusal("InvalidCert");
    if (transition.kind !== TRANSITION_KINDS.sendSplit || !equalBytes(transition.deviceId, cert.deviceId)) {
      return refusal("InvalidSignature");
    }
    const paymentId = transitionDigestOf(transition);
    if (!verifyP256Digest(cert.devicePublicKey, paymentId, payment.signature)) return refusal("InvalidSignature");
    if (!equalBytes(transition.counterparty, receiverDeviceId)) return refusal("WrongReceiver", { paymentId });
    if (options.credited?.has?.(toHex(paymentId))) return refusal("Duplicate", { paymentId });
    const revoked = revokedSet(options.crl);
    if (revoked.has(deviceKey(cert.deviceId))) return refusal("Revoked", { paymentId });
    const seen = options.lastSeen?.get?.(deviceKey(cert.deviceId));
    if (seen !== undefined && seen !== null && forkAgainstLastSeen(seen, transition, paymentId)) {
      return refusal("Fork", { paymentId });
    }
    const graceMs = this.#descriptor.receiverGraceMs;
    if (cert.notAfterMs + graceMs < effectiveNowMs) return refusal("Expired", { paymentId });
    if (transition.amount > cert.maxPayment || transition.unsyncedOutAfter > cert.maxUnsyncedOut) {
      return refusal("PayerLimit", { paymentId });
    }
    const alreadyCredited = BigInt(options.creditedFromPayerAtAckedSeq ?? 0n);
    if (alreadyCredited + transition.amount > cert.maxUnsyncedOut) return refusal("PayerLimit", { paymentId });
    return Object.freeze({
      ok: true,
      paymentId,
      amount: transition.amount,
      payerDeviceId: Uint8Array.from(cert.deviceId),
      payerSeq: transition.seq,
      payerAckedSeq: transition.ackedSeq,
      requestDigest: Uint8Array.from(transition.subject),
      payment,
    });
  }

  /** Verify a receiver acknowledgement against the receiver certificate from the original request. */
  verifyAcknowledgement(input, options = {}) {
    let ack;
    try {
      const message = decodeKagemushaAttestedMessage(input);
      if (message.kind !== "acknowledgement") return refusal("Malformed");
      ack = message.value;
    } catch {
      return refusal("Malformed");
    }
    if (ack.version !== WIRE_VERSION) return refusal("Malformed");
    const certResult = this.verifyCertificate(options.receiverCert);
    if (!certResult.ok) return refusal("InvalidCert");
    if (!equalBytes(ack.receiverDeviceId, certResult.deviceId)) return refusal("WrongReceiver");
    if (options.paymentId !== undefined && !equalBytes(ack.paymentId, fixedBytes(options.paymentId, 32, "paymentId"))) {
      return refusal("Malformed");
    }
    if (!verifyP256Digest(certResult.certificate.devicePublicKey, acknowledgementDigestOf(ack), ack.signature)) {
      return refusal("InvalidSignature");
    }
    return Object.freeze({ ok: true, paymentId: Uint8Array.from(ack.paymentId), acknowledgement: ack });
  }

  /** Verify an issuer mint voucher, optionally bound to the expected device. */
  verifyVoucher(input, options = {}) {
    let voucher;
    try {
      voucher = decodeSigned(P_VOUCHER, input, 4096);
    } catch {
      return refusal("Malformed");
    }
    if (!equalBytes(voucher.schemeId, this.#descriptor.schemeId)) return refusal("Malformed");
    if (voucher.amount === 0n || voucher.amount > MAXIMUM_AMOUNT) return refusal("Malformed");
    if (!equalBytes(voucherIdFor(voucher.txHash, voucher.loadId), voucher.voucherId)) return refusal("Malformed");
    if (options.deviceId !== undefined && !equalBytes(voucher.deviceId, fixedBytes(options.deviceId, 32, "deviceId"))) {
      return refusal("WrongReceiver");
    }
    const key = this.#issuerKeyFor(voucher.issuerKeyIndex, voucher.issuedAtMs);
    if (key === null || !verifyP256Digest(key.publicKey, voucherDigestOf(voucher), voucher.signature)) {
      return refusal("InvalidSignature");
    }
    return Object.freeze({ ok: true, voucherId: Uint8Array.from(voucher.voucherId), amount: voucher.amount, voucher });
  }

  /** Verify an issuer-signed revocation list. Throws on any failure. */
  verifyRevocationList(input) {
    const crl = decodeSigned(P_CRL, input, 64 * 1024 * 1024);
    if (!equalBytes(crl.schemeId, this.#descriptor.schemeId)) fail("WrongScheme", "CRL belongs to another scheme");
    const key = this.#issuerKeyFor(crl.keyIndex, crl.issuedAtMs);
    if (key === null || !verifyP256Digest(key.publicKey, revocationListDigestOf(crl), crl.signature)) {
      fail("InvalidSignature", "CRL signature is invalid");
    }
    const seen = new Set();
    for (const entry of crl.entries) {
      const key2 = deviceKey(entry.deviceId);
      if (seen.has(key2)) fail("Malformed", "CRL repeats a device");
      seen.add(key2);
      if (entry.epoch > crl.epoch || !Object.values(REVOCATION_REASONS).includes(entry.reason)) {
        fail("Malformed", "CRL entry is invalid");
      }
    }
    return crl;
  }

  /** Verify a gossiped CRL delta and optionally apply it to a verified CRL. Throws on failure. */
  verifyRevocationDelta(input, options = {}) {
    const delta = decodeSigned(P_DELTA, input, 4096);
    if (!equalBytes(delta.schemeId, this.#descriptor.schemeId)) fail("WrongScheme", "CRL delta belongs to another scheme");
    if (delta.fromEpoch >= delta.toEpoch) fail("Malformed", "CRL delta epochs are not increasing");
    const key = this.#issuerKeyFor(delta.keyIndex);
    if (key === null || !verifyP256Digest(key.publicKey, revocationDeltaDigestOf(delta), delta.signature)) {
      fail("InvalidSignature", "CRL delta signature is invalid");
    }
    for (const entry of delta.entries) {
      if (entry.epoch <= delta.fromEpoch || entry.epoch > delta.toEpoch
        || !Object.values(REVOCATION_REASONS).includes(entry.reason)) {
        fail("Malformed", "CRL delta entry is outside its epoch range");
      }
    }
    if (options.heldEpoch !== undefined && BigInt(options.heldEpoch) !== delta.fromEpoch) {
      fail("Stale", "CRL delta does not continue the held epoch");
    }
    return delta;
  }

  /** Verify an issuer delivery authorization for a payment the receiver never scanned. */
  verifyDelivery(input, options = {}) {
    let delivery;
    try {
      delivery = decodeSigned(P_DELIVERY, input, 4096);
    } catch {
      return refusal("Malformed");
    }
    if (!equalBytes(delivery.schemeId, this.#descriptor.schemeId)) return refusal("Malformed");
    if (options.receiverDeviceId !== undefined
      && !equalBytes(delivery.receiverDeviceId, fixedBytes(options.receiverDeviceId, 32, "receiverDeviceId"))) {
      return refusal("WrongReceiver");
    }
    const key = this.#issuerKeyFor(delivery.keyIndex);
    if (key === null || !verifyP256Digest(key.publicKey, deliveryDigestOf(delivery), delivery.signature)) {
      return refusal("InvalidSignature");
    }
    return Object.freeze({ ok: true, paymentId: Uint8Array.from(delivery.paymentId), delivery });
  }

  /**
   * Verify self-contained fork evidence: two distinct signed transitions from
   * one issuer-certified device that a genuine app could not have produced.
   */
  verifyForkEvidence(input) {
    let evidence;
    try {
      evidence = decodeSigned(T_FORK_EVIDENCE, input, 8192);
    } catch {
      return false;
    }
    if (this.#certificateProblem(evidence.cert) !== null) return false;
    const { cert, a, b } = evidence;
    if (a.version !== WIRE_VERSION || b.version !== WIRE_VERSION) return false;
    if (!equalBytes(a.deviceId, cert.deviceId) || !equalBytes(b.deviceId, cert.deviceId)) return false;
    const digestA = transitionDigestOf(a);
    const digestB = transitionDigestOf(b);
    if (equalBytes(digestA, digestB)) return false;
    if (!verifyP256Digest(cert.devicePublicKey, digestA, evidence.signatureA)) return false;
    if (!verifyP256Digest(cert.devicePublicKey, digestB, evidence.signatureB)) return false;
    if (evidence.type === "SameSeq") return a.seq === b.seq;
    return inconsistentPair(a, b);
  }
}

const SCHEME_TOKEN = Symbol("KagemushaAttestedScheme");

function isDebit(kind) {
  return kind === TRANSITION_KINDS.sendSplit || kind === TRANSITION_KINDS.redeemSplit;
}

/** `a` precedes `b` at the same acknowledged head, yet `b` undercounts its unsynced outflow. */
function inconsistentPair(a, b) {
  return a.ackedSeq === b.ackedSeq && a.seq < b.seq && isDebit(b.kind)
    && b.unsyncedOutAfter < a.unsyncedOutAfter + b.amount;
}

function forkAgainstLastSeen(seen, transition, digest) {
  const seenSeq = BigInt(seen.seq);
  if (seenSeq === transition.seq) return !equalBytes(fixedBytes(seen.digest, 32, "lastSeen.digest"), digest);
  if (seen.ackedSeq === undefined || BigInt(seen.ackedSeq) !== transition.ackedSeq) return false;
  const earlier = { ackedSeq: BigInt(seen.ackedSeq), seq: seenSeq, unsyncedOutAfter: BigInt(seen.unsyncedOutAfter ?? 0n), kind: seen.kind, amount: BigInt(seen.amount ?? 0n) };
  return seenSeq < transition.seq ? inconsistentPair(earlier, transition) : inconsistentPair(transition, earlier);
}

// ---------------------------------------------------------------------------
// Issuer HTTP client (`{issuerUrl}/v1/kagemusha/attested/...`)
// ---------------------------------------------------------------------------

const ISSUER_ROUTE_PREFIX = "v1/kagemusha/attested/";
const DEFAULT_TIMEOUT_MS = 15_000;
const DEFAULT_MAXIMUM_RESPONSE_BYTES = 8 * 1024 * 1024;

function normalizeBaseUrl(baseUrl) {
  if (typeof baseUrl !== "string" || baseUrl.length === 0) fail("InvalidArgument", "baseUrl must be a non-empty string");
  let url;
  try {
    url = new URL(baseUrl);
  } catch {
    return fail("InvalidArgument", "baseUrl must be an absolute URL");
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") fail("InvalidArgument", "baseUrl must use http or https");
  if (url.username !== "" || url.password !== "" || url.search !== "" || url.hash !== "") {
    fail("InvalidArgument", "baseUrl must not carry credentials, a query or a fragment");
  }
  if (!url.pathname.endsWith("/")) url.pathname = `${url.pathname}/`;
  return url;
}

async function readBoundedBody(response, maximumBytes) {
  const declared = response.headers?.get?.("content-length");
  if (declared !== null && declared !== undefined && Number(declared) > maximumBytes) {
    fail("Oversized", `issuer response exceeds ${maximumBytes} bytes`);
  }
  if (response.body === null || response.body === undefined || typeof response.body.getReader !== "function") {
    const buffer = new Uint8Array(await response.arrayBuffer());
    if (buffer.length > maximumBytes) fail("Oversized", `issuer response exceeds ${maximumBytes} bytes`);
    return buffer;
  }
  const reader = response.body.getReader();
  const chunks = [];
  let total = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    total += value.length;
    if (total > maximumBytes) {
      await reader.cancel().catch(() => {});
      fail("Oversized", `issuer response exceeds ${maximumBytes} bytes`);
    }
    chunks.push(value);
  }
  return concatBytes(...chunks);
}

/** Typed failure for a non-success issuer response. */
export class KagemushaAttestedIssuerError extends KagemushaAttestedError {
  constructor(status, issuerCode, message, retryable) {
    super(retryable ? "IssuerUnavailable" : "IssuerRejected", message);
    this.name = "KagemushaAttestedIssuerError";
    this.status = status;
    this.issuerCode = issuerCode;
    this.retryable = retryable;
  }
}

/**
 * Client for the attested-suite issuer routes. Signed objects returned by the
 * issuer (descriptor, CRL, deltas) are verified against the pinned scheme when
 * one is configured; requests that need device keys are forwarded as opaque,
 * already-signed bodies.
 */
export class KagemushaAttestedIssuerClient {
  #baseUrl;
  #scheme;
  #fetch;
  #timeoutMs;
  #maximumResponseBytes;

  constructor(options = {}) {
    this.#baseUrl = normalizeBaseUrl(options.baseUrl);
    if (options.scheme !== undefined && options.scheme !== null && !(options.scheme instanceof KagemushaAttestedScheme)) {
      fail("InvalidArgument", "scheme must be a KagemushaAttestedScheme");
    }
    this.#scheme = options.scheme ?? null;
    const fetchImpl = options.fetch ?? globalThis.fetch;
    if (typeof fetchImpl !== "function") fail("InvalidArgument", "a fetch implementation is required");
    this.#fetch = fetchImpl;
    this.#timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
    this.#maximumResponseBytes = options.maximumResponseBytes ?? DEFAULT_MAXIMUM_RESPONSE_BYTES;
    if (!Number.isSafeInteger(this.#timeoutMs) || this.#timeoutMs <= 0) fail("InvalidArgument", "timeoutMs must be a positive integer");
    if (!Number.isSafeInteger(this.#maximumResponseBytes) || this.#maximumResponseBytes <= 0) {
      fail("InvalidArgument", "maximumResponseBytes must be a positive integer");
    }
  }

  get scheme() { return this.#scheme; }

  #url(route, query = undefined) {
    const url = new URL(`${ISSUER_ROUTE_PREFIX}${route}`, this.#baseUrl);
    if (query !== undefined) {
      for (const [key, value] of Object.entries(query)) {
        if (value !== undefined && value !== null) url.searchParams.set(key, String(value));
      }
    }
    return url.toString();
  }

  async #request(method, route, { query, body, allowStatuses = [] } = {}) {
    const controller = typeof AbortController === "function" ? new AbortController() : null;
    const timer = controller === null ? null : setTimeout(() => controller.abort(), this.#timeoutMs);
    let response;
    try {
      response = await this.#fetch(this.#url(route, query), {
        method,
        headers: body === undefined
          ? { accept: "application/json" }
          : { accept: "application/json", "content-type": "application/json" },
        body: body === undefined ? undefined : stringifyStrictLosslessIntegerJson(body, `issuer ${route} request`),
        signal: controller?.signal,
        redirect: "error",
      });
    } catch (error) {
      throw new KagemushaAttestedIssuerError(0, "network", `issuer ${route} request failed: ${error?.message ?? error}`, true);
    } finally {
      if (timer !== null) clearTimeout(timer);
    }
    const raw = await readBoundedBody(response, this.#maximumResponseBytes);
    let json = null;
    if (raw.length > 0) {
      try {
        json = parseStrictLosslessIntegerJson(UTF8_DECODER.decode(raw), `issuer ${route} response`);
      } catch (error) {
        if (response.ok) fail("Malformed", `issuer ${route} response is not strict JSON: ${error.message}`);
      }
    }
    if (!response.ok && !allowStatuses.includes(response.status)) {
      const retryable = response.status === 429 || response.status >= 500;
      const issuerCode = typeof json?.code === "string" ? json.code : `http_${response.status}`;
      const message = typeof json?.message === "string" ? json.message : `issuer ${route} returned HTTP ${response.status}`;
      throw new KagemushaAttestedIssuerError(response.status, issuerCode, message, retryable);
    }
    return { status: response.status, json };
  }

  /** Liveness only (`GET healthz`). */
  async healthz() {
    const { status } = await this.#request("GET", "healthz");
    return status >= 200 && status < 300;
  }

  /**
   * Truthful readiness (`GET readiness`). A 503 is reported as `{ready: false}`;
   * offline payments are unaffected by issuer readiness.
   */
  async readiness() {
    const { status, json } = await this.#request("GET", "readiness", { allowStatuses: [503] });
    const ready = status === 200 && json?.ready === true;
    return Object.freeze({ ...(json ?? {}), ready });
  }

  /**
   * Fetch the descriptor (`GET descriptor`). With a pinned scheme the bytes
   * must verify under the same root key and scheme id.
   */
  async descriptor(options = {}) {
    const { json } = await this.#request("GET", "descriptor");
    const bytes = base64UrlDecode(requiredString(json, "descriptor"), "descriptor");
    const rootPublicKey = options.rootPublicKey ?? this.#scheme?.rootPublicKey;
    const schemeId = options.schemeId ?? this.#scheme?.schemeId;
    if (rootPublicKey === undefined || schemeId === undefined) {
      fail("InvalidArgument", "descriptor() needs a pinned scheme or rootPublicKey and schemeId");
    }
    return KagemushaAttestedScheme.fromDescriptor(bytes, { rootPublicKey, schemeId, testing: options.testing === true });
  }

  /** Fetch the full CRL, or a delta from `since` (`GET crl?since=`), verified against the scheme. */
  async revocations(options = {}) {
    const scheme = this.#requireScheme("revocations()");
    const since = options.since === undefined ? undefined : unsignedValue(options.since, MAX_U64, "since");
    const { json } = await this.#request("GET", "crl", { query: { since } });
    if (typeof json?.delta === "string") {
      return Object.freeze({ delta: scheme.verifyRevocationDelta(base64UrlDecode(json.delta, "crl delta"), { heldEpoch: since }) });
    }
    return Object.freeze({ crl: scheme.verifyRevocationList(base64UrlDecode(requiredString(json, "crl"), "crl")) });
  }

  /** Read an operation (load claim, redemption) status (`GET operations/{id}`). */
  async operation(operationId) {
    if (typeof operationId !== "string" || !/^[A-Za-z0-9_-]{1,128}$/u.test(operationId)) {
      fail("InvalidArgument", "operationId must be 1-128 URL-safe characters");
    }
    const { json } = await this.#request("GET", `operations/${operationId}`);
    return json;
  }

  /** `POST enroll/challenge {account_id, platform}` → `{server_nonce, expires_at_ms}`. */
  async enrollChallenge({ accountId, platform }) {
    if (typeof accountId !== "string" || accountId.length === 0) fail("InvalidArgument", "accountId is required");
    if (platform !== "android" && platform !== "apple") fail("InvalidArgument", "platform must be android or apple");
    const { json } = await this.#request("POST", "enroll/challenge", { body: { account_id: accountId, platform } });
    return json;
  }

  /** Forward an already-signed enrollment body (`POST enroll/android|apple`). */
  async enroll(platform, body) {
    if (platform !== "android" && platform !== "apple") fail("InvalidArgument", "platform must be android or apple");
    const { json } = await this.#request("POST", `enroll/${platform}`, { body: requireObject(body, "enroll body") });
    return json;
  }

  /** Forward a device-signed `POST load/prepare` body. */
  async prepareLoad(body) {
    return (await this.#request("POST", "load/prepare", { body: requireObject(body, "load/prepare body") })).json;
  }

  /** Forward `POST load/claim {load_id, tx_hash}`; idempotent per transaction hash. */
  async claimLoad(body) {
    return (await this.#request("POST", "load/claim", { body: requireObject(body, "load/claim body") })).json;
  }

  /** Forward a device sync upload (`POST sync`). Sync never confirms, holds or reverses a payment. */
  async sync(body) {
    return (await this.#request("POST", "sync", { body: requireObject(body, "sync body") })).json;
  }

  /** Forward an account-signed `POST report-lost` body. */
  async reportLost(body) {
    return (await this.#request("POST", "report-lost", { body: requireObject(body, "report-lost body") })).json;
  }

  #requireScheme(context) {
    if (this.#scheme === null) fail("InvalidArgument", `${context} needs a pinned scheme`);
    return this.#scheme;
  }
}

function requiredString(json, key) {
  if (json === null || typeof json !== "object" || typeof json[key] !== "string") {
    fail("Malformed", `issuer response is missing ${key}`);
  }
  return json[key];
}

function requireObject(value, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) fail("InvalidArgument", `${context} must be an object`);
  return value;
}

// ---------------------------------------------------------------------------
// Public codec surface
// ---------------------------------------------------------------------------

const CODECS = Object.freeze({
  descriptor: Object.freeze({ type: P_DESCRIPTOR.signed, maximumBytes: 64 * 1024 }),
  descriptorBody: Object.freeze({ type: P_DESCRIPTOR.body, maximumBytes: 64 * 1024 }),
  certificate: Object.freeze({ type: P_CERT.signed, maximumBytes: 4096 }),
  certificateBody: Object.freeze({ type: P_CERT.body, maximumBytes: 4096 }),
  transition: Object.freeze({ type: T_TRANSITION, maximumBytes: 4096 }),
  signedTransition: Object.freeze({ type: T_SIGNED_TRANSITION, maximumBytes: 4096 }),
  request: Object.freeze({ type: P_REQUEST.signed, maximumBytes: MAXIMUM_REQUEST_BYTES }),
  requestBody: Object.freeze({ type: P_REQUEST.body, maximumBytes: MAXIMUM_REQUEST_BYTES }),
  payment: Object.freeze({ type: T_PAYMENT, maximumBytes: MAXIMUM_PAYMENT_BYTES }),
  acknowledgement: Object.freeze({ type: P_ACK.signed, maximumBytes: MAXIMUM_ACKNOWLEDGEMENT_BYTES }),
  acknowledgementBody: Object.freeze({ type: P_ACK.body, maximumBytes: MAXIMUM_ACKNOWLEDGEMENT_BYTES }),
  voucher: Object.freeze({ type: P_VOUCHER.signed, maximumBytes: 4096 }),
  voucherBody: Object.freeze({ type: P_VOUCHER.body, maximumBytes: 4096 }),
  revocationList: Object.freeze({ type: P_CRL.signed, maximumBytes: 64 * 1024 * 1024 }),
  revocationListBody: Object.freeze({ type: P_CRL.body, maximumBytes: 64 * 1024 * 1024 }),
  revocationDelta: Object.freeze({ type: P_DELTA.signed, maximumBytes: 4096 }),
  revocationDeltaBody: Object.freeze({ type: P_DELTA.body, maximumBytes: 4096 }),
  delivery: Object.freeze({ type: P_DELIVERY.signed, maximumBytes: 4096 }),
  deliveryBody: Object.freeze({ type: P_DELIVERY.body, maximumBytes: 4096 }),
  forkEvidence: Object.freeze({ type: T_FORK_EVIDENCE, maximumBytes: 8192 }),
});

function codecFor(name) {
  if (typeof name !== "string" || !Object.hasOwn(CODECS, name)) fail("InvalidArgument", `unknown KAGEMUSHA attested type ${String(name)}`);
  return CODECS[name];
}

/** Encode a value as canonical Norito bytes of the named suite type. */
export function encodeKagemushaAttested(typeName, value) {
  return encodeNoritoFrame(codecFor(typeName).type, value);
}

/** Strictly decode canonical Norito bytes of the named suite type. */
export function decodeKagemushaAttested(typeName, bytes) {
  const codec = codecFor(typeName);
  return decodeNoritoFrame(codec.type, bytes, codec.maximumBytes);
}

/** Constants, identifiers and digests of the attested-app suite. */
export const KagemushaAttestedV1 = /* @__PURE__ */ Object.freeze({
  suite: SUITE_ID,
  wireVersion: WIRE_VERSION,
  textPrefix: TEXT_PREFIX,
  ipm1Profile: IPM1_PROFILE_ATTESTED,
  ipm1SchemaVersion: IPM1_SCHEMA_VERSION_ATTESTED,
  maximumAmount: MAXIMUM_AMOUNT,
  maximumRequestBytes: MAXIMUM_REQUEST_BYTES,
  maximumPaymentBytes: MAXIMUM_PAYMENT_BYTES,
  maximumAcknowledgementBytes: MAXIMUM_ACKNOWLEDGEMENT_BYTES,
  maximumQrFrameTextBytes: QR_MAXIMUM_TEXT_BYTES,
  transitionKinds: TRANSITION_KINDS,
  transitionKindNames: TRANSITION_KIND_NAMES,
  platforms: PLATFORMS,
  revocationReasons: REVOCATION_REASONS,
  refusalReasons: REFUSAL_REASONS,
  typeNames: Object.freeze(Object.keys(CODECS)),
  schemaName: (typeName) => codecFor(typeName).type.schema,
  domain: (purpose) => Uint8Array.from(domain(purpose)),
  schemeId: schemeIdFor,
  deviceId: deviceIdFor,
  descriptorDigest: (descriptor) => descriptorDigestOf(descriptor),
  certificateDigest: (cert) => certificateDigestOf(cert),
  transitionDigest: (transition) => transitionDigestOf(transition),
  paymentId: (transition) => transitionDigestOf(transition),
  requestDigest: (request) => requestDigestOf(request),
  acknowledgementDigest: (ack) => acknowledgementDigestOf(ack),
  voucherDigest: (voucher) => voucherDigestOf(voucher),
  revocationListDigest: (crl) => revocationListDigestOf(crl),
  revocationDeltaDigest: (delta) => revocationDeltaDigestOf(delta),
  deliveryDigest: (delivery) => deliveryDigestOf(delivery),
  voucherId: voucherIdFor,
  redemptionId: redemptionIdFor,
  loadBinding: loadBindingFor,
  verifyP256: (publicKey, digest, signature) => verifyP256Digest(
    asBytes(publicKey, "publicKey"), asBytes(digest, "digest"), asBytes(signature, "signature")),
  toHex,
  fromHex: (text) => fromHex(text, "hex"),
  base64UrlEncode: (bytes) => base64UrlEncode(asBytes(bytes, "bytes")),
  base64UrlDecode: (text) => base64UrlDecode(text, "base64url"),
});
