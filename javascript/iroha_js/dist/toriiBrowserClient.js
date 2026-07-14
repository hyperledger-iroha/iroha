import { NumericV1, NumericV1Error } from "./numericV1.js";
import {
  normalizeKagemushaAssetSelector,
  normalizeKagemushaOperationId,
  normalizeKagemushaOperationReference,
  normalizeKagemushaOperationStatus,
  normalizeKagemushaRedeemRequestV4,
  normalizeKagemushaReadinessV4,
  normalizeKagemushaTopUpRequestV4,
  requireKagemushaJsonContentType,
} from "./kagemushaOffline.js";

const DEFAULT_SUCCESS_STATUSES = [200];
const MULTISIG_PROPOSAL_STATUS_VALUES = new Set([
  "COLLECTING_SIGNATURES",
  "FINALIZED",
  "CANCELED",
  "EXPIRED",
]);

let noritoEncodersPromise;

function loadNoritoEncoders() {
  if (!noritoEncodersPromise) {
    noritoEncodersPromise = import("./norito.js");
  }
  return noritoEncodersPromise;
}

function normalizeBaseUrl(baseUrl) {
  const raw = String(baseUrl ?? "").trim();
  if (!raw) {
    throw new TypeError("ToriiBrowserClient baseUrl must be a non-empty URL");
  }
  return raw.replace(/\/+$/, "").replace(/\/v1\/explorer$/i, "").replace(/\/v1$/i, "");
}

function appendSearchParams(url, params) {
  if (!params) return;
  for (const [key, value] of Object.entries(params)) {
    if (value === undefined || value === null) continue;
    if (typeof value === "string" && value.trim() === "") continue;
    url.searchParams.set(key, String(value));
  }
}

function requireObject(value, context) {
  if (value === undefined || value === null) return {};
  if (typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return value;
}

function isPlainObject(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function requireNonEmptyString(value, context) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a string`);
  }
  const trimmed = value.trim();
  if (!trimmed) {
    throw new TypeError(`${context} must not be empty`);
  }
  return trimmed;
}

function requireCanonicalQuantity(value, context) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a canonical Kotodama V1 quantity string`);
  }
  try {
    return NumericV1.decodeQuantityJson(value).toString();
  } catch (error) {
    if (!(error instanceof NumericV1Error)) throw error;
    throw new TypeError(
      `${context} must be a canonical non-negative Kotodama V1 quantity (${error.code})`,
    );
  }
}

function normalizeQuantityRecord(value, context, fields, { optional = false } = {}) {
  const record = requireObject(value, context);
  const normalized = { ...record };
  for (const field of fields) {
    if (normalized[field] === undefined || normalized[field] === null) {
      if (!optional) {
        throw new TypeError(`${context}.${field} must be a canonical Kotodama V1 quantity string`);
      }
    } else {
      normalized[field] = requireCanonicalQuantity(
        normalized[field],
        `${context}.${field}`,
      );
    }
  }
  return normalized;
}

function normalizeQuantityPage(value, context, fields, options) {
  const page = requireObject(value, context);
  if (!Array.isArray(page.items)) {
    throw new TypeError(`${context}.items must be an array`);
  }
  return {
    ...page,
    items: page.items.map((item, index) =>
      normalizeQuantityRecord(item, `${context}.items[${index}]`, fields, options),
    ),
  };
}

function normalizePositiveInteger(value, context, fallback) {
  if (value === undefined || value === null) return fallback;
  const numeric = Number(value);
  if (!Number.isSafeInteger(numeric) || numeric < 1) {
    throw new TypeError(`${context} must be a positive safe integer`);
  }
  return numeric;
}

function normalizeOffset(value, context, fallback = 0) {
  if (value === undefined || value === null) return fallback;
  const numeric = Number(value);
  if (!Number.isSafeInteger(numeric) || numeric < 0) {
    throw new TypeError(`${context} must be a non-negative safe integer`);
  }
  return numeric;
}

function normalizeBoolean(value, context) {
  if (typeof value !== "boolean") {
    throw new TypeError(`${context} must be a boolean`);
  }
  return value;
}

function normalizeExplorerPagination(options, context) {
  const page = normalizePositiveInteger(options.page, `${context}.page`, 1);
  const perPage = normalizePositiveInteger(
    options.perPage ?? options.per_page,
    `${context}.perPage`,
    25,
  );
  return { page, per_page: perPage };
}

function normalizeIterablePagination(options, context) {
  const params = {};
  if (options.limit !== undefined && options.limit !== null) {
    params.limit = normalizePositiveInteger(options.limit, `${context}.limit`, undefined);
  }
  if (options.offset !== undefined && options.offset !== null) {
    params.offset = normalizeOffset(options.offset, `${context}.offset`);
  }
  return params;
}

function normalizeTransactionQuerySort(sort) {
  if (sort === undefined || sort === null) {
    return [];
  }
  if (typeof sort === "string") {
    const normalized = sort.trim().toLowerCase();
    if (normalized === "newest") {
      return [
        { key: "timestamp_ms", order: "desc" },
        { key: "entrypoint_hash", order: "desc" },
      ];
    }
    if (normalized === "oldest") {
      return [
        { key: "timestamp_ms", order: "asc" },
        { key: "entrypoint_hash", order: "asc" },
      ];
    }
    return normalized
      .split(",")
      .map((token) => token.trim())
      .filter(Boolean)
      .map((token) => {
        const parts = token.split(":");
        if (parts.length > 2) {
          throw new TypeError("sort entries must use key or key:asc/key:desc form");
        }
        const [key, order = "asc"] = parts;
        return {
          key: normalizeQueryFieldName(requireNonEmptyString(key, "sort key"), "sort key"),
          order: normalizeSortOrder(order, "sort order"),
        };
      });
  }
  if (Array.isArray(sort)) {
    return sort.map((entry, index) => {
      const item = requireObject(entry, `sort[${index}]`);
      return {
        key: normalizeQueryFieldName(requireNonEmptyString(item.key, `sort[${index}].key`), `sort[${index}].key`),
        order: normalizeSortOrder(item.order ?? "asc", `sort[${index}].order`),
      };
    });
  }
  throw new TypeError("sort must be a string or array");
}

function normalizeQueryFieldName(value, context) {
  const field = requireNonEmptyString(value, context);
  if (!/^[A-Za-z_][A-Za-z0-9_.-]*$/.test(field)) {
    throw new TypeError(`${context} must be an ASCII field name`);
  }
  return field;
}

function normalizeSortOrder(value, context) {
  const order = requireNonEmptyString(String(value ?? ""), context).toLowerCase();
  if (order !== "asc" && order !== "desc") {
    throw new TypeError(`${context} must be asc or desc`);
  }
  return order;
}

function normalizeCountMode(value, context) {
  if (value === undefined || value === null) {
    return undefined;
  }
  const mode = requireNonEmptyString(String(value), context).toLowerCase();
  if (mode !== "bounded" && mode !== "exact") {
    throw new TypeError(`${context} must be bounded or exact`);
  }
  return mode;
}

function normalizeSelectEntry(entry, context) {
  if (typeof entry === "string") {
    const fieldPath = entry.trim();
    if (!fieldPath) {
      throw new TypeError(`${context} must be a non-empty field path`);
    }
    return fieldPath;
  }
  if (isPlainObject(entry)) {
    return entry;
  }
  throw new TypeError(`${context} must be a field-path string or plain object`);
}

function transactionFilter(op, field, value) {
  return { op, args: [field, value] };
}

function normalizeTransactionQueryEnvelope(options, context) {
  const opts = requireObject(options, `${context} options`);
  const pagination = normalizeIterablePagination(opts, `${context} options`);
  const filters = [];
  if (opts.filter !== undefined && opts.filter !== null) {
    filters.push(requireObject(opts.filter, `${context}.filter`));
  }
  if (opts.assetId !== undefined && opts.assetId !== null) {
    filters.push(transactionFilter("eq", "asset_id", requireNonEmptyString(opts.assetId, "assetId")));
  }
  if (opts.authority !== undefined && opts.authority !== null) {
    filters.push(transactionFilter("eq", "authority", requireNonEmptyString(opts.authority, "authority")));
  }
  if (opts.resultOk !== undefined && opts.resultOk !== null) {
    filters.push(transactionFilter("eq", "result_ok", normalizeBoolean(opts.resultOk, "resultOk")));
  }
  if (opts.sinceTimestampMs !== undefined && opts.sinceTimestampMs !== null) {
    filters.push(transactionFilter("gte", "timestamp_ms", normalizeOffset(opts.sinceTimestampMs, "sinceTimestampMs")));
  }
  if (opts.untilTimestampMs !== undefined && opts.untilTimestampMs !== null) {
    filters.push(transactionFilter("lte", "timestamp_ms", normalizeOffset(opts.untilTimestampMs, "untilTimestampMs")));
  }
  const envelope = {
    pagination,
    sort: normalizeTransactionQuerySort(opts.sort),
  };
  if (filters.length === 1) {
    envelope.filter = filters[0];
  } else if (filters.length > 1) {
    envelope.filter = { op: "and", args: filters };
  }
  if (opts.fetch_size !== undefined && opts.fetch_size !== null) {
    envelope.fetch_size = normalizePositiveInteger(opts.fetch_size, "fetch_size", undefined);
  }
  const countMode = normalizeCountMode(opts.countMode ?? opts.count_mode, "countMode");
  if (countMode !== undefined) {
    envelope.count_mode = countMode;
  }
  const queryName = opts.queryName ?? opts.query_name;
  if (queryName !== undefined && queryName !== null) {
    envelope.query = requireNonEmptyString(queryName, "queryName");
  }
  if (opts.select !== undefined && opts.select !== null) {
    if (!Array.isArray(opts.select)) {
      throw new TypeError("select must be an array");
    }
    envelope.select = opts.select.map((entry, index) =>
      normalizeSelectEntry(entry, `select[${index}]`),
    );
  }
  return envelope;
}

function signalFrom(options) {
  return options.signal === undefined ? undefined : options.signal;
}

function kagemushaOptions(options, context) {
  const item = requireObject(options, context);
  const unknown = Object.keys(item).filter((key) => key !== "signal");
  if (unknown.length > 0) {
    throw new TypeError(`${context} contains unsupported option ${unknown[0]}`);
  }
  return item;
}

function copyRequestFields(source) {
  const body = { ...source };
  delete body.signal;
  delete body.headers;
  delete body.successStatuses;
  return body;
}

function normalizeMultisigSelectorBody(value, context) {
  const source = requireObject(value, context);
  const body = copyRequestFields(source);
  if (
    source.multisigAccountId !== undefined &&
    body.multisig_account_id !== undefined
  ) {
    throw new TypeError(`${context} must not duplicate multisigAccountId`);
  }
  if (
    source.multisigAccountAlias !== undefined &&
    body.multisig_account_alias !== undefined
  ) {
    throw new TypeError(`${context} must not duplicate multisigAccountAlias`);
  }
  if (source.multisigAccountId !== undefined && body.multisig_account_id === undefined) {
    body.multisig_account_id = requireNonEmptyString(
      source.multisigAccountId,
      `${context}.multisigAccountId`,
    );
  }
  if (source.multisigAccountAlias !== undefined && body.multisig_account_alias === undefined) {
    body.multisig_account_alias = requireNonEmptyString(
      source.multisigAccountAlias,
      `${context}.multisigAccountAlias`,
    );
  }
  delete body.multisigAccountId;
  delete body.multisigAccountAlias;
  if (body.multisig_account_id !== undefined) {
    body.multisig_account_id = requireNonEmptyString(
      body.multisig_account_id,
      `${context}.multisig_account_id`,
    );
  }
  if (body.multisig_account_alias !== undefined) {
    body.multisig_account_alias = requireNonEmptyString(
      body.multisig_account_alias,
      `${context}.multisig_account_alias`,
    );
  }
  const hasAccountId = body.multisig_account_id !== undefined;
  const hasAccountAlias = body.multisig_account_alias !== undefined;
  if (hasAccountId === hasAccountAlias) {
    throw new TypeError(
      `${context} requires exactly one of multisigAccountId or multisigAccountAlias`,
    );
  }
  return body;
}

function normalizeMultisigProposalsQueryBody(value, context) {
  const source = requireObject(value, context);
  const body = normalizeMultisigSelectorBody(source, context);
  if (source.status !== undefined) {
    if (!Array.isArray(source.status)) {
      throw new TypeError(`${context}.status must be an array`);
    }
    body.status = source.status.map((value, index) => {
      const status = requireNonEmptyString(value, `${context}.status[${index}]`).toUpperCase();
      if (!MULTISIG_PROPOSAL_STATUS_VALUES.has(status)) {
        throw new TypeError(
          `${context}.status[${index}] must be one of ${[
            ...MULTISIG_PROPOSAL_STATUS_VALUES,
          ].join(", ")}`,
        );
      }
      return status;
    });
  }
  if (body.cursor !== undefined && body.cursor !== null) {
    body.cursor = requireNonEmptyString(body.cursor, `${context}.cursor`);
  }
  if (body.limit !== undefined && body.limit !== null) {
    body.limit = normalizePositiveInteger(body.limit, `${context}.limit`, undefined);
  }
  return body;
}

function normalizeMultisigProposalsResolveBody(value, context) {
  const source = requireObject(value, context);
  const body = normalizeMultisigSelectorBody(source, context);
  if (source.proposalId !== undefined && body.proposal_id === undefined) {
    body.proposal_id = source.proposalId;
  }
  if (source.instructionsHash !== undefined && body.instructions_hash === undefined) {
    body.instructions_hash = source.instructionsHash;
  }
  delete body.proposalId;
  delete body.instructionsHash;
  if (body.proposal_id !== undefined) {
    body.proposal_id = requireNonEmptyString(body.proposal_id, `${context}.proposal_id`);
  }
  if (body.instructions_hash !== undefined) {
    body.instructions_hash = requireNonEmptyString(
      body.instructions_hash,
      `${context}.instructions_hash`,
    );
  }
  const hasProposalId = body.proposal_id !== undefined;
  const hasInstructionsHash = body.instructions_hash !== undefined;
  if (hasProposalId === hasInstructionsHash) {
    throw new TypeError(
      `${context} requires exactly one of proposalId or instructionsHash`,
    );
  }
  return body;
}

function responseStatus(response) {
  if (typeof response?.status === "number") return response.status;
  return response?.ok === true ? 200 : 0;
}

async function responseText(response) {
  if (typeof response?.text === "function") {
    return response.text().catch(() => "");
  }
  if (typeof response?.json === "function") {
    try {
      return JSON.stringify(await response.json());
    } catch {
      return "";
    }
  }
  return "";
}

export class ToriiBrowserHttpError extends Error {
  constructor(response, bodyText, status = responseStatus(response)) {
    super(`Torii request failed with status ${status}`);
    this.name = "ToriiBrowserHttpError";
    this.response = response;
    this.status = status;
    this.bodyText = bodyText;
  }
}

export class ToriiBrowserClient {
  constructor(baseUrl, options = {}) {
    const normalizedOptions = requireObject(options, "ToriiBrowserClient options");
    this.baseUrl = normalizeBaseUrl(baseUrl);
    this.fetchImpl = normalizedOptions.fetchImpl ?? globalThis.fetch?.bind(globalThis);
    if (typeof this.fetchImpl !== "function") {
      throw new TypeError("ToriiBrowserClient requires a fetch implementation");
    }
    this.defaultHeaders = {
      ...(normalizedOptions.config?.toriiClient?.defaultHeaders ?? {}),
      ...(normalizedOptions.defaultHeaders ?? {}),
    };
    this.timeoutMs =
      normalizedOptions.config?.toriiClient?.timeoutMs ?? normalizedOptions.timeoutMs ?? null;
  }

  getKagemushaReadinessV4(assetDefinitionId, options = {}) {
    const selector = normalizeKagemushaAssetSelector(assetDefinitionId);
    const opts = kagemushaOptions(options, "getKagemushaReadinessV4 options");
    return this._json("GET", "/v1/offline/readiness", {
      params: { asset_definition_id: selector },
      signal: opts.signal,
      responseObserver: (response) => requireKagemushaJsonContentType(
        response.headers.get("content-type"),
        "Kagemusha readiness response",
      ),
    }).then((payload) => normalizeKagemushaReadinessV4(payload, selector));
  }

  submitKagemushaTopUpV4(request, options = {}) {
    return this._submitKagemushaCommandV4(
      "/v1/offline/top-up",
      "top_up",
      request,
      options,
      "submitKagemushaTopUpV4",
    );
  }

  submitKagemushaRedeemV4(request, options = {}) {
    return this._submitKagemushaCommandV4(
      "/v1/offline/redeem",
      "redeem",
      request,
      options,
      "submitKagemushaRedeemV4",
    );
  }

  getKagemushaOperationStatus(operationId, options = {}) {
    const canonicalId = normalizeKagemushaOperationId(operationId);
    const opts = kagemushaOptions(options, "getKagemushaOperationStatus options");
    return this._json("GET", `/v1/offline/operations/${canonicalId}`, {
      signal: opts.signal,
      responseObserver: (response) => requireKagemushaJsonContentType(
        response.headers.get("content-type"),
        "Kagemusha operation status response",
      ),
    }).then((payload) => normalizeKagemushaOperationStatus(payload, canonicalId));
  }

  _submitKagemushaCommandV4(path, kind, request, options, context) {
    const normalizeRequest = kind === "top_up"
      ? normalizeKagemushaTopUpRequestV4
      : normalizeKagemushaRedeemRequestV4;
    const normalized = normalizeRequest(request, `${context} request`);
    const opts = kagemushaOptions(options, `${context} options`);
    let location = null;
    return this._json("POST", path, {
      rawBody: normalized.norito,
      contentType: "application/x-norito",
      headers: {
        Accept: "application/json",
        "Idempotency-Key": normalized.operationId,
      },
      signal: opts.signal,
      successStatuses: [202],
      responseObserver: (response) => {
        requireKagemushaJsonContentType(
          response.headers.get("content-type"),
          "Kagemusha operation reference response",
        );
        location = response.headers.get("location");
      },
    }).then((payload) => normalizeKagemushaOperationReference(payload, {
      expectedOperationId: normalized.operationId,
      expectedKind: kind,
      location,
    }));
  }

  _url(path, params) {
    const normalizedPath = requireNonEmptyString(path, "path").replace(/^\/+/, "");
    const base = new URL(`${this.baseUrl}/`);
    const url = new URL(normalizedPath, base);
    appendSearchParams(url, params);
    return url;
  }

  async _json(method, path, options = {}) {
    const normalizedOptions = requireObject(options, `${method} ${path} options`);
    const headers = {
      Accept: "application/json",
      ...this.defaultHeaders,
      ...(normalizedOptions.headers ?? {}),
    };
    let timeoutId;
    let signal = normalizedOptions.signal;
    if (
      signal === undefined &&
      this.timeoutMs !== null &&
      this.timeoutMs !== undefined &&
      Number(this.timeoutMs) > 0
    ) {
      const controller = new AbortController();
      timeoutId = setTimeout(() => controller.abort(), Number(this.timeoutMs));
      signal = controller.signal;
    }
    const init = {
      method,
      cache: "no-store",
      headers,
      signal,
    };
    if (normalizedOptions.rawBody !== undefined) {
      init.body = normalizedOptions.rawBody;
      init.headers = {
        ...headers,
        ...(normalizedOptions.contentType ? { "Content-Type": normalizedOptions.contentType } : {}),
      };
    } else if (normalizedOptions.body !== undefined) {
      init.body = JSON.stringify(normalizedOptions.body);
      init.headers = {
        ...headers,
        "Content-Type": "application/json",
      };
    }
    let response;
    try {
      response = await this.fetchImpl(this._url(path, normalizedOptions.params), init);
    } finally {
      if (timeoutId !== undefined) {
        clearTimeout(timeoutId);
      }
    }
    const status = responseStatus(response);
    const successStatuses = normalizedOptions.successStatuses ?? DEFAULT_SUCCESS_STATUSES;
    if (!successStatuses.includes(status)) {
      const errorResponse = typeof response?.clone === "function" ? response.clone() : response;
      const bodyText = await responseText(response);
      throw new ToriiBrowserHttpError(errorResponse, bodyText, status);
    }
    if (normalizedOptions.responseObserver !== undefined) {
      if (typeof normalizedOptions.responseObserver !== "function") {
        throw new TypeError(`${method} ${path} responseObserver must be a function`);
      }
      normalizedOptions.responseObserver(response);
    }
    if (status === 204) return null;
    const jsonParser = normalizedOptions.jsonParser ?? JSON.parse;
    if (typeof jsonParser !== "function") {
      throw new TypeError(`${method} ${path} jsonParser must be a function`);
    }
    if (typeof response.text !== "function" && typeof response.json === "function") {
      if (normalizedOptions.jsonParser !== undefined) {
        throw new TypeError(`${method} ${path} requires a text-capable response`);
      }
      return response.json();
    }
    const text = await response.text();
    return text ? jsonParser(text) : null;
  }

  listExplorerAccounts(options = {}) {
    const opts = requireObject(options, "listExplorerAccounts options");
    return this._json("GET", "/v1/explorer/accounts", {
      params: {
        ...normalizeExplorerPagination(opts, "listExplorerAccounts options"),
        domain: opts.domain,
        with_asset: opts.withAsset ?? opts.with_asset,
        address_format: opts.addressFormat ?? opts.address_format,
      },
      signal: signalFrom(opts),
    });
  }

  getExplorerAccount(accountId, options = {}) {
    const opts = requireObject(options, "getExplorerAccount options");
    return this._json("GET", `/v1/explorer/accounts/${encodeURIComponent(requireNonEmptyString(accountId, "accountId"))}`, {
      params: { address_format: opts.addressFormat ?? opts.address_format },
      signal: signalFrom(opts),
    });
  }

  listExplorerDomains(options = {}) {
    const opts = requireObject(options, "listExplorerDomains options");
    return this._json("GET", "/v1/explorer/domains", {
      params: {
        ...normalizeExplorerPagination(opts, "listExplorerDomains options"),
        owned_by: opts.ownedBy ?? opts.owned_by,
      },
      signal: signalFrom(opts),
    });
  }

  getExplorerDomain(domainId, options = {}) {
    const opts = requireObject(options, "getExplorerDomain options");
    return this._json("GET", `/v1/explorer/domains/${encodeURIComponent(requireNonEmptyString(domainId, "domainId"))}`, {
      signal: signalFrom(opts),
    });
  }

  listExplorerAssets(options = {}) {
    const opts = requireObject(options, "listExplorerAssets options");
    return this._json("GET", "/v1/explorer/assets", {
      params: {
        ...normalizeExplorerPagination(opts, "listExplorerAssets options"),
        owned_by: opts.ownedBy ?? opts.owned_by,
        definition: opts.definition,
        asset_id: opts.assetId ?? opts.asset_id,
      },
      signal: signalFrom(opts),
    }).then((payload) =>
      normalizeQuantityPage(payload, "explorer assets response", ["quantity"]),
    );
  }

  getExplorerAsset(assetId, options = {}) {
    const opts = requireObject(options, "getExplorerAsset options");
    return this._json("GET", `/v1/explorer/assets/${encodeURIComponent(requireNonEmptyString(assetId, "assetId"))}`, {
      signal: signalFrom(opts),
    }).then((payload) =>
      normalizeQuantityRecord(payload, "explorer asset response", ["quantity"]),
    );
  }

  listAccountAssets(accountId, options = {}) {
    const opts = requireObject(options, "listAccountAssets options");
    return this._json("GET", `/v1/accounts/${encodeURIComponent(requireNonEmptyString(accountId, "accountId"))}/assets`, {
      params: {
        ...normalizeIterablePagination(opts, "listAccountAssets options"),
        asset: opts.asset ?? opts.assetId,
        scope: opts.scope,
        count_mode: normalizeCountMode(opts.countMode ?? opts.count_mode, "countMode"),
      },
      signal: signalFrom(opts),
    }).then((payload) =>
      normalizeQuantityPage(payload, "account assets response", ["quantity"]),
    );
  }

  queryAccountTransactions(accountId, options = {}) {
    const opts = requireObject(options, "queryAccountTransactions options");
    return this._json("POST", `/v1/accounts/${encodeURIComponent(requireNonEmptyString(accountId, "accountId"))}/transactions/query`, {
      body: normalizeTransactionQueryEnvelope(opts, "queryAccountTransactions"),
      signal: signalFrom(opts),
    });
  }

  queryTransactions(options = {}) {
    const opts = requireObject(options, "queryTransactions options");
    return this._json("POST", "/v1/transactions/query", {
      body: normalizeTransactionQueryEnvelope(opts, "queryTransactions"),
      signal: signalFrom(opts),
    });
  }

  queryVisibleTransactions(options = {}) {
    const opts = requireObject(options, "queryVisibleTransactions options");
    return this._json("POST", "/v1/transactions/visible/query", {
      body: normalizeTransactionQueryEnvelope(opts, "queryVisibleTransactions"),
      signal: signalFrom(opts),
    });
  }

  listAssetHolders(assetDefinitionId, options = {}) {
    const opts = requireObject(options, "listAssetHolders options");
    return this._json("GET", `/v1/assets/${encodeURIComponent(requireNonEmptyString(assetDefinitionId, "assetDefinitionId"))}/holders`, {
      params: {
        ...normalizeIterablePagination(opts, "listAssetHolders options"),
        account_id: opts.accountId ?? opts.account_id,
        scope: opts.scope,
        count_mode: normalizeCountMode(opts.countMode ?? opts.count_mode, "countMode"),
      },
      signal: signalFrom(opts),
    }).then((payload) =>
      normalizeQuantityPage(payload, "asset holders response", ["quantity"]),
    );
  }

  listAssetDefinitions(options = {}) {
    const opts = requireObject(options, "listAssetDefinitions options");
    return this._json("GET", "/v1/assets/definitions", {
      params: {
        ...normalizeIterablePagination(opts, "listAssetDefinitions options"),
        count_mode: normalizeCountMode(opts.countMode ?? opts.count_mode, "countMode"),
      },
      signal: signalFrom(opts),
    }).then((payload) =>
      normalizeQuantityPage(
        payload,
        "asset definitions response",
        ["total_quantity"],
        { optional: true },
      ),
    );
  }

  getAssetDefinition(assetDefinitionId, options = {}) {
    const opts = requireObject(options, "getAssetDefinition options");
    return this._json("GET", `/v1/assets/definitions/${encodeURIComponent(requireNonEmptyString(assetDefinitionId, "assetDefinitionId"))}`, {
      signal: signalFrom(opts),
    }).then((payload) =>
      normalizeQuantityRecord(
        payload,
        "asset definition response",
        ["total_quantity"],
        { optional: true },
      ),
    );
  }

  resolveAlias(aliasOrRequest, options = {}) {
    const opts = requireObject(options, "resolveAlias options");
    const body =
      typeof aliasOrRequest === "string"
        ? { alias: requireNonEmptyString(aliasOrRequest, "alias") }
        : requireObject(aliasOrRequest, "resolveAlias request");
    return this._json("POST", "/v1/aliases/resolve", {
      body,
      signal: signalFrom(opts),
    });
  }

  resolveAssetAlias(aliasOrRequest, options = {}) {
    const opts = requireObject(options, "resolveAssetAlias options");
    const body =
      typeof aliasOrRequest === "string"
        ? { alias: requireNonEmptyString(aliasOrRequest, "alias") }
        : requireObject(aliasOrRequest, "resolveAssetAlias request");
    return this._json("POST", "/v1/assets/aliases/resolve", {
      body,
      signal: signalFrom(opts),
    });
  }

  listExplorerAssetDefinitions(options = {}) {
    const opts = requireObject(options, "listExplorerAssetDefinitions options");
    return this._json("GET", "/v1/explorer/asset-definitions", {
      params: {
        ...normalizeExplorerPagination(opts, "listExplorerAssetDefinitions options"),
        domain: opts.domain,
        owned_by: opts.ownedBy ?? opts.owned_by,
      },
      signal: signalFrom(opts),
    }).then((payload) =>
      normalizeQuantityPage(
        payload,
        "explorer asset definitions response",
        ["total_quantity"],
        { optional: true },
      ),
    );
  }

  getExplorerAssetDefinitionEconometrics(assetDefinitionId, options = {}) {
    const opts = requireObject(options, "getExplorerAssetDefinitionEconometrics options");
    return this._json("GET", `/v1/explorer/asset-definitions/${encodeURIComponent(requireNonEmptyString(assetDefinitionId, "assetDefinitionId"))}/econometrics`, {
      signal: signalFrom(opts),
    });
  }

  getExplorerAssetDefinitionSnapshot(assetDefinitionId, options = {}) {
    const opts = requireObject(options, "getExplorerAssetDefinitionSnapshot options");
    return this._json("GET", `/v1/explorer/asset-definitions/${encodeURIComponent(requireNonEmptyString(assetDefinitionId, "assetDefinitionId"))}/snapshot`, {
      signal: signalFrom(opts),
    });
  }

  listExplorerNfts(options = {}) {
    const opts = requireObject(options, "listExplorerNfts options");
    return this._json("GET", "/v1/explorer/nfts", {
      params: {
        ...normalizeExplorerPagination(opts, "listExplorerNfts options"),
        owned_by: opts.ownedBy ?? opts.owned_by,
        domain: opts.domain,
      },
      signal: signalFrom(opts),
    });
  }

  getExplorerNft(nftId, options = {}) {
    const opts = requireObject(options, "getExplorerNft options");
    return this._json("GET", `/v1/explorer/nfts/${encodeURIComponent(requireNonEmptyString(nftId, "nftId"))}`, {
      signal: signalFrom(opts),
    });
  }

  listExplorerRwas(options = {}) {
    const opts = requireObject(options, "listExplorerRwas options");
    return this._json("GET", "/v1/explorer/rwas", {
      params: {
        ...normalizeExplorerPagination(opts, "listExplorerRwas options"),
        owned_by: opts.ownedBy ?? opts.owned_by,
        domain: opts.domain,
      },
      signal: signalFrom(opts),
    }).then((payload) =>
      normalizeQuantityPage(payload, "explorer rwas response", ["quantity", "held_quantity"]),
    );
  }

  getExplorerRwa(rwaId, options = {}) {
    const opts = requireObject(options, "getExplorerRwa options");
    return this._json("GET", `/v1/explorer/rwas/${encodeURIComponent(requireNonEmptyString(rwaId, "rwaId"))}`, {
      signal: signalFrom(opts),
    }).then((payload) =>
      normalizeQuantityRecord(payload, "explorer rwa response", ["quantity", "held_quantity"]),
    );
  }

  listExplorerBlocks(options = {}) {
    const opts = requireObject(options, "listExplorerBlocks options");
    return this._json("GET", "/v1/explorer/blocks", {
      params: normalizeExplorerPagination(opts, "listExplorerBlocks options"),
      signal: signalFrom(opts),
    });
  }

  getExplorerBlock(identifier, options = {}) {
    const opts = requireObject(options, "getExplorerBlock options");
    return this._json("GET", `/v1/explorer/blocks/${encodeURIComponent(String(identifier))}`, {
      signal: signalFrom(opts),
    });
  }

  getExplorerMetrics(options = {}) {
    const opts = requireObject(options, "getExplorerMetrics options");
    return this._json("GET", "/v1/explorer/metrics", { signal: signalFrom(opts) });
  }

  getExplorerHealth(options = {}) {
    const opts = requireObject(options, "getExplorerHealth options");
    return this._json("GET", "/v1/explorer/health", { signal: signalFrom(opts) });
  }

  listExplorerTransactions(options = {}) {
    const opts = requireObject(options, "listExplorerTransactions options");
    return this._json("GET", "/v1/explorer/transactions", {
      params: {
        ...normalizeExplorerPagination(opts, "listExplorerTransactions options"),
        authority: opts.authority,
        block: opts.block,
        status: opts.status,
        asset_id: opts.assetId ?? opts.asset_id,
        address_format: opts.addressFormat ?? opts.address_format,
      },
      signal: signalFrom(opts),
    });
  }

  listLatestExplorerTransactions(options = {}) {
    const opts = requireObject(options, "listLatestExplorerTransactions options");
    return this._json("GET", "/v1/explorer/transactions/latest", {
      params: {
        per_page: opts.perPage ?? opts.per_page,
        authority: opts.authority,
        block: opts.block,
        status: opts.status,
        asset_id: opts.assetId ?? opts.asset_id,
        address_format: opts.addressFormat ?? opts.address_format,
      },
      signal: signalFrom(opts),
    });
  }

  getExplorerTransaction(hash, options = {}) {
    const opts = requireObject(options, "getExplorerTransaction options");
    return this._json("GET", `/v1/explorer/transactions/${encodeURIComponent(requireNonEmptyString(hash, "hash"))}`, {
      params: { address_format: opts.addressFormat ?? opts.address_format },
      signal: signalFrom(opts),
    });
  }

  listExplorerInstructions(options = {}) {
    const opts = requireObject(options, "listExplorerInstructions options");
    return this._json("GET", "/v1/explorer/instructions", {
      params: {
        ...normalizeExplorerPagination(opts, "listExplorerInstructions options"),
        account: opts.account,
        authority: opts.authority,
        kind: opts.kind,
        transaction_hash: opts.transactionHash ?? opts.transaction_hash,
        transaction_status: opts.transactionStatus ?? opts.transaction_status,
        block: opts.block,
        asset_id: opts.assetId ?? opts.asset_id,
        address_format: opts.addressFormat ?? opts.address_format,
      },
      signal: signalFrom(opts),
    });
  }

  listLatestExplorerInstructions(options = {}) {
    const opts = requireObject(options, "listLatestExplorerInstructions options");
    return this._json("GET", "/v1/explorer/instructions/latest", {
      params: {
        per_page: opts.perPage ?? opts.per_page,
        account: opts.account,
        authority: opts.authority,
        kind: opts.kind,
        transaction_hash: opts.transactionHash ?? opts.transaction_hash,
        transaction_status: opts.transactionStatus ?? opts.transaction_status,
        block: opts.block,
        asset_id: opts.assetId ?? opts.asset_id,
        address_format: opts.addressFormat ?? opts.address_format,
      },
      signal: signalFrom(opts),
    });
  }

  getExplorerInstruction(transactionHash, index, options = {}) {
    const opts = requireObject(options, "getExplorerInstruction options");
    return this._json("GET", `/v1/explorer/instructions/${encodeURIComponent(requireNonEmptyString(transactionHash, "transactionHash"))}/${encodeURIComponent(String(index))}`, {
      params: { address_format: opts.addressFormat ?? opts.address_format },
      signal: signalFrom(opts),
    });
  }

  getExplorerInstructionContractView(transactionHash, index, options = {}) {
    const opts = requireObject(options, "getExplorerInstructionContractView options");
    return this._json("GET", `/v1/explorer/instructions/${encodeURIComponent(requireNonEmptyString(transactionHash, "transactionHash"))}/${encodeURIComponent(String(index))}/contract-view`, {
      signal: signalFrom(opts),
    });
  }

  getMultisigSpec(selector, options = {}) {
    const opts = requireObject(options, "getMultisigSpec options");
    return this._json("POST", "/v1/multisig/spec", {
      body: normalizeMultisigSelectorBody(selector, "getMultisigSpec selector"),
      signal: signalFrom(opts),
    });
  }

  queryMultisigProposals(selector, options = {}) {
    const opts = requireObject(options, "queryMultisigProposals options");
    return this._json("POST", "/v1/multisig/proposals/query", {
      body: normalizeMultisigProposalsQueryBody(
        selector,
        "queryMultisigProposals selector",
      ),
      signal: signalFrom(opts),
    });
  }

  resolveMultisigProposal(request, options = {}) {
    const normalizedRequest = normalizeMultisigProposalsResolveBody(
      request,
      "resolveMultisigProposal request",
    );
    const opts = requireObject(options, "resolveMultisigProposal options");
    return this._json("POST", "/v1/multisig/proposals/resolve", {
      body: normalizedRequest,
      signal: signalFrom(opts),
    });
  }

  async submitMultisigPropose(request, options = {}) {
    const opts = requireObject(options, "submitMultisigPropose options");
    const { noritoEncodeMultisigProposeRequest } = await loadNoritoEncoders();
    return this._json("POST", "/v1/multisig/propose", {
      rawBody: noritoEncodeMultisigProposeRequest(requireObject(request, "submitMultisigPropose request")),
      contentType: "application/x-norito",
      headers: { Accept: "application/json", ...(opts.headers ?? {}) },
      signal: signalFrom(opts),
      successStatuses: opts.successStatuses ?? [200, 202],
    });
  }

  async submitMultisigContractCallPropose(request, options = {}) {
    const opts = requireObject(options, "submitMultisigContractCallPropose options");
    const { noritoEncodeMultisigContractCallProposeRequest } = await loadNoritoEncoders();
    return this._json("POST", "/v1/contracts/call/multisig/propose", {
      rawBody: noritoEncodeMultisigContractCallProposeRequest(
        requireObject(request, "submitMultisigContractCallPropose request"),
      ),
      contentType: "application/x-norito",
      headers: { Accept: "application/json", ...(opts.headers ?? {}) },
      signal: signalFrom(opts),
      successStatuses: opts.successStatuses ?? [200, 202],
    });
  }

  async submitMultisigContractCallApprove(request, options = {}) {
    const opts = requireObject(options, "submitMultisigContractCallApprove options");
    const { noritoEncodeMultisigContractCallApproveRequest } = await loadNoritoEncoders();
    return this._json("POST", "/v1/contracts/call/multisig/approve", {
      rawBody: noritoEncodeMultisigContractCallApproveRequest(
        requireObject(request, "submitMultisigContractCallApprove request"),
      ),
      contentType: "application/x-norito",
      headers: { Accept: "application/json", ...(opts.headers ?? {}) },
      signal: signalFrom(opts),
      successStatuses: opts.successStatuses ?? [200, 202],
    });
  }

  getSumeragiStatus(options = {}) {
    const opts = requireObject(options, "getSumeragiStatus options");
    return this._json("GET", "/v1/sumeragi/status", { signal: signalFrom(opts) });
  }

  getSumeragiTelemetry(options = {}) {
    const opts = requireObject(options, "getSumeragiTelemetry options");
    return this._json("GET", "/v1/sumeragi/telemetry", { signal: signalFrom(opts) });
  }

  listKaigiRelays(options = {}) {
    const opts = requireObject(options, "listKaigiRelays options");
    return this._json("GET", "/v1/kaigi/relays", { signal: signalFrom(opts) });
  }

  getKaigiRelay(relayId, options = {}) {
    const opts = requireObject(options, "getKaigiRelay options");
    return this._json("GET", `/v1/kaigi/relays/${encodeURIComponent(requireNonEmptyString(relayId, "relayId"))}`, {
      signal: signalFrom(opts),
    });
  }

  getKaigiRelaysHealth(options = {}) {
    const opts = requireObject(options, "getKaigiRelaysHealth options");
    return this._json("GET", "/v1/kaigi/relays/health", { signal: signalFrom(opts) });
  }

  deployContract(request, options = {}) {
    const opts = requireObject(options, "deployContract options");
    return this._json("POST", "/v1/contracts/deploy", {
      body: requireObject(request, "deployContract request"),
      signal: signalFrom(opts),
      successStatuses: [200, 202],
    });
  }
}

export { ToriiBrowserClient as ToriiClient, ToriiBrowserHttpError as ToriiHttpError };
