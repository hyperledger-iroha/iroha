import { Buffer } from "node:buffer";
import { noritoEncodeInstruction } from "./norito.js";
import {
  ensureCanonicalAccountId,
  normalizeAccountId,
  normalizeAssetId,
  normalizeAssetHoldingId,
  normalizeRwaId,
} from "./normalizers.js";
import { MultisigSpec, MultisigSpecBuilder } from "./multisig.js";
import {
  createValidationError,
  ValidationErrorCode,
} from "./validationError.js";

const MAX_SAFE_INTEGER = Number.MAX_SAFE_INTEGER;
const MAX_SAFE_INTEGER_BIGINT = BigInt(MAX_SAFE_INTEGER);
const MAX_NUMERIC_SCALE = 28;
const MAX_NUMERIC_BITS = 512;

function crc16(tag, body) {
  let crc = 0xffff;
  const processByte = (byte) => {
    crc ^= (byte & 0xff) << 8;
    for (let i = 0; i < 8; i += 1) {
      if ((crc & 0x8000) !== 0) {
        crc = ((crc << 1) ^ 0x1021) & 0xffff;
      } else {
        crc = (crc << 1) & 0xffff;
      }
    }
  };

  for (const byte of Buffer.from(tag, "utf8")) {
    processByte(byte);
  }
  processByte(":".charCodeAt(0));
  for (const byte of Buffer.from(body, "utf8")) {
    processByte(byte);
  }

  return crc & 0xffff;
}

function fail(code, message, path) {
  throw createValidationError(code, message, path);
}

function canonicalHashLiteral(buf) {
  const normalized = Buffer.from(buf);
  if (normalized.length !== 32) {
    fail(ValidationErrorCode.INVALID_HEX, "hash must be 32 bytes");
  }
  normalized[normalized.length - 1] |= 1;
  const body = normalized.toString("hex").toUpperCase();
  const checksum = crc16("hash", body).toString(16).toUpperCase().padStart(4, "0");
  return `hash:${body}#${checksum}`;
}

function parseHashLiteralToBuffer(literal, name) {
  const match = /^hash:([0-9A-Fa-f]{64})#([0-9A-Fa-f]{4})$/.exec(literal.trim());
  if (!match) {
    fail(
      ValidationErrorCode.INVALID_HEX,
      `${name} must be a canonical "hash:<HEX>#<CRC>" literal`,
      name,
    );
  }
  const [, body, checksum] = match;
  const bodyUpper = body.toUpperCase();
  const expected = crc16("hash", bodyUpper).toString(16).toUpperCase().padStart(4, "0");
  if (expected !== checksum.toUpperCase()) {
    fail(
      ValidationErrorCode.INVALID_HEX,
      `${name} has invalid checksum; expected ${expected}`,
      name,
    );
  }
  return Buffer.from(bodyUpper, "hex");
}

function parseHashLiteral(literal, name) {
  return canonicalHashLiteral(parseHashLiteralToBuffer(literal, name));
}

function assertString(value, name) {
  if (typeof value !== "string" || value.length === 0) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty string`, name);
  }
  return value;
}

function normalizeNumericLiteral(value, name, { allowNegative = false } = {}) {
  let raw;
  if (typeof value === "string") {
    raw = value.trim();
  } else if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be a finite number`, name);
    }
    raw = value.toString();
  } else if (typeof value === "bigint") {
    raw = value.toString();
  } else {
    fail(
      ValidationErrorCode.INVALID_NUMERIC,
      `${name} must be a string, number, or bigint representing a Numeric`,
      name,
    );
  }

  if (!raw) {
    fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be a valid Numeric literal`, name);
  }

  let digits = raw;
  const sign = digits[0];
  if (sign === "-" || sign === "+") {
    if (sign === "-" && !allowNegative) {
      fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be non-negative`, name);
    }
    digits = digits.slice(1);
  }
  if (!digits) {
    fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be a valid Numeric literal`, name);
  }

  let seenDot = false;
  let scale = 0;
  let mantissa = "";
  for (const ch of digits) {
    if (ch === ".") {
      if (seenDot) {
        fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be a valid Numeric literal`, name);
      }
      seenDot = true;
      continue;
    }
    if (ch < "0" || ch > "9") {
      fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be a valid Numeric literal`, name);
    }
    mantissa += ch;
    if (seenDot) {
      scale += 1;
    }
  }
  if (!mantissa) {
    fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be a valid Numeric literal`, name);
  }
  if (scale > MAX_NUMERIC_SCALE) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} scale exceeds ${MAX_NUMERIC_SCALE} decimal places`,
      name,
    );
  }

  let mantissaValue = BigInt(mantissa);
  if (sign === "-") {
    mantissaValue = -mantissaValue;
  }
  const absValue = mantissaValue < 0n ? -mantissaValue : mantissaValue;
  if (absValue !== 0n && absValue.toString(2).length > MAX_NUMERIC_BITS) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} mantissa exceeds ${MAX_NUMERIC_BITS} bits`,
      name,
    );
  }

  return raw;
}

function asNumericQuantity(value, name) {
  return normalizeNumericLiteral(value, name, { allowNegative: false });
}

function asU128JsonNumber(value, name) {
  if (typeof value === "number") {
    if (!Number.isFinite(value) || !Number.isInteger(value) || value < 0) {
      fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be a non-negative integer`, name);
    }
    if (!Number.isSafeInteger(value)) {
      fail(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name} must be between 0 and ${MAX_SAFE_INTEGER} (inclusive) for deterministic JSON encoding`,
        name,
      );
    }
    return value;
  }
  if (typeof value === "bigint") {
    if (value < 0n || value > MAX_SAFE_INTEGER_BIGINT) {
      fail(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name} must be between 0 and ${MAX_SAFE_INTEGER} (inclusive) for deterministic JSON encoding`,
        name,
      );
    }
    return Number(value);
  }
  if (typeof value === "string") {
    if (!/^[0-9]+$/.test(value)) {
      fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be a non-negative integer string`, name);
    }
    const numeric = BigInt(value);
    if (numeric > MAX_SAFE_INTEGER_BIGINT) {
      fail(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name} exceeds the maximum JSON-safe integer (${MAX_SAFE_INTEGER}); supply a smaller value`,
        name,
      );
    }
    return Number(numeric);
  }
  fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be a non-negative integer`, name);
}

function asPositiveInteger(value, name) {
  if (typeof value === "bigint") {
    if (value <= 0n) {
      fail(ValidationErrorCode.VALUE_OUT_OF_RANGE, `${name} must be greater than zero`, name);
    }
    if (value > MAX_SAFE_INTEGER_BIGINT) {
      fail(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name} exceeds JavaScript safe integer range`,
        name,
      );
    }
    return Number(value);
  }
  if (typeof value === "number") {
    if (!Number.isInteger(value) || value <= 0) {
      fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be a positive integer`, name);
    }
    if (!Number.isSafeInteger(value)) {
      fail(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name} exceeds JavaScript safe integer range`,
        name,
      );
    }
    return value;
  }
  if (typeof value === "string") {
    if (!/^[1-9]\d*$/.test(value)) {
      fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be a positive integer`, name);
    }
    const numeric = BigInt(value);
    if (numeric > MAX_SAFE_INTEGER_BIGINT) {
      fail(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name} exceeds JavaScript safe integer range`,
        name,
      );
    }
    return Number(numeric);
  }
  fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be a positive integer`, name);
}

function assertPlainObject(value, name) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(ValidationErrorCode.INVALID_OBJECT, `${name} must be a plain object`, name);
  }
  return value;
}

function normalizeJsonValue(value, path) {
  if (
    value === null ||
    typeof value === "string" ||
    typeof value === "boolean"
  ) {
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      fail(ValidationErrorCode.INVALID_JSON_VALUE, `${path} must not contain non-finite numbers`, path);
    }
    return value;
  }
  if (typeof value === "bigint") {
    return value.toString();
  }
  if (Array.isArray(value)) {
    return value.map((entry, index) =>
      normalizeJsonValue(entry, `${path}[${index}]`),
    );
  }
  if (typeof value === "object") {
    const result = {};
    for (const [key, nested] of Object.entries(value)) {
      if (typeof key !== "string" || key.length === 0) {
        fail(ValidationErrorCode.INVALID_JSON_VALUE, `${path} keys must be non-empty strings`, path);
      }
      result[key] = normalizeJsonValue(nested, `${path}.${key}`);
    }
    return result;
  }
  fail(
    ValidationErrorCode.INVALID_JSON_VALUE,
    `${path} contains unsupported value type: ${typeof value}`,
    path,
  );
}

function normalizeMetadata(metadata) {
  if (metadata === undefined || metadata === null) {
    return {};
  }
  const base = assertPlainObject(metadata, "metadata");
  return normalizeJsonValue(base, "metadata");
}

function normalizeBooleanFlag(value, name) {
  if (value === undefined || value === null) {
    return false;
  }
  if (typeof value !== "boolean") {
    fail(ValidationErrorCode.INVALID_OBJECT, `${name} must be a boolean`, name);
  }
  return value;
}

function normalizeJsonObjectLike(value, name) {
  if (typeof value === "string") {
    let parsed;
    try {
      parsed = JSON.parse(value);
    } catch (error) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${name} must be a plain object or JSON object string`,
        name,
      );
    }
    return assertPlainObject(parsed, name);
  }
  return assertPlainObject(value, name);
}

function normalizeOptionalString(value, name) {
  if (value === undefined || value === null) {
    return null;
  }
  return assertString(value, name);
}

function normalizeRwaParentRefs(value, path) {
  if (value === undefined || value === null) {
    return [];
  }
  if (!Array.isArray(value)) {
    fail(ValidationErrorCode.INVALID_OBJECT, `${path} must be an array`, path);
  }
  return value.map((entry, index) => {
    const source = normalizeJsonObjectLike(entry, `${path}[${index}]`);
    return {
      rwa: normalizeRwaId(source.rwa, `${path}[${index}].rwa`),
      quantity: asNumericQuantity(source.quantity, `${path}[${index}].quantity`),
    };
  });
}

function normalizeRwaControlPolicy(value, path) {
  const source =
    value === undefined || value === null ? {} : normalizeJsonObjectLike(value, path);
  const controllerAccountsInput =
    source.controllerAccounts ?? source.controller_accounts ?? [];
  const controllerRolesInput = source.controllerRoles ?? source.controller_roles ?? [];
  if (!Array.isArray(controllerAccountsInput)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${path}.controllerAccounts must be an array`,
      `${path}.controllerAccounts`,
    );
  }
  if (!Array.isArray(controllerRolesInput)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${path}.controllerRoles must be an array`,
      `${path}.controllerRoles`,
    );
  }
  return {
    controller_accounts: controllerAccountsInput.map((accountId, index) =>
      normalizeAccountId(accountId, `${path}.controllerAccounts[${index}]`),
    ),
    controller_roles: controllerRolesInput.map((roleId, index) =>
      assertString(roleId, `${path}.controllerRoles[${index}]`),
    ),
    freeze_enabled: normalizeBooleanFlag(
      source.freezeEnabled ?? source.freeze_enabled,
      `${path}.freezeEnabled`,
    ),
    hold_enabled: normalizeBooleanFlag(
      source.holdEnabled ?? source.hold_enabled,
      `${path}.holdEnabled`,
    ),
    force_transfer_enabled: normalizeBooleanFlag(
      source.forceTransferEnabled ?? source.force_transfer_enabled,
      `${path}.forceTransferEnabled`,
    ),
    redeem_enabled: normalizeBooleanFlag(
      source.redeemEnabled ?? source.redeem_enabled,
      `${path}.redeemEnabled`,
    ),
  };
}

function normalizeRegisterRwaPayload(value, path = "rwa") {
  const source = normalizeJsonObjectLike(value, path);
  return {
    domain: assertString(source.domain, `${path}.domain`),
    quantity: asNumericQuantity(source.quantity, `${path}.quantity`),
    spec: normalizeJsonValue(assertPlainObject(source.spec, `${path}.spec`), `${path}.spec`),
    primary_reference: assertString(
      source.primaryReference ?? source.primary_reference,
      `${path}.primaryReference`,
    ),
    status: normalizeOptionalString(source.status, `${path}.status`),
    metadata:
      source.metadata === undefined || source.metadata === null
        ? {}
        : normalizeJsonValue(assertPlainObject(source.metadata, `${path}.metadata`), `${path}.metadata`),
    parents: normalizeRwaParentRefs(source.parents, `${path}.parents`),
    controls: normalizeRwaControlPolicy(source.controls, `${path}.controls`),
  };
}

function normalizeMergeRwasPayload(value, path = "merge") {
  const source = normalizeJsonObjectLike(value, path);
  return {
    parents: normalizeRwaParentRefs(source.parents, `${path}.parents`),
    primary_reference: assertString(
      source.primaryReference ?? source.primary_reference,
      `${path}.primaryReference`,
    ),
    status: normalizeOptionalString(source.status, `${path}.status`),
    metadata:
      source.metadata === undefined || source.metadata === null
        ? {}
        : normalizeJsonValue(assertPlainObject(source.metadata, `${path}.metadata`), `${path}.metadata`),
  };
}

function normalizeMultisigSpecPayload(spec, path) {
  if (spec instanceof MultisigSpec) {
    return spec.toPayload();
  }
  const source = assertPlainObject(spec, path);
  const builder = new MultisigSpecBuilder();
  const quorum = source.quorum ?? source.quorumRaw;
  if (quorum === undefined || quorum === null) {
    fail(
      ValidationErrorCode.MISSING_FIELD,
      `${path}.quorum is required`,
      `${path}.quorum`,
    );
  }
  builder.setQuorum(quorum);

  const ttl =
    source.transaction_ttl_ms ??
    source.transactionTtlMs ??
    source.transaction_ttl ??
    source.transactionTtl;
  if (ttl === undefined || ttl === null) {
    fail(
      ValidationErrorCode.MISSING_FIELD,
      `${path}.transaction_ttl_ms is required`,
      `${path}.transaction_ttl_ms`,
    );
  }
  builder.setTransactionTtlMs(ttl);

  const rawSignatories = source.signatories ?? source.members;
  const signatories = assertPlainObject(
    rawSignatories,
    `${path}.signatories`,
  );
  const entries = Object.entries(signatories);
  if (entries.length === 0) {
    fail(
      ValidationErrorCode.MISSING_FIELD,
      `${path}.signatories must contain at least one entry`,
      `${path}.signatories`,
    );
  }
  for (const [accountId, weight] of entries) {
    builder.addSignatory(accountId, weight);
  }
  return builder.build().toPayload();
}

function asNonNegativeInteger(value, name) {
  if (typeof value === "bigint") {
    if (value < 0n) {
      fail(ValidationErrorCode.VALUE_OUT_OF_RANGE, `${name} must be greater than or equal to zero`, name);
    }
    if (value > MAX_SAFE_INTEGER_BIGINT) {
      fail(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name} exceeds JavaScript safe integer range`,
        name,
      );
    }
    const asNumber = Number(value);
    return asNumber;
  }
  if (typeof value === "number") {
    if (!Number.isInteger(value) || value < 0) {
      fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be a non-negative integer`, name);
    }
    if (!Number.isSafeInteger(value)) {
      fail(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name} exceeds JavaScript safe integer range`,
        name,
      );
    }
    return value;
  }
  if (typeof value === "string") {
    if (!/^(?:0|[1-9]\d*)$/.test(value)) {
      fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be a non-negative integer`, name);
    }
    const numeric = BigInt(value);
    if (numeric > MAX_SAFE_INTEGER_BIGINT) {
      fail(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name} exceeds JavaScript safe integer range`,
        name,
      );
    }
    return Number(numeric);
  }
  fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be a non-negative integer`, name);
}

function asByte(value, name) {
  const numeric = asNonNegativeInteger(value, name);
  if (numeric > 0xff) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} must be an integer between 0 and 255`,
      name,
    );
  }
  return numeric;
}

function toBinaryBuffer(value, name) {
  if (Buffer.isBuffer(value)) {
    return Buffer.from(value);
  }
  if (ArrayBuffer.isView(value)) {
    return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return Buffer.from(value);
  }
  if (Array.isArray(value)) {
    return Buffer.from(normalizeByteArray(value, name));
  }
  if (value && typeof value.length === "number" && typeof value !== "string") {
    return Buffer.from(normalizeByteArray(Array.from(value), name));
  }
  fail(
    ValidationErrorCode.INVALID_OBJECT,
    `${name} must be a Buffer, ArrayBuffer view, or byte array`,
    name,
  );
}

function normalizeHash(value, name) {
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (trimmed.startsWith("hash:")) {
      return parseHashLiteral(trimmed, name);
    }
    if (!/^[0-9A-Fa-f]{64}$/.test(trimmed)) {
      fail(
        ValidationErrorCode.INVALID_HEX,
        `${name} must be a 64-character hexadecimal string or hash literal`,
        name,
      );
    }
    return canonicalHashLiteral(Buffer.from(trimmed, "hex"));
  }
  const buffer = toBinaryBuffer(value, name);
  if (buffer.length !== 32) {
    fail(ValidationErrorCode.INVALID_HEX, `${name} must be 32 bytes`, name);
  }
  return canonicalHashLiteral(buffer);
}

function normalizeOptionalHash(value, name) {
  if (value === undefined || value === null) {
    return null;
  }
  return normalizeHash(value, name);
}

function normalizeKeyedHashInput(value, name) {
  const source = assertPlainObject(value, name);
  const pepperId =
    source.pepper_id ??
    source.pepperId ??
    source.pepper_id_hex ??
    source.pepper ??
    null;
  const digestValue =
    source.digest ??
    source.hash ??
    source.value ??
    source.bindingHash ??
    source.binding_hash;
  const pepper = assertString(
    pepperId,
    `${name}.pepperId`,
  );
  const digest = normalizeHash(
    digestValue,
    `${name}.digest`,
  );
  return {
    pepper_id: pepper,
    digest,
  };
}

function normalizeFixedBytes(value, name, length = 32) {
  if (value === undefined || value === null) {
    fail(ValidationErrorCode.INVALID_OBJECT, `${name} is required`, name);
  }
  if (Array.isArray(value)) {
    if (value.length !== length) {
      fail(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name} must contain exactly ${length} elements`,
        name,
      );
    }
    return value.map((byte, index) => {
      const numeric = Number(byte);
      if (!Number.isInteger(numeric) || numeric < 0 || numeric > 0xff) {
        fail(
          ValidationErrorCode.VALUE_OUT_OF_RANGE,
          `${name}[${index}] must be an integer between 0 and 255`,
          `${name}[${index}]`,
        );
      }
      return numeric;
    });
  }

  let buffer;
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (trimmed.length === 0) {
      fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty string`, name);
    }
    if (trimmed.startsWith("hash:")) {
      buffer = parseHashLiteralToBuffer(trimmed, name);
    } else if (/^[0-9A-Fa-f]+$/.test(trimmed) && trimmed.length === length * 2) {
      buffer = Buffer.from(trimmed, "hex");
    } else {
      buffer = decodeBase64Strict(trimmed, name);
    }
  } else {
    buffer = toBinaryBuffer(value, name);
  }

  if (buffer.length !== length) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} must be ${length} bytes; received ${buffer.length}`,
      name,
    );
  }

  return Array.from(buffer.values());
}

function normalizeOptionalFixedBytes(value, name, length = 32) {
  if (value === undefined || value === null) {
    return null;
  }
  return normalizeFixedBytes(value, name, length);
}

function normalizeByteArray(value, name) {
  if (value === undefined || value === null) {
    fail(ValidationErrorCode.INVALID_OBJECT, `${name} is required`, name);
  }
  if (Array.isArray(value)) {
    if (value.length === 0) {
      fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty byte array`, name);
    }
    return value.map((byte, index) => {
      const numeric = Number(byte);
      if (!Number.isInteger(numeric) || numeric < 0 || numeric > 0xff) {
        fail(
          ValidationErrorCode.VALUE_OUT_OF_RANGE,
          `${name}[${index}] must be an integer between 0 and 255`,
          `${name}[${index}]`,
        );
      }
      return numeric;
    });
  }
  if (Buffer.isBuffer(value)) {
    if (value.length === 0) {
      fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty byte array`, name);
    }
    return Array.from(value.values());
  }
  if (typeof value === "string") {
    const b64 = normalizeBase64(value, name);
    return Array.from(Buffer.from(b64, "base64").values());
  }
  const buffer = toBinaryBuffer(value, name);
  if (buffer.length === 0) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty byte array`, name);
  }
  return Array.from(buffer.values());
}

function normalizeHexHashString(value, name) {
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (!/^[0-9A-Fa-f]{64}$/.test(trimmed)) {
      fail(
        ValidationErrorCode.INVALID_HEX,
        `${name} must be a 64-character hexadecimal string`,
        name,
      );
    }
    return trimmed.toLowerCase();
  }
  const buffer = toBinaryBuffer(value, name);
  if (buffer.length !== 32) {
    fail(ValidationErrorCode.INVALID_HEX, `${name} must be 32 bytes`, name);
  }
  return Buffer.from(buffer).toString("hex");
}

function normalizeVerifyingKeyId(value, name) {
  if (value === undefined || value === null) {
    return null;
  }
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (trimmed.length === 0) {
      fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty string`, name);
    }
    const parts = trimmed.split(":");
    if (parts.length !== 2 || parts[0].length === 0 || parts[1].length === 0) {
      fail(
        ValidationErrorCode.INVALID_STRING,
        `${name} must be in 'backend:name' format`,
        name,
      );
    }
    return {
      backend: parts[0],
      name: parts[1],
    };
  }
  const object = assertPlainObject(value, name);
  const backend = assertString(object.backend ?? object.backendId, `${name}.backend`);
  const keyName = assertString(object.name ?? object.id ?? object.key, `${name}.name`);
  return { backend, name: keyName };
}

function normalizeZkAssetMode(value, name) {
  const raw = value ?? "Hybrid";
  const normalized = String(raw).trim().toLowerCase();
  if (normalized === "zknative" || normalized === "zk-native" || normalized === "zk_native") {
    return "ZkNative";
  }
  if (normalized === "hybrid") {
    return "Hybrid";
  }
  fail(
    ValidationErrorCode.INVALID_STRING,
    `${name} must be 'ZkNative' or 'Hybrid'`,
    name,
  );
}

function normalizeConfidentialPolicyMode(value, name) {
  const raw = value ?? "Convertible";
  const normalized = String(raw)
    .trim()
    .toLowerCase()
    .replace(/[-_]/g, "");
  switch (normalized) {
    case "transparentonly":
      return "TransparentOnly";
    case "shieldedonly":
      return "ShieldedOnly";
    case "convertible":
      return "Convertible";
    default:
      fail(
        ValidationErrorCode.INVALID_STRING,
        `${name} must be TransparentOnly, ShieldedOnly, or Convertible`,
        name,
      );
  }
}

function normalizeConfidentialEncryptedPayload(value, name) {
  const source = assertPlainObject(value, name);
  const version = asByte(source.version ?? source.payloadVersion ?? 1, `${name}.version`);
  const ephemeral = normalizeFixedBytes(
    source.ephemeralPublicKey ?? source.ephemeral_pubkey ?? source.ephemeralKey,
    `${name}.ephemeralPublicKey`,
    32,
  );
  const nonce = normalizeFixedBytes(source.nonce, `${name}.nonce`, 24);
  const ciphertextValue = source.ciphertext ?? source.ciphertextB64 ?? source.ciphertext_base64;
  const ciphertext = normalizeBase64(ciphertextValue, `${name}.ciphertext`);
  return {
    version,
    ephemeral_pubkey: ephemeral,
    nonce,
    ciphertext,
  };
}

function normalizeProofAttachment(value, name) {
  const source = assertPlainObject(value, name);
  const backend = assertString(source.backend, `${name}.backend`);
  let rawProof =
    source.proofBytes ??
    source.proof_bytes ??
    source.proof ??
    source.proof_b64 ??
    source.proofBase64;
  const attachmentHasProofBytes =
    "proof" in source || "proof_b64" in source || "proofB64" in source;
  const structuredProofBox =
    !attachmentHasProofBytes &&
    rawProof &&
    typeof rawProof === "object" &&
    !Array.isArray(rawProof);

  let verifyRef = source.verifyingKeyRef ?? source.vkRef ?? source.vk_ref;
  let verifyInline =
    source.verifyingKeyInline ?? source.vkInline ?? source.vk_inline;
  let commitmentInput =
    source.verifyingKeyCommitment ?? source.vkCommitment ?? source.vk_commitment;
  let envelopeInput =
    source.envelopeHash ?? source.envelope_hash ?? source.proofEnvelopeHash;

  let proofBox;
  if (attachmentHasProofBytes) {
    const proofBackend = assertString(
      source.backend ?? backend,
      `${name}.proof.backend`,
    );
    const proofValue = source.proof ?? source.proof_b64 ?? source.proofB64;
    proofBox = {
      backend: proofBackend,
      bytes: normalizeByteArray(proofValue, `${name}.proof`),
    };
    verifyRef ??=
      source.verifyingKeyRef ??
      source.vkRef ??
      source.vk_ref ??
      source.vk_reference;
    verifyInline ??=
      source.verifyingKeyInline ??
      source.vkInline ??
      source.vk_inline ??
      source.inline_vk;
    commitmentInput ??=
      source.verifyingKeyCommitment ??
      source.vkCommitment ??
      source.vk_commitment;
    envelopeInput ??= source.envelopeHash ?? source.envelope_hash;
  } else if (structuredProofBox) {
    const proofBackend = assertString(
      rawProof.backend ?? backend,
      `${name}.proof.backend`,
    );
    const proofBytes =
      rawProof.bytes ??
      rawProof.bytes_b64 ??
      rawProof.bytesBase64 ??
      rawProof.data ??
      rawProof.payload;
    proofBox = {
      backend: proofBackend,
      bytes: normalizeByteArray(proofBytes, `${name}.proof.bytes`),
    };
  } else {
    const proofBytes = normalizeByteArray(rawProof, `${name}.proof`);
    proofBox = {
      backend,
      bytes: proofBytes,
    };
  }

  if (!verifyRef && !verifyInline) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must include verifyingKeyRef or verifyingKeyInline`,
      name,
    );
  }

  const payload = { backend, proof: proofBox };
  if (verifyRef) {
    payload.vk_ref = normalizeVerifyingKeyId(verifyRef, `${name}.verifyingKeyRef`);
  } else {
    const inline = assertPlainObject(verifyInline, `${name}.verifyingKeyInline`);
    const inlineBackend = assertString(
      inline.backend ?? inline.backendId,
      `${name}.verifyingKeyInline.backend`,
    );
    const inlineBytes = inline.bytes ?? inline.bytes_b64 ?? inline.bytesBase64;
    payload.vk_inline = {
      backend: inlineBackend,
      bytes: normalizeByteArray(inlineBytes, `${name}.verifyingKeyInline.bytes`),
    };
  }

  const commitment = normalizeOptionalFixedBytes(
    commitmentInput,
    `${name}.verifyingKeyCommitment`,
    32,
  );
  if (commitment) {
    payload.vk_commitment = commitment;
  }
  const envelopeHash = normalizeOptionalFixedBytes(
    envelopeInput,
    `${name}.envelopeHash`,
    32,
  );
  if (envelopeHash) {
    payload.envelope_hash = envelopeHash;
  }
  const lanePrivacy = source.lanePrivacy ?? source.lane_privacy;
  if (lanePrivacy !== undefined && lanePrivacy !== null) {
    const lp = assertPlainObject(lanePrivacy, `${name}.lanePrivacy`);
    const commitmentId = asPositiveInteger(
      lp.commitmentId ?? lp.commitment_id,
      `${name}.lanePrivacy.commitmentId`,
    );
    if (commitmentId > 0xffff) {
      fail(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name}.lanePrivacy.commitmentId must fit within a u16`,
        `${name}.lanePrivacy.commitmentId`,
      );
    }
    const merkle = lp.merkle ?? {
      leaf: lp.leaf,
      leafIndex: lp.leafIndex ?? lp.leaf_index,
      auditPath: lp.auditPath ?? lp.audit_path,
    };
    const merklePayload = assertPlainObject(merkle, `${name}.lanePrivacy.merkle`);
    const leaf = normalizeFixedBytes(
      merklePayload.leaf,
      `${name}.lanePrivacy.merkle.leaf`,
      32,
    );
    const leafIndex = asNonNegativeInteger(
      merklePayload.leafIndex ?? merklePayload.leaf_index ?? 0,
      `${name}.lanePrivacy.merkle.leafIndex`,
    );
    const rawAudit = merklePayload.auditPath ?? merklePayload.audit_path ?? [];
    if (!Array.isArray(rawAudit)) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${name}.lanePrivacy.merkle.auditPath must be an array`,
        `${name}.lanePrivacy.merkle.auditPath`,
      );
    }
    const auditPath = rawAudit.map((entry, index) => {
      if (entry === null || entry === undefined) {
        return null;
      }
      return normalizeFixedBytes(
        entry,
        `${name}.lanePrivacy.merkle.auditPath[${index}]`,
        32,
      );
    });
    payload.lane_privacy = {
      commitment_id: commitmentId,
      witness: {
        kind: "merkle",
        payload: {
          leaf,
          proof: {
            leaf_index: leafIndex,
            audit_path: auditPath,
          },
        },
      },
    };
  }
  return payload;
}

function normalizeAccessSetHints(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  const hints = assertPlainObject(value, context);
  const normalizeKeys = (keys, name) => {
    if (keys === undefined || keys === null) {
      return [];
    }
    if (!Array.isArray(keys)) {
      fail(ValidationErrorCode.INVALID_OBJECT, `${name} must be an array of strings`, name);
    }
    return keys.map((entry, index) =>
      assertString(entry, `${name}[${index}]`),
    );
  };
  return {
    read_keys: normalizeKeys(
      hints.read_keys ?? hints.readKeys,
      `${context}.readKeys`,
    ),
    write_keys: normalizeKeys(
      hints.write_keys ?? hints.writeKeys,
      `${context}.writeKeys`,
    ),
  };
}

function decodeBase64Strict(value, name) {
  const compact = value.replace(/\s+/g, "");
  if (compact.length === 0) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be a non-empty base64 string`,
      name,
    );
  }

  let padded = compact;
  const paddingIndex = compact.indexOf("=");
  if (paddingIndex !== -1) {
    const head = compact.slice(0, paddingIndex);
    const padding = compact.slice(paddingIndex);
    if (!/^[0-9A-Za-z+/]*$/.test(head) || !/^={1,2}$/.test(padding)) {
      fail(ValidationErrorCode.INVALID_STRING, `${name} must be a valid base64 string`, name);
    }
    if (compact.length % 4 !== 0) {
      fail(ValidationErrorCode.INVALID_STRING, `${name} must be a valid base64 string`, name);
    }
  } else {
    if (!/^[0-9A-Za-z+/]+$/.test(compact) || compact.length % 4 === 1) {
      fail(ValidationErrorCode.INVALID_STRING, `${name} must be a valid base64 string`, name);
    }
    const padLength = (4 - (compact.length % 4)) % 4;
    padded = compact + "=".repeat(padLength);
  }

  const decoded = Buffer.from(padded, "base64");
  if (decoded.toString("base64") !== padded) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must be a valid base64 string`, name);
  }
  return decoded;
}

function normalizeBase64(value, name) {
  if (typeof value === "string") {
    return decodeBase64Strict(value.trim(), name).toString("base64");
  }
  const buffer = toBinaryBuffer(value, name);
  if (buffer.length === 0) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty base64 string`, name);
  }
  return buffer.toString("base64");
}

function normalizeOptionalBase64(value, name) {
  if (value === undefined || value === null) {
    return null;
  }
  return normalizeBase64(value, name);
}

function normalizeKaigiId(value, name) {
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (trimmed.length === 0 || !trimmed.includes(":")) {
      fail(
        ValidationErrorCode.INVALID_STRING,
        `${name} must be in 'domain:callName' format`,
        name,
      );
    }
    const [domain, ...rest] = trimmed.split(":");
    const call = rest.join(":");
    return {
      domain_id: assertString(domain, `${name}.domain_id`),
      call_name: assertString(call, `${name}.call_name`),
    };
  }
  const object = assertPlainObject(value, name);
  const domainId = object.domain_id ?? object.domainId;
  const callName = object.call_name ?? object.callName ?? object.name;
  return {
    domain_id: assertString(domainId, `${name}.domain_id`),
    call_name: assertString(callName, `${name}.call_name`),
  };
}

function normalizeKaigiRelayHop(value, context) {
  const hop = assertPlainObject(value, context);
  const relayId = hop.relay_id ?? hop.relayId;
  const hpkeKey = hop.hpke_public_key ?? hop.hpkePublicKey;
  return {
    relay_id: normalizeAccountId(relayId, `${context}.relayId`),
    hpke_public_key: normalizeBase64(
      hpkeKey,
      `${context}.hpkePublicKey`,
    ),
    weight: asByte(hop.weight ?? 1, `${context}.weight`),
  };
}

function normalizeKaigiRelayManifest(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  const manifest = assertPlainObject(value, context);
  const expiryMs = manifest.expiry_ms ?? manifest.expiryMs;
  const hopsValue = manifest.hops ?? [];
  if (!Array.isArray(hopsValue)) {
    fail(ValidationErrorCode.INVALID_OBJECT, `${context}.hops must be an array`, context);
  }
  const hops = hopsValue.map((hop, index) =>
    normalizeKaigiRelayHop(hop, `${context}.hops[${index}]`),
  );
  return {
    hops,
    expiry_ms: asNonNegativeInteger(expiryMs, `${context}.expiryMs`),
  };
}

function normalizePrivacyMode(value) {
  if (value && typeof value === "object") {
    const modeValue = value.mode ?? value.Mode ?? value.privacyMode ?? value.state;
    const stateValue =
      value.state === undefined ? null : value.state === null ? null : value.state;
    return {
      mode: normalizePrivacyModeTag(modeValue),
      state: stateValue,
    };
  }
  return {
    mode: normalizePrivacyModeTag(value),
    state: null,
  };
}

function normalizePrivacyModeTag(value) {
  if (value === undefined || value === null) {
    return "Transparent";
  }
  const normalized = String(value).trim().toLowerCase();
  if (normalized === "transparent") {
    return "Transparent";
  }
  if (
    normalized === "zkrosterv1" ||
    normalized === "zk_roster_v1" ||
    normalized === "zk-roster-v1" ||
    normalized === "zkroster-v1"
  ) {
    return "ZkRosterV1";
  }
  fail(
    ValidationErrorCode.INVALID_STRING,
    "privacyMode must be either 'Transparent' or 'ZkRosterV1'",
  );
}

function normalizeRoomPolicy(value) {
  if (value && typeof value === "object") {
    const policyValue =
      value.policy ?? value.Policy ?? value.roomPolicy ?? value.state;
    const stateValue =
      value.state === undefined ? null : value.state === null ? null : value.state;
    return {
      policy: normalizeRoomPolicyTag(policyValue),
      state: stateValue,
    };
  }
  return {
    policy: normalizeRoomPolicyTag(value),
    state: null,
  };
}

function normalizeRoomPolicyTag(value) {
  if (value === undefined || value === null) {
    return "Authenticated";
  }
  const normalized = String(value).trim().toLowerCase();
  if (
    normalized === "public" ||
    normalized === "read-only" ||
    normalized === "read_only" ||
    normalized === "open"
  ) {
    return "Public";
  }
  if (
    normalized === "authenticated" ||
    normalized === "auth" ||
    normalized === "protected"
  ) {
    return "Authenticated";
  }
  fail(
    ValidationErrorCode.INVALID_STRING,
    "roomPolicy must be either 'Public' or 'Authenticated'",
  );
}

function normalizeKaigiParticipantCommitment(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  const commitment = assertPlainObject(value, context);
  const alias = commitment.alias_tag ?? commitment.aliasTag ?? null;
  return {
    commitment: normalizeHash(
      commitment.commitment,
      `${context}.commitment`,
    ),
    alias_tag: alias === null || alias === undefined ? null : assertString(alias, `${context}.aliasTag`),
  };
}

function normalizeKaigiParticipantNullifier(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  const nullifier = assertPlainObject(value, context);
  const digest = nullifier.digest ?? nullifier.hash ?? nullifier.value;
  return {
    digest: normalizeHash(digest, `${context}.digest`),
    issued_at_ms: asNonNegativeInteger(
      nullifier.issued_at_ms ?? nullifier.issuedAtMs ?? nullifier.issuedAt,
      `${context}.issuedAtMs`,
    ),
  };
}

function normalizeNewKaigi(options) {
  const source = assertPlainObject(options, "createKaigi.call");
  const idValue = source.id ?? source.callId ?? source.call_id;
  const hostValue = source.host ?? source.hostAccountId ?? source.authority;
  const titleValue = source.title ?? null;
  const descriptionValue = source.description ?? null;
  const maxParticipantsValue = source.max_participants ?? source.maxParticipants;
  const gasRateValue =
    source.gas_rate_per_minute ?? source.gasRatePerMinute ?? source.gasRate ?? 0;
  const scheduledStartValue =
    source.scheduled_start_ms ?? source.scheduledStartMs ?? null;
  const billingAccountValue =
    source.billing_account ?? source.billingAccount ?? null;
  const privacyValue =
    source.privacy_mode ?? source.privacyMode ?? source.privacy ?? "Transparent";
  const roomPolicyValue =
    source.room_policy ?? source.roomPolicy ?? source.roomAccess ?? "authenticated";
  const relayManifestValue =
    source.relay_manifest ?? source.relayManifest ?? null;

  const call = {
    id: normalizeKaigiId(idValue, "call.id"),
    host: normalizeAccountId(hostValue, "call.host"),
    title:
      titleValue === undefined || titleValue === null
        ? null
        : assertString(titleValue, "call.title"),
    description:
      descriptionValue === undefined || descriptionValue === null
        ? null
        : assertString(descriptionValue, "call.description"),
    max_participants:
      maxParticipantsValue === undefined || maxParticipantsValue === null
        ? null
        : asPositiveInteger(maxParticipantsValue, "call.maxParticipants"),
    gas_rate_per_minute: asNonNegativeInteger(
      gasRateValue,
      "call.gasRatePerMinute",
    ),
    metadata: normalizeMetadata(source.metadata),
    scheduled_start_ms:
      scheduledStartValue === undefined || scheduledStartValue === null
        ? null
        : asNonNegativeInteger(
            scheduledStartValue,
            "call.scheduledStartMs",
          ),
    billing_account:
      billingAccountValue === undefined || billingAccountValue === null
        ? null
        : normalizeAccountId(
            billingAccountValue,
            "call.billingAccount",
          ),
    privacy_mode: normalizePrivacyMode(privacyValue),
    room_policy: normalizeRoomPolicy(roomPolicyValue),
    relay_manifest: normalizeKaigiRelayManifest(
      relayManifestValue,
      "call.relayManifest",
    ),
  };

  return call;
}

function normalizeCreateKaigiInput(options) {
  const source = assertPlainObject(options, "createKaigi");
  const callSource =
    source.call && typeof source.call === "object" && !Array.isArray(source.call)
      ? source.call
      : source;
  return {
    call: normalizeNewKaigi(callSource),
    commitment: normalizeKaigiParticipantCommitment(
      source.commitment,
      "createKaigi.commitment",
    ),
    nullifier: normalizeKaigiParticipantNullifier(
      source.nullifier,
      "createKaigi.nullifier",
    ),
    roster_root: normalizeOptionalHash(
      source.roster_root ?? source.rosterRoot,
      "createKaigi.rosterRoot",
    ),
    proof: normalizeOptionalBase64(
      source.proof,
      "createKaigi.proof",
    ),
  };
}

function normalizeJoinOrLeaveInput(type, options) {
  const source = assertPlainObject(options, type);
  const callId = source.call_id ?? source.callId ?? source.id;
  const participant = source.participant ?? source.accountId;
  return {
    call_id: normalizeKaigiId(callId, `${type}.callId`),
    participant: normalizeAccountId(
      participant,
      `${type}.participant`,
    ),
    commitment: normalizeKaigiParticipantCommitment(
      source.commitment,
      `${type}.commitment`,
    ),
    nullifier: normalizeKaigiParticipantNullifier(
      source.nullifier,
      `${type}.nullifier`,
    ),
    roster_root: normalizeOptionalHash(
      source.roster_root ?? source.rosterRoot,
      `${type}.rosterRoot`,
    ),
    proof: normalizeOptionalBase64(source.proof, `${type}.proof`),
  };
}

function normalizeEndKaigiInput(options) {
  const source = assertPlainObject(options, "endKaigi");
  const callId = source.call_id ?? source.callId ?? source.id;
  const endedValue =
    source.ended_at_ms ?? source.endedAtMs ?? source.endedAt ?? null;
  return {
    call_id: normalizeKaigiId(callId, "endKaigi.callId"),
    ended_at_ms:
      endedValue === null || endedValue === undefined
        ? null
        : asNonNegativeInteger(endedValue, "endKaigi.endedAtMs"),
    commitment: normalizeKaigiParticipantCommitment(
      source.commitment,
      "endKaigi.commitment",
    ),
    nullifier: normalizeKaigiParticipantNullifier(
      source.nullifier,
      "endKaigi.nullifier",
    ),
    roster_root: normalizeOptionalHash(
      source.roster_root ?? source.rosterRoot,
      "endKaigi.rosterRoot",
    ),
    proof: normalizeOptionalBase64(source.proof, "endKaigi.proof"),
  };
}

function normalizeKaigiUsageInput(options) {
  const source = assertPlainObject(options, "recordKaigiUsage");
  const callId = source.call_id ?? source.callId ?? source.id;
  return {
    call_id: normalizeKaigiId(callId, "recordKaigiUsage.callId"),
    duration_ms: asPositiveInteger(
      source.duration_ms ?? source.durationMs ?? source.duration,
      "recordKaigiUsage.durationMs",
    ),
    billed_gas: asNonNegativeInteger(
      source.billed_gas ?? source.billedGas ?? source.gas ?? 0,
      "recordKaigiUsage.billedGas",
    ),
    usage_commitment: normalizeOptionalHash(
      source.usage_commitment ?? source.usageCommitment,
      "recordKaigiUsage.usageCommitment",
    ),
    proof: normalizeOptionalBase64(
      source.proof,
      "recordKaigiUsage.proof",
    ),
  };
}

function normalizeSetRelayManifestInput(options) {
  const source = assertPlainObject(options, "setKaigiRelayManifest");
  const callId = source.call_id ?? source.callId ?? source.id;
  return {
    call_id: normalizeKaigiId(callId, "setKaigiRelayManifest.callId"),
    relay_manifest: normalizeKaigiRelayManifest(
      source.relay_manifest ?? source.relayManifest,
      "setKaigiRelayManifest.relayManifest",
    ),
  };
}

function normalizeRegisterRelayInput(options) {
  const source = assertPlainObject(options, "registerKaigiRelay");
  const relay = source.relay ?? source.registration ?? source;
  const relayId = relay.relay_id ?? relay.relayId ?? source.relayId;
  const hpkeKey =
    relay.hpke_public_key ??
    relay.hpkePublicKey ??
    source.hpke_public_key ??
    source.hpkePublicKey;
  const bandwidthValue =
    relay.bandwidth_class ?? relay.bandwidthClass ?? source.bandwidthClass;
  return {
    relay: {
      relay_id: normalizeAccountId(
        relayId,
        "registerKaigiRelay.relayId",
      ),
      hpke_public_key: normalizeBase64(
        hpkeKey,
        "registerKaigiRelay.hpkePublicKey",
      ),
      bandwidth_class: asByte(
        bandwidthValue ?? 0,
        "registerKaigiRelay.bandwidthClass",
      ),
    },
  };
}

function normalizeContractManifest(manifest) {
  const source = assertPlainObject(manifest, "manifest");
  const compilerFingerprint = source.compiler_fingerprint ?? source.compilerFingerprint;
  const featuresBitmap = source.features_bitmap ?? source.featuresBitmap;
  const entrypoints = source.entrypoints ?? source.entryPoints;
  return {
    code_hash: normalizeOptionalHash(
      source.code_hash ?? source.codeHash,
      "manifest.codeHash",
    ),
    abi_hash: normalizeOptionalHash(
      source.abi_hash ?? source.abiHash,
      "manifest.abiHash",
    ),
    compiler_fingerprint:
      compilerFingerprint === undefined || compilerFingerprint === null
        ? null
        : assertString(
            compilerFingerprint,
            "manifest.compilerFingerprint",
          ),
    features_bitmap:
      featuresBitmap === undefined || featuresBitmap === null
        ? null
        : asNonNegativeInteger(
            featuresBitmap,
            "manifest.featuresBitmap",
          ),
    access_set_hints: normalizeAccessSetHints(
      source.access_set_hints ?? source.accessSetHints,
      "manifest.accessSetHints",
    ),
    entrypoints: normalizeEntrypoints(entrypoints, "manifest.entrypoints"),
  };
}

function normalizeEntrypoints(value, name) {
  if (value === undefined || value === null) {
    return null;
  }
  if (!Array.isArray(value)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be an array of entrypoint descriptors`,
      name,
    );
  }
  if (value.length === 0) {
    return [];
  }
  return value.map((entry, index) => normalizeEntrypoint(entry, `${name}[${index}]`));
}

function normalizeEntrypoint(entry, name) {
  const source = assertPlainObject(entry, name);
  const rawName =
    source.name ??
    source.entrypoint ??
    source.entryPoint ??
    source.symbol ??
    source.id;
  const entrypointName = assertString(rawName, `${name}.name`).trim();
  if (!entrypointName) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name}.name must be a non-empty string`,
      `${name}.name`,
    );
  }
  const rawPermission =
    source.permission ?? source.permission_id ?? source.permissionId;
  const permission =
    rawPermission === undefined || rawPermission === null
      ? null
      : assertString(rawPermission, `${name}.permission`).trim();
  const kind = normalizeEntrypointKind(
    source.kind ?? source.type ?? source.variant ?? "Public",
    `${name}.kind`,
  );
  return {
    name: entrypointName,
    kind,
    permission,
  };
}

function normalizeEntrypointKind(value, name) {
  const normalized = String(value ?? "")
    .trim()
    .toLowerCase();
  switch (normalized) {
    case "public":
    case "kotoage":
      return { kind: "Public" };
    case "hajimari":
    case "init":
    case "initializer":
      return { kind: "Hajimari" };
    case "kaizen":
    case "upgrade":
      return { kind: "Kaizen" };
    default:
      fail(
        ValidationErrorCode.INVALID_STRING,
        `${name} must be one of 'Public', 'Hajimari', or 'Kaizen'`,
        name,
      );
  }
}

function normalizeAtWindow(value, name) {
  if (value === undefined || value === null) {
    return null;
  }
  const source = assertPlainObject(value, name);
  const lower = asNonNegativeInteger(
    source.lower ?? source.start ?? source.from,
    `${name}.lower`,
  );
  const upper = asNonNegativeInteger(
    source.upper ?? source.end ?? source.to,
    `${name}.upper`,
  );
  if (upper < lower) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name}.upper must be greater than or equal to lower`,
      `${name}.upper`,
    );
  }
  return { lower, upper };
}

function normalizeVotingMode(value, name) {
  if (value === undefined || value === null) {
    return null;
  }
  const normalized = String(value).trim().toLowerCase();
  if (normalized === "zk" || normalized === "zero-knowledge" || normalized === "zkp") {
    return "Zk";
  }
  if (
    normalized === "plain" ||
    normalized === "plaintext" ||
    normalized === "plain_text" ||
    normalized === "quadratic"
  ) {
    return "Plain";
  }
  fail(ValidationErrorCode.INVALID_STRING, `${name} must be either 'Zk' or 'Plain'`, name);
}

function normalizeJsonPayload(value, name) {
  if (value === null || value === undefined) {
    return "{}";
  }
  let payload = value;
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (!trimmed) {
      fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty string`, name);
    }
    try {
      payload = JSON.parse(trimmed);
    } catch (error) {
      fail(
        ValidationErrorCode.INVALID_JSON_VALUE,
        `${name} must be valid JSON`,
        name,
        error,
      );
    }
  }
  const normalized = normalizeZkBallotPublicInputs(payload, name);
  return canonicalJsonStringify(normalized, name);
}

function canonicalJsonStringify(value, name) {
  return JSON.stringify(canonicalizeJsonValue(value, name, new Set()));
}

function canonicalizeJsonValue(value, name, stack) {
  if (value === null || typeof value === "boolean" || typeof value === "string") {
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      fail(ValidationErrorCode.INVALID_JSON_VALUE, `${name} must not contain non-finite numbers`, name);
    }
    return value;
  }
  if (typeof value === "bigint") {
    fail(ValidationErrorCode.INVALID_JSON_VALUE, `${name} must not contain BigInt values`, name);
  }
  if (typeof value === "function" || typeof value === "symbol") {
    return undefined;
  }
  if (Array.isArray(value)) {
    return value.map((entry) => canonicalizeJsonValue(entry, name, stack));
  }
  if (value && typeof value === "object") {
    if (typeof value.toJSON === "function") {
      return canonicalizeJsonValue(value.toJSON(), name, stack);
    }
    if (stack.has(value)) {
      fail(
        ValidationErrorCode.INVALID_JSON_VALUE,
        `${name} must not contain circular references`,
        name,
      );
    }
    stack.add(value);
    const result = {};
    const keys = Object.keys(value).sort();
    for (const key of keys) {
      const entry = value[key];
      if (entry === undefined || typeof entry === "function" || typeof entry === "symbol") {
        continue;
      }
      result[key] = canonicalizeJsonValue(entry, name, stack);
    }
    stack.delete(value);
    return result;
  }
  return value;
}

function normalizeZkBallotPublicInputs(value, name) {
  const normalized = { ...assertPlainObject(value, name) };
  rejectPublicInputKey(normalized, "durationBlocks", "duration_blocks", name);
  rejectPublicInputKey(normalized, "root_hint_hex", "root_hint", name);
  rejectPublicInputKey(normalized, "rootHintHex", "root_hint", name);
  rejectPublicInputKey(normalized, "rootHint", "root_hint", name);
  rejectPublicInputKey(normalized, "nullifier_hex", "nullifier", name);
  rejectPublicInputKey(normalized, "nullifierHex", "nullifier", name);
  normalizeZkBallotPublicInputHex(normalized, "root_hint", name);
  normalizeZkBallotPublicInputHex(normalized, "nullifier", name);

  const hasOwner = normalized.owner !== undefined && normalized.owner !== null;
  const hasAmount = normalized.amount !== undefined && normalized.amount !== null;
  const hasDuration =
    normalized.duration_blocks !== undefined && normalized.duration_blocks !== null;
  const hasAnyLockHint = hasOwner || hasAmount || hasDuration;
  if (hasAnyLockHint && !(hasOwner && hasAmount && hasDuration)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must include owner, amount, and duration_blocks when providing lock hints`,
      name,
    );
  }
  if (hasOwner) {
    normalized.owner = ensureCanonicalAccountId(normalized.owner, `${name}.owner`);
  }
  return normalized;
}

function normalizeZkBallotPublicInputHex(target, key, name) {
  if (!Object.prototype.hasOwnProperty.call(target, key)) {
    return;
  }
  const value = target[key];
  if (value === null) {
    return;
  }
  if (typeof value !== "string") {
    fail(
      ValidationErrorCode.INVALID_HEX,
      `${name}.${key} must be a 32-byte hex string`,
      name,
    );
  }
  const trimmed = value.trim();
  let body = trimmed;
  if (trimmed.includes(":")) {
    const [scheme, rest] = trimmed.split(":", 2);
    if (scheme && scheme.toLowerCase() !== "blake2b32") {
      fail(
        ValidationErrorCode.INVALID_HEX,
        `${name}.${key} must be a 32-byte hex string`,
        name,
      );
    }
    body = rest.trim();
  }
  if (body.startsWith("0x") || body.startsWith("0X")) {
    body = body.slice(2);
  }
  if (!/^[0-9a-fA-F]{64}$/.test(body)) {
    fail(
      ValidationErrorCode.INVALID_HEX,
      `${name}.${key} must be a 32-byte hex string`,
      name,
    );
  }
  target[key] = body.toLowerCase();
}

function rejectPublicInputKey(target, key, canonicalKey, name) {
  if (!Object.prototype.hasOwnProperty.call(target, key)) {
    return;
  }
  fail(
    ValidationErrorCode.INVALID_OBJECT,
    `${name} must use ${canonicalKey} (unsupported key ${key})`,
    name,
  );
}

function normalizeUintString(value, name) {
  if (typeof value === "bigint") {
    if (value < 0n) {
      fail(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name} must be greater than or equal to zero`,
        name,
      );
    }
    return value.toString(10);
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value) || !Number.isInteger(value) || value < 0) {
      fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be a non-negative integer`, name);
    }
    return Math.trunc(value).toString(10);
  }
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (!/^(?:0|[1-9]\d*)$/.test(trimmed)) {
      fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be a decimal string`, name);
    }
    return trimmed.replace(/^0+(?=\d)/, "") || "0";
  }
  fail(
    ValidationErrorCode.INVALID_NUMERIC,
    `${name} must be a decimal string, number, or bigint`,
    name,
  );
}

function normalizeDirection(value, name) {
  if (value === undefined || value === null) {
    return 0;
  }
  if (typeof value === "number") {
    const byte = asByte(value, name);
    if (byte > 2) {
      fail(ValidationErrorCode.VALUE_OUT_OF_RANGE, `${name} must be between 0 and 2`, name);
    }
    return byte;
  }
  const normalized = String(value).trim().toLowerCase();
  if (normalized === "aye" || normalized === "yes" || normalized === "for") {
    return 0;
  }
  if (normalized === "nay" || normalized === "no" || normalized === "against") {
    return 1;
  }
  if (normalized === "abstain") {
    return 2;
  }
  fail(
    ValidationErrorCode.INVALID_STRING,
    `${name} must be 0, 1, 2 or a recognized direction string`,
    name,
  );
}

function normalizeAccountIds(values, name, { allowEmpty = false } = {}) {
  if (!Array.isArray(values) || (values.length === 0 && !allowEmpty)) {
    fail(ValidationErrorCode.INVALID_OBJECT, `${name} must be a non-empty array`, name);
  }
  if (values.length === 0) {
    return [];
  }
  return values.map((account, index) =>
    normalizeAccountId(account, `${name}[${index}]`),
  );
}

/**
 * Build a `Mint::Asset` instruction payload.
 * @param {{ assetHoldingId: string, quantity: string|number|bigint }} options
 * @returns {{Mint: {Asset: {object: string, destination: string}}}}
 */
export function buildMintAssetInstruction({ assetHoldingId, assetId, quantity }) {
  const destination = normalizeAssetHoldingId(
    assetHoldingId ?? assetId,
    assetHoldingId !== undefined ? "assetHoldingId" : "assetId",
  );
  const object = asNumericQuantity(quantity, "quantity");
  return {
    Mint: {
      Asset: {
        object,
        destination,
      },
    },
  };
}

/**
 * Build a `Burn::Asset` instruction payload.
 * @param {{ assetHoldingId: string, quantity: string|number|bigint }} options
 * @returns {{Burn: {Asset: {object: string, destination: string}}}}
 */
export function buildBurnAssetInstruction({ assetHoldingId, assetId, quantity }) {
  const destination = normalizeAssetHoldingId(
    assetHoldingId ?? assetId,
    assetHoldingId !== undefined ? "assetHoldingId" : "assetId",
  );
  const object = asNumericQuantity(quantity, "quantity");
  return {
    Burn: {
      Asset: {
        object,
        destination,
      },
    },
  };
}

/**
 * Build a `Mint::TriggerRepetitions` instruction payload.
 * @param {{ triggerId: string, repetitions: number|string|bigint }} options
 * @returns {{Mint: {TriggerRepetitions: {object: number, destination: string}}}}
 */
export function buildMintTriggerRepetitionsInstruction({
  triggerId,
  repetitions,
}) {
  const destination = assertString(triggerId, "triggerId");
  const object = asPositiveInteger(repetitions, "repetitions");
  return {
    Mint: {
      TriggerRepetitions: {
        object,
        destination,
      },
    },
  };
}

/**
 * Build a `Burn::TriggerRepetitions` instruction payload.
 * @param {{ triggerId: string, repetitions: number|string|bigint }} options
 * @returns {{Burn: {TriggerRepetitions: {object: number, destination: string}}}}
 */
export function buildBurnTriggerRepetitionsInstruction({
  triggerId,
  repetitions,
}) {
  const destination = assertString(triggerId, "triggerId");
  const object = asPositiveInteger(repetitions, "repetitions");
  return {
    Burn: {
      TriggerRepetitions: {
        object,
        destination,
      },
    },
  };
}

/**
 * Build a `Transfer::Asset` instruction payload.
 * @param {{ sourceAssetHoldingId: string, quantity: string|number|bigint, destinationAccountId: string }} options
 * @returns {{Transfer: {Asset: {source: string, object: string, destination: string}}}}
 */
export function buildTransferAssetInstruction({
  sourceAssetHoldingId,
  sourceAssetId,
  quantity,
  destinationAccountId,
}) {
  const source = normalizeAssetHoldingId(
    sourceAssetHoldingId ?? sourceAssetId,
    sourceAssetHoldingId !== undefined ? "sourceAssetHoldingId" : "sourceAssetId",
  );
  const destination = normalizeAccountId(
    destinationAccountId,
    "destinationAccountId",
  );
  const object = asNumericQuantity(quantity, "quantity");
  return {
    Transfer: {
      Asset: {
        source,
        object,
        destination,
      },
    },
  };
}

/**
 * Build a `Transfer::Domain` instruction payload.
 * @param {{ sourceAccountId: string, domainId: string, destinationAccountId: string }} options
 * @returns {{Transfer: {Domain: {source: string, object: string, destination: string}}}}
 */
export function buildTransferDomainInstruction({
  sourceAccountId,
  domainId,
  destinationAccountId,
}) {
  const source = normalizeAccountId(sourceAccountId, "sourceAccountId");
  const object = assertString(domainId, "domainId");
  const destination = normalizeAccountId(
    destinationAccountId,
    "destinationAccountId",
  );
  return {
    Transfer: {
      Domain: {
        source,
        object,
        destination,
      },
    },
  };
}

/**
 * Build a `Transfer::AssetDefinition` instruction payload.
 * @param {{ sourceAccountId: string, assetDefinitionId: string, destinationAccountId: string }} options
 * @returns {{Transfer: {AssetDefinition: {source: string, object: string, destination: string}}}}
 */
export function buildTransferAssetDefinitionInstruction({
  sourceAccountId,
  assetDefinitionId,
  destinationAccountId,
}) {
  const source = normalizeAccountId(sourceAccountId, "sourceAccountId");
  const object = assertString(assetDefinitionId, "assetDefinitionId");
  const destination = normalizeAccountId(
    destinationAccountId,
    "destinationAccountId",
  );
  return {
    Transfer: {
      AssetDefinition: {
        source,
        object,
        destination,
      },
    },
  };
}

/**
 * Build a `Transfer::Nft` instruction payload.
 * @param {{ sourceAccountId: string, nftId: string, destinationAccountId: string }} options
 * @returns {{Transfer: {Nft: {source: string, object: string, destination: string}}}}
 */
export function buildTransferNftInstruction({
  sourceAccountId,
  nftId,
  destinationAccountId,
}) {
  const source = normalizeAccountId(sourceAccountId, "sourceAccountId");
  const object = assertString(nftId, "nftId");
  const destination = normalizeAccountId(
    destinationAccountId,
    "destinationAccountId",
  );
  return {
    Transfer: {
      Nft: {
        source,
        object,
        destination,
      },
    },
  };
}

/**
 * Build a `RegisterRwa` instruction payload.
 * @param {object} options
 * @returns {{RegisterRwa: {rwa: object}}}
 */
export function buildRegisterRwaInstruction(options) {
  const source = assertPlainObject(options, "registerRwa");
  return {
    RegisterRwa: {
      rwa: normalizeRegisterRwaPayload(source.rwa ?? source.rwaJson ?? source, "registerRwa.rwa"),
    },
  };
}

/**
 * Build a `TransferRwa` instruction payload.
 * @param {{ sourceAccountId: string, rwaId: string, quantity: string|number|bigint, destinationAccountId: string }} options
 * @returns {{TransferRwa: {source: string, rwa: string, quantity: string, destination: string}}}
 */
export function buildTransferRwaInstruction({
  sourceAccountId,
  rwaId,
  quantity,
  destinationAccountId,
}) {
  return {
    TransferRwa: {
      source: normalizeAccountId(sourceAccountId, "sourceAccountId"),
      rwa: normalizeRwaId(rwaId, "rwaId"),
      quantity: asNumericQuantity(quantity, "quantity"),
      destination: normalizeAccountId(destinationAccountId, "destinationAccountId"),
    },
  };
}

/**
 * Build a `MergeRwas` instruction payload.
 * @param {object} options
 * @returns {{MergeRwas: object}}
 */
export function buildMergeRwasInstruction(options) {
  const source = assertPlainObject(options, "mergeRwas");
  return {
    MergeRwas: normalizeMergeRwasPayload(
      source.merge ?? source.mergeJson ?? source,
      "mergeRwas.merge",
    ),
  };
}

/**
 * Build a `RedeemRwa` instruction payload.
 * @param {{ rwaId: string, quantity: string|number|bigint }} options
 * @returns {{RedeemRwa: {rwa: string, quantity: string}}}
 */
export function buildRedeemRwaInstruction({ rwaId, quantity }) {
  return {
    RedeemRwa: {
      rwa: normalizeRwaId(rwaId, "rwaId"),
      quantity: asNumericQuantity(quantity, "quantity"),
    },
  };
}

/**
 * Build a `FreezeRwa` instruction payload.
 * @param {{ rwaId: string }} options
 * @returns {{FreezeRwa: {rwa: string}}}
 */
export function buildFreezeRwaInstruction({ rwaId }) {
  return {
    FreezeRwa: {
      rwa: normalizeRwaId(rwaId, "rwaId"),
    },
  };
}

/**
 * Build an `UnfreezeRwa` instruction payload.
 * @param {{ rwaId: string }} options
 * @returns {{UnfreezeRwa: {rwa: string}}}
 */
export function buildUnfreezeRwaInstruction({ rwaId }) {
  return {
    UnfreezeRwa: {
      rwa: normalizeRwaId(rwaId, "rwaId"),
    },
  };
}

/**
 * Build a `HoldRwa` instruction payload.
 * @param {{ rwaId: string, quantity: string|number|bigint }} options
 * @returns {{HoldRwa: {rwa: string, quantity: string}}}
 */
export function buildHoldRwaInstruction({ rwaId, quantity }) {
  return {
    HoldRwa: {
      rwa: normalizeRwaId(rwaId, "rwaId"),
      quantity: asNumericQuantity(quantity, "quantity"),
    },
  };
}

/**
 * Build a `ReleaseRwa` instruction payload.
 * @param {{ rwaId: string, quantity: string|number|bigint }} options
 * @returns {{ReleaseRwa: {rwa: string, quantity: string}}}
 */
export function buildReleaseRwaInstruction({ rwaId, quantity }) {
  return {
    ReleaseRwa: {
      rwa: normalizeRwaId(rwaId, "rwaId"),
      quantity: asNumericQuantity(quantity, "quantity"),
    },
  };
}

/**
 * Build a `ForceTransferRwa` instruction payload.
 * @param {{ rwaId: string, quantity: string|number|bigint, destinationAccountId: string }} options
 * @returns {{ForceTransferRwa: {rwa: string, quantity: string, destination: string}}}
 */
export function buildForceTransferRwaInstruction({
  rwaId,
  quantity,
  destinationAccountId,
}) {
  return {
    ForceTransferRwa: {
      rwa: normalizeRwaId(rwaId, "rwaId"),
      quantity: asNumericQuantity(quantity, "quantity"),
      destination: normalizeAccountId(destinationAccountId, "destinationAccountId"),
    },
  };
}

/**
 * Build a `SetRwaControls` instruction payload.
 * @param {{ rwaId: string, controls?: object|string, controlsJson?: string }} options
 * @returns {{SetRwaControls: {rwa: string, controls: object}}}
 */
export function buildSetRwaControlsInstruction(options) {
  const source = assertPlainObject(options, "setRwaControls");
  return {
    SetRwaControls: {
      rwa: normalizeRwaId(source.rwaId, "rwaId"),
      controls: normalizeRwaControlPolicy(
        source.controls ?? source.controlsJson,
        "setRwaControls.controls",
      ),
    },
  };
}

/**
 * Build a `SetRwaKeyValue` instruction payload.
 * @param {{ rwaId: string, key: string, value: unknown }} options
 * @returns {{SetRwaKeyValue: {rwa: string, key: string, value: unknown}}}
 */
export function buildSetRwaKeyValueInstruction({ rwaId, key, value }) {
  return {
    SetRwaKeyValue: {
      rwa: normalizeRwaId(rwaId, "rwaId"),
      key: assertString(key, "key"),
      value: normalizeJsonValue(value, "value"),
    },
  };
}

/**
 * Build a `RemoveRwaKeyValue` instruction payload.
 * @param {{ rwaId: string, key: string }} options
 * @returns {{RemoveRwaKeyValue: {rwa: string, key: string}}}
 */
export function buildRemoveRwaKeyValueInstruction({ rwaId, key }) {
  return {
    RemoveRwaKeyValue: {
      rwa: normalizeRwaId(rwaId, "rwaId"),
      key: assertString(key, "key"),
    },
  };
}

/**
 * Build a `Register::Domain` instruction payload.
 * @param {{ domainId: string, logo?: string | null, metadata?: object | null }} options
 * @returns {{Register: {Domain: {id: string, logo: string | null, metadata: object}}}}
 */
export function buildRegisterDomainInstruction({ domainId, logo = null, metadata }) {
  const id = assertString(domainId, "domainId");
  const normalizedLogo =
    logo === null || logo === undefined ? null : assertString(logo, "logo");
  const normalizedMetadata = normalizeMetadata(metadata);
  return {
    Register: {
      Domain: {
        id,
        logo: normalizedLogo,
        metadata: normalizedMetadata,
      },
    },
  };
}

/**
 * Build a `Register::Account` instruction payload.
 * @param {{ accountId: string, domainId?: string, domain?: string, metadata?: object | null }} options
 * @returns {{Register: {Account: {id: string, domain: string, metadata: object}}}}
 */
export function buildRegisterAccountInstruction({
  accountId,
  domainId,
  domain,
  metadata,
}) {
  const id = normalizeAccountId(accountId, "accountId");
  const resolvedDomainId = assertString(domainId ?? domain, "domainId");
  const normalizedMetadata = normalizeMetadata(metadata);
  return {
    Register: {
      Account: {
        id,
        domain: resolvedDomainId,
        metadata: normalizedMetadata,
      },
    },
  };
}

/**
 * Build a multisig registration instruction payload.
 * @param {{ accountId: string, spec: MultisigSpec | object }} options
 * @returns {{Custom: {payload: {Register: {account: string, spec: object}}}}}
 */
export function buildRegisterMultisigInstruction({ accountId, spec }) {
  const controller = normalizeAccountId(accountId, "accountId");
  const normalizedSpec = normalizeMultisigSpecPayload(spec, "spec");
  return {
    Custom: {
      payload: {
        Register: {
          account: controller,
          spec: normalizedSpec,
        },
      },
    },
  };
}

/**
 * Build a multisig proposal instruction payload with TTL enforcement against the policy cap.
 * @param {{ accountId: string, instructions: any[], spec: MultisigSpec | object, transactionTtlMs?: number }} options
 * @returns {{Custom: {payload: {Propose: {account: string, instructions: any[], transaction_ttl_ms?: number}}}}}
 */
export function buildProposeMultisigInstruction({
  accountId,
  instructions,
  spec,
  transactionTtlMs,
}) {
  const controller = normalizeAccountId(accountId, "accountId");
  if (!Array.isArray(instructions) || instructions.length === 0) {
    throw new TypeError("instructions must be a non-empty array");
  }
  const normalizedSpec = normalizeMultisigSpecPayload(spec, "spec");

  const policyCap = normalizedSpec.transaction_ttl_ms;
  if (policyCap === undefined || policyCap === null) {
    throw new Error("spec.transaction_ttl_ms is required to enforce the policy TTL cap");
  }
  if (
    transactionTtlMs !== undefined &&
    transactionTtlMs !== null &&
    Number(transactionTtlMs) > Number(policyCap)
  ) {
    throw new RangeError(
      `Requested multisig TTL ${transactionTtlMs} ms exceeds the policy cap ${policyCap} ms; choose a value at or below the cap.`,
    );
  }

  const payload = {
    account: controller,
    instructions,
  };
  if (transactionTtlMs !== undefined && transactionTtlMs !== null) {
    payload.transaction_ttl_ms = transactionTtlMs;
  }

  return {
    Custom: {
      payload: {
        Propose: payload,
      },
    },
  };
}

/**
 * Build a `Kaigi::CreateKaigi` instruction payload.
 * @param {object} call
 * @returns {{Kaigi: {CreateKaigi: {call: object}}}}
 */
export function buildCreateKaigiInstruction(call) {
  const normalizedCall = normalizeCreateKaigiInput(call);
  return {
    Kaigi: {
      CreateKaigi: normalizedCall,
    },
  };
}

/**
 * Build a `Kaigi::JoinKaigi` instruction payload.
 * @param {object} options
 * @returns {{Kaigi: {JoinKaigi: object}}}
 */
export function buildJoinKaigiInstruction(options) {
  const normalized = normalizeJoinOrLeaveInput("joinKaigi", options);
  return {
    Kaigi: {
      JoinKaigi: normalized,
    },
  };
}

/**
 * Build a `Kaigi::LeaveKaigi` instruction payload.
 * @param {object} options
 * @returns {{Kaigi: {LeaveKaigi: object}}}
 */
export function buildLeaveKaigiInstruction(options) {
  const normalized = normalizeJoinOrLeaveInput("leaveKaigi", options);
  return {
    Kaigi: {
      LeaveKaigi: normalized,
    },
  };
}

/**
 * Build a `Kaigi::EndKaigi` instruction payload.
 * @param {object} options
 * @returns {{Kaigi: {EndKaigi: object}}}
 */
export function buildEndKaigiInstruction(options) {
  const normalized = normalizeEndKaigiInput(options);
  return {
    Kaigi: {
      EndKaigi: normalized,
    },
  };
}

/**
 * Build a `Kaigi::RecordKaigiUsage` instruction payload.
 * @param {object} options
 * @returns {{Kaigi: {RecordKaigiUsage: object}}}
 */
export function buildRecordKaigiUsageInstruction(options) {
  const normalized = normalizeKaigiUsageInput(options);
  return {
    Kaigi: {
      RecordKaigiUsage: normalized,
    },
  };
}

/**
 * Build a `Kaigi::SetKaigiRelayManifest` instruction payload.
 * @param {object} options
 * @returns {{Kaigi: {SetKaigiRelayManifest: object}}}
 */
export function buildSetKaigiRelayManifestInstruction(options) {
  const normalized = normalizeSetRelayManifestInput(options);
  return {
    Kaigi: {
      SetKaigiRelayManifest: normalized,
    },
  };
}

/**
 * Build a `Kaigi::RegisterKaigiRelay` instruction payload.
 * @param {object} options
 * @returns {{Kaigi: {RegisterKaigiRelay: {relay: object}}}}
 */
export function buildRegisterKaigiRelayInstruction(options) {
  const normalized = normalizeRegisterRelayInput(options);
  return {
    Kaigi: {
      RegisterKaigiRelay: normalized,
    },
  };
}

/**
 * Build a `ProposeDeployContract` instruction payload.
 * @param {object} options
 * @returns {{ProposeDeployContract: object}}
 */
export function buildProposeDeployContractInstruction(options) {
  const source = assertPlainObject(options, "proposeDeployContract");
  const payload = {
    namespace: assertString(source.namespace, "namespace"),
    contract_id: assertString(
      source.contractId ?? source.contract_id,
      "contractId",
    ),
    code_hash_hex: normalizeHexHashString(
      source.codeHash ?? source.code_hash ?? source.codeHashHex,
      "codeHash",
    ),
    abi_hash_hex: normalizeHexHashString(
      source.abiHash ?? source.abi_hash ?? source.abiHashHex,
      "abiHash",
    ),
    abi_version: assertString(
      source.abiVersion ?? source.abi_version ?? "1",
      "abiVersion",
    ),
    window: normalizeAtWindow(source.window, "window"),
    mode: normalizeVotingMode(source.votingMode ?? source.mode, "votingMode"),
  };
  return { ProposeDeployContract: payload };
}

/**
 * Build a `CastZkBallot` instruction payload.
 * @param {object} options
 * @returns {{CastZkBallot: object}}
 */
export function buildCastZkBallotInstruction(options) {
  const source = assertPlainObject(options, "castZkBallot");
  const proofValue = source.proof ?? source.proofB64 ?? source.proof_b64;
  const publicInputs =
    source.publicInputs ?? source.publicInputsJson ?? source.public_inputs_json;
  return {
    CastZkBallot: {
      election_id: assertString(
        source.electionId ?? source.election_id,
        "electionId",
      ),
      proof_b64: normalizeBase64(proofValue, "proof"),
      public_inputs_json: normalizeJsonPayload(publicInputs, "publicInputs"),
    },
  };
}

/**
 * Build a `CastPlainBallot` instruction payload.
 * @param {object} options
 * @returns {{CastPlainBallot: object}}
 */
export function buildCastPlainBallotInstruction(options) {
  const source = assertPlainObject(options, "castPlainBallot");
  return {
    CastPlainBallot: {
      referendum_id: assertString(
        source.referendumId ?? source.referendum_id,
        "referendumId",
      ),
      owner: normalizeAccountId(source.owner, "owner"),
      amount: normalizeUintString(source.amount, "amount"),
      duration_blocks: asNonNegativeInteger(
        source.durationBlocks ?? source.duration_blocks,
        "durationBlocks",
      ),
      direction: normalizeDirection(source.direction, "direction"),
    },
  };
}

/**
 * Build an `EnactReferendum` instruction payload.
 * @param {object} options
 * @returns {{EnactReferendum: object}}
 */
export function buildEnactReferendumInstruction(options) {
  const source = assertPlainObject(options, "enactReferendum");
  const window =
    normalizeAtWindow(source.window ?? source.atWindow, "window") ?? {
      lower: 0,
      upper: 0,
    };
  const referendumId = normalizeFixedBytes(
    source.referendumId ?? source.referendum_id,
    "enactReferendum.referendumId",
  );
  const preimageHash = normalizeFixedBytes(
    source.preimageHash ?? source.preimage_hash,
    "enactReferendum.preimageHash",
  );
  return {
    EnactReferendum: {
      referendum_id: referendumId,
      preimage_hash: preimageHash,
      at_window: window,
    },
  };
}

/**
 * Build a `FinalizeReferendum` instruction payload.
 * @param {object} options
 * @returns {{FinalizeReferendum: object}}
 */
export function buildFinalizeReferendumInstruction(options) {
  const source = assertPlainObject(options, "finalizeReferendum");
  return {
    FinalizeReferendum: {
      referendum_id: assertString(
        source.referendumId ?? source.referendum_id,
        "referendumId",
      ),
      proposal_id: normalizeFixedBytes(
        source.proposalId ?? source.proposal_id,
        "finalizeReferendum.proposalId",
      ),
    },
  };
}

/**
 * Build a `ClaimTwitterFollowReward` instruction payload.
 * @param {{ bindingHash: object }} options
 * @returns {{ClaimTwitterFollowReward: { binding_hash: object }}}
 */
export function buildClaimTwitterFollowRewardInstruction(options) {
  const source = assertPlainObject(options, "claimTwitterFollowReward");
  const binding =
    source.binding_hash ??
    source.bindingHash ??
    source.binding ??
    source.hash;
  return {
    ClaimTwitterFollowReward: {
      binding_hash: normalizeKeyedHashInput(
        binding,
        "claimTwitterFollowReward.bindingHash",
      ),
    },
  };
}

/**
 * Build a `SendToTwitter` instruction payload.
 * @param {{ bindingHash: object, amount: string|number|bigint }} options
 * @returns {{SendToTwitter: { binding_hash: object, amount: string }}}
 */
export function buildSendToTwitterInstruction(options) {
  const source = assertPlainObject(options, "sendToTwitter");
  const binding =
    source.binding_hash ??
    source.bindingHash ??
    source.binding ??
    source.hash;
  const amountValue = source.amount ?? source.quantity;
  return {
    SendToTwitter: {
      binding_hash: normalizeKeyedHashInput(
        binding,
        "sendToTwitter.bindingHash",
      ),
      amount: asNumericQuantity(amountValue, "sendToTwitter.amount"),
    },
  };
}

/**
 * Build a `CancelTwitterEscrow` instruction payload.
 * @param {{ bindingHash: object }} options
 * @returns {{CancelTwitterEscrow: { binding_hash: object }}}
 */
export function buildCancelTwitterEscrowInstruction(options) {
  const source = assertPlainObject(options, "cancelTwitterEscrow");
  const binding =
    source.binding_hash ??
    source.bindingHash ??
    source.binding ??
    source.hash;
  return {
    CancelTwitterEscrow: {
      binding_hash: normalizeKeyedHashInput(
        binding,
        "cancelTwitterEscrow.bindingHash",
      ),
    },
  };
}

/**
 * Build a `PersistCouncilForEpoch` instruction payload.
 * @param {object} options
 * @returns {{PersistCouncilForEpoch: object}}
 */
export function buildPersistCouncilForEpochInstruction(options) {
  const source = assertPlainObject(options, "persistCouncilForEpoch");
  const derivedBy = source.derivedBy ?? source.derived_by ?? "Vrf";
  const normalizedDerived =
    String(derivedBy).trim().toLowerCase() === "fallback" ? "Fallback" : "Vrf";
  return {
    PersistCouncilForEpoch: {
      epoch: asNonNegativeInteger(source.epoch, "epoch"),
      members: normalizeAccountIds(
        source.members ?? source.council,
        "members",
      ),
      alternates: normalizeAccountIds(
        source.alternates ?? [],
        "alternates",
        { allowEmpty: true },
      ),
      verified: asNonNegativeInteger(
        source.verified ?? 0,
        "verified",
      ),
      candidates_count: asNonNegativeInteger(
        source.candidatesCount ?? source.candidates_count,
        "candidatesCount",
      ),
      derived_by: normalizedDerived,
    },
  };
}

/**
 * Build a `RegisterSmartContractCode` instruction payload.
 * @param {{manifest: object}} options
 * @returns {{RegisterSmartContractCode: {manifest: object}}}
 */
export function buildRegisterSmartContractCodeInstruction(options) {
  if (!options || typeof options !== "object") {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "buildRegisterSmartContractCodeInstruction options must be an object",
    );
  }
  const manifest =
    options.manifest ??
    options.RegisterSmartContractCode?.manifest ??
    options.registerSmartContractCode?.manifest;
  const normalized = normalizeContractManifest(manifest);
  return {
    RegisterSmartContractCode: {
      manifest: normalized,
    },
  };
}

/**
 * Build a `RegisterSmartContractBytes` instruction payload.
 * @param {{codeHash: string|Buffer, code: ArrayBufferView|ArrayBuffer|Buffer|string}} options
 * @returns {{RegisterSmartContractBytes: {code_hash: string, code: string}}}
 */
export function buildRegisterSmartContractBytesInstruction(options) {
  if (!options || typeof options !== "object") {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "buildRegisterSmartContractBytesInstruction options must be an object",
    );
  }
  const code = normalizeBase64(options.code, "registerSmartContractBytes.code");
  if (code.length === 0) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      "registerSmartContractBytes.code must be a non-empty base64 string",
      "registerSmartContractBytes.code",
    );
  }
  return {
    RegisterSmartContractBytes: {
      code_hash: normalizeHash(
        options.codeHash ?? options.code_hash,
        "registerSmartContractBytes.codeHash",
      ),
      code,
    },
  };
}

/**
 * Build a `DeactivateContractInstance` instruction payload.
 * @param {{namespace: string, contractId: string, reason?: string | null}} options
 * @returns {{DeactivateContractInstance: {namespace: string, contract_id: string, reason?: string}}}
 */
export function buildDeactivateContractInstanceInstruction(options) {
  const source = assertPlainObject(options, "deactivateContractInstance");
  const payload = {
    namespace: assertString(
      source.namespace,
      "deactivateContractInstance.namespace",
    ),
    contract_id: assertString(
      source.contractId ?? source.contract_id ?? source.contractID,
      "deactivateContractInstance.contractId",
    ),
  };
  const reason = source.reason ?? source.reasonText ?? source.reason_text;
  if (reason !== undefined && reason !== null) {
    payload.reason = assertString(
      reason,
      "deactivateContractInstance.reason",
    );
  }
  return {
    DeactivateContractInstance: payload,
  };
}

/**
 * Build an `ActivateContractInstance` instruction payload.
 * @param {{namespace: string, contractId: string, codeHash: string|Buffer}} options
 * @returns {{ActivateContractInstance: {namespace: string, contract_id: string, code_hash: string}}}
 */
export function buildActivateContractInstanceInstruction(options) {
  if (!options || typeof options !== "object") {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "buildActivateContractInstanceInstruction options must be an object",
    );
  }
  const namespace = assertString(options.namespace, "activateContractInstance.namespace");
  const contractId =
    options.contract_id ?? options.contractId ?? options.contractID ?? options.id;
  return {
    ActivateContractInstance: {
      namespace,
      contract_id: assertString(
        contractId,
        "activateContractInstance.contractId",
      ),
      code_hash: normalizeHash(
        options.codeHash ?? options.code_hash,
        "activateContractInstance.codeHash",
      ),
    },
  };
}

/**
 * Build a `RemoveSmartContractBytes` instruction payload.
 * @param {{codeHash: string | Buffer, reason?: string | null}} options
 * @returns {{RemoveSmartContractBytes: {code_hash: string, reason?: string}}}
 */
export function buildRemoveSmartContractBytesInstruction(options) {
  const source = assertPlainObject(options, "removeSmartContractBytes");
  const payload = {
    code_hash: normalizeHash(
      source.codeHash ?? source.code_hash,
      "removeSmartContractBytes.codeHash",
    ),
  };
  const reason = source.reason ?? source.reasonText ?? source.reason_text;
  if (reason !== undefined && reason !== null) {
    payload.reason = assertString(
      reason,
      "removeSmartContractBytes.reason",
    );
  }
  return {
    RemoveSmartContractBytes: payload,
  };
}

/**
 * Build a `zk::RegisterZkAsset` instruction payload.
 * @param {object} options
 * @returns {{zk: {RegisterZkAsset: object}}}
 */
export function buildRegisterZkAssetInstruction(options) {
  const source = assertPlainObject(options, "registerZkAsset");
  const asset =
    source.assetDefinitionId ??
    source.asset_definition_id ??
    source.asset ??
    source.definitionId;
  const payload = {
    asset: assertString(asset, "registerZkAsset.asset"),
    mode: normalizeZkAssetMode(source.mode ?? source.assetMode, "registerZkAsset.mode"),
    allow_shield: Boolean(source.allowShield ?? source.allow_shield ?? true),
    allow_unshield: Boolean(source.allowUnshield ?? source.allow_unshield ?? true),
    vk_transfer: normalizeVerifyingKeyId(
      source.transferVerifyingKey ?? source.vkTransfer ?? source.vk_transfer,
      "registerZkAsset.vkTransfer",
    ),
    vk_unshield: normalizeVerifyingKeyId(
      source.unshieldVerifyingKey ?? source.vkUnshield ?? source.vk_unshield,
      "registerZkAsset.vkUnshield",
    ),
    vk_shield: normalizeVerifyingKeyId(
      source.shieldVerifyingKey ?? source.vkShield ?? source.vk_shield,
      "registerZkAsset.vkShield",
    ),
  };
  return {
    zk: {
      RegisterZkAsset: payload,
    },
  };
}

/**
 * Build a `zk::ScheduleConfidentialPolicyTransition` instruction payload.
 * @param {object} options
 * @returns {{zk: {ScheduleConfidentialPolicyTransition: object}}}
 */
export function buildScheduleConfidentialPolicyTransitionInstruction(options) {
  const source = assertPlainObject(options, "scheduleConfidentialPolicyTransition");
  const asset =
    source.assetDefinitionId ??
    source.asset_definition_id ??
    source.asset ??
    source.definitionId;
  const conversionWindow =
    source.conversionWindow ?? source.conversion_window ?? source.window;
  const payload = {
    asset: assertString(asset, "scheduleConfidentialPolicyTransition.asset"),
    new_mode: normalizeConfidentialPolicyMode(
      source.newMode ?? source.mode ?? source.new_mode,
      "scheduleConfidentialPolicyTransition.newMode",
    ),
    effective_height: asNonNegativeInteger(
      source.effectiveHeight ?? source.effective_height,
      "scheduleConfidentialPolicyTransition.effectiveHeight",
    ),
    transition_id: normalizeHash(
      source.transitionId ?? source.transition_id,
      "scheduleConfidentialPolicyTransition.transitionId",
    ),
    conversion_window:
      conversionWindow === undefined || conversionWindow === null
        ? null
        : asNonNegativeInteger(
            conversionWindow,
            "scheduleConfidentialPolicyTransition.conversionWindow",
          ),
  };
  return {
    zk: {
      ScheduleConfidentialPolicyTransition: payload,
    },
  };
}

/**
 * Build a `zk::CancelConfidentialPolicyTransition` instruction payload.
 * @param {object} options
 * @returns {{zk: {CancelConfidentialPolicyTransition: object}}}
 */
export function buildCancelConfidentialPolicyTransitionInstruction(options) {
  const source = assertPlainObject(options, "cancelConfidentialPolicyTransition");
  const asset =
    source.assetDefinitionId ??
    source.asset_definition_id ??
    source.asset ??
    source.definitionId;
  return {
    zk: {
      CancelConfidentialPolicyTransition: {
        asset: assertString(asset, "cancelConfidentialPolicyTransition.asset"),
        transition_id: normalizeHash(
          source.transitionId ?? source.transition_id,
          "cancelConfidentialPolicyTransition.transitionId",
        ),
      },
    },
  };
}

/**
 * Build a `zk::Shield` instruction payload.
 * @param {object} options
 * @returns {{zk: {Shield: object}}}
 */
export function buildShieldInstruction(options) {
  const source = assertPlainObject(options, "shield");
  const payload = {
    asset: assertString(
      source.assetDefinitionId ?? source.asset_definition_id ?? source.asset,
      "shield.asset",
    ),
    from: normalizeAccountId(source.fromAccountId ?? source.from, "shield.from"),
    amount: asU128JsonNumber(source.amount, "shield.amount"),
    note_commitment: normalizeFixedBytes(source.noteCommitment ?? source.note_commitment, "shield.noteCommitment", 32),
    enc_payload: normalizeConfidentialEncryptedPayload(
      source.encPayload ?? source.enc_payload ?? source.encryptedPayload,
      "shield.encPayload",
    ),
  };
  return {
    zk: {
      Shield: payload,
    },
  };
}

/**
 * Build a `zk::ZkTransfer` instruction payload.
 * @param {object} options
 * @returns {{zk: {ZkTransfer: object}}}
 */
export function buildZkTransferInstruction(options) {
  const source = assertPlainObject(options, "zkTransfer");
  const inputs = Array.isArray(source.inputs)
    ? source.inputs.map((entry, index) => normalizeFixedBytes(entry, `zkTransfer.inputs[${index}]`, 32))
    : [];
  const outputs = Array.isArray(source.outputs)
    ? source.outputs.map((entry, index) => normalizeFixedBytes(entry, `zkTransfer.outputs[${index}]`, 32))
    : [];
  if (inputs.length === 0) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "zkTransfer.inputs must contain at least one nullifier",
    );
  }
  if (outputs.length === 0) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "zkTransfer.outputs must contain at least one commitment",
    );
  }
  const payload = {
    asset: assertString(
      source.assetDefinitionId ?? source.asset_definition_id ?? source.asset,
      "zkTransfer.asset",
    ),
    inputs,
    outputs,
    proof: normalizeProofAttachment(source.proof, "zkTransfer.proof"),
    root_hint: normalizeOptionalFixedBytes(source.rootHint ?? source.root_hint, "zkTransfer.rootHint"),
  };
  return {
    zk: {
      ZkTransfer: payload,
    },
  };
}

/**
 * Build a `zk::Unshield` instruction payload.
 * @param {object} options
 * @returns {{zk: {Unshield: object}}}
 */
export function buildUnshieldInstruction(options) {
  const source = assertPlainObject(options, "unshield");
  const inputs = Array.isArray(source.inputs)
    ? source.inputs.map((entry, index) => normalizeFixedBytes(entry, `unshield.inputs[${index}]`, 32))
    : [];
  if (inputs.length === 0) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "unshield.inputs must contain at least one nullifier",
    );
  }
  const payload = {
    asset: assertString(
      source.assetDefinitionId ?? source.asset_definition_id ?? source.asset,
      "unshield.asset",
    ),
    to: normalizeAccountId(source.toAccountId ?? source.to ?? source.destinationAccountId, "unshield.to"),
    public_amount: asU128JsonNumber(source.publicAmount ?? source.public_amount, "unshield.publicAmount"),
    inputs,
    proof: normalizeProofAttachment(source.proof, "unshield.proof"),
    root_hint: normalizeOptionalFixedBytes(source.rootHint ?? source.root_hint, "unshield.rootHint"),
  };
  return {
    zk: {
      Unshield: payload,
    },
  };
}

/**
 * Build a `zk::CreateElection` instruction payload.
 * @param {object} options
 * @returns {{zk: {CreateElection: object}}}
 */
export function buildCreateElectionInstruction(options) {
  const source = assertPlainObject(options, "createElection");
  const payload = {
    election_id: assertString(source.electionId ?? source.election_id, "createElection.electionId"),
    options: asPositiveInteger(source.options, "createElection.options"),
    eligible_root: normalizeFixedBytes(source.eligibleRoot ?? source.eligible_root, "createElection.eligibleRoot", 32),
    start_ts: asNonNegativeInteger(source.startTs ?? source.start_ts ?? source.startTimestampMs, "createElection.startTs"),
    end_ts: asNonNegativeInteger(source.endTs ?? source.end_ts ?? source.endTimestampMs, "createElection.endTs"),
    vk_ballot: normalizeVerifyingKeyId(source.vkBallot ?? source.ballotVerifyingKey, "createElection.vkBallot"),
    vk_tally: normalizeVerifyingKeyId(source.vkTally ?? source.tallyVerifyingKey, "createElection.vkTally"),
    domain_tag: assertString(source.domainTag ?? source.domain_tag ?? "zk", "createElection.domainTag"),
  };
  return {
    zk: {
      CreateElection: payload,
    },
  };
}

/**
 * Build a `zk::SubmitBallot` instruction payload.
 * @param {object} options
 * @returns {{zk: {SubmitBallot: object}}}
 */
export function buildSubmitBallotInstruction(options) {
  const source = assertPlainObject(options, "submitBallot");
  const payload = {
    election_id: assertString(source.electionId ?? source.election_id, "submitBallot.electionId"),
    ciphertext: normalizeByteArray(
      source.ciphertext ?? source.ciphertextBytes ?? source.ciphertext_b64 ?? source.ciphertextB64,
      "submitBallot.ciphertext",
    ),
    ballot_proof: normalizeProofAttachment(
      source.ballotProof ?? source.proof ?? source.ballot_proof,
      "submitBallot.ballotProof",
    ),
    nullifier: normalizeFixedBytes(source.nullifier, "submitBallot.nullifier", 32),
  };
  return {
    zk: {
      SubmitBallot: payload,
    },
  };
}

/**
 * Build a `zk::FinalizeElection` instruction payload.
 * @param {object} options
 * @returns {{zk: {FinalizeElection: object}}}
 */
export function buildFinalizeElectionInstruction(options) {
  const source = assertPlainObject(options, "finalizeElection");
  const tallyInput = Array.isArray(source.tally) ? source.tally : [];
  if (tallyInput.length === 0) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "finalizeElection.tally must contain at least one entry",
    );
  }
  const payload = {
    election_id: assertString(source.electionId ?? source.election_id, "finalizeElection.electionId"),
    tally: tallyInput.map((entry, index) =>
      asNonNegativeInteger(entry, `finalizeElection.tally[${index}]`),
    ),
    tally_proof: normalizeProofAttachment(source.tallyProof ?? source.proof ?? source.tally_proof, "finalizeElection.tallyProof"),
  };
  return {
    zk: {
      FinalizeElection: payload,
    },
  };
}

export { normalizeAccountId, normalizeAssetId, normalizeAssetHoldingId, normalizeRwaId };

/**
 * Helper that encodes a builder result to ensure structural validity.
 * Mostly used by tests; exposed for convenience.
 * @param {object} instruction
 * @returns {Buffer}
 */
export function encodeInstruction(instruction) {
  return noritoEncodeInstruction(instruction);
}
