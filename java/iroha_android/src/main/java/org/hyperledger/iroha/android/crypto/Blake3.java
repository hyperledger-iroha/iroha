// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.crypto;

/**
 * Pure Java implementation of the BLAKE3 hash function.
 *
 * <p>Supports inputs up to 1024 bytes (single chunk). This covers all asset definition ID
 * strings used for computing {@code aid:} identifiers from {@code "name#domain"} format.
 *
 * <p>Reference: <a href="https://github.com/BLAKE3-team/BLAKE3-specs">BLAKE3 Specification</a>
 */
public final class Blake3 {

  private static final int BLOCK_LEN = 64;
  private static final int CHUNK_LEN = 1024;

  private static final int CHUNK_START = 1;
  private static final int CHUNK_END = 2;
  private static final int ROOT = 8;

  private static final int[] IV = {
      0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
      0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19
  };

  private static final int[] MSG_PERMUTATION = {
      2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8
  };

  private Blake3() {}

  /**
   * Computes the BLAKE3 hash of the given input.
   *
   * @param input the data to hash (must be at most 1024 bytes)
   * @return the 32-byte BLAKE3 hash
   * @throws IllegalArgumentException if input exceeds 1024 bytes
   */
  public static byte[] hash(byte[] input) {
    if (input.length > CHUNK_LEN) {
      throw new IllegalArgumentException(
          "Input too large for single-chunk Blake3: " + input.length + " bytes (max " + CHUNK_LEN + ")");
    }

    int[] cv = IV.clone();
    int numBlocks = Math.max(1, (input.length + BLOCK_LEN - 1) / BLOCK_LEN);

    for (int i = 0; i < numBlocks; i++) {
      int blockStart = i * BLOCK_LEN;
      int blockLen = Math.min(BLOCK_LEN, Math.max(0, input.length - blockStart));

      int[] blockWords = parseBlockWords(input, blockStart, blockLen);

      int flags = 0;
      if (i == 0) flags |= CHUNK_START;
      if (i == numBlocks - 1) flags |= CHUNK_END | ROOT;

      int[] state = compress(cv, blockWords, 0, blockLen, flags);

      if (i < numBlocks - 1) {
        for (int j = 0; j < 8; j++) {
          cv[j] = state[j] ^ state[j + 8];
        }
      } else {
        return rootOutput(state);
      }
    }
    throw new IllegalStateException("Unreachable");
  }

  private static byte[] rootOutput(int[] state) {
    byte[] output = new byte[32];
    for (int j = 0; j < 8; j++) {
      int word = state[j] ^ state[j + 8];
      output[j * 4] = (byte) word;
      output[j * 4 + 1] = (byte) (word >>> 8);
      output[j * 4 + 2] = (byte) (word >>> 16);
      output[j * 4 + 3] = (byte) (word >>> 24);
    }
    return output;
  }

  private static int[] parseBlockWords(byte[] input, int blockStart, int blockLen) {
    int[] words = new int[16];
    for (int j = 0; j < 16; j++) {
      int offset = blockStart + j * 4;
      int b0 = (offset < input.length) ? (input[offset] & 0xFF) : 0;
      int b1 = (offset + 1 < input.length) ? (input[offset + 1] & 0xFF) : 0;
      int b2 = (offset + 2 < input.length) ? (input[offset + 2] & 0xFF) : 0;
      int b3 = (offset + 3 < input.length) ? (input[offset + 3] & 0xFF) : 0;
      words[j] = b0 | (b1 << 8) | (b2 << 16) | (b3 << 24);
    }
    return words;
  }

  private static int[] compress(int[] cv, int[] blockWords, long counter, int blockLen, int flags) {
    int[] state = {
        cv[0], cv[1], cv[2], cv[3],
        cv[4], cv[5], cv[6], cv[7],
        IV[0], IV[1], IV[2], IV[3],
        (int) (counter & 0xFFFFFFFFL), (int) ((counter >>> 32) & 0xFFFFFFFFL),
        blockLen, flags
    };
    int[] msg = blockWords.clone();

    for (int round = 0; round < 7; round++) {
      // Column step
      g(state, 0, 4, 8, 12, msg[0], msg[1]);
      g(state, 1, 5, 9, 13, msg[2], msg[3]);
      g(state, 2, 6, 10, 14, msg[4], msg[5]);
      g(state, 3, 7, 11, 15, msg[6], msg[7]);
      // Diagonal step
      g(state, 0, 5, 10, 15, msg[8], msg[9]);
      g(state, 1, 6, 11, 12, msg[10], msg[11]);
      g(state, 2, 7, 8, 13, msg[12], msg[13]);
      g(state, 3, 4, 9, 14, msg[14], msg[15]);

      if (round < 6) {
        msg = permute(msg);
      }
    }

    return state;
  }

  private static void g(int[] state, int a, int b, int c, int d, int mx, int my) {
    state[a] = state[a] + state[b] + mx;
    state[d] = Integer.rotateRight(state[d] ^ state[a], 16);
    state[c] = state[c] + state[d];
    state[b] = Integer.rotateRight(state[b] ^ state[c], 12);
    state[a] = state[a] + state[b] + my;
    state[d] = Integer.rotateRight(state[d] ^ state[a], 8);
    state[c] = state[c] + state[d];
    state[b] = Integer.rotateRight(state[b] ^ state[c], 7);
  }

  private static int[] permute(int[] msg) {
    int[] permuted = new int[16];
    for (int i = 0; i < 16; i++) {
      permuted[i] = msg[MSG_PERMUTATION[i]];
    }
    return permuted;
  }
}
