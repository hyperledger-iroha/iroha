package org.hyperledger.iroha.android.model;

import java.util.Objects;

/** Raw JSON literal wrapper for transaction metadata values. */
public final class JsonValue {
  private static final char[] HEX_DIGITS =
      new char[] {'0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f'};

  private final String rawJson;

  private JsonValue(final String rawJson) {
    this.rawJson = Objects.requireNonNull(rawJson, "rawJson");
  }

  /** Returns a JSON string literal containing {@code value}. */
  public static JsonValue string(final String value) {
    Objects.requireNonNull(value, "value");
    final StringBuilder builder = new StringBuilder(value.length() + 2);
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
          if (c < ' ') {
            builder.append("\\u00");
            builder.append(HEX_DIGITS[(c >> 4) & 0xF]);
            builder.append(HEX_DIGITS[c & 0xF]);
          } else {
            builder.append(c);
          }
        }
      }
    }
    builder.append('"');
    return new JsonValue(builder.toString());
  }

  /** Returns a JSON number literal for {@code value}. */
  public static JsonValue number(final long value) {
    return new JsonValue(Long.toString(value));
  }

  /** Returns a JSON boolean literal for {@code value}. */
  public static JsonValue bool(final boolean value) {
    return new JsonValue(value ? "true" : "false");
  }

  /** Returns an unchecked raw JSON literal. */
  public static JsonValue raw(final String json) {
    return new JsonValue(json);
  }

  /** Returns the raw JSON literal. */
  public String rawJson() {
    return rawJson;
  }

  @Override
  public boolean equals(final Object other) {
    if (this == other) {
      return true;
    }
    if (!(other instanceof JsonValue)) {
      return false;
    }
    final JsonValue that = (JsonValue) other;
    return rawJson.equals(that.rawJson);
  }

  @Override
  public int hashCode() {
    return rawJson.hashCode();
  }

  @Override
  public String toString() {
    return rawJson;
  }
}
