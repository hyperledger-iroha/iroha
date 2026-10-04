#!/usr/bin/env node

import {
  ToriiClient,
  field,
  normalizeAccountId,
} from "../src/index.js";

const BASE_URL = process.env.TORII_URL ?? "http://127.0.0.1:8080";
const ACCOUNT_LITERAL =
  process.env.ACCOUNT_ID ??
  "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB";
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

  console.log(`Listing NFTs owned by account: ${accountId}`);
  const nftPage = await client.nfts.list({
    filter: field("owned_by").eq(accountId),
    sort: "id",
    limit: 5,
  });
  for (const nft of nftPage.items) {
    console.log(" •", nft.id);
  }
  if (nftPage.nextCursor) {
    console.log("   (more NFTs: pass nextCursor as `cursor` to continue)");
  }

  console.log(`\nIterating account assets for ${accountId} (quantity >= 1)`);
  for await (const holding of client.accountAssets(accountId).iterate({
    filter: "quantity >= 1",
    select: ["asset", "quantity"],
    limit: 3,
  })) {
    console.log(`${holding.asset} => ${holding.quantity}`);
  }

  console.log("\nDone");
}

main().catch((error) => {
  console.error("nft/account iteration failed:", error);
  process.exitCode = 1;
});
