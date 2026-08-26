package org.hyperledger.iroha.android.client;

import java.math.BigDecimal;
import java.math.BigInteger;

/** Checked integer coercions for values emitted by {@link JsonParser}. */
public final class JsonNumbers {
  private static final BigInteger LONG_MIN = BigInteger.valueOf(Long.MIN_VALUE);
  private static final BigInteger LONG_MAX = BigInteger.valueOf(Long.MAX_VALUE);
  private static final double LONG_MIN_DOUBLE = -9223372036854775808.0;
  private static final double LONG_MAX_EXCLUSIVE_DOUBLE = 9223372036854775808.0;

  private JsonNumbers() {}

  /** Require a lexical JSON integer that fits in a signed 64-bit value. */
  public static long asLong(final Object value, final String path) {
    return asLong(value, path, false);
  }

  /** Allow a mathematically integral decimal token and require signed 64-bit range. */
  public static long asLongAllowingIntegralFloat(final Object value, final String path) {
    return asLong(value, path, true);
  }

  /** Require a lexical JSON integer that fits in a signed 32-bit value. */
  public static int asInt(final Object value, final String path) {
    final long parsed = asLong(value, path);
    if (parsed < Integer.MIN_VALUE || parsed > Integer.MAX_VALUE) {
      throw new IllegalStateException(path + " must fit in signed 32-bit range");
    }
    return (int) parsed;
  }

  /** Require a lexical JSON integer without narrowing its mathematical value. */
  public static BigInteger asBigInteger(final Object value, final String path) {
    if (value instanceof BigInteger integer) {
      return integer;
    }
    if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      return BigInteger.valueOf(((Number) value).longValue());
    }
    throw new IllegalStateException(path + " must be an integer");
  }

  private static long asLong(
      final Object value, final String path, final boolean allowFloatingPoint) {
    if (!(value instanceof Number number)) {
      throw new IllegalStateException(path + " must be a number");
    }
    if (number instanceof BigInteger integer) {
      return checkedLong(integer, path);
    }
    if (number instanceof BigDecimal decimal) {
      if (!allowFloatingPoint) {
        throw new IllegalStateException(path + " must be an integer");
      }
      try {
        return checkedLong(decimal.toBigIntegerExact(), path);
      } catch (final ArithmeticException error) {
        throw new IllegalStateException(path + " must be an integer", error);
      }
    }
    if (number instanceof Double || number instanceof Float) {
      if (!allowFloatingPoint) {
        throw new IllegalStateException(path + " must be an integer");
      }
      final double numeric = number.doubleValue();
      if (!Double.isFinite(numeric) || numeric % 1.0 != 0.0) {
        throw new IllegalStateException(path + " must be an integer");
      }
      if (numeric < LONG_MIN_DOUBLE || numeric >= LONG_MAX_EXCLUSIVE_DOUBLE) {
        throw new IllegalStateException(path + " must fit in signed 64-bit range");
      }
      return (long) numeric;
    }
    return number.longValue();
  }

  private static long checkedLong(final BigInteger value, final String path) {
    if (value.compareTo(LONG_MIN) < 0 || value.compareTo(LONG_MAX) > 0) {
      throw new IllegalStateException(path + " must fit in signed 64-bit range");
    }
    return value.longValue();
  }
}
