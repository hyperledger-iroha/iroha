#!/usr/bin/env node
/**
 * Build and publish the TAIRA <-> TON testnet XOR SCCP route manifest.
 *
 * This script does not deploy TON contracts. It consumes public deployment
 * evidence for already-deployed TON testnet contracts, browser proof modules,
 * TON source verifier material, and the TAIRA burn-record contract material,
 * then renders the production route manifest expected by TAIRA admission. Live
 * TAIRA publication is opt-in and requires a runtime-only private-key env var.
 */

import { createHash } from "node:crypto";
import { realpathSync } from "node:fs";
import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { isIP } from "node:net";
import { basename, dirname, resolve } from "node:path";
import { normalizeAccountId } from "../javascript/iroha_js/src/normalizers.js";

const ROUTE_ID = "taira_ton_xor";
const ASSET_KEY = "xor";
const ROUTE_MANIFEST_SCHEMA =
  "iroha-sccp-taira-ton-xor-route-manifest-draft/v1";
const ROUTE_MANIFEST_ISI_SCHEMA = "iroha-sccp-route-manifest-isi/v1";
const TAIRA_CHAIN_ID = "809574f5-fee7-5e69-bfcf-52451e42d50f";
const DEFAULT_TAIRA_TORII_URL = "https://taira.sora.org";
const DEFAULT_TAIRA_ROUTE_MANIFEST_PRIVATE_KEY_ENV =
  "SCCP_TAIRA_ROUTE_MANIFEST_PRIVATE_KEY";
const DEFAULT_COMMIT_TIMEOUT_MS = 120_000;
const DEFAULT_TAIRA_ROUTE_MANIFEST_GAS_LIMIT = 2_000_000;
const DEFAULT_ROUTE_MANIFEST_OUT =
  "artifacts/sccp-ton/testnet-taira-xor-route.manifest.json";
const DEFAULT_ROUTE_MANIFEST_ISI_OUT =
  "artifacts/sccp-ton/testnet-taira-xor-route.upsert-isi.json";
const TON_TESTNET_LEGACY_NETWORK = "testnet";
const TON_TESTNET_CHAIN_ID_HEX =
  "0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd";
const TON_TESTNET_EXPLORER_URL = "https://testnet.tonscan.org";
const TON_DESTINATION_BINDING_KEY = "sccp:0:4:ton:ton-contract-v1:3";
const TON_DESTINATION_BINDING_HASH =
  "0x8651c1b818973f92050f69e66e8491e9681d23db1cb37393b9ea15c5e7e02799";
const DEFAULT_TON_FINALIZE_MESSAGE_VALUE_NANO = "100000000";
const TON_COUNTERPARTY_DOMAIN = 4;
const TON_COUNTERPARTY_ACCOUNT_CODEC = 4;
const TON_COUNTERPARTY_ACCOUNT_CODEC_KEY = "ton_raw";
const TON_VERIFIER_TARGET = "TonContract";
const TAIRA_XOR_SETTLEMENT_ASSET_DEFINITION_ID = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
const TAIRA_BURN_RECORD_VK_BACKEND = "halo2/ipa";
const DEFAULT_TAIRA_BURN_RECORD_VK_NAME = "taira_bsc_xor_burn_record_v1";
const TAIRA_BURN_RECORD_GAS_LIMIT = 2_000_000;
const RETIRED_TON_ROUTE_MANIFEST_FIELDS = Object.freeze([
  ["tron_network", "chain"],
  ["sccp_tron_source_bridge_address", "source_bridge_address"],
  ["tron_verifier_address", "destination_verifier_address"],
]);

function usage(command = "") {
  const common = `Usage:
  node scripts/sccp_ton_taira_xor_deploy.mjs route-manifest --token <0:...> --bridge <0:...> --source-bridge <0:...> --verifier <0:...> --verifier-code-hash <0x...> --verifier-key-hash <0x...> --proof-artifact-hash <0x...> --proving-key-hash <0x...> --deployment-evidence <public-json> --source-verifier-material <public-json> --source-adapter-engine-deployment <public-json> --destination-browser-prover-manifest <public-json> --source-browser-prover-manifest <public-json> --taira-contract <public-json> --post-deploy-source-bridge-config-hash <0x...> --post-deploy-source-event-transaction-id <0x...> --post-deploy-route-canary-evidence-hash <0x...> --post-deploy-route-canary-transaction-id <0x...> --offline-full-toml-evidence <public-json-or-toml> [--vk-name ${DEFAULT_TAIRA_BURN_RECORD_VK_NAME}] [--ton-finalize-message-value-nano ${DEFAULT_TON_FINALIZE_MESSAGE_VALUE_NANO}] [--out ${DEFAULT_ROUTE_MANIFEST_OUT}]
  node scripts/sccp_ton_taira_xor_deploy.mjs publish-route-manifest [--manifest ${DEFAULT_ROUTE_MANIFEST_OUT}] [--out ${DEFAULT_ROUTE_MANIFEST_ISI_OUT}] [--submit true --authority <taira-route-manifest-manager-account> --private-key-env ${DEFAULT_TAIRA_ROUTE_MANIFEST_PRIVATE_KEY_ENV} --torii-url ${DEFAULT_TAIRA_TORII_URL} --chain-id ${TAIRA_CHAIN_ID}] [--gas-asset-id ${TAIRA_XOR_SETTLEMENT_ASSET_DEFINITION_ID}] [--gas-limit ${DEFAULT_TAIRA_ROUTE_MANIFEST_GAS_LIMIT}] [--wait-for-commit true|false] [--commit-timeout-ms ${DEFAULT_COMMIT_TIMEOUT_MS}]

Commands:
  route-manifest          Render a production TON testnet route manifest draft.
  publish-route-manifest  Render the UpsertSccpRouteManifest artifact; optionally submit it.

Notes:
  - TON contract deployment is intentionally external to this script.
  - Runtime private keys are read only from --private-key-env and are never written.
  - Generated manifests are production-ready only when all public evidence is present.
`;
  if (command === "route-manifest" || command === "publish-route-manifest") {
    return common;
  }
  return common;
}

function parseArgs(argv) {
  const [command, ...rest] = argv;
  if (!command || command === "--help" || command === "-h") {
    return { command: "help", options: {} };
  }
  const options = {};
  const setOption = (key, value) => {
    if (Object.hasOwn(options, key)) {
      throw new Error("Option must be specified at most once.");
    }
    options[key] = value;
  };
  for (let index = 0; index < rest.length; index += 1) {
    const token = rest[index];
    if (token === "--help" || token === "-h") {
      setOption("help", true);
      continue;
    }
    if (!token.startsWith("--")) {
      throw new Error("Unexpected positional argument.");
    }
    const equalsIndex = token.indexOf("=");
    if (equalsIndex !== -1) {
      const key = token.slice(2, equalsIndex);
      const value = token.slice(equalsIndex + 1);
      if (key === "help") {
        throw new Error("Help option must not have a value.");
      }
      setOption(key, value);
      continue;
    }
    const key = token.slice(2);
    const next = rest[index + 1];
    if (next === undefined || next.startsWith("--")) {
      throw new Error("Option requires a value.");
    }
    setOption(key, next);
    index += 1;
  }
  return { command, options };
}

const COMMAND_OPTION_ALLOWLISTS = Object.freeze({
  "route-manifest": new Set([
    "token",
    "bridge",
    "source-bridge",
    "verifier",
    "verifier-code-hash",
    "verifier-key-hash",
    "proof-artifact-hash",
    "proving-key-hash",
    "deployment-evidence",
    "source-verifier-material",
    "source-adapter-engine-deployment",
    "destination-browser-prover-manifest",
    "source-browser-prover-manifest",
    "taira-contract",
    "post-deploy-source-bridge-config-hash",
    "post-deploy-source-event-transaction-id",
    "post-deploy-source-event-explorer-url",
    "post-deploy-route-canary-evidence-hash",
    "post-deploy-route-canary-transaction-id",
    "post-deploy-route-canary-explorer-url",
    "offline-full-toml-evidence",
    "offline-full-toml-sha256",
    "vk-name",
    "ton-finalize-message-value-nano",
    "out",
    "help",
  ]),
  "publish-route-manifest": new Set([
    "manifest",
    "out",
    "submit",
    "authority",
    "private-key-env",
    "torii-url",
    "chain-id",
    "gas-asset-id",
    "gas-limit",
    "wait-for-commit",
    "commit-timeout-ms",
    "help",
  ]),
});

function assertKnownCommand(command) {
  if (!Object.hasOwn(COMMAND_OPTION_ALLOWLISTS, command)) {
    throw new Error("Unknown command.");
  }
}

function assertKnownOptions(command, options) {
  const allowlist = COMMAND_OPTION_ALLOWLISTS[command];
  if (!allowlist) {
    return;
  }
  for (const key of Object.keys(options)) {
    if (!allowlist.has(key)) {
      throw new Error(`Unknown option for ${command}.`);
    }
  }
}

function requireOption(options, key) {
  const value = options[key];
  if (typeof value !== "string" || value.trim() === "") {
    throw new Error(`--${key} is required.`);
  }
  return value;
}

function normalizeCliPathOption(value, label, fallback) {
  const candidate = value ?? fallback;
  if (
    typeof candidate !== "string" ||
    candidate.trim() !== candidate ||
    candidate === "" ||
    /[\u0000-\u001f\u007f]/u.test(candidate)
  ) {
    throw new Error(`${label} must be a non-empty path.`);
  }
  return candidate;
}

function optionEnabled(options, key, fallback = false) {
  if (!Object.hasOwn(options, key)) {
    return fallback;
  }
  const value = options[key];
  if (value === undefined) {
    return fallback;
  }
  if (value === "true") return true;
  if (value === "false") return false;
  throw new Error(`--${key} must be true or false.`);
}

const SUBMIT_ONLY_OPTIONS = Object.freeze([
  "authority",
  "private-key-env",
  "torii-url",
  "chain-id",
  "wait-for-commit",
  "commit-timeout-ms",
]);

function assertSubmitOnlyOptionsAbsentWhenNotSubmitting(options) {
  for (const key of SUBMIT_ONLY_OPTIONS) {
    if (Object.hasOwn(options, key)) {
      throw new Error(`--${key} requires --submit true.`);
    }
  }
}

function normalizePositiveInteger(value, label, fallback = null) {
  const candidate = value ?? fallback;
  const text = String(candidate ?? "").trim();
  if (!/^(?:0|[1-9][0-9]*)$/u.test(text)) {
    throw new Error(`${label} must be a positive integer.`);
  }
  const parsed = Number(text);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive integer.`);
  }
  return parsed;
}

function normalizePositiveDecimalString(value, label, fallback = null) {
  const candidate = value ?? fallback;
  if (typeof candidate !== "string") {
    throw new Error(`${label} must be a positive integer decimal string.`);
  }
  if (candidate.trim() !== candidate || !/^[1-9][0-9]*$/u.test(candidate)) {
    throw new Error(`${label} must be a positive integer decimal string.`);
  }
  return candidate;
}

function normalizeHex32(value, label, { nonzero = true } = {}) {
  if (typeof value !== "string") {
    throw new Error(`${label} must be a 0x-prefixed 32-byte hex string.`);
  }
  const text = value.trim();
  if (text !== value || text.startsWith("0X")) {
    throw new Error(`${label} must use canonical lowercase 0x hex.`);
  }
  if (!/^0x[0-9a-f]{64}$/u.test(text)) {
    throw new Error(`${label} must be a 0x-prefixed 32-byte hex string.`);
  }
  if (nonzero && /^0x0{64}$/u.test(text)) {
    throw new Error(`${label} must not be zero.`);
  }
  return text;
}

function normalizeTonRawAddress(value, label) {
  if (typeof value !== "string") {
    throw new Error(`${label} must be a TON raw address.`);
  }
  const text = value.trim();
  if (text !== value || text !== text.toLowerCase()) {
    throw new Error(
      `${label} must be canonical lowercase TON raw address text.`,
    );
  }
  const match = text.match(/^0:([0-9a-f]{64})$/u);
  if (!match) {
    throw new Error(`${label} must use basechain 0:<32-byte-hex> form.`);
  }
  if (/^0{64}$/u.test(match[1])) {
    throw new Error(`${label} account hash must not be zero.`);
  }
  return text;
}

function normalizeCanonicalAssetDefinitionId(value, label) {
  if (typeof value !== "string" || value.trim() !== value) {
    throw new Error(`${label} must be canonical text.`);
  }
  if (!/^[1-9A-HJ-NP-Za-km-z]{16,80}$/u.test(value)) {
    throw new Error(`${label} must be a canonical Base58 asset definition id.`);
  }
  return value;
}

function normalizeStrictBase64(value, label) {
  if (typeof value !== "string" || value.trim() !== value) {
    throw new Error(`${label} must be strict base64.`);
  }
  if (value.length < 8 || value.length % 4 !== 0) {
    throw new Error(`${label} must be strict base64.`);
  }
  if (!/^[A-Za-z0-9+/]+={0,2}$/u.test(value)) {
    throw new Error(`${label} must be strict base64.`);
  }
  const decoded = Buffer.from(value, "base64");
  if (decoded.length < 32 || decoded.length > 8 * 1024 * 1024) {
    throw new Error(`${label} must decode to 32 bytes-8 MiB.`);
  }
  if (decoded.toString("base64") !== value) {
    throw new Error(`${label} must be canonical strict base64.`);
  }
  return value;
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

function canonicalJson(value) {
  return JSON.stringify(stableJsonValue(value));
}

function sha256HexBytes(bytes) {
  return `0x${createHash("sha256").update(bytes).digest("hex")}`;
}

function sha256HexJson(value) {
  return sha256HexBytes(Buffer.from(canonicalJson(value), "utf8"));
}

function requireRecord(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be a JSON object.`);
  }
  return value;
}

function firstRecord(record, ...keys) {
  for (const key of keys) {
    const value = record?.[key];
    if (value && typeof value === "object" && !Array.isArray(value)) {
      return value;
    }
  }
  return null;
}

function firstString(record, ...keys) {
  for (const key of keys) {
    const value = record?.[key];
    if (typeof value === "string" && value.trim() !== "") {
      return value;
    }
  }
  return "";
}

async function readText(path, label) {
  try {
    return await readFile(resolve(path), "utf8");
  } catch (error) {
    throw new Error(`${label} cannot be read: ${error.message}`);
  }
}

async function readJson(path, label) {
  const text = await readText(path, label);
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`${label} must be valid JSON: ${error.message}`);
  }
}

async function sha256HexFileOrJson(path, label) {
  let raw;
  try {
    raw = await readFile(resolve(path));
  } catch (error) {
    throw new Error(`${label} cannot be read: ${error.message}`);
  }
  try {
    return sha256HexJson(JSON.parse(raw.toString("utf8")));
  } catch (_error) {
    return sha256HexBytes(raw);
  }
}

async function writeJsonNoSecrets(path, value) {
  const out = resolve(path);
  await mkdir(dirname(out), { recursive: true });
  await replaceWithTemporaryFile(out, `${JSON.stringify(value, null, 2)}\n`);
  return out;
}

function temporaryOutputPath(out) {
  return `${out}.tmp-${process.pid}-${Date.now()}-${Math.random()
    .toString(16)
    .slice(2)}`;
}

async function replaceWithTemporaryFile(out, value) {
  let lastCollision = null;
  for (let attempt = 0; attempt < 8; attempt += 1) {
    const temp = temporaryOutputPath(out);
    let created = false;
    try {
      await writeFile(temp, value, {
        encoding: "utf8",
        flag: "wx",
        mode: 0o644,
      });
      created = true;
      await rename(temp, out);
      return;
    } catch (error) {
      if (created) {
        await rm(temp, { force: true }).catch(() => {});
      }
      if (error?.code === "EEXIST") {
        lastCollision = error;
        continue;
      }
      throw error;
    }
  }
  throw lastCollision ?? new Error("unable to allocate temporary output file");
}

function canonicalPathForCollision(pathName) {
  const resolved = resolve(pathName);
  if (resolved.includes("\0")) {
    return resolved;
  }
  try {
    return realpathSync(resolved);
  } catch (error) {
    if (error?.code !== "ENOENT" && error?.code !== "ENOTDIR") {
      throw error;
    }
  }
  try {
    return resolve(realpathSync(dirname(resolved)), basename(resolved));
  } catch (error) {
    if (error?.code !== "ENOENT" && error?.code !== "ENOTDIR") {
      throw error;
    }
  }
  return resolved;
}

function assertDistinctResolvedPaths(outputPath, outputLabel, inputPaths) {
  const outputResolved = resolve(outputPath);
  const outputCanonical = canonicalPathForCollision(outputPath);
  for (const [inputPath, inputLabel] of inputPaths) {
    if (typeof inputPath !== "string" || inputPath.trim() === "") {
      continue;
    }
    const inputResolved = resolve(inputPath);
    if (
      outputResolved === inputResolved ||
      outputCanonical === canonicalPathForCollision(inputPath)
    ) {
      throw new Error(
        `${outputLabel} must not be the same path as ${inputLabel}.`,
      );
    }
  }
}

function normalizeUrlHostname(hostname) {
  return hostname
    .replace(/^\[/u, "")
    .replace(/\]$/u, "")
    .toLowerCase();
}

function isLoopbackHost(hostname) {
  const unbracketedHostname = normalizeUrlHostname(hostname);
  return (
    unbracketedHostname === "localhost" ||
    unbracketedHostname.endsWith(".localhost") ||
    unbracketedHostname === "127.0.0.1" ||
    unbracketedHostname === "::1"
  );
}

function isNonPublicDnsHost(hostname) {
  const unbracketedHostname = normalizeUrlHostname(hostname);
  const labels = unbracketedHostname.split(".");
  return (
    isLoopbackHost(unbracketedHostname) ||
    unbracketedHostname.endsWith(".local") ||
    !unbracketedHostname.includes(".") ||
    labels.some(
      (label) =>
        label === "" ||
        !/^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/u.test(label),
    ) ||
    isIP(unbracketedHostname) !== 0
  );
}

function normalizeModuleUrl(value, label) {
  if (
    typeof value !== "string" ||
    value.trim() !== value ||
    value === "" ||
    /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    throw new Error(`${label} must be a deterministic module URL.`);
  }
  if (/[?#\\]/u.test(value) || value.includes("\0")) {
    throw new Error(
      `${label} must not include credentials, query, fragment, or escapes.`,
    );
  }
  if (/^https:\/\//u.test(value)) {
    const url = new URL(value);
    if (url.username || url.password || url.search || url.hash) {
      throw new Error(
        `${label} must not include credentials, query, or fragment.`,
      );
    }
    if (isNonPublicDnsHost(url.hostname)) {
      throw new Error(`${label} HTTPS URLs must use a public DNS host.`);
    }
    return value;
  }
  if (/^http:\/\//u.test(value)) {
    const url = new URL(value);
    if (
      !isLoopbackHost(url.hostname) ||
      url.username ||
      url.password ||
      url.search ||
      url.hash
    ) {
      throw new Error(
        `${label} http URLs must be loopback and credential-free.`,
      );
    }
    return value;
  }
  if (/^[a-z][a-z0-9+.-]*:/iu.test(value) || value.startsWith("//")) {
    throw new Error(
      `${label} must be HTTPS, loopback HTTP, or package-relative.`,
    );
  }
  if (value.split("/").includes("..")) {
    throw new Error(`${label} must not traverse parent directories.`);
  }
  if (
    value.startsWith("/") ||
    !/^(?:\.{0,2}\/|@?[A-Za-z0-9_-])[-A-Za-z0-9_@./]*$/u.test(value) ||
    value.split("/").some(
      (segment, index) => segment === "" || (segment === "." && index !== 0),
    )
  ) {
    throw new Error(
      `${label} must be package-relative, HTTPS, or loopback HTTP.`,
    );
  }
  return value;
}

function normalizeExpectedExports(value, label) {
  if (!Array.isArray(value) || value.length === 0) {
    throw new Error(`${label} expected_exports must be a non-empty array.`);
  }
  const exports = value.map((entry) => {
    if (typeof entry !== "string" || !/^[A-Za-z_$][\w$]*$/u.test(entry)) {
      throw new Error(`${label} expected_exports contains an invalid export.`);
    }
    return entry;
  });
  if (new Set(exports).size !== exports.length) {
    throw new Error(`${label} expected_exports must not contain duplicates.`);
  }
  return exports;
}

function normalizeBrowserProverRef(record, label, proofArtifactHash) {
  const source = requireRecord(record, label);
  const moduleUrl = normalizeModuleUrl(
    firstString(source, "module_url", "moduleUrl"),
    `${label}.module_url`,
  );
  const moduleHash = normalizeHex32(
    firstString(source, "module_hash", "moduleHash"),
    `${label}.module_hash`,
  );
  const manifestHash = normalizeHex32(
    firstString(source, "manifest_hash", "manifestHash"),
    `${label}.manifest_hash`,
  );
  const boundRouteHash = normalizeHex32(
    firstString(source, "bound_route_hash", "boundRouteHash"),
    `${label}.bound_route_hash`,
  );
  const boundProofHash = normalizeHex32(
    firstString(source, "bound_proof_hash", "boundProofHash"),
    `${label}.bound_proof_hash`,
  );
  if (boundRouteHash !== TON_DESTINATION_BINDING_HASH) {
    throw new Error(
      `${label}.bound_route_hash must match the TON destination binding hash.`,
    );
  }
  if (boundProofHash !== proofArtifactHash) {
    throw new Error(
      `${label}.bound_proof_hash must match proof_artifact_hash.`,
    );
  }
  return {
    module_url: moduleUrl,
    module_specifier:
      firstString(source, "module_specifier", "moduleSpecifier") || null,
    module_hash: moduleHash,
    manifest_hash: manifestHash,
    expected_exports: normalizeExpectedExports(
      source.expected_exports ?? source.expectedExports,
      label,
    ),
    bound_route_hash: boundRouteHash,
    bound_proof_hash: boundProofHash,
  };
}

function rejectSecretLikeMaterial(value, label) {
  const secretKeyPattern =
    /(?:private[_-]?key|mnemonic|recovery[_-]?phrase|seed[_-]?phrase|secret)/iu;
  const stack = [{ value, path: label }];
  while (stack.length > 0) {
    const current = stack.pop();
    if (!current || !current.value || typeof current.value !== "object") {
      continue;
    }
    if (Array.isArray(current.value)) {
      current.value.forEach((entry, index) =>
        stack.push({ value: entry, path: `${current.path}[${index}]` }),
      );
      continue;
    }
    for (const [key, entry] of Object.entries(current.value)) {
      if (secretKeyPattern.test(key)) {
        throw new Error(
          `${current.path}.${key} looks secret-like; remove it before publishing.`,
        );
      }
      stack.push({ value: entry, path: `${current.path}.${key}` });
    }
  }
}

function normalizePublicJsonRecord(value, label) {
  const record = requireRecord(value, label);
  rejectSecretLikeMaterial(record, label);
  return stableJsonValue(record);
}

function normalizeTairaBurnRecordVkName(value, label = "--vk-name") {
  const normalized = value ?? DEFAULT_TAIRA_BURN_RECORD_VK_NAME;
  if (
    typeof normalized !== "string" ||
    normalized.trim() !== normalized ||
    !/^[A-Za-z0-9._:-]{1,128}$/u.test(normalized)
  ) {
    throw new Error(
      `${label} must be 1-128 verifier-key identifier characters.`,
    );
  }
  return normalized;
}

function normalizeTairaContractMaterial(raw, { vkName: expectedVkName } = {}) {
  const record = requireRecord(raw, "TAIRA burn-record contract");
  const contract =
    firstRecord(record, "tairaXorBurnRecord", "taira_xor_burn_record") ??
    record;
  const settlementAssetDefinitionId = normalizeCanonicalAssetDefinitionId(
    firstString(
      contract,
      "settlement_asset_definition_id",
      "settlementAssetDefinitionId",
      "settlement_asset",
      "settlementAsset",
    ) ||
      firstString(
        record,
        "settlement_asset_definition_id",
        "settlementAssetDefinitionId",
      ),
    "TAIRA burn-record settlement asset definition id",
  );
  if (
    settlementAssetDefinitionId !== TAIRA_XOR_SETTLEMENT_ASSET_DEFINITION_ID
  ) {
    throw new Error(
      `TAIRA burn-record settlement asset must be ${TAIRA_XOR_SETTLEMENT_ASSET_DEFINITION_ID}.`,
    );
  }
  const vkRef =
    firstRecord(
      contract,
      "vk_ref",
      "vkRef",
      "verifying_key_ref",
      "verifyingKeyRef",
    ) ?? {};
  const vkBackend =
    firstString(vkRef, "backend", "proof_backend", "proofBackend") ||
    firstString(contract, "vk_backend", "vkBackend");
  const vkName =
    firstString(vkRef, "name", "vk_name", "vkName") ||
    firstString(contract, "vk_name", "vkName");
  if (vkBackend !== TAIRA_BURN_RECORD_VK_BACKEND) {
    throw new Error(
      `TAIRA burn-record VK backend must be ${TAIRA_BURN_RECORD_VK_BACKEND}.`,
    );
  }
  if (vkName !== expectedVkName) {
    throw new Error(
      `TAIRA burn-record VK name must be ${expectedVkName}.`,
    );
  }
  const gasLimit = normalizePositiveInteger(
    contract.gas_limit ?? contract.gasLimit,
    "TAIRA burn-record gas limit",
    TAIRA_BURN_RECORD_GAS_LIMIT,
  );
  if (gasLimit !== TAIRA_BURN_RECORD_GAS_LIMIT) {
    throw new Error(
      `TAIRA burn-record gas limit must be ${TAIRA_BURN_RECORD_GAS_LIMIT}.`,
    );
  }
  return {
    settlementAssetDefinitionId,
    contractArtifactB64: normalizeStrictBase64(
      firstString(
        contract,
        "contract_artifact_b64",
        "contractArtifactB64",
        "artifact_b64",
        "artifactB64",
        "bytecode",
      ),
      "TAIRA burn-record contract artifact",
    ),
    artifactSha256: normalizeHex32(
      firstString(
        contract,
        "artifact_sha256",
        "artifactSha256",
        "contract_artifact_sha256",
        "contractArtifactSha256",
      ),
      "TAIRA burn-record artifact sha256",
    ),
    codeHash: normalizeHex32(
      firstString(contract, "code_hash", "codeHash"),
      "TAIRA burn-record code hash",
    ),
    vkBackend,
    vkName,
    gasLimit,
  };
}

function assertNoRetiredTonRouteManifestAliases(manifest) {
  for (const [field, replacement] of RETIRED_TON_ROUTE_MANIFEST_FIELDS) {
    if (Object.hasOwn(manifest, field)) {
      throw new Error(
        `TON route manifest must not use retired ${field}; use ${replacement}.`,
      );
    }
  }
}

function normalizeTonRouteManifestForPublication(input) {
  const source = requireRecord(
    input.manifest && typeof input.manifest === "object"
      ? input.manifest
      : input,
    "TON route manifest",
  );
  const manifest = { ...source };
  delete manifest.schema;
  delete manifest.generated_at_ms;
  delete manifest.generatedAtMs;
  assertNoRetiredTonRouteManifestAliases(manifest);
  if (manifest.route_id !== ROUTE_ID || manifest.asset_key !== ASSET_KEY) {
    throw new Error(`TON route manifest must target ${ROUTE_ID}/${ASSET_KEY}.`);
  }
  if (manifest.counterparty_domain !== TON_COUNTERPARTY_DOMAIN) {
    throw new Error("TON route manifest counterparty_domain must be 4.");
  }
  if (manifest.chain !== "ton-testnet") {
    throw new Error("TON route manifest chain must be ton-testnet.");
  }
  if (manifest.chain_id_hex !== TON_TESTNET_CHAIN_ID_HEX) {
    throw new Error("TON route manifest chain_id_hex must be TON testnet.");
  }
  if (manifest.network_id_hex !== TON_TESTNET_CHAIN_ID_HEX) {
    throw new Error("TON route manifest network_id_hex must be TON testnet.");
  }
  manifest.explorer_url = normalizeRequiredPublicExplorerUrl(
    manifest.explorer_url ?? TON_TESTNET_EXPLORER_URL,
    "TON route manifest explorer_url",
  );
  const expectedExplorerHost = new URL(manifest.explorer_url).host;
  if (manifest.explorer_host === undefined || manifest.explorer_host === null) {
    manifest.explorer_host = expectedExplorerHost;
  } else if (manifest.explorer_host !== expectedExplorerHost) {
    throw new Error(
      "TON route manifest explorer_host must match explorer_url host.",
    );
  }
  if (manifest.destination_binding_key !== TON_DESTINATION_BINDING_KEY) {
    throw new Error(
      "TON route manifest destination_binding_key is not canonical.",
    );
  }
  if (manifest.destination_binding_hash !== TON_DESTINATION_BINDING_HASH) {
    throw new Error(
      "TON route manifest destination_binding_hash is not canonical.",
    );
  }
  for (const [key, label] of [
    ["taira_xor_token_address", "TON TairaXOR token address"],
    ["taira_xor_bridge_address", "TON bridge address"],
  ]) {
    manifest[key] = normalizeTonRawAddress(manifest[key], label);
  }
  manifest.source_bridge_address = normalizeTonRawAddress(
    manifest.source_bridge_address,
    "TON source bridge address",
  );
  manifest.destination_verifier_address = normalizeTonRawAddress(
    manifest.destination_verifier_address,
    "TON verifier address",
  );
  if (
    new Set([
      manifest.taira_xor_token_address,
      manifest.taira_xor_bridge_address,
      manifest.source_bridge_address,
      manifest.destination_verifier_address,
    ]).size !== 4
  ) {
    throw new Error(
      "TON token, bridge, source bridge, and verifier addresses must be distinct.",
    );
  }
  manifest.ton_finalize_message_value_nano = normalizePositiveDecimalString(
    manifest.ton_finalize_message_value_nano,
    "TON finalize message value in nanoTON",
    DEFAULT_TON_FINALIZE_MESSAGE_VALUE_NANO,
  );
  manifest.verifier_code_hash = normalizeHex32(
    manifest.verifier_code_hash,
    "TON verifier code hash",
  );
  manifest.verifier_key_hash = normalizeHex32(
    manifest.verifier_key_hash,
    "TON verifier key hash",
  );
  manifest.proof_artifact_hash = normalizeHex32(
    manifest.proof_artifact_hash,
    "TON proof artifact hash",
  );
  manifest.proving_key_hash = normalizeHex32(
    manifest.proving_key_hash,
    "TON proving key hash",
  );
  manifest.deployment_evidence_sha256 = normalizeHex32(
    manifest.deployment_evidence_sha256,
    "TON deployment evidence sha256",
  );
  manifest.source_verifier_material = normalizePublicJsonRecord(
    manifest.source_verifier_material,
    "TON source verifier material",
  );
  manifest.source_adapter_engine_deployment = normalizePublicJsonRecord(
    manifest.source_adapter_engine_deployment,
    "TON source adapter engine deployment",
  );
  manifest.destination_browser_prover = normalizeBrowserProverRef(
    manifest.destination_browser_prover,
    "destination_browser_prover",
    manifest.proof_artifact_hash,
  );
  manifest.source_browser_prover = normalizeBrowserProverRef(
    manifest.source_browser_prover,
    "source_browser_prover",
    manifest.proof_artifact_hash,
  );
  manifest.taira_burn_record_settlement_asset_definition_id =
    TAIRA_XOR_SETTLEMENT_ASSET_DEFINITION_ID;
  manifest.taira_burn_record_contract_artifact_b64 = normalizeStrictBase64(
    manifest.taira_burn_record_contract_artifact_b64,
    "TAIRA burn-record contract artifact",
  );
  manifest.taira_burn_record_artifact_sha256 = normalizeHex32(
    manifest.taira_burn_record_artifact_sha256,
    "TAIRA burn-record artifact sha256",
  );
  manifest.taira_burn_record_code_hash = normalizeHex32(
    manifest.taira_burn_record_code_hash,
    "TAIRA burn-record code hash",
  );
  manifest.taira_burn_record_vk_backend = TAIRA_BURN_RECORD_VK_BACKEND;
  manifest.taira_burn_record_vk_name = normalizeTairaBurnRecordVkName(
    manifest.taira_burn_record_vk_name,
    "TON route manifest TAIRA burn-record VK name",
  );
  manifest.taira_burn_record_gas_limit = TAIRA_BURN_RECORD_GAS_LIMIT;
  if (manifest.production_ready !== true) {
    throw new Error("TON route manifest production_ready must be true.");
  }
  if (
    manifest.disabled_reason !== undefined &&
    manifest.disabled_reason !== null
  ) {
    throw new Error(
      "Production TON route manifest must not carry disabled_reason.",
    );
  }
  if (manifest.post_deploy_full_toml_ready !== true) {
    throw new Error(
      "TON route manifest post_deploy_full_toml_ready must be true.",
    );
  }
  for (const [key, label] of [
    [
      "post_deploy_source_bridge_config_hash",
      "post-deploy source bridge config hash",
    ],
    [
      "post_deploy_source_event_transaction_id",
      "post-deploy source event transaction id",
    ],
    [
      "post_deploy_route_canary_evidence_hash",
      "post-deploy route canary evidence hash",
    ],
    [
      "post_deploy_route_canary_transaction_id",
      "post-deploy route canary transaction id",
    ],
    [
      "post_deploy_offline_full_toml_sha256",
      "post-deploy offline full TOML sha256",
    ],
  ]) {
    manifest[key] = normalizeHex32(manifest[key], label);
  }
  return stableJsonValue(manifest);
}

function bridgeRouteManifestForInstruction(manifest) {
  const bridgeManifest = { ...manifest };
  // The v1 ISI struct still uses legacy field names; public TON manifests do not.
  bridgeManifest.tron_network = TON_TESTNET_LEGACY_NETWORK;
  bridgeManifest.sccp_tron_source_bridge_address =
    manifest.source_bridge_address;
  bridgeManifest.tron_verifier_address = manifest.destination_verifier_address;
  delete bridgeManifest.source_bridge_address;
  delete bridgeManifest.destination_verifier_address;
  return stableJsonValue(bridgeManifest);
}

async function commandRouteManifest(options) {
  const outPath = normalizeCliPathOption(
    options.out,
    "--out",
    DEFAULT_ROUTE_MANIFEST_OUT,
  );
  const tairaContractPath = normalizeCliPathOption(
    requireOption(options, "taira-contract"),
    "--taira-contract",
  );
  const sourceVerifierMaterialPath = normalizeCliPathOption(
    requireOption(options, "source-verifier-material"),
    "--source-verifier-material",
  );
  const sourceAdapterEngineDeploymentPath = normalizeCliPathOption(
    requireOption(options, "source-adapter-engine-deployment"),
    "--source-adapter-engine-deployment",
  );
  const destinationBrowserProverManifestPath = normalizeCliPathOption(
    requireOption(options, "destination-browser-prover-manifest"),
    "--destination-browser-prover-manifest",
  );
  const sourceBrowserProverManifestPath = normalizeCliPathOption(
    requireOption(options, "source-browser-prover-manifest"),
    "--source-browser-prover-manifest",
  );
  const deploymentEvidencePath = normalizeCliPathOption(
    requireOption(options, "deployment-evidence"),
    "--deployment-evidence",
  );
  const offlineFullTomlEvidencePath = options["offline-full-toml-sha256"]
    ? options["offline-full-toml-evidence"] === undefined
      ? undefined
      : normalizeCliPathOption(
          options["offline-full-toml-evidence"],
          "--offline-full-toml-evidence",
        )
    : normalizeCliPathOption(
        requireOption(options, "offline-full-toml-evidence"),
        "--offline-full-toml-evidence",
      );
  assertDistinctResolvedPaths(outPath, "--out", [
    [tairaContractPath, "--taira-contract"],
    [sourceVerifierMaterialPath, "--source-verifier-material"],
    [
      sourceAdapterEngineDeploymentPath,
      "--source-adapter-engine-deployment",
    ],
    [
      destinationBrowserProverManifestPath,
      "--destination-browser-prover-manifest",
    ],
    [sourceBrowserProverManifestPath, "--source-browser-prover-manifest"],
    [deploymentEvidencePath, "--deployment-evidence"],
    [offlineFullTomlEvidencePath, "--offline-full-toml-evidence"],
  ]);
  const proofArtifactHash = normalizeHex32(
    requireOption(options, "proof-artifact-hash"),
    "--proof-artifact-hash",
  );
  const vkName = normalizeTairaBurnRecordVkName(options["vk-name"]);
  const tairaContract = normalizeTairaContractMaterial(
    await readJson(tairaContractPath, "TAIRA burn-record contract"),
    { vkName },
  );
  const sourceVerifierMaterial = normalizePublicJsonRecord(
    await readJson(
      sourceVerifierMaterialPath,
      "TON source verifier material",
    ),
    "TON source verifier material",
  );
  const sourceAdapterEngineDeployment = normalizePublicJsonRecord(
    await readJson(
      sourceAdapterEngineDeploymentPath,
      "TON source adapter engine deployment",
    ),
    "TON source adapter engine deployment",
  );
  const destinationBrowserProver = normalizeBrowserProverRef(
    await readJson(
      destinationBrowserProverManifestPath,
      "TON destination browser prover manifest",
    ),
    "destination_browser_prover",
    proofArtifactHash,
  );
  const sourceBrowserProver = normalizeBrowserProverRef(
    await readJson(
      sourceBrowserProverManifestPath,
      "TON source browser prover manifest",
    ),
    "source_browser_prover",
    proofArtifactHash,
  );
  const tonFinalizeMessageValueNano = normalizePositiveDecimalString(
    options["ton-finalize-message-value-nano"],
    "--ton-finalize-message-value-nano",
    DEFAULT_TON_FINALIZE_MESSAGE_VALUE_NANO,
  );
  const tokenAddress = normalizeTonRawAddress(
    requireOption(options, "token"),
    "--token",
  );
  const bridgeAddress = normalizeTonRawAddress(
    requireOption(options, "bridge"),
    "--bridge",
  );
  const sourceBridgeAddress = normalizeTonRawAddress(
    requireOption(options, "source-bridge"),
    "--source-bridge",
  );
  const verifierAddress = normalizeTonRawAddress(
    requireOption(options, "verifier"),
    "--verifier",
  );
  const verifierCodeHash = normalizeHex32(
    requireOption(options, "verifier-code-hash"),
    "--verifier-code-hash",
  );
  const verifierKeyHash = normalizeHex32(
    requireOption(options, "verifier-key-hash"),
    "--verifier-key-hash",
  );
  const provingKeyHash = normalizeHex32(
    requireOption(options, "proving-key-hash"),
    "--proving-key-hash",
  );
  const manifest = normalizeTonRouteManifestForPublication({
    schema: ROUTE_MANIFEST_SCHEMA,
    version: 1,
    route_id: ROUTE_ID,
    asset_key: ASSET_KEY,
    chain: "ton-testnet",
    chain_id_hex: TON_TESTNET_CHAIN_ID_HEX,
    explorer_url: options["explorer-url"] ?? TON_TESTNET_EXPLORER_URL,
    explorer_host:
      options["explorer-host"] ??
      new URL(options["explorer-url"] ?? TON_TESTNET_EXPLORER_URL).host,
    counterparty_domain: TON_COUNTERPARTY_DOMAIN,
    counterparty_account_codec: TON_COUNTERPARTY_ACCOUNT_CODEC,
    counterparty_account_codec_key: TON_COUNTERPARTY_ACCOUNT_CODEC_KEY,
    verifier_target: TON_VERIFIER_TARGET,
    production_ready: true,
    network_id_hex: TON_TESTNET_CHAIN_ID_HEX,
    taira_xor_token_address: tokenAddress,
    taira_xor_bridge_address: bridgeAddress,
    source_bridge_address: sourceBridgeAddress,
    destination_verifier_address: verifierAddress,
    ton_finalize_message_value_nano: tonFinalizeMessageValueNano,
    verifier_code_hash: verifierCodeHash,
    verifier_key_hash: verifierKeyHash,
    proof_artifact_hash: proofArtifactHash,
    proving_key_hash: provingKeyHash,
    destination_rollout: {
      version: 1,
      destination_network_id: TON_TESTNET_CHAIN_ID_HEX,
      source_domain: 0,
      target_domain: TON_COUNTERPARTY_DOMAIN,
      verifier_identity: verifierAddress,
      verifier_backend: "ton-contract-v1",
      proof_family: "stark-fri-v1",
      verifier_code_hash: verifierCodeHash,
      verifier_key_hash: verifierKeyHash,
      proof_artifact_hash: proofArtifactHash,
      proving_key_hash: provingKeyHash,
      destination_bridge_address: bridgeAddress,
      source_bridge_address: sourceBridgeAddress,
      destination_binding_hash: TON_DESTINATION_BINDING_HASH,
      destination_binding_key: TON_DESTINATION_BINDING_KEY,
      finalize_message_value_nano: tonFinalizeMessageValueNano,
      ton_finalize_message_value_nano: tonFinalizeMessageValueNano,
    },
    source_verifier_material: sourceVerifierMaterial,
    source_adapter_engine_deployment: sourceAdapterEngineDeployment,
    destination_browser_prover: destinationBrowserProver,
    source_browser_prover: sourceBrowserProver,
    deployment_evidence_sha256: await sha256HexFileOrJson(
      deploymentEvidencePath,
      "TON deployment evidence",
    ),
    destination_binding_key: TON_DESTINATION_BINDING_KEY,
    destination_binding_hash: TON_DESTINATION_BINDING_HASH,
    taira_burn_record_settlement_asset_definition_id:
      tairaContract.settlementAssetDefinitionId,
    taira_burn_record_contract_artifact_b64: tairaContract.contractArtifactB64,
    taira_burn_record_artifact_sha256: tairaContract.artifactSha256,
    taira_burn_record_code_hash: tairaContract.codeHash,
    taira_burn_record_vk_backend: tairaContract.vkBackend,
    taira_burn_record_vk_name: tairaContract.vkName,
    taira_burn_record_gas_limit: tairaContract.gasLimit,
    post_deploy_full_toml_ready: true,
    post_deploy_source_bridge_config_hash: normalizeHex32(
      requireOption(options, "post-deploy-source-bridge-config-hash"),
      "--post-deploy-source-bridge-config-hash",
    ),
    post_deploy_source_event_transaction_id: normalizeHex32(
      requireOption(options, "post-deploy-source-event-transaction-id"),
      "--post-deploy-source-event-transaction-id",
    ),
    post_deploy_source_event_explorer_url: normalizeOptionalPublicExplorerUrl(
      options["post-deploy-source-event-explorer-url"],
      "--post-deploy-source-event-explorer-url",
    ),
    post_deploy_route_canary_evidence_hash: normalizeHex32(
      requireOption(options, "post-deploy-route-canary-evidence-hash"),
      "--post-deploy-route-canary-evidence-hash",
    ),
    post_deploy_route_canary_transaction_id: normalizeHex32(
      requireOption(options, "post-deploy-route-canary-transaction-id"),
      "--post-deploy-route-canary-transaction-id",
    ),
    post_deploy_route_canary_explorer_url: normalizeOptionalPublicExplorerUrl(
      options["post-deploy-route-canary-explorer-url"],
      "--post-deploy-route-canary-explorer-url",
    ),
    post_deploy_offline_full_toml_sha256: options["offline-full-toml-sha256"]
      ? normalizeHex32(
          options["offline-full-toml-sha256"],
          "--offline-full-toml-sha256",
        )
      : await sha256HexFileOrJson(
          offlineFullTomlEvidencePath,
          "TON offline full TOML evidence",
        ),
  });
  const out = await writeJsonNoSecrets(outPath, {
    schema: ROUTE_MANIFEST_SCHEMA,
    generated_at_ms: Date.now(),
    manifest,
  });
  return {
    ok: true,
    wrote: out,
    routeId: manifest.route_id,
    assetKey: manifest.asset_key,
    productionReady: manifest.production_ready,
    destinationBindingHash: manifest.destination_binding_hash,
    tonFinalizeMessageValueNano: manifest.ton_finalize_message_value_nano,
    deploymentEvidenceSha256: manifest.deployment_evidence_sha256,
    nextStep:
      "Review the manifest, publish it with publish-route-manifest, then rerun the public TAIRA TON preflight before UI smoke.",
  };
}

async function commandPublishRouteManifest(options) {
  const manifestPath = normalizeCliPathOption(
    options.manifest,
    "--manifest",
    DEFAULT_ROUTE_MANIFEST_OUT,
  );
  const outPath = normalizeCliPathOption(
    options.out,
    "--out",
    DEFAULT_ROUTE_MANIFEST_ISI_OUT,
  );
  assertDistinctResolvedPaths(outPath, "--out", [[manifestPath, "--manifest"]]);
  const submit = optionEnabled(options, "submit", false);
  if (!submit) {
    assertSubmitOnlyOptionsAbsentWhenNotSubmitting(options);
  }
  const waitForCommit = submit
    ? optionEnabled(options, "wait-for-commit", true)
    : true;
  const gasAssetId =
    options["gas-asset-id"] === undefined || options["gas-asset-id"] === null
      ? TAIRA_XOR_SETTLEMENT_ASSET_DEFINITION_ID
      : normalizeCanonicalAssetDefinitionId(
          options["gas-asset-id"],
          "--gas-asset-id",
        );
  const gasLimit = normalizePositiveInteger(
    options["gas-limit"],
    "--gas-limit",
    DEFAULT_TAIRA_ROUTE_MANIFEST_GAS_LIMIT,
  );
  const manifestEnvelope = await readJson(manifestPath, "TON route manifest");
  const manifest = normalizeTonRouteManifestForPublication(manifestEnvelope);
  const instructionManifest = bridgeRouteManifestForInstruction(manifest);
  const instruction = {
    UpsertSccpRouteManifest: {
      manifest: instructionManifest,
    },
  };
  const artifact = {
    schema: ROUTE_MANIFEST_ISI_SCHEMA,
    routeId: manifest.route_id,
    assetKey: manifest.asset_key,
    routeKey: {
      routeId: manifest.route_id,
      assetKey: manifest.asset_key,
      counterpartyDomain: manifest.counterparty_domain,
      chainIdHex: manifest.chain_id_hex,
    },
    requiredPermission: "CanManageSccpRouteManifests",
    instruction,
    manifestSha256: sha256HexJson(manifest),
    instructionManifestSha256: sha256HexJson(instructionManifest),
    productionReady: manifest.production_ready,
    tonFinalizeMessageValueNano: manifest.ton_finalize_message_value_nano,
    destinationBrowserProverManifestHash:
      manifest.destination_browser_prover.manifest_hash,
    sourceBrowserProverManifestHash:
      manifest.source_browser_prover.manifest_hash,
    gasAssetId,
    gasLimit,
  };
  if (!submit) {
    const out = await writeJsonNoSecrets(outPath, artifact);
    return {
      ok: true,
      wrote: out,
      submitted: false,
      routeId: manifest.route_id,
      assetKey: manifest.asset_key,
      requiredPermission: artifact.requiredPermission,
      nextStep:
        "Review the ISI artifact, then rerun with --submit true and a TAIRA authority holding CanManageSccpRouteManifests.",
    };
  }

  const authority = normalizeAuthority(requireOption(options, "authority"));
  const privateKeyEnv = normalizePrivateKeyEnvName(
    options["private-key-env"] ?? DEFAULT_TAIRA_ROUTE_MANIFEST_PRIVATE_KEY_ENV,
  );
  const chainId = options["chain-id"] ?? TAIRA_CHAIN_ID;
  if (chainId !== TAIRA_CHAIN_ID) {
    throw new Error(`--chain-id must be ${TAIRA_CHAIN_ID} for TAIRA.`);
  }
  const toriiUrl = normalizeToriiUrl(
    options["torii-url"] ?? DEFAULT_TAIRA_TORII_URL,
  );
  const timeoutMs = normalizePositiveInteger(
    options["commit-timeout-ms"],
    "--commit-timeout-ms",
    DEFAULT_COMMIT_TIMEOUT_MS,
  );
  const privateKeyHex = process.env[privateKeyEnv];
  if (typeof privateKeyHex !== "string" || privateKeyHex.trim() === "") {
    throw new Error(
      `${privateKeyEnv} must be set at runtime for --submit true.`,
    );
  }
  const privateKey = Buffer.from(
    normalizePrivateKeyHex(privateKeyHex, privateKeyEnv).slice(2),
    "hex",
  );
  const { buildUpsertSccpRouteManifestTransaction } = await import(
    "../javascript/iroha_js/src/transaction.js"
  );
  const { ToriiClient } = await import(
    "../javascript/iroha_js/src/toriiClient.js"
  );
  const transaction = buildUpsertSccpRouteManifestTransaction({
    chainId,
    authority,
    manifest: instructionManifest,
    metadata: {
      route_id: manifest.route_id,
      asset_key: manifest.asset_key,
      action: "publish_sccp_ton_route_manifest",
      gas_asset_id: gasAssetId,
      gas_limit: gasLimit,
    },
    privateKey,
  });
  const hash = transaction.hash.toString("hex");
  const client = new ToriiClient(toriiUrl);
  const status = waitForCommit
    ? await client.submitTransactionAndWait(transaction.signedTransaction, {
        hashHex: hash,
        timeoutMs,
      })
    : await client.submitTransaction(transaction.signedTransaction);
  const submission = {
    submitted: true,
    toriiUrl,
    chainId,
    authority,
    hash,
    waitForCommit,
    commitTimeoutMs: timeoutMs,
    status,
    gasAssetId,
    gasLimit,
  };
  const out = await writeJsonNoSecrets(outPath, {
    ...artifact,
    submission,
  });
  return {
    ok: true,
    wrote: out,
    submitted: true,
    toriiUrl,
    chainId,
    authority,
    hash,
    waitForCommit,
    gasAssetId,
    gasLimit,
    routeId: manifest.route_id,
    assetKey: manifest.asset_key,
  };
}

function normalizePrivateKeyHex(value, label) {
  const text = value.trim();
  const prefixed = text.startsWith("0x") ? text : `0x${text}`;
  if (!/^0x[0-9a-fA-F]{64}(?:[0-9a-fA-F]{64})?$/u.test(prefixed)) {
    throw new Error(
      `${label} must contain a 32- or 64-byte Ed25519 private key.`,
    );
  }
  return `0x${prefixed.slice(2).toLowerCase()}`;
}

function normalizePrivateKeyEnvName(
  value = DEFAULT_TAIRA_ROUTE_MANIFEST_PRIVATE_KEY_ENV,
) {
  if (
    typeof value !== "string" ||
    value.trim() !== value ||
    !/^[A-Z_][A-Z0-9_]{0,127}$/u.test(value)
  ) {
    throw new Error(
      "--private-key-env must be an uppercase environment variable name containing only letters, digits, and underscores.",
    );
  }
  return value;
}

function normalizeAuthority(value) {
  if (
    typeof value !== "string" ||
    value.trim() !== value ||
    /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    throw new Error("--authority must be a canonical I105 account id.");
  }
  try {
    return normalizeAccountId(value, "--authority");
  } catch (_error) {
    throw new Error("--authority must be a canonical I105 account id.");
  }
}

function normalizeToriiUrl(value) {
  if (
    typeof value !== "string" ||
    value.trim() !== value ||
    /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    throw new Error("--torii-url must be a valid HTTP(S) URL.");
  }
  let url;
  try {
    url = new URL(value);
  } catch (_error) {
    throw new Error("--torii-url must be a valid HTTP(S) URL.");
  }
  const loopback = isLoopbackHost(url.hostname);
  if (url.protocol !== "https:" && !(loopback && url.protocol === "http:")) {
    throw new Error("--torii-url must use HTTPS unless it is loopback HTTP.");
  }
  if (url.username || url.password || url.search || url.hash) {
    throw new Error(
      "--torii-url must not include credentials, query, or fragment.",
    );
  }
  if (url.protocol === "https:" && isNonPublicDnsHost(url.hostname)) {
    throw new Error("--torii-url HTTPS host must use public DNS.");
  }
  return url.toString().replace(/\/$/u, "");
}

function normalizeOptionalPublicExplorerUrl(value, label) {
  if (value === undefined || value === null) {
    return null;
  }
  if (
    typeof value !== "string" ||
    value.trim() !== value ||
    value === "" ||
    /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    throw new Error(`${label} must be a public HTTPS URL.`);
  }
  let url;
  try {
    url = new URL(value);
  } catch (_error) {
    throw new Error(`${label} must be a public HTTPS URL.`);
  }
  if (url.protocol !== "https:") {
    throw new Error(`${label} must be a public HTTPS URL.`);
  }
  if (url.username || url.password || url.search || url.hash) {
    throw new Error(
      `${label} must not include credentials, query, or fragment.`,
    );
  }
  if (isNonPublicDnsHost(url.hostname)) {
    throw new Error(`${label} must use a public DNS host.`);
  }
  return url.toString().replace(/\/$/u, "");
}

function normalizeRequiredPublicExplorerUrl(value, label) {
  const normalized = normalizeOptionalPublicExplorerUrl(value, label);
  if (normalized === null) {
    throw new Error(`${label} must be a public HTTPS URL.`);
  }
  return normalized;
}

async function main(argv = process.argv.slice(2)) {
  const { command, options } = parseArgs(argv);
  if (command === "help") {
    process.stdout.write(usage(command));
    return 0;
  }
  assertKnownCommand(command);
  assertKnownOptions(command, options);
  if (options.help) {
    process.stdout.write(usage(command));
    return 0;
  }
  let result;
  if (command === "route-manifest") {
    result = await commandRouteManifest(options);
  } else if (command === "publish-route-manifest") {
    result = await commandPublishRouteManifest(options);
  } else {
    throw new Error("Unknown command.");
  }
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  return 0;
}

main().catch((error) => {
  process.stderr.write(`${error?.message ?? String(error)}\n`);
  process.exitCode = 1;
});
