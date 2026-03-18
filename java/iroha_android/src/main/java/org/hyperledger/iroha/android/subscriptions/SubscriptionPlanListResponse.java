package org.hyperledger.iroha.android.subscriptions;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonEncoder;

/** Paginated list of subscription plans. */
public final class SubscriptionPlanListResponse {

  private final List<SubscriptionPlanListItem> items;
  private final long total;

  public SubscriptionPlanListResponse(final List<SubscriptionPlanListItem> items, final long total) {
    this.items = Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(items, "items")));
    this.total = total;
  }

  public List<SubscriptionPlanListItem> items() {
    return items;
  }

  public long total() {
    return total;
  }

  /** Subscription plan list item. */
  public static final class SubscriptionPlanListItem {
    private final String planId;
    private final Map<String, Object> plan;

    public SubscriptionPlanListItem(final String planId, final Map<String, Object> plan) {
      this.planId = Objects.requireNonNull(planId, "planId");
      this.plan = freezeMap(Objects.requireNonNull(plan, "plan"));
    }

    public String planId() {
      return planId;
    }

    public Map<String, Object> plan() {
      return plan;
    }

    public String planJson() {
      return JsonEncoder.encode(plan);
    }

    private static Map<String, Object> freezeMap(final Map<String, Object> input) {
      return Collections.unmodifiableMap(new java.util.LinkedHashMap<>(input));
    }
  }
}
