import { blake2b256 } from "./blake2b.js";
import { crc64 } from "./crc64.js";

export const ConnectDirection = Object.freeze({
  APP_TO_WALLET: "app_to_wallet",
  WALLET_TO_APP: "wallet_to_app",
});

const DIRECTION_TAG = Object.freeze({
  [ConnectDirection.APP_TO_WALLET]: 0,
  [ConnectDirection.WALLET_TO_APP]: 1,
});

const NORITO_HEADER_LEN = 40;
const NORITO_MAGIC = new Uint8Array([0x4e, 0x52, 0x54, 0x30]); // "NRT0"
const NORITO_VERSION_MAJOR = 0;
const NORITO_VERSION_MINOR = 0;
const NORITO_COMPRESSION_NONE = 0;
const NORITO_FLAGS_NONE = 0;
const MAX_HEADER_PADDING = 64;
const SCHEMA_NAME = "ConnectJournalRecordV1";
const SCHEMA_HASH = computeSchemaHash(SCHEMA_NAME);
const PAYLOAD_FIXED_LEN = 1 + 8 + 8 + 8 + 4 + 32;
const MAX_UINT64 = (1n << 64n) - 1n;

export class ConnectJournalError extends Error {
  constructor(message, options = {}) {
    super(message);
    this.name = "ConnectJournalError";
    if (options.cause !== undefined) {
      this.cause = options.cause;
    }
  }
}

function computeSchemaHash(typeName) {
  const input = new TextEncoder().encode(typeName);
  const offsetBasis = 0xcbf29ce484222325n;
  const fnvPrime = 0x100000001b3n;
  let hash = offsetBasis;
  for (const byte of input) {
    hash ^= BigInt(byte);
    hash = (hash * fnvPrime) & 0xffffffffffffffffn;
  }
  const buffer = new Uint8Array(16);
  writeUint64LE(buffer, 0, hash);
  writeUint64LE(buffer, 8, hash);
  return buffer;
}

function writeUint64LE(buffer, offset, value) {
  let remaining = BigInt(value);
  for (let index = 0; index < 8; index += 1) {
    buffer[offset + index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
}

function writeUint64BE(buffer, offset, value) {
  let remaining = BigInt(value);
  for (let index = 7; index >= 0; index -= 1) {
    buffer[offset + index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
}

function writeUint32LE(buffer, offset, value) {
  let remaining = Number(value >>> 0);
  for (let index = 0; index < 4; index += 1) {
    buffer[offset + index] = remaining & 0xff;
    remaining >>= 8;
  }
}

function readUint64LE(buffer, offset) {
  let value = 0n;
  for (let index = 7; index >= 0; index -= 1) {
    value = (value << 8n) | BigInt(buffer[offset + index]);
  }
  return value;
}

function readUint32LE(buffer, offset) {
  let value = 0;
  for (let index = 3; index >= 0; index -= 1) {
    value = (value << 8) | (buffer[offset + index] ?? 0);
  }
  return value >>> 0;
}

function asUint8Array(value, name) {
  if (value instanceof Uint8Array) {
    return value;
  }
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }
  throw new TypeError(`${name ?? "data"} must be an ArrayBuffer or Uint8Array`);
}

function normalizeDirection(direction) {
  const key = typeof direction === "string" ? direction.trim().toLowerCase() : direction;
  switch (key) {
    case ConnectDirection.APP_TO_WALLET:
    case "app-to-wallet":
    case "app_to_wallet":
    case "app":
      return ConnectDirection.APP_TO_WALLET;
    case ConnectDirection.WALLET_TO_APP:
    case "wallet-to-app":
    case "wallet_to_app":
    case "wallet":
      return ConnectDirection.WALLET_TO_APP;
    default:
      throw new ConnectJournalError(`Unknown Connect journal direction: ${direction}`);
  }
}

function normalizeUint64(value, name) {
  if (typeof value === "bigint") {
    if (value < 0n) {
      throw new ConnectJournalError(`${name} must be non-negative`);
    }
    if (value > MAX_UINT64) {
      throw new ConnectJournalError(`${name} must fit into a uint64`);
    }
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value) || !Number.isInteger(value) || value < 0) {
      throw new ConnectJournalError(`${name} must be a non-negative integer`);
    }
    if (!Number.isSafeInteger(value)) {
      throw new ConnectJournalError(`${name} must be a safe integer`);
    }
    return BigInt(value);
  }
  if (typeof value === "string" && value.trim().length > 0) {
    let normalized;
    try {
      normalized = BigInt(value.trim());
    } catch {
      throw new ConnectJournalError(`${name} must be a non-negative integer`);
    }
    if (normalized < 0n) {
      throw new ConnectJournalError(`${name} must be non-negative`);
    }
    if (normalized > MAX_UINT64) {
      throw new ConnectJournalError(`${name} must fit into a uint64`);
    }
    return normalized;
  }
  throw new ConnectJournalError(`${name} must be a bigint, number, or numeric string`);
}

function normalizeTimestampMs(value, name) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric) || !Number.isInteger(numeric) || numeric < 0) {
    throw new ConnectJournalError(`${name} must be a non-negative integer`);
  }
  if (!Number.isSafeInteger(numeric)) {
    throw new ConnectJournalError(`${name} must be a safe integer`);
  }
  return numeric;
}

function ensurePayloadHash(bytes) {
  const input = asUint8Array(bytes, "payloadHash");
  if (input.length !== 32) {
    throw new ConnectJournalError(
      `payload hash must contain 32 bytes (received ${input.length})`,
    );
  }
  return input;
}

export class ConnectJournalRecord {
  constructor({
    direction,
    sequence,
    ciphertext,
    payloadHash,
    receivedAtMs,
    expiresAtMs,
  }) {
    this.direction = normalizeDirection(direction);
    this.sequence = normalizeUint64(sequence, "sequence");
    this.ciphertext = asUint8Array(ciphertext, "ciphertext");
    this.payloadHash =
      payloadHash !== undefined && payloadHash !== null
        ? ensurePayloadHash(payloadHash)
        : blake2b256(this.ciphertext);
    this.receivedAtMs = normalizeTimestampMs(receivedAtMs ?? Date.now(), "receivedAtMs");
    this.expiresAtMs = normalizeTimestampMs(expiresAtMs ?? this.receivedAtMs, "expiresAtMs");
    if (this.expiresAtMs < this.receivedAtMs) {
      throw new ConnectJournalError("expiresAtMs must be >= receivedAtMs");
    }
  }

  static fromCiphertext(options) {
    const retentionMs = Math.max(Number(options.retentionMs ?? 0), 1);
    const receivedAtMs = Number(options.receivedAtMs ?? Date.now());
    const expiresAtMs = receivedAtMs + retentionMs;
    return new ConnectJournalRecord({
      direction: options.direction,
      sequence: options.sequence,
      ciphertext: options.ciphertext,
      receivedAtMs,
      expiresAtMs,
    });
  }

  get payloadLength() {
    return PAYLOAD_FIXED_LEN + this.ciphertext.length;
  }

  get encodedLength() {
    return NORITO_HEADER_LEN + this.payloadLength;
  }

  encode() {
    const payloadLen = this.payloadLength;
    const frame = new Uint8Array(NORITO_HEADER_LEN + payloadLen);
    frame.set(NORITO_MAGIC, 0);
    frame[4] = NORITO_VERSION_MAJOR;
    frame[5] = NORITO_VERSION_MINOR;
    frame.set(SCHEMA_HASH, 6);
    frame[22] = NORITO_COMPRESSION_NONE;
    writeUint64LE(frame, 23, BigInt(payloadLen));
    // checksum placeholder (filled later)
    frame[39] = NORITO_FLAGS_NONE;
    const payloadStart = NORITO_HEADER_LEN;
    let cursor = payloadStart;
    const directionTag = DIRECTION_TAG[this.direction];
    if (directionTag === undefined) {
      throw new ConnectJournalError(`Cannot encode unknown direction: ${this.direction}`);
    }
    frame[cursor] = directionTag;
    cursor += 1;
    writeUint64LE(frame, cursor, this.sequence);
    cursor += 8;
    writeUint64LE(frame, cursor, BigInt(this.receivedAtMs));
    cursor += 8;
    writeUint64LE(frame, cursor, BigInt(this.expiresAtMs));
    cursor += 8;
    writeUint32LE(frame, cursor, this.ciphertext.length);
    cursor += 4;
    frame.set(this.payloadHash, cursor);
    cursor += 32;
    frame.set(this.ciphertext, cursor);

    const checksum = crc64(frame.subarray(payloadStart));
    writeUint64LE(frame, 31, checksum);
    return frame;
  }

  static decode(data, offset = 0) {
    const buffer = asUint8Array(data, "record");
    if (buffer.length - offset < NORITO_HEADER_LEN) {
      throw new ConnectJournalError("insufficient bytes for Norito header");
    }
    if (!matchesMagic(buffer.subarray(offset, offset + 4))) {
      throw new ConnectJournalError("invalid Norito magic in journal entry");
    }
    const major = buffer[offset + 4];
    const minor = buffer[offset + 5];
    if (major !== NORITO_VERSION_MAJOR || minor !== NORITO_VERSION_MINOR) {
      throw new ConnectJournalError(`unsupported Norito version ${major}.${minor}`);
    }
    for (let index = 0; index < SCHEMA_HASH.length; index += 1) {
      if (buffer[offset + 6 + index] !== SCHEMA_HASH[index]) {
        throw new ConnectJournalError("schema hash mismatch in Connect journal entry");
      }
    }
    const compression = buffer[offset + 22];
    if (compression !== NORITO_COMPRESSION_NONE) {
      throw new ConnectJournalError("compressed journal entries are not supported");
    }
    const payloadLen = Number(readUint64LE(buffer, offset + 23));
    if (payloadLen < PAYLOAD_FIXED_LEN) {
      throw new ConnectJournalError("journal entry payload too small");
    }
    const checksum = readUint64LE(buffer, offset + 31);
    const flags = buffer[offset + 39];
    if (flags !== NORITO_FLAGS_NONE) {
      throw new ConnectJournalError("journal entry contains unsupported flags");
    }
    const { payload, paddingLen } = findPayloadWithPadding(
      buffer,
      offset,
      payloadLen,
      checksum,
    );
    let cursor = 0;
    const directionTag = payload[cursor];
    cursor += 1;
    const direction = Object.keys(DIRECTION_TAG).find(
      (key) => DIRECTION_TAG[key] === directionTag,
    );
    if (!direction) {
      throw new ConnectJournalError(`unknown journal direction tag ${directionTag}`);
    }
    const sequence = readUint64LE(payload, cursor);
    cursor += 8;
    const receivedAtMs = Number(readUint64LE(payload, cursor));
    cursor += 8;
    const expiresAtMs = Number(readUint64LE(payload, cursor));
    cursor += 8;
    const ciphertextLen = readUint32LE(payload, cursor);
    cursor += 4;
    const payloadHash = payload.subarray(cursor, cursor + 32);
    cursor += 32;
    const ciphertext = payload.subarray(cursor, cursor + ciphertextLen);
    if (ciphertext.length !== ciphertextLen) {
      throw new ConnectJournalError("journal entry truncated ciphertext");
    }
    const record = new ConnectJournalRecord({
      direction,
      sequence,
      ciphertext,
      payloadHash,
      receivedAtMs,
      expiresAtMs,
    });
    return { record, bytesConsumed: NORITO_HEADER_LEN + paddingLen + payloadLen };
  }
}

function findPayloadWithPadding(buffer, offset, payloadLen, checksum) {
  const headerEnd = offset + NORITO_HEADER_LEN;
  const maxAvailable = buffer.length - headerEnd - payloadLen;
  if (maxAvailable < 0) {
    throw new ConnectJournalError("journal entry exceeds provided buffer");
  }
  const maxPadding = Math.min(MAX_HEADER_PADDING, maxAvailable);
  for (let padding = 0; padding <= maxPadding; padding += 1) {
    if (!paddingIsZero(buffer, headerEnd, padding)) {
      continue;
    }
    const payloadStart = headerEnd + padding;
    const payloadEnd = payloadStart + payloadLen;
    if (payloadEnd > buffer.length) {
      break;
    }
    const payload = buffer.subarray(payloadStart, payloadEnd);
    if (crc64(payload) === checksum) {
      return { payload, paddingLen: padding };
    }
  }
  throw new ConnectJournalError("journal entry checksum mismatch");
}

function paddingIsZero(buffer, start, length) {
  for (let i = 0; i < length; i += 1) {
    if (buffer[start + i] !== 0) {
      return false;
    }
  }
  return true;
}

function matchesMagic(slice) {
  for (let index = 0; index < NORITO_MAGIC.length; index += 1) {
    if (slice[index] !== NORITO_MAGIC[index]) {
      return false;
    }
  }
  return true;
}
