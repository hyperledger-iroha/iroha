#!/usr/bin/env node
// Unit tests for the TAIRA XOR TRON deployment helper's offline validation
// paths. These tests do not contact TRON and must never broadcast.
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import test from "node:test";
import { secp256k1 } from "../javascript/iroha_js/node_modules/@noble/curves/secp256k1.js";
import { sha256 } from "../javascript/iroha_js/node_modules/@noble/hashes/sha256.js";
import {
  TRON_MAINNET_NETWORK_ID_HEX,
  assertDeploymentFundingReady,
  buildDeploymentDoctorReport,
  buildDeploymentFundingReadiness,
  buildTairaXorRouteManifestDraft,
  bytesToHex,
  compileTairaBurnRecordContract,
  estimateDeploymentFunding,
  hexToBytes,
  normalizeTronAddress,
  normalizeTronBase58Address,
  normalizeTronEndpoint,
  normalizeVerifierConstructorArgs,
  routeHash,
  signTransactionPayload,
  tronDestinationBindingHash,
  tronDestinationBindingKey,
  tronAddressFromPrivateKey,
} from "./sccp_tron_taira_xor_deploy.mjs";

const privateKey = new Uint8Array(32).fill(7);
const deployerAddress = tronAddressFromPrivateKey(privateKey);
const routeAddresses = {
  token: tronAddressFromPrivateKey(new Uint8Array(32).fill(1)),
  bridge: tronAddressFromPrivateKey(new Uint8Array(32).fill(2)),
  sourceBridge: tronAddressFromPrivateKey(new Uint8Array(32).fill(3)),
  verifier: tronAddressFromPrivateKey(new Uint8Array(32).fill(4)),
};
const burnArtifactBytes = Uint8Array.from([1, 2, 3, 4, 5, 6, 1, 8]);
const burnArtifactB64 = Buffer.from(burnArtifactBytes).toString("base64");
const burnArtifactSha256 = bytesToHex(sha256(burnArtifactBytes));

const mockTransaction = (ownerAddress = deployerAddress.base58, rawData = [1, 2, 3, 4]) => {
  const rawBytes = Uint8Array.from(rawData);
  return {
    visible: true,
    txID: bytesToHex(sha256(rawBytes), false),
    raw_data: {
      contract: [
        {
          parameter: {
            value: {
              owner_address: ownerAddress,
            },
          },
          type: "TriggerSmartContract",
        },
      ],
      timestamp: 1,
      expiration: 2,
    },
    raw_data_hex: bytesToHex(rawBytes, false),
  };
};

const verifierMaterial = () => ({
  alpha1: [1, 2],
  beta2: [
    [3, 4],
    [5, 6],
  ],
  gamma2: [7, 8, 9, 10],
  delta2: [11, 12, 13, 14],
  ic: [15, 16, 17, 18],
  verifierKeyHash: routeHash("verifier-key"),
});

const withTempDir = async (fn) => {
  const dir = await mkdtemp(join(tmpdir(), "iroha-sccp-tron-deploy-"));
  try {
    return await fn(dir);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
};

const writeJson = (path, value) =>
  writeFile(path, `${JSON.stringify(value, null, 2)}\n`);

const writeDeployerSecret = async (path) => {
  await writeJson(path, {
    schema: "iroha-sccp-tron-taira-xor-deployer/v1",
    created_at: "2026-06-01T00:00:00.000Z",
    network: "tron-mainnet",
    address_base58: deployerAddress.base58,
    address_hex: deployerAddress.hex,
    private_key_hex: bytesToHex(privateKey, false),
  });
};

const routeDeploymentEvidence = (overrides = {}) => ({
  schema: "iroha-sccp-tron-taira-xor-deployment-evidence/v1",
  created_at: "2026-06-01T00:00:00.000Z",
  route_id: "taira_tron_xor",
  route_id_hash: routeHash("taira_tron_xor"),
  asset_key: "xor",
  asset_key_hash: routeHash("xor"),
  network: "tron-mainnet",
  network_id_hex: TRON_MAINNET_NETWORK_ID_HEX,
  taira_xor_token_address: routeAddresses.token.base58,
  taira_xor_token_address_hex: routeAddresses.token.hex,
  taira_xor_bridge_address: routeAddresses.bridge.base58,
  taira_xor_bridge_address_hex: routeAddresses.bridge.hex,
  sccp_tron_source_bridge_address: routeAddresses.sourceBridge.base58,
  sccp_tron_source_bridge_address_hex: routeAddresses.sourceBridge.hex,
  sccp_tron_destination_verifier_address: routeAddresses.verifier.base58,
  sccp_tron_destination_verifier_address_hex: routeAddresses.verifier.hex,
  ...overrides,
});

const tairaBurnRecordContract = (overrides = {}) => ({
  schema: "iroha-sccp-taira-xor-burn-record-contract/v1",
  created_at: "2026-06-01T00:00:00.000Z",
  route_id: "taira_tron_xor",
  asset_key: "xor",
  source_name: "contracts/taira/sccp/TairaXorSccpBurnRecord.ko",
  compiler_fingerprint: "kotodama_lang/test",
  code_hash: routeHash("taira-burn-record-code").slice(2),
  abi_hash: routeHash("taira-burn-record-abi").slice(2),
  artifact_sha256: burnArtifactSha256,
  artifact_b64: burnArtifactB64,
  manifest: {
    features_bitmap: 1,
  },
  execution: {
    executable: "IvmProved",
    force_zk_mode: true,
    entrypoint: "burn_and_record",
  },
  ...overrides,
});

const writeRouteManifestInputs = async (dir, { evidence = {}, contract = {}, verifier = verifierMaterial() } = {}) => {
  const evidencePath = join(dir, "deployment.evidence.json");
  const contractPath = join(dir, "burn-record.contract.json");
  const verifierPath = join(dir, "verifier.json");
  await writeJson(evidencePath, routeDeploymentEvidence(evidence));
  await writeJson(contractPath, tairaBurnRecordContract(contract));
  await writeJson(verifierPath, verifier);
  return { evidencePath, contractPath, verifierPath };
};

test("normalizes canonical TRON Base58, 21-byte hex, and Solidity address forms", () => {
  const fromBase58 = normalizeTronBase58Address(deployerAddress.base58, "base58");
  assert.equal(fromBase58.hex, deployerAddress.hex);
  assert.equal(fromBase58.solidity, `0x${deployerAddress.hex.slice(4)}`);

  const fromHex = normalizeTronAddress(deployerAddress.hex, "hex");
  assert.equal(fromHex.base58, deployerAddress.base58);
  assert.equal(fromHex.solidity, fromBase58.solidity);

  const fromSolidity = normalizeTronAddress(fromBase58.solidity, "solidity");
  assert.equal(fromSolidity.base58, deployerAddress.base58);
});

test("rejects malformed, non-canonical, checksum-invalid, and zero TRON addresses", () => {
  assert.throws(() => normalizeTronBase58Address(` ${deployerAddress.base58}`, "address"), /canonical/);
  assert.throws(
    () =>
      normalizeTronBase58Address(
        `${deployerAddress.base58.slice(0, -1)}${deployerAddress.base58.endsWith("1") ? "2" : "1"}`,
        "address",
      ),
    /checksum/,
  );
  assert.throws(() => normalizeTronAddress("0x410000000000000000000000000000000000000000", "address"), /non-zero/);
  assert.throws(() => normalizeTronAddress("not-an-address", "address"), /hex|Base58/);
});

test("offline signing binds txID, raw_data owner, and recovered signer", () => {
  const transaction = mockTransaction();
  const signed = signTransactionPayload(transaction, { privateKey, address: deployerAddress });
  assert.equal(signed.metadata.txid, transaction.txID);
  assert.equal(signed.metadata.signature_recovers_to_owner, true);
  assert.equal(signed.metadata.signature_recovered_base58, deployerAddress.base58);
  assert.match(signed.signed.signature[0], /^[0-9a-f]{130}$/u);
});

test("offline signing rejects txid mismatch, wrong owner, duplicate signatures, and malformed owners", () => {
  assert.throws(
    () =>
      signTransactionPayload(
        {
          ...mockTransaction(),
          txID: "00".repeat(32),
        },
        { privateKey, address: deployerAddress },
      ),
    /txID/,
  );

  const otherAddress = tronAddressFromPrivateKey(new Uint8Array(32).fill(8));
  assert.throws(
    () => signTransactionPayload(mockTransaction(otherAddress.base58), { privateKey, address: deployerAddress }),
    /does not match deployer/,
  );

  assert.throws(
    () =>
      signTransactionPayload(
        {
          ...mockTransaction(),
          signature: ["00".repeat(65)],
        },
        { privateKey, address: deployerAddress },
      ),
    /already signed/,
  );

  assert.throws(
    () => signTransactionPayload(mockTransaction("bad-owner"), { privateKey, address: deployerAddress }),
    /hex|Base58/,
  );
});

test("verifier material normalization accepts production lane material", () => {
  const args = normalizeVerifierConstructorArgs(verifierMaterial());
  assert.equal(args.length, 10);
  assert.deepEqual(args[0], ["1", "2"]);
  assert.equal(args[5], routeHash("verifier-key"));
  assert.equal(args[6], "stark-fri-v1");
  assert.equal(args[7], TRON_MAINNET_NETWORK_ID_HEX);
  assert.equal(args[8], 0);
  assert.equal(args[9], 5);
});

test("verifier material normalization rejects stale or adversarial lane material", () => {
  assert.throws(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial(), proofFamily: "debug-proof-family" }),
    /proofFamily/,
  );
  assert.throws(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial(), networkId: routeHash("wrong-network") }),
    /networkId/,
  );
  assert.throws(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial(), sourceDomain: 5, targetDomain: 0 }),
    /SORA -> TRON/,
  );
  assert.throws(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial(), beta2: [1, 2, 3] }),
    /beta2/,
  );
  assert.throws(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial(), verifierKeyHash: "0x00" }),
    /32 bytes/,
  );
});

test("hex helper rejects odd, non-hex, and wrong-length values", () => {
  assert.deepEqual([...hexToBytes("0x0102", "sample", 2)], [1, 2]);
  assert.throws(() => hexToBytes("0x1", "sample"), /even-length/);
  assert.throws(() => hexToBytes("0xzz", "sample"), /hex/);
  assert.throws(() => hexToBytes("0x0102", "sample", 3), /3 bytes/);
});

test("generated deployer private key still derives a valid TRON address shape", () => {
  const generated = tronAddressFromPrivateKey(secp256k1.utils.randomPrivateKey());
  assert.match(generated.base58, /^T[1-9A-HJ-NP-Za-km-z]{33}$/u);
  assert.match(generated.hex, /^0x41[0-9a-f]{40}$/u);
});

test("deployment funding estimate is a conservative mainnet TRX and energy budget", () => {
  const estimate = estimateDeploymentFunding();
  assert.equal(estimate.schema, "iroha-sccp-tron-taira-xor-funding-estimate/v1");
  assert.equal(estimate.route_id, "taira_tron_xor");
  assert.equal(estimate.network, "tron-mainnet");
  assert.equal(estimate.deployment_transaction_count, 4);
  assert.equal(estimate.post_deploy_trigger_transaction_count, 4);
  assert.equal(estimate.deploy_fee_limit_sun, "15000000000");
  assert.equal(estimate.trigger_fee_limit_sun, "1000000000");
  assert.equal(estimate.total_fee_limit_sun, "64000000000");
  assert.equal(estimate.total_fee_limit_trx, "64000");
  assert.equal(estimate.safety_margin_percent, 15);
  assert.equal(estimate.recommended_min_balance_sun, "73600000000");
  assert.equal(estimate.recommended_min_balance_trx, "73600");
  assert.equal(estimate.origin_energy_limit_per_deploy, "10000000");
  assert.equal(estimate.max_origin_energy_limit_total, "40000000");
});

test("deployment funding estimate supports explicit operator fee limits", () => {
  const estimate = estimateDeploymentFunding({
    "fee-limit": "2000000",
    "trigger-fee-limit": "500000",
    "origin-energy-limit": "12345",
    "safety-margin-percent": "10",
  });
  assert.equal(estimate.total_fee_limit_sun, "10000000");
  assert.equal(estimate.total_fee_limit_trx, "10");
  assert.equal(estimate.safety_margin_sun, "1000000");
  assert.equal(estimate.recommended_min_balance_sun, "11000000");
  assert.equal(estimate.recommended_min_balance_trx, "11");
  assert.equal(estimate.max_origin_energy_limit_total, "49380");
});

test("deployment funding estimate rejects unsafe fee and margin inputs", () => {
  assert.throws(() => estimateDeploymentFunding({ "fee-limit": "0" }), /fee-limit/);
  assert.throws(
    () => estimateDeploymentFunding({ "trigger-fee-limit": "1.5" }),
    /trigger-fee-limit/,
  );
  assert.throws(
    () => estimateDeploymentFunding({ "origin-energy-limit": "-1" }),
    /origin-energy-limit/,
  );
  assert.throws(
    () => estimateDeploymentFunding({ "safety-margin-percent": "101" }),
    /safety-margin-percent/,
  );
});

test("TRON endpoint normalization rejects unsafe deployment gateway overrides", () => {
  assert.equal(normalizeTronEndpoint("https://api.trongrid.io/"), "https://api.trongrid.io");
  assert.equal(
    normalizeTronEndpoint("https://api.trongrid.io/custom/path/"),
    "https://api.trongrid.io/custom/path",
  );
  assert.throws(() => normalizeTronEndpoint("http://api.trongrid.io"), /HTTPS/);
  assert.throws(() => normalizeTronEndpoint("https://user:pass@api.trongrid.io"), /credentials/);
  assert.throws(() => normalizeTronEndpoint("https://api.trongrid.io?api_key=secret"), /query/);
  assert.throws(() => normalizeTronEndpoint("https://api.trongrid.io/#fragment"), /fragment|query/);
  assert.throws(() => normalizeTronEndpoint("https://localhost"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://node.localhost"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://local"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://0.0.0.0"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://127.0.0.1"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://10.0.0.7"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://100.64.0.1"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://172.16.0.1"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://192.0.0.1"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://192.168.1.2"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://198.18.0.1"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://224.0.0.1"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://[::1]"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://[fd00::1]"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://[fe80::1]"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://[::ffff:127.0.0.1]"), /localhost|private/);
  assert.throws(() => normalizeTronEndpoint("https://[2001:db8::1]"), /localhost|private/);
});

test("deployment funding readiness reports gaps and fails underfunded broadcast preflight", () => {
  const options = {
    "fee-limit": "2000000",
    "trigger-fee-limit": "500000",
    "origin-energy-limit": "12345",
    "safety-margin-percent": "10",
  };
  const ready = buildDeploymentFundingReadiness({ balance: "11000000" }, options);
  assert.equal(ready.schema, "iroha-sccp-tron-taira-xor-funding-readiness/v1");
  assert.equal(ready.funding_ready, true);
  assert.equal(ready.funding_gap_sun, "0");
  assert.equal(assertDeploymentFundingReady({ balance: "11000000" }, options).funding_ready, true);

  const underfunded = buildDeploymentFundingReadiness({ balance: "10999999" }, options);
  assert.equal(underfunded.funding_ready, false);
  assert.equal(underfunded.funding_gap_sun, "1");
  assert.throws(
    () => assertDeploymentFundingReady({ balance: "10999999" }, options),
    /below the recommended minimum.*gap=1 SUN/u,
  );
  assert.throws(
    () => buildDeploymentFundingReadiness({ balance: "-1" }, options),
    /account\.balance/,
  );
});

test("deployment doctor reports ready local prerequisites without exposing deployer secrets", async () => {
  await withTempDir(async (dir) => {
    const secretPath = join(dir, "deployer.secret.json");
    const verifierPath = join(dir, "verifier.json");
    await writeDeployerSecret(secretPath);
    await writeJson(verifierPath, verifierMaterial());

    const report = await buildDeploymentDoctorReport(
      {
        secret: secretPath,
        verifier: verifierPath,
        "require-secret": "true",
        "require-verifier": "true",
        "require-optional-packages": "true",
      },
      {
        resolveNodeModule: (name) => `/mock/node_modules/${name}/index.js`,
        nodeVersion: "20.11.0",
      },
    );

    assert.equal(report.schema, "iroha-sccp-tron-taira-xor-deployment-doctor/v1");
    assert.equal(report.ready, true);
    assert.equal(report.endpoint, "https://api.trongrid.io");
    assert.equal(report.summary.error ?? 0, 0);
    assert.equal(
      report.checks.find((entry) => entry.name === "deployer_secret")?.address_base58,
      deployerAddress.base58,
    );
    assert.equal(
      report.checks.find((entry) => entry.name === "verifier_material")?.network_id_hex,
      TRON_MAINNET_NETWORK_ID_HEX,
    );
    assert.equal(JSON.stringify(report).includes(bytesToHex(privateKey, false)), false);
  });
});

test("deployment doctor fails closed for unsafe or missing production inputs", async () => {
  await withTempDir(async (dir) => {
    const verifierPath = join(dir, "bad-verifier.json");
    await writeJson(verifierPath, { ...verifierMaterial(), networkId: routeHash("wrong-network") });

    const report = await buildDeploymentDoctorReport(
      {
        endpoint: "http://127.0.0.1:9090",
        secret: join(dir, "missing.secret.json"),
        verifier: verifierPath,
        "require-secret": "true",
        "require-optional-packages": "true",
      },
      {
        resolveNodeModule: () => {
          throw new Error("not installed");
        },
        nodeVersion: "16.20.0",
      },
    );

    assert.equal(report.ready, false);
    const byName = Object.fromEntries(report.checks.map((entry) => [entry.name, entry]));
    assert.equal(byName.node_version.status, "error");
    assert.equal(byName.tron_endpoint.status, "error");
    assert.equal(byName.deployer_secret.status, "error");
    assert.equal(byName.verifier_material.status, "error");
    assert.equal(byName["optional_package:solc"].status, "error");
    assert.equal(byName["optional_package:ethers"].status, "error");
  });
});

test("deployment doctor can check funded deployer account without broadcasting", async () => {
  await withTempDir(async (dir) => {
    const secretPath = join(dir, "deployer.secret.json");
    const verifierPath = join(dir, "verifier.json");
    await writeDeployerSecret(secretPath);
    await writeJson(verifierPath, verifierMaterial());
    const requests = [];

    const report = await buildDeploymentDoctorReport(
      {
        secret: secretPath,
        verifier: verifierPath,
        "check-account": "true",
      },
      {
        resolveNodeModule: (name) => `/mock/node_modules/${name}/index.js`,
        tronPost: async (endpoint, path, body) => {
          requests.push({ endpoint, path, body });
          return { balance: "73600000000" };
        },
      },
    );

    assert.equal(report.ready, true);
    assert.equal(requests.length, 1);
    assert.deepEqual(requests[0], {
      endpoint: "https://api.trongrid.io",
      path: "wallet/getaccount",
      body: { address: deployerAddress.base58, visible: true },
    });
    assert.equal(report.funding_readiness.funding_ready, true);
    assert.equal(
      report.checks.find((entry) => entry.name === "deployer_funding")?.status,
      "ok",
    );
  });
});

test("TAIRA burn-record contract artifact is compiled with forced IVM ZK mode", async () => {
  const contract = await compileTairaBurnRecordContract();
  assert.equal(contract.schema, "iroha-sccp-taira-xor-burn-record-contract/v1");
  assert.equal(contract.route_id, "taira_tron_xor");
  assert.equal(contract.asset_key, "xor");
  assert.match(contract.code_hash, /^[0-9a-f]{64}$/u);
  assert.match(contract.artifact_sha256, /^0x[0-9a-f]{64}$/u);
  assert.equal(contract.execution.executable, "IvmProved");
  assert.equal(contract.execution.force_zk_mode, true);
  assert.equal(contract.execution.entrypoint, "burn_and_record");
  assert.equal(contract.manifest?.features_bitmap, 1);
});

test("route manifest draft binds deployment evidence, verifier material, and TAIRA burn-record contract", async () => {
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir);
    const verifierCodeHash = routeHash("deployed-verifier-code");
    const expectedBindingHash = tronDestinationBindingHash({
      networkId: TRON_MAINNET_NETWORK_ID_HEX,
      verifierAddress: routeAddresses.verifier.base58,
      verifierCodeHash,
      verifierKeyHash: routeHash("verifier-key"),
    });
    const expectedBindingKey = tronDestinationBindingKey({
      networkId: TRON_MAINNET_NETWORK_ID_HEX,
      verifierAddress: routeAddresses.verifier.base58,
      verifierCodeHash,
      verifierKeyHash: routeHash("verifier-key"),
    });

    const manifest = await buildTairaXorRouteManifestDraft({
      evidence: evidencePath,
      "taira-contract": contractPath,
      verifier: verifierPath,
      "verifier-code-hash": verifierCodeHash,
      "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
      "vk-backend": "halo2/ipa",
      "vk-name": "taira_xor_burn_record_v1",
      "expected-destination-binding-hash": expectedBindingHash,
      "expected-destination-binding-key": expectedBindingKey,
      "production-ready": "true",
      "live-readback-checked": "true",
      "confirm-mainnet": "taira_tron_xor",
    });

    assert.equal(manifest.schema, "iroha-sccp-taira-xor-route-manifest-draft/v1");
    assert.equal(manifest.routeId, "taira_tron_xor");
    assert.equal(manifest.assetKey, "xor");
    assert.equal(manifest.productionReady, true);
    assert.equal(manifest.tairaXorTokenAddress, routeAddresses.token.base58);
    assert.equal(manifest.tairaXorBridgeAddress, routeAddresses.bridge.base58);
    assert.equal(manifest.sccpTronSourceBridgeAddress, routeAddresses.sourceBridge.base58);
    assert.equal(manifest.tronVerifierAddress, routeAddresses.verifier.base58);
    assert.equal(manifest.destinationRollout.destinationNetworkId, TRON_MAINNET_NETWORK_ID_HEX);
    assert.equal(manifest.destinationRollout.verifierCodeHash, verifierCodeHash);
    assert.equal(manifest.destinationRollout.verifierKeyHash, routeHash("verifier-key"));
    assert.equal(manifest.destinationRollout.destinationBindingHash, expectedBindingHash);
    assert.equal(manifest.destinationRollout.destinationBindingKey, expectedBindingKey);
    assert.equal(manifest.destinationBinding.bindingHash, expectedBindingHash);
    assert.equal(manifest.tairaXorBurnRecord.settlementAssetDefinitionId, "6TEAJqbb8oEPmLncoNiMRbLEK6tw");
    assert.equal(manifest.tairaXorBurnRecord.contractArtifactB64, burnArtifactB64);
    assert.equal(manifest.tairaXorBurnRecord.artifactSha256, burnArtifactSha256);
    assert.deepEqual(manifest.tairaXorBurnRecord.vkRef, {
      backend: "halo2/ipa",
      name: "taira_xor_burn_record_v1",
    });
    assert.equal(manifest.tairaXorBurnRecord.gasLimit, 2_000_000);
    assert.equal(JSON.stringify(manifest).includes("private_key"), false);
  });
});

test("route manifest draft defaults to disabled and requires production readiness acknowledgements", async () => {
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir);
    const baseOptions = {
      evidence: evidencePath,
      "taira-contract": contractPath,
      verifier: verifierPath,
      "verifier-code-hash": routeHash("deployed-verifier-code"),
      "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
      "vk-backend": "halo2/ipa",
      "vk-name": "taira_xor_burn_record_v1",
    };

    const disabled = await buildTairaXorRouteManifestDraft(baseOptions);
    assert.equal(disabled.productionReady, false);
    assert.match(disabled.disabledReason, /not production-ready/u);

    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "production-ready": "true",
          "live-readback-checked": "true",
        }),
      /confirm-mainnet/u,
    );
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "production-ready": "true",
          "confirm-mainnet": "taira_tron_xor",
        }),
      /live-readback-checked/u,
    );
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "production-ready": "true",
          "live-readback-checked": "true",
          "confirm-mainnet": "taira_tron_xor",
        }),
      /expected-destination-binding-hash/u,
    );
  });
});

test("route manifest draft rejects wrong route, wrong network, and duplicate deployment addresses", async () => {
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir, {
      evidence: { route_id: "minamoto_tron_xor" },
    });
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          evidence: evidencePath,
          "taira-contract": contractPath,
          verifier: verifierPath,
          "verifier-code-hash": routeHash("deployed-verifier-code"),
          "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
          "vk-backend": "halo2/ipa",
          "vk-name": "taira_xor_burn_record_v1",
        }),
      /route_id/u,
    );
  });
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir, {
      evidence: { network_id_hex: routeHash("wrong-network") },
    });
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          evidence: evidencePath,
          "taira-contract": contractPath,
          verifier: verifierPath,
          "verifier-code-hash": routeHash("deployed-verifier-code"),
          "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
          "vk-backend": "halo2/ipa",
          "vk-name": "taira_xor_burn_record_v1",
        }),
      /TRON mainnet/u,
    );
  });
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir, {
      evidence: { sccp_tron_destination_verifier_address: routeAddresses.bridge.base58 },
    });
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          evidence: evidencePath,
          "taira-contract": contractPath,
          verifier: verifierPath,
          "verifier-code-hash": routeHash("deployed-verifier-code"),
          "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
          "vk-backend": "halo2/ipa",
          "vk-name": "taira_xor_burn_record_v1",
        }),
      /distinct/u,
    );
  });
});

test("route manifest draft rejects aliases, malformed VK refs, and mismatched verifier hashes", async () => {
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir);
    const baseOptions = {
      evidence: evidencePath,
      "taira-contract": contractPath,
      verifier: verifierPath,
      "verifier-code-hash": routeHash("deployed-verifier-code"),
      "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
      "vk-backend": "halo2/ipa",
      "vk-name": "taira_xor_burn_record_v1",
    };
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "settlement-asset-definition-id": "xor#universal",
        }),
      /canonical Base58 asset definition ID/u,
    );
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "vk-name": "bad name",
        }),
      /unsupported characters/u,
    );
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "verifier-key-hash": routeHash("different-verifier-key"),
        }),
      /does not match --verifier material/u,
    );
  });
});

test("route manifest draft rejects tampered TAIRA burn-record contract artifacts", async () => {
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir, {
      contract: { artifact_sha256: routeHash("wrong-artifact-sha") },
    });
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          evidence: evidencePath,
          "taira-contract": contractPath,
          verifier: verifierPath,
          "verifier-code-hash": routeHash("deployed-verifier-code"),
          "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
          "vk-backend": "halo2/ipa",
          "vk-name": "taira_xor_burn_record_v1",
        }),
      /artifact_sha256/u,
    );
  });
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir, {
      contract: { artifact_b64: "not base64" },
    });
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          evidence: evidencePath,
          "taira-contract": contractPath,
          verifier: verifierPath,
          "verifier-code-hash": routeHash("deployed-verifier-code"),
          "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
          "vk-backend": "halo2/ipa",
          "vk-name": "taira_xor_burn_record_v1",
        }),
      /strict base64/u,
    );
  });
});

test("route manifest draft rejects destination binding and gas limit mismatches", async () => {
  await withTempDir(async (dir) => {
    const { evidencePath, contractPath, verifierPath } = await writeRouteManifestInputs(dir);
    const baseOptions = {
      evidence: evidencePath,
      "taira-contract": contractPath,
      verifier: verifierPath,
      "verifier-code-hash": routeHash("deployed-verifier-code"),
      "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
      "vk-backend": "halo2/ipa",
      "vk-name": "taira_xor_burn_record_v1",
    };
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "expected-destination-binding-hash": routeHash("wrong-binding"),
        }),
      /destination binding hash/u,
    );
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "expected-destination-binding-key": "tron:bad",
        }),
      /destination binding key/u,
    );
    await assert.rejects(
      () =>
        buildTairaXorRouteManifestDraft({
          ...baseOptions,
          "gas-limit": "0",
        }),
      /gas-limit/u,
    );
  });
});
