import { createHash } from "node:crypto";
import { parseCanonicalContractAddress } from "./contractAddress.js";
import { getNativeBinding } from "./native.js";
import {
  CRYPTO_ALGORITHMS,
  normalizeCryptoAlgorithm,
  publicKeyFromPrivate,
} from "./crypto.js";
import {
  computeIvmArtifactHashes,
  IVM_ARTIFACT_MAX_BYTES,
  IVM_PROGRAM_HEADER_LENGTH,
} from "./ivmArtifact.js";
import { ToriiClient } from "./toriiClient.js";
import { noritoDecodeInstruction } from "./norito.js";
import {
  KotodamaQuantity,
  NumericV1,
  NumericV1Error,
} from "./numericV1.js";
import {
  buildBurnAssetInstruction,
  buildMintAssetInstruction,
  buildMintTriggerRepetitionsInstruction,
  buildBurnTriggerRepetitionsInstruction,
  buildTransferAssetInstruction,
  buildTransferAssetDefinitionInstruction,
  buildTransferDomainInstruction,
  buildTransferNftInstruction,
  buildRegisterRwaInstruction,
  buildTransferRwaInstruction,
  buildMergeRwasInstruction,
  buildRedeemRwaInstruction,
  buildFreezeRwaInstruction,
  buildUnfreezeRwaInstruction,
  buildHoldRwaInstruction,
  buildReleaseRwaInstruction,
  buildForceTransferRwaInstruction,
  buildSetRwaControlsInstruction,
  buildSetRwaKeyValueInstruction,
  buildRemoveRwaKeyValueInstruction,
  buildRegisterDomainInstruction,
  buildRegisterAccountInstruction,
  buildRegisterMultisigInstruction,
  buildCreateKaigiInstruction,
  buildJoinKaigiInstruction,
  buildLeaveKaigiInstruction,
  buildEndKaigiInstruction,
  buildRecordKaigiUsageInstruction,
  buildSetKaigiRelayManifestInstruction,
  buildRegisterKaigiRelayInstruction,
  buildRegisterSmartContractCodeInstruction,
  buildRegisterSmartContractBytesInstruction,
  buildRemoveSmartContractBytesInstruction,
  buildProposeDeployContractInstruction,
  buildProposeSccpRouteGovernanceInstruction,
  buildCastZkBallotInstruction,
  buildCastPlainBallotInstruction,
  buildEnactReferendumInstruction,
  buildFinalizeReferendumInstruction,
  buildPersistCouncilForEpochInstruction,
  buildRegisterZkAssetInstruction,
  buildScheduleConfidentialPolicyTransitionInstruction,
  buildCancelConfidentialPolicyTransitionInstruction,
  buildShieldInstruction,
  buildZkTransferInstruction,
  buildUnshieldInstruction,
  buildCreateElectionInstruction,
  buildSubmitBallotInstruction,
  buildFinalizeElectionInstruction,
  normalizeAccountId,
  normalizeAssetId,
} from "./instructionBuilders.js";
import { normalizeSccpRouteGovernanceAction } from "./sccp.js";

const submissionAbortSignalAbortedGetter =
  typeof AbortSignal === "undefined"
    ? null
    : (Object.getOwnPropertyDescriptor(AbortSignal.prototype, "aborted")?.get ??
      null);
const submissionAbortSignalReasonGetter =
  typeof AbortSignal === "undefined"
    ? null
    : (Object.getOwnPropertyDescriptor(AbortSignal.prototype, "reason")?.get ??
      null);

function normalizeAuthority(authority) {
  const raw = String(authority ?? "");
  if (raw.length === 0) {
    return normalizeAccountId(authority, "authority");
  }
  if (raw.trim() !== raw) {
    throw new TypeError("authority must not contain surrounding whitespace");
  }
  return normalizeAccountId(raw, "authority");
}

function resolveNativeBinding() {
  // Allow tests to inject a fake binding.
  return globalThis.__IROHA_NATIVE_BINDING__ ?? getNativeBinding();
}

function composeAssetHoldingIdFromDefinitionAndAccount(
  assetDefinitionId,
  accountId,
  context,
) {
  const definition = normalizeTransactionAssetDefinitionId(
    assetDefinitionId,
    `${context}.assetDefinitionId`,
  );
  const normalizedAccountId = normalizeAccountId(
    accountId,
    `${context}.accountId`,
  );
  return `${definition}#${normalizedAccountId}`;
}

function normalizeTransactionAssetDefinitionId(assetDefinitionId, context) {
  const rawDefinition = String(assetDefinitionId ?? "");
  const definition = rawDefinition.trim();
  if (!definition) {
    throw new TypeError(`${context} must be a non-empty string`);
  }
  if (definition !== rawDefinition) {
    throw new TypeError(`${context} must not contain surrounding whitespace`);
  }
  if (
    /\s/.test(definition) ||
    definition.includes("%") ||
    definition.includes("/") ||
    definition.includes("?") ||
    definition.includes(":")
  ) {
    throw new TypeError(
      `${context} must be a canonical unprefixed Base58 asset definition id`,
    );
  }
  return definition;
}

function serializeInstructionPayloads(instructions, context) {
  if (!Array.isArray(instructions) || instructions.length === 0) {
    throw new Error(`${context ?? "instructions"} must be a non-empty array`);
  }
  return instructions.map((instruction, index) => {
    if (typeof instruction === "string") {
      return instruction;
    }
    if (instruction && typeof instruction === "object") {
      return JSON.stringify(instruction);
    }
    throw new TypeError(
      `${context ?? "instructions"}[${index}] must be an object or JSON string`,
    );
  });
}

const MAX_CONTRACT_ARGUMENT_RECORD_BYTES = 1024 * 1024;

function normalizeExecutableBatchHash(value, context) {
  let hash;
  if (typeof value === "string") {
    const literal = value.startsWith("0x") ? value.slice(2) : value;
    if (!/^[0-9a-fA-F]{64}$/u.test(literal)) {
      throw new TypeError(`${context} must be exactly 32 hexadecimal bytes`);
    }
    hash = Buffer.from(literal, "hex");
  } else {
    hash = toBuffer(value, context);
  }
  if (hash.length !== 32) {
    throw new TypeError(`${context} must be exactly 32 bytes`);
  }
  if ((hash[31] & 1) === 0) {
    throw new TypeError(`${context} must carry the canonical Iroha hash marker bit`);
  }
  return hash;
}

function serializeExecutableBatchEntries(entries) {
  if (!Array.isArray(entries) || entries.length === 0) {
    throw new TypeError("entries must be a non-empty array");
  }
  let containsContractCall = false;
  const serialized = entries.map((value, index) => {
    const entry = normalizePlainObject(value, `entries[${index}]`);
    if (entry.kind === "instruction") {
      if (entry.instruction === undefined) {
        throw new TypeError(`entries[${index}].instruction is required`);
      }
      const instruction = entry.instruction;
      if (
        typeof instruction !== "string" &&
        (!instruction || typeof instruction !== "object" || Array.isArray(instruction))
      ) {
        throw new TypeError(
          `entries[${index}].instruction must be an object or JSON string`,
        );
      }
      return JSON.stringify({ kind: "instruction", instruction });
    }
    if (entry.kind !== "contractCall") {
      throw new TypeError(
        `entries[${index}].kind must be instruction or contractCall`,
      );
    }
    containsContractCall = true;
    const contractAddress = parseCanonicalContractAddress(
      entry.contractAddress,
      `entries[${index}].contractAddress`,
    ).literal;
    if (
      typeof entry.entrypoint !== "string" ||
      entry.entrypoint.length === 0 ||
      entry.entrypoint.trim() !== entry.entrypoint
    ) {
      throw new TypeError(
        `entries[${index}].entrypoint must be a non-empty exact string`,
      );
    }
    const expectedCodeHash = normalizeExecutableBatchHash(
      entry.expectedCodeHash,
      `entries[${index}].expectedCodeHash`,
    );
    const argumentsBytes =
      entry.arguments === undefined || entry.arguments === null
        ? null
        : toBuffer(entry.arguments, `entries[${index}].arguments`);
    if (
      argumentsBytes !== null &&
      argumentsBytes.length > MAX_CONTRACT_ARGUMENT_RECORD_BYTES
    ) {
      throw new RangeError(
        `entries[${index}].arguments exceeds ${MAX_CONTRACT_ARGUMENT_RECORD_BYTES} bytes`,
      );
    }
    return JSON.stringify({
      kind: "contractCall",
      contractAddress,
      expectedCodeHash: expectedCodeHash.toString("hex").toUpperCase(),
      entrypoint: entry.entrypoint,
      arguments: argumentsBytes === null ? null : Array.from(argumentsBytes),
    });
  });
  return { serialized, containsContractCall };
}

function requireExecutableBatchGasLimit(feePayment, containsContractCall) {
  const feePaymentJson = feePaymentIntentToNoritoJson(feePayment);
  if (
    containsContractCall &&
    JSON.parse(feePaymentJson).value.gas_limit === null
  ) {
    throw new TypeError(
      "feePayment.gasLimit is required when entries contain a contract call",
    );
  }
  return feePaymentJson;
}

function normalizeMetadataPayload(metadata, context) {
  if (metadata === null || metadata === undefined) {
    return null;
  }
  if (typeof metadata === "string") {
    return metadata;
  }
  if (typeof metadata === "object" && !Array.isArray(metadata)) {
    return JSON.stringify(metadata);
  }
  throw new TypeError(
    `${context} must be an object or JSON string when provided`,
  );
}

function canonicalFeeUnsigned(value, context, { nonZero = false } = {}) {
  let literal;
  if (typeof value === "bigint") {
    literal = value.toString(10);
  } else if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) {
      throw new TypeError(`${context} must be a safe integer, bigint, or decimal string`);
    }
    literal = String(value);
  } else if (typeof value === "string" && /^(?:0|[1-9]\d*)$/u.test(value)) {
    literal = value;
  } else {
    throw new TypeError(`${context} must be a canonical unsigned integer`);
  }
  const parsed = BigInt(literal);
  if (parsed > 0xffff_ffff_ffff_ffffn || (nonZero && parsed === 0n)) {
    throw new RangeError(`${context} is outside its canonical u64 range`);
  }
  return literal;
}

function canonicalFeeQuantity(value, context) {
  if (typeof value === "number") {
    throw new TypeError(`${context} must not use a JavaScript number`);
  }
  let literal;
  if (typeof value === "string" || typeof value === "bigint") {
    literal = String(value);
  } else if (value && typeof value.toString === "function") {
    literal = value.toString();
  } else {
    throw new TypeError(`${context} must be a canonical positive quantity`);
  }
  if (!/^(?:0|[1-9]\d*)(?:\.\d*[1-9])?$/u.test(literal) || literal === "0") {
    throw new TypeError(`${context} must be a canonical positive quantity`);
  }
  return literal;
}

/**
 * Convert the ergonomic JavaScript fee-payment shape into the exact Norito
 * JSON representation accepted by the native signer.
 */
export function feePaymentIntentToNoritoJson(feePayment) {
  const input = normalizePlainObject(feePayment, "feePayment");
  if (input.payer !== "authority" && input.payer !== "sponsor") {
    throw new TypeError("feePayment.payer must be authority or sponsor");
  }
  if (!Array.isArray(input.chargeLimits)) {
    throw new TypeError("feePayment.chargeLimits must be an array");
  }
  let previousKind = -1;
  const chargeLimits = input.chargeLimits.map((value, index) => {
    const limit = normalizePlainObject(value, `feePayment.chargeLimits[${index}]`);
    const kind = limit.kind === "nexus" ? 0 : limit.kind === "pipelineGas" ? 1 : -1;
    if (kind < 0) {
      throw new TypeError(
        `feePayment.chargeLimits[${index}].kind must be nexus or pipelineGas`,
      );
    }
    if (kind <= previousKind) {
      throw new TypeError(
        "feePayment.chargeLimits must be unique and ordered nexus before pipelineGas",
      );
    }
    previousKind = kind;
    const assetDefinitionId = normalizeTransactionAssetDefinitionId(
      limit.assetDefinitionId,
      `feePayment.chargeLimits[${index}].assetDefinitionId`,
    );
    return {
      kind: { kind: kind === 0 ? "nexus" : "pipeline_gas", value: null },
      asset_definition_id: assetDefinitionId,
      max_amount: canonicalFeeQuantity(
        limit.maxAmount,
        `feePayment.chargeLimits[${index}].maxAmount`,
      ),
    };
  });
  const gasLimit =
    input.gasLimit === undefined || input.gasLimit === null
      ? null
      : canonicalFeeUnsigned(input.gasLimit, "feePayment.gasLimit", {
          nonZero: true,
        });
  const common = `"charge_limits":${JSON.stringify(chargeLimits)},"gas_limit":${gasLimit ?? "null"}`;
  if (input.payer === "authority") {
    if (input.programId !== undefined || input.programRevision !== undefined) {
      throw new TypeError(
        "authority feePayment must not include programId or programRevision",
      );
    }
    return `{"payer":"authority","value":{${common}}}`;
  }
  if (typeof input.programId !== "string" || input.programId.trim() !== input.programId) {
    throw new TypeError("feePayment.programId must be an exact sponsor/program string");
  }
  const slash = input.programId.indexOf("/");
  if (slash <= 0 || slash === input.programId.length - 1) {
    throw new TypeError("feePayment.programId must use sponsor/program");
  }
  const sponsor = input.programId.slice(0, slash);
  const name = input.programId.slice(slash + 1);
  const revision = canonicalFeeUnsigned(
    input.programRevision,
    "feePayment.programRevision",
    { nonZero: true },
  );
  return `{"payer":"sponsor","value":{"program_id":{"sponsor":${JSON.stringify(
    sponsor,
  )},"name":${JSON.stringify(name)}},"program_revision":${revision},${common}}}`;
}

function normalizeJsonObjectPayload(value, context) {
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (!trimmed) {
      throw new TypeError(`${context} must not be an empty JSON string`);
    }
    return trimmed;
  }
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return JSON.stringify(value);
  }
  throw new TypeError(`${context} must be an object or JSON string`);
}

function normalizePlainObject(value, context) {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return value;
  }
  throw new TypeError(`${context} must be a non-null object`);
}

function normalizeNonEmptyString(value, context) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a non-empty string`);
  }
  const trimmed = value.trim();
  if (!trimmed) {
    throw new TypeError(`${context} must be a non-empty string`);
  }
  return trimmed;
}

function normalizeUint32(value, context) {
  const max = 0xffff_ffffn;
  let parsed;
  if (typeof value === "bigint") {
    parsed = value;
  } else if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) {
      throw new TypeError(`${context} must be a uint32 integer`);
    }
    parsed = BigInt(value);
  } else if (typeof value === "string") {
    const trimmed = value.trim();
    if (!/^(?:0|[1-9][0-9]*)$/u.test(trimmed)) {
      throw new TypeError(`${context} must be a uint32 integer`);
    }
    parsed = BigInt(trimmed);
  } else {
    throw new TypeError(`${context} must be a uint32 integer`);
  }
  if (parsed < 0n || parsed > max) {
    throw new RangeError(`${context} must be between 0 and 4294967295`);
  }
  return Number(parsed);
}

function normalizeOptionalPositiveInteger(value, context) {
  if (value === null || value === undefined) {
    return null;
  }
  return ToriiClient._normalizeUnsignedInteger(value, context, {
    allowZero: false,
  });
}

/**
 * Compute the canonical transaction hash (blake2b-256) for a signed transaction.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} signedTransaction
 * @param {{ encoding?: BufferEncoding }} [options]
 * @returns {string | Buffer} Hex string by default, Buffer when `encoding` is `"buffer"`.
 */
export function hashSignedTransaction(signedTransaction, options = {}) {
  const native = resolveNativeBinding();
  if (!native || typeof native.hashSignedTransaction !== "function") {
    throw new Error("native binding 'hashSignedTransaction' is unavailable");
  }
  const buffer = toBuffer(signedTransaction);
  const hashBuffer = Buffer.from(native.hashSignedTransaction(buffer));
  if (options.encoding === "buffer") {
    return hashBuffer;
  }
  const encoding = options.encoding ?? "hex";
  return hashBuffer.toString(encoding);
}

/**
 * Compute the detached-signature preimage used by Torii for a transaction
 * scaffold (`HashOf::new(tx.payload())`).
 * @param {ArrayBufferView | ArrayBuffer | Buffer} signedTransaction
 * @param {{ encoding?: BufferEncoding }} [options]
 * @returns {string | Buffer} Hex string by default, Buffer when `encoding` is `"buffer"`.
 */
export function hashSignedTransactionPayload(signedTransaction, options = {}) {
  const native = resolveNativeBinding();
  if (!native || typeof native.hashSignedTransactionPayload !== "function") {
    throw new Error(
      "native binding 'hashSignedTransactionPayload' is unavailable",
    );
  }
  const buffer = toBuffer(signedTransaction);
  const hashBuffer = Buffer.from(native.hashSignedTransactionPayload(buffer));
  if (options.encoding === "buffer") {
    return hashBuffer;
  }
  const encoding = options.encoding ?? "hex";
  return hashBuffer.toString(encoding);
}

/**
 * Decode a canonical Norito signed transaction into its JSON representation.
 * This is intended for wallet policy checks before signing an untrusted
 * transaction scaffold.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} signedTransaction
 * @returns {Record<string, unknown>}
 */
export function decodeSignedTransaction(signedTransaction) {
  const native = resolveNativeBinding();
  if (!native || typeof native.decodeSignedTransactionJson !== "function") {
    throw new Error(
      "native binding 'decodeSignedTransactionJson' is unavailable",
    );
  }
  const decoded = JSON.parse(
    native.decodeSignedTransactionJson(toBuffer(signedTransaction)),
  );
  if (!decoded || typeof decoded !== "object" || Array.isArray(decoded)) {
    throw new Error("decoded signed transaction must be an object");
  }
  return decoded;
}

/**
 * Encode an entrypoint payload with the exact Kotodama ABI schema into the
 * canonical argument bytes that must be present in a signed contract call.
 * @param {Record<string, unknown>} argumentSchema
 * @param {Record<string, unknown>} payload
 * @returns {Buffer}
 */
export function encodeContractArgumentRecord(argumentSchema, payload) {
  const native = resolveNativeBinding();
  if (!native || typeof native.encodeContractArgumentRecordJson !== "function") {
    throw new Error(
      "native binding 'encodeContractArgumentRecordJson' is unavailable",
    );
  }
  let schemaJson;
  let payloadJson;
  try {
    schemaJson = JSON.stringify(argumentSchema);
    payloadJson = JSON.stringify(payload);
  } catch (error) {
    throw new TypeError(`contract argument input is not JSON serializable: ${error}`);
  }
  if (typeof schemaJson !== "string" || typeof payloadJson !== "string") {
    throw new TypeError("contract argument schema and payload must be JSON values");
  }
  return Buffer.from(
    native.encodeContractArgumentRecordJson(schemaJson, payloadJson),
  );
}

/**
 * Compute the canonical proposal identity for an authorized instruction batch
 * (`HashOf::new(&Vec<InstructionBox>)`). This is the value Torii exposes as
 * both `instructions_hash` and `proposal_id` for multisig proposals.
 * @param {Array<object | string>} instructions
 * @param {{ encoding?: BufferEncoding }} [options]
 * @returns {string | Buffer} Hex string by default, Buffer when `encoding` is `"buffer"`.
 */
export function hashInstructionBatch(instructions, options = {}) {
  const native = resolveNativeBinding();
  if (!native || typeof native.hashInstructionBatch !== "function") {
    throw new Error("native binding 'hashInstructionBatch' is unavailable");
  }
  const normalizedInstructions = serializeInstructionPayloads(
    instructions,
    "instructions",
  );
  const hashBuffer = Buffer.from(
    native.hashInstructionBatch(normalizedInstructions),
  );
  if (options.encoding === "buffer") {
    return hashBuffer;
  }
  const encoding = options.encoding ?? "hex";
  return hashBuffer.toString(encoding);
}

/**
 * Re-sign a Norito-encoded transaction with the provided Ed25519 private key.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} signedTransaction
 * @param {ArrayBufferView | ArrayBuffer | Buffer} privateKey 32- or 64-byte Ed25519 key.
 * @returns {Buffer}
 */
export function resignSignedTransaction(signedTransaction, privateKey) {
  const native = resolveNativeBinding();
  if (!native || typeof native.signTransaction !== "function") {
    throw new Error("native binding 'signTransaction' is unavailable");
  }
  const txBuffer = toBuffer(signedTransaction);
  const keyBuffer = toBuffer(privateKey);
  if (keyBuffer.byteLength !== 32 && keyBuffer.byteLength !== 64) {
    throw new Error("private key must be a 32- or 64-byte Ed25519 key");
  }
  return Buffer.from(native.signTransaction(txBuffer, keyBuffer));
}

/**
 * Build and sign a RegisterDomain transaction via the native helper.
 * @param {{
 *   chainId: string,
 *   authority: string,
 *   domainId: string,
 *   feePayment: object,
 *   metadata?: object | string | null,
 *   creationTimeMs?: number,
 *   ttlMs?: number,
 *   nonce?: number,
 *   privateKey: ArrayBufferView | ArrayBuffer | Buffer,
 *   privateKeyAlgorithm?: string
 * }} input
 * @returns {{signedTransaction: Buffer, hash: Buffer}}
 */
export function buildRegisterDomainTransaction(input) {
  const native = resolveNativeBinding();
  if (!native || typeof native.buildRegisterDomainTransaction !== "function") {
    throw new Error(
      "native binding 'build_register_domain_transaction' is unavailable",
    );
  }
  const {
    chainId,
    authority,
    domainId,
    feePayment,
    metadata = null,
    creationTimeMs = null,
    ttlMs = null,
    nonce = null,
    privateKey,
    privateKeyAlgorithm = null,
  } = input;

  const canonicalAuthority = normalizeAuthority(authority);

  const metadataPayload =
    metadata === null || metadata === undefined
      ? null
      : typeof metadata === "string"
        ? metadata
        : JSON.stringify(metadata);

  const result = native.buildRegisterDomainTransaction(
    chainId,
    canonicalAuthority,
    domainId,
    feePaymentIntentToNoritoJson(feePayment),
    metadataPayload,
    creationTimeMs,
    ttlMs,
    nonce,
    toBuffer(privateKey),
    privateKeyAlgorithm,
  );
  const signed =
    result?.signed_transaction ?? result?.signedTransaction ?? null;
  const hashBytes = result?.hash ?? result?.hashBytes ?? null;
  if (!signed || !hashBytes) {
    throw new Error(
      "native binding 'build_register_domain_transaction' returned missing fields",
    );
  }
  return {
    signedTransaction: Buffer.from(signed),
    hash: Buffer.from(hashBytes),
  };
}

/**
 * Build and sign a transaction from arbitrary instruction payloads.
 * @param {{
 *   chainId: string,
 *   authority: string,
 *   instructions: Array<object | string>,
 *   feePayment: object,
 *   metadata?: object | string | null,
 *   creationTimeMs?: number,
 *   ttlMs?: number,
 *   nonce?: number,
 *   privateKey: ArrayBufferView | ArrayBuffer | Buffer,
 *   privateKeyAlgorithm?: string
 * }} input
 * @returns {{signedTransaction: Buffer, hash: Buffer}}
 */
export function buildTransaction(input) {
  const native = resolveNativeBinding();
  if (!native || typeof native.buildTransaction !== "function") {
    throw new Error("native binding 'build_transaction' is unavailable");
  }

  const {
    chainId,
    authority,
    instructions,
    feePayment,
    metadata = null,
    creationTimeMs = null,
    ttlMs = null,
    nonce = null,
    privateKey,
    privateKeyAlgorithm = null,
  } = input;

  const normalizedInstructions = serializeInstructionPayloads(
    instructions,
    "instructions",
  );

  const metadataPayload = normalizeMetadataPayload(
    metadata,
    "transaction metadata",
  );

  const canonicalAuthority = normalizeAuthority(authority);

  const result = native.buildTransaction(
    chainId,
    canonicalAuthority,
    normalizedInstructions,
    feePaymentIntentToNoritoJson(feePayment),
    metadataPayload,
    creationTimeMs,
    ttlMs,
    nonce,
    toBuffer(privateKey),
    privateKeyAlgorithm,
  );

  const signed =
    result?.signed_transaction ?? result?.signedTransaction ?? null;
  const hashBytes = result?.hash ?? result?.hashBytes ?? null;
  if (!signed || !hashBytes) {
    throw new Error(
      "native binding 'build_transaction' returned missing fields",
    );
  }

  return {
    signedTransaction: Buffer.from(signed),
    hash: Buffer.from(hashBytes),
  };
}

/**
 * Build and sign one ordered, atomic mix of native instructions and deployed
 * contract calls. Instruction-only callers should keep using
 * {@link buildTransaction} to preserve the legacy executable wire tag.
 *
 * @param {object} input
 * @returns {{signedTransaction: Buffer, hash: Buffer}}
 */
export function buildExecutableBatchTransaction(input) {
  const native = resolveNativeBinding();
  if (
    !native ||
    typeof native.buildExecutableBatchTransaction !== "function"
  ) {
    throw new Error(
      "native binding 'build_executable_batch_transaction' is unavailable",
    );
  }
  const {
    chainId,
    authority,
    entries,
    feePayment,
    metadata = null,
    creationTimeMs = null,
    ttlMs = null,
    nonce = null,
    privateKey,
    privateKeyAlgorithm = null,
  } = input;
  const { serialized, containsContractCall } =
    serializeExecutableBatchEntries(entries);
  const feePaymentJson = requireExecutableBatchGasLimit(
    feePayment,
    containsContractCall,
  );
  const result = native.buildExecutableBatchTransaction(
    chainId,
    normalizeAuthority(authority),
    serialized,
    feePaymentJson,
    normalizeMetadataPayload(metadata, "transaction metadata"),
    creationTimeMs,
    ttlMs,
    nonce,
    toBuffer(privateKey, "privateKey"),
    privateKeyAlgorithm,
  );
  const signed =
    result?.signed_transaction ?? result?.signedTransaction ?? null;
  const hashBytes = result?.hash ?? result?.hashBytes ?? null;
  if (!signed || !hashBytes) {
    throw new Error(
      "native binding 'build_executable_batch_transaction' returned missing fields",
    );
  }
  return {
    signedTransaction: Buffer.from(signed),
    hash: Buffer.from(hashBytes),
  };
}

/**
 * Build, but do not sign, the exact payload submitted to `/v1/fees/quote`.
 * Only the returned payload's `fee_payment` field may be replaced before
 * calling {@link signQuotedTransactionPayload}.
 *
 * @param {{
 *   chainId: string,
 *   authority: string,
 *   instructions: Array<object | string>,
 *   feePayment: object,
 *   metadata?: object | string | null,
 *   creationTimeMs?: number,
 *   ttlMs?: number,
 *   nonce?: number
 * }} input
 * @returns {{payload: object, payloadJson: string, payloadBytes: Buffer, payloadHash: Buffer}}
 */
export function buildTransactionPayload(input) {
  const native = resolveNativeBinding();
  if (!native || typeof native.buildTransactionPayload !== "function") {
    throw new Error("native binding 'build_transaction_payload' is unavailable");
  }
  const {
    chainId,
    authority,
    instructions,
    feePayment,
    metadata = null,
    creationTimeMs = null,
    ttlMs = null,
    nonce = null,
  } = input;
  const result = native.buildTransactionPayload(
    chainId,
    normalizeAuthority(authority),
    serializeInstructionPayloads(instructions, "instructions"),
    feePaymentIntentToNoritoJson(feePayment),
    normalizeMetadataPayload(metadata, "transaction metadata"),
    creationTimeMs,
    ttlMs,
    nonce,
  );
  const payloadJson = result?.payload_json ?? result?.payloadJson ?? null;
  const payloadBytes = result?.payload_bytes ?? result?.payloadBytes ?? null;
  const payloadHash = result?.payload_hash ?? result?.payloadHash ?? null;
  if (typeof payloadJson !== "string" || !payloadBytes || !payloadHash) {
    throw new Error(
      "native binding 'build_transaction_payload' returned missing fields",
    );
  }
  return {
    payload: JSON.parse(payloadJson),
    payloadJson,
    payloadBytes: Buffer.from(payloadBytes),
    payloadHash: Buffer.from(payloadHash),
  };
}

/** Build an exact unsigned ordered mixed executable-batch payload. */
export function buildExecutableBatchTransactionPayload(input) {
  const native = resolveNativeBinding();
  if (
    !native ||
    typeof native.buildExecutableBatchTransactionPayload !== "function"
  ) {
    throw new Error(
      "native binding 'build_executable_batch_transaction_payload' is unavailable",
    );
  }
  const {
    chainId,
    authority,
    entries,
    feePayment,
    metadata = null,
    creationTimeMs = null,
    ttlMs = null,
    nonce = null,
  } = input;
  const { serialized, containsContractCall } =
    serializeExecutableBatchEntries(entries);
  const result = native.buildExecutableBatchTransactionPayload(
    chainId,
    normalizeAuthority(authority),
    serialized,
    requireExecutableBatchGasLimit(feePayment, containsContractCall),
    normalizeMetadataPayload(metadata, "transaction metadata"),
    creationTimeMs,
    ttlMs,
    nonce,
  );
  const payloadJson = result?.payload_json ?? result?.payloadJson ?? null;
  const payloadBytes = result?.payload_bytes ?? result?.payloadBytes ?? null;
  const payloadHash = result?.payload_hash ?? result?.payloadHash ?? null;
  if (typeof payloadJson !== "string" || !payloadBytes || !payloadHash) {
    throw new Error(
      "native binding 'build_executable_batch_transaction_payload' returned missing fields",
    );
  }
  return {
    payload: JSON.parse(payloadJson),
    payloadJson,
    payloadBytes: Buffer.from(payloadBytes),
    payloadHash: Buffer.from(payloadHash),
  };
}

/**
 * Replace only the fee intent in an exact unsigned draft and sign the result.
 * The native boundary rejects any quote that changes the selected authority
 * payer or exact sponsor program and revision.
 *
 * @param {{
 *   payload: object | {payload?: object, payloadJson?: string},
 *   quotedFeePayment: object | string,
 *   privateKey: ArrayBufferView | ArrayBuffer | Buffer,
 *   privateKeyAlgorithm?: string
 * }} input
 * @returns {{signedTransaction: Buffer, hash: Buffer}}
 */
export function signQuotedTransactionPayload(input) {
  const native = resolveNativeBinding();
  if (!native || typeof native.signQuotedTransactionPayload !== "function") {
    throw new Error(
      "native binding 'sign_quoted_transaction_payload' is unavailable",
    );
  }
  const draft = input?.payload;
  const payloadJson =
    typeof draft?.payloadJson === "string"
      ? draft.payloadJson
      : JSON.stringify(draft?.payload ?? draft);
  const quoted = input?.quotedFeePayment;
  const quotedFeePaymentJson =
    typeof quoted === "string"
      ? quoted
      : quoted && typeof quoted === "object" && "payer" in quoted && "value" in quoted
        ? JSON.stringify(quoted)
        : feePaymentIntentToNoritoJson(quoted);
  const result = native.signQuotedTransactionPayload(
    payloadJson,
    quotedFeePaymentJson,
    toBuffer(input?.privateKey),
    input?.privateKeyAlgorithm ?? null,
  );
  const signed = result?.signed_transaction ?? result?.signedTransaction ?? null;
  const hashBytes = result?.hash ?? result?.hashBytes ?? null;
  if (!signed || !hashBytes) {
    throw new Error(
      "native binding 'sign_quoted_transaction_payload' returned missing fields",
    );
  }
  return {
    signedTransaction: Buffer.from(signed),
    hash: Buffer.from(hashBytes),
  };
}

/**
 * Guided fee flow: freeze one unsigned payload, quote it through Torii, replace
 * only the fee limits, and sign the exact result.
 *
 * @param {ToriiClient} client
 * @param {object} input {@link buildTransactionPayload} fields plus private key material
 * @param {{canonicalAuth?: {accountId: string, privateKey: ArrayBufferView | ArrayBuffer | Buffer}, signal?: AbortSignal}} [options]
 * @returns {Promise<{signedTransaction: Buffer, hash: Buffer, draft: object, quote: object}>}
 */
export async function quoteAndSignTransaction(client, input, options = {}) {
  if (!client || typeof client.quoteFees !== "function") {
    throw new TypeError("client must provide quoteFees(payload, options)");
  }
  const {
    privateKey,
    privateKeyAlgorithm = null,
    ...draftInput
  } = input ?? {};
  const draft = buildTransactionPayload(draftInput);
  const canonicalAuth = options.canonicalAuth ?? {
    accountId: draftInput.authority,
    privateKey,
  };
  const quote = await client.quoteFees(draft, {
    canonicalAuth,
    signal: options.signal,
  });
  const signed = signQuotedTransactionPayload({
    payload: draft,
    quotedFeePayment: quote.intent,
    privateKey,
    privateKeyAlgorithm,
  });
  return { ...signed, draft, quote };
}

const SORAFS_PIN_REGISTER_MAX_MANIFEST_BYTES = 512 * 1024;
const SORAFS_PIN_REGISTER_MAX_ALIAS_PROOF_BYTES = 1024 * 1024;

function normalizeSorafsPinRegisterEpoch(value) {
  const epoch = ToriiClient._normalizeUnsignedInteger(
    value,
    "submittedEpoch",
    { allowZero: true },
  );
  if (!Number.isSafeInteger(epoch)) {
    throw new TypeError("submittedEpoch must be a safe uint64 integer");
  }
  return epoch;
}

function normalizeSorafsPinRegisterSegment(value, context) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.trim() !== value ||
    value.length > 128 ||
    !/^[a-z0-9._-]+$/u.test(value)
  ) {
    throw new TypeError(
      `${context} must contain 1..=128 lowercase ASCII letters, digits, '.', '-', or '_'`,
    );
  }
  return value;
}

function normalizeSorafsPinRegisterSuccessor(value) {
  if (value === null || value === undefined) {
    return null;
  }
  let bytes;
  if (typeof value === "string") {
    const exact = value.startsWith("0x") ? value.slice(2) : value;
    if (!/^[0-9a-fA-F]{64}$/u.test(exact)) {
      throw new TypeError("successorOf must be exactly 32 hexadecimal bytes");
    }
    bytes = Buffer.from(exact, "hex");
  } else {
    bytes = toBuffer(value, "successorOf");
  }
  if (bytes.length !== 32 || bytes.every((byte) => byte === 0)) {
    throw new TypeError("successorOf must be exactly 32 non-zero bytes");
  }
  return Array.from(bytes);
}

/**
 * Build the exact native instruction accepted by the signed pin-registration route.
 *
 * @param {{
 *   manifestPayload: ArrayBufferView | ArrayBuffer | Buffer,
 *   submittedEpoch: number | string | bigint,
 *   alias?: {namespace: string, name: string, proof: ArrayBufferView | ArrayBuffer | Buffer} | null,
 *   successorOf?: string | ArrayBufferView | ArrayBuffer | Buffer | null
 * }} input
 * @returns {{RegisterPinManifest: object}}
 */
export function buildRegisterPinManifestInstruction(input) {
  const manifestPayload = toBuffer(input?.manifestPayload, "manifestPayload");
  if (
    manifestPayload.length === 0 ||
    manifestPayload.length > SORAFS_PIN_REGISTER_MAX_MANIFEST_BYTES
  ) {
    throw new TypeError(
      `manifestPayload must contain 1..=${SORAFS_PIN_REGISTER_MAX_MANIFEST_BYTES} bytes`,
    );
  }
  let alias = null;
  if (input?.alias !== null && input?.alias !== undefined) {
    const proof = toBuffer(input.alias.proof, "alias.proof");
    if (
      proof.length === 0 ||
      proof.length > SORAFS_PIN_REGISTER_MAX_ALIAS_PROOF_BYTES
    ) {
      throw new TypeError(
        `alias.proof must contain 1..=${SORAFS_PIN_REGISTER_MAX_ALIAS_PROOF_BYTES} bytes`,
      );
    }
    alias = {
      name: normalizeSorafsPinRegisterSegment(input.alias.name, "alias.name"),
      namespace: normalizeSorafsPinRegisterSegment(
        input.alias.namespace,
        "alias.namespace",
      ),
      proof: proof.toString("base64"),
    };
  }
  return {
    RegisterPinManifest: {
      manifest_payload: manifestPayload.toString("base64"),
      submitted_epoch: normalizeSorafsPinRegisterEpoch(input?.submittedEpoch),
      alias,
      successor_of: normalizeSorafsPinRegisterSuccessor(input?.successorOf),
    },
  };
}

/**
 * Fee-quote and locally sign one pin-registration transaction.
 *
 * @param {ToriiClient} client
 * @param {object} input Transaction draft fields plus pin registration fields.
 * @param {object} [options] Guided quote/sign options.
 * @returns {Promise<{signedTransaction: Buffer, hash: Buffer, draft: object, quote: object}>}
 */
export function buildRegisterPinManifestTransaction(client, input, options = {}) {
  if (input && Object.prototype.hasOwnProperty.call(input, "instructions")) {
    throw new TypeError(
      "buildRegisterPinManifestTransaction fixes instructions to one RegisterPinManifest",
    );
  }
  const {
    manifestPayload,
    submittedEpoch,
    alias = null,
    successorOf = null,
    ...transactionInput
  } = input ?? {};
  const instruction = buildRegisterPinManifestInstruction({
    manifestPayload,
    submittedEpoch,
    alias,
    successorOf,
  });
  return quoteAndSignTransaction(
    client,
    { ...transactionInput, instructions: [instruction] },
    options,
  );
}

/**
 * Build an `ApplySccpRouteGovernance` instruction from one closed atomic action.
 * @param {object} action
 * @returns {{ApplySccpRouteGovernance: {action: object}}}
 */
export function buildApplySccpRouteGovernanceInstruction(action) {
  return {
    ApplySccpRouteGovernance: {
      action: normalizeSccpRouteGovernanceAction(action),
    },
  };
}

/**
 * Build and sign a transaction containing one `ApplySccpRouteGovernance` instruction.
 */
export function buildApplySccpRouteGovernanceTransaction({
  chainId,
  authority,
  feePayment,
  action,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildApplySccpRouteGovernanceInstruction(action);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build, but do not sign, the exact proved-IVM payload submitted to
 * `/v1/fees/quote`.
 *
 * @param {{
 *   chainId: string,
 *   authority: string,
 *   proved: object | string,
 *   attachment: object | string,
 *   feePayment: object,
 *   metadata?: object | string | null,
 *   creationTimeMs?: number,
 *   ttlMs?: number,
 *   nonce?: number
 * }} input
 * @returns {{payload: object, payloadJson: string, payloadBytes: Buffer, payloadHash: Buffer, attachment: object, attachmentJson: string}}
 */
export function buildIvmProvedTransactionPayload(input) {
  const native = resolveNativeBinding();
  if (
    !native ||
    typeof native.buildIvmProvedTransactionPayload !== "function"
  ) {
    throw new Error(
      "native binding 'build_ivm_proved_transaction_payload' is unavailable",
    );
  }

  const {
    chainId,
    authority,
    proved,
    attachment,
    feePayment,
    metadata = null,
    creationTimeMs = null,
    ttlMs = null,
    nonce = null,
  } = input;
  const canonicalAuthority = normalizeAuthority(authority);
  const provedPayload = normalizeJsonObjectPayload(proved, "proved");
  const attachmentPayload = normalizeJsonObjectPayload(
    attachment,
    "attachment",
  );
  const result = native.buildIvmProvedTransactionPayload(
    chainId,
    canonicalAuthority,
    provedPayload,
    attachmentPayload,
    feePaymentIntentToNoritoJson(feePayment),
    normalizeMetadataPayload(metadata, "transaction metadata"),
    creationTimeMs,
    ttlMs,
    nonce,
  );
  const payloadJson = result?.payload_json ?? result?.payloadJson ?? null;
  const payloadBytes = result?.payload_bytes ?? result?.payloadBytes ?? null;
  const payloadHash = result?.payload_hash ?? result?.payloadHash ?? null;
  if (typeof payloadJson !== "string" || !payloadBytes || !payloadHash) {
    throw new Error(
      "native binding 'build_ivm_proved_transaction_payload' returned missing fields",
    );
  }
  return {
    payload: JSON.parse(payloadJson),
    payloadJson,
    payloadBytes: Buffer.from(payloadBytes),
    payloadHash: Buffer.from(payloadHash),
    attachment: JSON.parse(attachmentPayload),
    attachmentJson: attachmentPayload,
  };
}

/**
 * Apply a quote to an exact proved-IVM draft, reattach its proof, and sign it.
 *
 * @param {{
 *   payload: object | {payload?: object, payloadJson?: string, attachment?: object | string},
 *   attachment?: object | string,
 *   quotedFeePayment: object | string,
 *   privateKey: ArrayBufferView | ArrayBuffer | Buffer,
 *   privateKeyAlgorithm?: string
 * }} input
 * @returns {{signedTransaction: Buffer, hash: Buffer}}
 */
export function signQuotedIvmProvedTransactionPayload(input) {
  const native = resolveNativeBinding();
  if (
    !native ||
    typeof native.signQuotedIvmProvedTransactionPayload !== "function"
  ) {
    throw new Error(
      "native binding 'sign_quoted_ivm_proved_transaction_payload' is unavailable",
    );
  }
  const draft = input?.payload;
  const payloadJson =
    typeof draft?.payloadJson === "string"
      ? draft.payloadJson
      : JSON.stringify(draft?.payload ?? draft);
  const attachment = input?.attachment ?? draft?.attachment;
  const quoted = input?.quotedFeePayment;
  const quotedFeePaymentJson =
    typeof quoted === "string"
      ? quoted
      : quoted && typeof quoted === "object" && "payer" in quoted && "value" in quoted
        ? JSON.stringify(quoted)
        : feePaymentIntentToNoritoJson(quoted);
  const result = native.signQuotedIvmProvedTransactionPayload(
    payloadJson,
    normalizeJsonObjectPayload(attachment, "attachment"),
    quotedFeePaymentJson,
    toBuffer(input?.privateKey),
    input?.privateKeyAlgorithm ?? null,
  );
  const signed =
    result?.signed_transaction ?? result?.signedTransaction ?? null;
  const hashBytes = result?.hash ?? result?.hashBytes ?? null;
  if (!signed || !hashBytes) {
    throw new Error(
      "native binding 'sign_quoted_ivm_proved_transaction_payload' returned missing fields",
    );
  }
  return {
    signedTransaction: Buffer.from(signed),
    hash: Buffer.from(hashBytes),
  };
}

/**
 * Build and sign a transaction whose executable is `Executable::IvmProved`.
 * @param {{
 *   chainId: string,
 *   authority: string,
 *   proved: object | string,
 *   attachment: object | string,
 *   feePayment: object,
 *   metadata?: object | string | null,
 *   creationTimeMs?: number,
 *   ttlMs?: number,
 *   nonce?: number,
 *   privateKey: ArrayBufferView | ArrayBuffer | Buffer
 * }} input
 * @returns {{signedTransaction: Buffer, hash: Buffer}}
 */
export function buildIvmProvedTransaction(input) {
  const native = resolveNativeBinding();
  if (!native || typeof native.buildIvmProvedTransaction !== "function") {
    throw new Error(
      "native binding 'build_ivm_proved_transaction' is unavailable",
    );
  }

  const {
    chainId,
    authority,
    proved,
    attachment,
    feePayment,
    metadata = null,
    creationTimeMs = null,
    ttlMs = null,
    nonce = null,
    privateKey,
    privateKeyAlgorithm = null,
  } = input;

  const canonicalAuthority = normalizeAuthority(authority);
  const provedPayload = normalizeJsonObjectPayload(proved, "proved");
  const attachmentPayload = normalizeJsonObjectPayload(
    attachment,
    "attachment",
  );
  const metadataPayload = normalizeMetadataPayload(
    metadata,
    "transaction metadata",
  );
  const result = native.buildIvmProvedTransaction(
    chainId,
    canonicalAuthority,
    provedPayload,
    attachmentPayload,
    feePaymentIntentToNoritoJson(feePayment),
    metadataPayload,
    creationTimeMs,
    ttlMs,
    nonce,
    toBuffer(privateKey),
    privateKeyAlgorithm,
  );

  const signed =
    result?.signed_transaction ?? result?.signedTransaction ?? null;
  const hashBytes = result?.hash ?? result?.hashBytes ?? null;
  if (!signed || !hashBytes) {
    throw new Error(
      "native binding 'build_ivm_proved_transaction' returned missing fields",
    );
  }

  return {
    signedTransaction: Buffer.from(signed),
    hash: Buffer.from(hashBytes),
  };
}

const IVM_PROVED_CONTRACT_METADATA_KEYS = new Set([
  "contract_address",
  "contract_alias",
  "contract_entrypoint",
  "contract_payload",
  "gas_asset_id",
  "fee_sponsor",
  "gas_limit",
  "validation_fee_policy_version",
  "validation_fee_policy_hash",
  "validation_fee_instruction_index",
  "validation_fee_transfer_entry_index",
]);

function readExclusiveInputAlias(record, aliases, context) {
  const supplied = [];
  for (const alias of aliases) {
    if (!Object.prototype.hasOwnProperty.call(record, alias)) continue;
    const descriptor = Object.getOwnPropertyDescriptor(record, alias);
    if (!descriptor || !("value" in descriptor) || !descriptor.enumerable) {
      throw new TypeError(`${context}.${alias} must be an enumerable data property`);
    }
    const value = descriptor.value;
    if (value !== undefined) supplied.push({ alias, value });
  }
  if (supplied.length > 1) {
    throw new TypeError(
      `${context} must use exactly one of ${aliases.join(", ")}`,
    );
  }
  return supplied.length === 0 ? undefined : supplied[0].value;
}

function readOwnEnumerableDataValue(record, key, context) {
  if (
    record === null ||
    (typeof record !== "object" && typeof record !== "function") ||
    !Object.prototype.hasOwnProperty.call(record, key)
  ) {
    return undefined;
  }
  const descriptor = Object.getOwnPropertyDescriptor(record, key);
  if (!descriptor || !("value" in descriptor) || !descriptor.enumerable) {
    throw new TypeError(`${context}.${key} must be an enumerable data property`);
  }
  return descriptor.value;
}

function readExactInstructionVariant(record, supportedKeys, context) {
  if (!record || typeof record !== "object" || Array.isArray(record)) {
    return null;
  }
  const ownKeys = Reflect.ownKeys(record);
  if (
    ownKeys.length !== 1 ||
    typeof ownKeys[0] !== "string" ||
    !supportedKeys.includes(ownKeys[0])
  ) {
    return null;
  }
  const name = ownKeys[0];
  return {
    name,
    value: readOwnEnumerableDataValue(record, name, context),
  };
}

function hasExactEnumerableDataShape(record, expectedKeys, context) {
  if (!record || typeof record !== "object" || Array.isArray(record)) {
    return false;
  }
  const ownKeys = Reflect.ownKeys(record);
  if (
    ownKeys.length !== expectedKeys.length ||
    ownKeys.some(
      (key) => typeof key !== "string" || !expectedKeys.includes(key),
    )
  ) {
    return false;
  }
  for (const key of expectedKeys) {
    readOwnEnumerableDataValue(record, key, context);
  }
  return true;
}

function normalizeIvmProvedContractMetadata(value) {
  if (value === undefined || value === null) {
    return {};
  }
  let record = value;
  if (typeof value === "string") {
    try {
      record = JSON.parse(value);
    } catch (error) {
      throw new TypeError(`metadata must be valid JSON: ${error?.message ?? error}`);
    }
  }
  record = normalizePlainObject(record, "metadata");
  for (const key of IVM_PROVED_CONTRACT_METADATA_KEYS) {
    if (Object.prototype.hasOwnProperty.call(record, key)) {
      throw new TypeError(
        `metadata.${key} is reserved by submitIvmProvedContractCall`,
      );
    }
  }
  return snapshotJsonValue(record, "metadata", 64 * 1024);
}

function canonicalJsonValue(value) {
  if (Array.isArray(value)) {
    return `[${value.map((entry) => canonicalJsonValue(entry)).join(",")}]`;
  }
  if (value && typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map(
        (key) =>
          `${JSON.stringify(key)}:${canonicalJsonValue(value[key])}`,
      )
      .join(",")}}`;
  }
  return JSON.stringify(value);
}

function snapshotJsonValue(value, context, maxBytes = 1024 * 1024) {
  let encoded;
  try {
    encoded = JSON.stringify(value);
  } catch (error) {
    throw new TypeError(`${context} must be JSON-serializable: ${error?.message ?? error}`);
  }
  if (encoded === undefined) {
    throw new TypeError(`${context} must be a JSON value`);
  }
  if (Buffer.byteLength(encoded, "utf8") > maxBytes) {
    throw new RangeError(`${context} exceeds ${maxBytes} serialized bytes`);
  }
  return JSON.parse(encoded);
}

const IVM_ARTIFACT_MAX_BASE64_LENGTH =
  Math.ceil(IVM_ARTIFACT_MAX_BYTES / 3) * 4;

function hasExactStandardBase64Shape(value) {
  if (value.length === 0 || value.length % 4 !== 0) return false;
  const padding = value.endsWith("==") ? 2 : value.endsWith("=") ? 1 : 0;
  const dataLength = value.length - padding;
  for (let index = 0; index < dataLength; index += 1) {
    const code = value.charCodeAt(index);
    if (
      !(
        (code >= 0x41 && code <= 0x5a) ||
        (code >= 0x61 && code <= 0x7a) ||
        (code >= 0x30 && code <= 0x39) ||
        code === 0x2b ||
        code === 0x2f
      )
    ) {
      return false;
    }
  }
  return true;
}

function normalizeExactBase64(value, context) {
  if (
    typeof value === "string" &&
    value.length > IVM_ARTIFACT_MAX_BASE64_LENGTH
  ) {
    throw new RangeError(
      `${context} exceeds the ${IVM_ARTIFACT_MAX_BYTES}-byte artifact limit`,
    );
  }
  if (typeof value !== "string" || value.length === 0) {
    throw new TypeError(`${context} must be non-empty canonical standard base64`);
  }
  const padding = value.endsWith("==") ? 2 : value.endsWith("=") ? 1 : 0;
  if (
    value.length % 4 === 0 &&
    (value.length / 4) * 3 - padding > IVM_ARTIFACT_MAX_BYTES
  ) {
    throw new RangeError(
      `${context} exceeds the ${IVM_ARTIFACT_MAX_BYTES}-byte artifact limit`,
    );
  }
  if (!hasExactStandardBase64Shape(value)) {
    throw new TypeError(`${context} must be non-empty canonical standard base64`);
  }
  const bytes = Buffer.from(value, "base64");
  if (bytes.length === 0 || bytes.toString("base64") !== value) {
    throw new TypeError(`${context} must be non-empty canonical standard base64`);
  }
  return { bytes, base64: value };
}

function requireExactContractCodeBytesResponse(value) {
  if (
    value === null ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    Object.getPrototypeOf(value) !== Object.prototype
  ) {
    throw new TypeError(
      "deployed contract bytecode response must be a plain object",
    );
  }
  const ownKeys = Reflect.ownKeys(value);
  if (ownKeys.length !== 1 || ownKeys[0] !== "code_b64") {
    throw new TypeError(
      "deployed contract bytecode response must contain exactly the code_b64 field",
    );
  }
  const descriptor = Object.getOwnPropertyDescriptor(value, "code_b64");
  if (!descriptor || !("value" in descriptor) || !descriptor.enumerable) {
    throw new TypeError(
      "deployed contract bytecode response code_b64 must be an enumerable data property",
    );
  }
  return descriptor.value;
}

function throwIfSubmissionAborted(signal) {
  if (!signal) return;
  let aborted;
  let reason;
  if (submissionAbortSignalAbortedGetter !== null) {
    try {
      aborted = submissionAbortSignalAbortedGetter.call(signal);
      reason = submissionAbortSignalReasonGetter?.call(signal);
    } catch {
      // AbortSignal-like fallbacks are read only from own data properties.
    }
  }
  if (aborted === undefined) {
    const abortedDescriptor = Object.getOwnPropertyDescriptor(signal, "aborted");
    if (
      !abortedDescriptor ||
      !("value" in abortedDescriptor) ||
      typeof abortedDescriptor.value !== "boolean"
    ) {
      throw new TypeError("signal.aborted must be an own boolean data property");
    }
    aborted = abortedDescriptor.value;
    const reasonDescriptor = Object.getOwnPropertyDescriptor(signal, "reason");
    reason = reasonDescriptor && "value" in reasonDescriptor
      ? reasonDescriptor.value
      : undefined;
  }
  if (aborted) {
    throw reason ?? new Error("The operation was aborted");
  }
}

function normalizeIvmCodeHashHex(value, context) {
  if (typeof value !== "string" || !/^[0-9a-fA-F]{64}$/u.test(value)) {
    throw new TypeError(`${context} must be exactly 32 hexadecimal bytes`);
  }
  return value.toLowerCase();
}

function normalizeIvmVerifyingKeyRef(value, context) {
  const record = normalizePlainObject(value, context);
  const keys = Object.keys(record).sort();
  if (keys.length !== 2 || keys[0] !== "backend" || keys[1] !== "name") {
    throw new TypeError(`${context} must contain exactly backend and name`);
  }
  const backend = record.backend;
  const name = record.name;
  if (
    typeof backend !== "string" ||
    backend.length === 0 ||
    backend.trim() !== backend
  ) {
    throw new TypeError(`${context}.backend must be an exact non-empty string`);
  }
  if (
    typeof name !== "string" ||
    name.length === 0 ||
    name.trim() !== name ||
    name.includes(":")
  ) {
    throw new TypeError(
      `${context}.name must be an exact non-empty string without ':'`,
    );
  }
  return { backend, name };
}

function assertZkModeIvmBytecode(
  bytecodeBase64,
  expectedCodeHashHex,
  expectedArtifactSha256Hex,
) {
  const { bytes: bytecode, base64 } = normalizeExactBase64(
    bytecodeBase64,
    "deployed contract bytecode",
  );
  if (
    bytecode.length < IVM_PROGRAM_HEADER_LENGTH ||
    !bytecode.subarray(0, 4).equals(Buffer.from([0x49, 0x56, 0x4d, 0x00]))
  ) {
    throw new Error("deployed contract bytecode has an invalid IVM header");
  }
  if ((bytecode[6] & 0x01) === 0) {
    throw new Error(
      "deployed contract bytecode is not ZK mode; compile and deploy the artifact with --force-zk",
    );
  }
  const {
    codeHashHex: actualCodeHashHex,
    artifactSha256Hex: actualArtifactSha256Hex,
  } = computeIvmArtifactHashes(bytecode);
  if (actualArtifactSha256Hex !== expectedArtifactSha256Hex) {
    throw new Error(
      `deployed contract artifact SHA-256 ${actualArtifactSha256Hex} does not match caller-trusted expected artifact SHA-256 ${expectedArtifactSha256Hex}`,
    );
  }
  if (actualCodeHashHex !== expectedCodeHashHex) {
    throw new Error(
      `deployed contract bytecode hash ${actualCodeHashHex} does not match expected code hash ${expectedCodeHashHex}`,
    );
  }
  return base64;
}

function assertIvmProvedBytecodeBinding(proved, expectedBytecode, context) {
  const record = normalizePlainObject(proved, context);
  const { base64 } = normalizeExactBase64(
    record.bytecode,
    `${context}.bytecode`,
  );
  if (base64 !== expectedBytecode) {
    throw new Error(
      `${context}.bytecode differs from the code-hash-bound deployed contract bytecode`,
    );
  }
}

function assertIvmProofAttachmentBinding(attachment, expectedVkRef) {
  const record = normalizePlainObject(attachment, "proof attachment");
  if (record.backend !== expectedVkRef.backend) {
    throw new Error(
      "proof attachment backend differs from the requested verifying-key backend",
    );
  }
  const proof = normalizePlainObject(record.proof, "proof attachment.proof");
  if (proof.backend !== expectedVkRef.backend) {
    throw new Error(
      "proof attachment proof backend differs from the requested verifying-key backend",
    );
  }
  const actualVkRef = normalizeIvmVerifyingKeyRef(
    record.vk_ref,
    "proof attachment.vk_ref",
  );
  if (
    actualVkRef.backend !== expectedVkRef.backend ||
    actualVkRef.name !== expectedVkRef.name
  ) {
    throw new Error(
      "proof attachment vk_ref differs from the requested verifying-key reference",
    );
  }
}

function assertRequiredOverlayTransfer(proved, requiredTransfer, context) {
  if (requiredTransfer === undefined || requiredTransfer === null) {
    return null;
  }
  const overlay = proved?.overlay;
  if (!Array.isArray(overlay)) {
    throw new TypeError(`${context}.overlay must be an array`);
  }
  const expectedCanonical = canonicalJsonValue(requiredTransfer);
  const matches = overlay.filter(
    (instruction, instructionIndex) =>
      canonicalJsonValue(
        decodeOverlayInstruction(
          instruction,
          `${context}.overlay[${instructionIndex}]`,
        ),
      ) === expectedCanonical,
  ).length;
  if (matches !== 1) {
    throw new Error(
      `${context} must contain the required overlay transfer exactly once (found ${matches})`,
    );
  }
  return requiredTransfer;
}

function normalizeRequiredOverlayTransfer(requiredTransfer) {
  const transfer = normalizePlainObject(
    requiredTransfer,
    "requiredOverlayTransfer",
  );
  return buildTransferAssetInstruction({
    sourceAssetHoldingId: readExclusiveInputAlias(
      transfer,
      [
        "sourceAssetHoldingId",
        "source_asset_holding_id",
        "sourceAssetId",
        "source_asset_id",
      ],
      "requiredOverlayTransfer.sourceAssetHoldingId",
    ),
    quantity: transfer.quantity,
    destinationAccountId: readExclusiveInputAlias(
      transfer,
      ["destinationAccountId", "destination_account_id"],
      "requiredOverlayTransfer.destinationAccountId",
    ),
  });
}

function directAssetTransfer(instruction) {
  const instructionVariant = readExactInstructionVariant(
    instruction,
    ["Transfer"],
    "overlay instruction",
  );
  if (instructionVariant === null) return null;
  const transferVariant = instructionVariant.value;
  if (
    transferVariant === undefined ||
    transferVariant === null ||
    typeof transferVariant !== "object" ||
    Array.isArray(transferVariant)
  ) {
    return null;
  }
  const assetVariant = readExactInstructionVariant(
    transferVariant,
    ["Asset"],
    "overlay instruction.Transfer",
  );
  if (assetVariant === null) return null;
  const transfer = assetVariant.value;
  if (
    !hasExactEnumerableDataShape(
      transfer,
      ["source", "object", "destination"],
      "overlay instruction.Transfer.Asset",
    )
  ) {
    return null;
  }
  const source = readOwnEnumerableDataValue(
    transfer,
    "source",
    "overlay instruction.Transfer.Asset",
  );
  if (typeof source !== "string") return null;
  const separator = source.indexOf("#");
  if (separator <= 0 || separator === source.length - 1) return null;
  return {
    assetDefinitionId: source.slice(0, separator),
    sourceAccountId: source.slice(separator + 1),
    destinationAccountId: readOwnEnumerableDataValue(
      transfer,
      "destination",
      "overlay instruction.Transfer.Asset",
    ),
    quantity: String(
      readOwnEnumerableDataValue(
        transfer,
        "object",
        "overlay instruction.Transfer.Asset",
      ),
    ),
  };
}

function batchAssetTransfers(instruction) {
  const variant = readExactInstructionVariant(
    instruction,
    ["TransferAssetBatch", "transfer_asset_batch", "AssetTransferBatch"],
    "overlay instruction",
  );
  if (variant === null) return null;
  const batch = variant.value;
  if (
    !hasExactEnumerableDataShape(
      batch,
      ["entries"],
      "overlay instruction TransferAssetBatch",
    )
  ) {
    throw new TypeError(
      "overlay instruction TransferAssetBatch must contain exactly entries",
    );
  }
  const entries = readOwnEnumerableDataValue(
    batch,
    "entries",
    "overlay instruction TransferAssetBatch",
  );
  if (!batch || typeof batch !== "object" || !Array.isArray(entries)) {
    throw new TypeError(
      "overlay instruction TransferAssetBatch.entries must be an array",
    );
  }
  return entries.map((entry, entryIndex) => {
    if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
      throw new TypeError(
        `overlay instruction TransferAssetBatch.entries[${entryIndex}] must be an object`,
      );
    }
    if (
      !hasExactEnumerableDataShape(
        entry,
        ["from", "to", "asset_definition", "amount"],
        `overlay instruction TransferAssetBatch.entries[${entryIndex}]`,
      )
    ) {
      throw new TypeError(
        `overlay instruction TransferAssetBatch.entries[${entryIndex}] must contain exactly from, to, asset_definition, and amount`,
      );
    }
    return {
      assetDefinitionId: readOwnEnumerableDataValue(
        entry,
        "asset_definition",
        `overlay instruction TransferAssetBatch.entries[${entryIndex}]`,
      ),
      sourceAccountId: readOwnEnumerableDataValue(
        entry,
        "from",
        `overlay instruction TransferAssetBatch.entries[${entryIndex}]`,
      ),
      destinationAccountId: readOwnEnumerableDataValue(
        entry,
        "to",
        `overlay instruction TransferAssetBatch.entries[${entryIndex}]`,
      ),
      quantity: String(
        readOwnEnumerableDataValue(
          entry,
          "amount",
          `overlay instruction TransferAssetBatch.entries[${entryIndex}]`,
        ),
      ),
    };
  });
}

function decodeOverlayInstruction(value, context) {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return value;
  }
  if (typeof value !== "string" || value.length === 0 || value.trim() !== value) {
    throw new TypeError(
      `${context} must be a base64 Norito InstructionBox or decoded instruction object`,
    );
  }
  const bytes = Buffer.from(value, "base64");
  if (bytes.length === 0 || bytes.toString("base64") !== value) {
    throw new TypeError(`${context} must be exact standard base64`);
  }
  try {
    return noritoDecodeInstruction(bytes);
  } catch (error) {
    throw new Error(
      `${context} could not be decoded as a canonical Norito InstructionBox: ${error?.message ?? error}`,
    );
  }
}

function multisigPropose(instruction) {
  const variant = readExactInstructionVariant(
    instruction,
    ["Custom", "MultisigPropose"],
    "overlay instruction",
  );
  if (variant === null) return null;
  let proposal = variant.value;
  if (variant.name === "Custom") {
    if (
      !hasExactEnumerableDataShape(
        variant.value,
        ["payload"],
        "overlay instruction.Custom",
      )
    ) {
      throw new TypeError(
        "overlay instruction.Custom must contain exactly payload",
      );
    }
    const payload = readOwnEnumerableDataValue(
      variant.value,
      "payload",
      "overlay instruction.Custom",
    );
    if (
      !hasExactEnumerableDataShape(
        payload,
        ["Propose"],
        "overlay instruction.Custom.payload",
      )
    ) {
      throw new TypeError(
        "overlay instruction.Custom.payload must contain exactly Propose",
      );
    }
    proposal = readOwnEnumerableDataValue(
      payload,
      "Propose",
      "overlay instruction.Custom.payload",
    );
  }
  if (proposal === undefined || proposal === null) {
    return null;
  }
  if (typeof proposal !== "object" || Array.isArray(proposal)) {
    throw new TypeError("MultisigPropose payload must be an object");
  }
  if (
    !hasExactEnumerableDataShape(
      proposal,
      ["account", "instructions"],
      "MultisigPropose",
    )
  ) {
    throw new TypeError(
      "MultisigPropose must contain exactly account and instructions",
    );
  }
  const instructions = readOwnEnumerableDataValue(
    proposal,
    "instructions",
    "MultisigPropose",
  );
  if (!Array.isArray(instructions)) {
    throw new TypeError("MultisigPropose.instructions must be an array");
  }
  const account = readOwnEnumerableDataValue(
    proposal,
    "account",
    "MultisigPropose",
  );
  if (typeof account !== "string" || account.length === 0) {
    throw new TypeError("MultisigPropose.account must be an account id");
  }
  return { account, instructions };
}

function collectOverlayTransferContexts(overlay, authority, context) {
  const contexts = [
    {
      contextIndex: 0,
      executionAccountId: authority,
      nested: false,
      transfers: [],
    },
  ];

  function collect(instructions, transferContext) {
    for (
      let instructionIndex = 0;
      instructionIndex < instructions.length;
      instructionIndex += 1
    ) {
      const instruction = decodeOverlayInstruction(
        instructions[instructionIndex],
        `${context}.overlay[${instructionIndex}]`,
      );
      const direct = directAssetTransfer(instruction);
      const batch = batchAssetTransfers(instruction);
      const proposal = multisigPropose(instruction);
      const recognizedKinds =
        Number(direct !== null) +
        Number(batch !== null) +
        Number(proposal !== null);
      if (recognizedKinds !== 1) {
        throw new Error(
          `${context}.overlay[${instructionIndex}] is not one unambiguous explicit asset transfer, transfer batch, or recursive multisig proposal; validation-fee submission fails closed on other instruction families`,
        );
      }
      if (direct) {
        transferContext.transfers.push({
          ...direct,
          contextIndex: transferContext.contextIndex,
          instructionIndex,
          transferEntryIndex: null,
        });
      }
      for (
        let transferEntryIndex = 0;
        transferEntryIndex < (batch?.length ?? 0);
        transferEntryIndex += 1
      ) {
        transferContext.transfers.push({
          ...batch[transferEntryIndex],
          contextIndex: transferContext.contextIndex,
          instructionIndex,
          transferEntryIndex,
        });
      }

      if (proposal) {
        const nestedContext = {
          contextIndex: contexts.length,
          executionAccountId: proposal.account,
          nested: true,
          transfers: [],
        };
        contexts.push(nestedContext);
        collect(proposal.instructions, nestedContext);
      }
    }
  }

  collect(overlay, contexts[0]);
  return contexts;
}

function sameFeeCoordinate(transfer, binding) {
  return (
    transfer.instructionIndex === binding.instructionIndex &&
    transfer.transferEntryIndex === binding.transferEntryIndex
  );
}

function validationFeeScaledUnits(value, scale, context) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a canonical Quantity string`);
  }
  let quantity;
  try {
    quantity = NumericV1.decodeQuantityJson(value);
  } catch (error) {
    if (!(error instanceof NumericV1Error)) throw error;
    throw new TypeError(`${context} must be canonical (${error.code})`);
  }
  if (quantity.scale > scale) {
    throw new RangeError(
      `${context} uses scale ${quantity.scale}, above policy scale ${scale}`,
    );
  }
  return quantity.mantissa * 10n ** BigInt(scale - quantity.scale);
}

/**
 * Resolve and simulate a deployed contract entrypoint, derive its authoritative
 * ZK IVM overlay, have the node prove that same overlay, sign the exact proved
 * executable, and submit it to the transaction pipeline.
 *
 * `requiredOverlayTransfer` is an assertion, not an appended instruction: the
 * deployed router/pool call must emit that exact transfer itself. This keeps the
 * transfer inside both the node-generated proof commitment and the user-signed
 * transaction payload. Pre-release signed/keyset validation-fee inputs are
 * rejected; only the ledger-native Parliament registry may authorize those
 * reserved policy fields.
 *
 * @param {ToriiClient} client
 * @param {object} input
 * @param {object} [options]
 * @returns {Promise<object>}
 */
export async function submitIvmProvedContractCall(client, input, options = {}) {
  if (!(client instanceof ToriiClient)) {
    throw new TypeError("client must be an instance of ToriiClient");
  }
  const record = normalizePlainObject(input, "input");
  const opts = normalizePlainObject(options, "options");
  if (Object.prototype.hasOwnProperty.call(opts, "transactionStatusScope")) {
    throw new TypeError(
      "options.transactionStatusScope is unsupported; finality waits always use global scope",
    );
  }
  const { signal } = ToriiClient._normalizeOptionsWithSignal(
    opts.signal === undefined ? {} : { signal: opts.signal },
    "submitIvmProvedContractCall",
  );
  if (opts.waitForCommit !== undefined && typeof opts.waitForCommit !== "boolean") {
    throw new TypeError("options.waitForCommit must be a boolean");
  }
  const proofIntervalMs =
    opts.proofIntervalMs === undefined
      ? undefined
      : ToriiClient._normalizeUnsignedInteger(
          opts.proofIntervalMs,
          "options.proofIntervalMs",
          { allowZero: true },
        );
  const proofTimeoutMs =
    opts.proofTimeoutMs === undefined
      ? undefined
      : opts.proofTimeoutMs === null
        ? null
        : ToriiClient._normalizeUnsignedInteger(
            opts.proofTimeoutMs,
            "options.proofTimeoutMs",
            { allowZero: true },
          );
  const hasTransactionPollOptions =
    opts.transactionIntervalMs !== undefined ||
    opts.transactionTimeoutMs !== undefined ||
    opts.waitForCommit === true;
  const transactionPollOptions = hasTransactionPollOptions
    ? ToriiClient._normalizeTransactionStatusPollOptions(
        {
          ...(opts.transactionIntervalMs === undefined
            ? {}
            : { intervalMs: opts.transactionIntervalMs }),
          ...(opts.transactionTimeoutMs === undefined
            ? {}
            : { timeoutMs: opts.transactionTimeoutMs }),
          ...(signal === undefined ? {} : { signal }),
        },
        "submitIvmProvedContractCall transaction status options",
      )
    : null;
  const authority = normalizeAuthority(record.authority);
  const chainIdValue = readExclusiveInputAlias(
    record, ["chainId", "chain_id"], "input.chainId",
  );
  const chainId = normalizeNonEmptyString(chainIdValue, "input.chainId");
  if (chainId !== chainIdValue) {
    throw new TypeError("input.chainId must not contain surrounding whitespace");
  }
  const expectedCodeHashHex = normalizeIvmCodeHashHex(
    readExclusiveInputAlias(
      record,
      ["expectedCodeHashHex", "expected_code_hash_hex"],
      "input.expectedCodeHashHex",
    ),
    "input.expectedCodeHashHex",
  );
  const expectedArtifactSha256Hex = normalizeIvmCodeHashHex(
    readExclusiveInputAlias(
      record,
      ["expectedArtifactSha256Hex", "expected_artifact_sha256_hex"],
      "input.expectedArtifactSha256Hex",
    ),
    "input.expectedArtifactSha256Hex",
  );
  const vkRef = normalizeIvmVerifyingKeyRef(
    readExclusiveInputAlias(record, ["vkRef", "vk_ref"], "input.vkRef"),
    "input.vkRef",
  );
  const privateKeyValue = readExclusiveInputAlias(
    record,
    ["privateKey", "private_key"],
    "input.privateKey",
  );
  const privateKey = Buffer.from(toBuffer(privateKeyValue, "input.privateKey"));
  const privateKeyAlgorithmValue = readExclusiveInputAlias(
    record,
    ["privateKeyAlgorithm", "private_key_algorithm"],
    "input.privateKeyAlgorithm",
  );
  if (
    typeof privateKeyAlgorithmValue === "string" &&
    privateKeyAlgorithmValue.trim() !== privateKeyAlgorithmValue
  ) {
    throw new TypeError(
      "input.privateKeyAlgorithm must not contain surrounding whitespace",
    );
  }
  const privateKeyAlgorithm = normalizeCryptoAlgorithm(
    privateKeyAlgorithmValue ?? undefined,
  );
  if (privateKeyAlgorithm === CRYPTO_ALGORITHMS.ED25519) {
    if (privateKey.length !== 32 && privateKey.length !== 64) {
      throw new TypeError("input.privateKey must be a 32- or 64-byte Ed25519 key");
    }
  } else {
    // Parse the algorithm-specific key before creating any proof-side effects.
    publicKeyFromPrivate(privateKey, { algorithm: privateKeyAlgorithm });
  }
  const contractAddressValue = readExclusiveInputAlias(
    record,
    ["contractAddress", "contract_address"],
    "input.contractAddress",
  );
  const contractAliasValue = readExclusiveInputAlias(
    record,
    ["contractAlias", "contract_alias"],
    "input.contractAlias",
  );
  if (
    (contractAddressValue === undefined || contractAddressValue === null) ===
    (contractAliasValue === undefined || contractAliasValue === null)
  ) {
    throw new TypeError(
      "input must provide exactly one of contractAddress or contractAlias",
    );
  }
  const contractAddress =
    contractAddressValue === undefined || contractAddressValue === null
      ? null
      : normalizeNonEmptyString(contractAddressValue, "input.contractAddress");
  const contractAlias =
    contractAliasValue === undefined || contractAliasValue === null
      ? null
      : normalizeNonEmptyString(contractAliasValue, "input.contractAlias");
  if (
    (contractAddress !== null && contractAddress !== contractAddressValue) ||
    (contractAlias !== null && contractAlias !== contractAliasValue)
  ) {
    throw new TypeError(
      "input contract selector must not contain surrounding whitespace",
    );
  }
  const entrypoint =
    record.entrypoint === undefined || record.entrypoint === null
      ? null
      : normalizeNonEmptyString(record.entrypoint, "input.entrypoint");
  if (entrypoint !== null && entrypoint !== record.entrypoint) {
    throw new TypeError("input.entrypoint must not contain surrounding whitespace");
  }
  const payload =
    record.payload === undefined
      ? undefined
      : snapshotJsonValue(record.payload, "input.payload");
  for (const retired of ["gasLimit", "gas_limit", "gasAssetId", "gas_asset_id", "feeSponsor", "fee_sponsor"]) {
    if (Object.prototype.hasOwnProperty.call(record, retired)) {
      throw new TypeError(
        `input.${retired} is retired; use the signature-bound feePayment field`,
      );
    }
  }
  const feePayment = readExclusiveInputAlias(
    record,
    ["feePayment", "fee_payment"],
    "input.feePayment",
  );
  // Serialize once before any proof-side effect so malformed or unbounded
  // intents fail before simulation and proving work is started.
  feePaymentIntentToNoritoJson(feePayment);
  if (feePayment.gasLimit === undefined || feePayment.gasLimit === null) {
    throw new TypeError("input.feePayment.gasLimit is required for an IVM transaction");
  }
  const gasLimit = ToriiClient._normalizeUnsignedInteger(
    feePayment.gasLimit,
    "input.feePayment.gasLimit",
    { allowZero: false },
  );
  const metadataInput = normalizeIvmProvedContractMetadata(record.metadata);
  function optionalTransactionInteger(aliases, context, { positive = false } = {}) {
    const value = readExclusiveInputAlias(record, aliases, context);
    if (value === undefined || value === null) return null;
    const normalized = ToriiClient._normalizeUnsignedInteger(value, context, {
      allowZero: !positive,
    });
    if (positive && normalized === 0) {
      throw new RangeError(`${context} must be positive`);
    }
    return normalized;
  }
  const creationTimeMs = optionalTransactionInteger(
    ["creationTimeMs", "creation_time_ms"],
    "input.creationTimeMs",
  );
  const ttlMs = optionalTransactionInteger(
    ["ttlMs", "ttl_ms"],
    "input.ttlMs",
  );
  const nonce = optionalTransactionInteger(["nonce"], "input.nonce", {
    positive: true,
  });
  if (nonce !== null && nonce > 0xffff_ffff) {
    throw new RangeError("input.nonce must fit in u32");
  }
  const callerRequiredTransferValue = readExclusiveInputAlias(
    record,
    ["requiredOverlayTransfer", "required_overlay_transfer"],
    "input.requiredOverlayTransfer",
  );
  const callerRequiredTransfer =
    callerRequiredTransferValue === undefined ||
    callerRequiredTransferValue === null
      ? null
      : normalizeRequiredOverlayTransfer(callerRequiredTransferValue);
  const validationFeeIntent = readExclusiveInputAlias(
    record,
    ["validationFeePolicy", "validation_fee_policy"],
    "input.validationFeePolicy",
  );
  if (validationFeeIntent !== undefined && validationFeeIntent !== null) {
    throw new TypeError(
      "input.validationFeePolicy is retired; validation-fee authority comes only from a locally verified Parliament registry proof",
    );
  }
  const simulationRequest = {
    authority,
    ...(contractAddress === null ? {} : { contractAddress }),
    ...(contractAlias === null ? {} : { contractAlias }),
    ...(entrypoint === null ? {} : { entrypoint }),
    ...(payload === undefined ? {} : { payload }),
    gasLimit,
  };
  const requestOptions = signal === undefined ? {} : { signal };
  const simulation = await client.simulateContractCall(
    simulationRequest,
    requestOptions,
  );
  if (!simulation.ok) {
    throw new Error(
      `contract call simulation failed: ${simulation.error ?? "unknown VM error"}`,
    );
  }
  if (!simulation.contract_address) {
    throw new Error("contract call simulation did not resolve a contract address");
  }
  if (
    contractAddress !== null &&
    simulation.contract_address !== contractAddress
  ) {
    throw new Error(
      "contract call simulation resolved a different contract address than requested",
    );
  }
  if (simulation.gas_limit !== gasLimit) {
    throw new Error(
      `contract call simulation gas limit ${simulation.gas_limit} does not match requested gas limit ${gasLimit}`,
    );
  }
  if (entrypoint !== null && simulation.entrypoint !== entrypoint) {
    throw new Error(
      "contract call simulation resolved a different entrypoint than requested",
    );
  }
  if (
    payload !== undefined &&
    (simulation.normalized_payload === null ||
      canonicalJsonValue(simulation.normalized_payload) !==
        canonicalJsonValue(payload))
  ) {
    throw new Error(
      "contract call simulation normalized payload differs from the requested payload",
    );
  }
  const simulationCodeHashHex = normalizeIvmCodeHashHex(
    simulation.code_hash_hex,
    "contract call simulation code_hash_hex",
  );
  if (simulationCodeHashHex !== expectedCodeHashHex) {
    throw new Error(
      `contract call simulation code hash ${simulationCodeHashHex} does not match caller-trusted expected code hash ${expectedCodeHashHex}`,
    );
  }

  const code = await client.getContractCodeBytes(
    expectedCodeHashHex,
    requestOptions,
  );
  if (code === null || code === undefined) {
    throw new Error(
      `deployed contract bytecode ${expectedCodeHashHex} is unavailable`,
    );
  }
  const codeBase64 = requireExactContractCodeBytesResponse(code);
  const deployedBytecode = assertZkModeIvmBytecode(
    codeBase64,
    expectedCodeHashHex,
    expectedArtifactSha256Hex,
  );

  const metadata = {
    ...metadataInput,
    contract_address: simulation.contract_address,
    contract_entrypoint: simulation.entrypoint,
  };
  if (contractAlias !== null) {
    metadata.contract_alias = contractAlias;
  }
  if (simulation.normalized_payload !== null) {
    metadata.contract_payload = simulation.normalized_payload;
  }
  const proofRequest = {
    vkRef,
    authority,
    metadata,
    bytecode: deployedBytecode,
    gasLimit,
  };
  const derived = await client.deriveIvmProved(proofRequest, requestOptions);
  assertIvmProvedBytecodeBinding(
    derived?.proved,
    deployedBytecode,
    "node-derived proved payload",
  );
  const requiredTransfer = assertRequiredOverlayTransfer(
    derived.proved,
    callerRequiredTransfer,
    "node-derived proved payload",
  );

  const proofJob = await client.proveIvmAndWait(
    proofRequest,
    {
      ...requestOptions,
      ...(proofIntervalMs === undefined
        ? {}
        : { intervalMs: proofIntervalMs }),
      ...(proofTimeoutMs === undefined
        ? {}
        : { timeoutMs: proofTimeoutMs }),
    },
  );
  if (canonicalJsonValue(proofJob.proved) !== canonicalJsonValue(derived.proved)) {
    throw new Error(
      "prover returned an IvmProved payload different from the authoritative derived payload",
    );
  }
  assertIvmProvedBytecodeBinding(
    proofJob.proved,
    deployedBytecode,
    "proved payload",
  );
  assertIvmProofAttachmentBinding(proofJob.attachment, vkRef);
  assertRequiredOverlayTransfer(
    proofJob.proved,
    callerRequiredTransfer,
    "proved payload",
  );

  throwIfSubmissionAborted(signal);
  const feeQuoteDraft = buildIvmProvedTransactionPayload({
    chainId,
    authority,
    proved: proofJob.proved,
    attachment: proofJob.attachment,
    feePayment,
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
  });
  const feeQuotePayloadJson = feeQuoteDraft.payloadJson;
  const feeQuoteAttachmentJson = feeQuoteDraft.attachmentJson;
  const feeQuote = await client.quoteFees(feeQuoteDraft, {
    canonicalAuth: {
      accountId: authority,
      privateKey,
    },
    ...(signal === undefined ? {} : { signal }),
  });
  throwIfSubmissionAborted(signal);
  const built = signQuotedIvmProvedTransactionPayload({
    payload: { payloadJson: feeQuotePayloadJson },
    attachment: feeQuoteAttachmentJson,
    quotedFeePayment: feeQuote.intent,
    privateKey,
    privateKeyAlgorithm,
  });
  throwIfSubmissionAborted(signal);
  const hashHex = built.hash.toString("hex");
  const submission = await client.submitTransaction(
    built.signedTransaction,
    requestOptions,
  );
  const status = opts.waitForCommit
    ? await client.waitForTransactionStatusTyped(hashHex, {
        intervalMs: transactionPollOptions.intervalMs,
        timeoutMs: transactionPollOptions.timeoutMs,
        ...(transactionPollOptions.signal === undefined
          ? {}
          : { signal: transactionPollOptions.signal }),
      })
    : null;

  return {
    hash: hashHex,
    signedTransaction: built.signedTransaction,
    submission,
    status,
    simulation,
    metadata,
    proved: proofJob.proved,
    attachment: proofJob.attachment,
    proofJobId: proofJob.job_id,
    feeQuoteDraft,
    feeQuote,
    requiredOverlayTransfer: requiredTransfer,
  };
}

export function buildTimeTriggerAction(options) {
  if (!options || typeof options !== "object") {
    throw new TypeError("buildTimeTriggerAction options must be an object");
  }
  const {
    authority,
    instructions,
    startTimestampMs,
    periodMs = null,
    repeats = null,
    metadata = null,
  } = options;
  const native = resolveNativeBinding();
  if (!native || typeof native.buildTimeTriggerAction !== "function") {
    throw new Error("native binding 'buildTimeTriggerAction' is unavailable");
  }
  const canonicalAuthority = normalizeAuthority(authority);
  const instructionPayloads = serializeInstructionPayloads(
    instructions,
    "buildTimeTriggerAction.instructions",
  );
  const startMs = ToriiClient._normalizeUnsignedInteger(
    startTimestampMs,
    "buildTimeTriggerAction.startTimestampMs",
    { allowZero: false },
  );
  const periodValue =
    periodMs === null || periodMs === undefined
      ? null
      : ToriiClient._normalizeUnsignedInteger(
          periodMs,
          "buildTimeTriggerAction.periodMs",
          { allowZero: false },
        );
  const repeatsValue = normalizeOptionalPositiveInteger(
    repeats,
    "buildTimeTriggerAction.repeats",
  );
  const metadataPayload = normalizeMetadataPayload(
    metadata,
    "buildTimeTriggerAction.metadata",
  );
  return native.buildTimeTriggerAction(
    canonicalAuthority,
    instructionPayloads,
    startMs,
    periodValue,
    repeatsValue,
    metadataPayload,
  );
}

export function buildPrecommitTriggerAction(options) {
  if (!options || typeof options !== "object") {
    throw new TypeError(
      "buildPrecommitTriggerAction options must be an object",
    );
  }
  const { authority, instructions, repeats = null, metadata = null } = options;
  const native = resolveNativeBinding();
  if (!native || typeof native.buildPrecommitTriggerAction !== "function") {
    throw new Error(
      "native binding 'buildPrecommitTriggerAction' is unavailable",
    );
  }
  const canonicalAuthority = normalizeAuthority(authority);
  const instructionPayloads = serializeInstructionPayloads(
    instructions,
    "buildPrecommitTriggerAction.instructions",
  );
  const repeatsValue = normalizeOptionalPositiveInteger(
    repeats,
    "buildPrecommitTriggerAction.repeats",
  );
  const metadataPayload = normalizeMetadataPayload(
    metadata,
    "buildPrecommitTriggerAction.metadata",
  );
  return native.buildPrecommitTriggerAction(
    canonicalAuthority,
    instructionPayloads,
    repeatsValue,
    metadataPayload,
  );
}

/**
 * Convenience helper to build a transaction with a single `Mint::Asset` instruction.
 * Additional transaction parameters mirror {@link buildTransaction}.
 */
export function buildMintAssetTransaction({
  chainId,
  authority,
  feePayment,
  assetHoldingId,
  assetId,
  quantity,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildMintAssetInstruction({
    assetHoldingId: assetHoldingId ?? assetId,
    quantity,
  });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Convenience helper to build a transaction with a single `Burn::Asset` instruction.
 * Additional transaction parameters mirror {@link buildTransaction}.
 */
export function buildBurnAssetTransaction({
  chainId,
  authority,
  feePayment,
  assetHoldingId,
  assetId,
  quantity,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildBurnAssetInstruction({
    assetHoldingId: assetHoldingId ?? assetId,
    quantity,
  });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `Burn::TriggerRepetitions` instruction.
 */
export function buildBurnTriggerTransaction({
  chainId,
  authority,
  feePayment,
  triggerId,
  repetitions,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildBurnTriggerRepetitionsInstruction({
    triggerId,
    repetitions,
  });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `Mint::TriggerRepetitions` instruction.
 */
export function buildMintTriggerTransaction({
  chainId,
  authority,
  feePayment,
  triggerId,
  repetitions,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildMintTriggerRepetitionsInstruction({
    triggerId,
    repetitions,
  });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `Transfer::Asset` instruction.
 */
export function buildTransferAssetTransaction({
  chainId,
  authority,
  feePayment,
  sourceAssetHoldingId,
  sourceAssetId,
  quantity,
  destinationAccountId,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildTransferAssetInstruction({
    sourceAssetHoldingId: sourceAssetHoldingId ?? sourceAssetId,
    quantity,
    destinationAccountId,
  });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build instructions combining a domain registration with an optional mint.
 */
function buildRegisterDomainInstructions({ domain, mints = [] }) {
  const instructions = [];
  instructions.push(
    buildRegisterDomainInstruction({
      domainId: domain.domainId,
      logo: domain.logo,
      metadata: domain.metadata,
    }),
  );
  mints.forEach((mint) => {
    instructions.push(
      buildMintAssetInstruction({
        assetHoldingId: mint.assetHoldingId ?? mint.assetId,
        quantity: mint.quantity,
      }),
    );
  });
  return instructions;
}

/**
 * Build instructions combining an account registration with a follow-up transfer.
 */
function buildRegisterAccountInstructions({ account, transfers = [] }) {
  if (account.domainId !== undefined || account.domain !== undefined) {
    throw new TypeError(
      "account registration is domainless; bind account aliases separately",
    );
  }
  const instructions = [];
  instructions.push(
    buildRegisterAccountInstruction({
      accountId: account.accountId,
      metadata: account.metadata,
    }),
  );
  transfers.forEach((transfer) => {
    const sourceAssetHoldingId =
      transfer.sourceAssetHoldingId ?? transfer.sourceAssetId;
    if (!sourceAssetHoldingId) {
      throw new TypeError("transfer.sourceAssetHoldingId is required");
    }
    instructions.push(
      buildTransferAssetInstruction({
        sourceAssetHoldingId,
        quantity: transfer.quantity,
        destinationAccountId: transfer.destinationAccountId,
      }),
    );
  });
  return instructions;
}

/**
 * Build a transaction containing a multisig registration (custom instruction).
 */
export function buildRegisterMultisigTransaction({
  chainId,
  authority,
  feePayment,
  accountId,
  spec,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildRegisterMultisigInstruction({ accountId, spec });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build instructions combining an asset definition registration with an optional mint.
 */
function buildRegisterAssetDefinitionInstructions({
  assetDefinition,
  mints = [],
}) {
  const instructions = [];
  const assetDefinitionId = normalizeTransactionAssetDefinitionId(
    assetDefinition.assetDefinitionId,
    "assetDefinition.assetDefinitionId",
  );
  const defaultConfidentialPolicy = {
    mode: "TransparentOnly",
    vk_set_hash: null,
    poseidon_params_id: null,
    pedersen_params_id: null,
    pending_transition: null,
  };
  const confidentialPolicy =
    assetDefinition.confidentialPolicy === undefined
      ? defaultConfidentialPolicy
      : { ...defaultConfidentialPolicy, ...assetDefinition.confidentialPolicy };
  instructions.push({
    Register: {
      AssetDefinition: {
        id: assetDefinitionId,
        logo: assetDefinition.logo ?? null,
        metadata: assetDefinition.metadata ?? {},
        mintable: assetDefinition.mintable ?? "Infinitely",
        spec: assetDefinition.spec ?? { scale: null },
        confidential_policy: confidentialPolicy,
      },
    },
  });
  mints.forEach((mint) => {
    instructions.push(
      buildMintAssetInstruction({
        assetHoldingId: mint.assetHoldingId ?? mint.assetId,
        quantity: mint.quantity,
      }),
    );
  });
  return instructions;
}

function resolveAssetHoldingIdForMint(
  assetDefinitionId,
  mint,
  context = "mint",
) {
  const providedAssetHoldingId = mint.assetHoldingId ?? mint.assetId;
  if (providedAssetHoldingId) {
    const normalizedAssetHoldingId = ToriiClient._normalizeAssetHoldingId(
      providedAssetHoldingId,
      mint.assetHoldingId !== undefined
        ? `${context}.assetHoldingId`
        : `${context}.assetId`,
    );
    if (!mint.accountId) {
      return normalizedAssetHoldingId;
    }
    const derivedAssetHoldingId = composeAssetHoldingIdFromDefinitionAndAccount(
      assetDefinitionId,
      mint.accountId,
      context,
    );
    if (normalizedAssetHoldingId !== derivedAssetHoldingId) {
      throw new TypeError(
        `${context}.assetHoldingId must match ${context}.assetDefinitionId + ${context}.accountId`,
      );
    }
    return normalizedAssetHoldingId;
  }
  if (!mint.accountId) {
    throw new TypeError(
      `${context}.assetId, ${context}.assetHoldingId, or ${context}.accountId must be provided`,
    );
  }
  return composeAssetHoldingIdFromDefinitionAndAccount(
    assetDefinitionId,
    mint.accountId,
    context,
  );
}

function normalizeDomainMintSpec(value, context) {
  if (!value || typeof value !== "object") {
    throw new TypeError(`${context} must be an object`);
  }
  const assetHoldingId = value.assetHoldingId ?? value.assetId;
  if (typeof assetHoldingId !== "string" || assetHoldingId.length === 0) {
    throw new TypeError(`${context}.assetId must be a non-empty string`);
  }
  return {
    assetHoldingId: ToriiClient._normalizeAssetHoldingId(
      assetHoldingId,
      value.assetHoldingId !== undefined
        ? `${context}.assetHoldingId`
        : `${context}.assetId`,
    ),
    quantity: value.quantity,
  };
}

function normalizeDomainMintSpecs(value, context) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array of mint descriptors`);
  }
  return value.map((item, index) =>
    normalizeDomainMintSpec(item, `${context}[${index}]`),
  );
}

function normalizeAssetDefinitionMintSpec(assetDefinitionId, value, context) {
  if (!value || typeof value !== "object") {
    throw new TypeError(`${context} must be an object`);
  }
  const assetHoldingId = resolveAssetHoldingIdForMint(
    assetDefinitionId,
    value,
    context,
  );
  return {
    assetHoldingId,
    accountId:
      value.accountId === undefined || value.accountId === null
        ? null
        : normalizeAccountId(value.accountId, `${context}.accountId`),
    quantity: value.quantity,
  };
}

function normalizeAssetDefinitionMintSpecs(assetDefinitionId, value, context) {
  if (!Array.isArray(value)) {
    throw new TypeError(
      `${context} must be an array of asset mint descriptors`,
    );
  }
  if (value.length === 0) {
    throw new TypeError(`${context} must contain at least one entry`);
  }
  return value.map((item, index) =>
    normalizeAssetDefinitionMintSpec(
      assetDefinitionId,
      item,
      `${context}[${index}]`,
    ),
  );
}

function normalizeTransferSpec(value, context, options = {}) {
  const { requireSource = false } = options;
  if (!value || typeof value !== "object") {
    throw new TypeError(`${context} must be an object`);
  }
  const spec = {
    sourceAssetHoldingId: value.sourceAssetHoldingId ?? value.sourceAssetId,
    quantity: value.quantity,
    destinationAccountId: value.destinationAccountId,
  };
  if (requireSource && !spec.sourceAssetHoldingId) {
    throw new TypeError(
      `${context}.sourceAssetId is required (or ${context}.sourceAssetHoldingId)`,
    );
  }
  return spec;
}

function normalizeTransferSpecs(value, context, options) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array of transfer descriptors`);
  }
  return value.map((item, index) =>
    normalizeTransferSpec(item, `${context}[${index}]`, options),
  );
}

/**
 * Build a transaction that first mints an asset and then transfers part of it.
 * Accepts either a single transfer descriptor or an array of transfers.
 */
export function buildMintAndTransferTransaction({
  chainId,
  authority,
  feePayment,
  mint,
  transfer,
  transfers,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  if (!mint || typeof mint !== "object") {
    throw new TypeError("mint options are required");
  }
  if (transfer && transfers) {
    throw new TypeError("provide either transfer or transfers, but not both");
  }
  const transferSpecs =
    transfers !== undefined
      ? normalizeTransferSpecs(transfers, "transfers")
      : transfer
        ? [normalizeTransferSpec(transfer, "transfer")]
        : [];
  if (transferSpecs.length === 0) {
    throw new TypeError("transfer or transfers options are required");
  }
  const mintInstruction = buildMintAssetInstruction(mint);
  const defaultSource = mint.assetHoldingId ?? mint.assetId;
  if (
    !defaultSource &&
    transferSpecs.some((spec) => spec.sourceAssetHoldingId === undefined)
  ) {
    throw new TypeError(
      "mint.assetHoldingId is required when transfer sourceAssetHoldingId is omitted",
    );
  }
  const instructions = [mintInstruction];
  for (const spec of transferSpecs) {
    const sourceAssetHoldingId = spec.sourceAssetHoldingId ?? defaultSource;
    instructions.push(
      buildTransferAssetInstruction({
        sourceAssetHoldingId,
        quantity: spec.quantity,
        destinationAccountId: spec.destinationAccountId,
      }),
    );
  }
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions,
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction that registers a domain and optionally mints an asset.
 */
export function buildRegisterDomainAndMintTransaction({
  chainId,
  authority,
  feePayment,
  domain,
  mint,
  mints,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  if (!domain || typeof domain !== "object") {
    throw new TypeError("domain registration parameters are required");
  }
  if (mint && mints) {
    throw new TypeError("provide either mint or mints, but not both");
  }
  const mintSpecs =
    mints !== undefined
      ? normalizeDomainMintSpecs(mints, "mints")
      : mint
        ? [normalizeDomainMintSpec(mint, "mint")]
        : [];
  const instructions = buildRegisterDomainInstructions({
    domain,
    mints: mintSpecs,
  });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions,
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction that registers a new account and optionally transfers an asset.
 */
export function buildRegisterAccountAndTransferTransaction({
  chainId,
  authority,
  feePayment,
  account,
  transfer,
  transfers,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  if (!account || typeof account !== "object") {
    throw new TypeError("account registration parameters are required");
  }
  if (transfer && transfers) {
    throw new TypeError("provide either transfer or transfers, but not both");
  }
  const transferSpecs =
    transfers !== undefined
      ? normalizeTransferSpecs(transfers, "transfers", { requireSource: true })
      : transfer
        ? [normalizeTransferSpec(transfer, "transfer", { requireSource: true })]
        : [];
  const instructions = buildRegisterAccountInstructions({
    account,
    transfers: transferSpecs,
  });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions,
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `Transfer::AssetDefinition` instruction.
 */
export function buildTransferAssetDefinitionTransaction({
  chainId,
  authority,
  feePayment,
  sourceAccountId,
  assetDefinitionId,
  destinationAccountId,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildTransferAssetDefinitionInstruction({
    sourceAccountId,
    assetDefinitionId,
    destinationAccountId,
  });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction that registers an asset definition and optionally mints to an account.
 */
export function buildRegisterAssetDefinitionAndMintTransaction({
  chainId,
  authority,
  feePayment,
  assetDefinition,
  mint,
  mints,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  if (!assetDefinition || typeof assetDefinition !== "object") {
    throw new TypeError("assetDefinition registration parameters are required");
  }
  if (mint && mints) {
    throw new TypeError("provide either mint or mints, but not both");
  }
  const mintSpecs =
    mints !== undefined
      ? normalizeAssetDefinitionMintSpecs(
          assetDefinition.assetDefinitionId,
          mints,
          "mints",
        )
      : mint
        ? [
            normalizeAssetDefinitionMintSpec(
              assetDefinition.assetDefinitionId,
              mint,
              "mint",
            ),
          ]
        : [];
  const instructions = buildRegisterAssetDefinitionInstructions({
    assetDefinition,
    mints: mintSpecs,
  });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions,
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction that registers an asset definition, mints, and optionally transfers it.
 * Supports either a single `transfer` descriptor or an array of `transfers` for batching.
 */
export function buildRegisterAssetDefinitionMintAndTransferTransaction({
  chainId,
  authority,
  feePayment,
  assetDefinition,
  mint,
  mints,
  transfer,
  transfers,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  if (!assetDefinition || typeof assetDefinition !== "object") {
    throw new TypeError("assetDefinition registration parameters are required");
  }
  if (mint && mints) {
    throw new TypeError("provide either mint or mints, but not both");
  }
  if (!mint && (mints === undefined || mints.length === 0)) {
    throw new TypeError("mint or mints parameters are required");
  }
  const mintSpecs =
    mints !== undefined
      ? normalizeAssetDefinitionMintSpecs(
          assetDefinition.assetDefinitionId,
          mints,
          "mints",
        )
      : [
          normalizeAssetDefinitionMintSpec(
            assetDefinition.assetDefinitionId,
            mint,
            "mint",
          ),
        ];

  const instructions = buildRegisterAssetDefinitionInstructions({
    assetDefinition,
    mints: mintSpecs,
  });

  if (transfer && transfers) {
    throw new TypeError("provide either transfer or transfers, but not both");
  }

  const transferSpecs =
    transfers !== undefined
      ? normalizeTransferSpecs(transfers, "transfers")
      : transfer
        ? [normalizeTransferSpec(transfer, "transfer")]
        : [];

  if (transferSpecs.length > 0) {
    const defaultSourceAssetHoldingId = mintSpecs[0].assetHoldingId;
    for (const spec of transferSpecs) {
      instructions.push(
        buildTransferAssetInstruction({
          sourceAssetHoldingId:
            spec.sourceAssetHoldingId ?? defaultSourceAssetHoldingId,
          quantity: spec.quantity,
          destinationAccountId: spec.destinationAccountId,
        }),
      );
    }
  }

  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions,
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `Transfer::Domain` instruction.
 */
export function buildTransferDomainTransaction({
  chainId,
  authority,
  feePayment,
  sourceAccountId,
  domainId,
  destinationAccountId,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildTransferDomainInstruction({
    sourceAccountId,
    domainId,
    destinationAccountId,
  });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `Transfer::Nft` instruction.
 */
export function buildTransferNftTransaction({
  chainId,
  authority,
  feePayment,
  sourceAccountId,
  nftId,
  destinationAccountId,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildTransferNftInstruction({
    sourceAccountId,
    nftId,
    destinationAccountId,
  });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `RegisterRwa` instruction.
 */
export function buildRegisterRwaTransaction({
  chainId,
  authority,
  feePayment,
  rwa,
  rwaJson,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildRegisterRwaInstruction({ rwa, rwaJson });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `TransferRwa` instruction.
 */
export function buildTransferRwaTransaction({
  chainId,
  authority,
  feePayment,
  sourceAccountId,
  rwaId,
  quantity,
  destinationAccountId,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildTransferRwaInstruction({
    sourceAccountId,
    rwaId,
    quantity,
    destinationAccountId,
  });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `MergeRwas` instruction.
 */
export function buildMergeRwasTransaction({
  chainId,
  authority,
  feePayment,
  merge,
  mergeJson,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildMergeRwasInstruction({ merge, mergeJson });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `RedeemRwa` instruction.
 */
export function buildRedeemRwaTransaction({
  chainId,
  authority,
  feePayment,
  rwaId,
  quantity,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildRedeemRwaInstruction({ rwaId, quantity });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `FreezeRwa` instruction.
 */
export function buildFreezeRwaTransaction({
  chainId,
  authority,
  feePayment,
  rwaId,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildFreezeRwaInstruction({ rwaId });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing an `UnfreezeRwa` instruction.
 */
export function buildUnfreezeRwaTransaction({
  chainId,
  authority,
  feePayment,
  rwaId,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildUnfreezeRwaInstruction({ rwaId });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `HoldRwa` instruction.
 */
export function buildHoldRwaTransaction({
  chainId,
  authority,
  feePayment,
  rwaId,
  quantity,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildHoldRwaInstruction({ rwaId, quantity });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `ReleaseRwa` instruction.
 */
export function buildReleaseRwaTransaction({
  chainId,
  authority,
  feePayment,
  rwaId,
  quantity,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildReleaseRwaInstruction({ rwaId, quantity });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `ForceTransferRwa` instruction.
 */
export function buildForceTransferRwaTransaction({
  chainId,
  authority,
  feePayment,
  rwaId,
  quantity,
  destinationAccountId,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildForceTransferRwaInstruction({
    rwaId,
    quantity,
    destinationAccountId,
  });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `SetRwaControls` instruction.
 */
export function buildSetRwaControlsTransaction({
  chainId,
  authority,
  feePayment,
  rwaId,
  controls,
  controlsJson,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildSetRwaControlsInstruction({
    rwaId,
    controls,
    controlsJson,
  });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `SetRwaKeyValue` instruction.
 */
export function buildSetRwaKeyValueTransaction({
  chainId,
  authority,
  feePayment,
  rwaId,
  key,
  value,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildSetRwaKeyValueInstruction({ rwaId, key, value });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `RemoveRwaKeyValue` instruction.
 */
export function buildRemoveRwaKeyValueTransaction({
  chainId,
  authority,
  feePayment,
  rwaId,
  key,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildRemoveRwaKeyValueInstruction({ rwaId, key });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `Kaigi::CreateKaigi` instruction.
 */
export function buildCreateKaigiTransaction({
  chainId,
  authority,
  feePayment,
  call,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildCreateKaigiInstruction(call);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `Kaigi::JoinKaigi` instruction.
 */
export function buildJoinKaigiTransaction({
  chainId,
  authority,
  feePayment,
  join,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildJoinKaigiInstruction(join);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `Kaigi::LeaveKaigi` instruction.
 */
export function buildLeaveKaigiTransaction({
  chainId,
  authority,
  feePayment,
  leave,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildLeaveKaigiInstruction(leave);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `Kaigi::EndKaigi` instruction.
 */
export function buildEndKaigiTransaction({
  chainId,
  authority,
  feePayment,
  end,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildEndKaigiInstruction(end);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

function normalizeInlineVerifyingKeyRecord(value, context) {
  const record =
    value && typeof value === "object" && !Array.isArray(value) ? value : null;
  if (!record) {
    throw new TypeError(`${context}.verifyingKey must be an object`);
  }
  const inlineKey = record.inline_key ?? record.inlineKey ?? null;
  if (!inlineKey || typeof inlineKey !== "object" || Array.isArray(inlineKey)) {
    throw new TypeError(`${context}.verifyingKey.inline_key must be present`);
  }
  const bytesBase64 = String(
    inlineKey.bytes_b64 ?? inlineKey.bytesBase64 ?? "",
  ).trim();
  if (!bytesBase64) {
    throw new TypeError(
      `${context}.verifyingKey.inline_key.bytes_b64 must be present`,
    );
  }
  const backend = normalizeExactMetadataString(
    record.id?.backend ?? record.backend,
    `${context}.verifyingKey.id.backend`,
  );
  const circuitId = normalizeExactMetadataString(
    record.record?.circuit_id ?? record.circuit_id ?? record.circuitId,
    `${context}.verifyingKey.record.circuit_id`,
  );
  return {
    record,
    backend,
    circuitId,
    bytes: Buffer.from(bytesBase64, "base64"),
  };
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
  const normalized = String(value ?? "");
  if (normalized.trim() !== normalized) {
    throw new TypeError(`${context} must not contain surrounding whitespace`);
  }
  if (!/^\d+$/.test(normalized)) {
    throw new TypeError(`${context} must be a whole-number string`);
  }
  return normalized;
}

function normalizeCanonicalQuantityInput(value, context) {
  try {
    if (value instanceof KotodamaQuantity) {
      return NumericV1.encodeQuantityJson(value);
    }
    if (typeof value === "string") {
      return NumericV1.decodeQuantityJson(value).toString();
    }
    if (typeof value === "bigint") {
      return new KotodamaQuantity(value, 0).toString();
    }
    throw new TypeError(
      `${context} must be a KotodamaQuantity, canonical quantity string, or bigint; JavaScript numbers are rejected`,
    );
  } catch (error) {
    if (!(error instanceof NumericV1Error)) throw error;
    throw new TypeError(
      `${context} must be a canonical non-negative Kotodama V1 Quantity (${error.code})`,
    );
  }
}

function normalizeFixed32HexInput(value, context) {
  if (typeof value === "string") {
    if (value.trim() !== value) {
      throw new TypeError(`${context} must not contain surrounding whitespace`);
    }
    const normalized = value.replace(/^0x/i, "").toLowerCase();
    if (!/^[0-9a-f]{64}$/.test(normalized)) {
      throw new TypeError(`${context} must be a 32-byte hex string`);
    }
    return normalized;
  }
  const buffer = toNamedBuffer(value, context);
  if (buffer.length !== 32) {
    throw new TypeError(`${context} must be 32 bytes`);
  }
  return Buffer.from(buffer).toString("hex");
}

function normalizeConfidentialInputDiversifierHex(input, index) {
  const context = `inputs[${index}].diversifier`;
  if (input?.diversifier_hex !== undefined || input?.diversifier !== undefined) {
    throw new TypeError(`${context} must use canonical diversifierHex`);
  }
  if (input?.diversifierHex === undefined) {
    throw new TypeError(`${context} is required`);
  }
  return normalizeFixed32HexInput(input.diversifierHex, context);
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
  throw new TypeError(`${context} must be a Buffer or ArrayBuffer view`);
}

/**
 * Build a deterministic confidential XOR fee-spend envelope for private Kaigi.
 */
export function buildPrivateKaigiFeeSpend({
  chainId,
  assetDefinitionId,
  actionHash,
  anchorRootHex,
  feeAmount,
  verifyingKey,
}) {
  const native = resolveNativeBinding();
  if (!native || typeof native.buildPrivateKaigiFeeSpend !== "function") {
    throw new Error(
      "native binding 'buildPrivateKaigiFeeSpend' is unavailable",
    );
  }
  const vk = normalizeInlineVerifyingKeyRecord(
    verifyingKey,
    "privateKaigiFeeSpend",
  );
  const result = native.buildPrivateKaigiFeeSpend(
    normalizeExactMetadataString(chainId, "privateKaigiFeeSpend.chainId"),
    normalizeExactMetadataString(
      assetDefinitionId,
      "privateKaigiFeeSpend.assetDefinitionId",
    ),
    toBuffer(actionHash),
    normalizeFixed32HexInput(anchorRootHex, "privateKaigiFeeSpend.anchorRootHex"),
    normalizeCanonicalQuantityInput(
      feeAmount,
      "privateKaigiFeeSpend.feeAmount",
    ),
    vk.backend,
    vk.circuitId,
    vk.bytes,
  );
  return {
    asset_definition_id: String(
      result.assetDefinitionId ?? result.asset_definition_id,
    ),
    anchor_root: Buffer.from(result.anchorRoot ?? result.anchor_root),
    nullifiers: Array.isArray(result.nullifiers)
      ? result.nullifiers.map((entry) => Buffer.from(entry))
      : [],
    output_commitments: Array.isArray(
      result.outputCommitments ?? result.output_commitments,
    )
      ? (result.outputCommitments ?? result.output_commitments).map((entry) =>
          Buffer.from(entry),
        )
      : [],
    encrypted_change_payloads: Array.isArray(
      result.encryptedChangePayloads ?? result.encrypted_change_payloads,
    )
      ? (
          result.encryptedChangePayloads ?? result.encrypted_change_payloads
        ).map((entry) => Buffer.from(entry))
      : [],
    proof: Buffer.from(result.proof),
  };
}

/**
 * Build a confidential transfer v2 proof envelope.
 */
export function buildConfidentialTransferProofV2({
  chainId,
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
  const normalizedInputs = Array.isArray(inputs)
    ? inputs.map((input, index) => ({
        amount: normalizeWholeNumberLiteral(
          input?.amount,
          `inputs[${index}].amount`,
        ),
        rhoHex: normalizeFixed32HexInput(
          input?.rhoHex ?? input?.rho,
          `inputs[${index}].rho`,
        ),
        diversifierHex: normalizeConfidentialInputDiversifierHex(input, index),
        leafIndex: Number(input?.leafIndex ?? input?.leaf_index ?? 0),
      }))
    : [];
  const normalizedOutputs = Array.isArray(outputs)
    ? outputs.map((output, index) => ({
        amount: normalizeWholeNumberLiteral(
          output?.amount,
          `outputs[${index}].amount`,
        ),
        rhoHex: normalizeFixed32HexInput(
          output?.rhoHex ?? output?.rho,
          `outputs[${index}].rho`,
        ),
        ownerTagHex: normalizeFixed32HexInput(
          output?.ownerTagHex ?? output?.owner_tag_hex ?? output?.ownerTag,
          `outputs[${index}].ownerTag`,
        ),
      }))
    : [];
  const normalizedTreeCommitments = Array.isArray(treeCommitments)
    ? treeCommitments.map((entry, index) =>
        normalizeFixed32HexInput(entry, `treeCommitments[${index}]`),
      )
    : [];
  const result = native.buildConfidentialTransferProofV2(
    normalizeExactMetadataString(chainId, "confidentialTransferProofV2.chainId"),
    normalizeExactMetadataString(
      assetDefinitionId,
      "confidentialTransferProofV2.assetDefinitionId",
    ),
    spendKeyBuffer,
    normalizedTreeCommitments,
    normalizedInputs,
    normalizedOutputs,
    normalizeFixed32HexInput(rootHintHex, "rootHintHex"),
    vk.backend,
    vk.circuitId,
    vk.bytes,
  );
  return {
    nullifiers: Array.isArray(result.nullifiers)
      ? result.nullifiers.map((entry) => Buffer.from(entry))
      : [],
    outputCommitments: Array.isArray(
      result.outputCommitments ?? result.output_commitments,
    )
      ? (result.outputCommitments ?? result.output_commitments).map((entry) =>
          Buffer.from(entry),
        )
      : [],
    root: Buffer.from(result.root),
    proof: Buffer.from(result.proof),
  };
}

/**
 * Build a confidential unshield v2 proof envelope.
 */
export function buildConfidentialUnshieldProofV2({
  chainId,
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
  const normalizedInputs = Array.isArray(inputs)
    ? inputs.map((input, index) => ({
        amount: normalizeWholeNumberLiteral(
          input?.amount,
          `inputs[${index}].amount`,
        ),
        rhoHex: normalizeFixed32HexInput(
          input?.rhoHex ?? input?.rho,
          `inputs[${index}].rho`,
        ),
        diversifierHex: normalizeConfidentialInputDiversifierHex(input, index),
        leafIndex: Number(input?.leafIndex ?? input?.leaf_index ?? 0),
      }))
    : [];
  const normalizedTreeCommitments = Array.isArray(treeCommitments)
    ? treeCommitments.map((entry, index) =>
        normalizeFixed32HexInput(entry, `treeCommitments[${index}]`),
      )
    : [];
  const result = native.buildConfidentialUnshieldProofV2(
    normalizeExactMetadataString(chainId, "confidentialUnshieldProofV2.chainId"),
    normalizeExactMetadataString(
      assetDefinitionId,
      "confidentialUnshieldProofV2.assetDefinitionId",
    ),
    spendKeyBuffer,
    normalizedTreeCommitments,
    normalizedInputs,
    normalizeWholeNumberLiteral(publicAmount, "publicAmount"),
    normalizeFixed32HexInput(rootHintHex, "rootHintHex"),
    vk.backend,
    vk.circuitId,
    vk.bytes,
  );
  return {
    nullifiers: Array.isArray(result.nullifiers)
      ? result.nullifiers.map((entry) => Buffer.from(entry))
      : [],
    root: Buffer.from(result.root),
    proof: Buffer.from(result.proof),
  };
}

/**
 * Build a confidential unshield v3 proof envelope with optional private change.
 */
export function buildConfidentialUnshieldProofV3({
  chainId,
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
  const normalizedInputs = Array.isArray(inputs)
    ? inputs.map((input, index) => ({
        amount: normalizeWholeNumberLiteral(
          input?.amount,
          `inputs[${index}].amount`,
        ),
        rhoHex: normalizeFixed32HexInput(
          input?.rhoHex ?? input?.rho,
          `inputs[${index}].rho`,
        ),
        diversifierHex: normalizeConfidentialInputDiversifierHex(input, index),
        leafIndex: Number(input?.leafIndex ?? input?.leaf_index ?? 0),
      }))
    : [];
  const normalizedOutputs = Array.isArray(outputs)
    ? outputs.map((output, index) => ({
        amount: normalizeWholeNumberLiteral(
          output?.amount,
          `outputs[${index}].amount`,
        ),
        rhoHex: normalizeFixed32HexInput(
          output?.rhoHex ?? output?.rho,
          `outputs[${index}].rho`,
        ),
      }))
    : [];
  const normalizedTreeCommitments = Array.isArray(treeCommitments)
    ? treeCommitments.map((entry, index) =>
        normalizeFixed32HexInput(entry, `treeCommitments[${index}]`),
      )
    : [];
  const result = native.buildConfidentialUnshieldProofV3(
    normalizeExactMetadataString(chainId, "confidentialUnshieldProofV3.chainId"),
    normalizeExactMetadataString(
      assetDefinitionId,
      "confidentialUnshieldProofV3.assetDefinitionId",
    ),
    spendKeyBuffer,
    normalizedTreeCommitments,
    normalizedInputs,
    normalizedOutputs,
    normalizeWholeNumberLiteral(publicAmount, "publicAmount"),
    normalizeFixed32HexInput(rootHintHex, "rootHintHex"),
    vk.backend,
    vk.circuitId,
    vk.bytes,
  );
  return {
    nullifiers: Array.isArray(result.nullifiers)
      ? result.nullifiers.map((entry) => Buffer.from(entry))
      : [],
    outputCommitments: Array.isArray(
      result.outputCommitments ?? result.output_commitments,
    )
      ? (result.outputCommitments ?? result.output_commitments).map((entry) =>
          Buffer.from(entry),
        )
      : [],
    root: Buffer.from(result.root),
    proof: Buffer.from(result.proof),
  };
}

/**
 * Build an authority-free private `TransactionEntrypoint::PrivateKaigi(Create)`.
 */
export function buildPrivateCreateKaigiTransaction({
  chainId,
  call,
  artifacts,
  feeSpend,
  metadata = null,
  creationTimeMs = null,
  nonce = null,
}) {
  const native = resolveNativeBinding();
  if (
    !native ||
    typeof native.buildPrivateCreateKaigiTransaction !== "function"
  ) {
    throw new Error(
      "native binding 'buildPrivateCreateKaigiTransaction' is unavailable",
    );
  }
  const result = native.buildPrivateCreateKaigiTransaction(
    normalizeExactMetadataString(chainId, "privateCreateKaigi.chainId"),
    JSON.stringify(call ?? {}),
    JSON.stringify(artifacts ?? {}),
    JSON.stringify(feeSpend ?? {}),
    normalizeMetadataPayload(metadata, "privateCreateKaigi.metadata"),
    creationTimeMs,
    nonce,
  );
  return {
    transactionEntrypoint: Buffer.from(result.transactionEntrypoint),
    hash: Buffer.from(result.hash),
    actionHash: Buffer.from(result.actionHash),
  };
}

/**
 * Build an authority-free private `TransactionEntrypoint::PrivateKaigi(Join)`.
 */
export function buildPrivateJoinKaigiTransaction({
  chainId,
  callId,
  artifacts,
  feeSpend,
  metadata = null,
  creationTimeMs = null,
  nonce = null,
}) {
  const native = resolveNativeBinding();
  if (
    !native ||
    typeof native.buildPrivateJoinKaigiTransaction !== "function"
  ) {
    throw new Error(
      "native binding 'buildPrivateJoinKaigiTransaction' is unavailable",
    );
  }
  const result = native.buildPrivateJoinKaigiTransaction(
    normalizeExactMetadataString(chainId, "privateJoinKaigi.chainId"),
    normalizeExactMetadataString(callId, "privateJoinKaigi.callId"),
    JSON.stringify(artifacts ?? {}),
    JSON.stringify(feeSpend ?? {}),
    normalizeMetadataPayload(metadata, "privateJoinKaigi.metadata"),
    creationTimeMs,
    nonce,
  );
  return {
    transactionEntrypoint: Buffer.from(result.transactionEntrypoint),
    hash: Buffer.from(result.hash),
    actionHash: Buffer.from(result.actionHash),
  };
}

/**
 * Build an authority-free private `TransactionEntrypoint::PrivateKaigi(End)`.
 */
export function buildPrivateEndKaigiTransaction({
  chainId,
  callId,
  endedAtMs = null,
  artifacts,
  feeSpend,
  metadata = null,
  creationTimeMs = null,
  nonce = null,
}) {
  const native = resolveNativeBinding();
  if (!native || typeof native.buildPrivateEndKaigiTransaction !== "function") {
    throw new Error(
      "native binding 'buildPrivateEndKaigiTransaction' is unavailable",
    );
  }
  const result = native.buildPrivateEndKaigiTransaction(
    normalizeExactMetadataString(chainId, "privateEndKaigi.chainId"),
    normalizeExactMetadataString(callId, "privateEndKaigi.callId"),
    endedAtMs,
    JSON.stringify(artifacts ?? {}),
    JSON.stringify(feeSpend ?? {}),
    normalizeMetadataPayload(metadata, "privateEndKaigi.metadata"),
    creationTimeMs,
    nonce,
  );
  return {
    transactionEntrypoint: Buffer.from(result.transactionEntrypoint),
    hash: Buffer.from(result.hash),
    actionHash: Buffer.from(result.actionHash),
  };
}

/**
 * Build a transaction containing a `Kaigi::RecordKaigiUsage` instruction.
 */
export function buildRecordKaigiUsageTransaction({
  chainId,
  authority,
  feePayment,
  usage,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildRecordKaigiUsageInstruction(usage);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `Kaigi::SetKaigiRelayManifest` instruction.
 */
export function buildSetKaigiRelayManifestTransaction({
  chainId,
  authority,
  feePayment,
  manifest,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildSetKaigiRelayManifestInstruction(manifest);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `Kaigi::RegisterKaigiRelay` instruction.
 */
export function buildRegisterKaigiRelayTransaction({
  chainId,
  authority,
  feePayment,
  relay,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildRegisterKaigiRelayInstruction(relay);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `ProposeDeployContract` instruction.
 */
export function buildProposeDeployContractTransaction({
  chainId,
  authority,
  feePayment,
  proposal,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildProposeDeployContractInstruction(proposal);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `ProposeSccpRouteGovernance` instruction.
 */
export function buildProposeSccpRouteGovernanceTransaction({
  chainId,
  authority,
  feePayment,
  proposal,
  action,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildProposeSccpRouteGovernanceInstruction(
    proposal ?? { action },
  );
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `CastZkBallot` instruction.
 */
export function buildCastZkBallotTransaction({
  chainId,
  authority,
  feePayment,
  ballot,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildCastZkBallotInstruction(ballot);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `CastPlainBallot` instruction.
 */
export function buildCastPlainBallotTransaction({
  chainId,
  authority,
  feePayment,
  ballot,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildCastPlainBallotInstruction(ballot);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing an `EnactReferendum` instruction.
 */
export function buildEnactReferendumTransaction({
  chainId,
  authority,
  feePayment,
  enactment,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildEnactReferendumInstruction(enactment);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `FinalizeReferendum` instruction.
 */
export function buildFinalizeReferendumTransaction({
  chainId,
  authority,
  feePayment,
  finalization,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildFinalizeReferendumInstruction(finalization);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `PersistCouncilForEpoch` instruction.
 */
export function buildPersistCouncilForEpochTransaction({
  chainId,
  authority,
  feePayment,
  record,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildPersistCouncilForEpochInstruction(record);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

export function buildRegisterZkAssetTransaction({
  chainId,
  authority,
  feePayment,
  registration,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildRegisterZkAssetInstruction(registration);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

export function buildScheduleConfidentialPolicyTransitionTransaction({
  chainId,
  authority,
  feePayment,
  transition,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction =
    buildScheduleConfidentialPolicyTransitionInstruction(transition);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

export function buildCancelConfidentialPolicyTransitionTransaction({
  chainId,
  authority,
  feePayment,
  cancellation,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction =
    buildCancelConfidentialPolicyTransitionInstruction(cancellation);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

export function buildShieldTransaction({
  chainId,
  authority,
  feePayment,
  shield,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildShieldInstruction(shield);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

export function buildZkTransferTransaction({
  chainId,
  authority,
  feePayment,
  transfer,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildZkTransferInstruction(transfer);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

export function buildUnshieldTransaction({
  chainId,
  authority,
  feePayment,
  unshield,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildUnshieldInstruction(unshield);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

export function buildCreateElectionTransaction({
  chainId,
  authority,
  feePayment,
  election,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildCreateElectionInstruction(election);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

export function buildSubmitBallotTransaction({
  chainId,
  authority,
  feePayment,
  ballot,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildSubmitBallotInstruction(ballot);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

export function buildFinalizeElectionTransaction({
  chainId,
  authority,
  feePayment,
  finalization,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildFinalizeElectionInstruction(finalization);
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `RegisterSmartContractCode` instruction.
 */
export function buildRegisterSmartContractCodeTransaction({
  chainId,
  authority,
  feePayment,
  manifest,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildRegisterSmartContractCodeInstruction({ manifest });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `RegisterSmartContractBytes` instruction.
 */
export function buildRegisterSmartContractBytesTransaction({
  chainId,
  authority,
  feePayment,
  codeHash,
  code,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildRegisterSmartContractBytesInstruction({
    codeHash,
    code,
  });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build a transaction containing a `RemoveSmartContractBytes` instruction.
 */
export function buildRemoveSmartContractBytesTransaction({
  chainId,
  authority,
  feePayment,
  codeHash,
  reason = null,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildRemoveSmartContractBytesInstruction({
    codeHash,
    reason,
  });
  return buildTransaction({
    chainId,
    authority,
    feePayment,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Submit a signed transaction and optionally wait for authoritative Applied finality.
 * @param {ToriiClient} client
 * @param {ArrayBufferView | ArrayBuffer | Buffer} signedTransaction
 * @param {{ waitForCommit?: boolean, pollIntervalMs?: number, timeoutMs?: number }} [options]
 * @returns {Promise<{hash: string, submission: any, status?: any}>}
 */
export async function submitSignedTransaction(
  client,
  signedTransaction,
  options = {},
) {
  if (!(client instanceof ToriiClient)) {
    throw new TypeError("client must be an instance of ToriiClient");
  }
  if (Object.prototype.hasOwnProperty.call(options, "scope")) {
    throw new TypeError(
      "options.scope is unsupported; finality waits always use global scope",
    );
  }
  let txBuffer = toBuffer(signedTransaction);
  if (options.privateKey) {
    txBuffer = resignSignedTransaction(txBuffer, options.privateKey);
  }
  const hashHex = hashSignedTransaction(txBuffer);
  const submission = await client.submitTransaction(txBuffer);

  if (!options.waitForCommit) {
    return { hash: hashHex, submission };
  }

  const status = await waitForAuthoritativeApplied(client, hashHex, options);
  return { hash: hashHex, submission, status };
}

/**
 * Submit a raw transaction entrypoint payload and optionally wait for authoritative Applied finality.
 * @param {ToriiClient} client
 * @param {ArrayBufferView | ArrayBuffer | Buffer} transactionEntrypoint
 * @param {{ hashHex: string, waitForCommit?: boolean, pollIntervalMs?: number, timeoutMs?: number }} options
 * @returns {Promise<{hash: string, submission: any, status?: any}>}
 */
export async function submitTransactionEntrypoint(
  client,
  transactionEntrypoint,
  options,
) {
  if (!(client instanceof ToriiClient)) {
    throw new TypeError("client must be an instance of ToriiClient");
  }
  if (!options || typeof options !== "object") {
    throw new TypeError(
      "options.hashHex is required for entrypoint submission",
    );
  }
  if (Object.prototype.hasOwnProperty.call(options, "scope")) {
    throw new TypeError(
      "options.scope is unsupported; finality waits always use global scope",
    );
  }
  const hashHex = String(options.hashHex ?? "").trim();
  if (!/^[0-9a-fA-F]{64}$/.test(hashHex)) {
    throw new TypeError("options.hashHex must be a 32-byte hex string");
  }
  const payload = toBuffer(transactionEntrypoint);
  const submission = await client.submitTransaction(payload);

  if (!options.waitForCommit) {
    return { hash: hashHex.toLowerCase(), submission };
  }

  const status = await waitForAuthoritativeApplied(client, hashHex, options);
  return { hash: hashHex.toLowerCase(), submission, status };
}

async function waitForAuthoritativeApplied(client, hashHex, options) {
  const pollOptions = {
    intervalMs: options.pollIntervalMs ?? 500,
    timeoutMs: options.timeoutMs ?? 30_000,
  };
  return client.waitForTransactionStatusTyped(hashHex, pollOptions);
}

function toBuffer(value, context = "signedTransaction") {
  if (Buffer.isBuffer(value)) {
    return Buffer.from(value);
  }
  if (ArrayBuffer.isView(value)) {
    return Buffer.from(new Uint8Array(value.buffer, value.byteOffset, value.byteLength));
  }
  if (value instanceof ArrayBuffer) {
    return Buffer.from(new Uint8Array(value));
  }
  throw new TypeError(`${context} must be a Buffer or ArrayBuffer view`);
}
