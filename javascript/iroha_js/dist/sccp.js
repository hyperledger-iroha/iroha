import { blake2b } from "@noble/hashes/blake2b";
import { keccak_256 } from "@noble/hashes/sha3";

export const SCCP_DOMAIN_SORA = 0;
export const SCCP_DOMAIN_ETH = 1;
export const SCCP_DOMAIN_BSC = 2;
export const SCCP_DOMAIN_SOL = 3;
export const SCCP_DOMAIN_TON = 4;
export const SCCP_DOMAIN_TRON = 5;
export const SCCP_DOMAIN_SORA_KUSAMA = 6;
export const SCCP_DOMAIN_SORA_POLKADOT = 7;
export const SCCP_DOMAIN_SORA2 = 8;
export const SCCP_STARK_FRI_PROOF_FAMILY_V1 = "stark-fri-v1";
export const SCCP_SOLANA_RECURSIVE_PROOF_BACKEND_V1 = "sccp-solana-recursive-mainnet-v1";
export const SCCP_SOLANA_MAINNET_GENESIS_HASH = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp";
export const SCCP_TON_CONTRACT_PROOF_BACKEND_V1 = "ton-contract-v1";
export const SCCP_TON_MESSAGE_BODY_BOC_V1 = "ton_message_body_boc_v1";

export const SCCP_CORE_REMOTE_DOMAINS = [
  SCCP_DOMAIN_ETH,
  SCCP_DOMAIN_BSC,
  SCCP_DOMAIN_SOL,
  SCCP_DOMAIN_TON,
  SCCP_DOMAIN_TRON,
  SCCP_DOMAIN_SORA_KUSAMA,
  SCCP_DOMAIN_SORA_POLKADOT,
  SCCP_DOMAIN_SORA2,
];

const SCCP_MSG_PREFIX_BURN_V1 = "sccp:burn:v1";
const SCCP_MSG_PREFIX_TOKEN_ADD_V1 = "sccp:token:add:v1";
const SCCP_MSG_PREFIX_TOKEN_PAUSE_V1 = "sccp:token:pause:v1";
const SCCP_MSG_PREFIX_TOKEN_RESUME_V1 = "sccp:token:resume:v1";
const SCCP_HUB_LEAF_PREFIX_V1 = "sccp:hub:leaf:v1";
const SCCP_HUB_NODE_PREFIX_V1 = "sccp:hub:node:v1";
const SCCP_PAYLOAD_HASH_PREFIX_V1 = "sccp:payload:v1";
const SCCP_TON_BOC_MAGIC = Uint8Array.from([0xb5, 0xee, 0x9c, 0x72]);
const SCCP_TON_SUBMIT_OP_V1 = 0x53434350;
const SCCP_TON_MESSAGE_SCHEMA_VERSION_V1 = 1;
const SCCP_TON_MAX_CELL_DATA_BYTES = 127;
const SCCP_TON_MAX_REFS = 4;

const textEncoder = new TextEncoder();

const normalizeHexInput = (value, label, byteLength = null) => {
  if (typeof value !== "string") {
    throw new TypeError(`${label} must be a hex string`);
  }
  const trimmed = value.trim().replace(/^0x/i, "").toLowerCase();
  if (!trimmed || /[^0-9a-f]/.test(trimmed) || trimmed.length % 2 !== 0) {
    throw new TypeError(`${label} must be canonical hex`);
  }
  if (byteLength !== null && trimmed.length !== byteLength * 2) {
    throw new TypeError(`${label} must be ${byteLength} bytes`);
  }
  return trimmed;
};

const hexToBytes = (value, label, byteLength = null) => {
  const normalized = normalizeHexInput(value, label, byteLength);
  const out = new Uint8Array(normalized.length / 2);
  for (let index = 0; index < normalized.length; index += 2) {
    out[index / 2] = Number.parseInt(normalized.slice(index, index + 2), 16);
  }
  return out;
};

const bytesToHex = (bytes, withPrefix = true) => {
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  return withPrefix ? `0x${hex}` : hex;
};

const concatBytes = (...parts) => {
  const total = parts.reduce((sum, part) => sum + part.length, 0);
  const out = new Uint8Array(total);
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
};

const writeU8 = (target, value) => {
  const out = new Uint8Array(1);
  out[0] = value;
  return concatBytes(target, out);
};

const writeU32Le = (target, value) => {
  const out = new Uint8Array(4);
  new DataView(out.buffer).setUint32(0, Number(value), true);
  return concatBytes(target, out);
};

const writeU64Le = (target, value) => {
  const out = new Uint8Array(8);
  const view = new DataView(out.buffer);
  view.setBigUint64(0, normalizeUnsignedBigInt(value, "u64"), true);
  return concatBytes(target, out);
};

const writeU16Be = (target, value) => {
  const out = new Uint8Array(2);
  new DataView(out.buffer).setUint16(0, Number(value), false);
  return concatBytes(target, out);
};

const writeU32Be = (target, value) => {
  const out = new Uint8Array(4);
  new DataView(out.buffer).setUint32(0, Number(value), false);
  return concatBytes(target, out);
};

const writeU64Be = (target, value) => {
  const out = new Uint8Array(8);
  new DataView(out.buffer).setBigUint64(0, normalizeUnsignedBigInt(value, "u64"), false);
  return concatBytes(target, out);
};

const writeU128Le = (target, value) => {
  const numeric = normalizeUnsignedBigInt(value, "u128");
  const out = new Uint8Array(16);
  let working = numeric;
  for (let index = 0; index < 16; index += 1) {
    out[index] = Number(working & 0xffn);
    working >>= 8n;
  }
  return concatBytes(target, out);
};

const writeBytes = (target, value) => {
  const bytes = toBytes(value, "bytes");
  return concatBytes(writeU32Le(target, bytes.length), bytes);
};

const writeString = (target, value, label) => {
  if (typeof value !== "string" || value.trim() === "") {
    throw new TypeError(`${label} must be a non-empty string`);
  }
  return writeBytes(target, textEncoder.encode(value.trim()));
};

const normalizeUnsignedBigInt = (value, label) => {
  if (typeof value === "bigint") {
    if (value < 0n) throw new RangeError(`${label} must not be negative`);
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isInteger(value) || value < 0 || !Number.isSafeInteger(value)) {
      throw new RangeError(`${label} must be a non-negative safe integer`);
    }
    return BigInt(value);
  }
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (!/^\d+$/.test(trimmed)) {
      throw new TypeError(`${label} must be an unsigned integer`);
    }
    return BigInt(trimmed);
  }
  throw new TypeError(`${label} must be an unsigned integer`);
};

const prefixedKeccak = (prefix, payload) => keccak_256(concatBytes(textEncoder.encode(prefix), payload));

const prefixedBlake2b = (prefix, payload) =>
  blake2b(concatBytes(textEncoder.encode(prefix), payload), { dkLen: 32 });

const normalizeHex32 = (value, label) => bytesToHex(hexToBytes(value, label, 32));

const normalizeNonEmptyString = (value, label) => {
  if (typeof value !== "string" || value.trim() === "") {
    throw new TypeError(`${label} must be a non-empty string`);
  }
  return value.trim();
};

const bytesToBase64 = (bytes) => {
  const bufferCtor = globalThis.Buffer;
  if (typeof bufferCtor !== "undefined") {
    return bufferCtor.from(bytes).toString("base64");
  }
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
};

const normalizeTokenMessagePayload = (payload) => {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) {
    throw new TypeError("token message payload must be an object");
  }
  if (typeof payload.kind === "string" && payload.value && typeof payload.value === "object") {
    return {
      kind: payload.kind,
      value: payload.value,
    };
  }
  if ("TokenAdd" in payload) {
    return { kind: "TokenAdd", value: payload.TokenAdd };
  }
  if ("TokenPause" in payload) {
    return { kind: "TokenPause", value: payload.TokenPause };
  }
  if ("TokenResume" in payload) {
    return { kind: "TokenResume", value: payload.TokenResume };
  }
  throw new TypeError("token message payload must be TokenAdd, TokenPause, or TokenResume");
};

const normalizeGovernanceMessagePayload = (payload) => {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) {
    throw new TypeError("governance payload must be an object");
  }
  if ("Add" in payload) {
    return { kind: "TokenAdd", value: payload.Add };
  }
  if ("Pause" in payload) {
    return { kind: "TokenPause", value: payload.Pause };
  }
  if ("Resume" in payload) {
    return { kind: "TokenResume", value: payload.Resume };
  }
  return normalizeTokenMessagePayload(payload);
};

const messageKindCode = (kind) => {
  switch (kind) {
    case "Burn":
      return 0;
    case "TokenAdd":
      return 1;
    case "TokenPause":
      return 2;
    case "TokenResume":
      return 3;
    case "AssetRegister":
      return 4;
    case "RouteActivate":
      return 5;
    case "Transfer":
      return 6;
    default:
      throw new TypeError(`unsupported SCCP message kind: ${kind}`);
  }
};

export const isSupportedSccpDomain = (domainId) =>
  [
    SCCP_DOMAIN_SORA,
    SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_SOL,
    SCCP_DOMAIN_TON,
    SCCP_DOMAIN_TRON,
    SCCP_DOMAIN_SORA_KUSAMA,
    SCCP_DOMAIN_SORA_POLKADOT,
    SCCP_DOMAIN_SORA2,
  ].includes(Number(domainId));

export const canonicalSccpBurnPayloadBytes = (payload) => {
  if (!payload || typeof payload !== "object") {
    throw new TypeError("payload must be an object");
  }
  let out = new Uint8Array();
  out = writeU8(out, Number(payload.version));
  out = writeU32Le(out, Number(payload.source_domain));
  out = writeU32Le(out, Number(payload.dest_domain));
  out = writeU64Le(out, payload.nonce);
  out = concatBytes(out, hexToBytes(payload.sora_asset_id, "payload.sora_asset_id", 32));
  out = writeU128Le(out, payload.amount);
  out = concatBytes(out, hexToBytes(payload.recipient, "payload.recipient", 32));
  return out;
};

export const canonicalSccpTokenAddPayloadBytes = (payload) => {
  if (!payload || typeof payload !== "object") {
    throw new TypeError("payload must be an object");
  }
  let out = new Uint8Array();
  out = writeU8(out, Number(payload.version));
  out = writeU32Le(out, Number(payload.target_domain));
  out = writeU64Le(out, payload.nonce);
  out = concatBytes(out, hexToBytes(payload.sora_asset_id, "payload.sora_asset_id", 32));
  out = writeU8(out, Number(payload.decimals));
  out = concatBytes(out, hexToBytes(payload.name, "payload.name", 32));
  out = concatBytes(out, hexToBytes(payload.symbol, "payload.symbol", 32));
  return out;
};

export const canonicalSccpTokenControlPayloadBytes = (payload) => {
  if (!payload || typeof payload !== "object") {
    throw new TypeError("payload must be an object");
  }
  let out = new Uint8Array();
  out = writeU8(out, Number(payload.version));
  out = writeU32Le(out, Number(payload.target_domain));
  out = writeU64Le(out, payload.nonce);
  out = concatBytes(out, hexToBytes(payload.sora_asset_id, "payload.sora_asset_id", 32));
  return out;
};

export const canonicalSccpTokenMessagePayloadBytes = (payload) => {
  const normalized = normalizeTokenMessagePayload(payload);
  if (normalized.kind === "TokenAdd") {
    return concatBytes(
      Uint8Array.from([3]),
      canonicalSccpTokenAddPayloadBytes(normalized.value),
    );
  }
  if (normalized.kind === "TokenPause") {
    return concatBytes(
      Uint8Array.from([4]),
      canonicalSccpTokenControlPayloadBytes(normalized.value),
    );
  }
  if (normalized.kind === "TokenResume") {
    return concatBytes(
      Uint8Array.from([5]),
      canonicalSccpTokenControlPayloadBytes(normalized.value),
    );
  }
  throw new TypeError(`unsupported token message payload kind: ${normalized.kind}`);
};

export const sccpBurnMessageId = (payload, options = {}) =>
  bytesToHex(prefixedKeccak(SCCP_MSG_PREFIX_BURN_V1, canonicalSccpBurnPayloadBytes(payload)), options.prefix !== false);

export const sccpTokenAddMessageId = (payload, options = {}) =>
  bytesToHex(
    prefixedKeccak(SCCP_MSG_PREFIX_TOKEN_ADD_V1, canonicalSccpTokenAddPayloadBytes(payload)),
    options.prefix !== false,
  );

export const sccpTokenPauseMessageId = (payload, options = {}) =>
  bytesToHex(
    prefixedKeccak(SCCP_MSG_PREFIX_TOKEN_PAUSE_V1, canonicalSccpTokenControlPayloadBytes(payload)),
    options.prefix !== false,
  );

export const sccpTokenResumeMessageId = (payload, options = {}) =>
  bytesToHex(
    prefixedKeccak(SCCP_MSG_PREFIX_TOKEN_RESUME_V1, canonicalSccpTokenControlPayloadBytes(payload)),
    options.prefix !== false,
  );

export const sccpTokenMessageId = (payload, options = {}) => {
  const normalized = normalizeTokenMessagePayload(payload);
  if (normalized.kind === "TokenAdd") return sccpTokenAddMessageId(normalized.value, options);
  if (normalized.kind === "TokenPause") return sccpTokenPauseMessageId(normalized.value, options);
  if (normalized.kind === "TokenResume") return sccpTokenResumeMessageId(normalized.value, options);
  throw new TypeError(`unsupported token message payload kind: ${normalized.kind}`);
};

export const sccpTokenMessageTargetDomain = (payload) => {
  const normalized = normalizeTokenMessagePayload(payload);
  return Number(normalized.value.target_domain);
};

export const canonicalSccpGovernancePayloadBytes = (payload) =>
  canonicalSccpTokenMessagePayloadBytes(normalizeGovernanceMessagePayload(payload));

export const sccpGovernanceMessageId = (payload, options = {}) =>
  sccpTokenMessageId(normalizeGovernanceMessagePayload(payload), options);

export const sccpParliamentCertificateHash = (certificate, options = {}) =>
  sccpPayloadHash(toBytes(certificate, "parliament certificate"), options);

export const sccpPayloadHash = (payload, options = {}) =>
  bytesToHex(prefixedBlake2b(SCCP_PAYLOAD_HASH_PREFIX_V1, toBytes(payload, "payload")), options.prefix !== false);

export const canonicalSccpCommitmentBytes = (commitment) => {
  if (!commitment || typeof commitment !== "object") {
    throw new TypeError("commitment must be an object");
  }
  let out = new Uint8Array();
  out = writeU8(out, Number(commitment.version));
  out = writeU8(out, messageKindCode(commitment.kind));
  out = writeU32Le(out, Number(commitment.target_domain));
  out = concatBytes(out, hexToBytes(commitment.message_id, "commitment.message_id", 32));
  out = concatBytes(out, hexToBytes(commitment.payload_hash, "commitment.payload_hash", 32));
  return out;
};

export const sccpCommitmentLeafHash = (commitment, options = {}) =>
  bytesToHex(
    prefixedBlake2b(SCCP_HUB_LEAF_PREFIX_V1, canonicalSccpCommitmentBytes(commitment)),
    options.prefix !== false,
  );

export const sccpMerkleRootFromCommitment = (commitment, proof, options = {}) => {
  if (!proof || typeof proof !== "object" || !Array.isArray(proof.steps)) {
    throw new TypeError("proof.steps must be an array");
  }
  let current = hexToBytes(sccpCommitmentLeafHash(commitment), "commitment leaf", 32);
  for (const [index, step] of proof.steps.entries()) {
    const sibling = hexToBytes(step?.sibling_hash, `proof.steps[${index}].sibling_hash`, 32);
    current = step?.sibling_is_left
      ? prefixedBlake2b(SCCP_HUB_NODE_PREFIX_V1, concatBytes(sibling, current))
      : prefixedBlake2b(SCCP_HUB_NODE_PREFIX_V1, concatBytes(current, sibling));
  }
  return bytesToHex(current, options.prefix !== false);
};

export const validateSccpBurnBundleSurface = (bundle) => {
  const expectedMessageId = sccpBurnMessageId(bundle.payload);
  const expectedPayloadHash = sccpPayloadHash(canonicalSccpBurnPayloadBytes(bundle.payload));
  const expectedMerkleRoot = sccpMerkleRootFromCommitment(bundle.commitment, bundle.merkle_proof);
  const checks = {
    bundleVersion: Number(bundle.version) === 1,
    payloadVersion: Number(bundle.payload?.version) === 1,
    sourceDomainSupported: isSupportedSccpDomain(bundle.payload?.source_domain),
    destDomainSupported: isSupportedSccpDomain(bundle.payload?.dest_domain),
    targetDomainMatches: Number(bundle.commitment?.target_domain) === Number(bundle.payload?.dest_domain),
    burnKindMatches: bundle.commitment?.kind === "Burn",
    messageIdMatches: normalizeHexInput(bundle.commitment?.message_id, "bundle.commitment.message_id", 32) === normalizeHexInput(expectedMessageId, "expectedMessageId", 32),
    payloadHashMatches: normalizeHexInput(bundle.commitment?.payload_hash, "bundle.commitment.payload_hash", 32) === normalizeHexInput(expectedPayloadHash, "expectedPayloadHash", 32),
    merkleRootMatches: normalizeHexInput(bundle.commitment_root, "bundle.commitment_root", 32) === normalizeHexInput(expectedMerkleRoot, "expectedMerkleRoot", 32),
  };
  return {
    ok: Object.values(checks).every(Boolean),
    expectedMessageId,
    expectedPayloadHash,
    expectedMerkleRoot,
    checks,
  };
};

export const validateSccpTokenMessageBundleSurface = (bundle) => {
  const normalizedPayload = normalizeTokenMessagePayload(bundle.payload);
  const expectedMessageId = sccpTokenMessageId(normalizedPayload);
  const expectedPayloadHash = sccpPayloadHash(canonicalSccpTokenMessagePayloadBytes(normalizedPayload));
  const expectedMerkleRoot = sccpMerkleRootFromCommitment(bundle.commitment, bundle.merkle_proof);
  const expectedKind =
    normalizedPayload.kind === "TokenAdd"
      ? "TokenAdd"
      : normalizedPayload.kind === "TokenPause"
        ? "TokenPause"
        : "TokenResume";
  const checks = {
    bundleVersion: Number(bundle.version) === 1,
    commitmentVersion: Number(bundle.commitment?.version) === 1,
    targetDomainSupported: isSupportedSccpDomain(sccpTokenMessageTargetDomain(normalizedPayload)),
    kindMatches: bundle.commitment?.kind === expectedKind,
    targetDomainMatches: Number(bundle.commitment?.target_domain) === sccpTokenMessageTargetDomain(normalizedPayload),
    messageIdMatches: normalizeHexInput(bundle.commitment?.message_id, "bundle.commitment.message_id", 32) === normalizeHexInput(expectedMessageId, "expectedMessageId", 32),
    payloadHashMatches: normalizeHexInput(bundle.commitment?.payload_hash, "bundle.commitment.payload_hash", 32) === normalizeHexInput(expectedPayloadHash, "expectedPayloadHash", 32),
    merkleRootMatches: normalizeHexInput(bundle.commitment_root, "bundle.commitment_root", 32) === normalizeHexInput(expectedMerkleRoot, "expectedMerkleRoot", 32),
  };
  return {
    ok: Object.values(checks).every(Boolean),
    expectedMessageId,
    expectedPayloadHash,
    expectedMerkleRoot,
    checks,
  };
};

export const validateSccpGovernanceBundleSurface = (bundle) => {
  const normalizedPayload = normalizeGovernanceMessagePayload(bundle.payload);
  const expectedMessageId = sccpTokenMessageId(normalizedPayload);
  const expectedPayloadHash = sccpPayloadHash(canonicalSccpGovernancePayloadBytes(bundle.payload));
  const expectedMerkleRoot = sccpMerkleRootFromCommitment(bundle.commitment, bundle.merkle_proof);
  const expectedCertificateHash = sccpParliamentCertificateHash(bundle.parliament_certificate || "");
  const checks = {
    bundleVersion: Number(bundle.version) === 1,
    commitmentVersion: Number(bundle.commitment?.version) === 1,
    targetDomainSupported: isSupportedSccpDomain(Number(normalizedPayload.value.target_domain)),
    kindMatches: bundle.commitment?.kind === normalizedPayload.kind,
    targetDomainMatches: Number(bundle.commitment?.target_domain) === Number(normalizedPayload.value.target_domain),
    messageIdMatches: normalizeHexInput(bundle.commitment?.message_id, "bundle.commitment.message_id", 32) === normalizeHexInput(expectedMessageId, "expectedMessageId", 32),
    payloadHashMatches: normalizeHexInput(bundle.commitment?.payload_hash, "bundle.commitment.payload_hash", 32) === normalizeHexInput(expectedPayloadHash, "expectedPayloadHash", 32),
    merkleRootMatches: normalizeHexInput(bundle.commitment_root, "bundle.commitment_root", 32) === normalizeHexInput(expectedMerkleRoot, "expectedMerkleRoot", 32),
    certificateHashMatches: normalizeHexInput(bundle.commitment?.parliament_certificate_hash, "bundle.commitment.parliament_certificate_hash", 32) === normalizeHexInput(expectedCertificateHash, "expectedCertificateHash", 32),
  };
  return {
    ok: Object.values(checks).every(Boolean),
    expectedMessageId,
    expectedPayloadHash,
    expectedMerkleRoot,
    expectedCertificateHash,
    checks,
  };
};

const normalizeSccpMessageTransparentPublicInputs = (input) => {
  if (!input || typeof input !== "object" || Array.isArray(input)) {
    throw new TypeError("publicInputs must be an object");
  }
  return {
    version: Number(input.version ?? 1),
    messageId: normalizeHex32(input.messageId ?? input.message_id, "publicInputs.messageId"),
    payloadHash: normalizeHex32(input.payloadHash ?? input.payload_hash, "publicInputs.payloadHash"),
    targetDomain: Number(input.targetDomain ?? input.target_domain),
    commitmentRoot: normalizeHex32(input.commitmentRoot ?? input.commitment_root, "publicInputs.commitmentRoot"),
    finalityHeight: normalizeUnsignedBigInt(
      input.finalityHeight ?? input.finality_height,
      "publicInputs.finalityHeight",
    ).toString(),
    finalityBlockHash: normalizeHex32(
      input.finalityBlockHash ?? input.finality_block_hash,
      "publicInputs.finalityBlockHash",
    ),
  };
};

export const canonicalSccpMessageTransparentPublicInputsBytes = (input) => {
  const publicInputs = normalizeSccpMessageTransparentPublicInputs(input);
  let out = new Uint8Array();
  out = writeU8(out, publicInputs.version);
  out = concatBytes(out, hexToBytes(publicInputs.messageId, "publicInputs.messageId", 32));
  out = concatBytes(out, hexToBytes(publicInputs.payloadHash, "publicInputs.payloadHash", 32));
  out = writeU32Le(out, publicInputs.targetDomain);
  out = concatBytes(out, hexToBytes(publicInputs.commitmentRoot, "publicInputs.commitmentRoot", 32));
  out = writeU64Le(out, publicInputs.finalityHeight);
  out = concatBytes(out, hexToBytes(publicInputs.finalityBlockHash, "publicInputs.finalityBlockHash", 32));
  return out;
};

export const sccpTonSubmissionQueryId = (publicInputs) => {
  const normalized = normalizeSccpMessageTransparentPublicInputs(publicInputs);
  const messageId = hexToBytes(normalized.messageId, "publicInputs.messageId", 32);
  return new DataView(messageId.buffer, messageId.byteOffset, messageId.byteLength).getBigUint64(0, false);
};

const tonMinSizeBytes = (value) => {
  const numeric = normalizeUnsignedBigInt(value, "TON sized integer");
  for (let size = 1; size <= 7; size += 1) {
    if (numeric <= (1n << BigInt(size * 8)) - 1n) return size;
  }
  throw new RangeError("TON sized integer is too large");
};

const tonSizedUint = (value, size) => {
  const numeric = normalizeUnsignedBigInt(value, "TON sized integer");
  if (!Number.isInteger(size) || size < 1 || size > 7) {
    throw new RangeError("TON size must be 1..7 bytes");
  }
  const out = new Uint8Array(size);
  let working = numeric;
  for (let index = size - 1; index >= 0; index -= 1) {
    out[index] = Number(working & 0xffn);
    working >>= 8n;
  }
  if (working !== 0n) throw new RangeError("TON sized integer overflows");
  return out;
};

const tonSerializeCells = (cells, sizeBytes) => {
  const parts = [];
  for (const [cellIndex, cell] of cells.entries()) {
    const data = toBytes(cell.data ?? new Uint8Array(), `cells[${cellIndex}].data`);
    const refs = Array.isArray(cell.refs) ? cell.refs : [];
    if (data.length > SCCP_TON_MAX_CELL_DATA_BYTES) {
      throw new RangeError(`cells[${cellIndex}].data exceeds one TON cell`);
    }
    if (refs.length > SCCP_TON_MAX_REFS) {
      throw new RangeError(`cells[${cellIndex}].refs exceeds TON ref count`);
    }
    parts.push(Uint8Array.from([refs.length, data.length * 2]), data);
    for (const refIndex of refs) {
      if (!Number.isInteger(refIndex) || refIndex < 0 || refIndex >= cells.length) {
        throw new RangeError(`cells[${cellIndex}].refs contains an invalid cell index`);
      }
      parts.push(tonSizedUint(refIndex, sizeBytes));
    }
  }
  return concatBytes(...parts);
};

const encodeTonBocSingleRoot = (cells, rootIndex = 0) => {
  if (!Array.isArray(cells) || cells.length === 0) throw new TypeError("TON BOC cells must not be empty");
  if (!Number.isInteger(rootIndex) || rootIndex < 0 || rootIndex >= cells.length) {
    throw new RangeError("TON BOC root index is invalid");
  }
  const sizeBytes = tonMinSizeBytes(Math.max(cells.length, rootIndex));
  const cellsBytes = tonSerializeCells(cells, sizeBytes);
  const offsetBytes = tonMinSizeBytes(cellsBytes.length);
  return concatBytes(
    SCCP_TON_BOC_MAGIC,
    Uint8Array.from([sizeBytes, offsetBytes]),
    tonSizedUint(cells.length, sizeBytes),
    tonSizedUint(1, sizeBytes),
    tonSizedUint(0, sizeBytes),
    tonSizedUint(cellsBytes.length, offsetBytes),
    tonSizedUint(rootIndex, sizeBytes),
    cellsBytes,
  );
};

const pushTonSnakeCells = (cells, bytes) => {
  const data = toBytes(bytes, "TON snake bytes");
  const start = cells.length;
  if (data.length === 0) {
    cells.push({ data: new Uint8Array(), refs: [] });
    return start;
  }
  const chunkCount = Math.ceil(data.length / SCCP_TON_MAX_CELL_DATA_BYTES);
  for (let index = 0; index < chunkCount; index += 1) {
    const chunkStart = index * SCCP_TON_MAX_CELL_DATA_BYTES;
    const chunk = data.subarray(chunkStart, Math.min(chunkStart + SCCP_TON_MAX_CELL_DATA_BYTES, data.length));
    cells.push({ data: chunk, refs: index + 1 === chunkCount ? [] : [start + index + 1] });
  }
  return start;
};

const enumCode = (value, table, label) => {
  const key = typeof value === "string" ? value : value?.family ?? value?.kind ?? value?.type;
  if (key in table) return table[key];
  throw new TypeError(`${label} is unsupported`);
};

const verifierBackendFamilyForTon = (manifest) =>
  manifest.verifierBackendFamily ??
  manifest.verifier_backend_family ??
  manifest.verifierBackend?.family ??
  manifest.verifier_backend?.family ??
  "TonContract";

export const canonicalSccpTonSubmissionMetadataBytes = (input) => {
  const manifest = input?.manifest ?? input;
  const destinationBinding = input?.destinationBinding ?? input?.destination_binding ?? manifest.destinationBinding ?? manifest.destination_binding;
  const publicInputs = input?.publicInputs ?? input?.public_inputs;
  const statementHash = normalizeHex32(input?.statementHash ?? input?.statement_hash, "statementHash");
  let out = new Uint8Array();
  out = writeU8(out, 1);
  out = writeU32Le(out, Number(manifest.localDomain ?? manifest.local_domain));
  out = writeU32Le(out, Number(manifest.counterpartyDomain ?? manifest.counterparty_domain));
  out = writeU8(out, enumCode(manifest.securityModel ?? manifest.security_model, { RecursiveZk: 1 }, "securityModel"));
  out = writeU8(out, enumCode(manifest.anchorGovernance ?? manifest.anchor_governance, { CryptographicProof: 1 }, "anchorGovernance"));
  out = writeU8(out, enumCode(manifest.verifierTarget ?? manifest.verifier_target, { TonContract: 3 }, "verifierTarget"));
  out = writeU8(out, enumCode(verifierBackendFamilyForTon(manifest), { TonContract: 3 }, "verifierBackendFamily"));
  out = writeString(out, manifest.proofFamily ?? manifest.proof_family, "proofFamily");
  out = writeString(
    out,
    manifest.verifierBackendKey ?? manifest.verifier_backend_key ?? manifest.verifierBackend?.key ?? manifest.verifier_backend?.key,
    "verifierBackendKey",
  );
  out = writeString(out, manifest.messageBackend ?? manifest.message_backend, "messageBackend");
  out = writeString(out, manifest.registryBackend ?? manifest.registry_backend, "registryBackend");
  out = writeString(out, manifest.manifestSeed ?? manifest.manifest_seed, "manifestSeed");
  out = writeString(out, destinationBinding.key, "destinationBinding.key");
  out = concatBytes(out, hexToBytes(destinationBinding.bindingHash ?? destinationBinding.binding_hash, "destinationBinding.bindingHash", 32));
  out = concatBytes(out, hexToBytes(statementHash, "statementHash", 32));
  out = concatBytes(out, canonicalSccpMessageTransparentPublicInputsBytes(publicInputs));
  return out;
};

export const buildSccpTonMessageBodyBoc = (input) => {
  if (!input || typeof input !== "object" || Array.isArray(input)) {
    throw new TypeError("TON SCCP submission input must be an object");
  }
  const publicInputs = normalizeSccpMessageTransparentPublicInputs(input.publicInputs ?? input.public_inputs);
  const publicInputsBytes = canonicalSccpMessageTransparentPublicInputsBytes(publicInputs);
  const proofBytes = toBytes(input.proofBytes ?? input.proof_bytes, "proofBytes");
  const bundleBytes = toBytes(input.bundleBytes ?? input.bundle_bytes, "bundleBytes");
  const statementHash = normalizeHex32(input.statementHash ?? input.statement_hash, "statementHash");
  const destinationBindingHash = normalizeHex32(
    input.destinationBindingHash ??
      input.destination_binding_hash ??
      input.destinationBinding?.bindingHash ??
      input.destination_binding?.binding_hash,
    "destinationBindingHash",
  );
  const metadataBytes =
    input.metadataBytes || input.metadata_bytes
      ? toBytes(input.metadataBytes ?? input.metadata_bytes, "metadataBytes")
      : input.manifest
        ? canonicalSccpTonSubmissionMetadataBytes({
            manifest: input.manifest,
            destinationBinding: input.destinationBinding ?? input.destination_binding,
            publicInputs,
            statementHash,
          })
        : new Uint8Array();
  const queryId = input.queryId ?? input.query_id ?? sccpTonSubmissionQueryId(publicInputs);
  let rootData = new Uint8Array();
  rootData = writeU32Be(rootData, SCCP_TON_SUBMIT_OP_V1);
  rootData = writeU64Be(rootData, queryId);
  rootData = writeU16Be(rootData, SCCP_TON_MESSAGE_SCHEMA_VERSION_V1);
  rootData = concatBytes(rootData, hexToBytes(statementHash, "statementHash", 32));
  rootData = concatBytes(rootData, hexToBytes(destinationBindingHash, "destinationBindingHash", 32));

  const cells = [{ data: rootData, refs: [] }];
  const publicInputsRoot = pushTonSnakeCells(cells, publicInputsBytes);
  const proofRoot = pushTonSnakeCells(cells, proofBytes);
  const bundleRoot = pushTonSnakeCells(cells, bundleBytes);
  const metadataRoot = pushTonSnakeCells(cells, metadataBytes);
  cells[0].refs = [publicInputsRoot, proofRoot, bundleRoot, metadataRoot];
  return encodeTonBocSingleRoot(cells, 0);
};

export const buildTonSccpProofRequest = (input) => {
  const publicInputs = normalizeSccpMessageTransparentPublicInputs(input.publicInputs ?? input.public_inputs);
  const publicInputsBytes = canonicalSccpMessageTransparentPublicInputsBytes(publicInputs);
  const bundleBytes = toBytes(input.bundleBytes ?? input.bundle_bytes, "bundleBytes");
  const sourceProofBytes = input.sourceProofBytes || input.source_proof_bytes
    ? toBytes(input.sourceProofBytes ?? input.source_proof_bytes, "sourceProofBytes")
    : new Uint8Array();
  const requestHash = bytesToHex(
    prefixedBlake2b("sccp:ton:proof-request:v1", concatBytes(publicInputsBytes, bundleBytes, sourceProofBytes)),
  );
  return {
    version: 1,
    backend: input.backend ?? SCCP_TON_CONTRACT_PROOF_BACKEND_V1,
    sourceDomain: Number(input.sourceDomain ?? input.source_domain ?? SCCP_DOMAIN_TON),
    targetDomain: publicInputs.targetDomain,
    publicInputs,
    publicInputsBytes,
    bundleBytes,
    sourceProofBytes,
    requestHash,
  };
};

export const buildTonSccpSubmission = (input) => {
  const messageBodyBoc = buildSccpTonMessageBodyBoc(input);
  return {
    version: 1,
    envelopeEncoding: SCCP_TON_MESSAGE_BODY_BOC_V1,
    submissionKind: "internal_message",
    verifierEntrypoint: "op::submit_sccp_message_proof",
    messageBodyBoc,
    messageBodyBocHex: bytesToHex(messageBodyBoc),
    arguments: [{ key: "message_body_boc", encoding: "ton_boc", bytes: bytesToHex(messageBodyBoc) }],
    envelopeBytes: messageBodyBoc,
    envelopeHex: bytesToHex(messageBodyBoc),
  };
};

export class TonSccpProver {
  constructor(options = {}) {
    if (!options || typeof options !== "object" || Array.isArray(options)) {
      throw new TypeError("TonSccpProver options must be an object");
    }
    this.witnessProvider = options.witnessProvider ?? options.witness_provider ?? null;
    this.proveFn = options.prove ?? options.proveFn ?? options.prove_fn ?? null;
  }

  async buildRequest(input, options = {}) {
    const witness = this.witnessProvider
      ? await this.witnessProvider.resolveWitness(input, options)
      : input;
    return buildTonSccpProofRequest(witness);
  }

  async prove(input, options = {}) {
    const request = await this.buildRequest(input, options);
    if (typeof this.proveFn !== "function") {
      const error = new Error(
        "TON SCCP local prover is not linked; provide a browser-safe prove function before generating production proofs",
      );
      error.code = "ERR_SCCP_TON_PROVER_UNAVAILABLE";
      throw error;
    }
    const result = await this.proveFn(request, options);
    const proofBytes = toBytes(result?.proofBytes ?? result?.proof_bytes ?? result?.proof, "proofBytes");
    if (proofBytes.length === 0) throw new TypeError("proofBytes must not be empty");
    return {
      version: 1,
      backend: request.backend,
      proofBytes,
      proofBase64: bytesToBase64(proofBytes),
      publicInputs: request.publicInputs,
      requestHash: request.requestHash,
    };
  }
}

export function normalizeSolanaSccpWitness(input) {
  if (!input || typeof input !== "object" || Array.isArray(input)) {
    throw new TypeError("Solana SCCP witness must be an object");
  }
  const bundle = input.bundle && typeof input.bundle === "object" ? input.bundle : null;
  const commitment = bundle?.commitment ?? input.commitment ?? {};
  const payload = bundle?.payload ?? input.payload ?? null;
  const targetDomain =
    input.targetDomain ?? input.target_domain ?? commitment.target_domain ?? SCCP_DOMAIN_SORA;
  const messageId = input.messageId ?? input.message_id ?? commitment.message_id;
  const payloadHash = input.payloadHash ?? input.payload_hash ?? commitment.payload_hash;
  return {
    version: 1,
    sourceDomain: SCCP_DOMAIN_SOL,
    targetDomain: Number(targetDomain),
    mainnetGenesisHash: normalizeNonEmptyString(
      input.mainnetGenesisHash ?? input.mainnet_genesis_hash ?? SCCP_SOLANA_MAINNET_GENESIS_HASH,
      "mainnetGenesisHash",
    ),
    finalizedSlot: normalizeUnsignedBigInt(
      input.finalizedSlot ?? input.finalized_slot ?? input.slot,
      "finalizedSlot",
    ).toString(),
    blockhash: normalizeNonEmptyString(input.blockhash, "blockhash"),
    bankHash: normalizeHex32(input.bankHash ?? input.bank_hash, "bankHash"),
    transactionStatusRoot: normalizeHex32(
      input.transactionStatusRoot ?? input.transaction_status_root,
      "transactionStatusRoot",
    ),
    messageProofHash: normalizeHex32(
      input.messageProofHash ?? input.message_proof_hash,
      "messageProofHash",
    ),
    transactionSignature: normalizeNonEmptyString(
      input.transactionSignature ?? input.transaction_signature,
      "transactionSignature",
    ),
    emitterProgramId: normalizeNonEmptyString(
      input.emitterProgramId ?? input.emitter_program_id,
      "emitterProgramId",
    ),
    messageId: normalizeHex32(messageId, "messageId"),
    payloadHash: normalizeHex32(payloadHash, "payloadHash"),
    commitmentRoot: normalizeHex32(
      input.commitmentRoot ?? input.commitment_root ?? bundle?.commitment_root,
      "commitmentRoot",
    ),
    sourceEventDigest: normalizeHex32(
      input.sourceEventDigest ?? input.source_event_digest,
      "sourceEventDigest",
    ),
    payload,
  };
}

export function canonicalSolanaSccpWitnessBytes(input) {
  const witness = normalizeSolanaSccpWitness(input);
  let out = new Uint8Array();
  out = writeU8(out, witness.version);
  out = writeU32Le(out, witness.sourceDomain);
  out = writeU32Le(out, witness.targetDomain);
  out = writeString(out, witness.mainnetGenesisHash, "mainnetGenesisHash");
  out = writeU64Le(out, witness.finalizedSlot);
  out = writeString(out, witness.blockhash, "blockhash");
  out = writeString(out, witness.transactionSignature, "transactionSignature");
  out = writeString(out, witness.emitterProgramId, "emitterProgramId");
  out = concatBytes(out, hexToBytes(witness.bankHash, "bankHash", 32));
  out = concatBytes(out, hexToBytes(witness.transactionStatusRoot, "transactionStatusRoot", 32));
  out = concatBytes(out, hexToBytes(witness.messageProofHash, "messageProofHash", 32));
  out = concatBytes(out, hexToBytes(witness.messageId, "messageId", 32));
  out = concatBytes(out, hexToBytes(witness.payloadHash, "payloadHash", 32));
  out = concatBytes(out, hexToBytes(witness.commitmentRoot, "commitmentRoot", 32));
  out = concatBytes(out, hexToBytes(witness.sourceEventDigest, "sourceEventDigest", 32));
  return out;
}

export function buildSolanaSccpProofRequest(input) {
  const witness = normalizeSolanaSccpWitness(input);
  const witnessHash = bytesToHex(
    prefixedBlake2b("sccp:solana:witness:v1", canonicalSolanaSccpWitnessBytes(witness)),
  );
  return {
    version: 1,
    backend: SCCP_SOLANA_RECURSIVE_PROOF_BACKEND_V1,
    sourceDomain: SCCP_DOMAIN_SOL,
    targetDomain: witness.targetDomain,
    mainnetGenesisHash: witness.mainnetGenesisHash,
    witnessHash,
    publicInputs: {
      messageId: witness.messageId,
      payloadHash: witness.payloadHash,
      commitmentRoot: witness.commitmentRoot,
      finalizedSlot: witness.finalizedSlot,
      blockhash: witness.blockhash,
      sourceEventDigest: witness.sourceEventDigest,
    },
    witness,
  };
}

function normalizeSolanaProofResult(result, request) {
  if (!result || typeof result !== "object" || Array.isArray(result)) {
    throw new TypeError("Solana SCCP proof result must be an object");
  }
  const proofBytes = toBytes(
    result.proofBytes ?? result.proof_bytes ?? result.proof,
    "proofBytes",
  );
  if (proofBytes.length === 0) {
    throw new TypeError("proofBytes must not be empty");
  }
  const envelopeHash = bytesToHex(
    prefixedBlake2b(
      "sccp:solana:proof-envelope:v1",
      concatBytes(hexToBytes(request.witnessHash, "witnessHash", 32), proofBytes),
    ),
  );
  return {
    version: 1,
    backend: request.backend,
    proofBytes,
    proofBase64: bytesToBase64(proofBytes),
    publicInputs: request.publicInputs,
    witnessHash: request.witnessHash,
    envelopeHash,
  };
}

export class SolanaSccpProver {
  constructor(options = {}) {
    if (!options || typeof options !== "object" || Array.isArray(options)) {
      throw new TypeError("SolanaSccpProver options must be an object");
    }
    this.witnessProvider = options.witnessProvider ?? options.witness_provider ?? null;
    this.proveFn = options.prove ?? options.proveFn ?? options.prove_fn ?? null;
  }

  async buildRequest(input, options = {}) {
    const witness = this.witnessProvider
      ? await this.witnessProvider.resolveWitness(input, options)
      : input;
    return buildSolanaSccpProofRequest(witness);
  }

  async prove(input, options = {}) {
    const request = await this.buildRequest(input, options);
    if (typeof this.proveFn !== "function") {
      const error = new Error(
        "Solana SCCP local prover is not linked; provide a pure TypeScript prove function before generating production proofs",
      );
      error.code = "ERR_SCCP_SOLANA_PROVER_UNAVAILABLE";
      throw error;
    }
    return normalizeSolanaProofResult(await this.proveFn(request, options), request);
  }
}

function toBytes(value, label) {
  if (value instanceof Uint8Array) return value;
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }
  if (Array.isArray(value)) {
    return Uint8Array.from(value);
  }
  if (typeof value === "string") {
    return hexToBytes(value, label);
  }
  throw new TypeError(`${label} must be bytes or hex`);
}
