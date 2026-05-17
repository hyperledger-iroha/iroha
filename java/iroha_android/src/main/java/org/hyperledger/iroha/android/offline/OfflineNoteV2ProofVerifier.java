package org.hyperledger.iroha.android.offline;

/** Verifies recursive proofs before locally final Offline Note V2 value transfer. */
public interface OfflineNoteV2ProofVerifier {
  boolean verifyAudit(OfflineNoteV2.AuditBundleV2 audit);

  boolean verifyRedeem(OfflineNoteV2.RedeemV2 redemption);
}
