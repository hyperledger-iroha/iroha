package org.hyperledger.iroha.android.client;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Minimal JSON parser sufficient for the SDK polling helpers.
 *
 * <p>Only the subset required by SDK payloads is implemented (objects, arrays, strings, booleans,
 * null, numbers).
 */
public final class JsonParser {

  private static final int MAX_NESTING_DEPTH = 128;

  private final String input;
  private int index;

  private JsonParser(final String input) {
    this.input = input;
  }

  public static Object parse(final String json) {
    final JsonParser parser = new JsonParser(json);
    parser.skipWhitespace();
    final Object value = parser.parseValue(0);
    parser.skipWhitespace();
    if (parser.index != parser.input.length()) {
      throw new IllegalStateException("Trailing characters after JSON payload");
    }
    return value;
  }

  private Object parseValue(final int depth) {
    if (depth > MAX_NESTING_DEPTH) {
      throw new IllegalStateException("JSON exceeds maximum nesting depth");
    }
    skipWhitespace();
    if (index >= input.length()) {
      throw new IllegalStateException("Unexpected end of JSON input");
    }
    final char c = input.charAt(index);
    return switch (c) {
      case '{' -> parseObject(depth);
      case '[' -> parseArray(depth);
      case '"' -> parseString();
      case 't' -> {
        consumeLiteral("true");
        yield Boolean.TRUE;
      }
      case 'f' -> {
        consumeLiteral("false");
        yield Boolean.FALSE;
      }
      case 'n' -> {
        consumeLiteral("null");
        yield null;
      }
      default -> parseNumber();
    };
  }

  private Map<String, Object> parseObject(final int depth) {
    expect('{');
    skipWhitespace();
    final Map<String, Object> map = new LinkedHashMap<>();
    if (peek('}')) {
      index++;
      return map;
    }
    while (true) {
      final String key = parseString();
      if (map.containsKey(key)) {
        throw new IllegalStateException("Duplicate JSON object key: " + key);
      }
      skipWhitespace();
      expect(':');
      skipWhitespace();
      map.put(key, parseValue(depth + 1));
      skipWhitespace();
      if (peek('}')) {
        index++;
        return map;
      }
      expect(',');
      skipWhitespace();
    }
  }

  private List<Object> parseArray(final int depth) {
    expect('[');
    skipWhitespace();
    final List<Object> list = new ArrayList<>();
    if (peek(']')) {
      index++;
      return list;
    }
    while (true) {
      list.add(parseValue(depth + 1));
      skipWhitespace();
      if (peek(']')) {
        index++;
        return list;
      }
      expect(',');
      skipWhitespace();
    }
  }

  private String parseString() {
    expect('"');
    final StringBuilder builder = new StringBuilder();
    while (index < input.length()) {
      final char c = input.charAt(index++);
      if (c == '"') {
        return builder.toString();
      }
      if (c == '\\') {
        if (index >= input.length()) {
          throw new IllegalStateException("Invalid escape sequence");
        }
        final char esc = input.charAt(index++);
        switch (esc) {
          case '"' -> builder.append('"');
          case '\\' -> builder.append('\\');
          case '/' -> builder.append('/');
          case 'b' -> builder.append('\b');
          case 'f' -> builder.append('\f');
          case 'n' -> builder.append('\n');
          case 'r' -> builder.append('\r');
          case 't' -> builder.append('\t');
          case 'u' -> {
            final char high = parseUnicodeEscapeUnit();
            if (Character.isHighSurrogate(high)) {
              if (index + 2 > input.length()
                  || input.charAt(index) != '\\'
                  || input.charAt(index + 1) != 'u') {
                throw new IllegalStateException("Invalid unicode surrogate pair");
              }
              index += 2;
              final char low = parseUnicodeEscapeUnit();
              if (!Character.isLowSurrogate(low)) {
                throw new IllegalStateException("Invalid unicode surrogate pair");
              }
              builder.append(high).append(low);
            } else if (Character.isLowSurrogate(high)) {
              throw new IllegalStateException("Invalid unicode surrogate pair");
            } else {
              builder.append(high);
            }
          }
          default -> throw new IllegalStateException("Unsupported escape: \\" + esc);
        }
      } else {
        if (c < 0x20) {
          throw new IllegalStateException("Unescaped control character in JSON string");
        }
        if (Character.isHighSurrogate(c)) {
          if (index >= input.length() || !Character.isLowSurrogate(input.charAt(index))) {
            throw new IllegalStateException("Invalid unicode surrogate pair");
          }
          builder.append(c).append(input.charAt(index++));
        } else if (Character.isLowSurrogate(c)) {
          throw new IllegalStateException("Invalid unicode surrogate pair");
        } else {
          builder.append(c);
        }
      }
    }
    throw new IllegalStateException("Unterminated string literal");
  }

  private char parseUnicodeEscapeUnit() {
    if (index + 4 > input.length()) {
      throw new IllegalStateException("Invalid unicode escape");
    }
    final String hex = input.substring(index, index + 4);
    index += 4;
    try {
      return (char) Integer.parseInt(hex, 16);
    } catch (NumberFormatException error) {
      throw new IllegalStateException("Invalid unicode escape", error);
    }
  }

  private Number parseNumber() {
    final int start = index;
    if (index < input.length() && input.charAt(index) == '-') {
      index++;
    }
    if (index >= input.length()) {
      throw new IllegalStateException("Invalid number: expected digit");
    }
    boolean hasDigits = false;
    if (index < input.length() && isDigit(input.charAt(index))) {
      hasDigits = true;
      if (input.charAt(index) == '0') {
        index++;
        if (index < input.length() && isDigit(input.charAt(index))) {
          throw new IllegalStateException("Invalid number: leading zero");
        }
      } else {
        while (index < input.length() && isDigit(input.charAt(index))) {
          index++;
        }
      }
    }
    if (!hasDigits) {
      throw new IllegalStateException("Invalid number: expected digit");
    }
    boolean hasFraction = false;
    if (index < input.length() && input.charAt(index) == '.') {
      hasFraction = true;
      index++;
      if (index >= input.length() || !isDigit(input.charAt(index))) {
        throw new IllegalStateException("Invalid number: missing digit after decimal point");
      }
      while (index < input.length() && isDigit(input.charAt(index))) {
        index++;
      }
    }
    boolean hasExponent = false;
    if (index < input.length()) {
      final char exp = input.charAt(index);
      if (exp == 'e' || exp == 'E') {
        hasExponent = true;
        index++;
        if (index < input.length()) {
          final char sign = input.charAt(index);
          if (sign == '+' || sign == '-') {
            index++;
          }
        }
        if (index >= input.length() || !isDigit(input.charAt(index))) {
          throw new IllegalStateException("Invalid number: missing exponent digits");
        }
        while (index < input.length() && isDigit(input.charAt(index))) {
          index++;
        }
      }
    }
    final String token = input.substring(start, index);
    try {
      if (!hasFraction && !hasExponent) {
        try {
          return Long.parseLong(token);
        } catch (NumberFormatException ex) {
          return new BigInteger(token);
        }
      }
      return new BigDecimal(token);
    } catch (NumberFormatException ex) {
      throw new IllegalStateException("Invalid number: " + token, ex);
    }
  }

  private void consumeLiteral(final String literal) {
    if (!input.regionMatches(index, literal, 0, literal.length())) {
      throw new IllegalStateException("Expected literal '" + literal + "'");
    }
    index += literal.length();
  }

  private void skipWhitespace() {
    while (index < input.length()) {
      final char c = input.charAt(index);
      if (c == ' ' || c == '\t' || c == '\n' || c == '\r') {
        index++;
      } else {
        break;
      }
    }
  }

  private void expect(final char expected) {
    if (index >= input.length() || input.charAt(index) != expected) {
      throw new IllegalStateException("Expected '" + expected + "'");
    }
    index++;
  }

  private boolean peek(final char expected) {
    return index < input.length() && input.charAt(index) == expected;
  }

  private static boolean isDigit(final char c) {
    return c >= '0' && c <= '9';
  }
}
