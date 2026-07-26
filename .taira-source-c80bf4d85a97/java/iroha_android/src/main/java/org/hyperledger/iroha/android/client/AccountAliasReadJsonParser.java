package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** Strict parsers for typed alias index and account-list responses. */
public final class AccountAliasReadJsonParser {
  private AccountAliasReadJsonParser() {}

  /** Parses {@code /v1/aliases/resolve-index}. */
  public static AccountAliasIndexResolution parseIndexResolution(final byte[] payload) {
    final Map<String, Object> root = root(payload, "alias index resolution");
    exactKeys(
        root,
        set("index", "alias", "account_id", "source"),
        set("source"),
        "alias index resolution");
    return new AccountAliasIndexResolution(
        AccountAliasUInt64.parse(root.get("index"), "alias index resolution.index"),
        string(root.get("alias"), "alias index resolution.alias"),
        string(root.get("account_id"), "alias index resolution.account_id"),
        optionalString(root.get("source"), "alias index resolution.source"));
  }

  /** Parses {@code /v1/aliases/by-account}. */
  public static AccountAliasesByAccount parseByAccount(final byte[] payload) {
    final Map<String, Object> root = root(payload, "aliases by account");
    exactKeys(
        root,
        set("account_id", "total", "items", "source"),
        set("source"),
        "aliases by account");
    final Object rawItems = root.get("items");
    if (!(rawItems instanceof List<?>)) {
      throw new IllegalStateException("aliases by account.items must be an array");
    }
    final List<?> values = (List<?>) rawItems;
    final List<AccountAliasListItem> items = new ArrayList<>(values.size());
    for (int index = 0; index < values.size(); index++) {
      final String path = "aliases by account.items[" + index + "]";
      final Map<String, Object> item = object(values.get(index), path);
      exactKeys(item, set("alias", "dataspace", "domain", "is_primary"), set("domain"), path);
      if (!(item.get("is_primary") instanceof Boolean)) {
        throw new IllegalStateException(path + ".is_primary must be a boolean");
      }
      items.add(
          new AccountAliasListItem(
              string(item.get("alias"), path + ".alias"),
              string(item.get("dataspace"), path + ".dataspace"),
              optionalString(item.get("domain"), path + ".domain"),
              ((Boolean) item.get("is_primary")).booleanValue()));
    }
    return new AccountAliasesByAccount(
        string(root.get("account_id"), "aliases by account.account_id"),
        AccountAliasUInt64.parse(root.get("total"), "aliases by account.total"),
        items,
        optionalString(root.get("source"), "aliases by account.source"));
  }

  private static Map<String, Object> root(final byte[] payload, final String path) {
    if (payload == null || payload.length == 0) {
      throw new IllegalStateException(path + " returned an empty payload");
    }
    return object(JsonParser.parse(new String(payload, StandardCharsets.UTF_8)), path);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> object(final Object value, final String path) {
    if (!(value instanceof Map<?, ?>)) throw new IllegalStateException(path + " must be an object");
    final Map<?, ?> map = (Map<?, ?>) value;
    for (final Object key : map.keySet()) {
      if (!(key instanceof String)) throw new IllegalStateException(path + " keys must be strings");
    }
    return (Map<String, Object>) map;
  }

  private static void exactKeys(
      final Map<String, Object> root,
      final Set<String> allowed,
      final Set<String> optional,
      final String path) {
    final Set<String> unknown = new HashSet<>(root.keySet());
    unknown.removeAll(allowed);
    if (!unknown.isEmpty()) throw new IllegalStateException(path + " contains unknown fields");
    final Set<String> missing = new HashSet<>(allowed);
    missing.removeAll(optional);
    missing.removeAll(root.keySet());
    if (!missing.isEmpty()) throw new IllegalStateException(path + " is missing required fields");
  }

  private static Set<String> set(final String... values) {
    return new HashSet<>(Arrays.asList(values));
  }

  private static String string(final Object value, final String path) {
    if (!(value instanceof String)) throw new IllegalStateException(path + " must be a string");
    return AccountAliasesByAccount.requireExactText((String) value, path);
  }

  private static String optionalString(final Object value, final String path) {
    return value == null ? null : string(value, path);
  }
}
