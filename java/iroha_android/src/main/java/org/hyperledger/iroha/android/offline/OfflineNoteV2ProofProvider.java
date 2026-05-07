package org.hyperledger.iroha.android.offline;

/** Builds recursive proofs for direct audit and redeem transactions. */
public interface OfflineNoteV2ProofProvider {
  OfflineNoteV2.RecursiveProofV2 proveAudit(OfflineNoteV2.AuditBundleV2 audit);
  OfflineNoteV2.RecursiveProofV2 proveRedeem(OfflineNoteV2.RedeemV2 redemption);
}
