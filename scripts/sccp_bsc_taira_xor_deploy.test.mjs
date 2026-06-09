import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1,
  SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1,
  SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
  SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
  SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
  SCCP_NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1,
  validateBscTestnetNativeEvmProverBundle,
} from "../javascript/iroha_js/src/sccp.js";
import {
  BSC_MAINNET_NETWORK_ID_HEX,
  BSC_TESTNET_NETWORK_ID_HEX,
  CANONICAL_BSC_PRODUCTION_ARTIFACT_ROOT,
  ROUTE_MANIFEST_SCHEMA,
  SCCP_BSC_DIAGNOSTIC_VERIFIER_KEY_HASHES,
  SCCP_DOMAIN_BSC,
  SCCP_DOMAIN_SORA,
  bscCanonicalProductionOutputProblems,
  canonicalBscNativeEvmProverBundleHash,
  bscDestinationBindingHash,
  bscDestinationBindingKey,
  buildBscNativeEvmProverBundleFromArtifacts,
  buildBscTairaXorRouteConfigToml,
  buildDeploymentEvidence,
  buildMergedBscTairaXorRouteConfigToml,
  main,
  isKnownDiagnosticBscVerifierKeyHash,
  isCanonicalBscProductionArtifactPath,
  isSmokeFixtureGroth16VerifierMaterial,
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
const HASH_44 = `0x${"44".repeat(32)}`;
const HASH_55 = `0x${"55".repeat(32)}`;
const HASH_66 = `0x${"66".repeat(32)}`;
const HASH_77 = `0x${"77".repeat(32)}`;
const hex32 = (byte) => `0x${byte.repeat(32)}`;
const SOURCE_EVENT_EXPLORER_URL = `https://testnet.bscscan.com/tx/${HASH_55}`;
const ROUTE_CANARY_EXPLORER_URL = `https://testnet.bscscan.com/tx/${HASH_77}`;
const MAINNET_SOURCE_EVENT_EXPLORER_URL = `https://bscscan.com/tx/${HASH_55}`;
const MAINNET_ROUTE_CANARY_EXPLORER_URL = `https://bscscan.com/tx/${HASH_77}`;
const DIAGNOSTIC_BSC_VERIFIER_KEY_HASH = [
  ...SCCP_BSC_DIAGNOSTIC_VERIFIER_KEY_HASHES,
][0];
const SMOKE_FIXTURE_G1 = ["1", "2"];
const SMOKE_FIXTURE_G2 = [
  "10857046999023057135944570762232829481370756359578518086990519993285655852781",
  "11559732032986387107991004021392285783925812861821192530917403151452391805634",
  "8495653923123431417604973247489272438418190587263600148770280649306958101930",
  "4082367875863433681332203403145435568316851327593401208105741076214120093531",
];
const SMOKE_FIXTURE_IC = Array.from(
  { length: 10 },
  () => SMOKE_FIXTURE_G1,
).flat();
const BURN_RECORD_BYTES = Buffer.from(
  "bsc taira xor burn-record artifact fixture for route-config tests",
  "utf8",
);
const BURN_RECORD_B64 = BURN_RECORD_BYTES.toString("base64");
const BURN_RECORD_SHA256 = `0x${createHash("sha256").update(BURN_RECORD_BYTES).digest("hex")}`;
const sha256Hex = (bytes) =>
  `0x${createHash("sha256").update(bytes).digest("hex")}`;

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

const nativeProverBundleForRollout = (destinationRollout, overrides = {}) => {
  const proofArtifactHash = destinationRollout.proofArtifactHash;
  const provingKeyHash = destinationRollout.provingKeyHash;
  return {
    schema: SCCP_NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1,
    bundle_id: SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
    domain: SCCP_DOMAIN_BSC,
    chain: "bsc-testnet",
    proof_backend: SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
    proof_artifact: "artifacts/bsc-testnet/proof-artifact.bin",
    proof_artifact_hash: proofArtifactHash,
    proving_key: "artifacts/bsc-testnet/proving-key.bin",
    proving_key_hash: provingKeyHash,
    verifier_key: "artifacts/bsc-testnet/verifier-key.bin",
    verifier_key_hash: destinationRollout.verifierKeyHash,
    destination_binding_hash: destinationRollout.destinationBindingHash,
    no_wasm: true,
    remote_prover_required: false,
    browser_implementation: "pure-typescript",
    cross_sdk_fixture_parity_artifact:
      "artifacts/bsc-testnet/cross-sdk-fixture-parity.json",
    native_prover_self_test_artifact:
      "artifacts/bsc-testnet/native-prover-self-test.json",
    native_sdk_artifacts: Object.entries(
      SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
    ).map(([sdk, implementation], index) => ({
      sdk,
      implementation,
      prover_artifact_hash: proofArtifactHash,
      proving_key_hash: provingKeyHash,
      implementation_artifact: `artifacts/bsc-testnet/${sdk}-implementation.bin`,
      implementation_hash: hex32((0x81 + index).toString(16)),
    })),
    audit_hashes: {
      circuit_security_audit: hex32("91"),
      native_implementation_audit: hex32("92"),
      reproducible_build_attestation: hex32("93"),
      cross_sdk_fixture_parity: hex32("94"),
      native_prover_self_test: hex32("95"),
      no_wasm_no_remote_scan: hex32("96"),
    },
    ...overrides,
  };
};

const attachNativeProverBundle = (manifest, bundleOverrides = {}) => ({
  ...manifest,
  nativeEvmProverBundle: nativeProverBundleForRollout(
    manifest.destinationRollout,
    bundleOverrides,
  ),
});
const hasOwn = (record, key) =>
  Object.prototype.hasOwnProperty.call(record, key);

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
    proofArtifactHash: HASH_44,
    provingKeyHash: HASH_55,
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
    sourceBridgeConfigHash: HASH_44,
    sourceEventTransactionId: HASH_55,
    sourceEventExplorerUrl: SOURCE_EVENT_EXPLORER_URL,
    routeCanaryEvidenceHash: HASH_66,
    routeCanaryTransactionId: HASH_77,
    routeCanaryExplorerUrl: ROUTE_CANARY_EXPLORER_URL,
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

const productionReadyRouteManifest = (overrides = {}) => {
  const {
    bundleOverrides,
    skipNativeEvmProverBundle,
    postDeployLiveEvidence: postDeployOverrides,
    ...manifestOverrides
  } = overrides;
  const manifest = routeManifest({
    productionReady: true,
    disabledReason: undefined,
    postDeployLiveEvidence: {
      fullTomlReady: true,
      offlineFullTomlSha256: HASH_33,
      ...postDeployOverrides,
    },
    ...manifestOverrides,
  });
  if (
    skipNativeEvmProverBundle ||
    hasOwn(manifestOverrides, "nativeEvmProverBundle")
  ) {
    return manifest;
  }
  return attachNativeProverBundle(manifest, bundleOverrides ?? {});
};

const fixtureWords = (byte) => Array.from({ length: 9 }, () => hex32(byte));

const nativeProverSdkResults = (fields) =>
  Object.fromEntries(
    Object.keys(SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1).map(
      (sdk) => [sdk, { ...fields }],
    ),
  );

const nativeProverParityFixture = ({
  proofArtifactHash,
  provingKeyHash,
  verifierKeyHash,
  destinationBindingHash,
}) => {
  const fields = {
    receipt_proof_hash: hex32("a1"),
    source_proof_hash: hex32("a2"),
    destination_binding_hash: destinationBindingHash,
    public_signal_words: fixtureWords("a3"),
    calldata_hash: hex32("a4"),
    torii_submit_payload_hash: hex32("a5"),
  };
  return {
    schema: SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1,
    domain: SCCP_DOMAIN_BSC,
    chain: "bsc-testnet",
    proof_backend: SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
    proof_artifact_hash: proofArtifactHash,
    proving_key_hash: provingKeyHash,
    verifier_key_hash: verifierKeyHash,
    destination_binding_hash: destinationBindingHash,
    ...fields,
    sdk_results: nativeProverSdkResults(fields),
  };
};

const nativeProverSelfTestFixture = ({
  proofArtifactHash,
  provingKeyHash,
  verifierKeyHash,
  destinationBindingHash,
}) => {
  const fields = {
    request_hash: hex32("b1"),
    witness_hash: hex32("b2"),
    source_proof_hash: hex32("b3"),
    proof_hash: hex32("b4"),
    public_signal_words: fixtureWords("b5"),
    calldata_hash: hex32("b6"),
    torii_submit_payload_hash: hex32("b7"),
  };
  return {
    schema: SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1,
    domain: SCCP_DOMAIN_BSC,
    chain: "bsc-testnet",
    proof_backend: SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
    proof_artifact_hash: proofArtifactHash,
    proving_key_hash: provingKeyHash,
    verifier_key_hash: verifierKeyHash,
    destination_binding_hash: destinationBindingHash,
    ...fields,
    sdk_results: nativeProverSdkResults(fields),
  };
};

async function writeNativeProverFixtureFiles({
  routeOverrides = {},
  artifactByteOverrides = {},
} = {}) {
  const workDir = await mkdtemp(join(tmpdir(), "iroha-bsc-native-prover."));
  const artifactRoot = join(workDir, "native");
  await mkdir(artifactRoot, { recursive: true });
  const writeArtifact = async (relativePath, bytes) => {
    const pathName = join(artifactRoot, relativePath);
    await writeFile(pathName, bytes);
    return pathName;
  };
  const bytesFor = (label, size) => {
    const seed = Buffer.from(`${label}: route-bound native bsc material\n`);
    const bytes = Buffer.alloc(size);
    for (let index = 0; index < bytes.length; index += 1) {
      bytes[index] =
        (seed[index % seed.length] + index * 31 + (index >> 7)) & 0xff;
    }
    return bytes;
  };
  const proofBytes =
    artifactByteOverrides.proofArtifact ??
    artifactByteOverrides.proof ??
    bytesFor("proof-artifact", 96 * 1024);
  const provingKeyBytes =
    artifactByteOverrides.provingKey ?? bytesFor("proving-key", 96 * 1024);
  const verifierKeyBytes =
    artifactByteOverrides.verifierKey ?? bytesFor("verifier-key", 2048);
  const proofArtifactHash = sha256Hex(proofBytes);
  const provingKeyHash = sha256Hex(provingKeyBytes);
  const verifierKeyHash = sha256Hex(verifierKeyBytes);
  const destinationBindingHash = bscDestinationBindingHash({
    verifierAddress: BSC_VERIFIER_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash,
  });
  const destinationBindingKey = bscDestinationBindingKey({
    verifierAddress: BSC_VERIFIER_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash,
  });
  const bundleBinding = {
    proofArtifactHash,
    provingKeyHash,
    verifierKeyHash,
    destinationBindingHash,
  };
  const parityBytes = Buffer.from(
    `${JSON.stringify(nativeProverParityFixture(bundleBinding), null, 2)}\n`,
  );
  const selfTestBytes = Buffer.from(
    `${JSON.stringify(nativeProverSelfTestFixture(bundleBinding), null, 2)}\n`,
  );
  await writeArtifact("proof-artifact.bin", proofBytes);
  await writeArtifact("proving-key.bin", provingKeyBytes);
  await writeArtifact("verifier-key.json", verifierKeyBytes);
  await writeArtifact("cross-sdk-fixture-parity.json", parityBytes);
  await writeArtifact("native-prover-self-test.json", selfTestBytes);
  const sdkImplementationPaths = {};
  for (const sdk of Object.keys(
    SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
  )) {
    const relativePath = `${sdk}-implementation.bin`;
    sdkImplementationPaths[sdk] = relativePath;
    await writeArtifact(
      relativePath,
      artifactByteOverrides[`${sdk}Implementation`] ??
        artifactByteOverrides[sdk] ??
        bytesFor(`${sdk}-implementation`, 2048),
    );
  }
  for (const name of [
    "circuit-security-audit.bin",
    "native-implementation-audit.bin",
    "reproducible-build-attestation.bin",
    "no-wasm-no-remote-scan.bin",
  ]) {
    await writeArtifact(name, bytesFor(name, 2048));
  }
  const {
    destinationRollout: routeDestinationRolloutOverrides,
    destinationBinding: routeDestinationBindingOverrides,
    ...topLevelRouteOverrides
  } = routeOverrides;
  const manifest = routeManifest({
    destinationRollout: {
      verifierKeyHash,
      proofArtifactHash,
      provingKeyHash,
      destinationBindingHash,
      destinationBindingKey,
      ...routeDestinationRolloutOverrides,
    },
    destinationBinding: {
      key: destinationBindingKey,
      bindingHash: destinationBindingHash,
      ...routeDestinationBindingOverrides,
    },
    ...topLevelRouteOverrides,
  });
  const routeManifestPath = join(workDir, "route.json");
  await writeFile(routeManifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  return {
    workDir,
    artifactRoot,
    routeManifestPath,
    proofArtifactHash,
    provingKeyHash,
    verifierKeyHash,
    destinationBindingHash,
    sdkImplementationPaths,
    options: {
      "route-manifest": routeManifestPath,
      "artifact-root": artifactRoot,
      "proof-artifact": "proof-artifact.bin",
      "proving-key": "proving-key.bin",
      "verifier-key": "verifier-key.json",
      "cross-sdk-fixture-parity": "cross-sdk-fixture-parity.json",
      "native-prover-self-test": "native-prover-self-test.json",
      "javascript-implementation": sdkImplementationPaths.javascript,
      "swift-implementation": sdkImplementationPaths.swift,
      "kotlin-implementation": sdkImplementationPaths.kotlin,
      "java-android-implementation": sdkImplementationPaths["java-android"],
      "dotnet-implementation": sdkImplementationPaths.dotnet,
      "audit-circuit-security": "circuit-security-audit.bin",
      "audit-native-implementation": "native-implementation-audit.bin",
      "audit-reproducible-build": "reproducible-build-attestation.bin",
      "audit-no-wasm-no-remote-scan": "no-wasm-no-remote-scan.bin",
    },
  };
}

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
  assert.equal(
    evidence.destinationRollout.destinationBindingHash,
    bindingHash(),
  );
  assert.equal(evidence.bscContractReadback.bridgeVerifierKeyHash, HASH_22);
  assert.doesNotMatch(
    JSON.stringify(evidence),
    /private[_-]?key|mnemonic|seed/iu,
  );
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
    [
      readyReadback({ sourceBridgeOwner: BSC_SOURCE_BRIDGE_ADDRESS }),
      /source bridge owner/u,
    ],
    [
      readyReadback({ bridgeDestinationBindingHash: HASH_33 }),
      /destination binding/u,
    ],
    [
      readyReadback({ bridgeVerifierAddress: BSC_BRIDGE_ADDRESS }),
      /verifier address/u,
    ],
    [readyReadback({ bridgeVerifierCodeHash: HASH_33 }), /verifier code hash/u],
    [readyReadback({ bridgeVerifierKeyHash: HASH_33 }), /verifier key hash/u],
    [
      readyReadback({ bridgeNetworkId: `0x${"38".padStart(64, "0")}` }),
      /network id/u,
    ],
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
  assert.match(
    unsafeSecretReason({ nested: { private_key: "0xabc" } }),
    /private key/u,
  );
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
    () =>
      normalizeVerifierMaterial(
        verifierMaterial({ proofFamily: "groth16-only" }),
      ),
    /proofFamily/u,
  );
  assert.throws(
    () =>
      normalizeVerifierMaterial(
        verifierMaterial({ networkId: `0x${"38".padStart(64, "0")}` }),
      ),
    /BSC testnet/u,
  );
  assert.throws(
    () => normalizeVerifierMaterial(verifierMaterial({ sourceDomain: 2 })),
    /SORA -> BSC/u,
  );
  assert.throws(
    () => normalizeVerifierMaterial(verifierMaterial({ targetDomain: 1 })),
    /SORA -> BSC/u,
  );
  assert.throws(
    () => normalizeVerifierMaterial(verifierMaterial({ ic: [1, 2] })),
    /20 uint256/u,
  );
  assert.throws(
    () =>
      normalizeVerifierMaterial({
        ...verifierMaterial(),
        verifierKeyHash: HASH_22,
        alpha1: [0],
      }),
    /2 uint256/u,
  );
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

test("BSC verifier material rejects smoke-test generator-point fixtures", () => {
  const smokeFixtureMaterial = verifierMaterial({
    alpha1: SMOKE_FIXTURE_G1,
    beta2: SMOKE_FIXTURE_G2,
    gamma2: SMOKE_FIXTURE_G2,
    delta2: SMOKE_FIXTURE_G2,
    ic: SMOKE_FIXTURE_IC,
  });

  const normalized = normalizeVerifierMaterial(smokeFixtureMaterial);
  assert.equal(isSmokeFixtureGroth16VerifierMaterial(smokeFixtureMaterial), true);
  assert.equal(normalized.fixtureShaped, true);
  assert.match(
    normalized.diagnosticVerifierReasons.join(" "),
    /smoke-test Groth16 fixture/u,
  );

  assert.equal(isSmokeFixtureGroth16VerifierMaterial(verifierMaterial()), false);
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
  assert.match(
    toml,
    new RegExp(`taira_xor_token_address = "${BSC_TOKEN_ADDRESS}"`, "u"),
  );
  assert.match(
    toml,
    new RegExp(`taira_xor_bridge_address = "${BSC_BRIDGE_ADDRESS}"`, "u"),
  );
  assert.match(
    toml,
    new RegExp(`source_bridge_address = "${BSC_SOURCE_BRIDGE_ADDRESS}"`, "u"),
  );
  assert.match(
    toml,
    new RegExp(
      `sccp_bsc_source_bridge_address = "${BSC_SOURCE_BRIDGE_ADDRESS}"`,
      "u",
    ),
  );
  assert.match(
    toml,
    new RegExp(
      `bsc_source_bridge_address = "${BSC_SOURCE_BRIDGE_ADDRESS}"`,
      "u",
    ),
  );
  assert.match(
    toml,
    new RegExp(
      `sccp_tron_source_bridge_address = "${BSC_SOURCE_BRIDGE_ADDRESS}"`,
      "u",
    ),
  );
  assert.match(
    toml,
    new RegExp(`destination_verifier_address = "${BSC_VERIFIER_ADDRESS}"`, "u"),
  );
  assert.match(
    toml,
    new RegExp(`verifier_address = "${BSC_VERIFIER_ADDRESS}"`, "u"),
  );
  assert.match(
    toml,
    new RegExp(
      `sccp_bsc_destination_verifier_address = "${BSC_VERIFIER_ADDRESS}"`,
      "u",
    ),
  );
  assert.match(
    toml,
    new RegExp(`bsc_verifier_address = "${BSC_VERIFIER_ADDRESS}"`, "u"),
  );
  assert.match(
    toml,
    new RegExp(`evm_verifier_address = "${BSC_VERIFIER_ADDRESS}"`, "u"),
  );
  assert.match(
    toml,
    new RegExp(`tron_verifier_address = "${BSC_VERIFIER_ADDRESS}"`, "u"),
  );
  assert.match(toml, new RegExp(`proof_artifact_hash = "${HASH_44}"`, "u"));
  assert.match(toml, new RegExp(`prover_artifact_hash = "${HASH_44}"`, "u"));
  assert.match(toml, new RegExp(`circuit_artifact_hash = "${HASH_44}"`, "u"));
  assert.match(toml, new RegExp(`proving_key_hash = "${HASH_55}"`, "u"));
  assert.doesNotMatch(toml, /native_evm_prover_bundle_hash/u);
  assert.match(
    toml,
    new RegExp(`destination_binding_hash = "${bindingHash()}"`, "u"),
  );
  assert.match(
    toml,
    new RegExp(
      `taira_burn_record_artifact_sha256 = "${BURN_RECORD_SHA256}"`,
      "u",
    ),
  );
  assert.match(toml, /post_deploy_full_toml_ready = false/u);
  assert.match(
    toml,
    new RegExp(
      `post_deploy_source_event_explorer_url = "${SOURCE_EVENT_EXPLORER_URL}"`,
      "u",
    ),
  );
  assert.match(
    toml,
    new RegExp(
      `post_deploy_route_canary_explorer_url = "${ROUTE_CANARY_EXPLORER_URL}"`,
      "u",
    ),
  );
  assert.doesNotMatch(toml, /private[_-]?key|mnemonic|seed[_-]?phrase/iu);
});

test("BSC route-config requires explicit post-deploy evidence for production-ready manifests", () => {
  const productionReadyManifest = (postDeployOverrides = {}, overrides = {}) => {
    const manifest = routeManifest({
      productionReady: true,
      disabledReason: undefined,
      postDeployLiveEvidence: {
        fullTomlReady: true,
        offlineFullTomlSha256: HASH_33,
        ...postDeployOverrides,
      },
      ...overrides,
    });
    return hasOwn(overrides, "nativeEvmProverBundle")
      ? manifest
      : attachNativeProverBundle(manifest);
  };

  const toml = buildBscTairaXorRouteConfigToml(productionReadyManifest());
  assert.match(toml, /production_ready = true/u);
  assert.match(
    toml,
    new RegExp(`post_deploy_offline_full_toml_sha256 = "${HASH_33}"`, "u"),
  );
  assert.match(
    toml,
    new RegExp(
      `post_deploy_source_event_explorer_url = "${SOURCE_EVENT_EXPLORER_URL}"`,
      "u",
    ),
  );
  assert.match(
    toml,
    new RegExp(
      `post_deploy_route_canary_explorer_url = "${ROUTE_CANARY_EXPLORER_URL}"`,
      "u",
    ),
  );

  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyManifest({ fullTomlReady: false }),
      ),
    /fullTomlReady true/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyManifest({ sourceEventExplorerUrl: undefined }),
      ),
    /sourceEventExplorerUrl/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyManifest({ routeCanaryExplorerUrl: undefined }),
      ),
    /routeCanaryExplorerUrl/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyManifest({ offlineFullTomlSha256: undefined }),
      ),
    /offlineFullTomlSha256/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyManifest({
          sourceEventTransactionProductionBlockers: [
            "source event transaction has not been observed on mainnet",
          ],
        }),
      ),
    /empty postDeployLiveEvidence production blockers.*sourceEventTransactionProductionBlockers/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyManifest({
          productionBlockers: "source event transaction is still pending",
        }),
      ),
    /productionBlockers must be a list of non-empty strings/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyManifest({
          routeCanaryProductionBlockers: [" route canary evidence is stale"],
        }),
      ),
    /routeCanaryProductionBlockers\[0\] must be a non-empty canonical string/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyManifest({
          sourceEventExplorerUrl: `https://bscscan.com/tx/${HASH_55}`,
        }),
      ),
    /BSC testnet explorer/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyManifest({
          routeCanaryExplorerUrl: `https://testnet.bscscan.com/tx/${HASH_55}`,
        }),
      ),
    /transaction hash must match/u,
  );
});

test("BSC route-config refuses allow-unready for production-ready manifests", () => {
  const manifest = productionReadyRouteManifest();
  const toml = buildBscTairaXorRouteConfigToml(manifest);
  assert.match(toml, /production_ready = true/u);
  assert.match(toml, /sccp_allow_unready_transparent_proofs = false/u);

  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(manifest, {
        "allow-unready": "true",
      }),
    /production-ready route manifests cannot enable --allow-unready/u,
  );
  assert.throws(
    () =>
      buildMergedBscTairaXorRouteConfigToml(
        "[zk]\nother_setting = true\n",
        manifest,
        {
          "allow-unready": "true",
        },
      ),
    /production-ready route manifests cannot enable --allow-unready/u,
  );
});

test("BSC route-config validates explorer URLs against the selected network", () => {
  const mainnetBindingHash = bscDestinationBindingHash({
    networkId: BSC_MAINNET_NETWORK_ID_HEX,
    verifierAddress: BSC_VERIFIER_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: HASH_22,
  });
  const mainnetBindingKey = bscDestinationBindingKey({
    networkId: BSC_MAINNET_NETWORK_ID_HEX,
    verifierAddress: BSC_VERIFIER_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: HASH_22,
  });
  const mainnetManifest = routeManifest({
    bscNetwork: "mainnet",
    chain: "bsc-mainnet",
    chainIdHex: "0x38",
    networkIdHex: BSC_MAINNET_NETWORK_ID_HEX,
    destinationRollout: {
      destinationNetworkId: BSC_MAINNET_NETWORK_ID_HEX,
      destinationBindingHash: mainnetBindingHash,
      destinationBindingKey: mainnetBindingKey,
    },
    destinationBinding: {
      networkIdHex: BSC_MAINNET_NETWORK_ID_HEX,
      key: mainnetBindingKey,
      bindingHash: mainnetBindingHash,
    },
    postDeployLiveEvidence: {
      sourceEventExplorerUrl: MAINNET_SOURCE_EVENT_EXPLORER_URL,
      routeCanaryExplorerUrl: MAINNET_ROUTE_CANARY_EXPLORER_URL,
    },
  });

  const toml = buildBscTairaXorRouteConfigToml(mainnetManifest, {
    "allow-unready": "true",
  });
  assert.match(
    toml,
    new RegExp(
      `post_deploy_source_event_explorer_url = "${MAINNET_SOURCE_EVENT_EXPLORER_URL}"`,
      "u",
    ),
  );
  assert.match(
    toml,
    new RegExp(
      `post_deploy_route_canary_explorer_url = "${MAINNET_ROUTE_CANARY_EXPLORER_URL}"`,
      "u",
    ),
  );

  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        {
          ...mainnetManifest,
          postDeployLiveEvidence: {
            ...mainnetManifest.postDeployLiveEvidence,
            sourceEventExplorerUrl: SOURCE_EVENT_EXPLORER_URL,
          },
        },
        { "allow-unready": "true" },
      ),
    /BSC mainnet explorer/u,
  );
});

test("BSC route-config requires SDK-valid native prover bundles for production readiness", () => {
  const manifest = productionReadyRouteManifest();
  const expectedNativeBundleHash = canonicalBscNativeEvmProverBundleHash(
    validateBscTestnetNativeEvmProverBundle(
      nativeProverBundleForRollout(manifest.destinationRollout),
      { expectedDestinationBindingHash: manifest.destinationRollout.destinationBindingHash },
    ),
  );
  const validToml = buildBscTairaXorRouteConfigToml(manifest);
  assert.match(validToml, /production_ready = true/u);
  assert.match(validToml, new RegExp(`proof_artifact_hash = "${HASH_44}"`, "u"));
  assert.match(validToml, new RegExp(`proving_key_hash = "${HASH_55}"`, "u"));
  assert.match(
    validToml,
    new RegExp(
      `native_evm_prover_bundle_hash = "${expectedNativeBundleHash}"`,
      "u",
    ),
  );

  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyRouteManifest({ skipNativeEvmProverBundle: true }),
      ),
    /productionReady requires nativeEvmProverBundle/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyRouteManifest({
          bundleOverrides: { chain: "eth" },
        }),
      ),
    /nativeEvmProverBundle.*BSC SDK validation.*chain must be bsc-testnet/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml({
        ...manifest,
        nativeEvmProverBundleHash: HASH_66,
      }),
    /nativeEvmProverBundleHash does not match nativeEvmProverBundle/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyRouteManifest({
          bundleOverrides: { verifier_key_hash: HASH_33 },
        }),
      ),
    /nativeEvmProverBundle verifierKeyHash must match route manifest verifierKeyHash/u,
  );
  const proofHashDriftBase = productionReadyRouteManifest();
  const proofHashDriftBundle = nativeProverBundleForRollout(
    proofHashDriftBase.destinationRollout,
    {
      proof_artifact_hash: HASH_66,
      native_sdk_artifacts: Object.entries(
        SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
      ).map(([sdk, implementation], index) => ({
        sdk,
        implementation,
        prover_artifact_hash: HASH_66,
        proving_key_hash: proofHashDriftBase.destinationRollout.provingKeyHash,
        implementation_artifact: `artifacts/bsc-testnet/${sdk}-implementation.bin`,
        implementation_hash: hex32((0x81 + index).toString(16)),
      })),
    },
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml({
        ...proofHashDriftBase,
        nativeEvmProverBundle: proofHashDriftBundle,
      }),
    /nativeEvmProverBundle proofArtifactHash must match route manifest proofArtifactHash/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyRouteManifest({
          bundleOverrides: { destination_binding_hash: HASH_66 },
        }),
      ),
    /nativeEvmProverBundle.*BSC SDK validation.*destinationBindingHash/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyRouteManifest({
          bundleOverrides: { proof_artifact: "../proof-artifact.bin" },
        }),
      ),
    /nativeEvmProverBundle.*BSC SDK validation.*proofArtifact/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyRouteManifest({
          bundleOverrides: {
            proving_key: "artifacts/bsc-testnet/proof-artifact.bin",
          },
        }),
      ),
    /nativeEvmProverBundle.*BSC SDK validation.*artifact paths must be role-separated/u,
  );

  const aliasBase = productionReadyRouteManifest();
  const aliasDriftBundle = nativeProverBundleForRollout(
    aliasBase.destinationRollout,
    {
      audit_hashes: {
        circuit_security_audit: hex32("91"),
        native_implementation_audit: hex32("92"),
        reproducible_build_attestation: hex32("93"),
        cross_sdk_fixture_parity: hex32("94"),
        native_prover_self_test: hex32("95"),
        no_wasm_no_remote_scan: hex32("97"),
      },
    },
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml({
        ...aliasBase,
        destinationRollout: {
          ...aliasBase.destinationRollout,
          nativeProverBundle: aliasDriftBundle,
        },
      }),
    /nativeEvmProverBundle aliases disagree/u,
  );
});

test("BSC native-prover-bundle builds SDK-valid route-bound bundles from artifact files", async () => {
  const fixture = await writeNativeProverFixtureFiles();
  const result = await buildBscNativeEvmProverBundleFromArtifacts(
    fixture.options,
  );

  assert.equal(result.descriptor.bundleId, SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1);
  assert.equal(result.descriptor.proofArtifactHash, fixture.proofArtifactHash);
  assert.equal(result.descriptor.provingKeyHash, fixture.provingKeyHash);
  assert.equal(result.descriptor.verifierKeyHash, fixture.verifierKeyHash);
  assert.equal(
    result.descriptor.destinationBindingHash,
    fixture.destinationBindingHash,
  );
  assert.deepEqual(
    result.verifiedSdks.sort(),
    Object.keys(SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1).sort(),
  );
  assert.equal(
    result.bundle.audit_hashes.cross_sdk_fixture_parity,
    sha256Hex(await readFile(join(fixture.artifactRoot, "cross-sdk-fixture-parity.json"))),
  );
  assert.equal(
    result.bundle.audit_hashes.native_prover_self_test,
    sha256Hex(await readFile(join(fixture.artifactRoot, "native-prover-self-test.json"))),
  );
  assert.equal(
    result.attachedRouteManifest.destinationRollout.nativeEvmProverBundle
      .proof_artifact_hash,
    fixture.proofArtifactHash,
  );
  assert.equal(
    result.attachedRouteManifest.nativeEvmProverBundleHash,
    canonicalBscNativeEvmProverBundleHash(result.descriptor),
  );
  assert.equal(
    result.attachedRouteManifest.destinationRollout.nativeEvmProverBundleHash,
    result.attachedRouteManifest.nativeEvmProverBundleHash,
  );

  const out = join(fixture.workDir, "bundle.json");
  const attachedOut = join(fixture.workDir, "route.attached.json");
  const cliResult = await main([
    "native-prover-bundle",
    ...Object.entries(fixture.options).flatMap(([key, value]) => [
      `--${key}`,
      value,
    ]),
    "--out",
    out,
    "--attach-route-manifest-out",
    attachedOut,
  ]);
  assert.equal(cliResult.ok, true);
  assert.equal(JSON.parse(await readFile(out, "utf8")).proof_artifact_hash, fixture.proofArtifactHash);
  assert.equal(
    JSON.parse(await readFile(attachedOut, "utf8")).nativeEvmProverBundle
      .proving_key_hash,
    fixture.provingKeyHash,
  );
  assert.equal(
    JSON.parse(await readFile(attachedOut, "utf8"))
      .nativeEvmProverBundleHash,
    canonicalBscNativeEvmProverBundleHash(result.descriptor),
  );
});

test("BSC native-prover-bundle rejects forged or incomplete artifact inputs", async () => {
  const fixture = await writeNativeProverFixtureFiles();
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts({
        ...fixture.options,
        "proof-artifact": "../proof-artifact.bin",
      }),
    /proof artifact must stay under artifact-root/u,
  );
  await assert.rejects(
    () => {
      const { "dotnet-implementation": _drop, ...options } = fixture.options;
      return buildBscNativeEvmProverBundleFromArtifacts(options);
    },
    /dotnet implementation artifact requires --dotnet-implementation/u,
  );
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts({
        ...fixture.options,
        "audit-cross-sdk-fixture-parity": HASH_11,
    }),
    /auditHashes.cross_sdk_fixture_parity must match the artifact sha256/u,
  );

  const tinyProof = await writeNativeProverFixtureFiles({
    artifactByteOverrides: {
      proofArtifact: Buffer.alloc(256, 0xa7),
    },
  });
  await assert.rejects(
    () => buildBscNativeEvmProverBundleFromArtifacts(tinyProof.options),
    /proofArtifactBytes must be at least 65536 bytes/u,
  );

  const repeatedProof = await writeNativeProverFixtureFiles({
    artifactByteOverrides: {
      proofArtifact: Buffer.alloc(96 * 1024, 0xa7),
    },
  });
  await assert.rejects(
    () => buildBscNativeEvmProverBundleFromArtifacts(repeatedProof.options),
    /proof artifact looks like placeholder proof material: repeated 1-byte pattern/u,
  );

  const repeatedPattern = Buffer.alloc(96 * 1024);
  for (let index = 0; index < repeatedPattern.length; index += 1) {
    repeatedPattern[index] = index % 32;
  }
  const repeatedProvingKey = await writeNativeProverFixtureFiles({
    artifactByteOverrides: {
      provingKey: repeatedPattern,
    },
  });
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts(repeatedProvingKey.options),
    /proving key looks like placeholder proof material: repeated 32-byte pattern/u,
  );

  const arithmeticProof = Buffer.alloc(96 * 1024);
  for (let index = 0; index < arithmeticProof.length; index += 1) {
    arithmeticProof[index] = (index * 17 + 23) & 0xff;
  }
  const arithmeticProofFixture = await writeNativeProverFixtureFiles({
    artifactByteOverrides: {
      proofArtifact: arithmeticProof,
    },
  });
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts(
        arithmeticProofFixture.options,
      ),
    /proof artifact looks like placeholder proof material: arithmetic byte sequence with step 17/u,
  );

  const tinyImplementation = await writeNativeProverFixtureFiles({
    artifactByteOverrides: {
      dotnetImplementation: Buffer.alloc(256, 0xb8),
    },
  });
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts(tinyImplementation.options),
    /implementationBytes must be at least 1024 bytes/u,
  );

  const proofDrift = await writeNativeProverFixtureFiles({
    routeOverrides: {
      destinationRollout: {
        proofArtifactHash: HASH_77,
      },
    },
  });
  await assert.rejects(
    () => buildBscNativeEvmProverBundleFromArtifacts(proofDrift.options),
    /proof artifact hash does not match route\/deployment evidence/u,
  );

  const diagnosticRoute = await writeNativeProverFixtureFiles({
    routeOverrides: {
      destinationRollout: {
        verifierKeyHash: DIAGNOSTIC_BSC_VERIFIER_KEY_HASH,
      },
    },
  });
  await assert.rejects(
    () => buildBscNativeEvmProverBundleFromArtifacts(diagnosticRoute.options),
    /known diagnostic BSC verifier key hash/u,
  );
});

test("BSC route-config refuses production-ready manifests with disabled reasons", () => {
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        attachNativeProverBundle(
          routeManifest({
            productionReady: true,
            disabledReason: "operator left this route disabled",
            postDeployLiveEvidence: {
              fullTomlReady: true,
              offlineFullTomlSha256: HASH_33,
            },
          }),
        ),
      ),
    /productionReady cannot be true when disabledReason is set/u,
  );
});

test("BSC route-config refuses non-production manifests unless explicitly allowed", () => {
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(routeManifest(), {
        "allow-unready": "false",
      }),
    /allow-unready/u,
  );
  assert.match(
    buildBscTairaXorRouteConfigToml(routeManifest()),
    /allow_unready/u,
  );
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

test("BSC canonical production output guard rejects diagnostic or draft material", () => {
  const canonicalEvidencePath = `${CANONICAL_BSC_PRODUCTION_ARTIFACT_ROOT}/taira-bsc-xor-deployment.evidence.json`;
  assert.equal(isCanonicalBscProductionArtifactPath(canonicalEvidencePath), true);

  const diagnosticEvidence = buildDeploymentEvidence({
    tokenAddress: BSC_TOKEN_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    sourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
    verifierAddress: BSC_VERIFIER_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: DIAGNOSTIC_BSC_VERIFIER_KEY_HASH,
    readback: readyReadback({
      bridgeDestinationBindingHash: diagnosticBindingHash(),
      bridgeVerifierKeyHash: DIAGNOSTIC_BSC_VERIFIER_KEY_HASH,
    }),
  });
  assert.match(
    bscCanonicalProductionOutputProblems(
      canonicalEvidencePath,
      diagnosticEvidence,
      "BSC deployment evidence",
    ).join(" "),
    /known diagnostic BSC verifier key hash/u,
  );
  assert.deepEqual(
    bscCanonicalProductionOutputProblems(
      join(tmpdir(), "bsc-diagnostic-draft.json"),
      diagnosticEvidence,
      "BSC deployment evidence",
    ),
    [],
  );

  assert.match(
    bscCanonicalProductionOutputProblems(
      `${CANONICAL_BSC_PRODUCTION_ARTIFACT_ROOT}/taira-bsc-xor-route.manifest.json`,
      routeManifest(),
      "BSC route manifest",
    ).join(" "),
    /not productionReady true|disabledReason|nativeEvmProverBundle/u,
  );
  assert.deepEqual(
    bscCanonicalProductionOutputProblems(
      `${CANONICAL_BSC_PRODUCTION_ARTIFACT_ROOT}/taira-bsc-xor-route.manifest.json`,
      productionReadyRouteManifest(),
      "BSC route manifest",
    ),
    [],
  );

  const canonicalBundlePath = `${CANONICAL_BSC_PRODUCTION_ARTIFACT_ROOT}/bsc-testnet-native-evm-prover-bundle.json`;
  const productionBundle = productionReadyRouteManifest().nativeEvmProverBundle;
  assert.deepEqual(
    bscCanonicalProductionOutputProblems(
      canonicalBundlePath,
      productionBundle,
      "BSC native EVM prover bundle",
    ),
    [],
  );
  assert.match(
    bscCanonicalProductionOutputProblems(
      canonicalBundlePath,
      {
        ...productionBundle,
        verifier_key_hash: DIAGNOSTIC_BSC_VERIFIER_KEY_HASH,
      },
      "BSC native EVM prover bundle",
    ).join(" "),
    /known diagnostic BSC verifier key hash/u,
  );
  const { native_sdk_artifacts: _dropSdkArtifacts, ...incompleteBundle } =
    productionBundle;
  assert.match(
    bscCanonicalProductionOutputProblems(
      canonicalBundlePath,
      incompleteBundle,
      "BSC native EVM prover bundle",
    ).join(" "),
    /nativeSdkArtifacts/u,
  );
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
    () =>
      buildMergedBscTairaXorRouteConfigToml(
        "[[zk.sccp_route_manifests]]\n",
        routeManifest(),
      ),
    /already contains/u,
  );
});

test("BSC route-config rejects malformed or foreign route manifests", () => {
  const cases = [
    [{ routeId: "taira_tron_xor" }, /routeId/u],
    [{ routeId: " taira_bsc_xor" }, /routeId.*canonical string/u],
    [{ assetKey: "dot" }, /assetKey/u],
    [{ assetKey: "xor " }, /assetKey.*canonical string/u],
    [{ bscNetwork: "BSC-TESTNET" }, /bscNetwork.*canonical lowercase text/u],
    [{ bscNetwork: "bsc_testnet" }, /bscNetwork.*canonical lowercase text/u],
    [{ chain: "bsc-mainnet" }, /chain/u],
    [{ chain: "BSC-TESTNET" }, /chain.*canonical lowercase text/u],
    [{ chainIdHex: "0x38" }, /chainIdHex/u],
    [{ chainIdHex: "0X61" }, /chainIdHex.*canonical lowercase hex/u],
    [{ networkIdHex: `0x${"38".padStart(64, "0")}` }, /networkIdHex/u],
    [{ networkIdHex: ` ${BSC_TESTNET_NETWORK_ID_HEX}` }, /networkIdHex.*canonical string/u],
    [{ networkIdHex: BSC_TESTNET_NETWORK_ID_HEX.toUpperCase() }, /networkIdHex.*canonical lowercase hex/u],
    [{ destinationBinding: { networkIdHex: HASH_33 } }, /networkIdHex.*aliases disagree/u],
    [{ counterpartyDomain: 1 }, /counterpartyDomain/u],
    [{ verifierTarget: "TronContract" }, /verifierTarget/u],
    [
      { bscTokenAddress: BSC_TOKEN_ADDRESS.toUpperCase() },
      /token address.*canonical lowercase hex/u,
    ],
    [
      {
        destinationRollout: {
          destinationBridgeAddress: BSC_BRIDGE_ADDRESS.toUpperCase(),
        },
      },
      /bridge address.*canonical lowercase hex/u,
    ],
    [
      { sccpBscSourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS.toUpperCase() },
      /source bridge address.*canonical lowercase hex/u,
    ],
    [
      { bscVerifierAddress: BSC_VERIFIER_ADDRESS.replace(/^0x/u, "0X") },
      /verifier address.*canonical lowercase hex/u,
    ],
    [
      {
        destinationRollout: {
          verifierIdentity: BSC_VERIFIER_ADDRESS.toUpperCase(),
        },
      },
      /verifier address.*canonical lowercase hex/u,
    ],
    [{ bscBridgeAddress: BSC_TOKEN_ADDRESS }, /bridge address aliases disagree|distinct/u],
    [{ tokenAddress: BSC_SOURCE_BRIDGE_ADDRESS }, /token address aliases disagree/u],
    [{ bridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS }, /bridge address aliases disagree/u],
    [
      { destinationRollout: { destinationBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS } },
      /bridge address aliases disagree/u,
    ],
    [{ destinationRollout: { targetDomain: 1 } }, /SORA -> BSC/u],
    [
      { destinationRollout: { verifierBackend: "tron-groth16-bn254-v1" } },
      /verifier backend/u,
    ],
    [{ verifierCodeHash: HASH_77 }, /verifierCodeHash aliases disagree/u],
    [{ destinationRollout: { verifierCodeHash: HASH_44.toUpperCase() } }, /verifierCodeHash.*canonical lowercase hex/u],
    [{ verifierKeyHash: HASH_77 }, /verifierKeyHash aliases disagree/u],
    [{ destinationRollout: { verifierKeyHash: HASH_55.toUpperCase() } }, /verifierKeyHash.*canonical lowercase hex/u],
    [
      { destinationRollout: { destinationBindingHash: HASH_33 } },
      /binding hash/u,
    ],
    [
      { destinationRollout: { destinationBindingHash: routeManifest().destinationRollout.destinationBindingHash.toUpperCase() } },
      /destination binding hash.*canonical lowercase hex/u,
    ],
    [{ destinationBindingHash: HASH_77 }, /destination binding hash aliases disagree/u],
    [{ destinationBindingKey: "stale-binding-key" }, /destination binding key aliases disagree/u],
    [{ proofArtifactHash: HASH_77 }, /proofArtifactHash aliases disagree/u],
    [{ provingKeyHash: HASH_77 }, /provingKeyHash aliases disagree/u],
    [
      { destinationRollout: { proofArtifactHash: undefined } },
      /supplied together/u,
    ],
    [
      {
        productionReady: true,
        destinationRollout: {
          proofArtifactHash: undefined,
          provingKeyHash: undefined,
        },
      },
      /productionReady requires proofArtifactHash and provingKeyHash/u,
    ],
    [
      { destinationRollout: { provingKeyHash: HASH_22 } },
      /provingKeyHash must not equal verifierKeyHash/u,
    ],
    [{ tairaXorBurnRecord: { artifactSha256: HASH_33 } }, /artifact sha256/u],
    [
      { tairaXorBurnRecord: { settlementAssetDefinitionId: "xor#universal" } },
      /Base58|alias/u,
    ],
    [
      { sourceBridgeAddress: BSC_BRIDGE_ADDRESS },
      /source bridge address aliases disagree/u,
    ],
    [
      { destinationVerifierAddress: BSC_BRIDGE_ADDRESS },
      /verifier address aliases disagree/u,
    ],
    [
      { postDeployLiveEvidence: { full_toml_ready: true } },
      /fullTomlReady aliases disagree/u,
    ],
    [
      { postDeployLiveEvidence: { source_bridge_config_hash: HASH_77 } },
      /sourceBridgeConfigHash aliases disagree/u,
    ],
    [
      { postDeployLiveEvidence: { sourceEventTransactionId: ` ${HASH_55}` } },
      /sourceEventTransactionId.*canonical string/u,
    ],
    [
      { postDeployLiveEvidence: { sourceEventTransactionId: HASH_55.toUpperCase() } },
      /sourceEventTransactionId.*canonical lowercase hex/u,
    ],
    [
      { postDeployLiveEvidence: { offlineFullTomlSha256: `${HASH_33} ` } },
      /offlineFullTomlSha256.*canonical string/u,
    ],
    [
      { postDeployLiveEvidence: { offlineFullTomlSha256: HASH_33.toUpperCase() } },
      /offlineFullTomlSha256.*canonical lowercase hex/u,
    ],
    [
      { postDeployLiveEvidence: { sourceEventTransactionUrl: ROUTE_CANARY_EXPLORER_URL } },
      /sourceEventExplorerUrl|transaction hash/u,
    ],
    [
      { postDeployLiveEvidence: { post_deploy_production_blockers: [""] } },
      /post_deploy_production_blockers\[0\] must be a non-empty canonical string/u,
    ],
    [{ secret_key: "0xabc" }, /private key|secrets/u],
  ];

  for (const [overrides, reason] of cases) {
    assert.throws(
      () =>
        buildBscTairaXorRouteConfigToml(routeManifest(overrides), {
          "allow-unready": "true",
        }),
      reason,
    );
  }
});

test("BSC route-config command writes an operator overlay", async () => {
  const dir = await mkdtemp(join(tmpdir(), "iroha-bsc-route-config-"));
  const manifestPath = join(dir, "manifest.json");
  const out = join(dir, "route.toml");
  await writeFile(
    manifestPath,
    `${JSON.stringify(routeManifest(), null, 2)}\n`,
  );

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
  assert.match(
    toml,
    /source_bridge_address = "0x3333333333333333333333333333333333333333"/u,
  );
  assert.match(
    toml,
    /destination_verifier_address = "0x4444444444444444444444444444444444444444"/u,
  );
  assert.match(
    toml,
    /tron_verifier_address = "0x4444444444444444444444444444444444444444"/u,
  );
});

test("BSC route-config command refuses draft manifests in the canonical default output", async () => {
  const dir = await mkdtemp(join(tmpdir(), "iroha-bsc-route-config-default-"));
  const manifestPath = join(dir, "manifest.json");
  await writeFile(
    manifestPath,
    `${JSON.stringify(routeManifest(), null, 2)}\n`,
  );

  await assert.rejects(
    () =>
      main([
        "route-config",
        "--manifest",
        manifestPath,
        "--allow-unready",
        "true",
      ]),
    /cannot be written to canonical BSC production artifact path.*not productionReady true/u,
  );
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
  const dir = await mkdtemp(join(tmpdir(), "bsc-deploy-test-"));
  const verifierFile = join(dir, "verifier.json");
  await writeFile(verifierFile, JSON.stringify(verifierMaterial()), "utf8");
  try {
    delete process.env[envName];
    await assert.rejects(
      () =>
        main([
          "deploy",
          "--verifier",
          verifierFile,
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
          verifierFile,
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

test("BSC deploy command refuses smoke-test verifier material before signer or RPC use", async () => {
  const dir = await mkdtemp(join(tmpdir(), "bsc-smoke-fixture-"));
  const verifierFile = join(dir, "verifier.json");
  await writeFile(
    verifierFile,
    JSON.stringify(
      verifierMaterial({
        alpha1: SMOKE_FIXTURE_G1,
        beta2: SMOKE_FIXTURE_G2,
        gamma2: SMOKE_FIXTURE_G2,
        delta2: SMOKE_FIXTURE_G2,
        ic: SMOKE_FIXTURE_IC,
      }),
    ),
    "utf8",
  );

  await assert.rejects(
    () =>
      main([
        "deploy",
        "--verifier",
        verifierFile,
        "--broadcast",
        "true",
        "--confirm-testnet",
        "taira_bsc_xor",
        "--allow-diagnostic-verifier",
        "true",
      ]),
    /refuses deterministic smoke-test Groth16 fixture/u,
  );
});

test("BSC deployment helper self-test covers public evidence and secret scanning", async () => {
  assert.deepEqual(await main(["self-test"]), { ok: true });
});
