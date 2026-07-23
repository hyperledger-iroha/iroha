package org.hyperledger.iroha.android.client;

import java.math.BigInteger;

/** Exact unsigned-64 validation shared by typed alias read models. */
final class AccountAliasUInt64 {
  private static final BigInteger MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);

  private AccountAliasUInt64() {}

  static BigInteger require(final BigInteger value, final String path) {
    if (value == null || value.signum() < 0 || value.compareTo(MAX) > 0) {
      throw new IllegalArgumentException(path + " must fit in unsigned 64-bit range");
    }
    return value;
  }

  static BigInteger parse(final Object value, final String path) {
    final BigInteger integer;
    if (value instanceof BigInteger) {
      integer = (BigInteger) value;
    } else if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      integer = BigInteger.valueOf(((Number) value).longValue());
    } else {
      throw new IllegalStateException(path + " must be an integer");
    }
    if (integer.signum() < 0 || integer.compareTo(MAX) > 0) {
      throw new IllegalStateException(path + " must fit in unsigned 64-bit range");
    }
    return integer;
  }
}
