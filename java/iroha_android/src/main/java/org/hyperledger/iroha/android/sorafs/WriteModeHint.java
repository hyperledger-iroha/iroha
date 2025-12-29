package org.hyperledger.iroha.android.sorafs;

import java.util.Locale;

/**
 * Mirrors {@code sorafs_orchestrator::WriteModeHint}.
 *
 * <p>Android callers can use this enum to request PQ-only upload paths when building gateway fetch
 * requests. The labels match the Norito JSON representation expected by the Rust orchestrator.
 */
public enum WriteModeHint {
  /** Default behaviour for read/replication workloads. */
  READ_ONLY("read_only"),
  /** Enforce PQ-only transport for upload workloads. */
  UPLOAD_PQ_ONLY("upload_pq_only");

  private final String label;

  WriteModeHint(final String label) {
    this.label = label;
  }

  /** Returns the canonical lowercase label emitted in JSON payloads. */
  public String label() {
    return label;
  }

  /**
   * Parse an incoming label. Returns {@code null} when the value does not map to a known hint.
   */
  public static WriteModeHint fromLabel(final String raw) {
    if (raw == null) {
      return null;
    }
    final String normalised = raw.trim().toLowerCase(Locale.ROOT).replace('-', '_');
    return switch (normalised) {
      case "read_only" -> READ_ONLY;
      case "upload_pq_only" -> UPLOAD_PQ_ONLY;
      default -> null;
    };
  }
}

