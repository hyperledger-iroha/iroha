#!/usr/bin/env node
/**
 * Contract deployment helper (roadmap JS-06).
 *
 * Uploads smart-contract manifests/bytecode via Torii's REST API and optionally
 * activates the instance in a single run. Environment variables:
 *
 *   TORII_URL (required)                — Base Torii URL (http://127.0.0.1:8080)
 *   TORII_AUTH_TOKEN / TORII_API_TOKEN  — Optional headers for locked-down nodes
 *   AUTHORITY (required)                — Account id with the relevant permissions
 *   PRIVATE_KEY / PRIVATE_KEY_HEX       — Signer key (ed25519:<hex> or raw hex)
 *   CONTRACT_CODE_PATH (required)       — Path to .to bytecode
 *   CONTRACT_MANIFEST_PATH              — Optional path to manifest JSON
 *   CONTRACT_MANIFEST_JSON              — Inline manifest JSON string (overrides path)
 *   CONTRACT_NAMESPACE / CONTRACT_ID    — Required when CONTRACT_STAGE includes "instance"
 *   CONTRACT_STAGE                      — "both" (default), "register", or "instance"
 *
 * Examples:
 *   CONTRACT_STAGE=register \
 *   AUTHORITY=alice@wonderland \
 *   PRIVATE_KEY_HEX=fedcba... \
 *   CONTRACT_CODE_PATH=./artifacts/contract.to \
 *   node javascript/iroha_js/recipes/contracts.mjs
 *
 *   CONTRACT_STAGE=instance \
 *   CONTRACT_NAMESPACE=apps \
 *   CONTRACT_ID=ledger \
 *   CONTRACT_CODE_PATH=./artifacts/contract.to \
 *   CONTRACT_MANIFEST_PATH=./manifest.json \
 *   node javascript/iroha_js/recipes/contracts.mjs
 */
import { Buffer } from "node:buffer";
import { readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import { ToriiClient } from "../src/index.js";

function fail(message) {
  console.error(`[contracts] ${message}`);
  process.exitCode = 1;
  throw new Error(message);
}

function selectStage(value = "both") {
  const normalized = value.trim().toLowerCase();
  switch (normalized) {
    case "":
    case "both":
      return { register: true, activate: true };
    case "register":
    case "code":
    case "upload":
      return { register: true, activate: false };
    case "instance":
    case "activate":
      return { register: false, activate: true };
    default:
      fail(
        `unsupported CONTRACT_STAGE="${value}". Use "register", "instance", or "both".`,
      );
  }
}

function trimToNull(value) {
  if (value === undefined || value === null) {
    return null;
  }
  const trimmed = String(value).trim();
  return trimmed.length === 0 ? null : trimmed;
}

function resolvePrivateKey() {
  const explicit = trimToNull(process.env.PRIVATE_KEY);
  if (explicit) {
    return explicit;
  }
  const hex = trimToNull(process.env.PRIVATE_KEY_HEX);
  if (hex) {
    const sanitized = hex.startsWith("0x") ? hex.slice(2) : hex;
    if (!/^[0-9a-fA-F]+$/.test(sanitized) || sanitized.length % 2 !== 0) {
      fail("PRIVATE_KEY_HEX must be an even-length hex string");
    }
    return `ed25519:${sanitized}`;
  }
  fail("PRIVATE_KEY or PRIVATE_KEY_HEX is required");
}

async function readJsonFile(filePath) {
  const resolved = path.resolve(filePath);
  const raw = await readFile(resolved, "utf8");
  try {
    const parsed = JSON.parse(raw);
    if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
      fail(`manifest at ${resolved} must be a JSON object`);
    }
    return parsed;
  } catch (error) {
    fail(`failed to parse manifest JSON at ${resolved}: ${error?.message ?? error}`);
  }
}

async function loadManifest() {
  const inline = trimToNull(process.env.CONTRACT_MANIFEST_JSON);
  if (inline) {
    try {
      const parsed = JSON.parse(inline);
      if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
        fail("CONTRACT_MANIFEST_JSON must encode a JSON object");
      }
      return parsed;
    } catch (error) {
      fail(
        `CONTRACT_MANIFEST_JSON is not valid JSON: ${error?.message ?? error}`,
      );
    }
  }
  const manifestPath = trimToNull(process.env.CONTRACT_MANIFEST_PATH);
  if (manifestPath) {
    return readJsonFile(manifestPath);
  }
  return null;
}

async function readCodeBytes() {
  const filePath = trimToNull(process.env.CONTRACT_CODE_PATH);
  if (!filePath) {
    fail("CONTRACT_CODE_PATH is required");
  }
  const resolved = path.resolve(filePath);
  try {
    return await readFile(resolved);
  } catch (error) {
    fail(`failed to read bytecode at ${resolved}: ${error?.message ?? error}`);
  }
}

async function main() {
  const toriiUrl = trimToNull(process.env.TORII_URL) ?? "http://localhost:8080";
  const authority = trimToNull(process.env.AUTHORITY);
  if (!authority) {
    fail("AUTHORITY is required");
  }

  const stage = selectStage(process.env.CONTRACT_STAGE ?? "both");
  const privateKey = resolvePrivateKey();
  const manifest = await loadManifest();
  const codeBytes = await readCodeBytes();

  const clientOptions = {};
  const authToken = trimToNull(process.env.TORII_AUTH_TOKEN);
  if (authToken) {
    clientOptions.authToken = authToken;
  }
  const apiToken = trimToNull(process.env.TORII_API_TOKEN);
  if (apiToken) {
    clientOptions.apiToken = apiToken;
  }
  const client = new ToriiClient(toriiUrl, clientOptions);

  if (stage.register) {
    console.log("[contracts] uploading manifest + bytecode via /v1/contracts/deploy");
    const response = await client.deployContract({
      authority,
      privateKey,
      codeB64: Buffer.from(codeBytes),
      manifest: manifest ?? undefined,
    });
    if (response) {
      console.log(
        "  code_hash_hex:",
        response.code_hash_hex ?? "<unspecified>",
        "abi_hash_hex:",
        response.abi_hash_hex ?? "<unspecified>",
      );
    } else {
      console.log("  Torii returned 202 Accepted (no body)");
    }
  }

  if (stage.activate) {
    const namespace =
      trimToNull(process.env.CONTRACT_NAMESPACE) ?? fail("CONTRACT_NAMESPACE is required");
    const contractId =
      trimToNull(process.env.CONTRACT_ID) ?? fail("CONTRACT_ID is required");
    console.log(
      "[contracts] activating instance via /v1/contracts/instance",
      `${namespace}::${contractId}`,
    );
    const response = await client.deployContractInstance({
      authority,
      privateKey,
      namespace,
      contractId,
      codeB64: Buffer.from(codeBytes),
      manifest: manifest ?? undefined,
    });
    if (response) {
      console.log(
        "  namespace:",
        response.namespace,
        "contract_id:",
        response.contract_id,
        "code_hash_hex:",
        response.code_hash_hex ?? "<unspecified>",
      );
    } else {
      console.log("  Torii returned 202 Accepted (no body)");
    }
  }
}

main().catch((error) => {
  console.error("[contracts] failed:", error?.stack ?? error);
  process.exitCode = 1;
});
