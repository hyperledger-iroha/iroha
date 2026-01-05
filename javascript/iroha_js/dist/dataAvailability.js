import fs from "node:fs/promises";
import path from "node:path";

import { canonicalizeMultihashHex } from "./normalizers.js";
import { publicKeyFromPrivate, signEd25519 } from "./crypto.js";
import { getNativeBinding } from "./native.js";

const DEFAULT_CHUNK_SIZE = 262_144;
const DEFAULT_ERASURE_PROFILE = {
  dataShards: 10,
  parityShards: 4,
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
const ED25519_FUNCTION_CODE = 0xed;
const MAX_SAFE_UINT = Number.MAX_SAFE_INTEGER;
const MAX_SAFE_UINT_BIGINT = BigInt(MAX_SAFE_UINT);

export function buildDaIngestRequest(options = {}) {
  const payloadBuffer = toBuffer(options.payload, "payload");
  if (payloadBuffer.length === 0) {
    throw new Error("payload must contain at least one byte");
  }

  const chunkSize = normalizeUnsignedInteger(
    options.chunkSize ?? DEFAULT_CHUNK_SIZE,
    "chunkSize",
    { allowZero: false },
  );
  const laneId = normalizeUnsignedInteger(options.laneId ?? 0, "laneId");
  const epoch = normalizeUnsignedInteger(options.epoch ?? 0, "epoch");
  const sequence = normalizeUnsignedInteger(options.sequence ?? 0, "sequence");

  const codec = tupleWrap(
    requireNonEmptyString(options.codec ?? "application/octet-stream", "codec"),
  );
  const blobClass = encodeBlobClass(options.blobClass ?? "TaikaiSegment");
  const erasureProfile = encodeErasureProfile(options.erasureProfile);
  const retentionPolicy = encodeRetentionPolicy(options.retentionPolicy);
  const metadata = encodeMetadata(options.metadata);

  const { digestTuple, digestHex } = resolveClientBlobId(options.clientBlobId, payloadBuffer);
  const signatureInfo = resolveSignature(options, payloadBuffer);

  const request = {
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
    payload: payloadBuffer.toString("base64"),
    metadata,
    submitter: signatureInfo.submitterPublicKey,
    signature: signatureInfo.signatureHex,
  };

  if (options.compression !== undefined) {
    request.compression = normalizeEnumLiteral(
      options.compression,
      "compression",
      ["Identity", "Gzip", "Deflate", "Zstd"],
    );
  }

  if (options.noritoManifest !== undefined && options.noritoManifest !== null) {
    request.norito_manifest = toBuffer(options.noritoManifest, "noritoManifest").toString("base64");
  }

  return {
    request,
    artifacts: {
      clientBlobIdHex: digestHex,
      submitterPublicKey: signatureInfo.submitterPublicKey,
      signatureHex: signatureInfo.signatureHex,
      payloadLength: payloadBuffer.length,
    },
  };
}

export function deriveDaChunkerHandle(manifestBytes, options = {}) {
  const record =
    options === undefined || options === null
      ? {}
      : ensureRecord(options, "deriveDaChunkerHandle options");
  assertSupportedOptionKeys(
    record,
    new Set(["__nativeBinding"]),
    "deriveDaChunkerHandle options",
  );
  const binding = resolveDaNativeBinding(record.__nativeBinding, "daManifestChunkerHandle");
  const buffer = toBuffer(manifestBytes, "deriveDaChunkerHandle.manifestBytes");
  return binding.daManifestChunkerHandle(buffer);
}

export function generateDaProofSummary(
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
      "__nativeBinding",
      "sampleCount",
      "sampleSeed",
      "leafIndexes",
      "sample_count",
      "sample_seed",
      "leaf_indexes",
    ]),
    "generateDaProofSummary options",
  );
  const { __nativeBinding: injectedBinding, ...rest } = optionsRecord;
  const binding = resolveDaNativeBinding(injectedBinding, "daGenerateProofs");
  const nativeOptions = normalizeProofOptions(rest);
  const rawSummary = binding.daGenerateProofs(
    manifestBytes,
    payloadBytes,
    nativeOptions,
  );
  return transformDaProofSummary(rawSummary);
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
  const binding = getNativeBinding();
  if (!binding || typeof binding.blake3Hash !== "function") {
    throw new Error(
      "blake3 hashing requires the native iroha_js_host binding. Run `npm run build:native` before calling submitDaBlob().",
    );
  }
  const digestBuffer = Buffer.from(binding.blake3Hash(payloadBuffer));
  if (digestBuffer.length !== 32) {
    throw new Error("native blake3Hash returned an unexpected digest length");
  }
  return {
    digestTuple: tupleWrap(encodeFixedBytes(digestBuffer, 32)),
    digestHex: bufferToHex(digestBuffer),
  };
}

function resolveSignature(options, payloadBuffer) {
  const submitter = options.submitterPublicKey
    ? canonicalizePublicKey(options.submitterPublicKey, "submitterPublicKey")
    : null;

  if (options.signatureHex) {
    const signatureHex = canonicalizeHex(options.signatureHex, "signatureHex");
    if (!submitter && !options.privateKey && !options.privateKeyHex) {
      throw new TypeError("submitterPublicKey or privateKey is required when providing signatureHex");
    }
    return {
      signatureHex,
      submitterPublicKey:
        submitter ?? encodeEd25519Multihash(publicKeyFromPrivate(normalizePrivateKey(options))),
    };
  }

  const privateKey = normalizePrivateKey(options);
  const signature = signEd25519(payloadBuffer, privateKey);
  const signatureHex = bufferToHex(signature);
  const submitterPublicKey =
    submitter ?? encodeEd25519Multihash(publicKeyFromPrivate(privateKey));
  return { signatureHex, submitterPublicKey };
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
  return {
    hot_retention_secs: normalizeUnsignedInteger(merged.hotRetentionSecs, "retentionPolicy.hotRetentionSecs", { allowZero: false }),
    cold_retention_secs: normalizeUnsignedInteger(merged.coldRetentionSecs, "retentionPolicy.coldRetentionSecs", { allowZero: false }),
    required_replicas: normalizeUnsignedInteger(merged.requiredReplicas, "retentionPolicy.requiredReplicas", { allowZero: false }),
    storage_class: encodeStorageClass(merged.storageClass),
    governance_tag: tupleWrap(requireNonEmptyString(merged.governanceTag, "retentionPolicy.governanceTag")),
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

function resolveDaNativeBinding(bindingOverride, methodName) {
  const binding = bindingOverride ?? getNativeBinding();
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
  if (typeof value === "string") {
    return Buffer.from(value, "utf8");
  }
  throw new TypeError(`${name} must be a Buffer, ArrayBuffer, typed array, or string`);
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
    chunk_roots_hex: Array.isArray(raw.chunk_roots_hex)
      ? raw.chunk_roots_hex.slice()
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

function canonicalizePublicKey(value, name) {
  const trimmed = requireNonEmptyString(value, name);
  if (trimmed.includes(":")) {
    const [, body] = trimmed.split(":", 2);
    return canonicalizeMultihashHex(body, name);
  }
  return canonicalizeMultihashHex(trimmed, name);
}

function encodeEd25519Multihash(publicKey) {
  if (publicKey.length !== 32) {
    throw new Error("publicKeyFromPrivate returned an unexpected length");
  }
  const bytes = [...encodeVarint(ED25519_FUNCTION_CODE), ...encodeVarint(publicKey.length), ...publicKey];
  return bufferToHex(Buffer.from(bytes));
}

function encodeVarint(value) {
  const bytes = [];
  let remaining = value >>> 0;
  while (remaining >= 0x80) {
    bytes.push((remaining & 0x7f) | 0x80);
    remaining >>>= 7;
  }
  bytes.push(remaining);
  return bytes;
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
    chunk_roots: normalizeHexArray(
      proof.chunk_roots_hex,
      `daProofSummary.proofs[${index}].chunk_roots_hex`,
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
