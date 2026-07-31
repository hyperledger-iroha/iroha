import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { ToriiClient } from "../src/toriiClient.js";

const repoListFixture = JSON.parse(
  readFileSync(new URL("./fixtures/torii_responses.json", import.meta.url), "utf8"),
).repo.list;

test("repo agreement normalization exposes lifecycle and custody fields", async () => {
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () =>
      new Response(JSON.stringify(repoListFixture), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
  });

  const agreement = (await client.listRepoAgreements({ limit: 2 })).items[0];
  assert.match(agreement.cashSource, /^7EAD8EFYUx1aVKZPUU1fyKvr8dF1@/);
  assert.match(agreement.collateralCustodyAsset, /^4fEiy2n5VMFVfi6BzDJge519zAzg@/);
  assert.equal(agreement.settlementTimestampMs, null);
  assert.equal(agreement.status, "active");
});
