package org.hyperledger.iroha.android.client;

import java.util.List;
import java.util.Map;
import java.util.Objects;

/** Minimal JSON encoder mirroring the structures parsed by {@link JsonParser} (sorted keys). */
public final class JsonEncoder {

  private JsonEncoder() {}

  public static String encode(final Object value) {
    final StringBuilder builder = new StringBuilder();
    write(builder, value);
    return builder.toString();
  }

  @SuppressWarnings("unchecked")
  private static void write(final StringBuilder builder, final Object value) {
    if (value == null) {
      builder.append("null");
      return;
    }
    if (value instanceof String string) {
      writeString(builder, string);
      return;
    }
    if (value instanceof Number number) {
      builder.append(number.toString());
      return;
    }
    if (value instanceof Boolean bool) {
      builder.append(bool.booleanValue() ? "true" : "false");
      return;
    }
    if (value instanceof Map<?, ?> map) {
      builder.append('{');
      final List<String> keys = new java.util.ArrayList<>(map.size());
      for (final Object key : map.keySet()) {
        if (!(key instanceof String stringKey)) {
          throw new IllegalStateException("JSON object keys must be strings");
        }
        keys.add(stringKey);
      }
      keys.sort(String::compareTo);
      boolean first = true;
      for (final String key : keys) {
        if (!first) {
          builder.append(',');
        } else {
          first = false;
        }
        writeString(builder, Objects.requireNonNull(key, "json key"));
        builder.append(':');
        write(builder, ((Map<String, Object>) map).get(key));
      }
      builder.append('}');
      return;
    }
    if (value instanceof List<?> list) {
      builder.append('[');
      for (int i = 0; i < list.size(); i++) {
        if (i > 0) {
          builder.append(',');
        }
        write(builder, list.get(i));
      }
      builder.append(']');
      return;
    }
    throw new IllegalStateException("Unsupported JSON value: " + value.getClass());
  }

  private static void writeString(final StringBuilder builder, final String value) {
    builder.append('"');
    for (int i = 0; i < value.length(); i++) {
      final char c = value.charAt(i);
      switch (c) {
        case '"' -> builder.append("\\\"");
        case '\\' -> builder.append("\\\\");
        case '\b' -> builder.append("\\b");
        case '\f' -> builder.append("\\f");
        case '\n' -> builder.append("\\n");
        case '\r' -> builder.append("\\r");
        case '\t' -> builder.append("\\t");
        default -> {
          if (c < 0x20) {
            builder.append(String.format("\\u%04x", (int) c));
          } else {
            builder.append(c);
          }
        }
      }
    }
    builder.append('"');
  }
}
