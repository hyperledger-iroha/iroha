import { getNativeBinding } from "./native.js";
import { ToriiClient } from "./toriiClient.js";
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
  buildDeactivateContractInstanceInstruction,
  buildActivateContractInstanceInstruction,
  buildRemoveSmartContractBytesInstruction,
  buildProposeDeployContractInstruction,
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
} from "./instructionBuilders.js";

function normalizeAuthority(authority) {
  const raw = String(authority ?? "").trim();
  if (!raw) {
    return normalizeAccountId(authority, "authority");
  }
  return normalizeAccountId(raw, "authority");
}

function resolveNativeBinding() {
  // Allow tests to inject a fake binding.
  return globalThis.__IROHA_NATIVE_BINDING__ ?? getNativeBinding();
}

function composeAssetHoldingIdFromDefinitionAndAccount(assetDefinitionId, accountId, context) {
  const definition = String(assetDefinitionId ?? "").trim();
  if (!definition) {
    throw new TypeError(`${context}.assetDefinitionId must be a non-empty string`);
  }
  if (
    /\s/.test(definition) ||
    definition.includes("%") ||
    definition.includes("/") ||
    definition.includes("?") ||
    definition.includes(":")
  ) {
    throw new TypeError(
      `${context}.assetDefinitionId must be a canonical unprefixed Base58 asset definition id`,
    );
  }
  const normalizedAccountId = normalizeAccountId(accountId, `${context}.accountId`);
  return `${definition}#${normalizedAccountId}`;
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
  throw new TypeError(`${context} must be an object or JSON string when provided`);
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
 * Convert a versioned signed transaction payload into Norito bytes for Torii `/transaction` submit routes.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} signedTransaction
 * @returns {Buffer}
 */
export function encodeSignedTransactionNorito(signedTransaction) {
  const native = resolveNativeBinding();
  if (!native || typeof native.encodeSignedTransactionNorito !== "function") {
    throw new Error("native binding 'encodeSignedTransactionNorito' is unavailable");
  }
  const txBuffer = toBuffer(signedTransaction);
  return Buffer.from(native.encodeSignedTransactionNorito(txBuffer));
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
 *   privateKey: ArrayBufferView | ArrayBuffer | Buffer
 * }} input
 * @returns {{signedTransaction: Buffer, hash: Buffer}}
 */
export function buildRegisterDomainTransaction(input) {
  const native = resolveNativeBinding();
  if (!native || typeof native.buildRegisterDomainTransaction !== "function") {
    throw new Error("native binding 'build_register_domain_transaction' is unavailable");
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
  );
  const signed =
    result?.signed_transaction ??
    result?.signedTransaction ??
    null;
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
 *   privateKey: ArrayBufferView | ArrayBuffer | Buffer
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
  } = input;

  const normalizedInstructions = serializeInstructionPayloads(
    instructions,
    "instructions",
  );

  const metadataPayload = normalizeMetadataPayload(metadata, "transaction metadata");

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
  );

  const signed =
    result?.signed_transaction ??
    result?.signedTransaction ??
    null;
  const hashBytes = result?.hash ?? result?.hashBytes ?? null;
  if (!signed || !hashBytes) {
    throw new Error("native binding 'build_transaction' returned missing fields");
  }

  return {
    signedTransaction: Buffer.from(signed),
    hash: Buffer.from(hashBytes),
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
    throw new TypeError("buildPrecommitTriggerAction options must be an object");
  }
  const { authority, instructions, repeats = null, metadata = null } = options;
  const native = resolveNativeBinding();
  if (!native || typeof native.buildPrecommitTriggerAction !== "function") {
    throw new Error("native binding 'buildPrecommitTriggerAction' is unavailable");
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
}) {
  const instruction = buildMintAssetInstruction({ assetHoldingId: assetHoldingId ?? assetId, quantity });
  return buildTransaction({
    chainId,
    authority,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
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
}) {
  const instruction = buildBurnAssetInstruction({ assetHoldingId: assetHoldingId ?? assetId, quantity });
  return buildTransaction({
    chainId,
    authority,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
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
  const instructions = [];
  instructions.push(
    buildRegisterAccountInstruction({
      accountId: account.accountId,
      domainId: account.domainId ?? account.domain,
      metadata: account.metadata,
    }),
  );
  transfers.forEach((transfer) => {
    const sourceAssetHoldingId = transfer.sourceAssetHoldingId ?? transfer.sourceAssetId;
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
  });
}

/**
 * Build instructions combining an asset definition registration with an optional mint.
 */
function buildRegisterAssetDefinitionInstructions({ assetDefinition, mints = [] }) {
  const instructions = [];
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
        id: assetDefinition.assetDefinitionId,
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

function resolveAssetHoldingIdForMint(assetDefinitionId, mint, context = "mint") {
  const providedAssetHoldingId = mint.assetHoldingId ?? mint.assetId;
  if (providedAssetHoldingId) {
    const normalizedAssetHoldingId = ToriiClient._normalizeAssetHoldingId(
      providedAssetHoldingId,
      mint.assetHoldingId !== undefined ? `${context}.assetHoldingId` : `${context}.assetId`,
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
    throw new TypeError(`${context}.assetHoldingId or ${context}.accountId must be provided`);
  }
  return composeAssetHoldingIdFromDefinitionAndAccount(assetDefinitionId, mint.accountId, context);
}

function normalizeDomainMintSpec(value, context) {
  if (!value || typeof value !== "object") {
    throw new TypeError(`${context} must be an object`);
  }
  const assetId = value.assetId;
  if (typeof assetId !== "string" || assetId.length === 0) {
    throw new TypeError(`${context}.assetId must be a non-empty string`);
  }
  return {
    assetId: ToriiClient._normalizeAssetId(assetId, `${context}.assetId`),
    quantity: value.quantity,
  };
}

function normalizeDomainMintSpecs(value, context) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array of mint descriptors`);
  }
  return value.map((item, index) => normalizeDomainMintSpec(item, `${context}[${index}]`));
}

function normalizeAssetDefinitionMintSpec(assetDefinitionId, value, context) {
  if (!value || typeof value !== "object") {
    throw new TypeError(`${context} must be an object`);
  }
  const assetHoldingId = resolveAssetHoldingIdForMint(assetDefinitionId, value, context);
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
    throw new TypeError(`${context} must be an array of asset mint descriptors`);
  }
  if (value.length === 0) {
    throw new TypeError(`${context} must contain at least one entry`);
  }
  return value.map((item, index) =>
    normalizeAssetDefinitionMintSpec(assetDefinitionId, item, `${context}[${index}]`),
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
    throw new TypeError(`${context}.sourceAssetHoldingId is required`);
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
  if (!defaultSource && transferSpecs.some((spec) => spec.sourceAssetHoldingId === undefined)) {
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
}) {
  if (!assetDefinition || typeof assetDefinition !== "object") {
    throw new TypeError("assetDefinition registration parameters are required");
  }
  if (mint && mints) {
    throw new TypeError("provide either mint or mints, but not both");
  }
  const mintSpecs =
    mints !== undefined
      ? normalizeAssetDefinitionMintSpecs(assetDefinition.assetDefinitionId, mints, "mints")
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
      ? normalizeAssetDefinitionMintSpecs(assetDefinition.assetDefinitionId, mints, "mints")
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
  });
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
  const record =
    verifyingKey && typeof verifyingKey === "object" && !Array.isArray(verifyingKey)
      ? verifyingKey
      : null;
  if (!record) {
    throw new TypeError("privateKaigiFeeSpend.verifyingKey must be an object");
  }
  const inlineKey =
    record.inline_key ??
    record.inlineKey ??
    null;
  if (!inlineKey || typeof inlineKey !== "object" || Array.isArray(inlineKey)) {
    throw new TypeError(
      "privateKaigiFeeSpend.verifyingKey.inline_key must be present",
    );
  }
  const bytesBase64 = String(
    inlineKey.bytes_b64 ??
      inlineKey.bytesBase64 ??
      "",
  ).trim();
  if (!bytesBase64) {
    throw new TypeError(
      "privateKaigiFeeSpend.verifyingKey.inline_key.bytes_b64 must be present",
    );
  }
  const result = native.buildPrivateKaigiFeeSpend(
    String(chainId ?? "").trim(),
    String(assetDefinitionId ?? "").trim(),
    toBuffer(actionHash),
    String(anchorRootHex ?? "").trim(),
    String(feeAmount ?? "").trim(),
    String(record.id?.backend ?? record.backend ?? "").trim(),
    String(record.record?.circuit_id ?? record.circuit_id ?? record.circuitId ?? "").trim(),
    Buffer.from(bytesBase64, "base64"),
  );
  return {
    asset_definition_id: String(result.assetDefinitionId ?? result.asset_definition_id),
    anchor_root: Buffer.from(result.anchorRoot ?? result.anchor_root),
    nullifiers: Array.isArray(result.nullifiers)
      ? result.nullifiers.map((entry) => Buffer.from(entry))
      : [],
    output_commitments: Array.isArray(result.outputCommitments ?? result.output_commitments)
      ? (result.outputCommitments ?? result.output_commitments).map((entry) => Buffer.from(entry))
      : [],
    encrypted_change_payloads: Array.isArray(
      result.encryptedChangePayloads ?? result.encrypted_change_payloads,
    )
      ? (result.encryptedChangePayloads ?? result.encrypted_change_payloads).map((entry) =>
          Buffer.from(entry),
        )
      : [],
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
  if (!native || typeof native.buildPrivateCreateKaigiTransaction !== "function") {
    throw new Error(
      "native binding 'buildPrivateCreateKaigiTransaction' is unavailable",
    );
  }
  const result = native.buildPrivateCreateKaigiTransaction(
    String(chainId ?? "").trim(),
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
  if (!native || typeof native.buildPrivateJoinKaigiTransaction !== "function") {
    throw new Error(
      "native binding 'buildPrivateJoinKaigiTransaction' is unavailable",
    );
  }
  const result = native.buildPrivateJoinKaigiTransaction(
    String(chainId ?? "").trim(),
    String(callId ?? "").trim(),
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
    String(chainId ?? "").trim(),
    String(callId ?? "").trim(),
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
}) {
  const instruction = buildScheduleConfidentialPolicyTransitionInstruction(transition);
  return buildTransaction({
    chainId,
    authority,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
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
}) {
  const instruction = buildCancelConfidentialPolicyTransitionInstruction(cancellation);
  return buildTransaction({
    chainId,
    authority,
    instructions: [instruction],
    metadata,
    creationTimeMs,
    ttlMs,
    nonce,
    privateKey,
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
  });
}

/**
 * Build a transaction containing a `DeactivateContractInstance` instruction.
 */
export function buildDeactivateContractInstanceTransaction({
  chainId,
  authority,
  namespace,
  contractId,
  reason = null,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
}) {
  const instruction = buildDeactivateContractInstanceInstruction({
    namespace,
    contractId,
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
  });
}

/**
 * Build a transaction containing an `ActivateContractInstance` instruction.
 */
export function buildActivateContractInstanceTransaction({
  chainId,
  authority,
  namespace,
  contractId,
  codeHash,
  metadata = null,
  creationTimeMs = null,
  ttlMs = null,
  nonce = null,
  privateKey,
}) {
  const instruction = buildActivateContractInstanceInstruction({
    namespace,
    contractId,
    codeHash,
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
    status = await client.getTransactionStatus(hashHex, { allowShortHash: true });
    if (isTerminalStatus(status)) {
      return { hash: hashHex, submission, status };
    }
    // eslint-disable-next-line no-await-in-loop
    await delay(pollIntervalMs);
  }

  throw new Error("timed out waiting for transaction status");
}

/**
 * Submit a raw transaction entrypoint payload and optionally wait for a terminal status.
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
    throw new TypeError("options.hashHex is required for entrypoint submission");
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
    status = await client.getTransactionStatus(hashHex, { allowShortHash: true });
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
  const value = status.status;
  if (typeof value !== "string") {
    return false;
  }
  const normalized = value.toLowerCase();
  return (
    normalized.includes("committed") ||
    normalized.includes("rejected") ||
    normalized.includes("failed")
  );
}

function toBuffer(value) {
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (ArrayBuffer.isView(value)) {
    return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return Buffer.from(value);
  }
  throw new TypeError("signedTransaction must be a Buffer or ArrayBuffer view");
}

function delay(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}
