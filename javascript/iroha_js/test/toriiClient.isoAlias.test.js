import assert from "node:assert/strict";
import { generateKeyPairSync } from "node:crypto";
import test from "node:test";

import {
  AccountAddress,
  ToriiClient,
  canonicalRequestSignatureMessage,
  generateKeyPair,
  verifyEd25519,
} from "../src/index.js";

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

test("resolveAlias normalises IBAN input and requires canonical alias responses", async () => {
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
        alias: "DE89370400440532013000",
        account_id: VALID_ACCOUNT_ID,
        index: 42,
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

test("resolveAlias attaches canonical auth when provided", async () => {
  const { privateKey, publicKey } = generateKeyPair({ seed: Buffer.alloc(32, 12) });
  const signerAccountId = AccountAddress.fromAccount({ publicKey }).toI105();
  let lastRequest = null;
  const fetchImpl = async (input, init) => {
    lastRequest = { input, init };
    return jsonResponse(200, {
      alias: "tidal-river-4160@mibank.paynet",
      account_id: VALID_ACCOUNT_ID,
      source: "runtime",
    });
  };
  fetchImpl.__irohaSupportsRawUtf8Headers = true;
  const client = new ToriiClient("https://example.test", {
    fetchImpl,
  });

  const result = await client.resolveAlias("tidal-river-4160@mibank.paynet", {
    canonicalAuth: { accountId: signerAccountId, privateKey },
  });

  assert.equal(result.account_id, ToriiClient._requireAccountId(VALID_ACCOUNT_ID));
  assert.equal(lastRequest.init.headers["X-Iroha-Account"], signerAccountId);
  const url = new URL(lastRequest.input);
  const timestampMs = Number(lastRequest.init.headers["X-Iroha-Timestamp-Ms"]);
  const nonce = lastRequest.init.headers["X-Iroha-Nonce"];
  const message = canonicalRequestSignatureMessage({
    method: lastRequest.init.method,
    path: url.pathname,
    body: lastRequest.init.body,
    timestampMs,
    nonce,
  });
  const signature = Buffer.from(lastRequest.init.headers["X-Iroha-Signature"], "base64");
  assert.equal(verifyEd25519(message, signature, publicKey), true);
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

test("resolveAliasByIndex posts payloads and requires canonical responses", async () => {
  let lastRequest = null;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      lastRequest = { input, init };
      const parsed = JSON.parse(init.body);
      assert.equal(parsed.index, 7);
      return jsonResponse(200, {
        alias: "GB82WEST12345698765432",
        account_id: ALT_ACCOUNT_ID,
        index: 7,
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
  assert.equal(url.pathname, "/v1/aliases/resolve-index");
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
      assert.equal(parsed.dataspace, "centralbank");
      assert.equal(parsed.domain, "banka");
      return jsonResponse(200, {
        account_id: VALID_ACCOUNT_ID,
        total: "1",
        items: [
          {
            alias: "merchant@banka.centralbank",
            dataspace: "centralbank",
            domain: "banka",
            is_primary: true,
          },
        ],
      });
    },
  });

  const result = await client.lookupAliasesByAccount(VALID_ACCOUNT_ID, {
    dataspace: "centralbank",
    domain: "banka",
  });
  assert.equal(result.account_id, ToriiClient._requireAccountId(VALID_ACCOUNT_ID));
  assert.equal(result.total, 1);
  assert.deepEqual(result.items, [
    {
      alias: "merchant@banka.centralbank",
      dataspace: "centralbank",
      domain: "banka",
      is_primary: true,
    },
  ]);
  const url = new URL(lastRequest.input);
  assert.equal(url.pathname, "/v1/aliases/by-account");
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

test("lookupRetailRecipient posts canonical account and alias payloads", async () => {
  let lastRequest = null;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      lastRequest = { input, init };
      const parsed = JSON.parse(init.body);
      assert.equal(parsed.account_id, ToriiClient._requireAccountId(VALID_ACCOUNT_ID));
      assert.equal(parsed.alias_fqn, "payee@hbl.sbp");
      return jsonResponse(200, {
        resolved: true,
        account_id: VALID_ACCOUNT_ID,
        alias_fqn: "payee@hbl.sbp",
        fi_id: "hbl.sbp",
        full_name: "Ayesha Khan",
      });
    },
  });

  const result = await client.lookupRetailRecipient({
    accountId: VALID_ACCOUNT_ID,
    aliasFqn: "payee@hbl.sbp",
  });

  assert.deepEqual(result, {
    resolved: true,
    account_id: ToriiClient._requireAccountId(VALID_ACCOUNT_ID),
    alias_fqn: "payee@hbl.sbp",
    fi_id: "hbl.sbp",
    full_name: "Ayesha Khan",
  });
  const url = new URL(lastRequest.input);
  assert.equal(url.pathname, "/v1/retail/recipients/lookup");
  assert.equal(lastRequest.init.method, "POST");
});

test("lookupRetailRecipient binds the response to the requested account and alias", async () => {
  for (const payload of [
    {
      resolved: false,
      account_id: ALT_ACCOUNT_ID,
      alias_fqn: "payee@hbl.sbp",
      fi_id: "hbl.sbp",
    },
    {
      resolved: false,
      account_id: VALID_ACCOUNT_ID,
      alias_fqn: "payee@ubl.sbp",
      fi_id: "ubl.sbp",
    },
  ]) {
    const client = new ToriiClient("https://example.test", {
      fetchImpl: async () => jsonResponse(200, payload),
    });
    await assert.rejects(
      () =>
        client.lookupRetailRecipient({
          accountId: VALID_ACCOUNT_ID,
          aliasFqn: "payee@hbl.sbp",
        }),
      /does not match the requested account and alias/i,
    );
  }
});

test("lookupRetailRecipient rejects noncanonical aliases before issuing a request", async () => {
  let fetchCalls = 0;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async () => {
      fetchCalls += 1;
      return jsonResponse(200, {});
    },
  });
  await assert.rejects(
    () =>
      client.lookupRetailRecipient({
        accountId: VALID_ACCOUNT_ID,
        aliasFqn: "Payee@hbl.sbp",
      }),
    /canonical/i,
  );
  assert.equal(fetchCalls, 0);
});

test("lookupRetailRecipient rejects conflicting request aliases", async () => {
  let fetchCalls = 0;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async () => {
      fetchCalls += 1;
      return jsonResponse(200, {});
    },
  });
  await assert.rejects(
    () =>
      client.lookupRetailRecipient({
        accountId: VALID_ACCOUNT_ID,
        account_id: ALT_ACCOUNT_ID,
        aliasFqn: "payee@hbl.sbp",
      }),
    /accountId and account_id must match/i,
  );
  await assert.rejects(
    () =>
      client.lookupRetailRecipient({
        accountId: VALID_ACCOUNT_ID,
        aliasFqn: "payee@hbl.sbp",
        alias_fqn: "other@hbl.sbp",
      }),
    /aliasFqn and alias_fqn must match/i,
  );
  assert.equal(fetchCalls, 0);
});

test("routeRetailRecipient returns the exact privacy-minimized route shape", async () => {
  let lastRequest = null;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      lastRequest = { input, init };
      assert.deepEqual(JSON.parse(init.body), {
        account_id: ToriiClient._requireAccountId(ALT_ACCOUNT_ID),
      });
      return jsonResponse(200, {
        account_id: ALT_ACCOUNT_ID,
        alias_fqn: "payee@ubl.sbp",
        fi_id: "ubl.sbp",
      });
    },
  });

  const result = await client.routeRetailRecipient(ALT_ACCOUNT_ID);

  assert.deepEqual(result, {
    account_id: ToriiClient._requireAccountId(ALT_ACCOUNT_ID),
    alias_fqn: "payee@ubl.sbp",
    fi_id: "ubl.sbp",
  });
  assert.deepEqual(Object.keys(result).sort(), ["account_id", "alias_fqn", "fi_id"]);
  assert.equal(new URL(lastRequest.input).pathname, "/v1/retail/recipients/route");
  assert.equal(lastRequest.init.method, "POST");
});

test("routeRetailRecipient rejects noncanonical route payloads", async () => {
  for (const payload of [
    {
      account_id: ALT_ACCOUNT_ID,
      alias_fqn: "Payee@ubl.sbp",
      fi_id: "ubl.sbp",
    },
    {
      account_id: ALT_ACCOUNT_ID,
      alias_fqn: "payee@ubl.sbp",
      fi_id: "ubl.sbp",
      extra: true,
    },
  ]) {
    const client = new ToriiClient("https://example.test", {
      fetchImpl: async () => jsonResponse(200, payload),
    });
    await assert.rejects(() => client.routeRetailRecipient(ALT_ACCOUNT_ID), /canonical|unsupported/i);
  }
});

test("routeRetailRecipient binds the response to the requested account", async () => {
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async () =>
      jsonResponse(200, {
        account_id: VALID_ACCOUNT_ID,
        alias_fqn: "payee@ubl.sbp",
        fi_id: "ubl.sbp",
      }),
  });
  await assert.rejects(
    () => client.routeRetailRecipient(ALT_ACCOUNT_ID),
    /does not match the requested account/i,
  );
});

test("findFeeSponsorPolicyById posts the exact id and returns the direct policy", async () => {
  const canonicalSponsor = ToriiClient._requireAccountId(VALID_ACCOUNT_ID);
  const policy = {
    id: { sponsor: canonicalSponsor, name: "wallet_fx" },
    enabled: true,
    rules: [
      {
        effect: { effect: "allow", value: null },
        dataspaces: [0],
        executable_kinds: [{ kind: "instructions", value: null }],
        instruction_wire_ids: ["iroha.settlement.fx_corridor.settle"],
        contract_selectors: [],
      },
    ],
  };
  let lastRequest = null;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      lastRequest = { input, init };
      assert.deepEqual(JSON.parse(init.body), {
        sponsor_account_id: canonicalSponsor,
        policy_name: "wallet_fx",
      });
      return jsonResponse(200, policy);
    },
  });

  const result = await client.findFeeSponsorPolicyById(VALID_ACCOUNT_ID, "wallet_fx");

  assert.deepEqual(result, policy);
  assert.equal(
    new URL(lastRequest.input).pathname,
    "/v1/fee-sponsor-policies/by-id",
  );
  assert.equal(lastRequest.init.method, "POST");
});

test("findFeeSponsorPolicyById returns null when the configured policy is absent", async () => {
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async () => jsonResponse(404, {}),
  });

  assert.equal(
    await client.findFeeSponsorPolicyById(VALID_ACCOUNT_ID, "wallet_fx"),
    null,
  );
});

test("findFeeSponsorPolicyById binds the response to the requested policy id", async () => {
  const sponsor = ToriiClient._requireAccountId(VALID_ACCOUNT_ID);
  for (const id of [
    { sponsor: ToriiClient._requireAccountId(ALT_ACCOUNT_ID), name: "wallet_fx" },
    { sponsor, name: "other_policy" },
  ]) {
    const client = new ToriiClient("https://example.test", {
      fetchImpl: async () =>
        jsonResponse(200, {
          id,
          enabled: true,
          rules: [],
        }),
    });
    await assert.rejects(
      () => client.findFeeSponsorPolicyById(sponsor, "wallet_fx"),
      /does not match the requested sponsor and policy name/i,
    );
  }
});

test("findFeeSponsorPolicyById rejects malformed canonical policy shapes", async () => {
  const sponsor = ToriiClient._requireAccountId(VALID_ACCOUNT_ID);
  const baseRule = {
    effect: { effect: "allow", value: null },
    dataspaces: [0],
    executable_kinds: [{ kind: "instructions", value: null }],
    instruction_wire_ids: ["iroha.settlement.fx_corridor.settle"],
    contract_selectors: [],
  };
  const cases = [
    { ...baseRule, unexpected: true },
    { ...baseRule, effect: { effect: "allow", value: "not-null" } },
    { ...baseRule, dataspaces: ["0"] },
    {
      ...baseRule,
      executable_kinds: [{ kind: "instructions", value: null, extra: true }],
    },
    { ...baseRule, instruction_wire_ids: ["z", "a"] },
    { ...baseRule, contract_selectors: [{ contract_alias: "c@sbp" }] },
  ];

  for (const rule of cases) {
    const client = new ToriiClient("https://example.test", {
      fetchImpl: async () =>
        jsonResponse(200, {
          id: { sponsor, name: "wallet_fx" },
          enabled: true,
          rules: [rule],
        }),
    });
    await assert.rejects(
      () => client.findFeeSponsorPolicyById(sponsor, "wallet_fx"),
      /canonical|array|sorted|unsupported|u64/i,
    );
  }

  const invalidMaxFeeClient = new ToriiClient("https://example.test", {
    fetchImpl: async () =>
      jsonResponse(200, {
        id: { sponsor, name: "wallet_fx" },
        enabled: true,
        max_fee: 10,
        rules: [baseRule],
      }),
  });
  await assert.rejects(
    () => invalidMaxFeeClient.findFeeSponsorPolicyById(sponsor, "wallet_fx"),
    /quantity/i,
  );
});
