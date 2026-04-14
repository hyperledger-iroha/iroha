#!/usr/bin/env node
/**
 * Contract deployment helper (roadmap JS-06).
 *
 * Uploads smart-contract bytecode via Torii's alias-first deploy
 * flow. Environment variables:
 *
 *   TORII_URL (required)                — Base Torii URL (http://127.0.0.1:8080)
 *   TORII_AUTH_TOKEN / TORII_API_TOKEN  — Optional headers for locked-down nodes
 *   AUTHORITY (required)                — Account id with the relevant permissions
 *   PRIVATE_KEY / PRIVATE_KEY_HEX       — Signer key (ed25519:<hex> or raw hex)
 *   CONTRACT_CODE_PATH (required)       — Path to .to bytecode
 *   CONTRACT_ALIAS (required)           — Stable public alias (for example `router::universal`)
 *   CONTRACT_LEASE_EXPIRY_MS            — Optional alias lease expiry
 *
 * Examples:
 *   AUTHORITY=sorauロ1Nタセhjセ7pZaG9L7エmBnクbヨ9ヰsウ4dqmナコmチホ24CウオEAE9L4 \
 *   PRIVATE_KEY_HEX=fedcba... \
 *   CONTRACT_CODE_PATH=./artifacts/contract.to \
 *   CONTRACT_ALIAS=router::universal \
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
  const contractAlias = trimToNull(process.env.CONTRACT_ALIAS);
  if (!contractAlias) {
    fail("CONTRACT_ALIAS is required");
  }

  const privateKey = resolvePrivateKey();
  const codeBytes = await readCodeBytes();
  const leaseExpiryMsRaw = trimToNull(process.env.CONTRACT_LEASE_EXPIRY_MS);
  let leaseExpiryMs;
  if (leaseExpiryMsRaw !== null) {
    if (!/^\d+$/.test(leaseExpiryMsRaw)) {
      fail("CONTRACT_LEASE_EXPIRY_MS must be an unsigned integer");
    }
    leaseExpiryMs = Number.parseInt(leaseExpiryMsRaw, 10);
  }

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
    contractAlias,
    codeB64: Buffer.from(codeBytes),
    leaseExpiryMs,
  });
  if (response) {
    console.log(
      "  contract_alias:",
      response.contract_alias ?? "<unspecified>",
      "  contract_address:",
      response.contract_address ?? "<unspecified>",
      "previous_contract_address:",
      response.previous_contract_address ?? "<none>",
      "upgraded:",
      response.upgraded,
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
