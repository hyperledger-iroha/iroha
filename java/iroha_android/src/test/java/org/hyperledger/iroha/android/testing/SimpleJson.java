package org.hyperledger.iroha.android.testing;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/**
 * Minimal JSON parser shared across Android test fixtures. Supports the subset required by the
 * compliance suites (objects, arrays, booleans, null, strings, and non-negative integers).
 */
public final class SimpleJson {

  private final String input;
  private int index;

  private SimpleJson(final String input) {
    this.input = Objects.requireNonNull(input, "input");
  }

  public static Object parse(final String json) {
    final SimpleJson parser = new SimpleJson(json);
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
    switch (c) {
      case '{':
        return parseObject();
      case '[':
        return parseArray();
      case '"':
        return parseString();
      case 't':
        consumeLiteral("true");
        return Boolean.TRUE;
      case 'f':
        consumeLiteral("false");
        return Boolean.FALSE;
      case 'n':
        consumeLiteral("null");
        return null;
      default:
        return parseNumber();
    }
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
          case '"':
            builder.append('"');
            break;
          case '\\':
            builder.append('\\');
            break;
          case '/':
            builder.append('/');
            break;
          case 'b':
            builder.append('\b');
            break;
          case 'f':
            builder.append('\f');
            break;
          case 'n':
            builder.append('\n');
            break;
          case 'r':
            builder.append('\r');
            break;
          case 't':
            builder.append('\t');
            break;
          case 'u':
            if (index + 4 > input.length()) {
              throw new IllegalStateException("Invalid unicode escape");
            }
            final String hex = input.substring(index, index + 4);
            index += 4;
            builder.append((char) Integer.parseInt(hex, 16));
            break;
          default:
            throw new IllegalStateException("Unsupported escape: \\" + esc);
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
    while (index < input.length() && Character.isDigit(input.charAt(index))) {
      index++;
    }
    final String token = input.substring(start, index);
    try {
      return Long.parseLong(token);
    } catch (final NumberFormatException ex) {
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
