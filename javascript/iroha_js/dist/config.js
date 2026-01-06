import { normalizeAccountId } from "./instructionBuilders.js";

const DEFAULT_TORII_CLIENT_CONFIG = Object.freeze({
  timeoutMs: 30_000,
  maxRetries: 3,
  backoffInitialMs: 500,
  backoffMultiplier: 2,
  maxBackoffMs: 5_000,
  retryStatuses: Object.freeze([429, 502, 503, 504]),
  retryMethods: Object.freeze(["GET", "HEAD", "OPTIONS"]),
  defaultHeaders: Object.freeze({ Accept: "application/json" }),
  authToken: null,
  apiToken: null,
  retryTelemetryHook: null,
  insecureTransportTelemetryHook: null,
});

const DEFAULT_RETRY_PROFILE_PIPELINE = Object.freeze({
  maxRetries: 5,
  backoffInitialMs: 250,
  backoffMultiplier: 1.8,
  maxBackoffMs: 8_000,
  retryStatuses: Object.freeze([408, 425, 429, 500, 502, 503, 504]),
  retryMethods: Object.freeze(["GET", "POST", "HEAD"]),
});

const DEFAULT_RETRY_PROFILE_STREAMING = Object.freeze({
  maxRetries: 6,
  backoffInitialMs: 500,
  backoffMultiplier: 1.5,
  maxBackoffMs: 12_000,
  retryStatuses: Object.freeze([408, 425, 429, 500, 502, 503, 504]),
  retryMethods: Object.freeze(["GET"]),
});

const ENV_KEYS = Object.freeze({
  timeoutMs: "IROHA_TORII_TIMEOUT_MS",
  maxRetries: "IROHA_TORII_MAX_RETRIES",
  backoffInitialMs: "IROHA_TORII_BACKOFF_INITIAL_MS",
  backoffMultiplier: "IROHA_TORII_BACKOFF_MULTIPLIER",
  maxBackoffMs: "IROHA_TORII_MAX_BACKOFF_MS",
  retryStatuses: "IROHA_TORII_RETRY_STATUSES",
  retryMethods: "IROHA_TORII_RETRY_METHODS",
  apiToken: "IROHA_TORII_API_TOKEN",
  authToken: "IROHA_TORII_AUTH_TOKEN",
});

const defaultEnv =
  typeof process !== "undefined" && process && typeof process.env === "object"
    ? process.env
    : {};

function coerceNumber(value, name) {
  if (value === null || value === undefined || value === "") {
    return null;
  }
  const coerced = Number(value);
  if (!Number.isFinite(coerced)) {
    throw new TypeError(`${name} must be a finite number`);
  }
  return coerced;
}

function coercePositiveInt(value, name, allowZero = false) {
  const number = coerceNumber(value, name);
  if (number === null) {
    return null;
  }
  if (!Number.isInteger(number)) {
    throw new TypeError(`${name} must be an integer`);
  }
  if (!Number.isSafeInteger(number)) {
    throw new RangeError(`${name} must be a safe integer`);
  }
  if (number < 0 || (!allowZero && number === 0)) {
    throw new TypeError(`${name} must be ${allowZero ? "non-negative" : "positive"}`);
  }
  return number;
}

function toSetFromIterable(iterable, transform) {
  const set = new Set();
  for (const entry of iterable || []) {
    set.add(transform(entry));
  }
  return set;
}

function parseList(value, transform) {
  if (value === null || value === undefined) {
    return [];
  }
  if (typeof value === "string") {
    if (value.trim().length === 0) {
      return [];
    }
    return value
      .split(",")
      .map((entry) => transform(entry.trim()))
      .filter((entry) => entry !== null && entry !== undefined);
  }
  if (Array.isArray(value)) {
    return value.map(transform);
  }
  if (typeof value[Symbol.iterator] === "function") {
    const result = [];
    for (const entry of value) {
      const transformed = transform(entry);
      if (transformed !== null && transformed !== undefined) {
        result.push(transformed);
      }
    }
    return result;
  }
  return [transform(value)];
}

function normalizeHeaders(headers) {
  const normalized = {};
  if (headers && typeof headers === "object") {
    for (const [key, value] of Object.entries(headers)) {
      if (typeof value === "undefined" || value === null) {
        continue;
      }
      normalized[String(key)] = String(value);
    }
  }
  if (!Object.keys(normalized).some((key) => key.toLowerCase() === "accept")) {
    normalized.Accept = "application/json";
  }
  return normalized;
}

function extractToriiClientSource(config) {
  if (!config || typeof config !== "object") {
    return {};
  }
  if (config.toriiClient && typeof config.toriiClient === "object") {
    return config.toriiClient;
  }
  return config;
}

function pickApiToken(config) {
  if (!config || typeof config !== "object") {
    return null;
  }
  const torii = config.torii;
  if (!torii || typeof torii !== "object") {
    return null;
  }
  const tokens = torii.apiTokens;
  if (Array.isArray(tokens) && tokens.length === 1) {
    return String(tokens[0]);
  }
  return null;
}

function cloneRetryProfileFromConfig(config) {
  return {
    maxRetries: config.maxRetries,
    backoffInitialMs: config.backoffInitialMs,
    backoffMultiplier: config.backoffMultiplier,
    maxBackoffMs: config.maxBackoffMs,
    retryStatuses: new Set(config.retryStatuses),
    retryMethods: new Set([...config.retryMethods].map((method) => method.toUpperCase())),
  };
}

function applyRetryProfilePatch(target, patch) {
  if (!patch || typeof patch !== "object") {
    return target;
  }
  const patchMaxRetries = coercePositiveInt(
    patch.maxRetries,
    "retryProfiles.maxRetries",
    true,
  );
  if (patchMaxRetries !== null) {
    target.maxRetries = patchMaxRetries;
  }
  const patchBackoffInitial = coercePositiveInt(
    patch.backoffInitialMs,
    "retryProfiles.backoffInitialMs",
    true,
  );
  if (patchBackoffInitial !== null) {
    target.backoffInitialMs = patchBackoffInitial;
  }
  const patchBackoffMultiplier = coerceNumber(
    patch.backoffMultiplier,
    "retryProfiles.backoffMultiplier",
  );
  if (patchBackoffMultiplier !== null && patchBackoffMultiplier >= 1) {
    target.backoffMultiplier = patchBackoffMultiplier;
  }
  const patchMaxBackoff = coercePositiveInt(
    patch.maxBackoffMs,
    "retryProfiles.maxBackoffMs",
    true,
  );
  if (patchMaxBackoff !== null) {
    target.maxBackoffMs = patchMaxBackoff;
  }
  const patchRetryStatuses = patch.retryStatuses;
  if (patchRetryStatuses !== undefined) {
    target.retryStatuses = toSetFromIterable(
      parseList(patchRetryStatuses, (value) =>
        coercePositiveInt(value, "retryProfiles.retryStatuses", true),
      ),
      (entry) => entry,
    );
  }
  const patchRetryMethods = patch.retryMethods;
  if (patchRetryMethods !== undefined) {
    target.retryMethods = toSetFromIterable(
      parseList(patchRetryMethods, (value) => (value ? String(value).toUpperCase() : null)),
      (entry) => entry,
    );
  }
  return target;
}

/**
 * Resolve Torii client configuration by merging defaults, a config object,
 * environment overrides, and explicit options.
 * @param {{
 *   config?: Record<string, unknown>;
 *   env?: Record<string, string | undefined>;
 *   overrides?: Record<string, unknown>;
 * }} [input]
 * @returns {{
 *   timeoutMs: number;
 *   maxRetries: number;
 *   backoffInitialMs: number;
 *   backoffMultiplier: number;
 *   maxBackoffMs: number;
 *   retryStatuses: Set<number>;
 *   retryMethods: Set<string>;
 *   defaultHeaders: Record<string, string>;
 *   authToken: string | null;
 *   apiToken: string | null;
 *   retryProfiles: Record<string, {
 *     maxRetries: number;
 *     backoffInitialMs: number;
 *     backoffMultiplier: number;
 *     maxBackoffMs: number;
 *     retryStatuses: Set<number>;
 *     retryMethods: Set<string>;
 *   }>;
 * }}
 */
export function resolveToriiClientConfig(input = {}) {
  const { config, env = defaultEnv, overrides = {} } = input;
  const result = {
    timeoutMs: DEFAULT_TORII_CLIENT_CONFIG.timeoutMs,
    maxRetries: DEFAULT_TORII_CLIENT_CONFIG.maxRetries,
    backoffInitialMs: DEFAULT_TORII_CLIENT_CONFIG.backoffInitialMs,
    backoffMultiplier: DEFAULT_TORII_CLIENT_CONFIG.backoffMultiplier,
    maxBackoffMs: DEFAULT_TORII_CLIENT_CONFIG.maxBackoffMs,
    retryStatuses: new Set(DEFAULT_TORII_CLIENT_CONFIG.retryStatuses),
    retryMethods: new Set(DEFAULT_TORII_CLIENT_CONFIG.retryMethods.map((method) => method.toUpperCase())),
    defaultHeaders: { ...DEFAULT_TORII_CLIENT_CONFIG.defaultHeaders },
    authToken: DEFAULT_TORII_CLIENT_CONFIG.authToken,
    apiToken: pickApiToken(config),
    retryTelemetryHook: DEFAULT_TORII_CLIENT_CONFIG.retryTelemetryHook,
    insecureTransportTelemetryHook: DEFAULT_TORII_CLIENT_CONFIG.insecureTransportTelemetryHook,
  };

  const sources = [extractToriiClientSource(config), overrides];
  const profileOverrides = new Map();

  const collectProfileOverrides = (container) => {
    if (!container || typeof container !== "object") {
      return;
    }
    const rawProfiles = container.retryProfiles;
    if (!rawProfiles || typeof rawProfiles !== "object") {
      return;
    }
    for (const [profileName, patch] of Object.entries(rawProfiles)) {
      if (!profileName || !patch || typeof patch !== "object") {
        continue;
      }
      const previous = profileOverrides.get(profileName) ?? {};
      profileOverrides.set(profileName, { ...previous, ...patch });
    }
  };

  for (const source of sources) {
    if (!source || typeof source !== "object") {
      continue;
    }
    collectProfileOverrides(source);
    const timeout = coercePositiveInt(
      source.timeoutMs,
      "timeoutMs",
      true,
    );
    if (timeout !== null) {
      result.timeoutMs = timeout;
    }
    const retries = coercePositiveInt(
      source.maxRetries,
      "maxRetries",
      true,
    );
    if (retries !== null) {
      result.maxRetries = retries;
    }
    const backoffInitial = coercePositiveInt(
      source.backoffInitialMs,
      "backoffInitialMs",
      true,
    );
    if (backoffInitial !== null) {
      result.backoffInitialMs = backoffInitial;
    }
    const backoffMultiplier = coerceNumber(
      source.backoffMultiplier,
      "backoffMultiplier",
    );
    if (backoffMultiplier !== null && backoffMultiplier >= 1) {
      result.backoffMultiplier = backoffMultiplier;
    }
    const maxBackoff = coercePositiveInt(
      source.maxBackoffMs,
      "maxBackoffMs",
      true,
    );
    if (maxBackoff !== null) {
      result.maxBackoffMs = maxBackoff;
    }
    const retryStatuses = source.retryStatuses;
    if (retryStatuses !== undefined) {
      result.retryStatuses = toSetFromIterable(
        parseList(retryStatuses, (value) => coercePositiveInt(value, "retryStatuses", true)),
        (entry) => entry,
      );
    }
    const retryMethods = source.retryMethods;
    if (retryMethods !== undefined) {
      result.retryMethods = toSetFromIterable(
        parseList(retryMethods, (value) => (value ? String(value).toUpperCase() : null)),
        (entry) => entry,
      );
    }
    if (source.defaultHeaders) {
      const headers = normalizeHeaders(source.defaultHeaders);
      result.defaultHeaders = headers;
    }
    if (source.authToken) {
      result.authToken = String(source.authToken);
    }
    if (source.apiToken) {
      result.apiToken = String(source.apiToken);
    }
    if (typeof source.retryTelemetryHook === "function") {
      result.retryTelemetryHook = source.retryTelemetryHook;
    }
    if (typeof source.insecureTransportTelemetryHook === "function") {
      result.insecureTransportTelemetryHook = source.insecureTransportTelemetryHook;
    }
  }

  if (ENV_KEYS.timeoutMs in env && env[ENV_KEYS.timeoutMs]) {
    const timeout = coercePositiveInt(env[ENV_KEYS.timeoutMs], "IROHA_TORII_TIMEOUT_MS", true);
    if (timeout !== null) {
      result.timeoutMs = timeout;
    }
  }
  if (ENV_KEYS.maxRetries in env && env[ENV_KEYS.maxRetries]) {
    const retries = coercePositiveInt(env[ENV_KEYS.maxRetries], "IROHA_TORII_MAX_RETRIES", true);
    if (retries !== null) {
      result.maxRetries = retries;
    }
  }
  if (ENV_KEYS.backoffInitialMs in env && env[ENV_KEYS.backoffInitialMs]) {
    const backoffInitial = coercePositiveInt(
      env[ENV_KEYS.backoffInitialMs],
      "IROHA_TORII_BACKOFF_INITIAL_MS",
      true,
    );
    if (backoffInitial !== null) {
      result.backoffInitialMs = backoffInitial;
    }
  }
  if (ENV_KEYS.backoffMultiplier in env && env[ENV_KEYS.backoffMultiplier]) {
    const backoffMultiplier = coerceNumber(
      env[ENV_KEYS.backoffMultiplier],
      "IROHA_TORII_BACKOFF_MULTIPLIER",
    );
    if (backoffMultiplier !== null && backoffMultiplier >= 1) {
      result.backoffMultiplier = backoffMultiplier;
    }
  }
  if (ENV_KEYS.maxBackoffMs in env && env[ENV_KEYS.maxBackoffMs]) {
    const maxBackoff = coercePositiveInt(
      env[ENV_KEYS.maxBackoffMs],
      "IROHA_TORII_MAX_BACKOFF_MS",
      true,
    );
    if (maxBackoff !== null) {
      result.maxBackoffMs = maxBackoff;
    }
  }
  if (ENV_KEYS.retryStatuses in env && env[ENV_KEYS.retryStatuses]) {
    result.retryStatuses = toSetFromIterable(
      parseList(env[ENV_KEYS.retryStatuses], (value) =>
        coercePositiveInt(value, "IROHA_TORII_RETRY_STATUSES", true),
      ),
      (entry) => entry,
    );
  }
  if (ENV_KEYS.retryMethods in env && env[ENV_KEYS.retryMethods]) {
    result.retryMethods = toSetFromIterable(
      parseList(env[ENV_KEYS.retryMethods], (value) =>
        value ? String(value).toUpperCase() : null,
      ),
      (entry) => entry,
    );
  }
  if (ENV_KEYS.apiToken in env && env[ENV_KEYS.apiToken]) {
    result.apiToken = env[ENV_KEYS.apiToken] || null;
  }
  if (ENV_KEYS.authToken in env && env[ENV_KEYS.authToken]) {
    result.authToken = env[ENV_KEYS.authToken] || null;
  }

  const defaultOverride = profileOverrides.get("default");
  if (defaultOverride) {
    profileOverrides.delete("default");
  }
  const defaultProfile = cloneRetryProfileFromConfig(result);
  applyRetryProfilePatch(defaultProfile, defaultOverride);
  const resolvedProfiles = Object.create(null);
  resolvedProfiles.default = defaultProfile;

  const buildProfile = (name, template) => {
    const profile = cloneRetryProfileFromConfig(resolvedProfiles.default);
    if (template) {
      applyRetryProfilePatch(profile, template);
    }
    const override = profileOverrides.get(name);
    if (override) {
      applyRetryProfilePatch(profile, override);
      profileOverrides.delete(name);
    }
    resolvedProfiles[name] = profile;
  };

  buildProfile("pipeline", DEFAULT_RETRY_PROFILE_PIPELINE);
  buildProfile("streaming", DEFAULT_RETRY_PROFILE_STREAMING);

  for (const [name, override] of profileOverrides.entries()) {
    const profile = cloneRetryProfileFromConfig(resolvedProfiles.default);
    applyRetryProfilePatch(profile, override);
    resolvedProfiles[name] = profile;
  }

  result.retryProfiles = resolvedProfiles;

  return result;
}

export {
  DEFAULT_TORII_CLIENT_CONFIG,
  DEFAULT_RETRY_PROFILE_PIPELINE,
  DEFAULT_RETRY_PROFILE_STREAMING,
};

/**
 * Extract feature configuration snapshots (ISO bridge, RBC sampling, Connect)
 * from an `iroha_config`-like object.
 * @param {{ config?: Record<string, unknown> } & Record<string, unknown>} [input]
 * @returns {{
 *   isoBridge: {
 *     enabled: boolean;
 *     dedupeTtlSecs: number;
 *     signer: { accountId: string; privateKey?: string | null } | null;
 *     accountAliases: Array<{ iban: string; accountId: string }>;
 *     currencyAssets: Array<{ currency: string; assetDefinition: string }>;
 *   } | null;
 *   rbcSampling: {
 *     enabled: boolean;
 *     maxSamplesPerRequest: number;
 *     maxBytesPerRequest: number;
 *     dailyByteBudget: number;
 *     ratePerMinute: number | null;
 *   } | null;
 *   connect: {
 *     enabled: boolean;
 *     wsMaxSessions: number;
 *     wsPerIpMaxSessions: number;
 *     wsRatePerIpPerMin: number;
 *     sessionTtlMs: number;
 *     frameMaxBytes: number;
 *     sessionBufferMaxBytes: number;
 *     dedupeTtlMs: number;
 *     dedupeCap: number;
 *     relayEnabled: boolean;
 *     relayStrategy: string;
 *     p2pTtlHops: number;
 *   } | null;
 * }}
 */
export function extractToriiFeatureConfig(input = {}) {
  const sourceRoot = getObject(input.config) ?? getObject(input);
  const toriiSource = getObject(sourceRoot?.torii);
  const connectSource = getObject(sourceRoot?.connect);
  return {
    isoBridge: normalizeIsoBridgeConfig(
      getObject(toriiSource?.iso_bridge),
    ),
    rbcSampling: normalizeRbcSamplingConfig(
      getObject(toriiSource?.rbc_sampling),
    ),
    connect: normalizeConnectConfig(connectSource),
  };
}

/**
 * Extract confidential gas schedule from an `iroha_config`-like object.
 * @param {{ config?: Record<string, unknown> } & Record<string, unknown>} [input]
 * @returns {{
 *   proofBase: number;
 *   perPublicInput: number;
 *   perProofByte: number;
 *   perNullifier: number;
 *   perCommitment: number;
 * } | null}
 */
export function extractConfidentialGasConfig(input = {}) {
  const sourceRoot = getObject(input.config) ?? getObject(input);
  const confidentialRoot = getObject(sourceRoot?.confidential) ?? null;
  let gasSection = confidentialRoot ? getObject(confidentialRoot.gas) : null;
  if (!gasSection) {
    gasSection = getObject(sourceRoot?.confidential_gas);
  }
  if (!gasSection) {
    return null;
  }
  const proofBase = coerceOptionalNumber(
    gasSection.proof_base,
    "confidential.gas.proof_base",
  );
  const perPublicInput = coerceOptionalNumber(
    gasSection.per_public_input,
    "confidential.gas.per_public_input",
  );
  const perProofByte = coerceOptionalNumber(
    gasSection.per_proof_byte,
    "confidential.gas.per_proof_byte",
  );
  const perNullifier = coerceOptionalNumber(
    gasSection.per_nullifier,
    "confidential.gas.per_nullifier",
  );
  const perCommitment = coerceOptionalNumber(
    gasSection.per_commitment,
    "confidential.gas.per_commitment",
  );
  if (
    proofBase === null ||
    perPublicInput === null ||
    perProofByte === null ||
    perNullifier === null ||
    perCommitment === null
  ) {
    return null;
  }
  return {
    proofBase,
    perPublicInput,
    perProofByte,
    perNullifier,
    perCommitment,
  };
}

function normalizeIsoBridgeConfig(section) {
  if (!section) {
    return null;
  }
  const accountAliases = Array.isArray(section.account_aliases)
    ? section.account_aliases
        .map((entry, index) => {
          const obj = getObject(entry);
          if (!obj) {
            return null;
          }
          const iban = requireString(obj.iban);
          const accountIdRaw = requireString(obj.account_id);
          if (!iban || !accountIdRaw) {
            return null;
          }
          return {
            iban,
            accountId: normalizeAccountId(
              accountIdRaw,
              `isoBridge.accountAliases[${index}].accountId`,
            ),
          };
        })
        .filter(Boolean)
    : [];
  const currencyAssets = Array.isArray(section.currency_assets)
    ? section.currency_assets
        .map((entry) => {
          const obj = getObject(entry);
          if (!obj) {
            return null;
          }
          const currency = requireString(obj.currency);
          const assetDefinition = requireString(obj.asset_definition);
          if (!currency || !assetDefinition) {
            return null;
          }
          return { currency, assetDefinition };
        })
        .filter(Boolean)
    : [];
  return {
    enabled: Boolean(section.enabled),
    dedupeTtlSecs: coerceNumberWithDefault(
      section.dedupe_ttl_secs,
      "IsoBridge.dedupe_ttl_secs",
      0,
    ),
    signer: normalizeIsoBridgeSigner(section.signer),
    accountAliases,
    currencyAssets,
  };
}

function normalizeIsoBridgeSigner(raw) {
  const signer = getObject(raw);
  if (!signer) {
    return null;
  }
  const accountIdSource = signer.account_id;
  if (!accountIdSource) {
    return null;
  }
  const accountId = normalizeAccountId(
    requireString(accountIdSource, "isoBridge.signer.accountId"),
    "isoBridge.signer.accountId",
  );
  const snapshot = { accountId };
  if (signer.private_key) {
    snapshot.privateKey = String(signer.private_key);
  }
  return snapshot;
}

function normalizeRbcSamplingConfig(section) {
  if (!section) {
    return null;
  }
  return {
    enabled: Boolean(section.enabled),
    maxSamplesPerRequest: coerceNumberWithDefault(
      section.max_samples_per_request,
      "RbcSampling.max_samples_per_request",
      0,
    ),
    maxBytesPerRequest: coerceNumberWithDefault(
      section.max_bytes_per_request,
      "RbcSampling.max_bytes_per_request",
      0,
    ),
    dailyByteBudget: coerceNumberWithDefault(
      section.daily_byte_budget,
      "RbcSampling.daily_byte_budget",
      0,
    ),
    ratePerMinute: coerceOptionalNumber(
      section.rate_per_minute,
      "RbcSampling.rate_per_minute",
    ),
  };
}

function normalizeConnectConfig(section) {
  if (!section) {
    return null;
  }
  return {
    enabled: Boolean(section.enabled),
    wsMaxSessions: coerceNumberWithDefault(
      section.ws_max_sessions,
      "connect.ws_max_sessions",
      0,
    ),
    wsPerIpMaxSessions: coerceNumberWithDefault(
      section.ws_per_ip_max_sessions,
      "connect.ws_per_ip_max_sessions",
      0,
    ),
    wsRatePerIpPerMin: coerceNumberWithDefault(
      section.ws_rate_per_ip_per_min,
      "connect.ws_rate_per_ip_per_min",
      0,
    ),
    sessionTtlMs: coerceNumberWithDefault(
      section.session_ttl_ms,
      "connect.session_ttl_ms",
      0,
    ),
    frameMaxBytes: coerceNumberWithDefault(
      section.frame_max_bytes,
      "connect.frame_max_bytes",
      0,
    ),
    sessionBufferMaxBytes: coerceNumberWithDefault(
      section.session_buffer_max_bytes,
      "connect.session_buffer_max_bytes",
      0,
    ),
    pingIntervalMs: coerceNumberWithDefault(
      section.ping_interval_ms,
      "connect.ping_interval_ms",
      0,
    ),
    pingMissTolerance: coerceNumberWithDefault(
      section.ping_miss_tolerance,
      "connect.ping_miss_tolerance",
      0,
    ),
    pingMinIntervalMs: coerceNumberWithDefault(
      section.ping_min_interval_ms,
      "connect.ping_min_interval_ms",
      0,
    ),
    dedupeTtlMs: coerceNumberWithDefault(
      section.dedupe_ttl_ms,
      "connect.dedupe_ttl_ms",
      0,
    ),
    dedupeCap: coerceNumberWithDefault(
      section.dedupe_cap,
      "connect.dedupe_cap",
      0,
    ),
    relayEnabled: Boolean(section.relay_enabled),
    relayStrategy: String(section.relay_strategy ?? ""),
    p2pTtlHops: coerceNumberWithDefault(
      section.p2p_ttl_hops,
      "connect.p2p_ttl_hops",
      0,
    ),
  };
}

function getObject(value) {
  if (!value || typeof value !== "object") {
    return null;
  }
  return value;
}

function requireString(value, _name) {
  if (value === null || value === undefined) {
    return null;
  }
  const trimmed = String(value).trim();
  if (!trimmed) {
    return null;
  }
  return trimmed;
}

function coerceNumberWithDefault(value, name, fallback) {
  if (value === null || value === undefined || value === "") {
    return fallback;
  }
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) {
    throw new TypeError(`${name} must be a finite number`);
  }
  return numeric;
}

function coerceOptionalNumber(value, name) {
  if (value === null || value === undefined || value === "") {
    return null;
  }
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) {
    throw new TypeError(`${name} must be a finite number`);
  }
  return numeric;
}
