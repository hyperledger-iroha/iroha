package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;

/** Minimal JSON parser for Torii account alias resolution responses. */
public final class AccountAliasJsonParser {

  private AccountAliasJsonParser() {}

  public static AccountAliasResolution parseResolution(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "account alias resolution"), "account alias resolution");
    exactKeys(
        root,
        new HashSet<>(Arrays.asList("alias", "account_id", "index", "source")),
        new HashSet<>(Arrays.asList("index", "source")),
        "account alias resolution");
    return new AccountAliasResolution(
        requiredExactString(root.get("alias"), "account alias resolution.alias"),
        requiredExactString(root.get("account_id"), "account alias resolution.account_id"),
        root.containsKey("index")
            ? asOptionalUInt64(root.get("index"), "account alias resolution.index")
            : null,
        optionalExactString(root.get("source"), "account alias resolution.source"));
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

  private static String requiredExactString(final Object value, final String path) {
    final String string = optionalString(value);
    if (string == null || string.trim().isEmpty()) {
      throw new IllegalStateException(path + " must be a non-empty string");
    }
    if (!string.trim().equals(string)) {
      throw new IllegalStateException(path + " must not contain surrounding whitespace");
    }
    return string;
  }

  private static String optionalExactString(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    return requiredExactString(value, path);
  }

  private static String optionalString(final Object value) {
    if (value == null) {
      return null;
    }
    if (!(value instanceof String)) {
      throw new IllegalStateException("account alias resolution string fields must be strings");
    }
    return (String) value;
  }

  private static java.math.BigInteger asOptionalUInt64(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    return AccountAliasUInt64.parse(value, path);
  }

  private static void exactKeys(
      final Map<String, Object> root,
      final Set<String> allowed,
      final Set<String> optional,
      final String path) {
    final Set<String> unknown = new HashSet<>(root.keySet());
    unknown.removeAll(allowed);
    if (!unknown.isEmpty()) {
      throw new IllegalStateException(path + " contains unknown or retired fields");
    }
    final Set<String> missing = new HashSet<>(allowed);
    missing.removeAll(optional);
    missing.removeAll(root.keySet());
    if (!missing.isEmpty()) {
      throw new IllegalStateException(path + " is missing required fields");
    }
  }
}
