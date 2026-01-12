package org.hyperledger.iroha.android.subscriptions;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonParser;

/** JSON parser for subscription Torii endpoints. */
public final class SubscriptionJsonParser {

  private SubscriptionJsonParser() {}

  public static SubscriptionPlanCreateResponse parsePlanCreateResponse(final byte[] payload) {
    final Map<String, Object> object = expectObject(parse(payload), "root");
    final boolean ok = asBoolean(object.get("ok"), "ok");
    final String planId = asString(object.get("plan_id"), "plan_id");
    final String txHashHex = asString(object.get("tx_hash_hex"), "tx_hash_hex");
    return new SubscriptionPlanCreateResponse(ok, planId, txHashHex);
  }

  public static SubscriptionPlanListResponse parsePlanList(final byte[] payload) {
    final Map<String, Object> object = expectObject(parse(payload), "root");
    final List<Object> items = asArrayOrEmpty(object.get("items"), "items");
    final long total = asLongOrDefault(object.get("total"), "total", items.size());
    final List<SubscriptionPlanListResponse.SubscriptionPlanListItem> parsed =
        new ArrayList<>(items.size());
    for (int i = 0; i < items.size(); i++) {
      final String path = "items[" + i + "]";
      final Map<String, Object> entry = expectObject(items.get(i), path);
      final String planId = asString(entry.get("plan_id"), path + ".plan_id");
      final Map<String, Object> plan = expectObject(entry.get("plan"), path + ".plan");
      parsed.add(new SubscriptionPlanListResponse.SubscriptionPlanListItem(planId, plan));
    }
    return new SubscriptionPlanListResponse(parsed, total);
  }

  public static SubscriptionCreateResponse parseSubscriptionCreateResponse(final byte[] payload) {
    final Map<String, Object> object = expectObject(parse(payload), "root");
    final boolean ok = asBoolean(object.get("ok"), "ok");
    final String subscriptionId = asString(object.get("subscription_id"), "subscription_id");
    final String billingTriggerId =
        asString(object.get("billing_trigger_id"), "billing_trigger_id");
    final String usageTriggerId =
        asOptionalString(object.get("usage_trigger_id"), "usage_trigger_id");
    final long firstChargeMs = asLong(object.get("first_charge_ms"), "first_charge_ms");
    final String txHashHex = asString(object.get("tx_hash_hex"), "tx_hash_hex");
    return new SubscriptionCreateResponse(
        ok, subscriptionId, billingTriggerId, usageTriggerId, firstChargeMs, txHashHex);
  }

  public static SubscriptionListResponse parseSubscriptionList(final byte[] payload) {
    final Map<String, Object> object = expectObject(parse(payload), "root");
    final List<Object> items = asArrayOrEmpty(object.get("items"), "items");
    final long total = asLongOrDefault(object.get("total"), "total", items.size());
    final List<SubscriptionListResponse.SubscriptionRecord> parsed =
        new ArrayList<>(items.size());
    for (int i = 0; i < items.size(); i++) {
      parsed.add(parseSubscriptionRecord(items.get(i), "items[" + i + "]"));
    }
    return new SubscriptionListResponse(parsed, total);
  }

  public static SubscriptionListResponse.SubscriptionRecord parseSubscriptionRecord(
      final byte[] payload) {
    return parseSubscriptionRecord(parse(payload), "root");
  }

  public static SubscriptionActionResponse parseActionResponse(final byte[] payload) {
    final Map<String, Object> object = expectObject(parse(payload), "root");
    final boolean ok = asBoolean(object.get("ok"), "ok");
    final String subscriptionId = asString(object.get("subscription_id"), "subscription_id");
    final String txHashHex = asString(object.get("tx_hash_hex"), "tx_hash_hex");
    return new SubscriptionActionResponse(ok, subscriptionId, txHashHex);
  }

  private static Object parse(final byte[] payload) {
    final String json = new String(payload, StandardCharsets.UTF_8).trim();
    if (json.isEmpty()) {
      throw new IllegalStateException("Empty JSON payload");
    }
    return JsonParser.parse(json);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> expectObject(final Object value, final String path) {
    if (!(value instanceof Map)) {
      throw new IllegalStateException(path + " is not a JSON object");
    }
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> asArray(final Object value, final String path) {
    if (!(value instanceof List)) {
      throw new IllegalStateException(path + " is not a JSON array");
    }
    return (List<Object>) value;
  }

  private static List<Object> asArrayOrEmpty(final Object value, final String path) {
    if (value == null) {
      return List.of();
    }
    return asArray(value, path);
  }

  private static String asString(final Object value, final String path) {
    if (value == null) {
      throw new IllegalStateException(path + " is missing");
    }
    if (value instanceof String string) {
      return string;
    }
    return String.valueOf(value);
  }

  private static String asOptionalString(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    if (value instanceof String string) {
      return string;
    }
    throw new IllegalStateException(path + " must be a string when present");
  }

  private static boolean asBoolean(final Object value, final String path) {
    if (!(value instanceof Boolean bool)) {
      throw new IllegalStateException(path + " is missing");
    }
    return bool.booleanValue();
  }

  private static long asLong(final Object value, final String path) {
    if (!(value instanceof Number number)) {
      throw new IllegalStateException(path + " is not a number");
    }
    if (number instanceof Double || number instanceof Float) {
      final double numeric = number.doubleValue();
      if (numeric % 1 != 0) {
        throw new IllegalStateException(path + " must be an integer");
      }
      return (long) numeric;
    }
    return number.longValue();
  }

  private static long asLongOrDefault(final Object value, final String path, final long fallback) {
    if (value == null) {
      return fallback;
    }
    return asLong(value, path);
  }

  private static SubscriptionListResponse.SubscriptionRecord parseSubscriptionRecord(
      final Object value, final String path) {
    final Map<String, Object> entry = expectObject(value, path);
    final String subscriptionId = asString(entry.get("subscription_id"), path + ".subscription_id");
    final Map<String, Object> subscription =
        expectObject(entry.get("subscription"), path + ".subscription");
    final Map<String, Object> invoice =
        asOptionalObject(entry.get("invoice"), path + ".invoice");
    final Map<String, Object> plan = asOptionalObject(entry.get("plan"), path + ".plan");
    return new SubscriptionListResponse.SubscriptionRecord(subscriptionId, subscription, invoice, plan);
  }

  private static Map<String, Object> asOptionalObject(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    return expectObject(value, path);
  }
}
