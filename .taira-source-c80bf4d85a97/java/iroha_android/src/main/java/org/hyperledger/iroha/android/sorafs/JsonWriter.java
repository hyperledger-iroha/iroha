package org.hyperledger.iroha.android.sorafs;

import java.util.Iterator;
import java.util.Map;

/** Minimal JSON writer for the SoraFS helpers. */
final class JsonWriter {
  private JsonWriter() {}

  static String encode(final Object value) {
    final StringBuilder builder = new StringBuilder();
    write(builder, value);
    return builder.toString();
  }

  static byte[] encodeBytes(final Object value) {
    return encode(value).getBytes(java.nio.charset.StandardCharsets.UTF_8);
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
      writeObject(builder, (Map<String, Object>) map);
      return;
    }
    if (value instanceof Iterable<?> iterable) {
      writeArray(builder, iterable.iterator());
      return;
    }
    if (value.getClass().isArray()) {
      // Only byte[] is expected at the moment.
      throw new IllegalArgumentException("array values are not supported: " + value.getClass());
    }
    throw new IllegalArgumentException("Unsupported JSON value: " + value);
  }

  private static void writeObject(
      final StringBuilder builder, final Map<String, Object> map) {
    builder.append('{');
    final Iterator<Map.Entry<String, Object>> iterator = map.entrySet().iterator();
    while (iterator.hasNext()) {
      final Map.Entry<String, Object> entry = iterator.next();
      writeString(builder, entry.getKey());
      builder.append(':');
      write(builder, entry.getValue());
      if (iterator.hasNext()) {
        builder.append(',');
      }
    }
    builder.append('}');
  }

  private static void writeArray(
      final StringBuilder builder, final Iterator<?> iterator) {
    builder.append('[');
    boolean first = true;
    while (iterator.hasNext()) {
      if (!first) {
        builder.append(',');
      } else {
        first = false;
      }
      write(builder, iterator.next());
    }
    builder.append(']');
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

