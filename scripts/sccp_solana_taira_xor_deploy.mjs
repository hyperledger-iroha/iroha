#!/usr/bin/env node
import { createHash } from "node:crypto";
import fs from "node:fs";
import { isIP } from "node:net";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const DEFAULT_SOLANA_RPC_URL = "https://api.testnet.solana.com";
const DEFAULT_TAIRA_TORII_URL = "https://taira.sora.org";
const SOLANA_TESTNET_CHAIN_ID_HEX = "0x736f6c616e612d746573746e6574";
const ROUTE_ID = "taira_sol_xor";
const CONFIRM_NETWORK = `${ROUTE_ID}:solana-testnet`;
const ASSET_KEY = "xor";
const SOL_DOMAIN = 3;
const RETIRED_SOLANA_ROUTE_MANIFEST_ALIASES = Object.freeze([
  ["productionReady", "production_ready"],
  ["routeId", "route_id"],
  ["assetKey", "asset_key"],
  ["solanaNetwork", "solana_network"],
  ["chainIdHex", "chain_id_hex"],
  ["counterpartyAccountCodec", "counterparty_account_codec"],
  ["counterpartyAccountCodecKey", "counterparty_account_codec_key"],
  ["counterpartyDomain", "counterparty_domain"],
  ["verifierTarget", "verifier_target"],
  ["networkIdHex", "network_id_hex"],
  ["destinationBrowserProver", "destination_browser_prover"],
  ["sourceBrowserProver", "source_browser_prover"],
  ["sourceVerifierMaterial", "source_verifier_material"],
  ["sourceAdapterEngineDeployment", "source_adapter_engine_deployment"],
  ["sourceAdapterEngine", "source_adapter_engine"],
  ["taira_xor_solana_program_id", "taira_xor_bridge_address"],
  ["solana_program_id", "taira_xor_bridge_address"],
  ["tairaXorSolanaProgramId", "taira_xor_bridge_address"],
  ["solanaProgramId", "taira_xor_bridge_address"],
  ["solana_token_mint", "taira_xor_token_address"],
  ["tairaXorTokenAddress", "taira_xor_token_address"],
  ["solanaTokenMint", "taira_xor_token_address"],
  ["solana_source_bridge_address", "sccp_solana_source_bridge_address"],
  ["sccpSolanaSourceBridgeAddress", "sccp_solana_source_bridge_address"],
  ["solanaSourceBridgeAddress", "sccp_solana_source_bridge_address"],
  ["solanaVerifierProgramId", "solana_verifier_program_id"],
  [
    "sccp_solana_destination_verifier_program_id",
    "solana_verifier_program_id",
  ],
  ["sccpSolanaDestinationVerifierProgramId", "solana_verifier_program_id"],
]);
const COMMAND_OPTION_ALLOWLISTS = Object.freeze({
  doctor: new Set(["solana-rpc-url", "torii-url"]),
  deploy: new Set([
    "program-so",
    "program-id-keypair",
    "keypair",
    "broadcast",
    "confirm-network",
    "solana-rpc-url",
    "final",
  ]),
  evidence: new Set(["program-id", "output", "solana-rpc-url", "keypair"]),
  "route-manifest": new Set(["template", "evidence", "output"]),
  "propose-route-manifest": new Set([
    "manifest",
    "torii-url",
    "mode",
    "output",
  ]),
});

const usage = () => {
  console.log(`Usage: node scripts/sccp_solana_taira_xor_deploy.mjs <command> [options]

Commands:
  doctor
    Check Solana CLI, Solana testnet RPC, and TAIRA governance endpoint readiness.

  deploy --program-so PATH --program-id-keypair PATH --keypair PATH --broadcast true --confirm-network ${CONFIRM_NETWORK} [--final true]
    Deploy a compiled Solana SCCP program to Solana testnet through the Solana CLI.

  evidence --program-id ADDRESS --output PATH
    Read live Solana program evidence with "solana program show --output json".

  route-manifest --template PATH --evidence PATH --output PATH
    Validate and write a production-ready taira_sol_xor route manifest JSON.

  propose-route-manifest --manifest PATH [--torii-url URL] [--mode Plain|Zk] [--output PATH]
    POST the manifest to /v1/gov/proposals/sccp-route-manifest and write the draft response.
`);
};

const parseArgs = (argv) => {
  const args = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!token.startsWith("--")) {
      throw new Error("Unexpected positional argument.");
    }
    const key = token.slice(2);
    if (Object.prototype.hasOwnProperty.call(args, key)) {
      throw new Error("Option must be specified at most once.");
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
};

const assertKnownOptions = (command, args) => {
  const allowed = COMMAND_OPTION_ALLOWLISTS[command];
  if (!allowed) {
    throw new Error("Unknown command.");
  }
  for (const key of Object.keys(args)) {
    if (!allowed.has(key)) {
      throw new Error("Unknown option.");
    }
  }
};

const requireOption = (args, key) => {
  const value = args[key];
  if (typeof value !== "string" || value === "true") {
    throw new Error(`--${key} must be specified with an explicit value.`);
  }
  if (!value || value.trim() !== value) {
    throw new Error(
      `--${key} must be a non-empty value without surrounding whitespace.`,
    );
  }
  return value;
};

const normalizeProposalMode = (value = "Plain") => {
  if (value !== "Plain" && value !== "Zk") {
    throw new Error("--mode must be Plain or Zk.");
  }
  return value;
};

const normalizeOptionalBooleanOption = (args, key, defaultValue = false) => {
  if (!Object.prototype.hasOwnProperty.call(args, key)) {
    return defaultValue;
  }
  const value = args[key];
  if (value === "true") {
    return true;
  }
  if (value === "false") {
    return false;
  }
  throw new Error(`--${key} must be true or false.`);
};

const normalizeUrlHostname = (hostname) =>
  String(hostname ?? "")
    .toLowerCase()
    .replace(/^\[/u, "")
    .replace(/\]$/u, "");

const isLoopbackHost = (hostname) => {
  const host = normalizeUrlHostname(hostname);
  if (host === "localhost" || host.endsWith(".localhost") || host === "::1") {
    return true;
  }
  if (!/^(\d{1,3})(?:\.(\d{1,3})){3}$/u.test(host)) {
    return false;
  }
  const octets = host.split(".").map((octet) => Number.parseInt(octet, 10));
  return (
    octets.every((octet) => octet >= 0 && octet <= 255) && octets[0] === 127
  );
};

const isNonPublicDnsHost = (hostname) => {
  const host = normalizeUrlHostname(hostname);
  const labels = host.split(".");
  return (
    !host ||
    isLoopbackHost(host) ||
    host.endsWith(".local") ||
    !host.includes(".") ||
    isIP(host) !== 0 ||
    labels.some(
      (label) =>
        label === "" ||
        !/^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/u.test(label),
    )
  );
};

const normalizeToriiUrl = (value = DEFAULT_TAIRA_TORII_URL) => {
  if (
    typeof value !== "string" ||
    !value ||
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
  if (
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    value.includes(";")
  ) {
    throw new Error(
      "--torii-url must not include credentials, params, query strings, or fragments.",
    );
  }
  if (url.protocol === "https:" && isNonPublicDnsHost(url.hostname)) {
    throw new Error("--torii-url HTTPS host must use public DNS.");
  }
  return url.toString().replace(/\/$/u, "");
};

const normalizeSolanaRpcUrl = (value = DEFAULT_SOLANA_RPC_URL) => {
  if (
    typeof value !== "string" ||
    !value ||
    value.trim() !== value ||
    /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    throw new Error("--solana-rpc-url must be a valid HTTP(S) URL.");
  }
  let url;
  try {
    url = new URL(value);
  } catch (_error) {
    throw new Error("--solana-rpc-url must be a valid HTTP(S) URL.");
  }
  const loopback = isLoopbackHost(url.hostname);
  if (url.protocol !== "https:" && !(loopback && url.protocol === "http:")) {
    throw new Error(
      "--solana-rpc-url must use HTTPS unless it is loopback HTTP.",
    );
  }
  if (
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    value.includes(";")
  ) {
    throw new Error(
      "--solana-rpc-url must not include credentials, params, query strings, or fragments.",
    );
  }
  if (url.protocol === "https:" && isNonPublicDnsHost(url.hostname)) {
    throw new Error("--solana-rpc-url HTTPS host must use public DNS.");
  }
  return url.toString().replace(/\/$/u, "");
};

const normalizeModuleUrl = (value, label) => {
  if (
    typeof value !== "string" ||
    !value ||
    value.trim() !== value ||
    /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    throw new Error(`${label} must be a deterministic module URL.`);
  }
  if (/^[a-z][a-z0-9+.-]*:/iu.test(value)) {
    let url;
    try {
      url = new URL(value);
    } catch (_error) {
      throw new Error(`${label} must be a valid URL.`);
    }
    const loopback = isLoopbackHost(url.hostname);
    if (url.protocol !== "https:" && !(loopback && url.protocol === "http:")) {
      throw new Error(`${label} must use HTTPS or loopback HTTP.`);
    }
    if (
      url.username ||
      url.password ||
      url.search ||
      url.hash ||
      value.includes(";")
    ) {
      throw new Error(
        `${label} must not contain credentials, params, query strings, or fragments.`,
      );
    }
    if (url.protocol === "https:" && isNonPublicDnsHost(url.hostname)) {
      throw new Error(`${label} HTTPS URLs must use public DNS.`);
    }
    return url.toString();
  }
  if (value.split("/").includes("..")) {
    throw new Error(`${label} must not traverse parent directories.`);
  }
  if (
    value.startsWith("/") ||
    value.startsWith("//") ||
    value.includes("?") ||
    value.includes("#") ||
    value.includes("\\") ||
    !/^(?:\.\/|@?[A-Za-z0-9_-])[-A-Za-z0-9_@./]*$/u.test(value) ||
    value.split("/").some(
      (segment, index) => segment === "" || (segment === "." && index !== 0),
    )
  ) {
    throw new Error(
      `${label} must be package-relative, HTTPS, or loopback HTTP without query strings or fragments.`,
    );
  }
  return value;
};

const normalizeExpectedExports = (value, label) => {
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
};

const readJson = (file) => JSON.parse(fs.readFileSync(file, "utf8"));

const canonicalPathForCollision = (file) => {
  const resolved = path.resolve(file);
  try {
    return fs.realpathSync(resolved);
  } catch {
    const parent = path.dirname(resolved);
    try {
      return path.join(fs.realpathSync(parent), path.basename(resolved));
    } catch {
      return resolved;
    }
  }
};

const assertDistinctResolvedPaths = (
  leftPath,
  leftLabel,
  rightPath,
  rightLabel,
) => {
  if (
    canonicalPathForCollision(leftPath) === canonicalPathForCollision(rightPath)
  ) {
    throw new Error(`${leftLabel} must not be the same path as ${rightLabel}.`);
  }
};

const temporaryOutputPath = (file) => {
  const resolved = path.resolve(file);
  const suffix = `${process.pid}.${Date.now()}.${Math.random()
    .toString(16)
    .slice(2)}`;
  return `${resolved}.tmp-${suffix}`;
};

const replaceWithTemporaryFile = (file, value) => {
  const resolved = path.resolve(file);
  fs.mkdirSync(path.dirname(resolved), { recursive: true });
  for (let attempt = 0; attempt < 8; attempt += 1) {
    const temp = temporaryOutputPath(resolved);
    try {
      fs.writeFileSync(temp, value, { flag: "wx" });
      fs.renameSync(temp, resolved);
      return;
    } catch (error) {
      try {
        fs.rmSync(temp, { force: true });
      } catch {
        // Best-effort cleanup only; the original write error is authoritative.
      }
      if (error?.code === "EEXIST") {
        continue;
      }
      throw error;
    }
  }
  throw new Error("Unable to allocate temporary output path.");
};

const writeJson = (file, value) => {
  replaceWithTemporaryFile(file, `${JSON.stringify(value, null, 2)}\n`);
};

const commandExists = (command) => {
  const result = spawnSync("sh", ["-c", `command -v ${command}`], {
    encoding: "utf8",
  });
  return result.status === 0;
};

const run = (command, args, options = {}) => {
  const result = spawnSync(command, args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    ...options,
  });
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed:\n${result.stderr || result.stdout}`,
    );
  }
  return result.stdout.trim();
};

const rpc = async (url, method, params = []) => {
  const response = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method,
      params,
    }),
  });
  if (!response.ok) {
    throw new Error(`Solana RPC ${method} failed with HTTP ${response.status}`);
  }
  const payload = await response.json();
  if (payload.error) {
    throw new Error(`Solana RPC ${method} failed: ${payload.error.message}`);
  }
  return payload.result;
};

const sha256Hex = (value) => createHash("sha256").update(value).digest("hex");

const normalizeHex32 = (value, label) => {
  if (typeof value !== "string" || value.trim() !== value) {
    throw new Error(`${label} must be a 32-byte lowercase hex value`);
  }
  const body = value.replace(/^0x/u, "");
  if (!/^[0-9a-f]{64}$/u.test(body)) {
    throw new Error(`${label} must be a 32-byte lowercase hex value`);
  }
  return body;
};

const normalizeRequiredString = (value, label) => {
  if (typeof value !== "string" || !value || value.trim() !== value) {
    throw new Error(
      `${label} must be a non-empty string without surrounding whitespace`,
    );
  }
  return value;
};

const normalizeOptionalString = (value, label) => {
  if (value === undefined || value === null) {
    return null;
  }
  return normalizeRequiredString(value, label);
};

const normalizeOptionalObject = (value, label) => {
  if (value === undefined || value === null) {
    return null;
  }
  if (typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  return value;
};

const normalizeSafeInteger = (value, label) => {
  if (!Number.isSafeInteger(value)) {
    throw new Error(`${label} must be an integer`);
  }
  return value;
};

const normalizeOptionalSafeInteger = (value, label) => {
  if (value === undefined || value === null) {
    return null;
  }
  return normalizeSafeInteger(value, label);
};

const normalizeOptionalHex32 = (value, label) => {
  if (value === undefined || value === null) {
    return null;
  }
  return normalizeHex32(value, label);
};

const normalizeRequiredBooleanField = (record, key, label) => {
  if (!Object.prototype.hasOwnProperty.call(record, key)) {
    throw new Error(`${label} must be the boolean true`);
  }
  const value = record[key];
  if (value !== true) {
    throw new Error(`${label} must be the boolean true`);
  }
  return true;
};

const assertNoRetiredFieldAliases = (record, aliases, label) => {
  for (const [field, replacement] of aliases) {
    if (Object.prototype.hasOwnProperty.call(record, field)) {
      throw new Error(`${label} must not use retired ${field}; use ${replacement}.`);
    }
  }
};

const RETIRED_BROWSER_PROVER_ALIASES = Object.freeze([
  ["moduleSpecifier", "module_specifier"],
  ["moduleUrl", "module_url"],
  ["moduleHash", "module_hash"],
  ["manifestHash", "manifest_hash"],
  ["expectedExports", "expected_exports"],
  ["boundRouteHash", "bound_route_hash"],
  ["boundProofHash", "bound_proof_hash"],
]);

const normalizeBrowserProver = (value, label) => {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  assertNoRetiredFieldAliases(
    value,
    RETIRED_BROWSER_PROVER_ALIASES,
    label,
  );
  const moduleSpecifier = value.module_specifier;
  return {
    module_url: normalizeModuleUrl(
      value.module_url,
      `${label}.module_url`,
    ),
    module_specifier:
      moduleSpecifier !== undefined
        ? normalizeRequiredString(moduleSpecifier, `${label}.module_specifier`)
        : null,
    module_hash: normalizeHex32(
      value.module_hash,
      `${label}.module_hash`,
    ),
    manifest_hash: normalizeHex32(
      value.manifest_hash,
      `${label}.manifest_hash`,
    ),
    expected_exports: normalizeExpectedExports(
      value.expected_exports,
      label,
    ),
    bound_route_hash: normalizeHex32(
      value.bound_route_hash,
      `${label}.bound_route_hash`,
    ),
    bound_proof_hash: normalizeHex32(
      value.bound_proof_hash,
      `${label}.bound_proof_hash`,
    ),
  };
};

const normalizeManifest = (template, evidence) => {
  const manifest = { ...template };
  for (const [
    field,
    replacement,
  ] of RETIRED_SOLANA_ROUTE_MANIFEST_ALIASES) {
    if (Object.prototype.hasOwnProperty.call(manifest, field)) {
      throw new Error(
        `Solana route-manifest template must not use retired ${field}; use ${replacement}.`,
      );
    }
  }
  for (const field of [
    "tron_network",
    "tronNetwork",
    "tron_verifier_address",
    "tronVerifierAddress",
    "sccp_tron_source_bridge_address",
    "sccpTronSourceBridgeAddress",
  ]) {
    if (Object.prototype.hasOwnProperty.call(manifest, field)) {
      throw new Error(`Solana route-manifest template must not use ${field}`);
    }
  }
  manifest.version = normalizeSafeInteger(manifest.version ?? 1, "version");
  manifest.route_id = normalizeRequiredString(
    manifest.route_id ?? ROUTE_ID,
    "route_id",
  );
  manifest.asset_key = normalizeRequiredString(
    manifest.asset_key ?? ASSET_KEY,
    "asset_key",
  );
  manifest.solana_network = normalizeRequiredString(
    manifest.solana_network ?? "testnet",
    "solana_network",
  );
  manifest.chain = normalizeRequiredString(
    manifest.chain ?? "solana-testnet",
    "chain",
  );
  manifest.chain_id_hex = normalizeRequiredString(
    manifest.chain_id_hex ?? SOLANA_TESTNET_CHAIN_ID_HEX,
    "chain_id_hex",
  );
  manifest.counterparty_account_codec = normalizeSafeInteger(
    manifest.counterparty_account_codec ?? 3,
    "counterparty_account_codec",
  );
  manifest.counterparty_account_codec_key = normalizeRequiredString(
    manifest.counterparty_account_codec_key ?? "solana_base58",
    "counterparty_account_codec_key",
  );
  manifest.counterparty_domain = normalizeSafeInteger(
    manifest.counterparty_domain ?? SOL_DOMAIN,
    "counterparty_domain",
  );
  manifest.verifier_target = normalizeRequiredString(
    manifest.verifier_target ?? "SolanaProgram",
    "verifier_target",
  );
  manifest.production_ready = normalizeRequiredBooleanField(
    manifest,
    "production_ready",
    "production_ready",
  );
  manifest.network_id_hex = normalizeRequiredString(
    manifest.network_id_hex ?? SOLANA_TESTNET_CHAIN_ID_HEX,
    "network_id_hex",
  );

  const programId =
    evidence.programId ?? evidence.program_id ?? evidence.program;
  if (
    programId &&
    !Object.prototype.hasOwnProperty.call(manifest, "taira_xor_bridge_address")
  ) {
    manifest.taira_xor_bridge_address = programId;
  }

  for (const [field, label] of [
    ["taira_xor_token_address", "Solana XOR token mint"],
    ["taira_xor_bridge_address", "Solana bridge program"],
    ["sccp_solana_source_bridge_address", "Solana source bridge program"],
    ["solana_verifier_program_id", "Solana verifier program"],
    ["destination_binding_key", "destination binding key"],
    [
      "taira_burn_record_settlement_asset_definition_id",
      "TAIRA settlement asset definition id",
    ],
    ["taira_burn_record_contract_artifact_b64", "TAIRA burn-record artifact"],
    ["taira_burn_record_vk_backend", "TAIRA burn-record VK backend"],
    ["taira_burn_record_vk_name", "TAIRA burn-record VK name"],
  ]) {
    manifest[field] = normalizeRequiredString(manifest[field], label);
  }

  for (const field of [
    "verifier_code_hash",
    "verifier_key_hash",
    "destination_binding_hash",
    "taira_burn_record_artifact_sha256",
    "taira_burn_record_code_hash",
  ]) {
    manifest[field] = normalizeHex32(manifest[field], field);
  }

  manifest.taira_burn_record_gas_limit = normalizeSafeInteger(
    manifest.taira_burn_record_gas_limit ?? 2_000_000,
    "taira_burn_record_gas_limit",
  );
  manifest.destination_browser_prover = normalizeBrowserProver(
    manifest.destination_browser_prover,
    "destination_browser_prover",
  );
  manifest.source_browser_prover = normalizeBrowserProver(
    manifest.source_browser_prover,
    "source_browser_prover",
  );
  for (const [field, label] of [
    ["destination_browser_prover", "destination_browser_prover"],
    ["source_browser_prover", "source_browser_prover"],
  ]) {
    if (manifest[field].bound_route_hash !== manifest.destination_binding_hash) {
      throw new Error(
        `${label}.bound_route_hash must match destination_binding_hash`,
      );
    }
  }
  manifest.source_verifier_material = normalizeOptionalObject(
    manifest.source_verifier_material,
    "source_verifier_material",
  );
  const sourceAdapterEngineDeployment = normalizeOptionalObject(
    manifest.source_adapter_engine_deployment,
    "source_adapter_engine_deployment",
  );
  manifest.source_adapter_engine_deployment = {
    ...(sourceAdapterEngineDeployment ?? {}),
    solana_programdata_address: normalizeOptionalString(
      evidence.programDataAddress ?? evidence.programdataAddress,
      "evidence programDataAddress",
    ),
    solana_programdata_slot: normalizeOptionalSafeInteger(
      evidence.programDataSlot ?? evidence.programdataSlot,
      "evidence programDataSlot",
    ),
    solana_program_account_sha256: normalizeOptionalHex32(
      evidence.programAccountDataSha256 ??
        evidence.program_account_data_sha256,
      "evidence programAccountDataSha256",
    ),
  };
  manifest.source_adapter_engine = normalizeOptionalObject(
    manifest.source_adapter_engine,
    "source_adapter_engine",
  );

  if (
    manifest.route_id !== ROUTE_ID ||
    manifest.asset_key !== ASSET_KEY ||
    manifest.counterparty_domain !== SOL_DOMAIN ||
    manifest.chain !== "solana-testnet" ||
    manifest.chain_id_hex !== SOLANA_TESTNET_CHAIN_ID_HEX ||
    manifest.verifier_target !== "SolanaProgram" ||
    manifest.counterparty_account_codec_key !== "solana_base58" ||
    manifest.counterparty_account_codec !== 3
  ) {
    throw new Error(
      "manifest is not the canonical taira_sol_xor Solana testnet route",
    );
  }
  return manifest;
};

const doctor = async (args) => {
  const solanaRpcUrl = normalizeSolanaRpcUrl(
    args["solana-rpc-url"] ?? DEFAULT_SOLANA_RPC_URL,
  );
  const toriiUrl = normalizeToriiUrl(
    args["torii-url"] ?? DEFAULT_TAIRA_TORII_URL,
  );
  const checks = [];
  checks.push({ name: "solana-cli", ok: commandExists("solana") });
  checks.push({
    name: "anchor-cli",
    ok: commandExists("anchor"),
    optional: true,
  });
  try {
    const health = await rpc(solanaRpcUrl, "getHealth");
    checks.push({
      name: "solana-testnet-rpc",
      ok: health === "ok",
      evidence: health,
    });
  } catch (error) {
    checks.push({
      name: "solana-testnet-rpc",
      ok: false,
      error: error.message,
    });
  }
  try {
    const response = await fetch(
      `${toriiUrl.replace(/\/+$/u, "")}/openapi.json`,
    );
    const openapi = response.ok ? await response.json() : null;
    checks.push({
      name: "taira-governance-sccp-route-endpoint",
      ok: Boolean(openapi?.paths?.["/v1/gov/proposals/sccp-route-manifest"]),
      httpStatus: response.status,
    });
  } catch (error) {
    checks.push({
      name: "taira-governance-sccp-route-endpoint",
      ok: false,
      error: error.message,
    });
  }
  console.log(
    JSON.stringify(
      { ok: checks.every((check) => check.ok || check.optional), checks },
      null,
      2,
    ),
  );
};

const deploy = (args) => {
  const final = normalizeOptionalBooleanOption(args, "final");
  if (
    args.broadcast !== "true" ||
    args["confirm-network"] !== CONFIRM_NETWORK
  ) {
    throw new Error(
      `deploy requires --broadcast true --confirm-network ${CONFIRM_NETWORK}`,
    );
  }
  const deployArgs = [
    "program",
    "deploy",
    "--url",
    normalizeSolanaRpcUrl(args["solana-rpc-url"] ?? DEFAULT_SOLANA_RPC_URL),
    "--keypair",
    requireOption(args, "keypair"),
    "--program-id",
    requireOption(args, "program-id-keypair"),
  ];
  if (final) {
    deployArgs.push("--final");
  }
  deployArgs.push(requireOption(args, "program-so"));
  const output = run("solana", deployArgs);
  console.log(output);
};

const evidence = (args) => {
  const programId = requireOption(args, "program-id");
  const output = run("solana", [
    "program",
    "show",
    "--url",
    normalizeSolanaRpcUrl(args["solana-rpc-url"] ?? DEFAULT_SOLANA_RPC_URL),
    ...(args.keypair ? ["--keypair", requireOption(args, "keypair")] : []),
    "--output",
    "json",
    programId,
  ]);
  const parsed = JSON.parse(output);
  const evidenceDoc = {
    schema: "sccp-solana-taira-xor-program-evidence/v1",
    programId,
    programDataAddress:
      parsed.programDataAddress ??
      parsed.programdataAddress ??
      parsed["ProgramData Address"] ??
      null,
    programDataSlot:
      parsed.lastDeploySlot ??
      parsed.lastDeployedSlot ??
      parsed["Last Deployed In Slot"] ??
      null,
    authority: parsed.authority ?? parsed["Authority"] ?? null,
    raw: parsed,
    evidenceSha256: sha256Hex(output),
  };
  writeJson(requireOption(args, "output"), evidenceDoc);
};

const routeManifest = (args) => {
  const templatePath = requireOption(args, "template");
  const evidencePath = requireOption(args, "evidence");
  const outputPath = requireOption(args, "output");
  assertDistinctResolvedPaths(outputPath, "--output", templatePath, "--template");
  assertDistinctResolvedPaths(outputPath, "--output", evidencePath, "--evidence");
  const template = readJson(templatePath);
  const evidenceDoc = readJson(evidencePath);
  const manifest = normalizeManifest(template, evidenceDoc);
  writeJson(outputPath, manifest);
};

const proposeRouteManifest = async (args) => {
  const manifestPath = requireOption(args, "manifest");
  const mode = normalizeProposalMode(args.mode);
  const outputPath = args.output ? requireOption(args, "output") : null;
  if (args.output) {
    assertDistinctResolvedPaths(
      outputPath,
      "--output",
      manifestPath,
      "--manifest",
    );
  }
  const toriiUrl = normalizeToriiUrl(
    args["torii-url"] ?? DEFAULT_TAIRA_TORII_URL,
  );
  const manifest = readJson(manifestPath);
  const response = await fetch(
    `${toriiUrl}/v1/gov/proposals/sccp-route-manifest`,
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify({ manifest, mode }),
    },
  );
  const text = await response.text();
  if (!response.ok) {
    throw new Error(
      `TAIRA governance proposal failed HTTP ${response.status}: ${text}`,
    );
  }
  const payload = text ? JSON.parse(text) : null;
  if (outputPath) {
    writeJson(outputPath, payload);
  } else {
    console.log(JSON.stringify(payload, null, 2));
  }
};

const main = async (argv = process.argv.slice(2)) => {
  const [command, ...rest] = argv;
  if (!command || command === "--help" || command === "-h") {
    usage();
    return;
  }
  const args = parseArgs(rest);
  assertKnownOptions(command, args);
  if (command === "doctor") return doctor(args);
  if (command === "deploy") return deploy(args);
  if (command === "evidence") return evidence(args);
  if (command === "route-manifest") return routeManifest(args);
  if (command === "propose-route-manifest") return proposeRouteManifest(args);
  throw new Error("Unknown command.");
};

if (process.argv[1] && path.resolve(process.argv[1]) === SCRIPT_PATH) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}

export {
  ASSET_KEY,
  ROUTE_ID,
  SOLANA_TESTNET_CHAIN_ID_HEX,
  SOL_DOMAIN,
  main,
  normalizeBrowserProver,
  normalizeHex32,
  normalizeManifest,
};
