import { test } from "node:test";
import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import { bootstrapConnectPreviewSession } from "../src/connectPreviewFlow.js";

class FakeToriiClient {
  constructor(response = {}) {
    this.response = {
      sid: "0x00",
      wallet_uri: "iroha://connect?sid=AA",
      app_uri: "iroha://connect/app?sid=AA",
      token_app: "token-app",
      token_wallet: "token-wallet",
      extra: {},
      raw: {},
      ...response,
    };
    this.calls = [];
  }

  async createConnectSession(payload) {
    this.calls.push(payload);
    return {
      ...this.response,
      sid: payload.sid,
    };
  }
}

function fixedPreviewOptions(overrides = {}) {
  return {
    chainId: "alpha-net",
    node: "torii.devnet.example",
    nonce: Buffer.alloc(16, 0x11),
    appKeyPair: {
      publicKey: Buffer.alloc(32, 0x22),
      privateKey: Buffer.alloc(32, 0x33),
    },
    ...overrides,
  };
}

test("bootstrapConnectPreviewSession registers by default", async () => {
  const client = new FakeToriiClient({
    token_app: "app-token",
    token_wallet: "wallet-token",
  });
  const result = await bootstrapConnectPreviewSession(
    client,
    fixedPreviewOptions(),
  );
  assert.match(result.preview.sidBase64Url, /^[A-Za-z0-9_-]+$/);
  assert.equal(result.preview.sidBase64Url.includes("="), false);
  assert.equal(result.session?.token_app, "app-token");
  assert.equal(result.tokens?.wallet, "wallet-token");
  assert.equal(client.calls.length, 1);
  assert.equal(client.calls[0].sid, result.preview.sidBase64Url);
  assert.equal(client.calls[0].node, "torii.devnet.example");
});

test("bootstrapConnectPreviewSession honours overrides and can skip registration", async () => {
  const client = new FakeToriiClient();
  const result = await bootstrapConnectPreviewSession(client, {
    ...fixedPreviewOptions({ node: null }),
    register: false,
    sessionOptions: { node: "torii.override" },
  });
  assert.match(result.preview.sidBase64Url, /^[A-Za-z0-9_-]+$/);
  assert.equal(result.session, null);
  assert.equal(result.tokens, null);
  assert.equal(client.calls.length, 0);

  const registered = await bootstrapConnectPreviewSession(client, {
    ...fixedPreviewOptions({ node: null }),
    sessionOptions: { node: "torii.override" },
  });
  assert.equal(client.calls.length, 1);
  assert.equal(client.calls[0].node, "torii.override");
  assert.equal(registered.session?.sid, registered.preview.sidBase64Url);
});
