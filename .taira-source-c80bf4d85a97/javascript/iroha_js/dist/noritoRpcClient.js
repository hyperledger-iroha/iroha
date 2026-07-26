import { Buffer } from "node:buffer";

function isSecureProtocol(protocol) {
  const normalized = typeof protocol === "string" ? protocol.toLowerCase() : "";
  return normalized === "https:" || normalized === "wss:";
}

function isAbsoluteUrl(candidate) {
  if (!candidate) {
    return false;
  }
  if (candidate instanceof URL) {
    return true;
  }
  if (typeof candidate !== "string") {
    return false;
  }
  return /^[a-z][a-z0-9+.-]*:\/\//iu.test(candidate);
}

function hasHeader(headers, name) {
  if (!headers || typeof headers !== "object") {
    return false;
  }
  const target = String(name).toLowerCase();
  return Object.keys(headers).some((key) => key.toLowerCase() === target);
}

function headersContainCredentials(headers) {
  return (
    hasHeader(headers, "authorization") ||
    hasHeader(headers, "x-api-token")
  );
}

function applyCredentialHeaders(headers, authToken, apiToken) {
  if (!headers || typeof headers !== "object") {
    return;
  }
  if (apiToken) {
    if (!hasHeader(headers, "x-api-token")) {
      headers["X-API-Token"] = String(apiToken);
    }
  }
  if (authToken && !hasHeader(headers, "authorization")) {
    headers.Authorization = `Bearer ${authToken}`;
  }
}

export class NoritoRpcError extends Error {
  constructor(status, body) {
    super(`Norito RPC request failed with status ${status}`);
    this.name = "NoritoRpcError";
    this.status = status;
    this.body = body;
  }
}

export class NoritoRpcClient {
  /**
   * @param {string} baseUrl Base Torii URL (e.g. http://localhost:8080).
   * @param {object} [options]
   * @param {typeof fetch} [options.fetchImpl] Custom fetch implementation.
   * @param {Record<string, string>} [options.defaultHeaders]
   * @param {number} [options.timeoutMs]
   * @param {boolean} [options.allowInsecure] Allow insecure http/ws when credentials are present (dev only).
   * @param {string} [options.authToken] Bearer token attached as Authorization when provided.
   * @param {string} [options.apiToken] API token attached as X-API-Token when provided.
   * @param {(event: import("../index.d.ts").InsecureTransportTelemetryEvent) => void} [options.insecureTransportTelemetryHook]
   */
  constructor(baseUrl, options = {}) {
    if (!baseUrl) {
      throw new Error("baseUrl is required");
    }
    this._baseUrl = baseUrl.endsWith("/") ? baseUrl.slice(0, -1) : baseUrl;
    const parsedBase = new URL(
      this._baseUrl.endsWith("/") ? this._baseUrl : `${this._baseUrl}/`,
    );
    this._baseOrigin = `${parsedBase.protocol}//${parsedBase.host}`;
    this._baseHost = parsedBase.host;
    this._baseProtocol = parsedBase.protocol.toLowerCase();
    this._allowInsecure = Boolean(options.allowInsecure);
    this._authToken = options.authToken ?? null;
    this._apiToken = options.apiToken ?? null;
    this._insecureTelemetryHook =
      typeof options.insecureTransportTelemetryHook === "function"
        ? options.insecureTransportTelemetryHook
        : null;
    this._fetch = options.fetchImpl ?? options.fetch ?? globalThis.fetch;
    if (typeof this._fetch !== "function") {
      throw new Error("fetch implementation is required");
    }
    this._defaultHeaders = normalizeHeaders(options.defaultHeaders);
    applyCredentialHeaders(this._defaultHeaders, this._authToken, this._apiToken);
    this._timeoutMs =
      options.timeoutMs === undefined || options.timeoutMs === null
        ? null
        : Math.max(0, Number(options.timeoutMs));
    const hasConfiguredCredentials =
      headersContainCredentials(this._defaultHeaders) ||
      this._authToken !== null ||
      this._apiToken !== null;
    if (hasConfiguredCredentials && !this._allowInsecure && !isSecureProtocol(this._baseProtocol)) {
      throw new Error(
        "NoritoRpcClient: auth/api tokens require an https base URL; pass allowInsecure: true for local/dev use only.",
      );
    }
  }

  get baseUrl() {
    return this._baseUrl;
  }

  close() {
    // Present for API parity with Python helper; no resources to release.
  }

  /**
   * Invoke a Torii Norito RPC endpoint.
   * @param {string} path Request path (e.g. /v1/pipeline/submit).
   * @param {ArrayBuffer | ArrayBufferView | Buffer} payload Norito-encoded bytes.
   * @param {object} [options]
   * @param {number} [options.timeoutMs]
   * @param {Record<string, string | null | undefined>} [options.headers]
   * @param {string | null} [options.accept]
   * @param {string} [options.method]
   * @param {Record<string, string | number | boolean>} [options.params]
   * @param {AbortSignal} [options.signal]
   * @param {boolean} [options.allowAbsoluteUrl] Allow cross-host URLs when no credentials are attached.
   * @param {string} [options.authToken] Per-call bearer token override.
   * @param {string} [options.apiToken] Per-call API token override.
   * @returns {Promise<Uint8Array>}
   */
  async call(path, payload, options = {}) {
    const body = normalizePayload(payload);
    const method = (options.method ?? "POST").toUpperCase();
    const pathIsAbsolute = isAbsoluteUrl(path);
    const urlObj = pathIsAbsolute
      ? new URL(path)
      : new URL(path ?? "", `${this._baseUrl}/`);
    if (options.params && Object.keys(options.params).length > 0) {
      for (const [key, value] of Object.entries(options.params)) {
        if (value === undefined || value === null) {
          continue;
        }
        urlObj.searchParams.append(key, String(value));
      }
    }
    const protocol = urlObj.protocol.toLowerCase();
    const originMatches = urlObj.host === this._baseHost && protocol === this._baseProtocol;
    const headers = {
      ...this._defaultHeaders,
      "Content-Type": "application/x-norito",
    };
    const disableAccept = options.accept === null;
    const acceptHeader =
      disableAccept || options.accept === undefined
        ? disableAccept
          ? null
          : "application/x-norito"
        : options.accept;
    if (acceptHeader) {
      headers.Accept = acceptHeader;
    } else {
      delete headers.Accept;
    }
    if (options.headers) {
      for (const [key, value] of Object.entries(options.headers)) {
        const lower = typeof key === "string" ? key.toLowerCase() : key;
        const targetKey =
          lower === "accept"
            ? "Accept"
            : lower === "content-type"
              ? "Content-Type"
              : key;
        if (value === undefined || value === null) {
          delete headers[targetKey];
          if (targetKey !== key) {
            delete headers[key];
          }
          continue;
        }
        if (lower === "accept" && disableAccept) {
          delete headers.Accept;
          continue;
        }
        headers[targetKey] = String(value);
      }
    }
    applyCredentialHeaders(
      headers,
      options.authToken ?? this._authToken,
      options.apiToken ?? this._apiToken,
    );
    const hasCredentials = headersContainCredentials(headers);
    const allowAbsoluteUrl = options.allowAbsoluteUrl === true;
    if (hasCredentials) {
      if (protocol !== this._baseProtocol) {
        throw new Error(
          `NoritoRpcClient: refusing protocol ${urlObj.protocol} when credentials are attached; use ${this._baseProtocol.replace(":", "")} URLs derived from the client base URL.`,
        );
      }
      if (pathIsAbsolute && urlObj.host !== this._baseHost) {
        throw new Error(
          `NoritoRpcClient: refusing host override ${urlObj.host} when credentials are attached; use relative paths on the configured base URL.`,
        );
      }
      if (!this._allowInsecure && !isSecureProtocol(protocol)) {
        throw new Error(
          `NoritoRpcClient: refusing insecure protocol ${urlObj.protocol} with credentials; use https or set allowInsecure: true for dev.`,
        );
      }
    } else if (pathIsAbsolute && !originMatches && !allowAbsoluteUrl) {
      throw new Error(
        "NoritoRpcClient: absolute URLs are blocked when no credentials are attached; pass allowAbsoluteUrl: true to override.",
      );
    }
    if (hasCredentials && this._allowInsecure && !isSecureProtocol(protocol)) {
      this._emitInsecureTransportTelemetry({
        client: "norito-rpc",
        method,
        hasCredentials: true,
        allowInsecure: true,
        url: urlObj.toString(),
        baseUrl: this._baseUrl,
        host: urlObj.host,
        protocol,
        pathIsAbsolute,
        originMatches,
      });
    }

    const timeout =
      options.timeoutMs === undefined || options.timeoutMs === null
        ? this._timeoutMs
        : Math.max(0, Number(options.timeoutMs));
    const response = await this._fetchWithTimeout(
      urlObj.toString(),
      { method, headers, body },
      timeout,
      options.signal,
    );
    if (response.status < 200 || response.status >= 300) {
      const text = await safeReadText(response);
      throw new NoritoRpcError(response.status, text);
    }
    const buffer = await response.arrayBuffer();
    return new Uint8Array(buffer);
  }

  async _fetchWithTimeout(url, init, timeoutMs, externalSignal) {
    if (timeoutMs == null) {
      const finalInit =
        externalSignal == null ? init : { ...init, signal: externalSignal };
      return this._fetch(url, finalInit);
    }
    const abortController = new AbortController();
    const signal = combineAbortSignals(externalSignal, abortController.signal);
    const finalInit = { ...init, signal };
    const timer = setTimeout(() => abortController.abort(), timeoutMs);
    try {
      return await this._fetch(url, finalInit);
    } finally {
      clearTimeout(timer);
    }
  }

  _emitInsecureTransportTelemetry(event) {
    if (typeof this._insecureTelemetryHook !== "function") {
      return;
    }
    try {
      this._insecureTelemetryHook({ ...event, timestampMs: Date.now() });
    } catch {
      // Telemetry must never interrupt the call path.
    }
  }
}

function normalizeHeaders(input) {
  if (!input) {
    return {};
  }
  const result = {};
  for (const [key, value] of Object.entries(input)) {
    if (value == null) {
      continue;
    }
    result[key] = String(value);
  }
  return result;
}

function normalizePayload(payload) {
  if (payload == null) {
    throw new TypeError("payload is required");
  }
  if (Buffer.isBuffer(payload)) {
    return payload;
  }
  if (payload instanceof Uint8Array) {
    return Buffer.from(payload);
  }
  if (ArrayBuffer.isView(payload)) {
    return Buffer.from(
      payload.buffer,
      payload.byteOffset,
      payload.byteLength,
    );
  }
  if (payload instanceof ArrayBuffer) {
    return Buffer.from(payload);
  }
  throw new TypeError("payload must be Buffer, Uint8Array, or ArrayBuffer");
}

async function safeReadText(response) {
  try {
    return await response.text();
  } catch (error) {
    return `Unable to read response body: ${String(error)}`;
  }
}

function combineAbortSignals(a, b) {
  if (!a) {
    return b;
  }
  if (!b) {
    return a;
  }
  if (typeof AbortSignal !== "undefined" && typeof AbortSignal.any === "function") {
    return AbortSignal.any([a, b]);
  }
  const controller = new AbortController();
  const abort = () => controller.abort();
  if (a.aborted || b.aborted) {
    controller.abort();
  } else {
    const opts = { once: true };
    a.addEventListener("abort", abort, opts);
    b.addEventListener("abort", abort, opts);
    controller.signal.addEventListener(
      "abort",
      () => {
        a.removeEventListener("abort", abort);
        b.removeEventListener("abort", abort);
      },
      { once: true },
    );
  }
  return controller.signal;
}
