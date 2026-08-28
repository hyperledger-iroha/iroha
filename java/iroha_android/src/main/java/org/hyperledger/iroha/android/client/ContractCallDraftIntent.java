package org.hyperledger.iroha.android.client;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.ContractInvocation;
import org.hyperledger.iroha.android.model.JsonValue;

/**
 * Caller-trusted, local-only intent used to validate an unsigned contract-call draft.
 *
 * <p>The invocation must come from a trusted contract artifact and argument schema. Metadata is
 * the exact final transaction metadata the caller is prepared to sign. Neither value is sent to
 * Torii as proof of its own response.
 */
public final class ContractCallDraftIntent {
  private final ContractInvocation invocation;
  private final Map<String, JsonValue> metadata;

  public ContractCallDraftIntent(
      final ContractInvocation invocation, final Map<String, JsonValue> metadata) {
    this.invocation = Objects.requireNonNull(invocation, "invocation");
    final Map<String, JsonValue> snapshot = new LinkedHashMap<>();
    Objects.requireNonNull(metadata, "metadata")
        .forEach(
            (key, value) ->
                snapshot.put(
                    Objects.requireNonNull(key, "metadata key"),
                    Objects.requireNonNull(value, "metadata value")));
    this.metadata = Collections.unmodifiableMap(snapshot);
  }

  /** Returns the exact resolved invocation authorized by the caller. */
  public ContractInvocation invocation() {
    return invocation;
  }

  /** Returns the exact final transaction metadata authorized by the caller. */
  public Map<String, JsonValue> metadata() {
    return metadata;
  }
}
