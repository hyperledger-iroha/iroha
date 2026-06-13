package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;

public final class OfflineJsonParser {

  private OfflineJsonParser() {}

  public static OfflineReadiness parseOfflineReadiness(final byte[] payload) {
    final Object root = parse(payload);
    final Map<String, Object> object = expectObject(root, "root");
    return new OfflineReadiness(
        asOptionalBoolean(object.get("offline_note"), false),
        asOptionalBoolean(object.get("offline_one_use_keys"), false),
        asOptionalBoolean(object.get("offline_recursive_note_proof"), false),
        asOptionalBoolean(object.get("offline_fountain_qr"), false),
        asOptionalBoolean(object.get("offline_sync_optional"), false),
        asBoolean(object.get("offline_telemetry"), "offline_telemetry"),
        kagemushaRecursiveCompactAvailable(object),
        matchingNullableStringAlias(
            object,
            "offline_kagemusha_abi7_mode",
            "offline_kagemusha_recursive_compact_mode"),
        matchingOptionalIntegerAlias(
            object,
            "offline_kagemusha_abi7_bridge_abi_version",
            "offline_kagemusha_recursive_compact_required_native_bridge_abi_version"),
        kagemushaRecursiveCompactCircuitId(object),
        kagemushaRecursiveCompactArtifactsAvailable(object));
  }

  public static OfflineV2Readiness parseOfflineV2Readiness(final byte[] payload) {
    final Object root = parse(payload);
    final Map<String, Object> object = expectObject(root, "root");
    return new OfflineV2Readiness(
        asBoolean(object.get("offline_telemetry"), "offline_telemetry"),
        kagemushaRecursiveCompactAvailable(object),
        matchingNullableStringAlias(
            object,
            "offline_kagemusha_abi7_mode",
            "offline_kagemusha_recursive_compact_mode"),
        matchingOptionalIntegerAlias(
            object,
            "offline_kagemusha_abi7_bridge_abi_version",
            "offline_kagemusha_recursive_compact_required_native_bridge_abi_version"),
        kagemushaRecursiveCompactCircuitId(object),
        kagemushaRecursiveCompactArtifactsAvailable(object));
  }

  private static boolean kagemushaRecursiveCompactAvailable(final Map<String, Object> object) {
    return matchingOptionalBooleanAlias(
        object,
        "offline_kagemusha_abi7",
        "offline_kagemusha_recursive_compact_available",
        false);
  }

  private static String kagemushaRecursiveCompactCircuitId(final Map<String, Object> object) {
    return matchingNullableStringAlias(
        object,
        "offline_kagemusha_abi7_circuit_id",
        "offline_kagemusha_recursive_compact_circuit_id");
  }

  private static boolean kagemushaRecursiveCompactArtifactsAvailable(final Map<String, Object> object) {
    return matchingOptionalBooleanAlias(
        object,
        "offline_kagemusha_abi7_artifacts",
        "offline_kagemusha_recursive_compact_artifacts_available",
        false);
  }

  public static String canonicalJson(final byte[] payload) {
    return JsonEncoder.encode(parse(payload));
  }

  public static OfflineTransferList parseTransfers(final byte[] payload) {
    final Object root = parse(payload);
    final Map<String, Object> object = expectObject(root, "root");
    final List<Object> rawItems = asList(object.get("items"), "items");
    final List<OfflineTransferList.OfflineTransferItem> items = new ArrayList<>(rawItems.size());
    for (int i = 0; i < rawItems.size(); i++) {
      items.add(parseTransferItem(expectObject(rawItems.get(i), "items[" + i + "]")));
    }
    final long total = asOptionalLong(object.get("total"), rawItems.size());
    return new OfflineTransferList(items, total);
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
    if (!(value instanceof Map<?, ?> map)) {
      throw new IllegalStateException(path + " is not a JSON object");
    }
    return (Map<String, Object>) map;
  }

  private static boolean asBoolean(final Object value, final String path) {
    if (!(value instanceof Boolean bool)) {
      throw new IllegalStateException(path + " must be a boolean");
    }
    return bool.booleanValue();
  }

  private static boolean asOptionalBoolean(final Object value, final boolean defaultValue) {
    return value instanceof Boolean bool ? bool.booleanValue() : defaultValue;
  }

  private static Integer asOptionalInteger(final Object value) {
    if (value == null) {
      return null;
    }
    if (value instanceof Number number) {
      return Integer.valueOf(number.intValue());
    }
    if (value instanceof String string) {
      final String trimmed = string.trim();
      return trimmed.isEmpty() ? null : Integer.valueOf(Integer.parseInt(trimmed));
    }
    return null;
  }

  private static String asNullableString(final Object value) {
    if (value == null) {
      return null;
    }
    if (value instanceof String string) {
      return string;
    }
    if (value instanceof Number || value instanceof Boolean) {
      return value.toString();
    }
    return null;
  }

  private static String matchingNullableStringAlias(
      final Map<String, Object> object, final String legacyKey, final String compactKey) {
    final boolean hasLegacy = object.containsKey(legacyKey);
    final boolean hasCompact = object.containsKey(compactKey);
    final String legacy = hasLegacy ? asNullableString(object.get(legacyKey)) : null;
    final String compact = hasCompact ? asNullableString(object.get(compactKey)) : null;
    if (hasLegacy && hasCompact && !valuesEqual(legacy, compact)) {
      throw new IllegalStateException(legacyKey + " and " + compactKey + " must match");
    }
    return legacy != null ? legacy : compact;
  }

  private static Integer matchingOptionalIntegerAlias(
      final Map<String, Object> object, final String legacyKey, final String compactKey) {
    final boolean hasLegacy = object.containsKey(legacyKey);
    final boolean hasCompact = object.containsKey(compactKey);
    final Integer legacy = hasLegacy ? asOptionalInteger(object.get(legacyKey)) : null;
    final Integer compact = hasCompact ? asOptionalInteger(object.get(compactKey)) : null;
    if (hasLegacy && hasCompact && !valuesEqual(legacy, compact)) {
      throw new IllegalStateException(legacyKey + " and " + compactKey + " must match");
    }
    return legacy != null ? legacy : compact;
  }

  private static boolean matchingOptionalBooleanAlias(
      final Map<String, Object> object,
      final String legacyKey,
      final String compactKey,
      final boolean defaultValue) {
    final boolean hasLegacy = object.containsKey(legacyKey);
    final boolean hasCompact = object.containsKey(compactKey);
    final Boolean legacy = hasLegacy ? Boolean.valueOf(asBoolean(object.get(legacyKey), legacyKey)) : null;
    final Boolean compact = hasCompact ? Boolean.valueOf(asBoolean(object.get(compactKey), compactKey)) : null;
    if (hasLegacy && hasCompact && !valuesEqual(legacy, compact)) {
      throw new IllegalStateException(legacyKey + " and " + compactKey + " must match");
    }
    if (legacy != null) {
      return legacy.booleanValue();
    }
    return compact != null ? compact.booleanValue() : defaultValue;
  }

  private static boolean valuesEqual(final Object left, final Object right) {
    return left == null ? right == null : left.equals(right);
  }

  private static OfflineTransferList.OfflineTransferItem parseTransferItem(
      final Map<String, Object> object) {
    final List<OfflineTransferList.ReceiptSummary> summaries = parseReceiptSummaries(object);
    return new OfflineTransferList.OfflineTransferItem(
        asOptionalString(pick(object, "bundle_id_hex", "bundleIdHex", "bundle_id", "bundleId")),
        asOptionalString(pick(object, "controller_id", "controllerId")),
        asOptionalString(pick(object, "controller_display", "controllerDisplay")),
        asOptionalString(pick(object, "receiver_id", "receiverId")),
        asOptionalString(pick(object, "receiver_display", "receiverDisplay")),
        asOptionalString(pick(object, "deposit_account_id", "depositAccountId")),
        asOptionalString(pick(object, "deposit_account_display", "depositAccountDisplay")),
        asOptionalString(pick(object, "asset_id", "assetId")),
        asOptionalString(pick(object, "total_amount", "totalAmount")),
        asOptionalString(pick(object, "claimed_delta", "claimedDelta")),
        asOptionalString(pick(object, "status")),
        asOptionalLong(pick(object, "receipt_count", "receiptCount"), summaries.size()),
        asOptionalLong(pick(object, "recorded_at_ms", "recordedAtMs"), 0L),
        asOptionalLong(pick(object, "recorded_at_height", "recordedAtHeight"), 0L),
        asOptionalObject(pick(object, "transfer")),
        summaries);
  }

  private static List<OfflineTransferList.ReceiptSummary> parseReceiptSummaries(
      final Map<String, Object> object) {
    final Object explicit = pick(object, "receipt_summaries", "receiptSummaries", "receipts");
    final Object transfer = pick(object, "transfer");
    final Object transferReceipts =
        transfer instanceof Map<?, ?> map ? ((Map<?, ?>) map).get("receipts") : null;
    final Object source = explicit != null ? explicit : transferReceipts;
    if (!(source instanceof List<?> list)) {
      return Collections.emptyList();
    }
    final List<OfflineTransferList.ReceiptSummary> summaries = new ArrayList<>(list.size());
    for (int i = 0; i < list.size(); i++) {
      if (list.get(i) instanceof Map<?, ?> map) {
        @SuppressWarnings("unchecked")
        final Map<String, Object> receipt = (Map<String, Object>) map;
        summaries.add(parseReceiptSummary(receipt));
      }
    }
    return summaries;
  }

  private static OfflineTransferList.ReceiptSummary parseReceiptSummary(
      final Map<String, Object> object) {
    return new OfflineTransferList.ReceiptSummary(
        asOptionalString(pick(object, "sender_id", "senderId", "from", "from_account_id")),
        asOptionalString(pick(object, "receiver_id", "receiverId", "to", "to_account_id")),
        asOptionalString(pick(object, "amount")),
        asOptionalString(pick(object, "asset_id", "assetId")),
        asOptionalString(pick(object, "status")));
  }

  private static Object pick(final Map<String, Object> object, final String... keys) {
    for (final String key : keys) {
      if (object.containsKey(key)) {
        return object.get(key);
      }
    }
    return null;
  }

  private static String asOptionalString(final Object value) {
    if (value == null) {
      return "";
    }
    if (value instanceof String string) {
      return string;
    }
    if (value instanceof Number || value instanceof Boolean) {
      return value.toString();
    }
    return "";
  }

  private static long asOptionalLong(final Object value, final long defaultValue) {
    if (value == null) {
      return defaultValue;
    }
    if (value instanceof Number number) {
      return number.longValue();
    }
    if (value instanceof String string) {
      final String trimmed = string.trim();
      if (trimmed.isEmpty()) {
        return defaultValue;
      }
      return Long.parseLong(trimmed);
    }
    return defaultValue;
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> asOptionalObject(final Object value) {
    if (value instanceof Map<?, ?> map) {
      return new LinkedHashMap<>((Map<String, Object>) map);
    }
    return Collections.emptyMap();
  }

  @SuppressWarnings("unchecked")
  private static List<Object> asList(final Object value, final String path) {
    if (!(value instanceof List<?> list)) {
      throw new IllegalStateException(path + " must be a JSON array");
    }
    return (List<Object>) list;
  }
}
