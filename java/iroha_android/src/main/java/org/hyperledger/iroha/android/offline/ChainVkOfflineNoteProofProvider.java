package org.hyperledger.iroha.android.offline;

/** Offline Note proof provider that proves against a chain-supplied VerifyingKeyBox. */
public final class ChainVkOfflineNoteProofProvider implements OfflineNoteProofProvider {
  private final byte[] vkBoxNorito;

  public ChainVkOfflineNoteProofProvider(final byte[] vkBoxNorito) {
    if (vkBoxNorito == null || vkBoxNorito.length == 0) {
      throw new IllegalArgumentException("vkBoxNorito must not be empty");
    }
    this.vkBoxNorito = vkBoxNorito.clone();
  }

  @Override
  public OfflineNote.RecursiveProof proveAudit(final OfflineNote.AuditBundle audit) {
    final byte[] proofNorito =
        NativeOfflineNoteProver.proveAudit(OfflineNote.encodeAudit(audit), vkBoxNorito);
    return OfflineNote.decodeRecursiveProof(proofNorito);
  }

  @Override
  public OfflineNote.RecursiveProof proveRedeem(final OfflineNote.Redeem redemption) {
    final byte[] proofNorito =
        NativeOfflineNoteProver.proveRedeem(OfflineNote.encodeRedeem(redemption), vkBoxNorito);
    return OfflineNote.decodeRecursiveProof(proofNorito);
  }
}
