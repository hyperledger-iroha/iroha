package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;

/** Helpers for extracting stable HTTP error details from Torii responses. */
final class HttpErrorMessageExtractor {

  private static final int MAX_MESSAGE_LENGTH = 512;

  private HttpErrorMessageExtractor() {}

  static String extractRejectCode(
      final Map<String, List<String>> headers, final String headerName) {
    if (headers == null || headers.isEmpty() || headerName == null || headerName.isBlank()) {
      return null;
    }

    for (final Map.Entry<String, List<String>> entry : headers.entrySet()) {
      final String key = entry.getKey();
      if (key == null || !key.equalsIgnoreCase(headerName)) {
        continue;
      }
      final String value = firstNonBlank(entry.getValue());
      if (value != null) {
        return value;
      }
    }
    return null;
  }

  static String extractMessage(final byte[] body) {
    if (body == null || body.length == 0) {
      return null;
    }
    final String text = new String(body, StandardCharsets.UTF_8).trim();
    if (text.isEmpty()) {
      return null;
    }

    try {
      final Object parsed = JsonParser.parse(text);
      final String extracted = extractStructuredMessage(parsed);
      if (extracted != null) {
        return truncate(extracted);
      }
      final String compact = compactJsonSorted(parsed);
      if (compact != null) {
        return truncate(compact);
      }
    } catch (final RuntimeException ignored) {
      // Fall through to plain-text fallback.
    }

    return truncate(text);
  }

  private static String extractStructuredMessage(final Object value) {
    if (value instanceof String) {
      final String text = ((String) value).trim();
      return text.isEmpty() ? null : text;
    }
    if (value instanceof List<?>) {
      for (final Object entry : (List<?>) value) {
        final String nested = extractStructuredMessage(entry);
        if (nested != null) {
          return nested;
        }
      }
      return null;
    }
    if (!(value instanceof Map<?, ?>)) {
      return null;
    }
    final Map<?, ?> map = (Map<?, ?>) value;
    final String[] candidateKeys = {
      "message",
      "error",
      "errors",
      "detail",
      "details",
      "reason",
      "rejection_reason",
      "description"
    };
    for (final String key : candidateKeys) {
      final Object nestedValue = getCaseInsensitiveValue(map, key);
      if (nestedValue == null) {
        continue;
      }
      final String nested = extractStructuredMessage(nestedValue);
      if (nested != null) {
        return nested;
      }
    }
    return null;
  }

  private static Object getCaseInsensitiveValue(final Map<?, ?> map, final String candidateKey) {
    if (map.containsKey(candidateKey)) {
      return map.get(candidateKey);
    }
    for (final Map.Entry<?, ?> entry : map.entrySet()) {
      final Object rawKey = entry.getKey();
      if (rawKey instanceof String && ((String) rawKey).equalsIgnoreCase(candidateKey)) {
        return entry.getValue();
      }
    }
    return null;
  }

  private static String compactJsonSorted(final Object value) {
    final StringBuilder builder = new StringBuilder();
    appendJsonValueSorted(value, builder);
    final String text = builder.toString().trim();
    return text.isEmpty() ? null : text;
  }

  private static void appendJsonValueSorted(final Object value, final StringBuilder builder) {
    if (value == null) {
      builder.append("null");
      return;
    }
    if (value instanceof String) {
      appendJsonString((String) value, builder);
      return;
    }
    if (value instanceof Boolean || value instanceof Integer || value instanceof Long) {
      builder.append(value);
      return;
    }
    if (value instanceof Number) {
      final Number number = (Number) value;
      builder.append(number.toString());
      return;
    }
    if (value instanceof List<?>) {
      builder.append('[');
      boolean first = true;
      for (final Object entry : (List<?>) value) {
        if (!first) {
          builder.append(',');
        }
        first = false;
        appendJsonValueSorted(entry, builder);
      }
      builder.append(']');
      return;
    }
    if (value instanceof Map<?, ?>) {
      final Map<?, ?> map = (Map<?, ?>) value;
      final List<String> keys = new ArrayList<>();
      for (final Object rawKey : map.keySet()) {
        if (rawKey != null) {
          keys.add(String.valueOf(rawKey));
        }
      }
      Collections.sort(keys);
      builder.append('{');
      boolean first = true;
      for (final String key : keys) {
        if (!first) {
          builder.append(',');
        }
        first = false;
        appendJsonString(key, builder);
        builder.append(':');
        appendJsonValueSorted(map.get(key), builder);
      }
      builder.append('}');
      return;
    }
    appendJsonString(String.valueOf(value), builder);
  }

  private static void appendJsonString(final String text, final StringBuilder builder) {
    builder.append('"');
    for (int i = 0; i < text.length(); i++) {
      final char ch = text.charAt(i);
      switch (ch) {
        case '"' -> builder.append("\\\"");
        case '\\' -> builder.append("\\\\");
        case '\b' -> builder.append("\\b");
        case '\f' -> builder.append("\\f");
        case '\n' -> builder.append("\\n");
        case '\r' -> builder.append("\\r");
        case '\t' -> builder.append("\\t");
        default -> {
          if (ch < 0x20) {
            builder.append(String.format("\\u%04x", (int) ch));
          } else {
            builder.append(ch);
          }
        }
      }
    }
    builder.append('"');
  }

  private static String truncate(final String text) {
    final String normalized = text == null ? "" : text.trim();
    if (normalized.isEmpty()) {
      return null;
    }
    if (normalized.length() > MAX_MESSAGE_LENGTH) {
      return normalized.substring(0, MAX_MESSAGE_LENGTH) + "...";
    }
    return normalized;
  }

  private static String firstNonBlank(final List<String> values) {
    if (values == null || values.isEmpty()) {
      return null;
    }
    for (final String value : values) {
      if (value != null && !value.isBlank()) {
        return value.trim();
      }
    }
    return null;
  }
}
