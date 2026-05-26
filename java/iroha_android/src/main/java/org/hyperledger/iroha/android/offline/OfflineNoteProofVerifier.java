package org.hyperledger.iroha.android.offline;

/** Verifies recursive proofs before locally final Offline Note value transfer. */
public interface OfflineNoteProofVerifier {
  boolean verifyAudit(OfflineNote.AuditBundle audit);

  boolean verifyRedeem(OfflineNote.Redeem redemption);
}
