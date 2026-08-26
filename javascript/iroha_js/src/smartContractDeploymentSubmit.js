import { Buffer } from "buffer";

import { verifyEd25519Strict as verifyEd25519 } from "./ed25519Strict.js";
import { parseCanonicalContractAddress } from "./contractAddress.js";
import {
  buildCommitContractDeploymentInstruction,
  buildRegisterSmartContractCodeInstruction,
} from "./instructionBuilders.js";
import { noritoEncodeContractManifestSignaturePayload } from "./norito.js";
import {
  browserTransactionPayloadHashHex,
  buildBrowserInstructionTransactionPayload,
  finalizeBrowserInstructionTransaction,
  validateBrowserInstructionTransactionSignable,
} from "./transactionCodec.js";

const U16_MAX = 0xffffn;
const U64_MAX = 0xffff_ffff_ffff_ffffn;
const HASH_LITERAL_PATTERN = /^hash:([0-9A-F]{64})#[0-9A-F]{4}$/u;

function requirePlainObject(value, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${context} must be a plain object`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new TypeError(`${context} must be a plain object`);
  }
  return value;
}

function assertOnlyObjectKeys(value, allowed, context) {
  const unexpected = Object.keys(value).filter((key) => !allowed.includes(key));
  if (unexpected.length > 0) {
    throw new TypeError(
      `${context} contains unsupported fields: ${unexpected.sort().join(", ")}`,
    );
  }
}

function requireExactString(value, context) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.trim() !== value ||
    /[\u0000-\u001F\u007F-\u009F]/u.test(value) ||
    value.normalize("NFC") !== value
  ) {
    throw new TypeError(
      `${context} must be a non-empty exact NFC string without control characters`,
    );
  }
  return value;
}

function requireExactHashHex(value, context) {
  if (typeof value !== "string" || !/^[0-9a-f]{63}[13579bdf]$/u.test(value)) {
    throw new TypeError(
      `${context} must be an exact canonical lowercase 32-byte Iroha hash`,
    );
  }
  return value;
}

function normalizeUnsigned(value, maximum, context) {
  let normalized;
  if (typeof value === "bigint") {
    normalized = value;
  } else if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) {
      throw new TypeError(
        `${context} must be a safe integer, bigint, or canonical decimal string`,
      );
    }
    normalized = BigInt(value);
  } else if (typeof value === "string" && /^(?:0|[1-9]\d*)$/u.test(value)) {
    normalized = BigInt(value);
  } else {
    throw new TypeError(
      `${context} must be an unsigned integer, bigint, or canonical decimal string`,
    );
  }
  if (normalized < 0n || normalized > maximum) {
    throw new RangeError(`${context} is outside its unsigned integer range`);
  }
  return normalized;
}

function copyBytes(value, context) {
  if (Buffer.isBuffer(value)) return Buffer.from(value);
  if (ArrayBuffer.isView(value)) {
    return Buffer.from(
      new Uint8Array(value.buffer, value.byteOffset, value.byteLength),
    );
  }
  if (value instanceof ArrayBuffer) return Buffer.from(new Uint8Array(value));
  throw new TypeError(`${context} must be bytes`);
}

function normalizeDetachedEd25519Signature(value, context) {
  let bytes;
  if (typeof value === "string") {
    if (!/^[0-9A-Fa-f]{128}$/u.test(value)) {
      throw new TypeError(`${context} string must be exactly 64 bytes of hexadecimal`);
    }
    bytes = Buffer.from(value, "hex");
  } else if (
    Buffer.isBuffer(value) ||
    ArrayBuffer.isView(value) ||
    value instanceof ArrayBuffer
  ) {
    bytes = copyBytes(value, context);
  } else {
    const envelope = requirePlainObject(value, context);
    assertOnlyObjectKeys(
      envelope,
      ["algorithm", "signature", "bytes", "payload"],
      context,
    );
    if (
      envelope.algorithm !== undefined &&
      envelope.algorithm !== "ed25519" &&
      envelope.algorithm !== 0
    ) {
      throw new TypeError(`${context}.algorithm must be ed25519`);
    }
    const aliases = ["signature", "bytes", "payload"].filter(
      (field) => envelope[field] !== undefined,
    );
    if (aliases.length !== 1) {
      throw new TypeError(
        `${context} must provide exactly one of signature, bytes, or payload`,
      );
    }
    return normalizeDetachedEd25519Signature(envelope[aliases[0]], context);
  }
  if (bytes.length !== 64) {
    throw new TypeError(`${context} must contain exactly 64 bytes`);
  }
  return bytes;
}

function normalizedHashHex(value, context) {
  let normalized;
  try {
    normalized = buildCommitContractDeploymentInstruction({
      expectedDeployNonce: 0,
      contractAddress: "c0-placeholder",
      codeHash: value,
      contractAlias: "placeholder::universal",
    }).CommitContractDeployment.code_hash;
  } catch (error) {
    throw new TypeError(`${context} is not a canonical 32-byte hash: ${error.message}`);
  }
  const match = HASH_LITERAL_PATTERN.exec(normalized);
  if (!match) {
    throw new TypeError(`${context} did not normalize to a canonical hash literal`);
  }
  return match[1].toLowerCase();
}

async function buildSignedManifestRegistrationStep({
  prepared,
  authority,
  signManifest,
}) {
  if (prepared.manifest.provenance !== null) {
    throw new Error("compiler manifest must be unsigned before local provenance signing");
  }
  const payloadBytes = noritoEncodeContractManifestSignaturePayload(
    prepared.manifest,
  );
  const signature = normalizeDetachedEd25519Signature(
    await signManifest(
      Object.freeze({
        payloadBytes: Buffer.from(payloadBytes),
        payloadBase64: payloadBytes.toString("base64"),
        signingPublicKey: Buffer.from(authority.signingPublicKey),
        signatureAlgorithm: "ed25519",
        manifest: prepared.manifest,
        codeHash: prepared.codeHash,
        abiHash: prepared.abiHash,
      }),
    ),
    "manifest signature",
  );
  let verified = false;
  try {
    verified = verifyEd25519(
      payloadBytes,
      signature,
      authority.signingPublicKey,
    );
  } catch {
    verified = false;
  }
  if (!verified) {
    throw new Error(
      "manifest signature does not verify over the canonical Norito payload",
    );
  }
  const provenance = Object.freeze({
    signer: `ed0120${authority.signingPublicKey.toString("hex").toUpperCase()}`,
    signature: signature.toString("hex").toUpperCase(),
  });
  const instruction = buildRegisterSmartContractCodeInstruction({
    manifest: {
      ...prepared.manifest,
      provenance,
    },
  });
  const signedManifest = instruction.RegisterSmartContractCode.manifest;
  const canonicalPayload = noritoEncodeContractManifestSignaturePayload(
    signedManifest,
  );
  if (!canonicalPayload.equals(payloadBytes)) {
    throw new Error("signed manifest changed its canonical provenance payload");
  }
  return Object.freeze({
    kind: "register_manifest",
    instruction,
  });
}

function requireAppliedStatus(result, expectedHash, context) {
  const envelope = requirePlainObject(result, `${context} status`);
  const observedHash = requireExactHashHex(envelope.hash, `${context} status hash`);
  if (observedHash !== expectedHash) {
    throw new Error(`${context} status hash does not match the submitted transaction`);
  }
  if (envelope.scope !== "global") {
    throw new Error(`${context} status scope must be global`);
  }
  if (envelope.resolved_from !== "state") {
    throw new Error(`${context} status must be resolved from persisted state`);
  }
  const status = requirePlainObject(envelope.status, `${context} status payload`);
  if (status.kind !== "Applied") {
    throw new Error(`${context} did not return state-resolved Applied finality`);
  }
  if (!Number.isSafeInteger(status.block_height) || status.block_height < 1) {
    throw new Error(`${context} Applied status must include a positive block height`);
  }
}

function requireCanonicalDecimal(value, maximum, context) {
  if (typeof value !== "string" || !/^(?:0|[1-9]\d*)$/u.test(value)) {
    throw new TypeError(`${context} must be a canonical decimal string`);
  }
  return normalizeUnsigned(value, maximum, context);
}

function validateDeploymentState(
  value,
  { authority, contractAlias, dataspaceAlias, chainDiscriminant },
) {
  const state = requirePlainObject(value, "deployment state");
  const fields = [
    "authority",
    "contract_alias",
    "deploy_nonce",
    "dataspace_alias",
    "dataspace_id",
    "previous_contract_address",
    "observed_block_height",
    "observed_block_hash",
    "ledger_time_ms",
    "chain_discriminant",
  ];
  assertOnlyObjectKeys(state, fields, "deployment state");
  for (const field of fields) {
    if (!Object.hasOwn(state, field)) {
      throw new TypeError(`deployment state is missing ${field}`);
    }
  }
  if (state.authority !== authority) {
    throw new Error("deployment state authority does not match the deployment authority");
  }
  if (state.contract_alias !== contractAlias) {
    throw new Error("deployment state alias does not match the requested alias");
  }
  if (state.dataspace_alias !== dataspaceAlias) {
    throw new Error("deployment state disagrees with the alias dataspace");
  }
  const observedChainDiscriminant = requireCanonicalDecimal(
    state.chain_discriminant,
    U16_MAX,
    "deployment state chain_discriminant",
  );
  if (observedChainDiscriminant !== chainDiscriminant) {
    throw new Error("deployment state chain discriminant does not match the deployment chain");
  }
  const deployNonce = requireCanonicalDecimal(
    state.deploy_nonce,
    U64_MAX,
    "deployment state deploy_nonce",
  );
  const dataspaceId = requireCanonicalDecimal(
    state.dataspace_id,
    U64_MAX,
    "deployment state dataspace_id",
  );
  const observedBlockHeight = requireCanonicalDecimal(
    state.observed_block_height,
    U64_MAX,
    "deployment state observed_block_height",
  );
  if (observedBlockHeight < 1n) {
    throw new Error("deployment state observed_block_height must be positive");
  }
  const observedBlockHash = requireExactString(
    state.observed_block_hash,
    "deployment state observed_block_hash",
  );
  if (!HASH_LITERAL_PATTERN.test(observedBlockHash)) {
    throw new Error(
      "deployment state observed_block_hash must be a canonical Iroha hash literal",
    );
  }
  const observedBlockHashHex = normalizedHashHex(
    observedBlockHash,
    "deployment state observed_block_hash",
  );
  const ledgerTimeMs = requireCanonicalDecimal(
    state.ledger_time_ms,
    U64_MAX,
    "deployment state ledger_time_ms",
  );
  const previous = state.previous_contract_address;
  return Object.freeze({
    deployNonce,
    dataspaceId,
    previousContractAddress:
      previous === null
        ? null
        : requireExactString(previous, "previousContractAddress"),
    observedBlockHeight,
    observedBlockHash,
    observedBlockHashHex,
    ledgerTimeMs,
  });
}

async function submitDeploymentStep({
  step,
  networkId,
  authority,
  chainDiscriminant,
  signingPublicKey,
  sign,
  submitAndWait,
  creationTimeMs,
  ttlMs,
  nonce,
  feePayment,
  metadata,
}) {
  const payloadBytes = buildBrowserInstructionTransactionPayload({
    networkId,
    authority,
    chainDiscriminant,
    instructions: [step.instruction],
    creationTimeMs,
    ttlMs,
    nonce,
    feePayment,
    metadata,
  });
  const signable = validateBrowserInstructionTransactionSignable({
    networkId,
    payloadBytes,
    payloadHashHex: browserTransactionPayloadHashHex(payloadBytes),
    authority,
    signingPublicKey,
    signatureAlgorithm: "ed25519",
  });
  const signature = await sign(
    Object.freeze({
      ...signable,
      payloadBytes: Buffer.from(signable.payloadBytes),
      signingPublicKey: Buffer.from(signable.signingPublicKey),
      payloadHashBytes: Buffer.from(signable.payloadHashHex, "hex"),
      step,
    }),
  );
  const finalized = finalizeBrowserInstructionTransaction(
    signable,
    signature,
    signingPublicKey,
  );
  const status = await submitAndWait(
    Object.freeze({
      signedTransaction: Buffer.from(finalized.signedTransaction),
      hashHex: finalized.hashHex,
      step,
    }),
  );
  requireAppliedStatus(status, finalized.hashHex, `deployment step ${step.kind}`);
  return Object.freeze({
    kind: step.kind,
    hashHex: finalized.hashHex,
    status,
  });
}

export async function continueDeploySmartContractBrowser({
  source,
  networkId,
  chainDiscriminant,
  authority,
  contractAlias,
  prepared,
  nodeCapabilities,
  deriveContractAddress,
}) {
  const state = validateDeploymentState(
    await source.readDeploymentState(
      Object.freeze({
        authority: authority.literal,
        contract_alias: contractAlias.literal,
      }),
    ),
    {
      authority: authority.literal,
      contractAlias: contractAlias.literal,
      dataspaceAlias: contractAlias.dataspaceAlias,
      chainDiscriminant,
    },
  );
  if (state.previousContractAddress !== null) {
    const previous = parseCanonicalContractAddress(
      state.previousContractAddress,
      "previousContractAddress",
    );
    if (previous.dataspaceId !== state.dataspaceId) {
      throw new Error(
        "previousContractAddress belongs to a different deployment dataspace",
      );
    }
  }
  const contractAddress = deriveContractAddress({
    networkId,
    chainDiscriminant,
    authority: authority.literal,
    deployNonce: state.deployNonce,
    dataspaceId: state.dataspaceId,
  });
  const registerManifestStep = await buildSignedManifestRegistrationStep({
    prepared,
    authority,
    signManifest: source.signManifest,
  });
  const results = [];
  let sequence = 0;
  const submit = async (step) => {
    const stepFeePayment =
      typeof source.feePaymentForStep === "function"
        ? await source.feePaymentForStep(step)
        : source.feePayment;
    const stepMetadata =
      typeof source.metadataForStep === "function"
        ? await source.metadataForStep(step)
        : source.metadata;
    const stepNonce =
      typeof source.nonceForStep === "function"
        ? await source.nonceForStep(step, sequence)
        : null;
    const stepCreationTime =
      typeof source.clock === "function" ? source.clock() : Date.now();
    sequence += 1;
    const result = await submitDeploymentStep({
      step,
      networkId,
      authority: authority.literal,
      chainDiscriminant,
      signingPublicKey: authority.signingPublicKey,
      sign: source.sign,
      submitAndWait: source.submitAndWait,
      creationTimeMs: stepCreationTime,
      ttlMs: source.ttlMs ?? null,
      nonce: stepNonce,
      feePayment: stepFeePayment,
      metadata: stepMetadata ?? null,
    });
    results.push(result);
  };
  const commitStep = Object.freeze({
    kind: "commit_deployment",
    instruction: buildCommitContractDeploymentInstruction({
      expectedDeployNonce: state.deployNonce,
      contractAddress,
      codeHash: prepared.codeHash,
      contractAlias: contractAlias.literal,
      leaseExpiryMs: source.leaseExpiryMs ?? null,
      expectedPreviousContractAddress: state.previousContractAddress,
    }),
  });
  for (const step of [...prepared.steps, registerManifestStep, commitStep]) {
    await submit(step);
  }
  return Object.freeze({
    contractAddress,
    contractAlias: contractAlias.literal,
    codeHash: prepared.codeHash,
    abiHash: prepared.abiHash,
    artifactSha256Hex: prepared.artifactSha256Hex,
    deployNonce: state.deployNonce.toString(),
    dataspaceId: state.dataspaceId.toString(),
    previousContractAddress: state.previousContractAddress,
    observedBlockHeight: state.observedBlockHeight.toString(),
    observedBlockHash: state.observedBlockHash,
    observedBlockHashHex: state.observedBlockHashHex,
    ledgerTimeMs: state.ledgerTimeMs.toString(),
    nodeCapabilities,
    transactions: Object.freeze(results),
  });
}
