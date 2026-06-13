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
    final ArrayList<ZkMerklePathResponse.Entry> paths = new ArrayList<>(pathValues.size());
    for (int i = 0; i < pathValues.size(); i++) {
      final Object rawPath = pathValues.get(i);
      if (!(rawPath instanceof Map<?, ?> path)) {
        throw new IllegalArgumentException("paths[" + i + "] must be an object");
      }
      paths.add(
          new ZkMerklePathResponse.Entry(
              jsonString(path.get("commitment"), "paths[" + i + "].commitment"),
              jsonInt(path.get("leaf_index"), "paths[" + i + "].leaf_index"),
              jsonStringList(path.get("siblings"), "paths[" + i + "].siblings"),
              jsonDirections(path.get("directions"), "paths[" + i + "].directions"),
              jsonStringList(path.get("witness_nodes"), "paths[" + i + "].witness_nodes"),
              jsonString(path.get("root"), "paths[" + i + "].root")));
    }
    return new ZkMerklePathResponse(
        jsonString(map.get("root"), "root"),
        jsonInt(map.get("frontier_len"), "frontier_len"),
        jsonInt(map.get("tree_depth"), "tree_depth"),
        paths);
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
