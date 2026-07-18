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
  const signerAccountAlias = "operator-1@mibank.paynet";
  let lastRequest = null;
  const fetchImpl = async (input, init) => {
    lastRequest = { input, init };
    return jsonResponse(200, {
      alias: "tidal-river-4160@mibank.paynet",
      account_id: VALID_ACCOUNT_ID,
      source: "runtime",
    });
  };
  const client = new ToriiClient("https://example.test", {
    fetchImpl,
  });

  const result = await client.resolveAlias("tidal-river-4160@mibank.paynet", {
    canonicalAuth: { accountId: signerAccountAlias, privateKey },
  });

  assert.equal(result.account_id, ToriiClient._requireAccountId(VALID_ACCOUNT_ID));
  assert.equal(lastRequest.init.headers["X-Iroha-Account"], signerAccountAlias);
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

test("findFeeSponsorProgramById posts and binds the exact program id", async () => {
  const sponsor = ToriiClient._requireAccountId(VALID_ACCOUNT_ID);
  const programId = `${sponsor}/wallet_fx`;
  const program = {
    id: { sponsor, name: "wallet_fx" },
    lifecycle: { state: "active", value: null },
    active_revision: 7,
  };
  let lastRequest = null;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      lastRequest = { input, init };
      assert.deepEqual(JSON.parse(init.body), { program_id: programId });
      return jsonResponse(200, program);
    },
  });
  const result = await client.findFeeSponsorProgramById(programId, {
    canonicalAuth: { accountId: sponsor, privateKey: Buffer.alloc(32, 12) },
  });
  assert.deepEqual(result, program);
  assert.equal(new URL(lastRequest.input).pathname, "/v1/fee-sponsor-programs/by-id");
  assert.equal(lastRequest.init.method, "POST");
});

test("findFeeSponsorProgramById returns null and rejects mismatched lifecycle records", async () => {
  const sponsor = ToriiClient._requireAccountId(VALID_ACCOUNT_ID);
  const canonicalAuth = { accountId: sponsor, privateKey: Buffer.alloc(32, 13) };
  const absent = new ToriiClient("https://example.test", {
    fetchImpl: async () => jsonResponse(404, {}),
  });
  assert.equal(
    await absent.findFeeSponsorProgramById(`${sponsor}/wallet_fx`, { canonicalAuth }),
    null,
  );

  const mismatched = new ToriiClient("https://example.test", {
    fetchImpl: async () =>
      jsonResponse(200, {
        id: { sponsor, name: "other" },
        lifecycle: { state: "active", value: null },
        active_revision: 1,
      }),
  });
  await assert.rejects(
    () => mismatched.findFeeSponsorProgramById(`${sponsor}/wallet_fx`, { canonicalAuth }),
    /does not match the requested exact program id/i,
  );
});

test("quoteFees account-signs the exact draft and returns typed limits", async () => {
  const authority = ToriiClient._requireAccountId(VALID_ACCOUNT_ID);
  const assetDefinitionId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa";
  const payload = {
    chain: "test-chain",
    authority,
    fee_payment: {
      payer: "authority",
      value: { charge_limits: [], gas_limit: null },
    },
  };
  const quote = {
    intent: {
      payer: "authority",
      value: {
        charge_limits: [
          {
            kind: { kind: "nexus", value: null },
            asset_definition_id: assetDefinitionId,
            max_amount: "2.5",
          },
        ],
        gas_limit: null,
      },
    },
    observation: {
      ledger_time_ms: 100,
      next_block_height: 9,
      route_dataspace_id: 0,
    },
    components: [
      {
        kind: { kind: "nexus", value: null },
        asset_definition_id: assetDefinitionId,
        max_amount: "2.5",
      },
    ],
    capacities: [],
    decision: {
      status: "accepted",
      value: {
        debit_source: { kind: "account", value: authority },
        program_revision: null,
      },
    },
  };
  let lastRequest = null;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async (input, init) => {
      lastRequest = { input, init };
      return jsonResponse(200, quote);
    },
  });
  const result = await client.quoteFees({ payload }, {
    canonicalAuth: { accountId: authority, privateKey: Buffer.alloc(32, 14) },
  });
  assert.deepEqual(result, quote);
  assert.deepEqual(JSON.parse(lastRequest.init.body), { payload });
  assert.equal(new URL(lastRequest.input).pathname, "/v1/fees/quote");
});

test("quoteFees requires canonical account authentication before sending", async () => {
  let fetchCalls = 0;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async () => {
      fetchCalls += 1;
      return jsonResponse(200, {});
    },
  });
  await assert.rejects(() => client.quoteFees({ authority: VALID_ACCOUNT_ID }), /canonicalAuth is required/);
  assert.equal(fetchCalls, 0);

  await assert.rejects(
    () => client.quoteFees(
      {
        authority: VALID_ACCOUNT_ID,
        fee_payment: {
          payer: "authority",
          value: { charge_limits: [], gas_limit: null },
        },
      },
      {
        canonicalAuth: {
          accountId: ALT_ACCOUNT_ID,
          privateKey: Buffer.alloc(32, 15),
        },
      },
    ),
    /must equal the exact payload authority/i,
  );
  assert.equal(fetchCalls, 0);
});
