// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.crypto;

/**
 * Pure Java implementation of the BLAKE3 hash function.
 *
 * <p>Supports inputs up to the largest Solana AccountLtHash preimage used by the Android SDK, and
 * exposes BLAKE3 XOF output for Agave-compatible 2048-byte account contributions.
 *
 * <p>Reference: <a href="https://github.com/BLAKE3-team/BLAKE3-specs">BLAKE3 Specification</a>
 */
public final class Blake3 {

  private static final int BLOCK_LEN = 64;
  private static final int CHUNK_LEN = 1024;
  private static final int OUT_LEN = 32;
  private static final int MAX_INPUT_LEN = 8 + 65_536 + 1 + 32 + 32;

  private static final int CHUNK_START = 1;
  private static final int CHUNK_END = 2;
  private static final int PARENT = 4;
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
   * @param input the data to hash
   * @return the 32-byte BLAKE3 hash
   * @throws IllegalArgumentException if input exceeds the SDK helper limit
   */
  public static byte[] hash(byte[] input) {
    return derive(input, OUT_LEN);
  }

  /**
   * Computes BLAKE3 XOF output for the given input.
   *
   * @param input the data to hash
   * @param outputLength number of output bytes to derive
   * @return BLAKE3 XOF bytes
   * @throws IllegalArgumentException if input or output length exceeds SDK helper bounds
   */
  public static byte[] derive(final byte[] input, final int outputLength) {
    if (input.length > MAX_INPUT_LEN) {
      throw new IllegalArgumentException(
          "Input too large for Blake3 helper: " + input.length + " bytes (max " + MAX_INPUT_LEN + ")");
    }
    if (outputLength < 0) {
      throw new IllegalArgumentException("outputLength must not be negative");
    }
    final Output rootOutput = rootOutput(input);
    final byte[] output = new byte[outputLength];
    int cursor = 0;
    long outputBlockCounter = 0L;
    while (cursor < outputLength) {
      final int[] words = rootOutput.rootWords(outputBlockCounter);
      for (final int word : words) {
        if (cursor >= outputLength) break;
        output[cursor++] = (byte) word;
        if (cursor >= outputLength) break;
        output[cursor++] = (byte) (word >>> 8);
        if (cursor >= outputLength) break;
        output[cursor++] = (byte) (word >>> 16);
        if (cursor >= outputLength) break;
        output[cursor++] = (byte) (word >>> 24);
      }
      outputBlockCounter += 1;
    }
    return output;
  }

  private static Output rootOutput(final byte[] input) {
    final int chunkCount = Math.max(1, (input.length + CHUNK_LEN - 1) / CHUNK_LEN);
    return subtreeOutput(input, 0, chunkCount);
  }

  private static Output subtreeOutput(
      final byte[] input, final int chunkIndex, final int chunkCount) {
    if (chunkCount == 1) {
      final int offset = chunkIndex * CHUNK_LEN;
      final int length = Math.min(CHUNK_LEN, Math.max(0, input.length - offset));
      return chunkOutput(input, offset, length, chunkIndex);
    }
    final int leftCount = leftSubtreeChunkCount(chunkCount);
    final int[] left = subtreeOutput(input, chunkIndex, leftCount).chainingValue();
    final int[] right = subtreeOutput(input, chunkIndex + leftCount, chunkCount - leftCount).chainingValue();
    return parentOutput(left, right);
  }

  private static int leftSubtreeChunkCount(final int chunkCount) {
    int power = 1;
    while (power * 2 < chunkCount) {
      power *= 2;
    }
    return power;
  }

  private static Output chunkOutput(
      final byte[] input, final int offset, final int length, final long chunkCounter) {
    int[] cv = IV.clone();
    int numBlocks = Math.max(1, (length + BLOCK_LEN - 1) / BLOCK_LEN);

    for (int i = 0; i < numBlocks; i++) {
      int blockStart = offset + i * BLOCK_LEN;
      int blockLen = Math.min(BLOCK_LEN, Math.max(0, length - i * BLOCK_LEN));

      int[] blockWords = parseBlockWords(input, blockStart);

      int flags = 0;
      if (i == 0) flags |= CHUNK_START;
      if (i == numBlocks - 1) {
        return new Output(cv.clone(), blockWords, chunkCounter, blockLen, flags | CHUNK_END);
      }
      int[] state = compress(cv, blockWords, chunkCounter, blockLen, flags);
      for (int j = 0; j < 8; j++) {
        cv[j] = state[j] ^ state[j + 8];
      }
    }
    throw new IllegalStateException("Unreachable");
  }

  private static Output parentOutput(final int[] left, final int[] right) {
    final int[] blockWords = new int[16];
    for (int i = 0; i < 8; i++) {
      blockWords[i] = left[i];
      blockWords[i + 8] = right[i];
    }
    return new Output(IV.clone(), blockWords, 0L, BLOCK_LEN, PARENT);
  }

  private static int[] parseBlockWords(byte[] input, int blockStart) {
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

  private static final class Output {
    private final int[] inputChainingValue;
    private final int[] blockWords;
    private final long counter;
    private final int blockLen;
    private final int flags;

    private Output(
        final int[] inputChainingValue,
        final int[] blockWords,
        final long counter,
        final int blockLen,
        final int flags) {
      this.inputChainingValue = inputChainingValue;
      this.blockWords = blockWords;
      this.counter = counter;
      this.blockLen = blockLen;
      this.flags = flags;
    }

    private int[] chainingValue() {
      final int[] state = compress(inputChainingValue, blockWords, counter, blockLen, flags);
      final int[] out = new int[8];
      for (int i = 0; i < 8; i++) {
        out[i] = state[i] ^ state[i + 8];
      }
      return out;
    }

    private int[] rootWords(final long outputBlockCounter) {
      final int[] state =
          compress(inputChainingValue, blockWords, outputBlockCounter, blockLen, flags | ROOT);
      final int[] words = new int[16];
      for (int i = 0; i < 8; i++) {
        words[i] = state[i] ^ state[i + 8];
        words[i + 8] = state[i + 8] ^ inputChainingValue[i];
      }
      return words;
    }
  }
}
