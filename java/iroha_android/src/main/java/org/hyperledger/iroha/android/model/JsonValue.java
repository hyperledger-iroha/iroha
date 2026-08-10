package org.hyperledger.iroha.android.model;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonParser;

/** One canonical JSON value suitable for the signed Norito metadata wire. */
public final class JsonValue {
  private static final int MAX_JSON_BYTES = 1_048_576;
  private static final BigInteger MAX_U64 = new BigInteger("18446744073709551615");
  private static final BigInteger MIN_I64 = BigInteger.valueOf(Long.MIN_VALUE);
  private static final char[] HEX_DIGITS =
      new char[] {'0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f'};

  private final String canonicalJson;

  private JsonValue(final String canonicalJson) {
    this.canonicalJson = Objects.requireNonNull(canonicalJson, "canonicalJson");
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
    return parse(builder.toString());
  }

  /** Returns a JSON number literal for {@code value}. */
  public static JsonValue number(final long value) {
    return new JsonValue(Long.toString(value));
  }

  /** Returns a JSON boolean literal for {@code value}. */
  public static JsonValue bool(final boolean value) {
    return new JsonValue(value ? "true" : "false");
  }

  /** Returns the canonical JSON null value. */
  public static JsonValue nullValue() {
    return new JsonValue("null");
  }

  /** Parses a JSON document and discards every alternate lexical spelling. */
  public static JsonValue parse(final String json) {
    return new JsonValue(canonicalize(json));
  }

  /**
   * Accepts a decoded signed-wire value only when it already uses the canonical spelling.
   *
   * <p>Binary decoding must reject rather than silently rewrite signed bytes.
   */
  public static JsonValue fromCanonicalWire(final String json) {
    final String canonical = canonicalize(json);
    if (!canonical.equals(json)) {
      throw new IllegalArgumentException(
          "JsonValue wire payload is valid but not in canonical lexical form");
    }
    return new JsonValue(canonical);
  }

  /** Returns the canonical JSON text. */
  public String canonicalJson() {
    return canonicalJson;
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
    return canonicalJson.equals(that.canonicalJson);
  }

  @Override
  public int hashCode() {
    return canonicalJson.hashCode();
  }

  @Override
  public String toString() {
    return canonicalJson;
  }

  private static String canonicalize(final String json) {
    Objects.requireNonNull(json, "json");
    if (json.length() > MAX_JSON_BYTES
        || json.getBytes(StandardCharsets.UTF_8).length > MAX_JSON_BYTES) {
      throw new IllegalArgumentException(
          "JsonValue exceeds the " + MAX_JSON_BYTES + "-byte UTF-8 limit");
    }
    final Object parsed;
    try {
      parsed = JsonParser.parse(json);
    } catch (IllegalStateException error) {
      throw new IllegalArgumentException(
          "JsonValue must contain exactly one valid JSON value", error);
    }
    final StringBuilder builder = new StringBuilder(json.length());
    writeValue(builder, parsed);
    final String canonical = builder.toString();
    if (canonical.getBytes(StandardCharsets.UTF_8).length > MAX_JSON_BYTES) {
      throw new IllegalArgumentException(
          "canonical JsonValue exceeds the " + MAX_JSON_BYTES + "-byte UTF-8 limit");
    }
    return canonical;
  }

  private static void writeValue(final StringBuilder builder, final Object value) {
    if (value == null) {
      builder.append("null");
    } else if (value instanceof Boolean bool) {
      builder.append(bool.booleanValue() ? "true" : "false");
    } else if (value instanceof String text) {
      writeString(builder, text);
    } else if (value instanceof Long integer) {
      builder.append(integer.longValue());
    } else if (value instanceof BigInteger integer) {
      writeBigInteger(builder, integer);
    } else if (value instanceof BigDecimal decimal) {
      builder.append(formatFinite(decimal.doubleValue()));
    } else if (value instanceof Double number) {
      builder.append(formatFinite(number.doubleValue()));
    } else if (value instanceof Map<?, ?> map) {
      writeObject(builder, map);
    } else if (value instanceof List<?> list) {
      writeArray(builder, list);
    } else {
      throw new IllegalArgumentException(
          "unsupported parsed JSON value: " + value.getClass().getName());
    }
  }

  private static void writeBigInteger(
      final StringBuilder builder, final BigInteger value) {
    if ((value.signum() >= 0 && value.compareTo(MAX_U64) <= 0)
        || (value.signum() < 0 && value.compareTo(MIN_I64) >= 0)) {
      builder.append(value);
    } else {
      builder.append(formatFinite(value.doubleValue()));
    }
  }

  private static void writeObject(final StringBuilder builder, final Map<?, ?> value) {
    final List<String> keys = new ArrayList<>(value.size());
    for (final Object key : value.keySet()) {
      if (!(key instanceof String text)) {
        throw new IllegalArgumentException("JSON object keys must be strings");
      }
      keys.add(text);
    }
    keys.sort(JsonValue::compareUnicodeScalars);
    builder.append('{');
    for (int index = 0; index < keys.size(); index++) {
      if (index > 0) {
        builder.append(',');
      }
      final String key = keys.get(index);
      writeString(builder, key);
      builder.append(':');
      writeValue(builder, value.get(key));
    }
    builder.append('}');
  }

  private static void writeArray(final StringBuilder builder, final List<?> value) {
    builder.append('[');
    for (int index = 0; index < value.size(); index++) {
      if (index > 0) {
        builder.append(',');
      }
      writeValue(builder, value.get(index));
    }
    builder.append(']');
  }

  /** Formats a parsed JSON float with Norito's finite-f64 Ryu presentation rules. */
  private static String formatFinite(final double value) {
    if (!Double.isFinite(value)) {
      throw new IllegalArgumentException(
          "JSON floating-point number is outside the finite f64 range");
    }
    final boolean negative = Double.doubleToRawLongBits(value) < 0;
    final double magnitude = Math.abs(value);
    if (magnitude == 0.0d) {
      return negative ? "-0.0" : "0.0";
    }
    // Java's historical spelling for the least subnormal is not the shortest
    // Ryu spelling selected by Norito.
    if (magnitude == Double.MIN_VALUE) {
      return negative ? "-5e-324" : "5e-324";
    }

    final BigDecimal decimal = BigDecimal.valueOf(magnitude).stripTrailingZeros();
    final String digits = decimal.unscaledValue().abs().toString();
    final int exponent = -decimal.scale();
    final int decimalPoint = digits.length() + exponent;
    final StringBuilder body = new StringBuilder(24);
    if (exponent >= 0 && decimalPoint <= 16) {
      body.append(digits);
      appendZeros(body, decimalPoint - digits.length());
      body.append(".0");
    } else if (decimalPoint > 0 && decimalPoint <= 16) {
      body.append(digits, 0, decimalPoint);
      body.append('.');
      body.append(digits, decimalPoint, digits.length());
    } else if (decimalPoint > -5 && decimalPoint <= 0) {
      body.append("0.");
      appendZeros(body, -decimalPoint);
      body.append(digits);
    } else if (digits.length() == 1) {
      body.append(digits);
      appendExponent(body, decimalPoint - 1);
    } else {
      body.append(digits.charAt(0));
      body.append('.');
      body.append(digits, 1, digits.length());
      appendExponent(body, decimalPoint - 1);
    }
    return negative ? "-" + body : body.toString();
  }

  private static void appendZeros(final StringBuilder builder, final int count) {
    for (int index = 0; index < count; index++) {
      builder.append('0');
    }
  }

  private static void appendExponent(final StringBuilder builder, final int exponent) {
    builder.append('e');
    if (exponent >= 0) {
      builder.append('+');
    }
    builder.append(exponent);
  }

  private static int compareUnicodeScalars(final String left, final String right) {
    int leftIndex = 0;
    int rightIndex = 0;
    while (leftIndex < left.length() && rightIndex < right.length()) {
      final int leftScalar = Character.codePointAt(left, leftIndex);
      final int rightScalar = Character.codePointAt(right, rightIndex);
      if (leftScalar != rightScalar) {
        return Integer.compare(leftScalar, rightScalar);
      }
      leftIndex += Character.charCount(leftScalar);
      rightIndex += Character.charCount(rightScalar);
    }
    return Integer.compare(left.length() - leftIndex, right.length() - rightIndex);
  }

  private static void writeString(final StringBuilder builder, final String value) {
    builder.append('"');
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      switch (character) {
        case '"' -> builder.append("\\\"");
        case '\\' -> builder.append("\\\\");
        case '\b' -> builder.append("\\b");
        case '\f' -> builder.append("\\f");
        case '\n' -> builder.append("\\n");
        case '\r' -> builder.append("\\r");
        case '\t' -> builder.append("\\t");
        default -> {
          if (character < 0x20) {
            builder.append("\\u00");
            builder.append(HEX_DIGITS[(character >> 4) & 0xF]);
            builder.append(HEX_DIGITS[character & 0xF]);
          } else {
            builder.append(character);
          }
        }
      }
    }
    builder.append('"');
  }
}
