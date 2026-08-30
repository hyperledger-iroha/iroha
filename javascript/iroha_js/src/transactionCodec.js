import { Buffer } from "buffer";
import {
  BASE58_ALPHABET_TEXT,
  ED25519_ALGORITHM,
  HEX_ENCODING,
  JS_TYPE_BIGINT,
  JS_TYPE_NUMBER,
  JS_TYPE_OBJECT,
  JS_TYPE_STRING,
  UTF8_ENCODING,
} from "./commonLiterals.js";
import { blake3 } from "@noble/hashes/blake3";
import { AccountAddress } from "./address.js";
import { blake2b256 } from "./blake2b.js";
import { verifyEd25519Strict as verifyEd25519 } from "./ed25519Strict.js";
import { crc64Xz } from "./crc64Xz.js";
import { parseCanonicalContractAddress } from "./contractAddress.js";
import {
  encodeAccountIdNoritoValue,
  noritoDecodeInstructionBoxArchive,
  noritoEncodeInstructionBoxArchive,
  validateNoritoFrame,
} from "./norito.js";
import {
  KotodamaQuantity,
  NumericV1,
  NumericV1Error,
} from "./numericV1.js";
import { networkIdBytes } from "./networkId.js";

const COMPACT_LEN_FLAG = 0x02;
const MALFORMED_PAYLOAD = "malformed_payload";
const AUTHORITY_MISMATCH = "authority_mismatch";
const INVALID_FEE_PAYMENT = "invalid_fee_payment";
const INVALID_METADATA = "invalid_metadata";
const UNSUPPORTED_INSTRUCTION = "unsupported_instruction";
const UNSUPPORTED_EXECUTABLE = "unsupported_executable";
const UNSUPPORTED_ALGORITHM = "unsupported_algorithm";
const INVALID_PUBLIC_KEY = "invalid_public_key";
const PAYLOAD_HASH_MISMATCH = "payload_hash_mismatch";
const UNSUPPORTED_PAYLOAD = "unsupported_payload";
const INVALID_SIGNATURE = "invalid_signature";
const MALFORMED_SIGNED_TRANSACTION = "malformed_signed_transaction";
const MALFORMED_INSTRUCTION = "malformed_instruction";
const INVALID_QUANTITY = "invalid_quantity";
const UNSUPPORTED_AUTHORITY = "unsupported_authority";
const BOUNDS_EXCEEDED = "bounds_exceeded";
const INVALID_INPUT = "invalid_input";
const INVALID_ASSET = "invalid_asset";
const INVALID_BYTES = "invalid_bytes";
const INVALID_INTEGER = "invalid_integer";
const INVALID_ACCOUNT = "invalid_account";
const SIGNABLE_PAYLOAD_HASH_MISMATCH_MESSAGE = "signable.payloadHashHex does not match payloadBytes";
const SIGNABLE_ALGORITHM_MESSAGE = "signable.signatureAlgorithm must be ed25519";
const NETWORK_SELECTOR_CONFLICT_MESSAGE = "provide only one of networkPrefix or chainDiscriminant";
const SIGNABLE_NETWORK_ID_CONTEXT = "signable.networkId";
const SIGNABLE_PUBLIC_KEY_CONTEXT = "signable.signingPublicKey";
const PROOF_ATTACHMENT_UNSUPPORTED_MESSAGE = "transaction payload proof attachments are not supported by the browser codec";
const APPROVED_SIGNER_MISMATCH_MESSAGE = "signable.signingPublicKey does not match the expected approved signing key";
const SIGNABLE_PAYLOAD_HASH_CONTEXT = "signable.payloadHashHex";
const SIGNABLE_PAYLOAD_BYTES_CONTEXT = "signable.payloadBytes";
const APPROVED_AUTHORITY_MISMATCH_MESSAGE = "signable.authority does not match the expected approved authority";
const SIGNATURE_VERIFY_MESSAGE = "Ed25519 signature does not verify over the Iroha payload hash";
const SIGNABLE_CONTROLLER_MISMATCH_MESSAGE = "signable.signingPublicKey does not control signable.authority";
const PUBLIC_KEY_ARGUMENT_MISMATCH_MESSAGE = "signable.signingPublicKey does not match signingPublicKey";
const ARGUMENT_CONTROLLER_MISMATCH_MESSAGE = "signingPublicKey does not control signable.authority";
const SIGNABLE_PUBLIC_KEY_LENGTH_MESSAGE = "signable.signingPublicKey must be exactly 32 bytes";
const SIGNABLE_AUTHORITY_PAYLOAD_MISMATCH_MESSAGE = "signable.authority does not match payloadBytes";
const UINT16_MAX = 0xffffn;
const UINT32_MAX = 0xffff_ffffn;
const UINT64_MAX = 0xffff_ffff_ffff_ffffn;
const MAX_METADATA_JSON_BYTES = 64 * 1024;
const MAX_METADATA_ENTRIES = 64;
const MAX_METADATA_DEPTH = 32;
const MAX_METADATA_NODES = 4096;
const MAX_METADATA_KEY_BYTES = 255;
const MAX_PAYLOAD_BYTES = 1024 * 1024;
const MAX_VERIFYING_KEY_DRAFT_PAYLOAD_BYTES = 16 * 1024 * 1024;
const MAX_VERIFYING_KEY_DRAFT_INSTRUCTION_FIELDS = 256;
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
  BASE58_ALPHABET_TEXT;
const BASE58_LOOKUP = new Map(
  Array.from(BASE58_ALPHABET, (character, index) => [character, BigInt(index)]),
);
const TRANSFER_SCHEMA_HASH = Buffer.from(
  "a4174c78d6341f8f98fc2adae8ed67b9",
  HEX_ENCODING,
);
const MAX_BROWSER_INSTRUCTIONS = 64;
const MAX_CONTRACT_ARGUMENT_RECORD_BYTES = 1024 * 1024;
const MAX_CONTRACT_ENTRYPOINT_BYTES = 1024;
const DEFAULT_TRANSACTION_TTL_MS = 100_000;
const TRANSACTION_ADMISSION_ORDINARY_TAG = 0;
const TRANSACTION_ADMISSION_QUEUE_PLAN_SYNCED_TAG = 1;
const SMART_CONTRACT_DEPLOYMENT_WIRE_IDS = new Set([
  "iroha.instruction.v1::smart_contract_code::UploadSmartContractCodeChunk",
  "iroha.instruction.v1::smart_contract_code::FinalizeSmartContractCodeUpload",
  "iroha.instruction.v1::smart_contract_code::CancelSmartContractCodeUpload",
  "iroha.instruction.v1::smart_contract_code::RegisterSmartContractCode",
  "iroha.instruction.v1::smart_contract_code::CommitContractDeployment",
]);
const TRANSFER_INPUT_FIELDS = new Set([
  "networkId",
  "authority",
  "sourceAssetHoldingId",
  "sourceAssetId",
  "quantity",
  "destinationAccountId",
  "metadata",
  "creationTimeMs",
  "ttlMs",
  "nonce",
  "feePayment",
  "networkPrefix",
  "chainDiscriminant",
]);
const INSTRUCTION_INPUT_FIELDS = new Set([
  "networkId",
  "authority",
  "instructions",
  "metadata",
  "creationTimeMs",
  "ttlMs",
  "nonce",
  "feePayment",
  "networkPrefix",
  "chainDiscriminant",
]);
const EXECUTABLE_BATCH_INPUT_FIELDS = new Set([
  "networkId",
  "authority",
  "entries",
  "metadata",
  "creationTimeMs",
  "ttlMs",
  "nonce",
  "feePayment",
  "networkPrefix",
  "chainDiscriminant",
]);
const BATCH_INSTRUCTION_ENTRY_FIELDS = new Set(["kind", "instruction"]);
const BATCH_CONTRACT_CALL_ENTRY_FIELDS = new Set([
  "kind",
  "contractAddress",
  "expectedCodeHash",
  "entrypoint",
  "arguments",
]);
const FEE_PAYMENT_FIELDS = new Set([
  "payer",
  "programId",
  "programRevision",
  "chargeLimits",
  "gasLimit",
]);
const FEE_CHARGE_LIMIT_FIELDS = new Set([
  "kind",
  "assetDefinitionId",
  "maxAmount",
]);
const FEE_CHARGE_KIND_TAG = Object.freeze({
  nexus: 0,
  pipelineGas: 1,
});
const LEGACY_FEE_METADATA_KEYS = Object.freeze([
  "fee_sponsor",
  "gas_limit",
  "gas_asset_id",
]);
const SIGNABLE_FIELDS = new Set([
  "networkId",
  "payloadBytes",
  "payloadHashHex",
  "authority",
  "signingPublicKey",
  "signatureAlgorithm",
]);
const SIGNABLE_CONSTRAINT_FIELDS = new Set([
  "networkId",
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
        fail(MALFORMED_PAYLOAD, `${this.context}.${field} is too large`);
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
          fail(MALFORMED_PAYLOAD, `${this.context}.${field} is too large`);
        }
        const canonical = encodeCompactLength(value);
        const consumed = this.bytes.subarray(start, this.offset);
        if (!consumed.equals(canonical)) {
          fail(
            MALFORMED_PAYLOAD,
            `${this.context}.${field} uses a non-canonical length prefix`,
          );
        }
        return Number(value);
      }
      shift += 7n;
    }
    fail(MALFORMED_PAYLOAD, `${this.context}.${field} length prefix is invalid`);
  }

  readField(field) {
    return this.readBytes(this.readLength(`${field}.length`), `${field}.value`);
  }

  readBytes(length, field) {
    if (!Number.isSafeInteger(length) || length < 0) {
      fail(MALFORMED_PAYLOAD, `${this.context}.${field} has an invalid length`);
    }
    const end = this.offset + length;
    if (end > this.bytes.length) {
      fail(MALFORMED_PAYLOAD, `${this.context}.${field} is truncated`);
    }
    const value = this.bytes.subarray(this.offset, end);
    this.offset = end;
    return value;
  }

  assertEof() {
    if (this.offset !== this.bytes.length) {
      fail(
        MALFORMED_PAYLOAD,
        `${this.context} contains ${this.bytes.length - this.offset} trailing bytes`,
      );
    }
  }
}

function fail(code, message) {
  throw new BrowserTransactionCodecError(code, message);
}

function exactNetworkId(value, context = "networkId") {
  try {
    return Buffer.from(networkIdBytes(value, context));
  } catch (error) {
    fail(
      INVALID_INPUT,
      error instanceof Error ? error.message : `${context} must be a NetworkId`,
    );
  }
}

function requireExpectedNetworkId(actual, expected, context) {
  if (!actual.equals(expected)) {
    fail(
      "network_id_mismatch",
      `${context} does not match the application-pinned NetworkId`,
    );
  }
}

function isPlainDataObject(value) {
  if (value === null || typeof value !== JS_TYPE_OBJECT || Array.isArray(value)) {
    return false;
  }
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function assertPlainDataObject(value, context) {
  if (value === null || typeof value !== JS_TYPE_OBJECT || Array.isArray(value)) {
    fail(INVALID_INPUT, `${context} must be a plain object`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    fail(INVALID_INPUT, `${context} must be a plain object`);
  }
  for (const key of Reflect.ownKeys(value)) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (
      typeof key !== JS_TYPE_STRING ||
      !descriptor ||
      !descriptor.enumerable ||
      !Object.prototype.hasOwnProperty.call(descriptor, "value")
    ) {
      fail(INVALID_INPUT, `${context} must contain only enumerable data fields`);
    }
  }
  return value;
}

function snapshotAllowedFields(value, allowed, context) {
  if (value === null || typeof value !== JS_TYPE_OBJECT || Array.isArray(value)) {
    fail(INVALID_INPUT, `${context} must be a plain object`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    fail(INVALID_INPUT, `${context} must be a plain object`);
  }
  const snapshot = Object.create(null);
  for (const key of Reflect.ownKeys(value)) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (
      typeof key !== JS_TYPE_STRING ||
      !descriptor ||
      !descriptor.enumerable ||
      !Object.prototype.hasOwnProperty.call(descriptor, "value")
    ) {
      fail(INVALID_INPUT, `${context} must contain only enumerable data fields`);
    }
    if (!allowed.has(key)) {
      fail(INVALID_INPUT, `${context}.${key} is not supported`);
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
  if (typeof value !== JS_TYPE_STRING || value.length === 0) {
    fail(INVALID_INPUT, `${context} must be a non-empty exact string`);
  }
  if (maxBytes !== undefined && value.length > maxBytes) {
    fail(BOUNDS_EXCEEDED, `${context} exceeds ${maxBytes} UTF-8 bytes`);
  }
  if (!isWellFormedUnicode(value)) {
    fail(INVALID_INPUT, `${context} must contain only Unicode scalar values`);
  }
  if (value.trim() !== value) {
    fail(INVALID_INPUT, `${context} must be a non-empty exact string`);
  }
  const length = Buffer.byteLength(value, UTF8_ENCODING);
  if (maxBytes !== undefined && length > maxBytes) {
    fail(BOUNDS_EXCEEDED, `${context} exceeds ${maxBytes} UTF-8 bytes`);
  }
  if (!allowControls && /[\u0000-\u001f\u007f-\u009f]/u.test(value)) {
    fail(INVALID_INPUT, `${context} must not contain control characters`);
  }
  if (value.normalize("NFC") !== value) {
    fail(INVALID_INPUT, `${context} must use NFC normalization`);
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
    fail(INVALID_ACCOUNT, `${context} is not a canonical I105 account: ${error.message}`);
  }
  const discriminant = parsed.chainDiscriminant;
  if (!Number.isInteger(discriminant)) {
    fail(INVALID_ACCOUNT, `${context} does not advertise a chain discriminant`);
  }
  let canonical;
  try {
    canonical = parsed.address.toI105(discriminant);
  } catch (error) {
    fail(INVALID_ACCOUNT, `${context} could not be rendered canonically: ${error.message}`);
  }
  if (canonical !== literal) {
    fail(INVALID_ACCOUNT, `${context} must use its exact canonical I105 form`);
  }
  const controller = parsed.address._controller;
  if (
    !controller ||
    controller.tag !== 0 ||
    controller.curve !== 1 ||
    controller.publicKey?.length !== 32
  ) {
    fail(
      UNSUPPORTED_AUTHORITY,
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
  if (typeof value === JS_TYPE_BIGINT) {
    result = value;
  } else if (typeof value === JS_TYPE_NUMBER) {
    if (!Number.isSafeInteger(value)) {
      fail(INVALID_INTEGER, `${context} must be a safe integer, bigint, or decimal string`);
    }
    result = BigInt(value);
  } else if (typeof value === JS_TYPE_STRING) {
    const maximumDigits = maximum.toString(10).length;
    if (value.length > maximumDigits) {
      fail(BOUNDS_EXCEEDED, `${context} is outside its canonical range`);
    }
    if (!/^(?:0|[1-9]\d*)$/u.test(value)) {
      fail(INVALID_INTEGER, `${context} must be a canonical unsigned integer`);
    }
    result = BigInt(value);
  } else {
    fail(INVALID_INTEGER, `${context} must be a canonical unsigned integer`);
  }
  if (result < 0n || result > maximum) {
    fail(BOUNDS_EXCEEDED, `${context} is outside its canonical range`);
  }
  return result;
}

function normalizeOptionalUnsigned(value, maximum, context, { nonZero = false } = {}) {
  if (value === undefined || value === null) {
    return null;
  }
  const result = normalizeUnsigned(value, maximum, context);
  if (nonZero && result === 0n) {
    fail(INVALID_INTEGER, `${context} must be non-zero when provided`);
  }
  return result;
}

function normalizeQuantity(value) {
  let quantity;
  try {
    if (value instanceof KotodamaQuantity) {
      quantity = new KotodamaQuantity(value.mantissa, value.scale);
    } else if (typeof value === JS_TYPE_STRING) {
      if (value.length > MAX_QUANTITY_LITERAL_CODE_UNITS) {
        fail(BOUNDS_EXCEEDED, "quantity exceeds the canonical 511-bit positive bound");
      }
      quantity = NumericV1.decodeQuantityJson(value);
    } else if (typeof value === JS_TYPE_BIGINT) {
      quantity = new KotodamaQuantity(value, 0);
    } else {
      fail(
        INVALID_QUANTITY,
        "quantity must be a KotodamaQuantity, canonical quantity string, or bigint; JavaScript numbers are rejected",
      );
    }
  } catch (error) {
    if (!(error instanceof NumericV1Error)) throw error;
    if (error.code === "mantissa_overflow" || error.code === "invalid_scale") {
      fail(BOUNDS_EXCEEDED, `quantity is outside the bounded Kotodama V1 domain (${error.code})`);
    }
    fail(INVALID_QUANTITY, `quantity must be canonical and non-negative (${error.code})`);
  }
  const literal = quantity.toString();
  const mantissa = quantity.mantissa;
  if (mantissa === 0n) {
    fail(INVALID_QUANTITY, "quantity must be greater than zero");
  }
  return { literal, mantissa, scale: quantity.scale };
}

function normalizeFeeProgramName(value, context) {
  const name = exactString(value, context, { maxBytes: MAX_METADATA_KEY_BYTES });
  if (
    name.normalize("NFC") !== name ||
    isRustUnicodeWhitespace(name) ||
    /[@#$]/u.test(name)
  ) {
    fail(INVALID_FEE_PAYMENT, `${context} is not a canonical program Name`);
  }
  return name;
}

function normalizeFeeProgramId(value, authority, context) {
  const literal = exactString(value, context, { maxBytes: 1024 });
  const separator = literal.indexOf("/");
  if (separator <= 0 || separator === literal.length - 1) {
    fail(
      INVALID_FEE_PAYMENT,
      `${context} must use <canonical-sponsor-account>/<program-name>`,
    );
  }
  const sponsor = accountInfo(
    literal.slice(0, separator),
    `${context}.sponsor`,
    authority.discriminant,
  );
  const name = normalizeFeeProgramName(
    literal.slice(separator + 1),
    `${context}.name`,
  );
  if (`${sponsor.literal}/${name}` !== literal) {
    fail(INVALID_FEE_PAYMENT, `${context} must use its exact canonical form`);
  }
  return { literal, sponsor, name };
}

function normalizeFeeChargeLimits(value) {
  if (!Array.isArray(value)) {
    fail(INVALID_FEE_PAYMENT, "feePayment.chargeLimits must be an array");
  }
  if (value.length > Object.keys(FEE_CHARGE_KIND_TAG).length) {
    fail(INVALID_FEE_PAYMENT, "feePayment.chargeLimits contains too many entries");
  }
  let previousTag = -1;
  return value.map((entry, index) => {
    entry = snapshotAllowedFields(
      entry,
      FEE_CHARGE_LIMIT_FIELDS,
      `feePayment.chargeLimits[${index}]`,
    );
    const kind = exactString(
      entry.kind,
      `feePayment.chargeLimits[${index}].kind`,
      { maxBytes: 32 },
    );
    const tag = FEE_CHARGE_KIND_TAG[kind];
    if (tag === undefined) {
      fail(
        INVALID_FEE_PAYMENT,
        `feePayment.chargeLimits[${index}].kind must be nexus or pipelineGas`,
      );
    }
    if (tag <= previousTag) {
      fail(
        INVALID_FEE_PAYMENT,
        "feePayment.chargeLimits must be unique and ordered nexus before pipelineGas",
      );
    }
    previousTag = tag;
    const assetDefinitionId = exactString(
      entry.assetDefinitionId,
      `feePayment.chargeLimits[${index}].assetDefinitionId`,
      { maxBytes: 128 },
    );
    assetDefinitionArchive(assetDefinitionId);
    const maxAmount = normalizeQuantity(entry.maxAmount);
    return { kind, tag, assetDefinitionId, maxAmount };
  });
}

function normalizeFeePayment(value, authority) {
  value = snapshotAllowedFields(value, FEE_PAYMENT_FIELDS, "feePayment");
  const payer = exactString(value.payer, "feePayment.payer", { maxBytes: 16 });
  if (payer !== "authority" && payer !== "sponsor") {
    fail(INVALID_FEE_PAYMENT, "feePayment.payer must be authority or sponsor");
  }
  const chargeLimits = normalizeFeeChargeLimits(value.chargeLimits);
  const gasLimit = normalizeOptionalUnsigned(
    value.gasLimit,
    UINT64_MAX,
    "feePayment.gasLimit",
    { nonZero: true },
  );
  if (payer === "authority") {
    if (value.programId !== undefined || value.programRevision !== undefined) {
      fail(
        INVALID_FEE_PAYMENT,
        "authority feePayment must not include programId or programRevision",
      );
    }
    return { payer, tag: 0, chargeLimits, gasLimit };
  }
  if (value.programId === undefined || value.programRevision === undefined) {
    fail(
      INVALID_FEE_PAYMENT,
      "sponsor feePayment requires programId and programRevision",
    );
  }
  const programRevision = normalizeUnsigned(
    value.programRevision,
    UINT64_MAX,
    "feePayment.programRevision",
  );
  if (programRevision === 0n) {
    fail(INVALID_FEE_PAYMENT, "feePayment.programRevision must be non-zero");
  }
  return {
    payer,
    tag: 1,
    program: normalizeFeeProgramId(value.programId, authority, "feePayment.programId"),
    programRevision,
    chargeLimits,
    gasLimit,
  };
}

function rejectLegacyFeeMetadata(metadata) {
  for (const key of LEGACY_FEE_METADATA_KEYS) {
    if (Object.prototype.hasOwnProperty.call(metadata, key)) {
      fail(
        INVALID_FEE_PAYMENT,
        `metadata.${key} is retired; use the signature-bound feePayment field`,
      );
    }
  }
}

function normalizeMetadata(input) {
  let value = input;
  let suppliedJson = null;
  if (value === undefined || value === null) {
    return Object.create(null);
  }
  if (typeof value === JS_TYPE_STRING) {
    if (
      value.length > MAX_METADATA_JSON_BYTES ||
      Buffer.byteLength(value, UTF8_ENCODING) > MAX_METADATA_JSON_BYTES
    ) {
      fail(BOUNDS_EXCEEDED, "metadata JSON exceeds 65536 UTF-8 bytes");
    }
    suppliedJson = value;
    try {
      value = JSON.parse(value);
    } catch (error) {
      fail(INVALID_METADATA, `metadata is not valid JSON: ${error.message}`);
    }
  }
  assertPlainDataObject(value, "metadata");
  const state = { nodes: 0, stack: new Set() };
  const normalized = normalizeMetadataValue(value, "metadata", 0, state);
  if (Object.keys(normalized).length > MAX_METADATA_ENTRIES) {
    fail(BOUNDS_EXCEEDED, `metadata exceeds ${MAX_METADATA_ENTRIES} top-level entries`);
  }
  const json = canonicalJsonStringify(normalized);
  if (suppliedJson !== null && suppliedJson !== json) {
    fail(
      INVALID_METADATA,
      "metadata JSON strings must already use the exact canonical encoding",
    );
  }
  if (Buffer.byteLength(json, UTF8_ENCODING) > MAX_METADATA_JSON_BYTES) {
    fail(BOUNDS_EXCEEDED, "canonical metadata JSON exceeds 65536 UTF-8 bytes");
  }
  return normalized;
}

function compareUtf8Strings(left, right) {
  return Buffer.compare(Buffer.from(left, UTF8_ENCODING), Buffer.from(right, UTF8_ENCODING));
}

function normalizeMetadataValue(value, context, depth, state) {
  state.nodes += 1;
  if (state.nodes > MAX_METADATA_NODES) {
    fail(BOUNDS_EXCEEDED, `metadata exceeds ${MAX_METADATA_NODES} values`);
  }
  if (depth > MAX_METADATA_DEPTH) {
    fail(BOUNDS_EXCEEDED, `metadata nesting exceeds ${MAX_METADATA_DEPTH}`);
  }
  if (value === null || typeof value === "boolean") {
    return value;
  }
  if (typeof value === JS_TYPE_STRING) {
    if (
      value.length > MAX_METADATA_JSON_BYTES ||
      Buffer.byteLength(value, UTF8_ENCODING) > MAX_METADATA_JSON_BYTES
    ) {
      fail(
        BOUNDS_EXCEEDED,
        `${context} string exceeds ${MAX_METADATA_JSON_BYTES} UTF-8 bytes`,
      );
    }
    if (!isWellFormedUnicode(value)) {
      fail(INVALID_METADATA, `${context} must contain only Unicode scalar values`);
    }
    return value;
  }
  if (typeof value === JS_TYPE_NUMBER) {
    if (!Number.isSafeInteger(value)) {
      fail(
        INVALID_METADATA,
        `${context} numbers must be safe integers; encode decimals as strings`,
      );
    }
    return value;
  }
  if (typeof value !== JS_TYPE_OBJECT || value instanceof Date) {
    fail(INVALID_METADATA, `${context} contains unsupported ${typeof value}`);
  }
  if (state.stack.has(value)) {
    fail(INVALID_METADATA, `${context} contains a cycle`);
  }
  state.stack.add(value);
  try {
    if (Array.isArray(value)) {
      return normalizeMetadataArray(value, context, depth, state);
    }
    assertPlainDataObject(value, context);
    const keys = Object.keys(value);
    if (keys.length > MAX_METADATA_NODES - state.nodes) {
      fail(BOUNDS_EXCEEDED, `metadata exceeds ${MAX_METADATA_NODES} values`);
    }
    for (let index = 0; index < keys.length; index += 1) {
      validateMetadataKey(keys[index], `${context} key ${index}`);
    }
    const output = Object.create(null);
    for (const key of keys.sort(compareUtf8Strings)) {
      const descriptor = Object.getOwnPropertyDescriptor(value, key);
      if (!descriptor || !Object.prototype.hasOwnProperty.call(descriptor, "value")) {
        fail(INVALID_METADATA, `${context} changed during normalization`);
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
    fail(INVALID_METADATA, `${context} arrays must use Array.prototype`);
  }
  const lengthDescriptor = Object.getOwnPropertyDescriptor(value, "length");
  const length = lengthDescriptor?.value;
  if (
    !lengthDescriptor ||
    !Object.prototype.hasOwnProperty.call(lengthDescriptor, "value") ||
    !Number.isSafeInteger(length) ||
    length < 0
  ) {
    fail(INVALID_METADATA, `${context} has an invalid array length`);
  }
  if (length > MAX_METADATA_NODES - state.nodes) {
    fail(BOUNDS_EXCEEDED, `metadata exceeds ${MAX_METADATA_NODES} values`);
  }
  const ownKeys = Reflect.ownKeys(value);
  if (ownKeys.length !== length + 1) {
    fail(
      INVALID_METADATA,
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
        INVALID_METADATA,
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
    Buffer.byteLength(key, UTF8_ENCODING) > MAX_METADATA_KEY_BYTES
  ) {
    fail(INVALID_METADATA, `${context} is not a canonical metadata Name`);
  }
}

function canonicalJsonStringify(value) {
  if (Array.isArray(value)) {
    return `[${value.map(canonicalJsonStringify).join(",")}]`;
  }
  if (value && typeof value === JS_TYPE_OBJECT) {
    return `{${Object.keys(value)
      .sort(compareUtf8Strings)
      .map((key) => `${rustPlainJsonString(key)}:${canonicalJsonStringify(value[key])}`)
      .join(",")}}`;
  }
  if (typeof value === JS_TYPE_STRING) {
    return rustPlainJsonString(value);
  }
  if (value === null) return "null";
  if (typeof value === "boolean") return value ? "true" : "false";
  if (typeof value === JS_TYPE_NUMBER && Number.isSafeInteger(value)) {
    return Object.is(value, -0) ? "0" : String(value);
  }
  fail(INVALID_METADATA, "metadata contains a non-canonical JSON value");
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
      case "\b":
      case "\f":
      case "\n":
      case "\r":
      case "\t":
        output += `\\${"bfnrt"["\b\f\n\r\t".indexOf(character)]}`;
        break;
      default:
        if (codePoint < 0x20) {
          output += `\\u00${codePoint.toString(16).padStart(2, "0")}`;
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
  return field(Buffer.from(value, UTF8_ENCODING));
}

function networkTransactionDomainArchive(value) {
  return Buffer.concat([u32(0), field(Buffer.from(value))]);
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
      fail(INVALID_ASSET, `${context} contains a non-Base58 character`);
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
    fail(INVALID_ASSET, "source asset definition is not a canonical v1 address");
  }
  const expectedChecksum = Buffer.from(blake3(bytes.subarray(0, 17))).subarray(0, 4);
  if (!bytes.subarray(17).equals(expectedChecksum)) {
    fail(INVALID_ASSET, "source asset definition checksum is invalid");
  }
  return Buffer.concat(Array.from(bytes.subarray(1, 17), (byte) => field(Buffer.of(byte))));
}

function assetScopeArchive(scope) {
  if (scope === null) {
    return u32(0);
  }
  const match = /^dataspace:(0|[1-9]\d*)$/u.exec(scope);
  if (!match) {
    fail(INVALID_ASSET, "asset scope must use canonical dataspace:<u64> syntax");
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

function feeChargeLimitsArchive(chargeLimits) {
  return vector(
    chargeLimits.map((limit) =>
      struct([
        u32(limit.tag),
        assetDefinitionArchive(limit.assetDefinitionId),
        numericArchive(limit.maxAmount),
      ]),
    ),
  );
}

function feePaymentArchive(feePayment) {
  const chargeLimits = feeChargeLimitsArchive(feePayment.chargeLimits);
  const gasLimit = option(
    feePayment.gasLimit === null ? null : u64(feePayment.gasLimit),
  );
  if (feePayment.tag === 0) {
    return Buffer.concat([
      u32(0),
      field(struct([chargeLimits, gasLimit])),
    ]);
  }
  const programId = struct([
    accountArchive(feePayment.program.sponsor),
    stringValue(feePayment.program.name),
  ]);
  return Buffer.concat([
    u32(1),
    field(
      struct([
        programId,
        u64(feePayment.programRevision),
        chargeLimits,
        gasLimit,
      ]),
    ),
  ]);
}

function frameTransferPayload(payload) {
  return Buffer.concat([
    Buffer.from("NRT0", "ascii"),
    Buffer.of(0, 0),
    TRANSFER_SCHEMA_HASH,
    Buffer.of(0),
    u64(payload.length),
    u64(crc64Xz(payload)),
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

function normalizeTransactionInputAuthority(input) {
  const networkId = exactNetworkId(input.networkId);
  const requestedDiscriminant =
    input.networkPrefix !== undefined && input.chainDiscriminant !== undefined
      ? fail(INVALID_INPUT, NETWORK_SELECTOR_CONFLICT_MESSAGE)
      : input.networkPrefix ?? input.chainDiscriminant;
  const authority = accountInfo(
    input.authority,
    "authority",
    requestedDiscriminant === undefined
      ? undefined
      : normalizeNetworkPrefix(requestedDiscriminant, "networkPrefix"),
  );
  return { networkId, authority };
}

function normalizeTransactionInputTail(
  input,
  authority,
  validateFeePayment,
) {
  const metadata = normalizeMetadata(input.metadata);
  rejectLegacyFeeMetadata(metadata);
  const feePayment = normalizeFeePayment(input.feePayment, authority);
  validateFeePayment?.(feePayment);
  const creationTimeMs = normalizeUnsigned(
    input.creationTimeMs ?? Date.now(),
    UINT64_MAX,
    "creationTimeMs",
  );
  const ttlMs = normalizeOptionalUnsigned(
    input.ttlMs ?? DEFAULT_TRANSACTION_TTL_MS,
    UINT64_MAX,
    "ttlMs",
    { nonZero: true },
  );
  const nonce = normalizeOptionalUnsigned(input.nonce, UINT32_MAX, "nonce", {
    nonZero: true,
  });
  return { feePayment, metadata, creationTimeMs, ttlMs, nonce };
}

function encodeCanonicalInstructionBox(instruction, context) {
  try {
    return Buffer.from(noritoEncodeInstructionBoxArchive(instruction));
  } catch (error) {
    fail(
      UNSUPPORTED_INSTRUCTION,
      `${context} cannot be encoded canonically: ${error.message}`,
    );
  }
}

function normalizeTransferInput(input) {
  input = snapshotAllowedFields(input, TRANSFER_INPUT_FIELDS, "transfer input");
  const { networkId, authority } = normalizeTransactionInputAuthority(input);
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
      INVALID_ASSET,
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
      INVALID_ASSET,
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
      AUTHORITY_MISMATCH,
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
  const tail = normalizeTransactionInputTail(input, authority);
  return {
    networkId,
    authority,
    source,
    quantity,
    destination,
    ...tail,
  };
}

function encodeTransactionPayload(normalized, executable) {
  const payload = struct([
    networkTransactionDomainArchive(normalized.networkId),
    accountArchive(normalized.authority),
    u64(normalized.creationTimeMs),
    executable,
    option(normalized.ttlMs === null ? null : u64(normalized.ttlMs)),
    option(normalized.nonce === null ? null : u32(normalized.nonce)),
    feePaymentArchive(normalized.feePayment),
    u32(TRANSACTION_ADMISSION_QUEUE_PLAN_SYNCED_TAG),
    metadataArchive(normalized.metadata),
    Buffer.of(0),
  ]);
  if (payload.length === 0 || payload.length > MAX_PAYLOAD_BYTES) {
    fail(BOUNDS_EXCEEDED, `transaction payload exceeds ${MAX_PAYLOAD_BYTES} bytes`);
  }
  return payload;
}

function encodeTransferPayload(normalized) {
  const instruction = transferInstructionArchive(
    normalized.source,
    normalized.quantity,
    normalized.destination,
  );
  return encodeTransactionPayload(
    normalized,
    Buffer.concat([u32(0), field(vector([instruction]))]),
  );
}

function normalizeInstructionTransactionInput(input) {
  input = snapshotAllowedFields(
    input,
    INSTRUCTION_INPUT_FIELDS,
    "instruction transaction input",
  );
  const { networkId, authority } = normalizeTransactionInputAuthority(input);
  if (!Array.isArray(input.instructions)) {
    fail(INVALID_INPUT, "instructions must be an array");
  }
  if (
    input.instructions.length === 0 ||
    input.instructions.length > MAX_BROWSER_INSTRUCTIONS
  ) {
    fail(
      BOUNDS_EXCEEDED,
      `instructions must contain 1..=${MAX_BROWSER_INSTRUCTIONS} items`,
    );
  }
  const instructions = input.instructions.map((instruction, index) =>
    encodeCanonicalInstructionBox(instruction, `instructions[${index}]`),
  );
  const tail = normalizeTransactionInputTail(input, authority);
  return {
    networkId,
    authority,
    instructions,
    ...tail,
  };
}

function encodeInstructionTransactionPayload(normalized) {
  return encodeTransactionPayload(
    normalized,
    Buffer.concat([u32(0), field(vector(normalized.instructions))]),
  );
}

function normalizeExecutableBatchTransactionInput(input) {
  input = snapshotAllowedFields(
    input,
    EXECUTABLE_BATCH_INPUT_FIELDS,
    "executable batch input",
  );
  const { networkId, authority } = normalizeTransactionInputAuthority(input);
  if (!Array.isArray(input.entries)) {
    fail(INVALID_INPUT, "entries must be an array");
  }
  if (
    input.entries.length === 0 ||
    input.entries.length > MAX_BROWSER_INSTRUCTIONS
  ) {
    fail(
      BOUNDS_EXCEEDED,
      `entries must contain 1..=${MAX_BROWSER_INSTRUCTIONS} items`,
    );
  }
  let containsContractCall = false;
  const entries = input.entries.map((source, index) => {
    source = assertPlainDataObject(source, `entries[${index}]`);
    if (source.kind === "instruction") {
      const entry = snapshotAllowedFields(
        source,
        BATCH_INSTRUCTION_ENTRY_FIELDS,
        `entries[${index}]`,
      );
      const instruction = encodeCanonicalInstructionBox(
        entry.instruction,
        `entries[${index}].instruction`,
      );
      return { kind: "instruction", instruction };
    }
    if (source.kind !== "contractCall") {
      fail(
        INVALID_INPUT,
        `entries[${index}].kind must be instruction or contractCall`,
      );
    }
    const entry = snapshotAllowedFields(
      source,
      BATCH_CONTRACT_CALL_ENTRY_FIELDS,
      `entries[${index}]`,
    );
    containsContractCall = true;
    const contractAddressLiteral = exactString(
      entry.contractAddress,
      `entries[${index}].contractAddress`,
      { maxBytes: 256 },
    );
    let contractAddress;
    try {
      contractAddress = parseCanonicalContractAddress(
        contractAddressLiteral,
        `entries[${index}].contractAddress`,
      ).literal;
    } catch (error) {
      fail(
        INVALID_INPUT,
        error instanceof Error
          ? error.message
          : `entries[${index}].contractAddress is invalid`,
      );
    }
    const expectedCodeHash = bytes(
      entry.expectedCodeHash,
      `entries[${index}].expectedCodeHash`,
      { hex: true, maxBytes: 32 },
    );
    if (expectedCodeHash.length !== 32) {
      fail(
        INVALID_BYTES,
        `entries[${index}].expectedCodeHash must be exactly 32 bytes`,
      );
    }
    if ((expectedCodeHash[31] & 1) === 0) {
      fail(
        INVALID_BYTES,
        `entries[${index}].expectedCodeHash must carry the canonical Iroha hash marker bit`,
      );
    }
    const entrypoint = exactString(
      entry.entrypoint,
      `entries[${index}].entrypoint`,
      { maxBytes: MAX_CONTRACT_ENTRYPOINT_BYTES },
    );
    const argumentBytes =
      entry.arguments === undefined || entry.arguments === null
        ? null
        : bytes(entry.arguments, `entries[${index}].arguments`, {
            maxBytes: MAX_CONTRACT_ARGUMENT_RECORD_BYTES,
          });
    return {
      kind: "contractCall",
      contractAddress,
      expectedCodeHash,
      entrypoint,
      arguments: argumentBytes,
    };
  });
  const tail = normalizeTransactionInputTail(input, authority, (feePayment) => {
    if (containsContractCall && feePayment.gasLimit === null) {
      fail(
        INVALID_FEE_PAYMENT,
        "feePayment.gasLimit is required when entries contain a contract call",
      );
    }
  });
  return {
    networkId,
    authority,
    entries,
    ...tail,
  };
}

function contractInvocationArchive(invocation) {
  const argumentsArchive =
    invocation.arguments === null
      ? null
      : Buffer.concat([u64(invocation.arguments.length), invocation.arguments]);
  return struct([
    stringValue(invocation.contractAddress),
    invocation.expectedCodeHash,
    stringValue(invocation.entrypoint),
    option(argumentsArchive),
  ]);
}

function executableBatchEntryArchive(entry) {
  if (entry.kind === "instruction") {
    return Buffer.concat([u32(0), field(entry.instruction)]);
  }
  return Buffer.concat([u32(1), field(contractInvocationArchive(entry))]);
}

function encodeExecutableBatchTransactionPayload(normalized) {
  return encodeTransactionPayload(
    normalized,
    Buffer.concat([
      u32(4),
      field(vector(normalized.entries.map(executableBatchEntryArchive))),
    ]),
  );
}

function bytes(value, context, { hex = false, maxBytes } = {}) {
  if (Buffer.isBuffer(value)) {
    if (maxBytes !== undefined && value.length > maxBytes) {
      fail(BOUNDS_EXCEEDED, `${context} exceeds ${maxBytes} bytes`);
    }
    return Buffer.from(value);
  }
  if (ArrayBuffer.isView(value)) {
    if (maxBytes !== undefined && value.byteLength > maxBytes) {
      fail(BOUNDS_EXCEEDED, `${context} exceeds ${maxBytes} bytes`);
    }
    try {
      return Buffer.from(
        new Uint8Array(value.buffer, value.byteOffset, value.byteLength),
      );
    } catch {
      fail(INVALID_BYTES, `${context} references detached or invalid bytes`);
    }
  }
  if (value instanceof ArrayBuffer) {
    if (maxBytes !== undefined && value.byteLength > maxBytes) {
      fail(BOUNDS_EXCEEDED, `${context} exceeds ${maxBytes} bytes`);
    }
    try {
      return Buffer.from(new Uint8Array(value));
    } catch {
      fail(INVALID_BYTES, `${context} references detached or invalid bytes`);
    }
  }
  if (hex && typeof value === JS_TYPE_STRING) {
    const maximum = maxBytes ?? MAX_SIGNED_TRANSACTION_BYTES;
    if (value.length > maximum * 2 + 2) {
      fail(BOUNDS_EXCEEDED, `${context} hexadecimal input is too large`);
    }
    const literal = value.startsWith("0x") ? value.slice(2) : value;
    if (/^(?:[0-9a-fA-F]{2})+$/u.test(literal)) {
      return Buffer.from(literal, HEX_ENCODING);
    }
  }
  fail(INVALID_BYTES, `${context} must be bytes${hex ? " or exact hexadecimal" : ""}`);
}

function irohaHash(value) {
  const digest = Buffer.from(blake2b256(value));
  digest[digest.length - 1] |= 1;
  return digest;
}

function exactHashHex(value, context) {
  if (typeof value !== JS_TYPE_STRING || !/^[0-9a-f]{63}[13579bdf]$/u.test(value)) {
    fail("invalid_hash", `${context} must be an exact canonical lowercase 32-byte Iroha hash`);
  }
  return value;
}

function validateStringArchive(payload, context, { maxBytes } = {}) {
  const reader = new Reader(payload, context);
  const value = reader.readField(UTF8_ENCODING);
  reader.assertEof();
  if (maxBytes !== undefined && value.length > maxBytes) {
    fail(BOUNDS_EXCEEDED, `${context} exceeds ${maxBytes} UTF-8 bytes`);
  }
  let decoded;
  try {
    decoded = new TextDecoder("utf-8", { fatal: true }).decode(value);
  } catch {
    fail(MALFORMED_PAYLOAD, `${context} contains invalid UTF-8`);
  }
  if (!stringValue(decoded).equals(payload)) {
    fail(MALFORMED_PAYLOAD, `${context} is not canonical`);
  }
  return decoded;
}

function validateNetworkTransactionDomainArchive(payload, context) {
  const reader = new Reader(payload, context);
  const variant = reader.readU32("variant");
  if (variant !== 0) {
    fail(
      UNSUPPORTED_PAYLOAD,
      `${context} must be TransactionDomain::Network`,
    );
  }
  const networkId = reader.readField("networkId");
  if (networkId.length !== 32 || (networkId[31] & 1) === 0) {
    fail(
      MALFORMED_PAYLOAD,
      `${context}.networkId must contain exactly 32 marked Iroha hash bytes`,
    );
  }
  reader.assertEof();
  if (!networkTransactionDomainArchive(networkId).equals(payload)) {
    fail(MALFORMED_PAYLOAD, `${context} is not canonical`);
  }
  return networkId;
}

function validateConstVecBytes(payload, context, expectedLength) {
  const reader = new Reader(payload, context);
  const count = reader.readU64("count");
  if (count !== BigInt(expectedLength)) {
    fail(MALFORMED_PAYLOAD, `${context} must contain ${expectedLength} bytes`);
  }
  const output = Buffer.alloc(expectedLength);
  for (let index = 0; index < expectedLength; index += 1) {
    const item = reader.readField(`item[${index}]`);
    if (item.length !== 1) {
      fail(MALFORMED_PAYLOAD, `${context}.item[${index}] must contain one byte`);
    }
    output[index] = item[0];
  }
  reader.assertEof();
  return output;
}

function validateAccountArchive(payload, context) {
  const reader = new Reader(payload, context);
  if (reader.readU32("controllerTag") !== 0) {
    fail(UNSUPPORTED_AUTHORITY, `${context} must use a single-key controller`);
  }
  const controller = validateConstVecBytes(
    reader.readField("controller"),
    `${context}.controller`,
    33,
  );
  reader.assertEof();
  if (controller[0] !== 0) {
    fail(UNSUPPORTED_ALGORITHM, `${context} must use Ed25519 algorithm tag 0`);
  }
  const publicKey = controller.subarray(1);
  try {
    AccountAddress.fromAccount({ algorithm: ED25519_ALGORITHM, publicKey });
  } catch (error) {
    fail(INVALID_PUBLIC_KEY, `${context} contains an invalid Ed25519 key: ${error.message}`);
  }
  return Buffer.from(publicKey);
}

function validateFixedByteArchive(payload, length, context) {
  const reader = new Reader(payload, context);
  const output = Buffer.alloc(length);
  for (let index = 0; index < length; index += 1) {
    const item = reader.readField(`item[${index}]`);
    if (item.length !== 1) {
      fail(MALFORMED_PAYLOAD, `${context}.item[${index}] must contain one byte`);
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
    fail(MALFORMED_PAYLOAD, `${context} uses unsupported tag ${tag}`);
  }
  const outer = new Reader(reader.readField("dataspace"), `${context}.dataspace`);
  const inner = outer.readField("value");
  outer.assertEof();
  if (inner.length !== 8) {
    fail(MALFORMED_PAYLOAD, `${context}.dataspace must contain a u64`);
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
      BOUNDS_EXCEEDED,
      `${context}.mantissa exceeds ${MAX_NUMERIC_BITS} bits`,
    );
  }
  const mantissaBytes = mantissaReader.readBytes(byteLength, "bytes");
  mantissaReader.assertEof();
  const mantissa = twosBytesToBigint(mantissaBytes);
  if (mantissa <= 0n || !bigintToTwosBytes(mantissa).equals(mantissaBytes)) {
    fail(MALFORMED_PAYLOAD, `${context}.mantissa is not canonical and positive`);
  }
  if (mantissa.toString(2).length > MAX_NUMERIC_BITS) {
    fail(BOUNDS_EXCEEDED, `${context}.mantissa exceeds ${MAX_NUMERIC_BITS} bits`);
  }
  if (scalePayload.length !== 4 || scalePayload.readUInt32LE(0) > MAX_NUMERIC_SCALE) {
    fail(MALFORMED_PAYLOAD, `${context}.scale is outside the supported range`);
  }
  const scale = scalePayload.readUInt32LE(0);
  if (scale > 0 && mantissa % 10n === 0n) {
    fail(
      MALFORMED_PAYLOAD,
      `${context} has a non-canonical fractional trailing zero`,
    );
  }
}

function validateFeeChargeLimitsArchive(payload, context) {
  const reader = new Reader(payload, context);
  const count = reader.readU64("count");
  if (count > BigInt(Object.keys(FEE_CHARGE_KIND_TAG).length)) {
    fail(MALFORMED_PAYLOAD, `${context} contains too many charge limits`);
  }
  let previousTag = -1;
  for (let index = 0; index < Number(count); index += 1) {
    const entry = new Reader(
      reader.readField(`item[${index}]`),
      `${context}[${index}]`,
    );
    const kind = entry.readField("kind");
    if (kind.length !== 4) {
      fail(MALFORMED_PAYLOAD, `${context}[${index}].kind must be a u32 tag`);
    }
    const tag = kind.readUInt32LE(0);
    if (tag > 1 || tag <= previousTag) {
      fail(
        MALFORMED_PAYLOAD,
        `${context} must be uniquely ordered by charge kind`,
      );
    }
    previousTag = tag;
    validateFixedByteArchive(
      entry.readField("assetDefinitionId"),
      16,
      `${context}[${index}].assetDefinitionId`,
    );
    validateNumericArchive(
      entry.readField("maxAmount"),
      `${context}[${index}].maxAmount`,
    );
    entry.assertEof();
  }
  reader.assertEof();
}

function validateFeeProgramIdArchive(payload, context) {
  const reader = new Reader(payload, context);
  validateAccountArchive(reader.readField("sponsor"), `${context}.sponsor`);
  const name = validateStringArchive(
    reader.readField("name"),
    `${context}.name`,
    { maxBytes: MAX_METADATA_KEY_BYTES },
  );
  normalizeFeeProgramName(name, `${context}.name`);
  reader.assertEof();
}

function validateFeePaymentArchive(payload, context) {
  const reader = new Reader(payload, context);
  const tag = reader.readU32("payer");
  if (tag > 1) {
    fail(MALFORMED_PAYLOAD, `${context} uses unsupported payer tag ${tag}`);
  }
  const body = new Reader(reader.readField("value"), `${context}.value`);
  reader.assertEof();
  if (tag === 1) {
    validateFeeProgramIdArchive(
      body.readField("programId"),
      `${context}.programId`,
    );
    const revision = body.readField("programRevision");
    if (revision.length !== 8 || revision.readBigUInt64LE(0) === 0n) {
      fail(MALFORMED_PAYLOAD, `${context}.programRevision must be non-zero u64`);
    }
  }
  validateFeeChargeLimitsArchive(
    body.readField("chargeLimits"),
    `${context}.chargeLimits`,
  );
  const gasLimit = validateOption(body.readField("gasLimit"), 8, `${context}.gasLimit`, {
    nonZero: true,
  });
  body.assertEof();
  return { gasLimit };
}

function validateTransactionAdmissionIntentArchive(payload, expectedTag, context) {
  const reader = new Reader(payload, context);
  const tag = reader.readU32("intent");
  reader.assertEof();
  if (tag !== expectedTag) {
    const expected = expectedTag === TRANSACTION_ADMISSION_ORDINARY_TAG
      ? "Ordinary"
      : "QueuePlanSynced";
    fail(
      UNSUPPORTED_PAYLOAD,
      `${context} must be TransactionAdmissionIntent::${expected}`,
    );
  }
}

function validateFrame(frame, context) {
  if (frame.length < 40 || frame.subarray(0, 4).toString("ascii") !== "NRT0") {
    fail(MALFORMED_PAYLOAD, `${context} is not an NRT0 frame`);
  }
  const reader = new Reader(frame, context, false);
  reader.readBytes(4, "magic");
  if (reader.readU8("major") !== 0 || reader.readU8("minor") !== 0) {
    fail(MALFORMED_PAYLOAD, `${context} uses an unsupported NRT0 version`);
  }
  if (!reader.readBytes(16, "schemaHash").equals(TRANSFER_SCHEMA_HASH)) {
    fail(MALFORMED_PAYLOAD, `${context} has the wrong transfer schema hash`);
  }
  if (reader.readU8("reserved") !== 0) {
    fail(MALFORMED_PAYLOAD, `${context} has a non-zero reserved byte`);
  }
  const length = reader.readU64("payloadLength");
  const expectedCrc = reader.readU64("crc64");
  if (reader.readU8("flags") !== COMPACT_LEN_FLAG) {
    fail(MALFORMED_PAYLOAD, `${context} must advertise compact-length layout only`);
  }
  if (length > BigInt(Number.MAX_SAFE_INTEGER)) {
    fail(MALFORMED_PAYLOAD, `${context} payload length is too large`);
  }
  const payload = reader.readBytes(Number(length), "payload");
  reader.assertEof();
  if (crc64Xz(payload) !== expectedCrc) {
    fail(MALFORMED_PAYLOAD, `${context} CRC64 does not match`);
  }
  return payload;
}

function validateTransferExecutable(payload, context) {
  const executable = new Reader(payload, context);
  if (executable.readU32("variant") !== 0) {
    fail(UNSUPPORTED_EXECUTABLE, `${context} must use Executable::Instructions`);
  }
  const instructions = new Reader(executable.readField("instructions"), `${context}.instructions`);
  if (instructions.readU64("count") !== 1n) {
    fail(UNSUPPORTED_EXECUTABLE, `${context} must contain exactly one instruction`);
  }
  const instruction = new Reader(
    instructions.readField("item[0]"),
    `${context}.instructions[0]`,
  );
  const wireId = validateStringArchive(
    instruction.readField("wireId"),
    `${context}.instructions[0].wireId`,
    { maxBytes: Buffer.byteLength("iroha.transfer", UTF8_ENCODING) },
  );
  if (wireId !== "iroha.transfer") {
    fail(UNSUPPORTED_INSTRUCTION, `${context} must contain iroha.transfer`);
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
    fail(UNSUPPORTED_INSTRUCTION, `${context} must contain Transfer::Asset`);
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

function validateSmartContractDeploymentExecutable(payload, context) {
  const executable = new Reader(payload, context);
  if (executable.readU32("variant") !== 0) {
    fail(UNSUPPORTED_EXECUTABLE, `${context} must use Executable::Instructions`);
  }
  const instructions = new Reader(
    executable.readField("instructions"),
    `${context}.instructions`,
  );
  const count = instructions.readU64("count");
  if (count === 0n || count > BigInt(MAX_BROWSER_INSTRUCTIONS)) {
    fail(
      BOUNDS_EXCEEDED,
      `${context} must contain 1..=${MAX_BROWSER_INSTRUCTIONS} instructions`,
    );
  }
  for (let index = 0; index < Number(count); index += 1) {
    const instruction = new Reader(
      instructions.readField(`item[${index}]`),
      `${context}.instructions[${index}]`,
    );
    const wireId = validateStringArchive(
      instruction.readField("wireId"),
      `${context}.instructions[${index}].wireId`,
      { maxBytes: 256 },
    );
    if (!SMART_CONTRACT_DEPLOYMENT_WIRE_IDS.has(wireId)) {
      fail(
        UNSUPPORTED_INSTRUCTION,
        `${context}.instructions[${index}] uses unsupported deployment instruction ${wireId}`,
      );
    }
    const frameContainer = new Reader(
      instruction.readField("frameContainer"),
      `${context}.instructions[${index}].frameContainer`,
      false,
    );
    const frame = frameContainer.readField("frame");
    frameContainer.assertEof();
    instruction.assertEof();
    try {
      validateNoritoFrame(frame, {
        context: `${context}.instructions[${index}].frame`,
        expectedTypeName: wireId,
        requireNonEmptyPayload: true,
      });
    } catch (error) {
      fail(
        MALFORMED_INSTRUCTION,
        `${context}.instructions[${index}] has an invalid inner frame: ${error.message}`,
      );
    }
  }
  instructions.assertEof();
  executable.assertEof();
}

function validateCanonicalInstructionBox(payload, context) {
  const instruction = new Reader(payload, context);
  const wireId = validateStringArchive(
    instruction.readField("wireId"),
    `${context}.wireId`,
    { maxBytes: 256 },
  );
  const frameContainer = new Reader(
    instruction.readField("frameContainer"),
    `${context}.frameContainer`,
    false,
  );
  const frame = frameContainer.readField("frame");
  frameContainer.assertEof();
  instruction.assertEof();
  try {
    validateNoritoFrame(frame, {
      context: `${context}.frame`,
      expectedTypeName: wireId,
      requireNonEmptyPayload: true,
    });
  } catch (error) {
    fail(
      MALFORMED_INSTRUCTION,
      `${context} has an invalid inner frame: ${error.message}`,
    );
  }
}

function validateContractArgumentOption(payload, context) {
  const optionReader = new Reader(payload, context);
  const tag = optionReader.readU8("tag");
  if (tag === 0) {
    optionReader.assertEof();
    return;
  }
  if (tag !== 1) {
    fail(MALFORMED_PAYLOAD, `${context} uses invalid option tag ${tag}`);
  }
  const record = new Reader(optionReader.readField("value"), `${context}.value`);
  optionReader.assertEof();
  const length = record.readU64("length");
  if (length > BigInt(MAX_CONTRACT_ARGUMENT_RECORD_BYTES)) {
    fail(
      BOUNDS_EXCEEDED,
      `${context} exceeds ${MAX_CONTRACT_ARGUMENT_RECORD_BYTES} bytes`,
    );
  }
  record.readBytes(Number(length), "bytes");
  record.assertEof();
}

function validateContractInvocationArchive(payload, context) {
  const invocation = new Reader(payload, context);
  const contractAddress = validateStringArchive(
    invocation.readField("contractAddress"),
    `${context}.contractAddress`,
    { maxBytes: 256 },
  );
  exactString(contractAddress, `${context}.contractAddress`, { maxBytes: 256 });
  try {
    parseCanonicalContractAddress(contractAddress, `${context}.contractAddress`);
  } catch (error) {
    fail(
      MALFORMED_PAYLOAD,
      error instanceof Error
        ? error.message
        : `${context}.contractAddress is invalid`,
    );
  }
  const expectedCodeHash = invocation.readField("expectedCodeHash");
  if (expectedCodeHash.length !== 32 || (expectedCodeHash[31] & 1) === 0) {
    fail(
      MALFORMED_PAYLOAD,
      `${context}.expectedCodeHash must be a canonical 32-byte Iroha hash`,
    );
  }
  const entrypoint = validateStringArchive(
    invocation.readField("entrypoint"),
    `${context}.entrypoint`,
    { maxBytes: MAX_CONTRACT_ENTRYPOINT_BYTES },
  );
  exactString(entrypoint, `${context}.entrypoint`, {
    maxBytes: MAX_CONTRACT_ENTRYPOINT_BYTES,
  });
  validateContractArgumentOption(
    invocation.readField("arguments"),
    `${context}.arguments`,
  );
  invocation.assertEof();
}

function validateExecutableBatch(payload, context) {
  const executable = new Reader(payload, context);
  if (executable.readU32("variant") !== 4) {
    fail(UNSUPPORTED_EXECUTABLE, `${context} must use Executable::Batch`);
  }
  const entries = new Reader(
    executable.readField("entries"),
    `${context}.entries`,
  );
  const count = entries.readU64("count");
  if (count === 0n || count > BigInt(MAX_BROWSER_INSTRUCTIONS)) {
    fail(
      BOUNDS_EXCEEDED,
      `${context} must contain 1..=${MAX_BROWSER_INSTRUCTIONS} entries`,
    );
  }
  let containsContractCall = false;
  for (let index = 0; index < Number(count); index += 1) {
    const entry = new Reader(
      entries.readField(`item[${index}]`),
      `${context}.entries[${index}]`,
    );
    const tag = entry.readU32("variant");
    const value = entry.readField("value");
    entry.assertEof();
    if (tag === 0) {
      validateCanonicalInstructionBox(
        value,
        `${context}.entries[${index}].instruction`,
      );
    } else if (tag === 1) {
      containsContractCall = true;
      validateContractInvocationArchive(
        value,
        `${context}.entries[${index}].contractCall`,
      );
    } else {
      fail(
        UNSUPPORTED_EXECUTABLE,
        `${context}.entries[${index}] uses unsupported variant ${tag}`,
      );
    }
  }
  entries.assertEof();
  executable.assertEof();
  return { containsContractCall };
}

function validateOption(payload, expectedLength, context, { nonZero = false } = {}) {
  const reader = new Reader(payload, context);
  const tag = reader.readU8("tag");
  if (tag === 0) {
    reader.assertEof();
    return null;
  }
  if (tag !== 1) {
    fail(MALFORMED_PAYLOAD, `${context} uses invalid option tag ${tag}`);
  }
  const value = reader.readField("value");
  reader.assertEof();
  if (value.length !== expectedLength) {
    fail(MALFORMED_PAYLOAD, `${context} has the wrong integer width`);
  }
  const numeric = expectedLength === 8 ? value.readBigUInt64LE(0) : BigInt(value.readUInt32LE(0));
  if (nonZero && numeric === 0n) {
    fail(MALFORMED_PAYLOAD, `${context} must be non-zero when present`);
  }
  return numeric;
}

function validateRequiredTtlOption(payload, context) {
  if (validateOption(payload, 8, context, { nonZero: true }) === null) {
    fail(MALFORMED_PAYLOAD, `${context} is required`);
  }
}

function validateMetadataArchive(payload, context) {
  const reader = new Reader(payload, context);
  const count = reader.readU64("count");
  if (count > BigInt(MAX_METADATA_ENTRIES)) {
    fail(BOUNDS_EXCEEDED, `${context} exceeds ${MAX_METADATA_ENTRIES} entries`);
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
      fail(MALFORMED_PAYLOAD, `${context} keys must be unique and sorted`);
    }
    previousKey = key;
    const json = new Reader(entry.readField("json"), `${context}[${index}].json`);
    const jsonText = validateStringArchive(
      json.readField("value"),
      `${context}[${index}].json.value`,
      { maxBytes: MAX_METADATA_JSON_BYTES },
    );
    if (Buffer.byteLength(jsonText, UTF8_ENCODING) > MAX_METADATA_JSON_BYTES) {
      fail(
        BOUNDS_EXCEEDED,
        `${context}[${index}] JSON exceeds ${MAX_METADATA_JSON_BYTES} UTF-8 bytes`,
      );
    }
    json.assertEof();
    entry.assertEof();
    let parsed;
    try {
      parsed = JSON.parse(jsonText);
    } catch {
      fail(MALFORMED_PAYLOAD, `${context}[${index}] contains invalid JSON`);
    }
    const normalized = normalizeMetadataValue(
      parsed,
      `${context}.${key}`,
      1,
      state,
    );
    if (canonicalJsonStringify(normalized) !== jsonText) {
      fail(MALFORMED_PAYLOAD, `${context}[${index}] JSON is not canonical`);
    }
    normalizedMetadata[key] = normalized;
  }
  reader.assertEof();
  if (
    Buffer.byteLength(canonicalJsonStringify(normalizedMetadata), UTF8_ENCODING) >
    MAX_METADATA_JSON_BYTES
  ) {
    fail(
      BOUNDS_EXCEEDED,
      `${context} canonical JSON exceeds ${MAX_METADATA_JSON_BYTES} UTF-8 bytes`,
    );
  }
  return normalizedMetadata;
}

function validateTransactionPayload(
  payload,
  authorityLiteral,
  expectedNetworkId = null,
) {
  return validateTransactionPayloadEnvelope(
    payload,
    authorityLiteral,
    expectedNetworkId,
    (executable, authorityPublicKey) => {
      const sourceOwner = validateTransferExecutable(
        executable,
        "transaction payload.executable",
      );
      if (!sourceOwner.equals(authorityPublicKey)) {
        fail(
          AUTHORITY_MISMATCH,
          "source asset owner does not match transaction authority",
        );
      }
    },
  );
}

function validateInstructionTransactionPayload(
  payload,
  authorityLiteral,
  expectedNetworkId = null,
) {
  return validateTransactionPayloadEnvelope(
    payload,
    authorityLiteral,
    expectedNetworkId,
    (executable) => {
      if (executable.length < 4) {
        fail(MALFORMED_PAYLOAD, "transaction payload.executable is truncated");
      }
      if (executable.readUInt32LE(0) === 4) {
        return validateExecutableBatch(
          executable,
          "transaction payload.executable",
        );
      }
      validateSmartContractDeploymentExecutable(
        executable,
        "transaction payload.executable",
      );
      return { containsContractCall: false };
    },
    (batchValidation, feePayment) => {
      if (batchValidation.containsContractCall && feePayment.gasLimit === null) {
        fail(
          INVALID_FEE_PAYMENT,
          "transaction payload feePayment.gasLimit is required for contract calls",
        );
      }
    },
  );
}

function validateTransactionPayloadEnvelope(
  payload,
  authorityLiteral,
  expectedNetworkId,
  validateExecutable,
  validateFeePayment,
) {
  if (payload.length === 0 || payload.length > MAX_PAYLOAD_BYTES) {
    fail(BOUNDS_EXCEEDED, `payloadBytes must contain 1..=${MAX_PAYLOAD_BYTES} bytes`);
  }
  const assertedAuthority =
    authorityLiteral === null
      ? null
      : accountInfo(authorityLiteral, "signable.authority");
  const reader = new Reader(payload, "transaction payload");
  const networkId = validateNetworkTransactionDomainArchive(
    reader.readField("domain"),
    "transaction payload.domain",
  );
  if (expectedNetworkId !== null) {
    requireExpectedNetworkId(
      networkId,
      expectedNetworkId,
      "transaction payload NetworkId",
    );
  }
  const authorityArchive = reader.readField("authority");
  const authorityPublicKey = validateAccountArchive(
    authorityArchive,
    "transaction payload.authority",
  );
  if (
    assertedAuthority !== null &&
    !authorityArchive.equals(accountArchive(assertedAuthority))
  ) {
    fail(AUTHORITY_MISMATCH, SIGNABLE_AUTHORITY_PAYLOAD_MISMATCH_MESSAGE);
  }
  const creationTime = reader.readField("creationTimeMs");
  if (creationTime.length !== 8) {
    fail(MALFORMED_PAYLOAD, "transaction payload.creationTimeMs must be a u64");
  }
  const executableValidation = validateExecutable(
    reader.readField("executable"),
    authorityPublicKey,
  );
  validateRequiredTtlOption(
    reader.readField("ttlMs"),
    "transaction payload.ttlMs",
  );
  validateOption(reader.readField("nonce"), 4, "transaction payload.nonce", {
    nonZero: true,
  });
  const feePayment = validateFeePaymentArchive(
    reader.readField("feePayment"),
    "transaction payload.feePayment",
  );
  validateFeePayment?.(executableValidation, feePayment);
  validateTransactionAdmissionIntentArchive(
    reader.readField("admissionIntent"),
    TRANSACTION_ADMISSION_QUEUE_PLAN_SYNCED_TAG,
    "transaction payload.admissionIntent",
  );
  rejectLegacyFeeMetadata(
    validateMetadataArchive(
      reader.readField("metadata"),
      "transaction payload.metadata",
    ),
  );
  if (!reader.readField("attachments").equals(Buffer.of(0))) {
    fail(
      UNSUPPORTED_PAYLOAD,
      PROOF_ATTACHMENT_UNSUPPORTED_MESSAGE,
    );
  }
  reader.assertEof();
  return assertedAuthority ?? { publicKey: authorityPublicKey };
}

/**
 * Inspect the signature-bound fields shared by every canonical transaction
 * payload without interpreting its executable.
 *
 * This is internal Torii response-validation plumbing. It deliberately keeps
 * the fee archive opaque so callers can compare it byte-for-byte with the
 * exact response DTO encoding.
 *
 * @param {ArrayBufferView | ArrayBuffer | Buffer} payloadBytes
 * @param {string | null} expectedAuthority
 * @returns {{
 *   networkId: Buffer,
 *   creationTimeMs: bigint,
 *   ttlMs: bigint,
 *   executableArchive: Buffer,
 *   feePaymentArchive: Buffer,
 *   metadataArchive: Buffer,
 * }}
 */
export function inspectCanonicalTransactionPayloadBindings(
  payloadBytes,
  expectedAuthority = null,
) {
  const payload = bytes(payloadBytes, "transaction payload", {
    maxBytes: MAX_PAYLOAD_BYTES,
  });
  if (payload.length === 0 || payload.length > MAX_PAYLOAD_BYTES) {
    fail(
      BOUNDS_EXCEEDED,
      `transaction payload must contain 1..=${MAX_PAYLOAD_BYTES} bytes`,
    );
  }
  const reader = new Reader(payload, "transaction payload");
  const networkId = validateNetworkTransactionDomainArchive(
    reader.readField("domain"),
    "transaction payload.domain",
  );
  const authorityArchive = reader.readField("authority");
  if (expectedAuthority !== null) {
    const expectedAuthorityArchive = Buffer.from(
      encodeAccountIdNoritoValue(
        expectedAuthority,
        "transaction payload expected authority",
      ),
    );
    if (!authorityArchive.equals(expectedAuthorityArchive)) {
      fail(
        AUTHORITY_MISMATCH,
        "transaction payload authority does not match the requested signer",
      );
    }
  }
  const creationTimeArchive = reader.readField("creationTimeMs");
  if (creationTimeArchive.length !== 8) {
    fail(MALFORMED_PAYLOAD, "transaction payload.creationTimeMs must be a u64");
  }
  const executableArchive = reader.readField("executable");
  const ttlMs = validateOption(
    reader.readField("ttlMs"),
    8,
    "transaction payload.ttlMs",
    { nonZero: true },
  );
  if (validateOption(reader.readField("nonce"), 4, "transaction payload.nonce", {
    nonZero: true,
  }) !== null) {
    fail(UNSUPPORTED_PAYLOAD, "transaction payload.nonce must be absent");
  }
  const feePaymentArchive = reader.readField("feePayment");
  validateTransactionAdmissionIntentArchive(
    reader.readField("admissionIntent"),
    TRANSACTION_ADMISSION_ORDINARY_TAG,
    "transaction payload.admissionIntent",
  );
  const metadataArchive = reader.readField("metadata");
  rejectLegacyFeeMetadata(
    validateMetadataArchive(metadataArchive, "transaction payload.metadata"),
  );
  if (!reader.readField("attachments").equals(Buffer.of(0))) {
    fail(UNSUPPORTED_PAYLOAD, PROOF_ATTACHMENT_UNSUPPORTED_MESSAGE);
  }
  reader.assertEof();
  return {
    networkId: Buffer.from(networkId),
    creationTimeMs: creationTimeArchive.readBigUInt64LE(0),
    ttlMs,
    executableArchive: Buffer.from(executableArchive),
    feePaymentArchive: Buffer.from(feePaymentArchive),
    metadataArchive: Buffer.from(metadataArchive),
  };
}

/**
 * Decode and bind one canonical unsigned verifying-key registry transaction.
 *
 * This is deliberately narrower than the general browser transaction decoder:
 * the payload must target the immutable network and authority supplied by the
 * caller and contain exactly one requested registry instruction. The returned
 * instruction has already passed a byte-for-byte canonical Norito round trip.
 *
 * @param {ArrayBufferView | ArrayBuffer | Buffer} payloadBytes
 * @param {{
 *   expectedNetworkId: import("./networkId.js").NetworkId,
 *   expectedAuthority: string,
 *   operation: "register" | "update",
 * }} constraints
 * @returns {object}
 */
export function decodeCanonicalVerifyingKeyTransactionPayload(
  payloadBytes,
  constraints,
) {
  const payload = bytes(payloadBytes, "verifying-key transaction payload", {
    maxBytes: MAX_VERIFYING_KEY_DRAFT_PAYLOAD_BYTES,
  });
  if (
    payload.length === 0 ||
    payload.length > MAX_VERIFYING_KEY_DRAFT_PAYLOAD_BYTES
  ) {
    fail(
      BOUNDS_EXCEEDED,
      `verifying-key transaction payload must contain 1..=${MAX_VERIFYING_KEY_DRAFT_PAYLOAD_BYTES} bytes`,
    );
  }
  const expectedNetworkId = exactNetworkId(
    constraints?.expectedNetworkId,
    "verifying-key signing context.networkId",
  );
  const expectedAuthority = accountInfo(
    constraints?.expectedAuthority,
    "verifying-key request.authority",
  );
  const operation = constraints?.operation;
  if (operation !== "register" && operation !== "update") {
    fail(
      INVALID_INPUT,
      "verifying-key signing context.operation must be register or update",
    );
  }

  const reader = new Reader(payload, "verifying-key transaction payload");
  const networkId = validateNetworkTransactionDomainArchive(
    reader.readField("domain"),
    "verifying-key transaction payload.domain",
  );
  if (!networkId.equals(expectedNetworkId)) {
    fail(
      "network_mismatch",
      "verifying-key transaction payload changed the configured NetworkId",
    );
  }
  const authorityArchive = reader.readField("authority");
  validateAccountArchive(
    authorityArchive,
    "verifying-key transaction payload.authority",
  );
  if (!authorityArchive.equals(accountArchive(expectedAuthority))) {
    fail(
      AUTHORITY_MISMATCH,
      "verifying-key transaction payload changed the requested authority",
    );
  }
  const creationTime = reader.readField("creationTimeMs");
  if (creationTime.length !== 8) {
    fail(
      MALFORMED_PAYLOAD,
      "verifying-key transaction payload.creationTimeMs must be a u64",
    );
  }

  const executable = new Reader(
    reader.readField("executable"),
    "verifying-key transaction payload.executable",
  );
  if (executable.readU32("variant") !== 0) {
    fail(
      UNSUPPORTED_EXECUTABLE,
      "verifying-key transaction payload must contain native instructions",
    );
  }
  const instructions = new Reader(
    executable.readField("instructions"),
    "verifying-key transaction payload.instructions",
  );
  const instructionCount = instructions.readU64("count");
  if (
    instructionCount >
    BigInt(MAX_VERIFYING_KEY_DRAFT_INSTRUCTION_FIELDS)
  ) {
    fail(
      BOUNDS_EXCEEDED,
      "verifying-key transaction payload contains too many instructions",
    );
  }
  const instructionArchives = [];
  for (let index = 0; index < Number(instructionCount); index += 1) {
    instructionArchives.push(instructions.readField(`item[${index}]`));
  }
  instructions.assertEof();
  executable.assertEof();
  if (instructionCount !== 1n) {
    fail(
      UNSUPPORTED_INSTRUCTION,
      "verifying-key transaction payload must contain exactly one instruction",
    );
  }
  const [instructionArchive] = instructionArchives;

  let instruction;
  try {
    instruction = noritoDecodeInstructionBoxArchive(instructionArchive);
  } catch (error) {
    fail(
      MALFORMED_INSTRUCTION,
      `verifying-key transaction payload contains a non-canonical instruction: ${error.message}`,
    );
  }

  validateRequiredTtlOption(
    reader.readField("ttlMs"),
    "verifying-key transaction payload.ttlMs",
  );
  validateOption(
    reader.readField("nonce"),
    4,
    "verifying-key transaction payload.nonce",
    { nonZero: true },
  );
  validateFeePaymentArchive(
    reader.readField("feePayment"),
    "verifying-key transaction payload.feePayment",
  );
  validateTransactionAdmissionIntentArchive(
    reader.readField("admissionIntent"),
    TRANSACTION_ADMISSION_QUEUE_PLAN_SYNCED_TAG,
    "verifying-key transaction payload.admissionIntent",
  );
  rejectLegacyFeeMetadata(
    validateMetadataArchive(
      reader.readField("metadata"),
      "verifying-key transaction payload.metadata",
    ),
  );
  if (!reader.readField("attachments").equals(Buffer.of(0))) {
    fail(
      UNSUPPORTED_PAYLOAD,
      "verifying-key transaction payload must not contain proof attachments",
    );
  }
  reader.assertEof();

  const variant =
    operation === "register" ? "RegisterVerifyingKey" : "UpdateVerifyingKey";
  const verifyingKeys = instruction?.verifying_keys;
  if (
    !isPlainDataObject(verifyingKeys) ||
    !isPlainDataObject(verifyingKeys[variant]) ||
    Object.keys(verifyingKeys).length !== 1
  ) {
    fail(
      UNSUPPORTED_INSTRUCTION,
      `verifying-key transaction payload must contain exactly one ${variant} instruction`,
    );
  }
  return verifyingKeys[variant];
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
    fail(INVALID_INPUT, "signature must not provide both algorithm and alg");
  }
  const byteAliases = ["signature", "bytes", "payload"].filter(
    (field) => signature[field] !== undefined,
  );
  if (byteAliases.length !== 1) {
    fail(
      INVALID_INPUT,
      "signature must provide exactly one of signature, bytes, or payload",
    );
  }
  const algorithm = signature.algorithm ?? signature.alg ?? ED25519_ALGORITHM;
  if (algorithm !== ED25519_ALGORITHM && algorithm !== 0) {
    fail(UNSUPPORTED_ALGORITHM, "only Ed25519 transaction signatures are supported");
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
  return struct([signatureArchive(signature), payload, Buffer.of(0)]);
}

function transactionHashFromPayload(payload) {
  return irohaHash(Buffer.concat([u32(0), field(payload)]));
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
  const normalized = normalizeTransferInput(input);
  const payload = encodeTransferPayload(normalized);
  validateTransactionPayload(
    payload,
    normalized.authority.literal,
    normalized.networkId,
  );
  return payload;
}

/**
 * Build a canonical browser-safe transaction payload containing only current
 * native smart-contract deployment instructions. No private key is accepted or
 * retained; the returned payload is intended for a local/external signer.
 *
 * @param {object} input
 * @returns {Buffer}
 */
export function buildBrowserInstructionTransactionPayload(input) {
  const normalized = normalizeInstructionTransactionInput(input);
  const payload = encodeInstructionTransactionPayload(normalized);
  validateInstructionTransactionPayload(
    payload,
    normalized.authority.literal,
    normalized.networkId,
  );
  return payload;
}

/**
 * Build one canonical transaction payload containing exactly one verifying-key
 * registry instruction. This narrow builder is useful for offline signing and
 * for independently checking server-produced drafts.
 *
 * @param {object} input
 * @param {"register" | "update"} operation
 * @returns {Buffer}
 */
export function buildBrowserVerifyingKeyTransactionPayload(input, operation) {
  const normalized = normalizeInstructionTransactionInput(input);
  if (normalized.instructions.length !== 1) {
    fail(
      UNSUPPORTED_INSTRUCTION,
      "verifying-key transaction must contain exactly one instruction",
    );
  }
  const payload = encodeInstructionTransactionPayload(normalized);
  decodeCanonicalVerifyingKeyTransactionPayload(payload, {
    expectedNetworkId: input.networkId,
    expectedAuthority: normalized.authority.literal,
    operation,
  });
  return payload;
}

/**
 * Build a canonical ordered `Executable::Batch` payload containing native
 * instructions and/or deployed-contract calls.
 *
 * @param {object} input
 * @returns {Buffer}
 */
export function buildBrowserExecutableBatchPayload(input) {
  const normalized = normalizeExecutableBatchTransactionInput(input);
  const payload = encodeExecutableBatchTransactionPayload(normalized);
  validateInstructionTransactionPayload(
    payload,
    normalized.authority.literal,
    normalized.networkId,
  );
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
    fail(BOUNDS_EXCEEDED, `payloadBytes must contain 1..=${MAX_PAYLOAD_BYTES} bytes`);
  }
  return irohaHash(payload).toString(HEX_ENCODING);
}

/**
 * Snapshot and validate one exact canonical Transfer::Asset signable.
 *
 * This is intended for wallet and hardware-signer trust boundaries: the
 * returned buffers are detached copies, the payload hash is recomputed, and
 * the application-pinned nominal NetworkId and both the asserted and optional
 * expected authority/public key are bound to the payload.
 *
 * @param {object} signable
 * @param {{networkId?: import("./networkId.js").NetworkId | null, authority?: string | null, signingPublicKey?: ArrayBufferView | ArrayBuffer | Buffer | string | null}} constraints
 * @returns {{networkId: import("./networkId.js").NetworkId, payloadBytes: Buffer, payloadHashHex: string, authority: string, signingPublicKey: Buffer, signatureAlgorithm: ED25519_ALGORITHM}}
 */
export function validateBrowserTransferSignable(signable, constraints = {}) {
  return validateBrowserTransactionSignable(
    signable,
    constraints,
    validateTransactionPayload,
  );
}

function validateBrowserTransactionSignable(
  signable,
  constraints,
  validatePayload,
) {
  signable = snapshotAllowedFields(signable, SIGNABLE_FIELDS, "signable");
  constraints = snapshotAllowedFields(
    constraints,
    SIGNABLE_CONSTRAINT_FIELDS,
    "signable constraints",
  );
  if (
    signable.signatureAlgorithm !== undefined &&
    signable.signatureAlgorithm !== ED25519_ALGORITHM &&
    signable.signatureAlgorithm !== "0" &&
    signable.signatureAlgorithm !== 0
  ) {
    fail(UNSUPPORTED_ALGORITHM, SIGNABLE_ALGORITHM_MESSAGE);
  }
  const payload = bytes(signable.payloadBytes, SIGNABLE_PAYLOAD_BYTES_CONTEXT, {
    maxBytes: MAX_PAYLOAD_BYTES,
  });
  const expectedNetworkId = exactNetworkId(
    signable.networkId,
    SIGNABLE_NETWORK_ID_CONTEXT,
  );
  const authorityLiteral = exactString(signable.authority, "signable.authority", {
    maxBytes: 512,
  });
  const authority = validatePayload(
    payload,
    authorityLiteral,
    expectedNetworkId,
  );
  const payloadHashHex = irohaHash(payload).toString(HEX_ENCODING);
  const assertedPayloadHashHex = exactHashHex(
    signable.payloadHashHex,
    SIGNABLE_PAYLOAD_HASH_CONTEXT,
  );
  if (assertedPayloadHashHex !== payloadHashHex) {
    fail(PAYLOAD_HASH_MISMATCH, SIGNABLE_PAYLOAD_HASH_MISMATCH_MESSAGE);
  }
  const signingPublicKey = bytes(
    signable.signingPublicKey,
    SIGNABLE_PUBLIC_KEY_CONTEXT,
    { hex: true, maxBytes: 32 },
  );
  if (signingPublicKey.length !== 32) {
    fail(INVALID_PUBLIC_KEY, SIGNABLE_PUBLIC_KEY_LENGTH_MESSAGE);
  }
  if (!signingPublicKey.equals(authority.publicKey)) {
    fail(
      AUTHORITY_MISMATCH,
      SIGNABLE_CONTROLLER_MISMATCH_MESSAGE,
    );
  }
  if (constraints.networkId !== undefined && constraints.networkId !== null) {
    requireExpectedNetworkId(
      expectedNetworkId,
      exactNetworkId(
        constraints.networkId,
        "signable constraints.networkId",
      ),
      SIGNABLE_NETWORK_ID_CONTEXT,
    );
  }
  if (constraints.authority !== undefined && constraints.authority !== null) {
    const expectedAuthority = accountInfo(
      constraints.authority,
      "signable constraints.authority",
    );
    if (expectedAuthority.literal !== authorityLiteral) {
      fail(
        AUTHORITY_MISMATCH,
        APPROVED_AUTHORITY_MISMATCH_MESSAGE,
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
        AUTHORITY_MISMATCH,
        APPROVED_SIGNER_MISMATCH_MESSAGE,
      );
    }
  }
  return Object.freeze({
    networkId: signable.networkId,
    payloadBytes: payload,
    payloadHashHex,
    authority: authorityLiteral,
    signingPublicKey,
    signatureAlgorithm: ED25519_ALGORITHM,
  });
}

/** Snapshot and validate a smart-contract deployment transaction signable. */
export function validateBrowserInstructionTransactionSignable(
  signable,
  constraints = {},
) {
  return validateBrowserTransactionSignable(
    signable,
    constraints,
    validateInstructionTransactionPayload,
  );
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
  return finalizeBrowserTransaction(
    signable,
    signature,
    signingPublicKey,
    validateTransactionPayload,
  );
}

function finalizeBrowserTransaction(
  signable,
  signature,
  signingPublicKey,
  validatePayload,
) {
  signable = snapshotAllowedFields(signable, SIGNABLE_FIELDS, "signable");
  if (
    signable.signatureAlgorithm !== undefined &&
    signable.signatureAlgorithm !== ED25519_ALGORITHM &&
    signable.signatureAlgorithm !== 0
  ) {
    fail(UNSUPPORTED_ALGORITHM, SIGNABLE_ALGORITHM_MESSAGE);
  }
  const payload = bytes(signable.payloadBytes, SIGNABLE_PAYLOAD_BYTES_CONTEXT, {
    maxBytes: MAX_PAYLOAD_BYTES,
  });
  const expectedNetworkId = exactNetworkId(
    signable.networkId,
    SIGNABLE_NETWORK_ID_CONTEXT,
  );
  const authority = validatePayload(
    payload,
    signable.authority,
    expectedNetworkId,
  );
  const publicKey = bytes(signingPublicKey, "signingPublicKey", {
    hex: true,
    maxBytes: 32,
  });
  if (publicKey.length !== 32) {
    fail(INVALID_PUBLIC_KEY, "signingPublicKey must be exactly 32 bytes");
  }
  if (signable.signingPublicKey != null) {
    const assertedPublicKey = bytes(
      signable.signingPublicKey,
      SIGNABLE_PUBLIC_KEY_CONTEXT,
      { hex: true, maxBytes: 32 },
    );
    if (assertedPublicKey.length !== 32 || !assertedPublicKey.equals(publicKey)) {
      fail(
        AUTHORITY_MISMATCH,
        PUBLIC_KEY_ARGUMENT_MISMATCH_MESSAGE,
      );
    }
  }
  if (!publicKey.equals(authority.publicKey)) {
    fail(AUTHORITY_MISMATCH, ARGUMENT_CONTROLLER_MISMATCH_MESSAGE);
  }
  const payloadHash = irohaHash(payload);
  if (signable.payloadHashHex !== undefined) {
    const asserted = exactHashHex(signable.payloadHashHex, SIGNABLE_PAYLOAD_HASH_CONTEXT);
    if (asserted !== payloadHash.toString(HEX_ENCODING)) {
      fail(PAYLOAD_HASH_MISMATCH, SIGNABLE_PAYLOAD_HASH_MISMATCH_MESSAGE);
    }
  }
  const signatureBytes = normalizeSignature(signature);
  if (signatureBytes.length !== 64) {
    fail(INVALID_SIGNATURE, "Ed25519 signature must be exactly 64 bytes");
  }
  let verified = false;
  try {
    verified = verifyEd25519(payloadHash, signatureBytes, publicKey);
  } catch {
    verified = false;
  }
  if (!verified) {
    fail(INVALID_SIGNATURE, SIGNATURE_VERIFY_MESSAGE);
  }
  const bare = bareSignedTransaction(payload, signatureBytes);
  const signedTransaction = Buffer.concat([Buffer.of(1), bare]);
  const hash = transactionHashFromPayload(payload);
  return {
    signedTransaction,
    hash,
    hashHex: hash.toString(HEX_ENCODING),
  };
}

/** Verify and finalize an externally signed deployment-instruction transaction. */
export function finalizeBrowserInstructionTransaction(
  signable,
  signature,
  signingPublicKey,
) {
  return finalizeBrowserTransaction(
    signable,
    signature,
    signingPublicKey,
    validateInstructionTransactionPayload,
  );
}

/** Validate an externally signed mixed executable-batch payload. */
export function validateBrowserExecutableBatchSignable(
  signable,
  constraints = {},
) {
  return validateBrowserInstructionTransactionSignable(signable, constraints);
}

/** Verify and finalize an externally signed mixed executable-batch payload. */
export function finalizeBrowserExecutableBatchTransaction(
  signable,
  signature,
  signingPublicKey,
) {
  return finalizeBrowserInstructionTransaction(
    signable,
    signature,
    signingPublicKey,
  );
}

/**
 * Compute the canonical pipeline hash of a versioned SignedTransaction's intent.
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
      MALFORMED_SIGNED_TRANSACTION,
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
  const multisig = reader.readField("multisigSignatures");
  reader.assertEof();
  if (
    rawSignature.length !== 64 ||
    !multisig.equals(Buffer.of(0)) ||
    payload.length === 0 ||
    payload.length > MAX_PAYLOAD_BYTES
  ) {
    fail(
      MALFORMED_SIGNED_TRANSACTION,
      "signedTransaction is not a supported single-signature external transaction",
    );
  }
  let transferError;
  try {
    validateTransactionPayload(payload, null);
  } catch (error) {
    transferError = error;
    try {
      validateInstructionTransactionPayload(payload, null);
    } catch (instructionError) {
      const boundsError = [transferError, instructionError].find(
        (error) =>
          error instanceof BrowserTransactionCodecError &&
          error.code === BOUNDS_EXCEEDED,
      );
      if (boundsError !== undefined) {
        throw boundsError;
      }
      fail(
        MALFORMED_SIGNED_TRANSACTION,
        `signedTransaction payload is neither a supported transfer nor a supported deployment transaction (${transferError.message}; ${instructionError.message})`,
      );
    }
  }
  return transactionHashFromPayload(payload).toString(HEX_ENCODING);
}

/** Browser-safe codec implementing the Nexus transaction-codec contract. */
export const browserTransactionCodec = Object.freeze({
  buildTransferPayload: buildBrowserTransferPayload,
  buildInstructionPayload: buildBrowserInstructionTransactionPayload,
  buildExecutableBatchPayload: buildBrowserExecutableBatchPayload,
  payloadHashHex: browserTransactionPayloadHashHex,
  finalizeSignedTransaction: finalizeBrowserSignedTransaction,
  finalizeInstructionTransaction: finalizeBrowserInstructionTransaction,
  finalizeExecutableBatchTransaction:
    finalizeBrowserExecutableBatchTransaction,
  validateSignable: validateBrowserTransferSignable,
  validateInstructionSignable: validateBrowserInstructionTransactionSignable,
  validateExecutableBatchSignable: validateBrowserExecutableBatchSignable,
});

export { BrowserTransactionCodecError };
