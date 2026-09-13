import type { BrowserExecutableBatchInput } from "./transaction-codec.js";
import type { ToriiBrowserClient } from "./torii-browser.js";

export type NftCounterV1 = string | number | bigint;
export interface NftCustodyPurposeV1 { kind: 'sale' | 'game_wager' | 'game_resource'; value: null }
export interface NftCustodyRecordV1 { version: 1; network_id: string; reservation_id: string; purpose: NftCustodyPurposeV1; nft_id: string; custody: string; original_owner: string; metadata_hash: string; released_to: string | null }
export interface NftSaleOfferV1 {
  network_id: string; offer_id: string; nft_id: string; seller: string;
  payment_asset: string; price: string; expires_at_height: NftCounterV1;
  reserved_buyer: string | null; metadata_hash: string;
}
export interface NftSaleRecordV1 {
  version: 1; offer: NftSaleOfferV1; offer_hash: string; custody: string;
  status: { kind: "open"; value: null } | { kind: "purchased"; value: string } | { kind: "cancelled"; value: null };
  created_at_height: NftCounterV1; closed_at_height: NftCounterV1 | null;
}
export interface OfferNftV1 {
  offer_id: string; nft_id: string; payment_asset: string; price: string;
  expires_at_height: NftCounterV1; reserved_buyer: string | null;
}
export function encodeNftMarketValueV1(name: string, value: unknown): Uint8Array;
export function decodeNftMarketValueV1(name: string, bytes: Uint8Array): unknown;
export function nftContentHashV1(metadata: Record<string, unknown>): string;
export function nftMarketProfileIdV1(): string;
export function nftSaleOfferHashV1(offer: NftSaleOfferV1): string;
export function normalizeNftSaleRecordV1(value: unknown, expectedNetworkId: string): NftSaleRecordV1;
export function buildNftMarketInstructionV1(name: "OfferNftV1", value: OfferNftV1): { OfferNftV1: OfferNftV1 };
export function buildNftMarketInstructionV1(name: "BuyNftV1", value: { offer: NftSaleOfferV1 }): { BuyNftV1: { offer: NftSaleOfferV1 } };
export function buildNftMarketInstructionV1(name: "CancelNftOfferV1", value: { offer_id: string; expected_offer_hash: string }): { CancelNftOfferV1: { offer_id: string; expected_offer_hash: string } };

export const NFT_INVENTORY_MAX_ITEMS_V1: 256;
export const NFT_INVENTORY_METADATA_MAX_BYTES_V1: number;
export interface NftInventoryItemV1 {
  id: string;
  ownedBy: string;
  metadata: Record<string, unknown>;
}
export interface NftInventoryV1 {
  ownerAccountId: string;
  verification: "endpoint_reported";
  items: NftInventoryItemV1[];
}
export interface OwnedNftTransferV1 {
  networkPrefix: number;
  ownerAccountId: string;
  nftId: string;
  destinationAccountId: string;
}
export function buildOwnedNftTransferInstructionV1(input: OwnedNftTransferV1): {
  Transfer: { Nft: { source: string; object: string; destination: string } };
};
export function buildBrowserOwnedNftTransferPayloadV1(input: Omit<BrowserExecutableBatchInput, "entries"> & {
  nftId: string;
  destinationAccountId: string;
}): Uint8Array;
export function normalizeNftInventoryItemV1(value: unknown, networkPrefix: number): NftInventoryItemV1;
export function readOwnedNftInventoryV1(client: Pick<ToriiBrowserClient, "listExplorerNfts">, options: {
  ownerAccountId: string;
  networkPrefix: number;
  domain?: string;
  limit?: number;
  maxItems?: number;
  signal?: AbortSignal;
}): Promise<NftInventoryV1>;
