import { ensureCanonicalAccountId } from "./normalizers.js";
import { createValidationError, ValidationErrorCode } from "./validationError.js";

const CANONICAL_HEADERS = new Set([
  "x-iroha-account",
  "x-iroha-nonce",
  "x-iroha-signature",
  "x-iroha-timestamp-ms",
  "x-iroha-witness",
]);

export function normalizeCanonicalApplicationPostOptions(options, context, Client, defaultAuth = null) {
  const { signal, rest } = Client._normalizeOptionsWithSignal(options, context);
  const requestedAuth = rest.canonicalAuth === undefined ? defaultAuth : rest.canonicalAuth;
  const canonicalAuth = Client._normalizeCanonicalAuth(requestedAuth, `${context}.canonicalAuth`);
  if (!canonicalAuth) {
    throw createValidationError(ValidationErrorCode.INVALID_OBJECT, `${context} options.canonicalAuth is required`, `${context}.canonicalAuth`);
  }
  const exactAccount = ensureCanonicalAccountId(canonicalAuth.accountId, `${context}.canonicalAuth.accountId`);
  if (exactAccount !== canonicalAuth.accountId) {
    throw new TypeError(`${context}.canonicalAuth.accountId must be an exact canonical I105 account id`);
  }
  const { canonicalAuth: _ignoredCanonical, ...payloadOptions } = rest;
  return { signal, canonicalAuth, rest: payloadOptions };
}

export function requireCanonicalApplicationAuthority(canonicalAuth, value, context) {
  const authority = ensureCanonicalAccountId(value, `${context}.authority`);
  if (authority !== value || canonicalAuth.accountId !== authority) {
    throw new TypeError(`${context} canonicalAuth.accountId must equal the exact canonical I105 authority`);
  }
  return authority;
}

export function rejectPrecomputedCanonicalHeaders(headers) {
  if (Object.keys(headers).some((name) => CANONICAL_HEADERS.has(name.toLowerCase()))) {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      "canonical signing headers must be generated locally and cannot be precomputed",
      "request.headers",
    );
  }
}
