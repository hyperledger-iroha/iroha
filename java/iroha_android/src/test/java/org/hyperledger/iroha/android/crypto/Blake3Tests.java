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
    xofOutputProducesKnownHashPrefix();
    twoChunkInputProducesKnownHash();
    hashIsDeterministic();
    inputExceedingHelperLimitThrows();
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

  private static void xofOutputProducesKnownHashPrefix() {
    final byte[] hash = Blake3.derive("abc".getBytes(StandardCharsets.UTF_8), 64);
    final byte[] expected = hexToBytes(
        "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
            + "1fb250ae7393f5d02813b65d521a0d492d9ba09cf7ce7f4cffd900f23374bf0b");
    assert Arrays.equals(expected, hash) : "BLAKE3 XOF output must match reference vector";
  }

  private static void hashIsDeterministic() {
    final byte[] input = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM".getBytes(StandardCharsets.UTF_8);
    final byte[] first = Blake3.hash(input);
    final byte[] second = Blake3.hash(input);
    assert Arrays.equals(first, second) : "hash must be deterministic across calls";
  }

  private static void twoChunkInputProducesKnownHash() {
    final byte[] hash = Blake3.hash(new byte[2048]);
    final byte[] expected = hexToBytes(
        "be2a8de3dcf46c94ce85cdc8e07ac308f4d8a95490d956c38d780fd610db0813");
    assert Arrays.equals(expected, hash) : "2048-byte input must match BLAKE3 reference vector";
  }

  private static void inputExceedingHelperLimitThrows() {
    boolean threw = false;
    try {
      Blake3.hash(new byte[65610]);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("Input too large");
    }
    assert threw : "expected input above Solana AccountLtHash preimage size to throw";
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
