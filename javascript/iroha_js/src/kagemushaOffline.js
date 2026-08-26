/** Transport-only ABI-22/manifest-V4 Kagemusha projections.
 *
 * This module deliberately has no native prover or artifact-install surface.
 * Top-up and redemption methods accept opaque NRT0-framed archives produced
 * and canonically validated by a supported wallet/prover implementation.
 */

import { computeHashLiteralCrc } from "./hashLiteralCrc.js";
import { validateNoritoFrame } from "./norito.js";

export const KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION = 22;
export const KAGEMUSHA_MANIFEST_VERSION = 4;
export const KAGEMUSHA_MAX_HOPS = 8;
export const KAGEMUSHA_CASH_HANDOFF_CAPABILITY = "cash_handoff_v1";
export const KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES = 512 * 1024;
export const KAGEMUSHA_REDEEM_REQUEST_MAX_BYTES = 48 * 1024 * 1024;

const HASH_32 = /^[0-9a-f]{64}$/u;
const OPERATION_ID = /^(?!0{64}$)[0-9a-f]{64}$/u;
const NETWORK_ID = /^[0-9a-f]{64}$/u;
const NETWORK_ID_LITERAL = /^hash:([0-9A-F]{64})#([0-9A-F]{4})$/u;
const POSITIVE_DECIMAL = /^[1-9][0-9]*$/u;
const MAX_U64 = (1n << 64n) - 1n;
const NORITO_COMPACT_LENGTH_FLAG = 0x02;
const NORITO_REQUEST_PADDING_BYTES = 8;
const TOP_UP_REQUEST_SCHEMA = "iroha.torii.v1.offline.top_up.request";
const REDEEM_REQUEST_SCHEMA = "iroha.torii.v1.offline.redeem.request";
const TOP_UP_REQUEST_FIELD_COUNT = 8;
const TOP_UP_OPERATION_ID_FIELD_INDEX = 6;
const TOP_UP_CURRENT_NOTE_FIELD_INDEX = 3;
const REDEEM_REQUEST_FIELD_COUNT = 10;
const REDEEM_OPERATION_ID_FIELD_INDEX = 8;
const REDEEM_BUNDLE_FIELD_INDEX = 1;
const AUTHORIZATION_FIELD_COUNT = 10;
const AUTHORIZATION_OPERATION_ID_FIELD_INDEX = 3;
const AUTHORIZATION_ISSUED_AT_FIELD_INDEX = 4;
const CURRENT_NOTE_FIELD_COUNT = 5;
const RECURSIVE_BUNDLE_FIELD_COUNT = 3;
const RECURSIVE_STATEMENT_FIELD_INDEX = 0;
const RECURSIVE_STATEMENT_FIELD_COUNT = 13;
const EXACT_JSON_MEDIA_TYPE =
  /^[ \t]*application\/json(?:[ \t]*;[ \t]*[!#$%&'*+\-.^_`|~0-9A-Za-z]+=(?:[!#$%&'*+\-.^_`|~0-9A-Za-z]+|"(?:[ \t!#-\[\]-~\u0080-\u00ff]|\\[ \t!-~\u0080-\u00ff])*"))*[ \t]*$/iu;
const OFFLINE_STATUS_FIELDS = Object.freeze([
  "mandatory",
  "cash_handoff_capability",
  "required_bridge_abi_version",
  "max_hops",
  "ready",
  "assets",
  "blockers",
]);
const OFFLINE_CAPABILITY_ACTIVATION_BLOCKERS_V1 = Object.freeze([
  Object.freeze({
    code: "offline_cash_authenticated_release_unavailable",
    message: "No authenticated Offline Cash V1 release is selected by this asset-neutral response.",
  }),
  Object.freeze({
    code: "offline_cash_eligible_asset_unavailable",
    message: "No eligible Offline Cash V1 asset is selected by this asset-neutral response.",
  }),
  Object.freeze({
    code: "offline_cash_proof_backend_unavailable",
    message:
      "No reviewed production Offline Cash V1 proof and secure-device backend is authenticated by this response.",
  }),
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

function exactString(value, context, { maximum = 1024 } = {}) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > maximum ||
    value.trim() !== value ||
    /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    throw new TypeError(`${context} must be exact non-empty text`);
  }
  return value;
}

function rejectionMessage(value, context) {
  if (typeof value !== "string" || value.length === 0 || value.trim() !== value) {
    throw new TypeError(`${context} must be exact non-empty text`);
  }
  let scalarCount = 0;
  for (let index = 0; index < value.length; index += 1) {
    const codeUnit = value.charCodeAt(index);
    if (codeUnit <= 0x1f || (codeUnit >= 0x7f && codeUnit <= 0x9f)) {
      throw new TypeError(`${context} must not contain control characters`);
    }
    if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      const trailing = value.charCodeAt(index + 1);
      if (!(trailing >= 0xdc00 && trailing <= 0xdfff)) {
        throw new TypeError(`${context} must contain only well-formed Unicode scalars`);
      }
      index += 1;
    } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      throw new TypeError(`${context} must contain only well-formed Unicode scalars`);
    }
    scalarCount += 1;
    if (scalarCount > 1024) {
      throw new TypeError(`${context} must not exceed 1024 Unicode scalars`);
    }
  }
  return value;
}

function safeUnsigned(value, context, { positive = false, maximum = Number.MAX_SAFE_INTEGER } = {}) {
  if (!Number.isSafeInteger(value) || value < (positive ? 1 : 0) || value > maximum) {
    throw new TypeError(`${context} must be a${positive ? " positive" : "n"} safe unsigned integer`);
  }
  return value;
}

function positiveDecimalHeader(value, context) {
  if (
    typeof value !== "string" ||
    !POSITIVE_DECIMAL.test(value)
  ) {
    throw new TypeError(`${context} must be one positive decimal header value`);
  }
  let parsed;
  try {
    parsed = BigInt(value);
  } catch {
    throw new TypeError(`${context} must be one positive decimal header value`);
  }
  if (parsed > MAX_U64) {
    throw new TypeError(`${context} must fit in an unsigned 64-bit integer`);
  }
  return value;
}

function readCanonicalStructFields(value, fieldCount, context) {
  const bytes = value instanceof Uint8Array ? value : new Uint8Array(value);
  let offset = 0;

  const readCompactLength = (fieldContext) => {
    let length = 0n;
    let shift = 0n;
    for (let index = 0; index < 10; index += 1) {
      if (offset >= bytes.byteLength) {
        throw new TypeError(`${fieldContext} compact length is truncated`);
      }
      const current = bytes[offset];
      offset += 1;
      const chunk = BigInt(current & 0x7f);
      if (shift === 63n && chunk > 1n) {
        throw new TypeError(`${fieldContext} compact length overflows UInt64`);
      }
      length |= chunk << shift;
      if ((current & 0x80) === 0) {
        if (index > 0 && chunk === 0n) {
          throw new TypeError(`${fieldContext} compact length is overlong`);
        }
        return length;
      }
      shift += 7n;
    }
    throw new TypeError(`${fieldContext} compact length overflows UInt64`);
  };

  const fields = [];
  for (let index = 0; index < fieldCount; index += 1) {
    const length = readCompactLength(`${context}.fields[${index}].length`);
    const remaining = BigInt(bytes.byteLength - offset);
    if (length > remaining) {
      throw new TypeError(`${context}.fields[${index}] is truncated`);
    }
    const end = offset + Number(length);
    fields.push(bytes.subarray(offset, end));
    offset = end;
  }
  if (offset !== bytes.byteLength) {
    throw new TypeError(`${context} contains trailing or unknown bytes`);
  }
  return fields;
}

function exactNonzeroBytes32(value, context) {
  if (value.byteLength !== 32 || value.every((byte) => byte === 0)) {
    throw new TypeError(`${context} must contain exactly 32 non-zero bytes`);
  }
  return value;
}

function fixedBytesHex(value) {
  return Array.from(value, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function projectKagemushaRequestV4(payload, schema, context) {
  const topUp = schema === TOP_UP_REQUEST_SCHEMA;
  const fields = readCanonicalStructFields(
    payload,
    topUp ? TOP_UP_REQUEST_FIELD_COUNT : REDEEM_REQUEST_FIELD_COUNT,
    `${context}.norito payload`,
  );
  if (
    fields[0].byteLength !== 2 ||
    new DataView(fields[0].buffer, fields[0].byteOffset, 2).getUint16(0, true) !==
      KAGEMUSHA_MANIFEST_VERSION
  ) {
    throw new TypeError(`${context}.norito request version must be exactly 4`);
  }

  const operationIdBytes = exactNonzeroBytes32(
    fields[topUp ? TOP_UP_OPERATION_ID_FIELD_INDEX : REDEEM_OPERATION_ID_FIELD_INDEX],
    `${context}.norito request operation id`,
  );
  const authorization = readCanonicalStructFields(
    fields.at(-1),
    AUTHORIZATION_FIELD_COUNT,
    `${context}.norito authorization`,
  );
  const authorizationOperationId = exactNonzeroBytes32(
    authorization[AUTHORIZATION_OPERATION_ID_FIELD_INDEX],
    `${context}.norito authorization operation id`,
  );
  if (!operationIdBytes.every((byte, index) => byte === authorizationOperationId[index])) {
    throw new TypeError(
      `${context}.norito request and authorization operation ids must match exactly`,
    );
  }

  const issuedAtBytes = authorization[AUTHORIZATION_ISSUED_AT_FIELD_INDEX];
  if (issuedAtBytes.byteLength !== 8) {
    throw new TypeError(`${context}.norito authorization issued_at_ms must be an exact UInt64`);
  }
  const issuedAt = new DataView(
    issuedAtBytes.buffer,
    issuedAtBytes.byteOffset,
    issuedAtBytes.byteLength,
  ).getBigUint64(0, true);
  if (issuedAt === 0n) {
    throw new TypeError(`${context}.norito authorization issued_at_ms must be at least 1`);
  }
  if (issuedAt > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new TypeError(
      `${context}.norito authorization issued_at_ms must fit in a safe unsigned integer`,
    );
  }

  let networkIdBytes;
  if (topUp) {
    const currentNote = readCanonicalStructFields(
      fields[TOP_UP_CURRENT_NOTE_FIELD_INDEX],
      CURRENT_NOTE_FIELD_COUNT,
      `${context}.norito current_note`,
    );
    [networkIdBytes] = currentNote;
  } else {
    const bundle = readCanonicalStructFields(
      fields[REDEEM_BUNDLE_FIELD_INDEX],
      RECURSIVE_BUNDLE_FIELD_COUNT,
      `${context}.norito redemption bundle`,
    );
    const statement = readCanonicalStructFields(
      bundle[RECURSIVE_STATEMENT_FIELD_INDEX],
      RECURSIVE_STATEMENT_FIELD_COUNT,
      `${context}.norito redemption statement`,
    );
    [networkIdBytes] = statement;
  }
  if (
    networkIdBytes.byteLength !== 32 ||
    (networkIdBytes[networkIdBytes.byteLength - 1] & 1) === 0
  ) {
    throw new TypeError(
      `${context}.norito signed request NetworkId must contain exactly 32 marked bytes`,
    );
  }

  return Object.freeze({
    operationId: fixedBytesHex(operationIdBytes),
    issuedAtMs: Number(issuedAt),
    networkId: fixedBytesHex(networkIdBytes),
  });
}

function networkIdLiteral(value, context) {
  const match = typeof value === "string" ? NETWORK_ID_LITERAL.exec(value) : null;
  if (match === null) {
    throw new TypeError(`${context} must be a canonical marked Iroha NetworkId`);
  }
  const [, body, checksum] = match;
  if (
    computeHashLiteralCrc("hash", body) !== checksum ||
    (Number.parseInt(body.slice(-2), 16) & 1) === 0
  ) {
    throw new TypeError(`${context} must be a canonical marked Iroha NetworkId`);
  }
  return body.toLowerCase();
}

function normalizeExpectedNetworkId(value, context) {
  if (
    typeof value === "string" &&
    NETWORK_ID.test(value) &&
    (Number.parseInt(value.slice(-2), 16) & 1) === 1
  ) {
    return value;
  }
  return networkIdLiteral(value, context);
}

function hash32(value, context, { nonzero = false } = {}) {
  if (typeof value !== "string" || !HASH_32.test(value) || (nonzero && value === "0".repeat(64))) {
    throw new TypeError(`${context} must be ${nonzero ? "non-zero " : ""}lowercase 32-byte hexadecimal`);
  }
  if ((Number.parseInt(value.slice(-2), 16) & 1) === 0) {
    throw new TypeError(`${context} must have the Iroha hash marker bit set`);
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
  if (item.mandatory !== false) {
    throw new TypeError(`${context}.mandatory must be false`);
  }
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
    throw new TypeError(`${context}.required_bridge_abi_version must be 22`);
  }
  if (
    safeUnsigned(item.max_hops, `${context}.max_hops`, {
      positive: true,
      maximum: 0xffff_ffff,
    }) !== KAGEMUSHA_MAX_HOPS
  ) {
    throw new TypeError(`${context}.max_hops must be 8`);
  }
  if (item.ready !== false) {
    throw new TypeError(`${context}.ready must be false`);
  }
  if (!Array.isArray(item.assets) || item.assets.length !== 0) {
    throw new TypeError(`${context}.assets must be an empty array`);
  }
  if (
    !Array.isArray(item.blockers) ||
    item.blockers.length !== OFFLINE_CAPABILITY_ACTIVATION_BLOCKERS_V1.length
  ) {
    throw new TypeError(`${context}.blockers must contain the three canonical activation blockers`);
  }
  const blockers = item.blockers.map((value, index) => {
    const blocker = exactFields(value, `${context}.blockers[${index}]`, ["code", "message"]);
    const expected = OFFLINE_CAPABILITY_ACTIVATION_BLOCKERS_V1[index];
    const code = exactString(blocker.code, `${context}.blockers[${index}].code`);
    const message = exactString(blocker.message, `${context}.blockers[${index}].message`);
    if (code !== expected.code || message !== expected.message) {
      throw new TypeError(`${context}.blockers[${index}] is not the canonical activation blocker`);
    }
    return Object.freeze({ code, message });
  });
  return Object.freeze({
    mandatory: false,
    cash_handoff_capability: KAGEMUSHA_CASH_HANDOFF_CAPABILITY,
    required_bridge_abi_version: KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION,
    max_hops: KAGEMUSHA_MAX_HOPS,
    ready: false,
    assets: Object.freeze([]),
    blockers: Object.freeze(blockers),
  });
}

function normalizeKagemushaNoritoRequestV4(value, maximumBytes, schema, context) {
  const item = exactFields(value, context, ["version", "operationId", "norito"]);
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
  if (archive.byteLength > maximumBytes) {
    throw new TypeError(`${context}.norito exceeds the ${maximumBytes}-byte limit`);
  }
  let frame;
  try {
    frame = validateNoritoFrame(archive, {
      context: `${context}.norito`,
      expectedTypeName: schema,
      expectedPaddingLength: NORITO_REQUEST_PADDING_BYTES,
      requireNonEmptyPayload: true,
    });
    if (frame.flags !== NORITO_COMPACT_LENGTH_FLAG) {
      throw new TypeError(`${context}.norito must use canonical compact Norito framing`);
    }
  } catch (error) {
    throw new TypeError(
      `${context}.norito must be a canonical compact ${schema} Norito archive`,
      { cause: error },
    );
  }
  const projection = projectKagemushaRequestV4(frame.payload, schema, context);
  const suppliedOperationId = normalizeKagemushaOperationId(
    item.operationId,
    `${context}.operationId`,
  );
  if (suppliedOperationId !== projection.operationId) {
    throw new TypeError(`${context}.operationId must match the signed Norito request body`);
  }
  return Object.freeze({
    version: KAGEMUSHA_MANIFEST_VERSION,
    operationId: projection.operationId,
    issuedAtMs: projection.issuedAtMs,
    networkId: projection.networkId,
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
    TOP_UP_REQUEST_SCHEMA,
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
    REDEEM_REQUEST_SCHEMA,
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

export function normalizeKagemushaOperationReference(
  payload,
  { expectedOperationId, expectedKind, expectedSubmittedAtMs, location, retryAfter },
) {
  const context = "Kagemusha operation reference";
  const item = exactFields(payload, context, [
    "operation_id",
    "kind",
    "state",
    "transaction_hash",
    "status_uri",
    "submitted_at_ms",
  ]);
  const operationId = normalizeKagemushaOperationId(item.operation_id, `${context}.operation_id`);
  const kind = taggedKind(item.kind, `${context}.kind`);
  taggedPending(item.state, `${context}.state`);
  const statusUri = `/v1/offline/operations/${operationId}`;
  if (
    operationId !== expectedOperationId ||
    kind !== expectedKind ||
    item.status_uri !== statusUri ||
    location !== statusUri
  ) {
    throw new TypeError(`${context} does not match the submitted V4 command`);
  }
  positiveDecimalHeader(retryAfter, `${context} Retry-After`);
  const submittedAtMs = safeUnsigned(item.submitted_at_ms, `${context}.submitted_at_ms`, {
    positive: true,
  });
  if (
    submittedAtMs !==
    safeUnsigned(expectedSubmittedAtMs, `${context} expected submitted_at_ms`, { positive: true })
  ) {
    throw new TypeError(`${context}.submitted_at_ms does not match the signed V4 command`);
  }
  return Object.freeze({
    operation_id: operationId,
    kind: Object.freeze({ kind, value: null }),
    state: Object.freeze({ state: "pending", value: null }),
    transaction_hash: hash32(item.transaction_hash, `${context}.transaction_hash`),
    status_uri: statusUri,
    submitted_at_ms: submittedAtMs,
  });
}

function bytes32(value, context, { nonzero = false } = {}) {
  if (
    !Array.isArray(value) ||
    value.length !== 32 ||
    value.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255) ||
    (nonzero && value.every((byte) => byte === 0))
  ) {
    throw new TypeError(`${context} must be an array of 32${nonzero ? " non-zero" : ""} bytes`);
  }
  return value;
}

function bytesToHex(value) {
  return value.map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function normalizeAppliedResult(value, operationId, context, expectedNetworkId) {
  const tagged = exactFields(value, context, ["kind", "result"]);
  const result = record(tagged.result, `${context}.result`);
  if (tagged.kind === "redeem") {
    exactFields(result, `${context}.result`, [
      "transaction_hash",
      "finalized_block_height",
      "server_time_ms",
    ]);
    return Object.freeze({
      kind: "redeem",
      result: Object.freeze({
        transaction_hash: hash32(result.transaction_hash, `${context}.result.transaction_hash`),
        finalized_block_height: safeUnsigned(
          result.finalized_block_height,
          `${context}.result.finalized_block_height`,
          { positive: true },
        ),
        server_time_ms: safeUnsigned(result.server_time_ms, `${context}.result.server_time_ms`, {
          positive: true,
        }),
      }),
    });
  }
  if (tagged.kind !== "top_up") {
    throw new TypeError(`${context}.kind must be top_up or redeem`);
  }
  exactFields(result, `${context}.result`, [
    "transaction_hash",
    "finalized_block_height",
    "server_time_ms",
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
  if (bytesToHex(bytes32(anchor.topup_operation_id, `${context}.result.anchor.topup_operation_id`)) !== operationId) {
    throw new TypeError(`${context}.result.anchor.topup_operation_id does not match the operation`);
  }
  const transactionHash = hash32(result.transaction_hash, `${context}.result.transaction_hash`);
  const finalizedBlockHeight = safeUnsigned(
    result.finalized_block_height,
    `${context}.result.finalized_block_height`,
    { positive: true },
  );
  const serverTimeMs = safeUnsigned(
    result.server_time_ms,
    `${context}.result.server_time_ms`,
    { positive: true },
  );
  const anchorTransactionHash = bytesToHex(bytes32(
    anchor.finalized_tx_hash,
    `${context}.result.anchor.finalized_tx_hash`,
    { nonzero: true },
  ));
  const anchorHeight = safeUnsigned(
    anchor.finalized_height,
    `${context}.result.anchor.finalized_height`,
    { positive: true },
  );
  const anchorNetworkId = networkIdLiteral(
    anchor.network_id,
    `${context}.result.anchor.network_id`,
  );
  const currentNote = record(anchor.current_note, `${context}.result.anchor.current_note`);
  const currentNoteNetworkId = networkIdLiteral(
    currentNote.network_id,
    `${context}.result.anchor.current_note.network_id`,
  );
  const anchorDigest = bytes32(
    anchor.anchor_digest,
    `${context}.result.anchor.anchor_digest`,
    { nonzero: true },
  );
  const finalityProof = record(result.finality_proof, `${context}.result.finality_proof`);
  const finalityAnchor = record(finalityProof.anchor, `${context}.result.finality_proof.anchor`);
  const finalityAnchorDigest = bytes32(
    finalityAnchor.anchor_digest,
    `${context}.result.finality_proof.anchor.anchor_digest`,
    { nonzero: true },
  );
  const commitQc = record(finalityProof.commit_qc, `${context}.result.finality_proof.commit_qc`);
  const heightContext = record(
    commitQc.height_context,
    `${context}.result.finality_proof.commit_qc.height_context`,
  );
  const proofHeight = safeUnsigned(
    heightContext.height,
    `${context}.result.finality_proof.commit_qc.height_context.height`,
    { positive: true },
  );
  const proofNetworkId = networkIdLiteral(
    heightContext.network_id,
    `${context}.result.finality_proof.commit_qc.height_context.network_id`,
  );
  if (
    finalityProof.version !== 1 ||
    bytesToHex(bytes32(
      finalityAnchor.topup_operation_id,
      `${context}.result.finality_proof.anchor.topup_operation_id`,
    )) !== operationId ||
    transactionHash !== anchorTransactionHash ||
    finalizedBlockHeight !== anchorHeight ||
    !anchorDigest.every((byte, index) => byte === finalityAnchorDigest[index]) ||
    finalizedBlockHeight !== proofHeight ||
    anchorNetworkId !== currentNoteNetworkId ||
    anchorNetworkId !== proofNetworkId ||
    (expectedNetworkId !== null && anchorNetworkId !== expectedNetworkId)
  ) {
    throw new TypeError(
      `${context}.result does not bind the V4 top-up operation, transaction, height, network, and proof`,
    );
  }
  return Object.freeze({
    kind: "top_up",
    result: Object.freeze({
      transaction_hash: transactionHash,
      finalized_block_height: finalizedBlockHeight,
      server_time_ms: serverTimeMs,
      anchor: jsonSnapshot(anchor),
      finality_proof: jsonSnapshot(finalityProof),
    }),
  });
}

export function normalizeKagemushaOperationStatus(
  payload,
  expectedOperationId,
  { expectedNetworkId = null } = {},
) {
  const context = "Kagemusha operation status";
  const expected = normalizeKagemushaOperationId(expectedOperationId, "expected operation id");
  const expectedNetwork = expectedNetworkId === null
    ? null
    : normalizeExpectedNetworkId(expectedNetworkId, "expected network id");
  const item = exactFields(payload, context, ["state", "value"]);
  const value = record(item.value, `${context}.value`);
  const operationId = normalizeKagemushaOperationId(
    value.operation_id,
    `${context}.value.operation_id`,
  );
  if (operationId !== expected) {
    throw new TypeError(`${context}.value.operation_id does not match the requested operation`);
  }
  if (item.state === "pending") {
    exactFields(value, `${context}.value`, [
      "operation_id",
      "kind",
      "transaction_hash",
      "submitted_at_ms",
    ]);
    return Object.freeze({
      state: "pending",
      value: Object.freeze({
        operation_id: operationId,
        kind: Object.freeze({
          kind: taggedKind(value.kind, `${context}.value.kind`),
          value: null,
        }),
        transaction_hash: hash32(value.transaction_hash, `${context}.value.transaction_hash`),
        submitted_at_ms: safeUnsigned(value.submitted_at_ms, `${context}.value.submitted_at_ms`, {
          positive: true,
        }),
      }),
    });
  }
  if (item.state === "applied") {
    exactFields(value, `${context}.value`, ["operation_id", "result"]);
    return Object.freeze({
      state: "applied",
      value: Object.freeze({
        operation_id: operationId,
        result: normalizeAppliedResult(
          value.result,
          operationId,
          `${context}.value.result`,
          expectedNetwork,
        ),
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
  const error = exactFields(value.error, `${context}.value.error`, ["code", "message"]);
  if (error.code !== "offline_operation_rejected") {
    throw new TypeError(`${context}.value.error.code must be offline_operation_rejected`);
  }
  return Object.freeze({
    state: "rejected",
    value: Object.freeze({
      operation_id: operationId,
      kind: Object.freeze({
        kind: taggedKind(value.kind, `${context}.value.kind`),
        value: null,
      }),
      transaction_hash: hash32(value.transaction_hash, `${context}.value.transaction_hash`),
      error: Object.freeze({
        code: "offline_operation_rejected",
        message: rejectionMessage(error.message, `${context}.value.error.message`),
      }),
    }),
  });
}
