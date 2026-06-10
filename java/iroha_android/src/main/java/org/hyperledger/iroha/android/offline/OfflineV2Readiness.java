package org.hyperledger.iroha.android.offline;

/** Feature flags reported by Torii's Offline V2 readiness endpoint. */
public final class OfflineV2Readiness {
  private final boolean offlineTelemetry;
  private final boolean offlineKagemushaAbi7;
  private final String offlineKagemushaAbi7Mode;
  private final Integer offlineKagemushaAbi7BridgeAbiVersion;
  private final String offlineKagemushaAbi7CircuitId;
  private final boolean offlineKagemushaAbi7Artifacts;

  public OfflineV2Readiness(
      final boolean offlineTelemetry,
      final boolean offlineKagemushaAbi7,
      final String offlineKagemushaAbi7Mode,
      final Integer offlineKagemushaAbi7BridgeAbiVersion,
      final String offlineKagemushaAbi7CircuitId,
      final boolean offlineKagemushaAbi7Artifacts) {
    this.offlineTelemetry = offlineTelemetry;
    this.offlineKagemushaAbi7 = offlineKagemushaAbi7;
    this.offlineKagemushaAbi7Mode = offlineKagemushaAbi7Mode;
    this.offlineKagemushaAbi7BridgeAbiVersion = offlineKagemushaAbi7BridgeAbiVersion;
    this.offlineKagemushaAbi7CircuitId = offlineKagemushaAbi7CircuitId;
    this.offlineKagemushaAbi7Artifacts = offlineKagemushaAbi7Artifacts;
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
