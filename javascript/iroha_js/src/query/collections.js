/**
 * One surface for every Torii collection.
 *
 * Each collection is a `ToriiCollection` with three methods:
 *
 * - `list(query?, options?)` fetches one page (`{items, nextCursor, total}`);
 * - `pages(query?, options?)` iterates pages, following `nextCursor`;
 * - `iterate(query?, options?)` iterates every item of every page.
 *
 * Requests use `POST <collection>/query` with the canonical JSON body. The
 * executor that performs the request (transport, signing, error decoding) is
 * supplied by the client that owns the collection.
 *
 * Transaction collections are history collections: rows come newest first by
 * block position, and `sort`, `includeTotal` and `aggregate` are rejected. A
 * page may hold fewer than `limit` items (even none) and still carry a
 * `nextCursor`; the iterators keep following it.
 */
import { ListQueryError, ToriiError } from "../toriiErrors.js";
import { ListQuery } from "./listQuery.js";

/** Collection paths that take no path parameter. */
export const TORII_COLLECTION_PATHS = Object.freeze({
  domains: "/v1/domains",
  accounts: "/v1/accounts",
  assetDefinitions: "/v1/assets/definitions",
  nfts: "/v1/nfts",
  rwas: "/v1/rwas",
  repoAgreements: "/v1/repo/agreements",
  transactions: "/v1/transactions",
});

/** Collections read in history order (newest first), without sort, total or aggregates. */
const HISTORY_COLLECTIONS = new Set(["transactions"]);

function abortReason(signal) {
  if (signal.reason !== undefined) return signal.reason;
  const error = new Error("The operation was aborted");
  error.name = "AbortError";
  return error;
}

function throwIfAborted(signal) {
  if (signal?.aborted) throw abortReason(signal);
}

function pathSegment(value, name) {
  if (typeof value !== "string" || value.length === 0 || value.trim() !== value) {
    throw new TypeError(`${name} must be a non-empty string without surrounding whitespace`);
  }
  return encodeURIComponent(value);
}

function signalOf(options) {
  if (options === undefined || options === null || typeof options !== "object") {
    return undefined;
  }
  return options.signal ?? undefined;
}

async function* paginate(execute, path, query, options) {
  const signal = signalOf(options);
  let current = query;
  for (;;) {
    throwIfAborted(signal);
    const page = await execute(path, current, options);
    yield page;
    if (page.nextCursor === null) return;
    if (page.nextCursor === current.cursor) {
      throw new ToriiError(
        `${path} returned the cursor it was given; refusing to repeat the same page`,
        { code: "invalid_response", details: { field: "next_cursor" } },
      );
    }
    current = current.withCursor(page.nextCursor);
  }
}

async function* pageItems(pages) {
  for await (const page of pages) {
    yield* page.items;
  }
}

function historyQuery(query) {
  if (query.sort.length > 0) {
    throw new ListQueryError(
      "sort",
      "transaction history is always newest first and cannot be sorted",
    );
  }
  if (query.includeTotal) {
    throw new ListQueryError(
      "include_total",
      "transaction history has no total; counting would scan the whole history",
    );
  }
  if (query.aggregate !== undefined) {
    throw new ListQueryError("aggregate", "transaction history cannot be aggregated");
  }
  return query;
}

/** A queryable Torii collection such as `client.assetDefinitions`. */
export class ToriiCollection {
  #path;
  #execute;
  #history;

  /**
   * @param {string} path collection path such as `/v1/assets/definitions`
   * @param {(path: string, query: ListQuery, options: unknown) => Promise<object>} execute
   * @param {{history?: boolean}} [options] `history` marks a transaction history collection
   */
  constructor(path, execute, { history = false } = {}) {
    if (typeof execute !== "function") {
      throw new TypeError("ToriiCollection requires an executor function");
    }
    this.#path = path;
    this.#execute = execute;
    this.#history = history === true;
    Object.freeze(this);
  }

  /** The collection path, e.g. `/v1/assets/definitions`. */
  get path() {
    return this.#path;
  }

  /** Whether this is a history collection (newest first; no sort, total or aggregate). */
  get history() {
    return this.#history;
  }

  #query(input) {
    const query = ListQuery.from(input);
    return this.#history ? historyQuery(query) : query;
  }

  /**
   * Fetch one page. Pass the previous page's `nextCursor` as `cursor` to
   * continue.
   */
  async list(query, options) {
    return this.#execute(this.#path, this.#query(query), options);
  }

  /**
   * Iterate pages until `nextCursor` is `null`. The query is validated before
   * the first request; abort with `options.signal` or by leaving the loop.
   */
  pages(query, options) {
    return paginate(this.#execute, this.#path, this.#query(query), options);
  }

  /** Iterate every matching item across all pages. */
  iterate(query, options) {
    return pageItems(this.pages(query, options));
  }
}

/**
 * Build the collection accessors for a client.
 *
 * @param {(path: string, query: ListQuery, options: unknown) => Promise<{items: unknown[], nextCursor: string | null, total?: number}>} execute
 */
export function createToriiCollections(execute) {
  const collection = (path, history = false) => new ToriiCollection(path, execute, { history });
  const fixed = {};
  for (const [name, path] of Object.entries(TORII_COLLECTION_PATHS)) {
    fixed[name] = collection(path, HISTORY_COLLECTIONS.has(name));
  }
  return Object.freeze({
    ...fixed,
    accountAssets: (accountId) =>
      collection(`/v1/accounts/${pathSegment(accountId, "accountId")}/assets`),
    assetHolders: (assetDefinitionId) =>
      collection(`/v1/assets/${pathSegment(assetDefinitionId, "assetDefinitionId")}/holders`),
    accountTransactions: (accountId) =>
      collection(`/v1/accounts/${pathSegment(accountId, "accountId")}/transactions`, true),
  });
}
