package org.hyperledger.iroha.android.offline;

/** Offline Note proof verifier that verifies against a chain-supplied VerifyingKeyBox. */
public final class ChainVkOfflineNoteProofVerifier implements OfflineNoteProofVerifier {
  private final byte[] vkBoxNorito;

  public ChainVkOfflineNoteProofVerifier(final byte[] vkBoxNorito) {
    if (vkBoxNorito == null || vkBoxNorito.length == 0) {
      throw new IllegalArgumentException("vkBoxNorito must not be empty");
    }
    this.vkBoxNorito = vkBoxNorito.clone();
  }

  @Override
  public boolean verifyAudit(final OfflineNote.AuditBundle audit) {
    return NativeOfflineNoteProver.verifyAudit(OfflineNote.encodeAudit(audit), vkBoxNorito);
  }

  @Override
  public boolean verifyRedeem(final OfflineNote.Redeem redemption) {
    return NativeOfflineNoteProver.verifyRedeem(OfflineNote.encodeRedeem(redemption), vkBoxNorito);
  }
}
