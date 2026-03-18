package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.nexus.UaidLiteral;

/** Minimal JSON parser for identifier-policy and identifier-resolution payloads. */
public final class IdentifierJsonParser {

  private IdentifierJsonParser() {}

  public static IdentifierPolicyListResponse parsePolicyList(final byte[] payload) {
    final Map<String, Object> root = expectObject(parse(payload, "identifier policy list"), "identifier policy list");
    final List<Object> itemValues = asArrayOrEmpty(root.get("items"), "identifier policy list.items");
    final List<IdentifierPolicySummary> items = new ArrayList<>(itemValues.size());
    for (int i = 0; i < itemValues.size(); i++) {
      final Map<String, Object> item =
          expectObject(itemValues.get(i), "identifier policy list.items[" + i + "]");
      items.add(
          new IdentifierPolicySummary(
              requiredString(item.get("policy_id"), "identifier policy list.items[" + i + "].policy_id"),
              requiredString(item.get("owner"), "identifier policy list.items[" + i + "].owner"),
              Boolean.TRUE.equals(item.get("active")),
              IdentifierNormalization.fromWireValue(
                  requiredString(
                      item.get("normalization"),
                      "identifier policy list.items[" + i + "].normalization")),
              requiredString(
                  item.get("resolver_public_key"),
                  "identifier policy list.items[" + i + "].resolver_public_key"),
              requiredString(item.get("backend"), "identifier policy list.items[" + i + "].backend"),
              optionalString(item.get("input_encryption")),
              optionalString(item.get("input_encryption_public_parameters")),
              optionalString(item.get("note"))));
    }
    final long total =
        root.containsKey("total")
            ? asLong(root.get("total"), "identifier policy list.total")
            : items.size();
    return new IdentifierPolicyListResponse(total, items);
  }

  public static IdentifierResolutionReceipt parseResolutionReceipt(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "identifier resolution receipt"), "identifier resolution receipt");
    return new IdentifierResolutionReceipt(
        requiredString(root.get("policy_id"), "identifier resolution receipt.policy_id"),
        canonicalizeOpaque(
            requiredString(root.get("opaque_id"), "identifier resolution receipt.opaque_id"),
            "identifier resolution receipt.opaque_id"),
        canonicalizeHex32(
            requiredString(root.get("receipt_hash"), "identifier resolution receipt.receipt_hash"),
            "identifier resolution receipt.receipt_hash"),
        UaidLiteral.canonicalize(
            requiredString(root.get("uaid"), "identifier resolution receipt.uaid"),
            "identifier resolution receipt.uaid"),
        requiredString(root.get("account_id"), "identifier resolution receipt.account_id"),
        asLong(root.get("resolved_at_ms"), "identifier resolution receipt.resolved_at_ms"),
        root.containsKey("expires_at_ms")
            ? asOptionalLong(root.get("expires_at_ms"), "identifier resolution receipt.expires_at_ms")
            : null,
        requiredString(root.get("backend"), "identifier resolution receipt.backend"),
        requiredString(root.get("signature"), "identifier resolution receipt.signature"));
  }

  private static Object parse(final byte[] payload, final String context) {
    if (payload == null || payload.length == 0) {
      throw new IllegalStateException(context + " returned an empty payload");
    }
    final String json = new String(payload, StandardCharsets.UTF_8).trim();
    if (json.isEmpty()) {
      throw new IllegalStateException(context + " returned a blank payload");
    }
    return JsonParser.parse(json);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> expectObject(final Object value, final String path) {
    if (!(value instanceof Map<?, ?>)) {
      throw new IllegalStateException(path + " must be a JSON object");
    }
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> asArrayOrEmpty(final Object value, final String path) {
    if (value == null) {
      return List.of();
    }
    if (!(value instanceof List<?> list)) {
      throw new IllegalStateException(path + " must be a JSON array");
    }
    return (List<Object>) list;
  }

  private static String requiredString(final Object value, final String path) {
    final String string = optionalString(value);
    if (string == null || string.isBlank()) {
      throw new IllegalStateException(path + " must be a non-empty string");
    }
    return string.trim();
  }

  private static String optionalString(final Object value) {
    if (value == null) {
      return null;
    }
    return value instanceof String string ? string : String.valueOf(value);
  }

  private static long asLong(final Object value, final String path) {
    if (!(value instanceof Number number)) {
      throw new IllegalStateException(path + " must be a number");
    }
    if (number instanceof Float || number instanceof Double) {
      throw new IllegalStateException(path + " must be an integer");
    }
    return number.longValue();
  }

  private static Long asOptionalLong(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    return asLong(value, path);
  }

  private static String canonicalizeOpaque(final String value, final String context) {
    Objects.requireNonNull(context, "context");
    final String literal = Objects.requireNonNull(value, context + " must not be null").trim();
    if (literal.isEmpty()) {
      throw new IllegalArgumentException(context + " must not be blank");
    }
    final String lower = literal.toLowerCase(Locale.ROOT);
    final String hexPortion =
        lower.startsWith("opaque:") ? literal.substring("opaque:".length()) : literal;
    final String trimmedHex = hexPortion.trim();
    if (trimmedHex.length() != 64 || !trimmedHex.matches("(?i)[0-9a-f]{64}")) {
      throw new IllegalArgumentException(context + " must contain 64 hex characters");
    }
    return "opaque:" + trimmedHex.toLowerCase(Locale.ROOT);
  }

  private static String canonicalizeHex32(final String value, final String context) {
    Objects.requireNonNull(context, "context");
    String trimmed = Objects.requireNonNull(value, context + " must not be null").trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(context + " must not be blank");
    }
    if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
      trimmed = trimmed.substring(2);
    }
    if (trimmed.length() != 64 || !trimmed.matches("(?i)[0-9a-f]{64}")) {
      throw new IllegalArgumentException(context + " must contain 64 hex characters");
    }
    return trimmed.toLowerCase(Locale.ROOT);
  }
}
