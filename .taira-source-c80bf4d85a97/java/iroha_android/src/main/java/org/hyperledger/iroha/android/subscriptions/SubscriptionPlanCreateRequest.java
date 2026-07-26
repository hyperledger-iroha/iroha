package org.hyperledger.iroha.android.subscriptions;

import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;

/** Request payload for `POST /v1/subscriptions/plans`. */
public final class SubscriptionPlanCreateRequest {

  private final String authority;
  private final String planId;
  private final Map<String, Object> plan;

  private SubscriptionPlanCreateRequest(final Builder builder) {
    this.authority = requireNonBlank(builder.authority, "authority");
    this.planId = requireNonBlank(builder.planId, "plan_id");
    this.plan = requirePlan(builder.plan, "plan");
  }

  public String authority() {
    return authority;
  }

  public String planId() {
    return planId;
  }

  public Map<String, Object> plan() {
    return plan;
  }

  public Map<String, Object> toJsonMap() {
    final Map<String, Object> json = new LinkedHashMap<>();
    json.put("authority", authority);
    json.put("plan_id", planId);
    json.put("plan", plan);
    return Collections.unmodifiableMap(json);
  }

  public byte[] toJsonBytes() {
    return JsonEncoder.encode(toJsonMap()).getBytes(StandardCharsets.UTF_8);
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private String authority;
    private String planId;
    private Map<String, Object> plan;

    private Builder() {}

    public Builder authority(final String authority) {
      this.authority = authority;
      return this;
    }

    public Builder planId(final String planId) {
      this.planId = planId;
      return this;
    }

    public Builder plan(final Map<String, Object> plan) {
      this.plan = plan;
      return this;
    }

    /** Parse and set the plan from a JSON string. */
    public Builder planJson(final String planJson) {
      this.plan = parsePlanJson(planJson);
      return this;
    }

    public SubscriptionPlanCreateRequest build() {
      return new SubscriptionPlanCreateRequest(this);
    }

    @SuppressWarnings("unchecked")
    private static Map<String, Object> parsePlanJson(final String planJson) {
      final String trimmed = planJson == null ? "" : planJson.trim();
      if (trimmed.isEmpty()) {
        throw new IllegalArgumentException("plan_json must not be blank");
      }
      final Object parsed = JsonParser.parse(trimmed);
      if (!(parsed instanceof Map)) {
        throw new IllegalArgumentException("plan_json must encode a JSON object");
      }
      return (Map<String, Object>) parsed;
    }
  }

  private static String requireNonBlank(final String value, final String field) {
    if (value == null || value.trim().isEmpty()) {
      throw new IllegalStateException(field + " is required");
    }
    return value.trim();
  }

  private static Map<String, Object> requirePlan(
      final Map<String, Object> plan, final String field) {
    Objects.requireNonNull(plan, field + " is required");
    return Collections.unmodifiableMap(new LinkedHashMap<>(plan));
  }
}
