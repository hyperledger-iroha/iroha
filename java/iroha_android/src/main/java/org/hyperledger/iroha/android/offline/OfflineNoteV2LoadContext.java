package org.hyperledger.iroha.android.offline;

/** Torii issuer load context needed before deriving a wallet-owned issue commitment. */
public final class OfflineNoteV2LoadContext {
  private final String operationId;
  private final String lineageId;
  private final long localRevision;
  private final OfflineNoteV2.KeyCertificateV2 keyCertificate;

  public OfflineNoteV2LoadContext(
      final String operationId,
      final String lineageId,
      final long localRevision,
      final OfflineNoteV2.KeyCertificateV2 keyCertificate) {
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

  public OfflineNoteV2.KeyCertificateV2 keyCertificate() {
    return keyCertificate;
  }
}
