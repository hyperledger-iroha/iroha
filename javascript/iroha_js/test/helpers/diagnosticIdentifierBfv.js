// Test-only exact-lift arithmetic for diagnostic vectors; not secure application encryption.
import {createHash, randomBytes} from 'node:crypto';
import {chacha20orig} from '@noble/ciphers/chacha';
import {blake2b256} from '../../src/blake2b.js';
import {crc64Xz} from '../../src/crc64Xz.js';
import {normalizeIdentifierInput} from '../../src/normalizers.js';
import {getIdentifierBfvPublicParameters} from '../../src/toriiClient.js';
import {createValidationError, ValidationErrorCode} from '../../src/validationError.js';
import {rejectError, rejectType} from '../../src/validationThrow.js';
import {JS_TYPE_BIGINT, JS_TYPE_NUMBER, JS_TYPE_OBJECT, JS_TYPE_STRING} from '../../src/commonLiterals.js';

const BFV_IDENTIFIER_SCHEMA_NAME =
  "iroha_crypto::fhe_bfv::BfvIdentifierCiphertext";

const NORITO_COMPACT_LEN_FLAG = 0x02;

const BFV_IDENTIFIER_SEED_BYTES = 32;

const BFV_IDENTIFIER_MAX_INPUT_BYTES = 63;

const BFV_RUST_ENCRYPT_DOMAIN = Buffer.from(
  "iroha.crypto.fhe.bfv.encrypt.v1",
  "utf8",
);

const BFV_RUST_IDENTIFIER_SLOT_DOMAIN = Buffer.from(
  "iroha.crypto.fhe.bfv.identifier.slot.v1",
  "utf8",
);

const BFV_IDENTIFIER_SHA512_DOMAIN = Buffer.from(
  "iroha.sdk.identifier.bfv.prg.v1",
  "utf8",
);

const BFV_IDENTIFIER_SLOT_DOMAIN = Buffer.from(
  "iroha.sdk.identifier.bfv.slot.v1",
  "utf8",
);

const BFV_IDENTIFIER_U_DOMAIN = Buffer.from(
  "iroha.sdk.identifier.bfv.u.v1",
  "utf8",
);

const BFV_IDENTIFIER_E1_DOMAIN = Buffer.from(
  "iroha.sdk.identifier.bfv.e1.v1",
  "utf8",
);

const BFV_IDENTIFIER_E2_DOMAIN = Buffer.from(
  "iroha.sdk.identifier.bfv.e2.v1",
  "utf8",
);

const MAX_SAFE_INTEGER = Number.MAX_SAFE_INTEGER;

const MAX_SAFE_INTEGER_BIGINT = BigInt(MAX_SAFE_INTEGER);

function isPlainObject(value) {
  if (value === null || typeof value !== JS_TYPE_OBJECT || Array.isArray(value)) {
    return false;
  }
  const proto = Object.getPrototypeOf(value);
  return proto === Object.prototype || proto === null;
}

function ensureRecord(value, context) {
  if (!isPlainObject(value)) {
    rejectType(`${context} must be an object`);
  }
  return value;
}

function toBuffer(value) {
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (ArrayBuffer.isView(value)) {
    return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return Buffer.from(value);
  }
  if (Array.isArray(value)) {
    if (value.length === 0) {
      return Buffer.alloc(0);
    }
    return normalizeByteArray(value, "payload");
  }
  rejectType("payload must be a Buffer or ArrayBuffer view");
}

function normalizeByteArray(value, context) {
  const bytes = value.map((entry, index) => {
    if (!Number.isInteger(entry) || entry < 0 || entry > 255) {
      throw createValidationError(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${context}[${index}] must be an integer between 0 and 255`,
        `${context}[${index}]`,
      );
    }
    return entry;
  });
  return Buffer.from(bytes);
}

function requireHexString(value, name) {
  if (typeof value !== JS_TYPE_STRING) {
    throw createValidationError(
      ValidationErrorCode.INVALID_HEX,
      `${name} must be a hex string`,
      name,
    );
  }
  const normalized = value.trim();
  if (!normalized) {
    throw createValidationError(
      ValidationErrorCode.INVALID_HEX,
      `${name} must be a hex string`,
      name,
    );
  }
  const hasPrefix =
    normalized.startsWith("0x") || normalized.startsWith("0X");
  const hex = hasPrefix ? normalized.slice(2) : normalized;
  if (hex.length === 0 || hex.length % 2 !== 0 || !/^[0-9a-fA-F]+$/.test(hex)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_HEX,
      `${name} must be a hex string`,
      name,
    );
  }
  return normalized;
}

function assertSupportedOptionKeys(record, allowedKeys, context) {
  const extras = Object.keys(record).filter((key) => !allowedKeys.has(key));
  if (extras.length > 0) {
    const path = typeof context === JS_TYPE_STRING ? context.replace(/\s+/g, ".") : context;
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} contains unsupported fields: ${extras.join(", ")}`,
      path,
    );
  }
}

function irohaHashBytes(parts) {
  const digest = Buffer.from(blake2b256(Buffer.concat(parts.map((part) => Buffer.from(part)))));
  digest[digest.length - 1] |= 1;
  return digest;
}

function requireBfvUint(value, name, options = {}) {
  const allowZero = options.allowZero !== false;
  let integer;
  if (typeof value === JS_TYPE_BIGINT) {
    integer = value;
  } else if (typeof value === JS_TYPE_NUMBER) {
    if (!Number.isFinite(value) || !Number.isInteger(value) || !Number.isSafeInteger(value)) {
      throw createValidationError(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name} must be a safe integer number, bigint, or decimal string`,
        name,
      );
    }
    integer = BigInt(value);
  } else if (typeof value === JS_TYPE_STRING) {
    const trimmed = value.trim();
    if (!/^[0-9]+$/.test(trimmed)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_NUMERIC,
        `${name} must be a non-negative integer`,
        name,
      );
    }
    integer = BigInt(trimmed);
  } else {
    throw createValidationError(
      ValidationErrorCode.INVALID_NUMERIC,
      `${name} must be a non-negative integer`,
      name,
    );
  }
  if (integer < 0n || (!allowZero && integer === 0n)) {
    const qualifier = allowZero ? "non-negative integer" : "positive integer";
    throw createValidationError(
      ValidationErrorCode.INVALID_NUMERIC,
      `${name} must be a ${qualifier}`,
      name,
    );
  }
  return integer;
}

function requireSafeBfvUint(value, name, options = {}) {
  const integer = requireBfvUint(value, name, options);
  if (integer > MAX_SAFE_INTEGER_BIGINT) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} must be at most ${MAX_SAFE_INTEGER}`,
      name,
    );
  }
  return integer;
}

function u64ToLittleEndianBuffer(value) {
  const normalized = BigInt.asUintN(64, BigInt(value));
  const buffer = Buffer.alloc(8);
  buffer.writeBigUInt64LE(normalized);
  return buffer;
}

function createSha512Digest(parts) {
  const hash = createHash("sha512");
  for (const part of parts) {
    hash.update(part);
  }
  return hash.digest();
}

function deriveIdentifierBfvSeed(record, context) {
  const hasSeedHex = record.seedHex !== undefined && record.seedHex !== null;
  const hasSeedBytes = record.seed !== undefined && record.seed !== null;
  if (hasSeedHex && hasSeedBytes) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} must not supply both seed and seedHex`,
      `${context}.seed`,
    );
  }
  if (hasSeedHex) {
    return Buffer.from(requireHexString(record.seedHex, `${context}.seedHex`), "hex");
  }
  if (hasSeedBytes) {
    return toBuffer(record.seed);
  }
  return randomBytes(BFV_IDENTIFIER_SEED_BYTES);
}

class IdentifierBfvDeterministicStream {
  constructor(seed, domain) {
    this.seed = Buffer.from(seed);
    this.domain = Buffer.from(domain);
    this.counter = 0n;
    this.buffer = Buffer.alloc(0);
    this.offset = 0;
  }

  nextBytes(length) {
    let remaining = length;
    const chunks = [];
    while (remaining > 0) {
      if (this.offset >= this.buffer.length) {
        this.buffer = createSha512Digest([
          BFV_IDENTIFIER_SHA512_DOMAIN,
          this.domain,
          this.seed,
          u64ToLittleEndianBuffer(this.counter),
        ]);
        this.counter += 1n;
        this.offset = 0;
      }
      const available = Math.min(remaining, this.buffer.length - this.offset);
      chunks.push(this.buffer.subarray(this.offset, this.offset + available));
      this.offset += available;
      remaining -= available;
    }
    return Buffer.concat(chunks, length);
  }

  nextU64() {
    return this.nextBytes(8).readBigUInt64LE(0);
  }
}

class IdentifierBfvRustChaCha20Rng {
  constructor(seed) {
    this.key = Buffer.from(seed);
    this.nonce = Buffer.alloc(8);
    this.counter = 0;
    this.buffer = Buffer.alloc(0);
    this.offset = 0;
  }

  refill() {
    this.buffer = Buffer.from(
      chacha20orig(this.key, this.nonce, new Uint8Array(64), undefined, this.counter),
    );
    this.counter += 1;
    this.offset = 0;
  }

  nextBytes(length) {
    let remaining = length;
    const chunks = [];
    while (remaining > 0) {
      if (this.offset >= this.buffer.length) {
        this.refill();
      }
      const available = Math.min(remaining, this.buffer.length - this.offset);
      chunks.push(this.buffer.subarray(this.offset, this.offset + available));
      this.offset += available;
      remaining -= available;
    }
    return Buffer.concat(chunks, length);
  }

  nextU32() {
    return this.nextBytes(4).readUInt32LE(0);
  }
}

function noritoSchemaHash(typeName) {
  return createHash("sha256")
    .update(Buffer.from("norito:v1:type-name\0", "utf8"))
    .update(Buffer.from(typeName, "utf8"))
    .digest()
    .subarray(0, 16);
}

function frameNoritoPayload(typeName, payload, flags = 0) {
  const header = Buffer.concat([
    Buffer.from("NRT0", "ascii"),
    Buffer.from([0, 0]),
    noritoSchemaHash(typeName),
    Buffer.from([0]),
    u64ToLittleEndianBuffer(payload.length),
    u64ToLittleEndianBuffer(crc64Xz(payload)),
    Buffer.from([flags & 0xff]),
  ]);
  return Buffer.concat([header, payload]);
}

function encodeUnsignedLeb128(value) {
  const out = [];
  let remaining = BigInt(value);
  do {
    let byte = Number(remaining & 0x7fn);
    remaining >>= 7n;
    if (remaining !== 0n) {
      byte |= 0x80;
    }
    out.push(byte);
  } while (remaining !== 0n);
  return Buffer.from(out);
}

function encodeNoritoLength(value, compact) {
  return compact ? encodeUnsignedLeb128(value) : u64ToLittleEndianBuffer(value);
}

function encodeNoritoField(payload, compact = false) {
  return Buffer.concat([encodeNoritoLength(payload.length, compact), payload]);
}

function encodeNoritoU64(value) {
  return u64ToLittleEndianBuffer(value);
}

function encodeNoritoVec(values, encode, compact = false) {
  const parts = [u64ToLittleEndianBuffer(values.length)];
  for (const value of values) {
    const payload = encode(value);
    parts.push(encodeNoritoLength(payload.length, compact), payload);
  }
  return Buffer.concat(parts);
}

function encodeNoritoBfvCiphertext(ciphertext, compact = false) {
  return Buffer.concat([
    encodeNoritoField(encodeNoritoVec(ciphertext.c0, encodeNoritoU64, compact), compact),
    encodeNoritoField(encodeNoritoVec(ciphertext.c1, encodeNoritoU64, compact), compact),
  ]);
}

function encodeNoritoBfvIdentifierCiphertext(ciphertext, compact = false) {
  return frameNoritoPayload(
    BFV_IDENTIFIER_SCHEMA_NAME,
    encodeNoritoField(
      encodeNoritoVec(
        ciphertext.slots,
        (slot) => encodeNoritoBfvCiphertext(slot, compact),
        compact,
      ),
      compact,
    ),
    compact ? NORITO_COMPACT_LEN_FLAG : 0,
  );
}

function identifierBfvInputBytes(input, normalization, name) {
  if (input instanceof Uint8Array) {
    const mode = requireNonEmptyString(normalization, `${name}Normalization`)
      .trim()
      .toLowerCase();
    if (mode !== "exact") {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${name} byte input requires exact normalization`,
        name,
      );
    }
    const bytes = Buffer.from(input);
    if (bytes.length === 0) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${name} must be non-empty`,
        name,
      );
    }
    return bytes;
  }
  const normalizedInput = normalizeIdentifierInput(input, normalization, name);
  return Buffer.from(normalizedInput, "utf8");
}

function normalizeIdentifierBfvEncryptionInputs(policySummary, input, options = {}) {
  const publicParameters = getIdentifierBfvPublicParameters(policySummary);
  const normalizedPolicy = policySummary;
  if (normalizedPolicy.input_encryption !== "bfv-v1") {
    rejectError(`encryptIdentifierInputForPolicy: policy ${normalizedPolicy.policy_id} does not publish BFV encrypted-input support`);
  }
  if (!publicParameters) {
    rejectError(`encryptIdentifierInputForPolicy: policy ${normalizedPolicy.policy_id} is missing decoded BFV public parameters`);
  }
  const inputBytes = identifierBfvInputBytes(
    input,
    normalizedPolicy.normalization,
    "encryptIdentifierInputForPolicy.input",
  );
  const record = ensureRecord(options, "encryptIdentifierInputForPolicy options");
  assertSupportedOptionKeys(
    record,
    new Set(["seed", "seedHex"]),
    "encryptIdentifierInputForPolicy options",
  );
  return {
    policy: normalizedPolicy,
    publicParameters,
    inputBytes,
    seed: deriveIdentifierBfvSeed(record, "encryptIdentifierInputForPolicy options"),
  };
}

function validateIdentifierBfvPublicParameters(publicParameters, context) {
  const params = publicParameters.parameters;
  const polynomialDegree = Number(
    requireSafeBfvUint(params.polynomial_degree, `${context}.parameters.polynomial_degree`),
  );
  const plaintextModulus = requireBfvUint(
    params.plaintext_modulus,
    `${context}.parameters.plaintext_modulus`,
  );
  const ciphertextModulus = requireBfvUint(
    params.ciphertext_modulus,
    `${context}.parameters.ciphertext_modulus`,
  );
  const decompositionBaseLog = Number(
    requireSafeBfvUint(
      params.decomposition_base_log,
      `${context}.parameters.decomposition_base_log`,
    ),
  );
  const maxInputBytes = Number(
    requireSafeBfvUint(publicParameters.max_input_bytes, `${context}.max_input_bytes`),
  );
  const noritoLengthEncoding =
    publicParameters.norito_length_encoding === undefined ||
    publicParameters.norito_length_encoding === null
      ? "u64-v1"
      : String(publicParameters.norito_length_encoding).trim();
  if (!["u64-v1", "compact-v1"].includes(noritoLengthEncoding)) {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${context}.norito_length_encoding must be u64-v1 or compact-v1`,
      `${context}.norito_length_encoding`,
    );
  }
  if (polynomialDegree < 2 || (polynomialDegree & (polynomialDegree - 1)) !== 0) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.parameters.polynomial_degree must be a power of two and at least 2`,
      `${context}.parameters.polynomial_degree`,
    );
  }
  if (decompositionBaseLog < 1 || decompositionBaseLog > 16) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.parameters.decomposition_base_log must be within 1..=16`,
      `${context}.parameters.decomposition_base_log`,
    );
  }
  if (plaintextModulus < 2n) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.parameters.plaintext_modulus must be at least 2`,
      `${context}.parameters.plaintext_modulus`,
    );
  }
  if (ciphertextModulus <= plaintextModulus) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.parameters.ciphertext_modulus must be greater than plaintext_modulus`,
      `${context}.parameters.ciphertext_modulus`,
    );
  }
  if (ciphertextModulus % plaintextModulus !== 0n) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.parameters.ciphertext_modulus must be divisible by plaintext_modulus`,
      `${context}.parameters.ciphertext_modulus`,
    );
  }
  if (maxInputBytes < 1) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.max_input_bytes must be at least 1`,
      `${context}.max_input_bytes`,
    );
  }
  if (BigInt(maxInputBytes) >= plaintextModulus) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.max_input_bytes must fit into one plaintext slot`,
      `${context}.max_input_bytes`,
    );
  }
  if (maxInputBytes > BFV_IDENTIFIER_MAX_INPUT_BYTES) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.max_input_bytes must be at most ${BFV_IDENTIFIER_MAX_INPUT_BYTES} for the registered RAM-LFE BFV identifier profile`,
      `${context}.max_input_bytes`,
    );
  }
  const publicKey = publicParameters.public_key;
  const b = publicKey.b.map((value, index) =>
    requireBfvUint(value, `${context}.public_key.b[${index}]`),
  );
  const a = publicKey.a.map((value, index) =>
    requireBfvUint(value, `${context}.public_key.a[${index}]`),
  );
  if (a.length !== polynomialDegree || b.length !== polynomialDegree) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.public_key polynomials must match polynomial_degree`,
      `${context}.public_key`,
    );
  }
  for (const [arrayName, coefficients] of [
    ["a", a],
    ["b", b],
  ]) {
    for (let index = 0; index < coefficients.length; index += 1) {
      if (coefficients[index] >= ciphertextModulus) {
        throw createValidationError(
          ValidationErrorCode.VALUE_OUT_OF_RANGE,
          `${context}.public_key.${arrayName}[${index}] exceeds ciphertext_modulus`,
          `${context}.public_key.${arrayName}[${index}]`,
        );
      }
    }
  }
  return {
    polynomialDegree,
    plaintextModulus,
    ciphertextModulus,
    maxInputBytes,
    noritoLengthEncoding,
    publicKey: { a, b },
  };
}

function addModBigInt(lhs, rhs, modulus) {
  return (lhs + rhs) % modulus;
}

function subModBigInt(lhs, rhs, modulus) {
  return lhs >= rhs ? lhs - rhs : modulus - ((rhs - lhs) % modulus);
}

function mulModBigInt(lhs, rhs, modulus) {
  return (lhs * rhs) % modulus;
}

function polyAddMod(params, lhs, rhs) {
  return lhs.map((value, index) =>
    addModBigInt(value, rhs[index], params.ciphertextModulus),
  );
}

function polyMulMod(params, lhs, rhs) {
  const out = Array.from({ length: params.polynomialDegree }, () => 0n);
  for (let i = 0; i < params.polynomialDegree; i += 1) {
    for (let j = 0; j < params.polynomialDegree; j += 1) {
      const term = mulModBigInt(lhs[i], rhs[j], params.ciphertextModulus);
      const target = i + j;
      if (target < params.polynomialDegree) {
        out[target] = addModBigInt(out[target], term, params.ciphertextModulus);
      } else {
        out[target - params.polynomialDegree] = subModBigInt(
          out[target - params.polynomialDegree],
          term,
          params.ciphertextModulus,
        );
      }
    }
  }
  return out;
}

function encodeIdentifierSlots(params, inputBytes) {
  if (inputBytes.length > params.maxInputBytes) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `encryptIdentifierInputForPolicy.input exceeds max_input_bytes ${params.maxInputBytes}`,
      "encryptIdentifierInputForPolicy.input",
    );
  }
  const slots = Array.from({ length: params.maxInputBytes + 1 }, () => 0n);
  slots[0] = BigInt(inputBytes.length);
  for (let index = 0; index < inputBytes.length; index += 1) {
    slots[index + 1] = BigInt(inputBytes[index]);
  }
  return slots;
}

function sampleSmallPoly(params, stream) {
  return Array.from({ length: params.polynomialDegree }, () => {
    const sample = Number(stream.nextBytes(1)[0] % 3);
    if (sample === 0) {
      return 0n;
    }
    if (sample === 1) {
      return 1n;
    }
    return params.ciphertextModulus - 1n;
  });
}

function sampleErrorPoly(params, stream) {
  return Array.from({ length: params.polynomialDegree }, () => {
    const sample = Number(stream.nextBytes(1)[0] % 3);
    if (sample === 0) {
      return 0n;
    }
    if (sample === 1) {
      return params.plaintextModulus;
    }
    return params.ciphertextModulus - params.plaintextModulus;
  });
}

function rustHashDerivedRng(domain, seed) {
  return new IdentifierBfvRustChaCha20Rng(irohaHashBytes([domain, seed]));
}

function rustIdentifierSlotSeed(seed, index) {
  return irohaHashBytes([
    BFV_RUST_IDENTIFIER_SLOT_DOMAIN,
    seed,
    u64ToLittleEndianBuffer(index),
  ]);
}

function sampleSmallPolyRust(params, rng) {
  return Array.from({ length: params.polynomialDegree }, () => {
    const reduced = rustRandomRangeU8Inclusive0To2(rng);
    if (reduced === 0) {
      return 0n;
    }
    if (reduced === 1) {
      return 1n;
    }
    return params.ciphertextModulus - 1n;
  });
}

function sampleErrorPolyRust(params, rng) {
  return Array.from({ length: params.polynomialDegree }, () => {
    const reduced = rustRandomRangeU8Inclusive0To2(rng);
    if (reduced === 0) {
      return 0n;
    }
    if (reduced === 1) {
      return params.plaintextModulus;
    }
    return params.ciphertextModulus - params.plaintextModulus;
  });
}

function rustRandomRangeU8Inclusive0To2(rng) {
  const range = 3n;
  const u32Modulus = 1n << 32n;
  const u32Mask = u32Modulus - 1n;
  const sample = BigInt(rng.nextU32());
  const product = sample * range;
  let result = Number(product >> 32n);
  const loOrder = product & u32Mask;
  const biasedThreshold = u32Modulus - range;
  if (loOrder > biasedThreshold) {
    const newProduct = BigInt(rng.nextU32()) * range;
    const newHiOrder = newProduct >> 32n;
    if (loOrder + newHiOrder > u32Mask) {
      result += 1;
    }
  }
  return result;
}

function encryptIdentifierScalar(params, scalar, seed) {
  const u = sampleSmallPoly(
    params,
    new IdentifierBfvDeterministicStream(seed, BFV_IDENTIFIER_U_DOMAIN),
  );
  const e1 = sampleErrorPoly(
    params,
    new IdentifierBfvDeterministicStream(seed, BFV_IDENTIFIER_E1_DOMAIN),
  );
  const e2 = sampleErrorPoly(
    params,
    new IdentifierBfvDeterministicStream(seed, BFV_IDENTIFIER_E2_DOMAIN),
  );
  const encoded = Array.from({ length: params.polynomialDegree }, () => 0n);
  encoded[0] = scalar % params.plaintextModulus;
  return {
    c0: polyAddMod(
      params,
      polyAddMod(params, polyMulMod(params, params.publicKey.b, u), e1),
      encoded,
    ),
    c1: polyAddMod(params, polyMulMod(params, params.publicKey.a, u), e2),
  };
}

function encryptIdentifierScalarRust(params, scalar, seed) {
  const rng = rustHashDerivedRng(BFV_RUST_ENCRYPT_DOMAIN, seed);
  const u = sampleSmallPolyRust(params, rng);
  const e1 = sampleErrorPolyRust(params, rng);
  const e2 = sampleErrorPolyRust(params, rng);
  const encoded = Array.from({ length: params.polynomialDegree }, () => 0n);
  encoded[0] = scalar % params.plaintextModulus;
  return {
    c0: polyAddMod(
      params,
      polyAddMod(params, polyMulMod(params, params.publicKey.b, u), e1),
      encoded,
    ),
    c1: polyAddMod(params, polyMulMod(params, params.publicKey.a, u), e2),
  };
}

export function encryptDiagnosticIdentifierInputForPolicy(policySummary, input, options = {}) {
  const { publicParameters, inputBytes, seed } = normalizeIdentifierBfvEncryptionInputs(
    policySummary,
    input,
    options,
  );
  const params = validateIdentifierBfvPublicParameters(
    publicParameters,
    "encryptIdentifierInputForPolicy.policy.input_encryption_public_parameters_decoded",
  );
  const slots = encodeIdentifierSlots(params, inputBytes).map((scalar, index) => {
    if (params.noritoLengthEncoding === "compact-v1") {
      return encryptIdentifierScalarRust(params, scalar, rustIdentifierSlotSeed(seed, index));
    }
    const slotSeed = createSha512Digest([
      BFV_IDENTIFIER_SLOT_DOMAIN,
      seed,
      u64ToLittleEndianBuffer(index),
    ]);
    return encryptIdentifierScalar(params, scalar, slotSeed);
  });
  const ciphertext = {
    slots: slots.map((slot) => ({
      c0: slot.c0,
      c1: slot.c1,
    })),
  };
  return encodeNoritoBfvIdentifierCiphertext(
    ciphertext,
    params.noritoLengthEncoding === "compact-v1",
  ).toString("hex");
}

function requireNonEmptyString(value, name) {
  if (typeof value !== JS_TYPE_STRING) {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be a string`,
      name,
    );
  }
  const trimmed = value.trim();
  if (!trimmed) {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${name} must not be empty`,
      name,
    );
  }
  return trimmed;
}
