package org.hyperledger.iroha.android.client;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;

/** Immutable response wrapper for `GET /v1/ram-lfe/program-policies`. */
public final class RamLfeProgramPolicyListResponse {
  private final long total;
  private final List<RamLfeProgramPolicySummary> items;

  public RamLfeProgramPolicyListResponse(
      final long total, final List<RamLfeProgramPolicySummary> items) {
    this.total = total;
    this.items =
        Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(items, "items")));
  }

  public long total() {
    return total;
  }

  public List<RamLfeProgramPolicySummary> items() {
    return items;
  }
}
