import assert from "node:assert/strict";
import test from "node:test";

import {
  buildConnectTokenProtocol,
  buildConnectWebSocketUrl,
  createConnectSessionPreview,
  deleteConnectSession,
  openConnectWebSocket,
  registerConnectSession,
  resolveConnectLaunchUri,
  resolveConnectLaunchUriForProtocol,
  rewriteConnectUriProtocol,
  toHex,
} from "../src/connect.browser.js";

class RecordingWebSocket {
  constructor(url, protocols) {
    this.url = url;
    this.protocols = protocols;
    RecordingWebSocket.instances.push(this);
  }
}
RecordingWebSocket.instances = [];

test("createConnectSessionPreview is deterministic with fixed nonce and keypair", () => {
  const options = {
    chainId: "alpha-net",
    node: "https://taira.sora.org",
    nonce: new Uint8Array(16).fill(0x11),
    appKeyPair: {
      publicKey: new Uint8Array(32).fill(0x22),
      privateKey: new Uint8Array(32).fill(0x33),
    },
  };

  const first = createConnectSessionPreview(options);
  const second = createConnectSessionPreview(options);

  assert.equal(first.sidBase64Url, second.sidBase64Url);
  assert.equal(first.sidBase64Url.includes("="), false);
  assert.equal(toHex(first.nonce), "11".repeat(16));
  assert.equal(first.walletUri, `iroha://connect?sid=${first.sidBase64Url}&chain_id=alpha-net&v=1&role=wallet&node=https%3A%2F%2Ftaira.sora.org`);
  assert.equal(first.appUri, `iroha://connect?sid=${first.sidBase64Url}&chain_id=alpha-net&v=1&role=app&node=https%3A%2F%2Ftaira.sora.org`);
  assert.equal(
    first.wsUrl,
    `wss://taira.sora.org/v1/connect/ws?sid=${first.sidBase64Url}&role=app`,
  );
});

test("buildConnectWebSocketUrl switches schemes for secure and insecure Torii urls", () => {
  assert.equal(
    buildConnectWebSocketUrl("https://taira.sora.org", "sid123", "app"),
    "wss://taira.sora.org/v1/connect/ws?sid=sid123&role=app",
  );
  assert.equal(
    buildConnectWebSocketUrl("http://127.0.0.1:8080", "sid123", "wallet"),
    "ws://127.0.0.1:8080/v1/connect/ws?sid=sid123&role=wallet",
  );
});

test("registerConnectSession posts sid and node directly to Torii", async () => {
  const calls = [];
  const response = await registerConnectSession("https://taira.sora.org", "sid123", {
    node: "https://taira.sora.org",
    fetchImpl: async (url, init) => {
      calls.push({ url: String(url), init });
      return new Response(
        JSON.stringify({
          sid: "sid123",
          wallet_uri: "iroha://connect?sid=sid123&role=wallet&token=wallet-token",
          app_uri: "iroha://connect?sid=sid123&role=app&token=app-token",
          token_app: "app-token",
          token_wallet: "wallet-token",
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      );
    },
  });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "https://taira.sora.org/v1/connect/session");
  assert.equal(calls[0].init.method, "POST");
  assert.equal(calls[0].init.body, JSON.stringify({ sid: "sid123", node: "https://taira.sora.org" }));
  assert.equal(response.token_app, "app-token");
});

test("deleteConnectSession tolerates missing sessions and uses DELETE", async () => {
  const calls = [];
  await deleteConnectSession("https://taira.sora.org", "sid123", {
    fetchImpl: async (url, init) => {
      calls.push({ url: String(url), init });
      return new Response("", { status: 404 });
    },
  });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "https://taira.sora.org/v1/connect/session/sid123");
  assert.equal(calls[0].init.method, "DELETE");
});

test("resolveConnectLaunchUri prefers canonical session deeplinks", () => {
  assert.equal(
    resolveConnectLaunchUri(
      "wallet",
      {
        walletUri: "iroha://connect?sid=preview&role=wallet",
        appUri: "iroha://connect?sid=preview&role=app",
      },
      {
        wallet_uri: "iroha://connect?sid=session&role=wallet&token=wallet-token",
        app_uri: "iroha://connect?sid=session&role=app&token=app-token",
      },
    ),
    "iroha://connect?sid=session&role=wallet&token=wallet-token",
  );
});

test("rewriteConnectUriProtocol swaps the scheme without changing the session payload", () => {
  assert.equal(
    rewriteConnectUriProtocol(
      "iroha://connect?sid=session&role=wallet&token=wallet-token",
    ),
    "irohaconnect://connect?sid=session&role=wallet&token=wallet-token",
  );
  assert.equal(
    rewriteConnectUriProtocol(
      "iroha://connect?sid=session&role=wallet&token=wallet-token",
      "irohaconnect:",
    ),
    "irohaconnect://connect?sid=session&role=wallet&token=wallet-token",
  );
});

test("resolveConnectLaunchUriForProtocol rewrites the selected launch URI", () => {
  assert.equal(
    resolveConnectLaunchUriForProtocol(
      "wallet",
      {
        walletUri: "iroha://connect?sid=preview&role=wallet",
        appUri: "iroha://connect?sid=preview&role=app",
      },
      {
        wallet_uri: "iroha://connect?sid=session&role=wallet&token=wallet-token",
        app_uri: "iroha://connect?sid=session&role=app&token=app-token",
      },
    ),
    "irohaconnect://connect?sid=session&role=wallet&token=wallet-token",
  );
});

test("openConnectWebSocket sends the connect token as the first subprotocol", () => {
  RecordingWebSocket.instances.length = 0;
  const socket = openConnectWebSocket("https://taira.sora.org", "sid123", "token-app", "app", {
    webSocketImpl: RecordingWebSocket,
    protocols: ["iroha-connect"],
  });

  assert(socket instanceof RecordingWebSocket);
  assert.equal(RecordingWebSocket.instances.length, 1);
  assert.equal(
    RecordingWebSocket.instances[0].url,
    "wss://taira.sora.org/v1/connect/ws?sid=sid123&role=app",
  );
  assert.deepEqual(RecordingWebSocket.instances[0].protocols, [
    buildConnectTokenProtocol("token-app"),
    "iroha-connect",
  ]);
});
