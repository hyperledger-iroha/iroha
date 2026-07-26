package org.hyperledger.iroha.android.crypto;

import java.math.BigInteger;
import java.util.Arrays;
import org.bouncycastle.math.ec.rfc8032.Ed25519;

/** Strict admission checks for canonical prime-order Ed25519 public keys. */
public final class Ed25519PublicKeyAdmission {

  /** Canonical compressed Ed25519 public-key length. */
  public static final int PUBLIC_KEY_LENGTH = 32;

  private static final BigInteger TWO = BigInteger.valueOf(2L);
  private static final BigInteger FIELD_MODULUS =
      BigInteger.ONE.shiftLeft(255).subtract(BigInteger.valueOf(19L));
  private static final BigInteger SUBGROUP_ORDER =
      BigInteger.ONE
          .shiftLeft(252)
          .add(new BigInteger("27742317777372353535851937790883648493"));
  private static final BigInteger CURVE_D =
      BigInteger.valueOf(-121665L)
          .multiply(BigInteger.valueOf(121666L).modInverse(FIELD_MODULUS))
          .mod(FIELD_MODULUS);
  private static final BigInteger TWO_CURVE_D = TWO.multiply(CURVE_D).mod(FIELD_MODULUS);
  private static final BigInteger SQRT_MINUS_ONE =
      TWO.modPow(FIELD_MODULUS.subtract(BigInteger.ONE).shiftRight(2), FIELD_MODULUS);
  private static final BigInteger SQRT_EXPONENT =
      FIELD_MODULUS.add(BigInteger.valueOf(3L)).shiftRight(3);
  private static final ExtendedPoint EXTENDED_IDENTITY =
      new ExtendedPoint(BigInteger.ZERO, BigInteger.ONE, BigInteger.ONE, BigInteger.ZERO);

  private Ed25519PublicKeyAdmission() {}

  /** Returns {@code true} only for canonical points in the prime-order Ed25519 subgroup. */
  public static boolean isValid(final byte[] publicKey) {
    if (publicKey == null || publicKey.length != PUBLIC_KEY_LENGTH) {
      return false;
    }

    final byte[] encoded = Arrays.copyOf(publicKey, publicKey.length);
    // Bouncy Castle's partial validator is only a curve/decompression
    // prefilter. Prime-order admission is enforced explicitly below so
    // mixed-torsion points cannot pass through provider-specific behavior.
    if (!Ed25519.validatePublicKeyPartial(encoded, 0)) {
      return false;
    }
    final ExtendedPoint point = decodeCanonicalPoint(encoded);
    if (point == null || isIdentity(point)) {
      return false;
    }
    return isIdentity(scalarMultiply(point, SUBGROUP_ORDER));
  }

  private static ExtendedPoint decodeCanonicalPoint(final byte[] encoded) {
    final int sign = (encoded[PUBLIC_KEY_LENGTH - 1] >>> 7) & 1;
    final byte[] yBytes = Arrays.copyOf(encoded, encoded.length);
    yBytes[PUBLIC_KEY_LENGTH - 1] &= 0x7F;
    final BigInteger y = decodeLittleEndian(yBytes);
    if (y.compareTo(FIELD_MODULUS) >= 0) {
      return null;
    }

    final BigInteger ySquared = mod(y.multiply(y));
    final BigInteger numerator = mod(ySquared.subtract(BigInteger.ONE));
    final BigInteger denominator = mod(CURVE_D.multiply(ySquared).add(BigInteger.ONE));
    final BigInteger xSquared;
    try {
      xSquared = mod(numerator.multiply(denominator.modInverse(FIELD_MODULUS)));
    } catch (final ArithmeticException ignored) {
      return null;
    }
    BigInteger x = xSquared.modPow(SQRT_EXPONENT, FIELD_MODULUS);
    if (!mod(x.multiply(x)).equals(xSquared)) {
      x = mod(x.multiply(SQRT_MINUS_ONE));
    }
    if (!mod(x.multiply(x)).equals(xSquared)) {
      return null;
    }
    if ((x.testBit(0) ? 1 : 0) != sign) {
      x = FIELD_MODULUS.subtract(x).mod(FIELD_MODULUS);
    }
    if (x.signum() == 0 && sign == 1) {
      return null;
    }
    if (!Arrays.equals(encodeCanonicalPoint(x, y), encoded)) {
      return null;
    }

    return new ExtendedPoint(x, y, BigInteger.ONE, mod(x.multiply(y)));
  }

  private static byte[] encodeCanonicalPoint(final BigInteger x, final BigInteger y) {
    final byte[] encoded = encodeLittleEndian(y, PUBLIC_KEY_LENGTH);
    if (x.testBit(0)) {
      encoded[PUBLIC_KEY_LENGTH - 1] |= (byte) 0x80;
    }
    return encoded;
  }

  private static BigInteger decodeLittleEndian(final byte[] encoded) {
    final byte[] bigEndian = new byte[encoded.length];
    for (int index = 0; index < encoded.length; index++) {
      bigEndian[index] = encoded[encoded.length - 1 - index];
    }
    return new BigInteger(1, bigEndian);
  }

  private static byte[] encodeLittleEndian(final BigInteger value, final int size) {
    final byte[] bigEndian = value.toByteArray();
    final byte[] encoded = new byte[size];
    final int copied = Math.min(size, bigEndian.length);
    for (int index = 0; index < copied; index++) {
      encoded[index] = bigEndian[bigEndian.length - 1 - index];
    }
    return encoded;
  }

  private static ExtendedPoint scalarMultiply(
      final ExtendedPoint point, final BigInteger scalar) {
    ExtendedPoint result = EXTENDED_IDENTITY;
    ExtendedPoint addend = point;
    BigInteger value = scalar;
    while (value.signum() != 0) {
      if (value.testBit(0)) {
        result = add(result, addend);
      }
      addend = add(addend, addend);
      value = value.shiftRight(1);
    }
    return result;
  }

  private static ExtendedPoint add(final ExtendedPoint left, final ExtendedPoint right) {
    final BigInteger a =
        mod(left.y.subtract(left.x).multiply(right.y.subtract(right.x)));
    final BigInteger b =
        mod(left.y.add(left.x).multiply(right.y.add(right.x)));
    final BigInteger c = mod(TWO_CURVE_D.multiply(left.t).multiply(right.t));
    final BigInteger d = mod(TWO.multiply(left.z).multiply(right.z));
    final BigInteger e = mod(b.subtract(a));
    final BigInteger f = mod(d.subtract(c));
    final BigInteger g = mod(d.add(c));
    final BigInteger h = mod(b.add(a));
    return new ExtendedPoint(
        mod(e.multiply(f)),
        mod(g.multiply(h)),
        mod(f.multiply(g)),
        mod(e.multiply(h)));
  }

  private static boolean isIdentity(final ExtendedPoint point) {
    final BigInteger z = mod(point.z);
    return z.signum() != 0 && mod(point.x).signum() == 0 && mod(point.y).equals(z);
  }

  private static BigInteger mod(final BigInteger value) {
    return value.mod(FIELD_MODULUS);
  }

  private static final class ExtendedPoint {
    private final BigInteger x;
    private final BigInteger y;
    private final BigInteger z;
    private final BigInteger t;

    private ExtendedPoint(
        final BigInteger x,
        final BigInteger y,
        final BigInteger z,
        final BigInteger t) {
      this.x = x;
      this.y = y;
      this.z = z;
      this.t = t;
    }
  }
}
