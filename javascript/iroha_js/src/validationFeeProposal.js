import { Buffer } from "node:buffer";

import { getNativeBinding } from "./native.js";

const LOWER_HEX_32 = /^[0-9a-f]{64}$/u;

function nativeFunction(name, rustName) {
  const native = globalThis.__IROHA_NATIVE_BINDING__ ?? getNativeBinding();
  if (typeof native?.[name] !== "function") {
    throw new Error(`native binding '${rustName}' is unavailable`);
  }
  return native[name].bind(native);
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

function exactJsonObject(value, name) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${name} must be an exact native object`);
  }
  let encoded;
  try {
    encoded = JSON.stringify(value);
  } catch (error) {
    throw new TypeError(
      `${name} must be JSON-serializable: ${error?.message ?? error}`,
    );
  }
  if (typeof encoded !== "string") {
    throw new TypeError(`${name} must be an exact native object`);
  }
  return encoded;
}

function exactProposalOperator(value) {
  if (typeof value !== "string" || value.length === 0 || value.trim() !== value) {
    throw new TypeError(
      "proposalOperator must be one canonical domainless AccountId",
    );
  }
  return value;
}

function exactFingerprint(value, proposalName) {
  const fingerprint = Buffer.from(value);
  if (fingerprint.length !== 32) {
    throw new Error(
      `native ${proposalName} proposal fingerprint must contain exactly 32 bytes`,
    );
  }
  return fingerprint.toString("hex");
}

/**
 * Compute the exact native Parliament fingerprint for a validation-fee policy.
 *
 * The policy must use the native snake-case `ValidationFeePolicyV1` JSON
 * contract. Native validation rejects missing, unknown, and legacy fields.
 *
 * @param {string} proposalOperator canonical domainless transaction authority
 * @param {Record<string, unknown>} policy
 * @param {string | null} [payoutLifecycleProposalId]
 * @returns {string} lowercase 32-byte proposal fingerprint
 */
export function computeValidationFeePolicyProposalFingerprintV1(
  proposalOperator,
  policy,
  payoutLifecycleProposalId = null,
) {
  if (arguments.length < 2 || arguments.length > 3) {
    throw new TypeError(
      "computeValidationFeePolicyProposalFingerprintV1 accepts exactly two or three arguments",
    );
  }
  const fingerprint = nativeFunction(
    "validationFeePolicyProposalFingerprintV1",
    "validation_fee_policy_proposal_fingerprint_v1",
  )(
    exactProposalOperator(proposalOperator),
    exactJsonObject(policy, "policy"),
    exactLifecycleProposalId(payoutLifecycleProposalId),
  );
  return exactFingerprint(fingerprint, "validation-fee policy");
}

/**
 * Compute the exact native Parliament fingerprint for a validation-fee payout lifecycle.
 *
 * The payout binding must use its exact native snake-case JSON contract.
 * Native validation rejects missing, unknown, legacy, and non-canonical fields.
 *
 * @param {string} proposalOperator canonical domainless transaction authority
 * @param {Record<string, unknown>} payoutBinding
 * @returns {string} lowercase 32-byte proposal fingerprint
 */
export function computeValidationFeePayoutLifecycleProposalFingerprintV1(
  proposalOperator,
  payoutBinding,
) {
  if (arguments.length !== 2) {
    throw new TypeError(
      "computeValidationFeePayoutLifecycleProposalFingerprintV1 accepts exactly two arguments",
    );
  }
  const fingerprint = nativeFunction(
    "validationFeePayoutLifecycleProposalFingerprintV1",
    "validation_fee_payout_lifecycle_proposal_fingerprint_v1",
  )(
    exactProposalOperator(proposalOperator),
    exactJsonObject(payoutBinding, "payoutBinding"),
  );
  return exactFingerprint(fingerprint, "validation-fee payout lifecycle");
}
