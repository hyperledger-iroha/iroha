const DEFAULT_SUCCESS_STATUSES = [200];

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

function signalFrom(options) {
  return options.signal === undefined ? undefined : options.signal;
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
    this.defaultHeaders = { ...(normalizedOptions.defaultHeaders ?? {}) };
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
    const init = {
      method,
      cache: "no-store",
      headers,
      signal: normalizedOptions.signal,
    };
    if (normalizedOptions.body !== undefined) {
      init.body = JSON.stringify(normalizedOptions.body);
      init.headers = {
        ...headers,
        "Content-Type": "application/json",
      };
    }
    const response = await this.fetchImpl(this._url(path, normalizedOptions.params), init);
    const status = responseStatus(response);
    const successStatuses = normalizedOptions.successStatuses ?? DEFAULT_SUCCESS_STATUSES;
    if (!successStatuses.includes(status)) {
      const errorResponse = typeof response?.clone === "function" ? response.clone() : response;
      const bodyText = await responseText(response);
      throw new ToriiBrowserHttpError(errorResponse, bodyText, status);
    }
    if (status === 204) return null;
    if (typeof response.text !== "function" && typeof response.json === "function") {
      return response.json();
    }
    const text = await response.text();
    return text ? JSON.parse(text) : null;
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
    });
  }

  getExplorerAsset(assetId, options = {}) {
    const opts = requireObject(options, "getExplorerAsset options");
    return this._json("GET", `/v1/explorer/assets/${encodeURIComponent(requireNonEmptyString(assetId, "assetId"))}`, {
      signal: signalFrom(opts),
    });
  }

  listAccountAssets(accountId, options = {}) {
    const opts = requireObject(options, "listAccountAssets options");
    return this._json("GET", `/v1/accounts/${encodeURIComponent(requireNonEmptyString(accountId, "accountId"))}/assets`, {
      params: {
        ...normalizeIterablePagination(opts, "listAccountAssets options"),
        asset: opts.asset ?? opts.assetId,
        scope: opts.scope,
        count_mode: opts.countMode ?? opts.count_mode,
      },
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
        count_mode: opts.countMode ?? opts.count_mode,
      },
      signal: signalFrom(opts),
    });
  }

  listAssetDefinitions(options = {}) {
    const opts = requireObject(options, "listAssetDefinitions options");
    return this._json("GET", "/v1/assets/definitions", {
      params: {
        ...normalizeIterablePagination(opts, "listAssetDefinitions options"),
        count_mode: opts.countMode ?? opts.count_mode,
      },
      signal: signalFrom(opts),
    });
  }

  getAssetDefinition(assetDefinitionId, options = {}) {
    const opts = requireObject(options, "getAssetDefinition options");
    return this._json("GET", `/v1/assets/definitions/${encodeURIComponent(requireNonEmptyString(assetDefinitionId, "assetDefinitionId"))}`, {
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
    });
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
    });
  }

  getExplorerRwa(rwaId, options = {}) {
    const opts = requireObject(options, "getExplorerRwa options");
    return this._json("GET", `/v1/explorer/rwas/${encodeURIComponent(requireNonEmptyString(rwaId, "rwaId"))}`, {
      signal: signalFrom(opts),
    });
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
