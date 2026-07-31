/**
 * Node-capability normalization and transaction data-model compatibility
 * probing for the Node Torii client.
 *
 * This module is implementation-internal: the public error continues to be
 * re-exported from `toriiClient.js`, while ToriiClient retains ownership of
 * HTTP transport, cancellation, and request ordering.
 */

import {
  createValidationError,
  ValidationError,
  ValidationErrorCode,
} from "./validationError.js";

/** Maximum decoded JSON bytes accepted from the node-capabilities endpoint. */
export const NODE_CAPABILITIES_JSON_MAX_BYTES = 1024 * 1024;

const EXPECTED_DATA_MODEL_VERSION = 4;
const MAX_SAFE_INTEGER = Number.MAX_SAFE_INTEGER;
const MAX_SAFE_INTEGER_BIGINT = BigInt(MAX_SAFE_INTEGER);

/**
 * Raised before transaction admission when Torii advertises an incompatible
 * data-model version.
 */
export class ToriiDataModelMismatchError extends Error {
  constructor(expected, actual, cause) {
    const actualLabel = actual == null ? "missing" : String(actual);
    super(`Torii data model version mismatch (expected ${expected}, got ${actualLabel}).`);
    this.name = "ToriiDataModelMismatchError";
    this.expected = expected;
    this.actual = actual ?? null;
    if (cause !== undefined) {
      this.cause = cause;
    }
  }
}

/**
 * Cache and coalesce the node-capability probe required before transaction
 * admission. The state fields remain owned by ToriiClient so its internal
 * object shape and compatibility cache lifecycle remain unchanged.
 */
export function ensureNodeDataModelCompatibility(state, fetchCapabilities) {
  const expected = EXPECTED_DATA_MODEL_VERSION;
  if (state._dataModelValidation.status === "matched") {
    return;
  }
  if (state._dataModelValidation.status === "mismatched") {
    throw new ToriiDataModelMismatchError(expected, state._dataModelValidation.actual);
  }
  if (state._dataModelValidationPromise) {
    return state._dataModelValidationPromise;
  }
  const promise = (async () => {
    let capabilities;
    try {
      capabilities = await fetchCapabilities();
    } catch (error) {
      if (error instanceof ValidationError) {
        state._dataModelValidation = { status: "mismatched", actual: null };
        throw new ToriiDataModelMismatchError(expected, null, error);
      }
      throw error;
    }
    const actual = capabilities.dataModelVersion;
    if (actual !== expected) {
      state._dataModelValidation = { status: "mismatched", actual };
      throw new ToriiDataModelMismatchError(expected, actual);
    }
    state._dataModelValidation = { status: "matched", actual };
  })();
  state._dataModelValidationPromise = promise;
  promise
    .finally(() => {
      if (state._dataModelValidationPromise === promise) {
        state._dataModelValidationPromise = null;
      }
    })
    .catch(() => {
      // Avoid unhandled rejections from the cleanup chain.
    });
  return promise;
}

/** Normalize the canonical `/v1/node/capabilities` response. */
export function normalizeNodeCapabilitiesResponse(payload) {
  const record = ensureRecord(payload, "node capabilities response");
  const cryptoRecord = ensureRecord(record.crypto ?? {}, "node capabilities response.crypto");
  const curvesRecord = cryptoRecord.curves ?? {};
  return {
    abiVersion: normalizeUnsignedInteger(
      record.abi_version,
      "node capabilities response.abi_version",
      { allowZero: false },
    ),
    dataModelVersion: normalizeUnsignedInteger(
      record.data_model_version,
      "node capabilities response.data_model_version",
      { allowZero: false },
    ),
    crypto: {
      sm: normalizeNodeSmCapabilities(cryptoRecord.sm, "node capabilities response.crypto.sm"),
      curves: normalizeNodeCurveCapabilities(
        curvesRecord,
        "node capabilities response.crypto.curves",
      ),
    },
  };
}

function normalizeNodeSmCapabilities(value, context) {
  const record = ensureRecord(value ?? {}, context);
  return {
    enabled: coerceBoolean(record.enabled, `${context}.enabled`),
    defaultHash: optionalString(record.default_hash, `${context}.default_hash`),
    allowedSigning: parseStringArray(
      record.allowed_signing,
      `${context}.allowed_signing`,
    ),
    sm2DistIdDefault: optionalString(
      record.sm2_distid_default,
      `${context}.sm2_distid_default`,
    ),
    opensslPreview: coerceBoolean(record.openssl_preview ?? false, `${context}.openssl_preview`),
    acceleration: normalizeNodeSmAcceleration(record.acceleration, `${context}.acceleration`),
  };
}

function normalizeNodeSmAcceleration(value, context) {
  const record = ensureRecord(value ?? {}, context);
  return {
    scalar: coerceBoolean(record.scalar ?? true, `${context}.scalar`),
    neonSm3: coerceBoolean(record.neon_sm3 ?? false, `${context}.neon_sm3`),
    neonSm4: coerceBoolean(record.neon_sm4 ?? false, `${context}.neon_sm4`),
    policy: requireNonEmptyString(record.policy ?? "", `${context}.policy`),
  };
}

function normalizeNodeCurveCapabilities(value, context) {
  const record = ensureRecord(value ?? {}, context);
  const rawVersion = record.registry_version;
  const registryVersion =
    rawVersion === undefined || rawVersion === null
      ? 1
      : normalizeUnsignedInteger(rawVersion, `${context}.registry_version`, {
          allowZero: false,
        });
  const allowedRaw = record.allowed_curve_ids ?? [];
  const bitmapRaw = record.allowed_curve_bitmap ?? [];
  return {
    registryVersion,
    allowedCurveIds: parseIntegerArray(allowedRaw, `${context}.allowed_curve_ids`),
    allowedCurveBitmap: parseIntegerArray(bitmapRaw, `${context}.allowed_curve_bitmap`),
  };
}

function normalizeUnsignedInteger(value, name, options = {}) {
  const allowZero = Boolean(options.allowZero);
  const min = options.min;
  const max = options.max;
  let numeric;
  if (typeof value === "number") {
    if (!Number.isFinite(value) || !Number.isInteger(value)) {
      const qualifier = allowZero ? "non-negative integer" : "positive integer";
      throw createValidationError(
        ValidationErrorCode.INVALID_NUMERIC,
        `${name} must be a ${qualifier}`,
        name,
      );
    }
    if (value < 0 || (!allowZero && value === 0)) {
      const qualifier = allowZero ? "non-negative integer" : "positive integer";
      throw createValidationError(
        ValidationErrorCode.INVALID_NUMERIC,
        `${name} must be a ${qualifier}`,
        name,
      );
    }
    if (!Number.isSafeInteger(value)) {
      throw createValidationError(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name} must be at most ${MAX_SAFE_INTEGER}`,
        name,
      );
    }
    numeric = value;
  } else if (typeof value === "bigint") {
    if (value < 0n || (!allowZero && value === 0n)) {
      const qualifier = allowZero ? "non-negative integer" : "positive integer";
      throw createValidationError(
        ValidationErrorCode.INVALID_NUMERIC,
        `${name} must be a ${qualifier}`,
        name,
      );
    }
    if (value > MAX_SAFE_INTEGER_BIGINT) {
      throw createValidationError(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name} must be at most ${MAX_SAFE_INTEGER}`,
        name,
      );
    }
    numeric = Number(value);
  } else if (typeof value === "string") {
    const trimmed = value.trim();
    if (!/^[0-9]+$/.test(trimmed)) {
      const qualifier = allowZero ? "non-negative integer" : "positive integer";
      throw createValidationError(
        ValidationErrorCode.INVALID_NUMERIC,
        `${name} must be a ${qualifier}`,
        name,
      );
    }
    const bigint = BigInt(trimmed);
    if (bigint < 0n || (!allowZero && bigint === 0n)) {
      const qualifier = allowZero ? "non-negative integer" : "positive integer";
      throw createValidationError(
        ValidationErrorCode.INVALID_NUMERIC,
        `${name} must be a ${qualifier}`,
        name,
      );
    }
    if (bigint > MAX_SAFE_INTEGER_BIGINT) {
      throw createValidationError(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name} must be at most ${MAX_SAFE_INTEGER}`,
        name,
      );
    }
    numeric = Number(bigint);
  } else {
    const qualifier = allowZero ? "non-negative" : "positive";
    throw createValidationError(
      ValidationErrorCode.INVALID_NUMERIC,
      `${name} must be a ${qualifier} integer`,
      name,
    );
  }
  if (numeric === 0) {
    return 0;
  }
  if (min !== undefined && numeric < min) {
    const qualifier = allowZero ? `at least ${min} or zero` : `at least ${min}`;
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} must be ${qualifier}`,
      name,
    );
  }
  if (max !== undefined && numeric > max) {
    throw createValidationError(
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      `${name} must be at most ${max}`,
      name,
    );
  }
  return Math.floor(numeric);
}

function parseIntegerArray(value, context) {
  if (value === undefined || value === null) {
    return [];
  }
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array`);
  }
  return value.map((entry, index) => {
    const numeric = coerceIntegerLike(entry, `${context}[${index}]`);
    if (numeric < 0) {
      throw new RangeError(`${context}[${index}] must be non-negative`);
    }
    return numeric;
  });
}

function coerceIntegerLike(value, context) {
  if (typeof value === "number") {
    if (!Number.isFinite(value) || !Number.isSafeInteger(value)) {
      throw new RangeError(`${context} must be a safe integer`);
    }
    return value;
  }
  if (typeof value === "bigint") {
    const numeric = Number(value);
    if (!Number.isSafeInteger(numeric)) {
      throw new RangeError(`${context} must be a safe integer`);
    }
    return numeric;
  }
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (!trimmed) {
      throw new TypeError(`${context} must be an integer`);
    }
    const numeric = Number(trimmed);
    if (!Number.isFinite(numeric) || !Number.isSafeInteger(numeric)) {
      throw new RangeError(`${context} must be a safe integer`);
    }
    return numeric;
  }
  throw new TypeError(`${context} must be an integer`);
}

function ensureRecord(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return value;
}

function isPlainObject(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const proto = Object.getPrototypeOf(value);
  return proto === Object.prototype || proto === null;
}

function coerceBoolean(value, context) {
  if (value === undefined || value === null || value === "") {
    return false;
  }
  if (typeof value === "boolean") {
    return value;
  }
  if (value === 1 || value === "1") {
    return true;
  }
  if (value === 0 || value === "0") {
    return false;
  }
  if (typeof value === "string") {
    const lower = value.toLowerCase();
    if (lower === "true") {
      return true;
    }
    if (lower === "false") {
      return false;
    }
  }
  throw new TypeError(`${context} must be boolean`);
}

function parseStringArray(value, context) {
  if (value == null) {
    return [];
  }
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array of strings`);
  }
  return value.map((entry, index) => {
    if (entry === undefined || entry === null || typeof entry !== "string") {
      throw new TypeError(`${context}[${index}] must be a string`);
    }
    return entry;
  });
}

function optionalString(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  if (typeof value === "string") {
    return value;
  }
  throw new TypeError(`${context} must be a string when present`);
}

function requireNonEmptyString(value, name) {
  if (typeof value !== "string") {
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
