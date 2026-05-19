package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Helpers for extracting stable HTTP error details from Torii responses. */
final class HttpErrorMessageExtractor {

  private static final int MAX_MESSAGE_LENGTH = 512;
  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<ErrorDetailsSummary> DETAILS_ADAPTER =
      new TypeAdapter<ErrorDetailsSummary>() {
        @Override
        public void encode(final NoritoEncoder encoder, final ErrorDetailsSummary value) {
          throw new UnsupportedOperationException("error detail encoding is not supported");
        }

        @Override
        public ErrorDetailsSummary decode(final NoritoDecoder decoder) {
          final String rejectCode = decodeRejectCodeField(decoder);
          if (decoder.remaining() > 0) {
            decoder.readBytes(decoder.remaining());
          }
          return new ErrorDetailsSummary(rejectCode);
        }
      };
  private static final TypeAdapter<Object> ERROR_ENVELOPE_ADAPTER =
      NoritoAdapters.struct(
          Arrays.asList(
              NoritoAdapters.field("code", STRING_ADAPTER),
              NoritoAdapters.field("message", STRING_ADAPTER),
              NoritoAdapters.field("details", NoritoAdapters.option(DETAILS_ADAPTER))),
          fields -> {
            final Optional<?> details = (Optional<?>) fields.get("details");
            final ErrorDetailsSummary summary =
                details.isPresent() ? (ErrorDetailsSummary) details.get() : null;
            return new ErrorEnvelopeSummary(
                (String) fields.get("code"),
                (String) fields.get("message"),
                summary == null ? null : summary.rejectCode);
          });

  private HttpErrorMessageExtractor() {}

  static String extractRejectCode(
      final Map<String, List<String>> headers, final String headerName) {
    if (headers == null || headers.isEmpty() || headerName == null || headerName.trim().isEmpty()) {
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

  static String extractRejectCode(
      final Map<String, List<String>> headers, final String headerName, final byte[] body) {
    final String fromHeader = extractRejectCode(headers, headerName);
    if (fromHeader != null) {
      return fromHeader;
    }
    if (body == null || body.length == 0) {
      return null;
    }
    final ErrorEnvelopeSummary norito = decodeNoritoErrorEnvelope(body);
    if (norito != null && norito.rejectCode != null) {
      return norito.rejectCode;
    }
    final String text = new String(body, StandardCharsets.UTF_8).trim();
    if (text.isEmpty()) {
      return null;
    }
    try {
      return extractStructuredRejectCode(JsonParser.parse(text));
    } catch (final RuntimeException ignored) {
      return null;
    }
  }

  static String extractMessage(final byte[] body) {
    if (body == null || body.length == 0) {
      return null;
    }
    final ErrorEnvelopeSummary norito = decodeNoritoErrorEnvelope(body);
    if (norito != null) {
      return truncate(norito.message);
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
      // Use the response body as plain text when it is not structured JSON.
    }

    return truncate(text);
  }

  private static ErrorEnvelopeSummary decodeNoritoErrorEnvelope(final byte[] body) {
    if (!hasNoritoMagic(body)) {
      return null;
    }
    try {
      return (ErrorEnvelopeSummary) NoritoCodec.decode(body, ERROR_ENVELOPE_ADAPTER, null);
    } catch (final RuntimeException ignored) {
      return null;
    }
  }

  private static boolean hasNoritoMagic(final byte[] body) {
    return body.length >= 4 && body[0] == 'N' && body[1] == 'R' && body[2] == 'T' && body[3] == '0';
  }

  private static String decodeRejectCodeField(final NoritoDecoder decoder) {
    final TypeAdapter<Optional<String>> optionalString = NoritoAdapters.option(STRING_ADAPTER);
    if ((decoder.flags() & NoritoHeader.PACKED_STRUCT) != 0
        && (decoder.flags() & NoritoHeader.FIELD_BITSET) != 0) {
      final int fieldCount = 5;
      final byte[] bitsetData = decoder.readBytes((fieldCount + 7) / 8);
      int bitset = 0;
      for (int i = 0; i < bitsetData.length; i++) {
        bitset |= (bitsetData[i] & 0xFF) << (i * 8);
      }
      final List<Integer> encodedSizes = new ArrayList<>(fieldCount);
      for (int i = 0; i < fieldCount; i++) {
        if ((bitset & (1 << i)) != 0) {
          final long size = decoder.readVarint();
          if (size > Integer.MAX_VALUE) {
            throw new IllegalArgumentException("Packed field too large");
          }
          encodedSizes.add((int) size);
        } else {
          encodedSizes.add(null);
        }
      }
      final Integer firstSize = encodedSizes.get(0);
      if (firstSize != null) {
        final NoritoDecoder child =
            new NoritoDecoder(decoder.readBytes(firstSize), decoder.flags(), decoder.flagsHint());
        final Optional<String> value = optionalString.decode(child);
        if (child.remaining() != 0) {
          throw new IllegalArgumentException("Packed reject_code field did not consume all bytes");
        }
        return value.orElse(null);
      }
      return optionalString.decode(decoder).orElse(null);
    }
    return optionalString.decode(decoder).orElse(null);
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

  private static String extractStructuredRejectCode(final Object value) {
    if (value instanceof List<?>) {
      for (final Object entry : (List<?>) value) {
        final String nested = extractStructuredRejectCode(entry);
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
    for (final String key : new String[] {"reject_code", "rejectCode"}) {
      final String direct = coerceNonBlankString(getCaseInsensitiveValue(map, key));
      if (direct != null) {
        return direct;
      }
    }
    final Object details = getCaseInsensitiveValue(map, "details");
    if (details instanceof Map<?, ?>) {
      final Map<?, ?> detailsMap = (Map<?, ?>) details;
      for (final String key : new String[] {"reject_code", "rejectCode"}) {
        final String nested = coerceNonBlankString(getCaseInsensitiveValue(detailsMap, key));
        if (nested != null) {
          return nested;
        }
      }
      final Object axt = getCaseInsensitiveValue(detailsMap, "axt");
      if (axt instanceof Map<?, ?>) {
        final Map<?, ?> axtMap = (Map<?, ?>) axt;
        final String axtCode = coerceNonBlankString(getCaseInsensitiveValue(axtMap, "code"));
        if (axtCode != null) {
          return axtCode;
        }
      }
    }
    return null;
  }

  private static String coerceNonBlankString(final Object value) {
    if (value == null) {
      return null;
    }
    final String text = String.valueOf(value).trim();
    return text.isEmpty() ? null : text;
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
      if (value != null && !value.trim().isEmpty()) {
        return value.trim();
      }
    }
    return null;
  }

  private static final class ErrorEnvelopeSummary {
    private final String code;
    private final String message;
    private final String rejectCode;

    private ErrorEnvelopeSummary(
        final String code, final String message, final String rejectCode) {
      this.code = code;
      this.message = message;
      this.rejectCode = rejectCode;
    }
  }

  private static final class ErrorDetailsSummary {
    private final String rejectCode;

    private ErrorDetailsSummary(final String rejectCode) {
      this.rejectCode = rejectCode;
    }
  }
}
