import { test } from "node:test";
import assert from "node:assert/strict";
import { NoritoRpcClient, NoritoRpcError } from "../src/noritoRpcClient.js";

const BASE_URL = "https://localhost:8080";

test("call posts Norito payload with default headers", async () => {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url, init });
    return createResponse({
      status: 200,
      arrayData: new Uint8Array([9, 9, 9]),
    });
  };
  const client = new NoritoRpcClient(BASE_URL, {
    fetchImpl,
    defaultHeaders: { Authorization: "Bearer token" },
  });
  const payload = new Uint8Array([1, 2, 3]);
  const bytes = await client.call("/v1/pipeline/submit", payload);
  assert.deepEqual([...bytes], [9, 9, 9]);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, `${BASE_URL}/v1/pipeline/submit`);
  assert.equal(calls[0].init.method, "POST");
  assert.equal(calls[0].init.headers.Authorization, "Bearer token");
  assert.equal(calls[0].init.headers["Content-Type"], "application/x-norito");
  assert.equal(calls[0].init.headers.Accept, "application/x-norito");
});

test("call merges params, headers, and method overrides", async () => {
  let initCapture;
  let urlCapture;
  const fetchImpl = async (url, init) => {
    initCapture = init;
    urlCapture = url;
    return createResponse({
      status: 200,
      arrayData: new Uint8Array([4]),
    });
  };
  const client = new NoritoRpcClient(BASE_URL, { fetchImpl });
  await client.call("v1/custom", new Uint8Array([0xff]), {
    method: "put",
    headers: { Accept: "application/json", "X-Test": "yes" },
    accept: null,
    params: { page: 2, limit: 10 },
  });
  assert.equal(urlCapture, `${BASE_URL}/v1/custom?page=2&limit=10`);
  assert.equal(initCapture.method, "PUT");
  assert.equal(initCapture.headers["Content-Type"], "application/x-norito");
  assert.equal(initCapture.headers["X-Test"], "yes");
  assert.ok(!("Accept" in initCapture.headers));
});

test("call attaches apiToken as X-API-Token only", async () => {
  let initCapture;
  const fetchImpl = async (_url, init) => {
    initCapture = init;
    return createResponse({
      status: 200,
      arrayData: new Uint8Array([1]),
    });
  };
  const client = new NoritoRpcClient(BASE_URL, { fetchImpl, apiToken: "token" });
  await client.call("/v1/pipeline/submit", new Uint8Array([0x01]));
  assert.equal(initCapture.headers["X-API-Token"], "token");
  assert.equal(initCapture.headers["X-Iroha-API-Token"], undefined);
});

test("call throws NoritoRpcError on non-2xx status", async () => {
  const client = new NoritoRpcClient(BASE_URL, {
    fetchImpl: async () =>
      createResponse({
        status: 503,
        textBody: "backend unavailable",
      }),
  });
  await assert.rejects(
    () => client.call("/v1/pipeline/submit", new Uint8Array([0])),
    (error) => {
      assert.ok(error instanceof NoritoRpcError);
      assert.equal(error.status, 503);
      assert.equal(error.body, "backend unavailable");
      return true;
    },
  );
});

test("call enforces timeout via AbortController", async () => {
  const client = new NoritoRpcClient(BASE_URL, {
    fetchImpl: (url, init) =>
      new Promise((_, reject) => {
        init.signal?.addEventListener(
          "abort",
          () => reject(new Error("aborted")),
          { once: true },
        );
      }),
  });
  await assert.rejects(
    () =>
      client.call("/v1/pipeline/status", new Uint8Array([1]), {
        timeoutMs: 5,
      }),
    /aborted/,
  );
});

function createResponse({ status, arrayData, textBody, headers }) {
  return {
    status,
    arrayBuffer: async () => {
      if (arrayData instanceof ArrayBuffer) {
        return arrayData;
      }
      if (ArrayBuffer.isView(arrayData)) {
        return arrayData.buffer.slice(
          arrayData.byteOffset,
          arrayData.byteOffset + arrayData.byteLength,
        );
      }
      if (arrayData == null) {
        return new Uint8Array().buffer;
      }
      return Uint8Array.from(arrayData).buffer;
    },
    text: async () => (textBody !== undefined ? textBody : ""),
    headers: {
      get(name) {
        if (!headers) {
          return null;
        }
        const normalized = name.toLowerCase();
        for (const [key, value] of Object.entries(headers)) {
          if (key.toLowerCase() === normalized) {
            return value;
          }
        }
        return null;
      },
    },
  };
}
