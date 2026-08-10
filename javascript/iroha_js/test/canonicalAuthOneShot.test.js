import assert from "node:assert/strict";
import test from "node:test";
import { ed25519 } from "@noble/curves/ed25519";

import {
  LocalSigningContext,
  NetworkId,
  ToriiClient,
} from "../src/index.js";
import { AccountAddress } from "../src/address.js";

const NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xa5));
const SIGNING_CONTEXT = new LocalSigningContext(NETWORK_ID);
const privateKey = Buffer.alloc(32, 0x29);
const publicKey = Buffer.from(ed25519.getPublicKey(privateKey));
const accountId = AccountAddress.fromAccount({ publicKey }).toI105();

function createClient(fetchImpl) {
  return new ToriiClient("https://torii.example", {
    fetchImpl,
    localSigningContext: SIGNING_CONTEXT,
    maxRetries: 8,
  });
}

async function signedRead(client) {
  return client.listAccountAssets(accountId, {
    canonicalAuth: { accountId, privateKey },
    limit: 1,
  });
}

for (const status of [307, 308, 503]) {
  test(`canonical auth status ${status} is surfaced without replay`, async () => {
    let calls = 0;
    const client = createClient(async (_url, init) => {
      calls += 1;
      assert.equal(init.redirect, "error");
      return new Response("failure", { status });
    });

    await assert.rejects(() => signedRead(client));
    assert.equal(calls, 1);
  });
}

test("canonical auth network failure is surfaced without retry", async () => {
  let calls = 0;
  const client = createClient(async (_url, init) => {
    calls += 1;
    assert.equal(init.redirect, "error");
    throw new Error("ambiguous canonical-auth transport failure");
  });

  await assert.rejects(
    () => signedRead(client),
    /ambiguous canonical-auth transport failure/u,
  );
  assert.equal(calls, 1);
});
