package org.hyperledger.iroha.android.telemetry;

import java.util.Locale;
import java.util.Map;
import java.util.Objects;

/**
 * Sanitised snapshot of the device network state used by {@code android.telemetry.network_context}.
 *
 * <p>The snapshot intentionally captures a coarse {@code network_type} label (e.g. {@code wifi},
 * {@code cellular}) and a roaming flag only. Carrier identifiers are excluded by design so the
 * signal complies with the AND7 redaction policy.
 */
public final class NetworkContext {

  private final String networkType;
  private final boolean roaming;

  private NetworkContext(final Builder builder) {
    this.networkType = builder.networkType;
    this.roaming = builder.roaming;
  }

  private NetworkContext(final String networkType, final boolean roaming) {
    this.networkType = Objects.requireNonNull(networkType, "networkType");
    this.roaming = roaming;
  }

  /** Convenience helper that creates a snapshot for {@code network_type} and {@code roaming}. */
  public static NetworkContext of(final String networkType, final boolean roaming) {
    return new NetworkContext(normaliseNetworkType(networkType), roaming);
  }

  public static Builder builder() {
    return new Builder();
  }

  /** Returns the canonicalised network type (always lowercase, never {@code null}). */
  public String networkType() {
    return networkType;
  }

  /** Returns {@code true} when the device is currently roaming. */
  public boolean roaming() {
    return roaming;
  }

  /** Serialises this snapshot to the map expected by {@link TelemetrySink#emitSignal}. */
  public Map<String, Object> toTelemetryFields() {
    return Map.of(
        "network_type", networkType,
        "roaming", roaming);
  }

  public Builder toBuilder() {
    return new Builder().setNetworkType(networkType).setRoaming(roaming);
  }

  private static String normaliseNetworkType(final String value) {
    final String trimmed = Objects.requireNonNull(value, "networkType").trim();
    if (trimmed.isEmpty()) {
      throw new IllegalStateException("networkType must be non-empty");
    }
    return trimmed.toLowerCase(Locale.ROOT);
  }

  /** Builder used by apps/tests that want to construct snapshots without helper factories. */
  public static final class Builder {
    private String networkType = "unknown";
    private boolean roaming;

    public Builder setNetworkType(final String networkType) {
      this.networkType = normaliseNetworkType(networkType);
      return this;
    }

    public Builder setRoaming(final boolean roaming) {
      this.roaming = roaming;
      return this;
    }

    public NetworkContext build() {
      return new NetworkContext(this);
    }
  }
}
