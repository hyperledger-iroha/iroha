#!/usr/bin/env node

import { ToriiClient } from "../src/index.js";

function parsePositiveInt(value, fallback) {
  const parsed = Number.parseInt(value ?? "", 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function parseBooleanFlag(value, fallback, context) {
  if (value === undefined || value === null || value === "") {
    return fallback;
  }
  if (value === "1") return true;
  if (value === "0") return false;
  throw new TypeError(`${context} must be exactly 0 or 1`);
}

const toriiUrl = process.env.TORII_URL ?? "http://localhost:8080";
const accountId =
  process.env.ACCOUNT_ID ??
  "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB";
const nftId = process.env.NFT_ID ?? null;
const pageSize = parsePositiveInt(process.env.PAGE_SIZE, 25);
const maxItemsEnv = parsePositiveInt(process.env.MAX_ITEMS, null);
const maxItems = Number.isFinite(maxItemsEnv) ? maxItemsEnv : undefined;
const hasCredentials = Boolean(
  process.env.TORII_API_TOKEN || process.env.TORII_AUTH_TOKEN,
);
const requirePermissions = parseBooleanFlag(
  process.env.TORII_REQUIRE_PERMISSIONS,
  hasCredentials,
  "TORII_REQUIRE_PERMISSIONS",
);
const allowInsecure = parseBooleanFlag(
  process.env.TORII_ALLOW_INSECURE,
  false,
  "TORII_ALLOW_INSECURE",
);

const client = new ToriiClient(toriiUrl, {
  apiToken: process.env.TORII_API_TOKEN,
  authToken: process.env.TORII_AUTH_TOKEN,
  allowInsecure,
});

async function listAccountAssets() {
  console.log(`\nAccount assets for ${accountId} (pageSize=${pageSize}, maxItems=${maxItems ?? "∞"})`);
  const seen = [];
  for await (const holding of client.iterateAccountAssets(accountId, {
    requirePermissions,
    pageSize,
    maxItems,
    sort: [{ key: "asset_id", order: "asc" }],
  })) {
    seen.push(`${holding.asset_id} => ${holding.quantity}`);
  }
  if (seen.length === 0) {
    console.log("(no holdings returned)");
    return;
  }
  for (const entry of seen) {
    console.log(`- ${entry}`);
  }
}

async function listNfts() {
  console.log(`\nNFTs${nftId ? ` matching ${nftId}` : ""} (pageSize=${pageSize}, maxItems=${maxItems ?? "∞"})`);
  const ids = [];
  for await (const nft of client.iterateNftsQuery({
    requirePermissions,
    pageSize,
    maxItems,
    filter: nftId ? { Eq: ["id", nftId] } : undefined,
    sort: [{ key: "id", order: "asc" }],
  })) {
    ids.push(nft.id);
  }
  if (ids.length === 0) {
    console.log("(no NFTs returned)");
    return;
  }
  for (const id of ids) {
    console.log(`- ${id}`);
  }
}

async function main() {
  console.log(`Torii endpoint: ${toriiUrl}`);
  if (!process.env.TORII_API_TOKEN && !process.env.TORII_AUTH_TOKEN) {
    console.warn("warning: no API/auth token provided; secured deployments may reject requests");
  }
  await listNfts();
  await listAccountAssets();
}

main().catch((error) => {
  console.error("iterator demo failed:", error);
  process.exitCode = 1;
});
