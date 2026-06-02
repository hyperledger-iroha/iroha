#!/usr/bin/env node
// Purpose: compile, sign, deploy, and evidence-check the TRON mainnet
// contracts for the TAIRA XOR SCCP bridge without ever using end-user wallet
// keys. Safe default: no transaction is broadcast unless the command includes
// `--confirm-mainnet taira_tron_xor`.
//
// Prerequisites:
// - Node.js 18+ for built-in fetch.
// - `solc` and `ethers` on NODE_PATH for compile/deploy commands.
// - Optional `TRON_PRO_API_KEY` or `TRON_GRID_API_KEY` for TronGrid calls.
import { createRequire } from "node:module";
import { lstat, mkdir, readFile, rename, stat, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { secp256k1 } from "../javascript/iroha_js/node_modules/@noble/curves/secp256k1.js";
import { sha256 } from "../javascript/iroha_js/node_modules/@noble/hashes/sha256.js";
import { keccak_256 } from "../javascript/iroha_js/node_modules/@noble/hashes/sha3.js";
import { compileKotodamaProgram } from "../javascript/iroha_js/src/index.js";

const requireFromScript = createRequire(import.meta.url);
const requireFromCwd = createRequire(`${resolve("noop.js")}`);
const SCRIPT_PATH = fileURLToPath(import.meta.url);
const REPO_ROOT = resolve(dirname(SCRIPT_PATH), "..");

const BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const BASE58_INDEX = new Map(
  Array.from(BASE58_ALPHABET, (character, index) => [character, BigInt(index)]),
);
const DEFAULT_SECRET_OUT = "artifacts/sccp-tron/taira-xor-deployer.secret.json";
const DEFAULT_EVIDENCE_OUT = "artifacts/sccp-tron/taira-xor-deployment.evidence.json";
const DEFAULT_ROUTE_MANIFEST_OUT = "artifacts/sccp-tron/taira-xor-route.manifest.json";
const DEFAULT_ARTIFACTS_OUT = "artifacts/sccp-tron/contracts";
const DEFAULT_TAIRA_CONTRACT_OUT = "artifacts/sccp-taira/taira-xor-burn-record.contract.json";
const DEFAULT_DEPLOYMENT_OUT = "artifacts/sccp-tron/taira-xor-deployment.plan.json";
const DEFAULT_SIGNED_TRANSACTION_OUT = "artifacts/sccp-tron/signed-transaction.json";
const DEFAULT_BROADCAST_OUT = "artifacts/sccp-tron/broadcast-result.json";
const DEFAULT_TRON_ENDPOINT = "https://api.trongrid.io";
const DEFAULT_DEPLOY_FEE_LIMIT_SUN = 15_000_000_000;
const DEFAULT_TRIGGER_FEE_LIMIT_SUN = 1_000_000_000;
const DEFAULT_ORIGIN_ENERGY_LIMIT = 10_000_000;
const DEFAULT_POLL_ATTEMPTS = 40;
const DEFAULT_POLL_MS = 3_000;
const ROUTE_ID = "taira_tron_xor";
const ASSET_KEY = "xor";
const SCCP_DOMAIN_SORA = 0;
const SCCP_DOMAIN_TRON = 5;
const TRON_MAINNET_NETWORK_ID_HEX =
  `0x${"0".repeat(56)}2b6653dc`;
const TRON_DESTINATION_BINDING_LABEL = "iroha:sccp:tron-destination-binding:v1";
const TRON_GROTH16_BACKEND = "tron-groth16-bn254-v1";
const SCCP_PROOF_FAMILY_STARK_FRI = "stark-fri-v1";
const CONFIRMATION_TEXT = "taira_tron_xor";
const DEPLOYER_SCHEMA = "iroha-sccp-tron-taira-xor-deployer/v1";
const EVIDENCE_SCHEMA = "iroha-sccp-tron-taira-xor-deployment-evidence/v1";
const ROUTE_MANIFEST_SCHEMA = "iroha-sccp-taira-xor-route-manifest-draft/v1";
const TAIRA_BURN_RECORD_CONTRACT_SCHEMA = "iroha-sccp-taira-xor-burn-record-contract/v1";
const UNSIGNED_TRANSACTION_SCHEMA = "iroha-sccp-tron-unsigned-transaction/v1";
const SIGNED_TRANSACTION_SCHEMA = "iroha-sccp-tron-signed-transaction/v1";
const SIGNED_TRANSACTION_PURPOSE = "taira-xor-sccp-deployment";
const BROADCAST_RESULT_SCHEMA = "iroha-sccp-tron-broadcast-result/v1";
const DEPLOYMENT_PLAN_SCHEMA = "iroha-sccp-tron-taira-xor-deployment-plan/v1";
const TAIRA_BURN_RECORD_ARTIFACT_MIN_BYTES = 32;
const TAIRA_BURN_RECORD_ARTIFACT_MAX_BYTES = 8 * 1024 * 1024;
const SECP256K1_ORDER =
  0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141n;
const SECP256K1_HALF_ORDER = SECP256K1_ORDER >> 1n;
const POST_DEPLOY_CONFIGURATION_CHECKS = Object.freeze([
  "TairaXOR.bridge() equals taira_xor_bridge_address",
  "TairaXOR.bridgeLocked() is true",
  "SccpTronSourceBridge.owner() equals taira_xor_bridge_address",
  "TairaXorSccpBridge.destinationBindingHash() equals verifier destinationBindingHash()",
]);
const REQUIRED_POST_DEPLOY_CHECKS = Object.freeze([
  ...POST_DEPLOY_CONFIGURATION_CHECKS,
  "Run scripts/sccp_tron_source_bridge_evidence.py for source bridge config evidence",
  "Run scripts/sccp_tron_live_evidence.py for live verifier/source/canary evidence",
]);
const DEPLOYMENT_ARTIFACT_SECRET_KEY_PATTERN =
  /(?:private[_-]?key|mnemonic|recovery[_-]?phrase|seed[_-]?phrase|secret)/iu;
const PRIVATE_KEY_PEM_PATTERN =
  /-----BEGIN(?: [A-Z0-9]+)* PRIVATE KEY-----[\s\S]*?-----END(?: [A-Z0-9]+)* PRIVATE KEY-----/iu;
const RECOVERY_PHRASE_WORD_COUNTS = new Set([12, 15, 18, 21, 24]);

const textEncoder = new TextEncoder();

const CONTRACT_SOURCES = {
  "contracts/evm/sccp/ISccpMessageVerifier.sol": repoPath(
    "contracts",
    "evm",
    "sccp",
    "ISccpMessageVerifier.sol",
  ),
  "contracts/evm/sccp/Ownable.sol": repoPath("contracts", "evm", "sccp", "Ownable.sol"),
  "contracts/evm/sccp/SccpGroth16Bn254MessageVerifier.sol": repoPath(
    "contracts",
    "evm",
    "sccp",
    "SccpGroth16Bn254MessageVerifier.sol",
  ),
  "contracts/tron/sccp/SccpTronGroth16Bn254MessageVerifier.sol": repoPath(
    "contracts",
    "tron",
    "sccp",
    "SccpTronGroth16Bn254MessageVerifier.sol",
  ),
  "contracts/tron/sccp/SccpTronSourceBridge.sol": repoPath(
    "contracts",
    "tron",
    "sccp",
    "SccpTronSourceBridge.sol",
  ),
  "contracts/tron/sccp/TairaXOR.sol": repoPath("contracts", "tron", "sccp", "TairaXOR.sol"),
  "contracts/tron/sccp/TairaXorSccpBridge.sol": repoPath(
    "contracts",
    "tron",
    "sccp",
    "TairaXorSccpBridge.sol",
  ),
};

const CONTRACT_DEFINITIONS = [
  {
    key: "verifier",
    file: "contracts/tron/sccp/SccpTronGroth16Bn254MessageVerifier.sol",
    contract: "SccpTronGroth16Bn254MessageVerifier",
    deployName: "SccpTronGroth16Bn254MessageVerifier",
  },
  {
    key: "source_bridge",
    file: "contracts/tron/sccp/SccpTronSourceBridge.sol",
    contract: "SccpTronSourceBridge",
    deployName: "SccpTronSourceBridge",
  },
  {
    key: "token",
    file: "contracts/tron/sccp/TairaXOR.sol",
    contract: "TairaXOR",
    deployName: "TairaXOR",
  },
  {
    key: "bridge",
    file: "contracts/tron/sccp/TairaXorSccpBridge.sol",
    contract: "TairaXorSccpBridge",
    deployName: "TairaXorSccpBridge",
  },
];
const TAIRA_BURN_RECORD_CONTRACT_SOURCE = repoPath(
  "contracts",
  "taira",
  "sccp",
  "TairaXorSccpBurnRecord.ko",
);

function repoPath(...segments) {
  return resolve(REPO_ROOT, ...segments);
}

function usage() {
  return `Usage:
  node scripts/sccp_tron_taira_xor_deploy.mjs generate-deployer [--out ${DEFAULT_SECRET_OUT}] [--force true]
  node scripts/sccp_tron_taira_xor_deploy.mjs doctor [--secret ${DEFAULT_SECRET_OUT}] [--verifier <verifier-key.json>] [--endpoint ${DEFAULT_TRON_ENDPOINT}] [--check-account true] [--require-secret true] [--require-verifier true] [--require-optional-packages true]
  node scripts/sccp_tron_taira_xor_deploy.mjs estimate-budget [--secret ${DEFAULT_SECRET_OUT}] [--fee-limit ${DEFAULT_DEPLOY_FEE_LIMIT_SUN}] [--trigger-fee-limit ${DEFAULT_TRIGGER_FEE_LIMIT_SUN}]
  node scripts/sccp_tron_taira_xor_deploy.mjs account-status [--secret ${DEFAULT_SECRET_OUT}] [--endpoint ${DEFAULT_TRON_ENDPOINT}]
  node scripts/sccp_tron_taira_xor_deploy.mjs compile [--out ${DEFAULT_ARTIFACTS_OUT}]
  node scripts/sccp_tron_taira_xor_deploy.mjs compile-taira-contract [--out ${DEFAULT_TAIRA_CONTRACT_OUT}]
  node scripts/sccp_tron_taira_xor_deploy.mjs deploy --verifier <verifier-key.json> [--secret ${DEFAULT_SECRET_OUT}] [--endpoint ${DEFAULT_TRON_ENDPOINT}] [--out ${DEFAULT_DEPLOYMENT_OUT}] [--broadcast true --confirm-mainnet ${CONFIRMATION_TEXT}]
  node scripts/sccp_tron_taira_xor_deploy.mjs sign-transaction --secret ${DEFAULT_SECRET_OUT} --transaction <unsigned-artifact.json> [--out ${DEFAULT_SIGNED_TRANSACTION_OUT}]
  node scripts/sccp_tron_taira_xor_deploy.mjs sign-transaction --secret ${DEFAULT_SECRET_OUT} --transaction ${DEFAULT_DEPLOYMENT_OUT} --step <step-key> [--out ${DEFAULT_SIGNED_TRANSACTION_OUT}]
  node scripts/sccp_tron_taira_xor_deploy.mjs broadcast --transaction <signed.json> [--endpoint ${DEFAULT_TRON_ENDPOINT}] --confirm-mainnet ${CONFIRMATION_TEXT} [--out ${DEFAULT_BROADCAST_OUT}]
  node scripts/sccp_tron_taira_xor_deploy.mjs evidence --token <addr> --bridge <addr> --source-bridge <addr> --verifier <addr> [--out ${DEFAULT_EVIDENCE_OUT}]
  node scripts/sccp_tron_taira_xor_deploy.mjs route-manifest --settlement-asset-definition-id <asset-id> --verifier-code-hash <0x...> (--verifier-key-hash <0x...> | --verifier <verifier-key.json>) --vk-backend <backend> --vk-name <name> [--evidence ${DEFAULT_EVIDENCE_OUT}] [--taira-contract ${DEFAULT_TAIRA_CONTRACT_OUT}] [--live-evidence <sccp-tron-live-evidence.json>] [--expected-destination-binding-hash <0x...>] [--expected-destination-binding-key <key>] [--gas-limit 2000000] [--production-ready true --live-readback-checked true --confirm-mainnet ${CONFIRMATION_TEXT}] [--out ${DEFAULT_ROUTE_MANIFEST_OUT}]
  node scripts/sccp_tron_taira_xor_deploy.mjs self-test

Required optional packages for compile/deploy: solc and ethers. The contract
smoke-test NODE_PATH can be reused, e.g.:
  NODE_PATH=/tmp/iroha-sccp-smoke-node/node_modules node scripts/sccp_tron_taira_xor_deploy.mjs compile

This helper writes only local deployment/deployer artifacts under ignored artifacts/.
End-user TRON wallets must still connect through WalletConnect; never use this deployer for user bridging.`;
}

function bytesToHex(bytes, prefix = true) {
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  return prefix ? `0x${hex}` : hex;
}

function strip0x(value) {
  return String(value).replace(/^0x/iu, "");
}

function assertHexText(value, label, byteLength = null) {
  if (typeof value !== "string") {
    throw new Error(`${label} must be hex text`);
  }
  const hex = strip0x(value);
  if (hex.length === 0 || hex.length % 2 !== 0 || /[^0-9a-f]/iu.test(hex)) {
    throw new Error(`${label} must be even-length hex`);
  }
  if (byteLength !== null && hex.length !== byteLength * 2) {
    throw new Error(`${label} must be ${byteLength} bytes`);
  }
  return hex.toLowerCase();
}

function hexToBytes(value, label, byteLength = null) {
  const hex = assertHexText(value, label, byteLength);
  return Uint8Array.from(hex.match(/.{1,2}/gu).map((byte) => Number.parseInt(byte, 16)));
}

function bytesToBigInt(bytes) {
  return BigInt(`0x${bytesToHex(bytes, false) || "0"}`);
}

function normalizeBytes32(value, label) {
  const bytes = hexToBytes(value, label, 32);
  if (bytes.every((byte) => byte === 0)) {
    throw new Error(`${label} must be non-zero bytes32`);
  }
  return bytesToHex(bytes);
}

function normalizeNonEmptyText(value, label) {
  if (typeof value !== "string" || value.length === 0 || value.trim() !== value) {
    throw new Error(`${label} must be non-empty text without surrounding whitespace`);
  }
  return value;
}

function isRecoveryPhraseShapedText(value) {
  if (typeof value !== "string") return false;
  const normalized = value.trim().replace(/\s+/gu, " ").toLowerCase();
  if (!normalized) return false;
  const words = normalized.split(" ");
  return (
    RECOVERY_PHRASE_WORD_COUNTS.has(words.length) &&
    words.every((word) => /^[a-z]{2,12}$/u.test(word))
  );
}

function isSecretLikeArtifactText(value) {
  return (
    typeof value === "string" &&
    (PRIVATE_KEY_PEM_PATTERN.test(value) || isRecoveryPhraseShapedText(value))
  );
}

function assertNoSecretLikeDeploymentArtifactFields(
  value,
  path = "deployment artifact",
  seen = new WeakSet(),
) {
  if (isSecretLikeArtifactText(value)) {
    throw new Error(
      `${path} must not contain recovery phrases or private key material`,
    );
  }
  if (Array.isArray(value)) {
    if (seen.has(value)) return;
    seen.add(value);
    value.forEach((entry, index) => {
      assertNoSecretLikeDeploymentArtifactFields(entry, `${path}[${index}]`, seen);
    });
    return;
  }
  if (!value || typeof value !== "object") return;
  if (seen.has(value)) return;
  seen.add(value);
  for (const [key, child] of Object.entries(value)) {
    if (DEPLOYMENT_ARTIFACT_SECRET_KEY_PATTERN.test(key)) {
      throw new Error(`${path}.${key} must not be present in deployment artifacts`);
    }
    assertNoSecretLikeDeploymentArtifactFields(child, `${path}.${key}`, seen);
  }
}

function normalizeVerifierKeyRefText(value, label) {
  const normalized = normalizeNonEmptyText(value, label);
  if (!/^[A-Za-z0-9][A-Za-z0-9._:/-]{0,127}$/u.test(normalized)) {
    throw new Error(`${label} contains unsupported characters`);
  }
  return normalized;
}

function normalizeCanonicalAssetDefinitionId(value, label) {
  const normalized = normalizeNonEmptyText(value, label);
  if (normalized.includes("#") || normalized.toLowerCase() === "xor") {
    throw new Error(`${label} must be a canonical Base58 asset definition ID, not an alias`);
  }
  if (!/^[1-9A-HJ-NP-Za-km-z]{16,80}$/u.test(normalized)) {
    throw new Error(`${label} must be a canonical Base58 asset definition ID`);
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
    throw new Error(`${label} must be strict base64`);
  }
  const decoded = Buffer.from(normalized, "base64");
  if (decoded.length === 0 || decoded.toString("base64") !== normalized) {
    throw new Error(`${label} must be canonical strict base64`);
  }
  return { text: normalized, bytes: decoded };
}

function normalizePositiveSafeInteger(value, label, fallback) {
  const source = value === undefined || value === null || value === "" ? fallback : value;
  const parsed = typeof source === "number" ? source : Number(source);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive safe integer`);
  }
  return parsed;
}

function normalizeUint32(value, label) {
  if (typeof value === "string" && /^0x[0-9a-f]+$/iu.test(value)) {
    const parsed = Number.parseInt(value.slice(2), 16);
    if (Number.isInteger(parsed) && parsed >= 0 && parsed <= 0xffffffff) return parsed;
  }
  const parsed = typeof value === "bigint" ? value : BigInt(String(value));
  if (parsed < 0n || parsed > 0xffffffffn) {
    throw new Error(`${label} must fit uint32`);
  }
  return Number(parsed);
}

function normalizeSun(value, label, fallback) {
  const source = value === undefined || value === null || value === "" ? fallback : value;
  const parsed = Number(source);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive safe integer SUN amount`);
  }
  return parsed;
}

function normalizePercent(value, label, fallback) {
  const source = value === undefined || value === null || value === "" ? fallback : value;
  const parsed = Number(source);
  if (!Number.isSafeInteger(parsed) || parsed < 0 || parsed > 100) {
    throw new Error(`${label} must be an integer percent between 0 and 100`);
  }
  return parsed;
}

function normalizeBalanceSun(value, label) {
  if (value === undefined || value === null || value === "") return 0n;
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < 0) {
      throw new Error(`${label} must be a non-negative safe integer SUN amount`);
    }
    return BigInt(value);
  }
  if (typeof value === "string" && /^[0-9]+$/u.test(value)) {
    return BigInt(value);
  }
  throw new Error(`${label} must be a non-negative SUN amount`);
}

function sunToTrxText(value) {
  const sun = BigInt(value);
  const whole = sun / 1_000_000n;
  const fractional = (sun % 1_000_000n).toString().padStart(6, "0").replace(/0+$/u, "");
  return fractional ? `${whole}.${fractional}` : whole.toString();
}

function parseIpv4Octets(hostname) {
  if (!/^\d{1,3}(?:\.\d{1,3}){3}$/u.test(hostname)) {
    return null;
  }
  const octets = hostname.split(".").map((part) => Number(part));
  return octets.every((octet) => Number.isInteger(octet) && octet >= 0 && octet <= 255)
    ? octets
    : null;
}

function isPrivateOrReservedIpv4Octets(octets) {
  const [first, second] = octets;
  return (
    first === 0 ||
    first === 10 ||
    first === 127 ||
    (first === 100 && second >= 64 && second <= 127) ||
    (first === 169 && second === 254) ||
    (first === 172 && second >= 16 && second <= 31) ||
    (first === 192 && second === 0) ||
    (first === 192 && second === 168) ||
    (first === 198 && (second === 18 || second === 19)) ||
    first >= 224
  );
}

function isPrivateOrReservedIpv4(hostname) {
  const octets = parseIpv4Octets(hostname);
  if (!octets) return false;
  return isPrivateOrReservedIpv4Octets(octets);
}

function parseIpv6Hextets(hostname) {
  if (!hostname.includes(":")) {
    return null;
  }
  const parts = hostname.split("::");
  if (parts.length > 2) {
    return null;
  }
  const parseSide = (side) =>
    side
      ? side.split(":").map((part) => {
          if (!/^[0-9a-f]{1,4}$/iu.test(part)) {
            return Number.NaN;
          }
          return Number.parseInt(part, 16);
        })
      : [];
  const left = parseSide(parts[0]);
  const right = parseSide(parts[1] ?? "");
  if (
    [...left, ...right].some((hextet) => !Number.isInteger(hextet)) ||
    (parts.length === 1 && left.length !== 8) ||
    left.length + right.length > 8
  ) {
    return null;
  }
  const zeroFill = parts.length === 2 ? Array(8 - left.length - right.length).fill(0) : [];
  return [...left, ...zeroFill, ...right];
}

function hextetsToIpv4Octets(high, low) {
  return [(high >> 8) & 0xff, high & 0xff, (low >> 8) & 0xff, low & 0xff];
}

function hasPrivateOrReservedEmbeddedIpv4(hextets) {
  if (hextets.length !== 8) {
    return false;
  }
  const lastIpv4 = hextetsToIpv4Octets(hextets[6], hextets[7]);
  const leadingCompatibleZeros = hextets.slice(0, 6).every((part) => part === 0);
  const leadingMappedZeros =
    hextets.slice(0, 5).every((part) => part === 0) && hextets[5] === 0xffff;
  const nat64WellKnownPrefix =
    hextets[0] === 0x64 &&
    hextets[1] === 0xff9b &&
    hextets.slice(2, 6).every((part) => part === 0);
  if (
    (leadingCompatibleZeros || leadingMappedZeros || nat64WellKnownPrefix) &&
    isPrivateOrReservedIpv4Octets(lastIpv4)
  ) {
    return true;
  }
  if (hextets[0] === 0x2002) {
    return isPrivateOrReservedIpv4Octets(hextetsToIpv4Octets(hextets[1], hextets[2]));
  }
  return false;
}

function isPrivateTronEndpointHost(hostname) {
  const normalized = String(hostname ?? "")
    .trim()
    .toLowerCase()
    .replace(/^\[/u, "")
    .replace(/\]$/u, "")
    .replace(/\.$/u, "");
  if (!normalized) return true;
  if (
    normalized === "localhost" ||
    normalized.endsWith(".localhost") ||
    normalized === "local" ||
    normalized === "::" ||
    normalized === "::1"
  ) {
    return true;
  }
  if (isPrivateOrReservedIpv4(normalized)) {
    return true;
  }
  const ipv4Mapped = normalized.match(/(?::ffff:)?(\d{1,3}(?:\.\d{1,3}){3})$/iu);
  if (ipv4Mapped?.[1] && isPrivateOrReservedIpv4(ipv4Mapped[1])) {
    return true;
  }
  if (!normalized.includes(":")) {
    return false;
  }
  const hextets = parseIpv6Hextets(normalized);
  if (!hextets) {
    return true;
  }
  if (
    hasPrivateOrReservedEmbeddedIpv4(hextets) ||
    (hextets[0] === 0x2001 && hextets[1] === 0)
  ) {
    return true;
  }
  const firstHextet = hextets[0];
  return (
    (firstHextet & 0xfe00) === 0xfc00 ||
    (firstHextet & 0xffc0) === 0xfe80 ||
    (firstHextet & 0xff00) === 0xff00 ||
    normalized.startsWith("2001:db8:")
  );
}

function normalizeTronEndpoint(endpoint = DEFAULT_TRON_ENDPOINT) {
  const raw =
    endpoint === undefined || endpoint === null || endpoint === ""
      ? DEFAULT_TRON_ENDPOINT
      : String(endpoint).trim();
  let parsed;
  try {
    parsed = new URL(raw);
  } catch (error) {
    throw new Error(`TRON endpoint must be a valid HTTPS URL: ${error.message}`);
  }
  if (parsed.protocol !== "https:") {
    throw new Error("TRON endpoint must use HTTPS");
  }
  if (parsed.username || parsed.password) {
    throw new Error("TRON endpoint must not include credentials");
  }
  if (parsed.search || parsed.hash) {
    throw new Error("TRON endpoint must not include query strings or fragments");
  }
  if (isPrivateTronEndpointHost(parsed.hostname)) {
    throw new Error("TRON endpoint must not target localhost or private networks");
  }
  const pathname = parsed.pathname.replace(/\/+$/u, "");
  return `${parsed.origin}${pathname === "/" ? "" : pathname}`;
}

function estimateDeploymentFunding(options = {}) {
  const deployFeeLimitSun = BigInt(
    normalizeSun(options["fee-limit"], "--fee-limit", DEFAULT_DEPLOY_FEE_LIMIT_SUN),
  );
  const triggerFeeLimitSun = BigInt(
    normalizeSun(
      options["trigger-fee-limit"],
      "--trigger-fee-limit",
      DEFAULT_TRIGGER_FEE_LIMIT_SUN,
    ),
  );
  const originEnergyLimit = BigInt(
    normalizeSun(
      options["origin-energy-limit"],
      "--origin-energy-limit",
      DEFAULT_ORIGIN_ENERGY_LIMIT,
    ),
  );
  const safetyMarginPercent = normalizePercent(
    options["safety-margin-percent"],
    "--safety-margin-percent",
    15,
  );
  const deployTransactionCount = 4n;
  const postDeployTriggerTransactionCount = BigInt(POST_DEPLOY_CONFIGURATION_CHECKS.length);
  const totalFeeLimitSun =
    deployTransactionCount * deployFeeLimitSun +
    postDeployTriggerTransactionCount * triggerFeeLimitSun;
  const safetyMarginSun =
    (totalFeeLimitSun * BigInt(safetyMarginPercent) + 99n) / 100n;
  const recommendedMinBalanceSun = totalFeeLimitSun + safetyMarginSun;
  const maxOriginEnergyLimitTotal = deployTransactionCount * originEnergyLimit;
  return {
    schema: "iroha-sccp-tron-taira-xor-funding-estimate/v1",
    network: "tron-mainnet",
    route_id: ROUTE_ID,
    asset_key: ASSET_KEY,
    deployment_transaction_count: Number(deployTransactionCount),
    post_deploy_trigger_transaction_count: Number(postDeployTriggerTransactionCount),
    deploy_fee_limit_sun: deployFeeLimitSun.toString(),
    trigger_fee_limit_sun: triggerFeeLimitSun.toString(),
    total_fee_limit_sun: totalFeeLimitSun.toString(),
    total_fee_limit_trx: sunToTrxText(totalFeeLimitSun),
    safety_margin_percent: safetyMarginPercent,
    safety_margin_sun: safetyMarginSun.toString(),
    recommended_min_balance_sun: recommendedMinBalanceSun.toString(),
    recommended_min_balance_trx: sunToTrxText(recommendedMinBalanceSun),
    origin_energy_limit_per_deploy: originEnergyLimit.toString(),
    max_origin_energy_limit_total: maxOriginEnergyLimitTotal.toString(),
    assumptions: [
      "Budget is a conservative upper bound from configured java-tron fee limits.",
      "Actual burned TRX depends on frozen energy/bandwidth, current TVM energy schedule, and TronGrid node policy.",
      "Run account-status immediately before deploy and fund at least recommended_min_balance_sun.",
    ],
  };
}

function buildDeploymentFundingReadiness(account, options = {}) {
  const balanceSun = normalizeBalanceSun(account?.balance, "account.balance");
  const fundingEstimate = estimateDeploymentFunding(options);
  const recommendedMinBalanceSun = BigInt(fundingEstimate.recommended_min_balance_sun);
  const fundingGapSun =
    balanceSun >= recommendedMinBalanceSun ? 0n : recommendedMinBalanceSun - balanceSun;
  return {
    schema: "iroha-sccp-tron-taira-xor-funding-readiness/v1",
    network: "tron-mainnet",
    route_id: ROUTE_ID,
    asset_key: ASSET_KEY,
    balance_sun: balanceSun.toString(),
    balance_trx: sunToTrxText(balanceSun),
    funding_ready: fundingGapSun === 0n,
    funding_gap_sun: fundingGapSun.toString(),
    funding_gap_trx: sunToTrxText(fundingGapSun),
    funding_estimate: fundingEstimate,
  };
}

function assertDeploymentFundingReady(account, options = {}) {
  const readiness = buildDeploymentFundingReadiness(account, options);
  if (!readiness.funding_ready) {
    throw new Error(
      [
        "TRON deployer funding is below the recommended minimum",
        `balance=${readiness.balance_sun} SUN (${readiness.balance_trx} TRX)`,
        `gap=${readiness.funding_gap_sun} SUN (${readiness.funding_gap_trx} TRX)`,
        `recommended_min=${readiness.funding_estimate.recommended_min_balance_sun} SUN`,
      ].join("; "),
    );
  }
  return readiness;
}

function normalizeUint256(value, label) {
  if (typeof value === "number" && !Number.isSafeInteger(value)) {
    throw new Error(`${label} must be a safe integer, decimal string, or hex string`);
  }
  const parsed =
    typeof value === "bigint"
      ? value
      : typeof value === "string" && /^0x[0-9a-f]+$/iu.test(value)
        ? BigInt(value)
        : BigInt(String(value));
  if (parsed < 0n || parsed >= (1n << 256n)) {
    throw new Error(`${label} must fit uint256`);
  }
  return parsed.toString(10);
}

function base58Encode(bytes) {
  let value = BigInt(`0x${bytesToHex(bytes, false) || "0"}`);
  let encoded = "";
  while (value > 0n) {
    const remainder = Number(value % 58n);
    encoded = BASE58_ALPHABET[remainder] + encoded;
    value /= 58n;
  }
  for (const byte of bytes) {
    if (byte !== 0) break;
    encoded = `${BASE58_ALPHABET[0]}${encoded}`;
  }
  return encoded || BASE58_ALPHABET[0];
}

function base58Decode(value) {
  let numeric = 0n;
  for (const character of value) {
    const digit = BASE58_INDEX.get(character);
    if (digit === undefined) {
      throw new Error("TRON address must use canonical Base58Check characters");
    }
    numeric = numeric * 58n + digit;
  }
  let payload = new Uint8Array();
  if (numeric !== 0n) {
    const hex = numeric.toString(16);
    const normalizedHex = hex.length % 2 === 0 ? hex : `0${hex}`;
    payload = Uint8Array.from(
      normalizedHex.match(/.{1,2}/gu).map((byte) => Number.parseInt(byte, 16)),
    );
  }
  let leadingZeroes = 0;
  while (leadingZeroes < value.length && value[leadingZeroes] === BASE58_ALPHABET[0]) {
    leadingZeroes += 1;
  }
  if (leadingZeroes === 0) return payload;
  const out = new Uint8Array(leadingZeroes + payload.length);
  out.set(payload, leadingZeroes);
  return out;
}

function tronBase58Check(payload) {
  const checksum = sha256(sha256(payload)).slice(0, 4);
  return base58Encode(new Uint8Array([...payload, ...checksum]));
}

function normalizeTronAddress(value, label) {
  if (typeof value !== "string" || value.trim() !== value || value.length === 0) {
    throw new Error(`${label} must be a canonical TRON address`);
  }
  if (value.startsWith("T")) {
    const decoded = base58Decode(value);
    if (decoded.length !== 25) {
      throw new Error(`${label} must decode to 25 Base58Check bytes`);
    }
    const payload = decoded.slice(0, 21);
    const checksum = decoded.slice(21);
    const expected = sha256(sha256(payload)).slice(0, 4);
    if (!checksum.every((byte, index) => byte === expected[index])) {
      throw new Error(`${label} checksum is invalid`);
    }
    if (payload[0] !== 0x41 || payload.slice(1).every((byte) => byte === 0)) {
      throw new Error(`${label} must be a non-zero TRON mainnet address`);
    }
    if (tronBase58Check(payload) !== value) {
      throw new Error(`${label} must be canonical Base58Check`);
    }
    return {
      payload,
      base58: value,
      hex: bytesToHex(payload),
      solidity: bytesToHex(payload.slice(1)),
    };
  }

  const hex = strip0x(value);
  const payload =
    hex.length === 40
      ? new Uint8Array([0x41, ...hexToBytes(hex, label, 20)])
      : hexToBytes(hex, label, 21);
  if (payload[0] !== 0x41 || payload.slice(1).every((byte) => byte === 0)) {
    throw new Error(`${label} must be a non-zero TRON mainnet address`);
  }
  return {
    payload,
    base58: tronBase58Check(payload),
    hex: bytesToHex(payload),
    solidity: bytesToHex(payload.slice(1)),
  };
}

function normalizeTronBase58Address(value, label) {
  if (typeof value !== "string" || !value.startsWith("T")) {
    throw new Error(`${label} must be a canonical TRON Base58Check mainnet address`);
  }
  return normalizeTronAddress(value, label);
}

function compactTronAddress(value, label) {
  const candidate =
    typeof value === "object" && value !== null
      ? value.base58 ?? value.hex ?? value.solidity
      : value;
  const normalized = normalizeTronAddress(candidate, label);
  return {
    base58: normalized.base58,
    hex: normalized.hex,
    solidity: normalized.solidity,
  };
}

function buildDeploymentConfigurationSpecs({
  tokenAddress,
  sourceBridgeAddress,
  verifierAddress,
  bridgeAddress,
}) {
  const token = compactTronAddress(tokenAddress, "token address");
  const sourceBridge = compactTronAddress(sourceBridgeAddress, "source bridge address");
  const verifier = compactTronAddress(verifierAddress, "verifier address");
  const bridge = compactTronAddress(bridgeAddress, "bridge address");
  return [
    {
      key: "token_set_bridge",
      contractKey: "token",
      contractAddress: token,
      functionName: "setBridge",
      args: [bridge.solidity],
      requiredPostDeployCheck: POST_DEPLOY_CONFIGURATION_CHECKS[0],
    },
    {
      key: "token_lock_bridge",
      contractKey: "token",
      contractAddress: token,
      functionName: "lockBridge",
      args: [],
      requiredPostDeployCheck: POST_DEPLOY_CONFIGURATION_CHECKS[1],
    },
    {
      key: "source_bridge_transfer_ownership",
      contractKey: "source_bridge",
      contractAddress: sourceBridge,
      functionName: "transferOwnership",
      args: [bridge.solidity],
      requiredPostDeployCheck: POST_DEPLOY_CONFIGURATION_CHECKS[2],
    },
    {
      key: "verifier_emit_destination_binding",
      contractKey: "verifier",
      contractAddress: verifier,
      functionName: "emitDestinationBindingConfigured",
      args: [],
      requiredPostDeployCheck: POST_DEPLOY_CONFIGURATION_CHECKS[3],
    },
  ];
}

function routeHash(text) {
  return bytesToHex(keccak_256(textEncoder.encode(text)));
}

function concatBytes(...chunks) {
  const total = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  const out = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.length;
  }
  return out;
}

function abiWordUint32(value, label) {
  const parsed = normalizeUint32(value, label);
  const out = new Uint8Array(32);
  new DataView(out.buffer).setUint32(28, parsed, false);
  return out;
}

function abiWordTronAddress(address, label) {
  const normalized = normalizeTronBase58Address(address, label);
  const out = new Uint8Array(32);
  out.set(normalized.payload, 11);
  return out;
}

function tronDestinationBindingHash({
  networkId,
  sourceDomain = SCCP_DOMAIN_SORA,
  targetDomain = SCCP_DOMAIN_TRON,
  verifierAddress,
  verifierCodeHash,
  verifierKeyHash,
  proofFamily = SCCP_PROOF_FAMILY_STARK_FRI,
}) {
  const normalizedNetworkId = hexToBytes(networkId, "destination networkId", 32);
  const normalizedVerifierCodeHash = hexToBytes(verifierCodeHash, "verifierCodeHash", 32);
  const normalizedVerifierKeyHash = hexToBytes(verifierKeyHash, "verifierKeyHash", 32);
  if (normalizedNetworkId.every((byte) => byte === 0)) {
    throw new Error("destination networkId must be non-zero bytes32");
  }
  if (normalizedVerifierCodeHash.every((byte) => byte === 0)) {
    throw new Error("verifierCodeHash must be non-zero bytes32");
  }
  if (normalizedVerifierKeyHash.every((byte) => byte === 0)) {
    throw new Error("verifierKeyHash must be non-zero bytes32");
  }
  if (sourceDomain !== SCCP_DOMAIN_SORA || targetDomain !== SCCP_DOMAIN_TRON) {
    throw new Error("TRON destination binding must be SORA -> TRON");
  }
  if (proofFamily !== SCCP_PROOF_FAMILY_STARK_FRI) {
    throw new Error("TRON destination proof family must be stark-fri-v1");
  }
  const payload = concatBytes(
    keccak_256(textEncoder.encode(TRON_DESTINATION_BINDING_LABEL)),
    keccak_256(textEncoder.encode(TRON_GROTH16_BACKEND)),
    keccak_256(textEncoder.encode(proofFamily)),
    normalizedNetworkId,
    abiWordUint32(sourceDomain, "destination sourceDomain"),
    abiWordUint32(targetDomain, "destination targetDomain"),
    abiWordTronAddress(verifierAddress, "destination verifierAddress"),
    normalizedVerifierCodeHash,
    normalizedVerifierKeyHash,
  );
  return bytesToHex(keccak_256(payload));
}

function tronDestinationBindingKey({
  networkId,
  sourceDomain = SCCP_DOMAIN_SORA,
  targetDomain = SCCP_DOMAIN_TRON,
  verifierAddress,
  verifierCodeHash,
  verifierKeyHash,
  proofFamily = SCCP_PROOF_FAMILY_STARK_FRI,
}) {
  if (sourceDomain !== SCCP_DOMAIN_SORA || targetDomain !== SCCP_DOMAIN_TRON) {
    throw new Error("TRON destination binding key must be SORA -> TRON");
  }
  if (proofFamily !== SCCP_PROOF_FAMILY_STARK_FRI) {
    throw new Error("TRON destination proof family must be stark-fri-v1");
  }
  const address = normalizeTronBase58Address(verifierAddress, "destination verifierAddress");
  const networkIdHex = normalizeBytes32(networkId, "destination networkId").slice(2);
  const codeHash = normalizeBytes32(verifierCodeHash, "verifierCodeHash").toLowerCase();
  const keyHash = normalizeBytes32(verifierKeyHash, "verifierKeyHash").toLowerCase();
  return `tron:${sourceDomain}:${targetDomain}:${networkIdHex}:${address.base58}:${codeHash}:${keyHash}`;
}

function tronAddressFromPublicKey(publicKey) {
  const evmAddress = keccak_256(publicKey.slice(1)).slice(-20);
  const payload = new Uint8Array(21);
  payload[0] = 0x41;
  payload.set(evmAddress, 1);
  return {
    payload,
    hex: bytesToHex(payload),
    base58: tronBase58Check(payload),
    solidity: bytesToHex(payload.slice(1)),
  };
}

function tronAddressFromPrivateKey(privateKey) {
  return tronAddressFromPublicKey(secp256k1.getPublicKey(privateKey, false));
}

function parseArgs(argv) {
  const [command, ...rest] = argv;
  const options = {};
  for (let index = 0; index < rest.length; index += 1) {
    const entry = rest[index];
    if (!entry.startsWith("--")) {
      throw new Error(`Unexpected argument: ${entry}`);
    }
    const key = entry.slice(2);
    const value = rest[index + 1];
    if (!value || value.startsWith("--")) {
      throw new Error(`Missing value for --${key}`);
    }
    options[key] = value;
    index += 1;
  }
  return { command, options };
}

async function writeJson(path, value, mode = 0o600) {
  const out = resolve(path);
  await mkdir(dirname(out), { recursive: true });
  await writeFile(`${out}.tmp`, `${JSON.stringify(value, null, 2)}\n`, { mode });
  await rename(`${out}.tmp`, out);
  return out;
}

async function pathExists(path) {
  try {
    await stat(resolve(path));
    return true;
  } catch (error) {
    if (error?.code === "ENOENT") {
      return false;
    }
    throw error;
  }
}

async function readJson(path, label) {
  let text;
  try {
    text = await readFile(resolve(path), "utf8");
  } catch (error) {
    throw new Error(`${label} could not be read: ${error.message}`);
  }
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${error.message}`);
  }
}

async function assertPrivateJsonFilePermissions(path, label) {
  const resolved = resolve(path);
  let fileStat;
  try {
    fileStat = await lstat(resolved);
  } catch (error) {
    throw new Error(`${label} could not be inspected: ${error.message}`);
  }
  if (fileStat.isSymbolicLink()) {
    throw new Error(`${label} must be a direct file, not a symbolic link`);
  }
  if (!fileStat.isFile()) {
    throw new Error(`${label} must be a regular file`);
  }
  const mode = fileStat.mode & 0o777;
  if (mode !== 0o600) {
    throw new Error(`${label} file mode must be 0600; run chmod 600 ${resolved}`);
  }
}

function loadNodeModule(name, purpose) {
  try {
    return requireFromScript(name);
  } catch (scriptError) {
    try {
      return requireFromCwd(name);
    } catch {
      throw new Error(
        `${purpose} requires the optional "${name}" package. Install it or set NODE_PATH ` +
          `to a node_modules directory that contains "${name}". Original error: ${scriptError.message}`,
      );
    }
  }
}

function resolveNodeModule(name) {
  try {
    return requireFromScript.resolve(name);
  } catch (scriptError) {
    try {
      return requireFromCwd.resolve(name);
    } catch {
      throw scriptError;
    }
  }
}

function parsePrivateKeyHex(value, label = "private_key_hex") {
  const privateKey = hexToBytes(value, label, 32);
  if (!secp256k1.utils.isValidPrivateKey(privateKey)) {
    throw new Error(`${label} is not a valid secp256k1 private key`);
  }
  return privateKey;
}

async function loadDeployerSecret(path) {
  await assertPrivateJsonFilePermissions(path, "deployer secret");
  const secret = await readJson(path, "deployer secret");
  if (secret.schema !== DEPLOYER_SCHEMA) {
    throw new Error(`deployer secret schema must be ${DEPLOYER_SCHEMA}`);
  }
  const privateKey = parsePrivateKeyHex(secret.private_key_hex);
  const derived = tronAddressFromPrivateKey(privateKey);
  if (secret.address_base58 !== derived.base58) {
    throw new Error("deployer secret address_base58 does not match private_key_hex");
  }
  if (secret.address_hex !== derived.hex) {
    throw new Error("deployer secret address_hex does not match private_key_hex");
  }
  return {
    privateKey,
    privateKeyHex: bytesToHex(privateKey, false),
    address: derived,
  };
}

async function generateDeployer(options) {
  const outputPath = options.out ?? DEFAULT_SECRET_OUT;
  if (!optionEnabled(options, "force", false) && (await pathExists(outputPath))) {
    throw new Error(
      `Refusing to overwrite existing deployer secret at ${resolve(outputPath)}. ` +
        "Use --force true only when intentionally rotating the deployment account.",
    );
  }
  const privateKey = secp256k1.utils.randomPrivateKey();
  const address = tronAddressFromPrivateKey(privateKey);
  const createdAt = new Date().toISOString();
  const out = await writeJson(outputPath, {
    schema: DEPLOYER_SCHEMA,
    created_at: createdAt,
    network: "tron-mainnet",
    address_base58: address.base58,
    address_hex: address.hex,
    private_key_hex: bytesToHex(privateKey, false),
    warning:
      "Deployment account only. Fund with the minimum required TRX/energy, deploy, record evidence, then rotate remaining funds out. Do not use for end-user bridging.",
  });
  console.log(JSON.stringify({
    wrote: out,
    address_base58: address.base58,
    address_hex: address.hex,
    next_step: "Fund this deployer with TRX/energy before broadcasting deployment transactions.",
  }, null, 2));
}

function optionEnabled(options, key, fallback = false) {
  if (options[key] === undefined || options[key] === null || options[key] === "") {
    return fallback;
  }
  if (options[key] === "true") return true;
  if (options[key] === "false") return false;
  throw new Error(`--${key} must be true or false`);
}

function addDoctorCheck(checks, name, status, detail = {}) {
  checks.push({ name, status, ...detail });
}

function doctorReady(checks) {
  return !checks.some((entry) => entry.status === "error");
}

async function tryReadText(readText, path, label) {
  try {
    await readText(path);
    return { ok: true };
  } catch (error) {
    return {
      ok: false,
      error: `${label} could not be read: ${error.message}`,
    };
  }
}

async function buildDeploymentDoctorReport(options = {}, deps = {}) {
  const checks = [];
  const readText = deps.readText ?? ((path) => readFile(resolve(path), "utf8"));
  const resolveModule = deps.resolveNodeModule ?? resolveNodeModule;
  const tronPostFn = deps.tronPost ?? tronPost;
  const nodeVersion = deps.nodeVersion ?? process.versions.node;
  const requireSecret = optionEnabled(options, "require-secret", false);
  const requireVerifier = optionEnabled(options, "require-verifier", false);
  const requireOptionalPackages = optionEnabled(
    options,
    "require-optional-packages",
    false,
  );
  const checkAccount = optionEnabled(options, "check-account", false);

  const nodeMajor = Number(String(nodeVersion).split(".")[0]);
  addDoctorCheck(
    checks,
    "node_version",
    Number.isInteger(nodeMajor) && nodeMajor >= 18 ? "ok" : "error",
    {
      node_version: nodeVersion,
      requirement: ">=18",
    },
  );

  let endpoint = options.endpoint ?? DEFAULT_TRON_ENDPOINT;
  try {
    endpoint = normalizeTronEndpoint(endpoint);
    addDoctorCheck(checks, "tron_endpoint", "ok", { endpoint });
  } catch (error) {
    addDoctorCheck(checks, "tron_endpoint", "error", {
      endpoint: String(options.endpoint ?? DEFAULT_TRON_ENDPOINT),
      error: error.message,
    });
  }

  for (const [sourceName, sourcePath] of Object.entries({
    ...CONTRACT_SOURCES,
    "contracts/taira/sccp/TairaXorSccpBurnRecord.ko":
      TAIRA_BURN_RECORD_CONTRACT_SOURCE,
  })) {
    const readable = await tryReadText(readText, sourcePath, sourceName);
    addDoctorCheck(
      checks,
      `source:${sourceName}`,
      readable.ok ? "ok" : "error",
      readable.ok
        ? { path: sourcePath }
        : { path: sourcePath, error: readable.error },
    );
  }

  for (const scriptName of [
    "scripts/sccp_tron_source_bridge_evidence.py",
    "scripts/sccp_tron_live_evidence.py",
  ]) {
    const scriptPath = repoPath(scriptName);
    const readable = await tryReadText(readText, scriptPath, scriptName);
    addDoctorCheck(
      checks,
      `evidence_script:${scriptName}`,
      readable.ok ? "ok" : "error",
      readable.ok
        ? { path: scriptPath }
        : { path: scriptPath, error: readable.error },
    );
  }

  for (const packageName of ["solc", "ethers"]) {
    try {
      const resolved = resolveModule(packageName);
      addDoctorCheck(checks, `optional_package:${packageName}`, "ok", {
        resolved,
      });
    } catch (error) {
      addDoctorCheck(
        checks,
        `optional_package:${packageName}`,
        requireOptionalPackages ? "error" : "warn",
        {
          error:
            `${packageName} is required for compile/deploy; set NODE_PATH or install it before deploy.`,
          detail: error.message,
        },
      );
    }
  }

  const secretPath = options.secret ?? DEFAULT_SECRET_OUT;
  let deployer = null;
  try {
    deployer = await loadDeployerSecret(secretPath);
    addDoctorCheck(checks, "deployer_secret", "ok", {
      path: resolve(secretPath),
      address_base58: deployer.address.base58,
      address_hex: deployer.address.hex,
    });
  } catch (error) {
    addDoctorCheck(checks, "deployer_secret", requireSecret ? "error" : "warn", {
      path: resolve(secretPath),
      error: error.message,
      next_step:
        `Run node scripts/sccp_tron_taira_xor_deploy.mjs generate-deployer --out ${secretPath}`,
    });
  }

  if (options.verifier) {
    try {
      const verifierMaterial = JSON.parse(await readText(options.verifier));
      const verifierArgs = normalizeVerifierConstructorArgs(
        verifierMaterial,
        options,
      );
      addDoctorCheck(checks, "verifier_material", "ok", {
        path: resolve(options.verifier),
        verifier_key_hash: verifierArgs[5],
        proof_family: verifierArgs[6],
        network_id_hex: verifierArgs[7],
      });
    } catch (error) {
      addDoctorCheck(checks, "verifier_material", "error", {
        path: resolve(options.verifier),
        error: error.message,
      });
    }
  } else {
    addDoctorCheck(checks, "verifier_material", requireVerifier ? "error" : "warn", {
      error: "--verifier is required before deploy",
      next_step:
        "Provide production verifier material with --verifier artifacts/sccp-tron/production-verifier-key.json",
    });
  }

  let fundingReadiness = null;
  if (checkAccount) {
    if (!deployer) {
      addDoctorCheck(checks, "deployer_funding", "error", {
        error: "Cannot check TRON funding without a valid deployer secret.",
      });
    } else if (checks.some((entry) => entry.name === "tron_endpoint" && entry.status === "error")) {
      addDoctorCheck(checks, "deployer_funding", "error", {
        error: "Cannot check TRON funding with an invalid endpoint.",
      });
    } else {
      try {
        const account = await tronPostFn(
          endpoint,
          "wallet/getaccount",
          { address: deployer.address.base58, visible: true },
          options,
        );
        fundingReadiness = buildDeploymentFundingReadiness(account, options);
        addDoctorCheck(
          checks,
          "deployer_funding",
          fundingReadiness.funding_ready ? "ok" : "error",
          {
            balance_sun: fundingReadiness.balance_sun,
            balance_trx: fundingReadiness.balance_trx,
            funding_gap_sun: fundingReadiness.funding_gap_sun,
            funding_gap_trx: fundingReadiness.funding_gap_trx,
            recommended_min_balance_sun:
              fundingReadiness.funding_estimate.recommended_min_balance_sun,
          },
        );
      } catch (error) {
        addDoctorCheck(checks, "deployer_funding", "error", {
          error: error.message,
        });
      }
    }
  } else {
    addDoctorCheck(checks, "deployer_funding", "skipped", {
      next_step:
        "Run doctor --check-account true or account-status after funding the deployer.",
    });
  }

  const summary = checks.reduce(
    (acc, entry) => {
      acc[entry.status] = (acc[entry.status] ?? 0) + 1;
      return acc;
    },
    {},
  );
  const ready = doctorReady(checks);
  return {
    schema: "iroha-sccp-tron-taira-xor-deployment-doctor/v1",
    checked_at: new Date().toISOString(),
    network: "tron-mainnet",
    route_id: ROUTE_ID,
    asset_key: ASSET_KEY,
    endpoint,
    ready,
    summary,
    checks,
    funding_estimate: estimateDeploymentFunding(options),
    ...(fundingReadiness ? { funding_readiness: fundingReadiness } : {}),
    next_steps: ready
      ? [
          "Compile TRON contracts and the TAIRA burn-record contract.",
          "If deployer_funding was skipped, run account-status immediately before broadcast deployment.",
          `Run deploy with --broadcast true --confirm-mainnet ${CONFIRMATION_TEXT} only after funding and verifier material are confirmed.`,
        ]
      : [
          "Resolve every error-status check before broadcasting deployment transactions.",
          "Warnings are allowed for early setup, but verifier material, deployer secret, optional packages, and funding must be ready before deploy.",
        ],
  };
}

async function doctorCommand(options) {
  const report = await buildDeploymentDoctorReport(options);
  console.log(JSON.stringify(report, null, 2));
  if (!report.ready) {
    process.exitCode = 1;
  }
}

async function compileTronContracts(options = {}) {
  const solc = loadNodeModule("solc", "TRON SCCP contract compilation");
  const sources = {};
  for (const [sourceName, path] of Object.entries(CONTRACT_SOURCES)) {
    sources[sourceName] = { content: await readFile(path, "utf8") };
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
  if (fatal.length > 0) {
    throw new Error(fatal.map((entry) => entry.formattedMessage).join("\n"));
  }

  const artifacts = {};
  for (const definition of CONTRACT_DEFINITIONS) {
    const contract = output.contracts?.[definition.file]?.[definition.contract];
    if (!contract?.evm?.bytecode?.object) {
      throw new Error(`${definition.contract} did not produce bytecode`);
    }
    artifacts[definition.key] = {
      contractName: definition.contract,
      sourceName: definition.file,
      abi: contract.abi,
      bytecode: `0x${contract.evm.bytecode.object}`,
      deployedBytecode: `0x${contract.evm.deployedBytecode.object}`,
      bytecodeSha256: bytesToHex(sha256(hexToBytes(contract.evm.bytecode.object, `${definition.contract} bytecode`))),
      deployedBytecodeSha256: bytesToHex(
        sha256(hexToBytes(contract.evm.deployedBytecode.object, `${definition.contract} runtime bytecode`)),
      ),
    };
  }

  if (options.out) {
    const outDir = resolve(options.out);
    await mkdir(outDir, { recursive: true });
    for (const [key, artifact] of Object.entries(artifacts)) {
      await writeJson(resolve(outDir, `${key}.json`), artifact, 0o644);
    }
    await writeJson(
      resolve(outDir, "manifest.json"),
      {
        schema: "iroha-sccp-tron-contract-artifacts/v1",
        created_at: new Date().toISOString(),
        solc_version: typeof solc.version === "function" ? solc.version() : "unknown",
        contracts: Object.fromEntries(
          Object.entries(artifacts).map(([key, artifact]) => [
            key,
            {
              contractName: artifact.contractName,
              sourceName: artifact.sourceName,
              bytecodeSha256: artifact.bytecodeSha256,
              deployedBytecodeSha256: artifact.deployedBytecodeSha256,
            },
          ]),
        ),
      },
      0o644,
    );
  }
  return {
    artifacts,
    warnings: errors.filter((entry) => entry.severity !== "error"),
    solcVersion: typeof solc.version === "function" ? solc.version() : "unknown",
  };
}

async function compileTairaBurnRecordContract(options = {}) {
  const source = await readFile(TAIRA_BURN_RECORD_CONTRACT_SOURCE, "utf8");
  const sourceName = "contracts/taira/sccp/TairaXorSccpBurnRecord.ko";
  const compiled = compileKotodamaProgram(source, {
    sourceName,
    forceZk: true,
  });
  if (compiled.diagnostics.length > 0) {
    throw new Error(
      compiled.diagnostics
        .map((entry) => `${entry.severity}: ${entry.message}`)
        .join("\n"),
    );
  }
  if (compiled.artifactBytes.length === 0 || compiled.artifactBytes[6] !== 1) {
    throw new Error("TAIRA burn-record contract must compile with IVM ZK mode bit");
  }

  const artifactBytes = Uint8Array.from(compiled.artifactBytes);
  const contract = {
    schema: "iroha-sccp-taira-xor-burn-record-contract/v1",
    created_at: new Date().toISOString(),
    route_id: ROUTE_ID,
    asset_key: ASSET_KEY,
    source_name: sourceName,
    compiler_fingerprint: compiled.compilerFingerprint,
    code_hash: compiled.codeHashHex,
    abi_hash: compiled.abiHashHex,
    artifact_sha256: bytesToHex(sha256(artifactBytes)),
    artifact_b64: Buffer.from(artifactBytes).toString("base64"),
    manifest: compiled.manifest,
    execution: {
      executable: "IvmProved",
      force_zk_mode: true,
      entrypoint: "burn_and_record",
      settlement_instruction: "Burn<Numeric, Asset>",
      record_instruction: "RecordSccpMessage",
    },
  };
  if (options.out) {
    await writeJson(options.out, contract, 0o644);
  }
  return contract;
}

async function compileTairaContractCommand(options) {
  const out = options.out ?? DEFAULT_TAIRA_CONTRACT_OUT;
  const contract = await compileTairaBurnRecordContract({ out });
  console.log(JSON.stringify({
    wrote: resolve(out),
    route_id: contract.route_id,
    code_hash: contract.code_hash,
    artifact_sha256: contract.artifact_sha256,
    entrypoint: contract.execution.entrypoint,
  }, null, 2));
}

async function compileCommand(options) {
  const { artifacts, warnings, solcVersion } = await compileTronContracts({
    out: options.out ?? DEFAULT_ARTIFACTS_OUT,
  });
  console.log(JSON.stringify({
    wrote: resolve(options.out ?? DEFAULT_ARTIFACTS_OUT),
    solc_version: solcVersion,
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
  }, null, 2));
}

function pickField(record, names, label) {
  for (const name of names) {
    if (record[name] !== undefined) return record[name];
  }
  throw new Error(`verifier material is missing ${label}`);
}

function flattenArray(value, label) {
  if (!Array.isArray(value)) {
    throw new Error(`${label} must be an array`);
  }
  return value.flat(Infinity);
}

function normalizeUint256Array(value, label, expectedLength = null) {
  const values = flattenArray(value, label).map((entry, index) =>
    normalizeUint256(entry, `${label}[${index}]`),
  );
  if (expectedLength !== null && values.length !== expectedLength) {
    throw new Error(`${label} must contain ${expectedLength} uint256 values`);
  }
  return values;
}

function normalizeVerifierConstructorArgs(material, options = {}) {
  if (!material || typeof material !== "object" || Array.isArray(material)) {
    throw new Error("verifier material must be a JSON object");
  }
  const alpha1 = normalizeUint256Array(
    pickField(material, ["alpha1", "configuredAlpha1", "vk_alpha_1"], "alpha1"),
    "alpha1",
    2,
  );
  const beta2 = normalizeUint256Array(
    pickField(material, ["beta2", "configuredBeta2", "vk_beta_2"], "beta2"),
    "beta2",
    4,
  );
  const gamma2 = normalizeUint256Array(
    pickField(material, ["gamma2", "configuredGamma2", "vk_gamma_2"], "gamma2"),
    "gamma2",
    4,
  );
  const delta2 = normalizeUint256Array(
    pickField(material, ["delta2", "configuredDelta2", "vk_delta_2"], "delta2"),
    "delta2",
    4,
  );
  const ic = normalizeUint256Array(
    pickField(material, ["ic", "configuredIc", "vk_ic", "IC"], "ic"),
    "ic",
  );
  if (ic.length < 2) {
    throw new Error("ic must contain at least two points");
  }
  const expectedVerifierKeyHash = normalizeBytes32(
    options.expectedVerifierKeyHash ??
      material.expectedVerifierKeyHash ??
      material.verifierKeyHash ??
      material.verifyingKeyHash,
    "expectedVerifierKeyHash",
  );
  const proofFamily = String(options.proofFamily ?? material.proofFamily ?? "stark-fri-v1");
  if (proofFamily !== "stark-fri-v1") {
    throw new Error("proofFamily must be stark-fri-v1 for production TRON SCCP");
  }
  const networkId = normalizeBytes32(
    options.networkId ?? material.networkId ?? TRON_MAINNET_NETWORK_ID_HEX,
    "networkId",
  );
  if (networkId !== TRON_MAINNET_NETWORK_ID_HEX) {
    throw new Error("networkId must be TRON mainnet for taira_tron_xor deployment");
  }
  const sourceDomain = normalizeUint32(
    options.sourceDomain ?? material.sourceDomain ?? SCCP_DOMAIN_SORA,
    "sourceDomain",
  );
  const targetDomain = normalizeUint32(
    options.targetDomain ?? material.targetDomain ?? SCCP_DOMAIN_TRON,
    "targetDomain",
  );
  if (sourceDomain !== SCCP_DOMAIN_SORA || targetDomain !== SCCP_DOMAIN_TRON) {
    throw new Error("destination verifier domains must be SORA -> TRON");
  }
  return [
    alpha1,
    beta2,
    gamma2,
    delta2,
    ic,
    expectedVerifierKeyHash,
    proofFamily,
    networkId,
    sourceDomain,
    targetDomain,
  ];
}

function decodeTronErrorMessage(value) {
  if (typeof value !== "string") return "";
  const hex = strip0x(value);
  if (hex.length > 0 && hex.length % 2 === 0 && !/[^0-9a-f]/iu.test(hex)) {
    try {
      return new TextDecoder().decode(hexToBytes(hex, "TRON error message"));
    } catch {
      return value;
    }
  }
  return value;
}

function endpointUrl(endpoint, path) {
  return `${normalizeTronEndpoint(endpoint)}/${path.replace(/^\/+/u, "")}`;
}

function runtimeApiKey(options) {
  return options["api-key"] ?? process.env.TRON_PRO_API_KEY ?? process.env.TRON_GRID_API_KEY ?? "";
}

async function tronPost(endpoint, path, body, options = {}) {
  const headers = {
    accept: "application/json",
    "content-type": "application/json",
  };
  const apiKey = runtimeApiKey(options);
  if (apiKey) headers["TRON-PRO-API-KEY"] = apiKey;
  const response = await fetch(endpointUrl(endpoint, path), {
    method: "POST",
    headers,
    body: JSON.stringify(body),
  });
  const text = await response.text();
  let payload;
  try {
    payload = text ? JSON.parse(text) : {};
  } catch (error) {
    throw new Error(`TRON ${path} returned non-JSON response: ${error.message}`);
  }
  if (!response.ok) {
    throw new Error(`TRON ${path} failed with HTTP ${response.status}: ${text}`);
  }
  return payload;
}

function assertTronResult(payload, context) {
  const result = payload?.result;
  if (result === false || (result && typeof result === "object" && result.result === false)) {
    const message = decodeTronErrorMessage(result?.message ?? payload.message ?? payload.code);
    throw new Error(`${context} failed${message ? `: ${message}` : ""}`);
  }
  if (payload?.Error) {
    throw new Error(`${context} failed: ${payload.Error}`);
  }
}

function extractTransaction(payload, label = "TRON response") {
  const transaction = payload?.transaction?.raw_data_hex
    ? payload.transaction
    : payload?.signed_transaction?.raw_data_hex
      ? payload.signed_transaction
      : payload?.transaction?.transaction?.raw_data_hex
        ? payload.transaction.transaction
        : payload?.raw_data_hex
          ? payload
          : null;
  if (!transaction || typeof transaction !== "object") {
    throw new Error(`${label} does not contain a transaction with raw_data_hex`);
  }
  return transaction;
}

function transactionRawDataHash(transaction, label = "transaction") {
  const rawData = hexToBytes(transaction.raw_data_hex, `${label}.raw_data_hex`);
  if (rawData.length === 0) {
    throw new Error(`${label}.raw_data_hex must not be empty`);
  }
  const txid = bytesToHex(sha256(rawData), false);
  if (transaction.txID !== undefined && String(transaction.txID).toLowerCase() !== txid) {
    throw new Error(`${label}.txID does not match SHA-256(raw_data_hex)`);
  }
  return { rawData, hash: sha256(rawData), txid };
}

function transactionOwnerAddress(transaction, label = "transaction") {
  const contracts = transaction.raw_data?.contract;
  if (!Array.isArray(contracts) || contracts.length !== 1) {
    throw new Error(`${label} must contain exactly one raw_data.contract entry`);
  }
  const value = contracts[0]?.parameter?.value;
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} contract parameter value is missing`);
  }
  const ownerAddress = value.owner_address ?? value.ownerAddress;
  if (typeof ownerAddress !== "string") {
    throw new Error(`${label} contract owner_address is missing`);
  }
  const normalizedOwner = normalizeTronAddress(ownerAddress, `${label}.owner_address`);
  const originAddress = value.new_contract?.origin_address ?? value.newContract?.origin_address;
  if (typeof originAddress === "string") {
    const normalizedOrigin = normalizeTronAddress(originAddress, `${label}.new_contract.origin_address`);
    if (normalizedOrigin.base58 !== normalizedOwner.base58) {
      throw new Error(`${label} deployment origin_address does not match owner_address`);
    }
  }
  return normalizedOwner;
}

function assertTransactionOwner(transaction, expected, label = "transaction") {
  const owner = transactionOwnerAddress(transaction, label);
  if (owner.base58 !== expected.base58) {
    throw new Error(`${label} owner ${owner.base58} does not match deployer ${expected.base58}`);
  }
  return owner;
}

function tronRecoverableSignatureIsCanonical(signature) {
  if (signature.length !== 65) return false;
  const recoveryId = signature[64];
  if (!((recoveryId >= 0 && recoveryId <= 3) || (recoveryId >= 27 && recoveryId <= 30))) {
    return false;
  }
  const r = bytesToBigInt(signature.slice(0, 32));
  const s = bytesToBigInt(signature.slice(32, 64));
  return r > 0n && r < SECP256K1_ORDER && s > 0n && s <= SECP256K1_HALF_ORDER;
}

function signTransactionPayload(transaction, deployer) {
  if (Array.isArray(transaction.signature) && transaction.signature.length > 0) {
    throw new Error("transaction is already signed");
  }
  assertTransactionOwner(transaction, deployer.address);
  const { hash, txid } = transactionRawDataHash(transaction);
  const signatureObject = secp256k1.sign(hash, deployer.privateKey, {
    prehash: false,
    lowS: true,
  });
  const compact = signatureObject.toCompactRawBytes();
  const signature = new Uint8Array(65);
  signature.set(compact);
  signature[64] = signatureObject.recovery;
  if (!tronRecoverableSignatureIsCanonical(signature)) {
    throw new Error("generated TRON signature is not canonical");
  }
  const recovered = tronAddressFromPublicKey(
    signatureObject.recoverPublicKey(hash).toRawBytes(false),
  );
  if (recovered.base58 !== deployer.address.base58) {
    throw new Error("generated TRON signature does not recover to deployer");
  }
  const signed = JSON.parse(JSON.stringify(transaction));
  signed.txID = txid;
  signed.signature = [bytesToHex(signature, false)];
  return {
    signed,
    metadata: {
      txid,
      signature: bytesToHex(signature, false),
      signature_recovery_id: signature[64],
      signature_recovered_address: recovered.hex,
      signature_recovered_base58: recovered.base58,
      signature_recovers_to_owner: true,
      owner_address: deployer.address.hex,
      owner_base58: deployer.address.base58,
    },
  };
}

function verifySignedTransactionPayload(transaction, label = "signed transaction") {
  if (!Array.isArray(transaction.signature) || transaction.signature.length !== 1) {
    throw new Error(`${label} must contain exactly one signature`);
  }
  const signature = hexToBytes(transaction.signature[0], `${label}.signature[0]`, 65);
  if (!tronRecoverableSignatureIsCanonical(signature)) {
    throw new Error(`${label}.signature[0] must be a canonical recoverable TRON signature`);
  }
  const owner = transactionOwnerAddress(transaction, label);
  const { hash, txid } = transactionRawDataHash(transaction, label);
  const recoveryId = signature[64] >= 27 ? signature[64] - 27 : signature[64];
  let recovered;
  try {
    const parsedSignature = secp256k1.Signature
      .fromCompact(signature.slice(0, 64))
      .addRecoveryBit(recoveryId);
    recovered = tronAddressFromPublicKey(
      parsedSignature.recoverPublicKey(hash).toRawBytes(false),
    );
  } catch (error) {
    throw new Error(`${label}.signature[0] could not recover signer: ${error.message}`);
  }
  if (recovered.base58 !== owner.base58) {
    throw new Error(
      `${label}.signature[0] recovers ${recovered.base58}, not owner ${owner.base58}`,
    );
  }
  return {
    txid,
    signature: bytesToHex(signature, false),
    signature_recovery_id: signature[64],
    signature_recovered_address: recovered.hex,
    signature_recovered_base58: recovered.base58,
    signature_recovers_to_owner: true,
    owner_address: owner.hex,
    owner_base58: owner.base58,
  };
}

function buildSignedTransactionArtifact(signed, signedAt = new Date()) {
  return {
    schema: SIGNED_TRANSACTION_SCHEMA,
    signed_at: signedAt.toISOString(),
    network: "tron-mainnet",
    network_id_hex: TRON_MAINNET_NETWORK_ID_HEX,
    route_id: ROUTE_ID,
    asset_key: ASSET_KEY,
    purpose: SIGNED_TRANSACTION_PURPOSE,
    ...signed.metadata,
    transaction: signed.signed,
  };
}

function assertExactArtifactField(payload, field, expected, label) {
  if (payload[field] !== expected) {
    throw new Error(`${label}.${field} must be ${expected}`);
  }
}

function buildUnsignedTransactionArtifact(input, createdAt = new Date()) {
  const transaction = extractTransaction(
    { transaction: input.transaction },
    "unsigned transaction",
  );
  const { txid } = transactionRawDataHash(transaction, "unsigned transaction");
  return {
    schema: UNSIGNED_TRANSACTION_SCHEMA,
    created_at: createdAt.toISOString(),
    network: "tron-mainnet",
    network_id_hex: TRON_MAINNET_NETWORK_ID_HEX,
    route_id: ROUTE_ID,
    asset_key: ASSET_KEY,
    purpose: SIGNED_TRANSACTION_PURPOSE,
    step_key: normalizeNonEmptyText(input.stepKey, "step_key"),
    step_kind: normalizeNonEmptyText(input.stepKind, "step_kind"),
    deployer_address_base58: input.deployerAddress.base58,
    deployer_address_hex: input.deployerAddress.hex,
    txid,
    transaction,
  };
}

function assertRouteScopedTransactionArtifact(payload, label) {
  assertExactArtifactField(payload, "network", "tron-mainnet", label);
  assertExactArtifactField(payload, "network_id_hex", TRON_MAINNET_NETWORK_ID_HEX, label);
  assertExactArtifactField(payload, "route_id", ROUTE_ID, label);
  assertExactArtifactField(payload, "asset_key", ASSET_KEY, label);
}

function normalizeUnsignedTransactionArtifactPayload(payload, deployer, label) {
  assertExactArtifactField(payload, "schema", UNSIGNED_TRANSACTION_SCHEMA, label);
  assertRouteScopedTransactionArtifact(payload, label);
  assertExactArtifactField(payload, "purpose", SIGNED_TRANSACTION_PURPOSE, label);
  const stepKey = normalizeNonEmptyText(payload.step_key, `${label}.step_key`);
  const stepKind = normalizeNonEmptyText(payload.step_kind, `${label}.step_kind`);
  if (!["deploy", "trigger"].includes(stepKind)) {
    throw new Error(`${label}.step_kind must be deploy or trigger`);
  }
  const artifactBase58 = normalizeTronAddress(
    payload.deployer_address_base58,
    `${label}.deployer_address_base58`,
  );
  const artifactHex = normalizeTronAddress(
    payload.deployer_address_hex,
    `${label}.deployer_address_hex`,
  );
  if (
    artifactBase58.base58 !== deployer.address.base58 ||
    artifactHex.base58 !== deployer.address.base58
  ) {
    throw new Error(`${label} deployer address does not match loaded deployer secret`);
  }
  const transaction = extractTransaction(payload, label);
  const { txid } = transactionRawDataHash(transaction, `${label}.transaction`);
  if (payload.txid !== txid) {
    throw new Error(`${label}.txid does not match unsigned transaction`);
  }
  return { transaction, stepKey, stepKind };
}

function normalizeDeploymentPlanUnsignedTransaction(payload, options, deployer, label) {
  assertExactArtifactField(payload, "schema", DEPLOYMENT_PLAN_SCHEMA, label);
  assertRouteScopedTransactionArtifact(payload, label);
  if (payload.broadcast !== false) {
    throw new Error(`${label} must be a dry-run deployment plan with broadcast false`);
  }
  const planBase58 = normalizeTronAddress(
    payload.deployer_address_base58,
    `${label}.deployer_address_base58`,
  );
  const planHex = normalizeTronAddress(
    payload.deployer_address_hex,
    `${label}.deployer_address_hex`,
  );
  if (
    planBase58.base58 !== deployer.address.base58 ||
    planHex.base58 !== deployer.address.base58
  ) {
    throw new Error(`${label} deployer address does not match loaded deployer secret`);
  }
  const stepKey = normalizeNonEmptyText(
    options.step ?? options["step-key"],
    "--step",
  );
  if (!Array.isArray(payload.steps)) {
    throw new Error(`${label}.steps must be an array`);
  }
  const matchingSteps = payload.steps.filter(
    (step) => step && typeof step === "object" && step.key === stepKey,
  );
  if (matchingSteps.length !== 1) {
    throw new Error(`${label} must contain exactly one step with key ${stepKey}`);
  }
  const [step] = matchingSteps;
  const stepKind = normalizeNonEmptyText(step.kind, `${label}.steps.${stepKey}.kind`);
  if (!["deploy", "trigger"].includes(stepKind)) {
    throw new Error(`${label}.steps.${stepKey}.kind must be deploy or trigger`);
  }
  const transaction = extractTransaction(
    {
      transaction: step.unsigned_artifact?.transaction ?? step.unsigned_transaction,
    },
    `${label}.steps.${stepKey}`,
  );
  transactionRawDataHash(transaction, `${label}.steps.${stepKey}.transaction`);
  if (step.unsigned_artifact !== undefined) {
    const normalized = normalizeUnsignedTransactionArtifactPayload(
      step.unsigned_artifact,
      deployer,
      `${label}.steps.${stepKey}.unsigned_artifact`,
    );
    if (normalized.stepKey !== stepKey || normalized.stepKind !== stepKind) {
      throw new Error(`${label}.steps.${stepKey}.unsigned_artifact step metadata does not match`);
    }
    return normalized;
  }
  return { transaction, stepKey, stepKind };
}

function normalizeUnsignedTransactionArtifact(payload, options = {}, deployer = null, label = "unsigned transaction artifact") {
  if (!deployer?.address) {
    throw new Error("loaded deployer secret is required to normalize unsigned transactions");
  }
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) {
    throw new Error(`${label} must be a JSON object`);
  }
  assertNoSecretLikeDeploymentArtifactFields(payload, label);
  if (payload.schema === UNSIGNED_TRANSACTION_SCHEMA) {
    return normalizeUnsignedTransactionArtifactPayload(payload, deployer, label);
  }
  if (payload.schema === DEPLOYMENT_PLAN_SCHEMA) {
    return normalizeDeploymentPlanUnsignedTransaction(payload, options, deployer, label);
  }
  throw new Error(
    `${label} must be a route-scoped unsigned artifact or dry-run deployment plan`,
  );
}

function normalizeSignedTransactionArtifact(payload, label = "signed transaction artifact") {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) {
    throw new Error(`${label} must be a JSON object`);
  }
  assertNoSecretLikeDeploymentArtifactFields(payload, label);
  assertExactArtifactField(payload, "schema", SIGNED_TRANSACTION_SCHEMA, label);
  assertRouteScopedTransactionArtifact(payload, label);
  assertExactArtifactField(payload, "purpose", SIGNED_TRANSACTION_PURPOSE, label);

  const transaction = extractTransaction(payload, label);
  const verified = verifySignedTransactionPayload(transaction, `${label}.transaction`);
  for (const field of [
    "txid",
    "signature",
    "signature_recovery_id",
    "signature_recovered_address",
    "signature_recovered_base58",
    "signature_recovers_to_owner",
    "owner_address",
    "owner_base58",
  ]) {
    if (payload[field] !== verified[field]) {
      throw new Error(`${label}.${field} does not match signed transaction`);
    }
  }
  return { transaction, verified };
}

async function signTransactionCommand(options) {
  if (!options.transaction) throw new Error("--transaction is required");
  const deployer = await loadDeployerSecret(options.secret ?? DEFAULT_SECRET_OUT);
  const payload = await readJson(options.transaction, "unsigned transaction");
  const { transaction, stepKey, stepKind } = normalizeUnsignedTransactionArtifact(
    payload,
    options,
    deployer,
    "unsigned transaction",
  );
  const signed = signTransactionPayload(transaction, deployer);
  const out = await writeJson(
    options.out ?? DEFAULT_SIGNED_TRANSACTION_OUT,
    {
      ...buildSignedTransactionArtifact(signed),
      step_key: stepKey,
      step_kind: stepKind,
    },
  );
  console.log(JSON.stringify({ wrote: out, ...signed.metadata }, null, 2));
}

async function estimateBudgetCommand(options) {
  const estimate = estimateDeploymentFunding(options);
  let deployer = null;
  if (options.secret) {
    deployer = await loadDeployerSecret(options.secret);
  }
  console.log(
    JSON.stringify(
      {
        ...(deployer
          ? {
              deployer_address_base58: deployer.address.base58,
              deployer_address_hex: deployer.address.hex,
            }
          : {}),
        funding_estimate: estimate,
      },
      null,
      2,
    ),
  );
}

function requireMainnetConfirmation(options, action) {
  if (options["confirm-mainnet"] !== CONFIRMATION_TEXT) {
    throw new Error(
      `${action} targets TRON mainnet and requires --confirm-mainnet ${CONFIRMATION_TEXT}`,
    );
  }
}

async function broadcastSignedTransaction(endpoint, transaction, options = {}) {
  const payload = await tronPost(endpoint, "wallet/broadcasttransaction", transaction, options);
  assertTronResult(payload, "broadcasttransaction");
  if (payload.result !== true && payload.result?.result !== true) {
    throw new Error(`broadcasttransaction did not report success: ${JSON.stringify(payload)}`);
  }
  return payload;
}

async function broadcastCommand(options) {
  if (!options.transaction) throw new Error("--transaction is required");
  requireMainnetConfirmation(options, "broadcast");
  const payload = await readJson(options.transaction, "signed transaction");
  const { transaction, verified } = normalizeSignedTransactionArtifact(payload);
  const result = await broadcastSignedTransaction(
    options.endpoint ?? DEFAULT_TRON_ENDPOINT,
    transaction,
    options,
  );
  const out = await writeJson(options.out ?? DEFAULT_BROADCAST_OUT, {
    schema: BROADCAST_RESULT_SCHEMA,
    broadcast_at: new Date().toISOString(),
    network: "tron-mainnet",
    network_id_hex: TRON_MAINNET_NETWORK_ID_HEX,
    route_id: ROUTE_ID,
    asset_key: ASSET_KEY,
    purpose: SIGNED_TRANSACTION_PURPOSE,
    txid: verified.txid,
    signature: verified,
    endpoint: options.endpoint ?? DEFAULT_TRON_ENDPOINT,
    result,
  });
  console.log(JSON.stringify({ wrote: out, txid: verified.txid, result }, null, 2));
}

function sleep(ms) {
  return new Promise((resolveSleep) => {
    setTimeout(resolveSleep, ms);
  });
}

async function waitForTransactionInfo(endpoint, txid, options = {}) {
  const attempts = Number(options["poll-attempts"] ?? DEFAULT_POLL_ATTEMPTS);
  const pollMs = Number(options["poll-ms"] ?? DEFAULT_POLL_MS);
  if (!Number.isInteger(attempts) || attempts <= 0) {
    throw new Error("--poll-attempts must be a positive integer");
  }
  if (!Number.isInteger(pollMs) || pollMs <= 0) {
    throw new Error("--poll-ms must be a positive integer");
  }
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    const info = await tronPost(endpoint, "walletsolidity/gettransactioninfobyid", { value: txid }, options);
    if (info && Object.keys(info).length > 0) {
      const receiptResult = info.receipt?.result ?? info.result;
      if (receiptResult && receiptResult !== "SUCCESS") {
        throw new Error(`transaction ${txid} finalized with result ${receiptResult}`);
      }
      return info;
    }
    await sleep(pollMs);
  }
  throw new Error(`transaction ${txid} was not found after ${attempts} polling attempts`);
}

function extractDeployAddress(response, label) {
  const candidates = [
    response.contract_address,
    response.contractAddress,
    response.transaction?.contract_address,
    response.transaction?.contractAddress,
    response.transaction?.raw_data?.contract?.[0]?.parameter?.value?.new_contract?.contract_address,
    response.transaction?.raw_data?.contract?.[0]?.parameter?.value?.newContract?.contract_address,
  ].filter((value) => typeof value === "string" && value.length > 0);
  if (candidates.length === 0) {
    throw new Error(`${label} deployment response did not include a contract address`);
  }
  return normalizeTronAddress(candidates[0], `${label} contract_address`);
}

function deployRequest(ethers, deployer, artifact, constructorArgs, options, contractName) {
  const iface = new ethers.Interface(artifact.abi);
  const parameter = strip0x(iface.encodeDeploy(constructorArgs));
  return {
    owner_address: deployer.address.base58,
    abi: JSON.stringify(artifact.abi),
    bytecode: strip0x(artifact.bytecode),
    fee_limit: normalizeSun(options["fee-limit"], "--fee-limit", DEFAULT_DEPLOY_FEE_LIMIT_SUN),
    parameter,
    origin_energy_limit: normalizeSun(
      options["origin-energy-limit"],
      "--origin-energy-limit",
      DEFAULT_ORIGIN_ENERGY_LIMIT,
    ),
    name: contractName,
    call_value: 0,
    consume_user_resource_percent: 100,
    visible: true,
  };
}

function triggerRequest(ethers, deployer, artifact, contractAddress, functionName, args, options) {
  const iface = new ethers.Interface(artifact.abi);
  return {
    owner_address: deployer.address.base58,
    contract_address: contractAddress.base58,
    data: strip0x(iface.encodeFunctionData(functionName, args)),
    fee_limit: normalizeSun(options["trigger-fee-limit"], "--trigger-fee-limit", DEFAULT_TRIGGER_FEE_LIMIT_SUN),
    call_value: 0,
    visible: true,
  };
}

async function submitSignedStep(endpoint, transaction, deployer, step, options) {
  const signed = signTransactionPayload(transaction, deployer);
  const broadcast = await broadcastSignedTransaction(endpoint, signed.signed, options);
  const receipt = await waitForTransactionInfo(endpoint, signed.metadata.txid, options);
  return {
    ...step,
    txid: signed.metadata.txid,
    signed: signed.metadata,
    broadcast,
    receipt,
  };
}

async function createDeployStep(context, key, artifact, constructorArgs) {
  const definition = CONTRACT_DEFINITIONS.find((entry) => entry.key === key);
  const request = deployRequest(
    context.ethers,
    context.deployer,
    artifact,
    constructorArgs,
    context.options,
    definition.deployName,
  );
  const response = await tronPost(context.endpoint, "wallet/deploycontract", request, context.options);
  assertTronResult(response, `${definition.deployName} deploycontract`);
  const transaction = extractTransaction(response, `${definition.deployName} deploycontract`);
  transactionRawDataHash(transaction, `${definition.deployName} deploy transaction`);
  assertTransactionOwner(transaction, context.deployer.address, `${definition.deployName} deploy transaction`);
  const address = extractDeployAddress(response, definition.deployName);
  const step = {
    kind: "deploy",
    key,
    contractName: definition.contract,
    address_base58: address.base58,
    address_hex: address.hex,
    address_solidity: address.solidity,
    request,
    unsigned_response: response,
    unsigned_transaction: transaction,
  };
  step.unsigned_artifact = buildUnsignedTransactionArtifact({
    stepKey: key,
    stepKind: step.kind,
    deployerAddress: context.deployer.address,
    transaction,
  });
  if (!context.broadcast) return step;
  return submitSignedStep(context.endpoint, transaction, context.deployer, step, context.options);
}

async function createTriggerStep(context, key, artifact, contractAddress, functionName, args) {
  const request = triggerRequest(
    context.ethers,
    context.deployer,
    artifact,
    contractAddress,
    functionName,
    args,
    context.options,
  );
  const response = await tronPost(context.endpoint, "wallet/triggersmartcontract", request, context.options);
  assertTronResult(response, `${functionName} triggersmartcontract`);
  const transaction = extractTransaction(response, `${functionName} triggersmartcontract`);
  transactionRawDataHash(transaction, `${functionName} transaction`);
  assertTransactionOwner(transaction, context.deployer.address, `${functionName} transaction`);
  const step = {
    kind: "trigger",
    key,
    contract: contractAddress.base58,
    functionName,
    args,
    request,
    unsigned_response: response,
    unsigned_transaction: transaction,
  };
  step.unsigned_artifact = buildUnsignedTransactionArtifact({
    stepKey: key,
    stepKind: step.kind,
    deployerAddress: context.deployer.address,
    transaction,
  });
  if (!context.broadcast) return step;
  return submitSignedStep(context.endpoint, transaction, context.deployer, step, context.options);
}

async function deployCommand(options) {
  if (!options.verifier) throw new Error("--verifier is required");
  const broadcast = options.broadcast === "true";
  if (options.broadcast !== undefined && !["true", "false"].includes(options.broadcast)) {
    throw new Error("--broadcast must be true or false");
  }
  if (broadcast) requireMainnetConfirmation(options, "deploy");
  const deployer = await loadDeployerSecret(options.secret ?? DEFAULT_SECRET_OUT);
  const endpoint = normalizeTronEndpoint(options.endpoint ?? DEFAULT_TRON_ENDPOINT);
  let fundingReadiness = null;
  if (broadcast) {
    const account = await tronPost(
      endpoint,
      "wallet/getaccount",
      { address: deployer.address.base58, visible: true },
      options,
    );
    fundingReadiness = assertDeploymentFundingReady(account, options);
  }
  const ethersModule = loadNodeModule("ethers", "TRON SCCP deployment ABI encoding");
  const ethers = ethersModule.ethers ?? ethersModule;
  const verifierMaterial = await readJson(options.verifier, "verifier material");
  const verifierArgs = normalizeVerifierConstructorArgs(verifierMaterial, options);
  const { artifacts, solcVersion } = await compileTronContracts(
    options["artifacts-out"] ? { out: options["artifacts-out"] } : {},
  );
  const context = { endpoint, deployer, ethers, options, broadcast };
  const steps = [];

  const verifierStep = await createDeployStep(context, "verifier", artifacts.verifier, verifierArgs);
  steps.push(verifierStep);
  const verifierAddress = normalizeTronAddress(verifierStep.address_base58, "verifier address");

  const sourceBridgeArgs = [TRON_MAINNET_NETWORK_ID_HEX, SCCP_DOMAIN_TRON, SCCP_DOMAIN_SORA];
  const sourceBridgeStep = await createDeployStep(
    context,
    "source_bridge",
    artifacts.source_bridge,
    sourceBridgeArgs,
  );
  steps.push(sourceBridgeStep);
  const sourceBridgeAddress = normalizeTronAddress(
    sourceBridgeStep.address_base58,
    "source bridge address",
  );

  const tokenStep = await createDeployStep(context, "token", artifacts.token, []);
  steps.push(tokenStep);
  const tokenAddress = normalizeTronAddress(tokenStep.address_base58, "token address");

  const bridgeArgs = [
    tokenAddress.solidity,
    verifierAddress.solidity,
    sourceBridgeAddress.solidity,
    routeHash(ROUTE_ID),
    routeHash(ASSET_KEY),
  ];
  const bridgeStep = await createDeployStep(context, "bridge", artifacts.bridge, bridgeArgs);
  steps.push(bridgeStep);
  const bridgeAddress = normalizeTronAddress(bridgeStep.address_base58, "bridge address");
  const requiredPostDeployConfiguration = buildDeploymentConfigurationSpecs({
    tokenAddress,
    sourceBridgeAddress,
    verifierAddress,
    bridgeAddress,
  });

  if (broadcast) {
    for (const configuration of requiredPostDeployConfiguration) {
      steps.push(
        await createTriggerStep(
          context,
          configuration.key,
          artifacts[configuration.contractKey],
          configuration.contractAddress,
          configuration.functionName,
          configuration.args,
        ),
      );
    }
  }

  const plan = {
    schema: DEPLOYMENT_PLAN_SCHEMA,
    created_at: new Date().toISOString(),
    endpoint,
    network: "tron-mainnet",
    network_id_hex: TRON_MAINNET_NETWORK_ID_HEX,
    route_id: ROUTE_ID,
    route_id_hash: routeHash(ROUTE_ID),
    asset_key: ASSET_KEY,
    asset_key_hash: routeHash(ASSET_KEY),
    broadcast,
    solc_version: solcVersion,
    deployer_address_base58: deployer.address.base58,
    deployer_address_hex: deployer.address.hex,
    funding_estimate: estimateDeploymentFunding(options),
    funding_readiness: fundingReadiness,
    deployment_addresses: {
      verifier: verifierAddress.base58,
      source_bridge: sourceBridgeAddress.base58,
      token: tokenAddress.base58,
      bridge: bridgeAddress.base58,
    },
    required_post_deploy_configuration: requiredPostDeployConfiguration.map((configuration) => ({
      key: configuration.key,
      contract_key: configuration.contractKey,
      contract_address: configuration.contractAddress.base58,
      function: configuration.functionName,
      args: configuration.args,
      required_check: configuration.requiredPostDeployCheck,
    })),
    max_fee_limit_sun_per_deploy_transaction: normalizeSun(
      options["fee-limit"],
      "--fee-limit",
      DEFAULT_DEPLOY_FEE_LIMIT_SUN,
    ),
    max_fee_limit_sun_per_trigger_transaction: normalizeSun(
      options["trigger-fee-limit"],
      "--trigger-fee-limit",
      DEFAULT_TRIGGER_FEE_LIMIT_SUN,
    ),
    steps,
    next_steps: broadcast
      ? [
          "Run evidence command with deployment_addresses.",
          "Run scripts/sccp_tron_live_evidence.py against the deployed verifier/source bridge.",
          "Activate TAIRA SCCP route evidence only after live readback and canary proofs match.",
        ]
      : [
          "Dry-run only: unsigned deploy transactions were created but no contracts were deployed.",
          `Re-run with --broadcast true --confirm-mainnet ${CONFIRMATION_TEXT} after funding the deployer.`,
        ],
  };
  const out = await writeJson(options.out ?? DEFAULT_DEPLOYMENT_OUT, plan);
  console.log(JSON.stringify({
    wrote: out,
    broadcast,
    deployer: deployer.address.base58,
    deployment_addresses: plan.deployment_addresses,
    next_steps: plan.next_steps,
  }, null, 2));
}

async function accountStatusCommand(options) {
  const deployer = await loadDeployerSecret(options.secret ?? DEFAULT_SECRET_OUT);
  const endpoint = normalizeTronEndpoint(options.endpoint ?? DEFAULT_TRON_ENDPOINT);
  const account = await tronPost(
    endpoint,
    "wallet/getaccount",
    { address: deployer.address.base58, visible: true },
    options,
  );
  const readiness = buildDeploymentFundingReadiness(account, options);
  console.log(JSON.stringify({
    endpoint,
    address_base58: deployer.address.base58,
    address_hex: deployer.address.hex,
    exists: Object.keys(account).length > 0,
    balance_sun: readiness.balance_sun,
    balance_trx: readiness.balance_trx,
    funding_ready: readiness.funding_ready,
    funding_gap_sun: readiness.funding_gap_sun,
    funding_gap_trx: readiness.funding_gap_trx,
    funding_estimate: readiness.funding_estimate,
    funding_readiness: readiness,
    raw: account,
  }, null, 2));
}

async function writeEvidence(options) {
  for (const key of ["token", "bridge", "source-bridge", "verifier"]) {
    if (!options[key]) throw new Error(`--${key} is required`);
  }
  const tokenAddress = normalizeTronBase58Address(options.token, "--token");
  const bridgeAddress = normalizeTronBase58Address(options.bridge, "--bridge");
  const sourceBridgeAddress = normalizeTronBase58Address(
    options["source-bridge"],
    "--source-bridge",
  );
  const verifierAddress = normalizeTronBase58Address(options.verifier, "--verifier");
  const uniqueAddresses = new Set([
    tokenAddress.base58,
    bridgeAddress.base58,
    sourceBridgeAddress.base58,
    verifierAddress.base58,
  ]);
  if (uniqueAddresses.size !== 4) {
    throw new Error("Token, bridge, source bridge, and verifier addresses must be distinct");
  }
  const evidence = {
    schema: EVIDENCE_SCHEMA,
    created_at: new Date().toISOString(),
    route_id: ROUTE_ID,
    route_id_hash: routeHash(ROUTE_ID),
    asset_key: ASSET_KEY,
    asset_key_hash: routeHash(ASSET_KEY),
    network: "tron-mainnet",
    network_id_hex: TRON_MAINNET_NETWORK_ID_HEX,
    taira_xor_token_address: tokenAddress.base58,
    taira_xor_token_address_hex: tokenAddress.hex,
    taira_xor_bridge_address: bridgeAddress.base58,
    taira_xor_bridge_address_hex: bridgeAddress.hex,
    sccp_tron_source_bridge_address: sourceBridgeAddress.base58,
    sccp_tron_source_bridge_address_hex: sourceBridgeAddress.hex,
    sccp_tron_destination_verifier_address: verifierAddress.base58,
    sccp_tron_destination_verifier_address_hex: verifierAddress.hex,
    required_post_deploy_checks: [...REQUIRED_POST_DEPLOY_CHECKS],
  };
  const out = await writeJson(options.out ?? DEFAULT_EVIDENCE_OUT, evidence);
  console.log(JSON.stringify({ wrote: out, evidence }, null, 2));
}

function readRequiredField(record, key, label) {
  if (!record || typeof record !== "object" || Array.isArray(record)) {
    throw new Error(`${label} must be a JSON object`);
  }
  if (record[key] === undefined || record[key] === null || record[key] === "") {
    throw new Error(`${label}.${key} is required`);
  }
  return record[key];
}

function assertOptionalAddressHex(record, key, expected, label) {
  const value = record[key];
  if (value === undefined || value === null || value === "") return;
  if (typeof value !== "string" || value.trim() !== value || value.startsWith("T")) {
    throw new Error(`${label} must be a 21-byte TRON hex address`);
  }
  const hex = strip0x(value);
  if (!/^[0-9a-f]{42}$/iu.test(hex)) {
    throw new Error(`${label} must be a 21-byte TRON hex address`);
  }
  const normalized = normalizeTronAddress(value, label);
  if (normalized.base58 !== expected.base58) {
    throw new Error(`${label} does not match its Base58 evidence address`);
  }
}

function normalizeDeploymentEvidence(evidence) {
  if (!evidence || typeof evidence !== "object" || Array.isArray(evidence)) {
    throw new Error("deployment evidence must be a JSON object");
  }
  if (evidence.schema !== EVIDENCE_SCHEMA) {
    throw new Error(`deployment evidence schema must be ${EVIDENCE_SCHEMA}`);
  }
  if (evidence.route_id !== ROUTE_ID) {
    throw new Error(`deployment evidence route_id must be ${ROUTE_ID}`);
  }
  if (evidence.asset_key !== ASSET_KEY) {
    throw new Error(`deployment evidence asset_key must be ${ASSET_KEY}`);
  }
  if (
    normalizeBytes32(readRequiredField(evidence, "route_id_hash", "deployment evidence"), "deployment evidence route_id_hash") !==
    routeHash(ROUTE_ID)
  ) {
    throw new Error(`deployment evidence route_id_hash must match ${ROUTE_ID}`);
  }
  if (
    normalizeBytes32(readRequiredField(evidence, "asset_key_hash", "deployment evidence"), "deployment evidence asset_key_hash") !==
    routeHash(ASSET_KEY)
  ) {
    throw new Error(`deployment evidence asset_key_hash must match ${ASSET_KEY}`);
  }
  if (evidence.network !== "tron-mainnet") {
    throw new Error("deployment evidence network must be tron-mainnet");
  }
  if (normalizeBytes32(evidence.network_id_hex, "deployment evidence network_id_hex") !== TRON_MAINNET_NETWORK_ID_HEX) {
    throw new Error("deployment evidence network_id_hex must be TRON mainnet");
  }

  const token = normalizeTronBase58Address(
    readRequiredField(evidence, "taira_xor_token_address", "deployment evidence"),
    "deployment evidence taira_xor_token_address",
  );
  const bridge = normalizeTronBase58Address(
    readRequiredField(evidence, "taira_xor_bridge_address", "deployment evidence"),
    "deployment evidence taira_xor_bridge_address",
  );
  const sourceBridge = normalizeTronBase58Address(
    readRequiredField(evidence, "sccp_tron_source_bridge_address", "deployment evidence"),
    "deployment evidence sccp_tron_source_bridge_address",
  );
  const verifier = normalizeTronBase58Address(
    readRequiredField(evidence, "sccp_tron_destination_verifier_address", "deployment evidence"),
    "deployment evidence sccp_tron_destination_verifier_address",
  );
  const unique = new Set([token.base58, bridge.base58, sourceBridge.base58, verifier.base58]);
  if (unique.size !== 4) {
    throw new Error("deployment evidence contract addresses must be distinct");
  }
  assertOptionalAddressHex(evidence, "taira_xor_token_address_hex", token, "deployment evidence token hex");
  assertOptionalAddressHex(evidence, "taira_xor_bridge_address_hex", bridge, "deployment evidence bridge hex");
  assertOptionalAddressHex(
    evidence,
    "sccp_tron_source_bridge_address_hex",
    sourceBridge,
    "deployment evidence source bridge hex",
  );
  assertOptionalAddressHex(
    evidence,
    "sccp_tron_destination_verifier_address_hex",
    verifier,
    "deployment evidence verifier hex",
  );
  const requiredPostDeployChecks = evidence.required_post_deploy_checks;
  if (
    !Array.isArray(requiredPostDeployChecks) ||
    !requiredPostDeployChecks.every((entry) => typeof entry === "string")
  ) {
    throw new Error("deployment evidence required_post_deploy_checks must be a string array");
  }
  for (const requiredCheck of REQUIRED_POST_DEPLOY_CHECKS) {
    if (!requiredPostDeployChecks.includes(requiredCheck)) {
      throw new Error(`deployment evidence required_post_deploy_checks is missing: ${requiredCheck}`);
    }
  }
  return { token, bridge, sourceBridge, verifier };
}

function requireJsonObject(value, label) {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return value;
  }
  throw new Error(`${label} must be a JSON object`);
}

function requireBooleanTrue(value, label) {
  if (value !== true) {
    throw new Error(`${label} must be true`);
  }
}

function normalizeLiveEvidenceForRoute(liveEvidence, expected) {
  const summary = requireJsonObject(liveEvidence, "live evidence");
  requireBooleanTrue(summary.full_toml_ready, "live evidence full_toml_ready");

  const sourceBridge = requireJsonObject(summary.source_bridge, "live evidence source_bridge");
  const destinationVerifier = requireJsonObject(
    summary.destination_verifier,
    "live evidence destination_verifier",
  );
  const routeCanary = requireJsonObject(summary.route_canary, "live evidence route_canary");
  const routeCanaryTransaction = requireJsonObject(
    summary.route_canary_transaction ?? routeCanary.transaction,
    "live evidence route_canary_transaction",
  );
  const triggerContract = requireJsonObject(
    routeCanaryTransaction.trigger_contract,
    "live evidence route_canary_transaction.trigger_contract",
  );

  if (
    normalizeTronBase58Address(sourceBridge.address, "live evidence source_bridge.address").base58 !==
    expected.addresses.sourceBridge.base58
  ) {
    throw new Error("live evidence source_bridge.address does not match deployment evidence");
  }
  if (
    normalizeTronBase58Address(destinationVerifier.address, "live evidence destination_verifier.address").base58 !==
    expected.addresses.verifier.base58
  ) {
    throw new Error("live evidence destination_verifier.address does not match deployment evidence");
  }
  if (
    normalizeBytes32(sourceBridge.source_bridge_network_id, "live evidence source_bridge.source_bridge_network_id") !==
    TRON_MAINNET_NETWORK_ID_HEX
  ) {
    throw new Error("live evidence source_bridge.source_bridge_network_id must be TRON mainnet");
  }
  if (sourceBridge.source_domain !== SCCP_DOMAIN_TRON || sourceBridge.target_domain !== SCCP_DOMAIN_SORA) {
    throw new Error("live evidence source bridge domains must be TRON -> SORA");
  }
  if (
    normalizeTronBase58Address(
      sourceBridge.source_bridge_owner_base58,
      "live evidence source_bridge.source_bridge_owner_base58",
    ).base58 !== expected.addresses.bridge.base58
  ) {
    throw new Error("live evidence source bridge owner must be the TAIRA XOR bridge");
  }
  requireBooleanTrue(sourceBridge.config_hash_matches, "live evidence source_bridge.config_hash_matches");
  const sourceBridgeConfigHash = normalizeBytes32(
    sourceBridge.source_bridge_config_hash,
    "live evidence source_bridge.source_bridge_config_hash",
  );

  if (
    normalizeBytes32(destinationVerifier.network_id, "live evidence destination_verifier.network_id") !==
    TRON_MAINNET_NETWORK_ID_HEX
  ) {
    throw new Error("live evidence destination_verifier.network_id must be TRON mainnet");
  }
  if (
    destinationVerifier.destination_source_domain !== SCCP_DOMAIN_SORA ||
    destinationVerifier.destination_target_domain !== SCCP_DOMAIN_TRON
  ) {
    throw new Error("live evidence destination verifier domains must be SORA -> TRON");
  }
  if (
    normalizeBytes32(
      destinationVerifier.destination_verifier_code_hash,
      "live evidence destination_verifier.destination_verifier_code_hash",
    ) !== expected.verifierCodeHash
  ) {
    throw new Error("live evidence destination verifier code hash does not match --verifier-code-hash");
  }
  if (
    normalizeBytes32(
      destinationVerifier.destination_verifier_key_hash,
      "live evidence destination_verifier.destination_verifier_key_hash",
    ) !== expected.verifierKeyHash
  ) {
    throw new Error("live evidence destination verifier key hash does not match verifier material");
  }
  requireBooleanTrue(
    destinationVerifier.verifier_backend_hash_matches,
    "live evidence destination_verifier.verifier_backend_hash_matches",
  );
  requireBooleanTrue(
    destinationVerifier.proof_family_hash_matches,
    "live evidence destination_verifier.proof_family_hash_matches",
  );
  requireBooleanTrue(
    destinationVerifier.destination_binding_hash_matches,
    "live evidence destination_verifier.destination_binding_hash_matches",
  );
  if (destinationVerifier.expected_destination_binding_hash_matches !== undefined) {
    requireBooleanTrue(
      destinationVerifier.expected_destination_binding_hash_matches,
      "live evidence destination_verifier.expected_destination_binding_hash_matches",
    );
  }
  if (destinationVerifier.bytecode_hash_matches_verifier_code_hash !== undefined) {
    requireBooleanTrue(
      destinationVerifier.bytecode_hash_matches_verifier_code_hash,
      "live evidence destination_verifier.bytecode_hash_matches_verifier_code_hash",
    );
  }
  const destinationBindingHash = normalizeBytes32(
    destinationVerifier.destination_binding_hash,
    "live evidence destination_verifier.destination_binding_hash",
  );
  if (destinationBindingHash !== expected.destinationBindingHash) {
    throw new Error("live evidence destination binding hash does not match computed destination binding hash");
  }
  const recomputedDestinationBindingHash = normalizeBytes32(
    destinationVerifier.recomputed_destination_binding_hash,
    "live evidence destination_verifier.recomputed_destination_binding_hash",
  );
  if (recomputedDestinationBindingHash !== expected.destinationBindingHash) {
    throw new Error("live evidence recomputed destination binding hash does not match computed destination binding hash");
  }
  const destinationBindingKey = normalizeNonEmptyText(
    destinationVerifier.destination_binding_key,
    "live evidence destination_verifier.destination_binding_key",
  );
  if (destinationBindingKey !== expected.destinationBindingKey) {
    throw new Error("live evidence destination binding key does not match computed destination binding key");
  }

  if (routeCanary.status !== "passed") {
    throw new Error("live evidence route_canary.status must be passed");
  }
  if (routeCanary.evidence_source !== "tron_message_proof_accepted_transaction") {
    throw new Error(
      "live evidence route_canary.evidence_source must be tron_message_proof_accepted_transaction",
    );
  }
  const routeCanaryEvidenceHash = normalizeBytes32(
    routeCanary.evidence_hash ?? routeCanaryTransaction.route_canary_evidence_hash,
    "live evidence route_canary.evidence_hash",
  );
  if (
    normalizeBytes32(
      routeCanaryTransaction.route_canary_evidence_hash,
      "live evidence route_canary_transaction.route_canary_evidence_hash",
    ) !== routeCanaryEvidenceHash
  ) {
    throw new Error("live evidence route canary transaction hash does not match route_canary.evidence_hash");
  }
  requireBooleanTrue(
    routeCanaryTransaction.message_proof_used,
    "live evidence route_canary_transaction.message_proof_used",
  );
  const routeCanarySourceDomain = normalizeUint32(
    readRequiredField(routeCanaryTransaction, "source_domain", "live evidence route_canary_transaction"),
    "live evidence route_canary_transaction.source_domain",
  );
  if (routeCanarySourceDomain !== SCCP_DOMAIN_SORA) {
    throw new Error("live evidence route canary source domain must be SORA");
  }
  const routeCanaryMessageId = normalizeBytes32(
    readRequiredField(routeCanaryTransaction, "message_id", "live evidence route_canary_transaction"),
    "live evidence route_canary_transaction.message_id",
  );
  const routeCanaryCommitmentRoot = normalizeBytes32(
    readRequiredField(routeCanaryTransaction, "commitment_root", "live evidence route_canary_transaction"),
    "live evidence route_canary_transaction.commitment_root",
  );
  const routeCanaryStatementHash = normalizeBytes32(
    readRequiredField(routeCanaryTransaction, "statement_hash", "live evidence route_canary_transaction"),
    "live evidence route_canary_transaction.statement_hash",
  );
  if (
    normalizeBytes32(
      readRequiredField(
        routeCanaryTransaction,
        "destination_binding_hash",
        "live evidence route_canary_transaction",
      ),
      "live evidence route_canary_transaction.destination_binding_hash",
    ) !== expected.destinationBindingHash
  ) {
    throw new Error("live evidence route canary destination binding hash does not match computed destination binding hash");
  }
  if (
    normalizeBytes32(
      readRequiredField(routeCanaryTransaction, "network_id", "live evidence route_canary_transaction"),
      "live evidence route_canary_transaction.network_id",
    ) !== TRON_MAINNET_NETWORK_ID_HEX
  ) {
    throw new Error("live evidence route canary network id must be TRON mainnet");
  }
  requireBooleanTrue(
    triggerContract.raw_data_owner_matches_transaction,
    "live evidence route_canary_transaction.trigger_contract.raw_data_owner_matches_transaction",
  );
  requireBooleanTrue(
    triggerContract.signature_recovers_to_owner,
    "live evidence route_canary_transaction.trigger_contract.signature_recovers_to_owner",
  );
  requireBooleanTrue(
    triggerContract.raw_data_call_matches ?? triggerContract.call_matches,
    "live evidence route_canary_transaction.trigger_contract.raw_data_call_matches",
  );
  const triggerProofSourceDomain = normalizeUint32(
    readRequiredField(
      triggerContract,
      "proof_source_domain",
      "live evidence route_canary_transaction.trigger_contract",
    ),
    "live evidence route_canary_transaction.trigger_contract.proof_source_domain",
  );
  if (triggerProofSourceDomain !== SCCP_DOMAIN_SORA) {
    throw new Error("live evidence route canary proof source domain must be SORA");
  }
  const triggerTargetDomain = normalizeUint32(
    readRequiredField(
      triggerContract,
      "public_inputs_target_domain",
      "live evidence route_canary_transaction.trigger_contract",
    ),
    "live evidence route_canary_transaction.trigger_contract.public_inputs_target_domain",
  );
  if (triggerTargetDomain !== SCCP_DOMAIN_TRON) {
    throw new Error("live evidence route canary target domain must be TRON");
  }
  if (
    normalizeBytes32(
      readRequiredField(
        triggerContract,
        "public_inputs_message_id",
        "live evidence route_canary_transaction.trigger_contract",
      ),
      "live evidence route_canary_transaction.trigger_contract.public_inputs_message_id",
    ) !== routeCanaryMessageId
  ) {
    throw new Error("live evidence route canary public input message id does not match the accepted event");
  }
  if (
    normalizeBytes32(
      readRequiredField(
        triggerContract,
        "public_inputs_commitment_root",
        "live evidence route_canary_transaction.trigger_contract",
      ),
      "live evidence route_canary_transaction.trigger_contract.public_inputs_commitment_root",
    ) !== routeCanaryCommitmentRoot
  ) {
    throw new Error("live evidence route canary public input commitment root does not match the accepted event");
  }
  if (
    normalizeBytes32(
      readRequiredField(
        triggerContract,
        "statement_hash",
        "live evidence route_canary_transaction.trigger_contract",
      ),
      "live evidence route_canary_transaction.trigger_contract.statement_hash",
    ) !== routeCanaryStatementHash
  ) {
    throw new Error("live evidence route canary statement hash does not match the accepted event");
  }
  if (triggerContract.contract_base58 !== undefined) {
    const triggerContractAddress = normalizeTronBase58Address(
      triggerContract.contract_base58,
      "live evidence route_canary_transaction.trigger_contract.contract_base58",
    );
    if (triggerContractAddress.base58 !== expected.addresses.verifier.base58) {
      throw new Error("live evidence route canary contract_base58 must match the destination verifier");
    }
  }
  if (triggerContract.contract_address !== undefined) {
    const triggerContractAddress = normalizeTronAddress(
      triggerContract.contract_address,
      "live evidence route_canary_transaction.trigger_contract.contract_address",
    );
    if (triggerContractAddress.base58 !== expected.addresses.verifier.base58) {
      throw new Error("live evidence route canary contract_address must match the destination verifier");
    }
  }

  return {
    sourceBridgeConfigHash,
    destinationBindingHash,
    destinationBindingKey,
    routeCanaryEvidenceHash,
    routeCanaryTransactionId: normalizeBytes32(
      routeCanaryTransaction.transaction_id,
      "live evidence route_canary_transaction.transaction_id",
    ),
    offlineFullTomlSha256:
      summary.offline_full_toml_sha256 === undefined
        ? null
        : normalizeBytes32(summary.offline_full_toml_sha256, "live evidence offline_full_toml_sha256"),
  };
}

function normalizeBurnRecordContract(contract) {
  if (!contract || typeof contract !== "object" || Array.isArray(contract)) {
    throw new Error("TAIRA burn-record contract must be a JSON object");
  }
  if (contract.schema !== TAIRA_BURN_RECORD_CONTRACT_SCHEMA) {
    throw new Error(`TAIRA burn-record contract schema must be ${TAIRA_BURN_RECORD_CONTRACT_SCHEMA}`);
  }
  if (contract.route_id !== ROUTE_ID) {
    throw new Error(`TAIRA burn-record contract route_id must be ${ROUTE_ID}`);
  }
  if (contract.asset_key !== ASSET_KEY) {
    throw new Error(`TAIRA burn-record contract asset_key must be ${ASSET_KEY}`);
  }
  if (contract.execution?.executable !== "IvmProved" || contract.execution?.force_zk_mode !== true) {
    throw new Error("TAIRA burn-record contract must be compiled as IvmProved with forced ZK mode");
  }
  if (contract.execution?.entrypoint !== "burn_and_record") {
    throw new Error("TAIRA burn-record contract entrypoint must be burn_and_record");
  }
  if (contract.manifest?.features_bitmap !== 1) {
    throw new Error("TAIRA burn-record contract manifest must carry the IVM ZK feature bit");
  }

  const artifact = normalizeStrictBase64(contract.artifact_b64, "TAIRA burn-record artifact_b64");
  if (
    artifact.bytes.length < TAIRA_BURN_RECORD_ARTIFACT_MIN_BYTES ||
    artifact.bytes.length > TAIRA_BURN_RECORD_ARTIFACT_MAX_BYTES
  ) {
    throw new Error(
      `TAIRA burn-record artifact_b64 must decode to ${TAIRA_BURN_RECORD_ARTIFACT_MIN_BYTES}-${TAIRA_BURN_RECORD_ARTIFACT_MAX_BYTES} bytes`,
    );
  }
  const artifactSha256 = bytesToHex(sha256(artifact.bytes));
  if (normalizeBytes32(contract.artifact_sha256, "TAIRA burn-record artifact_sha256") !== artifactSha256) {
    throw new Error("TAIRA burn-record artifact_sha256 does not match artifact_b64");
  }
  return {
    artifactB64: artifact.text,
    artifactSha256,
    codeHash: normalizeBytes32(contract.code_hash, "TAIRA burn-record code_hash"),
  };
}

async function normalizeVerifierKeyHashFromOptions(options) {
  const explicit = options["verifier-key-hash"]
    ? normalizeBytes32(options["verifier-key-hash"], "--verifier-key-hash")
    : null;
  let fromMaterial = null;
  if (options.verifier) {
    const verifierMaterial = await readJson(options.verifier, "verifier material");
    fromMaterial = normalizeVerifierConstructorArgs(verifierMaterial)[5];
  }
  if (!explicit && !fromMaterial) {
    throw new Error("--verifier-key-hash or --verifier is required");
  }
  if (explicit && fromMaterial && explicit !== fromMaterial) {
    throw new Error("--verifier-key-hash does not match --verifier material");
  }
  return explicit ?? fromMaterial;
}

function normalizeVkRef(options, contract = {}) {
  const contractVkRef = contract.vkRef ?? contract.vk_ref ?? {};
  const backendSource = options["vk-backend"] ?? contractVkRef.backend;
  const nameSource = options["vk-name"] ?? contractVkRef.name;
  const backend = normalizeVerifierKeyRefText(backendSource, "--vk-backend");
  const name = normalizeVerifierKeyRefText(nameSource, "--vk-name");
  if (
    options["vk-backend"] &&
    contractVkRef.backend &&
    backend !== normalizeVerifierKeyRefText(contractVkRef.backend, "contract vkRef.backend")
  ) {
    throw new Error("--vk-backend does not match TAIRA burn-record contract vkRef.backend");
  }
  if (
    options["vk-name"] &&
    contractVkRef.name &&
    name !== normalizeVerifierKeyRefText(contractVkRef.name, "contract vkRef.name")
  ) {
    throw new Error("--vk-name does not match TAIRA burn-record contract vkRef.name");
  }
  return { backend, name };
}

async function buildTairaXorRouteManifestDraft(options = {}) {
  const evidence = await readJson(options.evidence ?? DEFAULT_EVIDENCE_OUT, "deployment evidence");
  const contract = await readJson(
    options["taira-contract"] ?? DEFAULT_TAIRA_CONTRACT_OUT,
    "TAIRA burn-record contract",
  );
  const addresses = normalizeDeploymentEvidence(evidence);
  const burnContract = normalizeBurnRecordContract(contract);
  const settlementAssetDefinitionId = normalizeCanonicalAssetDefinitionId(
    options["settlement-asset-definition-id"],
    "--settlement-asset-definition-id",
  );
  const verifierCodeHash = normalizeBytes32(options["verifier-code-hash"], "--verifier-code-hash");
  const verifierKeyHash = await normalizeVerifierKeyHashFromOptions(options);
  const vkRef = normalizeVkRef(options, contract);
  const gasLimit = normalizePositiveSafeInteger(options["gas-limit"], "--gas-limit", 2_000_000);
  const productionReady = optionEnabled(options, "production-ready", false);
  const liveReadbackChecked = optionEnabled(options, "live-readback-checked", false);
  if (productionReady) {
    requireMainnetConfirmation(options, "route-manifest production readiness");
    if (!liveReadbackChecked) {
      throw new Error(
        "production-ready route manifests require --live-readback-checked true after TRON contract readback",
      );
    }
    if (!options["live-evidence"]) {
      throw new Error(
        "production-ready route manifests require --live-evidence from scripts/sccp_tron_live_evidence.py",
      );
    }
  }

  const destinationBindingHash = tronDestinationBindingHash({
    networkId: TRON_MAINNET_NETWORK_ID_HEX,
    verifierAddress: addresses.verifier.base58,
    verifierCodeHash,
    verifierKeyHash,
  });
  const destinationBindingKey = tronDestinationBindingKey({
    networkId: TRON_MAINNET_NETWORK_ID_HEX,
    verifierAddress: addresses.verifier.base58,
    verifierCodeHash,
    verifierKeyHash,
  });
  const liveRouteEvidence = options["live-evidence"]
    ? normalizeLiveEvidenceForRoute(
        await readJson(options["live-evidence"], "live evidence"),
        {
          addresses,
          verifierCodeHash,
          verifierKeyHash,
          destinationBindingHash,
          destinationBindingKey,
        },
      )
    : null;
  const expectedBindingHash =
    options["expected-destination-binding-hash"] ??
    options["destination-binding-hash"] ??
    liveRouteEvidence?.destinationBindingHash;
  if (productionReady && !expectedBindingHash) {
    throw new Error("production-ready route manifests require --expected-destination-binding-hash from live readback");
  }
  if (
    expectedBindingHash &&
    normalizeBytes32(expectedBindingHash, "--expected-destination-binding-hash") !== destinationBindingHash
  ) {
    throw new Error("--expected-destination-binding-hash does not match computed destination binding hash");
  }
  const expectedBindingKey =
    options["expected-destination-binding-key"] ??
    options["destination-binding-key"] ??
    liveRouteEvidence?.destinationBindingKey;
  if (productionReady && !expectedBindingKey) {
    throw new Error("production-ready route manifests require --expected-destination-binding-key from live readback");
  }
  if (expectedBindingKey && normalizeNonEmptyText(expectedBindingKey, "--expected-destination-binding-key") !== destinationBindingKey) {
    throw new Error("--expected-destination-binding-key does not match computed destination binding key");
  }

  const manifest = {
    schema: ROUTE_MANIFEST_SCHEMA,
    createdAt: new Date().toISOString(),
    routeId: ROUTE_ID,
    assetKey: ASSET_KEY,
    chain: "tron-mainnet",
    counterpartyDomain: SCCP_DOMAIN_TRON,
    verifierTarget: "TronContract",
    productionReady,
    ...(productionReady
      ? { postDeployReadbackChecked: true }
      : {
          disabledReason:
            "Route manifest draft is not production-ready until TRON contract readback and live canary evidence are complete.",
        }),
    networkIdHex: TRON_MAINNET_NETWORK_ID_HEX,
    tairaXorTokenAddress: addresses.token.base58,
    tairaXorBridgeAddress: addresses.bridge.base58,
    sccpTronSourceBridgeAddress: addresses.sourceBridge.base58,
    tronVerifierAddress: addresses.verifier.base58,
    sccpTronDestinationVerifierAddress: addresses.verifier.base58,
    destinationRollout: {
      version: 1,
      destinationNetworkId: TRON_MAINNET_NETWORK_ID_HEX,
      sourceDomain: SCCP_DOMAIN_SORA,
      targetDomain: SCCP_DOMAIN_TRON,
      verifierIdentity: addresses.verifier.base58,
      verifierBackend: TRON_GROTH16_BACKEND,
      proofFamily: SCCP_PROOF_FAMILY_STARK_FRI,
      verifierCodeHash,
      verifierKeyHash,
      destinationBindingHash,
      destinationBindingKey,
    },
    destinationBinding: {
      version: 1,
      key: destinationBindingKey,
      sourceDomain: SCCP_DOMAIN_SORA,
      targetDomain: SCCP_DOMAIN_TRON,
      bindingHash: destinationBindingHash,
      networkIdHex: TRON_MAINNET_NETWORK_ID_HEX,
    },
    tairaXorBurnRecord: {
      settlementAssetDefinitionId,
      contractArtifactB64: burnContract.artifactB64,
      artifactSha256: burnContract.artifactSha256,
      codeHash: burnContract.codeHash,
      vkRef,
      gasLimit,
    },
    settlement: {
      submitPath: "/v1/bridge/messages",
      mode: "finalize_inbound",
      routeId: ROUTE_ID,
      assetKey: ASSET_KEY,
    },
    ...(liveRouteEvidence
      ? {
          postDeployLiveEvidence: {
            fullTomlReady: true,
            sourceBridgeConfigHash: liveRouteEvidence.sourceBridgeConfigHash,
            routeCanaryEvidenceHash: liveRouteEvidence.routeCanaryEvidenceHash,
            routeCanaryTransactionId: liveRouteEvidence.routeCanaryTransactionId,
            ...(liveRouteEvidence.offlineFullTomlSha256
              ? { offlineFullTomlSha256: liveRouteEvidence.offlineFullTomlSha256 }
              : {}),
          },
        }
      : {}),
  };
  return manifest;
}

async function routeManifestCommand(options) {
  const manifest = await buildTairaXorRouteManifestDraft(options);
  const out = await writeJson(options.out ?? DEFAULT_ROUTE_MANIFEST_OUT, manifest, 0o644);
  console.log(JSON.stringify({
    wrote: out,
    routeId: manifest.routeId,
    assetKey: manifest.assetKey,
    productionReady: manifest.productionReady,
    tronBridgeAddress: manifest.tairaXorBridgeAddress,
    tronTokenAddress: manifest.tairaXorTokenAddress,
    tronVerifierAddress: manifest.tronVerifierAddress,
    destinationBindingHash: manifest.destinationRollout.destinationBindingHash,
    settlementAssetDefinitionId: manifest.tairaXorBurnRecord.settlementAssetDefinitionId,
    nextStep: manifest.productionReady
      ? "Publish this manifest only after the TAIRA SCCP route activation evidence is governed on-chain."
      : "Run TRON readback/canary evidence, then re-run with --live-evidence plus --production-ready true --live-readback-checked true --confirm-mainnet taira_tron_xor.",
  }, null, 2));
}

function expectThrows(fn, messagePattern) {
  try {
    fn();
  } catch (error) {
    if (messagePattern && !messagePattern.test(error.message)) {
      throw new Error(`Expected error ${messagePattern}, got: ${error.message}`);
    }
    return;
  }
  throw new Error("Expected function to throw");
}

function selfTest() {
  const privateKey = new Uint8Array(32).fill(1);
  const address = tronAddressFromPrivateKey(privateKey);
  if (!address.base58.startsWith("T") || !address.hex.startsWith("0x41")) {
    throw new Error("TRON address derivation self-test failed");
  }
  const normalized = normalizeTronBase58Address(address.base58, "self-test address");
  if (normalized.hex !== address.hex || normalized.solidity !== `0x${address.hex.slice(4)}`) {
    throw new Error("TRON address normalization self-test failed");
  }
  if (normalizeTronAddress(address.hex, "self-test address hex").base58 !== address.base58) {
    throw new Error("TRON hex address normalization self-test failed");
  }
  for (const invalid of [
    "",
    ` ${address.base58}`,
    `${address.base58.slice(0, -1)}${address.base58.endsWith("1") ? "2" : "1"}`,
    "1111111111111111111111111111111111",
    "0x410000000000000000000000000000000000000000",
  ]) {
    expectThrows(() => normalizeTronBase58Address(invalid, "invalid self-test address"));
  }
  if (routeHash(ROUTE_ID).length !== 66 || routeHash(ASSET_KEY).length !== 66) {
    throw new Error("Route hash self-test failed");
  }

  const mockRawData = bytesToHex(new Uint8Array([1, 2, 3, 4, 5]), false);
  const mockTx = {
    visible: true,
    txID: bytesToHex(sha256(hexToBytes(mockRawData, "mock raw data")), false),
    raw_data: {
      contract: [
        {
          parameter: {
            value: {
              owner_address: address.base58,
            },
          },
          type: "TriggerSmartContract",
        },
      ],
      timestamp: 1,
      expiration: 2,
    },
    raw_data_hex: mockRawData,
  };
  const signed = signTransactionPayload(mockTx, { privateKey, address });
  if (signed.signed.signature.length !== 1 || signed.metadata.txid !== mockTx.txID) {
    throw new Error("TRON signing self-test failed");
  }
  expectThrows(
    () => signTransactionPayload({ ...mockTx, txID: "00".repeat(32) }, { privateKey, address }),
    /txID/,
  );
  expectThrows(
    () =>
      signTransactionPayload(
        {
          ...mockTx,
          raw_data: {
            contract: [
              {
                parameter: {
                  value: {
                    owner_address: tronAddressFromPrivateKey(new Uint8Array(32).fill(2)).base58,
                  },
                },
              },
            ],
          },
        },
        { privateKey, address },
      ),
    /does not match deployer/,
  );
  expectThrows(
    () => signTransactionPayload({ ...mockTx, signature: [signed.metadata.signature] }, { privateKey, address }),
    /already signed/,
  );

  const verifierMaterial = {
    alpha1: [1, 2],
    beta2: [[3, 4], [5, 6]],
    gamma2: [7, 8, 9, 10],
    delta2: [11, 12, 13, 14],
    ic: [15, 16, 17, 18],
    verifierKeyHash: routeHash("verifier-key"),
  };
  const verifierArgs = normalizeVerifierConstructorArgs(verifierMaterial);
  if (verifierArgs.length !== 10 || verifierArgs[5] !== verifierMaterial.verifierKeyHash) {
    throw new Error("Verifier material normalization self-test failed");
  }
  expectThrows(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial, proofFamily: "debug" }),
    /proofFamily/,
  );
  expectThrows(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial, networkId: routeHash("wrong-network") }),
    /networkId/,
  );

  const duplicateEvidenceAddress = address.base58;
  expectThrows(() => {
    const uniqueAddresses = new Set([
      duplicateEvidenceAddress,
      duplicateEvidenceAddress,
      tronAddressFromPrivateKey(new Uint8Array(32).fill(3)).base58,
      tronAddressFromPrivateKey(new Uint8Array(32).fill(4)).base58,
    ]);
    if (uniqueAddresses.size !== 4) {
      throw new Error("Token, bridge, source bridge, and verifier addresses must be distinct");
    }
  }, /distinct/);

  console.log("sccp_tron_taira_xor_deploy: self-test ok");
}

async function main() {
  if (process.argv.includes("--help") || process.argv.includes("-h")) {
    console.log(usage());
    return;
  }
  const { command, options } = parseArgs(process.argv.slice(2));
  if (command === "generate-deployer") return generateDeployer(options);
  if (command === "doctor") return doctorCommand(options);
  if (command === "estimate-budget") return estimateBudgetCommand(options);
  if (command === "account-status") return accountStatusCommand(options);
  if (command === "compile") return compileCommand(options);
  if (command === "compile-taira-contract") return compileTairaContractCommand(options);
  if (command === "deploy") return deployCommand(options);
  if (command === "sign-transaction") return signTransactionCommand(options);
  if (command === "broadcast") return broadcastCommand(options);
  if (command === "evidence") return writeEvidence(options);
  if (command === "route-manifest") return routeManifestCommand(options);
  if (command === "self-test") return selfTest();
  throw new Error(usage());
}

if (process.argv[1] && resolve(process.argv[1]) === SCRIPT_PATH) {
  main().catch((error) => {
    console.error(error.message || error);
    process.exitCode = 1;
  });
}

export {
  ASSET_KEY,
  ROUTE_ID,
  TAIRA_BURN_RECORD_ARTIFACT_MAX_BYTES,
  TAIRA_BURN_RECORD_ARTIFACT_MIN_BYTES,
  TRON_MAINNET_NETWORK_ID_HEX,
  assertDeploymentFundingReady,
  buildDeploymentDoctorReport,
  buildDeploymentConfigurationSpecs,
  buildDeploymentFundingReadiness,
  buildSignedTransactionArtifact,
  buildUnsignedTransactionArtifact,
  buildTairaXorRouteManifestDraft,
  bytesToHex,
  compileTairaBurnRecordContract,
  estimateDeploymentFunding,
  generateDeployer,
  hexToBytes,
  normalizeTronAddress,
  normalizeTronBase58Address,
  normalizeTronEndpoint,
  normalizeSignedTransactionArtifact,
  normalizeUnsignedTransactionArtifact,
  normalizeVerifierConstructorArgs,
  routeHash,
  signTransactionPayload,
  tronDestinationBindingHash,
  tronDestinationBindingKey,
  tronAddressFromPrivateKey,
  verifySignedTransactionPayload,
};
