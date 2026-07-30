package org.hyperledger.iroha.android.sorafs;

/**
 * Staged anonymity roll-out policy for SoraNet fetches.
 *
 * <p>This mirrors {@code sorafs_orchestrator::AnonymityPolicy} and is used when serialising gateway
 * fetch requests or telemetry overrides.
 */
public enum AnonymityPolicy {
  /** Require at least one PQ-capable guard in the pinned relay set. */
  ANON_GUARD_PQ("anon-guard-pq"),
  /** Require PQ coverage on the majority of SoraNet hops. */
  ANON_MAJORITY_PQ("anon-majority-pq"),
  /** Enforce PQ-only SoraNet paths and reject direct transport substitution. */
  ANON_STRICT_PQ("anon-strict-pq");

  private final String label;

  AnonymityPolicy(final String label) {
    this.label = label;
  }

  public String label() {
    return label;
  }

  /**
   * Parse one exact canonical V1 policy label. Returns {@code null} for every alias or unknown
   * value.
   */
  public static AnonymityPolicy fromLabel(final String raw) {
    if (raw == null) {
      return null;
    }
    return switch (raw) {
      case "anon-guard-pq" -> ANON_GUARD_PQ;
      case "anon-majority-pq" -> ANON_MAJORITY_PQ;
      case "anon-strict-pq" -> ANON_STRICT_PQ;
      default -> null;
    };
  }
}
