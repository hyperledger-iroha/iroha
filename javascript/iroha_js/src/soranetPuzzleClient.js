/**
 * Lightweight client for the SoraNet puzzle/admission-token service.
 */
export class SoranetPuzzleError extends Error {
  constructor(status, body) {
    super(`SoraNet puzzle service request failed with status ${status}`);
    this.name = "SoranetPuzzleError";
    this.status = status;
    this.body = body;
  }
}

export class SoranetPuzzleClient {
  /**
   * @param {string} baseUrl Base service URL (e.g. http://localhost:8088).
   * @param {object} [options]
   * @param {typeof fetch} [options.fetchImpl] Custom fetch implementation.
   * @param {Record<string, string>} [options.defaultHeaders]
   * @param {number | null} [options.timeoutMs]
   */
  constructor(baseUrl, options = {}) {
    if (typeof baseUrl !== "string" || baseUrl.trim() === "") {
      throw new TypeError("baseUrl must be a non-empty string");
    }
    const trimmed = baseUrl.trim();
    this._baseUrl = trimmed.endsWith("/") ? trimmed.slice(0, -1) : trimmed;
    this._fetch = options.fetchImpl ?? options.fetch ?? globalThis.fetch;
    if (typeof this._fetch !== "function") {
      throw new Error("fetch implementation is required");
    }
    this._defaultHeaders = normalizeHeaders(options.defaultHeaders);
    this._timeoutMs =
      options.timeoutMs === undefined || options.timeoutMs === null
        ? null
        : coerceNonNegativeNumber(options.timeoutMs, "options.timeoutMs");
  }

  get baseUrl() {
    return this._baseUrl;
  }

  /**
   * Fetch the current puzzle/token configuration.
   * @param {object} [options]
   * @returns {Promise<{
   *   required: boolean,
   *   difficulty: number,
   *   maxFutureSkewSecs: number,
   *   minTicketTtlSecs: number,
   *   ticketTtlSecs: number,
   *   puzzle: null | { memoryKib: number, timeCost: number, lanes: number },
   *   token: ReturnType<typeof normalizeTokenConfig>
   * }>}
   */
  async getPuzzleConfig(options = {}) {
    const payload = await this._request("GET", "/v1/puzzle/config", null, options);
    return normalizePuzzleConfig(payload, "puzzle config response");
  }

  /**
   * Mint an Argon2 puzzle ticket.
   * @param {object} [options]
   * @param {number | bigint} [options.ttlSecs] Optional TTL override.
   * @param {string} [options.transcriptHashHex] Optional 32-byte transcript hash (hex) to bind.
   * @param {boolean} [options.signed] Request a relay-signed ticket when signing keys are configured.
   * @returns {Promise<{ ticketB64: string, signedTicketB64: string | null, signedTicketFingerprintHex: string | null, difficulty: number, ttlSecs: number, expiresAt: number }>}
   */
  async mintPuzzleTicket(options = {}) {
    const body = {};
    if (options.ttlSecs !== undefined && options.ttlSecs !== null) {
      body.ttl_secs = coercePositiveInteger(options.ttlSecs, "options.ttlSecs");
    }
    if (options.transcriptHashHex !== undefined && options.transcriptHashHex !== null) {
      body.transcript_hash_hex = normalizeHexString(
        options.transcriptHashHex,
        32,
        "options.transcriptHashHex",
      );
    }
    if (options.signed !== undefined && options.signed !== null) {
      body.signed = coerceBoolean(options.signed, "options.signed");
    }
    const finalBody = Object.keys(body).length === 0 ? undefined : body;
    const payload = await this._request("POST", "/v1/puzzle/mint", finalBody, options);
    return normalizePuzzleMintResponse(payload, "puzzle mint response");
  }

  /**
   * Fetch the admission-token configuration.
   * @param {object} [options]
   */
  async getTokenConfig(options = {}) {
    const payload = await this._request("GET", "/v1/token/config", null, options);
    return normalizeTokenConfig(payload, "token config response");
  }

  /**
   * Mint an ML-DSA admission token.
   * @param {string} transcriptHashHex 32-byte transcript hash (hex).
   * @param {object} [options]
   * @param {number | bigint} [options.ttlSecs]
   * @param {number} [options.flags]
   * @param {number | bigint} [options.issuedAtUnix]
   * @returns {Promise<{
   *   tokenB64: string,
   *   tokenIdHex: string,
   *   issuedAt: number,
   *   expiresAt: number,
   *   ttlSecs: number,
   *   flags: number,
   *   issuerFingerprintHex: string,
   *   relayIdHex: string
   * }>}
   */
  async mintAdmissionToken(transcriptHashHex, options = {}) {
    const normalizedHash = normalizeHexString(transcriptHashHex, 32, "transcriptHashHex");
    const body = {
      transcript_hash_hex: normalizedHash,
    };
    if (options.ttlSecs !== undefined && options.ttlSecs !== null) {
      body.ttl_secs = coercePositiveInteger(options.ttlSecs, "options.ttlSecs");
    }
    if (options.flags !== undefined && options.flags !== null) {
      body.flags = coerceByte(options.flags, "options.flags");
    }
    if (options.issuedAtUnix !== undefined && options.issuedAtUnix !== null) {
      body.issued_at_unix = coerceNonNegativeInteger(
        options.issuedAtUnix,
        "options.issuedAtUnix",
      );
    }
    const payload = await this._request("POST", "/v1/token/mint", body, options);
    return normalizeTokenMintResponse(payload, "token mint response");
  }

  async _request(method, path, body, options = {}) {
    const url = buildUrl(this._baseUrl, path);
    const headers = { ...this._defaultHeaders };
    if (body !== undefined) {
      headers["Content-Type"] = "application/json";
    }
    if (!headers.Accept) {
      headers.Accept = "application/json";
    }
    if (options.headers) {
      for (const [key, value] of Object.entries(options.headers)) {
        if (value === undefined || value === null) {
          delete headers[key];
        } else {
          headers[key] = String(value);
        }
      }
    }
    const timeout =
      options.timeoutMs === undefined || options.timeoutMs === null
        ? this._timeoutMs
        : coerceNonNegativeNumber(options.timeoutMs, "options.timeoutMs");
    const init = {
      method: (method ?? "GET").toUpperCase(),
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
    };
    const response = await this._fetchWithTimeout(url, init, timeout, options.signal);
    if (response.status < 200 || response.status >= 300) {
      const text = await safeReadText(response);
      throw new SoranetPuzzleError(response.status, text);
    }
    if (response.status === 204) {
      return null;
    }
    return safeReadJson(response);
  }

  async _fetchWithTimeout(url, init, timeoutMs, externalSignal) {
    if (timeoutMs == null) {
      const finalInit =
        externalSignal == null ? init : { ...init, signal: externalSignal };
      return this._fetch(url, finalInit);
    }
    const abortController = new AbortController();
    const combinedSignal = combineAbortSignals(externalSignal, abortController.signal);
    const finalInit = { ...init, signal: combinedSignal };
    const timer = setTimeout(() => abortController.abort(), timeoutMs);
    try {
      return await this._fetch(url, finalInit);
    } finally {
      clearTimeout(timer);
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

function buildUrl(base, path) {
  if (!path) {
    return base;
  }
  if (/^https?:\/\//i.test(path)) {
    return path;
  }
  if (path.startsWith("/")) {
    return `${base}${path}`;
  }
  return `${base}/${path}`;
}

function ensureRecord(value, context) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return value;
}

function coerceBoolean(value, context) {
  if (typeof value === "boolean") {
    return value;
  }
  if (value === 0 || value === 1) {
    return Boolean(value);
  }
  throw new TypeError(`${context} must be a boolean`);
}

function coerceNonNegativeNumber(value, context) {
  if (typeof value !== "number" || Number.isNaN(value) || !Number.isFinite(value)) {
    throw new TypeError(`${context} must be a finite number`);
  }
  if (value < 0) {
    throw new TypeError(`${context} must be greater than or equal to zero`);
  }
  return value;
}

function coerceNonNegativeInteger(value, context) {
  if (typeof value === "bigint") {
    if (value < 0n) {
      throw new TypeError(`${context} must be a non-negative integer`);
    }
    if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new TypeError(`${context} exceeds maximum safe integer`);
    }
    return Number(value);
  }
  const number = coerceNonNegativeNumber(Number(value), context);
  if (!Number.isInteger(number)) {
    throw new TypeError(`${context} must be an integer`);
  }
  return number;
}

function coercePositiveInteger(value, context) {
  const number = coerceNonNegativeInteger(value, context);
  if (number <= 0) {
    throw new TypeError(`${context} must be greater than zero`);
  }
  return number;
}

function coerceByte(value, context) {
  const number = coerceNonNegativeInteger(value, context);
  if (number > 0xff) {
    throw new TypeError(`${context} must be between 0 and 255`);
  }
  return number;
}

function normalizeHexString(value, expectedBytes, context) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a string`);
  }
  const trimmed = value.trim();
  if (trimmed.length === 0) {
    throw new TypeError(`${context} must not be empty`);
  }
  const body = trimmed.startsWith("0x") || trimmed.startsWith("0X") ? trimmed.slice(2) : trimmed;
  if (!/^[0-9a-fA-F]+$/.test(body) || body.length % 2 !== 0) {
    throw new TypeError(`${context} must be an even-length hexadecimal string`);
  }
  if (expectedBytes != null && body.length !== expectedBytes * 2) {
    throw new TypeError(
      `${context} must decode to ${expectedBytes} bytes (got ${body.length / 2})`,
    );
  }
  return body.toLowerCase();
}

function normalizePuzzleConfig(payload, context) {
  const record = ensureRecord(payload, context);
  const puzzle =
    record.puzzle === null || record.puzzle === undefined
      ? null
      : normalizePuzzleParams(record.puzzle, `${context}.puzzle`);
  return {
    required: coerceBoolean(record.required, `${context}.required`),
    difficulty: coerceNonNegativeInteger(record.difficulty, `${context}.difficulty`),
    maxFutureSkewSecs: coerceNonNegativeInteger(
      record.max_future_skew_secs,
      `${context}.max_future_skew_secs`,
    ),
    minTicketTtlSecs: coerceNonNegativeInteger(
      record.min_ticket_ttl_secs,
      `${context}.min_ticket_ttl_secs`,
    ),
    ticketTtlSecs: coerceNonNegativeInteger(
      record.ticket_ttl_secs,
      `${context}.ticket_ttl_secs`,
    ),
    puzzle,
    token: normalizeTokenConfig(record.token, `${context}.token`),
  };
}

function normalizePuzzleParams(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    memoryKib: coercePositiveInteger(record.memory_kib, `${context}.memory_kib`),
    timeCost: coercePositiveInteger(record.time_cost, `${context}.time_cost`),
    lanes: coercePositiveInteger(record.lanes, `${context}.lanes`),
  };
}

function normalizeTokenConfig(payload, context) {
  const record = ensureRecord(payload, context);
  const enabled = coerceBoolean(record.enabled, `${context}.enabled`);
  const coerceOption = (value, _label) =>
    value === undefined || value === null ? null : String(value);
  const coerceNumberOption = (value, label) =>
    value === undefined || value === null ? null : coerceNonNegativeInteger(value, label);
  const ids = record.revocation_ids_hex || [];
  if (!Array.isArray(ids)) {
    throw new TypeError(`${context}.revocation_ids_hex must be an array`);
  }
  const revocationIdsHex = ids.map((entry, idx) =>
    normalizeHexString(entry, 32, `${context}.revocation_ids_hex[${idx}]`),
  );
  return {
    enabled,
    suite: coerceOption(record.suite, `${context}.suite`),
    relayIdHex: coerceOption(
      record.relay_id_hex,
      `${context}.relay_id_hex`,
    ),
    issuerFingerprintHex: coerceOption(
      record.issuer_fingerprint_hex,
      `${context}.issuer_fingerprint_hex`,
    ),
    maxTtlSecs: coerceNumberOption(
      record.max_ttl_secs,
      `${context}.max_ttl_secs`,
    ),
    minTtlSecs: coerceNumberOption(
      record.min_ttl_secs,
      `${context}.min_ttl_secs`,
    ),
    defaultTtlSecs: coerceNumberOption(
      record.default_ttl_secs,
      `${context}.default_ttl_secs`,
    ),
    clockSkewSecs: coerceNumberOption(
      record.clock_skew_secs,
      `${context}.clock_skew_secs`,
    ),
    revocationIdsHex,
  };
}

function normalizePuzzleMintResponse(payload, context) {
  const record = ensureRecord(payload, context);
  const signedB64Raw = record.signed_ticket_b64 ?? null;
  const signedTicketB64 =
    signedB64Raw === undefined || signedB64Raw === null
      ? null
      : requireNonEmptyString(signedB64Raw, `${context}.signed_ticket_b64`);
  const fingerprintRaw = record.signed_ticket_fingerprint_hex ?? null;
  const signedTicketFingerprintHex =
    fingerprintRaw === undefined || fingerprintRaw === null
      ? null
      : normalizeHexString(
          fingerprintRaw,
          32,
          `${context}.signed_ticket_fingerprint_hex`,
        );
  return {
    ticketB64: requireNonEmptyString(record.ticket_b64, `${context}.ticket_b64`),
    signedTicketB64,
    signedTicketFingerprintHex,
    difficulty: coerceNonNegativeInteger(record.difficulty, `${context}.difficulty`),
    ttlSecs: coerceNonNegativeInteger(record.ttl_secs, `${context}.ttl_secs`),
    expiresAt: coerceNonNegativeInteger(
      record.expires_at,
      `${context}.expires_at`,
    ),
  };
}

function normalizeTokenMintResponse(payload, context) {
  const record = ensureRecord(payload, context);
  return {
    tokenB64: requireNonEmptyString(record.token_b64, `${context}.token_b64`),
    tokenIdHex: normalizeHexString(
      record.token_id_hex,
      32,
      `${context}.token_id_hex`,
    ),
    issuedAt: coerceNonNegativeInteger(record.issued_at, `${context}.issued_at`),
    expiresAt: coerceNonNegativeInteger(
      record.expires_at,
      `${context}.expires_at`,
    ),
    ttlSecs: coerceNonNegativeInteger(record.ttl_secs, `${context}.ttl_secs`),
    flags: coerceByte(record.flags, `${context}.flags`),
    issuerFingerprintHex: normalizeHexString(
      record.issuer_fingerprint_hex,
      32,
      `${context}.issuer_fingerprint_hex`,
    ),
    relayIdHex: normalizeHexString(
      record.relay_id_hex,
      32,
      `${context}.relay_id_hex`,
    ),
  };
}

function requireNonEmptyString(value, context) {
  if (typeof value !== "string" || value.trim() === "") {
    throw new TypeError(`${context} must be a non-empty string`);
  }
  return value;
}

async function safeReadText(response) {
  try {
    return await response.text();
  } catch (error) {
    return `failed to read response body: ${error?.message ?? error}`;
  }
}

async function safeReadJson(response) {
  const text = await response.text();
  try {
    return text.length === 0 ? null : JSON.parse(text);
  } catch (error) {
    throw new TypeError(`failed to parse JSON response: ${error?.message ?? error}`);
  }
}

function combineAbortSignals(primary, secondary) {
  if (!primary) {
    return secondary;
  }
  if (!secondary) {
    return primary;
  }
  const controller = new AbortController();
  const abort = () => controller.abort();
  const listeners = [
    [primary, abort],
    [secondary, abort],
  ];
  for (const [signal, handler] of listeners) {
    signal.addEventListener("abort", handler, { once: true });
  }
  if (primary.aborted || secondary.aborted) {
    controller.abort();
  }
  return controller.signal;
}
