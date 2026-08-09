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

export interface ToriiBrowserExplorerCursorMeta {
  limit: number;
  next_cursor: string | null;
  has_more: boolean;
}

export interface ToriiBrowserExplorerCursorPage<T = unknown> {
  pagination: ToriiBrowserExplorerCursorMeta;
  items: ReadonlyArray<T>;
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
