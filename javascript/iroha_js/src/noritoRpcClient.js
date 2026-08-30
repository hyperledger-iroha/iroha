import { Buffer } from "node:buffer";

function isSecureProtocol(protocol) {
  const normalized = typeof protocol === "string" ? protocol.toLowerCase() : "";
  return normalized === "https:";
}

function isAbsoluteUrl(candidate) {
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

function deleteHeader(headers, name) {
  const target = name.toLowerCase();
  for (const key of Object.keys(headers)) {
    if (key.toLowerCase() === target) {
      delete headers[key];
    }
  }
}

function setHeader(headers, name, value) {
  if (!/^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/u.test(name)) {
    throw new TypeError(`invalid HTTP header name: ${name}`);
  }
  if (typeof value !== "string" || /[\0\r\n]/u.test(value)) {
    throw new TypeError(`HTTP header ${name} must be a single-line string`);
  }
  Object.defineProperty(headers, name, {
    configurable: true,
    enumerable: true,
    value,
    writable: true,
  });
}

function applyCredentialOverride(
  headers,
  headerName,
  options,
  optionName,
  format,
) {
  if (!Object.hasOwn(options, optionName)) {
    return;
  }
  const token = normalizeOptionalToken(options[optionName], `options.${optionName}`);
  deleteHeader(headers, headerName);
  if (token !== null) {
    setHeader(headers, headerName, format(token));
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
  #allowInsecure;
  #baseHost;
  #baseProtocol;
  #baseUrl;
  #defaultHeaders;
  #fetch;
  #insecureTelemetryHook;
  #timeoutMs;

  /**
   * @param {string} baseUrl Base Torii URL (e.g. http://localhost:8080).
   * @param {object} [options]
   * @param {typeof fetch} [options.fetchImpl] Custom fetch implementation. It must honor
   * `redirect: "error"` and must not retry a dispatched request body.
   * @param {Record<string, string>} [options.defaultHeaders]
   * @param {number | null} [options.timeoutMs]
   * @param {boolean} [options.allowInsecure] Allow insecure http/ws when credentials are present (dev only).
   * @param {string | null} [options.authToken] Bearer token attached as Authorization when provided.
   * @param {string | null} [options.apiToken] API token attached as X-API-Token when provided.
   * @param {(event: import("../index.d.ts").InsecureTransportTelemetryEvent) => void} [options.insecureTransportTelemetryHook]
   */
  constructor(baseUrl, options = {}) {
    if (
      typeof baseUrl !== "string" ||
      baseUrl.length === 0 ||
      baseUrl.trim() !== baseUrl
    ) {
      throw new TypeError("baseUrl must be a non-empty string");
    }
    requireOptionsObject(options, "NoritoRpcClient options");

    const parsedBase = new URL(baseUrl);
    if (parsedBase.protocol !== "http:" && parsedBase.protocol !== "https:") {
      throw new TypeError("baseUrl must use http or https");
    }
    if (parsedBase.username !== "" || parsedBase.password !== "") {
      throw new TypeError("baseUrl must not contain credentials");
    }
    if (parsedBase.search !== "" || parsedBase.hash !== "") {
      throw new TypeError("baseUrl must not contain a query or fragment");
    }
    const basePath = parsedBase.pathname.replace(/\/+$/u, "");
    this.#baseUrl = `${parsedBase.origin}${basePath}`;
    this.#baseHost = parsedBase.host;
    this.#baseProtocol = parsedBase.protocol.toLowerCase();
    this.#allowInsecure = normalizeBooleanOption(
      options.allowInsecure,
      "options.allowInsecure",
      false,
    );
    this.#insecureTelemetryHook = normalizeOptionalFunction(
      options.insecureTransportTelemetryHook,
      "options.insecureTransportTelemetryHook",
    );
    this.#fetch = options.fetchImpl ?? globalThis.fetch;
    if (typeof this.#fetch !== "function") {
      throw new TypeError("options.fetchImpl must be a function");
    }
    const defaultHeaders = normalizeHeaders(
      options.defaultHeaders,
      "options.defaultHeaders",
    );
    applyCredentialOverride(
      defaultHeaders,
      "Authorization",
      options,
      "authToken",
      (token) => `Bearer ${token}`,
    );
    applyCredentialOverride(
      defaultHeaders,
      "X-API-Token",
      options,
      "apiToken",
      (token) => token,
    );
    this.#defaultHeaders = Object.freeze(defaultHeaders);
    this.#timeoutMs = normalizeTimeout(options.timeoutMs, "options.timeoutMs");
    if (
      headersContainCredentials(this.#defaultHeaders) &&
      !this.#allowInsecure &&
      !isSecureProtocol(this.#baseProtocol)
    ) {
      throw new Error(
        "NoritoRpcClient: auth/api tokens require an https base URL; pass allowInsecure: true for local/dev use only.",
      );
    }
  }

  get baseUrl() {
    return this.#baseUrl;
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
   * @param {string | null} [options.authToken] Per-call bearer token override.
   * @param {string | null} [options.apiToken] Per-call API token override.
   * @returns {Promise<Uint8Array>}
   */
  async call(path, payload, options = {}) {
    requireOptionsObject(options, "NoritoRpcClient.call options");
    if (typeof path !== "string" || path.length === 0) {
      throw new TypeError("path must be a non-empty string");
    }
    const body = normalizePayload(payload);
    const method = normalizeMethod(options.method);
    const pathIsAbsolute = isAbsoluteUrl(path);
    const urlObj = pathIsAbsolute
      ? new URL(path)
      : new URL(path ?? "", `${this.#baseUrl}/`);
    if (options.params && Object.keys(options.params).length > 0) {
      for (const [key, value] of Object.entries(options.params)) {
        if (value === undefined || value === null) {
          continue;
        }
        urlObj.searchParams.append(key, String(value));
      }
    }
    const protocol = urlObj.protocol.toLowerCase();
    if (protocol !== "http:" && protocol !== "https:") {
      throw new TypeError("NoritoRpcClient.call URL must use http or https");
    }
    if (urlObj.username !== "" || urlObj.password !== "") {
      throw new TypeError("NoritoRpcClient.call URL must not contain credentials");
    }
    if (urlObj.hash !== "") {
      throw new TypeError("NoritoRpcClient.call URL must not contain a fragment");
    }
    const originMatches =
      urlObj.host === this.#baseHost && protocol === this.#baseProtocol;
    const headers = {
      ...this.#defaultHeaders,
      "Content-Type": "application/x-norito",
    };
    const disableAccept = options.accept === null;
    const acceptHeader =
      disableAccept || options.accept === undefined
        ? disableAccept
          ? null
          : "application/x-norito"
        : options.accept;
    if (acceptHeader !== null && typeof acceptHeader !== "string") {
      throw new TypeError("options.accept must be a string or null");
    }
    if (acceptHeader) {
      headers.Accept = acceptHeader;
    } else {
      delete headers.Accept;
    }
    if (options.headers !== undefined && options.headers !== null) {
      requireOptionsObject(options.headers, "options.headers");
      for (const [key, value] of Object.entries(options.headers)) {
        const lower = typeof key === "string" ? key.toLowerCase() : key;
        const targetKey =
          lower === "accept"
            ? "Accept"
            : lower === "content-type"
              ? "Content-Type"
              : key;
        if (value === undefined || value === null) {
          deleteHeader(headers, targetKey);
          continue;
        }
        if (lower === "accept" && disableAccept) {
          deleteHeader(headers, "Accept");
          continue;
        }
        if (typeof value !== "string") {
          throw new TypeError(`options.headers.${key} must be a string, null, or undefined`);
        }
        deleteHeader(headers, targetKey);
        setHeader(headers, targetKey, value);
      }
    }
    applyCredentialOverride(
      headers,
      "Authorization",
      options,
      "authToken",
      (token) => `Bearer ${token}`,
    );
    applyCredentialOverride(
      headers,
      "X-API-Token",
      options,
      "apiToken",
      (token) => token,
    );
    const hasCredentials = headersContainCredentials(headers);
    const allowAbsoluteUrl = normalizeBooleanOption(
      options.allowAbsoluteUrl,
      "options.allowAbsoluteUrl",
      false,
    );
    if (hasCredentials) {
      if (protocol !== this.#baseProtocol) {
        throw new Error(
          `NoritoRpcClient: refusing protocol ${urlObj.protocol} when credentials are attached; use ${this.#baseProtocol.replace(":", "")} URLs derived from the client base URL.`,
        );
      }
      if (pathIsAbsolute && urlObj.host !== this.#baseHost) {
        throw new Error(
          `NoritoRpcClient: refusing host override ${urlObj.host} when credentials are attached; use relative paths on the configured base URL.`,
        );
      }
      if (!this.#allowInsecure && !isSecureProtocol(protocol)) {
        throw new Error(
          `NoritoRpcClient: refusing insecure protocol ${urlObj.protocol} with credentials; use https or set allowInsecure: true for dev.`,
        );
      }
    } else if (pathIsAbsolute && !originMatches && !allowAbsoluteUrl) {
      throw new Error(
        "NoritoRpcClient: absolute URLs are blocked when no credentials are attached; pass allowAbsoluteUrl: true to override.",
      );
    }
    if (hasCredentials && this.#allowInsecure && !isSecureProtocol(protocol)) {
      this.#emitInsecureTransportTelemetry({
        client: "norito-rpc",
        method,
        hasCredentials: true,
        allowInsecure: true,
        url: urlObj.toString(),
        baseUrl: this.#baseUrl,
        host: urlObj.host,
        protocol,
        pathIsAbsolute,
        originMatches,
      });
    }

    const timeout =
      options.timeoutMs === undefined || options.timeoutMs === null
        ? this.#timeoutMs
        : normalizeTimeout(options.timeoutMs, "options.timeoutMs");
    const response = await this.#fetchWithTimeout(
      urlObj.toString(),
      { method, headers, body, redirect: "error" },
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

  async #fetchWithTimeout(url, init, timeoutMs, externalSignal) {
    if (timeoutMs == null) {
      const finalInit =
        externalSignal == null ? init : { ...init, signal: externalSignal };
      return this.#fetch(url, finalInit);
    }
    const abortController = new AbortController();
    const signal = combineAbortSignals(externalSignal, abortController.signal);
    const finalInit = { ...init, signal };
    const timer = setTimeout(() => abortController.abort(), timeoutMs);
    try {
      return await this.#fetch(url, finalInit);
    } finally {
      clearTimeout(timer);
    }
  }

  #emitInsecureTransportTelemetry(event) {
    if (this.#insecureTelemetryHook === null) {
      return;
    }
    try {
      this.#insecureTelemetryHook({ ...event, timestampMs: Date.now() });
    } catch {
      // Telemetry must never interrupt the call path.
    }
  }
}

function normalizeHeaders(input, context) {
  if (input === undefined) {
    return {};
  }
  requireOptionsObject(input, context);
  const result = {};
  for (const [key, value] of Object.entries(input)) {
    if (typeof value !== "string") {
      throw new TypeError(`${context}.${key} must be a string`);
    }
    setHeader(result, key, value);
  }
  return result;
}

function requireOptionsObject(value, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${context} must be a plain object`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new TypeError(`${context} must be a plain object`);
  }
}

function normalizeBooleanOption(value, context, fallback) {
  if (value === undefined) {
    return fallback;
  }
  if (typeof value !== "boolean") {
    throw new TypeError(`${context} must be a boolean`);
  }
  return value;
}

function normalizeOptionalFunction(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  if (typeof value !== "function") {
    throw new TypeError(`${context} must be a function`);
  }
  return value;
}

function normalizeOptionalToken(value, context) {
  if (value === undefined || value === null || value === "") {
    return null;
  }
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a string or null`);
  }
  return value;
}

function normalizeTimeout(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new TypeError(`${context} must be a non-negative safe integer or null`);
  }
  return value;
}

function normalizeMethod(value) {
  if (value === undefined) {
    return "POST";
  }
  if (
    typeof value !== "string" ||
    !/^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/u.test(value)
  ) {
    throw new TypeError("options.method must be a non-empty HTTP token");
  }
  return value.toUpperCase();
}

function normalizePayload(payload) {
  if (payload == null) {
    throw new TypeError("payload is required");
  }
  if (ArrayBuffer.isView(payload)) {
    return Buffer.from(
      new Uint8Array(payload.buffer, payload.byteOffset, payload.byteLength),
    );
  }
  if (payload instanceof ArrayBuffer) {
    return Buffer.from(new Uint8Array(payload));
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
