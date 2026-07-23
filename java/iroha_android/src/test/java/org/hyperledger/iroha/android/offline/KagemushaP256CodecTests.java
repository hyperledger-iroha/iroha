// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.junit.Test;

public final class KagemushaP256CodecTests {
  private static final String P256_GENERATOR =
      "04"
          + "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
          + "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5";
  private static final BigInteger ORDER =
      new BigInteger("FFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551", 16);
  private static final BigInteger HALF_ORDER = ORDER.shiftRight(1);

  @Test
  public void uncompressedPublicKeyIsFixedValidAndDefensive() {
    final byte[] source = hex(P256_GENERATOR);
    final KagemushaDevicePublicKeyV2 key = new KagemushaDevicePublicKeyV2(source);
    Arrays.fill(source, (byte) 0);
    assertArrayEquals(hex(P256_GENERATOR), key.sec1Bytes());
    final byte[] returned = key.sec1Bytes();
    Arrays.fill(returned, (byte) 0);
    assertArrayEquals(hex(P256_GENERATOR), key.sec1Bytes());
    assertEquals(new KagemushaDevicePublicKeyV2(hex(P256_GENERATOR)), key);

    final List<byte[]> invalid = new ArrayList<>();
    invalid.add(Arrays.copyOf(hex(P256_GENERATOR), 64));
    invalid.add(Arrays.copyOf(hex(P256_GENERATOR), 66));
    invalid.add(mutate(hex(P256_GENERATOR), 0, (byte) 0x02));
    invalid.add(mutate(hex(P256_GENERATOR), 0, (byte) 0x03));
    final byte[] offCurve = hex(P256_GENERATOR);
    offCurve[offCurve.length - 1] ^= 1;
    invalid.add(offCurve);
    final byte[] infinity = new byte[65];
    infinity[0] = 0x04;
    invalid.add(infinity);
    for (final byte[] candidate : invalid) {
      assertThrows(IllegalArgumentException.class, () -> new KagemushaDevicePublicKeyV2(candidate));
    }
  }

  @Test
  public void rawSignatureRequiresExactInRangeLowSScalars() {
    final byte[] source = raw(BigInteger.ONE, BigInteger.ONE);
    final KagemushaDeviceSignatureV2 signature = new KagemushaDeviceSignatureV2(source);
    Arrays.fill(source, (byte) 0);
    assertArrayEquals(raw(BigInteger.ONE, BigInteger.ONE), signature.rawBytes());
    assertArrayEquals(
        new byte[] {0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01},
        signature.strictDer());

    for (final byte[] invalid : List.of(
        new byte[63],
        new byte[65],
        raw(BigInteger.ZERO, BigInteger.ONE),
        raw(BigInteger.ONE, BigInteger.ZERO),
        raw(ORDER, BigInteger.ONE),
        raw(BigInteger.ONE, ORDER),
        raw(BigInteger.ONE, HALF_ORDER.add(BigInteger.ONE)))) {
      assertThrows(IllegalArgumentException.class, () -> new KagemushaDeviceSignatureV2(invalid));
    }
  }

  @Test
  public void strictDerNormalizesHighSAndRoundTripsMinimally() {
    final byte[] low = raw(BigInteger.valueOf(128), BigInteger.ONE);
    final byte[] expectedDer =
        new byte[] {0x30, 0x07, 0x02, 0x02, 0x00, (byte) 0x80, 0x02, 0x01, 0x01};
    assertArrayEquals(expectedDer, KagemushaP256Codec.strictDerFromRawLowS(low));
    assertArrayEquals(low, KagemushaP256Codec.rawLowSFromStrictDer(expectedDer));

    final byte[] highDer = der(BigInteger.ONE, ORDER.subtract(BigInteger.ONE));
    final byte[] normalized = KagemushaP256Codec.rawLowSFromStrictDer(highDer);
    assertArrayEquals(raw(BigInteger.ONE, BigInteger.ONE), normalized);
    assertArrayEquals(normalized, KagemushaDeviceSignatureV2.fromStrictDer(highDer).rawBytes());
  }

  @Test
  public void strictDerRejectsAlternateMalformedAndOutOfRangeEncodings() {
    final byte[] valid = der(BigInteger.ONE, BigInteger.ONE);
    final List<byte[]> cases = List.of(
        new byte[0],
        Arrays.copyOf(valid, valid.length - 1),
        concat(valid, new byte[] {0}),
        new byte[] {0x30, (byte) 0x81, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01},
        new byte[] {0x30, 0x07, 0x02, 0x02, 0x00, 0x01, 0x02, 0x01, 0x01},
        new byte[] {0x30, 0x06, 0x02, 0x01, (byte) 0x80, 0x02, 0x01, 0x01},
        new byte[] {0x30, 0x06, 0x02, 0x01, 0x00, 0x02, 0x01, 0x01},
        new byte[] {0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x00},
        der(ORDER, BigInteger.ONE),
        der(BigInteger.ONE, ORDER),
        new byte[] {0x31, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01},
        new byte[] {0x30, 0x06, 0x03, 0x01, 0x01, 0x02, 0x01, 0x01});
    for (final byte[] encoded : cases) {
      assertThrows(
          Arrays.toString(encoded),
          IllegalArgumentException.class,
          () -> KagemushaP256Codec.rawLowSFromStrictDer(encoded));
    }
  }

  private static byte[] raw(final BigInteger r, final BigInteger s) {
    return concat(fixed(r), fixed(s));
  }

  private static byte[] fixed(final BigInteger value) {
    final byte[] signed = value.toByteArray();
    final int offset = signed.length == 33 && signed[0] == 0 ? 1 : 0;
    final int length = signed.length - offset;
    if (length > 32) throw new IllegalArgumentException("test scalar too large");
    final byte[] result = new byte[32];
    System.arraycopy(signed, offset, result, 32 - length, length);
    return result;
  }

  private static byte[] der(final BigInteger r, final BigInteger s) {
    final byte[] rInteger = derInteger(r);
    final byte[] sInteger = derInteger(s);
    final byte[] body = concat(rInteger, sInteger);
    return concat(new byte[] {0x30, (byte) body.length}, body);
  }

  private static byte[] derInteger(final BigInteger value) {
    final byte[] encoded = value.toByteArray();
    return concat(new byte[] {0x02, (byte) encoded.length}, encoded);
  }

  private static byte[] mutate(
      final byte[] value, final int index, final byte replacement) {
    value[index] = replacement;
    return value;
  }

  private static byte[] concat(final byte[] left, final byte[] right) {
    final byte[] result = Arrays.copyOf(left, left.length + right.length);
    System.arraycopy(right, 0, result, left.length, right.length);
    return result;
  }

  private static byte[] hex(final String value) {
    final byte[] result = new byte[value.length() / 2];
    for (int index = 0; index < result.length; index++) {
      result[index] = (byte) Integer.parseInt(value.substring(index * 2, index * 2 + 2), 16);
    }
    return result;
  }
}
