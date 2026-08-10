// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.consensus;

import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.regex.Pattern;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.util.HashLiteral;

/** Shared fail-closed JSON rules for the public Sumeragi status surfaces. */
final class SumeragiJsonSupport {
  private static final BigInteger U64_MAX =
      BigInteger.ONE.shiftLeft(Long.SIZE).subtract(BigInteger.ONE);
  private static final BigInteger U32_MAX =
      BigInteger.ONE.shiftLeft(Integer.SIZE).subtract(BigInteger.ONE);
  private static final Pattern CANONICAL_HASH =
      Pattern.compile("^hash:[0-9A-F]{64}#[0-9A-F]{4}$");
  private static final Pattern CANONICAL_BYTE_32 = Pattern.compile("^[0-9A-F]{64}$");

  private SumeragiJsonSupport() {}

  static String decodeUtf8(final byte[] payload, final String context) {
    if (payload == null) {
      throw new IllegalArgumentException(context + " payload must not be null");
    }
    try {
      return StandardCharsets.UTF_8
          .newDecoder()
          .onMalformedInput(CodingErrorAction.REPORT)
          .onUnmappableCharacter(CodingErrorAction.REPORT)
          .decode(ByteBuffer.wrap(payload))
          .toString();
    } catch (final Exception error) {
      throw new IllegalArgumentException(context + " must be valid UTF-8", error);
    }
  }

  static Map<String, Object> parseObject(final String payload, final String context) {
    if (payload == null) {
      throw new IllegalArgumentException(context + " payload must not be null");
    }
    rejectNegativeZeroTokens(payload, context);
    return object(JsonParser.parse(payload), context);
  }

  @SuppressWarnings("unchecked")
  static Map<String, Object> object(final Object value, final String context) {
    require(value instanceof Map<?, ?>, context + " must be a JSON object");
    final Map<?, ?> raw = (Map<?, ?>) value;
    for (final Object key : raw.keySet()) {
      require(key instanceof String, context + " contains a non-string field name");
    }
    return (Map<String, Object>) raw;
  }

  static Map<String, Object> exactObject(
      final Object value, final Set<String> fields, final String context) {
    final Map<String, Object> record = object(value, context);
    requireFields(record, fields, Collections.emptySet(), context);
    return record;
  }

  static void requireFields(
      final Map<String, Object> record,
      final Set<String> required,
      final Set<String> optional,
      final String context) {
    for (final String field : record.keySet()) {
      require(
          required.contains(field) || optional.contains(field),
          context + " contains unknown field " + field);
    }
    for (final String field : required) {
      require(record.containsKey(field), context + " is missing required field " + field);
    }
  }

  static List<?> array(final Object value, final String context, final int maximum) {
    require(value instanceof List<?>, context + " must be a JSON array");
    final List<?> list = (List<?>) value;
    require(list.size() <= maximum, context + " exceeds its protocol item bound");
    return list;
  }

  static BigInteger unsigned(
      final Object value,
      final BigInteger maximum,
      final String context,
      final boolean positive) {
    final BigInteger parsed;
    if (value instanceof BigInteger) {
      parsed = (BigInteger) value;
    } else if (value instanceof Long
        || value instanceof Integer
        || value instanceof Short
        || value instanceof Byte) {
      parsed = BigInteger.valueOf(((Number) value).longValue());
    } else {
      throw new IllegalArgumentException(context + " must be an unquoted integer");
    }
    require(parsed.signum() >= 0, context + " must be non-negative");
    require(!positive || parsed.signum() > 0, context + " must be positive");
    require(parsed.compareTo(maximum) <= 0, context + " exceeds its protocol bound");
    return parsed;
  }

  static BigInteger unsigned(
      final Object value, final BigInteger maximum, final String context) {
    return unsigned(value, maximum, context, false);
  }

  static BigInteger u64(final Object value, final String context) {
    return unsigned(value, U64_MAX, context);
  }

  static BigInteger positiveU64(final Object value, final String context) {
    return unsigned(value, U64_MAX, context, true);
  }

  static BigInteger u32(final Object value, final String context) {
    return unsigned(value, U32_MAX, context);
  }

  static BigInteger positiveU32(final Object value, final String context) {
    return unsigned(value, U32_MAX, context, true);
  }

  static int u16(final Object value, final String context) {
    return unsigned(value, BigInteger.valueOf(0xffffL), context).intValueExact();
  }

  static boolean bool(final Object value, final String context) {
    require(value instanceof Boolean, context + " must be a boolean");
    return ((Boolean) value).booleanValue();
  }

  static String string(final Object value, final String context) {
    require(value instanceof String, context + " must be a string");
    return (String) value;
  }

  static String exactNonemptyString(final Object value, final String context) {
    final String parsed = string(value, context);
    require(!parsed.isEmpty() && parsed.trim().equals(parsed), context + " must be exact and non-empty");
    return parsed;
  }

  static String hash(final Object value, final String context) {
    return hash(value, context, false);
  }

  static String nonzeroHash(final Object value, final String context) {
    return hash(value, context, true);
  }

  private static String hash(
      final Object value, final String context, final boolean requireNonzero) {
    require(
        value instanceof String && CANONICAL_HASH.matcher((String) value).matches(),
        context + " must be a canonical Iroha hash literal");
    final byte[] bytes;
    try {
      bytes = HashLiteral.decode((String) value);
    } catch (final IllegalArgumentException error) {
      throw new IllegalArgumentException(context + " must have a valid canonical checksum", error);
    }
    require(
        (bytes[bytes.length - 1] & 1) == 1,
        context + " has an invalid Iroha hash marker bit");
    if (requireNonzero) {
      boolean nonzero = false;
      for (final byte item : bytes) {
        nonzero |= item != 0;
      }
      require(nonzero, context + " must not be the zero hash");
    }
    return (String) value;
  }

  static String byte32(final Object value, final String context) {
    require(
        value instanceof String && CANONICAL_BYTE_32.matcher((String) value).matches(),
        context + " must be canonical uppercase 32-byte hex");
    return (String) value;
  }

  static String taggedUnit(final Object value, final String tag, final String context) {
    final Map<String, Object> record =
        exactObject(value, Set.of(tag, "details"), context);
    final String variant = string(record.get(tag), context + "." + tag);
    require(!variant.isEmpty(), context + "." + tag + " must not be empty");
    require(record.get("details") == null, context + ".details must be explicitly null");
    return variant;
  }

  static Object deepFreeze(final Object value, final String context) {
    if (value == null
        || value instanceof String
        || value instanceof Number
        || value instanceof Boolean) {
      return value;
    }
    if (value instanceof Map<?, ?>) {
      final Map<String, Object> record = object(value, context);
      final LinkedHashMap<String, Object> copy = new LinkedHashMap<>();
      for (final Map.Entry<String, Object> entry : record.entrySet()) {
        copy.put(
            entry.getKey(),
            deepFreeze(entry.getValue(), context + "." + entry.getKey()));
      }
      return Collections.unmodifiableMap(copy);
    }
    if (value instanceof List<?>) {
      final ArrayList<Object> copy = new ArrayList<>();
      final List<?> list = (List<?>) value;
      for (int index = 0; index < list.size(); index++) {
        copy.add(deepFreeze(list.get(index), context + "[" + index + "]"));
      }
      return Collections.unmodifiableList(copy);
    }
    throw new IllegalArgumentException(context + " contains an unsupported JSON value");
  }

  static void require(final boolean condition, final String message) {
    if (!condition) {
      throw new IllegalArgumentException(message);
    }
  }

  private static void rejectNegativeZeroTokens(final String payload, final String context) {
    boolean inString = false;
    boolean escaped = false;
    for (int index = 0; index < payload.length(); index++) {
      final char current = payload.charAt(index);
      if (inString) {
        if (escaped) {
          escaped = false;
        } else if (current == '\\') {
          escaped = true;
        } else if (current == '"') {
          inString = false;
        }
        continue;
      }
      if (current == '"') {
        inString = true;
        continue;
      }
      if (current == '-' && index + 1 < payload.length() && payload.charAt(index + 1) == '0') {
        final int afterIndex = index + 2;
        if (afterIndex == payload.length()
            || " \t\r\n,]}".indexOf(payload.charAt(afterIndex)) >= 0) {
          throw new IllegalArgumentException(context + " contains noncanonical negative zero");
        }
      }
    }
  }
}
