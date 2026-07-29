package org.hyperledger.iroha.android.sorafs;

/**
 * Mirrors {@code sorafs_orchestrator::WriteModeHint}.
 *
 * <p>Android callers can use this enum to request PQ-only upload paths when building gateway fetch
 * requests. The labels match the Norito JSON representation expected by the Rust orchestrator.
 */
public enum WriteModeHint {
  /** Default behaviour for read/replication workloads. */
  READ_ONLY("read-only"),
  /** Enforce PQ-only transport for upload workloads. */
  UPLOAD_PQ_ONLY("upload-pq-only");

  private final String label;

  WriteModeHint(final String label) {
    this.label = label;
  }

  /** Returns the canonical lowercase label emitted in JSON payloads. */
  public String label() {
    return label;
  }

  /**
   * Parse one exact canonical V1 label. Returns {@code null} for every alias or unknown value.
   */
  public static WriteModeHint fromLabel(final String raw) {
    if (raw == null) {
      return null;
    }
    return switch (raw) {
      case "read-only" -> READ_ONLY;
      case "upload-pq-only" -> UPLOAD_PQ_ONLY;
      default -> null;
    };
  }
}
