package org.hyperledger.iroha.android.client;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.FeePaymentIntent;

/** Successful deterministic fee quote preserving the exact payer and gas bound. */
public final class FeeQuoteResponse {
  private final FeePaymentIntent intent;
  private final Map<String, Object> observation;
  private final List<Map<String, Object>> components;
  private final List<Map<String, Object>> capacities;
  private final Map<String, Object> decision;

  FeeQuoteResponse(
      final FeePaymentIntent intent,
      final Map<String, Object> observation,
      final List<Map<String, Object>> components,
      final List<Map<String, Object>> capacities,
      final Map<String, Object> decision) {
    this.intent = Objects.requireNonNull(intent, "intent");
    this.observation = snapshot(observation);
    this.components = snapshotList(components);
    this.capacities = snapshotList(capacities);
    this.decision = snapshot(decision);
  }

  public FeePaymentIntent intent() { return intent; }
  public Map<String, Object> observation() { return observation; }
  public List<Map<String, Object>> components() { return components; }
  public List<Map<String, Object>> capacities() { return capacities; }
  public Map<String, Object> decision() { return decision; }

  private static Map<String, Object> snapshot(final Map<String, Object> value) {
    return Collections.unmodifiableMap(new LinkedHashMap<>(value));
  }

  private static List<Map<String, Object>> snapshotList(final List<Map<String, Object>> values) {
    final List<Map<String, Object>> out = new ArrayList<>();
    for (final Map<String, Object> value : values) out.add(snapshot(value));
    return Collections.unmodifiableList(out);
  }
}
