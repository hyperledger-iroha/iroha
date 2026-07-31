/**
 * Lifecycle and custody fields projected for every normalized repo agreement.
 *
 * This declaration fragment is internal to the root package declaration; the
 * public `ToriiRepoAgreement` API continues to expose these fields directly.
 */
export interface RepoAgreementLifecycleFields {
  cashSource: string;
  collateralCustodyAsset: string;
  settlementTimestampMs: number | null;
  status: "active" | "settled";
}
