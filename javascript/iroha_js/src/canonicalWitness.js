import { Buffer } from "buffer";

import { CANONICAL_REQUEST_MAX_WITNESS_BYTES_V1 } from "./canonicalLimits.js";

const MAX_ENCODED_WITNESS_BYTES_V1 =
  4 * Math.ceil(CANONICAL_REQUEST_MAX_WITNESS_BYTES_V1 / 3);
const PADDED_STANDARD_BASE64 =
  /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u;

function invalidWitness(context) {
  return new TypeError(
    `${context} must be exact standard-base64 with padding within the V1 witness limit`,
  );
}

/** Validate and return an exact padded-base64 V1 witness header. */
export function normalizeCanonicalWitnessHeader(value, context) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > MAX_ENCODED_WITNESS_BYTES_V1 ||
    !PADDED_STANDARD_BASE64.test(value)
  ) {
    throw invalidWitness(context);
  }
  const decoded = Buffer.from(value, "base64");
  if (
    decoded.length > CANONICAL_REQUEST_MAX_WITNESS_BYTES_V1 ||
    decoded.toString("base64") !== value
  ) {
    throw invalidWitness(context);
  }
  return value;
}
