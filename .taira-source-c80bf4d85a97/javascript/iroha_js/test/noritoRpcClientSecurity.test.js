import assert from "node:assert/strict";
import test from "node:test";
import { NoritoRpcClient } from "../src/index.js";

function makeResponse(status = 200, body = Buffer.alloc(0)) {
  return {
    status,
    async arrayBuffer() {
      return body;
    },
    async text() {
      return "";
    },
  };
}

test("NoritoRpcClient rejects insecure base with credentials", () => {
  assert.throws(
    () =>
      new NoritoRpcClient("http://127.0.0.1:8080", {
        defaultHeaders: { Authorization: "Bearer secret" },
        fetchImpl: async () => makeResponse(),
      }),
  );
});

test("NoritoRpcClient allows insecure base when allowInsecure is true", async () => {
  let called = false;
  const client = new NoritoRpcClient("http://127.0.0.1:8080", {
    allowInsecure: true,
    defaultHeaders: { Authorization: "Bearer secret" },
    fetchImpl: async () => {
      called = true;
      return makeResponse();
    },
  });
  await client.call("/v1/pipeline/submit", Buffer.alloc(1));
  assert(called);
});

test("NoritoRpcClient blocks credentialed absolute host override", async () => {
  const client = new NoritoRpcClient("https://torii.example", {
    defaultHeaders: { Authorization: "Bearer secret" },
    fetchImpl: async () => makeResponse(),
  });
  await assert.rejects(() =>
    client.call("https://evil.example/v1/pipeline/submit", Buffer.alloc(1)),
  );
});
