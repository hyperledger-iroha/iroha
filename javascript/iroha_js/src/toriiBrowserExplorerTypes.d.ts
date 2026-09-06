import type { NumericLike } from "../index.js";

export interface ToriiBrowserExplorerCursorOptions {
  cursor?: string;
  limit?: NumericLike;
  signal?: AbortSignal;
}

export interface ToriiBrowserExplorerAccountsOptions
  extends ToriiBrowserExplorerCursorOptions {
  domain?: string;
  withAsset?: string;
  addressFormat?: string;
}

export interface ToriiBrowserExplorerDomainsOptions
  extends ToriiBrowserExplorerCursorOptions {
  ownedBy?: string;
}

export interface ToriiBrowserExplorerAssetDefinitionsOptions
  extends ToriiBrowserExplorerCursorOptions {
  owningDomain?: string;
  owning_domain?: string;
  ownedBy?: string;
}

export interface ToriiBrowserExplorerAssetsOptions
  extends ToriiBrowserExplorerCursorOptions {
  ownedBy?: string;
  definition?: string;
  assetId?: string;
}

export interface ToriiBrowserExplorerOwnedDomainOptions
  extends ToriiBrowserExplorerCursorOptions {
  ownedBy?: string;
  domain?: string;
}

/** Options shared by snapshot-bound Explorer chain-history routes. */
export interface ToriiBrowserExplorerHistoryOptions
  extends ToriiBrowserExplorerCursorOptions {}

/** Filters accepted by both transaction-history projections. */
export interface ToriiBrowserExplorerTransactionHistoryOptions
  extends ToriiBrowserExplorerHistoryOptions {
  authority?: string;
  block?: NumericLike;
  status?: "committed" | "rejected";
  assetId?: string;
  asset_id?: string;
}

/** Filters accepted by both instruction-history projections. */
export interface ToriiBrowserExplorerInstructionHistoryOptions
  extends ToriiBrowserExplorerHistoryOptions {
  account?: string;
  authority?: string;
  kind?: string;
  transactionHash?: string;
  transaction_hash?: string;
  transactionStatus?: "committed" | "rejected";
  transaction_status?: "committed" | "rejected";
  block?: NumericLike;
  assetId?: string;
  asset_id?: string;
}

export interface ToriiBrowserExplorerCursorMeta {
  limit: number;
  next_cursor: string | null;
  has_more: boolean;
}

export interface ToriiBrowserExplorerCursorPage<T = unknown> {
  pagination: ToriiBrowserExplorerCursorMeta;
  items: ReadonlyArray<T>;
}

/** Snapshot-bound continuation metadata returned by chain-history routes. */
export interface ToriiBrowserExplorerHistoryCursorMeta {
  limit: number;
  snapshot_height: number;
  snapshot_hash: string | null;
  next_cursor: string | null;
  has_more: boolean;
}

export interface ToriiBrowserExplorerHistoryPage<T = unknown> {
  pagination: ToriiBrowserExplorerHistoryCursorMeta;
  items: ReadonlyArray<T>;
}

export interface ToriiBrowserExplorerLatestHistoryPage<T = unknown>
  extends ToriiBrowserExplorerHistoryPage<T> {
  sampled_at: string;
}

export interface ToriiBrowserExplorerBlock {
  hash: string;
  height: number;
  created_at: string;
  prev_block_hash: string | null;
  transactions_hash: string | null;
  transactions_rejected: number;
  transactions_total: number;
}

export interface ToriiBrowserExplorerTransaction {
  authority: string;
  hash: string;
  block: number;
  created_at: string;
  executable: string;
  status: string;
}

export interface ToriiBrowserExplorerInstructionBox {
  encoded: string;
  framed_sha256: string;
  json: unknown;
}

export interface ToriiBrowserExplorerInstruction {
  authority: string;
  created_at: string;
  kind: string;
  box: ToriiBrowserExplorerInstructionBox;
  transaction_hash: string;
  transaction_status: string;
  block: number;
  index: number;
}

export interface ToriiBrowserExplorerAssetDefinition {
  readonly id: string;
  /** Null denotes an intentionally unowned global definition. */
  readonly owning_domain: string | null;
  readonly mintable: string;
  readonly logo: string | null;
  readonly metadata: Readonly<Record<string, unknown>>;
  readonly owned_by: string;
  readonly assets: number;
  readonly total_quantity: string;
  readonly locked_quantity: string | null;
  readonly circulating_quantity: string | null;
}
