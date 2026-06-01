package org.hyperledger.iroha.android.offline;

/** Feature flags reported by Torii's Offline V2 readiness endpoint. */
public final class OfflineV2Readiness {
  private final boolean offlineNoteV2;
  private final boolean offlineOneUseKeys;
  private final boolean offlineRecursiveNoteProof;
  private final boolean offlineFountainQrV1;
  private final boolean offlineSyncOptional;
  private final boolean offlineTelemetry;

  public OfflineV2Readiness(
      final boolean offlineNoteV2,
      final boolean offlineOneUseKeys,
      final boolean offlineRecursiveNoteProof,
      final boolean offlineFountainQrV1,
      final boolean offlineSyncOptional,
      final boolean offlineTelemetry) {
    this.offlineNoteV2 = offlineNoteV2;
    this.offlineOneUseKeys = offlineOneUseKeys;
    this.offlineRecursiveNoteProof = offlineRecursiveNoteProof;
    this.offlineFountainQrV1 = offlineFountainQrV1;
    this.offlineSyncOptional = offlineSyncOptional;
    this.offlineTelemetry = offlineTelemetry;
  }

  public boolean offlineNoteV2() {
    return offlineNoteV2;
  }

  public boolean offlineOneUseKeys() {
    return offlineOneUseKeys;
  }

  public boolean offlineRecursiveNoteProof() {
    return offlineRecursiveNoteProof;
  }

  public boolean offlineFountainQrV1() {
    return offlineFountainQrV1;
  }

  public boolean offlineSyncOptional() {
    return offlineSyncOptional;
  }

  public boolean offlineTelemetry() {
    return offlineTelemetry;
  }
}
