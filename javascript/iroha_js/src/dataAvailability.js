import fs from "node:fs/promises";
import path from "node:path";

import { blake3 } from "@noble/hashes/blake3.js";
import { AccountAddress } from "./address.js";
import { canonicalPublicKeyMultihashHex } from "./normalizers.js";
import { publicKeyFromPrivate, publicKeyMultihash, signEd25519 } from "./crypto.js";
import {
  defaultNativeRuntime,
  resolveNativeRuntimeBinding,
} from "./nativeRuntime.js";
import { NetworkId, networkIdBytes } from "./networkId.js";
import { normalizeSorafsPinU64 } from "./sorafsPinDetail.js";

const DEFAULT_CHUNK_SIZE = 262_144;
const DEFAULT_ERASURE_PROFILE = {
  dataShards: 10,
  parityShards: 4,
  rowParityStripes: 0,
  chunkAlignment: 10,
  fecScheme: "Rs12_10",
};
const DEFAULT_RETENTION_POLICY = {
  hotRetentionSecs: 7 * 24 * 60 * 60,
  coldRetentionSecs: 90 * 24 * 60 * 60,
  requiredReplicas: 3,
  storageClass: "Hot",
  governanceTag: "da.default",
};
const MAX_SAFE_UINT = Number.MAX_SAFE_INTEGER;
const MAX_SAFE_UINT_BIGINT = BigInt(MAX_SAFE_UINT);
const DA_INGEST_REQUEST_SIGNING_DOMAIN_V1 = Buffer.from(
  "iroha:da-ingest-request:v1\0",
  "utf8",
);
const DA_INGEST_REQUEST_CONTENT_DOMAIN_V1 = Buffer.from(
  "iroha:da-ingest-request:content:v1\0",
  "utf8",
);

const DA_PART_BYTES = 1024 * 1024;
const DA_MAX_WIRE_BYTES = 64 * DA_PART_BYTES;

// Send the same signed request through either Torii representation. A new caller
// resumes from the acknowledged prefix, including when a prior Finish was interrupted.
export async function postDaIngestPhase(request, wireBytes, post) {
  if (!Buffer.isBuffer(wireBytes) || wireBytes.length === 0 || wireBytes.length > DA_MAX_WIRE_BYTES) {
    throw new RangeError("DA transmitted payload must contain 1 byte to 64 MiB");
  }
  if (wireBytes.length <= DA_PART_BYTES) return post("/v1/da/ingest", request, 202);
  const upload = {
    network_id: request.network_id,
    lane_id: request.lane_id,
    epoch: request.epoch,
    wire_bytes: wireBytes.length,
    wire_hash: [Array.from(blake3(wireBytes))],
  };
  const exchange = (kind, value, status) =>
    post("/v1/da/ingest/parts", { upload, action: { kind, value } }, status);
  const count = Math.ceil(wireBytes.length / DA_PART_BYTES);
  const next = async (kind, value) => {
    const progress = await exchange(kind, value, 200);
    if (!progress || Object.keys(progress).length !== 1 ||
        !Number.isSafeInteger(progress.next_part) || progress.next_part < 0 || progress.next_part > count) {
      throw new Error("Torii returned invalid DA upload progress");
    }
    return progress.next_part;
  };
  for (let index = await next("Begin", null); index < count; index += 1) {
    const part = wireBytes.subarray(index * DA_PART_BYTES, (index + 1) * DA_PART_BYTES);
    if (await next("Put", { index, bytes_b64: part.toString("base64") }) !== index + 1) {
      throw new Error("Torii returned inconsistent DA part acknowledgment");
    }
  }
  return exchange("Finish", { ...request, payload: "" }, 202);
}

export function buildDaIngestRequest(options = {}) {
  const { request, artifacts } = prepareDaIngest(options, true);
  return { request, artifacts };
}

function boundedDaBytes(value, name) {
  const length = typeof value === "string" ? Buffer.byteLength(value) : value?.byteLength ?? value?.length;
  if (!Number.isSafeInteger(length) || length < 1 || length > DA_MAX_WIRE_BYTES) {
    throw new RangeError(`${name} must contain 1 byte to 64 MiB`);
  }
  return toBuffer(value, name);
}

export function prepareDaIngest(options, includeFullPayload) {
  networkIdBytes(options.networkId, "networkId");
  const networkId = options.networkId.toString();
  const owner = requireNonEmptyString(options.owner, "owner");
  if (owner !== options.owner) {
    throw new TypeError("owner must be an exact canonical I105 account id");
  }
  AccountAddress.fromI105(owner);
  const payloadBuffer = boundedDaBytes(options.payload, "payload");
  const compression = normalizeEnumLiteral(options.compression ?? "Identity", "compression", [
    "Identity", "Gzip", "Deflate", "Zstd",
  ]);
  if ((compression === "Identity") !== (options.compressedPayload == null)) {
    throw new TypeError("compressedPayload is required exactly when DA transport compression is selected");
  }
  const wire = compression === "Identity" ? payloadBuffer : boundedDaBytes(options.compressedPayload, "compressedPayload");
  // A caller may mutate a Buffer while an HTTP acknowledgment is pending.
  const wireBytes = wire.length > DA_PART_BYTES ? Buffer.from(wire) : wire;
  const chunkSize = normalizeUnsignedInteger(
    options.chunkSize ?? DEFAULT_CHUNK_SIZE,
    "chunkSize",
    { allowZero: false },
  );
  const laneId = normalizeSorafsPinU64(options.laneId ?? 0, "laneId", 0xffff_ffffn);
  const epoch = normalizeSorafsPinU64(options.epoch ?? 0, "epoch");
  const sequence = normalizeSorafsPinU64(options.sequence ?? 0, "sequence");

  const codec = tupleWrap(
    requireNonEmptyString(options.codec ?? "application/octet-stream", "codec"),
  );
  const blobClass = encodeBlobClass(options.blobClass ?? "TaikaiSegment");
  const erasureProfile = encodeErasureProfile(options.erasureProfile);
  const retentionPolicy = encodeRetentionPolicy(options.retentionPolicy);
  const metadata = encodeMetadata(options.metadata);

  const { digestTuple, digestHex } = resolveClientBlobId(options.clientBlobId, payloadBuffer);
  const payloadHash = Buffer.from(blake3(payloadBuffer));
  const request = {
    network_id: networkId,
    owner,
    client_blob_id: digestTuple,
    lane_id: laneId,
    epoch,
    sequence,
    blob_class: blobClass,
    codec,
    erasure_profile: erasureProfile,
    retention_policy: retentionPolicy,
    chunk_size: chunkSize,
    total_size: payloadBuffer.length,
    payload_hash: tupleWrap(encodeFixedBytes(payloadHash, 32)),
    compression: { kind: compression, value: null },
    norito_manifest:
      options.noritoManifest === undefined || options.noritoManifest === null
        ? null
        : toBuffer(options.noritoManifest, "noritoManifest").toString("base64"),
    payload: includeFullPayload || wireBytes.length <= DA_PART_BYTES ? wireBytes.toString("base64") : "",
    metadata,
  };

  const signingDigest = daSigningDigest(request, wireBytes);
  const signatureInfo = resolveSignature(options, signingDigest);
  request.signatures = [{
    signer: signatureInfo.signerPublicKey,
    signature: signatureInfo.signatureHex,
  }];

  request.pin_scope_signatures = [];
  if (options.pinScopeSignatureHex !== undefined) {
    const signature = canonicalizeHex(options.pinScopeSignatureHex, "pinScopeSignatureHex");
    if (signature.length !== 128) throw new RangeError("pinScopeSignatureHex must be a 64-byte Ed25519 signature");
    request.pin_scope_signatures.push({ signer: signatureInfo.signerPublicKey, signature });
  }

  return {
    request,
    wireBytes,
    artifacts: {
      clientBlobIdHex: digestHex,
      payloadHashHex: bufferToHex(payloadHash),
      signerPublicKey: signatureInfo.signerPublicKey,
      signatureHex: signatureInfo.signatureHex,
      signingDigestHex: bufferToHex(signingDigest),
      payloadLength: payloadBuffer.length,
    },
  };
}

/**
 * Compute the version-one domain-separated DA request signing digest.
 *
 * The input is the normalized request object returned by
 * {@link buildDaIngestRequest}, excluding the `signatures` witness vector.
 */
export function computeDaIngestSigningDigest(requestInput) {
  const request = ensureRecord(requestInput, "DA ingest signing request");
  return daSigningDigest(request, Buffer.from(request.payload, "base64"));
}

function daSigningDigest(request, wireBytes) {
  const content = blake3.create().update(DA_INGEST_REQUEST_CONTENT_DOMAIN_V1);
  const append = (...pieces) => { for (const piece of pieces) content.update(piece); };

  append(
    fixedDigestFromTuple(request.client_blob_id, "client_blob_id"),
  );

  const blobClass = ensureRecord(request.blob_class, "blob_class");
  const blobClassTags = {
    TaikaiSegment: 0,
    NexusLaneSidecar: 1,
    GovernanceArtifact: 2,
    Custom: 3,
  };
  const blobClassName = requireEnumKey(blobClass.class, blobClassTags, "blob_class.class");
  append(
    Buffer.of(blobClassTags[blobClassName]),
    encodeUnsignedLe(blobClassName === "Custom" ? blobClass.value : 0, 2, "blob_class.value"),
  );
  append(encodeLengthPrefixedUtf8(unwrapTupleString(request.codec, "codec")));

  const erasure = ensureRecord(request.erasure_profile, "erasure_profile");
  append(
    encodeUnsignedLe(erasure.data_shards, 2, "erasure_profile.data_shards"),
    encodeUnsignedLe(erasure.parity_shards, 2, "erasure_profile.parity_shards"),
    encodeUnsignedLe(erasure.row_parity_stripes, 2, "erasure_profile.row_parity_stripes"),
    encodeUnsignedLe(erasure.chunk_alignment, 2, "erasure_profile.chunk_alignment"),
  );
  const fec = ensureRecord(erasure.fec_scheme, "erasure_profile.fec_scheme");
  const fecTags = { Rs12_10: 0, RsWin14_10: 1, Rs18_14: 2, Custom: 3 };
  const fecName = requireEnumKey(fec.scheme, fecTags, "erasure_profile.fec_scheme.scheme");
  append(
    Buffer.of(fecTags[fecName]),
    encodeUnsignedLe(fecName === "Custom" ? fec.value : 0, 2, "erasure_profile.fec_scheme.value"),
  );

  const retention = ensureRecord(request.retention_policy, "retention_policy");
  append(
    encodeUnsignedLe(retention.hot_retention_secs, 8, "retention_policy.hot_retention_secs"),
    encodeUnsignedLe(retention.cold_retention_secs, 8, "retention_policy.cold_retention_secs"),
    encodeUnsignedLe(retention.required_replicas, 2, "retention_policy.required_replicas"),
  );
  const storage = ensureRecord(retention.storage_class, "retention_policy.storage_class");
  const storageTags = { Hot: 0, Warm: 1, Cold: 2 };
  const storageName = requireEnumKey(
    storage.type,
    storageTags,
    "retention_policy.storage_class.type",
  );
  append(
    Buffer.of(storageTags[storageName]),
    encodeLengthPrefixedUtf8(unwrapTupleString(retention.governance_tag, "governance_tag")),
    encodeUnsignedLe(request.chunk_size, 4, "chunk_size"),
  );

  const compressionTags = { Identity: 0, Gzip: 1, Deflate: 2, Zstd: 3 };
  const compressionName = requireEnumKey(
    request.compression?.kind,
    compressionTags,
    "compression",
  );
  if (request.compression.value !== null) throw new TypeError("compression requires a null tagged value");
  append(Buffer.of(compressionTags[compressionName]));

  if (!Object.prototype.hasOwnProperty.call(request, "norito_manifest")) {
    throw new TypeError("norito_manifest is required and must be null or a base64 string");
  }
  if (request.norito_manifest === null) {
    append(Buffer.of(0));
  } else {
    append(
      Buffer.of(1),
      encodeLengthPrefixedBytes(
        Buffer.from(request.norito_manifest, "base64"),
        "norito_manifest",
      ),
    );
  }
  append(encodeUnsignedLe(wireBytes.length, 8, "payload.length"), wireBytes);

  const metadata = ensureRecord(request.metadata, "metadata");
  const items = Array.isArray(metadata.items) ? metadata.items : [];
  append(encodeUnsignedLe(items.length, 8, "metadata.items.length"));
  items.forEach((entryInput, index) => {
    const entry = ensureRecord(entryInput, `metadata.items[${index}]`);
    append(
      encodeLengthPrefixedUtf8(entry.key),
      encodeLengthPrefixedBytes(
        Buffer.from(entry.value, "base64"),
        `metadata.items[${index}].value`,
      ),
    );
    const visibility = ensureRecord(
      entry.visibility,
      `metadata.items[${index}].visibility`,
    );
    const visibilityTags = { Public: 0, GovernanceOnly: 1 };
    const visibilityName = requireEnumKey(
      visibility.visibility,
      visibilityTags,
      `metadata.items[${index}].visibility.visibility`,
    );
    append(Buffer.of(visibilityTags[visibilityName]));

    const encryption = ensureRecord(
      entry.encryption,
      `metadata.items[${index}].encryption`,
    );
    if (encryption.cipher === "None") {
      append(Buffer.of(0));
    } else if (encryption.cipher === "ChaCha20Poly1305") {
      append(Buffer.of(1));
      const label = encryption.params?.key_label;
      if (label === undefined || label === null) {
        append(Buffer.of(0));
      } else {
        append(Buffer.of(1), encodeLengthPrefixedUtf8(String(label)));
      }
    } else {
      throw new TypeError(
        `metadata.items[${index}].encryption.cipher is not supported`,
      );
    }
  });

  // The signed content commitment is an Iroha hash: BLAKE3 with its final bit set.
  const contentHash = Buffer.from(content.digest());
  contentHash[contentHash.length - 1] |= 1;
  const networkId = NetworkId.parse(
    requireNonEmptyString(request.network_id, "network_id"),
  );
  const owner = requireNonEmptyString(request.owner, "owner");
  const ownerAddress = AccountAddress.fromI105(owner);
  const payloadHash = fixedDigestFromTuple(request.payload_hash, "payload_hash");
  const authorizationParts = [
    DA_INGEST_REQUEST_SIGNING_DOMAIN_V1,
    Buffer.from(networkIdBytes(networkId, "network_id")),
    encodeLengthPrefixedBytes(Buffer.from(ownerAddress.canonicalBytes()), "owner"),
    encodeUnsignedLe(request.lane_id, 4, "lane_id"),
    encodeUnsignedLe(request.epoch, 8, "epoch"),
    encodeUnsignedLe(request.sequence, 8, "sequence"),
    payloadHash,
    encodeUnsignedLe(request.total_size, 8, "total_size"),
    contentHash,
  ];
  return Buffer.from(blake3(Buffer.concat(authorizationParts)));
}

export function deriveDaChunkerHandle(manifestBytes, options = {}) {
  return deriveDaChunkerHandleWithRuntime(
    defaultNativeRuntime,
    manifestBytes,
    options,
  );
}

function deriveDaChunkerHandleWithRuntime(
  nativeRuntime,
  manifestBytes,
  options = {},
) {
  const record =
    options === undefined || options === null
      ? {}
      : ensureRecord(options, "deriveDaChunkerHandle options");
  assertSupportedOptionKeys(
    record,
    new Set(),
    "deriveDaChunkerHandle options",
  );
  const binding = resolveDaNativeBinding(nativeRuntime, "daManifestChunkerHandle");
  const buffer = toBuffer(manifestBytes, "deriveDaChunkerHandle.manifestBytes");
  return binding.daManifestChunkerHandle(buffer);
}

export function generateDaProofSummary(
  manifestBytesInput,
  payloadBytesInput,
  options = {},
) {
  return generateDaProofSummaryWithRuntime(
    defaultNativeRuntime,
    manifestBytesInput,
    payloadBytesInput,
    options,
  );
}

function generateDaProofSummaryWithRuntime(
  nativeRuntime,
  manifestBytesInput,
  payloadBytesInput,
  options = {},
) {
  const manifestBytes = toBuffer(manifestBytesInput, "manifestBytes");
  if (manifestBytes.length === 0) {
    throw new TypeError("manifestBytes must contain at least one byte");
  }
  const payloadBytes = toBuffer(payloadBytesInput, "payloadBytes");
  if (payloadBytes.length === 0) {
    throw new TypeError("payloadBytes must contain at least one byte");
  }
  const optionsRecord =
    options === undefined || options === null ? {} : ensureRecord(options, "daProofOptions");
  assertSupportedOptionKeys(
    optionsRecord,
    new Set([
      "sampleCount",
      "sampleSeed",
      "leafIndexes",
      "sample_count",
      "sample_seed",
      "leaf_indexes",
    ]),
    "generateDaProofSummary options",
  );
  const binding = resolveDaNativeBinding(nativeRuntime, "daGenerateProofs");
  const nativeOptions = normalizeProofOptions(optionsRecord);
  const rawSummary = binding.daGenerateProofs(
    manifestBytes,
    payloadBytes,
    nativeOptions,
  );
  return transformDaProofSummary(rawSummary);
}

/** @internal Source-level DA facade bound to one immutable native runtime. */
export function _createDataAvailabilityApi(nativeRuntime) {
  return Object.freeze({
    deriveDaChunkerHandle: (manifestBytes, options = {}) =>
      deriveDaChunkerHandleWithRuntime(nativeRuntime, manifestBytes, options),
    generateDaProofSummary: (
      manifestBytes,
      payloadBytes,
      options = {},
    ) =>
      generateDaProofSummaryWithRuntime(
        nativeRuntime,
        manifestBytes,
        payloadBytes,
        options,
      ),
  });
}

export function buildDaProofSummaryArtifact(summaryInput, options = {}) {
  const summary = ensureRecord(summaryInput, "daProofSummary");
  const record = options == null ? {} : ensureRecord(options, "daProofSummaryArtifact options");
  const proofs = Array.isArray(summary.proofs) ? summary.proofs : [];
  return {
    manifest_path: normalizeOptionalPath(record.manifestPath ?? record.manifest_path),
    payload_path: normalizeOptionalPath(record.payloadPath ?? record.payload_path),
    blob_hash: toLowerHexField(
      summary.blob_hash_hex,
      "daProofSummary.blob_hash_hex",
    ),
    chunk_root: toLowerHexField(
      summary.chunk_root_hex,
      "daProofSummary.chunk_root_hex",
    ),
    por_root: toLowerHexField(
      summary.por_root_hex,
      "daProofSummary.por_root_hex",
    ),
    leaf_count: toJsonInteger(
      summary.leaf_count,
      "daProofSummary.leaf_count",
    ),
    segment_count: toJsonInteger(
      summary.segment_count,
      "daProofSummary.segment_count",
    ),
    chunk_count: toJsonInteger(
      summary.chunk_count,
      "daProofSummary.chunk_count",
    ),
    sample_count: toJsonInteger(
      summary.sample_count,
      "daProofSummary.sample_count",
    ),
    sample_seed: toJsonInteger(
      summary.sample_seed,
      "daProofSummary.sample_seed",
    ),
    proof_count: toJsonInteger(
      summary.proof_count ?? proofs.length,
      "daProofSummary.proof_count",
    ),
    proofs: proofs.map((proof, index) => buildDaProofRecord(proof, index)),
  };
}

export async function emitDaProofSummaryArtifact(options = {}) {
  const record = ensureRecord(options ?? {}, "emitDaProofSummaryArtifact options");
  let summary = record.summary ?? null;
  if (!summary) {
    const manifestBytes = toBuffer(
      record.manifestBytes ?? record.manifest_bytes,
      "emitDaProofSummaryArtifact.manifestBytes",
    );
    const payloadBytes = toBuffer(
      record.payloadBytes ?? record.payload_bytes,
      "emitDaProofSummaryArtifact.payloadBytes",
    );
    const proofOptions = record.proofOptions ?? record.proof_options;
    summary = generateDaProofSummary(manifestBytes, payloadBytes, proofOptions);
  }
  const artifact = buildDaProofSummaryArtifact(summary, record);
  const outputPathInput = record.outputPath ?? record.output_path;
  let outputPath = null;
  if (outputPathInput) {
    outputPath = path.resolve(String(outputPathInput));
    const spacing = resolveJsonSpacing(record.pretty);
    const jsonBody = JSON.stringify(artifact, null, spacing);
    await fs.mkdir(path.dirname(outputPath), { recursive: true });
    await fs.writeFile(outputPath, `${jsonBody}\n`, "utf8");
  }
  return { summary, artifact, outputPath };
}

function resolveClientBlobId(explicit, payloadBuffer) {
  if (explicit !== undefined && explicit !== null) {
    const digestBuffer = normalizeDigestInput(explicit, "clientBlobId");
    return {
      digestTuple: tupleWrap(encodeFixedBytes(digestBuffer, 32)),
      digestHex: bufferToHex(digestBuffer),
    };
  }
  const digestBuffer = Buffer.from(blake3(payloadBuffer));
  return {
    digestTuple: tupleWrap(encodeFixedBytes(digestBuffer, 32)),
    digestHex: bufferToHex(digestBuffer),
  };
}

function resolveSignature(options, signingDigest) {
  const signer = options.signerPublicKey
    ? canonicalPublicKeyMultihashHex(options.signerPublicKey, "signerPublicKey")
    : null;

  if (options.signatureHex) {
    const signatureHex = canonicalizeHex(options.signatureHex, "signatureHex");
    if (!signer && !options.privateKey && !options.privateKeyHex) {
      throw new TypeError("signerPublicKey or privateKey is required when providing signatureHex");
    }
    return {
      signatureHex,
      signerPublicKey:
        signer ?? publicKeyMultihash(publicKeyFromPrivate(normalizePrivateKey(options)), { algorithm: "ed25519" }),
    };
  }

  const privateKey = normalizePrivateKey(options);
  const signature = signEd25519(signingDigest, privateKey);
  const signatureHex = bufferToHex(signature);
  const signerPublicKey = publicKeyMultihash(publicKeyFromPrivate(privateKey), { algorithm: "ed25519" });
  if (signer !== null && signer !== signerPublicKey) {
    throw new TypeError("signerPublicKey must match the signing private key");
  }
  return { signatureHex, signerPublicKey };
}

function fixedDigestFromTuple(value, name) {
  if (
    !Array.isArray(value) ||
    value.length !== 1 ||
    !Array.isArray(value[0])
  ) {
    throw new TypeError(`${name} must be a single fixed-byte tuple`);
  }
  const bytes = normalizeByteArray(value[0], name);
  if (bytes.length !== 32) {
    throw new RangeError(`${name} must contain exactly 32 bytes`);
  }
  return bytes;
}

function encodeUnsignedLe(value, width, name) {
  const maximum = (1n << BigInt(width * 8)) - 1n;
  const integer = BigInt(normalizeSorafsPinU64(value, name, maximum));
  const bytes = Buffer.alloc(width);
  let remaining = integer;
  for (let index = 0; index < width; index += 1) {
    bytes[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  return bytes;
}

function encodeLengthPrefixedUtf8(value) {
  if (typeof value !== "string") {
    throw new TypeError("signing string must be a string");
  }
  return encodeLengthPrefixedBytes(
    Buffer.from(value, "utf8"),
    "signing string",
  );
}

function encodeLengthPrefixedBytes(bytes, name) {
  const normalized = toBuffer(bytes, name);
  return Buffer.concat([
    encodeUnsignedLe(normalized.length, 8, `${name}.length`),
    normalized,
  ]);
}

function unwrapTupleString(value, name) {
  if (!Array.isArray(value) || value.length !== 1 || typeof value[0] !== "string") {
    throw new TypeError(`${name} must be a single-string tuple`);
  }
  return value[0];
}

function requireEnumKey(value, tags, name) {
  const key = requireNonEmptyString(value, name);
  if (!Object.prototype.hasOwnProperty.call(tags, key)) {
    throw new TypeError(`${name} contains unsupported variant ${key}`);
  }
  return key;
}

function normalizePrivateKey(options) {
  if (options.privateKeyHex !== undefined && options.privateKeyHex !== null) {
    return parseHexPrivateKey(options.privateKeyHex, "privateKeyHex");
  }
  if (options.privateKey !== undefined && options.privateKey !== null) {
    const buffer = toBuffer(options.privateKey, "privateKey");
    if (buffer.length !== 32 && buffer.length !== 64) {
      throw new Error("privateKey must be a 32- or 64-byte Ed25519 key");
    }
    return buffer;
  }
  throw new TypeError("privateKey or privateKeyHex is required to sign the DA payload");
}

function normalizeDigestInput(value, name) {
  if (typeof value === "string") {
    return parseHexSeed(value, name, 32);
  }
  const buffer = toBuffer(value, name);
  if (buffer.length !== 32) {
    throw new Error(`${name} must be 32 bytes`);
  }
  return buffer;
}

function parseHexSeed(value, name, expectedLength = 32) {
  if (typeof value !== "string") {
    throw new TypeError(`${name} must be a hex string`);
  }
  const cleaned = value.trim().replace(/^0x/i, "");
  if (cleaned.length !== expectedLength * 2) {
    throw new Error(`${name} must be ${expectedLength * 2} hex characters`);
  }
  const buffer = Buffer.from(cleaned, "hex");
  if (buffer.length !== expectedLength) {
    throw new Error(`${name} must be ${expectedLength} bytes`);
  }
  return buffer;
}

function parseHexPrivateKey(value, name) {
  if (typeof value !== "string") {
    throw new TypeError(`${name} must be a hex string`);
  }
  const cleaned = value.trim().replace(/^0x/i, "");
  if (cleaned.length !== 64 && cleaned.length !== 128) {
    throw new Error(`${name} must be 64 or 128 hex characters`);
  }
  const buffer = Buffer.from(cleaned, "hex");
  if (buffer.length !== 32 && buffer.length !== 64) {
    throw new Error(`${name} must be 32 or 64 bytes`);
  }
  return buffer;
}

function encodeBlobClass(input) {
  if (typeof input === "object" && input !== null && input.class) {
    const variant = String(input.class).trim();
    if (variant === "Custom") {
      return encodeTaggedEnum(
        "class",
        "Custom",
        normalizeUnsignedInteger(input.value, "blobClass.value", { allowZero: true }),
      );
    }
    return encodeTaggedEnum("class", variant, null);
  }
  const normalized = normalizeEnumLiteral(
    input,
    "blobClass",
    ["TaikaiSegment", "NexusLaneSidecar", "GovernanceArtifact", "Custom"],
  );
  if (normalized === "Custom") {
    throw new TypeError("blobClass.Custom requires a { class: 'Custom', value: number } object");
  }
  return encodeTaggedEnum("class", normalized, null);
}

function encodeErasureProfile(profile = {}) {
  const merged = {
    ...DEFAULT_ERASURE_PROFILE,
    ...(profile ?? {}),
  };
  return {
    data_shards: normalizeUnsignedInteger(merged.dataShards, "erasureProfile.dataShards", { allowZero: false }),
    parity_shards: normalizeUnsignedInteger(merged.parityShards, "erasureProfile.parityShards", { allowZero: false }),
    row_parity_stripes: normalizeUnsignedInteger(
      merged.rowParityStripes,
      "erasureProfile.rowParityStripes",
    ),
    chunk_alignment: normalizeUnsignedInteger(merged.chunkAlignment, "erasureProfile.chunkAlignment", { allowZero: false }),
    fec_scheme: encodeFecScheme(merged.fecScheme),
  };
}

function encodeFecScheme(input) {
  if (typeof input === "object" && input !== null && input.scheme) {
    const name = normalizeEnumLiteral(
      input.scheme,
      "erasureProfile.fecScheme",
      ["Rs12_10", "RsWin14_10", "Rs18_14", "Custom"],
    );
    if (name === "Custom") {
      return encodeTaggedEnum(
        "scheme",
        name,
        normalizeUnsignedInteger(input.value, "erasureProfile.fecScheme.value", { allowZero: false }),
      );
    }
    return encodeTaggedEnum("scheme", name, null);
  }
  const normalized = normalizeEnumLiteral(
    input ?? DEFAULT_ERASURE_PROFILE.fecScheme,
    "erasureProfile.fecScheme",
    ["Rs12_10", "RsWin14_10", "Rs18_14", "Custom"],
  );
  if (normalized === "Custom") {
    throw new TypeError("Custom fecScheme requires an object with { scheme: 'Custom', value }");
  }
  return encodeTaggedEnum("scheme", normalized, null);
}

function encodeRetentionPolicy(policy = {}) {
  const merged = {
    ...DEFAULT_RETENTION_POLICY,
    ...(policy ?? {}),
  };
  if (typeof merged.governanceTag !== "string") {
    throw new TypeError("retentionPolicy.governanceTag must be a string");
  }
  return {
    hot_retention_secs: normalizeSorafsPinU64(merged.hotRetentionSecs, "retentionPolicy.hotRetentionSecs"),
    cold_retention_secs: normalizeSorafsPinU64(merged.coldRetentionSecs, "retentionPolicy.coldRetentionSecs"),
    required_replicas: normalizeSorafsPinU64(merged.requiredReplicas, "retentionPolicy.requiredReplicas", 0xffffn),
    storage_class: encodeStorageClass(merged.storageClass),
    governance_tag: tupleWrap(merged.governanceTag),
  };
}

function encodeStorageClass(input) {
  const normalized = normalizeEnumLiteral(
    input ?? DEFAULT_RETENTION_POLICY.storageClass,
    "retentionPolicy.storageClass",
    ["Hot", "Warm", "Cold"],
  );
  return encodeTaggedEnum("type", normalized, null);
}

function encodeMetadata(metadata) {
  if (metadata === undefined || metadata === null) {
    return { items: [] };
  }
  if (Array.isArray(metadata)) {
    return { items: metadata.map((entry, index) => normalizeMetadataEntry(entry, `metadata[${index}]`)) };
  }
  if (typeof metadata === "object") {
    return {
      items: Object.entries(metadata).map(([key, value]) => normalizeMetadataEntry({ key, value }, `metadata.${key}`)),
    };
  }
  throw new TypeError("metadata must be an object or array of entries");
}

function normalizeMetadataEntry(entry, context) {
  const key = requireNonEmptyString(entry.key ?? entry.name ?? entry.label ?? "", `${context}.key`);
  const value = toBuffer(entry.value ?? entry, `${context}.value`).toString("base64");
  const visibilityVariant = normalizeEnumLiteral(
    entry.visibility ?? "Public",
    `${context}.visibility`,
    ["Public", "GovernanceOnly"],
  );
  const encoded = {
    key,
    value,
    visibility: encodeTaggedEnum("visibility", visibilityVariant, null),
    encryption: encodeMetadataEncryption(entry.encryption, context),
  };
  return encoded;
}

function encodeMetadataEncryption(encryption, context) {
  if (!encryption) {
    return { cipher: "None", params: null };
  }
  const cipher = normalizeEnumLiteral(
    encryption.cipher ?? encryption.type,
    `${context}.encryption.cipher`,
    ["None", "ChaCha20Poly1305"],
  );
  if (cipher === "None") {
    return { cipher: "None", params: null };
  }
  if (cipher !== "ChaCha20Poly1305") {
    throw new TypeError(`${context}.encryption.cipher must be 'None' or 'ChaCha20Poly1305'`);
  }
  const keyLabel = encryption.keyLabel ?? encryption.key_label ?? null;
  const envelope = keyLabel === null || keyLabel === undefined ? null : { key_label: String(keyLabel) };
  return { cipher: "ChaCha20Poly1305", params: envelope };
}

function encodeTaggedEnum(tag, variant, value) {
  return {
    [tag]: requireNonEmptyString(variant, `${tag}`),
    value: value ?? null,
  };
}

function resolveDaNativeBinding(nativeRuntime, methodName) {
  const binding = resolveNativeRuntimeBinding(nativeRuntime);
  if (!binding || typeof binding[methodName] !== "function") {
    throw new Error(
      `DA helpers require the native iroha_js_host module (${methodName}). Run \`npm run build:native\` before using this helper.`,
    );
  }
  return binding;
}

function normalizeProofOptions(options = {}) {
  const record =
    options === undefined || options === null
      ? {}
      : ensureRecord(options, "generateDaProofSummary options");
  const native = {};
  const sampleCountValue = record.sampleCount ?? record.sample_count;
  if (sampleCountValue !== undefined) {
    native.sample_count = normalizeProofInteger(
      sampleCountValue,
      "generateDaProofSummary.sampleCount",
      { allowZero: true },
    );
  }
  const sampleSeedValue = record.sampleSeed ?? record.sample_seed;
  if (sampleSeedValue !== undefined) {
    native.sample_seed = normalizeProofInteger(
      sampleSeedValue,
      "generateDaProofSummary.sampleSeed",
      { allowZero: true },
    );
  }
  const leafIndexes = record.leafIndexes ?? record.leaf_indexes;
  if (leafIndexes !== undefined) {
    if (!Array.isArray(leafIndexes)) {
      throw new TypeError("generateDaProofSummary.leafIndexes must be an array");
    }
    native.leaf_indexes = leafIndexes.map((value, index) =>
      normalizeProofInteger(
        value,
        `generateDaProofSummary.leafIndexes[${index}]`,
        { allowZero: true },
      ),
    );
  }
  return Object.keys(native).length > 0 ? native : undefined;
}

function normalizeProofInteger(value, name, { allowZero = true } = {}) {
  if (typeof value === "bigint") {
    if (value < 0n || (!allowZero && value === 0n)) {
      throw new RangeError(`${name} must be ${allowZero ? "non-negative" : "greater than zero"}`);
    }
    if (value > MAX_SAFE_UINT_BIGINT) {
      throw new RangeError(`${name} exceeds Number.MAX_SAFE_INTEGER`);
    }
    return Number(value);
  }
  return normalizeUnsignedInteger(value, name, { allowZero });
}

function encodeFixedBytes(buffer, expectedLength) {
  if (buffer.length !== expectedLength) {
    throw new Error(`digest must be ${expectedLength} bytes`);
  }
  return Array.from(buffer.values());
}

function tupleWrap(value) {
  return [value];
}

function toBuffer(value, name) {
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (typeof ArrayBuffer !== "undefined" && value instanceof ArrayBuffer) {
    return Buffer.from(value);
  }
  if (ArrayBuffer.isView(value)) {
    return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  }
  if (Array.isArray(value)) {
    return normalizeByteArray(value, name);
  }
  if (value && typeof value.length === "number" && typeof value !== "string") {
    return normalizeByteArray(value, name);
  }
  if (typeof value === "string") {
    return Buffer.from(value, "utf8");
  }
  throw new TypeError(
    `${name} must be a Buffer, ArrayBuffer, typed array, byte array, or string`,
  );
}

function normalizeByteArray(value, name) {
  const bytes = Array.from(value);
  const normalized = bytes.map((entry, index) => {
    if (!Number.isInteger(entry) || entry < 0 || entry > 0xff) {
      throw new TypeError(`${name}[${index}] must be a byte`);
    }
    return entry;
  });
  return Buffer.from(normalized);
}

function normalizeUnsignedInteger(value, name, { allowZero = true } = {}) {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new TypeError(`${name} must be a finite number`);
  }
  if (!Number.isInteger(value)) {
    throw new TypeError(`${name} must be an integer`);
  }
  if (!allowZero && value === 0) {
    throw new RangeError(`${name} must be greater than zero`);
  }
  if (value < 0) {
    throw new RangeError(`${name} must be non-negative`);
  }
  if (value > MAX_SAFE_UINT) {
    throw new RangeError(`${name} exceeds Number.MAX_SAFE_INTEGER`);
  }
  return value;
}

function ensureRecord(value, context) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return value;
}

function assertSupportedOptionKeys(record, allowedKeys, context) {
  const extras = Object.keys(record).filter((key) => !allowedKeys.has(key));
  if (extras.length > 0) {
    throw new TypeError(`${context} contains unsupported fields: ${extras.join(", ")}`);
  }
}

function toSafeIntegerLike(value) {
  if (typeof value === "bigint") {
    return value <= MAX_SAFE_UINT_BIGINT ? Number(value) : value;
  }
  if (typeof value === "number") {
    return value;
  }
  throw new TypeError("native proof summary values must be numbers or bigint");
}

function transformDaProofSummary(raw) {
  if (!raw || typeof raw !== "object") {
    throw new TypeError("native proof summary payload must be an object");
  }
  return {
    blob_hash_hex: raw.blob_hash_hex,
    chunk_root_hex: raw.chunk_root_hex,
    por_root_hex: raw.por_root_hex,
    leaf_count: toSafeIntegerLike(raw.leaf_count),
    segment_count: toSafeIntegerLike(raw.segment_count),
    chunk_count: toSafeIntegerLike(raw.chunk_count),
    sample_count: raw.sample_count,
    sample_seed: toSafeIntegerLike(raw.sample_seed),
    proof_count: raw.proof_count,
    proofs: Array.isArray(raw.proofs)
      ? raw.proofs.map(transformDaProofRecord)
      : [],
  };
}

function transformDaProofRecord(raw) {
  if (!raw || typeof raw !== "object") {
    throw new TypeError("native proof record payload must be an object");
  }
  return {
    origin: raw.origin,
    leaf_index: raw.leaf_index,
    chunk_index: raw.chunk_index,
    segment_index: raw.segment_index,
    leaf_offset: toSafeIntegerLike(raw.leaf_offset),
    leaf_length: raw.leaf_length,
    segment_offset: toSafeIntegerLike(raw.segment_offset),
    segment_length: raw.segment_length,
    chunk_offset: toSafeIntegerLike(raw.chunk_offset),
    chunk_length: raw.chunk_length,
    payload_len: toSafeIntegerLike(raw.payload_len),
    chunk_digest_hex: raw.chunk_digest_hex,
    chunk_root_hex: raw.chunk_root_hex,
    segment_digest_hex: raw.segment_digest_hex,
    leaf_digest_hex: raw.leaf_digest_hex,
    leaf_bytes_b64: raw.leaf_bytes_b64,
    segment_leaves_hex: Array.isArray(raw.segment_leaves_hex)
      ? raw.segment_leaves_hex.slice()
      : [],
    chunk_segments_hex: Array.isArray(raw.chunk_segments_hex)
      ? raw.chunk_segments_hex.slice()
      : [],
    chunk_count: toSafeIntegerLike(raw.chunk_count),
    chunk_merkle_path_hex: Array.isArray(raw.chunk_merkle_path_hex)
      ? raw.chunk_merkle_path_hex.slice()
      : [],
    verified: Boolean(raw.verified),
  };
}

function requireNonEmptyString(value, name) {
  if (typeof value !== "string") {
    throw new TypeError(`${name} must be a string`);
  }
  const trimmed = value.trim();
  if (!trimmed) {
    throw new TypeError(`${name} must be a non-empty string`);
  }
  return trimmed;
}

function bufferToHex(buffer) {
  return Buffer.from(buffer).toString("hex").toUpperCase();
}

function canonicalizeHex(value, name) {
  const trimmed = requireNonEmptyString(value, name).replace(/^0x/i, "");
  if (trimmed.length === 0 || trimmed.length % 2 !== 0) {
    throw new TypeError(`${name} must be an even-length hex string`);
  }
  if (!/^([0-9A-Fa-f]{2})+$/.test(trimmed)) {
    throw new TypeError(`${name} must be a hex string`);
  }
  return trimmed.toUpperCase();
}

function normalizeEnumLiteral(value, name, allowed) {
  const literal = requireNonEmptyString(String(value ?? ""), name);
  if (!allowed || allowed.length === 0) {
    return literal.charAt(0).toUpperCase() + literal.slice(1);
  }
  const match = allowed.find((variant) => variant.toLowerCase() === literal.toLowerCase());
  if (!match) {
    throw new TypeError(`${name} must be one of ${allowed.join(", ")}`);
  }
  return match;
}

function buildDaProofRecord(proofInput, index) {
  const proof = ensureRecord(proofInput, `daProofSummary.proofs[${index}]`);
  return {
    origin: requireNonEmptyString(
      proof.origin ?? "",
      `daProofSummary.proofs[${index}].origin`,
    ),
    leaf_index: toJsonInteger(
      proof.leaf_index,
      `daProofSummary.proofs[${index}].leaf_index`,
    ),
    chunk_index: toJsonInteger(
      proof.chunk_index,
      `daProofSummary.proofs[${index}].chunk_index`,
    ),
    segment_index: toJsonInteger(
      proof.segment_index,
      `daProofSummary.proofs[${index}].segment_index`,
    ),
    leaf_offset: toJsonInteger(
      proof.leaf_offset,
      `daProofSummary.proofs[${index}].leaf_offset`,
    ),
    leaf_length: toJsonInteger(
      proof.leaf_length,
      `daProofSummary.proofs[${index}].leaf_length`,
    ),
    segment_offset: toJsonInteger(
      proof.segment_offset,
      `daProofSummary.proofs[${index}].segment_offset`,
    ),
    segment_length: toJsonInteger(
      proof.segment_length,
      `daProofSummary.proofs[${index}].segment_length`,
    ),
    chunk_offset: toJsonInteger(
      proof.chunk_offset,
      `daProofSummary.proofs[${index}].chunk_offset`,
    ),
    chunk_length: toJsonInteger(
      proof.chunk_length,
      `daProofSummary.proofs[${index}].chunk_length`,
    ),
    payload_len: toJsonInteger(
      proof.payload_len,
      `daProofSummary.proofs[${index}].payload_len`,
    ),
    chunk_digest: toLowerHexField(
      proof.chunk_digest_hex,
      `daProofSummary.proofs[${index}].chunk_digest_hex`,
    ),
    chunk_root: toLowerHexField(
      proof.chunk_root_hex,
      `daProofSummary.proofs[${index}].chunk_root_hex`,
    ),
    segment_digest: toLowerHexField(
      proof.segment_digest_hex,
      `daProofSummary.proofs[${index}].segment_digest_hex`,
    ),
    leaf_digest: toLowerHexField(
      proof.leaf_digest_hex,
      `daProofSummary.proofs[${index}].leaf_digest_hex`,
    ),
    leaf_bytes_b64: resolveLeafBytesB64(proof, index),
    segment_leaves: normalizeHexArray(
      proof.segment_leaves_hex,
      `daProofSummary.proofs[${index}].segment_leaves_hex`,
    ),
    chunk_segments: normalizeHexArray(
      proof.chunk_segments_hex,
      `daProofSummary.proofs[${index}].chunk_segments_hex`,
    ),
    chunk_count: toJsonInteger(
      proof.chunk_count,
      `daProofSummary.proofs[${index}].chunk_count`,
    ),
    chunk_merkle_path: normalizeHexArray(
      proof.chunk_merkle_path_hex,
      `daProofSummary.proofs[${index}].chunk_merkle_path_hex`,
    ),
    verified: Boolean(proof.verified),
  };
}

function normalizeOptionalPath(value) {
  if (value === undefined || value === null) {
    return null;
  }
  const stringValue = String(value).trim();
  return stringValue === "" ? null : stringValue;
}

function toLowerHexField(value, name) {
  const normalized = canonicalizeHex(value, name);
  return normalized.toLowerCase();
}

function toJsonInteger(value, name) {
  if (typeof value === "bigint") {
    if (value < 0n) {
      throw new RangeError(`${name} must be non-negative`);
    }
    return value <= MAX_SAFE_UINT_BIGINT ? Number(value) : value.toString();
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new TypeError(`${name} must be a finite number`);
    }
    if (!Number.isInteger(value)) {
      throw new TypeError(`${name} must be an integer`);
    }
    if (value < 0) {
      throw new RangeError(`${name} must be non-negative`);
    }
    return Number.isSafeInteger(value) ? value : value.toString();
  }
  throw new TypeError(`${name} must be a number or bigint`);
}

function resolveLeafBytesB64(proof, index) {
  if (typeof proof.leaf_bytes_b64 === "string" && proof.leaf_bytes_b64.trim() !== "") {
    return proof.leaf_bytes_b64;
  }
  throw new TypeError(`daProofSummary.proofs[${index}].leaf_bytes_b64 is required`);
}

function normalizeHexArray(values, context) {
  if (values === undefined || values === null) {
    return [];
  }
  if (!Array.isArray(values)) {
    throw new TypeError(`${context} must be an array`);
  }
  return values.map((value, index) => toLowerHexField(value, `${context}[${index}]`));
}

function resolveJsonSpacing(option) {
  if (option === undefined || option === null) {
    return 2;
  }
  if (option === false) {
    return undefined;
  }
  if (option === true) {
    return 2;
  }
  if (typeof option === "number") {
    if (!Number.isFinite(option) || option < 0) {
      throw new TypeError("pretty must be a non-negative number");
    }
    return Math.min(10, Math.trunc(option));
  }
  throw new TypeError("pretty must be a non-negative number, true, or false");
}
