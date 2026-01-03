package org.hyperledger.iroha.android.sorafs;

import java.util.Locale;

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
  ANON_MAJORIY_PQ("anon-majority-pq"),
  /** Enforce PQ-only SoraNet paths; fall back to direct transports when unavailable. */
  ANON_STRICT_PQ("anon-strict-pq");

  private final String label;

  AnonymityPolicy(final String label) {
    this.label = label;
  }

  public String label() {
    return label;
  }

  /**
   * Parse a policy label, accepting the canonical hyphenated names plus stage aliases. Returns
   * {@code null} for unknown values.
   */
  public static AnonymityPolicy fromLabel(final String raw) {
    if (raw == null) {
      return null;
    }
    final String normalised = raw.trim().toLowerCase(Locale.ROOT);
    return switch (normalised) {
      case "anon_guard_pq", "anon-guard-pq", "stage_a", "stage-a", "stagea" -> ANON_GUARD_PQ;
      case "anon_majority_pq", "anon-majority-pq", "stage_b", "stage-b", "stageb" -> ANON_MAJORIY_PQ;
      case "anon_strict_pq", "anon-strict-pq", "stage_c", "stage-c", "stagec" -> ANON_STRICT_PQ;
      default -> null;
    };
  }
}
