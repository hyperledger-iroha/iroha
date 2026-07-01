import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import test from "node:test";

const REPO_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const SCRIPT = join(REPO_ROOT, "scripts", "sccp_ton_taira_xor_deploy.mjs");
const TON_DESTINATION_BINDING_HASH =
  "0x8651c1b818973f92050f69e66e8491e9681d23db1cb37393b9ea15c5e7e02799";
const TON_TESTNET_CHAIN_ID_HEX =
  "0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd";
const DEFAULT_TON_FINALIZE_MESSAGE_VALUE_NANO = "100000000";

function hex32(byte) {
  return `0x${Buffer.alloc(32, byte).toString("hex")}`;
}

function tonRaw(byte) {
  return `0:${Buffer.alloc(32, byte).toString("hex")}`;
}

async function writeJson(path, value) {
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
  return path;
}

function sha256HexJson(value) {
  const canonical = JSON.stringify(stableJsonValue(value));
  return `0x${createHash("sha256").update(canonical).digest("hex")}`;
}

function stableJsonValue(value) {
  if (Array.isArray(value)) {
    return value.map(stableJsonValue);
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, entry]) => [key, stableJsonValue(entry)]),
    );
  }
  return value;
}

async function fixtureRoot() {
  return mkdtemp(join(tmpdir(), "sccp-ton-taira-xor-"));
}

async function writeFixtureFiles(root, overrides = {}) {
  const proofArtifactHash = overrides.proofArtifactHash ?? hex32(0x90);
  const deploymentEvidence = overrides.deploymentEvidence ?? {
    schema: "ton-testnet-deployment-evidence/v1",
    routeId: "taira_ton_xor",
    contracts: {
      token: tonRaw(0x11),
      bridge: tonRaw(0x22),
      sourceBridge: tonRaw(0x33),
      verifier: tonRaw(0x44),
    },
  };
  const sourceVerifierMaterial = overrides.sourceVerifierMaterial ?? {
    schema: "sccp-ton-source-verifier-material/v1",
    source_domain: 4,
    target_domain: 0,
    source_chain: "ton-testnet",
    masterchainConfigHash: hex32(0x45),
  };
  const sourceAdapterEngineDeployment =
    overrides.sourceAdapterEngineDeployment ?? {
      schema: "sccp-ton-source-adapter-engine-deployment/v1",
      source_domain: 4,
      target_domain: 0,
      source_chain: "ton-testnet",
      deployment_receipt_hash: hex32(0x46),
    };
  const prover = (seed, extra = {}) => ({
    module_url: `https://provers.sora.org/ton-${seed}.js`,
    module_hash: hex32(seed),
    manifest_hash: hex32(seed + 1),
    expected_exports: ["tonSccpProve", `tonSccpSelfTest${seed}`],
    bound_route_hash: TON_DESTINATION_BINDING_HASH,
    bound_proof_hash: proofArtifactHash,
    ...extra,
  });
  const tairaContract = overrides.tairaContract ?? {
    settlementAssetDefinitionId: "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
    contractArtifactB64: Buffer.alloc(32, 0x5a).toString("base64"),
    artifactSha256: hex32(0x51),
    codeHash: hex32(0x52),
    vkRef: {
      backend: "halo2/ipa",
      name: "taira_xor_burn_record_v1",
    },
    gasLimit: 2_000_000,
  };
  const offlineFullTomlEvidence = overrides.offlineFullTomlEvidence ?? {
    schema: "taira-full-config-evidence/v1",
    routeId: "taira_ton_xor",
    fullTomlSha256: hex32(0x53),
  };

  const paths = {
    deploymentEvidence: join(root, "deployment-evidence.json"),
    sourceVerifierMaterial: join(root, "source-verifier-material.json"),
    sourceAdapterEngineDeployment: join(
      root,
      "source-adapter-engine-deployment.json",
    ),
    destinationBrowserProver: join(root, "destination-browser-prover.json"),
    sourceBrowserProver: join(root, "source-browser-prover.json"),
    tairaContract: join(root, "taira-contract.json"),
    offlineFullTomlEvidence: join(root, "offline-full-toml.json"),
    out: join(root, "route.manifest.json"),
    publishOut: join(root, "route.upsert-isi.json"),
  };
  await writeJson(paths.deploymentEvidence, deploymentEvidence);
  await writeJson(paths.sourceVerifierMaterial, sourceVerifierMaterial);
  await writeJson(
    paths.sourceAdapterEngineDeployment,
    sourceAdapterEngineDeployment,
  );
  await writeJson(
    paths.destinationBrowserProver,
    overrides.destinationBrowserProver ?? prover(0x61),
  );
  await writeJson(
    paths.sourceBrowserProver,
    overrides.sourceBrowserProver ?? prover(0x71),
  );
  await writeJson(paths.tairaContract, tairaContract);
  await writeJson(paths.offlineFullTomlEvidence, offlineFullTomlEvidence);
  return { paths, proofArtifactHash };
}

function routeManifestArgs(paths, proofArtifactHash, extra = {}) {
  return [
    "route-manifest",
    "--token",
    extra.token ?? tonRaw(0x11),
    "--bridge",
    extra.bridge ?? tonRaw(0x22),
    "--source-bridge",
    extra.sourceBridge ?? tonRaw(0x33),
    "--verifier",
    extra.verifier ?? tonRaw(0x44),
    "--ton-finalize-message-value-nano",
    extra.tonFinalizeMessageValueNano ??
      DEFAULT_TON_FINALIZE_MESSAGE_VALUE_NANO,
    "--verifier-code-hash",
    extra.verifierCodeHash ?? hex32(0x81),
    "--verifier-key-hash",
    extra.verifierKeyHash ?? hex32(0x82),
    "--proof-artifact-hash",
    proofArtifactHash,
    "--proving-key-hash",
    extra.provingKeyHash ?? hex32(0x83),
    "--deployment-evidence",
    paths.deploymentEvidence,
    "--source-verifier-material",
    paths.sourceVerifierMaterial,
    "--source-adapter-engine-deployment",
    paths.sourceAdapterEngineDeployment,
    "--destination-browser-prover-manifest",
    paths.destinationBrowserProver,
    "--source-browser-prover-manifest",
    paths.sourceBrowserProver,
    "--taira-contract",
    paths.tairaContract,
    "--post-deploy-source-bridge-config-hash",
    extra.sourceBridgeConfigHash ?? hex32(0x84),
    "--post-deploy-source-event-transaction-id",
    extra.sourceEventTransactionId ?? hex32(0x85),
    "--post-deploy-route-canary-evidence-hash",
    extra.routeCanaryEvidenceHash ?? hex32(0x86),
    "--post-deploy-route-canary-transaction-id",
    extra.routeCanaryTransactionId ?? hex32(0x87),
    "--offline-full-toml-evidence",
    paths.offlineFullTomlEvidence,
    "--out",
    paths.out,
  ];
}

function runTonCli(args, options = {}) {
  return spawnSync(process.execPath, [SCRIPT, ...args], {
    cwd: REPO_ROOT,
    encoding: "utf8",
    env: { ...process.env, ...options.env },
  });
}

test("TON route manifest renders production-ready offline evidence", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const result = runTonCli(routeManifestArgs(paths, proofArtifactHash));

  assert.equal(result.status, 0, result.stderr);
  const summary = JSON.parse(result.stdout);
  assert.equal(summary.ok, true);
  assert.equal(summary.routeId, "taira_ton_xor");
  assert.equal(summary.assetKey, "xor");
  assert.equal(summary.productionReady, true);
  assert.equal(summary.destinationBindingHash, TON_DESTINATION_BINDING_HASH);
  assert.equal(
    summary.tonFinalizeMessageValueNano,
    DEFAULT_TON_FINALIZE_MESSAGE_VALUE_NANO,
  );

  const envelope = JSON.parse(await readFile(paths.out, "utf8"));
  assert.equal(
    envelope.schema,
    "iroha-sccp-taira-ton-xor-route-manifest-draft/v1",
  );
  assert.equal(envelope.manifest.route_id, "taira_ton_xor");
  assert.equal(envelope.manifest.asset_key, "xor");
  assert.equal(envelope.manifest.counterparty_domain, 4);
  assert.equal(envelope.manifest.chain, "ton-testnet");
  assert.equal(envelope.manifest.chain_id_hex, TON_TESTNET_CHAIN_ID_HEX);
  assert.equal(envelope.manifest.network_id_hex, TON_TESTNET_CHAIN_ID_HEX);
  assert.equal(
    envelope.manifest.ton_finalize_message_value_nano,
    DEFAULT_TON_FINALIZE_MESSAGE_VALUE_NANO,
  );
  assert.equal(envelope.manifest.production_ready, true);
  assert.equal(envelope.manifest.disabled_reason, undefined);
  assert.equal(
    envelope.manifest.destination_browser_prover.bound_route_hash,
    TON_DESTINATION_BINDING_HASH,
  );
  assert.equal(
    envelope.manifest.deployment_evidence_sha256,
    sha256HexJson({
      schema: "ton-testnet-deployment-evidence/v1",
      routeId: "taira_ton_xor",
      contracts: {
        token: tonRaw(0x11),
        bridge: tonRaw(0x22),
        sourceBridge: tonRaw(0x33),
        verifier: tonRaw(0x44),
      },
    }),
  );
  assert.doesNotMatch(JSON.stringify(envelope), /private|secret|mnemonic/iu);
});

test("TON publish-route-manifest writes a reviewable ISI artifact without submit", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const render = runTonCli(routeManifestArgs(paths, proofArtifactHash));
  assert.equal(render.status, 0, render.stderr);

  const publish = runTonCli([
    "publish-route-manifest",
    "--manifest",
    paths.out,
    "--out",
    paths.publishOut,
  ]);
  assert.equal(publish.status, 0, publish.stderr);

  const artifact = JSON.parse(await readFile(paths.publishOut, "utf8"));
  assert.equal(artifact.schema, "iroha-sccp-route-manifest-isi/v1");
  assert.equal(artifact.routeId, "taira_ton_xor");
  assert.equal(artifact.requiredPermission, "CanManageSccpRouteManifests");
  assert.equal(artifact.productionReady, true);
  assert.equal(
    artifact.tonFinalizeMessageValueNano,
    DEFAULT_TON_FINALIZE_MESSAGE_VALUE_NANO,
  );
  assert.equal(artifact.submission, undefined);
  assert.equal(
    artifact.instruction.UpsertSccpRouteManifest.manifest.route_id,
    "taira_ton_xor",
  );
});

test("TON route manifest rejects non-canonical TON addresses before writing", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const result = runTonCli(
    routeManifestArgs(paths, proofArtifactHash, {
      token:
        "0:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    }),
  );

  assert.notEqual(result.status, 0);
  assert.match(
    result.stderr,
    /--token must be canonical lowercase TON raw address text/u,
  );
  await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
});

test("TON route manifest rejects missing finalize message value", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const result = runTonCli(
    routeManifestArgs(paths, proofArtifactHash, {
      tonFinalizeMessageValueNano: "0",
    }),
  );

  assert.notEqual(result.status, 0);
  assert.match(
    result.stderr,
    /--ton-finalize-message-value-nano must be a positive integer decimal string/u,
  );
  await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
});

test("TON route manifest rejects secret-like public evidence", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root, {
    sourceVerifierMaterial: {
      schema: "sccp-ton-source-verifier-material/v1",
      source_domain: 4,
      privateKey: "must-not-ship",
    },
  });
  const result = runTonCli(routeManifestArgs(paths, proofArtifactHash));

  assert.notEqual(result.status, 0);
  assert.match(
    result.stderr,
    /TON source verifier material\.privateKey looks secret-like/u,
  );
  await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
});

test("TON route manifest rejects forged browser prover bindings", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root, {
    destinationBrowserProver: {
      module_url: "https://provers.sora.org/ton-destination.js",
      module_hash: hex32(0x61),
      manifest_hash: hex32(0x62),
      expected_exports: ["tonSccpProve"],
      bound_route_hash: hex32(0xee),
      bound_proof_hash: hex32(0x90),
    },
  });
  const result = runTonCli(routeManifestArgs(paths, proofArtifactHash));

  assert.notEqual(result.status, 0);
  assert.match(
    result.stderr,
    /destination_browser_prover\.bound_route_hash must match the TON destination binding hash/u,
  );
  await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
});

test("TON route manifest rejects ambiguous browser prover exports", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root, {
    sourceBrowserProver: {
      module_url: "https://provers.sora.org/ton-source.js",
      module_hash: hex32(0x71),
      manifest_hash: hex32(0x72),
      expected_exports: ["tonSccpProve", "tonSccpProve"],
      bound_route_hash: TON_DESTINATION_BINDING_HASH,
      bound_proof_hash: hex32(0x90),
    },
  });
  const result = runTonCli(routeManifestArgs(paths, proofArtifactHash));

  assert.notEqual(result.status, 0);
  assert.match(
    result.stderr,
    /source_browser_prover expected_exports must not contain duplicates/u,
  );
  await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
});

test("TON route manifest rejects credentialed browser module URLs", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root, {
    destinationBrowserProver: {
      module_url: "https://operator:secret@provers.sora.org/ton-destination.js",
      module_hash: hex32(0x61),
      manifest_hash: hex32(0x62),
      expected_exports: ["tonSccpProve"],
      bound_route_hash: TON_DESTINATION_BINDING_HASH,
      bound_proof_hash: hex32(0x90),
    },
  });
  const result = runTonCli(routeManifestArgs(paths, proofArtifactHash));

  assert.notEqual(result.status, 0);
  assert.match(
    result.stderr,
    /destination_browser_prover\.module_url must not include credentials, query, or fragment/u,
  );
  await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
});

test("TON publish-route-manifest refuses submit without runtime private key", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const render = runTonCli(routeManifestArgs(paths, proofArtifactHash));
  assert.equal(render.status, 0, render.stderr);

  const publish = runTonCli(
    [
      "publish-route-manifest",
      "--manifest",
      paths.out,
      "--out",
      paths.publishOut,
      "--submit",
      "true",
      "--authority",
      "route-manifest-manager@taira",
      "--private-key-env",
      "SCCP_TON_TEST_MISSING_PRIVATE_KEY",
    ],
    { env: { SCCP_TON_TEST_MISSING_PRIVATE_KEY: "" } },
  );

  assert.notEqual(publish.status, 0);
  assert.match(
    publish.stderr,
    /SCCP_TON_TEST_MISSING_PRIVATE_KEY must be set at runtime for --submit true/u,
  );
  await assert.rejects(readFile(paths.publishOut, "utf8"), /ENOENT/u);
});
