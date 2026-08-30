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
  assert.equal(calls[0].init.redirect, "error");
});

test("client keeps transport state private and snapshots credential headers", async () => {
  let headers;
  const defaultHeaders = {
    Authorization: "Bearer header-token",
    "X-Trace": "trace-1",
  };
  const client = new NoritoRpcClient(BASE_URL, {
    apiToken: "api-secret",
    authToken: "auth-secret",
    defaultHeaders,
    fetchImpl: async (_url, init) => {
      headers = init.headers;
      return createResponse({ status: 200 });
    },
  });

  defaultHeaders.Authorization = "Bearer mutated";
  defaultHeaders["X-Trace"] = "trace-2";

  assert.deepEqual(Object.keys(client), []);
  assert.equal(JSON.stringify(client), "{}");
  assert.equal(client._authToken, undefined);
  assert.equal(client._defaultHeaders, undefined);
  assert.equal(client.close, undefined);
  assert.equal(client.baseUrl, BASE_URL);

  await client.call("/v1/pipeline/submit", new Uint8Array([1]));
  assert.equal(headers.Authorization, "Bearer auth-secret");
  assert.equal(headers["X-API-Token"], "api-secret");
  assert.equal(headers["X-Trace"], "trace-1");
});

test("per-call token options override or remove constructor credentials", async () => {
  const calls = [];
  const client = new NoritoRpcClient(BASE_URL, {
    apiToken: "default-api",
    authToken: "default-auth",
    fetchImpl: async (_url, init) => {
      calls.push(init.headers);
      return createResponse({ status: 200 });
    },
  });

  await client.call("/v1/override", new Uint8Array([1]), {
    apiToken: null,
    authToken: "request-auth",
  });
  assert.equal(calls[0].Authorization, "Bearer request-auth");
  assert.equal(calls[0]["X-API-Token"], undefined);
});

test("call snapshots every supported caller-owned payload container", async () => {
  const captured = [];
  const client = new NoritoRpcClient(BASE_URL, {
    fetchImpl: async (_url, init) => {
      captured.push(init.body);
      return createResponse({ status: 200 });
    },
  });
  const buffer = Buffer.from([1, 2, 3]);
  const bytes = new Uint8Array([4, 5, 6]);
  const arrayBuffer = Uint8Array.from([7, 8, 9]).buffer;
  const dataViewBytes = new Uint8Array([10, 11, 12]);
  const dataView = new DataView(dataViewBytes.buffer);

  const calls = [
    client.call("/buffer", buffer),
    client.call("/bytes", bytes),
    client.call("/array-buffer", arrayBuffer),
    client.call("/data-view", dataView),
  ];
  buffer.fill(99);
  bytes.fill(99);
  new Uint8Array(arrayBuffer).fill(99);
  dataViewBytes.fill(99);
  await Promise.all(calls);

  assert.deepEqual(captured.map((body) => [...body]), [
    [1, 2, 3],
    [4, 5, 6],
    [7, 8, 9],
    [10, 11, 12],
  ]);
});

test("constructor rejects coercive security and timeout options", () => {
  const fetchImpl = async () => createResponse({ status: 200 });
  for (const allowInsecure of [null, 0, 1, "true"]) {
    assert.throws(
      () => new NoritoRpcClient(BASE_URL, { allowInsecure, fetchImpl }),
      /allowInsecure must be a boolean/u,
    );
  }
  for (const timeoutMs of [-1, 1.5, Number.NaN, Number.POSITIVE_INFINITY, "5"]) {
    assert.throws(
      () => new NoritoRpcClient(BASE_URL, { fetchImpl, timeoutMs }),
      /timeoutMs must be a non-negative safe integer or null/u,
    );
  }
  assert.throws(
    () => new NoritoRpcClient(BASE_URL, { authToken: 42, fetchImpl }),
    /authToken must be a string or null/u,
  );
  assert.throws(
    () =>
      new NoritoRpcClient(BASE_URL, {
        fetchImpl,
        insecureTransportTelemetryHook: "log",
      }),
    /insecureTransportTelemetryHook must be a function/u,
  );
});

test("constructor accepts only exact credential-free HTTP roots and headers", () => {
  const fetchImpl = async () => createResponse({ status: 200 });
  for (const baseUrl of [
    " https://torii.example",
    "ftp://torii.example",
    "https://user:password@torii.example",
    "https://torii.example?target=other",
    "https://torii.example#fragment",
  ]) {
    assert.throws(
      () => new NoritoRpcClient(baseUrl, { fetchImpl }),
      /baseUrl/u,
    );
  }
  assert.throws(
    () => new NoritoRpcClient(BASE_URL, {
      defaultHeaders: { Authorization: "Bearer secret\r\nX-Injected: yes" },
      fetchImpl,
    }),
    /single-line string/u,
  );
  assert.equal(
    new NoritoRpcClient("https://torii.example/proxy/", { fetchImpl }).baseUrl,
    "https://torii.example/proxy",
  );
});

test("call rejects coercive options before dispatch", async () => {
  let calls = 0;
  const client = new NoritoRpcClient(BASE_URL, {
    fetchImpl: async () => {
      calls += 1;
      return createResponse({ status: 200 });
    },
  });

  await assert.rejects(
    () => client.call("/timeout", new Uint8Array([1]), { timeoutMs: "5" }),
    /timeoutMs must be a non-negative safe integer or null/u,
  );
  await assert.rejects(
    () =>
      client.call("/absolute", new Uint8Array([1]), {
        allowAbsoluteUrl: 1,
      }),
    /allowAbsoluteUrl must be a boolean/u,
  );
  await assert.rejects(
    () => client.call("/method", new Uint8Array([1]), { method: 1 }),
    /method must be a non-empty HTTP token/u,
  );
  await assert.rejects(
    () => client.call("ftp://other.example/rpc", new Uint8Array([1]), {
      allowAbsoluteUrl: true,
    }),
    /must use http or https/u,
  );
  await assert.rejects(
    () => client.call("/headers", new Uint8Array([1]), {
      headers: { "X-Test": "value\nX-Injected: yes" },
    }),
    /single-line string/u,
  );
  assert.equal(calls, 0);
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

test("call does not retry a signed query on a retryable HTTP status", async () => {
  let attempts = 0;
  const client = new NoritoRpcClient(BASE_URL, {
    fetchImpl: async (_url, init) => {
      attempts += 1;
      assert.equal(init.redirect, "error");
      return createResponse({
        status: 503,
        textBody: "backend unavailable",
      });
    },
  });
  await assert.rejects(
    () => client.call("/query", new Uint8Array([0])),
    (error) => {
      assert.ok(error instanceof NoritoRpcError);
      assert.equal(error.status, 503);
      assert.equal(error.body, "backend unavailable");
      return true;
    },
  );
  assert.equal(attempts, 1);
});

test("call does not retry a signed query after a network failure", async () => {
  let attempts = 0;
  const networkError = new TypeError("socket closed after dispatch");
  const client = new NoritoRpcClient(BASE_URL, {
    fetchImpl: async (_url, init) => {
      attempts += 1;
      assert.equal(init.redirect, "error");
      throw networkError;
    },
  });

  await assert.rejects(
    () => client.call("/query", new Uint8Array([0x01])),
    (error) => error === networkError,
  );
  assert.equal(attempts, 1);
});

for (const redirectStatus of [307, 308]) {
  test(`call rejects signed-query ${redirectStatus} without redirecting or retrying`, async () => {
    let attempts = 0;
    const client = new NoritoRpcClient(BASE_URL, {
      fetchImpl: async (_url, init) => {
        attempts += 1;
        assert.equal(init.redirect, "error");
        return createResponse({
          status: redirectStatus,
          headers: { location: "https://redirect.example/replayed-query" },
        });
      },
    });

    await assert.rejects(
      () => client.call("/query", new Uint8Array([0x02])),
      (error) =>
        error instanceof NoritoRpcError && error.status === redirectStatus,
    );
    assert.equal(attempts, 1);
  });
}

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
