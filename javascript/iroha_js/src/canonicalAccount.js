import { Buffer } from "buffer";

import { parseCanonicalI105AccountLiteral } from "./address.js";
import { CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1 } from "./canonicalLimits.js";

function isCanonicalAuthAliasSegment(value) {
  return (
    value.length >= 1 &&
    value.length <= 63 &&
    /^[a-z0-9_-]+$/u.test(value) &&
    !value.startsWith("-") &&
    !value.endsWith("-") &&
    (value.slice(2, 4) !== "--" || value.startsWith("xn--"))
  );
}

function isCanonicalAccountAlias(value) {
  if (value.startsWith("0x")) {
    return false;
  }
  const separator = value.indexOf("@");
  if (separator <= 0 || separator !== value.lastIndexOf("@")) {
    return false;
  }
  const label = value.slice(0, separator);
  const scopeParts = value.slice(separator + 1).split(".");
  return (
    !label.includes(".") &&
    scopeParts.length >= 1 &&
    scopeParts.length <= 2 &&
    scopeParts.every(Boolean) &&
    [label, ...scopeParts].every(isCanonicalAuthAliasSegment)
  );
}

/** Validate the exact account identity carried by canonical request auth. */
export function requireCanonicalAuthAccount(value, context) {
  if (typeof value !== "string" || value.length === 0 || value.trim() !== value) {
    throw new TypeError(
      `${context} must be an exact canonical I105 account or ASCII account alias`,
    );
  }
  if (Buffer.byteLength(value, "utf8") > CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1) {
    throw new TypeError(
      `${context} exceeds ${CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1} UTF-8 bytes`,
    );
  }
  if (isCanonicalAccountAlias(value)) {
    return value;
  }
  try {
    parseCanonicalI105AccountLiteral(value);
    return value;
  } catch {
    // Use the single stable diagnostic below for every malformed identifier.
  }
  throw new TypeError(
    `${context} must be an exact canonical I105 account or ASCII account alias`,
  );
}

/** Render the account header after {@link requireCanonicalAuthAccount}. */
export function canonicalAuthAccountHeaderValue(accountId) {
  return isCanonicalAccountAlias(accountId)
    ? accountId
    : parseCanonicalI105AccountLiteral(accountId).canonicalHex;
}
