#!/usr/bin/env node
/**
 * Contract deployment helper (roadmap JS-06).
 *
 * Uploads smart-contract manifests/bytecode via Torii's address-first deploy
 * flow. Environment variables:
 *
 *   TORII_URL (required)                — Base Torii URL (http://127.0.0.1:8080)
 *   TORII_AUTH_TOKEN / TORII_API_TOKEN  — Optional headers for locked-down nodes
 *   AUTHORITY (required)                — Account id with the relevant permissions
 *   PRIVATE_KEY / PRIVATE_KEY_HEX       — Signer key (ed25519:<hex> or raw hex)
 *   CONTRACT_CODE_PATH (required)       — Path to .to bytecode
 *   CONTRACT_MANIFEST_PATH              — Optional path to manifest JSON
 *   CONTRACT_MANIFEST_JSON              — Inline manifest JSON string (overrides path)
 *   CONTRACT_DATASPACE                  — Optional dataspace alias (defaults to `universal`)
 *
 * Examples:
 *   AUTHORITY=sorauロ1Nタセhjセ7pZaG9L7エmBnクbヨ9ヰsウ4dqmナコmチホ24CウオEAE9L4 \
 *   PRIVATE_KEY_HEX=fedcba... \
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

  const privateKey = resolvePrivateKey();
  const manifest = await loadManifest();
  const codeBytes = await readCodeBytes();
  const dataspace = trimToNull(process.env.CONTRACT_DATASPACE);

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

  console.log("[contracts] deploying bytecode via /v1/contracts/deploy");
  const response = await client.deployContract({
    authority,
    privateKey,
    dataspace: dataspace ?? undefined,
    codeB64: Buffer.from(codeBytes),
    manifest: manifest ?? undefined,
  });
  if (response) {
    console.log(
      "  contract_address:",
      response.contract_address ?? "<unspecified>",
      "dataspace:",
      response.dataspace ?? "<unspecified>",
      "tx_hash_hex:",
      response.tx_hash_hex ?? "<unspecified>",
    );
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

main().catch((error) => {
  console.error("[contracts] failed:", error?.stack ?? error);
  process.exitCode = 1;
});
