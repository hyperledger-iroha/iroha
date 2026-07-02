import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { AccountAddress } from "../javascript/iroha_js/src/address.js";

const REPO_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const SCRIPT = join(REPO_ROOT, "scripts", "sccp_ton_taira_xor_deploy.mjs");
const TON_DESTINATION_BINDING_HASH =
  "0x8651c1b818973f92050f69e66e8491e9681d23db1cb37393b9ea15c5e7e02799";
const TON_TESTNET_CHAIN_ID_HEX =
  "0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd";
const DEFAULT_TON_FINALIZE_MESSAGE_VALUE_NANO = "100000000";
const TAIRA_XOR_SETTLEMENT_ASSET_DEFINITION_ID = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
const DEFAULT_TAIRA_BURN_RECORD_VK_NAME = "taira_bsc_xor_burn_record_v1";
const DEFAULT_TAIRA_ROUTE_MANIFEST_GAS_LIMIT = 2_000_000;
const malformedBooleanOptionValues = Object.freeze([
  " TRUE",
  "true ",
  "false ",
  "TRUE",
  "False",
  "1",
  "0",
  "yes",
  "on",
  "",
]);
const TAIRA_ROUTE_MANIFEST_AUTHORITY = AccountAddress.fromAccount({
  publicKey: Buffer.from(
    "CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
    "hex",
  ),
}).toI105();

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

function tairaContractFixture(overrides = {}) {
  const { vkRef: vkRefOverrides, ...rest } = overrides;
  return {
    settlementAssetDefinitionId: TAIRA_XOR_SETTLEMENT_ASSET_DEFINITION_ID,
    contractArtifactB64: Buffer.alloc(32, 0x5a).toString("base64"),
    artifactSha256: hex32(0x51),
    codeHash: hex32(0x52),
    vkRef: {
      backend: "halo2/ipa",
      name: DEFAULT_TAIRA_BURN_RECORD_VK_NAME,
      ...(vkRefOverrides ?? {}),
    },
    gasLimit: DEFAULT_TAIRA_ROUTE_MANIFEST_GAS_LIMIT,
    ...rest,
  };
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
  const tairaContract = overrides.tairaContract ?? tairaContractFixture();
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
    ...(extra.vkName === undefined ? [] : ["--vk-name", extra.vkName]),
    "--out",
    extra.out ?? paths.out,
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
  assert.equal(
    envelope.manifest.taira_burn_record_vk_name,
    DEFAULT_TAIRA_BURN_RECORD_VK_NAME,
  );
  assert.equal(envelope.manifest.production_ready, true);
  assert.equal(envelope.manifest.disabled_reason, undefined);
  const rollout = envelope.manifest.destination_rollout;
  assert.equal(rollout.version, 1);
  assert.equal(rollout.destination_network_id, TON_TESTNET_CHAIN_ID_HEX);
  assert.equal(rollout.source_domain, 0);
  assert.equal(rollout.target_domain, 4);
  assert.equal(rollout.verifier_identity, tonRaw(0x44));
  assert.equal(rollout.verifier_backend, "ton-contract-v1");
  assert.equal(rollout.proof_family, "stark-fri-v1");
  assert.equal(rollout.verifier_code_hash, hex32(0x81));
  assert.equal(rollout.verifier_key_hash, hex32(0x82));
  assert.equal(rollout.proof_artifact_hash, proofArtifactHash);
  assert.equal(rollout.proving_key_hash, hex32(0x83));
  assert.equal(rollout.destination_bridge_address, tonRaw(0x22));
  assert.equal(rollout.source_bridge_address, tonRaw(0x33));
  assert.equal(rollout.destination_binding_hash, TON_DESTINATION_BINDING_HASH);
  assert.equal(rollout.destination_binding_key, "sccp:0:4:ton:ton-contract-v1:3");
  assert.equal(
    rollout.finalize_message_value_nano,
    DEFAULT_TON_FINALIZE_MESSAGE_VALUE_NANO,
  );
  assert.equal(
    rollout.ton_finalize_message_value_nano,
    DEFAULT_TON_FINALIZE_MESSAGE_VALUE_NANO,
  );
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
  assert.equal(artifact.gasAssetId, TAIRA_XOR_SETTLEMENT_ASSET_DEFINITION_ID);
  assert.equal(artifact.gasLimit, DEFAULT_TAIRA_ROUTE_MANIFEST_GAS_LIMIT);
  assert.equal(artifact.submission, undefined);
  assert.equal(
    artifact.instruction.UpsertSccpRouteManifest.manifest.route_id,
    "taira_ton_xor",
  );
  assert.equal(
    artifact.instruction.UpsertSccpRouteManifest.manifest
      .sccp_tron_source_bridge_address,
    tonRaw(0x33),
  );
  assert.equal(
    artifact.instruction.UpsertSccpRouteManifest.manifest.tron_verifier_address,
    tonRaw(0x44),
  );
  assert.equal(
    artifact.instruction.UpsertSccpRouteManifest.manifest.source_bridge_address,
    undefined,
  );
  assert.equal(
    artifact.instruction.UpsertSccpRouteManifest.manifest
      .destination_verifier_address,
    undefined,
  );
});

test("TON publish-route-manifest rejects submit-only options without submit", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const render = runTonCli(routeManifestArgs(paths, proofArtifactHash));
  assert.equal(render.status, 0, render.stderr);
  const cases = [
    {
      args: ["--authority", TAIRA_ROUTE_MANIFEST_AUTHORITY],
      expected: /--authority requires --submit true/u,
    },
    {
      args: ["--private-key-env", "SCCP_TON_TEST_PRIVATE_KEY"],
      expected: /--private-key-env requires --submit true/u,
    },
    {
      args: ["--torii-url", "https://taira.sora.org"],
      expected: /--torii-url requires --submit true/u,
    },
    {
      args: ["--chain-id", "809574f5-fee7-5e69-bfcf-52451e42d50f"],
      expected: /--chain-id requires --submit true/u,
    },
    {
      args: ["--wait-for-commit", "false"],
      expected: /--wait-for-commit requires --submit true/u,
    },
    {
      args: ["--commit-timeout-ms", "120000"],
      expected: /--commit-timeout-ms requires --submit true/u,
    },
  ];

  for (const [index, testCase] of cases.entries()) {
    const outPath = join(root, `submit-only-${index}.json`);
    const sentinel = `sentinel:submit-only:${index}\n`;
    const submitArgs = index % 2 === 0 ? [] : ["--submit", "false"];
    await writeFile(outPath, sentinel, "utf8");
    const publish = runTonCli([
      "publish-route-manifest",
      "--manifest",
      paths.out,
      "--out",
      outPath,
      ...submitArgs,
      ...testCase.args,
    ]);

    assert.notEqual(publish.status, 0);
    assert.match(publish.stderr, testCase.expected);
    assert.equal(await readFile(outPath, "utf8"), sentinel);
  }
});

test("TON route manifest accepts an explicit TAIRA burn-record VK name", async () => {
  const root = await fixtureRoot();
  const vkName = "taira_ton_xor_burn_record_v2";
  const { paths, proofArtifactHash } = await writeFixtureFiles(root, {
    tairaContract: tairaContractFixture({ vkRef: { name: vkName } }),
  });
  const result = runTonCli(routeManifestArgs(paths, proofArtifactHash, { vkName }));

  assert.equal(result.status, 0, result.stderr);
  const envelope = JSON.parse(await readFile(paths.out, "utf8"));
  assert.equal(envelope.manifest.taira_burn_record_vk_name, vkName);
});

test("TON route manifest rejects mismatched TAIRA burn-record VK names", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const expectedVkName = "taira_ton_xor_burn_record_v2";
  const result = runTonCli(
    routeManifestArgs(paths, proofArtifactHash, { vkName: expectedVkName }),
  );

  assert.notEqual(result.status, 0);
  assert.match(
    result.stderr,
    new RegExp(`TAIRA burn-record VK name must be ${expectedVkName}`, "u"),
  );
  await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
});

test("TON route manifest rejects malformed TAIRA burn-record VK names", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const cases = [
    ` ${DEFAULT_TAIRA_BURN_RECORD_VK_NAME}`,
    `${DEFAULT_TAIRA_BURN_RECORD_VK_NAME}\n`,
    "taira/bsc/xor/burn-record",
  ];

  for (const vkName of cases) {
    const result = runTonCli(
      routeManifestArgs(paths, proofArtifactHash, { vkName }),
    );

    assert.notEqual(result.status, 0);
    assert.match(
      result.stderr,
      /--vk-name must be 1-128 verifier-key identifier characters/u,
    );
    await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
  }
});

test("TON publish-route-manifest records custom gas review settings", async () => {
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
    "--gas-asset-id",
    TAIRA_XOR_SETTLEMENT_ASSET_DEFINITION_ID,
    "--gas-limit",
    "123456",
  ]);
  assert.equal(publish.status, 0, publish.stderr);

  const artifact = JSON.parse(await readFile(paths.publishOut, "utf8"));
  assert.equal(artifact.gasAssetId, TAIRA_XOR_SETTLEMENT_ASSET_DEFINITION_ID);
  assert.equal(artifact.gasLimit, 123456);
});

test("TON publish-route-manifest rejects invalid gas before writing artifacts", async () => {
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
    "--gas-limit",
    "0",
  ]);

  assert.notEqual(publish.status, 0);
  assert.match(publish.stderr, /--gas-limit must be a positive integer/u);
  await assert.rejects(readFile(paths.publishOut, "utf8"), /ENOENT/u);
});

test("TON publish-route-manifest rejects gas metadata before manifest read", async () => {
  const root = await fixtureRoot();
  const missingManifest = join(root, "missing-route.manifest.json");
  const cases = [
    {
      args: ["--gas-limit", "0"],
      expected: /--gas-limit must be a positive integer/u,
      submitArgs: [],
    },
    {
      args: ["--gas-asset-id", ` ${TAIRA_XOR_SETTLEMENT_ASSET_DEFINITION_ID}`],
      expected: /--gas-asset-id must be canonical text/u,
      submitArgs: ["--submit", "true"],
    },
  ];

  for (const [index, testCase] of cases.entries()) {
    const outPath = join(root, `gas-before-manifest-${index}.json`);
    const sentinel = `sentinel:gas-before-manifest:${index}\n`;
    await writeFile(outPath, sentinel, "utf8");
    const publish = runTonCli([
      "publish-route-manifest",
      "--manifest",
      missingManifest,
      "--out",
      outPath,
      ...testCase.submitArgs,
      ...testCase.args,
    ]);

    assert.notEqual(publish.status, 0);
    assert.match(publish.stderr, testCase.expected);
    assert.doesNotMatch(publish.stderr, /missing-route\.manifest\.json/u);
    assert.equal(await readFile(outPath, "utf8"), sentinel);
  }
});

test("TON publish-route-manifest submits gas metadata to transaction builder", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const render = runTonCli(routeManifestArgs(paths, proofArtifactHash));
  assert.equal(render.status, 0, render.stderr);

  const capturePath = join(root, "transaction-capture.json");
  const preloadPath = join(root, "mock-submit.mjs");
  await writeFile(
    preloadPath,
    `
import { writeFileSync } from "node:fs";

function jsonResponse(status, payload) {
  return {
    status,
    statusText: String(status),
    json: async () => payload,
    text: async () => JSON.stringify(payload),
    arrayBuffer: async () => new TextEncoder().encode(JSON.stringify(payload)).buffer,
    headers: { get: (name) => name.toLowerCase() === "content-type" ? "application/json" : null },
  };
}

globalThis.__IROHA_NATIVE_BINDING__ = {
  buildTransaction(chainId, authority, instructions, metadataPayload, creationTimeMs, ttlMs, nonce, secret) {
    writeFileSync(
      process.env.SCCP_TON_TEST_CAPTURE_PATH,
      JSON.stringify({
        chainId,
        authority,
        instructions: instructions.map((entry) => JSON.parse(entry)),
        metadata: JSON.parse(metadataPayload),
        creationTimeMs,
        ttlMs,
        nonce,
        secretLength: secret.length,
      }, null, 2),
    );
    return {
      signedTransaction: new Uint8Array([1, 2, 3, 4]),
      hash: new Uint8Array(Array.from({ length: 32 }, (_, index) => index + 1)),
    };
  },
};

globalThis.fetch = async (url) => {
  const path = new URL(url).pathname;
  if (path === "/v1/node/capabilities") {
    return jsonResponse(200, {
      abi_version: 1,
      data_model_version: 1,
      crypto: {
        sm: {
          enabled: false,
          allowed_signing: [],
          acceleration: { policy: "portable" },
        },
        curves: {
          registry_version: 1,
          allowed_curve_ids: [],
          allowed_curve_bitmap: [],
        },
      },
    });
  }
  if (path === "/v1/pipeline/transactions") {
    return jsonResponse(202, { accepted: true });
  }
  return jsonResponse(404, { error: "unexpected path", path });
};
`,
    "utf8",
  );

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
      TAIRA_ROUTE_MANIFEST_AUTHORITY,
      "--private-key-env",
      "SCCP_TON_TEST_PRIVATE_KEY",
      "--wait-for-commit",
      "false",
      "--gas-asset-id",
      TAIRA_XOR_SETTLEMENT_ASSET_DEFINITION_ID,
      "--gas-limit",
      "345678",
    ],
    {
      env: {
        SCCP_TON_TEST_PRIVATE_KEY: "11".repeat(32),
        SCCP_TON_TEST_CAPTURE_PATH: capturePath,
        NODE_OPTIONS: `--import=${preloadPath}`,
      },
    },
  );
  assert.equal(publish.status, 0, publish.stderr);

  const artifact = JSON.parse(await readFile(paths.publishOut, "utf8"));
  assert.equal(artifact.submission.submitted, true);
  assert.equal(artifact.submission.gasAssetId, TAIRA_XOR_SETTLEMENT_ASSET_DEFINITION_ID);
  assert.equal(artifact.submission.gasLimit, 345678);
  const capture = JSON.parse(await readFile(capturePath, "utf8"));
  assert.equal(capture.chainId, "809574f5-fee7-5e69-bfcf-52451e42d50f");
  assert.equal(capture.authority, TAIRA_ROUTE_MANIFEST_AUTHORITY);
  assert.equal(capture.metadata.route_id, "taira_ton_xor");
  assert.equal(capture.metadata.asset_key, "xor");
  assert.equal(capture.metadata.action, "publish_sccp_ton_route_manifest");
  assert.equal(capture.metadata.gas_asset_id, TAIRA_XOR_SETTLEMENT_ASSET_DEFINITION_ID);
  assert.equal(capture.metadata.gas_limit, 345678);
  assert.equal(capture.secretLength, 32);
  assert.equal(
    capture.instructions[0].UpsertSccpRouteManifest.manifest.route_id,
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

test("TON route manifest rejects malformed finalize message values", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  for (const value of ["0", ` ${DEFAULT_TON_FINALIZE_MESSAGE_VALUE_NANO}`, `${DEFAULT_TON_FINALIZE_MESSAGE_VALUE_NANO}\n`]) {
    const result = runTonCli(
      routeManifestArgs(paths, proofArtifactHash, {
        tonFinalizeMessageValueNano: value,
      }),
    );

    assert.notEqual(result.status, 0);
    assert.match(
      result.stderr,
      /--ton-finalize-message-value-nano must be a positive integer decimal string/u,
    );
    await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
  }
});

test("TON route manifest rejects duplicate CLI options before writing", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const result = runTonCli([
    ...routeManifestArgs(paths, proofArtifactHash),
    "--token",
    tonRaw(0x55),
  ]);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Option must be specified at most once/u);
  await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
});

test("TON route manifest redacts unexpected positional arguments", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const secretArgument = "secret-token-ton-route-cli-private-key";
  const result = runTonCli([
    ...routeManifestArgs(paths, proofArtifactHash),
    secretArgument,
  ]);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Unexpected positional argument/u);
  assert.doesNotMatch(result.stderr, new RegExp(secretArgument, "u"));
  await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
});

test("TON route manifest redacts duplicate unknown option names", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const secretOptionName = "secret-token-ton-route-duplicate-option";
  const result = runTonCli([
    ...routeManifestArgs(paths, proofArtifactHash),
    `--${secretOptionName}`,
    "first",
    `--${secretOptionName}`,
    "second",
  ]);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Option must be specified at most once/u);
  assert.doesNotMatch(result.stderr, new RegExp(secretOptionName, "u"));
  await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
});

test("TON route manifest rejects missing option values before writing", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const result = runTonCli([
    ...routeManifestArgs(paths, proofArtifactHash),
    "--vk-name",
  ]);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Option requires a value/u);
  await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
});

test("TON route manifest rejects empty output paths before writing", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const result = runTonCli(
    routeManifestArgs(paths, proofArtifactHash, {
      out: "",
    }),
  );

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /--out must be a non-empty path/u);
  await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
});

test("TON route manifest rejects malformed input paths before reading", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const cases = [
    {
      option: "--taira-contract",
      value: ` ${paths.tairaContract}`,
      expected: /--taira-contract must be a non-empty path/u,
    },
    {
      option: "--offline-full-toml-evidence",
      value: `${paths.offlineFullTomlEvidence}\n`,
      expected: /--offline-full-toml-evidence must be a non-empty path/u,
    },
  ];

  for (const testCase of cases) {
    const args = routeManifestArgs(paths, proofArtifactHash);
    args[args.indexOf(testCase.option) + 1] = testCase.value;
    const result = runTonCli(args);

    assert.notEqual(result.status, 0);
    assert.match(result.stderr, testCase.expected);
    await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
  }
});

test("TON route manifest rejects unsafe public explorer URLs before writing", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const secretUrl = "https://operator:secret-token-ton-explorer-url@tonscan.org";
  const cases = [
    {
      args: ["--post-deploy-source-event-explorer-url", secretUrl],
      expected:
        /--post-deploy-source-event-explorer-url must not include credentials, query, or fragment/u,
      redacted: secretUrl,
    },
    {
      args: [
        "--post-deploy-route-canary-explorer-url",
        "https://tonscan.org/tx/abc?token=secret-token-ton-explorer-url",
      ],
      expected:
        /--post-deploy-route-canary-explorer-url must not include credentials, query, or fragment/u,
      redacted: "secret-token-ton-explorer-url",
    },
    {
      args: ["--post-deploy-route-canary-explorer-url", "http://tonscan.org/tx/abc"],
      expected: /--post-deploy-route-canary-explorer-url must be a public HTTPS URL/u,
      redacted: null,
    },
    {
      args: ["--post-deploy-source-event-explorer-url", "https://localhost/tx/abc"],
      expected: /--post-deploy-source-event-explorer-url must use a public DNS host/u,
      redacted: null,
    },
    {
      args: ["--post-deploy-source-event-explorer-url", "https://tonscan/tx/abc"],
      expected: /--post-deploy-source-event-explorer-url must use a public DNS host/u,
      redacted: null,
    },
    {
      args: ["--post-deploy-route-canary-explorer-url", "https://127.0.0.1/tx/abc"],
      expected: /--post-deploy-route-canary-explorer-url must use a public DNS host/u,
      redacted: null,
    },
  ];

  for (const testCase of cases) {
    const result = runTonCli([
      ...routeManifestArgs(paths, proofArtifactHash),
      ...testCase.args,
    ]);

    assert.notEqual(result.status, 0);
    assert.match(result.stderr, testCase.expected);
    if (testCase.redacted) {
      assert.doesNotMatch(result.stderr, new RegExp(testCase.redacted, "u"));
    }
    await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
  }
});

test("TON route manifest rejects unknown options without echoing names", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const secretOptionName = "secret-token-ton-route-unknown-option";
  const result = runTonCli([
    ...routeManifestArgs(paths, proofArtifactHash),
    `--${secretOptionName}`,
    "ignored",
  ]);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Unknown option for route-manifest/u);
  assert.doesNotMatch(result.stderr, new RegExp(secretOptionName, "u"));
  await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
});

test("TON CLI rejects unknown commands without echoing names", async () => {
  const secretCommandName = "secret-token-ton-route-unknown-command";
  const result = runTonCli([secretCommandName]);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Unknown command/u);
  assert.doesNotMatch(result.stderr, new RegExp(secretCommandName, "u"));
});

test("TON route manifest rejects unknown options even with help", async () => {
  const secretOptionName = "secret-token-ton-route-help-option";
  const result = runTonCli([
    "route-manifest",
    "--help",
    `--${secretOptionName}`,
    "ignored",
  ]);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Unknown option for route-manifest/u);
  assert.doesNotMatch(result.stderr, /Usage:/u);
  assert.doesNotMatch(result.stderr, new RegExp(secretOptionName, "u"));
});

test("TON CLI rejects valued help options without echoing values", async () => {
  const root = await fixtureRoot();
  const missingManifest = join(root, "missing-route.manifest.json");
  const outPath = join(root, "valued-help-publish.json");
  const sentinel = "sentinel:valued-help-publish\n";
  const secretHelpValue = "secret-token-ton-help-value";
  await writeFile(outPath, sentinel, "utf8");

  const route = runTonCli([
    "route-manifest",
    `--help=${secretHelpValue}`,
  ]);
  assert.notEqual(route.status, 0);
  assert.match(route.stderr, /Help option must not have a value/u);
  assert.doesNotMatch(route.stderr, /Usage:/u);
  assert.doesNotMatch(route.stderr, new RegExp(secretHelpValue, "u"));

  const publish = runTonCli([
    "publish-route-manifest",
    "--manifest",
    missingManifest,
    "--out",
    outPath,
    `--help=${secretHelpValue}`,
  ]);
  assert.notEqual(publish.status, 0);
  assert.match(publish.stderr, /Help option must not have a value/u);
  assert.doesNotMatch(publish.stderr, /Usage:/u);
  assert.doesNotMatch(publish.stderr, new RegExp(secretHelpValue, "u"));
  assert.doesNotMatch(publish.stderr, /missing-route\.manifest\.json/u);
  assert.equal(await readFile(outPath, "utf8"), sentinel);
});

test("TON publish-route-manifest rejects unknown options before manifest read", async () => {
  const root = await fixtureRoot();
  const missingManifest = join(root, "missing-route.manifest.json");
  const outPath = join(root, "publish-unknown-option.json");
  const sentinel = "sentinel:publish-unknown-option\n";
  const secretOptionName = "secret-token-ton-publish-unknown-option";
  await writeFile(outPath, sentinel, "utf8");
  const publish = runTonCli([
    "publish-route-manifest",
    "--manifest",
    missingManifest,
    "--out",
    outPath,
    "--submit",
    "true",
    `--${secretOptionName}`,
    "ignored",
  ]);

  assert.notEqual(publish.status, 0);
  assert.match(publish.stderr, /Unknown option for publish-route-manifest/u);
  assert.doesNotMatch(publish.stderr, new RegExp(secretOptionName, "u"));
  assert.doesNotMatch(publish.stderr, /missing-route\.manifest\.json/u);
  assert.equal(await readFile(outPath, "utf8"), sentinel);
});

test("TON publish-route-manifest rejects missing option values before manifest read", async () => {
  const root = await fixtureRoot();
  const missingManifest = join(root, "missing-route.manifest.json");
  const outPath = join(root, "publish-missing-option-value.json");
  const sentinel = "sentinel:publish-missing-option-value\n";
  await writeFile(outPath, sentinel, "utf8");
  const publish = runTonCli([
    "publish-route-manifest",
    "--manifest",
    missingManifest,
    "--out",
    outPath,
    "--submit",
  ]);

  assert.notEqual(publish.status, 0);
  assert.match(publish.stderr, /Option requires a value/u);
  assert.doesNotMatch(publish.stderr, /missing-route\.manifest\.json/u);
  assert.equal(await readFile(outPath, "utf8"), sentinel);
});

test("TON publish-route-manifest rejects empty path options before manifest read", async () => {
  const root = await fixtureRoot();
  const outPath = join(root, "publish-empty-path.json");
  const sentinel = "sentinel:publish-empty-path\n";
  await writeFile(outPath, sentinel, "utf8");

  const emptyManifest = runTonCli([
    "publish-route-manifest",
    "--manifest=",
    "--out",
    outPath,
  ]);
  assert.notEqual(emptyManifest.status, 0);
  assert.match(emptyManifest.stderr, /--manifest must be a non-empty path/u);
  assert.equal(await readFile(outPath, "utf8"), sentinel);

  const paddedOut = runTonCli([
    "publish-route-manifest",
    "--manifest",
    join(root, "missing-route.manifest.json"),
    "--out",
    ` ${outPath}`,
  ]);
  assert.notEqual(paddedOut.status, 0);
  assert.match(paddedOut.stderr, /--out must be a non-empty path/u);
  assert.doesNotMatch(paddedOut.stderr, /missing-route\.manifest\.json/u);
  assert.equal(await readFile(outPath, "utf8"), sentinel);
});

test("TON route manifest rejects output path collisions with inputs", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const originalEvidence = await readFile(paths.deploymentEvidence, "utf8");
  const result = runTonCli(
    routeManifestArgs(paths, proofArtifactHash, {
      out: paths.deploymentEvidence,
    }),
  );

  assert.notEqual(result.status, 0);
  assert.match(
    result.stderr,
    /--out must not be the same path as --deployment-evidence/u,
  );
  assert.equal(
    await readFile(paths.deploymentEvidence, "utf8"),
    originalEvidence,
  );
});

test("TON publish-route-manifest rejects output path collisions with manifest", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const render = runTonCli(routeManifestArgs(paths, proofArtifactHash));
  assert.equal(render.status, 0, render.stderr);
  const originalManifest = await readFile(paths.out, "utf8");

  const publish = runTonCli([
    "publish-route-manifest",
    "--manifest",
    paths.out,
    "--out",
    paths.out,
  ]);

  assert.notEqual(publish.status, 0);
  assert.match(publish.stderr, /--out must not be the same path as --manifest/u);
  assert.equal(await readFile(paths.out, "utf8"), originalManifest);
});

test("TON publish-route-manifest rejects unsafe top-level explorer metadata", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const render = runTonCli(routeManifestArgs(paths, proofArtifactHash));
  assert.equal(render.status, 0, render.stderr);
  const envelope = JSON.parse(await readFile(paths.out, "utf8"));
  const cases = [
    {
      patch: { explorer_url: "https://localhost/tx/abc" },
      expected: /TON route manifest explorer_url must use a public DNS host/u,
    },
    {
      patch: { explorer_url: "http://testnet.tonscan.org" },
      expected: /TON route manifest explorer_url must be a public HTTPS URL/u,
    },
    {
      patch: { explorer_host: "evil.example" },
      expected: /TON route manifest explorer_host must match explorer_url host/u,
    },
    {
      patch: {
        taira_burn_record_vk_name: ` ${DEFAULT_TAIRA_BURN_RECORD_VK_NAME}`,
      },
      expected:
        /TON route manifest TAIRA burn-record VK name must be 1-128 verifier-key identifier characters/u,
    },
  ];

  for (const [index, testCase] of cases.entries()) {
    const manifestPath = join(root, `unsafe-explorer-${index}.json`);
    const outPath = join(root, `unsafe-explorer-${index}.out.json`);
    const sentinel = `sentinel:unsafe-explorer:${index}\n`;
    await writeJson(manifestPath, {
      ...envelope,
      manifest: {
        ...envelope.manifest,
        ...testCase.patch,
      },
    });
    await writeFile(outPath, sentinel, "utf8");

    const publish = runTonCli([
      "publish-route-manifest",
      "--manifest",
      manifestPath,
      "--out",
      outPath,
    ]);

    assert.notEqual(publish.status, 0);
    assert.match(publish.stderr, testCase.expected);
    assert.equal(await readFile(outPath, "utf8"), sentinel);
  }
});

test("TON publish-route-manifest rejects malformed submit booleans before writing", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const render = runTonCli(routeManifestArgs(paths, proofArtifactHash));
  assert.equal(render.status, 0, render.stderr);

  for (const [index, value] of malformedBooleanOptionValues.entries()) {
    const outPath = join(root, `submit-boolean-${index}.json`);
    const sentinel = `sentinel:submit:${index}\n`;
    await writeFile(outPath, sentinel, "utf8");
    const publish = runTonCli([
      "publish-route-manifest",
      "--manifest",
      paths.out,
      "--out",
      outPath,
      "--submit",
      value,
    ]);

    assert.notEqual(publish.status, 0);
    assert.match(publish.stderr, /--submit must be true or false/u);
    assert.equal(await readFile(outPath, "utf8"), sentinel);
  }
});

test("TON publish-route-manifest rejects malformed wait booleans before writing", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const render = runTonCli(routeManifestArgs(paths, proofArtifactHash));
  assert.equal(render.status, 0, render.stderr);

  for (const [index, value] of malformedBooleanOptionValues.entries()) {
    const outPath = join(root, `wait-boolean-${index}.json`);
    const sentinel = `sentinel:wait:${index}\n`;
    await writeFile(outPath, sentinel, "utf8");
    const publish = runTonCli(
      [
        "publish-route-manifest",
        "--manifest",
        paths.out,
        "--out",
        outPath,
        "--submit",
        "true",
        "--authority",
        TAIRA_ROUTE_MANIFEST_AUTHORITY,
        "--private-key-env",
        "SCCP_TON_TEST_PRIVATE_KEY",
        "--wait-for-commit",
        value,
      ],
      {
        env: {
          SCCP_TON_TEST_PRIVATE_KEY: "11".repeat(32),
        },
      },
    );

    assert.notEqual(publish.status, 0);
    assert.match(publish.stderr, /--wait-for-commit must be true or false/u);
    assert.equal(await readFile(outPath, "utf8"), sentinel);
  }
});

test("TON publish-route-manifest rejects unsafe private-key env names before writing", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const render = runTonCli(routeManifestArgs(paths, proofArtifactHash));
  assert.equal(render.status, 0, render.stderr);
  const secretLikeEnvName = "SCCP_TON_TEST_PRIVATE_KEY=secret-token-ton-route";
  const badEnvNames = [
    " SCCP_TON_TEST_PRIVATE_KEY",
    "SCCP_TON_TEST_PRIVATE_KEY ",
    "sccp_ton_test_private_key",
    "SCCP-TON-TEST-PRIVATE-KEY",
    secretLikeEnvName,
    "SCCP_TON_TEST\nPRIVATE_KEY",
    "1SCCP_TON_TEST_PRIVATE_KEY",
    "",
  ];

  for (const [index, badEnvName] of badEnvNames.entries()) {
    const outPath = join(root, `private-key-env-${index}.json`);
    const sentinel = `sentinel:private-key-env:${index}\n`;
    await writeFile(outPath, sentinel, "utf8");
    const publish = runTonCli([
      "publish-route-manifest",
      "--manifest",
      paths.out,
      "--out",
      outPath,
      "--submit",
      "true",
      "--authority",
      TAIRA_ROUTE_MANIFEST_AUTHORITY,
      "--private-key-env",
      badEnvName,
      "--wait-for-commit",
      "false",
    ]);

    assert.notEqual(publish.status, 0);
    assert.match(
      publish.stderr,
      /--private-key-env must be an uppercase environment variable name/u,
    );
    assert.doesNotMatch(publish.stderr, new RegExp(secretLikeEnvName, "u"));
    assert.equal(await readFile(outPath, "utf8"), sentinel);
  }
});

test("TON publish-route-manifest rejects unsafe Torii URLs before writing", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const render = runTonCli(routeManifestArgs(paths, proofArtifactHash));
  assert.equal(render.status, 0, render.stderr);
  const credentialedUrl =
    "https://operator:secret-token-ton-torii-url@taira.sora.org";
  const cases = [
    {
      value: "http://taira.sora.org",
      expected: /--torii-url must use HTTPS unless it is loopback HTTP/u,
    },
    {
      value: credentialedUrl,
      expected: /--torii-url must not include credentials, query, or fragment/u,
    },
    {
      value: "https://taira.sora.org?private_key=secret-token-ton-torii-url",
      expected: /--torii-url must not include credentials, query, or fragment/u,
    },
    {
      value: "https://taira.sora.org#secret-token-ton-torii-url",
      expected: /--torii-url must not include credentials, query, or fragment/u,
    },
    {
      value: "ftp://taira.sora.org",
      expected: /--torii-url must use HTTPS unless it is loopback HTTP/u,
    },
    {
      value: " https://taira.sora.org",
      expected: /--torii-url must be a valid HTTP\(S\) URL/u,
    },
    {
      value: "https://taira.sora.org\n",
      expected: /--torii-url must be a valid HTTP\(S\) URL/u,
    },
    {
      value: "https://localhost",
      expected: /--torii-url HTTPS host must use public DNS/u,
    },
    {
      value: "https://127.0.0.1",
      expected: /--torii-url HTTPS host must use public DNS/u,
    },
    {
      value: "https://taira",
      expected: /--torii-url HTTPS host must use public DNS/u,
    },
    {
      value: "https://taira.local",
      expected: /--torii-url HTTPS host must use public DNS/u,
    },
    {
      value: "https://bad_host.sora.org",
      expected: /--torii-url HTTPS host must use public DNS/u,
    },
    {
      value: "not a url",
      expected: /--torii-url must be a valid HTTP\(S\) URL/u,
    },
  ];

  for (const [index, testCase] of cases.entries()) {
    const outPath = join(root, `torii-url-${index}.json`);
    const sentinel = `sentinel:torii-url:${index}\n`;
    await writeFile(outPath, sentinel, "utf8");
    const publish = runTonCli(
      [
        "publish-route-manifest",
        "--manifest",
        paths.out,
        "--out",
        outPath,
        "--submit",
        "true",
        "--authority",
        TAIRA_ROUTE_MANIFEST_AUTHORITY,
        "--private-key-env",
        "SCCP_TON_TEST_PRIVATE_KEY",
        "--wait-for-commit",
        "false",
        "--torii-url",
        testCase.value,
      ],
      {
        env: {
          SCCP_TON_TEST_PRIVATE_KEY: "11".repeat(32),
        },
      },
    );

    assert.notEqual(publish.status, 0);
    assert.match(publish.stderr, testCase.expected);
    assert.doesNotMatch(publish.stderr, new RegExp(credentialedUrl, "u"));
    assert.equal(await readFile(outPath, "utf8"), sentinel);
  }
});

test("TON publish-route-manifest rejects unsafe authorities before secret lookup", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const render = runTonCli(routeManifestArgs(paths, proofArtifactHash));
  assert.equal(render.status, 0, render.stderr);
  const secretLikeAuthority = "secret-token-ton-authority";
  const badAuthorities = [
    ` ${TAIRA_ROUTE_MANIFEST_AUTHORITY}`,
    `${TAIRA_ROUTE_MANIFEST_AUTHORITY} `,
    "route-manifest-manager@taira",
    "0x1111111111111111111111111111111111111111111111111111111111111111",
    "uaid:1111111111111111111111111111111111111111111111111111111111111111",
    "not-an-i105-authority",
    `bad\n${TAIRA_ROUTE_MANIFEST_AUTHORITY}`,
    secretLikeAuthority,
  ];

  for (const [index, authority] of badAuthorities.entries()) {
    const outPath = join(root, `authority-${index}.json`);
    const sentinel = `sentinel:authority:${index}\n`;
    await writeFile(outPath, sentinel, "utf8");
    const publish = runTonCli(
      [
        "publish-route-manifest",
        "--manifest",
        paths.out,
        "--out",
        outPath,
        "--submit",
        "true",
        "--authority",
        authority,
        "--private-key-env",
        "SCCP_TON_TEST_PRIVATE_KEY",
        "--wait-for-commit",
        "false",
      ],
      {
        env: {
          SCCP_TON_TEST_PRIVATE_KEY: "11".repeat(32),
        },
      },
    );

    assert.notEqual(publish.status, 0);
    assert.match(publish.stderr, /--authority must be a canonical I105 account id/u);
    assert.doesNotMatch(publish.stderr, new RegExp(secretLikeAuthority, "u"));
    assert.equal(await readFile(outPath, "utf8"), sentinel);
  }
});

test("TON publish-route-manifest rejects submit metadata before private key lookup", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const render = runTonCli(routeManifestArgs(paths, proofArtifactHash));
  assert.equal(render.status, 0, render.stderr);
  const cases = [
    {
      args: ["--chain-id", "809574F5-fee7-5e69-bfcf-52451e42d50f"],
      expected: /--chain-id must be 809574f5-fee7-5e69-bfcf-52451e42d50f for TAIRA/u,
    },
    {
      args: ["--torii-url", "http://taira.sora.org"],
      expected: /--torii-url must use HTTPS unless it is loopback HTTP/u,
    },
    {
      args: ["--commit-timeout-ms", "0"],
      expected: /--commit-timeout-ms must be a positive integer/u,
    },
  ];

  for (const [index, testCase] of cases.entries()) {
    const outPath = join(root, `submit-metadata-before-secret-${index}.json`);
    const sentinel = `sentinel:submit-metadata-before-secret:${index}\n`;
    await writeFile(outPath, sentinel, "utf8");
    const publish = runTonCli(
      [
        "publish-route-manifest",
        "--manifest",
        paths.out,
        "--out",
        outPath,
        "--submit",
        "true",
        "--authority",
        TAIRA_ROUTE_MANIFEST_AUTHORITY,
        "--private-key-env",
        "SCCP_TON_TEST_MISSING_PRIVATE_KEY",
        "--wait-for-commit",
        "false",
        ...testCase.args,
      ],
      {
        env: {
          SCCP_TON_TEST_MISSING_PRIVATE_KEY: "",
        },
      },
    );

    assert.notEqual(publish.status, 0);
    assert.match(publish.stderr, testCase.expected);
    assert.doesNotMatch(publish.stderr, /SCCP_TON_TEST_MISSING_PRIVATE_KEY/u);
    assert.equal(await readFile(outPath, "utf8"), sentinel);
  }
});

test("TON publish-route-manifest rejects malformed nanoTON manifest scalars", async () => {
  const root = await fixtureRoot();
  const { paths, proofArtifactHash } = await writeFixtureFiles(root);
  const render = runTonCli(routeManifestArgs(paths, proofArtifactHash));
  assert.equal(render.status, 0, render.stderr);

  const envelope = JSON.parse(await readFile(paths.out, "utf8"));
  const cases = [
    Number(DEFAULT_TON_FINALIZE_MESSAGE_VALUE_NANO),
    ` ${DEFAULT_TON_FINALIZE_MESSAGE_VALUE_NANO}`,
  ];
  for (const [index, value] of cases.entries()) {
    const manifestPath = join(root, `malformed-nanoton-${index}.json`);
    const outPath = join(root, `malformed-nanoton-${index}.out.json`);
    envelope.manifest.ton_finalize_message_value_nano = value;
    await writeJson(manifestPath, envelope);

    const publish = runTonCli([
      "publish-route-manifest",
      "--manifest",
      manifestPath,
      "--out",
      outPath,
    ]);

    assert.notEqual(publish.status, 0);
    assert.match(
      publish.stderr,
      /TON finalize message value in nanoTON must be a positive integer decimal string/u,
    );
    await assert.rejects(readFile(outPath, "utf8"), /ENOENT/u);
  }
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

test("TON route manifest rejects non-public HTTPS browser module URLs", async () => {
  const browserProver = (seed, moduleUrl) => ({
    module_url: moduleUrl,
    module_hash: hex32(seed),
    manifest_hash: hex32(seed + 1),
    expected_exports: ["tonSccpProve", `tonSccpSelfTest${seed}`],
    bound_route_hash: TON_DESTINATION_BINDING_HASH,
    bound_proof_hash: hex32(0x90),
  });
  const cases = [
    {
      overrides: {
        destinationBrowserProver: browserProver(
          0x61,
          "https://localhost/ton-destination.js",
        ),
      },
      expected:
        /destination_browser_prover\.module_url HTTPS URLs must use a public DNS host/u,
    },
    {
      overrides: {
        sourceBrowserProver: browserProver(
          0x71,
          "https://provers/ton-source.js",
        ),
      },
      expected:
        /source_browser_prover\.module_url HTTPS URLs must use a public DNS host/u,
    },
    {
      overrides: {
        destinationBrowserProver: browserProver(
          0x61,
          "https://127.0.0.1/ton-destination.js",
        ),
      },
      expected:
        /destination_browser_prover\.module_url HTTPS URLs must use a public DNS host/u,
    },
    {
      overrides: {
        sourceBrowserProver: browserProver(
          0x71,
          "https://provers.local/ton-source.js",
        ),
      },
      expected:
        /source_browser_prover\.module_url HTTPS URLs must use a public DNS host/u,
    },
    {
      overrides: {
        destinationBrowserProver: browserProver(
          0x61,
          "https://provers..sora.org/ton-destination.js",
        ),
      },
      expected:
        /destination_browser_prover\.module_url HTTPS URLs must use a public DNS host/u,
    },
  ];

  for (const testCase of cases) {
    const root = await fixtureRoot();
    const { paths, proofArtifactHash } = await writeFixtureFiles(
      root,
      testCase.overrides,
    );
    const result = runTonCli(routeManifestArgs(paths, proofArtifactHash));

    assert.notEqual(result.status, 0);
    assert.match(result.stderr, testCase.expected);
    await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
  }
});

test("TON route manifest rejects traversal and root-relative browser module URLs", async () => {
  const browserProver = (seed, moduleUrl) => ({
    module_url: moduleUrl,
    module_hash: hex32(seed),
    manifest_hash: hex32(seed + 1),
    expected_exports: ["tonSccpProve", `tonSccpSelfTest${seed}`],
    bound_route_hash: TON_DESTINATION_BINDING_HASH,
    bound_proof_hash: hex32(0x90),
  });
  const cases = [
    {
      overrides: {
        destinationBrowserProver: browserProver(0x61, "../ton-destination.js"),
      },
      expected:
        /destination_browser_prover\.module_url must not traverse parent directories/u,
    },
    {
      overrides: {
        sourceBrowserProver: browserProver(0x71, "./../ton-source.js"),
      },
      expected:
        /source_browser_prover\.module_url must not traverse parent directories/u,
    },
    {
      overrides: {
        destinationBrowserProver: browserProver(0x61, "/ton-destination.js"),
      },
      expected:
        /destination_browser_prover\.module_url must be package-relative, HTTPS, or loopback HTTP/u,
    },
    {
      overrides: {
        sourceBrowserProver: browserProver(
          0x71,
          "//provers.sora.org/ton-source.js",
        ),
      },
      expected:
        /source_browser_prover\.module_url must be HTTPS, loopback HTTP, or package-relative/u,
    },
    {
      overrides: {
        destinationBrowserProver: browserProver(0x61, ".//ton-destination.js"),
      },
      expected:
        /destination_browser_prover\.module_url must be package-relative, HTTPS, or loopback HTTP/u,
    },
    {
      overrides: {
        sourceBrowserProver: browserProver(0x71, "./ton-source%2ejs"),
      },
      expected:
        /source_browser_prover\.module_url must be package-relative, HTTPS, or loopback HTTP/u,
    },
  ];

  for (const testCase of cases) {
    const root = await fixtureRoot();
    const { paths, proofArtifactHash } = await writeFixtureFiles(
      root,
      testCase.overrides,
    );
    const result = runTonCli(routeManifestArgs(paths, proofArtifactHash));

    assert.notEqual(result.status, 0);
    assert.match(result.stderr, testCase.expected);
    await assert.rejects(readFile(paths.out, "utf8"), /ENOENT/u);
  }

  for (const moduleUrl of [
    "./ton-destination.js",
    "@sora/sccp-ton-destination-prover/ton-destination.js",
  ]) {
    const root = await fixtureRoot();
    const { paths, proofArtifactHash } = await writeFixtureFiles(root, {
      destinationBrowserProver: browserProver(0x61, moduleUrl),
    });
    const result = runTonCli(routeManifestArgs(paths, proofArtifactHash));

    assert.equal(result.status, 0, result.stderr);
  }
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
      TAIRA_ROUTE_MANIFEST_AUTHORITY,
      "--private-key-env",
      "SCCP_TON_TEST_MISSING_PRIVATE_KEY",
      "--torii-url",
      "http://127.0.0.1:8080",
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
