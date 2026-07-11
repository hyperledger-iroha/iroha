import { createHash } from "node:crypto";
import { getNativeBinding } from "./native.js";
import {
  ToriiClient,
  getTrustedValidationFeeVerificationContext,
} from "./toriiClient.js";
import { noritoDecodeInstruction } from "./norito.js";
import {
  VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS,
  validationFeeQuantity,
  verifySignedValidationFeePolicy,
} from "./validationFeePolicy.js";
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
  buildProposeSccpRouteManifestInstruction,
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

const KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPES = new Map([
  ["KagemushaTransfer", "iroha_data_model::isi::offline::KagemushaTransfer"],
  [
    "RedeemKagemushaRecursive",
    "iroha_data_model::isi::offline::RedeemKagemushaRecursive",
  ],
  [
    "TopUpKagemushaRecursive",
    "iroha_data_model::isi::offline::TopUpKagemushaRecursive",
  ],
]);
const KAGEMUSHA_INSTRUCTION_ARCHIVE_MAX_BYTES = 256 * 1024 * 1024;
const NORITO_HEADER_BYTES = 40;
const NORITO_MAX_HEADER_PADDING_BYTES = 64;
const NORITO_MAGIC = Buffer.from("NRT0", "ascii");
const NORITO_SUPPORTED_FLAGS_MASK = 0x27;
const NORITO_FIELD_BITSET_FLAG = 0x20;
const NORITO_FIELD_BITSET_REQUIRED_FLAGS = 0x06;
const NORITO_CRC64_MASK = 0xffff_ffff_ffff_ffffn;
const NORITO_CRC64_REFLECTED_POLY = 0xc96c_5795_d787_0f42n;
const NORITO_CRC64_TABLE = (() => {
  const table = new Array(256);
  for (let index = 0; index < table.length; index += 1) {
    let crc = BigInt(index);
    for (let bit = 0; bit < 8; bit += 1) {
      crc =
        (crc & 1n) !== 0n
          ? (crc >> 1n) ^ NORITO_CRC64_REFLECTED_POLY
          : crc >> 1n;
    }
    table[index] = crc;
  }
  return table;
})();

function normalizeKagemushaInstructionArchiveType(type, context) {
  if (
    typeof type !== "string" ||
    !KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPES.has(type)
  ) {
    throw new TypeError(
      `${context}.type must be KagemushaTransfer, RedeemKagemushaRecursive, or TopUpKagemushaRecursive`,
    );
  }
  return type;
}

function kagemushaArchiveBuffer(source, context) {
  const selected =
    source.instructionArchive ??
    source.instruction_archive ??
    source.archive ??
    source.bytes;
  if (selected !== undefined && selected !== null) {
    const buffer = toBuffer(selected, `${context}.instructionArchive`);
    if (buffer.length === 0) {
      throw new TypeError(`${context}.instructionArchive must not be empty`);
    }
    return Buffer.from(buffer);
  }
  const encoded = source.bytesBase64 ?? source.bytes_base64;
  if (typeof encoded !== "string" || encoded.trim().length === 0) {
    throw new TypeError(
      `${context}.instructionArchive or ${context}.bytesBase64 is required`,
    );
  }
  const normalized = encoded.trim();
  const canonicalBase64Pattern =
    /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u;
  if (encoded !== normalized || !canonicalBase64Pattern.test(normalized)) {
    throw new TypeError(
      `${context}.bytesBase64 must be canonical standard base64`,
    );
  }
  const buffer = Buffer.from(normalized, "base64");
  if (buffer.toString("base64") !== normalized) {
    throw new TypeError(
      `${context}.bytesBase64 must be canonical standard base64`,
    );
  }
  if (buffer.length === 0) {
    throw new TypeError(
      `${context}.bytesBase64 must decode to non-empty bytes`,
    );
  }
  return buffer;
}

function noritoSchemaHash(typeName) {
  return createHash("sha256")
    .update("norito:v1:type-name\0", "utf8")
    .update(typeName, "utf8")
    .digest()
    .subarray(0, 16);
}

function noritoCrc64(payload) {
  let crc = NORITO_CRC64_MASK;
  for (const byte of payload) {
    const index = Number((crc ^ BigInt(byte)) & 0xffn);
    crc = NORITO_CRC64_TABLE[index] ^ (crc >> 8n);
  }
  return BigInt.asUintN(64, crc ^ NORITO_CRC64_MASK);
}

function validateKagemushaInstructionArchive(type, archive, context) {
  const invalidMessage = `${context}.instructionArchive must be a valid ${type} Norito archive`;
  const fail = () => {
    throw new TypeError(invalidMessage);
  };
  if (archive.length > KAGEMUSHA_INSTRUCTION_ARCHIVE_MAX_BYTES) {
    throw new TypeError(
      `${context}.instructionArchive must not exceed ${KAGEMUSHA_INSTRUCTION_ARCHIVE_MAX_BYTES} bytes`,
    );
  }
  if (archive.length < NORITO_HEADER_BYTES) {
    fail();
  }
  if (!archive.subarray(0, 4).equals(NORITO_MAGIC)) {
    fail();
  }
  if (archive[4] !== 0 || archive[5] !== 0) {
    fail();
  }
  const wireName = KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPES.get(type);
  const expectedSchema = noritoSchemaHash(wireName);
  if (!archive.subarray(6, 22).equals(expectedSchema)) {
    throw new TypeError(
      `${context}.instructionArchive schema must match ${type}`,
    );
  }
  if (archive[22] !== 0) {
    throw new TypeError(`${context}.instructionArchive must not be compressed`);
  }
  const flags = archive[39];
  if (
    (flags & ~NORITO_SUPPORTED_FLAGS_MASK) !== 0 ||
    ((flags & NORITO_FIELD_BITSET_FLAG) !== 0 &&
      (flags & NORITO_FIELD_BITSET_REQUIRED_FLAGS) !==
        NORITO_FIELD_BITSET_REQUIRED_FLAGS)
  ) {
    fail();
  }
  const payloadLengthBig = archive.readBigUInt64LE(23);
  if (payloadLengthBig > BigInt(Number.MAX_SAFE_INTEGER)) {
    fail();
  }
  const payloadLength = Number(payloadLengthBig);
  if (payloadLength === 0) {
    throw new TypeError(
      `${context}.instructionArchive must contain a non-empty Norito payload`,
    );
  }
  const minimumLength = NORITO_HEADER_BYTES + payloadLength;
  if (archive.length < minimumLength) {
    fail();
  }
  const paddingLength = archive.length - minimumLength;
  if (paddingLength > NORITO_MAX_HEADER_PADDING_BYTES) {
    fail();
  }
  const padding = archive.subarray(
    NORITO_HEADER_BYTES,
    NORITO_HEADER_BYTES + paddingLength,
  );
  if (padding.some((byte) => byte !== 0)) {
    fail();
  }
  const payload = archive.subarray(NORITO_HEADER_BYTES + paddingLength);
  if (noritoCrc64(payload) !== archive.readBigUInt64LE(31)) {
    throw new TypeError(`${context}.instructionArchive checksum is invalid`);
  }
}

/**
 * Build a typed Kagemusha instruction archive payload accepted by
 * {@link buildTransaction}. The archive must be canonical Norito bytes for the
 * selected Kagemusha instruction type; native translation re-decodes the bytes
 * before signing.
 */
export function buildKagemushaInstructionArchiveInstruction(input) {
  if (!input || typeof input !== "object") {
    throw new TypeError(
      "Kagemusha instruction archive input must be an object",
    );
  }
  const type = normalizeKagemushaInstructionArchiveType(
    input.type ?? input.instructionType ?? input.instruction_type,
    "kagemushaInstructionArchive",
  );
  const archive = kagemushaArchiveBuffer(input, "kagemushaInstructionArchive");
  validateKagemushaInstructionArchive(
    type,
    archive,
    "kagemushaInstructionArchive",
  );
  return {
    KagemushaInstructionArchive: {
      type,
      bytes_base64: archive.toString("base64"),
    },
  };
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
 * Build an `UpsertSccpRouteManifest` instruction from a canonical data-model
 * route manifest. The manifest should use snake_case fields accepted by the
 * native host and chain-side ISI validation.
 * @param {{manifest?: object, routeManifest?: object, route_manifest?: object} | object} input
 * @returns {{UpsertSccpRouteManifest: {manifest: object}}}
 */
export function buildUpsertSccpRouteManifestInstruction(input) {
  const source = normalizePlainObject(
    input,
    "buildUpsertSccpRouteManifestInstruction input",
  );
  const hasExplicitManifest =
    Object.prototype.hasOwnProperty.call(source, "manifest") ||
    Object.prototype.hasOwnProperty.call(source, "routeManifest") ||
    Object.prototype.hasOwnProperty.call(source, "route_manifest");
  const manifest = hasExplicitManifest
    ? source.manifest ?? source.routeManifest ?? source.route_manifest
    : source;
  return {
    UpsertSccpRouteManifest: {
      manifest: normalizePlainObject(
        manifest,
        "buildUpsertSccpRouteManifestInstruction.manifest",
      ),
    },
  };
}

/**
 * Build a `RemoveSccpRouteManifest` instruction.
 * @param {{routeId?: string, route_id?: string, assetKey?: string, asset_key?: string, counterpartyDomain?: number|string|bigint, counterparty_domain?: number|string|bigint, chainIdHex?: string, chain_id_hex?: string}} input
 * @returns {{RemoveSccpRouteManifest: {route_id: string, asset_key: string, counterparty_domain: number, chain_id_hex: string}}}
 */
export function buildRemoveSccpRouteManifestInstruction(input) {
  const source = normalizePlainObject(
    input,
    "buildRemoveSccpRouteManifestInstruction input",
  );
  return {
    RemoveSccpRouteManifest: {
      route_id: normalizeNonEmptyString(
        source.route_id ?? source.routeId,
        "buildRemoveSccpRouteManifestInstruction.routeId",
      ),
      asset_key: normalizeNonEmptyString(
        source.asset_key ?? source.assetKey,
        "buildRemoveSccpRouteManifestInstruction.assetKey",
      ),
      counterparty_domain: normalizeUint32(
        source.counterparty_domain ?? source.counterpartyDomain,
        "buildRemoveSccpRouteManifestInstruction.counterpartyDomain",
      ),
      chain_id_hex: normalizeNonEmptyString(
        source.chain_id_hex ?? source.chainIdHex,
        "buildRemoveSccpRouteManifestInstruction.chainIdHex",
      ),
    },
  };
}

/**
 * Build and sign a transaction containing one `UpsertSccpRouteManifest`
 * instruction.
 */
export function buildUpsertSccpRouteManifestTransaction({
  chainId,
  authority,
  manifest,
  routeManifest,
  route_manifest,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildUpsertSccpRouteManifestInstruction({
    manifest: manifest ?? routeManifest ?? route_manifest,
  });
  return buildTransaction({
    chainId,
    authority,
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
 * Build and sign a transaction containing one `RemoveSccpRouteManifest`
 * instruction.
 */
export function buildRemoveSccpRouteManifestTransaction({
  chainId,
  authority,
  routeId,
  route_id,
  assetKey,
  asset_key,
  counterpartyDomain,
  counterparty_domain,
  chainIdHex,
  chain_id_hex,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildRemoveSccpRouteManifestInstruction({
    routeId: routeId ?? route_id,
    assetKey: assetKey ?? asset_key,
    counterpartyDomain: counterpartyDomain ?? counterparty_domain,
    chainIdHex: chainIdHex ?? chain_id_hex,
  });
  return buildTransaction({
    chainId,
    authority,
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
 * Build and sign an SNS name-registration consensus transaction.
 * @param {{
 *   chainId: string,
 *   authority: string,
 *   request: object,
 *   metadata?: object | string | null,
 *   creationTimeMs?: number,
 *   ttlMs?: number,
 *   nonce?: number,
 *   privateKey: ArrayBufferView | ArrayBuffer | Buffer,
 *   privateKeyAlgorithm?: string
 * }} input
 * @returns {{signedTransaction: Buffer, hash: Buffer}}
 */
export function buildRegisterSnsNameTransaction(input) {
  const request = input?.request;
  if (!request || typeof request !== "object" || Array.isArray(request)) {
    throw new TypeError("request must be a non-null object");
  }
  return buildTransaction({
    ...input,
    instructions: [{ RegisterSnsName: request }],
  });
}

/**
 * Build and sign a transaction whose executable is `Executable::IvmProved`.
 * @param {{
 *   chainId: string,
 *   authority: string,
 *   proved: object | string,
 *   attachment: object | string,
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

const INLINE_VALIDATION_FEE_CONTEXT_KEYS = new Set([
  "verificationContext",
  "verification_context",
  "networkId",
  "network_id",
  "genesisHash",
  "genesis_hash",
  "currentHeight",
  "current_height",
  "governanceKeyset",
  "governance_keyset",
  "governanceKeysets",
  "governance_keysets",
  "policyRegistry",
  "policy_registry",
  "requireActive",
  "require_active",
]);

function readExclusiveInputAlias(record, aliases, context) {
  const supplied = [];
  for (const alias of aliases) {
    if (!Object.prototype.hasOwnProperty.call(record, alias)) continue;
    const value = record[alias];
    if (value !== undefined) supplied.push({ alias, value });
  }
  if (supplied.length > 1) {
    throw new TypeError(
      `${context} must use exactly one of ${aliases.join(", ")}`,
    );
  }
  return supplied.length === 0 ? undefined : supplied[0].value;
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
  return { ...record };
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

function assertZkModeIvmBytecode(bytecodeBase64) {
  const bytecode = Buffer.from(bytecodeBase64, "base64");
  if (
    bytecode.length < 17 ||
    !bytecode.subarray(0, 4).equals(Buffer.from([0x49, 0x56, 0x4d, 0x00]))
  ) {
    throw new Error("deployed contract bytecode has an invalid IVM header");
  }
  if ((bytecode[6] & 0x01) === 0) {
    throw new Error(
      "deployed contract bytecode is not ZK mode; compile and deploy the artifact with --force-zk",
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

function normalizeSafeU64Number(value, context, { positive = false } = {}) {
  let parsed;
  if (typeof value === "bigint") {
    parsed = value;
  } else if (typeof value === "number" && Number.isSafeInteger(value)) {
    parsed = BigInt(value);
  } else if (typeof value === "string" && /^\d+$/u.test(value)) {
    parsed = BigInt(value);
  } else {
    throw new TypeError(`${context} must be an unsigned safe integer`);
  }
  if (
    parsed < (positive ? 1n : 0n) ||
    parsed > BigInt(Number.MAX_SAFE_INTEGER)
  ) {
    throw new RangeError(`${context} must be an unsigned safe integer`);
  }
  return Number(parsed);
}

function prepareValidationFeePolicyIntent(
  value,
  authority,
  chainId,
  trustedVerificationContext,
) {
  const intent = normalizePlainObject(value, "validationFeePolicy");
  for (const key of INLINE_VALIDATION_FEE_CONTEXT_KEYS) {
    if (
      Object.prototype.hasOwnProperty.call(intent, key) &&
      intent[key] !== undefined
    ) {
      throw new Error(
        `validationFeePolicy.${key} cannot override the ToriiClient trusted validation-fee verification context`,
      );
    }
  }
  if (trustedVerificationContext === null) {
    throw new Error(
      "validation-fee submission requires ToriiClient.options.validationFeeVerificationContext",
    );
  }
  const signedPolicy = readExclusiveInputAlias(
    intent,
    ["signedPolicy", "signed_policy"],
    "validationFeePolicy.signedPolicy",
  );
  const verified = verifySignedValidationFeePolicy(
    signedPolicy,
    trustedVerificationContext,
  );
  const networkId = normalizeNonEmptyString(verified.policy.network_id, "policy.network_id");
  const normalizedChainId = normalizeNonEmptyString(chainId, "chainId");
  if (typeof chainId !== "string" || chainId !== normalizedChainId) {
    throw new TypeError("chainId must be a non-empty trimmed string");
  }
  if (normalizedChainId !== networkId) {
    throw new Error(
      `chainId ${normalizedChainId} does not match verified validation-fee policy network ${networkId}`,
    );
  }

  const qualifyingTransferCountValue = readExclusiveInputAlias(
    intent,
    ["qualifyingTransferCount", "qualifying_transfer_count"],
    "validationFeePolicy.qualifyingTransferCount",
  );
  const assertedQualifyingTransferCount =
    qualifyingTransferCountValue === undefined ||
    qualifyingTransferCountValue === null
      ? null
      : normalizeSafeU64Number(
          qualifyingTransferCountValue,
          "validationFeePolicy.qualifyingTransferCount",
          { positive: true },
        );
  const instructionIndex = normalizeSafeU64Number(
    readExclusiveInputAlias(
      intent,
      ["feeInstructionIndex", "fee_instruction_index"],
      "validationFeePolicy.feeInstructionIndex",
    ),
    "validationFeePolicy.feeInstructionIndex",
  );
  const transferEntryValue = readExclusiveInputAlias(
    intent,
    ["feeTransferEntryIndex", "fee_transfer_entry_index"],
    "validationFeePolicy.feeTransferEntryIndex",
  );
  const transferEntryIndex =
    transferEntryValue === undefined || transferEntryValue === null
      ? null
      : normalizeSafeU64Number(
          transferEntryValue,
          "validationFeePolicy.feeTransferEntryIndex",
        );
  if (verified.policyVersion > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new RangeError(
      "verified validation-fee policy version cannot be represented safely in transaction metadata",
    );
  }

  return {
    verified,
    assertedQualifyingTransferCount,
    instructionIndex,
    transferEntryIndex,
    authority,
    dsAssetId: verified.policy.ds_asset_id,
    treasuryAccountId: verified.policy.treasury_account_id,
    treasuryPayoutExempt: verified.policy.exemption_classes.includes(
      VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS,
    ),
    metadata: {
      validation_fee_policy_version: Number(verified.policyVersion),
      validation_fee_policy_hash: verified.policyHashHex,
      validation_fee_instruction_index: instructionIndex,
      ...(transferEntryIndex === null
        ? {}
        : { validation_fee_transfer_entry_index: transferEntryIndex }),
    },
  };
}

function directAssetTransfer(instruction) {
  const transfer = instruction?.Transfer?.Asset;
  if (!transfer || typeof transfer !== "object" || Array.isArray(transfer)) {
    return null;
  }
  const source = transfer.source;
  if (typeof source !== "string") return null;
  const separator = source.indexOf("#");
  if (separator <= 0 || separator === source.length - 1) return null;
  return {
    assetDefinitionId: source.slice(0, separator),
    sourceAccountId: source.slice(separator + 1),
    destinationAccountId: transfer.destination,
    quantity: String(transfer.object),
  };
}

function batchAssetTransfers(instruction) {
  if (!instruction || typeof instruction !== "object" || Array.isArray(instruction)) {
    return [];
  }
  const batch = readExclusiveInputAlias(
    instruction,
    ["TransferAssetBatch", "transfer_asset_batch", "AssetTransferBatch"],
    "overlay instruction TransferAssetBatch",
  );
  if (!batch || typeof batch !== "object" || !Array.isArray(batch.entries)) {
    return [];
  }
  return batch.entries.map((entry) => {
    if (!entry || typeof entry !== "object" || Array.isArray(entry)) return null;
    return {
      assetDefinitionId: entry.asset_definition,
      sourceAccountId: entry.from,
      destinationAccountId: entry.to,
      quantity: String(entry.amount),
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
  const customProposal = instruction?.Custom?.payload?.Propose;
  const aliasProposal = instruction?.MultisigPropose;
  if (customProposal !== undefined && aliasProposal !== undefined) {
    throw new TypeError(
      "overlay multisig proposal must use exactly one of Custom.payload.Propose, MultisigPropose",
    );
  }
  const proposal = customProposal ?? aliasProposal;
  if (proposal === undefined || proposal === null) return null;
  if (typeof proposal !== "object" || Array.isArray(proposal)) {
    throw new TypeError("MultisigPropose payload must be an object");
  }
  if (!Array.isArray(proposal.instructions)) {
    throw new TypeError("MultisigPropose.instructions must be an array");
  }
  if (typeof proposal.account !== "string" || proposal.account.length === 0) {
    throw new TypeError("MultisigPropose.account must be an account id");
  }
  return proposal;
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
      if (direct) {
        transferContext.transfers.push({
          ...direct,
          contextIndex: transferContext.contextIndex,
          instructionIndex,
          transferEntryIndex: null,
        });
      }
      const batch = batchAssetTransfers(instruction);
      for (
        let transferEntryIndex = 0;
        transferEntryIndex < batch.length;
        transferEntryIndex += 1
      ) {
        if (batch[transferEntryIndex]) {
          transferContext.transfers.push({
            ...batch[transferEntryIndex],
            contextIndex: transferContext.contextIndex,
            instructionIndex,
            transferEntryIndex,
          });
        }
      }

      const proposal = multisigPropose(instruction);
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

const VALIDATION_FEE_U64_MAX = 0xffff_ffff_ffff_ffffn;

function validationFeeMinorUnits(value, scale, context) {
  let literal;
  if (typeof value === "bigint") {
    literal = value.toString();
  } else if (typeof value === "number" && Number.isFinite(value)) {
    literal = value.toString();
  } else if (typeof value === "string") {
    literal = value;
  } else {
    throw new TypeError(`${context} must be a non-negative Numeric literal`);
  }
  const match = /^(\d+)(?:\.(\d*))?$/u.exec(literal);
  if (!match) {
    throw new TypeError(`${context} must be a non-negative Numeric literal`);
  }
  const fractional = match[2] ?? "";
  if (fractional.length > scale) {
    throw new RangeError(
      `${context} uses scale ${fractional.length}, above policy scale ${scale}`,
    );
  }
  const mantissa = BigInt(`${match[1]}${fractional}`);
  const minorUnits = mantissa * 10n ** BigInt(scale - fractional.length);
  if (minorUnits > VALIDATION_FEE_U64_MAX) {
    throw new RangeError(`${context} exceeds the validation-fee u64 range`);
  }
  return minorUnits;
}

function assertValidationFeeOverlay(proved, binding, context) {
  const overlay = proved?.overlay;
  if (!Array.isArray(overlay)) {
    throw new TypeError(`${context}.overlay must be an array`);
  }
  const contexts = collectOverlayTransferContexts(
    overlay,
    binding.authority,
    context,
  );
  const allTransfers = contexts.flatMap((entry) => entry.transfers);
  for (const transfer of allTransfers) {
    if (transfer.assetDefinitionId !== binding.dsAssetId) continue;
    const transferCoordinate = `${transfer.contextIndex}:${transfer.instructionIndex}${
      transfer.transferEntryIndex === null
        ? ""
        : `:${transfer.transferEntryIndex}`
    }`;
    validationFeeMinorUnits(
      transfer.quantity,
      binding.verified.policy.ds_scale,
      `${context} DS transfer ${transferCoordinate}`,
    );
  }
  const coordinateMatches = allTransfers.filter((transfer) =>
    sameFeeCoordinate(transfer, binding),
  );
  if (coordinateMatches.length === 0) {
    throw new Error(
      `${context} does not contain the validation-fee transfer at overlay coordinate ${binding.instructionIndex}${
        binding.transferEntryIndex === null ? "" : `:${binding.transferEntryIndex}`
      }`,
    );
  }
  if (coordinateMatches.length > 1) {
    throw new Error(
      `${context} validation-fee coordinate ${binding.instructionIndex}${
        binding.transferEntryIndex === null ? "" : `:${binding.transferEntryIndex}`
      } is ambiguous across execution contexts`,
    );
  }
  const coordinate = coordinateMatches[0];
  if (coordinate.contextIndex !== 0) {
    throw new Error(
      `${context} validation-fee coordinate resolves to an unsupported nested multisig execution context`,
    );
  }

  for (const nestedContext of contexts.slice(1)) {
    if (
      nestedContext.transfers.some(
        (transfer) => transfer.assetDefinitionId === binding.dsAssetId,
      )
    ) {
      throw new Error(
        `${context} contains an unsupported nested multisig DS transfer context`,
      );
    }
  }

  if (coordinate.sourceAccountId !== binding.authority) {
    throw new Error(`${context} validation-fee transfer has the wrong source`);
  }
  if (coordinate.assetDefinitionId !== binding.dsAssetId) {
    throw new Error(`${context} validation-fee transfer has the wrong asset`);
  }
  if (coordinate.destinationAccountId !== binding.treasuryAccountId) {
    throw new Error(`${context} validation-fee transfer has the wrong beneficiary`);
  }

  const qualifyingTransferCount = contexts[0].transfers.filter((transfer) => {
    if (transfer === coordinate) return false;
    if (transfer.assetDefinitionId !== binding.dsAssetId) return false;
    return !(
      binding.treasuryPayoutExempt &&
      transfer.sourceAccountId === binding.treasuryAccountId
    );
  }).length;
  if (qualifyingTransferCount === 0) {
    throw new Error(`${context} contains no qualifying DS transfer`);
  }
  if (
    binding.assertedQualifyingTransferCount !== null &&
    qualifyingTransferCount !== binding.assertedQualifyingTransferCount
  ) {
    throw new Error(
      `${context} contains ${qualifyingTransferCount} qualifying DS transfers but validationFeePolicy declares ${binding.assertedQualifyingTransferCount}`,
    );
  }

  const quantity = validationFeeQuantity(
    binding.verified.policy,
    qualifyingTransferCount,
  );
  const expectedMinorUnits = validationFeeMinorUnits(
    quantity,
    binding.verified.policy.ds_scale,
    `${context} expected validation fee`,
  );
  const observedMinorUnits = validationFeeMinorUnits(
    coordinate.quantity,
    binding.verified.policy.ds_scale,
    `${context} validation-fee transfer amount`,
  );
  if (observedMinorUnits !== expectedMinorUnits) {
    throw new Error(
      `${context} validation-fee coordinate must contain exactly ${expectedMinorUnits} minor units (found ${observedMinorUnits})`,
    );
  }

  const requiredTransfer = buildTransferAssetInstruction({
    sourceAssetHoldingId: `${binding.dsAssetId}#${binding.authority}`,
    quantity,
    destinationAccountId: binding.treasuryAccountId,
  });
  return { requiredTransfer, qualifyingTransferCount, quantity };
}

/**
 * Resolve and simulate a deployed contract entrypoint, derive its authoritative
 * ZK IVM overlay, have the node prove that same overlay, sign the exact proved
 * executable, and submit it to the transaction pipeline.
 *
 * `requiredOverlayTransfer` is an assertion, not an appended instruction: the
 * deployed router/pool call must emit that exact transfer itself. This keeps the
 * transfer inside both the node-generated proof commitment and the user-signed
 * transaction payload. Use `submitValidationFeeIvmProvedContractCall` for a
 * fee-bearing call: its signed policy and active registry are verified locally
 * and exclusively determine the fee transfer and reserved policy metadata.
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
  const authority = normalizeAuthority(record.authority);
  const chainId = readExclusiveInputAlias(
    record,
    ["chainId", "chain_id"],
    "input.chainId",
  );
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
  const validationFeeBinding =
    validationFeeIntent === undefined || validationFeeIntent === null
      ? null
      : prepareValidationFeePolicyIntent(
          validationFeeIntent,
          authority,
          chainId,
          getTrustedValidationFeeVerificationContext(client),
        );
  const gasAssetValue = record.gasAssetId ?? record.gas_asset_id;
  const feeSponsorValue = record.feeSponsor ?? record.fee_sponsor;
  const gasAssetId =
    gasAssetValue === undefined || gasAssetValue === null
      ? null
      : normalizeAssetId(gasAssetValue, "gasAssetId");
  const feeSponsor =
    feeSponsorValue === undefined || feeSponsorValue === null
      ? null
      : normalizeAccountId(feeSponsorValue, "feeSponsor");
  const simulationRequest = {
    authority,
    ...(record.contractAddress !== undefined
      ? { contractAddress: record.contractAddress }
      : {}),
    ...(record.contract_address !== undefined
      ? { contract_address: record.contract_address }
      : {}),
    ...(record.contractAlias !== undefined
      ? { contractAlias: record.contractAlias }
      : {}),
    ...(record.contract_alias !== undefined
      ? { contract_alias: record.contract_alias }
      : {}),
    ...(record.entrypoint === undefined
      ? {}
      : { entrypoint: record.entrypoint }),
    ...(record.payload === undefined ? {} : { payload: record.payload }),
    ...(gasAssetId === null ? {} : { gasAssetId }),
    ...(feeSponsor === null ? {} : { feeSponsor }),
    gasLimit: record.gasLimit ?? record.gas_limit,
  };
  const requestOptions = opts.signal === undefined ? {} : { signal: opts.signal };
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

  const code = await client.getContractCodeBytes(simulation.code_hash_hex);
  if (!code?.code_b64) {
    throw new Error(
      `deployed contract bytecode ${simulation.code_hash_hex} is unavailable`,
    );
  }
  assertZkModeIvmBytecode(code.code_b64);

  const metadata = {
    ...normalizeIvmProvedContractMetadata(record.metadata),
    contract_address: simulation.contract_address,
    contract_entrypoint: simulation.entrypoint,
    gas_limit: simulation.gas_limit,
    ...(validationFeeBinding === null ? {} : validationFeeBinding.metadata),
  };
  const contractAlias = record.contractAlias ?? record.contract_alias;
  if (contractAlias !== undefined && contractAlias !== null) {
    metadata.contract_alias = normalizeNonEmptyString(
      contractAlias,
      "contractAlias",
    );
  }
  if (simulation.normalized_payload !== null) {
    metadata.contract_payload = simulation.normalized_payload;
  }
  if (gasAssetId !== null) {
    metadata.gas_asset_id = gasAssetId;
  }
  if (feeSponsor !== null) {
    metadata.fee_sponsor = feeSponsor;
  }

  const proofRequest = {
    vkRef: record.vkRef ?? record.vk_ref,
    authority,
    metadata,
    bytecode: code.code_b64,
  };
  const derived = await client.deriveIvmProved(proofRequest, requestOptions);
  const validationResult =
    validationFeeBinding === null
      ? null
      : assertValidationFeeOverlay(
          derived.proved,
          validationFeeBinding,
          "node-derived proved payload",
        );
  const requiredTransfer =
    validationResult === null
      ? assertRequiredOverlayTransfer(
          derived.proved,
          callerRequiredTransfer,
          "node-derived proved payload",
        )
      : validationResult.requiredTransfer;
  if (
    validationResult !== null &&
    callerRequiredTransfer !== null
  ) {
    if (
      canonicalJsonValue(callerRequiredTransfer) !==
      canonicalJsonValue(requiredTransfer)
    ) {
      throw new Error(
        "requiredOverlayTransfer conflicts with the verified validation-fee policy",
      );
    }
  }

  const proofJob = await client.proveIvmAndWait(
    { ...proofRequest, proved: derived.proved },
    {
      ...requestOptions,
      ...(opts.proofIntervalMs === undefined
        ? {}
        : { intervalMs: opts.proofIntervalMs }),
      ...(opts.proofTimeoutMs === undefined
        ? {}
        : { timeoutMs: opts.proofTimeoutMs }),
    },
  );
  if (canonicalJsonValue(proofJob.proved) !== canonicalJsonValue(derived.proved)) {
    throw new Error(
      "prover returned an IvmProved payload different from the authoritative derived payload",
    );
  }
  if (validationFeeBinding === null) {
    assertRequiredOverlayTransfer(
      proofJob.proved,
      callerRequiredTransfer,
      "proved payload",
    );
  } else {
    const provedValidationResult = assertValidationFeeOverlay(
      proofJob.proved,
      validationFeeBinding,
      "proved payload",
    );
    if (
      canonicalJsonValue(provedValidationResult.requiredTransfer) !==
        canonicalJsonValue(requiredTransfer) ||
      provedValidationResult.qualifyingTransferCount !==
        validationResult.qualifyingTransferCount
    ) {
      throw new Error(
        "proved payload validation-fee binding differs from the derived payload",
      );
    }
  }

  const built = buildIvmProvedTransaction({
    chainId,
    authority,
    proved: proofJob.proved,
    attachment: proofJob.attachment,
    metadata,
    creationTimeMs: record.creationTimeMs ?? record.creation_time_ms ?? null,
    ttlMs: record.ttlMs ?? record.ttl_ms ?? null,
    nonce: record.nonce ?? null,
    privateKey: record.privateKey ?? record.private_key,
    privateKeyAlgorithm:
      record.privateKeyAlgorithm ?? record.private_key_algorithm ?? null,
  });
  const hashHex = built.hash.toString("hex");
  const submission = await client.submitTransaction(built.signedTransaction);
  const status = opts.waitForCommit
    ? await client.waitForTransactionStatusTyped(hashHex, {
        ...(opts.transactionIntervalMs === undefined
          ? {}
          : { intervalMs: opts.transactionIntervalMs }),
        ...(opts.transactionTimeoutMs === undefined
          ? {}
          : { timeoutMs: opts.transactionTimeoutMs }),
        ...(opts.transactionStatusScope === undefined
          ? {}
          : { scope: opts.transactionStatusScope }),
        ...(opts.signal === undefined ? {} : { signal: opts.signal }),
        successStatuses: ["Committed", "Applied"],
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
    requiredOverlayTransfer: requiredTransfer,
    validationFeePolicy:
      validationFeeBinding === null
        ? null
        : {
            policyVersion: Number(validationFeeBinding.verified.policyVersion),
            policyHash: validationFeeBinding.verified.policyHashHex,
            qualifyingTransferCount: validationResult.qualifyingTransferCount,
            feeInstructionIndex: validationFeeBinding.instructionIndex,
            feeTransferEntryIndex: validationFeeBinding.transferEntryIndex,
            feeQuantity: validationResult.quantity,
          },
  };
}

/**
 * Strict validation-fee submission path. Unlike the generic helper, this
 * requires a signed active policy and will not submit a proof-bound call until
 * the real Norito overlay has passed independent fee verification.
 */
export function submitValidationFeeIvmProvedContractCall(
  client,
  input,
  options = {},
) {
  const record = normalizePlainObject(input, "input");
  const validationFeeIntent = readExclusiveInputAlias(
    record,
    ["validationFeePolicy", "validation_fee_policy"],
    "input.validationFeePolicy",
  );
  if (validationFeeIntent === undefined || validationFeeIntent === null) {
    throw new Error(
      "submitValidationFeeIvmProvedContractCall requires validationFeePolicy",
    );
  }
  return submitIvmProvedContractCall(client, record, options);
}

/**
 * Build and sign a transaction containing one archived Kagemusha instruction.
 */
export function buildKagemushaInstructionTransaction({
  chainId,
  authority,
  type,
  instructionType,
  instruction_type,
  instructionArchive,
  instruction_archive,
  archive,
  bytes,
  bytesBase64,
  bytes_base64,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildKagemushaInstructionArchiveInstruction({
    type: instructionType ?? type ?? instruction_type,
    instructionArchive,
    instruction_archive,
    archive,
    bytes,
    bytesBase64,
    bytes_base64,
  });
  return buildTransaction({
    chainId,
    authority,
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
 * Build the native recursive-redeem instruction from a redeem request archive
 * and sign it in a single-instruction transaction.
 */
export function buildKagemushaRecursiveRedeemTransaction({
  chainId,
  authority,
  redeemRequestArchive,
  redeem_request_archive,
  requestArchive,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const native = resolveNativeBinding();
  if (!native || typeof native.kagemushaRecursiveSpendRedeem !== "function") {
    throw new Error(
      "native binding 'kagemushaRecursiveSpendRedeem' is unavailable",
    );
  }
  const selectedArchive =
    redeemRequestArchive ?? redeem_request_archive ?? requestArchive;
  const instructionArchive = Buffer.from(
    native.kagemushaRecursiveSpendRedeem(
      toBuffer(
        selectedArchive,
        "kagemushaRecursiveRedeem.redeemRequestArchive",
      ),
    ),
  );
  return buildKagemushaInstructionTransaction({
    chainId,
    authority,
    type: "RedeemKagemushaRecursive",
    instructionArchive,
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
}

/**
 * Build the native online-to-offline top-up instruction from a top-up request
 * archive and sign it in a single-instruction transaction.
 */
export function buildKagemushaRecursiveTopUpTransaction({
  chainId,
  authority,
  topUpRequestArchive,
  top_up_request_archive,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const native = resolveNativeBinding();
  if (!native || typeof native.kagemushaRecursiveSpendTopUp !== "function") {
    throw new Error(
      "native binding 'kagemushaRecursiveSpendTopUp' is unavailable",
    );
  }
  const selectedArchive =
    topUpRequestArchive
    ?? top_up_request_archive;
  const instructionArchive = Buffer.from(
    native.kagemushaRecursiveSpendTopUp(
      toBuffer(
        selectedArchive,
        "kagemushaRecursiveTopUp.topUpRequestArchive",
      ),
    ),
  );
  return buildKagemushaInstructionTransaction({
    chainId,
    authority,
    type: "TopUpKagemushaRecursive",
    instructionArchive,
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
    privateKeyAlgorithm,
  });
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
    normalizeWholeNumberLiteral(feeAmount, "privateKaigiFeeSpend.feeAmount"),
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
 * Build an asset-hidden transfer v1 proof envelope.
 */
export function buildConfidentialAssetHiddenTransferProofV1({
  chainId,
  poolId,
  assetSetRootHex,
  assetSetRoot,
  inputCommitments,
  inputCommitmentsHex,
  nullifiers,
  nullifiersHex,
  outputCommitments,
  outputCommitmentsHex,
  rootHintHex,
  rootHint,
  verifyingKey,
}) {
  const native = resolveNativeBinding();
  if (
    !native ||
    typeof native.buildConfidentialAssetHiddenTransferProofV1 !== "function"
  ) {
    throw new Error(
      "native binding 'buildConfidentialAssetHiddenTransferProofV1' is unavailable",
    );
  }
  const vk = normalizeInlineVerifyingKeyRecord(
    verifyingKey,
    "confidentialAssetHiddenTransferProofV1",
  );
  const normalizeList = (values, context) =>
    Array.isArray(values)
      ? values.map((entry, index) =>
          normalizeFixed32HexInput(entry, `${context}[${index}]`),
        )
      : [];
  const result = native.buildConfidentialAssetHiddenTransferProofV1(
    normalizeExactMetadataString(chainId, "confidentialAssetHiddenTransferProofV1.chainId"),
    normalizeExactMetadataString(poolId, "confidentialAssetHiddenTransferProofV1.poolId"),
    normalizeFixed32HexInput(assetSetRootHex ?? assetSetRoot, "assetSetRoot"),
    normalizeList(inputCommitmentsHex ?? inputCommitments, "inputCommitments"),
    normalizeList(nullifiersHex ?? nullifiers, "nullifiers"),
    normalizeList(
      outputCommitmentsHex ?? outputCommitments,
      "outputCommitments",
    ),
    normalizeFixed32HexInput(rootHintHex ?? rootHint, "rootHint"),
    vk.backend,
    vk.circuitId,
    vk.bytes,
  );
  return {
    inputCommitments: Array.isArray(
      result.inputCommitments ?? result.input_commitments,
    )
      ? (result.inputCommitments ?? result.input_commitments).map((entry) =>
          Buffer.from(entry),
        )
      : [],
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
 * Build a transaction containing a `ProposeSccpRouteManifest` instruction.
 */
export function buildProposeSccpRouteManifestTransaction({
  chainId,
  authority,
  proposal,
  manifest,
  routeManifest,
  route_manifest,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
  privateKeyAlgorithm = null,
}) {
  const instruction = buildProposeSccpRouteManifestInstruction(
    proposal ?? {
      manifest: manifest ?? routeManifest ?? route_manifest,
    },
  );
  return buildTransaction({
    chainId,
    authority,
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
 * Submit a signed transaction and optionally wait for a terminal status.
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
  let txBuffer = toBuffer(signedTransaction);
  if (options.privateKey) {
    txBuffer = resignSignedTransaction(txBuffer, options.privateKey);
  }
  const hashHex = hashSignedTransaction(txBuffer);
  const submission = await client.submitTransaction(txBuffer);

  if (!options.waitForCommit) {
    return { hash: hashHex, submission };
  }

  const pollIntervalMs = options.pollIntervalMs ?? 500;
  const timeoutMs = options.timeoutMs ?? 30_000;
  const deadline = Date.now() + timeoutMs;

  let status;
  while (Date.now() <= deadline) {
    const statusOptions = {
      allowShortHash: true,
    };
    if (options.scope !== undefined && options.scope !== null) {
      statusOptions.scope = options.scope;
    }
    status = await client.getTransactionStatus(hashHex, statusOptions);
    if (isTerminalStatus(status)) {
      return { hash: hashHex, submission, status };
    }
    // eslint-disable-next-line no-await-in-loop
    await delay(pollIntervalMs);
  }

  const error = new Error("timed out waiting for transaction status");
  error.hash = hashHex;
  error.submission = submission;
  error.status = status;
  throw error;
}

/**
 * Build, submit, and optionally wait for an SNS name-registration transaction.
 * @param {{
 *   client?: ToriiClient,
 *   toriiUrl?: string,
 *   chainId: string,
 *   authority: string,
 *   request: object,
 *   metadata?: object | string | null,
 *   creationTimeMs?: number,
 *   ttlMs?: number,
 *   nonce?: number,
 *   privateKey: ArrayBufferView | ArrayBuffer | Buffer,
 *   waitForCommit?: boolean,
 *   pollIntervalMs?: number,
 *   timeoutMs?: number,
 *   scope?: "local" | "auto" | "global" | string | null
 * }} input
 * @returns {Promise<{hash: string, submittedHash: string | null, submission: any, status?: any}>}
 */
export async function registerSnsNameViaConsensus(input) {
  if (!input || typeof input !== "object") {
    throw new TypeError("input must be an object");
  }
  let client = input.client ?? null;
  if (client === null) {
    const toriiUrl = String(input.toriiUrl ?? "").trim();
    if (!toriiUrl) {
      throw new TypeError("client or toriiUrl is required");
    }
    client = new ToriiClient(toriiUrl);
  }
  if (!(client instanceof ToriiClient)) {
    throw new TypeError("client must be an instance of ToriiClient");
  }
  const transaction = buildRegisterSnsNameTransaction(input);
  try {
    const result = await submitSignedTransaction(
      client,
      transaction.signedTransaction,
      {
        waitForCommit: input.waitForCommit ?? true,
        pollIntervalMs: input.pollIntervalMs,
        timeoutMs: input.timeoutMs,
        scope: input.scope,
      },
    );
    return {
      hash: transaction.hash.toString("hex"),
      submittedHash: result?.hash ?? null,
      submission: result?.submission ?? null,
      status: result?.status ?? null,
    };
  } catch (error) {
    const message = String(error?.message ?? error);
    if (
      !message
        .toLowerCase()
        .includes("timed out waiting for transaction status")
    ) {
      throw error;
    }
    return {
      hash: transaction.hash.toString("hex"),
      submittedHash: error?.hash ?? null,
      submission: error?.submission ?? null,
      status: {
        kind: "PendingTimeout",
        error: message,
        ...(error?.status !== undefined ? { lastStatus: error.status } : {}),
      },
    };
  }
}

/**
 * Submit a raw transaction entrypoint payload and optionally wait for a terminal status.
 * @param {ToriiClient} client
 * @param {ArrayBufferView | ArrayBuffer | Buffer} transactionEntrypoint
 * @param {{ hashHex: string, waitForCommit?: boolean, pollIntervalMs?: number, timeoutMs?: number, scope?: "local" | "auto" | "global" }} options
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
  const hashHex = String(options.hashHex ?? "").trim();
  if (!/^[0-9a-fA-F]{64}$/.test(hashHex)) {
    throw new TypeError("options.hashHex must be a 32-byte hex string");
  }
  const payload = toBuffer(transactionEntrypoint);
  const submission = await client.submitTransaction(payload);

  if (!options.waitForCommit) {
    return { hash: hashHex.toLowerCase(), submission };
  }

  const pollIntervalMs = options.pollIntervalMs ?? 500;
  const timeoutMs = options.timeoutMs ?? 30_000;
  const deadline = Date.now() + timeoutMs;

  let status;
  while (Date.now() <= deadline) {
    const statusOptions = {
      allowShortHash: true,
    };
    if (options.scope !== undefined && options.scope !== null) {
      statusOptions.scope = options.scope;
    }
    status = await client.getTransactionStatus(hashHex, statusOptions);
    if (isTerminalStatus(status)) {
      return { hash: hashHex.toLowerCase(), submission, status };
    }
    // eslint-disable-next-line no-await-in-loop
    await delay(pollIntervalMs);
  }

  throw new Error("timed out waiting for transaction status");
}

function isTerminalStatus(status) {
  if (!status || typeof status !== "object") {
    return false;
  }
  const labels = [];
  const collect = (value) => {
    if (!value || typeof value !== "object") {
      return;
    }
    if (typeof value.status === "string") {
      labels.push(value.status);
    } else if (value.status && typeof value.status === "object") {
      collect(value.status);
    }
    if (typeof value.kind === "string") {
      labels.push(value.kind);
    }
    if (typeof value.type === "string") {
      labels.push(value.type);
    }
  };
  collect(status);
  if (labels.length === 0) {
    return false;
  }
  return labels.some((label) => {
    const normalized = label.toLowerCase();
    return (
      normalized.includes("committed") ||
      normalized.includes("applied") ||
      normalized.includes("rejected") ||
      normalized.includes("failed") ||
      normalized.includes("expired")
    );
  });
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

function delay(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}
