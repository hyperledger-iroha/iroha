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
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);
const FOREIGN_NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xa7));
const PRIVATE_KEY = Buffer.alloc(32, 0x0b);
const PUBLIC_KEY = Buffer.from(
  "66BE7E332C7A453332BD9D0A7F7DB055F5C5EF1A06ADA66D98B39FB6810C473A",
  "hex",
);
const PUBLIC_KEY_MULTIHASH = `ed0120${PUBLIC_KEY.toString("hex").toUpperCase()}`;
const RELAY_ID = "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE";
const KAIGI_HPKE_KEY_B64 = "QUJDRA==";
const KAIGI_HPKE_FINGERPRINT_HEX =
  "58c7dab691f514e0bd6f4082852ac0f1e08df24b5864038ff70ecd68419f4a23";

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
      return jsonResponse(200, { total: 0, items: [] });
    },
  });
  await signed.listKaigiRelays();
  assert.equal(signedCalls.length, 1);
  assert.equal(signedCalls[0].url, `${BASE_URL}/v1/kaigi/relays`);
  assert.equal(signedCalls[0].init.credentials, "omit");
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

  for (const defaultHeaders of [
    { "X-Iroha-Operator-Nonce": "precomputed" },
    { Cookie: "operator-session=ambient" },
    { "Proxy-Authorization": "Basic cHJveHk=" },
    { "X-Iroha-Iso-Profile": "retired" },
  ]) {
    const forbidden = new ToriiBrowserClient(BASE_URL, {
      operatorSigningContext: context,
      defaultHeaders,
      fetchImpl: async () => {
        calls += 1;
        return jsonResponse(200, {});
      },
    });
    // eslint-disable-next-line no-await-in-loop
    await assert.rejects(
      forbidden.listKaigiRelays(),
      /requires generated signing/u,
    );
  }
  assert.equal(calls, 0);
});

test("browser Kaigi diagnostics preserve full u64 values and validate relay binding", async () => {
  const u64Max = "18446744073709551615";
  const context = new BrowserOperatorSigningContext(NETWORK_ID, {
    publicKey: PUBLIC_KEY_MULTIHASH,
    sign: async () => Buffer.alloc(64, 0x33),
  });
  const responses = [
    `{"total":1,"items":[{"relay_id":"${RELAY_ID}","domain":"kaigi","bandwidth_class":1,"hpke_fingerprint_hex":"${KAIGI_HPKE_FINGERPRINT_HEX}","status":"healthy","reported_at_ms":${u64Max}}]}`,
    `{"relay":{"relay_id":"${RELAY_ID}","domain":"kaigi","bandwidth_class":1,"hpke_fingerprint_hex":"${KAIGI_HPKE_FINGERPRINT_HEX}"},"hpke_public_key_b64":"${KAIGI_HPKE_KEY_B64}","metrics":{"domain":"kaigi","registrations_total":${u64Max},"manifest_updates_total":${u64Max},"failovers_total":${u64Max},"health_reports_total":${u64Max}}}`,
    `{"healthy_total":1,"degraded_total":0,"unavailable_total":0,"reports_total":${u64Max},"registrations_total":${u64Max},"failovers_total":${u64Max},"domains":[{"domain":"kaigi","registrations_total":${u64Max},"manifest_updates_total":${u64Max},"failovers_total":${u64Max},"health_reports_total":${u64Max}}]}`,
  ];
  const client = new ToriiBrowserClient(BASE_URL, {
    operatorSigningContext: context,
    fetchImpl: async () => new Response(responses.shift(), {
      status: 200,
      headers: { "content-type": "application/json" },
    }),
  });

  const relays = await client.listKaigiRelays();
  assert.equal(relays.items[0].reported_at_ms, 18_446_744_073_709_551_615n);
  const detail = await client.getKaigiRelay(RELAY_ID);
  assert.equal(detail.relay.hpke_fingerprint_hex, KAIGI_HPKE_FINGERPRINT_HEX);
  assert.equal(detail.metrics.registrations_total, 18_446_744_073_709_551_615n);
  const health = await client.getKaigiRelaysHealth();
  assert.equal(health.reports_total, 18_446_744_073_709_551_615n);
  assert.equal(health.domains[0].health_reports_total, 18_446_744_073_709_551_615n);
});

test("browser Kaigi relay detail preserves 404 null semantics and rejects bad options", async () => {
  const context = new BrowserOperatorSigningContext(NETWORK_ID, {
    publicKey: PUBLIC_KEY_MULTIHASH,
    sign: async () => Buffer.alloc(64, 0x44),
  });
  let calls = 0;
  const client = new ToriiBrowserClient(BASE_URL, {
    operatorSigningContext: context,
    fetchImpl: async () => {
      calls += 1;
      return new Response(null, { status: 404 });
    },
  });
  assert.equal(await client.getKaigiRelay(RELAY_ID), null);
  assert.throws(
    () => client.listKaigiRelays({ extra: true }),
    /unsupported option extra/u,
  );
  assert.throws(
    () => client.getKaigiRelay(RELAY_ID, { extra: true }),
    /unsupported option extra/u,
  );
  assert.equal(calls, 1);

  const nonJson = new ToriiBrowserClient(BASE_URL, {
    operatorSigningContext: context,
    fetchImpl: async () => new Response('{"total":0,"items":[]}', {
      status: 200,
      headers: { "content-type": "text/plain" },
    }),
  });
  await assert.rejects(
    () => nonJson.listKaigiRelays(),
    /application\/json media type/u,
  );
});

test("browser Kaigi diagnostics reject sparse JSON arrays and oversized status totals", async () => {
  const context = new BrowserOperatorSigningContext(NETWORK_ID, {
    publicKey: PUBLIC_KEY_MULTIHASH,
    sign: async () => Buffer.alloc(64, 0x55),
  });
  const responses = [
    '{"total":1,"items":[,]}',
    '{"healthy_total":0,"degraded_total":0,"unavailable_total":0,"reports_total":0,"registrations_total":0,"failovers_total":0,"domains":[,]}',
    '{"healthy_total":501,"degraded_total":0,"unavailable_total":0,"reports_total":0,"registrations_total":0,"failovers_total":0,"domains":[]}',
  ];
  const client = new ToriiBrowserClient(BASE_URL, {
    operatorSigningContext: context,
    fetchImpl: async () => new Response(responses.shift(), {
      status: 200,
      headers: { "content-type": "application/json" },
    }),
  });

  await assert.rejects(() => client.listKaigiRelays(), /invalid JSON/u);
  await assert.rejects(() => client.getKaigiRelaysHealth(), /invalid JSON/u);
  await assert.rejects(
    () => client.getKaigiRelaysHealth(),
    /status totals must not exceed 500/u,
  );
});
