#!/usr/bin/env node
// Purpose: compile, deploy, configure, and evidence-check the BSC testnet and
// mainnet contracts for the TAIRA XOR SCCP bridge without persisting operator
// keys.
// Safe default: no transaction is broadcast unless the command includes
// `--broadcast true --bsc-network testnet --confirm-network taira_bsc_xor:testnet`.
//
// Prerequisites:
// - Node.js 18+.
// - `solc` and `ethers` on NODE_PATH for compile/deploy/evidence commands.
// - A funded BSC deployer key supplied only through an environment
//   variable such as SCCP_BSC_DEPLOYER_PRIVATE_KEY.
import { createRequire } from "node:module";
import {
  lstat,
  mkdir,
  readFile,
  realpath,
  rename,
  writeFile,
} from "node:fs/promises";
import { dirname, extname, isAbsolute, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { sha256 } from "../javascript/iroha_js/node_modules/@noble/hashes/sha256.js";
import { keccak_256 } from "../javascript/iroha_js/node_modules/@noble/hashes/sha3.js";
import {
  SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
  SCCP_BSC_MAINNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
  SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
  SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
  SCCP_NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1,
  parseBscTestnetNativeEvmProverParityFixture,
  parseBscMainnetNativeEvmProverParityFixture,
  parseBscTestnetNativeEvmProverSelfTestFixture,
  parseBscMainnetNativeEvmProverSelfTestFixture,
  validateBscTestnetNativeEvmProverBundle,
  validateBscMainnetNativeEvmProverBundle,
  verifyBscTestnetNativeEvmProverArtifactsFromBundle,
  verifyBscMainnetNativeEvmProverArtifactsFromBundle,
} from "../javascript/iroha_js/src/sccp.js";

const requireFromScript = createRequire(import.meta.url);
const requireFromCwd = createRequire(`${resolve("noop.js")}`);
const SCRIPT_PATH = fileURLToPath(import.meta.url);
const REPO_ROOT = resolve(dirname(SCRIPT_PATH), "..");
const textEncoder = new TextEncoder();

export const ROUTE_ID = "taira_bsc_xor";
export const ASSET_KEY = "xor";
export const CONFIRMATION_TEXT = ROUTE_ID;
export const SCCP_DOMAIN_SORA = 0;
export const SCCP_DOMAIN_BSC = 2;
export const BSC_TESTNET_CHAIN_ID_HEX = "0x61";
export const BSC_TESTNET_NETWORK_ID_HEX =
  "0x0000000000000000000000000000000000000000000000000000000000000061";
export const DEFAULT_BSC_RPC_URL =
  "https://data-seed-prebsc-1-s1.bnbchain.org:8545";
export const BSC_MAINNET_CHAIN_ID_HEX = "0x38";
export const BSC_MAINNET_NETWORK_ID_HEX =
  "0x0000000000000000000000000000000000000000000000000000000000000038";
export const DEFAULT_BSC_MAINNET_RPC_URL = "https://bsc-dataseed.bnbchain.org";
export const BSC_NETWORK_PROFILES = Object.freeze({
  testnet: Object.freeze({
    key: "testnet",
    chain: "bsc-testnet",
    label: "BSC testnet",
    confirmNetwork: `${ROUTE_ID}:testnet`,
    chainIdHex: BSC_TESTNET_CHAIN_ID_HEX,
    networkIdHex: BSC_TESTNET_NETWORK_ID_HEX,
    defaultRpcUrl: DEFAULT_BSC_RPC_URL,
    explorerUrl: "https://testnet.bscscan.com",
    explorerHost: "testnet.bscscan.com",
    deploymentEvidenceOut:
      "artifacts/sccp-bsc/taira-bsc-xor-deployment.evidence.json",
    routeManifestOut: "artifacts/sccp-bsc/taira-bsc-xor-route.manifest.json",
    routeConfigOut: "artifacts/sccp-bsc/taira-bsc-xor-route.torii.toml",
    routeFullConfigOut:
      "artifacts/sccp-bsc/taira-bsc-xor-route.full-taira-config.toml",
    routeFullConfigEvidenceOut:
      "artifacts/sccp-bsc/taira-bsc-xor-route.full-taira-config.evidence.json",
    nativeBundleOut:
      "artifacts/sccp-bsc/bsc-testnet-native-evm-prover-bundle.json",
  }),
  mainnet: Object.freeze({
    key: "mainnet",
    chain: "bsc-mainnet",
    label: "BSC mainnet",
    confirmNetwork: `${ROUTE_ID}:mainnet`,
    chainIdHex: BSC_MAINNET_CHAIN_ID_HEX,
    networkIdHex: BSC_MAINNET_NETWORK_ID_HEX,
    defaultRpcUrl: DEFAULT_BSC_MAINNET_RPC_URL,
    explorerUrl: "https://bscscan.com",
    explorerHost: "bscscan.com",
    deploymentEvidenceOut:
      "artifacts/sccp-bsc/taira-bsc-mainnet-xor-deployment.evidence.json",
    routeManifestOut:
      "artifacts/sccp-bsc/taira-bsc-mainnet-xor-route.manifest.json",
    routeConfigOut: "artifacts/sccp-bsc/taira-bsc-mainnet-xor-route.torii.toml",
    routeFullConfigOut:
      "artifacts/sccp-bsc/taira-bsc-mainnet-xor-route.full-taira-config.toml",
    routeFullConfigEvidenceOut:
      "artifacts/sccp-bsc/taira-bsc-mainnet-xor-route.full-taira-config.evidence.json",
    nativeBundleOut:
      "artifacts/sccp-bsc/bsc-mainnet-native-evm-prover-bundle.json",
  }),
});
export const BSC_EVM_GROTH16_BACKEND = "evm-groth16-bn254-v1";
export const SCCP_PROOF_FAMILY_STARK_FRI = "stark-fri-v1";
export const SCCP_BSC_DIAGNOSTIC_VERIFIER_KEY_HASHES = new Set([
  "0x9ef8067d260532f88e60cfa4b458fe678fc46b9c242de18fc91ba646e0857fc4",
]);
const SMOKE_FIXTURE_G1 = Object.freeze(["1", "2"]);
const SMOKE_FIXTURE_G2 = Object.freeze([
  "10857046999023057135944570762232829481370756359578518086990519993285655852781",
  "11559732032986387107991004021392285783925812861821192530917403151452391805634",
  "8495653923123431417604973247489272438418190587263600148770280649306958101930",
  "4082367875863433681332203403145435568316851327593401208105741076214120093531",
]);
const SMOKE_FIXTURE_IC = Object.freeze(
  Array.from({ length: 10 }, () => SMOKE_FIXTURE_G1).flat(),
);
const BN254_BASE_FIELD_MODULUS = BigInt(
  "21888242871839275222246405745257275088696311157297823662689037894645226208583",
);
const BN254_TWIST_B_COEFFICIENT = Object.freeze([
  BigInt(
    "19485874751759354771024239261021720505790618469301721065564631296452457478373",
  ),
  BigInt(
    "266929791119991161246907387137283842545076965332900288569378510910307636690",
  ),
]);
export const DEPLOYMENT_EVIDENCE_SCHEMA =
  "iroha-sccp-bsc-taira-xor-deployment-evidence/v1";
export const ROUTE_MANIFEST_SCHEMA =
  "iroha-sccp-taira-xor-route-manifest-draft/v1";
export const OFFLINE_FULL_TOML_EVIDENCE_SCHEMA =
  "iroha-sccp-bsc-taira-xor-offline-full-toml-evidence/v1";
export const PRODUCTION_REQUIREMENTS_SCHEMA =
  "iroha-sccp-bsc-taira-xor-production-requirements/v1";
export const TAIRA_BURN_RECORD_CONTRACT_SCHEMA =
  "iroha-sccp-taira-xor-burn-record-contract/v1";
export const DEFAULT_ARTIFACTS_OUT = "artifacts/sccp-bsc/contracts";
export const DEFAULT_EVIDENCE_OUT =
  "artifacts/sccp-bsc/taira-bsc-xor-deployment.evidence.json";
export const DEFAULT_TAIRA_BURN_RECORD_CONTRACT_OUT =
  "artifacts/sccp-bsc/taira-bsc-xor-burn-record.contract.json";
export const DEFAULT_ROUTE_MANIFEST_OUT =
  "artifacts/sccp-bsc/taira-bsc-xor-route.manifest.json";
export const DEFAULT_ROUTE_CONFIG_OUT =
  "artifacts/sccp-bsc/taira-bsc-xor-route.torii.toml";
export const DEFAULT_ROUTE_FULL_CONFIG_OUT =
  "artifacts/sccp-bsc/taira-bsc-xor-route.full-taira-config.toml";
export const DEFAULT_ROUTE_FULL_CONFIG_EVIDENCE_OUT =
  "artifacts/sccp-bsc/taira-bsc-xor-route.full-taira-config.evidence.json";
export const DEFAULT_NATIVE_EVM_PROVER_BUNDLE_OUT =
  "artifacts/sccp-bsc/bsc-testnet-native-evm-prover-bundle.json";
export const DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT =
  "artifacts/sccp-bsc/native-prover";
export const DEFAULT_PRODUCTION_REQUIREMENTS_OUT =
  "artifacts/sccp-bsc/taira-bsc-xor-production-requirements.json";
export const CANONICAL_BSC_PRODUCTION_ARTIFACT_ROOT = "artifacts/sccp-bsc";
export const DEFAULT_PRIVATE_KEY_ENV = "SCCP_BSC_DEPLOYER_PRIVATE_KEY";
export const TAIRA_BURN_RECORD_ARTIFACT_MIN_BYTES = 32;
export const TAIRA_BURN_RECORD_ARTIFACT_MAX_BYTES = 8 * 1024 * 1024;
export const TAIRA_BURN_RECORD_PRODUCTION_ARTIFACT_MIN_BYTES = 256;
export const SCCP_BSC_JSON_INPUT_MAX_BYTES = 12 * 1024 * 1024;
export const SCCP_BSC_TEXT_INPUT_MAX_BYTES = 8 * 1024 * 1024;
export const SCCP_BSC_BINARY_ARTIFACT_INPUT_MAX_BYTES = 512 * 1024 * 1024;

const DESTINATION_BINDING_LABEL = "iroha:sccp:evm-destination-binding:v1";
const SECRET_KEY_PATTERN =
  /(?:private[_-]?key|mnemonic|recovery[_-]?phrase|seed[_-]?phrase|secret|password|api[_-]?(?:key|token)|access[_-]?token|auth[_-]?token|bearer(?:[_-]?token)?|session[_-]?token|refresh[_-]?token)/iu;
const SECRET_ASSIGNMENT_PATTERN =
  /(?:private[_-]?key|mnemonic|recovery[_-]?phrase|seed[_-]?phrase|secret|password|api[_-]?(?:key|token)|access[_-]?token|auth[_-]?token|bearer(?:[_-]?token)?|session[_-]?token|refresh[_-]?token)\s*[:=]\s*("[^"]*"|'[^']*'|<[^>]+>|\S+)/giu;
const BEARER_TOKEN_TEXT_PATTERN = /\bbearer\s+[A-Za-z0-9._~+/=-]{12,}\b/iu;
const REDACTED_SECRET_ASSIGNMENT_VALUE_PATTERN =
  /^(?:redacted|<redacted>|\*{3,}|runtime[-_ ]?only|<runtime[-_ ]?only>|operator[-_ ]?provided|<operator[-_ ]?provided>|replace[_-]with[_-][A-Z0-9_ -]+)$/iu;
const PRIVATE_KEY_PEM_PATTERN =
  /-----BEGIN(?: [A-Z0-9]+)* PRIVATE KEY-----[\s\S]*?-----END(?: [A-Z0-9]+)* PRIVATE KEY-----/iu;
const RECOVERY_PHRASE_WORD_COUNTS = new Set([12, 15, 18, 21, 24]);
const PLACEHOLDER_BURN_RECORD_TEXT_PATTERN =
  /(?:diagnostic|dummy|fixture|mock|placeholder|stub|test-only)/iu;
const PRODUCTION_HANDOFF_PLACEHOLDER_PATTERN =
  /(?:change[-_ ]?me|changeme|dummy|example|mock|placeholder|replace[-_ ]?me|sample|stub|test[-_ ]?only|fixture[-_ ]?only|todo|your[-_ ]?[a-z0-9_-]*)/iu;
const DIAGNOSTIC_TEXT_KEYS = [
  "schema",
  "warning",
  "warnings",
  "note",
  "notes",
  "operatorWarning",
  "operator_warning",
  "verifierWarning",
  "verifier_warning",
  "verifierMaterialWarning",
  "verifier_material_warning",
  "diagnosticReason",
  "diagnostic_reason",
];
const DIAGNOSTIC_FLAG_KEYS = [
  "diagnosticVerifier",
  "diagnostic_verifier",
  "diagnosticVerifierMaterial",
  "diagnostic_verifier_material",
  "diagnostic",
];
const NATIVE_EVM_PROVER_BUNDLE_KEYS = Object.freeze([
  "nativeEvmProverBundle",
  "native_evm_prover_bundle",
  "bscNativeEvmProverBundle",
  "bsc_native_evm_prover_bundle",
  "nativeProverBundle",
  "native_prover_bundle",
  "proverBundle",
  "prover_bundle",
]);
const NATIVE_EVM_PROVER_BUNDLE_VERIFIER_KEY_ARTIFACT_HASH_KEYS = Object.freeze([
  "verifierKeyArtifactHash",
  "verifier_key_artifact_hash",
]);
const NATIVE_EVM_PROVER_ROLE_SEPARATED_HASH_FIELDS = Object.freeze([
  ["verifierKeyHash", ["verifierKeyHash", "verifier_key_hash"]],
  [
    "verifierKeyArtifactHash",
    ["verifierKeyArtifactHash", "verifier_key_artifact_hash"],
  ],
  ["proofArtifactHash", ["proofArtifactHash", "proof_artifact_hash"]],
  ["provingKeyHash", ["provingKeyHash", "proving_key_hash"]],
  [
    "nativeEvmProverBundleHash",
    ["nativeEvmProverBundleHash", "native_evm_prover_bundle_hash"],
  ],
  [
    "destinationBindingHash",
    ["destinationBindingHash", "destination_binding_hash"],
  ],
]);
const FORBIDDEN_BSC_ROUTE_MANIFEST_ADDRESS_ALIASES = Object.freeze({
  sourceBridgeAddress: Object.freeze([
    "sccpTronSourceBridgeAddress",
    "sccp_tron_source_bridge_address",
    "tronSourceBridgeAddress",
    "tron_source_bridge_address",
  ]),
  verifierAddress: Object.freeze([
    "sccpTronDestinationVerifierAddress",
    "sccp_tron_destination_verifier_address",
    "tronVerifierAddress",
    "tron_verifier_address",
  ]),
});
const POST_DEPLOY_LIVE_EVIDENCE_BLOCKER_KEYS = Object.freeze([
  "productionBlockers",
  "production_blockers",
  "postDeployProductionBlockers",
  "post_deploy_production_blockers",
  "fullTomlProductionBlockers",
  "full_toml_production_blockers",
  "sourceEventTransactionProductionBlockers",
  "source_event_transaction_production_blockers",
  "routeCanaryProductionBlockers",
  "route_canary_production_blockers",
]);
const NATIVE_EVM_PROVER_AUDIT_OPTION_KEYS = Object.freeze({
  circuit_security_audit: [
    "audit-circuit-security",
    "audit-circuit-security-audit",
  ],
  native_implementation_audit: [
    "audit-native-implementation",
    "audit-native-implementation-audit",
  ],
  reproducible_build_attestation: [
    "audit-reproducible-build",
    "audit-reproducible-build-attestation",
  ],
  cross_sdk_fixture_parity: [
    "audit-cross-sdk-parity",
    "audit-cross-sdk-fixture-parity",
  ],
  native_prover_self_test: ["audit-native-prover-self-test", "audit-self-test"],
  no_wasm_no_remote_scan: [
    "audit-no-wasm-no-remote-scan",
    "audit-no-wasm-scan",
  ],
});

const CONTRACT_SOURCES = Object.freeze({
  "contracts/evm/sccp/ISccpMessageVerifier.sol": repoPath(
    "contracts",
    "evm",
    "sccp",
    "ISccpMessageVerifier.sol",
  ),
  "contracts/evm/sccp/Ownable.sol": repoPath(
    "contracts",
    "evm",
    "sccp",
    "Ownable.sol",
  ),
  "contracts/evm/sccp/SccpGroth16Bn254MessageVerifier.sol": repoPath(
    "contracts",
    "evm",
    "sccp",
    "SccpGroth16Bn254MessageVerifier.sol",
  ),
  "contracts/bsc/sccp/SccpBscSourceBridge.sol": repoPath(
    "contracts",
    "bsc",
    "sccp",
    "SccpBscSourceBridge.sol",
  ),
  "contracts/bsc/sccp/TairaXOR.sol": repoPath(
    "contracts",
    "bsc",
    "sccp",
    "TairaXOR.sol",
  ),
  "contracts/bsc/sccp/TairaXorBscSccpBridge.sol": repoPath(
    "contracts",
    "bsc",
    "sccp",
    "TairaXorBscSccpBridge.sol",
  ),
});

const CONTRACT_DEFINITIONS = Object.freeze([
  {
    key: "verifier",
    file: "contracts/evm/sccp/SccpGroth16Bn254MessageVerifier.sol",
    contract: "SccpGroth16Bn254MessageVerifier",
  },
  {
    key: "sourceBridge",
    file: "contracts/bsc/sccp/SccpBscSourceBridge.sol",
    contract: "SccpBscSourceBridge",
  },
  {
    key: "token",
    file: "contracts/bsc/sccp/TairaXOR.sol",
    contract: "TairaXOR",
  },
  {
    key: "bridge",
    file: "contracts/bsc/sccp/TairaXorBscSccpBridge.sol",
    contract: "TairaXorBscSccpBridge",
  },
]);

const TOKEN_ABI = Object.freeze([
  "function bridge() view returns (address)",
  "function bridgeLocked() view returns (bool)",
  "function setBridge(address configuredBridge)",
  "function lockBridge()",
]);
const SOURCE_BRIDGE_ABI = Object.freeze([
  "function owner() view returns (address)",
  "function transferOwnership(address newOwner)",
]);
const VERIFIER_ABI = Object.freeze([
  "function verifyingKeyHash() view returns (bytes32)",
]);
const ROUTE_BRIDGE_ABI = Object.freeze([
  "function destinationBindingHash() view returns (bytes32)",
  "function verifier() view returns (address)",
  "function verifierCodeHash() view returns (bytes32)",
  "function verifierKeyHash() view returns (bytes32)",
  "function networkId() view returns (bytes32)",
  "function expectedSourceDomain() view returns (uint32)",
  "function expectedTargetDomain() view returns (uint32)",
]);

function repoPath(...segments) {
  return resolve(REPO_ROOT, ...segments);
}

function usage() {
  return `Usage:
	  node scripts/sccp_bsc_taira_xor_deploy.mjs compile [--out ${DEFAULT_ARTIFACTS_OUT}]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs deploy --bsc-network testnet|mainnet --verifier <verifier-key.json> --broadcast true --confirm-network ${ROUTE_ID}:testnet|${ROUTE_ID}:mainnet [--confirm-mainnet true] [--private-key-env ${DEFAULT_PRIVATE_KEY_ENV}] [--rpc-url ${DEFAULT_BSC_RPC_URL}] [--out ${DEFAULT_EVIDENCE_OUT}]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs evidence --bsc-network testnet|mainnet --token <addr> --bridge <addr> --source-bridge <addr> --verifier <addr> [--rpc-url ${DEFAULT_BSC_RPC_URL}] [--out ${DEFAULT_EVIDENCE_OUT}]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs route-manifest --evidence ${DEFAULT_EVIDENCE_OUT} --taira-contract ${DEFAULT_TAIRA_BURN_RECORD_CONTRACT_OUT} --settlement-asset-definition-id <asset-id> [--proof-artifact-hash <0x...> --proving-key-hash <0x...>] [--native-prover-bundle ${DEFAULT_NATIVE_EVM_PROVER_BUNDLE_OUT}] [--source-bridge-config-hash <0x...> --source-event-transaction-id <0x...> --source-event-explorer-url <url> --route-canary-evidence-hash <0x...> --route-canary-transaction-id <0x...> --route-canary-explorer-url <url> --full-toml-ready true --offline-full-toml-sha256 <0x...>|--offline-full-toml-evidence ${DEFAULT_ROUTE_FULL_CONFIG_EVIDENCE_OUT}] [--production-ready true --offline-full-toml-evidence ${DEFAULT_ROUTE_FULL_CONFIG_EVIDENCE_OUT} --live-readback-checked true --confirm-testnet ${ROUTE_ID}|--confirm-mainnet true --confirm-network ${ROUTE_ID}] [--out ${DEFAULT_ROUTE_MANIFEST_OUT}]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs native-prover-bundle --route-manifest ${DEFAULT_ROUTE_MANIFEST_OUT} --artifact-root ${DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT} --proof-artifact <relative-file> --proving-key <relative-file> --verifier-key <relative-file> --cross-sdk-parity <relative-json> --native-prover-self-test <relative-json> --javascript-implementation <relative-file> --swift-implementation <relative-file> --kotlin-implementation <relative-file> --java-android-implementation <relative-file> --dotnet-implementation <relative-file> --audit-circuit-security <hex-or-relative-file> --audit-native-implementation <hex-or-relative-file> --audit-reproducible-build <hex-or-relative-file> --audit-no-wasm-no-remote-scan <hex-or-relative-file> [--audit-cross-sdk-parity <matching-hex-or-relative-file>] [--audit-native-prover-self-test <matching-hex-or-relative-file>] [--out ${DEFAULT_NATIVE_EVM_PROVER_BUNDLE_OUT}] [--attach-route-manifest-out ${DEFAULT_ROUTE_MANIFEST_OUT}]
  node scripts/sccp_bsc_taira_xor_deploy.mjs route-config [--manifest ${DEFAULT_ROUTE_MANIFEST_OUT}] [--allow-unready true|false] [--base-config configs/soranexus/taira/config.toml] [--out ${DEFAULT_ROUTE_CONFIG_OUT}] [--write-offline-full-toml-evidence ${DEFAULT_ROUTE_FULL_CONFIG_EVIDENCE_OUT}]
  node scripts/sccp_bsc_taira_xor_deploy.mjs requirements [--bsc-network testnet|mainnet] [--out ${DEFAULT_PRODUCTION_REQUIREMENTS_OUT}]
  node scripts/sccp_bsc_taira_xor_deploy.mjs self-test

Required optional packages for compile/deploy/evidence: solc and ethers. The
contract smoke NODE_PATH can be reused after scripts/sccp_evm_contract_smoke.sh
has installed its temporary dependencies, or install equivalent local packages.

This helper writes only public deployment evidence and public prover-bundle
metadata. It reads deployer key material only from the named environment
variable at runtime and never writes it. Diagnostic verifier material is refused
by deploy unless --allow-diagnostic-verifier true is supplied explicitly.`;
}

const trim = (value) => String(value ?? "").trim();

function parseArgs(argv) {
  const args = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!token.startsWith("--")) {
      throw new Error(`Unexpected argument: ${token}`);
    }
    const key = token.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      args[key] = "true";
    } else {
      args[key] = next;
      index += 1;
    }
  }
  return args;
}

function parseBoolean(value, label = "boolean option") {
  if (value === undefined || value === null || value === "") return false;
  if (value === "true") return true;
  if (value === "false") return false;
  throw new Error(`${label} must be true or false.`);
}

export function normalizeBscNetworkProfile(value = "testnet") {
  const normalized = trim(value || "testnet")
    .toLowerCase()
    .replace(/_/gu, "-");
  if (
    !normalized ||
    ["testnet", "bsc-testnet", "chapel", "bsc-chapel"].includes(normalized)
  ) {
    return BSC_NETWORK_PROFILES.testnet;
  }
  if (["mainnet", "bsc-mainnet", "bnb-mainnet", "bsc"].includes(normalized)) {
    return BSC_NETWORK_PROFILES.mainnet;
  }
  throw new Error("--bsc-network must be testnet or mainnet.");
}

const bscNetworkProfileFromOptions = (options = {}) =>
  normalizeBscNetworkProfile(
    ownValue(options, "bsc-network") ??
      ownValue(options, "network") ??
      process.env.SCCP_BSC_NETWORK ??
      "testnet",
  );

const productionRequirementInput = ({
  id,
  kind,
  placeholder,
  requiredBy,
  description,
}) => ({
  id,
  kind,
  placeholder,
  requiredBy,
  description,
});

export function bscProductionRequirements(options = {}) {
  const profile = bscNetworkProfileFromOptions(options);
  const mainnetConfirmation =
    profile.key === "mainnet" ? " --confirm-mainnet true" : "";
  const routeManifestConfirmation =
    profile.key === "mainnet"
      ? `--confirm-mainnet true --confirm-network ${ROUTE_ID}`
      : `--confirm-testnet ${ROUTE_ID}`;
  const deploymentEvidenceOut = defaultDeploymentEvidenceOut(profile);
  const routeManifestOut = defaultRouteManifestOut(profile);
  const nativeBundleOut = defaultNativeEvmProverBundleOut(profile);
  const fullConfigEvidenceOut = defaultRouteFullConfigEvidenceOut(profile);
  return {
    schema: PRODUCTION_REQUIREMENTS_SCHEMA,
    routeId: ROUTE_ID,
    assetKey: ASSET_KEY,
    bsc: {
      network: profile.key,
      chain: profile.chain,
      chainIdHex: profile.chainIdHex,
      networkIdHex: profile.networkIdHex,
      explorerUrl: profile.explorerUrl,
      explorerHost: profile.explorerHost,
    },
    commands: {
      requirements:
        `node scripts/sccp_bsc_taira_xor_deploy.mjs requirements --bsc-network ${profile.key} ` +
        `--out ${defaultProductionRequirementsOut(profile)}`,
      deploy:
        `node scripts/sccp_bsc_taira_xor_deploy.mjs deploy --bsc-network ${profile.key} ` +
        `--verifier <production-verifier-key.json> --broadcast true ` +
        `--confirm-network ${profile.confirmNetwork}${mainnetConfirmation}`,
      evidence:
        `node scripts/sccp_bsc_taira_xor_deploy.mjs evidence --bsc-network ${profile.key} ` +
        "--token <addr> --bridge <addr> --source-bridge <addr> --verifier <addr>",
      routeManifest:
        `node scripts/sccp_bsc_taira_xor_deploy.mjs route-manifest --evidence ${deploymentEvidenceOut} ` +
        `--taira-contract ${DEFAULT_TAIRA_BURN_RECORD_CONTRACT_OUT} ` +
        "--settlement-asset-definition-id <canonical-asset-definition-id> " +
        "--proof-artifact-hash <0x...> --proving-key-hash <0x...> " +
        `--native-prover-bundle ${nativeBundleOut} ` +
        "--source-bridge-config-hash <0x...> " +
        "--source-event-transaction-id <0x...> " +
        "--source-event-explorer-url <url> " +
        "--route-canary-evidence-hash <0x...> " +
        "--route-canary-transaction-id <0x...> " +
        "--route-canary-explorer-url <url> " +
        "--full-toml-ready true " +
        `--offline-full-toml-evidence ${fullConfigEvidenceOut} ` +
        "--production-ready true --live-readback-checked true " +
        `${routeManifestConfirmation} --out ${routeManifestOut}`,
      nativeProverBundle:
        "node scripts/sccp_bsc_taira_xor_deploy.mjs native-prover-bundle " +
        `--route-manifest ${routeManifestOut} ` +
        `--artifact-root ${DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT} ` +
        "--proof-artifact <relative-circuit.r1cs> " +
        "--proving-key <relative-circuit.zkey> " +
        "--verifier-key <relative-verifier-key.json> " +
        "--cross-sdk-parity <relative-cross-sdk-parity.json> " +
        "--native-prover-self-test <relative-native-self-test.json> " +
        "--javascript-implementation <relative-js-implementation> " +
        "--swift-implementation <relative-swift-implementation> " +
        "--kotlin-implementation <relative-kotlin-implementation> " +
        "--java-android-implementation <relative-java-android-implementation> " +
        "--dotnet-implementation <relative-dotnet-implementation> " +
        "--audit-circuit-security <hex-or-relative-file> " +
        "--audit-native-implementation <hex-or-relative-file> " +
        "--audit-reproducible-build <hex-or-relative-file> " +
        "--audit-no-wasm-no-remote-scan <hex-or-relative-file> " +
        `--out ${nativeBundleOut} ` +
        `--attach-route-manifest-out ${routeManifestOut}`,
      routeConfig:
        `node scripts/sccp_bsc_taira_xor_deploy.mjs route-config --manifest ${routeManifestOut} ` +
        `--base-config <deployed-taira-config.toml> --write-offline-full-toml-evidence ${fullConfigEvidenceOut}`,
    },
    inputs: [
      productionRequirementInput({
        id: "production-groth16-verifier-key-json",
        kind: "file",
        placeholder: "<production-verifier-key.json>",
        requiredBy: ["deploy", "native-prover-bundle"],
        description:
          "BN254 Groth16 verifier key JSON whose hash is not in the diagnostic denylist.",
      }),
      productionRequirementInput({
        id: `${profile.key}-funded-bsc-deployer`,
        kind: "operator-environment",
        placeholder: `<${profile.key}-deployer-signing-env>`,
        requiredBy: ["deploy"],
        description:
          "Funded BSC deployer configured outside generated reports for the selected network.",
      }),
      productionRequirementInput({
        id: `${profile.key}-bsc-rpc-endpoint`,
        kind: "url",
        placeholder: `<${profile.key}-bsc-rpc-url>`,
        requiredBy: ["deploy", "evidence"],
        description:
          "Selected BSC RPC endpoint used for deployment and contract readback.",
      }),
      productionRequirementInput({
        id: `${profile.key}-bsc-deployment-evidence`,
        kind: "file",
        placeholder: deploymentEvidenceOut,
        requiredBy: ["route-manifest"],
        description:
          "BSC deployment evidence generated from live contract deployment and readback for the selected network.",
      }),
      productionRequirementInput({
        id: "production-route-manifest",
        kind: "file",
        placeholder: routeManifestOut,
        requiredBy: ["native-prover-bundle", "route-config"],
        description:
          "Production route manifest bound to BSC deployment evidence, TAIRA route publication, and live canary evidence.",
      }),
      productionRequirementInput({
        id: "taira-burn-record-contract",
        kind: "file",
        placeholder: DEFAULT_TAIRA_BURN_RECORD_CONTRACT_OUT,
        requiredBy: ["route-manifest"],
        description:
          "Compiled TAIRA burn-record IVM contract artifact used by the BSC route manifest.",
      }),
      productionRequirementInput({
        id: "canonical-settlement-asset-definition-id",
        kind: "asset-definition-id",
        placeholder: "<canonical-asset-definition-id>",
        requiredBy: ["route-manifest"],
        description:
          "Canonical Base58 XOR settlement asset definition id used by the BSC route manifest.",
      }),
      productionRequirementInput({
        id: "post-deploy-live-evidence",
        kind: "hashes-and-urls",
        placeholder:
          "--source-bridge-config-hash/--source-event-transaction-id/--route-canary-evidence-hash/--route-canary-transaction-id",
        requiredBy: ["route-manifest"],
        description:
          "Live post-deploy source-event and route-canary evidence for production-ready route manifests.",
      }),
      productionRequirementInput({
        id: "deployed-taira-base-config",
        kind: "file",
        placeholder: "<deployed-taira-config.toml>",
        requiredBy: ["route-config"],
        description:
          "Deployed TAIRA peer config used to render the merged route config and generated offline full-TOML evidence.",
      }),
      productionRequirementInput({
        id: "offline-full-toml-evidence",
        kind: "file",
        placeholder: fullConfigEvidenceOut,
        requiredBy: ["route-manifest"],
        description:
          "Generated offline full-TOML evidence artifact consumed by the final production route manifest.",
      }),
      productionRequirementInput({
        id: "native-prover-artifact-root",
        kind: "directory",
        placeholder: DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT,
        requiredBy: ["native-prover-bundle"],
        description:
          "Canonical artifact root containing native EVM prover inputs and implementation evidence.",
      }),
      productionRequirementInput({
        id: "burn-record-proof-artifact",
        kind: "file",
        placeholder: "<relative-circuit.r1cs>",
        requiredBy: ["native-prover-bundle"],
        description:
          "Production burn-record proof artifact referenced relative to the artifact root.",
      }),
      productionRequirementInput({
        id: "burn-record-proving-key",
        kind: "file",
        placeholder: "<relative-circuit.zkey>",
        requiredBy: ["native-prover-bundle"],
        description:
          "Production burn-record proving key referenced relative to the artifact root.",
      }),
      productionRequirementInput({
        id: "cross-sdk-parity-report",
        kind: "file",
        placeholder: "<relative-cross-sdk-parity.json>",
        requiredBy: ["native-prover-bundle"],
        description:
          "Cross-SDK production parity report covering JavaScript, Swift, Kotlin, Java Android, and .NET bindings.",
      }),
      productionRequirementInput({
        id: "native-prover-self-test-report",
        kind: "file",
        placeholder: "<relative-native-self-test.json>",
        requiredBy: ["native-prover-bundle"],
        description:
          "Native EVM prover self-test report bound to the selected BSC network.",
      }),
      ...[
        ["javascript-sdk-implementation", "<relative-js-implementation>"],
        ["swift-sdk-implementation", "<relative-swift-implementation>"],
        ["kotlin-sdk-implementation", "<relative-kotlin-implementation>"],
        [
          "java-android-sdk-implementation",
          "<relative-java-android-implementation>",
        ],
        ["dotnet-sdk-implementation", "<relative-dotnet-implementation>"],
      ].map(([id, placeholder]) =>
        productionRequirementInput({
          id,
          kind: "file-or-directory",
          placeholder,
          requiredBy: ["native-prover-bundle"],
          description: `${id.replace(/-/gu, " ")} evidence.`,
        }),
      ),
      ...[
        "audit-circuit-security",
        "audit-native-implementation",
        "audit-reproducible-build",
        "audit-no-wasm-no-remote-scan",
      ].map((id) =>
        productionRequirementInput({
          id,
          kind: "hash-or-file",
          placeholder: "<hex-or-relative-file>",
          requiredBy: ["native-prover-bundle"],
          description: `${id.replace(/-/gu, " ")} evidence.`,
        }),
      ),
      productionRequirementInput({
        id: "taira-peer-config-targets",
        kind: "operator-environment",
        placeholder: "<taira-peer-config-targets>",
        requiredBy: ["route-config"],
        description:
          "TAIRA peer configuration targets that will receive the generated route stanza.",
      }),
    ],
    requiredReports: [
      "route-preflight",
      "peer-config-audit",
      "smoke-readiness",
      "production-material-inventory",
      "live-ui-video-proof",
    ],
    deniedVerifierKeyHashes: [...SCCP_BSC_DIAGNOSTIC_VERIFIER_KEY_HASHES],
  };
}

function requireBscNetworkConfirmation(options, profile, action) {
  const modern = trim(ownValue(options, "confirm-network"));
  const legacyTestnet = trim(ownValue(options, "confirm-testnet"));
  if (modern !== profile.confirmNetwork) {
    if (
      profile.key === "testnet" &&
      !modern &&
      legacyTestnet === CONFIRMATION_TEXT
    ) {
      return;
    }
    if (profile.key === "testnet" && !modern) {
      throw new Error(
        `${action} requires --confirm-testnet ${CONFIRMATION_TEXT} or --confirm-network ${profile.confirmNetwork}.`,
      );
    }
    throw new Error(
      `${action} requires --confirm-network ${profile.confirmNetwork}.`,
    );
  }
  if (
    profile.key === "mainnet" &&
    !parseBoolean(ownValue(options, "confirm-mainnet"), "--confirm-mainnet")
  ) {
    throw new Error(`${action} requires --confirm-mainnet true.`);
  }
}

const defaultBscRpcUrl = (profile) => profile.defaultRpcUrl;

const defaultDeploymentEvidenceOut = (profile) =>
  profile.deploymentEvidenceOut ?? DEFAULT_EVIDENCE_OUT;

const defaultRouteManifestOut = (profile) =>
  profile.routeManifestOut ?? DEFAULT_ROUTE_MANIFEST_OUT;

const defaultRouteConfigOut = (profile, { fullConfigMode = false } = {}) =>
  fullConfigMode
    ? (profile.routeFullConfigOut ?? DEFAULT_ROUTE_FULL_CONFIG_OUT)
    : (profile.routeConfigOut ?? DEFAULT_ROUTE_CONFIG_OUT);

const defaultRouteFullConfigEvidenceOut = (profile) =>
  profile.routeFullConfigEvidenceOut ?? DEFAULT_ROUTE_FULL_CONFIG_EVIDENCE_OUT;

const defaultNativeEvmProverBundleOut = (profile) =>
  profile.nativeBundleOut ?? DEFAULT_NATIVE_EVM_PROVER_BUNDLE_OUT;

const defaultProductionRequirementsOut = (profile) =>
  profile.key === "mainnet"
    ? "artifacts/sccp-bsc/taira-bsc-mainnet-xor-production-requirements.json"
    : DEFAULT_PRODUCTION_REQUIREMENTS_OUT;

const bscNativeProverBundleId = (profile) =>
  profile.key === "mainnet"
    ? SCCP_BSC_MAINNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1
    : SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1;

const validateBscNativeEvmProverBundleForProfile = (
  bundle,
  profile,
  options = {},
) =>
  profile.key === "mainnet"
    ? validateBscMainnetNativeEvmProverBundle(bundle, options)
    : validateBscTestnetNativeEvmProverBundle(bundle, options);

const parseBscNativeProverParityFixtureForProfile = (
  fixture,
  descriptor,
  profile,
) =>
  profile.key === "mainnet"
    ? parseBscMainnetNativeEvmProverParityFixture(fixture, descriptor)
    : parseBscTestnetNativeEvmProverParityFixture(fixture, descriptor);

const parseBscNativeProverSelfTestFixtureForProfile = (
  fixture,
  descriptor,
  profile,
) =>
  profile.key === "mainnet"
    ? parseBscMainnetNativeEvmProverSelfTestFixture(fixture, descriptor)
    : parseBscTestnetNativeEvmProverSelfTestFixture(fixture, descriptor);

const verifyBscNativeEvmProverArtifactsFromBundleForProfile = (
  input,
  options,
  profile,
) =>
  profile.key === "mainnet"
    ? verifyBscMainnetNativeEvmProverArtifactsFromBundle(input, options)
    : verifyBscTestnetNativeEvmProverArtifactsFromBundle(input, options);

function bytesToHex(bytes, prefix = true) {
  const hex = Array.from(bytes, (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
  return prefix ? `0x${hex}` : hex;
}

function hexToBytes(
  value,
  label,
  byteLength = null,
  { allowZero = false } = {},
) {
  const normalized = trim(value).toLowerCase().replace(/^0x/u, "");
  if (!/^(?:[0-9a-f]{2})*$/u.test(normalized)) {
    throw new Error(`${label} must be hex bytes.`);
  }
  if (byteLength !== null && normalized.length !== byteLength * 2) {
    throw new Error(`${label} must be ${byteLength} bytes.`);
  }
  const bytes = Uint8Array.from(
    normalized.match(/.{2}/gu)?.map((chunk) => Number.parseInt(chunk, 16)) ??
      [],
  );
  if (!allowZero && bytes.every((byte) => byte === 0)) {
    throw new Error(`${label} must be non-zero.`);
  }
  return bytes;
}

export function normalizeHex32(value, label = "value") {
  return bytesToHex(hexToBytes(value, label, 32));
}

function normalizeCanonicalHex32(value, label = "value") {
  const text = canonicalRecordString(value, label);
  if (!text) {
    throw new Error(`${label} is required.`);
  }
  if (/^0X/u.test(text) || /[A-F]/u.test(text.replace(/^0x/u, ""))) {
    throw new Error(`${label} must be canonical lowercase hex.`);
  }
  return normalizeHex32(text, label);
}

function normalizeCanonicalEvmAddress(value, label = "address") {
  const text = canonicalRecordString(value, label);
  if (!text) {
    throw new Error(`${label} is required.`);
  }
  if (/^0X/u.test(text) || /[A-F]/u.test(text.replace(/^0x/u, ""))) {
    throw new Error(`${label} must be canonical lowercase hex.`);
  }
  return normalizeEvmAddress(text, label);
}

export function isKnownDiagnosticBscVerifierKeyHash(value) {
  try {
    return SCCP_BSC_DIAGNOSTIC_VERIFIER_KEY_HASHES.has(
      normalizeHex32(value, "BSC verifier key hash"),
    );
  } catch (_error) {
    return false;
  }
}

export function normalizeEvmAddress(value, label = "address") {
  const bytes = hexToBytes(value, label, 20);
  return bytesToHex(bytes);
}

function normalizePrivateKey(value, label = "private key") {
  return bytesToHex(hexToBytes(value, label, 32));
}

function normalizeUint256(value, label) {
  const text = trim(value);
  if (!/^(?:0x[0-9a-f]+|[0-9]+)$/iu.test(text)) {
    throw new Error(`${label} must be a uint256.`);
  }
  const parsed = BigInt(text);
  if (parsed < 0n || parsed >= 2n ** 256n) {
    throw new Error(`${label} must fit uint256.`);
  }
  return parsed.toString();
}

function normalizeUint32(value, label) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 0 || parsed > 0xffffffff) {
    throw new Error(`${label} must fit uint32.`);
  }
  return parsed;
}

export function normalizeBscRpcUrl(
  value = DEFAULT_BSC_RPC_URL,
  { allowLocal = false } = {},
) {
  const endpoint = trim(value) || DEFAULT_BSC_RPC_URL;
  let url;
  try {
    url = new URL(endpoint);
  } catch (_error) {
    throw new Error("BSC RPC URL must be a valid URL.");
  }
  const isLocalhost = ["localhost", "127.0.0.1", "::1"].includes(url.hostname);
  if (url.protocol !== "https:" && !(allowLocal && url.protocol === "http:")) {
    throw new Error("BSC RPC URL must use HTTPS unless localhost is allowed.");
  }
  if (url.username || url.password || url.search || url.hash) {
    throw new Error(
      "BSC RPC URL must not contain credentials, query strings, or fragments.",
    );
  }
  if (url.protocol === "http:" && !isLocalhost) {
    throw new Error("HTTP BSC RPC URLs are only allowed for localhost.");
  }
  url.pathname = url.pathname.replace(/\/+$/u, "") || "/";
  return url.toString().replace(/\/$/u, "");
}

function normalizeBscExplorerTxUrl(
  value,
  label,
  expectedTxHash,
  profile = BSC_NETWORK_PROFILES.testnet,
) {
  const text = normalizeNonEmptyText(value, label);
  let url;
  try {
    url = new URL(text);
  } catch (_error) {
    throw new Error(`${label} must be a valid URL.`);
  }
  if (
    url.protocol !== "https:" ||
    url.hostname !== profile.explorerHost ||
    url.username ||
    url.password ||
    url.search ||
    url.hash
  ) {
    throw new Error(
      `${label} must be an HTTPS ${profile.label} explorer transaction URL without credentials, query strings, or fragments.`,
    );
  }
  const match = url.pathname
    .replace(/\/+$/u, "")
    .match(/^\/tx\/0x([0-9a-f]{64})$/iu);
  if (!match) {
    throw new Error(`${label} must use the /tx/0x<hash> path.`);
  }
  const expected = normalizeHex32(expectedTxHash, `${label} transaction id`);
  const actual = `0x${match[1].toLowerCase()}`;
  if (actual !== expected) {
    throw new Error(`${label} transaction hash must match ${expected}.`);
  }
  return `${profile.explorerUrl}/tx/${expected}`;
}

function normalizeBscExplorerBaseUrl(
  value,
  label,
  profile = BSC_NETWORK_PROFILES.testnet,
) {
  const text = normalizeNonEmptyText(value, label).replace(/\/+$/u, "");
  let url;
  try {
    url = new URL(text);
  } catch (_error) {
    throw new Error(`${label} must be a valid URL.`);
  }
  if (
    url.protocol !== "https:" ||
    url.hostname !== profile.explorerHost ||
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    (url.pathname && url.pathname !== "/")
  ) {
    throw new Error(
      `${label} must be the HTTPS ${profile.label} explorer origin without credentials, path, query, or fragment.`,
    );
  }
  return profile.explorerUrl;
}

function normalizeBscExplorerHost(
  value,
  label,
  profile = BSC_NETWORK_PROFILES.testnet,
) {
  const text = normalizeNonEmptyText(value, label).toLowerCase();
  if (text.includes("://") || /[/?#@]/u.test(text)) {
    throw new Error(`${label} must be a hostname, not a URL.`);
  }
  let url;
  try {
    url = new URL(`https://${text}`);
  } catch (_error) {
    throw new Error(`${label} must be a valid hostname.`);
  }
  if (url.host !== profile.explorerHost) {
    throw new Error(`${label} must be ${profile.explorerHost}.`);
  }
  return profile.explorerHost;
}

function abiWordBytes(bytes, label, byteLength) {
  const valueBytes = hexToBytes(bytes, label, byteLength);
  if (byteLength === 32) {
    return valueBytes;
  }
  const out = new Uint8Array(32);
  out.set(valueBytes, 32 - byteLength);
  return out;
}

function abiWordUint(value) {
  const out = new Uint8Array(32);
  let current = BigInt(value);
  for (let index = 31; index >= 0; index -= 1) {
    out[index] = Number(current & 0xffn);
    current >>= 8n;
  }
  return out;
}

function concatBytes(parts) {
  const out = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}

function keccakTextHex(value) {
  return bytesToHex(keccak_256(textEncoder.encode(value)));
}

export function bscDestinationBindingHash(input = {}) {
  const networkId =
    ownValue(input, "networkId") ??
    ownValue(input, "network_id") ??
    BSC_TESTNET_NETWORK_ID_HEX;
  const verifierAddress =
    ownValue(input, "verifierAddress") ?? ownValue(input, "verifier_address");
  const bridgeAddress =
    ownValue(input, "bridgeAddress") ?? ownValue(input, "bridge_address");
  const verifierCodeHash =
    ownValue(input, "verifierCodeHash") ??
    ownValue(input, "verifier_code_hash");
  const verifierKeyHash =
    ownValue(input, "verifierKeyHash") ?? ownValue(input, "verifier_key_hash");
  const encoded = concatBytes([
    abiWordBytes(
      keccakTextHex(DESTINATION_BINDING_LABEL),
      "destination binding label",
      32,
    ),
    abiWordBytes(
      keccakTextHex(BSC_EVM_GROTH16_BACKEND),
      "verifier backend hash",
      32,
    ),
    abiWordBytes(
      keccakTextHex(SCCP_PROOF_FAMILY_STARK_FRI),
      "proof family hash",
      32,
    ),
    abiWordBytes(networkId, "BSC network id", 32),
    abiWordUint(SCCP_DOMAIN_SORA),
    abiWordUint(SCCP_DOMAIN_BSC),
    abiWordBytes(verifierAddress, "BSC verifier address", 20),
    abiWordBytes(bridgeAddress, "BSC bridge address", 20),
    abiWordBytes(verifierCodeHash, "BSC verifier code hash", 32),
    abiWordBytes(verifierKeyHash, "BSC verifier key hash", 32),
  ]);
  return bytesToHex(keccak_256(encoded));
}

export function bscDestinationBindingKey(input = {}) {
  const networkId =
    ownValue(input, "networkId") ??
    ownValue(input, "network_id") ??
    BSC_TESTNET_NETWORK_ID_HEX;
  const verifierAddress =
    ownValue(input, "verifierAddress") ?? ownValue(input, "verifier_address");
  const bridgeAddress =
    ownValue(input, "bridgeAddress") ?? ownValue(input, "bridge_address");
  const verifierCodeHash =
    ownValue(input, "verifierCodeHash") ??
    ownValue(input, "verifier_code_hash");
  const verifierKeyHash =
    ownValue(input, "verifierKeyHash") ?? ownValue(input, "verifier_key_hash");
  return `evm:${SCCP_DOMAIN_SORA}:${SCCP_DOMAIN_BSC}:${normalizeHex32(
    networkId,
    "BSC network id",
  ).slice(2)}:${normalizeEvmAddress(
    verifierAddress,
    "BSC verifier address",
  )}:${normalizeEvmAddress(bridgeAddress, "BSC bridge address")}:${normalizeHex32(
    verifierCodeHash,
    "BSC verifier code hash",
  )}:${normalizeHex32(verifierKeyHash, "BSC verifier key hash")}`;
}

function pickField(record, names, label) {
  if (!isRecord(record)) {
    throw new Error(`verifier material is missing ${label}`);
  }
  for (const name of names) {
    if (hasOwn(record, name) && ownValue(record, name) !== undefined) {
      return ownValue(record, name);
    }
  }
  throw new Error(`verifier material is missing ${label}`);
}

function flattenArray(value, label) {
  if (!Array.isArray(value)) {
    throw new Error(`${label} must be an array.`);
  }
  const out = [];
  const visit = (entries) => {
    for (const [, entry] of ownArrayValues(entries)) {
      if (Array.isArray(entry)) {
        visit(entry);
      } else {
        out.push(entry);
      }
    }
  };
  visit(value);
  return out;
}

function normalizeUint256Array(value, label, expectedLength) {
  const values = flattenArray(value, label).map((entry, index) =>
    normalizeUint256(entry, `${label}[${index}]`),
  );
  if (values.length !== expectedLength) {
    throw new Error(`${label} must contain ${expectedLength} uint256 values.`);
  }
  return values;
}

function normalizeBn254FieldElement(value, label) {
  const parsed = BigInt(value);
  if (parsed < 0n || parsed >= BN254_BASE_FIELD_MODULUS) {
    throw new Error(`${label} must be a BN254 field element.`);
  }
  return parsed;
}

function bn254Mod(value) {
  const remainder = value % BN254_BASE_FIELD_MODULUS;
  return remainder >= 0n ? remainder : remainder + BN254_BASE_FIELD_MODULUS;
}

function bn254Fp2Add(left, right) {
  return [bn254Mod(left[0] + right[0]), bn254Mod(left[1] + right[1])];
}

function bn254Fp2Mul(left, right) {
  return [
    bn254Mod(left[0] * right[0] - left[1] * right[1]),
    bn254Mod(left[0] * right[1] + left[1] * right[0]),
  ];
}

function bn254Fp2Square(value) {
  return bn254Fp2Mul(value, value);
}

function bn254Fp2Cube(value) {
  return bn254Fp2Mul(bn254Fp2Square(value), value);
}

const sameBn254Fp2 = (left, right) =>
  left[0] === right[0] && left[1] === right[1];

function assertBn254G1Point(point, label) {
  if (point.length !== 2) {
    throw new Error(`${label} must contain two BN254 G1 coordinates.`);
  }
  const x = normalizeBn254FieldElement(point[0], `${label}.x`);
  const y = normalizeBn254FieldElement(point[1], `${label}.y`);
  if (x === 0n && y === 0n) {
    throw new Error(`${label} must not be the BN254 point at infinity.`);
  }
  if (bn254Mod(y * y) !== bn254Mod(x * x * x + 3n)) {
    throw new Error(`${label} must be on the BN254 G1 curve.`);
  }
}

function assertBn254G2Point(point, label) {
  if (point.length !== 4) {
    throw new Error(`${label} must contain four BN254 G2 coordinates.`);
  }
  const x = [
    normalizeBn254FieldElement(point[0], `${label}.x.c0`),
    normalizeBn254FieldElement(point[1], `${label}.x.c1`),
  ];
  const y = [
    normalizeBn254FieldElement(point[2], `${label}.y.c0`),
    normalizeBn254FieldElement(point[3], `${label}.y.c1`),
  ];
  if (x[0] === 0n && x[1] === 0n && y[0] === 0n && y[1] === 0n) {
    throw new Error(`${label} must not be the BN254 G2 point at infinity.`);
  }
  const expected = bn254Fp2Add(bn254Fp2Cube(x), BN254_TWIST_B_COEFFICIENT);
  if (!sameBn254Fp2(bn254Fp2Square(y), expected)) {
    throw new Error(`${label} must be on the BN254 G2 twist curve.`);
  }
}

function assertBn254G1VectorPairs(values, label) {
  if (values.length % 2 !== 0) {
    throw new Error(
      `${label} must contain complete BN254 G1 coordinate pairs.`,
    );
  }
  for (let offset = 0; offset < values.length; offset += 2) {
    assertBn254G1Point(
      values.slice(offset, offset + 2),
      `${label}[${offset / 2}]`,
    );
  }
}

const sameVector = (actual, expected) =>
  actual.length === expected.length &&
  actual.every((entry, index) => entry === expected[index]);

const isNormalizedSmokeFixtureGroth16VerifierMaterial = (material) =>
  sameVector(material.alpha1, SMOKE_FIXTURE_G1) &&
  sameVector(material.beta2, SMOKE_FIXTURE_G2) &&
  sameVector(material.gamma2, SMOKE_FIXTURE_G2) &&
  sameVector(material.delta2, SMOKE_FIXTURE_G2) &&
  sameVector(material.ic, SMOKE_FIXTURE_IC);

function normalizeVerifierCoordinates(material) {
  return {
    alpha1: normalizeUint256Array(
      pickField(
        material,
        ["alpha1", "configuredAlpha1", "vk_alpha_1"],
        "alpha1",
      ),
      "alpha1",
      2,
    ),
    beta2: normalizeUint256Array(
      pickField(material, ["beta2", "configuredBeta2", "vk_beta_2"], "beta2"),
      "beta2",
      4,
    ),
    gamma2: normalizeUint256Array(
      pickField(
        material,
        ["gamma2", "configuredGamma2", "vk_gamma_2"],
        "gamma2",
      ),
      "gamma2",
      4,
    ),
    delta2: normalizeUint256Array(
      pickField(
        material,
        ["delta2", "configuredDelta2", "vk_delta_2"],
        "delta2",
      ),
      "delta2",
      4,
    ),
    ic: normalizeUint256Array(
      pickField(material, ["ic", "configuredIc", "vk_ic", "IC"], "ic"),
      "ic",
      20,
    ),
  };
}

function bscGroth16VerifierKeyHashFromCoordinates(material) {
  return bytesToHex(
    keccak_256(
      concatBytes(
        [
          ...material.alpha1,
          ...material.beta2,
          ...material.gamma2,
          ...material.delta2,
          ...material.ic,
        ].map((value) => abiWordUint(value)),
      ),
    ),
  );
}

export function bscGroth16VerifierKeyHash(material) {
  return bscGroth16VerifierKeyHashFromCoordinates(
    normalizeVerifierCoordinates(material),
  );
}

export function isSmokeFixtureGroth16VerifierMaterial(material) {
  try {
    return isNormalizedSmokeFixtureGroth16VerifierMaterial(
      normalizeVerifierCoordinates(material),
    );
  } catch (_error) {
    return false;
  }
}

export function normalizeVerifierMaterial(
  material,
  profile = BSC_NETWORK_PROFILES.testnet,
) {
  if (!material || typeof material !== "object" || Array.isArray(material)) {
    throw new Error("verifier material must be a JSON object.");
  }
  const proofFamily = String(
    readFirstValue(material, "proofFamily") ?? SCCP_PROOF_FAMILY_STARK_FRI,
  );
  if (proofFamily !== SCCP_PROOF_FAMILY_STARK_FRI) {
    throw new Error("proofFamily must be stark-fri-v1 for BSC SCCP.");
  }
  const networkId = normalizeHex32(
    readFirstValue(material, "networkId") ?? profile.networkIdHex,
    "networkId",
  );
  if (networkId !== profile.networkIdHex) {
    throw new Error(`networkId must be ${profile.label} for ${ROUTE_ID}.`);
  }
  const sourceDomain = normalizeUint32(
    readFirstValue(material, "sourceDomain") ?? SCCP_DOMAIN_SORA,
    "sourceDomain",
  );
  const targetDomain = normalizeUint32(
    readFirstValue(material, "targetDomain") ?? SCCP_DOMAIN_BSC,
    "targetDomain",
  );
  if (sourceDomain !== SCCP_DOMAIN_SORA || targetDomain !== SCCP_DOMAIN_BSC) {
    throw new Error("destination verifier domains must be SORA -> BSC.");
  }
  const expectedVerifierKeyHash = normalizeHex32(
    readFirstValue(
      material,
      "expectedVerifierKeyHash",
      "verifierKeyHash",
      "verifyingKeyHash",
    ),
    "expectedVerifierKeyHash",
  );
  const normalizedMaterial = normalizeVerifierCoordinates(material);
  const fixtureShaped =
    isNormalizedSmokeFixtureGroth16VerifierMaterial(normalizedMaterial);
  assertBn254G1Point(normalizedMaterial.alpha1, "alpha1");
  assertBn254G2Point(normalizedMaterial.beta2, "beta2");
  assertBn254G2Point(normalizedMaterial.gamma2, "gamma2");
  assertBn254G2Point(normalizedMaterial.delta2, "delta2");
  assertBn254G1VectorPairs(normalizedMaterial.ic, "ic");
  const computedVerifierKeyHash =
    bscGroth16VerifierKeyHashFromCoordinates(normalizedMaterial);
  if (expectedVerifierKeyHash !== computedVerifierKeyHash) {
    throw new Error(
      `expectedVerifierKeyHash must match Solidity verifyingKeyHash() ${computedVerifierKeyHash}.`,
    );
  }
  const diagnosticVerifierReasons = [
    diagnosticFlagReason(material, "verifier material"),
    fixtureShaped
      ? "verifier material matches the deterministic smoke-test Groth16 fixture key"
      : "",
    isKnownDiagnosticBscVerifierKeyHash(expectedVerifierKeyHash)
      ? `verifierKeyHash=${expectedVerifierKeyHash} is a known diagnostic BSC verifier key hash`
      : "",
  ].filter(Boolean);
  return {
    ...normalizedMaterial,
    expectedVerifierKeyHash,
    diagnosticVerifierReasons,
    fixtureShaped,
    proofFamily,
    networkId,
    sourceDomain,
    targetDomain,
  };
}

function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasOwn(record, key) {
  return isRecord(record) && Object.prototype.hasOwnProperty.call(record, key);
}

function ownValue(record, key) {
  if (!hasOwn(record, key)) {
    return undefined;
  }
  const descriptor = Object.getOwnPropertyDescriptor(record, key);
  return descriptor && Object.prototype.hasOwnProperty.call(descriptor, "value")
    ? descriptor.value
    : undefined;
}

function ownRecordEntries(record) {
  if (!isRecord(record)) {
    return [];
  }
  return Object.keys(record).map((key) => [key, ownValue(record, key)]);
}

function ownArrayValues(value) {
  if (!Array.isArray(value)) {
    return [];
  }
  const values = [];
  for (let index = 0; index < value.length; index += 1) {
    const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
    if (
      descriptor &&
      Object.prototype.hasOwnProperty.call(descriptor, "value")
    ) {
      values.push([index, descriptor.value]);
    }
  }
  return values;
}

function hasAnyOwnManifestKey(record, keys) {
  return isRecord(record) && keys.some((key) => hasOwn(record, key));
}

function canonicalRecordString(value, label) {
  if (typeof value !== "string" || value.length === 0) {
    return "";
  }
  if (value.trim() !== value) {
    throw new Error(`${label} must be a non-empty canonical string.`);
  }
  return value;
}

function readFirstString(record, ...keys) {
  if (!isRecord(record)) {
    return "";
  }
  for (const key of keys) {
    if (!hasOwn(record, key)) {
      continue;
    }
    const value = canonicalRecordString(ownValue(record, key), key);
    if (value) {
      return value;
    }
  }
  return "";
}

function readFirstRecord(record, ...keys) {
  if (!isRecord(record)) {
    return null;
  }
  for (const key of keys) {
    if (!hasOwn(record, key)) {
      continue;
    }
    const value = ownValue(record, key);
    if (isRecord(value)) {
      return value;
    }
  }
  return null;
}

function readFirstValue(record, ...keys) {
  if (!isRecord(record)) {
    return undefined;
  }
  for (const key of keys) {
    if (hasOwn(record, key)) {
      return ownValue(record, key);
    }
  }
  return undefined;
}

function diagnosticTextValue(value) {
  if (typeof value === "string") {
    return /\bdiagnostic\b/iu.test(value);
  }
  if (Array.isArray(value)) {
    return ownArrayValues(value).some(([, entry]) =>
      diagnosticTextValue(entry),
    );
  }
  return false;
}

function diagnosticFlagReason(record, pathName) {
  if (!isRecord(record)) {
    return "";
  }
  for (const key of DIAGNOSTIC_FLAG_KEYS) {
    if (hasOwn(record, key) && ownValue(record, key) === true) {
      return `${pathName}.${key}=true`;
    }
  }
  for (const key of DIAGNOSTIC_TEXT_KEYS) {
    if (hasOwn(record, key) && diagnosticTextValue(ownValue(record, key))) {
      return `${pathName}.${key} mentions diagnostic verifier material`;
    }
  }
  return "";
}

function isOpaqueProductionMaterialString(value) {
  const text = value.trim();
  return (
    /^0x[0-9a-f]{40,}$/iu.test(text) ||
    /^sha256:[0-9a-f]{64}$/iu.test(text) ||
    /^https?:\/\/[^\s]+$/iu.test(text) ||
    (text.length >= 96 && /^[A-Za-z0-9+/=_-]+$/u.test(text))
  );
}

function productionHandoffPlaceholderReason(
  value,
  pathName,
  seen = new WeakSet(),
) {
  if (typeof value === "string") {
    return !isOpaqueProductionMaterialString(value) &&
      PRODUCTION_HANDOFF_PLACEHOLDER_PATTERN.test(value)
      ? pathName
      : "";
  }
  if (Array.isArray(value)) {
    for (const [index, entry] of ownArrayValues(value)) {
      const reason = productionHandoffPlaceholderReason(
        entry,
        `${pathName}[${index}]`,
        seen,
      );
      if (reason) {
        return reason;
      }
    }
    return "";
  }
  if (!isRecord(value)) {
    return "";
  }
  if (seen.has(value)) {
    return "";
  }
  seen.add(value);
  for (const [key, entry] of ownRecordEntries(value)) {
    const childPath = `${pathName}.${key}`;
    if (PRODUCTION_HANDOFF_PLACEHOLDER_PATTERN.test(key)) {
      return childPath;
    }
    const reason = productionHandoffPlaceholderReason(entry, childPath, seen);
    if (reason) {
      return reason;
    }
  }
  return "";
}

function uniqueNonEmpty(values) {
  return [...new Set(values.filter(Boolean))];
}

function pathIsWithin(candidatePath, rootPath) {
  const relativePath = relative(rootPath, candidatePath);
  return (
    relativePath === "" ||
    (relativePath &&
      !relativePath.startsWith("..") &&
      !isAbsolute(relativePath))
  );
}

export function isCanonicalBscProductionArtifactPath(pathName) {
  const resolvedPath = resolve(pathName);
  return [
    resolve(CANONICAL_BSC_PRODUCTION_ARTIFACT_ROOT),
    repoPath("artifacts", "sccp-bsc"),
  ].some((rootPath) => pathIsWithin(resolvedPath, rootPath));
}

function routeManifestProductionProblems(record, label) {
  if (
    !isRecord(record) ||
    readFirstString(record, "schema") !== ROUTE_MANIFEST_SCHEMA
  ) {
    return [];
  }
  const problems = [];
  let route = null;
  try {
    route = normalizeRouteManifestForConfig(record);
  } catch (error) {
    problems.push(
      `${label} failed production route validation: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
  const productionReady =
    readFirstValue(record, "productionReady", "production_ready") === true;
  const handoffPlaceholderReason = productionHandoffPlaceholderReason(
    record,
    label,
  );
  if (!productionReady) {
    problems.push(`${label} is not productionReady true`);
  } else if (handoffPlaceholderReason) {
    problems.push(
      `${label} contains production handoff placeholder material at ${handoffPlaceholderReason}`,
    );
  }
  if (route) {
    if (route.disabledReason) {
      problems.push(`${label} carries disabledReason`);
    }
    if (!route.proofArtifactHash || !route.provingKeyHash) {
      problems.push(`${label} is missing proofArtifactHash or provingKeyHash`);
    }
    if (!route.nativeEvmProverBundle) {
      problems.push(`${label} is missing nativeEvmProverBundle`);
    }
    if (!route.postDeployLiveEvidence?.fullTomlReady) {
      problems.push(`${label} is missing full postDeployLiveEvidence`);
    }
  }
  return uniqueNonEmpty(problems);
}

function requireExplicitBscNativeEvmVerifierKeyArtifactHash(
  record,
  label,
  verifierKeyHash = "",
) {
  const entries = collectStringEntries(
    record,
    NATIVE_EVM_PROVER_BUNDLE_VERIFIER_KEY_ARTIFACT_HASH_KEYS,
    label,
  );
  if (entries.length === 0) {
    throw new Error(
      `${label} verifierKeyArtifactHash is required for production BSC native EVM prover bundles.`,
    );
  }
  if (entries.length > 1) {
    throw new Error(
      `${label} verifierKeyArtifactHash must not use multiple aliases: ${entries
        .map((entry) => entry.key)
        .join(", ")}.`,
    );
  }
  const verifierKeyArtifactHash = normalizeCanonicalHex32(
    entries[0].value,
    `${entries[0].path}`,
  );
  if (verifierKeyHash && verifierKeyArtifactHash === verifierKeyHash) {
    throw new Error(
      `${label} verifierKeyArtifactHash must be role-separated from verifierKeyHash.`,
    );
  }
  return verifierKeyArtifactHash;
}

function bscNativeEvmProverBundleRoleSeparatedHashProblems(
  record,
  label,
  overrides = {},
) {
  if (!isRecord(record)) {
    return [];
  }
  const problems = [];
  const seen = new Map();
  for (const [role, keys] of NATIVE_EVM_PROVER_ROLE_SEPARATED_HASH_FIELDS) {
    let value = overrides[role];
    if (!value) {
      try {
        value = readConsistentNormalizedString(
          [{ record, keys, pathName: label }],
          `${label} ${role}`,
          (entryValue, fieldLabel) =>
            normalizeCanonicalHex32(entryValue, fieldLabel),
        );
      } catch (_error) {
        value = "";
      }
    }
    if (!value) {
      continue;
    }
    const previous = seen.get(value);
    if (previous) {
      problems.push(
        `${label} ${role} must be role-separated from ${previous}.`,
      );
    } else {
      seen.set(value, role);
    }
  }
  return problems;
}

function nativeProverBundleProductionProblems(record, label) {
  if (!isRecord(record)) {
    return [];
  }
  const schema = readFirstString(record, "schema");
  const bundleId = readFirstString(record, "bundleId", "bundle_id");
  if (
    schema !== SCCP_NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1 &&
    bundleId !== SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1 &&
    bundleId !== SCCP_BSC_MAINNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1
  ) {
    return [];
  }

  const problems = [];
  let descriptor = null;
  let verifierKeyArtifactHash = "";
  const profile =
    bundleId === SCCP_BSC_MAINNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1 ||
    readFirstString(record, "chain") === "bsc-mainnet"
      ? BSC_NETWORK_PROFILES.mainnet
      : BSC_NETWORK_PROFILES.testnet;
  try {
    descriptor = validateBscNativeEvmProverBundleForProfile(record, profile);
  } catch (error) {
    problems.push(
      `${label} failed BSC native EVM prover bundle validation: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
  try {
    verifierKeyArtifactHash =
      requireExplicitBscNativeEvmVerifierKeyArtifactHash(
        record,
        label,
        descriptor?.verifierKeyHash,
      );
  } catch (error) {
    problems.push(error instanceof Error ? error.message : String(error));
  }
  const verifierKeyHash =
    descriptor?.verifierKeyHash ??
    readConsistentNormalizedString(
      [
        {
          record,
          keys: ["verifierKeyHash", "verifier_key_hash"],
          pathName: label,
        },
      ],
      `${label} verifierKeyHash`,
      (value, fieldLabel) => normalizeHex32(value, fieldLabel),
    );
  if (verifierKeyHash && isKnownDiagnosticBscVerifierKeyHash(verifierKeyHash)) {
    problems.push(
      `${label} verifierKeyHash=${verifierKeyHash} is a known diagnostic BSC verifier key hash`,
    );
  }
  if (
    verifierKeyArtifactHash &&
    verifierKeyHash &&
    verifierKeyArtifactHash === verifierKeyHash
  ) {
    problems.push(
      `${label} verifierKeyArtifactHash must be role-separated from verifierKeyHash.`,
    );
  }
  problems.push(
    ...bscNativeEvmProverBundleRoleSeparatedHashProblems(record, label, {
      verifierKeyHash,
      verifierKeyArtifactHash,
      proofArtifactHash: descriptor?.proofArtifactHash,
      provingKeyHash: descriptor?.provingKeyHash,
      nativeEvmProverBundleHash: descriptor
        ? canonicalBscNativeEvmProverBundleHash(descriptor)
        : "",
      destinationBindingHash: descriptor?.destinationBindingHash,
    }),
  );
  return uniqueNonEmpty(problems);
}

export function canonicalBscNativeEvmProverBundleHash(bundle) {
  return bytesToHex(
    sha256(
      textEncoder.encode(
        JSON.stringify({
          schema: bundle.schema,
          bundleId: bundle.bundleId,
          domain: bundle.domain,
          chain: bundle.chain,
          proofBackend: bundle.proofBackend,
          proofArtifact: bundle.proofArtifact,
          proofArtifactHash: bundle.proofArtifactHash,
          provingKey: bundle.provingKey,
          provingKeyHash: bundle.provingKeyHash,
          verifierKey: bundle.verifierKey,
          verifierKeyHash: bundle.verifierKeyHash,
          verifierKeyArtifactHash: bundle.verifierKeyArtifactHash,
          destinationBindingHash: bundle.destinationBindingHash,
          noWasm: bundle.noWasm,
          remoteProverRequired: bundle.remoteProverRequired,
          browserImplementation: bundle.browserImplementation,
          nativeSdkArtifacts: bundle.nativeSdkArtifacts,
          crossSdkFixtureParityArtifact: bundle.crossSdkFixtureParityArtifact,
          nativeProverSelfTestArtifact: bundle.nativeProverSelfTestArtifact,
          auditHashes: bundle.auditHashes,
        }),
      ),
    ),
  );
}

function bscDiagnosticProductionMaterialReasons(record, label) {
  if (!isRecord(record)) {
    return [];
  }
  const rollout = readFirstRecord(
    record,
    "destinationRollout",
    "destination_rollout",
  );
  const binding = readFirstRecord(
    record,
    "destinationBinding",
    "destination_binding",
  );
  const verifierKeyHash = readConsistentNormalizedString(
    [
      {
        record,
        keys: ["verifierKeyHash", "verifier_key_hash"],
        pathName: label,
      },
      {
        record: rollout,
        keys: ["verifierKeyHash", "verifier_key_hash"],
        pathName: `${label}.destinationRollout`,
      },
    ],
    `${label} verifierKeyHash`,
    (value, fieldLabel) => normalizeHex32(value, fieldLabel),
  );
  return uniqueNonEmpty([
    diagnosticFlagReason(record, label),
    diagnosticFlagReason(rollout, `${label}.destinationRollout`),
    diagnosticFlagReason(binding, `${label}.destinationBinding`),
    verifierKeyHash && isKnownDiagnosticBscVerifierKeyHash(verifierKeyHash)
      ? `${label} verifierKeyHash=${verifierKeyHash} is a known diagnostic BSC verifier key hash`
      : "",
  ]);
}

function bscProductionHandoffPlaceholderReasons(record, label) {
  if (!isRecord(record)) {
    return [];
  }
  if (readFirstString(record, "schema") === PRODUCTION_REQUIREMENTS_SCHEMA) {
    return [];
  }
  const reason = productionHandoffPlaceholderReason(record, label);
  return reason
    ? [`${label} contains production handoff placeholder material at ${reason}`]
    : [];
}

export function bscCanonicalProductionOutputProblems(
  pathName,
  value,
  label = "BSC production material",
) {
  if (!isCanonicalBscProductionArtifactPath(pathName)) {
    return [];
  }
  return uniqueNonEmpty([
    ...nativeProverBundleProductionProblems(value, label),
    ...bscDiagnosticProductionMaterialReasons(value, label),
    ...bscProductionHandoffPlaceholderReasons(value, label),
    ...routeManifestProductionProblems(value, label),
  ]);
}

function assertBscCanonicalProductionOutputSafe(pathName, value, label) {
  const problems = bscCanonicalProductionOutputProblems(pathName, value, label);
  if (problems.length > 0) {
    throw new Error(
      `${label} cannot be written to canonical BSC production artifact path ${resolve(
        pathName,
      )}: ${problems.join("; ")}. Write draft or diagnostic material under output/sccp-bsc-production/ or regenerate with production verifier, proof, native-prover, and live readback material.`,
    );
  }
}

function normalizeNonEmptyText(value, label) {
  const normalized = trim(value);
  if (!normalized) {
    throw new Error(`${label} is required.`);
  }
  return normalized;
}

function normalizeCanonicalManifestText(value, label) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.trim() !== value
  ) {
    throw new Error(`${label} must be a non-empty canonical string.`);
  }
  return value;
}

function readOptionalCanonicalManifestText(
  record,
  keys,
  label,
  { allowNull = false } = {},
) {
  let selected;
  let selectedKey = "";
  for (const key of keys) {
    if (!hasOwn(record, key)) {
      continue;
    }
    const value = ownValue(record, key);
    if (value === undefined) {
      continue;
    }
    const normalized =
      value === null && allowNull
        ? null
        : normalizeCanonicalManifestText(value, label);
    if (selected !== undefined && selected !== normalized) {
      throw new Error(
        `${label} aliases disagree: ${selectedKey}=${selected} but ${key}=${normalized}.`,
      );
    }
    selected = normalized;
    selectedKey = key;
  }
  return selected === undefined ? null : selected;
}

function normalizeCanonicalStringList(value, label) {
  if (value === undefined || value === null) {
    return [];
  }
  if (!Array.isArray(value)) {
    throw new Error(`${label} must be a list of non-empty strings.`);
  }
  return ownArrayValues(value).map(([index, entry]) => {
    if (
      typeof entry !== "string" ||
      entry.length === 0 ||
      entry.trim() !== entry
    ) {
      throw new Error(
        `${label}[${index}] must be a non-empty canonical string.`,
      );
    }
    return entry;
  });
}

function postDeployLiveEvidenceProductionBlockers(record) {
  if (!isRecord(record)) {
    return [];
  }
  const blockers = [];
  for (const key of POST_DEPLOY_LIVE_EVIDENCE_BLOCKER_KEYS) {
    if (!hasOwn(record, key)) {
      continue;
    }
    for (const blocker of normalizeCanonicalStringList(
      ownValue(record, key),
      `route manifest postDeployLiveEvidence.${key}`,
    )) {
      blockers.push(`${key}: ${blocker}`);
    }
  }
  return blockers;
}

function normalizeCanonicalAssetDefinitionId(value, label) {
  const normalized = normalizeNonEmptyText(value, label);
  if (normalized.includes("#") || normalized.toLowerCase() === "xor") {
    throw new Error(
      `${label} must be a canonical Base58 asset definition ID, not an alias.`,
    );
  }
  if (!/^[1-9A-HJ-NP-Za-km-z]{16,80}$/u.test(normalized)) {
    throw new Error(`${label} must be a canonical Base58 asset definition ID.`);
  }
  return normalized;
}

function normalizeStrictBase64(value, label) {
  const normalized = normalizeNonEmptyText(value, label);
  if (
    normalized.length < 8 ||
    normalized.length % 4 !== 0 ||
    !/^[A-Za-z0-9+/]+={0,2}$/u.test(normalized)
  ) {
    throw new Error(`${label} must be strict base64.`);
  }
  const decoded = Buffer.from(normalized, "base64");
  if (decoded.length === 0 || decoded.toString("base64") !== normalized) {
    throw new Error(`${label} must be canonical strict base64.`);
  }
  return { text: normalized, bytes: decoded };
}

function normalizePositiveSafeInteger(value, label, fallback = undefined) {
  const source =
    value === undefined || value === null || value === "" ? fallback : value;
  const parsed = typeof source === "number" ? source : Number(source);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive safe integer.`);
  }
  return parsed;
}

function optionEnabled(options, key, fallback = false) {
  const value = ownValue(options, key);
  if (value === undefined || value === null || value === "") {
    return fallback;
  }
  if (value === "true") return true;
  if (value === "false") return false;
  throw new Error(`--${key} must be true or false.`);
}

function secretLikeTextReason(value, pathName) {
  const normalized = value.trim().replace(/\s+/gu, " ");
  if (PRIVATE_KEY_PEM_PATTERN.test(normalized)) {
    return `${pathName} must not contain private key material.`;
  }
  for (const match of normalized.matchAll(SECRET_ASSIGNMENT_PATTERN)) {
    const assignmentValue = String(match[1] ?? "")
      .trim()
      .replace(/^['"]|['"]$/gu, "");
    if (!REDACTED_SECRET_ASSIGNMENT_VALUE_PATTERN.test(assignmentValue)) {
      return `${pathName} must not contain private key, token, or secret material.`;
    }
  }
  if (BEARER_TOKEN_TEXT_PATTERN.test(normalized)) {
    return `${pathName} must not contain private key, token, or secret material.`;
  }
  const words = normalized.split(" ");
  if (
    RECOVERY_PHRASE_WORD_COUNTS.has(words.length) &&
    words.every((word) => /^[a-z]+$/iu.test(word))
  ) {
    return `${pathName} must not contain recovery phrases.`;
  }
  return "";
}

export function unsafeSecretReason(
  value,
  pathName = "deployment evidence",
  seen = new WeakSet(),
) {
  if (typeof value === "string") {
    return secretLikeTextReason(value, pathName);
  }
  if (Array.isArray(value)) {
    if (seen.has(value)) {
      return "";
    }
    seen.add(value);
    for (const [index, child] of ownArrayValues(value)) {
      const reason = unsafeSecretReason(child, `${pathName}[${index}]`, seen);
      if (reason) {
        return reason;
      }
    }
    return "";
  }
  if (!isRecord(value)) {
    return "";
  }
  if (seen.has(value)) {
    return "";
  }
  seen.add(value);
  for (const [key, child] of ownRecordEntries(value)) {
    if (SECRET_KEY_PATTERN.test(key)) {
      return `${pathName}.${key} must not contain private key, token, or secret material.`;
    }
    const reason = unsafeSecretReason(child, `${pathName}.${key}`, seen);
    if (reason) {
      return reason;
    }
  }
  return "";
}

export function validateBscReadbackEvidence(input = {}) {
  const addresses = ownValue(input, "addresses");
  const readback = ownValue(input, "readback");
  const bindingHash = ownValue(input, "bindingHash");
  const verifierCodeHash = ownValue(input, "verifierCodeHash");
  const verifierKeyHash = ownValue(input, "verifierKeyHash");
  const bscNetwork = ownValue(input, "bscNetwork") ?? "testnet";
  const profile = normalizeBscNetworkProfile(bscNetwork);
  if (!isRecord(readback)) {
    throw new Error("BSC contract readback must be an object.");
  }
  const chainIdHex = ownValue(readback, "chainIdHex");
  if (String(chainIdHex).toLowerCase() !== profile.chainIdHex) {
    throw new Error(
      `BSC contract readback must report ${profile.label} chain id ${profile.chainIdHex}.`,
    );
  }
  const readbackCodePresent = ownValue(readback, "codePresent");
  const codePresent = isRecord(readbackCodePresent) ? readbackCodePresent : {};
  for (const key of ["token", "bridge", "sourceBridge", "verifier"]) {
    if (ownValue(codePresent, key) !== true) {
      throw new Error(`BSC contract readback must confirm ${key} bytecode.`);
    }
  }
  if (
    normalizeEvmAddress(
      ownValue(readback, "tokenBridgeAddress"),
      "tokenBridgeAddress",
    ) !== ownValue(addresses, "bridge")
  ) {
    throw new Error("BSC readback token bridge does not match route bridge.");
  }
  if (ownValue(readback, "tokenBridgeLocked") !== true) {
    throw new Error("BSC readback token bridge must be locked.");
  }
  if (
    normalizeEvmAddress(
      ownValue(readback, "sourceBridgeOwner"),
      "sourceBridgeOwner",
    ) !== ownValue(addresses, "bridge")
  ) {
    throw new Error(
      "BSC readback source bridge owner does not match route bridge.",
    );
  }
  if (
    normalizeHex32(
      ownValue(readback, "bridgeDestinationBindingHash"),
      "bridgeDestinationBindingHash",
    ) !== bindingHash
  ) {
    throw new Error(
      "BSC readback bridge destination binding hash does not match.",
    );
  }
  if (
    normalizeEvmAddress(
      ownValue(readback, "bridgeVerifierAddress"),
      "bridgeVerifierAddress",
    ) !== ownValue(addresses, "verifier")
  ) {
    throw new Error(
      "BSC readback bridge verifier address does not match verifier.",
    );
  }
  if (
    normalizeHex32(
      ownValue(readback, "bridgeVerifierCodeHash"),
      "bridgeVerifierCodeHash",
    ) !== verifierCodeHash
  ) {
    throw new Error("BSC readback bridge verifier code hash does not match.");
  }
  if (
    normalizeHex32(
      ownValue(readback, "bridgeVerifierKeyHash"),
      "bridgeVerifierKeyHash",
    ) !== verifierKeyHash
  ) {
    throw new Error("BSC readback bridge verifier key hash does not match.");
  }
  if (
    normalizeHex32(ownValue(readback, "verifierKeyHash"), "verifierKeyHash") !==
    verifierKeyHash
  ) {
    throw new Error(
      "BSC readback deployed verifier key hash does not match declared verifier key hash.",
    );
  }
  if (
    normalizeHex32(ownValue(readback, "bridgeNetworkId"), "bridgeNetworkId") !==
    profile.networkIdHex
  ) {
    throw new Error(`BSC readback bridge network id must be ${profile.label}.`);
  }
  if (
    ownValue(readback, "bridgeSourceDomain") !== SCCP_DOMAIN_SORA ||
    ownValue(readback, "bridgeTargetDomain") !== SCCP_DOMAIN_BSC
  ) {
    throw new Error("BSC readback bridge domains must be SORA to BSC.");
  }
  return true;
}

function requireOptionalPackage(name) {
  try {
    return requireFromScript(name);
  } catch (firstError) {
    try {
      return requireFromCwd(name);
    } catch (_secondError) {
      throw new Error(
        `${name} is required. Install it or run with NODE_PATH pointing to a directory containing ${name}. Original error: ${firstError.message}`,
      );
    }
  }
}

const parseJsonStringToken = (text, start) => {
  let index = start + 1;
  while (index < text.length) {
    const char = text[index];
    if (char === '"') {
      return {
        value: JSON.parse(text.slice(start, index + 1)),
        end: index,
      };
    }
    if (char === "\\") {
      index += 2;
      continue;
    }
    index += 1;
  }
  return null;
};

const nextNonWhitespaceIndex = (text, start) => {
  let index = start;
  while (index < text.length && /\s/u.test(text[index])) {
    index += 1;
  }
  return index;
};

const duplicateJsonObjectKeyReason = (text, label) => {
  const stack = [];
  for (let index = 0; index < text.length; index += 1) {
    const char = text[index];
    if (char === '"') {
      const token = parseJsonStringToken(text, index);
      if (!token) {
        return "";
      }
      const current = stack.at(-1);
      if (
        current?.type === "object" &&
        current.expectingKey === true &&
        text[nextNonWhitespaceIndex(text, token.end + 1)] === ":"
      ) {
        if (current.keys.has(token.value)) {
          return `${label} contains a duplicate JSON object key.`;
        }
        current.keys.add(token.value);
        current.expectingKey = false;
      }
      index = token.end;
      continue;
    }
    if (char === "{") {
      stack.push({ type: "object", keys: new Set(), expectingKey: true });
      continue;
    }
    if (char === "[") {
      stack.push({ type: "array" });
      continue;
    }
    if (char === "}" || char === "]") {
      stack.pop();
      continue;
    }
    if (char === ",") {
      const current = stack.at(-1);
      if (current?.type === "object") {
        current.expectingKey = true;
      }
    }
  }
  return "";
};

export const parseJsonWithoutDuplicateKeys = (text, label = "JSON file") => {
  const duplicateReason = duplicateJsonObjectKeyReason(text, label);
  if (duplicateReason) {
    throw new Error(duplicateReason);
  }
  return JSON.parse(text);
};

async function readJson(pathName, label = "JSON file") {
  let text;
  const resolved = resolve(pathName);
  try {
    const info = await lstat(resolved);
    if (info.isSymbolicLink()) {
      throw new Error("path must not be a symbolic link");
    }
    if (!info.isFile()) {
      throw new Error("path must be a regular file");
    }
    if (info.size > SCCP_BSC_JSON_INPUT_MAX_BYTES) {
      throw new Error(
        `path is ${info.size} bytes; maximum allowed is ${SCCP_BSC_JSON_INPUT_MAX_BYTES} bytes`,
      );
    }
    text = await readFile(resolved, "utf8");
  } catch (error) {
    throw new Error(`${label} could not be read: ${error.message}`);
  }
  try {
    const parsed = parseJsonWithoutDuplicateKeys(text, label);
    if (!isRecord(parsed)) {
      throw new Error(`${label} must be a JSON object.`);
    }
    return parsed;
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${error.message}`);
  }
}

async function readText(pathName, label = "text file") {
  const resolved = resolve(pathName);
  try {
    const info = await lstat(resolved);
    if (info.isSymbolicLink()) {
      throw new Error("path must not be a symbolic link");
    }
    if (!info.isFile()) {
      throw new Error("path must be a regular file");
    }
    if (info.size > SCCP_BSC_TEXT_INPUT_MAX_BYTES) {
      throw new Error(
        `path is ${info.size} bytes; maximum allowed is ${SCCP_BSC_TEXT_INPUT_MAX_BYTES} bytes`,
      );
    }
    return await readFile(resolved, "utf8");
  } catch (error) {
    throw new Error(`${label} could not be read: ${error.message}`);
  }
}

async function writeJsonNoSecrets(pathName, value) {
  const reason = unsafeSecretReason(value);
  if (reason) {
    throw new Error(reason);
  }
  const resolved = resolve(pathName);
  await mkdir(dirname(resolved), { recursive: true });
  const temp = `${resolved}.tmp-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  await writeFile(temp, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o644 });
  await rename(temp, resolved);
}

async function writeTextNoSecrets(pathName, value, mode = 0o644) {
  const reason = unsafeSecretReason(value);
  if (reason) {
    throw new Error(reason);
  }
  const resolved = resolve(pathName);
  await mkdir(dirname(resolved), { recursive: true });
  const temp = `${resolved}.tmp-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  await writeFile(temp, value, { mode });
  await rename(temp, resolved);
  return resolved;
}

function optionValue(options, names) {
  for (const name of Array.isArray(names) ? names : [names]) {
    const value = ownValue(options, name);
    if (value !== undefined) {
      return value;
    }
  }
  return undefined;
}

function requiredOption(options, names, label) {
  const value = optionValue(options, names);
  if (value === undefined || value === null || trim(value) === "") {
    const display = Array.isArray(names) ? names[0] : names;
    throw new Error(`${label} requires --${display}.`);
  }
  return value;
}

function artifactRootPath(value) {
  return resolve(trim(value) || DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT);
}

function relativePosixPath(root, target, label) {
  const relativePath = relative(root, target);
  if (
    !relativePath ||
    relativePath.startsWith("..") ||
    isAbsolute(relativePath)
  ) {
    throw new Error(`${label} must stay under artifact-root.`);
  }
  return relativePath.split(/[\\/]+/u).join("/");
}

function pathHasDecodedParentSegment(value) {
  let normalized = String(value ?? "").replace(/\\/gu, "/");
  for (let depth = 0; depth < 8; depth += 1) {
    if (/(?:^|\/)\.\.(?:\/|$)/u.test(normalized)) {
      return true;
    }
    let decoded;
    try {
      decoded = decodeURIComponent(normalized).replace(/\\/gu, "/");
    } catch (_error) {
      return true;
    }
    if (decoded === normalized) {
      return false;
    }
    normalized = decoded;
  }
  // Still-changing values are over-encoded; fail closed rather than relying
  // on a specific decode depth across downstream consumers.
  return true;
}

function assertBundleArtifactPathSafe(pathName, label) {
  if (pathHasDecodedParentSegment(pathName)) {
    throw new Error(
      `${label} must not use URL-encoded parent-directory segments.`,
    );
  }
}

async function readArtifactUnderRoot(root, value, label) {
  const text = normalizeNonEmptyText(value, label);
  const target = isAbsolute(text) ? resolve(text) : resolve(root, text);
  const pathName = relativePosixPath(root, target, label);
  assertBundleArtifactPathSafe(pathName, label);
  let bytes;
  try {
    const info = await lstat(target);
    if (info.isSymbolicLink()) {
      throw new Error("path must not be a symbolic link");
    }
    if (!info.isFile()) {
      throw new Error("path must be a regular file");
    }
    if (info.size > SCCP_BSC_BINARY_ARTIFACT_INPUT_MAX_BYTES) {
      throw new Error(
        `path is ${info.size} bytes; maximum allowed is ${SCCP_BSC_BINARY_ARTIFACT_INPUT_MAX_BYTES} bytes`,
      );
    }
    const [rootRealPath, targetRealPath] = await Promise.all([
      realpath(root),
      realpath(target),
    ]);
    relativePosixPath(rootRealPath, targetRealPath, label);
    bytes = await readFile(targetRealPath);
  } catch (error) {
    throw new Error(`${label} could not be read: ${error.message}`);
  }
  if (bytes.length === 0) {
    throw new Error(`${label} must not be empty.`);
  }
  return {
    absolutePath: target,
    path: pathName,
    bytes,
    sha256: bytesToHex(sha256(new Uint8Array(bytes))),
    sizeBytes: bytes.length,
  };
}

const PRODUCTION_PROOF_MATERIAL_SHAPE_MIN_BYTES = 4096;
const PRODUCTION_PROOF_MATERIAL_MIN_UNIQUE_BYTES = 16;
const PRODUCTION_PROOF_MATERIAL_MAX_REPEATED_PATTERN_BYTES = 64;
const PRODUCTION_PROOF_MATERIAL_MAX_DOMINANT_BYTE_FRACTION = 0.98;
const PRODUCTION_BURN_RECORD_ARTIFACT_MIN_UNIQUE_BYTES = 16;
const PRODUCTION_BURN_RECORD_ARTIFACT_MAX_DOMINANT_BYTE_FRACTION = 0.98;
const SNARKJS_R1CS_MAGIC = [0x72, 0x31, 0x63, 0x73];
const SNARKJS_ZKEY_MAGIC = [0x7a, 0x6b, 0x65, 0x79];

function repeatedPrefixPatternLength(
  bytes,
  maxPatternLength = PRODUCTION_PROOF_MATERIAL_MAX_REPEATED_PATTERN_BYTES,
) {
  const maxLength = Math.min(maxPatternLength, Math.floor(bytes.length / 2));
  for (let length = 1; length <= maxLength; length += 1) {
    let repeated = true;
    for (let index = length; index < bytes.length; index += 1) {
      if (bytes[index] !== bytes[index % length]) {
        repeated = false;
        break;
      }
    }
    if (repeated) {
      return length;
    }
  }
  return 0;
}

function constantByteDelta(bytes) {
  if (bytes.length < 16) {
    return null;
  }
  const delta = (bytes[1] - bytes[0] + 256) & 0xff;
  for (let index = 2; index < bytes.length; index += 1) {
    if (((bytes[index] - bytes[index - 1] + 256) & 0xff) !== delta) {
      return null;
    }
  }
  return delta;
}

function dominantByteFrequency(bytes) {
  const counts = new Uint32Array(256);
  let dominantByte = 0;
  let dominantCount = 0;
  for (const byte of bytes) {
    const count = counts[byte] + 1;
    counts[byte] = count;
    if (count > dominantCount) {
      dominantByte = byte;
      dominantCount = count;
    }
  }
  return { byte: dominantByte, count: dominantCount };
}

function u32le(bytes, offset) {
  return (
    (bytes[offset] |
      (bytes[offset + 1] << 8) |
      (bytes[offset + 2] << 16) |
      (bytes[offset + 3] << 24)) >>>
    0
  );
}

function u64leSafe(bytes, offset) {
  const low = u32le(bytes, offset);
  const high = u32le(bytes, offset + 4);
  const value = high * 0x100000000 + low;
  return Number.isSafeInteger(value) ? value : null;
}

function hasBytePrefix(bytes, prefix) {
  return prefix.every((byte, index) => bytes[index] === byte);
}

function assertSnarkjsBinaryHeader(artifact, label, magic, formatLabel) {
  const bytes = artifact.bytes;
  if (bytes.length < 12) {
    throw new Error(`${label} ${formatLabel} header is truncated.`);
  }
  if (!hasBytePrefix(bytes, magic)) {
    throw new Error(`${label} must start with ${formatLabel} magic bytes.`);
  }
  const version = u32le(bytes, 4);
  if (version < 1 || version > 2) {
    throw new Error(`${label} ${formatLabel} version is unsupported.`);
  }
  const sectionCount = u32le(bytes, 8);
  if (sectionCount < 1 || sectionCount > 128) {
    throw new Error(`${label} ${formatLabel} section count is invalid.`);
  }
  let offset = 12;
  const sectionIds = new Set();
  for (let index = 0; index < sectionCount; index += 1) {
    if (offset + 12 > bytes.length) {
      throw new Error(`${label} ${formatLabel} section table is truncated.`);
    }
    const sectionId = u32le(bytes, offset);
    const sectionSize = u64leSafe(bytes, offset + 4);
    offset += 12;
    if (sectionId === 0) {
      throw new Error(`${label} ${formatLabel} section id must be non-zero.`);
    }
    if (sectionIds.has(sectionId)) {
      throw new Error(`${label} ${formatLabel} section ids must be unique.`);
    }
    sectionIds.add(sectionId);
    if (sectionSize === null || sectionSize <= 0) {
      throw new Error(`${label} ${formatLabel} section size is invalid.`);
    }
    if (sectionSize > bytes.length - offset) {
      throw new Error(`${label} ${formatLabel} section exceeds file size.`);
    }
    offset += sectionSize;
  }
  if (offset !== bytes.length) {
    throw new Error(
      `${label} ${formatLabel} section table does not consume the full file.`,
    );
  }
}

function assertProductionProofMaterialFormat(artifact, label, kind) {
  const extension = extname(artifact.path).toLowerCase();
  if (kind === "proof-artifact") {
    if (extension === ".r1cs") {
      assertSnarkjsBinaryHeader(artifact, label, SNARKJS_R1CS_MAGIC, ".r1cs");
      return;
    }
    throw new Error(
      `${label} must be a .r1cs artifact; received ${artifact.path}.`,
    );
  }
  if (kind === "proving-key") {
    if (extension === ".zkey") {
      assertSnarkjsBinaryHeader(artifact, label, SNARKJS_ZKEY_MAGIC, ".zkey");
      return;
    }
    throw new Error(
      `${label} must be a .zkey artifact; received ${artifact.path}.`,
    );
  }
}

function assertProductionProofMaterialShape(artifact, label, kind = null) {
  const bytes = artifact.bytes;
  if (bytes.length < PRODUCTION_PROOF_MATERIAL_SHAPE_MIN_BYTES) {
    return;
  }
  const repeatedPatternLength = repeatedPrefixPatternLength(bytes);
  if (repeatedPatternLength > 0) {
    throw new Error(
      `${label} looks like placeholder proof material: repeated ${repeatedPatternLength}-byte pattern.`,
    );
  }
  const arithmeticDelta = constantByteDelta(bytes);
  if (arithmeticDelta !== null) {
    throw new Error(
      `${label} looks like placeholder proof material: arithmetic byte sequence with step ${arithmeticDelta}.`,
    );
  }
  const dominant = dominantByteFrequency(bytes);
  if (
    dominant.count / bytes.length >
    PRODUCTION_PROOF_MATERIAL_MAX_DOMINANT_BYTE_FRACTION
  ) {
    throw new Error(
      `${label} looks like placeholder proof material: byte 0x${dominant.byte
        .toString(16)
        .padStart(
          2,
          "0",
        )} dominates ${dominant.count} of ${bytes.length} bytes.`,
    );
  }
  const uniqueBytes = new Set();
  for (const byte of bytes) {
    uniqueBytes.add(byte);
    if (uniqueBytes.size >= PRODUCTION_PROOF_MATERIAL_MIN_UNIQUE_BYTES) {
      break;
    }
  }
  if (uniqueBytes.size >= PRODUCTION_PROOF_MATERIAL_MIN_UNIQUE_BYTES) {
    if (kind) {
      assertProductionProofMaterialFormat(artifact, label, kind);
    }
    return;
  }
  throw new Error(
    `${label} looks like placeholder proof material: only ${uniqueBytes.size} unique byte values across ${bytes.length} bytes.`,
  );
}

function assertProductionBurnRecordArtifactShape(bytes, label) {
  if (bytes.length < TAIRA_BURN_RECORD_PRODUCTION_ARTIFACT_MIN_BYTES) {
    throw new Error(
      `${label} must be at least ${TAIRA_BURN_RECORD_PRODUCTION_ARTIFACT_MIN_BYTES} bytes for production-ready BSC routes.`,
    );
  }
  const decodedText = Buffer.from(bytes).toString("utf8");
  if (PLACEHOLDER_BURN_RECORD_TEXT_PATTERN.test(decodedText)) {
    throw new Error(
      `${label} looks like placeholder burn-record material: text contains fixture, diagnostic, mock, stub, dummy, or placeholder markers.`,
    );
  }
  const repeatedPatternLength = repeatedPrefixPatternLength(bytes);
  if (repeatedPatternLength > 0) {
    throw new Error(
      `${label} looks like placeholder burn-record material: repeated ${repeatedPatternLength}-byte pattern.`,
    );
  }
  const arithmeticDelta = constantByteDelta(bytes);
  if (arithmeticDelta !== null) {
    throw new Error(
      `${label} looks like placeholder burn-record material: arithmetic byte sequence with step ${arithmeticDelta}.`,
    );
  }
  const dominant = dominantByteFrequency(bytes);
  if (
    dominant.count / bytes.length >
    PRODUCTION_BURN_RECORD_ARTIFACT_MAX_DOMINANT_BYTE_FRACTION
  ) {
    throw new Error(
      `${label} looks like placeholder burn-record material: byte 0x${dominant.byte
        .toString(16)
        .padStart(
          2,
          "0",
        )} dominates ${dominant.count} of ${bytes.length} bytes.`,
    );
  }
  const uniqueBytes = new Set();
  for (const byte of bytes) {
    uniqueBytes.add(byte);
    if (uniqueBytes.size >= PRODUCTION_BURN_RECORD_ARTIFACT_MIN_UNIQUE_BYTES) {
      return;
    }
  }
  throw new Error(
    `${label} looks like placeholder burn-record material: only ${uniqueBytes.size} unique byte values across ${bytes.length} bytes.`,
  );
}

function parseBscVerifierKeyArtifact(artifact, profile) {
  if (extname(artifact.path).toLowerCase() !== ".json") {
    throw new Error(
      `verifier key must be a production Groth16 verifier JSON artifact; received ${artifact.path}.`,
    );
  }
  let material;
  try {
    material = parseJsonWithoutDuplicateKeys(
      artifact.bytes.toString("utf8"),
      "verifier key",
    );
  } catch (error) {
    throw new Error(
      `verifier key must be valid duplicate-free JSON: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
  const normalized = normalizeVerifierMaterial(material, profile);
  if (normalized.fixtureShaped) {
    throw new Error(
      "verifier key uses deterministic smoke-test Groth16 fixture material.",
    );
  }
  if (normalized.diagnosticVerifierReasons.length > 0) {
    throw new Error(
      `verifier key uses diagnostic BSC verifier material: ${normalized.diagnosticVerifierReasons.join("; ")}.`,
    );
  }
  return normalized;
}

function assertProductionAuditHashLiteral(value, label) {
  const normalized = normalizeHex32(value, label);
  const bytes = Buffer.from(normalized.slice(2), "hex");
  if (bytes.every((byte) => byte === 0)) {
    throw new Error(`${label} must not be the zero hash.`);
  }
  const repeatedPatternLength = repeatedPrefixPatternLength(bytes, 16);
  if (repeatedPatternLength > 0) {
    throw new Error(
      `${label} looks like placeholder audit hash: repeated ${repeatedPatternLength}-byte pattern.`,
    );
  }
  const arithmeticDelta = constantByteDelta(bytes);
  if (arithmeticDelta !== null) {
    throw new Error(
      `${label} looks like placeholder audit hash: arithmetic byte sequence with step ${arithmeticDelta}.`,
    );
  }
  return normalized;
}

async function normalizeAuditHashOrFile(root, value, label) {
  const text = normalizeNonEmptyText(value, label);
  if (/^(?:0x)?[0-9a-f]{64}$/iu.test(text)) {
    return assertProductionAuditHashLiteral(text, label);
  }
  if (text.startsWith("0x")) {
    throw new Error(`${label} must be a 32-byte hex hash or artifact file.`);
  }
  return (await readArtifactUnderRoot(root, text, label)).sha256;
}

async function readNativeProverAuditHashes(
  root,
  options,
  { parityFixture, selfTestFixture } = {},
) {
  const entries = [];
  for (const [key, optionNames] of Object.entries(
    NATIVE_EVM_PROVER_AUDIT_OPTION_KEYS,
  )) {
    const derived =
      key === "cross_sdk_fixture_parity"
        ? parityFixture?.sha256
        : key === "native_prover_self_test"
          ? selfTestFixture?.sha256
          : null;
    const raw = optionValue(options, optionNames);
    if (derived && (raw === undefined || raw === null || trim(raw) === "")) {
      entries.push([key, derived]);
      continue;
    }
    const normalized = await normalizeAuditHashOrFile(
      root,
      requiredOption(options, optionNames, `auditHashes.${key}`),
      `auditHashes.${key}`,
    );
    if (derived && normalized !== derived) {
      throw new Error(`auditHashes.${key} must match the artifact sha256.`);
    }
    entries.push([key, normalized]);
  }
  return Object.fromEntries(entries);
}

function extractBscBundleRouteBinding(record, label) {
  if (!isRecord(record)) {
    throw new Error(`${label} must be a JSON object.`);
  }
  const destinationRollout =
    readFirstRecord(record, "destinationRollout", "destination_rollout") ?? {};
  const destinationBinding =
    readFirstRecord(record, "destinationBinding", "destination_binding") ?? {};
  const routeId = readRequiredString(record, ["routeId", "route_id"], label);
  if (routeId !== ROUTE_ID) {
    throw new Error(`${label} routeId must be ${ROUTE_ID}.`);
  }
  const assetKey = readRequiredString(record, ["assetKey", "asset_key"], label);
  if (assetKey !== ASSET_KEY) {
    throw new Error(`${label} assetKey must be ${ASSET_KEY}.`);
  }
  const profile = normalizeBscNetworkProfile(
    readFirstString(record, "chain") ||
      readFirstString(record, "bscNetwork", "bsc_network", "network") ||
      "testnet",
  );
  const networkIdHex = readConsistentNormalizedString(
    [
      {
        record,
        keys: ["networkIdHex", "network_id_hex"],
        pathName: label,
      },
      {
        record: destinationRollout,
        keys: ["destinationNetworkId", "destination_network_id"],
        pathName: `${label} destinationRollout`,
      },
      {
        record: destinationBinding,
        keys: ["networkIdHex", "network_id_hex"],
        pathName: `${label} destinationBinding`,
      },
    ],
    `${label} networkIdHex`,
    (value, fieldLabel) => normalizeHex32(value, fieldLabel),
  );
  if (networkIdHex && networkIdHex !== profile.networkIdHex) {
    throw new Error(`${label} networkIdHex must be ${profile.label}.`);
  }
  const verifierKeyHashSources = [
    {
      record,
      keys: ["verifierKeyHash", "verifier_key_hash"],
      pathName: label,
    },
    {
      record: destinationRollout,
      keys: ["verifierKeyHash", "verifier_key_hash"],
      pathName: `${label} destinationRollout`,
    },
  ];
  assertSingleStringAliasPerSource(
    verifierKeyHashSources,
    `${label} verifierKeyHash`,
  );
  const verifierKeyHash = readRequiredConsistentNormalizedString(
    verifierKeyHashSources,
    `${label} verifierKeyHash`,
    (value, fieldLabel) => normalizeHex32(value, fieldLabel),
  );
  if (isKnownDiagnosticBscVerifierKeyHash(verifierKeyHash)) {
    throw new Error(
      `${label} verifierKeyHash is a known diagnostic BSC verifier key hash.`,
    );
  }
  const destinationBindingHashSources = [
    {
      record,
      keys: ["destinationBindingHash", "destination_binding_hash"],
      pathName: label,
    },
    {
      record: destinationRollout,
      keys: ["destinationBindingHash", "destination_binding_hash"],
      pathName: `${label} destinationRollout`,
    },
    {
      record: destinationBinding,
      keys: ["bindingHash", "binding_hash"],
      pathName: `${label} destinationBinding`,
    },
  ];
  assertSingleStringAliasPerSource(
    destinationBindingHashSources,
    `${label} destinationBindingHash`,
  );
  const destinationBindingHash = readRequiredConsistentNormalizedString(
    destinationBindingHashSources,
    `${label} destinationBindingHash`,
    (value, fieldLabel) => normalizeHex32(value, fieldLabel),
  );
  const optionalRouteHash = (fieldLabel, keys) => {
    const sources = [
      { record, keys, pathName: label },
      {
        record: destinationRollout,
        keys,
        pathName: `${label} destinationRollout`,
      },
    ];
    assertSingleStringAliasPerSource(sources, `${label} ${fieldLabel}`);
    return (
      readConsistentNormalizedString(
        sources,
        `${label} ${fieldLabel}`,
        (value, hashLabel) => normalizeHex32(value, hashLabel),
      ) || null
    );
  };
  return {
    routeId,
    assetKey,
    bscNetwork: profile.key,
    chain: profile.chain,
    chainIdHex: profile.chainIdHex,
    networkIdHex: profile.networkIdHex,
    verifierKeyHash,
    destinationBindingHash,
    proofArtifactHash: optionalRouteHash("proofArtifactHash", [
      "proofArtifactHash",
      "proof_artifact_hash",
      "proverArtifactHash",
      "prover_artifact_hash",
      "circuitArtifactHash",
      "circuit_artifact_hash",
    ]),
    provingKeyHash: optionalRouteHash("provingKeyHash", [
      "provingKeyHash",
      "proving_key_hash",
    ]),
  };
}

async function readBscBundleRouteBinding(options) {
  const routeManifestPath =
    ownValue(options, "route-manifest") ??
    ownValue(options, "manifest") ??
    null;
  const evidencePath =
    ownValue(options, "evidence") ??
    ownValue(options, "deployment-evidence") ??
    null;
  if (routeManifestPath && evidencePath) {
    throw new Error(
      "native-prover-bundle accepts either --route-manifest or --deployment-evidence, not both.",
    );
  }
  if (!routeManifestPath && !evidencePath) {
    throw new Error(
      "native-prover-bundle requires --route-manifest or --deployment-evidence.",
    );
  }
  const label = routeManifestPath ? "route manifest" : "deployment evidence";
  const pathName = routeManifestPath ?? evidencePath;
  const record = await readJson(pathName, `BSC ${label}`);
  return {
    record,
    path: resolve(pathName),
    kind: routeManifestPath ? "route-manifest" : "deployment-evidence",
    binding: extractBscBundleRouteBinding(record, `BSC ${label}`),
  };
}

function sdkImplementationOptionName(sdk) {
  return `${sdk}-implementation`;
}

function buildNativeEvmProverBundleObject({
  routeBinding,
  proofArtifact,
  provingKey,
  verifierKey,
  parityFixture,
  selfTestFixture,
  sdkArtifacts,
  auditHashes,
}) {
  return {
    schema: SCCP_NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1,
    bundle_id: bscNativeProverBundleId(
      BSC_NETWORK_PROFILES[routeBinding.bscNetwork],
    ),
    domain: SCCP_DOMAIN_BSC,
    chain: routeBinding.chain,
    proof_backend: SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
    proof_artifact: proofArtifact.path,
    proof_artifact_hash: proofArtifact.sha256,
    proving_key: provingKey.path,
    proving_key_hash: provingKey.sha256,
    verifier_key: verifierKey.path,
    verifier_key_hash: routeBinding.verifierKeyHash,
    verifier_key_artifact_hash: verifierKey.sha256,
    destination_binding_hash: routeBinding.destinationBindingHash,
    no_wasm: true,
    remote_prover_required: false,
    browser_implementation: "pure-typescript",
    cross_sdk_fixture_parity_artifact: parityFixture.path,
    native_prover_self_test_artifact: selfTestFixture.path,
    native_sdk_artifacts: sdkArtifacts.map((artifact) => ({
      sdk: artifact.sdk,
      implementation: artifact.implementation,
      prover_artifact_hash: proofArtifact.sha256,
      proving_key_hash: provingKey.sha256,
      implementation_artifact: artifact.path,
      implementation_hash: artifact.sha256,
    })),
    audit_hashes: auditHashes,
  };
}

function attachNativeProverBundleToManifest(manifest, bundle) {
  const profile =
    bundle.bundle_id === SCCP_BSC_MAINNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1 ||
    bundle.chain === BSC_NETWORK_PROFILES.mainnet.chain
      ? BSC_NETWORK_PROFILES.mainnet
      : BSC_NETWORK_PROFILES.testnet;
  const normalizedBundle = validateBscNativeEvmProverBundleForProfile(
    bundle,
    profile,
  );
  const nativeEvmProverBundleHash =
    canonicalBscNativeEvmProverBundleHash(normalizedBundle);
  const destinationRollout = {
    ...(readFirstRecord(
      manifest,
      "destinationRollout",
      "destination_rollout",
    ) ?? {}),
    proofArtifactHash: bundle.proof_artifact_hash,
    provingKeyHash: bundle.proving_key_hash,
    nativeEvmProverBundleHash,
    nativeEvmProverBundle: bundle,
  };
  return {
    ...manifest,
    proofArtifactHash: bundle.proof_artifact_hash,
    provingKeyHash: bundle.proving_key_hash,
    nativeEvmProverBundleHash,
    nativeEvmProverBundle: bundle,
    destinationRollout,
  };
}

function bscProfileFromNativeEvmProverBundle(bundle) {
  return bundle?.bundle_id ===
    SCCP_BSC_MAINNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1 ||
    bundle?.chain === BSC_NETWORK_PROFILES.mainnet.chain
    ? BSC_NETWORK_PROFILES.mainnet
    : BSC_NETWORK_PROFILES.testnet;
}

export async function buildBscNativeEvmProverBundleFromArtifacts(options = {}) {
  const root = artifactRootPath(
    ownValue(options, "artifact-root") ??
      DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT,
  );
  const routeSource = await readBscBundleRouteBinding(options);
  const proofArtifact = await readArtifactUnderRoot(
    root,
    requiredOption(options, "proof-artifact", "proof artifact"),
    "proof artifact",
  );
  const provingKey = await readArtifactUnderRoot(
    root,
    requiredOption(options, "proving-key", "proving key"),
    "proving key",
  );
  const verifierKey = await readArtifactUnderRoot(
    root,
    requiredOption(options, "verifier-key", "verifier key"),
    "verifier key",
  );
  const parityFixture = await readArtifactUnderRoot(
    root,
    requiredOption(
      options,
      ["cross-sdk-parity", "cross-sdk-fixture-parity", "parity-fixture"],
      "cross-SDK parity artifact",
    ),
    "cross-SDK parity artifact",
  );
  const selfTestFixture = await readArtifactUnderRoot(
    root,
    requiredOption(
      options,
      ["native-prover-self-test", "self-test"],
      "native prover self-test artifact",
    ),
    "native prover self-test artifact",
  );
  const sdkArtifacts = [];
  for (const [sdk, implementation] of Object.entries(
    SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
  )) {
    const optionName = sdkImplementationOptionName(sdk);
    const artifact = await readArtifactUnderRoot(
      root,
      requiredOption(options, optionName, `${sdk} implementation artifact`),
      `${sdk} implementation artifact`,
    );
    sdkArtifacts.push({ ...artifact, sdk, implementation });
  }
  assertProductionProofMaterialShape(
    proofArtifact,
    "proof artifact",
    "proof-artifact",
  );
  assertProductionProofMaterialShape(provingKey, "proving key", "proving-key");
  const auditHashes = await readNativeProverAuditHashes(root, options, {
    parityFixture,
    selfTestFixture,
  });
  const binding = routeSource.binding;
  const profile = BSC_NETWORK_PROFILES[binding.bscNetwork];
  const verifierMaterial = parseBscVerifierKeyArtifact(verifierKey, profile);
  if (
    binding.proofArtifactHash &&
    binding.proofArtifactHash !== proofArtifact.sha256
  ) {
    throw new Error(
      "proof artifact hash does not match route/deployment evidence.",
    );
  }
  if (binding.provingKeyHash && binding.provingKeyHash !== provingKey.sha256) {
    throw new Error(
      "proving key hash does not match route/deployment evidence.",
    );
  }
  if (binding.verifierKeyHash !== verifierMaterial.expectedVerifierKeyHash) {
    throw new Error(
      "verifier key material hash does not match route/deployment evidence.",
    );
  }
  const bundle = buildNativeEvmProverBundleObject({
    routeBinding: binding,
    proofArtifact,
    provingKey,
    verifierKey,
    parityFixture,
    selfTestFixture,
    sdkArtifacts,
    auditHashes,
  });
  const descriptor = validateBscNativeEvmProverBundleForProfile(
    bundle,
    profile,
    {
      expectedDestinationBindingHash: binding.destinationBindingHash,
    },
  );
  parseBscNativeProverParityFixtureForProfile(
    parityFixture.bytes.toString("utf8"),
    descriptor,
    profile,
  );
  parseBscNativeProverSelfTestFixtureForProfile(
    selfTestFixture.bytes.toString("utf8"),
    descriptor,
    profile,
  );
  const bytesByPath = new Map([
    [proofArtifact.path, proofArtifact.bytes],
    [provingKey.path, provingKey.bytes],
    [verifierKey.path, verifierKey.bytes],
    [parityFixture.path, parityFixture.bytes],
    [selfTestFixture.path, selfTestFixture.bytes],
    ...sdkArtifacts.map((artifact) => [artifact.path, artifact.bytes]),
  ]);
  const verifiedSdks = [];
  for (const artifact of sdkArtifacts) {
    await verifyBscNativeEvmProverArtifactsFromBundleForProfile(
      {
        nativeProverBundle: descriptor,
        sdk: artifact.sdk,
        artifactResolver(pathName) {
          return bytesByPath.get(pathName);
        },
      },
      { expectedDestinationBindingHash: binding.destinationBindingHash },
      profile,
    );
    verifiedSdks.push(artifact.sdk);
  }
  return {
    bundle,
    descriptor,
    routeSource,
    artifactRoot: root,
    artifacts: {
      proofArtifact,
      provingKey,
      verifierKey,
      parityFixture,
      selfTestFixture,
      sdkArtifacts,
    },
    verifiedSdks,
    attachedRouteManifest:
      routeSource.kind === "route-manifest"
        ? attachNativeProverBundleToManifest(routeSource.record, bundle)
        : null,
  };
}

async function compileBscContracts({ writeOut = null } = {}) {
  const solc = requireOptionalPackage("solc");
  const sources = {};
  for (const [key, sourcePath] of Object.entries(CONTRACT_SOURCES)) {
    sources[key] = { content: await readText(sourcePath, `${key} source`) };
  }
  const input = {
    language: "Solidity",
    sources,
    settings: {
      optimizer: { enabled: true, runs: 200 },
      outputSelection: {
        "*": {
          "*": ["abi", "evm.bytecode.object", "evm.deployedBytecode.object"],
        },
      },
    },
  };
  const output = JSON.parse(solc.compile(JSON.stringify(input)));
  const errors = output.errors ?? [];
  const fatal = errors.filter((entry) => entry.severity === "error");
  if (fatal.length) {
    throw new Error(
      fatal.map((entry) => entry.formattedMessage ?? entry.message).join("\n"),
    );
  }
  const artifacts = {};
  for (const definition of CONTRACT_DEFINITIONS) {
    const contract = output.contracts?.[definition.file]?.[definition.contract];
    if (!contract?.evm?.bytecode?.object) {
      throw new Error(`Missing compiled artifact for ${definition.contract}.`);
    }
    artifacts[definition.key] = {
      file: definition.file,
      contractName: definition.contract,
      abi: contract.abi,
      bytecode: `0x${contract.evm.bytecode.object}`,
      deployedBytecode: `0x${contract.evm.deployedBytecode.object}`,
      bytecodeSha256: bytesToHex(
        sha256(
          hexToBytes(
            contract.evm.bytecode.object,
            `${definition.contract} bytecode`,
            null,
            { allowZero: false },
          ),
        ),
      ),
      deployedBytecodeSha256: bytesToHex(
        sha256(
          hexToBytes(
            contract.evm.deployedBytecode.object,
            `${definition.contract} deployed bytecode`,
            null,
            { allowZero: false },
          ),
        ),
      ),
    };
  }
  if (writeOut) {
    for (const [key, artifact] of Object.entries(artifacts)) {
      await writeJsonNoSecrets(resolve(writeOut, `${key}.json`), artifact);
    }
  }
  return {
    artifacts,
    warnings: errors.filter((entry) => entry.severity !== "error"),
  };
}

async function deployContract(ethers, signer, artifact, args) {
  const factory = new ethers.ContractFactory(
    artifact.abi,
    artifact.bytecode,
    signer,
  );
  const contract = await factory.deploy(...args);
  const receipt = await contract.deploymentTransaction().wait();
  return {
    contract,
    address: normalizeEvmAddress(await contract.getAddress()),
    txHash: receipt.hash,
  };
}

async function readCodePresent(provider, addresses) {
  const entries = await Promise.all(
    Object.entries(addresses).map(async ([key, address]) => [
      key,
      (await provider.getCode(address)) !== "0x",
    ]),
  );
  return Object.fromEntries(entries);
}

async function fetchReadback(
  ethers,
  provider,
  { tokenAddress, bridgeAddress, sourceBridgeAddress, verifierAddress },
) {
  const addresses = {
    token: normalizeEvmAddress(tokenAddress, "token address"),
    bridge: normalizeEvmAddress(bridgeAddress, "bridge address"),
    sourceBridge: normalizeEvmAddress(
      sourceBridgeAddress,
      "source bridge address",
    ),
    verifier: normalizeEvmAddress(verifierAddress, "verifier address"),
  };
  const token = new ethers.Contract(addresses.token, TOKEN_ABI, provider);
  const sourceBridge = new ethers.Contract(
    addresses.sourceBridge,
    SOURCE_BRIDGE_ABI,
    provider,
  );
  const verifier = new ethers.Contract(
    addresses.verifier,
    VERIFIER_ABI,
    provider,
  );
  const bridge = new ethers.Contract(
    addresses.bridge,
    ROUTE_BRIDGE_ABI,
    provider,
  );
  const [
    network,
    codePresent,
    tokenBridgeAddress,
    tokenBridgeLocked,
    sourceBridgeOwner,
    verifierKeyHash,
    bridgeDestinationBindingHash,
    bridgeVerifierAddress,
    bridgeVerifierCodeHash,
    bridgeVerifierKeyHash,
    bridgeNetworkId,
    bridgeSourceDomain,
    bridgeTargetDomain,
  ] = await Promise.all([
    provider.getNetwork(),
    readCodePresent(provider, addresses),
    token.bridge(),
    token.bridgeLocked(),
    sourceBridge.owner(),
    verifier.verifyingKeyHash(),
    bridge.destinationBindingHash(),
    bridge.verifier(),
    bridge.verifierCodeHash(),
    bridge.verifierKeyHash(),
    bridge.networkId(),
    bridge.expectedSourceDomain(),
    bridge.expectedTargetDomain(),
  ]);
  return {
    chainIdHex: `0x${network.chainId.toString(16)}`,
    codePresent,
    tokenBridgeAddress: normalizeEvmAddress(tokenBridgeAddress),
    tokenBridgeLocked,
    sourceBridgeOwner: normalizeEvmAddress(sourceBridgeOwner),
    verifierKeyHash: normalizeHex32(verifierKeyHash),
    bridgeDestinationBindingHash: normalizeHex32(bridgeDestinationBindingHash),
    bridgeVerifierAddress: normalizeEvmAddress(bridgeVerifierAddress),
    bridgeVerifierCodeHash: normalizeHex32(bridgeVerifierCodeHash),
    bridgeVerifierKeyHash: normalizeHex32(bridgeVerifierKeyHash),
    bridgeNetworkId: normalizeHex32(bridgeNetworkId),
    bridgeSourceDomain: Number(bridgeSourceDomain),
    bridgeTargetDomain: Number(bridgeTargetDomain),
  };
}

export function buildDeploymentEvidence(input = {}) {
  const tokenAddress = ownValue(input, "tokenAddress");
  const bridgeAddress = ownValue(input, "bridgeAddress");
  const sourceBridgeAddress = ownValue(input, "sourceBridgeAddress");
  const verifierAddress = ownValue(input, "verifierAddress");
  const verifierCodeHash = ownValue(input, "verifierCodeHash");
  const verifierKeyHash = ownValue(input, "verifierKeyHash");
  const readback = ownValue(input, "readback");
  const bscNetwork = ownValue(input, "bscNetwork") ?? "testnet";
  const profile = normalizeBscNetworkProfile(bscNetwork);
  const addresses = {
    token: normalizeEvmAddress(tokenAddress, "token address"),
    bridge: normalizeEvmAddress(bridgeAddress, "bridge address"),
    sourceBridge: normalizeEvmAddress(
      sourceBridgeAddress,
      "source bridge address",
    ),
    verifier: normalizeEvmAddress(verifierAddress, "verifier address"),
  };
  if (
    new Set(Object.values(addresses)).size !== Object.keys(addresses).length
  ) {
    throw new Error(
      "BSC deployment token, bridge, source bridge, and verifier addresses must be distinct.",
    );
  }
  const codeHash = normalizeHex32(verifierCodeHash, "verifierCodeHash");
  const keyHash = normalizeHex32(verifierKeyHash, "verifierKeyHash");
  const bindingHash = bscDestinationBindingHash({
    networkId: profile.networkIdHex,
    verifierAddress: addresses.verifier,
    bridgeAddress: addresses.bridge,
    verifierCodeHash: codeHash,
    verifierKeyHash: keyHash,
  });
  validateBscReadbackEvidence({
    addresses,
    readback,
    bindingHash,
    verifierCodeHash: codeHash,
    verifierKeyHash: keyHash,
    bscNetwork: profile.key,
  });
  const bindingKey = bscDestinationBindingKey({
    networkId: profile.networkIdHex,
    verifierAddress: addresses.verifier,
    bridgeAddress: addresses.bridge,
    verifierCodeHash: codeHash,
    verifierKeyHash: keyHash,
  });
  return {
    schema: DEPLOYMENT_EVIDENCE_SCHEMA,
    routeId: ROUTE_ID,
    assetKey: ASSET_KEY,
    bscNetwork: profile.key,
    chain: profile.chain,
    chainIdHex: profile.chainIdHex,
    networkIdHex: profile.networkIdHex,
    explorerUrl: profile.explorerUrl,
    explorerHost: profile.explorerHost,
    bscBridgeAddress: addresses.bridge,
    bscTokenAddress: addresses.token,
    sccpBscSourceBridgeAddress: addresses.sourceBridge,
    bscVerifierAddress: addresses.verifier,
    destinationRollout: {
      version: 1,
      destinationNetworkId: profile.networkIdHex,
      sourceDomain: SCCP_DOMAIN_SORA,
      targetDomain: SCCP_DOMAIN_BSC,
      verifierIdentity: addresses.verifier,
      verifierBackend: BSC_EVM_GROTH16_BACKEND,
      proofFamily: SCCP_PROOF_FAMILY_STARK_FRI,
      verifierCodeHash: codeHash,
      verifierKeyHash: keyHash,
      destinationBridgeAddress: addresses.bridge,
      destinationBindingHash: bindingHash,
      destinationBindingKey: bindingKey,
    },
    destinationBinding: {
      version: 1,
      sourceDomain: SCCP_DOMAIN_SORA,
      targetDomain: SCCP_DOMAIN_BSC,
      networkIdHex: profile.networkIdHex,
      key: bindingKey,
      bindingHash,
    },
    bscContractReadback: readback,
    postDeployChecklist: [
      "TairaXOR.bridge() equals bscBridgeAddress",
      "TairaXOR.bridgeLocked() is true",
      "SccpBscSourceBridge.owner() equals bscBridgeAddress",
      "TairaXorBscSccpBridge.destinationBindingHash() equals destinationRollout.destinationBindingHash",
      "TairaXorBscSccpBridge.verifier() equals bscVerifierAddress",
      "TairaXorBscSccpBridge verifier code/key hashes and domains match destinationRollout",
    ],
    disabledReason:
      "Deployment evidence is not production-ready until TAIRA route publication, live canary evidence, and TAIRA burn-record material are attached by the route manifest step.",
  };
}

function normalizeBscRouteEvidenceProfile(record, options = {}) {
  assertSingleStringAliasPerSource(
    [
      {
        record,
        keys: ["bscNetwork", "bsc_network", "network"],
        pathName: "BSC deployment evidence",
      },
    ],
    "BSC deployment evidence bscNetwork",
  );
  assertSingleStringAliasPerSource(
    [
      {
        record,
        keys: ["chainIdHex", "chain_id_hex"],
        pathName: "BSC deployment evidence",
      },
    ],
    "BSC deployment evidence chainIdHex",
  );
  assertSingleStringAliasPerSource(
    [
      {
        record,
        keys: ["networkIdHex", "network_id_hex"],
        pathName: "BSC deployment evidence",
      },
    ],
    "BSC deployment evidence networkIdHex",
  );
  const profile = bscNetworkProfileFromOptions({
    ...Object.fromEntries(ownRecordEntries(options)),
    "bsc-network":
      ownValue(options, "bsc-network") ??
      readFirstString(record, "bscNetwork", "bsc_network", "network") ??
      readFirstString(record, "chain"),
  });
  const routeId = readRequiredString(
    record,
    ["routeId", "route_id"],
    "BSC deployment evidence routeId",
  );
  if (routeId !== ROUTE_ID) {
    throw new Error(`BSC deployment evidence routeId must be ${ROUTE_ID}.`);
  }
  const assetKey = readRequiredString(
    record,
    ["assetKey", "asset_key"],
    "BSC deployment evidence assetKey",
  );
  if (assetKey !== ASSET_KEY) {
    throw new Error(`BSC deployment evidence assetKey must be ${ASSET_KEY}.`);
  }
  const chain = readFirstString(record, "chain");
  if (chain && chain !== profile.chain) {
    throw new Error(`BSC deployment evidence chain must be ${profile.chain}.`);
  }
  const chainIdHex = readFirstString(record, "chainIdHex", "chain_id_hex");
  if (chainIdHex && chainIdHex !== profile.chainIdHex) {
    throw new Error(
      `BSC deployment evidence chainIdHex must be ${profile.chainIdHex}.`,
    );
  }
  const networkIdHex = readFirstString(
    record,
    "networkIdHex",
    "network_id_hex",
  );
  if (networkIdHex && normalizeHex32(networkIdHex) !== profile.networkIdHex) {
    throw new Error(
      `BSC deployment evidence networkIdHex must be ${profile.networkIdHex}.`,
    );
  }
  return profile;
}

function normalizeBscDeploymentEvidenceForRouteManifest(record, options = {}) {
  if (!isRecord(record)) {
    throw new Error("BSC deployment evidence must be a JSON object.");
  }
  const reason = unsafeSecretReason(record, "BSC deployment evidence");
  if (reason) {
    throw new Error(reason);
  }
  const schema = readFirstString(record, "schema");
  if (schema && schema !== DEPLOYMENT_EVIDENCE_SCHEMA) {
    throw new Error(
      `BSC deployment evidence schema must be ${DEPLOYMENT_EVIDENCE_SCHEMA}.`,
    );
  }
  const profile = normalizeBscRouteEvidenceProfile(record, options);
  assertSingleRecordAlias(
    record,
    ["destinationRollout", "destination_rollout"],
    "BSC deployment evidence",
    "BSC deployment evidence destinationRollout",
  );
  assertSingleRecordAlias(
    record,
    ["destinationBinding", "destination_binding"],
    "BSC deployment evidence",
    "BSC deployment evidence destinationBinding",
  );
  const rollout =
    readFirstRecord(record, "destinationRollout", "destination_rollout") ?? {};
  const binding =
    readFirstRecord(record, "destinationBinding", "destination_binding") ?? {};
  const address = (label, keys, extraSources = []) => {
    const sources = [
      { record, keys, pathName: "BSC deployment evidence" },
      ...extraSources,
    ];
    assertSingleStringAliasPerSource(
      sources,
      `BSC deployment evidence ${label}`,
    );
    return readRequiredConsistentNormalizedString(
      sources,
      `BSC deployment evidence ${label}`,
      (value, fieldLabel) => normalizeCanonicalEvmAddress(value, fieldLabel),
    );
  };
  const addresses = {
    token: address("token address", [
      "bscTokenAddress",
      "bsc_token_address",
      "tairaXorTokenAddress",
      "taira_xor_token_address",
      "tokenAddress",
      "token_address",
    ]),
    bridge: address(
      "bridge address",
      [
        "bscBridgeAddress",
        "bsc_bridge_address",
        "tairaXorBridgeAddress",
        "taira_xor_bridge_address",
        "bridgeAddress",
        "bridge_address",
      ],
      [
        {
          record: rollout,
          keys: ["destinationBridgeAddress", "destination_bridge_address"],
          pathName: "BSC deployment evidence destinationRollout",
        },
      ],
    ),
    sourceBridge: address("source bridge address", [
      "sccpBscSourceBridgeAddress",
      "sccp_bsc_source_bridge_address",
      "bscSourceBridgeAddress",
      "bsc_source_bridge_address",
      "sourceBridgeAddress",
      "source_bridge_address",
    ]),
    verifier: address(
      "verifier address",
      [
        "bscVerifierAddress",
        "bsc_verifier_address",
        "destinationVerifierAddress",
        "destination_verifier_address",
        "verifierAddress",
        "verifier_address",
      ],
      [
        {
          record: rollout,
          keys: ["verifierIdentity", "verifier_identity"],
          pathName: "BSC deployment evidence destinationRollout",
        },
      ],
    ),
  };
  if (new Set(Object.values(addresses)).size !== 4) {
    throw new Error(
      "BSC deployment evidence token, bridge, source bridge, and verifier addresses must be distinct.",
    );
  }
  const verifierCodeHashSources = [
    {
      record,
      keys: ["verifierCodeHash", "verifier_code_hash"],
      pathName: "BSC deployment evidence",
    },
    {
      record: rollout,
      keys: ["verifierCodeHash", "verifier_code_hash"],
      pathName: "BSC deployment evidence destinationRollout",
    },
  ];
  assertSingleStringAliasPerSource(
    verifierCodeHashSources,
    "BSC deployment evidence verifierCodeHash",
  );
  const verifierCodeHash = readRequiredConsistentNormalizedString(
    verifierCodeHashSources,
    "BSC deployment evidence verifierCodeHash",
    (value, label) => normalizeCanonicalHex32(value, label),
  );
  const verifierKeyHashSources = [
    {
      record,
      keys: ["verifierKeyHash", "verifier_key_hash"],
      pathName: "BSC deployment evidence",
    },
    {
      record: rollout,
      keys: ["verifierKeyHash", "verifier_key_hash"],
      pathName: "BSC deployment evidence destinationRollout",
    },
  ];
  assertSingleStringAliasPerSource(
    verifierKeyHashSources,
    "BSC deployment evidence verifierKeyHash",
  );
  const verifierKeyHash = readRequiredConsistentNormalizedString(
    verifierKeyHashSources,
    "BSC deployment evidence verifierKeyHash",
    (value, label) => normalizeCanonicalHex32(value, label),
  );
  const destinationBindingKey = bscDestinationBindingKey({
    networkId: profile.networkIdHex,
    verifierAddress: addresses.verifier,
    bridgeAddress: addresses.bridge,
    verifierCodeHash,
    verifierKeyHash,
  });
  const destinationBindingHash = bscDestinationBindingHash({
    networkId: profile.networkIdHex,
    verifierAddress: addresses.verifier,
    bridgeAddress: addresses.bridge,
    verifierCodeHash,
    verifierKeyHash,
  });
  const destinationBindingHashSources = [
    {
      record,
      keys: ["destinationBindingHash", "destination_binding_hash"],
      pathName: "BSC deployment evidence",
    },
    {
      record: rollout,
      keys: ["destinationBindingHash", "destination_binding_hash"],
      pathName: "BSC deployment evidence destinationRollout",
    },
    {
      record: binding,
      keys: ["bindingHash", "binding_hash"],
      pathName: "BSC deployment evidence destinationBinding",
    },
  ];
  assertSingleStringAliasPerSource(
    destinationBindingHashSources,
    "BSC deployment evidence destinationBindingHash",
  );
  const declaredBindingHash = readConsistentNormalizedString(
    destinationBindingHashSources,
    "BSC deployment evidence destinationBindingHash",
    (value, label) => normalizeCanonicalHex32(value, label),
  );
  if (declaredBindingHash && declaredBindingHash !== destinationBindingHash) {
    throw new Error(
      "BSC deployment evidence destinationBindingHash does not match computed binding hash.",
    );
  }
  const destinationBindingKeySources = [
    {
      record,
      keys: ["destinationBindingKey", "destination_binding_key"],
      pathName: "BSC deployment evidence",
    },
    {
      record: rollout,
      keys: ["destinationBindingKey", "destination_binding_key"],
      pathName: "BSC deployment evidence destinationRollout",
    },
    {
      record: binding,
      keys: ["key", "destinationBindingKey", "destination_binding_key"],
      pathName: "BSC deployment evidence destinationBinding",
    },
  ];
  assertSingleStringAliasPerSource(
    destinationBindingKeySources,
    "BSC deployment evidence destinationBindingKey",
  );
  const declaredBindingKey = readConsistentNormalizedString(
    destinationBindingKeySources,
    "BSC deployment evidence destinationBindingKey",
    (value) =>
      normalizeNonEmptyText(
        value,
        "BSC deployment evidence destinationBindingKey",
      ),
  );
  if (declaredBindingKey && declaredBindingKey !== destinationBindingKey) {
    throw new Error(
      "BSC deployment evidence destinationBindingKey does not match computed binding key.",
    );
  }
  return {
    profile,
    rollout,
    binding,
    addresses,
    verifierCodeHash,
    verifierKeyHash,
    destinationBindingKey,
    destinationBindingHash,
  };
}

function normalizeBscTairaBurnRecordContract(contract, options = {}) {
  if (!isRecord(contract)) {
    throw new Error("TAIRA burn-record contract must be a JSON object.");
  }
  const reason = unsafeSecretReason(contract, "TAIRA burn-record contract");
  if (reason) {
    throw new Error(reason);
  }
  const schema = readRequiredString(
    contract,
    ["schema"],
    "TAIRA burn-record contract schema",
  );
  if (schema !== TAIRA_BURN_RECORD_CONTRACT_SCHEMA) {
    throw new Error(
      `TAIRA burn-record contract schema must be ${TAIRA_BURN_RECORD_CONTRACT_SCHEMA}.`,
    );
  }
  const routeId = readFirstString(contract, "routeId", "route_id");
  if (routeId && routeId !== ROUTE_ID) {
    throw new Error(`TAIRA burn-record contract routeId must be ${ROUTE_ID}.`);
  }
  const assetKey = readFirstString(contract, "assetKey", "asset_key");
  if (assetKey && assetKey !== ASSET_KEY) {
    throw new Error(
      `TAIRA burn-record contract assetKey must be ${ASSET_KEY}.`,
    );
  }
  const artifact = normalizeStrictBase64(
    readRequiredString(
      contract,
      [
        "contractArtifactB64",
        "contract_artifact_b64",
        "artifactB64",
        "artifact_b64",
      ],
      "TAIRA burn-record contract artifact",
    ),
    "TAIRA burn-record contract artifact",
  );
  if (
    artifact.bytes.length < TAIRA_BURN_RECORD_ARTIFACT_MIN_BYTES ||
    artifact.bytes.length > TAIRA_BURN_RECORD_ARTIFACT_MAX_BYTES
  ) {
    throw new Error(
      `TAIRA burn-record contract artifact must decode to ${TAIRA_BURN_RECORD_ARTIFACT_MIN_BYTES}-${TAIRA_BURN_RECORD_ARTIFACT_MAX_BYTES} bytes.`,
    );
  }
  if (optionEnabled(options, "production-ready", false)) {
    assertProductionBurnRecordArtifactShape(
      artifact.bytes,
      "TAIRA burn-record contract artifact",
    );
  }
  const artifactSha256 = bytesToHex(sha256(new Uint8Array(artifact.bytes)));
  const declaredArtifactSha256 = normalizeCanonicalHex32(
    readRequiredString(
      contract,
      ["artifactSha256", "artifact_sha256"],
      "TAIRA burn-record contract artifactSha256",
    ),
    "TAIRA burn-record contract artifactSha256",
  );
  if (declaredArtifactSha256 !== artifactSha256) {
    throw new Error(
      "TAIRA burn-record contract artifactSha256 does not match artifact bytes.",
    );
  }
  const vkRef =
    readFirstRecord(contract, "vkRef", "vk_ref") ??
    readFirstRecord(contract, "verifierKeyRef", "verifier_key_ref") ??
    {};
  const vkBackend = readRequiredConsistentNormalizedString(
    [
      {
        record: options,
        keys: ["vk-backend", "vkBackend"],
        pathName: "route-manifest options",
      },
      {
        record: vkRef,
        keys: ["backend"],
        pathName: "TAIRA burn-record contract vkRef",
      },
    ],
    "TAIRA burn-record contract vkRef.backend",
    (value, label) => normalizeVerifierKeyRefText(value, label),
  );
  const vkName = readRequiredConsistentNormalizedString(
    [
      {
        record: options,
        keys: ["vk-name", "vkName"],
        pathName: "route-manifest options",
      },
      {
        record: vkRef,
        keys: ["name"],
        pathName: "TAIRA burn-record contract vkRef",
      },
    ],
    "TAIRA burn-record contract vkRef.name",
    (value, label) => normalizeVerifierKeyRefText(value, label),
  );
  return {
    contractArtifactB64: artifact.text,
    artifactSha256,
    codeHash: normalizeCanonicalHex32(
      readRequiredString(
        contract,
        ["codeHash", "code_hash"],
        "TAIRA burn-record contract codeHash",
      ),
      "TAIRA burn-record contract codeHash",
    ),
    vkRef: {
      backend: vkBackend,
      name: vkName,
    },
  };
}

function readOptionalBscRouteHash(
  options,
  record,
  rollout,
  optionKeys,
  recordKeys,
  label,
) {
  const sources = [
    {
      record: options,
      keys: optionKeys,
      pathName: "route-manifest options",
    },
    { record, keys: recordKeys, pathName: "BSC deployment evidence" },
    {
      record: rollout,
      keys: recordKeys,
      pathName: "BSC deployment evidence destinationRollout",
    },
  ];
  assertSingleStringAliasPerSource(sources, label);
  return (
    readConsistentNormalizedString(sources, label, (value, fieldLabel) =>
      normalizeCanonicalHex32(value, fieldLabel),
    ) || null
  );
}

async function readBscRouteManifestNativeProverBundle(
  options,
  {
    profile,
    verifierKeyHash,
    proofArtifactHash,
    provingKeyHash,
    destinationBindingHash,
    productionReady,
  },
) {
  const bundlePath = optionValue(options, [
    "native-prover-bundle",
    "native-evm-prover-bundle",
    "bsc-native-prover-bundle",
    "bsc-native-evm-prover-bundle",
  ]);
  if (!bundlePath) {
    if (productionReady) {
      throw new Error(
        "production-ready BSC route manifests require --native-prover-bundle.",
      );
    }
    return null;
  }
  const bundle = await readJson(bundlePath, "BSC native EVM prover bundle");
  const reason = unsafeSecretReason(bundle, "BSC native EVM prover bundle");
  if (reason) {
    throw new Error(reason);
  }
  const normalized = validateBscNativeEvmProverBundleForProfile(
    bundle,
    profile,
    {
      expectedDestinationBindingHash: destinationBindingHash,
    },
  );
  if (normalized.verifierKeyHash !== verifierKeyHash) {
    throw new Error(
      "BSC native EVM prover bundle verifierKeyHash must match route verifierKeyHash.",
    );
  }
  if (proofArtifactHash && normalized.proofArtifactHash !== proofArtifactHash) {
    throw new Error(
      "BSC native EVM prover bundle proofArtifactHash must match route proofArtifactHash.",
    );
  }
  if (provingKeyHash && normalized.provingKeyHash !== provingKeyHash) {
    throw new Error(
      "BSC native EVM prover bundle provingKeyHash must match route provingKeyHash.",
    );
  }
  return {
    bundle: normalized,
    hash: canonicalBscNativeEvmProverBundleHash(normalized),
  };
}

function bscPostDeployRecordSources(evidence, liveEvidence) {
  const sources = [];
  assertSingleRecordAlias(
    liveEvidence,
    ["postDeployLiveEvidence", "post_deploy_live_evidence"],
    "BSC live evidence",
    "BSC live evidence postDeployLiveEvidence",
  );
  assertSingleRecordAlias(
    evidence,
    ["postDeployLiveEvidence", "post_deploy_live_evidence"],
    "BSC deployment evidence",
    "BSC deployment evidence postDeployLiveEvidence",
  );
  const liveRecord =
    readFirstRecord(
      liveEvidence,
      "postDeployLiveEvidence",
      "post_deploy_live_evidence",
    ) ?? (isRecord(liveEvidence) ? liveEvidence : null);
  const evidenceRecord =
    readFirstRecord(
      evidence,
      "postDeployLiveEvidence",
      "post_deploy_live_evidence",
    ) ?? null;
  if (liveRecord) {
    sources.push({
      record: liveRecord,
      pathName: "BSC live evidence postDeployLiveEvidence",
    });
  }
  if (evidenceRecord) {
    sources.push({
      record: evidenceRecord,
      pathName: "BSC deployment evidence postDeployLiveEvidence",
    });
  }
  return sources;
}

function hasBscPostDeployEvidence(evidence, liveEvidence, options = {}) {
  const optionKeys = [
    "full-toml-ready",
    "source-bridge-config-hash",
    "source-event-transaction-id",
    "source-event-explorer-url",
    "route-canary-evidence-hash",
    "route-canary-transaction-id",
    "route-canary-explorer-url",
    "offline-full-toml-sha256",
  ];
  return (
    bscPostDeployRecordSources(evidence, liveEvidence).length > 0 ||
    optionKeys.some((key) => {
      const value = ownValue(options, key);
      return value !== undefined && value !== "";
    })
  );
}

function readBscPostDeployString(
  options,
  sources,
  optionKeys,
  recordKeys,
  label,
  normalizeValue,
) {
  const allSources = [
    { record: options, keys: optionKeys, pathName: "route-manifest options" },
    ...sources.map((source) => ({
      ...source,
      keys: recordKeys,
    })),
  ];
  assertSingleStringAliasPerSource(allSources, label);
  return readConsistentNormalizedString(allSources, label, normalizeValue);
}

function normalizeBscPostDeployEvidence(
  evidence,
  liveEvidence,
  options = {},
  { profile, requireFullTomlReady = false } = {},
) {
  if (!hasBscPostDeployEvidence(evidence, liveEvidence, options)) {
    return null;
  }
  const sources = bscPostDeployRecordSources(evidence, liveEvidence);
  const booleanSources = sources.map((source) => ({
    ...source,
    keys: ["fullTomlReady", "full_toml_ready"],
  }));
  for (const source of booleanSources) {
    assertSingleValueAlias(
      source.record,
      source.keys,
      source.pathName,
      "postDeployLiveEvidence.fullTomlReady",
    );
  }
  let fullTomlReady =
    ownValue(options, "full-toml-ready") === undefined
      ? false
      : optionEnabled(options, "full-toml-ready", false);
  if (ownValue(options, "full-toml-ready") === undefined) {
    let selected;
    for (const source of booleanSources) {
      const value = readConsistentBoolean(
        source.record,
        source.keys,
        `${source.pathName}.fullTomlReady`,
      );
      if (
        selected === undefined &&
        hasAnyOwnManifestKey(source.record, source.keys)
      ) {
        selected = value;
      } else if (
        hasAnyOwnManifestKey(source.record, source.keys) &&
        selected !== value
      ) {
        throw new Error(
          "postDeployLiveEvidence.fullTomlReady sources disagree.",
        );
      }
    }
    fullTomlReady = selected === true;
  }
  const sourceBridgeConfigHash = readBscPostDeployString(
    options,
    sources,
    ["source-bridge-config-hash"],
    ["sourceBridgeConfigHash", "source_bridge_config_hash"],
    "postDeployLiveEvidence.sourceBridgeConfigHash",
    (value, label) => normalizeCanonicalHex32(value, label),
  );
  const sourceEventTransactionId = readBscPostDeployString(
    options,
    sources,
    ["source-event-transaction-id"],
    ["sourceEventTransactionId", "source_event_transaction_id"],
    "postDeployLiveEvidence.sourceEventTransactionId",
    (value, label) => normalizeCanonicalHex32(value, label),
  );
  const routeCanaryEvidenceHash = readBscPostDeployString(
    options,
    sources,
    ["route-canary-evidence-hash"],
    ["routeCanaryEvidenceHash", "route_canary_evidence_hash"],
    "postDeployLiveEvidence.routeCanaryEvidenceHash",
    (value, label) => normalizeCanonicalHex32(value, label),
  );
  const routeCanaryTransactionId = readBscPostDeployString(
    options,
    sources,
    ["route-canary-transaction-id"],
    ["routeCanaryTransactionId", "route_canary_transaction_id"],
    "postDeployLiveEvidence.routeCanaryTransactionId",
    (value, label) => normalizeCanonicalHex32(value, label),
  );
  const offlineFullTomlSha256 = readBscPostDeployString(
    options,
    sources,
    ["offline-full-toml-sha256"],
    ["offlineFullTomlSha256", "offline_full_toml_sha256"],
    "postDeployLiveEvidence.offlineFullTomlSha256",
    (value, label) => normalizeCanonicalHex32(value, label),
  );
  if (requireFullTomlReady && !fullTomlReady) {
    throw new Error("postDeployLiveEvidence.fullTomlReady must be true.");
  }
  if (fullTomlReady && !offlineFullTomlSha256) {
    throw new Error(
      "postDeployLiveEvidence.fullTomlReady requires postDeployLiveEvidence.offlineFullTomlSha256.",
    );
  }
  const sourceEventExplorerUrl = readBscPostDeployString(
    options,
    sources,
    ["source-event-explorer-url"],
    [
      "sourceEventExplorerUrl",
      "source_event_explorer_url",
      "sourceEventTransactionUrl",
      "source_event_transaction_url",
    ],
    "postDeployLiveEvidence.sourceEventExplorerUrl",
    (value, label) =>
      normalizeBscExplorerTxUrl(
        value,
        label,
        sourceEventTransactionId,
        profile,
      ),
  );
  const routeCanaryExplorerUrl = readBscPostDeployString(
    options,
    sources,
    ["route-canary-explorer-url"],
    [
      "routeCanaryExplorerUrl",
      "route_canary_explorer_url",
      "routeCanaryTransactionUrl",
      "route_canary_transaction_url",
    ],
    "postDeployLiveEvidence.routeCanaryExplorerUrl",
    (value, label) =>
      normalizeBscExplorerTxUrl(
        value,
        label,
        routeCanaryTransactionId,
        profile,
      ),
  );
  if (sourceBridgeConfigHash === routeCanaryEvidenceHash) {
    throw new Error(
      "postDeployLiveEvidence sourceBridgeConfigHash and routeCanaryEvidenceHash must be distinct.",
    );
  }
  if (sourceEventTransactionId === routeCanaryTransactionId) {
    throw new Error(
      "postDeployLiveEvidence sourceEventTransactionId and routeCanaryTransactionId must be distinct.",
    );
  }
  return {
    fullTomlReady,
    sourceBridgeConfigHash,
    sourceEventTransactionId,
    sourceEventExplorerUrl,
    routeCanaryEvidenceHash,
    routeCanaryTransactionId,
    routeCanaryExplorerUrl,
    offlineFullTomlSha256: offlineFullTomlSha256 || null,
  };
}

function normalizeBscOfflineFullTomlEvidence(record, profile) {
  if (!isRecord(record)) {
    throw new Error("BSC offline full TOML evidence must be a JSON object.");
  }
  const reason = unsafeSecretReason(record, "BSC offline full TOML evidence");
  if (reason) {
    throw new Error(reason);
  }
  const schema = readRequiredString(
    record,
    ["schema"],
    "BSC offline full TOML evidence schema",
  );
  if (schema !== OFFLINE_FULL_TOML_EVIDENCE_SCHEMA) {
    throw new Error(
      `BSC offline full TOML evidence schema must be ${OFFLINE_FULL_TOML_EVIDENCE_SCHEMA}.`,
    );
  }
  const routeId = readRequiredString(
    record,
    ["routeId", "route_id"],
    "BSC offline full TOML evidence routeId",
  );
  if (routeId !== ROUTE_ID) {
    throw new Error(
      `BSC offline full TOML evidence routeId must be ${ROUTE_ID}.`,
    );
  }
  const assetKey = readRequiredString(
    record,
    ["assetKey", "asset_key"],
    "BSC offline full TOML evidence assetKey",
  );
  if (assetKey !== ASSET_KEY) {
    throw new Error(
      `BSC offline full TOML evidence assetKey must be ${ASSET_KEY}.`,
    );
  }
  assertSingleStringAliasPerSource(
    [
      {
        record,
        keys: ["bscNetwork", "bsc_network", "network"],
        pathName: "BSC offline full TOML evidence",
      },
    ],
    "BSC offline full TOML evidence network",
  );
  const networkText = readFirstString(
    record,
    "bscNetwork",
    "bsc_network",
    "network",
  );
  const chainText = readFirstString(record, "chain");
  if (!networkText && !chainText) {
    throw new Error("BSC offline full TOML evidence network is required.");
  }
  const evidenceProfile = normalizeBscNetworkProfile(networkText || chainText);
  if (chainText) {
    if (chainText !== profile.chain) {
      throw new Error(
        `BSC offline full TOML evidence chain must be ${profile.chain}.`,
      );
    }
  }
  if (evidenceProfile.key !== profile.key) {
    throw new Error(
      "BSC offline full TOML evidence network must match deployment evidence network.",
    );
  }
  assertSingleRecordAlias(
    record,
    ["postDeployLiveEvidence", "post_deploy_live_evidence"],
    "BSC offline full TOML evidence",
    "BSC offline full TOML evidence postDeployLiveEvidence",
  );
  const postDeployLiveEvidence =
    readFirstRecord(
      record,
      "postDeployLiveEvidence",
      "post_deploy_live_evidence",
    ) ?? {};
  const fullTomlReadySources = [
    {
      record,
      keys: ["fullTomlReady", "full_toml_ready"],
      pathName: "BSC offline full TOML evidence",
    },
    {
      record: postDeployLiveEvidence,
      keys: ["fullTomlReady", "full_toml_ready"],
      pathName: "BSC offline full TOML evidence postDeployLiveEvidence",
    },
  ];
  for (const source of fullTomlReadySources) {
    assertSingleValueAlias(
      source.record,
      source.keys,
      source.pathName,
      "BSC offline full TOML evidence fullTomlReady",
    );
  }
  const fullTomlReady = readConsistentBoolean(
    record,
    ["fullTomlReady", "full_toml_ready"],
    "BSC offline full TOML evidence fullTomlReady",
  );
  const nestedFullTomlReady = readConsistentBoolean(
    postDeployLiveEvidence,
    ["fullTomlReady", "full_toml_ready"],
    "BSC offline full TOML evidence postDeployLiveEvidence.fullTomlReady",
  );
  if (fullTomlReady !== true || nestedFullTomlReady !== true) {
    throw new Error(
      "BSC offline full TOML evidence fullTomlReady must be true.",
    );
  }
  const offlineFullTomlSha256Sources = [
    {
      record,
      keys: ["offlineFullTomlSha256", "offline_full_toml_sha256"],
      pathName: "BSC offline full TOML evidence",
    },
    {
      record: postDeployLiveEvidence,
      keys: ["offlineFullTomlSha256", "offline_full_toml_sha256"],
      pathName: "BSC offline full TOML evidence postDeployLiveEvidence",
    },
  ];
  assertSingleStringAliasPerSource(
    offlineFullTomlSha256Sources,
    "BSC offline full TOML evidence offlineFullTomlSha256",
  );
  const offlineFullTomlSha256 = readRequiredConsistentNormalizedString(
    offlineFullTomlSha256Sources,
    "BSC offline full TOML evidence offlineFullTomlSha256",
    (value, label) => normalizeCanonicalHex32(value, label),
  );
  return {
    fullTomlReady: true,
    offlineFullTomlSha256,
  };
}

function mergeBscOfflineFullTomlEvidenceOptions(options, offlineEvidence) {
  if (!offlineEvidence) {
    return options;
  }
  const next = Object.fromEntries(ownRecordEntries(options));
  if (ownValue(options, "full-toml-ready") !== undefined) {
    const suppliedReady = optionEnabled(options, "full-toml-ready", false);
    if (!suppliedReady) {
      throw new Error(
        "--full-toml-ready disagrees with --offline-full-toml-evidence.",
      );
    }
  }
  next["full-toml-ready"] = "true";
  if (ownValue(options, "offline-full-toml-sha256") !== undefined) {
    const suppliedHash = normalizeCanonicalHex32(
      ownValue(options, "offline-full-toml-sha256"),
      "--offline-full-toml-sha256",
    );
    if (suppliedHash !== offlineEvidence.offlineFullTomlSha256) {
      throw new Error(
        "--offline-full-toml-sha256 disagrees with --offline-full-toml-evidence.",
      );
    }
  }
  next["offline-full-toml-sha256"] = offlineEvidence.offlineFullTomlSha256;
  return next;
}

export async function buildBscTairaXorRouteManifestDraft(input = {}) {
  const options = ownValue(input, "options") ?? {};
  const evidence = ownValue(input, "evidence");
  const tairaContract = ownValue(input, "tairaContract");
  const liveEvidence = ownValue(input, "liveEvidence") ?? null;
  const offlineFullTomlEvidence =
    ownValue(input, "offlineFullTomlEvidence") ?? null;
  const createdAt = ownValue(input, "createdAt") ?? new Date().toISOString();
  const routeEvidence = normalizeBscDeploymentEvidenceForRouteManifest(
    evidence,
    options,
  );
  const { profile, rollout, addresses, verifierCodeHash, verifierKeyHash } =
    routeEvidence;
  const productionReady = optionEnabled(options, "production-ready", false);
  if (productionReady) {
    const confirmed =
      profile.key === "testnet"
        ? ownValue(options, "confirm-testnet") === ROUTE_ID
        : optionEnabled(options, "confirm-mainnet", false) &&
          ownValue(options, "confirm-network") === ROUTE_ID;
    if (!confirmed) {
      throw new Error(
        profile.key === "testnet"
          ? `production-ready BSC testnet route manifests require --confirm-testnet ${ROUTE_ID}.`
          : `production-ready BSC mainnet route manifests require --confirm-mainnet true --confirm-network ${ROUTE_ID}.`,
      );
    }
    if (!optionEnabled(options, "live-readback-checked", false)) {
      throw new Error(
        "production-ready BSC route manifests require --live-readback-checked true.",
      );
    }
  }
  const burnRecord = normalizeBscTairaBurnRecordContract(
    tairaContract,
    options,
  );
  const nativeBundle = await readBscRouteManifestNativeProverBundle(options, {
    profile,
    verifierKeyHash,
    proofArtifactHash: null,
    provingKeyHash: null,
    destinationBindingHash: routeEvidence.destinationBindingHash,
    productionReady: false,
  });
  const proofArtifactHash =
    readOptionalBscRouteHash(
      options,
      evidence,
      rollout,
      ["proof-artifact-hash", "prover-artifact-hash", "circuit-artifact-hash"],
      [
        "proofArtifactHash",
        "proof_artifact_hash",
        "proverArtifactHash",
        "prover_artifact_hash",
        "circuitArtifactHash",
        "circuit_artifact_hash",
      ],
      "BSC route proofArtifactHash",
    ) ??
    nativeBundle?.bundle.proofArtifactHash ??
    null;
  const provingKeyHash =
    readOptionalBscRouteHash(
      options,
      evidence,
      rollout,
      ["proving-key-hash"],
      ["provingKeyHash", "proving_key_hash"],
      "BSC route provingKeyHash",
    ) ??
    nativeBundle?.bundle.provingKeyHash ??
    null;
  if (Boolean(proofArtifactHash) !== Boolean(provingKeyHash)) {
    throw new Error(
      "BSC route proofArtifactHash and provingKeyHash must be supplied together.",
    );
  }
  if (nativeBundle) {
    if (nativeBundle.bundle.proofArtifactHash !== proofArtifactHash) {
      throw new Error(
        "BSC native EVM prover bundle proofArtifactHash must match route proofArtifactHash.",
      );
    }
    if (nativeBundle.bundle.provingKeyHash !== provingKeyHash) {
      throw new Error(
        "BSC native EVM prover bundle provingKeyHash must match route provingKeyHash.",
      );
    }
  } else if (productionReady) {
    throw new Error(
      "production-ready BSC route manifests require --native-prover-bundle.",
    );
  }
  if (productionReady && (!proofArtifactHash || !provingKeyHash)) {
    throw new Error(
      "production-ready BSC route manifests require proofArtifactHash and provingKeyHash.",
    );
  }
  const normalizedOfflineFullTomlEvidence = offlineFullTomlEvidence
    ? normalizeBscOfflineFullTomlEvidence(offlineFullTomlEvidence, profile)
    : null;
  if (productionReady && !normalizedOfflineFullTomlEvidence) {
    throw new Error(
      "production-ready BSC route manifests require --offline-full-toml-evidence generated by route-config.",
    );
  }
  const routeOptions = mergeBscOfflineFullTomlEvidenceOptions(
    options,
    normalizedOfflineFullTomlEvidence,
  );
  const postDeployLiveEvidence = normalizeBscPostDeployEvidence(
    evidence,
    liveEvidence,
    routeOptions,
    { profile, requireFullTomlReady: productionReady },
  );
  if (productionReady && !postDeployLiveEvidence) {
    throw new Error(
      "production-ready BSC route manifests require postDeployLiveEvidence.",
    );
  }
  const diagnosticVerifierReasons = [
    diagnosticFlagReason(evidence, "BSC deployment evidence"),
    diagnosticFlagReason(rollout, "BSC deployment evidence destinationRollout"),
    isKnownDiagnosticBscVerifierKeyHash(verifierKeyHash)
      ? `verifierKeyHash=${verifierKeyHash} is a known diagnostic BSC verifier key hash`
      : "",
  ].filter(Boolean);
  const nativeEvmProverBundleHash = nativeBundle?.hash ?? null;
  const manifest = {
    schema: ROUTE_MANIFEST_SCHEMA,
    createdAt,
    routeId: ROUTE_ID,
    assetKey: ASSET_KEY,
    bscNetwork: profile.key,
    chain: profile.chain,
    chainIdHex: profile.chainIdHex,
    networkIdHex: profile.networkIdHex,
    explorerUrl: profile.explorerUrl,
    explorerHost: profile.explorerHost,
    counterpartyDomain: SCCP_DOMAIN_BSC,
    counterpartyAccountCodecKey: "evm_hex",
    counterpartyAccountCodec: 2,
    verifierTarget: "EvmContract",
    productionReady,
    ...(diagnosticVerifierReasons.length > 0
      ? {
          diagnosticVerifier: true,
          verifierMaterialWarnings: diagnosticVerifierReasons,
        }
      : {}),
    ...(productionReady
      ? { postDeployReadbackChecked: true }
      : {
          disabledReason:
            diagnosticVerifierReasons.length > 0
              ? "BSC verifier material is diagnostic and must be replaced before production readiness."
              : "Route manifest draft is not production-ready until BSC contract readback, native prover, TAIRA route publication, and live canary evidence are complete.",
        }),
    bscTokenAddress: addresses.token,
    bscBridgeAddress: addresses.bridge,
    sccpBscSourceBridgeAddress: addresses.sourceBridge,
    bscVerifierAddress: addresses.verifier,
    ...(proofArtifactHash ? { proofArtifactHash } : {}),
    ...(provingKeyHash ? { provingKeyHash } : {}),
    ...(nativeEvmProverBundleHash ? { nativeEvmProverBundleHash } : {}),
    ...(nativeBundle ? { nativeEvmProverBundle: nativeBundle.bundle } : {}),
    destinationRollout: {
      version: 1,
      destinationNetworkId: profile.networkIdHex,
      sourceDomain: SCCP_DOMAIN_SORA,
      targetDomain: SCCP_DOMAIN_BSC,
      verifierIdentity: addresses.verifier,
      verifierBackend: BSC_EVM_GROTH16_BACKEND,
      proofFamily: SCCP_PROOF_FAMILY_STARK_FRI,
      verifierCodeHash,
      verifierKeyHash,
      ...(proofArtifactHash ? { proofArtifactHash } : {}),
      ...(provingKeyHash ? { provingKeyHash } : {}),
      ...(nativeEvmProverBundleHash ? { nativeEvmProverBundleHash } : {}),
      ...(nativeBundle ? { nativeEvmProverBundle: nativeBundle.bundle } : {}),
      destinationBridgeAddress: addresses.bridge,
      destinationBindingHash: routeEvidence.destinationBindingHash,
      destinationBindingKey: routeEvidence.destinationBindingKey,
    },
    destinationBinding: {
      version: 1,
      key: routeEvidence.destinationBindingKey,
      sourceDomain: SCCP_DOMAIN_SORA,
      targetDomain: SCCP_DOMAIN_BSC,
      bindingHash: routeEvidence.destinationBindingHash,
      networkIdHex: profile.networkIdHex,
    },
    tairaXorBurnRecord: {
      settlementAssetDefinitionId: normalizeCanonicalAssetDefinitionId(
        readRequiredConsistentNormalizedString(
          [
            {
              record: options,
              keys: [
                "settlement-asset-definition-id",
                "settlementAssetDefinitionId",
              ],
              pathName: "route-manifest options",
            },
            {
              record: evidence,
              keys: [
                "settlementAssetDefinitionId",
                "settlement_asset_definition_id",
              ],
              pathName: "BSC deployment evidence",
            },
          ],
          "--settlement-asset-definition-id",
          (value, label) => normalizeNonEmptyText(value, label),
        ),
        "--settlement-asset-definition-id",
      ),
      contractArtifactB64: burnRecord.contractArtifactB64,
      artifactSha256: burnRecord.artifactSha256,
      codeHash: burnRecord.codeHash,
      vkRef: burnRecord.vkRef,
      gasLimit: normalizePositiveSafeInteger(
        ownValue(options, "gas-limit"),
        "--gas-limit",
        2_000_000,
      ),
    },
    settlement: {
      submitPath: "/v1/bridge/messages",
      mode: "finalize_inbound",
      routeId: ROUTE_ID,
      assetKey: ASSET_KEY,
    },
    ...(postDeployLiveEvidence ? { postDeployLiveEvidence } : {}),
  };
  normalizeRouteManifestForConfig(manifest);
  return manifest;
}

function normalizeVerifierKeyRefText(value, label) {
  const normalized = normalizeNonEmptyText(value, label);
  if (!/^[A-Za-z0-9][A-Za-z0-9._:/-]{0,127}$/u.test(normalized)) {
    throw new Error(`${label} contains unsupported characters.`);
  }
  return normalized;
}

function normalizeBscTestnetKey(value, label) {
  try {
    return normalizeBscNetworkProfile(value).key;
  } catch (error) {
    throw new Error(
      `${label} must be BSC testnet or BSC mainnet: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
}

function readRequiredString(record, keys, label) {
  const entries = collectStringEntries(record, keys, label);
  if (entries.length > 1) {
    throw new Error(
      `${label} must not use multiple aliases: ${entries
        .map((entry) => entry.key)
        .join(", ")}.`,
    );
  }
  const value = readFirstString(record, ...keys);
  if (!value) {
    throw new Error(`${label} is required.`);
  }
  return value;
}

function readConsistentString(record, keys, label) {
  if (!isRecord(record)) {
    return "";
  }
  let selected = "";
  let selectedKey = "";
  for (const key of keys) {
    if (!hasOwn(record, key)) {
      continue;
    }
    const value = canonicalRecordString(
      ownValue(record, key),
      `${label}.${key}`,
    );
    if (!value) {
      continue;
    }
    const normalized = value;
    if (!selected) {
      selected = normalized;
      selectedKey = key;
      continue;
    }
    if (selected !== normalized) {
      throw new Error(
        `${label} aliases disagree: ${selectedKey}=${selected} but ${key}=${normalized}.`,
      );
    }
  }
  return selected;
}

function collectStringEntries(record, keys, pathName) {
  if (!isRecord(record)) {
    return [];
  }
  const entries = [];
  for (const key of keys) {
    if (!hasOwn(record, key)) {
      continue;
    }
    const value = canonicalRecordString(
      ownValue(record, key),
      `${pathName}.${key}`,
    );
    if (value) {
      entries.push({
        key,
        path: `${pathName}.${key}`,
        value,
      });
    }
  }
  return entries;
}

function collectRecordEntries(record, keys, pathName) {
  if (!isRecord(record)) {
    return [];
  }
  const entries = [];
  for (const key of keys) {
    if (!hasOwn(record, key)) {
      continue;
    }
    const value = ownValue(record, key);
    if (isRecord(value)) {
      entries.push({
        key,
        path: `${pathName}.${key}`,
        value,
      });
    }
  }
  return entries;
}

function readConsistentNormalizedString(sources, label, normalizeValue) {
  let selected = null;
  for (const source of sources) {
    for (const entry of collectStringEntries(
      source.record,
      source.keys,
      source.pathName,
    )) {
      const normalized = normalizeValue(entry.value, label);
      if (!selected) {
        selected = { ...entry, normalized };
        continue;
      }
      if (selected.normalized !== normalized) {
        throw new Error(
          `${label} aliases disagree: ${selected.path}=${selected.value} but ${entry.path}=${entry.value}.`,
        );
      }
    }
  }
  return selected?.normalized ?? "";
}

function assertSingleStringAliasPerSource(sources, label) {
  for (const source of sources) {
    const entries = collectStringEntries(
      source.record,
      source.keys,
      source.pathName,
    );
    if (entries.length > 1) {
      throw new Error(
        `${label} must not use multiple aliases in ${source.pathName}: ${entries
          .map((entry) => entry.key)
          .join(", ")}.`,
      );
    }
  }
}

function assertSingleRecordAlias(record, keys, pathName, label) {
  const entries = collectRecordEntries(record, keys, pathName);
  if (entries.length > 1) {
    throw new Error(
      `${label} must not use multiple aliases in ${pathName}: ${entries
        .map((entry) => entry.key)
        .join(", ")}.`,
    );
  }
}

function assertNoForbiddenStringAliases(record, keys, pathName, label) {
  const entries = collectStringEntries(record, keys, pathName);
  if (entries.length > 0) {
    throw new Error(
      `${label} must not use TRON aliases on a BSC route manifest: ${entries
        .map((entry) => entry.key)
        .join(", ")}.`,
    );
  }
}

function assertSingleValueAlias(record, keys, pathName, label) {
  if (!isRecord(record)) {
    return;
  }
  const presentKeys = keys.filter((key) => {
    if (!hasOwn(record, key)) {
      return false;
    }
    const value = ownValue(record, key);
    return (
      (typeof value === "string" && value.trim()) ||
      typeof value === "boolean" ||
      typeof value === "number"
    );
  });
  if (presentKeys.length > 1) {
    throw new Error(
      `${label} must not use multiple aliases in ${pathName}: ${presentKeys.join(", ")}.`,
    );
  }
}

function readRequiredConsistentNormalizedString(
  sources,
  label,
  normalizeValue,
) {
  const value = readConsistentNormalizedString(sources, label, normalizeValue);
  if (!value) {
    throw new Error(`${label} is required.`);
  }
  return value;
}

function readRequiredConsistentString(record, keys, label) {
  const value = readConsistentString(record, keys, label);
  if (!value) {
    throw new Error(`${label} is required.`);
  }
  return value;
}

function readConsistentBoolean(record, keys, label) {
  if (!isRecord(record)) {
    return false;
  }
  let selected;
  let selectedKey = "";
  for (const key of keys) {
    if (!hasOwn(record, key)) {
      continue;
    }
    const value = ownValue(record, key);
    if (typeof value !== "boolean") {
      throw new Error(`${label}.${key} must be boolean.`);
    }
    if (selected === undefined) {
      selected = value;
      selectedKey = key;
      continue;
    }
    if (selected !== value) {
      throw new Error(
        `${label} aliases disagree: ${selectedKey}=${selected} but ${key}=${value}.`,
      );
    }
  }
  return selected === true;
}

function routeConfigRequiredRecord(value, label) {
  if (!isRecord(value)) {
    throw new Error(`${label} must be an object.`);
  }
  return value;
}

function normalizeBscRouteNativeEvmProverBundle({
  record,
  destinationRollout,
  productionReady,
  verifierKeyHash,
  proofArtifactHash,
  provingKeyHash,
  destinationBindingHash,
  bscProfile = BSC_NETWORK_PROFILES.testnet,
}) {
  const entries = [
    ...collectRecordEntries(
      record,
      NATIVE_EVM_PROVER_BUNDLE_KEYS,
      "route manifest",
    ),
    ...collectRecordEntries(
      destinationRollout,
      NATIVE_EVM_PROVER_BUNDLE_KEYS,
      "route manifest destinationRollout",
    ),
  ];
  if (entries.length === 0) {
    if (productionReady) {
      throw new Error(
        "route manifest productionReady requires nativeEvmProverBundle.",
      );
    }
    return null;
  }

  let selected = null;
  let selectedJson = "";
  for (const entry of entries) {
    const rawVerifierKeyArtifactHash =
      requireExplicitBscNativeEvmVerifierKeyArtifactHash(
        entry.value,
        entry.path,
      );
    let normalized;
    try {
      normalized = validateBscNativeEvmProverBundleForProfile(
        entry.value,
        bscProfile,
        {
          expectedDestinationBindingHash: destinationBindingHash,
        },
      );
    } catch (error) {
      throw new Error(
        `${entry.path} failed BSC SDK validation: ${error instanceof Error ? error.message : String(error)}.`,
      );
    }
    if (rawVerifierKeyArtifactHash === normalized.verifierKeyHash) {
      throw new Error(
        `${entry.path} verifierKeyArtifactHash must be role-separated from verifierKeyHash.`,
      );
    }
    const roleSeparatedHashProblems =
      bscNativeEvmProverBundleRoleSeparatedHashProblems(
        entry.value,
        entry.path,
        {
          verifierKeyHash: normalized.verifierKeyHash,
          verifierKeyArtifactHash: rawVerifierKeyArtifactHash,
          proofArtifactHash: normalized.proofArtifactHash,
          provingKeyHash: normalized.provingKeyHash,
          nativeEvmProverBundleHash:
            canonicalBscNativeEvmProverBundleHash(normalized),
          destinationBindingHash: normalized.destinationBindingHash,
        },
      );
    if (roleSeparatedHashProblems.length > 0) {
      throw new Error(roleSeparatedHashProblems.join("; "));
    }
    if (normalized.verifierKeyHash !== verifierKeyHash) {
      throw new Error(
        `${entry.path} verifierKeyHash must match route manifest verifierKeyHash.`,
      );
    }
    if (
      proofArtifactHash &&
      normalized.proofArtifactHash !== proofArtifactHash
    ) {
      throw new Error(
        `${entry.path} proofArtifactHash must match route manifest proofArtifactHash.`,
      );
    }
    if (provingKeyHash && normalized.provingKeyHash !== provingKeyHash) {
      throw new Error(
        `${entry.path} provingKeyHash must match route manifest provingKeyHash.`,
      );
    }

    const normalizedJson = JSON.stringify(normalized);
    if (selected && selectedJson !== normalizedJson) {
      throw new Error(
        `route manifest nativeEvmProverBundle aliases disagree: ${selected.path} does not match ${entry.path}.`,
      );
    }
    selected = { path: entry.path, value: normalized };
    selectedJson = normalizedJson;
  }

  return selected.value;
}

function normalizeRouteManifestForConfig(manifest) {
  const record = routeConfigRequiredRecord(manifest, "route manifest");
  if (readFirstString(record, "schema") !== ROUTE_MANIFEST_SCHEMA) {
    throw new Error(`route manifest schema must be ${ROUTE_MANIFEST_SCHEMA}.`);
  }
  const reason = unsafeSecretReason(record, "route manifest");
  if (reason) {
    throw new Error(reason);
  }

  assertSingleRecordAlias(
    record,
    ["destinationRollout", "destination_rollout"],
    "route manifest",
    "route manifest destinationRollout",
  );
  assertSingleRecordAlias(
    record,
    ["destinationBinding", "destination_binding"],
    "route manifest",
    "route manifest destinationBinding",
  );
  assertSingleRecordAlias(
    record,
    ["tairaXorBurnRecord", "taira_xor_burn_record"],
    "route manifest",
    "route manifest tairaXorBurnRecord",
  );
  const destinationRollout = routeConfigRequiredRecord(
    readFirstRecord(record, "destinationRollout", "destination_rollout"),
    "route manifest destinationRollout",
  );
  const destinationBinding =
    readFirstRecord(record, "destinationBinding", "destination_binding") ?? {};
  const burnRecord = routeConfigRequiredRecord(
    readFirstRecord(record, "tairaXorBurnRecord", "taira_xor_burn_record"),
    "route manifest tairaXorBurnRecord",
  );
  assertSingleRecordAlias(
    burnRecord,
    ["vkRef", "vk_ref"],
    "route manifest tairaXorBurnRecord",
    "route manifest tairaXorBurnRecord.vkRef",
  );
  const vkRef = routeConfigRequiredRecord(
    readFirstRecord(burnRecord, "vkRef", "vk_ref"),
    "route manifest tairaXorBurnRecord.vkRef",
  );
  const settlement = routeConfigRequiredRecord(
    readFirstRecord(record, "settlement"),
    "route manifest settlement",
  );

  const routeId = readRequiredString(
    record,
    ["routeId", "route_id"],
    "route manifest routeId",
  );
  if (routeId !== ROUTE_ID) {
    throw new Error(`route manifest routeId must be ${ROUTE_ID}.`);
  }
  const assetKey = readRequiredString(
    record,
    ["assetKey", "asset_key"],
    "route manifest assetKey",
  );
  if (assetKey !== ASSET_KEY) {
    throw new Error(`route manifest assetKey must be ${ASSET_KEY}.`);
  }

  assertSingleStringAliasPerSource(
    [
      {
        record,
        keys: ["bscNetwork", "bsc_network", "network"],
        pathName: "route manifest",
      },
    ],
    "route manifest bscNetwork",
  );
  const bscNetworkText =
    readFirstString(record, "bscNetwork", "bsc_network", "network") ||
    readFirstString(record, "chain") ||
    "testnet";
  if (
    bscNetworkText !== bscNetworkText.toLowerCase() ||
    bscNetworkText.includes("_")
  ) {
    throw new Error(
      "route manifest bscNetwork must be canonical lowercase text.",
    );
  }
  const bscNetwork = normalizeBscTestnetKey(
    bscNetworkText,
    "route manifest bscNetwork",
  );
  const bscProfile = BSC_NETWORK_PROFILES[bscNetwork];
  const chain = readRequiredString(record, ["chain"], "route manifest chain");
  if (chain !== chain.toLowerCase()) {
    throw new Error("route manifest chain must be canonical lowercase text.");
  }
  if (chain !== bscProfile.chain) {
    throw new Error(`route manifest chain must be ${bscProfile.chain}.`);
  }
  const chainIdHex = readRequiredString(
    record,
    ["chainIdHex", "chain_id_hex"],
    "route manifest chainIdHex",
  );
  if (
    /^0X/u.test(chainIdHex) ||
    /[A-F]/u.test(chainIdHex.replace(/^0x/u, ""))
  ) {
    throw new Error(
      "route manifest chainIdHex must be canonical lowercase hex.",
    );
  }
  if (chainIdHex !== bscProfile.chainIdHex) {
    throw new Error(
      `route manifest chainIdHex must be ${bscProfile.label} ${bscProfile.chainIdHex}.`,
    );
  }
  const networkIdHex = readRequiredConsistentNormalizedString(
    [
      {
        record,
        keys: ["networkIdHex", "network_id_hex"],
        pathName: "route manifest",
      },
      {
        record: destinationRollout,
        keys: ["destinationNetworkId", "destination_network_id"],
        pathName: "route manifest destinationRollout",
      },
      {
        record: destinationBinding,
        keys: ["networkIdHex", "network_id_hex"],
        pathName: "route manifest destinationBinding",
      },
    ],
    "route manifest networkIdHex",
    (value, label) => normalizeCanonicalHex32(value, label),
  );
  if (networkIdHex !== bscProfile.networkIdHex) {
    throw new Error(`route manifest networkIdHex must be ${bscProfile.label}.`);
  }

  assertSingleValueAlias(
    record,
    ["counterpartyDomain", "counterparty_domain"],
    "route manifest",
    "route manifest counterpartyDomain",
  );
  const counterpartyDomain = normalizeUint32(
    readFirstValue(record, "counterpartyDomain", "counterparty_domain"),
    "route manifest counterpartyDomain",
  );
  if (counterpartyDomain !== SCCP_DOMAIN_BSC) {
    throw new Error("route manifest counterpartyDomain must be BSC domain 2.");
  }
  assertSingleValueAlias(
    destinationRollout,
    ["sourceDomain", "source_domain"],
    "route manifest destinationRollout",
    "route manifest sourceDomain",
  );
  assertSingleValueAlias(
    destinationBinding,
    ["sourceDomain", "source_domain"],
    "route manifest destinationBinding",
    "route manifest sourceDomain",
  );
  assertSingleValueAlias(
    destinationRollout,
    ["targetDomain", "target_domain"],
    "route manifest destinationRollout",
    "route manifest targetDomain",
  );
  assertSingleValueAlias(
    destinationBinding,
    ["targetDomain", "target_domain"],
    "route manifest destinationBinding",
    "route manifest targetDomain",
  );
  const rolloutSourceDomain = readFirstValue(
    destinationRollout,
    "sourceDomain",
    "source_domain",
  );
  const bindingSourceDomain = readFirstValue(
    destinationBinding,
    "sourceDomain",
    "source_domain",
  );
  const sourceDomain = normalizeUint32(
    rolloutSourceDomain ?? bindingSourceDomain ?? SCCP_DOMAIN_SORA,
    "route manifest sourceDomain",
  );
  if (
    rolloutSourceDomain !== undefined &&
    bindingSourceDomain !== undefined &&
    normalizeUint32(
      bindingSourceDomain,
      "route manifest destinationBinding.sourceDomain",
    ) !== sourceDomain
  ) {
    throw new Error(
      "route manifest sourceDomain aliases disagree between destinationRollout and destinationBinding.",
    );
  }
  const rolloutTargetDomain = readFirstValue(
    destinationRollout,
    "targetDomain",
    "target_domain",
  );
  const bindingTargetDomain = readFirstValue(
    destinationBinding,
    "targetDomain",
    "target_domain",
  );
  const targetDomain = normalizeUint32(
    rolloutTargetDomain ?? bindingTargetDomain ?? SCCP_DOMAIN_BSC,
    "route manifest targetDomain",
  );
  if (
    rolloutTargetDomain !== undefined &&
    bindingTargetDomain !== undefined &&
    normalizeUint32(
      bindingTargetDomain,
      "route manifest destinationBinding.targetDomain",
    ) !== targetDomain
  ) {
    throw new Error(
      "route manifest targetDomain aliases disagree between destinationRollout and destinationBinding.",
    );
  }
  if (sourceDomain !== SCCP_DOMAIN_SORA || targetDomain !== SCCP_DOMAIN_BSC) {
    throw new Error(
      "route manifest destination rollout domains must be SORA -> BSC.",
    );
  }

  const verifierTarget = readRequiredString(
    record,
    ["verifierTarget", "verifier_target"],
    "route manifest verifierTarget",
  );
  if (verifierTarget !== "EvmContract") {
    throw new Error("route manifest verifierTarget must be EvmContract.");
  }
  assertSingleStringAliasPerSource(
    [
      {
        record: destinationRollout,
        keys: ["verifierBackend", "verifier_backend"],
        pathName: "route manifest destinationRollout",
      },
    ],
    "route manifest verifierBackend",
  );
  const verifierBackend =
    readFirstString(
      destinationRollout,
      "verifierBackend",
      "verifier_backend",
    ) || BSC_EVM_GROTH16_BACKEND;
  if (verifierBackend !== BSC_EVM_GROTH16_BACKEND) {
    throw new Error(
      `route manifest verifier backend must be ${BSC_EVM_GROTH16_BACKEND}.`,
    );
  }
  assertSingleStringAliasPerSource(
    [
      {
        record: destinationRollout,
        keys: ["proofFamily", "proof_family"],
        pathName: "route manifest destinationRollout",
      },
    ],
    "route manifest proofFamily",
  );
  const proofFamily =
    readFirstString(destinationRollout, "proofFamily", "proof_family") ||
    SCCP_PROOF_FAMILY_STARK_FRI;
  if (proofFamily !== SCCP_PROOF_FAMILY_STARK_FRI) {
    throw new Error(
      `route manifest proof family must be ${SCCP_PROOF_FAMILY_STARK_FRI}.`,
    );
  }

  assertSingleValueAlias(
    record,
    ["productionReady", "production_ready"],
    "route manifest",
    "route manifest productionReady",
  );
  const productionReadyValue = readFirstValue(
    record,
    "productionReady",
    "production_ready",
  );
  if (productionReadyValue !== true && productionReadyValue !== false) {
    throw new Error("route manifest productionReady must be true or false.");
  }
  const productionReady = productionReadyValue === true;
  const explorerUrlSources = [
    {
      record,
      keys: [
        "explorerUrl",
        "explorer_url",
        "bscExplorerUrl",
        "bsc_explorer_url",
      ],
      pathName: "route manifest",
    },
  ];
  assertSingleStringAliasPerSource(
    explorerUrlSources,
    "route manifest BSC explorerUrl",
  );
  const declaredExplorerUrl = readConsistentNormalizedString(
    explorerUrlSources,
    "route manifest BSC explorerUrl",
    (value, label) => normalizeBscExplorerBaseUrl(value, label, bscProfile),
  );
  const explorerHostSources = [
    {
      record,
      keys: [
        "explorerHost",
        "explorer_host",
        "bscExplorerHost",
        "bsc_explorer_host",
      ],
      pathName: "route manifest",
    },
  ];
  assertSingleStringAliasPerSource(
    explorerHostSources,
    "route manifest BSC explorerHost",
  );
  const declaredExplorerHost = readConsistentNormalizedString(
    explorerHostSources,
    "route manifest BSC explorerHost",
    (value, label) => normalizeBscExplorerHost(value, label, bscProfile),
  );
  if (productionReady && !declaredExplorerUrl) {
    throw new Error("route manifest productionReady requires explorerUrl.");
  }
  if (productionReady && !declaredExplorerHost) {
    throw new Error("route manifest productionReady requires explorerHost.");
  }
  const explorerUrl = declaredExplorerUrl || bscProfile.explorerUrl;
  const explorerHost = declaredExplorerHost || bscProfile.explorerHost;
  const tokenAddressSources = [
    {
      record,
      keys: [
        "bscTokenAddress",
        "bsc_token_address",
        "tairaXorTokenAddress",
        "taira_xor_token_address",
        "tokenAddress",
        "token_address",
      ],
      pathName: "route manifest",
    },
  ];
  assertSingleStringAliasPerSource(
    tokenAddressSources,
    "route manifest BSC token address",
  );
  const tokenAddress = readRequiredConsistentNormalizedString(
    tokenAddressSources,
    "route manifest BSC token address",
    (value, label) => normalizeCanonicalEvmAddress(value, label),
  );
  const bridgeAddressSources = [
    {
      record,
      keys: [
        "bscBridgeAddress",
        "bsc_bridge_address",
        "tairaXorBridgeAddress",
        "taira_xor_bridge_address",
        "bridgeAddress",
        "bridge_address",
      ],
      pathName: "route manifest",
    },
    {
      record: destinationRollout,
      keys: ["destinationBridgeAddress", "destination_bridge_address"],
      pathName: "route manifest destinationRollout",
    },
  ];
  assertSingleStringAliasPerSource(
    bridgeAddressSources,
    "route manifest BSC bridge address",
  );
  const bridgeAddress = readRequiredConsistentNormalizedString(
    bridgeAddressSources,
    "route manifest BSC bridge address",
    (value, label) => normalizeCanonicalEvmAddress(value, label),
  );
  const sourceBridgeAddressSources = [
    {
      record,
      keys: [
        "sccpBscSourceBridgeAddress",
        "sccp_bsc_source_bridge_address",
        "bscSourceBridgeAddress",
        "bsc_source_bridge_address",
        "sourceBridgeAddress",
        "source_bridge_address",
      ],
      pathName: "route manifest",
    },
  ];
  assertNoForbiddenStringAliases(
    record,
    FORBIDDEN_BSC_ROUTE_MANIFEST_ADDRESS_ALIASES.sourceBridgeAddress,
    "route manifest",
    "route manifest BSC source bridge address",
  );
  assertSingleStringAliasPerSource(
    sourceBridgeAddressSources,
    "route manifest BSC source bridge address",
  );
  const sourceBridgeAddress = readRequiredConsistentNormalizedString(
    sourceBridgeAddressSources,
    "route manifest BSC source bridge address",
    (value, label) => normalizeCanonicalEvmAddress(value, label),
  );
  const verifierAddressSources = [
    {
      record,
      keys: [
        "destinationVerifierAddress",
        "destination_verifier_address",
        "verifierAddress",
        "verifier_address",
        "sccpBscDestinationVerifierAddress",
        "sccp_bsc_destination_verifier_address",
        "bscVerifierAddress",
        "bsc_verifier_address",
        "evmVerifierAddress",
        "evm_verifier_address",
      ],
      pathName: "route manifest",
    },
    {
      record: destinationRollout,
      keys: ["verifierIdentity", "verifier_identity"],
      pathName: "route manifest destinationRollout",
    },
  ];
  assertNoForbiddenStringAliases(
    record,
    FORBIDDEN_BSC_ROUTE_MANIFEST_ADDRESS_ALIASES.verifierAddress,
    "route manifest",
    "route manifest BSC verifier address",
  );
  assertSingleStringAliasPerSource(
    verifierAddressSources,
    "route manifest BSC verifier address",
  );
  const verifierAddress = readRequiredConsistentNormalizedString(
    verifierAddressSources,
    "route manifest BSC verifier address",
    (value, label) => normalizeCanonicalEvmAddress(value, label),
  );
  if (
    new Set([tokenAddress, bridgeAddress, sourceBridgeAddress, verifierAddress])
      .size !== 4
  ) {
    throw new Error(
      "route manifest BSC token, bridge, source bridge, and verifier addresses must be distinct.",
    );
  }
  const verifierCodeHashSources = [
    {
      record,
      keys: ["verifierCodeHash", "verifier_code_hash"],
      pathName: "route manifest",
    },
    {
      record: destinationRollout,
      keys: ["verifierCodeHash", "verifier_code_hash"],
      pathName: "route manifest destinationRollout",
    },
  ];
  assertSingleStringAliasPerSource(
    verifierCodeHashSources,
    "route manifest verifierCodeHash",
  );
  const verifierCodeHash = readRequiredConsistentNormalizedString(
    verifierCodeHashSources,
    "route manifest verifierCodeHash",
    (value, label) => normalizeCanonicalHex32(value, label),
  );
  const verifierKeyHashSources = [
    {
      record,
      keys: ["verifierKeyHash", "verifier_key_hash"],
      pathName: "route manifest",
    },
    {
      record: destinationRollout,
      keys: ["verifierKeyHash", "verifier_key_hash"],
      pathName: "route manifest destinationRollout",
    },
  ];
  assertSingleStringAliasPerSource(
    verifierKeyHashSources,
    "route manifest verifierKeyHash",
  );
  const verifierKeyHash = readRequiredConsistentNormalizedString(
    verifierKeyHashSources,
    "route manifest verifierKeyHash",
    (value, label) => normalizeCanonicalHex32(value, label),
  );
  const optionalRouteHash = (label, keys) => {
    const sources = [
      { record, keys, pathName: "route manifest" },
      {
        record: destinationRollout,
        keys,
        pathName: "route manifest destinationRollout",
      },
    ];
    assertSingleStringAliasPerSource(sources, label);
    return (
      readConsistentNormalizedString(sources, label, (value, fieldLabel) =>
        normalizeCanonicalHex32(value, fieldLabel),
      ) || null
    );
  };
  const proofArtifactHash = optionalRouteHash(
    "route manifest proofArtifactHash",
    [
      "proofArtifactHash",
      "proof_artifact_hash",
      "proverArtifactHash",
      "prover_artifact_hash",
      "circuitArtifactHash",
      "circuit_artifact_hash",
    ],
  );
  const provingKeyHash = optionalRouteHash("route manifest provingKeyHash", [
    "provingKeyHash",
    "proving_key_hash",
  ]);
  const declaredNativeEvmProverBundleHash = optionalRouteHash(
    "route manifest nativeEvmProverBundleHash",
    [
      "nativeEvmProverBundleHash",
      "native_evm_prover_bundle_hash",
      "nativeProverBundleHash",
      "native_prover_bundle_hash",
      "bscNativeEvmProverBundleHash",
      "bsc_native_evm_prover_bundle_hash",
    ],
  );
  if (Boolean(proofArtifactHash) !== Boolean(provingKeyHash)) {
    throw new Error(
      "route manifest proofArtifactHash and provingKeyHash must be supplied together.",
    );
  }
  const diagnosticVerifierReasons = [
    diagnosticFlagReason(record, "route manifest"),
    diagnosticFlagReason(
      destinationRollout,
      "route manifest destinationRollout",
    ),
    diagnosticFlagReason(
      destinationBinding,
      "route manifest destinationBinding",
    ),
    isKnownDiagnosticBscVerifierKeyHash(verifierKeyHash)
      ? `verifierKeyHash=${verifierKeyHash} is a known diagnostic BSC verifier key hash`
      : "",
  ].filter(Boolean);
  if (productionReady && diagnosticVerifierReasons.length > 0) {
    throw new Error(
      `route manifest productionReady cannot be true with diagnostic BSC verifier material: ${diagnosticVerifierReasons.join("; ")}.`,
    );
  }
  const handoffPlaceholderReason = productionHandoffPlaceholderReason(
    record,
    "route manifest",
  );
  if (productionReady && handoffPlaceholderReason) {
    throw new Error(
      `route manifest productionReady cannot be true with placeholder handoff material at ${handoffPlaceholderReason}.`,
    );
  }
  if (productionReady && (!proofArtifactHash || !provingKeyHash)) {
    throw new Error(
      "route manifest productionReady requires proofArtifactHash and provingKeyHash.",
    );
  }
  const roleSeparatedHashes = [
    ["verifierCodeHash", verifierCodeHash],
    ["verifierKeyHash", verifierKeyHash],
    ["destinationBindingHash", null],
    ["proofArtifactHash", proofArtifactHash],
    ["provingKeyHash", provingKeyHash],
  ];
  const expectedBindingKey = bscDestinationBindingKey({
    networkId: networkIdHex,
    verifierAddress,
    bridgeAddress,
    verifierCodeHash,
    verifierKeyHash,
  });
  const destinationBindingKey = readRequiredConsistentNormalizedString(
    [
      {
        record,
        keys: ["destinationBindingKey", "destination_binding_key"],
        pathName: "route manifest",
      },
      {
        record: destinationRollout,
        keys: ["destinationBindingKey", "destination_binding_key"],
        pathName: "route manifest destinationRollout",
      },
      {
        record: destinationBinding,
        keys: ["key", "destinationBindingKey", "destination_binding_key"],
        pathName: "route manifest destinationBinding",
      },
    ],
    "route manifest destination binding key",
    (value) => value,
  );
  if (destinationBindingKey !== expectedBindingKey) {
    throw new Error(
      "route manifest destination binding key does not match BSC deployment evidence.",
    );
  }
  const expectedBindingHash = bscDestinationBindingHash({
    networkId: networkIdHex,
    verifierAddress,
    bridgeAddress,
    verifierCodeHash,
    verifierKeyHash,
  });
  const destinationBindingHashSources = [
    {
      record,
      keys: ["destinationBindingHash", "destination_binding_hash"],
      pathName: "route manifest",
    },
    {
      record: destinationRollout,
      keys: ["destinationBindingHash", "destination_binding_hash"],
      pathName: "route manifest destinationRollout",
    },
    {
      record: destinationBinding,
      keys: ["bindingHash", "binding_hash"],
      pathName: "route manifest destinationBinding",
    },
  ];
  assertSingleStringAliasPerSource(
    destinationBindingHashSources,
    "route manifest destinationBindingHash",
  );
  const destinationBindingHash = readRequiredConsistentNormalizedString(
    destinationBindingHashSources,
    "route manifest destination binding hash",
    (value, label) => normalizeCanonicalHex32(value, label),
  );
  if (destinationBindingHash !== expectedBindingHash) {
    throw new Error(
      "route manifest destination binding hash does not match BSC deployment evidence.",
    );
  }
  roleSeparatedHashes[2][1] = destinationBindingHash;
  const nativeEvmProverBundle = normalizeBscRouteNativeEvmProverBundle({
    record,
    destinationRollout,
    productionReady,
    verifierKeyHash,
    proofArtifactHash,
    provingKeyHash,
    destinationBindingHash,
    bscProfile,
  });
  const nativeEvmProverBundleHash = nativeEvmProverBundle
    ? canonicalBscNativeEvmProverBundleHash(nativeEvmProverBundle)
    : null;
  if (
    declaredNativeEvmProverBundleHash &&
    declaredNativeEvmProverBundleHash !== nativeEvmProverBundleHash
  ) {
    throw new Error(
      "route manifest nativeEvmProverBundleHash does not match nativeEvmProverBundle.",
    );
  }
  if (nativeEvmProverBundleHash) {
    roleSeparatedHashes.push([
      "nativeEvmProverBundleHash",
      nativeEvmProverBundleHash,
    ]);
  }
  const seenRouteHashes = new Map();
  for (const [label, value] of roleSeparatedHashes.filter(([, value]) =>
    Boolean(value),
  )) {
    const previous = seenRouteHashes.get(value);
    if (previous) {
      throw new Error(`route manifest ${label} must not equal ${previous}.`);
    }
    seenRouteHashes.set(value, label);
  }

  assertSingleStringAliasPerSource(
    [
      {
        record: burnRecord,
        keys: [
          "contractArtifactB64",
          "contract_artifact_b64",
          "artifactB64",
          "artifact_b64",
        ],
        pathName: "route manifest tairaXorBurnRecord",
      },
    ],
    "route manifest tairaXorBurnRecord.contractArtifactB64",
  );
  const artifact = normalizeStrictBase64(
    readFirstString(
      burnRecord,
      "contractArtifactB64",
      "contract_artifact_b64",
      "artifactB64",
      "artifact_b64",
    ),
    "route manifest tairaXorBurnRecord.contractArtifactB64",
  );
  const artifactSha256 = bytesToHex(sha256(new Uint8Array(artifact.bytes)));
  assertSingleStringAliasPerSource(
    [
      {
        record: burnRecord,
        keys: ["artifactSha256", "artifact_sha256"],
        pathName: "route manifest tairaXorBurnRecord",
      },
    ],
    "route manifest tairaXorBurnRecord.artifactSha256",
  );
  const declaredArtifactSha256 = normalizeCanonicalHex32(
    readFirstString(burnRecord, "artifactSha256", "artifact_sha256"),
    "route manifest tairaXorBurnRecord.artifactSha256",
  );
  if (declaredArtifactSha256 !== artifactSha256) {
    throw new Error(
      "route manifest TAIRA burn-record artifact sha256 does not match artifact bytes.",
    );
  }
  if (productionReady) {
    assertProductionBurnRecordArtifactShape(
      artifact.bytes,
      "route manifest TAIRA burn-record artifact",
    );
  }
  assertSingleStringAliasPerSource(
    [
      {
        record: burnRecord,
        keys: ["settlementAssetDefinitionId", "settlement_asset_definition_id"],
        pathName: "route manifest tairaXorBurnRecord",
      },
    ],
    "route manifest tairaXorBurnRecord.settlementAssetDefinitionId",
  );
  const settlementAssetDefinitionId = normalizeCanonicalAssetDefinitionId(
    readFirstString(
      burnRecord,
      "settlementAssetDefinitionId",
      "settlement_asset_definition_id",
    ),
    "route manifest tairaXorBurnRecord.settlementAssetDefinitionId",
  );
  assertSingleValueAlias(
    burnRecord,
    ["gasLimit", "gas_limit"],
    "route manifest tairaXorBurnRecord",
    "route manifest burn-record gasLimit",
  );
  const gasLimit = normalizePositiveSafeInteger(
    readFirstValue(burnRecord, "gasLimit", "gas_limit"),
    "route manifest burn-record gasLimit",
  );
  assertSingleStringAliasPerSource(
    [
      {
        record: settlement,
        keys: ["routeId", "route_id"],
        pathName: "route manifest settlement",
      },
    ],
    "route manifest settlement.routeId",
  );
  const settlementRouteId = readFirstString(settlement, "routeId", "route_id");
  assertSingleStringAliasPerSource(
    [
      {
        record: settlement,
        keys: ["assetKey", "asset_key"],
        pathName: "route manifest settlement",
      },
    ],
    "route manifest settlement.assetKey",
  );
  const settlementAssetKey = readFirstString(
    settlement,
    "assetKey",
    "asset_key",
  );
  if (settlementRouteId && settlementRouteId !== ROUTE_ID) {
    throw new Error(`route manifest settlement.routeId must be ${ROUTE_ID}.`);
  }
  if (settlementAssetKey && settlementAssetKey !== ASSET_KEY) {
    throw new Error(`route manifest settlement.assetKey must be ${ASSET_KEY}.`);
  }

  assertSingleRecordAlias(
    record,
    ["postDeployLiveEvidence", "post_deploy_live_evidence"],
    "route manifest",
    "route manifest postDeployLiveEvidence",
  );
  const postDeployLiveEvidence =
    readFirstRecord(
      record,
      "postDeployLiveEvidence",
      "post_deploy_live_evidence",
    ) ?? null;
  if (productionReady && !postDeployLiveEvidence) {
    throw new Error(
      "route manifest productionReady requires postDeployLiveEvidence.",
    );
  }
  let normalizedPostDeployLiveEvidence = null;
  if (postDeployLiveEvidence) {
    assertSingleValueAlias(
      postDeployLiveEvidence,
      ["fullTomlReady", "full_toml_ready"],
      "route manifest postDeployLiveEvidence",
      "route manifest postDeployLiveEvidence.fullTomlReady",
    );
    const fullTomlReady = readConsistentBoolean(
      postDeployLiveEvidence,
      ["fullTomlReady", "full_toml_ready"],
      "route manifest postDeployLiveEvidence.fullTomlReady",
    );
    if (productionReady && !fullTomlReady) {
      throw new Error(
        "route manifest productionReady requires postDeployLiveEvidence.fullTomlReady true.",
      );
    }
    const sourceBridgeConfigHashSources = [
      {
        record: postDeployLiveEvidence,
        keys: ["sourceBridgeConfigHash", "source_bridge_config_hash"],
        pathName: "route manifest postDeployLiveEvidence",
      },
    ];
    assertSingleStringAliasPerSource(
      sourceBridgeConfigHashSources,
      "route manifest postDeployLiveEvidence.sourceBridgeConfigHash",
    );
    const postDeployProductionBlockers =
      postDeployLiveEvidenceProductionBlockers(postDeployLiveEvidence);
    if (productionReady && postDeployProductionBlockers.length > 0) {
      throw new Error(
        "route manifest productionReady requires empty postDeployLiveEvidence " +
          `production blockers: ${postDeployProductionBlockers.join("; ")}.`,
      );
    }
    const sourceBridgeConfigHash = readRequiredConsistentNormalizedString(
      sourceBridgeConfigHashSources,
      "route manifest postDeployLiveEvidence.sourceBridgeConfigHash",
      (value, label) => normalizeCanonicalHex32(value, label),
    );
    const sourceEventTransactionIdSources = [
      {
        record: postDeployLiveEvidence,
        keys: ["sourceEventTransactionId", "source_event_transaction_id"],
        pathName: "route manifest postDeployLiveEvidence",
      },
    ];
    assertSingleStringAliasPerSource(
      sourceEventTransactionIdSources,
      "route manifest postDeployLiveEvidence.sourceEventTransactionId",
    );
    const sourceEventTransactionId = readRequiredConsistentNormalizedString(
      sourceEventTransactionIdSources,
      "route manifest postDeployLiveEvidence.sourceEventTransactionId",
      (value, label) => normalizeCanonicalHex32(value, label),
    );
    const routeCanaryEvidenceHashSources = [
      {
        record: postDeployLiveEvidence,
        keys: ["routeCanaryEvidenceHash", "route_canary_evidence_hash"],
        pathName: "route manifest postDeployLiveEvidence",
      },
    ];
    assertSingleStringAliasPerSource(
      routeCanaryEvidenceHashSources,
      "route manifest postDeployLiveEvidence.routeCanaryEvidenceHash",
    );
    const routeCanaryEvidenceHash = readRequiredConsistentNormalizedString(
      routeCanaryEvidenceHashSources,
      "route manifest postDeployLiveEvidence.routeCanaryEvidenceHash",
      (value, label) => normalizeCanonicalHex32(value, label),
    );
    const routeCanaryTransactionIdSources = [
      {
        record: postDeployLiveEvidence,
        keys: ["routeCanaryTransactionId", "route_canary_transaction_id"],
        pathName: "route manifest postDeployLiveEvidence",
      },
    ];
    assertSingleStringAliasPerSource(
      routeCanaryTransactionIdSources,
      "route manifest postDeployLiveEvidence.routeCanaryTransactionId",
    );
    const routeCanaryTransactionId = readRequiredConsistentNormalizedString(
      routeCanaryTransactionIdSources,
      "route manifest postDeployLiveEvidence.routeCanaryTransactionId",
      (value, label) => normalizeCanonicalHex32(value, label),
    );
    const sourceEventExplorerUrlSources = [
      {
        record: postDeployLiveEvidence,
        keys: [
          "sourceEventExplorerUrl",
          "source_event_explorer_url",
          "sourceEventTransactionUrl",
          "source_event_transaction_url",
        ],
        pathName: "route manifest postDeployLiveEvidence",
      },
    ];
    assertSingleStringAliasPerSource(
      sourceEventExplorerUrlSources,
      "route manifest postDeployLiveEvidence.sourceEventExplorerUrl",
    );
    const sourceEventExplorerUrl = readConsistentNormalizedString(
      sourceEventExplorerUrlSources,
      "route manifest postDeployLiveEvidence.sourceEventExplorerUrl",
      (value, label) =>
        normalizeBscExplorerTxUrl(
          value,
          label,
          sourceEventTransactionId,
          bscProfile,
        ),
    );
    const routeCanaryExplorerUrlSources = [
      {
        record: postDeployLiveEvidence,
        keys: [
          "routeCanaryExplorerUrl",
          "route_canary_explorer_url",
          "routeCanaryTransactionUrl",
          "route_canary_transaction_url",
        ],
        pathName: "route manifest postDeployLiveEvidence",
      },
    ];
    assertSingleStringAliasPerSource(
      routeCanaryExplorerUrlSources,
      "route manifest postDeployLiveEvidence.routeCanaryExplorerUrl",
    );
    const routeCanaryExplorerUrl = readConsistentNormalizedString(
      routeCanaryExplorerUrlSources,
      "route manifest postDeployLiveEvidence.routeCanaryExplorerUrl",
      (value, label) =>
        normalizeBscExplorerTxUrl(
          value,
          label,
          routeCanaryTransactionId,
          bscProfile,
        ),
    );
    const offlineFullTomlSha256Sources = [
      {
        record: postDeployLiveEvidence,
        keys: ["offlineFullTomlSha256", "offline_full_toml_sha256"],
        pathName: "route manifest postDeployLiveEvidence",
      },
    ];
    assertSingleStringAliasPerSource(
      offlineFullTomlSha256Sources,
      "route manifest postDeployLiveEvidence.offlineFullTomlSha256",
    );
    const offlineFullTomlSha256 = readConsistentNormalizedString(
      offlineFullTomlSha256Sources,
      "route manifest postDeployLiveEvidence.offlineFullTomlSha256",
      (value, label) => normalizeCanonicalHex32(value, label),
    );
    if (productionReady && !sourceEventExplorerUrl) {
      throw new Error(
        "route manifest productionReady requires postDeployLiveEvidence.sourceEventExplorerUrl.",
      );
    }
    if (productionReady && !routeCanaryExplorerUrl) {
      throw new Error(
        "route manifest productionReady requires postDeployLiveEvidence.routeCanaryExplorerUrl.",
      );
    }
    if (fullTomlReady && !offlineFullTomlSha256) {
      throw new Error(
        "route manifest postDeployLiveEvidence.fullTomlReady requires postDeployLiveEvidence.offlineFullTomlSha256.",
      );
    }
    if (productionReady && !offlineFullTomlSha256) {
      throw new Error(
        "route manifest productionReady requires postDeployLiveEvidence.offlineFullTomlSha256.",
      );
    }
    normalizedPostDeployLiveEvidence = {
      fullTomlReady,
      sourceBridgeConfigHash,
      sourceEventTransactionId,
      routeCanaryEvidenceHash,
      routeCanaryTransactionId,
      sourceEventExplorerUrl: sourceEventExplorerUrl || null,
      routeCanaryExplorerUrl: routeCanaryExplorerUrl || null,
      offlineFullTomlSha256: offlineFullTomlSha256 || null,
    };
  }
  const explicitDisabledReason = readOptionalCanonicalManifestText(
    record,
    ["disabledReason", "disabled_reason"],
    "route manifest disabledReason",
  );
  if (productionReady && explicitDisabledReason) {
    throw new Error(
      "route manifest productionReady cannot be true when disabledReason is set.",
    );
  }

  assertSingleValueAlias(
    record,
    ["version"],
    "route manifest",
    "route manifest version",
  );
  assertSingleStringAliasPerSource(
    [
      {
        record: burnRecord,
        keys: ["codeHash", "code_hash"],
        pathName: "route manifest tairaXorBurnRecord",
      },
    ],
    "route manifest tairaXorBurnRecord.codeHash",
  );

  return {
    version: normalizeUint32(
      readFirstValue(record, "version") ?? 1,
      "route manifest version",
    ),
    routeId,
    assetKey,
    bscNetwork,
    legacyTronNetwork: chain,
    chain,
    chainIdHex,
    explorerUrl,
    explorerHost,
    counterpartyDomain,
    verifierTarget,
    productionReady,
    disabledReason:
      explicitDisabledReason ??
      (diagnosticVerifierReasons.length > 0
        ? "BSC verifier material is diagnostic and must be replaced before production readiness."
        : null),
    networkIdHex,
    tokenAddress,
    bridgeAddress,
    sourceBridgeAddress,
    verifierAddress,
    verifierCodeHash,
    verifierKeyHash,
    proofArtifactHash,
    provingKeyHash,
    nativeEvmProverBundleHash,
    nativeEvmProverBundle,
    destinationBindingKey,
    destinationBindingHash,
    settlementAssetDefinitionId,
    contractArtifactB64: artifact.text,
    artifactSha256,
    codeHash: normalizeCanonicalHex32(
      readFirstString(burnRecord, "codeHash", "code_hash"),
      "route manifest tairaXorBurnRecord.codeHash",
    ),
    vkBackend: normalizeVerifierKeyRefText(
      readFirstString(vkRef, "backend"),
      "route manifest tairaXorBurnRecord.vkRef.backend",
    ),
    vkName: normalizeVerifierKeyRefText(
      readFirstString(vkRef, "name"),
      "route manifest tairaXorBurnRecord.vkRef.name",
    ),
    gasLimit,
    settlementContractAddress: readOptionalCanonicalManifestText(
      settlement,
      ["contractAddress", "contract_address"],
      "route manifest settlement.contractAddress",
      { allowNull: true },
    ),
    settlementContractAlias: readOptionalCanonicalManifestText(
      settlement,
      ["contractAlias", "contract_alias"],
      "route manifest settlement.contractAlias",
      { allowNull: true },
    ),
    postDeployLiveEvidence: normalizedPostDeployLiveEvidence,
  };
}

function tomlString(value, label) {
  return JSON.stringify(normalizeNonEmptyText(value, label));
}

function tomlOptionalStringLine(key, value, label) {
  if (value === undefined || value === null || value === "") return [];
  return [`${key} = ${tomlString(value, label)}`];
}

export function buildBscTairaXorRouteConfigToml(manifest, options = {}) {
  const route = normalizeRouteManifestForConfig(manifest);
  const allowUnready = optionEnabled(
    options,
    "allow-unready",
    !route.productionReady,
  );
  if (!route.productionReady && !allowUnready) {
    throw new Error(
      "non-production route manifests require --allow-unready true.",
    );
  }
  if (route.productionReady && allowUnready) {
    throw new Error(
      "production-ready route manifests cannot enable --allow-unready.",
    );
  }
  const lines = [
    "# Generated by scripts/sccp_bsc_taira_xor_deploy.mjs route-config.",
    `# Merge this overlay into the TAIRA Torii/Iroha runtime config for ${route.chain} smoke.`,
    "# BSC route fields are emitted once to avoid ambiguous alias selection.",
    "[zk]",
    `sccp_allow_unready_transparent_proofs = ${allowUnready ? "true" : "false"}`,
    "",
    "[[zk.sccp_route_manifests]]",
    `version = ${route.version}`,
    `route_id = ${tomlString(route.routeId, "route_id")}`,
    `asset_key = ${tomlString(route.assetKey, "asset_key")}`,
    `tron_network = ${tomlString(route.legacyTronNetwork, "tron_network")}`,
    `chain = ${tomlString(route.chain, "chain")}`,
    `chain_id_hex = ${tomlString(route.chainIdHex, "chain_id_hex")}`,
    `explorer_url = ${tomlString(route.explorerUrl, "explorer_url")}`,
    `explorer_host = ${tomlString(route.explorerHost, "explorer_host")}`,
    `counterparty_domain = ${route.counterpartyDomain}`,
    `verifier_target = ${tomlString(route.verifierTarget, "verifier_target")}`,
    `production_ready = ${route.productionReady ? "true" : "false"}`,
    ...tomlOptionalStringLine(
      "disabled_reason",
      route.disabledReason,
      "disabled_reason",
    ),
    `network_id_hex = ${tomlString(route.networkIdHex, "network_id_hex")}`,
    `taira_xor_token_address = ${tomlString(route.tokenAddress, "taira_xor_token_address")}`,
    `taira_xor_bridge_address = ${tomlString(route.bridgeAddress, "taira_xor_bridge_address")}`,
    `sccp_bsc_source_bridge_address = ${tomlString(route.sourceBridgeAddress, "sccp_bsc_source_bridge_address")}`,
    `sccp_bsc_destination_verifier_address = ${tomlString(route.verifierAddress, "sccp_bsc_destination_verifier_address")}`,
    `verifier_code_hash = ${tomlString(route.verifierCodeHash, "verifier_code_hash")}`,
    `verifier_key_hash = ${tomlString(route.verifierKeyHash, "verifier_key_hash")}`,
    ...tomlOptionalStringLine(
      "proof_artifact_hash",
      route.proofArtifactHash,
      "proof_artifact_hash",
    ),
    ...tomlOptionalStringLine(
      "proving_key_hash",
      route.provingKeyHash,
      "proving_key_hash",
    ),
    ...tomlOptionalStringLine(
      "native_evm_prover_bundle_hash",
      route.nativeEvmProverBundleHash,
      "native_evm_prover_bundle_hash",
    ),
    `destination_binding_key = ${tomlString(route.destinationBindingKey, "destination_binding_key")}`,
    `destination_binding_hash = ${tomlString(route.destinationBindingHash, "destination_binding_hash")}`,
    `taira_burn_record_settlement_asset_definition_id = ${tomlString(route.settlementAssetDefinitionId, "taira_burn_record_settlement_asset_definition_id")}`,
    `taira_burn_record_contract_artifact_b64 = ${tomlString(route.contractArtifactB64, "taira_burn_record_contract_artifact_b64")}`,
    `taira_burn_record_artifact_sha256 = ${tomlString(route.artifactSha256, "taira_burn_record_artifact_sha256")}`,
    `taira_burn_record_code_hash = ${tomlString(route.codeHash, "taira_burn_record_code_hash")}`,
    `taira_burn_record_vk_backend = ${tomlString(route.vkBackend, "taira_burn_record_vk_backend")}`,
    `taira_burn_record_vk_name = ${tomlString(route.vkName, "taira_burn_record_vk_name")}`,
    `taira_burn_record_gas_limit = ${route.gasLimit}`,
    ...tomlOptionalStringLine(
      "settlement_contract_address",
      route.settlementContractAddress,
      "settlement_contract_address",
    ),
    ...tomlOptionalStringLine(
      "settlement_contract_alias",
      route.settlementContractAlias,
      "settlement_contract_alias",
    ),
  ];
  if (route.postDeployLiveEvidence) {
    lines.push(
      `post_deploy_full_toml_ready = ${route.postDeployLiveEvidence.fullTomlReady ? "true" : "false"}`,
      `post_deploy_source_bridge_config_hash = ${tomlString(route.postDeployLiveEvidence.sourceBridgeConfigHash, "post_deploy_source_bridge_config_hash")}`,
      `post_deploy_source_event_transaction_id = ${tomlString(route.postDeployLiveEvidence.sourceEventTransactionId, "post_deploy_source_event_transaction_id")}`,
      ...tomlOptionalStringLine(
        "post_deploy_source_event_explorer_url",
        route.postDeployLiveEvidence.sourceEventExplorerUrl,
        "post_deploy_source_event_explorer_url",
      ),
      `post_deploy_route_canary_evidence_hash = ${tomlString(route.postDeployLiveEvidence.routeCanaryEvidenceHash, "post_deploy_route_canary_evidence_hash")}`,
      `post_deploy_route_canary_transaction_id = ${tomlString(route.postDeployLiveEvidence.routeCanaryTransactionId, "post_deploy_route_canary_transaction_id")}`,
      ...tomlOptionalStringLine(
        "post_deploy_route_canary_explorer_url",
        route.postDeployLiveEvidence.routeCanaryExplorerUrl,
        "post_deploy_route_canary_explorer_url",
      ),
      ...tomlOptionalStringLine(
        "post_deploy_offline_full_toml_sha256",
        route.postDeployLiveEvidence.offlineFullTomlSha256,
        "post_deploy_offline_full_toml_sha256",
      ),
    );
  }
  return `${lines.join("\n")}\n`;
}

function routeConfigOverlayParts(manifest, options = {}) {
  const overlay = buildBscTairaXorRouteConfigToml(manifest, options);
  const overlayLines = overlay.trimEnd().split(/\r?\n/u);
  const allowLine = overlayLines.find((line) =>
    /^sccp_allow_unready_transparent_proofs\s*=/u.test(line),
  );
  const routeStart = overlayLines.findIndex(
    (line) => line.trim() === "[[zk.sccp_route_manifests]]",
  );
  if (!allowLine || routeStart < 0) {
    throw new Error("generated BSC route config overlay is incomplete.");
  }
  return { allowLine, routeLines: overlayLines.slice(routeStart) };
}

export function buildMergedBscTairaXorRouteConfigToml(
  baseConfigText,
  manifest,
  options = {},
) {
  const baseConfig = String(baseConfigText ?? "").replace(/\r\n?/gu, "\n");
  if (/^\s*\[\[zk\.sccp_route_manifests\]\]\s*$/mu.test(baseConfig)) {
    throw new Error(
      "base TAIRA config already contains zk.sccp_route_manifests; merge route manifests manually to avoid duplicate routes.",
    );
  }
  const { allowLine, routeLines } = routeConfigOverlayParts(manifest, options);
  const lines = baseConfig.split("\n");
  const zkStart = lines.findIndex((line) => line.trim() === "[zk]");
  const mergedRouteLines = [
    "# Generated by scripts/sccp_bsc_taira_xor_deploy.mjs route-config --base-config.",
    "# Public TAIRA/BSC smoke requires this route to be present in the node runtime config.",
    ...routeLines,
  ];

  if (zkStart < 0) {
    const trimmed = baseConfig.replace(/\s*$/u, "");
    return `${trimmed}\n\n[zk]\n${allowLine}\n\n${mergedRouteLines.join("\n")}\n`;
  }

  let zkEnd = lines.length;
  for (let index = zkStart + 1; index < lines.length; index += 1) {
    if (/^\s*\[/u.test(lines[index])) {
      zkEnd = index;
      break;
    }
  }
  const zkBody = lines
    .slice(zkStart + 1, zkEnd)
    .filter(
      (line) => !/^\s*sccp_allow_unready_transparent_proofs\s*=/u.test(line),
    );
  const mergedLines = [
    ...lines.slice(0, zkStart),
    "[zk]",
    allowLine,
    ...zkBody,
    "",
    ...mergedRouteLines,
    ...lines.slice(zkEnd),
  ];
  return `${mergedLines.join("\n").replace(/\s*$/u, "")}\n`;
}

function canonicalizeBscOfflineFullConfigTomlForHash(toml) {
  const normalized = String(toml ?? "").replace(/\r\n?/gu, "\n");
  const filtered = normalized
    .split("\n")
    .filter(
      (line) => !/^\s*post_deploy_offline_full_toml_sha256\s*=/u.test(line),
    )
    .join("\n");
  return filtered.endsWith("\n") ? filtered : `${filtered}\n`;
}

function bscOfflineFullTomlSha256(toml) {
  return bytesToHex(
    sha256(
      textEncoder.encode(canonicalizeBscOfflineFullConfigTomlForHash(toml)),
    ),
  );
}

function normalizeBscRouteManifestPath(value) {
  if (value === undefined || value === null || value === "") {
    return DEFAULT_ROUTE_MANIFEST_OUT;
  }
  return normalizeNonEmptyText(value, "BSC route manifest path");
}

function buildBscOfflineFullTomlEvidence({
  manifest,
  profile,
  manifestPath,
  baseConfigPath,
  fullConfigPath,
  renderedTomlSha256,
  offlineFullTomlSha256,
  hashInputSha256,
}) {
  const route = normalizeRouteManifestForConfig(manifest);
  const expectedProfile =
    BSC_NETWORK_PROFILES[route.bscNetwork] ?? BSC_NETWORK_PROFILES.testnet;
  if (expectedProfile.key !== profile.key) {
    throw new Error(
      "BSC offline full TOML evidence profile must match route manifest network.",
    );
  }
  return {
    schema: OFFLINE_FULL_TOML_EVIDENCE_SCHEMA,
    routeId: route.routeId,
    assetKey: route.assetKey,
    bscNetwork: profile.key,
    chain: profile.chain,
    chainIdHex: profile.chainIdHex,
    networkIdHex: profile.networkIdHex,
    fullTomlReady: true,
    offlineFullTomlSha256,
    hashMode:
      "sha256:merged-full-config-without-post_deploy_offline_full_toml_sha256",
    hashInputSha256,
    renderedTomlSha256,
    routeManifestPath: manifestPath,
    fullConfigPath,
    baseConfigProvided: Boolean(baseConfigPath),
    postDeployLiveEvidence: {
      fullTomlReady: true,
      offlineFullTomlSha256,
    },
  };
}

async function commandCompile(options) {
  const out = resolve(options.out ?? DEFAULT_ARTIFACTS_OUT);
  const { artifacts, warnings } = await compileBscContracts({ writeOut: out });
  return {
    ok: true,
    wrote: out,
    warnings: warnings.map((entry) => entry.formattedMessage ?? entry.message),
    contracts: Object.fromEntries(
      Object.entries(artifacts).map(([key, artifact]) => [
        key,
        {
          contractName: artifact.contractName,
          bytecodeSha256: artifact.bytecodeSha256,
          deployedBytecodeSha256: artifact.deployedBytecodeSha256,
        },
      ]),
    ),
  };
}

async function commandDeploy(options) {
  const profile = bscNetworkProfileFromOptions(options);
  if (!parseBoolean(options.broadcast, "--broadcast")) {
    throw new Error(
      `deploy requires --broadcast true and --confirm-network ${profile.confirmNetwork}.`,
    );
  }
  requireBscNetworkConfirmation(options, profile, "deploy");
  if (!options.verifier) {
    throw new Error("deploy requires --verifier <verifier-key.json>.");
  }
  const allowDiagnosticVerifier = parseBoolean(
    options["allow-diagnostic-verifier"],
    "--allow-diagnostic-verifier",
  );
  const verifierMaterial = normalizeVerifierMaterial(
    await readJson(options.verifier),
    profile,
  );
  if (verifierMaterial.fixtureShaped) {
    throw new Error(
      "deploy refuses deterministic smoke-test Groth16 fixture BSC verifier material.",
    );
  }
  if (
    verifierMaterial.diagnosticVerifierReasons.length > 0 &&
    !allowDiagnosticVerifier
  ) {
    throw new Error(
      `deploy refuses diagnostic BSC verifier material without --allow-diagnostic-verifier true: ${verifierMaterial.diagnosticVerifierReasons.join("; ")}.`,
    );
  }
  const privateKeyEnv = options["private-key-env"] ?? DEFAULT_PRIVATE_KEY_ENV;
  const privateKey = normalizePrivateKey(
    process.env[privateKeyEnv],
    privateKeyEnv,
  );
  const rpcUrl = normalizeBscRpcUrl(
    options["rpc-url"] ?? defaultBscRpcUrl(profile),
    {
      allowLocal: parseBoolean(options["allow-local-rpc"], "--allow-local-rpc"),
    },
  );
  const ethers = requireOptionalPackage("ethers");
  const provider = new ethers.JsonRpcProvider(
    rpcUrl,
    BigInt(profile.chainIdHex),
  );
  const network = await provider.getNetwork();
  if (network.chainId !== BigInt(profile.chainIdHex)) {
    throw new Error(
      `BSC RPC must report ${profile.label} chain id ${profile.chainIdHex}; received ${network.chainId}.`,
    );
  }
  const wallet = new ethers.Wallet(privateKey, provider);
  const signer = new ethers.NonceManager(wallet);
  const { artifacts } = await compileBscContracts();
  const verifierArgs = [
    verifierMaterial.alpha1,
    verifierMaterial.beta2,
    verifierMaterial.gamma2,
    verifierMaterial.delta2,
    verifierMaterial.ic,
  ];
  const verifier = await deployContract(
    ethers,
    signer,
    artifacts.verifier,
    verifierArgs,
  );
  const verifierCodeHash = normalizeHex32(
    ethers.keccak256(await provider.getCode(verifier.address)),
  );
  const sourceBridge = await deployContract(
    ethers,
    signer,
    artifacts.sourceBridge,
    [profile.networkIdHex, SCCP_DOMAIN_BSC, SCCP_DOMAIN_SORA],
  );
  const token = await deployContract(ethers, signer, artifacts.token, []);
  const routeIdHash = keccakTextHex(ROUTE_ID);
  const assetKeyHash = keccakTextHex(ASSET_KEY);
  const bridge = await deployContract(ethers, signer, artifacts.bridge, [
    token.address,
    verifier.address,
    sourceBridge.address,
    verifierCodeHash,
    verifierMaterial.expectedVerifierKeyHash,
    BSC_EVM_GROTH16_BACKEND,
    SCCP_PROOF_FAMILY_STARK_FRI,
    profile.networkIdHex,
    SCCP_DOMAIN_SORA,
    SCCP_DOMAIN_BSC,
    routeIdHash,
    assetKeyHash,
  ]);
  const tokenContract = new ethers.Contract(token.address, TOKEN_ABI, signer);
  const sourceBridgeContract = new ethers.Contract(
    sourceBridge.address,
    SOURCE_BRIDGE_ABI,
    signer,
  );
  const setBridgeTx = await tokenContract.setBridge(bridge.address);
  const setBridgeReceipt = await setBridgeTx.wait();
  const lockBridgeTx = await tokenContract.lockBridge();
  const lockBridgeReceipt = await lockBridgeTx.wait();
  const transferSourceOwnerTx = await sourceBridgeContract.transferOwnership(
    bridge.address,
  );
  const transferSourceOwnerReceipt = await transferSourceOwnerTx.wait();
  const readback = await fetchReadback(ethers, provider, {
    tokenAddress: token.address,
    bridgeAddress: bridge.address,
    sourceBridgeAddress: sourceBridge.address,
    verifierAddress: verifier.address,
  });
  const evidence = buildDeploymentEvidence({
    tokenAddress: token.address,
    bridgeAddress: bridge.address,
    sourceBridgeAddress: sourceBridge.address,
    verifierAddress: verifier.address,
    verifierCodeHash,
    verifierKeyHash: verifierMaterial.expectedVerifierKeyHash,
    readback,
    bscNetwork: profile.key,
  });
  const out = resolve(options.out ?? defaultDeploymentEvidenceOut(profile));
  assertBscCanonicalProductionOutputSafe(
    out,
    evidence,
    "BSC deployment evidence",
  );
  await writeJsonNoSecrets(out, {
    ...evidence,
    deploymentTransactions: {
      verifier: verifier.txHash,
      sourceBridge: sourceBridge.txHash,
      token: token.txHash,
      bridge: bridge.txHash,
      setBridge: setBridgeReceipt.hash,
      lockBridge: lockBridgeReceipt.hash,
      transferSourceBridgeOwnership: transferSourceOwnerReceipt.hash,
    },
    deployerAddress: normalizeEvmAddress(await wallet.getAddress()),
  });
  return {
    ok: true,
    wrote: out,
    deployerAddress: normalizeEvmAddress(await wallet.getAddress()),
    bscVerifierAddress: verifier.address,
    sccpBscSourceBridgeAddress: sourceBridge.address,
    bscTokenAddress: token.address,
    bscBridgeAddress: bridge.address,
    destinationBindingHash: evidence.destinationRollout.destinationBindingHash,
  };
}

async function commandEvidence(options) {
  const profile = bscNetworkProfileFromOptions(options);
  for (const key of ["token", "bridge", "source-bridge", "verifier"]) {
    if (!options[key]) {
      throw new Error(`evidence requires --${key} <address>.`);
    }
  }
  const rpcUrl = normalizeBscRpcUrl(
    options["rpc-url"] ?? defaultBscRpcUrl(profile),
    {
      allowLocal: parseBoolean(options["allow-local-rpc"], "--allow-local-rpc"),
    },
  );
  const ethers = requireOptionalPackage("ethers");
  const provider = new ethers.JsonRpcProvider(
    rpcUrl,
    BigInt(profile.chainIdHex),
  );
  const readback = await fetchReadback(ethers, provider, {
    tokenAddress: options.token,
    bridgeAddress: options.bridge,
    sourceBridgeAddress: options["source-bridge"],
    verifierAddress: options.verifier,
  });
  const evidence = buildDeploymentEvidence({
    tokenAddress: options.token,
    bridgeAddress: options.bridge,
    sourceBridgeAddress: options["source-bridge"],
    verifierAddress: options.verifier,
    verifierCodeHash: readback.bridgeVerifierCodeHash,
    verifierKeyHash: readback.bridgeVerifierKeyHash,
    readback,
    bscNetwork: profile.key,
  });
  const out = resolve(options.out ?? defaultDeploymentEvidenceOut(profile));
  assertBscCanonicalProductionOutputSafe(
    out,
    evidence,
    "BSC deployment evidence",
  );
  await writeJsonNoSecrets(out, evidence);
  return {
    ok: true,
    wrote: out,
    destinationBindingHash: evidence.destinationRollout.destinationBindingHash,
  };
}

async function commandRouteManifest(options) {
  const evidence = await readJson(
    options.evidence ?? options["deployment-evidence"] ?? DEFAULT_EVIDENCE_OUT,
    "BSC deployment evidence",
  );
  const tairaContract = await readJson(
    options["taira-contract"] ?? DEFAULT_TAIRA_BURN_RECORD_CONTRACT_OUT,
    "TAIRA burn-record contract",
  );
  const liveEvidence = options["live-evidence"]
    ? await readJson(options["live-evidence"], "BSC live route evidence")
    : null;
  const offlineFullTomlEvidence = options["offline-full-toml-evidence"]
    ? await readJson(
        options["offline-full-toml-evidence"],
        "BSC offline full TOML evidence",
      )
    : null;
  const manifest = await buildBscTairaXorRouteManifestDraft({
    options,
    evidence,
    tairaContract,
    liveEvidence,
    offlineFullTomlEvidence,
  });
  const out = resolve(
    options.out ??
      defaultRouteManifestOut(
        BSC_NETWORK_PROFILES[manifest.bscNetwork] ??
          BSC_NETWORK_PROFILES.testnet,
      ),
  );
  assertBscCanonicalProductionOutputSafe(out, manifest, "BSC route manifest");
  await writeJsonNoSecrets(out, manifest);
  return {
    ok: true,
    wrote: out,
    routeId: manifest.routeId,
    assetKey: manifest.assetKey,
    bscNetwork: manifest.bscNetwork,
    productionReady: manifest.productionReady,
    bscBridgeAddress: manifest.bscBridgeAddress,
    bscTokenAddress: manifest.bscTokenAddress,
    bscVerifierAddress: manifest.bscVerifierAddress,
    destinationBindingHash: manifest.destinationRollout.destinationBindingHash,
    proofArtifactHash: manifest.proofArtifactHash ?? null,
    provingKeyHash: manifest.provingKeyHash ?? null,
    nativeEvmProverBundleHash: manifest.nativeEvmProverBundleHash ?? null,
    offlineFullTomlSha256:
      manifest.postDeployLiveEvidence?.offlineFullTomlSha256 ?? null,
    settlementAssetDefinitionId:
      manifest.tairaXorBurnRecord.settlementAssetDefinitionId,
    nextStep: manifest.productionReady
      ? "Generate the TAIRA route config from this manifest, deploy it to every public peer, rerun peer audit/smoke-readiness/production gate, then capture live UI video proof."
      : "Attach production verifier/proof/native-prover/live evidence, rerun route-manifest with production-ready confirmations, then generate and deploy the TAIRA route config.",
  };
}

async function commandRouteConfig(options) {
  const manifestPath = normalizeBscRouteManifestPath(
    options.manifest ?? DEFAULT_ROUTE_MANIFEST_OUT,
  );
  const manifest = await readJson(manifestPath, "BSC route manifest");
  const profile =
    BSC_NETWORK_PROFILES[
      readFirstValue(manifest, "bscNetwork", "bsc_network", "network")
    ] ?? BSC_NETWORK_PROFILES.testnet;
  const baseConfigPath = options["base-config"] ?? null;
  const toml = baseConfigPath
    ? buildMergedBscTairaXorRouteConfigToml(
        await readText(baseConfigPath, "base TAIRA config"),
        manifest,
        options,
      )
    : buildBscTairaXorRouteConfigToml(manifest, options);
  const outPath =
    options.out ??
    defaultRouteConfigOut(profile, { fullConfigMode: Boolean(baseConfigPath) });
  assertBscCanonicalProductionOutputSafe(
    outPath,
    manifest,
    "BSC route config manifest",
  );
  const out = await writeTextNoSecrets(outPath, toml, 0o644);
  const renderedTomlSha256 = bytesToHex(sha256(textEncoder.encode(toml)));
  const fullConfigMode = Boolean(baseConfigPath);
  const hashInputToml = fullConfigMode
    ? canonicalizeBscOfflineFullConfigTomlForHash(toml)
    : null;
  const hashInputSha256 = hashInputToml
    ? bytesToHex(sha256(textEncoder.encode(hashInputToml)))
    : null;
  const offlineFullTomlSha256 = fullConfigMode ? hashInputSha256 : null;
  let offlineFullTomlEvidenceOut = null;
  let offlineFullTomlEvidence = null;
  if (options["write-offline-full-toml-evidence"]) {
    if (!fullConfigMode) {
      throw new Error(
        "--write-offline-full-toml-evidence requires --base-config.",
      );
    }
    offlineFullTomlEvidenceOut = resolve(
      options["write-offline-full-toml-evidence"] === "true"
        ? defaultRouteFullConfigEvidenceOut(profile)
        : options["write-offline-full-toml-evidence"],
    );
    offlineFullTomlEvidence = buildBscOfflineFullTomlEvidence({
      manifest,
      profile,
      manifestPath,
      baseConfigPath,
      fullConfigPath: out,
      renderedTomlSha256,
      offlineFullTomlSha256,
      hashInputSha256,
    });
    assertBscCanonicalProductionOutputSafe(
      offlineFullTomlEvidenceOut,
      offlineFullTomlEvidence,
      "BSC offline full TOML evidence",
    );
    await writeJsonNoSecrets(
      offlineFullTomlEvidenceOut,
      offlineFullTomlEvidence,
    );
  }
  return {
    ok: true,
    wrote: out,
    wroteOfflineFullTomlEvidence: offlineFullTomlEvidenceOut,
    mode: fullConfigMode ? "merged-full-config" : "overlay",
    baseConfig: baseConfigPath ? resolve(baseConfigPath) : null,
    renderedTomlSha256,
    offlineFullTomlSha256,
    offlineFullTomlHashMode: fullConfigMode
      ? "sha256:merged-full-config-without-post_deploy_offline_full_toml_sha256"
      : null,
    offlineFullTomlEvidence,
    routeId: readFirstValue(manifest, "routeId", "route_id") ?? null,
    assetKey: readFirstValue(manifest, "assetKey", "asset_key") ?? null,
    productionReady:
      readFirstValue(manifest, "productionReady", "production_ready") ?? null,
    allowUnready: optionEnabled(
      options,
      "allow-unready",
      readFirstValue(manifest, "productionReady", "production_ready") !== true,
    ),
    nextStep: baseConfigPath
      ? "Deploy this merged TAIRA node config on every public validator and restart Torii/Iroha before rerunning the BSC SCCP preflight without --manifest-file."
      : "Merge this TOML into the TAIRA node config and redeploy/restart Torii before rerunning the BSC SCCP preflight.",
  };
}

async function commandNativeProverBundle(options) {
  const result = await buildBscNativeEvmProverBundleFromArtifacts(options);
  const profile = bscProfileFromNativeEvmProverBundle(result.bundle);
  const out = resolve(options.out ?? defaultNativeEvmProverBundleOut(profile));
  assertBscCanonicalProductionOutputSafe(
    out,
    result.bundle,
    "BSC native EVM prover bundle",
  );
  await writeJsonNoSecrets(out, result.bundle);
  let attachedRouteManifestOut = null;
  if (options["attach-route-manifest-out"]) {
    if (!result.attachedRouteManifest) {
      throw new Error(
        "--attach-route-manifest-out requires --route-manifest input.",
      );
    }
    attachedRouteManifestOut = resolve(options["attach-route-manifest-out"]);
    assertBscCanonicalProductionOutputSafe(
      attachedRouteManifestOut,
      result.attachedRouteManifest,
      "BSC route manifest",
    );
    await writeJsonNoSecrets(
      attachedRouteManifestOut,
      result.attachedRouteManifest,
    );
  }
  return {
    ok: true,
    wrote: out,
    attachedRouteManifest: attachedRouteManifestOut,
    routeSource: result.routeSource.kind,
    artifactRoot: result.artifactRoot,
    routeId: ROUTE_ID,
    assetKey: ASSET_KEY,
    bundleId: result.descriptor.bundleId,
    destinationBindingHash: result.descriptor.destinationBindingHash,
    proofArtifactHash: result.descriptor.proofArtifactHash,
    provingKeyHash: result.descriptor.provingKeyHash,
    verifierKeyHash: result.descriptor.verifierKeyHash,
    verifiedSdks: result.verifiedSdks,
    nextStep:
      "Attach this nativeEvmProverBundle to the production BSC route manifest, regenerate the TAIRA route config, redeploy every public peer, then rerun the BSC SCCP production gates.",
  };
}

async function commandRequirements(options) {
  const profile = bscNetworkProfileFromOptions(options);
  const requirements = bscProductionRequirements(options);
  if (!options.out) {
    return requirements;
  }
  const out = resolve(options.out);
  await writeJsonNoSecrets(out, requirements);
  return {
    ok: true,
    wrote: out,
    schema: requirements.schema,
    routeId: requirements.routeId,
    assetKey: requirements.assetKey,
    bscNetwork: profile.key,
    inputCount: requirements.inputs.length,
    requiredReports: requirements.requiredReports,
    nextStep:
      "Fill every public requirement with production artifacts/evidence, then run deploy, native-prover-bundle, route-config, peer audit, smoke readiness, production gate, and live video proof.",
  };
}

async function commandSelfTest() {
  const verifierAddress = "0x4444444444444444444444444444444444444444";
  const bridgeAddress = "0x1111111111111111111111111111111111111111";
  const verifierCodeHash = `0x${"11".repeat(32)}`;
  const verifierKeyHash = `0x${"22".repeat(32)}`;
  const bindingHash = bscDestinationBindingHash({
    verifierAddress,
    bridgeAddress,
    verifierCodeHash,
    verifierKeyHash,
  });
  const readback = {
    chainIdHex: BSC_TESTNET_CHAIN_ID_HEX,
    codePresent: {
      token: true,
      bridge: true,
      sourceBridge: true,
      verifier: true,
    },
    tokenBridgeAddress: bridgeAddress,
    tokenBridgeLocked: true,
    sourceBridgeOwner: bridgeAddress,
    verifierKeyHash,
    bridgeDestinationBindingHash: bindingHash,
    bridgeVerifierAddress: verifierAddress,
    bridgeVerifierCodeHash: verifierCodeHash,
    bridgeVerifierKeyHash: verifierKeyHash,
    bridgeNetworkId: BSC_TESTNET_NETWORK_ID_HEX,
    bridgeSourceDomain: SCCP_DOMAIN_SORA,
    bridgeTargetDomain: SCCP_DOMAIN_BSC,
  };
  buildDeploymentEvidence({
    tokenAddress: "0x2222222222222222222222222222222222222222",
    bridgeAddress,
    sourceBridgeAddress: "0x3333333333333333333333333333333333333333",
    verifierAddress,
    verifierCodeHash,
    verifierKeyHash,
    readback,
  });
  if (
    !unsafeSecretReason({ public: "ok" }) &&
    unsafeSecretReason({ private_key: "0x1" })
  ) {
    return { ok: true };
  }
  throw new Error("self-test secret scanner failed.");
}

export async function main(argv = process.argv.slice(2)) {
  const [command, ...rest] = argv;
  if (!command || command === "help" || command === "--help") {
    return { help: usage() };
  }
  const options = parseArgs(rest);
  switch (command) {
    case "compile":
      return commandCompile(options);
    case "deploy":
      return commandDeploy(options);
    case "evidence":
      return commandEvidence(options);
    case "route-manifest":
      return commandRouteManifest(options);
    case "native-prover-bundle":
      return commandNativeProverBundle(options);
    case "route-config":
      return commandRouteConfig(options);
    case "requirements":
      return commandRequirements(options);
    case "self-test":
      return commandSelfTest();
    default:
      throw new Error(`Unknown command: ${command}\n${usage()}`);
  }
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  main()
    .then((result) => {
      if (result?.help) {
        console.log(result.help);
      } else {
        console.log(JSON.stringify(result, null, 2));
      }
    })
    .catch((error) => {
      console.error(error instanceof Error ? error.message : String(error));
      process.exitCode = 1;
    });
}
