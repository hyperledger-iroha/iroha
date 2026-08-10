import { getNativeBinding } from "./native.js";
import { networkIdBytes } from "./networkId.js";

function resolveNativeBinding() {
  // Allow tests to inject a fake binding.
  return globalThis.__IROHA_NATIVE_BINDING__ ?? getNativeBinding();
}

function requireRecord(value, context) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return value;
}

function rejectRetiredFields(record, fields, context) {
  for (const [field, canonical] of fields) {
    if (Object.prototype.hasOwnProperty.call(record, field)) {
      throw new TypeError(
        `${context}.${field} is retired; use canonical ${canonical}`,
      );
    }
  }
}

function normalizeInlineVerifyingKeyRecord(value, context) {
  const detail = requireRecord(value, `${context}.verifyingKey`);
  rejectRetiredFields(
    detail,
    [
      ["inlineKey", "verifyingKey.record.inline_key"],
      ["inline_key", "verifyingKey.record.inline_key"],
      ["bytesBase64", "verifyingKey.record.inline_key.bytes_b64"],
      ["bytes_b64", "verifyingKey.record.inline_key.bytes_b64"],
      ["backend", "verifyingKey.id.backend"],
      ["circuitId", "verifyingKey.record.circuit_id"],
      ["circuit_id", "verifyingKey.record.circuit_id"],
    ],
    `${context}.verifyingKey`,
  );
  const id = requireRecord(detail.id, `${context}.verifyingKey.id`);
  const record = requireRecord(detail.record, `${context}.verifyingKey.record`);
  rejectRetiredFields(
    record,
    [
      ["circuitId", "verifyingKey.record.circuit_id"],
      ["inlineKey", "verifyingKey.record.inline_key"],
      ["bytesBase64", "verifyingKey.record.inline_key.bytes_b64"],
      ["bytes_b64", "verifyingKey.record.inline_key.bytes_b64"],
    ],
    `${context}.verifyingKey.record`,
  );
  const inlineKey = requireRecord(
    record.inline_key,
    `${context}.verifyingKey.record.inline_key`,
  );
  rejectRetiredFields(
    inlineKey,
    [["bytesBase64", "verifyingKey.record.inline_key.bytes_b64"]],
    `${context}.verifyingKey.record.inline_key`,
  );
  const idBackend = normalizeExactMetadataString(
    id.backend,
    `${context}.verifyingKey.id.backend`,
  );
  const recordBackend = normalizeExactMetadataString(
    record.backend,
    `${context}.verifyingKey.record.backend`,
  );
  const inlineBackend = normalizeExactMetadataString(
    inlineKey.backend,
    `${context}.verifyingKey.record.inline_key.backend`,
  );
  if (idBackend !== recordBackend || idBackend !== inlineBackend) {
    throw new TypeError(`${context}.verifyingKey backend fields must match exactly`);
  }
  const circuitId = normalizeExactMetadataString(
    record.circuit_id,
    `${context}.verifyingKey.record.circuit_id`,
  );
  return {
    backend: idBackend,
    circuitId,
    bytes: normalizeExactBase64Bytes(
      inlineKey.bytes_b64,
      `${context}.verifyingKey.record.inline_key.bytes_b64`,
    ),
  };
}

function normalizeExactBase64Bytes(value, context) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u.test(
      value,
    )
  ) {
    throw new TypeError(`${context} must be canonical non-empty base64`);
  }
  const bytes = Buffer.from(value, "base64");
  if (bytes.length === 0 || bytes.toString("base64") !== value) {
    throw new TypeError(`${context} must be canonical non-empty base64`);
  }
  return bytes;
}

function normalizeExactMetadataString(value, context) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a string`);
  }
  if (!value.trim()) {
    throw new TypeError(`${context} must be present`);
  }
  if (value.trim() !== value) {
    throw new TypeError(`${context} must not contain surrounding whitespace`);
  }
  return value;
}

function normalizeWholeNumberLiteral(value, context) {
  if (value === undefined || value === null) {
    throw new TypeError(`${context} must be a whole-number string`);
  }
  const normalized = String(value);
  if (normalized.trim() !== normalized) {
    throw new TypeError(`${context} must not contain surrounding whitespace`);
  }
  if (!/^\d+$/.test(normalized)) {
    throw new TypeError(`${context} must be a whole-number string`);
  }
  return normalized;
}

function normalizeFixed32HexLiteral(value, context) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)) {
    throw new TypeError(`${context} must be exactly 64 lowercase hex characters`);
  }
  return value;
}

function normalizeFixed32BinaryLike(value, context) {
  if (typeof value === "string") {
    return normalizeFixed32HexLiteral(value, context);
  }
  const buffer = toNamedBuffer(value, context);
  if (buffer.length !== 32) {
    throw new TypeError(`${context} must be 32 bytes`);
  }
  return Buffer.from(buffer).toString("hex");
}

function normalizeLeafIndex(value, context) {
  if (
    typeof value !== "number" ||
    !Number.isInteger(value) ||
    value < 0 ||
    value > 0xffff_ffff
  ) {
    throw new TypeError(`${context} must be an unsigned 32-bit integer`);
  }
  return value;
}

function normalizeConfidentialInput(value, index) {
  const context = `inputs[${index}]`;
  const input = requireRecord(value, context);
  rejectRetiredFields(
    input,
    [
      ["rho", "rhoHex"],
      ["diversifier_hex", "diversifierHex"],
      ["diversifier", "diversifierHex"],
      ["leaf_index", "leafIndex"],
    ],
    context,
  );
  return {
    amount: normalizeWholeNumberLiteral(input.amount, `${context}.amount`),
    rhoHex: normalizeFixed32HexLiteral(input.rhoHex, `${context}.rhoHex`),
    diversifierHex: normalizeFixed32HexLiteral(
      input.diversifierHex,
      `${context}.diversifierHex`,
    ),
    leafIndex: normalizeLeafIndex(input.leafIndex, `${context}.leafIndex`),
  };
}

function normalizeConfidentialOutput(value, index, ownerTagRequired) {
  const context = `outputs[${index}]`;
  const output = requireRecord(value, context);
  const retiredFields = [["rho", "rhoHex"]];
  if (ownerTagRequired) {
    retiredFields.push(
      ["owner_tag_hex", "ownerTagHex"],
      ["ownerTag", "ownerTagHex"],
    );
  }
  rejectRetiredFields(output, retiredFields, context);
  const normalized = {
    amount: normalizeWholeNumberLiteral(output.amount, `${context}.amount`),
    rhoHex: normalizeFixed32HexLiteral(output.rhoHex, `${context}.rhoHex`),
  };
  if (ownerTagRequired) {
    normalized.ownerTagHex = normalizeFixed32HexLiteral(
      output.ownerTagHex,
      `${context}.ownerTagHex`,
    );
  }
  return normalized;
}

function normalizeRequiredArray(value, context, normalizeEntry) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array`);
  }
  return value.map(normalizeEntry);
}

function normalizeOptionalOutputs(value) {
  if (value === undefined) {
    return [];
  }
  return normalizeRequiredArray(value, "outputs", (entry, index) =>
    normalizeConfidentialOutput(entry, index, false),
  );
}

function toNamedBuffer(value, context) {
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (ArrayBuffer.isView(value)) {
    return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return Buffer.from(value);
  }
  if (
    Array.isArray(value) &&
    value.every(
      (entry) => Number.isInteger(entry) && entry >= 0 && entry <= 0xff,
    )
  ) {
    return Buffer.from(value);
  }
  throw new TypeError(`${context} must be a Buffer or ArrayBuffer view`);
}

function normalizeNativeFixed32Array(value, context) {
  return normalizeRequiredArray(value, context, (entry, index) => {
    const buffer = toNamedBuffer(entry, `${context}[${index}]`);
    if (buffer.length !== 32) {
      throw new TypeError(`${context}[${index}] must be 32 bytes`);
    }
    return Buffer.from(buffer);
  });
}

function normalizeNativeProofResult(value, context, includeOutputCommitments) {
  const result = requireRecord(value, context);
  rejectRetiredFields(
    result,
    [["output_commitments", "outputCommitments"]],
    context,
  );
  const canonicalFields = new Set(
    includeOutputCommitments
      ? ["nullifiers", "outputCommitments", "root", "proof"]
      : ["nullifiers", "root", "proof"],
  );
  for (const field of Object.keys(result)) {
    if (!canonicalFields.has(field)) {
      throw new TypeError(`${context}.${field} is not a canonical result field`);
    }
  }
  const nullifiers = normalizeNativeFixed32Array(
    result.nullifiers,
    `${context}.nullifiers`,
  );
  const root = toNamedBuffer(result.root, `${context}.root`);
  if (root.length !== 32) {
    throw new TypeError(`${context}.root must be 32 bytes`);
  }
  const proof = toNamedBuffer(result.proof, `${context}.proof`);
  if (proof.length === 0) {
    throw new TypeError(`${context}.proof must be non-empty`);
  }
  const normalized = {
    nullifiers,
    root: Buffer.from(root),
    proof: Buffer.from(proof),
  };
  if (includeOutputCommitments) {
    normalized.outputCommitments = normalizeNativeFixed32Array(
      result.outputCommitments,
      `${context}.outputCommitments`,
    );
  }
  return normalized;
}

/**
 * Build a confidential transfer v2 proof envelope.
 */
export function buildConfidentialTransferProofV2({
  networkId,
  assetDefinitionId,
  spendKey,
  treeCommitments,
  inputs,
  outputs,
  rootHintHex,
  verifyingKey,
}) {
  const native = resolveNativeBinding();
  if (
    !native ||
    typeof native.buildConfidentialTransferProofV2 !== "function"
  ) {
    throw new Error(
      "native binding 'buildConfidentialTransferProofV2' is unavailable",
    );
  }
  const vk = normalizeInlineVerifyingKeyRecord(
    verifyingKey,
    "confidentialTransferProofV2",
  );
  const spendKeyBuffer = toNamedBuffer(spendKey, "spendKey");
  if (spendKeyBuffer.length !== 32) {
    throw new TypeError("spendKey must be 32 bytes");
  }
  const normalizedInputs = normalizeRequiredArray(
    inputs,
    "inputs",
    normalizeConfidentialInput,
  );
  const normalizedOutputs = normalizeRequiredArray(
    outputs,
    "outputs",
    (entry, index) => normalizeConfidentialOutput(entry, index, true),
  );
  const normalizedTreeCommitments = normalizeRequiredArray(
    treeCommitments,
    "treeCommitments",
    (entry, index) =>
      normalizeFixed32BinaryLike(entry, `treeCommitments[${index}]`),
  );
  const result = native.buildConfidentialTransferProofV2(
    Buffer.from(
      networkIdBytes(networkId, "confidentialTransferProofV2.networkId"),
    ),
    normalizeExactMetadataString(
      assetDefinitionId,
      "confidentialTransferProofV2.assetDefinitionId",
    ),
    spendKeyBuffer,
    normalizedTreeCommitments,
    normalizedInputs,
    normalizedOutputs,
    normalizeFixed32HexLiteral(rootHintHex, "rootHintHex"),
    vk.backend,
    vk.circuitId,
    vk.bytes,
  );
  return normalizeNativeProofResult(
    result,
    "buildConfidentialTransferProofV2 result",
    true,
  );
}

/**
 * Build a confidential unshield v2 proof envelope.
 */
export function buildConfidentialUnshieldProofV2({
  networkId,
  assetDefinitionId,
  spendKey,
  treeCommitments,
  inputs,
  publicAmount,
  rootHintHex,
  verifyingKey,
}) {
  const native = resolveNativeBinding();
  if (
    !native ||
    typeof native.buildConfidentialUnshieldProofV2 !== "function"
  ) {
    throw new Error(
      "native binding 'buildConfidentialUnshieldProofV2' is unavailable",
    );
  }
  const vk = normalizeInlineVerifyingKeyRecord(
    verifyingKey,
    "confidentialUnshieldProofV2",
  );
  const spendKeyBuffer = toNamedBuffer(spendKey, "spendKey");
  if (spendKeyBuffer.length !== 32) {
    throw new TypeError("spendKey must be 32 bytes");
  }
  const normalizedInputs = normalizeRequiredArray(
    inputs,
    "inputs",
    normalizeConfidentialInput,
  );
  const normalizedTreeCommitments = normalizeRequiredArray(
    treeCommitments,
    "treeCommitments",
    (entry, index) =>
      normalizeFixed32BinaryLike(entry, `treeCommitments[${index}]`),
  );
  const result = native.buildConfidentialUnshieldProofV2(
    Buffer.from(
      networkIdBytes(networkId, "confidentialUnshieldProofV2.networkId"),
    ),
    normalizeExactMetadataString(
      assetDefinitionId,
      "confidentialUnshieldProofV2.assetDefinitionId",
    ),
    spendKeyBuffer,
    normalizedTreeCommitments,
    normalizedInputs,
    normalizeWholeNumberLiteral(publicAmount, "publicAmount"),
    normalizeFixed32HexLiteral(rootHintHex, "rootHintHex"),
    vk.backend,
    vk.circuitId,
    vk.bytes,
  );
  return normalizeNativeProofResult(
    result,
    "buildConfidentialUnshieldProofV2 result",
    false,
  );
}

/**
 * Build a confidential unshield v3 proof envelope with optional private change.
 */
export function buildConfidentialUnshieldProofV3({
  networkId,
  assetDefinitionId,
  spendKey,
  treeCommitments,
  inputs,
  outputs,
  publicAmount,
  rootHintHex,
  verifyingKey,
}) {
  const native = resolveNativeBinding();
  if (
    !native ||
    typeof native.buildConfidentialUnshieldProofV3 !== "function"
  ) {
    throw new Error(
      "native binding 'buildConfidentialUnshieldProofV3' is unavailable",
    );
  }
  const vk = normalizeInlineVerifyingKeyRecord(
    verifyingKey,
    "confidentialUnshieldProofV3",
  );
  const spendKeyBuffer = toNamedBuffer(spendKey, "spendKey");
  if (spendKeyBuffer.length !== 32) {
    throw new TypeError("spendKey must be 32 bytes");
  }
  const normalizedInputs = normalizeRequiredArray(
    inputs,
    "inputs",
    normalizeConfidentialInput,
  );
  const normalizedOutputs = normalizeOptionalOutputs(outputs);
  const normalizedTreeCommitments = normalizeRequiredArray(
    treeCommitments,
    "treeCommitments",
    (entry, index) =>
      normalizeFixed32BinaryLike(entry, `treeCommitments[${index}]`),
  );
  const result = native.buildConfidentialUnshieldProofV3(
    Buffer.from(
      networkIdBytes(networkId, "confidentialUnshieldProofV3.networkId"),
    ),
    normalizeExactMetadataString(
      assetDefinitionId,
      "confidentialUnshieldProofV3.assetDefinitionId",
    ),
    spendKeyBuffer,
    normalizedTreeCommitments,
    normalizedInputs,
    normalizedOutputs,
    normalizeWholeNumberLiteral(publicAmount, "publicAmount"),
    normalizeFixed32HexLiteral(rootHintHex, "rootHintHex"),
    vk.backend,
    vk.circuitId,
    vk.bytes,
  );
  return normalizeNativeProofResult(
    result,
    "buildConfidentialUnshieldProofV3 result",
    true,
  );
}
