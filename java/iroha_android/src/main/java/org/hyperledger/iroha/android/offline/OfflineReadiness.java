package org.hyperledger.iroha.android.offline;

/** Feature flags reported by Torii's Offline readiness endpoint. */
public final class OfflineReadiness {
  private final boolean offlineNote;
  private final boolean offlineOneUseKeys;
  private final boolean offlineRecursiveNoteProof;
  private final boolean offlineFountainQr;
  private final boolean offlineSyncOptional;
  private final boolean offlineTelemetry;
  private final boolean offlineKagemushaRecursiveCompactAvailable;
  private final String offlineKagemushaRecursiveCompactMode;
  private final Integer offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion;
  private final String offlineKagemushaRecursiveCompactCircuitId;
  private final boolean offlineKagemushaRecursiveCompactArtifactsAvailable;

  public OfflineReadiness(
      final boolean offlineNote,
      final boolean offlineOneUseKeys,
      final boolean offlineRecursiveNoteProof,
      final boolean offlineFountainQr,
      final boolean offlineSyncOptional,
      final boolean offlineTelemetry) {
    this(
        offlineNote,
        offlineOneUseKeys,
        offlineRecursiveNoteProof,
        offlineFountainQr,
        offlineSyncOptional,
        offlineTelemetry,
        false,
        null,
        null,
        null,
        false);
  }

  public OfflineReadiness(
      final boolean offlineNote,
      final boolean offlineOneUseKeys,
      final boolean offlineRecursiveNoteProof,
      final boolean offlineFountainQr,
      final boolean offlineSyncOptional,
      final boolean offlineTelemetry,
      final boolean offlineKagemushaRecursiveCompactAvailable,
      final String offlineKagemushaRecursiveCompactMode,
      final Integer offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion,
      final String offlineKagemushaRecursiveCompactCircuitId,
      final boolean offlineKagemushaRecursiveCompactArtifactsAvailable) {
    this.offlineNote = offlineNote;
    this.offlineOneUseKeys = offlineOneUseKeys;
    this.offlineRecursiveNoteProof = offlineRecursiveNoteProof;
    this.offlineFountainQr = offlineFountainQr;
    this.offlineSyncOptional = offlineSyncOptional;
    this.offlineTelemetry = offlineTelemetry;
    this.offlineKagemushaRecursiveCompactAvailable = offlineKagemushaRecursiveCompactAvailable;
    this.offlineKagemushaRecursiveCompactMode = offlineKagemushaRecursiveCompactMode;
    this.offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion = offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion;
    this.offlineKagemushaRecursiveCompactCircuitId = offlineKagemushaRecursiveCompactCircuitId;
    this.offlineKagemushaRecursiveCompactArtifactsAvailable = offlineKagemushaRecursiveCompactArtifactsAvailable;
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

  public boolean offlineKagemushaRecursiveCompactAvailable() {
    return offlineKagemushaRecursiveCompactAvailable;
  }

  public String offlineKagemushaRecursiveCompactMode() {
    return offlineKagemushaRecursiveCompactMode;
  }

  public Integer offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion() {
    return offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion;
  }

  public String offlineKagemushaRecursiveCompactCircuitId() {
    return offlineKagemushaRecursiveCompactCircuitId;
  }

  public boolean offlineKagemushaRecursiveCompactArtifactsAvailable() {
    return offlineKagemushaRecursiveCompactArtifactsAvailable;
  }
}
