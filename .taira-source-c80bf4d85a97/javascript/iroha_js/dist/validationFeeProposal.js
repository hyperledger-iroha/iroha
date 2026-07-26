import { Buffer } from "node:buffer";

import { getNativeBinding } from "./native.js";

const LOWER_HEX_32 = /^[0-9a-f]{64}$/u;

function nativeBinding() {
  const native = globalThis.__IROHA_NATIVE_BINDING__ ?? getNativeBinding();
  if (
    typeof native?.validationFeePolicyProposalFingerprintV1 !== "function"
  ) {
    throw new Error(
      "native binding 'validation_fee_policy_proposal_fingerprint_v1' is unavailable",
    );
  }
  return native;
}

function exactLifecycleProposalId(value) {
  if (value === null) {
    return null;
  }
  if (typeof value !== "string" || !LOWER_HEX_32.test(value)) {
    throw new TypeError(
      "payoutLifecycleProposalId must be null or exactly 64 lowercase hexadecimal digits",
    );
  }
  const bytes = Buffer.from(value, "hex");
  if (bytes.every((byte) => byte === 0)) {
    throw new TypeError("payoutLifecycleProposalId must be non-zero");
  }
  return bytes;
}

/**
 * Compute the exact native Parliament fingerprint for a validation-fee policy.
 *
 * The policy must use the native snake-case `ValidationFeePolicyV1` JSON
 * contract. Native validation rejects missing, unknown, and legacy fields.
 *
 * @param {Record<string, unknown>} policy
 * @param {string | null} [payoutLifecycleProposalId]
 * @returns {string} lowercase 32-byte proposal fingerprint
 */
export function computeValidationFeePolicyProposalFingerprintV1(
  policy,
  payoutLifecycleProposalId = null,
) {
  if (
    policy === null ||
    typeof policy !== "object" ||
    Array.isArray(policy)
  ) {
    throw new TypeError("policy must be an exact native ValidationFeePolicyV1 object");
  }
  let policyJson;
  try {
    policyJson = JSON.stringify(policy);
  } catch (error) {
    throw new TypeError(`policy must be JSON-serializable: ${error?.message ?? error}`);
  }
  if (typeof policyJson !== "string") {
    throw new TypeError("policy must be an exact native ValidationFeePolicyV1 object");
  }
  const fingerprint = Buffer.from(
    nativeBinding().validationFeePolicyProposalFingerprintV1(
      policyJson,
      exactLifecycleProposalId(payoutLifecycleProposalId),
    ),
  );
  if (fingerprint.length !== 32) {
    throw new Error(
      "native validation-fee proposal fingerprint must contain exactly 32 bytes",
    );
  }
  return fingerprint.toString("hex");
}
