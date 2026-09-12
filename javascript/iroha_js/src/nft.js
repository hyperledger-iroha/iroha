import { requireNetworkPrefix } from "./networkPrefix.js";
/** Browser-safe native NFT ownership reads and owner-authorized transfers. */
import { ensureCanonicalAccountId } from "./normalizers.js";
import { buildTransferNftInstruction } from "./instructionBuilders.js";
import { noritoEncodeInstructionBoxArchive, noritoDecodeInstructionBoxArchive } from "./norito.js";
import { buildBrowserExecutableBatchPayload } from "./transactionCodec.js";
import { noritoEncodeNftMarketValueV1, noritoDecodeNftMarketValueV1 } from "./norito.js";
import { NFT_MARKET_INSTRUCTION_NAMES_V1 } from "./noritoNftMarketCodecs.js";
import { blake2b256 } from "./blake2b.js";
import { computeHashLiteralCrc } from "./hashLiteralCrc.js";
import { Buffer } from "buffer";

export { noritoEncodeNftMarketValueV1 as encodeNftMarketValueV1, noritoDecodeNftMarketValueV1 as decodeNftMarketValueV1 } from "./norito.js";

function hashLiteral(bytes) {
  const hash = Uint8Array.from(blake2b256(bytes)); hash[31] |= 1;
  const body = Buffer.from(hash).toString("hex").toUpperCase();
  return `hash:${body}#${computeHashLiteralCrc("hash", body)}`;
}
/** Versioned native custody/settlement policy identity; qualification is separate. */
export function nftMarketProfileIdV1() {
  return hashLiteral(Buffer.from("iroha:nft-market:v1:owner-reservation:exact-terms:atomic-payment:immutable-content:height-expiry:one-shot"));
}
/** Exact native NFT content commitment, including canonical Norito metadata serialization. */
export function nftContentHashV1(metadata) {
  return hashLiteral(noritoEncodeNftMarketValueV1("Metadata", cloneMetadata(metadata)));
}
/** Exact domain-separated commitment to immutable sale terms. */
export function nftSaleOfferHashV1(offer) {
  return hashLiteral(Buffer.concat([Buffer.from("iroha:nft:exact-price-offer:v1\0"), noritoEncodeNftMarketValueV1("NftSaleOfferV1", offer)]));
}
/** Typed native instructions only; never substitutes an unsafe two-transfer purchase batch. */
export function buildNftMarketInstructionV1(name, value) {
  if (!NFT_MARKET_INSTRUCTION_NAMES_V1.includes(name)) throw new TypeError("unknown native NFT market instruction");
  return { [name]: noritoDecodeNftMarketValueV1(name, noritoEncodeNftMarketValueV1(name, value)) };
}
/** Validate a complete endpoint-reported native quote against an independently pinned network. */
export function normalizeNftSaleRecordV1(value, expectedNetworkId) {
  const record = noritoDecodeNftMarketValueV1("NftSaleRecordV1", noritoEncodeNftMarketValueV1("NftSaleRecordV1", value));
  if (record.offer.network_id !== expectedNetworkId || record.offer_hash !== nftSaleOfferHashV1(record.offer)
      || record.offer.price === "0" || BigInt(record.offer.expires_at_height) <= BigInt(record.created_at_height)
      || (record.status.kind === "open") !== (record.closed_at_height === null)
      || record.closed_at_height !== null && BigInt(record.closed_at_height) < BigInt(record.created_at_height)) {
    throw new TypeError("NFT quote contradicts its network, immutable terms or lifecycle");
  }
  return record;
}

export const NFT_INVENTORY_MAX_ITEMS_V1 = 256;
export const NFT_INVENTORY_METADATA_MAX_BYTES_V1 = 16 * 1024;

function object(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)
      || ![Object.prototype, null].includes(Object.getPrototypeOf(value))) {
    throw new TypeError(`${label} must be a plain object`);
  }
  return value;
}
function exactAccount(value, label) {
  const canonical = ensureCanonicalAccountId(value, label);
  if (canonical !== value) throw new TypeError(`${label} must be canonical`);
  return canonical;
}
function exactKeys(value, allowed, label) {
  object(value, label);
  for (const key of Object.keys(value)) if (!allowed.includes(key)) {
    throw new TypeError(`${label} contains unsupported field ${key}`);
  }
}
function cloneMetadata(value) {
  object(value, "NFT metadata");
  let nodes = 0;
  const visit = (item, depth) => {
    if (++nodes > 4096 || depth > 16) throw new TypeError("NFT metadata exceeds structural bounds");
    if (item === null || typeof item === "string" || typeof item === "boolean") return;
    if (typeof item === "number" && Number.isFinite(item)) return;
    if (Array.isArray(item)) { for (const entry of item) visit(entry, depth + 1); return; }
    object(item, "NFT metadata value");
    for (const entry of Object.values(item)) visit(entry, depth + 1);
  };
  visit(value, 0);
  const json = JSON.stringify(value);
  if (new TextEncoder().encode(json).byteLength > NFT_INVENTORY_METADATA_MAX_BYTES_V1) {
    throw new TypeError("NFT metadata exceeds byte bound");
  }
  return JSON.parse(json);
}

/** Native codec round-trip rejects malformed identifiers; source is always the owner. */
export function buildOwnedNftTransferInstructionV1(input) {
  exactKeys(input, ["ownerAccountId", "nftId", "destinationAccountId", "networkPrefix"], "NFT transfer");
  const networkPrefix = requireNetworkPrefix(input.networkPrefix);
  const owner = exactAccount(input.ownerAccountId, "ownerAccountId");
  const destination = exactAccount(input.destinationAccountId, "destinationAccountId");
  const encoded = noritoEncodeInstructionBoxArchive(buildTransferNftInstruction({
    sourceAccountId: owner, nftId: input.nftId, destinationAccountId: destination,
  }), networkPrefix);
  const instruction = noritoDecodeInstructionBoxArchive(encoded, networkPrefix);
  if (instruction.Transfer.Nft.object !== input.nftId) {
    throw new TypeError("nftId must use its exact canonical name$domain.dataspace form");
  }
  return instruction;
}

/** Construct one native NFT transfer for an external wallet; never accepts custody keys. */
export function buildBrowserOwnedNftTransferPayloadV1(input) {
  exactKeys(input, ["networkId", "authority", "nftId", "destinationAccountId", "feePayment", "metadata", "creationTimeMs", "ttlMs", "nonce", "networkPrefix"], "NFT transfer transaction");
  const { nftId, destinationAccountId, ...transaction } = input;
  const instruction = buildOwnedNftTransferInstructionV1({
    ownerAccountId: transaction.authority, nftId, destinationAccountId, networkPrefix: transaction.networkPrefix,
  });
  return buildBrowserExecutableBatchPayload({
    ...transaction, entries: [{ kind: "instruction", instruction }],
  });
}

/** Normalize endpoint-reported ownership. Metadata does not prove item authenticity. */
export function normalizeNftInventoryItemV1(value, networkPrefix) {
  exactKeys(value, ["id", "owned_by", "metadata"], "NFT record");
  const owner = exactAccount(value.owned_by, "NFT owner");
  buildOwnedNftTransferInstructionV1({ ownerAccountId: owner, nftId: value.id, destinationAccountId: owner, networkPrefix });
  return { id: value.id, ownedBy: owner, metadata: cloneMetadata(value.metadata) };
}

/**
 * Read all bounded owner-filtered cursor pages from a ToriiBrowserClient-compatible reader.
 * This detects malformed/contradictory pages but does not authenticate ledger inclusion.
 * A concurrent transfer may change the inventory between pages; callers refresh before signing.
 */
export async function readOwnedNftInventoryV1(client, options) {
  exactKeys(options, ["ownerAccountId", "domain", "limit", "maxItems", "signal", "networkPrefix"], "NFT inventory options");
  const networkPrefix = requireNetworkPrefix(options.networkPrefix);
  const owner = exactAccount(options.ownerAccountId, "ownerAccountId");
  const limit = options.limit ?? 100;
  const maxItems = options.maxItems ?? NFT_INVENTORY_MAX_ITEMS_V1;
  if (!Number.isInteger(limit) || limit < 1 || limit > 100
      || !Number.isInteger(maxItems) || maxItems < 1 || maxItems > NFT_INVENTORY_MAX_ITEMS_V1) {
    throw new TypeError("NFT inventory pagination exceeds bounds");
  }
  if (options.domain !== undefined) {
    buildOwnedNftTransferInstructionV1({ ownerAccountId: owner, nftId: `inventory$${options.domain}`, destinationAccountId: owner, networkPrefix });
  }
  let cursor;
  const cursors = new Set(), ids = new Set(), items = [];
  for (let pageIndex = 0; pageIndex <= maxItems; pageIndex += 1) {
    options.signal?.throwIfAborted();
    const page = await client.listExplorerNfts({ ownedBy: owner, domain: options.domain, limit, cursor, signal: options.signal });
    exactKeys(page, ["pagination", "items"], "NFT page");
    exactKeys(page.pagination, ["limit", "next_cursor", "has_more"], "NFT pagination");
    if (!Array.isArray(page.items) || page.items.length > limit || page.pagination.limit !== limit
        || typeof page.pagination.has_more !== "boolean") throw new TypeError("Invalid NFT page geometry");
    for (const value of page.items) {
      const item = normalizeNftInventoryItemV1(value, networkPrefix);
      if (item.ownedBy !== owner) throw new TypeError("NFT query returned another owner's item");
      if (options.domain !== undefined && item.id.slice(item.id.indexOf("$") + 1) !== options.domain) throw new TypeError("NFT query returned another domain's item");
      if (ids.has(item.id)) throw new TypeError("NFT inventory repeated an item across pages");
      ids.add(item.id); items.push(item);
      if (items.length > maxItems) throw new TypeError("NFT inventory exceeds item bound");
    }
    const next = page.pagination.next_cursor;
    if (!page.pagination.has_more) {
      if (next !== null) throw new TypeError("Finished NFT inventory retained a cursor");
      return { ownerAccountId: owner, verification: "endpoint_reported", items };
    }
    if (!page.items.length || items.length >= maxItems || typeof next !== "string" || next.length > 1424
        || !/^[A-Za-z0-9_-]+$/u.test(next) || cursors.has(next)) {
      throw new TypeError("NFT inventory cursor did not make bounded progress");
    }
    cursors.add(next); cursor = next;
  }
  throw new TypeError("NFT inventory exceeded page bound");
}
