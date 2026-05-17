package org.hyperledger.iroha.android.offline;

/** Halo2-backed Offline Note V2 proof verifier. */
public final class Halo2OfflineNoteV2ProofVerifier implements OfflineNoteV2ProofVerifier {
  @Override
  public boolean verifyAudit(final OfflineNoteV2.AuditBundleV2 audit) {
    return OfflineNoteV2Halo2Prover.verifyAudit(audit);
  }

  @Override
  public boolean verifyRedeem(final OfflineNoteV2.RedeemV2 redemption) {
    return OfflineNoteV2Halo2Prover.verifyRedeem(redemption);
  }
}
