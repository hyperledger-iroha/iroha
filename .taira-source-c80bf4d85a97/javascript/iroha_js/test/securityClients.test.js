import assert from "node:assert/strict";
import test from "node:test";
import { ToriiClient } from "../src/index.js";

function makeResponse(status = 200, body = null) {
  return {
    status,
    statusText: status === 200 ? "OK" : "ERR",
    async json() {
      return body;
    },
  };
}

test("ToriiClient rejects http base URL when credentials are present", () => {
  assert.throws(
    () =>
      new ToriiClient("http://127.0.0.1:8080", {
        authToken: "secret",
        fetchImpl: async () => makeResponse(),
      }),
  );
});

test("ToriiClient allows insecure base URL when allowInsecure is true", async () => {
  let called = false;
  const client = new ToriiClient("http://127.0.0.1:8080", {
    authToken: "secret",
    allowInsecure: true,
    fetchImpl: async () => {
      called = true;
      return makeResponse();
    },
  });
  const response = await client._request("GET", "/v1/health");
  assert(called);
  assert.equal(response.status, 200);
});

test("ToriiClient blocks credentialed requests to a mismatched host", async () => {
  const client = new ToriiClient("https://torii.example", {
    apiToken: "secret",
    fetchImpl: async () => makeResponse(),
  });
  await assert.rejects(() =>
    client._request("GET", "https://evil.example/v1/status"),
  );
});
