import assert from "node:assert/strict";
import test from "node:test";
import { ed25519 } from "@noble/curves/ed25519";

import { AccountAddress } from "../src/address.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";
import { NetworkId } from "../src/networkId.js";
import { LocalSigningContext, ToriiClient } from "../src/toriiClient.js";
import { TORII_TEST_NATIVE_BINDING } from "../src/toriiTestHooks.js";
import {
  VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES,
  VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES,
  createValidationFeeHijiriQuoteApi,
} from "../src/validationFeeHijiriQuote.js";

const ACCOUNT_ID = AccountAddress.fromAccount({
  publicKey: Buffer.from(ed25519.getPublicKey(Buffer.alloc(32, 0x51))),
}).toI105();
const REQUEST_NORITO = Buffer.from([1, 2, 3]);
const RESPONSE_NORITO = Buffer.from([4, 5, 6]);
const NETWORK_ID = NetworkId.parse(
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);
const QUOTE_URL = "https://example.test/v1/validation-fee/hijiri/quote";

function quoteResponse(body, init = {}, metadata = {}) {
  const response = new Response(body, init);
  return {
    body: response.body,
    headers: response.headers,
    redirected: metadata.redirected ?? false,
    status: response.status,
    statusText: response.statusText,
    url: metadata.url ?? QUOTE_URL,
  };
}

function projection() {
  return {
    schema: "iroha.torii.v1.validation_fee.hijiri_quote.response",
    version: 1,
    assurance: "EVALUATED_PROJECTION_NOT_INDEPENDENTLY_WITNESS_VERIFIED",
    evaluatedStateHeight: "42",
    quotedExecutionHeight: "43",
    accountId: ACCOUNT_ID,
    activePolicyVersion: "1",
    activePolicyHash: "03".repeat(32),
    feeAssetDefinitionId: "fee-asset",
    treasuryAccountId: ACCOUNT_ID,
    feeScale: 2,
    hijiriParametersVersion: 1,
    hijiriParametersRevision: "1",
    hijiriParametersDigest: "05".repeat(32),
    defaultAccountRiskQ16: 0,
    effectiveAccountRiskQ16: 0,
    accountRiskRevision: null,
    accountRiskDigest: null,
    feeMultiplierQ16: 81_920,
    hijiriFeeQuoteHash: "07".repeat(32),
    basePerTransferFeeMinorUnits: "10",
    adjustedPerTransferFeeMinorUnits: "13",
    qualifyingTransferCount: 2,
    aggregateBaseFeeMinorUnits: "20",
    aggregateAdjustedFeeMinorUnits: "25",
  };
}

async function withNativeBinding(native, body) {
  return body(
    createValidationFeeHijiriQuoteApi(createNativeRuntime(native)),
    Object.freeze({ [TORII_TEST_NATIVE_BINDING]: native }),
  );
}

function quoteNative(overrides = {}) {
  return {
    connectNoritoBridgeAbiVersion() {
      return 23;
    },
    validationFeeHijiriQuoteRequestV1(accountId, count) {
      assert.equal(accountId, ACCOUNT_ID);
      assert.equal(count, 2);
      return REQUEST_NORITO;
    },
    validationFeeVerifyHijiriQuoteResponseV1(response, request) {
      assert.deepEqual(response, RESPONSE_NORITO);
      assert.deepEqual(request, REQUEST_NORITO);
      return JSON.stringify(projection());
    },
    ...overrides,
  };
}

test("Hijiri quote factories isolate immutable native runtimes", async () => {
  const bindingA = {
    connectNoritoBridgeAbiVersion: () => 23,
    validationFeeHijiriQuoteRequestV1: () => Buffer.from([0xa1]),
    validationFeeVerifyHijiriQuoteResponseV1() {},
  };
  const apiA = createValidationFeeHijiriQuoteApi(createNativeRuntime(bindingA));
  const apiB = createValidationFeeHijiriQuoteApi(createNativeRuntime({
    connectNoritoBridgeAbiVersion: () => 23,
    validationFeeHijiriQuoteRequestV1: () => Buffer.from([0xb2]),
    validationFeeVerifyHijiriQuoteResponseV1() {},
  }));
  bindingA.validationFeeHijiriQuoteRequestV1 = () => Buffer.from([0xff]);

  const [requestA, requestB] = await Promise.all([
    Promise.resolve().then(() =>
      apiA.encodeValidationFeeHijiriQuoteRequestV1(ACCOUNT_ID, 2)),
    Promise.resolve().then(() =>
      apiB.encodeValidationFeeHijiriQuoteRequestV1(ACCOUNT_ID, 2)),
  ]);
  assert.equal(Object.isFrozen(apiA), true);
  assert.deepEqual(requestA, Buffer.from([0xa1]));
  assert.deepEqual(requestB, Buffer.from([0xb2]));
});

test("Hijiri quote codec delegates exclusively to the ABI-23 native bridge", async () => {
  await withNativeBinding(quoteNative(), (api) => {
    assert.deepEqual(
      api.encodeValidationFeeHijiriQuoteRequestV1(ACCOUNT_ID, 2),
      REQUEST_NORITO,
    );
    assert.deepEqual(
      api.verifyValidationFeeHijiriQuoteResponseV1(
        RESPONSE_NORITO,
        REQUEST_NORITO,
      ),
      projection(),
    );
  });

  await withNativeBinding(
    {
      connectNoritoBridgeAbiVersion() {
        return 23;
      },
    },
    (api) => {
      assert.throws(
        () => api.encodeValidationFeeHijiriQuoteRequestV1(ACCOUNT_ID, 2),
        /native binding lacks/u,
      );
      assert.throws(
        () =>
          api.verifyValidationFeeHijiriQuoteResponseV1(
            RESPONSE_NORITO,
            REQUEST_NORITO,
          ),
        /native binding lacks/u,
      );
    },
  );
});

test("Hijiri quote codec enforces request, response, and transfer bounds", async () => {
  await withNativeBinding(quoteNative(), (api) => {
    for (const count of [0, 100_001, -1, 1.5]) {
      assert.throws(
        () => api.encodeValidationFeeHijiriQuoteRequestV1(ACCOUNT_ID, count),
        /qualifyingTransferCount/u,
      );
    }
    assert.throws(
      () =>
        api.verifyValidationFeeHijiriQuoteResponseV1(
          Buffer.alloc(VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES + 1),
          REQUEST_NORITO,
        ),
      /responseNorito/u,
    );
    assert.throws(
      () =>
        api.verifyValidationFeeHijiriQuoteResponseV1(
          RESPONSE_NORITO,
          Buffer.alloc(VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES + 1),
        ),
      /requestNorito/u,
    );

    class MaterializationGuard extends Uint8Array {
      get byteLength() {
        return 1;
      }
    }
    const validResponse = new MaterializationGuard(RESPONSE_NORITO);
    const oversizedResponse = new MaterializationGuard(
      VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES + 1,
    );
    const oversizedRequest = new MaterializationGuard(
      VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES + 1,
    );
    const originalBufferFrom = Buffer.from;
    let materializations = 0;
    Buffer.from = (...args) => {
      materializations += 1;
      return originalBufferFrom(...args);
    };
    try {
      assert.throws(
        () =>
          api.verifyValidationFeeHijiriQuoteResponseV1(
            oversizedResponse,
            REQUEST_NORITO,
          ),
        /responseNorito/u,
      );
      assert.throws(
        () =>
          api.verifyValidationFeeHijiriQuoteResponseV1(
            validResponse,
            oversizedRequest,
          ),
        /requestNorito/u,
      );
    } finally {
      Buffer.from = originalBufferFrom;
    }
    assert.equal(materializations, 0);
  });
});

test("Hijiri quote codec closes the native projection shape", async () => {
  const validRiskProjection = {
    ...projection(),
    accountRiskRevision: "2",
    accountRiskDigest: "09".repeat(32),
  };
  await withNativeBinding(
    quoteNative({
      validationFeeVerifyHijiriQuoteResponseV1() {
        return JSON.stringify(validRiskProjection);
      },
    }),
    (api) => {
      assert.deepEqual(
        api.verifyValidationFeeHijiriQuoteResponseV1(
          RESPONSE_NORITO,
          REQUEST_NORITO,
        ),
        validRiskProjection,
      );
    },
  );

  for (const [invalidProjection, pattern] of [
    [{ ...projection(), hijiriParametersVersion: 2 }, /exactly 1/u],
    [
      {
        ...projection(),
        accountRiskRevision: "02",
        accountRiskDigest: "09".repeat(32),
      },
      /accountRiskRevision/u,
    ],
    [
      {
        ...projection(),
        accountRiskRevision: "2",
        accountRiskDigest: "AA".repeat(32),
      },
      /accountRiskDigest/u,
    ],
  ]) {
    await withNativeBinding(
      quoteNative({
        validationFeeVerifyHijiriQuoteResponseV1() {
          return JSON.stringify(invalidProjection);
        },
      }),
      (api) => {
        assert.throws(
          () =>
            api.verifyValidationFeeHijiriQuoteResponseV1(
              RESPONSE_NORITO,
              REQUEST_NORITO,
            ),
          pattern,
        );
      },
    );
  }
});

test("Torii Hijiri quote is authenticated native Norito, bounded, and private", async () => {
  await withNativeBinding(quoteNative(), async (_api, nativeOptions) => {
    let observed = null;
    const client = new ToriiClient("https://example.test", {
      ...nativeOptions,
      fetchImpl: async () => {
        throw new Error("overridden request path should be used");
      },
    });
    client._request = async (method, path, init) => {
      observed = { method, path, init };
      return quoteResponse(RESPONSE_NORITO, {
        status: 200,
        headers: {
          "Cache-Control": "private, no-store",
          "Content-Length": String(RESPONSE_NORITO.byteLength),
          "Content-Type": "application/x-norito",
        },
      });
    };

    const quote = await client.quoteValidationFeeHijiri(ACCOUNT_ID, 2, {
      canonicalAuth: {
        accountId: "quote-reader@taira",
        privateKey: Buffer.alloc(32, 0x52),
      },
    });
    assert.deepEqual(quote, projection());
    assert.equal(Object.isFrozen(quote), true);
    assert.equal(observed.method, "POST");
    assert.equal(observed.path, "/v1/validation-fee/hijiri/quote");
    assert.deepEqual(observed.init.body, REQUEST_NORITO);
    assert.deepEqual(observed.init.headers, {
      "Content-Type": "application/x-norito",
      "Content-Encoding": "identity",
      Accept: "application/x-norito",
      "Accept-Encoding": "identity",
      "Cache-Control": "no-store",
    });
    assert.equal(observed.init.canonicalAuth.accountId, "quote-reader@taira");
  });
});

test("Torii Hijiri quote rejects cacheable or non-Norito success responses", async () => {
  await withNativeBinding(quoteNative(), async (_api, nativeOptions) => {
    for (const headers of [
      { "Content-Type": "application/x-norito" },
      {
        "Content-Type": "application/json",
        "Cache-Control": "private, no-store",
      },
      {
        "Content-Type": "application/x-norito; charset=binary",
        "Cache-Control": "private, no-store",
      },
      {
        "Content-Type": "application/x-norito",
        "Content-Encoding": "gzip",
        "Cache-Control": "private, no-store",
      },
      {
        "Content-Type": "application/x-norito",
        "Cache-Control": "private, no-store",
        "X-Iroha-Reject-Code": "should-not-appear",
      },
      {
        "Content-Type": "application/x-norito",
        "Cache-Control": "private, no-store",
        "X-Iroha-Reject-Code": "",
      },
      {
        "Content-Type": "application/x-norito",
        "Cache-Control": "private, no-store, public",
      },
      {
        "Content-Type": "application/x-norito",
        "Cache-Control": 'private="field", no-store',
      },
      {
        "Content-Type": "application/x-norito",
        "Cache-Control": "private, no-store=foo",
      },
      {
        "Content-Type": "application/x-norito",
        "Cache-Control": "private, no-store, public=max-age",
      },
      {
        "Content-Type": "application/x-norito",
        "Cache-Control": 'extension="decoy, private, no-store, suffix"',
      },
      {
        "Content-Type": "application/x-norito",
        "Cache-Control": 'private, no-store, extension="unterminated',
      },
      {
        "Content-Type": "application/x-norito",
        "Cache-Control": 'private, no-store, extension="dangling\\',
      },
      {
        "Content-Type": "application/x-norito",
        "Cache-Control": "private, no-store",
        "Content-Length": String(RESPONSE_NORITO.byteLength + 1),
      },
    ]) {
      const client = new ToriiClient("https://example.test", {
        ...nativeOptions,
        fetchImpl: async () => {
          throw new Error("overridden request path should be used");
        },
      });
      client._request = async () =>
        quoteResponse(RESPONSE_NORITO, { status: 200, headers });
      await assert.rejects(
        () =>
          client.quoteValidationFeeHijiri(ACCOUNT_ID, 2, {
            canonicalAuth: {
              accountId: "quote-reader@taira",
              privateKey: Buffer.alloc(32, 0x53),
            },
          }),
        /private and no-store|application\/x-norito|Content-Encoding|rejection code|Content-Length/u,
      );
    }
  });
});

test("Torii Hijiri quote permits commas and directive names inside a quoted extension", async () => {
  await withNativeBinding(quoteNative(), async (_api, nativeOptions) => {
    const client = new ToriiClient("https://example.test", {
      ...nativeOptions,
      fetchImpl: async () => {
        throw new Error("overridden request path should be used");
      },
    });
    client._request = async () =>
      quoteResponse(RESPONSE_NORITO, {
        status: 200,
        headers: {
          "Cache-Control":
            'private, no-store, extension="quoted, public, private, no-store"',
          "Content-Type": "application/x-norito",
        },
      });

    assert.deepEqual(
      await client.quoteValidationFeeHijiri(ACCOUNT_ID, 2, {
        canonicalAuth: {
          accountId: "quote-reader@taira",
          privateKey: Buffer.alloc(32, 0x54),
        },
      }),
      projection(),
    );
  });
});

test("Torii Hijiri quote signs the exact URL and denies redirects", async () => {
  await withNativeBinding(quoteNative(), async (_api, nativeOptions) => {
    let observed = null;
    let fetchResponse = quoteResponse(RESPONSE_NORITO, {
      status: 200,
      headers: {
        "Cache-Control": "private, no-store",
        "Content-Type": "application/x-norito",
      },
    });
    const client = new ToriiClient("https://example.test/base", {
      ...nativeOptions,
      defaultHeaders: { "Content-Encoding": "gzip" },
      localSigningContext: new LocalSigningContext(NETWORK_ID),
      fetchImpl: async (url, init) => {
        observed = { url, init };
        return fetchResponse;
      },
    });
    await client.quoteValidationFeeHijiri(ACCOUNT_ID, 2, {
      canonicalAuth: {
        accountId: ACCOUNT_ID,
        privateKey: Buffer.alloc(32, 0x56),
      },
    });

    assert.equal(
      observed.url,
      "https://example.test/v1/validation-fee/hijiri/quote",
    );
    assert.equal(observed.init.redirect, "error");
    assert.equal(
      new Headers(observed.init.headers).get("content-type"),
      "application/x-norito",
    );
    assert.equal(
      new Headers(observed.init.headers).get("accept-encoding"),
      "identity",
    );
    assert.equal(
      new Headers(observed.init.headers).get("content-encoding"),
      "identity",
    );
    assert.match(
      new Headers(observed.init.headers).get("x-iroha-signature"),
      /^[A-Za-z0-9+/]+={0,2}$/u,
    );

    fetchResponse = quoteResponse(
      RESPONSE_NORITO,
      {
        status: 200,
        headers: {
          "Cache-Control": "private, no-store",
          "Content-Type": "application/x-norito",
        },
      },
      {
        redirected: true,
        url: "https://example.test/redirected",
      },
    );
    await assert.rejects(
      () =>
        client.quoteValidationFeeHijiri(ACCOUNT_ID, 2, {
          canonicalAuth: {
            accountId: ACCOUNT_ID,
            privateKey: Buffer.alloc(32, 0x57),
          },
        }),
      /exact signed URL without redirects/u,
    );
  });
});

test("Torii Hijiri quote validates every error response before status failure", async () => {
  await withNativeBinding(
    quoteNative({
      validationFeeVerifyHijiriQuoteResponseV1() {
        assert.fail("non-200 response reached native verification");
      },
    }),
    async (_api, nativeOptions) => {
      for (const [response, pattern] of [
        [
          quoteResponse(RESPONSE_NORITO, {
            status: 503,
            headers: {
              "Cache-Control": "private, no-store",
              "Content-Type": "application/x-norito",
            },
          }),
          /unexpected status 503/u,
        ],
        [
          quoteResponse(RESPONSE_NORITO, {
            status: 503,
            headers: { "Content-Type": "application/x-norito" },
          }),
          /private and no-store/u,
        ],
        [
          quoteResponse(
            Buffer.alloc(VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES + 1),
            {
              status: 503,
              headers: {
                "Cache-Control": "private, no-store",
                "Content-Type": "application/x-norito",
              },
            },
          ),
          /exceeds the 65536-byte response limit/u,
        ],
        [
          quoteResponse(RESPONSE_NORITO, {
            status: 503,
            headers: {
              "Cache-Control": "private, no-store",
              "Content-Length": String(RESPONSE_NORITO.byteLength + 1),
              "Content-Type": "application/x-norito",
            },
          }),
          /Content-Length does not match/u,
        ],
        [
          quoteResponse(
            new ReadableStream({
              start(controller) {
                controller.enqueue(new Uint8Array(0));
              },
            }),
            {
              status: 503,
              headers: {
                "Cache-Control": "private, no-store",
                "Content-Type": "application/x-norito",
              },
            },
          ),
          /empty non-progress chunk/u,
        ],
      ]) {
        const client = new ToriiClient("https://example.test", {
          ...nativeOptions,
          fetchImpl: async () => {
            throw new Error("overridden request path should be used");
          },
        });
        client._request = async () => response;
        await assert.rejects(
          () =>
            client.quoteValidationFeeHijiri(ACCOUNT_ID, 2, {
              canonicalAuth: {
                accountId: "quote-reader@taira",
                privateKey: Buffer.alloc(32, 0x55),
              },
            }),
          pattern,
        );
      }
    },
  );
});

test("Torii Hijiri quote requires HTTPS before native encoding", async () => {
  await withNativeBinding(
    quoteNative({
      validationFeeHijiriQuoteRequestV1() {
        assert.fail("insecure quote reached native encoding");
      },
    }),
    async (_api, nativeOptions) => {
      const client = new ToriiClient("http://example.test", {
        ...nativeOptions,
        allowInsecure: true,
        fetchImpl: async () => assert.fail("insecure quote reached fetch"),
      });
      await assert.rejects(
        () =>
          client.quoteValidationFeeHijiri(ACCOUNT_ID, 2, {
            canonicalAuth: {
              accountId: "quote-reader@taira",
              privateKey: Buffer.alloc(32, 0x54),
            },
          }),
        /HTTPS/u,
      );
    },
  );
});
