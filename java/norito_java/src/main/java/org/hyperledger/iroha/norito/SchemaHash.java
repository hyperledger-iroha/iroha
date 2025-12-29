// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.Map;
import java.util.Objects;
import java.util.TreeMap;
import java.util.Locale;

/** Computes FNV-1a 64-bit schema hashes matching the Rust implementation. */
public final class SchemaHash {
  private static final long OFFSET_BASIS = 0xCBF29CE484222325L;
  private static final long FNV_PRIME = 0x100000001B3L;

  private SchemaHash() {}

  public static byte[] hash16(String canonicalPath) {
    byte[] input = canonicalPath.getBytes(StandardCharsets.UTF_8);
    long hash = OFFSET_BASIS;
    for (byte b : input) {
      hash ^= (b & 0xFFL);
      hash = (hash * FNV_PRIME) & 0xFFFFFFFFFFFFFFFFL;
    }
    ByteBuffer buffer = ByteBuffer.allocate(16);
    buffer.putLong(Long.reverseBytes(hash));
    buffer.putLong(Long.reverseBytes(hash));
    return buffer.array();
  }

  public static byte[] hash16FromStructural(Object schema) {
    Objects.requireNonNull(schema, "schema");
    String canonical = encodeCanonicalJson(schema);
    byte[] input = canonical.getBytes(StandardCharsets.UTF_8);
    long hash = OFFSET_BASIS;
    for (byte b : input) {
      hash ^= (b & 0xFFL);
      hash = (hash * FNV_PRIME) & 0xFFFFFFFFFFFFFFFFL;
    }
    ByteBuffer buffer = ByteBuffer.allocate(16);
    buffer.putLong(Long.reverseBytes(hash));
    buffer.putLong(Long.reverseBytes(hash));
    return buffer.array();
  }

  private static String encodeCanonicalJson(Object value) {
    StringBuilder out = new StringBuilder();
    encodeCanonicalJson(value, out);
    return out.toString();
  }

  private static void encodeCanonicalJson(Object value, StringBuilder out) {
    if (value == null) {
      out.append("null");
    } else if (value instanceof Boolean b) {
      out.append(b ? "true" : "false");
    } else if (value instanceof String s) {
      encodeJsonString(s, out);
    } else if (value instanceof Number number && !(number instanceof Float || number instanceof Double)) {
      out.append(number.toString());
    } else if (value instanceof Double d) {
      out.append(encodeFloat(d.doubleValue()));
    } else if (value instanceof Float f) {
      out.append(encodeFloat(f.doubleValue()));
    } else if (value instanceof Map<?, ?> map) {
      out.append('{');
      TreeMap<String, Object> sorted = new TreeMap<>();
      for (Map.Entry<?, ?> entry : map.entrySet()) {
        if (!(entry.getKey() instanceof String)) {
          throw new IllegalArgumentException("Structural schema keys must be strings");
        }
        sorted.put((String) entry.getKey(), entry.getValue());
      }
      boolean first = true;
      for (Map.Entry<String, Object> entry : sorted.entrySet()) {
        if (!first) {
          out.append(',');
        }
        encodeJsonString(entry.getKey(), out);
        out.append(':');
        encodeCanonicalJson(entry.getValue(), out);
        first = false;
      }
      out.append('}');
    } else if (value instanceof Iterable<?> iterable) {
      out.append('[');
      boolean first = true;
      for (Object item : iterable) {
        if (!first) {
          out.append(',');
        }
        encodeCanonicalJson(item, out);
        first = false;
      }
      out.append(']');
    } else if (value.getClass().isArray()) {
      out.append('[');
      int length = java.lang.reflect.Array.getLength(value);
      for (int i = 0; i < length; i++) {
        if (i > 0) {
          out.append(',');
        }
        Object element = java.lang.reflect.Array.get(value, i);
        encodeCanonicalJson(element, out);
      }
      out.append(']');
    } else {
      throw new IllegalArgumentException("Unsupported structural schema element: " + value.getClass());
    }
  }

  private static String encodeFloat(double value) {
    if (Double.isNaN(value)) {
      return "NaN";
    }
    if (Double.isInfinite(value)) {
      return value > 0 ? "inf" : "-inf";
    }
    double abs = Math.abs(value);
    if (Math.rint(value) == value && abs <= 9_007_199_254_740_992.0) {
      return String.format(java.util.Locale.ROOT, "%.1f", value);
    }
    return String.format(java.util.Locale.ROOT, "%.17g", value);
  }

  private static void encodeJsonString(String value, StringBuilder out) {
    out.append('"');
    for (int i = 0; i < value.length(); ) {
      int codePoint = value.codePointAt(i);
      i += Character.charCount(codePoint);
      switch (codePoint) {
        case '"' -> out.append("\\\"");
        case '\\' -> out.append("\\\\");
        case '\n' -> out.append("\\n");
        case '\r' -> out.append("\\r");
        case '\t' -> out.append("\\t");
        case '\b' -> out.append("\\b");
        case '\f' -> out.append("\\f");
        case 0x2028 -> out.append("\\u2028");
        case 0x2029 -> out.append("\\u2029");
        default -> {
          if (codePoint < 0x20) {
            out.append(String.format(Locale.ROOT, "\\u%04X", codePoint));
          } else if (codePoint >= 0x10000) {
            int tmp = codePoint - 0x10000;
            int hi = 0xD800 + (tmp >> 10);
            int lo = 0xDC00 + (tmp & 0x3FF);
            out.append(String.format(Locale.ROOT, "\\u%04X\\u%04X", hi, lo));
          } else {
            out.appendCodePoint(codePoint);
          }
        }
      }
    }
    out.append('"');
  }
}
