import type { CanonicalRequestAuth, NetworkId } from "./index.js";
/** Current private wallet read selectors: exact, nonzero lowercase 32-byte hex. */
export interface KagemushaWalletLoadSelectionV1 {
  schemeId: string;
  walletId: string;
  requestId: string;
}
export interface KagemushaWalletLedgerSelectionV1 {
  schemeId: string;
  walletId: string;
  /** Exact marked Iroha entrypoint hash as lowercase 32-byte hex. */
  entrypointHash: string;
}
export type KagemushaWalletEpochSelectionV1<T> = T & {
  /** Canonical u64 decimal, at least 2. */
  boundary: string;
};
export interface KagemushaWalletReadOptionsV1 {
  /** Exact canonical I105 caller; aliases are not accepted for private evidence reads. */
  canonicalAuth: CanonicalRequestAuth;
  signal?: AbortSignal;
}
/** Bounded original bytes only. Independently verify before admitting value or settlement. */
export interface KagemushaWalletOriginalV1<T> {
  readonly selection: Readonly<T>;
  readonly accountId: string;
  readonly networkId: NetworkId;
  readonly canonicalResponseOriginal: Uint8Array;
}
