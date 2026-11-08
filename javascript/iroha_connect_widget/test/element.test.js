// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import test from "node:test";

import { installDom, settle, waitFor } from "./helpers/dom.js";
import { createFakeSdk, walletRejection } from "./helpers/fakeSdk.js";

const window = installDom();
const { document } = window;
const { IROHA_CONNECT_EVENTS, IrohaConnectElement, encodeQr, qrSvgPath } = await import("../src/index.js");

const TORII = "https://taira.sora.org";
const NETWORK_ID = "hash:A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5#95D7";

function mount(attributes = {}, { sdk = null, parent = document.body } = {}) {
  const element = document.createElement("iroha-connect");
  const merged = { "torii-url": TORII, "network-id": NETWORK_ID, ...attributes };
  for (const [name, value] of Object.entries(merged)) {
    if (value !== null) {
      element.setAttribute(name, value);
    }
  }
  if (sdk) {
    element.sdk = sdk;
  }
  parent.append(element);
  return element;
}

function ui(element) {
  const root = element.shadowRoot;
  const find = (selector) => root.querySelector(selector);
  return {
    root: find(".root"),
    trigger: find(".trigger"),
    label: find(".trigger-label"),
    account: find(".trigger-account"),
    dialog: find(".dialog"),
    title: find(".title"),
    lede: find(".lede"),
    status: find(".status"),
    statusText: find(".status-text"),
    qr: find(".qr"),
    open: find('[data-action="open"]'),
    copy: find('[data-action="copy"]'),
    retry: find('[data-action="retry"]'),
    close: find(".close"),
    accountId: find(".account-id"),
    disconnect: find('[data-action="disconnect"]'),
    network: find(".network"),
  };
}

function isOpen(dialog) {
  return dialog.hasAttribute("open");
}

function record(target) {
  const events = [];
  for (const type of Object.values(IROHA_CONNECT_EVENTS)) {
    target.addEventListener(type, (event) => events.push([type, event.detail]));
  }
  return events;
}

async function removeAll() {
  for (const element of document.querySelectorAll("iroha-connect")) {
    element.remove();
  }
  await settle();
}

test("the element registers, renders the branded trigger and adds the shared sakura layer", async () => {
  const element = mount({}, { sdk: createFakeSdk().sdk });
  assert.ok(element instanceof IrohaConnectElement);
  const { trigger, label, root } = ui(element);
  assert.equal(label.textContent, "IrohaConnect");
  assert.equal(trigger.getAttribute("aria-label"), "Connect a wallet with IrohaConnect");
  assert.equal(trigger.getAttribute("aria-haspopup"), "dialog");
  assert.equal(trigger.getAttribute("aria-expanded"), "false");
  assert.ok(trigger.querySelector(".badge svg path[fill='#e4232d']"), "the Iroha flower mark");
  assert.match(root.className, /theme-(dark|light)/u);
  const layer = document.body.firstElementChild;
  assert.equal(layer.localName, "iroha-sakura");
  assert.ok(layer.hasAttribute("data-iroha-auto"));
  await removeAll();
  assert.equal(document.querySelector("iroha-sakura"), null);
});

test("the sakura attribute controls the shared layer", async () => {
  const element = mount({ sakura: "off" }, { sdk: createFakeSdk().sdk });
  assert.equal(document.querySelector("iroha-sakura"), null);
  element.setAttribute("sakura", "overlay");
  assert.equal(document.querySelector("iroha-sakura")?.getAttribute("mode"), "overlay");
  element.setAttribute("backdrop", "");
  const layer = document.querySelector("iroha-sakura");
  assert.ok(layer.hasAttribute("backdrop"));
  assert.equal(document.querySelectorAll("iroha-sakura").length, 1);
  element.setAttribute("sakura", "none");
  assert.equal(document.querySelector("iroha-sakura"), null);
  await removeAll();
});

test("pairing shows the QR code and same-device link, then approval shows the account", async () => {
  const fake = createFakeSdk();
  const element = mount({ "app-name": "Hanami Market" }, { sdk: fake.sdk });
  const events = record(element);
  const bubbled = record(document);
  const parts = ui(element);

  parts.trigger.click();
  assert.ok(isOpen(parts.dialog));
  assert.equal(parts.trigger.getAttribute("aria-expanded"), "true");
  await waitFor(() => element.state.status === "awaiting", { label: "awaiting" });

  const walletUri = "iroha://connect?sid=sid-1&role=wallet&token=wallet-1&relay=relay-1";
  assert.equal(parts.dialog.dataset.status, "awaiting");
  assert.equal(parts.title.textContent, "Connect to Hanami Market");
  assert.equal(parts.statusText.textContent, "Waiting for wallet approval");
  assert.equal(parts.qr.getAttribute("role"), "img");
  assert.equal(parts.qr.getAttribute("aria-label"), "IrohaConnect pairing code");
  assert.equal(parts.qr.querySelector("path").getAttribute("d"), qrSvgPath(encodeQr(walletUri, { level: "M" })));
  assert.equal(parts.open.getAttribute("href"), walletUri.replace("iroha:", "irohaconnect:"));
  assert.equal(parts.network.textContent, "taira.sora.org");
  assert.equal(parts.network.title, "Network: taira.sora.org", "a truncated host stays readable on hover");
  assert.deepEqual(fake.calls.appSession[0].appMeta, {
    name: "Hanami Market",
    url: "https://app.example",
    iconHash: null,
  });

  fake.sessions[0].approve("sorau7xHanamiAccount000000000000000000000001");
  await waitFor(() => element.state.status === "connected", { label: "connected" });
  assert.equal(parts.dialog.dataset.status, "connected");
  assert.equal(parts.status.dataset.tone, "success");
  assert.ok(parts.trigger.hasAttribute("data-connected"));
  assert.equal(parts.label.hidden, true);
  assert.equal(parts.account.textContent, "sorau7…0001");
  assert.match(parts.trigger.getAttribute("aria-label"), /connected as sorau7xHanamiAccount0+1$/u);
  const approved = events.find(([type]) => type === "iroha-connect-approved");
  assert.equal(approved[1].accountId, "sorau7xHanamiAccount000000000000000000000001");
  assert.ok(bubbled.some(([type]) => type === "iroha-connect-approved"), "events bubble out of the shadow root");
  assert.equal(element.accountId, "sorau7xHanamiAccount000000000000000000000001");
  assert.deepEqual(await element.signTransaction(Uint8Array.of(5)), Uint8Array.of(0xaa, 5));
  assert.equal((await element.accountSigningKey()).generation, 1);

  await waitFor(() => !isOpen(parts.dialog), { timeout: 3000, label: "auto close" });
  assert.equal(parts.trigger.getAttribute("aria-expanded"), "false");
  assert.equal(element.state.status, "connected", "closing after approval keeps the session");

  parts.trigger.click();
  assert.equal(parts.dialog.dataset.view, "account");
  assert.equal(parts.accountId.textContent, "sorau7xHanamiAccount000000000000000000000001");
  assert.equal(parts.title.textContent, "Wallet connected");
  parts.disconnect.click();
  await waitFor(() => element.state.status === "idle", { label: "disconnect" });
  assert.ok(!isOpen(parts.dialog));
  assert.ok(!parts.trigger.hasAttribute("data-connected"));
  assert.deepEqual(events.filter(([type]) => type === "iroha-connect-disconnected").map(([, detail]) => detail.initiator), ["app"]);
  assert.equal(fake.calls.delete.length, 1);
  await removeAll();
});

test("closing the dialog mid-pairing cancels and deletes the session", async () => {
  const fake = createFakeSdk();
  const element = mount({}, { sdk: fake.sdk });
  const parts = ui(element);
  parts.trigger.click();
  await waitFor(() => element.state.status === "awaiting");
  parts.close.click();
  assert.ok(!isOpen(parts.dialog));
  await waitFor(() => fake.calls.delete.length === 1, { label: "delete" });
  assert.equal(element.state.status, "idle");
  assert.deepEqual(fake.sessions[0].closedWith, ["dismissed"]);

  parts.trigger.click();
  await waitFor(() => element.state.status === "awaiting");
  parts.dialog.dispatchEvent(new window.PointerEvent("pointerdown", { bubbles: true }));
  parts.dialog.click();
  assert.ok(!isOpen(parts.dialog), "a click on the backdrop closes the dialog");
  await waitFor(() => fake.calls.delete.length === 2, { label: "second delete" });
  await removeAll();
});

test("a close event left over from the previous opening does not cancel a new pairing", async () => {
  const fake = createFakeSdk();
  const element = mount({}, { sdk: fake.sdk });
  const parts = ui(element);
  element.connect().catch(() => undefined);
  await waitFor(() => element.state.status === "awaiting");
  element.close();
  parts.trigger.click();
  // Browsers queue the first close's `close` event; it lands after the reopen.
  parts.dialog.dispatchEvent(new window.Event("close"));
  await waitFor(() => element.state.status === "awaiting" && element.state.sid === "sid-2", { label: "second pairing" });
  assert.ok(isOpen(parts.dialog));
  assert.equal(parts.dialog.dataset.status, "awaiting");
  assert.deepEqual(fake.sessions[1].closedWith, []);
  element.close();
  await removeAll();
});

test("a declined connection explains itself and offers a new code", async () => {
  const fake = createFakeSdk();
  const element = mount({}, { sdk: fake.sdk });
  const errors = [];
  element.addEventListener("iroha-connect-error", (event) => errors.push(event.detail));
  const parts = ui(element);
  const pending = element.connect();
  await waitFor(() => element.state.status === "awaiting");
  fake.sessions[0].reject(walletRejection());
  await assert.rejects(pending, { code: "rejected" });
  assert.equal(parts.dialog.dataset.status, "error");
  assert.equal(parts.status.dataset.tone, "error");
  assert.equal(parts.statusText.textContent, "The wallet declined the connection.");
  assert.equal(errors[0].code, "rejected");
  assert.equal(parts.qr.childElementCount, 0, "a dead pairing code is removed");
  parts.retry.click();
  await waitFor(() => element.state.status === "awaiting" && element.state.sid === "sid-2", { label: "retry" });
  element.close();
  await waitFor(() => element.state.status === "idle");
  await removeAll();
});

test("a misconfigured widget renders guidance instead of throwing", async () => {
  const element = mount({ "torii-url": null, "network-id": null });
  const parts = ui(element);
  assert.equal(element.controller, null);
  await assert.rejects(element.connect(), { code: "config", message: /torii-url is required/u });
  assert.equal(parts.dialog.dataset.status, "config");
  assert.equal(parts.statusText.textContent, "torii-url is required");
  assert.equal(parts.lede.textContent, "This IrohaConnect button is not configured.");
  element.close();
  element.setAttribute("torii-url", "https://taira.sora.org/v1");
  assert.equal(element.controller, null);
  element.setAttribute("torii-url", TORII);
  element.setAttribute("network-id", NETWORK_ID);
  assert.ok(element.controller);
  assert.equal(element.controller.config.toriiUrl, TORII);
  await removeAll();
});

test("copy follows the page language and can be overridden", async () => {
  const fake = createFakeSdk();
  const element = mount({ lang: "ja", "app-name": "花見" }, { sdk: fake.sdk });
  const parts = ui(element);
  assert.equal(parts.trigger.getAttribute("aria-label"), "IrohaConnect でウォレットを接続");
  parts.trigger.click();
  assert.equal(parts.title.textContent, "花見 に接続");
  await waitFor(() => element.state.status === "awaiting");
  assert.equal(parts.statusText.textContent, "ウォレットの承認を待っています");
  element.strings = { statusAwaiting: "Scan me" };
  assert.equal(parts.statusText.textContent, "Scan me");

  const copied = [];
  Object.defineProperty(window.navigator, "clipboard", {
    configurable: true,
    value: { writeText: async (text) => copied.push(text) },
  });
  parts.copy.click();
  await waitFor(() => copied.length === 1);
  assert.equal(copied[0], element.state.walletUri);
  await waitFor(() => parts.copy.textContent === "コピーしました");
  element.close();
  await removeAll();
});

test("auto theme follows the host page", async () => {
  const element = mount({}, { sdk: createFakeSdk().sdk });
  const { root } = ui(element);
  document.documentElement.setAttribute("data-theme", "light");
  await waitFor(() => root.className.includes("theme-light"), { label: "light" });
  document.documentElement.setAttribute("data-theme", "dark");
  await waitFor(() => root.className.includes("theme-dark"), { label: "dark" });
  element.setAttribute("theme", "light");
  assert.ok(root.className.includes("theme-light"), "an explicit theme wins");
  document.documentElement.removeAttribute("data-theme");
  await removeAll();
});

test("widgets with one configuration share a session that survives re-mounting", async () => {
  const fake = createFakeSdk();
  const header = mount({ "app-name": "Shared" }, { sdk: fake.sdk });
  const cta = mount({ "app-name": "Shared" }, { sdk: fake.sdk });
  assert.equal(header.controller, cta.controller);
  const other = mount({ "app-name": "Other" }, { sdk: fake.sdk });
  assert.notEqual(other.controller, header.controller);

  ui(header).trigger.click();
  await waitFor(() => header.state.status === "awaiting");
  fake.sessions[0].approve();
  await waitFor(() => cta.state.status === "connected");
  assert.ok(ui(cta).trigger.hasAttribute("data-connected"));
  header.close();

  await removeAll();
  const remounted = mount({ "app-name": "Shared" }, { sdk: fake.sdk });
  assert.equal(remounted.state.status, "connected", "an approved session outlives a route change");
  await remounted.disconnect();
  assert.equal(remounted.state.status, "idle");
  await removeAll();
});

test("moving a widget keeps its pairing; removing it cancels the pairing", async () => {
  const fake = createFakeSdk();
  const element = mount({}, { sdk: fake.sdk });
  const sidebar = document.createElement("aside");
  document.body.append(sidebar);
  element.connect().catch(() => undefined);
  await waitFor(() => element.state.status === "awaiting");
  sidebar.append(element);
  await settle();
  assert.equal(element.state.status, "awaiting");
  assert.equal(fake.calls.delete.length, 0);
  element.remove();
  await waitFor(() => fake.calls.delete.length === 1, { label: "cancel on removal" });
  assert.equal(fake.sessions[0].closedWith.length, 1);
  sidebar.remove();
  await removeAll();
});
