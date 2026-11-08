// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

// The controller against the real `@iroha/iroha-js` Connect code, with Torii
// and the relay socket replaced by in-memory fakes.

import assert from "node:assert/strict";
import test from "node:test";

import { IrohaConnectController } from "../src/controller.js";
import sdk, { createAccountInspector } from "../src/sdk.js";
import { waitFor } from "./helpers/wait.js";

const TORII = "https://taira.sora.org";
const NETWORK_ID = sdk.NetworkId.fromBytes(new Uint8Array(32).fill(0xa5)).toString();
const TOKENS = Object.freeze({
  app: "A".repeat(43),
  wallet: "B".repeat(43),
  management: "C".repeat(43),
  relay: "D".repeat(43),
});

function installFakeTorii(t) {
  const requests = [];
  const original = globalThis.fetch;
  globalThis.fetch = async (url, init = {}) => {
    const target = new URL(String(url));
    const method = init.method ?? "GET";
    requests.push({ method, path: target.pathname, body: init.body ?? null, headers: init.headers ?? {} });
    if (method === "POST" && target.pathname === "/v1/connect/session") {
      const body = JSON.parse(init.body);
      const uri = (role, token) =>
        `iroha://connect?${new URLSearchParams({
          sid: body.sid,
          network_id: body.network_id,
          app_pk: body.app_pk,
          nonce: body.nonce,
          node: body.node,
          v: "1",
          role,
          token,
          relay: TOKENS.relay,
        })}`;
      return new Response(
        JSON.stringify({
          sid: body.sid,
          network_id: body.network_id,
          app_pk: body.app_pk,
          nonce: body.nonce,
          wallet_uri: uri("wallet", TOKENS.wallet),
          app_uri: uri("app", TOKENS.app),
          token_app: TOKENS.app,
          token_wallet: TOKENS.wallet,
          token_management: TOKENS.management,
          token_relay: TOKENS.relay,
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      );
    }
    if (method === "DELETE") {
      return new Response(null, { status: 204 });
    }
    return new Response("not found", { status: 404, statusText: "Not Found" });
  };
  t.after(() => {
    globalThis.fetch = original;
  });
  return requests;
}

class RecordingWebSocket extends EventTarget {
  static instances = [];

  constructor(url, protocols) {
    super();
    this.url = url;
    this.protocols = protocols;
    this.sent = [];
    this.readyState = 0;
    RecordingWebSocket.instances.push(this);
  }

  send(data) {
    this.sent.push(Uint8Array.from(data));
  }

  open() {
    this.readyState = 1;
    this.dispatchEvent(new Event("open"));
  }

  close() {
    this.readyState = 3;
    this.dispatchEvent(new Event("close"));
  }
}

test("the default bindings cover everything the controller needs", () => {
  for (const name of [
    "createConnectSessionPreview",
    "registerConnectSession",
    "createConnectAppSession",
    "deleteConnectSession",
    "resolveConnectLaunchUri",
    "resolveConnectLaunchUriForProtocol",
    "createConnectCanonicalRequestAuth",
    "createAccountInspector",
  ]) {
    assert.equal(typeof sdk[name], "function", name);
  }
  assert.equal(sdk.TORII_CANONICAL_REQUEST_DOMAIN_TAG, "iroha:torii:canonical-request:v1");
  assert.ok(Object.isFrozen(sdk));
});

test("pairing uses the real SDK preview, Torii registration and launch URIs", async (t) => {
  const requests = installFakeTorii(t);
  RecordingWebSocket.instances.length = 0;
  const controller = new IrohaConnectController({
    toriiUrl: TORII,
    networkId: NETWORK_ID,
    appMeta: { name: "Hanami Market", url: "https://hanami.example" },
    permissions: { methods: ["sign_raw"], resources: [sdk.TORII_CANONICAL_REQUEST_DOMAIN_TAG] },
    sdk: {
      ...sdk,
      createConnectAppSession: (options) => sdk.createConnectAppSession({ ...options, webSocketImpl: RecordingWebSocket }),
    },
  });
  const pending = controller.connect();
  pending.catch(() => undefined);
  await waitFor(() => controller.state.status === "awaiting", { label: "awaiting" });

  const { sid, walletUri, launchUri } = controller.state;
  const register = requests[0];
  assert.equal(register.method, "POST");
  assert.equal(register.path, "/v1/connect/session");
  assert.deepEqual(Object.keys(JSON.parse(register.body)).sort(), ["app_pk", "network_id", "node", "nonce", "sid"]);
  assert.equal(JSON.parse(register.body).network_id, NETWORK_ID);
  assert.equal(JSON.parse(register.body).node, TORII);
  assert.equal(JSON.parse(register.body).sid, sid);

  const wallet = new URL(walletUri);
  assert.equal(wallet.protocol, "iroha:");
  assert.equal(wallet.host, "connect");
  assert.equal(wallet.searchParams.get("role"), "wallet");
  assert.equal(wallet.searchParams.get("token"), TOKENS.wallet);
  assert.equal(wallet.searchParams.get("relay"), TOKENS.relay);
  assert.equal(wallet.searchParams.get("network_id"), NETWORK_ID);
  assert.equal(wallet.searchParams.get("node"), TORII);
  const launch = new URL(launchUri);
  assert.equal(launch.protocol, "irohaconnect:");
  assert.equal(launch.search, wallet.search);

  const [socket] = RecordingWebSocket.instances;
  assert.equal(socket.url, `wss://taira.sora.org/v1/connect/ws?sid=${encodeURIComponent(sid)}&role=app`);
  assert.equal(socket.protocols[0], `iroha-connect.token.v1.${Buffer.from(TOKENS.app).toString("base64url")}`);
  socket.open();
  assert.equal(socket.sent.length, 1, "the SDK opens the session with the app metadata and permissions");
  const openFrame = Buffer.from(socket.sent[0]);
  assert.ok(openFrame.includes(Buffer.from("Hanami Market")));
  assert.ok(openFrame.includes(Buffer.from(sdk.TORII_CANONICAL_REQUEST_DOMAIN_TAG)));

  await controller.cancel();
  await assert.rejects(pending, { code: "cancelled" });
  assert.equal(socket.readyState, 3);
  const removal = requests.find((request) => request.method === "DELETE");
  assert.equal(removal.path, `/v1/connect/session/${sid}`);
  assert.equal(removal.headers.Authorization, `Bearer ${TOKENS.management}`);
});

test("account inspection goes to the configured Torii origin", async (t) => {
  const requests = installFakeTorii(t);
  const inspect = createAccountInspector({ baseUrl: TORII, networkId: sdk.NetworkId.parse(NETWORK_ID) });
  await assert.rejects(inspect("sorau7xDemoAccount", { signal: new AbortController().signal }));
  assert.deepEqual(
    requests.map(({ method, path }) => [method, path]),
    [["POST", "/v1/accounts/inspect"]],
  );
});
