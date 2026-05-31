package org.hyperledger.iroha.android.offline;

/** Java Halo2 proof provider backed by the SDK's native Offline Note prover. */
public final class NativeOfflineNoteProofProvider implements OfflineNoteProofProvider {
  @Override
  public OfflineNote.RecursiveProof proveAudit(final OfflineNote.AuditBundle audit) {
    return OfflineNoteHalo2Prover.proveAudit(audit);
  }

  @Override
  public OfflineNote.RecursiveProof proveRedeem(final OfflineNote.Redeem redemption) {
    return OfflineNoteHalo2Prover.proveRedeem(redemption);
  }
}
