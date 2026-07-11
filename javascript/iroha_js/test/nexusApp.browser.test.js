// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { NexusAppClient } from "../src/nexusApp.js";

const HASH_HEX = "ab".repeat(32);
const fixture = JSON.parse(
  readFileSync(
    new URL("../../../fixtures/sdk/nexus_connect_transfer_v1.json", import.meta.url),
    "utf8",
  ),
);

function hexBytes(value) {
  return Uint8Array.from(
    value.match(/../gu),
    (octet) => Number.parseInt(octet, 16),
  );
}

function mockResponse(status, body = "", headers = {}) {
  const encoded = new TextEncoder().encode(body);
  const normalizedHeaders = new Map(
    Object.entries(headers).map(([key, value]) => [key.toLowerCase(), String(value)]),
  );
  return {
    status,
    headers: {
      get(name) {
        return normalizedHeaders.get(String(name).toLowerCase()) ?? null;
      },
    },
    async arrayBuffer() {
      return encoded.buffer.slice(
        encoded.byteOffset,
        encoded.byteOffset + encoded.byteLength,
      );
    },
  };
}

test("browser Nexus runtime does not depend on a global Buffer shim", async () => {
  const originalBuffer = globalThis.Buffer;
  let digest;
  let submittedBody;
  try {
    globalThis.Buffer = undefined;
    const browserModule = await import(
      new URL(`../src/nexusApp.js?no-global-buffer=${Date.now()}`, import.meta.url)
    );
    digest = browserModule.nexusPayloadHashHex(Uint8Array.from([1]));
    const client = new browserModule.NexusAppClient({
      toriiBaseUrl: "https://torii.example",
      async fetchImpl(_url, init) {
        submittedBody = Uint8Array.from(init.body);
        return mockResponse(204);
      },
    });
    await client.toriiClient.submitTransaction(Uint8Array.from([1, 2]));
  } finally {
    globalThis.Buffer = originalBuffer;
  }
  assert.match(digest, /^[0-9a-f]{64}$/u);
  assert.deepEqual(submittedBody, Uint8Array.from([1, 2]));
});

test("browser Nexus default Torii transport submits exact signed bytes", async () => {
  const calls = [];
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example/gateway/v1/",
    async fetchImpl(url, init) {
      calls.push({ url, init, body: Buffer.from(init.body) });
      return mockResponse(
        202,
        JSON.stringify({ hashHex: HASH_HEX }),
        { "content-type": "application/json" },
      );
    },
  });
  const signed = Uint8Array.from([1, 2, 3, 4]);
  const receipt = await client.toriiClient.submitTransaction(signed);

  assert.deepEqual(receipt, { hashHex: HASH_HEX });
  assert.equal(calls.length, 1);
  assert.equal(
    calls[0].url,
    "https://torii.example/gateway/v1/pipeline/transactions",
  );
  assert.equal(calls[0].init.method, "POST");
  assert.equal(calls[0].init.headers["Content-Type"], "application/x-norito");
  assert.equal(calls[0].init.credentials, "omit");
  assert.equal(calls[0].init.redirect, "error");
  assert.equal(calls[0].init.referrerPolicy, "no-referrer");
  assert.equal(calls[0].init.signal.aborted, true);
  assert.deepEqual(calls[0].body, Buffer.from(signed));
  signed.fill(0xff);
  assert.deepEqual(calls[0].body, Buffer.from([1, 2, 3, 4]));
});

test("browser Nexus defaults build, finalize, and submit the shared canonical transfer", async () => {
  let submittedBody;
  const client = new NexusAppClient({
    chainId: fixture.transfer_input.chain_id,
    authority: fixture.transfer_input.authority,
    signingPublicKey: hexBytes(
      fixture.connect.approval_frame.signing_public_key_hex,
    ),
    toriiBaseUrl: "https://torii.example",
    async fetchImpl(_url, init) {
      submittedBody = Uint8Array.from(init.body);
      return mockResponse(204);
    },
  });
  const draft = client.buildTransferDraft({
    sourceAssetHoldingId: fixture.transfer_input.source_asset_id,
    quantity: fixture.transfer_input.quantity,
    destinationAccountId: fixture.transfer_input.destination_account_id,
    creationTimeMs: fixture.transfer_input.creation_time_ms,
    ttlMs: fixture.transfer_input.ttl_ms,
    nonce: fixture.transfer_input.nonce,
    metadata: fixture.transfer_input.metadata,
  });
  const receipt = await client.finalizeAndSubmit(
    draft.signable,
    hexBytes(fixture.expected.wallet_signature_hex),
    { wait: false },
  );

  assert.equal(
    receipt.signedTransactionHashHex,
    fixture.expected.signed_transaction_hash_hex,
  );
  assert.deepEqual(submittedBody, Uint8Array.from(receipt.signedTransaction));
  assert.equal(submittedBody[0], 1);
});

test("browser Nexus Torii base URL rejects ambient authority and URL smuggling", () => {
  for (const baseUrl of [
    "ftp://torii.example",
    "https://user:secret@torii.example",
    "https://torii.example?target=evil",
    "https://torii.example/#fragment",
    "not a URL",
  ]) {
    assert.throws(
      () => new NexusAppClient({ toriiBaseUrl: baseUrl, fetchImpl() {} }),
      /toriiBaseUrl/u,
      baseUrl,
    );
  }
});

test("browser Nexus snapshots config descriptors without invoking getters", () => {
  let proxyGets = 0;
  const proxied = new Proxy(
    { chainId: "snapshot-chain" },
    {
      get(target, key, receiver) {
        proxyGets += 1;
        return Reflect.get(target, key, receiver);
      },
    },
  );
  const client = new NexusAppClient(proxied);
  assert.equal(client.config.chainId, "snapshot-chain");
  assert.equal(proxyGets, 0);

  let accessorGets = 0;
  const accessor = {};
  Object.defineProperty(accessor, "chainId", {
    enumerable: true,
    get() {
      accessorGets += 1;
      return "getter-chain";
    },
  });
  for (const malformed of [
    Object.assign(Object.create({ inherited: true }), { chainId: "chain" }),
    accessor,
    { unsupported: true },
    { [Symbol("hidden")]: true },
  ]) {
    assert.throws(() => new NexusAppClient(malformed));
  }
  assert.equal(accessorGets, 0);
});

test("browser Nexus snapshots transfer descriptors before alias resolution", () => {
  const client = new NexusAppClient({
    chainId: "snapshot-chain",
    authority: "snapshot-authority",
    signingPublicKey: new Uint8Array(32),
    transactionCodec: {
      buildTransferPayload() {
        return Uint8Array.from([1]);
      },
    },
  });
  const valid = {
    sourceAssetHoldingId: "asset#snapshot-authority",
    quantity: "1",
    destinationAccountId: "destination",
  };
  let proxyGets = 0;
  const draft = client.buildTransferDraft(
    new Proxy(valid, {
      get(target, key, receiver) {
        proxyGets += 1;
        return Reflect.get(target, key, receiver);
      },
    }),
  );
  assert.deepEqual([...draft.signable.payloadBytes], [1]);
  assert.equal(proxyGets, 0);

  let accessorGets = 0;
  const accessor = { ...valid };
  Object.defineProperty(accessor, "quantity", {
    enumerable: true,
    get() {
      accessorGets += 1;
      return "1";
    },
  });
  for (const malformed of [
    Object.assign(Object.create({ polluted: true }), valid),
    accessor,
    { ...valid, unsupported: true },
    { ...valid, [Symbol("hidden")]: true },
  ]) {
    assert.throws(() => client.buildTransferDraft(malformed));
  }
  assert.equal(accessorGets, 0);
});

test("browser Nexus snapshots Connect options, sessions, and approvals", async () => {
  const approvedAccount = fixture.transfer_input.authority;
  const approvedSigningPublicKey = hexBytes(
    fixture.connect.approval_frame.signing_public_key_hex,
  );
  const client = new NexusAppClient({
    signingPublicKey: approvedSigningPublicKey,
    connectTransport: {
      startConnect(options) {
        return { sid: options.sid };
      },
      awaitApproval() {
        return { accountId: approvedAccount };
      },
    },
  });
  let optionGets = 0;
  const session = await client.startConnect(
    new Proxy(
      { sid: "safe-sid" },
      {
        get(target, key, receiver) {
          optionGets += 1;
          return Reflect.get(target, key, receiver);
        },
      },
    ),
  );
  assert.equal(session.sid, "safe-sid");
  assert.equal(optionGets, 0);

  let sessionGets = 0;
  const approval = await client.awaitApproval(
    new Proxy(
      { sid: "safe-sid" },
      {
        get(target, key, receiver) {
          sessionGets += 1;
          return Reflect.get(target, key, receiver);
        },
      },
    ),
  );
  assert.equal(approval.accountId, approvedAccount);
  assert.equal(sessionGets, 0);

  let optionAccessorGets = 0;
  const accessorOptions = {};
  Object.defineProperty(accessorOptions, "sid", {
    enumerable: true,
    get() {
      optionAccessorGets += 1;
      return "unsafe-sid";
    },
  });
  await assert.rejects(client.startConnect(accessorOptions));
  assert.equal(optionAccessorGets, 0);

  let approvalAccessorGets = 0;
  const approvalAccessor = {};
  Object.defineProperty(approvalAccessor, "accountId", {
    enumerable: true,
    get() {
      approvalAccessorGets += 1;
      return "unsafe-account";
    },
  });
  const unsafeApprovalClient = new NexusAppClient({
    signingPublicKey: new Uint8Array(32),
    connectTransport: {
      awaitApproval() {
        return approvalAccessor;
      },
    },
  });
  await assert.rejects(
    unsafeApprovalClient.awaitApproval({ sid: "safe-sid" }),
  );
  assert.equal(approvalAccessorGets, 0);

  for (const malformed of [
    Object.assign(Object.create({ polluted: true }), { sid: "safe-sid" }),
    { sid: "safe-sid", unsupported: true },
    { sid: "safe-sid", [Symbol("hidden")]: true },
  ]) {
    await assert.rejects(client.startConnect(malformed));
  }
});

test("browser Nexus Torii responses are bounded and canonical JSON objects", async () => {
  let oversizedCancelled = 0;
  const oversized = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl() {
      return {
        ...mockResponse(202, "{}", { "content-length": "65537" }),
        body: {
          async cancel() {
            oversizedCancelled += 1;
          },
        },
      };
    },
  });
  await assert.rejects(
    oversized.toriiClient.submitTransaction(Uint8Array.from([1, 2])),
    /exceeds 65536 response bytes/u,
  );
  assert.equal(oversizedCancelled, 1);

  const scalar = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl() {
      return mockResponse(202, JSON.stringify("not-an-object"));
    },
  });
  await assert.rejects(
    scalar.toriiClient.submitTransaction(Uint8Array.from([1, 2])),
    /JSON object or null/u,
  );
});

test("browser Nexus cancels rejected Torii response streams", async () => {
  let submissionCancelled = 0;
  let statusCancelled = 0;
  const submissionClient = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl() {
      return {
        ...mockResponse(500),
        body: {
          async cancel() {
            submissionCancelled += 1;
          },
        },
      };
    },
  });
  await assert.rejects(
    submissionClient.toriiClient.submitTransaction(Uint8Array.from([1, 2])),
    /submission returned HTTP 500/u,
  );
  assert.equal(submissionCancelled, 1);

  const statusClient = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl() {
      return {
        ...mockResponse(404),
        body: {
          async cancel() {
            statusCancelled += 1;
          },
        },
      };
    },
  });
  assert.equal(
    await statusClient.toriiClient.getTransactionStatus(HASH_HEX),
    null,
  );
  assert.equal(statusCancelled, 1);
});

test("browser Nexus Torii polling reaches nested terminal status without Node APIs", async () => {
  const responses = [
    { content: { status: "Pending" } },
    { content: { status: { kind: "Committed" } } },
  ];
  const urls = [];
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl(url) {
      urls.push(url);
      return mockResponse(
        200,
        JSON.stringify(responses.shift()),
        { "content-type": "application/json" },
      );
    },
  });
  const observed = [];
  const result = await client.toriiClient.waitForTransactionStatus(HASH_HEX, {
    intervalMs: 0,
    maxAttempts: 2,
    onStatus(status, _payload, attempt) {
      observed.push([status, attempt]);
    },
  });

  assert.deepEqual(result, { content: { status: { kind: "Committed" } } });
  assert.deepEqual(observed, [["Pending", 1], ["Committed", 2]]);
  assert.equal(urls.length, 2);
  for (const url of urls) {
    assert.equal(
      url,
      `https://torii.example/v1/pipeline/transactions/status?hash=${HASH_HEX}&scope=global`,
    );
  }
});

test("browser Nexus Torii polling fails closed on rejection and status-set overlap", async () => {
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl() {
      return mockResponse(200, JSON.stringify({ status: "Rejected" }));
    },
  });
  await assert.rejects(
    client.toriiClient.waitForTransactionStatus(HASH_HEX, {
      intervalMs: 0,
      maxAttempts: 1,
    }),
    /failure status Rejected/u,
  );
  await assert.rejects(
    client.toriiClient.waitForTransactionStatus(HASH_HEX, {
      intervalMs: 0,
      maxAttempts: 1,
      successStatuses: ["Done"],
      failureStatuses: ["Done"],
    }),
    /cannot be both success and failure/u,
  );
});

test("browser Nexus counts duplicate raw statuses before any fetch", async () => {
  let fetchCalls = 0;
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl() {
      fetchCalls += 1;
      return mockResponse(200, JSON.stringify({ status: "Committed" }));
    },
  });
  for (const options of [
    { successStatuses: new Array(33).fill("Committed") },
    { failureStatuses: new Array(33).fill("Rejected") },
  ]) {
    await assert.rejects(
      client.toriiClient.waitForTransactionStatus(HASH_HEX, options),
      /must not contain more than 32 statuses/u,
    );
    assert.equal(fetchCalls, 0, "invalid raw status iterables must not fetch");
  }
});

test("browser Nexus closes an effectively unbounded duplicate status iterator", async () => {
  let yielded = 0;
  let cleanedUp = 0;
  let fetchCalls = 0;
  function* duplicateStatuses() {
    try {
      for (let index = 0; index < 1_000; index += 1) {
        yielded += 1;
        yield "Committed";
      }
      throw new Error("duplicate status iterator was over-consumed");
    } finally {
      cleanedUp += 1;
    }
  }
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl() {
      fetchCalls += 1;
      return mockResponse(200, JSON.stringify({ status: "Committed" }));
    },
  });

  await assert.rejects(
    client.toriiClient.waitForTransactionStatus(HASH_HEX, {
      successStatuses: duplicateStatuses(),
    }),
    /must not contain more than 32 statuses/u,
  );
  assert.equal(yielded, 33);
  assert.equal(cleanedUp, 1);
  assert.equal(fetchCalls, 0);
});

test("browser Nexus Torii status polling propagates an already-aborted signal", async () => {
  const controller = new AbortController();
  controller.abort(new Error("cancelled-by-wallet"));
  let requests = 0;
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl() {
      requests += 1;
      return mockResponse(200, "{}");
    },
  });
  await assert.rejects(
    client.toriiClient.waitForTransactionStatus(HASH_HEX, {
      signal: controller.signal,
    }),
    /cancelled-by-wallet/u,
  );
  assert.equal(requests, 0);
});
