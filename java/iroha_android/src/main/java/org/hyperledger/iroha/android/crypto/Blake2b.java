package org.hyperledger.iroha.android.crypto;

import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.security.Provider;
import java.security.Security;
import java.util.Arrays;

/** Minimal BLAKE2b digest implementation (supports 256- and 512-bit output). */
public final class Blake2b {

  private static final int BLOCK_BYTES = 128;
  private static final int ROUNDS = 12;
  private static final long[] IV = {
      0x6a09e667f3bcc908L,
      0xbb67ae8584caa73bL,
      0x3c6ef372fe94f82bL,
      0xa54ff53a5f1d36f1L,
      0x510e527fade682d1L,
      0x9b05688c2b3e6c1fL,
      0x1f83d9abfb41bd6bL,
      0x5be0cd19137e2179L
  };

  private static final byte[][] SIGMA = {
      {0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15},
      {14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3},
      {11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4},
      {7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8},
      {9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13},
      {2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9},
      {12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11},
      {13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10},
      {6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5},
      {10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0},
      {0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15},
      {14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3}
  };

  private Blake2b() {}

  public static byte[] digest256(final byte[] message) {
    final byte[] providerDigest = tryMessageDigest("BLAKE2B-256", message);
    return providerDigest != null ? providerDigest : digest(message, 32);
  }

  public static byte[] digest512(final byte[] message) {
    final byte[] providerDigest = tryMessageDigest("BLAKE2B-512", message);
    return providerDigest != null ? providerDigest : digest(message, 64);
  }

  public static byte[] digest(final byte[] message) {
    return digest256(message);
  }

  public static byte[] digest(final byte[] message, final int outLen) {
    if (outLen <= 0 || outLen > 64) {
      throw new IllegalArgumentException("BLAKE2b output length must be between 1 and 64 bytes");
    }

    final long[] h = IV.clone();
    h[0] ^= 0x01010000L ^ outLen;

    long t0 = 0;
    long t1 = 0;
    final int total = message.length;

    if (total > 0) {
      int offset = 0;
      final int fullBlocks = total / BLOCK_BYTES;
      final int remainder = total % BLOCK_BYTES;

      for (int i = 0; i < fullBlocks; i++) {
        final boolean isLastFull = (i == fullBlocks - 1) && remainder == 0;
        t0 += BLOCK_BYTES;
        if (t0 < BLOCK_BYTES) {
          t1++;
        }
        compress(h, message, offset, t0, t1, isLastFull);
        offset += BLOCK_BYTES;
      }

      if (remainder > 0) {
        final byte[] block = new byte[BLOCK_BYTES];
        System.arraycopy(message, total - remainder, block, 0, remainder);
        t0 += remainder;
        if (t0 < remainder) {
          t1++;
        }
        compress(h, block, 0, t0, t1, true);
      }
    } else {
      final byte[] block = new byte[BLOCK_BYTES];
      compress(h, block, 0, 0, 0, true);
    }

    return toOutput(h, outLen);
  }

  private static byte[] tryMessageDigest(final String algorithm, final byte[] message) {
    try {
      final MessageDigest digest = MessageDigest.getInstance(algorithm);
      return digest.digest(message);
    } catch (final NoSuchAlgorithmException ignored) {
      try {
        final Class<?> providerClass =
            Class.forName("org.bouncycastle.jce.provider.BouncyCastleProvider");
        final Provider provider =
            (Provider) providerClass.getDeclaredConstructor().newInstance();
        if (Security.getProvider(provider.getName()) == null) {
          Security.addProvider(provider);
        }
        final MessageDigest digest = MessageDigest.getInstance(algorithm, provider);
        return digest.digest(message);
      } catch (final Exception ignoredAgain) {
        return null;
      }
    }
  }

  private static byte[] toOutput(final long[] h, final int outLen) {
    final byte[] out = new byte[outLen];
    int idx = 0;
    for (int i = 0; i < h.length && idx < outLen; i++) {
      long value = h[i];
      for (int j = 0; j < 8 && idx < outLen; j++) {
        out[idx++] = (byte) (value & 0xFF);
        value >>>= 8;
      }
    }
    return out;
  }

  private static void compress(
      final long[] h,
      final byte[] block,
      final int blockOffset,
      final long t0,
      final long t1,
      final boolean last) {
    final long[] m = new long[16];
    for (int i = 0; i < 16; i++) {
      m[i] = readLongLE(block, blockOffset + i * 8);
    }

    final long[] v = new long[16];
    System.arraycopy(h, 0, v, 0, 8);
    System.arraycopy(IV, 0, v, 8, 8);
    v[12] ^= t0;
    v[13] ^= t1;
    if (last) {
      v[14] = ~v[14];
    }

    for (int round = 0; round < ROUNDS; round++) {
      final byte[] s = SIGMA[round];
      G(v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
      G(v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
      G(v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
      G(v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
      G(v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
      G(v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
      G(v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
      G(v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
    }

    for (int i = 0; i < 8; i++) {
      h[i] ^= v[i] ^ v[i + 8];
    }
  }

  private static void G(
      final long[] v,
      final int a,
      final int b,
      final int c,
      final int d,
      final long x,
      final long y) {
    v[a] = v[a] + v[b] + x;
    v[d] = rotateRight(v[d] ^ v[a], 32);
    v[c] = v[c] + v[d];
    v[b] = rotateRight(v[b] ^ v[c], 24);
    v[a] = v[a] + v[b] + y;
    v[d] = rotateRight(v[d] ^ v[a], 16);
    v[c] = v[c] + v[d];
    v[b] = rotateRight(v[b] ^ v[c], 63);
  }

  private static long rotateRight(final long x, final int n) {
    return (x >>> n) | (x << (64 - n));
  }

  private static long readLongLE(final byte[] data, final int offset) {
    return ((long) data[offset] & 0xFF)
        | (((long) data[offset + 1] & 0xFF) << 8)
        | (((long) data[offset + 2] & 0xFF) << 16)
        | (((long) data[offset + 3] & 0xFF) << 24)
        | (((long) data[offset + 4] & 0xFF) << 32)
        | (((long) data[offset + 5] & 0xFF) << 40)
        | (((long) data[offset + 6] & 0xFF) << 48)
        | (((long) data[offset + 7] & 0xFF) << 56);
  }
}
