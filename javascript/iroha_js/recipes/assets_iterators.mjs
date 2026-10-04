#!/usr/bin/env node

import { ToriiClient, field } from "../src/index.js";

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
  // `iterate()` follows `next_cursor`; leaving the loop stops paging.
  for await (const holding of client.accountAssets(accountId).iterate({
    sort: "asset",
    limit: pageSize,
  })) {
    seen.push(`${holding.asset} => ${holding.quantity}`);
    if (seen.length === maxItems) break;
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
  for await (const nft of client.nfts.iterate({
    filter: nftId ? field("id").eq(nftId) : undefined,
    sort: "id",
    limit: pageSize,
  })) {
    ids.push(nft.id);
    if (ids.length === maxItems) break;
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
  await listNfts();
  await listAccountAssets();
}

main().catch((error) => {
  console.error("iterator demo failed:", error);
  process.exitCode = 1;
});
