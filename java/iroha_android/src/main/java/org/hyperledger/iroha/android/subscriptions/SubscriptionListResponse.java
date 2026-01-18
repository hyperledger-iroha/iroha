package org.hyperledger.iroha.android.subscriptions;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonEncoder;

/** Paginated list of subscriptions. */
public final class SubscriptionListResponse {

  private final List<SubscriptionRecord> items;
  private final long total;

  public SubscriptionListResponse(final List<SubscriptionRecord> items, final long total) {
    this.items = Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(items, "items")));
    this.total = total;
  }

  public List<SubscriptionRecord> items() {
    return items;
  }

  public long total() {
    return total;
  }

  /** Subscription list entry (also returned by GET /v1/subscriptions/{id}). */
  public static final class SubscriptionRecord {
    private final String subscriptionId;
    private final Map<String, Object> subscription;
    private final Map<String, Object> invoice;
    private final Map<String, Object> plan;

    public SubscriptionRecord(
        final String subscriptionId,
        final Map<String, Object> subscription,
        final Map<String, Object> invoice,
        final Map<String, Object> plan) {
      this.subscriptionId = Objects.requireNonNull(subscriptionId, "subscriptionId");
      this.subscription = freezeMap(Objects.requireNonNull(subscription, "subscription"));
      this.invoice = invoice == null ? null : freezeMap(invoice);
      this.plan = plan == null ? null : freezeMap(plan);
    }

    public String subscriptionId() {
      return subscriptionId;
    }

    public Map<String, Object> subscription() {
      return subscription;
    }

    public Map<String, Object> invoice() {
      return invoice;
    }

    public Map<String, Object> plan() {
      return plan;
    }

    public String subscriptionJson() {
      return JsonEncoder.encode(subscription);
    }

    public String invoiceJson() {
      return invoice == null ? null : JsonEncoder.encode(invoice);
    }

    public String planJson() {
      return plan == null ? null : JsonEncoder.encode(plan);
    }

    private static Map<String, Object> freezeMap(final Map<String, Object> input) {
      return Collections.unmodifiableMap(new java.util.LinkedHashMap<>(input));
    }
  }
}
