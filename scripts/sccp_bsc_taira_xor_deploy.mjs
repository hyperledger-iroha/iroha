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
import { spawn } from "node:child_process";
import {
  createPublicKey,
  verify as verifyDetachedSignature,
} from "node:crypto";
import {
  lstat,
  mkdtemp,
  mkdir,
  readFile,
  realpath,
  rename,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, extname, isAbsolute, join, relative, resolve, win32 } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { sha256 } from "../javascript/iroha_js/node_modules/@noble/hashes/sha256.js";
import { keccak_256 } from "../javascript/iroha_js/node_modules/@noble/hashes/sha3.js";
import {
  SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
  SCCP_BSC_MAINNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
  SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
  SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
  SCCP_NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1,
  parseBscTestnetNativeEvmProverParityReport,
  parseBscMainnetNativeEvmProverParityReport,
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
const BN254_SCALAR_FIELD_MODULUS = BigInt(
  "21888242871839275222246405745257275088548364400416034343698204186575808495617",
);
const BN254_BASE_FIELD_MODULUS = BigInt(
  "21888242871839275222246405745257275088696311157297823662689037894645226208583",
);
const DECIMAL_WORD = /^(?:0|[1-9][0-9]*)$/u;
const SNARKJS_ZKEY_VERIFY_OK = "ZKey Ok!";
const BSC_GROTH16_SELF_TEST_SAMPLE_ID =
  "sccp-bsc-groth16-full-message-self-test-v1";
const BSC_GROTH16_SIGNAL_INPUT_NAMES = Object.freeze([
  "messageIdBits",
  "payloadHashBits",
  "targetDomainBits",
  "commitmentRootBits",
  "finalityHeightBits",
  "finalityBlockHashBits",
  "sourceDomainBits",
  "statementHashBits",
  "destinationBindingHashBits",
]);
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
export const SOURCE_PARITY_ATTESTATION_SCHEMA =
  "iroha-sccp-bsc-native-evm-source-parity-attestation/v1";
const NO_WASM_NO_REMOTE_SCAN_SCHEMA =
  "iroha-sccp-bsc-native-evm-no-wasm-no-remote-scan/v1";
const BSC_GROTH16_MATERIAL_MANIFEST_SCHEMA =
  "iroha-sccp-bsc-groth16-material-manifest/v1";
const BSC_GROTH16_PROOF_SELF_TEST_SCHEMA =
  "iroha-sccp-bsc-groth16-proof-self-test/v1";
const BSC_BROWSER_PROVER_MANIFEST_SCHEMA =
  "iroha-demo-sccp-bsc-browser-prover-manifest/v1";
const BSC_FULL_SCCP_CIRCUIT_PROFILE = "sccp-bsc-full-message-v1";
const DEFAULT_BSC_FULL_MESSAGE_CIRCUIT_SOURCE =
  "artifacts/sccp-bsc/circuits/sccp-bsc-full-message-v1.circom";
const BSC_GROTH16_SEMANTIC_ATTESTATION_SCHEMA =
  "iroha-sccp-bsc-groth16-semantic-circuit-attestation/v1";
const BSC_GROTH16_CIRCUIT_SECURITY_ATTESTATION_SCHEMA =
  "iroha-sccp-bsc-groth16-circuit-security-attestation/v1";
const BSC_GROTH16_SEMANTIC_REVIEW_EVIDENCE_SCHEMA =
  "iroha-sccp-bsc-groth16-semantic-review-evidence/v1";
const BSC_GROTH16_CIRCUIT_SECURITY_AUDIT_EVIDENCE_SCHEMA =
  "iroha-sccp-bsc-groth16-circuit-security-audit-evidence/v1";
const BSC_GROTH16_TRUSTED_SETUP_ATTESTATION_SCHEMA =
  "iroha-sccp-bsc-groth16-trusted-setup-attestation/v1";
const BSC_GROTH16_REPRODUCIBLE_BUILD_ATTESTATION_SCHEMA =
  "iroha-sccp-bsc-groth16-reproducible-build-attestation/v1";
const BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA =
  "iroha-sccp-bsc-groth16-attestation-signature/v1";
const BSC_GROTH16_PUBLIC_SIGNAL_NAMES = Object.freeze([
  "message_id",
  "payload_hash",
  "target_domain",
  "commitment_root",
  "finality_height",
  "finality_block_hash",
  "source_domain",
  "statement_hash",
  "destination_binding_hash",
]);
const BSC_GROTH16_PUBLIC_SIGNAL_LABEL_HASHES = Object.freeze([
  "0x091b1715f31adbc0239378caf77a4370e8348599048ec45efb203368dbcc5073",
  "0xd40cf4310af21ab1b3f12db20df99ab8fe63dbe55fc473e5456691c39c1859ac",
  "0x5f7c135fa34a3f53c3733c64f172ef8a639790cfe240c9b454311f8cbfe74f96",
  "0xc3aa105618977410007f32f4eefe0b3eab174af6dac0d95829b92e18912bfbe3",
  "0x0d3499b9350c0ac6add6e0076775de67baee79c5b691f3a4f9317dcb974db599",
  "0x1c5d4645e72d75c0152153a5fe8679a3c0a7ba6cfe3b91986e647c4b26c144bc",
  "0xd07ef0087259b42adc11497be275f42091c6ef51becccd113be860e1b48a5109",
  "0xa4895607d62c8e116357ba7d102e08b5636840e0816a608f3a1fc9d0a1077569",
  "0x094cf24d193ac65c8a450188d16282fba8ee8c5a7539b751857d231f4380c2dd",
]);
const PRODUCTION_FULL_SCCP_MIN_R1CS_CONSTRAINTS = 4096;
const BSC_GROTH16_R1CS_INFO_SOURCES = new Set([
  "snarkjs-cli",
  "binary-header-fallback",
]);
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
export const DEFAULT_NATIVE_EVM_SOURCE_PARITY_ATTESTATION_OUT =
  "artifacts/sccp-bsc/native-prover/source-parity-attestation.json";
export const DEFAULT_PRODUCTION_REQUIREMENTS_OUT =
  "artifacts/sccp-bsc/taira-bsc-xor-production-requirements.json";
export const DEFAULT_ROUTE_MANIFEST_ISI_OUT =
  "artifacts/sccp-bsc/taira-bsc-xor-route-manifest.upsert-isi.json";
export const DEFAULT_TAIRA_BURN_RECORD_VK_TEMPLATE =
  "artifacts/sccp-taira/ivm-execution/taira-xor-burn-record-vk-register.template.json";
export const DEFAULT_TAIRA_BSC_BURN_RECORD_VK_ISI_OUT =
  "artifacts/sccp-bsc/taira-bsc-xor-burn-record-vk.register-isi.json";
export const CANONICAL_BSC_PRODUCTION_ARTIFACT_ROOT = "artifacts/sccp-bsc";
export const DEFAULT_PRIVATE_KEY_ENV = "SCCP_BSC_DEPLOYER_PRIVATE_KEY";
export const DEFAULT_TAIRA_ROUTE_MANIFEST_PRIVATE_KEY_ENV =
  "SCCP_TAIRA_ROUTE_MANIFEST_PRIVATE_KEY";
export const DEFAULT_TAIRA_TORII_URL = "https://taira.sora.org";
export const DEFAULT_TAIRA_CHAIN_ID =
  "809574f5-fee7-5e69-bfcf-52451e42d50f";
export const DEFAULT_TAIRA_ROUTE_MANIFEST_GAS_LIMIT = 2_000_000;
export const TAIRA_BURN_RECORD_ARTIFACT_MIN_BYTES = 32;
export const TAIRA_BURN_RECORD_ARTIFACT_MAX_BYTES = 8 * 1024 * 1024;
export const TAIRA_BURN_RECORD_PRODUCTION_ARTIFACT_MIN_BYTES = 256;
export const SCCP_BSC_JSON_INPUT_MAX_BYTES = 12 * 1024 * 1024;
export const SCCP_BSC_TEXT_INPUT_MAX_BYTES = 8 * 1024 * 1024;
export const SCCP_BSC_BINARY_ARTIFACT_INPUT_MAX_BYTES = 2 * 1024 * 1024 * 1024;
const SCCP_BSC_BROWSER_PROVER_MANIFEST_MAX_BYTES = 256 * 1024;
const EVM_EMPTY_CODE_KECCAK256 =
  "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";
const BSC_CONTRACT_CODE_ROLES = Object.freeze([
  "token",
  "bridge",
  "sourceBridge",
  "verifier",
]);

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
const PRODUCTION_EVIDENCE_FORBIDDEN_WORDS =
  /\b(?:diagnostic|fixture|mock|placeholder|sample|stub|test-fixture|test-only)\b/iu;
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
  cross_sdk_parity: [
    "audit-cross-sdk-parity",
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

const SOURCE_PARITY_REQUIRED_MARKERS_BY_PROFILE = Object.freeze({
  testnet: Object.freeze([
    "BSC_TESTNET_NATIVE_EVM_LOCAL_ADMISSION_BUILDER",
    "BSC_TESTNET_LOCAL_ADMISSION_METADATA",
    "BSC_TESTNET_LOCAL_ADMISSION_ADVERSARIAL_TESTS",
  ]),
  mainnet: Object.freeze([
    "BSC_MAINNET_NATIVE_EVM_LOCAL_ADMISSION_BUILDER",
    "BSC_MAINNET_LOCAL_ADMISSION_METADATA",
    "BSC_MAINNET_LOCAL_ADMISSION_ADVERSARIAL_TESTS",
  ]),
});

const SOURCE_PARITY_SDK_SPECS_BY_PROFILE = Object.freeze({
  testnet: Object.freeze({
    javascript: Object.freeze({
      implementation: "pure-typescript",
      files: Object.freeze([
        Object.freeze({
          path: "javascript/iroha_js/src/sccp.js",
          markers: Object.freeze([
            "export function buildBscTestnetSccpLocalAdmissionSubmission",
            "SCCP_LOCAL_ADMISSION_ENVELOPE_ENCODING_V1",
            "SCCP_LOCAL_ADMISSION_SUBMISSION_KIND_V1",
            "SCCP_LOCAL_ADMISSION_ENTRYPOINT_V1",
          ]),
        }),
        Object.freeze({
          path: "javascript/iroha_js/test/sccpBscMainnet.test.js",
          markers: Object.freeze([
            "buildBscTestnetSccpLocalAdmissionSubmission(input)",
            "new BscTestnetSccp().buildLocalAdmissionSubmission(input)",
            "localAdmission.proofBytes",
          ]),
        }),
      ]),
    }),
    swift: Object.freeze({
      implementation: "native-swift",
      files: Object.freeze([
        Object.freeze({
          path: "IrohaSwift/Sources/IrohaSwift/SccpEvmProver.swift",
          markers: Object.freeze([
            "BscTestnetLocalAdmissionSubmissionInput",
            "buildBscTestnetSccpLocalAdmissionSubmission",
            "public final class BscTestnetSccp",
            "public func buildLocalAdmissionSubmission",
          ]),
        }),
        Object.freeze({
          path: "IrohaSwift/Tests/IrohaSwiftTests/SccpSolanaProverTests.swift",
          markers: Object.freeze([
            "testBscTestnetSccpBuildsLocalAdmissionSubmission",
            "XCTAssertThrowsError",
            "stark-fri-v1",
          ]),
        }),
      ]),
    }),
    kotlin: Object.freeze({
      implementation: "native-kotlin",
      files: Object.freeze([
        Object.freeze({
          path: "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/EvmSccpProver.kt",
          markers: Object.freeze([
            "BscTestnetLocalAdmissionSubmissionInput",
            "fun buildLocalAdmissionSubmission",
            "LOCAL_ADMISSION_ENVELOPE_ENCODING",
          ]),
        }),
        Object.freeze({
          path: "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/EvmSccpProverTest.kt",
          markers: Object.freeze([
            "bscTestnetFacadeBuildsLocalAdmissionSubmission",
            "assertFailsWith<IllegalArgumentException>",
            "evm-groth16-bn254-v1",
          ]),
        }),
      ]),
    }),
    "java-android": Object.freeze({
      implementation: "native-java",
      files: Object.freeze([
        Object.freeze({
          path: "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/BscTestnetSccpProver.java",
          markers: Object.freeze([
            "LocalAdmissionSubmissionInput",
            "buildLocalAdmissionSubmission",
            "LOCAL_ADMISSION_ENVELOPE_ENCODING",
          ]),
        }),
        Object.freeze({
          path: "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/EvmSccpProverTests.java",
          markers: Object.freeze([
            "bscTestnetFacadeBuildsLocalAdmissionSubmission",
            "catch (final IllegalArgumentException ex)",
            "evm-groth16-bn254-v1",
          ]),
        }),
      ]),
    }),
    dotnet: Object.freeze({
      implementation: "native-csharp",
      files: Object.freeze([
        Object.freeze({
          path: "csharp/src/Hyperledger.Iroha.Sdk/Sccp/BscTestnetSccp.cs",
          markers: Object.freeze([
            "BscTestnetLocalAdmissionSubmissionInput",
            "BuildLocalAdmissionSubmission",
            "LocalAdmissionEnvelopeEncoding",
          ]),
        }),
        Object.freeze({
          path: "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SccpBscTestnetTests.cs",
          markers: Object.freeze([
            "LocalAdmissionSubmissionWrapsNativeBscTestnetOutput",
            "Assert.Throws<ArgumentException>",
            "EvmGroth16Bn254ProofBackend",
          ]),
        }),
      ]),
    }),
  }),
  mainnet: Object.freeze({
    javascript: Object.freeze({
      implementation: "pure-typescript",
      files: Object.freeze([
        Object.freeze({
          path: "javascript/iroha_js/src/sccp.js",
          markers: Object.freeze([
            "export function buildBscMainnetSccpLocalAdmissionSubmission",
            "SCCP_LOCAL_ADMISSION_ENVELOPE_ENCODING_V1",
            "SCCP_LOCAL_ADMISSION_SUBMISSION_KIND_V1",
            "SCCP_LOCAL_ADMISSION_ENTRYPOINT_V1",
          ]),
        }),
        Object.freeze({
          path: "javascript/iroha_js/test/sccpBscMainnet.test.js",
          markers: Object.freeze([
            "buildBscMainnetSccpLocalAdmissionSubmission(input)",
            "const facadeSubmission = new BscMainnetSccp().buildLocalAdmissionSubmission(",
            "localAdmission.proofBytes",
          ]),
        }),
      ]),
    }),
    swift: Object.freeze({
      implementation: "native-swift",
      files: Object.freeze([
        Object.freeze({
          path: "IrohaSwift/Sources/IrohaSwift/SccpEvmProver.swift",
          markers: Object.freeze([
            "BscMainnetLocalAdmissionSubmissionInput",
            "buildBscMainnetSccpLocalAdmissionSubmission",
            "public final class BscMainnetSccp",
            "public func buildLocalAdmissionSubmission",
          ]),
        }),
        Object.freeze({
          path: "IrohaSwift/Tests/IrohaSwiftTests/SccpSolanaProverTests.swift",
          markers: Object.freeze([
            "testBscMainnetSccpBuildsLocalAdmissionSubmission",
            "XCTAssertThrowsError",
            "stark-fri-v1",
          ]),
        }),
      ]),
    }),
    kotlin: Object.freeze({
      implementation: "native-kotlin",
      files: Object.freeze([
        Object.freeze({
          path: "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/EvmSccpProver.kt",
          markers: Object.freeze([
            "BscMainnetLocalAdmissionSubmissionInput",
            "fun buildLocalAdmissionSubmission",
            "LOCAL_ADMISSION_ENVELOPE_ENCODING",
          ]),
        }),
        Object.freeze({
          path: "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp/EvmSccpProverTest.kt",
          markers: Object.freeze([
            "bscMainnetFacadeBuildsLocalAdmissionSubmission",
            "assertFailsWith<IllegalArgumentException>",
            "evm-groth16-bn254-v1",
          ]),
        }),
      ]),
    }),
    "java-android": Object.freeze({
      implementation: "native-java",
      files: Object.freeze([
        Object.freeze({
          path: "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/BscMainnetSccp.java",
          markers: Object.freeze([
            "LocalAdmissionSubmissionInput",
            "buildLocalAdmissionSubmission",
            "LOCAL_ADMISSION_ENVELOPE_ENCODING",
          ]),
        }),
        Object.freeze({
          path: "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp/EvmSccpProverTests.java",
          markers: Object.freeze([
            "bscMainnetFacadeBuildsLocalAdmissionSubmission",
            "catch (final IllegalArgumentException ex)",
            "evm-groth16-bn254-v1",
          ]),
        }),
      ]),
    }),
    dotnet: Object.freeze({
      implementation: "native-csharp",
      files: Object.freeze([
        Object.freeze({
          path: "csharp/src/Hyperledger.Iroha.Sdk/Sccp/BscMainnetSccp.cs",
          markers: Object.freeze([
            "BscMainnetLocalAdmissionSubmissionInput",
            "BuildLocalAdmissionSubmission",
            "LocalAdmissionEnvelopeEncoding",
          ]),
        }),
        Object.freeze({
          path: "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SccpBscMainnetTests.cs",
          markers: Object.freeze([
            "LocalAdmissionSubmissionWrapsNativeBscOutput",
            "Assert.Throws<ArgumentException>",
            "EvmGroth16Bn254ProofBackend",
          ]),
        }),
      ]),
    }),
  }),
});

const sourceParityRequiredMarkersForProfile = (profileKey = "testnet") => {
  const markers = SOURCE_PARITY_REQUIRED_MARKERS_BY_PROFILE[profileKey];
  if (!markers) {
    throw new Error(`unsupported BSC source-parity profile: ${profileKey}.`);
  }
  return markers;
};

const sourceParitySdkSpecsForProfile = (profileKey = "testnet") => {
  const specs = SOURCE_PARITY_SDK_SPECS_BY_PROFILE[profileKey];
  if (!specs) {
    throw new Error(`unsupported BSC source-parity profile: ${profileKey}.`);
  }
  return specs;
};

function repoPath(...segments) {
  return resolve(REPO_ROOT, ...segments);
}

function usage() {
  return `Usage:
	  node scripts/sccp_bsc_taira_xor_deploy.mjs compile [--out ${DEFAULT_ARTIFACTS_OUT}]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs deploy --bsc-network testnet|mainnet --verifier <verifier-key.json> --broadcast true --confirm-network ${ROUTE_ID}:testnet|${ROUTE_ID}:mainnet [--confirm-mainnet true] [--private-key-env ${DEFAULT_PRIVATE_KEY_ENV}] [--rpc-url ${DEFAULT_BSC_RPC_URL}] [--out ${DEFAULT_EVIDENCE_OUT}]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs evidence --bsc-network testnet|mainnet --token <addr> --bridge <addr> --source-bridge <addr> --verifier <addr> [--rpc-url ${DEFAULT_BSC_RPC_URL}] [--out ${DEFAULT_EVIDENCE_OUT}]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs route-manifest --evidence ${DEFAULT_EVIDENCE_OUT} --taira-contract ${DEFAULT_TAIRA_BURN_RECORD_CONTRACT_OUT} --settlement-asset-definition-id <asset-id> [--proof-artifact-hash <0x...> --proving-key-hash <0x...>] [--native-prover-bundle ${DEFAULT_NATIVE_EVM_PROVER_BUNDLE_OUT}] [--destination-browser-prover-manifest <destination-sidecar.json> --source-browser-prover-manifest <source-sidecar.json>] [--source-bridge-config-hash <0x...> --source-event-transaction-id <0x...> --source-event-explorer-url <url> --route-canary-evidence-hash <0x...> --route-canary-transaction-id <0x...> --route-canary-explorer-url <url> --full-toml-ready true --offline-full-toml-sha256 <0x...>|--offline-full-toml-evidence ${DEFAULT_ROUTE_FULL_CONFIG_EVIDENCE_OUT}] [--production-ready true --offline-full-toml-evidence ${DEFAULT_ROUTE_FULL_CONFIG_EVIDENCE_OUT} --live-readback-checked true --confirm-testnet ${ROUTE_ID}|--confirm-mainnet true --confirm-network ${ROUTE_ID}] [--out ${DEFAULT_ROUTE_MANIFEST_OUT}]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs source-parity-attestation [--bsc-network testnet|mainnet] [--out ${DEFAULT_NATIVE_EVM_SOURCE_PARITY_ATTESTATION_OUT}]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material generate --bsc-network testnet --ptau <phase2.ptau> [--out-dir output/sccp-bsc-production/groth16-material/testnet] [--circom-bin circom2] [--snarkjs-bin snarkjs]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material toolchain-fingerprint [--transcript <reproducible-build-transcript.json>] [--circom-bin circom2] [--snarkjs-bin snarkjs] [--out <json>]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material transcript-template --bsc-network testnet|mainnet --r1cs <file.r1cs> --zkey <file.zkey> --ptau <powersOfTau28_hez_final_22.ptau> --snarkjs-verifier-key <verification_key.json> [--circuit-source <full-message.circom>] [--witness-wasm <circuit.wasm>] [--circom-bin circom2] [--snarkjs-bin snarkjs] [--out-dir <transcript-dir>] [--overwrite true]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material materialize --bsc-network testnet|mainnet --r1cs <file.r1cs> --zkey <file.zkey> --ptau <powersOfTau28_hez_final_22.ptau> --snarkjs-verifier-key <verification_key.json> [--circuit-source <full-message.circom>] [--witness-wasm <circuit.wasm>] --trusted-setup-transcript <json> --reproducible-build-transcript <json> [--snarkjs-bin snarkjs] [--out-dir ${DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT}/testnet]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material proof-self-test --manifest <groth16-material.manifest.json> [--witness-wasm <circuit.wasm>] [--snarkjs-bin snarkjs] [--allow-unready-candidate true|--allow-unready-mainnet-candidate true] [--out <proof-self-test.json>]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material evidence-template --manifest <candidate-groth16-material.manifest.json> [--out-dir <review-evidence-dir>] [--overwrite true]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material attestation-request --manifest <candidate-groth16-material.manifest.json> --semantic-review-evidence <semantic-review-evidence.json> --circuit-security-audit-evidence <circuit-security-audit-evidence.json> [--out <attestation-request.json>]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material handoff-bundle --manifest <candidate-groth16-material.manifest.json> [--transcript-template-package <json>] [--evidence-template-package <json>] [--request <attestation-request.json>] [--out <handoff.json>]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material verify-handoff --handoff <handoff.json> [--trusted-attestation-signer <0x...>]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material sign-attestation --request <attestation-request.json> --role semanticSccpCircuit|circuitSecurity|trustedSetup|reproducibleBuild --private-key-pem <ed25519-private-key.pem> [--out <signed-role-attestation.json>]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material attestation-status --request <attestation-request.json> --semantic-attestation <json> --circuit-security-attestation <json> --trusted-setup-attestation <json> --reproducible-build-attestation <json> --trusted-attestation-signer <0x...>
	  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material attestation-inventory --request <attestation-request.json> --scan-dir <dir> --trusted-attestation-signer <0x...>
	  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material finalize-attestations --request <attestation-request.json> --semantic-attestation <json> --circuit-security-attestation <json> --trusted-setup-attestation <json> --reproducible-build-attestation <json> --trusted-attestation-signer <0x...> [--out-dir ${DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT}/testnet]
	  node scripts/sccp_bsc_taira_xor_deploy.mjs native-prover-bundle --route-manifest ${DEFAULT_ROUTE_MANIFEST_OUT} --artifact-root ${DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT} --proof-artifact <relative-file> --proving-key <relative-file> --verifier-key <relative-file> --groth16-material-manifest <relative-json> --groth16-proof-self-test <relative-json> --snarkjs-bin <snarkjs> --trusted-attestation-signer <0x...> --cross-sdk-parity <relative-json> --native-prover-self-test <relative-json> --javascript-implementation <relative-file> --swift-implementation <relative-file> --kotlin-implementation <relative-file> --java-android-implementation <relative-file> --dotnet-implementation <relative-file> --audit-circuit-security <hex-or-relative-file> --audit-native-implementation <hex-or-relative-file> --audit-reproducible-build <hex-or-relative-file> --audit-no-wasm-no-remote-scan <hex-or-relative-file> [--audit-cross-sdk-parity <matching-hex-or-relative-file>] [--audit-native-prover-self-test <matching-hex-or-relative-file>] [--out ${DEFAULT_NATIVE_EVM_PROVER_BUNDLE_OUT}] [--attach-route-manifest-out ${DEFAULT_ROUTE_MANIFEST_OUT}]
  node scripts/sccp_bsc_taira_xor_deploy.mjs publish-burn-record-vk [--route-manifest ${DEFAULT_ROUTE_MANIFEST_OUT}] [--vk-template ${DEFAULT_TAIRA_BURN_RECORD_VK_TEMPLATE}] [--name <vk-name>] [--out ${DEFAULT_TAIRA_BSC_BURN_RECORD_VK_ISI_OUT}] [--submit true --torii-url ${DEFAULT_TAIRA_TORII_URL} --chain-id ${DEFAULT_TAIRA_CHAIN_ID} --authority <account> --private-key-env ${DEFAULT_TAIRA_ROUTE_MANIFEST_PRIVATE_KEY_ENV} --gas-asset-id <asset-definition-id> --gas-limit ${DEFAULT_TAIRA_ROUTE_MANIFEST_GAS_LIMIT}] [--wait-for-commit true|false] [--commit-timeout-ms 120000]
  node scripts/sccp_bsc_taira_xor_deploy.mjs publish-route-manifest [--manifest ${DEFAULT_ROUTE_MANIFEST_OUT}] [--out ${DEFAULT_ROUTE_MANIFEST_ISI_OUT}] [--submit true --torii-url ${DEFAULT_TAIRA_TORII_URL} --chain-id ${DEFAULT_TAIRA_CHAIN_ID} --authority <account> --private-key-env ${DEFAULT_TAIRA_ROUTE_MANIFEST_PRIVATE_KEY_ENV} --gas-asset-id <asset-definition-id> --gas-limit ${DEFAULT_TAIRA_ROUTE_MANIFEST_GAS_LIMIT}] [--wait-for-commit true|false] [--commit-timeout-ms 120000]
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

const COMMAND_HELP = Object.freeze({
  compile: `Usage:
  node scripts/sccp_bsc_taira_xor_deploy.mjs compile [--out ${DEFAULT_ARTIFACTS_OUT}]

Compiles the BSC SCCP contracts into public artifacts. Requires solc on
NODE_PATH. This command does not broadcast transactions.`,
  deploy: `Usage:
  node scripts/sccp_bsc_taira_xor_deploy.mjs deploy --bsc-network testnet|mainnet --verifier <verifier-key.json> --broadcast true --confirm-network ${ROUTE_ID}:testnet|${ROUTE_ID}:mainnet [--confirm-mainnet true] [--private-key-env ${DEFAULT_PRIVATE_KEY_ENV}] [--rpc-url ${DEFAULT_BSC_RPC_URL}] [--out ${DEFAULT_EVIDENCE_OUT}]

Deploys the BSC SCCP token, bridge, source bridge, and verifier contracts and
writes public deployment evidence. The deployer key is read only from the named
environment variable at runtime and is never written to disk. Diagnostic
verifier material is refused unless explicitly allowed for non-production use.`,
  evidence: `Usage:
  node scripts/sccp_bsc_taira_xor_deploy.mjs evidence --bsc-network testnet|mainnet --token <addr> --bridge <addr> --source-bridge <addr> --verifier <addr> [--rpc-url ${DEFAULT_BSC_RPC_URL}] [--out ${DEFAULT_EVIDENCE_OUT}]

Reads deployed BSC contracts and writes public deployment evidence. The
readback must bind the token, bridge, source bridge, verifier, network id,
verifier key hash, and destination binding hash to the selected BSC profile.`,
  "route-manifest": `Usage:
  node scripts/sccp_bsc_taira_xor_deploy.mjs route-manifest --evidence ${DEFAULT_EVIDENCE_OUT} --taira-contract ${DEFAULT_TAIRA_BURN_RECORD_CONTRACT_OUT} --settlement-asset-definition-id <asset-id> [--proof-artifact-hash <0x...> --proving-key-hash <0x...>] [--native-prover-bundle ${DEFAULT_NATIVE_EVM_PROVER_BUNDLE_OUT}] [--destination-browser-prover-manifest <destination-sidecar.json> --source-browser-prover-manifest <source-sidecar.json>] [--source-bridge-config-hash <0x...> --source-event-transaction-id <0x...> --source-event-explorer-url <url> --route-canary-evidence-hash <0x...> --route-canary-transaction-id <0x...> --route-canary-explorer-url <url> --full-toml-ready true --offline-full-toml-sha256 <0x...>|--offline-full-toml-evidence ${DEFAULT_ROUTE_FULL_CONFIG_EVIDENCE_OUT}] [--production-ready true --offline-full-toml-evidence ${DEFAULT_ROUTE_FULL_CONFIG_EVIDENCE_OUT} --live-readback-checked true --confirm-testnet ${ROUTE_ID}|--confirm-mainnet true --confirm-network ${ROUTE_ID}] [--out ${DEFAULT_ROUTE_MANIFEST_OUT}]

Builds the TAIRA/BSC SCCP route manifest from public deployment evidence,
TAIRA burn-record contract material, production proof hashes, native prover
bundle attestations, browser prover sidecar references, and post-deploy live
evidence. Canonical production output is refused unless the manifest is
productionReady true and free of diagnostic or placeholder material.`,
  "native-prover-bundle": `Usage:
  node scripts/sccp_bsc_taira_xor_deploy.mjs native-prover-bundle --route-manifest ${DEFAULT_ROUTE_MANIFEST_OUT} --artifact-root ${DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT} --proof-artifact <relative-file> --proving-key <relative-file> --verifier-key <relative-file> --groth16-material-manifest <relative-json> --groth16-proof-self-test <relative-json> --snarkjs-bin <snarkjs> --trusted-attestation-signer <0x...> --cross-sdk-parity <relative-json> --native-prover-self-test <relative-json> --javascript-implementation <relative-file> --swift-implementation <relative-file> --kotlin-implementation <relative-file> --java-android-implementation <relative-file> --dotnet-implementation <relative-file> --audit-circuit-security <hex-or-relative-file> --audit-native-implementation <hex-or-relative-file> --audit-reproducible-build <hex-or-relative-file> --audit-no-wasm-no-remote-scan <hex-or-relative-file> [--audit-cross-sdk-parity <matching-hex-or-relative-file>] [--audit-native-prover-self-test <matching-hex-or-relative-file>] [--out ${DEFAULT_NATIVE_EVM_PROVER_BUNDLE_OUT}] [--attach-route-manifest-out ${DEFAULT_ROUTE_MANIFEST_OUT}]

Builds the SDK-validated native EVM prover bundle from production proof
artifacts, keys, implementation files, Groth16 proof self-test evidence,
parity/self-test evidence, and signed audit attestations. This independently
verifies Groth16 material attestations against --trusted-attestation-signer and
requires --groth16-proof-self-test to bind a successful SnarkJS prove/verify
report with adversarial witness rejection evidence to the same productionReady
material manifest, then reruns --snarkjs-bin groth16 verify against the embedded
proof and signed SnarkJS verification key before a productionReady BSC route
manifest can be emitted.`,
  "source-parity-attestation": `Usage:
  node scripts/sccp_bsc_taira_xor_deploy.mjs source-parity-attestation [--bsc-network testnet|mainnet] [--out ${DEFAULT_NATIVE_EVM_SOURCE_PARITY_ATTESTATION_OUT}]

Builds a deterministic source-parity attestation from the JavaScript, Swift,
Kotlin, Java Android, and .NET BSC testnet local-admission implementation and
negative-test surfaces. The report is public and contains only file hashes and
marker evidence.`,
  "groth16-material": `Usage:
  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material generate --bsc-network testnet --ptau <phase2.ptau> [--out-dir output/sccp-bsc-production/groth16-material/testnet] [--circom-bin circom2] [--snarkjs-bin snarkjs]
  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material generate --bsc-network testnet --create-local-ptau-power 8 --allow-local-testnet-setup true [--out-dir output/sccp-bsc-production/groth16-material/testnet]
  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material toolchain-fingerprint [--transcript <reproducible-build-transcript.json>] [--circom-bin circom2] [--snarkjs-bin snarkjs] [--out <json>]
  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material transcript-template --bsc-network testnet|mainnet --r1cs <file.r1cs> --zkey <file.zkey> --ptau <powersOfTau28_hez_final_22.ptau> --snarkjs-verifier-key <verification_key.json> [--circuit-source <full-message.circom>] [--witness-wasm <circuit.wasm>] [--circom-bin circom2] [--snarkjs-bin snarkjs] [--out-dir <transcript-dir>] [--overwrite true]
  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material materialize --bsc-network testnet|mainnet --r1cs <file.r1cs> --zkey <file.zkey> --ptau <powersOfTau28_hez_final_22.ptau> --snarkjs-verifier-key <verification_key.json> [--circuit-source <full-message.circom>] [--witness-wasm <circuit.wasm>] --trusted-setup-transcript <json> --reproducible-build-transcript <json> [--snarkjs-bin snarkjs] [--out-dir ${DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT}/testnet]
  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material proof-self-test --manifest <groth16-material.manifest.json> [--witness-wasm <circuit.wasm>] [--snarkjs-bin snarkjs] [--allow-unready-candidate true|--allow-unready-mainnet-candidate true] [--out <proof-self-test.json>]
  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material evidence-template --manifest <candidate-groth16-material.manifest.json> [--out-dir <review-evidence-dir>] [--overwrite true]
  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material attestation-request --manifest <candidate-groth16-material.manifest.json> --semantic-review-evidence <semantic-review-evidence.json> --circuit-security-audit-evidence <circuit-security-audit-evidence.json> [--out <attestation-request.json>]
  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material handoff-bundle --manifest <candidate-groth16-material.manifest.json> [--transcript-template-package <json>] [--evidence-template-package <json>] [--request <attestation-request.json>] [--out <handoff.json>]
  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material verify-handoff --handoff <handoff.json> [--trusted-attestation-signer <0x...>]
  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material sign-attestation --request <attestation-request.json> --role semanticSccpCircuit|circuitSecurity|trustedSetup|reproducibleBuild --private-key-pem <ed25519-private-key.pem> [--out <signed-role-attestation.json>]
  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material attestation-status --request <attestation-request.json> --semantic-attestation <json> --circuit-security-attestation <json> --trusted-setup-attestation <json> --reproducible-build-attestation <json> --trusted-attestation-signer <0x...>
  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material attestation-inventory --request <attestation-request.json> --scan-dir <dir> --trusted-attestation-signer <0x...>
  node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material finalize-attestations --request <attestation-request.json> --semantic-attestation <json> --circuit-security-attestation <json> --trusted-setup-attestation <json> --reproducible-build-attestation <json> --trusted-attestation-signer <0x...> [--out-dir ${DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT}/testnet]

Generates real Circom/SnarkJS Groth16 candidate material or materializes
externally audited production circuit/proving/verifier material into the BSC
native-prover artifact shape. When --circuit-source is omitted, materialize uses
${DEFAULT_BSC_FULL_MESSAGE_CIRCUIT_SOURCE}. toolchain-fingerprint writes actual
Circom/SnarkJS executable hashes into a public transcript copy. Materialize
consumes transcript-template draft outputs only after external ceremony and
rebuild fields have been replaced with production evidence. Materialize
creates unsigned candidate material; attestation-request emits role-separated payload hashes;
evidence-template creates manifest-bound public review/audit drafts that remain
not signable until independent reviewers replace pending fields with real pass
evidence; handoff-bundle records one public hash-bound operator packet for
external review/signing without changing readiness; verify-handoff re-hashes
that packet's referenced files and reruns attestation status without writing
outputs; sign-attestation signs exactly one ready role body with an Ed25519
private key file without writing secrets; attestation-status audits request,
transcript, signature, and
trusted-signer readiness without writing outputs; and
finalize-attestations refuses productionReady output unless every signed
attestation matches the request package and trusted signer policy.
proof-self-test requires productionReady material by default;
--allow-unready-candidate true only refreshes testnet candidate evidence, while
--allow-unready-mainnet-candidate true is the separate explicit opt-in for
mainnet candidate evidence. Neither mode makes candidate material
production-ready.`,
  "route-config": `Usage:
  node scripts/sccp_bsc_taira_xor_deploy.mjs route-config [--manifest ${DEFAULT_ROUTE_MANIFEST_OUT}] [--allow-unready true|false] [--base-config configs/soranexus/taira/config.toml] [--out ${DEFAULT_ROUTE_CONFIG_OUT}] [--write-offline-full-toml-evidence ${DEFAULT_ROUTE_FULL_CONFIG_EVIDENCE_OUT}]

Renders a legacy/offline TAIRA route TOML overlay or merged full config for
evidence hashing only. Production route material is published on-chain with
publish-route-manifest; peer configs should not carry a local BSC SCCP route
stanza. Production-ready manifests must not use --allow-unready; draft
manifests must opt in to --allow-unready true and cannot be written as
canonical production material.`,
  "publish-route-manifest": `Usage:
  node scripts/sccp_bsc_taira_xor_deploy.mjs publish-route-manifest [--manifest ${DEFAULT_ROUTE_MANIFEST_OUT}] [--out ${DEFAULT_ROUTE_MANIFEST_ISI_OUT}] [--submit true --torii-url ${DEFAULT_TAIRA_TORII_URL} --chain-id ${DEFAULT_TAIRA_CHAIN_ID} --authority <account> --private-key-env ${DEFAULT_TAIRA_ROUTE_MANIFEST_PRIVATE_KEY_ENV} --gas-asset-id <asset-definition-id> --gas-limit ${DEFAULT_TAIRA_ROUTE_MANIFEST_GAS_LIMIT}] [--wait-for-commit true|false] [--commit-timeout-ms 120000]

Builds the UpsertSccpRouteManifest ISI payload from a production BSC route
manifest. By default the command writes the public ISI artifact without
broadcasting. With --submit true it signs and submits the ISI to TAIRA using an
operator account that holds CanManageSccpRouteManifests.`,
  "publish-burn-record-vk": `Usage:
  node scripts/sccp_bsc_taira_xor_deploy.mjs publish-burn-record-vk [--route-manifest ${DEFAULT_ROUTE_MANIFEST_OUT}] [--vk-template ${DEFAULT_TAIRA_BURN_RECORD_VK_TEMPLATE}] [--name <vk-name>] [--out ${DEFAULT_TAIRA_BSC_BURN_RECORD_VK_ISI_OUT}] [--submit true --torii-url ${DEFAULT_TAIRA_TORII_URL} --chain-id ${DEFAULT_TAIRA_CHAIN_ID} --authority <account> --private-key-env ${DEFAULT_TAIRA_ROUTE_MANIFEST_PRIVATE_KEY_ENV} --gas-asset-id <asset-definition-id> --gas-limit ${DEFAULT_TAIRA_ROUTE_MANIFEST_GAS_LIMIT}] [--wait-for-commit true|false] [--commit-timeout-ms 120000]

Builds and optionally submits the RegisterVerifyingKey ISI for the TAIRA
burn-record IVM verifier used by the BSC route. The VK backend/name default to
the route manifest's tairaXorBurnRecord.vkRef, while the public verifier bytes,
commitment, circuit id, and schema hash come from the canonical VK template.
With --submit true it signs and submits the ISI to TAIRA using an operator
account that holds CanManageVerifyingKeys.`,
  requirements: `Usage:
  node scripts/sccp_bsc_taira_xor_deploy.mjs requirements [--bsc-network testnet|mainnet] [--out ${DEFAULT_PRODUCTION_REQUIREMENTS_OUT}]

Prints or writes the public BSC SCCP production handoff requirements,
including required artifacts, reports, commands, and denied diagnostic verifier
key hashes.`,
  "self-test": `Usage:
  node scripts/sccp_bsc_taira_xor_deploy.mjs self-test

Runs local invariant checks for public deployment evidence and secret scanning.`,
});

function commandUsage(command) {
  return COMMAND_HELP[command] ?? usage();
}

const isHelpToken = (token) =>
  token === "--help" || token === "-h" || token === "help";

const trim = (value) => String(value ?? "").trim();

function parseArgs(argv) {
  const args = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!token.startsWith("--")) {
      throw new Error(`Unexpected argument: ${token}`);
    }
    const key = token.slice(2);
    if (hasOwn(args, key)) {
      throw new Error(`Duplicate option: --${key}`);
    }
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
  const sourceParityOut = defaultNativeEvmSourceParityAttestationOut(profile);
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
      sourceParityAttestation:
        `node scripts/sccp_bsc_taira_xor_deploy.mjs source-parity-attestation --bsc-network ${profile.key} ` +
        `--out ${sourceParityOut}`,
      groth16ToolchainFingerprint:
        "node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material toolchain-fingerprint " +
        "--transcript <reproducible-build-transcript.json> " +
        "--circom-bin <circom> " +
        "--snarkjs-bin <snarkjs> " +
        "--out <reproducible-build-transcript.with-toolchain-hashes.json>",
      groth16TranscriptTemplate:
        "node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material transcript-template " +
        `--bsc-network ${profile.key} ` +
        "--r1cs <production-circuit.r1cs> " +
        "--zkey <production-proving-key.zkey> " +
        "--ptau <powersOfTau28_hez_final_22.ptau> " +
        "--snarkjs-verifier-key <production-verification_key.json> " +
        "[--circuit-source <production-full-message.circom>] " +
        "--witness-wasm <production-circuit.wasm> " +
        "--circom-bin <circom> " +
        "--snarkjs-bin <snarkjs> " +
        "--out-dir <transcript-dir>",
      groth16Material:
        "node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material materialize " +
        `--bsc-network ${profile.key} ` +
        "--r1cs <production-circuit.r1cs> " +
        "--zkey <production-proving-key.zkey> " +
        "--ptau <powersOfTau28_hez_final_22.ptau> " +
        "--snarkjs-verifier-key <production-verification_key.json> " +
        "[--circuit-source <production-full-message.circom>] " +
        "--witness-wasm <production-circuit.wasm> " +
        "--trusted-setup-transcript <trusted-setup-transcript.json> " +
        "--reproducible-build-transcript <reproducible-build-transcript.json> " +
        "--snarkjs-bin <snarkjs> " +
        `--out-dir ${DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT}/${profile.key}`,
      groth16AttestationRequest:
        "node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material attestation-request " +
        "--manifest <candidate-groth16-material.manifest.json> " +
        "--semantic-review-evidence <semantic-review-evidence.json> " +
        "--circuit-security-audit-evidence <circuit-security-audit-evidence.json> " +
        "--out <attestation-request.json>",
      groth16AttestationHandoff:
        "node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material handoff-bundle " +
        "--manifest <candidate-groth16-material.manifest.json> " +
        "--transcript-template-package <transcript-template-package.json> " +
        "--evidence-template-package <evidence-template-package.json> " +
        "--request <attestation-request.json> " +
        "--out <handoff.json>",
      groth16VerifyHandoff:
        "node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material verify-handoff " +
        "--handoff <handoff.json> " +
        "--trusted-attestation-signer <0x...>",
      groth16EvidenceTemplate:
        "node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material evidence-template " +
        "--manifest <candidate-groth16-material.manifest.json> " +
        "--out-dir <review-evidence-dir>",
      groth16SignAttestation:
        "node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material sign-attestation " +
        "--request <attestation-request.json> " +
        "--role semanticSccpCircuit|circuitSecurity|trustedSetup|reproducibleBuild " +
        "--private-key-pem <ed25519-private-key.pem> " +
        "--out <signed-role-attestation.json>",
      groth16AttestationStatus:
        "node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material attestation-status " +
        "--request <attestation-request.json> " +
        "--semantic-attestation <semantic-sccp-circuit-attestation.json> " +
        "--circuit-security-attestation <circuit-security-audit.json> " +
        "--trusted-setup-attestation <trusted-setup-ceremony.json> " +
        "--reproducible-build-attestation <reproducible-build-attestation.json> " +
        "--trusted-attestation-signer <0x...>",
      groth16AttestationInventory:
        "node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material attestation-inventory " +
        "--request <attestation-request.json> " +
        "--scan-dir <native-prover-artifact-root> " +
        "--trusted-attestation-signer <0x...>",
      groth16ProofSelfTest:
        "node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material proof-self-test " +
        "--manifest <production-ready-groth16-material.manifest.json> " +
        "--witness-wasm <production-circuit.wasm> " +
        "--snarkjs-bin <snarkjs> " +
        "--out <proof-self-test.json>",
      groth16FinalizeAttestations:
        "node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material finalize-attestations " +
        "--request <attestation-request.json> " +
        "--semantic-attestation <semantic-sccp-circuit-attestation.json> " +
        "--circuit-security-attestation <circuit-security-audit.json> " +
        "--trusted-setup-attestation <trusted-setup-ceremony.json> " +
        "--reproducible-build-attestation <reproducible-build-attestation.json> " +
        "--trusted-attestation-signer <0x...> " +
        `--out-dir ${DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT}/${profile.key}`,
      nativeProverBundle:
        "node scripts/sccp_bsc_taira_xor_deploy.mjs native-prover-bundle " +
        `--route-manifest ${routeManifestOut} ` +
        `--artifact-root ${DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT} ` +
        "--proof-artifact <relative-circuit.r1cs> " +
        "--proving-key <relative-circuit.zkey> " +
        "--verifier-key <relative-verifier-key.json> " +
        "--groth16-material-manifest <relative-groth16-material-manifest.json> " +
        "--groth16-proof-self-test <relative-groth16-proof-self-test.json> " +
        "--snarkjs-bin <snarkjs> " +
        "--trusted-attestation-signer <0x...> " +
        "--cross-sdk-parity <relative-cross-sdk-parity.json> " +
        "--native-prover-self-test <relative-native-self-test.json> " +
        "--javascript-implementation <relative-js-implementation> " +
        "--swift-implementation <relative-swift-implementation> " +
        "--kotlin-implementation <relative-kotlin-implementation> " +
        "--java-android-implementation <relative-java-android-implementation> " +
        "--dotnet-implementation <relative-dotnet-implementation> " +
        "--audit-circuit-security <hex-or-relative-file> " +
        "--audit-native-implementation source-parity-attestation.json " +
        "--audit-reproducible-build <hex-or-relative-file> " +
        "--audit-no-wasm-no-remote-scan <hex-or-relative-file> " +
        `--out ${nativeBundleOut} ` +
        `--attach-route-manifest-out ${routeManifestOut}`,
      publishRouteManifest:
        "node scripts/sccp_bsc_taira_xor_deploy.mjs publish-route-manifest " +
        `--manifest ${routeManifestOut} ` +
        `--out ${DEFAULT_ROUTE_MANIFEST_ISI_OUT} ` +
        `--submit true --torii-url ${DEFAULT_TAIRA_TORII_URL} ` +
        `--chain-id ${DEFAULT_TAIRA_CHAIN_ID} ` +
        "--authority <taira-route-manifest-manager-account> " +
        `--private-key-env ${DEFAULT_TAIRA_ROUTE_MANIFEST_PRIVATE_KEY_ENV} ` +
        `--gas-limit ${DEFAULT_TAIRA_ROUTE_MANIFEST_GAS_LIMIT}`,
      publishBurnRecordVk:
        "node scripts/sccp_bsc_taira_xor_deploy.mjs publish-burn-record-vk " +
        `--route-manifest ${routeManifestOut} ` +
        `--vk-template ${DEFAULT_TAIRA_BURN_RECORD_VK_TEMPLATE} ` +
        `--out ${DEFAULT_TAIRA_BSC_BURN_RECORD_VK_ISI_OUT} ` +
        `--submit true --torii-url ${DEFAULT_TAIRA_TORII_URL} ` +
        `--chain-id ${DEFAULT_TAIRA_CHAIN_ID} ` +
        "--authority <taira-verifying-key-manager-account> " +
        `--private-key-env ${DEFAULT_TAIRA_ROUTE_MANIFEST_PRIVATE_KEY_ENV} ` +
        `--gas-limit ${DEFAULT_TAIRA_ROUTE_MANIFEST_GAS_LIMIT}`,
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
        requiredBy: [
          "native-prover-bundle",
          "route-config",
          "publish-route-manifest",
        ],
        description:
          "Production route manifest bound to BSC deployment evidence, browser prover references, TAIRA route publication, and live canary evidence.",
      }),
      productionRequirementInput({
        id: "destination-browser-prover-manifest",
        kind: "file-or-url",
        placeholder: "<destination-browser-prover-manifest.json>",
        requiredBy: ["route-manifest"],
        description:
          "Route-bound TAIRA-to-BSC browser prover sidecar manifest; the route manifest publishes only its URL/specifier, module hash, sidecar hash, expected exports, and bound route/proof hashes.",
      }),
      productionRequirementInput({
        id: "source-browser-prover-manifest",
        kind: "file-or-url",
        placeholder: "<source-browser-prover-manifest.json>",
        requiredBy: ["route-manifest"],
        description:
          "Route-bound BSC-to-TAIRA browser prover sidecar manifest; the route manifest publishes only its URL/specifier, module hash, sidecar hash, expected exports, and bound route/proof hashes.",
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
          "Deployed TAIRA base config used only to render legacy/offline full-TOML evidence; production SCCP BSC route material is published on-chain.",
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
        id: "native-prover-snarkjs-verifier",
        kind: "tool",
        placeholder: "<snarkjs>",
        requiredBy: [
          "groth16-toolchain-fingerprint",
          "groth16-material",
          "groth16-proof-self-test",
          "native-prover-bundle",
        ],
        description:
          "SnarkJS executable whose bytes are fingerprinted for reproducible-build evidence and used by materialize, proof-self-test, and native-prover-bundle to verify Groth16 material.",
      }),
      productionRequirementInput({
        id: "groth16-circom-compiler",
        kind: "tool",
        placeholder: "<circom>",
        requiredBy: ["groth16-toolchain-fingerprint"],
        description:
          "Circom executable whose bytes are fingerprinted into the reproducible-build transcript toolchain evidence.",
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
        id: "groth16-powers-of-tau",
        kind: "file",
        placeholder: "<powersOfTau28_hez_final_22.ptau>",
        requiredBy: ["groth16-material"],
        description:
          "Powers-of-Tau transcript passed to SnarkJS zkey verification; its sha256 is embedded in the Groth16 material manifest and every role attestation payload.",
      }),
      productionRequirementInput({
        id: "groth16-witness-wasm",
        kind: "file",
        placeholder: "<production-circuit.wasm>",
        requiredBy: ["groth16-material", "groth16-proof-self-test"],
        description:
          "Witness WASM artifact bound into the Groth16 material manifest and proof self-test report for reproducible-build traceability.",
      }),
      productionRequirementInput({
        id: "groth16-material-manifest",
        kind: "file",
        placeholder: "<relative-groth16-material-manifest.json>",
        requiredBy: ["native-prover-bundle"],
        description:
          "ProductionReady Groth16 material manifest generated by groth16-material finalize-attestations and bound to the proof artifact, proving key, verifier key, transcript artifacts, semantic attestation, trusted setup, and reproducible build evidence.",
      }),
      productionRequirementInput({
        id: "candidate-groth16-material-manifest",
        kind: "file",
        placeholder: "<candidate-groth16-material.manifest.json>",
        requiredBy: ["groth16-attestation-request"],
        description:
          "Unsigned candidate Groth16 material manifest emitted by materialize and hashed into the attestation request package.",
      }),
      productionRequirementInput({
        id: "groth16-attestation-request-package",
        kind: "file",
        placeholder: "<attestation-request.json>",
        requiredBy: [
          "groth16-sign-attestation",
          "groth16-attestation-status",
          "groth16-finalize-attestations",
        ],
        description:
          "Role-separated attestation request package whose signedPayloadSha256 values must match the semantic, security, setup, and reproducible-build signatures.",
      }),
      productionRequirementInput({
        id: "signed-groth16-role-attestations",
        kind: "file-set",
        placeholder:
          "<semantic-sccp-circuit-attestation.json>,<circuit-security-audit.json>,<trusted-setup-ceremony.json>,<reproducible-build-attestation.json>",
        requiredBy: [
          "groth16-attestation-status",
          "groth16-finalize-attestations",
          "native-prover-bundle",
        ],
        description:
          "Four public Ed25519-signed role attestation files produced from the request package; each signed body must match its request role payload hash and use a trusted, role-separated signer fingerprint.",
      }),
      productionRequirementInput({
        id: "groth16-proof-self-test-report",
        kind: "file",
        placeholder: "<proof-self-test.json>",
        requiredBy: [
          "groth16-proof-self-test",
          "native-prover-bundle",
          "production-material-preflight",
        ],
        description:
          "Public SnarkJS wtns/prove/verify report generated from the productionReady manifest-bound full-message circuit, proving key, verifier key, and deterministic synthetic SCCP witness.",
      }),
      productionRequirementInput({
        id: "trusted-groth16-attestation-signer",
        kind: "hex-fingerprint",
        placeholder: "<0x...>",
        requiredBy: [
          "groth16-attestation-status",
          "groth16-finalize-attestations",
          "native-prover-bundle",
        ],
        description:
          "Trusted Ed25519 public-key fingerprint used to verify detached signatures on Groth16 semantic, security, setup, and reproducible-build attestations.",
      }),
      productionRequirementInput({
        id: "trusted-setup-transcript",
        kind: "file",
        placeholder: "<trusted-setup-transcript.json>",
        requiredBy: ["groth16-material"],
        description:
          "Concrete trusted-setup ceremony transcript whose sha256 must match the trusted setup attestation contributionTranscriptSha256.",
      }),
      productionRequirementInput({
        id: "reproducible-build-transcript",
        kind: "file",
        placeholder: "<reproducible-build-transcript.json>",
        requiredBy: ["groth16-material"],
        description:
          "Concrete reproducible-build transcript whose sha256 must match the reproducible build attestation buildTranscriptSha256.",
      }),
      productionRequirementInput({
        id: "semantic-sccp-circuit-attestation",
        kind: "file",
        placeholder: "<semantic-sccp-circuit-attestation.json>",
        requiredBy: ["groth16-finalize-attestations", "native-prover-bundle"],
        description:
          "Public attestation that the Groth16 circuit enforces the full SCCP message, finality, route, and destination-binding semantics, not only the 9 public signal shape.",
      }),
      productionRequirementInput({
        id: "trusted-setup-ceremony-attestation",
        kind: "file",
        placeholder: "<trusted-setup-ceremony.json>",
        requiredBy: ["groth16-finalize-attestations", "native-prover-bundle"],
        description:
          "Public ceremony evidence binding the ptau, phase2 zkey, circuit hash, contribution transcript, and verifier key hash.",
      }),
      productionRequirementInput({
        id: "reproducible-groth16-build-attestation",
        kind: "file",
        placeholder: "<reproducible-build-attestation.json>",
        requiredBy: ["groth16-finalize-attestations", "native-prover-bundle"],
        description:
          "Independent reproducible build evidence for the circuit source, R1CS, proving key, SnarkJS verification key, and BSC verifier-key JSON.",
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
      productionRequirementInput({
        id: "source-parity-attestation",
        kind: "file",
        placeholder: sourceParityOut,
        requiredBy: ["native-prover-bundle"],
        description:
          "Deterministic source-parity attestation for JavaScript, Swift, Kotlin, Java Android, and .NET BSC local-admission implementations.",
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
        id: "taira-route-manifest-manager",
        kind: "operator-environment",
        placeholder: "<taira-route-manifest-manager-account-and-key-env>",
        requiredBy: ["publish-route-manifest"],
        description:
          "TAIRA account with CanManageSccpRouteManifests plus an operator key provided only through the named environment variable.",
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

const defaultNativeEvmSourceParityAttestationOut = (profile) =>
  profile.key === "mainnet"
    ? "artifacts/sccp-bsc/native-prover/mainnet-source-parity-attestation.json"
    : DEFAULT_NATIVE_EVM_SOURCE_PARITY_ATTESTATION_OUT;

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
    ? parseBscMainnetNativeEvmProverParityReport(fixture, descriptor)
    : parseBscTestnetNativeEvmProverParityReport(fixture, descriptor);

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

function canonicalJson(value) {
  if (value === null) return "null";
  if (typeof value === "string") return JSON.stringify(value);
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new Error("canonical JSON number must be finite");
    }
    return JSON.stringify(value);
  }
  if (typeof value === "boolean") return value ? "true" : "false";
  if (Array.isArray(value)) {
    return `[${value.map((entry) => canonicalJson(entry)).join(",")}]`;
  }
  if (isRecord(value)) {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`)
      .join(",")}}`;
  }
  throw new Error("canonical JSON supports only JSON values");
}

function attestationSignedBody(record) {
  if (!isRecord(record)) {
    return record;
  }
  const { signature: _signature, signatures: _signatures, ...body } = record;
  return body;
}

function attestationSignaturePayload(record) {
  return Buffer.from(canonicalJson(attestationSignedBody(record)), "utf8");
}

function sha256HexBytes(bytes) {
  return bytesToHex(sha256(new Uint8Array(bytes)));
}

function parseTrustedAttestationSignerFingerprints(options = {}) {
  const raw = [
    optionValue(options, "trusted-attestation-signer"),
    optionValue(options, "trusted-attestation-signer-fingerprint"),
    optionValue(options, "trusted-attestation-signers"),
    process.env.SCCP_BSC_TRUSTED_ATTESTATION_SIGNERS,
  ]
    .filter((value) => value !== undefined && value !== null && trim(value) !== "")
    .flatMap((value) => String(value).split(/[,\s]+/u))
    .map((value) => trim(value))
    .filter(Boolean);
  return [...new Set(raw.map((value) => normalizeHex32(value, "trusted attestation signer fingerprint")))];
}

function publicKeyFingerprint(publicKeyPem, label) {
  const publicKey = createPublicKey(String(publicKeyPem));
  const der = publicKey.export({ format: "der", type: "spki" });
  return { publicKey, fingerprint: sha256HexBytes(der) };
}

function attestationSignatureBytes(value, label) {
  if (typeof value !== "string" || trim(value) === "") {
    throw new Error(`${label} is required`);
  }
  const normalized = trim(value);
  if (/^0x[0-9a-f]+$/iu.test(normalized)) {
    const hex = normalized.slice(2);
    if (hex.length % 2 !== 0) {
      throw new Error(`${label} hex must have an even number of digits`);
    }
    return Buffer.from(hex, "hex");
  }
  return Buffer.from(normalized, "base64");
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

function normalizePrivateKeyEnvName(value = DEFAULT_PRIVATE_KEY_ENV) {
  const raw = String(value ?? "");
  const normalized = trim(raw);
  if (
    !normalized ||
    normalized !== raw ||
    !/^[A-Z_][A-Z0-9_]{0,127}$/u.test(normalized)
  ) {
    throw new Error(
      "--private-key-env must be an uppercase environment variable name containing only letters, digits, and underscores.",
    );
  }
  return normalized;
}

function normalizeTairaPrivateKeyEnvName(
  value = DEFAULT_TAIRA_ROUTE_MANIFEST_PRIVATE_KEY_ENV,
) {
  const raw = String(value ?? "");
  const normalized = trim(raw);
  if (
    !normalized ||
    normalized !== raw ||
    !/^[A-Z_][A-Z0-9_]{0,127}$/u.test(normalized)
  ) {
    throw new Error(
      "--private-key-env must be an uppercase environment variable name containing only letters, digits, and underscores.",
    );
  }
  return normalized;
}

function normalizeTairaToriiUrl(value = DEFAULT_TAIRA_TORII_URL) {
  const text = trim(value) || DEFAULT_TAIRA_TORII_URL;
  let url;
  try {
    url = new URL(text);
  } catch (_error) {
    throw new Error("--torii-url must be a valid URL.");
  }
  const isLoopback = ["localhost", "127.0.0.1", "::1"].includes(url.hostname);
  if (url.protocol !== "https:" && !(isLoopback && url.protocol === "http:")) {
    throw new Error("--torii-url must use HTTPS unless it is loopback HTTP.");
  }
  if (url.username || url.password || url.search || url.hash) {
    throw new Error(
      "--torii-url must not contain credentials, query strings, or fragments.",
    );
  }
  return url.toString().replace(/\/$/u, "");
}

function normalizeTairaChainId(value = DEFAULT_TAIRA_CHAIN_ID) {
  const text = trim(value) || DEFAULT_TAIRA_CHAIN_ID;
  if (
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u.test(
      text,
    )
  ) {
    throw new Error("--chain-id must be a canonical lowercase UUID.");
  }
  return text;
}

function normalizeBrowserProverModuleUrl(value, label) {
  const text = normalizeNonEmptyText(value, label);
  if (/[\u0000-\u001f\u007f]/u.test(text)) {
    throw new Error(`${label} contains control characters.`);
  }
  if (/^[a-z][a-z0-9+.-]*:/iu.test(text)) {
    let url;
    try {
      url = new URL(text);
    } catch (_error) {
      throw new Error(`${label} must be a valid URL.`);
    }
    const isLoopback = ["localhost", "127.0.0.1", "::1"].includes(
      url.hostname,
    );
    if (
      url.protocol !== "https:" &&
      !(isLoopback && url.protocol === "http:")
    ) {
      throw new Error(`${label} must use HTTPS or loopback HTTP.`);
    }
    if (url.username || url.password || url.search || url.hash) {
      throw new Error(
        `${label} must not contain credentials, query strings, or fragments.`,
      );
    }
    return url.toString();
  }
  if (
    !(
      text.startsWith("/") ||
      text.startsWith("./") ||
      text.startsWith("../")
    ) ||
    text.includes("?") ||
    text.includes("#") ||
    text.includes("\\")
  ) {
    throw new Error(
      `${label} must be package-relative, root-relative, HTTPS, or loopback HTTP without query strings or fragments.`,
    );
  }
  return text;
}

function normalizeCanonicalChainIdHex(value, label) {
  const text = normalizeNonEmptyText(value, label);
  if (!/^0x[0-9a-f]+$/u.test(text)) {
    throw new Error(`${label} must be a canonical lowercase 0x hex string.`);
  }
  return text;
}

function readStringArray(record, keys, label) {
  if (!isRecord(record)) return [];
  let selected = null;
  let selectedKey = "";
  for (const key of keys) {
    if (!hasOwn(record, key)) continue;
    const value = ownValue(record, key);
    if (!Array.isArray(value)) {
      throw new Error(`${label}.${key} must be an array of strings.`);
    }
    const normalized = value.map((entry, index) =>
      normalizeNonEmptyText(entry, `${label}.${key}[${index}]`),
    );
    if (selected) {
      throw new Error(
        `${label} must not use multiple export aliases: ${selectedKey}, ${key}.`,
      );
    }
    selected = normalized;
    selectedKey = key;
  }
  return selected ?? [];
}

function normalizeBrowserProverRefRecord(record, label) {
  const source = routeConfigRequiredRecord(record, label);
  assertSingleStringAliasPerSource(
    [
      {
        record: source,
        keys: ["moduleUrl", "module_url", "browserModuleUrl", "browser_module_url"],
        pathName: label,
      },
    ],
    `${label}.moduleUrl`,
  );
  assertSingleStringAliasPerSource(
    [
      {
        record: source,
        keys: ["moduleSpecifier", "module_specifier", "specifier"],
        pathName: label,
      },
    ],
    `${label}.moduleSpecifier`,
  );
  const moduleSpecifier = readOptionalCanonicalManifestText(
    source,
    ["moduleSpecifier", "module_specifier", "specifier"],
    `${label}.moduleSpecifier`,
    { allowNull: true },
  );
  const expectedExports = readStringArray(
    source,
    ["expectedExports", "expected_exports", "exports", "exportNames", "export_names"],
    label,
  );
  if (expectedExports.length === 0) {
    throw new Error(`${label}.expectedExports must not be empty.`);
  }
  return {
    moduleUrl: normalizeBrowserProverModuleUrl(
      readFirstString(source, "moduleUrl", "module_url", "browserModuleUrl", "browser_module_url"),
      `${label}.moduleUrl`,
    ),
    moduleSpecifier: moduleSpecifier || null,
    moduleHash: normalizeCanonicalHex32(
      readFirstString(source, "moduleHash", "module_hash", "moduleSha256", "module_sha256", "sha256"),
      `${label}.moduleHash`,
    ),
    manifestHash: normalizeCanonicalHex32(
      readFirstString(source, "manifestHash", "manifest_hash", "manifestSha256", "manifest_sha256"),
      `${label}.manifestHash`,
    ),
    expectedExports,
    boundRouteHash: normalizeCanonicalHex32(
      readFirstString(source, "boundRouteHash", "bound_route_hash", "routeHash", "route_hash", "destinationBindingHash", "destination_binding_hash"),
      `${label}.boundRouteHash`,
    ),
    boundProofHash: normalizeCanonicalHex32(
      readFirstString(source, "boundProofHash", "bound_proof_hash", "proofHash", "proof_hash", "proofArtifactHash", "proof_artifact_hash"),
      `${label}.boundProofHash`,
    ),
  };
}

async function readBscRouteBrowserProverManifestRef(
  options,
  direction,
  {
    profile,
    proofArtifactHash,
    provingKeyHash,
    nativeEvmProverBundleHash,
    destinationBindingHash,
  },
) {
  const optionKeys =
    direction === "source"
      ? [
          "source-browser-prover-manifest",
          "sourceBrowserProverManifest",
          "source-prover-manifest",
        ]
      : [
          "destination-browser-prover-manifest",
          "destinationBrowserProverManifest",
          "destination-prover-manifest",
        ];
  const provided = optionKeys
    .map((key) => [key, ownValue(options, key)])
    .filter(([, value]) => value !== undefined && value !== null && trim(value) !== "");
  if (provided.length === 0) {
    return null;
  }
  if (provided.length > 1) {
    throw new Error(
      `${direction} browser prover manifest must not use multiple aliases: ${provided
        .map(([key]) => `--${key}`)
        .join(", ")}.`,
    );
  }
  const [optionKey, pathName] = provided[0];
  const filePath = resolve(String(pathName));
  const bytes = await readFile(filePath);
  if (bytes.byteLength > SCCP_BSC_BROWSER_PROVER_MANIFEST_MAX_BYTES) {
    throw new Error(
      `--${optionKey} exceeds ${SCCP_BSC_BROWSER_PROVER_MANIFEST_MAX_BYTES} bytes.`,
    );
  }
  const manifest = JSON.parse(Buffer.from(bytes).toString("utf8"));
  if (!isRecord(manifest)) {
    throw new Error(`--${optionKey} must contain a JSON object.`);
  }
  const reason = unsafeSecretReason(manifest, `--${optionKey}`);
  if (reason) {
    throw new Error(reason);
  }
  const label = `${direction} browser prover manifest`;
  const schema = readRequiredString(manifest, ["schema"], `${label}.schema`);
  if (schema !== BSC_BROWSER_PROVER_MANIFEST_SCHEMA) {
    throw new Error(
      `${label}.schema must be ${BSC_BROWSER_PROVER_MANIFEST_SCHEMA}.`,
    );
  }
  const routeId = readRequiredString(
    manifest,
    ["routeId", "route_id"],
    `${label}.routeId`,
  );
  const assetKey = readRequiredString(
    manifest,
    ["assetKey", "asset_key"],
    `${label}.assetKey`,
  );
  if (routeId !== ROUTE_ID || assetKey !== ASSET_KEY) {
    throw new Error(`${label} must bind ${ROUTE_ID}/${ASSET_KEY}.`);
  }
  const bscNetwork = readRequiredString(
    manifest,
    ["bscNetwork", "bsc_network", "network"],
    `${label}.bscNetwork`,
  );
  if (normalizeBscTestnetKey(bscNetwork, `${label}.bscNetwork`) !== profile.key) {
    throw new Error(`${label}.bscNetwork must be ${profile.key}.`);
  }
  const chainIdHex = normalizeCanonicalChainIdHex(
    readRequiredString(
      manifest,
      ["bscChainIdHex", "bsc_chain_id_hex", "chainIdHex", "chain_id_hex"],
      `${label}.bscChainIdHex`,
    ),
    `${label}.bscChainIdHex`,
  );
  if (chainIdHex !== profile.chainIdHex) {
    throw new Error(`${label}.bscChainIdHex must be ${profile.chainIdHex}.`);
  }
  const moduleUrl = normalizeBrowserProverModuleUrl(
    readRequiredString(
      manifest,
      ["moduleUrl", "module_url", "browserModuleUrl", "browser_module_url"],
      `${label}.moduleUrl`,
    ),
    `${label}.moduleUrl`,
  );
  const moduleSpecifier = readOptionalCanonicalManifestText(
    manifest,
    ["moduleSpecifier", "module_specifier", "specifier"],
    `${label}.moduleSpecifier`,
    { allowNull: true },
  );
  const moduleHash = normalizeCanonicalHex32(
    readRequiredString(
      manifest,
      ["moduleSha256", "module_sha256", "moduleHash", "module_hash", "sha256"],
      `${label}.moduleSha256`,
    ),
    `${label}.moduleSha256`,
  );
  const expectedExports = readStringArray(
    manifest,
    ["exports", "exportNames", "export_names", "expectedExports", "expected_exports"],
    label,
  );
  if (expectedExports.length === 0) {
    throw new Error(`${label}.exports must not be empty.`);
  }
  const manifestProofArtifactHash = normalizeCanonicalHex32(
    readRequiredString(
      manifest,
      ["proofArtifactHash", "proof_artifact_hash"],
      `${label}.proofArtifactHash`,
    ),
    `${label}.proofArtifactHash`,
  );
  if (proofArtifactHash && manifestProofArtifactHash !== proofArtifactHash) {
    throw new Error(`${label}.proofArtifactHash must match route manifest.`);
  }
  const manifestProvingKeyHash = normalizeCanonicalHex32(
    readRequiredString(
      manifest,
      ["provingKeyHash", "proving_key_hash"],
      `${label}.provingKeyHash`,
    ),
    `${label}.provingKeyHash`,
  );
  if (provingKeyHash && manifestProvingKeyHash !== provingKeyHash) {
    throw new Error(`${label}.provingKeyHash must match route manifest.`);
  }
  const manifestNativeBundleHash = normalizeCanonicalHex32(
    readRequiredString(
      manifest,
      [
        "nativeEvmProverBundleHash",
        "native_evm_prover_bundle_hash",
        "nativeProverBundleHash",
        "native_prover_bundle_hash",
      ],
      `${label}.nativeEvmProverBundleHash`,
    ),
    `${label}.nativeEvmProverBundleHash`,
  );
  if (
    nativeEvmProverBundleHash &&
    manifestNativeBundleHash !== nativeEvmProverBundleHash
  ) {
    throw new Error(
      `${label}.nativeEvmProverBundleHash must match route manifest.`,
    );
  }
  const manifestHash = sha256HexBytes(bytes);
  return {
    moduleUrl,
    moduleSpecifier: moduleSpecifier || null,
    moduleHash,
    manifestHash,
    expectedExports,
    boundRouteHash: destinationBindingHash,
    boundProofHash: manifestProofArtifactHash,
  };
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
  const entries = flattenArray(value, label);
  if (entries.length !== expectedLength) {
    throw new Error(`${label} must contain ${expectedLength} uint256 values.`);
  }
  const values = entries.map((entry, index) =>
    normalizeBn254DecimalWord(entry, `${label}[${index}]`),
  );
  return values;
}

function normalizeBn254FieldElement(value, label) {
  const parsed = BigInt(value);
  if (parsed < 0n || parsed >= BN254_BASE_FIELD_MODULUS) {
    throw new Error(`${label} must be a BN254 field element.`);
  }
  return parsed;
}

function normalizeBn254DecimalWord(value, label) {
  if (typeof value !== "string" || !DECIMAL_WORD.test(value)) {
    throw new Error(`${label} must be a canonical decimal BN254 field word.`);
  }
  const parsed = BigInt(value);
  if (parsed >= BN254_BASE_FIELD_MODULUS) {
    throw new Error(`${label} must be a BN254 field element.`);
  }
  return parsed.toString();
}

function normalizeBn254ScalarDecimalWord(value, label) {
  if (typeof value !== "string" || !DECIMAL_WORD.test(value)) {
    throw new Error(`${label} must be a canonical decimal BN254 field word.`);
  }
  const parsed = BigInt(value);
  if (parsed >= BN254_SCALAR_FIELD_MODULUS) {
    throw new Error(`${label} must be a BN254 scalar field element.`);
  }
  return parsed.toString();
}

function normalizeGroth16ProofSelfTestPublicSignals(value, label) {
  if (
    !Array.isArray(value) ||
    value.length !== BSC_GROTH16_PUBLIC_SIGNAL_NAMES.length
  ) {
    throw new Error(
      `${label} must contain ${BSC_GROTH16_PUBLIC_SIGNAL_NAMES.length} canonical decimal BN254 field words.`,
    );
  }
  return value.map((entry, index) =>
    normalizeBn254ScalarDecimalWord(entry, `${label}[${index}]`),
  );
}

function hex32Bytes(value, label) {
  return hexToBytes(normalizeHex32(value, label), label, 32);
}

function bigintFromBytes(bytes) {
  return BigInt(bytesToHex(bytes));
}

function byteBitsLittleEndian(byte) {
  return Array.from({ length: 8 }, (_, bit) => (byte >> bit) & 1);
}

function wordBitsLittleEndianByByte(bytes) {
  return Array.from(bytes).flatMap(byteBitsLittleEndian);
}

function bscGroth16SelfTestWord(profile, signalName) {
  return sha256(
    textEncoder.encode(
      `${BSC_GROTH16_SELF_TEST_SAMPLE_ID}:${profile.key}:${signalName}`,
    ),
  );
}

function bscGroth16PublicSignal(labelHash, valueBytes) {
  const input = Buffer.concat([
    Buffer.from(hex32Bytes(labelHash, "BSC Groth16 signal label")),
    Buffer.from(valueBytes),
  ]);
  const digest = keccak_256(
    new Uint8Array(input.buffer, input.byteOffset, input.byteLength),
  );
  return (bigintFromBytes(digest) % BN254_SCALAR_FIELD_MODULUS).toString(10);
}

export function bscGroth16DeterministicProofSelfTestSample(profileValue = "testnet") {
  const profile = normalizeBscNetworkProfile(
    isRecord(profileValue) ? readFirstString(profileValue, "key") : profileValue,
  );
  const syntheticInputWords = Object.fromEntries(
    BSC_GROTH16_PUBLIC_SIGNAL_NAMES.map((signalName) => [
      signalName,
      bytesToHex(bscGroth16SelfTestWord(profile, signalName)),
    ]),
  );
  const publicSignalWords = BSC_GROTH16_PUBLIC_SIGNAL_NAMES.map(
    (signalName, index) =>
      bscGroth16PublicSignal(
        BSC_GROTH16_PUBLIC_SIGNAL_LABEL_HASHES[index],
        hex32Bytes(syntheticInputWords[signalName], `${signalName} self-test word`),
      ),
  );
  const input = { publicSignals: publicSignalWords };
  for (const [index, signalName] of BSC_GROTH16_PUBLIC_SIGNAL_NAMES.entries()) {
    input[BSC_GROTH16_SIGNAL_INPUT_NAMES[index]] = wordBitsLittleEndianByByte(
      hex32Bytes(syntheticInputWords[signalName], `${signalName} self-test word`),
    );
  }
  return {
    sampleId: BSC_GROTH16_SELF_TEST_SAMPLE_ID,
    syntheticInputWords,
    publicSignalNames: [...BSC_GROTH16_PUBLIC_SIGNAL_NAMES],
    publicSignalWords,
    input,
    inputSha256: sha256HexBytes(Buffer.from(canonicalJson(input), "utf8")),
  };
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
  const publicInputCount = Number(
    readFirstValue(material, "publicInputCount", "public_input_count"),
  );
  if (!Number.isSafeInteger(publicInputCount) || publicInputCount !== 9) {
    throw new Error("publicInputCount must be 9 for BSC SCCP verifier material.");
  }
  const verifierKeyHashValues = [
    ["expectedVerifierKeyHash", readFirstValue(material, "expectedVerifierKeyHash")],
    ["verifierKeyHash", readFirstValue(material, "verifierKeyHash")],
    ["verifyingKeyHash", readFirstValue(material, "verifyingKeyHash")],
  ]
    .filter(([, value]) => value !== undefined && value !== null && trim(value) !== "")
    .map(([label, value]) => [label, normalizeHex32(value, label)]);
  if (verifierKeyHashValues.length === 0) {
    throw new Error("expectedVerifierKeyHash is required.");
  }
  const expectedVerifierKeyHash = verifierKeyHashValues[0][1];
  for (const [label, value] of verifierKeyHashValues.slice(1)) {
    if (value !== expectedVerifierKeyHash) {
      throw new Error(`${label} must match expectedVerifierKeyHash.`);
    }
  }
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

const OFFLINE_FULL_TOML_EVIDENCE_HASH_MODE =
  "sha256:merged-full-config-without-post_deploy_offline_full_toml_sha256";

const OFFLINE_FULL_TOML_EVIDENCE_FORBIDDEN_PAYLOAD_KEYS = new Set([
  "baseConfig",
  "base_config",
  "baseConfigToml",
  "base_config_toml",
  "configToml",
  "config_toml",
  "fullConfig",
  "full_config",
  "fullConfigToml",
  "full_config_toml",
  "fullToml",
  "full_toml",
  "toml",
]);

function offlineFullTomlEvidenceForbiddenPayloadField(
  value,
  pathName,
  seen = new WeakSet(),
) {
  if (Array.isArray(value)) {
    if (seen.has(value)) {
      return "";
    }
    seen.add(value);
    for (const [index, entry] of ownArrayValues(value)) {
      const reason = offlineFullTomlEvidenceForbiddenPayloadField(
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
    if (OFFLINE_FULL_TOML_EVIDENCE_FORBIDDEN_PAYLOAD_KEYS.has(key)) {
      return childPath;
    }
    const reason = offlineFullTomlEvidenceForbiddenPayloadField(
      entry,
      childPath,
      seen,
    );
    if (reason) {
      return reason;
    }
  }
  return "";
}

function bscProductionHashPlaceholderProblem(value, label) {
  let normalized = "";
  try {
    normalized = normalizeCanonicalHex32(value, label);
  } catch (error) {
    return `${label} is invalid: ${
      error instanceof Error ? error.message : String(error)
    }`;
  }
  const bytes = hexToBytes(normalized, label, 32);
  const repeatedPatternLength = repeatedPrefixPatternLength(bytes, 16);
  if (repeatedPatternLength > 0) {
    return `${label} looks like placeholder material: repeated ${repeatedPatternLength}-byte pattern.`;
  }
  const arithmeticDelta = constantByteDelta(bytes);
  if (arithmeticDelta !== null) {
    return `${label} looks like placeholder material: arithmetic byte sequence with step ${arithmeticDelta}.`;
  }
  const uniqueBytes = new Set(bytes);
  if (uniqueBytes.size <= 4) {
    return `${label} looks like placeholder material: only ${uniqueBytes.size} unique byte values.`;
  }
  return "";
}

function readOfflineFullTomlEvidenceCanonicalHash(record, keys, label) {
  return readRequiredConsistentNormalizedString(
    [
      {
        record,
        keys,
        pathName: label,
      },
    ],
    label,
    (value, fieldLabel) => normalizeCanonicalHex32(value, fieldLabel),
  );
}

function offlineFullTomlEvidenceProductionProblems(record, label) {
  if (
    !isRecord(record) ||
    readFirstString(record, "schema") !== OFFLINE_FULL_TOML_EVIDENCE_SCHEMA
  ) {
    return [];
  }

  const problems = [];
  const forbiddenPayloadField = offlineFullTomlEvidenceForbiddenPayloadField(
    record,
    label,
  );
  if (forbiddenPayloadField) {
    problems.push(
      `${label} must not embed raw TAIRA config or TOML payload material at ${forbiddenPayloadField}`,
    );
  }

  const routeManifestPath = readFirstString(
    record,
    "routeManifestPath",
    "route_manifest_path",
  );
  if (!routeManifestPath) {
    problems.push(`${label} routeManifestPath is required`);
  } else {
    const pathProblem = bscOfflineFullTomlEvidencePathProblem(
      routeManifestPath,
      `${label} routeManifestPath`,
    );
    if (pathProblem) {
      problems.push(pathProblem);
    } else if (!isCanonicalBscProductionArtifactPath(routeManifestPath)) {
      problems.push(
        `${label} routeManifestPath must point to canonical BSC production artifacts`,
      );
    }
  }

  const fullConfigPath = readFirstString(
    record,
    "fullConfigPath",
    "full_config_path",
  );
  if (!fullConfigPath) {
    problems.push(`${label} fullConfigPath is required`);
  } else {
    const pathProblem = bscOfflineFullTomlEvidencePathProblem(
      fullConfigPath,
      `${label} fullConfigPath`,
    );
    if (pathProblem) {
      problems.push(pathProblem);
    } else if (!isCanonicalBscProductionArtifactPath(fullConfigPath)) {
      problems.push(
        `${label} fullConfigPath must point to canonical BSC production artifacts`,
      );
    }
  }

  const networkText =
    readFirstString(record, "bscNetwork", "bsc_network", "network") ||
    readFirstString(record, "chain") ||
    "testnet";
  try {
    normalizeBscOfflineFullTomlEvidence(
      record,
      normalizeBscNetworkProfile(networkText),
    );
  } catch (error) {
    problems.push(
      `${label} failed offline full TOML evidence validation: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }

  const postDeployLiveEvidence =
    readFirstRecord(
      record,
      "postDeployLiveEvidence",
      "post_deploy_live_evidence",
    ) ?? {};
  let offlineFullTomlSha256 = "";
  try {
    offlineFullTomlSha256 = readRequiredConsistentNormalizedString(
      [
        {
          record,
          keys: ["offlineFullTomlSha256", "offline_full_toml_sha256"],
          pathName: label,
        },
        {
          record: postDeployLiveEvidence,
          keys: ["offlineFullTomlSha256", "offline_full_toml_sha256"],
          pathName: `${label}.postDeployLiveEvidence`,
        },
      ],
      `${label} offlineFullTomlSha256`,
      (value, fieldLabel) => normalizeCanonicalHex32(value, fieldLabel),
    );
  } catch (error) {
    problems.push(error instanceof Error ? error.message : String(error));
  }
  if (offlineFullTomlSha256) {
    problems.push(
      bscProductionHashPlaceholderProblem(
        offlineFullTomlSha256,
        `${label} offlineFullTomlSha256`,
      ),
    );
  }

  let hashInputSha256 = "";
  try {
    hashInputSha256 = readOfflineFullTomlEvidenceCanonicalHash(
      record,
      ["hashInputSha256", "hash_input_sha256"],
      `${label} hashInputSha256`,
    );
  } catch (error) {
    problems.push(error instanceof Error ? error.message : String(error));
  }
  if (hashInputSha256) {
    problems.push(
      bscProductionHashPlaceholderProblem(
        hashInputSha256,
        `${label} hashInputSha256`,
      ),
    );
  }

  let renderedTomlSha256 = "";
  try {
    renderedTomlSha256 = readOfflineFullTomlEvidenceCanonicalHash(
      record,
      ["renderedTomlSha256", "rendered_toml_sha256"],
      `${label} renderedTomlSha256`,
    );
  } catch (error) {
    problems.push(error instanceof Error ? error.message : String(error));
  }
  if (renderedTomlSha256) {
    problems.push(
      bscProductionHashPlaceholderProblem(
        renderedTomlSha256,
        `${label} renderedTomlSha256`,
      ),
    );
  }

  if (
    offlineFullTomlSha256 &&
    hashInputSha256 &&
    offlineFullTomlSha256 !== hashInputSha256
  ) {
    problems.push(
      `${label} hashInputSha256 must equal offlineFullTomlSha256`,
    );
  }

  try {
    assertSingleStringAliasPerSource(
      [
        {
          record,
          keys: ["hashMode", "hash_mode"],
          pathName: label,
        },
      ],
      `${label} hashMode`,
    );
    const hashMode = readFirstString(record, "hashMode", "hash_mode");
    if (hashMode !== OFFLINE_FULL_TOML_EVIDENCE_HASH_MODE) {
      problems.push(
        `${label} hashMode must be ${OFFLINE_FULL_TOML_EVIDENCE_HASH_MODE}`,
      );
    }
  } catch (error) {
    problems.push(error instanceof Error ? error.message : String(error));
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
    sha256(textEncoder.encode(JSON.stringify(bundle))),
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
    ...offlineFullTomlEvidenceProductionProblems(value, label),
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

function normalizeTransactionHash(value, label) {
  const text = normalizeNonEmptyText(value, label);
  const hex = text.startsWith("0x") ? text.slice(2) : text;
  if (!/^[0-9a-f]{64}$/iu.test(hex) || /^0{64}$/u.test(hex.toLowerCase())) {
    throw new Error(`${label} must be a non-zero 32-byte transaction hash.`);
  }
  return `0x${hex.toLowerCase()}`;
}

function transactionStatusKind(status) {
  if (typeof status === "string") {
    return status;
  }
  if (!isRecord(status)) {
    return null;
  }
  for (const value of [
    ownValue(ownValue(status, "status"), "kind"),
    ownValue(status, "summary"),
    ownValue(status, "status"),
    ownValue(status, "kind"),
    ownValue(ownValue(status, "value"), "kind"),
  ]) {
    if (typeof value === "string" && value.trim()) {
      return value.trim();
    }
  }
  return null;
}

function isTerminalTransactionStatus(status) {
  const kind = transactionStatusKind(status);
  if (typeof kind !== "string") {
    return false;
  }
  return /applied|committed|rejected|failed|expired/iu.test(kind);
}

async function delayMs(ms) {
  await new Promise((resolveDelay) => {
    setTimeout(resolveDelay, ms);
  });
}

async function responseBodyPreview(response) {
  const bytes = Buffer.from(await response.arrayBuffer());
  if (bytes.length === 0) {
    return "";
  }
  const utf8 = bytes.toString("utf8").replace(/[^\t\n\r -~]/gu, "");
  if (utf8.trim()) {
    return utf8.trim().slice(0, 512);
  }
  return `0x${bytes.toString("hex").slice(0, 512)}`;
}

async function submitSignedTransactionRawToTairaPipeline(
  client,
  toriiUrl,
  signedTransaction,
  hashHex,
  options = {},
) {
  const txBuffer = Buffer.from(signedTransaction);
  const versionedPayload = Buffer.concat([Buffer.from([1]), txBuffer]);
  const pipelineUrl = new URL("/v1/pipeline/transactions", toriiUrl);
  const response = await fetch(pipelineUrl, {
    method: "POST",
    headers: {
      "Content-Type": "application/x-norito",
      Accept: "application/x-norito, application/json",
    },
    body: versionedPayload,
  });
  if (![200, 201, 202, 204].includes(response.status)) {
    const preview = await responseBodyPreview(response);
    throw new Error(
      `Torii responded with HTTP ${response.status} while submitting raw signed transaction${
        preview ? `: ${preview}` : ""
      }`,
    );
  }
  const submission = {
    accepted: true,
    httpStatus: response.status,
    pipelineUrl: pipelineUrl.toString(),
  };
  if (!options.waitForCommit) {
    return { hash: hashHex, submission };
  }

  const pollIntervalMs = options.pollIntervalMs ?? 500;
  const timeoutMs = options.timeoutMs ?? 30_000;
  const deadline = Date.now() + timeoutMs;
  let status = null;
  while (Date.now() <= deadline) {
    status = await client.getTransactionStatus(hashHex, {
      allowShortHash: true,
      scope: options.scope ?? "auto",
    });
    if (isTerminalTransactionStatus(status)) {
      return { hash: hashHex, submission, status };
    }
    await delayMs(pollIntervalMs);
  }

  const error = new Error("timed out waiting for transaction status");
  error.hash = hashHex;
  error.submission = submission;
  error.status = status;
  throw error;
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
  const codeHashes = normalizeBscReadbackCodeHashes(readback);
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
  if (codeHashes.verifier !== verifierCodeHash) {
    throw new Error(
      "BSC readback verifier bytecode hash does not match declared verifier code hash.",
    );
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
  const optionalAddressChecks = [
    ["tokenAddress", "token", ownValue(addresses, "token")],
    ["bridgeAddress", "bridge", ownValue(addresses, "bridge")],
    [
      "sourceBridgeAddress",
      "source bridge",
      ownValue(addresses, "sourceBridge"),
    ],
    ["verifierAddress", "verifier", ownValue(addresses, "verifier")],
  ];
  for (const [key, label, expected] of optionalAddressChecks) {
    const value = ownValue(readback, key);
    if (value === undefined) {
      continue;
    }
    if (normalizeEvmAddress(value, key) !== expected) {
      throw new Error(`BSC readback ${label} address does not match.`);
    }
  }
  return true;
}

function normalizeBscContractCodeHashMap(record, label) {
  if (!isRecord(record)) {
    throw new Error(`${label} must be an object.`);
  }
  const codeHashes = {};
  for (const key of BSC_CONTRACT_CODE_ROLES) {
    const codeHash = normalizeHex32(
      ownValue(record, key),
      `${label}.${key}`,
    );
    if (codeHash === EVM_EMPTY_CODE_KECCAK256) {
      throw new Error(
        `${label}.${key} must not be the empty account code hash.`,
      );
    }
    codeHashes[key] = codeHash;
  }
  return codeHashes;
}

function normalizeBscReadbackCodeHashes(readback) {
  const readbackCodeHashes = ownValue(readback, "codeHashes");
  if (!isRecord(readbackCodeHashes)) {
    throw new Error("BSC contract readback must include codeHashes.");
  }
  return normalizeBscContractCodeHashMap(
    readbackCodeHashes,
    "BSC contract readback codeHashes",
  );
}

function normalizeBscCompiledContractCodeHashes(input, required = false) {
  if (input === undefined || input === null) {
    if (required) {
      throw new Error(
        "BSC deployment evidence must include compiledContractCodeHashes.",
      );
    }
    return null;
  }
  return normalizeBscContractCodeHashMap(
    input,
    "BSC compiled contract code hashes",
  );
}

function hasImmutableReferences(artifact) {
  return (
    isRecord(artifact?.immutableReferences) &&
    Object.keys(artifact.immutableReferences).length > 0
  );
}

function uint256WordHex(value, label) {
  const bigint = typeof value === "bigint" ? value : BigInt(value);
  if (bigint < 0n || bigint >= 2n ** 256n) {
    throw new Error(`${label} must fit into a uint256 word.`);
  }
  return `0x${bigint.toString(16).padStart(64, "0")}`;
}

function sourceBridgeImmutableValues(profile) {
  return {
    networkId: profile.networkIdHex,
    sourceDomain: uint256WordHex(SCCP_DOMAIN_BSC, "sourceBridge.sourceDomain"),
    targetDomain: uint256WordHex(SCCP_DOMAIN_SORA, "sourceBridge.targetDomain"),
  };
}

function deployedBytecodeWithNamedImmutables(artifact, valuesByName, label) {
  if (!hasImmutableReferences(artifact)) {
    return artifact.deployedBytecode;
  }
  if (!isRecord(artifact.immutableReferenceNames)) {
    throw new Error(`${label} immutable reference names are missing.`);
  }
  const bytes = hexToBytes(artifact.deployedBytecode, `${label} bytecode`, null, {
    allowZero: false,
  });
  for (const [id, locations] of ownRecordEntries(artifact.immutableReferences)) {
    if (!Array.isArray(locations) || locations.length === 0) {
      throw new Error(`${label} immutable reference ${id} has no locations.`);
    }
    const metadata = ownValue(artifact.immutableReferenceNames, id);
    const name = isRecord(metadata) ? ownValue(metadata, "name") : undefined;
    if (typeof name !== "string" || !name) {
      throw new Error(`${label} immutable reference ${id} has no source name.`);
    }
    const value = ownValue(valuesByName, name);
    if (value === undefined) {
      throw new Error(`${label} immutable ${name} has no deployment value.`);
    }
    for (const location of locations) {
      if (!isRecord(location)) {
        throw new Error(`${label} immutable ${name} location is invalid.`);
      }
      const start = ownValue(location, "start");
      const length = ownValue(location, "length");
      if (
        !Number.isSafeInteger(start) ||
        !Number.isSafeInteger(length) ||
        start < 0 ||
        length <= 0 ||
        start + length > bytes.length
      ) {
        throw new Error(`${label} immutable ${name} location is out of range.`);
      }
      const valueBytes = hexToBytes(value, `${label} immutable ${name}`, length, {
        allowZero: true,
      });
      const existing = bytes.slice(start, start + length);
      if (!existing.every((byte) => byte === 0)) {
        throw new Error(
          `${label} immutable ${name} placeholder is not zero-filled.`,
        );
      }
      bytes.set(valueBytes, start);
    }
  }
  return bytesToHex(bytes);
}

function deploymentAwareCompiledCodeHash(key, artifact, { profile }) {
  if (!hasImmutableReferences(artifact)) {
    return artifact?.deployedBytecodeKeccak256;
  }
  if (key !== "sourceBridge") {
    throw new Error(
      `BSC ${key} has immutables but no deployment-aware code hash binding.`,
    );
  }
  const patchedBytecode = deployedBytecodeWithNamedImmutables(
    artifact,
    sourceBridgeImmutableValues(profile),
    "BSC sourceBridge",
  );
  return bytesToHex(
    keccak_256(
      hexToBytes(patchedBytecode, "BSC sourceBridge patched bytecode", null, {
        allowZero: false,
      }),
    ),
  );
}

export function compiledContractCodeHashesFromArtifacts(
  artifacts,
  { profile } = {},
) {
  const normalizedProfile = normalizeBscNetworkProfile(profile?.key ?? profile);
  return normalizeBscCompiledContractCodeHashes(
    Object.fromEntries(
      BSC_CONTRACT_CODE_ROLES.map((key) => [
        key,
        deploymentAwareCompiledCodeHash(key, artifacts?.[key], {
          profile: normalizedProfile,
        }),
      ]),
    ),
    true,
  );
}

function collectSolcImmutableDeclarations(node, declarations = {}) {
  if (!isRecord(node)) {
    return declarations;
  }
  if (
    ownValue(node, "nodeType") === "VariableDeclaration" &&
    ownValue(node, "stateVariable") === true &&
    ownValue(node, "mutability") === "immutable"
  ) {
    const id = ownValue(node, "id");
    const name = ownValue(node, "name");
    if (Number.isSafeInteger(id) && typeof name === "string" && name) {
      const typeDescriptions = ownValue(node, "typeDescriptions");
      declarations[String(id)] = {
        name,
        type: isRecord(typeDescriptions)
          ? ownValue(typeDescriptions, "typeString") ?? null
          : null,
      };
    }
  }
  for (const value of Object.values(node)) {
    if (Array.isArray(value)) {
      for (const child of value) {
        collectSolcImmutableDeclarations(child, declarations);
      }
    } else if (isRecord(value)) {
      collectSolcImmutableDeclarations(value, declarations);
    }
  }
  return declarations;
}

function immutableReferenceNamesForContract(
  output,
  definition,
  immutableReferences,
) {
  if (!isRecord(immutableReferences) || Object.keys(immutableReferences).length === 0) {
    return {};
  }
  const declarations = {};
  for (const source of Object.values(output.sources ?? {})) {
    collectSolcImmutableDeclarations(ownValue(source, "ast"), declarations);
  }
  const result = {};
  for (const id of Object.keys(immutableReferences)) {
    const declaration = ownValue(declarations, id);
    if (!isRecord(declaration)) {
      throw new Error(
        `Missing immutable declaration ${id} for ${definition.contract}.`,
      );
    }
    result[id] = declaration;
  }
  return result;
}

function assertBscCompiledCodeHashesMatchReadback({
  compiledCodeHashes,
  readbackCodeHashes,
  verifierCodeHash,
}) {
  if (!compiledCodeHashes) {
    return;
  }
  for (const key of BSC_CONTRACT_CODE_ROLES) {
    if (compiledCodeHashes[key] !== readbackCodeHashes[key]) {
      throw new Error(
        `BSC compiled ${key} code hash does not match live readback.`,
      );
    }
  }
  if (compiledCodeHashes.verifier !== verifierCodeHash) {
    throw new Error(
      "BSC compiled verifier code hash does not match declared verifier code hash.",
    );
  }
}

function bscDeploymentEvidenceHashSnapshot(routeEvidence) {
  const readback = routeEvidence.contractReadback
    ? {
        chainIdHex: routeEvidence.profile.chainIdHex,
        tokenAddress: routeEvidence.addresses.token,
        bridgeAddress: routeEvidence.addresses.bridge,
        sourceBridgeAddress: routeEvidence.addresses.sourceBridge,
        verifierAddress: routeEvidence.addresses.verifier,
        codePresent: {
          token: true,
          bridge: true,
          sourceBridge: true,
          verifier: true,
        },
        codeHashes: normalizeBscReadbackCodeHashes(
          routeEvidence.contractReadback,
        ),
        tokenBridgeAddress: normalizeEvmAddress(
          ownValue(routeEvidence.contractReadback, "tokenBridgeAddress"),
          "BSC deployment evidence bscContractReadback.tokenBridgeAddress",
        ),
        tokenBridgeLocked: ownValue(
          routeEvidence.contractReadback,
          "tokenBridgeLocked",
        ),
        sourceBridgeOwner: normalizeEvmAddress(
          ownValue(routeEvidence.contractReadback, "sourceBridgeOwner"),
          "BSC deployment evidence bscContractReadback.sourceBridgeOwner",
        ),
        verifierKeyHash: normalizeHex32(
          ownValue(routeEvidence.contractReadback, "verifierKeyHash"),
          "BSC deployment evidence bscContractReadback.verifierKeyHash",
        ),
        bridgeDestinationBindingHash: normalizeHex32(
          ownValue(
            routeEvidence.contractReadback,
            "bridgeDestinationBindingHash",
          ),
          "BSC deployment evidence bscContractReadback.bridgeDestinationBindingHash",
        ),
        bridgeVerifierAddress: normalizeEvmAddress(
          ownValue(routeEvidence.contractReadback, "bridgeVerifierAddress"),
          "BSC deployment evidence bscContractReadback.bridgeVerifierAddress",
        ),
        bridgeVerifierCodeHash: normalizeHex32(
          ownValue(routeEvidence.contractReadback, "bridgeVerifierCodeHash"),
          "BSC deployment evidence bscContractReadback.bridgeVerifierCodeHash",
        ),
        bridgeVerifierKeyHash: normalizeHex32(
          ownValue(routeEvidence.contractReadback, "bridgeVerifierKeyHash"),
          "BSC deployment evidence bscContractReadback.bridgeVerifierKeyHash",
        ),
        bridgeNetworkId: normalizeHex32(
          ownValue(routeEvidence.contractReadback, "bridgeNetworkId"),
          "BSC deployment evidence bscContractReadback.bridgeNetworkId",
        ),
        bridgeSourceDomain: SCCP_DOMAIN_SORA,
        bridgeTargetDomain: SCCP_DOMAIN_BSC,
      }
    : null;
  return {
    schema: DEPLOYMENT_EVIDENCE_SCHEMA,
    routeId: ROUTE_ID,
    assetKey: ASSET_KEY,
    bscNetwork: routeEvidence.profile.key,
    chain: routeEvidence.profile.chain,
    chainIdHex: routeEvidence.profile.chainIdHex,
    networkIdHex: routeEvidence.profile.networkIdHex,
    addresses: routeEvidence.addresses,
    verifierCodeHash: routeEvidence.verifierCodeHash,
    verifierKeyHash: routeEvidence.verifierKeyHash,
    destinationBindingKey: routeEvidence.destinationBindingKey,
    destinationBindingHash: routeEvidence.destinationBindingHash,
    compiledContractCodeHashes: routeEvidence.compiledCodeHashes,
    bscContractReadback: readback,
  };
}

function bscDeploymentEvidenceSha256(routeEvidence) {
  return bytesToHex(
    sha256(
      textEncoder.encode(
        stableJsonString(bscDeploymentEvidenceHashSnapshot(routeEvidence)),
      ),
    ),
  );
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

function stableJsonValue(value) {
  if (Array.isArray(value)) {
    return value.map((entry) => stableJsonValue(entry));
  }
  if (isRecord(value)) {
    return Object.fromEntries(
      ownRecordEntries(value)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, entry]) => [key, stableJsonValue(entry)]),
    );
  }
  return value;
}

function stableJsonString(value) {
  return JSON.stringify(stableJsonValue(value));
}

function sourceParityTreeHash(attestation) {
  const payload = {
    schema: attestation.schema,
    routeId: attestation.routeId,
    assetKey: attestation.assetKey,
    bscNetwork: attestation.bscNetwork,
    chain: attestation.chain,
    chainIdHex: attestation.chainIdHex,
    networkIdHex: attestation.networkIdHex,
    domain: attestation.domain,
    proofBackend: attestation.proofBackend,
    proofFamily: attestation.proofFamily,
    requiredMarkers: attestation.requiredMarkers,
    sdks: attestation.sdks,
  };
  return bytesToHex(sha256(textEncoder.encode(stableJsonString(payload))));
}

function normalizeSourceParitySpecs(specs = sourceParitySdkSpecsForProfile()) {
  const requiredSdkEntries = Object.entries(
    SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
  );
  for (const [sdk, implementation] of requiredSdkEntries) {
    const spec = specs[sdk];
    if (!isRecord(spec)) {
      throw new Error(`source parity spec is missing SDK: ${sdk}.`);
    }
    if (spec.implementation !== implementation) {
      throw new Error(
        `source parity spec ${sdk} implementation must be ${implementation}.`,
      );
    }
    if (!Array.isArray(spec.files) || spec.files.length === 0) {
      throw new Error(`source parity spec ${sdk} requires files.`);
    }
  }
  const unknown = Object.keys(specs).filter(
    (sdk) =>
      !Object.prototype.hasOwnProperty.call(
        SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
        sdk,
      ),
  );
  if (unknown.length > 0) {
    throw new Error(
      `source parity spec contains unknown SDKs: ${unknown.sort().join(", ")}.`,
    );
  }
  return specs;
}

async function readSourceParityFile(repoRoot, sdk, fileSpec) {
  const relativePath = normalizeNonEmptyText(
    fileSpec.path,
    `source parity ${sdk} file path`,
  ).replace(/\\/gu, "/");
  if (
    relativePath.startsWith("/") ||
    relativePath.includes("\0") ||
    pathHasDecodedParentSegment(relativePath)
  ) {
    throw new Error(
      `source parity ${sdk} file path must be a safe repo-relative path.`,
    );
  }
  const absolutePath = resolve(repoRoot, relativePath);
  relativePosixPath(repoRoot, absolutePath, `source parity ${sdk} file`);
  const text = await readText(absolutePath, `source parity ${sdk} ${relativePath}`);
  const markers = Array.isArray(fileSpec.markers) ? fileSpec.markers : [];
  if (markers.length === 0) {
    throw new Error(`source parity ${sdk} ${relativePath} requires markers.`);
  }
  const missingMarkers = markers.filter((marker) => !text.includes(marker));
  if (missingMarkers.length > 0) {
    throw new Error(
      `source parity ${sdk} ${relativePath} is missing markers: ${missingMarkers.join(", ")}.`,
    );
  }
  return {
    path: relativePath,
    sha256: bytesToHex(sha256(textEncoder.encode(text))),
    sizeBytes: textEncoder.encode(text).length,
    markers: [...markers],
  };
}

export async function buildBscNativeEvmSourceParityAttestation(options = {}) {
  const profile = bscNetworkProfileFromOptions(options);
  const requiredMarkers = sourceParityRequiredMarkersForProfile(profile.key);
  const specs = normalizeSourceParitySpecs(
    ownValue(options, "sourceParitySpecs") ??
      sourceParitySdkSpecsForProfile(profile.key),
  );
  const repoRoot = resolve(ownValue(options, "repoRoot") ?? REPO_ROOT);
  const sdks = {};
  for (const [sdk, spec] of Object.entries(specs).sort(([left], [right]) =>
    left.localeCompare(right),
  )) {
    const files = [];
    for (const fileSpec of spec.files) {
      files.push(await readSourceParityFile(repoRoot, sdk, fileSpec));
    }
    const implementationHash = bytesToHex(
      sha256(
        textEncoder.encode(
          stableJsonString({
            sdk,
            implementation: spec.implementation,
            files: files.map(({ path, sha256: fileHash, markers }) => ({
              path,
              sha256: fileHash,
              markers,
            })),
          }),
        ),
      ),
    );
    sdks[sdk] = {
      implementation: spec.implementation,
      implementationHash,
      files,
    };
  }
  const attestation = {
    schema: SOURCE_PARITY_ATTESTATION_SCHEMA,
    generatedAt: ownValue(options, "generatedAt") ?? new Date().toISOString(),
    routeId: ROUTE_ID,
    assetKey: ASSET_KEY,
    bscNetwork: profile.key,
    chain: profile.chain,
    chainIdHex: profile.chainIdHex,
    networkIdHex: profile.networkIdHex,
    domain: SCCP_DOMAIN_BSC,
    proofBackend: BSC_EVM_GROTH16_BACKEND,
    proofFamily: SCCP_PROOF_FAMILY_STARK_FRI,
    requiredMarkers: [...requiredMarkers],
    sdks,
  };
  attestation.sourceTreeHash = sourceParityTreeHash(attestation);
  return attestation;
}

function validateBscNativeEvmSourceParityAttestationArtifact(artifact, profile) {
  const label = "auditHashes.native_implementation_audit source parity attestation";
  let record;
  try {
    record = parseJsonWithoutDuplicateKeys(artifact.bytes.toString("utf8"), label);
  } catch (error) {
    throw new Error(
      `${label} must be valid duplicate-free JSON: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
  const reason = unsafeSecretReason(record, label);
  if (reason) {
    throw new Error(reason);
  }
  if (!isRecord(record)) {
    throw new Error(`${label} must be a JSON object.`);
  }
  const knownTopLevelFields = new Set([
    "schema",
    "generatedAt",
    "routeId",
    "assetKey",
    "bscNetwork",
    "chain",
    "chainIdHex",
    "networkIdHex",
    "domain",
    "proofBackend",
    "proofFamily",
    "requiredMarkers",
    "sdks",
    "sourceTreeHash",
  ]);
  const problems = [
    ...Object.keys(record)
      .filter((key) => !knownTopLevelFields.has(key))
      .map((key) => `${label} contains unknown field: ${key}`),
    groth16ManifestStringProblem(
      record,
      ["schema"],
      SOURCE_PARITY_ATTESTATION_SCHEMA,
      `${label} schema`,
    ),
    groth16ManifestStringProblem(record, ["routeId"], ROUTE_ID, `${label} routeId`),
    groth16ManifestStringProblem(record, ["assetKey"], ASSET_KEY, `${label} assetKey`),
    groth16ManifestStringProblem(
      record,
      ["bscNetwork"],
      profile.key,
      `${label} bscNetwork`,
    ),
    groth16ManifestStringProblem(record, ["chain"], profile.chain, `${label} chain`),
    groth16ManifestStringProblem(
      record,
      ["chainIdHex"],
      profile.chainIdHex,
      `${label} chainIdHex`,
    ),
    groth16ManifestHashProblem(
      record,
      ["networkIdHex"],
      profile.networkIdHex,
      `${label} networkIdHex`,
    ),
    readFirstValue(record, "domain") === SCCP_DOMAIN_BSC
      ? ""
      : `${label} domain must be ${SCCP_DOMAIN_BSC}`,
    groth16ManifestStringProblem(
      record,
      ["proofBackend"],
      BSC_EVM_GROTH16_BACKEND,
      `${label} proofBackend`,
    ),
    groth16ManifestStringProblem(
      record,
      ["proofFamily"],
      SCCP_PROOF_FAMILY_STARK_FRI,
      `${label} proofFamily`,
    ),
  ].filter(Boolean);
  const requiredMarkers = sourceParityRequiredMarkersForProfile(profile.key);
  const markers = readFirstValue(record, "requiredMarkers");
  if (
    !Array.isArray(markers) ||
    JSON.stringify(markers) !== JSON.stringify(requiredMarkers)
  ) {
    problems.push(`${label} requiredMarkers must match ${profile.key} profile`);
  }
  const sdks = readFirstRecord(record, "sdks");
  if (!sdks) {
    problems.push(`${label} sdks block is required`);
  } else {
    const knownSdkFields = new Set(["implementation", "implementationHash", "files"]);
    const knownFileFields = new Set(["path", "sha256", "sizeBytes", "markers"]);
    const expectedSpecs = sourceParitySdkSpecsForProfile(profile.key);
    const expectedSdkNames = Object.keys(
      SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
    ).sort();
    const actualSdkNames = Object.keys(sdks).sort();
    if (JSON.stringify(actualSdkNames) !== JSON.stringify(expectedSdkNames)) {
      problems.push(`${label} sdks must contain exactly ${expectedSdkNames.join(", ")}`);
    }
    for (const sdk of expectedSdkNames) {
      const sdkRecord = readFirstRecord(sdks, sdk);
      const expectedImplementation =
        SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1[sdk];
      const expectedSpec = expectedSpecs[sdk];
      if (!sdkRecord) {
        problems.push(`${label} ${sdk} SDK record is required`);
        continue;
      }
      for (const key of Object.keys(sdkRecord)) {
        if (!knownSdkFields.has(key)) {
          problems.push(`${label} ${sdk} contains unknown field: ${key}`);
        }
      }
      if (readFirstString(sdkRecord, "implementation") !== expectedImplementation) {
        problems.push(
          `${label} ${sdk} implementation must be ${expectedImplementation}`,
        );
      }
      const files = readFirstValue(sdkRecord, "files");
      const implementationHashFiles = [];
      if (!Array.isArray(files) || files.length !== expectedSpec.files.length) {
        problems.push(`${label} ${sdk} files must match source parity spec`);
      } else {
        for (const [index, expectedFile] of expectedSpec.files.entries()) {
          const file = files[index];
          if (!isRecord(file)) {
            problems.push(`${label} ${sdk} files[${index}] must be an object`);
            continue;
          }
          for (const key of Object.keys(file)) {
            if (!knownFileFields.has(key)) {
              problems.push(
                `${label} ${sdk} files[${index}] contains unknown field: ${key}`,
              );
            }
          }
          const filePath = readFirstString(file, "path");
          if (filePath !== expectedFile.path) {
            problems.push(
              `${label} ${sdk} files[${index}].path must be ${expectedFile.path}`,
            );
          }
          let fileHash = "";
          try {
            fileHash = normalizeCanonicalHex32(
              readFirstValue(file, "sha256"),
              `${label} ${sdk} ${expectedFile.path} sha256`,
            );
          } catch (error) {
            problems.push(error instanceof Error ? error.message : String(error));
          }
          const sizeBytes = Number(readFirstValue(file, "sizeBytes"));
          if (!Number.isSafeInteger(sizeBytes) || sizeBytes <= 0) {
            problems.push(`${label} ${sdk} ${expectedFile.path} sizeBytes is required`);
          }
          const fileMarkers = readFirstValue(file, "markers");
          if (
            !Array.isArray(fileMarkers) ||
            JSON.stringify(fileMarkers) !== JSON.stringify(expectedFile.markers)
          ) {
            problems.push(
              `${label} ${sdk} ${expectedFile.path} markers must match source parity spec`,
            );
          }
          implementationHashFiles.push({
            path: filePath,
            sha256: fileHash,
            markers: Array.isArray(fileMarkers) ? fileMarkers : [],
          });
        }
      }
      try {
        const expectedImplementationHash = bytesToHex(
          sha256(
            textEncoder.encode(
              stableJsonString({
                sdk,
                implementation: expectedImplementation,
                files: implementationHashFiles,
              }),
            ),
          ),
        );
        const implementationHash = normalizeCanonicalHex32(
          readFirstValue(sdkRecord, "implementationHash"),
          `${label} ${sdk} implementationHash`,
        );
        if (implementationHash !== expectedImplementationHash) {
          problems.push(`${label} ${sdk} implementationHash must match files`);
        }
      } catch (error) {
        problems.push(error instanceof Error ? error.message : String(error));
      }
    }
  }
  try {
    const declaredTreeHash = normalizeCanonicalHex32(
      readFirstValue(record, "sourceTreeHash"),
      `${label} sourceTreeHash`,
    );
    const actualTreeHash = sourceParityTreeHash(record);
    if (declaredTreeHash !== actualTreeHash) {
      problems.push(`${label} sourceTreeHash must match source parity payload`);
    }
  } catch (error) {
    problems.push(error instanceof Error ? error.message : String(error));
  }
  if (problems.length > 0) {
    throw new Error(`${label} is not production-ready: ${problems.join("; ")}`);
  }
}

function noWasmNoRemoteScanHash(record) {
  return bytesToHex(
    sha256(
      textEncoder.encode(
        stableJsonString({
          schema: record.schema,
          routeId: record.routeId,
          assetKey: record.assetKey,
          bscNetwork: record.bscNetwork,
          chain: record.chain,
          chainIdHex: record.chainIdHex,
          networkIdHex: record.networkIdHex,
          domain: record.domain,
          proofBackend: record.proofBackend,
          proofFamily: record.proofFamily,
          noWasm: record.noWasm,
          remoteProverRequired: record.remoteProverRequired,
          browserImplementation: record.browserImplementation,
          scanResult: record.scanResult,
          forbiddenWasmReferences: record.forbiddenWasmReferences,
          forbiddenRemoteReferences: record.forbiddenRemoteReferences,
          inspectedSdkArtifacts: record.inspectedSdkArtifacts,
        }),
      ),
    ),
  );
}

function validateBscNativeEvmNoWasmNoRemoteScanArtifact({
  artifact,
  profile,
  sdkArtifacts,
}) {
  const label = "auditHashes.no_wasm_no_remote_scan";
  let record;
  try {
    record = parseJsonWithoutDuplicateKeys(artifact.bytes.toString("utf8"), label);
  } catch (error) {
    throw new Error(
      `${label} must be valid duplicate-free JSON: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
  const reason = unsafeSecretReason(record, label);
  if (reason) {
    throw new Error(reason);
  }
  if (!isRecord(record)) {
    throw new Error(`${label} must be a JSON object.`);
  }
  const knownTopLevelFields = new Set([
    "schema",
    "generatedAt",
    "routeId",
    "assetKey",
    "bscNetwork",
    "chain",
    "chainIdHex",
    "networkIdHex",
    "domain",
    "proofBackend",
    "proofFamily",
    "noWasm",
    "remoteProverRequired",
    "browserImplementation",
    "scanResult",
    "forbiddenWasmReferences",
    "forbiddenRemoteReferences",
    "inspectedSdkArtifacts",
    "scanHash",
  ]);
  const problems = [
    ...Object.keys(record)
      .filter((key) => !knownTopLevelFields.has(key))
      .map((key) => `${label} contains unknown field: ${key}`),
    groth16ManifestStringProblem(record, ["schema"], NO_WASM_NO_REMOTE_SCAN_SCHEMA, `${label} schema`),
    groth16ManifestStringProblem(record, ["routeId"], ROUTE_ID, `${label} routeId`),
    groth16ManifestStringProblem(record, ["assetKey"], ASSET_KEY, `${label} assetKey`),
    groth16ManifestStringProblem(record, ["bscNetwork"], profile.key, `${label} bscNetwork`),
    groth16ManifestStringProblem(record, ["chain"], profile.chain, `${label} chain`),
    groth16ManifestStringProblem(record, ["chainIdHex"], profile.chainIdHex, `${label} chainIdHex`),
    groth16ManifestHashProblem(record, ["networkIdHex"], profile.networkIdHex, `${label} networkIdHex`),
    readFirstValue(record, "domain") === SCCP_DOMAIN_BSC
      ? ""
      : `${label} domain must be ${SCCP_DOMAIN_BSC}`,
    groth16ManifestStringProblem(record, ["proofBackend"], BSC_EVM_GROTH16_BACKEND, `${label} proofBackend`),
    groth16ManifestStringProblem(record, ["proofFamily"], SCCP_PROOF_FAMILY_STARK_FRI, `${label} proofFamily`),
    groth16ManifestBooleanProblem(record, ["noWasm"], true, `${label} noWasm`),
    groth16ManifestBooleanProblem(record, ["remoteProverRequired"], false, `${label} remoteProverRequired`),
    groth16ManifestStringProblem(record, ["browserImplementation"], "pure-typescript", `${label} browserImplementation`),
    groth16ManifestStringProblem(record, ["scanResult"], "pass", `${label} scanResult`),
    groth16ManifestIntegerProblem(record, ["forbiddenWasmReferences"], 0, `${label} forbiddenWasmReferences`),
    groth16ManifestIntegerProblem(record, ["forbiddenRemoteReferences"], 0, `${label} forbiddenRemoteReferences`),
  ].filter(Boolean);
  const inspected = readFirstValue(record, "inspectedSdkArtifacts");
  const sdkArtifactsBySdk = new Map((sdkArtifacts ?? []).map((entry) => [entry.sdk, entry]));
  const expectedSdkNames = Object.keys(
    SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1,
  ).sort();
  if (!Array.isArray(inspected) || inspected.length !== expectedSdkNames.length) {
    problems.push(`${label} inspectedSdkArtifacts must cover every required SDK`);
  } else {
    const knownEntryFields = new Set([
      "sdk",
      "implementation",
      "path",
      "sha256",
      "sizeBytes",
      "forbiddenWasmReferences",
      "forbiddenRemoteReferences",
    ]);
    const seen = new Set();
    for (const [index, entry] of inspected.entries()) {
      if (!isRecord(entry)) {
        problems.push(`${label} inspectedSdkArtifacts[${index}] must be an object`);
        continue;
      }
      for (const key of Object.keys(entry)) {
        if (!knownEntryFields.has(key)) {
          problems.push(
            `${label} inspectedSdkArtifacts[${index}] contains unknown field: ${key}`,
          );
        }
      }
      const sdk = readFirstString(entry, "sdk");
      if (!expectedSdkNames.includes(sdk)) {
        problems.push(`${label} inspectedSdkArtifacts[${index}].sdk is not required`);
        continue;
      }
      if (seen.has(sdk)) {
        problems.push(`${label} inspectedSdkArtifacts contains duplicate sdk: ${sdk}`);
      }
      seen.add(sdk);
      const actualArtifact = sdkArtifactsBySdk.get(sdk);
      if (!actualArtifact) {
        problems.push(`${label} ${sdk} SDK artifact is missing from bundle`);
        continue;
      }
      const expectedImplementation =
        SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1[sdk];
      if (readFirstString(entry, "implementation") !== expectedImplementation) {
        problems.push(`${label} ${sdk} implementation must be ${expectedImplementation}`);
      }
      if (readFirstString(entry, "path") !== actualArtifact.path) {
        problems.push(`${label} ${sdk} path must match native SDK artifact`);
      }
      try {
        const hash = normalizeCanonicalHex32(
          readFirstValue(entry, "sha256"),
          `${label} ${sdk} sha256`,
        );
        if (hash !== actualArtifact.sha256) {
          problems.push(`${label} ${sdk} sha256 must match native SDK artifact`);
        }
      } catch (error) {
        problems.push(error instanceof Error ? error.message : String(error));
      }
      const sizeBytes = Number(readFirstValue(entry, "sizeBytes"));
      if (!Number.isSafeInteger(sizeBytes) || sizeBytes !== actualArtifact.sizeBytes) {
        problems.push(`${label} ${sdk} sizeBytes must match native SDK artifact`);
      }
      if (readFirstValue(entry, "forbiddenWasmReferences") !== 0) {
        problems.push(`${label} ${sdk} forbiddenWasmReferences must be 0`);
      }
      if (readFirstValue(entry, "forbiddenRemoteReferences") !== 0) {
        problems.push(`${label} ${sdk} forbiddenRemoteReferences must be 0`);
      }
    }
    for (const sdk of expectedSdkNames) {
      if (!seen.has(sdk)) {
        problems.push(`${label} inspectedSdkArtifacts missing sdk: ${sdk}`);
      }
    }
  }
  try {
    const declaredScanHash = normalizeCanonicalHex32(
      readFirstValue(record, "scanHash"),
      `${label} scanHash`,
    );
    const actualScanHash = noWasmNoRemoteScanHash(record);
    if (declaredScanHash !== actualScanHash) {
      problems.push(`${label} scanHash must match scan payload`);
    }
  } catch (error) {
    problems.push(error instanceof Error ? error.message : String(error));
  }
  if (problems.length > 0) {
    throw new Error(`${label} is not production-ready: ${problems.join("; ")}`);
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

function runCommand(command, args, options = {}) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(command, args, {
      cwd: options.cwd,
      env: options.env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString("utf8");
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString("utf8");
    });
    child.on("error", (error) => {
      reject(error);
    });
    child.on("close", (code) => {
      if (code === 0) {
        resolvePromise({ stdout, stderr });
        return;
      }
      const output = `${stdout}\n${stderr}`
        .trim()
        .split(/\n/u)
        .slice(-30)
        .join("\n");
      reject(
        new Error(
          `${command} ${args.join(" ")} failed with exit ${code}${
            output ? `:\n${output}` : ""
          }`,
        ),
      );
    });
  });
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

function generatedEvidenceReferencePath(value, evidenceOut, label) {
  const source = normalizeNonEmptyText(value, label);
  if (
    source.includes("\0") ||
    /^[a-z][a-z0-9+.-]*:/iu.test(source) ||
    /[?#]/u.test(source)
  ) {
    throw new Error(`${label} must be a relative artifact path.`);
  }
  const normalizedSource = source.replace(/\\/gu, "/");
  const sourceParts = normalizedSource.split("/");
  if (
    !isAbsolute(source) &&
    !win32.isAbsolute(source) &&
    sourceParts.every((part) => part && part !== "." && part !== "..")
  ) {
    return normalizedSource;
  }

  const resolvedSource = resolve(source);
  const baseDirs = [
    resolve(),
    REPO_ROOT,
    evidenceOut ? dirname(resolve(evidenceOut)) : "",
  ].filter(Boolean);
  for (const baseDir of [...new Set(baseDirs)]) {
    if (!pathIsWithin(resolvedSource, baseDir)) {
      continue;
    }
    const relativePath = relative(baseDir, resolvedSource)
      .split(/[\\/]+/u)
      .join("/");
    if (
      relativePath &&
      relativePath
        .split("/")
        .every((part) => part && part !== "." && part !== "..")
    ) {
      return relativePath;
    }
  }

  if (evidenceOut && isCanonicalBscProductionArtifactPath(evidenceOut)) {
    return source;
  }

  throw new Error(
    `${label} must be relative to the working tree or evidence output.`,
  );
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

function bscOfflineFullTomlEvidencePathProblem(value, label) {
  let normalized = "";
  try {
    normalized = normalizeNonEmptyText(value, label);
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  if (normalized.includes("\0")) {
    return `${label} must not contain NUL bytes`;
  }
  if (/[?#]/u.test(normalized)) {
    return `${label} must not contain query strings or fragments`;
  }
  if (/^[a-z][a-z0-9+.-]*:/iu.test(normalized)) {
    return `${label} must not be a URL or URI`;
  }
  if (normalized.includes("\\")) {
    return `${label} must use POSIX separators`;
  }
  if (/%[0-9a-f]{2}/iu.test(normalized)) {
    return `${label} must not contain percent-encoded path segments`;
  }
  if (pathHasDecodedParentSegment(normalized)) {
    return `${label} must not contain encoded traversal or malformed percent escapes`;
  }
  if (isCanonicalBscProductionArtifactPath(normalized)) {
    return "";
  }
  if (isAbsolute(normalized) || win32.isAbsolute(normalized)) {
    return `${label} must be a safe relative path or canonical BSC production artifact path`;
  }
  const segments = normalized.split("/");
  if (!segments.every((segment) => segment && segment !== "." && segment !== "..")) {
    return `${label} must not contain empty, current-directory, or parent-directory segments`;
  }
  return "";
}

function assertBundleArtifactPathSafe(pathName, label) {
  if (pathHasDecodedParentSegment(pathName)) {
    throw new Error(
      `${label} must not use URL-encoded parent-directory segments.`,
    );
  }
}

function nativeBundleArtifactPathProblem(value, label) {
  let normalized = "";
  try {
    normalized = normalizeNonEmptyText(value, label);
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  if (normalized.includes("\0")) {
    return `${label} must not contain NUL bytes`;
  }
  if (/^[a-z][a-z0-9+.-]*:/iu.test(normalized)) {
    return `${label} must be a relative artifact path, not a URL or URI`;
  }
  if (/[?#]/u.test(normalized)) {
    return `${label} must not contain query strings or fragments`;
  }
  if (normalized.includes("\\")) {
    return `${label} must use POSIX separators`;
  }
  if (/%[0-9a-f]{2}/iu.test(normalized)) {
    return `${label} must not contain percent-encoded path segments`;
  }
  if (isAbsolute(normalized) || win32.isAbsolute(normalized)) {
    return `${label} must be a relative artifact path under artifact-root`;
  }
  const segments = normalized.split("/");
  if (!segments.every((segment) => segment && segment !== "." && segment !== "..")) {
    return `${label} must not contain empty, current-directory, or parent-directory segments`;
  }
  if (pathHasDecodedParentSegment(normalized)) {
    return `${label} must not contain encoded traversal or malformed percent escapes`;
  }
  return "";
}

async function readArtifactUnderRoot(root, value, label) {
  const text = normalizeNonEmptyText(value, label);
  const pathProblem = nativeBundleArtifactPathProblem(text, label);
  if (pathProblem) {
    throw new Error(pathProblem);
  }
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

async function normalizeAuditHashOrFile(root, value, label, options = {}) {
  const text = normalizeNonEmptyText(value, label);
  if (/^(?:0x)?[0-9a-f]{64}$/iu.test(text)) {
    return assertProductionAuditHashLiteral(text, label);
  }
  if (text.startsWith("0x")) {
    throw new Error(`${label} must be a 32-byte hex hash or artifact file.`);
  }
  const artifact = await readArtifactUnderRoot(root, text, label);
  options.validateArtifact?.(artifact);
  return artifact.sha256;
}

async function readNativeProverAuditHashes(
  root,
  options,
  { parityFixture, selfTestFixture, profile, sdkArtifacts } = {},
) {
  const entries = [];
  for (const [key, optionNames] of Object.entries(
    NATIVE_EVM_PROVER_AUDIT_OPTION_KEYS,
  )) {
    const derived =
      key === "cross_sdk_parity"
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
      {
        validateArtifact:
          key === "native_implementation_audit" && profile
            ? (artifact) =>
                validateBscNativeEvmSourceParityAttestationArtifact(
                  artifact,
                  profile,
                )
            : key === "no_wasm_no_remote_scan" && profile
              ? (artifact) =>
                  validateBscNativeEvmNoWasmNoRemoteScanArtifact({
                    artifact,
                    profile,
                    sdkArtifacts,
                  })
            : null,
      },
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
  const routeManifestAliases = ["route-manifest", "manifest"];
  const evidenceAliases = ["evidence", "deployment-evidence"];
  const presentRouteManifestAliases = routeManifestAliases.filter((key) =>
    ownValue(options, key) !== undefined,
  );
  const presentEvidenceAliases = evidenceAliases.filter((key) =>
    ownValue(options, key) !== undefined,
  );
  if (presentRouteManifestAliases.length > 1) {
    throw new Error(
      `native-prover-bundle route manifest source must not use multiple aliases: ${presentRouteManifestAliases.join(", ")}.`,
    );
  }
  if (presentEvidenceAliases.length > 1) {
    throw new Error(
      `native-prover-bundle deployment evidence source must not use multiple aliases: ${presentEvidenceAliases.join(", ")}.`,
    );
  }
  const routeManifestPath =
    ownValue(options, presentRouteManifestAliases[0]) ?? null;
  const evidencePath = ownValue(options, presentEvidenceAliases[0]) ?? null;
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

function groth16MaterialManifestHash(record, keys, label) {
  const entries = collectStringEntries(record, keys, label);
  if (entries.length > 1) {
    throw new Error(
      `${label} must not use multiple aliases: ${entries
        .map((entry) => entry.key)
        .join(", ")}.`,
    );
  }
  if (entries.length === 0) {
    throw new Error(`${label} is required.`);
  }
  return normalizeCanonicalHex32(entries[0].value, label);
}

function groth16MaterialManifestArtifactHash(manifest, artifactKeys, label) {
  const artifacts = readFirstRecord(manifest, "artifacts") ?? {};
  const artifactAliasProblem = groth16ManifestAliasProblem(
    artifacts,
    artifactKeys,
    `Groth16 material manifest ${label}`,
  );
  if (artifactAliasProblem) {
    throw new Error(artifactAliasProblem);
  }
  const artifact = readFirstRecord(artifacts, ...artifactKeys) ?? {};
  return groth16MaterialManifestHash(
    artifact,
    ["sha256", "hash", "artifactHash", "artifact_hash"],
    `Groth16 material manifest ${label} sha256`,
  );
}

function groth16MaterialManifestArtifactPath(manifest, artifactKeys, label) {
  const artifacts = readFirstRecord(manifest, "artifacts") ?? {};
  const artifactAliasProblem = groth16ManifestAliasProblem(
    artifacts,
    artifactKeys,
    `Groth16 material manifest ${label}`,
  );
  if (artifactAliasProblem) {
    throw new Error(artifactAliasProblem);
  }
  const artifact = readFirstRecord(artifacts, ...artifactKeys) ?? {};
  const path = readFirstString(artifact, "path");
  if (!path) {
    throw new Error(`Groth16 material manifest ${label} path is required.`);
  }
  return path;
}

function groth16ManifestAliasProblem(record, keys, label) {
  if (!isRecord(record)) {
    return "";
  }
  const presentKeys = keys.filter((key) => hasOwn(record, key));
  return presentKeys.length > 1
    ? `${label} must not use multiple aliases: ${presentKeys.join(", ")}`
    : "";
}

function groth16ManifestStringProblem(record, keys, expected, label) {
  const aliasProblem = groth16ManifestAliasProblem(record, keys, label);
  if (aliasProblem) {
    return aliasProblem;
  }
  const actual = readFirstString(record, ...keys);
  return actual === expected ? "" : `${label} must be ${expected}`;
}

function groth16ManifestIntegerProblem(record, keys, expected, label) {
  const aliasProblem = groth16ManifestAliasProblem(record, keys, label);
  if (aliasProblem) {
    return aliasProblem;
  }
  const actual = Number(readFirstValue(record, ...keys));
  return Number.isSafeInteger(actual) && actual === expected
    ? ""
    : `${label} must be ${expected}`;
}

function groth16ManifestIntegerAtLeastProblem(record, keys, expected, label) {
  const aliasProblem = groth16ManifestAliasProblem(record, keys, label);
  if (aliasProblem) {
    return aliasProblem;
  }
  const actual = Number(readFirstValue(record, ...keys));
  return Number.isSafeInteger(actual) && actual >= expected
    ? ""
    : `${label} must be at least ${expected}`;
}

function groth16ManifestBooleanProblem(record, keys, expected, label) {
  const aliasProblem = groth16ManifestAliasProblem(record, keys, label);
  if (aliasProblem) {
    return aliasProblem;
  }
  return readFirstValue(record, ...keys) === expected
    ? ""
    : `${label} must be ${expected}`;
}

function groth16ManifestHashPresentProblem(record, keys, label) {
  try {
    groth16MaterialManifestHash(record, keys, label);
    return "";
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
}

function groth16ManifestHashProblem(record, keys, expected, label) {
  try {
    const actual = groth16MaterialManifestHash(record, keys, label);
    return actual === expected ? "" : `${label} must match ${expected}`;
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
}

function groth16ManifestArrayOrCountAtLeastProblem(
  record,
  arrayKeys,
  countKeys,
  minimum,
  label,
) {
  const arrayAliasProblem = groth16ManifestAliasProblem(record, arrayKeys, label);
  if (arrayAliasProblem) {
    return arrayAliasProblem;
  }
  const countAliasProblem = groth16ManifestAliasProblem(record, countKeys, label);
  if (countAliasProblem) {
    return countAliasProblem;
  }
  for (const key of arrayKeys) {
    const value = readFirstValue(record, key);
    if (Array.isArray(value)) {
      const distinct = new Set(value.map((entry) => trim(entry)).filter(Boolean));
      return distinct.size >= minimum
        ? ""
        : `${label} must record at least ${minimum}`;
    }
  }
  const count = Number(readFirstValue(record, ...countKeys));
  if (Number.isSafeInteger(count)) {
    return count >= minimum ? "" : `${label} must record at least ${minimum}`;
  }
  return `${label} is required`;
}

function groth16ManifestOptionalBooleanProblem(record, keys, expected, label) {
  const aliasProblem = groth16ManifestAliasProblem(record, keys, label);
  if (aliasProblem) {
    return aliasProblem;
  }
  const value = readFirstValue(record, ...keys);
  return value === undefined ? "" : value === expected ? "" : `${label} must be ${expected}`;
}

function groth16ManifestOptionalStringProblem(record, keys, expected, label) {
  const aliasProblem = groth16ManifestAliasProblem(record, keys, label);
  if (aliasProblem) {
    return aliasProblem;
  }
  const value = readFirstValue(record, ...keys);
  if (value === undefined) {
    return "";
  }
  return groth16ManifestStringProblem(record, keys, expected, label);
}

function groth16ManifestOptionalIntegerProblem(record, keys, expected, label) {
  const aliasProblem = groth16ManifestAliasProblem(record, keys, label);
  if (aliasProblem) {
    return aliasProblem;
  }
  const value = readFirstValue(record, ...keys);
  if (value === undefined) {
    return "";
  }
  return groth16ManifestIntegerProblem(record, keys, expected, label);
}

function bscGroth16MaterialManifestShapeProblems(manifest) {
  const problems = [
    ...unknownGroth16ProofSelfTestFields(
      manifest,
      new Set([
        "schema",
        "generatedAt",
        "generated_at",
        "routeId",
        "route_id",
        "assetKey",
        "asset_key",
        "bscNetwork",
        "bsc_network",
        "network",
        "chain",
        "chainIdHex",
        "chain_id_hex",
        "networkIdHex",
        "network_id_hex",
        "proofBackend",
        "proof_backend",
        "proofFamily",
        "proof_family",
        "sourceDomain",
        "source_domain",
        "targetDomain",
        "target_domain",
        "circuitProfile",
        "circuit_profile",
        "publicInputCount",
        "public_input_count",
        "publicSignalNames",
        "public_signal_names",
        "verifierKeyHash",
        "verifier_key_hash",
        "proofArtifactHash",
        "proof_artifact_hash",
        "provingKeyHash",
        "proving_key_hash",
        "productionReady",
        "production_ready",
        "productionBlockers",
        "production_blockers",
        "artifacts",
        "trustedSetup",
        "trusted_setup",
        "selfChecks",
        "self_checks",
        "attestationTrustPolicy",
        "attestation_trust_policy",
        "attestations",
        "nextStep",
        "next_step",
      ]),
      "Groth16 material manifest",
    ),
    ...groth16ProofSelfTestAliasProblems(
      manifest,
      [
        ["generatedAt", ["generatedAt", "generated_at"]],
        ["routeId", ["routeId", "route_id"]],
        ["assetKey", ["assetKey", "asset_key"]],
        ["bscNetwork", ["bscNetwork", "bsc_network", "network"]],
        ["chainIdHex", ["chainIdHex", "chain_id_hex"]],
        ["networkIdHex", ["networkIdHex", "network_id_hex"]],
        ["proofBackend", ["proofBackend", "proof_backend"]],
        ["proofFamily", ["proofFamily", "proof_family"]],
        ["sourceDomain", ["sourceDomain", "source_domain"]],
        ["targetDomain", ["targetDomain", "target_domain"]],
        ["circuitProfile", ["circuitProfile", "circuit_profile"]],
        ["publicInputCount", ["publicInputCount", "public_input_count"]],
        ["publicSignalNames", ["publicSignalNames", "public_signal_names"]],
        ["verifierKeyHash", ["verifierKeyHash", "verifier_key_hash"]],
        ["proofArtifactHash", ["proofArtifactHash", "proof_artifact_hash"]],
        ["provingKeyHash", ["provingKeyHash", "proving_key_hash"]],
        ["productionReady", ["productionReady", "production_ready"]],
        ["productionBlockers", ["productionBlockers", "production_blockers"]],
        ["trustedSetup", ["trustedSetup", "trusted_setup"]],
        ["selfChecks", ["selfChecks", "self_checks"]],
        [
          "attestationTrustPolicy",
          ["attestationTrustPolicy", "attestation_trust_policy"],
        ],
        ["nextStep", ["nextStep", "next_step"]],
      ],
      "Groth16 material manifest",
    ),
  ];

  const artifacts = readFirstValue(manifest, "artifacts");
  problems.push(
    ...unknownGroth16ProofSelfTestFields(
      artifacts,
      new Set([
        "circuitSource",
        "circuit_source",
        "r1cs",
        "provingKey",
        "proving_key",
        "snarkjsVerificationKey",
        "snarkjs_verification_key",
        "bscVerifierKey",
        "bsc_verifier_key",
        "witnessWasm",
        "witness_wasm",
        "symbols",
        "powersOfTau",
        "powers_of_tau",
        "trustedSetupTranscript",
        "trusted_setup_transcript",
        "reproducibleBuildTranscript",
        "reproducible_build_transcript",
      ]),
      "Groth16 material manifest artifacts",
    ),
    ...groth16ProofSelfTestAliasProblems(
      artifacts,
      [
        ["circuitSource", ["circuitSource", "circuit_source"]],
        ["provingKey", ["provingKey", "proving_key"]],
        [
          "snarkjsVerificationKey",
          ["snarkjsVerificationKey", "snarkjs_verification_key"],
        ],
        ["bscVerifierKey", ["bscVerifierKey", "bsc_verifier_key"]],
        ["witnessWasm", ["witnessWasm", "witness_wasm"]],
        ["powersOfTau", ["powersOfTau", "powers_of_tau"]],
        [
          "trustedSetupTranscript",
          ["trustedSetupTranscript", "trusted_setup_transcript"],
        ],
        [
          "reproducibleBuildTranscript",
          ["reproducibleBuildTranscript", "reproducible_build_transcript"],
        ],
      ],
      "Groth16 material manifest artifacts",
    ),
  );
  if (isRecord(artifacts)) {
    for (const [keys, label] of [
      [["circuitSource", "circuit_source"], "circuit source artifact"],
      [["r1cs"], "R1CS artifact"],
      [["provingKey", "proving_key"], "proving key artifact"],
      [
        ["snarkjsVerificationKey", "snarkjs_verification_key"],
        "SnarkJS verification key artifact",
      ],
      [["bscVerifierKey", "bsc_verifier_key"], "BSC verifier key artifact"],
      [["witnessWasm", "witness_wasm"], "witness WASM artifact"],
      [["symbols"], "symbols artifact"],
      [["powersOfTau", "powers_of_tau"], "Powers of Tau artifact"],
      [
        ["trustedSetupTranscript", "trusted_setup_transcript"],
        "trusted setup transcript artifact",
      ],
      [
        ["reproducibleBuildTranscript", "reproducible_build_transcript"],
        "reproducible build transcript artifact",
      ],
    ]) {
      const artifact = readFirstValue(artifacts, ...keys);
      problems.push(
        ...unknownGroth16ProofSelfTestFields(
          artifact,
          new Set(["path", "sha256", "hash", "artifactHash", "artifact_hash"]),
          `Groth16 material manifest ${label}`,
        ),
        ...groth16ProofSelfTestAliasProblems(
          artifact,
          [["sha256", ["sha256", "hash", "artifactHash", "artifact_hash"]]],
          `Groth16 material manifest ${label}`,
        ),
      );
    }
  }

  const trustedSetup = readFirstValue(
    manifest,
    "trustedSetup",
    "trusted_setup",
  );
  problems.push(
    ...unknownGroth16ProofSelfTestFields(
      trustedSetup,
      new Set([
        "localPowersOfTau",
        "local_powers_of_tau",
        "localPhase2Contribution",
        "local_phase2_contribution",
        "contributionMaterialPersisted",
        "contribution_material_persisted",
      ]),
      "Groth16 material manifest trustedSetup",
    ),
    ...groth16ProofSelfTestAliasProblems(
      trustedSetup,
      [
        ["localPowersOfTau", ["localPowersOfTau", "local_powers_of_tau"]],
        [
          "localPhase2Contribution",
          ["localPhase2Contribution", "local_phase2_contribution"],
        ],
        [
          "contributionMaterialPersisted",
          ["contributionMaterialPersisted", "contribution_material_persisted"],
        ],
      ],
      "Groth16 material manifest trustedSetup",
    ),
  );

  const selfChecks = readFirstValue(manifest, "selfChecks", "self_checks");
  problems.push(
    ...unknownGroth16ProofSelfTestFields(
      selfChecks,
      new Set(["snarkjs", "snark_js", "circuitSource", "circuit_source"]),
      "Groth16 material manifest selfChecks",
    ),
    ...groth16ProofSelfTestAliasProblems(
      selfChecks,
      [
        ["snarkjs", ["snarkjs", "snark_js"]],
        ["circuitSource", ["circuitSource", "circuit_source"]],
      ],
      "Groth16 material manifest selfChecks",
    ),
  );
  const snarkjs = isRecord(selfChecks)
    ? readFirstValue(selfChecks, "snarkjs", "snark_js")
    : null;
  problems.push(
    ...unknownGroth16ProofSelfTestFields(
      snarkjs,
      new Set([
        "snarkjsBinary",
        "snarkjs_binary",
        "r1csInfo",
        "r1cs_info",
        "r1csInfoSource",
        "r1cs_info_source",
        "r1csInfoError",
        "r1cs_info_error",
        "r1csConstraintCount",
        "r1cs_constraint_count",
        "r1csPublicInputCount",
        "r1cs_public_input_count",
        "r1csBinaryHeader",
        "r1cs_binary_header",
        "zkeyVerify",
        "zkey_verify",
        "zkeyVerifyResult",
        "zkey_verify_result",
        "zkeyVerifyError",
        "zkey_verify_error",
        "zkeyVerificationKeyExport",
        "zkey_verification_key_export",
        "verifierKeyHashMatches",
        "verifier_key_hash_matches",
        "exportedVerifierKeyHash",
        "exported_verifier_key_hash",
      ]),
      "Groth16 material manifest selfChecks.snarkjs",
    ),
    ...groth16ProofSelfTestAliasProblems(
      snarkjs,
      [
        ["snarkjsBinary", ["snarkjsBinary", "snarkjs_binary"]],
        ["r1csInfo", ["r1csInfo", "r1cs_info"]],
        ["r1csInfoSource", ["r1csInfoSource", "r1cs_info_source"]],
        ["r1csInfoError", ["r1csInfoError", "r1cs_info_error"]],
        ["r1csConstraintCount", ["r1csConstraintCount", "r1cs_constraint_count"]],
        ["r1csPublicInputCount", ["r1csPublicInputCount", "r1cs_public_input_count"]],
        ["r1csBinaryHeader", ["r1csBinaryHeader", "r1cs_binary_header"]],
        ["zkeyVerify", ["zkeyVerify", "zkey_verify"]],
        ["zkeyVerifyResult", ["zkeyVerifyResult", "zkey_verify_result"]],
        ["zkeyVerifyError", ["zkeyVerifyError", "zkey_verify_error"]],
        [
          "zkeyVerificationKeyExport",
          ["zkeyVerificationKeyExport", "zkey_verification_key_export"],
        ],
        [
          "verifierKeyHashMatches",
          ["verifierKeyHashMatches", "verifier_key_hash_matches"],
        ],
        [
          "exportedVerifierKeyHash",
          ["exportedVerifierKeyHash", "exported_verifier_key_hash"],
        ],
      ],
      "Groth16 material manifest selfChecks.snarkjs",
    ),
  );
  const circuitSource = isRecord(selfChecks)
    ? readFirstValue(selfChecks, "circuitSource", "circuit_source")
    : null;
  problems.push(
    ...unknownGroth16ProofSelfTestFields(
      circuitSource,
      new Set([
        "fullMessageCircuit",
        "full_message_circuit",
        "signalBindingFixture",
        "signal_binding_fixture",
        "unresolvedPlaceholders",
        "unresolved_placeholders",
        "keccakPublicSignalDerivation",
        "keccak_public_signal_derivation",
        "digestReductionModuloScalarField",
        "digest_reduction_modulo_scalar_field",
        "valueBitBooleanConstraints",
        "value_bit_boolean_constraints",
        "publicSignalConstraintCount",
        "public_signal_constraint_count",
        "labelBindingCount",
        "label_binding_count",
      ]),
      "Groth16 material manifest selfChecks.circuitSource",
    ),
    ...groth16ProofSelfTestAliasProblems(
      circuitSource,
      [
        ["fullMessageCircuit", ["fullMessageCircuit", "full_message_circuit"]],
        [
          "signalBindingFixture",
          ["signalBindingFixture", "signal_binding_fixture"],
        ],
        [
          "unresolvedPlaceholders",
          ["unresolvedPlaceholders", "unresolved_placeholders"],
        ],
        [
          "keccakPublicSignalDerivation",
          ["keccakPublicSignalDerivation", "keccak_public_signal_derivation"],
        ],
        [
          "digestReductionModuloScalarField",
          [
            "digestReductionModuloScalarField",
            "digest_reduction_modulo_scalar_field",
          ],
        ],
        [
          "valueBitBooleanConstraints",
          ["valueBitBooleanConstraints", "value_bit_boolean_constraints"],
        ],
        [
          "publicSignalConstraintCount",
          ["publicSignalConstraintCount", "public_signal_constraint_count"],
        ],
        ["labelBindingCount", ["labelBindingCount", "label_binding_count"]],
      ],
      "Groth16 material manifest selfChecks.circuitSource",
    ),
  );

  const trustPolicy = readFirstValue(
    manifest,
    "attestationTrustPolicy",
    "attestation_trust_policy",
  );
  problems.push(
    ...unknownGroth16ProofSelfTestFields(
      trustPolicy,
      new Set([
        "signatureSchema",
        "signature_schema",
        "requiredAlgorithm",
        "required_algorithm",
        "trustedSignerFingerprints",
        "trusted_signer_fingerprints",
      ]),
      "Groth16 material manifest attestationTrustPolicy",
    ),
    ...groth16ProofSelfTestAliasProblems(
      trustPolicy,
      [
        ["signatureSchema", ["signatureSchema", "signature_schema"]],
        ["requiredAlgorithm", ["requiredAlgorithm", "required_algorithm"]],
        [
          "trustedSignerFingerprints",
          ["trustedSignerFingerprints", "trusted_signer_fingerprints"],
        ],
      ],
      "Groth16 material manifest attestationTrustPolicy",
    ),
  );

  const attestations = readFirstValue(manifest, "attestations");
  problems.push(
    ...unknownGroth16ProofSelfTestFields(
      attestations,
      new Set([
        "semanticSccpCircuit",
        "circuitSecurity",
        "trustedSetup",
        "reproducibleBuild",
      ]),
      "Groth16 material manifest attestations",
    ),
  );
  if (isRecord(attestations)) {
    for (const [key, label] of [
      ["semanticSccpCircuit", "semantic SCCP circuit attestation reference"],
      ["circuitSecurity", "circuit security attestation reference"],
      ["trustedSetup", "trusted setup attestation reference"],
      ["reproducibleBuild", "reproducible build attestation reference"],
    ]) {
      const reference = readFirstValue(attestations, key);
      problems.push(
        ...unknownGroth16ProofSelfTestFields(
          reference,
          new Set([
            "path",
            "sha256",
            "attestationHash",
            "attestation_hash",
            "schema",
            "signature",
            "readError",
            "read_error",
          ]),
          `Groth16 material manifest ${label}`,
        ),
        ...groth16ProofSelfTestAliasProblems(
          reference,
          [
            ["sha256", ["sha256", "attestationHash", "attestation_hash"]],
            ["readError", ["readError", "read_error"]],
          ],
          `Groth16 material manifest ${label}`,
        ),
      );
      const signature = isRecord(reference)
        ? readFirstValue(reference, "signature")
        : null;
      problems.push(
        ...unknownGroth16ProofSelfTestFields(
          signature,
          new Set([
            "verified",
            "algorithm",
            "signerFingerprint",
            "signer_fingerprint",
            "signedPayloadSha256",
            "signed_payload_sha256",
          ]),
          `Groth16 material manifest ${label} signature`,
        ),
        ...groth16ProofSelfTestAliasProblems(
          signature,
          [
            [
              "signerFingerprint",
              ["signerFingerprint", "signer_fingerprint"],
            ],
            [
              "signedPayloadSha256",
              ["signedPayloadSha256", "signed_payload_sha256"],
            ],
          ],
          `Groth16 material manifest ${label} signature`,
        ),
      );
    }
  }

  return problems;
}

function productionEvidenceTextProblems(value, label, path = "") {
  const problems = [];
  if (typeof value === "string") {
    const scanValue =
      path.endsWith(".path") || path === ".path"
        ? basename(win32.basename(value))
        : value;
    if (PRODUCTION_EVIDENCE_FORBIDDEN_WORDS.test(scanValue)) {
      problems.push(
        `${label}${path} must not reference diagnostic, fixture, mock, placeholder, sample, stub, or test-only material`,
      );
    }
    return problems;
  }
  if (Array.isArray(value)) {
    for (const [index, entry] of value.entries()) {
      problems.push(
        ...productionEvidenceTextProblems(entry, label, `${path}[${index}]`),
      );
    }
    return problems;
  }
  if (isRecord(value)) {
    for (const [key, entry] of Object.entries(value)) {
      problems.push(
        ...productionEvidenceTextProblems(
          entry,
          label,
          path ? `${path}.${key}` : `.${key}`,
        ),
      );
    }
  }
  return problems;
}

function groth16MaterialManifestReferenceProblems(manifest) {
  const artifacts = readFirstRecord(manifest, "artifacts");
  const attestations = readFirstRecord(manifest, "attestations");
  return [
    ...(artifacts
      ? productionEvidenceTextProblems(
          artifacts,
          "Groth16 material manifest artifacts",
        )
      : []),
    ...(attestations
      ? productionEvidenceTextProblems(
          attestations,
          "Groth16 material manifest attestations",
        )
      : []),
  ];
}

function groth16AttestationAllowedFields(expectedSchema) {
  const common = [
    "schema",
    "routeId",
    "route_id",
    "assetKey",
    "asset_key",
    "bscNetwork",
    "bsc_network",
    "network",
    "chain",
    "chainIdHex",
    "chain_id_hex",
    "networkIdHex",
    "network_id_hex",
    "proofBackend",
    "proof_backend",
    "proofFamily",
    "proof_family",
    "circuitProfile",
    "circuit_profile",
    "publicInputCount",
    "public_input_count",
    "publicSignalNames",
    "public_signal_names",
    "verifierKeyHash",
    "verifier_key_hash",
    "circuitSourceSha256",
    "circuit_source_sha256",
    "r1csSha256",
    "r1cs_sha256",
      "proofArtifactHash",
      "proof_artifact_hash",
      "powersOfTauSha256",
      "powers_of_tau_sha256",
      "ptauSha256",
      "ptau_sha256",
      "provingKeySha256",
      "proving_key_sha256",
    "provingKeyHash",
    "proving_key_hash",
    "snarkjsVerificationKeySha256",
    "snarkjs_verification_key_sha256",
    "bscVerifierKeySha256",
    "bsc_verifier_key_sha256",
    "signature",
  ];
  const bySchema = {
    [BSC_GROTH16_SEMANTIC_ATTESTATION_SCHEMA]: [
      "semanticReviewEvidenceSchema",
      "semantic_review_evidence_schema",
      "semanticReviewEvidenceSha256",
      "semantic_review_evidence_sha256",
      "semanticReviewReportSha256",
      "semantic_review_report_sha256",
      "fullSccpMessageSemantics",
      "full_sccp_message_semantics",
      "sourceFinalitySemantics",
      "source_finality_semantics",
      "destinationBindingSemantics",
      "destination_binding_semantics",
      "publicSignalDerivationSemantics",
      "public_signal_derivation_semantics",
      "negativeCaseCoverage",
      "negative_case_coverage",
    ],
    [BSC_GROTH16_CIRCUIT_SECURITY_ATTESTATION_SCHEMA]: [
      "circuitSecurityAuditEvidenceSchema",
      "circuit_security_audit_evidence_schema",
      "circuitSecurityAuditEvidenceSha256",
      "circuit_security_audit_evidence_sha256",
      "circuitSecurityAuditReportSha256",
      "circuit_security_audit_report_sha256",
      "auditResult",
      "audit_result",
      "approved",
      "productionApproved",
      "production_approved",
      "criticalFindings",
      "critical_findings",
      "highFindings",
      "high_findings",
      "unresolvedFindings",
      "unresolved_findings",
    ],
    [BSC_GROTH16_TRUSTED_SETUP_ATTESTATION_SCHEMA]: [
      "ceremonyResult",
      "ceremony_result",
      "localSingleContributor",
      "local_single_contributor",
      "minimumContributors",
      "minimum_contributors",
      "toxicWasteDestroyed",
      "toxic_waste_destroyed",
      "contributionTranscriptSha256",
      "contribution_transcript_sha256",
    ],
    [BSC_GROTH16_REPRODUCIBLE_BUILD_ATTESTATION_SCHEMA]: [
      "reproducible",
      "reproducibleBuild",
      "reproducible_build",
      "independentRebuilders",
      "independent_rebuilders",
      "buildTranscriptSha256",
      "build_transcript_sha256",
      "toolchainSha256",
      "toolchain_sha256",
      "r1csInfoSource",
      "r1cs_info_source",
      "r1csPublicInputCount",
      "r1cs_public_input_count",
      "r1csConstraintCount",
      "r1cs_constraint_count",
      "zkeyVerify",
      "zkey_verify",
      "zkeyVerifyResult",
      "zkey_verify_result",
      "zkeyVerificationKeyExport",
      "zkey_verification_key_export",
      "verifierKeyHashMatches",
      "verifier_key_hash_matches",
      "exportedVerifierKeyHash",
      "exported_verifier_key_hash",
    ],
  };
  return new Set([...common, ...(bySchema[expectedSchema] ?? [])]);
}

function groth16AttestationUnknownFieldProblems(record, expectedSchema, label) {
  const allowed = groth16AttestationAllowedFields(expectedSchema);
  return Object.keys(record)
    .filter((key) => !allowed.has(key))
    .map((key) => `${label} contains unknown field: ${key}`);
}

function groth16PublicSignalsProblem(record, label) {
  const aliasProblem = groth16ManifestAliasProblem(
    record,
    ["publicSignalNames", "public_signal_names"],
    `${label} publicSignalNames`,
  );
  if (aliasProblem) {
    return aliasProblem;
  }
  const actual = readFirstValue(
    record,
    "publicSignalNames",
    "public_signal_names",
  );
  if (
    !Array.isArray(actual) ||
    JSON.stringify(actual) !== JSON.stringify(BSC_GROTH16_PUBLIC_SIGNAL_NAMES)
  ) {
    return `${label} publicSignalNames must match BSC Groth16 public signals`;
  }
  return "";
}

function groth16MaterialManifestAttestationProblems(manifest) {
  const attestations = readFirstRecord(manifest, "attestations") ?? {};
  const required = [
    [
      "semanticSccpCircuit",
      BSC_GROTH16_SEMANTIC_ATTESTATION_SCHEMA,
      "semantic SCCP circuit",
    ],
    [
      "circuitSecurity",
      BSC_GROTH16_CIRCUIT_SECURITY_ATTESTATION_SCHEMA,
      "circuit security",
    ],
    [
      "trustedSetup",
      BSC_GROTH16_TRUSTED_SETUP_ATTESTATION_SCHEMA,
      "trusted setup",
    ],
    [
      "reproducibleBuild",
      BSC_GROTH16_REPRODUCIBLE_BUILD_ATTESTATION_SCHEMA,
      "reproducible build",
    ],
  ];
  const problems = [];
  const verifiedSigners = [];
  for (const [key, expectedSchema, label] of required) {
    const record = readFirstRecord(attestations, key);
    if (!record) {
      problems.push(`Groth16 material manifest ${label} attestation is required`);
      continue;
    }
    const schema = readFirstString(record, "schema");
    if (schema !== expectedSchema) {
      problems.push(
        `Groth16 material manifest ${label} attestation schema must be ${expectedSchema}`,
      );
    }
    if (!readFirstString(record, "path")) {
      problems.push(
        `Groth16 material manifest ${label} attestation path is required`,
      );
    }
    try {
      groth16MaterialManifestHash(
        record,
        ["sha256", "attestationHash", "attestation_hash"],
        `Groth16 material manifest ${label} attestation sha256`,
      );
    } catch (error) {
      problems.push(error instanceof Error ? error.message : String(error));
    }
    if (readFirstValue(record, "readError", "read_error")) {
      problems.push(
        `Groth16 material manifest ${label} attestation must be readable JSON`,
      );
    }
    const signature = readFirstRecord(record, "signature");
    if (!signature) {
      problems.push(
        `Groth16 material manifest ${label} attestation signature summary is required`,
      );
      continue;
    }
    if (readFirstValue(signature, "verified") !== true) {
      problems.push(
        `Groth16 material manifest ${label} attestation signature must be verified`,
      );
    }
    if (readFirstString(signature, "algorithm") !== "ed25519") {
      problems.push(
        `Groth16 material manifest ${label} attestation signature algorithm must be ed25519`,
      );
    }
    try {
      const signerFingerprint = normalizeCanonicalHex32(
        readFirstValue(signature, "signerFingerprint", "signer_fingerprint"),
        `Groth16 material manifest ${label} attestation signerFingerprint`,
      );
      if (readFirstValue(signature, "verified") === true) {
        verifiedSigners.push([`Groth16 material manifest ${label} attestation`, signerFingerprint]);
      }
    } catch (error) {
      problems.push(error instanceof Error ? error.message : String(error));
    }
    try {
      normalizeCanonicalHex32(
        readFirstValue(signature, "signedPayloadSha256", "signed_payload_sha256"),
        `Groth16 material manifest ${label} attestation signedPayloadSha256`,
      );
    } catch (error) {
      problems.push(error instanceof Error ? error.message : String(error));
    }
  }
  problems.push(...groth16AttestationSignerDiversityProblems(verifiedSigners));
  return problems;
}

function groth16MaterialManifestTrustPolicyProblems(
  manifest,
  trustedSignerFingerprints,
) {
  const policyAliasProblem = groth16ManifestAliasProblem(
    manifest,
    ["attestationTrustPolicy", "attestation_trust_policy"],
    "Groth16 material manifest attestationTrustPolicy",
  );
  if (policyAliasProblem) {
    return [policyAliasProblem];
  }
  const policy = readFirstRecord(
    manifest,
    "attestationTrustPolicy",
    "attestation_trust_policy",
  );
  if (!policy) {
    return ["Groth16 material manifest attestationTrustPolicy is required"];
  }
  const problems = [];
  problems.push(
    groth16ManifestStringProblem(
      policy,
      ["signatureSchema", "signature_schema"],
      BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA,
      "Groth16 material manifest attestationTrustPolicy signatureSchema",
    ),
    groth16ManifestStringProblem(
      policy,
      ["requiredAlgorithm", "required_algorithm"],
      "ed25519",
      "Groth16 material manifest attestationTrustPolicy requiredAlgorithm",
    ),
  );
  const trustedSignerAliasProblem = groth16ManifestAliasProblem(
    policy,
    ["trustedSignerFingerprints", "trusted_signer_fingerprints"],
    "Groth16 material manifest attestationTrustPolicy trustedSignerFingerprints",
  );
  if (trustedSignerAliasProblem) {
    problems.push(trustedSignerAliasProblem);
  }
  const manifestFingerprints = readFirstValue(
    policy,
    "trustedSignerFingerprints",
    "trusted_signer_fingerprints",
  );
  if (!Array.isArray(manifestFingerprints) || manifestFingerprints.length === 0) {
    problems.push(
      "Groth16 material manifest attestationTrustPolicy trustedSignerFingerprints is required",
    );
    return uniqueNonEmpty(problems);
  }
  let normalizedManifestFingerprints = [];
  try {
    normalizedManifestFingerprints = [
      ...new Set(
        manifestFingerprints.map((value) =>
          normalizeCanonicalHex32(
            value,
            "Groth16 material manifest attestationTrustPolicy trusted signer fingerprint",
          ),
        ),
      ),
    ].sort();
  } catch (error) {
    problems.push(error instanceof Error ? error.message : String(error));
  }
  const normalizedTrusted = [...new Set(trustedSignerFingerprints)].sort();
  if (normalizedTrusted.length === 0) {
    problems.push(
      "native-prover-bundle requires --trusted-attestation-signer for Groth16 material attestations",
    );
  } else if (
    normalizedManifestFingerprints.length > 0 &&
    JSON.stringify(normalizedManifestFingerprints) !== JSON.stringify(normalizedTrusted)
  ) {
    problems.push(
      "Groth16 material manifest attestationTrustPolicy trusted signers must match configured native-prover-bundle trust roots",
    );
  }
  return uniqueNonEmpty(problems);
}

function groth16MaterialManifestSelfCheckProblems(manifest, binding) {
  const problems = [];
  const selfChecks = readFirstRecord(manifest, "selfChecks", "self_checks");
  const snarkjs = selfChecks
    ? readFirstRecord(selfChecks, "snarkjs", "snark_js")
    : null;
  if (!snarkjs) {
    return ["Groth16 material manifest selfChecks.snarkjs is required"];
  }
  const snarkjsBinary = readFirstString(
    snarkjs,
    "snarkjsBinary",
    "snarkjs_binary",
  );
  if (!snarkjsBinary) {
    problems.push(
      "Groth16 material manifest SnarkJS binary command is required",
    );
  }
  if (readFirstValue(snarkjs, "r1csInfo", "r1cs_info") !== true) {
    problems.push("Groth16 material manifest SnarkJS R1CS self-check must pass");
  }
  const r1csInfoSource = readFirstString(
    snarkjs,
    "r1csInfoSource",
    "r1cs_info_source",
  );
  if (!BSC_GROTH16_R1CS_INFO_SOURCES.has(r1csInfoSource)) {
    problems.push(
      `Groth16 material manifest SnarkJS R1CS info source must be one of ${[
        ...BSC_GROTH16_R1CS_INFO_SOURCES,
      ].join(", ")}`,
    );
  }
  const r1csPublicInputCount = Number(
    readFirstValue(
      snarkjs,
      "r1csPublicInputCount",
      "r1cs_public_input_count",
    ),
  );
  if (!Number.isSafeInteger(r1csPublicInputCount) || r1csPublicInputCount !== 9) {
    problems.push(
      "Groth16 material manifest SnarkJS R1CS public input count must be 9",
    );
  }
  const r1csConstraintCount = Number(
    readFirstValue(
      snarkjs,
      "r1csConstraintCount",
      "r1cs_constraint_count",
    ),
  );
  if (
    !Number.isSafeInteger(r1csConstraintCount) ||
    r1csConstraintCount < PRODUCTION_FULL_SCCP_MIN_R1CS_CONSTRAINTS
  ) {
    problems.push(
      `Groth16 material manifest SnarkJS R1CS constraint count must be at least ${PRODUCTION_FULL_SCCP_MIN_R1CS_CONSTRAINTS}`,
    );
  }
  if (
    readFirstValue(
      snarkjs,
      "zkeyVerify",
      "zkey_verify",
    ) !== true
  ) {
    problems.push(
      "Groth16 material manifest SnarkJS zkey verify self-check must pass",
    );
  }
  if (
    readFirstString(
      snarkjs,
      "zkeyVerifyResult",
      "zkey_verify_result",
    ) !== SNARKJS_ZKEY_VERIFY_OK
  ) {
    problems.push(
      `Groth16 material manifest SnarkJS zkey verify result must be ${SNARKJS_ZKEY_VERIFY_OK}`,
    );
  }
  if (
    readFirstValue(
      snarkjs,
      "zkeyVerificationKeyExport",
      "zkey_verification_key_export",
    ) !== true
  ) {
    problems.push(
      "Groth16 material manifest SnarkJS zkey verification-key export must pass",
    );
  }
  if (
    readFirstValue(
      snarkjs,
      "verifierKeyHashMatches",
      "verifier_key_hash_matches",
    ) !== true
  ) {
    problems.push(
      "Groth16 material manifest SnarkJS exported verifier hash must match",
    );
  }
  try {
    const exportedVerifierKeyHash = groth16MaterialManifestHash(
      snarkjs,
      ["exportedVerifierKeyHash", "exported_verifier_key_hash"],
      "Groth16 material manifest SnarkJS exported verifier key hash",
    );
    if (exportedVerifierKeyHash !== binding.verifierKeyHash) {
      problems.push(
        "Groth16 material manifest SnarkJS exported verifier key hash must match route verifier key",
      );
    }
  } catch (error) {
    problems.push(error instanceof Error ? error.message : String(error));
  }
  const circuitSource = selfChecks
    ? readFirstRecord(selfChecks, "circuitSource", "circuit_source")
    : null;
  if (!circuitSource) {
    problems.push("Groth16 material manifest circuitSource self-check is required");
  } else {
    if (
      readFirstValue(
        circuitSource,
        "fullMessageCircuit",
        "full_message_circuit",
      ) !== true
    ) {
      problems.push(
        "Groth16 material manifest circuit source must be a full-message circuit",
      );
    }
    if (
      readFirstValue(
        circuitSource,
        "signalBindingFixture",
        "signal_binding_fixture",
      ) !== false
    ) {
      problems.push(
        "Groth16 material manifest circuit source must not be signal-binding fixture material",
      );
    }
    if (
      readFirstValue(
        circuitSource,
        "unresolvedPlaceholders",
        "unresolved_placeholders",
      ) !== false
    ) {
      problems.push(
        "Groth16 material manifest circuit source must not contain unresolved placeholders",
      );
    }
    if (
      readFirstValue(
        circuitSource,
        "keccakPublicSignalDerivation",
        "keccak_public_signal_derivation",
      ) !== true
    ) {
      problems.push(
        "Groth16 material manifest circuit source must derive public signals with Keccak",
      );
    }
    if (
      readFirstValue(
        circuitSource,
        "digestReductionModuloScalarField",
        "digest_reduction_modulo_scalar_field",
      ) !== true
    ) {
      problems.push(
        "Groth16 material manifest circuit source must reduce digest signals modulo the scalar field",
      );
    }
    if (
      readFirstValue(
        circuitSource,
        "valueBitBooleanConstraints",
        "value_bit_boolean_constraints",
      ) !== true
    ) {
      problems.push(
        "Groth16 material manifest circuit source must boolean-constrain value bits",
      );
    }
    const publicSignalConstraintCount = Number(
      readFirstValue(
        circuitSource,
        "publicSignalConstraintCount",
        "public_signal_constraint_count",
      ),
    );
    if (
      !Number.isSafeInteger(publicSignalConstraintCount) ||
      publicSignalConstraintCount !== 9
    ) {
      problems.push(
        "Groth16 material manifest circuit source must constrain all 9 public signals",
      );
    }
    const labelBindingCount = Number(
      readFirstValue(circuitSource, "labelBindingCount", "label_binding_count"),
    );
    if (!Number.isSafeInteger(labelBindingCount) || labelBindingCount !== 9) {
      problems.push(
        "Groth16 material manifest circuit source must bind all 9 Solidity signal labels",
      );
    }
  }
  return problems;
}

function referencedGroth16AttestationPathCandidates(root, pathRef) {
  const normalized = normalizeNonEmptyText(
    pathRef,
    "Groth16 material manifest attestation path",
  );
  if (
    normalized.includes("\0") ||
    /^[a-z][a-z0-9+.-]*:/iu.test(normalized) ||
    /[?#]/u.test(normalized) ||
    normalized.includes("\\") ||
    /%[0-9a-f]{2}/iu.test(normalized) ||
    isAbsolute(normalized) ||
    win32.isAbsolute(normalized) ||
    pathHasDecodedParentSegment(normalized)
  ) {
    throw new Error(
      "Groth16 material manifest attestation path must be a safe relative path",
    );
  }
  const segments = normalized.split("/");
  if (
    !segments.every(
      (segment) => segment && segment !== "." && segment !== "..",
    )
  ) {
    throw new Error(
      "Groth16 material manifest attestation path must be a safe relative path",
    );
  }
  return [
    resolve(root, normalized),
    resolve(REPO_ROOT, normalized),
    resolve(normalized),
  ].filter((candidate, index, candidates) => candidates.indexOf(candidate) === index);
}

async function readReferencedGroth16Attestation({
  root,
  reference,
  expectedSha256,
  label,
}) {
  const pathRef = readFirstString(reference, "path");
  if (!pathRef) {
    return { record: null, problems: [`${label} path is required`] };
  }
  let candidates;
  try {
    candidates = referencedGroth16AttestationPathCandidates(root, pathRef);
  } catch (error) {
    return {
      record: null,
      problems: [error instanceof Error ? error.message : String(error)],
    };
  }
  const problems = [];
  for (const candidate of candidates) {
    try {
      const info = await lstat(candidate);
      if (info.isSymbolicLink()) {
        problems.push(`${pathRef} must not be a symbolic link`);
        continue;
      }
      if (!info.isFile()) {
        problems.push(`${pathRef} must be a regular file`);
        continue;
      }
      if (info.size > SCCP_BSC_JSON_INPUT_MAX_BYTES) {
        problems.push(
          `${pathRef} is ${info.size} bytes; maximum allowed is ${SCCP_BSC_JSON_INPUT_MAX_BYTES}`,
        );
        continue;
      }
      const realCandidate = await realpath(candidate);
      const realRoot = await realpath(root);
      const withinRoot = pathIsWithin(realCandidate, realRoot);
      const withinRepo = pathIsWithin(realCandidate, REPO_ROOT);
      if (!withinRoot && !withinRepo) {
        problems.push(`${pathRef} must stay under artifact-root or repository`);
        continue;
      }
      const bytes = await readFile(realCandidate);
      const actualSha256 = bytesToHex(sha256(new Uint8Array(bytes)));
      if (actualSha256 !== expectedSha256) {
        return {
          record: null,
          problems: [`${label} sha256 does not match referenced file`],
        };
      }
      let record;
      try {
        record = parseJsonWithoutDuplicateKeys(bytes.toString("utf8"), label);
      } catch (error) {
        return {
          record: null,
          problems: [
            `${label} is not valid duplicate-free JSON: ${
              error instanceof Error ? error.message : String(error)
            }`,
          ],
        };
      }
      if (!isRecord(record)) {
        return { record: null, problems: [`${label} must be a JSON object`] };
      }
      const secretReason = unsafeSecretReason(record, label);
      if (secretReason) {
        return { record: null, problems: [secretReason] };
      }
      return { record, problems: [] };
    } catch (_error) {
      continue;
    }
  }
  return {
    record: null,
    problems: problems.length
      ? problems
      : [`${label} referenced file ${pathRef} could not be found`],
  };
}

function groth16AttestationBodyProblems({
  record,
  expectedSchema,
  label,
  binding,
  profile,
  manifest,
  proofArtifact,
  provingKey,
  verifierKey,
  verifierMaterial,
  snarkjsVerificationKeyHash,
  circuitSourceHash,
  powersOfTauHash,
  trustedSetupTranscriptHash,
  reproducibleBuildTranscriptHash,
  reproducibleBuildToolchainSha256,
}) {
  const problems = [
    ...groth16AttestationUnknownFieldProblems(record, expectedSchema, label),
    ...productionEvidenceTextProblems(record, label),
    groth16ManifestStringProblem(record, ["schema"], expectedSchema, `${label} schema`),
    groth16ManifestStringProblem(record, ["routeId", "route_id"], ROUTE_ID, `${label} routeId`),
    groth16ManifestStringProblem(record, ["assetKey", "asset_key"], ASSET_KEY, `${label} assetKey`),
    groth16ManifestStringProblem(
      record,
      ["bscNetwork", "bsc_network", "network"],
      profile.key,
      `${label} bscNetwork`,
    ),
    groth16ManifestStringProblem(record, ["chain"], profile.chain, `${label} chain`),
    groth16ManifestStringProblem(
      record,
      ["chainIdHex", "chain_id_hex"],
      profile.chainIdHex,
      `${label} chainIdHex`,
    ),
    groth16ManifestHashProblem(
      record,
      ["networkIdHex", "network_id_hex"],
      profile.networkIdHex,
      `${label} networkIdHex`,
    ),
    groth16ManifestStringProblem(
      record,
      ["proofBackend", "proof_backend"],
      BSC_EVM_GROTH16_BACKEND,
      `${label} proofBackend`,
    ),
    groth16ManifestStringProblem(
      record,
      ["proofFamily", "proof_family"],
      SCCP_PROOF_FAMILY_STARK_FRI,
      `${label} proofFamily`,
    ),
    groth16ManifestStringProblem(
      record,
      ["circuitProfile", "circuit_profile"],
      BSC_FULL_SCCP_CIRCUIT_PROFILE,
      `${label} circuitProfile`,
    ),
    groth16ManifestIntegerProblem(
      record,
      ["publicInputCount", "public_input_count"],
      9,
      `${label} publicInputCount`,
    ),
    groth16PublicSignalsProblem(record, label),
    groth16ManifestHashProblem(
      record,
      ["verifierKeyHash", "verifier_key_hash"],
      binding.verifierKeyHash,
      `${label} verifierKeyHash`,
    ),
    groth16ManifestHashProblem(
      record,
      ["verifierKeyHash", "verifier_key_hash"],
      verifierMaterial.expectedVerifierKeyHash,
      `${label} verifierKeyHash`,
    ),
    groth16ManifestHashProblem(
      record,
      ["r1csSha256", "r1cs_sha256", "proofArtifactHash", "proof_artifact_hash"],
      proofArtifact.sha256,
      `${label} r1csSha256`,
    ),
    groth16ManifestHashProblem(
      record,
      [
        "powersOfTauSha256",
        "powers_of_tau_sha256",
        "ptauSha256",
        "ptau_sha256",
      ],
      powersOfTauHash,
      `${label} powersOfTauSha256`,
    ),
    groth16ManifestHashProblem(
      record,
      ["provingKeySha256", "proving_key_sha256", "provingKeyHash", "proving_key_hash"],
      provingKey.sha256,
      `${label} provingKeySha256`,
    ),
    groth16ManifestHashProblem(
      record,
      ["bscVerifierKeySha256", "bsc_verifier_key_sha256"],
      verifierKey.sha256,
      `${label} bscVerifierKeySha256`,
    ),
    groth16ManifestHashProblem(
      record,
      ["snarkjsVerificationKeySha256", "snarkjs_verification_key_sha256"],
      snarkjsVerificationKeyHash,
      `${label} snarkjsVerificationKeySha256`,
    ),
    groth16ManifestHashProblem(
      record,
      ["circuitSourceSha256", "circuit_source_sha256"],
      circuitSourceHash,
      `${label} circuitSourceSha256`,
    ),
    ...(expectedSchema === BSC_GROTH16_REPRODUCIBLE_BUILD_ATTESTATION_SCHEMA
      ? groth16ReproducibleBuildAttestationProblems({
          record,
          label,
          manifest,
          binding,
          reproducibleBuildTranscriptHash,
          reproducibleBuildToolchainSha256,
        })
      : []),
    ...(expectedSchema === BSC_GROTH16_TRUSTED_SETUP_ATTESTATION_SCHEMA
      ? groth16TrustedSetupAttestationProblems({
          record,
          label,
          trustedSetupTranscriptHash,
        })
      : []),
    ...(expectedSchema === BSC_GROTH16_SEMANTIC_ATTESTATION_SCHEMA
      ? groth16SemanticAttestationProblems({ record, label })
      : []),
    ...(expectedSchema === BSC_GROTH16_CIRCUIT_SECURITY_ATTESTATION_SCHEMA
      ? groth16CircuitSecurityAttestationProblems({ record, label })
      : []),
  ];
  return problems.filter(Boolean);
}

function groth16SemanticAttestationProblems({ record, label }) {
  return [
    groth16ManifestStringProblem(
      record,
      ["semanticReviewEvidenceSchema", "semantic_review_evidence_schema"],
      BSC_GROTH16_SEMANTIC_REVIEW_EVIDENCE_SCHEMA,
      `${label} semanticReviewEvidenceSchema`,
    ),
    groth16ManifestHashPresentProblem(
      record,
      ["semanticReviewEvidenceSha256", "semantic_review_evidence_sha256"],
      `${label} semanticReviewEvidenceSha256`,
    ),
    groth16ManifestHashPresentProblem(
      record,
      ["semanticReviewReportSha256", "semantic_review_report_sha256"],
      `${label} semanticReviewReportSha256`,
    ),
    groth16ManifestBooleanProblem(
      record,
      ["fullSccpMessageSemantics", "full_sccp_message_semantics"],
      true,
      `${label} fullSccpMessageSemantics`,
    ),
    groth16ManifestBooleanProblem(
      record,
      ["sourceFinalitySemantics", "source_finality_semantics"],
      true,
      `${label} sourceFinalitySemantics`,
    ),
    groth16ManifestBooleanProblem(
      record,
      ["destinationBindingSemantics", "destination_binding_semantics"],
      true,
      `${label} destinationBindingSemantics`,
    ),
    groth16ManifestBooleanProblem(
      record,
      ["publicSignalDerivationSemantics", "public_signal_derivation_semantics"],
      true,
      `${label} publicSignalDerivationSemantics`,
    ),
    groth16ManifestBooleanProblem(
      record,
      ["negativeCaseCoverage", "negative_case_coverage"],
      true,
      `${label} negativeCaseCoverage`,
    ),
  ].filter(Boolean);
}

function groth16CircuitSecurityAttestationProblems({ record, label }) {
  return [
    groth16ManifestStringProblem(
      record,
      ["circuitSecurityAuditEvidenceSchema", "circuit_security_audit_evidence_schema"],
      BSC_GROTH16_CIRCUIT_SECURITY_AUDIT_EVIDENCE_SCHEMA,
      `${label} circuitSecurityAuditEvidenceSchema`,
    ),
    groth16ManifestHashPresentProblem(
      record,
      ["circuitSecurityAuditEvidenceSha256", "circuit_security_audit_evidence_sha256"],
      `${label} circuitSecurityAuditEvidenceSha256`,
    ),
    groth16ManifestHashPresentProblem(
      record,
      ["circuitSecurityAuditReportSha256", "circuit_security_audit_report_sha256"],
      `${label} circuitSecurityAuditReportSha256`,
    ),
    groth16ManifestStringProblem(
      record,
      ["auditResult", "audit_result"],
      "pass",
      `${label} auditResult`,
    ),
    groth16ManifestBooleanProblem(
      record,
      ["approved", "productionApproved", "production_approved"],
      true,
      `${label} approved`,
    ),
    groth16ManifestIntegerProblem(
      record,
      ["criticalFindings", "critical_findings"],
      0,
      `${label} criticalFindings`,
    ),
    groth16ManifestIntegerProblem(
      record,
      ["highFindings", "high_findings"],
      0,
      `${label} highFindings`,
    ),
    groth16ManifestIntegerProblem(
      record,
      ["unresolvedFindings", "unresolved_findings"],
      0,
      `${label} unresolvedFindings`,
    ),
  ].filter(Boolean);
}

function groth16TrustedSetupAttestationProblems({
  record,
  label,
  trustedSetupTranscriptHash,
}) {
  return [
    groth16ManifestStringProblem(
      record,
      ["ceremonyResult", "ceremony_result"],
      "pass",
      `${label} ceremonyResult`,
    ),
    groth16ManifestBooleanProblem(
      record,
      ["localSingleContributor", "local_single_contributor"],
      false,
      `${label} localSingleContributor`,
    ),
    groth16ManifestIntegerAtLeastProblem(
      record,
      ["minimumContributors", "minimum_contributors"],
      2,
      `${label} minimumContributors`,
    ),
    groth16ManifestBooleanProblem(
      record,
      ["toxicWasteDestroyed", "toxic_waste_destroyed"],
      true,
      `${label} toxicWasteDestroyed`,
    ),
    groth16ManifestHashProblem(
      record,
      ["contributionTranscriptSha256", "contribution_transcript_sha256"],
      trustedSetupTranscriptHash,
      `${label} contributionTranscriptSha256`,
    ),
  ].filter(Boolean);
}

function groth16ReproducibleBuildAttestationProblems({
  record,
  label,
  manifest,
  binding,
  reproducibleBuildTranscriptHash,
  reproducibleBuildToolchainSha256,
}) {
  const selfChecks = readFirstRecord(manifest, "selfChecks", "self_checks");
  const snarkjs = selfChecks
    ? readFirstRecord(selfChecks, "snarkjs", "snark_js")
    : null;
  if (!snarkjs) {
    return [`${label} requires Groth16 material manifest selfChecks.snarkjs`];
  }
  const problems = [
    groth16ManifestBooleanProblem(
      record,
      ["reproducible", "reproducibleBuild", "reproducible_build"],
      true,
      `${label} reproducible`,
    ),
    groth16ManifestIntegerAtLeastProblem(
      record,
      ["independentRebuilders", "independent_rebuilders"],
      2,
      `${label} independentRebuilders`,
    ),
    groth16ManifestHashProblem(
      record,
      ["buildTranscriptSha256", "build_transcript_sha256"],
      reproducibleBuildTranscriptHash,
      `${label} buildTranscriptSha256`,
    ),
    reproducibleBuildToolchainSha256
      ? groth16ManifestHashProblem(
          record,
          ["toolchainSha256", "toolchain_sha256"],
          reproducibleBuildToolchainSha256,
          `${label} toolchainSha256`,
        )
      : `${label} requires reproducible build transcript-derived toolchainSha256`,
    groth16ManifestBooleanProblem(
      record,
      ["zkeyVerify", "zkey_verify"],
      true,
      `${label} zkeyVerify`,
    ),
    groth16ManifestStringProblem(
      record,
      ["zkeyVerifyResult", "zkey_verify_result"],
      readFirstString(snarkjs, "zkeyVerifyResult", "zkey_verify_result"),
      `${label} zkeyVerifyResult`,
    ),
    groth16ManifestBooleanProblem(
      record,
      ["zkeyVerificationKeyExport", "zkey_verification_key_export"],
      true,
      `${label} zkeyVerificationKeyExport`,
    ),
    groth16ManifestBooleanProblem(
      record,
      ["verifierKeyHashMatches", "verifier_key_hash_matches"],
      true,
      `${label} verifierKeyHashMatches`,
    ),
    groth16ManifestHashProblem(
      record,
      ["exportedVerifierKeyHash", "exported_verifier_key_hash"],
      binding.verifierKeyHash,
      `${label} exportedVerifierKeyHash`,
    ),
  ];
  const r1csInfoSource = readFirstString(
    snarkjs,
    "r1csInfoSource",
    "r1cs_info_source",
  );
  if (r1csInfoSource) {
    problems.push(
      groth16ManifestStringProblem(
        record,
        ["r1csInfoSource", "r1cs_info_source"],
        r1csInfoSource,
        `${label} r1csInfoSource`,
      ),
    );
  } else {
    problems.push(
      `${label} requires Groth16 material manifest SnarkJS R1CS info source`,
    );
  }
  const r1csPublicInputCount = Number(
    readFirstValue(
      snarkjs,
      "r1csPublicInputCount",
      "r1cs_public_input_count",
    ),
  );
  if (Number.isSafeInteger(r1csPublicInputCount)) {
    problems.push(
      groth16ManifestIntegerProblem(
        record,
        ["r1csPublicInputCount", "r1cs_public_input_count"],
        r1csPublicInputCount,
        `${label} r1csPublicInputCount`,
      ),
    );
  } else {
    problems.push(
      `${label} requires Groth16 material manifest SnarkJS R1CS public input count`,
    );
  }
  const r1csConstraintCount = Number(
    readFirstValue(
      snarkjs,
      "r1csConstraintCount",
      "r1cs_constraint_count",
    ),
  );
  if (Number.isSafeInteger(r1csConstraintCount)) {
    problems.push(
      groth16ManifestIntegerProblem(
        record,
        ["r1csConstraintCount", "r1cs_constraint_count"],
        r1csConstraintCount,
        `${label} r1csConstraintCount`,
      ),
    );
  } else {
    problems.push(
      `${label} requires Groth16 material manifest SnarkJS R1CS constraint count`,
    );
  }
  return problems.filter(Boolean);
}

function groth16AttestationSignatureProblems({
  record,
  trustedSignerFingerprints,
  label,
}) {
  const problems = [];
  const trusted = new Set(trustedSignerFingerprints);
  if (trusted.size === 0) {
    problems.push(`${label} trusted attestation signer fingerprint is required`);
  }
  const signature = readFirstRecord(record, "signature");
  if (!signature) {
    return [...problems, `${label} signature is required`];
  }
  if (
    readFirstString(signature, "schema") !==
    BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA
  ) {
    problems.push(
      `${label} signature schema must be ${BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA}`,
    );
  }
  if (readFirstString(signature, "algorithm") !== "ed25519") {
    problems.push(`${label} signature algorithm must be ed25519`);
  }
  const payload = attestationSignaturePayload(record);
  const actualPayloadHash = sha256HexBytes(payload);
  try {
    const expectedPayloadHash = normalizeCanonicalHex32(
      readFirstValue(signature, "signedPayloadSha256", "signed_payload_sha256"),
      `${label} signature signedPayloadSha256`,
    );
    if (expectedPayloadHash !== actualPayloadHash) {
      problems.push(`${label} signature signedPayloadSha256 must match attestation body`);
    }
  } catch (error) {
    problems.push(error instanceof Error ? error.message : String(error));
  }
  let publicKey;
  try {
    const result = publicKeyFingerprint(
      readFirstValue(signature, "publicKeyPem", "public_key_pem"),
      `${label} signature publicKeyPem`,
    );
    publicKey = result.publicKey;
    const declaredFingerprint = normalizeCanonicalHex32(
      readFirstValue(signature, "signerFingerprint", "signer_fingerprint"),
      `${label} signature signerFingerprint`,
    );
    if (declaredFingerprint !== result.fingerprint) {
      problems.push(`${label} signature signerFingerprint must match public key`);
    }
    if (trusted.size > 0 && !trusted.has(declaredFingerprint)) {
      problems.push(`${label} signature signerFingerprint is not trusted`);
    }
  } catch (error) {
    problems.push(error instanceof Error ? error.message : String(error));
  }
  try {
    const signatureBuffer = attestationSignatureBytes(
      readFirstValue(signature, "signature", "signatureBase64"),
      `${label} signature`,
    );
    if (
      !publicKey ||
      !verifyDetachedSignature(null, payload, publicKey, signatureBuffer)
    ) {
      problems.push(`${label} detached signature verification failed`);
    }
  } catch (error) {
    problems.push(error instanceof Error ? error.message : String(error));
  }
  return problems.filter(Boolean);
}

function verifiedGroth16AttestationSignerFingerprint({
  record,
  trustedSignerFingerprints,
}) {
  try {
    const signature = readFirstRecord(record, "signature");
    if (
      !signature ||
      readFirstString(signature, "schema") !==
        BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA ||
      readFirstString(signature, "algorithm") !== "ed25519"
    ) {
      return null;
    }
    const trusted = new Set(trustedSignerFingerprints);
    const payload = attestationSignaturePayload(record);
    const expectedPayloadHash = normalizeCanonicalHex32(
      readFirstValue(signature, "signedPayloadSha256", "signed_payload_sha256"),
      "Groth16 attestation signature signedPayloadSha256",
    );
    if (expectedPayloadHash !== sha256HexBytes(payload)) {
      return null;
    }
    const result = publicKeyFingerprint(
      readFirstValue(signature, "publicKeyPem", "public_key_pem"),
      "Groth16 attestation signature publicKeyPem",
    );
    const declaredFingerprint = normalizeCanonicalHex32(
      readFirstValue(signature, "signerFingerprint", "signer_fingerprint"),
      "Groth16 attestation signature signerFingerprint",
    );
    if (declaredFingerprint !== result.fingerprint) {
      return null;
    }
    if (trusted.size > 0 && !trusted.has(declaredFingerprint)) {
      return null;
    }
    const signatureBuffer = attestationSignatureBytes(
      readFirstValue(signature, "signature", "signatureBase64"),
      "Groth16 attestation signature",
    );
    return verifyDetachedSignature(null, payload, result.publicKey, signatureBuffer)
      ? declaredFingerprint
      : null;
  } catch (_error) {
    return null;
  }
}

function groth16AttestationSignerDiversityProblems(rows) {
  const seen = new Map();
  const problems = [];
  for (const [label, fingerprint] of rows) {
    if (!fingerprint) {
      continue;
    }
    const previous = seen.get(fingerprint);
    if (previous) {
      problems.push(
        `Groth16 material manifest attestation signers must be role-separated; ${previous} and ${label} reuse signer ${fingerprint}`,
      );
    } else {
      seen.set(fingerprint, label);
    }
  }
  return problems;
}

function groth16TrustedSetupTranscriptProblems(record, label) {
  const phase1 = readFirstRecord(record, "phase1");
  const snarkjsPowersOfTauVerify = phase1
    ? readFirstRecord(
        phase1,
        "snarkjsPowersOfTauVerify",
        "snarkjs_powers_of_tau_verify",
      )
    : null;
  const phase2 = readFirstRecord(record, "phase2");
  return [
    ...productionEvidenceTextProblems(record, label),
    groth16ManifestArrayOrCountAtLeastProblem(
      record,
      ["contributors", "participants", "contributions"],
      ["minimumContributors", "minimum_contributors"],
      2,
      `${label} contributors`,
    ),
    groth16ManifestOptionalIntegerProblem(
      record,
      ["minimumContributorsObserved", "minimum_contributors_observed"],
      2,
      `${label} minimumContributorsObserved`,
    ),
    groth16ManifestBooleanProblem(
      record,
      ["localSingleContributor", "local_single_contributor"],
      false,
      `${label} localSingleContributor`,
    ),
    groth16ManifestBooleanProblem(
      record,
      ["toxicWasteDestroyed", "toxic_waste_destroyed"],
      true,
      `${label} toxicWasteDestroyed`,
    ),
    groth16ManifestStringProblem(
      record,
      ["ceremonyResult", "ceremony_result"],
      "pass",
      `${label} ceremonyResult`,
    ),
    phase1 ? "" : `${label} phase1 block is required`,
    snarkjsPowersOfTauVerify
      ? groth16ManifestBooleanProblem(
          snarkjsPowersOfTauVerify,
          ["completed"],
          true,
          `${label} snarkjsPowersOfTauVerify.completed`,
        )
      : `${label} snarkjsPowersOfTauVerify block is required`,
    phase2 ? "" : `${label} phase2 block is required`,
    phase2
      ? groth16ManifestStringProblem(
          phase2,
          ["snarkjsZkeyVerify", "snarkjs_zkey_verify"],
          "ZKey Ok!",
          `${label} snarkjsZkeyVerify`,
        )
      : "",
  ].filter(Boolean);
}

function groth16ReproducibleBuildTranscriptProblems(record, label, manifest) {
  const selfChecks = readFirstRecord(manifest, "selfChecks", "self_checks");
  const snarkjs = selfChecks
    ? readFirstRecord(selfChecks, "snarkjs", "snark_js")
    : null;
  const toolchainEvidence =
    groth16ReproducibleBuildTranscriptToolchainEvidence(record, label, manifest);
  const problems = [
    ...productionEvidenceTextProblems(record, label),
    ...toolchainEvidence.problems,
    groth16ManifestArrayOrCountAtLeastProblem(
      record,
      ["independentRebuilders", "independent_rebuilders", "rebuilders"],
      ["independentRebuilderCount", "independent_rebuilder_count"],
      2,
      `${label} independentRebuilders`,
    ),
    groth16ManifestOptionalIntegerProblem(
      record,
      ["independentRebuildersObserved", "independent_rebuilders_observed"],
      2,
      `${label} independentRebuildersObserved`,
    ),
    groth16ManifestBooleanProblem(
      record,
      ["reproducible"],
      true,
      `${label} reproducible`,
    ),
    groth16ManifestOptionalBooleanProblem(
      record,
      ["reproducibleBuildComplete", "reproducible_build_complete"],
      true,
      `${label} reproducibleBuildComplete`,
    ),
  ];
  if (snarkjs) {
    problems.push(
      groth16ManifestBooleanProblem(
        record,
        ["zkeyVerify", "zkey_verify"],
        true,
        `${label} zkeyVerify`,
      ),
      groth16ManifestStringProblem(
        record,
        ["zkeyVerifyResult", "zkey_verify_result"],
        readFirstString(snarkjs, "zkeyVerifyResult", "zkey_verify_result"),
        `${label} zkeyVerifyResult`,
      ),
    );
    const r1csInfoSource = readFirstString(
      snarkjs,
      "r1csInfoSource",
      "r1cs_info_source",
    );
    if (r1csInfoSource) {
      problems.push(
        groth16ManifestOptionalStringProblem(
          record,
          ["r1csInfoSource", "r1cs_info_source"],
          r1csInfoSource,
          `${label} r1csInfoSource`,
        ),
      );
    }
    const r1csPublicInputCount = Number(
      readFirstValue(
        snarkjs,
        "r1csPublicInputCount",
        "r1cs_public_input_count",
      ),
    );
    if (Number.isSafeInteger(r1csPublicInputCount)) {
      problems.push(
        groth16ManifestOptionalIntegerProblem(
          record,
          ["r1csPublicInputCount", "r1cs_public_input_count"],
          r1csPublicInputCount,
          `${label} r1csPublicInputCount`,
        ),
      );
    }
    const r1csConstraintCount = Number(
      readFirstValue(
        snarkjs,
        "r1csConstraintCount",
        "r1cs_constraint_count",
      ),
    );
    if (Number.isSafeInteger(r1csConstraintCount)) {
      problems.push(
        groth16ManifestOptionalIntegerProblem(
          record,
          ["r1csConstraintCount", "r1cs_constraint_count"],
          r1csConstraintCount,
          `${label} r1csConstraintCount`,
        ),
      );
    }
  }
  return problems.filter(Boolean);
}

function groth16ReproducibleBuildTranscriptToolchainEvidence(
  record,
  label,
  manifest,
) {
  const problems = [];
  const toolchain = readFirstRecord(record, "toolchain");
  if (!toolchain) {
    return {
      problems: [`${label} toolchain object is required`],
      toolchainSha256: null,
      snarkjsBinary: null,
    };
  }
  let toolchainSha256 = null;
  try {
    toolchainSha256 = sha256HexBytes(
      Buffer.from(canonicalJson(toolchain), "utf8"),
    );
  } catch (error) {
    problems.push(error instanceof Error ? error.message : String(error));
  }
  const toolchainSnarkjs = readFirstRecord(toolchain, "snarkjs", "snark_js");
  let snarkjsBinary = null;
  let snarkjsBinarySha256 = null;
  if (!toolchainSnarkjs) {
    problems.push(`${label} toolchain.snarkjs block is required`);
  } else {
    snarkjsBinary = readFirstString(
      toolchainSnarkjs,
      "binary",
      "path",
      "snarkjsBinary",
      "snarkjs_binary",
    );
    if (!snarkjsBinary) {
      problems.push(`${label} toolchain.snarkjs.binary is required`);
    }
    try {
      snarkjsBinarySha256 = groth16MaterialManifestHash(
        toolchainSnarkjs,
        ["binarySha256", "binary_sha256"],
        `${label} toolchain.snarkjs.binarySha256`,
      );
    } catch (error) {
      problems.push(error instanceof Error ? error.message : String(error));
    }
  }
  const toolchainCircom = readFirstRecord(toolchain, "circom");
  if (!toolchainCircom) {
    problems.push(`${label} toolchain.circom block is required`);
  } else {
    if (!readFirstString(toolchainCircom, "binary")) {
      problems.push(`${label} toolchain.circom.binary is required`);
    }
    try {
      groth16MaterialManifestHash(
        toolchainCircom,
        ["binarySha256", "binary_sha256"],
        `${label} toolchain.circom.binarySha256`,
      );
    } catch (error) {
      problems.push(error instanceof Error ? error.message : String(error));
    }
  }
  const selfChecks = readFirstRecord(manifest, "selfChecks", "self_checks");
  const manifestSnarkjs = selfChecks
    ? readFirstRecord(selfChecks, "snarkjs", "snark_js")
    : null;
  const manifestSnarkjsBinary = manifestSnarkjs
    ? readFirstString(manifestSnarkjs, "snarkjsBinary", "snarkjs_binary")
    : "";
  if (
    snarkjsBinary &&
    manifestSnarkjsBinary &&
    resolve(snarkjsBinary) !== resolve(manifestSnarkjsBinary)
  ) {
    problems.push(
      "Groth16 material manifest selfChecks.snarkjs.snarkjsBinary must match reproducible build transcript toolchain.snarkjs.binary",
    );
  }
  return {
    problems,
    toolchainSha256,
    snarkjsBinary,
    snarkjsBinarySha256,
  };
}

async function referencedGroth16MaterialTranscriptProblems({ root, manifest }) {
  const artifacts = readFirstRecord(manifest, "artifacts") ?? {};
  const required = [
    [
      ["trustedSetupTranscript", "trusted_setup_transcript"],
      "trusted setup transcript",
      (record, label) => groth16TrustedSetupTranscriptProblems(record, label),
    ],
    [
      ["reproducibleBuildTranscript", "reproducible_build_transcript"],
      "reproducible build transcript",
      (record, label) =>
        groth16ReproducibleBuildTranscriptProblems(record, label, manifest),
    ],
  ];
  const problems = [];
  let reproducibleBuildToolchainSha256 = null;
  let reproducibleBuildSnarkjsBinary = null;
  let reproducibleBuildSnarkjsBinarySha256 = null;
  for (const [keys, label, validate] of required) {
    const reference = readFirstRecord(artifacts, ...keys);
    if (!reference) {
      problems.push(`Groth16 material manifest ${label} reference is required`);
      continue;
    }
    let expectedSha256;
    try {
      expectedSha256 = groth16MaterialManifestHash(
        reference,
        ["sha256", "hash", "artifactHash", "artifact_hash"],
        `Groth16 material manifest ${label} sha256`,
      );
    } catch (error) {
      problems.push(error instanceof Error ? error.message : String(error));
      continue;
    }
    const readResult = await readReferencedGroth16Attestation({
      root,
      reference,
      expectedSha256,
      label: `Groth16 material manifest ${label}`,
    });
    problems.push(...readResult.problems);
    if (readResult.record) {
      problems.push(
        ...validate(readResult.record, `Groth16 material manifest ${label}`),
      );
      if (label === "reproducible build transcript") {
        const evidence = groth16ReproducibleBuildTranscriptToolchainEvidence(
          readResult.record,
          `Groth16 material manifest ${label}`,
          manifest,
        );
        if (evidence.problems.length === 0) {
          reproducibleBuildToolchainSha256 = evidence.toolchainSha256;
          reproducibleBuildSnarkjsBinary = evidence.snarkjsBinary;
          reproducibleBuildSnarkjsBinarySha256 = evidence.snarkjsBinarySha256;
        }
      }
    }
  }
  return {
    problems: uniqueNonEmpty(problems),
    reproducibleBuildToolchainSha256,
    reproducibleBuildSnarkjsBinary,
    reproducibleBuildSnarkjsBinarySha256,
  };
}

async function referencedGroth16MaterialAttestationProblems({
  root,
  manifest,
  binding,
  profile,
  proofArtifact,
  provingKey,
  verifierKey,
  verifierMaterial,
  trustedSignerFingerprints,
  reproducibleBuildToolchainSha256,
}) {
  const attestations = readFirstRecord(manifest, "attestations") ?? {};
  const snarkjsVerificationKeyHash = groth16MaterialManifestArtifactHash(
    manifest,
    ["snarkjsVerificationKey", "snarkjs_verification_key"],
    "SnarkJS verification key",
  );
  const circuitSourceHash = groth16MaterialManifestArtifactHash(
    manifest,
    ["circuitSource", "circuit_source"],
    "circuit source",
  );
  const powersOfTauHash = groth16MaterialManifestArtifactHash(
    manifest,
    ["powersOfTau", "powers_of_tau"],
    "Powers of Tau",
  );
  const trustedSetupTranscriptHash = groth16MaterialManifestArtifactHash(
    manifest,
    ["trustedSetupTranscript", "trusted_setup_transcript"],
    "trusted setup transcript",
  );
  const reproducibleBuildTranscriptHash = groth16MaterialManifestArtifactHash(
    manifest,
    ["reproducibleBuildTranscript", "reproducible_build_transcript"],
    "reproducible build transcript",
  );
  const required = [
    [
      "semanticSccpCircuit",
      BSC_GROTH16_SEMANTIC_ATTESTATION_SCHEMA,
      "semantic SCCP circuit attestation",
    ],
    [
      "circuitSecurity",
      BSC_GROTH16_CIRCUIT_SECURITY_ATTESTATION_SCHEMA,
      "circuit security attestation",
    ],
    [
      "trustedSetup",
      BSC_GROTH16_TRUSTED_SETUP_ATTESTATION_SCHEMA,
      "trusted setup attestation",
    ],
    [
      "reproducibleBuild",
      BSC_GROTH16_REPRODUCIBLE_BUILD_ATTESTATION_SCHEMA,
      "reproducible build attestation",
    ],
  ];
  const problems = [];
  const verifiedSigners = [];
  for (const [key, expectedSchema, label] of required) {
    const reference = readFirstRecord(attestations, key);
    if (!reference) {
      problems.push(`Groth16 material manifest ${label} reference is required`);
      continue;
    }
    let expectedSha256;
    try {
      expectedSha256 = groth16MaterialManifestHash(
        reference,
        ["sha256", "attestationHash", "attestation_hash"],
        `Groth16 material manifest ${label} sha256`,
      );
    } catch (error) {
      problems.push(error instanceof Error ? error.message : String(error));
      continue;
    }
    const readResult = await readReferencedGroth16Attestation({
      root,
      reference,
      expectedSha256,
      label: `Groth16 material manifest ${label}`,
    });
    problems.push(...readResult.problems);
    if (!readResult.record) {
      continue;
    }
    const bodyProblems = groth16AttestationBodyProblems({
        record: readResult.record,
        expectedSchema,
        label: `Groth16 material manifest ${label}`,
        binding,
        profile,
        manifest,
        proofArtifact,
        provingKey,
        verifierKey,
        verifierMaterial,
        snarkjsVerificationKeyHash,
        circuitSourceHash,
        powersOfTauHash,
        trustedSetupTranscriptHash,
        reproducibleBuildTranscriptHash,
        reproducibleBuildToolchainSha256,
      });
    const signatureProblems = groth16AttestationSignatureProblems({
        record: readResult.record,
        trustedSignerFingerprints,
        label: `Groth16 material manifest ${label}`,
      });
    problems.push(...bodyProblems, ...signatureProblems);
    if (signatureProblems.length === 0) {
      verifiedSigners.push([
        `Groth16 material manifest ${label}`,
        verifiedGroth16AttestationSignerFingerprint({
          record: readResult.record,
          trustedSignerFingerprints,
        }),
      ]);
    }
  }
  problems.push(...groth16AttestationSignerDiversityProblems(verifiedSigners));
  return uniqueNonEmpty(problems);
}

function validateBscGroth16MaterialManifest({
  manifest,
  binding,
  profile,
  proofArtifact,
  provingKey,
  verifierKey,
  verifierMaterial,
  trustedSignerFingerprints = [],
}) {
  if (!isRecord(manifest)) {
    throw new Error("Groth16 material manifest must be a JSON object.");
  }
  const problems = [];
  const addCheck = (fn) => {
    try {
      const problem = fn();
      if (problem) {
        problems.push(problem);
      }
    } catch (error) {
      problems.push(error instanceof Error ? error.message : String(error));
    }
  };
  problems.push(...bscGroth16MaterialManifestShapeProblems(manifest));
  addCheck(() =>
    readFirstString(manifest, "schema") === BSC_GROTH16_MATERIAL_MANIFEST_SCHEMA
      ? ""
      : `Groth16 material manifest schema must be ${BSC_GROTH16_MATERIAL_MANIFEST_SCHEMA}`,
  );
  addCheck(() =>
    readFirstString(manifest, "routeId", "route_id") === ROUTE_ID
      ? ""
      : `Groth16 material manifest routeId must be ${ROUTE_ID}`,
  );
  addCheck(() =>
    readFirstString(manifest, "assetKey", "asset_key") === ASSET_KEY
      ? ""
      : `Groth16 material manifest assetKey must be ${ASSET_KEY}`,
  );
  addCheck(() =>
    readFirstString(manifest, "bscNetwork", "bsc_network", "network") ===
    profile.key
      ? ""
      : `Groth16 material manifest bscNetwork must be ${profile.key}`,
  );
  addCheck(() =>
    readFirstString(manifest, "chain") === profile.chain
      ? ""
      : `Groth16 material manifest chain must be ${profile.chain}`,
  );
  addCheck(() =>
    readFirstString(manifest, "chainIdHex", "chain_id_hex") === profile.chainIdHex
      ? ""
      : `Groth16 material manifest chainIdHex must be ${profile.chainIdHex}`,
  );
  addCheck(() => {
    const networkIdHex = normalizeCanonicalHex32(
      readFirstValue(manifest, "networkIdHex", "network_id_hex"),
      "Groth16 material manifest networkIdHex",
    );
    return networkIdHex === profile.networkIdHex
      ? ""
      : `Groth16 material manifest networkIdHex must be ${profile.networkIdHex}`;
  });
  addCheck(() =>
    readFirstString(manifest, "circuitProfile", "circuit_profile") ===
    BSC_FULL_SCCP_CIRCUIT_PROFILE
      ? ""
      : `Groth16 material manifest circuitProfile must be ${BSC_FULL_SCCP_CIRCUIT_PROFILE}`,
  );
  addCheck(() =>
    readFirstString(manifest, "proofBackend", "proof_backend") === BSC_EVM_GROTH16_BACKEND
      ? ""
      : `Groth16 material manifest proofBackend must be ${BSC_EVM_GROTH16_BACKEND}`,
  );
  addCheck(() =>
    readFirstString(manifest, "proofFamily", "proof_family") === SCCP_PROOF_FAMILY_STARK_FRI
      ? ""
      : `Groth16 material manifest proofFamily must be ${SCCP_PROOF_FAMILY_STARK_FRI}`,
  );
  addCheck(() =>
    groth16ManifestIntegerProblem(
      manifest,
      ["sourceDomain", "source_domain"],
      SCCP_DOMAIN_SORA,
      "Groth16 material manifest sourceDomain",
    ),
  );
  addCheck(() =>
    groth16ManifestIntegerProblem(
      manifest,
      ["targetDomain", "target_domain"],
      SCCP_DOMAIN_BSC,
      "Groth16 material manifest targetDomain",
    ),
  );
  addCheck(() =>
    groth16ManifestIntegerProblem(
      manifest,
      ["publicInputCount", "public_input_count"],
      9,
      "Groth16 material manifest publicInputCount",
    ),
  );
  addCheck(() =>
    groth16PublicSignalsProblem(manifest, "Groth16 material manifest"),
  );
  addCheck(() =>
    groth16ManifestBooleanProblem(
      manifest,
      ["productionReady", "production_ready"],
      true,
      "Groth16 material manifest productionReady",
    ),
  );
  const productionBlockersAliasProblem = groth16ManifestAliasProblem(
    manifest,
    ["productionBlockers", "production_blockers"],
    "Groth16 material manifest productionBlockers",
  );
  if (productionBlockersAliasProblem) {
    problems.push(productionBlockersAliasProblem);
  }
  const productionBlockers =
    readFirstValue(manifest, "productionBlockers", "production_blockers") ?? [];
  if (Array.isArray(productionBlockers) && productionBlockers.length > 0) {
    problems.push("Groth16 material manifest productionBlockers must be empty");
  } else if (!Array.isArray(productionBlockers)) {
    problems.push("Groth16 material manifest productionBlockers must be an array");
  }
  problems.push(...groth16MaterialManifestReferenceProblems(manifest));
  addCheck(() => {
    const verifierKeyHash = normalizeCanonicalHex32(
      readFirstValue(manifest, "verifierKeyHash", "verifier_key_hash"),
      "Groth16 material manifest verifierKeyHash",
    );
    return verifierKeyHash === binding.verifierKeyHash &&
      verifierKeyHash === verifierMaterial.expectedVerifierKeyHash
      ? ""
      : "Groth16 material manifest verifierKeyHash must match route and verifier key";
  });
  addCheck(() =>
    groth16MaterialManifestArtifactHash(manifest, ["r1cs"], "R1CS") ===
    proofArtifact.sha256
      ? ""
      : "Groth16 material manifest R1CS hash must match proof artifact",
  );
  addCheck(() => {
    groth16MaterialManifestArtifactHash(
      manifest,
      ["powersOfTau", "powers_of_tau"],
      "Powers of Tau",
    );
    return "";
  });
  addCheck(() => {
    groth16MaterialManifestArtifactHash(
      manifest,
      ["circuitSource", "circuit_source"],
      "circuit source",
    );
    return "";
  });
  addCheck(() =>
    groth16MaterialManifestArtifactHash(
      manifest,
      ["provingKey", "proving_key"],
      "proving key",
    ) === provingKey.sha256
      ? ""
      : "Groth16 material manifest proving key hash must match proving key",
  );
  addCheck(() =>
    groth16MaterialManifestArtifactHash(
      manifest,
      ["bscVerifierKey", "bsc_verifier_key"],
      "BSC verifier key",
    ) === verifierKey.sha256
      ? ""
      : "Groth16 material manifest verifier key hash must match verifier key artifact",
  );
  addCheck(() => {
    groth16MaterialManifestArtifactHash(
      manifest,
      ["snarkjsVerificationKey", "snarkjs_verification_key"],
      "SnarkJS verification key",
    );
    return "";
  });
  addCheck(() => {
    groth16MaterialManifestArtifactHash(
      manifest,
      ["trustedSetupTranscript", "trusted_setup_transcript"],
      "trusted setup transcript",
    );
    return "";
  });
  addCheck(() => {
    groth16MaterialManifestArtifactHash(
      manifest,
      ["reproducibleBuildTranscript", "reproducible_build_transcript"],
      "reproducible build transcript",
    );
    return "";
  });
  problems.push(
    ...groth16MaterialManifestTrustPolicyProblems(
      manifest,
      trustedSignerFingerprints,
    ),
  );
  problems.push(...groth16MaterialManifestSelfCheckProblems(manifest, binding));
  problems.push(...groth16MaterialManifestAttestationProblems(manifest));
  if (problems.length > 0) {
    throw new Error(
      `Groth16 material manifest is not production-ready: ${uniqueNonEmpty(problems).join("; ")}`,
    );
  }
}

async function readRequiredBscGroth16MaterialManifest({
  root,
  options,
  binding,
  profile,
  proofArtifact,
  provingKey,
  verifierKey,
  verifierMaterial,
}) {
  const trustedSignerFingerprints =
    parseTrustedAttestationSignerFingerprints(options);
  const value = optionValue(options, [
    "groth16-material-manifest",
    "material-manifest",
  ]);
  if (value === undefined || value === null || trim(value) === "") {
    throw new Error(
      "native-prover-bundle requires --groth16-material-manifest.",
    );
  }
  const artifact = await readArtifactUnderRoot(
    root,
    value,
    "Groth16 material manifest",
  );
  let manifest;
  try {
    manifest = parseJsonWithoutDuplicateKeys(
      artifact.bytes.toString("utf8"),
      "Groth16 material manifest",
    );
  } catch (error) {
    throw new Error(
      `Groth16 material manifest must be valid duplicate-free JSON: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
  validateBscGroth16MaterialManifest({
    manifest,
    binding,
    profile,
    proofArtifact,
    provingKey,
    verifierKey,
    verifierMaterial,
    trustedSignerFingerprints,
  });
  const transcriptEvidence =
    await referencedGroth16MaterialTranscriptProblems({
      root,
      manifest,
    });
  if (transcriptEvidence.problems.length > 0) {
    throw new Error(
      `Groth16 material manifest transcripts are not production-ready: ${transcriptEvidence.problems.join("; ")}`,
    );
  }
  const attestationProblems =
    await referencedGroth16MaterialAttestationProblems({
      root,
      manifest,
      binding,
      profile,
      proofArtifact,
      provingKey,
      verifierKey,
      verifierMaterial,
      trustedSignerFingerprints,
      reproducibleBuildToolchainSha256:
        transcriptEvidence.reproducibleBuildToolchainSha256,
    });
  if (attestationProblems.length > 0) {
    throw new Error(
      `Groth16 material manifest attestations are not production-ready: ${attestationProblems.join("; ")}`,
    );
  }
  return { artifact, manifest, transcriptEvidence };
}

function groth16MaterialManifestAttestationHash(manifest, key, label) {
  const attestations = readFirstRecord(manifest, "attestations") ?? {};
  const reference = readFirstRecord(attestations, key);
  if (!reference) {
    throw new Error(`Groth16 material manifest ${label} reference is required.`);
  }
  return groth16MaterialManifestHash(
    reference,
    ["sha256", "attestationHash", "attestation_hash"],
    `Groth16 material manifest ${label} sha256`,
  );
}

function requireNativeProverAuditHashesBindGroth16Material(
  auditHashes,
  groth16MaterialManifest,
) {
  const manifest = groth16MaterialManifest?.manifest;
  if (!isRecord(manifest)) {
    throw new Error("Groth16 material manifest is required for audit hash binding.");
  }
  const expectedCircuitSecurityHash = groth16MaterialManifestAttestationHash(
    manifest,
    "circuitSecurity",
    "circuit security attestation",
  );
  if (auditHashes.circuit_security_audit !== expectedCircuitSecurityHash) {
    throw new Error(
      "auditHashes.circuit_security_audit must match Groth16 material manifest circuit security attestation sha256.",
    );
  }
  const expectedReproducibleBuildHash = groth16MaterialManifestAttestationHash(
    manifest,
    "reproducibleBuild",
    "reproducible build attestation",
  );
  if (auditHashes.reproducible_build_attestation !== expectedReproducibleBuildHash) {
    throw new Error(
      "auditHashes.reproducible_build_attestation must match Groth16 material manifest reproducible build attestation sha256.",
    );
  }
}

function readGroth16ProofSelfTestArtifactHash(report, artifactKeys, label) {
  const artifacts = readFirstRecord(report, "artifacts") ?? {};
  const artifact = readFirstRecord(artifacts, ...artifactKeys) ?? {};
  return groth16MaterialManifestHash(
    artifact,
    ["sha256", "hash", "artifactHash", "artifact_hash"],
    `Groth16 proof self-test ${label} sha256`,
  );
}

function readGroth16ProofSelfTestArtifactPath(report, artifactKeys, label) {
  const artifacts = readFirstRecord(report, "artifacts") ?? {};
  const artifact = readFirstRecord(artifacts, ...artifactKeys) ?? {};
  const path = readFirstString(artifact, "path");
  if (!path) {
    throw new Error(`Groth16 proof self-test ${label} path is required.`);
  }
  return path;
}

function groth16ProofSelfTestPathProblem(record, keys, expected, label) {
  const actual = readFirstString(record, ...keys);
  if (!actual) {
    return `${label} path is required`;
  }
  return actual === expected ? "" : `${label} path must be ${expected}`;
}

function bscGroth16ProofSelfTestAdversarialProblems(report) {
  const problems = [];
  const checks = readFirstRecord(report, "adversarialChecks", "adversarial_checks");
  if (!checks) {
    return ["Groth16 proof self-test adversarialChecks block is required"];
  }
  const publicSignalMismatch = readFirstRecord(
    checks,
    "publicSignalMismatch",
    "public_signal_mismatch",
  );
  if (!publicSignalMismatch) {
    problems.push(
      "Groth16 proof self-test adversarialChecks.publicSignalMismatch is required",
    );
  } else {
    if (readFirstValue(publicSignalMismatch, "attempted") !== 9) {
      problems.push(
        "Groth16 proof self-test adversarial publicSignalMismatch.attempted must be 9",
      );
    }
    if (readFirstValue(publicSignalMismatch, "rejected") !== 9) {
      problems.push(
        "Groth16 proof self-test adversarial publicSignalMismatch.rejected must be 9",
      );
    }
    const cases = readFirstValue(publicSignalMismatch, "cases");
    if (!Array.isArray(cases) || cases.length !== 9) {
      problems.push(
        "Groth16 proof self-test adversarial publicSignalMismatch.cases must contain 9 entries",
      );
    } else {
      for (const [index, entry] of cases.entries()) {
        if (!isRecord(entry)) {
          problems.push(
            `Groth16 proof self-test adversarial publicSignalMismatch.cases[${index}] must be an object`,
          );
          continue;
        }
        if (readFirstValue(entry, "index") !== index) {
          problems.push(
            `Groth16 proof self-test adversarial publicSignalMismatch.cases[${index}].index must be ${index}`,
          );
        }
        if (readFirstValue(entry, "name") !== BSC_GROTH16_PUBLIC_SIGNAL_NAMES[index]) {
          problems.push(
            `Groth16 proof self-test adversarial publicSignalMismatch.cases[${index}].name must be ${BSC_GROTH16_PUBLIC_SIGNAL_NAMES[index]}`,
          );
        }
        if (readFirstValue(entry, "rejected") !== true) {
          problems.push(
            `Groth16 proof self-test adversarial publicSignalMismatch.cases[${index}].rejected must be true`,
          );
        }
        if (readFirstValue(entry, "phase") !== "wtnsCalculate") {
          problems.push(
            `Groth16 proof self-test adversarial publicSignalMismatch.cases[${index}].phase must be wtnsCalculate`,
          );
        }
      }
    }
  }

  const nonBooleanValueBit = readFirstRecord(
    checks,
    "nonBooleanValueBit",
    "non_boolean_value_bit",
  );
  if (!nonBooleanValueBit) {
    problems.push(
      "Groth16 proof self-test adversarialChecks.nonBooleanValueBit is required",
    );
  } else {
    if (readFirstValue(nonBooleanValueBit, "attempted") !== 1) {
      problems.push(
        "Groth16 proof self-test adversarial nonBooleanValueBit.attempted must be 1",
      );
    }
    if (readFirstValue(nonBooleanValueBit, "rejected") !== 1) {
      problems.push(
        "Groth16 proof self-test adversarial nonBooleanValueBit.rejected must be 1",
      );
    }
    const testCase = readFirstRecord(nonBooleanValueBit, "case");
    if (!testCase) {
      problems.push(
        "Groth16 proof self-test adversarial nonBooleanValueBit.case is required",
      );
    } else {
      if (readFirstValue(testCase, "signalName", "signal_name") !== BSC_GROTH16_PUBLIC_SIGNAL_NAMES[0]) {
        problems.push(
          `Groth16 proof self-test adversarial nonBooleanValueBit.case.signalName must be ${BSC_GROTH16_PUBLIC_SIGNAL_NAMES[0]}`,
        );
      }
      if (readFirstValue(testCase, "inputName", "input_name") !== "messageIdBits") {
        problems.push(
          "Groth16 proof self-test adversarial nonBooleanValueBit.case.inputName must be messageIdBits",
        );
      }
      if (readFirstValue(testCase, "bitIndex", "bit_index") !== 0) {
        problems.push(
          "Groth16 proof self-test adversarial nonBooleanValueBit.case.bitIndex must be 0",
        );
      }
      if (readFirstValue(testCase, "rejected") !== true) {
        problems.push(
          "Groth16 proof self-test adversarial nonBooleanValueBit.case.rejected must be true",
        );
      }
      if (readFirstValue(testCase, "phase") !== "wtnsCalculate") {
        problems.push(
          "Groth16 proof self-test adversarial nonBooleanValueBit.case.phase must be wtnsCalculate",
        );
      }
    }
  }
  return problems;
}

function unknownGroth16ProofSelfTestFields(record, allowedFields, label) {
  if (!isRecord(record)) {
    return [];
  }
  return Object.keys(record)
    .filter((key) => !allowedFields.has(key))
    .map((key) => `${label} contains unknown field: ${key}`);
}

function groth16ProofSelfTestAliasProblems(record, groups, label) {
  if (!isRecord(record)) {
    return [];
  }
  const problems = [];
  for (const [fieldLabel, keys] of groups) {
    const presentKeys = keys.filter((key) => hasOwn(record, key));
    if (presentKeys.length > 1) {
      problems.push(
        `${label} ${fieldLabel} must not use multiple aliases: ${presentKeys.join(", ")}`,
      );
    }
  }
  return problems;
}

function bscGroth16ProofSelfTestShapeProblems(report) {
  const problems = [
    ...unknownGroth16ProofSelfTestFields(
      report,
      new Set([
        "schema",
        "routeId",
        "route_id",
        "assetKey",
        "asset_key",
        "bscNetwork",
        "bsc_network",
        "network",
        "chain",
        "chainIdHex",
        "chain_id_hex",
        "networkIdHex",
        "network_id_hex",
        "circuitProfile",
        "circuit_profile",
        "proofBackend",
        "proof_backend",
        "proofFamily",
        "proof_family",
        "generatedAt",
        "generated_at",
        "manifest",
        "artifacts",
        "sample",
        "witnessHash",
        "witness_hash",
        "proofHash",
        "proof_hash",
        "publicSignalsHash",
        "public_signals_hash",
        "snarkjs",
        "adversarialChecks",
        "adversarial_checks",
        "proof",
        "publicSignals",
        "public_signals",
      ]),
      "Groth16 proof self-test report",
    ),
    ...groth16ProofSelfTestAliasProblems(
      report,
      [
        ["routeId", ["routeId", "route_id"]],
        ["assetKey", ["assetKey", "asset_key"]],
        ["bscNetwork", ["bscNetwork", "bsc_network", "network"]],
        ["chainIdHex", ["chainIdHex", "chain_id_hex"]],
        ["networkIdHex", ["networkIdHex", "network_id_hex"]],
        ["circuitProfile", ["circuitProfile", "circuit_profile"]],
        ["proofBackend", ["proofBackend", "proof_backend"]],
        ["proofFamily", ["proofFamily", "proof_family"]],
        ["generatedAt", ["generatedAt", "generated_at"]],
        ["witnessHash", ["witnessHash", "witness_hash"]],
        ["proofHash", ["proofHash", "proof_hash"]],
        ["publicSignalsHash", ["publicSignalsHash", "public_signals_hash"]],
        ["adversarialChecks", ["adversarialChecks", "adversarial_checks"]],
        ["publicSignals", ["publicSignals", "public_signals"]],
      ],
      "Groth16 proof self-test report",
    ),
  ];
  const manifest = readFirstValue(report, "manifest");
  problems.push(
    ...unknownGroth16ProofSelfTestFields(
      manifest,
      new Set([
        "path",
        "sha256",
        "manifestSha256",
        "manifest_sha256",
        "productionReady",
        "production_ready",
        "productionBlockers",
        "production_blockers",
      ]),
      "Groth16 proof self-test manifest",
    ),
    ...groth16ProofSelfTestAliasProblems(
      manifest,
      [
        ["sha256", ["sha256", "manifestSha256", "manifest_sha256"]],
        ["productionReady", ["productionReady", "production_ready"]],
        ["productionBlockers", ["productionBlockers", "production_blockers"]],
      ],
      "Groth16 proof self-test manifest",
    ),
  );
  const artifacts = readFirstValue(report, "artifacts");
  problems.push(
    ...unknownGroth16ProofSelfTestFields(
      artifacts,
      new Set([
        "circuitSource",
        "circuit_source",
        "r1cs",
        "provingKey",
        "proving_key",
        "snarkjsVerificationKey",
        "snarkjs_verification_key",
        "bscVerifierKey",
        "bsc_verifier_key",
        "witnessWasm",
        "witness_wasm",
      ]),
      "Groth16 proof self-test artifacts",
    ),
    ...groth16ProofSelfTestAliasProblems(
      artifacts,
      [
        ["circuitSource", ["circuitSource", "circuit_source"]],
        ["provingKey", ["provingKey", "proving_key"]],
        [
          "snarkjsVerificationKey",
          ["snarkjsVerificationKey", "snarkjs_verification_key"],
        ],
        ["bscVerifierKey", ["bscVerifierKey", "bsc_verifier_key"]],
        ["witnessWasm", ["witnessWasm", "witness_wasm"]],
      ],
      "Groth16 proof self-test artifacts",
    ),
  );
  if (isRecord(artifacts)) {
    for (const [keys, label] of [
      [["circuitSource", "circuit_source"], "circuit source artifact"],
      [["r1cs"], "R1CS artifact"],
      [["provingKey", "proving_key"], "proving key artifact"],
      [
        ["snarkjsVerificationKey", "snarkjs_verification_key"],
        "SnarkJS verification key artifact",
      ],
      [["bscVerifierKey", "bsc_verifier_key"], "BSC verifier key artifact"],
      [["witnessWasm", "witness_wasm"], "witness WASM artifact"],
    ]) {
      problems.push(
        ...unknownGroth16ProofSelfTestFields(
          readFirstValue(artifacts, ...keys),
          new Set(["path", "sha256", "hash", "artifactHash", "artifact_hash"]),
          `Groth16 proof self-test ${label}`,
        ),
        ...groth16ProofSelfTestAliasProblems(
          readFirstValue(artifacts, ...keys),
          [["sha256", ["sha256", "hash", "artifactHash", "artifact_hash"]]],
          `Groth16 proof self-test ${label}`,
        ),
      );
    }
  }
  const sample = readFirstValue(report, "sample");
  problems.push(
    ...unknownGroth16ProofSelfTestFields(
      sample,
      new Set([
        "id",
        "syntheticInputWords",
        "synthetic_input_words",
        "publicSignalNames",
        "public_signal_names",
        "publicSignalWords",
        "public_signal_words",
        "inputSha256",
        "input_sha256",
      ]),
      "Groth16 proof self-test sample",
    ),
    ...groth16ProofSelfTestAliasProblems(
      sample,
      [
        [
          "syntheticInputWords",
          ["syntheticInputWords", "synthetic_input_words"],
        ],
        ["publicSignalNames", ["publicSignalNames", "public_signal_names"]],
        ["publicSignalWords", ["publicSignalWords", "public_signal_words"]],
        ["inputSha256", ["inputSha256", "input_sha256"]],
      ],
      "Groth16 proof self-test sample",
    ),
  );
  const syntheticInputWords = isRecord(sample)
    ? readFirstValue(sample, "syntheticInputWords", "synthetic_input_words")
    : null;
  problems.push(
    ...unknownGroth16ProofSelfTestFields(
      syntheticInputWords,
      new Set(BSC_GROTH16_PUBLIC_SIGNAL_NAMES),
      "Groth16 proof self-test sample.syntheticInputWords",
    ),
  );
  const snarkjs = readFirstValue(report, "snarkjs");
  problems.push(
    ...unknownGroth16ProofSelfTestFields(
      snarkjs,
      new Set(["binary", "wtnsCalculate", "groth16Prove", "groth16Verify"]),
      "Groth16 proof self-test snarkjs",
    ),
  );
  const adversarialChecks = readFirstValue(
    report,
    "adversarialChecks",
    "adversarial_checks",
  );
  problems.push(
    ...unknownGroth16ProofSelfTestFields(
      adversarialChecks,
      new Set([
        "publicSignalMismatch",
        "public_signal_mismatch",
        "nonBooleanValueBit",
        "non_boolean_value_bit",
      ]),
      "Groth16 proof self-test adversarialChecks",
    ),
    ...groth16ProofSelfTestAliasProblems(
      adversarialChecks,
      [
        [
          "publicSignalMismatch",
          ["publicSignalMismatch", "public_signal_mismatch"],
        ],
        [
          "nonBooleanValueBit",
          ["nonBooleanValueBit", "non_boolean_value_bit"],
        ],
      ],
      "Groth16 proof self-test adversarialChecks",
    ),
  );
  const publicSignalMismatch = isRecord(adversarialChecks)
    ? readFirstValue(
        adversarialChecks,
        "publicSignalMismatch",
        "public_signal_mismatch",
      )
    : null;
  problems.push(
    ...unknownGroth16ProofSelfTestFields(
      publicSignalMismatch,
      new Set(["attempted", "rejected", "cases"]),
      "Groth16 proof self-test adversarialChecks.publicSignalMismatch",
    ),
  );
  const publicSignalCases = isRecord(publicSignalMismatch)
    ? readFirstValue(publicSignalMismatch, "cases")
    : null;
  if (Array.isArray(publicSignalCases)) {
    for (const [index, entry] of publicSignalCases.entries()) {
      problems.push(
        ...unknownGroth16ProofSelfTestFields(
          entry,
          new Set(["index", "name", "phase", "rejected"]),
          `Groth16 proof self-test adversarialChecks.publicSignalMismatch.cases[${index}]`,
        ),
      );
    }
  }
  const nonBooleanValueBit = isRecord(adversarialChecks)
    ? readFirstValue(
        adversarialChecks,
        "nonBooleanValueBit",
        "non_boolean_value_bit",
      )
    : null;
  problems.push(
    ...unknownGroth16ProofSelfTestFields(
      nonBooleanValueBit,
      new Set(["attempted", "rejected", "case"]),
      "Groth16 proof self-test adversarialChecks.nonBooleanValueBit",
    ),
  );
  const nonBooleanCase = isRecord(nonBooleanValueBit)
    ? readFirstValue(nonBooleanValueBit, "case")
    : null;
  problems.push(
    ...unknownGroth16ProofSelfTestFields(
      nonBooleanCase,
      new Set([
        "signalName",
        "signal_name",
        "inputName",
        "input_name",
        "bitIndex",
        "bit_index",
        "phase",
        "rejected",
      ]),
      "Groth16 proof self-test adversarialChecks.nonBooleanValueBit.case",
    ),
    ...groth16ProofSelfTestAliasProblems(
      nonBooleanCase,
      [
        ["signalName", ["signalName", "signal_name"]],
        ["inputName", ["inputName", "input_name"]],
        ["bitIndex", ["bitIndex", "bit_index"]],
      ],
      "Groth16 proof self-test adversarialChecks.nonBooleanValueBit.case",
    ),
  );
  problems.push(
    ...unknownGroth16ProofSelfTestFields(
      readFirstValue(report, "proof"),
      new Set(["pi_a", "pi_b", "pi_c", "protocol", "curve"]),
      "Groth16 proof self-test proof",
    ),
  );
  return problems;
}

function groth16ProofSelfTestDecimalWordProblems(values, expectedLength, label) {
  if (!Array.isArray(values) || values.length !== expectedLength) {
    return [
      `${label} must contain ${expectedLength} canonical decimal BN254 field words`,
    ];
  }
  return values.flatMap((entry, index) => {
    try {
      normalizeBn254DecimalWord(entry, `${label}[${index}]`);
      return [];
    } catch (error) {
      return [error instanceof Error ? error.message : String(error)];
    }
  });
}

function bscGroth16ProofSelfTestProofProblems(proof) {
  if (!isRecord(proof)) {
    return ["Groth16 proof self-test proof object is required"];
  }
  const problems = [];
  if (readFirstValue(proof, "protocol") !== "groth16") {
    problems.push("Groth16 proof self-test proof.protocol must be groth16");
  }
  if (readFirstValue(proof, "curve") !== "bn128") {
    problems.push("Groth16 proof self-test proof.curve must be bn128");
  }
  problems.push(
    ...groth16ProofSelfTestDecimalWordProblems(
      readFirstValue(proof, "pi_a"),
      3,
      "Groth16 proof self-test proof.pi_a",
    ),
  );
  const piB = readFirstValue(proof, "pi_b");
  if (!Array.isArray(piB) || piB.length !== 3) {
    problems.push("Groth16 proof self-test proof.pi_b must contain 3 coordinate pairs");
  } else {
    for (const [index, row] of piB.entries()) {
      problems.push(
        ...groth16ProofSelfTestDecimalWordProblems(
          row,
          2,
          `Groth16 proof self-test proof.pi_b[${index}]`,
        ),
      );
    }
  }
  problems.push(
    ...groth16ProofSelfTestDecimalWordProblems(
      readFirstValue(proof, "pi_c"),
      3,
      "Groth16 proof self-test proof.pi_c",
    ),
  );
  return problems;
}

function validateBscGroth16ProofSelfTestReport({
  report,
  artifact,
  profile,
  proofArtifact,
  provingKey,
  verifierKey,
  verifierMaterial,
  groth16MaterialManifest,
}) {
  if (!isRecord(report)) {
    throw new Error("Groth16 proof self-test report must be a JSON object.");
  }
  const problems = [...bscGroth16ProofSelfTestShapeProblems(report)];
  const check = (fn) => {
    try {
      const problem = fn();
      if (problem) {
        problems.push(problem);
      }
    } catch (error) {
      problems.push(error instanceof Error ? error.message : String(error));
    }
  };
  check(() =>
    readFirstString(report, "schema") === BSC_GROTH16_PROOF_SELF_TEST_SCHEMA
      ? ""
      : `Groth16 proof self-test schema must be ${BSC_GROTH16_PROOF_SELF_TEST_SCHEMA}`,
  );
  check(() =>
    readFirstString(report, "routeId", "route_id") === ROUTE_ID
      ? ""
      : `Groth16 proof self-test routeId must be ${ROUTE_ID}`,
  );
  check(() =>
    readFirstString(report, "assetKey", "asset_key") === ASSET_KEY
      ? ""
      : `Groth16 proof self-test assetKey must be ${ASSET_KEY}`,
  );
  check(() =>
    readFirstString(report, "bscNetwork", "bsc_network", "network") ===
    profile.key
      ? ""
      : `Groth16 proof self-test bscNetwork must be ${profile.key}`,
  );
  check(() =>
    readFirstString(report, "chain") === profile.chain
      ? ""
      : `Groth16 proof self-test chain must be ${profile.chain}`,
  );
  check(() =>
    readFirstString(report, "chainIdHex", "chain_id_hex") === profile.chainIdHex
      ? ""
      : `Groth16 proof self-test chainIdHex must be ${profile.chainIdHex}`,
  );
  check(() => {
    const networkIdHex = normalizeCanonicalHex32(
      readFirstValue(report, "networkIdHex", "network_id_hex"),
      "Groth16 proof self-test networkIdHex",
    );
    return networkIdHex === profile.networkIdHex
      ? ""
      : `Groth16 proof self-test networkIdHex must be ${profile.networkIdHex}`;
  });
  check(() =>
    readFirstString(report, "circuitProfile", "circuit_profile") ===
    BSC_FULL_SCCP_CIRCUIT_PROFILE
      ? ""
      : `Groth16 proof self-test circuitProfile must be ${BSC_FULL_SCCP_CIRCUIT_PROFILE}`,
  );
  check(() =>
    readFirstString(report, "proofBackend", "proof_backend") ===
    BSC_EVM_GROTH16_BACKEND
      ? ""
      : `Groth16 proof self-test proofBackend must be ${BSC_EVM_GROTH16_BACKEND}`,
  );
  check(() =>
    readFirstString(report, "proofFamily", "proof_family") ===
    SCCP_PROOF_FAMILY_STARK_FRI
      ? ""
      : `Groth16 proof self-test proofFamily must be ${SCCP_PROOF_FAMILY_STARK_FRI}`,
  );
  const manifestBlock = readFirstRecord(report, "manifest") ?? {};
  check(() =>
    groth16ProofSelfTestPathProblem(
      manifestBlock,
      ["path"],
      groth16MaterialManifest.artifact.path,
      "Groth16 proof self-test manifest",
    ),
  );
  check(() =>
    groth16MaterialManifestHash(
      manifestBlock,
      ["sha256", "manifestSha256", "manifest_sha256"],
      "Groth16 proof self-test manifest sha256",
    ) === groth16MaterialManifest.artifact.sha256
      ? ""
      : "Groth16 proof self-test manifest sha256 must match signed material manifest",
  );
  check(() =>
    readFirstValue(manifestBlock, "productionReady", "production_ready") ===
    true
      ? ""
      : "Groth16 proof self-test manifest.productionReady must be true",
  );
  const productionBlockers =
    readFirstValue(manifestBlock, "productionBlockers", "production_blockers") ??
    [];
  if (!Array.isArray(productionBlockers) || productionBlockers.length > 0) {
    problems.push(
      "Groth16 proof self-test manifest.productionBlockers must be an empty array",
    );
  }
  let circuitSourceHash = "";
  let snarkjsVerificationKeyHash = "";
  let witnessWasmHash = "";
  check(() =>
    readGroth16ProofSelfTestArtifactPath(
      report,
      ["circuitSource", "circuit_source"],
      "circuit source",
    ) ===
    groth16MaterialManifestArtifactPath(
      groth16MaterialManifest.manifest,
      ["circuitSource", "circuit_source"],
      "circuit source",
    )
      ? ""
      : "Groth16 proof self-test circuit source path must match signed material manifest",
  );
  check(() =>
    readGroth16ProofSelfTestArtifactHash(
      report,
      ["circuitSource", "circuit_source"],
      "circuit source",
    ) ===
    groth16MaterialManifestArtifactHash(
      groth16MaterialManifest.manifest,
      ["circuitSource", "circuit_source"],
      "circuit source",
    )
      ? ""
      : "Groth16 proof self-test circuit source hash must match signed material manifest",
  );
  check(() =>
    readGroth16ProofSelfTestArtifactPath(report, ["r1cs"], "R1CS") ===
    proofArtifact.path
      ? ""
      : `Groth16 proof self-test R1CS path must be ${proofArtifact.path}`,
  );
  check(() =>
    readGroth16ProofSelfTestArtifactHash(report, ["r1cs"], "R1CS") ===
    proofArtifact.sha256
      ? ""
      : "Groth16 proof self-test R1CS hash must match proof artifact",
  );
  check(() =>
    readGroth16ProofSelfTestArtifactPath(
      report,
      ["provingKey", "proving_key"],
      "proving key",
    ) === provingKey.path
      ? ""
      : `Groth16 proof self-test proving key path must be ${provingKey.path}`,
  );
  check(() =>
    readGroth16ProofSelfTestArtifactHash(
      report,
      ["provingKey", "proving_key"],
      "proving key",
    ) === provingKey.sha256
      ? ""
      : "Groth16 proof self-test proving key hash must match proving key",
  );
  check(() =>
    readGroth16ProofSelfTestArtifactPath(
      report,
      ["bscVerifierKey", "bsc_verifier_key"],
      "BSC verifier key",
    ) === verifierKey.path
      ? ""
      : `Groth16 proof self-test BSC verifier key path must be ${verifierKey.path}`,
  );
  check(() =>
    readGroth16ProofSelfTestArtifactHash(
      report,
      ["bscVerifierKey", "bsc_verifier_key"],
      "BSC verifier key",
    ) === verifierKey.sha256
      ? ""
      : "Groth16 proof self-test BSC verifier key hash must match verifier key artifact",
  );
  check(() =>
    readGroth16ProofSelfTestArtifactPath(
      report,
      ["snarkjsVerificationKey", "snarkjs_verification_key"],
      "SnarkJS verification key",
    ) ===
    groth16MaterialManifestArtifactPath(
      groth16MaterialManifest.manifest,
      ["snarkjsVerificationKey", "snarkjs_verification_key"],
      "SnarkJS verification key",
    )
      ? ""
      : "Groth16 proof self-test SnarkJS verification key path must match signed material manifest",
  );
  check(() => {
    snarkjsVerificationKeyHash = readGroth16ProofSelfTestArtifactHash(
      report,
      ["snarkjsVerificationKey", "snarkjs_verification_key"],
      "SnarkJS verification key",
    );
    return snarkjsVerificationKeyHash ===
      groth16MaterialManifestArtifactHash(
        groth16MaterialManifest.manifest,
        ["snarkjsVerificationKey", "snarkjs_verification_key"],
        "SnarkJS verification key",
      )
      ? ""
      : "Groth16 proof self-test SnarkJS verification key hash must match signed material manifest";
  });
  check(() => {
    circuitSourceHash = readGroth16ProofSelfTestArtifactHash(
      report,
      ["circuitSource", "circuit_source"],
      "circuit source",
    );
    return "";
  });
  check(() => {
    witnessWasmHash = readGroth16ProofSelfTestArtifactHash(
      report,
      ["witnessWasm", "witness_wasm"],
      "witness WASM",
    );
    return "";
  });
  void verifierMaterial;
  const sample = readFirstRecord(report, "sample") ?? {};
  const expectedSample = bscGroth16DeterministicProofSelfTestSample(profile);
  if (readFirstString(sample, "id") !== expectedSample.sampleId) {
    problems.push(
      `Groth16 proof self-test sample.id must be ${expectedSample.sampleId}`,
    );
  }
  const syntheticInputWords = readFirstValue(
    sample,
    "syntheticInputWords",
    "synthetic_input_words",
  );
  if (
    !isRecord(syntheticInputWords) ||
    JSON.stringify(syntheticInputWords) !==
      JSON.stringify(expectedSample.syntheticInputWords)
  ) {
    problems.push(
      "Groth16 proof self-test sample.syntheticInputWords must match deterministic BSC Groth16 self-test input",
    );
  }
  check(() => {
    const inputSha256 = groth16MaterialManifestHash(
      sample,
      ["inputSha256", "input_sha256"],
      "Groth16 proof self-test sample.inputSha256",
    );
    return inputSha256 === expectedSample.inputSha256
      ? ""
      : "Groth16 proof self-test sample.inputSha256 must match deterministic self-test input";
  });
  const publicSignalNames = readFirstValue(sample, "publicSignalNames");
  if (
    !Array.isArray(publicSignalNames) ||
    JSON.stringify(publicSignalNames) !==
      JSON.stringify(BSC_GROTH16_PUBLIC_SIGNAL_NAMES)
  ) {
    problems.push(
      "Groth16 proof self-test publicSignalNames must match BSC Groth16 public signals",
    );
  }
  let normalizedSamplePublicSignals = null;
  const samplePublicSignalWords = readFirstValue(
    sample,
    "publicSignalWords",
    "public_signal_words",
  );
  try {
    normalizedSamplePublicSignals = normalizeGroth16ProofSelfTestPublicSignals(
      samplePublicSignalWords,
      "Groth16 proof self-test sample.publicSignalWords",
    );
    if (
      JSON.stringify(normalizedSamplePublicSignals) !==
      JSON.stringify(expectedSample.publicSignalWords)
    ) {
      problems.push(
        "Groth16 proof self-test sample.publicSignalWords must match deterministic BSC Groth16 self-test input",
      );
    }
  } catch (error) {
    problems.push(error instanceof Error ? error.message : String(error));
  }
  let normalizedPublicSignals = null;
  const publicSignals = readFirstValue(report, "publicSignals", "public_signals");
  try {
    normalizedPublicSignals = normalizeGroth16ProofSelfTestPublicSignals(
      publicSignals,
      "Groth16 proof self-test publicSignals",
    );
    if (
      normalizedSamplePublicSignals &&
      JSON.stringify(normalizedPublicSignals) !==
        JSON.stringify(normalizedSamplePublicSignals)
    ) {
      problems.push(
        "Groth16 proof self-test publicSignals must match sample.publicSignalWords",
      );
    }
  } catch (error) {
    problems.push(error instanceof Error ? error.message : String(error));
  }
  const snarkjs = readFirstRecord(report, "snarkjs") ?? {};
  for (const key of ["wtnsCalculate", "groth16Prove", "groth16Verify"]) {
    if (readFirstValue(snarkjs, key) !== true) {
      problems.push(`Groth16 proof self-test snarkjs.${key} must be true`);
    }
  }
  let witnessHash = "";
  let proofHash = "";
  let publicSignalsHash = "";
  check(() => {
    witnessHash = groth16MaterialManifestHash(
      report,
      ["witnessHash", "witness_hash"],
      "Groth16 proof self-test witnessHash",
    );
    return "";
  });
  check(() => {
    proofHash = groth16MaterialManifestHash(
      report,
      ["proofHash", "proof_hash"],
      "Groth16 proof self-test proofHash",
    );
    return "";
  });
  check(() => {
    publicSignalsHash = groth16MaterialManifestHash(
      report,
      ["publicSignalsHash", "public_signals_hash"],
      "Groth16 proof self-test publicSignalsHash",
    );
    return "";
  });
  if (normalizedPublicSignals && publicSignalsHash) {
    const actualPublicSignalsHash = sha256HexBytes(
      Buffer.from(canonicalJson(normalizedPublicSignals), "utf8"),
    );
    if (actualPublicSignalsHash !== publicSignalsHash) {
      problems.push(
        "Groth16 proof self-test publicSignalsHash must match publicSignals",
      );
    }
  }
  const proof = readFirstRecord(report, "proof");
  if (!proof) {
    problems.push("Groth16 proof self-test proof object is required");
  } else {
    problems.push(...bscGroth16ProofSelfTestProofProblems(proof));
    if (proofHash) {
      const actualProofHash = sha256HexBytes(
        Buffer.from(canonicalJson(proof), "utf8"),
      );
      if (actualProofHash !== proofHash) {
        problems.push("Groth16 proof self-test proofHash must match proof");
      }
    }
  }
  problems.push(...bscGroth16ProofSelfTestAdversarialProblems(report));
  const roleHashes = [
    ["report", artifact.sha256],
    ["circuit source", circuitSourceHash],
    ["R1CS proof artifact", proofArtifact.sha256],
    ["proving key", provingKey.sha256],
    ["BSC verifier key artifact", verifierKey.sha256],
    ["SnarkJS verification key", snarkjsVerificationKeyHash],
    ["witness WASM", witnessWasmHash],
    ["witness", witnessHash],
    ["proof", proofHash],
    ["public signals", publicSignalsHash],
  ].filter(([, value]) => value);
  const seenRoleHashes = new Map();
  for (const [label, value] of roleHashes) {
    const previous = seenRoleHashes.get(value);
    if (previous) {
      problems.push(
        `Groth16 proof self-test ${label} hash must be role-separated from ${previous} hash`,
      );
    } else {
      seenRoleHashes.set(value, label);
    }
  }
  if (problems.length > 0) {
    throw new Error(
      `Groth16 proof self-test report is not production-ready: ${uniqueNonEmpty(problems).join("; ")}`,
    );
  }
}

async function verifyBscGroth16ProofSelfTestWithSnarkjs({
  root,
  options,
  report,
  groth16MaterialManifest,
}) {
  const snarkjsBin = requiredOption(
    options,
    "snarkjs-bin",
    "native-prover-bundle SnarkJS proof verifier",
  );
  const manifestSnarkjs = readFirstRecord(
    readFirstRecord(
      groth16MaterialManifest.manifest,
      "selfChecks",
      "self_checks",
    ),
    "snarkjs",
    "snark_js",
  );
  const manifestSnarkjsBinary = readFirstString(
    manifestSnarkjs,
    "snarkjsBinary",
    "snarkjs_binary",
  );
  if (!manifestSnarkjsBinary) {
    throw new Error(
      "Groth16 material manifest SnarkJS binary command is required.",
    );
  }
  if (String(snarkjsBin) !== manifestSnarkjsBinary) {
    throw new Error(
      "native-prover-bundle --snarkjs-bin must match signed Groth16 material manifest selfChecks.snarkjs.snarkjsBinary.",
    );
  }
  const expectedSnarkjsBinarySha256 =
    groth16MaterialManifest.transcriptEvidence
      ?.reproducibleBuildSnarkjsBinarySha256;
  if (!expectedSnarkjsBinarySha256) {
    throw new Error(
      "reproducible build transcript toolchain.snarkjs.binarySha256 is required.",
    );
  }
  let actualSnarkjsBinarySha256;
  try {
    actualSnarkjsBinarySha256 = sha256HexBytes(await readFile(String(snarkjsBin)));
  } catch (error) {
    throw new Error(
      `native-prover-bundle --snarkjs-bin must be a readable file for reproducible build transcript binary hash verification: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
  if (actualSnarkjsBinarySha256 !== expectedSnarkjsBinarySha256) {
    throw new Error(
      "native-prover-bundle --snarkjs-bin sha256 must match reproducible build transcript toolchain.snarkjs.binarySha256.",
    );
  }
  const snarkjsVerificationKeyPath = groth16MaterialManifestArtifactPath(
    groth16MaterialManifest.manifest,
    ["snarkjsVerificationKey", "snarkjs_verification_key"],
    "SnarkJS verification key",
  );
  const snarkjsVerificationKeyHash = groth16MaterialManifestArtifactHash(
    groth16MaterialManifest.manifest,
    ["snarkjsVerificationKey", "snarkjs_verification_key"],
    "SnarkJS verification key",
  );
  const snarkjsVerificationKey = await readArtifactUnderRoot(
    root,
    snarkjsVerificationKeyPath,
    "SnarkJS verification key",
  );
  if (snarkjsVerificationKey.sha256 !== snarkjsVerificationKeyHash) {
    throw new Error(
      "SnarkJS verification key artifact hash must match signed Groth16 material manifest.",
    );
  }
  const publicSignals = normalizeGroth16ProofSelfTestPublicSignals(
    readFirstValue(report, "publicSignals", "public_signals"),
    "Groth16 proof self-test publicSignals",
  );
  const proof = readFirstRecord(report, "proof");
  if (!proof) {
    throw new Error("Groth16 proof self-test proof object is required");
  }
  const tempRoot = await mkdtemp(
    join(tmpdir(), "iroha-bsc-native-groth16-proof-verify-"),
  );
  try {
    const publicPath = join(tempRoot, "public.json");
    const proofPath = join(tempRoot, "proof.json");
    await writeFile(publicPath, `${JSON.stringify(publicSignals)}\n`, {
      mode: 0o600,
    });
    await writeFile(proofPath, `${JSON.stringify(proof)}\n`, { mode: 0o600 });
    await runCommand(snarkjsBin, [
      "groth16",
      "verify",
      snarkjsVerificationKey.absolutePath,
      publicPath,
      proofPath,
    ]);
  } catch (error) {
    throw new Error(
      `Groth16 proof self-test embedded Groth16 proof must verify against SnarkJS verification key: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
}

async function readRequiredBscGroth16ProofSelfTestReport({
  root,
  options,
  profile,
  proofArtifact,
  provingKey,
  verifierKey,
  verifierMaterial,
  groth16MaterialManifest,
}) {
  const value = optionValue(options, [
    "groth16-proof-self-test",
    "groth16-proof-self-test-report",
    "proof-self-test",
  ]);
  if (value === undefined || value === null || trim(value) === "") {
    throw new Error(
      "native-prover-bundle requires --groth16-proof-self-test.",
    );
  }
  const artifact = await readArtifactUnderRoot(
    root,
    value,
    "Groth16 proof self-test report",
  );
  let report;
  try {
    report = parseJsonWithoutDuplicateKeys(
      artifact.bytes.toString("utf8"),
      "Groth16 proof self-test report",
    );
  } catch (error) {
    throw new Error(
      `Groth16 proof self-test report must be valid duplicate-free JSON: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
  const reason = unsafeSecretReason(report, "Groth16 proof self-test report");
  if (reason) {
    throw new Error(reason);
  }
  validateBscGroth16ProofSelfTestReport({
    report,
    artifact,
    profile,
    proofArtifact,
    provingKey,
    verifierKey,
    verifierMaterial,
    groth16MaterialManifest,
  });
  await verifyBscGroth16ProofSelfTestWithSnarkjs({
    root,
    options,
    report,
    groth16MaterialManifest,
  });
  return { artifact, report };
}

function sdkImplementationOptionName(sdk) {
  return `${sdk}-implementation`;
}

function buildNativeEvmProverBundleObject({
  routeBinding,
  proofArtifact,
  provingKey,
  verifierKey,
  groth16ProofSelfTest,
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
    cross_sdk_parity_artifact: parityFixture.path,
    native_prover_self_test_artifact: selfTestFixture.path,
    groth16_proof_self_test_artifact: groth16ProofSelfTest.path,
    groth16_proof_self_test_hash: groth16ProofSelfTest.sha256,
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

export function bscNativeProverReportProductionAttestationHash(
  kind,
  groth16MaterialManifestSha256,
) {
  const manifestHash = normalizeCanonicalHex32(
    groth16MaterialManifestSha256,
    "Groth16 material manifest sha256",
  );
  const role =
    kind === "cross-sdk-parity"
      ? "cross-sdk-parity"
      : kind === "native-prover-self-test"
        ? "native-prover-self-test"
        : "";
  if (!role) {
    throw new Error("native prover report production attestation kind is invalid.");
  }
  return bytesToHex(
    sha256(
      textEncoder.encode(
        `iroha-sccp-bsc-native-prover-report-production-attestation/v1:${role}:${manifestHash}`,
      ),
    ),
  );
}

function requireNativeProverReportProductionAttestationBinding({
  parityReport,
  selfTestReport,
  groth16MaterialManifest,
}) {
  const materialManifestHash = groth16MaterialManifest?.artifact?.sha256;
  if (!materialManifestHash) {
    throw new Error("Groth16 material manifest hash is required.");
  }
  const expectedParityHash = bscNativeProverReportProductionAttestationHash(
    "cross-sdk-parity",
    materialManifestHash,
  );
  const expectedSelfTestHash = bscNativeProverReportProductionAttestationHash(
    "native-prover-self-test",
    materialManifestHash,
  );
  if (parityReport.productionAttestationHash !== expectedParityHash) {
    throw new Error(
      "cross-SDK parity production_attestation_hash must be role-derived from the signed Groth16 material manifest sha256.",
    );
  }
  if (selfTestReport.productionAttestationHash !== expectedSelfTestHash) {
    throw new Error(
      "native prover self-test production_attestation_hash must be role-derived from the signed Groth16 material manifest sha256.",
    );
  }
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
  const legacyParityOption = optionValue(options, [
    "cross-sdk-fixture-parity",
    "parity-fixture",
  ]);
  if (legacyParityOption !== undefined) {
    throw new Error(
      "BSC native EVM prover bundles require --cross-sdk-parity; legacy --cross-sdk-fixture-parity/--parity-fixture options are not valid for production material.",
    );
  }
  const parityFixture = await readArtifactUnderRoot(
    root,
    requiredOption(options, "cross-sdk-parity", "cross-SDK parity artifact"),
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
  const groth16MaterialManifest =
    await readRequiredBscGroth16MaterialManifest({
      root,
      options,
      binding,
      profile,
      proofArtifact,
      provingKey,
      verifierKey,
      verifierMaterial,
    });
  const auditHashes = await readNativeProverAuditHashes(root, options, {
    parityFixture,
    selfTestFixture,
    profile,
    sdkArtifacts,
  });
  requireNativeProverAuditHashesBindGroth16Material(
    auditHashes,
    groth16MaterialManifest,
  );
  const groth16ProofSelfTest =
    await readRequiredBscGroth16ProofSelfTestReport({
      root,
      options,
      profile,
      proofArtifact,
      provingKey,
      verifierKey,
      verifierMaterial,
      groth16MaterialManifest,
    });
  const bundle = buildNativeEvmProverBundleObject({
    routeBinding: binding,
    proofArtifact,
    provingKey,
    verifierKey,
    groth16ProofSelfTest: groth16ProofSelfTest.artifact,
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
  const parityReport = parseBscNativeProverParityFixtureForProfile(
    parityFixture.bytes.toString("utf8"),
    descriptor,
    profile,
  );
  const selfTestReport = parseBscNativeProverSelfTestFixtureForProfile(
    selfTestFixture.bytes.toString("utf8"),
    descriptor,
    profile,
  );
  requireNativeProverReportProductionAttestationBinding({
    parityReport,
    selfTestReport,
    groth16MaterialManifest,
  });
  const bytesByPath = new Map([
    [proofArtifact.path, proofArtifact.bytes],
    [provingKey.path, provingKey.bytes],
    [verifierKey.path, verifierKey.bytes],
    [groth16ProofSelfTest.artifact.path, groth16ProofSelfTest.artifact.bytes],
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
      groth16MaterialManifest: groth16MaterialManifest.artifact,
      groth16ProofSelfTest: groth16ProofSelfTest.artifact,
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
          "": ["ast"],
          "*": [
            "abi",
            "evm.bytecode.object",
            "evm.deployedBytecode.object",
            "evm.deployedBytecode.immutableReferences",
          ],
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
    const immutableReferences =
      contract.evm.deployedBytecode?.immutableReferences ?? {};
    const immutableReferenceNames = immutableReferenceNamesForContract(
      output,
      definition,
      immutableReferences,
    );
    artifacts[definition.key] = {
      file: definition.file,
      contractName: definition.contract,
      abi: contract.abi,
      bytecode: `0x${contract.evm.bytecode.object}`,
      deployedBytecode: `0x${contract.evm.deployedBytecode.object}`,
      ...(Object.keys(immutableReferences).length
        ? { immutableReferences, immutableReferenceNames }
        : {}),
      bytecodeKeccak256: bytesToHex(
        keccak_256(
          hexToBytes(
            contract.evm.bytecode.object,
            `${definition.contract} bytecode`,
            null,
            { allowZero: false },
          ),
        ),
      ),
      deployedBytecodeKeccak256: bytesToHex(
        keccak_256(
          hexToBytes(
            contract.evm.deployedBytecode.object,
            `${definition.contract} deployed bytecode`,
            null,
            { allowZero: false },
          ),
        ),
      ),
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

async function readCodeMetadata(provider, addresses) {
  const entries = await Promise.all(
    Object.entries(addresses).map(async ([key, address]) => {
      const code = await provider.getCode(address);
      const present = code !== "0x";
      return [
        key,
        {
          present,
          codeHash: present
            ? bytesToHex(
                keccak_256(
                  hexToBytes(code, `${key} deployed bytecode`, null, {
                    allowZero: false,
                  }),
                ),
              )
            : null,
        },
      ];
    }),
  );
  return {
    codePresent: Object.fromEntries(
      entries.map(([key, metadata]) => [key, metadata.present]),
    ),
    codeHashes: Object.fromEntries(
      entries
        .filter(([, metadata]) => metadata.codeHash)
        .map(([key, metadata]) => [key, metadata.codeHash]),
    ),
  };
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
    codeMetadata,
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
    readCodeMetadata(provider, addresses),
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
    tokenAddress: addresses.token,
    bridgeAddress: addresses.bridge,
    sourceBridgeAddress: addresses.sourceBridge,
    verifierAddress: addresses.verifier,
    codePresent: codeMetadata.codePresent,
    codeHashes: codeMetadata.codeHashes,
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
  const compiledContractCodeHashes = ownValue(
    input,
    "compiledContractCodeHashes",
  );
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
  const readbackCodeHashes = normalizeBscReadbackCodeHashes(readback);
  const compiledCodeHashes = normalizeBscCompiledContractCodeHashes(
    compiledContractCodeHashes,
  );
  assertBscCompiledCodeHashesMatchReadback({
    compiledCodeHashes: compiledCodeHashes,
    readbackCodeHashes,
    verifierCodeHash: codeHash,
  });
  const bindingKey = bscDestinationBindingKey({
    networkId: profile.networkIdHex,
    verifierAddress: addresses.verifier,
    bridgeAddress: addresses.bridge,
    verifierCodeHash: codeHash,
    verifierKeyHash: keyHash,
  });
  const publicReadback = {
    ...Object.fromEntries(ownRecordEntries(readback)),
    tokenAddress: addresses.token,
    bridgeAddress: addresses.bridge,
    sourceBridgeAddress: addresses.sourceBridge,
    verifierAddress: addresses.verifier,
  };
  return {
    schema: DEPLOYMENT_EVIDENCE_SCHEMA,
    routeId: ROUTE_ID,
    assetKey: ASSET_KEY,
    bscNetwork: profile.key,
    chain: profile.chain,
    chainIdHex: profile.chainIdHex,
    networkIdHex: profile.networkIdHex,
    bscBridgeAddress: addresses.bridge,
    bscTokenAddress: addresses.token,
    sccpBscSourceBridgeAddress: addresses.sourceBridge,
    bscVerifierAddress: addresses.verifier,
    ...(compiledCodeHashes
      ? { compiledContractCodeHashes: compiledCodeHashes }
      : {}),
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
    bscContractReadback: publicReadback,
    postDeployChecklist: [
      "TairaXOR.bridge() equals bscBridgeAddress",
      "TairaXOR.bridgeLocked() is true",
      "SccpBscSourceBridge.owner() equals bscBridgeAddress",
      "TairaXorBscSccpBridge.destinationBindingHash() equals destinationRollout.destinationBindingHash",
      "TairaXorBscSccpBridge.verifier() equals bscVerifierAddress",
      "TairaXorBscSccpBridge verifier code/key hashes and domains match destinationRollout",
      "compiledContractCodeHashes match live bscContractReadback.codeHashes",
    ],
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
  assertSingleRecordAlias(
    record,
    ["bscContractReadback", "bsc_contract_readback"],
    "BSC deployment evidence",
    "BSC deployment evidence bscContractReadback",
  );
  assertSingleRecordAlias(
    record,
    ["compiledContractCodeHashes", "compiled_contract_code_hashes"],
    "BSC deployment evidence",
    "BSC deployment evidence compiledContractCodeHashes",
  );
  const contractReadback = readFirstRecord(
    record,
    "bscContractReadback",
    "bsc_contract_readback",
  );
  const requireDeploymentReadback =
    optionEnabled(options, "production-ready", false) ||
    optionEnabled(options, "live-readback-checked", false);
  const compiledCodeHashes = normalizeBscCompiledContractCodeHashes(
    readFirstRecord(
      record,
      "compiledContractCodeHashes",
      "compiled_contract_code_hashes",
    ),
    requireDeploymentReadback,
  );
  if (contractReadback) {
    validateBscReadbackEvidence({
      addresses,
      readback: contractReadback,
      bindingHash: destinationBindingHash,
      verifierCodeHash,
      verifierKeyHash,
      bscNetwork: profile.key,
    });
    assertBscCompiledCodeHashesMatchReadback({
      compiledCodeHashes,
      readbackCodeHashes: normalizeBscReadbackCodeHashes(contractReadback),
      verifierCodeHash,
    });
  } else if (requireDeploymentReadback) {
    throw new Error(
      "production-ready BSC route manifests require embedded bscContractReadback in deployment evidence.",
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
    contractReadback: contractReadback ?? null,
    compiledCodeHashes,
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
  const routeManifestPath = readRequiredString(
    record,
    ["routeManifestPath", "route_manifest_path"],
    "BSC offline full TOML evidence routeManifestPath",
  );
  const routeManifestPathProblem = bscOfflineFullTomlEvidencePathProblem(
    routeManifestPath,
    "BSC offline full TOML evidence routeManifestPath",
  );
  if (routeManifestPathProblem) {
    throw new Error(routeManifestPathProblem);
  }
  const fullConfigPath = readRequiredString(
    record,
    ["fullConfigPath", "full_config_path"],
    "BSC offline full TOML evidence fullConfigPath",
  );
  const fullConfigPathProblem = bscOfflineFullTomlEvidencePathProblem(
    fullConfigPath,
    "BSC offline full TOML evidence fullConfigPath",
  );
  if (fullConfigPathProblem) {
    throw new Error(fullConfigPathProblem);
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

function formatBscRouteMissingReadinessItems(items) {
  if (items.length === 0) {
    return "production readiness acknowledgement";
  }
  if (items.length === 1) {
    return items[0];
  }
  return `${items.slice(0, -1).join(", ")} and ${items.at(-1)}`;
}

function buildBscRouteDraftDisabledReason({
  diagnosticVerifierReasons,
  routeEvidence,
  proofArtifactHash,
  provingKeyHash,
  nativeEvmProverBundleHash,
  destinationBrowserProver,
  sourceBrowserProver,
  postDeployLiveEvidence,
}) {
  if (diagnosticVerifierReasons.length > 0) {
    return "BSC verifier material is diagnostic and must be replaced before production readiness.";
  }
  const missing = [];
  if (!routeEvidence.contractReadback) {
    missing.push("BSC contract readback");
  }
  if (!proofArtifactHash || !provingKeyHash) {
    missing.push("proof artifact and proving key hashes");
  }
  if (!nativeEvmProverBundleHash) {
    missing.push("native EVM prover bundle");
  }
  if (!destinationBrowserProver) {
    missing.push("TAIRA-to-BSC browser prover manifest");
  }
  if (!sourceBrowserProver) {
    missing.push("BSC-to-TAIRA browser prover manifest");
  }
  if (!postDeployLiveEvidence) {
    missing.push("post-deploy live evidence");
  } else {
    if (postDeployLiveEvidence.fullTomlReady !== true) {
      missing.push("offline full-TOML readiness evidence");
    }
    if (!postDeployLiveEvidence.offlineFullTomlSha256) {
      missing.push("offline full-TOML hash");
    }
    if (
      !postDeployLiveEvidence.sourceBridgeConfigHash ||
      !postDeployLiveEvidence.sourceEventTransactionId ||
      !postDeployLiveEvidence.sourceEventExplorerUrl
    ) {
      missing.push("BSC source-event evidence");
    }
    if (
      !postDeployLiveEvidence.routeCanaryEvidenceHash ||
      !postDeployLiveEvidence.routeCanaryTransactionId ||
      !postDeployLiveEvidence.routeCanaryExplorerUrl
    ) {
      missing.push("BSC route-canary evidence");
    }
  }
  return `Route manifest draft is not production-ready; missing ${formatBscRouteMissingReadinessItems(
    missing,
  )}.`;
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
  const deploymentEvidenceSha256 = bscDeploymentEvidenceSha256(routeEvidence);
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
  const browserProverRefContext = {
    profile,
    proofArtifactHash,
    provingKeyHash,
    nativeEvmProverBundleHash,
    destinationBindingHash: routeEvidence.destinationBindingHash,
  };
  const destinationBrowserProver =
    await readBscRouteBrowserProverManifestRef(
      options,
      "destination",
      browserProverRefContext,
    );
  const sourceBrowserProver = await readBscRouteBrowserProverManifestRef(
    options,
    "source",
    browserProverRefContext,
  );
  if (
    productionReady &&
    (!destinationBrowserProver || !sourceBrowserProver)
  ) {
    throw new Error(
      "production-ready BSC route manifests require --destination-browser-prover-manifest and --source-browser-prover-manifest.",
    );
  }
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
          disabledReason: buildBscRouteDraftDisabledReason({
            diagnosticVerifierReasons,
            routeEvidence,
            proofArtifactHash,
            provingKeyHash,
            nativeEvmProverBundleHash,
            destinationBrowserProver,
            sourceBrowserProver,
            postDeployLiveEvidence,
          }),
        }),
    bscTokenAddress: addresses.token,
    bscBridgeAddress: addresses.bridge,
    sccpBscSourceBridgeAddress: addresses.sourceBridge,
    bscVerifierAddress: addresses.verifier,
    deploymentEvidenceSha256,
    ...(proofArtifactHash ? { proofArtifactHash } : {}),
    ...(provingKeyHash ? { provingKeyHash } : {}),
    ...(nativeEvmProverBundleHash ? { nativeEvmProverBundleHash } : {}),
    ...(destinationBrowserProver
      ? { destinationBrowserProver }
      : {}),
    ...(sourceBrowserProver ? { sourceBrowserProver } : {}),
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
  const compact = normalized.toLowerCase().replace(/[-_\s]+/gu, "");
  if (compact === "halo2ipa") {
    return "halo2/ipa";
  }
  if (compact === "starkfri") {
    return "stark/fri";
  }
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
  const deploymentEvidenceSha256 = optionalRouteHash(
    "route manifest deploymentEvidenceSha256",
    ["deploymentEvidenceSha256", "deployment_evidence_sha256"],
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
  if (productionReady && !deploymentEvidenceSha256) {
    throw new Error(
      "route manifest productionReady requires deploymentEvidenceSha256.",
    );
  }
  const roleSeparatedHashes = [
    ["verifierCodeHash", verifierCodeHash],
    ["verifierKeyHash", verifierKeyHash],
    ["destinationBindingHash", null],
    ["proofArtifactHash", proofArtifactHash],
    ["provingKeyHash", provingKeyHash],
    ["deploymentEvidenceSha256", deploymentEvidenceSha256],
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
  assertSingleRecordAlias(
    record,
    ["destinationBrowserProver", "destination_browser_prover"],
    "route manifest",
    "route manifest destinationBrowserProver",
  );
  assertSingleRecordAlias(
    record,
    ["sourceBrowserProver", "source_browser_prover"],
    "route manifest",
    "route manifest sourceBrowserProver",
  );
  const destinationBrowserProverRecord = readFirstRecord(
    record,
    "destinationBrowserProver",
    "destination_browser_prover",
  );
  const sourceBrowserProverRecord = readFirstRecord(
    record,
    "sourceBrowserProver",
    "source_browser_prover",
  );
  const destinationBrowserProver = destinationBrowserProverRecord
    ? normalizeBrowserProverRefRecord(
        destinationBrowserProverRecord,
        "route manifest destinationBrowserProver",
      )
    : null;
  const sourceBrowserProver = sourceBrowserProverRecord
    ? normalizeBrowserProverRefRecord(
        sourceBrowserProverRecord,
        "route manifest sourceBrowserProver",
      )
    : null;
  if (productionReady && (!destinationBrowserProver || !sourceBrowserProver)) {
    throw new Error(
      "route manifest productionReady requires destinationBrowserProver and sourceBrowserProver.",
    );
  }
  for (const [label, ref] of [
    ["destinationBrowserProver", destinationBrowserProver],
    ["sourceBrowserProver", sourceBrowserProver],
  ]) {
    if (!ref) continue;
    if (ref.boundRouteHash !== destinationBindingHash) {
      throw new Error(
        `route manifest ${label}.boundRouteHash must match destinationBindingHash.`,
      );
    }
    if (proofArtifactHash && ref.boundProofHash !== proofArtifactHash) {
      throw new Error(
        `route manifest ${label}.boundProofHash must match proofArtifactHash.`,
      );
    }
    roleSeparatedHashes.push(
      [`${label}.moduleHash`, ref.moduleHash],
      [`${label}.manifestHash`, ref.manifestHash],
    );
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
    deploymentEvidenceSha256,
    nativeEvmProverBundleHash,
    nativeEvmProverBundle,
    destinationBrowserProver,
    sourceBrowserProver,
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

function tomlStringArray(values, label) {
  if (!Array.isArray(values) || values.length === 0) {
    throw new Error(`${label} must be a non-empty string array.`);
  }
  return `[${values
    .map((entry, index) => tomlString(entry, `${label}[${index}]`))
    .join(", ")}]`;
}

function tomlBrowserProverRefLine(key, ref, label) {
  if (!ref) return [];
  const entries = [
    `module_url = ${tomlString(ref.moduleUrl, `${label}.moduleUrl`)}`,
    ...(ref.moduleSpecifier
      ? [
          `module_specifier = ${tomlString(
            ref.moduleSpecifier,
            `${label}.moduleSpecifier`,
          )}`,
        ]
      : []),
    `module_hash = ${tomlString(ref.moduleHash, `${label}.moduleHash`)}`,
    `manifest_hash = ${tomlString(ref.manifestHash, `${label}.manifestHash`)}`,
    `expected_exports = ${tomlStringArray(ref.expectedExports, `${label}.expectedExports`)}`,
    `bound_route_hash = ${tomlString(ref.boundRouteHash, `${label}.boundRouteHash`)}`,
    `bound_proof_hash = ${tomlString(ref.boundProofHash, `${label}.boundProofHash`)}`,
  ];
  return [`${key} = { ${entries.join(", ")} }`];
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
    ...tomlBrowserProverRefLine(
      "destination_browser_prover",
      route.destinationBrowserProver,
      "destination_browser_prover",
    ),
    ...tomlBrowserProverRefLine(
      "source_browser_prover",
      route.sourceBrowserProver,
      "source_browser_prover",
    ),
    ...tomlOptionalStringLine(
      "deployment_evidence_sha256",
      route.deploymentEvidenceSha256,
      "deployment_evidence_sha256",
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
    hashMode: OFFLINE_FULL_TOML_EVIDENCE_HASH_MODE,
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
          bytecodeKeccak256: artifact.bytecodeKeccak256,
          deployedBytecodeKeccak256: artifact.deployedBytecodeKeccak256,
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
  const privateKeyEnv = normalizePrivateKeyEnvName(
    options["private-key-env"] ?? DEFAULT_PRIVATE_KEY_ENV,
  );
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
    compiledContractCodeHashes: compiledContractCodeHashesFromArtifacts(
      artifacts,
      { profile },
    ),
    bscNetwork: profile.key,
  });
  const out = resolve(options.out ?? defaultDeploymentEvidenceOut(profile));
  assertBscCanonicalProductionOutputSafe(
    out,
    evidence,
    "BSC deployment evidence",
  );
  const deploymentTransactions = {
    verifier: verifier.txHash,
    sourceBridge: sourceBridge.txHash,
    token: token.txHash,
    bridge: bridge.txHash,
    setBridge: setBridgeReceipt.hash,
    lockBridge: lockBridgeReceipt.hash,
    transferSourceBridgeOwnership: transferSourceOwnerReceipt.hash,
  };
  await writeJsonNoSecrets(out, evidence);
  return {
    ok: true,
    wrote: out,
    deployerAddress: normalizeEvmAddress(await wallet.getAddress()),
    deploymentTransactions,
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
  const { artifacts } = await compileBscContracts();
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
    compiledContractCodeHashes: compiledContractCodeHashesFromArtifacts(
      artifacts,
      { profile },
    ),
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
      ? "Publish this route manifest on-chain with publish-route-manifest, rerun peer override audit/smoke-readiness/production gate, then capture live UI video proof."
      : "Attach production verifier/proof/native-prover/browser-prover/live evidence, rerun route-manifest with production-ready confirmations, then publish it on-chain with publish-route-manifest.",
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
  const out = resolve(outPath);
  assertBscCanonicalProductionOutputSafe(
    out,
    manifest,
    "BSC route config manifest",
  );
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
    const manifestReferencePath = generatedEvidenceReferencePath(
      manifestPath,
      offlineFullTomlEvidenceOut,
      "BSC offline full TOML evidence routeManifestPath",
    );
    const fullConfigReferencePath = generatedEvidenceReferencePath(
      out,
      offlineFullTomlEvidenceOut,
      "BSC offline full TOML evidence fullConfigPath",
    );
    offlineFullTomlEvidence = buildBscOfflineFullTomlEvidence({
      manifest,
      profile,
      manifestPath: manifestReferencePath,
      baseConfigPath,
      fullConfigPath: fullConfigReferencePath,
      renderedTomlSha256,
      offlineFullTomlSha256,
      hashInputSha256,
    });
    assertBscCanonicalProductionOutputSafe(
      offlineFullTomlEvidenceOut,
      offlineFullTomlEvidence,
      "BSC offline full TOML evidence",
    );
  }
  await writeTextNoSecrets(out, toml, 0o644);
  if (offlineFullTomlEvidenceOut && offlineFullTomlEvidence) {
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
      ? OFFLINE_FULL_TOML_EVIDENCE_HASH_MODE
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
      ? "Use this merged TAIRA node config only as legacy/offline evidence. Publish production BSC route material on-chain with publish-route-manifest, and keep peer configs free of local BSC SCCP route stanzas."
      : "Use this TOML only as legacy/offline evidence. Publish production BSC route material on-chain with publish-route-manifest, and keep peer configs free of local BSC SCCP route stanzas.",
  };
}

function sccpRouteBrowserProverRefIsi(ref) {
  if (!ref) return null;
  return {
    module_url: ref.moduleUrl,
    module_specifier: ref.moduleSpecifier ?? null,
    module_hash: ref.moduleHash,
    manifest_hash: ref.manifestHash,
    expected_exports: ref.expectedExports,
    bound_route_hash: ref.boundRouteHash,
    bound_proof_hash: ref.boundProofHash,
  };
}

export function buildUpsertSccpRouteManifestInstruction(manifest) {
  const route = normalizeRouteManifestForConfig(manifest);
  if (!Number.isInteger(route.version) || route.version < 0 || route.version > 255) {
    throw new Error("route manifest version must fit u8 for ISI publication.");
  }
  const counterpartyAccountCodec =
    readFirstValue(
      manifest,
      "counterpartyAccountCodec",
      "counterparty_account_codec",
    ) ?? (route.counterpartyDomain === 2 ? 2 : null);
  const counterpartyAccountCodecKey =
    readFirstValue(
      manifest,
      "counterpartyAccountCodecKey",
      "counterparty_account_codec_key",
    ) ?? (route.counterpartyDomain === 2 ? "evm_hex" : null);
  const payload = {
    version: route.version,
    route_id: route.routeId,
    asset_key: route.assetKey,
    tron_network: route.legacyTronNetwork,
    chain: route.chain,
    chain_id_hex: route.chainIdHex,
    explorer_url: route.explorerUrl,
    explorer_host: route.explorerHost,
    counterparty_domain: route.counterpartyDomain,
    counterparty_account_codec: counterpartyAccountCodec,
    counterparty_account_codec_key: counterpartyAccountCodecKey,
    verifier_target: route.verifierTarget,
    production_ready: route.productionReady,
    disabled_reason: route.disabledReason ?? null,
    network_id_hex: route.networkIdHex,
    taira_xor_token_address: route.tokenAddress,
    taira_xor_bridge_address: route.bridgeAddress,
    sccp_tron_source_bridge_address: route.sourceBridgeAddress,
    tron_verifier_address: route.verifierAddress,
    verifier_code_hash: route.verifierCodeHash,
    verifier_key_hash: route.verifierKeyHash,
    proof_artifact_hash: route.proofArtifactHash ?? null,
    proving_key_hash: route.provingKeyHash ?? null,
    native_evm_prover_bundle_hash: route.nativeEvmProverBundleHash ?? null,
    native_evm_prover_bundle: route.nativeEvmProverBundle ?? null,
    destination_browser_prover: sccpRouteBrowserProverRefIsi(
      route.destinationBrowserProver,
    ),
    source_browser_prover: sccpRouteBrowserProverRefIsi(
      route.sourceBrowserProver,
    ),
    deployment_evidence_sha256: route.deploymentEvidenceSha256 ?? null,
    destination_binding_key: route.destinationBindingKey,
    destination_binding_hash: route.destinationBindingHash,
    taira_burn_record_settlement_asset_definition_id:
      route.settlementAssetDefinitionId,
    taira_burn_record_contract_artifact_b64: route.contractArtifactB64,
    taira_burn_record_artifact_sha256: route.artifactSha256,
    taira_burn_record_code_hash: route.codeHash,
    taira_burn_record_vk_backend: route.vkBackend,
    taira_burn_record_vk_name: route.vkName,
    taira_burn_record_gas_limit: route.gasLimit,
    settlement_contract_address: route.settlementContractAddress ?? null,
    settlement_contract_alias: route.settlementContractAlias ?? null,
    post_deploy_full_toml_ready:
      route.postDeployLiveEvidence?.fullTomlReady ?? null,
    post_deploy_source_bridge_config_hash:
      route.postDeployLiveEvidence?.sourceBridgeConfigHash ?? null,
    post_deploy_source_event_transaction_id:
      route.postDeployLiveEvidence?.sourceEventTransactionId ?? null,
    post_deploy_source_event_explorer_url:
      route.postDeployLiveEvidence?.sourceEventExplorerUrl ?? null,
    post_deploy_route_canary_evidence_hash:
      route.postDeployLiveEvidence?.routeCanaryEvidenceHash ?? null,
    post_deploy_route_canary_transaction_id:
      route.postDeployLiveEvidence?.routeCanaryTransactionId ?? null,
    post_deploy_route_canary_explorer_url:
      route.postDeployLiveEvidence?.routeCanaryExplorerUrl ?? null,
    post_deploy_offline_full_toml_sha256:
      route.postDeployLiveEvidence?.offlineFullTomlSha256 ?? null,
  };
  const instruction = {
    UpsertSccpRouteManifest: {
      manifest: payload,
    },
  };
  return {
    instruction,
    routeKey: {
      routeId: route.routeId,
      assetKey: route.assetKey,
      counterpartyDomain: route.counterpartyDomain,
      chainIdHex: route.chainIdHex,
    },
    productionReady: route.productionReady,
    nativeEvmProverBundleHash: route.nativeEvmProverBundleHash ?? null,
    destinationBrowserProverManifestHash:
      route.destinationBrowserProver?.manifestHash ?? null,
    sourceBrowserProverManifestHash:
      route.sourceBrowserProver?.manifestHash ?? null,
  };
}

async function commandPublishRouteManifest(options) {
  const manifestPath = normalizeBscRouteManifestPath(
    options.manifest ?? DEFAULT_ROUTE_MANIFEST_OUT,
  );
  const manifest = await readJson(manifestPath, "BSC route manifest");
  const publication = buildUpsertSccpRouteManifestInstruction(manifest);
  const artifact = {
    schema: "iroha-sccp-route-manifest-isi/v1",
    routeId: publication.routeKey.routeId,
    assetKey: publication.routeKey.assetKey,
    routeKey: publication.routeKey,
    requiredPermission: "CanManageSccpRouteManifests",
    instruction: publication.instruction,
    manifestSha256: sha256HexBytes(
      Buffer.from(canonicalJson(manifest), "utf8"),
    ),
    productionReady: publication.productionReady,
    nativeEvmProverBundleHash: publication.nativeEvmProverBundleHash,
    destinationBrowserProverManifestHash:
      publication.destinationBrowserProverManifestHash,
    sourceBrowserProverManifestHash:
      publication.sourceBrowserProverManifestHash,
  };
  const out = resolve(options.out ?? DEFAULT_ROUTE_MANIFEST_ISI_OUT);
  await writeJsonNoSecrets(out, artifact);

  const submit = optionEnabled(options, "submit", false);
  if (!submit) {
    return {
      ok: true,
      wrote: out,
      submitted: false,
      routeId: publication.routeKey.routeId,
      assetKey: publication.routeKey.assetKey,
      requiredPermission: artifact.requiredPermission,
      nextStep:
        "Review the ISI artifact, then rerun with --submit true and a TAIRA authority holding CanManageSccpRouteManifests.",
    };
  }

  const authority = normalizeNonEmptyText(options.authority, "--authority");
  const chainId = normalizeTairaChainId(options["chain-id"]);
  const toriiUrl = normalizeTairaToriiUrl(options["torii-url"]);
  const waitForCommit = optionEnabled(options, "wait-for-commit", true);
  const commitTimeoutMs = normalizePositiveSafeInteger(
    options["commit-timeout-ms"],
    "--commit-timeout-ms",
    120_000,
  );
  const manifestSettlementAssetId = normalizeCanonicalAssetDefinitionId(
    manifest?.tairaXorBurnRecord?.settlementAssetDefinitionId,
    "manifest tairaXorBurnRecord.settlementAssetDefinitionId",
  );
  const gasAssetId =
    options["gas-asset-id"] === undefined || options["gas-asset-id"] === null
      ? manifestSettlementAssetId
      : normalizeCanonicalAssetDefinitionId(
          options["gas-asset-id"],
          "--gas-asset-id",
        );
  const gasLimit = normalizePositiveSafeInteger(
    options["gas-limit"],
    "--gas-limit",
    DEFAULT_TAIRA_ROUTE_MANIFEST_GAS_LIMIT,
  );
  const privateKeyEnv = normalizeTairaPrivateKeyEnvName(
    options["private-key-env"] ?? DEFAULT_TAIRA_ROUTE_MANIFEST_PRIVATE_KEY_ENV,
  );
  const privateKey = Buffer.from(
    normalizePrivateKey(process.env[privateKeyEnv], privateKeyEnv).slice(2),
    "hex",
  );
  const metadata = {
    routeId: publication.routeKey.routeId,
    assetKey: publication.routeKey.assetKey,
    action: "publish_sccp_route_manifest",
    gas_asset_id: gasAssetId,
    gas_limit: gasLimit,
  };
  const { buildTransaction } = await import(
    "../javascript/iroha_js/src/transaction.js"
  );
  const { ToriiClient } = await import(
    "../javascript/iroha_js/src/toriiClient.js"
  );
  const transaction = buildTransaction({
    chainId,
    authority,
    instructions: [publication.instruction],
    metadata,
    privateKey,
  });
  const client = new ToriiClient(toriiUrl);
  const hash = normalizeTransactionHash(
    transaction.hash.toString("hex"),
    "local transaction hash",
  );
  const submission = await submitSignedTransactionRawToTairaPipeline(
    client,
    toriiUrl,
    transaction.signedTransaction,
    hash,
    { waitForCommit, timeoutMs: commitTimeoutMs },
  );
  const submittedHash = normalizeTransactionHash(
    submission.hash,
    "submitted transaction hash",
  );
  const statusKind = transactionStatusKind(submission.status);
  const submissionEvidence = {
    submitted: true,
    toriiUrl,
    chainId,
    authority,
    hash,
    submittedHash,
    statusKind,
    status: submission.status ?? null,
    gasAssetId,
    gasLimit,
    waitForCommit,
    commitTimeoutMs,
  };
  await writeJsonNoSecrets(out, {
    ...artifact,
    submission: submissionEvidence,
  });
  if (waitForCommit && statusKind !== "Applied") {
    throw new Error(
      `TAIRA route manifest publication was not applied: ${statusKind ?? "unknown"}.`,
    );
  }
  return {
    ok: true,
    wrote: out,
    submitted: true,
    toriiUrl,
    chainId,
    authority,
    hash,
    submittedHash,
    statusKind,
    status: submission.status ?? null,
    gasAssetId,
    gasLimit,
    waitForCommit,
    commitTimeoutMs,
    routeId: publication.routeKey.routeId,
    assetKey: publication.routeKey.assetKey,
  };
}

function readBurnRecordVkRefFromManifest(manifest) {
  const burnRecord = isRecord(manifest?.tairaXorBurnRecord)
    ? manifest.tairaXorBurnRecord
    : isRecord(manifest?.taira_xor_burn_record)
      ? manifest.taira_xor_burn_record
      : {};
  const vkRef = isRecord(burnRecord.vkRef)
    ? burnRecord.vkRef
    : isRecord(burnRecord.vk_ref)
      ? burnRecord.vk_ref
      : {};
  return {
    backend: vkRef.backend ?? burnRecord.vkBackend ?? burnRecord.vk_backend ?? null,
    name: vkRef.name ?? burnRecord.vkName ?? burnRecord.vk_name ?? null,
    settlementAssetDefinitionId:
      burnRecord.settlementAssetDefinitionId ??
      burnRecord.settlement_asset_definition_id ??
      null,
  };
}

function normalizeBurnRecordVkTemplate(template, routeVkRef, options) {
  const backend = normalizeNonEmptyText(
    options.backend ?? routeVkRef.backend ?? template.backend,
    "burn-record VK backend",
  );
  const name = normalizeNonEmptyText(
    options.name ?? routeVkRef.name ?? template.name,
    "burn-record VK name",
  );
  if (name.includes(":")) {
    throw new Error("burn-record VK name must not contain ':'.");
  }
  const vkBytes = normalizeStrictBase64(
    template.vk_bytes ?? template.vkBytes,
    "burn-record VK bytes",
  );
  const vkLen = normalizePositiveSafeInteger(
    options["vk-len"] ?? template.vk_len ?? template.vkLen,
    "burn-record VK length",
  );
  if (vkBytes.bytes.length !== vkLen) {
    throw new Error("burn-record VK length must match the canonical VK bytes.");
  }
  const publicInputsSchemaHash = normalizeCanonicalHex32(
    template.public_inputs_schema_hex ??
      template.publicInputsSchemaHash ??
      template.public_inputs_schema_hash,
    "burn-record VK public input schema hash",
  );
  const commitment = normalizeCanonicalHex32(
    template.commitment_hex ??
      template.commitment ??
      template.vkCommitment ??
      template.vk_commitment,
    "burn-record VK commitment",
  );
  return {
    id: { backend, name },
    publicInputsSchemaHash,
    commitment,
    recordInput: {
      id: { backend, name },
      version: normalizePositiveSafeInteger(template.version, "burn-record VK version"),
      circuitId: normalizeNonEmptyText(
        options["circuit-id"] ?? template.circuit_id ?? template.circuitId,
        "burn-record VK circuit id",
      ),
      backend,
      curve: normalizeNonEmptyText(template.curve, "burn-record VK curve"),
      publicInputsSchemaHash: Array.from(hex32Bytes(publicInputsSchemaHash, "burn-record VK public input schema hash")),
      commitment: Array.from(hex32Bytes(commitment, "burn-record VK commitment")),
      vkLen,
      maxProofBytes: normalizePositiveSafeInteger(
        options["max-proof-bytes"] ??
          template.max_proof_bytes ??
          template.maxProofBytes,
        "burn-record VK max proof bytes",
      ),
      gasScheduleId: normalizeNonEmptyText(
        options["gas-schedule-id"] ??
          template.gas_schedule_id ??
          template.gasScheduleId,
        "burn-record VK gas schedule id",
      ),
      vkBytes: vkBytes.text,
      status: normalizeNonEmptyText(template.status ?? "Active", "burn-record VK status"),
    },
    vkBytes,
  };
}

function burnRecordVkRegistryProblems(registryEntry, vk) {
  const id = registryEntry?.id;
  const record = registryEntry?.record;
  const problems = [];
  if (!isRecord(id) || id.backend !== vk.id.backend || id.name !== vk.id.name) {
    problems.push("registry id does not match the requested VK ref");
  }
  if (!isRecord(record)) {
    problems.push("registry entry is missing a record");
    return problems;
  }
  if (record.status !== vk.recordInput.status) {
    problems.push(`registry status is ${record.status ?? "missing"}`);
  }
  if (record.circuit_id !== vk.recordInput.circuitId) {
    problems.push("registry circuit_id does not match");
  }
  if (record.commitment !== vk.commitment.slice(2)) {
    problems.push("registry commitment does not match");
  }
  if (record.public_inputs_schema_hash !== vk.publicInputsSchemaHash.slice(2)) {
    problems.push("registry public_inputs_schema_hash does not match");
  }
  if (record.vk_len !== vk.vkBytes.bytes.length) {
    problems.push("registry vk_len does not match");
  }
  if (record.key?.bytes_b64 !== vk.vkBytes.text) {
    problems.push("registry inline VK bytes do not match");
  }
  return problems;
}

async function fetchBurnRecordVkRegistryEntry(toriiUrl, vk) {
  const url = new URL("/v1/zk/vk", toriiUrl);
  url.searchParams.set("backend", vk.id.backend);
  url.searchParams.set("name_contains", vk.id.name);
  url.searchParams.set("limit", "10");
  const response = await fetch(url, {
    method: "GET",
    headers: { Accept: "application/json" },
  });
  if (!response.ok) {
    const preview = await responseBodyPreview(response);
    throw new Error(
      `Torii responded with HTTP ${response.status} while reading VK registry${
        preview ? `: ${preview}` : ""
      }`,
    );
  }
  const entries = await response.json();
  if (!Array.isArray(entries)) {
    throw new Error("TAIRA VK registry response must be a JSON array.");
  }
  return (
    entries.find(
      (entry) =>
        entry?.id?.backend === vk.id.backend && entry?.id?.name === vk.id.name,
    ) ?? null
  );
}

async function waitForBurnRecordVkRegistryEntry(toriiUrl, vk, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastEntry = null;
  let lastProblems = [];
  while (Date.now() <= deadline) {
    lastEntry = await fetchBurnRecordVkRegistryEntry(toriiUrl, vk);
    if (lastEntry) {
      lastProblems = burnRecordVkRegistryProblems(lastEntry, vk);
      if (lastProblems.length === 0) {
        return { entry: lastEntry, problems: [] };
      }
    }
    await delayMs(500);
  }
  return { entry: lastEntry, problems: lastProblems };
}

async function commandPublishBurnRecordVk(options) {
  const manifestPath = normalizeBscRouteManifestPath(
    options["route-manifest"] ?? options.manifest ?? DEFAULT_ROUTE_MANIFEST_OUT,
  );
  const manifest = await readJson(manifestPath, "BSC route manifest");
  const templatePath = resolve(
    options["vk-template"] ?? DEFAULT_TAIRA_BURN_RECORD_VK_TEMPLATE,
  );
  const template = await readJson(templatePath, "TAIRA burn-record VK template");
  const routeVkRef = readBurnRecordVkRefFromManifest(manifest);
  const vk = normalizeBurnRecordVkTemplate(template, routeVkRef, options);
  const { buildRegisterPrivacyVerifierKeyInstruction } = await import(
    "../javascript/iroha_js/src/instructionBuilders.js"
  );
  const instruction = buildRegisterPrivacyVerifierKeyInstruction(vk.recordInput);
  const routeId = normalizeNonEmptyText(manifest.routeId ?? manifest.route_id, "route id");
  const assetKey = normalizeNonEmptyText(manifest.assetKey ?? manifest.asset_key, "asset key");
  const artifact = {
    schema: "iroha-sccp-bsc-burn-record-vk-register-isi/v1",
    routeId,
    assetKey,
    requiredPermission: "CanManageVerifyingKeys",
    id: vk.id,
    vkRef: vk.id,
    instruction,
    routeManifestSha256: sha256HexBytes(
      Buffer.from(canonicalJson(manifest), "utf8"),
    ),
    vkTemplateSha256: sha256HexBytes(
      Buffer.from(canonicalJson(template), "utf8"),
    ),
    vkBytesSha256: sha256HexBytes(vk.vkBytes.bytes),
    vkLen: vk.vkBytes.bytes.length,
    circuitId: vk.recordInput.circuitId,
    publicInputsSchemaHash: vk.publicInputsSchemaHash,
    commitment: vk.commitment,
    gasScheduleId: vk.recordInput.gasScheduleId,
    status: vk.recordInput.status,
  };
  const out = resolve(options.out ?? DEFAULT_TAIRA_BSC_BURN_RECORD_VK_ISI_OUT);
  await writeJsonNoSecrets(out, artifact);

  const submit = optionEnabled(options, "submit", false);
  if (!submit) {
    return {
      ok: true,
      wrote: out,
      submitted: false,
      routeId,
      assetKey,
      vkRef: vk.id,
      requiredPermission: artifact.requiredPermission,
      nextStep:
        "Review the VK registration artifact, then rerun with --submit true and a TAIRA authority holding CanManageVerifyingKeys.",
    };
  }

  const authority = normalizeNonEmptyText(options.authority, "--authority");
  const chainId = normalizeTairaChainId(options["chain-id"]);
  const toriiUrl = normalizeTairaToriiUrl(options["torii-url"]);
  const waitForCommit = optionEnabled(options, "wait-for-commit", true);
  const commitTimeoutMs = normalizePositiveSafeInteger(
    options["commit-timeout-ms"],
    "--commit-timeout-ms",
    120_000,
  );
  const gasAssetId =
    options["gas-asset-id"] === undefined || options["gas-asset-id"] === null
      ? normalizeCanonicalAssetDefinitionId(
          routeVkRef.settlementAssetDefinitionId,
          "route manifest tairaXorBurnRecord.settlementAssetDefinitionId",
        )
      : normalizeCanonicalAssetDefinitionId(
          options["gas-asset-id"],
          "--gas-asset-id",
        );
  const gasLimit = normalizePositiveSafeInteger(
    options["gas-limit"],
    "--gas-limit",
    DEFAULT_TAIRA_ROUTE_MANIFEST_GAS_LIMIT,
  );

  const existingEntry = await fetchBurnRecordVkRegistryEntry(toriiUrl, vk);
  if (existingEntry) {
    const existingProblems = burnRecordVkRegistryProblems(existingEntry, vk);
    if (existingProblems.length > 0) {
      throw new Error(
        `Existing TAIRA burn-record VK registry entry does not match: ${existingProblems.join("; ")}.`,
      );
    }
    const submissionEvidence = {
      submitted: false,
      alreadyRegistered: true,
      toriiUrl,
      chainId,
      authority,
      registryReadback: existingEntry,
      gasAssetId,
      gasLimit,
      waitForCommit,
      commitTimeoutMs,
    };
    await writeJsonNoSecrets(out, {
      ...artifact,
      submission: submissionEvidence,
    });
    return {
      ok: true,
      wrote: out,
      submitted: false,
      alreadyRegistered: true,
      toriiUrl,
      chainId,
      authority,
      gasAssetId,
      gasLimit,
      waitForCommit,
      commitTimeoutMs,
      routeId,
      assetKey,
      vkRef: vk.id,
    };
  }

  const privateKeyEnv = normalizeTairaPrivateKeyEnvName(
    options["private-key-env"] ?? DEFAULT_TAIRA_ROUTE_MANIFEST_PRIVATE_KEY_ENV,
  );
  const privateKey = Buffer.from(
    normalizePrivateKey(process.env[privateKeyEnv], privateKeyEnv).slice(2),
    "hex",
  );
  const metadata = {
    routeId,
    assetKey,
    action: "publish_burn_record_vk",
    gas_asset_id: gasAssetId,
    gas_limit: gasLimit,
  };
  const { buildTransaction } = await import(
    "../javascript/iroha_js/src/transaction.js"
  );
  const { ToriiClient } = await import(
    "../javascript/iroha_js/src/toriiClient.js"
  );
  const transaction = buildTransaction({
    chainId,
    authority,
    instructions: [instruction],
    metadata,
    privateKey,
  });
  const client = new ToriiClient(toriiUrl);
  const hash = normalizeTransactionHash(
    transaction.hash.toString("hex"),
    "local transaction hash",
  );
  const submission = await submitSignedTransactionRawToTairaPipeline(
    client,
    toriiUrl,
    transaction.signedTransaction,
    hash,
    { waitForCommit, timeoutMs: commitTimeoutMs },
  );
  const submittedHash = normalizeTransactionHash(
    submission.hash,
    "submitted transaction hash",
  );
  const statusKind = transactionStatusKind(submission.status);
  const registryResult =
    waitForCommit && statusKind === "Applied"
      ? await waitForBurnRecordVkRegistryEntry(toriiUrl, vk, commitTimeoutMs)
      : { entry: null, problems: [] };
  const submissionEvidence = {
    submitted: true,
    alreadyRegistered: false,
    toriiUrl,
    chainId,
    authority,
    hash,
    submittedHash,
    statusKind,
    status: submission.status ?? null,
    registryReadback: registryResult.entry,
    gasAssetId,
    gasLimit,
    waitForCommit,
    commitTimeoutMs,
  };
  await writeJsonNoSecrets(out, {
    ...artifact,
    submission: submissionEvidence,
  });
  if (waitForCommit && statusKind !== "Applied") {
    throw new Error(
      `TAIRA burn-record VK registration was not applied: ${statusKind ?? "unknown"}.`,
    );
  }
  if (
    waitForCommit &&
    (!registryResult.entry || registryResult.problems.length > 0)
  ) {
    throw new Error(
      `TAIRA burn-record VK registry readback failed${
        registryResult.problems.length > 0
          ? `: ${registryResult.problems.join("; ")}`
          : "."
      }`,
    );
  }
  return {
    ok: true,
    wrote: out,
    submitted: true,
    alreadyRegistered: false,
    toriiUrl,
    chainId,
    authority,
    hash,
    submittedHash,
    statusKind,
    status: submission.status ?? null,
    gasAssetId,
    gasLimit,
    waitForCommit,
    commitTimeoutMs,
    routeId,
    assetKey,
    vkRef: vk.id,
  };
}

async function commandNativeProverBundle(options) {
  const result = await buildBscNativeEvmProverBundleFromArtifacts(options);
  const profile = bscProfileFromNativeEvmProverBundle(result.bundle);
  const out = resolve(options.out ?? defaultNativeEvmProverBundleOut(profile));
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
  }
  assertBscCanonicalProductionOutputSafe(
    out,
    result.bundle,
    "BSC native EVM prover bundle",
  );
  await writeJsonNoSecrets(out, result.bundle);
  if (attachedRouteManifestOut) {
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
    groth16MaterialManifest:
      result.artifacts.groth16MaterialManifest?.path ?? null,
    verifiedSdks: result.verifiedSdks,
    nextStep:
      "Attach this nativeEvmProverBundle plus browser prover references to the production BSC route manifest, publish the manifest on-chain via ISI, then rerun the BSC SCCP production gates.",
  };
}

async function commandSourceParityAttestation(options) {
  const profile = bscNetworkProfileFromOptions(options);
  const attestation = await buildBscNativeEvmSourceParityAttestation(options);
  const out = resolve(
    options.out ?? defaultNativeEvmSourceParityAttestationOut(profile),
  );
  await writeJsonNoSecrets(out, attestation);
  return {
    ok: true,
    wrote: out,
    schema: attestation.schema,
    routeId: attestation.routeId,
    assetKey: attestation.assetKey,
    bscNetwork: attestation.bscNetwork,
    sourceTreeHash: attestation.sourceTreeHash,
    sdkCount: Object.keys(attestation.sdks).length,
    nextStep:
      "Use this source-parity attestation as the native implementation audit artifact when building the BSC native EVM prover bundle.",
  };
}

async function commandGroth16Material(argv = []) {
  const { main: groth16MaterialMain } = await import(
    "./sccp_bsc_groth16_material.mjs"
  );
  return groth16MaterialMain(argv);
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
      "Fill every public requirement with production artifacts/evidence, then run deploy, native-prover-bundle, publish-route-manifest, peer override audit, smoke readiness, production gate, and live video proof.",
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
    codeHashes: {
      token: `0x${"33".repeat(32)}`,
      bridge: `0x${"44".repeat(32)}`,
      sourceBridge: `0x${"55".repeat(32)}`,
      verifier: verifierCodeHash,
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
  if (!command || isHelpToken(command)) {
    const requestedCommand = command === "help" ? rest[0] : undefined;
    return { help: requestedCommand ? commandUsage(requestedCommand) : usage() };
  }
  if (rest.some(isHelpToken)) {
    return { help: commandUsage(command) };
  }
  if (command === "groth16-material") {
    return commandGroth16Material(rest);
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
    case "source-parity-attestation":
      return commandSourceParityAttestation(options);
    case "native-prover-bundle":
      return commandNativeProverBundle(options);
    case "publish-route-manifest":
      return commandPublishRouteManifest(options);
    case "publish-burn-record-vk":
      return commandPublishBurnRecordVk(options);
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
