// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Headless app-side IrohaConnect pairing.
 *
 * The controller walks the current `@iroha/iroha-js/connect-browser` flow:
 * derive a session preview bound to the app's pinned NetworkId, register it
 * with Torii, expose the wallet URI for the QR code and deep link, open the
 * encrypted relay socket, and wait for the wallet's signed approval. Wallet
 * approval is verified by the SDK against Torii's native account inspection;
 * private keys never enter the app. Cancelling or disconnecting closes the
 * socket and deletes the Torii session with its management token.
 */

/** Pairing states, in the order a successful connection visits them. */
export const CONNECT_STATUSES = Object.freeze(["idle", "preparing", "awaiting", "connected", "error"]);

/** Launch scheme of IrohaConnect wallets for same-device deep links. */
export const DEFAULT_LAUNCH_PROTOCOL = "irohaconnect";

/** Error raised by the controller; `code` is stable for UI decisions. */
export class IrohaConnectError extends Error {
  /**
   * @param {"config" | "cancelled" | "rejected" | "closed" | "network" | "not_connected" | "failed"} code
   * @param {string} message
   * @param {{ cause?: unknown }} [options]
   */
  constructor(code, message, options = {}) {
    super(message, options.cause === undefined ? undefined : { cause: options.cause });
    this.name = "IrohaConnectError";
    this.code = code;
  }
}

const IDLE_STATE = Object.freeze({ status: "idle" });

function normalizeToriiUrl(value, allowInsecure) {
  if (typeof value !== "string" || !value.trim()) {
    throw new IrohaConnectError("config", "torii-url is required");
  }
  let url;
  try {
    url = new URL(value.trim());
  } catch {
    throw new IrohaConnectError("config", "torii-url must be an absolute http(s) URL");
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") {
    throw new IrohaConnectError("config", "torii-url must use https");
  }
  if (url.username || url.password) {
    throw new IrohaConnectError("config", "torii-url must not contain credentials");
  }
  if (url.pathname !== "/" || url.search || url.hash) {
    throw new IrohaConnectError("config", "torii-url must be the Torii origin without a path");
  }
  if (url.protocol === "http:" && !allowInsecure) {
    throw new IrohaConnectError(
      "config",
      "torii-url must use https; allow-insecure is for local development only",
    );
  }
  return url.origin;
}

function normalizeNetworkId(value) {
  if (typeof value !== "string" || !value.trim()) {
    throw new IrohaConnectError("config", "network-id is required (the deployed network's exact NetworkId)");
  }
  return value.trim();
}

function normalizeLaunchProtocol(value) {
  const protocol = (value ?? DEFAULT_LAUNCH_PROTOCOL).trim().replace(/:$/u, "");
  if (!/^[a-z][a-z0-9+.-]*$/iu.test(protocol) || /^(https?|javascript|data|file|vbscript)$/iu.test(protocol)) {
    throw new IrohaConnectError("config", "launch-protocol must be a custom wallet URI scheme");
  }
  return protocol;
}

function normalizeAppMeta(appMeta) {
  if (appMeta === undefined || appMeta === null) {
    return null;
  }
  const name = typeof appMeta.name === "string" ? appMeta.name.trim() : "";
  if (!name) {
    throw new IrohaConnectError("config", "appMeta.name must be a non-empty string");
  }
  const url = typeof appMeta.url === "string" && appMeta.url.trim() ? appMeta.url.trim() : null;
  const iconHash = typeof appMeta.iconHash === "string" && appMeta.iconHash.trim()
    ? appMeta.iconHash.trim()
    : null;
  return Object.freeze({ name, url, iconHash });
}

function freezePermissions(permissions) {
  if (permissions === undefined || permissions === null) {
    return null;
  }
  if (typeof permissions !== "object") {
    throw new IrohaConnectError("config", "permissions must be an object");
  }
  const copy = (list) => (list === undefined || list === null ? list : Object.freeze([...list]));
  return Object.freeze({
    methods: copy(permissions.methods),
    events: copy(permissions.events),
    resources: copy(permissions.resources),
  });
}

function requireSdk(sdk) {
  const required = [
    "createConnectSessionPreview",
    "registerConnectSession",
    "createConnectAppSession",
    "deleteConnectSession",
    "resolveConnectLaunchUri",
    "resolveConnectLaunchUriForProtocol",
    "createAccountInspector",
  ];
  if (!sdk || typeof sdk !== "object") {
    throw new IrohaConnectError("config", "IrohaConnect SDK bindings are missing");
  }
  for (const name of required) {
    if (typeof sdk[name] !== "function") {
      throw new IrohaConnectError("config", `IrohaConnect SDK binding ${name} is missing`);
    }
  }
  if (!sdk.NetworkId || typeof sdk.NetworkId.parse !== "function") {
    throw new IrohaConnectError("config", "IrohaConnect SDK binding NetworkId is missing");
  }
  return sdk;
}

function classifyError(error) {
  if (error instanceof IrohaConnectError) {
    return error;
  }
  const name = error?.name;
  const message = error instanceof Error && error.message ? error.message : String(error);
  if (name === "ConnectApprovalRejectedError") {
    return new IrohaConnectError("rejected", message, { cause: error });
  }
  if (name === "ConnectSessionClosedError") {
    return new IrohaConnectError("closed", message, { cause: error });
  }
  if (error instanceof TypeError && /fetch|network|load failed/iu.test(message)) {
    return new IrohaConnectError("network", message, { cause: error });
  }
  return new IrohaConnectError("failed", message, { cause: error });
}

const defaultLoadSdk = () => import("./sdk.js").then((module) => module.default);

/**
 * Event target that pairs an app with an IrohaConnect wallet.
 *
 * Events: `statechange` (detail: state), `approved` (detail: approval),
 * `disconnected` (detail: `{ reason, initiator }`), `error` (detail: error).
 */
export class IrohaConnectController extends EventTarget {
  #config;
  #sdk;
  #loadSdk;
  #state = IDLE_STATE;
  #generation = 0;
  #pending = null;
  #active = null;

  /**
   * @param {{
   *   toriiUrl: string,
   *   networkId: string,
   *   appMeta?: { name: string, url?: string | null, iconHash?: string | null } | null,
   *   permissions?: { methods?: string[], events?: string[], resources?: string[] | null } | null,
   *   launchProtocol?: string,
   *   allowInsecure?: boolean,
   *   sdk?: object,
   *   loadSdk?: () => Promise<object>,
   * }} options
   */
  constructor(options) {
    super();
    if (!options || typeof options !== "object") {
      throw new IrohaConnectError("config", "IrohaConnect options are required");
    }
    const allowInsecure = options.allowInsecure === true;
    this.#config = Object.freeze({
      toriiUrl: normalizeToriiUrl(options.toriiUrl, allowInsecure),
      networkId: normalizeNetworkId(options.networkId),
      appMeta: normalizeAppMeta(options.appMeta),
      permissions: freezePermissions(options.permissions),
      launchProtocol: normalizeLaunchProtocol(options.launchProtocol),
      allowInsecure,
    });
    this.#sdk = options.sdk ? requireSdk(options.sdk) : null;
    this.#loadSdk = typeof options.loadSdk === "function" ? options.loadSdk : defaultLoadSdk;
  }

  /** Normalized, frozen configuration. */
  get config() {
    return this.#config;
  }

  /** Current state snapshot (frozen). */
  get state() {
    return this.#state;
  }

  /** Approved account id while connected, otherwise null. */
  get accountId() {
    return this.#state.status === "connected" ? this.#state.accountId : null;
  }

  /** The SDK app session while connected (advanced use), otherwise null. */
  get appSession() {
    return this.#state.status === "connected" ? this.#active?.appSession ?? null : null;
  }

  /**
   * Register a session and wait for wallet approval. Resolves with the
   * approval; rejects with `IrohaConnectError` (code `cancelled` when
   * `cancel()` interrupts it).
   */
  connect() {
    if (this.#state.status === "connected") {
      return Promise.resolve(this.#state.approval);
    }
    if (this.#pending) {
      return this.#pending;
    }
    const generation = ++this.#generation;
    this.#setState({ status: "preparing" });
    const pending = this.#runConnect(generation);
    this.#pending = pending;
    pending.then(
      () => this.#clearPending(pending),
      () => this.#clearPending(pending),
    );
    return pending;
  }

  /** Abandon a pairing in progress (or disconnect a connected session). */
  async cancel(reason = "cancelled") {
    if (this.#state.status === "connected") {
      await this.disconnect(reason);
      return;
    }
    this.#generation += 1;
    this.#pending = null;
    const active = this.#active;
    this.#active = null;
    if (this.#state.status !== "idle") {
      this.#setState(IDLE_STATE);
    }
    await this.#closeActive(active, reason);
  }

  /** Close an approved session and delete it on Torii. */
  async disconnect(reason = "disconnected") {
    if (this.#state.status !== "connected") {
      await this.cancel(reason);
      return;
    }
    this.#generation += 1;
    const active = this.#active;
    this.#active = null;
    this.#setState(IDLE_STATE);
    this.#emit("disconnected", Object.freeze({ reason, initiator: "app" }));
    await this.#closeActive(active, reason);
  }

  /** Ask the wallet to sign an unsigned transaction payload. */
  signTransaction(unsignedTxBytes) {
    return this.#requireSession().signTransaction(unsignedTxBytes);
  }

  /**
   * Ask the wallet to sign raw bytes under a granted domain tag. The signature
   * is by the account's off-chain signing key over the off-chain envelope.
   */
  signRaw(domainTag, bytes) {
    return this.#requireSession().signRaw(domainTag, bytes);
  }

  /** The approved account's off-chain signing key, which `signRaw` signatures verify against. */
  accountSigningKey() {
    return this.#requireSession().accountSigningKey();
  }

  /** Torii canonical-request authentication backed by the connected wallet. */
  async createCanonicalRequestAuth() {
    const session = this.#requireSession();
    const sdk = this.#active.sdk;
    if (typeof sdk.createConnectCanonicalRequestAuth !== "function") {
      throw new IrohaConnectError("config", "IrohaConnect SDK binding createConnectCanonicalRequestAuth is missing");
    }
    return sdk.createConnectCanonicalRequestAuth(session);
  }

  #requireSession() {
    const session = this.appSession;
    if (!session) {
      throw new IrohaConnectError("not_connected", "IrohaConnect is not connected");
    }
    return session;
  }

  #clearPending(pending) {
    if (this.#pending === pending) {
      this.#pending = null;
    }
  }

  async #resolveSdk() {
    if (!this.#sdk) {
      this.#sdk = requireSdk(await this.#loadSdk());
    }
    return this.#sdk;
  }

  async #runConnect(generation) {
    const config = this.#config;
    let active = null;
    try {
      const sdk = await this.#resolveSdk();
      this.#assertCurrent(generation);
      const networkId = sdk.NetworkId.parse(config.networkId);
      const preview = sdk.createConnectSessionPreview({ networkId, node: config.toriiUrl });
      const registration = await sdk.registerConnectSession(config.toriiUrl, preview, {
        node: config.toriiUrl,
      });
      active = { sdk, registration, appSession: null, detach: null, closed: false };
      this.#assertCurrent(generation, active);
      const walletUri = sdk.resolveConnectLaunchUri("wallet", preview, registration);
      const launchUri = sdk.resolveConnectLaunchUriForProtocol(
        "wallet",
        preview,
        registration,
        config.launchProtocol,
      );
      const appSession = sdk.createConnectAppSession({
        baseUrl: config.toriiUrl,
        preview,
        session: registration,
        inspectAccountIdentity: sdk.createAccountInspector({ baseUrl: config.toriiUrl, networkId }),
        appMeta: config.appMeta,
        permissions: config.permissions,
        allowInsecure: config.allowInsecure,
      });
      active.appSession = appSession;
      this.#active = active;
      this.#setState({
        status: "awaiting",
        walletUri,
        launchUri,
        sid: registration.sid,
        toriiUrl: config.toriiUrl,
      });
      const approval = await appSession.waitForApproval();
      this.#assertCurrent(generation, active);
      const snapshot = Object.freeze({
        accountId: approval.accountId,
        signingPublicKey: Uint8Array.from(approval.signingPublicKey),
        walletPublicKey: Uint8Array.from(approval.walletPublicKey),
        sid: registration.sid,
        toriiUrl: config.toriiUrl,
      });
      active.detach = this.#watchSocket(appSession.socket, generation);
      this.#setState({
        status: "connected",
        accountId: snapshot.accountId,
        sid: registration.sid,
        toriiUrl: config.toriiUrl,
        approval: snapshot,
      });
      this.#emit("approved", snapshot);
      return snapshot;
    } catch (cause) {
      if (generation !== this.#generation) {
        await this.#closeActive(active, "cancelled");
        throw new IrohaConnectError("cancelled", "IrohaConnect pairing was cancelled", { cause });
      }
      const error = classifyError(cause);
      if (this.#active === active) {
        this.#active = null;
      }
      this.#generation += 1;
      this.#setState({ status: "error", code: error.code, message: error.message, toriiUrl: config.toriiUrl });
      this.#emit("error", error);
      await this.#closeActive(active, "failed");
      throw error;
    }
  }

  #assertCurrent(generation, active = null) {
    if (generation !== this.#generation) {
      throw new IrohaConnectError("cancelled", "IrohaConnect pairing was cancelled");
    }
    if (active && this.#active && this.#active !== active) {
      throw new IrohaConnectError("cancelled", "IrohaConnect pairing was replaced");
    }
  }

  #watchSocket(socket, generation) {
    if (!socket || typeof socket.addEventListener !== "function") {
      return null;
    }
    const onClose = () => {
      if (generation !== this.#generation || this.#state.status !== "connected") {
        return;
      }
      this.#generation += 1;
      const active = this.#active;
      this.#active = null;
      this.#setState(IDLE_STATE);
      this.#emit("disconnected", Object.freeze({ reason: "session closed", initiator: "remote" }));
      void this.#closeActive(active, "session closed");
    };
    socket.addEventListener("close", onClose);
    socket.addEventListener("error", onClose);
    return () => {
      socket.removeEventListener?.("close", onClose);
      socket.removeEventListener?.("error", onClose);
    };
  }

  async #closeActive(active, reason) {
    if (!active || active.closed) {
      return;
    }
    active.closed = true;
    active.detach?.();
    try {
      active.appSession?.close(reason);
    } catch {
      // Closing is best-effort; the relay also expires abandoned sessions.
    }
    const { registration, sdk } = active;
    if (!registration?.sid || !registration.token_management) {
      return;
    }
    try {
      await sdk.deleteConnectSession(this.#config.toriiUrl, registration.sid, {
        tokenManagement: registration.token_management,
      });
    } catch {
      // Deleting is best-effort; Torii expires sessions that outlive their TTL.
    }
  }

  #setState(next) {
    this.#state = Object.freeze({ ...next });
    this.#emit("statechange", this.#state);
  }

  #emit(type, detail) {
    this.dispatchEvent(new CustomEvent(type, { detail }));
  }
}
