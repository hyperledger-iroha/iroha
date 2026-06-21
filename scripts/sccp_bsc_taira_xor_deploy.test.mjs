import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  mkdir,
  mkdtemp,
  readFile,
  symlink,
  truncate,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
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
  PRODUCTION_REQUIREMENTS_SCHEMA,
  ROUTE_MANIFEST_SCHEMA,
  SCCP_BSC_BINARY_ARTIFACT_INPUT_MAX_BYTES,
  SCCP_BSC_JSON_INPUT_MAX_BYTES,
  SCCP_BSC_TEXT_INPUT_MAX_BYTES,
  SCCP_BSC_DIAGNOSTIC_VERIFIER_KEY_HASHES,
  SCCP_DOMAIN_BSC,
  SCCP_DOMAIN_SORA,
  bscCanonicalProductionOutputProblems,
  buildBscTairaXorRouteManifestDraft,
  bscGroth16VerifierKeyHash,
  canonicalBscNativeEvmProverBundleHash,
  bscDestinationBindingHash,
  bscDestinationBindingKey,
  buildBscNativeEvmProverBundleFromArtifacts,
  buildBscTairaXorRouteConfigToml,
  buildDeploymentEvidence,
  buildMergedBscTairaXorRouteConfigToml,
  bscProductionRequirements,
  main,
  isKnownDiagnosticBscVerifierKeyHash,
  isCanonicalBscProductionArtifactPath,
  isSmokeFixtureGroth16VerifierMaterial,
  normalizeBscRpcUrl,
  parseJsonWithoutDuplicateKeys,
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
const HASH_88 = `0x${"88".repeat(32)}`;
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
const BN254_BASE_FIELD_MODULUS =
  "21888242871839275222246405745257275088696311157297823662689037894645226208583";
const VALID_G1_POINTS = Object.freeze([
  ["1", "2"],
  [
    "1368015179489954701390400359078579693043519447331113978918064868415326638035",
    "9918110051302171585080402603319702774565515993150576347155970296011118125764",
  ],
  [
    "3353031288059533942658390886683067124040920775575537747144343083137631628272",
    "19321533766552368860946552437480515441416830039777911637913418824951667761761",
  ],
  [
    "3010198690406615200373504922352659861758983907867017329644089018310584441462",
    "4027184618003122424972590350825261965929648733675738730716654005365300998076",
  ],
  [
    "10744596414106452074759370245733544594153395043370666422502510773307029471145",
    "848677436511517736191562425154572367705380862894644942948681172815252343932",
  ],
  [
    "4503322228978077916651710446042370109107355802721800704639343137502100212473",
    "6132642251294427119375180147349983541569387941788025780665104001559216576968",
  ],
  [
    "10415861484417082502655338383609494480414113902179649885744799961447382638712",
    "10196215078179488638353184030336251401353352596818396260819493263908881608606",
  ],
  [
    "3932705576657793550893430333273221375907985235130430286685735064194643946083",
    "18813763293032256545937756946359266117037834559191913266454084342712532869153",
  ],
  [
    "1624070059937464756887933993293429854168590106605707304006200119738501412969",
    "3269329550605213075043232856820720631601935657990457502777101397807070461336",
  ],
  [
    "4444740815889402603535294170722302758225367627362056425101568584910268024244",
    "10537263096529483164618820017164668921386457028564663708352735080900270541420",
  ],
  [
    "19033251874843656108471242320417533909414939332036131356573128480367742634479",
    "20792135454608030201903199625673964159744755218442260092768620403349374102584",
  ],
]);
const VALID_IC = VALID_G1_POINTS.slice(1, 11).flat();
const SCALAR_FIELD_ONLY_G1_POINT = Object.freeze([
  "9576106256429682909732802513550057851239909425182015025367964331626916216831",
  "3762041743597375428823600987466094155844250131321505902823128844567717085184",
]);
const SCALAR_FIELD_ONLY_G2_POINT = Object.freeze(["1", "2", "3", "4"]);
const deterministicBytes = (label, length) => {
  const chunks = [];
  let index = 0;
  while (Buffer.concat(chunks).length < length) {
    chunks.push(createHash("sha256").update(`${label}:${index}`).digest());
    index += 1;
  }
  return Buffer.concat(chunks).subarray(0, length);
};
const BURN_RECORD_BYTES = deterministicBytes(
  "bsc taira xor production burn-record artifact",
  768,
);
const FIXTURE_BURN_RECORD_BYTES = Buffer.from(
  "bsc taira xor burn-record artifact fixture for route-config tests",
  "utf8",
);
const BURN_RECORD_B64 = BURN_RECORD_BYTES.toString("base64");
const BURN_RECORD_SHA256 = `0x${createHash("sha256").update(BURN_RECORD_BYTES).digest("hex")}`;
const sha256Hex = (bytes) =>
  `0x${createHash("sha256").update(bytes).digest("hex")}`;
const fixtureHash = (label) => sha256Hex(Buffer.from(label, "utf8"));

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
    proof_artifact: "artifacts/bsc-testnet/proof-artifact.r1cs",
    proof_artifact_hash: proofArtifactHash,
    proving_key: "artifacts/bsc-testnet/proving-key.zkey",
    proving_key_hash: provingKeyHash,
    verifier_key: "artifacts/bsc-testnet/verifier-key.bin",
    verifier_key_hash: destinationRollout.verifierKeyHash,
    verifier_key_artifact_hash: HASH_88,
    destination_binding_hash: destinationRollout.destinationBindingHash,
    no_wasm: true,
    remote_prover_required: false,
    browser_implementation: "pure-typescript",
    cross_sdk_fixture_parity_artifact:
      "artifacts/bsc-testnet/cross-sdk-parity.json",
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
      circuit_security_audit: fixtureHash("route circuit security audit"),
      native_implementation_audit: fixtureHash(
        "route native implementation audit",
      ),
      reproducible_build_attestation: fixtureHash(
        "route reproducible build attestation",
      ),
      cross_sdk_fixture_parity: fixtureHash("route cross-SDK fixture parity"),
      native_prover_self_test: fixtureHash("route native prover self-test"),
      no_wasm_no_remote_scan: fixtureHash("route no-wasm no-remote scan"),
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
const accessorBackedRecord = (keys, value = "getter-value") => {
  let reads = 0;
  const record = {};
  for (const key of keys) {
    Object.defineProperty(record, key, {
      enumerable: true,
      get() {
        reads += 1;
        return value;
      },
    });
  }
  return {
    record,
    readCount: () => reads,
  };
};

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
  verifierKeyHash: HASH_22,
  bridgeDestinationBindingHash: bindingHash(),
  bridgeVerifierAddress: BSC_VERIFIER_ADDRESS,
  bridgeVerifierCodeHash: HASH_11,
  bridgeVerifierKeyHash: HASH_22,
  bridgeNetworkId: BSC_TESTNET_NETWORK_ID_HEX,
  bridgeSourceDomain: SCCP_DOMAIN_SORA,
  bridgeTargetDomain: SCCP_DOMAIN_BSC,
  ...overrides,
});

const verifierMaterial = (overrides = {}) => {
  const material = {
    alpha1: VALID_G1_POINTS[0],
    beta2: SMOKE_FIXTURE_G2,
    gamma2: SMOKE_FIXTURE_G2,
    delta2: SMOKE_FIXTURE_G2,
    ic: VALID_IC,
    proofFamily: "stark-fri-v1",
    networkId: BSC_TESTNET_NETWORK_ID_HEX,
    sourceDomain: 0,
    targetDomain: 2,
    ...overrides,
  };
  const expectedVerifierKeyHash =
    overrides.expectedVerifierKeyHash ??
    overrides.verifierKeyHash ??
    overrides.verifyingKeyHash ??
    bscGroth16VerifierKeyHash(material);
  return {
    ...material,
    expectedVerifierKeyHash,
    verifierKeyHash: expectedVerifierKeyHash,
  };
};

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
    explorerUrl: "https://testnet.bscscan.com",
    explorerHost: "testnet.bscscan.com",
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

const tairaBurnRecordContract = (overrides = {}) => ({
  schema: "iroha-sccp-taira-xor-burn-record-contract/v1",
  route_id: "taira_bsc_xor",
  asset_key: "xor",
  artifact_b64: BURN_RECORD_B64,
  artifact_sha256: BURN_RECORD_SHA256,
  code_hash: HASH_33,
  vk_ref: {
    backend: "halo2_ipa",
    name: "taira_bsc_xor_burn_record_v1",
  },
  ...overrides,
});

const tairaBurnRecordContractWithBytes = (bytes, overrides = {}) =>
  tairaBurnRecordContract({
    artifact_b64: Buffer.from(bytes).toString("base64"),
    artifact_sha256: `0x${createHash("sha256").update(bytes).digest("hex")}`,
    ...overrides,
  });

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

const offlineFullTomlEvidence = (overrides = {}) => ({
  schema: "iroha-sccp-bsc-taira-xor-offline-full-toml-evidence/v1",
  routeId: "taira_bsc_xor",
  assetKey: "xor",
  bscNetwork: "testnet",
  chain: "bsc-testnet",
  chainIdHex: "0x61",
  networkIdHex: BSC_TESTNET_NETWORK_ID_HEX,
  fullTomlReady: true,
  offlineFullTomlSha256: HASH_88,
  hashMode:
    "sha256:merged-full-config-without-post_deploy_offline_full_toml_sha256",
  hashInputSha256: HASH_88,
  renderedTomlSha256: HASH_66,
  postDeployLiveEvidence: {
    fullTomlReady: true,
    offlineFullTomlSha256: HASH_88,
  },
  ...overrides,
});

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
    production_attestation_hash: fixtureHash(
      "BSC script native prover parity production attestation",
    ),
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
    production_attestation_hash: fixtureHash(
      "BSC script native prover self-test production attestation",
    ),
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
  const snarkjsBytes = (magic, sectionCount, bytes) => {
    const out = Buffer.from(bytes);
    const headerBytes = 12;
    const sectionHeaderBytes = sectionCount * 12;
    const payloadBytes = out.length - headerBytes - sectionHeaderBytes;
    if (payloadBytes < sectionCount) {
      throw new Error("snarkjs test fixture is too small");
    }
    out.set(Buffer.from(magic, "ascii"), 0);
    out.writeUInt32LE(1, 4);
    out.writeUInt32LE(sectionCount, 8);
    let offset = headerBytes;
    for (let index = 0; index < sectionCount; index += 1) {
      const sectionSize =
        Math.floor(payloadBytes / sectionCount) +
        (index < payloadBytes % sectionCount ? 1 : 0);
      out.writeUInt32LE(index + 1, offset);
      out.writeUInt32LE(sectionSize, offset + 4);
      out.writeUInt32LE(0, offset + 8);
      offset += 12 + sectionSize;
    }
    if (offset !== out.length) {
      throw new Error("snarkjs test fixture sections do not fill the file");
    }
    return out;
  };
  const proofBytes =
    artifactByteOverrides.proofArtifact ??
    artifactByteOverrides.proof ??
    snarkjsBytes("r1cs", 3, bytesFor("proof-artifact", 96 * 1024));
  const provingKeyBytes =
    artifactByteOverrides.provingKey ??
    snarkjsBytes("zkey", 10, bytesFor("proving-key", 96 * 1024));
  const verifierKeyMaterial = verifierMaterial(
    artifactByteOverrides.verifierMaterial ?? {},
  );
  const verifierKeyBytes =
    artifactByteOverrides.verifierKey ??
    Buffer.from(`${JSON.stringify(verifierKeyMaterial, null, 2)}\n`, "utf8");
  const proofArtifactHash = sha256Hex(proofBytes);
  const provingKeyHash = sha256Hex(provingKeyBytes);
  const verifierKeyHash = bscGroth16VerifierKeyHash(verifierKeyMaterial);
  const verifierKeyArtifactHash = sha256Hex(verifierKeyBytes);
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
    verifierKeyArtifactHash,
    destinationBindingHash,
  };
  const parityBytes = Buffer.from(
    `${JSON.stringify(nativeProverParityFixture(bundleBinding), null, 2)}\n`,
  );
  const selfTestBytes = Buffer.from(
    `${JSON.stringify(nativeProverSelfTestFixture(bundleBinding), null, 2)}\n`,
  );
  await writeArtifact("proof-artifact.r1cs", proofBytes);
  await writeArtifact("proving-key.zkey", provingKeyBytes);
  await writeArtifact("verifier-key.json", verifierKeyBytes);
  await writeArtifact("cross-sdk-parity.json", parityBytes);
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
    verifierKeyArtifactHash,
    destinationBindingHash,
    sdkImplementationPaths,
    options: {
      "route-manifest": routeManifestPath,
      "artifact-root": artifactRoot,
      "proof-artifact": "proof-artifact.r1cs",
      "proving-key": "proving-key.zkey",
      "verifier-key": "verifier-key.json",
      "cross-sdk-parity": "cross-sdk-parity.json",
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

test("BSC deployment binding helpers ignore accessor-backed options", () => {
  const { record, readCount } = accessorBackedRecord([
    "networkId",
    "verifierAddress",
    "bridgeAddress",
    "verifierCodeHash",
    "verifierKeyHash",
  ]);

  assert.throws(
    () => bscDestinationBindingHash(record),
    /BSC verifier address/u,
  );
  assert.throws(
    () => bscDestinationBindingKey(record),
    /BSC verifier address/u,
  );
  assert.equal(readCount(), 0);
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
  assert.equal(evidence.bscContractReadback.verifierKeyHash, HASH_22);
  assert.doesNotMatch(
    JSON.stringify(evidence),
    /private[_-]?key|mnemonic|seed/iu,
  );
});

test("BSC deployment evidence ignores accessor-backed public inputs", () => {
  const deploymentInput = accessorBackedRecord([
    "tokenAddress",
    "bridgeAddress",
    "sourceBridgeAddress",
    "verifierAddress",
    "verifierCodeHash",
    "verifierKeyHash",
    "readback",
    "bscNetwork",
  ]);
  assert.throws(
    () => buildDeploymentEvidence(deploymentInput.record),
    /token address/u,
  );
  assert.equal(deploymentInput.readCount(), 0);

  const readbackInput = accessorBackedRecord([
    "addresses",
    "readback",
    "bindingHash",
    "verifierCodeHash",
    "verifierKeyHash",
    "bscNetwork",
  ]);
  assert.throws(
    () => validateBscReadbackEvidence(readbackInput.record),
    /BSC contract readback must be an object/u,
  );
  assert.equal(readbackInput.readCount(), 0);

  let nestedReads = 0;
  const readback = readyReadback();
  Object.defineProperty(readback, "codePresent", {
    enumerable: true,
    get() {
      nestedReads += 1;
      return { token: true, bridge: true, sourceBridge: true, verifier: true };
    },
  });
  assert.throws(
    () =>
      validateBscReadbackEvidence({
        addresses,
        readback,
        bindingHash: bindingHash(),
        verifierCodeHash: HASH_11,
        verifierKeyHash: HASH_22,
      }),
    /token bytecode/u,
  );
  assert.equal(nestedReads, 0);
});

test("BSC route-manifest command binds deployment evidence and TAIRA burn-record material", async () => {
  const dir = await mkdtemp(join(tmpdir(), "iroha-bsc-route-manifest-"));
  const evidencePath = join(dir, "deployment.evidence.json");
  const contractPath = join(dir, "burn-record.contract.json");
  const out = join(dir, "route.manifest.json");
  const evidence = buildDeploymentEvidence({
    tokenAddress: BSC_TOKEN_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    sourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
    verifierAddress: BSC_VERIFIER_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: HASH_22,
    readback: readyReadback(),
  });
  await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);
  await writeFile(
    contractPath,
    `${JSON.stringify(tairaBurnRecordContract(), null, 2)}\n`,
  );

  const result = await main([
    "route-manifest",
    "--evidence",
    evidencePath,
    "--taira-contract",
    contractPath,
    "--settlement-asset-definition-id",
    "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
    "--proof-artifact-hash",
    HASH_44,
    "--proving-key-hash",
    HASH_55,
    "--out",
    out,
  ]);
  const manifest = JSON.parse(await readFile(out, "utf8"));

  assert.equal(result.ok, true);
  assert.equal(manifest.schema, ROUTE_MANIFEST_SCHEMA);
  assert.equal(manifest.routeId, "taira_bsc_xor");
  assert.equal(manifest.bscNetwork, "testnet");
  assert.equal(manifest.productionReady, false);
  assert.equal(manifest.bscBridgeAddress, BSC_BRIDGE_ADDRESS);
  assert.equal(manifest.bscTokenAddress, BSC_TOKEN_ADDRESS);
  assert.equal(manifest.bscVerifierAddress, BSC_VERIFIER_ADDRESS);
  assert.equal(manifest.proofArtifactHash, HASH_44);
  assert.equal(manifest.provingKeyHash, HASH_55);
  assert.equal(
    manifest.destinationRollout.destinationBindingHash,
    bindingHash(),
  );
  assert.equal(manifest.tairaXorBurnRecord.artifactSha256, BURN_RECORD_SHA256);
  assert.deepEqual(manifest.tairaXorBurnRecord.vkRef, {
    backend: "halo2_ipa",
    name: "taira_bsc_xor_burn_record_v1",
  });
  assert.doesNotMatch(
    JSON.stringify(manifest),
    /private[_-]?key|mnemonic|seed/iu,
  );
});

test("BSC route-manifest command builds production-ready manifests only with bound native and post-deploy evidence", async () => {
  const dir = await mkdtemp(join(tmpdir(), "iroha-bsc-route-manifest-ready-"));
  const evidencePath = join(dir, "deployment.evidence.json");
  const contractPath = join(dir, "burn-record.contract.json");
  const bundlePath = join(dir, "native-prover-bundle.json");
  const fullTomlEvidencePath = join(dir, "full-config.evidence.json");
  const out = join(dir, "route.manifest.json");
  const evidence = buildDeploymentEvidence({
    tokenAddress: BSC_TOKEN_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    sourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
    verifierAddress: BSC_VERIFIER_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: HASH_22,
    readback: readyReadback(),
  });
  const bundle = nativeProverBundleForRollout(
    routeManifest().destinationRollout,
  );
  await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);
  await writeFile(
    contractPath,
    `${JSON.stringify(tairaBurnRecordContract(), null, 2)}\n`,
  );
  await writeFile(bundlePath, `${JSON.stringify(bundle, null, 2)}\n`);
  await writeFile(
    fullTomlEvidencePath,
    `${JSON.stringify(offlineFullTomlEvidence(), null, 2)}\n`,
  );

  const result = await main([
    "route-manifest",
    "--evidence",
    evidencePath,
    "--taira-contract",
    contractPath,
    "--settlement-asset-definition-id",
    "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
    "--native-prover-bundle",
    bundlePath,
    "--source-bridge-config-hash",
    HASH_33,
    "--source-event-transaction-id",
    HASH_55,
    "--source-event-explorer-url",
    SOURCE_EVENT_EXPLORER_URL,
    "--route-canary-evidence-hash",
    HASH_66,
    "--route-canary-transaction-id",
    HASH_77,
    "--route-canary-explorer-url",
    ROUTE_CANARY_EXPLORER_URL,
    "--offline-full-toml-evidence",
    fullTomlEvidencePath,
    "--production-ready",
    "true",
    "--live-readback-checked",
    "true",
    "--confirm-testnet",
    "taira_bsc_xor",
    "--out",
    out,
  ]);
  const manifest = JSON.parse(await readFile(out, "utf8"));
  const expectedBundleHash = canonicalBscNativeEvmProverBundleHash(
    validateBscTestnetNativeEvmProverBundle(bundle, {
      expectedDestinationBindingHash: bindingHash(),
    }),
  );

  assert.equal(result.productionReady, true);
  assert.equal(manifest.productionReady, true);
  assert.equal(manifest.disabledReason, undefined);
  assert.equal(manifest.postDeployReadbackChecked, true);
  assert.equal(manifest.proofArtifactHash, HASH_44);
  assert.equal(manifest.provingKeyHash, HASH_55);
  assert.equal(manifest.nativeEvmProverBundleHash, expectedBundleHash);
  assert.equal(
    manifest.destinationRollout.nativeEvmProverBundleHash,
    expectedBundleHash,
  );
  assert.equal(
    manifest.postDeployLiveEvidence.offlineFullTomlSha256,
    hex32("88"),
  );

  await assert.rejects(
    () =>
      main([
        "route-manifest",
        "--evidence",
        evidencePath,
        "--taira-contract",
        contractPath,
        "--settlement-asset-definition-id",
        "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
        "--native-prover-bundle",
        bundlePath,
        "--source-bridge-config-hash",
        HASH_33,
        "--source-event-transaction-id",
        HASH_55,
        "--source-event-explorer-url",
        SOURCE_EVENT_EXPLORER_URL,
        "--route-canary-evidence-hash",
        HASH_66,
        "--route-canary-transaction-id",
        HASH_77,
        "--route-canary-explorer-url",
        ROUTE_CANARY_EXPLORER_URL,
        "--full-toml-ready",
        "true",
        "--offline-full-toml-sha256",
        hex32("88"),
        "--production-ready",
        "true",
        "--live-readback-checked",
        "true",
        "--confirm-testnet",
        "taira_bsc_xor",
        "--out",
        join(dir, "route.raw-hash-only.manifest.json"),
      ]),
    /production-ready BSC route manifests require --offline-full-toml-evidence/u,
  );
});

test("BSC route-manifest command accepts generated offline full TOML evidence", async () => {
  const dir = await mkdtemp(
    join(tmpdir(), "iroha-bsc-route-manifest-full-evidence-"),
  );
  const evidencePath = join(dir, "deployment.evidence.json");
  const contractPath = join(dir, "burn-record.contract.json");
  const bundlePath = join(dir, "native-prover-bundle.json");
  const fullTomlEvidencePath = join(dir, "full-config.evidence.json");
  const out = join(dir, "route.manifest.json");
  const evidence = buildDeploymentEvidence({
    tokenAddress: BSC_TOKEN_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    sourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
    verifierAddress: BSC_VERIFIER_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: HASH_22,
    bscNetwork: "testnet",
    readback: readyReadback(),
  });
  const bundle = nativeProverBundleForRollout(
    routeManifest().destinationRollout,
  );
  await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);
  await writeFile(
    contractPath,
    `${JSON.stringify(tairaBurnRecordContract(), null, 2)}\n`,
  );
  await writeFile(bundlePath, `${JSON.stringify(bundle, null, 2)}\n`);
  await writeFile(
    fullTomlEvidencePath,
    `${JSON.stringify(offlineFullTomlEvidence(), null, 2)}\n`,
  );

  const result = await main([
    "route-manifest",
    "--evidence",
    evidencePath,
    "--taira-contract",
    contractPath,
    "--settlement-asset-definition-id",
    "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
    "--native-prover-bundle",
    bundlePath,
    "--source-bridge-config-hash",
    HASH_33,
    "--source-event-transaction-id",
    HASH_55,
    "--source-event-explorer-url",
    SOURCE_EVENT_EXPLORER_URL,
    "--route-canary-evidence-hash",
    HASH_66,
    "--route-canary-transaction-id",
    HASH_77,
    "--route-canary-explorer-url",
    ROUTE_CANARY_EXPLORER_URL,
    "--offline-full-toml-evidence",
    fullTomlEvidencePath,
    "--production-ready",
    "true",
    "--live-readback-checked",
    "true",
    "--confirm-testnet",
    "taira_bsc_xor",
    "--out",
    out,
  ]);
  const manifest = JSON.parse(await readFile(out, "utf8"));

  assert.equal(result.productionReady, true);
  assert.equal(result.offlineFullTomlSha256, HASH_88);
  assert.equal(manifest.postDeployLiveEvidence.fullTomlReady, true);
  assert.equal(manifest.postDeployLiveEvidence.offlineFullTomlSha256, HASH_88);

  await assert.rejects(
    () =>
      main([
        "route-manifest",
        "--evidence",
        evidencePath,
        "--taira-contract",
        contractPath,
        "--settlement-asset-definition-id",
        "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
        "--native-prover-bundle",
        bundlePath,
        "--source-bridge-config-hash",
        HASH_33,
        "--source-event-transaction-id",
        HASH_55,
        "--source-event-explorer-url",
        SOURCE_EVENT_EXPLORER_URL,
        "--route-canary-evidence-hash",
        HASH_66,
        "--route-canary-transaction-id",
        HASH_77,
        "--route-canary-explorer-url",
        ROUTE_CANARY_EXPLORER_URL,
        "--offline-full-toml-evidence",
        fullTomlEvidencePath,
        "--offline-full-toml-sha256",
        HASH_77,
        "--production-ready",
        "true",
        "--live-readback-checked",
        "true",
        "--confirm-testnet",
        "taira_bsc_xor",
        "--out",
        join(dir, "route.bad.manifest.json"),
      ]),
    /--offline-full-toml-sha256 disagrees with --offline-full-toml-evidence/u,
  );
});

test("BSC route-manifest command rejects ambiguous offline full TOML evidence", async () => {
  const dir = await mkdtemp(
    join(tmpdir(), "iroha-bsc-route-manifest-full-evidence-alias-"),
  );
  const evidencePath = join(dir, "deployment.evidence.json");
  const contractPath = join(dir, "burn-record.contract.json");
  const bundlePath = join(dir, "native-prover-bundle.json");
  const evidence = buildDeploymentEvidence({
    tokenAddress: BSC_TOKEN_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    sourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
    verifierAddress: BSC_VERIFIER_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: HASH_22,
    bscNetwork: "testnet",
    readback: readyReadback(),
  });
  const bundle = nativeProverBundleForRollout(
    routeManifest().destinationRollout,
  );
  await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);
  await writeFile(
    contractPath,
    `${JSON.stringify(tairaBurnRecordContract(), null, 2)}\n`,
  );
  await writeFile(bundlePath, `${JSON.stringify(bundle, null, 2)}\n`);
  const baseArgs = [
    "route-manifest",
    "--evidence",
    evidencePath,
    "--taira-contract",
    contractPath,
    "--settlement-asset-definition-id",
    "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
    "--native-prover-bundle",
    bundlePath,
    "--source-bridge-config-hash",
    HASH_33,
    "--source-event-transaction-id",
    HASH_55,
    "--source-event-explorer-url",
    SOURCE_EVENT_EXPLORER_URL,
    "--route-canary-evidence-hash",
    HASH_66,
    "--route-canary-transaction-id",
    HASH_77,
    "--route-canary-explorer-url",
    ROUTE_CANARY_EXPLORER_URL,
    "--production-ready",
    "true",
    "--live-readback-checked",
    "true",
    "--confirm-testnet",
    "taira_bsc_xor",
  ];

  for (const [name, fullTomlEvidence, pattern] of [
    [
      "duplicate BSC network aliases",
      { ...offlineFullTomlEvidence(), bsc_network: "testnet" },
      /BSC offline full TOML evidence network must not use multiple aliases/u,
    ],
    [
      "generic network alias",
      { ...offlineFullTomlEvidence(), network: "testnet" },
      /BSC offline full TOML evidence network must not use multiple aliases/u,
    ],
    [
      "noncanonical chain label",
      { ...offlineFullTomlEvidence(), chain: "testnet" },
      /BSC offline full TOML evidence chain must be bsc-testnet/u,
    ],
    [
      "duplicate post-deploy containers",
      {
        ...offlineFullTomlEvidence(),
        post_deploy_live_evidence: {
          fullTomlReady: true,
          offlineFullTomlSha256: HASH_77,
        },
      },
      /BSC offline full TOML evidence postDeployLiveEvidence must not use multiple aliases/u,
    ],
  ]) {
    const fullTomlEvidencePath = join(dir, `${name.replaceAll(" ", "-")}.json`);
    await writeFile(
      fullTomlEvidencePath,
      `${JSON.stringify(fullTomlEvidence, null, 2)}\n`,
    );
    await assert.rejects(
      () =>
        main([
          ...baseArgs,
          "--offline-full-toml-evidence",
          fullTomlEvidencePath,
          "--out",
          join(dir, `${name.replaceAll(" ", "-")}.manifest.json`),
        ]),
      pattern,
      name,
    );
  }
});

test("BSC route-manifest production readiness rejects missing TOML hash and diagnostic verifier material", async () => {
  const dir = await mkdtemp(join(tmpdir(), "iroha-bsc-route-manifest-bad-"));
  const evidencePath = join(dir, "deployment.evidence.json");
  const diagnosticEvidencePath = join(dir, "diagnostic.evidence.json");
  const contractPath = join(dir, "burn-record.contract.json");
  const bundlePath = join(dir, "native-prover-bundle.json");
  const diagnosticBundlePath = join(
    dir,
    "diagnostic-native-prover-bundle.json",
  );
  const fullTomlEvidencePath = join(dir, "full-config.evidence.json");
  const evidence = buildDeploymentEvidence({
    tokenAddress: BSC_TOKEN_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    sourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
    verifierAddress: BSC_VERIFIER_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: HASH_22,
    readback: readyReadback(),
  });
  const diagnosticEvidence = buildDeploymentEvidence({
    tokenAddress: BSC_TOKEN_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    sourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
    verifierAddress: BSC_VERIFIER_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: DIAGNOSTIC_BSC_VERIFIER_KEY_HASH,
    readback: readyReadback({
      verifierKeyHash: DIAGNOSTIC_BSC_VERIFIER_KEY_HASH,
      bridgeVerifierKeyHash: DIAGNOSTIC_BSC_VERIFIER_KEY_HASH,
      bridgeDestinationBindingHash: diagnosticBindingHash(),
    }),
  });
  const bundle = nativeProverBundleForRollout(
    routeManifest().destinationRollout,
  );
  const diagnosticRoute = routeManifest({
    destinationRollout: {
      verifierKeyHash: DIAGNOSTIC_BSC_VERIFIER_KEY_HASH,
      destinationBindingHash: diagnosticBindingHash(),
      destinationBindingKey: diagnosticBindingKey(),
    },
    destinationBinding: {
      key: diagnosticBindingKey(),
      bindingHash: diagnosticBindingHash(),
    },
  });
  const diagnosticBundle = nativeProverBundleForRollout(
    diagnosticRoute.destinationRollout,
  );
  await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);
  await writeFile(
    diagnosticEvidencePath,
    `${JSON.stringify(diagnosticEvidence, null, 2)}\n`,
  );
  await writeFile(
    contractPath,
    `${JSON.stringify(tairaBurnRecordContract(), null, 2)}\n`,
  );
  await writeFile(bundlePath, `${JSON.stringify(bundle, null, 2)}\n`);
  await writeFile(
    diagnosticBundlePath,
    `${JSON.stringify(diagnosticBundle, null, 2)}\n`,
  );
  await writeFile(
    fullTomlEvidencePath,
    `${JSON.stringify(offlineFullTomlEvidence(), null, 2)}\n`,
  );
  const readyArgs = [
    "route-manifest",
    "--taira-contract",
    contractPath,
    "--settlement-asset-definition-id",
    "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
    "--source-bridge-config-hash",
    HASH_33,
    "--source-event-transaction-id",
    HASH_55,
    "--source-event-explorer-url",
    SOURCE_EVENT_EXPLORER_URL,
    "--route-canary-evidence-hash",
    HASH_66,
    "--route-canary-transaction-id",
    HASH_77,
    "--route-canary-explorer-url",
    ROUTE_CANARY_EXPLORER_URL,
    "--full-toml-ready",
    "true",
    "--production-ready",
    "true",
    "--live-readback-checked",
    "true",
    "--confirm-testnet",
    "taira_bsc_xor",
    "--out",
    join(dir, "bad-route.manifest.json"),
  ];

  await assert.rejects(
    () =>
      main([
        ...readyArgs,
        "--evidence",
        evidencePath,
        "--native-prover-bundle",
        bundlePath,
      ]),
    /production-ready BSC route manifests require --offline-full-toml-evidence/u,
  );
  await assert.rejects(
    () =>
      main([
        ...readyArgs,
        "--evidence",
        diagnosticEvidencePath,
        "--native-prover-bundle",
        diagnosticBundlePath,
        "--offline-full-toml-evidence",
        fullTomlEvidencePath,
      ]),
    /diagnostic BSC verifier material/u,
  );
});

test("BSC route-manifest production readiness rejects placeholder TAIRA burn-record artifacts", async () => {
  const dir = await mkdtemp(join(tmpdir(), "iroha-bsc-route-manifest-burn-"));
  const evidencePath = join(dir, "deployment.evidence.json");
  const bundlePath = join(dir, "native-prover-bundle.json");
  const evidence = buildDeploymentEvidence({
    tokenAddress: BSC_TOKEN_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    sourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
    verifierAddress: BSC_VERIFIER_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: HASH_22,
    readback: readyReadback(),
  });
  const bundle = nativeProverBundleForRollout(
    routeManifest().destinationRollout,
  );
  await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);
  await writeFile(bundlePath, `${JSON.stringify(bundle, null, 2)}\n`);
  const readyArgs = [
    "route-manifest",
    "--evidence",
    evidencePath,
    "--settlement-asset-definition-id",
    "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
    "--native-prover-bundle",
    bundlePath,
    "--source-bridge-config-hash",
    HASH_33,
    "--source-event-transaction-id",
    HASH_55,
    "--source-event-explorer-url",
    SOURCE_EVENT_EXPLORER_URL,
    "--route-canary-evidence-hash",
    HASH_66,
    "--route-canary-transaction-id",
    HASH_77,
    "--route-canary-explorer-url",
    ROUTE_CANARY_EXPLORER_URL,
    "--full-toml-ready",
    "true",
    "--offline-full-toml-sha256",
    hex32("88"),
    "--production-ready",
    "true",
    "--live-readback-checked",
    "true",
    "--confirm-testnet",
    "taira_bsc_xor",
  ];
  const adversarialArtifacts = [
    {
      name: "short fixture text",
      bytes: FIXTURE_BURN_RECORD_BYTES,
      pattern: /burn-record contract artifact.*at least/u,
    },
    {
      name: "diagnostic marker inside binary",
      bytes: Buffer.concat([
        Buffer.from("diagnostic burn-record placeholder", "utf8"),
        deterministicBytes("diagnostic burn record padding", 512),
      ]),
      pattern: /placeholder burn-record material.*diagnostic/u,
    },
    {
      name: "repeated byte material",
      bytes: Buffer.alloc(512, 0xab),
      pattern: /placeholder burn-record material.*repeated/u,
    },
    {
      name: "arithmetic byte sequence",
      bytes: Buffer.from(
        Array.from({ length: 512 }, (_, index) => index & 0xff),
      ),
      pattern: /placeholder burn-record material.*arithmetic/u,
    },
    {
      name: "dominant byte padding",
      bytes: Buffer.concat([Buffer.alloc(508, 0), Buffer.from([1, 2, 3, 4])]),
      pattern: /placeholder burn-record material.*dominates/u,
    },
  ];

  for (const artifact of adversarialArtifacts) {
    const contractPath = join(
      dir,
      `${artifact.name.replaceAll(" ", "-")}.json`,
    );
    await writeFile(
      contractPath,
      `${JSON.stringify(
        tairaBurnRecordContractWithBytes(artifact.bytes),
        null,
        2,
      )}\n`,
    );
    await assert.rejects(
      () =>
        main([
          ...readyArgs,
          "--taira-contract",
          contractPath,
          "--out",
          join(dir, `${artifact.name.replaceAll(" ", "-")}.manifest.json`),
        ]),
      artifact.pattern,
      artifact.name,
    );
  }
});

test("BSC route-manifest command refuses draft manifests in the canonical default output", async () => {
  const dir = await mkdtemp(
    join(tmpdir(), "iroha-bsc-route-manifest-default-"),
  );
  const evidencePath = join(dir, "deployment.evidence.json");
  const contractPath = join(dir, "burn-record.contract.json");
  const evidence = buildDeploymentEvidence({
    tokenAddress: BSC_TOKEN_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    sourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
    verifierAddress: BSC_VERIFIER_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: HASH_22,
    readback: readyReadback(),
  });
  await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);
  await writeFile(
    contractPath,
    `${JSON.stringify(tairaBurnRecordContract(), null, 2)}\n`,
  );

  await assert.rejects(
    () =>
      main([
        "route-manifest",
        "--evidence",
        evidencePath,
        "--taira-contract",
        contractPath,
        "--settlement-asset-definition-id",
        "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
      ]),
    /cannot be written to canonical BSC production artifact path.*not productionReady true/u,
  );
});

test("BSC route-manifest command rejects duplicate JSON keys in operator inputs", async () => {
  const dir = await mkdtemp(
    join(tmpdir(), "iroha-bsc-route-manifest-duplicates-"),
  );
  const evidencePath = join(dir, "deployment.evidence.json");
  const duplicateEvidencePath = join(dir, "deployment.duplicate.json");
  const contractPath = join(dir, "burn-record.contract.json");
  const duplicateContractPath = join(dir, "burn-record.duplicate.json");
  const evidence = buildDeploymentEvidence({
    tokenAddress: BSC_TOKEN_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    sourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
    verifierAddress: BSC_VERIFIER_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: HASH_22,
    readback: readyReadback(),
  });
  const contract = tairaBurnRecordContract();
  await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);
  await writeFile(
    duplicateEvidencePath,
    `${JSON.stringify(evidence, null, 2).replace(
      '"routeId": "taira_bsc_xor"',
      '"routeId": "shadow",\n  "routeId": "taira_bsc_xor"',
    )}\n`,
  );
  await writeFile(contractPath, `${JSON.stringify(contract, null, 2)}\n`);
  await writeFile(
    duplicateContractPath,
    `${JSON.stringify(contract, null, 2).replace(
      '"artifact_b64":',
      '"artifact_b64": "unsafe-overwrite",\n  "artifact_b64":',
    )}\n`,
  );
  const baseArgs = [
    "route-manifest",
    "--settlement-asset-definition-id",
    "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
    "--proof-artifact-hash",
    HASH_44,
    "--proving-key-hash",
    HASH_55,
  ];

  await assert.rejects(
    () =>
      main([
        ...baseArgs,
        "--evidence",
        duplicateEvidencePath,
        "--taira-contract",
        contractPath,
        "--out",
        join(dir, "duplicate-evidence.manifest.json"),
      ]),
    /BSC deployment evidence is not valid JSON: BSC deployment evidence contains a duplicate JSON object key/u,
  );
  await assert.rejects(
    () =>
      main([
        ...baseArgs,
        "--evidence",
        evidencePath,
        "--taira-contract",
        duplicateContractPath,
        "--out",
        join(dir, "duplicate-contract.manifest.json"),
      ]),
    /TAIRA burn-record contract is not valid JSON: TAIRA burn-record contract contains a duplicate JSON object key/u,
  );
});

test("BSC route-manifest command rejects non-object JSON operator inputs", async () => {
  const dir = await mkdtemp(
    join(tmpdir(), "iroha-bsc-route-manifest-non-object-"),
  );
  const evidencePath = join(dir, "deployment.evidence.json");
  const arrayEvidencePath = join(dir, "deployment.array.json");
  const contractPath = join(dir, "burn-record.contract.json");
  const arrayContractPath = join(dir, "burn-record.array.json");
  const evidence = buildDeploymentEvidence({
    tokenAddress: BSC_TOKEN_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    sourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
    verifierAddress: BSC_VERIFIER_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: HASH_22,
    readback: readyReadback(),
  });
  await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);
  await writeFile(arrayEvidencePath, "[]\n");
  await writeFile(
    contractPath,
    `${JSON.stringify(tairaBurnRecordContract(), null, 2)}\n`,
  );
  await writeFile(arrayContractPath, "[]\n");
  const baseArgs = [
    "route-manifest",
    "--settlement-asset-definition-id",
    "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
    "--proof-artifact-hash",
    HASH_44,
    "--proving-key-hash",
    HASH_55,
  ];

  await assert.rejects(
    () =>
      main([
        ...baseArgs,
        "--evidence",
        arrayEvidencePath,
        "--taira-contract",
        contractPath,
        "--out",
        join(dir, "array-evidence.manifest.json"),
      ]),
    /BSC deployment evidence is not valid JSON: BSC deployment evidence must be a JSON object/u,
  );
  await assert.rejects(
    () =>
      main([
        ...baseArgs,
        "--evidence",
        evidencePath,
        "--taira-contract",
        arrayContractPath,
        "--out",
        join(dir, "array-contract.manifest.json"),
      ]),
    /TAIRA burn-record contract is not valid JSON: TAIRA burn-record contract must be a JSON object/u,
  );
});

test("BSC route-manifest command rejects oversized JSON operator inputs before parsing", async () => {
  const dir = await mkdtemp(
    join(tmpdir(), "iroha-bsc-route-manifest-oversized-"),
  );
  const evidencePath = join(dir, "deployment.oversized.json");
  const contractPath = join(dir, "burn-record.contract.json");
  await writeFile(evidencePath, "");
  await truncate(evidencePath, SCCP_BSC_JSON_INPUT_MAX_BYTES + 1);
  await writeFile(
    contractPath,
    `${JSON.stringify(tairaBurnRecordContract(), null, 2)}\n`,
  );

  await assert.rejects(
    () =>
      main([
        "route-manifest",
        "--evidence",
        evidencePath,
        "--taira-contract",
        contractPath,
        "--settlement-asset-definition-id",
        "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
        "--proof-artifact-hash",
        HASH_44,
        "--proving-key-hash",
        HASH_55,
        "--out",
        join(dir, "oversized.manifest.json"),
      ]),
    /BSC deployment evidence could not be read: path is .*maximum allowed/u,
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

test("BSC route-manifest command rejects duplicate deployment evidence aliases", async () => {
  const baseEvidence = buildDeploymentEvidence({
    tokenAddress: BSC_TOKEN_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    sourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
    verifierAddress: BSC_VERIFIER_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: HASH_22,
    readback: readyReadback(),
  });
  const baseOptions = {
    "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
    "proof-artifact-hash": HASH_44,
    "proving-key-hash": HASH_55,
  };
  const buildDraft = (evidence, options = {}) =>
    buildBscTairaXorRouteManifestDraft({
      evidence,
      tairaContract: tairaBurnRecordContract(),
      options: { ...baseOptions, ...options },
      createdAt: "2026-06-13T00:00:00.000Z",
    });
  const bindingKey = bscDestinationBindingKey({
    verifierAddress: BSC_VERIFIER_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: HASH_22,
  });

  for (const [name, evidence, pattern] of [
    [
      "BSC network",
      { ...baseEvidence, bsc_network: "testnet" },
      /BSC deployment evidence bscNetwork must not use multiple aliases in BSC deployment evidence: bscNetwork, bsc_network/u,
    ],
    [
      "chain id",
      { ...baseEvidence, chain_id_hex: "0x61" },
      /BSC deployment evidence chainIdHex must not use multiple aliases in BSC deployment evidence: chainIdHex, chain_id_hex/u,
    ],
    [
      "network id",
      { ...baseEvidence, network_id_hex: BSC_TESTNET_NETWORK_ID_HEX },
      /BSC deployment evidence networkIdHex must not use multiple aliases in BSC deployment evidence: networkIdHex, network_id_hex/u,
    ],
    [
      "destination rollout container",
      { ...baseEvidence, destination_rollout: baseEvidence.destinationRollout },
      /BSC deployment evidence destinationRollout must not use multiple aliases in BSC deployment evidence: destinationRollout, destination_rollout/u,
    ],
    [
      "destination binding container",
      { ...baseEvidence, destination_binding: baseEvidence.destinationBinding },
      /BSC deployment evidence destinationBinding must not use multiple aliases in BSC deployment evidence: destinationBinding, destination_binding/u,
    ],
    [
      "token address",
      { ...baseEvidence, bsc_token_address: BSC_TOKEN_ADDRESS },
      /BSC deployment evidence token address must not use multiple aliases in BSC deployment evidence: bscTokenAddress, bsc_token_address/u,
    ],
    [
      "rollout bridge address",
      {
        ...baseEvidence,
        destinationRollout: {
          ...baseEvidence.destinationRollout,
          destination_bridge_address: BSC_BRIDGE_ADDRESS,
        },
      },
      /BSC deployment evidence bridge address must not use multiple aliases in BSC deployment evidence destinationRollout: destinationBridgeAddress, destination_bridge_address/u,
    ],
    [
      "source bridge address",
      {
        ...baseEvidence,
        sccp_bsc_source_bridge_address: BSC_SOURCE_BRIDGE_ADDRESS,
      },
      /BSC deployment evidence source bridge address must not use multiple aliases in BSC deployment evidence: sccpBscSourceBridgeAddress, sccp_bsc_source_bridge_address/u,
    ],
    [
      "verifier address",
      { ...baseEvidence, verifier_address: BSC_VERIFIER_ADDRESS },
      /BSC deployment evidence verifier address must not use multiple aliases in BSC deployment evidence: bscVerifierAddress, verifier_address/u,
    ],
    [
      "verifier code hash",
      {
        ...baseEvidence,
        verifierCodeHash: HASH_11,
        verifier_code_hash: HASH_11,
      },
      /BSC deployment evidence verifierCodeHash must not use multiple aliases in BSC deployment evidence: verifierCodeHash, verifier_code_hash/u,
    ],
    [
      "rollout verifier key hash",
      {
        ...baseEvidence,
        destinationRollout: {
          ...baseEvidence.destinationRollout,
          verifier_key_hash: HASH_22,
        },
      },
      /BSC deployment evidence verifierKeyHash must not use multiple aliases in BSC deployment evidence destinationRollout: verifierKeyHash, verifier_key_hash/u,
    ],
    [
      "destination binding hash",
      {
        ...baseEvidence,
        destinationBinding: {
          ...baseEvidence.destinationBinding,
          binding_hash: bindingHash(),
        },
      },
      /BSC deployment evidence destinationBindingHash must not use multiple aliases in BSC deployment evidence destinationBinding: bindingHash, binding_hash/u,
    ],
    [
      "destination binding key",
      {
        ...baseEvidence,
        destinationBinding: {
          ...baseEvidence.destinationBinding,
          destinationBindingKey: bindingKey,
        },
      },
      /BSC deployment evidence destinationBindingKey must not use multiple aliases in BSC deployment evidence destinationBinding: key, destinationBindingKey/u,
    ],
    [
      "proof artifact hash",
      {
        ...baseEvidence,
        proofArtifactHash: HASH_44,
        proof_artifact_hash: HASH_44,
      },
      /BSC route proofArtifactHash must not use multiple aliases in BSC deployment evidence: proofArtifactHash, proof_artifact_hash/u,
    ],
    [
      "post-deploy evidence container",
      {
        ...baseEvidence,
        postDeployLiveEvidence: { fullTomlReady: false },
        post_deploy_live_evidence: { fullTomlReady: false },
      },
      /BSC deployment evidence postDeployLiveEvidence must not use multiple aliases in BSC deployment evidence: postDeployLiveEvidence, post_deploy_live_evidence/u,
    ],
  ]) {
    await assert.rejects(() => buildDraft(evidence), pattern, name);
  }

  await assert.rejects(
    () => buildDraft(baseEvidence, { "prover-artifact-hash": HASH_44 }),
    /BSC route proofArtifactHash must not use multiple aliases in route-manifest options: proof-artifact-hash, prover-artifact-hash/u,
  );
});

test("BSC route-manifest helper ignores accessor-backed request fields", async () => {
  const requestInput = accessorBackedRecord([
    "options",
    "evidence",
    "tairaContract",
    "liveEvidence",
    "offlineFullTomlEvidence",
    "createdAt",
  ]);
  await assert.rejects(
    () => buildBscTairaXorRouteManifestDraft(requestInput.record),
    /BSC deployment evidence must be a JSON object/u,
  );
  assert.equal(requestInput.readCount(), 0);

  let optionReads = 0;
  const options = {
    "settlement-asset-definition-id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
    "proof-artifact-hash": HASH_44,
    "proving-key-hash": HASH_55,
  };
  for (const key of [
    "production-ready",
    "confirm-testnet",
    "live-readback-checked",
    "native-prover-bundle",
  ]) {
    Object.defineProperty(options, key, {
      enumerable: true,
      get() {
        optionReads += 1;
        return key === "confirm-testnet" ? "taira_bsc_xor" : "true";
      },
    });
  }
  const manifest = await buildBscTairaXorRouteManifestDraft({
    evidence: buildDeploymentEvidence({
      tokenAddress: BSC_TOKEN_ADDRESS,
      bridgeAddress: BSC_BRIDGE_ADDRESS,
      sourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
      verifierAddress: BSC_VERIFIER_ADDRESS,
      verifierCodeHash: HASH_11,
      verifierKeyHash: HASH_22,
      readback: readyReadback(),
    }),
    tairaContract: tairaBurnRecordContract(),
    options,
    createdAt: "2026-06-13T00:00:00.000Z",
  });
  assert.equal(manifest.productionReady, false);
  assert.equal(hasOwn(manifest, "nativeEvmProverBundle"), false);
  assert.equal(optionReads, 0);
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
      readyReadback({ verifierKeyHash: HASH_33 }),
      /deployed verifier key hash/u,
    ],
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
  assert.match(
    unsafeSecretReason({ nested: { api_token: "tok_live_operator" } }),
    /token|secret/u,
  );
  assert.match(
    unsafeSecretReason({
      headers: ["Authorization: Bearer abcdefghijklmnopqrstuvwxyz"],
    }),
    /token|secret/u,
  );
  assert.match(
    unsafeSecretReason({ notes: "password=correct horse battery staple" }),
    /token|secret/u,
  );
  assert.match(
    unsafeSecretReason({ notes: "api_token=<operator-token>" }),
    /token|secret/u,
  );
  assert.match(
    unsafeSecretReason({ nested: { refreshToken: "refresh-token-value" } }),
    /token|secret/u,
  );
  assert.equal(
    unsafeSecretReason({
      notes:
        'private_key = "<redacted>"\npassword = <runtime-only>\napi_token = "***"',
    }),
    "",
  );
  assert.equal(
    unsafeSecretReason({
      notes: 'private_key = "REPLACE_WITH_VALIDATOR_PRIVATE_KEY"',
    }),
    "",
  );
});

test("BSC deployment helper secret scan ignores accessor-backed values", () => {
  let objectReads = 0;
  const record = {};
  Object.defineProperty(record, "notes", {
    enumerable: true,
    get() {
      objectReads += 1;
      return "private_key=0xabc";
    },
  });
  assert.equal(unsafeSecretReason(record), "");
  assert.equal(objectReads, 0);

  let arrayReads = 0;
  const nested = [];
  Object.defineProperty(nested, "0", {
    enumerable: true,
    get() {
      arrayReads += 1;
      return "api_token=tok_live_operator";
    },
  });
  nested.length = 1;
  assert.equal(unsafeSecretReason({ nested }), "");
  assert.equal(arrayReads, 0);

  let keyReads = 0;
  const secretKeyRecord = {};
  Object.defineProperty(secretKeyRecord, "apiToken", {
    enumerable: true,
    get() {
      keyReads += 1;
      return "tok_live_operator";
    },
  });
  assert.match(unsafeSecretReason(secretKeyRecord), /token|secret/u);
  assert.equal(keyReads, 0);
});

test("BSC JSON input parser rejects duplicate keys before overwrite", () => {
  assert.throws(
    () =>
      parseJsonWithoutDuplicateKeys(
        '{"\\u0072outeId":"shadow","routeId":"taira_bsc_xor"}',
        "BSC fixture",
      ),
    /BSC fixture contains a duplicate JSON object key/u,
  );
  assert.deepEqual(
    parseJsonWithoutDuplicateKeys(
      '{"routeId":"taira_bsc_xor","nested":[{"routeId":"shadow"},{"routeId":"taira_bsc_xor"}]}',
      "BSC fixture",
    ),
    {
      routeId: "taira_bsc_xor",
      nested: [{ routeId: "shadow" }, { routeId: "taira_bsc_xor" }],
    },
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
  assert.equal(
    normalized.expectedVerifierKeyHash,
    bscGroth16VerifierKeyHash(verifierMaterial()),
  );
  assert.equal(
    isKnownDiagnosticBscVerifierKeyHash(normalized.expectedVerifierKeyHash),
    false,
  );
  assert.equal(normalized.ic.length, 20);

  assert.throws(
    () =>
      normalizeVerifierMaterial(verifierMaterial({ verifierKeyHash: HASH_22 })),
    /expectedVerifierKeyHash must match Solidity verifyingKeyHash\(\)/u,
  );
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
  assert.throws(
    () => normalizeVerifierMaterial(verifierMaterial({ alpha1: [1, 3] })),
    /BN254 G1 curve/u,
  );
  assert.throws(
    () =>
      normalizeVerifierMaterial(
        verifierMaterial({ alpha1: SCALAR_FIELD_ONLY_G1_POINT }),
      ),
    /BN254 G1 curve/u,
  );
  assert.throws(
    () =>
      normalizeVerifierMaterial(
        verifierMaterial({ alpha1: [BN254_BASE_FIELD_MODULUS, 2] }),
      ),
    /BN254 field element/u,
  );
  for (const field of ["beta2", "gamma2", "delta2"]) {
    assert.throws(
      () =>
        normalizeVerifierMaterial(
          verifierMaterial({ [field]: SCALAR_FIELD_ONLY_G2_POINT }),
        ),
      new RegExp(`${field} must be on the BN254 G2 twist curve`, "u"),
    );
  }
  assert.throws(
    () =>
      normalizeVerifierMaterial(
        verifierMaterial({
          beta2: [
            BN254_BASE_FIELD_MODULUS,
            SMOKE_FIXTURE_G2[1],
            ...SMOKE_FIXTURE_G2.slice(2),
          ],
        }),
      ),
    /BN254 field element/u,
  );
  assert.throws(
    () =>
      normalizeVerifierMaterial(
        verifierMaterial({ ic: [0, 0, ...VALID_IC.slice(2)] }),
      ),
    /point at infinity/u,
  );
});

test("BSC verifier material normalization ignores inherited verifier fields", () => {
  const inheritedMaterial = Object.create(verifierMaterial());

  assert.throws(
    () => normalizeVerifierMaterial(inheritedMaterial),
    /expectedVerifierKeyHash/u,
  );
  assert.equal(
    isSmokeFixtureGroth16VerifierMaterial(
      Object.create(
        verifierMaterial({
          alpha1: SMOKE_FIXTURE_G1,
          beta2: SMOKE_FIXTURE_G2,
          gamma2: SMOKE_FIXTURE_G2,
          delta2: SMOKE_FIXTURE_G2,
          ic: SMOKE_FIXTURE_IC,
        }),
      ),
    ),
    false,
  );
});

test("BSC verifier material normalization ignores accessor-backed arrays", () => {
  let reads = 0;
  const alpha1 = [];
  Object.defineProperty(alpha1, "0", {
    enumerable: true,
    get() {
      reads += 1;
      return VALID_G1_POINTS[0][0];
    },
  });
  alpha1[1] = VALID_G1_POINTS[0][1];
  alpha1.length = 2;

  assert.throws(
    () =>
      normalizeVerifierMaterial(
        verifierMaterial({
          alpha1,
          expectedVerifierKeyHash: HASH_22,
        }),
      ),
    /alpha1 must contain 2 uint256/u,
  );
  assert.equal(reads, 0);
});

test("BSC verifier material diagnostic flags must be own fields", () => {
  const material = {
    ...verifierMaterial(),
  };
  Object.setPrototypeOf(material, {
    diagnostic: true,
    warning: "Generated diagnostic BSC testnet verifier material.",
  });

  const normalized = normalizeVerifierMaterial(material);
  assert.deepEqual(normalized.diagnosticVerifierReasons, []);
});

test("BSC verifier material reports diagnostic key material before deployment", () => {
  const normalized = normalizeVerifierMaterial(
    verifierMaterial({
      schema: "iroha-sccp-bsc-testnet-diagnostic-verifier-key/v1",
      warning: "Generated diagnostic BSC testnet verifier material.",
      alpha1: SMOKE_FIXTURE_G1,
      beta2: SMOKE_FIXTURE_G2,
      gamma2: SMOKE_FIXTURE_G2,
      delta2: SMOKE_FIXTURE_G2,
      ic: SMOKE_FIXTURE_IC,
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
  assert.equal(
    isSmokeFixtureGroth16VerifierMaterial(smokeFixtureMaterial),
    true,
  );
  assert.equal(normalized.fixtureShaped, true);
  assert.match(
    normalized.diagnosticVerifierReasons.join(" "),
    /smoke-test Groth16 fixture/u,
  );

  assert.equal(
    isSmokeFixtureGroth16VerifierMaterial(verifierMaterial()),
    false,
  );
});

test("BSC local deploy smoke verifier shape is valid but not fixture-shaped", () => {
  const localSmokeVerifierMaterial = verifierMaterial({
    alpha1: SMOKE_FIXTURE_G1,
    beta2: SMOKE_FIXTURE_G2,
    gamma2: SMOKE_FIXTURE_G2,
    delta2: SMOKE_FIXTURE_G2,
    ic: VALID_IC,
  });

  const normalized = normalizeVerifierMaterial(localSmokeVerifierMaterial);
  assert.equal(
    isSmokeFixtureGroth16VerifierMaterial(localSmokeVerifierMaterial),
    false,
  );
  assert.equal(normalized.fixtureShaped, false);
  assert.deepEqual(normalized.diagnosticVerifierReasons, []);
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
  assert.match(toml, /explorer_url = "https:\/\/testnet\.bscscan\.com"/u);
  assert.match(toml, /explorer_host = "testnet\.bscscan\.com"/u);
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
    new RegExp(
      `sccp_bsc_source_bridge_address = "${BSC_SOURCE_BRIDGE_ADDRESS}"`,
      "u",
    ),
  );
  assert.doesNotMatch(toml, /(^|\n)source_bridge_address =/u);
  assert.doesNotMatch(toml, /(^|\n)bsc_source_bridge_address =/u);
  assert.doesNotMatch(toml, /(^|\n)sccp_tron_source_bridge_address =/u);
  assert.match(
    toml,
    new RegExp(
      `sccp_bsc_destination_verifier_address = "${BSC_VERIFIER_ADDRESS}"`,
      "u",
    ),
  );
  assert.doesNotMatch(toml, /(^|\n)destination_verifier_address =/u);
  assert.doesNotMatch(toml, /(^|\n)verifier_address =/u);
  assert.doesNotMatch(toml, /(^|\n)bsc_verifier_address =/u);
  assert.doesNotMatch(toml, /(^|\n)evm_verifier_address =/u);
  assert.doesNotMatch(toml, /(^|\n)tron_verifier_address =/u);
  assert.match(toml, new RegExp(`proof_artifact_hash = "${HASH_44}"`, "u"));
  assert.doesNotMatch(toml, /(^|\n)prover_artifact_hash =/u);
  assert.doesNotMatch(toml, /(^|\n)circuit_artifact_hash =/u);
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

test("BSC route-config rejects route material supplied only by prototypes", () => {
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(Object.create(routeManifest()), {
        "allow-unready": "true",
      }),
    /route manifest schema/u,
  );

  const route = routeManifest();
  const {
    destinationRollout,
    productionReady: _productionReady,
    ...ownRouteWithoutRolloutOrReady
  } = route;
  Object.setPrototypeOf(ownRouteWithoutRolloutOrReady, {
    destinationRollout,
    productionReady: false,
  });
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(ownRouteWithoutRolloutOrReady, {
        "allow-unready": "true",
      }),
    /route manifest destinationRollout/u,
  );
});

test("BSC native-prover-bundle helper ignores accessor-backed options", async () => {
  const { record, readCount } = accessorBackedRecord([
    "artifact-root",
    "route-manifest",
    "manifest",
    "evidence",
    "deployment-evidence",
    "proof-artifact",
    "proving-key",
    "verifier-key",
    "cross-sdk-parity",
    "native-prover-self-test",
    "typescript-implementation",
  ]);

  await assert.rejects(
    () => buildBscNativeEvmProverBundleFromArtifacts(record),
    /requires --route-manifest or --deployment-evidence/u,
  );
  assert.equal(readCount(), 0);
});

test("BSC route-config rejects duplicate required route manifest string aliases", () => {
  for (const [name, manifest, pattern] of [
    [
      "route id",
      routeManifest({ route_id: "taira_bsc_xor" }),
      /route manifest routeId must not use multiple aliases: routeId, route_id/u,
    ],
    [
      "asset key",
      routeManifest({ asset_key: "xor" }),
      /route manifest assetKey must not use multiple aliases: assetKey, asset_key/u,
    ],
    [
      "chain id",
      routeManifest({ chain_id_hex: "0x61" }),
      /route manifest chainIdHex must not use multiple aliases: chainIdHex, chain_id_hex/u,
    ],
    [
      "verifier target",
      routeManifest({ verifier_target: "EvmContract" }),
      /route manifest verifierTarget must not use multiple aliases: verifierTarget, verifier_target/u,
    ],
  ]) {
    assert.throws(
      () =>
        buildBscTairaXorRouteConfigToml(manifest, {
          "allow-unready": "true",
        }),
      pattern,
      name,
    );
  }
});

test("BSC route-config rejects duplicate route manifest container and scalar aliases", () => {
  const base = routeManifest();
  for (const [name, manifest, pattern] of [
    [
      "destination rollout container",
      { ...base, destination_rollout: base.destinationRollout },
      /route manifest destinationRollout must not use multiple aliases in route manifest: destinationRollout, destination_rollout/u,
    ],
    [
      "destination binding container",
      { ...base, destination_binding: base.destinationBinding },
      /route manifest destinationBinding must not use multiple aliases in route manifest: destinationBinding, destination_binding/u,
    ],
    [
      "burn-record container",
      { ...base, taira_xor_burn_record: base.tairaXorBurnRecord },
      /route manifest tairaXorBurnRecord must not use multiple aliases in route manifest: tairaXorBurnRecord, taira_xor_burn_record/u,
    ],
    [
      "burn-record vk ref container",
      routeManifest({
        tairaXorBurnRecord: { vk_ref: base.tairaXorBurnRecord.vkRef },
      }),
      /route manifest tairaXorBurnRecord\.vkRef must not use multiple aliases in route manifest tairaXorBurnRecord: vkRef, vk_ref/u,
    ],
    [
      "post-deploy container",
      { ...base, post_deploy_live_evidence: base.postDeployLiveEvidence },
      /route manifest postDeployLiveEvidence must not use multiple aliases in route manifest: postDeployLiveEvidence, post_deploy_live_evidence/u,
    ],
    [
      "BSC network",
      routeManifest({ bsc_network: "testnet" }),
      /route manifest bscNetwork must not use multiple aliases in route manifest: bscNetwork, bsc_network/u,
    ],
    [
      "production-ready flag",
      routeManifest({ production_ready: false }),
      /route manifest productionReady must not use multiple aliases in route manifest: productionReady, production_ready/u,
    ],
    [
      "counterparty domain",
      routeManifest({ counterparty_domain: SCCP_DOMAIN_BSC }),
      /route manifest counterpartyDomain must not use multiple aliases in route manifest: counterpartyDomain, counterparty_domain/u,
    ],
    [
      "rollout source domain",
      routeManifest({
        destinationRollout: { source_domain: SCCP_DOMAIN_SORA },
      }),
      /route manifest sourceDomain must not use multiple aliases in route manifest destinationRollout: sourceDomain, source_domain/u,
    ],
    [
      "binding source domain drift",
      routeManifest({ destinationBinding: { sourceDomain: 1 } }),
      /route manifest sourceDomain aliases disagree between destinationRollout and destinationBinding/u,
    ],
    [
      "rollout target domain",
      routeManifest({
        destinationRollout: { target_domain: SCCP_DOMAIN_BSC },
      }),
      /route manifest targetDomain must not use multiple aliases in route manifest destinationRollout: targetDomain, target_domain/u,
    ],
    [
      "verifier backend",
      routeManifest({
        destinationRollout: { verifier_backend: "evm-groth16-bn254-v1" },
      }),
      /route manifest verifierBackend must not use multiple aliases in route manifest destinationRollout: verifierBackend, verifier_backend/u,
    ],
    [
      "proof family",
      routeManifest({
        destinationRollout: { proof_family: "stark-fri-v1" },
      }),
      /route manifest proofFamily must not use multiple aliases in route manifest destinationRollout: proofFamily, proof_family/u,
    ],
    [
      "burn-record artifact bytes",
      routeManifest({
        tairaXorBurnRecord: { artifact_b64: BURN_RECORD_B64 },
      }),
      /route manifest tairaXorBurnRecord\.contractArtifactB64 must not use multiple aliases in route manifest tairaXorBurnRecord: contractArtifactB64, artifact_b64/u,
    ],
    [
      "burn-record artifact hash",
      routeManifest({
        tairaXorBurnRecord: { artifact_sha256: BURN_RECORD_SHA256 },
      }),
      /route manifest tairaXorBurnRecord\.artifactSha256 must not use multiple aliases in route manifest tairaXorBurnRecord: artifactSha256, artifact_sha256/u,
    ],
    [
      "burn-record settlement asset",
      routeManifest({
        tairaXorBurnRecord: {
          settlement_asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
        },
      }),
      /route manifest tairaXorBurnRecord\.settlementAssetDefinitionId must not use multiple aliases in route manifest tairaXorBurnRecord: settlementAssetDefinitionId, settlement_asset_definition_id/u,
    ],
    [
      "burn-record gas limit",
      routeManifest({ tairaXorBurnRecord: { gas_limit: 2_000_000 } }),
      /route manifest burn-record gasLimit must not use multiple aliases in route manifest tairaXorBurnRecord: gasLimit, gas_limit/u,
    ],
    [
      "burn-record code hash",
      routeManifest({ tairaXorBurnRecord: { code_hash: HASH_33 } }),
      /route manifest tairaXorBurnRecord\.codeHash must not use multiple aliases in route manifest tairaXorBurnRecord: codeHash, code_hash/u,
    ],
    [
      "settlement route id",
      routeManifest({ settlement: { route_id: "taira_bsc_xor" } }),
      /route manifest settlement\.routeId must not use multiple aliases in route manifest settlement: routeId, route_id/u,
    ],
    [
      "settlement asset key",
      routeManifest({ settlement: { asset_key: "xor" } }),
      /route manifest settlement\.assetKey must not use multiple aliases in route manifest settlement: assetKey, asset_key/u,
    ],
  ]) {
    assert.throws(
      () =>
        buildBscTairaXorRouteConfigToml(manifest, {
          "allow-unready": "true",
        }),
      pattern,
      name,
    );
  }
});

test("BSC route-config requires explicit post-deploy evidence for production-ready manifests", () => {
  const productionReadyManifest = (
    postDeployOverrides = {},
    overrides = {},
  ) => {
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
  const snakeCaseSettlementToml = buildBscTairaXorRouteConfigToml(
    productionReadyManifest(
      {},
      {
        settlement: {
          contract_address: "bsc-settlement-v1",
          contract_alias: "taira-bsc-xor",
        },
      },
    ),
  );
  assert.match(
    snakeCaseSettlementToml,
    /settlement_contract_address = "bsc-settlement-v1"/u,
  );
  assert.match(
    snakeCaseSettlementToml,
    /settlement_contract_alias = "taira-bsc-xor"/u,
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
        productionReadyManifest(
          { offlineFullTomlSha256: undefined },
          {
            productionReady: false,
            disabledReason: "disabled while verifier material is diagnostic",
          },
        ),
      ),
    /fullTomlReady requires postDeployLiveEvidence\.offlineFullTomlSha256/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyManifest({}, { explorerUrl: undefined }),
      ),
    /requires explorerUrl/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyManifest({}, { explorerHost: undefined }),
      ),
    /requires explorerHost/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyManifest({}, { bscExplorerUrl: "https://bscscan.com" }),
      ),
    /explorerUrl must not use multiple aliases|BSC testnet explorer origin/u,
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
          source_event_transaction_production_blockers: [
            "witness seal proof required",
          ],
        }),
      ),
    /productionReady requires empty postDeployLiveEvidence production blockers.*source_event_transaction_production_blockers: witness seal proof required/u,
    "BSC source event transaction contradictory blockers",
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyManifest({
          source_event_transaction_production_blockers:
            "witness seal proof required",
        }),
      ),
    /source_event_transaction_production_blockers must be a list/u,
    "BSC source event transaction scalar blockers",
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyManifest({
          source_event_transaction_production_blockers: [
            " witness seal proof required",
          ],
        }),
      ),
    /source_event_transaction_production_blockers\[0\].*non-empty canonical string/u,
    "BSC source event transaction malformed blocker entry",
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyManifest({
          post_deploy_production_blockers: ["route overlay still pending"],
        }),
      ),
    /productionReady requires empty postDeployLiveEvidence production blockers.*post_deploy_production_blockers: route overlay still pending/u,
    "BSC post-deploy blocker contradictory blockers",
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyManifest({
          full_toml_production_blockers: [123],
        }),
      ),
    /full_toml_production_blockers\[0\].*non-empty canonical string/u,
    "BSC full TOML blocker malformed entry",
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyManifest({
          route_canary_production_blockers: [" route canary evidence is stale"],
        }),
      ),
    /route_canary_production_blockers\[0\].*non-empty canonical string/u,
    "BSC route canary blocker malformed entry",
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

test("BSC route-config rejects malformed allow-unready option values", () => {
  const manifest = productionReadyRouteManifest();
  for (const value of [" TRUE", "true ", "TRUE", true, false, 1, 0]) {
    assert.throws(
      () =>
        buildBscTairaXorRouteConfigToml(manifest, { "allow-unready": value }),
      /--allow-unready must be true or false/u,
    );
    assert.throws(
      () =>
        buildMergedBscTairaXorRouteConfigToml(
          "[zk]\nother_setting = true\n",
          manifest,
          { "allow-unready": value },
        ),
      /--allow-unready must be true or false/u,
    );
  }
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
    explorerUrl: "https://bscscan.com",
    explorerHost: "bscscan.com",
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

test("BSC route-config rejects production-ready manifests with placeholder TAIRA burn-record artifacts", () => {
  const badArtifacts = [
    {
      bytes: FIXTURE_BURN_RECORD_BYTES,
      pattern: /TAIRA burn-record artifact.*at least/u,
    },
    {
      bytes: Buffer.from(
        Array.from({ length: 512 }, (_, index) => index & 0xff),
      ),
      pattern: /TAIRA burn-record artifact.*arithmetic/u,
    },
    {
      bytes: Buffer.concat([Buffer.alloc(508, 0), Buffer.from([1, 2, 3, 4])]),
      pattern: /TAIRA burn-record artifact.*dominates/u,
    },
  ];

  for (const artifact of badArtifacts) {
    const manifest = productionReadyRouteManifest();
    assert.throws(
      () =>
        buildBscTairaXorRouteConfigToml({
          ...manifest,
          tairaXorBurnRecord: {
            ...manifest.tairaXorBurnRecord,
            contractArtifactB64: Buffer.from(artifact.bytes).toString("base64"),
            artifactSha256: `0x${createHash("sha256")
              .update(artifact.bytes)
              .digest("hex")}`,
          },
        }),
      artifact.pattern,
    );
  }
});

test("BSC route-config requires SDK-valid native prover bundles for production readiness", () => {
  const manifest = productionReadyRouteManifest();
  const expectedNativeBundleHash = canonicalBscNativeEvmProverBundleHash(
    validateBscTestnetNativeEvmProverBundle(
      nativeProverBundleForRollout(manifest.destinationRollout),
      {
        expectedDestinationBindingHash:
          manifest.destinationRollout.destinationBindingHash,
      },
    ),
  );
  const validToml = buildBscTairaXorRouteConfigToml(manifest);
  assert.match(validToml, /production_ready = true/u);
  assert.match(
    validToml,
    new RegExp(`proof_artifact_hash = "${HASH_44}"`, "u"),
  );
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
  const missingVerifierArtifactHashBase = productionReadyRouteManifest();
  const missingVerifierArtifactHashBundle = nativeProverBundleForRollout(
    missingVerifierArtifactHashBase.destinationRollout,
  );
  delete missingVerifierArtifactHashBundle.verifier_key_artifact_hash;
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml({
        ...missingVerifierArtifactHashBase,
        nativeEvmProverBundle: missingVerifierArtifactHashBundle,
      }),
    /nativeEvmProverBundle verifierKeyArtifactHash is required/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyRouteManifest({
          bundleOverrides: { verifierKeyArtifactHash: HASH_88 },
        }),
      ),
    /nativeEvmProverBundle verifierKeyArtifactHash must not use multiple aliases/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyRouteManifest({
          bundleOverrides: { verifier_key_artifact_hash: HASH_22 },
        }),
      ),
    /nativeProverBundle hashes must be role-separated: verifierKeyArtifactHash matches verifierKeyHash/u,
  );
  assert.throws(
    () =>
      buildBscTairaXorRouteConfigToml(
        productionReadyRouteManifest({
          bundleOverrides: { verifier_key_artifact_hash: HASH_44 },
        }),
      ),
    /nativeEvmProverBundle.*hashes must be role-separated.*verifierKeyArtifactHash matches proofArtifactHash/u,
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
            verifier_key: "artifacts/bsc-testnet/proof-artifact.r1cs",
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
        circuit_security_audit: fixtureHash("alias circuit security audit"),
        native_implementation_audit: fixtureHash(
          "alias native implementation audit",
        ),
        reproducible_build_attestation: fixtureHash(
          "alias reproducible build attestation",
        ),
        cross_sdk_fixture_parity: fixtureHash("alias cross-SDK fixture parity"),
        native_prover_self_test: fixtureHash("alias native prover self-test"),
        no_wasm_no_remote_scan: fixtureHash("alias no-wasm no-remote scan"),
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

  assert.equal(
    result.descriptor.bundleId,
    SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
  );
  assert.equal(result.descriptor.proofArtifactHash, fixture.proofArtifactHash);
  assert.equal(result.descriptor.provingKeyHash, fixture.provingKeyHash);
  assert.equal(result.descriptor.verifierKeyHash, fixture.verifierKeyHash);
  assert.equal(
    result.descriptor.verifierKeyArtifactHash,
    fixture.verifierKeyArtifactHash,
  );
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
    sha256Hex(
      await readFile(join(fixture.artifactRoot, "cross-sdk-parity.json")),
    ),
  );
  assert.equal(
    result.bundle.audit_hashes.native_prover_self_test,
    sha256Hex(
      await readFile(
        join(fixture.artifactRoot, "native-prover-self-test.json"),
      ),
    ),
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
  assert.equal(
    JSON.parse(await readFile(out, "utf8")).proof_artifact_hash,
    fixture.proofArtifactHash,
  );
  assert.equal(
    JSON.parse(await readFile(attachedOut, "utf8")).nativeEvmProverBundle
      .proving_key_hash,
    fixture.provingKeyHash,
  );
  assert.equal(
    JSON.parse(await readFile(attachedOut, "utf8")).nativeEvmProverBundleHash,
    canonicalBscNativeEvmProverBundleHash(result.descriptor),
  );
});

test("BSC native-prover-bundle rejects duplicate JSON keys in route manifests", async () => {
  const fixture = await writeNativeProverFixtureFiles();
  const duplicateRouteManifestPath = join(
    fixture.workDir,
    "route.duplicate.json",
  );
  await writeFile(
    duplicateRouteManifestPath,
    `${(await readFile(fixture.routeManifestPath, "utf8")).replace(
      '"routeId": "taira_bsc_xor"',
      '"routeId": "shadow",\n  "routeId": "taira_bsc_xor"',
    )}\n`,
  );

  await assert.rejects(
    () =>
      main([
        "native-prover-bundle",
        ...Object.entries({
          ...fixture.options,
          "route-manifest": duplicateRouteManifestPath,
        }).flatMap(([key, value]) => [`--${key}`, value]),
        "--out",
        join(fixture.workDir, "bundle.duplicate.json"),
      ]),
    /BSC route manifest is not valid JSON: BSC route manifest contains a duplicate JSON object key/u,
  );
});

test("BSC native-prover-bundle rejects non-object JSON route manifests", async () => {
  const fixture = await writeNativeProverFixtureFiles();
  const arrayRouteManifestPath = join(fixture.workDir, "route.array.json");
  await writeFile(arrayRouteManifestPath, "[]\n");

  await assert.rejects(
    () =>
      main([
        "native-prover-bundle",
        ...Object.entries({
          ...fixture.options,
          "route-manifest": arrayRouteManifestPath,
        }).flatMap(([key, value]) => [`--${key}`, value]),
        "--out",
        join(fixture.workDir, "bundle.array.json"),
      ]),
    /BSC route manifest is not valid JSON: BSC route manifest must be a JSON object/u,
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
  for (const encodedPath of [
    "%2e%2e/proof-artifact.r1cs",
    "%252e%252e/proof-artifact.r1cs",
    "%252525252e%252525252e/proof-artifact.r1cs",
  ]) {
    const encodedDir = dirname(join(fixture.artifactRoot, encodedPath));
    await mkdir(encodedDir, { recursive: true });
    await writeFile(
      join(fixture.artifactRoot, encodedPath),
      await readFile(join(fixture.artifactRoot, "proof-artifact.r1cs")),
    );
    await assert.rejects(
      () =>
        buildBscNativeEvmProverBundleFromArtifacts({
          ...fixture.options,
          "proof-artifact": encodedPath,
        }),
      /proof artifact must not use URL-encoded parent-directory segments/u,
    );
  }
  const routeManifestLink = join(fixture.workDir, "route-link.json");
  await symlink(fixture.routeManifestPath, routeManifestLink);
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts({
        ...fixture.options,
        "route-manifest": routeManifestLink,
      }),
    /BSC route manifest could not be read: path must not be a symbolic link/u,
  );
  const proofArtifactLink = join(fixture.artifactRoot, "proof-link.r1cs");
  await symlink(
    join(fixture.artifactRoot, "proof-artifact.r1cs"),
    proofArtifactLink,
  );
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts({
        ...fixture.options,
        "proof-artifact": "proof-link.r1cs",
      }),
    /proof artifact could not be read: path must not be a symbolic link/u,
  );
  const outsideArtifactDir = await mkdtemp(
    join(tmpdir(), "bsc-native-proof-outside-"),
  );
  await writeFile(
    join(outsideArtifactDir, "proof-artifact.r1cs"),
    await readFile(join(fixture.artifactRoot, "proof-artifact.r1cs")),
  );
  await symlink(outsideArtifactDir, join(fixture.artifactRoot, "linked-dir"));
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts({
        ...fixture.options,
        "proof-artifact": "linked-dir/proof-artifact.r1cs",
      }),
    /proof artifact could not be read: proof artifact must stay under artifact-root/u,
  );
  const aliasFixture = await writeNativeProverFixtureFiles();
  const aliasSmuggledRoute = JSON.parse(
    await readFile(aliasFixture.routeManifestPath, "utf8"),
  );
  aliasSmuggledRoute.proofArtifactHash = aliasFixture.proofArtifactHash;
  aliasSmuggledRoute.proof_artifact_hash = aliasFixture.proofArtifactHash;
  await writeFile(
    aliasFixture.routeManifestPath,
    `${JSON.stringify(aliasSmuggledRoute, null, 2)}\n`,
  );
  await assert.rejects(
    () => buildBscNativeEvmProverBundleFromArtifacts(aliasFixture.options),
    /BSC route manifest proofArtifactHash must not use multiple aliases in BSC route manifest/u,
  );
  const verifierAliasFixture = await writeNativeProverFixtureFiles();
  const verifierAliasSmuggledRoute = JSON.parse(
    await readFile(verifierAliasFixture.routeManifestPath, "utf8"),
  );
  verifierAliasSmuggledRoute.verifierKeyHash =
    verifierAliasFixture.verifierKeyHash;
  verifierAliasSmuggledRoute.verifier_key_hash =
    verifierAliasFixture.verifierKeyHash;
  await writeFile(
    verifierAliasFixture.routeManifestPath,
    `${JSON.stringify(verifierAliasSmuggledRoute, null, 2)}\n`,
  );
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts(verifierAliasFixture.options),
    /BSC route manifest verifierKeyHash must not use multiple aliases in BSC route manifest/u,
  );
  await assert.rejects(() => {
    const { "dotnet-implementation": _drop, ...options } = fixture.options;
    return buildBscNativeEvmProverBundleFromArtifacts(options);
  }, /dotnet implementation artifact requires --dotnet-implementation/u);
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts({
        ...fixture.options,
        "audit-circuit-security": `0x${"00".repeat(32)}`,
      }),
    /auditHashes\.circuit_security_audit must be non-zero/u,
  );
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts({
        ...fixture.options,
        "audit-native-implementation": HASH_11,
      }),
    /auditHashes\.native_implementation_audit looks like placeholder audit hash: repeated 1-byte pattern/u,
  );
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts({
        ...fixture.options,
        "audit-reproducible-build": `0x${Array.from(
          { length: 32 },
          (_, index) => index.toString(16).padStart(2, "0"),
        ).join("")}`,
      }),
    /auditHashes\.reproducible_build_attestation looks like placeholder audit hash: arithmetic byte sequence with step 1/u,
  );
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts({
        ...fixture.options,
        "audit-cross-sdk-fixture-parity": sha256Hex(
          Buffer.from("wrong cross-SDK fixture parity audit hash", "utf8"),
        ),
      }),
    /auditHashes.cross_sdk_fixture_parity must match the artifact sha256/u,
  );
  await writeFile(
    join(fixture.artifactRoot, "cross-sdk-fixture-parity.json"),
    await readFile(join(fixture.artifactRoot, "cross-sdk-parity.json")),
  );
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts({
        ...fixture.options,
        "cross-sdk-parity": "cross-sdk-fixture-parity.json",
      }),
    /crossSdkFixtureParityArtifact must not reference diagnostic, fixture, mock, placeholder, sample, stub, or test-only material/u,
  );
  await writeFile(
    join(fixture.artifactRoot, "sample-native-prover-self-test.json"),
    await readFile(join(fixture.artifactRoot, "native-prover-self-test.json")),
  );
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts({
        ...fixture.options,
        "native-prover-self-test": "sample-native-prover-self-test.json",
      }),
    /nativeProverSelfTestArtifact must not reference diagnostic, fixture, mock, placeholder, sample, stub, or test-only material/u,
  );

  const crossReportCollision = await writeNativeProverFixtureFiles();
  const crossReportParityPath = join(
    crossReportCollision.artifactRoot,
    "cross-sdk-parity.json",
  );
  const crossReportSelfTestPath = join(
    crossReportCollision.artifactRoot,
    "native-prover-self-test.json",
  );
  const crossReportParity = JSON.parse(
    await readFile(crossReportParityPath, "utf8"),
  );
  const crossReportSelfTest = JSON.parse(
    await readFile(crossReportSelfTestPath, "utf8"),
  );
  crossReportSelfTest.source_proof_hash = crossReportParity.source_proof_hash;
  for (const sdkResult of Object.values(crossReportSelfTest.sdk_results)) {
    sdkResult.source_proof_hash = crossReportParity.source_proof_hash;
  }
  await writeFile(
    crossReportSelfTestPath,
    `${JSON.stringify(crossReportSelfTest, null, 2)}\n`,
  );
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts(crossReportCollision.options),
    /nativeProverReports hashes must be role-separated: nativeProverSelfTest\.sourceProofHash matches crossSdkFixtureParity\.sourceProofHash/u,
  );

  const oversizedProofPath = join(fixture.artifactRoot, "oversized-proof.r1cs");
  await writeFile(oversizedProofPath, "");
  await truncate(
    oversizedProofPath,
    SCCP_BSC_BINARY_ARTIFACT_INPUT_MAX_BYTES + 1,
  );
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts({
        ...fixture.options,
        "proof-artifact": "oversized-proof.r1cs",
      }),
    /proof artifact could not be read: path is .*maximum allowed/u,
  );

  const oversizedImplementationPath = join(
    fixture.artifactRoot,
    "oversized-javascript-implementation.bin",
  );
  await writeFile(oversizedImplementationPath, "");
  await truncate(
    oversizedImplementationPath,
    SCCP_BSC_BINARY_ARTIFACT_INPUT_MAX_BYTES + 1,
  );
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts({
        ...fixture.options,
        "javascript-implementation": "oversized-javascript-implementation.bin",
      }),
    /javascript implementation artifact could not be read: path is .*maximum allowed/u,
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

  const sparsePaddedProof = Buffer.alloc(96 * 1024, 0);
  for (let index = 0; index < 128; index += 1) {
    sparsePaddedProof[sparsePaddedProof.length - 128 + index] = index & 0xff;
  }
  const sparsePaddedProofFixture = await writeNativeProverFixtureFiles({
    artifactByteOverrides: {
      proofArtifact: sparsePaddedProof,
    },
  });
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts(
        sparsePaddedProofFixture.options,
      ),
    /proof artifact looks like placeholder proof material: byte 0x00 dominates/u,
  );

  const badR1csHeader = await writeNativeProverFixtureFiles();
  const badR1csBytes = Buffer.from(
    await readFile(join(badR1csHeader.artifactRoot, "proof-artifact.r1cs")),
  );
  badR1csBytes[0] = 0x78;
  await writeFile(
    join(badR1csHeader.artifactRoot, "bad-proof.r1cs"),
    badR1csBytes,
  );
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts({
        ...badR1csHeader.options,
        "proof-artifact": "bad-proof.r1cs",
      }),
    /proof artifact must start with \.r1cs magic bytes/u,
  );

  const wasmProofPath = await writeNativeProverFixtureFiles();
  await writeFile(
    join(wasmProofPath.artifactRoot, "proof-artifact.wasm"),
    await readFile(join(wasmProofPath.artifactRoot, "proof-artifact.r1cs")),
  );
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts({
        ...wasmProofPath.options,
        "proof-artifact": "proof-artifact.wasm",
      }),
    /proof artifact must be a \.r1cs artifact/u,
  );

  const badR1csSections = await writeNativeProverFixtureFiles();
  const badR1csSectionBytes = Buffer.from(
    await readFile(join(badR1csSections.artifactRoot, "proof-artifact.r1cs")),
  );
  badR1csSectionBytes.writeUInt32LE(badR1csSectionBytes.length, 16);
  await writeFile(
    join(badR1csSections.artifactRoot, "bad-proof-sections.r1cs"),
    badR1csSectionBytes,
  );
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts({
        ...badR1csSections.options,
        "proof-artifact": "bad-proof-sections.r1cs",
      }),
    /proof artifact \.r1cs section exceeds file size/u,
  );

  const badZkeyHeader = await writeNativeProverFixtureFiles();
  const badZkeyBytes = Buffer.from(
    await readFile(join(badZkeyHeader.artifactRoot, "proving-key.zkey")),
  );
  badZkeyBytes[0] = 0x78;
  await writeFile(
    join(badZkeyHeader.artifactRoot, "bad-proving-key.zkey"),
    badZkeyBytes,
  );
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts({
        ...badZkeyHeader.options,
        "proving-key": "bad-proving-key.zkey",
      }),
    /proving key must start with \.zkey magic bytes/u,
  );

  const badZkeySections = await writeNativeProverFixtureFiles();
  const badZkeySectionBytes = Buffer.from(
    await readFile(join(badZkeySections.artifactRoot, "proving-key.zkey")),
  );
  badZkeySectionBytes.writeUInt32LE(badZkeySectionBytes.length, 16);
  await writeFile(
    join(badZkeySections.artifactRoot, "bad-proving-key-sections.zkey"),
    badZkeySectionBytes,
  );
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts({
        ...badZkeySections.options,
        "proving-key": "bad-proving-key-sections.zkey",
      }),
    /proving key \.zkey section exceeds file size/u,
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

  const nonJsonVerifierKey = await writeNativeProverFixtureFiles({
    artifactByteOverrides: {
      verifierKey: Buffer.from("not a verifier json artifact", "utf8"),
    },
  });
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts(nonJsonVerifierKey.options),
    /verifier key must be valid duplicate-free JSON/u,
  );

  const mismatchedVerifierKey = await writeNativeProverFixtureFiles({
    artifactByteOverrides: {
      verifierMaterial: { verifierKeyHash: HASH_22 },
    },
  });
  await assert.rejects(
    () =>
      buildBscNativeEvmProverBundleFromArtifacts(mismatchedVerifierKey.options),
    /expectedVerifierKeyHash must match Solidity verifyingKeyHash\(\)/u,
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

test("BSC route-config refuses production-ready manifests with handoff placeholders", () => {
  const cases = [
    [
      { operatorNote: "TODO replace verifier material before launch" },
      /placeholder handoff material.*route manifest\.operatorNote/u,
    ],
    [
      {
        postDeployLiveEvidence: {
          operatorNote: "example verifier evidence must not ship",
        },
      },
      /placeholder handoff material.*route manifest\.postDeployLiveEvidence\.operatorNote/u,
    ],
    [
      {
        destinationRollout: {
          replaceMeVerifierKeyHash: HASH_22,
        },
      },
      /placeholder handoff material.*route manifest\.destinationRollout\.replaceMeVerifierKeyHash/u,
    ],
  ];
  for (const [overrides, pattern] of cases) {
    assert.throws(
      () =>
        buildBscTairaXorRouteConfigToml(
          productionReadyRouteManifest(overrides),
        ),
      pattern,
    );
  }
});

test("BSC canonical production output guard rejects diagnostic or draft material", () => {
  const canonicalEvidencePath = `${CANONICAL_BSC_PRODUCTION_ARTIFACT_ROOT}/taira-bsc-xor-deployment.evidence.json`;
  assert.equal(
    isCanonicalBscProductionArtifactPath(canonicalEvidencePath),
    true,
  );

  const cleanEvidence = buildDeploymentEvidence({
    tokenAddress: BSC_TOKEN_ADDRESS,
    bridgeAddress: BSC_BRIDGE_ADDRESS,
    sourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
    verifierAddress: BSC_VERIFIER_ADDRESS,
    verifierCodeHash: HASH_11,
    verifierKeyHash: HASH_22,
    readback: readyReadback(),
  });
  assert.match(
    bscCanonicalProductionOutputProblems(
      canonicalEvidencePath,
      {
        ...cleanEvidence,
        operatorNote: "TODO replace verifier material before launch",
      },
      "BSC deployment evidence",
    ).join(" "),
    /production handoff placeholder material.*operatorNote/u,
  );

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
      verifierKeyHash: DIAGNOSTIC_BSC_VERIFIER_KEY_HASH,
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
  assert.match(
    bscCanonicalProductionOutputProblems(
      `${CANONICAL_BSC_PRODUCTION_ARTIFACT_ROOT}/taira-bsc-xor-route.manifest.json`,
      productionReadyRouteManifest({
        operatorNote: "TODO replace verifier material before launch",
      }),
      "BSC route manifest",
    ).join(" "),
    /production handoff placeholder material.*operatorNote/u,
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
        operatorNote: "example verifier evidence must not ship",
      },
      "BSC native EVM prover bundle",
    ).join(" "),
    /production handoff placeholder material.*operatorNote/u,
  );
  const {
    verifier_key_artifact_hash: _dropVerifierKeyArtifactHash,
    ...legacyCompatBundle
  } = productionBundle;
  assert.match(
    bscCanonicalProductionOutputProblems(
      canonicalBundlePath,
      legacyCompatBundle,
      "BSC native EVM prover bundle",
    ).join(" "),
    /verifierKeyArtifactHash is required/u,
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
  assert.match(
    bscCanonicalProductionOutputProblems(
      canonicalBundlePath,
      {
        ...productionBundle,
        verifier_key_artifact_hash: productionBundle.proof_artifact_hash,
      },
      "BSC native EVM prover bundle",
    ).join(" "),
    /proofArtifactHash must be role-separated from verifierKeyArtifactHash/u,
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
    [
      { networkIdHex: ` ${BSC_TESTNET_NETWORK_ID_HEX}` },
      /networkIdHex.*canonical string/u,
    ],
    [
      { networkIdHex: BSC_TESTNET_NETWORK_ID_HEX.toUpperCase() },
      /networkIdHex.*canonical lowercase hex/u,
    ],
    [
      { destinationBinding: { networkIdHex: HASH_33 } },
      /networkIdHex.*aliases disagree/u,
    ],
    [{ counterpartyDomain: 1 }, /counterpartyDomain/u],
    [{ verifierTarget: "TronContract" }, /verifierTarget/u],
    [{ productionReady: "true" }, /productionReady must be true or false/u],
    [{ productionReady: 1 }, /productionReady must be true or false/u],
    [{ disabledReason: " disabled" }, /disabledReason.*canonical string/u],
    [{ disabledReason: 1 }, /disabledReason.*canonical string/u],
    [
      { disabledReason: "disabled", disabled_reason: "different" },
      /disabledReason aliases disagree/u,
    ],
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
      {
        sccpBscSourceBridgeAddress: undefined,
        sccp_tron_source_bridge_address: BSC_SOURCE_BRIDGE_ADDRESS,
      },
      /source bridge address.*must not use TRON aliases.*sccp_tron_source_bridge_address/u,
    ],
    [
      { bscVerifierAddress: BSC_VERIFIER_ADDRESS.replace(/^0x/u, "0X") },
      /verifier address.*canonical lowercase hex/u,
    ],
    [
      {
        bscVerifierAddress: undefined,
        tron_verifier_address: BSC_VERIFIER_ADDRESS,
      },
      /verifier address.*must not use TRON aliases.*tron_verifier_address/u,
    ],
    [
      {
        destinationRollout: {
          verifierIdentity: BSC_VERIFIER_ADDRESS.toUpperCase(),
        },
      },
      /verifier address.*canonical lowercase hex/u,
    ],
    [
      { bscBridgeAddress: BSC_TOKEN_ADDRESS },
      /bridge address aliases disagree|distinct/u,
    ],
    [
      { tokenAddress: BSC_SOURCE_BRIDGE_ADDRESS },
      /BSC token address must not use multiple aliases in route manifest/u,
    ],
    [
      { bscTokenAddress: BSC_TOKEN_ADDRESS, token_address: BSC_TOKEN_ADDRESS },
      /BSC token address must not use multiple aliases in route manifest/u,
    ],
    [
      { bridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS },
      /BSC bridge address must not use multiple aliases in route manifest/u,
    ],
    [
      {
        bscBridgeAddress: BSC_BRIDGE_ADDRESS,
        bridge_address: BSC_BRIDGE_ADDRESS,
      },
      /BSC bridge address must not use multiple aliases in route manifest/u,
    ],
    [
      {
        destinationRollout: {
          destinationBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
        },
      },
      /bridge address aliases disagree/u,
    ],
    [
      {
        sccpBscSourceBridgeAddress: BSC_SOURCE_BRIDGE_ADDRESS,
        source_bridge_address: BSC_SOURCE_BRIDGE_ADDRESS,
      },
      /BSC source bridge address must not use multiple aliases in route manifest/u,
    ],
    [
      {
        bscVerifierAddress: BSC_VERIFIER_ADDRESS,
        verifier_address: BSC_VERIFIER_ADDRESS,
      },
      /BSC verifier address must not use multiple aliases in route manifest/u,
    ],
    [
      { destinationRollout: { targetDomain: 1 } },
      /targetDomain aliases disagree between destinationRollout and destinationBinding/u,
    ],
    [
      { destinationRollout: { verifierBackend: "tron-groth16-bn254-v1" } },
      /verifier backend/u,
    ],
    [{ verifierCodeHash: HASH_77 }, /verifierCodeHash aliases disagree/u],
    [
      { destinationRollout: { verifierCodeHash: HASH_44.toUpperCase() } },
      /verifierCodeHash.*canonical lowercase hex/u,
    ],
    [{ verifierKeyHash: HASH_77 }, /verifierKeyHash aliases disagree/u],
    [
      { destinationRollout: { verifierKeyHash: HASH_55.toUpperCase() } },
      /verifierKeyHash.*canonical lowercase hex/u,
    ],
    [
      { verifierCodeHash: HASH_11, verifier_code_hash: HASH_11 },
      /verifierCodeHash must not use multiple aliases in route manifest/u,
    ],
    [
      { verifierKeyHash: HASH_22, verifier_key_hash: HASH_22 },
      /verifierKeyHash must not use multiple aliases in route manifest/u,
    ],
    [
      { destinationRollout: { destinationBindingHash: HASH_33 } },
      /binding hash/u,
    ],
    [
      {
        destinationRollout: {
          destinationBindingHash:
            routeManifest().destinationRollout.destinationBindingHash.toUpperCase(),
        },
      },
      /destination binding hash.*canonical lowercase hex/u,
    ],
    [
      { destinationBindingHash: HASH_77 },
      /destination binding hash aliases disagree/u,
    ],
    [
      { destinationBindingKey: "stale-binding-key" },
      /destination binding key aliases disagree/u,
    ],
    [
      {
        destinationBinding: {
          bindingHash: bindingHash(),
          binding_hash: bindingHash(),
        },
      },
      /destinationBindingHash must not use multiple aliases in route manifest destinationBinding/u,
    ],
    [{ proofArtifactHash: HASH_77 }, /proofArtifactHash aliases disagree/u],
    [{ provingKeyHash: HASH_77 }, /provingKeyHash aliases disagree/u],
    [
      { proofArtifactHash: HASH_44, proof_artifact_hash: HASH_44 },
      /proofArtifactHash must not use multiple aliases in route manifest/u,
    ],
    [
      {
        destinationRollout: {
          provingKeyHash: HASH_55,
          proving_key_hash: HASH_55,
        },
      },
      /provingKeyHash must not use multiple aliases in route manifest destinationRollout/u,
    ],
    [
      {
        nativeEvmProverBundleHash: HASH_66,
        native_evm_prover_bundle_hash: HASH_66,
      },
      /nativeEvmProverBundleHash must not use multiple aliases in route manifest/u,
    ],
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
      { settlement: { contractAddress: " contract-v1" } },
      /settlement\.contractAddress.*canonical string/u,
    ],
    [
      { settlement: { contractAddress: 1 } },
      /settlement\.contractAddress.*canonical string/u,
    ],
    [
      {
        settlement: {
          contractAddress: "contract-v1",
          contract_address: "contract-v2",
        },
      },
      /settlement\.contractAddress aliases disagree/u,
    ],
    [
      { settlement: { contractAlias: " taira-bsc-xor" } },
      /settlement\.contractAlias.*canonical string/u,
    ],
    [
      { settlement: { contractAlias: 1 } },
      /settlement\.contractAlias.*canonical string/u,
    ],
    [
      {
        settlement: {
          contractAlias: "taira-bsc-xor",
          contract_alias: "taira-bsc-xor-v2",
        },
      },
      /settlement\.contractAlias aliases disagree/u,
    ],
    [
      { sourceBridgeAddress: BSC_BRIDGE_ADDRESS },
      /BSC source bridge address must not use multiple aliases in route manifest/u,
    ],
    [
      { destinationVerifierAddress: BSC_BRIDGE_ADDRESS },
      /BSC verifier address must not use multiple aliases in route manifest/u,
    ],
    [
      { postDeployLiveEvidence: { full_toml_ready: true } },
      /fullTomlReady must not use multiple aliases in route manifest postDeployLiveEvidence/u,
    ],
    [
      { postDeployLiveEvidence: { fullTomlReady: "true" } },
      /postDeployLiveEvidence\.fullTomlReady\.fullTomlReady must be boolean/u,
    ],
    [
      { postDeployLiveEvidence: { fullTomlReady: 1 } },
      /postDeployLiveEvidence\.fullTomlReady\.fullTomlReady must be boolean/u,
    ],
    [
      { postDeployLiveEvidence: { source_bridge_config_hash: HASH_77 } },
      /sourceBridgeConfigHash must not use multiple aliases in route manifest postDeployLiveEvidence/u,
    ],
    [
      {
        postDeployLiveEvidence: {
          routeCanaryExplorerUrl: ROUTE_CANARY_EXPLORER_URL,
          route_canary_explorer_url: ROUTE_CANARY_EXPLORER_URL,
        },
      },
      /routeCanaryExplorerUrl must not use multiple aliases in route manifest postDeployLiveEvidence/u,
    ],
    [
      { postDeployLiveEvidence: { sourceEventTransactionId: ` ${HASH_55}` } },
      /sourceEventTransactionId.*canonical string/u,
    ],
    [
      {
        postDeployLiveEvidence: {
          sourceEventTransactionId: HASH_55.toUpperCase(),
        },
      },
      /sourceEventTransactionId.*canonical lowercase hex/u,
    ],
    [
      { postDeployLiveEvidence: { offlineFullTomlSha256: `${HASH_33} ` } },
      /offlineFullTomlSha256.*canonical string/u,
    ],
    [
      {
        postDeployLiveEvidence: {
          offlineFullTomlSha256: HASH_33.toUpperCase(),
        },
      },
      /offlineFullTomlSha256.*canonical lowercase hex/u,
    ],
    [
      {
        postDeployLiveEvidence: {
          sourceEventTransactionUrl: ROUTE_CANARY_EXPLORER_URL,
        },
      },
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
  assert.equal(result.renderedTomlSha256, sha256Hex(Buffer.from(toml, "utf8")));
  assert.equal(result.offlineFullTomlSha256, null);
  assert.match(toml, /route_id = "taira_bsc_xor"/u);
  assert.match(
    toml,
    /sccp_bsc_source_bridge_address = "0x3333333333333333333333333333333333333333"/u,
  );
  assert.match(
    toml,
    /sccp_bsc_destination_verifier_address = "0x4444444444444444444444444444444444444444"/u,
  );
  assert.doesNotMatch(toml, /(^|\n)source_bridge_address =/u);
  assert.doesNotMatch(toml, /(^|\n)destination_verifier_address =/u);
  assert.doesNotMatch(toml, /(^|\n)tron_verifier_address =/u);
});

test("BSC route-config command reports the exact merged full-config hash", async () => {
  const dir = await mkdtemp(join(tmpdir(), "iroha-bsc-route-config-full-"));
  const manifestPath = join(dir, "manifest.json");
  const baseConfigPath = join(dir, "base.toml");
  const out = join(dir, "full-config.toml");
  await writeFile(
    manifestPath,
    `${JSON.stringify(routeManifest(), null, 2)}\n`,
  );
  await writeFile(
    baseConfigPath,
    [
      "[network]",
      'address = "127.0.0.1:1337"',
      "",
      "[zk]",
      "sccp_allow_unready_transparent_proofs = false",
      "",
      "[torii]",
      'address = "127.0.0.1:8080"',
      "",
    ].join("\n"),
  );

  const result = await main([
    "route-config",
    "--manifest",
    manifestPath,
    "--base-config",
    baseConfigPath,
    "--out",
    out,
    "--allow-unready",
    "true",
  ]);
  const toml = await readFile(out, "utf8");
  const expectedHash = sha256Hex(Buffer.from(toml, "utf8"));

  assert.equal(result.ok, true);
  assert.equal(result.mode, "merged-full-config");
  assert.equal(result.baseConfig, baseConfigPath);
  assert.equal(result.renderedTomlSha256, expectedHash);
  assert.equal(result.offlineFullTomlSha256, expectedHash);
  assert.match(toml, /\[network\]/u);
  assert.match(toml, /\[\[zk\.sccp_route_manifests\]\]/u);
});

test("BSC route-config command accepts redacted secret placeholders in public base configs", async () => {
  const dir = await mkdtemp(join(tmpdir(), "iroha-bsc-route-config-redacted-"));
  const manifestPath = join(dir, "manifest.json");
  const baseConfigPath = join(dir, "base.toml");
  const out = join(dir, "full-config.toml");
  await writeFile(
    manifestPath,
    `${JSON.stringify(routeManifest(), null, 2)}\n`,
  );
  await writeFile(
    baseConfigPath,
    [
      "[network]",
      'address = "127.0.0.1:1337"',
      "",
      "[account]",
      'private_key = "<redacted>"',
      'validator_private_key = "REPLACE_WITH_VALIDATOR_PRIVATE_KEY"',
      "",
      "[torii]",
      'identity_private_key = "<runtime-only>"',
      'address = "127.0.0.1:8080"',
      "",
    ].join("\n"),
  );

  const result = await main([
    "route-config",
    "--manifest",
    manifestPath,
    "--base-config",
    baseConfigPath,
    "--out",
    out,
    "--allow-unready",
    "true",
  ]);
  const toml = await readFile(out, "utf8");

  assert.equal(result.ok, true);
  assert.equal(result.mode, "merged-full-config");
  assert.match(toml, /private_key = "<redacted>"/u);
  assert.match(
    toml,
    /validator_private_key = "REPLACE_WITH_VALIDATOR_PRIVATE_KEY"/u,
  );
  assert.match(toml, /identity_private_key = "<runtime-only>"/u);
});

test("BSC route-config command writes non-self-referential offline full TOML evidence", async () => {
  const dir = await mkdtemp(join(tmpdir(), "iroha-bsc-route-config-evidence-"));
  const manifestPath = join(dir, "manifest.json");
  const baseConfigPath = join(dir, "base.toml");
  const out = join(dir, "full-config.toml");
  const evidenceOut = join(dir, "full-config.evidence.json");
  await writeFile(
    manifestPath,
    `${JSON.stringify(productionReadyRouteManifest(), null, 2)}\n`,
  );
  await writeFile(
    baseConfigPath,
    [
      "[network]",
      'address = "127.0.0.1:1337"',
      'operator_marker = "base-config-marker-must-not-leak"',
      "",
      "[torii]",
      'address = "127.0.0.1:8080"',
      "",
    ].join("\n"),
  );

  const result = await main([
    "route-config",
    "--manifest",
    manifestPath,
    "--base-config",
    baseConfigPath,
    "--out",
    out,
    "--write-offline-full-toml-evidence",
    evidenceOut,
  ]);
  const toml = await readFile(out, "utf8");
  const renderedHash = sha256Hex(Buffer.from(toml, "utf8"));
  const canonicalToml = `${toml
    .split(/\r?\n/u)
    .filter(
      (line) => !/^\s*post_deploy_offline_full_toml_sha256\s*=/u.test(line),
    )
    .join("\n")
    .replace(/\s*$/u, "")}\n`;
  const expectedOfflineHash = sha256Hex(Buffer.from(canonicalToml, "utf8"));
  const evidence = JSON.parse(await readFile(evidenceOut, "utf8"));

  assert.equal(result.ok, true);
  assert.equal(result.renderedTomlSha256, renderedHash);
  assert.equal(result.offlineFullTomlSha256, expectedOfflineHash);
  assert.notEqual(result.offlineFullTomlSha256, result.renderedTomlSha256);
  assert.equal(result.wroteOfflineFullTomlEvidence, evidenceOut);
  assert.equal(
    result.offlineFullTomlHashMode,
    "sha256:merged-full-config-without-post_deploy_offline_full_toml_sha256",
  );
  assert.equal(evidence.fullTomlReady, true);
  assert.equal(evidence.renderedTomlSha256, renderedHash);
  assert.equal(evidence.hashInputSha256, expectedOfflineHash);
  assert.equal(evidence.offlineFullTomlSha256, expectedOfflineHash);
  assert.equal(
    evidence.postDeployLiveEvidence.offlineFullTomlSha256,
    expectedOfflineHash,
  );
  assert.equal(
    JSON.stringify(evidence).includes("base-config-marker-must-not-leak"),
    false,
  );

  const finalManifestPath = join(dir, "manifest.final.json");
  const finalOut = join(dir, "full-config.final.toml");
  await writeFile(
    finalManifestPath,
    `${JSON.stringify(
      productionReadyRouteManifest({
        postDeployLiveEvidence: { offlineFullTomlSha256: expectedOfflineHash },
      }),
      null,
      2,
    )}\n`,
  );
  const finalResult = await main([
    "route-config",
    "--manifest",
    finalManifestPath,
    "--base-config",
    baseConfigPath,
    "--out",
    finalOut,
    "--write-offline-full-toml-evidence",
    join(dir, "full-config.final.evidence.json"),
  ]);

  assert.equal(finalResult.offlineFullTomlSha256, expectedOfflineHash);
});

test("BSC route-config command refuses offline full TOML evidence without full config mode", async () => {
  const dir = await mkdtemp(join(tmpdir(), "iroha-bsc-route-config-no-base-"));
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
        "--out",
        join(dir, "route.toml"),
        "--allow-unready",
        "true",
        "--write-offline-full-toml-evidence",
        join(dir, "full-config.evidence.json"),
      ]),
    /--write-offline-full-toml-evidence requires --base-config/u,
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

test("BSC route-config command rejects duplicate JSON keys in manifests", async () => {
  const dir = await mkdtemp(
    join(tmpdir(), "iroha-bsc-route-config-duplicates-"),
  );
  const manifestPath = join(dir, "manifest.duplicate.json");
  await writeFile(
    manifestPath,
    `${JSON.stringify(routeManifest(), null, 2).replace(
      '"routeId": "taira_bsc_xor"',
      '"routeId": "shadow",\n  "routeId": "taira_bsc_xor"',
    )}\n`,
  );

  await assert.rejects(
    () =>
      main([
        "route-config",
        "--manifest",
        manifestPath,
        "--out",
        join(dir, "route.toml"),
        "--allow-unready",
        "true",
      ]),
    /BSC route manifest is not valid JSON: BSC route manifest contains a duplicate JSON object key/u,
  );
});

test("BSC route-config command rejects non-object JSON manifests", async () => {
  const dir = await mkdtemp(
    join(tmpdir(), "iroha-bsc-route-config-non-object-"),
  );
  const manifestPath = join(dir, "manifest.array.json");
  await writeFile(manifestPath, "[]\n");

  await assert.rejects(
    () =>
      main([
        "route-config",
        "--manifest",
        manifestPath,
        "--out",
        join(dir, "route.toml"),
        "--allow-unready",
        "true",
      ]),
    /BSC route manifest is not valid JSON: BSC route manifest must be a JSON object/u,
  );
});

test("BSC route-config command rejects oversized base TAIRA configs before merging", async () => {
  const dir = await mkdtemp(
    join(tmpdir(), "iroha-bsc-route-config-oversized-"),
  );
  const manifestPath = join(dir, "manifest.json");
  const baseConfigPath = join(dir, "base-config.toml");
  await writeFile(
    manifestPath,
    `${JSON.stringify(routeManifest(), null, 2)}\n`,
  );
  await writeFile(baseConfigPath, "");
  await truncate(baseConfigPath, SCCP_BSC_TEXT_INPUT_MAX_BYTES + 1);

  await assert.rejects(
    () =>
      main([
        "route-config",
        "--manifest",
        manifestPath,
        "--base-config",
        baseConfigPath,
        "--out",
        join(dir, "route.toml"),
        "--allow-unready",
        "true",
      ]),
    /base TAIRA config could not be read: path is .*maximum allowed/u,
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

test("BSC deploy command rejects malformed boolean switches before network use", async () => {
  for (const value of [" TRUE", "true ", "TRUE", "1", "yes", "on", "false "]) {
    await assert.rejects(
      () =>
        main([
          "deploy",
          "--verifier",
          "missing-verifier.json",
          "--broadcast",
          value,
        ]),
      /--broadcast must be true or false/u,
    );
  }

  await assert.rejects(
    () =>
      main([
        "deploy",
        "--bsc-network",
        "mainnet",
        "--verifier",
        "missing-verifier.json",
        "--broadcast",
        "true",
        "--confirm-network",
        "taira_bsc_xor:mainnet",
        "--confirm-mainnet",
        " TRUE",
      ]),
    /--confirm-mainnet must be true or false/u,
  );

  const dir = await mkdtemp(join(tmpdir(), "bsc-deploy-boolean-"));
  const diagnosticVerifierFile = join(dir, "diagnostic-verifier.json");
  await writeFile(
    diagnosticVerifierFile,
    JSON.stringify(
      verifierMaterial({
        schema: "iroha-sccp-bsc-testnet-diagnostic-verifier-key/v1",
        warning: "Generated diagnostic BSC testnet verifier material.",
        verifierKeyHash: DIAGNOSTIC_BSC_VERIFIER_KEY_HASH,
      }),
    ),
    "utf8",
  );
  await assert.rejects(
    () =>
      main([
        "deploy",
        "--verifier",
        diagnosticVerifierFile,
        "--broadcast",
        "true",
        "--confirm-testnet",
        "taira_bsc_xor",
        "--allow-diagnostic-verifier",
        " TRUE",
      ]),
    /--allow-diagnostic-verifier must be true or false/u,
  );

  const envName = "SCCP_BSC_TEST_DEPLOYER_PRIVATE_KEY";
  const previous = process.env[envName];
  const verifierFile = join(dir, "verifier.json");
  await writeFile(verifierFile, JSON.stringify(verifierMaterial()), "utf8");
  try {
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
          "--allow-local-rpc",
          " TRUE",
        ]),
      /--allow-local-rpc must be true or false/u,
    );
  } finally {
    if (previous === undefined) {
      delete process.env[envName];
    } else {
      process.env[envName] = previous;
    }
  }
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

test("BSC deployment helper help documents production network confirmations", async () => {
  const result = await main(["--help"]);
  assert.match(result.help, /--bsc-network testnet\|mainnet/u);
  assert.match(
    result.help,
    /--confirm-network taira_bsc_xor:testnet\|taira_bsc_xor:mainnet/u,
  );
  assert.match(result.help, /\[--confirm-mainnet true\]/u);
  assert.match(result.help, /--route-manifest .* --artifact-root/u);
  assert.match(result.help, /--audit-no-wasm-no-remote-scan/u);
  assert.match(result.help, /requirements \[--bsc-network testnet\|mainnet\]/u);
  assert.match(
    result.help,
    /Diagnostic verifier material is refused\s+by deploy/u,
  );
  assert.doesNotMatch(result.help, /deploy .*--confirm-testnet/u);
});

test("BSC production requirements expose network-specific public handoff inputs", async () => {
  const testnet = await main(["requirements", "--bsc-network", "testnet"]);
  assert.equal(testnet.schema, PRODUCTION_REQUIREMENTS_SCHEMA);
  assert.equal(testnet.routeId, "taira_bsc_xor");
  assert.equal(testnet.assetKey, "xor");
  assert.equal(testnet.bsc.network, "testnet");
  assert.equal(testnet.bsc.chainIdHex, "0x61");
  assert.match(testnet.commands.deploy, /--bsc-network testnet/u);
  assert.match(
    testnet.commands.deploy,
    /--confirm-network taira_bsc_xor:testnet/u,
  );
  assert.match(
    testnet.commands.requirements,
    /requirements --bsc-network testnet --out artifacts\/sccp-bsc\/taira-bsc-xor-production-requirements\.json/u,
  );
  for (const required of [
    "--evidence artifacts/sccp-bsc/taira-bsc-xor-deployment.evidence.json",
    "--taira-contract artifacts/sccp-bsc/taira-bsc-xor-burn-record.contract.json",
    "--settlement-asset-definition-id <canonical-asset-definition-id>",
    "--native-prover-bundle artifacts/sccp-bsc/bsc-testnet-native-evm-prover-bundle.json",
    "--source-bridge-config-hash <0x...>",
    "--source-event-transaction-id <0x...>",
    "--source-event-explorer-url <url>",
    "--route-canary-evidence-hash <0x...>",
    "--route-canary-transaction-id <0x...>",
    "--route-canary-explorer-url <url>",
    "--full-toml-ready true",
    "--offline-full-toml-evidence artifacts/sccp-bsc/taira-bsc-xor-route.full-taira-config.evidence.json",
    "--production-ready true",
    "--live-readback-checked true",
    "--confirm-testnet taira_bsc_xor",
    "--out artifacts/sccp-bsc/taira-bsc-xor-route.manifest.json",
  ]) {
    assert.ok(
      testnet.commands.routeManifest.includes(required),
      `routeManifest command should include ${required}`,
    );
  }
  assert.doesNotMatch(testnet.commands.deploy, /--confirm-mainnet true/u);
  assert.doesNotMatch(testnet.commands.deploy, /--confirm-testnet/u);
  assert.deepEqual(
    testnet.inputs
      .filter((entry) =>
        [
          "production-groth16-verifier-key-json",
          "testnet-funded-bsc-deployer",
          "testnet-bsc-deployment-evidence",
          "taira-burn-record-contract",
          "canonical-settlement-asset-definition-id",
          "post-deploy-live-evidence",
          "deployed-taira-base-config",
          "offline-full-toml-evidence",
          "burn-record-proof-artifact",
          "burn-record-proving-key",
          "cross-sdk-parity-report",
          "audit-no-wasm-no-remote-scan",
        ].includes(entry.id),
      )
      .map((entry) => entry.id),
    [
      "production-groth16-verifier-key-json",
      "testnet-funded-bsc-deployer",
      "testnet-bsc-deployment-evidence",
      "taira-burn-record-contract",
      "canonical-settlement-asset-definition-id",
      "post-deploy-live-evidence",
      "deployed-taira-base-config",
      "offline-full-toml-evidence",
      "burn-record-proof-artifact",
      "burn-record-proving-key",
      "cross-sdk-parity-report",
      "audit-no-wasm-no-remote-scan",
    ],
  );
  assert.deepEqual(testnet.requiredReports, [
    "route-preflight",
    "peer-config-audit",
    "smoke-readiness",
    "production-material-inventory",
    "live-ui-video-proof",
  ]);
  assert.deepEqual(testnet.deniedVerifierKeyHashes, [
    DIAGNOSTIC_BSC_VERIFIER_KEY_HASH,
  ]);
  assert.doesNotMatch(
    JSON.stringify(testnet),
    /privateKey|private_key|mnemonic|seed phrase|password/u,
  );

  const mainnet = bscProductionRequirements({ "bsc-network": "mainnet" });
  assert.equal(mainnet.schema, PRODUCTION_REQUIREMENTS_SCHEMA);
  assert.equal(mainnet.bsc.network, "mainnet");
  assert.equal(mainnet.bsc.chainIdHex, "0x38");
  assert.match(mainnet.commands.deploy, /--bsc-network mainnet/u);
  assert.match(
    mainnet.commands.deploy,
    /--confirm-network taira_bsc_xor:mainnet/u,
  );
  assert.match(
    mainnet.commands.requirements,
    /requirements --bsc-network mainnet --out artifacts\/sccp-bsc\/taira-bsc-mainnet-xor-production-requirements\.json/u,
  );
  assert.match(mainnet.commands.routeManifest, /--confirm-mainnet true/u);
  assert.match(
    mainnet.commands.routeManifest,
    /--confirm-network taira_bsc_xor/u,
  );
  assert.doesNotMatch(mainnet.commands.routeManifest, /--confirm-testnet/u);
  for (const required of [
    "--evidence artifacts/sccp-bsc/taira-bsc-mainnet-xor-deployment.evidence.json",
    "--native-prover-bundle artifacts/sccp-bsc/bsc-mainnet-native-evm-prover-bundle.json",
    "--out artifacts/sccp-bsc/taira-bsc-mainnet-xor-route.manifest.json",
  ]) {
    assert.ok(
      mainnet.commands.routeManifest.includes(required),
      `mainnet routeManifest command should include ${required}`,
    );
  }
  for (const required of [
    "--route-manifest artifacts/sccp-bsc/taira-bsc-mainnet-xor-route.manifest.json",
    "--out artifacts/sccp-bsc/bsc-mainnet-native-evm-prover-bundle.json",
    "--attach-route-manifest-out artifacts/sccp-bsc/taira-bsc-mainnet-xor-route.manifest.json",
  ]) {
    assert.ok(
      mainnet.commands.nativeProverBundle.includes(required),
      `mainnet nativeProverBundle command should include ${required}`,
    );
  }
  assert.match(
    mainnet.commands.routeConfig,
    /--manifest artifacts\/sccp-bsc\/taira-bsc-mainnet-xor-route\.manifest\.json/u,
  );
  assert.match(
    mainnet.commands.routeConfig,
    /--write-offline-full-toml-evidence artifacts\/sccp-bsc\/taira-bsc-mainnet-xor-route\.full-taira-config\.evidence\.json/u,
  );
  assert.match(JSON.stringify(mainnet.inputs), /offline-full-toml-evidence/u);
  assert.match(mainnet.commands.deploy, /--confirm-mainnet true/u);
  assert.doesNotMatch(JSON.stringify(mainnet), /testnet-funded-bsc-deployer/u);
  assert.doesNotMatch(
    JSON.stringify(mainnet),
    /bsc-testnet-native-evm-prover-bundle\.json/u,
  );
  assert.doesNotMatch(
    JSON.stringify(mainnet),
    /artifacts\/sccp-bsc\/taira-bsc-xor-route\.manifest\.json/u,
  );
  assert.match(JSON.stringify(mainnet), /mainnet-funded-bsc-deployer/u);
});

test("BSC production requirements command writes public artifact without deployer secrets", async () => {
  const dir = await mkdtemp(join(tmpdir(), "iroha-bsc-requirements-"));
  const out = join(dir, "requirements.json");
  const previousPrivateKey = process.env.SCCP_BSC_DEPLOYER_PRIVATE_KEY;
  process.env.SCCP_BSC_DEPLOYER_PRIVATE_KEY = `0x${"12".repeat(32)}`;
  try {
    const result = await main([
      "requirements",
      "--bsc-network",
      "mainnet",
      "--out",
      out,
    ]);
    assert.equal(result.ok, true);
    assert.equal(result.wrote, out);
    assert.equal(result.schema, PRODUCTION_REQUIREMENTS_SCHEMA);
    assert.equal(result.bscNetwork, "mainnet");
    assert.equal(result.inputCount, 25);
    assert.deepEqual(result.requiredReports, [
      "route-preflight",
      "peer-config-audit",
      "smoke-readiness",
      "production-material-inventory",
      "live-ui-video-proof",
    ]);

    const written = JSON.parse(await readFile(out, "utf8"));
    assert.equal(written.schema, PRODUCTION_REQUIREMENTS_SCHEMA);
    assert.equal(written.bsc.network, "mainnet");
    assert.match(written.commands.deploy, /--confirm-mainnet true/u);
    assert.doesNotMatch(
      JSON.stringify(written),
      /0x1212121212121212121212121212121212121212121212121212121212121212|privateKey|private_key|mnemonic|seed phrase|password/u,
    );
  } finally {
    if (previousPrivateKey === undefined) {
      delete process.env.SCCP_BSC_DEPLOYER_PRIVATE_KEY;
    } else {
      process.env.SCCP_BSC_DEPLOYER_PRIVATE_KEY = previousPrivateKey;
    }
  }
});

test("BSC deployment helper self-test covers public evidence and secret scanning", async () => {
  assert.deepEqual(await main(["self-test"]), { ok: true });
});
