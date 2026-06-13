package org.hyperledger.iroha.android.client;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;

/** Response body emitted by {@code POST /v1/zk/roots}. */
public final class ZkRootsResponse {
  private final String latest;
  private final List<String> roots;
  private final int height;

  public ZkRootsResponse(final String latest, final List<String> roots, final int height) {
    this.latest = normalizeRootHexOrEmpty(latest, "latest");
    final List<String> checkedRoots = Objects.requireNonNull(roots, "roots");
    final ArrayList<String> normalized = new ArrayList<>(checkedRoots.size());
    for (int i = 0; i < checkedRoots.size(); i++) {
      normalized.add(normalizeRootHex(checkedRoots.get(i), "roots[" + i + "]"));
    }
    if (height < 0) {
      throw new IllegalArgumentException("height must be non-negative");
    }
    this.roots = Collections.unmodifiableList(normalized);
    this.height = height;
  }

  public String latest() {
    return latest;
  }

  public List<String> roots() {
    return roots;
  }

  public int height() {
    return height;
  }

  public byte[] latestRootBytes() {
    return latest.isEmpty() ? null : decodeHex32(latest, "latest");
  }

  public byte[] rootBytes(final int index) {
    return decodeHex32(roots.get(index), "roots[" + index + "]");
  }

  static ZkRootsResponse parse(final byte[] payload) {
    return ZkRootsJson.parseResponse(payload);
  }

  static String normalizeRootHexOrEmpty(final String value, final String field) {
    final String checked =
        java.util.Objects.requireNonNull(value, field + " must not be null");
    if (!checked.trim().equals(checked)) {
      throw new IllegalArgumentException(field + " must be canonical lowercase hex or empty");
    }
    if (checked.isEmpty()) {
      return "";
    }
    return normalizeRootHex(checked, field);
  }

  static String normalizeRootHex(final String value, final String field) {
    final String normalized = HttpClientTransport.normalizeHex32(value, field);
    if (!normalized.equals(value)) {
      throw new IllegalArgumentException(field + " must be canonical lowercase hex");
    }
    return normalized;
  }

  public static byte[] decodeHex32(final String value, final String field) {
    final String normalized = normalizeRootHex(value, field);
    final byte[] out = new byte[32];
    for (int i = 0; i < out.length; i++) {
      out[i] =
          (byte)
              ((hexDigit(normalized.charAt(2 * i), field, 2 * i) << 4)
                  | hexDigit(normalized.charAt(2 * i + 1), field, 2 * i + 1));
    }
    return out;
  }

  public static String encodeHex(final byte[] bytes) {
    return encodeHex(bytes, "bytes");
  }

  public static String encodeHex(final byte[] bytes, final String field) {
    if (bytes == null || bytes.length != 32) {
      throw new IllegalArgumentException(field + " must be 32 bytes");
    }
    final char[] out = new char[64];
    final char[] digits = "0123456789abcdef".toCharArray();
    for (int i = 0; i < bytes.length; i++) {
      final int value = bytes[i] & 0xff;
      out[2 * i] = digits[value >>> 4];
      out[2 * i + 1] = digits[value & 0x0f];
    }
    return new String(out);
  }

  private static int hexDigit(final char c, final String field, final int index) {
    if (c >= '0' && c <= '9') {
      return c - '0';
    }
    if (c >= 'a' && c <= 'f') {
      return c - 'a' + 10;
    }
    throw new IllegalArgumentException(
        "invalid lowercase hex digit `" + c + "` at " + field + "[" + index + "]");
  }
}
