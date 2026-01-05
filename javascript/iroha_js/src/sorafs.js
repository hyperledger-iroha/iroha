import { getNativeBinding } from "./native.js";

const NORITO_HEADER_LENGTH = 40;
const MAX_SAFE_NORITO_LENGTH = BigInt(Number.MAX_SAFE_INTEGER);
const utf8Decoder =
  typeof TextDecoder === "function" ? new TextDecoder("utf-8", { fatal: true }) : null;

function isPlainObject(value) {
  if (value === null || typeof value !== "object") {
    return false;
  }
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
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
  throw new TypeError("bytes must be a Buffer, ArrayBuffer, or typed array");
}

function requireSorafsNativeBinding() {
  const binding = getNativeBinding();
  if (
    !binding ||
    typeof binding.sorafsDecodeReplicationOrder !== "function"
  ) {
    throw new Error(
      "SoraFS decoding requires the native iroha_js_host module. Run `npm run build:native` before using these helpers.",
    );
  }
  return binding;
}

function formatAssignment(raw) {
  if (!raw || typeof raw !== "object") {
    return {
      providerIdHex: "",
      sliceGiB: 0,
      lane: null,
    };
  }
  const providerIdHex =
    typeof raw.provider_id_hex === "string" ? raw.provider_id_hex : "";
  const sliceValue = raw.slice_gib;
  return {
    providerIdHex,
    sliceGiB: typeof sliceValue === "number" ? sliceValue : Number(sliceValue ?? 0),
    lane:
      typeof raw.lane === "string"
        ? raw.lane
        : raw.lane === null || raw.lane === undefined
        ? null
        : String(raw.lane),
  };
}

function formatMetadata(raw) {
  if (!raw || typeof raw !== "object") {
    return { key: "", value: "" };
  }
  return {
    key: typeof raw.key === "string" ? raw.key : String(raw.key ?? ""),
    value: typeof raw.value === "string" ? raw.value : String(raw.value ?? ""),
  };
}

function formatSla(raw) {
  if (!raw || typeof raw !== "object") {
    return {
      ingestDeadlineSecs: 0,
      minAvailabilityPercentMilli: 0,
      minPorSuccessPercentMilli: 0,
    };
  }
  const ingest = raw.ingest_deadline_secs;
  const availability = raw.min_availability_percent_milli;
  const por = raw.min_por_success_percent_milli;
  return {
    ingestDeadlineSecs: Number(ingest ?? 0),
    minAvailabilityPercentMilli: Number(availability ?? 0),
    minPorSuccessPercentMilli: Number(por ?? 0),
  };
}

export class SorafsGatewayFetchError extends Error {
  /**
   * @param {Record<string, any>} [payload]
   * @param {Error | null} [original]
   */
  constructor(payload = {}, original = null) {
    const message =
      (payload && typeof payload.message === "string" && payload.message) ||
      original?.message ||
      "SoraFS gateway fetch failed";
    super(message);
    this.name = "SorafsGatewayFetchError";
    this.kind = typeof payload.kind === "string" ? payload.kind : "multi_source";
    this.code = typeof payload.code === "string" ? payload.code : "unknown";
    this.retryable = Boolean(payload.retryable);
    this.chunkIndex =
      typeof payload.chunkIndex === "number" ? payload.chunkIndex : null;
    this.attempts =
      typeof payload.attempts === "number" ? payload.attempts : null;
    this.lastError =
      payload && typeof payload.lastError === "object" ? payload.lastError : null;
    this.providers = Array.isArray(payload.providers) ? payload.providers : null;
    this.details =
      payload && typeof payload.details === "object" ? payload.details : null;
    this.observerError =
      payload && typeof payload.observerError === "string"
        ? payload.observerError
        : null;
    this.original = original ?? null;
    this.payload = payload || {};
  }
}

/**
 * Decode a Norito-encoded replication order into a structured object.
 * @param {ArrayBufferView | ArrayBuffer | Buffer} bytes
 * @returns {{
 *   schemaVersion: number,
 *   orderIdHex: string,
 *   manifestCidUtf8: string | null,
 *   manifestCidBase64: string,
 *   manifestDigestHex: string,
 *   chunkingProfile: string,
 *   targetReplicas: number,
 *   assignments: Array<{ providerIdHex: string, sliceGiB: number, lane: string | null }>,
 *   issuedAtUnix: number,
 *   deadlineAtUnix: number,
 *   sla: { ingestDeadlineSecs: number, minAvailabilityPercentMilli: number, minPorSuccessPercentMilli: number },
 *   metadata: Array<{ key: string, value: string }>
 * }}
 */
export function decodeReplicationOrder(bytes) {
  const buffer = toBuffer(bytes);
  const binding = getNativeBinding();
  let payload;
  if (binding && typeof binding.sorafsDecodeReplicationOrder === "function") {
    payload = binding.sorafsDecodeReplicationOrder(buffer);
  } else {
    payload = decodeReplicationOrderFallback(buffer);
  }
  const assignments = Array.isArray(payload.assignments)
    ? payload.assignments.map((entry) => formatAssignment(entry))
    : [];
  const metadata = Array.isArray(payload.metadata)
    ? payload.metadata.map((entry) => formatMetadata(entry))
    : [];
  const schemaVersion = Number(payload.schema_version ?? 0);
  const orderIdHex =
    typeof payload.order_id_hex === "string" ? payload.order_id_hex : "";
  const manifestCidUtf8 =
    payload.manifest_cid_utf8 === null
      ? null
      : typeof payload.manifest_cid_utf8 === "string"
      ? payload.manifest_cid_utf8
      : null;
  const manifestCidBase64 =
    typeof payload.manifest_cid_base64 === "string"
      ? payload.manifest_cid_base64
      : "";
  const manifestDigestHex =
    typeof payload.manifest_digest_hex === "string"
      ? payload.manifest_digest_hex
      : "";
  const chunkingProfile =
    typeof payload.chunking_profile === "string"
      ? payload.chunking_profile
      : "";
  const targetReplicas = payload.target_replicas ?? 0;
  const issuedAtUnix = payload.issued_at_unix ?? 0;
  const deadlineAtUnix = payload.deadline_at_unix ?? 0;
  const sla = formatSla(payload.sla);
  return {
    schemaVersion,
    orderIdHex,
    manifestCidUtf8,
    manifestCidBase64,
    manifestDigestHex,
    chunkingProfile,
    targetReplicas: Number(targetReplicas ?? 0),
    assignments,
    issuedAtUnix: Number(issuedAtUnix ?? 0),
    deadlineAtUnix: Number(deadlineAtUnix ?? 0),
    sla,
    metadata,
  };
}

function decodeReplicationOrderFallback(buffer) {
  try {
    if (buffer.length < NORITO_HEADER_LENGTH) {
      throw new Error("replication order payload is too small");
    }
    const magic = buffer.toString("ascii", 0, 4);
    if (magic !== "NRT0") {
      throw new Error("replication order payload is missing the Norito header");
    }
    const compression = buffer[22];
    if (compression !== 0) {
      throw new Error("compressed replication orders require the native decoder");
    }
    const declaredLength = buffer.readBigUInt64LE(23);
    if (declaredLength > MAX_SAFE_NORITO_LENGTH) {
      throw new Error("replication order payload is too large to decode in pure JS");
    }
    const payloadLength = Number(declaredLength);
    const available = buffer.length - NORITO_HEADER_LENGTH;
    if (available < payloadLength) {
      throw new Error("replication order payload is truncated");
    }
    const payload = buffer.slice(NORITO_HEADER_LENGTH, NORITO_HEADER_LENGTH + payloadLength);
    const reader = new NoritoChunkReader(payload, "replication order");
    const schemaVersion = readU8Chunk(reader.readChunk("schema_version"), "schema_version");
    const orderIdHex = toHex(reader.readChunk("order_id"), 32, "order_id_hex");
    const manifestCidBytes = readByteVector(reader.readChunk("manifest_cid"), "manifest_cid");
    const manifestDigestHex = toHex(
      reader.readChunk("manifest_digest"),
      32,
      "manifest_digest_hex",
    );
    const chunkingProfile = readNoritoString(reader.readChunk("chunking_profile"));
    const targetReplicas = readU16(reader.readChunk("target_replicas"), "target_replicas");
    const assignments = readSequence(
      reader.readChunk("assignments"),
      (entry, index) => parseAssignment(entry, index),
      "assignments",
    );
    const issuedAtUnix = readU64(reader.readChunk("issued_at"), "issued_at_unix");
    const deadlineAtUnix = readU64(reader.readChunk("deadline_at"), "deadline_at_unix");
    const sla = parseReplicationSla(reader.readChunk("sla"));
    const metadata = readSequence(
      reader.readChunk("metadata"),
      (entry, index) => parseMetadata(entry, index),
      "metadata",
    );
    reader.ensureFullyConsumed();
    return {
      schema_version: schemaVersion,
      order_id_hex: orderIdHex,
      manifest_cid_utf8: decodeUtf8Maybe(manifestCidBytes),
      manifest_cid_base64: manifestCidBytes.toString("base64"),
      manifest_digest_hex: manifestDigestHex,
      chunking_profile: chunkingProfile,
      target_replicas: targetReplicas,
      assignments,
      issued_at_unix: issuedAtUnix,
      deadline_at_unix: deadlineAtUnix,
      sla,
      metadata,
    };
  } catch (error) {
    const message =
      error && typeof error.message === "string"
        ? `failed to decode SoraFS replication order: ${error.message}`
        : "failed to decode SoraFS replication order";
    const wrapped = new Error(message);
    if (error instanceof Error) {
      wrapped.cause = error;
    }
    throw wrapped;
  }
}

class NoritoChunkReader {
  constructor(buffer, label = "chunk") {
    this.buffer = buffer;
    this.offset = 0;
    this.label = label;
  }

  readChunk(context = this.label) {
    if (this.offset + 8 > this.buffer.length) {
      throw new Error(`${context} is truncated`);
    }
    const length = Number(this.buffer.readBigUInt64LE(this.offset));
    this.offset += 8;
    if (length < 0 || this.offset + length > this.buffer.length) {
      throw new Error(`${context} is truncated`);
    }
    const slice = this.buffer.slice(this.offset, this.offset + length);
    this.offset += length;
    return slice;
  }

  ensureFullyConsumed(context = this.label) {
    if (this.offset !== this.buffer.length) {
      throw new Error(`${context} contains trailing bytes`);
    }
  }
}

function readSequence(chunk, parser, label) {
  if (chunk.length < 8) {
    throw new Error(`${label} is truncated`);
  }
  const length = chunk.readBigUInt64LE(0);
  if (length > MAX_SAFE_NORITO_LENGTH) {
    throw new Error(`${label} contains too many entries`);
  }
  const count = Number(length);
  const reader = new NoritoChunkReader(chunk.slice(8), label);
  const result = [];
  for (let index = 0; index < count; index += 1) {
    result.push(parser(reader.readChunk(`${label}[${index}]`), index));
  }
  reader.ensureFullyConsumed(label);
  return result;
}

function readByteVector(chunk, label) {
  const bytes = readSequence(
    chunk,
    (entry, index) => {
      if (entry.length !== 1) {
        throw new Error(`${label}[${index}] must be a byte`);
      }
      return entry[0];
    },
    label,
  );
  return Buffer.from(bytes);
}

function readNoritoString(chunk) {
  if (chunk.length < 8) {
    throw new Error("string chunk is truncated");
  }
  const declared = chunk.readBigUInt64LE(0);
  if (declared > MAX_SAFE_NORITO_LENGTH) {
    throw new Error("string value exceeds safe length");
  }
  const length = Number(declared);
  const start = 8;
  const end = start + length;
  if (end > chunk.length) {
    throw new Error("string chunk is truncated");
  }
  if (end !== chunk.length) {
    throw new Error("string chunk contains trailing bytes");
  }
  return chunk.slice(start, end).toString("utf8");
}

function readU8Chunk(chunk, label) {
  if (chunk.length !== 1) {
    throw new Error(`${label} must be one byte`);
  }
  return chunk[0];
}

function readU16(chunk, label) {
  if (chunk.length !== 2) {
    throw new Error(`${label} must be two bytes`);
  }
  return chunk.readUInt16LE(0);
}

function readU32(chunk, label) {
  if (chunk.length !== 4) {
    throw new Error(`${label} must be four bytes`);
  }
  return chunk.readUInt32LE(0);
}

function readU64(chunk, label) {
  if (chunk.length !== 8) {
    throw new Error(`${label} must be eight bytes`);
  }
  const value = chunk.readBigUInt64LE(0);
  if (value > MAX_SAFE_NORITO_LENGTH) {
    throw new Error(`${label} exceeds the safe integer range`);
  }
  return Number(value);
}

function parseAssignment(chunk, index) {
  const reader = new NoritoChunkReader(chunk, `assignments[${index}]`);
  const providerHex = toHex(reader.readChunk(`assignments[${index}].provider_id_hex`), 32);
  const sliceGiB = readU64(reader.readChunk(`assignments[${index}].slice_gib`), "slice_gib");
  const laneChunk = reader.readChunk(`assignments[${index}].lane`);
  const lane = parseOptionalString(laneChunk, `assignments[${index}].lane`);
  reader.ensureFullyConsumed(`assignments[${index}]`);
  return {
    provider_id_hex: providerHex,
    slice_gib: sliceGiB,
    lane,
  };
}

function parseOptionalString(chunk, label) {
  if (chunk.length === 0) {
    return null;
  }
  const discriminant = chunk[0];
  if (discriminant === 0) {
    return null;
  }
  if (chunk.length < 17) {
    throw new Error(`${label} is truncated`);
  }
  const payloadLength = chunk.readBigUInt64LE(1);
  let offset = 9;
  if (payloadLength > MAX_SAFE_NORITO_LENGTH) {
    throw new Error(`${label} payload length exceeds safe range`);
  }
  const declared = Number(payloadLength);
  const stringLength = chunk.readBigUInt64LE(offset);
  offset += 8;
  if (stringLength > MAX_SAFE_NORITO_LENGTH) {
    throw new Error(`${label} string is too long`);
  }
  const lengthNumber = Number(stringLength);
  const end = offset + lengthNumber;
  if (end > chunk.length) {
    throw new Error(`${label} string is truncated`);
  }
  const text = chunk.slice(offset, end).toString("utf8");
  const expected = lengthNumber + 8;
  if (declared !== expected) {
    throw new Error(`${label} length mismatch`);
  }
  return text;
}

function parseReplicationSla(chunk) {
  const reader = new NoritoChunkReader(chunk, "sla");
  const ingestDeadlineSecs = readU32(reader.readChunk("sla.ingestDeadlineSecs"), "sla.ingestDeadlineSecs");
  const minAvailabilityPercentMilli = readU32(
    reader.readChunk("sla.minAvailabilityPercentMilli"),
    "sla.minAvailabilityPercentMilli",
  );
  const minPorSuccessPercentMilli = readU32(
    reader.readChunk("sla.minPorSuccessPercentMilli"),
    "sla.minPorSuccessPercentMilli",
  );
  reader.ensureFullyConsumed("sla");
  return {
    ingest_deadline_secs: ingestDeadlineSecs,
    min_availability_percent_milli: minAvailabilityPercentMilli,
    min_por_success_percent_milli: minPorSuccessPercentMilli,
  };
}

function parseMetadata(chunk, index) {
  const reader = new NoritoChunkReader(chunk, `metadata[${index}]`);
  const key = readNoritoString(reader.readChunk(`metadata[${index}].key`));
  const value = readNoritoString(reader.readChunk(`metadata[${index}].value`));
  reader.ensureFullyConsumed(`metadata[${index}]`);
  return { key, value };
}

function toHex(bytes, expectedLength, label) {
  if (bytes.length !== expectedLength) {
    throw new Error(`${label} must contain ${expectedLength} bytes`);
  }
  return bytes.toString("hex");
}

function decodeUtf8Maybe(bytes) {
  if (bytes.length === 0) {
    return "";
  }
  if (utf8Decoder) {
    try {
      return utf8Decoder.decode(bytes);
    } catch {
      return null;
    }
  }
  const text = bytes.toString("utf8");
  return Buffer.from(text, "utf8").equals(bytes) ? text : null;
}

function assertNonEmptyString(value, label) {
  if (typeof value !== "string" || value.trim() === "") {
    throw new TypeError(`${label} must be a non-empty string`);
  }
  return value.trim();
}

function assertPositiveIntegerLike(value, label) {
  if (typeof value === "bigint") {
    if (value <= 0n) {
      throw new TypeError(`${label} must be greater than zero`);
    }
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value) || Number.isNaN(value)) {
      throw new TypeError(`${label} must be a finite number`);
    }
    const coerced = Math.trunc(value);
    if (coerced <= 0) {
      throw new TypeError(`${label} must be greater than zero`);
    }
    if (coerced !== value) {
      throw new TypeError(`${label} must be an integer`);
    }
    return coerced;
  }
  throw new TypeError(`${label} must be a positive integer`);
}

function assertNonNegativeIntegerLike(value, label) {
  if (typeof value === "bigint") {
    if (value < 0n) {
      throw new TypeError(`${label} must be a non-negative integer`);
    }
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value) || Number.isNaN(value)) {
      throw new TypeError(`${label} must be a finite number`);
    }
    const coerced = Math.trunc(value);
    if (coerced < 0) {
      throw new TypeError(`${label} must be a non-negative integer`);
    }
    return coerced;
  }
  throw new TypeError(`${label} must be a non-negative integer`);
}

function toSafeNumber(value) {
  if (typeof value === "bigint") {
    if (value <= BigInt(Number.MAX_SAFE_INTEGER) && value >= BigInt(Number.MIN_SAFE_INTEGER)) {
      return Number(value);
    }
    return value;
  }
  return value;
}

function normaliseGatewayProvider(spec) {
  if (spec == null || typeof spec !== "object") {
    throw new TypeError("provider specification must be an object");
  }
  const name = assertNonEmptyString(spec.name, "provider.name");
  const providerIdHex = assertNonEmptyString(spec.providerIdHex, "provider.providerIdHex");
  if (providerIdHex.length !== 64) {
    throw new TypeError("provider.providerIdHex must be a 32-byte hex string");
  }
  const baseUrl = assertNonEmptyString(spec.baseUrl, "provider.baseUrl");
  const streamTokenB64 = assertNonEmptyString(spec.streamTokenB64, "provider.streamTokenB64");
  const native = {
    name,
    provider_id_hex: providerIdHex.toLowerCase(),
    base_url: baseUrl,
    stream_token_b64: streamTokenB64,
  };
  if (typeof spec.privacyEventsUrl === "string" && spec.privacyEventsUrl.trim() !== "") {
    native.privacy_events_url = spec.privacyEventsUrl.trim();
  }
  return native;
}

function normaliseLocalProxyOptions(options) {
  if (options == null) {
    return undefined;
  }
  if (typeof options !== "object") {
    throw new TypeError("localProxy options must be an object");
  }
  const result = {};
  if (typeof options.bindAddr === "string" && options.bindAddr.trim() !== "") {
    result.bind_addr = options.bindAddr.trim();
  }
  if (typeof options.telemetryLabel === "string" && options.telemetryLabel.trim() !== "") {
    result.telemetry_label = options.telemetryLabel.trim();
  }
  if (typeof options.guardCacheKeyHex === "string" && options.guardCacheKeyHex.trim() !== "") {
    result.guard_cache_key_hex = options.guardCacheKeyHex.trim();
  }
  if (typeof options.emitBrowserManifest === "boolean") {
    result.emit_browser_manifest = options.emitBrowserManifest;
  }
  if (typeof options.proxyMode === "string" && options.proxyMode.trim() !== "") {
    result.proxy_mode = options.proxyMode.trim();
  }
  if (typeof options.prewarmCircuits === "boolean") {
    result.prewarm_circuits = options.prewarmCircuits;
  }
  if (typeof options.maxStreamsPerCircuit === "number") {
    result.max_streams_per_circuit = options.maxStreamsPerCircuit;
  }
  if (typeof options.circuitTtlHintSecs === "number") {
    result.circuit_ttl_hint_secs = options.circuitTtlHintSecs;
  }
  if (options.noritoBridge) {
    const bridge = options.noritoBridge;
    const spoolDir = assertNonEmptyString(bridge.spoolDir, "localProxy.noritoBridge.spoolDir");
    result.norito_bridge = { spool_dir: spoolDir };
    if (typeof bridge.extension === "string" && bridge.extension.trim() !== "") {
      result.norito_bridge.extension = bridge.extension.trim();
    }
  }
  if (options.carBridge) {
    const bridge = options.carBridge;
    const cacheDir = assertNonEmptyString(bridge.cacheDir, "localProxy.carBridge.cacheDir");
    result.car_bridge = { cache_dir: cacheDir };
    if (typeof bridge.extension === "string" && bridge.extension.trim() !== "") {
      result.car_bridge.extension = bridge.extension.trim();
    }
    if (typeof bridge.allowZst === "boolean") {
      result.car_bridge.allow_zst = bridge.allowZst;
    }
  }
  if (options.kaigiBridge) {
    const bridge = options.kaigiBridge;
    const spoolDir = assertNonEmptyString(bridge.spoolDir, "localProxy.kaigiBridge.spoolDir");
    result.kaigi_bridge = { spool_dir: spoolDir };
    if (typeof bridge.extension === "string" && bridge.extension.trim() !== "") {
      result.kaigi_bridge.extension = bridge.extension.trim();
    }
    if (typeof bridge.roomPolicy === "string" && bridge.roomPolicy.trim() !== "") {
      const normalized = bridge.roomPolicy.trim().toLowerCase();
      if (normalized !== "public" && normalized !== "authenticated") {
        throw new TypeError(
          "localProxy.kaigiBridge.roomPolicy must be `public` or `authenticated`",
        );
      }
      result.kaigi_bridge.room_policy = normalized;
    }
  }
  return result;
}

function deriveScoreboardTelemetryLabel(options) {
  if (
    typeof options.scoreboardTelemetryLabel === "string" &&
    options.scoreboardTelemetryLabel.trim() !== ""
  ) {
    return options.scoreboardTelemetryLabel.trim();
  }
  if (typeof options.telemetryRegion === "string" && options.telemetryRegion.trim() !== "") {
    return `region:${options.telemetryRegion.trim()}`;
  }
  if (typeof options.clientId === "string" && options.clientId.trim() !== "") {
    return `client:${options.clientId.trim()}`;
  }
  return "sdk:js";
}

function normaliseGatewayOptions(options = {}) {
  if (options == null) {
    return undefined;
  }
  if (typeof options !== "object") {
    throw new TypeError("options must be an object");
  }
  const native = {};
  if (typeof options.manifestEnvelopeB64 === "string" && options.manifestEnvelopeB64.trim() !== "") {
    native.manifest_envelope_b64 = options.manifestEnvelopeB64.trim();
  }
  if (typeof options.manifestCidHex === "string" && options.manifestCidHex.trim() !== "") {
    native.manifest_cid_hex = options.manifestCidHex.trim().toLowerCase();
  }
  if (typeof options.clientId === "string" && options.clientId.trim() !== "") {
    native.client_id = options.clientId.trim();
  }
  if (typeof options.telemetryRegion === "string" && options.telemetryRegion.trim() !== "") {
    native.telemetry_region = options.telemetryRegion.trim();
  }
  if (typeof options.rolloutPhase === "string" && options.rolloutPhase.trim() !== "") {
    native.rollout_phase = options.rolloutPhase.trim();
  }
  if (typeof options.maxPeers === "number") {
    native.max_peers = Math.max(1, Math.trunc(options.maxPeers));
  }
  if (options.retryBudget !== undefined && options.retryBudget !== null) {
    if (typeof options.retryBudget !== "number" || Number.isNaN(options.retryBudget)) {
      throw new TypeError("retryBudget must be a non-negative integer");
    }
    if (!Number.isFinite(options.retryBudget)) {
      throw new TypeError("retryBudget must be a finite number");
    }
    const coerced = Math.trunc(options.retryBudget);
    if (coerced < 0) {
      throw new TypeError("retryBudget must be a non-negative integer");
    }
    native.retry_budget = coerced;
  }
  if (typeof options.transportPolicy === "string" && options.transportPolicy.trim() !== "") {
    native.transport_policy = options.transportPolicy.trim();
  }
  if (typeof options.anonymityPolicy === "string" && options.anonymityPolicy.trim() !== "") {
    native.anonymity_policy = options.anonymityPolicy.trim();
  }
  if (typeof options.writeMode === "string" && options.writeMode.trim() !== "") {
    native.write_mode = options.writeMode.trim();
  }
  if (
    options.policyOverride != null &&
    typeof options.policyOverride === "object"
  ) {
    const override = {};
    const { transportPolicy, anonymityPolicy } = options.policyOverride;
    if (typeof transportPolicy === "string" && transportPolicy.trim() !== "") {
      override.transport_policy = transportPolicy.trim();
    }
    if (typeof anonymityPolicy === "string" && anonymityPolicy.trim() !== "") {
      override.anonymity_policy = anonymityPolicy.trim();
    }
    if (Object.keys(override).length > 0) {
      native.policy_override = override;
    }
  }
  const proxyOptions = normaliseLocalProxyOptions(options.localProxy);
  if (proxyOptions) {
    native.local_proxy = proxyOptions;
  }
  if (options.taikaiCache != null) {
    native.taikai_cache = normaliseTaikaiCacheOptions(options.taikaiCache);
  }
  let scoreboardOutRequested = false;
  if (typeof options.scoreboardOutPath === "string" && options.scoreboardOutPath.trim() !== "") {
    native.scoreboard_out_path = options.scoreboardOutPath.trim();
    scoreboardOutRequested = true;
  }
  if (
    options.scoreboardNowUnixSecs !== undefined &&
    options.scoreboardNowUnixSecs !== null
  ) {
    native.scoreboard_now_unix_secs = assertNonNegativeIntegerLike(
      options.scoreboardNowUnixSecs,
      "scoreboardNowUnixSecs",
    );
  }
  if (
    typeof options.scoreboardTelemetryLabel === "string" &&
    options.scoreboardTelemetryLabel.trim() !== ""
  ) {
    native.scoreboard_telemetry_label = options.scoreboardTelemetryLabel.trim();
  } else if (scoreboardOutRequested) {
    native.scoreboard_telemetry_label = deriveScoreboardTelemetryLabel(options);
  }
  if (typeof options.scoreboardAllowImplicitMetadata === "boolean") {
    native.scoreboard_allow_implicit_metadata = options.scoreboardAllowImplicitMetadata;
  }
  if (typeof options.allowSingleSourceFallback === "boolean") {
    native.allow_single_source_fallback = options.allowSingleSourceFallback;
  }
  return native;
}

function normaliseTaikaiCacheOptions(cache) {
  if (!cache || typeof cache !== "object") {
    throw new TypeError("taikaiCache must be an object");
  }
  if (!cache.qos || typeof cache.qos !== "object") {
    throw new TypeError("taikaiCache.qos must be an object with rate fields");
  }
  const qos = cache.qos;
  const burstValue = assertPositiveIntegerLike(qos.burstMultiplier, "taikaiCache.qos.burstMultiplier");
  const burst =
    typeof burstValue === "bigint"
      ? Number(burstValue)
      : burstValue;
  if (!Number.isSafeInteger(burst) || burst > 0xffffffff) {
    throw new TypeError("taikaiCache.qos.burstMultiplier must fit within a 32-bit unsigned integer");
  }

  const result = {
    hot_capacity_bytes: assertPositiveIntegerLike(
      cache.hotCapacityBytes,
      "taikaiCache.hotCapacityBytes",
    ),
    hot_retention_secs: assertPositiveIntegerLike(
      cache.hotRetentionSecs,
      "taikaiCache.hotRetentionSecs",
    ),
    warm_capacity_bytes: assertPositiveIntegerLike(
      cache.warmCapacityBytes,
      "taikaiCache.warmCapacityBytes",
    ),
    warm_retention_secs: assertPositiveIntegerLike(
      cache.warmRetentionSecs,
      "taikaiCache.warmRetentionSecs",
    ),
    cold_capacity_bytes: assertPositiveIntegerLike(
      cache.coldCapacityBytes,
      "taikaiCache.coldCapacityBytes",
    ),
    cold_retention_secs: assertPositiveIntegerLike(
      cache.coldRetentionSecs,
      "taikaiCache.coldRetentionSecs",
    ),
    qos: {
      priority_rate_bps: assertPositiveIntegerLike(
        qos.priorityRateBps,
        "taikaiCache.qos.priorityRateBps",
      ),
      standard_rate_bps: assertPositiveIntegerLike(
        qos.standardRateBps,
        "taikaiCache.qos.standardRateBps",
      ),
      bulk_rate_bps: assertPositiveIntegerLike(
        qos.bulkRateBps,
        "taikaiCache.qos.bulkRateBps",
      ),
      burst_multiplier: burst,
    },
  };

  if (cache.reliability != null) {
    const reliability = {};
    if (
      cache.reliability.failuresToTrip !== undefined &&
      cache.reliability.failuresToTrip !== null
    ) {
      reliability.failures_to_trip = assertPositiveIntegerLike(
        cache.reliability.failuresToTrip,
        "taikaiCache.reliability.failuresToTrip",
      );
    }
    if (
      cache.reliability.openSecs !== undefined &&
      cache.reliability.openSecs !== null
    ) {
      reliability.open_secs = assertPositiveIntegerLike(
        cache.reliability.openSecs,
        "taikaiCache.reliability.openSecs",
      );
    }
    result.reliability = reliability;
  }

  return result;
}

function transformProviderReport(report) {
  return {
    provider: report.provider,
    successes: report.successes,
    failures: report.failures,
    disabled: report.disabled,
  };
}

function transformChunkReceipt(receipt) {
  return {
    chunkIndex: receipt.chunk_index,
    provider: receipt.provider,
    attempts: receipt.attempts,
    latencyMs: receipt.latency_ms,
    bytes: receipt.bytes,
  };
}

function transformCarVerification(raw) {
  if (!raw) {
    return null;
  }
  return {
    manifestDigestHex: raw.manifest_digest_hex,
    manifestPayloadDigestHex: raw.manifest_payload_digest_hex,
    manifestCarDigestHex: raw.manifest_car_digest_hex,
    manifestContentLength: toSafeNumber(raw.manifest_content_length),
    manifestChunkCount: toSafeNumber(raw.manifest_chunk_count),
    manifestChunkProfileHandle: raw.manifest_chunk_profile_handle,
    manifestGovernance: {
      councilSignatures: Array.isArray(raw.manifest_governance?.council_signatures)
        ? raw.manifest_governance.council_signatures.map((entry) => ({
            signerHex: entry.signer_hex,
            signatureHex: entry.signature_hex,
          }))
        : [],
    },
    carArchive: {
      size: toSafeNumber(raw.car_archive.size),
      payloadDigestHex: raw.car_archive.payload_digest_hex,
      archiveDigestHex: raw.car_archive.archive_digest_hex,
      cidHex: raw.car_archive.cid_hex,
      rootCidsHex: Array.isArray(raw.car_archive.root_cids_hex)
        ? raw.car_archive.root_cids_hex.slice()
        : [],
      verified: Boolean(raw.car_archive.verified),
      porLeafCount: toSafeNumber(raw.car_archive.por_leaf_count),
    },
  };
}

function normaliseQosCounts(raw) {
  if (!raw || typeof raw !== "object") {
    return { priority: 0, standard: 0, bulk: 0 };
  }
  return {
    priority: toSafeNumber(raw.priority ?? 0),
    standard: toSafeNumber(raw.standard ?? 0),
    bulk: toSafeNumber(raw.bulk ?? 0),
  };
}

function normaliseTierCounts(raw) {
  if (!raw || typeof raw !== "object") {
    return { hot: 0, warm: 0, cold: 0 };
  }
  return {
    hot: toSafeNumber(raw.hot ?? 0),
    warm: toSafeNumber(raw.warm ?? 0),
    cold: toSafeNumber(raw.cold ?? 0),
  };
}

function normaliseEvictionCounts(raw) {
  if (!raw || typeof raw !== "object") {
    return { expired: 0, capacity: 0 };
  }
  return {
    expired: toSafeNumber(raw.expired ?? 0),
    capacity: toSafeNumber(raw.capacity ?? 0),
  };
}

function transformTaikaiCacheSummary(raw) {
  if (!raw || typeof raw !== "object") {
    return null;
  }
  const evictions = raw.evictions || {};
  const promotions = raw.promotions || {};
  return {
    hits: normaliseTierCounts(raw.hits),
    misses: toSafeNumber(raw.misses ?? 0),
    inserts: normaliseTierCounts(raw.inserts),
    evictions: {
      hot: normaliseEvictionCounts(evictions.hot),
      warm: normaliseEvictionCounts(evictions.warm),
      cold: normaliseEvictionCounts(evictions.cold),
    },
    promotions: {
      warmToHot: toSafeNumber(promotions.warm_to_hot ?? 0),
      coldToWarm: toSafeNumber(promotions.cold_to_warm ?? 0),
      coldToHot: toSafeNumber(promotions.cold_to_hot ?? 0),
    },
    qosDenials: normaliseQosCounts(raw.qos_denials),
  };
}

function transformTaikaiCacheQueue(raw) {
  if (!raw || typeof raw !== "object") {
    return null;
  }
  return {
    pendingSegments: toSafeNumber(raw.pending_segments ?? 0),
    pendingBytes: toSafeNumber(raw.pending_bytes ?? 0),
    pendingBatches: toSafeNumber(raw.pending_batches ?? 0),
    inFlightBatches: toSafeNumber(raw.in_flight_batches ?? 0),
    hedgedBatches: toSafeNumber(raw.hedged_batches ?? 0),
    shaperDenials: normaliseQosCounts(raw.shaper_denials),
    droppedSegments: toSafeNumber(raw.dropped_segments ?? 0),
    failovers: toSafeNumber(raw.failovers ?? 0),
    openCircuits: toSafeNumber(raw.open_circuits ?? 0),
  };
}

function transformGatewayResult(raw) {
  const localManifest = raw.local_proxy_manifest_json
    ? JSON.parse(raw.local_proxy_manifest_json)
    : null;
  return {
    manifestIdHex: raw.manifest_id_hex,
    chunkerHandle: raw.chunker_handle,
    chunkCount: raw.chunk_count,
    assembledBytes: toSafeNumber(raw.assembled_bytes),
    payload: raw.payload,
    telemetryRegion: raw.telemetry_region ?? null,
    anonymity: {
      policy: raw.anonymity_policy,
      status: raw.anonymity_status,
      reason: raw.anonymity_reason,
      soranetSelected: raw.anonymity_soranet_selected,
      pqSelected: raw.anonymity_pq_selected,
      classicalSelected: raw.anonymity_classical_selected,
      classicalRatio: raw.anonymity_classical_ratio,
      pqRatio: raw.anonymity_pq_ratio,
      candidateRatio: raw.anonymity_candidate_ratio,
      deficitRatio: raw.anonymity_deficit_ratio,
      supplyDelta: raw.anonymity_supply_delta,
      brownout: Boolean(raw.anonymity_brownout),
      brownoutEffective: Boolean(raw.anonymity_brownout_effective),
      usesClassical: Boolean(raw.anonymity_uses_classical),
    },
    providerReports: Array.isArray(raw.provider_reports)
      ? raw.provider_reports.map(transformProviderReport)
      : [],
    chunkReceipts: Array.isArray(raw.chunk_receipts)
      ? raw.chunk_receipts.map(transformChunkReceipt)
      : [],
    localProxyManifest: localManifest,
    carVerification: transformCarVerification(raw.car_verification),
    metadata: transformGatewayMetadata(raw.metadata),
    scoreboard: transformGatewayScoreboard(raw.scoreboard),
    taikaiCacheSummary: transformTaikaiCacheSummary(raw.taikai_cache_summary),
    taikaiCacheQueue: transformTaikaiCacheQueue(raw.taikai_cache_queue),
  };
}

function transformGatewayScoreboard(raw) {
  if (raw === undefined) {
    return undefined;
  }
  if (raw === null) {
    return null;
  }
  if (!Array.isArray(raw)) {
    return undefined;
  }
  return raw.map((entry) => ({
    provider_id:
      typeof entry.provider_id === "string" ? entry.provider_id : "",
    alias: entry.alias ?? null,
    raw_score: entry.raw_score ?? 0,
    normalized_weight: entry.normalized_weight ?? 0,
    eligibility: entry.eligibility ?? null,
  }));
}

function transformGatewayMetadata(raw) {
  if (!raw || typeof raw !== "object") {
    return {
      providerCount: 0,
      gatewayProviderCount: 0,
      providerMix: "none",
      transportPolicy: "",
      transportPolicyOverride: false,
      transportPolicyOverrideLabel: null,
      anonymityPolicy: "",
      anonymityPolicyOverride: false,
      anonymityPolicyOverrideLabel: null,
      maxParallel: null,
      maxPeers: null,
      retryBudget: null,
      providerFailureThreshold: 0,
      assumeNowUnix: 0,
      telemetrySourceLabel: null,
      telemetryRegion: null,
      gatewayManifestProvided: false,
      gatewayManifestId: null,
      gatewayManifestCid: null,
      writeMode: "read-only",
      writeModeEnforcesPq: false,
      allowSingleSourceFallback: false,
      allowImplicitMetadata: false,
    };
  }
  const coerceOptionalNumber = (value) =>
    value === undefined || value === null ? null : toSafeNumber(value);
  return {
    providerCount: toSafeNumber(raw.provider_count ?? 0),
    gatewayProviderCount: toSafeNumber(raw.gateway_provider_count ?? 0),
    providerMix: typeof raw.provider_mix === "string" ? raw.provider_mix : "none",
    transportPolicy: typeof raw.transport_policy === "string" ? raw.transport_policy : "",
    transportPolicyOverride: Boolean(raw.transport_policy_override),
    transportPolicyOverrideLabel:
      typeof raw.transport_policy_override_label === "string"
        ? raw.transport_policy_override_label
        : null,
    anonymityPolicy: typeof raw.anonymity_policy === "string" ? raw.anonymity_policy : "",
    anonymityPolicyOverride: Boolean(raw.anonymity_policy_override),
    anonymityPolicyOverrideLabel:
      typeof raw.anonymity_policy_override_label === "string"
        ? raw.anonymity_policy_override_label
        : null,
    writeMode: typeof raw.write_mode === "string" ? raw.write_mode : "read-only",
    writeModeEnforcesPq: Boolean(raw.write_mode_enforces_pq),
    maxParallel: coerceOptionalNumber(raw.max_parallel),
    maxPeers: coerceOptionalNumber(raw.max_peers),
    retryBudget: coerceOptionalNumber(raw.retry_budget),
    providerFailureThreshold: toSafeNumber(raw.provider_failure_threshold ?? 0),
    assumeNowUnix: toSafeNumber(raw.assume_now_unix ?? 0),
    telemetrySourceLabel:
      typeof raw.telemetry_source_label === "string" ? raw.telemetry_source_label : null,
    telemetryRegion: typeof raw.telemetry_region === "string" ? raw.telemetry_region : null,
    gatewayManifestProvided: Boolean(raw.gateway_manifest_provided),
    gatewayManifestId:
      typeof raw.gateway_manifest_id === "string" ? raw.gateway_manifest_id : null,
    gatewayManifestCid:
      typeof raw.gateway_manifest_cid === "string" ? raw.gateway_manifest_cid : null,
    allowSingleSourceFallback: Boolean(raw.allow_single_source_fallback),
    allowImplicitMetadata: Boolean(raw.allow_implicit_metadata),
  };
}

export function sorafsGatewayFetch(
  manifestIdHex,
  chunkerHandle,
  planJson,
  providers,
  options = {},
) {
  const baseOptions = options ?? {};
  if (!isPlainObject(baseOptions)) {
    throw new TypeError("sorafsGatewayFetch options must be a plain object");
  }
  const { __nativeBinding: injectedBinding, ...restOptions } = baseOptions;
  if (
    restOptions.allowSingleSourceFallback !== undefined &&
    restOptions.allowSingleSourceFallback !== null &&
    typeof restOptions.allowSingleSourceFallback !== "boolean"
  ) {
    throw new TypeError(
      "sorafsGatewayFetch options.allowSingleSourceFallback must be a boolean",
    );
  }
  const binding = injectedBinding ?? requireSorafsNativeBinding();
  if (!binding || typeof binding.sorafsGatewayFetch !== "function") {
    throw new Error(
      "sorafsGatewayFetch requires the native iroha_js_host module. Run `npm run build:native` before using this helper.",
    );
  }
  if (!Array.isArray(providers) || providers.length === 0) {
    throw new TypeError("providers must be a non-empty array");
  }
  const allowSingleSourceFallback = restOptions.allowSingleSourceFallback === true;
  const nativeProviders = providers.map(normaliseGatewayProvider);
  const uniqueProviderCount = new Set(
    nativeProviders.map((spec) => spec.provider_id_hex),
  ).size;
  if (!allowSingleSourceFallback && uniqueProviderCount < 2) {
    throw new Error(
      "sorafsGatewayFetch requires at least two gateway providers; set allowSingleSourceFallback: true to bypass this guard during incident downgrades.",
    );
  }
  const nativeOptions = normaliseGatewayOptions(restOptions);
  try {
    const raw = binding.sorafsGatewayFetch(
      manifestIdHex,
      chunkerHandle,
      planJson,
      nativeProviders,
      nativeOptions,
    );
    return transformGatewayResult(raw);
  } catch (error) {
    throw convertSorafsGatewayError(error);
  }
}

function convertSorafsGatewayError(error) {
  const payload = parseGatewayErrorPayload(error?.message);
  if (payload && payload.kind === "multi_source") {
    return new SorafsGatewayFetchError(payload, error);
  }
  return error;
}

function parseGatewayErrorPayload(text) {
  if (typeof text !== "string") {
    return null;
  }
  const trimmed = text.trim();
  if (!trimmed.startsWith("{")) {
    return null;
  }
  try {
    const parsed = JSON.parse(trimmed);
    return parsed && typeof parsed === "object" ? parsed : null;
  } catch {
    return null;
  }
}
