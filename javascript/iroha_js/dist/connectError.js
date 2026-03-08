const CATEGORY = Object.freeze({
  TRANSPORT: "transport",
  CODEC: "codec",
  AUTHORIZATION: "authorization",
  TIMEOUT: "timeout",
  QUEUE_OVERFLOW: "queueOverflow",
  INTERNAL: "internal",
});

const DEFAULT_CODE = "unknown_error";
const TRANSPORT_CODES = new Set([
  "aborted",
  "ECONNRESET",
  "ECONNREFUSED",
  "ECONNABORTED",
  "EPIPE",
  "ENETUNREACH",
  "EHOSTUNREACH",
  "ENOTFOUND",
  "UND_ERR_CONNECT_TIMEOUT",
  "UND_ERR_HEADERS_TIMEOUT",
]);
const TIMEOUT_CODES = new Set([
  "ETIMEDOUT",
  "ETIME",
  "ESOCKETTIMEDOUT",
  "UND_ERR_BODY_TIMEOUT",
]);
const TLS_CODES = new Set([
  "ERR_TLS_CERT_ALTNAME_INVALID",
  "ERR_TLS_CERT_SIGNATURE_ALGORITHM_UNSUPPORTED",
  "ERR_TLS_CERT_REQUIRED",
  "ERR_TLS_CERT_VALIDATION",
  "ERR_TLS_DH_PARAM_SIZE",
  "ERR_TLS_HANDSHAKE_TIMEOUT",
  "ERR_TLS_RENEGOTIATION_FAILED",
  "ERR_TLS_CERTIFICATE_REQUIRED",
  "ERR_TLS_CERTIFICATE_UNKNOWN",
  "ERR_TLS_INVALID_PROTOCOL_VERSION",
  "ERR_TLS_PROTOCOL_VERSION_CONFLICT",
  "ERR_SSL_PROTOCOL_ERROR",
  "UNABLE_TO_VERIFY_LEAF_SIGNATURE",
  "CERT_SIGNATURE_FAILURE",
  "CERT_NOT_YET_VALID",
  "CERT_HAS_EXPIRED",
  "DEPTH_ZERO_SELF_SIGNED_CERT",
]);

const HTTP_AUTH_STATUS = new Set([401, 403, 407]);
const HTTP_TIMEOUT_STATUS = 408;
const HTTP_RATE_LIMIT_STATUS = 429;

function normalizeFatal(value) {
  return value === true;
}

function normalizeHttpStatus(value) {
  if (value === undefined || value === null) {
    return undefined;
  }
  const num = Number(value);
  // eslint-disable-next-line no-restricted-globals
  return Number.isFinite(num) ? num : undefined;
}

function formatUnderlying(input) {
  if (typeof input === "string") {
    return input;
  }
  if (input && typeof input === "object") {
    if (typeof input.stack === "string" && input.stack.trim().length > 0) {
      return input.stack;
    }
    if (typeof input.message === "string" && input.message.trim().length > 0) {
      return input.message;
    }
    if (typeof input.code === "string") {
      return input.code;
    }
  }
  if (input === undefined || input === null) {
    return undefined;
  }
  try {
    return JSON.stringify(input);
  } catch {
    return String(input);
  }
}

export class ConnectError extends Error {
  constructor({
    category,
    code,
    message,
    fatal = false,
    httpStatus,
    underlying,
    cause,
  }) {
    super(message ?? code ?? DEFAULT_CODE);
    this.name = "ConnectError";
    this.category = category ?? CATEGORY.INTERNAL;
    this.code = code ?? DEFAULT_CODE;
    this.fatal = normalizeFatal(fatal);
    this.httpStatus = normalizeHttpStatus(httpStatus);
    this.underlying = underlying ?? formatUnderlying(cause);
    if (cause !== undefined) {
      this.cause = cause;
    }
  }

  telemetryAttributes(options = {}) {
    const fatal =
      options.fatal !== undefined ? normalizeFatal(options.fatal) : this.fatal;
    const httpStatus =
      normalizeHttpStatus(options.httpStatus) ?? this.httpStatus;
    const underlying = options.underlying ?? this.underlying;
    const attrs = {
      category: this.category,
      code: this.code,
      fatal: String(fatal),
    };
    if (httpStatus !== undefined) {
      attrs.http_status = String(httpStatus);
    }
    if (underlying) {
      attrs.underlying = underlying;
    }
    return attrs;
  }
}

export const ConnectErrorCategory = CATEGORY;

export class ConnectQueueError extends Error {
  static KIND = Object.freeze({
    OVERFLOW: "overflow",
    EXPIRED: "expired",
  });

  constructor(kind, { limit, ttlMs } = {}) {
    const message =
      kind === ConnectQueueError.KIND.EXPIRED
        ? "Connect queue entry expired before transmission"
        : "Connect queue overflow";
    super(message);
    this.name = "ConnectQueueError";
    this.kind =
      kind ?? ConnectQueueError.KIND.OVERFLOW;
    this.limit = typeof limit === "number" ? limit : undefined;
    this.ttlMs = typeof ttlMs === "number" ? ttlMs : undefined;
  }

  static overflow(limit) {
    return new ConnectQueueError(ConnectQueueError.KIND.OVERFLOW, { limit });
  }

  static expired(ttlMs) {
    return new ConnectQueueError(ConnectQueueError.KIND.EXPIRED, { ttlMs });
  }

  toConnectError() {
    if (this.kind === ConnectQueueError.KIND.EXPIRED) {
      return new ConnectError({
        category: CATEGORY.TIMEOUT,
        code: "queue.expired",
        message: this.message,
        fatal: false,
        underlying:
          this.ttlMs !== undefined ? `ttlMs=${this.ttlMs}` : undefined,
        cause: this,
      });
    }
    return new ConnectError({
      category: CATEGORY.QUEUE_OVERFLOW,
      code: "queue.overflow",
      message: this.message,
      fatal: false,
      underlying:
        this.limit !== undefined ? `limit=${this.limit}` : undefined,
      cause: this,
    });
  }
}

function isHttpLike(error) {
  if (error && typeof error === "object") {
    if (typeof error.status === "number") {
      return { status: error.status, statusText: error.statusText };
    }
    if (error.response && typeof error.response.status === "number") {
      return {
        status: error.response.status,
        statusText: error.response.statusText,
      };
    }
  }
  return null;
}

function httpErrorDetails(error) {
  const info = isHttpLike(error);
  if (!info) {
    return null;
  }
  const status = info.status;
  const statusText =
    typeof info.statusText === "string" ? info.statusText : undefined;
  if (status === HTTP_TIMEOUT_STATUS) {
    return {
      category: CATEGORY.TIMEOUT,
      code: "http.timeout",
      message: statusText ?? `HTTP ${status}`,
      httpStatus: status,
    };
  }
  if (status === HTTP_RATE_LIMIT_STATUS) {
    return {
      category: CATEGORY.TRANSPORT,
      code: "http.rate_limited",
      message: statusText ?? `HTTP ${status}`,
      httpStatus: status,
    };
  }
  if (status >= 500) {
    return {
      category: CATEGORY.TRANSPORT,
      code: "http.server_error",
      message: statusText ?? `HTTP ${status}`,
      httpStatus: status,
    };
  }
  if (HTTP_AUTH_STATUS.has(status)) {
    return {
      category: CATEGORY.AUTHORIZATION,
      code: "http.forbidden",
      message: statusText ?? `HTTP ${status}`,
      httpStatus: status,
    };
  }
  if (status >= 400 && status < 500) {
    return {
      category: CATEGORY.TRANSPORT,
      code: "http.client_error",
      message: statusText ?? `HTTP ${status}`,
      httpStatus: status,
    };
  }
  return {
    category: CATEGORY.TRANSPORT,
    code: "http.other",
    message: statusText ?? `HTTP ${status}`,
    httpStatus: status,
  };
}

function tlsErrorDetails(error) {
  if (
    !error ||
    typeof error !== "object" ||
    (typeof error.code !== "string" &&
      typeof error.message !== "string" &&
      typeof error.name !== "string")
  ) {
    return null;
  }
  if (
    (typeof error.code === "string" && TLS_CODES.has(error.code)) ||
    (typeof error.name === "string" &&
      error.name.toLowerCase().includes("security")) ||
    (typeof error.message === "string" &&
      error.message.toLowerCase().includes("certificate"))
  ) {
    return {
      category: CATEGORY.AUTHORIZATION,
      code: "network.tls_failure",
      message: error.message,
    };
  }
  return null;
}

function timeoutErrorDetails(error) {
  if (!error) {
    return null;
  }
  const name = typeof error.name === "string" ? error.name.toLowerCase() : "";
  if (name.includes("timeout")) {
    return {
      category: CATEGORY.TIMEOUT,
      code: "network.timeout",
      message: error.message,
    };
  }
  const code = typeof error.code === "string" ? error.code : undefined;
  if (code && TIMEOUT_CODES.has(code)) {
    return {
      category: CATEGORY.TIMEOUT,
      code: "network.timeout",
      message: error.message,
    };
  }
  return null;
}

function transportErrorDetails(error) {
  if (!error) {
    return null;
  }
  if (error instanceof ConnectError) {
    return {
      category: error.category,
      code: error.code,
      message: error.message,
      httpStatus: error.httpStatus,
    };
  }
  if (
    typeof error.code === "string" &&
    error.code === "ECONNRESET"
  ) {
    return {
      category: CATEGORY.TRANSPORT,
      code: "client.closed",
      message: error.message,
    };
  }
  if (
    typeof error.code === "string" &&
    TRANSPORT_CODES.has(error.code)
  ) {
    return {
      category: CATEGORY.TRANSPORT,
      code: "network.socket_failure",
      message: error.message,
    };
  }
  if (
    typeof error.name === "string" &&
    error.name === "AbortError"
  ) {
    return {
      category: CATEGORY.TRANSPORT,
      code: "client.aborted",
      message: error.message ?? "The connection was aborted",
    };
  }
  if (
    typeof error.message === "string" &&
    error.message.toLowerCase().includes("websocket")
  ) {
    return {
      category: CATEGORY.TRANSPORT,
      code: "network.websocket_failure",
      message: error.message,
    };
  }
  return null;
}

function codecErrorDetails(error) {
  if (!error) {
    return null;
  }
  if (error instanceof SyntaxError) {
    return {
      category: CATEGORY.CODEC,
      code: "codec.syntax_error",
      message: error.message,
    };
  }
  if (
    typeof error.name === "string" &&
    error.name.toLowerCase().includes("decode")
  ) {
    return {
      category: CATEGORY.CODEC,
      code: "codec.decode_failure",
      message: error.message,
    };
  }
  return null;
}

function classifyUnknownError(error) {
  return {
    category: CATEGORY.INTERNAL,
    code: DEFAULT_CODE,
    message:
      typeof error?.message === "string"
        ? error.message
        : "Unknown Connect error",
  };
}

function mapErrorDetails(error) {
  const direct =
    httpErrorDetails(error) ??
    tlsErrorDetails(error) ??
    timeoutErrorDetails(error) ??
    transportErrorDetails(error) ??
    codecErrorDetails(error);
  if (direct) {
    return direct;
  }
  return classifyUnknownError(error);
}

export function connectErrorFrom(error, options = {}) {
  if (error instanceof ConnectError) {
    return error;
  }
  if (error && typeof error.toConnectError === "function") {
    const converted = error.toConnectError();
    if (converted instanceof ConnectError) {
      return converted;
    }
  }
  const details = mapErrorDetails(error);
  const fatal =
    options.fatal !== undefined ? normalizeFatal(options.fatal) : false;
  const httpStatus =
    normalizeHttpStatus(options.httpStatus) ?? details.httpStatus;
  return new ConnectError({
    ...details,
    fatal,
    httpStatus,
    underlying: formatUnderlying(error),
    cause: error instanceof Error ? error : undefined,
  });
}
