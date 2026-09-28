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

function requireCanonicalFields(value, allowed, context) {
  const record = requireRecord(value, context);
  for (const field of Object.keys(record)) {
    if (!allowed.includes(field)) throw new TypeError(`${context}.${field} is not a canonical field`);
  }
  return record;
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

function normalizeNativeProofResult(value, context) {
  const result = requireRecord(value, context);
  rejectRetiredFields(
    result,
    [["output_commitments", "outputCommitments"]],
    context,
  );
  const canonicalFields = new Set(["nullifiers", "outputCommitments", "root", "proof"]);
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
  normalized.outputCommitments = normalizeNativeFixed32Array(
    result.outputCommitments, `${context}.outputCommitments`,
  );
  return normalized;
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
        const { networkId, assetDefinitionId, spendKey } = requireCanonicalFields(options, ["networkId", "assetDefinitionId", "spendKey"], "wallet options");
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
        const result = normalizeNativeProofResult(await pending, "confidential proof");
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
        requireCanonicalFields(request, ["inputs", "outputs", "treeCommitments", "rootHex"], "request");
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
        requireCanonicalFields(request, ["inputs", "publicAmount", "change", "treeCommitments", "rootHex"], "request");
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

/** @internal Bind the canonical root helper to one immutable runtime. */
export function createConfidentialRootComputer(nativeRuntime) {
  return async function computeConfidentialRoot(options) {
    const { commitments } = requireCanonicalFields(options, ["commitments"], "root options");
    const leaves = normalizeRequiredArray(commitments, "commitments",
      (entry, index) => normalizeFixed32BinaryLike(entry, `commitments[${index}]`),
      0, CONFIDENTIAL_TREE_CAPACITY);
    const native = resolveNativeRuntimeBinding(nativeRuntime);
    if (!native || typeof native.computeConfidentialRoot !== "function") {
      throw new ConfidentialProverError("NATIVE_UNAVAILABLE", "The installed native runtime does not support confidential Merkle roots");
    }
    const root = toNamedBuffer(await native.computeConfidentialRoot(leaves), "confidential root");
    if (root.length !== 32) throw new Error("Native confidential root must be 32 bytes");
    return Buffer.from(root);
  };
}

/** Compute a local history root; this does not authenticate the root against ledger state. */
export const computeConfidentialRoot = createConfidentialRootComputer(defaultNativeRuntime);

/** @internal Bind public change-note helpers to one immutable native runtime. */
export function createConfidentialChangeHelpers(nativeRuntime) {
  function defaultConfidentialDiversifier() {
    const native = resolveNativeRuntimeBinding(nativeRuntime);
    if (!native || typeof native.defaultConfidentialDiversifier !== "function") {
      throw new ConfidentialProverError("NATIVE_UNAVAILABLE", "The installed native runtime does not expose the confidential default diversifier");
    }
    const value = toNamedBuffer(native.defaultConfidentialDiversifier(), "default diversifier");
    if (value.length !== 32) throw new Error("Native default diversifier must be 32 bytes");
    return Buffer.from(value);
  }
  function confidentialChangeToInput(change, leafIndex) {
    requireCanonicalFields(change, ["amount", "rhoHex"], "change");
    const amount = walletAmount(change.amount, "change amount").toString();
    const rhoHex = normalizeFixed32HexLiteral(change.rhoHex, "change.rhoHex");
    const index = normalizeLeafIndex(leafIndex, "leafIndex");
    return { amount, rhoHex, diversifierHex: defaultConfidentialDiversifier().toString("hex"), leafIndex: index };
  }
  return Object.freeze({ defaultConfidentialDiversifier, confidentialChangeToInput });
}
const DEFAULT_CHANGE_HELPERS = createConfidentialChangeHelpers(defaultNativeRuntime);
/** The canonical native default diversifier used for redemption change. */
export function defaultConfidentialDiversifier() {
  return DEFAULT_CHANGE_HELPERS.defaultConfidentialDiversifier();
}
/** Create a later spend input from retained change and an authenticated leaf index.
 * This does not consume the source opening, authenticate its index or prove membership.
 * Private JavaScript strings remain caller-owned and runtime-managed.
 */
export function confidentialChangeToInput(change, leafIndex) {
  return DEFAULT_CHANGE_HELPERS.confidentialChangeToInput(change, leafIndex);
}
