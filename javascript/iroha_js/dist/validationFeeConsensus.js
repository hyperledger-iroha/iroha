import { Buffer } from "node:buffer";

import { getNativeBinding } from "./native.js";

export const VALIDATION_FEE_LEDGER_BINDING_SCHEMA =
  "cbsi.mobile-validation-fee-ledger-binding.v1";
export const VALIDATION_FEE_VERIFIED_POLICY_PROJECTION_SCHEMA =
  "iroha.validation_fee.verified_policy_projection.v1";
export const VALIDATION_FEE_CURRENT_POLICY_PROOF_PATH =
  "/v1/validation-fee/policy/current/proof";
export const VALIDATION_FEE_POLICY_PROOF_MAX_RESPONSE_BYTES = 4 * 1024 * 1024;
export const VALIDATION_FEE_REQUIRED_BRIDGE_ABI_VERSION = 21;

const LOWER_HEX_32 = /^[0-9a-f]{64}$/u;
const BINDING_KEYS = Object.freeze([
  "chainId",
  "checkpoint",
  "genesisHash",
  "policyChainGenesisHash",
  "schema",
]);
const CHECKPOINT_KEYS = Object.freeze(["contextId", "height"]);
const PROJECTION_KEYS = Object.freeze([
  "chain_id",
  "current_policy",
  "evaluated_block_hash",
  "evaluated_block_height",
  "evaluated_context_id",
  "genesis_hash",
  "head_policy_hash",
  "head_policy_version",
  "more_available",
  "observed_ledger_tip_height",
  "policy_chain_genesis_hash",
  "registry_hash",
  "schema",
  "trusted_checkpoint_context_id",
  "trusted_checkpoint_height",
  "version",
]);

function record(value, label) {
  if (
    value === null ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    Object.getPrototypeOf(value) !== Object.prototype
  ) {
    throw new TypeError(`${label} must be a plain object`);
  }
  return value;
}

function exactKeys(value, expected, label) {
  const keys = Object.keys(value).sort();
  if (
    keys.length !== expected.length ||
    keys.some((key, index) => key !== expected[index])
  ) {
    throw new TypeError(`${label} must contain exactly ${expected.join(", ")}`);
  }
}

function lowerHex32(value, label) {
  if (typeof value !== "string" || !LOWER_HEX_32.test(value)) {
    throw new TypeError(`${label} must be exactly 64 lowercase hexadecimal digits`);
  }
  if (/^0+$/u.test(value)) {
    throw new TypeError(`${label} must be non-zero`);
  }
  return value;
}

function positiveU64(value, label) {
  let parsed;
  if (typeof value === "bigint") {
    parsed = value;
  } else if (typeof value === "number" && Number.isSafeInteger(value)) {
    parsed = BigInt(value);
  } else if (typeof value === "string" && /^[1-9][0-9]*$/u.test(value)) {
    parsed = BigInt(value);
  } else {
    throw new TypeError(`${label} must be a positive uint64`);
  }
  if (parsed <= 0n || parsed > 0xffff_ffff_ffff_ffffn) {
    throw new TypeError(`${label} must be a positive uint64`);
  }
  return parsed;
}

function exactChainId(value, label) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > 256 ||
    value.trim() !== value ||
    /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    throw new TypeError(`${label} must be canonical bounded text`);
  }
  return value;
}

/** Parse the exact immutable CBSI deployment binding. */
export function normalizeValidationFeeLedgerBindingV1(value) {
  const binding = record(value, "validation-fee ledger binding");
  exactKeys(binding, BINDING_KEYS, "validation-fee ledger binding");
  if (binding.schema !== VALIDATION_FEE_LEDGER_BINDING_SCHEMA) {
    throw new TypeError(
      `validation-fee ledger binding.schema must be ${VALIDATION_FEE_LEDGER_BINDING_SCHEMA}`,
    );
  }
  const checkpoint = normalizeValidationFeeCheckpointV1(binding.checkpoint);
  return Object.freeze({
    schema: binding.schema,
    chainId: exactChainId(binding.chainId, "validation-fee ledger binding.chainId"),
    genesisHash: lowerHex32(
      binding.genesisHash,
      "validation-fee ledger binding.genesisHash",
    ),
    policyChainGenesisHash: lowerHex32(
      binding.policyChainGenesisHash,
      "validation-fee ledger binding.policyChainGenesisHash",
    ),
    checkpoint,
  });
}

/** Normalize one durable checkpoint used for page promotion. */
export function normalizeValidationFeeCheckpointV1(value) {
  const checkpoint = record(value, "validation-fee checkpoint");
  exactKeys(checkpoint, CHECKPOINT_KEYS, "validation-fee checkpoint");
  return Object.freeze({
    height: positiveU64(checkpoint.height, "validation-fee checkpoint.height"),
    contextId: lowerHex32(
      checkpoint.contextId,
      "validation-fee checkpoint.contextId",
    ),
  });
}

function nativeBinding() {
  const native = globalThis.__IROHA_NATIVE_BINDING__ ?? getNativeBinding();
  if (
    typeof native?.connectNoritoBridgeAbiVersion !== "function" ||
    native.connectNoritoBridgeAbiVersion() !==
      VALIDATION_FEE_REQUIRED_BRIDGE_ABI_VERSION ||
    typeof native?.validationFeeCurrentPolicyProofRequestV1 !== "function" ||
    typeof native?.validationFeeVerifyCurrentPolicyProofV1 !== "function"
  ) {
    throw new Error(
      `native binding lacks the ABI ${VALIDATION_FEE_REQUIRED_BRIDGE_ABI_VERSION} validation-fee consensus proof verifier`,
    );
  }
  return native;
}

/** Encode the exact Norito V1 proof request for `checkpoint`. */
export function encodeValidationFeeCurrentPolicyProofRequestV1(checkpoint) {
  const normalized = normalizeValidationFeeCheckpointV1(checkpoint);
  const encoded = nativeBinding().validationFeeCurrentPolicyProofRequestV1(
    normalized.height,
    Buffer.from(normalized.contextId, "hex"),
  );
  if (!encoded || encoded.length === 0) {
    throw new Error("native validation-fee request encoder returned no bytes");
  }
  return Buffer.from(encoded);
}

function projectionHeight(value, label) {
  return positiveU64(value, label);
}

function freezeProjection(value) {
  const stack = [value];
  let visited = 0;
  while (stack.length > 0) {
    const next = stack.pop();
    if (next === null || typeof next !== "object" || Object.isFrozen(next)) continue;
    visited += 1;
    if (visited > 100_000) {
      throw new TypeError("validation-fee projection exceeds the object bound");
    }
    for (const child of Object.values(next)) stack.push(child);
    Object.freeze(next);
  }
  return value;
}

/**
 * Locally verify one canonical Norito proof page and return its immutable
 * policy projection. The native verifier performs all consensus cryptography.
 */
export function verifyValidationFeeCurrentPolicyProofV1(
  proofNorito,
  bindingValue,
  checkpointValue,
) {
  const binding = normalizeValidationFeeLedgerBindingV1(bindingValue);
  const checkpoint = normalizeValidationFeeCheckpointV1(checkpointValue);
  const proof = Buffer.from(proofNorito ?? []);
  if (
    proof.length === 0 ||
    proof.length > VALIDATION_FEE_POLICY_PROOF_MAX_RESPONSE_BYTES
  ) {
    throw new TypeError(
      `proofNorito must contain 1..${VALIDATION_FEE_POLICY_PROOF_MAX_RESPONSE_BYTES} bytes`,
    );
  }
  const json = nativeBinding().validationFeeVerifyCurrentPolicyProofV1(
    proof,
    binding.chainId,
    Buffer.from(binding.genesisHash, "hex"),
    Buffer.from(binding.policyChainGenesisHash, "hex"),
    checkpoint.height,
    Buffer.from(checkpoint.contextId, "hex"),
  );
  if (typeof json !== "string" || json.length === 0) {
    throw new Error("native validation-fee verifier returned no projection");
  }
  const projection = record(JSON.parse(json), "validation-fee verified projection");
  exactKeys(
    projection,
    PROJECTION_KEYS,
    "validation-fee verified projection",
  );
  const projectedTrustedCheckpointHeight = projectionHeight(
    projection.trusted_checkpoint_height,
    "validation-fee projection.trusted_checkpoint_height",
  );
  if (
    projection.schema !== VALIDATION_FEE_VERIFIED_POLICY_PROJECTION_SCHEMA ||
    projection.version !== 1 ||
    projection.chain_id !== binding.chainId ||
    projection.genesis_hash !== binding.genesisHash ||
    projection.policy_chain_genesis_hash !== binding.policyChainGenesisHash ||
    projection.trusted_checkpoint_context_id !== checkpoint.contextId ||
    projectedTrustedCheckpointHeight !== checkpoint.height
  ) {
    throw new TypeError(
      "validation-fee verified projection differs from its immutable binding or checkpoint",
    );
  }
  const normalized = {
    ...projection,
    head_policy_version: projectionHeight(
      projection.head_policy_version,
      "validation-fee projection.head_policy_version",
    ),
    trusted_checkpoint_height: projectedTrustedCheckpointHeight,
    evaluated_block_height: projectionHeight(
      projection.evaluated_block_height,
      "validation-fee projection.evaluated_block_height",
    ),
    observed_ledger_tip_height: projectionHeight(
      projection.observed_ledger_tip_height,
      "validation-fee projection.observed_ledger_tip_height",
    ),
  };
  lowerHex32(
    normalized.evaluated_context_id,
    "validation-fee projection.evaluated_context_id",
  );
  lowerHex32(
    normalized.evaluated_block_hash,
    "validation-fee projection.evaluated_block_hash",
  );
  lowerHex32(
    normalized.registry_hash,
    "validation-fee projection.registry_hash",
  );
  lowerHex32(
    normalized.head_policy_hash,
    "validation-fee projection.head_policy_hash",
  );
  if (typeof normalized.more_available !== "boolean") {
    throw new TypeError(
      "validation-fee projection.more_available must be boolean",
    );
  }
  return freezeProjection(normalized);
}
