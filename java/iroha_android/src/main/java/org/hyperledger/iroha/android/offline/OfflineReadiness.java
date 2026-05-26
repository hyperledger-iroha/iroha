package org.hyperledger.iroha.android.offline;

/** Feature flags reported by Torii's Offline readiness endpoint. */
public final class OfflineReadiness {
  private final boolean offlineNote;
  private final boolean offlineOneUseKeys;
  private final boolean offlineRecursiveNoteProof;
  private final boolean offlineFountainQr;
  private final boolean offlineSyncOptional;
  private final boolean offlineTelemetry;

  public OfflineReadiness(
      final boolean offlineNote,
      final boolean offlineOneUseKeys,
      final boolean offlineRecursiveNoteProof,
      final boolean offlineFountainQr,
      final boolean offlineSyncOptional,
      final boolean offlineTelemetry) {
    this.offlineNote = offlineNote;
    this.offlineOneUseKeys = offlineOneUseKeys;
    this.offlineRecursiveNoteProof = offlineRecursiveNoteProof;
    this.offlineFountainQr = offlineFountainQr;
    this.offlineSyncOptional = offlineSyncOptional;
    this.offlineTelemetry = offlineTelemetry;
  }

  public boolean offlineNote() {
    return offlineNote;
  }

  public boolean offlineOneUseKeys() {
    return offlineOneUseKeys;
  }

  public boolean offlineRecursiveNoteProof() {
    return offlineRecursiveNoteProof;
  }

  public boolean offlineFountainQr() {
    return offlineFountainQr;
  }

  public boolean offlineSyncOptional() {
    return offlineSyncOptional;
  }

  public boolean offlineTelemetry() {
    return offlineTelemetry;
  }
}
