import { test as baseTest } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, writeFileSync, rmSync, mkdtempSync } from "node:fs";
import path from "node:path";
import os from "node:os";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

import { getNativeBinding } from "../src/native.js";
import { makeNativeTest } from "./helpers/native.js";

const test = makeNativeTest(baseTest);

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "..", "..", "..");
const FIXTURE_ROOT = path.join(
  REPO_ROOT,
  "fixtures",
  "sorafs_orchestrator",
  "multi_peer_parity_v1",
);

function loadJson(relativePath) {
  return JSON.parse(readFileSync(path.join(FIXTURE_ROOT, relativePath), "utf8"));
}

const METADATA = loadJson("metadata.json");
const PLAN_SPECS = loadJson("plan.json");
const PROVIDER_FIXTURE = loadJson("providers.json");
const OPTIONS_FIXTURE = loadJson("options.json");
const TELEMETRY_FIXTURE = loadJson("telemetry.json");

const PLAN_JSON = JSON.stringify(PLAN_SPECS, null, 2);
const PAYLOAD_FIXTURE_PATH = path.join(REPO_ROOT, METADATA.payload_path);
const PAYLOAD_BYTES = readFileSync(PAYLOAD_FIXTURE_PATH);

const MAX_PARALLEL_FETCHES = OPTIONS_FIXTURE.max_parallel;
const RETRY_BUDGET = OPTIONS_FIXTURE.retry_budget;
const FAILURE_THRESHOLD = OPTIONS_FIXTURE.provider_failure_threshold;
const MAX_PROVIDERS = OPTIONS_FIXTURE.max_peers;
const MAX_CHUNK_LENGTH = PLAN_SPECS.reduce(
  (max, spec) => (spec.length > max ? spec.length : max),
  0,
);

function requireSorafsNative() {
  const binding = getNativeBinding();
  if (!binding || typeof binding.sorafsMultiFetchLocal !== "function") {
    throw new Error(
      "SoraFS parity tests require the iroha_js_host native module. Run `npm run build:native` before executing this test suite.",
    );
  }
  return binding;
}

function createProviders(tempDir) {
  return PROVIDER_FIXTURE.map((entry, index) => {
    const providerId = entry.provider_id;
    const filePath = path.join(tempDir, `${providerId}.bin`);
    writeFileSync(filePath, PAYLOAD_BYTES);
    const alias = entry.profile_aliases?.[0] ?? `fixture-peer-${index}`;
    return {
      providerId,
      path: filePath,
      alias,
      metadata: entry,
    };
  });
}

function buildProviderMetadata(entry, alias) {
  const rangeSnake = entry.range_capability;
  const streamSnake = entry.stream_budget;
  const transportHintsSnake = Array.isArray(entry.transport_hints) ? entry.transport_hints : [];
  return {
    provider_id: entry.provider_id,
    providerId: entry.provider_id,
    profile_aliases: entry.profile_aliases ?? [alias],
    profileAliases: entry.profile_aliases ?? [alias],
    availability: entry.availability,
    allow_unknown_capabilities: Boolean(entry.allow_unknown_capabilities),
    allowUnknownCapabilities: Boolean(entry.allow_unknown_capabilities),
    capability_names: entry.capability_names,
    capabilityNames: entry.capability_names,
    range_capability: rangeSnake,
    rangeCapability: {
      maxChunkSpan: rangeSnake.max_chunk_span,
      minGranularity: rangeSnake.min_granularity,
      supportsSparseOffsets: rangeSnake.supports_sparse_offsets,
      requiresAlignment: rangeSnake.requires_alignment,
      supportsMerkleProof: rangeSnake.supports_merkle_proof,
    },
    stream_budget: streamSnake,
    streamBudget: {
      maxInFlight: streamSnake.max_in_flight,
      maxBytesPerSec: BigInt(streamSnake.max_bytes_per_sec),
      burstBytes: BigInt(streamSnake.burst_bytes ?? 0),
    },
    refresh_deadline: entry.refresh_deadline,
    refreshDeadline: BigInt(entry.refresh_deadline),
    expires_at: entry.expires_at,
    expiresAt: BigInt(entry.expires_at),
    ttl_secs: entry.ttl_secs,
    ttlSecs: BigInt(entry.ttl_secs),
    notes: entry.notes,
    transport_hints: transportHintsSnake,
    transportHints: transportHintsSnake.map((hint) => ({
      protocol: hint.protocol,
      protocolId: hint.protocol_id,
      priority: hint.priority,
    })),
  };
}

function buildProviderSpecs(providers) {
  return providers.map(({ path: filePath, alias, metadata }) => ({
    name: alias,
    path: filePath,
    max_concurrent: 2,
    maxConcurrent: 2,
    weight: 1,
    metadata: buildProviderMetadata(metadata, alias),
  }));
}

function toBigIntOrUndefined(value) {
  if (typeof value === "bigint") {
    return value;
  }
  if (typeof value === "number" && Number.isFinite(value)) {
    return BigInt(Math.trunc(value));
  }
  return undefined;
}

function buildOptions() {
  const scoreboardOptions = OPTIONS_FIXTURE.scoreboard ?? null;
  const scoreboardNowUnix = toBigIntOrUndefined(scoreboardOptions?.now_unix_secs);
  const scoreboardSnake = scoreboardOptions
    ? {
        now_unix_secs: scoreboardNowUnix ?? scoreboardOptions.now_unix_secs,
        telemetry_grace_period_secs: scoreboardOptions.telemetry_grace_period_secs,
        latency_cap_ms: scoreboardOptions.latency_cap_ms,
      }
    : undefined;
  const scoreboardCamel = scoreboardOptions
    ? {
        nowUnixSecs: scoreboardNowUnix ?? scoreboardOptions.now_unix_secs,
        telemetryGracePeriodSecs: scoreboardOptions.telemetry_grace_period_secs,
        latencyCapMs: scoreboardOptions.latency_cap_ms,
      }
    : undefined;
  return {
    verify_digests: Boolean(OPTIONS_FIXTURE.verify_digests),
    verifyDigests: Boolean(OPTIONS_FIXTURE.verify_digests),
    verify_lengths: Boolean(OPTIONS_FIXTURE.verify_lengths),
    verifyLengths: Boolean(OPTIONS_FIXTURE.verify_lengths),
    retry_budget: RETRY_BUDGET,
    retryBudget: RETRY_BUDGET,
    provider_failure_threshold: FAILURE_THRESHOLD,
    providerFailureThreshold: FAILURE_THRESHOLD,
    max_parallel: MAX_PARALLEL_FETCHES,
    maxParallel: MAX_PARALLEL_FETCHES,
    max_peers: MAX_PROVIDERS,
    maxPeers: MAX_PROVIDERS,
    use_scoreboard: Boolean(OPTIONS_FIXTURE.use_scoreboard),
    useScoreboard: Boolean(OPTIONS_FIXTURE.use_scoreboard),
    return_scoreboard: Boolean(OPTIONS_FIXTURE.return_scoreboard),
    returnScoreboard: Boolean(OPTIONS_FIXTURE.return_scoreboard),
    ...(scoreboardNowUnix === undefined
      ? {}
      : {
          scoreboard_now_unix_secs: scoreboardNowUnix,
          scoreboardNowUnixSecs: scoreboardNowUnix,
        }),
    scoreboard: scoreboardSnake,
    scoreboardOptions: scoreboardCamel,
    telemetry: buildTelemetryEntries(),
  };
}

function buildTelemetryEntries() {
  return TELEMETRY_FIXTURE.map((entry) => {
    const lastUpdated = toBigIntOrUndefined(entry.last_updated_unix);
    return {
      provider_id: entry.provider_id,
      providerId: entry.provider_id,
      qos_score: entry.qos_score,
      qosScore: entry.qos_score,
      latency_p95_ms: entry.latency_p95_ms,
      latencyP95Ms: entry.latency_p95_ms,
      failure_rate_ewma: entry.failure_rate_ewma,
      failureRateEwma: entry.failure_rate_ewma,
      token_health: entry.token_health,
      tokenHealth: entry.token_health,
      staking_weight: entry.staking_weight,
      stakingWeight: entry.staking_weight,
      penalty: entry.penalty,
      last_updated_unix: lastUpdated ?? entry.last_updated_unix,
      lastUpdatedUnix: lastUpdated ?? entry.last_updated_unix,
    };
  });
}

function sortReports(reports) {
  return [...reports].sort((left, right) =>
    getField(left, "provider", "provider").localeCompare(getField(right, "provider", "provider")),
  );
}

function sortReceipts(receipts) {
  return [...receipts].sort((left, right) => {
    const leftIndex = getField(left, "chunk_index", "chunkIndex");
    const rightIndex = getField(right, "chunk_index", "chunkIndex");
    if (leftIndex !== rightIndex) {
      return leftIndex - rightIndex;
    }
    return getField(left, "provider", "provider").localeCompare(
      getField(right, "provider", "provider"),
    );
  });
}

function sortScoreboard(entries = []) {
  return [...entries].sort((left, right) =>
    getField(left, "provider_id", "providerId").localeCompare(
      getField(right, "provider_id", "providerId"),
    ),
  );
}

function getField(record, snakeCase, camelCase) {
  if (record == null) {
    return undefined;
  }
  if (snakeCase in record) {
    return record[snakeCase];
  }
  if (camelCase in record) {
    return record[camelCase];
  }
  return undefined;
}

function normalizeReceipt(receipt) {
  return {
    chunkIndex: getField(receipt, "chunk_index", "chunkIndex"),
    provider: getField(receipt, "provider", "provider"),
    attempts: getField(receipt, "attempts", "attempts"),
    bytes: getField(receipt, "bytes", "bytes"),
  };
}

test("SoraFS orchestrator multi-fetch is deterministic across runs", () => {
  const native = requireSorafsNative();
  const tempDir = mkdtempSync(path.join(os.tmpdir(), "sorafs-js-parity-"));
  try {
    const providers = createProviders(tempDir);
    const options = buildOptions();
    const plan = PLAN_JSON;

    const firstStart = performance.now();
    const first = native.sorafsMultiFetchLocal(plan, buildProviderSpecs(providers), options);
    const firstDurationMs = performance.now() - firstStart;

    const secondStart = performance.now();
    const second = native.sorafsMultiFetchLocal(plan, buildProviderSpecs(providers), options);
    const secondDurationMs = performance.now() - secondStart;

    const firstChunkCount = getField(first, "chunk_count", "chunkCount");
    const secondChunkCount = getField(second, "chunk_count", "chunkCount");
    assert.equal(firstChunkCount, PLAN_SPECS.length);
    assert.equal(secondChunkCount, PLAN_SPECS.length);

    const firstPayload = getField(first, "payload", "payload");
    const secondPayload = getField(second, "payload", "payload");
    assert.equal(firstPayload.length, METADATA.payload_bytes);
    assert.equal(secondPayload.length, METADATA.payload_bytes);
    assert.equal(
      Buffer.compare(firstPayload, secondPayload),
      0,
      "payload bytes must remain identical between runs",
    );

    const firstReports = getField(first, "provider_reports", "providerReports") ?? [];
    const secondReports = getField(second, "provider_reports", "providerReports") ?? [];
    assert.deepEqual(sortReports(firstReports), sortReports(secondReports));

    const firstReceipts = getField(first, "chunk_receipts", "chunkReceipts") ?? [];
    const secondReceipts = getField(second, "chunk_receipts", "chunkReceipts") ?? [];
    const normalizedFirstReceipts = sortReceipts(firstReceipts).map(normalizeReceipt);
    const normalizedSecondReceipts = sortReceipts(secondReceipts).map(normalizeReceipt);
    assert.deepEqual(normalizedFirstReceipts, normalizedSecondReceipts);

    const firstScoreboard = getField(first, "scoreboard", "scoreboard") ?? [];
    const secondScoreboard = getField(second, "scoreboard", "scoreboard") ?? [];
    assert.deepEqual(sortScoreboard(firstScoreboard), sortScoreboard(secondScoreboard));

    assert(firstDurationMs <= 2_000, `first parity run exceeded 2s (${firstDurationMs.toFixed(2)} ms)`);
    assert(
      secondDurationMs <= 2_000,
      `second parity run exceeded 2s (${secondDurationMs.toFixed(2)} ms)`,
    );

    const peakReservedBytes = MAX_PARALLEL_FETCHES * MAX_CHUNK_LENGTH;
    console.log(
      [
        `JS orchestrator parity: duration_ms=${firstDurationMs.toFixed(2)}`,
        `total_bytes=${firstPayload.length}`,
        `max_parallel=${MAX_PARALLEL_FETCHES}`,
        `peak_reserved_bytes=${peakReservedBytes}`,
      ].join(" "),
    );
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
});
