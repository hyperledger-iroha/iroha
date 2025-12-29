package org.hyperledger.iroha.android.offline;

import java.util.Objects;

/** Canonical proof payload returned by Torii (`/v1/offline/transfers/proof`). */
public final class OfflineProofRequestResult {

  private final OfflineProofRequestKind kind;
  private final String json;

  public OfflineProofRequestResult(
      final OfflineProofRequestKind kind, final String jsonPayload) {
    this.kind = Objects.requireNonNull(kind, "kind");
    this.json = Objects.requireNonNull(jsonPayload, "jsonPayload");
  }

  public OfflineProofRequestKind kind() {
    return kind;
  }

  /**
   * Canonical JSON representation of the proof payload; this string can be passed directly to the
   * FASTPQ prover.
   */
  public String json() {
    return json;
  }
}
