package org.hyperledger.iroha.android.offline;

/** Feature flags reported by Torii's Offline V2 readiness endpoint. */
public final class OfflineV2Readiness {
  private final boolean offlineTelemetry;
  private final boolean offlineKagemushaRecursiveCompactAvailable;
  private final String offlineKagemushaRecursiveCompactMode;
  private final Integer offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion;
  private final String offlineKagemushaRecursiveCompactCircuitId;
  private final boolean offlineKagemushaRecursiveCompactArtifactsAvailable;

  public OfflineV2Readiness(
      final boolean offlineTelemetry,
      final boolean offlineKagemushaRecursiveCompactAvailable,
      final String offlineKagemushaRecursiveCompactMode,
      final Integer offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion,
      final String offlineKagemushaRecursiveCompactCircuitId,
      final boolean offlineKagemushaRecursiveCompactArtifactsAvailable) {
    this.offlineTelemetry = offlineTelemetry;
    this.offlineKagemushaRecursiveCompactAvailable = offlineKagemushaRecursiveCompactAvailable;
    this.offlineKagemushaRecursiveCompactMode = offlineKagemushaRecursiveCompactMode;
    this.offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion = offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion;
    this.offlineKagemushaRecursiveCompactCircuitId = offlineKagemushaRecursiveCompactCircuitId;
    this.offlineKagemushaRecursiveCompactArtifactsAvailable = offlineKagemushaRecursiveCompactArtifactsAvailable;
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
