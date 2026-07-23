// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.util.Arrays;
import java.util.Objects;

/** Strict, selector-free NIST P-256 codec for the sole Kagemusha device authority. */
public final class KagemushaP256Codec {

  public static final int SCALAR_BYTES = 32;
  public static final int PUBLIC_KEY_BYTES = 65;
  public static final int RAW_SIGNATURE_BYTES = 64;

  private static final BigInteger FIELD_PRIME =
      new BigInteger("FFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFF", 16);
  private static final BigInteger CURVE_B =
      new BigInteger("5AC635D8AA3A93E7B3EBBD55769886BC651D06B0CC53B0F63BCE3C3E27D2604B", 16);
  private static final BigInteger ORDER =
      new BigInteger("FFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551", 16);
  private static final BigInteger HALF_ORDER = ORDER.shiftRight(1);
  private static final BigInteger TWO = BigInteger.valueOf(2L);
  private static final BigInteger THREE = BigInteger.valueOf(3L);

  private KagemushaP256Codec() {}

  /** Validate and defensively copy exact uncompressed {@code 04 || x || y} SEC1 bytes. */
  public static byte[] requireUncompressedPublicKey(final byte[] sec1Bytes) {
    final byte[] value = Objects.requireNonNull(sec1Bytes, "sec1Bytes").clone();
    if (value.length != PUBLIC_KEY_BYTES || value[0] != 0x04) {
      throw new IllegalArgumentException(
          "Kagemusha device public key must be exactly 65-byte uncompressed P-256 SEC1");
    }
    final BigInteger x = new BigInteger(1, Arrays.copyOfRange(value, 1, 33));
    final BigInteger y = new BigInteger(1, Arrays.copyOfRange(value, 33, 65));
    if (x.compareTo(FIELD_PRIME) >= 0 || y.compareTo(FIELD_PRIME) >= 0) {
      throw new IllegalArgumentException(
          "Kagemusha device public key coordinates exceed the P-256 field");
    }
    final BigInteger lhs = y.modPow(TWO, FIELD_PRIME);
    final BigInteger rhs =
        x.modPow(THREE, FIELD_PRIME)
            .subtract(THREE.multiply(x))
            .add(CURVE_B)
            .mod(FIELD_PRIME);
    if (!lhs.equals(rhs)) {
      throw new IllegalArgumentException("Kagemusha device public key is not a P-256 point");
    }
    return value;
  }

  /** Validate and defensively copy the exact 64-byte raw low-S wire signature. */
  public static byte[] requireRawLowSSignature(final byte[] rawBytes) {
    final byte[] value = Objects.requireNonNull(rawBytes, "rawBytes").clone();
    if (value.length != RAW_SIGNATURE_BYTES) {
      throw new IllegalArgumentException(
          "Kagemusha device signature must be exactly 64-byte r||s");
    }
    final BigInteger r = new BigInteger(1, Arrays.copyOfRange(value, 0, SCALAR_BYTES));
    final BigInteger s =
        new BigInteger(1, Arrays.copyOfRange(value, SCALAR_BYTES, RAW_SIGNATURE_BYTES));
    requireScalar(r, "r");
    requireScalar(s, "s");
    if (s.compareTo(HALF_ORDER) > 0) {
      throw new IllegalArgumentException("Kagemusha device signature must use low-S form");
    }
    return value;
  }

  /**
   * Convert strict DER ECDSA to raw low-S form.
   *
   * <p>Valid high-S platform signatures are normalized to {@code n - s}. Non-minimal DER,
   * negative/zero/out-of-range scalars, long-form lengths, and trailing bytes are rejected.
   */
  public static byte[] rawLowSFromStrictDer(final byte[] derBytes) {
    final byte[] der = Objects.requireNonNull(derBytes, "derBytes").clone();
    if (der.length < 8 || der.length > 72 || unsigned(der[0]) != 0x30) {
      throw new IllegalArgumentException("Kagemusha ECDSA signature is not strict DER");
    }
    if (unsigned(der[1]) >= 0x80 || unsigned(der[1]) != der.length - 2) {
      throw new IllegalArgumentException(
          "Kagemusha ECDSA signature uses a non-canonical DER length");
    }
    final DecodedInteger r = decodeInteger(der, 2);
    final DecodedInteger s = decodeInteger(der, r.nextOffset);
    if (s.nextOffset != der.length) {
      throw new IllegalArgumentException("Kagemusha ECDSA signature has trailing DER bytes");
    }
    final BigInteger lowS = s.value.compareTo(HALF_ORDER) > 0
        ? ORDER.subtract(s.value) : s.value;
    final byte[] result = new byte[RAW_SIGNATURE_BYTES];
    copyFixedScalar(r.value, result, 0);
    copyFixedScalar(lowS, result, SCALAR_BYTES);
    return result;
  }

  /** Convert one canonical raw low-S signature to minimal DER. */
  public static byte[] strictDerFromRawLowS(final byte[] rawBytes) {
    final byte[] raw = requireRawLowSSignature(rawBytes);
    final byte[] r = encodeInteger(Arrays.copyOfRange(raw, 0, SCALAR_BYTES));
    final byte[] s = encodeInteger(Arrays.copyOfRange(raw, SCALAR_BYTES, RAW_SIGNATURE_BYTES));
    final int bodyLength = 2 + r.length + 2 + s.length;
    if (bodyLength >= 0x80) {
      throw new IllegalStateException("P-256 DER body unexpectedly requires long-form length");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream(bodyLength + 2);
    out.write(0x30);
    out.write(bodyLength);
    out.write(0x02);
    out.write(r.length);
    out.write(r, 0, r.length);
    out.write(0x02);
    out.write(s.length);
    out.write(s, 0, s.length);
    return out.toByteArray();
  }

  private static DecodedInteger decodeInteger(final byte[] bytes, final int start) {
    if (start + 2 > bytes.length || unsigned(bytes[start]) != 0x02) {
      throw new IllegalArgumentException(
          "Kagemusha ECDSA signature is missing a DER INTEGER");
    }
    final int length = unsigned(bytes[start + 1]);
    if (length < 1 || length > SCALAR_BYTES + 1 || start + 2 + length > bytes.length) {
      throw new IllegalArgumentException(
          "Kagemusha ECDSA signature has an invalid DER INTEGER length");
    }
    final byte[] encoded = Arrays.copyOfRange(bytes, start + 2, start + 2 + length);
    if (unsigned(encoded[0]) >= 0x80) {
      throw new IllegalArgumentException(
          "Kagemusha ECDSA signature contains a negative DER INTEGER");
    }
    if (encoded.length > 1 && encoded[0] == 0 && unsigned(encoded[1]) < 0x80) {
      throw new IllegalArgumentException(
          "Kagemusha ECDSA signature contains non-minimal DER INTEGER padding");
    }
    final byte[] scalar;
    if (encoded.length == SCALAR_BYTES + 1) {
      if (encoded[0] != 0 || unsigned(encoded[1]) < 0x80) {
        throw new IllegalArgumentException(
            "Kagemusha ECDSA signature DER INTEGER exceeds P-256 width");
      }
      scalar = Arrays.copyOfRange(encoded, 1, encoded.length);
    } else {
      scalar = encoded;
    }
    final BigInteger value = new BigInteger(1, scalar);
    requireScalar(value, "DER scalar");
    return new DecodedInteger(value, start + 2 + length);
  }

  private static void requireScalar(final BigInteger value, final String field) {
    if (value.signum() <= 0 || value.compareTo(ORDER) >= 0) {
      throw new IllegalArgumentException(
          "Kagemusha ECDSA " + field + " scalar is outside the P-256 order");
    }
  }

  private static void copyFixedScalar(
      final BigInteger value, final byte[] destination, final int offset) {
    final byte[] signed = value.toByteArray();
    final int sourceOffset = signed.length == SCALAR_BYTES + 1 && signed[0] == 0 ? 1 : 0;
    final int length = signed.length - sourceOffset;
    if (length > SCALAR_BYTES) {
      throw new IllegalStateException("P-256 scalar unexpectedly exceeds fixed width");
    }
    System.arraycopy(signed, sourceOffset, destination, offset + SCALAR_BYTES - length, length);
  }

  private static byte[] encodeInteger(final byte[] fixed) {
    int first = 0;
    while (first < fixed.length - 1 && fixed[first] == 0) {
      first++;
    }
    final byte[] significant = Arrays.copyOfRange(fixed, first, fixed.length);
    if (unsigned(significant[0]) < 0x80) {
      return significant;
    }
    final byte[] padded = new byte[significant.length + 1];
    System.arraycopy(significant, 0, padded, 1, significant.length);
    return padded;
  }

  private static int unsigned(final byte value) {
    return value & 0xff;
  }

  private static final class DecodedInteger {
    private final BigInteger value;
    private final int nextOffset;

    private DecodedInteger(final BigInteger value, final int nextOffset) {
      this.value = value;
      this.nextOffset = nextOffset;
    }
  }
}
