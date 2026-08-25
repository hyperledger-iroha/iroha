import assert from "node:assert/strict";
import test from "node:test";

import { canonicalRequestMessage } from "../src/canonicalRequest.js";
import { signEd25519, verifyEd25519 } from "../src/crypto.js";
import { NetworkId, networkIdBytes } from "../src/networkId.js";
import {
  OperatorSigningContext as BrowserOperatorSigningContext,
} from "../src/operatorRequest.browser.js";
import { ToriiBrowserClient } from "../src/toriiBrowserClient.js";
import {
  OperatorSigningContext,
  ToriiClient,
} from "../src/toriiClient.js";

const BASE_URL = "https://torii.example";
const NETWORK_ID = NetworkId.parse(
  "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149",
);
const FOREIGN_NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xa7));
const PRIVATE_KEY = Buffer.alloc(32, 0x0b);
const PUBLIC_KEY = Buffer.from(
  "66BE7E332C7A453332BD9D0A7F7DB055F5C5EF1A06ADA66D98B39FB6810C473A",
  "hex",
);
const PUBLIC_KEY_MULTIHASH = `ed0120${PUBLIC_KEY.toString("hex").toUpperCase()}`;
const RELAY_ID = "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE";

function signingContext() {
  return new OperatorSigningContext(NETWORK_ID, {
    publicKey: PUBLIC_KEY_MULTIHASH,
    sign: (message) => signEd25519(message, PRIVATE_KEY),
  });
}

function jsonResponse(status, payload) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function header(headers, name) {
  return new Headers(headers).get(name);
}

function assertExactSignature(call, target) {
  const timestamp = header(call.init.headers, "x-iroha-operator-timestamp-ms");
  const nonce = header(call.init.headers, "x-iroha-operator-nonce");
  const signature = Buffer.from(
    header(call.init.headers, "x-iroha-operator-signature"),
    "base64",
  );
  const suffix = Buffer.from(`\n${timestamp}\n${nonce}`);
  const message = Buffer.concat([
    Buffer.from("iroha.operator.http-request.network.v1\0"),
    Buffer.from(networkIdBytes(NETWORK_ID)),
    canonicalRequestMessage({ method: "GET", path: target, query: "", body: Buffer.alloc(0) }),
    suffix,
  ]);
  assert.equal(verifyEd25519(message, signature, PUBLIC_KEY), true);

  for (const variant of [
    Buffer.concat([
      Buffer.from("iroha.operator.http-request.network.v1\0"),
      Buffer.from(networkIdBytes(FOREIGN_NETWORK_ID)),
      canonicalRequestMessage({ method: "GET", path: target, query: "", body: Buffer.alloc(0) }),
      suffix,
    ]),
    Buffer.concat([
      Buffer.from("iroha.operator.http-request.network.v1\0"),
      Buffer.from(networkIdBytes(NETWORK_ID)),
      canonicalRequestMessage({ method: "GET", path: "/v1/kaigi/relays/foreign", query: "", body: Buffer.alloc(0) }),
      suffix,
    ]),
    Buffer.concat([
      Buffer.from("iroha.operator.http-request.network.v1\0"),
      Buffer.from(networkIdBytes(NETWORK_ID)),
      canonicalRequestMessage({ method: "GET", path: target, query: "format=json", body: Buffer.alloc(0) }),
      suffix,
    ]),
  ]) {
    assert.equal(verifyEd25519(variant, signature, PUBLIC_KEY), false);
  }
}

test("Kaigi diagnostics require generated operator authentication before dispatch", async () => {
  let calls = 0;
  const missing = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      calls += 1;
      return jsonResponse(200, {});
    },
  });
  for (const operation of [
    () => missing.listKaigiRelays(),
    () => missing.getKaigiRelay(RELAY_ID),
    () => missing.getKaigiRelaysHealth(),
  ]) {
    // eslint-disable-next-line no-await-in-loop
    await assert.rejects(operation, /requires an immutable OperatorSigningContext/u);
  }
  assert.equal(calls, 0);

  const precomputed = new ToriiClient(BASE_URL, {
    operatorSigningContext: signingContext(),
    defaultHeaders: { "X-Iroha-Operator-Nonce": "precomputed" },
    fetchImpl: async () => {
      calls += 1;
      return jsonResponse(200, {});
    },
  });
  await assert.rejects(
    () => precomputed.listKaigiRelays(),
    /requires generated operator signing/u,
  );
  assert.equal(calls, 0);
});

test("Kaigi diagnostics bind the exact network and target and dispatch once", async () => {
  const calls = [];
  const responses = [
    jsonResponse(200, { total: 0, items: [] }),
    jsonResponse(404, {}),
    jsonResponse(200, {
      healthy_total: 0,
      degraded_total: 0,
      unavailable_total: 0,
      reports_total: 0,
      registrations_total: 0,
      failovers_total: 0,
      domains: [],
    }),
  ];
  const client = new ToriiClient(BASE_URL, {
    operatorSigningContext: signingContext(),
    retry: { maxRetries: 7, retryOnMethods: ["GET"], retryOnStatus: [503] },
    fetchImpl: async (url, init) => {
      calls.push({ url: String(url), init });
      return responses.shift();
    },
  });

  assert.equal((await client.listKaigiRelays()).total, 0);
  assert.equal(await client.getKaigiRelay(RELAY_ID), null);
  assert.equal((await client.getKaigiRelaysHealth()).healthy_total, 0);

  const targets = [
    "/v1/kaigi/relays",
    `/v1/kaigi/relays/${encodeURIComponent(RELAY_ID)}`,
    "/v1/kaigi/relays/health",
  ];
  assert.equal(calls.length, targets.length);
  calls.forEach((call, index) => {
    assert.equal(call.url, `${BASE_URL}${targets[index]}`);
    assert.equal(call.init.method, "GET");
    assert.equal(call.init.redirect, "error");
    assertExactSignature(call, targets[index]);
  });
});

test("Kaigi operator diagnostics never retry a dispatched request", async () => {
  let calls = 0;
  const client = new ToriiClient(BASE_URL, {
    operatorSigningContext: signingContext(),
    maxRetries: 7,
    retryMethods: ["GET"],
    retryStatuses: [503],
    fetchImpl: async (_url, init) => {
      calls += 1;
      assert.equal(init.redirect, "error");
      return jsonResponse(503, { error: "unavailable" });
    },
  });

  await assert.rejects(() => client.listKaigiRelays());
  assert.equal(calls, 1);
});

test("browser Kaigi diagnostics require generated one-shot operator auth", async () => {
  let calls = 0;
  const missing = new ToriiBrowserClient(BASE_URL, {
    fetchImpl: async () => {
      calls += 1;
      return jsonResponse(200, {});
    },
  });
  for (const operation of [
    () => missing.listKaigiRelays(),
    () => missing.getKaigiRelay(RELAY_ID),
    () => missing.getKaigiRelaysHealth(),
  ]) {
    assert.throws(operation, /requires an immutable OperatorSigningContext/u);
  }
  assert.equal(calls, 0);

  const messages = [];
  const context = new BrowserOperatorSigningContext(NETWORK_ID, {
    publicKey: PUBLIC_KEY_MULTIHASH,
    sign: async (message) => {
      messages.push(Buffer.from(message));
      return Buffer.alloc(64, 0x22);
    },
  });
  const signedCalls = [];
  const signed = new ToriiBrowserClient(BASE_URL, {
    operatorSigningContext: context,
    fetchImpl: async (url, init) => {
      signedCalls.push({ url: String(url), init });
      return jsonResponse(200, {});
    },
  });
  await signed.listKaigiRelays();
  assert.equal(signedCalls.length, 1);
  assert.equal(signedCalls[0].url, `${BASE_URL}/v1/kaigi/relays`);
  assert.equal(signedCalls[0].init.redirect, "error");
  assert.equal(signedCalls[0].init.body, undefined);
  assert.ok(signedCalls[0].init.headers["X-Iroha-Operator-Signature"]);
  assert.equal(messages.length, 1);
  assert.ok(
    messages[0].includes(canonicalRequestMessage({
      method: "GET",
      path: "/v1/kaigi/relays",
      query: "",
      body: Buffer.alloc(0),
    })),
  );

  const precomputed = new ToriiBrowserClient(BASE_URL, {
    operatorSigningContext: context,
    defaultHeaders: { "X-Iroha-Operator-Nonce": "precomputed" },
    fetchImpl: async () => {
      calls += 1;
      return jsonResponse(200, {});
    },
  });
  await assert.rejects(
    precomputed.listKaigiRelays(),
    /requires generated signing/u,
  );
  assert.equal(calls, 0);
});
