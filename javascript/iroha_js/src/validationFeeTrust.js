import { Buffer } from "node:buffer";
import { networkIdBytes } from "./networkId.js";
import { snapshotBoundedBytes as boundedBytes } from "./boundedByteSnapshot.js";

export const VALIDATION_FEE_LEDGER_BINDING_SCHEMA =
  "iroha.validation-fee-ledger-binding.v1";
const LOWER_HEX_32 = /^[0-9a-f]{64}$/u;
const BINDING_KEYS = Object.freeze([
  "checkpoint",
  "networkId",
  "policyChainGenesisHash",
  "schema",
]);
// Mirrors the sole bounded native checkpoint codec: two 32 MiB frames plus 4 MiB context.
const MAX_CHECKPOINT_BYTES = 68 * 1024 * 1024;
const CHECKPOINT_KEYS = Object.freeze(["checkpointNorito"]);
export function record(value, label) {
  if (
    value === null ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    (
      Object.getPrototypeOf(value) !== Object.prototype &&
      Object.getPrototypeOf(value) !== null
    )
  ) {
    throw new TypeError(`${label} must be a plain object`);
  }
  return value;
}

export function exactKeys(value, expected, label) {
  const keys = Object.keys(value).sort();
  const expectedKeys = [...expected].sort();
  if (
    keys.length !== expectedKeys.length ||
    keys.some((key, index) => key !== expectedKeys[index])
  ) {
    throw new TypeError(`${label} must contain exactly ${expected.join(", ")}`);
  }
}

export function lowerHex32(value, label) {
  if (typeof value !== "string" || !LOWER_HEX_32.test(value)) {
    throw new TypeError(`${label} must be exactly 64 lowercase hexadecimal digits`);
  }
  if (/^0+$/u.test(value)) {
    throw new TypeError(`${label} must be non-zero`);
  }
  return value;
}

export function irohaHash32(value, label) {
  const normalized = lowerHex32(value, label);
  if ((Number.parseInt(normalized.slice(-2), 16) & 1) === 0) {
    throw new TypeError(`${label} must carry the canonical Iroha hash marker`);
  }
  return normalized;
}

/** Validate the exact immutable Iroha deployment binding. */
export function normalizeValidationFeeLedgerBindingV1(value) {
  const binding = record(value, "validation-fee ledger binding");
  exactKeys(binding, BINDING_KEYS, "validation-fee ledger binding");
  const { schema, checkpoint: selectedCheckpoint, networkId, policyChainGenesisHash } = binding;
  if (schema !== VALIDATION_FEE_LEDGER_BINDING_SCHEMA) {
    throw new TypeError(
      `validation-fee ledger binding.schema must be ${VALIDATION_FEE_LEDGER_BINDING_SCHEMA}`,
    );
  }
  const checkpoint = normalizeValidationFeeCheckpointV1(selectedCheckpoint);
  networkIdBytes(networkId, "validation-fee ledger binding.networkId");
  return Object.freeze({
    schema,
    networkId,
    policyChainGenesisHash: irohaHash32(
      policyChainGenesisHash,
      "validation-fee ledger binding.policyChainGenesisHash",
    ),
    checkpoint,
  });
}

/** Retain full independently selected canonical native checkpoint bytes for page promotion.
 * Canonical decoding and native verification occur in the native owner, never in JavaScript.
 */
export function normalizeValidationFeeCheckpointV1(value) {
  const checkpoint = record(value, "validation-fee checkpoint");
  exactKeys(checkpoint, CHECKPOINT_KEYS, "validation-fee checkpoint");
  const bytes = boundedBytes(checkpoint.checkpointNorito, "validation-fee checkpoint.checkpointNorito", MAX_CHECKPOINT_BYTES);
  // Neither the caller's original view nor a returned view can mutate the retained binding.
  return Object.freeze({ get checkpointNorito() { return Buffer.from(bytes); } });
}
