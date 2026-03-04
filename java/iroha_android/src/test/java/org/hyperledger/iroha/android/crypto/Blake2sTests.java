package org.hyperledger.iroha.android.crypto;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;

public final class Blake2sTests {

  private Blake2sTests() {}

  public static void main(final String[] args) {
    unkeyedDigestMatchesKnownVector();
    keyedDigestMatchesKnownVector();
    keyedDigestWithTruncatedOutput();
    invalidOutputLengthThrows();
    invalidKeyLengthThrows();
    System.out.println("[IrohaAndroid] Blake2s tests passed.");
  }

  private static void unkeyedDigestMatchesKnownVector() {
    final byte[] digest =
        Blake2s.digest("abc".getBytes(StandardCharsets.UTF_8), new byte[0], 32);
    final byte[] expected = hexToBytes(
        "508c5e8c327c14e2e1a72ba34eeb452f37458b209ed63a294d999b4c86675982");
    assert Arrays.equals(expected, digest) : "unkeyed digest must match reference vector";
  }

  private static void keyedDigestMatchesKnownVector() {
    final byte[] key = new byte[32];
    for (int i = 0; i < key.length; i++) {
      key[i] = (byte) i;
    }
    final byte[] digest =
        Blake2s.digest("abc".getBytes(StandardCharsets.UTF_8), key, 32);
    final byte[] expected = hexToBytes(
        "a281f725754969a702f6fe36fc591b7def866e4b70173ece402fc01c064d6b65");
    assert Arrays.equals(expected, digest) : "keyed digest must match reference vector";
  }

  private static void keyedDigestWithTruncatedOutput() {
    final byte[] digest =
        Blake2s.digest(
            "abc".getBytes(StandardCharsets.UTF_8),
            "key".getBytes(StandardCharsets.UTF_8),
            16);
    final byte[] expected = hexToBytes("94fdf6f35b9999920dcdcaee361ad435");
    assert Arrays.equals(expected, digest) : "digest truncation must match reference vector";
  }

  private static void invalidOutputLengthThrows() {
    boolean threw = false;
    try {
      Blake2s.digest(new byte[1], new byte[0], 0);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("output length");
    }
    assert threw : "expected invalid output length to throw";
  }

  private static void invalidKeyLengthThrows() {
    final byte[] key = new byte[33];
    boolean threw = false;
    try {
      Blake2s.digest(new byte[1], key, 32);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("key must be at most");
    }
    assert threw : "expected invalid key length to throw";
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
