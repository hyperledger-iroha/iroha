// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * `<iroha-connect>`: the drop-in IrohaConnect button for SORA Nexus apps.
 *
 * One element gives an app the whole IrohaConnect experience: the branded
 * button, the pairing dialog with its QR code and same-device wallet link,
 * the approval moment, the connected-account chip, and the sakura layer that
 * every SORA Nexus app shares. Widgets with the same configuration share one
 * session, so a header button and an in-page call to action stay in sync and
 * a single-page app can re-mount the button without dropping the wallet.
 */

import { irohaMarkSvg } from "./brand.js";
import { IrohaConnectController, IrohaConnectError } from "./controller.js";
import { encodeQr, qrToSvg } from "./qr.js";
import { SakuraField, acquireSakura, defineIrohaSakura } from "./sakura.js";
import { STRINGS, formatString, languageFor } from "./strings.js";
import { CONNECT_STYLES } from "./styles.js";
import { normalizeThemePreference, watchTheme } from "./theme.js";

/** Element name of the IrohaConnect widget. */
export const IROHA_CONNECT_TAG = "iroha-connect";

/** DOM events dispatched by `<iroha-connect>` (they bubble and cross shadow roots). */
export const IROHA_CONNECT_EVENTS = Object.freeze({
  state: "iroha-connect-state",
  approved: "iroha-connect-approved",
  disconnected: "iroha-connect-disconnected",
  error: "iroha-connect-error",
});

const CONFIG_ATTRIBUTES = Object.freeze([
  "torii-url",
  "network-id",
  "app-name",
  "app-url",
  "launch-protocol",
  "allow-insecure",
]);
const APPEARANCE_ATTRIBUTES = Object.freeze(["theme", "sakura", "backdrop", "lang"]);
const IDLE_STATE = Object.freeze({ status: "idle" });
const CLOSE_AFTER_APPROVAL_MS = 1300;
const COPY_FEEDBACK_MS = 1600;
const DIALOG_SAKURA_DENSITY = 0.6;

const CLOSE_ICON =
  `<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">` +
  `<path d="M6 6l12 12M18 6L6 18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>`;
const CHECK_ICON =
  `<svg class="icon-check" viewBox="0 0 24 24" aria-hidden="true" focusable="false">` +
  `<path d="M5 12.5l4.2 4.2L19 7" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
const ERROR_ICON =
  `<svg class="icon-error" viewBox="0 0 24 24" aria-hidden="true" focusable="false">` +
  `<path d="M12 6.5v7.5" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round"/>` +
  `<circle cx="12" cy="18" r="1.6" fill="currentColor"/></svg>`;

const TEMPLATE = `<style>${CONNECT_STYLES}</style>
<div class="root theme-dark" part="root">
  <button class="trigger" part="trigger" type="button" aria-haspopup="dialog" aria-expanded="false">
    <span class="badge" aria-hidden="true">${irohaMarkSvg()}</span>
    <span class="trigger-label">IrohaConnect</span>
    <span class="trigger-account" hidden></span>
    <span class="dot" aria-hidden="true" hidden></span>
  </button>
  <dialog class="dialog" part="dialog" aria-labelledby="iroha-connect-title" aria-describedby="iroha-connect-lede" data-status="idle" data-view="pairing">
    <div class="sky" aria-hidden="true"></div>
    <canvas class="dialog-sakura" aria-hidden="true"></canvas>
    <div class="panel" part="panel">
      <div class="head">
        <span class="mark" aria-hidden="true">${irohaMarkSvg()}</span>
        <div class="heading">
          <h2 class="title" id="iroha-connect-title"></h2>
          <p class="meta"><span class="eyebrow"></span><span class="network"></span></p>
        </div>
        <button class="close" type="button">${CLOSE_ICON}</button>
      </div>
      <div class="stage">
        <div class="qr-frame" part="qr">
          <div class="qr" role="img"></div>
          <span class="spinner" aria-hidden="true"></span>
          <span class="result" aria-hidden="true">${CHECK_ICON}${ERROR_ICON}</span>
        </div>
      </div>
      <div class="body">
        <p class="lede" id="iroha-connect-lede"></p>
        <p class="status" role="status" aria-live="polite" data-tone="info"><span class="pulse" aria-hidden="true"></span><span class="status-text"></span></p>
        <div class="account-card">
          <span class="account-label"></span>
          <code class="account-id"></code>
        </div>
        <div class="actions">
          <a class="button primary" data-action="open" part="open-wallet"></a>
          <button class="button secondary" data-action="copy" type="button"></button>
          <button class="button primary" data-action="retry" type="button"></button>
          <button class="button secondary" data-action="cancel" type="button"></button>
          <button class="button secondary" data-action="copy-account" type="button"></button>
          <button class="button danger" data-action="disconnect" type="button"></button>
        </div>
      </div>
    </div>
  </dialog>
</div>`;

const sharedSessions = new Map();
const sdkIds = new WeakMap();
let nextSdkId = 1;

function identityOf(sdk) {
  if (!sdk) {
    return 0;
  }
  let id = sdkIds.get(sdk);
  if (!id) {
    id = nextSdkId;
    nextSdkId += 1;
    sdkIds.set(sdk, id);
  }
  return id;
}

function shortAccount(accountId) {
  const value = String(accountId ?? "");
  return value.length > 14 ? `${value.slice(0, 6)}…${value.slice(-4)}` : value;
}

function hostOf(url) {
  if (!url) {
    return "";
  }
  try {
    return new URL(url).host;
  } catch {
    return String(url);
  }
}

function asConfigError(error) {
  return error instanceof IrohaConnectError
    ? error
    : new IrohaConnectError("config", error instanceof Error ? error.message : String(error));
}

const HTMLElementBase = globalThis.HTMLElement ?? class {};

/**
 * The IrohaConnect widget. See the package README for attributes, events
 * and the signing API.
 */
export class IrohaConnectElement extends HTMLElementBase {
  static get observedAttributes() {
    return [...CONFIG_ATTRIBUTES, ...APPEARANCE_ATTRIBUTES];
  }

  #refs;
  #entry = null;
  #configError = null;
  #sdk = null;
  #permissions = null;
  #strings = null;
  #theme = "dark";
  #view = "pairing";
  #dialogOpen = false;
  #sakuraLease = null;
  #sakuraSignature = "";
  #dialogField = null;
  #stopThemeWatch = null;
  #closeTimer = 0;
  #copyTimers = new Map();
  #renderedUri = "";

  constructor() {
    super();
    const shadow = this.attachShadow({ mode: "open" });
    shadow.innerHTML = TEMPLATE;
    const find = (selector) => shadow.querySelector(selector);
    this.#refs = Object.freeze({
      root: find(".root"),
      trigger: find(".trigger"),
      triggerLabel: find(".trigger-label"),
      triggerAccount: find(".trigger-account"),
      dot: find(".dot"),
      dialog: find(".dialog"),
      dialogCanvas: find(".dialog-sakura"),
      close: find(".close"),
      eyebrow: find(".eyebrow"),
      title: find(".title"),
      lede: find(".lede"),
      qrFrame: find(".qr-frame"),
      qr: find(".qr"),
      status: find(".status"),
      statusText: find(".status-text"),
      open: find('[data-action="open"]'),
      copy: find('[data-action="copy"]'),
      retry: find('[data-action="retry"]'),
      cancel: find('[data-action="cancel"]'),
      accountLabel: find(".account-label"),
      accountId: find(".account-id"),
      copyAccount: find('[data-action="copy-account"]'),
      disconnect: find('[data-action="disconnect"]'),
      network: find(".network"),
    });
    const refs = this.#refs;
    refs.trigger.addEventListener("click", () => this.#onTriggerClick());
    refs.close.addEventListener("click", () => this.close());
    refs.cancel.addEventListener("click", () => this.close());
    refs.retry.addEventListener("click", () => {
      this.#startPairing().catch(() => undefined);
    });
    refs.copy.addEventListener("click", () => {
      void this.#copyText(refs.copy, this.state.walletUri);
    });
    refs.copyAccount.addEventListener("click", () => {
      void this.#copyText(refs.copyAccount, this.accountId);
    });
    refs.disconnect.addEventListener("click", () => {
      void this.disconnect();
    });
    refs.dialog.addEventListener("close", () => {
      // `close` is queued as a task: when the dialog was reopened before the
      // task ran, the event belongs to the previous opening and is ignored.
      if (!refs.dialog.open) {
        this.#onDialogClosed();
      }
    });
    let pressedBackdrop = false;
    refs.dialog.addEventListener("pointerdown", (event) => {
      pressedBackdrop = event.target === refs.dialog;
    });
    refs.dialog.addEventListener("click", (event) => {
      if (event.target === refs.dialog && pressedBackdrop) {
        this.close();
      }
      pressedBackdrop = false;
    });
  }

  connectedCallback() {
    this.#syncTheme();
    this.#syncSakura();
    this.#attach();
    this.#render();
  }

  disconnectedCallback() {
    // A move within the document reconnects before microtasks run.
    queueMicrotask(() => {
      if (!this.isConnected) {
        this.#teardown();
      }
    });
  }

  attributeChangedCallback(name, previous, next) {
    if (previous === next || !this.isConnected) {
      return;
    }
    if (CONFIG_ATTRIBUTES.includes(name)) {
      this.#attach();
    } else if (name === "theme") {
      this.#syncTheme();
      this.#syncSakura();
    } else if (name === "sakura" || name === "backdrop") {
      this.#syncSakura();
      if (this.#sakuraMode() === "off") {
        this.#stopDialogSakura();
      }
    }
    this.#render();
  }

  /** Current pairing state (`idle`, `preparing`, `awaiting`, `connected` or `error`). */
  get state() {
    return this.#entry?.controller.state ?? IDLE_STATE;
  }

  /** Approved account id while connected, otherwise null. */
  get accountId() {
    return this.#entry?.controller.accountId ?? null;
  }

  /** The shared headless controller, or null while the attributes are invalid. */
  get controller() {
    return this.#entry?.controller ?? null;
  }

  /** The `@iroha/iroha-js` Connect app session while connected (advanced use). */
  get appSession() {
    return this.#entry?.controller.appSession ?? null;
  }

  /** Replacement SDK bindings (tests and custom transports); null uses `@iroha/iroha-js`. */
  get sdk() {
    return this.#sdk;
  }

  set sdk(value) {
    this.#sdk = value ?? null;
    if (this.isConnected) {
      this.#attach();
      this.#render();
    }
  }

  /** Connect permissions requested from the wallet (`{ methods, events, resources }`). */
  get permissions() {
    return this.#permissions;
  }

  set permissions(value) {
    this.#permissions = value ?? null;
    if (this.isConnected) {
      this.#attach();
      this.#render();
    }
  }

  /** Copy overrides merged over the built-in English or Japanese strings. */
  get strings() {
    return this.#strings;
  }

  set strings(value) {
    this.#strings = value && typeof value === "object" ? Object.freeze({ ...value }) : null;
    this.#render();
  }

  /** Open the pairing dialog and resolve with the wallet approval. */
  connect() {
    if (this.state.status === "connected") {
      return Promise.resolve(this.state.approval);
    }
    this.#openDialog("pairing");
    return this.#startPairing();
  }

  /** Close the session (or abandon pairing) and close the dialog. */
  async disconnect() {
    const controller = this.#entry?.controller;
    this.close();
    await controller?.disconnect("disconnected by app");
  }

  /** Close the dialog; an unfinished pairing is cancelled. */
  close() {
    const dialog = this.#refs.dialog;
    if (dialog.open && typeof dialog.close === "function") {
      dialog.close();
    } else {
      dialog.removeAttribute("open");
    }
    this.#onDialogClosed();
  }

  /** Ask the connected wallet to sign an unsigned transaction payload. */
  signTransaction(unsignedTxBytes) {
    return this.#requireController().signTransaction(unsignedTxBytes);
  }

  /** Ask the connected wallet to sign raw bytes under a granted domain tag. */
  signRaw(domainTag, bytes) {
    return this.#requireController().signRaw(domainTag, bytes);
  }

  /** The connected account's off-chain signing key, which `signRaw` signatures verify against. */
  accountSigningKey() {
    return this.#requireController().accountSigningKey();
  }

  /** Torii canonical-request authentication backed by the connected wallet. */
  createCanonicalRequestAuth() {
    return this.#requireController().createCanonicalRequestAuth();
  }

  #requireController() {
    const controller = this.#entry?.controller;
    if (!controller) {
      throw this.#configError ?? new IrohaConnectError("not_connected", "IrohaConnect is not connected");
    }
    return controller;
  }

  #text() {
    const tag =
      this.getAttribute("lang") ||
      this.closest?.("[lang]")?.getAttribute("lang") ||
      this.ownerDocument?.documentElement?.lang ||
      globalThis.navigator?.language;
    return { ...STRINGS[languageFor(tag)], ...(this.#strings ?? {}) };
  }

  #appName() {
    const declared = this.getAttribute("app-name")?.trim();
    if (declared) {
      return declared;
    }
    const title = this.ownerDocument?.title?.trim();
    return title || globalThis.location?.host || "this app";
  }

  #appUrl() {
    const declared = this.getAttribute("app-url")?.trim();
    if (declared) {
      return declared;
    }
    const location = globalThis.location;
    return location && /^https?:$/u.test(location.protocol) ? location.origin : null;
  }

  #controllerOptions() {
    return {
      toriiUrl: this.getAttribute("torii-url") ?? "",
      networkId: this.getAttribute("network-id") ?? "",
      appMeta: { name: this.#appName(), url: this.#appUrl() },
      permissions: this.#permissions,
      launchProtocol: this.getAttribute("launch-protocol") ?? undefined,
      allowInsecure: this.hasAttribute("allow-insecure"),
      sdk: this.#sdk ?? undefined,
    };
  }

  #attach() {
    let candidate;
    try {
      candidate = new IrohaConnectController(this.#controllerOptions());
    } catch (error) {
      this.#detach();
      this.#configError = asConfigError(error);
      return;
    }
    this.#configError = null;
    const { config } = candidate;
    const key = JSON.stringify([
      config.toriiUrl,
      config.networkId,
      config.appMeta,
      config.permissions,
      config.launchProtocol,
      config.allowInsecure,
      identityOf(this.#sdk),
    ]);
    if (this.#entry?.key === key) {
      return;
    }
    const wasOpen = this.#dialogOpen;
    this.#detach();
    let entry = sharedSessions.get(key);
    if (!entry) {
      entry = { key, controller: candidate, attachments: 0 };
      sharedSessions.set(key, entry);
    }
    entry.attachments += 1;
    this.#entry = entry;
    const { controller } = entry;
    controller.addEventListener("statechange", this.#onState);
    controller.addEventListener("approved", this.#onApproved);
    controller.addEventListener("disconnected", this.#onDisconnected);
    controller.addEventListener("error", this.#onError);
    if (wasOpen) {
      this.close();
    }
  }

  #detach() {
    const entry = this.#entry;
    if (!entry) {
      return;
    }
    this.#entry = null;
    const { controller } = entry;
    controller.removeEventListener("statechange", this.#onState);
    controller.removeEventListener("approved", this.#onApproved);
    controller.removeEventListener("disconnected", this.#onDisconnected);
    controller.removeEventListener("error", this.#onError);
    entry.attachments -= 1;
    // Keep an approved session for the next widget with this configuration;
    // a pairing nobody can see any more is cancelled.
    if (entry.attachments <= 0 && controller.state.status !== "connected") {
      if (sharedSessions.get(entry.key) === entry) {
        sharedSessions.delete(entry.key);
      }
      void controller.cancel("widget removed");
    }
  }

  #teardown() {
    this.#stopThemeWatch?.();
    this.#stopThemeWatch = null;
    this.#releaseSakura();
    this.close();
    this.#detach();
    for (const timer of this.#copyTimers.values()) {
      clearTimeout(timer);
    }
    this.#copyTimers.clear();
  }

  #syncTheme() {
    this.#stopThemeWatch?.();
    this.#stopThemeWatch = watchTheme(
      normalizeThemePreference(this.getAttribute("theme")),
      (theme) => {
        this.#theme = theme;
        this.#dialogField?.setTheme(theme);
        this.#render();
      },
      this.ownerDocument,
    );
  }

  #sakuraMode() {
    const value = (this.getAttribute("sakura") ?? "").trim().toLowerCase();
    if (value === "off" || value === "none" || value === "false" || value === "0") {
      return "off";
    }
    return value === "overlay" ? "overlay" : "background";
  }

  #syncSakura() {
    const mode = this.#sakuraMode();
    const backdrop = this.hasAttribute("backdrop");
    const theme = normalizeThemePreference(this.getAttribute("theme"));
    const signature = `${mode}|${backdrop}|${theme}`;
    if (this.#sakuraLease && signature === this.#sakuraSignature) {
      return;
    }
    this.#releaseSakura();
    if (mode === "off") {
      return;
    }
    this.#sakuraLease = acquireSakura({ mode, backdrop, theme });
    this.#sakuraSignature = signature;
  }

  #releaseSakura() {
    this.#sakuraLease?.release();
    this.#sakuraLease = null;
    this.#sakuraSignature = "";
  }

  #startDialogSakura() {
    if (this.#dialogField || this.#sakuraMode() === "off") {
      return;
    }
    try {
      this.#dialogField = new SakuraField(this.#refs.dialogCanvas, {
        theme: this.#theme,
        density: DIALOG_SAKURA_DENSITY,
        parallaxVariables: null,
      }).start();
    } catch {
      this.#dialogField = null;
    }
  }

  #stopDialogSakura() {
    this.#dialogField?.destroy();
    this.#dialogField = null;
  }

  #onTriggerClick() {
    if (this.state.status === "connected") {
      this.#openDialog("account");
      return;
    }
    this.connect().catch(() => undefined);
  }

  #startPairing() {
    const controller = this.#entry?.controller;
    if (!controller) {
      this.#render();
      return Promise.reject(this.#configError ?? new IrohaConnectError("config", "IrohaConnect is not configured"));
    }
    return controller.connect();
  }

  #openDialog(view) {
    this.#view = view;
    clearTimeout(this.#closeTimer);
    this.#render();
    const dialog = this.#refs.dialog;
    if (this.#dialogOpen || !this.isConnected) {
      return;
    }
    this.#dialogOpen = true;
    if (typeof dialog.showModal === "function") {
      try {
        dialog.showModal();
      } catch {
        dialog.setAttribute("open", "");
      }
    } else {
      dialog.setAttribute("open", "");
    }
    this.#refs.trigger.setAttribute("aria-expanded", "true");
    this.#startDialogSakura();
  }

  #onDialogClosed() {
    if (!this.#dialogOpen) {
      return;
    }
    this.#dialogOpen = false;
    clearTimeout(this.#closeTimer);
    this.#stopDialogSakura();
    this.#refs.trigger.setAttribute("aria-expanded", "false");
    const status = this.state.status;
    if (this.#view === "pairing" && (status === "preparing" || status === "awaiting" || status === "error")) {
      void this.#entry?.controller.cancel("dismissed");
    }
    this.#view = "pairing";
    this.#render();
  }

  #onState = (event) => {
    this.#render();
    this.#emit(IROHA_CONNECT_EVENTS.state, event.detail);
  };

  #onApproved = (event) => {
    this.#emit(IROHA_CONNECT_EVENTS.approved, event.detail);
    if (!this.#dialogOpen || this.#view !== "pairing") {
      return;
    }
    this.#celebrate();
    clearTimeout(this.#closeTimer);
    this.#closeTimer = setTimeout(() => {
      if (this.state.status === "connected" && this.#view === "pairing") {
        this.close();
      }
    }, CLOSE_AFTER_APPROVAL_MS);
  };

  #onDisconnected = (event) => {
    this.#emit(IROHA_CONNECT_EVENTS.disconnected, event.detail);
    if (this.#dialogOpen && this.#view === "account") {
      this.close();
    }
  };

  #onError = (event) => {
    const error = event.detail;
    this.#emit(IROHA_CONNECT_EVENTS.error, { code: error?.code ?? "failed", message: error?.message ?? "", error });
  };

  #celebrate() {
    const rect = this.#refs.qrFrame.getBoundingClientRect?.();
    if (!rect) {
      return;
    }
    const x = rect.left + rect.width / 2;
    const y = rect.top + rect.height / 2;
    this.#dialogField?.burst({ x, y, count: 30 });
    this.#sakuraLease?.burst(x, y, 16);
  }

  #emit(type, detail) {
    this.dispatchEvent(new CustomEvent(type, { detail, bubbles: true, composed: true }));
  }

  async #copyText(button, text) {
    const clipboard = globalThis.navigator?.clipboard;
    if (!text || typeof clipboard?.writeText !== "function") {
      return;
    }
    try {
      await clipboard.writeText(text);
    } catch {
      return;
    }
    button.dataset.copied = "";
    button.textContent = this.#text().copied;
    clearTimeout(this.#copyTimers.get(button));
    this.#copyTimers.set(
      button,
      setTimeout(() => {
        delete button.dataset.copied;
        this.#copyTimers.delete(button);
        this.#render();
      }, COPY_FEEDBACK_MS),
    );
  }

  #renderQr(uri) {
    if (uri === undefined || uri === this.#renderedUri) {
      return;
    }
    this.#renderedUri = uri;
    if (!uri) {
      this.#refs.qr.replaceChildren();
      return;
    }
    try {
      this.#refs.qr.innerHTML = qrToSvg(encodeQr(uri, { level: "M" }));
    } catch {
      this.#refs.qr.replaceChildren();
    }
  }

  #lede(status, text) {
    if (this.#view === "account") {
      return text.ledeAccount;
    }
    switch (status) {
      case "config":
        return text.ledeConfig;
      case "awaiting":
        return text.ledeAwaiting;
      case "connected":
        return text.ledeConnected;
      case "error":
        return text.ledeError;
      default:
        return text.ledePreparing;
    }
  }

  #statusLine(status, state, text) {
    switch (status) {
      case "config":
        return formatString(text.errorConfig, { message: this.#configError?.message ?? "" });
      case "awaiting":
        return text.statusAwaiting;
      case "connected":
        return text.statusConnected;
      case "error": {
        const key = {
          rejected: "errorRejected",
          closed: "errorClosed",
          network: "errorNetwork",
          config: "errorConfig",
        }[state.code] ?? "errorFailed";
        return formatString(text[key], { message: state.message });
      }
      default:
        return text.statusPreparing;
    }
  }

  #render() {
    const refs = this.#refs;
    const text = this.#text();
    const state = this.state;
    const status = !this.#entry && this.#configError ? "config" : state.status;
    const connected = status === "connected";

    refs.root.className = `root theme-${this.#theme}`;
    refs.trigger.toggleAttribute("data-connected", connected);
    refs.triggerLabel.textContent = text.trigger;
    refs.triggerLabel.hidden = connected;
    refs.triggerAccount.hidden = !connected;
    refs.triggerAccount.textContent = connected ? shortAccount(state.accountId) : "";
    refs.dot.hidden = !connected;
    refs.trigger.setAttribute(
      "aria-label",
      connected ? formatString(text.connectedLabel, { account: state.accountId }) : text.triggerLabel,
    );

    refs.dialog.dataset.status = status;
    refs.dialog.dataset.view = this.#view;
    refs.close.setAttribute("aria-label", text.close);
    refs.eyebrow.textContent = text.eyebrow;
    refs.title.textContent =
      this.#view === "account" ? text.accountTitle : formatString(text.title, { app: this.#appName() });
    refs.lede.textContent = this.#lede(status, text);

    refs.status.dataset.tone = status === "error" || status === "config" ? "error" : connected ? "success" : "info";
    refs.statusText.textContent = this.#statusLine(status, state, text);
    refs.qr.setAttribute("aria-label", text.qrLabel);
    this.#renderQr(status === "awaiting" ? state.walletUri ?? "" : connected ? undefined : "");

    refs.open.textContent = text.openWallet;
    if (status === "awaiting" && state.launchUri) {
      refs.open.setAttribute("href", state.launchUri);
    } else {
      refs.open.removeAttribute("href");
    }
    if (!("copied" in refs.copy.dataset)) {
      refs.copy.textContent = text.copyLink;
    }
    refs.retry.textContent = text.retry;
    refs.cancel.textContent = text.cancel;

    refs.accountLabel.textContent = text.account;
    refs.accountId.textContent = connected ? state.accountId : "";
    if (!("copied" in refs.copyAccount.dataset)) {
      refs.copyAccount.textContent = text.copyAccount;
    }
    refs.disconnect.textContent = text.disconnect;

    const host = hostOf(this.#entry?.controller.config.toriiUrl ?? this.getAttribute("torii-url"));
    refs.network.textContent = host;
    refs.network.title = host ? `${text.network}: ${host}` : "";
    refs.network.hidden = !host;
  }
}

/** Register `<iroha-connect>` and `<iroha-sakura>` once; returns false outside a browser. */
export function defineIrohaConnect(registry = globalThis.customElements) {
  if (!registry || typeof registry.define !== "function") {
    return false;
  }
  defineIrohaSakura(registry);
  if (!registry.get(IROHA_CONNECT_TAG)) {
    registry.define(IROHA_CONNECT_TAG, IrohaConnectElement);
  }
  return true;
}
