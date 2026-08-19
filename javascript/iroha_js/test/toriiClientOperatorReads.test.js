import assert from "node:assert/strict";
import test from "node:test";

import { canonicalRequestMessage } from "../src/canonicalRequest.js";
import { signEd25519, verifyEd25519 } from "../src/crypto.js";
import { NetworkId, networkIdBytes } from "../src/networkId.js";
import { OperatorSigningContext, ToriiClient } from "../src/toriiClient.js";

const BASE_URL = "https://torii.example";
const NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xa5));
const FOREIGN_NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xa7));
const PRIVATE_KEY = Buffer.alloc(32, 0x0b);
const PUBLIC_KEY = Buffer.from(
  "66BE7E332C7A453332BD9D0A7F7DB055F5C5EF1A06ADA66D98B39FB6810C473A",
  "hex",
);
const PUBLIC_KEY_MULTIHASH = `ed0120${PUBLIC_KEY.toString("hex").toUpperCase()}`;

function signingContext() {
  return new OperatorSigningContext(NETWORK_ID, {
    publicKey: PUBLIC_KEY_MULTIHASH,
    sign: (message) => signEd25519(message, PRIVATE_KEY),
  });
}

function header(headers, name) {
  return Object.entries(headers).find(
    ([candidate]) => candidate.toLowerCase() === name.toLowerCase(),
  )?.[1];
}

const OPERATOR_READS = [
  ["/v1/peers", (client) => client.listPeers()],
  ["/v1/time/status", (client) => client.getNetworkTimeStatus()],
  ["/v1/pipeline/preflight", (client) => client.getPipelinePreflight()],
  ["/v1/pipeline/recovery/42", (client) => client.getPipelineRecovery(42)],
  ["/v1/sumeragi/status", (client) => client.getSumeragiStatus()],
  ["/v1/sumeragi/diagnostics", (client) => client.getSumeragiDiagnostics()],
  ["/v1/sumeragi/qc", (client) => client.getSumeragiQc()],
  ["/v1/sumeragi/bls-keys", (client) => client.getSumeragiBlsKeys()],
  ["/v1/sumeragi/leader", (client) => client.getSumeragiLeader()],
  ["/v1/sumeragi/params", (client) => client.getSumeragiParams()],
  [
    "/v1/sumeragi/key-lifecycle",
    async (client) => {
      await client.listSumeragiKeyLifecycle();
      throw new Error("terminal test response");
    },
  ],
  [
    "/v1/sumeragi/evidence/count",
    (client) => client.getSumeragiEvidenceCount(),
  ],
];

for (const [path, invoke] of OPERATOR_READS) {
  test(`${path} signs one exact-network empty-body GET`, async () => {
    const calls = [];
    const client = new ToriiClient(BASE_URL, {
      operatorSigningContext: signingContext(),
      maxRetries: 5,
      retryMethods: ["GET"],
      retryStatuses: [503],
      fetchImpl: async (url, init) => {
        calls.push({ url, init });
        return new Response(null, { status: 503 });
      },
    });

    await assert.rejects(() => invoke(client));
    assert.equal(calls.length, 1);
    const { url, init } = calls[0];
    assert.equal(url, `${BASE_URL}${path}`);
    assert.equal(init.method, "GET");
    assert.equal(init.body, undefined);
    assert.equal(init.redirect, "error");
    assert.equal(new URL(url).search, "");
    assert.equal(header(init.headers, "authorization"), undefined);
    assert.equal(header(init.headers, "x-api-token"), undefined);

    const timestamp = header(init.headers, "x-iroha-operator-timestamp-ms");
    const nonce = header(init.headers, "x-iroha-operator-nonce");
    const signature = Buffer.from(
      header(init.headers, "x-iroha-operator-signature"),
      "base64",
    );
    const canonical = canonicalRequestMessage({
      method: "GET",
      path,
      query: "",
      body: Buffer.alloc(0),
    });
    const freshness = Buffer.from(`\n${timestamp}\n${nonce}`);
    const localMessage = Buffer.concat([
      Buffer.from("iroha.operator.http-request.network.v1\0"),
      Buffer.from(networkIdBytes(NETWORK_ID)),
      canonical,
      freshness,
    ]);
    const foreignMessage = Buffer.concat([
      Buffer.from("iroha.operator.http-request.network.v1\0"),
      Buffer.from(networkIdBytes(FOREIGN_NETWORK_ID)),
      canonical,
      freshness,
    ]);
    assert.equal(verifyEd25519(localMessage, signature, PUBLIC_KEY), true);
    assert.equal(verifyEd25519(foreignMessage, signature, PUBLIC_KEY), false);
  });

  test(`${path} fails before dispatch without operator context`, async () => {
    let calls = 0;
    const client = new ToriiClient(BASE_URL, {
      fetchImpl: async () => {
        calls += 1;
        return new Response(null, { status: 200 });
      },
    });
    await assert.rejects(
      () => invoke(client),
      /requires an immutable OperatorSigningContext/u,
    );
    assert.equal(calls, 0);
  });
}

test("Sumeragi evidence signs the final query target exactly once", async () => {
  const calls = [];
  const client = new ToriiClient(BASE_URL, {
    operatorSigningContext: signingContext(),
    maxRetries: 5,
    fetchImpl: async (url, init) => {
      calls.push({ url, init });
      return new Response(null, { status: 503 });
    },
  });

  await assert.rejects(() => client.listSumeragiEvidence({
    limit: 2,
    offset: 1,
    kind: "SumeragiV2Equivocation",
  }));

  assert.equal(calls.length, 1);
  const { url, init } = calls[0];
  assert.equal(
    url,
    `${BASE_URL}/v1/sumeragi/evidence?limit=2&offset=1&kind=SumeragiV2Equivocation`,
  );
  assert.equal(init.method, "GET");
  assert.equal(init.body, undefined);
  assert.equal(init.redirect, "error");
  assert.ok(header(init.headers, "x-iroha-operator-signature"));

  let missingContextCalls = 0;
  const missingContext = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      missingContextCalls += 1;
      return new Response(null, { status: 200 });
    },
  });
  await assert.rejects(
    () => missingContext.listSumeragiEvidence({ limit: 2 }),
    /requires an immutable OperatorSigningContext/u,
  );
  assert.equal(missingContextCalls, 0);
});

test("operator reads generate a fresh nonce instead of replaying a request", async () => {
  const nonces = [];
  const client = new ToriiClient(BASE_URL, {
    operatorSigningContext: signingContext(),
    fetchImpl: async (_url, init) => {
      nonces.push(header(init.headers, "x-iroha-operator-nonce"));
      return new Response("[]", {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    },
  });

  await client.listPeers();
  await client.listPeers();
  assert.equal(nonces.length, 2);
  assert.notEqual(nonces[0], nonces[1]);
});

test("operator reads reject token and precomputed signature fallback", async () => {
  for (const options of [
    { authToken: "retired-bearer" },
    { apiToken: "retired-token" },
    { defaultHeaders: { "X-Iroha-Account": "retired-account" } },
    { defaultHeaders: { "X-Iroha-Operator-Nonce": "precomputed" } },
  ]) {
    const client = new ToriiClient(BASE_URL, {
      ...options,
      operatorSigningContext: signingContext(),
      fetchImpl: async () => {
        throw new Error("retired auth must fail before dispatch");
      },
    });
    // eslint-disable-next-line no-await-in-loop
    await assert.rejects(() => client.listPeers(), /generated operator signing/u);
  }
});
