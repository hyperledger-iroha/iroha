// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { NexusAppClient } from "../src/nexusApp.js";
import { NetworkId } from "../src/networkId.js";

const HASH_HEX = "ab".repeat(32);
const fixture = JSON.parse(
  readFileSync(
    new URL("../../../fixtures/sdk/nexus_connect_transfer_v1.json", import.meta.url),
    "utf8",
  ),
);
const fixtureNetworkId = NetworkId.parse(fixture.transfer_input.network_id);

function hexBytes(value) {
  return Uint8Array.from(
    value.match(/../gu),
    (octet) => Number.parseInt(octet, 16),
  );
}

function fixtureFeePayment() {
  return {
    payer: fixture.transfer_input.fee_payment.payer,
    chargeLimits: [...fixture.transfer_input.fee_payment.value.charge_limits],
    gasLimit: fixture.transfer_input.fee_payment.value.gas_limit,
  };
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

function pipelineStatus(
  kind,
  {
    hash = HASH_HEX,
    blockHeight,
    scope = "global",
    resolvedFrom = "queue",
  } = {},
) {
  return {
    hash,
    status: {
      kind,
      ...(blockHeight === undefined ? {} : { block_height: blockHeight }),
    },
    scope,
    resolved_from: resolvedFrom,
  };
}

function authoritativeAppliedStatus(hash = HASH_HEX, blockHeight = 1) {
  return pipelineStatus("Applied", {
    hash,
    blockHeight,
    resolvedFrom: "state",
  });
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
    networkId: fixtureNetworkId,
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
    feePayment: fixtureFeePayment(),
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

test("browser Nexus classifies rejected and timed-out post-submit status waits", async () => {
  for (const [status, expectedCode] of [
    ["Rejected", "transaction_rejected"],
    ["Queued", "status_wait_timeout"],
  ]) {
    const client = new NexusAppClient({
      networkId: fixtureNetworkId,
      authority: fixture.transfer_input.authority,
      signingPublicKey: hexBytes(
        fixture.connect.approval_frame.signing_public_key_hex,
      ),
      toriiBaseUrl: "https://torii.example",
      async fetchImpl(_url, init) {
        if (init.method === "POST") return mockResponse(204);
        return mockResponse(
          200,
          JSON.stringify(
            pipelineStatus(status, {
              hash: fixture.expected.signed_transaction_hash_hex,
              resolvedFrom: status === "Rejected" ? "state" : "queue",
            }),
          ),
          { "content-type": "application/json" },
        );
      },
    });
    const draft = client.buildTransferDraft({
      sourceAssetHoldingId: fixture.transfer_input.source_asset_id,
      quantity: fixture.transfer_input.quantity,
      destinationAccountId: fixture.transfer_input.destination_account_id,
      creationTimeMs: fixture.transfer_input.creation_time_ms,
      ttlMs: fixture.transfer_input.ttl_ms,
      nonce: fixture.transfer_input.nonce,
      feePayment: fixtureFeePayment(),
      metadata: fixture.transfer_input.metadata,
      feePayment: {
        payer: fixture.transfer_input.fee_payment.payer,
        chargeLimits: fixture.transfer_input.fee_payment.value.charge_limits,
      },
    });

    await assert.rejects(
      () =>
        client.finalizeAndSubmit(
          draft.signable,
          hexBytes(fixture.expected.wallet_signature_hex),
          { intervalMs: 0, maxAttempts: 1 },
        ),
      (error) => {
        assert.equal(error.code, expectedCode);
        assert.equal(error.submissionState, "submitted");
        assert.equal(
          error.signedTransactionHashHex,
          fixture.expected.signed_transaction_hash_hex,
        );
        assert.equal(error.status?.status?.kind ?? error.status, status);
        return true;
      },
    );
  }
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

test("browser Nexus snapshots transfers and rejects retired chain aliases", () => {
  const client = new NexusAppClient({
    networkId: fixtureNetworkId,
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
    feePayment: fixtureFeePayment(),
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
    { ...valid, chain: "legacy-chain" },
    { ...valid, chainId: "legacy-chain" },
    { ...valid, chain_id: "legacy-chain" },
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

test("browser Nexus raw status reads allow only explicit diagnostic scopes", async () => {
  const urls = [];
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl(url) {
      urls.push(url);
      return mockResponse(404);
    },
  });

  assert.equal(await client.toriiClient.getTransactionStatus(HASH_HEX), null);
  assert.equal(
    await client.toriiClient.getTransactionStatus(HASH_HEX, {
      scope: undefined,
    }),
    null,
  );
  assert.equal(
    await client.toriiClient.getTransactionStatus(HASH_HEX, { scope: "local" }),
    null,
  );
  assert.deepEqual(urls, [
    `https://torii.example/v1/pipeline/transactions/status?hash=${HASH_HEX}&scope=global`,
    `https://torii.example/v1/pipeline/transactions/status?hash=${HASH_HEX}&scope=global`,
    `https://torii.example/v1/pipeline/transactions/status?hash=${HASH_HEX}&scope=local`,
  ]);
  for (const scope of [null, "", "auto"]) {
    await assert.rejects(
      client.toriiClient.getTransactionStatus(HASH_HEX, { scope }),
      /must be local or global/u,
    );
  }
  for (const scope of [undefined, null, "global"]) {
    await assert.rejects(
      client.toriiClient.waitForTransactionStatus(HASH_HEX, { scope }),
      /unsupported field scope/u,
    );
  }
  assert.equal(urls.length, 3);
});

test("browser Nexus Torii polling retries cached Applied until state finality", async () => {
  const responses = [
    pipelineStatus("Queued"),
    pipelineStatus("Committed", { resolvedFrom: "cache" }),
    pipelineStatus("Applied", { blockHeight: 3, resolvedFrom: "cache" }),
    authoritativeAppliedStatus(HASH_HEX, 3),
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
    maxAttempts: 4,
    failureStatuses: ["Committed"],
    onStatus(status, _payload, attempt) {
      observed.push([status, attempt]);
    },
  });

  assert.deepEqual(result, authoritativeAppliedStatus(HASH_HEX, 3));
  assert.deepEqual(observed, [
    ["Queued", 1],
    ["Committed", 2],
    ["Applied", 3],
    ["Applied", 4],
  ]);
  assert.equal(urls.length, 4);
  for (const url of urls) {
    assert.equal(
      url,
      `https://torii.example/v1/pipeline/transactions/status?hash=${HASH_HEX}&scope=global`,
    );
  }
});

test("browser Nexus Torii polling retries cached failures until state resolution", async () => {
  const responses = [
    pipelineStatus("Rejected", { resolvedFrom: "cache" }),
    pipelineStatus("Rejected", { resolvedFrom: "state" }),
  ];
  const observed = [];
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl() {
      return mockResponse(200, JSON.stringify(responses.shift()));
    },
  });

  await assert.rejects(
    client.toriiClient.waitForTransactionStatus(HASH_HEX, {
      intervalMs: 0,
      maxAttempts: 2,
      failureStatuses: ["Committed"],
      onStatus(status, _payload, attempt) {
        observed.push([status, attempt]);
      },
    }),
    /failure status Rejected/u,
  );
  assert.deepEqual(observed, [["Rejected", 1], ["Rejected", 2]]);
});

test("browser Nexus Torii polling fails closed on malformed finality envelopes", async () => {
  for (const [payload, pattern] of [
    [
      pipelineStatus("Applied", {
        hash: "cd".repeat(32),
        blockHeight: 1,
        resolvedFrom: "state",
      }),
      /hash must match/u,
    ],
    [
      pipelineStatus("Applied", {
        blockHeight: 1,
        scope: "local",
        resolvedFrom: "state",
      }),
      /scope must be global/u,
    ],
    [
      pipelineStatus("Applied", {
        blockHeight: 1,
        resolvedFrom: "queue",
      }),
      /cache- or state-resolved/u,
    ],
    [
      pipelineStatus("Applied", {
        blockHeight: 0,
        resolvedFrom: "state",
      }),
      /positive block height/u,
    ],
  ]) {
    const client = new NexusAppClient({
      toriiBaseUrl: "https://torii.example",
      async fetchImpl() {
        return mockResponse(200, JSON.stringify(payload));
      },
    });
    await assert.rejects(
      client.toriiClient.waitForTransactionStatus(HASH_HEX, {
        intervalMs: 0,
        maxAttempts: 1,
      }),
      pattern,
    );
  }
});

test("browser Nexus Torii polling fails closed on rejection and success override", async () => {
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl() {
      return mockResponse(
        200,
        JSON.stringify(pipelineStatus("Rejected", { resolvedFrom: "state" })),
      );
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
    }),
    /unsupported field.*successStatuses/u,
  );
  await assert.rejects(
    client.toriiClient.waitForTransactionStatus(HASH_HEX, {
      failureStatuses: ["Applied"],
    }),
    /Applied cannot be configured as failure/u,
  );
});

test("browser Nexus counts duplicate raw statuses before any fetch", async () => {
  let fetchCalls = 0;
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl() {
      fetchCalls += 1;
      return mockResponse(
        200,
        JSON.stringify(pipelineStatus("Committed", { resolvedFrom: "cache" })),
      );
    },
  });
  for (const options of [
    { failureStatuses: new Array(33).fill("Rejected") },
  ]) {
    await assert.rejects(
      client.toriiClient.waitForTransactionStatus(HASH_HEX, options),
      /must not contain more than 32 statuses/u,
    );
    assert.equal(fetchCalls, 0, "invalid raw status iterables must not fetch");
  }
});

test("browser Nexus closes an infinite duplicate status iterator", async () => {
  let yielded = 0;
  let cleanedUp = 0;
  let fetchCalls = 0;
  function* duplicateStatuses() {
    try {
      while (true) {
        yielded += 1;
        yield "Rejected";
      }
    } finally {
      cleanedUp += 1;
    }
  }
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl() {
      fetchCalls += 1;
      return mockResponse(
        200,
        JSON.stringify(pipelineStatus("Committed", { resolvedFrom: "cache" })),
      );
    },
  });

  await assert.rejects(
    client.toriiClient.waitForTransactionStatus(HASH_HEX, {
      failureStatuses: duplicateStatuses(),
    }),
    /must not contain more than 32 statuses/u,
  );
  assert.equal(yielded, 33);
  assert.equal(cleanedUp, 1);
  assert.equal(fetchCalls, 0);
});

test("browser Nexus acquires a custom status iterator exactly once", async () => {
  let iteratorReads = 0;
  const failureStatuses = {};
  Object.defineProperty(failureStatuses, Symbol.iterator, {
    get() {
      iteratorReads += 1;
      if (iteratorReads > 1) return undefined;
      return function* statuses() {
        yield "Rejected";
      };
    },
  });
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl() {
      return mockResponse(200, JSON.stringify(authoritativeAppliedStatus()));
    },
  });

  const result = await client.toriiClient.waitForTransactionStatus(HASH_HEX, {
    failureStatuses,
  });
  assert.deepEqual(result, authoritativeAppliedStatus());
  assert.equal(iteratorReads, 1);
});

test("browser Nexus polling uses intrinsic AbortSignal state and listeners", async () => {
  const controller = new AbortController();
  const reason = new Error("stop after the first observed status");
  Object.defineProperties(controller.signal, {
    aborted: { value: false },
    reason: { value: undefined },
    addEventListener: { value() {} },
    removeEventListener: { value() {} },
  });
  let fetches = 0;
  let callbacks = 0;
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl() {
      fetches += 1;
      return mockResponse(200, JSON.stringify(pipelineStatus("Queued")));
    },
  });

  await assert.rejects(
    client.toriiClient.waitForTransactionStatus(HASH_HEX, {
      signal: controller.signal,
      intervalMs: 0,
      maxAttempts: 2,
      onStatus() {
        callbacks += 1;
        controller.abort(reason);
      },
    }),
    (error) => error === reason,
  );
  assert.equal(fetches, 1);
  assert.equal(callbacks, 1);
});

test("browser Nexus rejects AbortSignal impostors without invoking public accessors", async () => {
  let accessorReads = 0;
  let fetches = 0;
  const fakeSignal = {};
  for (const property of [
    "aborted",
    "reason",
    "addEventListener",
    "removeEventListener",
  ]) {
    Object.defineProperty(fakeSignal, property, {
      get() {
        accessorReads += 1;
        throw new Error(`fake AbortSignal.${property} was read`);
      },
    });
  }
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl() {
      fetches += 1;
      return mockResponse(
        200,
        JSON.stringify(pipelineStatus("Committed", { resolvedFrom: "cache" })),
      );
    },
  });

  await assert.rejects(
    client.toriiClient.waitForTransactionStatus(HASH_HEX, {
      signal: fakeSignal,
    }),
    /must be an AbortSignal/u,
  );
  assert.equal(accessorReads, 0);
  assert.equal(fetches, 0);

  const proxiedSignal = new Proxy(
    { aborted: false },
    {
      get() {
        throw new Error("AbortSignal proxy trap");
      },
    },
  );
  await assert.rejects(
    client.toriiClient.waitForTransactionStatus(HASH_HEX, {
      signal: proxiedSignal,
    }),
    /must be an AbortSignal/u,
  );
  assert.equal(fetches, 0);
});

test("browser Nexus aborts an uncooperative Fetch implementation via intrinsics", async () => {
  const controller = new AbortController();
  const reason = new Error("cancel uncooperative Fetch");
  Object.defineProperties(controller.signal, {
    aborted: { value: false },
    reason: { value: undefined },
    addEventListener: { value() {} },
    removeEventListener: { value() {} },
  });
  let fetches = 0;
  let fetchStarted;
  const started = new Promise((resolve) => {
    fetchStarted = resolve;
  });
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    fetchImpl() {
      fetches += 1;
      fetchStarted();
      return new Promise(() => {});
    },
  });
  const pending = client.toriiClient.getTransactionStatus(HASH_HEX, {
    signal: controller.signal,
  });
  await started;
  controller.abort(reason);

  const outcome = await Promise.race([
    pending.then(
      () => ({ resolved: true }),
      (error) => ({ error }),
    ),
    new Promise((resolve) =>
      setTimeout(() => resolve({ timedOut: true }), 250),
    ),
  ]);
  assert.deepEqual(outcome, { error: reason });
  assert.equal(fetches, 1);
});

test("browser Nexus hard-bounds stalled bodies, cancellation, and callbacks", async () => {
  for (const scenario of ["body", "cancel"]) {
    let stalledOperations = 0;
    const client = new NexusAppClient({
      toriiBaseUrl: "https://torii.example",
      fetchImpl() {
        if (scenario === "body") {
          return {
            status: 200,
            headers: {
              get(name) {
                return name.toLowerCase() === "content-type"
                  ? "application/json"
                  : null;
              },
            },
            arrayBuffer() {
              stalledOperations += 1;
              return new Promise(() => {});
            },
          };
        }
        if (scenario === "cancel") {
          return {
            ...mockResponse(404),
            body: {
              cancel() {
                stalledOperations += 1;
                return new Promise(() => {});
              },
            },
          };
        }
        throw new Error(`unexpected scenario: ${scenario}`);
      },
    });
    const pending = client.toriiClient.waitForTransactionStatus(HASH_HEX, {
      intervalMs: 0,
      timeoutMs: 5,
    });
    const outcome = await Promise.race([
      pending.then(
        () => ({ resolved: true }),
        (error) => ({ error }),
      ),
      new Promise((resolve) =>
        setTimeout(() => resolve({ timedOut: true }), 500),
      ),
    ]);
    assert.ok(outcome.error instanceof Error, `${scenario} must reject`);
    assert.match(outcome.error.message, /did not settle within 5ms/u);
    assert.equal(stalledOperations, 1, scenario);
  }

  const controller = new AbortController();
  const abortReason = new Error("abort stalled status callback");
  let callbacks = 0;
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    fetchImpl() {
      return mockResponse(
        200,
        JSON.stringify(pipelineStatus("Queued")),
        { "content-type": "application/json" },
      );
    },
  });
  await assert.rejects(
    client.toriiClient.waitForTransactionStatus(HASH_HEX, {
      intervalMs: 0,
      signal: controller.signal,
      onStatus() {
        callbacks += 1;
        controller.abort(abortReason);
        return new Promise(() => {});
      },
    }),
    (error) => error === abortReason,
  );
  assert.equal(callbacks, 1);
});

test("browser Nexus does not invoke Fetch for an already-aborted direct status request", async () => {
  const controller = new AbortController();
  const reason = new Error("abort before direct status request");
  controller.abort(reason);
  let fetches = 0;
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl() {
      fetches += 1;
      return mockResponse(
        200,
        JSON.stringify(pipelineStatus("Committed", { resolvedFrom: "cache" })),
      );
    },
  });

  await assert.rejects(
    client.toriiClient.getTransactionStatus(HASH_HEX, {
      signal: controller.signal,
    }),
    (error) => error === reason,
  );
  assert.equal(fetches, 0);
});

test("browser Nexus aborts an in-flight fetch despite hostile listener shadows", async () => {
  const controller = new AbortController();
  const reason = new Error("cancel in-flight status fetch");
  Object.defineProperties(controller.signal, {
    aborted: { value: false },
    reason: { value: undefined },
    addEventListener: { value() {} },
    removeEventListener: { value() {} },
  });
  let markStarted;
  const started = new Promise((resolve) => {
    markStarted = resolve;
  });
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    fetchImpl(_url, init) {
      markStarted();
      return new Promise((_resolve, reject) => {
        if (init.signal.aborted) {
          reject(init.signal.reason);
          return;
        }
        init.signal.addEventListener(
          "abort",
          () => reject(init.signal.reason),
          { once: true },
        );
      });
    },
  });

  const pending = client.toriiClient.getTransactionStatus(HASH_HEX, {
    signal: controller.signal,
  });
  await started;
  controller.abort(reason);
  await assert.rejects(pending, (error) => error === reason);
});

test("browser Nexus preserves a null abort reason during polling", async () => {
  const controller = new AbortController();
  const client = new NexusAppClient({
    toriiBaseUrl: "https://torii.example",
    async fetchImpl() {
      return mockResponse(200, JSON.stringify(pipelineStatus("Queued")));
    },
  });
  let caught = Symbol("not rejected");
  try {
    await client.toriiClient.waitForTransactionStatus(HASH_HEX, {
      signal: controller.signal,
      intervalMs: 0,
      onStatus() {
        controller.abort(null);
      },
    });
  } catch (error) {
    caught = error;
  }
  assert.equal(caught, null);
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
