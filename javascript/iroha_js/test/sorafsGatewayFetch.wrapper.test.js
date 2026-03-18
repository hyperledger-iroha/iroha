import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test as baseTest } from "node:test";
import assert from "node:assert/strict";

import { SorafsGatewayFetchError, sorafsGatewayFetch } from "../src/sorafs.js";
import { makeNativeTest } from "./helpers/native.js";

const test = makeNativeTest(baseTest);

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const MANIFEST_HEX = "aa".repeat(32);
const PROVIDER_ID_HEX = "bb".repeat(32);
const SECOND_PROVIDER_ID_HEX = "cc".repeat(32);

function createStubResult() {
  return {
    manifest_id_hex: MANIFEST_HEX,
    chunker_handle: "sorafs.sf1@1.0.0",
    chunk_count: 2,
    assembled_bytes: 512n,
    payload: Buffer.from([1, 2, 3, 4]),
    telemetry_region: "ci-region",
    anonymity_policy: "anon-guard-pq",
    anonymity_status: "met",
    anonymity_reason: "none",
    anonymity_soranet_selected: 2,
    anonymity_pq_selected: 2,
    anonymity_classical_selected: 0,
    anonymity_classical_ratio: 0,
    anonymity_pq_ratio: 1,
    anonymity_candidate_ratio: 1,
    anonymity_deficit_ratio: 0,
    anonymity_supply_delta: 0,
    anonymity_brownout: false,
    anonymity_brownout_effective: false,
    anonymity_uses_classical: false,
    provider_reports: [
      { provider: "alpha", successes: 1, failures: 0, disabled: false },
    ],
    chunk_receipts: [
      { chunk_index: 0, provider: "alpha", attempts: 1, latency_ms: 12.5, bytes: 256 },
    ],
    local_proxy_manifest_json: JSON.stringify({ authority: "127.0.0.1:9000", proxy_mode: "bridge" }),
    car_verification: {
      manifest_digest_hex: "DEADBEEF",
      manifest_payload_digest_hex: "FEEDFACE",
      manifest_car_digest_hex: "CAFEBABE",
      manifest_content_length: 1024n,
      manifest_chunk_count: 4n,
      manifest_chunk_profile_handle: "sorafs.sf1@1.0.0",
      manifest_governance: {
        council_signatures: [
          { signer_hex: "AA", signature_hex: "BB" },
          { signer_hex: "CC", signature_hex: "DD" },
        ],
      },
      car_archive: {
        size: 2048n,
        payload_digest_hex: "AA",
        archive_digest_hex: "BB",
        cid_hex: "CC",
        root_cids_hex: ["11"],
        verified: true,
        por_leaf_count: 8n,
      },
    },
    metadata: {
      provider_count: 0n,
      gateway_provider_count: 2n,
      provider_mix: "gateway-only",
      transport_policy: "soranet-first",
      transport_policy_override: false,
      transport_policy_override_label: null,
      anonymity_policy: "anon-guard-pq",
      anonymity_policy_override: false,
      anonymity_policy_override_label: null,
      max_parallel: null,
      max_peers: null,
      retry_budget: null,
      provider_failure_threshold: 1n,
      assume_now_unix: 1_700_000_000n,
      telemetry_source_label: "ci-fixture",
      telemetry_region: "iad-prod",
      gateway_manifest_provided: true,
      gateway_manifest_id: MANIFEST_HEX,
      gateway_manifest_cid: MANIFEST_HEX,
      write_mode: "read-only",
      write_mode_enforces_pq: false,
      allow_single_source_fallback: false,
      allow_implicit_metadata: false,
    },
  };
}

test("sorafsGatewayFetch normalises native gateway bindings", (_t) => {
  const calls = [];
  const stubResult = createStubResult();
  const stubBinding = {
    sorafsGatewayFetch: (...args) => {
      calls.push(args);
      return stubResult;
    },
  };

  const providers = [
    {
      name: "alpha",
      providerIdHex: PROVIDER_ID_HEX,
      baseUrl: "https://gateway.test/",
      streamTokenB64: "dG9rZW4=",
      privacyEventsUrl: "https://gateway.test/privacy",
    },
    {
      name: "beta",
      providerIdHex: SECOND_PROVIDER_ID_HEX,
      baseUrl: "https://beta-gateway.test/",
      streamTokenB64: "bWV0cmljc19hY2Nlc3M=",
    },
  ];

  const options = {
    manifestEnvelopeB64: "ZW52ZWxvcGU=",
    manifestCidHex: MANIFEST_HEX,
    clientId: "ci-client",
    telemetryRegion: "ci-region",
    maxPeers: 4,
    retryBudget: 5,
    transportPolicy: "soranet-first",
    anonymityPolicy: "anon-guard-pq",
    writeMode: "upload-pq-only",
    policyOverride: {
      transportPolicy: "soranet-strict",
      anonymityPolicy: "anon-strict-pq",
    },
    localProxy: {
      bindAddr: "127.0.0.1:0",
      telemetryLabel: "proxy-ci",
      emitBrowserManifest: true,
      proxyMode: "bridge",
      prewarmCircuits: true,
      maxStreamsPerCircuit: 4,
      circuitTtlHintSecs: 300,
      noritoBridge: { spoolDir: "/tmp/norito", extension: "norito" },
      carBridge: { cacheDir: "/tmp/car", extension: "car", allowZst: false },
      kaigiBridge: { spoolDir: "/tmp/kaigi", extension: "norito", roomPolicy: "authenticated" },
    },
    taikaiCache: {
      hotCapacityBytes: 8_388_608,
      hotRetentionSecs: 45,
      warmCapacityBytes: 33_554_432,
      warmRetentionSecs: 180,
      coldCapacityBytes: 268_435_456,
      coldRetentionSecs: 3_600,
      qos: {
        priorityRateBps: 83_886_080,
        standardRateBps: 41_943_040,
        bulkRateBps: 12_582_912,
        burstMultiplier: 4,
      },
    },
    scoreboardOutPath: "/tmp/scoreboard.json",
    scoreboardNowUnixSecs: 1_700_000_000n,
    scoreboardTelemetryLabel: "ci-fixture",
    scoreboardAllowImplicitMetadata: true,
  };

  const result = sorafsGatewayFetch(MANIFEST_HEX, "sorafs.sf1@1.0.0", "[]", providers, { ...options, __nativeBinding: stubBinding });

  assert.equal(result.manifestIdHex, MANIFEST_HEX);
  assert.equal(result.chunkerHandle, "sorafs.sf1@1.0.0");
  assert.equal(result.chunkCount, 2);
  assert.equal(result.assembledBytes, 512);
  assert.deepEqual(result.payload, stubResult.payload);
  assert.equal(result.telemetryRegion, "ci-region");
  assert.equal(result.anonymity.policy, "anon-guard-pq");
  assert.equal(result.anonymity.status, "met");
  assert.equal(result.anonymity.reason, "none");
  assert.equal(result.anonymity.soranetSelected, 2);
  assert.equal(result.anonymity.pqSelected, 2);
  assert.equal(result.providerReports.length, 1);
  assert.equal(result.chunkReceipts.length, 1);
  assert.equal(result.localProxyManifest.authority, "127.0.0.1:9000");
  assert(result.carVerification);
  assert.equal(result.carVerification?.carArchive.size, 2048);
  assert.equal(
    result.carVerification?.manifestGovernance.councilSignatures.length,
    2,
  );
  assert.equal(
    result.carVerification?.manifestGovernance.councilSignatures[0].signerHex,
    "AA",
  );
  assert.equal(result.metadata.providerCount, 0);
  assert.equal(result.metadata.gatewayProviderCount, 2);
  assert.equal(result.metadata.providerMix, "gateway-only");
  assert.equal(result.metadata.gatewayManifestId, MANIFEST_HEX);
  assert.equal(result.metadata.gatewayManifestCid, MANIFEST_HEX);
  assert.equal(result.metadata.transportPolicy, "soranet-first");
  assert.equal(result.metadata.anonymityPolicy, "anon-guard-pq");
  assert.equal(result.metadata.writeMode, "read-only");
  assert.equal(result.metadata.writeModeEnforcesPq, false);
  assert.equal(result.metadata.telemetrySourceLabel, "ci-fixture");
  assert.equal(result.metadata.telemetryRegion, "iad-prod");
  assert.equal(result.metadata.allowSingleSourceFallback, false);

  assert.equal(calls.length, 1);
  const [manifestArg, handleArg, planArg, providerArgs, optionArg] = calls[0];
  assert.equal(manifestArg, MANIFEST_HEX);
  assert.equal(handleArg, "sorafs.sf1@1.0.0");
  assert.equal(planArg, "[]");
  assert.equal(providerArgs.length, 2);
  assert.equal(providerArgs[0].provider_id_hex, PROVIDER_ID_HEX.toLowerCase());
  assert.equal(providerArgs[1].provider_id_hex, SECOND_PROVIDER_ID_HEX.toLowerCase());
  assert.equal(optionArg.telemetry_region, "ci-region");
  assert(optionArg.local_proxy);
  assert.equal(optionArg.local_proxy.kaigi_bridge.spool_dir, "/tmp/kaigi");
  assert.equal(optionArg.local_proxy.kaigi_bridge.room_policy, "authenticated");
  assert(optionArg.taikai_cache);
  assert.equal(optionArg.taikai_cache.hot_capacity_bytes, 8_388_608);
  assert.equal(optionArg.taikai_cache.qos.burst_multiplier, 4);
  assert.equal(optionArg.scoreboard_out_path, "/tmp/scoreboard.json");
  assert.equal(optionArg.scoreboard_now_unix_secs, 1_700_000_000n);
  assert.equal(optionArg.scoreboard_telemetry_label, "ci-fixture");
  assert.equal(optionArg.scoreboard_allow_implicit_metadata, true);
  assert.equal(optionArg.write_mode, "upload-pq-only");
  const policyFixturePath = path.join(
    __dirname,
    "../../..",
    "fixtures",
    "sorafs_gateway",
    "policy_override",
    "override.json",
  );
  const policyFixture = JSON.parse(fs.readFileSync(policyFixturePath, "utf8"));
  assert.deepEqual(optionArg.policy_override, policyFixture.policy_override);
  assert.equal(optionArg.policy_override.transport_policy, "soranet-strict");
  assert.equal(optionArg.policy_override.anonymity_policy, "anon-strict-pq");
});

test("sorafsGatewayFetch rejects non-hex provider ids", () => {
  let calls = 0;
  const stubBinding = {
    sorafsGatewayFetch: () => {
      calls += 1;
      return createStubResult();
    },
  };
  const providers = [
    {
      name: "alpha",
      providerIdHex: "zz".repeat(32),
      baseUrl: "https://gateway.test/",
      streamTokenB64: "dG9rZW4=",
    },
  ];
  assert.throws(
    () =>
      sorafsGatewayFetch(
        MANIFEST_HEX,
        "sorafs.sf1@1.0.0",
        "[]",
        providers,
        { allowSingleSourceFallback: true, __nativeBinding: stubBinding },
      ),
    /provider\.providerIdHex must be a 32-byte hex string/,
  );
  assert.equal(calls, 0);
});

test("sorafsGatewayFetch rejects invalid manifest ids", () => {
  let calls = 0;
  const stubBinding = {
    sorafsGatewayFetch: () => {
      calls += 1;
      return createStubResult();
    },
  };
  const providers = [
    {
      name: "alpha",
      providerIdHex: PROVIDER_ID_HEX,
      baseUrl: "https://gateway.test/",
      streamTokenB64: "dG9rZW4=",
    },
  ];
  assert.throws(
    () =>
      sorafsGatewayFetch(
        "zz",
        "sorafs.sf1@1.0.0",
        "[]",
        providers,
        { allowSingleSourceFallback: true, __nativeBinding: stubBinding },
      ),
    /manifestIdHex must be a 32-byte hex string/,
  );
  assert.equal(calls, 0);
});

test("sorafsGatewayFetch rejects invalid manifest envelopes", () => {
  let calls = 0;
  const stubBinding = {
    sorafsGatewayFetch: () => {
      calls += 1;
      return createStubResult();
    },
  };
  const providers = [
    {
      name: "alpha",
      providerIdHex: PROVIDER_ID_HEX,
      baseUrl: "https://gateway.test/",
      streamTokenB64: "dG9rZW4=",
    },
    {
      name: "beta",
      providerIdHex: SECOND_PROVIDER_ID_HEX,
      baseUrl: "https://beta-gateway.test/",
      streamTokenB64: "bWV0cmljc19hY2Nlc3M=",
    },
  ];
  assert.throws(
    () =>
      sorafsGatewayFetch(
        MANIFEST_HEX,
        "sorafs.sf1@1.0.0",
        "[]",
        providers,
        { manifestEnvelopeB64: "not*base64", __nativeBinding: stubBinding },
      ),
    /manifestEnvelopeB64 must be a valid base64 string/,
  );
  assert.equal(calls, 0);
});

test("sorafsGatewayFetch rejects fractional maxPeers", () => {
  let calls = 0;
  const stubBinding = {
    sorafsGatewayFetch: () => {
      calls += 1;
      return createStubResult();
    },
  };
  const providers = [
    {
      name: "alpha",
      providerIdHex: PROVIDER_ID_HEX,
      baseUrl: "https://gateway.test/",
      streamTokenB64: "dG9rZW4=",
    },
    {
      name: "beta",
      providerIdHex: SECOND_PROVIDER_ID_HEX,
      baseUrl: "https://beta-gateway.test/",
      streamTokenB64: "bWV0cmljc19hY2Nlc3M=",
    },
  ];
  assert.throws(
    () =>
      sorafsGatewayFetch(
        MANIFEST_HEX,
        "sorafs.sf1@1.0.0",
        "[]",
        providers,
        { maxPeers: 1.5, __nativeBinding: stubBinding },
      ),
    /maxPeers must be a positive safe integer/,
  );
  assert.equal(calls, 0);
});

test("sorafsGatewayFetch rejects invalid stream tokens", () => {
  let calls = 0;
  const stubBinding = {
    sorafsGatewayFetch: () => {
      calls += 1;
      return createStubResult();
    },
  };
  const providers = [
    {
      name: "alpha",
      providerIdHex: PROVIDER_ID_HEX,
      baseUrl: "https://gateway.test/",
      streamTokenB64: "not-base64!!",
    },
  ];
  assert.throws(
    () =>
      sorafsGatewayFetch(
        MANIFEST_HEX,
        "sorafs.sf1@1.0.0",
        "[]",
        providers,
        { allowSingleSourceFallback: true, __nativeBinding: stubBinding },
      ),
    /streamTokenB64 must be a valid base64/i,
  );
  assert.equal(calls, 0);
});

test("sorafsGatewayFetch rejects fractional scoreboardNowUnixSecs", () => {
  let calls = 0;
  const stubBinding = {
    sorafsGatewayFetch: () => {
      calls += 1;
      return createStubResult();
    },
  };
  const providers = [
    {
      name: "alpha",
      providerIdHex: PROVIDER_ID_HEX,
      baseUrl: "https://gateway.test/",
      streamTokenB64: "dG9rZW4=",
    },
    {
      name: "beta",
      providerIdHex: SECOND_PROVIDER_ID_HEX,
      baseUrl: "https://beta-gateway.test/",
      streamTokenB64: "bWV0cmljc19hY2Nlc3M=",
    },
  ];
  assert.throws(
    () =>
      sorafsGatewayFetch(
        MANIFEST_HEX,
        "sorafs.sf1@1.0.0",
        "[]",
        providers,
        { scoreboardNowUnixSecs: 1.5, __nativeBinding: stubBinding },
      ),
    /scoreboardNowUnixSecs must be an integer/i,
  );
  assert.equal(calls, 0);
});

test("sorafsGatewayFetch rejects fractional taikai cache burst multipliers", () => {
  let calls = 0;
  const stubBinding = {
    sorafsGatewayFetch: () => {
      calls += 1;
      return createStubResult();
    },
  };
  const providers = [
    {
      name: "alpha",
      providerIdHex: PROVIDER_ID_HEX,
      baseUrl: "https://gateway.test/",
      streamTokenB64: "dG9rZW4=",
    },
    {
      name: "beta",
      providerIdHex: SECOND_PROVIDER_ID_HEX,
      baseUrl: "https://beta-gateway.test/",
      streamTokenB64: "bWV0cmljc19hY2Nlc3M=",
    },
  ];
  assert.throws(
    () =>
      sorafsGatewayFetch(
        MANIFEST_HEX,
        "sorafs.sf1@1.0.0",
        "[]",
        providers,
        {
          taikaiCache: {
            hotCapacityBytes: 8_388_608,
            hotRetentionSecs: 45,
            warmCapacityBytes: 33_554_432,
            warmRetentionSecs: 180,
            coldCapacityBytes: 268_435_456,
            coldRetentionSecs: 3_600,
            qos: {
              priorityRateBps: 83_886_080,
              standardRateBps: 41_943_040,
              bulkRateBps: 12_582_912,
              burstMultiplier: 1.5,
            },
          },
          __nativeBinding: stubBinding,
        },
      ),
    /taikaiCache\.qos\.burstMultiplier must be an integer/i,
  );
  assert.equal(calls, 0);
});

test("sorafsGatewayFetch forwards retryBudget zero to disable cap", () => {
  const calls = [];
  const stubResult = createStubResult();
  const stubBinding = {
    sorafsGatewayFetch: (...args) => {
      calls.push(args);
      return stubResult;
    },
  };
  const providers = [
    {
      name: "alpha",
      providerIdHex: PROVIDER_ID_HEX,
      baseUrl: "https://gateway.test/",
      streamTokenB64: "dG9rZW4=",
    },
    {
      name: "beta",
      providerIdHex: SECOND_PROVIDER_ID_HEX,
      baseUrl: "https://beta-gateway.test/",
      streamTokenB64: "YmltZXRyaWNz",
    },
  ];

  sorafsGatewayFetch(MANIFEST_HEX, "sorafs.sf1@1.0.0", "[]", providers, {
    retryBudget: 0,
    __nativeBinding: stubBinding,
  });

  assert.equal(calls.length, 1);
  const [, , , , nativeOptions] = calls[0];
  assert.ok(nativeOptions);
  assert.equal(nativeOptions.retry_budget, 0);
});

test("sorafsGatewayFetch surfaces structured multi-source errors", () => {
  const payload = {
    kind: "multi_source",
    code: "no_providers",
    message: "no providers available for multi-source fetch",
    retryable: false,
  };
  const stubBinding = {
    sorafsGatewayFetch: () => {
      throw new Error(JSON.stringify(payload));
    },
  };
  const providers = [
    {
      name: "alpha",
      providerIdHex: PROVIDER_ID_HEX,
      baseUrl: "https://gateway.test/",
      streamTokenB64: "dG9rZW4=",
    },
    {
      name: "beta",
      providerIdHex: SECOND_PROVIDER_ID_HEX,
      baseUrl: "https://beta-gateway.test/",
      streamTokenB64: "Y2hhbm5lbA==",
    },
  ];

  assert.throws(
    () =>
      sorafsGatewayFetch(MANIFEST_HEX, "sorafs.sf1@1.0.0", "[]", providers, {
        __nativeBinding: stubBinding,
      }),
    (error) => {
      assert.ok(error instanceof SorafsGatewayFetchError);
      assert.equal(error.code, "no_providers");
      assert.equal(error.retryable, false);
      assert.equal(error.chunkIndex, null);
      assert.equal(error.original?.message, JSON.stringify(payload));
      return true;
    },
  );
});

test("sorafsGatewayFetch validates Taikai cache options", () => {
  const stubBinding = {
    sorafsGatewayFetch: () => createStubResult(),
  };
  const providers = [
    {
      name: "alpha",
      providerIdHex: PROVIDER_ID_HEX,
      baseUrl: "https://gateway.test/",
      streamTokenB64: "dG9rZW4=",
    },
    {
      name: "beta",
      providerIdHex: SECOND_PROVIDER_ID_HEX,
      baseUrl: "https://beta-gateway.test/",
      streamTokenB64: "Z2FsbG9w",
    },
  ];

  assert.throws(() => {
    sorafsGatewayFetch(MANIFEST_HEX, "sorafs.sf1@1.0.0", "[]", providers, {
      taikaiCache: {
        hotCapacityBytes: 0,
        hotRetentionSecs: 45,
        warmCapacityBytes: 1,
        warmRetentionSecs: 1,
        coldCapacityBytes: 1,
        coldRetentionSecs: 1,
        qos: {
          priorityRateBps: 1,
          standardRateBps: 1,
          bulkRateBps: 1,
          burstMultiplier: 0,
        },
      },
      __nativeBinding: stubBinding,
    });
  }, /burstMultiplier/i);
});

test("sorafsGatewayFetch rejects single-provider sessions without override", () => {
  const stubBinding = {
    sorafsGatewayFetch: () => createStubResult(),
  };
  assert.throws(
    () =>
      sorafsGatewayFetch(
        MANIFEST_HEX,
        "sorafs.sf1@1.0.0",
        "[]",
        [
          {
            name: "alpha",
            providerIdHex: PROVIDER_ID_HEX,
            baseUrl: "https://gateway.test/",
            streamTokenB64: "dG9rZW4=",
          },
        ],
        { __nativeBinding: stubBinding },
      ),
    /at least two gateway providers/,
  );
});

test("sorafsGatewayFetch forwards allowSingleSourceFallback override", () => {
  const calls = [];
  const stubBinding = {
    sorafsGatewayFetch: (...args) => {
      calls.push(args);
      return createStubResult();
    },
  };
  sorafsGatewayFetch(
    MANIFEST_HEX,
    "sorafs.sf1@1.0.0",
    "[]",
    [
      {
        name: "alpha",
        providerIdHex: PROVIDER_ID_HEX,
        baseUrl: "https://gateway.test/",
        streamTokenB64: "dG9rZW4=",
      },
    ],
    { allowSingleSourceFallback: true, __nativeBinding: stubBinding },
  );
  assert.equal(calls.length, 1);
  const opts = calls[0][4];
  assert.equal(opts.allow_single_source_fallback, true);
});

test("sorafsGatewayFetch validates option and allowSingleSourceFallback types", () => {
  const providers = [
    {
      name: "alpha",
      providerIdHex: PROVIDER_ID_HEX,
      baseUrl: "https://gateway.test/",
      streamTokenB64: "dG9rZW4=",
    },
    {
      name: "beta",
      providerIdHex: SECOND_PROVIDER_ID_HEX,
      baseUrl: "https://beta-gateway.test/",
      streamTokenB64: "bWV0YQ==",
    },
  ];
  assert.throws(
    () =>
      sorafsGatewayFetch(
        MANIFEST_HEX,
        "sorafs.sf1@1.0.0",
        "[]",
        providers,
        "not-an-object",
      ),
    /options must be a plain object/i,
  );
  const stubBinding = {
    sorafsGatewayFetch: () => createStubResult(),
  };
  assert.throws(
    () =>
      sorafsGatewayFetch(
        MANIFEST_HEX,
        "sorafs.sf1@1.0.0",
        "[]",
        providers,
        { allowSingleSourceFallback: "yes", __nativeBinding: stubBinding },
      ),
    /allowSingleSourceFallback must be a boolean/i,
  );
});
