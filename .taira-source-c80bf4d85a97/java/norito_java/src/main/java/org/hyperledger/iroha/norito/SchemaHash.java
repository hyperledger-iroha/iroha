// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Map;
import java.util.Objects;
import java.util.TreeMap;
import java.util.Locale;

/** Computes domain-separated SHA-256 schema hashes matching the Rust implementation. */
public final class SchemaHash {
  private static final byte[] TYPE_NAME_DOMAIN =
      "norito:v1:type-name\0".getBytes(StandardCharsets.UTF_8);
  private static final byte[] STRUCTURAL_DOMAIN =
      "norito:v1:structural-schema\0".getBytes(StandardCharsets.UTF_8);

  private SchemaHash() {}

  public static byte[] hash16(String canonicalPath) {
    return hash16(TYPE_NAME_DOMAIN, canonicalPath.getBytes(StandardCharsets.UTF_8));
  }

  public static byte[] hash16FromStructural(Object schema) {
    Objects.requireNonNull(schema, "schema");
    String canonical = encodeCanonicalJson(schema);
    return hash16(STRUCTURAL_DOMAIN, canonical.getBytes(StandardCharsets.UTF_8));
  }

  private static byte[] hash16(byte[] domain, byte[] input) {
    MessageDigest digest = sha256();
    digest.update(domain);
    digest.update(input);
    return Arrays.copyOf(digest.digest(), 16);
  }

  private static MessageDigest sha256() {
    try {
      return MessageDigest.getInstance("SHA-256");
    } catch (NoSuchAlgorithmException ex) {
      throw new IllegalStateException("SHA-256 digest is unavailable", ex);
    }
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
