import { Buffer } from "node:buffer";
import { createHash } from "node:crypto";
import {
  noritoEncodeInstruction,
  noritoDecodePrivacyProofEnvelope,
  noritoEncodePrivacyProofEnvelope,
} from "./norito.js";
import {
  canonicalizeMultihashHex,
  ensureCanonicalAccountId,
  normalizeAccountAliasLiteral,
  normalizeAccountId,
  normalizeAccountIdOrAliasLiteral,
  normalizeAssetId,
  normalizeAssetHoldingId,
  normalizeRwaId,
} from "./normalizers.js";
import { MultisigSpec, MultisigSpecBuilder } from "./multisig.js";
import { getCurveEntryByPublicKeyMulticodec } from "./curveRegistry.js";
import {
  createValidationError,
  ValidationErrorCode,
} from "./validationError.js";
import { getNativeBinding } from "./native.js";

const MAX_SAFE_INTEGER = Number.MAX_SAFE_INTEGER;
const MAX_SAFE_INTEGER_BIGINT = BigInt(MAX_SAFE_INTEGER);
const MAX_NUMERIC_SCALE = 28;
const MAX_NUMERIC_BITS = 512;
const UINT32_MAX = 0xffff_ffff;
const DEFAULT_PRIVACY_MAX_PROOF_BYTES = 64 * 1024 * 1024;
const DEFAULT_PRIVACY_MAX_PUBLIC_INPUT_BYTES = 1024 * 1024;
const DEFAULT_PRIVACY_MAX_AUX_BYTES = 64 * 1024;
const ZK_ACE_BACKEND = "stark/fri/sha256-goldilocks";
const ZK_ACE_DOMAIN_TAG = "iroha:zk-ace:pq-authorization:v0";
const ZK_ACE_ACTION_TRANSFER = "transparent_asset_transfer";
const ZK_ACE_ALGORITHM_ID = "zk-ace-pq-authorization-v0";
const ZK_ACE_PRODUCTION_ENTRYPOINT = "buildZkAceAuthorizationProofV1";
const ZK_ACE_PRODUCTION_VK_REF = "stark-fri:zk_ace_pq_authorization_v0";
const ZK_ACE_PRODUCTION_DISABLED_MESSAGE =
  "native ZK-ACE prover returned PRIVACY_FFI_ERROR_PRODUCTION_DISABLED for " +
  `${ZK_ACE_ALGORITHM_ID} ${ZK_ACE_PRODUCTION_ENTRYPOINT} ` +
  `${ZK_ACE_PRODUCTION_VK_REF}: ` +
  "Iroha production allowlist is not enabled for this audited row";
const ANON_PGC_BACKEND = "stark/fri/sha256-goldilocks";
const ANON_PGC_CIRCUIT_ID = "stark/fri/sha256-goldilocks:anonymous_pgc_k_out_of_n_v1";
const ANON_PGC_DOMAIN_SEPARATOR = "iroha:anonymous-pgc:k-out-of-n:v1";
const ANON_PGC_DEV_PROOF_PREFIX = Buffer.from(
  "iroha:anonymous-pgc:dev-fixture:v1:",
  "utf8",
);
const ANON_PGC_MAX_RECEIVERS = 64;
const ANON_PGC_MAX_BALANCE_COMMITMENTS = 64;
const ANON_PGC_MAX_RANGE_COMMITMENTS = 64;
const ANON_PGC_MAX_CIPHERTEXT_BYTES = 64 * 1024;
const ZKAT_BACKEND = "stark/fri/sha256-goldilocks";
const ZKAT_CIRCUIT_ID = "stark/fri/sha256-goldilocks:zkat_policy_private_auth_v1";
const ZKAT_DOMAIN_SEPARATOR = "iroha:zkat:policy-private-auth:v1";
const ZKAT_DEV_PROOF_PREFIX = Buffer.from(
  "iroha:zkat:dev-fixture:v1:",
  "utf8",
);
const ZKAT_MAX_POLICY_BYTES = 1024 * 1024;
const ZK_AMS_BACKEND = "stark/fri/sha256-goldilocks";
const ZK_AMS_CIRCUIT_ID = "stark/fri/sha256-goldilocks:zk_ams_recursive_admission_v0";
const ZK_AMS_DOMAIN_SEPARATOR = "iroha:zk-ams:recursive-admission:v0";
const ZK_AMS_DEV_PROOF_PREFIX = Buffer.from(
  "iroha:zk-ams:dev-fixture:v0:",
  "utf8",
);
const ZK_AMS_MAX_ADMISSIONS = 4096;
const ZK_AMS_MAX_RECURSIVE_PROOF_BYTES = 64 * 1024 * 1024;
const VEGA_BACKEND = "stark/fri/sha256-goldilocks";
const VEGA_CIRCUIT_ID = "stark/fri/sha256-goldilocks:vega_existing_credential_zk_v0";
const VEGA_DOMAIN_SEPARATOR = "iroha:vega:existing-credential-zk:v0";
const VEGA_DEV_PROOF_PREFIX = Buffer.from(
  "iroha:vega:dev-fixture:v0:",
  "utf8",
);
const VEGA_MAX_PREDICATE_BYTES = 1024 * 1024;
const VEGA_MAX_ISSUER_BYTES = 1024 * 1024;
const SILENT_THRESHOLD_BACKEND = "stark/fri/sha256-goldilocks";
const SILENT_THRESHOLD_CIRCUIT_ID =
  "stark/fri/sha256-goldilocks:silent_threshold_anoncred_v0";
const SILENT_THRESHOLD_DOMAIN_SEPARATOR =
  "iroha:silent-threshold:anoncred:v0";
const SILENT_THRESHOLD_DEV_PROOF_PREFIX = Buffer.from(
  "iroha:silent-threshold:dev-fixture:v0:",
  "utf8",
);
const SILENT_THRESHOLD_MAX_ISSUER_SET_BYTES = 1024 * 1024;
const SILENT_THRESHOLD_MAX_POLICY_BYTES = 1024 * 1024;
const SILENT_THRESHOLD_MAX_SHOWING_BYTES = 1024 * 1024;
const ZK_X509_BACKEND = "stark/fri/sha256-goldilocks";
const ZK_X509_CIRCUIT_ID =
  "stark/fri/sha256-goldilocks:zk_x509_onchain_identity_v0";
const ZK_X509_DOMAIN_SEPARATOR = "iroha:zk-x509:onchain-identity:v0";
const ZK_X509_DEV_PROOF_PREFIX = Buffer.from(
  "iroha:zk-x509:dev-fixture:v0:",
  "utf8",
);
const ZK_X509_MAX_CA_ROOT_BYTES = 1024 * 1024;
const ZK_X509_MAX_POLICY_BYTES = 1024 * 1024;
const ZK_X509_MAX_REVOCATION_BYTES = 1024 * 1024;
const ZK_X509_MAX_SUBJECT_BYTES = 1024 * 1024;
const JINDO_BACKEND = "unsupported";
const JINDO_CIRCUIT_ID = "lattice/jindo-pcs-v0:jindo_lattice_pcs_zk_v0";
const JINDO_DOMAIN_SEPARATOR = "iroha:jindo:lattice-pcs:v0";
const JINDO_DEV_PROOF_PREFIX = Buffer.from(
  "iroha:jindo:dev-fixture:v0:",
  "utf8",
);
const JINDO_MAX_POLYNOMIAL_BYTES = 16 * 1024 * 1024;
const JINDO_MAX_OPENING_CLAIM_BYTES = 1024 * 1024;
const JINDO_MAX_QUERY_SET_BYTES = 1024 * 1024;
const JINDO_MAX_PARAMETER_BYTES = 1024 * 1024;
const SIS_HINTS_BACKEND = "unsupported";
const SIS_HINTS_CIRCUIT_ID =
  "lattice/sis-hints-anoncred-v0:sis_hints_anoncred_pq_v0";
const SIS_HINTS_DOMAIN_SEPARATOR = "iroha:sis-hints:anoncred:v0";
const SIS_HINTS_DEV_PROOF_PREFIX = Buffer.from(
  "iroha:sis-hints:dev-fixture:v0:",
  "utf8",
);
const SIS_HINTS_MAX_ISSUER_BYTES = 1024 * 1024;
const SIS_HINTS_MAX_CREDENTIAL_BYTES = 1024 * 1024;
const SIS_HINTS_MAX_POLICY_BYTES = 1024 * 1024;
const SIS_HINTS_MAX_PARAMETER_BYTES = 1024 * 1024;
const VERANGE_BACKEND = "stark/fri/sha256-goldilocks";
const VERANGE_CIRCUIT_ID = "stark/fri/sha256-goldilocks:verange_transparent_range_v1";
const VERANGE_DOMAIN_SEPARATOR = "iroha:verange:transparent-range:v1";
const VERANGE_DEV_PROOF_PREFIX = Buffer.from(
  "iroha:verange:dev-fixture:v1:",
  "utf8",
);
const VERANGE_MAX_AGGREGATION_COUNT = 1024;
const VERANGE_MAX_BIT_LENGTH = 256;
const VERANGE_MAX_PAYLOAD_BYTES = 1024 * 1024;
const VERANGE_COMMITMENT_SCHEMES = Object.freeze(new Set([
  "pedersen-v1",
  "pedersen-bls12-381",
  "pedersen-decaf377",
  "verange-pedersen-v1",
]));

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

function assertNonBlankString(value, name) {
  const raw = assertString(value, name);
  const trimmed = raw.trim();
  if (trimmed.length === 0) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty string`, name);
  }
  return trimmed;
}

function readSingleAlias(source, aliases, name, description) {
  const present = aliases.filter((key) => Object.prototype.hasOwnProperty.call(source, key));
  if (present.length > 1) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must not include multiple ${description} aliases: ${present.join(", ")}`,
      name,
    );
  }
  if (present.length === 0) {
    return { key: null, value: undefined };
  }
  return { key: present[0], value: source[present[0]] };
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

function asPositiveU128JsonNumber(value, name) {
  const amount = asU128JsonNumber(value, name);
  if (amount <= 0) {
    fail(ValidationErrorCode.VALUE_OUT_OF_RANGE, `${name} must be greater than zero`, name);
  }
  return amount;
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
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    fail(ValidationErrorCode.INVALID_OBJECT, `${name} must be a plain object`, name);
  }
  return value;
}

function assertAllowedFields(source, allowed, name) {
  for (const field of Reflect.ownKeys(source)) {
    const label = typeof field === "symbol" ? field.toString() : field;
    const descriptor = Object.getOwnPropertyDescriptor(source, field);
    if (typeof field !== "string" || !allowed.has(field)) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${name}.${label} is not supported`,
        `${name}.${label}`,
      );
    }
    if (
      !descriptor ||
      !descriptor.enumerable ||
      !Object.prototype.hasOwnProperty.call(descriptor, "value")
    ) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${name}.${label} must be an enumerable data field`,
        `${name}.${label}`,
      );
    }
  }
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

function normalizeSafeIntegerJson(value, name, { allowNegative = false } = {}) {
  if (typeof value === "bigint") {
    if ((!allowNegative && value < 0n) || value < BigInt(Number.MIN_SAFE_INTEGER)) {
      fail(ValidationErrorCode.VALUE_OUT_OF_RANGE, `${name} must fit in JavaScript's safe integer range`, name);
    }
    if (value > MAX_SAFE_INTEGER_BIGINT) {
      fail(ValidationErrorCode.VALUE_OUT_OF_RANGE, `${name} must fit in JavaScript's safe integer range`, name);
    }
    return Number(value);
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value) || !Number.isInteger(value)) {
      fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be an integer`, name);
    }
    if (!allowNegative && value < 0) {
      fail(ValidationErrorCode.VALUE_OUT_OF_RANGE, `${name} must be non-negative`, name);
    }
    if (!Number.isSafeInteger(value)) {
      fail(ValidationErrorCode.VALUE_OUT_OF_RANGE, `${name} must fit in JavaScript's safe integer range`, name);
    }
    return value;
  }
  if (typeof value === "string") {
    const trimmed = value.trim();
    const pattern = allowNegative ? /^-?\d+$/ : /^\d+$/;
    if (!pattern.test(trimmed)) {
      fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be an integer literal`, name);
    }
    const numeric = BigInt(trimmed);
    if ((!allowNegative && numeric < 0n) || numeric < BigInt(Number.MIN_SAFE_INTEGER)) {
      fail(ValidationErrorCode.VALUE_OUT_OF_RANGE, `${name} must fit in JavaScript's safe integer range`, name);
    }
    if (numeric > MAX_SAFE_INTEGER_BIGINT) {
      fail(ValidationErrorCode.VALUE_OUT_OF_RANGE, `${name} must fit in JavaScript's safe integer range`, name);
    }
    return Number(numeric);
  }
  fail(ValidationErrorCode.INVALID_NUMERIC, `${name} must be an integer`, name);
}

function normalizeExecuteTriggerBuilderInput(triggerOrOptions, args, context = "executeTrigger") {
  if (typeof triggerOrOptions === "string") {
    return {
      trigger: assertString(triggerOrOptions, `${context}.trigger`),
      args:
        args === undefined
          ? null
          : normalizeJsonValue(args, `${context}.args`),
    };
  }
  const source = assertPlainObject(triggerOrOptions, context);
  return {
    trigger: assertString(
      source.trigger ?? source.triggerId,
      `${context}.trigger`,
    ),
    args:
      source.args === undefined
        ? null
        : normalizeJsonValue(source.args, `${context}.args`),
  };
}

function resolveMultisigTriggerArgs(options, context) {
  if (options.args !== undefined) {
    return normalizeJsonValue(options.args, `${context}.args`);
  }
  const preset = options.argPreset ?? options.preset;
  if (preset === undefined || preset === null) {
    return null;
  }
  return buildMultisigTriggerArgs(
    preset,
    options.argInput ?? options.presetInput ?? options.input ?? {},
  );
}

function normalizeMultisigExecuteTriggerOptions(options, context) {
  const source = assertPlainObject(options, context);
  const normalized = {
    trigger: assertString(source.trigger, `${context}.trigger`),
    args: resolveMultisigTriggerArgs(source, context),
    signerAccountId:
      source.signerAccountId === undefined || source.signerAccountId === null
        ? null
        : normalizeAccountId(source.signerAccountId, `${context}.signerAccountId`),
    strictSignerCheck: Boolean(source.strictSignerCheck ?? source.strict_signer_check),
    multisigSpec:
      source.multisigSpec === undefined && source.spec === undefined
        ? null
        : normalizeMultisigSpecPayload(
            source.multisigSpec ?? source.spec,
            `${context}.multisigSpec`,
          ),
  };

  if (normalized.strictSignerCheck) {
    if (!normalized.multisigSpec) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.multisigSpec is required when strictSignerCheck is true`,
        `${context}.multisigSpec`,
      );
    }
    if (!normalized.signerAccountId) {
      fail(
        ValidationErrorCode.INVALID_ACCOUNT_ID,
        `${context}.signerAccountId is required when strictSignerCheck is true`,
        `${context}.signerAccountId`,
      );
    }
    if (!isMultisigSignerAuthorized(normalized.multisigSpec, normalized.signerAccountId)) {
      fail(
        ValidationErrorCode.INVALID_ACCOUNT_ID,
        `${context}.signerAccountId is not present in multisigSpec.signatories`,
        `${context}.signerAccountId`,
      );
    }
  }

  return normalized;
}

function normalizeMultisigAccountSelectorInput(source, context) {
  const hasAccountId =
    source.multisigAccountId !== undefined ||
    source.multisig_account_id !== undefined;
  const hasAlias =
    source.multisigAccountAlias !== undefined ||
    source.multisig_account_alias !== undefined;
  if ((hasAccountId ? 1 : 0) + (hasAlias ? 1 : 0) !== 1) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} requires exactly one of multisigAccountId or multisigAccountAlias`,
      context,
    );
  }
  if (hasAccountId) {
    return {
      multisig_account_id: normalizeAccountId(
        source.multisigAccountId ?? source.multisig_account_id,
        `${context}.multisigAccountId`,
      ),
    };
  }
  const alias = normalizeAccountAliasLiteral(
    source.multisigAccountAlias ?? source.multisig_account_alias,
    `${context}.multisigAccountAlias`,
  );
  return {
    multisig_account_alias: alias,
  };
}

function normalizeDetachedPrivateKeyForMultisigRequest(source, context) {
  const direct = source.privateKey ?? source.private_key;
  if (direct !== undefined && direct !== null) {
    if (
      typeof direct !== "string" &&
      !Buffer.isBuffer(direct) &&
      !ArrayBuffer.isView(direct) &&
      !(direct instanceof ArrayBuffer)
    ) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.privateKey must be a string or binary payload`,
        `${context}.privateKey`,
      );
    }
    return direct;
  }

  const multihash = source.privateKeyMultihash ?? source.private_key_multihash;
  if (multihash !== undefined && multihash !== null) {
    return assertString(multihash, `${context}.privateKeyMultihash`);
  }

  const rawHex = source.privateKeyHex ?? source.private_key_hex;
  if (rawHex !== undefined && rawHex !== null) {
    return formatDetachedPrivateKeyAlgorithmPrefixedHex(
      rawHex,
      source,
      context,
      "privateKeyHex",
    );
  }

  const rawBytes = source.privateKeyBytes ?? source.private_key_bytes;
  if (rawBytes !== undefined && rawBytes !== null) {
    return formatDetachedPrivateKeyAlgorithmPrefixedHex(
      Buffer.from(normalizeByteArray(rawBytes, `${context}.privateKeyBytes`)).toString("hex"),
      source,
      context,
      "privateKeyBytes",
    );
  }

  return null;
}

function formatDetachedPrivateKeyAlgorithmPrefixedHex(value, source, context, label) {
  const normalizedHex = normalizeOptionalHexString(value, `${context}.${label}`);
  const algorithm = assertString(
    source.privateKeyAlgorithm ??
      source.private_key_algorithm ??
      "ed25519",
    `${context}.privateKeyAlgorithm`,
  ).toLowerCase();
  return `${algorithm}:${normalizedHex}`;
}

function normalizeOptionalHexString(value, name) {
  const literal = assertString(value, name);
  const compact = literal.replace(/^0x/i, "");
  if (!/^[0-9A-Fa-f]{64}$/.test(compact)) {
    fail(ValidationErrorCode.INVALID_HEX, `${name} must be a 32-byte hex string`, name);
  }
  return compact.toLowerCase();
}

function normalizeOptionalBase64String(value, name) {
  const literal = assertString(value, name);
  if (literal.length === 0) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must not be empty`, name);
  }
  try {
    Buffer.from(literal, "base64");
  } catch (error) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must be valid base64`, name);
  }
  return literal;
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
    const raw = value;
    if (raw.trim().length === 0) {
      fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty string`, name);
    }
    const parts = raw.split(":");
    if (parts.length !== 2 || parts[0].length === 0 || parts[1].length === 0) {
      fail(
        ValidationErrorCode.INVALID_STRING,
        `${name} must be in 'backend:name' format`,
        name,
      );
    }
    const backend = parts[0];
    const keyName = parts[1].trim();
    if (backend.trim().length === 0 || keyName.length === 0) {
      fail(
        ValidationErrorCode.INVALID_STRING,
        `${name} must be in 'backend:name' format`,
        name,
      );
    }
    return {
      backend,
      name: keyName,
    };
  }
  const object = assertPlainObject(value, name);
  const allowedFields = new Set(["backend", "backendId", "name", "id", "key"]);
  for (const field of Object.keys(object)) {
    if (!allowedFields.has(field)) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${name}.${field} is not supported`,
        `${name}.${field}`,
      );
    }
  }
  const backendAlias = readSingleAlias(
    object,
    ["backend", "backendId"],
    `${name}.backend`,
    "backend",
  );
  const nameAlias = readSingleAlias(
    object,
    ["name", "id", "key"],
    `${name}.name`,
    "name",
  );
  const backend = assertString(backendAlias.value, `${name}.backend`);
  if (backend.trim().length === 0) {
    fail(ValidationErrorCode.INVALID_STRING, `${name}.backend must be a non-empty string`, `${name}.backend`);
  }
  const keyName = assertNonBlankString(nameAlias.value, `${name}.name`);
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
  for (const field of [
    "vk_inline",
    "vkInline",
    "verifyingKeyInline",
    "verifying_key_inline",
    "vk_reference",
  ]) {
    if (Object.prototype.hasOwnProperty.call(source, field)) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${name}.${field} is not supported; use verifyingKeyRef`,
        `${name}.${field}`,
      );
    }
  }
  const backend = assertString(source.backend, `${name}.backend`);
  const proofAlias = readSingleAlias(
    source,
    ["proofBytes", "proof_bytes", "proof", "proof_b64", "proofBase64", "proofB64"],
    `${name}.proof`,
    "proof byte",
  );
  const rawProof = proofAlias.value;
  const rawProofIsStructuredObject =
    rawProof &&
    typeof rawProof === "object" &&
    !Array.isArray(rawProof) &&
    !Buffer.isBuffer(rawProof) &&
    !ArrayBuffer.isView(rawProof) &&
    !(rawProof instanceof ArrayBuffer);
  const proofAliasMayCarryBytes =
    proofAlias.key === "proofBytes" ||
    proofAlias.key === "proof_bytes" ||
    proofAlias.key === "proof" ||
    proofAlias.key === "proof_b64" ||
    proofAlias.key === "proofBase64" ||
    proofAlias.key === "proofB64";
  const attachmentHasProofBytes = proofAliasMayCarryBytes && !rawProofIsStructuredObject;
  const structuredProofBox =
    proofAliasMayCarryBytes && rawProofIsStructuredObject;

  const verifyRef = readSingleAlias(
    source,
    ["verifyingKeyRef", "vkRef", "vk_ref"],
    `${name}.verifyingKeyRef`,
    "verifying key reference",
  ).value;
  const commitmentInput = readSingleAlias(
    source,
    ["verifyingKeyCommitment", "vkCommitment", "vk_commitment"],
    `${name}.verifyingKeyCommitment`,
    "verifying key commitment",
  ).value;
  const envelopeInput = readSingleAlias(
    source,
    ["envelopeHash", "envelope_hash", "proofEnvelopeHash"],
    `${name}.envelopeHash`,
    "envelope hash",
  ).value;

  let proofBox;
  if (attachmentHasProofBytes) {
    const proofBackend = assertString(
      source.backend ?? backend,
      `${name}.proof.backend`,
    );
    proofBox = {
      backend: proofBackend,
      bytes: normalizeByteArray(rawProof, `${name}.proof`),
    };
  } else if (structuredProofBox) {
    const allowedProofFields = new Set([
      "backend",
      "bytes",
      "bytes_b64",
      "bytesBase64",
      "data",
      "payload",
    ]);
    for (const field of Object.keys(rawProof)) {
      if (!allowedProofFields.has(field)) {
        fail(
          ValidationErrorCode.INVALID_OBJECT,
          `${name}.proof.${field} is not supported`,
          `${name}.proof.${field}`,
        );
      }
    }
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
  if (proofBox.backend !== backend) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name}.proof.backend must match ${name}.backend`,
      `${name}.proof.backend`,
    );
  }

  if (!verifyRef) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must include verifyingKeyRef`,
      name,
    );
  }

  const payload = { backend, proof: proofBox };
  payload.vk_ref = normalizeVerifyingKeyId(verifyRef, `${name}.verifyingKeyRef`);
  if (payload.vk_ref.backend !== backend) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name}.verifyingKeyRef.backend must match ${name}.backend`,
      `${name}.verifyingKeyRef.backend`,
    );
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

function normalizeNonZeroFixedBytes(value, name, length = 32) {
  const bytes = normalizeFixedBytes(value, name, length);
  if (bytes.every((byte) => byte === 0)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be nonzero`,
      name,
    );
  }
  return bytes;
}

function normalizeU32(value, name) {
  const numeric = asNonNegativeInteger(value, name);
  if (numeric > UINT32_MAX) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} must fit in an unsigned 32-bit integer`,
      name,
    );
  }
  return numeric;
}

function normalizePositiveU32(value, name) {
  const numeric = asPositiveInteger(value, name);
  if (numeric > UINT32_MAX) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} must fit in an unsigned 32-bit integer`,
      name,
    );
  }
  return numeric;
}

function normalizeOptionalPrivacyMaxLimits(source, context) {
  const maxProofAlias = readSingleAlias(
    source,
    ["maxProofBytes", "max_proof_bytes"],
    `${context}.maxProofBytes`,
    "max proof byte limit",
  );
  const maxPublicInputAlias = readSingleAlias(
    source,
    ["maxPublicInputBytes", "max_public_input_bytes"],
    `${context}.maxPublicInputBytes`,
    "max public input byte limit",
  );
  return {
    maxProofBytes:
      maxProofAlias.key === null
        ? undefined
        : normalizePositiveU32(maxProofAlias.value, `${context}.maxProofBytes`),
    maxPublicInputBytes:
      maxPublicInputAlias.key === null
        ? undefined
        : normalizePositiveU32(
            maxPublicInputAlias.value,
            `${context}.maxPublicInputBytes`,
    ),
  };
}

function normalizePositiveU32AliasOrDefault(
  source,
  aliases,
  context,
  description,
  defaultValue,
) {
  const alias = readSingleAlias(source, aliases, context, description);
  return alias.key === null
    ? defaultValue
    : normalizePositiveU32(alias.value, context);
}

function normalizeOptionalU64(value, name) {
  if (value === undefined || value === null) {
    return null;
  }
  return asNonNegativeInteger(value, name);
}

function normalizeOptionalNonBlankString(value, name) {
  if (value === undefined || value === null) {
    return null;
  }
  return assertNonBlankString(value, name);
}

const PRODUCTION_NATIVE_HALO2_PASTA_BACKENDS = new Set([
  "halo2/pasta/kaigi-roster-v1",
  "halo2/pasta/kaigi-usage-v1",
  "halo2/pasta/ivm-overlay-bind",
  "halo2/pasta/ivm-execution-v1",
  "halo2/pasta/offline-note-recursive",
  "halo2/pasta/kagemusha-folded-v1",
  "halo2/pasta/kagemusha-recursive-aggregation-v1",
  "halo2/pasta/kagemusha-recursive-spend-lineage-v1",
  "halo2/pasta/kagemusha-recursive-spend-lineage-onehop-v1",
  "halo2/pasta/kagemusha-recursive-spend-lineage-append-v1",
  "halo2/pasta/anon-transfer-2x2-merkle16-poseidon-diversified",
  "halo2/pasta/anon-unshield-merkle16-poseidon-diversified",
  "halo2/pasta/anon-unshield-2in-1change-merkle16-poseidon-diversified",
]);

const TRUSTED_SETUP_BACKEND_SEGMENTS = new Set([
  "groth16",
  "kzg",
  "bn254",
  "bn256",
  "bls12",
  "srs",
  "crs",
  "ptau",
  "ceremony",
  "powersoftau",
]);

const TRUSTED_SETUP_COMPACT_TOKENS = [
  "groth16",
  "kzg",
  "bn254",
  "bn256",
  "bls12381",
  "bls12",
  "srs",
  "crs",
  "ptau",
  "ceremony",
  "trustedsetup",
  "structuredreferencestring",
  "universalsrs",
  "powersoftau",
];

const PRODUCTION_CLAIM_BACKEND_FRAGMENTS = [
  "productionready",
  "productionhardened",
  "productionenabled",
  "productionapproved",
  "productioncertified",
  "productionclaim",
  "claimedproduction",
  "mainnetready",
  "mainnetcomplete",
  "mainnetclaim",
  "claimedmainnet",
  "auditedproduction",
  "externallyaudited",
  "auditpassed",
  "auditapproved",
  "auditsignoff",
  "auditclaim",
  "claimedaudit",
  "securityreviewpassed",
];

function compactPrivacyBackendLabel(value) {
  return String(value).trim().toLowerCase().replace(/[^a-z0-9]/g, "");
}

function isPendingProductionVerifierBackendLabel(value) {
  const compact = compactPrivacyBackendLabel(value);
  return PENDING_PRODUCTION_VERIFIER_BACKEND_ALIASES.has(compact);
}

const PENDING_PRODUCTION_VERIFIER_BACKEND_ALIASES = new Set([
  "halo2ipaorchard",
  "orchard",
  "zcashorchard",
  "groth16bls12377",
  "groth16bls12377decaf377",
  "bls12377",
  "decaf377",
  "masp",
  "penumbra",
  "penumbramasp",
  "halo2ipapenumbra",
  "halo2ipamasp",
  "fcmppluspluscurvetree",
  "fcmp",
  "monero",
  "monerofcmp",
  "monerofcmpplusplus",
  "curvetree",
  "halo2ipamonero",
  "halo2ipacurvetree",
  "latticepcssis",
  "latticepcszk",
  "jindo",
  "jindolatticepcszk",
  "jindolatticepcszkv0",
  "jindolatticepcssis",
  "starkfrimiden",
  "midenstark",
  "aztecplonkishprivatekernel",
  "aztecprivatekernel",
  "pqmaspstarkfri",
  "pqmaspstark",
  "starkfripqmaspstarkfri",
  "postquantummasp",
  "anonymouspgc",
  "anonymouspgckoutofn",
  "anonymouspgckoutofnv1",
  "verange",
  "verangetransparentrange",
  "verangetransparentrangev1",
  "zkat",
  "zkatpolicyprivateauthenticator",
  "zkatpolicyprivateauthv1",
  "recursiveanonymousadmission",
  "recursiveanonymousadmissionv0",
  "zkamsrecursiveadmission",
  "zkamsrecursiveadmissionv0",
  "vegaexistingcredentialzk",
  "vegaexistingcredentialzkv0",
  "silentthresholdanoncred",
  "silentthresholdanoncredv0",
  "silentthresholdanonymouscredential",
  "thresholdanonymouscredentials",
  "zkx509",
  "zkvmx509identity",
  "zkx509onchainidentity",
  "zkx509onchainidentityv0",
  "siswithhints",
  "sishints",
  "sishintsanoncredpqv0",
  "latticeanonymouscredentials",
]);

function hasTrustedSetupBackendSegment(value) {
  return String(value)
    .trim()
    .toLowerCase()
    .split(/[^a-z0-9]+/u)
    .some((segment) => TRUSTED_SETUP_BACKEND_SEGMENTS.has(segment));
}

function isTrustedSetupVerifierBackendLabel(value) {
  const backend = String(value).trim().toLowerCase();
  const compact = compactPrivacyBackendLabel(value);
  return (
    hasTrustedSetupBackendSegment(value) ||
    TRUSTED_SETUP_COMPACT_TOKENS.some((token) => compact.includes(token)) ||
    backend === "groth16" ||
    backend.startsWith("groth16/") ||
    backend === "kzg" ||
    backend.startsWith("kzg/") ||
    backend === "bn254" ||
    backend === "bn256" ||
    backend === "bls12_381" ||
    backend === "bls12-381" ||
    backend === "halo2/bn254" ||
    backend.startsWith("halo2/bn254/") ||
    backend.includes("/bn254") ||
    backend.includes(":bn254") ||
    backend.includes("/bn256") ||
    backend.includes(":bn256") ||
    backend.includes("/bls12") ||
    backend.includes(":bls12") ||
    backend === "halo2/kzg" ||
    backend.startsWith("halo2/kzg/") ||
    backend.includes("/kzg") ||
    backend.includes(":kzg")
  );
}

function isDeveloperOnlyVerifierBackendLabel(value) {
  const backend = String(value).trim().toLowerCase();
  const embedded = ["debug", "mock", "fixture", "dev"];
  const exact = new Set(["test", "dummy", "fake", "stub", "sample", "placeholder"]);
  const isDeveloperOnlyRun = (run) => embedded.some((token) => run.includes(token)) || exact.has(run);
  let letterRun = "";
  for (const token of backend.split(/[^a-z0-9]+/u).filter(Boolean)) {
    if (isDeveloperOnlyRun(token)) {
      return true;
    }
    if (token.length === 1) {
      letterRun += token;
      continue;
    }
    if (isDeveloperOnlyRun(letterRun)) {
      return true;
    }
    letterRun = "";
  }
  return isDeveloperOnlyRun(letterRun);
}

function isProductionClaimVerifierBackendLabel(value) {
  const compact = compactPrivacyBackendLabel(value);
  return PRODUCTION_CLAIM_BACKEND_FRAGMENTS.some((fragment) => compact.includes(fragment));
}

const STARK_FRI_PRODUCTION_BACKEND_LABELS = new Set([
  "stark/fri",
  "stark/fri/sha256-goldilocks",
  "stark/fri/poseidon2-goldilocks",
  "stark/fri/sha256_goldilocks.v1",
]);

const HUMAN_READABLE_PRIVACY_BACKEND_ALIASES = new Set([
  "zkAt policy-private authenticator",
]);

const PLUS_PRIVACY_BACKEND_ALIASES = new Set([
  "fcmp++",
  "monero-fcmp++",
]);

function isStarkFriProductionBackendLabel(backend) {
  return STARK_FRI_PRODUCTION_BACKEND_LABELS.has(backend);
}

function isPortableVerifierBackendLabel(backend) {
  if (HUMAN_READABLE_PRIVACY_BACKEND_ALIASES.has(backend)) {
    return true;
  }
  if (backend.includes("+") && !PLUS_PRIVACY_BACKEND_ALIASES.has(backend)) {
    return false;
  }
  return /^[A-Za-z0-9/_.:+-]+$/u.test(backend);
}

function normalizeNativeHalo2PastaBackendLabel(value) {
  const backend = String(value);
  if (backend.length === 0 || backend.trim() !== backend) {
    return null;
  }
  for (const [prefix, targetPrefix] of [
    ["halo2/pasta/ipa/", "halo2/pasta/"],
    ["halo2/pasta/", "halo2/pasta/"],
    ["halo2/ipa::", "halo2/pasta/"],
    ["halo2/ipa:", "halo2/pasta/"],
    ["halo2/ipa/", "halo2/pasta/"],
  ]) {
    if (backend.startsWith(prefix)) {
      const rest = backend.slice(prefix.length);
      return rest.length === 0 ? null : `${targetPrefix}${rest}`;
    }
  }
  return null;
}

function isNativeHalo2PastaProductionBackendLabel(backend) {
  const normalized = normalizeNativeHalo2PastaBackendLabel(backend);
  return normalized !== null && PRODUCTION_NATIVE_HALO2_PASTA_BACKENDS.has(normalized);
}

function isProductionVerifyBackendLabel(value) {
  if (typeof value !== "string") {
    return false;
  }
  const backend = value;
  if (
    backend.length === 0 ||
    backend.trim() !== backend ||
    !isPortableVerifierBackendLabel(backend) ||
    isPendingProductionVerifierBackendLabel(backend) ||
    isProductionClaimVerifierBackendLabel(backend) ||
    isTrustedSetupVerifierBackendLabel(backend) ||
    isDeveloperOnlyVerifierBackendLabel(backend)
  ) {
    return false;
  }
  return (
    backend === "halo2/ipa" ||
    isStarkFriProductionBackendLabel(backend) ||
    isNativeHalo2PastaProductionBackendLabel(backend)
  );
}

function assertProductionVerifyBackendLabel(value, name) {
  const backend = assertString(value, name);
  if (backend.trim().length === 0) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty string`, name);
  }
  if (!isProductionVerifyBackendLabel(backend)) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} uses unsupported production verifier backend ${backend}`,
      name,
    );
  }
  return backend;
}

function normalizePrivacyBackendTag(value, name) {
  const raw = assertString(value, name);
  if (raw.trim().length === 0) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty string`, name);
  }
  if (raw.trim() !== raw || !isPortableVerifierBackendLabel(raw)) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} uses a non-portable verifier backend label`,
      name,
    );
  }
  const backend = raw.trim().toLowerCase();
  if (isProductionVerifyBackendLabel(backend) && isStarkFriProductionBackendLabel(backend)) {
    return "Stark";
  }
  const normalized = raw.toLowerCase().replace(/[^a-z0-9]/g, "");
  switch (normalized) {
    case "halo2ipapasta":
    case "halo2pasta":
    case "halo2ipa":
    case "pasta":
      return "Halo2IpaPasta";
    case "halo2bn254":
      return "Halo2Bn254";
    case "groth16":
      return "Groth16";
    case "stark":
    case "starkfri":
    case "starkfrisha256goldilocks":
      return "Stark";
    case "unsupported":
      return "Unsupported";
    case "halo2ipaorchard":
    case "orchard":
    case "zcashorchard":
      return "Halo2IpaOrchard";
    case "groth16bls12377":
    case "groth16bls12377decaf377":
    case "bls12377":
    case "decaf377":
    case "masp":
    case "penumbra":
    case "penumbramasp":
    case "halo2ipapenumbra":
    case "halo2ipamasp":
      return "Groth16Bls12377";
    case "fcmppluspluscurvetree":
    case "fcmp":
    case "monero":
    case "monerofcmp":
    case "monerofcmpplusplus":
    case "curvetree":
    case "halo2ipamonero":
    case "halo2ipacurvetree":
      return "FcmpPlusPlusCurveTree";
    case "latticepcssis":
    case "latticepcszk":
    case "jindo":
    case "jindolatticepcszk":
    case "jindolatticepcszkv0":
    case "jindolatticepcssis":
      return "LatticePcsSis";
    case "starkfrimiden":
    case "midenstark":
      return "MidenStark";
    case "aztecplonkishprivatekernel":
    case "aztecprivatekernel":
      return "AztecPlonkishPrivateKernel";
    case "pqmaspstarkfri":
    case "pqmaspstark":
    case "starkfripqmaspstarkfri":
    case "postquantummasp":
      return "PqMaspStarkFri";
    case "anonymouspgc":
    case "anonymouspgckoutofn":
    case "anonymouspgckoutofnv1":
      return "AnonymousPgc";
    case "verange":
    case "verangetransparentrange":
    case "verangetransparentrangev1":
      return "VeRange";
    case "zkat":
    case "zkatpolicyprivateauthenticator":
    case "zkatpolicyprivateauthv1":
      return "ZkAt";
    case "recursiveanonymousadmission":
    case "recursiveanonymousadmissionv0":
    case "zkamsrecursiveadmission":
    case "zkamsrecursiveadmissionv0":
      return "RecursiveAnonymousAdmission";
    case "vegaexistingcredentialzk":
    case "vegaexistingcredentialzkv0":
      return "VegaExistingCredentialZk";
    case "silentthresholdanoncred":
    case "silentthresholdanoncredv0":
    case "silentthresholdanonymouscredential":
    case "thresholdanonymouscredentials":
      return "SilentThresholdAnoncred";
    case "zkx509":
    case "zkvmx509identity":
    case "zkx509onchainidentity":
    case "zkx509onchainidentityv0":
      return "ZkX509";
    case "siswithhints":
    case "sishints":
    case "sishintsanoncredpqv0":
    case "latticeanonymouscredentials":
      return "SisWithHints";
    default:
      fail(
        ValidationErrorCode.INVALID_STRING,
        `${name} uses unsupported privacy verifier backend ${raw}`,
        name,
      );
  }
}

function inferPrivacyBackendTagFromVerifierId(id, name) {
  const backend = id?.backend;
  if (!backend) {
    fail(ValidationErrorCode.INVALID_OBJECT, `${name} is required`, name);
  }
  return normalizePrivacyBackendTag(backend, name);
}

function defaultPrivacyCurveForBackendTag(tag, name) {
  switch (tag) {
    case "Halo2IpaPasta":
      return "pallas";
    case "Stark":
      return "goldilocks";
    default:
      fail(
        ValidationErrorCode.INVALID_STRING,
        `${name} must be Halo2IpaPasta or Stark`,
        name,
      );
  }
}

function assertRegisterablePrivacyBackendTag(tag, name) {
  if (tag !== "Halo2IpaPasta" && tag !== "Stark") {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be Halo2IpaPasta or Stark`,
      name,
    );
  }
}

function normalizePrivacyCurve(value, backendTag, name) {
  const curve = assertNonBlankString(
    value ?? defaultPrivacyCurveForBackendTag(backendTag, name),
    name,
  );
  if (backendTag === "Halo2IpaPasta" && curve !== "pallas") {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be "pallas" for Halo2IpaPasta verifier keys`,
      name,
    );
  }
  if (backendTag === "Stark" && curve !== "goldilocks") {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be "goldilocks" for Stark verifier keys`,
      name,
    );
  }
  return curve;
}

function normalizeConfidentialStatus(value, name, defaultValue = "Proposed") {
  const raw = assertNonBlankString(value ?? defaultValue, name);
  const normalized = raw.toLowerCase();
  switch (normalized) {
    case "proposed":
      return "Proposed";
    case "active":
      return "Active";
    case "withdrawn":
      return "Withdrawn";
    default:
      fail(
        ValidationErrorCode.INVALID_STRING,
        `${name} must be Proposed, Active, or Withdrawn`,
        name,
      );
  }
}

function normalizeBoundedByteArray(
  value,
  name,
  { maxBytes, allowEmpty = false } = {},
) {
  let bytes;
  if (allowEmpty && (value === undefined || value === null)) {
    bytes = [];
  } else if (allowEmpty && Array.isArray(value) && value.length === 0) {
    bytes = [];
  } else if (allowEmpty && Buffer.isBuffer(value) && value.length === 0) {
    bytes = [];
  } else if (
    allowEmpty &&
    ArrayBuffer.isView(value) &&
    value.byteLength === 0
  ) {
    bytes = [];
  } else if (allowEmpty && value instanceof ArrayBuffer && value.byteLength === 0) {
    bytes = [];
  } else {
    bytes = normalizeByteArray(value, name);
  }
  if (maxBytes !== undefined && bytes.length > maxBytes) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} must be no larger than ${maxBytes} bytes`,
      name,
    );
  }
  return bytes;
}

function normalizeOpenVerifyByteArray(
  value,
  name,
  { maxBytes, allowEmpty = false } = {},
) {
  let bytes;
  if (typeof value === "string") {
    if (value.length === 0) {
      fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty base64 string`, name);
    }
    if (value.trim() !== value || /\s/.test(value)) {
      fail(
        ValidationErrorCode.INVALID_STRING,
        `${name} must be a clean base64 string without whitespace`,
        name,
      );
    }
    bytes = Array.from(decodeBase64Strict(value, name).values());
  } else if (Array.isArray(value)) {
    if (!allowEmpty && value.length === 0) {
      fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty byte array`, name);
    }
    bytes = value.map((byte, index) => {
      if (typeof byte !== "number" || !Number.isInteger(byte) || byte < 0 || byte > 0xff) {
        fail(
          ValidationErrorCode.VALUE_OUT_OF_RANGE,
          `${name}[${index}] must be an integer between 0 and 255`,
          `${name}[${index}]`,
        );
      }
      return byte;
    });
  } else if (allowEmpty && (value === undefined || value === null)) {
    bytes = [];
  } else if (Buffer.isBuffer(value) || value instanceof Uint8Array) {
    bytes = Array.from(Buffer.from(value.buffer, value.byteOffset, value.byteLength).values());
  } else if (value instanceof DataView) {
    bytes = Array.from(Buffer.from(value.buffer, value.byteOffset, value.byteLength).values());
  } else if (value instanceof ArrayBuffer) {
    bytes = Array.from(Buffer.from(value).values());
  } else {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be a Buffer, Uint8Array, DataView, ArrayBuffer, or explicit byte array`,
      name,
    );
  }
  if (!allowEmpty && bytes.length === 0) {
    fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty byte array`, name);
  }
  if (maxBytes !== undefined && bytes.length > maxBytes) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} must be no larger than ${maxBytes} bytes`,
      name,
    );
  }
  return bytes;
}

function normalizeOpenVerifyFixedBytes(value, name, length = 32, { nonzero = false } = {}) {
  let buffer;
  if (typeof value === "string") {
    if (value.length === 0) {
      fail(ValidationErrorCode.INVALID_STRING, `${name} must be a non-empty string`, name);
    }
    if (value.trim() !== value || /\s/.test(value)) {
      fail(
        ValidationErrorCode.INVALID_STRING,
        `${name} must be clean and without whitespace`,
        name,
      );
    }
    if (/^[0-9A-Fa-f]+$/.test(value) && value.length === length * 2) {
      buffer = Buffer.from(value, "hex");
    } else {
      buffer = decodeBase64Strict(value, name);
    }
  } else if (Array.isArray(value)) {
    if (value.length !== length) {
      fail(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name} must contain exactly ${length} elements`,
        name,
      );
    }
    buffer = Buffer.from(value.map((byte, index) => {
      if (typeof byte !== "number" || !Number.isInteger(byte) || byte < 0 || byte > 0xff) {
        fail(
          ValidationErrorCode.VALUE_OUT_OF_RANGE,
          `${name}[${index}] must be an integer between 0 and 255`,
          `${name}[${index}]`,
        );
      }
      return byte;
    }));
  } else if (Buffer.isBuffer(value) || value instanceof Uint8Array) {
    buffer = Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  } else if (value instanceof DataView) {
    buffer = Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  } else if (value instanceof ArrayBuffer) {
    buffer = Buffer.from(value);
  } else {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be a Buffer, Uint8Array, DataView, ArrayBuffer, or explicit byte array`,
      name,
    );
  }
  if (buffer.length !== length) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} must be ${length} bytes; received ${buffer.length}`,
      name,
    );
  }
  if (nonzero && buffer.every((byte) => byte === 0)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be nonzero`,
      name,
    );
  }
  return Array.from(buffer.values());
}

function normalizeVeRangeVersion(value, name) {
  const version = normalizePositiveU32(value ?? 1, name);
  if (version !== 1) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be 1`,
      name,
    );
  }
  return version;
}

function normalizeVeRangeBitLength(value, name) {
  const bitLength = normalizePositiveU32(value, name);
  if (bitLength > VERANGE_MAX_BIT_LENGTH) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} must be between 1 and ${VERANGE_MAX_BIT_LENGTH}`,
      name,
    );
  }
  return bitLength;
}

function normalizeVeRangeAggregationCount(value, name) {
  const count = normalizePositiveU32(value ?? 1, name);
  if (count > VERANGE_MAX_AGGREGATION_COUNT) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} must be between 1 and ${VERANGE_MAX_AGGREGATION_COUNT}`,
      name,
    );
  }
  return count;
}

function normalizeVeRangeCommitmentScheme(value, name) {
  const scheme = assertNonBlankString(value ?? "pedersen-v1", name).toLowerCase();
  if (!VERANGE_COMMITMENT_SCHEMES.has(scheme)) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be one of ${Array.from(VERANGE_COMMITMENT_SCHEMES).join(", ")}`,
      name,
    );
  }
  return scheme;
}

function normalizeVeRangePayloadBytes(value, aliasKey, name, maxPayloadBytes) {
  let bytes;
  if (aliasKey === "payloadJson" || aliasKey === "payload_json") {
    const normalizedJson = normalizeJsonValue(value, `${name}.payloadJson`);
    bytes = Array.from(
      Buffer.from(
        canonicalJsonStringify(normalizedJson, `${name}.payloadJson`),
        "utf8",
      ).values(),
    );
  } else {
    bytes = normalizeBoundedByteArray(value, `${name}.payload`, {
      maxBytes: maxPayloadBytes,
    });
  }
  if (bytes.length > maxPayloadBytes) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name}.payload must be no larger than ${maxPayloadBytes} bytes`,
      `${name}.payload`,
    );
  }
  return bytes;
}

function normalizeVeRangePayloadDigest(source, name) {
  const digestAlias = readSingleAlias(
    source,
    ["payloadDigest", "payload_digest", "txDigest", "tx_digest"],
    `${name}.payloadDigest`,
    "payload digest",
  );
  const payloadAlias = readSingleAlias(
    source,
    ["payload", "payloadBytes", "payload_bytes", "payloadJson", "payload_json"],
    `${name}.payload`,
    "payload",
  );
  const maxPayloadBytes = normalizePositiveU32AliasOrDefault(
    source,
    ["maxPayloadBytes", "max_payload_bytes"],
    `.maxPayloadBytes`,
    "max payload byte limit",
    VERANGE_MAX_PAYLOAD_BYTES,
  );
  const explicitDigest =
    digestAlias.value === undefined
      ? null
      : normalizeNonZeroFixedBytes(
          digestAlias.value,
          `${name}.payloadDigest`,
          32,
        );
  const payloadDigest =
    payloadAlias.value === undefined
      ? null
      : Array.from(
          createHash("sha256")
            .update(
              Buffer.from(
                normalizeVeRangePayloadBytes(
                  payloadAlias.value,
                  payloadAlias.key,
                  name,
                  maxPayloadBytes,
                ),
              ),
            )
            .digest()
            .values(),
        );
  if (explicitDigest === null && payloadDigest === null) {
    fail(
      ValidationErrorCode.MISSING_FIELD,
      `${name}.payloadDigest or ${name}.payload is required`,
      `${name}.payloadDigest`,
    );
  }
  if (
    explicitDigest !== null &&
    payloadDigest !== null &&
    !fixedBytesEqual(explicitDigest, payloadDigest)
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name}.payloadDigest must match the SHA-256 digest of ${name}.payload`,
      `${name}.payloadDigest`,
    );
  }
  return explicitDigest ?? payloadDigest;
}

function normalizeAnonymousPgcVersion(value, name) {
  const version = normalizePositiveU32(value ?? 1, name);
  if (version !== 1) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be 1`,
      name,
    );
  }
  return version;
}

function normalizeAnonymousPgcBackend(value, name) {
  const backendTag = normalizePrivacyBackendTag(value ?? ANON_PGC_BACKEND, name);
  if (backendTag !== "Stark") {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be ${ANON_PGC_BACKEND}`,
      name,
    );
  }
  return ANON_PGC_BACKEND;
}

function normalizeAnonymousPgcCircuitId(value, name) {
  const circuitId = assertNonBlankString(value ?? ANON_PGC_CIRCUIT_ID, name);
  if (
    circuitId !== ANON_PGC_CIRCUIT_ID &&
    circuitId !== "anonymous_pgc_k_out_of_n_v1"
  ) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must identify anonymous_pgc_k_out_of_n_v1`,
      name,
    );
  }
  return circuitId;
}

function normalizeAnonymousPgcPayloadDigest(source, name) {
  const digestAlias = readSingleAlias(
    source,
    ["txDigest", "tx_digest", "payloadDigest", "payload_digest"],
    `${name}.txDigest`,
    "transaction digest",
  );
  const payloadAlias = readSingleAlias(
    source,
    ["payload", "payloadBytes", "payload_bytes", "payloadJson", "payload_json"],
    `${name}.payload`,
    "payload",
  );
  const maxPayloadBytes = normalizePositiveU32AliasOrDefault(
    source,
    ["maxPayloadBytes", "max_payload_bytes"],
    `.maxPayloadBytes`,
    "max payload byte limit",
    VERANGE_MAX_PAYLOAD_BYTES,
  );
  const explicitDigest =
    digestAlias.value === undefined
      ? null
      : normalizeNonZeroFixedBytes(digestAlias.value, `${name}.txDigest`, 32);
  const payloadDigest =
    payloadAlias.value === undefined
      ? null
      : Array.from(
          createHash("sha256")
            .update(
              Buffer.from(
                normalizeVeRangePayloadBytes(
                  payloadAlias.value,
                  payloadAlias.key,
                  name,
                  maxPayloadBytes,
                ),
              ),
            )
            .digest()
            .values(),
        );
  if (explicitDigest === null && payloadDigest === null) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name}.txDigest or ${name}.payload is required`,
      `${name}.txDigest`,
    );
  }
  if (
    explicitDigest !== null &&
    payloadDigest !== null &&
    !fixedBytesEqual(explicitDigest, payloadDigest)
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name}.txDigest must match the SHA-256 digest of ${name}.payload`,
      `${name}.txDigest`,
    );
  }
  return explicitDigest ?? payloadDigest;
}

function normalizeAnonymousPgcCommitmentList(value, name, maxItems) {
  if (!Array.isArray(value) || value.length === 0) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be a non-empty array`,
      name,
    );
  }
  if (value.length > maxItems) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} must contain at most ${maxItems} entries`,
      name,
    );
  }
  const commitments = value.map((entry, index) => {
    let raw = entry;
    if (
      entry !== null &&
      typeof entry === "object" &&
      !Array.isArray(entry) &&
      !Buffer.isBuffer(entry) &&
      !ArrayBuffer.isView(entry) &&
      !(entry instanceof ArrayBuffer)
    ) {
      const source = assertPlainObject(entry, `${name}[${index}]`);
      const commitmentAlias = readSingleAlias(
        source,
        ["commitment", "rangeCommitment", "range_commitment", "valueCommitment", "value_commitment"],
        `${name}[${index}].commitment`,
        "commitment",
      );
      raw = commitmentAlias.value;
    }
    return normalizeNonZeroFixedBytes(raw, `${name}[${index}]`, 32);
  });
  const seen = new Set();
  for (const commitment of commitments) {
    const hex = Buffer.from(commitment).toString("hex");
    if (seen.has(hex)) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${name} must not contain duplicates`,
        name,
      );
    }
    seen.add(hex);
  }
  return commitments;
}

function normalizeAnonymousPgcReceiverEntry(value, name) {
  const source = assertPlainObject(value, name);
  assertAllowedFields(
    source,
    new Set([
      "accountCommitment",
      "account_commitment",
      "receiverCommitment",
      "receiver_commitment",
      "ciphertextCommitment",
      "ciphertext_commitment",
      "receiverCiphertextCommitment",
      "receiver_ciphertext_commitment",
      "ciphertext",
      "receiverCiphertext",
      "receiver_ciphertext",
      "encryptedNote",
      "encrypted_note",
      "ciphertextDigest",
      "ciphertext_digest",
    ]),
    name,
  );
  const accountCommitmentAlias = readSingleAlias(
    source,
    ["accountCommitment", "account_commitment", "receiverCommitment", "receiver_commitment"],
    `${name}.accountCommitment`,
    "receiver account commitment",
  );
  const ciphertextCommitmentAlias = readSingleAlias(
    source,
    [
      "ciphertextCommitment",
      "ciphertext_commitment",
      "receiverCiphertextCommitment",
      "receiver_ciphertext_commitment",
    ],
    `${name}.ciphertextCommitment`,
    "receiver ciphertext commitment",
  );
  const ciphertextAlias = readSingleAlias(
    source,
    ["ciphertext", "receiverCiphertext", "receiver_ciphertext", "encryptedNote", "encrypted_note"],
    `${name}.ciphertext`,
    "receiver ciphertext",
  );
  const ciphertextDigestAlias = readSingleAlias(
    source,
    ["ciphertextDigest", "ciphertext_digest"],
    `${name}.ciphertextDigest`,
    "receiver ciphertext digest",
  );
  const receiver = {
    account_commitment: normalizeNonZeroFixedBytes(
      accountCommitmentAlias.value,
      `${name}.accountCommitment`,
      32,
    ),
    ciphertext_commitment: normalizeNonZeroFixedBytes(
      ciphertextCommitmentAlias.value,
      `${name}.ciphertextCommitment`,
      32,
    ),
  };
  const suppliedDigest =
    ciphertextDigestAlias.value === undefined
      ? null
      : normalizeNonZeroFixedBytes(
          ciphertextDigestAlias.value,
          `${name}.ciphertextDigest`,
          32,
        );
  const computedDigest =
    ciphertextAlias.value === undefined
      ? null
      : Array.from(
          createHash("sha256")
            .update(
              Buffer.from(
                normalizeBoundedByteArray(ciphertextAlias.value, `${name}.ciphertext`, {
                  maxBytes: ANON_PGC_MAX_CIPHERTEXT_BYTES,
                }),
              ),
            )
            .digest()
            .values(),
        );
  if (
    suppliedDigest !== null &&
    computedDigest !== null &&
    !fixedBytesEqual(suppliedDigest, computedDigest)
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name}.ciphertextDigest must match the SHA-256 digest of ${name}.ciphertext`,
      `${name}.ciphertextDigest`,
    );
  }
  if (suppliedDigest !== null || computedDigest !== null) {
    receiver.ciphertext_digest = suppliedDigest ?? computedDigest;
  }
  return receiver;
}

function normalizeAnonymousPgcReceiverSet(value, name) {
  if (
    value !== null &&
    typeof value === "object" &&
    !Array.isArray(value) &&
    !Buffer.isBuffer(value) &&
    !ArrayBuffer.isView(value) &&
    !(value instanceof ArrayBuffer)
  ) {
    const source = assertPlainObject(value, name);
    const rebuilt = buildAnonymousPgcReceiverSet(
      pickPresentFields(source, ["version", "threshold", "k", "receivers"]),
      name,
    );
    if (Object.prototype.hasOwnProperty.call(source, "receiver_set_commitment")) {
      const supplied = normalizeNonZeroFixedBytes(
        source.receiver_set_commitment,
        `${name}.receiverSetCommitment`,
        32,
      );
      if (!fixedBytesEqual(supplied, rebuilt.receiver_set_commitment)) {
        fail(
          ValidationErrorCode.INVALID_OBJECT,
          `${name}.receiverSetCommitment must match receivers and threshold`,
          `${name}.receiverSetCommitment`,
        );
      }
    }
    if (
      Object.prototype.hasOwnProperty.call(source, "receiver_count") &&
      normalizePositiveU32(source.receiver_count, `${name}.receiverCount`) !==
        rebuilt.receiver_count
    ) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${name}.receiverCount must match receivers length`,
        `${name}.receiverCount`,
      );
    }
    return rebuilt;
  }
  fail(
    ValidationErrorCode.INVALID_OBJECT,
    `${name} must be a receiver-set object`,
    name,
  );
}

function anonymousPgcReceiverSetCommitment({ version, threshold, receivers }) {
  const payload = {
    version,
    receiver_count: receivers.length,
    threshold,
    receivers: receivers.map((entry) => ({
      account_commitment: Buffer.from(entry.account_commitment).toString("hex"),
      ciphertext_commitment: Buffer.from(entry.ciphertext_commitment).toString("hex"),
    })),
  };
  return Array.from(
    createHash("sha256")
      .update("iroha:anonymous-pgc:receiver-set:v1", "utf8")
      .update(Buffer.from([0]))
      .update(Buffer.from(canonicalJsonStringify(payload, "anonymousPgcReceiverSet.commitment"), "utf8"))
      .digest()
      .values(),
  );
}

function normalizeZkAtVersion(value, name) {
  const version = normalizePositiveU32(value ?? 1, name);
  if (version !== 1) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be 1`,
      name,
    );
  }
  return version;
}

function normalizeZkAtBackend(value, name) {
  const backendTag = normalizePrivacyBackendTag(value ?? ZKAT_BACKEND, name);
  if (backendTag !== "Stark") {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be ${ZKAT_BACKEND}`,
      name,
    );
  }
  return ZKAT_BACKEND;
}

function normalizeZkAtCircuitId(value, name) {
  const circuitId = assertNonBlankString(value ?? ZKAT_CIRCUIT_ID, name);
  if (circuitId !== ZKAT_CIRCUIT_ID && circuitId !== "zkat_policy_private_auth_v1") {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must identify zkat_policy_private_auth_v1`,
      name,
    );
  }
  return circuitId;
}

function normalizeZkAtPolicyEpoch(value, name) {
  return normalizePositiveU32(value, name);
}

function normalizeZkAtPolicyBytes(value, aliasKey, name, maxPolicyBytes) {
  let bytes;
  if (aliasKey === "policyJson" || aliasKey === "policy_json" || aliasKey === "policy") {
    const normalizedJson = normalizeJsonValue(value, `${name}.policyJson`);
    bytes = Array.from(
      Buffer.from(
        canonicalJsonStringify(normalizedJson, `${name}.policyJson`),
        "utf8",
      ).values(),
    );
  } else {
    bytes = normalizeBoundedByteArray(value, `${name}.policy`, {
      maxBytes: maxPolicyBytes,
    });
  }
  if (bytes.length > maxPolicyBytes) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name}.policy must be no larger than ${maxPolicyBytes} bytes`,
      `${name}.policy`,
    );
  }
  return bytes;
}

function zkatPolicyCommitmentBytes({
  policyBytes,
  policyEpoch,
  domainSeparator,
  policySchema,
}) {
  const digest = createHash("sha256");
  digest.update("iroha:zkat:policy-commitment:v1", "utf8");
  digest.update(Buffer.from([0]));
  digest.update(String(policyEpoch), "utf8");
  digest.update(Buffer.from([0]));
  digest.update(domainSeparator, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(policySchema, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(policyBytes));
  return Array.from(digest.digest().values());
}

function normalizeZkAtPolicyCommitmentFromSource(source, context) {
  const policyCommitmentAlias = readSingleAlias(
    source,
    ["policyCommitment", "policy_commitment", "commitment"],
    `${context}.policyCommitment`,
    "policy commitment",
  );
  const policyAlias = readSingleAlias(
    source,
    ["policy", "policyBytes", "policy_bytes", "policyJson", "policy_json"],
    `${context}.policy`,
    "policy",
  );
  if (policyCommitmentAlias.value !== undefined && policyAlias.value === undefined) {
    return normalizeNonZeroFixedBytes(
      policyCommitmentAlias.value,
      `${context}.policyCommitment`,
      32,
    );
  }
  const commitment = buildZkAtPolicyCommitment(
    pickPresentFields(source, [
      "version",
      "policyCommitment",
      "policy_commitment",
      "commitment",
      "policy",
      "policyBytes",
      "policy_bytes",
      "policyJson",
      "policy_json",
      "policyEpoch",
      "policy_epoch",
      "domainSeparator",
      "domain_separator",
      "policySchema",
      "policy_schema",
      "maxPolicyBytes",
      "max_policy_bytes",
    ]),
    `${context}.policyCommitment`,
  );
  return commitment.policy_commitment;
}

function normalizeZkAtPayloadDigest(source, name) {
  const digestAlias = readSingleAlias(
    source,
    ["txDigest", "tx_digest", "payloadDigest", "payload_digest"],
    `${name}.txDigest`,
    "transaction digest",
  );
  const payloadAlias = readSingleAlias(
    source,
    ["payload", "payloadBytes", "payload_bytes", "payloadJson", "payload_json"],
    `${name}.payload`,
    "payload",
  );
  const maxPayloadBytes = normalizePositiveU32AliasOrDefault(
    source,
    ["maxPayloadBytes", "max_payload_bytes"],
    `.maxPayloadBytes`,
    "max payload byte limit",
    VERANGE_MAX_PAYLOAD_BYTES,
  );
  const explicitDigest =
    digestAlias.value === undefined
      ? null
      : normalizeNonZeroFixedBytes(digestAlias.value, `${name}.txDigest`, 32);
  const payloadDigest =
    payloadAlias.value === undefined
      ? null
      : Array.from(
          createHash("sha256")
            .update(
              Buffer.from(
                normalizeVeRangePayloadBytes(
                  payloadAlias.value,
                  payloadAlias.key,
                  name,
                  maxPayloadBytes,
                ),
              ),
            )
            .digest()
            .values(),
        );
  if (explicitDigest === null && payloadDigest === null) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name}.txDigest or ${name}.payload is required`,
      `${name}.txDigest`,
    );
  }
  if (
    explicitDigest !== null &&
    payloadDigest !== null &&
    !fixedBytesEqual(explicitDigest, payloadDigest)
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name}.txDigest must match the SHA-256 digest of ${name}.payload`,
      `${name}.txDigest`,
    );
  }
  return explicitDigest ?? payloadDigest;
}

function normalizeZkAtPublicInputs(value, name) {
  const source = assertPlainObject(value, name);
  assertAllowedFields(
    source,
    new Set([
      "version",
      "policy_commitment",
      "policyCommitment",
      "tx_digest",
      "txDigest",
      "account_id",
      "accountId",
      "action_class",
      "actionClass",
      "domain_separator",
      "domainSeparator",
      "policy_epoch",
      "policyEpoch",
    ]),
    name,
  );
  const policyAlias = readSingleAlias(
    source,
    ["policy_commitment", "policyCommitment"],
    `${name}.policyCommitment`,
    "policy commitment",
  );
  const txDigestAlias = readSingleAlias(
    source,
    ["tx_digest", "txDigest"],
    `${name}.txDigest`,
    "transaction digest",
  );
  const accountAlias = readSingleAlias(
    source,
    ["account_id", "accountId"],
    `${name}.accountId`,
    "account id",
  );
  const actionAlias = readSingleAlias(
    source,
    ["action_class", "actionClass"],
    `${name}.actionClass`,
    "action class",
  );
  const domainAlias = readSingleAlias(
    source,
    ["domain_separator", "domainSeparator"],
    `${name}.domainSeparator`,
    "domain separator",
  );
  const epochAlias = readSingleAlias(
    source,
    ["policy_epoch", "policyEpoch"],
    `${name}.policyEpoch`,
    "policy epoch",
  );
  return {
    version: normalizeZkAtVersion(source.version, `${name}.version`),
    policy_commitment: Buffer.from(
      normalizeNonZeroFixedBytes(
        policyAlias.value,
        `${name}.policyCommitment`,
        32,
      ),
    ).toString("hex"),
    tx_digest: Buffer.from(
      normalizeNonZeroFixedBytes(txDigestAlias.value, `${name}.txDigest`, 32),
    ).toString("hex"),
    account_id: normalizeAccountId(accountAlias.value, `${name}.accountId`),
    action_class: assertNonBlankString(actionAlias.value, `${name}.actionClass`),
    domain_separator: assertNonBlankString(
      domainAlias.value,
      `${name}.domainSeparator`,
    ),
    policy_epoch: normalizeZkAtPolicyEpoch(
      epochAlias.value,
      `${name}.policyEpoch`,
    ),
  };
}

function normalizeZkAtAuthenticatorParts(
  source,
  context,
  { requireProofBytes = true } = {},
) {
  const backendAlias = readSingleAlias(
    source,
    ["backendTag", "backend_tag", "backend"],
    `${context}.backendTag`,
    "backend tag",
  );
  const circuitAlias = readSingleAlias(
    source,
    ["circuitId", "circuit_id"],
    `${context}.circuitId`,
    "circuit id",
  );
  const vkHashAlias = readSingleAlias(
    source,
    ["vkHash", "vk_hash", "verifierKeyHash", "verifyingKeyHash"],
    `${context}.vkHash`,
    "verifying key hash",
  );
  const proofAlias = readSingleAlias(
    source,
    ["proofBytes", "proof_bytes", "proof"],
    `${context}.proofBytes`,
    "proof bytes",
  );
  if (requireProofBytes && proofAlias.value === undefined) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.proofBytes is required`,
      `${context}.proofBytes`,
    );
  }
  const maxLimits = normalizeOptionalPrivacyMaxLimits(source, context);
  const policyEpoch = normalizeZkAtPolicyEpoch(
    source.policyEpoch ?? source.policy_epoch,
    `${context}.policyEpoch`,
  );
  const domainSeparator = assertNonBlankString(
    source.domainSeparator ?? source.domain_separator ?? ZKAT_DOMAIN_SEPARATOR,
    `${context}.domainSeparator`,
  );
  const publicInputs = {
    version: 1,
    policy_commitment: Buffer.from(
      normalizeZkAtPolicyCommitmentFromSource(
        {
          ...source,
          policyEpoch,
          domainSeparator,
        },
        context,
      ),
    ).toString("hex"),
    tx_digest: Buffer.from(normalizeZkAtPayloadDigest(source, context)).toString("hex"),
    account_id: normalizeAccountId(source.accountId ?? source.account_id, `${context}.accountId`),
    action_class: assertNonBlankString(
      source.actionClass ?? source.action_class,
      `${context}.actionClass`,
    ),
    domain_separator: domainSeparator,
    policy_epoch: policyEpoch,
  };
  const publicInputBytes = Array.from(
    Buffer.from(
      canonicalJsonStringify(publicInputs, `${context}.publicInputs`),
      "utf8",
    ).values(),
  );
  return {
    backend: normalizeZkAtBackend(backendAlias.value, `${context}.backendTag`),
    circuitId: normalizeZkAtCircuitId(
      circuitAlias.value,
      `${context}.circuitId`,
    ),
    vkHash: normalizeNonZeroFixedBytes(vkHashAlias.value, `${context}.vkHash`, 32),
    publicInputs,
    publicInputBytes,
    proofBytes:
      proofAlias.value === undefined
        ? null
        : normalizeBoundedByteArray(proofAlias.value, `${context}.proofBytes`, {
            maxBytes: maxLimits.maxProofBytes ?? DEFAULT_PRIVACY_MAX_PROOF_BYTES,
          }),
    maxProofBytes: maxLimits.maxProofBytes,
    maxPublicInputBytes: maxLimits.maxPublicInputBytes,
  };
}

function zkatDevProofBytes({ circuitId, vkHash, publicInputBytes }) {
  const digest = createHash("sha256");
  digest.update("iroha:zkat:dev-fixture:v1", "utf8");
  digest.update(Buffer.from([0]));
  digest.update(circuitId, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(vkHash));
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(publicInputBytes));
  return Array.from(
    Buffer.concat([ZKAT_DEV_PROOF_PREFIX, digest.digest()]).values(),
  );
}

function parseZkAtPublicInputs(bytes, name) {
  let parsed;
  try {
    parsed = JSON.parse(Buffer.from(bytes).toString("utf8"));
  } catch (error) {
    fail(
      ValidationErrorCode.INVALID_JSON_VALUE,
      `${name} must contain valid JSON public inputs`,
      name,
      error,
    );
  }
  const normalized = normalizeZkAtPublicInputs(parsed, name);
  const canonical = canonicalJsonStringify(normalized, name);
  if (!Buffer.from(bytes).equals(Buffer.from(canonical, "utf8"))) {
    fail(
      ValidationErrorCode.INVALID_JSON_VALUE,
      `${name} must use canonical JSON encoding`,
      name,
    );
  }
  return normalized;
}

function ensureZkAtVerificationExpectations(source, publicInputs, context) {
  if (
    hasPresentField(source, [
      "policyCommitment",
      "policy_commitment",
      "commitment",
      "policy",
      "policyBytes",
      "policy_bytes",
      "policyJson",
      "policy_json",
    ])
  ) {
    const expectedPolicy = Buffer.from(
      normalizeZkAtPolicyCommitmentFromSource(
        {
          ...source,
          policyEpoch: publicInputs.policy_epoch,
          domainSeparator: publicInputs.domain_separator,
        },
        context,
      ),
    ).toString("hex");
    if (expectedPolicy !== publicInputs.policy_commitment) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.policyCommitment must match the envelope public inputs`,
        `${context}.policyCommitment`,
      );
    }
  }
  if (
    hasPresentField(source, [
      "payload",
      "payloadBytes",
      "payload_bytes",
      "payloadJson",
      "payload_json",
      "txDigest",
      "tx_digest",
      "payloadDigest",
      "payload_digest",
    ])
  ) {
    const expectedDigest = normalizeZkAtPayloadDigest(source, context);
    if (Buffer.from(expectedDigest).toString("hex") !== publicInputs.tx_digest) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.txDigest must match the envelope public inputs`,
        `${context}.txDigest`,
      );
    }
  }
  const scalarChecks = [
    [
      ["accountId", "account_id"],
      "accountId",
      (value) => normalizeAccountId(value, `${context}.accountId`),
      publicInputs.account_id,
    ],
    [
      ["actionClass", "action_class"],
      "actionClass",
      (value) => assertNonBlankString(value, `${context}.actionClass`),
      publicInputs.action_class,
    ],
    [
      ["domainSeparator", "domain_separator"],
      "domainSeparator",
      (value) => assertNonBlankString(value, `${context}.domainSeparator`),
      publicInputs.domain_separator,
    ],
    [
      ["policyEpoch", "policy_epoch"],
      "policyEpoch",
      (value) => normalizeZkAtPolicyEpoch(value, `${context}.policyEpoch`),
      publicInputs.policy_epoch,
    ],
  ];
  for (const [fields, path, normalize, actual] of scalarChecks) {
    const alias = readSingleAlias(source, fields, `${context}.${path}`, path);
    if (alias.value !== undefined && normalize(alias.value) !== actual) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.${path} must match the envelope public inputs`,
        `${context}.${path}`,
      );
    }
  }
}

function normalizeZkAmsVersion(value, name) {
  const version = normalizePositiveU32(value ?? 1, name);
  if (version !== 1) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be 1`,
      name,
    );
  }
  return version;
}

function normalizeZkAmsBackend(value, name) {
  const backendTag = normalizePrivacyBackendTag(value ?? ZK_AMS_BACKEND, name);
  if (backendTag !== "Stark") {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be ${ZK_AMS_BACKEND}`,
      name,
    );
  }
  return ZK_AMS_BACKEND;
}

function normalizeZkAmsCircuitId(value, name) {
  const circuitId = assertNonBlankString(value ?? ZK_AMS_CIRCUIT_ID, name);
  if (
    circuitId !== ZK_AMS_CIRCUIT_ID &&
    circuitId !== "zk_ams_recursive_admission_v0"
  ) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must identify zk_ams_recursive_admission_v0`,
      name,
    );
  }
  return circuitId;
}

function normalizeZkAmsAdmissionList(value, name, maxItems) {
  if (!Array.isArray(value)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be an array`,
      name,
    );
  }
  if (value.length === 0) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must not be empty`,
      name,
    );
  }
  if (value.length > maxItems) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} must contain no more than ${maxItems} entries`,
      name,
    );
  }
  const seen = new Set();
  return value.map((entry, index) => {
    const bytes = normalizeNonZeroFixedBytes(entry, `${name}[${index}]`, 32);
    const hex = Buffer.from(bytes).toString("hex");
    if (seen.has(hex)) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${name} must not contain duplicate entries`,
        `${name}[${index}]`,
      );
    }
    seen.add(hex);
    return bytes;
  });
}

function normalizeZkAmsRecursiveProofDigest(source, context) {
  const digestAlias = readSingleAlias(
    source,
    ["recursiveProofDigest", "recursive_proof_digest"],
    `${context}.recursiveProofDigest`,
    "recursive proof digest",
  );
  const proofAlias = readSingleAlias(
    source,
    ["recursiveProof", "recursiveProofBytes", "recursive_proof", "recursive_proof_bytes"],
    `${context}.recursiveProof`,
    "recursive proof bytes",
  );
  const explicitDigest =
    digestAlias.value === undefined
      ? null
      : normalizeNonZeroFixedBytes(
          digestAlias.value,
          `${context}.recursiveProofDigest`,
          32,
        );
  const proofDigest =
    proofAlias.value === undefined
      ? null
      : Array.from(
          createHash("sha256")
            .update(
              Buffer.from(
                normalizeBoundedByteArray(
                  proofAlias.value,
                  `${context}.recursiveProof`,
                  {
                    maxBytes: normalizePositiveU32AliasOrDefault(
                      source,
                      ["maxRecursiveProofBytes", "max_recursive_proof_bytes"],
                      `${context}.maxRecursiveProofBytes`,
                      "max recursive proof byte limit",
                      ZK_AMS_MAX_RECURSIVE_PROOF_BYTES,
                    ),
                  },
                ),
              ),
            )
            .digest()
            .values(),
        );
  if (explicitDigest === null && proofDigest === null) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.recursiveProofDigest or ${context}.recursiveProof is required`,
      `${context}.recursiveProofDigest`,
    );
  }
  if (
    explicitDigest !== null &&
    proofDigest !== null &&
    !fixedBytesEqual(explicitDigest, proofDigest)
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.recursiveProofDigest must match the SHA-256 digest of ${context}.recursiveProof`,
      `${context}.recursiveProofDigest`,
    );
  }
  return explicitDigest ?? proofDigest;
}

function zkamAdmissionBatchRootBytes({
  issuerRoot,
  admissionNullifiers,
  anonymousAccountCommitments,
  recursiveProofDigest,
  domainSeparator,
}) {
  const payload = {
    issuer_root: Buffer.from(issuerRoot).toString("hex"),
    admission_nullifiers: admissionNullifiers.map((entry) =>
      Buffer.from(entry).toString("hex"),
    ),
    anonymous_account_commitments: anonymousAccountCommitments.map((entry) =>
      Buffer.from(entry).toString("hex"),
    ),
    recursive_proof_digest: Buffer.from(recursiveProofDigest).toString("hex"),
    domain_separator: domainSeparator,
  };
  return Array.from(
    createHash("sha256")
      .update("iroha:zk-ams:admission-batch-root:v0", "utf8")
      .update(Buffer.from([0]))
      .update(Buffer.from(canonicalJsonStringify(payload, "zkAmsAdmissionBatch.root"), "utf8"))
      .digest()
      .values(),
  );
}

function normalizeZkAmsAdmissionBatchParts(source, context) {
  const issuerRootAlias = readSingleAlias(
    source,
    ["issuerRoot", "issuer_root"],
    `${context}.issuerRoot`,
    "issuer root",
  );
  const batchRootAlias = readSingleAlias(
    source,
    ["admissionBatchRoot", "admission_batch_root", "batchRoot", "batch_root"],
    `${context}.admissionBatchRoot`,
    "admission batch root",
  );
  const nullifierAlias = readSingleAlias(
    source,
    ["admissionNullifiers", "admission_nullifiers", "nullifiers"],
    `${context}.admissionNullifiers`,
    "admission nullifiers",
  );
  const accountCommitmentAlias = readSingleAlias(
    source,
    [
      "anonymousAccountCommitments",
      "anonymous_account_commitments",
      "accountCommitments",
      "account_commitments",
    ],
    `${context}.anonymousAccountCommitments`,
    "anonymous account commitments",
  );
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  const maxBatchSize = normalizePositiveU32AliasOrDefault(
    source,
    ["maxBatchSize", "max_batch_size"],
    `${context}.maxBatchSize`,
    "max admission batch size",
    ZK_AMS_MAX_ADMISSIONS,
  );
  if (maxBatchSize > ZK_AMS_MAX_ADMISSIONS) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.maxBatchSize must be no greater than ${ZK_AMS_MAX_ADMISSIONS}`,
      `${context}.maxBatchSize`,
    );
  }
  const issuerRoot = normalizeNonZeroFixedBytes(
    issuerRootAlias.value,
    `${context}.issuerRoot`,
    32,
  );
  const admissionNullifiers = normalizeZkAmsAdmissionList(
    nullifierAlias.value,
    `${context}.admissionNullifiers`,
    maxBatchSize,
  );
  const anonymousAccountCommitments = normalizeZkAmsAdmissionList(
    accountCommitmentAlias.value,
    `${context}.anonymousAccountCommitments`,
    maxBatchSize,
  );
  if (admissionNullifiers.length !== anonymousAccountCommitments.length) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.admissionNullifiers length must match anonymousAccountCommitments length`,
      `${context}.admissionNullifiers`,
    );
  }
  const nullifierSet = new Set(
    admissionNullifiers.map((entry) => Buffer.from(entry).toString("hex")),
  );
  for (const [index, commitment] of anonymousAccountCommitments.entries()) {
    if (nullifierSet.has(Buffer.from(commitment).toString("hex"))) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.anonymousAccountCommitments must not overlap admissionNullifiers`,
        `${context}.anonymousAccountCommitments[${index}]`,
      );
    }
  }
  const recursiveProofDigest = normalizeZkAmsRecursiveProofDigest(source, context);
  const domainSeparator = assertNonBlankString(
    domainAlias.value ?? ZK_AMS_DOMAIN_SEPARATOR,
    `${context}.domainSeparator`,
  );
  const derivedBatchRoot = zkamAdmissionBatchRootBytes({
    issuerRoot,
    admissionNullifiers,
    anonymousAccountCommitments,
    recursiveProofDigest,
    domainSeparator,
  });
  const explicitBatchRoot =
    batchRootAlias.value === undefined
      ? null
      : normalizeNonZeroFixedBytes(
          batchRootAlias.value,
          `${context}.admissionBatchRoot`,
          32,
        );
  if (
    explicitBatchRoot !== null &&
    !fixedBytesEqual(explicitBatchRoot, derivedBatchRoot)
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.admissionBatchRoot must match the derived admission batch root`,
      `${context}.admissionBatchRoot`,
    );
  }
  return {
    version: normalizeZkAmsVersion(source.version, `${context}.version`),
    issuerRoot,
    admissionBatchRoot: explicitBatchRoot ?? derivedBatchRoot,
    admissionNullifiers,
    anonymousAccountCommitments,
    recursiveProofDigest,
    domainSeparator,
    batchSize: admissionNullifiers.length,
  };
}

function normalizeZkAmsPublicInputs(value, name) {
  const source = assertPlainObject(value, name);
  assertAllowedFields(
    source,
    new Set([
      "version",
      "issuer_root",
      "issuerRoot",
      "admission_batch_root",
      "admissionBatchRoot",
      "admission_nullifiers",
      "admissionNullifiers",
      "anonymous_account_commitments",
      "anonymousAccountCommitments",
      "recursive_proof_digest",
      "recursiveProofDigest",
      "domain_separator",
      "domainSeparator",
    ]),
    name,
  );
  const issuerRootAlias = readSingleAlias(
    source,
    ["issuer_root", "issuerRoot"],
    `${name}.issuerRoot`,
    "issuer root",
  );
  const batchRootAlias = readSingleAlias(
    source,
    ["admission_batch_root", "admissionBatchRoot"],
    `${name}.admissionBatchRoot`,
    "admission batch root",
  );
  const nullifierAlias = readSingleAlias(
    source,
    ["admission_nullifiers", "admissionNullifiers"],
    `${name}.admissionNullifiers`,
    "admission nullifiers",
  );
  const accountCommitmentAlias = readSingleAlias(
    source,
    ["anonymous_account_commitments", "anonymousAccountCommitments"],
    `${name}.anonymousAccountCommitments`,
    "anonymous account commitments",
  );
  const proofDigestAlias = readSingleAlias(
    source,
    ["recursive_proof_digest", "recursiveProofDigest"],
    `${name}.recursiveProofDigest`,
    "recursive proof digest",
  );
  const domainAlias = readSingleAlias(
    source,
    ["domain_separator", "domainSeparator"],
    `${name}.domainSeparator`,
    "domain separator",
  );
  const batch = normalizeZkAmsAdmissionBatchParts(
    {
      version: source.version,
      issuerRoot: issuerRootAlias.value,
      admissionBatchRoot: batchRootAlias.value,
      admissionNullifiers: nullifierAlias.value,
      anonymousAccountCommitments: accountCommitmentAlias.value,
      recursiveProofDigest: proofDigestAlias.value,
      domainSeparator: domainAlias.value,
    },
    name,
  );
  return {
    version: batch.version,
    issuer_root: Buffer.from(batch.issuerRoot).toString("hex"),
    admission_batch_root: Buffer.from(batch.admissionBatchRoot).toString("hex"),
    admission_nullifiers: batch.admissionNullifiers.map((entry) =>
      Buffer.from(entry).toString("hex"),
    ),
    anonymous_account_commitments: batch.anonymousAccountCommitments.map((entry) =>
      Buffer.from(entry).toString("hex"),
    ),
    recursive_proof_digest: Buffer.from(batch.recursiveProofDigest).toString("hex"),
    domain_separator: batch.domainSeparator,
  };
}

function normalizeZkAmsAdmissionProofParts(
  source,
  context,
  { requireProofBytes = true } = {},
) {
  const backendAlias = readSingleAlias(
    source,
    ["backendTag", "backend_tag", "backend"],
    `${context}.backendTag`,
    "backend tag",
  );
  const circuitAlias = readSingleAlias(
    source,
    ["circuitId", "circuit_id"],
    `${context}.circuitId`,
    "circuit id",
  );
  const vkHashAlias = readSingleAlias(
    source,
    ["vkHash", "vk_hash", "verifierKeyHash", "verifyingKeyHash"],
    `${context}.vkHash`,
    "verifying key hash",
  );
  const proofAlias = readSingleAlias(
    source,
    ["proofBytes", "proof_bytes", "proof"],
    `${context}.proofBytes`,
    "proof bytes",
  );
  if (requireProofBytes && proofAlias.value === undefined) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.proofBytes is required`,
      `${context}.proofBytes`,
    );
  }
  const maxLimits = normalizeOptionalPrivacyMaxLimits(source, context);
  const batch = normalizeZkAmsAdmissionBatchParts(source, context);
  const publicInputs = {
    version: batch.version,
    issuer_root: Buffer.from(batch.issuerRoot).toString("hex"),
    admission_batch_root: Buffer.from(batch.admissionBatchRoot).toString("hex"),
    admission_nullifiers: batch.admissionNullifiers.map((entry) =>
      Buffer.from(entry).toString("hex"),
    ),
    anonymous_account_commitments: batch.anonymousAccountCommitments.map((entry) =>
      Buffer.from(entry).toString("hex"),
    ),
    recursive_proof_digest: Buffer.from(batch.recursiveProofDigest).toString("hex"),
    domain_separator: batch.domainSeparator,
  };
  const publicInputBytes = Array.from(
    Buffer.from(
      canonicalJsonStringify(publicInputs, `${context}.publicInputs`),
      "utf8",
    ).values(),
  );
  return {
    backend: normalizeZkAmsBackend(backendAlias.value, `${context}.backendTag`),
    circuitId: normalizeZkAmsCircuitId(
      circuitAlias.value,
      `${context}.circuitId`,
    ),
    vkHash: normalizeNonZeroFixedBytes(vkHashAlias.value, `${context}.vkHash`, 32),
    batch,
    publicInputs,
    publicInputBytes,
    proofBytes:
      proofAlias.value === undefined
        ? null
        : normalizeBoundedByteArray(proofAlias.value, `${context}.proofBytes`, {
            maxBytes: maxLimits.maxProofBytes ?? DEFAULT_PRIVACY_MAX_PROOF_BYTES,
          }),
    maxProofBytes: maxLimits.maxProofBytes,
    maxPublicInputBytes: maxLimits.maxPublicInputBytes,
  };
}

function zkamDevProofBytes({ circuitId, vkHash, publicInputBytes }) {
  const digest = createHash("sha256");
  digest.update("iroha:zk-ams:dev-fixture:v0", "utf8");
  digest.update(Buffer.from([0]));
  digest.update(circuitId, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(vkHash));
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(publicInputBytes));
  return Array.from(
    Buffer.concat([ZK_AMS_DEV_PROOF_PREFIX, digest.digest()]).values(),
  );
}

function parseZkAmsPublicInputs(bytes, name) {
  let parsed;
  try {
    parsed = JSON.parse(Buffer.from(bytes).toString("utf8"));
  } catch (error) {
    fail(
      ValidationErrorCode.INVALID_JSON_VALUE,
      `${name} must contain valid JSON public inputs`,
      name,
      error,
    );
  }
  const normalized = normalizeZkAmsPublicInputs(parsed, name);
  const canonical = canonicalJsonStringify(normalized, name);
  if (!Buffer.from(bytes).equals(Buffer.from(canonical, "utf8"))) {
    fail(
      ValidationErrorCode.INVALID_JSON_VALUE,
      `${name} must use canonical JSON encoding`,
      name,
    );
  }
  return normalized;
}

function ensureZkAmsVerificationExpectations(source, publicInputs, context) {
  const scalarChecks = [
    [
      ["issuerRoot", "issuer_root"],
      "issuerRoot",
      (value) => Buffer.from(
        normalizeNonZeroFixedBytes(value, `${context}.issuerRoot`, 32),
      ).toString("hex"),
      publicInputs.issuer_root,
    ],
    [
      ["admissionBatchRoot", "admission_batch_root", "batchRoot", "batch_root"],
      "admissionBatchRoot",
      (value) => Buffer.from(
        normalizeNonZeroFixedBytes(value, `${context}.admissionBatchRoot`, 32),
      ).toString("hex"),
      publicInputs.admission_batch_root,
    ],
    [
      [
        "recursiveProofDigest",
        "recursive_proof_digest",
        "recursiveProof",
        "recursiveProofBytes",
        "recursive_proof",
        "recursive_proof_bytes",
      ],
      "recursiveProofDigest",
      () => Buffer.from(
        normalizeZkAmsRecursiveProofDigest(source, context),
      ).toString("hex"),
      publicInputs.recursive_proof_digest,
    ],
    [
      ["domainSeparator", "domain_separator"],
      "domainSeparator",
      (value) => assertNonBlankString(value, `${context}.domainSeparator`),
      publicInputs.domain_separator,
    ],
  ];
  for (const [fields, path, normalize, actual] of scalarChecks) {
    const alias = readSingleAlias(source, fields, `${context}.${path}`, path);
    if (alias.value !== undefined && normalize(alias.value) !== actual) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.${path} must match the envelope public inputs`,
        `${context}.${path}`,
      );
    }
  }
  const nullifierAlias = readSingleAlias(
    source,
    ["admissionNullifiers", "admission_nullifiers", "nullifiers"],
    `${context}.admissionNullifiers`,
    "admission nullifiers",
  );
  const maxBatchSize = normalizePositiveU32AliasOrDefault(
    source,
    ["maxBatchSize", "max_batch_size"],
    `${context}.maxBatchSize`,
    "max admission batch size",
    ZK_AMS_MAX_ADMISSIONS,
  );
  if (maxBatchSize > ZK_AMS_MAX_ADMISSIONS) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.maxBatchSize must be no greater than ${ZK_AMS_MAX_ADMISSIONS}`,
      `${context}.maxBatchSize`,
    );
  }
  if (nullifierAlias.value !== undefined) {
    const expected = normalizeZkAmsAdmissionList(
      nullifierAlias.value,
      `${context}.admissionNullifiers`,
      maxBatchSize,
    ).map((entry) => Buffer.from(entry).toString("hex"));
    if (
      expected.length !== publicInputs.admission_nullifiers.length ||
      expected.some((entry, index) => entry !== publicInputs.admission_nullifiers[index])
    ) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.admissionNullifiers must match the envelope public inputs`,
        `${context}.admissionNullifiers`,
      );
    }
  }
  const accountAlias = readSingleAlias(
    source,
    [
      "anonymousAccountCommitments",
      "anonymous_account_commitments",
      "accountCommitments",
      "account_commitments",
    ],
    `${context}.anonymousAccountCommitments`,
    "anonymous account commitments",
  );
  if (accountAlias.value !== undefined) {
    const expected = normalizeZkAmsAdmissionList(
      accountAlias.value,
      `${context}.anonymousAccountCommitments`,
      maxBatchSize,
    ).map((entry) => Buffer.from(entry).toString("hex"));
    if (
      expected.length !== publicInputs.anonymous_account_commitments.length ||
      expected.some(
        (entry, index) => entry !== publicInputs.anonymous_account_commitments[index],
      )
    ) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.anonymousAccountCommitments must match the envelope public inputs`,
        `${context}.anonymousAccountCommitments`,
      );
    }
  }
}

function normalizeVegaVersion(value, name) {
  const version = normalizePositiveU32(value ?? 1, name);
  if (version !== 1) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be 1`,
      name,
    );
  }
  return version;
}

function normalizeVegaBackend(value, name) {
  const backendTag = normalizePrivacyBackendTag(value ?? VEGA_BACKEND, name);
  if (backendTag !== "Stark") {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be ${VEGA_BACKEND}`,
      name,
    );
  }
  return VEGA_BACKEND;
}

function normalizeVegaCircuitId(value, name) {
  const circuitId = assertNonBlankString(value ?? VEGA_CIRCUIT_ID, name);
  if (
    circuitId !== VEGA_CIRCUIT_ID &&
    circuitId !== "vega_existing_credential_zk_v0"
  ) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must identify vega_existing_credential_zk_v0`,
      name,
    );
  }
  return circuitId;
}

function normalizeVegaCredentialSchema(value, name) {
  const schema = assertNonBlankString(value, name);
  if (schema.length > 256) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} must be no longer than 256 characters`,
      name,
    );
  }
  return schema;
}

function normalizeVegaExpirationEpoch(value, name) {
  return normalizeU32(value, name);
}

function normalizeVegaStructuredBytes(
  value,
  aliasKey,
  name,
  jsonPath,
  bytePath,
  maxBytes,
) {
  let bytes;
  if (
    aliasKey.endsWith("Json") ||
    aliasKey.endsWith("_json") ||
    aliasKey === "predicate" ||
    aliasKey === "issuer"
  ) {
    const normalizedJson = normalizeJsonValue(value, jsonPath);
    bytes = Array.from(
      Buffer.from(
        canonicalJsonStringify(normalizedJson, jsonPath),
        "utf8",
      ).values(),
    );
  } else {
    bytes = normalizeBoundedByteArray(value, bytePath, { maxBytes });
  }
  if (bytes.length > maxBytes) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${bytePath} must be no larger than ${maxBytes} bytes`,
      bytePath,
    );
  }
  return bytes;
}

function vegaPredicateCommitmentBytes({
  predicateBytes,
  credentialSchema,
  domainSeparator,
}) {
  const digest = createHash("sha256");
  digest.update("iroha:vega:predicate-commitment:v0", "utf8");
  digest.update(Buffer.from([0]));
  digest.update(credentialSchema, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(domainSeparator, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(predicateBytes));
  return Array.from(digest.digest().values());
}

function vegaIssuerCommitmentBytes({ issuerBytes, domainSeparator }) {
  const digest = createHash("sha256");
  digest.update("iroha:vega:issuer-commitment:v0", "utf8");
  digest.update(Buffer.from([0]));
  digest.update(domainSeparator, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(issuerBytes));
  return Array.from(digest.digest().values());
}

function vegaSubjectBindingBytes({ accountId, domainSeparator }) {
  const digest = createHash("sha256");
  digest.update("iroha:vega:subject-binding:v0", "utf8");
  digest.update(Buffer.from([0]));
  digest.update(domainSeparator, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(accountId, "utf8");
  return Array.from(digest.digest().values());
}

function normalizeVegaPredicateCommitmentFromSource(source, context) {
  const commitmentAlias = readSingleAlias(
    source,
    ["predicateCommitment", "predicate_commitment", "commitment"],
    `${context}.predicateCommitment`,
    "predicate commitment",
  );
  const predicateAlias = readSingleAlias(
    source,
    ["predicate", "predicateBytes", "predicate_bytes", "predicateJson", "predicate_json"],
    `${context}.predicate`,
    "predicate",
  );
  const schemaAlias = readSingleAlias(
    source,
    ["credentialSchema", "credential_schema"],
    `${context}.credentialSchema`,
    "credential schema",
  );
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  const schema = normalizeVegaCredentialSchema(
    schemaAlias.value,
    `${context}.credentialSchema`,
  );
  const domainSeparator = assertNonBlankString(
    domainAlias.value ?? VEGA_DOMAIN_SEPARATOR,
    `${context}.domainSeparator`,
  );
  const explicitCommitment =
    commitmentAlias.value === undefined
      ? null
      : normalizeNonZeroFixedBytes(
          commitmentAlias.value,
          `${context}.predicateCommitment`,
          32,
        );
  const predicateBytes =
    predicateAlias.value === undefined
      ? null
      : normalizeVegaStructuredBytes(
          predicateAlias.value,
          predicateAlias.key,
          context,
          `${context}.predicateJson`,
          `${context}.predicate`,
          normalizePositiveU32AliasOrDefault(
            source,
            ["maxPredicateBytes", "max_predicate_bytes"],
            `${context}.maxPredicateBytes`,
            "max predicate byte limit",
            VEGA_MAX_PREDICATE_BYTES,
          ),
        );
  if (explicitCommitment === null && predicateBytes === null) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.predicateCommitment or ${context}.predicate is required`,
      `${context}.predicateCommitment`,
    );
  }
  const derivedCommitment =
    predicateBytes === null
      ? null
      : vegaPredicateCommitmentBytes({
          predicateBytes,
          credentialSchema: schema,
          domainSeparator,
        });
  if (
    explicitCommitment !== null &&
    derivedCommitment !== null &&
    !fixedBytesEqual(explicitCommitment, derivedCommitment)
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.predicateCommitment must match the derived predicate commitment`,
      `${context}.predicateCommitment`,
    );
  }
  return {
    version: normalizeVegaVersion(source.version, `${context}.version`),
    predicate_commitment: explicitCommitment ?? derivedCommitment,
    credential_schema: schema,
    domain_separator: domainSeparator,
    commitment_kind:
      predicateBytes === null ? "external" : "dev-sha256-predicate-digest",
    predicate_digest:
      predicateBytes === null
        ? null
        : Array.from(createHash("sha256").update(Buffer.from(predicateBytes)).digest().values()),
  };
}

function normalizeVegaIssuerCommitmentFromSource(source, context) {
  const commitmentAlias = readSingleAlias(
    source,
    ["issuerCommitment", "issuer_commitment"],
    `${context}.issuerCommitment`,
    "issuer commitment",
  );
  const issuerAlias = readSingleAlias(
    source,
    ["issuer", "issuerBytes", "issuer_bytes", "issuerJson", "issuer_json"],
    `${context}.issuer`,
    "issuer",
  );
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  const domainSeparator = assertNonBlankString(
    domainAlias.value ?? VEGA_DOMAIN_SEPARATOR,
    `${context}.domainSeparator`,
  );
  const explicitCommitment =
    commitmentAlias.value === undefined
      ? null
      : normalizeNonZeroFixedBytes(
          commitmentAlias.value,
          `${context}.issuerCommitment`,
          32,
        );
  const issuerBytes =
    issuerAlias.value === undefined
      ? null
      : normalizeVegaStructuredBytes(
          issuerAlias.value,
          issuerAlias.key,
          context,
          `${context}.issuerJson`,
          `${context}.issuer`,
          normalizePositiveU32AliasOrDefault(
            source,
            ["maxIssuerBytes", "max_issuer_bytes"],
            `${context}.maxIssuerBytes`,
            "max issuer byte limit",
            VEGA_MAX_ISSUER_BYTES,
          ),
        );
  if (explicitCommitment === null && issuerBytes === null) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.issuerCommitment or ${context}.issuer is required`,
      `${context}.issuerCommitment`,
    );
  }
  const derivedCommitment =
    issuerBytes === null
      ? null
      : vegaIssuerCommitmentBytes({ issuerBytes, domainSeparator });
  if (
    explicitCommitment !== null &&
    derivedCommitment !== null &&
    !fixedBytesEqual(explicitCommitment, derivedCommitment)
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.issuerCommitment must match the derived issuer commitment`,
      `${context}.issuerCommitment`,
    );
  }
  return explicitCommitment ?? derivedCommitment;
}

function normalizeVegaSubjectBindingFromSource(source, context) {
  const bindingAlias = readSingleAlias(
    source,
    [
      "subjectBinding",
      "subject_binding",
      "identityCommitment",
      "identity_commitment",
      "accountCommitment",
      "account_commitment",
    ],
    `${context}.subjectBinding`,
    "subject binding",
  );
  const accountAlias = readSingleAlias(
    source,
    ["accountId", "account_id", "subjectAccountId", "subject_account_id"],
    `${context}.accountId`,
    "account id",
  );
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  const domainSeparator = assertNonBlankString(
    domainAlias.value ?? VEGA_DOMAIN_SEPARATOR,
    `${context}.domainSeparator`,
  );
  const explicitBinding =
    bindingAlias.value === undefined
      ? null
      : normalizeNonZeroFixedBytes(bindingAlias.value, `${context}.subjectBinding`, 32);
  const accountBinding =
    accountAlias.value === undefined
      ? null
      : vegaSubjectBindingBytes({
          accountId: normalizeAccountId(accountAlias.value, `${context}.accountId`),
          domainSeparator,
        });
  if (explicitBinding === null && accountBinding === null) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.subjectBinding or ${context}.accountId is required`,
      `${context}.subjectBinding`,
    );
  }
  if (
    explicitBinding !== null &&
    accountBinding !== null &&
    !fixedBytesEqual(explicitBinding, accountBinding)
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.subjectBinding must match the derived account subject binding`,
      `${context}.subjectBinding`,
    );
  }
  return explicitBinding ?? accountBinding;
}

function normalizeVegaPublicInputs(value, name) {
  const source = assertPlainObject(value, name);
  assertAllowedFields(
    source,
    new Set([
      "version",
      "issuer_commitment",
      "issuerCommitment",
      "credential_schema",
      "credentialSchema",
      "predicate_commitment",
      "predicateCommitment",
      "subject_binding",
      "subjectBinding",
      "expiration_epoch",
      "expirationEpoch",
      "domain_separator",
      "domainSeparator",
    ]),
    name,
  );
  const issuerAlias = readSingleAlias(
    source,
    ["issuer_commitment", "issuerCommitment"],
    `${name}.issuerCommitment`,
    "issuer commitment",
  );
  const schemaAlias = readSingleAlias(
    source,
    ["credential_schema", "credentialSchema"],
    `${name}.credentialSchema`,
    "credential schema",
  );
  const predicateAlias = readSingleAlias(
    source,
    ["predicate_commitment", "predicateCommitment"],
    `${name}.predicateCommitment`,
    "predicate commitment",
  );
  const subjectAlias = readSingleAlias(
    source,
    ["subject_binding", "subjectBinding"],
    `${name}.subjectBinding`,
    "subject binding",
  );
  const expirationAlias = readSingleAlias(
    source,
    ["expiration_epoch", "expirationEpoch"],
    `${name}.expirationEpoch`,
    "expiration epoch",
  );
  const domainAlias = readSingleAlias(
    source,
    ["domain_separator", "domainSeparator"],
    `${name}.domainSeparator`,
    "domain separator",
  );
  return {
    version: normalizeVegaVersion(source.version, `${name}.version`),
    issuer_commitment: Buffer.from(
      normalizeNonZeroFixedBytes(
        issuerAlias.value,
        `${name}.issuerCommitment`,
        32,
      ),
    ).toString("hex"),
    credential_schema: normalizeVegaCredentialSchema(
      schemaAlias.value,
      `${name}.credentialSchema`,
    ),
    predicate_commitment: Buffer.from(
      normalizeNonZeroFixedBytes(
        predicateAlias.value,
        `${name}.predicateCommitment`,
        32,
      ),
    ).toString("hex"),
    subject_binding: Buffer.from(
      normalizeNonZeroFixedBytes(
        subjectAlias.value,
        `${name}.subjectBinding`,
        32,
      ),
    ).toString("hex"),
    expiration_epoch: normalizeVegaExpirationEpoch(
      expirationAlias.value,
      `${name}.expirationEpoch`,
    ),
    domain_separator: assertNonBlankString(
      domainAlias.value,
      `${name}.domainSeparator`,
    ),
  };
}

function normalizeVegaProofParts(
  source,
  context,
  { requireProofBytes = true } = {},
) {
  const backendAlias = readSingleAlias(
    source,
    ["backendTag", "backend_tag", "backend"],
    `${context}.backendTag`,
    "backend tag",
  );
  const circuitAlias = readSingleAlias(
    source,
    ["circuitId", "circuit_id"],
    `${context}.circuitId`,
    "circuit id",
  );
  const vkHashAlias = readSingleAlias(
    source,
    ["vkHash", "vk_hash", "verifierKeyHash", "verifyingKeyHash"],
    `${context}.vkHash`,
    "verifying key hash",
  );
  const proofAlias = readSingleAlias(
    source,
    ["proofBytes", "proof_bytes", "proof"],
    `${context}.proofBytes`,
    "proof bytes",
  );
  if (requireProofBytes && proofAlias.value === undefined) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.proofBytes is required`,
      `${context}.proofBytes`,
    );
  }
  const maxLimits = normalizeOptionalPrivacyMaxLimits(source, context);
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  const schemaAlias = readSingleAlias(
    source,
    ["credentialSchema", "credential_schema"],
    `${context}.credentialSchema`,
    "credential schema",
  );
  const expirationAlias = readSingleAlias(
    source,
    ["expirationEpoch", "expiration_epoch"],
    `${context}.expirationEpoch`,
    "expiration epoch",
  );
  const domainSeparator = assertNonBlankString(
    domainAlias.value ?? VEGA_DOMAIN_SEPARATOR,
    `${context}.domainSeparator`,
  );
  const credentialSchema = normalizeVegaCredentialSchema(
    schemaAlias.value,
    `${context}.credentialSchema`,
  );
  const publicInputs = {
    version: 1,
    issuer_commitment: Buffer.from(
      normalizeVegaIssuerCommitmentFromSource(
        source,
        context,
      ),
    ).toString("hex"),
    credential_schema: credentialSchema,
    predicate_commitment: Buffer.from(
      normalizeVegaPredicateCommitmentFromSource(
        source,
        context,
      ).predicate_commitment,
    ).toString("hex"),
    subject_binding: Buffer.from(
      normalizeVegaSubjectBindingFromSource(
        source,
        context,
      ),
    ).toString("hex"),
    expiration_epoch: normalizeVegaExpirationEpoch(
      expirationAlias.value,
      `${context}.expirationEpoch`,
    ),
    domain_separator: domainSeparator,
  };
  const publicInputBytes = Array.from(
    Buffer.from(
      canonicalJsonStringify(publicInputs, `${context}.publicInputs`),
      "utf8",
    ).values(),
  );
  return {
    backend: normalizeVegaBackend(backendAlias.value, `${context}.backendTag`),
    circuitId: normalizeVegaCircuitId(
      circuitAlias.value,
      `${context}.circuitId`,
    ),
    vkHash: normalizeNonZeroFixedBytes(vkHashAlias.value, `${context}.vkHash`, 32),
    publicInputs,
    publicInputBytes,
    proofBytes:
      proofAlias.value === undefined
        ? null
        : normalizeBoundedByteArray(proofAlias.value, `${context}.proofBytes`, {
            maxBytes: maxLimits.maxProofBytes ?? DEFAULT_PRIVACY_MAX_PROOF_BYTES,
          }),
    maxProofBytes: maxLimits.maxProofBytes,
    maxPublicInputBytes: maxLimits.maxPublicInputBytes,
  };
}

function vegaDevProofBytes({ circuitId, vkHash, publicInputBytes }) {
  const digest = createHash("sha256");
  digest.update("iroha:vega:dev-fixture:v0", "utf8");
  digest.update(Buffer.from([0]));
  digest.update(circuitId, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(vkHash));
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(publicInputBytes));
  return Array.from(
    Buffer.concat([VEGA_DEV_PROOF_PREFIX, digest.digest()]).values(),
  );
}

function parseVegaPublicInputs(bytes, name) {
  let parsed;
  try {
    parsed = JSON.parse(Buffer.from(bytes).toString("utf8"));
  } catch (error) {
    fail(
      ValidationErrorCode.INVALID_JSON_VALUE,
      `${name} must contain valid JSON public inputs`,
      name,
      error,
    );
  }
  const normalized = normalizeVegaPublicInputs(parsed, name);
  const canonical = canonicalJsonStringify(normalized, name);
  if (!Buffer.from(bytes).equals(Buffer.from(canonical, "utf8"))) {
    fail(
      ValidationErrorCode.INVALID_JSON_VALUE,
      `${name} must use canonical JSON encoding`,
      name,
    );
  }
  return normalized;
}

function ensureVegaVerificationExpectations(source, publicInputs, context) {
  const expectationSource = { ...source };
  if (!hasPresentField(expectationSource, ["domainSeparator", "domain_separator"])) {
    expectationSource.domainSeparator = publicInputs.domain_separator;
  }
  if (!hasPresentField(expectationSource, ["credentialSchema", "credential_schema"])) {
    expectationSource.credentialSchema = publicInputs.credential_schema;
  }
  if (
    hasPresentField(source, [
      "issuerCommitment",
      "issuer_commitment",
      "issuer",
      "issuerBytes",
      "issuer_bytes",
      "issuerJson",
      "issuer_json",
    ])
  ) {
    const expectedIssuer = Buffer.from(
      normalizeVegaIssuerCommitmentFromSource(
        expectationSource,
        context,
      ),
    ).toString("hex");
    if (expectedIssuer !== publicInputs.issuer_commitment) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.issuerCommitment must match the envelope public inputs`,
        `${context}.issuerCommitment`,
      );
    }
  }
  if (
    hasPresentField(source, [
      "predicateCommitment",
      "predicate_commitment",
      "commitment",
      "predicate",
      "predicateBytes",
      "predicate_bytes",
      "predicateJson",
      "predicate_json",
    ])
  ) {
    const expectedPredicate = Buffer.from(
      normalizeVegaPredicateCommitmentFromSource(
        expectationSource,
        context,
      ).predicate_commitment,
    ).toString("hex");
    if (expectedPredicate !== publicInputs.predicate_commitment) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.predicateCommitment must match the envelope public inputs`,
        `${context}.predicateCommitment`,
      );
    }
  }
  if (
    hasPresentField(source, [
      "subjectBinding",
      "subject_binding",
      "identityCommitment",
      "identity_commitment",
      "accountCommitment",
      "account_commitment",
      "accountId",
      "account_id",
      "subjectAccountId",
      "subject_account_id",
    ])
  ) {
    const expectedSubject = Buffer.from(
      normalizeVegaSubjectBindingFromSource(
        expectationSource,
        context,
      ),
    ).toString("hex");
    if (expectedSubject !== publicInputs.subject_binding) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.subjectBinding must match the envelope public inputs`,
        `${context}.subjectBinding`,
      );
    }
  }
  const scalarChecks = [
    [
      ["credentialSchema", "credential_schema"],
      "credentialSchema",
      (value) => normalizeVegaCredentialSchema(value, `${context}.credentialSchema`),
      publicInputs.credential_schema,
    ],
    [
      ["expirationEpoch", "expiration_epoch"],
      "expirationEpoch",
      (value) => normalizeVegaExpirationEpoch(value, `${context}.expirationEpoch`),
      publicInputs.expiration_epoch,
    ],
    [
      ["domainSeparator", "domain_separator"],
      "domainSeparator",
      (value) => assertNonBlankString(value, `${context}.domainSeparator`),
      publicInputs.domain_separator,
    ],
  ];
  for (const [fields, path, normalize, actual] of scalarChecks) {
    const alias = readSingleAlias(source, fields, `${context}.${path}`, path);
    if (alias.value !== undefined && normalize(alias.value) !== actual) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.${path} must match the envelope public inputs`,
        `${context}.${path}`,
      );
    }
  }
}

function normalizeSilentThresholdVersion(value, name) {
  const version = normalizePositiveU32(value ?? 1, name);
  if (version !== 1) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be 1`,
      name,
    );
  }
  return version;
}

function normalizeSilentThresholdBackend(value, name) {
  const backendTag = normalizePrivacyBackendTag(
    value ?? SILENT_THRESHOLD_BACKEND,
    name,
  );
  if (backendTag !== "Stark") {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be ${SILENT_THRESHOLD_BACKEND}`,
      name,
    );
  }
  return SILENT_THRESHOLD_BACKEND;
}

function normalizeSilentThresholdCircuitId(value, name) {
  const circuitId = assertNonBlankString(
    value ?? SILENT_THRESHOLD_CIRCUIT_ID,
    name,
  );
  if (
    circuitId !== SILENT_THRESHOLD_CIRCUIT_ID &&
    circuitId !== "silent_threshold_anoncred_v0"
  ) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must identify silent_threshold_anoncred_v0`,
      name,
    );
  }
  return circuitId;
}

function normalizeSilentThresholdStructuredBytes(
  value,
  aliasKey,
  jsonAliases,
  jsonPath,
  bytePath,
  maxBytes,
) {
  let bytes;
  if (
    aliasKey.endsWith("Json") ||
    aliasKey.endsWith("_json") ||
    jsonAliases.has(aliasKey)
  ) {
    const normalizedJson = normalizeJsonValue(value, jsonPath);
    bytes = Array.from(
      Buffer.from(
        canonicalJsonStringify(normalizedJson, jsonPath),
        "utf8",
      ).values(),
    );
  } else {
    bytes = normalizeBoundedByteArray(value, bytePath, { maxBytes });
  }
  if (bytes.length > maxBytes) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${bytePath} must be no larger than ${maxBytes} bytes`,
      bytePath,
    );
  }
  return bytes;
}

function silentThresholdDigestBytes(label, bytes, domainSeparator) {
  const digest = createHash("sha256");
  digest.update(`iroha:silent-threshold:${label}:v0`, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(domainSeparator, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(bytes));
  return Array.from(digest.digest().values());
}

function normalizeSilentThresholdDerivedField(
  source,
  context,
  {
    explicitAliases,
    dataAliases,
    jsonAliases,
    fieldPath,
    dataPath,
    dataJsonPath,
    dataDescription,
    digestLabel,
    kindLabel,
    maxBytes,
  },
) {
  const explicitAlias = readSingleAlias(
    source,
    explicitAliases,
    `${context}.${fieldPath}`,
    dataDescription,
  );
  const dataAlias = readSingleAlias(
    source,
    dataAliases,
    `${context}.${dataPath}`,
    dataDescription,
  );
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  const domainSeparator = assertNonBlankString(
    domainAlias.value ?? SILENT_THRESHOLD_DOMAIN_SEPARATOR,
    `${context}.domainSeparator`,
  );
  const explicit =
    explicitAlias.value === undefined
      ? null
      : normalizeNonZeroFixedBytes(
          explicitAlias.value,
          `${context}.${fieldPath}`,
          32,
        );
  const dataBytes =
    dataAlias.value === undefined
      ? null
      : normalizeSilentThresholdStructuredBytes(
          dataAlias.value,
          dataAlias.key,
          jsonAliases,
          `${context}.${dataJsonPath}`,
          `${context}.${dataPath}`,
          normalizePositiveU32(maxBytes, `${context}.maxBytes`),
        );
  if (explicit === null && dataBytes === null) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.${fieldPath} or ${context}.${dataPath} is required`,
      `${context}.${fieldPath}`,
    );
  }
  const derived =
    dataBytes === null
      ? null
      : silentThresholdDigestBytes(digestLabel, dataBytes, domainSeparator);
  if (explicit !== null && derived !== null && !fixedBytesEqual(explicit, derived)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.${fieldPath} must match the derived ${dataDescription}`,
      `${context}.${fieldPath}`,
    );
  }
  return {
    value: explicit ?? derived,
    kind: dataBytes === null ? "external" : `dev-sha256-${kindLabel}`,
    digest:
      dataBytes === null
        ? null
        : Array.from(createHash("sha256").update(Buffer.from(dataBytes)).digest().values()),
  };
}

function silentThresholdIssuerSetField(source, context) {
  return normalizeSilentThresholdDerivedField(source, context, {
    explicitAliases: ["issuerSetCommitment", "issuer_set_commitment"],
    dataAliases: [
      "issuerSet",
      "issuer_set",
      "issuerSetBytes",
      "issuer_set_bytes",
      "issuerSetJson",
      "issuer_set_json",
    ],
    jsonAliases: new Set(["issuerSet", "issuer_set"]),
    fieldPath: "issuerSetCommitment",
    dataPath: "issuerSet",
    dataJsonPath: "issuerSetJson",
    dataDescription: "issuer-set commitment",
    digestLabel: "issuer-set-commitment",
    kindLabel: "issuer-set-digest",
    maxBytes: normalizePositiveU32AliasOrDefault(
      source,
      ["maxIssuerSetBytes", "max_issuer_set_bytes"],
      `${context}.maxIssuerSetBytes`,
      "max issuer-set byte limit",
      SILENT_THRESHOLD_MAX_ISSUER_SET_BYTES,
    ),
  });
}

function silentThresholdPolicyHashField(source, context) {
  return normalizeSilentThresholdDerivedField(source, context, {
    explicitAliases: ["thresholdPolicyHash", "threshold_policy_hash"],
    dataAliases: [
      "thresholdPolicy",
      "threshold_policy",
      "thresholdPolicyBytes",
      "threshold_policy_bytes",
      "thresholdPolicyJson",
      "threshold_policy_json",
    ],
    jsonAliases: new Set(["thresholdPolicy", "threshold_policy"]),
    fieldPath: "thresholdPolicyHash",
    dataPath: "thresholdPolicy",
    dataJsonPath: "thresholdPolicyJson",
    dataDescription: "threshold policy hash",
    digestLabel: "threshold-policy-hash",
    kindLabel: "threshold-policy-digest",
    maxBytes: normalizePositiveU32AliasOrDefault(
      source,
      ["maxPolicyBytes", "max_policy_bytes"],
      `${context}.maxPolicyBytes`,
      "max policy byte limit",
      SILENT_THRESHOLD_MAX_POLICY_BYTES,
    ),
  });
}

function silentThresholdShowingCommitmentField(source, context) {
  return normalizeSilentThresholdDerivedField(source, context, {
    explicitAliases: [
      "credentialShowingCommitment",
      "credential_showing_commitment",
      "showingCommitment",
      "showing_commitment",
    ],
    dataAliases: [
      "credentialShowing",
      "credential_showing",
      "credentialShowingBytes",
      "credential_showing_bytes",
      "credentialShowingJson",
      "credential_showing_json",
      "showing",
      "showingBytes",
      "showing_bytes",
      "showingJson",
      "showing_json",
    ],
    jsonAliases: new Set(["credentialShowing", "credential_showing", "showing"]),
    fieldPath: "credentialShowingCommitment",
    dataPath: "credentialShowing",
    dataJsonPath: "credentialShowingJson",
    dataDescription: "credential showing commitment",
    digestLabel: "credential-showing-commitment",
    kindLabel: "credential-showing-digest",
    maxBytes: normalizePositiveU32AliasOrDefault(
      source,
      ["maxShowingBytes", "max_showing_bytes"],
      `${context}.maxShowingBytes`,
      "max showing byte limit",
      SILENT_THRESHOLD_MAX_SHOWING_BYTES,
    ),
  });
}

function silentThresholdShowingNullifierField(source, context) {
  return normalizeSilentThresholdDerivedField(source, context, {
    explicitAliases: [
      "showingNullifier",
      "showing_nullifier",
      "credentialShowingNullifier",
      "credential_showing_nullifier",
      "nullifier",
    ],
    dataAliases: [
      "credentialShowing",
      "credential_showing",
      "credentialShowingBytes",
      "credential_showing_bytes",
      "credentialShowingJson",
      "credential_showing_json",
      "showing",
      "showingBytes",
      "showing_bytes",
      "showingJson",
      "showing_json",
    ],
    jsonAliases: new Set(["credentialShowing", "credential_showing", "showing"]),
    fieldPath: "showingNullifier",
    dataPath: "credentialShowing",
    dataJsonPath: "credentialShowingJson",
    dataDescription: "credential showing nullifier",
    digestLabel: "credential-showing-nullifier",
    kindLabel: "credential-showing-nullifier",
    maxBytes: normalizePositiveU32AliasOrDefault(
      source,
      ["maxShowingBytes", "max_showing_bytes"],
      `${context}.maxShowingBytes`,
      "max showing byte limit",
      SILENT_THRESHOLD_MAX_SHOWING_BYTES,
    ),
  });
}

function silentThresholdVerifierPolicyHashField(source, context) {
  return normalizeSilentThresholdDerivedField(source, context, {
    explicitAliases: ["verifierPolicyHash", "verifier_policy_hash"],
    dataAliases: [
      "verifierPolicy",
      "verifier_policy",
      "verifierPolicyBytes",
      "verifier_policy_bytes",
      "verifierPolicyJson",
      "verifier_policy_json",
    ],
    jsonAliases: new Set(["verifierPolicy", "verifier_policy"]),
    fieldPath: "verifierPolicyHash",
    dataPath: "verifierPolicy",
    dataJsonPath: "verifierPolicyJson",
    dataDescription: "verifier policy hash",
    digestLabel: "verifier-policy-hash",
    kindLabel: "verifier-policy-digest",
    maxBytes: normalizePositiveU32AliasOrDefault(
      source,
      ["maxPolicyBytes", "max_policy_bytes"],
      `${context}.maxPolicyBytes`,
      "max policy byte limit",
      SILENT_THRESHOLD_MAX_POLICY_BYTES,
    ),
  });
}

function normalizeSilentThresholdCommitmentParts(source, context) {
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  const domainSeparator = assertNonBlankString(
    domainAlias.value ?? SILENT_THRESHOLD_DOMAIN_SEPARATOR,
    `${context}.domainSeparator`,
  );
  const normalized = { ...source, domainSeparator };
  const issuerSet = silentThresholdIssuerSetField(normalized, context);
  const thresholdPolicy = silentThresholdPolicyHashField(normalized, context);
  const showing = silentThresholdShowingCommitmentField(normalized, context);
  const showingNullifier = silentThresholdShowingNullifierField(normalized, context);
  const verifierPolicy = silentThresholdVerifierPolicyHashField(normalized, context);
  return {
    version: normalizeSilentThresholdVersion(source.version, `${context}.version`),
    issuerSet,
    thresholdPolicy,
    showing,
    showingNullifier,
    verifierPolicy,
    domainSeparator,
  };
}

function normalizeSilentThresholdPublicInputs(value, name) {
  const source = assertPlainObject(value, name);
  assertAllowedFields(
    source,
    new Set([
      "version",
      "issuer_set_commitment",
      "issuerSetCommitment",
      "threshold_policy_hash",
      "thresholdPolicyHash",
      "credential_showing_commitment",
      "credentialShowingCommitment",
      "showing_nullifier",
      "showingNullifier",
      "credential_showing_nullifier",
      "credentialShowingNullifier",
      "verifier_policy_hash",
      "verifierPolicyHash",
      "domain_separator",
      "domainSeparator",
    ]),
    name,
  );
  const issuerAlias = readSingleAlias(
    source,
    ["issuer_set_commitment", "issuerSetCommitment"],
    `${name}.issuerSetCommitment`,
    "issuer-set commitment",
  );
  const thresholdAlias = readSingleAlias(
    source,
    ["threshold_policy_hash", "thresholdPolicyHash"],
    `${name}.thresholdPolicyHash`,
    "threshold policy hash",
  );
  const showingAlias = readSingleAlias(
    source,
    ["credential_showing_commitment", "credentialShowingCommitment"],
    `${name}.credentialShowingCommitment`,
    "credential showing commitment",
  );
  const nullifierAlias = readSingleAlias(
    source,
    [
      "showing_nullifier",
      "showingNullifier",
      "credential_showing_nullifier",
      "credentialShowingNullifier",
    ],
    `${name}.showingNullifier`,
    "showing nullifier",
  );
  const verifierAlias = readSingleAlias(
    source,
    ["verifier_policy_hash", "verifierPolicyHash"],
    `${name}.verifierPolicyHash`,
    "verifier policy hash",
  );
  const domainAlias = readSingleAlias(
    source,
    ["domain_separator", "domainSeparator"],
    `${name}.domainSeparator`,
    "domain separator",
  );
  return {
    version: normalizeSilentThresholdVersion(source.version, `${name}.version`),
    issuer_set_commitment: Buffer.from(
      normalizeNonZeroFixedBytes(
        issuerAlias.value,
        `${name}.issuerSetCommitment`,
        32,
      ),
    ).toString("hex"),
    threshold_policy_hash: Buffer.from(
      normalizeNonZeroFixedBytes(
        thresholdAlias.value,
        `${name}.thresholdPolicyHash`,
        32,
      ),
    ).toString("hex"),
    credential_showing_commitment: Buffer.from(
      normalizeNonZeroFixedBytes(
        showingAlias.value,
        `${name}.credentialShowingCommitment`,
        32,
      ),
    ).toString("hex"),
    showing_nullifier: Buffer.from(
      normalizeNonZeroFixedBytes(
        nullifierAlias.value,
        `${name}.showingNullifier`,
        32,
      ),
    ).toString("hex"),
    verifier_policy_hash: Buffer.from(
      normalizeNonZeroFixedBytes(
        verifierAlias.value,
        `${name}.verifierPolicyHash`,
        32,
      ),
    ).toString("hex"),
    domain_separator: assertNonBlankString(
      domainAlias.value,
      `${name}.domainSeparator`,
    ),
  };
}

function normalizeSilentThresholdProofParts(
  source,
  context,
  { requireProofBytes = true } = {},
) {
  const backendAlias = readSingleAlias(
    source,
    ["backendTag", "backend_tag", "backend"],
    `${context}.backendTag`,
    "backend tag",
  );
  const circuitAlias = readSingleAlias(
    source,
    ["circuitId", "circuit_id"],
    `${context}.circuitId`,
    "circuit id",
  );
  const vkHashAlias = readSingleAlias(
    source,
    ["vkHash", "vk_hash", "verifierKeyHash", "verifyingKeyHash"],
    `${context}.vkHash`,
    "verifying key hash",
  );
  const proofAlias = readSingleAlias(
    source,
    ["proofBytes", "proof_bytes", "proof"],
    `${context}.proofBytes`,
    "proof bytes",
  );
  if (requireProofBytes && proofAlias.value === undefined) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.proofBytes is required`,
      `${context}.proofBytes`,
    );
  }
  const maxLimits = normalizeOptionalPrivacyMaxLimits(source, context);
  const parts = normalizeSilentThresholdCommitmentParts(source, context);
  const publicInputs = {
    version: parts.version,
    issuer_set_commitment: Buffer.from(parts.issuerSet.value).toString("hex"),
    threshold_policy_hash: Buffer.from(parts.thresholdPolicy.value).toString("hex"),
    credential_showing_commitment: Buffer.from(parts.showing.value).toString("hex"),
    showing_nullifier: Buffer.from(parts.showingNullifier.value).toString("hex"),
    verifier_policy_hash: Buffer.from(parts.verifierPolicy.value).toString("hex"),
    domain_separator: parts.domainSeparator,
  };
  const publicInputBytes = Array.from(
    Buffer.from(
      canonicalJsonStringify(publicInputs, `${context}.publicInputs`),
      "utf8",
    ).values(),
  );
  return {
    backend: normalizeSilentThresholdBackend(
      backendAlias.value,
      `${context}.backendTag`,
    ),
    circuitId: normalizeSilentThresholdCircuitId(
      circuitAlias.value,
      `${context}.circuitId`,
    ),
    vkHash: normalizeNonZeroFixedBytes(vkHashAlias.value, `${context}.vkHash`, 32),
    commitments: parts,
    publicInputs,
    publicInputBytes,
    proofBytes:
      proofAlias.value === undefined
        ? null
        : normalizeBoundedByteArray(proofAlias.value, `${context}.proofBytes`, {
            maxBytes: maxLimits.maxProofBytes ?? DEFAULT_PRIVACY_MAX_PROOF_BYTES,
          }),
    maxProofBytes: maxLimits.maxProofBytes,
    maxPublicInputBytes: maxLimits.maxPublicInputBytes,
  };
}

function silentThresholdDevProofBytes({ circuitId, vkHash, publicInputBytes }) {
  const digest = createHash("sha256");
  digest.update("iroha:silent-threshold:dev-fixture:v0", "utf8");
  digest.update(Buffer.from([0]));
  digest.update(circuitId, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(vkHash));
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(publicInputBytes));
  return Array.from(
    Buffer.concat([SILENT_THRESHOLD_DEV_PROOF_PREFIX, digest.digest()]).values(),
  );
}

function parseSilentThresholdPublicInputs(bytes, name) {
  let parsed;
  try {
    parsed = JSON.parse(Buffer.from(bytes).toString("utf8"));
  } catch (error) {
    fail(
      ValidationErrorCode.INVALID_JSON_VALUE,
      `${name} must contain valid JSON public inputs`,
      name,
      error,
    );
  }
  const normalized = normalizeSilentThresholdPublicInputs(parsed, name);
  const canonical = canonicalJsonStringify(normalized, name);
  if (!Buffer.from(bytes).equals(Buffer.from(canonical, "utf8"))) {
    fail(
      ValidationErrorCode.INVALID_JSON_VALUE,
      `${name} must use canonical JSON encoding`,
      name,
    );
  }
  return normalized;
}

function ensureSilentThresholdVerificationExpectations(source, publicInputs, context) {
  const expectationSource = { ...source };
  if (!hasPresentField(expectationSource, ["domainSeparator", "domain_separator"])) {
    expectationSource.domainSeparator = publicInputs.domain_separator;
  }
  const derivedChecks = [
    [
      [
        "issuerSetCommitment",
        "issuer_set_commitment",
        "issuerSet",
        "issuer_set",
        "issuerSetBytes",
        "issuer_set_bytes",
        "issuerSetJson",
        "issuer_set_json",
      ],
      "issuerSetCommitment",
      () => Buffer.from(
        silentThresholdIssuerSetField(expectationSource, context).value,
      ).toString("hex"),
      publicInputs.issuer_set_commitment,
    ],
    [
      [
        "thresholdPolicyHash",
        "threshold_policy_hash",
        "thresholdPolicy",
        "threshold_policy",
        "thresholdPolicyBytes",
        "threshold_policy_bytes",
        "thresholdPolicyJson",
        "threshold_policy_json",
      ],
      "thresholdPolicyHash",
      () => Buffer.from(
        silentThresholdPolicyHashField(expectationSource, context).value,
      ).toString("hex"),
      publicInputs.threshold_policy_hash,
    ],
    [
      [
        "credentialShowingCommitment",
        "credential_showing_commitment",
        "showingCommitment",
        "showing_commitment",
        "credentialShowing",
        "credential_showing",
        "credentialShowingBytes",
        "credential_showing_bytes",
        "credentialShowingJson",
        "credential_showing_json",
        "showing",
        "showingBytes",
        "showing_bytes",
        "showingJson",
        "showing_json",
      ],
      "credentialShowingCommitment",
      () => Buffer.from(
        silentThresholdShowingCommitmentField(expectationSource, context).value,
      ).toString("hex"),
      publicInputs.credential_showing_commitment,
    ],
    [
      [
        "showingNullifier",
        "showing_nullifier",
        "credentialShowingNullifier",
        "credential_showing_nullifier",
        "nullifier",
        "credentialShowing",
        "credential_showing",
        "credentialShowingBytes",
        "credential_showing_bytes",
        "credentialShowingJson",
        "credential_showing_json",
        "showing",
        "showingBytes",
        "showing_bytes",
        "showingJson",
        "showing_json",
      ],
      "showingNullifier",
      () => Buffer.from(
        silentThresholdShowingNullifierField(expectationSource, context).value,
      ).toString("hex"),
      publicInputs.showing_nullifier,
    ],
    [
      [
        "verifierPolicyHash",
        "verifier_policy_hash",
        "verifierPolicy",
        "verifier_policy",
        "verifierPolicyBytes",
        "verifier_policy_bytes",
        "verifierPolicyJson",
        "verifier_policy_json",
      ],
      "verifierPolicyHash",
      () => Buffer.from(
        silentThresholdVerifierPolicyHashField(expectationSource, context).value,
      ).toString("hex"),
      publicInputs.verifier_policy_hash,
    ],
  ];
  for (const [fields, path, normalize, actual] of derivedChecks) {
    if (hasPresentField(source, fields) && normalize() !== actual) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.${path} must match the envelope public inputs`,
        `${context}.${path}`,
      );
    }
  }
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  if (
    domainAlias.value !== undefined &&
    assertNonBlankString(domainAlias.value, `${context}.domainSeparator`) !==
      publicInputs.domain_separator
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.domainSeparator must match the envelope public inputs`,
      `${context}.domainSeparator`,
    );
  }
}

function normalizeZkX509Version(value, name) {
  const version = normalizePositiveU32(value ?? 1, name);
  if (version !== 1) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be 1`,
      name,
    );
  }
  return version;
}

function normalizeZkX509Backend(value, name) {
  const backendTag = normalizePrivacyBackendTag(value ?? ZK_X509_BACKEND, name);
  if (backendTag !== "Stark") {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be ${ZK_X509_BACKEND}`,
      name,
    );
  }
  return ZK_X509_BACKEND;
}

function normalizeZkX509CircuitId(value, name) {
  const circuitId = assertNonBlankString(value ?? ZK_X509_CIRCUIT_ID, name);
  if (
    circuitId !== ZK_X509_CIRCUIT_ID &&
    circuitId !== "zk_x509_onchain_identity_v0"
  ) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must identify zk_x509_onchain_identity_v0`,
      name,
    );
  }
  return circuitId;
}

function normalizeZkX509StructuredBytes(
  value,
  aliasKey,
  jsonAliases,
  jsonPath,
  bytePath,
  maxBytes,
) {
  let bytes;
  if (
    aliasKey.endsWith("Json") ||
    aliasKey.endsWith("_json") ||
    jsonAliases.has(aliasKey)
  ) {
    const normalizedJson = normalizeJsonValue(value, jsonPath);
    bytes = Array.from(
      Buffer.from(
        canonicalJsonStringify(normalizedJson, jsonPath),
        "utf8",
      ).values(),
    );
  } else {
    bytes = normalizeBoundedByteArray(value, bytePath, { maxBytes });
  }
  if (bytes.length > maxBytes) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${bytePath} must be no larger than ${maxBytes} bytes`,
      bytePath,
    );
  }
  return bytes;
}

function zkX509DigestBytes(label, bytes, domainSeparator) {
  const digest = createHash("sha256");
  digest.update(`iroha:zk-x509:${label}:v0`, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(domainSeparator, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(bytes));
  return Array.from(digest.digest().values());
}

function zkX509AddressBindingBytes({ bindingText, domainSeparator }) {
  const digest = createHash("sha256");
  digest.update("iroha:zk-x509:address-binding:v0", "utf8");
  digest.update(Buffer.from([0]));
  digest.update(domainSeparator, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(bindingText, "utf8");
  return Array.from(digest.digest().values());
}

function normalizeZkX509DerivedField(
  source,
  context,
  {
    explicitAliases,
    dataAliases,
    jsonAliases,
    fieldPath,
    dataPath,
    dataJsonPath,
    description,
    digestLabel,
    kindLabel,
    maxBytes,
  },
) {
  const explicitAlias = readSingleAlias(
    source,
    explicitAliases,
    `${context}.${fieldPath}`,
    description,
  );
  const dataAlias = readSingleAlias(
    source,
    dataAliases,
    `${context}.${dataPath}`,
    description,
  );
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  const domainSeparator = assertNonBlankString(
    domainAlias.value ?? ZK_X509_DOMAIN_SEPARATOR,
    `${context}.domainSeparator`,
  );
  const explicit =
    explicitAlias.value === undefined
      ? null
      : normalizeNonZeroFixedBytes(
          explicitAlias.value,
          `${context}.${fieldPath}`,
          32,
        );
  const dataBytes =
    dataAlias.value === undefined
      ? null
      : normalizeZkX509StructuredBytes(
          dataAlias.value,
          dataAlias.key,
          jsonAliases,
          `${context}.${dataJsonPath}`,
          `${context}.${dataPath}`,
          normalizePositiveU32(maxBytes, `${context}.maxBytes`),
        );
  if (explicit === null && dataBytes === null) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.${fieldPath} or ${context}.${dataPath} is required`,
      `${context}.${fieldPath}`,
    );
  }
  const derived =
    dataBytes === null ? null : zkX509DigestBytes(digestLabel, dataBytes, domainSeparator);
  if (explicit !== null && derived !== null && !fixedBytesEqual(explicit, derived)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.${fieldPath} must match the derived ${description}`,
      `${context}.${fieldPath}`,
    );
  }
  return {
    value: explicit ?? derived,
    kind: dataBytes === null ? "external" : `dev-sha256-${kindLabel}`,
    digest:
      dataBytes === null
        ? null
        : Array.from(createHash("sha256").update(Buffer.from(dataBytes)).digest().values()),
  };
}

function zkX509CaRootField(source, context) {
  return normalizeZkX509DerivedField(source, context, {
    explicitAliases: ["caRootCommitment", "ca_root_commitment"],
    dataAliases: [
      "caRoot",
      "ca_root",
      "caRootBytes",
      "ca_root_bytes",
      "caRootJson",
      "ca_root_json",
      "trustRoot",
      "trust_root",
      "trustRootJson",
      "trust_root_json",
    ],
    jsonAliases: new Set(["caRoot", "ca_root", "trustRoot", "trust_root"]),
    fieldPath: "caRootCommitment",
    dataPath: "caRoot",
    dataJsonPath: "caRootJson",
    description: "CA root commitment",
    digestLabel: "ca-root-commitment",
    kindLabel: "ca-root-digest",
    maxBytes: normalizePositiveU32AliasOrDefault(
      source,
      ["maxCaRootBytes", "max_ca_root_bytes"],
      `${context}.maxCaRootBytes`,
      "max CA root byte limit",
      ZK_X509_MAX_CA_ROOT_BYTES,
    ),
  });
}

function zkX509CertificatePolicyField(source, context) {
  return normalizeZkX509DerivedField(source, context, {
    explicitAliases: ["certificatePolicyHash", "certificate_policy_hash"],
    dataAliases: [
      "certificatePolicy",
      "certificate_policy",
      "certificatePolicyBytes",
      "certificate_policy_bytes",
      "certificatePolicyJson",
      "certificate_policy_json",
    ],
    jsonAliases: new Set(["certificatePolicy", "certificate_policy"]),
    fieldPath: "certificatePolicyHash",
    dataPath: "certificatePolicy",
    dataJsonPath: "certificatePolicyJson",
    description: "certificate policy hash",
    digestLabel: "certificate-policy-hash",
    kindLabel: "certificate-policy-digest",
    maxBytes: normalizePositiveU32AliasOrDefault(
      source,
      ["maxPolicyBytes", "max_policy_bytes"],
      `${context}.maxPolicyBytes`,
      "max policy byte limit",
      ZK_X509_MAX_POLICY_BYTES,
    ),
  });
}

function zkX509RevocationRootField(source, context) {
  return normalizeZkX509DerivedField(source, context, {
    explicitAliases: ["revocationRoot", "revocation_root"],
    dataAliases: [
      "revocationData",
      "revocation_data",
      "revocationBytes",
      "revocation_bytes",
      "revocationJson",
      "revocation_json",
      "revocationSet",
      "revocation_set",
      "revocationSetJson",
      "revocation_set_json",
      "revocationList",
      "revocation_list",
      "revocationListJson",
      "revocation_list_json",
    ],
    jsonAliases: new Set([
      "revocationData",
      "revocation_data",
      "revocationSet",
      "revocation_set",
      "revocationList",
      "revocation_list",
    ]),
    fieldPath: "revocationRoot",
    dataPath: "revocationData",
    dataJsonPath: "revocationJson",
    description: "revocation root",
    digestLabel: "revocation-root",
    kindLabel: "revocation-root-digest",
    maxBytes: normalizePositiveU32AliasOrDefault(
      source,
      ["maxRevocationBytes", "max_revocation_bytes"],
      `${context}.maxRevocationBytes`,
      "max revocation byte limit",
      ZK_X509_MAX_REVOCATION_BYTES,
    ),
  });
}

function zkX509SubjectCommitmentField(source, context) {
  return normalizeZkX509DerivedField(source, context, {
    explicitAliases: ["subjectCommitment", "subject_commitment"],
    dataAliases: [
      "subject",
      "subjectBytes",
      "subject_bytes",
      "subjectJson",
      "subject_json",
      "certificateSubject",
      "certificate_subject",
      "certificateSubjectJson",
      "certificate_subject_json",
    ],
    jsonAliases: new Set(["subject", "certificateSubject", "certificate_subject"]),
    fieldPath: "subjectCommitment",
    dataPath: "subject",
    dataJsonPath: "subjectJson",
    description: "subject commitment",
    digestLabel: "subject-commitment",
    kindLabel: "subject-digest",
    maxBytes: normalizePositiveU32AliasOrDefault(
      source,
      ["maxSubjectBytes", "max_subject_bytes"],
      `${context}.maxSubjectBytes`,
      "max subject byte limit",
      ZK_X509_MAX_SUBJECT_BYTES,
    ),
  });
}

function zkX509AddressBindingField(source, context) {
  const bindingAlias = readSingleAlias(
    source,
    ["addressBinding", "address_binding", "walletBinding", "wallet_binding"],
    `${context}.addressBinding`,
    "address binding",
  );
  const accountAlias = readSingleAlias(
    source,
    ["accountId", "account_id", "walletAccountId", "wallet_account_id"],
    `${context}.accountId`,
    "account id",
  );
  const walletAlias = readSingleAlias(
    source,
    ["walletAddress", "wallet_address"],
    `${context}.walletAddress`,
    "wallet address",
  );
  if (accountAlias.value !== undefined && walletAlias.value !== undefined) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.accountId and ${context}.walletAddress must not both be provided`,
      `${context}.addressBinding`,
    );
  }
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  const domainSeparator = assertNonBlankString(
    domainAlias.value ?? ZK_X509_DOMAIN_SEPARATOR,
    `${context}.domainSeparator`,
  );
  const explicit =
    bindingAlias.value === undefined
      ? null
      : normalizeNonZeroFixedBytes(bindingAlias.value, `${context}.addressBinding`, 32);
  const bindingText =
    accountAlias.value !== undefined
      ? normalizeAccountId(accountAlias.value, `${context}.accountId`)
      : walletAlias.value === undefined
        ? null
        : assertNonBlankString(walletAlias.value, `${context}.walletAddress`);
  const derived =
    bindingText === null
      ? null
      : zkX509AddressBindingBytes({
          bindingText,
          domainSeparator,
        });
  if (explicit === null && derived === null) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.addressBinding or ${context}.accountId is required`,
      `${context}.addressBinding`,
    );
  }
  if (explicit !== null && derived !== null && !fixedBytesEqual(explicit, derived)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.addressBinding must match the derived account binding`,
      `${context}.addressBinding`,
    );
  }
  return {
    value: explicit ?? derived,
    kind: derived === null ? "external" : "dev-sha256-account-binding",
    digest: null,
  };
}

function normalizeZkX509CommitmentParts(source, context) {
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  const domainSeparator = assertNonBlankString(
    domainAlias.value ?? ZK_X509_DOMAIN_SEPARATOR,
    `${context}.domainSeparator`,
  );
  const normalized = { ...source, domainSeparator };
  const caRoot = zkX509CaRootField(normalized, context);
  const certificatePolicy = zkX509CertificatePolicyField(normalized, context);
  const revocationRoot = zkX509RevocationRootField(normalized, context);
  const subject = zkX509SubjectCommitmentField(normalized, context);
  const addressBinding = zkX509AddressBindingField(normalized, context);
  return {
    version: normalizeZkX509Version(source.version, `${context}.version`),
    caRoot,
    certificatePolicy,
    revocationRoot,
    subject,
    addressBinding,
    domainSeparator,
  };
}

function normalizeZkX509PublicInputs(value, name) {
  const source = assertPlainObject(value, name);
  assertAllowedFields(
    source,
    new Set([
      "version",
      "ca_root_commitment",
      "caRootCommitment",
      "certificate_policy_hash",
      "certificatePolicyHash",
      "revocation_root",
      "revocationRoot",
      "subject_commitment",
      "subjectCommitment",
      "address_binding",
      "addressBinding",
      "domain_separator",
      "domainSeparator",
    ]),
    name,
  );
  const caAlias = readSingleAlias(
    source,
    ["ca_root_commitment", "caRootCommitment"],
    `${name}.caRootCommitment`,
    "CA root commitment",
  );
  const policyAlias = readSingleAlias(
    source,
    ["certificate_policy_hash", "certificatePolicyHash"],
    `${name}.certificatePolicyHash`,
    "certificate policy hash",
  );
  const revocationAlias = readSingleAlias(
    source,
    ["revocation_root", "revocationRoot"],
    `${name}.revocationRoot`,
    "revocation root",
  );
  const subjectAlias = readSingleAlias(
    source,
    ["subject_commitment", "subjectCommitment"],
    `${name}.subjectCommitment`,
    "subject commitment",
  );
  const addressAlias = readSingleAlias(
    source,
    ["address_binding", "addressBinding"],
    `${name}.addressBinding`,
    "address binding",
  );
  const domainAlias = readSingleAlias(
    source,
    ["domain_separator", "domainSeparator"],
    `${name}.domainSeparator`,
    "domain separator",
  );
  return {
    version: normalizeZkX509Version(source.version, `${name}.version`),
    ca_root_commitment: Buffer.from(
      normalizeNonZeroFixedBytes(
        caAlias.value,
        `${name}.caRootCommitment`,
        32,
      ),
    ).toString("hex"),
    certificate_policy_hash: Buffer.from(
      normalizeNonZeroFixedBytes(
        policyAlias.value,
        `${name}.certificatePolicyHash`,
        32,
      ),
    ).toString("hex"),
    revocation_root: Buffer.from(
      normalizeNonZeroFixedBytes(
        revocationAlias.value,
        `${name}.revocationRoot`,
        32,
      ),
    ).toString("hex"),
    subject_commitment: Buffer.from(
      normalizeNonZeroFixedBytes(
        subjectAlias.value,
        `${name}.subjectCommitment`,
        32,
      ),
    ).toString("hex"),
    address_binding: Buffer.from(
      normalizeNonZeroFixedBytes(
        addressAlias.value,
        `${name}.addressBinding`,
        32,
      ),
    ).toString("hex"),
    domain_separator: assertNonBlankString(
      domainAlias.value,
      `${name}.domainSeparator`,
    ),
  };
}

function normalizeZkX509ProofParts(
  source,
  context,
  { requireProofBytes = true } = {},
) {
  const backendAlias = readSingleAlias(
    source,
    ["backendTag", "backend_tag", "backend"],
    `${context}.backendTag`,
    "backend tag",
  );
  const circuitAlias = readSingleAlias(
    source,
    ["circuitId", "circuit_id"],
    `${context}.circuitId`,
    "circuit id",
  );
  const vkHashAlias = readSingleAlias(
    source,
    ["vkHash", "vk_hash", "verifierKeyHash", "verifyingKeyHash"],
    `${context}.vkHash`,
    "verifying key hash",
  );
  const proofAlias = readSingleAlias(
    source,
    ["proofBytes", "proof_bytes", "proof"],
    `${context}.proofBytes`,
    "proof bytes",
  );
  if (requireProofBytes && proofAlias.value === undefined) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.proofBytes is required`,
      `${context}.proofBytes`,
    );
  }
  const maxLimits = normalizeOptionalPrivacyMaxLimits(source, context);
  const parts = normalizeZkX509CommitmentParts(source, context);
  const publicInputs = {
    version: parts.version,
    ca_root_commitment: Buffer.from(parts.caRoot.value).toString("hex"),
    certificate_policy_hash: Buffer.from(parts.certificatePolicy.value).toString("hex"),
    revocation_root: Buffer.from(parts.revocationRoot.value).toString("hex"),
    subject_commitment: Buffer.from(parts.subject.value).toString("hex"),
    address_binding: Buffer.from(parts.addressBinding.value).toString("hex"),
    domain_separator: parts.domainSeparator,
  };
  const publicInputBytes = Array.from(
    Buffer.from(
      canonicalJsonStringify(publicInputs, `${context}.publicInputs`),
      "utf8",
    ).values(),
  );
  return {
    backend: normalizeZkX509Backend(backendAlias.value, `${context}.backendTag`),
    circuitId: normalizeZkX509CircuitId(
      circuitAlias.value,
      `${context}.circuitId`,
    ),
    vkHash: normalizeNonZeroFixedBytes(vkHashAlias.value, `${context}.vkHash`, 32),
    commitments: parts,
    publicInputs,
    publicInputBytes,
    proofBytes:
      proofAlias.value === undefined
        ? null
        : normalizeBoundedByteArray(proofAlias.value, `${context}.proofBytes`, {
            maxBytes: maxLimits.maxProofBytes ?? DEFAULT_PRIVACY_MAX_PROOF_BYTES,
          }),
    maxProofBytes: maxLimits.maxProofBytes,
    maxPublicInputBytes: maxLimits.maxPublicInputBytes,
  };
}

function zkX509DevProofBytes({ circuitId, vkHash, publicInputBytes }) {
  const digest = createHash("sha256");
  digest.update("iroha:zk-x509:dev-fixture:v0", "utf8");
  digest.update(Buffer.from([0]));
  digest.update(circuitId, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(vkHash));
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(publicInputBytes));
  return Array.from(
    Buffer.concat([ZK_X509_DEV_PROOF_PREFIX, digest.digest()]).values(),
  );
}

function parseZkX509PublicInputs(bytes, name) {
  let parsed;
  try {
    parsed = JSON.parse(Buffer.from(bytes).toString("utf8"));
  } catch (error) {
    fail(
      ValidationErrorCode.INVALID_JSON_VALUE,
      `${name} must contain valid JSON public inputs`,
      name,
      error,
    );
  }
  const normalized = normalizeZkX509PublicInputs(parsed, name);
  const canonical = canonicalJsonStringify(normalized, name);
  if (!Buffer.from(bytes).equals(Buffer.from(canonical, "utf8"))) {
    fail(
      ValidationErrorCode.INVALID_JSON_VALUE,
      `${name} must use canonical JSON encoding`,
      name,
    );
  }
  return normalized;
}

function ensureZkX509VerificationExpectations(source, publicInputs, context) {
  const expectationSource = { ...source };
  if (!hasPresentField(expectationSource, ["domainSeparator", "domain_separator"])) {
    expectationSource.domainSeparator = publicInputs.domain_separator;
  }
  const checks = [
    [
      [
        "caRootCommitment",
        "ca_root_commitment",
        "caRoot",
        "ca_root",
        "caRootBytes",
        "ca_root_bytes",
        "caRootJson",
        "ca_root_json",
        "trustRoot",
        "trust_root",
        "trustRootJson",
        "trust_root_json",
      ],
      "caRootCommitment",
      () => Buffer.from(zkX509CaRootField(expectationSource, context).value).toString("hex"),
      publicInputs.ca_root_commitment,
    ],
    [
      [
        "certificatePolicyHash",
        "certificate_policy_hash",
        "certificatePolicy",
        "certificate_policy",
        "certificatePolicyBytes",
        "certificate_policy_bytes",
        "certificatePolicyJson",
        "certificate_policy_json",
      ],
      "certificatePolicyHash",
      () => Buffer.from(
        zkX509CertificatePolicyField(expectationSource, context).value,
      ).toString("hex"),
      publicInputs.certificate_policy_hash,
    ],
    [
      [
        "revocationRoot",
        "revocation_root",
        "revocationData",
        "revocation_data",
        "revocationBytes",
        "revocation_bytes",
        "revocationJson",
        "revocation_json",
        "revocationSet",
        "revocation_set",
        "revocationSetJson",
        "revocation_set_json",
        "revocationList",
        "revocation_list",
        "revocationListJson",
        "revocation_list_json",
      ],
      "revocationRoot",
      () => Buffer.from(zkX509RevocationRootField(expectationSource, context).value).toString("hex"),
      publicInputs.revocation_root,
    ],
    [
      [
        "subjectCommitment",
        "subject_commitment",
        "subject",
        "subjectBytes",
        "subject_bytes",
        "subjectJson",
        "subject_json",
        "certificateSubject",
        "certificate_subject",
        "certificateSubjectJson",
        "certificate_subject_json",
      ],
      "subjectCommitment",
      () => Buffer.from(zkX509SubjectCommitmentField(expectationSource, context).value).toString("hex"),
      publicInputs.subject_commitment,
    ],
    [
      [
        "addressBinding",
        "address_binding",
        "walletBinding",
        "wallet_binding",
        "accountId",
        "account_id",
        "walletAccountId",
        "wallet_account_id",
        "walletAddress",
        "wallet_address",
      ],
      "addressBinding",
      () => Buffer.from(zkX509AddressBindingField(expectationSource, context).value).toString("hex"),
      publicInputs.address_binding,
    ],
  ];
  for (const [fields, path, normalize, actual] of checks) {
    if (hasPresentField(source, fields) && normalize() !== actual) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.${path} must match the envelope public inputs`,
        `${context}.${path}`,
      );
    }
  }
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  if (
    domainAlias.value !== undefined &&
    assertNonBlankString(domainAlias.value, `${context}.domainSeparator`) !==
      publicInputs.domain_separator
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.domainSeparator must match the envelope public inputs`,
      `${context}.domainSeparator`,
    );
  }
}

function normalizeJindoVersion(value, name) {
  const version = normalizePositiveU32(value ?? 1, name);
  if (version !== 1) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be 1`,
      name,
    );
  }
  return version;
}

function normalizeJindoBackend(value, name) {
  const backendTag = normalizePrivacyBackendTag(value ?? JINDO_BACKEND, name);
  if (backendTag !== "Unsupported") {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must remain unsupported until a production Jindo backend is registered`,
      name,
    );
  }
  return JINDO_BACKEND;
}

function normalizeJindoCircuitId(value, name) {
  const circuitId = assertNonBlankString(value ?? JINDO_CIRCUIT_ID, name);
  if (
    circuitId !== JINDO_CIRCUIT_ID &&
    circuitId !== "jindo_lattice_pcs_zk_v0"
  ) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must identify jindo_lattice_pcs_zk_v0`,
      name,
    );
  }
  return circuitId;
}

function jindoDigestBytes(label, bytes, domainSeparator) {
  const digest = createHash("sha256");
  digest.update(`iroha:jindo:${label}:v0`, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(domainSeparator, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(bytes));
  return Array.from(digest.digest().values());
}

function normalizeJindoDerivedField(
  source,
  context,
  {
    explicitAliases,
    dataAliases,
    jsonAliases,
    fieldPath,
    dataPath,
    dataJsonPath,
    description,
    digestLabel,
    kindLabel,
    maxBytes,
  },
) {
  const explicitAlias = readSingleAlias(
    source,
    explicitAliases,
    `${context}.${fieldPath}`,
    description,
  );
  const dataAlias = readSingleAlias(
    source,
    dataAliases,
    `${context}.${dataPath}`,
    description,
  );
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  const domainSeparator = assertNonBlankString(
    domainAlias.value ?? JINDO_DOMAIN_SEPARATOR,
    `${context}.domainSeparator`,
  );
  const explicit =
    explicitAlias.value === undefined
      ? null
      : normalizeNonZeroFixedBytes(
          explicitAlias.value,
          `${context}.${fieldPath}`,
          32,
        );
  const dataBytes =
    dataAlias.value === undefined
      ? null
      : normalizeZkX509StructuredBytes(
          dataAlias.value,
          dataAlias.key,
          jsonAliases,
          `${context}.${dataJsonPath}`,
          `${context}.${dataPath}`,
          normalizePositiveU32(maxBytes, `${context}.maxBytes`),
        );
  if (explicit === null && dataBytes === null) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.${fieldPath} or ${context}.${dataPath} is required`,
      `${context}.${fieldPath}`,
    );
  }
  const derived =
    dataBytes === null ? null : jindoDigestBytes(digestLabel, dataBytes, domainSeparator);
  if (explicit !== null && derived !== null && !fixedBytesEqual(explicit, derived)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.${fieldPath} must match the derived ${description}`,
      `${context}.${fieldPath}`,
    );
  }
  return {
    value: explicit ?? derived,
    kind: dataBytes === null ? "external" : `dev-sha256-${kindLabel}`,
    digest:
      dataBytes === null
        ? null
        : Array.from(createHash("sha256").update(Buffer.from(dataBytes)).digest().values()),
  };
}

function jindoCommitmentField(source, context) {
  return normalizeJindoDerivedField(source, context, {
    explicitAliases: [
      "commitment",
      "polynomialCommitment",
      "polynomial_commitment",
    ],
    dataAliases: [
      "polynomial",
      "polynomialBytes",
      "polynomial_bytes",
      "polynomialJson",
      "polynomial_json",
      "commitmentMaterial",
      "commitment_material",
      "commitmentMaterialJson",
      "commitment_material_json",
    ],
    jsonAliases: new Set([
      "polynomial",
      "commitmentMaterial",
      "commitment_material",
    ]),
    fieldPath: "commitment",
    dataPath: "polynomial",
    dataJsonPath: "polynomialJson",
    description: "polynomial commitment",
    digestLabel: "commitment",
    kindLabel: "commitment-digest",
    maxBytes: normalizePositiveU32AliasOrDefault(
      source,
      ["maxPolynomialBytes", "max_polynomial_bytes"],
      `${context}.maxPolynomialBytes`,
      "max polynomial byte limit",
      JINDO_MAX_POLYNOMIAL_BYTES,
    ),
  });
}

function jindoOpeningClaimField(source, context) {
  return normalizeJindoDerivedField(source, context, {
    explicitAliases: [
      "openingClaimCommitment",
      "opening_claim_commitment",
      "openingClaimHash",
      "opening_claim_hash",
      "openingClaimDigest",
      "opening_claim_digest",
      "opening_claim",
      "openingClaim",
    ],
    dataAliases: [
      "claim",
      "claimBytes",
      "claim_bytes",
      "claimJson",
      "claim_json",
      "openingClaimBytes",
      "opening_claim_bytes",
      "openingClaimJson",
      "opening_claim_json",
      "evaluationClaim",
      "evaluation_claim",
      "evaluationClaimJson",
      "evaluation_claim_json",
    ],
    jsonAliases: new Set([
      "claim",
      "evaluationClaim",
      "evaluation_claim",
    ]),
    fieldPath: "openingClaim",
    dataPath: "openingClaim",
    dataJsonPath: "openingClaimJson",
    description: "opening claim",
    digestLabel: "opening-claim",
    kindLabel: "opening-claim-digest",
    maxBytes: normalizePositiveU32AliasOrDefault(
      source,
      ["maxOpeningClaimBytes", "max_opening_claim_bytes"],
      `${context}.maxOpeningClaimBytes`,
      "max opening-claim byte limit",
      JINDO_MAX_OPENING_CLAIM_BYTES,
    ),
  });
}

function jindoQuerySetField(source, context) {
  return normalizeJindoDerivedField(source, context, {
    explicitAliases: [
      "querySetHash",
      "query_set_hash",
      "querySetRoot",
      "query_set_root",
    ],
    dataAliases: [
      "querySet",
      "query_set",
      "querySetBytes",
      "query_set_bytes",
      "querySetJson",
      "query_set_json",
      "queries",
      "queriesJson",
      "queries_json",
    ],
    jsonAliases: new Set(["querySet", "query_set", "queries"]),
    fieldPath: "querySet",
    dataPath: "querySet",
    dataJsonPath: "querySetJson",
    description: "query set",
    digestLabel: "query-set",
    kindLabel: "query-set-digest",
    maxBytes: normalizePositiveU32AliasOrDefault(
      source,
      ["maxQuerySetBytes", "max_query_set_bytes"],
      `${context}.maxQuerySetBytes`,
      "max query-set byte limit",
      JINDO_MAX_QUERY_SET_BYTES,
    ),
  });
}

function jindoParameterHashField(source, context) {
  return normalizeJindoDerivedField(source, context, {
    explicitAliases: [
      "parameterHash",
      "parameter_hash",
      "paramsHash",
      "params_hash",
    ],
    dataAliases: [
      "parameters",
      "parametersBytes",
      "parameters_bytes",
      "parametersJson",
      "parameters_json",
      "parameterSet",
      "parameter_set",
      "parameterSetJson",
      "parameter_set_json",
      "params",
      "paramsBytes",
      "params_bytes",
      "paramsJson",
      "params_json",
    ],
    jsonAliases: new Set([
      "parameters",
      "parameterSet",
      "parameter_set",
      "params",
    ]),
    fieldPath: "parameterHash",
    dataPath: "parameters",
    dataJsonPath: "parametersJson",
    description: "parameter hash",
    digestLabel: "parameter-hash",
    kindLabel: "parameter-hash",
    maxBytes: normalizePositiveU32AliasOrDefault(
      source,
      ["maxParameterBytes", "max_parameter_bytes"],
      `${context}.maxParameterBytes`,
      "max parameter byte limit",
      JINDO_MAX_PARAMETER_BYTES,
    ),
  });
}

function normalizeJindoPublicInputParts(source, context) {
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  const domainSeparator = assertNonBlankString(
    domainAlias.value ?? JINDO_DOMAIN_SEPARATOR,
    `${context}.domainSeparator`,
  );
  const normalized = { ...source, domainSeparator };
  const commitment = jindoCommitmentField(normalized, context);
  const openingClaim = jindoOpeningClaimField(normalized, context);
  const querySet = jindoQuerySetField(normalized, context);
  const parameterHash = jindoParameterHashField(normalized, context);
  return {
    version: normalizeJindoVersion(source.version, `${context}.version`),
    commitment,
    openingClaim,
    querySet,
    parameterHash,
    domainSeparator,
  };
}

function normalizeJindoPublicInputs(value, name) {
  const source = assertPlainObject(value, name);
  assertAllowedFields(
    source,
    new Set([
      "version",
      "commitment",
      "opening_claim",
      "openingClaim",
      "query_set",
      "querySet",
      "parameter_hash",
      "parameterHash",
      "domain_separator",
      "domainSeparator",
    ]),
    name,
  );
  const commitmentAlias = readSingleAlias(
    source,
    ["commitment"],
    `${name}.commitment`,
    "commitment",
  );
  const openingAlias = readSingleAlias(
    source,
    ["opening_claim", "openingClaim"],
    `${name}.openingClaim`,
    "opening claim",
  );
  const queryAlias = readSingleAlias(
    source,
    ["query_set", "querySet"],
    `${name}.querySet`,
    "query set",
  );
  const parameterAlias = readSingleAlias(
    source,
    ["parameter_hash", "parameterHash"],
    `${name}.parameterHash`,
    "parameter hash",
  );
  const domainAlias = readSingleAlias(
    source,
    ["domain_separator", "domainSeparator"],
    `${name}.domainSeparator`,
    "domain separator",
  );
  return {
    version: normalizeJindoVersion(source.version, `${name}.version`),
    commitment: Buffer.from(
      normalizeNonZeroFixedBytes(
        commitmentAlias.value,
        `${name}.commitment`,
        32,
      ),
    ).toString("hex"),
    opening_claim: Buffer.from(
      normalizeNonZeroFixedBytes(
        openingAlias.value,
        `${name}.openingClaim`,
        32,
      ),
    ).toString("hex"),
    query_set: Buffer.from(
      normalizeNonZeroFixedBytes(
        queryAlias.value,
        `${name}.querySet`,
        32,
      ),
    ).toString("hex"),
    parameter_hash: Buffer.from(
      normalizeNonZeroFixedBytes(
        parameterAlias.value,
        `${name}.parameterHash`,
        32,
      ),
    ).toString("hex"),
    domain_separator: assertNonBlankString(
      domainAlias.value,
      `${name}.domainSeparator`,
    ),
  };
}

function normalizeJindoProofParts(
  source,
  context,
  { requireProofBytes = true } = {},
) {
  const backendAlias = readSingleAlias(
    source,
    ["backendTag", "backend_tag", "backend"],
    `${context}.backendTag`,
    "backend tag",
  );
  const circuitAlias = readSingleAlias(
    source,
    ["circuitId", "circuit_id"],
    `${context}.circuitId`,
    "circuit id",
  );
  const vkHashAlias = readSingleAlias(
    source,
    ["vkHash", "vk_hash", "verifierKeyHash", "verifyingKeyHash"],
    `${context}.vkHash`,
    "verifying key hash",
  );
  const proofAlias = readSingleAlias(
    source,
    ["proofBytes", "proof_bytes", "proof"],
    `${context}.proofBytes`,
    "proof bytes",
  );
  if (requireProofBytes && proofAlias.value === undefined) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.proofBytes is required`,
      `${context}.proofBytes`,
    );
  }
  const maxLimits = normalizeOptionalPrivacyMaxLimits(source, context);
  const parts = normalizeJindoPublicInputParts(source, context);
  const publicInputs = {
    version: parts.version,
    commitment: Buffer.from(parts.commitment.value).toString("hex"),
    opening_claim: Buffer.from(parts.openingClaim.value).toString("hex"),
    query_set: Buffer.from(parts.querySet.value).toString("hex"),
    parameter_hash: Buffer.from(parts.parameterHash.value).toString("hex"),
    domain_separator: parts.domainSeparator,
  };
  const publicInputBytes = Array.from(
    Buffer.from(
      canonicalJsonStringify(publicInputs, `${context}.publicInputs`),
      "utf8",
    ).values(),
  );
  return {
    backend: normalizeJindoBackend(backendAlias.value, `${context}.backendTag`),
    circuitId: normalizeJindoCircuitId(
      circuitAlias.value,
      `${context}.circuitId`,
    ),
    vkHash: normalizeNonZeroFixedBytes(vkHashAlias.value, `${context}.vkHash`, 32),
    inputs: parts,
    publicInputs,
    publicInputBytes,
    proofBytes:
      proofAlias.value === undefined
        ? null
        : normalizeBoundedByteArray(proofAlias.value, `${context}.proofBytes`, {
            maxBytes: maxLimits.maxProofBytes ?? DEFAULT_PRIVACY_MAX_PROOF_BYTES,
          }),
    maxProofBytes: maxLimits.maxProofBytes,
    maxPublicInputBytes: maxLimits.maxPublicInputBytes,
  };
}

function jindoDevProofBytes({ circuitId, vkHash, publicInputBytes }) {
  const digest = createHash("sha256");
  digest.update("iroha:jindo:dev-fixture:v0", "utf8");
  digest.update(Buffer.from([0]));
  digest.update(circuitId, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(vkHash));
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(publicInputBytes));
  return Array.from(
    Buffer.concat([JINDO_DEV_PROOF_PREFIX, digest.digest()]).values(),
  );
}

function parseJindoPublicInputs(bytes, name) {
  let parsed;
  try {
    parsed = JSON.parse(Buffer.from(bytes).toString("utf8"));
  } catch (error) {
    fail(
      ValidationErrorCode.INVALID_JSON_VALUE,
      `${name} must contain valid JSON public inputs`,
      name,
      error,
    );
  }
  const normalized = normalizeJindoPublicInputs(parsed, name);
  const canonical = canonicalJsonStringify(normalized, name);
  if (!Buffer.from(bytes).equals(Buffer.from(canonical, "utf8"))) {
    fail(
      ValidationErrorCode.INVALID_JSON_VALUE,
      `${name} must use canonical JSON encoding`,
      name,
    );
  }
  return normalized;
}

function ensureJindoVerificationExpectations(source, publicInputs, context) {
  const expectationSource = { ...source };
  if (!hasPresentField(expectationSource, ["domainSeparator", "domain_separator"])) {
    expectationSource.domainSeparator = publicInputs.domain_separator;
  }
  const checks = [
    [
      [
        "commitment",
        "polynomialCommitment",
        "polynomial_commitment",
        "polynomial",
        "polynomialBytes",
        "polynomial_bytes",
        "polynomialJson",
        "polynomial_json",
        "commitmentMaterial",
        "commitment_material",
        "commitmentMaterialJson",
        "commitment_material_json",
      ],
      "commitment",
      () => Buffer.from(jindoCommitmentField(expectationSource, context).value).toString("hex"),
      publicInputs.commitment,
    ],
    [
      [
        "openingClaimCommitment",
        "opening_claim_commitment",
        "openingClaimHash",
        "opening_claim_hash",
        "openingClaimDigest",
        "opening_claim_digest",
        "opening_claim",
        "openingClaim",
        "claim",
        "claimBytes",
        "claim_bytes",
        "claimJson",
        "claim_json",
        "openingClaimBytes",
        "opening_claim_bytes",
        "openingClaimJson",
        "opening_claim_json",
        "evaluationClaim",
        "evaluation_claim",
        "evaluationClaimJson",
        "evaluation_claim_json",
      ],
      "openingClaim",
      () => Buffer.from(jindoOpeningClaimField(expectationSource, context).value).toString("hex"),
      publicInputs.opening_claim,
    ],
    [
      [
        "querySetHash",
        "query_set_hash",
        "querySetRoot",
        "query_set_root",
        "querySet",
        "query_set",
        "querySetBytes",
        "query_set_bytes",
        "querySetJson",
        "query_set_json",
        "queries",
        "queriesJson",
        "queries_json",
      ],
      "querySet",
      () => Buffer.from(jindoQuerySetField(expectationSource, context).value).toString("hex"),
      publicInputs.query_set,
    ],
    [
      [
        "parameterHash",
        "parameter_hash",
        "paramsHash",
        "params_hash",
        "parameters",
        "parametersBytes",
        "parameters_bytes",
        "parametersJson",
        "parameters_json",
        "parameterSet",
        "parameter_set",
        "parameterSetJson",
        "parameter_set_json",
        "params",
        "paramsBytes",
        "params_bytes",
        "paramsJson",
        "params_json",
      ],
      "parameterHash",
      () => Buffer.from(jindoParameterHashField(expectationSource, context).value).toString("hex"),
      publicInputs.parameter_hash,
    ],
  ];
  for (const [fields, path, normalize, actual] of checks) {
    if (hasPresentField(source, fields) && normalize() !== actual) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.${path} must match the envelope public inputs`,
        `${context}.${path}`,
      );
    }
  }
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  if (
    domainAlias.value !== undefined &&
    assertNonBlankString(domainAlias.value, `${context}.domainSeparator`) !==
      publicInputs.domain_separator
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.domainSeparator must match the envelope public inputs`,
      `${context}.domainSeparator`,
    );
  }
}

function normalizeSisHintsVersion(value, name) {
  const version = normalizePositiveU32(value ?? 1, name);
  if (version !== 1) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be 1`,
      name,
    );
  }
  return version;
}

function normalizeSisHintsBackend(value, name) {
  const backendTag = normalizePrivacyBackendTag(value ?? SIS_HINTS_BACKEND, name);
  if (backendTag !== "Unsupported") {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must remain unsupported until a production SIS-with-hints backend is registered`,
      name,
    );
  }
  return SIS_HINTS_BACKEND;
}

function normalizeSisHintsCircuitId(value, name) {
  const circuitId = assertNonBlankString(value ?? SIS_HINTS_CIRCUIT_ID, name);
  if (
    circuitId !== SIS_HINTS_CIRCUIT_ID &&
    circuitId !== "sis_hints_anoncred_pq_v0"
  ) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must identify sis_hints_anoncred_pq_v0`,
      name,
    );
  }
  return circuitId;
}

function sisHintsDigestBytes(label, bytes, domainSeparator) {
  const digest = createHash("sha256");
  digest.update(`iroha:sis-hints:${label}:v0`, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(domainSeparator, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(bytes));
  return Array.from(digest.digest().values());
}

function normalizeSisHintsDerivedField(
  source,
  context,
  {
    explicitAliases,
    dataAliases,
    jsonAliases,
    fieldPath,
    dataPath,
    dataJsonPath,
    description,
    digestLabel,
    kindLabel,
    maxBytes,
  },
) {
  const explicitAlias = readSingleAlias(
    source,
    explicitAliases,
    `${context}.${fieldPath}`,
    description,
  );
  const dataAlias = readSingleAlias(
    source,
    dataAliases,
    `${context}.${dataPath}`,
    description,
  );
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  const domainSeparator = assertNonBlankString(
    domainAlias.value ?? SIS_HINTS_DOMAIN_SEPARATOR,
    `${context}.domainSeparator`,
  );
  const explicit =
    explicitAlias.value === undefined
      ? null
      : normalizeNonZeroFixedBytes(
          explicitAlias.value,
          `${context}.${fieldPath}`,
          32,
        );
  const dataBytes =
    dataAlias.value === undefined
      ? null
      : normalizeZkX509StructuredBytes(
          dataAlias.value,
          dataAlias.key,
          jsonAliases,
          `${context}.${dataJsonPath}`,
          `${context}.${dataPath}`,
          normalizePositiveU32(maxBytes, `${context}.maxBytes`),
        );
  if (explicit === null && dataBytes === null) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.${fieldPath} or ${context}.${dataPath} is required`,
      `${context}.${fieldPath}`,
    );
  }
  const derived =
    dataBytes === null ? null : sisHintsDigestBytes(digestLabel, dataBytes, domainSeparator);
  if (explicit !== null && derived !== null && !fixedBytesEqual(explicit, derived)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.${fieldPath} must match the derived ${description}`,
      `${context}.${fieldPath}`,
    );
  }
  return {
    value: explicit ?? derived,
    kind: dataBytes === null ? "external" : `dev-sha256-${kindLabel}`,
    digest:
      dataBytes === null
        ? null
        : Array.from(createHash("sha256").update(Buffer.from(dataBytes)).digest().values()),
  };
}

function sisHintsIssuerField(source, context) {
  return normalizeSisHintsDerivedField(source, context, {
    explicitAliases: [
      "issuerCommitment",
      "issuer_commitment",
      "issuerParameterCommitment",
      "issuer_parameter_commitment",
    ],
    dataAliases: [
      "issuer",
      "issuerBytes",
      "issuer_bytes",
      "issuerJson",
      "issuer_json",
      "issuerParameters",
      "issuer_parameters",
      "issuerParametersJson",
      "issuer_parameters_json",
    ],
    jsonAliases: new Set(["issuer", "issuerParameters", "issuer_parameters"]),
    fieldPath: "issuerCommitment",
    dataPath: "issuer",
    dataJsonPath: "issuerJson",
    description: "issuer commitment",
    digestLabel: "issuer-commitment",
    kindLabel: "issuer-digest",
    maxBytes: normalizePositiveU32AliasOrDefault(
      source,
      ["maxIssuerBytes", "max_issuer_bytes"],
      `${context}.maxIssuerBytes`,
      "max issuer byte limit",
      SIS_HINTS_MAX_ISSUER_BYTES,
    ),
  });
}

function sisHintsCredentialField(source, context) {
  return normalizeSisHintsDerivedField(source, context, {
    explicitAliases: [
      "credentialCommitment",
      "credential_commitment",
      "credentialShowingCommitment",
      "credential_showing_commitment",
    ],
    dataAliases: [
      "credential",
      "credentialBytes",
      "credential_bytes",
      "credentialJson",
      "credential_json",
      "credentialShowing",
      "credential_showing",
      "credentialShowingJson",
      "credential_showing_json",
      "showing",
      "showingJson",
      "showing_json",
    ],
    jsonAliases: new Set([
      "credential",
      "credentialShowing",
      "credential_showing",
      "showing",
    ]),
    fieldPath: "credentialCommitment",
    dataPath: "credential",
    dataJsonPath: "credentialJson",
    description: "credential commitment",
    digestLabel: "credential-commitment",
    kindLabel: "credential-digest",
    maxBytes: normalizePositiveU32AliasOrDefault(
      source,
      ["maxCredentialBytes", "max_credential_bytes"],
      `${context}.maxCredentialBytes`,
      "max credential byte limit",
      SIS_HINTS_MAX_CREDENTIAL_BYTES,
    ),
  });
}

function sisHintsShowingPolicyField(source, context) {
  return normalizeSisHintsDerivedField(source, context, {
    explicitAliases: [
      "showingPolicyHash",
      "showing_policy_hash",
      "policyHash",
      "policy_hash",
      "verifierPolicyHash",
      "verifier_policy_hash",
    ],
    dataAliases: [
      "showingPolicy",
      "showing_policy",
      "showingPolicyBytes",
      "showing_policy_bytes",
      "showingPolicyJson",
      "showing_policy_json",
      "policy",
      "policyBytes",
      "policy_bytes",
      "policyJson",
      "policy_json",
      "verifierPolicy",
      "verifier_policy",
      "verifierPolicyJson",
      "verifier_policy_json",
    ],
    jsonAliases: new Set([
      "showingPolicy",
      "showing_policy",
      "policy",
      "verifierPolicy",
      "verifier_policy",
    ]),
    fieldPath: "showingPolicyHash",
    dataPath: "showingPolicy",
    dataJsonPath: "showingPolicyJson",
    description: "showing policy hash",
    digestLabel: "showing-policy-hash",
    kindLabel: "showing-policy-hash",
    maxBytes: normalizePositiveU32AliasOrDefault(
      source,
      ["maxPolicyBytes", "max_policy_bytes"],
      `${context}.maxPolicyBytes`,
      "max policy byte limit",
      SIS_HINTS_MAX_POLICY_BYTES,
    ),
  });
}

function sisHintsParameterHashField(source, context) {
  return normalizeSisHintsDerivedField(source, context, {
    explicitAliases: [
      "parameterHash",
      "parameter_hash",
      "paramsHash",
      "params_hash",
    ],
    dataAliases: [
      "parameters",
      "parametersBytes",
      "parameters_bytes",
      "parametersJson",
      "parameters_json",
      "parameterSet",
      "parameter_set",
      "parameterSetJson",
      "parameter_set_json",
      "sisParameters",
      "sis_parameters",
      "sisParametersJson",
      "sis_parameters_json",
      "params",
      "paramsJson",
      "params_json",
    ],
    jsonAliases: new Set([
      "parameters",
      "parameterSet",
      "parameter_set",
      "sisParameters",
      "sis_parameters",
      "params",
    ]),
    fieldPath: "parameterHash",
    dataPath: "parameters",
    dataJsonPath: "parametersJson",
    description: "parameter hash",
    digestLabel: "parameter-hash",
    kindLabel: "parameter-hash",
    maxBytes: normalizePositiveU32AliasOrDefault(
      source,
      ["maxParameterBytes", "max_parameter_bytes"],
      `${context}.maxParameterBytes`,
      "max parameter byte limit",
      SIS_HINTS_MAX_PARAMETER_BYTES,
    ),
  });
}

function normalizeSisHintsCommitmentParts(source, context) {
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  const domainSeparator = assertNonBlankString(
    domainAlias.value ?? SIS_HINTS_DOMAIN_SEPARATOR,
    `${context}.domainSeparator`,
  );
  const normalized = { ...source, domainSeparator };
  const issuer = sisHintsIssuerField(normalized, context);
  const credential = sisHintsCredentialField(normalized, context);
  const showingPolicy = sisHintsShowingPolicyField(normalized, context);
  const parameterHash = sisHintsParameterHashField(normalized, context);
  return {
    version: normalizeSisHintsVersion(source.version, `${context}.version`),
    issuer,
    credential,
    showingPolicy,
    parameterHash,
    domainSeparator,
  };
}

function normalizeSisHintsPublicInputs(value, name) {
  const source = assertPlainObject(value, name);
  assertAllowedFields(
    source,
    new Set([
      "version",
      "issuer_commitment",
      "issuerCommitment",
      "credential_commitment",
      "credentialCommitment",
      "showing_policy_hash",
      "showingPolicyHash",
      "parameter_hash",
      "parameterHash",
      "domain_separator",
      "domainSeparator",
    ]),
    name,
  );
  const issuerAlias = readSingleAlias(
    source,
    ["issuer_commitment", "issuerCommitment"],
    `${name}.issuerCommitment`,
    "issuer commitment",
  );
  const credentialAlias = readSingleAlias(
    source,
    ["credential_commitment", "credentialCommitment"],
    `${name}.credentialCommitment`,
    "credential commitment",
  );
  const policyAlias = readSingleAlias(
    source,
    ["showing_policy_hash", "showingPolicyHash"],
    `${name}.showingPolicyHash`,
    "showing policy hash",
  );
  const parameterAlias = readSingleAlias(
    source,
    ["parameter_hash", "parameterHash"],
    `${name}.parameterHash`,
    "parameter hash",
  );
  const domainAlias = readSingleAlias(
    source,
    ["domain_separator", "domainSeparator"],
    `${name}.domainSeparator`,
    "domain separator",
  );
  return {
    version: normalizeSisHintsVersion(source.version, `${name}.version`),
    issuer_commitment: Buffer.from(
      normalizeNonZeroFixedBytes(
        issuerAlias.value,
        `${name}.issuerCommitment`,
        32,
      ),
    ).toString("hex"),
    credential_commitment: Buffer.from(
      normalizeNonZeroFixedBytes(
        credentialAlias.value,
        `${name}.credentialCommitment`,
        32,
      ),
    ).toString("hex"),
    showing_policy_hash: Buffer.from(
      normalizeNonZeroFixedBytes(
        policyAlias.value,
        `${name}.showingPolicyHash`,
        32,
      ),
    ).toString("hex"),
    parameter_hash: Buffer.from(
      normalizeNonZeroFixedBytes(
        parameterAlias.value,
        `${name}.parameterHash`,
        32,
      ),
    ).toString("hex"),
    domain_separator: assertNonBlankString(
      domainAlias.value,
      `${name}.domainSeparator`,
    ),
  };
}

function normalizeSisHintsProofParts(
  source,
  context,
  { requireProofBytes = true } = {},
) {
  const backendAlias = readSingleAlias(
    source,
    ["backendTag", "backend_tag", "backend"],
    `${context}.backendTag`,
    "backend tag",
  );
  const circuitAlias = readSingleAlias(
    source,
    ["circuitId", "circuit_id"],
    `${context}.circuitId`,
    "circuit id",
  );
  const vkHashAlias = readSingleAlias(
    source,
    ["vkHash", "vk_hash", "verifierKeyHash", "verifyingKeyHash"],
    `${context}.vkHash`,
    "verifying key hash",
  );
  const proofAlias = readSingleAlias(
    source,
    ["proofBytes", "proof_bytes", "proof"],
    `${context}.proofBytes`,
    "proof bytes",
  );
  if (requireProofBytes && proofAlias.value === undefined) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.proofBytes is required`,
      `${context}.proofBytes`,
    );
  }
  const maxLimits = normalizeOptionalPrivacyMaxLimits(source, context);
  const parts = normalizeSisHintsCommitmentParts(source, context);
  const publicInputs = {
    version: parts.version,
    issuer_commitment: Buffer.from(parts.issuer.value).toString("hex"),
    credential_commitment: Buffer.from(parts.credential.value).toString("hex"),
    showing_policy_hash: Buffer.from(parts.showingPolicy.value).toString("hex"),
    parameter_hash: Buffer.from(parts.parameterHash.value).toString("hex"),
    domain_separator: parts.domainSeparator,
  };
  const publicInputBytes = Array.from(
    Buffer.from(
      canonicalJsonStringify(publicInputs, `${context}.publicInputs`),
      "utf8",
    ).values(),
  );
  return {
    backend: normalizeSisHintsBackend(backendAlias.value, `${context}.backendTag`),
    circuitId: normalizeSisHintsCircuitId(
      circuitAlias.value,
      `${context}.circuitId`,
    ),
    vkHash: normalizeNonZeroFixedBytes(vkHashAlias.value, `${context}.vkHash`, 32),
    commitments: parts,
    publicInputs,
    publicInputBytes,
    proofBytes:
      proofAlias.value === undefined
        ? null
        : normalizeBoundedByteArray(proofAlias.value, `${context}.proofBytes`, {
            maxBytes: maxLimits.maxProofBytes ?? DEFAULT_PRIVACY_MAX_PROOF_BYTES,
          }),
    maxProofBytes: maxLimits.maxProofBytes,
    maxPublicInputBytes: maxLimits.maxPublicInputBytes,
  };
}

function sisHintsDevProofBytes({ circuitId, vkHash, publicInputBytes }) {
  const digest = createHash("sha256");
  digest.update("iroha:sis-hints:dev-fixture:v0", "utf8");
  digest.update(Buffer.from([0]));
  digest.update(circuitId, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(vkHash));
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(publicInputBytes));
  return Array.from(
    Buffer.concat([SIS_HINTS_DEV_PROOF_PREFIX, digest.digest()]).values(),
  );
}

function parseSisHintsPublicInputs(bytes, name) {
  let parsed;
  try {
    parsed = JSON.parse(Buffer.from(bytes).toString("utf8"));
  } catch (error) {
    fail(
      ValidationErrorCode.INVALID_JSON_VALUE,
      `${name} must contain valid JSON public inputs`,
      name,
      error,
    );
  }
  const normalized = normalizeSisHintsPublicInputs(parsed, name);
  const canonical = canonicalJsonStringify(normalized, name);
  if (!Buffer.from(bytes).equals(Buffer.from(canonical, "utf8"))) {
    fail(
      ValidationErrorCode.INVALID_JSON_VALUE,
      `${name} must use canonical JSON encoding`,
      name,
    );
  }
  return normalized;
}

function ensureSisHintsVerificationExpectations(source, publicInputs, context) {
  const expectationSource = { ...source };
  if (!hasPresentField(expectationSource, ["domainSeparator", "domain_separator"])) {
    expectationSource.domainSeparator = publicInputs.domain_separator;
  }
  const checks = [
    [
      [
        "issuerCommitment",
        "issuer_commitment",
        "issuerParameterCommitment",
        "issuer_parameter_commitment",
        "issuer",
        "issuerBytes",
        "issuer_bytes",
        "issuerJson",
        "issuer_json",
        "issuerParameters",
        "issuer_parameters",
        "issuerParametersJson",
        "issuer_parameters_json",
      ],
      "issuerCommitment",
      () => Buffer.from(sisHintsIssuerField(expectationSource, context).value).toString("hex"),
      publicInputs.issuer_commitment,
    ],
    [
      [
        "credentialCommitment",
        "credential_commitment",
        "credentialShowingCommitment",
        "credential_showing_commitment",
        "credential",
        "credentialBytes",
        "credential_bytes",
        "credentialJson",
        "credential_json",
        "credentialShowing",
        "credential_showing",
        "credentialShowingJson",
        "credential_showing_json",
        "showing",
        "showingJson",
        "showing_json",
      ],
      "credentialCommitment",
      () => Buffer.from(sisHintsCredentialField(expectationSource, context).value).toString("hex"),
      publicInputs.credential_commitment,
    ],
    [
      [
        "showingPolicyHash",
        "showing_policy_hash",
        "policyHash",
        "policy_hash",
        "verifierPolicyHash",
        "verifier_policy_hash",
        "showingPolicy",
        "showing_policy",
        "showingPolicyBytes",
        "showing_policy_bytes",
        "showingPolicyJson",
        "showing_policy_json",
        "policy",
        "policyBytes",
        "policy_bytes",
        "policyJson",
        "policy_json",
        "verifierPolicy",
        "verifier_policy",
        "verifierPolicyJson",
        "verifier_policy_json",
      ],
      "showingPolicyHash",
      () => Buffer.from(sisHintsShowingPolicyField(expectationSource, context).value).toString("hex"),
      publicInputs.showing_policy_hash,
    ],
    [
      [
        "parameterHash",
        "parameter_hash",
        "paramsHash",
        "params_hash",
        "parameters",
        "parametersBytes",
        "parameters_bytes",
        "parametersJson",
        "parameters_json",
        "parameterSet",
        "parameter_set",
        "parameterSetJson",
        "parameter_set_json",
        "sisParameters",
        "sis_parameters",
        "sisParametersJson",
        "sis_parameters_json",
        "params",
        "paramsJson",
        "params_json",
      ],
      "parameterHash",
      () => Buffer.from(sisHintsParameterHashField(expectationSource, context).value).toString("hex"),
      publicInputs.parameter_hash,
    ],
  ];
  for (const [fields, path, normalize, actual] of checks) {
    if (hasPresentField(source, fields) && normalize() !== actual) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.${path} must match the envelope public inputs`,
        `${context}.${path}`,
      );
    }
  }
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  if (
    domainAlias.value !== undefined &&
    assertNonBlankString(domainAlias.value, `${context}.domainSeparator`) !==
      publicInputs.domain_separator
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.domainSeparator must match the envelope public inputs`,
      `${context}.domainSeparator`,
    );
  }
}

function pickPresentFields(source, fields) {
  const result = {};
  for (const field of fields) {
    if (Object.prototype.hasOwnProperty.call(source, field)) {
      result[field] = source[field];
    }
  }
  return result;
}

function hasPresentField(source, fields) {
  return fields.some((field) => Object.prototype.hasOwnProperty.call(source, field));
}

function normalizeVeRangeBackend(value, name) {
  const backendTag = normalizePrivacyBackendTag(value ?? VERANGE_BACKEND, name);
  if (backendTag !== "Stark") {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be ${VERANGE_BACKEND}`,
      name,
    );
  }
  return VERANGE_BACKEND;
}

function normalizeVeRangeCircuitId(value, name) {
  const circuitId = assertNonBlankString(value ?? VERANGE_CIRCUIT_ID, name);
  if (
    circuitId !== VERANGE_CIRCUIT_ID &&
    circuitId !== "verange_transparent_range_v1"
  ) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must identify verange_transparent_range_v1`,
      name,
    );
  }
  return circuitId;
}

function normalizeVeRangePublicInputs(value, name) {
  const source = assertPlainObject(value, name);
  assertAllowedFields(
    source,
    new Set([
      "version",
      "commitments",
      "range_parameters",
      "rangeParameters",
      "aggregation_count",
      "aggregationCount",
      "domain_separator",
      "domainSeparator",
      "payload_digest",
      "payloadDigest",
    ]),
    name,
  );
  const rangeParametersAlias = readSingleAlias(
    source,
    ["range_parameters", "rangeParameters"],
    `${name}.rangeParameters`,
    "range parameters",
  );
  const rangeParameters = assertPlainObject(
    rangeParametersAlias.value,
    `${name}.rangeParameters`,
  );
  assertAllowedFields(
    rangeParameters,
    new Set(["bit_length", "bitLength", "commitment_scheme", "commitmentScheme"]),
    `${name}.rangeParameters`,
  );
  const bitLengthAlias = readSingleAlias(
    rangeParameters,
    ["bit_length", "bitLength"],
    `${name}.rangeParameters.bitLength`,
    "bit length",
  );
  const schemeAlias = readSingleAlias(
    rangeParameters,
    ["commitment_scheme", "commitmentScheme"],
    `${name}.rangeParameters.commitmentScheme`,
    "commitment scheme",
  );
  const aggregationAlias = readSingleAlias(
    source,
    ["aggregation_count", "aggregationCount"],
    `${name}.aggregationCount`,
    "aggregation count",
  );
  const domainAlias = readSingleAlias(
    source,
    ["domain_separator", "domainSeparator"],
    `${name}.domainSeparator`,
    "domain separator",
  );
  const digestAlias = readSingleAlias(
    source,
    ["payload_digest", "payloadDigest"],
    `${name}.payloadDigest`,
    "payload digest",
  );
  if (!Array.isArray(source.commitments) || source.commitments.length === 0) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name}.commitments must be a non-empty array`,
      `${name}.commitments`,
    );
  }
  const commitments = source.commitments.map((entry, index) =>
    Buffer.from(
      normalizeNonZeroFixedBytes(entry, `${name}.commitments[${index}]`, 32),
    ).toString("hex"),
  );
  const seen = new Set();
  for (const commitment of commitments) {
    if (seen.has(commitment)) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${name}.commitments must not contain duplicate commitments`,
        `${name}.commitments`,
      );
    }
    seen.add(commitment);
  }
  const aggregationCount = normalizeVeRangeAggregationCount(
    aggregationAlias.value,
    `${name}.aggregationCount`,
  );
  if (aggregationCount !== commitments.length) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name}.aggregationCount must equal the number of commitments`,
      `${name}.aggregationCount`,
    );
  }
  return {
    version: normalizeVeRangeVersion(source.version, `${name}.version`),
    commitments,
    range_parameters: {
      bit_length: normalizeVeRangeBitLength(
        bitLengthAlias.value,
        `${name}.rangeParameters.bitLength`,
      ),
      commitment_scheme: normalizeVeRangeCommitmentScheme(
        schemeAlias.value,
        `${name}.rangeParameters.commitmentScheme`,
      ),
    },
    aggregation_count: aggregationCount,
    domain_separator: assertNonBlankString(
      domainAlias.value,
      `${name}.domainSeparator`,
    ),
    payload_digest: Buffer.from(
      normalizeNonZeroFixedBytes(
        digestAlias.value,
        `${name}.payloadDigest`,
        32,
      ),
    ).toString("hex"),
  };
}

function normalizeVeRangeCommitments(source, name) {
  const listAlias = readSingleAlias(
    source,
    ["commitments", "rangeCommitments", "range_commitments"],
    `${name}.commitments`,
    "range commitment list",
  );
  const singleAlias = readSingleAlias(
    source,
    ["commitment", "rangeCommitment", "range_commitment", "valueCommitment", "value_commitment"],
    `${name}.commitment`,
    "range commitment",
  );
  if (listAlias.value !== undefined && singleAlias.value !== undefined) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must include either commitments or commitment, not both`,
      name,
    );
  }

  const commonFields = pickPresentFields(source, [
    "version",
    "bitLength",
    "bit_length",
    "aggregationCount",
    "aggregation_count",
    "commitmentScheme",
    "commitment_scheme",
    "domainSeparator",
    "domain_separator",
    "payloadDigest",
    "payload_digest",
    "txDigest",
    "tx_digest",
    "payload",
    "payloadBytes",
    "payload_bytes",
    "payloadJson",
    "payload_json",
    "maxPayloadBytes",
    "max_payload_bytes",
  ]);

  let commitmentSources;
  if (listAlias.value !== undefined) {
    if (!Array.isArray(listAlias.value) || listAlias.value.length === 0) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${name}.commitments must be a non-empty array`,
        `${name}.commitments`,
      );
    }
    commitmentSources = listAlias.value.map((entry, index) => {
      if (
        entry !== null &&
        typeof entry === "object" &&
        !Array.isArray(entry) &&
        !Buffer.isBuffer(entry) &&
        !ArrayBuffer.isView(entry) &&
        !(entry instanceof ArrayBuffer)
      ) {
        return {
          ...commonFields,
          ...assertPlainObject(entry, `${name}.commitments[${index}]`),
        };
      }
      return {
        ...commonFields,
        commitment: entry,
      };
    });
  } else {
    commitmentSources = [
      {
        ...commonFields,
        ...pickPresentFields(source, [
          "commitment",
          "rangeCommitment",
          "range_commitment",
          "valueCommitment",
          "value_commitment",
        ]),
      },
    ];
  }

  return commitmentSources.map((entry, index) =>
    buildRangeCommitment(entry, `${name}.commitments[${index}]`),
  );
}

function ensureVeRangeCommitmentConsistency(commitments, name) {
  const first = commitments[0];
  const seenCommitments = new Set();
  for (const [index, commitment] of commitments.entries()) {
    const prefix = `${name}.commitments[${index}]`;
    if (commitment.bit_length !== first.bit_length) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${prefix}.bitLength must match the first commitment`,
        `${prefix}.bitLength`,
      );
    }
    if (commitment.commitment_scheme !== first.commitment_scheme) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${prefix}.commitmentScheme must match the first commitment`,
        `${prefix}.commitmentScheme`,
      );
    }
    if (commitment.domain_separator !== first.domain_separator) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${prefix}.domainSeparator must match the first commitment`,
        `${prefix}.domainSeparator`,
      );
    }
    if (!fixedBytesEqual(commitment.payload_digest, first.payload_digest)) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${prefix}.payloadDigest must match the first commitment`,
        `${prefix}.payloadDigest`,
      );
    }
    const commitmentHex = Buffer.from(commitment.commitment).toString("hex");
    if (seenCommitments.has(commitmentHex)) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${name}.commitments must not contain duplicate commitments`,
        `${name}.commitments`,
      );
    }
    seenCommitments.add(commitmentHex);
  }
}

function normalizeVeRangeProofEnvelopeParts(
  source,
  context,
  { requireProofBytes = true } = {},
) {
  const backendAlias = readSingleAlias(
    source,
    ["backendTag", "backend_tag", "backend"],
    `${context}.backendTag`,
    "backend tag",
  );
  const circuitAlias = readSingleAlias(
    source,
    ["circuitId", "circuit_id"],
    `${context}.circuitId`,
    "circuit id",
  );
  const vkHashAlias = readSingleAlias(
    source,
    ["vkHash", "vk_hash", "verifierKeyHash", "verifyingKeyHash"],
    `${context}.vkHash`,
    "verifying key hash",
  );
  const proofAlias = readSingleAlias(
    source,
    ["proofBytes", "proof_bytes", "proof"],
    `${context}.proofBytes`,
    "proof bytes",
  );
  if (requireProofBytes && proofAlias.value === undefined) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.proofBytes is required`,
      `${context}.proofBytes`,
    );
  }
  const maxLimits = normalizeOptionalPrivacyMaxLimits(source, context);
  const aggregationAlias = readSingleAlias(
    source,
    ["aggregationCount", "aggregation_count"],
    `${context}.aggregationCount`,
    "aggregation count",
  );
  const commitments = normalizeVeRangeCommitments(source, context);
  ensureVeRangeCommitmentConsistency(commitments, context);
  const aggregationCount =
    aggregationAlias.value === undefined
      ? commitments.length
      : normalizeVeRangeAggregationCount(
          aggregationAlias.value,
          `${context}.aggregationCount`,
        );
  if (aggregationCount !== commitments.length) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.aggregationCount must equal the number of commitments`,
      `${context}.aggregationCount`,
    );
  }
  for (const [index, commitment] of commitments.entries()) {
    if (
      commitment.aggregation_count !== 1 &&
      commitment.aggregation_count !== aggregationCount
    ) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.commitments[${index}].aggregationCount must be 1 or match ${context}.aggregationCount`,
        `${context}.commitments[${index}].aggregationCount`,
      );
    }
  }

  const first = commitments[0];
  const publicInputs = {
    version: 1,
    commitments: commitments.map((entry) =>
      Buffer.from(entry.commitment).toString("hex"),
    ),
    range_parameters: {
      bit_length: first.bit_length,
      commitment_scheme: first.commitment_scheme,
    },
    aggregation_count: aggregationCount,
    domain_separator: first.domain_separator,
    payload_digest: Buffer.from(first.payload_digest).toString("hex"),
  };
  const publicInputBytes = Array.from(
    Buffer.from(
      canonicalJsonStringify(publicInputs, `${context}.publicInputs`),
      "utf8",
    ).values(),
  );
  return {
    backend: normalizeVeRangeBackend(
      backendAlias.value,
      `${context}.backendTag`,
    ),
    circuitId: normalizeVeRangeCircuitId(
      circuitAlias.value,
      `${context}.circuitId`,
    ),
    vkHash: normalizeNonZeroFixedBytes(vkHashAlias.value, `${context}.vkHash`, 32),
    publicInputs,
    publicInputBytes,
    proofBytes:
      proofAlias.value === undefined
        ? null
        : normalizeBoundedByteArray(proofAlias.value, `${context}.proofBytes`, {
            maxBytes: maxLimits.maxProofBytes ?? DEFAULT_PRIVACY_MAX_PROOF_BYTES,
          }),
    maxProofBytes: maxLimits.maxProofBytes,
    maxPublicInputBytes: maxLimits.maxPublicInputBytes,
  };
}

function veRangeDevProofBytes({ circuitId, vkHash, publicInputBytes }) {
  const digest = createHash("sha256");
  digest.update("iroha:verange:dev-fixture:v1", "utf8");
  digest.update(Buffer.from([0]));
  digest.update(circuitId, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(vkHash));
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(publicInputBytes));
  return Array.from(
    Buffer.concat([VERANGE_DEV_PROOF_PREFIX, digest.digest()]).values(),
  );
}

function decodeVeRangeEnvelope(value, name) {
  try {
    return noritoDecodePrivacyProofEnvelope(value);
  } catch (error) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be a valid OpenVerifyEnvelope`,
      name,
      error,
    );
  }
}

function parseVeRangePublicInputs(bytes, name) {
  let parsed;
  const text = Buffer.from(bytes).toString("utf8");
  try {
    parsed = JSON.parse(text);
  } catch (error) {
    fail(
      ValidationErrorCode.INVALID_JSON_VALUE,
      `${name} must contain valid JSON public inputs`,
      name,
      error,
    );
  }
  const normalized = normalizeVeRangePublicInputs(parsed, name);
  const canonical = canonicalJsonStringify(normalized, name);
  if (!Buffer.from(bytes).equals(Buffer.from(canonical, "utf8"))) {
    fail(
      ValidationErrorCode.INVALID_JSON_VALUE,
      `${name} must use canonical JSON encoding`,
      name,
    );
  }
  return normalized;
}

function ensureVeRangeVerificationExpectations(source, publicInputs, context) {
  if (
    hasPresentField(source, [
      "payloadDigest",
      "payload_digest",
      "txDigest",
      "tx_digest",
      "payload",
      "payloadBytes",
      "payload_bytes",
      "payloadJson",
      "payload_json",
    ])
  ) {
    const expectedDigest = normalizeVeRangePayloadDigest(source, context);
    if (Buffer.from(expectedDigest).toString("hex") !== publicInputs.payload_digest) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.payloadDigest must match the envelope public inputs`,
        `${context}.payloadDigest`,
      );
    }
  }
  const bitLengthAlias = readSingleAlias(
    source,
    ["bitLength", "bit_length"],
    `${context}.bitLength`,
    "bit length",
  );
  if (
    bitLengthAlias.value !== undefined &&
    normalizeVeRangeBitLength(bitLengthAlias.value, `${context}.bitLength`) !==
      publicInputs.range_parameters.bit_length
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.bitLength must match the envelope public inputs`,
      `${context}.bitLength`,
    );
  }
  const schemeAlias = readSingleAlias(
    source,
    ["commitmentScheme", "commitment_scheme"],
    `${context}.commitmentScheme`,
    "commitment scheme",
  );
  if (
    schemeAlias.value !== undefined &&
    normalizeVeRangeCommitmentScheme(
      schemeAlias.value,
      `${context}.commitmentScheme`,
    ) !== publicInputs.range_parameters.commitment_scheme
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.commitmentScheme must match the envelope public inputs`,
      `${context}.commitmentScheme`,
    );
  }
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  if (
    domainAlias.value !== undefined &&
    assertNonBlankString(domainAlias.value, `${context}.domainSeparator`) !==
      publicInputs.domain_separator
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.domainSeparator must match the envelope public inputs`,
      `${context}.domainSeparator`,
    );
  }
  const aggregationAlias = readSingleAlias(
    source,
    ["aggregationCount", "aggregation_count"],
    `${context}.aggregationCount`,
    "aggregation count",
  );
  if (
    aggregationAlias.value !== undefined &&
    normalizeVeRangeAggregationCount(
      aggregationAlias.value,
      `${context}.aggregationCount`,
    ) !== publicInputs.aggregation_count
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.aggregationCount must match the envelope public inputs`,
      `${context}.aggregationCount`,
    );
  }
  if (
    hasPresentField(source, [
      "commitments",
      "rangeCommitments",
      "range_commitments",
      "commitment",
      "rangeCommitment",
      "range_commitment",
      "valueCommitment",
      "value_commitment",
    ])
  ) {
    const expectedCommitments = normalizeVeRangeCommitments(source, context);
    const expectedCommitmentHex = expectedCommitments.map((entry) =>
      Buffer.from(entry.commitment).toString("hex"),
    );
    if (
      expectedCommitmentHex.length !== publicInputs.commitments.length ||
      expectedCommitmentHex.some(
        (commitment, index) => commitment !== publicInputs.commitments[index],
      )
    ) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.commitments must match the envelope public inputs`,
        `${context}.commitments`,
      );
    }
  }
}

function normalizeAnonymousPgcReceiverSetFromSource(source, context) {
  const receiverSetAlias = readSingleAlias(
    source,
    ["receiverSet", "receiver_set"],
    `${context}.receiverSet`,
    "receiver set",
  );
  if (receiverSetAlias.value !== undefined) {
    return normalizeAnonymousPgcReceiverSet(
      receiverSetAlias.value,
      `${context}.receiverSet`,
    );
  }
  return buildAnonymousPgcReceiverSet(
    pickPresentFields(source, ["version", "threshold", "k", "receivers"]),
    `${context}.receiverSet`,
  );
}

function normalizeAnonymousPgcPublicInputs(value, name) {
  const source = assertPlainObject(value, name);
  assertAllowedFields(
    source,
    new Set([
      "version",
      "anonymity_set_root",
      "anonymitySetRoot",
      "tx_digest",
      "txDigest",
      "balance_commitments",
      "balanceCommitments",
      "receiver_set_commitment",
      "receiverSetCommitment",
      "receiver_ciphertext_commitments",
      "receiverCiphertextCommitments",
      "receiver_threshold",
      "receiverThreshold",
      "receiver_count",
      "receiverCount",
      "link_tag",
      "linkTag",
      "range_commitments",
      "rangeCommitments",
      "chain_id",
      "chainId",
      "domain_separator",
      "domainSeparator",
    ]),
    name,
  );
  const anonymityRootAlias = readSingleAlias(
    source,
    ["anonymity_set_root", "anonymitySetRoot"],
    `${name}.anonymitySetRoot`,
    "anonymity set root",
  );
  const txDigestAlias = readSingleAlias(
    source,
    ["tx_digest", "txDigest"],
    `${name}.txDigest`,
    "transaction digest",
  );
  const balanceAlias = readSingleAlias(
    source,
    ["balance_commitments", "balanceCommitments"],
    `${name}.balanceCommitments`,
    "balance commitments",
  );
  const receiverSetAlias = readSingleAlias(
    source,
    ["receiver_set_commitment", "receiverSetCommitment"],
    `${name}.receiverSetCommitment`,
    "receiver-set commitment",
  );
  const receiverCiphertextAlias = readSingleAlias(
    source,
    ["receiver_ciphertext_commitments", "receiverCiphertextCommitments"],
    `${name}.receiverCiphertextCommitments`,
    "receiver ciphertext commitments",
  );
  const receiverThresholdAlias = readSingleAlias(
    source,
    ["receiver_threshold", "receiverThreshold"],
    `${name}.receiverThreshold`,
    "receiver threshold",
  );
  const receiverCountAlias = readSingleAlias(
    source,
    ["receiver_count", "receiverCount"],
    `${name}.receiverCount`,
    "receiver count",
  );
  const linkTagAlias = readSingleAlias(
    source,
    ["link_tag", "linkTag"],
    `${name}.linkTag`,
    "link tag",
  );
  const rangeAlias = readSingleAlias(
    source,
    ["range_commitments", "rangeCommitments"],
    `${name}.rangeCommitments`,
    "range commitments",
  );
  const chainAlias = readSingleAlias(
    source,
    ["chain_id", "chainId"],
    `${name}.chainId`,
    "chain id",
  );
  const domainAlias = readSingleAlias(
    source,
    ["domain_separator", "domainSeparator"],
    `${name}.domainSeparator`,
    "domain separator",
  );
  const receiverCiphertextCommitments = normalizeAnonymousPgcCommitmentList(
    receiverCiphertextAlias.value,
    `${name}.receiverCiphertextCommitments`,
    ANON_PGC_MAX_RECEIVERS,
  );
  const receiverThreshold = normalizePositiveU32(
    receiverThresholdAlias.value,
    `${name}.receiverThreshold`,
  );
  const receiverCount = normalizePositiveU32(
    receiverCountAlias.value,
    `${name}.receiverCount`,
  );
  if (receiverCount !== receiverCiphertextCommitments.length) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name}.receiverCount must match receiverCiphertextCommitments length`,
      `${name}.receiverCount`,
    );
  }
  if (receiverThreshold > receiverCount) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name}.receiverThreshold must not exceed receiverCount`,
      `${name}.receiverThreshold`,
    );
  }
  return {
    version: normalizeAnonymousPgcVersion(source.version, `${name}.version`),
    anonymity_set_root: Buffer.from(
      normalizeNonZeroFixedBytes(
        anonymityRootAlias.value,
        `${name}.anonymitySetRoot`,
        32,
      ),
    ).toString("hex"),
    tx_digest: Buffer.from(
      normalizeNonZeroFixedBytes(txDigestAlias.value, `${name}.txDigest`, 32),
    ).toString("hex"),
    balance_commitments: normalizeAnonymousPgcCommitmentList(
      balanceAlias.value,
      `${name}.balanceCommitments`,
      ANON_PGC_MAX_BALANCE_COMMITMENTS,
    ).map((entry) => Buffer.from(entry).toString("hex")),
    receiver_set_commitment: Buffer.from(
      normalizeNonZeroFixedBytes(
        receiverSetAlias.value,
        `${name}.receiverSetCommitment`,
        32,
      ),
    ).toString("hex"),
    receiver_ciphertext_commitments: receiverCiphertextCommitments.map((entry) =>
      Buffer.from(entry).toString("hex"),
    ),
    receiver_threshold: receiverThreshold,
    receiver_count: receiverCount,
    link_tag: Buffer.from(
      normalizeNonZeroFixedBytes(linkTagAlias.value, `${name}.linkTag`, 32),
    ).toString("hex"),
    range_commitments: normalizeAnonymousPgcCommitmentList(
      rangeAlias.value,
      `${name}.rangeCommitments`,
      ANON_PGC_MAX_RANGE_COMMITMENTS,
    ).map((entry) => Buffer.from(entry).toString("hex")),
    chain_id: assertNonBlankString(chainAlias.value, `${name}.chainId`),
    domain_separator: assertNonBlankString(
      domainAlias.value,
      `${name}.domainSeparator`,
    ),
  };
}

function normalizeAnonymousPgcProofParts(
  source,
  context,
  { requireProofBytes = false } = {},
) {
  const backendAlias = readSingleAlias(
    source,
    ["backendTag", "backend_tag", "backend"],
    `${context}.backendTag`,
    "backend tag",
  );
  const circuitAlias = readSingleAlias(
    source,
    ["circuitId", "circuit_id"],
    `${context}.circuitId`,
    "circuit id",
  );
  const vkHashAlias = readSingleAlias(
    source,
    ["vkHash", "vk_hash", "verifierKeyHash", "verifyingKeyHash"],
    `${context}.vkHash`,
    "verifying key hash",
  );
  const proofAlias = readSingleAlias(
    source,
    ["proofBytes", "proof_bytes", "proof"],
    `${context}.proofBytes`,
    "proof bytes",
  );
  if (requireProofBytes && proofAlias.value === undefined) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.proofBytes is required`,
      `${context}.proofBytes`,
    );
  }
  const maxLimits = normalizeOptionalPrivacyMaxLimits(source, context);
  const receiverSet = normalizeAnonymousPgcReceiverSetFromSource(source, context);
  const anonymityRoot = normalizeNonZeroFixedBytes(
    source.anonymitySetRoot ?? source.anonymity_set_root,
    `${context}.anonymitySetRoot`,
    32,
  );
  const txDigest = normalizeAnonymousPgcPayloadDigest(source, context);
  const balanceCommitments = normalizeAnonymousPgcCommitmentList(
    source.balanceCommitments ?? source.balance_commitments,
    `${context}.balanceCommitments`,
    ANON_PGC_MAX_BALANCE_COMMITMENTS,
  );
  const rangeCommitments = normalizeAnonymousPgcCommitmentList(
    source.rangeCommitments ?? source.range_commitments,
    `${context}.rangeCommitments`,
    ANON_PGC_MAX_RANGE_COMMITMENTS,
  );
  const publicInputs = {
    version: 1,
    anonymity_set_root: Buffer.from(anonymityRoot).toString("hex"),
    tx_digest: Buffer.from(txDigest).toString("hex"),
    balance_commitments: balanceCommitments.map((entry) =>
      Buffer.from(entry).toString("hex"),
    ),
    receiver_set_commitment: Buffer.from(
      receiverSet.receiver_set_commitment,
    ).toString("hex"),
    receiver_ciphertext_commitments: receiverSet.receivers.map((entry) =>
      Buffer.from(entry.ciphertext_commitment).toString("hex"),
    ),
    receiver_threshold: receiverSet.threshold,
    receiver_count: receiverSet.receiver_count,
    link_tag: Buffer.from(
      normalizeNonZeroFixedBytes(source.linkTag ?? source.link_tag, `${context}.linkTag`, 32),
    ).toString("hex"),
    range_commitments: rangeCommitments.map((entry) =>
      Buffer.from(entry).toString("hex"),
    ),
    chain_id: assertNonBlankString(source.chainId ?? source.chain_id, `${context}.chainId`),
    domain_separator: assertNonBlankString(
      source.domainSeparator ?? source.domain_separator ?? ANON_PGC_DOMAIN_SEPARATOR,
      `${context}.domainSeparator`,
    ),
  };
  const publicInputBytes = Array.from(
    Buffer.from(
      canonicalJsonStringify(publicInputs, `${context}.publicInputs`),
      "utf8",
    ).values(),
  );
  return {
    backend: normalizeAnonymousPgcBackend(
      backendAlias.value,
      `${context}.backendTag`,
    ),
    circuitId: normalizeAnonymousPgcCircuitId(
      circuitAlias.value,
      `${context}.circuitId`,
    ),
    vkHash: normalizeNonZeroFixedBytes(vkHashAlias.value, `${context}.vkHash`, 32),
    receiverSet,
    publicInputs,
    publicInputBytes,
    proofBytes:
      proofAlias.value === undefined
        ? null
        : normalizeBoundedByteArray(proofAlias.value, `${context}.proofBytes`, {
            maxBytes: maxLimits.maxProofBytes ?? DEFAULT_PRIVACY_MAX_PROOF_BYTES,
          }),
    maxProofBytes: maxLimits.maxProofBytes,
    maxPublicInputBytes: maxLimits.maxPublicInputBytes,
  };
}

function anonymousPgcDevProofBytes({ circuitId, vkHash, publicInputBytes }) {
  const digest = createHash("sha256");
  digest.update("iroha:anonymous-pgc:dev-fixture:v1", "utf8");
  digest.update(Buffer.from([0]));
  digest.update(circuitId, "utf8");
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(vkHash));
  digest.update(Buffer.from([0]));
  digest.update(Buffer.from(publicInputBytes));
  return Array.from(
    Buffer.concat([ANON_PGC_DEV_PROOF_PREFIX, digest.digest()]).values(),
  );
}

function parseAnonymousPgcPublicInputs(bytes, name) {
  let parsed;
  try {
    parsed = JSON.parse(Buffer.from(bytes).toString("utf8"));
  } catch (error) {
    fail(
      ValidationErrorCode.INVALID_JSON_VALUE,
      `${name} must contain valid JSON public inputs`,
      name,
      error,
    );
  }
  const normalized = normalizeAnonymousPgcPublicInputs(parsed, name);
  const canonical = canonicalJsonStringify(normalized, name);
  if (!Buffer.from(bytes).equals(Buffer.from(canonical, "utf8"))) {
    fail(
      ValidationErrorCode.INVALID_JSON_VALUE,
      `${name} must use canonical JSON encoding`,
      name,
    );
  }
  return normalized;
}

function ensureAnonymousPgcVerificationExpectations(source, publicInputs, context) {
  if (
    hasPresentField(source, [
      "payload",
      "payloadBytes",
      "payload_bytes",
      "payloadJson",
      "payload_json",
      "txDigest",
      "tx_digest",
      "payloadDigest",
      "payload_digest",
    ])
  ) {
    const expectedDigest = normalizeAnonymousPgcPayloadDigest(source, context);
    if (Buffer.from(expectedDigest).toString("hex") !== publicInputs.tx_digest) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.txDigest must match the envelope public inputs`,
        `${context}.txDigest`,
      );
    }
  }
  for (const [fields, path, normalize, actual] of [
    [
      ["anonymitySetRoot", "anonymity_set_root"],
      "anonymitySetRoot",
      (value) => Buffer.from(normalizeNonZeroFixedBytes(value, `${context}.anonymitySetRoot`, 32)).toString("hex"),
      publicInputs.anonymity_set_root,
    ],
    [
      ["linkTag", "link_tag"],
      "linkTag",
      (value) => Buffer.from(normalizeNonZeroFixedBytes(value, `${context}.linkTag`, 32)).toString("hex"),
      publicInputs.link_tag,
    ],
  ]) {
    const alias = readSingleAlias(source, fields, `${context}.${path}`, path);
    if (alias.value !== undefined && normalize(alias.value) !== actual) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.${path} must match the envelope public inputs`,
        `${context}.${path}`,
      );
    }
  }
  if (hasPresentField(source, ["receiverSet", "receiver_set", "receivers"])) {
    const receiverSet = normalizeAnonymousPgcReceiverSetFromSource(source, context);
    if (
      Buffer.from(receiverSet.receiver_set_commitment).toString("hex") !==
        publicInputs.receiver_set_commitment ||
      receiverSet.threshold !== publicInputs.receiver_threshold ||
      receiverSet.receiver_count !== publicInputs.receiver_count
    ) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.receiverSet must match the envelope public inputs`,
        `${context}.receiverSet`,
      );
    }
    const ciphertextCommitments = receiverSet.receivers.map((entry) =>
      Buffer.from(entry.ciphertext_commitment).toString("hex"),
    );
    if (
      ciphertextCommitments.length !==
        publicInputs.receiver_ciphertext_commitments.length ||
      ciphertextCommitments.some(
        (entry, index) => entry !== publicInputs.receiver_ciphertext_commitments[index],
      )
    ) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.receiverSet ciphertext commitments must match the envelope public inputs`,
        `${context}.receiverSet`,
      );
    }
  }
  for (const [fields, path, maxItems, actual] of [
    [
      ["balanceCommitments", "balance_commitments"],
      "balanceCommitments",
      ANON_PGC_MAX_BALANCE_COMMITMENTS,
      publicInputs.balance_commitments,
    ],
    [
      ["rangeCommitments", "range_commitments"],
      "rangeCommitments",
      ANON_PGC_MAX_RANGE_COMMITMENTS,
      publicInputs.range_commitments,
    ],
  ]) {
    const alias = readSingleAlias(source, fields, `${context}.${path}`, path);
    if (alias.value !== undefined) {
      const expected = normalizeAnonymousPgcCommitmentList(
        alias.value,
        `${context}.${path}`,
        maxItems,
      ).map((entry) => Buffer.from(entry).toString("hex"));
      if (
        expected.length !== actual.length ||
        expected.some((entry, index) => entry !== actual[index])
      ) {
        fail(
          ValidationErrorCode.INVALID_OBJECT,
          `${context}.${path} must match the envelope public inputs`,
          `${context}.${path}`,
        );
      }
    }
  }
  const chainAlias = readSingleAlias(
    source,
    ["chainId", "chain_id"],
    `${context}.chainId`,
    "chain id",
  );
  if (
    chainAlias.value !== undefined &&
    assertNonBlankString(chainAlias.value, `${context}.chainId`) !==
      publicInputs.chain_id
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.chainId must match the envelope public inputs`,
      `${context}.chainId`,
    );
  }
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  if (
    domainAlias.value !== undefined &&
    assertNonBlankString(domainAlias.value, `${context}.domainSeparator`) !==
      publicInputs.domain_separator
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.domainSeparator must match the envelope public inputs`,
      `${context}.domainSeparator`,
    );
  }
}

function normalizePrivacyVerifierKeyIdFromOptions(source, name) {
  const alias = readSingleAlias(
    source,
    [
      "id",
      "verifierKey",
      "verifierKeyId",
      "verifyingKeyId",
      "keyId",
      "vkRef",
      "verifyingKeyRef",
    ],
    `${name}.id`,
    "verifier key id",
  );
  const id = normalizeVerifyingKeyId(alias.value, `${name}.id`);
  if (id === null) {
    fail(ValidationErrorCode.INVALID_OBJECT, `${name}.id is required`, `${name}.id`);
  }
  return {
    ...id,
    backend: assertProductionVerifyBackendLabel(id.backend, `${name}.id.backend`),
  };
}

function normalizePrivacyVerifyingKeyBox(value, id, name) {
  if (value === undefined || value === null) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} is required when a verifying key alias is present`,
      name,
    );
  }
  let backend = id.backend;
  let bytesValue = value;
  if (value && typeof value === "object" && !Buffer.isBuffer(value) && !ArrayBuffer.isView(value) && !(value instanceof ArrayBuffer) && !Array.isArray(value)) {
    const source = assertPlainObject(value, name);
    assertAllowedFields(
      source,
      new Set(["backend", "backendId", "bytes", "keyBytes", "verifyingKeyBytes", "vkBytes"]),
      name,
    );
    const backendAlias = readSingleAlias(
      source,
      ["backend", "backendId"],
      `${name}.backend`,
      "verifying key backend",
    );
    const bytesAlias = readSingleAlias(
      source,
      ["bytes", "keyBytes", "verifyingKeyBytes", "vkBytes"],
      `${name}.bytes`,
      "verifying key bytes",
    );
    backend =
      backendAlias.key === null
        ? id.backend
        : assertNonBlankString(backendAlias.value, `${name}.backend`);
    if (bytesAlias.key === null) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${name}.bytes is required`,
        `${name}.bytes`,
      );
    }
    bytesValue = bytesAlias.value;
  }
  if (backend !== id.backend) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name}.backend must match verifier key id backend`,
      `${name}.backend`,
    );
  }
  return {
    backend,
    bytes: normalizeBoundedByteArray(bytesValue, `${name}.bytes`, {
      maxBytes: UINT32_MAX,
    }),
  };
}

function normalizePrivacyVerifierKeyRecord(sourceValue, id, context, options = {}) {
  const source = assertPlainObject(sourceValue, context);
  assertAllowedFields(
    source,
    new Set([
      "id",
      "verifierKey",
      "verifierKeyId",
      "verifyingKeyId",
      "keyId",
      "vkRef",
      "verifyingKeyRef",
      "record",
      "verifierRecord",
      "verifyingKeyRecord",
      "version",
      "circuitId",
      "circuit_id",
      "ownerManifestId",
      "owner_manifest_id",
      "owner",
      "namespace",
      "backendTag",
      "backend_tag",
      "backend",
      "curve",
      "publicInputsSchemaHash",
      "public_inputs_schema_hash",
      "schemaHash",
      "schema_hash",
      "commitment",
      "verifyingKeyCommitment",
      "vkCommitment",
      "vk_commitment",
      "vkLen",
      "vk_len",
      "verifyingKeyLength",
      "maxProofBytes",
      "max_proof_bytes",
      "gasScheduleId",
      "gas_schedule_id",
      "metadataUriCid",
      "metadata_uri_cid",
      "vkBytesCid",
      "vk_bytes_cid",
      "activationHeight",
      "activation_height",
      "withdrawHeight",
      "withdraw_height",
      "key",
      "verifyingKey",
      "verifying_key",
      "verifyingKeyBytes",
      "verifying_key_bytes",
      "vkBytes",
      "vk_bytes",
      "status",
    ]),
    context,
  );
  const backendAlias = readSingleAlias(
    source,
    ["backendTag", "backend_tag", "backend"],
    `${context}.backendTag`,
    "backend tag",
  );
  const backendTag =
    backendAlias.key === null
      ? inferPrivacyBackendTagFromVerifierId(id, `${context}.backendTag`)
      : normalizePrivacyBackendTag(backendAlias.value, `${context}.backendTag`);
  assertRegisterablePrivacyBackendTag(backendTag, `${context}.backendTag`);
  const circuitAlias = readSingleAlias(
    source,
    ["circuitId", "circuit_id"],
    `${context}.circuitId`,
    "circuit id",
  );
  const ownerAlias = readSingleAlias(
    source,
    ["ownerManifestId", "owner_manifest_id", "owner"],
    `${context}.ownerManifestId`,
    "owner manifest id",
  );
  const keyAlias = readSingleAlias(
    source,
    [
      "key",
      "verifyingKey",
      "verifying_key",
      "verifyingKeyBytes",
      "verifying_key_bytes",
      "vkBytes",
      "vk_bytes",
    ],
    `${context}.key`,
    "verifying key bytes",
  );
  const key =
    keyAlias.key === null
      ? null
      : normalizePrivacyVerifyingKeyBox(keyAlias.value, id, `${context}.key`);
  const vkLenAlias = readSingleAlias(
    source,
    ["vkLen", "vk_len", "verifyingKeyLength"],
    `${context}.vkLen`,
    "verifying key length",
  );
  const vkLen =
    vkLenAlias.key === null
      ? key?.bytes.length ?? 0
      : normalizeU32(vkLenAlias.value, `${context}.vkLen`);
  if (key && vkLen !== key.bytes.length) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.vkLen must match inline verifying key byte length`,
      `${context}.vkLen`,
    );
  }
  const status = options.forceStatus ?? normalizeConfidentialStatus(
    source.status,
    `${context}.status`,
    options.defaultStatus ?? "Proposed",
  );
  if (!options.allowWithdrawn && status === "Withdrawn") {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${context}.status cannot be Withdrawn when registering a verifier key`,
      `${context}.status`,
    );
  }
  const gasScheduleAlias = readSingleAlias(
    source,
    ["gasScheduleId", "gas_schedule_id"],
    `${context}.gasScheduleId`,
    "gas schedule id",
  );
  const gasScheduleId = normalizeOptionalNonBlankString(
    gasScheduleAlias.value,
    `${context}.gasScheduleId`,
  );
  if (gasScheduleId === null) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${context}.gasScheduleId is required`,
      `${context}.gasScheduleId`,
    );
  }
  const schemaHashAlias = readSingleAlias(
    source,
    ["publicInputsSchemaHash", "public_inputs_schema_hash", "schemaHash", "schema_hash"],
    `${context}.publicInputsSchemaHash`,
    "public-input schema hash",
  );
  const commitmentAlias = readSingleAlias(
    source,
    ["commitment", "verifyingKeyCommitment", "vkCommitment", "vk_commitment"],
    `${context}.commitment`,
    "verifying key commitment",
  );
  const metadataAlias = readSingleAlias(
    source,
    ["metadataUriCid", "metadata_uri_cid"],
    `${context}.metadataUriCid`,
    "metadata URI CID",
  );
  const vkBytesCidAlias = readSingleAlias(
    source,
    ["vkBytesCid", "vk_bytes_cid"],
    `${context}.vkBytesCid`,
    "verifying key bytes CID",
  );
  const activationAlias = readSingleAlias(
    source,
    ["activationHeight", "activation_height"],
    `${context}.activationHeight`,
    "activation height",
  );
  const withdrawAlias = readSingleAlias(
    source,
    ["withdrawHeight", "withdraw_height"],
    `${context}.withdrawHeight`,
    "withdraw height",
  );
  const activationHeight = normalizeOptionalU64(
    activationAlias.value,
    `${context}.activationHeight`,
  );
  const withdrawHeight = normalizeOptionalU64(
    withdrawAlias.value,
    `${context}.withdrawHeight`,
  );
  if (activationHeight !== null && withdrawHeight !== null && withdrawHeight < activationHeight) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.withdrawHeight must be >= activationHeight`,
      `${context}.withdrawHeight`,
    );
  }
  return {
    version: normalizePositiveU32(source.version, `${context}.version`),
    circuit_id: assertNonBlankString(
      circuitAlias.value,
      `${context}.circuitId`,
    ),
    owner_manifest_id: normalizeOptionalNonBlankString(
      ownerAlias.value,
      `${context}.ownerManifestId`,
    ),
    namespace: assertNonBlankString(source.namespace ?? "core", `${context}.namespace`),
    backend: backendTag,
    curve: normalizePrivacyCurve(source.curve, backendTag, `${context}.curve`),
    public_inputs_schema_hash: normalizeNonZeroFixedBytes(
      schemaHashAlias.value,
      `${context}.publicInputsSchemaHash`,
      32,
    ),
    commitment: normalizeNonZeroFixedBytes(
      commitmentAlias.value,
      `${context}.commitment`,
      32,
    ),
    vk_len: vkLen,
    max_proof_bytes: normalizePositiveU32AliasOrDefault(
      source,
      ["maxProofBytes", "max_proof_bytes"],
      `${context}.maxProofBytes`,
      "maximum proof byte length",
      DEFAULT_PRIVACY_MAX_PROOF_BYTES,
    ),
    gas_schedule_id: gasScheduleId,
    metadata_uri_cid: normalizeOptionalNonBlankString(
      metadataAlias.value,
      `${context}.metadataUriCid`,
    ),
    vk_bytes_cid: normalizeOptionalNonBlankString(
      vkBytesCidAlias.value,
      `${context}.vkBytesCid`,
    ),
    activation_height: activationHeight,
    withdraw_height: withdrawHeight,
    key,
    status,
  };
}

function normalizeZkAceVerifierKeyId(value, name) {
  const id = normalizeVerifyingKeyId(value, name);
  if (id === null) {
    fail(ValidationErrorCode.INVALID_OBJECT, `${name} is required`, name);
  }
  if (id.backend !== ZK_ACE_BACKEND) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name}.backend must be ${ZK_ACE_BACKEND}`,
      `${name}.backend`,
    );
  }
  return id;
}

function normalizeZkAceAction(value, name) {
  const action = assertNonBlankString(value ?? ZK_ACE_ACTION_TRANSFER, name);
  if (action !== ZK_ACE_ACTION_TRANSFER) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be ${ZK_ACE_ACTION_TRANSFER}`,
      name,
    );
  }
  return action;
}

function normalizeZkAceDomainTag(value, name) {
  const domainTag = assertNonBlankString(value ?? ZK_ACE_DOMAIN_TAG, name);
  if (domainTag !== ZK_ACE_DOMAIN_TAG) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be ${ZK_ACE_DOMAIN_TAG}`,
      name,
    );
  }
  return domainTag;
}

function normalizeZkAceAllowedAccounts(value, name) {
  if (!Array.isArray(value)) {
    fail(ValidationErrorCode.INVALID_OBJECT, `${name} must be an array`, name);
  }
  if (value.length === 0) {
    fail(ValidationErrorCode.INVALID_OBJECT, `${name} must be non-empty`, name);
  }
  if (value.length > 16) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} must contain at most 16 accounts`,
      name,
    );
  }
  const accounts = value.map((entry, index) =>
    normalizeAccountId(entry, `${name}[${index}]`),
  );
  const seen = new Set();
  for (const account of accounts) {
    if (seen.has(account)) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${name} must not contain duplicates`,
        name,
      );
    }
    seen.add(account);
  }
  return accounts.sort();
}

function normalizeZkAcePublicInputs(value, name) {
  const source = assertPlainObject(value, name);
  const version = asNonNegativeInteger(source.version ?? 1, `${name}.version`);
  if (version !== 1) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name}.version must be 1`,
      `${name}.version`,
    );
  }
  return {
    version,
    identity_commitment: normalizeNonZeroFixedBytes(
      source.identityCommitment ?? source.identity_commitment,
      `${name}.identityCommitment`,
      32,
    ),
    tx_digest: normalizeNonZeroFixedBytes(
      source.txDigest ?? source.tx_digest,
      `${name}.txDigest`,
      32,
    ),
    chain_id: assertNonBlankString(source.chainId ?? source.chain_id, `${name}.chainId`),
    domain_tag: normalizeZkAceDomainTag(source.domainTag ?? source.domain_tag, `${name}.domainTag`),
    action_class: normalizeZkAceAction(
      source.actionClass ?? source.action_class,
      `${name}.actionClass`,
    ),
    replay_nullifier: normalizeNonZeroFixedBytes(
      source.replayNullifier ?? source.replay_nullifier,
      `${name}.replayNullifier`,
      32,
    ),
    policy_hash: normalizeNonZeroFixedBytes(
      source.policyHash ?? source.policy_hash,
      `${name}.policyHash`,
      32,
    ),
    from: normalizeAccountId(source.fromAccountId ?? source.from, `${name}.from`),
    to: normalizeAccountId(source.toAccountId ?? source.to, `${name}.to`),
    asset: assertString(
      source.assetDefinitionId ?? source.asset_definition_id ?? source.asset,
      `${name}.asset`,
    ),
    amount: asPositiveU128JsonNumber(source.amount, `${name}.amount`),
    verifier_key_id: normalizeZkAceVerifierKeyId(
      source.verifierKeyId ?? source.verifier_key_id ?? source.verifyingKeyRef,
      `${name}.verifierKeyId`,
    ),
  };
}

function fixedBytesEqual(left, right) {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
}

function ensureZkAceAuthorizationMatchesTransfer(publicInputs, payload, name) {
  const byteFields = [
    ["identity_commitment", "identityCommitment"],
    ["tx_digest", "txDigest"],
    ["replay_nullifier", "replayNullifier"],
    ["policy_hash", "policyHash"],
  ];
  for (const [field, transferField] of byteFields) {
    if (!fixedBytesEqual(publicInputs[field], payload[field])) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${name}.publicInputs.${field} must match zkAceAuthorizedTransfer.${transferField}`,
        `${name}.publicInputs.${field}`,
      );
    }
  }
  const scalarFields = [
    ["chain_id", "chainId"],
    ["domain_tag", "domainTag"],
    ["action_class", "actionClass"],
    ["from", "from"],
    ["to", "to"],
    ["asset", "asset"],
    ["amount", "amount"],
  ];
  for (const [field, transferField] of scalarFields) {
    if (publicInputs[field] !== payload[field]) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${name}.publicInputs.${field} must match zkAceAuthorizedTransfer.${transferField}`,
        `${name}.publicInputs.${field}`,
      );
    }
  }
  if (
    publicInputs.verifier_key_id.backend !== payload.proof.vk_ref.backend ||
    publicInputs.verifier_key_id.name !== payload.proof.vk_ref.name
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name}.publicInputs.verifier_key_id must match zkAceAuthorizedTransfer.proof.verifyingKeyRef`,
      `${name}.publicInputs.verifier_key_id`,
    );
  }
}

function normalizeZkAceWitness(value, name) {
  const source = assertPlainObject(value, name);
  return {
    identity_root: normalizeNonZeroFixedBytes(
      source.identityRoot ?? source.identity_root,
      `${name}.identityRoot`,
      32,
    ),
    identity_blinding: normalizeNonZeroFixedBytes(
      source.identityBlinding ?? source.identity_blinding,
      `${name}.identityBlinding`,
      32,
    ),
    replay_secret: normalizeNonZeroFixedBytes(
      source.replaySecret ?? source.replay_secret,
      `${name}.replaySecret`,
      32,
    ),
  };
}

function normalizeZkAceNativeResult(resultJson, name) {
  let result;
  try {
    result = JSON.parse(resultJson);
  } catch (error) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} native prover returned invalid JSON: ${error.message}`,
      name,
    );
  }
  return assertPlainObject(result, name);
}

function sanitizeZkAceNativeAuthorizationProofError(error, name) {
  const message = error?.message ?? String(error ?? "");
  const sanitizedMessage =
    /PRIVACY_FFI_ERROR_PRODUCTION_DISABLED|production[- ]disabled|Iroha production allowlist/i.test(
      message,
    )
      ? ZK_ACE_PRODUCTION_DISABLED_MESSAGE
      : "native ZK-ACE prover failed";
  fail(ValidationErrorCode.INVALID_OBJECT, sanitizedMessage, name);
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
  const normalizeDynamicHints = (entries, name) => {
    if (entries === undefined || entries === null) {
      return [];
    }
    if (!Array.isArray(entries)) {
      fail(ValidationErrorCode.INVALID_OBJECT, `${name} must be an array of dynamic access hints`, name);
    }
    return entries.map((entry, index) => {
      const hint = assertPlainObject(entry, `${name}[${index}]`);
      const maxKeys = asNonNegativeInteger(
        hint.max_keys ?? hint.maxKeys,
        `${name}[${index}].maxKeys`,
      );
      if (maxKeys > 0xffffffff) {
        fail(
          ValidationErrorCode.VALUE_OUT_OF_RANGE,
          `${name}[${index}].maxKeys must fit in u32`,
          `${name}[${index}].maxKeys`,
        );
      }
      return {
        base_key: assertString(
          hint.base_key ?? hint.baseKey,
          `${name}[${index}].baseKey`,
        ),
        key_type: assertString(
          hint.key_type ?? hint.keyType,
          `${name}[${index}].keyType`,
        ),
        bound_kind: assertString(
          hint.bound_kind ?? hint.boundKind,
          `${name}[${index}].boundKind`,
        ),
        max_keys: maxKeys,
      };
    });
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
    dynamic_reads: normalizeDynamicHints(
      hints.dynamic_reads ?? hints.dynamicReads,
      `${context}.dynamicReads`,
    ),
    dynamic_writes: normalizeDynamicHints(
      hints.dynamic_writes ?? hints.dynamicWrites,
      `${context}.dynamicWrites`,
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
  const normalized = {
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
  if (Object.prototype.hasOwnProperty.call(source, "kotoba")) {
    normalized.kotoba =
      source.kotoba === null
        ? null
        : normalizeContractKotobaEntries(source.kotoba, "manifest.kotoba");
  }
  if (Object.prototype.hasOwnProperty.call(source, "provenance")) {
    normalized.provenance =
      source.provenance === null
        ? null
        : normalizeManifestProvenance(source.provenance, "manifest.provenance");
  }
  return normalized;
}

function normalizeContractKotobaEntries(value, name) {
  if (!Array.isArray(value)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${name} must be an array of translation entries`,
      name,
    );
  }
  return value.map((entry, index) => {
    const normalizedEntry = assertPlainObject(entry, `${name}[${index}]`);
    return {
      msg_id: assertString(
        normalizedEntry.msg_id ?? normalizedEntry.msgId,
        `${name}[${index}].msg_id`,
      ),
      translations: normalizeJsonValue(
        normalizedEntry.translations,
        `${name}[${index}].translations`,
      ),
    };
  });
}

function decodeManifestVarint(buffer, startIndex, context) {
  let value = 0n;
  let shift = 0n;
  let index = startIndex;
  while (index < buffer.length) {
    const byte = BigInt(buffer[index]);
    value |= (byte & 0x7fn) << shift;
    index += 1;
    if ((byte & 0x80n) === 0n) {
      if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
        fail(
          ValidationErrorCode.INVALID_MULTIHASH,
          `${context} contains an oversized multihash varint`,
          context,
        );
      }
      return { value: Number(value), nextIndex: index };
    }
    shift += 7n;
    if (shift > 63n) {
      fail(
        ValidationErrorCode.INVALID_MULTIHASH,
        `${context} contains an invalid multihash varint`,
        context,
      );
    }
  }
  fail(
    ValidationErrorCode.INVALID_MULTIHASH,
    `${context} contains a truncated multihash varint`,
    context,
  );
}

function normalizeManifestPublicKeyLiteral(value, name) {
  const literal = assertString(value, name).trim();
  let prefixedAlgorithm = null;
  let multihashLiteral = literal;
  const separator = literal.indexOf(":");
  if (separator > 0) {
    prefixedAlgorithm = literal.slice(0, separator).trim().toLowerCase();
    multihashLiteral = literal.slice(separator + 1);
  }
  const canonical = canonicalizeMultihashHex(multihashLiteral, name);
  const bytes = Buffer.from(canonical, "hex");
  const functionCode = decodeManifestVarint(bytes, 0, name);
  const digestLength = decodeManifestVarint(bytes, functionCode.nextIndex, name);
  const payload = bytes.subarray(digestLength.nextIndex);
  if (payload.length !== digestLength.value) {
    fail(
      ValidationErrorCode.INVALID_MULTIHASH,
      `${name} multihash payload length does not match its digest header`,
      name,
    );
  }
  const entry = getCurveEntryByPublicKeyMulticodec(functionCode.value);
  if (!entry) {
    fail(
      ValidationErrorCode.INVALID_MULTIHASH,
      `${name} uses unsupported multihash code 0x${functionCode.value.toString(16)}`,
      name,
    );
  }
  if (
    prefixedAlgorithm &&
    prefixedAlgorithm !== entry.algorithm &&
    !(prefixedAlgorithm === "mldsa" && entry.algorithm === "ml-dsa")
  ) {
    fail(
      ValidationErrorCode.INVALID_MULTIHASH,
      `${name} algorithm prefix does not match the multihash payload`,
      name,
    );
  }
  const fnHex = bytes.subarray(0, functionCode.nextIndex).toString("hex");
  const lenHex = bytes.subarray(functionCode.nextIndex, digestLength.nextIndex).toString("hex");
  const payloadHex = payload.toString("hex").toUpperCase();
  return `${fnHex}${lenHex}${payloadHex}`;
}

function normalizeManifestSignatureLiteral(value, name) {
  if (Buffer.isBuffer(value) || value instanceof Uint8Array) {
    return Buffer.from(value).toString("hex").toUpperCase();
  }
  if (Array.isArray(value)) {
    return normalizeBytesLikeToBuffer(value, name).toString("hex").toUpperCase();
  }
  const literal = assertString(value, name).trim();
  const body =
    literal.includes(":") && literal.indexOf(":") > 0
      ? literal.slice(literal.indexOf(":") + 1)
      : literal;
  if (body.length === 0 || body.length % 2 !== 0 || !/^[0-9A-Fa-f]+$/u.test(body)) {
    fail(
      ValidationErrorCode.INVALID_HEX,
      `${name} must be an even-length hexadecimal string`,
      name,
    );
  }
  return body.toUpperCase();
}

function normalizeManifestProvenance(value, name) {
  const source = assertPlainObject(value, name);
  return {
    signer: normalizeManifestPublicKeyLiteral(source.signer, `${name}.signer`),
    signature: normalizeManifestSignatureLiteral(source.signature, `${name}.signature`),
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
 * @param {{ accountId: string, metadata?: object | null }} options
 * @returns {{Register: {Account: {id: string, label: null, uaid: null, opaque_ids: [], metadata: object}}}}
 */
export function buildRegisterAccountInstruction({
  accountId,
  domainId,
  domain,
  metadata,
}) {
  if (domainId !== undefined || domain !== undefined) {
    throw new TypeError("account registration is domainless; bind account aliases separately");
  }
  const id = normalizeAccountId(accountId, "accountId");
  const normalizedMetadata = normalizeMetadata(metadata);
  return {
    Register: {
      Account: {
        id,
        label: null,
        uaid: null,
        opaque_ids: [],
        metadata: normalizedMetadata,
      },
    },
  };
}

/**
 * Build a `Register::AssetDefinition` instruction payload.
 * @param {{
 *   assetDefinitionId?: string,
 *   id?: string,
 *   name?: string,
 *   description?: string | null,
 *   alias?: string | null,
 *   logo?: string | null,
 *   scale?: number|string|bigint|null,
 *   mintable?: string,
 *   mintOnce?: boolean,
 *   metadata?: object | null,
 *   balanceScopePolicy?: string,
 *   balance_scope_policy?: string,
 *   confidentialPolicy?: object,
 *   confidential_policy?: object
 * }} options
 * @returns {{Register: {AssetDefinition: object}}}
 */
export function buildRegisterAssetDefinitionInstruction(options = {}) {
  const source = assertPlainObject(options, "registerAssetDefinition");
  const scale = source.scale === undefined || source.scale === null
    ? null
    : asU128JsonNumber(source.scale, "registerAssetDefinition.scale");
  const description = source.description === undefined || source.description === null
    ? null
    : assertString(source.description, "registerAssetDefinition.description");
  const alias = source.alias === undefined || source.alias === null
    ? null
    : assertString(source.alias, "registerAssetDefinition.alias");
  const logo = source.logo === undefined || source.logo === null
    ? null
    : assertString(source.logo, "registerAssetDefinition.logo");
  return {
    Register: {
      AssetDefinition: {
        id: assertString(
          source.assetDefinitionId ?? source.asset_definition_id ?? source.id,
          "registerAssetDefinition.assetDefinitionId",
        ),
        name: assertString(source.name ?? "", "registerAssetDefinition.name"),
        description,
        alias,
        spec: { scale },
        mintable: source.mintOnce === true
          ? "Once"
          : assertString(source.mintable ?? "Infinitely", "registerAssetDefinition.mintable"),
        logo,
        metadata: normalizeMetadata(source.metadata),
        balance_scope_policy: assertString(
          source.balanceScopePolicy ?? source.balance_scope_policy ?? "Global",
          "registerAssetDefinition.balanceScopePolicy",
        ),
        confidential_policy: source.confidentialPolicy ?? source.confidential_policy ?? {
          mode: "TransparentOnly",
          vk_set_hash: null,
          poseidon_params_id: null,
          pedersen_params_id: null,
          pending_transition: null,
        },
      },
    },
  };
}

/**
 * Build a `Grant::Permission` instruction payload for an account.
 * @param {{ accountId?: string, destinationAccountId?: string, permission?: object, name?: string, payload?: any }} options
 * @returns {{Grant: {Permission: {object: {name: string, payload: any}, destination: string}}}}
 */
export function buildGrantAccountPermissionInstruction(options = {}) {
  const source = assertPlainObject(options, "grantAccountPermission");
  const permissionSource = source.permission === undefined || source.permission === null
    ? source
    : assertPlainObject(source.permission, "grantAccountPermission.permission");
  return {
    Grant: {
      Permission: {
        object: {
          name: assertString(
            permissionSource.name,
            "grantAccountPermission.permission.name",
          ),
          payload: permissionSource.payload === undefined
            ? null
            : normalizeJsonValue(
                permissionSource.payload,
                "grantAccountPermission.permission.payload",
              ),
        },
        destination: normalizeAccountId(
          source.accountId ?? source.destinationAccountId ?? source.destination,
          "grantAccountPermission.accountId",
        ),
      },
    },
  };
}

/**
 * Build a `SetAssetDefinitionAlias` instruction payload.
 * @param {{ assetDefinitionId?: string, asset_definition_id?: string, alias?: string | null, leaseExpiryMs?: number|string|bigint|null, lease_expiry_ms?: number|string|bigint|null }} options
 * @returns {{SetAssetDefinitionAlias: {asset_definition_id: string, alias: string | null, lease_expiry_ms: number | null}}}
 */
export function buildSetAssetDefinitionAliasInstruction(options = {}) {
  const source = assertPlainObject(options, "setAssetDefinitionAlias");
  const leaseExpiryMs = source.leaseExpiryMs ?? source.lease_expiry_ms;
  return {
    SetAssetDefinitionAlias: {
      asset_definition_id: assertString(
        source.assetDefinitionId ?? source.asset_definition_id,
        "setAssetDefinitionAlias.assetDefinitionId",
      ),
      alias: source.alias === undefined || source.alias === null
        ? null
        : assertString(source.alias, "setAssetDefinitionAlias.alias"),
      lease_expiry_ms: leaseExpiryMs === undefined || leaseExpiryMs === null
        ? null
        : asU128JsonNumber(leaseExpiryMs, "setAssetDefinitionAlias.leaseExpiryMs"),
    },
  };
}

/**
 * Build an `ExecuteTrigger` instruction payload.
 * @param {string | { trigger: string, args?: any }} triggerOrOptions
 * @param {any} [args]
 * @returns {{ExecuteTrigger: {trigger: string, args: any}}}
 */
export function buildExecuteTriggerInstruction(triggerOrOptions, args) {
  const normalized = normalizeExecuteTriggerBuilderInput(
    triggerOrOptions,
    args,
    "executeTrigger",
  );
  return {
    ExecuteTrigger: normalized,
  };
}

/**
 * Encode an `ExecuteTrigger` instruction payload to canonical Norito.
 * @param {string | { trigger: string, args?: any }} triggerOrOptions
 * @param {any} [args]
 * @returns {Buffer}
 */
export function buildExecuteTriggerNorito(triggerOrOptions, args) {
  return noritoEncodeInstruction(buildExecuteTriggerInstruction(triggerOrOptions, args));
}

/**
 * Build common Kotodama trigger-argument payloads for multisig/direct-contract flows.
 * @param {"lifecycle" | "lookup"} preset
 * @param {object} [input]
 * @returns {object}
 */
export function buildMultisigTriggerArgs(preset, input = {}) {
  const normalizedPreset = assertString(preset, "preset");
  const source = assertPlainObject(input, "input");
  if (normalizedPreset === "lifecycle") {
    const payload = {
      action: assertString(source.action, "input.action"),
      request_id: assertString(
        source.requestId ?? source.request_id,
        "input.requestId",
      ),
    };
    const fiId = source.fiId ?? source.fi_id;
    if (fiId !== undefined && fiId !== null) {
      payload.fi_id = assertString(fiId, "input.fiId");
    }
    const toAccountId = source.toAccountId ?? source.to_account_id;
    if (toAccountId !== undefined && toAccountId !== null) {
      payload.to_account_id = normalizeAccountId(toAccountId, "input.toAccountId");
    }
    const amountI64 = source.amountI64 ?? source.amount_i64;
    if (amountI64 !== undefined && amountI64 !== null) {
      payload.amount_i64 = normalizeSafeIntegerJson(amountI64, "input.amountI64", {
        allowNegative: true,
      });
    }
    const requestedByActorId =
      source.requestedByActorId ?? source.requested_by_actor_id;
    if (requestedByActorId !== undefined) {
      payload.requested_by_actor_id = normalizeJsonValue(
        requestedByActorId,
        "input.requestedByActorId",
      );
    }
    const createdAtMs = source.createdAtMs ?? source.created_at_ms;
    if (createdAtMs !== undefined && createdAtMs !== null) {
      payload.created_at_ms = asNonNegativeInteger(createdAtMs, "input.createdAtMs");
    }
    const expiresAtMs = source.expiresAtMs ?? source.expires_at_ms;
    if (expiresAtMs !== undefined && expiresAtMs !== null) {
      payload.expires_at_ms = asNonNegativeInteger(expiresAtMs, "input.expiresAtMs");
    }
    return payload;
  }
  if (normalizedPreset === "lookup") {
    const payload = {
      request_id: assertString(
        source.requestId ?? source.request_id,
        "input.requestId",
      ),
    };
    const requestedByActorId =
      source.requestedByActorId ?? source.requested_by_actor_id;
    if (requestedByActorId !== undefined) {
      payload.requested_by_actor_id = normalizeJsonValue(
        requestedByActorId,
        "input.requestedByActorId",
      );
    }
    return payload;
  }
  fail(
    ValidationErrorCode.INVALID_STRING,
    'preset must be either "lifecycle" or "lookup"',
    "preset",
  );
}

/**
 * Check whether a signer is present in a multisig spec.
 * @param {MultisigSpec | object} spec
 * @param {string} signerAccountId
 * @returns {boolean}
 */
export function isMultisigSignerAuthorized(spec, signerAccountId) {
  const normalizedSpec = normalizeMultisigSpecPayload(spec, "spec");
  const normalizedSigner = normalizeAccountId(signerAccountId, "signerAccountId");
  return Object.prototype.hasOwnProperty.call(
    normalizedSpec.signatories,
    normalizedSigner,
  );
}

/**
 * Build an `ExecuteTrigger` instruction with optional strict signer validation against a multisig spec.
 * @param {{ trigger: string, args?: any, argPreset?: "lifecycle" | "lookup", argInput?: object, signerAccountId?: string, multisigSpec?: MultisigSpec | object, spec?: MultisigSpec | object, strictSignerCheck?: boolean }} options
 * @returns {{ExecuteTrigger: {trigger: string, args: any}}}
 */
export function buildMultisigExecuteTriggerInstruction(options) {
  const normalized = normalizeMultisigExecuteTriggerOptions(
    options,
    "multisigExecuteTrigger",
  );
  return buildExecuteTriggerInstruction(normalized.trigger, normalized.args);
}

/**
 * Encode an `ExecuteTrigger` instruction with optional strict signer validation against a multisig spec.
 * @param {{ trigger: string, args?: any, argPreset?: "lifecycle" | "lookup", argInput?: object, signerAccountId?: string, multisigSpec?: MultisigSpec | object, spec?: MultisigSpec | object, strictSignerCheck?: boolean }} options
 * @returns {Buffer}
 */
export function buildMultisigExecuteTriggerNorito(options) {
  return noritoEncodeInstruction(buildMultisigExecuteTriggerInstruction(options));
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
 * Build a multisig proposal wrapping a single `ExecuteTrigger` instruction.
 * @param {{ accountId: string, trigger: string, args?: any, argPreset?: "lifecycle" | "lookup", argInput?: object, spec: MultisigSpec | object, signerAccountId?: string, strictSignerCheck?: boolean, transactionTtlMs?: number | null }} options
 * @returns {{Custom: {payload: {Propose: {account: string, instructions: object[], transaction_ttl_ms?: number}}}}}
 */
export function buildProposeMultisigExecuteTriggerInstruction(options) {
  const source = assertPlainObject(options, "proposeMultisigExecuteTrigger");
  const normalized = normalizeMultisigExecuteTriggerOptions(
    {
      trigger: source.trigger,
      args: source.args,
      argPreset: source.argPreset ?? source.preset,
      argInput: source.argInput ?? source.presetInput,
      signerAccountId: source.signerAccountId,
      multisigSpec: source.spec,
      strictSignerCheck: source.strictSignerCheck,
    },
    "proposeMultisigExecuteTrigger",
  );
  return buildProposeMultisigInstruction({
    accountId: source.accountId,
    instructions: [buildExecuteTriggerInstruction(normalized.trigger, normalized.args)],
    spec: source.spec,
    transactionTtlMs: source.transactionTtlMs ?? source.transaction_ttl_ms,
  });
}

/**
 * Encode a multisig proposal wrapping a single `ExecuteTrigger` instruction.
 * @param {{ accountId: string, trigger: string, args?: any, argPreset?: "lifecycle" | "lookup", argInput?: object, spec: MultisigSpec | object, signerAccountId?: string, strictSignerCheck?: boolean, transactionTtlMs?: number | null }} options
 * @returns {Buffer}
 */
export function buildProposeMultisigExecuteTriggerNorito(options) {
  return noritoEncodeInstruction(buildProposeMultisigExecuteTriggerInstruction(options));
}

function normalizeMultisigProposeInstructionInput(value, context) {
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (!trimmed) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context} must be a JSON instruction object or native Norito instruction payload input`,
        context,
      );
    }
    return trimmed;
  }
  if (Buffer.isBuffer(value) || ArrayBuffer.isView(value) || value instanceof ArrayBuffer) {
    return value;
  }
  if (Array.isArray(value)) {
    return Buffer.from(normalizeByteArray(value, context));
  }
  return assertPlainObject(value, context);
}

/**
 * Build a normalized payload for `ToriiClient.proposeMultisig(...)`.
 * @param {object} options
 * @returns {object}
 */
export function buildMultisigProposeRequest(options) {
  const source = assertPlainObject(options, "multisigPropose");
  const instructions = source.instructions;
  if (!Array.isArray(instructions) || instructions.length === 0) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "multisigPropose.instructions must be a non-empty array",
      "multisigPropose.instructions",
    );
  }
  const payload = {
    ...normalizeMultisigAccountSelectorInput(source, "multisigPropose"),
    signer_account_id: normalizeAccountId(
      source.signerAccountId ?? source.signer_account_id,
      "multisigPropose.signerAccountId",
    ),
    instructions: instructions.map((instruction, index) =>
      normalizeMultisigProposeInstructionInput(
        instruction,
        `multisigPropose.instructions[${index}]`,
      ),
    ),
  };
  const feeSponsor = source.feeSponsor ?? source.fee_sponsor;
  if (feeSponsor !== undefined && feeSponsor !== null) {
    payload.fee_sponsor = normalizeAccountIdOrAliasLiteral(
      feeSponsor,
      "multisigPropose.feeSponsor",
    );
  }
  const publicKeyHex = source.publicKeyHex ?? source.public_key_hex;
  if (publicKeyHex !== undefined && publicKeyHex !== null) {
    payload.public_key_hex = normalizeOptionalHexString(publicKeyHex, "multisigPropose.publicKeyHex");
  }
  const signatureB64 = source.signatureB64 ?? source.signature_b64;
  if (signatureB64 !== undefined && signatureB64 !== null) {
    payload.signature_b64 = normalizeOptionalBase64String(signatureB64, "multisigPropose.signatureB64");
  }
  const creationTimeMs = source.creationTimeMs ?? source.creation_time_ms;
  if (creationTimeMs !== undefined && creationTimeMs !== null) {
    payload.creation_time_ms = asNonNegativeInteger(creationTimeMs, "multisigPropose.creationTimeMs");
  }
  const privateKey = normalizeDetachedPrivateKeyForMultisigRequest(source, "multisigPropose");
  if (privateKey !== null) {
    payload.private_key = privateKey;
  }
  return payload;
}

/**
 * Build a normalized payload for `ToriiClient.proposeMultisigContractCall(...)`.
 * @param {object} options
 * @returns {object}
 */
export function buildMultisigContractCallProposeRequest(options) {
  const source = assertPlainObject(options, "multisigContractCallPropose");
  const selector = normalizeMultisigAccountSelectorInput(
    source,
    "multisigContractCallPropose",
  );
  const normalized = normalizeMultisigExecuteTriggerOptions(
    {
      trigger: source.trigger,
      args: source.args,
      argPreset: source.argPreset ?? source.preset,
      argInput: source.argInput ?? source.presetInput,
      signerAccountId: source.signerAccountId,
      multisigSpec: source.multisigSpec ?? source.spec,
      strictSignerCheck: source.strictSignerCheck,
    },
    "multisigContractCallPropose",
  );
  const payload = {
    ...selector,
    signer_account_id: normalized.signerAccountId ?? normalizeAccountId(
      source.signerAccountId,
      "multisigContractCallPropose.signerAccountId",
    ),
    ...normalizeContractTargetSelectorInput(source, "multisigContractCallPropose"),
    entrypoint: assertString(
      source.entrypoint,
      "multisigContractCallPropose.entrypoint",
    ),
    payload:
      source.payload !== undefined
        ? normalizeJsonValue(source.payload, "multisigContractCallPropose.payload")
        : {
            trigger: normalized.trigger,
            args: normalized.args,
          },
  };

  const gasAssetId = source.gasAssetId ?? source.gas_asset_id;
  if (gasAssetId !== undefined && gasAssetId !== null) {
    payload.gas_asset_id = normalizeAssetId(
      gasAssetId,
      "multisigContractCallPropose.gasAssetId",
    );
  }
  const feeSponsor = source.feeSponsor ?? source.fee_sponsor;
  if (feeSponsor !== undefined && feeSponsor !== null) {
    payload.fee_sponsor = normalizeAccountIdOrAliasLiteral(
      feeSponsor,
      "multisigContractCallPropose.feeSponsor",
    );
  }
  const gasLimit = source.gasLimit ?? source.gas_limit;
  if (gasLimit !== undefined && gasLimit !== null) {
    payload.gas_limit = asPositiveInteger(gasLimit, "multisigContractCallPropose.gasLimit");
  }
  const publicKeyHex = source.publicKeyHex ?? source.public_key_hex;
  if (publicKeyHex !== undefined && publicKeyHex !== null) {
    payload.public_key_hex = normalizeOptionalHexString(
      publicKeyHex,
      "multisigContractCallPropose.publicKeyHex",
    );
  }
  const signatureB64 = source.signatureB64 ?? source.signature_b64;
  if (signatureB64 !== undefined && signatureB64 !== null) {
    payload.signature_b64 = normalizeOptionalBase64String(
      signatureB64,
      "multisigContractCallPropose.signatureB64",
    );
  }
  const creationTimeMs = source.creationTimeMs ?? source.creation_time_ms;
  if (creationTimeMs !== undefined && creationTimeMs !== null) {
    payload.creation_time_ms = asNonNegativeInteger(
      creationTimeMs,
      "multisigContractCallPropose.creationTimeMs",
    );
  }
  const privateKey = normalizeDetachedPrivateKeyForMultisigRequest(
    source,
    "multisigContractCallPropose",
  );
  if (privateKey !== null) {
    payload.private_key = privateKey;
  }
  return payload;
}

function normalizeContractTargetSelectorInput(source, context) {
  const contractAddress = source.contractAddress ?? source.contract_address;
  const contractAlias = source.contractAlias ?? source.contract_alias;
  const hasContractAddress = contractAddress !== undefined && contractAddress !== null;
  const hasContractAlias = contractAlias !== undefined && contractAlias !== null;
  if (hasContractAddress === hasContractAlias) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} requires exactly one of contractAddress or contractAlias`,
      context,
    );
  }
  if (hasContractAddress) {
    return {
      contract_address: assertString(
        contractAddress,
        `${context}.contractAddress`,
      ),
    };
  }
  return {
    contract_alias: assertString(
      contractAlias,
      `${context}.contractAlias`,
    ),
  };
}

/**
 * Build a normalized payload for `ToriiClient.approveMultisigContractCall(...)`.
 * @param {object} options
 * @returns {object}
 */
export function buildMultisigContractCallApproveRequest(options) {
  const source = assertPlainObject(options, "multisigContractCallApprove");
  const selector = normalizeMultisigAccountSelectorInput(
    source,
    "multisigContractCallApprove",
  );
  const payload = {
    ...selector,
    signer_account_id: normalizeAccountId(
      source.signerAccountId ?? source.signer_account_id,
      "multisigContractCallApprove.signerAccountId",
    ),
  };
  const proposalId = source.proposalId ?? source.proposal_id;
  if (proposalId !== undefined && proposalId !== null) {
    payload.proposal_id = assertString(
      proposalId,
      "multisigContractCallApprove.proposalId",
    );
  }
  const instructionsHash = source.instructionsHash ?? source.instructions_hash;
  if (instructionsHash !== undefined && instructionsHash !== null) {
    payload.instructions_hash = normalizeOptionalHexString(
      instructionsHash,
      "multisigContractCallApprove.instructionsHash",
    );
  }
  if (!payload.proposal_id && !payload.instructions_hash) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "multisigContractCallApprove requires proposalId or instructionsHash",
      "multisigContractCallApprove",
    );
  }
  const publicKeyHex = source.publicKeyHex ?? source.public_key_hex;
  if (publicKeyHex !== undefined && publicKeyHex !== null) {
    payload.public_key_hex = normalizeOptionalHexString(
      publicKeyHex,
      "multisigContractCallApprove.publicKeyHex",
    );
  }
  const signatureB64 = source.signatureB64 ?? source.signature_b64;
  if (signatureB64 !== undefined && signatureB64 !== null) {
    payload.signature_b64 = normalizeOptionalBase64String(
      signatureB64,
      "multisigContractCallApprove.signatureB64",
    );
  }
  const creationTimeMs = source.creationTimeMs ?? source.creation_time_ms;
  if (creationTimeMs !== undefined && creationTimeMs !== null) {
    payload.creation_time_ms = asNonNegativeInteger(
      creationTimeMs,
      "multisigContractCallApprove.creationTimeMs",
    );
  }
  const privateKey = normalizeDetachedPrivateKeyForMultisigRequest(
    source,
    "multisigContractCallApprove",
  );
  if (privateKey !== null) {
    payload.private_key = privateKey;
  }
  return payload;
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
    ...normalizeContractTargetSelectorInput(source, "proposeDeployContract"),
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
  const normalizedDerived = String(derivedBy).trim();
  if (normalizedDerived.toLowerCase() !== "vrf") {
    throw new TypeError("persistCouncilForEpoch.derivedBy must be Vrf");
  }
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
      derived_by: "Vrf",
    },
  };
}

/**
 * Build a `SubmitAgendaProposal` instruction payload.
 * @param {{ proposal: object }} options
 * @returns {{SubmitAgendaProposal: { proposal: object }}}
 */
export function buildSubmitAgendaProposalInstruction(options) {
  const source = assertPlainObject(options, "submitAgendaProposal");
  const proposal = assertPlainObject(
    source.proposal,
    "submitAgendaProposal.proposal",
  );
  return {
    SubmitAgendaProposal: {
      proposal,
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
 * Normalize a zkAt policy commitment descriptor.
 *
 * Supplying `policyJson`/`policyBytes` derives a deterministic dev commitment
 * for SDK fixtures. Supplying only `policyCommitment` preserves an externally
 * generated commitment without claiming production policy proving support.
 * @param {object} options
 * @returns {object}
 */
export function buildZkAtPolicyCommitment(options, context = "zkAtPolicyCommitment") {
  const source = assertPlainObject(options, context);
  assertAllowedFields(
    source,
    new Set([
      "version",
      "policyCommitment",
      "policy_commitment",
      "commitment",
      "policy",
      "policyBytes",
      "policy_bytes",
      "policyJson",
      "policy_json",
      "policyEpoch",
      "policy_epoch",
      "domainSeparator",
      "domain_separator",
      "policySchema",
      "policy_schema",
      "maxPolicyBytes",
      "max_policy_bytes",
    ]),
    context,
  );
  const commitmentAlias = readSingleAlias(
    source,
    ["policyCommitment", "policy_commitment", "commitment"],
    `${context}.policyCommitment`,
    "policy commitment",
  );
  const policyAlias = readSingleAlias(
    source,
    ["policy", "policyBytes", "policy_bytes", "policyJson", "policy_json"],
    `${context}.policy`,
    "policy",
  );
  const epochAlias = readSingleAlias(
    source,
    ["policyEpoch", "policy_epoch"],
    `${context}.policyEpoch`,
    "policy epoch",
  );
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  const schemaAlias = readSingleAlias(
    source,
    ["policySchema", "policy_schema"],
    `${context}.policySchema`,
    "policy schema",
  );
  const policyEpoch = normalizeZkAtPolicyEpoch(
    epochAlias.value,
    `${context}.policyEpoch`,
  );
  const domainSeparator = assertNonBlankString(
    domainAlias.value ?? ZKAT_DOMAIN_SEPARATOR,
    `${context}.domainSeparator`,
  );
  const policySchema = assertNonBlankString(
    schemaAlias.value ?? "zkat-policy-json-v1",
    `${context}.policySchema`,
  );
  const explicitCommitment =
    commitmentAlias.value === undefined
      ? null
      : normalizeNonZeroFixedBytes(
          commitmentAlias.value,
          `${context}.policyCommitment`,
          32,
        );
  const policyBytes =
    policyAlias.value === undefined
      ? null
      : normalizeZkAtPolicyBytes(
          policyAlias.value,
          policyAlias.key,
          context,
          normalizePositiveU32AliasOrDefault(
            source,
            ["maxPolicyBytes", "max_policy_bytes"],
            `${context}.maxPolicyBytes`,
            "max policy byte limit",
            ZKAT_MAX_POLICY_BYTES,
          ),
        );
  if (explicitCommitment === null && policyBytes === null) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.policyCommitment or ${context}.policy is required`,
      `${context}.policyCommitment`,
    );
  }
  const derivedCommitment =
    policyBytes === null
      ? null
      : zkatPolicyCommitmentBytes({
          policyBytes,
          policyEpoch,
          domainSeparator,
          policySchema,
        });
  if (
    explicitCommitment !== null &&
    derivedCommitment !== null &&
    !fixedBytesEqual(explicitCommitment, derivedCommitment)
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.policyCommitment must match the derived policy commitment`,
      `${context}.policyCommitment`,
    );
  }
  const policyCommitment = explicitCommitment ?? derivedCommitment;
  return {
    version: normalizeZkAtVersion(source.version, `${context}.version`),
    policy_commitment: policyCommitment,
    policy_epoch: policyEpoch,
    domain_separator: domainSeparator,
    policy_schema: policySchema,
    commitment_kind: policyBytes === null ? "external" : "dev-sha256-policy-digest",
    policy_digest:
      policyBytes === null
        ? null
        : Array.from(createHash("sha256").update(Buffer.from(policyBytes)).digest().values()),
  };
}

/**
 * Build canonical OpenVerifyEnvelope bytes for a prepared zkAt authenticator.
 *
 * This accepts externally generated proof bytes and binds them to normalized
 * policy-private authorization public inputs.
 * @param {object} options
 * @returns {Buffer}
 */
export function buildZkAtAuthenticatorEnvelope(options) {
  const source = assertPlainObject(options, "zkAtAuthenticatorEnvelope");
  assertAllowedFields(
    source,
    new Set([
      "backend",
      "backendTag",
      "backend_tag",
      "circuitId",
      "circuit_id",
      "vkHash",
      "vk_hash",
      "verifierKeyHash",
      "verifyingKeyHash",
      "policyCommitment",
      "policy_commitment",
      "commitment",
      "policy",
      "policyBytes",
      "policy_bytes",
      "policyJson",
      "policy_json",
      "policyEpoch",
      "policy_epoch",
      "policySchema",
      "policy_schema",
      "txDigest",
      "tx_digest",
      "payloadDigest",
      "payload_digest",
      "payload",
      "payloadBytes",
      "payload_bytes",
      "payloadJson",
      "payload_json",
      "accountId",
      "account_id",
      "actionClass",
      "action_class",
      "domainSeparator",
      "domain_separator",
      "proofBytes",
      "proof_bytes",
      "proof",
      "aux",
      "maxProofBytes",
      "max_proof_bytes",
      "maxPublicInputBytes",
      "max_public_input_bytes",
      "maxPayloadBytes",
      "max_payload_bytes",
      "maxPolicyBytes",
      "max_policy_bytes",
      "version",
    ]),
    "zkAtAuthenticatorEnvelope",
  );
  const parts = normalizeZkAtAuthenticatorParts(
    source,
    "zkAtAuthenticatorEnvelope",
  );
  return buildPrivacyProofEnvelopeWithOptionalLimits({
    backend: parts.backend,
    circuitId: parts.circuitId,
    vkHash: parts.vkHash,
    publicInputs: parts.publicInputBytes,
    proofBytes: parts.proofBytes,
    aux: source.aux,
    maxProofBytes: parts.maxProofBytes,
    maxPublicInputBytes: parts.maxPublicInputBytes,
  });
}

/**
 * Build a deterministic zkAt dev proof fixture bound to an OpenVerify envelope.
 *
 * This fixture is only for SDK/dev-verifier public-input binding tests. It is
 * not a production zkAt proof.
 * @param {object} options
 * @returns {object}
 */
export function buildZkAtDevProofFixture(options) {
  const source = assertPlainObject(options, "zkAtDevProofFixture");
  assertAllowedFields(
    source,
    new Set([
      "backend",
      "backendTag",
      "backend_tag",
      "circuitId",
      "circuit_id",
      "vkHash",
      "vk_hash",
      "verifierKeyHash",
      "verifyingKeyHash",
      "policyCommitment",
      "policy_commitment",
      "commitment",
      "policy",
      "policyBytes",
      "policy_bytes",
      "policyJson",
      "policy_json",
      "policyEpoch",
      "policy_epoch",
      "policySchema",
      "policy_schema",
      "txDigest",
      "tx_digest",
      "payloadDigest",
      "payload_digest",
      "payload",
      "payloadBytes",
      "payload_bytes",
      "payloadJson",
      "payload_json",
      "accountId",
      "account_id",
      "actionClass",
      "action_class",
      "domainSeparator",
      "domain_separator",
      "aux",
      "maxProofBytes",
      "max_proof_bytes",
      "maxPublicInputBytes",
      "max_public_input_bytes",
      "maxPayloadBytes",
      "max_payload_bytes",
      "maxPolicyBytes",
      "max_policy_bytes",
      "version",
    ]),
    "zkAtDevProofFixture",
  );
  const parts = normalizeZkAtAuthenticatorParts(
    source,
    "zkAtDevProofFixture",
    { requireProofBytes: false },
  );
  const proofBytes = zkatDevProofBytes(parts);
  const envelope = buildPrivacyProofEnvelopeWithOptionalLimits({
    backend: parts.backend,
    circuitId: parts.circuitId,
    vkHash: parts.vkHash,
    publicInputs: parts.publicInputBytes,
    proofBytes,
    aux: source.aux,
    maxProofBytes: parts.maxProofBytes,
    maxPublicInputBytes: parts.maxPublicInputBytes,
  });
  return {
    kind: "zkat-dev-fixture-v1",
    production: false,
    proof_bytes: proofBytes,
    proofBytes: Buffer.from(proofBytes),
    public_inputs: parts.publicInputs,
    publicInputBytes: Buffer.from(parts.publicInputBytes),
    envelope,
  };
}

/**
 * Verify a deterministic zkAt dev proof fixture through OpenVerifyEnvelope bytes.
 *
 * This verifier only accepts the SDK dev-fixture format and must not be used as
 * production zkAt cryptography.
 * @param {object|Buffer|string} options
 * @returns {object}
 */
export function verifyZkAtAuthenticatorLocally(options) {
  const source =
    options &&
    typeof options === "object" &&
    !Array.isArray(options) &&
    !Buffer.isBuffer(options) &&
    !ArrayBuffer.isView(options) &&
    !(options instanceof ArrayBuffer)
      ? assertPlainObject(options, "zkAtAuthenticatorLocalVerification")
      : { envelope: options };
  assertAllowedFields(
    source,
    new Set([
      "envelope",
      "proofEnvelope",
      "proof_envelope",
      "bytes",
      "policyCommitment",
      "policy_commitment",
      "commitment",
      "policy",
      "policyBytes",
      "policy_bytes",
      "policyJson",
      "policy_json",
      "policyEpoch",
      "policy_epoch",
      "policySchema",
      "policy_schema",
      "txDigest",
      "tx_digest",
      "payloadDigest",
      "payload_digest",
      "payload",
      "payloadBytes",
      "payload_bytes",
      "payloadJson",
      "payload_json",
      "accountId",
      "account_id",
      "actionClass",
      "action_class",
      "domainSeparator",
      "domain_separator",
      "maxPayloadBytes",
      "max_payload_bytes",
      "maxPolicyBytes",
      "max_policy_bytes",
    ]),
    "zkAtAuthenticatorLocalVerification",
  );
  const envelopeAlias = readSingleAlias(
    source,
    ["envelope", "proofEnvelope", "proof_envelope", "bytes"],
    "zkAtAuthenticatorLocalVerification.envelope",
    "proof envelope",
  );
  const decoded = decodeVeRangeEnvelope(
    envelopeAlias.value,
    "zkAtAuthenticatorLocalVerification.envelope",
  );
  if (decoded.backend !== "Stark") {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "zkAtAuthenticatorLocalVerification.envelope.backend must be Stark",
      "zkAtAuthenticatorLocalVerification.envelope.backend",
    );
  }
  const circuitId = normalizeZkAtCircuitId(
    decoded.circuit_id,
    "zkAtAuthenticatorLocalVerification.envelope.circuitId",
  );
  const vkHash = normalizeNonZeroFixedBytes(
    decoded.vk_hash,
    "zkAtAuthenticatorLocalVerification.envelope.vkHash",
    32,
  );
  const publicInputs = parseZkAtPublicInputs(
    decoded.public_inputs,
    "zkAtAuthenticatorLocalVerification.publicInputs",
  );
  ensureZkAtVerificationExpectations(
    source,
    publicInputs,
    "zkAtAuthenticatorLocalVerification",
  );
  const expectedProofBytes = zkatDevProofBytes({
    circuitId,
    vkHash,
    publicInputBytes: decoded.public_inputs,
  });
  if (!fixedBytesEqual(decoded.proof_bytes, expectedProofBytes)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "zkAtAuthenticatorLocalVerification proof bytes are not a valid zkAt dev fixture",
      "zkAtAuthenticatorLocalVerification.proofBytes",
    );
  }
  return {
    ok: true,
    production: false,
    kind: "zkat-dev-fixture-v1",
    backend: ZKAT_BACKEND,
    circuit_id: circuitId,
    verifier_key_hash: Buffer.from(vkHash).toString("hex"),
    public_inputs: publicInputs,
    public_input_bytes: decoded.public_inputs.length,
    proof_bytes: decoded.proof_bytes.length,
    aux_bytes: decoded.aux.length,
    account_id: publicInputs.account_id,
    action_class: publicInputs.action_class,
    policy_epoch: publicInputs.policy_epoch,
  };
}

/**
 * Normalize a ZK-AMS recursive anonymous admission batch and derive its root.
 *
 * The batch root binds issuer root, admission nullifiers, anonymous account
 * commitments, recursive proof digest, and domain separator. It does not submit
 * chain state or claim production recursive proving support.
 * @param {object} options
 * @returns {object}
 */
export function buildZkAmsAdmissionBatch(options) {
  const source = assertPlainObject(options, "zkAmsAdmissionBatch");
  assertAllowedFields(
    source,
    new Set([
      "version",
      "issuerRoot",
      "issuer_root",
      "admissionBatchRoot",
      "admission_batch_root",
      "batchRoot",
      "batch_root",
      "admissionNullifiers",
      "admission_nullifiers",
      "nullifiers",
      "anonymousAccountCommitments",
      "anonymous_account_commitments",
      "accountCommitments",
      "account_commitments",
      "recursiveProofDigest",
      "recursive_proof_digest",
      "recursiveProof",
      "recursiveProofBytes",
      "recursive_proof",
      "recursive_proof_bytes",
      "domainSeparator",
      "domain_separator",
      "maxBatchSize",
      "max_batch_size",
      "maxRecursiveProofBytes",
      "max_recursive_proof_bytes",
    ]),
    "zkAmsAdmissionBatch",
  );
  const batch = normalizeZkAmsAdmissionBatchParts(source, "zkAmsAdmissionBatch");
  return {
    version: batch.version,
    issuer_root: batch.issuerRoot,
    admission_batch_root: batch.admissionBatchRoot,
    admission_nullifiers: batch.admissionNullifiers,
    anonymous_account_commitments: batch.anonymousAccountCommitments,
    recursive_proof_digest: batch.recursiveProofDigest,
    domain_separator: batch.domainSeparator,
    batch_size: batch.batchSize,
    root_kind: "dev-sha256-admission-batch-root",
  };
}

/**
 * Build canonical OpenVerifyEnvelope bytes for a prepared ZK-AMS admission proof.
 *
 * This accepts externally generated proof bytes and binds them to normalized
 * recursive admission public inputs.
 * @param {object} options
 * @returns {Buffer}
 */
export function buildZkAmsAdmissionProofEnvelope(options) {
  const source = assertPlainObject(options, "zkAmsAdmissionProofEnvelope");
  assertAllowedFields(
    source,
    new Set([
      "version",
      "backend",
      "backendTag",
      "backend_tag",
      "circuitId",
      "circuit_id",
      "vkHash",
      "vk_hash",
      "verifierKeyHash",
      "verifyingKeyHash",
      "issuerRoot",
      "issuer_root",
      "admissionBatchRoot",
      "admission_batch_root",
      "batchRoot",
      "batch_root",
      "admissionNullifiers",
      "admission_nullifiers",
      "nullifiers",
      "anonymousAccountCommitments",
      "anonymous_account_commitments",
      "accountCommitments",
      "account_commitments",
      "recursiveProofDigest",
      "recursive_proof_digest",
      "recursiveProof",
      "recursiveProofBytes",
      "recursive_proof",
      "recursive_proof_bytes",
      "domainSeparator",
      "domain_separator",
      "proofBytes",
      "proof_bytes",
      "proof",
      "aux",
      "maxBatchSize",
      "max_batch_size",
      "maxRecursiveProofBytes",
      "max_recursive_proof_bytes",
      "maxProofBytes",
      "max_proof_bytes",
      "maxPublicInputBytes",
      "max_public_input_bytes",
    ]),
    "zkAmsAdmissionProofEnvelope",
  );
  const parts = normalizeZkAmsAdmissionProofParts(
    source,
    "zkAmsAdmissionProofEnvelope",
  );
  return buildPrivacyProofEnvelopeWithOptionalLimits({
    backend: parts.backend,
    circuitId: parts.circuitId,
    vkHash: parts.vkHash,
    publicInputs: parts.publicInputBytes,
    proofBytes: parts.proofBytes,
    aux: source.aux,
    maxProofBytes: parts.maxProofBytes,
    maxPublicInputBytes: parts.maxPublicInputBytes,
  });
}

/**
 * Build a deterministic ZK-AMS dev proof fixture bound to an OpenVerify envelope.
 *
 * This fixture is only for SDK/dev-verifier public-input binding tests. It is
 * not a production recursive admission proof.
 * @param {object} options
 * @returns {object}
 */
export function buildZkAmsAdmissionDevProofFixture(options) {
  const source = assertPlainObject(options, "zkAmsAdmissionDevProofFixture");
  assertAllowedFields(
    source,
    new Set([
      "version",
      "backend",
      "backendTag",
      "backend_tag",
      "circuitId",
      "circuit_id",
      "vkHash",
      "vk_hash",
      "verifierKeyHash",
      "verifyingKeyHash",
      "issuerRoot",
      "issuer_root",
      "admissionBatchRoot",
      "admission_batch_root",
      "batchRoot",
      "batch_root",
      "admissionNullifiers",
      "admission_nullifiers",
      "nullifiers",
      "anonymousAccountCommitments",
      "anonymous_account_commitments",
      "accountCommitments",
      "account_commitments",
      "recursiveProofDigest",
      "recursive_proof_digest",
      "recursiveProof",
      "recursiveProofBytes",
      "recursive_proof",
      "recursive_proof_bytes",
      "domainSeparator",
      "domain_separator",
      "aux",
      "maxBatchSize",
      "max_batch_size",
      "maxRecursiveProofBytes",
      "max_recursive_proof_bytes",
      "maxProofBytes",
      "max_proof_bytes",
      "maxPublicInputBytes",
      "max_public_input_bytes",
    ]),
    "zkAmsAdmissionDevProofFixture",
  );
  const parts = normalizeZkAmsAdmissionProofParts(
    source,
    "zkAmsAdmissionDevProofFixture",
    { requireProofBytes: false },
  );
  const proofBytes = zkamDevProofBytes(parts);
  const envelope = buildPrivacyProofEnvelopeWithOptionalLimits({
    backend: parts.backend,
    circuitId: parts.circuitId,
    vkHash: parts.vkHash,
    publicInputs: parts.publicInputBytes,
    proofBytes,
    aux: source.aux,
    maxProofBytes: parts.maxProofBytes,
    maxPublicInputBytes: parts.maxPublicInputBytes,
  });
  return {
    kind: "zk-ams-dev-fixture-v0",
    production: false,
    proof_bytes: proofBytes,
    proofBytes: Buffer.from(proofBytes),
    batch: {
      version: parts.batch.version,
      issuer_root: parts.batch.issuerRoot,
      admission_batch_root: parts.batch.admissionBatchRoot,
      admission_nullifiers: parts.batch.admissionNullifiers,
      anonymous_account_commitments: parts.batch.anonymousAccountCommitments,
      recursive_proof_digest: parts.batch.recursiveProofDigest,
      domain_separator: parts.batch.domainSeparator,
      batch_size: parts.batch.batchSize,
    },
    public_inputs: parts.publicInputs,
    publicInputBytes: Buffer.from(parts.publicInputBytes),
    envelope,
  };
}

/**
 * Verify a deterministic ZK-AMS dev proof fixture through OpenVerifyEnvelope bytes.
 *
 * This verifier only accepts the SDK dev-fixture format and must not be used as
 * production recursive admission cryptography.
 * @param {object|Buffer|string} options
 * @returns {object}
 */
export function verifyZkAmsAdmissionProofLocally(options) {
  const source =
    options &&
    typeof options === "object" &&
    !Array.isArray(options) &&
    !Buffer.isBuffer(options) &&
    !ArrayBuffer.isView(options) &&
    !(options instanceof ArrayBuffer)
      ? assertPlainObject(options, "zkAmsAdmissionLocalVerification")
      : { envelope: options };
  assertAllowedFields(
    source,
    new Set([
      "envelope",
      "proofEnvelope",
      "proof_envelope",
      "bytes",
      "issuerRoot",
      "issuer_root",
      "admissionBatchRoot",
      "admission_batch_root",
      "batchRoot",
      "batch_root",
      "admissionNullifiers",
      "admission_nullifiers",
      "nullifiers",
      "anonymousAccountCommitments",
      "anonymous_account_commitments",
      "accountCommitments",
      "account_commitments",
      "recursiveProofDigest",
      "recursive_proof_digest",
      "recursiveProof",
      "recursiveProofBytes",
      "recursive_proof",
      "recursive_proof_bytes",
      "domainSeparator",
      "domain_separator",
      "maxBatchSize",
      "max_batch_size",
      "maxRecursiveProofBytes",
      "max_recursive_proof_bytes",
    ]),
    "zkAmsAdmissionLocalVerification",
  );
  const envelopeAlias = readSingleAlias(
    source,
    ["envelope", "proofEnvelope", "proof_envelope", "bytes"],
    "zkAmsAdmissionLocalVerification.envelope",
    "proof envelope",
  );
  const decoded = decodeVeRangeEnvelope(
    envelopeAlias.value,
    "zkAmsAdmissionLocalVerification.envelope",
  );
  if (decoded.backend !== "Stark") {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "zkAmsAdmissionLocalVerification.envelope.backend must be Stark",
      "zkAmsAdmissionLocalVerification.envelope.backend",
    );
  }
  const circuitId = normalizeZkAmsCircuitId(
    decoded.circuit_id,
    "zkAmsAdmissionLocalVerification.envelope.circuitId",
  );
  const vkHash = normalizeNonZeroFixedBytes(
    decoded.vk_hash,
    "zkAmsAdmissionLocalVerification.envelope.vkHash",
    32,
  );
  const publicInputs = parseZkAmsPublicInputs(
    decoded.public_inputs,
    "zkAmsAdmissionLocalVerification.publicInputs",
  );
  ensureZkAmsVerificationExpectations(
    source,
    publicInputs,
    "zkAmsAdmissionLocalVerification",
  );
  const expectedProofBytes = zkamDevProofBytes({
    circuitId,
    vkHash,
    publicInputBytes: decoded.public_inputs,
  });
  if (!fixedBytesEqual(decoded.proof_bytes, expectedProofBytes)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "zkAmsAdmissionLocalVerification proof bytes are not a valid ZK-AMS dev fixture",
      "zkAmsAdmissionLocalVerification.proofBytes",
    );
  }
  return {
    ok: true,
    production: false,
    kind: "zk-ams-dev-fixture-v0",
    backend: ZK_AMS_BACKEND,
    circuit_id: circuitId,
    verifier_key_hash: Buffer.from(vkHash).toString("hex"),
    public_inputs: publicInputs,
    public_input_bytes: decoded.public_inputs.length,
    proof_bytes: decoded.proof_bytes.length,
    aux_bytes: decoded.aux.length,
    admission_batch_root: publicInputs.admission_batch_root,
    batch_size: publicInputs.admission_nullifiers.length,
  };
}

/**
 * Normalize a Vega credential predicate commitment descriptor.
 *
 * Supplying `predicateJson`/`predicateBytes` derives a deterministic dev
 * commitment for SDK fixtures. Supplying only `predicateCommitment` preserves an
 * externally generated commitment without claiming production Vega proving
 * support.
 * @param {object} options
 * @returns {object}
 */
export function buildVegaCredentialPredicateCommitment(
  options,
  context = "vegaCredentialPredicateCommitment",
) {
  const source = assertPlainObject(options, context);
  assertAllowedFields(
    source,
    new Set([
      "version",
      "predicateCommitment",
      "predicate_commitment",
      "commitment",
      "predicate",
      "predicateBytes",
      "predicate_bytes",
      "predicateJson",
      "predicate_json",
      "credentialSchema",
      "credential_schema",
      "domainSeparator",
      "domain_separator",
      "maxPredicateBytes",
      "max_predicate_bytes",
    ]),
    context,
  );
  return normalizeVegaPredicateCommitmentFromSource(source, context);
}

/**
 * Build canonical OpenVerifyEnvelope bytes for a prepared Vega credential proof.
 *
 * This accepts externally generated proof bytes and binds them to normalized
 * existing-credential public inputs.
 * @param {object} options
 * @returns {Buffer}
 */
export function buildVegaCredentialProofEnvelope(options) {
  const source = assertPlainObject(options, "vegaCredentialProofEnvelope");
  assertAllowedFields(
    source,
    new Set([
      "version",
      "backend",
      "backendTag",
      "backend_tag",
      "circuitId",
      "circuit_id",
      "vkHash",
      "vk_hash",
      "verifierKeyHash",
      "verifyingKeyHash",
      "issuerCommitment",
      "issuer_commitment",
      "issuer",
      "issuerBytes",
      "issuer_bytes",
      "issuerJson",
      "issuer_json",
      "predicateCommitment",
      "predicate_commitment",
      "commitment",
      "predicate",
      "predicateBytes",
      "predicate_bytes",
      "predicateJson",
      "predicate_json",
      "credentialSchema",
      "credential_schema",
      "subjectBinding",
      "subject_binding",
      "identityCommitment",
      "identity_commitment",
      "accountCommitment",
      "account_commitment",
      "accountId",
      "account_id",
      "subjectAccountId",
      "subject_account_id",
      "expirationEpoch",
      "expiration_epoch",
      "domainSeparator",
      "domain_separator",
      "proofBytes",
      "proof_bytes",
      "proof",
      "aux",
      "maxIssuerBytes",
      "max_issuer_bytes",
      "maxPredicateBytes",
      "max_predicate_bytes",
      "maxProofBytes",
      "max_proof_bytes",
      "maxPublicInputBytes",
      "max_public_input_bytes",
    ]),
    "vegaCredentialProofEnvelope",
  );
  const parts = normalizeVegaProofParts(source, "vegaCredentialProofEnvelope");
  return buildPrivacyProofEnvelopeWithOptionalLimits({
    backend: parts.backend,
    circuitId: parts.circuitId,
    vkHash: parts.vkHash,
    publicInputs: parts.publicInputBytes,
    proofBytes: parts.proofBytes,
    aux: source.aux,
    maxProofBytes: parts.maxProofBytes,
    maxPublicInputBytes: parts.maxPublicInputBytes,
  });
}

/**
 * Build a deterministic Vega dev proof fixture bound to an OpenVerify envelope.
 *
 * This fixture is only for SDK/dev-verifier public-input binding tests. It is
 * not a production existing-credential proof.
 * @param {object} options
 * @returns {object}
 */
export function buildVegaCredentialDevProofFixture(options) {
  const source = assertPlainObject(options, "vegaCredentialDevProofFixture");
  assertAllowedFields(
    source,
    new Set([
      "version",
      "backend",
      "backendTag",
      "backend_tag",
      "circuitId",
      "circuit_id",
      "vkHash",
      "vk_hash",
      "verifierKeyHash",
      "verifyingKeyHash",
      "issuerCommitment",
      "issuer_commitment",
      "issuer",
      "issuerBytes",
      "issuer_bytes",
      "issuerJson",
      "issuer_json",
      "predicateCommitment",
      "predicate_commitment",
      "commitment",
      "predicate",
      "predicateBytes",
      "predicate_bytes",
      "predicateJson",
      "predicate_json",
      "credentialSchema",
      "credential_schema",
      "subjectBinding",
      "subject_binding",
      "identityCommitment",
      "identity_commitment",
      "accountCommitment",
      "account_commitment",
      "accountId",
      "account_id",
      "subjectAccountId",
      "subject_account_id",
      "expirationEpoch",
      "expiration_epoch",
      "domainSeparator",
      "domain_separator",
      "aux",
      "maxIssuerBytes",
      "max_issuer_bytes",
      "maxPredicateBytes",
      "max_predicate_bytes",
      "maxProofBytes",
      "max_proof_bytes",
      "maxPublicInputBytes",
      "max_public_input_bytes",
    ]),
    "vegaCredentialDevProofFixture",
  );
  const parts = normalizeVegaProofParts(
    source,
    "vegaCredentialDevProofFixture",
    { requireProofBytes: false },
  );
  const proofBytes = vegaDevProofBytes(parts);
  const envelope = buildPrivacyProofEnvelopeWithOptionalLimits({
    backend: parts.backend,
    circuitId: parts.circuitId,
    vkHash: parts.vkHash,
    publicInputs: parts.publicInputBytes,
    proofBytes,
    aux: source.aux,
    maxProofBytes: parts.maxProofBytes,
    maxPublicInputBytes: parts.maxPublicInputBytes,
  });
  return {
    kind: "vega-dev-fixture-v0",
    production: false,
    proof_bytes: proofBytes,
    proofBytes: Buffer.from(proofBytes),
    public_inputs: parts.publicInputs,
    publicInputBytes: Buffer.from(parts.publicInputBytes),
    envelope,
  };
}

/**
 * Verify a deterministic Vega dev proof fixture through OpenVerifyEnvelope bytes.
 *
 * This verifier only accepts the SDK dev-fixture format and must not be used as
 * production Vega credential cryptography.
 * @param {object|Buffer|string} options
 * @returns {object}
 */
export function verifyVegaCredentialProofLocally(options) {
  const source =
    options &&
    typeof options === "object" &&
    !Array.isArray(options) &&
    !Buffer.isBuffer(options) &&
    !ArrayBuffer.isView(options) &&
    !(options instanceof ArrayBuffer)
      ? assertPlainObject(options, "vegaCredentialLocalVerification")
      : { envelope: options };
  assertAllowedFields(
    source,
    new Set([
      "envelope",
      "proofEnvelope",
      "proof_envelope",
      "bytes",
      "issuerCommitment",
      "issuer_commitment",
      "issuer",
      "issuerBytes",
      "issuer_bytes",
      "issuerJson",
      "issuer_json",
      "predicateCommitment",
      "predicate_commitment",
      "commitment",
      "predicate",
      "predicateBytes",
      "predicate_bytes",
      "predicateJson",
      "predicate_json",
      "credentialSchema",
      "credential_schema",
      "subjectBinding",
      "subject_binding",
      "identityCommitment",
      "identity_commitment",
      "accountCommitment",
      "account_commitment",
      "accountId",
      "account_id",
      "subjectAccountId",
      "subject_account_id",
      "expirationEpoch",
      "expiration_epoch",
      "domainSeparator",
      "domain_separator",
      "maxIssuerBytes",
      "max_issuer_bytes",
      "maxPredicateBytes",
      "max_predicate_bytes",
    ]),
    "vegaCredentialLocalVerification",
  );
  const envelopeAlias = readSingleAlias(
    source,
    ["envelope", "proofEnvelope", "proof_envelope", "bytes"],
    "vegaCredentialLocalVerification.envelope",
    "proof envelope",
  );
  const decoded = decodeVeRangeEnvelope(
    envelopeAlias.value,
    "vegaCredentialLocalVerification.envelope",
  );
  if (decoded.backend !== "Stark") {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "vegaCredentialLocalVerification.envelope.backend must be Stark",
      "vegaCredentialLocalVerification.envelope.backend",
    );
  }
  const circuitId = normalizeVegaCircuitId(
    decoded.circuit_id,
    "vegaCredentialLocalVerification.envelope.circuitId",
  );
  const vkHash = normalizeNonZeroFixedBytes(
    decoded.vk_hash,
    "vegaCredentialLocalVerification.envelope.vkHash",
    32,
  );
  const publicInputs = parseVegaPublicInputs(
    decoded.public_inputs,
    "vegaCredentialLocalVerification.publicInputs",
  );
  ensureVegaVerificationExpectations(
    source,
    publicInputs,
    "vegaCredentialLocalVerification",
  );
  const expectedProofBytes = vegaDevProofBytes({
    circuitId,
    vkHash,
    publicInputBytes: decoded.public_inputs,
  });
  if (!fixedBytesEqual(decoded.proof_bytes, expectedProofBytes)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "vegaCredentialLocalVerification proof bytes are not a valid Vega dev fixture",
      "vegaCredentialLocalVerification.proofBytes",
    );
  }
  return {
    ok: true,
    production: false,
    kind: "vega-dev-fixture-v0",
    backend: VEGA_BACKEND,
    circuit_id: circuitId,
    verifier_key_hash: Buffer.from(vkHash).toString("hex"),
    public_inputs: publicInputs,
    public_input_bytes: decoded.public_inputs.length,
    proof_bytes: decoded.proof_bytes.length,
    aux_bytes: decoded.aux.length,
    credential_schema: publicInputs.credential_schema,
    expiration_epoch: publicInputs.expiration_epoch,
  };
}

const SILENT_THRESHOLD_COMMON_FIELDS = Object.freeze([
  "version",
  "issuerSetCommitment",
  "issuer_set_commitment",
  "issuerSet",
  "issuer_set",
  "issuerSetBytes",
  "issuer_set_bytes",
  "issuerSetJson",
  "issuer_set_json",
  "thresholdPolicyHash",
  "threshold_policy_hash",
  "thresholdPolicy",
  "threshold_policy",
  "thresholdPolicyBytes",
  "threshold_policy_bytes",
  "thresholdPolicyJson",
  "threshold_policy_json",
  "credentialShowingCommitment",
  "credential_showing_commitment",
  "showingCommitment",
  "showing_commitment",
  "credentialShowing",
  "credential_showing",
  "credentialShowingBytes",
  "credential_showing_bytes",
  "credentialShowingJson",
  "credential_showing_json",
  "showing",
  "showingBytes",
  "showing_bytes",
  "showingJson",
  "showing_json",
  "showingNullifier",
  "showing_nullifier",
  "credentialShowingNullifier",
  "credential_showing_nullifier",
  "nullifier",
  "verifierPolicyHash",
  "verifier_policy_hash",
  "verifierPolicy",
  "verifier_policy",
  "verifierPolicyBytes",
  "verifier_policy_bytes",
  "verifierPolicyJson",
  "verifier_policy_json",
  "domainSeparator",
  "domain_separator",
  "maxIssuerSetBytes",
  "max_issuer_set_bytes",
  "maxPolicyBytes",
  "max_policy_bytes",
  "maxShowingBytes",
  "max_showing_bytes",
]);

/**
 * Normalize silent-threshold anonymous credential commitment descriptors.
 *
 * Supplying structured issuer, policy, and showing material derives
 * deterministic dev commitments and hashes. Supplying only explicit
 * commitments preserves externally generated values without claiming
 * production anonymous-credential proving support.
 * @param {object} options
 * @returns {object}
 */
export function buildSilentThresholdCredentialCommitments(options) {
  const source = assertPlainObject(
    options,
    "silentThresholdCredentialCommitments",
  );
  assertAllowedFields(
    source,
    new Set(SILENT_THRESHOLD_COMMON_FIELDS),
    "silentThresholdCredentialCommitments",
  );
  const parts = normalizeSilentThresholdCommitmentParts(
    source,
    "silentThresholdCredentialCommitments",
  );
  return {
    version: parts.version,
    issuer_set_commitment: parts.issuerSet.value,
    threshold_policy_hash: parts.thresholdPolicy.value,
    credential_showing_commitment: parts.showing.value,
    showing_nullifier: parts.showingNullifier.value,
    verifier_policy_hash: parts.verifierPolicy.value,
    domain_separator: parts.domainSeparator,
    commitment_kinds: {
      issuer_set_commitment: parts.issuerSet.kind,
      threshold_policy_hash: parts.thresholdPolicy.kind,
      credential_showing_commitment: parts.showing.kind,
      showing_nullifier: parts.showingNullifier.kind,
      verifier_policy_hash: parts.verifierPolicy.kind,
    },
    source_digests: {
      issuer_set: parts.issuerSet.digest,
      threshold_policy: parts.thresholdPolicy.digest,
      credential_showing: parts.showing.digest,
      showing_nullifier: parts.showingNullifier.digest,
      verifier_policy: parts.verifierPolicy.digest,
    },
  };
}

/**
 * Build canonical OpenVerifyEnvelope bytes for a prepared silent-threshold
 * anonymous credential showing proof.
 * @param {object} options
 * @returns {Buffer}
 */
export function buildSilentThresholdCredentialEnvelope(options) {
  const source = assertPlainObject(
    options,
    "silentThresholdCredentialEnvelope",
  );
  assertAllowedFields(
    source,
    new Set([
      ...SILENT_THRESHOLD_COMMON_FIELDS,
      "backend",
      "backendTag",
      "backend_tag",
      "circuitId",
      "circuit_id",
      "vkHash",
      "vk_hash",
      "verifierKeyHash",
      "verifyingKeyHash",
      "proofBytes",
      "proof_bytes",
      "proof",
      "aux",
      "maxProofBytes",
      "max_proof_bytes",
      "maxPublicInputBytes",
      "max_public_input_bytes",
    ]),
    "silentThresholdCredentialEnvelope",
  );
  const parts = normalizeSilentThresholdProofParts(
    source,
    "silentThresholdCredentialEnvelope",
  );
  return buildPrivacyProofEnvelopeWithOptionalLimits({
    backend: parts.backend,
    circuitId: parts.circuitId,
    vkHash: parts.vkHash,
    publicInputs: parts.publicInputBytes,
    proofBytes: parts.proofBytes,
    aux: source.aux,
    maxProofBytes: parts.maxProofBytes,
    maxPublicInputBytes: parts.maxPublicInputBytes,
  });
}

/**
 * Build a deterministic silent-threshold credential dev proof fixture.
 *
 * This fixture is only for SDK/dev-verifier public-input binding tests. It is
 * not a production anonymous credential showing proof.
 * @param {object} options
 * @returns {object}
 */
export function buildSilentThresholdCredentialDevProofFixture(options) {
  const source = assertPlainObject(
    options,
    "silentThresholdCredentialDevProofFixture",
  );
  assertAllowedFields(
    source,
    new Set([
      ...SILENT_THRESHOLD_COMMON_FIELDS,
      "backend",
      "backendTag",
      "backend_tag",
      "circuitId",
      "circuit_id",
      "vkHash",
      "vk_hash",
      "verifierKeyHash",
      "verifyingKeyHash",
      "aux",
      "maxProofBytes",
      "max_proof_bytes",
      "maxPublicInputBytes",
      "max_public_input_bytes",
    ]),
    "silentThresholdCredentialDevProofFixture",
  );
  const parts = normalizeSilentThresholdProofParts(
    source,
    "silentThresholdCredentialDevProofFixture",
    { requireProofBytes: false },
  );
  const proofBytes = silentThresholdDevProofBytes(parts);
  const envelope = buildPrivacyProofEnvelopeWithOptionalLimits({
    backend: parts.backend,
    circuitId: parts.circuitId,
    vkHash: parts.vkHash,
    publicInputs: parts.publicInputBytes,
    proofBytes,
    aux: source.aux,
    maxProofBytes: parts.maxProofBytes,
    maxPublicInputBytes: parts.maxPublicInputBytes,
  });
  return {
    kind: "silent-threshold-dev-fixture-v0",
    production: false,
    proof_bytes: proofBytes,
    proofBytes: Buffer.from(proofBytes),
    commitments: {
      issuer_set_commitment: parts.commitments.issuerSet.value,
      threshold_policy_hash: parts.commitments.thresholdPolicy.value,
      credential_showing_commitment: parts.commitments.showing.value,
      showing_nullifier: parts.commitments.showingNullifier.value,
      verifier_policy_hash: parts.commitments.verifierPolicy.value,
      domain_separator: parts.commitments.domainSeparator,
    },
    public_inputs: parts.publicInputs,
    publicInputBytes: Buffer.from(parts.publicInputBytes),
    envelope,
  };
}

/**
 * Verify a deterministic silent-threshold credential dev proof fixture through
 * OpenVerifyEnvelope bytes.
 *
 * This verifier only accepts the SDK dev-fixture format and must not be used as
 * production anonymous credential cryptography.
 * @param {object|Buffer|string} options
 * @returns {object}
 */
export function verifySilentThresholdCredentialProofLocally(options) {
  const source =
    options &&
    typeof options === "object" &&
    !Array.isArray(options) &&
    !Buffer.isBuffer(options) &&
    !ArrayBuffer.isView(options) &&
    !(options instanceof ArrayBuffer)
      ? assertPlainObject(options, "silentThresholdCredentialLocalVerification")
      : { envelope: options };
  assertAllowedFields(
    source,
    new Set([
      ...SILENT_THRESHOLD_COMMON_FIELDS,
      "envelope",
      "proofEnvelope",
      "proof_envelope",
      "bytes",
    ]),
    "silentThresholdCredentialLocalVerification",
  );
  const envelopeAlias = readSingleAlias(
    source,
    ["envelope", "proofEnvelope", "proof_envelope", "bytes"],
    "silentThresholdCredentialLocalVerification.envelope",
    "proof envelope",
  );
  const decoded = decodeVeRangeEnvelope(
    envelopeAlias.value,
    "silentThresholdCredentialLocalVerification.envelope",
  );
  if (decoded.backend !== "Stark") {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "silentThresholdCredentialLocalVerification.envelope.backend must be Stark",
      "silentThresholdCredentialLocalVerification.envelope.backend",
    );
  }
  const circuitId = normalizeSilentThresholdCircuitId(
    decoded.circuit_id,
    "silentThresholdCredentialLocalVerification.envelope.circuitId",
  );
  const vkHash = normalizeNonZeroFixedBytes(
    decoded.vk_hash,
    "silentThresholdCredentialLocalVerification.envelope.vkHash",
    32,
  );
  const publicInputs = parseSilentThresholdPublicInputs(
    decoded.public_inputs,
    "silentThresholdCredentialLocalVerification.publicInputs",
  );
  ensureSilentThresholdVerificationExpectations(
    source,
    publicInputs,
    "silentThresholdCredentialLocalVerification",
  );
  const expectedProofBytes = silentThresholdDevProofBytes({
    circuitId,
    vkHash,
    publicInputBytes: decoded.public_inputs,
  });
  if (!fixedBytesEqual(decoded.proof_bytes, expectedProofBytes)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "silentThresholdCredentialLocalVerification proof bytes are not a valid silent-threshold dev fixture",
      "silentThresholdCredentialLocalVerification.proofBytes",
    );
  }
  return {
    ok: true,
    production: false,
    kind: "silent-threshold-dev-fixture-v0",
    backend: SILENT_THRESHOLD_BACKEND,
    circuit_id: circuitId,
    verifier_key_hash: Buffer.from(vkHash).toString("hex"),
    public_inputs: publicInputs,
    public_input_bytes: decoded.public_inputs.length,
    proof_bytes: decoded.proof_bytes.length,
    aux_bytes: decoded.aux.length,
    showing_nullifier: publicInputs.showing_nullifier,
  };
}

const ZK_X509_COMMON_FIELDS = Object.freeze([
  "version",
  "caRootCommitment",
  "ca_root_commitment",
  "caRoot",
  "ca_root",
  "caRootBytes",
  "ca_root_bytes",
  "caRootJson",
  "ca_root_json",
  "trustRoot",
  "trust_root",
  "trustRootJson",
  "trust_root_json",
  "certificatePolicyHash",
  "certificate_policy_hash",
  "certificatePolicy",
  "certificate_policy",
  "certificatePolicyBytes",
  "certificate_policy_bytes",
  "certificatePolicyJson",
  "certificate_policy_json",
  "revocationRoot",
  "revocation_root",
  "revocationData",
  "revocation_data",
  "revocationBytes",
  "revocation_bytes",
  "revocationJson",
  "revocation_json",
  "revocationSet",
  "revocation_set",
  "revocationSetJson",
  "revocation_set_json",
  "revocationList",
  "revocation_list",
  "revocationListJson",
  "revocation_list_json",
  "subjectCommitment",
  "subject_commitment",
  "subject",
  "subjectBytes",
  "subject_bytes",
  "subjectJson",
  "subject_json",
  "certificateSubject",
  "certificate_subject",
  "certificateSubjectJson",
  "certificate_subject_json",
  "addressBinding",
  "address_binding",
  "walletBinding",
  "wallet_binding",
  "accountId",
  "account_id",
  "walletAccountId",
  "wallet_account_id",
  "walletAddress",
  "wallet_address",
  "domainSeparator",
  "domain_separator",
  "maxCaRootBytes",
  "max_ca_root_bytes",
  "maxPolicyBytes",
  "max_policy_bytes",
  "maxRevocationBytes",
  "max_revocation_bytes",
  "maxSubjectBytes",
  "max_subject_bytes",
]);

/**
 * Normalize ZK-X.509 identity proof public-input commitments.
 *
 * Structured CA-root, certificate-policy, revocation, subject, and address
 * material is hashed into deterministic dev commitments. Explicit commitments
 * are preserved for externally generated proof systems.
 * @param {object} options
 * @returns {object}
 */
export function buildZkX509IdentityCommitments(options) {
  const source = assertPlainObject(options, "zkX509IdentityCommitments");
  assertAllowedFields(
    source,
    new Set(ZK_X509_COMMON_FIELDS),
    "zkX509IdentityCommitments",
  );
  const parts = normalizeZkX509CommitmentParts(
    source,
    "zkX509IdentityCommitments",
  );
  return {
    version: parts.version,
    ca_root_commitment: parts.caRoot.value,
    certificate_policy_hash: parts.certificatePolicy.value,
    revocation_root: parts.revocationRoot.value,
    subject_commitment: parts.subject.value,
    address_binding: parts.addressBinding.value,
    domain_separator: parts.domainSeparator,
    commitment_kinds: {
      ca_root_commitment: parts.caRoot.kind,
      certificate_policy_hash: parts.certificatePolicy.kind,
      revocation_root: parts.revocationRoot.kind,
      subject_commitment: parts.subject.kind,
      address_binding: parts.addressBinding.kind,
    },
    source_digests: {
      ca_root: parts.caRoot.digest,
      certificate_policy: parts.certificatePolicy.digest,
      revocation: parts.revocationRoot.digest,
      subject: parts.subject.digest,
      address_binding: parts.addressBinding.digest,
    },
  };
}

/**
 * Build canonical OpenVerifyEnvelope bytes for a prepared ZK-X.509 identity
 * proof.
 * @param {object} options
 * @returns {Buffer}
 */
export function buildZkX509IdentityEnvelope(options) {
  const source = assertPlainObject(options, "zkX509IdentityEnvelope");
  assertAllowedFields(
    source,
    new Set([
      ...ZK_X509_COMMON_FIELDS,
      "backend",
      "backendTag",
      "backend_tag",
      "circuitId",
      "circuit_id",
      "vkHash",
      "vk_hash",
      "verifierKeyHash",
      "verifyingKeyHash",
      "proofBytes",
      "proof_bytes",
      "proof",
      "aux",
      "maxProofBytes",
      "max_proof_bytes",
      "maxPublicInputBytes",
      "max_public_input_bytes",
    ]),
    "zkX509IdentityEnvelope",
  );
  const parts = normalizeZkX509ProofParts(source, "zkX509IdentityEnvelope");
  return buildPrivacyProofEnvelopeWithOptionalLimits({
    backend: parts.backend,
    circuitId: parts.circuitId,
    vkHash: parts.vkHash,
    publicInputs: parts.publicInputBytes,
    proofBytes: parts.proofBytes,
    aux: source.aux,
    maxProofBytes: parts.maxProofBytes,
    maxPublicInputBytes: parts.maxPublicInputBytes,
  });
}

/**
 * Build a deterministic ZK-X.509 identity dev proof fixture.
 *
 * This fixture is only for SDK/dev-verifier public-input binding tests. It is
 * not a production X.509 certificate validity, revocation, or ownership proof.
 * @param {object} options
 * @returns {object}
 */
export function buildZkX509IdentityDevProofFixture(options) {
  const source = assertPlainObject(options, "zkX509IdentityDevProofFixture");
  assertAllowedFields(
    source,
    new Set([
      ...ZK_X509_COMMON_FIELDS,
      "backend",
      "backendTag",
      "backend_tag",
      "circuitId",
      "circuit_id",
      "vkHash",
      "vk_hash",
      "verifierKeyHash",
      "verifyingKeyHash",
      "aux",
      "maxProofBytes",
      "max_proof_bytes",
      "maxPublicInputBytes",
      "max_public_input_bytes",
    ]),
    "zkX509IdentityDevProofFixture",
  );
  const parts = normalizeZkX509ProofParts(
    source,
    "zkX509IdentityDevProofFixture",
    { requireProofBytes: false },
  );
  const proofBytes = zkX509DevProofBytes(parts);
  const envelope = buildPrivacyProofEnvelopeWithOptionalLimits({
    backend: parts.backend,
    circuitId: parts.circuitId,
    vkHash: parts.vkHash,
    publicInputs: parts.publicInputBytes,
    proofBytes,
    aux: source.aux,
    maxProofBytes: parts.maxProofBytes,
    maxPublicInputBytes: parts.maxPublicInputBytes,
  });
  return {
    kind: "zk-x509-dev-fixture-v0",
    production: false,
    proof_bytes: proofBytes,
    proofBytes: Buffer.from(proofBytes),
    commitments: {
      ca_root_commitment: parts.commitments.caRoot.value,
      certificate_policy_hash: parts.commitments.certificatePolicy.value,
      revocation_root: parts.commitments.revocationRoot.value,
      subject_commitment: parts.commitments.subject.value,
      address_binding: parts.commitments.addressBinding.value,
      domain_separator: parts.commitments.domainSeparator,
    },
    public_inputs: parts.publicInputs,
    publicInputBytes: Buffer.from(parts.publicInputBytes),
    envelope,
  };
}

/**
 * Verify a deterministic ZK-X.509 identity dev proof fixture through
 * OpenVerifyEnvelope bytes.
 *
 * This verifier only accepts the SDK dev-fixture format and must not be used
 * as production X.509 certificate cryptography.
 * @param {object|Buffer|string} options
 * @returns {object}
 */
export function verifyZkX509IdentityProofLocally(options) {
  const source =
    options &&
    typeof options === "object" &&
    !Array.isArray(options) &&
    !Buffer.isBuffer(options) &&
    !ArrayBuffer.isView(options) &&
    !(options instanceof ArrayBuffer)
      ? assertPlainObject(options, "zkX509IdentityLocalVerification")
      : { envelope: options };
  assertAllowedFields(
    source,
    new Set([
      ...ZK_X509_COMMON_FIELDS,
      "envelope",
      "proofEnvelope",
      "proof_envelope",
      "bytes",
    ]),
    "zkX509IdentityLocalVerification",
  );
  const envelopeAlias = readSingleAlias(
    source,
    ["envelope", "proofEnvelope", "proof_envelope", "bytes"],
    "zkX509IdentityLocalVerification.envelope",
    "proof envelope",
  );
  const decoded = decodeVeRangeEnvelope(
    envelopeAlias.value,
    "zkX509IdentityLocalVerification.envelope",
  );
  if (decoded.backend !== "Stark") {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "zkX509IdentityLocalVerification.envelope.backend must be Stark",
      "zkX509IdentityLocalVerification.envelope.backend",
    );
  }
  const circuitId = normalizeZkX509CircuitId(
    decoded.circuit_id,
    "zkX509IdentityLocalVerification.envelope.circuitId",
  );
  const vkHash = normalizeNonZeroFixedBytes(
    decoded.vk_hash,
    "zkX509IdentityLocalVerification.envelope.vkHash",
    32,
  );
  const publicInputs = parseZkX509PublicInputs(
    decoded.public_inputs,
    "zkX509IdentityLocalVerification.publicInputs",
  );
  ensureZkX509VerificationExpectations(
    source,
    publicInputs,
    "zkX509IdentityLocalVerification",
  );
  const expectedProofBytes = zkX509DevProofBytes({
    circuitId,
    vkHash,
    publicInputBytes: decoded.public_inputs,
  });
  if (!fixedBytesEqual(decoded.proof_bytes, expectedProofBytes)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "zkX509IdentityLocalVerification proof bytes are not a valid ZK-X.509 dev fixture",
      "zkX509IdentityLocalVerification.proofBytes",
    );
  }
  return {
    ok: true,
    production: false,
    kind: "zk-x509-dev-fixture-v0",
    backend: ZK_X509_BACKEND,
    circuit_id: circuitId,
    verifier_key_hash: Buffer.from(vkHash).toString("hex"),
    public_inputs: publicInputs,
    public_input_bytes: decoded.public_inputs.length,
    proof_bytes: decoded.proof_bytes.length,
    aux_bytes: decoded.aux.length,
    address_binding: publicInputs.address_binding,
  };
}

const JINDO_COMMON_FIELDS = Object.freeze([
  "version",
  "commitment",
  "polynomialCommitment",
  "polynomial_commitment",
  "polynomial",
  "polynomialBytes",
  "polynomial_bytes",
  "polynomialJson",
  "polynomial_json",
  "commitmentMaterial",
  "commitment_material",
  "commitmentMaterialJson",
  "commitment_material_json",
  "openingClaimCommitment",
  "opening_claim_commitment",
  "openingClaimHash",
  "opening_claim_hash",
  "openingClaimDigest",
  "opening_claim_digest",
  "opening_claim",
  "openingClaim",
  "claim",
  "claimBytes",
  "claim_bytes",
  "claimJson",
  "claim_json",
  "openingClaimBytes",
  "opening_claim_bytes",
  "openingClaimJson",
  "opening_claim_json",
  "evaluationClaim",
  "evaluation_claim",
  "evaluationClaimJson",
  "evaluation_claim_json",
  "querySetHash",
  "query_set_hash",
  "querySetRoot",
  "query_set_root",
  "querySet",
  "query_set",
  "querySetBytes",
  "query_set_bytes",
  "querySetJson",
  "query_set_json",
  "queries",
  "queriesJson",
  "queries_json",
  "parameterHash",
  "parameter_hash",
  "paramsHash",
  "params_hash",
  "parameters",
  "parametersBytes",
  "parameters_bytes",
  "parametersJson",
  "parameters_json",
  "parameterSet",
  "parameter_set",
  "parameterSetJson",
  "parameter_set_json",
  "params",
  "paramsBytes",
  "params_bytes",
  "paramsJson",
  "params_json",
  "domainSeparator",
  "domain_separator",
  "maxPolynomialBytes",
  "max_polynomial_bytes",
  "maxOpeningClaimBytes",
  "max_opening_claim_bytes",
  "maxQuerySetBytes",
  "max_query_set_bytes",
  "maxParameterBytes",
  "max_parameter_bytes",
]);

/**
 * Normalize Jindo lattice PCS public inputs for SDK/dev-fixture use.
 *
 * This helper derives deterministic hashes from polynomial, opening claim,
 * query-set, and parameter material. It does not implement a production Jindo
 * lattice PCS prover or verifier.
 * @param {object} options
 * @returns {object}
 */
export function buildJindoLatticePublicInputs(options) {
  const source = assertPlainObject(options, "jindoLatticePublicInputs");
  assertAllowedFields(
    source,
    new Set(JINDO_COMMON_FIELDS),
    "jindoLatticePublicInputs",
  );
  const parts = normalizeJindoPublicInputParts(source, "jindoLatticePublicInputs");
  return {
    version: parts.version,
    commitment: parts.commitment.value,
    opening_claim: parts.openingClaim.value,
    query_set: parts.querySet.value,
    parameter_hash: parts.parameterHash.value,
    domain_separator: parts.domainSeparator,
    commitment_kinds: {
      commitment: parts.commitment.kind,
      opening_claim: parts.openingClaim.kind,
      query_set: parts.querySet.kind,
      parameter_hash: parts.parameterHash.kind,
    },
    source_digests: {
      polynomial: parts.commitment.digest,
      opening_claim: parts.openingClaim.digest,
      query_set: parts.querySet.digest,
      parameters: parts.parameterHash.digest,
    },
  };
}

/**
 * Build canonical OpenVerifyEnvelope bytes for a prepared Jindo lattice PCS
 * proof candidate.
 * @param {object} options
 * @returns {Buffer}
 */
export function buildJindoLatticeProofEnvelope(options) {
  const source = assertPlainObject(options, "jindoLatticeProofEnvelope");
  assertAllowedFields(
    source,
    new Set([
      ...JINDO_COMMON_FIELDS,
      "backend",
      "backendTag",
      "backend_tag",
      "circuitId",
      "circuit_id",
      "vkHash",
      "vk_hash",
      "verifierKeyHash",
      "verifyingKeyHash",
      "proofBytes",
      "proof_bytes",
      "proof",
      "aux",
      "maxProofBytes",
      "max_proof_bytes",
      "maxPublicInputBytes",
      "max_public_input_bytes",
    ]),
    "jindoLatticeProofEnvelope",
  );
  const parts = normalizeJindoProofParts(source, "jindoLatticeProofEnvelope");
  return buildPrivacyProofEnvelopeWithOptionalLimits({
    backend: parts.backend,
    circuitId: parts.circuitId,
    vkHash: parts.vkHash,
    publicInputs: parts.publicInputBytes,
    proofBytes: parts.proofBytes,
    aux: source.aux,
    maxProofBytes: parts.maxProofBytes,
    maxPublicInputBytes: parts.maxPublicInputBytes,
  });
}

/**
 * Build a deterministic Jindo lattice PCS dev proof fixture.
 *
 * This fixture is only for public-input binding and SDK packaging tests. It is
 * not a production lattice polynomial-commitment proof.
 * @param {object} options
 * @returns {object}
 */
export function buildJindoLatticeDevProofFixture(options) {
  const source = assertPlainObject(options, "jindoLatticeDevProofFixture");
  assertAllowedFields(
    source,
    new Set([
      ...JINDO_COMMON_FIELDS,
      "backend",
      "backendTag",
      "backend_tag",
      "circuitId",
      "circuit_id",
      "vkHash",
      "vk_hash",
      "verifierKeyHash",
      "verifyingKeyHash",
      "aux",
      "maxProofBytes",
      "max_proof_bytes",
      "maxPublicInputBytes",
      "max_public_input_bytes",
    ]),
    "jindoLatticeDevProofFixture",
  );
  const parts = normalizeJindoProofParts(
    source,
    "jindoLatticeDevProofFixture",
    { requireProofBytes: false },
  );
  const proofBytes = jindoDevProofBytes(parts);
  const envelope = buildPrivacyProofEnvelopeWithOptionalLimits({
    backend: parts.backend,
    circuitId: parts.circuitId,
    vkHash: parts.vkHash,
    publicInputs: parts.publicInputBytes,
    proofBytes,
    aux: source.aux,
    maxProofBytes: parts.maxProofBytes,
    maxPublicInputBytes: parts.maxPublicInputBytes,
  });
  return {
    kind: "jindo-lattice-dev-fixture-v0",
    production: false,
    proof_bytes: proofBytes,
    proofBytes: Buffer.from(proofBytes),
    public_inputs: parts.publicInputs,
    publicInputBytes: Buffer.from(parts.publicInputBytes),
    envelope,
  };
}

/**
 * Verify a deterministic Jindo lattice PCS dev proof fixture through
 * OpenVerifyEnvelope bytes.
 *
 * This verifier only accepts the SDK dev-fixture format and must not be used
 * as production lattice polynomial-commitment cryptography.
 * @param {object|Buffer|string} options
 * @returns {object}
 */
export function verifyJindoLatticeProofLocally(options) {
  const source =
    options &&
    typeof options === "object" &&
    !Array.isArray(options) &&
    !Buffer.isBuffer(options) &&
    !ArrayBuffer.isView(options) &&
    !(options instanceof ArrayBuffer)
      ? assertPlainObject(options, "jindoLatticeLocalVerification")
      : { envelope: options };
  assertAllowedFields(
    source,
    new Set([
      ...JINDO_COMMON_FIELDS,
      "envelope",
      "proofEnvelope",
      "proof_envelope",
      "bytes",
    ]),
    "jindoLatticeLocalVerification",
  );
  const envelopeAlias = readSingleAlias(
    source,
    ["envelope", "proofEnvelope", "proof_envelope", "bytes"],
    "jindoLatticeLocalVerification.envelope",
    "proof envelope",
  );
  const decoded = decodeVeRangeEnvelope(
    envelopeAlias.value,
    "jindoLatticeLocalVerification.envelope",
  );
  if (decoded.backend !== "Unsupported") {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "jindoLatticeLocalVerification.envelope.backend must be Unsupported until a production Jindo backend is registered",
      "jindoLatticeLocalVerification.envelope.backend",
    );
  }
  const circuitId = normalizeJindoCircuitId(
    decoded.circuit_id,
    "jindoLatticeLocalVerification.envelope.circuitId",
  );
  const vkHash = normalizeNonZeroFixedBytes(
    decoded.vk_hash,
    "jindoLatticeLocalVerification.envelope.vkHash",
    32,
  );
  const publicInputs = parseJindoPublicInputs(
    decoded.public_inputs,
    "jindoLatticeLocalVerification.publicInputs",
  );
  ensureJindoVerificationExpectations(
    source,
    publicInputs,
    "jindoLatticeLocalVerification",
  );
  const expectedProofBytes = jindoDevProofBytes({
    circuitId,
    vkHash,
    publicInputBytes: decoded.public_inputs,
  });
  if (!fixedBytesEqual(decoded.proof_bytes, expectedProofBytes)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "jindoLatticeLocalVerification proof bytes are not a valid Jindo dev fixture",
      "jindoLatticeLocalVerification.proofBytes",
    );
  }
  return {
    ok: true,
    production: false,
    kind: "jindo-lattice-dev-fixture-v0",
    backend: JINDO_BACKEND,
    circuit_id: circuitId,
    verifier_key_hash: Buffer.from(vkHash).toString("hex"),
    public_inputs: publicInputs,
    public_input_bytes: decoded.public_inputs.length,
    proof_bytes: decoded.proof_bytes.length,
    aux_bytes: decoded.aux.length,
    parameter_hash: publicInputs.parameter_hash,
  };
}

const SIS_HINTS_COMMON_FIELDS = Object.freeze([
  "version",
  "issuerCommitment",
  "issuer_commitment",
  "issuerParameterCommitment",
  "issuer_parameter_commitment",
  "issuer",
  "issuerBytes",
  "issuer_bytes",
  "issuerJson",
  "issuer_json",
  "issuerParameters",
  "issuer_parameters",
  "issuerParametersJson",
  "issuer_parameters_json",
  "credentialCommitment",
  "credential_commitment",
  "credentialShowingCommitment",
  "credential_showing_commitment",
  "credential",
  "credentialBytes",
  "credential_bytes",
  "credentialJson",
  "credential_json",
  "credentialShowing",
  "credential_showing",
  "credentialShowingJson",
  "credential_showing_json",
  "showing",
  "showingJson",
  "showing_json",
  "showingPolicyHash",
  "showing_policy_hash",
  "policyHash",
  "policy_hash",
  "verifierPolicyHash",
  "verifier_policy_hash",
  "showingPolicy",
  "showing_policy",
  "showingPolicyBytes",
  "showing_policy_bytes",
  "showingPolicyJson",
  "showing_policy_json",
  "policy",
  "policyBytes",
  "policy_bytes",
  "policyJson",
  "policy_json",
  "verifierPolicy",
  "verifier_policy",
  "verifierPolicyJson",
  "verifier_policy_json",
  "parameterHash",
  "parameter_hash",
  "paramsHash",
  "params_hash",
  "parameters",
  "parametersBytes",
  "parameters_bytes",
  "parametersJson",
  "parameters_json",
  "parameterSet",
  "parameter_set",
  "parameterSetJson",
  "parameter_set_json",
  "sisParameters",
  "sis_parameters",
  "sisParametersJson",
  "sis_parameters_json",
  "params",
  "paramsJson",
  "params_json",
  "domainSeparator",
  "domain_separator",
  "maxIssuerBytes",
  "max_issuer_bytes",
  "maxCredentialBytes",
  "max_credential_bytes",
  "maxPolicyBytes",
  "max_policy_bytes",
  "maxParameterBytes",
  "max_parameter_bytes",
]);

/**
 * Normalize SIS-with-hints anonymous credential public-input commitments.
 *
 * This helper derives deterministic issuer, credential, showing-policy, and
 * parameter hashes for SDK/dev fixtures. It does not implement production
 * lattice anonymous credential proving.
 * @param {object} options
 * @returns {object}
 */
export function buildSisHintsCredentialCommitments(options) {
  const source = assertPlainObject(options, "sisHintsCredentialCommitments");
  assertAllowedFields(
    source,
    new Set(SIS_HINTS_COMMON_FIELDS),
    "sisHintsCredentialCommitments",
  );
  const parts = normalizeSisHintsCommitmentParts(
    source,
    "sisHintsCredentialCommitments",
  );
  return {
    version: parts.version,
    issuer_commitment: parts.issuer.value,
    credential_commitment: parts.credential.value,
    showing_policy_hash: parts.showingPolicy.value,
    parameter_hash: parts.parameterHash.value,
    domain_separator: parts.domainSeparator,
    commitment_kinds: {
      issuer_commitment: parts.issuer.kind,
      credential_commitment: parts.credential.kind,
      showing_policy_hash: parts.showingPolicy.kind,
      parameter_hash: parts.parameterHash.kind,
    },
    source_digests: {
      issuer: parts.issuer.digest,
      credential: parts.credential.digest,
      showing_policy: parts.showingPolicy.digest,
      parameters: parts.parameterHash.digest,
    },
  };
}

/**
 * Build canonical OpenVerifyEnvelope bytes for a prepared SIS-with-hints
 * credential proof candidate.
 * @param {object} options
 * @returns {Buffer}
 */
export function buildSisHintsCredentialEnvelope(options) {
  const source = assertPlainObject(options, "sisHintsCredentialEnvelope");
  assertAllowedFields(
    source,
    new Set([
      ...SIS_HINTS_COMMON_FIELDS,
      "backend",
      "backendTag",
      "backend_tag",
      "circuitId",
      "circuit_id",
      "vkHash",
      "vk_hash",
      "verifierKeyHash",
      "verifyingKeyHash",
      "proofBytes",
      "proof_bytes",
      "proof",
      "aux",
      "maxProofBytes",
      "max_proof_bytes",
      "maxPublicInputBytes",
      "max_public_input_bytes",
    ]),
    "sisHintsCredentialEnvelope",
  );
  const parts = normalizeSisHintsProofParts(
    source,
    "sisHintsCredentialEnvelope",
  );
  return buildPrivacyProofEnvelopeWithOptionalLimits({
    backend: parts.backend,
    circuitId: parts.circuitId,
    vkHash: parts.vkHash,
    publicInputs: parts.publicInputBytes,
    proofBytes: parts.proofBytes,
    aux: source.aux,
    maxProofBytes: parts.maxProofBytes,
    maxPublicInputBytes: parts.maxPublicInputBytes,
  });
}

/**
 * Build a deterministic SIS-with-hints credential dev proof fixture.
 *
 * This fixture is only for public-input binding and SDK packaging tests. It is
 * not a production lattice anonymous credential proof.
 * @param {object} options
 * @returns {object}
 */
export function buildSisHintsCredentialDevProofFixture(options) {
  const source = assertPlainObject(
    options,
    "sisHintsCredentialDevProofFixture",
  );
  assertAllowedFields(
    source,
    new Set([
      ...SIS_HINTS_COMMON_FIELDS,
      "backend",
      "backendTag",
      "backend_tag",
      "circuitId",
      "circuit_id",
      "vkHash",
      "vk_hash",
      "verifierKeyHash",
      "verifyingKeyHash",
      "aux",
      "maxProofBytes",
      "max_proof_bytes",
      "maxPublicInputBytes",
      "max_public_input_bytes",
    ]),
    "sisHintsCredentialDevProofFixture",
  );
  const parts = normalizeSisHintsProofParts(
    source,
    "sisHintsCredentialDevProofFixture",
    { requireProofBytes: false },
  );
  const proofBytes = sisHintsDevProofBytes(parts);
  const envelope = buildPrivacyProofEnvelopeWithOptionalLimits({
    backend: parts.backend,
    circuitId: parts.circuitId,
    vkHash: parts.vkHash,
    publicInputs: parts.publicInputBytes,
    proofBytes,
    aux: source.aux,
    maxProofBytes: parts.maxProofBytes,
    maxPublicInputBytes: parts.maxPublicInputBytes,
  });
  return {
    kind: "sis-hints-dev-fixture-v0",
    production: false,
    proof_bytes: proofBytes,
    proofBytes: Buffer.from(proofBytes),
    commitments: {
      issuer_commitment: parts.commitments.issuer.value,
      credential_commitment: parts.commitments.credential.value,
      showing_policy_hash: parts.commitments.showingPolicy.value,
      parameter_hash: parts.commitments.parameterHash.value,
      domain_separator: parts.commitments.domainSeparator,
    },
    public_inputs: parts.publicInputs,
    publicInputBytes: Buffer.from(parts.publicInputBytes),
    envelope,
  };
}

/**
 * Verify a deterministic SIS-with-hints credential dev proof fixture through
 * OpenVerifyEnvelope bytes.
 *
 * This verifier only accepts the SDK dev-fixture format and must not be used
 * as production lattice anonymous credential cryptography.
 * @param {object|Buffer|string} options
 * @returns {object}
 */
export function verifySisHintsCredentialProofLocally(options) {
  const source =
    options &&
    typeof options === "object" &&
    !Array.isArray(options) &&
    !Buffer.isBuffer(options) &&
    !ArrayBuffer.isView(options) &&
    !(options instanceof ArrayBuffer)
      ? assertPlainObject(options, "sisHintsCredentialLocalVerification")
      : { envelope: options };
  assertAllowedFields(
    source,
    new Set([
      ...SIS_HINTS_COMMON_FIELDS,
      "envelope",
      "proofEnvelope",
      "proof_envelope",
      "bytes",
    ]),
    "sisHintsCredentialLocalVerification",
  );
  const envelopeAlias = readSingleAlias(
    source,
    ["envelope", "proofEnvelope", "proof_envelope", "bytes"],
    "sisHintsCredentialLocalVerification.envelope",
    "proof envelope",
  );
  const decoded = decodeVeRangeEnvelope(
    envelopeAlias.value,
    "sisHintsCredentialLocalVerification.envelope",
  );
  if (decoded.backend !== "Unsupported") {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "sisHintsCredentialLocalVerification.envelope.backend must be Unsupported until a production SIS-with-hints backend is registered",
      "sisHintsCredentialLocalVerification.envelope.backend",
    );
  }
  const circuitId = normalizeSisHintsCircuitId(
    decoded.circuit_id,
    "sisHintsCredentialLocalVerification.envelope.circuitId",
  );
  const vkHash = normalizeNonZeroFixedBytes(
    decoded.vk_hash,
    "sisHintsCredentialLocalVerification.envelope.vkHash",
    32,
  );
  const publicInputs = parseSisHintsPublicInputs(
    decoded.public_inputs,
    "sisHintsCredentialLocalVerification.publicInputs",
  );
  ensureSisHintsVerificationExpectations(
    source,
    publicInputs,
    "sisHintsCredentialLocalVerification",
  );
  const expectedProofBytes = sisHintsDevProofBytes({
    circuitId,
    vkHash,
    publicInputBytes: decoded.public_inputs,
  });
  if (!fixedBytesEqual(decoded.proof_bytes, expectedProofBytes)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "sisHintsCredentialLocalVerification proof bytes are not a valid SIS-with-hints dev fixture",
      "sisHintsCredentialLocalVerification.proofBytes",
    );
  }
  return {
    ok: true,
    production: false,
    kind: "sis-hints-dev-fixture-v0",
    backend: SIS_HINTS_BACKEND,
    circuit_id: circuitId,
    verifier_key_hash: Buffer.from(vkHash).toString("hex"),
    public_inputs: publicInputs,
    public_input_bytes: decoded.public_inputs.length,
    proof_bytes: decoded.proof_bytes.length,
    aux_bytes: decoded.aux.length,
    parameter_hash: publicInputs.parameter_hash,
  };
}

/**
 * Normalize an Anonymous PGC receiver set and derive its deterministic set commitment.
 *
 * This helper binds receiver account commitments and receiver ciphertext
 * commitments for SDK/dev-fixture use. It does not register receiver state on
 * chain and does not create production Anonymous PGC ciphertexts.
 * @param {object} options
 * @returns {object}
 */
export function buildAnonymousPgcReceiverSet(options, context = "anonymousPgcReceiverSet") {
  const source = assertPlainObject(options, context);
  assertAllowedFields(
    source,
    new Set(["version", "threshold", "k", "receivers"]),
    context,
  );
  const thresholdAlias = readSingleAlias(
    source,
    ["threshold", "k"],
    `${context}.threshold`,
    "receiver threshold",
  );
  if (!Array.isArray(source.receivers) || source.receivers.length === 0) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.receivers must be a non-empty array`,
      `${context}.receivers`,
    );
  }
  if (source.receivers.length > ANON_PGC_MAX_RECEIVERS) {
    fail(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${context}.receivers must contain at most ${ANON_PGC_MAX_RECEIVERS} entries`,
      `${context}.receivers`,
    );
  }
  const version = normalizeAnonymousPgcVersion(source.version, `${context}.version`);
  const receivers = source.receivers.map((entry, index) =>
    normalizeAnonymousPgcReceiverEntry(entry, `${context}.receivers[${index}]`),
  );
  const threshold = normalizePositiveU32(
    thresholdAlias.value ?? receivers.length,
    `${context}.threshold`,
  );
  if (threshold > receivers.length) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `${context}.threshold must not exceed receivers length`,
      `${context}.threshold`,
    );
  }
  const seenAccounts = new Set();
  const seenCiphertexts = new Set();
  for (const receiver of receivers) {
    const accountHex = Buffer.from(receiver.account_commitment).toString("hex");
    const ciphertextHex = Buffer.from(receiver.ciphertext_commitment).toString("hex");
    if (seenAccounts.has(accountHex)) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.receivers must not contain duplicate account commitments`,
        `${context}.receivers`,
      );
    }
    if (seenCiphertexts.has(ciphertextHex)) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.receivers must not contain duplicate ciphertext commitments`,
        `${context}.receivers`,
      );
    }
    seenAccounts.add(accountHex);
    seenCiphertexts.add(ciphertextHex);
  }
  const receiverSet = {
    version,
    threshold,
    receiver_count: receivers.length,
    receivers,
  };
  receiverSet.receiver_set_commitment = anonymousPgcReceiverSetCommitment(receiverSet);
  return receiverSet;
}

/**
 * Build a deterministic Anonymous PGC dev proof fixture bound to an OpenVerify envelope.
 *
 * This fixture exercises public-input binding for the SDK and future dev
 * verifier. It is not a production Anonymous PGC proof.
 * @param {object} options
 * @returns {object}
 */
export function buildAnonymousPgcDevProofFixture(options) {
  const source = assertPlainObject(options, "anonymousPgcDevProofFixture");
  assertAllowedFields(
    source,
    new Set([
      "backend",
      "backendTag",
      "backend_tag",
      "circuitId",
      "circuit_id",
      "vkHash",
      "vk_hash",
      "verifierKeyHash",
      "verifyingKeyHash",
      "receiverSet",
      "receiver_set",
      "version",
      "threshold",
      "k",
      "receivers",
      "anonymitySetRoot",
      "anonymity_set_root",
      "txDigest",
      "tx_digest",
      "payloadDigest",
      "payload_digest",
      "payload",
      "payloadBytes",
      "payload_bytes",
      "payloadJson",
      "payload_json",
      "balanceCommitments",
      "balance_commitments",
      "linkTag",
      "link_tag",
      "rangeCommitments",
      "range_commitments",
      "chainId",
      "chain_id",
      "domainSeparator",
      "domain_separator",
      "aux",
      "maxProofBytes",
      "max_proof_bytes",
      "maxPublicInputBytes",
      "max_public_input_bytes",
      "maxPayloadBytes",
      "max_payload_bytes",
    ]),
    "anonymousPgcDevProofFixture",
  );
  const parts = normalizeAnonymousPgcProofParts(
    source,
    "anonymousPgcDevProofFixture",
  );
  const proofBytes = anonymousPgcDevProofBytes(parts);
  const envelope = buildPrivacyProofEnvelopeWithOptionalLimits({
    backend: parts.backend,
    circuitId: parts.circuitId,
    vkHash: parts.vkHash,
    publicInputs: parts.publicInputBytes,
    proofBytes,
    aux: source.aux,
    maxProofBytes: parts.maxProofBytes,
    maxPublicInputBytes: parts.maxPublicInputBytes,
  });
  return {
    kind: "anonymous-pgc-dev-fixture-v1",
    production: false,
    proof_bytes: proofBytes,
    proofBytes: Buffer.from(proofBytes),
    receiver_set: parts.receiverSet,
    public_inputs: parts.publicInputs,
    publicInputBytes: Buffer.from(parts.publicInputBytes),
    envelope,
  };
}

/**
 * Verify a deterministic Anonymous PGC dev proof fixture through OpenVerifyEnvelope bytes.
 *
 * This verifier only accepts the SDK dev-fixture format and must not be used
 * as production Anonymous PGC cryptography.
 * @param {object|Buffer|string} options
 * @returns {object}
 */
export function verifyAnonymousPgcDevProofLocally(options) {
  const source =
    options &&
    typeof options === "object" &&
    !Array.isArray(options) &&
    !Buffer.isBuffer(options) &&
    !ArrayBuffer.isView(options) &&
    !(options instanceof ArrayBuffer)
      ? assertPlainObject(options, "anonymousPgcDevProofLocalVerification")
      : { envelope: options };
  assertAllowedFields(
    source,
    new Set([
      "envelope",
      "proofEnvelope",
      "proof_envelope",
      "bytes",
      "receiverSet",
      "receiver_set",
      "version",
      "threshold",
      "k",
      "receivers",
      "anonymitySetRoot",
      "anonymity_set_root",
      "txDigest",
      "tx_digest",
      "payloadDigest",
      "payload_digest",
      "payload",
      "payloadBytes",
      "payload_bytes",
      "payloadJson",
      "payload_json",
      "balanceCommitments",
      "balance_commitments",
      "linkTag",
      "link_tag",
      "rangeCommitments",
      "range_commitments",
      "chainId",
      "chain_id",
      "domainSeparator",
      "domain_separator",
      "maxPayloadBytes",
      "max_payload_bytes",
    ]),
    "anonymousPgcDevProofLocalVerification",
  );
  const envelopeAlias = readSingleAlias(
    source,
    ["envelope", "proofEnvelope", "proof_envelope", "bytes"],
    "anonymousPgcDevProofLocalVerification.envelope",
    "proof envelope",
  );
  const decoded = decodeVeRangeEnvelope(
    envelopeAlias.value,
    "anonymousPgcDevProofLocalVerification.envelope",
  );
  if (decoded.backend !== "Stark") {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "anonymousPgcDevProofLocalVerification.envelope.backend must be Stark",
      "anonymousPgcDevProofLocalVerification.envelope.backend",
    );
  }
  const circuitId = normalizeAnonymousPgcCircuitId(
    decoded.circuit_id,
    "anonymousPgcDevProofLocalVerification.envelope.circuitId",
  );
  const vkHash = normalizeNonZeroFixedBytes(
    decoded.vk_hash,
    "anonymousPgcDevProofLocalVerification.envelope.vkHash",
    32,
  );
  const publicInputs = parseAnonymousPgcPublicInputs(
    decoded.public_inputs,
    "anonymousPgcDevProofLocalVerification.publicInputs",
  );
  ensureAnonymousPgcVerificationExpectations(
    source,
    publicInputs,
    "anonymousPgcDevProofLocalVerification",
  );
  const expectedProofBytes = anonymousPgcDevProofBytes({
    circuitId,
    vkHash,
    publicInputBytes: decoded.public_inputs,
  });
  if (!fixedBytesEqual(decoded.proof_bytes, expectedProofBytes)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "anonymousPgcDevProofLocalVerification proof bytes are not a valid Anonymous PGC dev fixture",
      "anonymousPgcDevProofLocalVerification.proofBytes",
    );
  }
  return {
    ok: true,
    production: false,
    kind: "anonymous-pgc-dev-fixture-v1",
    backend: ANON_PGC_BACKEND,
    circuit_id: circuitId,
    verifier_key_hash: Buffer.from(vkHash).toString("hex"),
    public_inputs: publicInputs,
    public_input_bytes: decoded.public_inputs.length,
    proof_bytes: decoded.proof_bytes.length,
    aux_bytes: decoded.aux.length,
    receiver_count: publicInputs.receiver_count,
    receiver_threshold: publicInputs.receiver_threshold,
  };
}

/**
 * Normalize a prepared VeRange commitment descriptor.
 *
 * This builder does not create a cryptographic commitment; callers must pass
 * a nonzero 32-byte commitment produced by their VeRange-compatible wallet.
 * @param {object} options
 * @returns {object}
 */
export function buildRangeCommitment(options, context = "rangeCommitment") {
  const source = assertPlainObject(options, context);
  assertAllowedFields(
    source,
    new Set([
      "version",
      "commitment",
      "rangeCommitment",
      "range_commitment",
      "valueCommitment",
      "value_commitment",
      "bitLength",
      "bit_length",
      "aggregationCount",
      "aggregation_count",
      "commitmentScheme",
      "commitment_scheme",
      "domainSeparator",
      "domain_separator",
      "payloadDigest",
      "payload_digest",
      "txDigest",
      "tx_digest",
      "payload",
      "payloadBytes",
      "payload_bytes",
      "payloadJson",
      "payload_json",
      "maxPayloadBytes",
      "max_payload_bytes",
    ]),
    context,
  );
  const commitmentAlias = readSingleAlias(
    source,
    ["commitment", "rangeCommitment", "range_commitment", "valueCommitment", "value_commitment"],
    `${context}.commitment`,
    "commitment",
  );
  const bitLengthAlias = readSingleAlias(
    source,
    ["bitLength", "bit_length"],
    `${context}.bitLength`,
    "bit length",
  );
  const aggregationAlias = readSingleAlias(
    source,
    ["aggregationCount", "aggregation_count"],
    `${context}.aggregationCount`,
    "aggregation count",
  );
  const schemeAlias = readSingleAlias(
    source,
    ["commitmentScheme", "commitment_scheme"],
    `${context}.commitmentScheme`,
    "commitment scheme",
  );
  const domainAlias = readSingleAlias(
    source,
    ["domainSeparator", "domain_separator"],
    `${context}.domainSeparator`,
    "domain separator",
  );
  return {
    version: normalizeVeRangeVersion(source.version, `${context}.version`),
    commitment: normalizeNonZeroFixedBytes(
      commitmentAlias.value,
      `${context}.commitment`,
      32,
    ),
    bit_length: normalizeVeRangeBitLength(
      bitLengthAlias.value,
      `${context}.bitLength`,
    ),
    aggregation_count: normalizeVeRangeAggregationCount(
      aggregationAlias.value,
      `${context}.aggregationCount`,
    ),
    commitment_scheme: normalizeVeRangeCommitmentScheme(
      schemeAlias.value,
      `${context}.commitmentScheme`,
    ),
    domain_separator: assertNonBlankString(
      domainAlias.value ?? VERANGE_DOMAIN_SEPARATOR,
      `${context}.domainSeparator`,
    ),
    payload_digest: normalizeVeRangePayloadDigest(source, context),
  };
}

/**
 * Build canonical Norito bytes for a prepared VeRange proof envelope.
 *
 * This accepts externally generated proof bytes and binds them to normalized
 * range commitments. It intentionally does not expose a fake prover/verifier.
 * @param {object} options
 * @returns {Buffer}
 */
export function buildVeRangeProofEnvelope(options) {
  const source = assertPlainObject(options, "veRangeProofEnvelope");
  assertAllowedFields(
    source,
    new Set([
      "backend",
      "backendTag",
      "backend_tag",
      "circuitId",
      "circuit_id",
      "vkHash",
      "vk_hash",
      "verifierKeyHash",
      "verifyingKeyHash",
      "commitments",
      "rangeCommitments",
      "range_commitments",
      "commitment",
      "rangeCommitment",
      "range_commitment",
      "valueCommitment",
      "value_commitment",
      "bitLength",
      "bit_length",
      "aggregationCount",
      "aggregation_count",
      "commitmentScheme",
      "commitment_scheme",
      "domainSeparator",
      "domain_separator",
      "payloadDigest",
      "payload_digest",
      "txDigest",
      "tx_digest",
      "payload",
      "payloadBytes",
      "payload_bytes",
      "payloadJson",
      "payload_json",
      "proofBytes",
      "proof_bytes",
      "proof",
      "aux",
      "maxProofBytes",
      "max_proof_bytes",
      "maxPublicInputBytes",
      "max_public_input_bytes",
      "maxPayloadBytes",
      "max_payload_bytes",
      "version",
    ]),
    "veRangeProofEnvelope",
  );
  const parts = normalizeVeRangeProofEnvelopeParts(
    source,
    "veRangeProofEnvelope",
  );
  return buildPrivacyProofEnvelopeWithOptionalLimits({
    backend: parts.backend,
    circuitId: parts.circuitId,
    vkHash: parts.vkHash,
    publicInputs: parts.publicInputBytes,
    proofBytes: parts.proofBytes,
    aux: source.aux,
    maxProofBytes: parts.maxProofBytes,
    maxPublicInputBytes: parts.maxPublicInputBytes,
  });
}

/**
 * Build a deterministic VeRange dev proof fixture bound to an OpenVerify envelope.
 *
 * The returned proof bytes are only for SDK/validator fixture tests; they are
 * not a VeRange production proof.
 * @param {object} options
 * @returns {object}
 */
export function buildVeRangeDevProofFixture(options) {
  const source = assertPlainObject(options, "veRangeDevProofFixture");
  assertAllowedFields(
    source,
    new Set([
      "backend",
      "backendTag",
      "backend_tag",
      "circuitId",
      "circuit_id",
      "vkHash",
      "vk_hash",
      "verifierKeyHash",
      "verifyingKeyHash",
      "commitments",
      "rangeCommitments",
      "range_commitments",
      "commitment",
      "rangeCommitment",
      "range_commitment",
      "valueCommitment",
      "value_commitment",
      "bitLength",
      "bit_length",
      "aggregationCount",
      "aggregation_count",
      "commitmentScheme",
      "commitment_scheme",
      "domainSeparator",
      "domain_separator",
      "payloadDigest",
      "payload_digest",
      "txDigest",
      "tx_digest",
      "payload",
      "payloadBytes",
      "payload_bytes",
      "payloadJson",
      "payload_json",
      "aux",
      "maxProofBytes",
      "max_proof_bytes",
      "maxPublicInputBytes",
      "max_public_input_bytes",
      "maxPayloadBytes",
      "max_payload_bytes",
      "version",
    ]),
    "veRangeDevProofFixture",
  );
  const parts = normalizeVeRangeProofEnvelopeParts(
    source,
    "veRangeDevProofFixture",
    { requireProofBytes: false },
  );
  const proofBytes = veRangeDevProofBytes(parts);
  const envelope = buildPrivacyProofEnvelopeWithOptionalLimits({
    backend: parts.backend,
    circuitId: parts.circuitId,
    vkHash: parts.vkHash,
    publicInputs: parts.publicInputBytes,
    proofBytes,
    aux: source.aux,
    maxProofBytes: parts.maxProofBytes,
    maxPublicInputBytes: parts.maxPublicInputBytes,
  });
  return {
    kind: "verange-dev-fixture-v1",
    production: false,
    proof_bytes: proofBytes,
    proofBytes: Buffer.from(proofBytes),
    public_inputs: parts.publicInputs,
    publicInputBytes: Buffer.from(parts.publicInputBytes),
    envelope,
  };
}

/**
 * Verify a deterministic VeRange dev proof fixture through OpenVerifyEnvelope bytes.
 *
 * This verifier is intentionally limited to the SDK dev-fixture format and
 * must not be treated as a production VeRange proof verifier.
 * @param {object|Buffer|string} options
 * @returns {object}
 */
export function verifyVeRangeProofLocally(options) {
  const source =
    options &&
    typeof options === "object" &&
    !Array.isArray(options) &&
    !Buffer.isBuffer(options) &&
    !ArrayBuffer.isView(options) &&
    !(options instanceof ArrayBuffer)
      ? assertPlainObject(options, "veRangeProofLocalVerification")
      : { envelope: options };
  assertAllowedFields(
    source,
    new Set([
      "envelope",
      "proofEnvelope",
      "proof_envelope",
      "bytes",
      "payloadDigest",
      "payload_digest",
      "txDigest",
      "tx_digest",
      "payload",
      "payloadBytes",
      "payload_bytes",
      "payloadJson",
      "payload_json",
      "commitments",
      "rangeCommitments",
      "range_commitments",
      "commitment",
      "rangeCommitment",
      "range_commitment",
      "valueCommitment",
      "value_commitment",
      "bitLength",
      "bit_length",
      "aggregationCount",
      "aggregation_count",
      "commitmentScheme",
      "commitment_scheme",
      "domainSeparator",
      "domain_separator",
      "maxPayloadBytes",
      "max_payload_bytes",
    ]),
    "veRangeProofLocalVerification",
  );
  const envelopeAlias = readSingleAlias(
    source,
    ["envelope", "proofEnvelope", "proof_envelope", "bytes"],
    "veRangeProofLocalVerification.envelope",
    "proof envelope",
  );
  const decoded = decodeVeRangeEnvelope(
    envelopeAlias.value,
    "veRangeProofLocalVerification.envelope",
  );
  if (decoded.backend !== "Stark") {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "veRangeProofLocalVerification.envelope.backend must be Stark",
      "veRangeProofLocalVerification.envelope.backend",
    );
  }
  const circuitId = normalizeVeRangeCircuitId(
    decoded.circuit_id,
    "veRangeProofLocalVerification.envelope.circuitId",
  );
  const vkHash = normalizeNonZeroFixedBytes(
    decoded.vk_hash,
    "veRangeProofLocalVerification.envelope.vkHash",
    32,
  );
  const publicInputs = parseVeRangePublicInputs(
    decoded.public_inputs,
    "veRangeProofLocalVerification.publicInputs",
  );
  ensureVeRangeVerificationExpectations(
    source,
    publicInputs,
    "veRangeProofLocalVerification",
  );
  const expectedProofBytes = veRangeDevProofBytes({
    circuitId,
    vkHash,
    publicInputBytes: decoded.public_inputs,
  });
  if (!fixedBytesEqual(decoded.proof_bytes, expectedProofBytes)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "veRangeProofLocalVerification proof bytes are not a valid VeRange dev fixture",
      "veRangeProofLocalVerification.proofBytes",
    );
  }
  return {
    ok: true,
    production: false,
    kind: "verange-dev-fixture-v1",
    backend: VERANGE_BACKEND,
    circuit_id: circuitId,
    verifier_key_hash: Buffer.from(vkHash).toString("hex"),
    public_inputs: publicInputs,
    public_input_bytes: decoded.public_inputs.length,
    proof_bytes: decoded.proof_bytes.length,
    aux_bytes: decoded.aux.length,
  };
}

/**
 * Build canonical Norito bytes for an open-verify proof envelope.
 * @param {object} options
 * @returns {Buffer}
 */
export function buildPrivacyProofEnvelope(options) {
  return buildPrivacyProofEnvelopeInternal(options);
}

function buildPrivacyProofEnvelopeInternal(
  options,
  { allowUnsupportedBackend = false } = {},
) {
  const source = assertPlainObject(options, "privacyProofEnvelope");
  assertAllowedFields(
    source,
    new Set([
      "backend",
      "backendTag",
      "backend_tag",
      "circuitId",
      "circuit_id",
      "vkHash",
      "vk_hash",
      "verifierKeyHash",
      "verifyingKeyHash",
      "publicInputs",
      "public_inputs",
      "proofBytes",
      "proof_bytes",
      "proof",
      "aux",
      "maxProofBytes",
      "max_proof_bytes",
      "maxPublicInputBytes",
      "max_public_input_bytes",
    ]),
    "privacyProofEnvelope",
  );
  const backendAlias = readSingleAlias(
    source,
    ["backendTag", "backend_tag", "backend"],
    "privacyProofEnvelope.backendTag",
    "backend tag",
  );
  const circuitAlias = readSingleAlias(
    source,
    ["circuitId", "circuit_id"],
    "privacyProofEnvelope.circuitId",
    "circuit id",
  );
  const vkHashAlias = readSingleAlias(
    source,
    ["vkHash", "vk_hash", "verifierKeyHash", "verifyingKeyHash"],
    "privacyProofEnvelope.vkHash",
    "verifying key hash",
  );
  const publicInputsAlias = readSingleAlias(
    source,
    ["publicInputs", "public_inputs"],
    "privacyProofEnvelope.publicInputs",
    "public inputs",
  );
  const proofAlias = readSingleAlias(
    source,
    ["proofBytes", "proof_bytes", "proof"],
    "privacyProofEnvelope.proofBytes",
    "proof bytes",
  );
  const maxProofAlias = readSingleAlias(
    source,
    ["maxProofBytes", "max_proof_bytes"],
    "privacyProofEnvelope.maxProofBytes",
    "max proof byte limit",
  );
  const maxPublicInputAlias = readSingleAlias(
    source,
    ["maxPublicInputBytes", "max_public_input_bytes"],
    "privacyProofEnvelope.maxPublicInputBytes",
    "max public input byte limit",
  );
  const maxProofBytes = normalizePositiveU32(
    maxProofAlias.key === null
      ? DEFAULT_PRIVACY_MAX_PROOF_BYTES
      : maxProofAlias.value,
    "privacyProofEnvelope.maxProofBytes",
  );
  const maxPublicInputBytes = normalizePositiveU32(
    maxPublicInputAlias.key === null
      ? DEFAULT_PRIVACY_MAX_PUBLIC_INPUT_BYTES
      : maxPublicInputAlias.value,
    "privacyProofEnvelope.maxPublicInputBytes",
  );
  const circuitId = assertString(
    circuitAlias.value,
    "privacyProofEnvelope.circuitId",
  );
  if (circuitId.trim().length === 0) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      "privacyProofEnvelope.circuitId must be a non-empty string",
      "privacyProofEnvelope.circuitId",
    );
  }
  if (circuitId.trim() !== circuitId) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      "privacyProofEnvelope.circuitId must be clean and already trimmed",
      "privacyProofEnvelope.circuitId",
    );
  }
  const backend = normalizePrivacyBackendTag(
    backendAlias.value,
    "privacyProofEnvelope.backendTag",
  );
  if (backend === "Unsupported" && !allowUnsupportedBackend) {
    fail(
      ValidationErrorCode.INVALID_STRING,
      "privacyProofEnvelope.backendTag uses unsupported privacy verifier backend unsupported",
      "privacyProofEnvelope.backendTag",
    );
  }
  const envelope = {
    backend,
    circuit_id: circuitId,
    vk_hash: normalizeOpenVerifyFixedBytes(
      vkHashAlias.value,
      "privacyProofEnvelope.vkHash",
      32,
      { nonzero: true },
    ),
    public_inputs: normalizeOpenVerifyByteArray(
      publicInputsAlias.value,
      "privacyProofEnvelope.publicInputs",
      { maxBytes: maxPublicInputBytes },
    ),
    proof_bytes: normalizeOpenVerifyByteArray(
      proofAlias.value,
      "privacyProofEnvelope.proofBytes",
      { maxBytes: maxProofBytes },
    ),
    aux: normalizeOpenVerifyByteArray(source.aux, "privacyProofEnvelope.aux", {
      maxBytes: DEFAULT_PRIVACY_MAX_AUX_BYTES,
      allowEmpty: true,
    }),
  };
  return noritoEncodePrivacyProofEnvelope(envelope);
}

function buildPrivacyProofEnvelopeWithOptionalLimits(options) {
  const envelopeOptions = { ...options };
  if (envelopeOptions.maxProofBytes === undefined) {
    delete envelopeOptions.maxProofBytes;
  }
  if (envelopeOptions.maxPublicInputBytes === undefined) {
    delete envelopeOptions.maxPublicInputBytes;
  }
  return buildPrivacyProofEnvelopeInternal(envelopeOptions, {
    allowUnsupportedBackend:
      envelopeOptions.backend === JINDO_BACKEND ||
      envelopeOptions.backend === SIS_HINTS_BACKEND,
  });
}

/**
 * Build a verifier-key registration instruction for the privacy verifier WSV registry.
 * @param {object} options
 * @returns {{verifying_keys: {RegisterVerifyingKey: object}}}
 */
export function buildRegisterPrivacyVerifierKeyInstruction(options) {
  const source = assertPlainObject(options, "registerPrivacyVerifierKey");
  const id = normalizePrivacyVerifierKeyIdFromOptions(
    source,
    "registerPrivacyVerifierKey",
  );
  const recordAlias = readSingleAlias(
    source,
    ["record", "verifierRecord", "verifyingKeyRecord"],
    "registerPrivacyVerifierKey.record",
    "verifier key record",
  );
  const record = normalizePrivacyVerifierKeyRecord(
    recordAlias.key === null ? source : recordAlias.value,
    id,
    "registerPrivacyVerifierKey.record",
    { defaultStatus: "Proposed", allowWithdrawn: false },
  );
  return {
    verifying_keys: {
      RegisterVerifyingKey: {
        id,
        record,
      },
    },
  };
}

/**
 * Build a verifier-key retirement instruction by updating the record status to Withdrawn.
 * @param {object} options
 * @returns {{verifying_keys: {UpdateVerifyingKey: object}}}
 */
export function buildRetirePrivacyVerifierKeyInstruction(options) {
  const source = assertPlainObject(options, "retirePrivacyVerifierKey");
  const id = normalizePrivacyVerifierKeyIdFromOptions(source, "retirePrivacyVerifierKey");
  const recordAlias = readSingleAlias(
    source,
    ["record", "verifierRecord", "verifyingKeyRecord"],
    "retirePrivacyVerifierKey.record",
    "verifier key record",
  );
  const record = normalizePrivacyVerifierKeyRecord(
    recordAlias.key === null ? source : recordAlias.value,
    id,
    "retirePrivacyVerifierKey.record",
    { defaultStatus: "Withdrawn", allowWithdrawn: true, forceStatus: "Withdrawn" },
  );
  return {
    verifying_keys: {
      UpdateVerifyingKey: {
        id,
        record,
      },
    },
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
 * Build a `zk::RegisterAssetHiddenZkPool` instruction payload.
 * @param {object} options
 * @returns {{zk: {RegisterAssetHiddenZkPool: object}}}
 */
export function buildRegisterAssetHiddenZkPoolInstruction(options) {
  const source = assertPlainObject(options, "registerAssetHiddenZkPool");
  const poolId = readSingleAlias(
    source,
    ["poolId", "pool_id", "assetPoolId", "asset_pool_id"],
    "registerAssetHiddenZkPool.poolId",
    "pool id",
  );
  const storageAsset = readSingleAlias(
    source,
    ["storageAsset", "storage_asset", "storageAssetDefinitionId", "storage_asset_definition_id"],
    "registerAssetHiddenZkPool.storageAsset",
    "storage asset",
  );
  const assetSetRoot = readSingleAlias(
    source,
    ["assetSetRoot", "asset_set_root"],
    "registerAssetHiddenZkPool.assetSetRoot",
    "asset-set root",
  );
  const vkTransfer = readSingleAlias(
    source,
    ["transferVerifyingKey", "vkTransfer", "vk_transfer", "verifyingKeyRef", "verifying_key_ref"],
    "registerAssetHiddenZkPool.vkTransfer",
    "transfer verifier",
  );
  const normalizedRoot = normalizeFixedBytes(
    assetSetRoot.value,
    "registerAssetHiddenZkPool.assetSetRoot",
    32,
  );
  if (normalizedRoot.every((byte) => byte === 0)) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "registerAssetHiddenZkPool.assetSetRoot must be nonzero",
      "registerAssetHiddenZkPool.assetSetRoot",
    );
  }
  const payload = {
    pool_id: assertNonBlankString(
      poolId.value,
      "registerAssetHiddenZkPool.poolId",
    ),
    storage_asset: assertString(
      storageAsset.value,
      "registerAssetHiddenZkPool.storageAsset",
    ),
    asset_set_root: normalizedRoot,
    vk_transfer: normalizeVerifyingKeyId(
      vkTransfer.value,
      "registerAssetHiddenZkPool.vkTransfer",
    ),
  };
  if (payload.vk_transfer === null) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "registerAssetHiddenZkPool.vkTransfer is required",
      "registerAssetHiddenZkPool.vkTransfer",
    );
  }
  return {
    zk: {
      RegisterAssetHiddenZkPool: payload,
    },
  };
}

/**
 * Build a `zk::RegisterZkAceIdentityCommitment` instruction payload.
 * @param {object} options
 * @returns {{zk: {RegisterZkAceIdentityCommitment: object}}}
 */
export function buildRegisterZkAceIdentityCommitmentInstruction(options) {
  const source = assertPlainObject(options, "registerZkAceIdentityCommitment");
  const payload = {
    asset: assertString(
      source.assetDefinitionId ?? source.asset_definition_id ?? source.asset,
      "registerZkAceIdentityCommitment.asset",
    ),
    identity_commitment: normalizeNonZeroFixedBytes(
      source.identityCommitment ?? source.identity_commitment,
      "registerZkAceIdentityCommitment.identityCommitment",
      32,
    ),
    policy_hash: normalizeNonZeroFixedBytes(
      source.policyHash ?? source.policy_hash,
      "registerZkAceIdentityCommitment.policyHash",
      32,
    ),
    allowed_accounts: normalizeZkAceAllowedAccounts(
      source.allowedAccounts ?? source.allowed_accounts,
      "registerZkAceIdentityCommitment.allowedAccounts",
    ),
    action_class: normalizeZkAceAction(
      source.actionClass ?? source.action_class,
      "registerZkAceIdentityCommitment.actionClass",
    ),
    domain_tag: normalizeZkAceDomainTag(
      source.domainTag ?? source.domain_tag,
      "registerZkAceIdentityCommitment.domainTag",
    ),
    verifier_key: normalizeZkAceVerifierKeyId(
      source.verifierKey ?? source.verifier_key ?? source.verifyingKeyRef,
      "registerZkAceIdentityCommitment.verifierKey",
    ),
  };
  return {
    zk: {
      RegisterZkAceIdentityCommitment: payload,
    },
  };
}

/**
 * Build a `zk::RotateZkAceIdentityCommitment` instruction payload.
 * @param {object} options
 * @returns {{zk: {RotateZkAceIdentityCommitment: object}}}
 */
export function buildRotateZkAceIdentityCommitmentInstruction(options) {
  const source = assertPlainObject(options, "rotateZkAceIdentityCommitment");
  const oldCommitment = normalizeNonZeroFixedBytes(
    source.oldIdentityCommitment ?? source.old_identity_commitment,
    "rotateZkAceIdentityCommitment.oldIdentityCommitment",
    32,
  );
  const newCommitment = normalizeNonZeroFixedBytes(
    source.newIdentityCommitment ?? source.new_identity_commitment,
    "rotateZkAceIdentityCommitment.newIdentityCommitment",
    32,
  );
  if (Buffer.from(oldCommitment).equals(Buffer.from(newCommitment))) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "rotateZkAceIdentityCommitment replacement commitment must differ from the old commitment",
      "rotateZkAceIdentityCommitment.newIdentityCommitment",
    );
  }
  const payload = {
    asset: assertString(
      source.assetDefinitionId ?? source.asset_definition_id ?? source.asset,
      "rotateZkAceIdentityCommitment.asset",
    ),
    old_identity_commitment: oldCommitment,
    new_identity_commitment: newCommitment,
    policy_hash: normalizeNonZeroFixedBytes(
      source.policyHash ?? source.policy_hash,
      "rotateZkAceIdentityCommitment.policyHash",
      32,
    ),
    allowed_accounts: normalizeZkAceAllowedAccounts(
      source.allowedAccounts ?? source.allowed_accounts,
      "rotateZkAceIdentityCommitment.allowedAccounts",
    ),
    action_class: normalizeZkAceAction(
      source.actionClass ?? source.action_class,
      "rotateZkAceIdentityCommitment.actionClass",
    ),
    domain_tag: normalizeZkAceDomainTag(
      source.domainTag ?? source.domain_tag,
      "rotateZkAceIdentityCommitment.domainTag",
    ),
    verifier_key: normalizeZkAceVerifierKeyId(
      source.verifierKey ?? source.verifier_key ?? source.verifyingKeyRef,
      "rotateZkAceIdentityCommitment.verifierKey",
    ),
  };
  return {
    zk: {
      RotateZkAceIdentityCommitment: payload,
    },
  };
}

/**
 * Build a `zk::RevokeZkAceIdentityCommitment` instruction payload.
 * @param {object} options
 * @returns {{zk: {RevokeZkAceIdentityCommitment: object}}}
 */
export function buildRevokeZkAceIdentityCommitmentInstruction(options) {
  const source = assertPlainObject(options, "revokeZkAceIdentityCommitment");
  return {
    zk: {
      RevokeZkAceIdentityCommitment: {
        asset: assertString(
          source.assetDefinitionId ?? source.asset_definition_id ?? source.asset,
          "revokeZkAceIdentityCommitment.asset",
        ),
        identity_commitment: normalizeNonZeroFixedBytes(
          source.identityCommitment ?? source.identity_commitment,
          "revokeZkAceIdentityCommitment.identityCommitment",
          32,
        ),
        reason_hash: normalizeOptionalFixedBytes(
          source.reasonHash ?? source.reason_hash,
          "revokeZkAceIdentityCommitment.reasonHash",
          32,
        ),
      },
    },
  };
}

/**
 * Normalize a prepared ZK-ACE authorization proof attachment and its public inputs.
 * The native Rust prover crate produces the STARK/FRI envelope bytes consumed here.
 * @param {object} options
 * @returns {{public_inputs: object, proof: object}}
 */
export function buildZkAceAuthorizationProofV1(options) {
  const source = assertPlainObject(options, "zkAceAuthorizationProof");
  const publicInputs = normalizeZkAcePublicInputs(
    source.publicInputs ?? source.public_inputs,
    "zkAceAuthorizationProof.publicInputs",
  );
  const witnessSource = source.witness ?? source.privateWitness ?? source.private_witness;
  const hasPreparedProof =
    source.proof !== undefined ||
    source.proofBytes !== undefined ||
    source.proof_bytes !== undefined ||
    source.proofAttachment !== undefined ||
    source.proof_attachment !== undefined ||
    source.attachment !== undefined;
  if (witnessSource !== undefined && witnessSource !== null && !hasPreparedProof) {
    const witness = normalizeZkAceWitness(witnessSource, "zkAceAuthorizationProof.witness");
    const native = getNativeBinding();
    if (!native || typeof native.zkAceBuildAuthorizationProofV1 !== "function") {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        "zkAceAuthorizationProof witness proving requires the iroha_js_host native ZK-ACE prover",
        "zkAceAuthorizationProof.witness",
      );
    }
    const commitment = normalizeOptionalFixedBytes(
      source.verifyingKeyCommitment ??
        source.verifying_key_commitment ??
        source.vkCommitment ??
        source.vk_commitment,
      "zkAceAuthorizationProof.verifyingKeyCommitment",
      32,
    );
    let nativeResultJson;
    let nativeError;
    try {
      nativeResultJson = native.zkAceBuildAuthorizationProofV1(
        JSON.stringify(publicInputs),
        JSON.stringify(witness),
        commitment ? Buffer.from(commitment) : undefined,
      );
    } catch (error) {
      nativeError = error;
    }
    if (nativeError !== undefined) {
      sanitizeZkAceNativeAuthorizationProofError(
        nativeError,
        "zkAceAuthorizationProof.nativeResult",
      );
    }
    const nativeResult = normalizeZkAceNativeResult(
      nativeResultJson,
      "zkAceAuthorizationProof.nativeResult",
    );
    const proof = normalizeProofAttachment(
      nativeResult.proof,
      "zkAceAuthorizationProof.proof",
    );
    const provedPublicInputs = normalizeZkAcePublicInputs(
      nativeResult.public_inputs ?? nativeResult.publicInputs ?? publicInputs,
      "zkAceAuthorizationProof.publicInputs",
    );
    if (
      proof.vk_ref.backend !== provedPublicInputs.verifier_key_id.backend ||
      proof.vk_ref.name !== provedPublicInputs.verifier_key_id.name
    ) {
      fail(
        ValidationErrorCode.INVALID_OBJECT,
        "zkAceAuthorizationProof proof verifier must match public inputs",
        "zkAceAuthorizationProof.proof.verifyingKeyRef",
      );
    }
    return {
      public_inputs: provedPublicInputs,
      proof,
    };
  }
  const directProof = source.proof;
  const directProofIsAttachment =
    directProof &&
    typeof directProof === "object" &&
    !Array.isArray(directProof) &&
    !Buffer.isBuffer(directProof) &&
    !ArrayBuffer.isView(directProof) &&
    !(directProof instanceof ArrayBuffer) &&
    Object.prototype.hasOwnProperty.call(directProof, "backend");
  const proofSource =
    source.proofAttachment ??
    source.proof_attachment ??
    source.attachment ??
    (directProofIsAttachment ? directProof : undefined) ??
    {
      backend: ZK_ACE_BACKEND,
      proofBytes: source.proofBytes ?? source.proof_bytes ?? source.proof,
      verifyingKeyRef:
        source.verifyingKeyRef ??
        source.verifying_key_ref ??
        publicInputs.verifier_key_id,
      verifyingKeyCommitment:
        source.verifyingKeyCommitment ??
        source.verifying_key_commitment ??
        source.vkCommitment ??
        source.vk_commitment,
      envelopeHash: source.envelopeHash ?? source.envelope_hash,
    };
  const proof = normalizeProofAttachment(proofSource, "zkAceAuthorizationProof.proof");
  if (proof.backend !== ZK_ACE_BACKEND) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      `zkAceAuthorizationProof.proof.backend must be ${ZK_ACE_BACKEND}`,
      "zkAceAuthorizationProof.proof.backend",
    );
  }
  if (
    proof.vk_ref.backend !== publicInputs.verifier_key_id.backend ||
    proof.vk_ref.name !== publicInputs.verifier_key_id.name
  ) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "zkAceAuthorizationProof proof verifier must match public inputs",
      "zkAceAuthorizationProof.proof.verifyingKeyRef",
    );
  }
  return {
    public_inputs: publicInputs,
    proof,
  };
}

/**
 * Build a `zk::SubmitZkAceAuthorizedTransfer` instruction payload.
 * @param {object} options
 * @returns {{zk: {SubmitZkAceAuthorizedTransfer: object}}}
 */
export function buildZkAceAuthorizedTransferInstruction(options) {
  const source = assertPlainObject(options, "zkAceAuthorizedTransfer");
  const proofBundle = source.authorizationProof ?? source.authorization_proof;
  const authorization =
    proofBundle && typeof proofBundle === "object" && !Array.isArray(proofBundle)
      ? buildZkAceAuthorizationProofV1(proofBundle)
      : null;
  const proof =
    authorization !== null
      ? authorization.proof
      : normalizeProofAttachment(source.proof, "zkAceAuthorizedTransfer.proof");
  const payload = {
    from: normalizeAccountId(source.fromAccountId ?? source.from, "zkAceAuthorizedTransfer.from"),
    to: normalizeAccountId(source.toAccountId ?? source.to, "zkAceAuthorizedTransfer.to"),
    asset: assertString(
      source.assetDefinitionId ?? source.asset_definition_id ?? source.asset,
      "zkAceAuthorizedTransfer.asset",
    ),
    amount: asPositiveU128JsonNumber(source.amount, "zkAceAuthorizedTransfer.amount"),
    identity_commitment: normalizeNonZeroFixedBytes(
      source.identityCommitment ?? source.identity_commitment,
      "zkAceAuthorizedTransfer.identityCommitment",
      32,
    ),
    tx_digest: normalizeNonZeroFixedBytes(
      source.txDigest ?? source.tx_digest,
      "zkAceAuthorizedTransfer.txDigest",
      32,
    ),
    chain_id: assertNonBlankString(
      source.chainId ?? source.chain_id,
      "zkAceAuthorizedTransfer.chainId",
    ),
    domain_tag: normalizeZkAceDomainTag(
      source.domainTag ?? source.domain_tag,
      "zkAceAuthorizedTransfer.domainTag",
    ),
    action_class: normalizeZkAceAction(
      source.actionClass ?? source.action_class,
      "zkAceAuthorizedTransfer.actionClass",
    ),
    replay_nullifier: normalizeNonZeroFixedBytes(
      source.replayNullifier ?? source.replay_nullifier,
      "zkAceAuthorizedTransfer.replayNullifier",
      32,
    ),
    policy_hash: normalizeNonZeroFixedBytes(
      source.policyHash ?? source.policy_hash,
      "zkAceAuthorizedTransfer.policyHash",
      32,
    ),
    proof,
  };
  if (authorization !== null) {
    ensureZkAceAuthorizationMatchesTransfer(
      authorization.public_inputs,
      payload,
      "zkAceAuthorizedTransfer.authorizationProof",
    );
  }
  return {
    zk: {
      SubmitZkAceAuthorizedTransfer: payload,
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
 * Build an experimental `zk::AssetHiddenZkTransfer` instruction payload.
 *
 * This helper exposes the SDK request shape for asset-hidden private transfers.
 * The Rust data model and Norito wire encoder accept the instruction, while the
 * validator execution path remains fail-closed until pool verifier state exists.
 *
 * @param {object} options
 * @returns {{zk: {AssetHiddenZkTransfer: object}}}
 */
export function buildAssetHiddenZkTransferInstruction(options) {
  const source = assertPlainObject(options, "assetHiddenZkTransfer");
  const poolId = readSingleAlias(
    source,
    ["poolId", "pool_id", "assetPoolId", "asset_pool_id"],
    "assetHiddenZkTransfer.poolId",
    "pool id",
  );
  const rootHint = readSingleAlias(
    source,
    ["rootHint", "root_hint"],
    "assetHiddenZkTransfer.rootHint",
    "root hint",
  );
  const inputs = Array.isArray(source.inputs)
    ? source.inputs.map((entry, index) =>
        normalizeFixedBytes(entry, `assetHiddenZkTransfer.inputs[${index}]`, 32),
      )
    : [];
  const outputs = Array.isArray(source.outputs)
    ? source.outputs.map((entry, index) =>
        normalizeFixedBytes(entry, `assetHiddenZkTransfer.outputs[${index}]`, 32),
      )
    : [];
  if (inputs.length === 0) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "assetHiddenZkTransfer.inputs must contain at least one nullifier",
    );
  }
  if (outputs.length === 0) {
    fail(
      ValidationErrorCode.INVALID_OBJECT,
      "assetHiddenZkTransfer.outputs must contain at least one commitment",
    );
  }
  const payload = {
    pool_id: assertNonBlankString(
      poolId.value,
      "assetHiddenZkTransfer.poolId",
    ),
    inputs,
    outputs,
    proof: normalizeProofAttachment(source.proof, "assetHiddenZkTransfer.proof"),
    root_hint: normalizeOptionalFixedBytes(
      rootHint.value,
      "assetHiddenZkTransfer.rootHint",
    ),
  };
  return {
    zk: {
      AssetHiddenZkTransfer: payload,
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
  const outputs = Array.isArray(source.outputs)
    ? source.outputs.map((entry, index) =>
        normalizeFixedBytes(entry, `unshield.outputs[${index}]`, 32),
      )
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
    outputs,
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
