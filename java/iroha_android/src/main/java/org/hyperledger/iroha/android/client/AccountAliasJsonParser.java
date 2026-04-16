package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Map;

/** Minimal JSON parser for Torii account alias resolution responses. */
public final class AccountAliasJsonParser {

  private AccountAliasJsonParser() {}

  public static AccountAliasResolution parseResolution(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "account alias resolution"), "account alias resolution");
    return new AccountAliasResolution(
        requiredString(root.get("alias"), "account alias resolution.alias"),
        requiredString(root.get("account_id"), "account alias resolution.account_id"),
        root.containsKey("index")
            ? asOptionalLong(root.get("index"), "account alias resolution.index")
            : null,
        optionalString(root.get("source")));
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

  private static String requiredString(final Object value, final String path) {
    final String string = optionalString(value);
    if (string == null || string.isBlank()) {
      throw new IllegalStateException(path + " must be a non-empty string");
    }
    return string;
  }

  private static String optionalString(final Object value) {
    if (value == null) {
      return null;
    }
    final String string = value instanceof String ? (String) value : String.valueOf(value);
    final String trimmed = string.trim();
    return trimmed.isEmpty() ? null : trimmed;
  }

  private static Long asOptionalLong(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    if (!(value instanceof Number number)) {
      throw new IllegalStateException(path + " must be a number");
    }
    if (number instanceof Float || number instanceof Double) {
      throw new IllegalStateException(path + " must be an integer");
    }
    return number.longValue();
  }
}
