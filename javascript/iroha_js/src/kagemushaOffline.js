/** Transport-only ABI-23/manifest-V4 Kagemusha projections.
 *
 * This module deliberately has no native prover or artifact-install surface.
 * Top-up and redemption methods accept canonical Norito archives produced by
 * a supported wallet/prover implementation.
 */

import { blake2b256 } from "./blake2b.js";
import { computeHashLiteralCrc } from "./hashLiteralCrc.js";
import { validateNoritoFrame } from "./norito.js";

export const KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION = 23;
export const KAGEMUSHA_MANIFEST_VERSION = 4;
export const KAGEMUSHA_MAX_HOPS = 8;
export const KAGEMUSHA_CASH_HANDOFF_CAPABILITY = "cash_handoff_v1";
export const KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES = 512 * 1024;
export const KAGEMUSHA_REDEEM_REQUEST_MAX_BYTES = 48 * 1024 * 1024;

const HASH_32 = /^[0-9a-f]{63}[13579bdf]$/u;
const OPERATION_ID = /^(?!0{64}$)[0-9a-f]{64}$/u;
const ERROR_CODE = /^[a-z0-9][a-z0-9_]{0,63}$/u;
const POSITIVE_DECIMAL = /^[0-9]+$/u;
const MAX_U64 = 0xffff_ffff_ffff_ffffn;
const MAX_SAFE_INTEGER_BIGINT = BigInt(Number.MAX_SAFE_INTEGER);
const TOP_UP_REQUEST_SCHEMA_NAME = "iroha.torii.v1.offline.top_up.request";
const REDEEM_REQUEST_SCHEMA_NAME = "iroha.torii.v1.offline.redeem.request";
const TOP_UP_REQUEST_FIELD_COUNT = 8;
const TOP_UP_OPERATION_ID_FIELD_INDEX = 6;
const REDEEM_REQUEST_FIELD_COUNT = 10;
const REDEEM_OPERATION_ID_FIELD_INDEX = 8;
const KAGEMUSHA_TOP_UP_ANCHOR_WITNESS_KEY_TAG = 0xd2;
const KAGEMUSHA_TOP_UP_FINALITY_MAX_ANCHORS_PER_BLOCK = 16;
const KAGEMUSHA_OPERATION_STATUS_JSON_MAX_BYTES = 16 * 1024 * 1024;
const KAGEMUSHA_TOP_UP_NODE_DOMAIN = new TextEncoder().encode(
  "iroha:kagemusha:v2:topup-node",
);
const KAGEMUSHA_TOP_UP_POST_STATE_ROOT_DOMAIN = new TextEncoder().encode(
  "iroha:kagemusha:v2:post-state-root",
);
const EXACT_JSON_MEDIA_TYPE =
  /^[ \t]*application\/json(?:[ \t]*;[ \t]*[!#$%&'*+\-.^_`|~0-9A-Za-z]+=(?:[!#$%&'*+\-.^_`|~0-9A-Za-z]+|"(?:[ \t!#-\[\]-~\u0080-\u00ff]|\\[ \t!-~\u0080-\u00ff])*"))*[ \t]*$/iu;
const OFFLINE_STATUS_FIELDS = Object.freeze([
  "cash_handoff_capability",
  "required_bridge_abi_version",
  "max_hops",
  "ready",
]);

function record(value, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return value;
}

function exactFields(value, context, fields) {
  const item = record(value, context);
  const actual = Object.keys(item);
  const expected = new Set(fields);
  if (actual.length !== fields.length || actual.some((field) => !expected.has(field))) {
    throw new TypeError(`${context} contains missing or unknown fields`);
  }
  return item;
}

function exactString(
  value,
  context,
  { maximum = 1024, maximumUtf8Bytes = maximum * 4 } = {},
) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    [...value].length > maximum ||
    new TextEncoder().encode(value).byteLength > maximumUtf8Bytes ||
    value.trim() !== value ||
    /[\ud800-\udfff]/u.test(value) ||
    /[\u0000-\u001f\u007f-\u009f]/u.test(value)
  ) {
    throw new TypeError(`${context} must be exact non-empty text`);
  }
  return value;
}

function safeUnsigned(value, context, { positive = false, maximum = Number.MAX_SAFE_INTEGER } = {}) {
  if (!Number.isSafeInteger(value) || value < (positive ? 1 : 0) || value > maximum) {
    throw new TypeError(`${context} must be a${positive ? " positive" : "n"} safe unsigned integer`);
  }
  return value;
}

function losslessU64(value, context, { positive = false } = {}) {
  let integer;
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) {
      throw new TypeError(
        `${context} must be ${positive ? "a positive" : "a"} lossless unsigned 64-bit integer`,
      );
    }
    integer = BigInt(value);
  } else if (typeof value === "bigint") {
    integer = value;
  } else {
    throw new TypeError(
      `${context} must be ${positive ? "a positive" : "a"} lossless unsigned 64-bit integer`,
    );
  }
  if (integer < (positive ? 1n : 0n) || integer > MAX_U64) {
    throw new TypeError(
      `${context} must be ${positive ? "a positive" : "a"} lossless unsigned 64-bit integer`,
    );
  }
  return integer <= MAX_SAFE_INTEGER_BIGINT ? Number(integer) : integer;
}

function readCompactFieldLength(payload, offset, context) {
  let value = 0n;
  let shift = 0n;
  for (let used = 0; used < 10; used += 1) {
    if (offset + used >= payload.length) {
      throw new TypeError(`${context} has a truncated compact field length`);
    }
    const byte = payload[offset + used];
    if (used === 9 && (byte & 0xfe) !== 0) {
      throw new TypeError(`${context} compact field length exceeds u64`);
    }
    value |= BigInt(byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) {
      if (used > 0 && byte === 0) {
        throw new TypeError(`${context} compact field length is not minimally encoded`);
      }
      return { length: value, bytesRead: used + 1 };
    }
    shift += 7n;
  }
  throw new TypeError(`${context} compact field length exceeds u64`);
}

function operationIdFromCompactRequest(
  payload,
  fieldCount,
  operationIdFieldIndex,
  context,
) {
  let offset = 0;
  let operationId;
  for (let index = 0; index < fieldCount; index += 1) {
    const fieldContext = `${context}.field[${index}]`;
    const { length, bytesRead } = readCompactFieldLength(payload, offset, fieldContext);
    offset += bytesRead;
    const remaining = BigInt(payload.length - offset);
    if (length > remaining) {
      throw new TypeError(`${fieldContext} has an invalid compact payload length`);
    }
    const end = offset + Number(length);
    const field = payload.subarray(offset, end);
    offset = end;
    if (index === 0 && (field.length !== 2 || field[0] !== 4 || field[1] !== 0)) {
      throw new TypeError(`${context} payload version must be the canonical u16 value 4`);
    }
    if (index === operationIdFieldIndex) {
      if (field.length !== 32 || field.every((byte) => byte === 0)) {
        throw new TypeError(`${fieldContext} must contain one non-zero 32-byte operation id`);
      }
      operationId = Array.from(field, (byte) => byte.toString(16).padStart(2, "0")).join("");
    }
  }
  if (offset !== payload.length) {
    throw new TypeError(`${context} contains trailing compact fields or bytes`);
  }
  return operationId;
}

function hash32(value, context, { nonzero = false } = {}) {
  if (typeof value !== "string" || !HASH_32.test(value) || (nonzero && value === "0".repeat(64))) {
    throw new TypeError(`${context} must be ${nonzero ? "non-zero " : ""}an exact canonical lowercase 32-byte Iroha hash`);
  }
  return value;
}

function jsonSnapshot(value) {
  if (typeof structuredClone === "function") {
    return structuredClone(value);
  }
  return JSON.parse(JSON.stringify(value));
}

export function normalizeKagemushaOperationId(value, context = "operationId") {
  if (typeof value !== "string" || !OPERATION_ID.test(value)) {
    throw new TypeError(`${context} must be non-zero lowercase 32-byte hexadecimal`);
  }
  return value;
}

export function requireKagemushaJsonContentType(value, context) {
  if (typeof value !== "string" || !EXACT_JSON_MEDIA_TYPE.test(value)) {
    throw new TypeError(`${context} must use Content-Type application/json`);
  }
}

/** Normalize the exact universal OfflineStatus projection returned by Torii. */
export function normalizeOfflineStatus(payload) {
  const context = "Offline capability";
  const item = exactFields(payload, context, OFFLINE_STATUS_FIELDS);
  if (
    exactString(
      item.cash_handoff_capability,
      `${context}.cash_handoff_capability`,
    ) !== KAGEMUSHA_CASH_HANDOFF_CAPABILITY
  ) {
    throw new TypeError(
      `${context}.cash_handoff_capability must be ${KAGEMUSHA_CASH_HANDOFF_CAPABILITY}`,
    );
  }
  if (
    safeUnsigned(item.required_bridge_abi_version, `${context}.required_bridge_abi_version`, {
      positive: true,
      maximum: 0xffff_ffff,
    }) !== KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION
  ) {
    throw new TypeError(`${context}.required_bridge_abi_version must be 23`);
  }
  if (
    safeUnsigned(item.max_hops, `${context}.max_hops`, {
      positive: true,
      maximum: 0xffff_ffff,
    }) !== KAGEMUSHA_MAX_HOPS
  ) {
    throw new TypeError(`${context}.max_hops must be 8`);
  }
  if (item.ready !== true) {
    throw new TypeError(`${context}.ready must be true`);
  }
  return Object.freeze({
    cash_handoff_capability: KAGEMUSHA_CASH_HANDOFF_CAPABILITY,
    required_bridge_abi_version: KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION,
    max_hops: KAGEMUSHA_MAX_HOPS,
    ready: true,
  });
}

function normalizeKagemushaNoritoRequestV4(
  value,
  maximumBytes,
  expectedSchemaName,
  fieldCount,
  operationIdFieldIndex,
  context,
) {
  const item = exactFields(value, context, ["version", "norito"]);
  if (item.version !== KAGEMUSHA_MANIFEST_VERSION) {
    throw new TypeError(`${context}.version must be 4; V3 archives are not upgraded`);
  }
  if (!ArrayBuffer.isView(item.norito) || item.norito.BYTES_PER_ELEMENT !== 1) {
    throw new TypeError(`${context}.norito must be a byte-array view`);
  }
  const archive = new Uint8Array(
    item.norito.buffer,
    item.norito.byteOffset,
    item.norito.byteLength,
  );
  if (
    archive.byteLength < 40 ||
    archive.byteLength > maximumBytes
  ) {
    throw new TypeError(`${context}.norito must be a bounded canonical Norito archive`);
  }
  let operationId;
  try {
    const frame = validateNoritoFrame(archive, {
      context: `${context}.norito`,
      expectedTypeName: expectedSchemaName,
      expectedPaddingLength: 8,
      requireNonEmptyPayload: true,
    });
    if (frame.flags !== 0x02) {
      throw new TypeError(
        `${context}.norito must use canonical compact-length layout flags`,
      );
    }
    operationId = operationIdFromCompactRequest(
      frame.payload,
      fieldCount,
      operationIdFieldIndex,
      `${context}.norito payload`,
    );
  } catch (error) {
    throw new TypeError(
      `${context}.norito must be a schema-bound canonical Norito archive: ${error.message}`,
      { cause: error },
    );
  }
  return Object.freeze({
    version: KAGEMUSHA_MANIFEST_VERSION,
    operationId,
    norito: new Uint8Array(archive),
  });
}

export function normalizeKagemushaTopUpRequestV4(
  value,
  context = "Kagemusha V4 top-up request",
) {
  return normalizeKagemushaNoritoRequestV4(
    value,
    KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES,
    TOP_UP_REQUEST_SCHEMA_NAME,
    TOP_UP_REQUEST_FIELD_COUNT,
    TOP_UP_OPERATION_ID_FIELD_INDEX,
    context,
  );
}

export function normalizeKagemushaRedeemRequestV4(
  value,
  context = "Kagemusha V4 redemption request",
) {
  return normalizeKagemushaNoritoRequestV4(
    value,
    KAGEMUSHA_REDEEM_REQUEST_MAX_BYTES,
    REDEEM_REQUEST_SCHEMA_NAME,
    REDEEM_REQUEST_FIELD_COUNT,
    REDEEM_OPERATION_ID_FIELD_INDEX,
    context,
  );
}

function taggedKind(value, context) {
  const item = exactFields(value, context, ["kind", "value"]);
  if (item.value !== null || (item.kind !== "top_up" && item.kind !== "redeem")) {
    throw new TypeError(`${context} must be a top_up or redeem unit tag`);
  }
  return item.kind;
}

function taggedPending(value, context) {
  const item = exactFields(value, context, ["state", "value"]);
  if (item.state !== "pending" || item.value !== null) {
    throw new TypeError(`${context} must be a pending unit tag`);
  }
  return "pending";
}

function normalizeAcceptedKagemushaOperationReference(
  payload,
  context = "Accepted Kagemusha operation reference",
) {
  const item = exactFields(payload, context, [
    "operation_id",
    "kind",
    "state",
    "transaction_hash",
    "status_uri",
    "submitted_at_ms",
  ]);
  const operationId = normalizeKagemushaOperationId(
    item.operation_id,
    `${context}.operation_id`,
  );
  const kind = taggedKind(item.kind, `${context}.kind`);
  taggedPending(item.state, `${context}.state`);
  const statusUri = `/v1/offline/operations/${operationId}`;
  if (item.status_uri !== statusUri) {
    throw new TypeError(`${context}.status_uri does not match its operation_id`);
  }
  return Object.freeze({
    operation_id: operationId,
    kind: Object.freeze({ kind, value: null }),
    state: Object.freeze({ state: "pending", value: null }),
    transaction_hash: hash32(item.transaction_hash, `${context}.transaction_hash`),
    status_uri: statusUri,
    submitted_at_ms: losslessU64(item.submitted_at_ms, `${context}.submitted_at_ms`, {
      positive: true,
    }),
  });
}

export function normalizeKagemushaOperationReference(
  payload,
  expected,
) {
  const context = "Kagemusha operation reference";
  const detachedPayload = jsonSnapshot(record(payload, context));
  const normalized = normalizeAcceptedKagemushaOperationReference(detachedPayload, context);
  if (expected === undefined) return normalized;
  const expectedContext = `${context} expectation`;
  const detachedExpected = exactFields(
    jsonSnapshot(record(expected, expectedContext)),
    expectedContext,
    ["expectedOperationId", "expectedKind", "location", "retryAfter"],
  );
  const { expectedOperationId, expectedKind, location, retryAfter } = detachedExpected;
  if (
    normalized.operation_id !== expectedOperationId ||
    normalized.kind.kind !== expectedKind ||
    location !== normalized.status_uri
  ) {
    throw new TypeError(`${context} does not match the submitted V4 command`);
  }
  if (
    typeof retryAfter !== "string" ||
    retryAfter.length > 20 ||
    !POSITIVE_DECIMAL.test(retryAfter) ||
    BigInt(retryAfter) === 0n ||
    BigInt(retryAfter) > MAX_U64
  ) {
    throw new TypeError(`${context} Retry-After must be a positive u64 number of seconds`);
  }
  return normalized;
}

function bytes32(value, context) {
  if (
    !Array.isArray(value) ||
    value.length !== 32 ||
    value.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255)
  ) {
    throw new TypeError(`${context} must be an array of 32 bytes`);
  }
  return value;
}

function bytesToHex(value) {
  return value.map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function bytesEqual(left, right) {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
}

function isMarkedZeroHash(bytes) {
  return bytes.every((byte, index) => byte === (index === bytes.length - 1 ? 1 : 0));
}

function nonzeroBytes32(value, context) {
  const bytes = bytes32(value, context);
  if (bytes.every((byte) => byte === 0)) {
    throw new TypeError(`${context} must be non-zero`);
  }
  return bytes;
}

function concatenateBytes(...parts) {
  const output = new Uint8Array(parts.reduce((length, part) => length + part.length, 0));
  let offset = 0;
  for (const part of parts) {
    output.set(part, offset);
    offset += part.length;
  }
  return output;
}

function irohaHash(bytes) {
  const digest = blake2b256(bytes instanceof Uint8Array ? bytes : Uint8Array.from(bytes));
  digest[digest.length - 1] |= 1;
  return digest;
}

function u16LittleEndian(value) {
  return Uint8Array.of(value & 0xff, (value >>> 8) & 0xff);
}

function u32LittleEndian(value) {
  return Uint8Array.of(
    value & 0xff,
    (value >>> 8) & 0xff,
    (value >>> 16) & 0xff,
    (value >>> 24) & 0xff,
  );
}

function irohaHashLiteralBytes(value, context) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a canonical Iroha hash literal`);
  }
  const match = /^hash:([0-9A-F]{64})#([0-9A-F]{4})$/u.exec(value);
  if (match === null) {
    throw new TypeError(`${context} must be a canonical Iroha hash literal`);
  }
  const [, body, checksum] = match;
  if (computeHashLiteralCrc("hash", body) !== checksum) {
    throw new TypeError(`${context} has an invalid Iroha hash checksum`);
  }
  const bytes = Uint8Array.from(
    Array.from({ length: 32 }, (_, index) => Number.parseInt(body.slice(index * 2, index * 2 + 2), 16)),
  );
  if ((bytes[bytes.length - 1] & 1) === 0) {
    throw new TypeError(`${context} has an invalid Iroha hash marker bit`);
  }
  return bytes;
}

function topUpAnchorLeafHash(operationId, anchorDigest) {
  const keyHash = irohaHash(concatenateBytes(
    Uint8Array.of(KAGEMUSHA_TOP_UP_ANCHOR_WITNESS_KEY_TAG),
    operationId,
  ));
  const valueHash = irohaHash(anchorDigest);
  return irohaHash(concatenateBytes(Uint8Array.of(0), keyHash, valueHash));
}

function topUpAnchorNodeHash(level, left, right) {
  return irohaHash(concatenateBytes(
    KAGEMUSHA_TOP_UP_NODE_DOMAIN,
    Uint8Array.of(0),
    u16LittleEndian(level),
    left,
    right,
  ));
}

function topUpPostStateRoot(anchorCount, ordinaryWritesRoot, topUpAnchorRoot) {
  return irohaHash(concatenateBytes(
    KAGEMUSHA_TOP_UP_POST_STATE_ROOT_DOMAIN,
    Uint8Array.of(0),
    u32LittleEndian(anchorCount),
    ordinaryWritesRoot,
    topUpAnchorRoot,
  ));
}

/**
 * Authenticate one top-up anchor path against its advertised execution
 * commitment. This checks neither the Commit-QC signature nor roster trust.
 */
function validateTopUpFinalityBinding(anchor, finalityProof, operationId, context) {
  const anchorOperationId = nonzeroBytes32(
    anchor.topup_operation_id,
    `${context}.anchor.topup_operation_id`,
  );
  const anchorDigest = nonzeroBytes32(
    anchor.anchor_digest,
    `${context}.anchor.anchor_digest`,
  );
  if (bytesToHex(anchorOperationId) !== operationId) {
    throw new TypeError(`${context}.anchor.topup_operation_id does not match the operation`);
  }

  const proof = exactFields(finalityProof, `${context}.finality_proof`, [
    "version",
    "anchor",
    "commit_qc",
    "anchor_path",
  ]);
  if (proof.version !== 1) {
    throw new TypeError(`${context}.finality_proof.version must be 1`);
  }
  const proofAnchor = exactFields(
    proof.anchor,
    `${context}.finality_proof.anchor`,
    ["topup_operation_id", "anchor_digest"],
  );
  const proofOperationId = nonzeroBytes32(
    proofAnchor.topup_operation_id,
    `${context}.finality_proof.anchor.topup_operation_id`,
  );
  const proofAnchorDigest = nonzeroBytes32(
    proofAnchor.anchor_digest,
    `${context}.finality_proof.anchor.anchor_digest`,
  );
  if (
    !bytesEqual(proofOperationId, anchorOperationId) ||
    !bytesEqual(proofAnchorDigest, anchorDigest)
  ) {
    throw new TypeError(`${context}.finality_proof.anchor does not match the V4 top-up anchor`);
  }

  const path = exactFields(
    proof.anchor_path,
    `${context}.finality_proof.anchor_path`,
    ["leaf_index", "leaf_count", "siblings"],
  );
  const leafCount = safeUnsigned(
    path.leaf_count,
    `${context}.finality_proof.anchor_path.leaf_count`,
    { positive: true, maximum: KAGEMUSHA_TOP_UP_FINALITY_MAX_ANCHORS_PER_BLOCK },
  );
  const leafIndex = safeUnsigned(
    path.leaf_index,
    `${context}.finality_proof.anchor_path.leaf_index`,
    { maximum: KAGEMUSHA_TOP_UP_FINALITY_MAX_ANCHORS_PER_BLOCK - 1 },
  );
  if (leafIndex >= leafCount || !Array.isArray(path.siblings)) {
    throw new TypeError(`${context}.finality_proof.anchor_path has an invalid leaf position`);
  }
  let expectedDepth = 0;
  for (let width = 1; width < leafCount; width *= 2) expectedDepth += 1;
  if (path.siblings.length !== expectedDepth) {
    throw new TypeError(`${context}.finality_proof.anchor_path has a non-canonical depth`);
  }
  const siblings = path.siblings.map((sibling, index) => {
    const normalized = nonzeroBytes32(
      sibling,
      `${context}.finality_proof.anchor_path.siblings[${index}]`,
    );
    if ((normalized[normalized.length - 1] & 1) === 0) {
      throw new TypeError(
        `${context}.finality_proof.anchor_path.siblings[${index}] has an invalid Iroha hash marker bit`,
      );
    }
    return normalized;
  });

  const commitQc = exactFields(
    proof.commit_qc,
    `${context}.finality_proof.commit_qc`,
    ["height_context", "certificate"],
  );
  const heightContext = record(
    commitQc.height_context,
    `${context}.finality_proof.commit_qc.height_context`,
  );
  const certificate = record(
    commitQc.certificate,
    `${context}.finality_proof.commit_qc.certificate`,
  );
  const commitment = record(
    certificate.execution_commitment,
    `${context}.finality_proof.commit_qc.certificate.execution_commitment`,
  );
  const committedCount = safeUnsigned(
    commitment.topup_anchor_count,
    `${context}.finality_proof.commit_qc.certificate.execution_commitment.topup_anchor_count`,
    { positive: true, maximum: KAGEMUSHA_TOP_UP_FINALITY_MAX_ANCHORS_PER_BLOCK },
  );
  if (committedCount !== leafCount) {
    throw new TypeError(`${context}.finality_proof anchor count does not match its path`);
  }
  const committedTopUpRoot = irohaHashLiteralBytes(
    commitment.topup_anchor_root,
    `${context}.finality_proof.commit_qc.certificate.execution_commitment.topup_anchor_root`,
  );
  const ordinaryWritesRoot = irohaHashLiteralBytes(
    commitment.ordinary_writes_root,
    `${context}.finality_proof.commit_qc.certificate.execution_commitment.ordinary_writes_root`,
  );
  const committedPostStateRoot = irohaHashLiteralBytes(
    commitment.post_state_root,
    `${context}.finality_proof.commit_qc.certificate.execution_commitment.post_state_root`,
  );

  let reconstructedRoot = topUpAnchorLeafHash(anchorOperationId, anchorDigest);
  let index = leafIndex;
  for (let level = 0; level < siblings.length; level += 1) {
    reconstructedRoot = (index & 1) === 0
      ? topUpAnchorNodeHash(level, reconstructedRoot, siblings[level])
      : topUpAnchorNodeHash(level, siblings[level], reconstructedRoot);
    index = Math.floor(index / 2);
  }
  if (!bytesEqual(reconstructedRoot, committedTopUpRoot)) {
    throw new TypeError(`${context}.finality_proof anchor path does not match the committed root`);
  }
  const reconstructedPostStateRoot = topUpPostStateRoot(
    committedCount,
    ordinaryWritesRoot,
    committedTopUpRoot,
  );
  if (!bytesEqual(reconstructedPostStateRoot, committedPostStateRoot)) {
    throw new TypeError(`${context}.finality_proof execution post-state root is invalid`);
  }
  return { heightContext };
}

function normalizeAppliedResult(value, operationId, context) {
  const tagged = exactFields(value, context, ["kind", "result"]);
  const result = record(tagged.result, `${context}.result`);
  if (tagged.kind === "redeem") {
    exactFields(result, `${context}.result`, [
      "transaction_hash",
      "finalized_block_height",
    ]);
    return Object.freeze({
      kind: "redeem",
      result: Object.freeze({
        transaction_hash: hash32(result.transaction_hash, `${context}.result.transaction_hash`),
        finalized_block_height: losslessU64(
          result.finalized_block_height,
          `${context}.result.finalized_block_height`,
          { positive: true },
        ),
      }),
    });
  }
  if (tagged.kind !== "top_up") {
    throw new TypeError(`${context}.kind must be top_up or redeem`);
  }
  exactFields(result, `${context}.result`, [
    "transaction_hash",
    "finalized_block_height",
    "anchor",
    "finality_proof",
  ]);
  const anchor = record(result.anchor, `${context}.result.anchor`);
  const artifactBinding = record(
    anchor.artifact_binding,
    `${context}.result.anchor.artifact_binding`,
  );
  if (anchor.version !== 4 || artifactBinding.version !== 4) {
    throw new TypeError(`${context}.result anchor and artifact binding must use V4`);
  }
  const finalityProof = record(result.finality_proof, `${context}.result.finality_proof`);
  const { heightContext } = validateTopUpFinalityBinding(
    anchor,
    finalityProof,
    operationId,
    `${context}.result`,
  );
  const transactionHash = hash32(
    result.transaction_hash,
    `${context}.result.transaction_hash`,
  );
  const finalizedBlockHeight = losslessU64(
    result.finalized_block_height,
    `${context}.result.finalized_block_height`,
    { positive: true },
  );
  const anchorFinalizedHeight = losslessU64(
    anchor.finalized_height,
    `${context}.result.anchor.finalized_height`,
    { positive: true },
  );
  const proofHeight = losslessU64(
    heightContext.height,
    `${context}.result.finality_proof.commit_qc.height_context.height`,
    { positive: true },
  );
  const anchorTransactionHash = bytesToHex(nonzeroBytes32(
    anchor.finalized_tx_hash,
    `${context}.result.anchor.finalized_tx_hash`,
  ));
  const anchorNetwork = irohaHashLiteralBytes(
    anchor.network_id,
    `${context}.result.anchor.network_id`,
  );
  const proofNetwork = irohaHashLiteralBytes(
    heightContext.network_id,
    `${context}.result.finality_proof.commit_qc.height_context.network_id`,
  );
  if (
    isMarkedZeroHash(anchorNetwork) ||
    isMarkedZeroHash(proofNetwork) ||
    transactionHash !== anchorTransactionHash ||
    finalizedBlockHeight !== anchorFinalizedHeight ||
    finalizedBlockHeight !== proofHeight ||
    !bytesEqual(anchorNetwork, proofNetwork)
  ) {
    throw new TypeError(`${context}.result top-up anchor, proof, and terminal result do not match`);
  }
  return Object.freeze({
    kind: "top_up",
    result: Object.freeze({
      transaction_hash: transactionHash,
      finalized_block_height: finalizedBlockHeight,
      anchor,
      finality_proof: finalityProof,
    }),
  });
}

/**
 * Normalize one status against the accepted operation identity.
 * The operation id, kind, and status URI remain fixed, while an exact retry or
 * a foreign-authority global Applied winner may advance the transaction hash.
 * After Pending advances its hash and timestamp, use that returned pair as the
 * accepted reference for the next poll so same-attempt timestamp checks remain
 * exact.
 * Applied top-ups reach this projection only after the exact response bytes
 * pass the native ABI-23 structural validator. JavaScript then independently
 * checks the anchor path and execution post-state projection; Commit-QC
 * signature and roster trust remain separate.
 */
function normalizeKagemushaOperationStatusCore(
  payload,
  acceptedReference,
  nativeValidatedAppliedTopUp,
) {
  const context = "Kagemusha operation status";
  // Detach both complete public inputs before reading any discriminants or
  // nested proof bytes. Validation and output then use this same stable graph.
  const detachedPayload = jsonSnapshot(record(payload, context));
  const detachedReference = jsonSnapshot(record(
    acceptedReference,
    "Accepted Kagemusha operation reference",
  ));
  const expected = normalizeAcceptedKagemushaOperationReference(detachedReference);
  const item = exactFields(detachedPayload, context, ["state", "value"]);
  const value = record(item.value, `${context}.value`);
  const operationId = normalizeKagemushaOperationId(
    value.operation_id,
    `${context}.value.operation_id`,
  );
  if (operationId !== expected.operation_id) {
    throw new TypeError(
      `${context}.value.operation_id does not match the accepted operation reference`,
    );
  }
  if (item.state === "pending") {
    exactFields(value, `${context}.value`, [
      "operation_id",
      "kind",
      "transaction_hash",
      "submitted_at_ms",
    ]);
    const kind = taggedKind(value.kind, `${context}.value.kind`);
    const transactionHash = hash32(
      value.transaction_hash,
      `${context}.value.transaction_hash`,
    );
    const submittedAtMs = losslessU64(
      value.submitted_at_ms,
      `${context}.value.submitted_at_ms`,
      { positive: true },
    );
    if (
      kind !== expected.kind.kind ||
      (transactionHash === expected.transaction_hash &&
        submittedAtMs !== expected.submitted_at_ms)
    ) {
      throw new TypeError(
        `${context}.value does not match the accepted operation reference`,
      );
    }
    return Object.freeze({
      state: "pending",
      value: Object.freeze({
        operation_id: operationId,
        kind: Object.freeze({ kind, value: null }),
        transaction_hash: transactionHash,
        submitted_at_ms: submittedAtMs,
      }),
    });
  }
  if (item.state === "applied") {
    exactFields(value, `${context}.value`, ["operation_id", "result"]);
    const taggedResult = record(value.result, `${context}.value.result`);
    if (taggedResult.kind === "top_up" && !nativeValidatedAppliedTopUp) {
      throw new TypeError(
        `${context} Applied top-up requires the native ABI-23 structural validator`,
      );
    }
    const result = normalizeAppliedResult(
      value.result,
      operationId,
      `${context}.value.result`,
    );
    if (result.kind !== expected.kind.kind) {
      throw new TypeError(
        `${context}.value.result does not match the accepted operation reference`,
      );
    }
    return Object.freeze({
      state: "applied",
      value: Object.freeze({
        operation_id: operationId,
        result,
      }),
    });
  }
  if (item.state !== "rejected") {
    throw new TypeError(`${context}.state must be pending, applied, or rejected`);
  }
  exactFields(value, `${context}.value`, [
    "operation_id",
    "kind",
    "transaction_hash",
    "error",
  ]);
  const error = record(value.error, `${context}.value.error`);
  const errorFields = Object.keys(error);
  const hasDetails = Object.prototype.hasOwnProperty.call(error, "details");
  const expectedErrorFields = hasDetails
    ? ["code", "message", "details"]
    : ["code", "message"];
  if (
    errorFields.length !== expectedErrorFields.length ||
    errorFields.some((field) => !expectedErrorFields.includes(field))
  ) {
    throw new TypeError(`${context}.value.error contains missing or unknown fields`);
  }
  const code = exactString(error.code, `${context}.value.error.code`, { maximum: 64 });
  if (!ERROR_CODE.test(code)) {
    throw new TypeError(
      `${context}.value.error.code must be a stable lowercase error code`,
    );
  }
  const normalizedError = {
    code,
    message: exactString(error.message, `${context}.value.error.message`),
  };
  if (hasDetails) {
    normalizedError.details = record(error.details, `${context}.value.error.details`);
  }
  const kind = taggedKind(value.kind, `${context}.value.kind`);
  const transactionHash = hash32(
    value.transaction_hash,
    `${context}.value.transaction_hash`,
  );
  if (kind !== expected.kind.kind) {
    throw new TypeError(
      `${context}.value does not match the accepted operation reference`,
    );
  }
  return Object.freeze({
    state: "rejected",
    value: Object.freeze({
      operation_id: operationId,
      kind: Object.freeze({ kind, value: null }),
      transaction_hash: transactionHash,
      error: Object.freeze(normalizedError),
    }),
  });
}

/**
 * Normalize a status using only the portable JavaScript boundary.
 * Applied top-ups fail closed because their complete anchor digest must be
 * recomputed by the native ABI-23 validator before projection.
 */
export function normalizeKagemushaOperationStatus(payload, acceptedReference) {
  return normalizeKagemushaOperationStatusCore(payload, acceptedReference, false);
}

/** @internal Normalize an Applied top-up only after validating its exact response bytes. */
export function _normalizeKagemushaOperationStatusWithNativeValidation(
  payload,
  acceptedReference,
  sourceBytes,
  nativeBinding,
) {
  const isAppliedTopUp =
    payload !== null &&
    typeof payload === "object" &&
    payload.state === "applied" &&
    payload.value !== null &&
    typeof payload.value === "object" &&
    payload.value.result !== null &&
    typeof payload.value.result === "object" &&
    payload.value.result.kind === "top_up";
  if (!isAppliedTopUp) {
    return normalizeKagemushaOperationStatusCore(payload, acceptedReference, false);
  }
  if (
    !ArrayBuffer.isView(sourceBytes) ||
    sourceBytes.BYTES_PER_ELEMENT !== 1 ||
    sourceBytes.byteLength === 0 ||
    sourceBytes.byteLength > KAGEMUSHA_OPERATION_STATUS_JSON_MAX_BYTES
  ) {
    throw new TypeError(
      "Kagemusha native status validation requires the exact bounded response bytes",
    );
  }
  const validator = Object.getOwnPropertyDescriptor(
    nativeBinding ?? {},
    "kagemushaOfflineOperationStatusJsonValidateV1",
  )?.value;
  if (typeof validator !== "function") {
    throw new TypeError(
      "Native binding does not expose kagemushaOfflineOperationStatusJsonValidateV1",
    );
  }
  const exactBytes = new Uint8Array(
    sourceBytes.buffer,
    sourceBytes.byteOffset,
    sourceBytes.byteLength,
  );
  validator.call(nativeBinding, exactBytes);
  return normalizeKagemushaOperationStatusCore(payload, acceptedReference, true);
}
