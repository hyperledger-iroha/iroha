/**
 * Rows of the asset movement feed (`GET /v1/assets/{definition_id}/movements`).
 *
 * Every committed movement of one asset definition, newest first by `block_height`,
 * `block_index` and `movement_index`, one row per movement:
 *
 * - `TRANSFER`, `MINT`, `BURN`: instruction transfers, mints and burns, applied batch receipts and
 *   executed multisig transfers, as account history projects them;
 * - `OFFLINE_LOAD`, `OFFLINE_UNLOAD`, `OFFLINE_FEE`: KAGEMUSHA Loads (payer to reserve, and a
 *   quoted online charge), paid Unloads (reserve to the account, and the online charge) and paid
 *   fee claims, read from the ledger records of the transaction that moved the value.
 *
 * A failed transaction keeps its attempted instruction movements with `result_ok: false`; sum
 * only `result_ok: true` rows for balances. `from_account_id` is null only for `MINT` and
 * `to_account_id` only for `BURN`. The node answers from a local index: `503
 * asset_movements_indexing` (with `Retry-After`) while it catches up with the committed tip, and
 * `asset_movements_not_retained` for movements below its retained floor.
 *
 * ```js
 * const feed = torii.assetMovements(kinaDefinitionId);
 * for await (const page of feed.pages({ filter: field("block_height").gt(newestSeen), limit: 500 })) {
 *   for (const row of page.items) record(row); // decoded with decodeAssetMovementRow
 * }
 * ```
 */
import { ToriiError } from "../toriiErrors.js";

/** Every movement kind, in the order Torii documents them. */
export const ASSET_MOVEMENT_KINDS = Object.freeze([
  "TRANSFER",
  "MINT",
  "BURN",
  "OFFLINE_LOAD",
  "OFFLINE_UNLOAD",
  "OFFLINE_FEE",
]);

/** Fields of a complete movement row; `decodeAssetMovementRow` requires every one. */
export const ASSET_MOVEMENT_ROW_FIELDS = Object.freeze([
  "id",
  "kind",
  "timestamp_ms",
  "from_account_id",
  "to_account_id",
  "asset_definition_id",
  "amount",
  "tx_hash",
  "authority",
  "block_height",
  "block_index",
  "movement_index",
  "result_ok",
]);

const OFFLINE_KINDS = Object.freeze(["OFFLINE_LOAD", "OFFLINE_UNLOAD", "OFFLINE_FEE"]);
/** Canonical non-negative decimal text: no sign, exponent, leading or trailing zeros. */
const CANONICAL_AMOUNT = /^(?:0|[1-9][0-9]*)(?:\.[0-9]*[1-9])?$/u;
const TX_HASH = /^[0-9a-f]{64}$/u;

function invalid(message) {
  return new ToriiError(message, { code: "invalid_response" });
}

function isPlainRecord(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function requireText(value, context) {
  if (typeof value !== "string" || value.length === 0) {
    throw invalid(`${context} must be a non-empty string`);
  }
  return value;
}

function requireInteger(value, minimum, context) {
  if (!Number.isSafeInteger(value) || value < minimum) {
    throw invalid(`${context} must be an integer of at least ${minimum}`);
  }
  return value;
}

/** An account field: null exactly for the kind that has no such account. */
function requireAccount(value, absentFor, kind, context) {
  if (kind === absentFor) {
    if (value !== null) throw invalid(`${context} must be null for ${absentFor}`);
    return null;
  }
  if (value === null) throw invalid(`${context} may be null only for ${absentFor}`);
  return requireText(value, context);
}

/**
 * Decode one complete movement row exactly. Fields Torii may add later are ignored; a missing
 * or malformed field, an unknown kind, a non-canonical amount, a null account outside `MINT` or
 * `BURN`, a non-integer coordinate, an `id` other than `<tx_hash>:<movement_index>` or a failed
 * KAGEMUSHA row is `invalid_response`.
 *
 * @param {unknown} row
 * @param {string} [context]
 * @returns {Readonly<{id: string, kind: string, timestamp_ms: number, from_account_id: string | null,
 *   to_account_id: string | null, asset_definition_id: string, amount: string, tx_hash: string,
 *   authority: string, block_height: number, block_index: number, movement_index: number,
 *   result_ok: boolean}>}
 */
export function decodeAssetMovementRow(row, context = "asset movement") {
  if (!isPlainRecord(row)) throw invalid(`${context} must be an object`);
  const { kind } = row;
  if (!ASSET_MOVEMENT_KINDS.includes(kind)) {
    throw invalid(`${context}.kind must be one of ${ASSET_MOVEMENT_KINDS.join(", ")}`);
  }
  const fromAccountId = requireAccount(row.from_account_id, "MINT", kind, `${context}.from_account_id`);
  const toAccountId = requireAccount(row.to_account_id, "BURN", kind, `${context}.to_account_id`);
  if (typeof row.amount !== "string" || !CANONICAL_AMOUNT.test(row.amount)) {
    throw invalid(`${context}.amount must be canonical decimal text`);
  }
  if (typeof row.tx_hash !== "string" || !TX_HASH.test(row.tx_hash)) {
    throw invalid(`${context}.tx_hash must be 64 lowercase hex digits`);
  }
  const timestampMs = requireInteger(row.timestamp_ms, 0, `${context}.timestamp_ms`);
  const blockHeight = requireInteger(row.block_height, 1, `${context}.block_height`);
  const blockIndex = requireInteger(row.block_index, 0, `${context}.block_index`);
  const movementIndex = requireInteger(row.movement_index, 0, `${context}.movement_index`);
  if (row.id !== `${row.tx_hash}:${movementIndex}`) {
    throw invalid(`${context}.id must be \`<tx_hash>:<movement_index>\``);
  }
  if (typeof row.result_ok !== "boolean") throw invalid(`${context}.result_ok must be a boolean`);
  if (OFFLINE_KINDS.includes(kind) && !row.result_ok) {
    throw invalid(`${context}: ${kind} rows come only from successful transactions`);
  }
  return Object.freeze({
    id: row.id,
    kind,
    timestamp_ms: timestampMs,
    from_account_id: fromAccountId,
    to_account_id: toAccountId,
    asset_definition_id: requireText(row.asset_definition_id, `${context}.asset_definition_id`),
    amount: row.amount,
    tx_hash: row.tx_hash,
    authority: requireText(row.authority, `${context}.authority`),
    block_height: blockHeight,
    block_index: blockIndex,
    movement_index: movementIndex,
    result_ok: row.result_ok,
  });
}

/**
 * Decode the items of an asset movement page read without `select`.
 *
 * @param {unknown} items `page.items`
 * @param {string} [context]
 * @returns {Array<ReturnType<typeof decodeAssetMovementRow>>}
 */
export function decodeAssetMovementRows(items, context = "asset movements page") {
  if (!Array.isArray(items)) throw invalid(`${context} items must be an array`);
  return items.map((row, index) => decodeAssetMovementRow(row, `${context}.items[${index}]`));
}
