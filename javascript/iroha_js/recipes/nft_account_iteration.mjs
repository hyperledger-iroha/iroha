#!/usr/bin/env node

import {
  ToriiClient,
  normalizeAccountId,
} from "../src/index.js";

const BASE_URL = process.env.TORII_URL ?? "http://127.0.0.1:8080";
const ACCOUNT_LITERAL =
  process.env.ACCOUNT_ID ??
  "6cmzPVPX5ZhYaa7sushd7mC66PG1BrtMPRnpi9p3suF2mFeiR1ekAkT";
const AUTH_TOKEN = process.env.TORII_AUTH_TOKEN ?? null;
const API_TOKEN = process.env.TORII_API_TOKEN ?? null;
const ALLOW_INSECURE = process.env.TORII_ALLOW_INSECURE === "1";

async function main() {
  const client = new ToriiClient(BASE_URL, {
    authToken: AUTH_TOKEN,
    apiToken: API_TOKEN,
    allowInsecure: ALLOW_INSECURE,
  });
  const accountId = normalizeAccountId(ACCOUNT_LITERAL);

  console.log(`Listing NFTs for account: ${accountId}`);
  const nftPage = await client.listNfts({
    requirePermissions: true,
    limit: 5,
    sort: [{ key: "id", order: "asc" }],
  });
  for (const nft of nftPage.items) {
    console.log(" •", nft.id);
  }

  console.log(`\nIterating account assets for ${accountId} (quantity >= 1)`);
  for await (const holding of client.iterateAccountAssetsQuery(accountId, {
    requirePermissions: true,
    pageSize: 3,
    filter: { Gte: ["quantity", 1] },
    select: [{ Fields: ["asset_id", "quantity"] }],
    addressFormat: "compressed",
  })) {
    console.log(`${holding.asset_id} => ${holding.quantity}`);
  }

  console.log("\nDone");
}

main().catch((error) => {
  console.error("nft/account iteration failed:", error);
  process.exitCode = 1;
});
