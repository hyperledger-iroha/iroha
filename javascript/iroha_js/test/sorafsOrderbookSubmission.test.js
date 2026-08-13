import assert from "node:assert/strict";
import { test } from "node:test";

import {
  LocalSigningContext,
  SorafsOrderbookSubmissionAmbiguousError,
  ToriiClient,
} from "../src/toriiClient.js";
import { NetworkId } from "../src/networkId.js";

const BASE_URL = "https://torii.example";
const SIGNER = "ed0120ABCDEF";
const IDENTITY = Object.freeze({
  txHash: "aa".repeat(32),
  entrypointHash: "aa".repeat(32),
  signedTransactionHash: "aa".repeat(32),
});
const NETWORK_ID = NetworkId.parse(
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);

function receiptJson() {
  const body = "AA".repeat(32);
  let crc = 0xffff;
  for (const byte of Buffer.from(`hash:${body}`, "ascii")) {
    crc ^= byte << 8;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = crc & 0x8000 ? ((crc << 1) ^ 0x1021) & 0xffff : (crc << 1) & 0xffff;
    }
  }
  const hash = `hash:${body}#${crc.toString(16).toUpperCase().padStart(4, "0")}`;
  return JSON.stringify({
    payload: {
      tx_hash: hash,
      entrypoint_hash: hash,
      signed_transaction_hash: hash,
      submitted_at_ms: 1,
      submitted_at_height: 2,
      signer: SIGNER,
    },
    signature: "AB",
  });
}

function nativeBinding(overrides = {}) {
  return {
    inspectSorafsOrderbookSubmissionV1(route, network, discriminant, signer, body) {
      assert.ok(["order", "cancel", "receipt"].includes(route));
      assert.equal(Buffer.from(network).byteLength, 32);
      assert.equal(discriminant, 369);
      assert.equal(signer, SIGNER);
      assert.ok(Buffer.from(body).byteLength > 0);
      return IDENTITY;
    },
    verifySorafsOrderbookSubmissionReceiptV1(
      body,
      txHash,
      entrypointHash,
      signedTransactionHash,
      signer,
    ) {
      assert.ok(Buffer.from(body).byteLength > 0);
      assert.deepEqual(
        { txHash, entrypointHash, signedTransactionHash },
        IDENTITY,
      );
      assert.equal(signer, SIGNER);
      return receiptJson();
    },
    ...overrides,
  };
}

function acceptedResponse(headers = {}, body = Uint8Array.of(9)) {
  return new Response(body, {
    status: 202,
    headers: {
      "content-type": "application/x-norito",
      "content-length": String(body.byteLength),
      "x-iroha-transaction-hash": IDENTITY.txHash,
      "x-iroha-entrypoint-hash": IDENTITY.entrypointHash,
      "x-iroha-signed-transaction-hash": IDENTITY.signedTransactionHash,
      ...headers,
    },
  });
}

function client(fetchImpl, native = nativeBinding(), options = {}) {
  const { validation = async () => {}, ...clientOptions } = options;
  const sdk = new ToriiClient(BASE_URL, {
    fetchImpl,
    __nativeBinding: native,
    localSigningContext: new LocalSigningContext(NETWORK_ID, 369),
    ...clientOptions,
  });
  sdk._ensureDataModelValidation = validation;
  return sdk;
}

test("orderbook submit snapshots bytes and sends one exact authenticated Norito request", async () => {
  let releaseCapabilities;
  const capabilitiesGate = new Promise((resolve) => { releaseCapabilities = resolve; });
  const dispatched = [];
  const fetchImpl = async (url, init) => {
    dispatched.push({ url, init });
    return acceptedResponse();
  };
  const signed = Buffer.from([1, 2, 3]);
  const native = nativeBinding();
  const pending = client(fetchImpl, native, {
    validation: () => capabilitiesGate,
  }).submitSorafsOrderbookOrder(signed, {
    expectedReceiptSigner: SIGNER,
  });
  signed.fill(0xff);
  native.verifySorafsOrderbookSubmissionReceiptV1 = () => { throw new Error("mutable replacement"); };
  releaseCapabilities();
  const receipt = await pending;

  assert.equal(receipt.payload.signer, SIGNER);
  assert.equal(dispatched.length, 1);
  assert.equal(dispatched[0].url, `${BASE_URL}/v1/sorafs/orderbook/orders`);
  assert.deepEqual(Buffer.from(dispatched[0].init.body), Buffer.from([1, 2, 3]));
  assert.deepEqual(
    {
      accept: dispatched[0].init.headers.Accept,
      encoding: dispatched[0].init.headers["Accept-Encoding"],
      type: dispatched[0].init.headers["Content-Type"],
      redirect: dispatched[0].init.redirect,
    },
    {
      accept: "application/x-norito",
      encoding: "identity",
      type: "application/x-norito",
      redirect: "error",
    },
  );
});

test("orderbook submit binds snapshotted native callables to their native receiver", async () => {
  const native = nativeBinding();
  const inspect = native.inspectSorafsOrderbookSubmissionV1;
  const verify = native.verifySorafsOrderbookSubmissionReceiptV1;
  native.inspectSorafsOrderbookSubmissionV1 = function (...args) {
    assert.equal(this, native);
    return Reflect.apply(inspect, this, args);
  };
  native.verifySorafsOrderbookSubmissionReceiptV1 = function (...args) {
    assert.equal(this, native);
    return Reflect.apply(verify, this, args);
  };
  const receipt = await client(async () => acceptedResponse(), native)
    .submitSorafsOrderbookOrder(Buffer.of(1), { expectedReceiptSigner: SIGNER });
  assert.equal(receipt.payload.signer, SIGNER);
});

test("orderbook submit keeps its original target and transport across validation", async () => {
  let releaseCapabilities;
  let originalFetches = 0;
  let substitutedFetches = 0;
  let substitutedRequests = 0;
  const sdk = client(async () => { originalFetches += 1; return acceptedResponse(); }, nativeBinding(), {
    validation: () => new Promise((resolve) => { releaseCapabilities = resolve; }),
  });
  const pending = sdk.submitSorafsOrderbookOrder(Buffer.of(1), {
    expectedReceiptSigner: SIGNER,
  });
  assert.throws(() => { sdk._baseUrl = "https://attacker.example"; }, TypeError);
  assert.throws(() => { sdk._fetch = async () => { substitutedFetches += 1; }; }, TypeError);
  sdk._request = async () => { substitutedRequests += 1; };
  releaseCapabilities();
  await pending;
  assert.equal(originalFetches, 1);
  assert.equal(substitutedFetches, 0);
  assert.equal(substitutedRequests, 0);
});

test("orderbook submit requires https unless insecure transport is explicit and reported", async () => {
  let fetches = 0;
  const insecure = new ToriiClient("http://torii.example", {
    fetchImpl: async () => { fetches += 1; return acceptedResponse(); },
    __nativeBinding: nativeBinding(),
    localSigningContext: new LocalSigningContext(NETWORK_ID, 369),
  });
  insecure._ensureDataModelValidation = async () => {};
  await assert.rejects(
    insecure.submitSorafsOrderbookOrder(Buffer.of(1), { expectedReceiptSigner: SIGNER }),
    /requires an https Torii base URL/u,
  );
  assert.equal(fetches, 0);
  const events = [];
  const optedIn = new ToriiClient("http://torii.example", {
    allowInsecure: true,
    insecureTransportTelemetryHook: (event) => events.push(event),
    fetchImpl: async () => { fetches += 1; return acceptedResponse(); },
    __nativeBinding: nativeBinding(),
    localSigningContext: new LocalSigningContext(NETWORK_ID, 369),
  });
  optedIn._ensureDataModelValidation = async () => {};
  await optedIn.submitSorafsOrderbookOrder(Buffer.of(1), { expectedReceiptSigner: SIGNER });
  assert.equal(fetches, 1);
  assert.equal(events.some((event) => event.hasSensitiveBody === true), true);
  for (const baseUrl of [
    "ftp://torii.example", "file:///tmp/torii", "data:text/plain,torii",
    "https://user:pass@torii.example", "https://torii.example?redirect=yes",
    "https://torii.example#fragment",
  ]) {
    const sdk = new ToriiClient(baseUrl, {
      allowInsecure: true, fetchImpl: async () => { fetches += 1; },
      __nativeBinding: nativeBinding(), localSigningContext: new LocalSigningContext(NETWORK_ID, 369),
    });
    sdk._ensureDataModelValidation = async () => {};
    await assert.rejects(
      sdk.submitSorafsOrderbookOrder(Buffer.of(1), { expectedReceiptSigner: SIGNER }),
      /canonical HTTP\(S\) Torii base URL/u,
    );
  }
  assert.equal(fetches, 1);
});

test("orderbook submit bounds pre-dispatch validation even with a caller signal", async () => {
  let fetches = 0;
  let validationAborted = false;
  const sdk = client(async () => { fetches += 1; }, nativeBinding(), {
    timeoutMs: 5,
    validation: (signal) => new Promise(() => {
      signal.addEventListener("abort", () => { validationAborted = true; }, { once: true });
    }),
  });
  await assert.rejects(
    sdk.submitSorafsOrderbookOrder(Buffer.of(1), {
      expectedReceiptSigner: SIGNER,
      signal: new AbortController().signal,
    }),
    (error) => error?.name === "TimeoutError",
  );
  assert.equal(fetches, 0);
  assert.equal(validationAborted, true);
  await assert.rejects(
    client(async () => { fetches += 1; }, nativeBinding(), { timeoutMs: 0 })
      .submitSorafsOrderbookOrder(Buffer.of(1), { expectedReceiptSigner: SIGNER }),
    /positive finite client timeoutMs/u,
  );
  assert.equal(fetches, 0);
});

test("orderbook deadline races a custom fetch that ignores AbortSignal", async () => {
  let fetches = 0;
  const sdk = client(async () => { fetches += 1; return new Promise(() => {}); }, nativeBinding(), {
    timeoutMs: 5,
  });
  await assert.rejects(
    sdk.submitSorafsOrderbookOrder(Buffer.of(1), { expectedReceiptSigner: SIGNER }),
    (error) => error instanceof SorafsOrderbookSubmissionAmbiguousError,
  );
  assert.equal(fetches, 1);
});

test("orderbook deadline cancels a response that arrives after ambiguity", async () => {
  let resolveFetch;
  let cancelled = false;
  const sdk = client(() => new Promise((resolve) => { resolveFetch = resolve; }), nativeBinding(), {
    timeoutMs: 5,
  });
  await assert.rejects(
    sdk.submitSorafsOrderbookOrder(Buffer.of(1), { expectedReceiptSigner: SIGNER }),
    SorafsOrderbookSubmissionAmbiguousError,
  );
  resolveFetch({ status: 202, body: { cancel() { cancelled = true; } } });
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(cancelled, true);
});

test("orderbook deadline uses captured AbortSignal intrinsics and nonthrowing cleanup", async () => {
  const controller = new AbortController();
  Object.defineProperties(controller.signal, {
    aborted: { get() { throw new Error("shadowed aborted"); } },
    reason: { get() { throw new Error("shadowed reason"); } },
    addEventListener: { value() { throw new Error("shadowed add"); } },
    removeEventListener: { value() { throw new Error("shadowed remove"); } },
  });
  const callerSignal = controller.signal;
  const originalAdd = EventTarget.prototype.addEventListener;
  const originalRemove = EventTarget.prototype.removeEventListener;
  const originalSignal = Object.getOwnPropertyDescriptor(AbortController.prototype, "signal");
  try {
    EventTarget.prototype.addEventListener = () => { throw new Error("mutated add"); };
    EventTarget.prototype.removeEventListener = () => { throw new Error("mutated remove"); };
    Object.defineProperty(AbortController.prototype, "signal", {
      configurable: true, get() { throw new Error("mutated signal"); },
    });
    const receipt = await client(async () => acceptedResponse()).submitSorafsOrderbookOrder(
      Buffer.of(1), { expectedReceiptSigner: SIGNER, signal: callerSignal },
    );
    assert.equal(receipt.payload.signer, SIGNER);
  } finally {
    EventTarget.prototype.addEventListener = originalAdd;
    EventTarget.prototype.removeEventListener = originalRemove;
    Object.defineProperty(AbortController.prototype, "signal", originalSignal);
  }
});

test("orderbook submit rechecks mutable effective headers immediately before dispatch", async () => {
  let releaseCapabilities;
  let fetches = 0;
  const sdk = client(async () => { fetches += 1; }, nativeBinding(), {
    validation: () => new Promise((resolve) => { releaseCapabilities = resolve; }),
  });
  const pending = sdk.submitSorafsOrderbookOrder(Buffer.of(1), {
    expectedReceiptSigner: SIGNER,
  });
  sdk._config.defaultHeaders.Prefer = "return=minimal";
  releaseCapabilities();
  await assert.rejects(pending, /forbids overriding Prefer/u);
  assert.equal(fetches, 0);
});

test("orderbook submit freezes all effective credentials and benign headers before validation", async () => {
  for (const mutate of [
    (sdk) => { sdk._config.defaultHeaders.Authorization = "Bearer replacement"; },
    (sdk) => { sdk._config.apiToken = "replacement"; },
    (sdk) => { sdk._config.defaultHeaders["X-Tenant"] = "replacement"; },
  ]) {
    let releaseCapabilities;
    let fetches = 0;
    const sdk = client(async () => { fetches += 1; }, nativeBinding(), {
      validation: () => new Promise((resolve) => { releaseCapabilities = resolve; }),
      defaultHeaders: { "X-Tenant": "original" }, apiToken: "original",
    });
    const pending = sdk.submitSorafsOrderbookOrder(Buffer.of(1), {
      expectedReceiptSigner: SIGNER,
    });
    mutate(sdk);
    releaseCapabilities();
    await assert.rejects(pending, /effective request headers changed/u);
    assert.equal(fetches, 0);
  }
});

test("orderbook submit fails before HTTP without its signer and strict native verifier", async () => {
  let fetches = 0;
  const fetchImpl = async () => { fetches += 1; throw new Error("must not fetch"); };
  await assert.rejects(
    client(fetchImpl).submitSorafsOrderbookOrder(Buffer.of(1), {}),
    /expectedReceiptSigner/u,
  );
  await assert.rejects(
    client(fetchImpl, {}).submitSorafsOrderbookOrder(Buffer.of(1), {
      expectedReceiptSigner: SIGNER,
    }),
    /missing inspectSorafsOrderbookSubmissionV1/u,
  );
  assert.equal(fetches, 0);
});

test("orderbook submit marks every failure after dispatch as non-resubmittable ambiguity", async () => {
  const badHash = "bb".repeat(32);
  const cases = [
    ["network", async () => { throw new Error("connection reset"); }],
    ["status", async () => new Response(null, { status: 500 })],
    ["media", async () => acceptedResponse({ "content-type": "application/json" })],
    ["identity", async () => acceptedResponse({ "x-iroha-entrypoint-hash": badHash })],
    ["coalesced", async () => acceptedResponse({
      "x-iroha-transaction-hash": `${IDENTITY.txHash}, ${IDENTITY.txHash}`,
    })],
    ["oversize", async () => acceptedResponse({ "content-length": "1048577" })],
    ["length mismatch", async () => acceptedResponse({ "content-length": "2" })],
    ["bad receipt", async () => acceptedResponse()],
  ];
  for (const [label, submissionResponse] of cases) {
    let submissions = 0;
    const native = label === "bad receipt"
      ? nativeBinding({ verifySorafsOrderbookSubmissionReceiptV1() { throw new Error("bad"); } })
      : nativeBinding();
    const fetchImpl = async () => {
      submissions += 1;
      return submissionResponse();
    };
    await assert.rejects(
      client(fetchImpl, native).submitSorafsOrderbookOrder(Buffer.of(1), {
        expectedReceiptSigner: SIGNER,
      }),
      (error) => {
        assert.ok(error instanceof SorafsOrderbookSubmissionAmbiguousError, label);
        assert.equal(error.route, "order");
        assert.deepEqual(error.expectedIdentity, IDENTITY);
        assert.equal(Object.isFrozen(error.expectedIdentity), true);
        assert.equal("body" in error, false);
        return true;
      },
    );
    assert.equal(submissions, 1, label);
  }
});

test("orderbook submit rejects fixed routing and framing header overrides before HTTP", async () => {
  for (const name of [
    "Accept-Encoding", "Connection", "Content-Encoding", "Content-Length", "Content-Type",
    "Expect", "Host", "Keep-Alive", "Prefer", "Proxy-Connection", "TE", "Trailer",
    "Transfer-Encoding", "Upgrade", "X-HTTP-Method-Override", "X-Method-Override",
  ]) {
    let fetches = 0;
    const sdk = client(async () => { fetches += 1; }, nativeBinding(), {
      defaultHeaders: { [name]: "forbidden" },
    });
    await assert.rejects(
      sdk.submitSorafsOrderbookOrder(Buffer.of(1), { expectedReceiptSigner: SIGNER }),
      /forbids overriding/u,
    );
    assert.equal(fetches, 0);
  }
});
