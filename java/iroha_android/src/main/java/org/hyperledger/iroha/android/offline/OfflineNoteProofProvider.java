package org.hyperledger.iroha.android.offline;

/** Builds recursive proofs for direct audit and redeem transactions. */
public interface OfflineNoteProofProvider {
  OfflineNote.RecursiveProof proveAudit(OfflineNote.AuditBundle audit);
  OfflineNote.RecursiveProof proveRedeem(OfflineNote.Redeem redemption);
}
