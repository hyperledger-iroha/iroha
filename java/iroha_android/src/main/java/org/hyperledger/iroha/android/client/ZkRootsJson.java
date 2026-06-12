package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

final class ZkRootsJson {
  private ZkRootsJson() {}

  static ZkRootsResponse parseResponse(final byte[] payload) {
    final Object root = JsonParser.parse(new String(payload, StandardCharsets.UTF_8).trim());
    if (!(root instanceof Map<?, ?> map)) {
      throw new IllegalArgumentException("zk roots response must be a JSON object");
    }
    final Object latest = map.get("latest");
    if (!(latest instanceof String latestString)) {
      throw new IllegalArgumentException("latest must be a string");
    }
    final Object roots = map.get("roots");
    if (!(roots instanceof List<?> rootList)) {
      throw new IllegalArgumentException("roots must be an array");
    }
    final ArrayList<String> rootStrings = new ArrayList<>(rootList.size());
    for (int i = 0; i < rootList.size(); i++) {
      final Object value = rootList.get(i);
      if (!(value instanceof String string)) {
        throw new IllegalArgumentException("roots[" + i + "] must be a string");
      }
      rootStrings.add(string);
    }
    return new ZkRootsResponse(latestString, rootStrings, jsonInt(map.get("height"), "height"));
  }

  private static int jsonInt(final Object value, final String field) {
    final long parsed;
    if (value instanceof Number number) {
      parsed = number.longValue();
    } else if (value instanceof String string) {
      parsed = Long.parseLong(string.trim());
    } else {
      throw new IllegalArgumentException(field + " must be an integer");
    }
    if (parsed < 0 || parsed > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(field + " is outside u32-compatible Int range");
    }
    return (int) parsed;
  }
}
