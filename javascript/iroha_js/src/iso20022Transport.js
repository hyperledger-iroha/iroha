import {
  createValidationError,
  ValidationErrorCode,
} from "./validationError.js";

const ISO_STATUS_VALUES = new Map([
  ["pending", "Pending"],
  ["accepted", "Accepted"],
  ["rejected", "Rejected"],
  ["committed", "Committed"],
]);
const PACS002_STATUS_CODES = new Set(["ACTC", "ACSP", "ACSC", "ACWC", "PDNG", "RJCT"]);

function requireIsoNonEmptyString(value, context) {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${context} must be a non-empty string`,
      context,
    );
  }
  return value;
}

/** Normalize a Torii ISO lifecycle status label. */
export function normalizeIsoStatus(value, context) {
  const status = requireIsoNonEmptyString(value, context).trim();
  const normalized = ISO_STATUS_VALUES.get(status.toLowerCase());
  if (!normalized) {
    throw new TypeError(
      `${context} must be one of ${[...ISO_STATUS_VALUES.values()].join(", ")}`,
    );
  }
  return normalized;
}

/** Normalize an ISO pacs.002 status code. */
export function normalizePacs002Code(value, context) {
  const code = requireIsoNonEmptyString(value, context).trim().toUpperCase();
  if (!PACS002_STATUS_CODES.has(code)) {
    throw new TypeError(
      `${context} must be one of ${[...PACS002_STATUS_CODES.values()].join(", ")}`,
    );
  }
  return code;
}

/** Normalize an optional ISO response string. */
export function normalizeIsoOptionalString(value, name, options = {}) {
  if (value === undefined || value === null) {
    return null;
  }
  if (typeof value !== "string") {
    throw new TypeError(`${name} must be a string or null`);
  }
  const trimmed = value.trim();
  if (!trimmed && !options.allowEmpty) {
    return null;
  }
  return trimmed;
}

/** Normalize a list of ISO response strings. */
export function normalizeIsoStringArray(value, name) {
  if (value === undefined || value === null) {
    return [];
  }
  if (!Array.isArray(value)) {
    throw new TypeError(`${name} must be an array of strings`);
  }
  return value.map((entry, index) => {
    if (typeof entry !== "string") {
      throw new TypeError(`${name}[${index}] must be a string`);
    }
    const normalized = entry.trim();
    if (!normalized) {
      throw new TypeError(`${name}[${index}] must be a non-empty string`);
    }
    return normalized;
  });
}

/** Require the catalog's exact lowercase profile identifier spelling. */
export function normalizeIsoProfile(value, path) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.trim() !== value ||
    !/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/u.test(value)
  ) {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${path} must be a canonical lowercase profile id`,
      path,
    );
  }
  return value;
}

/** One-shot ISO 20022 submission transport shared by the public pacs methods. */
export async function submitIsoTransport(client, kind, body, options) {
  const response = await client._request("POST", `/v1/iso20022/${kind}`, {
    headers: {
      "Content-Type": options.contentType ?? "application/xml",
      Accept: "application/json",
    },
    body,
    params: options.profile ? { profile: options.profile } : undefined,
    signal: options.signal,
    retryProfile: options.retryProfile,
    operatorSigningContext: options.operatorSigningContext,
    requireIsoOperatorAuth: true,
  });
  await client._expectStatus(response, [202]);
  return client._maybeJson(response);
}

/** One-shot ISO 20022 status transport; every call receives a fresh signature. */
export async function getIsoStatusTransport(client, messageId, options) {
  const response = await client._request(
    "GET",
    `/v1/iso20022/messages/${encodeURIComponent(messageId)}`,
    {
      headers: { Accept: "application/json" },
      signal: options.signal,
      retryProfile: options.retryProfile,
      operatorSigningContext: options.operatorSigningContext,
      requireIsoOperatorAuth: true,
    },
  );
  await client._expectStatus(response, [200]);
  return client._maybeJson(response);
}
