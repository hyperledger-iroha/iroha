import { Buffer } from "buffer";
import { blake3 } from "@noble/hashes/blake3";

import { AccountAddress } from "./address.js";
import {
  CONTRACT_ADDRESS_HRP,
  CONTRACT_ADDRESS_V1_VERSION,
  encodeContractAddressBech32m,
} from "./contractAddress.js";
import {
  buildCommitContractDeploymentInstruction,
  buildFinalizeSmartContractCodeUploadInstruction,
  buildRegisterSmartContractCodeInstruction,
  buildUploadSmartContractCodeChunkInstruction,
} from "./instructionBuilders.js";
import { computeIvmArtifactHashes } from "./ivmArtifact.js";
import { verifyCompiledContractArtifact } from "./kotodamaCompiler/normalize.js";
import { networkIdBytes } from "./networkId.js";

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
const CURRENT_DATA_MODEL_VERSION = 4;
const CURRENT_SIGNED_TRANSACTION_SCHEMA_HASH_HEX =
  "7ab5ff9c572efb316deac478f19209c5";
const BROWSER_DEPLOYMENT_OPTION_KEYS = Object.freeze([
  "artifactBytes",
  "manifest",
  "compilerCodeHash",
  "compilerAbiHash",
  "networkId",
  "chainDiscriminant",
  "authority",
  "contractAlias",
  "leaseExpiryMs",
  "ttlMs",
  "feePayment",
  "feePaymentForStep",
  "metadata",
  "clock",
  "nonceForStep",
  "metadataForStep",
  "sign",
  "signManifest",
  "readNodeCapabilities",
  "submitAndWait",
  "readDeploymentState",
]);

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

function u32Be(value) {
  const output = Buffer.allocUnsafe(4);
  output.writeUInt32BE(Number(value));
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
export function deriveContractAddress(input) {
  const source = requirePlainObject(input, "contract-address derivation input");
  assertOnlyObjectKeys(
    source,
    ["networkId", "chainDiscriminant", "authority", "deployNonce", "dataspaceId"],
    "contract-address derivation input",
  );
  const networkBytes = Buffer.from(
    networkIdBytes(source.networkId, "contract-address derivation input.networkId"),
  );
  const discriminant = normalizeUnsigned(
    source.chainDiscriminant,
    U16_MAX,
    "chainDiscriminant",
  );
  const nonce = normalizeUnsigned(source.deployNonce, U64_MAX, "deployNonce");
  const dataspace = normalizeUnsigned(source.dataspaceId, U64_MAX, "dataspaceId");
  const authorityInfo = authorityDetails(source.authority, discriminant);
  const preimage = Buffer.concat([
    CONTRACT_ADDRESS_DOMAIN,
    networkBytes,
    u64Be(dataspace),
    u64Be(nonce),
    u32Be(BigInt(authorityInfo.canonicalBytes.length)),
    authorityInfo.canonicalBytes,
  ]);
  const digest = Buffer.from(blake3(preimage));
  const payload = Buffer.concat([
    Buffer.of(CONTRACT_ADDRESS_V1_VERSION),
    u64Be(dataspace),
    digest.subarray(0, CONTRACT_ADDRESS_HASH_BYTES),
  ]);
  return encodeContractAddressBech32m(CONTRACT_ADDRESS_HRP, payload);
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

/**
 * Execute the exact current locally signed deployment sequence. Signing and
 * canonical state reads are callbacks so browser keystores can retain existing
 * user keys; this API never accepts or transmits a private key.
 */
export async function deploySmartContractBrowser(options) {
  const source = requirePlainObject(options, "deployment options");
  assertOnlyObjectKeys(
    source,
    BROWSER_DEPLOYMENT_OPTION_KEYS,
    "deployment options",
  );
  networkIdBytes(source.networkId, "deployment options.networkId");
  const networkId = source.networkId;
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
  const chainDiscriminant = normalizeUnsigned(
    source.chainDiscriminant,
    U16_MAX,
    "chainDiscriminant",
  );
  const authority = authorityDetails(source.authority, chainDiscriminant);
  const contractAlias = normalizeContractAlias(source.contractAlias);
  const prepared = prepareBrowserContractArtifact(source);
  const nodeCapabilities = validateNodeCapabilities(
    await source.readNodeCapabilities(
      Object.freeze({
        networkId,
        chainDiscriminant: chainDiscriminant.toString(),
      }),
    ),
  );
  const { continueDeploySmartContractBrowser } = await import(
    "./smartContractDeploymentSubmit.js"
  );
  return continueDeploySmartContractBrowser({
    source,
    networkId,
    chainDiscriminant,
    authority,
    contractAlias,
    prepared,
    nodeCapabilities,
    deriveContractAddress,
  });
}
