import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  ToriiClient,
  ToriiHttpError,
  IsoMessageTimeoutError,
  bootstrapConnectPreviewSession,
  extractToriiFeatureConfig,
  buildRegisterDomainTransaction,
  buildRegisterAccountAndTransferTransaction,
  buildRegisterAssetDefinitionAndMintTransaction,
  buildMintAssetTransaction,
  buildTransferAssetTransaction,
  buildMintAssetInstruction,
  buildTimeTriggerAction,
  buildPacs008Message,
  buildPacs009Message,
  inspectAccountId,
} from "../src/index.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const BASE_URL = process.env.IROHA_TORII_INTEGRATION_URL ?? "";
const AUTH_TOKEN = process.env.IROHA_TORII_INTEGRATION_AUTH_TOKEN ?? null;
const API_TOKEN = process.env.IROHA_TORII_INTEGRATION_API_TOKEN ?? null;
const CONFIG_PATH = process.env.IROHA_TORII_INTEGRATION_CONFIG ?? null;
const RBC_SAMPLE = process.env.IROHA_TORII_INTEGRATION_RBC_SAMPLE ?? null;
const CONNECT_SESSION = process.env.IROHA_TORII_INTEGRATION_CONNECT_SESSION ?? null;
const ISO_ENABLED = parseBooleanEnv(
  process.env.IROHA_TORII_INTEGRATION_ISO_ENABLED ?? "0",
);
const ISO_ALIAS_LABEL_RAW = process.env.IROHA_TORII_INTEGRATION_ISO_ALIAS ?? "";
const ISO_ALIAS_LABEL =
  ISO_ALIAS_LABEL_RAW.trim().length === 0 ? null : ISO_ALIAS_LABEL_RAW.trim();
const ISO_ALIAS_INDEX_RAW =
  process.env.IROHA_TORII_INTEGRATION_ISO_ALIAS_INDEX ?? "";
const ISO_ALIAS_INDEX =
  ISO_ALIAS_INDEX_RAW.trim().length === 0 ? null : ISO_ALIAS_INDEX_RAW.trim();
const ISO_PACS008_OVERRIDES = parseJsonEnv(
  process.env.IROHA_TORII_INTEGRATION_ISO_PACS008 ?? null,
);
const ISO_PACS009_OVERRIDES = parseJsonEnv(
  process.env.IROHA_TORII_INTEGRATION_ISO_PACS009 ?? null,
);
const ISO_VOPRF_INPUT_RAW =
  process.env.IROHA_TORII_INTEGRATION_ISO_VOPRF ?? "";
const ISO_VOPRF_INPUT =
  ISO_VOPRF_INPUT_RAW.trim().length === 0 ? "deadbeef" : ISO_VOPRF_INPUT_RAW.trim();
const ISO_POLL_INTERVAL_MS = parsePositiveIntegerEnv(
  process.env.IROHA_TORII_INTEGRATION_ISO_POLL_MS,
  2_000,
);
const ISO_MAX_ATTEMPTS = parsePositiveIntegerEnv(
  process.env.IROHA_TORII_INTEGRATION_ISO_MAX_ATTEMPTS,
  5,
);
const SORAFS_ENABLED = parseBooleanEnv(
  process.env.IROHA_TORII_INTEGRATION_SORAFS_ENABLED ?? "0",
);
const SORAFS_POR_WEEK =
  process.env.IROHA_TORII_INTEGRATION_SORAFS_POR_WEEK ?? null;
const SORAFS_FETCH_MANIFEST_RAW =
  process.env.IROHA_TORII_INTEGRATION_SORAFS_FETCH_MANIFEST ?? "";
const SORAFS_FETCH_MANIFEST =
  SORAFS_FETCH_MANIFEST_RAW.trim().length === 0 ? null : SORAFS_FETCH_MANIFEST_RAW.trim();
const SORAFS_FETCH_OFFSET = parseUnsignedIntegerEnv(
  process.env.IROHA_TORII_INTEGRATION_SORAFS_FETCH_OFFSET,
  { allowZero: true },
);
const SORAFS_FETCH_LENGTH = parseUnsignedIntegerEnv(
  process.env.IROHA_TORII_INTEGRATION_SORAFS_FETCH_LENGTH,
  { allowZero: false },
);
const SORAFS_FETCH_PROVIDER_RAW =
  process.env.IROHA_TORII_INTEGRATION_SORAFS_FETCH_PROVIDER ?? "";
const SORAFS_FETCH_PROVIDER =
  SORAFS_FETCH_PROVIDER_RAW.trim().length === 0 ? null : SORAFS_FETCH_PROVIDER_RAW.trim();
const DA_ENABLED = parseBooleanEnv(
  process.env.IROHA_TORII_INTEGRATION_DA_ENABLED ?? "0",
);
const DA_GATEWAY_SPECS = parseJsonEnv(
  process.env.IROHA_TORII_INTEGRATION_DA_GATEWAYS ?? null,
);
const DA_TICKET_RAW = process.env.IROHA_TORII_INTEGRATION_DA_TICKET ?? "";
const DA_TICKET_HEX = DA_TICKET_RAW.trim().length === 0 ? null : DA_TICKET_RAW.trim();
const UAID_LITERAL_RAW = process.env.IROHA_TORII_INTEGRATION_UAID ?? "";
const UAID_LITERAL =
  UAID_LITERAL_RAW.trim().length === 0 ? null : UAID_LITERAL_RAW.trim();
const UAID_DATASPACE_ID = parseUnsignedIntegerEnv(
  process.env.IROHA_TORII_INTEGRATION_UAID_DATASPACE,
  { allowZero: true },
);
const SNS_SUFFIX_ID = parseUnsignedIntegerEnv(
  process.env.IROHA_TORII_INTEGRATION_SNS_SUFFIX,
  { allowZero: true },
);
const SNS_SELECTOR_RAW = process.env.IROHA_TORII_INTEGRATION_SNS_SELECTOR ?? "";
const SNS_SELECTOR =
  SNS_SELECTOR_RAW.trim().length === 0 ? null : SNS_SELECTOR_RAW.trim();
const SPACE_DIRECTORY_ENABLED = parseBooleanEnv(
  process.env.IROHA_TORII_INTEGRATION_SPACE_DIRECTORY_ENABLED ?? "0",
);
const SPACE_DIRECTORY_MANIFEST_PATH =
  process.env.IROHA_TORII_INTEGRATION_SPACE_DIRECTORY_MANIFEST ?? "";
const SPACE_DIRECTORY_REVOKE_EPOCH = parseUnsignedIntegerEnv(
  process.env.IROHA_TORII_INTEGRATION_SPACE_DIRECTORY_REVOKE_EPOCH,
  { allowZero: true },
);
const RBC_SAMPLE_OPTIONS = parseJsonEnv(RBC_SAMPLE);
const CONNECT_APP_CONFIG = parseJsonEnv(
  process.env.IROHA_TORII_INTEGRATION_CONNECT_APP ?? null,
);
const CONNECT_PREVIEW_CONFIG = parseJsonEnv(
  process.env.IROHA_TORII_INTEGRATION_CONNECT_PREVIEW ?? null,
);
const CONTRACT_CALL_OPTIONS = parseJsonEnv(
  process.env.IROHA_TORII_INTEGRATION_CONTRACT_CALL ?? null,
);
const GOVERNANCE_BALLOT_OPTIONS = parseJsonEnv(
  process.env.IROHA_TORII_INTEGRATION_GOV_BALLOT ?? null,
);
const CHAIN_ID =
  process.env.IROHA_TORII_INTEGRATION_CHAIN_ID ?? "00000000-0000-0000-0000-000000000000";
const AUTHORITY_ACCOUNT_ID =
  process.env.IROHA_TORII_INTEGRATION_ACCOUNT_ID ??
  "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland";
const PRIVATE_KEY_HEX =
  process.env.IROHA_TORII_INTEGRATION_PRIVATE_KEY_HEX ??
  "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53";
const MUTATION_ENABLED = parseBooleanEnv(process.env.IROHA_TORII_INTEGRATION_MUTATE ?? "0");
const STREAM_ENABLED = parseBooleanEnv(
  process.env.IROHA_TORII_INTEGRATION_STREAM_ENABLED ?? "0",
);
const PROJECT_ROOT = path.resolve(__dirname, "..", "..");

const SKIP_REASON = BASE_URL
  ? null
  : "set IROHA_TORII_INTEGRATION_URL to enable Torii integration smoke tests";

function parseBooleanEnv(value) {
  if (!value) {
    return false;
  }
  const normalized = value.trim().toLowerCase();
  return normalized !== "0" && normalized !== "false";
}

const SUCCESS_STATUSES = new Set(["committed", "approved", "applied"]);
const KAIGI_HEALTH_STATUSES = new Set(["healthy", "degraded", "unavailable"]);

/**
 * Integration smoke test that exercises a real Torii instance when the relevant
 * environment variables are provided. The test suite remains skipped by default
 * so CI can run without a live node.
 */
test(
  "torii integration smoke test",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
    todo: false,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }

    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });

    const health = await client.getHealth();
    assert.ok(health, "health snapshot should be present");

    const metricsText = await client.getMetrics({ asText: true });
    assert.equal(typeof metricsText, "string");
    assert.notEqual(metricsText.length, 0);

    const sumeragiStatus = await client.getSumeragiStatus();
    assert.ok(sumeragiStatus === null || typeof sumeragiStatus === "object");

    const rbcStatus = await client.getSumeragiRbc();
    assert.ok(rbcStatus === null || typeof rbcStatus === "object");

    const connectStatus = await client.getConnectStatus();
    assert.ok(connectStatus === null || typeof connectStatus === "object");

    const timeNow = await client.getNetworkTimeNow();
    assertNonNegativeInteger(timeNow.timestampMs, "network time timestampMs");
    assert.ok(
      Number.isInteger(timeNow.offsetMs),
      "network time offsetMs must be an integer",
    );
    assert.ok(
      Number.isInteger(timeNow.confidenceMs) && timeNow.confidenceMs >= 0,
      "network time confidenceMs must be a non-negative integer",
    );

    const timeStatus = await client.getNetworkTimeStatus();
    assertNonNegativeInteger(timeStatus.peers, "network time peers");
    assert.ok(Array.isArray(timeStatus.samples), "network time samples must be an array");
    timeStatus.samples.forEach((sample, index) => {
      assert.equal(
        typeof sample.peer,
        "string",
        `network time samples[${index}].peer must be a string`,
      );
      assert.notEqual(
        sample.peer.length,
        0,
        `network time samples[${index}].peer must be non-empty`,
      );
      assert.ok(
        Number.isInteger(sample.lastOffsetMs),
        `network time samples[${index}].lastOffsetMs must be an integer`,
      );
      assert.ok(
        Number.isInteger(sample.lastRttMs) && sample.lastRttMs >= 0,
        `network time samples[${index}].lastRttMs must be a non-negative integer`,
      );
      assertNonNegativeInteger(
        sample.count,
        `network time samples[${index}].count`,
      );
    });
    assert.ok(timeStatus.rtt && typeof timeStatus.rtt === "object", "network time rtt payload");
    assert.ok(
      Array.isArray(timeStatus.rtt.buckets),
      "network time rtt buckets must be an array",
    );
    timeStatus.rtt.buckets.forEach((bucket, index) => {
      assert.equal(
        typeof bucket,
        "object",
        `network time rtt buckets[${index}] must be an object`,
      );
      assert.ok(
        Number.isInteger(bucket.le),
        `network time rtt buckets[${index}].le must be an integer`,
      );
      assertNonNegativeInteger(
        bucket.count,
        `network time rtt buckets[${index}].count`,
      );
    });
    assertNonNegativeInteger(timeStatus.rtt.sumMs, "network time rtt sumMs");
    assertNonNegativeInteger(timeStatus.rtt.count, "network time rtt count");
    if (timeStatus.note !== undefined && timeStatus.note !== null) {
      assert.equal(
        typeof timeStatus.note,
        "string",
        "network time status note must be a string when present",
      );
    }

    if (CONFIG_PATH) {
      const resolved = path.resolve(CONFIG_PATH);
      const configRaw = fs.readFileSync(resolved, "utf8");
      const configData = JSON.parse(configRaw);
      const features = extractToriiFeatureConfig({ config: configData });
      assert.ok("isoBridge" in features);
      assert.ok("rbcSampling" in features);
      assert.ok("connect" in features);
    }

    if (CONNECT_SESSION) {
      const sessionOptions = JSON.parse(CONNECT_SESSION);
      assert.ok(sessionOptions.sid, "connect session payload must provide sid");
      const session = await client.createConnectSession(sessionOptions);
      assert.ok(session?.wallet_uri ?? session?.walletUri, "connect session should return a wallet URI");
    }
  },
);

test(
  "status snapshot exposes governance and queue metrics",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const snapshot = await client.getStatusSnapshot();
    assert.ok(snapshot, "status snapshot response must be present");
    assert.equal(typeof snapshot.timestamp, "number", "snapshot timestamp should be numeric");

    const { status, metrics } = snapshot;
    assert.ok(status && typeof status === "object", "status payload should be an object");
    assert.ok(metrics && typeof metrics === "object", "status metrics should be an object");

    assert.equal(typeof status.peers, "number");
    assert.equal(typeof status.queue_size, "number");
    assert.equal(typeof status.commit_time_ms, "number");
    assert.ok(Array.isArray(status.lane_governance), "lane governance entries must be an array");
    assert.ok(
      Array.isArray(status.lane_governance_sealed_aliases),
      "lane governance sealed aliases must be an array",
    );
    assert.ok(
      Array.isArray(status.dataspace_commitments),
      "dataspace commitments must be an array",
    );

    assert.equal(typeof metrics.commit_latency_ms, "number");
    assert.equal(typeof metrics.queue_size, "number");
    assert.equal(typeof metrics.queue_delta, "number");
    assert.equal(typeof metrics.da_reschedule_delta, "number");
    assert.equal(typeof metrics.tx_approved_delta, "number");
    assert.equal(typeof metrics.tx_rejected_delta, "number");
    assert.equal(typeof metrics.view_change_delta, "number");
    assert.equal(typeof metrics.has_activity, "boolean");

    if (status.governance) {
      const {
        proposals,
        protected_namespace: protectedNamespace,
        manifest_admission: manifestAdmission,
        manifest_quorum: manifestQuorum,
        recent_manifest_activations: activations,
      } = status.governance;

      assertNonNegativeInteger(
        proposals.proposed,
        "governance.proposals.proposed",
      );
      assertNonNegativeInteger(
        proposals.approved,
        "governance.proposals.approved",
      );
      assertNonNegativeInteger(
        proposals.rejected,
        "governance.proposals.rejected",
      );
      assertNonNegativeInteger(
        proposals.enacted,
        "governance.proposals.enacted",
      );

      assertNonNegativeInteger(
        protectedNamespace.total_checks,
        "governance.protected_namespace.total_checks",
      );
      assertNonNegativeInteger(
        protectedNamespace.allowed,
        "governance.protected_namespace.allowed",
      );
      assertNonNegativeInteger(
        protectedNamespace.rejected,
        "governance.protected_namespace.rejected",
      );

      assertNonNegativeInteger(
        manifestAdmission.total_checks,
        "governance.manifest_admission.total_checks",
      );
      assertNonNegativeInteger(
        manifestAdmission.allowed,
        "governance.manifest_admission.allowed",
      );
      assertNonNegativeInteger(
        manifestAdmission.quorum_rejected,
        "governance.manifest_admission.quorum_rejected",
      );

      assertNonNegativeInteger(
        manifestQuorum.total_checks,
        "governance.manifest_quorum.total_checks",
      );
      assertNonNegativeInteger(
        manifestQuorum.allowed,
        "governance.manifest_quorum.allowed",
      );
      assertNonNegativeInteger(
        manifestQuorum.rejected,
        "governance.manifest_quorum.rejected",
      );

      assert.ok(
        Array.isArray(activations),
        "governance recent activations must be an array when provided",
      );
    } else {
      t.diagnostic("status snapshot did not include governance counters");
    }
  },
);

test(
  "peer metadata endpoints return typed payloads",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });

    const peers = await client.listPeersTyped();
    assert.ok(Array.isArray(peers), "peer listing must return an array");
    if (peers.length === 0) {
      t.diagnostic("listPeersTyped returned an empty array; skipping shape assertions");
    } else {
      for (const peer of peers) {
        assert.equal(typeof peer.address, "string");
        assert.notEqual(peer.address.length, 0);
        assert.equal(typeof peer.public_key_hex, "string");
        assert.equal(peer.public_key_hex.length, 64);
      }
    }

    const telemetryPeers = await client.listTelemetryPeersInfo();
    assert.ok(
      Array.isArray(telemetryPeers),
      "telemetry peer listing must return an array",
    );
    if (telemetryPeers.length === 0) {
      t.diagnostic(
        "listTelemetryPeersInfo returned an empty array; skipping telemetry shape assertions",
      );
      return;
    }

    for (const entry of telemetryPeers) {
      assert.equal(typeof entry.url, "string");
      assert.notEqual(entry.url.length, 0);
      assert.equal(typeof entry.connected, "boolean");
      if (entry.config) {
        assert.equal(typeof entry.config.publicKey, "string");
        assert.notEqual(entry.config.publicKey.length, 0);
      }
      if (entry.location) {
        assert.equal(typeof entry.location.countryCode, "string");
        assert.notEqual(entry.location.countryCode.length, 0);
      }
      if (entry.connectedPeers) {
        assert.ok(Array.isArray(entry.connectedPeers));
        for (const peerUrl of entry.connectedPeers) {
          assert.equal(typeof peerUrl, "string");
          assert.notEqual(peerUrl.length, 0);
        }
      }
    }
  },
);

test(
  "block list endpoint returns pagination metadata",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const page = await client.listBlocks({ limit: 5 });
    assert.ok(page, "blocks page payload must be present");
    const { pagination, items } = page;
    assert.ok(pagination, "block list should include pagination metadata");
    assertNonNegativeInteger(pagination.page, "blocks.pagination.page");
    assertNonNegativeInteger(pagination.perPage, "blocks.pagination.perPage");
    assertNonNegativeInteger(pagination.totalPages, "blocks.pagination.totalPages");
    assertNonNegativeInteger(pagination.totalItems, "blocks.pagination.totalItems");
    assert.ok(Array.isArray(items), "block list should return an items array");
    assert.ok(items.length <= 5, "block list must respect provided limit");
    if (items.length === 0) {
      t.diagnostic("block list returned zero entries");
      return;
    }

    const first = items[0];
    assert.equal(typeof first.hash, "string", "block hash must be a string");
    assertNonNegativeInteger(first.height, "blocks.items[0].height");
    assert.equal(typeof first.createdAt, "string", "block createdAt must be a string");
    assert.equal(
      typeof first.transactionsTotal,
      "number",
      "block transactionsTotal must be numeric",
    );
    const fetched = await client.getBlock(first.height);
    assert.ok(fetched, "getBlock should return payload for heights returned by listBlocks");
    assert.equal(fetched.hash, first.hash, "block hash should match the list entry");
    assert.equal(fetched.height, first.height, "block height should match the list entry");
  },
);

test(
  "account permission endpoints expose authority tokens",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const accountId = AUTHORITY_ACCOUNT_ID;
    const page = await client.listAccountPermissions(accountId, { limit: 10 });
    assert.ok(page, "account permission list should return a payload");
    assert.ok(
      typeof page.total === "number",
      "account permission payload should expose a total count",
    );
    assert.ok(
      Array.isArray(page.items),
      "account permission payload should expose an items array",
    );
    if (page.items.length === 0) {
      t.diagnostic(
        "authority account reported zero permission tokens; skipping iterator assertions",
      );
      return;
    }
    const sample = page.items[0];
    assert.equal(typeof sample.name, "string", "permission token must include a name");
    assert.notEqual(sample.name.length, 0, "permission token name must be non-empty");
    if (sample.payload !== null && sample.payload !== undefined) {
      assert.equal(
        typeof sample.payload,
        "object",
        "permission token payload must be an object when present",
      );
    }
    const limitedPage = await client.listAccountPermissions(accountId, { limit: 1 });
    assert.ok(
      Array.isArray(limitedPage.items),
      "account permission list should respect pagination parameters",
    );
    assert.ok(
      limitedPage.items.length <= 1,
      "account permission list must respect the provided limit",
    );
    const iteratorIncludesSample = await iteratorIncludes(
      client.iterateAccountPermissions(accountId, { limit: 2 }),
      (entry) => entry?.name === sample.name,
    );
    assert.ok(
      iteratorIncludesSample,
      "account permission iterator should surface entries returned by listAccountPermissions",
    );
  },
);

test(
  "configuration snapshot returns JSON payload (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 30_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const config = await client.getConfiguration();
    if (!config) {
      t.diagnostic("Torii configuration endpoint unavailable on target node");
      return;
    }
    assert.equal(typeof config, "object", "configuration snapshot must be an object");
    assert.ok(
      Object.keys(config).length > 0,
      "configuration snapshot should include at least one field",
    );
  },
);

test(
  "confidential gas schedule helper reports numeric fields (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 30_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const gasSchedule = await client.getConfidentialGasSchedule();
    if (!gasSchedule) {
      t.diagnostic(
        "Torii configuration did not expose a confidential gas schedule; skipping numeric assertions",
      );
      return;
    }
    assertNonNegativeNumber(gasSchedule.proofBase, "gasSchedule.proofBase");
    assertNonNegativeNumber(gasSchedule.perPublicInput, "gasSchedule.perPublicInput");
    assertNonNegativeNumber(gasSchedule.perProofByte, "gasSchedule.perProofByte");
    assertNonNegativeNumber(gasSchedule.perNullifier, "gasSchedule.perNullifier");
    assertNonNegativeNumber(gasSchedule.perCommitment, "gasSchedule.perCommitment");
  },
);

test(
  "node capabilities and runtime ABI endpoints respond",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });

    const capabilities = await client.getNodeCapabilities();
    assert.ok(
      Array.isArray(capabilities.supportedAbiVersions),
      "node capabilities should list supported ABI versions",
    );
    assert.ok(
      capabilities.supportedAbiVersions.length > 0,
      "at least one ABI version should be advertised",
    );
    assertNonNegativeInteger(
      capabilities.defaultCompileTarget,
      "node capabilities defaultCompileTarget",
    );
    assert.equal(typeof capabilities.crypto, "object", "crypto capabilities must exist");
    assert.equal(
      typeof capabilities.crypto.sm.enabled,
      "boolean",
      "SM capability flag must be boolean",
    );
    assert.ok(
      Array.isArray(capabilities.crypto.sm.allowedSigning),
      "SM allowedSigning list must be an array",
    );
    assert.ok(
      Array.isArray(capabilities.crypto.curves.allowedCurveIds),
      "curve capability list must be an array",
    );

    const abiActive = await client.getRuntimeAbiActive();
    assert.ok(
      Array.isArray(abiActive.activeVersions),
      "runtime ABI active versions must be an array",
    );
    assertNonNegativeInteger(
      abiActive.defaultCompileTarget,
      "runtime ABI default compile target",
    );

    const abiHash = await client.getRuntimeAbiHash();
    assert.ok(abiHash.policy, "runtime ABI hash response must include policy label");
    assertHexString(abiHash.abiHashHex, "runtime ABI hash hex");

    const runtimeMetrics = await client.getRuntimeMetrics();
    assertNonNegativeInteger(
      runtimeMetrics.activeAbiVersionsCount,
      "runtime metrics activeAbiVersionsCount",
    );
    assert.equal(
      typeof runtimeMetrics.upgradeEventsTotal.proposed,
      "number",
      "runtime metrics upgradeEventsTotal.proposed must be numeric",
    );
    assert.equal(
      typeof runtimeMetrics.upgradeEventsTotal.activated,
      "number",
      "runtime metrics upgradeEventsTotal.activated must be numeric",
    );
    assert.equal(
      typeof runtimeMetrics.upgradeEventsTotal.canceled,
      "number",
      "runtime metrics upgradeEventsTotal.canceled must be numeric",
    );

    const runtimeUpgrades = await client.listRuntimeUpgrades();
    assert.ok(Array.isArray(runtimeUpgrades), "runtime upgrades list must be an array");
    if (runtimeUpgrades.length > 0) {
      const entry = runtimeUpgrades[0];
      assertHexString(entry.idHex, "runtime upgrade idHex");
      assert.ok(
        entry.record?.manifest?.name,
        "runtime upgrade entries must expose manifest metadata",
      );
    } else {
      t.diagnostic("runtime upgrades list is empty on target node");
    }
  },
);

test(
  "sumeragi evidence list/count endpoints respond",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const count = await client.getSumeragiEvidenceCount();
    assertNonNegativeInteger(count.count, "sumeragi evidence count");

    const page = await client.listSumeragiEvidence({ limit: 5 });
    assertNonNegativeInteger(page.total, "sumeragi evidence total");
    assert.ok(Array.isArray(page.items), "sumeragi evidence items must be an array");
    assert.ok(
      page.items.length <= 5,
      "sumeragi evidence list should respect the provided limit",
    );
    if (page.items.length === 0) {
      t.diagnostic("no consensus evidence entries returned by the target node");
      return;
    }
    for (const entry of page.items) {
      assertEvidenceRecord(entry);
    }
  },
);

test(
  "zk prover report endpoints respond",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });

    const fetchReports = async (filters, context) => {
      try {
        return await client.listProverReports(filters);
      } catch (error) {
        if (shouldSkipZkProverEndpoints(error)) {
          t.diagnostic(
            `${context} unavailable: ${
              error instanceof Error ? error.message : String(error)
            }`,
          );
          return null;
        }
        throw error;
      }
    };

    const reportsResult = await fetchReports({ limit: 5 }, "prover report listing");
    if (!reportsResult) {
      return;
    }
    assertProverReportResult(reportsResult, "prover report listing");

    const hasEntries = hasProverReportEntries(reportsResult);
    if (!hasEntries) {
      t.diagnostic("prover report list returned no entries; skipping projection checks");
      return;
    }

    const idsResult = await fetchReports({ limit: 5, ids_only: true }, "prover id projection");
    if (!idsResult) {
      return;
    }
    assert.equal(
      idsResult.kind,
      "ids",
      "ids_only projection must return identifier list when reports exist",
    );
    assertProverReportResult(idsResult, "prover id projection");

    const failedCount = countFailedProverReports(reportsResult);
    if (failedCount === 0) {
      t.diagnostic("no failed prover reports; skipping messages-only projection");
      return;
    }

    const messagesResult = await fetchReports(
      { limit: 5, messages_only: true },
      "prover messages projection",
    );
    if (!messagesResult) {
      return;
    }
    assert.equal(
      messagesResult.kind,
      "messages",
      "messages_only projection must return message summaries for failed reports",
    );
    assertProverReportResult(messagesResult, "prover messages projection");
  },
);

test(
  "nft list and query endpoints respond (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });

    let listPage;
    try {
      listPage = await client.listNfts({ limit: 5 });
    } catch (error) {
      if (isUnexpectedNotFoundError(error)) {
        t.diagnostic(
          `NFT list endpoint unavailable on target node: ${
            error instanceof Error ? error.message : String(error)
          }`,
        );
        return;
      }
      throw error;
    }
    assertNonNegativeInteger(listPage.total, "nft list total");
    assert.ok(Array.isArray(listPage.items), "nft list must expose items array");
    if (listPage.items.length === 0) {
      t.diagnostic("NFT list returned zero entries; skipping filter assertions");
    } else {
      const entry = listPage.items[0];
      assert.equal(typeof entry.id, "string", "nft list entry id must be a string");
      assert.notEqual(entry.id.length, 0, "nft list entry id must not be empty");
    }

    let queryPage;
    try {
      queryPage = await client.queryNfts({ limit: 5 });
    } catch (error) {
      if (isUnexpectedNotFoundError(error)) {
        t.diagnostic(
          `NFT query endpoint unavailable on target node: ${
            error instanceof Error ? error.message : String(error)
          }`,
        );
        return;
      }
      throw error;
    }
    assertNonNegativeInteger(queryPage.total, "nft query total");
    assert.ok(Array.isArray(queryPage.items), "nft query must expose items array");

    const referenceId =
      listPage.items.find((entry) => entry?.id)?.id ??
      queryPage.items.find((entry) => entry?.id)?.id ??
      null;
    if (referenceId) {
      assert.ok(
        queryPage.items.some((entry) => entry?.id === referenceId),
        "nft query should include at least one known id",
      );
      const iteratorListFound = await iteratorIncludes(
        client.iterateNfts({ limit: 2, maxItems: 10 }),
        (entry) => entry?.id === referenceId,
      );
      assert.ok(iteratorListFound, "nft iterator should surface the reference id");
      const iteratorQueryFound = await iteratorIncludes(
        client.iterateNftsQuery({ limit: 2, maxItems: 10 }),
        (entry) => entry?.id === referenceId,
      );
      assert.ok(iteratorQueryFound, "nft query iterator should surface the reference id");
    } else {
      t.diagnostic("NFT endpoints returned empty payloads; iterator assertions skipped");
      for await (const _unused of client.iterateNfts({ limit: 2, maxItems: 2 })) {
        t.diagnostic(`drained nft iterator item: ${JSON.stringify(_unused)}`);
        break;
      }
    }
  },
);

test(
  "register domain via pipeline and fetch via listDomains",
  {
    skip: !!SKIP_REASON,
    timeout: 90_000,
  },
  async (t) => {
    if (!MUTATION_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_MUTATE=1 to enable mutation coverage");
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    if (!PRIVATE_KEY_HEX) {
      t.diagnostic("IROHA_TORII_INTEGRATION_PRIVATE_KEY_HEX is required for mutation coverage");
      return;
    }
    const privateKey = decodePrivateKeyHex(PRIVATE_KEY_HEX);
    const domainId = randomDomainId();
    const { signedTransaction, hash } = buildRegisterDomainTransaction({
      chainId: CHAIN_ID,
      authority: AUTHORITY_ACCOUNT_ID,
      domainId,
      metadata: {
        suite: "js-integration",
        timestamp: Date.now(),
      },
      privateKey,
    });

    const status = await client.submitTransactionAndWaitTyped(signedTransaction, {
      hashHex: hash.toString("hex"),
      intervalMs: 1_000,
      timeoutMs: 30_000,
    });
    assertSuccessfulStatus(status, domainId);

    let found = false;
    for await (const entry of client.iterateDomains({
      contains: domainId,
      limit: 5,
    })) {
      if (entry?.id === domainId) {
        found = true;
        break;
      }
    }
    assert.ok(found, `expected listDomains to include ${domainId}`);
  },
);

test(
  "pipeline status endpoints surface typed payloads",
  {
    skip: !!SKIP_REASON,
    timeout: 90_000,
  },
  async (t) => {
    if (!MUTATION_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_MUTATE=1 to enable mutation coverage");
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    if (!PRIVATE_KEY_HEX) {
      t.diagnostic("IROHA_TORII_INTEGRATION_PRIVATE_KEY_HEX is required for mutation coverage");
      return;
    }
    const privateKey = decodePrivateKeyHex(PRIVATE_KEY_HEX);
    const domainId = randomDomainId();
    const { signedTransaction, hash } = buildRegisterDomainTransaction({
      chainId: CHAIN_ID,
      authority: AUTHORITY_ACCOUNT_ID,
      domainId,
      metadata: {
        suite: "js-integration",
        step: "pipeline-status",
        timestamp: Date.now(),
      },
      privateKey,
    });
    const hashHex = hash.toString("hex");

    await client.submitTransaction(signedTransaction);

    const typedStatus = await client.waitForTransactionStatusTyped(hashHex, {
      intervalMs: 1_000,
      timeoutMs: 60_000,
    });
    assertSuccessfulStatus(typedStatus, domainId);
    assert.equal(
      typedStatus?.hashHex?.toLowerCase(),
      hashHex,
      "pipeline status payload should expose the submitted hash",
    );
    assert.ok(
      typedStatus?.status?.kind,
      "pipeline status payload should report a terminal status kind",
    );

    const fetchedStatus = await client.getTransactionStatusTyped(hashHex);
    assert.ok(
      fetchedStatus,
      "pipeline status endpoint should retain the terminal snapshot after polling",
    );
    assert.equal(
      fetchedStatus?.hashHex?.toLowerCase(),
      hashHex,
      "fetching the pipeline status should return the same transaction hash",
    );
    assert.equal(
      fetchedStatus?.status?.kind,
      typedStatus?.status?.kind,
      "fetching the pipeline status should preserve the terminal status kind",
    );
  },
);

test(
  "register account, mint asset, and inspect balances",
  {
    skip: !!SKIP_REASON,
    timeout: 120_000,
  },
  async (t) => {
    if (!MUTATION_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_MUTATE=1 to enable mutation coverage");
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    if (!PRIVATE_KEY_HEX) {
      t.diagnostic("IROHA_TORII_INTEGRATION_PRIVATE_KEY_HEX is required for mutation coverage");
      return;
    }
    const privateKey = decodePrivateKeyHex(PRIVATE_KEY_HEX);
    const domainId = randomDomainId();
    const { signedTransaction: domainTx, hash: domainHash } = buildRegisterDomainTransaction({
      chainId: CHAIN_ID,
      authority: AUTHORITY_ACCOUNT_ID,
      domainId,
      metadata: {
        suite: "js-integration",
        step: "account-asset",
        timestamp: Date.now(),
      },
      privateKey,
    });
    const domainStatus = await client.submitTransactionAndWaitTyped(domainTx, {
      hashHex: domainHash.toString("hex"),
      intervalMs: 1_000,
      timeoutMs: 30_000,
    });
    assertSuccessfulStatus(domainStatus, domainId);

    const accountId = randomAccountId(domainId);
    const { signedTransaction: accountTx, hash: accountHash } =
      buildRegisterAccountAndTransferTransaction({
        chainId: CHAIN_ID,
        authority: AUTHORITY_ACCOUNT_ID,
        account: {
          accountId,
          metadata: {
            suite: "js-integration",
            step: "account-asset",
          },
        },
        privateKey,
      });
    const accountStatus = await client.submitTransactionAndWaitTyped(accountTx, {
      hashHex: accountHash.toString("hex"),
      intervalMs: 1_000,
      timeoutMs: 30_000,
    });
    assertSuccessfulStatus(accountStatus, accountId);

    const assetDefinitionId = randomAssetDefinitionId(domainId);
    const assetId = `${assetDefinitionId}#${accountId}`;
    const { signedTransaction: assetTx, hash: assetHash } =
      buildRegisterAssetDefinitionAndMintTransaction({
        chainId: CHAIN_ID,
        authority: AUTHORITY_ACCOUNT_ID,
        assetDefinition: {
          assetDefinitionId,
          metadata: {
            suite: "js-integration",
            step: "asset-mint",
          },
        },
        mint: {
          accountId,
          quantity: "7",
        },
        privateKey,
      });
    const assetStatus = await client.submitTransactionAndWaitTyped(assetTx, {
      hashHex: assetHash.toString("hex"),
      intervalMs: 1_000,
      timeoutMs: 30_000,
    });
    assertSuccessfulStatus(assetStatus, assetDefinitionId);

    let definitionFound = false;
    for await (const definition of client.iterateAssetDefinitions({
      contains: assetDefinitionId,
      limit: 5,
    })) {
      if (definition?.id === assetDefinitionId) {
        definitionFound = true;
        break;
      }
    }
    assert.ok(definitionFound, `expected asset definitions to include ${assetDefinitionId}`);

    let assetFound = null;
    for await (const asset of client.iterateAccountAssets(accountId, { limit: 5 })) {
      if (asset?.asset_id === assetId) {
        assetFound = asset;
        break;
      }
    }
    assert.ok(assetFound, `expected ${accountId} to own ${assetId}`);
    assert.equal(assetFound.quantity, "7");

    const { signedTransaction: extraMintTx, hash: extraMintHash } =
      buildMintAssetTransaction({
        chainId: CHAIN_ID,
        authority: AUTHORITY_ACCOUNT_ID,
        assetId,
        quantity: "3",
        metadata: {
          suite: "js-integration",
          step: "asset-remint",
        },
        privateKey,
      });
    const extraMintStatus = await client.submitTransactionAndWaitTyped(extraMintTx, {
      hashHex: extraMintHash.toString("hex"),
      intervalMs: 1_000,
      timeoutMs: 30_000,
    });
    assertSuccessfulStatus(extraMintStatus, assetId);

    const refreshedAsset = await findAccountAsset(client, accountId, assetId);
    assert.ok(refreshedAsset, `expected refreshed asset snapshot for ${assetId}`);
    assert.equal(refreshedAsset.quantity, "10");

    const domainQueryFound = await iteratorIncludes(
      client.iterateDomainsQuery({ pageSize: 5, maxItems: 25 }),
      (domain) => domain?.id === domainId,
    );
    assert.ok(domainQueryFound, `expected domain query iterator to include ${domainId}`);

    const accountQueryFound = await iteratorIncludes(
      client.iterateAccountsQuery({ pageSize: 5, maxItems: 25 }),
      (account) => account?.id === accountId,
    );
    assert.ok(accountQueryFound, `expected account query iterator to include ${accountId}`);

    const assetDefinitionQueryFound = await iteratorIncludes(
      client.iterateAssetDefinitionsQuery({ pageSize: 5, maxItems: 25 }),
      (definition) => definition?.id === assetDefinitionId,
    );
    assert.ok(
      assetDefinitionQueryFound,
      `expected asset definition query iterator to include ${assetDefinitionId}`,
    );

    const accountTransactions = await client.listAccountTransactions(accountId, {
      limit: 5,
    });
    assert.ok(
      Array.isArray(accountTransactions?.items) && accountTransactions.items.length > 0,
      "account transactions list should include at least one entry",
    );

    const accountAssetQuery = await client.queryAccountAssets(accountId, {
      limit: 5,
      sort: [{ key: "quantity", order: "desc" }],
    });
    assert.ok(
      accountAssetQuery && typeof accountAssetQuery.total === "number",
      "account asset query response should expose totals",
    );
    assert.ok(
      Array.isArray(accountAssetQuery.items) && accountAssetQuery.items.length > 0,
      "account asset query response should include an items array",
    );
    assert.ok(
      accountAssetQuery.items.some((entry) => entry?.asset_id === assetId),
      `expected account asset query results to include ${assetId}`,
    );

    const assetQueryIteratorFound = await iteratorIncludes(
      client.iterateAccountAssetsQuery(accountId, { limit: 2, maxItems: 10 }),
      (entry) => entry?.asset_id === assetId,
    );
    assert.ok(
      assetQueryIteratorFound,
      `expected account asset query iterator to include ${assetId}`,
    );
  },
);

test(
  "transfer asset between accounts and verify balances",
  {
    skip: !!SKIP_REASON,
    timeout: 120_000,
  },
  async (t) => {
    if (!MUTATION_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_MUTATE=1 to enable mutation coverage");
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    if (!PRIVATE_KEY_HEX) {
      t.diagnostic("IROHA_TORII_INTEGRATION_PRIVATE_KEY_HEX is required for mutation coverage");
      return;
    }
    const privateKey = decodePrivateKeyHex(PRIVATE_KEY_HEX);
    const domainId = randomDomainId();
    const senderAccountId = randomAccountId(domainId);
    const receiverAccountId = randomAccountId(domainId);
    const assetDefinitionId = randomAssetDefinitionId(domainId);
    const senderAssetId = `${assetDefinitionId}#${senderAccountId}`;
    const receiverAssetId = `${assetDefinitionId}#${receiverAccountId}`;
    const mintedQuantity = "15";
    const transferQuantity = "6";
    const remainingQuantity = (BigInt(mintedQuantity) - BigInt(transferQuantity)).toString();

    const { signedTransaction: domainTx, hash: domainHash } = buildRegisterDomainTransaction({
      chainId: CHAIN_ID,
      authority: AUTHORITY_ACCOUNT_ID,
      domainId,
      metadata: {
        suite: "js-integration",
        step: "asset-transfer",
      },
      privateKey,
    });
    const domainStatus = await client.submitTransactionAndWaitTyped(domainTx, {
      hashHex: domainHash.toString("hex"),
      intervalMs: 1_000,
      timeoutMs: 30_000,
    });
    assertSuccessfulStatus(domainStatus, domainId);

    const { signedTransaction: senderTx, hash: senderHash } =
      buildRegisterAccountAndTransferTransaction({
        chainId: CHAIN_ID,
        authority: AUTHORITY_ACCOUNT_ID,
        account: {
          accountId: senderAccountId,
          metadata: {
            suite: "js-integration",
            step: "asset-transfer",
            role: "sender",
          },
        },
        privateKey,
      });
    const senderStatus = await client.submitTransactionAndWaitTyped(senderTx, {
      hashHex: senderHash.toString("hex"),
      intervalMs: 1_000,
      timeoutMs: 30_000,
    });
    assertSuccessfulStatus(senderStatus, senderAccountId);

    const { signedTransaction: receiverTx, hash: receiverHash } =
      buildRegisterAccountAndTransferTransaction({
        chainId: CHAIN_ID,
        authority: AUTHORITY_ACCOUNT_ID,
        account: {
          accountId: receiverAccountId,
          metadata: {
            suite: "js-integration",
            step: "asset-transfer",
            role: "receiver",
          },
        },
        privateKey,
      });
    const receiverStatus = await client.submitTransactionAndWaitTyped(receiverTx, {
      hashHex: receiverHash.toString("hex"),
      intervalMs: 1_000,
      timeoutMs: 30_000,
    });
    assertSuccessfulStatus(receiverStatus, receiverAccountId);

    const { signedTransaction: assetTx, hash: assetHash } =
      buildRegisterAssetDefinitionAndMintTransaction({
        chainId: CHAIN_ID,
        authority: AUTHORITY_ACCOUNT_ID,
        assetDefinition: {
          assetDefinitionId,
          metadata: {
            suite: "js-integration",
            step: "asset-transfer",
          },
        },
        mint: {
          accountId: senderAccountId,
          quantity: mintedQuantity,
        },
        privateKey,
      });
    const assetStatus = await client.submitTransactionAndWaitTyped(assetTx, {
      hashHex: assetHash.toString("hex"),
      intervalMs: 1_000,
      timeoutMs: 30_000,
    });
    assertSuccessfulStatus(assetStatus, assetDefinitionId);

    const mintedAsset = await findAccountAsset(client, senderAccountId, senderAssetId);
    assert.ok(mintedAsset, `expected minted asset snapshot for ${senderAssetId}`);
    assert.equal(mintedAsset.quantity, mintedQuantity);

    const { signedTransaction: transferTx, hash: transferHash } = buildTransferAssetTransaction({
      chainId: CHAIN_ID,
      authority: AUTHORITY_ACCOUNT_ID,
      sourceAssetId: senderAssetId,
      destinationAccountId: receiverAccountId,
      quantity: transferQuantity,
      metadata: {
        suite: "js-integration",
        step: "asset-transfer",
      },
      privateKey,
    });
    const transferStatus = await client.submitTransactionAndWaitTyped(transferTx, {
      hashHex: transferHash.toString("hex"),
      intervalMs: 1_000,
      timeoutMs: 30_000,
    });
    assertSuccessfulStatus(transferStatus, senderAssetId);

    const senderSnapshot = await findAccountAsset(client, senderAccountId, senderAssetId);
    assert.ok(senderSnapshot, `expected refreshed asset snapshot for ${senderAssetId}`);
    assert.equal(senderSnapshot.quantity, remainingQuantity);

    const receiverSnapshot = await findAccountAsset(client, receiverAccountId, receiverAssetId);
    assert.ok(receiverSnapshot, `expected receiver asset snapshot for ${receiverAssetId}`);
    assert.equal(receiverSnapshot.quantity, transferQuantity);

    const receiverAssets = await client.listAccountAssets(receiverAccountId, { limit: 5 });
    const receiverItems = receiverAssets?.items ?? [];
    assert.ok(Array.isArray(receiverItems), "account asset list should expose items");
    assert.ok(
      receiverItems.some((entry) => entry?.asset_id === receiverAssetId),
      `expected account asset list to include ${receiverAssetId}`,
    );

    const holderPage = await client.listAssetHolders(assetDefinitionId, { limit: 10 });
    assert.ok(
      holderPage && Array.isArray(holderPage.items),
      "asset holder list should return entries",
    );
    const senderHolder = holderPage.items.find(
      (entry) => entry?.account_id === senderAccountId,
    );
    assert.ok(senderHolder, `expected holders list to include ${senderAccountId}`);
    assert.equal(
      senderHolder.quantity,
      remainingQuantity,
      "sender holder entry must report remaining quantity",
    );
    const receiverHolder = holderPage.items.find(
      (entry) => entry?.account_id === receiverAccountId,
    );
    assert.ok(receiverHolder, `expected holders list to include ${receiverAccountId}`);
    assert.equal(
      receiverHolder.quantity,
      transferQuantity,
      "receiver holder entry must report transferred quantity",
    );

    const holderQuery = await client.queryAssetHolders(assetDefinitionId, {
      limit: 5,
      offset: 0,
      sort: [{ key: "quantity", order: "desc" }],
    });
    assert.ok(
      holderQuery && typeof holderQuery.total === "number",
      "asset holder query should return a payload with totals",
    );
    assert.ok(
      Array.isArray(holderQuery.items) && holderQuery.items.length > 0,
      "asset holder query should expose an items array",
    );
    const holderQueryIncludesReceiver = holderQuery.items.some(
      (entry) => entry?.account_id === receiverAccountId,
    );
    assert.ok(
      holderQueryIncludesReceiver,
      `asset holder query should include ${receiverAccountId}`,
    );

    const iteratorListFound = await iteratorIncludes(
      client.iterateAssetHolders(assetDefinitionId, { limit: 5 }),
      (entry) => entry?.account_id === receiverAccountId && entry.quantity === transferQuantity,
    );
    assert.ok(iteratorListFound, "asset holder iterator should surface the receiver account");

    const iteratorQueryFound = await iteratorIncludes(
      client.iterateAssetHoldersQuery(assetDefinitionId, { limit: 5 }),
      (entry) => entry?.account_id === senderAccountId && entry.quantity === remainingQuantity,
    );
    assert.ok(iteratorQueryFound, "asset holder query iterator should surface the sender account");

    const accountFilter = { Eq: { id: senderAccountId } };
    const senderCompressedLiteral = compressedLiteralForAccount(senderAccountId);
    const authorityCompressedLiteral = compressedLiteralForAccount(AUTHORITY_ACCOUNT_ID);
    const accountList = await client.listAccounts({ filter: accountFilter, limit: 5 });
    const accountListItems = accountList?.items ?? [];
    assert.ok(
      accountListItems.some((entry) => entry?.id === senderAccountId),
      "account list should surface the sender account",
    );

    const accountQueryPage = await client.queryAccounts({ filter: accountFilter, limit: 1 });
    const accountQueryItems = accountQueryPage?.items ?? [];
    assert.ok(
      accountQueryItems.length >= 1 && accountQueryItems[0]?.id === senderAccountId,
      "account query should return the sender account as the first entry",
    );

    const listIteratorFound = await iteratorIncludes(
      client.iterateAccounts({ filter: accountFilter, limit: 5 }),
      (entry) => entry?.id === senderAccountId,
    );
    assert.ok(listIteratorFound, "account iterator should include the sender account");

    const queryIteratorFound = await iteratorIncludes(
      client.iterateAccountsQuery({ filter: accountFilter, limit: 5 }),
      (entry) => entry?.id === senderAccountId,
    );
    assert.ok(
      queryIteratorFound,
      "account query iterator should include the sender account",
    );

    const compressedAccountList = await client.listAccounts({
      filter: accountFilter,
      limit: 5,
      addressFormat: "compressed",
    });
    const compressedAccountItems = compressedAccountList?.items ?? [];
    assert.ok(
      compressedAccountItems.some((entry) => entry?.id === senderCompressedLiteral),
      "account list should honour compressed address formatting",
    );

    const compressedAccountQuery = await client.queryAccounts({
      filter: accountFilter,
      limit: 1,
      addressFormat: "compressed",
    });
    const compressedQueryItems = compressedAccountQuery?.items ?? [];
    assert.ok(
      compressedQueryItems.length > 0 && compressedQueryItems[0]?.id === senderCompressedLiteral,
      "account query should return compressed literals when requested",
    );

    const compressedListIteratorFound = await iteratorIncludes(
      client.iterateAccounts({
        filter: accountFilter,
        limit: 5,
        maxItems: 10,
        addressFormat: "compressed",
      }),
      (entry) => entry?.id === senderCompressedLiteral,
    );
    assert.ok(
      compressedListIteratorFound,
      "account iterator should emit compressed literals when requested",
    );

    const compressedQueryIteratorFound = await iteratorIncludes(
      client.iterateAccountsQuery({
        filter: accountFilter,
        limit: 5,
        maxItems: 10,
        addressFormat: "compressed",
      }),
      (entry) => entry?.id === senderCompressedLiteral,
    );
    assert.ok(
      compressedQueryIteratorFound,
      "account query iterator should emit compressed literals when requested",
    );

    const compressedTransactions = await client.listAccountTransactions(senderAccountId, {
      limit: 5,
      addressFormat: "compressed",
    });
    const listAuthority = (compressedTransactions?.items ?? []).find(
      (entry) => typeof entry?.authority === "string",
    );
    assert.ok(
      listAuthority,
      "account transaction list should include an entry with an authority literal",
    );
    assert.equal(
      listAuthority.authority,
      authorityCompressedLiteral,
      "account transaction list should honour addressFormat=compressed for authority fields",
    );

    const compressedTransactionQuery = await client.queryAccountTransactions(senderAccountId, {
      limit: 5,
      addressFormat: "compressed",
    });
    const queryAuthority = (compressedTransactionQuery?.items ?? []).find(
      (entry) => typeof entry?.authority === "string",
    );
    assert.ok(
      queryAuthority,
      "account transaction query should include an entry with an authority literal",
    );
    assert.equal(
      queryAuthority.authority,
      authorityCompressedLiteral,
      "account transaction query should honour addressFormat=compressed for authority fields",
    );

    const compressedTransactionIteratorFound = await iteratorIncludes(
      client.iterateAccountTransactions(senderAccountId, {
        limit: 5,
        maxItems: 10,
        addressFormat: "compressed",
      }),
      (entry) => entry?.authority === authorityCompressedLiteral,
    );
    assert.ok(
      compressedTransactionIteratorFound,
      "account transaction iterator should emit compressed authority literals",
    );

    const compressedTransactionQueryIteratorFound = await iteratorIncludes(
      client.iterateAccountTransactionsQuery(senderAccountId, {
        limit: 5,
        maxItems: 10,
        addressFormat: "compressed",
      }),
      (entry) => entry?.authority === authorityCompressedLiteral,
    );
    assert.ok(
      compressedTransactionQueryIteratorFound,
      "account transaction query iterator should emit compressed authority literals",
    );
  },
);

test(
  "repo agreement list and query endpoints return typed payloads (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });

    let listPage;
    try {
      listPage = await client.listRepoAgreements({ limit: 5 });
    } catch (error) {
      if (isUnexpectedNotFoundError(error)) {
        t.diagnostic(
          `repo agreement list endpoint unavailable: ${
            error instanceof Error ? error.message : String(error)
          }`,
        );
        return;
      }
      throw error;
    }

    assertNonNegativeInteger(listPage.total, "repo agreement list total");
    assert.ok(Array.isArray(listPage.items), "repo agreement list must expose items array");

    if (listPage.items.length === 0) {
      t.diagnostic("Torii repo agreement list returned zero entries; skipping field assertions");
      return;
    }

    const agreement = listPage.items[0];
    assertRepoAgreementSnapshot(agreement, "repo agreement list entry");

    const repoFilter = { Eq: { id: agreement.id } };
    let queryPage;
    try {
      queryPage = await client.queryRepoAgreements({ filter: repoFilter, limit: 1 });
    } catch (error) {
      if (isUnexpectedNotFoundError(error)) {
        t.diagnostic(
          `repo agreement query endpoint unavailable: ${
            error instanceof Error ? error.message : String(error)
          }`,
        );
        return;
      }
      throw error;
    }

    assertNonNegativeInteger(queryPage.total, "repo agreement query total");
    assert.ok(Array.isArray(queryPage.items), "repo agreement query must expose items array");
    assert.ok(
      queryPage.items.some((entry) => entry?.id === agreement.id),
      "repo agreement query should include the filtered agreement",
    );

    const iteratorListFound = await iteratorIncludes(
      client.iterateRepoAgreements({ filter: repoFilter, limit: 5 }),
      (entry) => entry?.id === agreement.id,
    );
    assert.ok(iteratorListFound, "repo agreement iterator should surface the filtered agreement");

    const iteratorQueryFound = await iteratorIncludes(
      client.iterateRepoAgreementsQuery({ filter: repoFilter, limit: 5 }),
      (entry) => entry?.id === agreement.id,
    );
    assert.ok(
      iteratorQueryFound,
      "repo agreement query iterator should surface the filtered agreement",
    );
  },
);

test(
  "attachment upload/list/fetch/delete lifecycle (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (!MUTATION_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_MUTATE=1 to enable attachment coverage");
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });

    const attachmentPayload = Buffer.from(
      `iroha-js-attachment-${Date.now().toString(16)}-${Math.random().toString(16).slice(2)}`,
      "utf8",
    );

    let uploaded;
    try {
      uploaded = await client.uploadAttachment(attachmentPayload, {
        contentType: "text/plain",
      });
    } catch (error) {
      if (isAttachmentEndpointUnavailable(error)) {
        t.diagnostic(
          `attachment upload endpoint unavailable on target node: ${
            error instanceof Error ? error.message : String(error)
          }`,
        );
        return;
      }
      throw error;
    }
    assertAttachmentMetadata(uploaded, "attachment upload metadata");

    let attachmentList;
    try {
      attachmentList = await client.listAttachments();
    } catch (error) {
      if (isAttachmentEndpointUnavailable(error)) {
        t.diagnostic(
          `attachment list endpoint unavailable on target node: ${
            error instanceof Error ? error.message : String(error)
          }`,
        );
        return;
      }
      throw error;
    }
    assert.ok(Array.isArray(attachmentList), "listAttachments must return an array");

    const listedEntry = attachmentList.find((entry) => entry?.id === uploaded.id);
    if (listedEntry) {
      assertAttachmentMetadata(listedEntry, "listed attachment metadata");
    } else {
      t.diagnostic("uploaded attachment not yet visible in list; continuing with direct fetch");
    }

    let fetched;
    try {
      fetched = await client.getAttachment(uploaded.id);
    } catch (error) {
      if (isAttachmentEndpointUnavailable(error)) {
        t.diagnostic(
          `attachment fetch endpoint unavailable on target node: ${
            error instanceof Error ? error.message : String(error)
          }`,
        );
        return;
      }
      throw error;
    }
    assert.ok(Buffer.isBuffer(fetched.data), "getAttachment should return a Buffer");
    assert.equal(
      fetched.contentType,
      uploaded.contentType,
      "getAttachment should preserve content type metadata",
    );
    assert.equal(
      fetched.data.toString("utf8"),
      attachmentPayload.toString("utf8"),
      "attachment payload should round-trip exactly",
    );

    try {
      await client.deleteAttachment(uploaded.id);
    } catch (error) {
      if (isAttachmentEndpointUnavailable(error)) {
        t.diagnostic(
          `attachment delete endpoint unavailable on target node: ${
            error instanceof Error ? error.message : String(error)
          }`,
        );
        return;
      }
      throw error;
    }

    const deleted = await waitForAttachmentDeletion(client, uploaded.id);
    if (!deleted) {
      t.diagnostic(
        `attachment ${uploaded.id} still retrievable after delete; retention policy may delay cleanup`,
      );
    }
  },
);

test(
  "trigger list, lookup, and query endpoints return typed payloads (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });

    let listPage;
    try {
      listPage = await client.listTriggers({ limit: 5 });
    } catch (error) {
      if (shouldSkipTriggerEndpoints(error)) {
        t.diagnostic(
          `trigger list endpoint unavailable: ${
            error instanceof Error ? error.message : String(error)
          }`,
        );
        return;
      }
      throw error;
    }

    assertNonNegativeInteger(listPage.total, "trigger list total");
    assert.ok(Array.isArray(listPage.items), "trigger list must expose items array");

    if (listPage.items.length === 0) {
      let emptyQuery;
      try {
        emptyQuery = await client.queryTriggers({ limit: 1 });
      } catch (error) {
        if (shouldSkipTriggerEndpoints(error)) {
          t.diagnostic(
            `trigger query endpoint unavailable: ${
              error instanceof Error ? error.message : String(error)
            }`,
          );
          return;
        }
        throw error;
      }
      assertNonNegativeInteger(emptyQuery.total, "trigger query total");
      assert.ok(Array.isArray(emptyQuery.items), "trigger query must expose items array");
      t.diagnostic("trigger list returned zero entries; skipping lookup assertions");
      return;
    }

    const example = listPage.items[0];
    assertTriggerRecord(example, "trigger list entry");

    let lookedUp;
    try {
      lookedUp = await client.getTrigger(example.id);
    } catch (error) {
      if (shouldSkipTriggerEndpoints(error)) {
        t.diagnostic(
          `trigger lookup endpoint unavailable: ${
            error instanceof Error ? error.message : String(error)
          }`,
        );
        return;
      }
      throw error;
    }
    if (!lookedUp) {
      t.diagnostic(`trigger ${example.id} disappeared before lookup finished`);
      return;
    }
    assertTriggerRecord(lookedUp, "trigger lookup response");

    let queryPage;
    try {
      queryPage = await client.queryTriggers({
        filter: { Eq: ["id", example.id] },
        limit: 1,
      });
    } catch (error) {
      if (shouldSkipTriggerEndpoints(error)) {
        t.diagnostic(
          `trigger query endpoint unavailable: ${
            error instanceof Error ? error.message : String(error)
          }`,
        );
        return;
      }
      throw error;
    }

    assertNonNegativeInteger(queryPage.total, "trigger query total");
    assert.ok(Array.isArray(queryPage.items), "trigger query must expose items array");
    assert.ok(
      queryPage.items.some((entry) => entry?.id === example.id),
      "trigger query should include the filtered trigger id",
    );
  },
);

test(
  "register trigger lifecycle via Torii (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 90_000,
  },
  async (t) => {
    if (!MUTATION_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_MUTATE=1 to enable mutation coverage");
      return;
    }
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const triggerId = randomTriggerId();
    const namespace = "apps";
    const assetId = `rose#wonderland#${AUTHORITY_ACCOUNT_ID}`;
    const action = buildTimeTriggerAction({
      authority: AUTHORITY_ACCOUNT_ID,
      instructions: [
        buildMintAssetInstruction({
          assetId,
          quantity: "1",
        }),
      ],
      startTimestampMs: Date.now() + 5_000,
      periodMs: 60_000,
      repeats: 1,
      metadata: { label: "js-integration" },
    });
    const metadata = {
      suite: "js-integration",
      intent: "trigger-lifecycle",
      timestamp: Date.now(),
    };

    let created = false;
    try {
      const draft = await client.registerTriggerTyped({
        id: triggerId,
        namespace,
        action,
        metadata,
      });
      created = true;
      assert.ok(draft);
      assert.equal(draft.trigger_id, triggerId);
      assert.ok(
        Array.isArray(draft.tx_instructions),
        "trigger registration should surface Torii tx instructions",
      );

      const record = await client.getTrigger(triggerId);
      assert.ok(record, "trigger lookup should return a payload for the newly registered id");
      assertTriggerRecord(record, "registered trigger lookup");
      assert.equal(record.id, triggerId);
      assert.equal(record.metadata.intent, metadata.intent);

      const page = await client.queryTriggers({
        filter: { Eq: ["id", triggerId] },
        limit: 1,
      });
      assertNonNegativeInteger(page.total, "trigger query total after registration");
      assert.ok(Array.isArray(page.items), "trigger query must expose items array");
      assert.ok(
        page.items.some((entry) => entry?.id === triggerId),
        "trigger query should return the newly registered trigger",
      );
    } catch (error) {
      if (shouldSkipTriggerEndpoints(error)) {
        t.diagnostic(
          `trigger mutation endpoints unavailable: ${
            error instanceof Error ? error.message : String(error)
          }`,
        );
        return;
      }
      throw error;
    } finally {
      if (created) {
        try {
          await client.deleteTrigger(triggerId);
        } catch (cleanupError) {
          const message =
            cleanupError instanceof Error ? cleanupError.message : String(cleanupError);
          t.diagnostic(`failed to delete trigger ${triggerId} during cleanup: ${message}`);
        }
      }
    }
  },
);

test(
  "pipeline block streaming emits events (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 90_000,
  },
  async (t) => {
    if (!STREAM_ENABLED) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_STREAM_ENABLED=1 to exercise event-stream coverage",
      );
      return;
    }
    if (!MUTATION_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_MUTATE=1 to enable mutation coverage");
      return;
    }
    if (!PRIVATE_KEY_HEX) {
      t.diagnostic("IROHA_TORII_INTEGRATION_PRIVATE_KEY_HEX is required for mutation coverage");
      return;
    }

    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const privateKey = decodePrivateKeyHex(PRIVATE_KEY_HEX);
    const domainId = randomDomainId();
    const controller = new AbortController();

    const blockEventPromise = waitForPipelineBlockEvent(client, controller.signal);
    const { signedTransaction, hash } = buildRegisterDomainTransaction({
      chainId: CHAIN_ID,
      authority: AUTHORITY_ACCOUNT_ID,
      domainId,
      metadata: {
        suite: "js-integration",
        step: "event-stream",
        timestamp: Date.now(),
      },
      privateKey,
    });
    const statusPromise = client.submitTransactionAndWaitTyped(signedTransaction, {
      hashHex: hash.toString("hex"),
      intervalMs: 1_000,
      timeoutMs: 30_000,
    });

    let blockEvent;
    try {
      blockEvent = await withTimeout(
        blockEventPromise,
        45_000,
        "timed out waiting for a pipeline block event",
      );
    } finally {
      controller.abort();
    }

    const status = await statusPromise;
    assertSuccessfulStatus(status, domainId);
    assertPipelineBlockEvent(blockEvent);
  },
);

test(
  "sumeragi status streaming emits consensus snapshots (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 90_000,
  },
  async (t) => {
    if (!STREAM_ENABLED) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_STREAM_ENABLED=1 to exercise stream coverage",
      );
      return;
    }
    if (!MUTATION_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_MUTATE=1 to enable mutation coverage");
      return;
    }
    if (!PRIVATE_KEY_HEX) {
      t.diagnostic("IROHA_TORII_INTEGRATION_PRIVATE_KEY_HEX is required for mutation coverage");
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const privateKey = decodePrivateKeyHex(PRIVATE_KEY_HEX);
    const controller = new AbortController();
    const domainId = randomDomainId();
    const streamPromise = waitForSumeragiStatusEvent(client, controller.signal);
    const { signedTransaction, hash } = buildRegisterDomainTransaction({
      chainId: CHAIN_ID,
      authority: AUTHORITY_ACCOUNT_ID,
      domainId,
      metadata: {
        suite: "js-integration",
        step: "sumeragi-stream",
        timestamp: Date.now(),
      },
      privateKey,
    });
    const statusPromise = client.submitTransactionAndWaitTyped(signedTransaction, {
      hashHex: hash.toString("hex"),
      intervalMs: 1_000,
      timeoutMs: 30_000,
    });

    let event;
    try {
      event = await withTimeout(
        streamPromise,
        45_000,
        "timed out waiting for a Sumeragi status event",
      );
    } finally {
      controller.abort();
      const submissionStatus = await statusPromise;
      assertSuccessfulStatus(submissionStatus, domainId);
    }
    assertSumeragiStatusEvent(event);
  },
);

test(
  "account transaction listings surface recent submissions",
  {
    skip: !!SKIP_REASON,
    timeout: 120_000,
  },
  async (t) => {
    if (!MUTATION_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_MUTATE=1 to enable mutation coverage");
      return;
    }
    if (!PRIVATE_KEY_HEX) {
      t.diagnostic("IROHA_TORII_INTEGRATION_PRIVATE_KEY_HEX is required for mutation coverage");
      return;
    }

    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const privateKey = decodePrivateKeyHex(PRIVATE_KEY_HEX);
    const domainId = randomDomainId();
    const { signedTransaction, hash } = buildRegisterDomainTransaction({
      chainId: CHAIN_ID,
      authority: AUTHORITY_ACCOUNT_ID,
      domainId,
      metadata: {
        suite: "js-integration",
        step: "account-transactions",
        timestamp: Date.now(),
      },
      privateKey,
    });
    const hashHex = hash.toString("hex");

    const status = await client.submitTransactionAndWaitTyped(signedTransaction, {
      hashHex,
      intervalMs: 1_000,
      timeoutMs: 30_000,
    });
    assertSuccessfulStatus(status, domainId);

    const matchesHash = (entry) =>
      !!entry &&
      typeof entry.entrypoint_hash === "string" &&
      entry.entrypoint_hash.toLowerCase() === hashHex;

    const waitForMatch = async (fetchPage, context) => {
      for (let attempt = 0; attempt < 5; attempt += 1) {
        const page = await fetchPage();
        assert.ok(page, `${context} should return a payload`);
        assert.equal(typeof page.total, "number", `${context} should expose total count`);
        const items = page.items ?? [];
        assert.ok(Array.isArray(items), `${context} should expose an items array`);
        const entry = items.find(matchesHash);
        if (entry) {
          return entry;
        }
        await delay(1_000);
      }
      throw new Error(`${context} did not include transaction ${hashHex}`);
    };

    const listEntry = await waitForMatch(
      () => client.listAccountTransactions(AUTHORITY_ACCOUNT_ID, { limit: 25 }),
      "listAccountTransactions",
    );
    assert.equal(
      listEntry.entrypoint_hash.toLowerCase(),
      hashHex,
      "account transactions list should echo the entrypoint hash",
    );
    if (listEntry.authority) {
      assert.equal(
        listEntry.authority,
        AUTHORITY_ACCOUNT_ID,
        "account transactions entry should report the authority",
      );
    }
    assert.equal(listEntry.result_ok, true, "account transactions entry should report success");

    const iteratorFound = await iteratorIncludes(
      client.iterateAccountTransactions(AUTHORITY_ACCOUNT_ID, { limit: 5 }),
      matchesHash,
    );
    assert.ok(iteratorFound, "account transaction iterator should surface the submission");

    const queryEntry = await waitForMatch(
      () =>
        client.queryAccountTransactions(AUTHORITY_ACCOUNT_ID, {
          limit: 10,
          offset: 0,
          sort: [{ key: "timestamp_ms", order: "desc" }],
        }),
      "queryAccountTransactions",
    );
    assert.equal(
      queryEntry.entrypoint_hash.toLowerCase(),
      hashHex,
      "account transactions query should echo the entrypoint hash",
    );

    const iteratorQueryFound = await iteratorIncludes(
      client.iterateAccountTransactionsQuery(AUTHORITY_ACCOUNT_ID, { limit: 5 }),
      matchesHash,
    );
    assert.ok(
      iteratorQueryFound,
      "account transaction query iterator should surface the submission",
    );
  },
);

test(
  "Sumeragi RBC telemetry endpoints respond (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 90_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    let snapshot;
    try {
      snapshot = await client.getSumeragiRbc();
    } catch (error) {
      t.diagnostic(
        `RBC snapshot endpoint unavailable: ${error instanceof Error ? error.message : String(error)}`,
      );
      return;
    }
    assert.equal(typeof snapshot.sessionsActive, "number");
    assert.equal(typeof snapshot.sessionsPrunedTotal, "number");
    assert.equal(typeof snapshot.readyBroadcastsTotal, "number");
    assert.equal(typeof snapshot.deliverBroadcastsTotal, "number");
    assert.equal(typeof snapshot.payloadBytesDeliveredTotal, "number");

    let sessionSnapshot;
    try {
      sessionSnapshot = await client.getSumeragiRbcSessions();
    } catch (error) {
      t.diagnostic(
        `RBC sessions endpoint unavailable: ${error instanceof Error ? error.message : String(error)}`,
      );
      return;
    }
    assert.equal(typeof sessionSnapshot.sessionsActive, "number");
    assert.ok(Array.isArray(sessionSnapshot.items), "RBC session list must expose an items array");
    if (sessionSnapshot.items.length === 0) {
      t.diagnostic("RBC sessions endpoint returned no active sessions");
      return;
    }
    const sampleSession = sessionSnapshot.items[0];
    assert.equal(typeof sampleSession.height, "number");
    assert.equal(typeof sampleSession.view, "number");
    assert.equal(typeof sampleSession.totalChunks, "number");
    assert.equal(typeof sampleSession.receivedChunks, "number");
    assert.equal(typeof sampleSession.readyCount, "number");
    assert.equal(typeof sampleSession.delivered, "boolean");
    assert.equal(typeof sampleSession.invalid, "boolean");
    assert.equal(typeof sampleSession.recovered, "boolean");
    if (sampleSession.blockHash !== null) {
      assert.ok(sampleSession.blockHash.length > 0, "RBC session block hash must be non-empty");
    }
    if (sampleSession.payloadHash !== null) {
      assert.ok(sampleSession.payloadHash.length > 0, "RBC session payload hash must be non-empty");
    }

    const deliveredCandidate = sessionSnapshot.items.find(
      (session) => session.delivered && session.blockHash,
    );
    if (!deliveredCandidate) {
      t.diagnostic("RBC sessions list does not include a delivered session with block hash");
      return;
    }
    let delivery;
    try {
      delivery = await client.getSumeragiRbcDelivered(
        deliveredCandidate.height,
        deliveredCandidate.view,
      );
    } catch (error) {
      t.diagnostic(
        `RBC delivered endpoint unavailable: ${error instanceof Error ? error.message : String(error)}`,
      );
      return;
    }
    if (!delivery) {
      t.diagnostic("RBC delivered endpoint returned no payload for selected session");
      return;
    }
    assert.equal(delivery.height, deliveredCandidate.height);
    assert.equal(delivery.view, deliveredCandidate.view);
    assert.equal(typeof delivery.delivered, "boolean");
    assert.equal(typeof delivery.present, "boolean");
    assert.equal(typeof delivery.readyCount, "number");
    assert.equal(typeof delivery.receivedChunks, "number");
    assert.equal(typeof delivery.totalChunks, "number");
    if (delivery.blockHash !== null) {
      assert.ok(
        delivery.blockHash.length > 0,
        "RBC delivered payload must include a non-empty block hash when present",
      );
    }
  },
);
test(
  "sample RBC chunks returns typed payload (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 90_000,
  },
  async (t) => {
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const { sample: sampleOptions, reason } = await resolveRbcSampleOptions(client);
    if (!sampleOptions) {
      if (reason) {
        t.diagnostic(reason);
      }
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_RBC_SAMPLE={\"blockHash\":\"...\",\"height\":1,\"view\":0} to exercise RBC sampling coverage",
      );
      return;
    }
    const { blockHash, height, view } = sampleOptions;
    if (!blockHash || height === undefined || view === undefined) {
      t.diagnostic("RBC sample payload must include blockHash, height, and view");
      return;
    }
    if (!RBC_SAMPLE_OPTIONS) {
      t.diagnostic(
        `auto-selected RBC session height=${height} view=${view} (block ${blockHash.slice(0, 12)}…)`,
      );
    }
    const sampleRequest = { ...sampleOptions };
    if (!sampleRequest.apiToken && API_TOKEN) {
      sampleRequest.apiToken = API_TOKEN;
    }
    let sample;
    try {
      sample = await client.sampleRbcChunks(sampleRequest);
    } catch (error) {
      t.diagnostic(`sampleRbcChunks failed: ${error instanceof Error ? error.message : String(error)}`);
      throw error;
    }
    if (!sample) {
      t.diagnostic("Torii returned no RBC sample for the supplied payload");
      return;
    }
    assertHexString(sample.blockHash, "sample.blockHash");
    assertHexString(sample.chunkRoot, "sample.chunkRoot");
    if (sample.payloadHash !== null) {
      assertHexString(sample.payloadHash, "sample.payloadHash");
    }
    assertNonNegativeInteger(sample.height, "sample.height");
    assertNonNegativeInteger(sample.view, "sample.view");
    assertNonNegativeInteger(sample.totalChunks, "sample.totalChunks");
    assert.ok(
      Array.isArray(sample.samples) && sample.samples.length > 0,
      "expected RBC sample response to include chunk proofs",
    );
    sample.samples.forEach((entry, index) => {
      assertNonNegativeInteger(entry.index, `sample.samples[${index}].index`);
      assertHexString(entry.chunkHex, `sample.samples[${index}].chunkHex`);
      assertHexString(entry.digestHex, `sample.samples[${index}].digestHex`);
      assert.ok(entry.proof, `sample.samples[${index}].proof must be present`);
      assertNonNegativeInteger(
        entry.proof.leafIndex,
        `sample.samples[${index}].proof.leafIndex`,
      );
      if (entry.proof.depth !== null) {
        assertNonNegativeInteger(
          entry.proof.depth,
          `sample.samples[${index}].proof.depth`,
        );
      }
      assert.ok(
        Array.isArray(entry.proof.auditPath),
        `sample.samples[${index}].proof.auditPath must be an array`,
      );
      entry.proof.auditPath.forEach((value, auditIndex) => {
        if (value === null) {
          return;
        }
        assertHexString(
          value,
          `sample.samples[${index}].proof.auditPath[${auditIndex}]`,
        );
      });
    });
  },
);

test(
  "verifying key registry endpoints respond (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    let summaries;
    try {
      summaries = await client.listVerifyingKeysTyped();
    } catch (error) {
      t.diagnostic(
        `verifying key list endpoint unavailable: ${error instanceof Error ? error.message : String(error)}`,
      );
      return;
    }
    assert.ok(Array.isArray(summaries), "verifying key listing must return an array");
    if (summaries.length === 0) {
      t.diagnostic("verifying key registry returned no entries; skipping detail coverage");
      return;
    }
    const firstSummary = summaries[0];
    assertVerifyingKeyListEntry(firstSummary, "verifying key summary[0]");
    const entryWithRecord = firstSummary.record
      ? firstSummary
      : summaries.find((entry) => entry && entry.record);
    if (!entryWithRecord) {
      t.diagnostic("verifying key list did not include metadata payloads; skipping detail coverage");
      return;
    }
    const targetId = entryWithRecord.id;
    let detail;
    try {
      detail = await client.getVerifyingKeyTyped(targetId.backend, targetId.name);
    } catch (error) {
      t.diagnostic(
        `verifying key detail endpoint unavailable for ${targetId.backend}/${targetId.name}: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      return;
    }
    assertVerifyingKeyDetail(detail, targetId);
  },
);

test(
  "kaigi relay endpoints respond (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    let relayList;
    try {
      relayList = await client.listKaigiRelays();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      t.diagnostic(`kaigi relay list unavailable on target node: ${message}`);
      return;
    }
    assertKaigiRelaySummaryList(relayList);

    let healthSnapshot;
    try {
      healthSnapshot = await client.getKaigiRelaysHealth();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      t.diagnostic(`kaigi relay health endpoint unavailable on target node: ${message}`);
      return;
    }
    assertKaigiRelayHealthSnapshot(healthSnapshot);

    if (relayList.items.length === 0) {
      t.diagnostic("kaigi relay list returned no entries; skipping relay detail coverage");
      return;
    }

    const relayId = relayList.items[0].relay_id;
    let relayDetail;
    try {
      relayDetail = await client.getKaigiRelay(relayId);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      t.diagnostic(`kaigi relay detail fetch failed for ${relayId}: ${message}`);
      return;
    }
    if (!relayDetail) {
      t.diagnostic(`kaigi relay detail returned null for ${relayId}`);
      return;
    }
    assertKaigiRelayDetail(relayDetail, relayId);
  },
);

test(
  "offline allowance, summary, and transfer endpoints respond (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });

    const allowances = await fetchOfflineListPage(
      t,
      "offline allowances list",
      () => client.listOfflineAllowances({ limit: 5 }),
    );
    if (!allowances) {
      return;
    }
    assertOfflineAllowanceResponse(allowances);

    const summaries = await fetchOfflineListPage(
      t,
      "offline summaries list",
      () => client.listOfflineSummaries({ limit: 5 }),
    );
    if (!summaries) {
      return;
    }
    assertOfflineSummaryResponse(summaries);

    const transfers = await fetchOfflineListPage(
      t,
      "offline transfers list",
      () =>
        client.listOfflineTransfers({
          limit: 5,
          addressFormat: "compressed",
        }),
    );
    if (!transfers) {
      return;
    }
    assertOfflineTransferResponse(transfers);
  },
);

test(
  "offline revocation endpoints respond (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });

    const revocations = await fetchOfflineListPage(
      t,
      "offline revocations list",
      () => client.listOfflineRevocations({ limit: 5 }),
    );
    if (!revocations) {
      return;
    }
    assertOfflineRevocationResponse(revocations);

    const revocationQuery = await fetchOfflineListPage(
      t,
      "offline revocations query",
      () => client.queryOfflineRevocations({ limit: 5 }),
    );
    if (!revocationQuery) {
      return;
    }
    assertOfflineRevocationResponse(revocationQuery);
  },
);

test(
  "offline rejection telemetry responds (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    let stats;
    try {
      stats = await client.getOfflineRejectionStats();
    } catch (error) {
      if (isOfflineApiUnavailableError(error)) {
        t.diagnostic(`offline rejection telemetry unavailable on target node: ${error.message}`);
        return;
      }
      throw error;
    }
    if (!stats) {
      t.diagnostic("offline rejection telemetry disabled on target node");
      return;
    }
    assertOfflineRejectionStatsResponse(stats);
  },
);

test(
  "call contract entrypoint via Torii (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 120_000,
  },
  async (t) => {
    if (!MUTATION_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_MUTATE=1 to enable contract call coverage");
      return;
    }
    if (!CONTRACT_CALL_OPTIONS) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_CONTRACT_CALL to a JSON object to exercise contract call coverage",
      );
      return;
    }
    if (!PRIVATE_KEY_HEX) {
      t.diagnostic(
        "IROHA_TORII_INTEGRATION_PRIVATE_KEY_HEX is required for contract call coverage",
      );
      return;
    }

    const namespace = (CONTRACT_CALL_OPTIONS.namespace ?? "").trim();
    const contractId = (
      CONTRACT_CALL_OPTIONS.contractId ??
      CONTRACT_CALL_OPTIONS.contract_id ??
      ""
    ).trim();
    if (!namespace || !contractId) {
      t.diagnostic(
        "contract call payload must include non-empty `namespace` and `contractId` fields",
      );
      return;
    }

    const authority =
      CONTRACT_CALL_OPTIONS.authority ??
      CONTRACT_CALL_OPTIONS.account ??
      AUTHORITY_ACCOUNT_ID;
    const privateKeyHex =
      CONTRACT_CALL_OPTIONS.privateKeyHex ??
      CONTRACT_CALL_OPTIONS.private_key ??
      PRIVATE_KEY_HEX;

    const request = {
      authority,
      privateKey: privateKeyHex,
      namespace,
      contractId,
    };

    const entrypoint =
      CONTRACT_CALL_OPTIONS.entrypoint ?? CONTRACT_CALL_OPTIONS.entryPoint ?? null;
    if (isNonEmptyString(entrypoint)) {
      request.entrypoint = entrypoint;
    }
    if (Object.prototype.hasOwnProperty.call(CONTRACT_CALL_OPTIONS, "payload")) {
      request.payload = CONTRACT_CALL_OPTIONS.payload;
    }
    const gasAssetId =
      CONTRACT_CALL_OPTIONS.gasAssetId ?? CONTRACT_CALL_OPTIONS.gas_asset_id ?? null;
    if (isNonEmptyString(gasAssetId)) {
      request.gasAssetId = gasAssetId;
    }
    const gasLimitRaw =
      CONTRACT_CALL_OPTIONS.gasLimit ?? CONTRACT_CALL_OPTIONS.gas_limit ?? null;
    let gasLimit = null;
    if (
      typeof gasLimitRaw === "number" &&
      Number.isFinite(gasLimitRaw) &&
      Number.isInteger(gasLimitRaw)
    ) {
      gasLimit = gasLimitRaw;
    } else if (typeof gasLimitRaw === "string") {
      const trimmed = gasLimitRaw.trim();
      if (/^[0-9]+$/.test(trimmed)) {
        gasLimit = trimmed;
      }
    }
    if (gasLimit === null) {
      t.diagnostic(
        "contract call payload must include numeric `gasLimit`/`gas_limit` field",
      );
      return;
    }
    request.gasLimit = gasLimit;

    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });

    let response;
    try {
      response = await client.callContract(request);
    } catch (error) {
      t.diagnostic(
        `callContract failed: ${error instanceof Error ? error.message : String(error)}`,
      );
      throw error;
    }
    assert.ok(response, "contract call should return a response payload");
    assert.equal(
      response.namespace,
      namespace,
      "contract call response must echo namespace",
    );
    assert.equal(
      response.contract_id,
      contractId,
      "contract call response must echo contract id",
    );
    assertHexString(response.tx_hash_hex, "contract call response.tx_hash_hex");
    assertHexString(response.code_hash_hex, "contract call response.code_hash_hex");
    assertHexString(response.abi_hash_hex, "contract call response.abi_hash_hex");

    const status = await client.waitForTransactionStatusTyped(response.tx_hash_hex, {
      intervalMs: 1_000,
      timeoutMs: 30_000,
    });
    assert.ok(status, "waitForTransactionStatusTyped must return a payload");
    assert.ok(
      SUCCESS_STATUSES.has(status.status),
      `expected contract call to succeed but observed status=${status.status}`,
    );
  },
);

test(
  "pipeline recovery sidecar exposes DAG + tx metadata (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const blocksPage = await client.listBlocks({ limit: 5 });
    const candidateHeights = (blocksPage?.items ?? [])
      .map((entry) => entry?.height)
      .filter(
        (value) =>
          (Number.isFinite(value) && value >= 0) ||
          (typeof value === "bigint" && value >= 0n),
      );
    if (candidateHeights.length === 0) {
      t.diagnostic("no block heights available for pipeline recovery sampling");
      return;
    }

    const attemptedHeights = [];
    for (const height of candidateHeights) {
      attemptedHeights.push(height);
      const recovery = await client.getPipelineRecoveryTyped(height);
      if (!recovery) {
        continue;
      }
      assert.ok(
        typeof recovery.format === "string" && recovery.format.length > 0,
        "pipeline recovery format must be a non-empty string",
      );
      assertNonNegativeInteger(recovery.height, "pipeline recovery height");
      assert.ok(
        recovery.dag && typeof recovery.dag === "object",
        "pipeline recovery must include a dag payload",
      );
      assertHexString(recovery.dag.fingerprintHex, "pipeline recovery dag fingerprint");
      assertNonNegativeInteger(recovery.dag.keyCount, "pipeline recovery dag keyCount");
      assert.ok(Array.isArray(recovery.txs), "pipeline recovery txs must be an array");
      recovery.txs.forEach((tx, index) => {
        assertHexString(tx.hashHex, `pipeline recovery txs[${index}].hashHex`);
        assert.ok(Array.isArray(tx.reads), `pipeline recovery txs[${index}].reads must be an array`);
        tx.reads.forEach((key, keyIndex) => {
          assert.ok(
            isNonEmptyString(key),
            `pipeline recovery txs[${index}].reads[${keyIndex}] must be a non-empty string`,
          );
        });
        assert.ok(Array.isArray(tx.writes), `pipeline recovery txs[${index}].writes must be an array`);
        tx.writes.forEach((key, keyIndex) => {
          assert.ok(
            isNonEmptyString(key),
            `pipeline recovery txs[${index}].writes[${keyIndex}] must be a non-empty string`,
          );
        });
      });
      if (recovery.txs.length === 0) {
        t.diagnostic(`pipeline recovery sidecar for height ${height} returned zero transactions`);
      }
      return;
    }
    t.diagnostic(
      `pipeline recovery endpoint did not expose artefacts for sampled heights: ${attemptedHeights.join(
        ", ",
      )}`,
    );
  },
);

test(
  "SoraFS registry endpoints respond (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 90_000,
  },
  async (t) => {
    if (!SORAFS_ENABLED) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_SORAFS_ENABLED=1 to exercise SoraFS registry/storage coverage",
      );
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    let pinList;
    try {
      pinList = await client.listSorafsPinManifests({ limit: 5 });
    } catch (error) {
      if (isSorafsUnavailableError(error)) {
        t.diagnostic(`SoraFS pin registry unavailable on target node: ${error.message}`);
        return;
      }
      throw error;
    }
    assert.ok(Array.isArray(pinList.manifests), "pin manifest list must include manifests array");
    assert.ok(
      Number.isInteger(pinList.limit) && pinList.limit > 0,
      "pin manifest list must include a positive limit",
    );

    if (pinList.manifests.length > 0) {
      const [manifest] = pinList.manifests;
      assertHexString(manifest.digest_hex, "SoraFS manifest digest");
      const resolved = await client.getSorafsPinManifestTyped(manifest.digest_hex);
      assert.equal(
        resolved.manifest.digest_hex,
        manifest.digest_hex,
        "resolved manifest digest must match list entry",
      );
      assert.ok(Array.isArray(resolved.aliases), "manifest response must include aliases array");
      assert.ok(
        Array.isArray(resolved.replication_orders),
        "manifest response must include replication orders",
      );
      const iteratorHit = await iteratorIncludes(
        client.iterateSorafsPinManifests({ pageSize: 1, maxItems: 10 }),
        (entry) => entry?.digest_hex === manifest.digest_hex,
      );
      assert.ok(iteratorHit, "pin manifest iterator should surface the sampled manifest");
    } else {
      t.diagnostic("SoraFS pin registry returned no manifests");
    }

    const aliasList = await client.listSorafsAliases({ limit: 5 });
    assert.ok(Array.isArray(aliasList.aliases), "alias list must include aliases array");
    if (aliasList.aliases.length > 0) {
      const aliasRecord = aliasList.aliases[0];
      assert.ok(aliasRecord.alias.length > 0, "alias entries must expose alias labels");
      assertHexString(aliasRecord.manifest_digest_hex, "alias manifest digest");
      const aliasIteratorHit = await iteratorIncludes(
        client.iterateSorafsAliases({ pageSize: 1, maxItems: 10 }),
        (entry) =>
          entry?.alias === aliasRecord.alias &&
          entry?.manifest_digest_hex === aliasRecord.manifest_digest_hex,
      );
      assert.ok(aliasIteratorHit, "alias iterator should surface the sampled alias");
    } else {
      t.diagnostic("SoraFS alias list is empty on target node");
    }

    const replicationList = await client.listSorafsReplicationOrders({ limit: 5 });
    assert.ok(
      Array.isArray(replicationList.replication_orders),
      "replication order list must expose replication_orders",
    );
    if (replicationList.replication_orders.length > 0) {
      const order = replicationList.replication_orders[0];
      assertHexString(order.order_id_hex, "replication order id");
      assert.ok(
        Array.isArray(order.receipts),
        "replication order entries must include receipts array",
      );
      const replicationIteratorHit = await iteratorIncludes(
        client.iterateSorafsReplicationOrders({ pageSize: 1, maxItems: 10 }),
        (entry) => entry?.order_id_hex === order.order_id_hex,
      );
      assert.ok(
        replicationIteratorHit,
        "replication iterator should surface the sampled order id",
      );
    } else {
      t.diagnostic("SoraFS replication order list is empty on target node");
    }

    const storageState = await client.getSorafsStorageState();
    assert.equal(
      typeof storageState.bytes_capacity,
      "number",
      "storage state must expose bytes_capacity",
    );
    assert.equal(
      typeof storageState.bytes_used,
      "number",
      "storage state must expose bytes_used",
    );
  },
);

test(
  "SoraFS payload fetch returns requested range (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 90_000,
  },
  async (t) => {
    if (!SORAFS_ENABLED) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_SORAFS_ENABLED=1 to exercise SoraFS payload fetch coverage",
      );
      return;
    }
    if (!SORAFS_FETCH_MANIFEST || SORAFS_FETCH_LENGTH === null) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_SORAFS_FETCH_MANIFEST=<digest> and IROHA_TORII_INTEGRATION_SORAFS_FETCH_LENGTH=<bytes> to enable payload fetch coverage",
      );
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const offset = SORAFS_FETCH_OFFSET ?? 0;
    const length = SORAFS_FETCH_LENGTH;
    let fetchResponse;
    try {
      const requestInput = {
        manifestIdHex: SORAFS_FETCH_MANIFEST,
        offset,
        length,
      };
      if (SORAFS_FETCH_PROVIDER) {
        requestInput.providerIdHex = SORAFS_FETCH_PROVIDER;
      }
      fetchResponse = await client.fetchSorafsPayloadRange(requestInput);
    } catch (error) {
      if (isSorafsUnavailableError(error) || isUnexpectedNotFoundError(error)) {
        t.diagnostic(
          `SoraFS payload fetch endpoint unavailable on target node: ${
            error instanceof Error ? error.message : String(error)
          }`,
        );
        return;
      }
      throw error;
    }
    assertHexString(fetchResponse.manifest_id_hex, "sorafs fetch manifest_id_hex");
    assert.equal(
      fetchResponse.manifest_id_hex.toLowerCase(),
      SORAFS_FETCH_MANIFEST.toLowerCase(),
      "SoraFS payload fetch digest must match configured manifest",
    );
    assert.equal(fetchResponse.offset, offset, "SoraFS payload fetch offset must echo request");
    assert.equal(fetchResponse.length, length, "SoraFS payload fetch length must echo request");
    const dataBuffer = Buffer.from(fetchResponse.data_b64, "base64");
    assert.ok(
      dataBuffer.length > 0,
      "SoraFS payload fetch should return at least one byte for configured length",
    );
    assert.equal(
      dataBuffer.length,
      fetchResponse.length,
      "SoraFS payload fetch data must match reported length",
    );
    t.diagnostic(
      `SoraFS payload fetch read ${dataBuffer.length} bytes at offset ${offset} (provider=${
        SORAFS_FETCH_PROVIDER ?? "auto"
      })`,
    );
  },
);

test(
  "SoraFS PoR status/export endpoints respond (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 90_000,
  },
  async (t) => {
    if (!SORAFS_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_SORAFS_ENABLED=1 to exercise SoraFS PoR coverage");
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    let statusBuffer;
    try {
      statusBuffer = await client.getSorafsPorStatus();
    } catch (error) {
      if (shouldSkipSorafsPorEndpoints(error)) {
        t.diagnostic(
          `SoraFS PoR status endpoint unavailable on target node: ${error.message}`,
        );
        return;
      }
      throw error;
    }
    assert.ok(Buffer.isBuffer(statusBuffer), "PoR status endpoint must return a Buffer");

    let exportBuffer;
    try {
      exportBuffer = await client.exportSorafsPorStatus();
    } catch (error) {
      if (shouldSkipSorafsPorEndpoints(error)) {
        t.diagnostic(
          `SoraFS PoR export endpoint unavailable on target node: ${error.message}`,
        );
        return;
      }
      throw error;
    }
    assert.ok(Buffer.isBuffer(exportBuffer), "PoR export endpoint must return a Buffer");
  },
);

test(
  "SoraFS PoR weekly report fetch (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 90_000,
  },
  async (t) => {
    if (!SORAFS_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_SORAFS_ENABLED=1 to exercise SoraFS PoR coverage");
      return;
    }
    if (!SORAFS_POR_WEEK) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_SORAFS_POR_WEEK (e.g. 2026-W05) to enable PoR weekly report coverage",
      );
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    let reportBuffer;
    try {
      reportBuffer = await client.getSorafsPorWeeklyReport(SORAFS_POR_WEEK);
    } catch (error) {
      if (isSorafsUnavailableError(error)) {
        t.diagnostic(`SoraFS PoR weekly report unavailable on target node: ${error.message}`);
        return;
      }
      throw error;
    }
    assert.ok(Buffer.isBuffer(reportBuffer), "PoR weekly report must return a Buffer");
  },
);

test(
  "SoraFS capacity sampling endpoints respond (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 90_000,
  },
  async (t) => {
    if (!SORAFS_ENABLED) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_SORAFS_ENABLED=1 to exercise SoraFS capacity coverage",
      );
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });

    let uptimeSample;
    try {
      uptimeSample = await client.submitSorafsUptimeObservation({
        uptimeSecs: 60,
        observedSecs: 60,
      });
    } catch (error) {
      if (shouldSkipSorafsPorEndpoints(error)) {
        t.diagnostic(
          `SoraFS uptime endpoint unavailable on target node: ${
            error instanceof Error ? error.message : String(error)
          }`,
        );
        return;
      }
      throw error;
    }
    assert.ok(
      typeof uptimeSample.status === "string" && uptimeSample.status.length > 0,
      "SoraFS uptime response must include a status string",
    );
    assertNonNegativeInteger(
      uptimeSample.uptime_secs,
      "sorafs uptime response uptime_secs must be non-negative",
    );
    assertNonNegativeInteger(
      uptimeSample.observed_secs,
      "sorafs uptime response observed_secs must be non-negative",
    );

    let porObservation;
    try {
      porObservation = await client.submitSorafsPorObservation({ success: true });
    } catch (error) {
      if (shouldSkipSorafsPorEndpoints(error)) {
        t.diagnostic(
          `SoraFS PoR observation endpoint unavailable on target node: ${
            error instanceof Error ? error.message : String(error)
          }`,
        );
        return;
      }
      throw error;
    }
    assert.ok(
      typeof porObservation.status === "string" && porObservation.status.length > 0,
      "SoraFS PoR observation response must include a status string",
    );
    assert.equal(
      porObservation.success,
      true,
      "SoraFS PoR observation payload should echo the submitted success flag",
    );
  },
);

test(
  "UAID portfolio endpoint responds (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (!UAID_LITERAL) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_UAID=uaid:<hex> (or raw 64-hex digest) to exercise UAID portfolio coverage",
      );
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    let portfolio;
    try {
      portfolio = await client.getUaidPortfolio(UAID_LITERAL);
    } catch (error) {
      t.diagnostic(
        `UAID portfolio endpoint unavailable: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      return;
    }
    assertUaidPortfolioSnapshot(portfolio, UAID_LITERAL);
  },
);

test(
  "UAID bindings endpoint responds (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (!UAID_LITERAL) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_UAID=uaid:<hex> (or raw 64-hex digest) to exercise UAID bindings coverage",
      );
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    let bindings;
    try {
      bindings = await client.getUaidBindings(UAID_LITERAL);
    } catch (error) {
      t.diagnostic(
        `UAID bindings endpoint unavailable: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      return;
    }
    assertUaidBindingsSnapshot(bindings, UAID_LITERAL);
  },
);

test(
  "UAID manifests endpoint responds (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (!UAID_LITERAL) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_UAID=uaid:<hex> (or raw 64-hex digest) to exercise UAID manifest coverage",
      );
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const manifestOptions =
      UAID_DATASPACE_ID === null ? undefined : { dataspaceId: UAID_DATASPACE_ID };
    let manifests;
    try {
      manifests = await client.getUaidManifests(UAID_LITERAL, manifestOptions);
    } catch (error) {
      t.diagnostic(
        `UAID manifests endpoint unavailable: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      return;
    }
    assertUaidManifestsSnapshot(manifests, UAID_LITERAL);
    if (manifests.manifests.length === 0) {
      t.diagnostic("UAID manifests endpoint returned zero manifests for supplied UAID");
    }
  },
);

test(
  "Sora Name Service policy endpoint responds (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SNS_SUFFIX_ID === null) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_SNS_SUFFIX=<u16> to exercise SNS policy coverage",
      );
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    let policy;
    try {
      policy = await client.getSnsPolicy(SNS_SUFFIX_ID);
    } catch (error) {
      t.diagnostic(
        `SNS policy endpoint unavailable: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      return;
    }
    assertSnsPolicySnapshot(policy, SNS_SUFFIX_ID);
  },
);

test(
  "Sora Name Service registration endpoint responds (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    const selector = normalizeIntegrationString(SNS_SELECTOR);
    if (!selector) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_SNS_SELECTOR=<label.suffix> to exercise SNS registration coverage",
      );
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    let registration;
    try {
      registration = await client.getSnsRegistration(selector);
    } catch (error) {
      t.diagnostic(
        `SNS registration endpoint unavailable: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      return;
    }
    assertSnsNameRecord(registration, selector);
  },
);

test(
  "Space Directory manifest publish (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 120_000,
  },
  async (t) => {
    if (!SPACE_DIRECTORY_ENABLED) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_SPACE_DIRECTORY_ENABLED=1 with a manifest path to exercise Space Directory publish coverage",
      );
      return;
    }
    if (!MUTATION_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_MUTATE=1 to enable Space Directory coverage");
      return;
    }
    if (!PRIVATE_KEY_HEX) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_PRIVATE_KEY_HEX=<hex> to sign Space Directory manifests",
      );
      return;
    }
    const manifestFixture = loadSpaceDirectoryManifestFixture(t);
    if (!manifestFixture) {
      return;
    }
    const manifestPayload = canonicalizeSpaceDirectoryManifest(manifestFixture.manifest);
    const uaidLiteral = typeof manifestPayload.uaid === "string" ? manifestPayload.uaid.trim() : "";
    if (!uaidLiteral) {
      t.diagnostic(
        `Space Directory manifest fixture ${manifestFixture.path} is missing a uaid literal`,
      );
      return;
    }
    const dataspaceId = coerceNonNegativeInteger(manifestPayload.dataspace);
    if (dataspaceId === null) {
      t.diagnostic(
        `Space Directory manifest fixture ${manifestFixture.path} is missing a dataspace id`,
      );
      return;
    }
    const publishReason = `js-integration publish ${new Date().toISOString()}`;
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    try {
      const publishResult = await client.publishSpaceDirectoryManifest({
        authority: AUTHORITY_ACCOUNT_ID,
        privateKey: PRIVATE_KEY_HEX,
        manifest: manifestPayload,
        reason: publishReason,
      });
      assert.ok(
        publishResult === null || typeof publishResult === "object",
        "publishSpaceDirectoryManifest should return null or a JSON body",
      );
      const entriesCount = Array.isArray(manifestPayload.entries)
        ? manifestPayload.entries.length
        : 0;
      const activationEpoch = coerceNonNegativeInteger(
        manifestPayload.activation_epoch ?? manifestPayload.activationEpoch,
      );
      const expiryEpoch = coerceNonNegativeInteger(
        manifestPayload.expiry_epoch ?? manifestPayload.expiryEpoch,
      );
      const { record: publishedRecord } = await waitForUaidManifestRecord(
        client,
        uaidLiteral,
        dataspaceId,
        {
          predicate: (record) => record.status === "Pending" || record.status === "Active",
          attempts: 10,
          delayMs: 1_000,
        },
      );
      assert.equal(
        publishedRecord.manifest.uaid,
        uaidLiteral,
        "published manifest should echo the UAID literal from the fixture",
      );
      assert.equal(
        publishedRecord.manifest.dataspace,
        dataspaceId,
        "published manifest should report the dataspace id from the fixture",
      );
      if (activationEpoch !== null) {
        assert.equal(
          publishedRecord.manifest.activation_epoch,
          activationEpoch,
          "published manifest should retain the activation epoch from the fixture",
        );
      }
      if (expiryEpoch !== null) {
        assert.equal(
          publishedRecord.manifest.expiry_epoch,
          expiryEpoch,
          "published manifest should retain the expiry epoch from the fixture",
        );
      }
      assert.equal(
        publishedRecord.manifest.entries.length,
        entriesCount,
        "published manifest entries count should match the fixture entries",
      );
      assert.equal(
        publishedRecord.manifest.version,
        manifestPayload.version,
        "published manifest version should match the fixture",
      );
    } catch (error) {
      t.diagnostic(
        `Space Directory manifest publish failed for fixture ${manifestFixture.path}: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      throw error;
    }
  },
);

test(
  "Space Directory manifest revoke (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 120_000,
  },
  async (t) => {
    if (!SPACE_DIRECTORY_ENABLED) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_SPACE_DIRECTORY_ENABLED=1 with revoke inputs to exercise Space Directory revoke coverage",
      );
      return;
    }
    if (!MUTATION_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_MUTATE=1 to enable Space Directory coverage");
      return;
    }
    if (!PRIVATE_KEY_HEX) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_PRIVATE_KEY_HEX=<hex> to sign Space Directory requests",
      );
      return;
    }
    const manifestFixture = loadSpaceDirectoryManifestFixture(t);
    if (!manifestFixture) {
      return;
    }
    const manifestPayload = canonicalizeSpaceDirectoryManifest(manifestFixture.manifest);
    const uaidLiteral = typeof manifestPayload.uaid === "string" ? manifestPayload.uaid.trim() : "";
    if (!uaidLiteral) {
      t.diagnostic(
        `Space Directory manifest fixture ${manifestFixture.path} is missing a uaid literal`,
      );
      return;
    }
    const dataspaceId = coerceNonNegativeInteger(manifestPayload.dataspace);
    if (dataspaceId === null) {
      t.diagnostic(
        `Space Directory manifest fixture ${manifestFixture.path} must include a numeric dataspace id`,
      );
      return;
    }
    const fallbackEpoch =
      coerceNonNegativeInteger(manifestPayload.expiry_epoch ?? manifestPayload.expiryEpoch) ??
      coerceNonNegativeInteger(manifestPayload.activation_epoch ?? manifestPayload.activationEpoch);
    const revokedEpoch = SPACE_DIRECTORY_REVOKE_EPOCH ?? fallbackEpoch;
    if (revokedEpoch === null) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_SPACE_DIRECTORY_REVOKE_EPOCH=<epoch> or include activation/expiry epochs in the manifest fixture",
      );
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    try {
      const revokeReason = `js-integration revoke ${new Date().toISOString()}`;
      const revokeResult = await client.revokeSpaceDirectoryManifest({
        authority: AUTHORITY_ACCOUNT_ID,
        privateKey: PRIVATE_KEY_HEX,
        uaid: uaidLiteral,
        dataspace: dataspaceId,
        revokedEpoch,
        reason: revokeReason,
      });
      assert.ok(
        revokeResult === null || typeof revokeResult === "object",
        "revokeSpaceDirectoryManifest should return null or a JSON body",
      );
      const { record: revokedRecord } = await waitForUaidManifestRecord(
        client,
        uaidLiteral,
        dataspaceId,
        {
          predicate: (record) =>
            Boolean(
              record.lifecycle.revocation && record.lifecycle.revocation.epoch === revokedEpoch,
            ),
          attempts: 10,
          delayMs: 1_000,
        },
      );
      const revocation = revokedRecord.lifecycle.revocation;
      assert.ok(revocation, "revoked manifest should expose a revocation lifecycle entry");
      assert.equal(
        revocation.epoch,
        revokedEpoch,
        "revoked manifest lifecycle should echo the requested revocation epoch",
      );
      if (typeof revocation.reason === "string" && revocation.reason.length > 0) {
        assert.ok(
          revocation.reason.includes("js-integration"),
          "revoked manifest reason should mirror the integration request tag",
        );
      }
      assert.equal(
        revokedRecord.manifest.dataspace,
        dataspaceId,
        "revoked manifest should remain bound to the requested dataspace",
      );
      if (revokedRecord.status !== "Revoked") {
        t.diagnostic(
          `Space Directory manifest revocation recorded with status=${revokedRecord.status}`,
        );
      }
    } catch (error) {
      t.diagnostic(
        `Space Directory manifest revoke failed for UAID ${uaidLiteral}: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      throw error;
    }
  },
);

test(
  "DA ingest submits payload (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 150_000,
  },
  async (t) => {
    if (!DA_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_DA_ENABLED=1 to exercise DA ingest coverage");
      return;
    }
    if (!MUTATION_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_MUTATE=1 to enable DA ingest coverage");
      return;
    }
    if (!PRIVATE_KEY_HEX) {
      t.diagnostic("IROHA_TORII_INTEGRATION_PRIVATE_KEY_HEX is required for DA ingest coverage");
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const payloadBuffer = Buffer.from(
      `js-da-ingest-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`,
      "utf8",
    );
    const metadata = {
      suite: "js-integration",
      step: "da-ingest",
      timestamp: new Date().toISOString(),
    };
    const submission = await client.submitDaBlob({
      payload: payloadBuffer,
      codec: "application/octet-stream",
      metadata,
      privateKeyHex: PRIVATE_KEY_HEX,
    });
    assert.equal(
      typeof submission.status,
      "string",
      "da ingest response should include status string",
    );
    assert.equal(
      typeof submission.duplicate,
      "boolean",
      "da ingest response should include duplicate flag",
    );
    assert.ok(submission.receipt, "da ingest response should provide a receipt");
    assertHexString(
      submission.receipt.storage_ticket_hex,
      "da ingest receipt storage_ticket_hex",
    );
    assertHexString(
      submission.receipt.blob_hash_hex,
      "da ingest receipt blob_hash_hex",
    );
    assertHexString(
      submission.artifacts.clientBlobIdHex,
      "da ingest artifacts clientBlobIdHex",
    );
    assert.equal(
      submission.artifacts.clientBlobIdHex,
      submission.receipt.client_blob_id_hex,
      "ingest artifacts should mirror the receipt digest",
    );

    try {
      const manifest = await waitForDaManifest(
        client,
        submission.receipt.storage_ticket_hex,
        { attempts: 6, intervalMs: 5_000 },
      );
      assert.equal(
        manifest.storage_ticket_hex,
        submission.receipt.storage_ticket_hex,
        "manifest ticket should match ingest receipt",
      );
      assertHexString(manifest.blob_hash_hex, "da manifest blob hash");
      assert.ok(manifest.manifest_bytes.length > 0, "da manifest should return bytes payload");
    } catch (error) {
      t.diagnostic(`da manifest polling failed: ${error instanceof Error ? error.message : error}`);
      throw error;
    }
  },
);

test(
  "DA manifest and gateway fetch (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 120_000,
  },
  async (t) => {
    if (!DA_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_DA_ENABLED=1 to exercise DA gateway coverage");
      return;
    }
    if (!DA_TICKET_HEX) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_DA_TICKET=<hex storage ticket> to enable DA manifest fetch coverage",
      );
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    let manifest;
    try {
      manifest = await client.getDaManifest(DA_TICKET_HEX);
    } catch (error) {
      t.diagnostic(
        `da manifest fetch failed for ${DA_TICKET_HEX}: ${
          error instanceof Error ? error.message : error
        }`,
      );
      throw error;
    }
    assert.equal(
      manifest.storage_ticket_hex,
      DA_TICKET_HEX.toUpperCase(),
      "manifest ticket should match requested value",
    );
    assertHexString(manifest.client_blob_id_hex, "da manifest client blob id");
    assertHexString(manifest.chunk_root_hex, "da manifest chunk root");
    if (!Array.isArray(DA_GATEWAY_SPECS) || DA_GATEWAY_SPECS.length === 0) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_DA_GATEWAYS=[...] to exercise gateway fetch coverage",
      );
      return;
    }
    const session = await client.fetchDaPayloadViaGateway({
      manifestBundle: manifest,
      gatewayProviders: DA_GATEWAY_SPECS,
      proofSummary: false,
    });
    assert.ok(session, "da gateway fetch should return a session record");
    assert.equal(
      session.manifest?.storage_ticket_hex ?? null,
      manifest.storage_ticket_hex,
      "gateway session manifest should mirror fetched manifest",
    );
    assert.ok(
      Array.isArray(session.reports),
      "da gateway fetch response must include reports array",
    );
  },
);

test(
  "connect app registry lifecycle (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 120_000,
  },
  async (t) => {
    if (!MUTATION_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_MUTATE=1 to enable connect registry coverage");
      return;
    }
    if (!isPlainObject(CONNECT_APP_CONFIG)) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_CONNECT_APP to a JSON object to exercise the connect registry lifecycle",
      );
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const base = CONNECT_APP_CONFIG ?? {};
    const baseAppId =
      typeof base.appId === "string" && base.appId.trim().length > 0
        ? base.appId.trim()
        : "js-connect-app";
    const suffix = `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
    const appId = `${baseAppId}-${suffix}`;
    const namespaces =
      Array.isArray(base.namespaces) && base.namespaces.length > 0
        ? base.namespaces.map((value, index) => {
            if (typeof value !== "string") {
              throw new TypeError(
                `IROHA_TORII_INTEGRATION_CONNECT_APP.namespaces[${index}] must be a string`,
              );
            }
            const trimmed = value.trim();
            if (!trimmed) {
              throw new TypeError(
                `IROHA_TORII_INTEGRATION_CONNECT_APP.namespaces[${index}] must not be empty`,
              );
            }
            return trimmed;
          })
        : ["apps"];
    const metadata = deepMerge(
      {
        suite: "js-integration",
        timestamp: Date.now(),
      },
      isPlainObject(base.metadata) ? base.metadata : {},
    );
    const policy = deepMerge({}, isPlainObject(base.policy) ? base.policy : {});
    const extra = isPlainObject(base.extra) ? base.extra : undefined;
    const record = {
      appId,
      displayName:
        typeof base.displayName === "string" && base.displayName.trim().length > 0
          ? base.displayName
          : `JS Integration ${appId}`,
      description:
        typeof base.description === "string" && base.description.trim().length > 0
          ? base.description
          : "JS integration connect app",
      iconUrl:
        typeof base.iconUrl === "string" && base.iconUrl.trim().length > 0
          ? base.iconUrl
          : undefined,
      namespaces,
      metadata,
      policy,
      extra,
    };
    let createdAppId = null;
    try {
      await client.deleteConnectApp(appId);
      const created = await client.registerConnectApp(record);
      assert.ok(created, "connect app registration should return a payload");
      assert.equal(created.appId, appId);
      createdAppId = appId;

      const fetched = await client.getConnectApp(appId);
      assert.equal(fetched.appId, appId, "getConnectApp should round-trip the newly registered app");

      const page = await client.listConnectApps({ limit: 50 });
      assert.ok(
        page.items.some((entry) => entry.appId === appId),
        "connect registry page should include the newly registered app",
      );

      const iteratorHit = await iteratorIncludes(
        client.iterateConnectApps({ pageSize: 20, maxItems: 200 }),
        (entry) => entry?.appId === appId,
      );
      assert.ok(iteratorHit, "connect registry iterator should yield the test app");

      const removed = await client.deleteConnectApp(appId);
      assert.ok(removed, "connect app deletion should report success");
      createdAppId = null;
    } finally {
      if (createdAppId) {
        try {
          await client.deleteConnectApp(createdAppId);
        } catch (cleanupError) {
          t.diagnostic(
            `failed to delete connect app ${createdAppId}: ${
              cleanupError instanceof Error ? cleanupError.message : String(cleanupError)
            }`,
          );
        }
      }
    }
  },
);

test(
  "connect preview bootstrapper registers Torii sessions (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 120_000,
  },
  async (t) => {
    if (!MUTATION_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_MUTATE=1 to exercise connect preview coverage");
      return;
    }
    if (!isPlainObject(CONNECT_PREVIEW_CONFIG)) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_CONNECT_PREVIEW to a JSON object (e.g. {\"node\":\"connect.devnet.example\"}) to exercise the preview bootstrapper",
      );
      return;
    }
    if (CONNECT_PREVIEW_CONFIG.register === false) {
      t.diagnostic("connect preview coverage requires register !== false");
      return;
    }

    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const previewEnv = CONNECT_PREVIEW_CONFIG ?? {};
    const chainId = normalizeIntegrationString(previewEnv.chainId) ?? CHAIN_ID;
    const previewNode = normalizeIntegrationString(previewEnv.node);
    const sessionOptionsRaw = isPlainObject(previewEnv.sessionOptions)
      ? previewEnv.sessionOptions
      : null;
    const sanitizedSessionOptions = {};
    const sessionNode = normalizeIntegrationString(sessionOptionsRaw?.node);
    if (sessionNode) {
      sanitizedSessionOptions.node = sessionNode;
    }
    const sessionOptions =
      Object.keys(sanitizedSessionOptions).length === 0 ? undefined : sanitizedSessionOptions;

    let createdSid = null;
    try {
      const result = await bootstrapConnectPreviewSession(client, {
        chainId,
        node: previewNode ?? null,
        nonce: previewEnv.nonce ?? null,
        sessionOptions,
      });
      createdSid = result.preview.sidBase64Url;
      assert.ok(
        typeof result.preview.walletUri === "string" &&
          result.preview.walletUri.startsWith("iroha://connect"),
        "preview wallet URI must be canonical",
      );
      assert.ok(result.session, "bootstrap helper should register a session");
      assert.ok(result.tokens, "bootstrap helper should return session tokens");
      assert.equal(
        result.session.sid,
        result.preview.sidBase64Url,
        "session sid must match the preview sid",
      );
      assert.equal(
        result.tokens.wallet,
        result.session.token_wallet,
        "wallet token helper output should match session payload",
      );
      assert.equal(
        result.tokens.app,
        result.session.token_app,
        "app token helper output should match session payload",
      );
      assert.ok(
        typeof result.session.wallet_uri === "string" &&
          result.session.wallet_uri.startsWith("iroha://connect"),
        "connect session wallet URI must be populated",
      );
      assert.ok(
        typeof result.session.app_uri === "string" &&
          result.session.app_uri.includes("connect/app"),
        "connect session app URI must be populated",
      );
      t.diagnostic(
        `bootstrapConnectPreviewSession registered sid=${result.preview.sidBase64Url}`,
      );
    } finally {
      if (createdSid) {
        try {
          await client.deleteConnectSession(createdSid);
        } catch (error) {
          t.diagnostic(
            `failed to delete connect session ${createdSid}: ${error instanceof Error ? error.message : String(error)}`,
          );
        }
      }
    }
  },
);

test(
  "telemetry peer inventory and explorer metrics respond",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const peers = await client.listTelemetryPeersInfo();
    assert.ok(Array.isArray(peers) && peers.length > 0, "expected telemetry peers list to be non-empty");
    const peer = peers[0];
    assert.equal(typeof peer.url, "string", "telemetry peer must expose url");
    assert.equal(typeof peer.connected, "boolean", "telemetry peer must expose connected flag");
    assert.equal(
      typeof peer.telemetryUnsupported,
      "boolean",
      "telemetry peer must expose telemetryUnsupported flag",
    );
    if (peer.config) {
      assert.equal(typeof peer.config.publicKey, "string", "telemetry peer config must expose publicKey");
    }
    if (peer.location) {
      assert.equal(typeof peer.location.country, "string", "telemetry peer location must expose country");
      assert.equal(typeof peer.location.city, "string", "telemetry peer location must expose city");
    }
    if (peer.connectedPeers) {
      assert.ok(Array.isArray(peer.connectedPeers), "connectedPeers must be an array when present");
    }

    const peerInventory = await client.listPeers();
    assert.ok(Array.isArray(peerInventory), "peer inventory payload must be an array");
    const typedPeerInventory = await client.listPeersTyped();
    assert.ok(Array.isArray(typedPeerInventory), "typed peer inventory payload must be an array");
    if (typedPeerInventory.length === 0) {
      t.diagnostic("Torii peer inventory returned zero entries");
    } else {
      const typedPeer = typedPeerInventory[0];
      assert.equal(typeof typedPeer.address, "string", "typed peer address must be a string");
      assert.notEqual(typedPeer.address.length, 0, "typed peer address must not be empty");
      assertHexString(typedPeer.public_key_hex, "typed peer public_key_hex");
    }

    const metrics = await client.getExplorerMetrics();
    if (!metrics) {
      t.diagnostic("explorer metrics endpoint disabled on target node");
      return;
    }
    assertNonNegativeInteger(metrics.peers, "explorer metrics peers");
    assertNonNegativeInteger(metrics.domains, "explorer metrics domains");
    assertNonNegativeInteger(metrics.accounts, "explorer metrics accounts");
    assertNonNegativeInteger(metrics.assets, "explorer metrics assets");
    assertNonNegativeInteger(
      metrics.transactionsAccepted,
      "explorer metrics transactionsAccepted",
    );
    assertNonNegativeInteger(
      metrics.transactionsRejected,
      "explorer metrics transactionsRejected",
    );
    assertNonNegativeInteger(metrics.blockHeight, "explorer metrics blockHeight");
    assertNonNegativeInteger(metrics.finalizedBlockHeight, "explorer metrics finalizedBlockHeight");
    if (typeof metrics.blockCreatedAt === "string") {
      assert.notEqual(metrics.blockCreatedAt.length, 0, "blockCreatedAt must not be empty when present");
    }
    if (metrics.averageCommitTimeMs !== null) {
      assert.ok(
        Number.isFinite(metrics.averageCommitTimeMs) && metrics.averageCommitTimeMs >= 0,
        "averageCommitTimeMs must be a non-negative number when present",
      );
    }
    if (metrics.averageBlockTimeMs !== null) {
      assert.ok(
        Number.isFinite(metrics.averageBlockTimeMs) && metrics.averageBlockTimeMs >= 0,
        "averageBlockTimeMs must be a non-negative number when present",
      );
    }
  },
);

test(
  "explorer account QR endpoint provides share-ready payloads",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    let defaultSnapshot;
    try {
      defaultSnapshot = await client.getExplorerAccountQr(AUTHORITY_ACCOUNT_ID);
    } catch (error) {
      if (isUnexpectedNotFoundError(error)) {
        t.diagnostic("explorer QR endpoint unavailable on target node; skipping assertions");
        return;
      }
      throw error;
    }

    assertExplorerAccountQrSnapshot(defaultSnapshot, "default explorer QR snapshot");
    assert.equal(
      defaultSnapshot.addressFormat,
      "ih58",
      "default explorer QR snapshot should use IH58 format",
    );
    assert.ok(
      defaultSnapshot.literal.includes("@"),
      "IH58 literal should include the domain separator",
    );

    const compressedSnapshot = await client.getExplorerAccountQr(AUTHORITY_ACCOUNT_ID, {
      addressFormat: "compressed",
    });
    assertExplorerAccountQrSnapshot(compressedSnapshot, "compressed explorer QR snapshot");

    assert.equal(
      compressedSnapshot.addressFormat,
      "compressed",
      "compressed snapshot should honour the requested address format",
    );
    assert.equal(
      compressedSnapshot.canonicalId,
      defaultSnapshot.canonicalId,
      "canonicalId must remain stable across address formats",
    );
    assert.equal(
      compressedSnapshot.networkPrefix,
      defaultSnapshot.networkPrefix,
      "networkPrefix must remain stable across address formats",
    );
    assert.notEqual(
      compressedSnapshot.addressFormat,
      defaultSnapshot.addressFormat,
      "address format flag should change when requesting compressed output",
    );
  },
);

test(
  "sumeragi telemetry snapshot exposes availability, backlog, and VRF stats",
  {
    skip: !!SKIP_REASON,
    timeout: 90_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const telemetry = await client.getSumeragiTelemetryTyped();
    assert.ok(telemetry, "sumeragi telemetry response must be present");

    const availability = telemetry.availability;
    assertNonNegativeInteger(
      availability.total_votes_ingested,
      "sumeragi telemetry availability.total_votes_ingested",
    );
    assert.ok(
      Array.isArray(availability.collectors),
      "sumeragi telemetry collectors must be an array",
    );
    if (availability.collectors.length > 0) {
      const collector = availability.collectors[0];
      assert.equal(typeof collector.peer_id, "string", "collector peer_id must be a string");
      assert.ok(
        Number.isInteger(collector.collector_idx) && collector.collector_idx >= 0,
        "collector collector_idx must be a non-negative integer",
      );
    }

    assert.ok(
      Array.isArray(telemetry.qc_latency_ms),
      "sumeragi telemetry qc_latency_ms must be an array",
    );
    telemetry.qc_latency_ms.forEach((entry, index) => {
      assert.equal(typeof entry.kind, "string", `qc_latency_ms[${index}].kind must be a string`);
      assert.ok(
        Number.isInteger(entry.last_ms),
        `qc_latency_ms[${index}].last_ms must be an integer`,
      );
    });

    const backlog = telemetry.rbc_backlog;
    assertNonNegativeInteger(backlog.pending_sessions, "sumeragi telemetry backlog.pending_sessions");
    assertNonNegativeInteger(
      backlog.total_missing_chunks,
      "sumeragi telemetry backlog.total_missing_chunks",
    );
    assertNonNegativeInteger(
      backlog.max_missing_chunks,
      "sumeragi telemetry backlog.max_missing_chunks",
    );

    const vrf = telemetry.vrf;
    assert.equal(typeof vrf.found, "boolean", "sumeragi telemetry vrf.found must be boolean");
    assertNonNegativeInteger(vrf.epoch, "sumeragi telemetry vrf.epoch");
    assertNonNegativeInteger(vrf.roster_len, "sumeragi telemetry vrf.roster_len");
    assertNonNegativeInteger(vrf.participants_total, "sumeragi telemetry vrf.participants_total");

    const phases = await client.getSumeragiPhases();
    assert.ok(phases, "sumeragi phases payload must be present");
    assertNonNegativeInteger(phases.propose_ms, "sumeragi phases propose_ms");
    assertNonNegativeInteger(phases.collect_da_ms, "sumeragi phases collect_da_ms");
    assertNonNegativeInteger(phases.commit_ms, "sumeragi phases commit_ms");
    assert.ok(
      phases.ema_ms && typeof phases.ema_ms === "object",
      "sumeragi phases ema_ms must be present",
    );

    const pacemaker = await client.getSumeragiPacemaker();
    if (!pacemaker) {
      t.diagnostic("sumeragi pacemaker endpoint disabled on target node");
    } else {
      assertNonNegativeInteger(pacemaker.backoff_ms, "sumeragi pacemaker backoff_ms");
      assertNonNegativeInteger(
        pacemaker.view_timeout_target_ms,
        "sumeragi pacemaker view_timeout_target_ms",
      );
      assertNonNegativeInteger(
        pacemaker.view_timeout_remaining_ms,
        "sumeragi pacemaker view_timeout_remaining_ms",
      );
    }
  },
);

test(
  "sumeragi BLS key registry exposes peer map",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const keyMap = await client.getSumeragiBlsKeys();
    assert.ok(keyMap && typeof keyMap === "object", "BLS key map must be an object");
    const entries = Object.entries(keyMap);
    if (entries.length === 0) {
      t.diagnostic("sumeragi BLS key registry returned zero entries");
    }
    for (const [peerId, blsHex] of entries) {
      assert.equal(typeof peerId, "string", "peer id must be a string");
      assert.notEqual(peerId.length, 0, "peer id must be non-empty");
      if (blsHex === null) {
        continue;
      }
      assertHexString(blsHex, `BLS key for ${peerId}`);
    }
  },
);

test(
  "sumeragi leader snapshot exposes PRF context",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const leader = await client.getSumeragiLeader();
    assert.ok(leader && typeof leader === "object", "leader snapshot must be present");
    assertNonNegativeInteger(leader.leader_index, "sumeragi leader_index");
    assert.ok(leader.prf && typeof leader.prf === "object", "leader prf snapshot must exist");
    assertNonNegativeInteger(leader.prf.height, "sumeragi leader prf.height");
    assertNonNegativeInteger(leader.prf.view, "sumeragi leader prf.view");
    if (leader.prf.epoch_seed !== null) {
      assert.equal(typeof leader.prf.epoch_seed, "string", "leader prf epoch_seed must be string");
      assert.notEqual(leader.prf.epoch_seed.length, 0, "leader prf epoch_seed must be non-empty");
    }
  },
);

test(
  "sumeragi collectors plan snapshot exposes routing metadata",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const plan = await client.getSumeragiCollectors();
    assert.ok(plan && typeof plan === "object", "collector plan must be an object");
    assert.equal(typeof plan.consensus_mode, "string", "consensus_mode must be a string");
    assert.equal(typeof plan.mode, "string", "mode must be a string");
    assertNonNegativeInteger(plan.topology_len, "collectors topology_len");
    assertNonNegativeInteger(plan.min_votes_for_commit, "collectors min_votes_for_commit");
    assertNonNegativeInteger(plan.proxy_tail_index, "collectors proxy_tail_index");
    assertNonNegativeInteger(plan.height, "collectors height");
    assertNonNegativeInteger(plan.view, "collectors view");
    assertNonNegativeInteger(plan.collectors_k, "collectors collectors_k");
    assertNonNegativeInteger(plan.redundant_send_r, "collectors redundant_send_r");
    if (plan.epoch_seed !== null) {
      assert.equal(typeof plan.epoch_seed, "string", "epoch_seed must be a string when present");
      assert.notEqual(plan.epoch_seed.length, 0, "epoch_seed must be non-empty when present");
    }
    assert.ok(Array.isArray(plan.collectors), "collector plan must expose a collectors array");
    if (plan.collectors.length === 0) {
      t.diagnostic("collector plan returned zero collector entries");
    } else {
      const sample = plan.collectors[0];
      assert.equal(typeof sample.peer_id, "string", "collector peer_id must be a string");
      assertNonNegativeInteger(sample.index, "collector index");
    }
    assert.ok(plan.prf && typeof plan.prf === "object", "collector plan prf snapshot must be set");
    assertNonNegativeInteger(plan.prf.height, "collector prf height");
    assertNonNegativeInteger(plan.prf.view, "collector prf view");
    if (plan.prf.epoch_seed !== null) {
      assert.equal(typeof plan.prf.epoch_seed, "string", "prf epoch_seed must be a string");
      assert.notEqual(plan.prf.epoch_seed.length, 0, "prf epoch_seed must be non-empty");
    }
  },
);

test(
  "sumeragi params snapshot exposes runtime configuration",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const params = await client.getSumeragiParams();
    assert.ok(params && typeof params === "object", "params snapshot must be an object");
    assertNonNegativeInteger(params.block_time_ms, "sumeragi params block_time_ms");
    assertNonNegativeInteger(params.commit_time_ms, "sumeragi params commit_time_ms");
    assertNonNegativeInteger(params.max_clock_drift_ms, "sumeragi params max_clock_drift_ms");
    assertNonNegativeInteger(params.collectors_k, "sumeragi params collectors_k");
    assertNonNegativeInteger(params.redundant_send_r, "sumeragi params redundant_send_r");
    assert.equal(typeof params.da_enabled, "boolean", "sumeragi params da_enabled must be boolean");
    if (params.next_mode !== null) {
      assert.equal(typeof params.next_mode, "string", "sumeragi params next_mode must be string");
    }
    if (params.mode_activation_height !== null) {
      assertNonNegativeInteger(
        params.mode_activation_height,
        "sumeragi params mode_activation_height",
      );
    }
    assertNonNegativeInteger(params.chain_height, "sumeragi params chain_height");
  },
);

test(
  "governance unlock stats endpoint responds",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const stats = await client.getGovernanceUnlockStatsTyped();
    assert.ok(stats, "unlock stats payload must be present");
    assert.equal(
      typeof stats.height_current,
      "number",
      "unlock stats height must be numeric",
    );
    assert.equal(
      typeof stats.expired_locks_now,
      "number",
      "unlock stats expired_locks_now must be numeric",
    );
    assert.equal(
      typeof stats.referenda_with_expired,
      "number",
      "unlock stats referenda_with_expired must be numeric",
    );
    assert.equal(
      typeof stats.last_sweep_height,
      "number",
      "unlock stats last_sweep_height must be numeric",
    );
    t.diagnostic(
      `governance unlock stats height=${stats.height_current} expired=${stats.expired_locks_now} last_sweep=${stats.last_sweep_height}`,
    );
  },
);

test(
  "governance council endpoints respond",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });

    let council;
    try {
      council = await client.getGovernanceCouncilCurrent();
    } catch (error) {
      if (shouldSkipGovernanceCouncilEndpoints(error)) {
        t.diagnostic(
          "governance council endpoint unavailable on target node; skipping council assertions",
        );
        return;
      }
      throw error;
    }

    assert.ok(council, "council payload should be present");
    assertNonNegativeInteger(council.epoch, "council.epoch");
    assert.ok(Array.isArray(council.members), "council members must be an array");
    if (council.members.length === 0) {
      t.diagnostic("governance council returned zero members");
    } else {
      council.members.forEach((member, index) => {
        assert.equal(
          typeof member.account_id,
          "string",
          `council.members[${index}].account_id must be a string`,
        );
        assert.notEqual(
          member.account_id.length,
          0,
          `council.members[${index}].account_id must not be empty`,
        );
      });
    }

    let audit = null;
    try {
      audit = await client.getGovernanceCouncilAudit({ epoch: council.epoch });
    } catch (error) {
      if (shouldSkipGovernanceCouncilEndpoints(error)) {
        t.diagnostic("governance council audit endpoint unavailable; skipping audit assertions");
      } else {
        throw error;
      }
    }

    if (audit) {
      assertNonNegativeInteger(audit.epoch, "council audit epoch");
      assertHexString(audit.seed_hex, "council audit seed_hex");
      assertHexString(audit.beacon_hex, "council audit beacon_hex");
      assertNonNegativeInteger(audit.members_count, "council audit members_count");
      assertNonNegativeInteger(audit.candidate_count, "council audit candidate_count");
      assert.equal(typeof audit.chain_id, "string", "council audit chain_id must be a string");
      assert.notEqual(audit.chain_id.length, 0, "council audit chain_id must not be empty");
    }
  },
);

test(
  "governance protected namespaces endpoint responds",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const response = await client.getProtectedNamespaces();
    assert.ok(response, "protected namespaces payload must be present");
    assert.equal(
      typeof response.found,
      "boolean",
      "protected namespaces response must include `found` flag",
    );
    assert.ok(
      Array.isArray(response.namespaces),
      "protected namespaces response must include an array",
    );
    for (const namespace of response.namespaces) {
      assert.equal(
        typeof namespace,
        "string",
        "protected namespaces entries must be strings",
      );
    }
    t.diagnostic(
      `protected namespaces found=${response.found} namespaces=${response.namespaces.join(",")}`,
    );
  },
);

test(
  "governance protected namespaces apply round-trips snapshot (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (!MUTATION_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_MUTATE=1 to enable governance mutation coverage");
      return;
    }
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });

    let baseline;
    try {
      baseline = await client.getProtectedNamespaces();
    } catch (error) {
      t.diagnostic(
        `protected namespaces endpoint unavailable: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      return;
    }
    if (!baseline.found) {
      t.diagnostic(
        "protected namespaces configuration not found on the target node; skipping apply coverage",
      );
      return;
    }
    if (baseline.namespaces.length === 0) {
      t.diagnostic("protected namespaces list is empty; skipping apply coverage");
      return;
    }

    let applyResponse;
    try {
      applyResponse = await client.setProtectedNamespaces(baseline.namespaces);
    } catch (error) {
      t.diagnostic(
        `failed to reapply protected namespaces: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      return;
    }

    assert.ok(applyResponse.ok, "protected namespaces apply should indicate success");
    assertNonNegativeInteger(
      applyResponse.applied,
      "protected namespaces apply response should include numeric applied count",
    );

    const confirmation = await client.getProtectedNamespaces();
    assert.ok(confirmation.found, "protected namespaces should remain available after apply");
    assert.deepEqual(
      confirmation.namespaces,
      baseline.namespaces,
      "protected namespaces apply should preserve the namespace snapshot",
    );
  },
);

test(
  "governance instances listing responds for protected namespace",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    let namespacesResponse;
    try {
      namespacesResponse = await client.getProtectedNamespaces();
    } catch (error) {
      t.diagnostic(
        `protected namespaces endpoint unavailable: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      return;
    }
    const targetNamespace = namespacesResponse.namespaces?.[0];
    if (!targetNamespace) {
      t.diagnostic("protected namespaces list is empty; skipping governance instance assertions");
      return;
    }
    let instancePage;
    try {
      instancePage = await client.listGovernanceInstances(targetNamespace, { limit: 5 });
    } catch (error) {
      t.diagnostic(
        `governance instances endpoint unavailable for namespace ${targetNamespace}: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      return;
    }
    assert.equal(
      instancePage.namespace,
      targetNamespace,
      "governance instance page should echo namespace",
    );
    assertNonNegativeInteger(instancePage.total, "governance instance page total");
    assertNonNegativeInteger(instancePage.offset, "governance instance page offset");
    assertNonNegativeInteger(instancePage.limit, "governance instance page limit");
    assert.ok(
      Array.isArray(instancePage.instances),
      "governance instance page must include instances array",
    );
    if (instancePage.instances.length === 0) {
      t.diagnostic(`no governance instances returned for namespace ${targetNamespace}`);
      return;
    }
    instancePage.instances.forEach((instance, index) => {
      assert.equal(
        typeof instance.contract_id,
        "string",
        `governance instance ${index} contract_id must be a string`,
      );
      assert.notEqual(
        instance.contract_id.length,
        0,
        `governance instance ${index} contract_id must not be empty`,
      );
      assertHexString(instance.code_hash_hex, `governance instance ${index} code_hash_hex`);
    });
    const firstInstance = instancePage.instances[0];
    const iteratorHit = await iteratorIncludes(
      client.iterateGovernanceInstances(targetNamespace, { pageSize: 2, maxItems: 10 }),
      (entry) => entry?.contract_id === firstInstance.contract_id,
    );
    assert.ok(
      iteratorHit,
      "governance instance iterator should surface the listed contract id at least once",
    );
  },
);

test(
  "contract instances listing responds for protected namespace",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });

    let namespacesResponse;
    try {
      namespacesResponse = await client.getProtectedNamespaces();
    } catch (error) {
      t.diagnostic(
        `protected namespaces endpoint unavailable: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      return;
    }

    const targetNamespace = namespacesResponse.namespaces?.[0];
    if (!targetNamespace) {
      t.diagnostic("protected namespaces list is empty; skipping contract instance assertions");
      return;
    }

    let instancePage;
    try {
      instancePage = await client.listContractInstances(targetNamespace, { limit: 5 });
    } catch (error) {
      t.diagnostic(
        `contract instances endpoint unavailable for namespace ${targetNamespace}: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      return;
    }

    assertContractInstanceListResponse(instancePage, targetNamespace);

    if (instancePage.instances.length === 0) {
      t.diagnostic(`no contract instances returned for namespace ${targetNamespace}`);
      return;
    }

    const firstInstance = instancePage.instances[0];
    const iteratorHit = await iteratorIncludes(
      client.iterateContractInstances(targetNamespace, { pageSize: 2, maxItems: 10 }),
      (entry) => entry?.contract_id === firstInstance.contract_id,
    );
    assert.ok(
      iteratorHit,
      "contract instance iterator should surface the listed contract id at least once",
    );
  },
);

test(
  "governance plain ballot submission (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 120_000,
  },
  async (t) => {
    if (SKIP_REASON) {
      t.diagnostic(SKIP_REASON);
      return;
    }
    if (!MUTATION_ENABLED) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_MUTATE=1 to enable governance ballot submission coverage",
      );
      return;
    }
    if (!GOVERNANCE_BALLOT_OPTIONS || !isPlainObject(GOVERNANCE_BALLOT_OPTIONS)) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_GOV_BALLOT to a JSON object with referendumId/owner/amount/durationBlocks/direction to exercise ballot submission coverage",
      );
      return;
    }
    let ballotPayload;
    try {
      ballotPayload = buildGovernancePlainBallotPayload(GOVERNANCE_BALLOT_OPTIONS);
    } catch (error) {
      t.diagnostic(
        `invalid governance ballot payload from IROHA_TORII_INTEGRATION_GOV_BALLOT: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      throw error;
    }
    if (!ballotPayload) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_GOV_BALLOT.referendumId to run the governance ballot submission coverage",
      );
      return;
    }

    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });

    let response;
    try {
      response = await client.governanceSubmitPlainBallot(ballotPayload);
    } catch (error) {
      if (shouldSkipGovernanceBallotEndpoints(error)) {
        t.diagnostic(
          `governance ballot endpoint unavailable on target node: ${
            error instanceof Error ? error.message : String(error)
          }`,
        );
        return;
      }
      throw error;
    }

    assert.ok(response, "governance ballot response should be present");
    assert.equal(typeof response.ok, "boolean", "governance ballot response should expose ok flag");
    assert.equal(
      typeof response.accepted,
      "boolean",
      "governance ballot response should expose accepted flag",
    );
    assert.ok(
      Array.isArray(response.tx_instructions),
      "governance ballot response must expose tx_instructions array",
    );
    response.tx_instructions.forEach((instruction, index) => {
      assert.ok(
        isNonEmptyString(instruction.wire_id),
        `governance ballot tx_instructions[${index}].wire_id must be a string`,
      );
      if (instruction.payload_hex !== undefined) {
        assertHexString(
          instruction.payload_hex,
          `governance ballot tx_instructions[${index}].payload_hex`,
        );
      }
    });
    if (response.proposal_id !== null && response.proposal_id !== undefined) {
      assertHexString(response.proposal_id, "governance ballot proposal_id");
    }
    if (response.reason !== undefined && response.reason !== null) {
      assert.equal(
        typeof response.reason,
        "string",
        "governance ballot response reason must be a string when present",
      );
    }
    t.diagnostic(
      `governance ballot accepted=${response.accepted} instructions=${response.tx_instructions.length} proposal=${response.proposal_id ?? "none"}`,
    );
  },
);

test(
  "ISO bridge alias VOPRF helper responds (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (!ISO_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_ISO_ENABLED=1 to exercise ISO alias coverage");
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    try {
      const response = await client.evaluateAliasVoprf(ISO_VOPRF_INPUT);
      assert.ok(
        typeof response.evaluated_element_hex === "string" &&
          response.evaluated_element_hex.length > 0,
        "alias VOPRF response must include evaluated_element_hex",
      );
      assert.ok(
        typeof response.backend === "string" && response.backend.length > 0,
        "alias VOPRF response must include backend label",
      );
      t.diagnostic(
        `alias VOPRF backend=${response.backend} digest=${response.evaluated_element_hex.slice(
          0,
          16,
        )}…`,
      );
    } catch (error) {
      if (isIsoBridgeDisabledError(error)) {
        t.diagnostic(`ISO bridge disabled on target node: ${error.message}`);
        return;
      }
      if (error instanceof Error && /hex string/i.test(error.message ?? "")) {
        t.diagnostic(
          `alias VOPRF helper rejected configured input '${ISO_VOPRF_INPUT}': ${error.message}`,
        );
        return;
      }
      throw error;
    }
  },
);

test(
  "ISO bridge pacs.008 submission (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 120_000,
  },
  async (t) => {
    if (!ISO_ENABLED) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_ISO_ENABLED=1 (and optionally IROHA_TORII_INTEGRATION_ISO_PACS008) to exercise ISO coverage",
      );
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const fields = buildIsoPacs008Fields(ISO_PACS008_OVERRIDES);
    const xmlPayload = buildPacs008Message(fields);
    try {
      const submission = await client.submitIsoPacs008(xmlPayload);
      assert.ok(
        submission?.message_id,
        "ISO submission should return a message_id for follow-up status queries",
      );
      const status = await client.getIsoMessageStatus(submission.message_id);
      assert.ok(
        status === null || typeof status === "object",
        "ISO status response should be structured when available",
      );
    } catch (error) {
      if (
        error instanceof Error &&
        error.message.includes("ISO bridge runtime is disabled on the target node")
      ) {
        t.diagnostic("ISO bridge runtime disabled on target node; skipping ISO submission test");
        return;
      }
      throw error;
    }
  },
);

test(
  "ISO bridge pacs.009 submission (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 120_000,
  },
  async (t) => {
    if (!ISO_ENABLED) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_ISO_ENABLED=1 (and optionally IROHA_TORII_INTEGRATION_ISO_PACS009) to exercise ISO coverage",
      );
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const fields = buildIsoPacs009Fields(ISO_PACS009_OVERRIDES);
    const xmlPayload = buildPacs009Message(fields);
    try {
      const submission = await client.submitIsoPacs009(xmlPayload);
      assert.ok(
        submission?.message_id,
        "ISO submission should return a message_id for follow-up status queries",
      );
      const status = await client.getIsoMessageStatus(submission.message_id);
      assert.ok(
        status === null || typeof status === "object",
        "ISO status response should be structured when available",
      );
    } catch (error) {
      if (
        error instanceof Error &&
        error.message.includes("ISO bridge runtime is disabled on the target node")
      ) {
        t.diagnostic("ISO bridge runtime disabled on target node; skipping ISO submission test");
        return;
      }
      throw error;
    }
  },
);

test(
  "ISO bridge waitForIsoMessageStatus polls message ids (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 180_000,
  },
  async (t) => {
    if (!ISO_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_ISO_ENABLED=1 to exercise ISO status polling");
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const fields = buildIsoPacs008Fields(ISO_PACS008_OVERRIDES);
    const xmlPayload = buildPacs008Message(fields);
    let submission;
    try {
      submission = await client.submitIsoPacs008(xmlPayload);
    } catch (error) {
      if (isIsoBridgeDisabledError(error)) {
        t.diagnostic(`ISO bridge runtime disabled on target node: ${error.message}`);
        return;
      }
      t.diagnostic(
        `ISO pacs.008 submission failed before polling: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      throw error;
    }
    const messageId = submission?.message_id;
    if (!isNonEmptyString(messageId)) {
      t.diagnostic(
        "ISO pacs.008 submission did not include message_id; cannot exercise waitForIsoMessageStatus",
      );
      return;
    }
    try {
      const status = await client.waitForIsoMessageStatus(messageId, {
        pollIntervalMs: ISO_POLL_INTERVAL_MS,
        maxAttempts: ISO_MAX_ATTEMPTS,
        resolveOnAcceptedWithoutTransaction: true,
      });
      assert.ok(status, "waitForIsoMessageStatus should resolve with a status payload");
      assert.ok(
        typeof status.status === "string" && status.status.length > 0,
        "waitForIsoMessageStatus payload must include a status string",
      );
      t.diagnostic(
        `waitForIsoMessageStatus resolved for message_id=${messageId} status=${status.status}`,
      );
    } catch (error) {
      if (isIsoBridgeDisabledError(error)) {
        t.diagnostic(`ISO bridge disabled while polling status: ${error.message}`);
        return;
      }
      if (error instanceof IsoMessageTimeoutError) {
        t.diagnostic(
          `waitForIsoMessageStatus timed out for message_id=${messageId} after ${ISO_MAX_ATTEMPTS} attempts`,
        );
        return;
      }
      throw error;
    }
  },
);

test(
  "ISO bridge wait helper surfaces timeout for unknown message ids (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 120_000,
  },
  async (t) => {
    if (!ISO_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_ISO_ENABLED=1 to exercise ISO timeout coverage");
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const missingMessageId = randomIdentifier("iso-timeout-id-");
    try {
      await client.waitForIsoMessageStatus(missingMessageId, {
        pollIntervalMs: Math.max(ISO_POLL_INTERVAL_MS, 500),
        maxAttempts: 2,
        resolveOnAcceptedWithoutTransaction: true,
      });
      t.fail(`expected waitForIsoMessageStatus to time out for ${missingMessageId}`);
    } catch (error) {
      if (isIsoBridgeDisabledError(error)) {
        t.diagnostic(`ISO bridge disabled while polling status: ${error.message}`);
        return;
      }
      assert.ok(
        error instanceof IsoMessageTimeoutError,
        `expected IsoMessageTimeoutError when polling missing id (received ${error?.message ?? error})`,
      );
    }
  },
);

test(
  "ISO bridge pacs.008 wait helper (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 180_000,
  },
  async (t) => {
    if (!ISO_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_ISO_ENABLED=1 to exercise ISO wait helpers");
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const fields = buildIsoPacs008Fields(ISO_PACS008_OVERRIDES);
    const xmlPayload = buildPacs008Message(fields);
    try {
      const status = await client.submitIsoPacs008AndWait(xmlPayload, {
        wait: {
          pollIntervalMs: ISO_POLL_INTERVAL_MS,
          maxAttempts: ISO_MAX_ATTEMPTS,
          resolveOnAcceptedWithoutTransaction: true,
        },
      });
      assert.ok(status, "ISO wait helper should return a status payload");
      assert.ok(
        typeof status.status === "string" && status.status.length > 0,
        "ISO status payload must include a status string",
      );
      t.diagnostic(
        `ISO pacs.008 wait helper resolved with status=${status.status} tx=${status.transaction_hash ?? "none"}`,
      );
    } catch (error) {
      if (isIsoBridgeDisabledError(error)) {
        t.diagnostic(`ISO bridge disabled on target node: ${error.message}`);
        return;
      }
      if (error instanceof IsoMessageTimeoutError) {
        t.diagnostic(
          `ISO pacs.008 wait helper timed out after ${ISO_MAX_ATTEMPTS} attempts: ${error.message}`,
        );
        return;
      }
      throw error;
    }
  },
);

test(
  "ISO bridge submitIsoMessage pacs.008 wait flow (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 120_000,
  },
  async (t) => {
    if (!ISO_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_ISO_ENABLED=1 to exercise ISO submitIsoMessage coverage");
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const fields = buildIsoPacs008Fields(ISO_PACS008_OVERRIDES);
    try {
      const status = await client.submitIsoMessage(fields, {
        kind: "pacs.008",
        wait: {
          pollIntervalMs: ISO_POLL_INTERVAL_MS,
          maxAttempts: ISO_MAX_ATTEMPTS,
        },
      });
      assert.ok(status, "ISO submitIsoMessage pacs.008 helper should return a status payload");
      assert.equal(
        typeof status.status,
        "string",
        "ISO status payload must include a status string",
      );
      t.diagnostic(
        `ISO submitIsoMessage pacs.008 resolved with status=${status.status} tx=${status.transaction_hash ?? "none"}`,
      );
    } catch (error) {
      if (isIsoBridgeDisabledError(error)) {
        t.diagnostic(`ISO bridge disabled on target node: ${error.message}`);
        return;
      }
      if (error instanceof IsoMessageTimeoutError) {
        t.diagnostic(
          `ISO submitIsoMessage pacs.008 timed out after ${ISO_MAX_ATTEMPTS} attempts: ${error.message}`,
        );
        return;
      }
      throw error;
    }
  },
);

test(
  "ISO bridge submitIsoMessage pacs.009 wait flow (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 120_000,
  },
  async (t) => {
    if (!ISO_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_ISO_ENABLED=1 to exercise ISO submitIsoMessage coverage");
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const fields = buildIsoPacs009Fields(ISO_PACS009_OVERRIDES);
    try {
      const status = await client.submitIsoMessage(fields, {
        kind: "pacs.009",
        wait: {
          pollIntervalMs: ISO_POLL_INTERVAL_MS,
          maxAttempts: ISO_MAX_ATTEMPTS,
          resolveOnAcceptedWithoutTransaction: true,
        },
      });
      assert.ok(status, "ISO submitIsoMessage pacs.009 helper should return a status payload");
      assert.equal(
        typeof status.status,
        "string",
        "ISO status payload must include a status string",
      );
      t.diagnostic(
        `ISO submitIsoMessage pacs.009 resolved with status=${status.status} tx=${status.transaction_hash ?? "none"}`,
      );
    } catch (error) {
      if (isIsoBridgeDisabledError(error)) {
        t.diagnostic(`ISO bridge disabled on target node: ${error.message}`);
        return;
      }
      if (error instanceof IsoMessageTimeoutError) {
        t.diagnostic(
          `ISO submitIsoMessage pacs.009 timed out after ${ISO_MAX_ATTEMPTS} attempts: ${error.message}`,
        );
        return;
      }
      throw error;
    }
  },
);

test(
  "ISO bridge submitIsoMessage pacs.009 default headers wait flow (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 120_000,
  },
  async (t) => {
    if (!ISO_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_ISO_ENABLED=1 to exercise ISO submitIsoMessage coverage");
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const fields = buildIsoPacs009Fields({
      ...ISO_PACS009_OVERRIDES,
      messageId: undefined,
      businessMessageId: undefined,
      messageDefinitionId: undefined,
    });
    const instructionId = fields.instructionId;
    const xml = buildPacs009Message(fields);
    assert.match(
      xml,
      new RegExp(`<MsgId>${instructionId}</MsgId>`),
      "MsgId should fall back to instruction id",
    );
    assert.match(
      xml,
      new RegExp(`<BizMsgIdr>${instructionId}</BizMsgIdr>`),
      "BizMsgIdr should fall back to instruction id",
    );
    assert.match(
      xml,
      /<MsgDefIdr>pacs\.009\.001\.10<\/MsgDefIdr>/,
      "MsgDefIdr should default to pacs.009.001.10",
    );
    try {
      const status = await client.submitIsoMessage(fields, {
        kind: "pacs.009",
        wait: {
          pollIntervalMs: ISO_POLL_INTERVAL_MS,
          maxAttempts: ISO_MAX_ATTEMPTS,
          resolveOnAcceptedWithoutTransaction: true,
        },
      });
      assert.ok(status, "ISO submitIsoMessage pacs.009 helper should return a status payload");
      assert.equal(typeof status.status, "string", "ISO status payload must include a status string");
      t.diagnostic(
        `ISO submitIsoMessage pacs.009 (default headers) resolved with status=${status.status} tx=${status.transaction_hash ?? "none"}`,
      );
    } catch (error) {
      if (isIsoBridgeDisabledError(error)) {
        t.diagnostic(`ISO bridge disabled on target node: ${error.message}`);
        return;
      }
      if (error instanceof IsoMessageTimeoutError) {
        t.diagnostic(
          `ISO submitIsoMessage pacs.009 default headers timed out after ${ISO_MAX_ATTEMPTS} attempts: ${error.message}`,
        );
        return;
      }
      throw error;
    }
  },
);

test(
  "ISO bridge pacs.009 wait helper (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 180_000,
  },
  async (t) => {
    if (!ISO_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_ISO_ENABLED=1 to exercise ISO wait helpers");
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const fields = buildIsoPacs009Fields(ISO_PACS009_OVERRIDES);
    const xmlPayload = buildPacs009Message(fields);
    try {
      const status = await client.submitIsoPacs009AndWait(xmlPayload, {
        wait: {
          pollIntervalMs: ISO_POLL_INTERVAL_MS,
          maxAttempts: ISO_MAX_ATTEMPTS,
          resolveOnAcceptedWithoutTransaction: true,
        },
      });
      assert.ok(status, "ISO wait helper should return a status payload");
      assert.ok(
        typeof status.status === "string" && status.status.length > 0,
        "ISO status payload must include a status string",
      );
      t.diagnostic(
        `ISO pacs.009 wait helper resolved with status=${status.status} tx=${status.transaction_hash ?? "none"}`,
      );
    } catch (error) {
      if (isIsoBridgeDisabledError(error)) {
        t.diagnostic(`ISO bridge disabled on target node: ${error.message}`);
        return;
      }
      if (error instanceof IsoMessageTimeoutError) {
        t.diagnostic(
          `ISO pacs.009 wait helper timed out after ${ISO_MAX_ATTEMPTS} attempts: ${error.message}`,
        );
        return;
      }
      throw error;
    }
  },
);

test(
  "ISO bridge alias resolution by label (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (!ISO_ENABLED || !ISO_ALIAS_LABEL) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_ISO_ENABLED=1 and IROHA_TORII_INTEGRATION_ISO_ALIAS=<alias> to exercise alias resolution coverage",
      );
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    try {
      const resolved = await client.resolveAlias(ISO_ALIAS_LABEL);
      if (!resolved) {
        t.diagnostic(
          `alias resolution returned null for ${ISO_ALIAS_LABEL}; ensure the alias exists on the target node`,
        );
        return;
      }
      assert.ok(
        typeof resolved.alias === "string" && resolved.alias.length > 0,
        "alias resolution must include the alias label",
      );
      assert.ok(
        typeof resolved.account_id === "string" && resolved.account_id.length > 0,
        "alias resolution must include the target account_id",
      );
      if (resolved.index !== undefined) {
        assert.equal(
          typeof resolved.index,
          "number",
          "alias resolution index must be numeric when present",
        );
      }
      if (resolved.source !== undefined) {
        assert.ok(
          typeof resolved.source === "string" && resolved.source.length > 0,
          "alias resolution source must be a non-empty string when present",
        );
      }
    } catch (error) {
      if (isIsoBridgeDisabledError(error)) {
        t.diagnostic(`ISO bridge disabled on target node: ${error.message}`);
        return;
      }
      throw error;
    }
  },
);

test(
  "ISO bridge alias resolution by index (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (!ISO_ENABLED || !ISO_ALIAS_INDEX) {
      t.diagnostic(
        "set IROHA_TORII_INTEGRATION_ISO_ENABLED=1 and IROHA_TORII_INTEGRATION_ISO_ALIAS_INDEX=<integer> to exercise alias index coverage",
      );
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    try {
      const resolved = await client.resolveAliasByIndex(ISO_ALIAS_INDEX);
      if (!resolved) {
        t.diagnostic(
          `alias resolve_index returned null for index ${ISO_ALIAS_INDEX}; ensure the index is valid on the target node`,
        );
        return;
      }
      assert.ok(
        typeof resolved.alias === "string" && resolved.alias.length > 0,
        "alias resolution must include the alias label",
      );
      assert.ok(
        typeof resolved.account_id === "string" && resolved.account_id.length > 0,
        "alias resolution must include the target account_id",
      );
      if (resolved.index !== undefined) {
        assert.equal(
          typeof resolved.index,
          "number",
          "alias resolution index must be numeric when present",
        );
      }
      if (resolved.source !== undefined) {
        assert.ok(
          typeof resolved.source === "string" && resolved.source.length > 0,
          "alias resolution source must be a non-empty string when present",
        );
      }
    } catch (error) {
      if (isIsoBridgeDisabledError(error)) {
        t.diagnostic(`ISO bridge disabled on target node: ${error.message}`);
        return;
      }
      throw error;
    }
  },
);

test(
  "ISO bridge alias resolution returns null for missing entries (optional)",
  {
    skip: !!SKIP_REASON,
    timeout: 60_000,
  },
  async (t) => {
    if (!ISO_ENABLED) {
      t.diagnostic("set IROHA_TORII_INTEGRATION_ISO_ENABLED=1 to exercise alias fallback coverage");
      return;
    }
    const client = new ToriiClient(BASE_URL, {
      authToken: AUTH_TOKEN,
      apiToken: API_TOKEN,
    });
    const unlikelyAlias = randomIdentifier("js-missing-alias-");
    try {
      const resolved = await client.resolveAlias(unlikelyAlias);
      assert.equal(
        resolved,
        null,
        `expected resolveAlias to return null for missing alias ${unlikelyAlias}`,
      );
    } catch (error) {
      if (isIsoBridgeDisabledError(error)) {
        t.diagnostic(`ISO bridge disabled on target node: ${error.message}`);
        return;
      }
      throw error;
    }
  },
);

async function waitForPipelineBlockEvent(client, signal) {
  const iterator = client.streamEvents({
    filter: { Pipeline: { Block: {} } },
    signal,
  });
  for await (const event of iterator) {
    if (event?.data?.Pipeline?.Block) {
      return event;
    }
  }
  throw new Error("pipeline event stream closed before emitting a block payload");
}

function assertPipelineBlockEvent(event) {
  assert.ok(event, "expected streamEvents to yield an event payload");
  assert.ok(isNonEmptyString(event?.id), "pipeline event must include an id");
  assert.ok(
    event?.data?.Pipeline?.Block,
    "pipeline event must contain a Pipeline.Block payload",
  );
  const payload = event.data.Pipeline.Block;
  if (payload.height !== undefined) {
    assert.equal(typeof payload.height, "number", "block event height must be numeric when present");
  }
  if (payload.hash !== undefined) {
    assert.ok(
      isNonEmptyString(payload.hash),
      "block event hash must be a non-empty string when present",
    );
  }
}

async function withTimeout(promise, timeoutMs, message) {
  let timeoutId;
  const timeoutPromise = new Promise((_, reject) => {
    timeoutId = setTimeout(() => reject(new Error(message)), timeoutMs);
  });
  try {
    return await Promise.race([promise, timeoutPromise]);
  } finally {
    clearTimeout(timeoutId);
  }
}

function decodePrivateKeyHex(hexLiteral) {
  const trimmed = hexLiteral.trim();
  if (!trimmed) {
    throw new Error("private key hex must not be empty");
  }
  if (trimmed.length % 2 !== 0) {
    throw new Error("private key hex must have an even length");
  }
  return Buffer.from(trimmed, "hex");
}

function randomDomainId() {
  return randomIdentifier("jsintegration");
}

function randomAccountId(domainId) {
  const accountName = randomIdentifier("jsacct");
  return `${accountName}@${domainId}`;
}

function randomAssetDefinitionId(domainId) {
  const assetName = randomIdentifier("jsasset");
  return `${assetName}#${domainId}`;
}

function randomTriggerId(namespace = "apps") {
  const suffix = randomIdentifier("trigger");
  return `${namespace}::${suffix}`;
}

function randomIdentifier(prefix) {
  return `${prefix}${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

function assertSuccessfulStatus(status, referenceId) {
  assert.ok(status, "expected submitTransactionAndWaitTyped to return a payload");
  const kind = status?.status?.kind;
  assert.ok(kind, "pipeline status should provide kind");
  const normalized = kind.toLowerCase();
  assert.ok(
    SUCCESS_STATUSES.has(normalized),
    `transaction for ${referenceId} reported ${kind}`,
  );
}

async function waitForSumeragiStatusEvent(client, signal) {
  const iterator = client.streamSumeragiStatus({ signal });
  for await (const event of iterator) {
    if (event?.data && typeof event.data === "object") {
      return event;
    }
  }
  throw new Error("sumeragi status stream closed before emitting a payload");
}

function assertSumeragiStatusEvent(event) {
  assert.ok(event, "expected streamSumeragiStatus to yield an event payload");
  assert.ok(
    event?.data && typeof event.data === "object",
    "sumeragi status event must expose a JSON payload",
  );
  const snapshot = event.data;
  assert.ok(
    Array.isArray(snapshot.lane_commitments),
    "sumeragi status event must include lane_commitments array",
  );
  snapshot.lane_commitments.forEach((entry, index) =>
    assertLaneCommitmentSnapshot(entry, `lane_commitments[${index}]`),
  );
  assert.ok(
    Array.isArray(snapshot.dataspace_commitments),
    "sumeragi status event must include dataspace_commitments array",
  );
  snapshot.dataspace_commitments.forEach((entry, index) =>
    assertDataspaceCommitmentSnapshot(entry, `dataspace_commitments[${index}]`),
  );
  assert.ok(
    Array.isArray(snapshot.lane_governance),
    "sumeragi status event must include lane_governance array",
  );
  if (snapshot.lane_governance_sealed_total !== undefined) {
    assertNumberLike(
      snapshot.lane_governance_sealed_total,
      "sumeragi status lane_governance_sealed_total",
    );
  }
  if (snapshot.lane_governance_sealed_aliases !== undefined) {
    assert.ok(
      Array.isArray(snapshot.lane_governance_sealed_aliases),
      "sumeragi status lane_governance_sealed_aliases must be an array when present",
    );
  }
}

function assertLaneCommitmentSnapshot(entry, context) {
  assert.ok(entry && typeof entry === "object", `${context} must be an object`);
  assertNumberLike(entry.block_height, `${context}.block_height`);
  assertNumberLike(entry.lane_id, `${context}.lane_id`);
  assertNumberLike(entry.tx_count, `${context}.tx_count`);
  assertNumberLike(entry.total_chunks, `${context}.total_chunks`);
  assertNumberLike(entry.rbc_bytes_total, `${context}.rbc_bytes_total`);
  assertNumberLike(entry.teu_total, `${context}.teu_total`);
  if (entry.dataspace_id !== undefined) {
    assertNumberLike(entry.dataspace_id, `${context}.dataspace_id`);
  }
  assertHexString(entry.block_hash, `${context}.block_hash`);
}

function assertDataspaceCommitmentSnapshot(entry, context) {
  assertLaneCommitmentSnapshot(entry, context);
  assertNumberLike(entry.dataspace_id, `${context}.dataspace_id`);
}

function assertUaidPortfolioSnapshot(snapshot, expectedUaid) {
  assert.ok(snapshot && typeof snapshot === "object", "uaid portfolio snapshot must be an object");
  assert.equal(
    snapshot.uaid,
    expectedUaid,
    "uaid portfolio response must echo the requested UAID",
  );
  assert.ok(snapshot.totals && typeof snapshot.totals === "object");
  assert.equal(typeof snapshot.totals.accounts, "number");
  assert.equal(typeof snapshot.totals.positions, "number");
  assert.ok(Array.isArray(snapshot.dataspaces), "uaid portfolio dataspaces must be an array");
  snapshot.dataspaces.forEach((dataspace, index) => {
    assert.equal(
      typeof dataspace.dataspace_id,
      "number",
      `portfolio dataspaces[${index}].dataspace_id must be numeric`,
    );
    assert.ok(
      Array.isArray(dataspace.accounts),
      `portfolio dataspaces[${index}].accounts must be an array`,
    );
    dataspace.accounts.forEach((account, accountIndex) => {
      assert.ok(
        typeof account.account_id === "string" && account.account_id.length > 0,
        `portfolio dataspaces[${index}].accounts[${accountIndex}].account_id must be a string`,
      );
      assert.ok(
        Array.isArray(account.assets),
        `portfolio dataspaces[${index}].accounts[${accountIndex}].assets must be an array`,
      );
      account.assets.forEach((asset, assetIndex) => {
        assert.ok(
          typeof asset.asset_id === "string" && asset.asset_id.length > 0,
          `portfolio dataspaces[${index}].accounts[${accountIndex}].assets[${assetIndex}].asset_id must be string`,
        );
        assert.ok(
          typeof asset.asset_definition_id === "string" &&
            asset.asset_definition_id.length > 0,
          `portfolio dataspaces[${index}].accounts[${accountIndex}].assets[${assetIndex}].asset_definition_id must be string`,
        );
        assert.ok(
          typeof asset.quantity === "string" && asset.quantity.length > 0,
          `portfolio dataspaces[${index}].accounts[${accountIndex}].assets[${assetIndex}].quantity must be string`,
        );
      });
    });
  });
}

function assertUaidBindingsSnapshot(snapshot, expectedUaid) {
  assert.ok(snapshot && typeof snapshot === "object", "uaid bindings snapshot must be an object");
  assert.equal(
    snapshot.uaid,
    expectedUaid,
    "uaid bindings response must echo the requested UAID",
  );
  assert.ok(Array.isArray(snapshot.dataspaces), "uaid bindings dataspaces must be an array");
  snapshot.dataspaces.forEach((dataspace, index) => {
    assert.equal(
      typeof dataspace.dataspace_id,
      "number",
      `bindings dataspaces[${index}].dataspace_id must be numeric`,
    );
    assert.ok(
      Array.isArray(dataspace.accounts),
      `bindings dataspaces[${index}].accounts must be an array`,
    );
  });
}

function assertUaidManifestsSnapshot(snapshot, expectedUaid) {
  assert.ok(snapshot && typeof snapshot === "object", "uaid manifests snapshot must be an object");
  assert.equal(
    snapshot.uaid,
    expectedUaid,
    "uaid manifests response must echo the requested UAID",
  );
  assert.ok(Array.isArray(snapshot.manifests), "uaid manifests array must be present");
  snapshot.manifests.forEach((record, index) => {
    assert.equal(
      typeof record.dataspace_id,
      "number",
      `manifests[${index}].dataspace_id must be numeric`,
    );
    assert.ok(
      typeof record.status === "string" && record.status.length > 0,
      `manifests[${index}].status must be a string`,
    );
    assert.ok(
      typeof record.manifest_hash === "string" && record.manifest_hash.length > 0,
      `manifests[${index}].manifest_hash must be string`,
    );
    assert.ok(
      record.lifecycle && typeof record.lifecycle === "object",
      `manifests[${index}].lifecycle must be an object`,
    );
    assert.ok(
      Array.isArray(record.accounts),
      `manifests[${index}].accounts must be an array`,
    );
    assert.ok(
      record.manifest && typeof record.manifest === "object",
      `manifests[${index}].manifest must be an object`,
    );
    assert.ok(
      Array.isArray(record.manifest.entries),
      `manifests[${index}].manifest.entries must be an array`,
    );
  });
}

function assertSnsPolicySnapshot(policy, expectedSuffixId) {
  assert.ok(policy && typeof policy === "object", "sns policy response must be an object");
  assert.equal(
    policy.suffixId,
    expectedSuffixId,
    "sns policy response must echo the requested suffix id",
  );
  assert.ok(
    typeof policy.suffix === "string" && policy.suffix.length > 0,
    "sns policy suffix must be a non-empty string",
  );
  assert.ok(typeof policy.steward === "string" && policy.steward.length > 0);
  assert.ok(typeof policy.status === "string" && policy.status.length > 0);
  assertNonNegativeInteger(policy.minTermYears, "sns policy minTermYears");
  assertNonNegativeInteger(policy.maxTermYears, "sns policy maxTermYears");
  assertNonNegativeInteger(policy.gracePeriodDays, "sns policy gracePeriodDays");
  assertNonNegativeInteger(policy.redemptionPeriodDays, "sns policy redemptionPeriodDays");
  assertNonNegativeInteger(policy.referralCapBps, "sns policy referralCapBps");
  assert.ok(Array.isArray(policy.reservedLabels), "sns policy reservedLabels must be an array");
  policy.reservedLabels.forEach((label, index) => {
    assert.ok(
      label && typeof label === "object",
      `sns policy reservedLabels[${index}] must be an object`,
    );
    assert.ok(
      typeof label.normalizedLabel === "string" && label.normalizedLabel.length > 0,
      `sns policy reservedLabels[${index}].normalizedLabel must be a non-empty string`,
    );
  });
  assert.ok(Array.isArray(policy.pricing), "sns policy pricing must be an array");
  policy.pricing.forEach((tier, index) => {
    assert.ok(tier && typeof tier === "object", `sns policy pricing[${index}] must be object`);
    assertNonNegativeInteger(tier.tierId, `sns policy pricing[${index}].tierId`);
    assert.ok(
      typeof tier.labelRegex === "string" && tier.labelRegex.length > 0,
      `sns policy pricing[${index}].labelRegex must be a string`,
    );
  });
}

function assertSnsNameRecord(record, expectedSelector) {
  assert.ok(record && typeof record === "object", "sns name record must be an object");
  assert.ok(
    record.selector && typeof record.selector === "object",
    "sns name record selector must be an object",
  );
  assert.ok(
    typeof record.selector.label === "string" && record.selector.label.length > 0,
    "sns name record selector.label must be a non-empty string",
  );
  assert.ok(
    typeof record.selector.suffix_id === "number",
    "sns name record selector.suffix_id must be numeric",
  );
  assert.ok(
    typeof record.selector.version === "number",
    "sns name record selector.version must be numeric",
  );
  if (typeof expectedSelector === "string" && expectedSelector.length > 0) {
    assert.ok(
      expectedSelector.includes(record.selector.label),
      "sns name record selector label should match the requested selector label",
    );
  }
  assertHexString(record.nameHash, "sns name record nameHash");
  assert.ok(
    typeof record.owner === "string" && record.owner.length > 0,
    "sns name record owner must be a non-empty string",
  );
  assert.ok(Array.isArray(record.controllers), "sns name record controllers must be an array");
  assert.ok(record.status && typeof record.status === "object", "sns name status must be an object");
  assert.ok(
    typeof record.status.status === "string" && record.status.status.length > 0,
    "sns name status.status must be a non-empty string",
  );
  assertNonNegativeInteger(record.pricingClass, "sns name record pricingClass");
  assertNonNegativeInteger(record.registeredAtMs, "sns name record registeredAtMs");
  assertNonNegativeInteger(record.expiresAtMs, "sns name record expiresAtMs");
}

async function findAccountAsset(client, accountId, assetId) {
  for await (const asset of client.iterateAccountAssets(accountId, { limit: 10 })) {
    if (asset?.asset_id === assetId) {
      return asset;
    }
  }
  return null;
}

async function iteratorIncludes(iterator, predicate) {
  for await (const item of iterator) {
    if (predicate(item)) {
      return true;
    }
  }
  return false;
}

function compressedLiteralForAccount(accountId) {
  const literal = String(accountId);
  const separator = literal.lastIndexOf("@");
  if (separator === -1) {
    throw new Error(`account id "${literal}" must include a domain`);
  }
  const domain = literal.slice(separator + 1);
  const { compressed } = inspectAccountId(literal);
  return `${compressed}@${domain}`;
}

async function waitForDaManifest(client, ticketHex, options = {}) {
  const attempts = options.attempts ?? 5;
  const intervalMs = options.intervalMs ?? 2_000;
  let lastError = null;
  for (let index = 0; index < attempts; index += 1) {
    try {
      return await client.getDaManifest(ticketHex);
    } catch (error) {
      lastError = error;
      if (!shouldRetryDaManifestFetch(error)) {
        throw error;
      }
      await delay(intervalMs);
    }
  }
  const message =
    lastError instanceof Error ? lastError.message : String(lastError ?? "unknown error");
  throw new Error(`da manifest for ${ticketHex} unavailable after ${attempts} attempts: ${message}`);
}

function shouldRetryDaManifestFetch(error) {
  if (!error) {
    return false;
  }
  const message = error instanceof Error ? error.message : String(error);
  return /404/.test(message) || /not\s+found/i.test(message);
}

async function waitForAttachmentDeletion(client, attachmentId, attempts = 3, delayMs = 500) {
  for (let index = 0; index < attempts; index += 1) {
    try {
      await client.getAttachment(attachmentId);
    } catch (error) {
      if (isAttachmentNotFoundError(error) || isAttachmentEndpointUnavailable(error)) {
        return true;
      }
      throw error;
    }
    await delay(delayMs);
  }
  return false;
}

async function waitForUaidManifestRecord(client, uaidLiteral, dataspaceId, options = {}) {
  const {
    attempts = 5,
    delayMs = 1_000,
    predicate = () => true,
  } = options;
  let lastError = null;
  let lastSnapshot = null;
  for (let index = 0; index < attempts; index += 1) {
    try {
      const snapshot = await client.getUaidManifests(uaidLiteral, {
        dataspaceId,
      });
      assertUaidManifestsSnapshot(snapshot, uaidLiteral);
      lastSnapshot = snapshot;
      const record =
        snapshot.manifests.find(
          (entry) => entry && entry.dataspace_id === dataspaceId,
        ) ?? null;
      if (record && predicate(record)) {
        return { record, snapshot };
      }
    } catch (error) {
      lastError = error;
    }
    await delay(delayMs);
  }
  if (lastSnapshot) {
    throw new Error(
      `Space Directory manifest for dataspace ${dataspaceId} not ready after ${attempts} attempts (last manifest count=${lastSnapshot.manifests.length})`,
    );
  }
  if (lastError) {
    throw new Error(
      `Space Directory manifest fetch failed after ${attempts} attempts: ${
        lastError instanceof Error ? lastError.message : String(lastError)
      }`,
    );
  }
  throw new Error(
    `Space Directory manifest record not observed for dataspace ${dataspaceId} after ${attempts} attempts`,
  );
}

function delay(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function isNonEmptyString(value) {
  return typeof value === "string" && value.trim().length > 0;
}

function normalizeIntegrationString(value) {
  if (typeof value !== "string") {
    return null;
  }
  const trimmed = value.trim();
  return trimmed.length === 0 ? null : trimmed;
}

function resolveProjectPath(rawPath) {
  if (path.isAbsolute(rawPath)) {
    return rawPath;
  }
  return path.resolve(PROJECT_ROOT, rawPath);
}

function loadSpaceDirectoryManifestFixture(t) {
  const trimmed = normalizeIntegrationString(SPACE_DIRECTORY_MANIFEST_PATH);
  if (!trimmed) {
    t.diagnostic(
      "set IROHA_TORII_INTEGRATION_SPACE_DIRECTORY_MANIFEST=/path/to/manifest.json to exercise Space Directory coverage",
    );
    return null;
  }
  const resolvedPath = resolveProjectPath(trimmed);
  let manifestText;
  try {
    manifestText = fs.readFileSync(resolvedPath, "utf8");
  } catch (error) {
    t.diagnostic(
      `failed to read Space Directory manifest fixture ${resolvedPath}: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
    return null;
  }
  let manifest;
  try {
    manifest = JSON.parse(manifestText);
  } catch (error) {
    t.diagnostic(
      `failed to parse Space Directory manifest fixture ${resolvedPath}: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
    return null;
  }
  return { manifest, path: resolvedPath };
}

function canonicalizeSpaceDirectoryManifest(input) {
  const manifest = JSON.parse(JSON.stringify(input));
  if (manifest.version === undefined || manifest.version === null) {
    manifest.version = "1";
  } else if (typeof manifest.version !== "string") {
    manifest.version = String(manifest.version);
  }
  return manifest;
}

function coerceNonNegativeInteger(value) {
  if (value === undefined || value === null) {
    return null;
  }
  if (typeof value === "number" && Number.isFinite(value) && value >= 0) {
    return value;
  }
  if (typeof value === "string" && value.trim().length > 0) {
    const parsed = Number.parseInt(value.trim(), 10);
    if (Number.isFinite(parsed) && parsed >= 0) {
      return parsed;
    }
  }
  return null;
}

function buildGovernancePlainBallotPayload(overrides) {
  if (!isPlainObject(overrides)) {
    return null;
  }
  const referendumId = normalizeIntegrationString(
    overrides.referendumId ?? overrides.referendum_id,
  );
  if (!referendumId) {
    return null;
  }
  const authority =
    normalizeIntegrationString(overrides.authority) ?? AUTHORITY_ACCOUNT_ID;
  const owner = normalizeIntegrationString(overrides.owner) ?? AUTHORITY_ACCOUNT_ID;
  const chainId =
    normalizeIntegrationString(overrides.chainId ?? overrides.chain_id) ?? CHAIN_ID;
  const durationBlocksRaw =
    overrides.durationBlocks ?? overrides.duration_blocks ?? overrides.duration ?? 10;
  const durationBlocks =
    typeof durationBlocksRaw === "number"
      ? durationBlocksRaw
      : Number.parseInt(String(durationBlocksRaw), 10);
  if (!Number.isFinite(durationBlocks) || durationBlocks <= 0) {
    throw new Error("governance ballot durationBlocks must be a positive integer");
  }
  const amountRaw = overrides.amount ?? "1";
  const amountText =
    typeof amountRaw === "number" && Number.isFinite(amountRaw)
      ? amountRaw.toString()
      : normalizeIntegrationString(String(amountRaw));
  if (!amountText) {
    throw new Error("governance ballot amount must be a non-empty string or number");
  }
  const direction =
    normalizeIntegrationString(overrides.direction ?? "Aye") ?? "Aye";
  return {
    authority,
    chain_id: chainId,
    referendum_id: referendumId,
    owner,
    amount: amountText,
    duration_blocks: durationBlocks,
    direction,
  };
}

function buildIsoPacs008Fields(overrides) {
  const now = new Date();
  const settlementDate = new Date(now.getTime() + 24 * 60 * 60 * 1000)
    .toISOString()
    .slice(0, 10);
  const base = {
    messageId: randomIsoId("msg"),
    creationDateTime: now.toISOString(),
    instructionId: randomIsoId("instr"),
    endToEndId: randomIsoId("e2e"),
    transactionId: randomIsoId("tx"),
    settlementDate,
    amount: { currency: "USD", value: "5.00" },
    instigatingAgent: { bic: "DEUTDEFF", lei: "529900ODI3047E2LIV03" },
    instructedAgent: { bic: "COBADEFF" },
    debtorAccount: { iban: "DE89370400440532013000" },
    creditorAccount: { otherId: "34mSYnDgbaJM58rbLoif4Tkp7G4LTcGTWkBnWUGuYYFogLyNhhuq386y2zQoSXk5oi1iY4YYx" },
    purposeCode: "SECU",
    supplementaryData: {
      suite: "js-integration",
    },
  };
  if (!overrides) {
    return base;
  }
  return deepMerge(base, overrides);
}

function buildIsoPacs009Fields(overrides) {
  const now = new Date();
  const settlementDate = new Date(now.getTime() + 48 * 60 * 60 * 1000)
    .toISOString()
    .slice(0, 10);
  const base = {
    messageId: randomIsoId("msg009"),
    businessMessageId: randomIsoId("biz009"),
    messageDefinitionId: "pacs.009.001.10",
    creationDateTime: now.toISOString(),
    instructionId: randomIsoId("instr009"),
    transactionId: randomIsoId("tx009"),
    settlementDate,
    amount: { currency: "USD", value: "10.00" },
    instigatingAgent: { bic: "DEUTDEFF", lei: "529900ODI3047E2LIV03" },
    instructedAgent: { bic: "COBADEFF" },
    purposeCode: "TRAD",
    supplementaryData: {
      suite: "js-integration",
    },
  };
  if (!overrides) {
    return base;
  }
  return deepMerge(base, overrides);
}

function deepMerge(target, source) {
  const result = Array.isArray(target) ? [...target] : { ...target };
  if (!source || typeof source !== "object") {
    return result;
  }
  for (const [key, value] of Object.entries(source)) {
    if (value && typeof value === "object" && !Array.isArray(value)) {
      const prior = result[key];
      result[key] = deepMerge(
        prior && typeof prior === "object" ? prior : {},
        value,
      );
    } else {
      result[key] = value;
    }
  }
  return result;
}

function randomIsoId(prefix) {
  return `${prefix}-${Date.now().toString(36)}-${Math.random()
    .toString(36)
    .slice(2, 10)}`;
}

function parseJsonEnv(rawValue) {
  if (!rawValue) {
    return null;
  }
  try {
    return JSON.parse(rawValue);
  } catch (error) {
    throw new Error(
      `failed to parse JSON from environment variable: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
}

function parsePositiveIntegerEnv(rawValue, fallback) {
  if (!rawValue) {
    return fallback;
  }
  const parsed = Number.parseInt(rawValue, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(
      `expected positive integer but received \`${rawValue}\` for ISO integration settings`,
    );
  }
  return parsed;
}

function parseUnsignedIntegerEnv(rawValue, { allowZero = false } = {}) {
  if (!rawValue) {
    return null;
  }
  const trimmed = rawValue.trim();
  if (!trimmed) {
    return null;
  }
  const parsed = Number.parseInt(trimmed, 10);
  const isValid = Number.isFinite(parsed) && parsed >= 0 && (allowZero || parsed > 0);
  if (!isValid) {
    throw new Error(
      `expected ${allowZero ? "non-negative" : "positive"} integer but received \`${rawValue}\` for UAID integration settings`,
    );
  }
  return parsed;
}

async function resolveRbcSampleOptions(client) {
  if (RBC_SAMPLE_OPTIONS) {
    return { sample: RBC_SAMPLE_OPTIONS };
  }
  try {
    const candidate = await client.findRbcSamplingCandidate();
    if (!candidate) {
      return {
        sample: null,
        reason: "no delivered RBC session with a block hash was found",
      };
    }
    return {
      sample: {
        blockHash: candidate.blockHash,
        height: candidate.height,
        view: candidate.view,
      },
    };
  } catch (error) {
    return {
      sample: null,
      reason: `failed to auto-detect RBC session: ${
        error instanceof Error ? error.message : String(error)
      }`,
    };
  }
}

function isIsoBridgeDisabledError(error) {
  if (!(error instanceof Error)) {
    return false;
  }
  const message = error.message ?? "";
  return (
    message.includes("ISO bridge runtime is disabled") ||
    message.includes("ISO bridge is disabled") ||
    message.includes("ISO bridge runtime disabled")
  );
}

function isSorafsUnavailableError(error) {
  if (!(error instanceof Error)) {
    return false;
  }
  const message = error.message ?? "";
  return (
    /sorafs/i.test(message) &&
    (message.includes("disabled") || message.includes("404") || message.includes("503"))
  );
}

function isAttachmentEndpointUnavailable(error) {
  return (
    error instanceof ToriiHttpError &&
    (error.status === 404 ||
      error.status === 405 ||
      error.status === 403 ||
      error.status === 501)
  );
}

function isAttachmentNotFoundError(error) {
  return error instanceof ToriiHttpError && (error.status === 404 || error.status === 410);
}

function shouldSkipSorafsPorEndpoints(error) {
  return isSorafsUnavailableError(error) || isUnexpectedNotFoundError(error);
}

function shouldSkipTriggerEndpoints(error) {
  if (!(error instanceof Error)) {
    return false;
  }
  const message = error.message ?? "";
  return (
    isUnexpectedNotFoundError(error) ||
    (/trigger/i.test(message) &&
      (/disabled/i.test(message) || /unexpected status 503/i.test(message)))
  );
}

function isOfflineApiUnavailableError(error) {
  if (!(error instanceof Error)) {
    return false;
  }
  const message = error.message ?? "";
  return (
    /unexpected status 404/i.test(message) ||
    (/offline/i.test(message) && /disabled/i.test(message))
  );
}

function shouldSkipGovernanceCouncilEndpoints(error) {
  if (!(error instanceof Error)) {
    return false;
  }
  const message = error.message ?? "";
  return (
    isUnexpectedNotFoundError(error) ||
    /unexpected status 403/i.test(message) ||
    /unexpected status 503/i.test(message) ||
    (/council/i.test(message) && /disabled/i.test(message))
  );
}

function shouldSkipGovernanceBallotEndpoints(error) {
  if (error instanceof ToriiHttpError) {
    return error.status === 404 || error.status === 501 || error.status === 503;
  }
  if (!(error instanceof Error)) {
    return false;
  }
  const message = error.message ?? "";
  return /ballot/i.test(message) && /disabled/i.test(message);
}

function shouldSkipZkProverEndpoints(error) {
  if (error instanceof ToriiHttpError) {
    return error.status === 404 || error.status === 501 || error.status === 503;
  }
  if (!(error instanceof Error)) {
    return false;
  }
  const message = error.message ?? "";
  return /prover/i.test(message) && /disabled/i.test(message);
}

function isUnexpectedNotFoundError(error) {
  return error instanceof Error && /unexpected status 404/i.test(error.message ?? "");
}

function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function assertExplorerAccountQrSnapshot(snapshot, label) {
  assert.ok(isPlainObject(snapshot), `${label} must be an object`);
  assert.equal(typeof snapshot.canonicalId, "string", `${label}.canonicalId must be a string`);
  assert.notEqual(snapshot.canonicalId.length, 0, `${label}.canonicalId must not be empty`);
  assert.equal(typeof snapshot.literal, "string", `${label}.literal must be a string`);
  assert.notEqual(snapshot.literal.length, 0, `${label}.literal must not be empty`);
  assert.equal(typeof snapshot.addressFormat, "string", `${label}.addressFormat must be a string`);
  assert.ok(
    snapshot.addressFormat === "ih58" || snapshot.addressFormat === "compressed",
    `${label}.addressFormat must be "ih58" or "compressed"`,
  );
  assert.ok(
    Number.isInteger(snapshot.networkPrefix) && snapshot.networkPrefix > 0,
    `${label}.networkPrefix must be a positive integer`,
  );
  assert.equal(
    typeof snapshot.errorCorrection,
    "string",
    `${label}.errorCorrection must be a string`,
  );
  assert.notEqual(
    snapshot.errorCorrection.length,
    0,
    `${label}.errorCorrection must not be empty`,
  );
  assert.ok(
    Number.isInteger(snapshot.modules) && snapshot.modules > 0,
    `${label}.modules must be a positive integer`,
  );
  assert.ok(
    Number.isInteger(snapshot.qrVersion) && snapshot.qrVersion > 0,
    `${label}.qrVersion must be a positive integer`,
  );
  assert.equal(typeof snapshot.svg, "string", `${label}.svg must be a string`);
  assert.ok(snapshot.svg.startsWith("<svg"), `${label}.svg must contain an <svg> payload`);
}

async function fetchOfflineListPage(t, label, fetcher) {
  try {
    return await fetcher();
  } catch (error) {
    if (isOfflineApiUnavailableError(error)) {
      t.diagnostic(`${label} unavailable on target node: ${error.message}`);
      return null;
    }
    throw error;
  }
}

function assertOfflineAllowanceResponse(page) {
  assertNonNegativeInteger(page.total, "offline allowances total");
  assert.ok(Array.isArray(page.items), "offline allowances must expose items array");
  if (page.items.length === 0) {
    return;
  }
  const entry = page.items[0];
  assert.equal(typeof entry.certificate_id_hex, "string", "allowance certificate_id_hex must be a string");
  assert.equal(typeof entry.controller_id, "string", "allowance controller_id must be a string");
  assert.equal(typeof entry.asset_id, "string", "allowance asset_id must be a string");
  assertNonNegativeInteger(entry.registered_at_ms, "allowance registered_at_ms");
  assertNonNegativeInteger(entry.expires_at_ms, "allowance expires_at_ms");
  assert.ok(
    Object.prototype.hasOwnProperty.call(entry, "integrity_metadata"),
    "allowance entries must expose integrity_metadata",
  );
  if (entry.integrity_metadata !== null) {
    assert.equal(
      typeof entry.integrity_metadata.policy,
      "string",
      "allowance integrity_metadata.policy must be a string",
    );
    if (entry.integrity_metadata.provisioned) {
      const provisioned = entry.integrity_metadata.provisioned;
      assert.equal(
        typeof provisioned.manifest_schema,
        "string",
        "allowance provisioned manifest_schema must be a string",
      );
    }
  }
}

function assertOfflineSummaryResponse(page) {
  assertNonNegativeInteger(page.total, "offline summaries total");
  assert.ok(Array.isArray(page.items), "offline summaries must expose items array");
  if (page.items.length === 0) {
    return;
  }
  const entry = page.items[0];
  assert.equal(
    typeof entry.certificate_id_hex,
    "string",
    "summary certificate_id_hex must be a string",
  );
  assert.equal(typeof entry.controller_id, "string", "summary controller_id must be a string");
  assert.equal(
    typeof entry.summary_hash_hex,
    "string",
    "summary summary_hash_hex must be a string",
  );
  assertCounterMap(entry.apple_key_counters, "summary apple_key_counters");
  assertCounterMap(entry.android_series_counters, "summary android_series_counters");
  assertCounterMap(entry.policy_key_counters, "summary policy_key_counters");
  assertOfflineCounterTotals(entry.counter_totals);
  if (entry.metadata !== null && entry.metadata !== undefined) {
    assert.ok(isPlainObject(entry.metadata), "summary metadata must be a plain object");
  }
}

function assertCounterMap(map, label) {
  assert.ok(isPlainObject(map), `${label} must be a plain object`);
  for (const [key, value] of Object.entries(map)) {
    assert.ok(typeof key === "string" && key.length > 0, `${label} keys must be non-empty strings`);
    assertNonNegativeInteger(value, `${label}.${key}`);
  }
}

function assertOfflineCounterTotals(totals) {
  if (totals === null || totals === undefined) {
    return;
  }
  assert.ok(isPlainObject(totals), "summary counter_totals must be an object when present");
  assertNonNegativeInteger(totals.total_counters, "summary counter_totals.total_counters");
  assertNonNegativeInteger(totals.total_weight, "summary counter_totals.total_weight");
  assertNonNegativeInteger(totals.apple, "summary counter_totals.apple");
  assertNonNegativeInteger(totals.android, "summary counter_totals.android");
  assertNonNegativeInteger(totals.policy, "summary counter_totals.policy");
}

function assertOfflineTransferResponse(page) {
  assertNonNegativeInteger(page.total, "offline transfers total");
  assert.ok(Array.isArray(page.items), "offline transfers must expose items array");
  if (page.items.length === 0) {
    return;
  }
  const entry = page.items[0];
  assert.equal(typeof entry.bundle_id_hex, "string", "transfer bundle_id_hex must be a string");
  assert.equal(typeof entry.controller_id, "string", "transfer controller_id must be a string");
  assert.equal(typeof entry.receiver_id, "string", "transfer receiver_id must be a string");
  assert.ok(typeof entry.total_amount === "string", "transfer total_amount must be a string");
  assert.equal(typeof entry.status, "string", "transfer status must be a string");
  assertNonNegativeInteger(entry.recorded_at_ms, "transfer recorded_at_ms");
  assert.ok(
    Object.prototype.hasOwnProperty.call(entry, "integrity_metadata"),
    "transfer entries must expose integrity_metadata",
  );
  if (entry.integrity_metadata !== null) {
    assert.equal(
      typeof entry.integrity_metadata.policy,
      "string",
      "transfer integrity_metadata.policy must be a string",
    );
  }
  assert.ok(
    Array.isArray(entry.status_transitions),
    "transfer entries must expose status_transitions",
  );
  if (entry.status_transitions.length > 0) {
    const transition = entry.status_transitions[0];
    assert.equal(typeof transition.status, "string", "status transition status must be a string");
    assertNonNegativeInteger(
      transition.transitioned_at_ms,
      "status transition transitioned_at_ms",
    );
    if (transition.verdict_snapshot !== null) {
      assert.equal(
        typeof transition.verdict_snapshot.certificate_id,
        "string",
        "status transition verdict_snapshot.certificate_id must be a string",
      );
    }
  }
  assert.ok(
    Object.prototype.hasOwnProperty.call(entry, "verdict_snapshot"),
    "transfer entries must expose verdict_snapshot",
  );
  if (entry.verdict_snapshot !== null) {
    assert.equal(
      typeof entry.verdict_snapshot.certificate_id,
      "string",
      "transfer verdict_snapshot.certificate_id must be a string",
    );
  }
}

function assertOfflineRevocationResponse(page) {
  assertNonNegativeInteger(page.total, "offline revocations total");
  assert.ok(Array.isArray(page.items), "offline revocations must expose items array");
  if (page.items.length === 0) {
    return;
  }
  const entry = page.items[0];
  assert.equal(typeof entry.verdict_id_hex, "string", "revocation verdict_id_hex must be a string");
  assert.equal(typeof entry.issuer_id, "string", "revocation issuer_id must be a string");
  assert.equal(typeof entry.issuer_display, "string", "revocation issuer_display must be a string");
  assertNonNegativeInteger(entry.revoked_at_ms, "revocation revoked_at_ms");
  assert.equal(typeof entry.reason, "string", "revocation reason must be a string");
  if (entry.note !== null) {
    assert.equal(typeof entry.note, "string", "revocation note must be a string when present");
  }
  if (entry.metadata !== null) {
    assert.ok(
      isPlainObject(entry.metadata),
      "revocation metadata must be a plain object when present",
    );
  }
  assert.ok(isPlainObject(entry.record), "revocation record must be a plain object");
}

function assertOfflineRejectionStatsResponse(page) {
  assertNonNegativeInteger(page.total, "offline rejection stats total");
  assert.ok(Array.isArray(page.items), "offline rejection stats must expose items array");
  if (page.items.length === 0) {
    return;
  }
  const entry = page.items[0];
  assert.equal(typeof entry.platform, "string", "rejection stats platform must be a string");
  assert.equal(typeof entry.reason, "string", "rejection stats reason must be a string");
  assertNonNegativeInteger(entry.count, "rejection stats entry count");
}

function assertKaigiRelaySummaryList(list) {
  assertNonNegativeInteger(list.total, "kaigi relay list total");
  assert.ok(Array.isArray(list.items), "kaigi relay list items must be an array");
  list.items.forEach((entry, index) => {
    assertKaigiRelaySummary(entry, `kaigi relay summary[${index}]`);
  });
}

function assertKaigiRelaySummary(summary, label = "kaigi relay summary") {
  assert.ok(isPlainObject(summary), `${label} must be an object`);
  assert.ok(isNonEmptyString(summary.relay_id), `${label}.relay_id must be a string`);
  assert.ok(isNonEmptyString(summary.domain), `${label}.domain must be a string`);
  assertNonNegativeInteger(summary.bandwidth_class, `${label}.bandwidth_class`);
  assert.ok(
    isNonEmptyString(summary.hpke_fingerprint_hex),
    `${label}.hpke_fingerprint_hex must be a string`,
  );
  if (summary.status !== undefined && summary.status !== null) {
    assert.ok(
      KAIGI_HEALTH_STATUSES.has(summary.status),
      `${label}.status must be healthy, degraded, or unavailable`,
    );
  }
  if (summary.reported_at_ms !== undefined && summary.reported_at_ms !== null) {
    assertNonNegativeInteger(summary.reported_at_ms, `${label}.reported_at_ms`);
  }
}

function assertKaigiRelayDetail(detail, relayId) {
  assert.ok(detail && typeof detail === "object", "kaigi relay detail must be an object");
  assert.equal(
    typeof detail.hpke_public_key_b64,
    "string",
    "kaigi relay detail hpke_public_key_b64 must be a string",
  );
  assertKaigiRelaySummary(detail.relay, "kaigi relay detail.relay");
  if (detail.reported_call !== undefined && detail.reported_call !== null) {
    assert.equal(typeof detail.reported_call, "object", "kaigi relay detail reported_call must be an object");
    assert.ok(
      isNonEmptyString(detail.reported_call.domain_id),
      "kaigi relay detail reported_call.domain_id must be a string",
    );
    assert.ok(
      isNonEmptyString(detail.reported_call.call_name),
      "kaigi relay detail reported_call.call_name must be a string",
    );
  }
  if (detail.reported_by !== undefined && detail.reported_by !== null) {
    assert.ok(
      isNonEmptyString(detail.reported_by),
      "kaigi relay detail reported_by must be a string when present",
    );
  }
  if (detail.notes !== undefined && detail.notes !== null) {
    assert.equal(typeof detail.notes, "string", "kaigi relay detail notes must be a string");
  }
  if (detail.metrics) {
    assertKaigiRelayDomainMetrics(detail.metrics, "kaigi relay detail.metrics");
  }
  assert.equal(
    detail.relay.relay_id,
    relayId,
    "kaigi relay detail relay_id should match the requested relay id",
  );
}

function assertKaigiRelayHealthSnapshot(snapshot) {
  assert.ok(snapshot && typeof snapshot === "object", "kaigi relay health snapshot must be an object");
  assertNonNegativeInteger(snapshot.healthy_total, "kaigi relay health healthy_total");
  assertNonNegativeInteger(snapshot.degraded_total, "kaigi relay health degraded_total");
  assertNonNegativeInteger(snapshot.unavailable_total, "kaigi relay health unavailable_total");
  assertNonNegativeInteger(snapshot.reports_total, "kaigi relay health reports_total");
  assertNonNegativeInteger(snapshot.registrations_total, "kaigi relay health registrations_total");
  assertNonNegativeInteger(snapshot.failovers_total, "kaigi relay health failovers_total");
  assert.ok(
    Array.isArray(snapshot.domains),
    "kaigi relay health domains must be an array",
  );
  snapshot.domains.forEach((metrics, index) => {
    assertKaigiRelayDomainMetrics(metrics, `kaigi relay health domains[${index}]`);
  });
}

function assertKaigiRelayDomainMetrics(metrics, label) {
  assert.ok(metrics && typeof metrics === "object", `${label} must be an object`);
  assert.ok(isNonEmptyString(metrics.domain), `${label}.domain must be a string`);
  assertNonNegativeInteger(metrics.registrations_total, `${label}.registrations_total`);
  assertNonNegativeInteger(metrics.manifest_updates_total, `${label}.manifest_updates_total`);
  assertNonNegativeInteger(metrics.failovers_total, `${label}.failovers_total`);
  assertNonNegativeInteger(metrics.health_reports_total, `${label}.health_reports_total`);
}

function assertVerifyingKeyListEntry(entry, label = "verifying key summary") {
  assert.ok(isPlainObject(entry), `${label} must be an object`);
  assertVerifyingKeyIdentifier(entry.id, `${label}.id`);
  if (entry.record !== undefined && entry.record !== null) {
    assertVerifyingKeyRecord(entry.record, `${label}.record`);
  }
}

function assertVerifyingKeyIdentifier(id, label = "verifying key id") {
  assert.ok(isPlainObject(id), `${label} must be an object`);
  assert.ok(isNonEmptyString(id.backend), `${label}.backend must be a string`);
  assert.ok(isNonEmptyString(id.name), `${label}.name must be a string`);
}

function assertVerifyingKeyDetail(detail, expectedId = null) {
  assert.ok(isPlainObject(detail), "verifying key detail must be an object");
  assertVerifyingKeyIdentifier(detail.id, "verifying key detail.id");
  if (expectedId) {
    assert.equal(detail.id.backend, expectedId.backend, "verifying key detail backend mismatch");
    assert.equal(detail.id.name, expectedId.name, "verifying key detail name mismatch");
  }
  assertVerifyingKeyRecord(detail.record, "verifying key detail.record");
}

function assertVerifyingKeyRecord(record, label = "verifying key record") {
  assert.ok(isPlainObject(record), `${label} must be an object`);
  assertNonNegativeInteger(record.version, `${label}.version`);
  assert.ok(isNonEmptyString(record.circuit_id), `${label}.circuit_id must be a string`);
  assert.ok(isNonEmptyString(record.backend), `${label}.backend must be a string`);
  if (record.curve !== null) {
    assert.ok(isNonEmptyString(record.curve), `${label}.curve must be a string when present`);
  }
  assertHexString(record.public_inputs_schema_hash, `${label}.public_inputs_schema_hash`);
  assertHexString(record.commitment_hex, `${label}.commitment_hex`);
  assertNonNegativeInteger(record.vk_len, `${label}.vk_len`);
  if (record.max_proof_bytes !== null) {
    assertNonNegativeInteger(record.max_proof_bytes, `${label}.max_proof_bytes`);
  }
  if (record.gas_schedule_id !== null) {
    assert.ok(isNonEmptyString(record.gas_schedule_id), `${label}.gas_schedule_id must be a string`);
  }
  if (record.metadata_uri_cid !== null) {
    assert.ok(
      isNonEmptyString(record.metadata_uri_cid),
      `${label}.metadata_uri_cid must be a string`,
    );
  }
  if (record.vk_bytes_cid !== null) {
    assert.ok(isNonEmptyString(record.vk_bytes_cid), `${label}.vk_bytes_cid must be a string`);
  }
  if (record.activation_height !== null) {
    assertNonNegativeInteger(record.activation_height, `${label}.activation_height`);
  }
  if (record.deprecation_height !== null) {
    assertNonNegativeInteger(record.deprecation_height, `${label}.deprecation_height`);
  }
  if (record.withdraw_height !== null) {
    assertNonNegativeInteger(record.withdraw_height, `${label}.withdraw_height`);
  }
  if (record.status !== undefined && record.status !== null) {
    assert.ok(isNonEmptyString(record.status), `${label}.status must be a string`);
  }
  if (record.inline_key !== null) {
    assertVerifyingKeyInline(record.inline_key, `${label}.inline_key`);
  }
}

function assertVerifyingKeyInline(inline, label = "verifying key inline payload") {
  assert.ok(isPlainObject(inline), `${label} must be an object`);
  assert.ok(isNonEmptyString(inline.backend), `${label}.backend must be a string`);
  assert.ok(isNonEmptyString(inline.bytes_b64), `${label}.bytes_b64 must be a string`);
}

function assertRepoAgreementSnapshot(entry, label) {
  assert.ok(entry && typeof entry === "object", `${label} must be an object`);
  assert.equal(typeof entry.id, "string", `${label}.id must be a string`);
  assert.notEqual(entry.id.length, 0, `${label}.id must not be empty`);
  assert.equal(typeof entry.initiator, "string", `${label}.initiator must be a string`);
  assert.notEqual(entry.initiator.length, 0, `${label}.initiator must not be empty`);
  assert.equal(typeof entry.counterparty, "string", `${label}.counterparty must be a string`);
  assert.notEqual(entry.counterparty.length, 0, `${label}.counterparty must not be empty`);
  if (entry.custodian !== null) {
    assert.equal(typeof entry.custodian, "string", `${label}.custodian must be a string when set`);
  }
  assertRepoLegSnapshot(entry.cashLeg, `${label}.cashLeg`);
  assertRepoLegSnapshot(entry.collateralLeg, `${label}.collateralLeg`);
  assert.equal(typeof entry.rateBps, "number", `${label}.rateBps must be a number`);
  assert.ok(Number.isFinite(entry.rateBps), `${label}.rateBps must be finite`);
  assertNonNegativeInteger(entry.maturityTimestampMs, `${label}.maturityTimestampMs`);
  assertNonNegativeInteger(entry.initiatedTimestampMs, `${label}.initiatedTimestampMs`);
  assertNonNegativeInteger(entry.lastMarginCheckTimestampMs, `${label}.lastMarginCheckTimestampMs`);
  assertRepoGovernanceSnapshot(entry.governance, `${label}.governance`);
}

function assertRepoLegSnapshot(leg, label) {
  assert.ok(leg && typeof leg === "object", `${label} must be an object`);
  assert.equal(
    typeof leg.assetDefinitionId,
    "string",
    `${label}.assetDefinitionId must be a string`,
  );
  assert.notEqual(
    leg.assetDefinitionId.length,
    0,
    `${label}.assetDefinitionId must not be empty`,
  );
  assert.equal(typeof leg.quantity, "string", `${label}.quantity must be a string`);
  assert.notEqual(leg.quantity.length, 0, `${label}.quantity must not be empty`);
}

function assertRepoGovernanceSnapshot(governance, label) {
  assert.ok(governance && typeof governance === "object", `${label} must be an object`);
  assert.equal(typeof governance.haircutBps, "number", `${label}.haircutBps must be a number`);
  assert.ok(Number.isFinite(governance.haircutBps), `${label}.haircutBps must be finite`);
  assertNonNegativeInteger(governance.marginFrequencySecs, `${label}.marginFrequencySecs`);
}

function assertContractInstanceListResponse(page, expectedNamespace = null) {
  assert.ok(page && typeof page === "object", "contract instance page must be an object");
  assert.equal(typeof page.namespace, "string", "contract instance page namespace must be a string");
  if (expectedNamespace) {
    assert.equal(
      page.namespace,
      expectedNamespace,
      "contract instance page should echo namespace",
    );
  }
  assertNonNegativeInteger(page.total, "contract instance page total");
  assertNonNegativeInteger(page.offset, "contract instance page offset");
  assertNonNegativeInteger(page.limit, "contract instance page limit");
  assert.ok(
    Array.isArray(page.instances),
    "contract instance page must include an instances array",
  );
  page.instances.forEach((instance, index) => {
    assertContractInstanceRecord(instance, `contract instance ${index}`);
  });
}

function assertContractInstanceRecord(record, label = "contract instance record") {
  assert.ok(record && typeof record === "object", `${label} must be an object`);
  assert.equal(typeof record.contract_id, "string", `${label}.contract_id must be a string`);
  assert.notEqual(record.contract_id.length, 0, `${label}.contract_id must not be empty`);
  assertHexString(record.code_hash_hex, `${label}.code_hash_hex`);
}

function assertAttachmentMetadata(entry, label) {
  assert.ok(entry && typeof entry === "object", `${label} must be an object`);
  assert.ok(
    typeof entry.id === "string" && entry.id.length > 0,
    `${label}.id must be a non-empty string`,
  );
  assert.ok(
    typeof entry.contentType === "string" && entry.contentType.length > 0,
    `${label}.contentType must be a non-empty string`,
  );
  assertNonNegativeInteger(entry.size, `${label}.size`);
  assertNonNegativeInteger(entry.createdMs, `${label}.createdMs`);
  if (entry.tenant !== undefined && entry.tenant !== null) {
    assert.equal(typeof entry.tenant, "string", `${label}.tenant must be a string when present`);
  }
}

function assertTriggerRecord(record, label) {
  assert.ok(record && typeof record === "object", `${label} must be an object`);
  assert.equal(typeof record.id, "string", `${label}.id must be a string`);
  assert.notEqual(record.id.length, 0, `${label}.id must not be empty`);
  assert.ok(
    record.action && typeof record.action === "object" && !Array.isArray(record.action),
    `${label}.action must be an object`,
  );
  if (record.metadata !== undefined && record.metadata !== null) {
    assert.ok(
      typeof record.metadata === "object" && !Array.isArray(record.metadata),
      `${label}.metadata must be an object when present`,
    );
  }
}

function assertHexString(value, label) {
  assert.equal(typeof value, "string", `${label} must be a string`);
  assert.notEqual(value.length, 0, `${label} must not be empty`);
  assert.ok(/^[0-9a-f]+$/i.test(value), `${label} must be hexadecimal`);
}

function assertNumberLike(value, label) {
  const isNumericString =
    typeof value === "string" && value.trim().length > 0 && Number.isFinite(Number(value));
  if (
    typeof value === "number" ||
    typeof value === "bigint" ||
    isNumericString
  ) {
    return;
  }
  throw new Error(`${label} must be numeric`);
}

function assertNonNegativeInteger(value, label) {
  assert.ok(Number.isInteger(value) && value >= 0, `${label} must be a non-negative integer`);
}

function assertNonNegativeNumber(value, label) {
  assert.equal(typeof value, "number", `${label} must be a number`);
  assert.ok(Number.isFinite(value), `${label} must be finite`);
  assert.ok(value >= 0, `${label} must be non-negative`);
}

function assertEvidenceRecord(entry) {
  assert.ok(entry && typeof entry === "object", "evidence entry must be an object");
  assert.equal(typeof entry.kind, "string", "evidence entry must expose a kind");
  assertNonNegativeInteger(
    entry.recorded_height,
    "evidence entry recorded_height must be non-negative",
  );
  assertNonNegativeInteger(
    entry.recorded_view,
    "evidence entry recorded_view must be non-negative",
  );
  assertNonNegativeInteger(entry.recorded_ms, "evidence entry recorded_ms must be non-negative");
  switch (entry.kind) {
    case "DoublePrepare":
    case "DoubleCommit":
      assert.equal(typeof entry.phase, "string", "double vote evidence must include a phase");
      assertNonNegativeInteger(entry.height, "double vote evidence height must be non-negative");
      assertNonNegativeInteger(entry.view, "double vote evidence view must be non-negative");
      assertNonNegativeInteger(entry.epoch, "double vote evidence epoch must be non-negative");
      assert.equal(typeof entry.signer, "string", "double vote evidence signer must be a string");
      assertHexString(entry.block_hash_1, "double vote evidence block_hash_1");
      assertHexString(entry.block_hash_2, "double vote evidence block_hash_2");
      break;
    case "InvalidQc":
      assertNonNegativeInteger(entry.height, "invalid QC height must be non-negative");
      assertNonNegativeInteger(entry.view, "invalid QC view must be non-negative");
      assertNonNegativeInteger(entry.epoch, "invalid QC epoch must be non-negative");
      assertHexString(entry.subject_block_hash, "invalid QC subject_block_hash");
      assert.equal(typeof entry.phase, "string", "invalid QC entries must expose a phase");
      assert.equal(typeof entry.reason, "string", "invalid QC entries must expose a reason");
      break;
    case "InvalidProposal":
      assertNonNegativeInteger(entry.height, "invalid proposal height must be non-negative");
      assertNonNegativeInteger(entry.view, "invalid proposal view must be non-negative");
      assertNonNegativeInteger(entry.epoch, "invalid proposal epoch must be non-negative");
      assertHexString(entry.subject_block_hash, "invalid proposal subject_block_hash");
      assertHexString(entry.payload_hash, "invalid proposal payload_hash");
      assert.equal(typeof entry.reason, "string", "invalid proposal entries must expose a reason");
      break;
    case "Censorship":
      assertHexString(entry.tx_hash, "censorship tx_hash");
      assertNonNegativeInteger(entry.receipt_count, "censorship receipt_count must be non-negative");
      assertNonNegativeInteger(entry.min_height, "censorship min_height must be non-negative");
      assertNonNegativeInteger(entry.max_height, "censorship max_height must be non-negative");
      assert.ok(Array.isArray(entry.signers), "censorship signers must be an array");
      entry.signers.forEach((signer) => {
        assert.equal(typeof signer, "string", "censorship signer must be a string");
      });
      break;
    default:
      if ("detail" in entry && entry.detail !== undefined && entry.detail !== null) {
        assert.equal(typeof entry.detail, "string", "unknown evidence detail must be a string");
      }
      break;
  }
}

function assertProverReportResult(result, label = "prover report response") {
  assert.ok(result && typeof result === "object", `${label} must be an object`);
  switch (result.kind) {
    case "reports":
      assert.ok(
        Array.isArray(result.reports),
        `${label}.reports must be an array when kind is reports`,
      );
      result.reports.forEach((entry, index) =>
        assertProverReportRecord(entry, `${label}.reports[${index}]`),
      );
      break;
    case "ids":
      assert.ok(Array.isArray(result.ids), `${label}.ids must be an array when kind is ids`);
      result.ids.forEach((value, index) => {
        assert.ok(
          isNonEmptyString(value),
          `${label}.ids[${index}] must be a non-empty string`,
        );
      });
      break;
    case "messages":
      assert.ok(
        Array.isArray(result.messages),
        `${label}.messages must be an array when kind is messages`,
      );
      result.messages.forEach((entry, index) => {
        assert.ok(isPlainObject(entry), `${label}.messages[${index}] must be an object`);
        assert.ok(
          isNonEmptyString(entry.id),
          `${label}.messages[${index}].id must be a non-empty string`,
        );
        if (entry.error !== null) {
          assert.equal(
            typeof entry.error,
            "string",
            `${label}.messages[${index}].error must be null or a string`,
          );
        }
      });
      break;
    default:
      throw new Error(`${label} has unknown kind: ${String(result.kind)}`);
  }
}

function assertProverReportRecord(entry, label) {
  assert.ok(entry && typeof entry === "object", `${label} must be an object`);
  assert.ok(isNonEmptyString(entry.id), `${label}.id must be a non-empty string`);
  assert.equal(typeof entry.ok, "boolean", `${label}.ok must be a boolean`);
  if (entry.error !== null) {
    assert.equal(typeof entry.error, "string", `${label}.error must be null or a string`);
  }
  assert.ok(isNonEmptyString(entry.content_type), `${label}.content_type must be a string`);
  assertNonNegativeInteger(entry.size, `${label}.size`);
  assertNonNegativeInteger(entry.created_ms, `${label}.created_ms`);
  assertNonNegativeInteger(entry.processed_ms, `${label}.processed_ms`);
  assertNonNegativeInteger(entry.latency_ms, `${label}.latency_ms`);
  if (entry.zk1_tags !== null) {
    assert.ok(Array.isArray(entry.zk1_tags), `${label}.zk1_tags must be an array when present`);
    entry.zk1_tags.forEach((tag, index) => {
      assert.ok(
        isNonEmptyString(tag),
        `${label}.zk1_tags[${index}] must be a non-empty string`,
      );
    });
  }
}

function hasProverReportEntries(result) {
  switch (result.kind) {
    case "reports":
      return Array.isArray(result.reports) && result.reports.length > 0;
    case "ids":
      return Array.isArray(result.ids) && result.ids.length > 0;
    case "messages":
      return Array.isArray(result.messages) && result.messages.length > 0;
    default:
      return false;
  }
}

function countFailedProverReports(result) {
  if (result.kind !== "reports" || !Array.isArray(result.reports)) {
    return 0;
  }
  return result.reports.filter((entry) => entry && entry.ok === false).length;
}
