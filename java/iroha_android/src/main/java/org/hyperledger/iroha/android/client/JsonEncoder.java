package org.hyperledger.iroha.android.client;

import java.util.List;
import java.util.Map;
import java.util.Objects;

/** Minimal JSON encoder mirroring the structures parsed by {@link JsonParser}. */
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
      boolean first = true;
      for (final Map.Entry<String, Object> entry : ((Map<String, Object>) map).entrySet()) {
        if (!first) {
          builder.append(',');
        } else {
          first = false;
        }
        writeString(builder, Objects.requireNonNull(entry.getKey(), "json key"));
        builder.append(':');
        write(builder, entry.getValue());
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
