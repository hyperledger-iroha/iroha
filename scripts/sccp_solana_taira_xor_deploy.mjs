#!/usr/bin/env node
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const DEFAULT_SOLANA_RPC_URL =
  process.env.SCCP_SOLANA_TESTNET_RPC_URL ||
  process.env.SOLANA_RPC_URL ||
  "https://api.testnet.solana.com";
const DEFAULT_TAIRA_TORII_URL =
  process.env.SCCP_TAIRA_TORII_URL || "https://taira.sora.org";
const SOLANA_TESTNET_CHAIN_ID_HEX =
  "0x736f6c616e612d746573746e6574";
const ROUTE_ID = "taira_sol_xor";
const ASSET_KEY = "xor";
const SOL_DOMAIN = 3;

const usage = () => {
  console.log(`Usage: node scripts/sccp_solana_taira_xor_deploy.mjs <command> [options]

Commands:
  doctor
    Check Solana CLI, Solana testnet RPC, and TAIRA governance endpoint readiness.

  deploy --program-so PATH --program-id-keypair PATH --keypair PATH --broadcast true --confirm-testnet solana-testnet
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
      throw new Error(`Unexpected positional argument: ${token}`);
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
};

const requireOption = (args, key) => {
  const value = args[key];
  if (typeof value !== "string" || !value.trim()) {
    throw new Error(`--${key} is required`);
  }
  return value.trim();
};

const readJson = (file) => JSON.parse(fs.readFileSync(file, "utf8"));

const writeJson = (file, value) => {
  fs.mkdirSync(path.dirname(path.resolve(file)), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
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

const sha256Hex = (value) =>
  createHash("sha256").update(value).digest("hex");

const normalizeHex32 = (value, label) => {
  const body = String(value ?? "").trim().replace(/^0x/u, "").toLowerCase();
  if (!/^[0-9a-f]{64}$/u.test(body)) {
    throw new Error(`${label} must be a 32-byte lowercase hex value`);
  }
  return body;
};

const normalizeRequiredString = (value, label) => {
  const text = String(value ?? "").trim();
  if (!text) throw new Error(`${label} is required`);
  return text;
};

const normalizeBrowserProver = (value, label) => {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  return {
    module_url: normalizeRequiredString(value.module_url ?? value.moduleUrl, `${label}.module_url`),
    module_specifier:
      value.module_specifier ?? value.moduleSpecifier
        ? normalizeRequiredString(
            value.module_specifier ?? value.moduleSpecifier,
            `${label}.module_specifier`,
          )
        : null,
    module_hash: normalizeHex32(value.module_hash ?? value.moduleHash, `${label}.module_hash`),
    manifest_hash: normalizeHex32(
      value.manifest_hash ?? value.manifestHash,
      `${label}.manifest_hash`,
    ),
    expected_exports: Array.isArray(value.expected_exports ?? value.expectedExports)
      ? (value.expected_exports ?? value.expectedExports).map((entry, index) =>
          normalizeRequiredString(entry, `${label}.expected_exports[${index}]`),
        )
      : [],
    bound_route_hash: normalizeHex32(
      value.bound_route_hash ?? value.boundRouteHash,
      `${label}.bound_route_hash`,
    ),
    bound_proof_hash: normalizeHex32(
      value.bound_proof_hash ?? value.boundProofHash,
      `${label}.bound_proof_hash`,
    ),
  };
};

const normalizeManifest = (template, evidence) => {
  const manifest = { ...template };
  manifest.version = Number(manifest.version ?? 1);
  manifest.route_id = normalizeRequiredString(
    manifest.route_id ?? manifest.routeId ?? ROUTE_ID,
    "route_id",
  );
  manifest.asset_key = normalizeRequiredString(
    manifest.asset_key ?? manifest.assetKey ?? ASSET_KEY,
    "asset_key",
  );
  manifest.tron_network = normalizeRequiredString(
    manifest.tron_network ?? manifest.tronNetwork ?? "testnet",
    "tron_network",
  );
  manifest.chain = normalizeRequiredString(
    manifest.chain ?? "solana-testnet",
    "chain",
  );
  manifest.chain_id_hex = normalizeRequiredString(
    manifest.chain_id_hex ?? manifest.chainIdHex ?? SOLANA_TESTNET_CHAIN_ID_HEX,
    "chain_id_hex",
  );
  manifest.counterparty_account_codec = Number(
    manifest.counterparty_account_codec ?? manifest.counterpartyAccountCodec ?? 6,
  );
  manifest.counterparty_account_codec_key = normalizeRequiredString(
    manifest.counterparty_account_codec_key ??
      manifest.counterpartyAccountCodecKey ??
      "solana_base58",
    "counterparty_account_codec_key",
  );
  manifest.counterparty_domain = Number(
    manifest.counterparty_domain ?? manifest.counterpartyDomain ?? SOL_DOMAIN,
  );
  manifest.verifier_target = normalizeRequiredString(
    manifest.verifier_target ?? manifest.verifierTarget ?? "SolanaProgram",
    "verifier_target",
  );
  manifest.production_ready = Boolean(
    manifest.production_ready ?? manifest.productionReady,
  );
  manifest.network_id_hex = normalizeRequiredString(
    manifest.network_id_hex ?? manifest.networkIdHex ?? SOLANA_TESTNET_CHAIN_ID_HEX,
    "network_id_hex",
  );

  const programId = evidence.programId ?? evidence.program_id ?? evidence.program;
  if (programId && !manifest.taira_xor_bridge_address) {
    manifest.taira_xor_bridge_address = programId;
  }

  for (const [field, label] of [
    ["taira_xor_token_address", "Solana XOR token mint"],
    ["taira_xor_bridge_address", "Solana bridge program"],
    ["sccp_tron_source_bridge_address", "Solana source bridge program"],
    ["tron_verifier_address", "Solana verifier program"],
    ["destination_binding_key", "destination binding key"],
    ["taira_burn_record_settlement_asset_definition_id", "TAIRA settlement asset definition id"],
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

  manifest.taira_burn_record_gas_limit = Number(
    manifest.taira_burn_record_gas_limit ?? 2_000_000,
  );
  if (!Number.isSafeInteger(manifest.taira_burn_record_gas_limit)) {
    throw new Error("taira_burn_record_gas_limit must be an integer");
  }
  manifest.destination_browser_prover = normalizeBrowserProver(
    manifest.destination_browser_prover ?? manifest.destinationBrowserProver,
    "destination_browser_prover",
  );
  manifest.source_browser_prover = normalizeBrowserProver(
    manifest.source_browser_prover ?? manifest.sourceBrowserProver,
    "source_browser_prover",
  );
  manifest.source_verifier_material =
    manifest.source_verifier_material ?? manifest.sourceVerifierMaterial ?? null;
  manifest.source_adapter_engine_deployment = {
    ...(manifest.source_adapter_engine_deployment ??
      manifest.sourceAdapterEngineDeployment ??
      {}),
    solana_programdata_address:
      evidence.programDataAddress ?? evidence.programdataAddress ?? null,
    solana_programdata_slot:
      evidence.programDataSlot ?? evidence.programdataSlot ?? null,
    solana_program_account_sha256:
      evidence.programAccountDataSha256 ?? evidence.program_account_data_sha256 ?? null,
  };
  manifest.source_adapter_engine =
    manifest.source_adapter_engine ?? manifest.sourceAdapterEngine ?? null;

  if (
    manifest.route_id !== ROUTE_ID ||
    manifest.asset_key !== ASSET_KEY ||
    manifest.counterparty_domain !== SOL_DOMAIN ||
    manifest.chain !== "solana-testnet" ||
    manifest.chain_id_hex !== SOLANA_TESTNET_CHAIN_ID_HEX ||
    manifest.verifier_target !== "SolanaProgram" ||
    manifest.counterparty_account_codec_key !== "solana_base58"
  ) {
    throw new Error("manifest is not the canonical taira_sol_xor Solana testnet route");
  }
  if (!manifest.production_ready) {
    throw new Error("route-manifest refuses to publish production_ready=false");
  }
  return manifest;
};

const doctor = async (args) => {
  const solanaRpcUrl = args["solana-rpc-url"] || DEFAULT_SOLANA_RPC_URL;
  const toriiUrl = args["torii-url"] || DEFAULT_TAIRA_TORII_URL;
  const checks = [];
  checks.push({ name: "solana-cli", ok: commandExists("solana") });
  checks.push({ name: "anchor-cli", ok: commandExists("anchor"), optional: true });
  try {
    const health = await rpc(solanaRpcUrl, "getHealth");
    checks.push({ name: "solana-testnet-rpc", ok: health === "ok", evidence: health });
  } catch (error) {
    checks.push({ name: "solana-testnet-rpc", ok: false, error: error.message });
  }
  try {
    const response = await fetch(`${toriiUrl.replace(/\/+$/u, "")}/openapi.json`);
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
  console.log(JSON.stringify({ ok: checks.every((check) => check.ok || check.optional), checks }, null, 2));
};

const deploy = (args) => {
  if (args.broadcast !== "true" || args["confirm-testnet"] !== "solana-testnet") {
    throw new Error(
      "deploy requires --broadcast true --confirm-testnet solana-testnet",
    );
  }
  const output = run("solana", [
    "program",
    "deploy",
    "--url",
    args["solana-rpc-url"] || DEFAULT_SOLANA_RPC_URL,
    "--keypair",
    requireOption(args, "keypair"),
    "--program-id",
    requireOption(args, "program-id-keypair"),
    requireOption(args, "program-so"),
  ]);
  console.log(output);
};

const evidence = (args) => {
  const programId = requireOption(args, "program-id");
  const output = run("solana", [
    "program",
    "show",
    "--url",
    args["solana-rpc-url"] || DEFAULT_SOLANA_RPC_URL,
    "--output",
    "json",
    programId,
  ]);
  const parsed = JSON.parse(output);
  const evidenceDoc = {
    schema: "sccp-solana-taira-xor-program-evidence/v1",
    programId,
    programDataAddress:
      parsed.programDataAddress ?? parsed["ProgramData Address"] ?? null,
    programDataSlot: parsed.lastDeploySlot ?? parsed["Last Deployed In Slot"] ?? null,
    authority: parsed.authority ?? parsed["Authority"] ?? null,
    raw: parsed,
    evidenceSha256: sha256Hex(output),
  };
  writeJson(requireOption(args, "output"), evidenceDoc);
};

const routeManifest = (args) => {
  const template = readJson(requireOption(args, "template"));
  const evidenceDoc = readJson(requireOption(args, "evidence"));
  const manifest = normalizeManifest(template, evidenceDoc);
  writeJson(requireOption(args, "output"), manifest);
};

const proposeRouteManifest = async (args) => {
  const manifest = readJson(requireOption(args, "manifest"));
  const toriiUrl = args["torii-url"] || DEFAULT_TAIRA_TORII_URL;
  const response = await fetch(
    `${toriiUrl.replace(/\/+$/u, "")}/v1/gov/proposals/sccp-route-manifest`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify({ manifest, mode: args.mode || "Plain" }),
    },
  );
  const text = await response.text();
  if (!response.ok) {
    throw new Error(`TAIRA governance proposal failed HTTP ${response.status}: ${text}`);
  }
  const payload = text ? JSON.parse(text) : null;
  if (args.output) {
    writeJson(args.output, payload);
  } else {
    console.log(JSON.stringify(payload, null, 2));
  }
};

const main = async () => {
  const [command, ...rest] = process.argv.slice(2);
  if (!command || command === "--help" || command === "-h") {
    usage();
    return;
  }
  const args = parseArgs(rest);
  if (command === "doctor") return doctor(args);
  if (command === "deploy") return deploy(args);
  if (command === "evidence") return evidence(args);
  if (command === "route-manifest") return routeManifest(args);
  if (command === "propose-route-manifest") return proposeRouteManifest(args);
  throw new Error(`Unknown command: ${command}`);
};

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
