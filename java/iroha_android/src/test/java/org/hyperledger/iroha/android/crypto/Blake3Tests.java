// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.crypto;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;

public final class Blake3Tests {

  private Blake3Tests() {}

  public static void main(final String[] args) {
    emptyInputProducesKnownHash();
    abcInputProducesKnownHash();
    hashIsDeterministic();
    inputExceedingChunkLenThrows();
    System.out.println("[IrohaAndroid] Blake3 tests passed.");
  }

  private static void emptyInputProducesKnownHash() {
    final byte[] hash = Blake3.hash(new byte[0]);
    final byte[] expected = hexToBytes(
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262");
    assert Arrays.equals(expected, hash) : "empty input must match BLAKE3 reference vector";
  }

  private static void abcInputProducesKnownHash() {
    final byte[] hash = Blake3.hash("abc".getBytes(StandardCharsets.UTF_8));
    final byte[] expected = hexToBytes(
        "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85");
    assert Arrays.equals(expected, hash) : "abc input must match BLAKE3 reference vector";
  }

  private static void hashIsDeterministic() {
    final byte[] input = "rose#wonderland".getBytes(StandardCharsets.UTF_8);
    final byte[] first = Blake3.hash(input);
    final byte[] second = Blake3.hash(input);
    assert Arrays.equals(first, second) : "hash must be deterministic across calls";
  }

  private static void inputExceedingChunkLenThrows() {
    boolean threw = false;
    try {
      Blake3.hash(new byte[1025]);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("Input too large");
    }
    assert threw : "expected input > 1024 bytes to throw";
  }

  private static byte[] hexToBytes(final String hex) {
    final byte[] out = new byte[hex.length() / 2];
    for (int i = 0; i < out.length; i++) {
      final int hi = Character.digit(hex.charAt(i * 2), 16);
      final int lo = Character.digit(hex.charAt(i * 2 + 1), 16);
      out[i] = (byte) ((hi << 4) | lo);
    }
    return out;
  }
}
