package org.hyperledger.iroha.android.client;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Minimal JSON parser sufficient for the SDK polling helpers.
 *
 * <p>Only the subset required by pipeline status payloads is implemented (objects, arrays, strings,
 * booleans, null, integers).
 */
public final class JsonParser {

  private final String input;
  private int index;

  private JsonParser(final String input) {
    this.input = input;
  }

  public static Object parse(final String json) {
    final JsonParser parser = new JsonParser(json);
    parser.skipWhitespace();
    final Object value = parser.parseValue();
    parser.skipWhitespace();
    if (parser.index != parser.input.length()) {
      throw new IllegalStateException("Trailing characters after JSON payload");
    }
    return value;
  }

  private Object parseValue() {
    skipWhitespace();
    if (index >= input.length()) {
      throw new IllegalStateException("Unexpected end of JSON input");
    }
    final char c = input.charAt(index);
    return switch (c) {
      case '{' -> parseObject();
      case '[' -> parseArray();
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

  private Map<String, Object> parseObject() {
    expect('{');
    skipWhitespace();
    final Map<String, Object> map = new LinkedHashMap<>();
    if (peek('}')) {
      index++;
      return map;
    }
    while (true) {
      final String key = parseString();
      skipWhitespace();
      expect(':');
      skipWhitespace();
      map.put(key, parseValue());
      skipWhitespace();
      if (peek('}')) {
        index++;
        return map;
      }
      expect(',');
      skipWhitespace();
    }
  }

  private List<Object> parseArray() {
    expect('[');
    skipWhitespace();
    final List<Object> list = new ArrayList<>();
    if (peek(']')) {
      index++;
      return list;
    }
    while (true) {
      list.add(parseValue());
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
            if (index + 4 > input.length()) {
              throw new IllegalStateException("Invalid unicode escape");
            }
            final String hex = input.substring(index, index + 4);
            index += 4;
            builder.append((char) Integer.parseInt(hex, 16));
          }
          default -> throw new IllegalStateException("Unsupported escape: \\" + esc);
        }
      } else {
        builder.append(c);
      }
    }
    throw new IllegalStateException("Unterminated string literal");
  }

  private Number parseNumber() {
    final int start = index;
    if (index < input.length() && input.charAt(index) == '-') {
      index++;
    }
    boolean hasDigits = false;
    while (index < input.length() && Character.isDigit(input.charAt(index))) {
      index++;
      hasDigits = true;
    }
    if (!hasDigits) {
      throw new IllegalStateException("Invalid number: expected digit");
    }
    boolean hasFraction = false;
    if (index < input.length() && input.charAt(index) == '.') {
      hasFraction = true;
      index++;
      if (index >= input.length() || !Character.isDigit(input.charAt(index))) {
        throw new IllegalStateException("Invalid number: missing digit after decimal point");
      }
      while (index < input.length() && Character.isDigit(input.charAt(index))) {
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
        if (index >= input.length() || !Character.isDigit(input.charAt(index))) {
          throw new IllegalStateException("Invalid number: missing exponent digits");
        }
        while (index < input.length() && Character.isDigit(input.charAt(index))) {
          index++;
        }
      }
    }
    final String token = input.substring(start, index);
    try {
      if (!hasFraction && !hasExponent) {
        return Long.parseLong(token);
      }
      return Double.parseDouble(token);
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
}
