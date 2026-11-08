// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import test from "node:test";

import { IrohaConnectController, IrohaConnectError } from "../src/controller.js";
import { createFakeSdk, walletRejection } from "./helpers/fakeSdk.js";
import { waitFor } from "./helpers/wait.js";

const TORII = "https://taira.sora.org";
const NETWORK_ID = "hash:A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5#95D7";

function createController(overrides = {}) {
  const fake = createFakeSdk(overrides.fakeOptions);
  const controller = new IrohaConnectController({
    toriiUrl: TORII,
    networkId: NETWORK_ID,
    appMeta: { name: "Hanami Market", url: "https://hanami.example" },
    sdk: fake.sdk,
    ...overrides.options,
  });
  const states = [];
  const events = [];
  controller.addEventListener("statechange", (event) => states.push(event.detail.status));
  for (const type of ["approved", "disconnected", "error"]) {
    controller.addEventListener(type, (event) => events.push([type, event.detail]));
  }
  return { controller, fake, states, events };
}

test("connect walks preparing, awaiting and connected with the SDK's exact inputs", async () => {
  const { controller, fake, states, events } = createController({
    options: { permissions: { methods: ["sign_raw"], resources: ["iroha:torii:canonical-request:v1"] } },
  });
  const pending = controller.connect();
  assert.equal(controller.state.status, "preparing");
  await waitFor(() => controller.state.status === "awaiting", { label: "awaiting" });

  const state = controller.state;
  assert.equal(state.walletUri, "iroha://connect?sid=sid-1&role=wallet&token=wallet-1&relay=relay-1");
  assert.equal(state.launchUri, "irohaconnect://connect?sid=sid-1&role=wallet&token=wallet-1&relay=relay-1");
  assert.equal(state.sid, "sid-1");
  assert.equal(state.toriiUrl, TORII);

  assert.equal(fake.calls.preview.length, 1);
  assert.equal(fake.calls.preview[0].node, TORII);
  assert.equal(fake.calls.preview[0].networkId.literal, NETWORK_ID);
  assert.deepEqual(fake.calls.register[0].options, { node: TORII });
  assert.equal(fake.calls.register[0].baseUrl, TORII);
  const sessionOptions = fake.calls.appSession[0];
  assert.equal(sessionOptions.baseUrl, TORII);
  assert.equal(sessionOptions.session.token_app, "app-1");
  assert.equal(sessionOptions.preview, fake.calls.register[0].preview);
  assert.deepEqual(sessionOptions.appMeta, { name: "Hanami Market", url: "https://hanami.example", iconHash: null });
  assert.deepEqual(sessionOptions.permissions, {
    methods: ["sign_raw"],
    events: undefined,
    resources: ["iroha:torii:canonical-request:v1"],
  });
  assert.equal(sessionOptions.allowInsecure, false);
  assert.equal(typeof sessionOptions.inspectAccountIdentity, "function");
  assert.equal(fake.calls.inspector[0].baseUrl, TORII);
  assert.equal(fake.calls.inspector[0].networkId.literal, NETWORK_ID);

  fake.sessions[0].approve("sorau7xApprovedAccount");
  const approval = await pending;
  assert.equal(approval.accountId, "sorau7xApprovedAccount");
  assert.deepEqual(approval.signingPublicKey, new Uint8Array(32).fill(7));
  assert.equal(controller.state.status, "connected");
  assert.equal(controller.accountId, "sorau7xApprovedAccount");
  assert.equal(controller.appSession, fake.sessions[0]);
  assert.deepEqual(states, ["preparing", "awaiting", "connected"]);
  assert.deepEqual(events.map(([type]) => type), ["approved"]);
  assert.equal(events[0][1], approval);
  assert.equal(fake.calls.delete.length, 0);
});

test("connect returns the in-flight promise and resolves at once when connected", async () => {
  const { controller, fake } = createController();
  const first = controller.connect();
  assert.equal(controller.connect(), first);
  await waitFor(() => controller.state.status === "awaiting");
  fake.sessions[0].approve();
  const approval = await first;
  assert.equal(await controller.connect(), approval);
  assert.equal(fake.calls.register.length, 1);
});

test("cancelling while the wallet is pending closes the relay socket and deletes the Torii session", async () => {
  const { controller, fake, states } = createController();
  const pending = controller.connect();
  await waitFor(() => controller.state.status === "awaiting");
  await controller.cancel();
  await assert.rejects(pending, (error) => error instanceof IrohaConnectError && error.code === "cancelled");
  assert.equal(controller.state.status, "idle");
  assert.deepEqual(fake.sessions[0].closedWith, ["cancelled"]);
  assert.deepEqual(fake.calls.delete, [
    { baseUrl: TORII, sid: "sid-1", options: { tokenManagement: "management-1" } },
  ]);
  assert.deepEqual(states, ["preparing", "awaiting", "idle"]);
});

test("a registration that completes after cancel is deleted and never opened", async () => {
  let finishRegistration;
  const { controller, fake } = createController({
    fakeOptions: {
      register: () =>
        new Promise((resolve) => {
          finishRegistration = () =>
            resolve({
              sid: "late",
              wallet_uri: "iroha://connect?sid=late&role=wallet",
              app_uri: "iroha://connect?sid=late&role=app",
              token_app: "a",
              token_wallet: "w",
              token_management: "late-management",
              token_relay: "r",
            });
        }),
    },
  });
  const pending = controller.connect();
  await waitFor(() => typeof finishRegistration === "function");
  await controller.cancel();
  finishRegistration();
  await assert.rejects(pending, { code: "cancelled" });
  assert.equal(fake.calls.appSession.length, 0);
  assert.deepEqual(fake.calls.delete, [
    { baseUrl: TORII, sid: "late", options: { tokenManagement: "late-management" } },
  ]);
  assert.equal(controller.state.status, "idle");
});

test("a wallet rejection becomes an error state and the session is cleaned up", async () => {
  const { controller, fake, events } = createController();
  const pending = controller.connect();
  await waitFor(() => controller.state.status === "awaiting");
  fake.sessions[0].reject(walletRejection("user declined"));
  await assert.rejects(pending, { code: "rejected", message: "user declined" });
  assert.equal(controller.state.status, "error");
  assert.equal(controller.state.code, "rejected");
  assert.equal(events.at(-1)[0], "error");
  assert.equal(events.at(-1)[1].code, "rejected");
  assert.equal(fake.calls.delete.length, 1);
  assert.deepEqual(fake.sessions[0].closedWith, ["failed"]);
});

test("unreachable Torii is reported as a network error and retry starts a new session", async () => {
  let failures = 1;
  const { controller, fake } = createController({
    fakeOptions: {
      register: (baseUrl, preview, options) => {
        if (failures > 0) {
          failures -= 1;
          throw new TypeError("Failed to fetch");
        }
        return createFakeSdk().sdk.registerConnectSession(baseUrl, preview, options);
      },
    },
  });
  await assert.rejects(controller.connect(), { code: "network" });
  assert.equal(controller.state.status, "error");
  assert.equal(controller.state.code, "network");
  const retry = controller.connect();
  await waitFor(() => controller.state.status === "awaiting");
  fake.sessions[0].approve();
  await retry;
  assert.equal(controller.state.status, "connected");
});

test("a session the wallet closes after approval returns to idle with a remote disconnect", async () => {
  const { controller, fake, events } = createController();
  const pending = controller.connect();
  await waitFor(() => controller.state.status === "awaiting");
  fake.sessions[0].approve();
  await pending;
  fake.sessions[0].remoteClose();
  assert.equal(controller.state.status, "idle");
  assert.deepEqual(events.at(-1), ["disconnected", { reason: "session closed", initiator: "remote" }]);
  await waitFor(() => fake.calls.delete.length === 1, { label: "session delete" });
});

test("disconnect closes the session once, deletes it and reports an app disconnect", async () => {
  const { controller, fake, events } = createController();
  const pending = controller.connect();
  await waitFor(() => controller.state.status === "awaiting");
  fake.sessions[0].approve();
  await pending;
  await controller.disconnect("user");
  assert.equal(controller.state.status, "idle");
  assert.deepEqual(fake.sessions[0].closedWith, ["user"]);
  assert.equal(fake.calls.delete.length, 1);
  assert.deepEqual(events.at(-1), ["disconnected", { reason: "user", initiator: "app" }]);
  assert.equal(events.filter(([type]) => type === "disconnected").length, 1);
});

test("signing requires an approved session and delegates to the wallet", async () => {
  const { controller, fake } = createController();
  assert.throws(() => controller.signTransaction(Uint8Array.of(1)), { code: "not_connected" });
  assert.throws(() => controller.accountSigningKey(), { code: "not_connected" });
  await assert.rejects(controller.createCanonicalRequestAuth(), { code: "not_connected" });
  const pending = controller.connect();
  await waitFor(() => controller.state.status === "awaiting");
  fake.sessions[0].approve();
  await pending;
  assert.deepEqual(await controller.signTransaction(Uint8Array.of(1, 2)), Uint8Array.of(0xaa, 1, 2));
  assert.deepEqual(await controller.signRaw("iroha:test:v1", Uint8Array.of(3)), Uint8Array.of(0xbb, 3));
  assert.deepEqual(fake.sessions[0].signed[1], ["raw", "iroha:test:v1", Uint8Array.of(3)]);
  assert.deepEqual(await controller.accountSigningKey(), { publicKey: new Uint8Array(32).fill(5), generation: 1 });
  const auth = await controller.createCanonicalRequestAuth();
  assert.equal(auth.authAccountId, "approved");
  assert.equal(auth.signingKeyGeneration, 1, "canonical-request auth signs through the account signing key");
});

test("signing-key failures reach the app unchanged", async () => {
  const { controller, fake } = createController();
  const pending = controller.connect();
  await waitFor(() => controller.state.status === "awaiting");
  const missing = Object.assign(new Error("the approved account has no registered off-chain signing key"), {
    name: "ConnectSignRequestError",
    code: "SIGNING_KEY_REQUIRED",
  });
  fake.sessions[0].accountSigningKey = async () => {
    throw missing;
  };
  fake.sessions[0].approve();
  await pending;
  await assert.rejects(controller.accountSigningKey(), missing);
  await assert.rejects(controller.createCanonicalRequestAuth(), missing);
  assert.equal(controller.state.status, "connected", "a missing signing key does not end the session");
});

test("the default SDK binding is loaded lazily, once", async () => {
  const fake = createFakeSdk();
  let loads = 0;
  const controller = new IrohaConnectController({
    toriiUrl: TORII,
    networkId: NETWORK_ID,
    loadSdk: async () => {
      loads += 1;
      return fake.sdk;
    },
  });
  assert.equal(loads, 0);
  const pending = controller.connect();
  await waitFor(() => controller.state.status === "awaiting");
  await controller.cancel();
  await assert.rejects(pending, { code: "cancelled" });
  const again = controller.connect();
  await waitFor(() => controller.state.status === "awaiting");
  await controller.cancel();
  await assert.rejects(again, { code: "cancelled" });
  assert.equal(loads, 1);
});

test("incomplete SDK bindings are reported as configuration errors", async () => {
  const { sdk } = createFakeSdk();
  const { createAccountInspector: _omitted, ...incomplete } = sdk;
  assert.throws(
    () => new IrohaConnectController({ toriiUrl: TORII, networkId: NETWORK_ID, sdk: incomplete }),
    { code: "config", message: /createAccountInspector/u },
  );
  const controller = new IrohaConnectController({ toriiUrl: TORII, networkId: NETWORK_ID, loadSdk: async () => ({}) });
  await assert.rejects(controller.connect(), { code: "config" });
  assert.equal(controller.state.status, "error");
});

test("configuration is validated up front", () => {
  const sdk = createFakeSdk().sdk;
  const make = (options) => new IrohaConnectController({ toriiUrl: TORII, networkId: NETWORK_ID, sdk, ...options });
  assert.equal(make({ toriiUrl: "https://taira.sora.org/" }).config.toriiUrl, TORII);
  assert.throws(() => make({ toriiUrl: "" }), { code: "config", message: /torii-url is required/u });
  assert.throws(() => make({ toriiUrl: "taira.sora.org" }), { code: "config" });
  assert.throws(() => make({ toriiUrl: "https://taira.sora.org/v1" }), { code: "config", message: /without a path/u });
  assert.throws(() => make({ toriiUrl: "https://user:pass@taira.sora.org" }), { code: "config", message: /credentials/u });
  assert.throws(() => make({ toriiUrl: "http://127.0.0.1:8080" }), { code: "config", message: /allow-insecure/u });
  assert.equal(make({ toriiUrl: "http://127.0.0.1:8080", allowInsecure: true }).config.toriiUrl, "http://127.0.0.1:8080");
  assert.throws(() => make({ toriiUrl: "ftp://taira.sora.org" }), { code: "config" });
  assert.throws(() => make({ networkId: "  " }), { code: "config", message: /network-id is required/u });
  assert.throws(() => make({ launchProtocol: "javascript" }), { code: "config" });
  assert.throws(() => make({ launchProtocol: "https:" }), { code: "config" });
  assert.equal(make({ launchProtocol: "sorawallet:" }).config.launchProtocol, "sorawallet");
  assert.throws(() => make({ appMeta: { name: " " } }), { code: "config" });
  assert.throws(() => make({ permissions: "sign_raw" }), { code: "config" });
  assert.ok(Object.isFrozen(make({}).config));
});
