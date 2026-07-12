package org.hyperledger.iroha.android.offline;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;

public final class OfflineJsonParser {
  private static final BigInteger U64_MAX =
      BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
  private OfflineJsonParser() {}

  public static OfflineReadiness parseOfflineReadiness(final byte[] payload) {
    final Object root = parse(payload);
    final Map<String, Object> object = expectObject(root, "root");
    final List<Object> rawBlockers = asList(object.get("blockers"), "blockers");
    final List<OfflineReadinessBlocker> blockers = new ArrayList<>(rawBlockers.size());
    for (int i = 0; i < rawBlockers.size(); i++) {
      final String path = "blockers[" + i + "]";
      final Map<String, Object> blocker = expectObject(rawBlockers.get(i), path);
      blockers.add(
          new OfflineReadinessBlocker(
              asExactReadinessString(blocker.get("code"), path + ".code"),
              asExactReadinessString(blocker.get("message"), path + ".message")));
    }
    final Object rawAssetScale = required(object, "asset_scale", "root");
    final BigInteger evaluatedBlockHeight =
        asReadinessU64(required(object, "evaluated_block_height", "root"),
            "evaluated_block_height");
    final Object rawActiveTransferVerifier =
        required(object, "active_transfer_verifier", "root");
    return new OfflineReadiness(
        asExactReadinessString(
            required(object, "asset_definition_id", "root"), "asset_definition_id"),
        rawAssetScale == null ? null : Long.valueOf(asReadinessU32(rawAssetScale, "asset_scale")),
        evaluatedBlockHeight,
        asExactLowercaseHash(
            required(object, "evaluated_block_hash", "root"), "evaluated_block_hash"),
        rawActiveTransferVerifier == null
            ? null
            : parseActiveTransferVerifier(
                rawActiveTransferVerifier,
                evaluatedBlockHeight,
                "active_transfer_verifier"),
        asBoolean(required(object, "ready", "root"), "ready"),
        blockers);
  }

  private static OfflineActiveTransferVerifier parseActiveTransferVerifier(
      final Object value, final BigInteger evaluatedBlockHeight, final String path) {
    final Map<String, Object> object = expectObject(value, path);
    final String idPath = path + ".id";
    final Map<String, Object> id = expectObject(required(object, "id", path), idPath);
    final Object rawWithdrawalHeight = required(object, "withdrawal_height", path);
    final OfflineActiveTransferVerifier verifier =
        new OfflineActiveTransferVerifier(
            new OfflineVerifierId(
                asExactReadinessString(required(id, "backend", idPath), idPath + ".backend"),
                asExactReadinessString(required(id, "name", idPath), idPath + ".name")),
            asReadinessU32(required(object, "version", path), path + ".version"),
            asExactReadinessString(
                required(object, "circuit_id", path), path + ".circuit_id"),
            asExactLowercaseHash(
                required(object, "commitment", path), path + ".commitment"),
            asExactLowercaseHash(
                required(object, "public_inputs_schema_hash", path),
                path + ".public_inputs_schema_hash"),
            asReadinessU32(
                required(object, "max_proof_bytes", path), path + ".max_proof_bytes"),
            asReadinessU64(
                required(object, "activation_height", path), path + ".activation_height"),
            rawWithdrawalHeight == null
                ? null
                : asReadinessU64(rawWithdrawalHeight, path + ".withdrawal_height"));
    if (!verifier.isActiveAt(evaluatedBlockHeight)) {
      throw new IllegalStateException(path + " must be active at evaluated_block_height");
    }
    return verifier;
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
      final String itemPath = "items[" + i + "]";
      items.add(parseTransferItem(expectObject(rawItems.get(i), itemPath), itemPath));
    }
    final long total = asOptionalLong(object.get("total"), "total", rawItems.size());
    return new OfflineTransferList(items, total);
  }

  private static Object parse(final byte[] payload) {
    final String json;
    try {
      json =
          StandardCharsets.UTF_8
              .newDecoder()
              .onMalformedInput(CodingErrorAction.REPORT)
              .onUnmappableCharacter(CodingErrorAction.REPORT)
              .decode(ByteBuffer.wrap(payload))
              .toString();
    } catch (final CharacterCodingException error) {
      throw new IllegalStateException("Offline JSON payload must be valid UTF-8", error);
    }
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

  private static Object required(
      final Map<String, Object> object, final String field, final String path) {
    if (!object.containsKey(field)) {
      throw new IllegalStateException(path + "." + field + " is required");
    }
    return object.get(field);
  }

  private static boolean asBoolean(final Object value, final String path) {
    if (!(value instanceof Boolean bool)) {
      throw new IllegalStateException(path + " must be a boolean");
    }
    return bool.booleanValue();
  }

  private static String asExactReadinessString(final Object value, final String path) {
    if (!(value instanceof String string)) {
      throw new IllegalStateException(path + " must be a string");
    }
    if (string.isEmpty() || !string.equals(string.trim())) {
      throw new IllegalStateException(path + " must be an exact non-empty string");
    }
    return string;
  }

  private static String asExactLowercaseHash(final Object value, final String path) {
    final String string = asExactReadinessString(value, path);
    if (string.length() != 64) {
      throw new IllegalStateException(path + " must be exact lowercase 32-byte hexadecimal");
    }
    for (int index = 0; index < string.length(); index++) {
      final char character = string.charAt(index);
      if (!((character >= '0' && character <= '9') || (character >= 'a' && character <= 'f'))) {
        throw new IllegalStateException(path + " must be exact lowercase 32-byte hexadecimal");
      }
    }
    return string;
  }

  private static BigInteger asReadinessU64(final Object value, final String path) {
    final BigInteger integer;
    if (value instanceof BigInteger bigInteger) {
      integer = bigInteger;
    } else if (value instanceof BigDecimal) {
      throw new IllegalStateException(path + " must be a JSON integer number");
    } else if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      integer = BigInteger.valueOf(((Number) value).longValue());
    } else if (value instanceof Float || value instanceof Double) {
      throw new IllegalStateException(path + " must be an integer");
    } else {
      throw new IllegalStateException(path + " must be a JSON integer number");
    }
    if (integer.signum() < 0 || integer.compareTo(U64_MAX) > 0) {
      throw new IllegalStateException(path + " must fit in an unsigned 64-bit integer");
    }
    return integer;
  }

  private static long asReadinessU32(final Object value, final String path) {
    final BigInteger integer = asReadinessU64(value, path);
    if (integer.compareTo(BigInteger.valueOf(0xffff_ffffL)) > 0) {
      throw new IllegalStateException(path + " must fit in an unsigned 32-bit integer");
    }
    return integer.longValue();
  }

  private static OfflineTransferList.OfflineTransferItem parseTransferItem(
      final Map<String, Object> object, final String path) {
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
        asOptionalLong(
            pick(object, "receipt_count", "receiptCount"),
            path + ".receipt_count",
            summaries.size()),
        asOptionalLong(
            pick(object, "recorded_at_ms", "recordedAtMs"), path + ".recorded_at_ms", 0L),
        asOptionalLong(
            pick(object, "recorded_at_height", "recordedAtHeight"),
            path + ".recorded_at_height",
            0L),
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

  private static long asOptionalLong(
      final Object value, final String path, final long defaultValue) {
    if (value == null) {
      return defaultValue;
    }
    if (value instanceof BigInteger bigInteger) {
      return checkedLong(bigInteger, path);
    }
    if (value instanceof BigDecimal) {
      throw new IllegalStateException(path + " must be an integer");
    }
    if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      return ((Number) value).longValue();
    }
    if (value instanceof Float || value instanceof Double) {
      throw new IllegalStateException(path + " must be an integer");
    }
    throw new IllegalStateException(path + " must be a JSON integer number");
  }

  private static long checkedLong(final BigInteger value, final String path) {
    try {
      return value.longValueExact();
    } catch (final ArithmeticException ex) {
      throw new IllegalStateException(path + " must fit in signed 64-bit range", ex);
    }
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
