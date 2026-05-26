package org.hyperledger.iroha.android.offline;

/** Torii issuer load context needed before deriving a wallet-owned issue commitment. */
public final class OfflineNoteLoadContext {
  private final String operationId;
  private final String lineageId;
  private final long localRevision;
  private final OfflineNote.KeyCertificate keyCertificate;

  public OfflineNoteLoadContext(
      final String operationId,
      final String lineageId,
      final long localRevision,
      final OfflineNote.KeyCertificate keyCertificate) {
    this.operationId = operationId;
    this.lineageId = lineageId;
    this.localRevision = localRevision;
    this.keyCertificate = keyCertificate;
  }

  public String operationId() {
    return operationId;
  }

  public String lineageId() {
    return lineageId;
  }

  public long localRevision() {
    return localRevision;
  }

  public OfflineNote.KeyCertificate keyCertificate() {
    return keyCertificate;
  }
}
