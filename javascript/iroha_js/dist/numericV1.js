import { blake2b256 } from "./blake2b.js";

const MAX_MANTISSA_BYTES = 64;
const MAX_INT_TEXT_BYTES = 155;
const MAX_SIGNIFICANT_DIGITS = 154;
const FRAME_HEADER_BYTES = 40;
const ENVELOPE_HEADER_BYTES = 7;
const HASH_BYTES = 32;
const U64_MASK = (1n << 64n) - 1n;
const CRC64_POLY = 0xC96C5795D7870F42n;
const INT_MIN = -(1n << 511n);
const INT_MAX = (1n << 511n) - 1n;

const SCHEMAS = Object.freeze({
  int: Object.freeze({
    name: "iroha.numeric.IntValueV1",
    hash: "07c039457363b9e1d36bbd31d93dec4a",
    pointerType: 0x0011,
    scaled: false,
  }),
  decimal: Object.freeze({
    name: "iroha.numeric.DecimalValueV1",
    hash: "ba2ffed52e4d8ee16f17efefe1828524",
    pointerType: 0x0012,
    scaled: true,
  }),
  quantity: Object.freeze({
    name: "iroha.numeric.QuantityValueV1",
    hash: "e4769984c81ce0e8b678f2eb06274ee3",
    pointerType: 0x0013,
    scaled: true,
  }),
});

const CRC64_TABLE = Object.freeze(Array.from({ length: 256 }, (_, index) => {
  let crc = BigInt(index);
  for (let bit = 0; bit < 8; bit += 1) {
    crc = (crc & 1n) === 0n ? crc >> 1n : (crc >> 1n) ^ CRC64_POLY;
  }
  return crc;
}));

/** Stable validation failure raised by the Kotodama V1 numeric codec. */
export class NumericV1Error extends Error {
  constructor(code, message) {
    super(message);
    this.name = "NumericV1Error";
    this.code = code;
  }
}

function fail(code, message) {
  throw new NumericV1Error(code, message);
}

function asBytes(value, context) {
  if (value instanceof Uint8Array) return Uint8Array.from(value);
  if (ArrayBuffer.isView(value)) {
    return Uint8Array.from(new Uint8Array(value.buffer, value.byteOffset, value.byteLength));
  }
  if (value instanceof ArrayBuffer) return Uint8Array.from(new Uint8Array(value));
  throw new TypeError(`${context} must be an ArrayBuffer or Uint8Array`);
}

function concatBytes(...parts) {
  const length = parts.reduce((sum, part) => sum + part.length, 0);
  const result = new Uint8Array(length);
  let offset = 0;
  for (const part of parts) {
    result.set(part, offset);
    offset += part.length;
  }
  return result;
}

function hexToBytes(hex) {
  const result = new Uint8Array(hex.length / 2);
  for (let index = 0; index < result.length; index += 1) {
    result[index] = Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16);
  }
  return result;
}

function equalBytes(left, right) {
  if (left.length !== right.length) return false;
  let difference = 0;
  for (let index = 0; index < left.length; index += 1) {
    difference |= left[index] ^ right[index];
  }
  return difference === 0;
}

function payloadHash(frame) {
  const digest = blake2b256(frame);
  digest[digest.length - 1] |= 1;
  return digest;
}

function writeU32Le(value) {
  const out = new Uint8Array(4);
  new DataView(out.buffer).setUint32(0, value, true);
  return out;
}

function writeU32Be(value) {
  const out = new Uint8Array(4);
  new DataView(out.buffer).setUint32(0, value, false);
  return out;
}

function writeU64Le(value) {
  const out = new Uint8Array(8);
  let remaining = BigInt.asUintN(64, value);
  for (let index = 0; index < 8; index += 1) {
    out[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  return out;
}

function readU32Le(bytes, offset) {
  return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(offset, true);
}

function readU32Be(bytes, offset) {
  return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(offset, false);
}

function readU64Le(bytes, offset) {
  let value = 0n;
  for (let index = 7; index >= 0; index -= 1) {
    value = (value << 8n) | BigInt(bytes[offset + index]);
  }
  return value;
}

function crc64Xz(bytes) {
  let crc = U64_MASK;
  for (const byte of bytes) {
    const index = Number((crc ^ BigInt(byte)) & 0xffn);
    crc = CRC64_TABLE[index] ^ (crc >> 8n);
  }
  return BigInt.asUintN(64, crc ^ U64_MASK);
}

function checkedBigInt(value, context) {
  if (typeof value === "bigint") return value;
  if (typeof value === "string") {
    if (!/^-?(?:0|[1-9][0-9]*)$/u.test(value) || value === "-0") {
      fail("invalid_text", `${context} must use canonical base-10 syntax`);
    }
    if (value.length > MAX_INT_TEXT_BYTES) {
      fail("mantissa_overflow", "integer text exceeds the signed 512-bit input bound");
    }
    return BigInt(value);
  }
  throw new TypeError(`${context} must be a bigint or canonical integer string`);
}

function checkIntRange(value) {
  if (value < INT_MIN || value > INT_MAX) {
    fail("mantissa_overflow", "numeric mantissa is outside the signed 512-bit domain");
  }
  return value;
}

function encodeTwosComplement(value) {
  checkIntRange(value);
  if (value === 0n) return new Uint8Array();
  if (value > 0n) {
    const bytes = [];
    let remaining = value;
    while (remaining !== 0n) {
      bytes.push(Number(remaining & 0xffn));
      remaining >>= 8n;
    }
    if ((bytes[bytes.length - 1] & 0x80) !== 0) bytes.push(0);
    if (bytes.length > MAX_MANTISSA_BYTES) fail("mantissa_overflow", "mantissa is too wide");
    return Uint8Array.from(bytes);
  }
  let width = 1;
  while (value < -(1n << BigInt(width * 8 - 1))) width += 1;
  if (width > MAX_MANTISSA_BYTES) fail("mantissa_overflow", "mantissa is too wide");
  let encoded = (1n << BigInt(width * 8)) + value;
  const bytes = new Uint8Array(width);
  for (let index = 0; index < width; index += 1) {
    bytes[index] = Number(encoded & 0xffn);
    encoded >>= 8n;
  }
  return bytes;
}

function decodeTwosComplement(input) {
  const bytes = asBytes(input, "mantissa");
  if (bytes.length > MAX_MANTISSA_BYTES) fail("mantissa_overflow", "mantissa is too wide");
  if (bytes.length === 0) return 0n;
  const last = bytes[bytes.length - 1];
  if (bytes.length === 1 && last === 0) {
    fail("noncanonical_mantissa", "zero must use an empty mantissa");
  }
  if (bytes.length > 1) {
    const previous = bytes[bytes.length - 2];
    if ((last === 0 && (previous & 0x80) === 0)
      || (last === 0xff && (previous & 0x80) !== 0)) {
      fail("noncanonical_mantissa", "mantissa has redundant sign extension");
    }
  }
  let unsigned = 0n;
  for (let index = bytes.length - 1; index >= 0; index -= 1) {
    unsigned = (unsigned << 8n) | BigInt(bytes[index]);
  }
  const value = (last & 0x80) === 0
    ? unsigned
    : unsigned - (1n << BigInt(bytes.length * 8));
  return checkIntRange(value);
}

function normalizeScaled(mantissa, scale, quantity) {
  if (!Number.isSafeInteger(scale) || scale < 0) {
    fail("invalid_scale", "numeric scale must be a non-negative safe integer");
  }
  let normalizedScale = scale;
  let normalizedMantissa;
  if (typeof mantissa === "string") {
    if (!/^-?(?:0|[1-9][0-9]*)$/u.test(mantissa) || mantissa === "-0") {
      fail("invalid_text", "mantissa must use canonical base-10 syntax");
    }
    const negative = mantissa.startsWith("-");
    let magnitude = negative ? mantissa.slice(1) : mantissa;
    if (magnitude === "0") {
      normalizedMantissa = 0n;
      normalizedScale = 0;
    } else {
      while (normalizedScale > 0 && magnitude.endsWith("0")) {
        magnitude = magnitude.slice(0, -1);
        normalizedScale -= 1;
      }
      const normalizedText = `${negative ? "-" : ""}${magnitude}`;
      if (normalizedText.length > MAX_INT_TEXT_BYTES) {
        fail("mantissa_overflow", "numeric mantissa is outside the signed 512-bit domain");
      }
      normalizedMantissa = BigInt(normalizedText);
    }
  } else {
    normalizedMantissa = checkedBigInt(mantissa, "mantissa");
  }
  if (normalizedMantissa === 0n) {
    normalizedScale = 0;
  } else {
    while (normalizedScale > 0 && normalizedMantissa % 10n === 0n) {
      normalizedMantissa /= 10n;
      normalizedScale -= 1;
    }
  }
  if (normalizedScale > 28) fail("invalid_scale", "canonical numeric scale exceeds 28");
  checkIntRange(normalizedMantissa);
  if (quantity && normalizedMantissa < 0n) {
    fail("negative_quantity", "quantity cannot be negative");
  }
  return [normalizedMantissa, normalizedScale];
}

function parseScaled(value, quantity) {
  if (typeof value !== "string") {
    throw new TypeError("decimal and quantity values must be strings");
  }
  const match = /^(-?)(0|[1-9][0-9]*)(?:\.([0-9]+))?$/u.exec(value);
  if (!match || value === "-0") fail("invalid_text", "numeric text is not canonical decimal syntax");
  const fraction = match[3] ?? "";
  const rawDigits = `${match[2]}${fraction}`;
  let first = 0;
  while (first < rawDigits.length && rawDigits.charCodeAt(first) === 0x30) first += 1;
  if (first === rawDigits.length) return normalizeScaled(0n, 0, quantity);
  let end = rawDigits.length;
  let scale = fraction.length;
  while (scale > 0 && rawDigits.charCodeAt(end - 1) === 0x30) {
    end -= 1;
    scale -= 1;
  }
  if (scale > 28) fail("invalid_scale", "canonical numeric scale exceeds 28");
  if (end - first > MAX_SIGNIFICANT_DIGITS) {
    fail("mantissa_overflow", "decimal mantissa exceeds the signed 512-bit input bound");
  }
  const digits = rawDigits.slice(first, end);
  const mantissa = BigInt(`${match[1]}${digits}`);
  return normalizeScaled(mantissa, scale, quantity);
}

function canonicalScaledText(mantissa, scale) {
  if (scale === 0) return mantissa.toString();
  const negative = mantissa < 0n;
  let digits = (negative ? -mantissa : mantissa).toString();
  if (digits.length <= scale) digits = `${"0".repeat(scale + 1 - digits.length)}${digits}`;
  const split = digits.length - scale;
  return `${negative ? "-" : ""}${digits.slice(0, split)}.${digits.slice(split)}`;
}

/** Lossless Kotodama V1 signed integer. JavaScript Number inputs are rejected. */
export class KotodamaInt {
  constructor(value) {
    this.value = checkIntRange(checkedBigInt(value, "int"));
    Object.freeze(this);
  }

  toString() {
    return this.value.toString();
  }
}

/** Lossless exact Kotodama V1 decimal. */
export class KotodamaDecimal {
  constructor(value, scale) {
    const normalized = scale === undefined
      ? parseScaled(value, false)
      : normalizeScaled(value, scale, false);
    [this.mantissa, this.scale] = normalized;
    Object.freeze(this);
  }

  toString() {
    return canonicalScaledText(this.mantissa, this.scale);
  }
}

/** Lossless nominal non-negative Kotodama V1 asset quantity. */
export class KotodamaQuantity {
  constructor(value, scale) {
    const normalized = scale === undefined
      ? parseScaled(value, true)
      : normalizeScaled(value, scale, true);
    [this.mantissa, this.scale] = normalized;
    Object.freeze(this);
  }

  toString() {
    return canonicalScaledText(this.mantissa, this.scale);
  }
}

function canonicalIntValue(value) {
  return value instanceof KotodamaInt ? new KotodamaInt(value.value) : new KotodamaInt(value);
}

function canonicalDecimalValue(value) {
  return value instanceof KotodamaDecimal
    ? new KotodamaDecimal(value.mantissa, value.scale)
    : new KotodamaDecimal(value);
}

function canonicalQuantityValue(value) {
  return value instanceof KotodamaQuantity
    ? new KotodamaQuantity(value.mantissa, value.scale)
    : new KotodamaQuantity(value);
}

function bodyFor(kind, value) {
  const schema = SCHEMAS[kind];
  const mantissa = kind === "int" ? value.value : value.mantissa;
  const encoded = encodeTwosComplement(mantissa);
  const parts = [writeU32Le(encoded.length), encoded];
  if (schema.scaled) parts.push(Uint8Array.of(value.scale));
  return concatBytes(...parts);
}

function frameFor(kind, value) {
  const schema = SCHEMAS[kind];
  const body = bodyFor(kind, value);
  return concatBytes(
    Uint8Array.from([0x4e, 0x52, 0x54, 0x30, 0, 0]),
    hexToBytes(schema.hash),
    Uint8Array.of(0),
    writeU64Le(BigInt(body.length)),
    writeU64Le(crc64Xz(body)),
    Uint8Array.of(0),
    body,
  );
}

function decodeFrame(kind, input) {
  const schema = SCHEMAS[kind];
  const frame = asBytes(input, "numeric frame");
  const maximum = FRAME_HEADER_BYTES + 4 + MAX_MANTISSA_BYTES + (schema.scaled ? 1 : 0);
  if (frame.length < FRAME_HEADER_BYTES) fail("frame_too_short", "numeric frame is truncated");
  if (frame.length > maximum) fail("frame_too_large", "numeric frame is oversized");
  if (!equalBytes(frame.subarray(0, 6), Uint8Array.from([0x4e, 0x52, 0x54, 0x30, 0, 0]))) {
    fail("invalid_header", "numeric frame has the wrong Norito magic or version");
  }
  if (!equalBytes(frame.subarray(6, 22), hexToBytes(schema.hash))) {
    fail("schema_mismatch", "numeric frame schema does not match its type");
  }
  if (frame[22] !== 0) fail("compression_not_allowed", "numeric frames cannot be compressed");
  if (frame[39] !== 0) fail("layout_flags_not_allowed", "numeric frame flags must be zero");
  const bodyLength = readU64Le(frame, 23);
  if (bodyLength > BigInt(Number.MAX_SAFE_INTEGER)
    || Number(bodyLength) !== frame.length - FRAME_HEADER_BYTES) {
    fail("length_mismatch", "numeric frame length is inconsistent");
  }
  const body = frame.subarray(FRAME_HEADER_BYTES);
  if (readU64Le(frame, 31) !== crc64Xz(body)) fail("checksum_mismatch", "numeric frame checksum failed");
  if (body.length < 4) fail("length_mismatch", "numeric body has no mantissa length");
  const mantissaLength = readU32Le(body, 0);
  const expectedBodyLength = 4 + mantissaLength + (schema.scaled ? 1 : 0);
  if (mantissaLength > MAX_MANTISSA_BYTES || expectedBodyLength !== body.length) {
    fail(mantissaLength > MAX_MANTISSA_BYTES ? "mantissa_overflow" : "length_mismatch",
      "numeric body length is inconsistent");
  }
  const mantissa = decodeTwosComplement(body.subarray(4, 4 + mantissaLength));
  if (!schema.scaled) return new KotodamaInt(mantissa);
  const scale = body[body.length - 1];
  if (scale > 28) fail("invalid_scale", "numeric scale exceeds 28");
  if ((mantissa === 0n && scale !== 0) || (scale > 0 && mantissa % 10n === 0n)) {
    fail("noncanonical_decimal", "numeric value has a noncanonical scale");
  }
  return kind === "quantity"
    ? new KotodamaQuantity(mantissa, scale)
    : new KotodamaDecimal(mantissa, scale);
}

function envelopeFor(kind, value) {
  const schema = SCHEMAS[kind];
  const frame = frameFor(kind, value);
  return concatBytes(
    Uint8Array.of(schema.pointerType >> 8, schema.pointerType & 0xff, 1),
    writeU32Be(frame.length),
    frame,
    payloadHash(frame),
  );
}

function decodeEnvelope(kind, input) {
  const schema = SCHEMAS[kind];
  const envelope = asBytes(input, "numeric pointer envelope");
  if (envelope.length < ENVELOPE_HEADER_BYTES) fail("truncated_envelope", "numeric envelope is truncated");
  const pointerType = (envelope[0] << 8) | envelope[1];
  if (pointerType === 0x0010) {
    fail("type_not_allowed", "retired Amount pointer type is permanently reserved");
  }
  const knownAllowedType = (pointerType >= 0x0001 && pointerType <= 0x000f)
    || (pointerType >= 0x0011 && pointerType <= 0x0013);
  if (!knownAllowedType) {
    fail("unknown_type", "numeric envelope has an unknown pointer type");
  }
  if (pointerType !== schema.pointerType) fail("wrong_type", "numeric envelope type does not match");
  if (envelope[2] !== 1) fail("invalid_envelope_version", "numeric envelope version must be 1");
  const frameLength = readU32Be(envelope, 3);
  const maximum = FRAME_HEADER_BYTES + 4 + MAX_MANTISSA_BYTES + (schema.scaled ? 1 : 0);
  if (frameLength > maximum) fail("oversized_length", "numeric envelope declares an oversized frame");
  if (ENVELOPE_HEADER_BYTES + frameLength + HASH_BYTES !== envelope.length) {
    fail("truncated_envelope", "numeric envelope length is inconsistent");
  }
  const frame = envelope.subarray(ENVELOPE_HEADER_BYTES, ENVELOPE_HEADER_BYTES + frameLength);
  const suppliedHash = envelope.subarray(ENVELOPE_HEADER_BYTES + frameLength);
  if (!equalBytes(payloadHash(frame), suppliedHash)) {
    fail("payload_hash_mismatch", "numeric envelope payload hash failed");
  }
  return decodeFrame(kind, frame);
}

export const NumericV1 = Object.freeze({
  INT_MIN,
  INT_MAX,
  MAX_MANTISSA_BYTES,
  MAX_SCALE: 28,
  schemas: SCHEMAS,
  encodeIntFrame: (value) => frameFor("int", canonicalIntValue(value)),
  encodeDecimalFrame: (value) => frameFor("decimal", canonicalDecimalValue(value)),
  encodeQuantityFrame: (value) => frameFor("quantity", canonicalQuantityValue(value)),
  decodeIntFrame: (bytes) => decodeFrame("int", bytes),
  decodeDecimalFrame: (bytes) => decodeFrame("decimal", bytes),
  decodeQuantityFrame: (bytes) => decodeFrame("quantity", bytes),
  encodeIntEnvelope: (value) => envelopeFor("int", canonicalIntValue(value)),
  encodeDecimalEnvelope: (value) => envelopeFor("decimal", canonicalDecimalValue(value)),
  encodeQuantityEnvelope: (value) => envelopeFor("quantity", canonicalQuantityValue(value)),
  decodeIntEnvelope: (bytes) => decodeEnvelope("int", bytes),
  decodeDecimalEnvelope: (bytes) => decodeEnvelope("decimal", bytes),
  decodeQuantityEnvelope: (bytes) => decodeEnvelope("quantity", bytes),
  encodeIntJson: (value) => canonicalIntValue(value).toString(),
  encodeDecimalJson: (value) => canonicalDecimalValue(value).toString(),
  encodeQuantityJson: (value) => canonicalQuantityValue(value).toString(),
  decodeIntJson: (value) => {
    if (typeof value !== "string") fail("invalid_text", "int JSON must be a string");
    return new KotodamaInt(value);
  },
  decodeDecimalJson: (value) => {
    if (typeof value !== "string") fail("invalid_text", "decimal JSON must be a string");
    const decoded = new KotodamaDecimal(value);
    if (decoded.toString() !== value) fail("invalid_text", "decimal JSON must use canonical spelling");
    return decoded;
  },
  decodeQuantityJson: (value) => {
    if (typeof value !== "string") fail("invalid_text", "quantity JSON must be a string");
    const decoded = new KotodamaQuantity(value);
    if (decoded.toString() !== value) fail("invalid_text", "quantity JSON must use canonical spelling");
    return decoded;
  },
});
