import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  BSC_TESTNET_NETWORK_ID_HEX,
  ROUTE_MANIFEST_SCHEMA,
  SCCP_BSC_DIAGNOSTIC_VERIFIER_KEY_HASHES,
  SCCP_DOMAIN_BSC,
  SCCP_DOMAIN_SORA,
  bscDestinationBindingHash,
  bscDestinationBindingKey,
  buildBscTairaXorRouteConfigToml,
  buildDeploymentEvidence,
  buildMergedBscTairaXorRouteConfigToml,
  main,
  isKnownDiagnosticBscVerifierKeyHash,
  normalizeBscRpcUrl,
  normalizeVerifierMaterial,
  unsafeSecretReason,
  validateBscReadbackEvidence,
} from "./sccp_bsc_taira_xor_deploy.mjs";

const BSC_BRIDGE_ADDRESS = "0x1111111111111111111111111111111111111111";
const BSC_TOKEN_ADDRESS = "0x2222222222222222222222222222222222222222";
const BSC_SOURCE_BRIDGE_ADDRESS = "0x3333333333333333333333333333333333333333";
const BSC_VERIFIER_ADDRESS = "0x4444444444444444444444444444444444444444";
const HASH_11 = `0x${"11".repeat(32)}`;
const HASH_22 = `0x${"22".repeat(32)}`;
const HASH_33 = `0x${"33".repeat(32)}`;
const DIAGNOSTIC_BSC_VERIFIER_KEY_HASH = [
  ...SCCP_BSC_DIAGNOSTIC_VERIFIER_KEY_HASHES,
][0];
const BURN_RECORD_BYTES = Buffer.from(
  "bsc taira xor burn-record artifact fixture for route-config tests",
  "utf8",
);
const BURN_RECORD_B64 = BURN_RECORD_BYTES.toString("base64");
const BURN_RECORD_SHA256 = `0x${createHash("sha256").update(BURN_RECORD_BYTES).digest("hex")}`;

const addresses = Object.freeze({
  token: BSC_TOKEN_ADDRESS,
  bridge: BSC_BRIDGE_ADDRESS,
  sourceBridge: BSC_SOURCE_BRIDGE_ADDRESS,
  verifier: BSC_VERIFIER_ADDRESS,
});

const bindingHash = () =>
  bscDestinationBindingHash({
    verifierAddress: BSC_VERIFIER_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: HASH_22,
  });

const diagnosticBindingHash = () =>
  bscDestinationBindingHash({
    verifierAddress: BSC_VERIFIER_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: DIAGNOSTIC_BSC_VERIFIER_KEY_HASH,
  });

const diagnosticBindingKey = () =>
  bscDestinationBindingKey({
    verifierAddress: BSC_VERIFIER_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: DIAGNOSTIC_BSC_VERIFIER_KEY_HASH,
  });

const readyReadback = (overrides = {}) => ({
  chainIdHex: "0x61",
  codePresent: {
    token: true,
    bridge: true,
    sourceBridge: true,
    verifier: true,
    ...overrides.codePresent,
  },
  tokenBridgeAddress: BSC_BRIDGE_ADDRESS,
  tokenBridgeLocked: true,
  sourceBridgeOwner: BSC_BRIDGE_ADDRESS,
  bridgeDestinationBindingHash: bindingHash(),
  bridgeVerifierAddress: BSC_VERIFIER_ADDRESS,
  bridgeVerifierCodeHash: HASH_11,
  bridgeVerifierKeyHash: HASH_22,
  bridgeNetworkId: BSC_TESTNET_NETWORK_ID_HEX,
  bridgeSourceDomain: SCCP_DOMAIN_SORA,
  bridgeTargetDomain: SCCP_DOMAIN_BSC,
  ...overrides,
});

const verifierMaterial = (overrides = {}) => ({
  alpha1: [1, 2],
  beta2: [3, 4, 5, 6],
  gamma2: [7, 8, 9, 10],
  delta2: [11, 12, 13, 14],
  ic: Array.from({ length: 20 }, (_, index) => index + 15),
  verifierKeyHash: HASH_22,
  proofFamily: "stark-fri-v1",
  networkId: BSC_TESTNET_NETWORK_ID_HEX,
  sourceDomain: 0,
  targetDomain: 2,
  ...overrides,
});

const routeManifest = (overrides = {}) => {
  const {
    destinationRollout: destinationRolloutOverrides,
    destinationBinding: destinationBindingOverrides,
    tairaXorBurnRecord: burnRecordOverrides,
    settlement: settlementOverrides,
    postDeployLiveEvidence: postDeployOverrides,
    ...topLevelOverrides
  } = overrides;
  const { vkRef: burnVkRefOverrides, ...burnRecordRestOverrides } =
    burnRecordOverrides ?? {};
  const destinationRollout = {
    version: 1,
    destinationNetworkId: BSC_TESTNET_NETWORK_ID_HEX,
    sourceDomain: SCCP_DOMAIN_SORA,
    targetDomain: SCCP_DOMAIN_BSC,
    verifierIdentity: BSC_VERIFIER_ADDRESS,
    verifierBackend: "evm-groth16-bn254-v1",
    proofFamily: "stark-fri-v1",
    verifierCodeHash: HASH_11,
    verifierKeyHash: HASH_22,
    destinationBridgeAddress: BSC_BRIDGE_ADDRESS,
    destinationBindingHash: bindingHash(),
    destinationBindingKey: bscDestinationBindingKey({
      verifierAddress: BSC_VERIFIER_ADDRESS,
      bridgeAddress: BSC_BRIDGE_ADDRESS,
      verifierCodeHash: HASH_11,
      verifierKeyHash: HASH_22,
    }),
    ...destinationRolloutOverrides,
  };
  const destinationBinding = {
    version: 1,
    sourceDomain: SCCP_DOMAIN_SORA,
    targetDomain: SCCP_DOMAIN_BSC,
    networkIdHex: BSC_TESTNET_NETWORK_ID_HEX,
    key: destinationRollout.destinationBindingKey,
    bindingHash: destinationRollout.destinationBindingHash,
    ...destinationBindingOverrides,
  };
  const tairaXorBurnRecord = {
    settlementAssetDefinitionId: "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
    contractArtifactB64: BURN_RECORD_B64,
    artifactSha256: BURN_RECORD_SHA256,
    codeHash: HASH_33,
    vkRef: {
      backend: "halo2_ipa",
      name: "taira_bsc_xor_burn_record_v1",
      ...burnVkRefOverrides,
    },
    gasLimit: 2_000_000,
    ...burnRecordRestOverrides,
  };
  const settlement = {
    submitPath: "/v1/bridge/messages",
    mode: "finalize_inbound",
    routeId: "taira_bsc_xor",
    assetKey: "xor",
    ...settlementOverrides,
  };
  const postDeployLiveEvidence = {
    fullTomlReady: false,
    sourceBridgeConfigHash: `0x${"44".repeat(32)}`,
    sourceEventTransactionId: `0x${"55".repeat(32)}`,
    routeCanaryEvidenceHash: `0x${"66".repeat(32)}`,
    routeCanaryTransactionId: `0x${"77".repeat(32)}`,
    ...postDeployOverrides,
  };
  return {
    schema: ROUTE_MANIFEST_SCHEMA,
    routeId: "taira_bsc_xor",
    assetKey: "xor",
    bscNetwork: "testnet",
    chain: "bsc-testnet",
    chainIdHex: "0x61",
    networkIdHex: BSC_TESTNET_NETWORK_ID_HEX,
    counterpartyDomain: SCCP_DOMAIN_BSC,
    verifierTarget: "EvmContract",
    productionReady: false,
    disabledReason: "BSC test route is not public on TAIRA yet.",
    bscBridgeAddress: BSC_BRIDGE_ADDRESS,
    bscTokenAddress: BSC_TOKEN_ADDRESS,
    sccpBscSourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
    bscVerifierAddress: BSC_VERIFIER_ADDRESS,
    destinationRollout,
    destinationBinding,
    tairaXorBurnRecord,
    settlement,
    postDeployLiveEvidence,
    ...topLevelOverrides,
  };
};

test("BSC deployment binding key and hash are canonical public evidence", () => {
  const key = bscDestinationBindingKey({
    verifierAddress: BSC_VERIFIER_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: HASH_22,
  });

  assert.equal(
    key,
    `evm:0:2:${BSC_TESTNET_NETWORK_ID_HEX.slice(
      2,
    )}:${BSC_VERIFIER_ADDRESS}:${BSC_BRIDGE_ADDRESS}:${HASH_11}:${HASH_22}`,
  );
  assert.match(bindingHash(), /^0x[0-9a-f]{64}$/u);
  assert.notEqual(
    bindingHash(),
    bscDestinationBindingHash({
      verifierAddress: BSC_VERIFIER_ADDRESS,
      bridgeAddress: BSC_TOKEN_ADDRESS,
      verifierCodeHash: HASH_11,
      verifierKeyHash: HASH_22,
    }),
  );
});

test("BSC deployment evidence accepts only matching live readback", () => {
  const evidence = buildDeploymentEvidence({
    tokenAddress: BSC_TOKEN_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    sourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
    verifierAddress: BSC_VERIFIER_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: HASH_22,
    readback: readyReadback(),
  });

  assert.equal(evidence.routeId, "taira_bsc_xor");
  assert.equal(evidence.assetKey, "xor");
  assert.equal(evidence.destinationRollout.destinationBindingHash, bindingHash());
  assert.equal(evidence.bscContractReadback.bridgeVerifierKeyHash, HASH_22);
  assert.doesNotMatch(JSON.stringify(evidence), /private[_-]?key|mnemonic|seed/iu);
});

test("BSC deployment evidence rejects duplicate contract addresses", () => {
  assert.throws(
    () =>
      buildDeploymentEvidence({
        tokenAddress: BSC_BRIDGE_ADDRESS,
        bridgeAddress: BSC_BRIDGE_ADDRESS,
        sourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
        verifierAddress: BSC_VERIFIER_ADDRESS,
        verifierCodeHash: HASH_11,
        verifierKeyHash: HASH_22,
        readback: readyReadback(),
      }),
    /addresses must be distinct/u,
  );
});

test("BSC deployment readback rejects drift and incomplete contracts", () => {
  const cases = [
    [readyReadback({ chainIdHex: "0x38" }), /chain id/u],
    [readyReadback({ codePresent: { token: false } }), /token bytecode/u],
    [readyReadback({ tokenBridgeAddress: BSC_TOKEN_ADDRESS }), /token bridge/u],
    [readyReadback({ tokenBridgeLocked: false }), /must be locked/u],
    [readyReadback({ sourceBridgeOwner: BSC_SOURCE_BRIDGE_ADDRESS }), /source bridge owner/u],
    [readyReadback({ bridgeDestinationBindingHash: HASH_33 }), /destination binding/u],
    [readyReadback({ bridgeVerifierAddress: BSC_BRIDGE_ADDRESS }), /verifier address/u],
    [readyReadback({ bridgeVerifierCodeHash: HASH_33 }), /verifier code hash/u],
    [readyReadback({ bridgeVerifierKeyHash: HASH_33 }), /verifier key hash/u],
    [readyReadback({ bridgeNetworkId: `0x${"38".padStart(64, "0")}` }), /network id/u],
    [readyReadback({ bridgeSourceDomain: 2 }), /domains/u],
    [readyReadback({ bridgeTargetDomain: 1 }), /domains/u],
  ];

  for (const [readback, reason] of cases) {
    assert.throws(
      () =>
        validateBscReadbackEvidence({
          addresses,
          readback,
          bindingHash: bindingHash(),
          verifierCodeHash: HASH_11,
          verifierKeyHash: HASH_22,
        }),
      reason,
    );
  }
});

test("BSC deployment helper rejects unsafe secret-like evidence material", () => {
  assert.equal(unsafeSecretReason({ public: "ok" }), "");
  assert.match(unsafeSecretReason({ nested: { private_key: "0xabc" } }), /private key/u);
  assert.match(
    unsafeSecretReason({
      notes:
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    }),
    /recovery phrases/u,
  );
  assert.match(
    unsafeSecretReason({
      notes: "-----BEGIN PRIVATE KEY-----\nabc\n-----END PRIVATE KEY-----",
    }),
    /private key material/u,
  );
});

test("BSC RPC endpoint normalization is fail-closed", () => {
  assert.equal(
    normalizeBscRpcUrl("https://data-seed-prebsc-1-s1.bnbchain.org:8545/"),
    "https://data-seed-prebsc-1-s1.bnbchain.org:8545",
  );
  assert.equal(
    normalizeBscRpcUrl("http://localhost:8545", { allowLocal: true }),
    "http://localhost:8545",
  );
  for (const endpoint of [
    "http://example.com",
    "https://user:pass@example.com",
    "https://example.com?token=secret",
    "https://example.com/#fragment",
    "not a url",
  ]) {
    assert.throws(() => normalizeBscRpcUrl(endpoint), /BSC RPC URL/u);
  }
});

test("BSC verifier material normalization rejects foreign or malformed inputs", () => {
  const normalized = normalizeVerifierMaterial(verifierMaterial());
  assert.equal(normalized.expectedVerifierKeyHash, HASH_22);
  assert.equal(isKnownDiagnosticBscVerifierKeyHash(HASH_22), false);
  assert.equal(normalized.ic.length, 20);

  assert.throws(
    () => normalizeVerifierMaterial(verifierMaterial({ proofFamily: "groth16-only" })),
    /proofFamily/u,
  );
  assert.throws(
    () => normalizeVerifierMaterial(verifierMaterial({ networkId: `0x${"38".padStart(64, "0")}` })),
    /BSC testnet/u,
  );
  assert.throws(() => normalizeVerifierMaterial(verifierMaterial({ sourceDomain: 2 })), /SORA -> BSC/u);
  assert.throws(() => normalizeVerifierMaterial(verifierMaterial({ targetDomain: 1 })), /SORA -> BSC/u);
  assert.throws(() => normalizeVerifierMaterial(verifierMaterial({ ic: [1, 2] })), /20 uint256/u);
  assert.throws(() => normalizeVerifierMaterial({ ...verifierMaterial(), verifierKeyHash: HASH_22, alpha1: [0] }), /2 uint256/u);
});

test("BSC verifier material reports diagnostic key material before deployment", () => {
  const normalized = normalizeVerifierMaterial(
    verifierMaterial({
      schema: "iroha-sccp-bsc-testnet-diagnostic-verifier-key/v1",
      warning: "Generated diagnostic BSC testnet verifier material.",
      verifierKeyHash: DIAGNOSTIC_BSC_VERIFIER_KEY_HASH,
    }),
  );

  assert.equal(
    isKnownDiagnosticBscVerifierKeyHash(DIAGNOSTIC_BSC_VERIFIER_KEY_HASH),
    true,
  );
  assert.equal(
    normalized.expectedVerifierKeyHash,
    DIAGNOSTIC_BSC_VERIFIER_KEY_HASH,
  );
  assert.match(
    normalized.diagnosticVerifierReasons.join(" "),
    /diagnostic.*known diagnostic BSC verifier key hash/u,
  );
});

test("BSC route-config writes backend-compatible TOML with BSC deployment evidence", () => {
  const toml = buildBscTairaXorRouteConfigToml(routeManifest(), {
    "allow-unready": "true",
  });

  assert.match(toml, /route_id = "taira_bsc_xor"/u);
  assert.match(toml, /asset_key = "xor"/u);
  assert.match(toml, /tron_network = "bsc-testnet"/u);
  assert.match(toml, /chain = "bsc-testnet"/u);
  assert.match(toml, /chain_id_hex = "0x61"/u);
  assert.match(toml, /counterparty_domain = 2/u);
  assert.match(toml, /verifier_target = "EvmContract"/u);
  assert.match(toml, /sccp_allow_unready_transparent_proofs = true/u);
  assert.match(toml, new RegExp(`taira_xor_token_address = "${BSC_TOKEN_ADDRESS}"`, "u"));
  assert.match(toml, new RegExp(`taira_xor_bridge_address = "${BSC_BRIDGE_ADDRESS}"`, "u"));
  assert.match(toml, new RegExp(`source_bridge_address = "${BSC_SOURCE_BRIDGE_ADDRESS}"`, "u"));
  assert.match(
    toml,
    new RegExp(`sccp_bsc_source_bridge_address = "${BSC_SOURCE_BRIDGE_ADDRESS}"`, "u"),
  );
  assert.match(
    toml,
    new RegExp(`bsc_source_bridge_address = "${BSC_SOURCE_BRIDGE_ADDRESS}"`, "u"),
  );
  assert.match(
    toml,
    new RegExp(`sccp_tron_source_bridge_address = "${BSC_SOURCE_BRIDGE_ADDRESS}"`, "u"),
  );
  assert.match(
    toml,
    new RegExp(`destination_verifier_address = "${BSC_VERIFIER_ADDRESS}"`, "u"),
  );
  assert.match(toml, new RegExp(`verifier_address = "${BSC_VERIFIER_ADDRESS}"`, "u"));
  assert.match(
    toml,
    new RegExp(`sccp_bsc_destination_verifier_address = "${BSC_VERIFIER_ADDRESS}"`, "u"),
  );
  assert.match(toml, new RegExp(`bsc_verifier_address = "${BSC_VERIFIER_ADDRESS}"`, "u"));
  assert.match(toml, new RegExp(`evm_verifier_address = "${BSC_VERIFIER_ADDRESS}"`, "u"));
  assert.match(toml, new RegExp(`tron_verifier_address = "${BSC_VERIFIER_ADDRESS}"`, "u"));
  assert.match(toml, new RegExp(`destination_binding_hash = "${bindingHash()}"`, "u"));
  assert.match(toml, new RegExp(`taira_burn_record_artifact_sha256 = "${BURN_RECORD_SHA256}"`, "u"));
  assert.match(toml, /post_deploy_full_toml_ready = false/u);
  assert.doesNotMatch(toml, /private[_-]?key|mnemonic|seed[_-]?phrase/iu);
});

test("BSC route-config refuses non-production manifests unless explicitly allowed", () => {
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(routeManifest(), {
        "allow-unready": "false",
      }),
    /allow-unready/u,
  );
  assert.match(buildBscTairaXorRouteConfigToml(routeManifest()), /allow_unready/u);
});

test("BSC route-config refuses production-ready diagnostic verifier manifests", () => {
  const diagnosticProductionManifest = routeManifest({
    productionReady: true,
    disabledReason: undefined,
    destinationRollout: {
      verifierKeyHash: DIAGNOSTIC_BSC_VERIFIER_KEY_HASH,
      destinationBindingHash: diagnosticBindingHash(),
      destinationBindingKey: diagnosticBindingKey(),
    },
    destinationBinding: {
      bindingHash: diagnosticBindingHash(),
      key: diagnosticBindingKey(),
    },
  });

  assert.throws(
    () => buildBscTairaXorRouteConfigToml(diagnosticProductionManifest),
    /productionReady.*diagnostic BSC verifier material/u,
  );

  const diagnosticDisabledToml = buildBscTairaXorRouteConfigToml(
    {
      ...diagnosticProductionManifest,
      productionReady: false,
    },
    { "allow-unready": "true" },
  );
  assert.match(diagnosticDisabledToml, /production_ready = false/u);
  assert.match(diagnosticDisabledToml, /diagnostic and must be replaced/u);
});

test("BSC route-config can merge into TAIRA config while preserving zk settings", () => {
  const base = [
    "[network]",
    'address = "127.0.0.1:1337"',
    "",
    "[zk]",
    "sccp_allow_unready_transparent_proofs = false",
    "other_setting = true",
    "",
    "[torii]",
    'address = "127.0.0.1:8080"',
    "",
  ].join("\n");
  const merged = buildMergedBscTairaXorRouteConfigToml(base, routeManifest(), {
    "allow-unready": "true",
  });

  assert.match(merged, /\[zk\]\nsccp_allow_unready_transparent_proofs = true/u);
  assert.match(merged, /other_setting = true/u);
  assert.match(merged, /\[\[zk\.sccp_route_manifests\]\]/u);
  assert.match(merged, /\[torii\]/u);
  assert.equal(
    merged.match(/sccp_allow_unready_transparent_proofs\s*=/gu)?.length,
    1,
  );
  assert.throws(
    () => buildMergedBscTairaXorRouteConfigToml("[[zk.sccp_route_manifests]]\n", routeManifest()),
    /already contains/u,
  );
});

test("BSC route-config rejects malformed or foreign route manifests", () => {
  const cases = [
    [{ routeId: "taira_tron_xor" }, /routeId/u],
    [{ assetKey: "dot" }, /assetKey/u],
    [{ chain: "bsc-mainnet" }, /chain/u],
    [{ chainIdHex: "0x38" }, /chainIdHex/u],
    [{ networkIdHex: `0x${"38".padStart(64, "0")}` }, /networkIdHex/u],
    [{ counterpartyDomain: 1 }, /counterpartyDomain/u],
    [{ verifierTarget: "TronContract" }, /verifierTarget/u],
    [{ bscBridgeAddress: BSC_TOKEN_ADDRESS }, /distinct/u],
    [{ destinationRollout: { targetDomain: 1 } }, /SORA -> BSC/u],
    [{ destinationRollout: { verifierBackend: "tron-groth16-bn254-v1" } }, /verifier backend/u],
    [{ destinationRollout: { destinationBindingHash: HASH_33 } }, /binding hash/u],
    [{ tairaXorBurnRecord: { artifactSha256: HASH_33 } }, /artifact sha256/u],
    [{ tairaXorBurnRecord: { settlementAssetDefinitionId: "xor#universal" } }, /Base58|alias/u],
    [{ sourceBridgeAddress: BSC_BRIDGE_ADDRESS }, /source bridge address aliases disagree/u],
    [{ destinationVerifierAddress: BSC_BRIDGE_ADDRESS }, /verifier address aliases disagree/u],
    [{ secret_key: "0xabc" }, /private key|secrets/u],
  ];

  for (const [overrides, reason] of cases) {
    assert.throws(
      () => buildBscTairaXorRouteConfigToml(routeManifest(overrides), { "allow-unready": "true" }),
      reason,
    );
  }
});

test("BSC route-config command writes an operator overlay", async () => {
  const dir = await mkdtemp(join(tmpdir(), "iroha-bsc-route-config-"));
  const manifestPath = join(dir, "manifest.json");
  const out = join(dir, "route.toml");
  await writeFile(manifestPath, `${JSON.stringify(routeManifest(), null, 2)}\n`);

  const result = await main([
    "route-config",
    "--manifest",
    manifestPath,
    "--out",
    out,
    "--allow-unready",
    "true",
  ]);
  assert.equal(result.ok, true);
  assert.equal(result.mode, "overlay");
  assert.equal(result.routeId, "taira_bsc_xor");
  const toml = await readFile(out, "utf8");
  assert.match(toml, /route_id = "taira_bsc_xor"/u);
  assert.match(toml, /source_bridge_address = "0x3333333333333333333333333333333333333333"/u);
  assert.match(toml, /destination_verifier_address = "0x4444444444444444444444444444444444444444"/u);
  assert.match(toml, /tron_verifier_address = "0x4444444444444444444444444444444444444444"/u);
});

test("BSC deploy command refuses to broadcast without explicit testnet confirmation", async () => {
  await assert.rejects(
    () => main(["deploy", "--verifier", "missing-verifier.json"]),
    /broadcast true/u,
  );
  await assert.rejects(
    () =>
      main([
        "deploy",
        "--verifier",
        "missing-verifier.json",
        "--broadcast",
        "true",
        "--confirm-testnet",
        "wrong",
      ]),
    /confirm-testnet/u,
  );
});

test("BSC deploy command rejects missing signer and unsafe local RPC before network use", async () => {
  const envName = "SCCP_BSC_TEST_DEPLOYER_PRIVATE_KEY";
  const previous = process.env[envName];
  try {
    delete process.env[envName];
    await assert.rejects(
      () =>
        main([
          "deploy",
          "--verifier",
          "missing-verifier.json",
          "--broadcast",
          "true",
          "--confirm-testnet",
          "taira_bsc_xor",
          "--private-key-env",
          envName,
        ]),
      new RegExp(envName, "u"),
    );

    process.env[envName] = `0x${"11".repeat(32)}`;
    await assert.rejects(
      () =>
        main([
          "deploy",
          "--verifier",
          "missing-verifier.json",
          "--broadcast",
          "true",
          "--confirm-testnet",
          "taira_bsc_xor",
          "--private-key-env",
          envName,
          "--rpc-url",
          "http://127.0.0.1:8545",
        ]),
      /HTTPS unless localhost is allowed/u,
    );
  } finally {
    if (previous === undefined) {
      delete process.env[envName];
    } else {
      process.env[envName] = previous;
    }
  }
});

test("BSC deployment helper self-test covers public evidence and secret scanning", async () => {
  assert.deepEqual(await main(["self-test"]), { ok: true });
});
