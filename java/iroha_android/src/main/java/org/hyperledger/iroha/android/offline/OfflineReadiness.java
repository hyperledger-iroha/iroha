package org.hyperledger.iroha.android.offline;

/** Feature flags reported by Torii's Offline readiness endpoint. */
public final class OfflineReadiness {
  private final boolean offlineNote;
  private final boolean offlineOneUseKeys;
  private final boolean offlineRecursiveNoteProof;
  private final boolean offlineFountainQr;
  private final boolean offlineSyncOptional;
  private final boolean offlineTelemetry;
  private final boolean offlineKagemushaAbi7;
  private final String offlineKagemushaAbi7Mode;
  private final Integer offlineKagemushaAbi7BridgeAbiVersion;
  private final String offlineKagemushaAbi7CircuitId;
  private final boolean offlineKagemushaAbi7Artifacts;

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
      final boolean offlineKagemushaAbi7,
      final String offlineKagemushaAbi7Mode,
      final Integer offlineKagemushaAbi7BridgeAbiVersion,
      final String offlineKagemushaAbi7CircuitId,
      final boolean offlineKagemushaAbi7Artifacts) {
    this.offlineNote = offlineNote;
    this.offlineOneUseKeys = offlineOneUseKeys;
    this.offlineRecursiveNoteProof = offlineRecursiveNoteProof;
    this.offlineFountainQr = offlineFountainQr;
    this.offlineSyncOptional = offlineSyncOptional;
    this.offlineTelemetry = offlineTelemetry;
    this.offlineKagemushaAbi7 = offlineKagemushaAbi7;
    this.offlineKagemushaAbi7Mode = offlineKagemushaAbi7Mode;
    this.offlineKagemushaAbi7BridgeAbiVersion = offlineKagemushaAbi7BridgeAbiVersion;
    this.offlineKagemushaAbi7CircuitId = offlineKagemushaAbi7CircuitId;
    this.offlineKagemushaAbi7Artifacts = offlineKagemushaAbi7Artifacts;
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

  public boolean offlineKagemushaAbi7() {
    return offlineKagemushaAbi7;
  }

  public String offlineKagemushaAbi7Mode() {
    return offlineKagemushaAbi7Mode;
  }

  public Integer offlineKagemushaAbi7BridgeAbiVersion() {
    return offlineKagemushaAbi7BridgeAbiVersion;
  }

  public String offlineKagemushaAbi7CircuitId() {
    return offlineKagemushaAbi7CircuitId;
  }

  public boolean offlineKagemushaAbi7Artifacts() {
    return offlineKagemushaAbi7Artifacts;
  }
}
