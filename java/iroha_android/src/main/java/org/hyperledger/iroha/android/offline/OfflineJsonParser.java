package org.hyperledger.iroha.android.offline;

import java.math.BigDecimal;
import java.math.BigInteger;
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
    rejectRemovedKagemushaAbi7ReadinessFields(object);
    return new OfflineReadiness(
        asOptionalBoolean(object.get("offline_note"), false),
        asOptionalBoolean(object.get("offline_one_use_keys"), false),
        asOptionalBoolean(object.get("offline_recursive_note_proof"), false),
        asOptionalBoolean(object.get("offline_fountain_qr"), false),
        asOptionalBoolean(object.get("offline_sync_optional"), false),
        asBoolean(object.get("offline_telemetry"), "offline_telemetry"),
        asBoolean(
            object.get("offline_kagemusha_recursive_compact_available"),
            "offline_kagemusha_recursive_compact_available"),
        asPresentReadinessString(
            object.get("offline_kagemusha_recursive_compact_mode"),
            "offline_kagemusha_recursive_compact_mode"),
        Integer.valueOf(asPresentReadinessInteger(
            object.get("offline_kagemusha_recursive_compact_required_native_bridge_abi_version"),
            "offline_kagemusha_recursive_compact_required_native_bridge_abi_version")),
        asPresentReadinessString(
            object.get("offline_kagemusha_recursive_compact_circuit_id"),
            "offline_kagemusha_recursive_compact_circuit_id"),
        asBoolean(
            object.get("offline_kagemusha_recursive_compact_artifacts_available"),
            "offline_kagemusha_recursive_compact_artifacts_available"));
  }

  public static OfflineV2Readiness parseOfflineV2Readiness(final byte[] payload) {
    final Object root = parse(payload);
    final Map<String, Object> object = expectObject(root, "root");
    rejectRemovedKagemushaAbi7ReadinessFields(object);
    return new OfflineV2Readiness(
        asBoolean(object.get("offline_telemetry"), "offline_telemetry"),
        asBoolean(
            object.get("offline_kagemusha_recursive_compact_available"),
            "offline_kagemusha_recursive_compact_available"),
        asPresentReadinessString(
            object.get("offline_kagemusha_recursive_compact_mode"),
            "offline_kagemusha_recursive_compact_mode"),
        Integer.valueOf(asPresentReadinessInteger(
            object.get("offline_kagemusha_recursive_compact_required_native_bridge_abi_version"),
            "offline_kagemusha_recursive_compact_required_native_bridge_abi_version")),
        asPresentReadinessString(
            object.get("offline_kagemusha_recursive_compact_circuit_id"),
            "offline_kagemusha_recursive_compact_circuit_id"),
        asBoolean(
            object.get("offline_kagemusha_recursive_compact_artifacts_available"),
            "offline_kagemusha_recursive_compact_artifacts_available"));
  }

  private static final String[] REMOVED_KAGEMUSHA_ABI7_READINESS_FIELDS = {
    "offline_kagemusha_abi7",
    "offline_kagemusha_abi7_mode",
    "offline_kagemusha_abi7_bridge_abi_version",
    "offline_kagemusha_abi7_circuit_id",
    "offline_kagemusha_abi7_artifacts"
  };

  private static void rejectRemovedKagemushaAbi7ReadinessFields(final Map<String, Object> object) {
    for (final String field : REMOVED_KAGEMUSHA_ABI7_READINESS_FIELDS) {
      if (object.containsKey(field)) {
        throw new IllegalStateException(
            field + " is not supported; use offline_kagemusha_recursive_compact_*");
      }
    }
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

  private static String asPresentReadinessString(final Object value, final String path) {
    if (!(value instanceof String string)) {
      throw new IllegalStateException(path + " must be a string");
    }
    if (string.isEmpty() || !string.equals(string.trim())) {
      throw new IllegalStateException(path + " must be an exact non-empty string");
    }
    return string;
  }

  private static int asPresentReadinessInteger(final Object value, final String path) {
    if (value instanceof String string) {
      if (string.isEmpty() || !string.equals(string.trim())) {
        throw new IllegalStateException(path + " must be an exact integer string");
      }
      if (!string.matches("[1-9][0-9]*")) {
        throw new IllegalStateException(path + " must be an exact integer string");
      }
      try {
        return asPositiveReadinessInteger(new BigInteger(string), path);
      } catch (final NumberFormatException ex) {
        throw new IllegalStateException(path + " must be an integer", ex);
      }
    } else if (value instanceof BigInteger bigInteger) {
      return asPositiveReadinessInteger(bigInteger, path);
    } else if (value instanceof BigDecimal bigDecimal) {
      try {
        return asPositiveReadinessInteger(bigDecimal.toBigIntegerExact(), path);
      } catch (final ArithmeticException ex) {
        throw new IllegalStateException(path + " must be an integer", ex);
      }
    } else if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      return asPositiveReadinessInteger(((Number) value).longValue(), path);
    } else {
      throw new IllegalStateException(path + " must be an integer");
    }
  }

  private static int asPositiveReadinessInteger(final BigInteger value, final String path) {
    if (value.signum() <= 0) {
      throw new IllegalStateException(path + " must be a positive integer");
    }
    try {
      return value.intValueExact();
    } catch (final ArithmeticException ex) {
      throw new IllegalStateException(path + " must fit in signed 32-bit range", ex);
    }
  }

  private static int asPositiveReadinessInteger(final long value, final String path) {
    if (value <= 0) {
      throw new IllegalStateException(path + " must be a positive integer");
    }
    try {
      return Math.toIntExact(value);
    } catch (final ArithmeticException ex) {
      throw new IllegalStateException(path + " must fit in signed 32-bit range", ex);
    }
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

  private static String asNullableString(final Object value) {
    return asOptionalString(value);
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
