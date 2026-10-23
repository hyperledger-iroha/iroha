/**
 * Alias bindings on rows of the accounts collection (`GET /v1/accounts`).
 *
 * Torii joins an account's active alias bindings onto its row for the queries that mention
 * `alias_scopes` or `aliases`; every other accounts read keeps today's rows:
 *
 * - `alias_scopes`: the text after `@` of every alias the caller may read (`domain.dataspace`,
 *   or `dataspace` for a domainless alias), each once. One entry stands for one binding, so
 *   `field("alias_scopes").eq("mibank.bpng")` finds the accounts that hold an alias in that scope,
 *   not a domain on one alias and a dataspace on another.
 * - `aliases`: those bindings as `{alias, dataspace, domain, is_primary}`, in the order of
 *   `POST /v1/aliases/by-account`, which also decides what the caller may read.
 *
 * ```js
 * const query = accountsInAliasScope("mibank.bpng", { limit: 500 });
 * for await (const page of torii.accounts.pages(query)) {
 *   for (const row of decodeAccountAliasRows(page.items)) console.log(row.id, row.aliases);
 * }
 * ```
 */
import { ToriiError } from "../toriiErrors.js";
import { field } from "./grammar.js";
import { ListQuery } from "./listQuery.js";

/** Row fields that join an account's alias bindings onto it. */
export const ACCOUNT_ALIAS_FIELDS = Object.freeze(["alias_scopes", "aliases"]);

/** Fields `accountsInAliasScope` selects, which `decodeAccountAliasRows` requires exactly. */
export const ACCOUNT_ALIAS_ROW_FIELDS = Object.freeze(["id", "uaid", "aliases"]);

const ALIAS_BINDING_FIELDS = Object.freeze(["alias", "dataspace", "domain", "is_primary"]);

function invalid(message) {
  return new ToriiError(message, { code: "invalid_response" });
}

function isPlainRecord(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function rejectUnknownFields(record, allowed, context) {
  for (const key of Object.keys(record)) {
    if (!allowed.includes(key)) throw invalid(`${context} contains unknown field \`${key}\``);
  }
}

function requireText(value, context) {
  if (typeof value !== "string" || value.length === 0) {
    throw invalid(`${context} must be a non-empty string`);
  }
  return value;
}

/**
 * The alias scope of a canonical alias literal: the text after `@`.
 *
 * @param {string} alias for example `alice@mibank.bpng`
 * @returns {string} for example `mibank.bpng`
 */
export function accountAliasScope(alias) {
  const separator = typeof alias === "string" ? alias.indexOf("@") : -1;
  if (separator <= 0 || separator === alias.length - 1) {
    throw new TypeError("alias must be an alias literal such as `name@domain.dataspace`");
  }
  return alias.slice(separator + 1);
}

/**
 * A query for the accounts that hold an active alias in `scope`, selecting
 * `id`, `uaid` and `aliases`. Page it with `torii.accounts.pages(query)` or a signed `GET`
 * (`query.toQueryPairs()`), and decode the items with `decodeAccountAliasRows`.
 *
 * With `accountId` the read is a point lookup of that one account: it returns the account when
 * it holds an alias in `scope` and nothing otherwise.
 *
 * @param {string} scope `domain.dataspace`, or `dataspace` for domainless aliases
 * @param {{limit?: number, cursor?: string, accountId?: string}} [options]
 * @returns {ListQuery}
 */
export function accountsInAliasScope(scope, options = {}) {
  if (typeof scope !== "string" || scope.length === 0 || scope.trim() !== scope || scope.includes("@")) {
    throw new TypeError("scope must be `domain.dataspace` or `dataspace`, without `@` or spaces");
  }
  const { limit, cursor, accountId } = options;
  const inScope = field("alias_scopes").eq(scope);
  if (accountId !== undefined && (typeof accountId !== "string" || accountId.length === 0 || accountId.trim() !== accountId)) {
    throw new TypeError("accountId must be a non-empty account literal without surrounding whitespace");
  }
  return ListQuery.from({
    filter: accountId === undefined ? inScope : field("id").eq(accountId).and(inScope),
    select: ACCOUNT_ALIAS_ROW_FIELDS,
    ...(limit === undefined ? {} : { limit }),
    ...(cursor === undefined ? {} : { cursor }),
  });
}

/**
 * Decode an `aliases` field exactly: every binding has `alias`, `dataspace`, `is_primary` and an
 * optional `domain`, and nothing else.
 *
 * @param {unknown} value
 * @param {string} [context]
 * @returns {Array<{alias: string, dataspace: string, domain: string | null, is_primary: boolean}>}
 */
export function decodeAccountAliases(value, context = "account aliases") {
  if (!Array.isArray(value)) throw invalid(`${context} must be an array`);
  return value.map((entry, index) => {
    const at = `${context}[${index}]`;
    if (!isPlainRecord(entry)) throw invalid(`${at} must be an object`);
    rejectUnknownFields(entry, ALIAS_BINDING_FIELDS, at);
    const alias = requireText(entry.alias, `${at}.alias`);
    const dataspace = requireText(entry.dataspace, `${at}.dataspace`);
    const domain =
      entry.domain === undefined || entry.domain === null
        ? null
        : requireText(entry.domain, `${at}.domain`);
    if (typeof entry.is_primary !== "boolean") throw invalid(`${at}.is_primary must be a boolean`);
    return Object.freeze({ alias, dataspace, domain, is_primary: entry.is_primary });
  });
}

/**
 * Decode the items of a page read with `accountsInAliasScope`: exactly `id`, `uaid`
 * (a string or null) and `aliases` per row.
 *
 * @param {unknown} items `page.items`
 * @param {string} [context]
 * @returns {Array<{id: string, uaid: string | null, aliases: ReturnType<typeof decodeAccountAliases>}>}
 */
export function decodeAccountAliasRows(items, context = "accounts page") {
  if (!Array.isArray(items)) throw invalid(`${context} items must be an array`);
  return items.map((row, index) => {
    const at = `${context}.items[${index}]`;
    if (!isPlainRecord(row)) throw invalid(`${at} must be an object`);
    rejectUnknownFields(row, ACCOUNT_ALIAS_ROW_FIELDS, at);
    const id = requireText(row.id, `${at}.id`);
    if (row.uaid !== undefined && row.uaid !== null && typeof row.uaid !== "string") {
      throw invalid(`${at}.uaid must be a string or null`);
    }
    return Object.freeze({
      id,
      uaid: row.uaid ?? null,
      aliases: Object.freeze(decodeAccountAliases(row.aliases, `${at}.aliases`)),
    });
  });
}
