package org.hyperledger.iroha.android.privacy;

import java.math.BigInteger;

/** Pasta-field compressor matching {@code poseidon_pair} in confidential-v2 Rust code. */
public final class PastaPoseidonNodeHasher implements ZkAssetMerkleHasher {
  private static final BigInteger MODULUS =
      new BigInteger("40000000000000000000000000000000224698fc094cf91b992d30ed00000001", 16);
  private static final BigInteger TWO = BigInteger.valueOf(2L);
  private static final BigInteger THREE = BigInteger.valueOf(3L);
  private static final BigInteger SEVEN = BigInteger.valueOf(7L);
  private static final BigInteger THIRTEEN = BigInteger.valueOf(13L);
  private static final PastaPoseidonNodeHasher INSTANCE = new PastaPoseidonNodeHasher();

  private PastaPoseidonNodeHasher() {}

  public static PastaPoseidonNodeHasher instance() {
    return INSTANCE;
  }

  @Override
  public byte[] hashPair(final byte[] left, final byte[] right) {
    final BigInteger lhs = littleEndianScalar(left, "left").add(SEVEN).mod(MODULUS);
    final BigInteger rhs = littleEndianScalar(right, "right").add(THIRTEEN).mod(MODULUS);
    final BigInteger value =
        TWO.multiply(pow5(lhs)).add(THREE.multiply(pow5(rhs))).mod(MODULUS);
    return scalarToLittleEndian(value);
  }

  private static BigInteger pow5(final BigInteger value) {
    final BigInteger square = value.multiply(value).mod(MODULUS);
    final BigInteger fourth = square.multiply(square).mod(MODULUS);
    return fourth.multiply(value).mod(MODULUS);
  }

  private static BigInteger littleEndianScalar(final byte[] bytes, final String field) {
    if (bytes == null || bytes.length != 32) {
      throw new IllegalArgumentException(field + " must be 32 bytes");
    }
    final byte[] bigEndian = bytes.clone();
    reverse(bigEndian);
    final BigInteger value = new BigInteger(1, bigEndian);
    if (value.compareTo(MODULUS) >= 0) {
      throw new IllegalArgumentException(field + " must be a canonical Pasta scalar");
    }
    return value;
  }

  private static byte[] scalarToLittleEndian(final BigInteger value) {
    byte[] bigEndian = value.mod(MODULUS).toByteArray();
    int first = 0;
    while (first < bigEndian.length && bigEndian[first] == 0) {
      first++;
    }
    final int size = bigEndian.length - first;
    if (size > 32) {
      throw new IllegalStateException("scalar encoding overflow");
    }
    final byte[] out = new byte[32];
    for (int i = 0; i < size; i++) {
      out[i] = bigEndian[bigEndian.length - 1 - i];
    }
    return out;
  }

  private static void reverse(final byte[] bytes) {
    for (int left = 0, right = bytes.length - 1; left < right; left++, right--) {
      final byte tmp = bytes[left];
      bytes[left] = bytes[right];
      bytes[right] = tmp;
    }
  }
}
