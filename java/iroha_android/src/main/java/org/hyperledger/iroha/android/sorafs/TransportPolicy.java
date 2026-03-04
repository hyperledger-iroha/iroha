package org.hyperledger.iroha.android.sorafs;

import java.util.Locale;

/**
 * Transport fallback ordering used by the SoraFS orchestrator.
 *
 * <p>The enum mirrors the Rust {@code sorafs_orchestrator::TransportPolicy} so Android callers can
 * deterministically map between labels and policy variants when building fetch requests.
 */
public enum TransportPolicy {
  /** Prefer SoraNet relays, then QUIC, then Torii/HTTP, finally any vendor transport. */
  SORANET_FIRST("soranet-first"),
  /** Require SoraNet relays and fail rather than falling back to direct transports. */
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
     * Parse a policy label, accepting dash or underscore separated forms. Returns {@code null} when
     * the input does not match a known policy.
     */
  public static TransportPolicy fromLabel(final String raw) {
    if (raw == null) {
      return null;
    }
    final String normalised =
        raw.trim().toLowerCase(Locale.ROOT).replace('-', '_');
    return switch (normalised) {
      case "soranet_first" -> SORANET_FIRST;
      case "soranet_strict", "soranet_only" -> SORANET_STRICT;
      case "direct_only" -> DIRECT_ONLY;
      default -> null;
    };
  }
}
