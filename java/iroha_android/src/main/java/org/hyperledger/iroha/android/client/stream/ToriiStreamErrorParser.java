package org.hyperledger.iroha.android.client.stream;

import java.math.BigInteger;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;
import org.hyperledger.iroha.android.client.JsonParser;

final class ToriiStreamErrorParser {

  private static final Set<String> EXPECTED_KEYS =
      Collections.unmodifiableSet(
          new HashSet<>(
              Arrays.asList("code", "message", "dropped_messages", "replay_available")));
  private static final BigInteger MAX_U64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);

  private ToriiStreamErrorParser() {}

  static ToriiStreamException parse(final String rawData) {
    final Object parsed;
    try {
      parsed = JsonParser.parse(rawData);
    } catch (final IllegalStateException error) {
      throw new ToriiStreamProtocolException(
          "data must be a valid JSON object without duplicate keys", rawData, error);
    }
    if (!(parsed instanceof Map<?, ?> rawPayload)) {
      throw malformed("data must be a JSON object", rawData);
    }
    for (final Object key : rawPayload.keySet()) {
      if (!(key instanceof String)) {
        throw malformed("data must use string property names", rawData);
      }
    }
    @SuppressWarnings("unchecked")
    final Map<String, Object> payload = (Map<String, Object>) rawPayload;
    if (!payload.keySet().equals(EXPECTED_KEYS)) {
      throw malformed(
          "data must contain exactly code, message, dropped_messages, and replay_available",
          rawData);
    }

    final String code = exactText(payload.get("code"), "code", true, rawData);
    final String message = exactText(payload.get("message"), "message", false, rawData);
    final BigInteger droppedMessages =
        unsignedIntegerOrNull(payload.get("dropped_messages"), rawData);
    final Object replayValue = payload.get("replay_available");
    if (!(replayValue instanceof Boolean replayAvailable)) {
      throw malformed("replay_available must be a boolean", rawData);
    }
    return new ToriiStreamException(
        code, message, droppedMessages, replayAvailable, rawData);
  }

  private static String exactText(
      final Object value,
      final String property,
      final boolean token,
      final String rawData) {
    if (!(value instanceof String text)) {
      throw malformed(property + " must be a string", rawData);
    }
    boolean hasControl = false;
    boolean hasTokenWhitespace = false;
    for (int offset = 0; offset < text.length(); ) {
      final int codePoint = text.codePointAt(offset);
      hasControl |= Character.isISOControl(codePoint);
      hasTokenWhitespace |= token && isProtocolWhitespace(codePoint);
      offset += Character.charCount(codePoint);
    }
    if (text.isEmpty()
        || hasSurroundingWhitespace(text)
        || hasControl
        || hasTokenWhitespace
        || hasUnpairedSurrogate(text)) {
      final String shape = token ? "a non-empty exact token" : "non-empty exact text";
      throw malformed(property + " must be " + shape, rawData);
    }
    return text;
  }

  private static boolean hasUnpairedSurrogate(final String text) {
    for (int index = 0; index < text.length(); index++) {
      final char current = text.charAt(index);
      if (Character.isHighSurrogate(current)) {
        if (index + 1 >= text.length() || !Character.isLowSurrogate(text.charAt(index + 1))) {
          return true;
        }
        index++;
      } else if (Character.isLowSurrogate(current)) {
        return true;
      }
    }
    return false;
  }

  private static boolean hasSurroundingWhitespace(final String text) {
    if (text.isEmpty()) {
      return false;
    }
    return isProtocolWhitespace(text.codePointAt(0))
        || isProtocolWhitespace(text.codePointBefore(text.length()));
  }

  private static boolean isProtocolWhitespace(final int codePoint) {
    return Character.isWhitespace(codePoint) || Character.isSpaceChar(codePoint);
  }

  private static BigInteger unsignedIntegerOrNull(
      final Object value, final String rawData) {
    if (value == null) {
      return null;
    }
    final BigInteger integer;
    if (value instanceof BigInteger bigInteger) {
      integer = bigInteger;
    } else if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      integer = BigInteger.valueOf(((Number) value).longValue());
    } else {
      throw malformed("dropped_messages must be null or an unsigned integer", rawData);
    }
    if (integer.signum() < 0 || integer.compareTo(MAX_U64) > 0) {
      throw malformed("dropped_messages must be null or an unsigned 64-bit integer", rawData);
    }
    return integer;
  }

  private static ToriiStreamProtocolException malformed(
      final String reason, final String rawData) {
    return new ToriiStreamProtocolException(reason, rawData);
  }
}
