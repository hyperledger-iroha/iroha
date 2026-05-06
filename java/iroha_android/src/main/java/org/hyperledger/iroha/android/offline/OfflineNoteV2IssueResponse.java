package org.hyperledger.iroha.android.offline;

import java.util.Arrays;

/** Issuer response after Torii accepts the supplied note commitment. */
public final class OfflineNoteV2IssueResponse {
  private final byte[] noteCommitment;
  private final String operationId;
  private final String lineageId;
  private final long localRevision;
  private final OfflineNoteV2.KeyCertificateV2 keyCertificate;
  private final String settlementEntryHashHex;

  public OfflineNoteV2IssueResponse(
      final byte[] noteCommitment,
      final String operationId,
      final String lineageId,
      final long localRevision,
      final OfflineNoteV2.KeyCertificateV2 keyCertificate,
      final String settlementEntryHashHex) {
    this.noteCommitment = Arrays.copyOf(noteCommitment, noteCommitment.length);
    this.operationId = operationId;
    this.lineageId = lineageId;
    this.localRevision = localRevision;
    this.keyCertificate = keyCertificate;
    this.settlementEntryHashHex = settlementEntryHashHex;
  }

  public byte[] noteCommitment() {
    return Arrays.copyOf(noteCommitment, noteCommitment.length);
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

  public String settlementEntryHashHex() {
    return settlementEntryHashHex;
  }
}
