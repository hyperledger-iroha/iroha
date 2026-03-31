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

export const SCCP_CORE_REMOTE_DOMAINS = [
  SCCP_DOMAIN_ETH,
  SCCP_DOMAIN_BSC,
  SCCP_DOMAIN_SOL,
  SCCP_DOMAIN_TON,
  SCCP_DOMAIN_TRON,
  SCCP_DOMAIN_SORA_KUSAMA,
  SCCP_DOMAIN_SORA_POLKADOT,
];

const SCCP_MSG_PREFIX_BURN_V1 = "sccp:burn:v1";
const SCCP_MSG_PREFIX_TOKEN_ADD_V1 = "sccp:token:add:v1";
const SCCP_MSG_PREFIX_TOKEN_PAUSE_V1 = "sccp:token:pause:v1";
const SCCP_MSG_PREFIX_TOKEN_RESUME_V1 = "sccp:token:resume:v1";
const SCCP_HUB_LEAF_PREFIX_V1 = "sccp:hub:leaf:v1";
const SCCP_HUB_NODE_PREFIX_V1 = "sccp:hub:node:v1";
const SCCP_PAYLOAD_HASH_PREFIX_V1 = "sccp:payload:v1";
const SCCP_PARLIAMENT_HASH_PREFIX_V1 = "sccp:parliament:v1";

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

const normalizeGovernancePayload = (payload) => {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) {
    throw new TypeError("governance payload must be an object");
  }
  if (typeof payload.kind === "string" && payload.value && typeof payload.value === "object") {
    return {
      kind: payload.kind,
      value: payload.value,
    };
  }
  if ("Add" in payload) {
    return { kind: "Add", value: payload.Add };
  }
  if ("Pause" in payload) {
    return { kind: "Pause", value: payload.Pause };
  }
  if ("Resume" in payload) {
    return { kind: "Resume", value: payload.Resume };
  }
  throw new TypeError("governance payload must be Add, Pause, or Resume");
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

export const canonicalSccpGovernancePayloadBytes = (payload) => {
  const normalized = normalizeGovernancePayload(payload);
  if (normalized.kind === "Add") {
    return concatBytes(
      Uint8Array.from([0]),
      canonicalSccpTokenAddPayloadBytes(normalized.value),
    );
  }
  if (normalized.kind === "Pause") {
    return concatBytes(
      Uint8Array.from([1]),
      canonicalSccpTokenControlPayloadBytes(normalized.value),
    );
  }
  if (normalized.kind === "Resume") {
    return concatBytes(
      Uint8Array.from([2]),
      canonicalSccpTokenControlPayloadBytes(normalized.value),
    );
  }
  throw new TypeError(`unsupported governance payload kind: ${normalized.kind}`);
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

export const sccpGovernanceMessageId = (payload, options = {}) => {
  const normalized = normalizeGovernancePayload(payload);
  if (normalized.kind === "Add") return sccpTokenAddMessageId(normalized.value, options);
  if (normalized.kind === "Pause") return sccpTokenPauseMessageId(normalized.value, options);
  if (normalized.kind === "Resume") return sccpTokenResumeMessageId(normalized.value, options);
  throw new TypeError(`unsupported governance payload kind: ${normalized.kind}`);
};

export const sccpGovernanceTargetDomain = (payload) => {
  const normalized = normalizeGovernancePayload(payload);
  return Number(normalized.value.target_domain);
};

export const sccpPayloadHash = (payload, options = {}) =>
  bytesToHex(prefixedBlake2b(SCCP_PAYLOAD_HASH_PREFIX_V1, toBytes(payload, "payload")), options.prefix !== false);

export const sccpParliamentCertificateHash = (certificateBytes, options = {}) =>
  bytesToHex(
    prefixedBlake2b(SCCP_PARLIAMENT_HASH_PREFIX_V1, toBytes(certificateBytes, "certificateBytes")),
    options.prefix !== false,
  );

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
  if (commitment.parliament_certificate_hash == null) {
    out = writeU8(out, 0);
    return out;
  }
  out = writeU8(out, 1);
  out = concatBytes(
    out,
    hexToBytes(commitment.parliament_certificate_hash, "commitment.parliament_certificate_hash", 32),
  );
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
    parliamentCertificateAbsent: bundle.commitment?.parliament_certificate_hash == null,
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
  const normalizedPayload = normalizeGovernancePayload(bundle.payload);
  const expectedMessageId = sccpGovernanceMessageId(normalizedPayload);
  const expectedPayloadHash = sccpPayloadHash(canonicalSccpGovernancePayloadBytes(normalizedPayload));
  const expectedMerkleRoot = sccpMerkleRootFromCommitment(bundle.commitment, bundle.merkle_proof);
  const expectedCertificateHash =
    bundle.parliament_certificate == null
      ? null
      : sccpParliamentCertificateHash(hexToBytes(bundle.parliament_certificate, "bundle.parliament_certificate"));
  const expectedKind =
    normalizedPayload.kind === "Add"
      ? "TokenAdd"
      : normalizedPayload.kind === "Pause"
        ? "TokenPause"
        : "TokenResume";
  const checks = {
    bundleVersion: Number(bundle.version) === 1,
    commitmentVersion: Number(bundle.commitment?.version) === 1,
    targetDomainSupported: isSupportedSccpDomain(sccpGovernanceTargetDomain(normalizedPayload)),
    kindMatches: bundle.commitment?.kind === expectedKind,
    targetDomainMatches: Number(bundle.commitment?.target_domain) === sccpGovernanceTargetDomain(normalizedPayload),
    messageIdMatches: normalizeHexInput(bundle.commitment?.message_id, "bundle.commitment.message_id", 32) === normalizeHexInput(expectedMessageId, "expectedMessageId", 32),
    payloadHashMatches: normalizeHexInput(bundle.commitment?.payload_hash, "bundle.commitment.payload_hash", 32) === normalizeHexInput(expectedPayloadHash, "expectedPayloadHash", 32),
    parliamentCertificateHashMatches:
      expectedCertificateHash == null
        ? bundle.commitment?.parliament_certificate_hash == null
        : normalizeHexInput(bundle.commitment?.parliament_certificate_hash, "bundle.commitment.parliament_certificate_hash", 32) === normalizeHexInput(expectedCertificateHash, "expectedCertificateHash", 32),
    merkleRootMatches: normalizeHexInput(bundle.commitment_root, "bundle.commitment_root", 32) === normalizeHexInput(expectedMerkleRoot, "expectedMerkleRoot", 32),
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

function toBytes(value, label) {
  if (value instanceof Uint8Array) return value;
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }
  if (typeof value === "string") {
    return hexToBytes(value, label);
  }
  throw new TypeError(`${label} must be bytes or hex`);
}
