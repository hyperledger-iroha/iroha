import assert from "node:assert/strict";
import { test } from "node:test";
import {
  BSC_TESTNET_NETWORK_ID_HEX,
  SCCP_DOMAIN_BSC,
  SCCP_DOMAIN_SORA,
  bscDestinationBindingHash,
  bscDestinationBindingKey,
  buildDeploymentEvidence,
  main,
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
