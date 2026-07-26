package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
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
    final Object evaluatedBlockHash = map.get("evaluated_block_hash");
    if (!(evaluatedBlockHash instanceof String blockHash)) {
      throw new IllegalArgumentException("evaluated_block_hash must be a string");
    }
    return new ZkRootsResponse(
        latestString,
        rootStrings,
        jsonUnsignedLong(map.get("evaluated_block_height"), "evaluated_block_height"),
        blockHash);
  }

  private static int jsonInt(final Object value, final String field) {
    final long parsed;
    if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      final Number number = (Number) value;
      parsed = number.longValue();
    } else if (value instanceof BigInteger bigInteger) {
      if (bigInteger.signum() < 0 || bigInteger.compareTo(BigInteger.valueOf(Integer.MAX_VALUE)) > 0) {
        throw new IllegalArgumentException(field + " is outside u32-compatible Int range");
      }
      parsed = bigInteger.longValue();
    } else {
      throw new IllegalArgumentException(field + " must be a JSON integer");
    }
    if (parsed < 0 || parsed > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(field + " is outside u32-compatible Int range");
    }
    return (int) parsed;
  }

  static long jsonUnsignedLong(final Object value, final String field) {
    final long parsed;
    if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      parsed = ((Number) value).longValue();
    } else if (value instanceof BigInteger bigInteger) {
      if (bigInteger.signum() < 0 || bigInteger.compareTo(BigInteger.valueOf(Long.MAX_VALUE)) > 0) {
        throw new IllegalArgumentException(field + " is outside the supported uint64 range");
      }
      parsed = bigInteger.longValue();
    } else {
      throw new IllegalArgumentException(field + " must be a JSON integer");
    }
    if (parsed < 0) {
      throw new IllegalArgumentException(field + " is outside the supported uint64 range");
    }
    return parsed;
  }
}
