import test from "node:test";
import assert from "node:assert/strict";
import { ed25519 } from "@noble/curves/ed25519";
import { AccountAddress } from "../src/address.js";
import { NetworkId } from "../src/networkId.js";
import { blake2b256 } from "../src/blake2b.js";
import { buildBrowserOwnedNftTransferPayloadV1, buildOwnedNftTransferInstructionV1, readOwnedNftInventoryV1, normalizeNftInventoryItemV1, nftContentHashV1, nftSaleOfferHashV1, buildNftMarketInstructionV1, normalizeNftSaleRecordV1, encodeNftMarketValueV1, decodeNftMarketValueV1 } from "../src/nft.js";
import { browserTransactionPayloadHashHex, validateBrowserExecutableBatchSignable, finalizeBrowserExecutableBatchTransaction, browserSignedTransactionHashHex, buildBrowserInstructionTransactionPayload, validateBrowserInstructionTransactionSignable } from "../src/transactionCodec.js";
import { noritoEncodeInstructionBoxArchive, noritoDecodeInstructionBoxArchive } from "../src/norito.js";

const key = new Uint8Array(32).fill(7), publicKey = ed25519.getPublicKey(key);
const account = n => AccountAddress.fromAccount({ algorithm: "ed25519", publicKey: ed25519.getPublicKey(new Uint8Array(32).fill(n)) }).toI105();
const owner = account(7), destination = account(9);
const nftId = "sora_skin_01$sora_cars.universal";
const networkId = NetworkId.fromBytes(new Uint8Array(32).fill(17));
const row = (id = nftId, owned_by = owner, metadata = {}) => ({ id, owned_by, metadata });
const page = (items, next = null, limit = 2) => ({ items, pagination: { limit, has_more: next !== null, next_cursor: next } });

test("owned NFT transfer uses canonical native bytes, pins source to signer, and checks signature/network", () => {
  const input = { networkId, authority: owner, nftId, destinationAccountId: destination, feePayment: { payer: "authority", chargeLimits: [{ kind: "nexus", assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM", maxAmount: "0.1" }] }, creationTimeMs: 1, ttlMs: 100_000 };
  const payloadBytes = buildBrowserOwnedNftTransferPayloadV1(input);
  const signable = { networkId, authority: owner, signingPublicKey: publicKey, payloadBytes, payloadHashHex: browserTransactionPayloadHashHex(payloadBytes, 753) };
  validateBrowserExecutableBatchSignable(signable);
  const hash = Uint8Array.from(blake2b256(payloadBytes)); hash[31] |= 1;
  const signature = ed25519.sign(hash, key);
  const signed = finalizeBrowserExecutableBatchTransaction(signable, signature, publicKey);
  assert.equal(browserSignedTransactionHashHex(signed.signedTransaction, 753), signed.hashHex);
  assert.throws(() => validateBrowserExecutableBatchSignable({ ...signable, networkId: NetworkId.fromBytes(new Uint8Array(32).fill(19)) }), /network/i);
  assert.throws(() => finalizeBrowserExecutableBatchTransaction(signable, new Uint8Array(64), publicKey), /signature/i);
  assert.throws(() => buildBrowserOwnedNftTransferPayloadV1({ ...input, ownerAccountId: destination }), /unsupported field/);
  assert.throws(() => buildBrowserOwnedNftTransferPayloadV1({ ...input, entries: [] }), /unsupported field/);
  assert.throws(() => buildOwnedNftTransferInstructionV1({ ownerAccountId: owner, nftId: "skin$sora", destinationAccountId: destination }), /canonical/);
});

test("owned inventory paginates with strict owner/domain checks and retains endpoint-reported status", async () => {
  const requests = [], pages = [page([row()], "YWJj"), page([row("sora_skin_02$sora_cars.universal")])];
  const inventory = await readOwnedNftInventoryV1({ async listExplorerNfts(options) { requests.push(options); return pages.shift(); } }, { ownerAccountId: owner, domain: "sora_cars.universal", limit: 2 });
  assert.equal(inventory.verification, "endpoint_reported");
  assert.equal(inventory.items.length, 2);
  assert.equal(requests[0].ownedBy, owner);
  assert.equal(requests[1].cursor, "YWJj");
  assert.equal(requests[0].domain, "sora_cars.universal");
});

test("inventory refuses contradictory ownership, forged domain, duplicate items, stuck cursors and oversized metadata", async () => {
  const read = (pages, extra = {}) => readOwnedNftInventoryV1({ async listExplorerNfts() { return pages.shift(); } }, { ownerAccountId: owner, domain: "sora_cars.universal", limit: 2, ...extra });
  await assert.rejects(read([page([row(nftId, destination)])]), /another owner's/);
  await assert.rejects(read([page([row("skin$forged.universal")])]), /another domain/);
  await assert.rejects(read([page([row()], "YWJj"), page([row()])]), /repeated an item/);
  await assert.rejects(read([page([row()], "YWJj"), page([row("b$sora_cars.universal")], "YWJj")]), /bounded progress/);
  await assert.rejects(read([page([], "YWJj")]), /bounded progress/);
  await assert.rejects(read([page([row()], "YWJj")], { maxItems: 1 }), /bounded progress/);
  await assert.rejects(read([page([row(nftId, owner, { huge: "x".repeat(16 * 1024) })])]), /byte bound/);
  await assert.rejects(read([{ ...page([]), extra: true }]), /unsupported field/);
  const cycle = {}; cycle.nested = cycle;
  assert.throws(() => normalizeNftInventoryItemV1(row(nftId, owner, cycle)), /structural bounds/);
  assert.throws(() => normalizeNftInventoryItemV1(row(nftId, owner, { absent: undefined })), /plain object/);
});

test("inventory aborts before a query and never accepts options that alter the fixed owner filter", async () => {
  let calls = 0;
  const client = { async listExplorerNfts() { calls++; return page([]); } };
  const controller = new AbortController(); controller.abort();
  await assert.rejects(readOwnedNftInventoryV1(client, { ownerAccountId: owner, signal: controller.signal }));
  await assert.rejects(readOwnedNftInventoryV1(client, { ownerAccountId: owner, ownedBy: destination }), /unsupported field/);
  assert.equal(calls, 0);
});

test("native NFT offer codecs bind all purchase terms and the exact metadata content commitment", () => {
  const hash = nftContentHashV1({ a: 1, b: "violet" });
  assert.equal(hash, nftContentHashV1({ b: "violet", a: 1 }));
  assert.notEqual(hash, nftContentHashV1({ a: 1, b: "red" }));
  const offer = { network_id: networkId.toString(), offer_id: hash, nft_id: nftId, seller: owner, payment_asset: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM", price: "3", expires_at_height: "100", reserved_buyer: destination, metadata_hash: hash };
  const record = { version: 1, offer, offer_hash: nftSaleOfferHashV1(offer), custody: account(11), status: { kind: "open", value: null }, created_at_height: "1", closed_at_height: null };
  assert.equal(normalizeNftSaleRecordV1(record, networkId.toString()).offer_hash, record.offer_hash);
  for (const name of ["OfferNftV1", "BuyNftV1", "CancelNftOfferV1"]) {
    const payload = name === "OfferNftV1" ? { offer_id: offer.offer_id, nft_id: nftId, payment_asset: offer.payment_asset, price: offer.price, expires_at_height: offer.expires_at_height, reserved_buyer: null }
      : name === "BuyNftV1" ? { offer } : { offer_id: offer.offer_id, expected_offer_hash: record.offer_hash };
    const instruction = buildNftMarketInstructionV1(name, payload);
    assert.deepEqual(noritoDecodeInstructionBoxArchive(noritoEncodeInstructionBoxArchive(instruction, 753), 753), instruction);
    assert.throws(() => buildNftMarketInstructionV1(name, { ...payload, arbitrary_code: [] }), /exact/);
  }
  const bytes = encodeNftMarketValueV1("NftSaleRecordV1", record);
  assert.throws(() => decodeNftMarketValueV1("NftSaleRecordV1", Buffer.concat([bytes, Buffer.of(0)])));
  for (const altered of [{ price: "1" }, { seller: destination }, { nft_id: "other$sora_cars.universal" }, { metadata_hash: nftContentHashV1({ other: true }) }, { expires_at_height: "101" }, { reserved_buyer: null }]) {
    assert.notEqual(nftSaleOfferHashV1({ ...offer, ...altered }), record.offer_hash);
    assert.throws(() => normalizeNftSaleRecordV1({ ...record, offer: { ...offer, ...altered } }, networkId.toString()), /contradicts/);
  }
  assert.throws(() => normalizeNftSaleRecordV1(record, NetworkId.fromBytes(new Uint8Array(32).fill(23)).toString()), /network/);
  for (const status of [{ kind: "purchased", value: destination }, { kind: "cancelled", value: null }]) {
    const closed = { ...record, status, closed_at_height: "2" };
    assert.equal(normalizeNftSaleRecordV1(closed, networkId.toString()).status.kind, status.kind);
  }
  const payloadBytes = buildBrowserInstructionTransactionPayload({ networkId, authority: destination, instructions: [buildNftMarketInstructionV1("BuyNftV1", { offer })], feePayment: { payer: "authority", chargeLimits: [] }, creationTimeMs: 1 });
  validateBrowserInstructionTransactionSignable({ networkId, authority: destination, signingPublicKey: ed25519.getPublicKey(new Uint8Array(32).fill(9)), payloadBytes, payloadHashHex: browserTransactionPayloadHashHex(payloadBytes, 753) });
});
