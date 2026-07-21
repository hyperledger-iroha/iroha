package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

final class ZkMerklePathJson {
  private ZkMerklePathJson() {}

  static ZkMerklePathResponse parseResponse(final byte[] payload) {
    final Object root = JsonParser.parse(new String(payload, StandardCharsets.UTF_8).trim());
    if (!(root instanceof Map<?, ?> map)) {
      throw new IllegalArgumentException("zk merkle-path response must be a JSON object");
    }
    final Object rawPaths = map.get("paths");
    if (!(rawPaths instanceof List<?> pathValues)) {
      throw new IllegalArgumentException("paths must be an array");
    }
    if (!map.containsKey("next_zero_path")) {
      throw new IllegalArgumentException("next_zero_path field is required");
    }
    final Object rawNextZeroPath = map.get("next_zero_path");
    final ZkMerklePathResponse.Entry nextZeroPath;
    if (rawNextZeroPath == null) {
      nextZeroPath = null;
    } else if (rawNextZeroPath instanceof Map<?, ?> path) {
      nextZeroPath = parseEntry(path, "next_zero_path");
    } else {
      throw new IllegalArgumentException("next_zero_path must be an object or null");
    }
    final ArrayList<ZkMerklePathResponse.Entry> paths = new ArrayList<>(pathValues.size());
    for (int i = 0; i < pathValues.size(); i++) {
      final Object rawPath = pathValues.get(i);
      if (!(rawPath instanceof Map<?, ?> path)) {
        throw new IllegalArgumentException("paths[" + i + "] must be an object");
      }
      paths.add(parseEntry(path, "paths[" + i + "]"));
    }
    return new ZkMerklePathResponse(
        ZkRootsJson.jsonUnsignedLong(
            map.get("evaluated_block_height"), "evaluated_block_height"),
        jsonString(map.get("evaluated_block_hash"), "evaluated_block_hash"),
        jsonString(map.get("root"), "root"),
        jsonInt(map.get("frontier_len"), "frontier_len"),
        jsonInt(map.get("tree_depth"), "tree_depth"),
        nextZeroPath,
        paths);
  }

  private static ZkMerklePathResponse.Entry parseEntry(
      final Map<?, ?> path, final String field) {
    return new ZkMerklePathResponse.Entry(
        jsonString(path.get("commitment"), field + ".commitment"),
        jsonInt(path.get("leaf_index"), field + ".leaf_index"),
        jsonStringList(path.get("siblings"), field + ".siblings"),
        jsonDirections(path.get("directions"), field + ".directions"),
        jsonStringList(path.get("witness_nodes"), field + ".witness_nodes"),
        jsonString(path.get("root"), field + ".root"));
  }

  private static String jsonString(final Object value, final String field) {
    if (!(value instanceof String string)) {
      throw new IllegalArgumentException(field + " must be a string");
    }
    return string;
  }

  private static List<String> jsonStringList(final Object value, final String field) {
    if (!(value instanceof List<?> list)) {
      throw new IllegalArgumentException(field + " must be an array");
    }
    final ArrayList<String> out = new ArrayList<>(list.size());
    for (int i = 0; i < list.size(); i++) {
      final Object item = list.get(i);
      if (!(item instanceof String string)) {
        throw new IllegalArgumentException(field + "[" + i + "] must be a string");
      }
      out.add(string);
    }
    return out;
  }

  private static byte[] jsonDirections(final Object value, final String field) {
    if (!(value instanceof List<?> list)) {
      throw new IllegalArgumentException(field + " must be an array");
    }
    final byte[] out = new byte[list.size()];
    for (int i = 0; i < list.size(); i++) {
      final Object item = list.get(i);
      final long parsed;
      if (item instanceof Byte
          || item instanceof Short
          || item instanceof Integer
          || item instanceof Long) {
        final Number number = (Number) item;
        parsed = number.longValue();
      } else if (item instanceof BigInteger bigInteger) {
        if (!BigInteger.ZERO.equals(bigInteger) && !BigInteger.ONE.equals(bigInteger)) {
          throw new IllegalArgumentException(field + "[" + i + "] must be 0 or 1");
        }
        parsed = bigInteger.longValue();
      } else {
        throw new IllegalArgumentException(field + "[" + i + "] must be a JSON integer");
      }
      if (parsed != 0L && parsed != 1L) {
        throw new IllegalArgumentException(field + "[" + i + "] must be 0 or 1");
      }
      out[i] = (byte) parsed;
    }
    return out;
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
}
