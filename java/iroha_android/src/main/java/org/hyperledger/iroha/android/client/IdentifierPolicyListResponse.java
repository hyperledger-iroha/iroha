package org.hyperledger.iroha.android.client;

import java.util.List;
import java.util.Objects;

/** Immutable response wrapper for `GET /v1/identifier-policies`. */
public final class IdentifierPolicyListResponse {
  private final long total;
  private final List<IdentifierPolicySummary> items;

  public IdentifierPolicyListResponse(
      final long total, final List<IdentifierPolicySummary> items) {
    this.total = total;
    this.items = List.copyOf(Objects.requireNonNull(items, "items"));
  }

  public long total() {
    return total;
  }

  public List<IdentifierPolicySummary> items() {
    return items;
  }
}
