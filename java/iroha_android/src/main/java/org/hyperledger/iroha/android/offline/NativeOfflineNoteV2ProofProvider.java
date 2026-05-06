package org.hyperledger.iroha.android.offline;

/** Java Halo2 proof provider backed by the SDK's native Offline Note V2 prover. */
public final class NativeOfflineNoteV2ProofProvider implements OfflineNoteV2ProofProvider {
  @Override
  public OfflineNoteV2.RecursiveProofV2 proveAudit(final OfflineNoteV2.AuditBundleV2 audit) {
    return OfflineNoteV2Halo2Prover.proveAudit(audit);
  }

  @Override
  public OfflineNoteV2.RecursiveProofV2 proveRedeem(final OfflineNoteV2.RedeemV2 redemption) {
    return OfflineNoteV2Halo2Prover.proveRedeem(redemption);
  }
}
