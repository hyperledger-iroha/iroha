#!/usr/bin/env node
/**
 * ISO alias helper
 *
 * Evaluates a blinded alias element via the mock VOPRF endpoint and resolves
 * ISO aliases either by literal label (IBAN-style strings) or by deterministic
 * index. Useful for roadmap item JS-06 when exercising ISO bridge helpers from
 * CI or during manual rehearsals without writing bespoke client code.
 *
 * Usage:
 *   TORII_URL=http://localhost:8080 \
 *   ISO_VOPRF_INPUT=deadbeef ISO_ALIAS_LABEL="GB82 WEST 1234 5698 7654 32" \
 *   node ./recipes/iso_alias.mjs
 */
import { ToriiClient } from "../src/index.js";

const TORII_URL = process.env.TORII_URL ?? "http://localhost:8080";
const AUTH_TOKEN =
  process.env.TORII_AUTH_TOKEN ?? process.env.IROHA_TORII_AUTH_TOKEN ?? null;
const API_TOKEN =
  process.env.TORII_API_TOKEN ?? process.env.IROHA_TORII_API_TOKEN ?? null;

const SHOULD_EVALUATE = process.env.ISO_SKIP_VOPRF === "1" ? false : true;
const VOPRF_INPUT = (process.env.ISO_VOPRF_INPUT ?? "deadbeef").trim();
const ALIAS_LABEL = process.env.ISO_ALIAS_LABEL?.trim();
const ALIAS_INDEX_INPUT = process.env.ISO_ALIAS_INDEX?.trim();

function buildClient() {
  return new ToriiClient(TORII_URL, {
    authToken: AUTH_TOKEN ?? undefined,
    apiToken: API_TOKEN ?? undefined,
  });
}

function parseAliasIndex(value) {
  if (!value) {
    return null;
  }
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }
  try {
    if (/^0[xX]/.test(trimmed)) {
      return BigInt(trimmed);
    }
    return BigInt(trimmed);
  } catch (error) {
    throw new Error(`ISO_ALIAS_INDEX must be an integer (received '${value}')`);
  }
}

function formatResolution(record) {
  const segments = [`${record.alias} → ${record.account_id}`];
  if (record.source) {
    segments.push(`source=${record.source}`);
  }
  if (record.index !== undefined) {
    segments.push(`index=${record.index}`);
  }
  return segments.join(" ");
}

function isRuntimeDisabledError(error) {
  return (
    error instanceof Error &&
    /runtime is disabled/i.test(error.message ?? "")
  );
}

async function evaluateAliasVoprf(client, inputHex) {
  if (!SHOULD_EVALUATE) {
    console.log("Skipping VOPRF evaluation (ISO_SKIP_VOPRF=1).");
    return;
  }
  console.log(`Evaluating alias VOPRF input (${inputHex.length / 2} bytes)…`);
  const response = await client.evaluateAliasVoprf(inputHex);
  const digestPreview = response.evaluated_element_hex.slice(0, 32);
  console.log(
    `backend=${response.backend} evaluated_element_hex=${response.evaluated_element_hex}`,
  );
  console.log(`digest preview: ${digestPreview}…`);
}

async function resolveAliasLabel(client, alias) {
  if (!alias) {
    console.log(
      "No ISO_ALIAS_LABEL provided; skip literal alias resolution. Set ISO_ALIAS_LABEL to resolve IBAN-like strings.",
    );
    return;
  }
  console.log(`Resolving alias label '${alias}'…`);
  try {
    const resolved = await client.resolveAlias(alias);
    if (!resolved) {
      console.log("Alias not found (Torii returned HTTP 404).");
      return;
    }
    console.log(formatResolution(resolved));
  } catch (error) {
    if (isRuntimeDisabledError(error)) {
      console.warn(
        "Alias runtime disabled on target node; skipping literal resolution.",
      );
      return;
    }
    throw error;
  }
}

async function resolveAliasIndex(client, indexInput) {
  const parsed = parseAliasIndex(indexInput);
  if (parsed === null) {
    console.log(
      "No ISO_ALIAS_INDEX provided; skip indexed alias resolution. Set ISO_ALIAS_INDEX (decimal or 0x-prefixed) to query by index.",
    );
    return;
  }
  console.log(`Resolving alias index=${parsed}…`);
  try {
    const resolved = await client.resolveAliasByIndex(parsed);
    if (!resolved) {
      console.log("Index not found (Torii returned HTTP 404).");
      return;
    }
    console.log(formatResolution(resolved));
  } catch (error) {
    if (isRuntimeDisabledError(error)) {
      console.warn(
        "Alias runtime disabled on target node; skipping indexed resolution.",
      );
      return;
    }
    throw error;
  }
}

async function main() {
  console.log(`Torii endpoint: ${TORII_URL}`);
  const client = buildClient();
  await evaluateAliasVoprf(client, VOPRF_INPUT);
  await resolveAliasLabel(client, ALIAS_LABEL);
  await resolveAliasIndex(client, ALIAS_INDEX_INPUT);
  if (
    !SHOULD_EVALUATE &&
    !ALIAS_LABEL &&
    (ALIAS_INDEX_INPUT === undefined || ALIAS_INDEX_INPUT === null)
  ) {
    console.log(
      "Nothing to do — provide ISO_VOPRF_INPUT, ISO_ALIAS_LABEL, or ISO_ALIAS_INDEX to exercise the helpers.",
    );
  }
}

main().catch((error) => {
  console.error("ISO alias helper failed:", error);
  process.exitCode = 1;
});
