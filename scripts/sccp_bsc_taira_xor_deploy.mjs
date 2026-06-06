#!/usr/bin/env node
// Purpose: compile, deploy, configure, and evidence-check the BSC testnet
// contracts for the TAIRA XOR SCCP bridge without persisting operator keys.
// Safe default: no transaction is broadcast unless the command includes
// `--broadcast true --confirm-testnet taira_bsc_xor`.
//
// Prerequisites:
// - Node.js 18+.
// - `solc` and `ethers` on NODE_PATH for compile/deploy/evidence commands.
// - A funded BSC testnet deployer key supplied only through an environment
//   variable such as SCCP_BSC_DEPLOYER_PRIVATE_KEY.
import { createRequire } from "node:module";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { sha256 } from "../javascript/iroha_js/node_modules/@noble/hashes/sha256.js";
import { keccak_256 } from "../javascript/iroha_js/node_modules/@noble/hashes/sha3.js";

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
export const BSC_EVM_GROTH16_BACKEND = "evm-groth16-bn254-v1";
export const SCCP_PROOF_FAMILY_STARK_FRI = "stark-fri-v1";
export const SCCP_BSC_DIAGNOSTIC_VERIFIER_KEY_HASHES = new Set([
  "0x9ef8067d260532f88e60cfa4b458fe678fc46b9c242de18fc91ba646e0857fc4",
]);
export const DEPLOYMENT_EVIDENCE_SCHEMA =
  "iroha-sccp-bsc-taira-xor-deployment-evidence/v1";
export const ROUTE_MANIFEST_SCHEMA =
  "iroha-sccp-taira-xor-route-manifest-draft/v1";
export const DEFAULT_ARTIFACTS_OUT = "artifacts/sccp-bsc/contracts";
export const DEFAULT_EVIDENCE_OUT =
  "artifacts/sccp-bsc/taira-bsc-xor-deployment.evidence.json";
export const DEFAULT_ROUTE_MANIFEST_OUT =
  "artifacts/sccp-bsc/taira-bsc-xor-route.manifest.json";
export const DEFAULT_ROUTE_CONFIG_OUT =
  "artifacts/sccp-bsc/taira-bsc-xor-route.torii.toml";
export const DEFAULT_ROUTE_FULL_CONFIG_OUT =
  "artifacts/sccp-bsc/taira-bsc-xor-route.full-taira-config.toml";
export const DEFAULT_PRIVATE_KEY_ENV = "SCCP_BSC_DEPLOYER_PRIVATE_KEY";

const DESTINATION_BINDING_LABEL = "iroha:sccp:evm-destination-binding:v1";
const SECRET_KEY_PATTERN =
  /(?:private[_-]?key|mnemonic|recovery[_-]?phrase|seed[_-]?phrase|secret)/iu;
const PRIVATE_KEY_PEM_PATTERN =
  /-----BEGIN(?: [A-Z0-9]+)* PRIVATE KEY-----[\s\S]*?-----END(?: [A-Z0-9]+)* PRIVATE KEY-----/iu;
const RECOVERY_PHRASE_WORD_COUNTS = new Set([12, 15, 18, 21, 24]);
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

const CONTRACT_SOURCES = Object.freeze({
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
  "contracts/bsc/sccp/SccpBscSourceBridge.sol": repoPath(
    "contracts",
    "bsc",
    "sccp",
    "SccpBscSourceBridge.sol",
  ),
  "contracts/bsc/sccp/TairaXOR.sol": repoPath("contracts", "bsc", "sccp", "TairaXOR.sol"),
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
  node scripts/sccp_bsc_taira_xor_deploy.mjs deploy --verifier <verifier-key.json> --broadcast true --confirm-testnet ${CONFIRMATION_TEXT} [--allow-diagnostic-verifier true] [--private-key-env ${DEFAULT_PRIVATE_KEY_ENV}] [--rpc-url ${DEFAULT_BSC_RPC_URL}] [--out ${DEFAULT_EVIDENCE_OUT}]
  node scripts/sccp_bsc_taira_xor_deploy.mjs evidence --token <addr> --bridge <addr> --source-bridge <addr> --verifier <addr> [--rpc-url ${DEFAULT_BSC_RPC_URL}] [--out ${DEFAULT_EVIDENCE_OUT}]
  node scripts/sccp_bsc_taira_xor_deploy.mjs route-config [--manifest ${DEFAULT_ROUTE_MANIFEST_OUT}] [--allow-unready true|false] [--base-config configs/soranexus/taira/config.toml] [--out ${DEFAULT_ROUTE_CONFIG_OUT}]
  node scripts/sccp_bsc_taira_xor_deploy.mjs self-test

Required optional packages for compile/deploy/evidence: solc and ethers. The
contract smoke NODE_PATH can be reused after scripts/sccp_evm_contract_smoke.sh
has installed its temporary dependencies, or install equivalent local packages.

This helper writes only public deployment evidence. It reads deployer key
material only from the named environment variable at runtime and never writes it.`;
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

const parseBoolean = (value) => ["1", "true", "yes", "on"].includes(trim(value).toLowerCase());

function bytesToHex(bytes, prefix = true) {
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  return prefix ? `0x${hex}` : hex;
}

function hexToBytes(value, label, byteLength = null, { allowZero = false } = {}) {
  const normalized = trim(value).toLowerCase().replace(/^0x/u, "");
  if (!/^(?:[0-9a-f]{2})*$/u.test(normalized)) {
    throw new Error(`${label} must be hex bytes.`);
  }
  if (byteLength !== null && normalized.length !== byteLength * 2) {
    throw new Error(`${label} must be ${byteLength} bytes.`);
  }
  const bytes = Uint8Array.from(
    normalized.match(/.{2}/gu)?.map((chunk) => Number.parseInt(chunk, 16)) ?? [],
  );
  if (!allowZero && bytes.every((byte) => byte === 0)) {
    throw new Error(`${label} must be non-zero.`);
  }
  return bytes;
}

export function normalizeHex32(value, label = "value") {
  return bytesToHex(hexToBytes(value, label, 32));
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

export function normalizeBscRpcUrl(value = DEFAULT_BSC_RPC_URL, { allowLocal = false } = {}) {
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
    throw new Error("BSC RPC URL must not contain credentials, query strings, or fragments.");
  }
  if (url.protocol === "http:" && !isLocalhost) {
    throw new Error("HTTP BSC RPC URLs are only allowed for localhost.");
  }
  url.pathname = url.pathname.replace(/\/+$/u, "") || "/";
  return url.toString().replace(/\/$/u, "");
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

export function bscDestinationBindingHash({
  networkId = BSC_TESTNET_NETWORK_ID_HEX,
  verifierAddress,
  bridgeAddress,
  verifierCodeHash,
  verifierKeyHash,
} = {}) {
  const encoded = concatBytes([
    abiWordBytes(keccakTextHex(DESTINATION_BINDING_LABEL), "destination binding label", 32),
    abiWordBytes(keccakTextHex(BSC_EVM_GROTH16_BACKEND), "verifier backend hash", 32),
    abiWordBytes(keccakTextHex(SCCP_PROOF_FAMILY_STARK_FRI), "proof family hash", 32),
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

export function bscDestinationBindingKey({
  networkId = BSC_TESTNET_NETWORK_ID_HEX,
  verifierAddress,
  bridgeAddress,
  verifierCodeHash,
  verifierKeyHash,
} = {}) {
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
  for (const name of names) {
    if (record[name] !== undefined) {
      return record[name];
    }
  }
  throw new Error(`verifier material is missing ${label}`);
}

function flattenArray(value, label) {
  if (!Array.isArray(value)) {
    throw new Error(`${label} must be an array.`);
  }
  return value.flat(Infinity);
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

export function normalizeVerifierMaterial(material) {
  if (!material || typeof material !== "object" || Array.isArray(material)) {
    throw new Error("verifier material must be a JSON object.");
  }
  const proofFamily = String(material.proofFamily ?? SCCP_PROOF_FAMILY_STARK_FRI);
  if (proofFamily !== SCCP_PROOF_FAMILY_STARK_FRI) {
    throw new Error("proofFamily must be stark-fri-v1 for BSC SCCP.");
  }
  const networkId = normalizeHex32(material.networkId ?? BSC_TESTNET_NETWORK_ID_HEX, "networkId");
  if (networkId !== BSC_TESTNET_NETWORK_ID_HEX) {
    throw new Error("networkId must be BSC testnet for taira_bsc_xor.");
  }
  const sourceDomain = normalizeUint32(material.sourceDomain ?? SCCP_DOMAIN_SORA, "sourceDomain");
  const targetDomain = normalizeUint32(material.targetDomain ?? SCCP_DOMAIN_BSC, "targetDomain");
  if (sourceDomain !== SCCP_DOMAIN_SORA || targetDomain !== SCCP_DOMAIN_BSC) {
    throw new Error("destination verifier domains must be SORA -> BSC.");
  }
  const expectedVerifierKeyHash = normalizeHex32(
    material.expectedVerifierKeyHash ?? material.verifierKeyHash ?? material.verifyingKeyHash,
    "expectedVerifierKeyHash",
  );
  const diagnosticVerifierReasons = [
    diagnosticFlagReason(material, "verifier material"),
    isKnownDiagnosticBscVerifierKeyHash(expectedVerifierKeyHash)
      ? `verifierKeyHash=${expectedVerifierKeyHash} is a known diagnostic BSC verifier key hash`
      : "",
  ].filter(Boolean);
  return {
    alpha1: normalizeUint256Array(
      pickField(material, ["alpha1", "configuredAlpha1", "vk_alpha_1"], "alpha1"),
      "alpha1",
      2,
    ),
    beta2: normalizeUint256Array(
      pickField(material, ["beta2", "configuredBeta2", "vk_beta_2"], "beta2"),
      "beta2",
      4,
    ),
    gamma2: normalizeUint256Array(
      pickField(material, ["gamma2", "configuredGamma2", "vk_gamma_2"], "gamma2"),
      "gamma2",
      4,
    ),
    delta2: normalizeUint256Array(
      pickField(material, ["delta2", "configuredDelta2", "vk_delta_2"], "delta2"),
      "delta2",
      4,
    ),
    ic: normalizeUint256Array(
      pickField(material, ["ic", "configuredIc", "vk_ic", "IC"], "ic"),
      "ic",
      20,
    ),
    expectedVerifierKeyHash,
    diagnosticVerifierReasons,
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
  return Object.prototype.hasOwnProperty.call(record, key);
}

function readFirstString(record, ...keys) {
  if (!isRecord(record)) {
    return "";
  }
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string" && value.trim()) {
      return value.trim();
    }
  }
  return "";
}

function readFirstRecord(record, ...keys) {
  if (!isRecord(record)) {
    return null;
  }
  for (const key of keys) {
    const value = record[key];
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
      return record[key];
    }
  }
  return undefined;
}

function diagnosticTextValue(value) {
  if (typeof value === "string") {
    return /\bdiagnostic\b/iu.test(value);
  }
  if (Array.isArray(value)) {
    return value.some((entry) => diagnosticTextValue(entry));
  }
  return false;
}

function diagnosticFlagReason(record, pathName) {
  if (!isRecord(record)) {
    return "";
  }
  for (const key of DIAGNOSTIC_FLAG_KEYS) {
    if (record[key] === true) {
      return `${pathName}.${key}=true`;
    }
  }
  for (const key of DIAGNOSTIC_TEXT_KEYS) {
    if (diagnosticTextValue(record[key])) {
      return `${pathName}.${key} mentions diagnostic verifier material`;
    }
  }
  return "";
}

function normalizeNonEmptyText(value, label) {
  const normalized = trim(value);
  if (!normalized) {
    throw new Error(`${label} is required.`);
  }
  return normalized;
}

function normalizeCanonicalAssetDefinitionId(value, label) {
  const normalized = normalizeNonEmptyText(value, label);
  if (normalized.includes("#") || normalized.toLowerCase() === "xor") {
    throw new Error(`${label} must be a canonical Base58 asset definition ID, not an alias.`);
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
  const source = value === undefined || value === null || value === "" ? fallback : value;
  const parsed = typeof source === "number" ? source : Number(source);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive safe integer.`);
  }
  return parsed;
}

function optionEnabled(options, key, fallback = false) {
  if (options[key] === undefined || options[key] === null || options[key] === "") {
    return fallback;
  }
  if (options[key] === "true") return true;
  if (options[key] === "false") return false;
  throw new Error(`--${key} must be true or false.`);
}

function secretLikeTextReason(value, pathName) {
  const normalized = value.trim().replace(/\s+/gu, " ");
  if (PRIVATE_KEY_PEM_PATTERN.test(normalized)) {
    return `${pathName} must not contain private key material.`;
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

export function unsafeSecretReason(value, pathName = "deployment evidence", seen = new WeakSet()) {
  if (typeof value === "string") {
    return secretLikeTextReason(value, pathName);
  }
  if (Array.isArray(value)) {
    if (seen.has(value)) {
      return "";
    }
    seen.add(value);
    for (const [index, child] of value.entries()) {
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
  for (const [key, child] of Object.entries(value)) {
    if (SECRET_KEY_PATTERN.test(key)) {
      return `${pathName}.${key} must not contain private key material.`;
    }
    const reason = unsafeSecretReason(child, `${pathName}.${key}`, seen);
    if (reason) {
      return reason;
    }
  }
  return "";
}

export function validateBscReadbackEvidence({ addresses, readback, bindingHash, verifierCodeHash, verifierKeyHash }) {
  if (!isRecord(readback)) {
    throw new Error("BSC contract readback must be an object.");
  }
  if (String(readback.chainIdHex).toLowerCase() !== BSC_TESTNET_CHAIN_ID_HEX) {
    throw new Error("BSC contract readback must report BSC testnet chain id 0x61.");
  }
  const codePresent = isRecord(readback.codePresent) ? readback.codePresent : {};
  for (const key of ["token", "bridge", "sourceBridge", "verifier"]) {
    if (codePresent[key] !== true) {
      throw new Error(`BSC contract readback must confirm ${key} bytecode.`);
    }
  }
  if (normalizeEvmAddress(readback.tokenBridgeAddress, "tokenBridgeAddress") !== addresses.bridge) {
    throw new Error("BSC readback token bridge does not match route bridge.");
  }
  if (readback.tokenBridgeLocked !== true) {
    throw new Error("BSC readback token bridge must be locked.");
  }
  if (normalizeEvmAddress(readback.sourceBridgeOwner, "sourceBridgeOwner") !== addresses.bridge) {
    throw new Error("BSC readback source bridge owner does not match route bridge.");
  }
  if (normalizeHex32(readback.bridgeDestinationBindingHash, "bridgeDestinationBindingHash") !== bindingHash) {
    throw new Error("BSC readback bridge destination binding hash does not match.");
  }
  if (normalizeEvmAddress(readback.bridgeVerifierAddress, "bridgeVerifierAddress") !== addresses.verifier) {
    throw new Error("BSC readback bridge verifier address does not match verifier.");
  }
  if (normalizeHex32(readback.bridgeVerifierCodeHash, "bridgeVerifierCodeHash") !== verifierCodeHash) {
    throw new Error("BSC readback bridge verifier code hash does not match.");
  }
  if (normalizeHex32(readback.bridgeVerifierKeyHash, "bridgeVerifierKeyHash") !== verifierKeyHash) {
    throw new Error("BSC readback bridge verifier key hash does not match.");
  }
  if (normalizeHex32(readback.bridgeNetworkId, "bridgeNetworkId") !== BSC_TESTNET_NETWORK_ID_HEX) {
    throw new Error("BSC readback bridge network id must be BSC testnet.");
  }
  if (readback.bridgeSourceDomain !== SCCP_DOMAIN_SORA || readback.bridgeTargetDomain !== SCCP_DOMAIN_BSC) {
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

async function readJson(pathName, label = "JSON file") {
  let text;
  try {
    text = await readFile(resolve(pathName), "utf8");
  } catch (error) {
    throw new Error(`${label} could not be read: ${error.message}`);
  }
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${error.message}`);
  }
}

async function readText(pathName, label = "text file") {
  try {
    return await readFile(resolve(pathName), "utf8");
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

async function compileBscContracts({ writeOut = null } = {}) {
  const solc = requireOptionalPackage("solc");
  const sources = {};
  for (const [key, sourcePath] of Object.entries(CONTRACT_SOURCES)) {
    sources[key] = { content: await readFile(sourcePath, "utf8") };
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
    throw new Error(fatal.map((entry) => entry.formattedMessage ?? entry.message).join("\n"));
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
      bytecodeSha256: bytesToHex(sha256(hexToBytes(contract.evm.bytecode.object, `${definition.contract} bytecode`, null, { allowZero: false }))),
      deployedBytecodeSha256: bytesToHex(
        sha256(hexToBytes(contract.evm.deployedBytecode.object, `${definition.contract} deployed bytecode`, null, { allowZero: false })),
      ),
    };
  }
  if (writeOut) {
    for (const [key, artifact] of Object.entries(artifacts)) {
      await writeJsonNoSecrets(resolve(writeOut, `${key}.json`), artifact);
    }
  }
  return { artifacts, warnings: errors.filter((entry) => entry.severity !== "error") };
}

async function deployContract(ethers, signer, artifact, args) {
  const factory = new ethers.ContractFactory(artifact.abi, artifact.bytecode, signer);
  const contract = await factory.deploy(...args);
  const receipt = await contract.deploymentTransaction().wait();
  return { contract, address: normalizeEvmAddress(await contract.getAddress()), txHash: receipt.hash };
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

async function fetchReadback(ethers, provider, { tokenAddress, bridgeAddress, sourceBridgeAddress, verifierAddress }) {
  const addresses = {
    token: normalizeEvmAddress(tokenAddress, "token address"),
    bridge: normalizeEvmAddress(bridgeAddress, "bridge address"),
    sourceBridge: normalizeEvmAddress(sourceBridgeAddress, "source bridge address"),
    verifier: normalizeEvmAddress(verifierAddress, "verifier address"),
  };
  const token = new ethers.Contract(addresses.token, TOKEN_ABI, provider);
  const sourceBridge = new ethers.Contract(addresses.sourceBridge, SOURCE_BRIDGE_ABI, provider);
  const bridge = new ethers.Contract(addresses.bridge, ROUTE_BRIDGE_ABI, provider);
  const [
    network,
    codePresent,
    tokenBridgeAddress,
    tokenBridgeLocked,
    sourceBridgeOwner,
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
    bridgeDestinationBindingHash: normalizeHex32(bridgeDestinationBindingHash),
    bridgeVerifierAddress: normalizeEvmAddress(bridgeVerifierAddress),
    bridgeVerifierCodeHash: normalizeHex32(bridgeVerifierCodeHash),
    bridgeVerifierKeyHash: normalizeHex32(bridgeVerifierKeyHash),
    bridgeNetworkId: normalizeHex32(bridgeNetworkId),
    bridgeSourceDomain: Number(bridgeSourceDomain),
    bridgeTargetDomain: Number(bridgeTargetDomain),
  };
}

export function buildDeploymentEvidence({
  tokenAddress,
  bridgeAddress,
  sourceBridgeAddress,
  verifierAddress,
  verifierCodeHash,
  verifierKeyHash,
  readback,
}) {
  const addresses = {
    token: normalizeEvmAddress(tokenAddress, "token address"),
    bridge: normalizeEvmAddress(bridgeAddress, "bridge address"),
    sourceBridge: normalizeEvmAddress(sourceBridgeAddress, "source bridge address"),
    verifier: normalizeEvmAddress(verifierAddress, "verifier address"),
  };
  if (new Set(Object.values(addresses)).size !== Object.keys(addresses).length) {
    throw new Error("BSC deployment token, bridge, source bridge, and verifier addresses must be distinct.");
  }
  const codeHash = normalizeHex32(verifierCodeHash, "verifierCodeHash");
  const keyHash = normalizeHex32(verifierKeyHash, "verifierKeyHash");
  const bindingHash = bscDestinationBindingHash({
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
  });
  const bindingKey = bscDestinationBindingKey({
    verifierAddress: addresses.verifier,
    bridgeAddress: addresses.bridge,
    verifierCodeHash: codeHash,
    verifierKeyHash: keyHash,
  });
  return {
    schema: DEPLOYMENT_EVIDENCE_SCHEMA,
    routeId: ROUTE_ID,
    assetKey: ASSET_KEY,
    bscNetwork: "testnet",
    chain: "bsc-testnet",
    chainIdHex: BSC_TESTNET_CHAIN_ID_HEX,
    networkIdHex: BSC_TESTNET_NETWORK_ID_HEX,
    bscBridgeAddress: addresses.bridge,
    bscTokenAddress: addresses.token,
    sccpBscSourceBridgeAddress: addresses.sourceBridge,
    bscVerifierAddress: addresses.verifier,
    destinationRollout: {
      version: 1,
      destinationNetworkId: BSC_TESTNET_NETWORK_ID_HEX,
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
      networkIdHex: BSC_TESTNET_NETWORK_ID_HEX,
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

function normalizeVerifierKeyRefText(value, label) {
  const normalized = normalizeNonEmptyText(value, label);
  if (!/^[A-Za-z0-9][A-Za-z0-9._:/-]{0,127}$/u.test(normalized)) {
    throw new Error(`${label} contains unsupported characters.`);
  }
  return normalized;
}

function normalizeBscTestnetKey(value, label) {
  const normalized = trim(value || "testnet").toLowerCase();
  if (
    normalized === "testnet" ||
    normalized === "bsc-testnet" ||
    normalized === "chapel" ||
    normalized === "bsc-chapel"
  ) {
    return "testnet";
  }
  throw new Error(`${label} must be BSC testnet.`);
}

function readRequiredString(record, keys, label) {
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
    const value = record[key];
    if (typeof value !== "string" || !value.trim()) {
      continue;
    }
    const normalized = value.trim();
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

function readRequiredConsistentString(record, keys, label) {
  const value = readConsistentString(record, keys, label);
  if (!value) {
    throw new Error(`${label} is required.`);
  }
  return value;
}

function routeConfigRequiredRecord(value, label) {
  if (!isRecord(value)) {
    throw new Error(`${label} must be an object.`);
  }
  return value;
}

function normalizeRouteManifestForConfig(manifest) {
  const record = routeConfigRequiredRecord(manifest, "route manifest");
  if (record.schema !== ROUTE_MANIFEST_SCHEMA) {
    throw new Error(`route manifest schema must be ${ROUTE_MANIFEST_SCHEMA}.`);
  }
  const reason = unsafeSecretReason(record, "route manifest");
  if (reason) {
    throw new Error(reason);
  }

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
  const vkRef = routeConfigRequiredRecord(
    readFirstRecord(burnRecord, "vkRef", "vk_ref"),
    "route manifest tairaXorBurnRecord.vkRef",
  );
  const settlement = routeConfigRequiredRecord(
    readFirstRecord(record, "settlement"),
    "route manifest settlement",
  );

  const routeId = readRequiredString(record, ["routeId", "route_id"], "route manifest routeId");
  if (routeId !== ROUTE_ID) {
    throw new Error(`route manifest routeId must be ${ROUTE_ID}.`);
  }
  const assetKey = readRequiredString(record, ["assetKey", "asset_key"], "route manifest assetKey");
  if (assetKey !== ASSET_KEY) {
    throw new Error(`route manifest assetKey must be ${ASSET_KEY}.`);
  }

  const bscNetwork = normalizeBscTestnetKey(
    readFirstString(record, "bscNetwork", "bsc_network", "network") ||
      readFirstString(record, "chain") ||
      "testnet",
    "route manifest bscNetwork",
  );
  const chain = readRequiredString(record, ["chain"], "route manifest chain").toLowerCase();
  if (chain !== "bsc-testnet") {
    throw new Error("route manifest chain must be bsc-testnet.");
  }
  const chainIdHex = readRequiredString(
    record,
    ["chainIdHex", "chain_id_hex"],
    "route manifest chainIdHex",
  ).toLowerCase();
  if (chainIdHex !== BSC_TESTNET_CHAIN_ID_HEX) {
    throw new Error("route manifest chainIdHex must be BSC testnet 0x61.");
  }
  const networkIdHex = normalizeHex32(
    readFirstString(record, "networkIdHex", "network_id_hex") ||
      readFirstString(destinationRollout, "destinationNetworkId", "destination_network_id") ||
      readFirstString(destinationBinding, "networkIdHex", "network_id_hex"),
    "route manifest networkIdHex",
  );
  if (networkIdHex !== BSC_TESTNET_NETWORK_ID_HEX) {
    throw new Error("route manifest networkIdHex must be BSC testnet.");
  }

  const counterpartyDomain = normalizeUint32(
    readFirstValue(record, "counterpartyDomain", "counterparty_domain"),
    "route manifest counterpartyDomain",
  );
  if (counterpartyDomain !== SCCP_DOMAIN_BSC) {
    throw new Error("route manifest counterpartyDomain must be BSC domain 2.");
  }
  const sourceDomain = normalizeUint32(
    readFirstValue(destinationRollout, "sourceDomain", "source_domain") ??
      readFirstValue(destinationBinding, "sourceDomain", "source_domain") ??
      SCCP_DOMAIN_SORA,
    "route manifest sourceDomain",
  );
  const targetDomain = normalizeUint32(
    readFirstValue(destinationRollout, "targetDomain", "target_domain") ??
      readFirstValue(destinationBinding, "targetDomain", "target_domain") ??
      SCCP_DOMAIN_BSC,
    "route manifest targetDomain",
  );
  if (sourceDomain !== SCCP_DOMAIN_SORA || targetDomain !== SCCP_DOMAIN_BSC) {
    throw new Error("route manifest destination rollout domains must be SORA -> BSC.");
  }

  const verifierTarget = readRequiredString(
    record,
    ["verifierTarget", "verifier_target"],
    "route manifest verifierTarget",
  );
  if (verifierTarget !== "EvmContract") {
    throw new Error("route manifest verifierTarget must be EvmContract.");
  }
  const verifierBackend =
    readFirstString(destinationRollout, "verifierBackend", "verifier_backend") ||
    BSC_EVM_GROTH16_BACKEND;
  if (verifierBackend !== BSC_EVM_GROTH16_BACKEND) {
    throw new Error(`route manifest verifier backend must be ${BSC_EVM_GROTH16_BACKEND}.`);
  }
  const proofFamily =
    readFirstString(destinationRollout, "proofFamily", "proof_family") ||
    SCCP_PROOF_FAMILY_STARK_FRI;
  if (proofFamily !== SCCP_PROOF_FAMILY_STARK_FRI) {
    throw new Error(`route manifest proof family must be ${SCCP_PROOF_FAMILY_STARK_FRI}.`);
  }

  if (record.productionReady !== true && record.productionReady !== false) {
    throw new Error("route manifest productionReady must be true or false.");
  }
  const productionReady = record.productionReady === true;
  const tokenAddress = normalizeEvmAddress(
    readRequiredString(
      record,
      [
        "bscTokenAddress",
        "bsc_token_address",
        "tairaXorTokenAddress",
        "taira_xor_token_address",
        "tokenAddress",
        "token_address",
      ],
      "route manifest BSC token address",
    ),
    "route manifest BSC token address",
  );
  const bridgeAddress = normalizeEvmAddress(
    readRequiredString(
      record,
      [
        "bscBridgeAddress",
        "bsc_bridge_address",
        "tairaXorBridgeAddress",
        "taira_xor_bridge_address",
        "bridgeAddress",
        "bridge_address",
      ],
      "route manifest BSC bridge address",
    ),
    "route manifest BSC bridge address",
  );
  const sourceBridgeAddress = normalizeEvmAddress(
    readRequiredConsistentString(
      record,
      [
        "sccpBscSourceBridgeAddress",
        "sccp_bsc_source_bridge_address",
        "bscSourceBridgeAddress",
        "bsc_source_bridge_address",
        "sccpTronSourceBridgeAddress",
        "sccp_tron_source_bridge_address",
        "sourceBridgeAddress",
        "source_bridge_address",
      ],
      "route manifest BSC source bridge address",
    ),
    "route manifest BSC source bridge address",
  );
  const verifierAddressSource =
    readConsistentString(
      record,
      [
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
        "tronVerifierAddress",
        "tron_verifier_address",
      ],
      "route manifest BSC verifier address",
    ) || readFirstString(destinationRollout, "verifierIdentity", "verifier_identity");
  const verifierAddress = normalizeEvmAddress(
    normalizeNonEmptyText(verifierAddressSource, "route manifest BSC verifier address"),
    "route manifest BSC verifier address",
  );
  if (
    new Set([tokenAddress, bridgeAddress, sourceBridgeAddress, verifierAddress]).size !== 4
  ) {
    throw new Error("route manifest BSC token, bridge, source bridge, and verifier addresses must be distinct.");
  }

  const rolloutVerifierIdentity = readFirstString(
    destinationRollout,
    "verifierIdentity",
    "verifier_identity",
  );
  if (rolloutVerifierIdentity && normalizeEvmAddress(rolloutVerifierIdentity) !== verifierAddress) {
    throw new Error("route manifest destinationRollout.verifierIdentity does not match BSC verifier address.");
  }
  const rolloutBridgeAddress = readFirstString(
    destinationRollout,
    "destinationBridgeAddress",
    "destination_bridge_address",
  );
  if (rolloutBridgeAddress && normalizeEvmAddress(rolloutBridgeAddress) !== bridgeAddress) {
    throw new Error("route manifest destinationRollout.destinationBridgeAddress does not match BSC bridge address.");
  }

  const verifierCodeHash = normalizeHex32(
    readFirstString(record, "verifierCodeHash", "verifier_code_hash") ||
      readFirstString(destinationRollout, "verifierCodeHash", "verifier_code_hash"),
    "route manifest verifierCodeHash",
  );
  const verifierKeyHash = normalizeHex32(
    readFirstString(record, "verifierKeyHash", "verifier_key_hash") ||
      readFirstString(destinationRollout, "verifierKeyHash", "verifier_key_hash"),
    "route manifest verifierKeyHash",
  );
  const optionalRouteHash = (label, ...keys) => {
    const value =
      readFirstString(record, ...keys) || readFirstString(destinationRollout, ...keys);
    return value ? normalizeHex32(value, label) : null;
  };
  const proofArtifactHash = optionalRouteHash(
    "route manifest proofArtifactHash",
    "proofArtifactHash",
    "proof_artifact_hash",
    "proverArtifactHash",
    "prover_artifact_hash",
    "circuitArtifactHash",
    "circuit_artifact_hash",
  );
  const provingKeyHash = optionalRouteHash(
    "route manifest provingKeyHash",
    "provingKeyHash",
    "proving_key_hash",
  );
  if (Boolean(proofArtifactHash) !== Boolean(provingKeyHash)) {
    throw new Error("route manifest proofArtifactHash and provingKeyHash must be supplied together.");
  }
  const diagnosticVerifierReasons = [
    diagnosticFlagReason(record, "route manifest"),
    diagnosticFlagReason(destinationRollout, "route manifest destinationRollout"),
    diagnosticFlagReason(destinationBinding, "route manifest destinationBinding"),
    isKnownDiagnosticBscVerifierKeyHash(verifierKeyHash)
      ? `verifierKeyHash=${verifierKeyHash} is a known diagnostic BSC verifier key hash`
      : "",
  ].filter(Boolean);
  if (productionReady && diagnosticVerifierReasons.length > 0) {
    throw new Error(
      `route manifest productionReady cannot be true with diagnostic BSC verifier material: ${diagnosticVerifierReasons.join("; ")}.`,
    );
  }
  if (productionReady && (!proofArtifactHash || !provingKeyHash)) {
    throw new Error("route manifest productionReady requires proofArtifactHash and provingKeyHash.");
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
  const destinationBindingKey =
    readFirstString(record, "destinationBindingKey", "destination_binding_key") ||
    readFirstString(destinationRollout, "destinationBindingKey", "destination_binding_key") ||
    readFirstString(destinationBinding, "key", "destinationBindingKey", "destination_binding_key");
  if (destinationBindingKey !== expectedBindingKey) {
    throw new Error("route manifest destination binding key does not match BSC deployment evidence.");
  }
  const expectedBindingHash = bscDestinationBindingHash({
    networkId: networkIdHex,
    verifierAddress,
    bridgeAddress,
    verifierCodeHash,
    verifierKeyHash,
  });
  const destinationBindingHash = normalizeHex32(
    readFirstString(record, "destinationBindingHash", "destination_binding_hash") ||
      readFirstString(destinationRollout, "destinationBindingHash", "destination_binding_hash") ||
      readFirstString(destinationBinding, "bindingHash", "binding_hash"),
    "route manifest destination binding hash",
  );
  if (destinationBindingHash !== expectedBindingHash) {
    throw new Error("route manifest destination binding hash does not match BSC deployment evidence.");
  }
  roleSeparatedHashes[2][1] = destinationBindingHash;
  const seenRouteHashes = new Map();
  for (const [label, value] of roleSeparatedHashes.filter(([, value]) => Boolean(value))) {
    const previous = seenRouteHashes.get(value);
    if (previous) {
      throw new Error(`route manifest ${label} must not equal ${previous}.`);
    }
    seenRouteHashes.set(value, label);
  }

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
  const declaredArtifactSha256 = normalizeHex32(
    readFirstString(burnRecord, "artifactSha256", "artifact_sha256"),
    "route manifest tairaXorBurnRecord.artifactSha256",
  );
  if (declaredArtifactSha256 !== artifactSha256) {
    throw new Error("route manifest TAIRA burn-record artifact sha256 does not match artifact bytes.");
  }
  const settlementAssetDefinitionId = normalizeCanonicalAssetDefinitionId(
    readFirstString(
      burnRecord,
      "settlementAssetDefinitionId",
      "settlement_asset_definition_id",
    ),
    "route manifest tairaXorBurnRecord.settlementAssetDefinitionId",
  );
  const gasLimit = normalizePositiveSafeInteger(
    readFirstValue(burnRecord, "gasLimit", "gas_limit"),
    "route manifest burn-record gasLimit",
  );
  const settlementRouteId = readFirstString(settlement, "routeId", "route_id");
  const settlementAssetKey = readFirstString(settlement, "assetKey", "asset_key");
  if (settlementRouteId && settlementRouteId !== ROUTE_ID) {
    throw new Error(`route manifest settlement.routeId must be ${ROUTE_ID}.`);
  }
  if (settlementAssetKey && settlementAssetKey !== ASSET_KEY) {
    throw new Error(`route manifest settlement.assetKey must be ${ASSET_KEY}.`);
  }

  const postDeployLiveEvidence =
    readFirstRecord(record, "postDeployLiveEvidence", "post_deploy_live_evidence") ?? null;
  const normalizeOptionalPostDeployHash = (keys, label) => {
    if (!postDeployLiveEvidence) {
      return null;
    }
    return normalizeHex32(readFirstString(postDeployLiveEvidence, ...keys), label);
  };
  const explicitDisabledReason =
    record.disabledReason === undefined && record.disabled_reason === undefined
      ? null
      : normalizeNonEmptyText(
          readFirstValue(record, "disabledReason", "disabled_reason"),
          "route manifest disabledReason",
        );

  return {
    version: normalizeUint32(readFirstValue(record, "version") ?? 1, "route manifest version"),
    routeId,
    assetKey,
    bscNetwork,
    legacyTronNetwork: "bsc-testnet",
    chain,
    chainIdHex,
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
    destinationBindingKey,
    destinationBindingHash,
    settlementAssetDefinitionId,
    contractArtifactB64: artifact.text,
    artifactSha256,
    codeHash: normalizeHex32(
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
    settlementContractAddress:
      readFirstValue(settlement, "contractAddress", "contract_address") === undefined ||
      readFirstValue(settlement, "contractAddress", "contract_address") === null
        ? null
        : normalizeNonEmptyText(
            readFirstValue(settlement, "contractAddress", "contract_address"),
            "route manifest settlement.contractAddress",
          ),
    settlementContractAlias:
      readFirstValue(settlement, "contractAlias", "contract_alias") === undefined ||
      readFirstValue(settlement, "contractAlias", "contract_alias") === null
        ? null
        : normalizeNonEmptyText(
            readFirstValue(settlement, "contractAlias", "contract_alias"),
            "route manifest settlement.contractAlias",
          ),
    postDeployLiveEvidence: postDeployLiveEvidence
      ? {
          fullTomlReady:
            readFirstValue(postDeployLiveEvidence, "fullTomlReady", "full_toml_ready") === true,
          sourceBridgeConfigHash: normalizeOptionalPostDeployHash(
            ["sourceBridgeConfigHash", "source_bridge_config_hash"],
            "route manifest postDeployLiveEvidence.sourceBridgeConfigHash",
          ),
          sourceEventTransactionId: normalizeOptionalPostDeployHash(
            ["sourceEventTransactionId", "source_event_transaction_id"],
            "route manifest postDeployLiveEvidence.sourceEventTransactionId",
          ),
          routeCanaryEvidenceHash: normalizeOptionalPostDeployHash(
            ["routeCanaryEvidenceHash", "route_canary_evidence_hash"],
            "route manifest postDeployLiveEvidence.routeCanaryEvidenceHash",
          ),
          routeCanaryTransactionId: normalizeOptionalPostDeployHash(
            ["routeCanaryTransactionId", "route_canary_transaction_id"],
            "route manifest postDeployLiveEvidence.routeCanaryTransactionId",
          ),
          offlineFullTomlSha256: readFirstString(
            postDeployLiveEvidence,
            "offlineFullTomlSha256",
            "offline_full_toml_sha256",
          )
            ? normalizeHex32(
                readFirstString(
                  postDeployLiveEvidence,
                  "offlineFullTomlSha256",
                  "offline_full_toml_sha256",
                ),
                "route manifest postDeployLiveEvidence.offlineFullTomlSha256",
              )
            : null,
        }
      : null,
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
  const allowUnready = optionEnabled(options, "allow-unready", !route.productionReady);
  if (!route.productionReady && !allowUnready) {
    throw new Error("non-production route manifests require --allow-unready true.");
  }
  const lines = [
    "# Generated by scripts/sccp_bsc_taira_xor_deploy.mjs route-config.",
    "# Merge this overlay into the TAIRA Torii/Iroha runtime config for BSC testnet smoke.",
    "# Generic BSC address fields are emitted with legacy TRON mirrors for mixed-version nodes.",
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
    `counterparty_domain = ${route.counterpartyDomain}`,
    `verifier_target = ${tomlString(route.verifierTarget, "verifier_target")}`,
    `production_ready = ${route.productionReady ? "true" : "false"}`,
    ...tomlOptionalStringLine("disabled_reason", route.disabledReason, "disabled_reason"),
    `network_id_hex = ${tomlString(route.networkIdHex, "network_id_hex")}`,
    "# Generic address fields are preferred; legacy TRON field names mirror the same BSC EVM addresses.",
    `taira_xor_token_address = ${tomlString(route.tokenAddress, "taira_xor_token_address")}`,
    `taira_xor_bridge_address = ${tomlString(route.bridgeAddress, "taira_xor_bridge_address")}`,
    `source_bridge_address = ${tomlString(route.sourceBridgeAddress, "source_bridge_address")}`,
    `sccp_bsc_source_bridge_address = ${tomlString(route.sourceBridgeAddress, "sccp_bsc_source_bridge_address")}`,
    `bsc_source_bridge_address = ${tomlString(route.sourceBridgeAddress, "bsc_source_bridge_address")}`,
    `sccp_tron_source_bridge_address = ${tomlString(route.sourceBridgeAddress, "sccp_tron_source_bridge_address")}`,
    `destination_verifier_address = ${tomlString(route.verifierAddress, "destination_verifier_address")}`,
    `verifier_address = ${tomlString(route.verifierAddress, "verifier_address")}`,
    `sccp_bsc_destination_verifier_address = ${tomlString(route.verifierAddress, "sccp_bsc_destination_verifier_address")}`,
    `bsc_verifier_address = ${tomlString(route.verifierAddress, "bsc_verifier_address")}`,
    `evm_verifier_address = ${tomlString(route.verifierAddress, "evm_verifier_address")}`,
    `tron_verifier_address = ${tomlString(route.verifierAddress, "tron_verifier_address")}`,
    `verifier_code_hash = ${tomlString(route.verifierCodeHash, "verifier_code_hash")}`,
    `verifier_key_hash = ${tomlString(route.verifierKeyHash, "verifier_key_hash")}`,
    ...tomlOptionalStringLine(
      "proof_artifact_hash",
      route.proofArtifactHash,
      "proof_artifact_hash",
    ),
    ...tomlOptionalStringLine(
      "prover_artifact_hash",
      route.proofArtifactHash,
      "prover_artifact_hash",
    ),
    ...tomlOptionalStringLine(
      "circuit_artifact_hash",
      route.proofArtifactHash,
      "circuit_artifact_hash",
    ),
    ...tomlOptionalStringLine("proving_key_hash", route.provingKeyHash, "proving_key_hash"),
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
      `post_deploy_route_canary_evidence_hash = ${tomlString(route.postDeployLiveEvidence.routeCanaryEvidenceHash, "post_deploy_route_canary_evidence_hash")}`,
      `post_deploy_route_canary_transaction_id = ${tomlString(route.postDeployLiveEvidence.routeCanaryTransactionId, "post_deploy_route_canary_transaction_id")}`,
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
    .filter((line) => !/^\s*sccp_allow_unready_transparent_proofs\s*=/u.test(line));
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
  if (!parseBoolean(options.broadcast)) {
    throw new Error("deploy requires --broadcast true and --confirm-testnet taira_bsc_xor.");
  }
  if (options["confirm-testnet"] !== CONFIRMATION_TEXT) {
    throw new Error(`deploy requires --confirm-testnet ${CONFIRMATION_TEXT}.`);
  }
  if (!options.verifier) {
    throw new Error("deploy requires --verifier <verifier-key.json>.");
  }
  const privateKeyEnv = options["private-key-env"] ?? DEFAULT_PRIVATE_KEY_ENV;
  const privateKey = normalizePrivateKey(process.env[privateKeyEnv], privateKeyEnv);
  const rpcUrl = normalizeBscRpcUrl(options["rpc-url"] ?? DEFAULT_BSC_RPC_URL, {
    allowLocal: parseBoolean(options["allow-local-rpc"]),
  });
  const ethers = requireOptionalPackage("ethers");
  const provider = new ethers.JsonRpcProvider(rpcUrl, 97);
  const network = await provider.getNetwork();
  if (network.chainId !== 97n) {
    throw new Error(`BSC RPC must report chain id 97; received ${network.chainId}.`);
  }
  const wallet = new ethers.Wallet(privateKey, provider);
  const signer = new ethers.NonceManager(wallet);
  const verifierMaterial = normalizeVerifierMaterial(await readJson(options.verifier));
  if (
    verifierMaterial.diagnosticVerifierReasons.length > 0 &&
    !parseBoolean(options["allow-diagnostic-verifier"])
  ) {
    throw new Error(
      `deploy refuses diagnostic BSC verifier material without --allow-diagnostic-verifier true: ${verifierMaterial.diagnosticVerifierReasons.join("; ")}.`,
    );
  }
  const { artifacts } = await compileBscContracts();
  const verifierArgs = [
    verifierMaterial.alpha1,
    verifierMaterial.beta2,
    verifierMaterial.gamma2,
    verifierMaterial.delta2,
    verifierMaterial.ic,
  ];
  const verifier = await deployContract(ethers, signer, artifacts.verifier, verifierArgs);
  const verifierCodeHash = normalizeHex32(ethers.keccak256(await provider.getCode(verifier.address)));
  const sourceBridge = await deployContract(ethers, signer, artifacts.sourceBridge, [
    BSC_TESTNET_NETWORK_ID_HEX,
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_SORA,
  ]);
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
    BSC_TESTNET_NETWORK_ID_HEX,
    SCCP_DOMAIN_SORA,
    SCCP_DOMAIN_BSC,
    routeIdHash,
    assetKeyHash,
  ]);
  const tokenContract = new ethers.Contract(token.address, TOKEN_ABI, signer);
  const sourceBridgeContract = new ethers.Contract(sourceBridge.address, SOURCE_BRIDGE_ABI, signer);
  const setBridgeTx = await tokenContract.setBridge(bridge.address);
  const setBridgeReceipt = await setBridgeTx.wait();
  const lockBridgeTx = await tokenContract.lockBridge();
  const lockBridgeReceipt = await lockBridgeTx.wait();
  const transferSourceOwnerTx = await sourceBridgeContract.transferOwnership(bridge.address);
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
  });
  const out = resolve(options.out ?? DEFAULT_EVIDENCE_OUT);
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
  for (const key of ["token", "bridge", "source-bridge", "verifier"]) {
    if (!options[key]) {
      throw new Error(`evidence requires --${key} <address>.`);
    }
  }
  const rpcUrl = normalizeBscRpcUrl(options["rpc-url"] ?? DEFAULT_BSC_RPC_URL, {
    allowLocal: parseBoolean(options["allow-local-rpc"]),
  });
  const ethers = requireOptionalPackage("ethers");
  const provider = new ethers.JsonRpcProvider(rpcUrl, 97);
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
  });
  const out = resolve(options.out ?? DEFAULT_EVIDENCE_OUT);
  await writeJsonNoSecrets(out, evidence);
  return { ok: true, wrote: out, destinationBindingHash: evidence.destinationRollout.destinationBindingHash };
}

async function commandRouteConfig(options) {
  const manifest = await readJson(options.manifest ?? DEFAULT_ROUTE_MANIFEST_OUT, "BSC route manifest");
  const baseConfigPath = options["base-config"] ?? null;
  const toml = baseConfigPath
    ? buildMergedBscTairaXorRouteConfigToml(
        await readText(baseConfigPath, "base TAIRA config"),
        manifest,
        options,
      )
    : buildBscTairaXorRouteConfigToml(manifest, options);
  const out = await writeTextNoSecrets(
    options.out ??
      (baseConfigPath ? DEFAULT_ROUTE_FULL_CONFIG_OUT : DEFAULT_ROUTE_CONFIG_OUT),
    toml,
    0o644,
  );
  return {
    ok: true,
    wrote: out,
    mode: baseConfigPath ? "merged-full-config" : "overlay",
    baseConfig: baseConfigPath ? resolve(baseConfigPath) : null,
    routeId: manifest.routeId ?? manifest.route_id,
    assetKey: manifest.assetKey ?? manifest.asset_key,
    productionReady: manifest.productionReady ?? manifest.production_ready,
    allowUnready: optionEnabled(
      options,
      "allow-unready",
      (manifest.productionReady ?? manifest.production_ready) !== true,
    ),
    nextStep: baseConfigPath
      ? "Deploy this merged TAIRA node config on every public validator and restart Torii/Iroha before rerunning the BSC SCCP preflight without --manifest-file."
      : "Merge this TOML into the TAIRA node config and redeploy/restart Torii before rerunning the BSC SCCP preflight.",
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
    codePresent: { token: true, bridge: true, sourceBridge: true, verifier: true },
    tokenBridgeAddress: bridgeAddress,
    tokenBridgeLocked: true,
    sourceBridgeOwner: bridgeAddress,
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
  if (!unsafeSecretReason({ public: "ok" }) && unsafeSecretReason({ private_key: "0x1" })) {
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
    case "route-config":
      return commandRouteConfig(options);
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
