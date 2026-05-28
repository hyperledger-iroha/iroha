package org.hyperledger.iroha.android.offline;

/** Halo2-backed Offline Note proof verifier. */
public final class Halo2OfflineNoteProofVerifier implements OfflineNoteProofVerifier {
  @Override
  public boolean verifyAudit(final OfflineNote.AuditBundle audit) {
    return OfflineNoteHalo2Prover.verifyAudit(audit);
  }

  @Override
  public boolean verifyRedeem(final OfflineNote.Redeem redemption) {
    return OfflineNoteHalo2Prover.verifyRedeem(redemption);
  }
}
