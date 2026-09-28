import {
  defaultNativeRuntime,
  resolveNativeRuntimeBinding,
} from "./nativeRuntime.js";
import { networkIdBytes } from "./networkId.js";

const CONFIDENTIAL_TREE_CAPACITY = 1 << 16;

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
  if (value >= CONFIDENTIAL_TREE_CAPACITY) {
    throw new TypeError(`${context} must be below the confidential tree capacity (65536)`);
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

function normalizeRequiredArray(value, context, normalizeEntry, minimum = 0, maximum = Infinity) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array`);
  }
  if (value.length < minimum || value.length > maximum) {
    throw new TypeError(`${context} must contain between ${minimum} and ${maximum} entries`);
  }
  return Array.from(value, normalizeEntry);
}

function normalizeOptionalOutputs(value) {
  if (value === undefined) {
    return [];
  }
  return normalizeRequiredArray(
    value, "outputs", (entry, index) => normalizeConfidentialOutput(entry, index, false),
    0, 1,
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
function buildConfidentialTransferProofV2WithRuntime(
  nativeRuntime,
  {
    networkId,
    assetDefinitionId,
    spendKey,
    treeCommitments,
    inputs,
    outputs,
    rootHintHex,
    verifyingKey,
  },
) {
  const native = resolveNativeRuntimeBinding(nativeRuntime);
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
    1, 2,
  );
  const normalizedOutputs = normalizeRequiredArray(
    outputs,
    "outputs",
    (entry, index) => normalizeConfidentialOutput(entry, index, true),
    1, 2,
  );
  const normalizedTreeCommitments = normalizeRequiredArray(
    treeCommitments,
    "treeCommitments",
    (entry, index) =>
      normalizeFixed32BinaryLike(entry, `treeCommitments[${index}]`),
    0, CONFIDENTIAL_TREE_CAPACITY,
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
function buildConfidentialUnshieldProofV2WithRuntime(
  nativeRuntime,
  {
    networkId,
    assetDefinitionId,
    spendKey,
    treeCommitments,
    inputs,
    publicAmount,
    rootHintHex,
    verifyingKey,
  },
) {
  const native = resolveNativeRuntimeBinding(nativeRuntime);
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
    1, 2,
  );
  const normalizedTreeCommitments = normalizeRequiredArray(
    treeCommitments,
    "treeCommitments",
    (entry, index) =>
      normalizeFixed32BinaryLike(entry, `treeCommitments[${index}]`),
    0, CONFIDENTIAL_TREE_CAPACITY,
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
function buildConfidentialUnshieldProofV3WithRuntime(
  nativeRuntime,
  {
    networkId,
    assetDefinitionId,
    spendKey,
    treeCommitments,
    inputs,
    outputs,
    publicAmount,
    rootHintHex,
    verifyingKey,
  },
) {
  const native = resolveNativeRuntimeBinding(nativeRuntime);
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
    1, 2,
  );
  const normalizedOutputs = normalizeOptionalOutputs(outputs);
  const normalizedTreeCommitments = normalizeRequiredArray(
    treeCommitments,
    "treeCommitments",
    (entry, index) =>
      normalizeFixed32BinaryLike(entry, `treeCommitments[${index}]`),
    0, CONFIDENTIAL_TREE_CAPACITY,
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

/** @internal Create confidential proof builders bound to one immutable runtime. */
export function createConfidentialProofBuilders(nativeRuntime) {
  return Object.freeze({
    buildConfidentialTransferProofV2: (input) =>
      buildConfidentialTransferProofV2WithRuntime(nativeRuntime, input),
    buildConfidentialUnshieldProofV2: (input) =>
      buildConfidentialUnshieldProofV2WithRuntime(nativeRuntime, input),
    buildConfidentialUnshieldProofV3: (input) =>
      buildConfidentialUnshieldProofV3WithRuntime(nativeRuntime, input),
  });
}

const DEFAULT_CONFIDENTIAL_PROOF_BUILDERS =
  createConfidentialProofBuilders(defaultNativeRuntime);

export function buildConfidentialTransferProofV2(input) {
  return DEFAULT_CONFIDENTIAL_PROOF_BUILDERS.buildConfidentialTransferProofV2(
    input,
  );
}

export function buildConfidentialUnshieldProofV2(input) {
  return DEFAULT_CONFIDENTIAL_PROOF_BUILDERS.buildConfidentialUnshieldProofV2(
    input,
  );
}

export function buildConfidentialUnshieldProofV3(input) {
  return DEFAULT_CONFIDENTIAL_PROOF_BUILDERS.buildConfidentialUnshieldProofV3(
    input,
  );
}

/** Failure from the local wallet API; `code` never depends on private values. */
export class ConfidentialProverError extends Error {
  constructor(code, message, options) {
    super(message, options);
    this.name = "ConfidentialProverError";
    this.code = code;
  }
}

const U128_MAX = (1n << 128n) - 1n;

function walletAmount(value, context) {
  if (typeof value === "number" && !Number.isSafeInteger(value)) {
    throw new TypeError(`${context} must use bigint or a decimal string outside the safe integer range`);
  }
  const amount = BigInt(normalizeWholeNumberLiteral(value, context));
  if (amount <= 0n || amount > U128_MAX) {
    throw new TypeError(`${context} must be a positive u128 amount`);
  }
  return amount;
}

function walletTotal(notes, context) {
  const total = notes.reduce((sum, note) => sum + walletAmount(note.amount, context), 0n);
  if (total > U128_MAX) throw new TypeError(`${context} total exceeds u128`);
  return total;
}

/** @internal Bind a wallet class to one immutable native runtime for source tests. */
export function createConfidentialProverClass(nativeRuntime) {
  return class ConfidentialProver {
    #key;
    #network;
    #asset;
    #native;

    constructor(options) {
      // Resolve capability before retaining the key or reading any witness.
      let native;
      try {
        native = resolveNativeRuntimeBinding(nativeRuntime);
      } catch (cause) {
        throw new ConfidentialProverError("NATIVE_UNAVAILABLE", "The native confidential proving runtime could not be loaded", { cause });
      }
      if (!native || typeof native.proveConfidentialTransfer !== "function" ||
          typeof native.proveConfidentialRedemption !== "function") {
        throw new ConfidentialProverError("NATIVE_UNAVAILABLE", "The installed native runtime does not support canonical confidential wallet proving");
      }
      try {
        const { networkId, assetDefinitionId, spendKey } = requireRecord(options, "wallet options");
        this.#network = Buffer.from(networkIdBytes(networkId, "networkId"));
        this.#asset = normalizeExactMetadataString(assetDefinitionId, "assetDefinitionId");
        // Only mutable binary input is accepted; callers can erase their source.
        if (!(spendKey instanceof Uint8Array) || spendKey.length !== 32 ||
            spendKey.every((byte) => byte === 0)) {
          throw new TypeError("spendKey must be a nonzero 32-byte Uint8Array");
        }
        this.#key = Buffer.from(spendKey);
        this.#native = native;
      } catch (cause) {
        this.#key?.fill(0);
        throw new ConfidentialProverError("INVALID_INPUT", cause instanceof Error ? cause.message : "Invalid confidential wallet options", { cause });
      }
      Object.freeze(this);
    }

    /** Erase this prover's key and close it; already queued native jobs finish independently. */
    dispose() {
      this.#key?.fill(0);
      this.#key = undefined;
    }

    #prepare(request) {
      if (!this.#key) throw new ConfidentialProverError("DISPOSED", "Confidential prover has been disposed");
      requireRecord(request, "request");
      const inputs = normalizeRequiredArray(request.inputs, "inputs", normalizeConfidentialInput, 1, 2);
      // Check exact numeric semantics before the lower-level normalizer stringifies numbers.
      request.inputs.forEach((input) => walletAmount(input.amount, "input amount"));
      const leaves = normalizeRequiredArray(request.treeCommitments, "treeCommitments",
        (entry, index) => normalizeFixed32BinaryLike(entry, `treeCommitments[${index}]`),
        1, CONFIDENTIAL_TREE_CAPACITY);
      const indices = new Set();
      for (const input of inputs) {
        if (input.leafIndex >= leaves.length) throw new TypeError("input note index lies outside the supplied tree");
        if (indices.has(input.leafIndex)) throw new TypeError("a confidential spend cannot consume the same note twice");
        indices.add(input.leafIndex);
      }
      return { inputs, leaves, root: normalizeFixed32HexLiteral(request.rootHex, "rootHex"), total: walletTotal(inputs, "inputs") };
    }

    async #prove(relation, prepared, outputs, invoke) {
      // A request getter can dispose the wallet during synchronous normalization.
      if (!this.#key) throw new ConfidentialProverError("DISPOSED", "Confidential prover has been disposed");
      // Native preparation synchronously takes its own clearing copy before
      // returning a promise. Do not retain this FFI copy for the worker lifetime.
      const key = Buffer.from(this.#key);
      try {
        let pending;
        try {
          pending = invoke(key);
        } finally {
          key.fill(0);
        }
        const result = normalizeNativeProofResult(await pending, "confidential proof", true);
        if (result.nullifiers.length !== prepared.inputs.length || result.outputCommitments.length !== outputs ||
            result.root.toString("hex") !== prepared.root) {
          throw new Error("Native confidential proof returned inconsistent public outputs");
        }
        return { relation, ...result };
      } catch (cause) {
        throw new ConfidentialProverError("PROVING_FAILED", "Confidential proof generation or verification failed", { cause });
      }
    }

    /** Prove and locally verify a transfer using internally selected circuit and key. */
    async proveTransfer(request) {
      let prepared;
      let outputs;
      try {
        prepared = this.#prepare(request);
        outputs = normalizeRequiredArray(request.outputs, "outputs",
          (entry, index) => normalizeConfidentialOutput(entry, index, true), 1, 2);
        request.outputs.forEach((output) => walletAmount(output.amount, "output amount"));
        if (walletTotal(outputs, "outputs") !== prepared.total) throw new TypeError("transfer input and output totals must match");
      } catch (cause) {
        if (cause instanceof ConfidentialProverError) throw cause;
        throw new ConfidentialProverError("INVALID_INPUT", cause instanceof Error ? cause.message : "Invalid confidential transfer input", { cause });
      }
      return this.#prove("confidential-transfer", prepared, outputs.length, (key) =>
        this.#native.proveConfidentialTransfer(this.#network, this.#asset, key,
          prepared.leaves, prepared.inputs, outputs, prepared.root));
    }

    /** Redeem a positive amount, selecting the full or private-change relation internally. */
    async proveRedemption(request) {
      let prepared;
      let amount;
      let change;
      try {
        prepared = this.#prepare(request);
        amount = walletAmount(request.publicAmount, "publicAmount");
        if (amount > prepared.total) throw new TypeError("publicAmount exceeds the input total");
        if (request.change !== undefined) {
          walletAmount(request.change.amount, "change amount");
          change = normalizeConfidentialOutput(request.change, 0, false);
        }
        const remainder = prepared.total - amount;
        if ((change === undefined ? 0n : BigInt(change.amount)) !== remainder ||
            (change !== undefined && remainder === 0n)) {
          throw new TypeError("supply one exact change note for a nonzero remainder and none for full redemption");
        }
      } catch (cause) {
        if (cause instanceof ConfidentialProverError) throw cause;
        throw new ConfidentialProverError("INVALID_INPUT", cause instanceof Error ? cause.message : "Invalid confidential redemption input", { cause });
      }
      return this.#prove(change === undefined ? "confidential-redemption" : "confidential-redemption-with-change",
        prepared, change === undefined ? 0 : 1, (key) =>
          this.#native.proveConfidentialRedemption(this.#network, this.#asset, key,
            prepared.leaves, prepared.inputs, amount.toString(), prepared.root, change));
    }
  };
}

/** Canonical local wallet prover. Always dispose it when the wallet operation ends. */
export const ConfidentialProver = createConfidentialProverClass(defaultNativeRuntime);
