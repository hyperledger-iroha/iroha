package org.hyperledger.iroha.android.sorafs;

/**
 * Transport selection ordering used by the SoraFS orchestrator.
 *
 * <p>The enum mirrors the Rust {@code sorafs_orchestrator::TransportPolicy} so Android callers can
 * deterministically map between labels and policy variants when building fetch requests.
 */
public enum TransportPolicy {
  /** Prefer SoraNet relays, then QUIC, then Torii/HTTP, finally any vendor transport. */
  SORANET_FIRST("soranet-first"),
  /** Require SoraNet relays and fail rather than selecting direct transports. */
  SORANET_STRICT("soranet-strict"),
  /** Restrict selection to direct transports (Torii/QUIC). */
  DIRECT_ONLY("direct-only");

  private final String label;

  TransportPolicy(final String label) {
    this.label = label;
  }

  /**
     * Canonical lowercase label used by the CLI/SDKs.
     *
     * @return label string (e.g. {@code soranet-first}).
     */
  public String label() {
    return label;
  }

  /**
   * Parse one exact canonical V1 policy label. Returns {@code null} when the input does not match a
   * known policy byte-for-byte.
   */
  public static TransportPolicy fromLabel(final String raw) {
    if (raw == null) {
      return null;
    }
    return switch (raw) {
      case "soranet-first" -> SORANET_FIRST;
      case "soranet-strict" -> SORANET_STRICT;
      case "direct-only" -> DIRECT_ONLY;
      default -> null;
    };
  }
}
