import { Buffer } from "buffer";
import { blake3 } from "@noble/hashes/blake3";

import { AccountAddress } from "./address.js";
import { verifyEd25519 } from "./crypto.browser.js";
import {
  CONTRACT_ADDRESS_V1_VERSION,
  contractAddressHrp,
  encodeContractAddressBech32m,
  requireContractAddressForChain,
} from "./contractAddress.js";
import {
  buildCommitContractDeploymentInstruction,
  buildFinalizeSmartContractCodeUploadInstruction,
  buildRegisterSmartContractCodeInstruction,
  buildUploadSmartContractCodeChunkInstruction,
} from "./instructionBuilders.js";
import { computeIvmArtifactHashes } from "./ivmArtifact.js";
import { verifyIvmContractArtifactAdmission } from "./ivmArtifactAdmissionWasm.js";
import { verifyCompiledContractArtifact } from "./kotodamaCompiler/normalize.js";
import { noritoEncodeContractManifestSignaturePayload } from "./norito.js";
import {
  browserTransactionPayloadHashHex,
  buildBrowserInstructionTransactionPayload,
  finalizeBrowserInstructionTransaction,
  validateBrowserInstructionTransactionSignable,
} from "./transactionCodec.js";

export const SMART_CONTRACT_CODE_CHUNK_BYTES = 65_536;
const U16_MAX = 0xffffn;
const U32_MAX = 0xffff_ffffn;
const U64_MAX = 0xffff_ffff_ffff_ffffn;
const CONTRACT_ADDRESS_DOMAIN = Buffer.from(
  "iroha:contract-address:v1",
  "utf8",
);
const CONTRACT_ADDRESS_HASH_BYTES = 20;
const HASH_LITERAL_PATTERN = /^hash:([0-9A-F]{64})#[0-9A-F]{4}$/u;
const CURRENT_IVM_ABI_VERSION = 1;
const CURRENT_DATA_MODEL_VERSION = 3;
const CURRENT_SIGNED_TRANSACTION_SCHEMA_HASH_HEX =
  "7ab5ff9c572efb316deac478f19209c5";

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
  if (typeof value !== "string" || !/^[0-9a-fA-F]{64}$/u.test(value)) {
    throw new TypeError(`${context} must be an exact 32-byte hexadecimal string`);
  }
  return value.toLowerCase();
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

function validateNodeCapabilities(value) {
  const capabilities = requirePlainObject(value, "node capabilities");
  if (capabilities.abi_version !== CURRENT_IVM_ABI_VERSION) {
    throw new Error(
      `node capabilities abi_version must be ${CURRENT_IVM_ABI_VERSION}`,
    );
  }
  if (capabilities.data_model_version !== CURRENT_DATA_MODEL_VERSION) {
    throw new Error(
      `node capabilities data_model_version must be ${CURRENT_DATA_MODEL_VERSION}`,
    );
  }
  const signedSchema = capabilities.signed_transaction_schema_hash_hex;
  if (
    typeof signedSchema !== "string" ||
    !/^[0-9a-f]{32}$/u.test(signedSchema) ||
    signedSchema !== CURRENT_SIGNED_TRANSACTION_SCHEMA_HASH_HEX
  ) {
    throw new Error(
      "node capabilities signed_transaction_schema_hash_hex does not match this client",
    );
  }
  return Object.freeze({
    abiVersion: capabilities.abi_version,
    dataModelVersion: capabilities.data_model_version,
    signedTransactionSchemaHashHex: signedSchema,
  });
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

function normalizeContractAlias(value, context = "contractAlias") {
  const literal = requireExactString(value, context);
  if (/[@#$]/u.test(literal) || /\s/u.test(literal)) {
    throw new TypeError(`${context} must use canonical contract-alias syntax`);
  }
  const separator = literal.indexOf("::");
  if (
    separator <= 0 ||
    separator !== literal.lastIndexOf("::") ||
    separator + 2 >= literal.length
  ) {
    throw new TypeError(
      `${context} must use <name>::<dataspace> or <name>::<domain>.<dataspace>`,
    );
  }
  const name = literal.slice(0, separator);
  const scope = literal.slice(separator + 2);
  const scopeParts = scope.split(".");
  if (
    scopeParts.length < 1 ||
    scopeParts.length > 2 ||
    [name, ...scopeParts].some(
      (segment) => segment.length === 0 || segment.includes(":"),
    )
  ) {
    throw new TypeError(`${context} contains an invalid alias segment`);
  }
  return Object.freeze({
    literal,
    dataspaceAlias: scopeParts[scopeParts.length - 1],
  });
}

function u16Be(value) {
  const output = Buffer.allocUnsafe(2);
  output.writeUInt16BE(Number(value));
  return output;
}

function u64Be(value) {
  const output = Buffer.allocUnsafe(8);
  output.writeBigUInt64BE(value);
  return output;
}

function authorityDetails(authority, expectedDiscriminant) {
  const literal = requireExactString(authority, "authority");
  let parsed;
  try {
    parsed = AccountAddress.parseEncoded(literal, Number(expectedDiscriminant));
  } catch (error) {
    throw new TypeError(`authority is not a canonical I105 account: ${error.message}`);
  }
  if (parsed.chainDiscriminant !== Number(expectedDiscriminant)) {
    throw new TypeError("authority chain discriminant does not match deployment chain");
  }
  if (parsed.address.toI105(Number(expectedDiscriminant)) !== literal) {
    throw new TypeError("authority must use its exact canonical I105 literal");
  }
  const controller = parsed.address._controller;
  if (
    !controller ||
    controller.tag !== 0 ||
    controller.curve !== 1 ||
    controller.publicKey?.length !== 32
  ) {
    throw new TypeError(
      "browser deployment requires a single-key Ed25519 I105 authority",
    );
  }
  return Object.freeze({
    literal,
    canonicalBytes: Buffer.from(parsed.address.canonicalBytes()),
    signingPublicKey: Buffer.from(controller.publicKey),
  });
}

/** Derive the exact current V1 Bech32m contract address locally. */
export function deriveContractAddress({
  chainDiscriminant,
  authority,
  deployNonce,
  dataspaceId,
}) {
  const discriminant = normalizeUnsigned(
    chainDiscriminant,
    U16_MAX,
    "chainDiscriminant",
  );
  const nonce = normalizeUnsigned(deployNonce, U64_MAX, "deployNonce");
  const dataspace = normalizeUnsigned(dataspaceId, U64_MAX, "dataspaceId");
  const authorityInfo = authorityDetails(authority, discriminant);
  const preimage = Buffer.concat([
    CONTRACT_ADDRESS_DOMAIN,
    u16Be(discriminant),
    u64Be(dataspace),
    u64Be(nonce),
    authorityInfo.canonicalBytes,
  ]);
  const digest = Buffer.from(blake3(preimage));
  const payload = Buffer.concat([
    Buffer.of(CONTRACT_ADDRESS_V1_VERSION),
    u64Be(dataspace),
    digest.subarray(0, CONTRACT_ADDRESS_HASH_BYTES),
  ]);
  return encodeContractAddressBech32m(contractAddressHrp(discriminant), payload);
}

/**
 * Verify compiler identities and create exact bounded upload/finalize/manifest
 * instructions. The artifact bytes and normalized manifest are detached from
 * caller-owned mutable values.
 */
export function prepareBrowserContractArtifact({
  artifactBytes,
  manifest,
  compilerCodeHash,
  compilerAbiHash,
}) {
  const artifact = copyBytes(artifactBytes, "artifactBytes");
  const hashes = computeIvmArtifactHashes(artifact);
  const codeHash = normalizedHashHex(compilerCodeHash, "compilerCodeHash");
  if (hashes.codeHashHex !== codeHash) {
    throw new Error(
      `compiled artifact code hash mismatch: computed ${hashes.codeHashHex}, compiler supplied ${codeHash}`,
    );
  }
  const unsignedManifestInstruction = buildRegisterSmartContractCodeInstruction({
    manifest,
  });
  const normalizedManifest =
    unsignedManifestInstruction.RegisterSmartContractCode.manifest;
  const manifestCodeHash = normalizedHashHex(
    normalizedManifest.code_hash,
    "manifest.code_hash",
  );
  if (manifestCodeHash !== codeHash) {
    throw new Error("manifest.code_hash does not match the compiled artifact");
  }
  const abiHash = normalizedHashHex(compilerAbiHash, "compilerAbiHash");
  const manifestAbiHash = normalizedHashHex(
    normalizedManifest.abi_hash,
    "manifest.abi_hash",
  );
  if (manifestAbiHash !== abiHash) {
    throw new Error("manifest.abi_hash does not match the compiler ABI hash");
  }
  verifyCompiledContractArtifact(
    artifact,
    normalizedManifest,
    codeHash,
    abiHash,
  );
  const chunkCount = Math.ceil(artifact.length / SMART_CONTRACT_CODE_CHUNK_BYTES);
  if (chunkCount < 1 || BigInt(chunkCount) > U32_MAX) {
    throw new RangeError("artifact cannot be represented by the native chunk protocol");
  }
  const uploads = [];
  for (let index = 0; index < chunkCount; index += 1) {
    const start = index * SMART_CONTRACT_CODE_CHUNK_BYTES;
    const chunk = Buffer.from(
      artifact.subarray(start, start + SMART_CONTRACT_CODE_CHUNK_BYTES),
    );
    uploads.push(
      Object.freeze({
        kind: "upload_chunk",
        index,
        instruction: buildUploadSmartContractCodeChunkInstruction({
          codeHash,
          totalSize: artifact.length,
          chunkIndex: index,
          chunkCount,
          chunk,
        }),
      }),
    );
  }
  const steps = [
    ...uploads,
    Object.freeze({
      kind: "finalize_upload",
      instruction: buildFinalizeSmartContractCodeUploadInstruction({
        codeHash,
        totalSize: artifact.length,
        chunkCount,
      }),
    }),
  ];
  return Object.freeze({
    artifactBytes: artifact,
    artifactSha256Hex: hashes.artifactSha256Hex,
    codeHash,
    abiHash,
    manifest: normalizedManifest,
    chunkCount,
    steps: Object.freeze(steps),
  });
}

function requireSharedArtifactAdmission(prepared, verifier) {
  const admission = verifyIvmContractArtifactAdmission(
    verifier,
    prepared.artifactBytes,
  );
  if (!admission.ok) {
    throw new Error(`shared IVM artifact admission rejected deployment: ${admission.error}`);
  }
  if (admission.codeHashHex !== prepared.codeHash) {
    throw new Error(
      "shared IVM artifact admission code hash does not match the compiled artifact",
    );
  }
  if (admission.abiHashHex !== prepared.abiHash) {
    throw new Error(
      "shared IVM artifact admission ABI hash does not match the compiler ABI hash",
    );
  }
  if (
    admission.headerLength > admission.codeOffset ||
    admission.codeOffset > prepared.artifactBytes.length
  ) {
    throw new Error("shared IVM artifact admission returned invalid artifact offsets");
  }
  const admittedManifest =
    buildRegisterSmartContractCodeInstruction({ manifest: admission.manifest })
      .RegisterSmartContractCode.manifest;
  if (admittedManifest.provenance !== null) {
    throw new Error("shared IVM artifact admission manifest must be unsigned");
  }
  if (admittedManifest.entrypoints.length !== admission.entrypointCount) {
    throw new Error(
      "shared IVM artifact admission entrypoint count disagrees with its manifest",
    );
  }
  const suppliedPayload = noritoEncodeContractManifestSignaturePayload(
    prepared.manifest,
  );
  const admittedPayload = noritoEncodeContractManifestSignaturePayload(
    admittedManifest,
  );
  if (
    !admittedPayload.equals(suppliedPayload) ||
    JSON.stringify(admittedManifest) !== JSON.stringify(prepared.manifest)
  ) {
    throw new Error(
      "shared IVM artifact admission manifest does not match the compiler manifest",
    );
  }
  return Object.freeze({
    verifierSha256Hex: verifier.verifierSha256Hex,
    headerLength: admission.headerLength,
    codeOffset: admission.codeOffset,
    entrypointCount: admission.entrypointCount,
  });
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
    throw new Error(`${context} did not return terminal Applied status`);
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
  chainId,
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
    chainId,
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

/**
 * Execute the exact current locally signed deployment sequence. Signing and
 * canonical state reads are callbacks so browser keystores can retain existing
 * user keys; this API never accepts or transmits a private key.
 */
export async function deploySmartContractBrowser(options) {
  const source = requirePlainObject(options, "deployment options");
  if (typeof source.sign !== "function") {
    throw new TypeError("deployment options.sign must be a local signer callback");
  }
  if (typeof source.submitAndWait !== "function") {
    throw new TypeError(
      "deployment options.submitAndWait must submit signed bytes and await finality",
    );
  }
  if (typeof source.signManifest !== "function") {
    throw new TypeError(
      "deployment options.signManifest must be a local manifest signer callback",
    );
  }
  if (typeof source.readNodeCapabilities !== "function") {
    throw new TypeError(
      "deployment options.readNodeCapabilities must fetch fresh node capabilities",
    );
  }
  if (typeof source.readDeploymentState !== "function") {
    throw new TypeError(
      "deployment options.readDeploymentState must call the authenticated deployment-state endpoint",
    );
  }
  if (
    source.feePayment === undefined &&
    typeof source.feePaymentForStep !== "function"
  ) {
    throw new TypeError(
      "deployment options require feePayment or a feePaymentForStep callback",
    );
  }
  const chainId = requireExactString(source.chainId, "chainId");
  const chainDiscriminant = normalizeUnsigned(
    source.chainDiscriminant,
    U16_MAX,
    "chainDiscriminant",
  );
  const authority = authorityDetails(source.authority, chainDiscriminant);
  const contractAlias = normalizeContractAlias(source.contractAlias);
  const prepared = prepareBrowserContractArtifact(source);
  const artifactAdmission = requireSharedArtifactAdmission(
    prepared,
    source.artifactAdmissionVerifier,
  );
  const nodeCapabilities = validateNodeCapabilities(
    await source.readNodeCapabilities(
      Object.freeze({
        chainId,
        chainDiscriminant: chainDiscriminant.toString(),
      }),
    ),
  );
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
    const previous = requireContractAddressForChain(
      state.previousContractAddress,
      chainDiscriminant,
      "previousContractAddress",
    );
    if (previous.dataspaceId !== state.dataspaceId) {
      throw new Error(
        "previousContractAddress belongs to a different deployment dataspace",
      );
    }
  }
  const contractAddress = deriveContractAddress({
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
      chainId,
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
    artifactAdmission,
    transactions: Object.freeze(results),
  });
}
