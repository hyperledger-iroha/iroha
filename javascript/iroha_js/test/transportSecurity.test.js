import assert from "node:assert/strict";
import test from "node:test";
import { ToriiClient } from "../src/toriiClient.js";
import { NoritoRpcClient } from "../src/noritoRpcClient.js";
import { generateKeyPair } from "../src/index.js";
import { AccountAddress } from "../src/address.js";

test("ToriiClient rejects cross-host absolute URLs when credentials are attached", async () => {
  const client = new ToriiClient("https://torii.primary.example", {
    authToken: "secret-token",
    fetchImpl: async () => ({ status: 200 }),
  });
  await assert.rejects(
    () => client._request("GET", "https://torii.other.example/v1/accounts"),
    /mismatched host/i,
  );
});

test("ToriiClient rejects scheme overrides when credentials are attached", async () => {
  const client = new ToriiClient("https://torii.primary.example", {
    authToken: "secret-token",
    fetchImpl: async () => ({ status: 200 }),
  });
  await assert.rejects(
    () => client._request("GET", "http://torii.primary.example/v1/accounts"),
    /mismatched scheme/i,
  );
});

test("ToriiClient rejects insecure transport when body contains private_key material", async () => {
  const client = new ToriiClient("http://torii.primary.example", {
    fetchImpl: async () => ({ status: 200 }),
  });
  await assert.rejects(
    () =>
      client._request("POST", "/v1/subscriptions/plans", {
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ authority: "i105:test", private_key: "ed25519:deadbeef" }),
      }),
    /sensitive request material/i,
  );
});

test("ToriiClient rejects cross-host absolute URLs when body contains private_key material", async () => {
  const client = new ToriiClient("https://torii.primary.example", {
    fetchImpl: async () => ({ status: 200 }),
  });
  await assert.rejects(
    () =>
      client._request("POST", "https://torii.other.example/v1/subscriptions/plans", {
        allowAbsoluteUrl: true,
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ authority: "i105:test", private_key: "ed25519:deadbeef" }),
      }),
    /mismatched host/i,
  );
});

test("ToriiClient rejects insecure transport when canonicalAuth is present", async () => {
  const client = new ToriiClient("http://torii.primary.example", {
    fetchImpl: async () => ({ status: 200 }),
  });
  const { publicKey } = generateKeyPair({ seed: Buffer.alloc(32, 0x21) });
  const accountId = AccountAddress.fromAccount({ publicKey }).toI105();
  await assert.rejects(
    () =>
      client._request("GET", "/v1/accounts", {
        canonicalAuth: {
          accountId,
          privateKey: Buffer.alloc(32, 0x11),
        },
      }),
    /sensitive request material/i,
  );
});

test("ToriiClient requires allowAbsoluteUrl for cross-host requests without credentials", async () => {
  const calls = [];
  const client = new ToriiClient("https://torii.safe.example", {
    fetchImpl: async (url, init) => {
      calls.push({ url, init });
      return { status: 200 };
    },
  });
  await assert.rejects(
    () => client._request("GET", "https://other.example/v1/status"),
    /absolute URLs are blocked/i,
  );
  assert.equal(calls.length, 0);

  await client._request("GET", "https://other.example/v1/status", {
    allowAbsoluteUrl: true,
  });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "https://other.example/v1/status");
});

test("ToriiClient emits telemetry when allowInsecure is used with credentials", async () => {
  const events = [];
  const client = new ToriiClient("http://localhost:8080", {
    authToken: "dev-token",
    allowInsecure: true,
    insecureTransportTelemetryHook: (event) => events.push(event),
    fetchImpl: async () => ({ status: 200 }),
  });
  await client._request("GET", "/v1/health");
  assert.equal(events.length, 1);
  const event = events[0];
  assert.equal(event.client, "torii");
  assert.equal(event.protocol, "http:");
  assert.equal(event.method, "GET");
  assert(event.allowInsecure);
  assert(event.hasCredentials);
});

test("ToriiClient emits telemetry when allowInsecure is used with private_key request bodies", async () => {
  const events = [];
  const client = new ToriiClient("http://localhost:8080", {
    allowInsecure: true,
    insecureTransportTelemetryHook: (event) => events.push(event),
    fetchImpl: async () => ({ status: 200 }),
  });
  await client._request("POST", "/v1/subscriptions/plans", {
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ authority: "i105:test", private_key: "ed25519:deadbeef" }),
  });
  assert.equal(events.length, 1);
  const event = events[0];
  assert.equal(event.client, "torii");
  assert.equal(event.protocol, "http:");
  assert.equal(event.method, "POST");
  assert.equal(event.hasCredentials, false);
  assert.equal(event.hasSensitiveBody, true);
  assert.equal(event.hasCanonicalAuth, false);
});

test("NoritoRpcClient rejects insecure base URLs when credentials are configured", () => {
  assert.throws(
    () =>
      new NoritoRpcClient("http://localhost:8080", {
        authToken: "secret",
        fetchImpl: async () => ({ status: 200, arrayBuffer: async () => new ArrayBuffer(0) }),
      }),
    /https base URL/,
  );
});

test("NoritoRpcClient blocks cross-host absolute URLs when credentials are attached", async () => {
  const client = new NoritoRpcClient("https://torii.primary.example", {
    authToken: "secret",
    fetchImpl: async () => ({ status: 200, arrayBuffer: async () => new ArrayBuffer(0) }),
  });
  await assert.rejects(
    () =>
      client.call("https://torii.secondary.example/v1/pipeline/submit", new Uint8Array([0x01])),
    /host override/i,
  );
});

test("NoritoRpcClient allows absolute URLs without credentials when allowAbsoluteUrl is set", async () => {
  const calls = [];
  const client = new NoritoRpcClient("https://torii.primary.example", {
    fetchImpl: async (url, init) => {
      calls.push({ url, init });
      return { status: 200, arrayBuffer: async () => new Uint8Array([7]).buffer };
    },
  });
  await assert.rejects(
    () =>
      client.call("https://torii.secondary.example/v1/pipeline/status", new Uint8Array([0x02])),
    /absolute URLs are blocked/i,
  );
  assert.equal(calls.length, 0);

  const bytes = await client.call(
    "https://torii.secondary.example/v1/pipeline/status",
    new Uint8Array([0x03]),
    { allowAbsoluteUrl: true },
  );
  assert.deepEqual([...bytes], [7]);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "https://torii.secondary.example/v1/pipeline/status");
});

test("NoritoRpcClient emits telemetry when allowInsecure is used with credentials", async () => {
  const events = [];
  const client = new NoritoRpcClient("http://localhost:8080", {
    authToken: "dev-token",
    allowInsecure: true,
    insecureTransportTelemetryHook: (event) => events.push(event),
    fetchImpl: async () => ({ status: 200, arrayBuffer: async () => new ArrayBuffer(0) }),
  });
  await client.call("/v1/pipeline/submit", new Uint8Array([0x04]));
  assert.equal(events.length, 1);
  const event = events[0];
  assert.equal(event.client, "norito-rpc");
  assert.equal(event.protocol, "http:");
  assert.equal(event.method, "POST");
  assert(event.allowInsecure);
  assert(event.hasCredentials);
});
