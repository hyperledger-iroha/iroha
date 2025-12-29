package org.hyperledger.iroha.android.crypto;

/** Minimal BLAKE2s implementation supporting keyed hashing. */
public final class Blake2s {

  private static final int BLOCK_BYTES = 64;
  private static final int ROUNDS = 10;
  private static final int[] IV = {
      0x6a09e667,
      0xbb67ae85,
      0x3c6ef372,
      0xa54ff53a,
      0x510e527f,
      0x9b05688c,
      0x1f83d9ab,
      0x5be0cd19
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
      {10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0}
  };

  private Blake2s() {}

  public static byte[] digest(final byte[] message, final byte[] key, final int outLen) {
    if (outLen <= 0 || outLen > 32) {
      throw new IllegalArgumentException("BLAKE2s output length must be between 1 and 32 bytes");
    }
    if (key.length > 32) {
      throw new IllegalArgumentException("BLAKE2s key must be at most 32 bytes");
    }

    final int[] h = IV.clone();
    final int param = outLen | (key.length << 8) | (1 << 16) | (1 << 24);
    h[0] ^= param;

    int t0 = 0;
    int t1 = 0;

    if (key.length > 0) {
      final byte[] block = new byte[BLOCK_BYTES];
      System.arraycopy(key, 0, block, 0, key.length);
      final long sum = (t0 & 0xFFFFFFFFL) + BLOCK_BYTES;
      t0 = (int) sum;
      if ((sum >>> 32) != 0) {
        t1++;
      }
      compress(h, block, 0, t0, t1, message.length == 0);
    }

    final int total = message.length;
    if (total > 0) {
      int offset = 0;
      final int fullBlocks = total / BLOCK_BYTES;
      final int remainder = total % BLOCK_BYTES;

      for (int i = 0; i < fullBlocks; i++) {
        offset += BLOCK_BYTES;
        final long sum = (t0 & 0xFFFFFFFFL) + BLOCK_BYTES;
        t0 = (int) sum;
        if ((sum >>> 32) != 0) {
          t1++;
        }
        final boolean isLastFull = (i == fullBlocks - 1) && remainder == 0;
        compress(h, message, offset - BLOCK_BYTES, t0, t1, isLastFull);
      }

      if (remainder > 0) {
        final byte[] block = new byte[BLOCK_BYTES];
        System.arraycopy(message, total - remainder, block, 0, remainder);
        final long sum = (t0 & 0xFFFFFFFFL) + remainder;
        t0 = (int) sum;
        if ((sum >>> 32) != 0) {
          t1++;
        }
        compress(h, block, 0, t0, t1, true);
      }
    } else if (key.length == 0) {
      final byte[] block = new byte[BLOCK_BYTES];
      compress(h, block, 0, 0, 0, true);
    }

    return toOutput(h, outLen);
  }

  private static void compress(
      final int[] h,
      final byte[] block,
      final int blockOffset,
      final int t0,
      final int t1,
      final boolean last) {
    final int[] m = new int[16];
    for (int i = 0; i < 16; i++) {
      m[i] = readIntLE(block, blockOffset + i * 4);
    }

    final int[] v = new int[16];
    System.arraycopy(h, 0, v, 0, 8);
    System.arraycopy(IV, 0, v, 8, 8);
    v[12] ^= t0;
    v[13] ^= t1;
    if (last) {
      v[14] ^= 0xFFFFFFFF;
    }

    for (int round = 0; round < ROUNDS; round++) {
      final byte[] s = SIGMA[round];
      G(v, 0, 4, 8, 12, m[s[0] & 0xFF], m[s[1] & 0xFF]);
      G(v, 1, 5, 9, 13, m[s[2] & 0xFF], m[s[3] & 0xFF]);
      G(v, 2, 6, 10, 14, m[s[4] & 0xFF], m[s[5] & 0xFF]);
      G(v, 3, 7, 11, 15, m[s[6] & 0xFF], m[s[7] & 0xFF]);
      G(v, 0, 5, 10, 15, m[s[8] & 0xFF], m[s[9] & 0xFF]);
      G(v, 1, 6, 11, 12, m[s[10] & 0xFF], m[s[11] & 0xFF]);
      G(v, 2, 7, 8, 13, m[s[12] & 0xFF], m[s[13] & 0xFF]);
      G(v, 3, 4, 9, 14, m[s[14] & 0xFF], m[s[15] & 0xFF]);
    }

    for (int i = 0; i < 8; i++) {
      h[i] ^= v[i] ^ v[i + 8];
    }
  }

  private static void G(
      final int[] v,
      final int a,
      final int b,
      final int c,
      final int d,
      final int x,
      final int y) {
    v[a] = v[a] + v[b] + x;
    v[d] = Integer.rotateRight(v[d] ^ v[a], 16);
    v[c] = v[c] + v[d];
    v[b] = Integer.rotateRight(v[b] ^ v[c], 12);
    v[a] = v[a] + v[b] + y;
    v[d] = Integer.rotateRight(v[d] ^ v[a], 8);
    v[c] = v[c] + v[d];
    v[b] = Integer.rotateRight(v[b] ^ v[c], 7);
  }

  private static int readIntLE(final byte[] data, final int offset) {
    return (data[offset] & 0xFF)
        | ((data[offset + 1] & 0xFF) << 8)
        | ((data[offset + 2] & 0xFF) << 16)
        | ((data[offset + 3] & 0xFF) << 24);
  }

  private static byte[] toOutput(final int[] h, final int outLen) {
    final byte[] out = new byte[outLen];
    int idx = 0;
    for (int value : h) {
      for (int j = 0; j < 4 && idx < outLen; j++) {
        out[idx++] = (byte) (value & 0xFF);
        value >>>= 8;
      }
      if (idx >= outLen) {
        break;
      }
    }
    return out;
  }
}
