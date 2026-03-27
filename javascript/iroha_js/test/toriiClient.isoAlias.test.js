import assert from "node:assert/strict";
import { generateKeyPairSync } from "node:crypto";
import test from "node:test";

import { AccountAddress, ToriiClient } from "../src/index.js";

function ed25519PublicKeyBytes() {
  const { publicKey } = generateKeyPairSync("ed25519");
  const der = publicKey.export({ format: "der", type: "spki" });
  // SPKI encoding stores the raw ed25519 key in the last 32 bytes.
  return new Uint8Array(der.subarray(der.length - 32));
}

function demoAccountId() {
  const address = AccountAddress.fromAccount({ publicKey: ed25519PublicKeyBytes() });
  return address.toI105();
}

const VALID_ACCOUNT_ID = demoAccountId();
const ALT_ACCOUNT_ID = demoAccountId();

function jsonResponse(status, body) {
  return new Response(body == null ? null : JSON.stringify(body), {
    status,
    headers: body == null ? undefined : { "Content-Type": "application/json" },
  });
}

test("resolveAliasByIndex enforces non-negative indices before issuing requests", async () => {
  let fetchCalls = 0;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async () => {
      fetchCalls += 1;
      return jsonResponse(200, {});
    },
  });

  await assert.rejects(
    () => client.resolveAliasByIndex(-5),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(error.message, /index must be a non-negative integer/);
      return true;
    },
  );
  assert.equal(fetchCalls, 0, "validation should occur before network requests");
});

test("resolveAliasByIndex rejects non-numeric indices early", async () => {
  let fetchCalls = 0;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async () => {
      fetchCalls += 1;
      return jsonResponse(200, {});
    },
  });

  await assert.rejects(
    () => client.resolveAliasByIndex("not-a-number"),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(error.message, /index must be a non-negative integer/);
      return true;
    },
  );
  assert.equal(fetchCalls, 0, "invalid indices should not trigger network calls");
});

test("resolveAlias normalises IBAN input and parses alias responses", async () => {
  let lastRequest = null;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      lastRequest = { input, init };
      assert.equal(init.method, "POST");
      const parsed = JSON.parse(init.body);
      assert.equal(
        parsed.alias,
        "DE89370400440532013000",
        "request payload should carry normalised IBAN",
      );
      return jsonResponse(200, {
        alias: "de89 3704 0044 0532 0130 00",
        account_id: VALID_ACCOUNT_ID,
        index: "42",
        source: "runtime",
      });
    },
  });

  const result = await client.resolveAlias("de89 3704 0044 0532 0130 00");
  assert.equal(result.alias, "DE89370400440532013000");
  assert.equal(result.account_id, ToriiClient._requireAccountId(VALID_ACCOUNT_ID));
  assert.equal(result.index, 42);
  assert.equal(result.source, "runtime");
  const url = new URL(lastRequest.input);
  assert.equal(url.pathname, "/v1/aliases/resolve");
  assert.equal(lastRequest.init.headers["Content-Type"], "application/json");
});

test("resolveAlias returns null for missing aliases and rejects when runtime is disabled", async () => {
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (_input, init) => {
      const parsed = JSON.parse(init.body);
      if (parsed.alias === "missing-alias") {
        return jsonResponse(404, {});
      }
      return jsonResponse(503, {});
    },
  });

  const missing = await client.resolveAlias("missing-alias");
  assert.equal(missing, null, "missing aliases should return null");

  await assert.rejects(
    () => client.resolveAlias("disabled-alias"),
    /ISO bridge runtime is disabled/,
  );
});

test("resolveAliasByIndex posts payloads and normalises responses", async () => {
  let lastRequest = null;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      lastRequest = { input, init };
      const parsed = JSON.parse(init.body);
      assert.equal(parsed.index, 7);
      return jsonResponse(200, {
        alias: "GB82 WEST 1234 5698 7654 32",
        account_id: ALT_ACCOUNT_ID,
        index: "7",
        source: "imported",
      });
    },
  });

  const result = await client.resolveAliasByIndex("7");
  assert.equal(result.alias, "GB82WEST12345698765432");
  assert.equal(result.account_id, ToriiClient._requireAccountId(ALT_ACCOUNT_ID));
  assert.equal(result.index, 7);
  assert.equal(result.source, "imported");
  const url = new URL(lastRequest.input);
  assert.equal(url.pathname, "/v1/aliases/resolve_index");
  assert.equal(lastRequest.init.method, "POST");
});

test("resolveAliasByIndex forwards service errors", async () => {
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (_input, init) => {
      const parsed = JSON.parse(init.body);
      return parsed.index === 0 ? jsonResponse(404, {}) : jsonResponse(503, {});
    },
  });

  const missing = await client.resolveAliasByIndex(0);
  assert.equal(missing, null);

  await assert.rejects(
    () => client.resolveAliasByIndex(9),
    /ISO bridge runtime is disabled/,
  );
});

test("lookupAliasesByAccount posts canonical account ids with optional filters", async () => {
  let lastRequest = null;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      lastRequest = { input, init };
      const parsed = JSON.parse(init.body);
      assert.equal(parsed.account_id, ToriiClient._requireAccountId(VALID_ACCOUNT_ID));
      assert.equal(parsed.dataspace, "sbp");
      assert.equal(parsed.domain, "hbl");
      return jsonResponse(200, {
        account_id: VALID_ACCOUNT_ID,
        total: "1",
        items: [
          {
            alias: "merchant@hbl.sbp",
            dataspace: "sbp",
            domain: "hbl",
            is_primary: true,
          },
        ],
      });
    },
  });

  const result = await client.lookupAliasesByAccount(VALID_ACCOUNT_ID, {
    dataspace: "sbp",
    domain: "hbl",
  });
  assert.equal(result.account_id, ToriiClient._requireAccountId(VALID_ACCOUNT_ID));
  assert.equal(result.total, 1);
  assert.deepEqual(result.items, [
    {
      alias: "merchant@hbl.sbp",
      dataspace: "sbp",
      domain: "hbl",
      is_primary: true,
    },
  ]);
  const url = new URL(lastRequest.input);
  assert.equal(url.pathname, "/v1/aliases/by_account");
  assert.equal(lastRequest.init.method, "POST");
});

test("lookupAliasesByAccount returns null for missing accounts", async () => {
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async () => jsonResponse(404, {}),
  });

  const result = await client.lookupAliasesByAccount(ALT_ACCOUNT_ID);
  assert.equal(result, null);
});

test("lookupAliasesByAccount validates options before issuing requests", async () => {
  let fetchCalls = 0;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async () => {
      fetchCalls += 1;
      return jsonResponse(200, {});
    },
  });

  await assert.rejects(
    () => client.lookupAliasesByAccount(VALID_ACCOUNT_ID, { unexpected: true }),
    /unsupported/i,
  );
  assert.equal(fetchCalls, 0);
});
